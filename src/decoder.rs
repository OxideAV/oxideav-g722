//! G.722 decoder (64 / 56 / 48 kbit/s).
//!
//! Accepts packets of arbitrary byte length; each byte is one G.722
//! sample-serial code. The layout inside the byte depends on the mode
//! (see [`crate::mode::Mode::unpack`]): at every rate the high-band 2-bit
//! code sits in the low 2 bits, and the low-band code (6 / 5 / 4 bits)
//! sits in the top bits, optionally separated from the high-band by 1 or
//! 2 auxiliary bits. Every byte produces two 16-bit PCM samples at 16 kHz
//! mono.

use std::collections::VecDeque;

use oxideav_codec::Decoder;
use oxideav_core::{
    AudioFrame, CodecId, CodecParameters, Error, Frame, Packet, Result, SampleFormat, TimeBase,
};

use crate::band_high::HighBand;
use crate::band_low::LowBand;
use crate::mode::Mode;
use crate::qmf::QmfSynthesis;
use crate::CODEC_ID_STR;

/// Default sample rate for G.722 output (16 kHz wideband).
pub const SAMPLE_RATE_HZ: u32 = 16_000;

pub fn make_decoder(params: &CodecParameters) -> Result<Box<dyn Decoder>> {
    let sample_rate = params.sample_rate.unwrap_or(SAMPLE_RATE_HZ);
    if sample_rate != SAMPLE_RATE_HZ {
        return Err(Error::unsupported(format!(
            "G.722: only {SAMPLE_RATE_HZ} Hz is supported (got {sample_rate})"
        )));
    }
    let channels = params.channels.unwrap_or(1);
    if channels != 1 {
        return Err(Error::unsupported(format!(
            "G.722: only mono is supported (got {channels} channels)"
        )));
    }
    let mode = Mode::from_bit_rate(params.bit_rate)?;
    Ok(Box::new(G722Decoder::with_mode(mode)))
}

pub struct G722Decoder {
    codec_id: CodecId,
    mode: Mode,
    low: LowBand,
    high: HighBand,
    qmf: QmfSynthesis,
    pending: VecDeque<Frame>,
    drained: bool,
    next_pts: i64,
    time_base: TimeBase,
    /// Auxiliary side-channel bytes recovered from packed G.722 bytes.
    /// One entry per decoded byte, holding `mode.aux_bits()` bits right-
    /// aligned (always 0 in Mode 1). Drained via [`Self::take_aux`].
    aux_queue: VecDeque<u8>,
}

impl G722Decoder {
    pub fn new() -> Self {
        Self::with_mode(Mode::Mode1)
    }

    pub fn with_mode(mode: Mode) -> Self {
        Self {
            codec_id: CodecId::new(CODEC_ID_STR),
            mode,
            low: LowBand::for_mode(mode),
            high: HighBand::new(),
            qmf: QmfSynthesis::new(),
            pending: VecDeque::new(),
            drained: false,
            next_pts: 0,
            time_base: TimeBase::new(1, SAMPLE_RATE_HZ as i64),
            aux_queue: VecDeque::new(),
        }
    }

    /// The G.722 operating mode this decoder was constructed with.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Drain accumulated auxiliary side-channel bytes recovered from packed
    /// G.722 bytes. Each entry contains `mode.aux_bits()` bits right-aligned
    /// (always 0 on Mode 1, since no aux data is carried at 64 kbit/s). The
    /// queue is filled in send-packet order — one entry per decoded byte.
    pub fn take_aux(&mut self) -> Vec<u8> {
        self.aux_queue.drain(..).collect()
    }

    /// Number of queued auxiliary bytes still waiting to be drained.
    pub fn pending_aux(&self) -> usize {
        self.aux_queue.len()
    }
}

impl Default for G722Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for G722Decoder {
    fn codec_id(&self) -> &CodecId {
        &self.codec_id
    }

    fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        if packet.data.is_empty() {
            // Empty packets are tolerated — produce no output.
            return Ok(());
        }
        // Each input byte → 2 output samples at 16 kHz.
        let n_samples = packet.data.len() * 2;
        let mut out_pcm = Vec::<i16>::with_capacity(n_samples);
        for &byte in &packet.data {
            let (il, ih, aux) = self.mode.unpack_with_aux(byte);
            self.aux_queue.push_back(aux);
            let rl = self.low.decode(il);
            let rh = self.high.decode(ih);
            let (s0, s1) = self.qmf.process(rl as i16, rh as i16);
            out_pcm.push(s0);
            out_pcm.push(s1);
        }
        // Pack into little-endian byte buffer.
        let mut bytes = Vec::with_capacity(n_samples * 2);
        for s in &out_pcm {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        let pts = packet.pts.or(Some(self.next_pts));
        self.next_pts = pts.unwrap_or(self.next_pts) + n_samples as i64;

        self.pending.push_back(Frame::Audio(AudioFrame {
            format: SampleFormat::S16,
            channels: 1,
            sample_rate: SAMPLE_RATE_HZ,
            samples: n_samples as u32,
            pts,
            time_base: self.time_base,
            data: vec![bytes],
        }));
        Ok(())
    }

    fn receive_frame(&mut self) -> Result<Frame> {
        if let Some(f) = self.pending.pop_front() {
            return Ok(f);
        }
        if self.drained {
            return Err(Error::Eof);
        }
        Err(Error::NeedMore)
    }

    fn flush(&mut self) -> Result<()> {
        self.drained = true;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        // Wipe the two sub-band adaptive quantiser states (predictor memory,
        // log-scale, pole/zero history) and the QMF synthesis filter history
        // so the next packet decodes as if it were the first. Config fields
        // (codec_id, time_base, mode) are left untouched. `next_pts` is
        // zeroed so auto-assigned PTS restart at 0 post-seek.
        self.low = LowBand::for_mode(self.mode);
        self.high = HighBand::new();
        self.qmf = QmfSynthesis::new();
        self.pending.clear();
        self.drained = false;
        self.next_pts = 0;
        self.aux_queue.clear();
        Ok(())
    }
}

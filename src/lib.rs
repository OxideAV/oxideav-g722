//! ITU-T G.722 wideband sub-band ADPCM codec.
//!
//! G.722 compresses 7 kHz-bandwidth speech / audio sampled at 16 kHz mono
//! into a nominal 64 / 56 / 48 kbit/s bitstream. The codec works by
//! splitting the input into two 4 kHz sub-bands via a 24-tap QMF, and
//! running a backward-adaptive ADPCM quantiser on each band independently:
//!
//! - Low-band (0..4 kHz): 4 / 5 / 6-bit quantiser (rate-dependent) with a
//!   2-pole / 6-zero predictor.
//! - High-band (4..8 kHz): 2-bit quantiser with the same predictor topology
//!   at every rate.
//!
//! On encode the two bands are packed into an 8-bit sample-serial byte:
//! low-band in the high bits, high-band in the low 2 bits. One byte
//! therefore describes two PCM samples worth of audio — hence 64 kbit/s
//! at 16 kHz (8 bits × 8000 packed pairs / sec). At the lower rates one
//! or two auxiliary bits sit between the low-band and high-band fields;
//! see [`mode::Mode`] for the exact bit layout. By default these are zero
//! on encode and discarded on decode, but the encoder / decoder also expose
//! `push_aux` / `take_aux` to carry an 8 kbit/s (Mode 2) or 16 kbit/s
//! (Mode 3) side-channel through them.
//!
//! # Operating modes
//!
//! | Mode | Bit rate  | Low-band bits | Aux bits | Selected via `CodecParameters::bit_rate` |
//! |------|-----------|---------------|----------|------------------------------------------|
//! | 1    | 64 kbit/s | 6             | 0        | `Some(64000)` or `None`                  |
//! | 2    | 56 kbit/s | 5             | 1        | `Some(56000)`                            |
//! | 3    | 48 kbit/s | 4             | 2        | `Some(48000)`                            |
//!
//! Both encoder and decoder default to Mode 1 (64 kbit/s) when no bit rate
//! is specified.
//!
//! # Scope
//!
//! - The optional G.722 sync word / framing is **not** emitted — this crate
//!   is transparent on byte boundaries, leaving framing to the surrounding
//!   container.
//! - Explicit reset-on-keyframe — the decoder runs continuously across
//!   packet boundaries, which matches how G.722 is typically carried in
//!   RTP / SIP.
//! - The ADPCM pipeline uses the normative ITU-T G.722 tables: `QM6` /
//!   `QM5` / `QM4` / `QM2` for inverse quantisation, `Q6` / `ILN` / `ILP`
//!   for the 6-bit forward quantiser, `WL` / `WH` / `RL42` / `RH2` / `ILB`
//!   for the log-scale adapter, all available as constants in [`tables`].
//!   The 2-pole / 6-zero (low-band) and 2-pole / 1-zero (high-band)
//!   predictors follow BLOCK4 verbatim (see [`band_low`] / [`band_high`]).
//!   Per the spec the encoder uses `QM4` for its local reconstruction at
//!   every rate (INVQAL) — the decoder picks the rate-matched table —
//!   which is what enables the same encoder output to feed an any-rate
//!   decoder. Bit-exact interop with a canonical ITU-T bitstream has not
//!   yet been verified against ITU test vectors (the byte layout in this
//!   crate puts the low-band field in the high bits; SpanDSP's layout is
//!   the other way round).
//!
//! Reference: ITU-T Recommendation G.722 (09/2012) and its public-domain
//! reference implementation, plus SpanDSP / libg722 (Steve Underwood, public
//! domain) for the QMF coefficient convention.

#![allow(
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::doc_lazy_continuation,
    clippy::doc_overindented_list_items
)]

pub mod band_high;
pub mod band_low;
pub mod decoder;
pub mod encoder;
pub mod mode;
pub mod qmf;
pub mod tables;

use oxideav_core::{CodecCapabilities, CodecId, CodecTag};
use oxideav_core::{CodecInfo, CodecRegistry};

pub const CODEC_ID_STR: &str = "g722";

/// Register the G.722 decoder + encoder under the single codec id `"g722"`.
pub fn register(reg: &mut CodecRegistry) {
    let caps = CodecCapabilities::audio("g722_sw")
        .with_lossy(true)
        .with_intra_only(false)
        .with_max_channels(1)
        .with_max_sample_rate(16_000);
    // AVI / WAVEFORMATEX tag: WAVE_FORMAT_G722 = 0x0028.
    reg.register(
        CodecInfo::new(CodecId::new(CODEC_ID_STR))
            .capabilities(caps)
            .decoder(decoder::make_decoder)
            .encoder(encoder::make_encoder)
            .tag(CodecTag::wave_format(0x0028)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use oxideav_core::Encoder;
    use oxideav_core::{CodecParameters, SampleFormat};

    fn params() -> CodecParameters {
        let mut p = CodecParameters::audio(CodecId::new(CODEC_ID_STR));
        p.sample_rate = Some(16_000);
        p.channels = Some(1);
        p.sample_format = Some(SampleFormat::S16);
        p
    }

    #[test]
    fn registers_both_directions() {
        let mut reg = CodecRegistry::new();
        register(&mut reg);
        let id = CodecId::new(CODEC_ID_STR);
        assert!(reg.has_decoder(&id));
        assert!(reg.has_encoder(&id));
    }

    #[test]
    fn rejects_wrong_sample_rate() {
        let mut p = params();
        p.sample_rate = Some(8_000);
        assert!(decoder::make_decoder(&p).is_err());
        assert!(encoder::make_encoder(&p).is_err());
    }

    #[test]
    fn rejects_stereo() {
        let mut p = params();
        p.channels = Some(2);
        assert!(decoder::make_decoder(&p).is_err());
        assert!(encoder::make_encoder(&p).is_err());
    }

    #[test]
    fn accepts_all_standard_bit_rates() {
        for br in [48_000u64, 56_000, 64_000] {
            let mut p = params();
            p.bit_rate = Some(br);
            assert!(
                decoder::make_decoder(&p).is_ok(),
                "decoder must accept {br} bit/s"
            );
            assert!(
                encoder::make_encoder(&p).is_ok(),
                "encoder must accept {br} bit/s"
            );
        }
    }

    #[test]
    fn rejects_unknown_bit_rate() {
        let mut p = params();
        p.bit_rate = Some(32_000u64);
        assert!(matches!(
            decoder::make_decoder(&p),
            Err(oxideav_core::Error::Unsupported(_))
        ));
        assert!(matches!(
            encoder::make_encoder(&p),
            Err(oxideav_core::Error::Unsupported(_))
        ));
    }

    #[test]
    fn default_bit_rate_is_mode_1_64k() {
        // No bit_rate set → encoder should report 64 kbit/s as its output.
        let enc = encoder::make_encoder(&params()).expect("encoder");
        assert_eq!(enc.output_params().bit_rate, Some(64_000));
    }
}

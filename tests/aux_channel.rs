//! Auxiliary side-channel round-trip — push aux data into the encoder,
//! drain it out of the decoder, verify it matches.
//!
//! G.722 modes 2 and 3 reserve 1 / 2 LSBs in the low-band field for an
//! 8 kbit/s and 16 kbit/s side-channel respectively. Mode 1 has no aux
//! bits, so the aux queue stays empty there and `take_aux` returns zeros
//! (matching the on-wire layout of "no aux carried").

#[allow(unused_imports)]
use oxideav_codec::{Decoder, Encoder};
use oxideav_core::{AudioFrame, CodecId, CodecParameters, Frame, SampleFormat, TimeBase};
use oxideav_g722::{decoder::G722Decoder, encoder::G722Encoder, mode::Mode};

fn output_params(mode: Mode) -> CodecParameters {
    let mut p = CodecParameters::audio(CodecId::new(oxideav_g722::CODEC_ID_STR));
    p.sample_rate = Some(16_000);
    p.channels = Some(1);
    p.sample_format = Some(SampleFormat::S16);
    p.bit_rate = Some(mode.bit_rate());
    p
}

fn audio_frame(samples: &[i16]) -> Frame {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    Frame::Audio(AudioFrame {
        format: SampleFormat::S16,
        channels: 1,
        sample_rate: 16_000,
        samples: samples.len() as u32,
        pts: Some(0),
        time_base: TimeBase::new(1, 16_000),
        data: vec![bytes],
    })
}

fn run_aux_roundtrip(mode: Mode, n_pairs: usize, aux: &[u8]) -> Vec<u8> {
    let mut enc = G722Encoder::new(output_params(mode), mode);
    enc.push_aux(aux);
    // Generate `n_pairs * 2` mono S16 samples so the encoder produces
    // exactly `n_pairs` packed bytes — one byte per aux entry.
    let samples: Vec<i16> = (0..n_pairs * 2)
        .map(|n| ((n as f32 * 0.05).sin() * 4_000.0) as i16)
        .collect();
    enc.send_frame(&audio_frame(&samples)).expect("send_frame");
    enc.flush().expect("flush");

    let mut dec = G722Decoder::with_mode(mode);
    while let Ok(pkt) = enc.receive_packet() {
        dec.send_packet(&pkt).expect("send_packet");
        // Drain frames so the queue doesn't grow unbounded.
        while dec.receive_frame().is_ok() {}
    }
    dec.take_aux()
}

#[test]
fn mode1_has_no_aux_capacity() {
    // Pushing aux into Mode 1 is a no-op; decoder always sees zeros.
    let n_pairs = 64;
    let aux_in: Vec<u8> = (0..n_pairs as u8).collect();
    let aux_out = run_aux_roundtrip(Mode::Mode1, n_pairs, &aux_in);
    assert_eq!(aux_out.len(), n_pairs);
    assert!(
        aux_out.iter().all(|&b| b == 0),
        "Mode 1 carries no aux bits but decoder saw non-zero: {aux_out:?}"
    );
}

#[test]
fn mode2_aux_round_trip_8kbps() {
    // Mode 2 carries 1 aux bit per byte. Use only the low bit of each
    // input byte; the rest is masked off by `pack_with_aux`.
    let n_pairs = 200;
    let aux_in: Vec<u8> = (0..n_pairs).map(|i| (i & 1) as u8).collect();
    let aux_out = run_aux_roundtrip(Mode::Mode2, n_pairs, &aux_in);
    assert_eq!(aux_out.len(), n_pairs);
    for (i, (a, b)) in aux_in.iter().zip(aux_out.iter()).enumerate() {
        assert_eq!(*a & 0x01, *b, "aux mismatch at {i}: in={a} out={b}");
    }
}

#[test]
fn mode3_aux_round_trip_16kbps() {
    // Mode 3 carries 2 aux bits per byte. Cycle through 0..=3.
    let n_pairs = 200;
    let aux_in: Vec<u8> = (0..n_pairs).map(|i| (i & 0x03) as u8).collect();
    let aux_out = run_aux_roundtrip(Mode::Mode3, n_pairs, &aux_in);
    assert_eq!(aux_out.len(), n_pairs);
    for (i, (a, b)) in aux_in.iter().zip(aux_out.iter()).enumerate() {
        assert_eq!(*a & 0x03, *b, "aux mismatch at {i}: in={a} out={b}");
    }
}

#[test]
fn aux_queue_drains_partially() {
    // Push fewer aux bytes than packets — the encoder should pad the rest
    // with zeros, and the decoder should still report one aux entry per
    // decoded byte.
    let n_pairs = 64;
    let aux_in = vec![1u8, 0, 1, 1, 0, 1, 0, 0]; // 8 bytes
    let aux_out = run_aux_roundtrip(Mode::Mode2, n_pairs, &aux_in);
    assert_eq!(aux_out.len(), n_pairs);
    for (i, (a, b)) in aux_in.iter().zip(aux_out.iter()).enumerate() {
        assert_eq!(*a & 1, *b, "aux mismatch at {i}");
    }
    for (i, b) in aux_out.iter().enumerate().skip(aux_in.len()) {
        assert_eq!(*b, 0, "expected zero-padded aux at {i}");
    }
}

#[test]
fn pending_aux_reflects_queue_state() {
    let mut enc = G722Encoder::new(output_params(Mode::Mode2), Mode::Mode2);
    assert_eq!(enc.pending_aux(), 0);
    enc.push_aux(&[1, 0, 1, 0]);
    assert_eq!(enc.pending_aux(), 4);
}

#![no_main]

//! Full-range PCM through the encoder under fuzz-chosen chunking,
//! cross-checked against a one-shot encode, then decoded in all
//! three modes.
//!
//! The encoder documents a 14-bit uniform-PCM input domain (clause
//! 1.4.1) but its API takes `i32`; the transmit QMF saturates its
//! sub-band outputs to the Table 9 range before the ADPCM loops see
//! them, so the encoder must be total (and deterministic) over the
//! whole type. The fuzz input is reinterpreted as raw little-endian
//! i32 samples — deliberately including values that overdrive the
//! QMF accumulator — and the first byte picks a chunking pattern for
//! `encode_into`, exercising the odd-length `pending` buffering.
//!
//! Invariants asserted:
//!
//! - chunked and one-shot encodes of the same samples produce the
//!   identical octet stream (`pending` is invisible on the wire);
//! - one octet per sample pair;
//! - decoding the produced stream in each of the three modes stays
//!   inside the Table 9 output range.

use libfuzzer_sys::fuzz_target;
use oxideav_g722::{Decoder, Encoder, Mode};

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, rest)) = data.split_first() else {
        return;
    };
    let pcm: Vec<i32> = rest
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // One-shot reference.
    let mut one_shot = Encoder::new();
    let octets = one_shot.encode(&pcm);
    assert_eq!(octets.len(), pcm.len() / 2);

    // Fuzz-chosen chunking: chunk length cycles through 1..=ctl%7+1.
    let step = (ctl % 7) as usize + 1;
    let mut chunked_enc = Encoder::new();
    let mut chunked = Vec::new();
    for chunk in pcm.chunks(step) {
        chunked_enc.encode_into(chunk, &mut chunked);
    }
    assert_eq!(chunked, octets, "chunked encode diverged from one-shot");
    assert_eq!(chunked_enc.pending_samples(), pcm.len() % 2);

    for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
        let mut dec = Decoder::new(mode);
        let out = dec.decode(&octets);
        assert_eq!(out.len(), octets.len() * 2);
        assert!(
            out.iter().all(|s| (-16384..=16383).contains(s)),
            "{mode:?}: round-trip output escaped the Table 9 range"
        );
    }
});

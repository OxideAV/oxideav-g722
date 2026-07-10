#![no_main]

//! The 16-bit-PCM convention entry points (`encode_pcm16` /
//! `decode_pcm16`) under fuzz-chosen inputs, cross-checked against the
//! Table 9 native path they must shadow.
//!
//! The pcm16 convention is the clause 5.2 Note 2 rescaling freedom:
//! the same two sub-band ADPCM state machines behind a QMF normalised
//! one bit differently. Two invariants make that checkable without a
//! reference stream:
//!
//! - **Decode shadowing**: on the same octet stream, the pcm16 output
//!   must stay within one fine LSB of `2 ×` the 15-bit output at every
//!   sample, in every mode (the extra bit is true accumulator content,
//!   so exact `2 ×` equality is *not* required — but any tap-alignment
//!   or state divergence blows the bound apart immediately).
//! - **Encode chunk transparency**: fuzz-chosen odd chunkings through
//!   the shared pending-sample buffer must be invisible on the wire.
//!
//! Output-range totality (`i16` window) rides along for free.

use libfuzzer_sys::fuzz_target;
use oxideav_g722::{Decoder, Encoder, Mode};

const MODES: [Mode; 3] = [Mode::Mode1, Mode::Mode2, Mode::Mode3];

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, rest)) = data.split_first() else {
        return;
    };

    // Decode shadowing on the raw bytes as an octet stream.
    let mode = MODES[(ctl % 3) as usize];
    let mut d15 = Decoder::new(mode);
    let mut d16 = Decoder::new(mode);
    let coarse = d15.decode(rest);
    let fine = d16.decode_pcm16(rest);
    assert_eq!(coarse.len(), fine.len());
    for (i, (c, f)) in coarse.iter().zip(fine.iter()).enumerate() {
        let diff = i32::from(*f) - *c * 2;
        assert!(
            (-1..=1).contains(&diff),
            "sample {i}: pcm16 {f} vs 2x native {c}"
        );
    }

    // Encode chunk transparency on the bytes as i16 PCM.
    let pcm: Vec<i16> = rest
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    let mut whole_enc = Encoder::new();
    let whole = whole_enc.encode_pcm16(&pcm);
    assert_eq!(whole.len(), pcm.len() / 2);

    let mut chunked_enc = Encoder::new();
    let mut chunked = Vec::new();
    let mut rest_pcm = pcm.as_slice();
    let mut n = usize::from(ctl >> 2) % 7 + 1;
    while !rest_pcm.is_empty() {
        let take = n.min(rest_pcm.len());
        chunked_enc.encode_pcm16_into(&rest_pcm[..take], &mut chunked);
        rest_pcm = &rest_pcm[take..];
        n = (n * 5 + 3) % 11 + 1;
    }
    assert_eq!(chunked, whole, "pcm16 chunking visible on the wire");
});

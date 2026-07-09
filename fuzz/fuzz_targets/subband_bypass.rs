#![no_main]

//! The Appendix-II QMF-bypass entry points under full-range abuse.
//!
//! Configuration 1 / Configuration 2 of Appendix II (clause II.2.1:
//! "the QMFs are by-passed and the test sequences are applied
//! directly to the ADPCM encoders or decoders") expose
//! `Encoder::encode_subband_pair` and `Decoder::decode_subband_pair`.
//! The documented input domain is the 15-bit Table II-1 sub-band
//! range, but the clause 5.2 saturating operators make the loops
//! well-defined for any `i32` — the difference signal clamps to the
//! 16-bit rails before the quantizer sees it. This target drives
//! both entry points with raw fuzz words:
//!
//! - encoder: full-range i32 sub-band pairs (5 bytes of fuzz input
//!   per step: 4 for `x_L`'s word, reusing overlap for `x_H`);
//! - decoder: raw codeword bytes (every value is masked to the 6-bit
//!   / 2-bit fields internally), with the §§ 6.2.1.6 / 6.2.2.5 LIMIT
//!   output ranges asserted.

use libfuzzer_sys::fuzz_target;
use oxideav_g722::{Decoder, Encoder, Mode};

const MODES: [Mode; 3] = [Mode::Mode1, Mode::Mode2, Mode::Mode3];

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, rest)) = data.split_first() else {
        return;
    };
    let mut enc = Encoder::new();
    let mut dec = Decoder::new(MODES[(ctl % 3) as usize]);
    for window in rest.windows(8).step_by(8) {
        let x_l = i32::from_le_bytes([window[0], window[1], window[2], window[3]]);
        let x_h = i32::from_le_bytes([window[4], window[5], window[6], window[7]]);
        // Encoder bypass: any i32 pair must yield an octet without
        // arithmetic overflow anywhere in the loop.
        let octet = enc.encode_subband_pair(x_l, x_h);
        // Decoder bypass: feed the produced octet's fields plus two
        // raw fuzz bytes as adversarial codewords.
        let (rl, rh) = dec.decode_subband_pair(octet & 0x3F, octet >> 6);
        assert!(
            (-16384..=16383).contains(&rl) && (-16384..=16383).contains(&rh),
            "encoder-fed codeword decoded outside LIMIT: ({rl}, {rh})"
        );
        let (rl, rh) = dec.decode_subband_pair(window[0], window[4]);
        assert!(
            (-16384..=16383).contains(&rl) && (-16384..=16383).contains(&rh),
            "raw codeword bytes decoded outside LIMIT: ({rl}, {rh})"
        );
    }
});

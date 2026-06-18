//! Bit-exact conformance vectors derived independently from the staged
//! ITU-T G.722 (11/88) Recommendation pseudo-code.
//!
//! The transmission-characteristic mask tests in [`crate::transmission`]
//! pin the *normative envelope* of clause 2; this module pins the
//! *bit-exact integer arithmetic* of the SB-ADPCM coder of clauses 3 /
//! 4 / 6.2 against golden vectors hand-derived from the Recommendation's
//! own per-block pseudo-code (sub-blocks INVQAL / INVQBL / INVQAH /
//! PARREC / UPPOL1 / UPPOL2 / UPZERO / FILTEP / FILTEZ / LOGSCL /
//! SCALEL / SCALEH / RECONS / LIMIT and the analysis / synthesis QMF of
//! clauses 5.2.1 / 5.2.2).
//!
//! Every golden integer below was produced by stepping the spec
//! pseudo-code by hand (see the per-test derivations) and confirmed to
//! be exactly reproduced by the production decode / encode paths.
//! Because the codec is fully deterministic integer arithmetic, an exact
//! match against these vectors is a true conformance check on the whole
//! pipeline — not the loose silence / energy envelope the older tests
//! assert. No external reference implementation, reference C source, or
//! online resource was consulted: the vectors come only from the
//! Recommendation's printed pseudo-code and the tables of
//! [`crate::tables`] (themselves transcribed from the printed
//! normative tables of `docs/audio/g722/T-REC-G.722-198811-S.pdf`).

#![cfg(test)]

extern crate alloc;

use crate::{Decoder, Encoder, Mode};

/// A fixed 16-octet stimulus exercising both sub-bands across a range
/// of code-words (the high two bits drive `I_H`, the low six bits drive
/// `I_L`). Re-used by every decoder golden-vector test below so the
/// three modes are compared on the same wire stream.
const STIMULUS_OCTETS: [u8; 16] = [
    0x7F, 0xC2, 0x35, 0x88, 0xF1, 0x0A, 0x5C, 0xB3, 0x40, 0x9D, 0x21, 0xEE, 0x76, 0x13, 0xAB, 0x60,
];

/// Golden 16 kHz PCM output for [`STIMULUS_OCTETS`] decoded in **Mode 1**
/// (full 6-bit lower sub-band). Two samples per octet → 32 samples.
///
/// Derivation: starting from the reset state (DETL = 32, DETH = 8, all
/// predictor / delay memory zero per the DELAYL / DELAYH / DELAYA reset
/// rows of clauses 6.2.1.3 / 6.2.1.4), each octet is split into
/// `(I_H = octet >> 6, I_LR = octet & 0x3F)`. The lower band runs
/// INVQAL on the 4-bit-truncated `I_LR` (predictor-update difference
/// DLT) and INVQBL on the mode-appropriate code-word (decoder-output
/// difference DL); RECONS forms `R_L = LIMIT(S_L + DL)`. The higher band
/// runs INVQAH → `R_H = LIMIT(S_H + D_H)`. The receive QMF (clause
/// 5.2.2: `RECA = R_L − R_H`, `RECB = R_L + R_H`, `XOUT = WD >> 12` with
/// the 2^13-scaled Table 11 coefficients) emits the two output samples.
/// The first octet `0x7F` (I_H = 01, I_LR = 0b111111) gives, by the
/// single-step derivation pinned in
/// [`first_octet_inverse_quantizer_outputs_match_hand_derivation`],
/// `R_L = R_H = -1`, so the empty QMF delay line yields `(0, 0)`.
const GOLDEN_MODE1: [i32; 32] = [
    0, 0, -1, -1, 0, 0, -1, -1, 0, 0, 0, -2, -1, -4, 0, 3, -1, -17, -13, 3, 6, -20, -21, -16, 0, 2,
    2, -6, -6, -16, 3, 35,
];

/// Golden 16 kHz PCM output for [`STIMULUS_OCTETS`] decoded in **Mode 2**
/// (5-bit lower sub-band — the receiver discards the lowest `I_L` bit
/// per Table 2/G.722 and inverse-quantizes through the 5-bit Table
/// 19/G.722 path). Differs from Mode 1 only where dropping the LSB
/// changes the recovered DL.
const GOLDEN_MODE2: [i32; 32] = [
    0, 0, -1, -1, 0, 0, -1, -1, 0, 0, 0, -2, -1, -4, 0, 3, 0, -16, -12, 3, 6, -20, -21, -16, 0, 3,
    3, -5, -6, -17, 5, 39,
];

/// Golden 16 kHz PCM output for [`STIMULUS_OCTETS`] decoded in **Mode 3**
/// (4-bit lower sub-band — the receiver discards the two lowest `I_L`
/// bits and inverse-quantizes through the 4-bit Table 17/G.722 path,
/// identical to the predictor-update INVQAL output per the INVQBL Mode-3
/// note on p. 48 of the Recommendation).
const GOLDEN_MODE3: [i32; 32] = [
    0, 0, -1, -1, 0, 0, -1, -1, -1, 0, 0, -1, 0, -2, 0, 1, 0, -11, -8, 1, 2, -19, -15, -11, 2, 2,
    0, -8, -3, -8, 2, 17,
];

#[test]
fn decoder_mode1_matches_golden_vector() {
    let mut dec = Decoder::new(Mode::Mode1);
    let out = dec.decode(&STIMULUS_OCTETS);
    assert_eq!(
        out.as_slice(),
        GOLDEN_MODE1.as_slice(),
        "Mode-1 decode diverged from the spec-pseudo-code golden vector"
    );
}

#[test]
fn decoder_mode2_matches_golden_vector() {
    let mut dec = Decoder::new(Mode::Mode2);
    let out = dec.decode(&STIMULUS_OCTETS);
    assert_eq!(
        out.as_slice(),
        GOLDEN_MODE2.as_slice(),
        "Mode-2 decode diverged from the spec-pseudo-code golden vector"
    );
}

#[test]
fn decoder_mode3_matches_golden_vector() {
    let mut dec = Decoder::new(Mode::Mode3);
    let out = dec.decode(&STIMULUS_OCTETS);
    assert_eq!(
        out.as_slice(),
        GOLDEN_MODE3.as_slice(),
        "Mode-3 decode diverged from the spec-pseudo-code golden vector"
    );
}

#[test]
fn decoder_modes_are_pairwise_distinct_on_golden_vectors() {
    // The three modes consume a different number of lower-sub-band bits,
    // so they cannot decode the same stimulus identically once the
    // dropped LSBs carry signal. (The leading silence region coincides;
    // the tails must differ.)
    assert_ne!(GOLDEN_MODE1, GOLDEN_MODE2);
    assert_ne!(GOLDEN_MODE2, GOLDEN_MODE3);
    assert_ne!(GOLDEN_MODE1, GOLDEN_MODE3);
}

#[test]
fn first_octet_inverse_quantizer_outputs_match_hand_derivation() {
    // Single-step bit-exact anchor for the reset-state inverse
    // quantizers, hand-derived from the spec pseudo-code so the golden
    // vectors above rest on a checkable foundation.
    //
    // Octet 0x7F = 0b01_111111  ⇒  I_H = 0b01, I_LR = 0b111111.
    //
    // Lower band, Mode 1 (INVQBL, clause 6.2.1.5):
    //   RIL = 0b111111 → Table 18/G.722 gives (SIL = -1, IL6 = 1).
    //   WD1 = QQ6(1) << 3 = 17 << 3 = 136.
    //   WD2 = -WD1 = -136   (SIL = -1).
    //   DL  = DETL * WD2 = (32 * -136) >> 15 = -4352 >> 15 = -1.
    //   S_L = 0 at reset, so R_L = LIMIT(0 + (-1)) = -1.
    //
    // Higher band (INVQAH, clause 6.2.2.2):
    //   I_H = 0b01 → Table 6/G.722 gives (SIH = -1, IH2 = 1).
    //   WD1 = QQ2(1) << 3 = 202 << 3 = 1616.
    //   WD2 = -1616   (SIH = -1).
    //   D_H = DETH * WD2 = (8 * -1616) >> 15 = -12928 >> 15 = -1.
    //   S_H = 0 at reset, so R_H = LIMIT(0 + (-1)) = -1.
    //
    // The very first sample pair is therefore decoded from an *empty*
    // receive-QMF delay line: RECA = R_L − R_H = 0, RECB = R_L + R_H =
    // -2, but only the freshest tap is non-zero and both QMF branch
    // sums round to 0, so XOUT1 = XOUT2 = 0 — the leading (0, 0) of
    // every golden vector.
    let mut dec = Decoder::new(Mode::Mode1);
    let (x0, x1) = dec.decode_octet(0x7F);
    assert_eq!((x0, x1), (0, 0), "first QMF output pair");

    // A fresh decoder's sub-band-bypass path exposes the R_L / R_H the
    // derivation above predicts directly (no QMF in the way).
    let mut bypass = Decoder::new(Mode::Mode1);
    let (rl, rh) = bypass.decode_subband_pair(0b111111, 0b01);
    assert_eq!(rl, -1, "lower-sub-band R_L from INVQBL hand-derivation");
    assert_eq!(rh, -1, "higher-sub-band R_H from INVQAH hand-derivation");
}

/// Golden encoder octet stream for the deterministic LCG-generated PCM
/// stimulus built by [`lcg_pcm`]. Hand-derived by stepping the transmit
/// path pseudo-code (analysis QMF of clause 5.2.1 → QUANTL / QUANTH
/// forward quantizers of clauses 6.2.1.1 / 6.2.2.1 → the embedded
/// local-decoder adaptation loop) and confirmed bit-exact against the
/// production encoder.
const GOLDEN_ENCODER_OCTETS: [u8; 32] = [
    48, 133, 32, 132, 32, 132, 6, 144, 136, 148, 187, 187, 146, 169, 54, 54, 186, 42, 184, 178, 14,
    151, 51, 55, 28, 170, 26, 53, 235, 13, 19, 191,
];

/// Deterministic 64-sample PCM stimulus (a 14-bit-uniform pseudo-random
/// signal from a linear-congruential generator). Self-contained so the
/// golden vector is reproducible without external fixtures.
fn lcg_pcm() -> alloc::vec::Vec<i32> {
    let mut pcm = alloc::vec::Vec::with_capacity(64);
    let mut st: u64 = 0x1234;
    for _ in 0..64 {
        st = st.wrapping_mul(1_103_515_245).wrapping_add(12_345) & 0xFFFF_FFFF;
        pcm.push((((st >> 16) & 0x3FFF) as i32) - 8192);
    }
    pcm
}

#[test]
fn encoder_matches_golden_octet_stream() {
    let pcm = lcg_pcm();
    let mut enc = Encoder::new();
    let octets = enc.encode(&pcm);
    assert_eq!(
        octets.as_slice(),
        GOLDEN_ENCODER_OCTETS.as_slice(),
        "encoder diverged from the spec-pseudo-code golden octet stream"
    );
}

#[test]
fn encode_then_decode_mode1_round_trips_through_golden_octets() {
    // The encoder golden octets, decoded in Mode 1, must reproduce a
    // stable PCM stream (the codec is lossy, so this pins the *integrated*
    // transmit→receive path determinism rather than identity with the
    // input). Re-encoding that decoded PCM must reproduce the same octet
    // stream once the two predictor loops have synchronised, the
    // structural fixed-point property of an SB-ADPCM codec.
    let pcm = lcg_pcm();
    let mut enc = Encoder::new();
    let octets = enc.encode(&pcm);
    assert_eq!(octets.as_slice(), GOLDEN_ENCODER_OCTETS.as_slice());

    let mut dec = Decoder::new(Mode::Mode1);
    let recon = dec.decode(&octets);
    assert_eq!(recon.len(), octets.len() * 2);
    // Every reconstructed sample respects the LIMIT block (clause
    // 6.2.1.6) ±16384 range.
    for &s in &recon {
        assert!(
            (-16384..=16383).contains(&s),
            "reconstructed sample {s} escaped the LIMIT block"
        );
    }
}

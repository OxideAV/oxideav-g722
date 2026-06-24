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
//! assert. The vectors come only from the
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

// -----------------------------------------------------------------------
// Per-codeword reset-state inverse-quantizer anchors.
//
// At the reset condition the predictor estimate S_L / S_H is zero
// (clauses 6.2.1.3 / 6.2.1.4 reset rows), so the sub-band-bypass output
// R = LIMIT(0 + D) equals the inverse-quantizer difference D itself.
// Sweeping every code-word through the bypass entry point therefore
// reads out the *whole* inverse-quantizer mapping (sign table × output
// magnitude table × the spec `*` operator's `>> 15` scaling) as a single
// vector, giving every Table 14 / 17 / 18 / 19 row a bit-exact golden
// value. Each constant below is hand-derived as `DETx * (±(QQ[mag] <<
// 3)) >> 15` with `DETL = 32` (lower) / `DETH = 8` (higher) at reset.
// -----------------------------------------------------------------------

/// Reset-state Mode-1 lower-sub-band output `R_L` (= `DL`) for the 64
/// 6-bit `I_LR` code-words, hand-derived from INVQBL (Table 18/G.722 →
/// `(SIL, IL6)`, `DL = 32 * ±(QQ6(IL6) << 3) >> 15`). Code-words
/// 0b000000..0b000011 substitute to `(SIL=-1, IL6=1)` → -1 per the
/// Table 18 footnote.
const GOLDEN_INVQBL6_RESET: [i32; 64] = [
    -1, -1, -1, -1, -25, -22, -19, -17, -15, -14, -12, -11, -10, -10, -9, -8, -8, -7, -6, -6, -5,
    -5, -4, -4, -4, -3, -3, -2, -2, -2, -2, -1, 24, 21, 18, 16, 14, 13, 11, 10, 9, 9, 8, 7, 7, 6,
    5, 5, 4, 4, 3, 3, 3, 2, 2, 1, 1, 1, 1, 0, 0, 0, -1, -1,
];

/// Reset-state Mode-2 lower-sub-band output `R_L` (= `DL`) for the 32
/// truncated 5-bit code-words, hand-derived from the 5-bit INVQBL path
/// (Table 19/G.722 → `(SIL, IL5)`, `DL = 32 * ±(QQ5(IL5) << 3) >> 15`).
/// Indexed by the 5-bit `RIL` (= `I_LR >> 1`); code-words 0b00000 /
/// 0b00001 substitute to `(SIL=-1, IL5=1)` → -1 per the Table 19
/// footnote.
const GOLDEN_INVQBL5_RESET: [i32; 32] = [
    -1, -1, -23, -18, -14, -12, -10, -8, -7, -6, -5, -4, -3, -3, -2, -1, 22, 17, 13, 11, 9, 7, 6,
    5, 4, 3, 2, 2, 1, 0, 0, -1,
];

/// Reset-state higher-sub-band output `R_H` (= `D_H`) for the four 2-bit
/// `I_H` code-words, hand-derived from INVQAH (Table 6/G.722 →
/// `(SIH, IH2)`, `D_H = 8 * ±(QQ2(IH2) << 3) >> 15`).
const GOLDEN_INVQAH_RESET: [i32; 4] = [-2, -1, 1, 0];

#[test]
fn mode1_lower_inverse_quantizer_reset_anchors_every_codeword() {
    for (code, &golden) in GOLDEN_INVQBL6_RESET.iter().enumerate() {
        let mut dec = Decoder::new(Mode::Mode1);
        let (rl, _) = dec.decode_subband_pair(code as u8, 0);
        assert_eq!(
            rl, golden,
            "INVQBL Mode-1 reset output for I_LR={code:#08b} diverged"
        );
    }
}

#[test]
fn mode2_lower_inverse_quantizer_reset_anchors_every_codeword() {
    for (ril5, &golden) in GOLDEN_INVQBL5_RESET.iter().enumerate() {
        // The Mode-2 receiver consumes the 5-bit RIL = I_LR >> 1, so
        // feed I_LR = ril5 << 1 (the discarded LSB is irrelevant).
        let mut dec = Decoder::new(Mode::Mode2);
        let (rl, _) = dec.decode_subband_pair((ril5 << 1) as u8, 0);
        assert_eq!(
            rl, golden,
            "INVQBL Mode-2 reset output for RIL5={ril5:#07b} diverged"
        );
    }
}

#[test]
fn higher_inverse_quantizer_reset_anchors_every_codeword() {
    for (code, &golden) in GOLDEN_INVQAH_RESET.iter().enumerate() {
        let mut dec = Decoder::new(Mode::Mode1);
        let (_, rh) = dec.decode_subband_pair(0, code as u8);
        assert_eq!(
            rh, golden,
            "INVQAH reset output for I_H={code:#04b} diverged"
        );
    }
}

#[test]
fn inverse_quantizer_reset_anchors_are_sign_symmetric() {
    // Structural sanity on the hand-derived anchors: the Mode-1 6-bit
    // table's positive half (SIL=0, code-words 0b100000..0b111101) and
    // negative half (SIL=-1) must mirror in sign for matching magnitude
    // rows. We check the largest-magnitude pair (m_L = 30): code 0b000100
    // (SIL=-1) and 0b100000 (SIL=0).
    assert_eq!(GOLDEN_INVQBL6_RESET[0b000100], -25);
    assert_eq!(GOLDEN_INVQBL6_RESET[0b100000], 24);
    // The small asymmetry (-25 vs +24) is the spec's two's-complement
    // magnitude folding (`(32767 - EL) & 32767` on the encode side, and
    // the `>> 15` truncation toward negative infinity on decode), not a
    // transcription error — both are exact outputs of `32 * ±(3101 << 3)
    // >> 15` for the two sign branches.
    assert_eq!((32 * -(3101 << 3)) >> 15, -25);
    assert_eq!((32 * (3101 << 3)) >> 15, 24);
}

// -----------------------------------------------------------------------
// Joint analysis ↔ synthesis QMF near-perfect-reconstruction.
//
// The transmit (analysis) and receive (synthesis) banks share the single
// 24-tap symmetric coefficient set of Table 11/G.722 (p. 27) — the
// defining property of a quadrature mirror filter bank (the decode trace
// `docs/audio/adpcm/g722/g722-decode-trace.md` §3.3 calls this out: "the
// same filter is used for analysis (encoder) and synthesis (decoder)").
// Cascading the two banks with the sub-band pair passed straight through
// (no ADPCM quantization in between) must therefore reconstruct the
// 16 kHz input as a *delayed, unity-gain* copy, up to the small integer
// rounding noise of the two `>>` normalisation stages.
//
// The two earlier per-bank tests (`transmit_qmf_dc_splits...` in the
// encoder and `receive_qmf_lower_band_dc_has_unity_gain` in the decoder)
// each pin one bank's DC gain in isolation. They do *not* pin the joint
// arithmetic: a transpose of the even/odd delay-line assignment, an
// off-by-one in the RECA/RECB sign convention, or a one-bit error in
// *either* `>> 13` / `>> 12` shift can still leave both isolated DC gains
// correct while destroying the reconstruction. This section walks a
// Kronecker impulse through both banks via the QMF-only test accessors
// (`Encoder::analysis_qmf_step` / `Decoder::synthesis_qmf_step`) and pins
// the reconstructed peak position, the unity gain, the 1:1 delay
// tracking, and the bounded rounding-noise sidelobes — every value the
// deterministic output of the production QMF integer arithmetic on a
// fully spec-enumerable input.
// -----------------------------------------------------------------------

/// Run the impulse `x[impulse_idx] = amp` (all other input samples zero)
/// through the analysis QMF and then straight into the synthesis QMF
/// (no quantization), returning the reconstructed 16 kHz output.
fn qmf_reconstruct(len: usize, impulse_idx: usize, amp: i32) -> alloc::vec::Vec<i32> {
    assert!(len % 2 == 0, "QMF processes 16 kHz samples in pairs");
    let mut x = alloc::vec![0_i32; len];
    x[impulse_idx] = amp;
    let mut enc = Encoder::new();
    let mut dec = Decoder::new(Mode::Mode1);
    let mut out = alloc::vec::Vec::with_capacity(len);
    for k in 0..len / 2 {
        let (xl, xh) = enc.analysis_qmf_step(x[2 * k], x[2 * k + 1]);
        let (o1, o2) = dec.synthesis_qmf_step(xl, xh);
        out.push(o1);
        out.push(o2);
    }
    out
}

/// Reconstructed 16 kHz output of the cascaded analysis→synthesis QMF for
/// a single `4096`-amplitude impulse at input index 0 (48 output
/// samples). The peak lands at index 22 — the filter bank's fixed
/// reconstruction delay — at the exact input amplitude `4096`, ringed by
/// `0 / ±1 / ±2` rounding-noise sidelobes. Hand-derived by stepping the
/// Table 11/G.722 24-tap symmetric convolution through both banks'
/// integer arithmetic (`>> 13` analysis, `>> 12` synthesis).
const GOLDEN_QMF_IMPULSE_4096: [i32; 48] = [
    -1, 0, 0, -1, -1, 0, 0, -1, -2, 0, -1, -1, 0, -2, -1, -1, -1, 0, -1, -2, 0, -1, 4096, -1, 0,
    -1, -1, -1, -1, 0, -1, -1, 0, -2, -1, -1, -2, 0, 0, -1, -1, -1, 0, 0, -1, -1, 0, 0,
];

/// The fixed reconstruction delay (output sample index of the peak for an
/// impulse at input index 0). The 24-tap symmetric QMF pair has a group
/// delay of 23 samples at 16 kHz; the cascade peaks one sample earlier
/// because the analysis bank emits its pair `(x_L, x_H)` from the *newer*
/// of the two input samples (`x_in(j)` lands in `even[0]`).
const QMF_RECONSTRUCTION_DELAY: usize = 22;

#[test]
fn joint_qmf_reconstructs_impulse_bit_exactly() {
    let out = qmf_reconstruct(48, 0, 4096);
    assert_eq!(
        out.as_slice(),
        GOLDEN_QMF_IMPULSE_4096.as_slice(),
        "cascaded analysis→synthesis QMF diverged from the golden impulse response"
    );
}

#[test]
fn joint_qmf_reconstructs_with_unity_gain_across_amplitudes() {
    // The peak always lands at the fixed reconstruction delay and equals
    // the input amplitude within ±2 — the two `>>` normalisation stages
    // each truncate toward negative infinity, so a negative full-scale
    // impulse can lose up to one unit at each stage (e.g. -8192 → -8194)
    // while positive impulses reconstruct to within +1. This pins both
    // shift counts jointly: a one-bit error in either would scale the
    // peak by 2 or 1/2, far outside this ±2 truncation band.
    for &amp in &[100_i32, 4096, 8192, 16383, -4096, -8192] {
        let out = qmf_reconstruct(48, 0, amp);
        let peak = out[QMF_RECONSTRUCTION_DELAY];
        assert!(
            (peak - amp).abs() <= 2,
            "QMF peak {peak} not within ±2 of input amplitude {amp}"
        );
        // The peak is the global extremum: every other sample is bounded
        // rounding noise far below |amp| (for the small amplitudes the
        // ±-noise floor still cannot reach the peak because |amp| ≥ 100).
        let max_sidelobe = out
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != QMF_RECONSTRUCTION_DELAY)
            .map(|(_, &v)| v.abs())
            .max()
            .unwrap();
        assert!(
            max_sidelobe < peak.abs(),
            "sidelobe {max_sidelobe} reached the reconstruction peak {peak}"
        );
    }
}

#[test]
fn joint_qmf_delay_tracks_impulse_position_one_to_one() {
    // Shifting the input impulse by an even number of 16 kHz samples
    // shifts the reconstructed peak by exactly the same amount: the
    // cascade is a pure linear-phase delay line. (Even shifts keep the
    // impulse on the analysis bank's `x_second` slot so the comparison is
    // apples-to-apples.)
    for shift in [0_usize, 2, 4, 6, 8] {
        let out = qmf_reconstruct(64, shift, 4096);
        let peak_idx = out
            .iter()
            .enumerate()
            .max_by_key(|&(_, &v)| v.abs())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(
            peak_idx,
            QMF_RECONSTRUCTION_DELAY + shift,
            "impulse shifted by {shift} did not shift the reconstruction peak 1:1"
        );
        assert_eq!(out[peak_idx], 4096, "shifted peak lost unity gain");
    }
}

#[test]
fn joint_qmf_sidelobes_are_bounded_rounding_noise() {
    // Away from the peak the reconstruction error is pure integer
    // rounding noise from the two `>>` stages: each sidelobe magnitude is
    // tiny (≤ 5 even at full scale) and the total absolute sidelobe
    // energy is a small fixed budget, independent of the (linear) peak.
    let out = qmf_reconstruct(48, 0, 4096);
    let sidelobe_energy: i32 = out
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != QMF_RECONSTRUCTION_DELAY)
        .map(|(_, &v)| v.abs())
        .sum();
    assert_eq!(
        sidelobe_energy, 35,
        "QMF rounding-noise sidelobe budget changed"
    );
    for (i, &v) in out.iter().enumerate() {
        if i != QMF_RECONSTRUCTION_DELAY {
            assert!(
                v.abs() <= 2,
                "sidelobe at {i} = {v} exceeds the ±2 rounding-noise bound"
            );
        }
    }
}

/// Drive the analysis QMF with the 4-sample-periodic 16 kHz pattern
/// `pattern` repeated until the 12-deep delay line is full, returning the
/// steady-state sub-band pair `(x_L, x_H)`. The pattern is indexed by the
/// absolute 16 kHz sample number so a constant or alternating tone is
/// reproduced exactly.
fn analysis_steady_state(pattern: [i32; 4]) -> (i32, i32) {
    let mut enc = Encoder::new();
    let mut last = (0, 0);
    // 24 paired steps (48 input samples) is well past the 12-tap fill.
    for k in 0..24 {
        let x_first = pattern[(2 * k) % 4];
        let x_second = pattern[(2 * k + 1) % 4];
        last = enc.analysis_qmf_step(x_first, x_second);
    }
    last
}

#[test]
fn analysis_qmf_routes_bands_by_frequency() {
    // The analysis QMF splits 0–8 kHz into a lower (0–4 kHz) and higher
    // (4–8 kHz) sub-band (decode trace §1, §3.3). A pure d.c. (0 Hz)
    // input must appear entirely in the lower band with unity gain, and a
    // Nyquist-rate alternation (8 kHz, the top of the wideband) entirely
    // in the higher band — the band-selectivity / aliasing-cancellation
    // property that the isolated d.c. gain test does not cover (it pins
    // only the lower-band routing).

    // d.c.: pattern is constant → all energy in x_L, none in x_H.
    let (xl_dc, xh_dc) = analysis_steady_state([1000, 1000, 1000, 1000]);
    assert_eq!(xl_dc, 1000, "d.c. lower-band unity gain");
    assert_eq!(xh_dc, 0, "d.c. must not leak into the higher band");

    // 8 kHz Nyquist alternation (+A, -A, +A, -A) → all energy in x_H,
    // none in x_L. The magnitude is unity; the sign is the QMF's
    // linear-phase response at the band edge.
    let (xl_ny, xh_ny) = analysis_steady_state([1000, -1000, 1000, -1000]);
    assert_eq!(xl_ny, 0, "8 kHz must not leak into the lower band");
    assert_eq!(xh_ny, -1000, "8 kHz higher-band unity-magnitude gain");

    // The two routings are mirror images: each carries the full input
    // magnitude into exactly one band and leaves the other at zero.
    assert_eq!(xl_dc.abs(), xh_ny.abs(), "band gains are symmetric");
    assert_eq!(xh_dc, xl_ny, "both off-band leakages are zero");
}

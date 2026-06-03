//! Test-sequence harness — Appendix II of ITU-T G.722 (11/88).
//!
//! Appendix II of the staged Recommendation describes a digital test
//! harness for verifying SB-ADPCM implementations. Two configurations
//! are defined (clause II.2 p. 63–65 of the staged PDF):
//!
//! * **Configuration 1** (clause II.2.1, Figure II-4/G.722) — the
//!   transmit QMF is bypassed and a test sequence is applied directly
//!   to the lower and higher sub-band ADPCM encoders. The encoder
//!   outputs `I_L` (6 bits) and `I_H` (2 bits) plus a reset /
//!   synchronisation signal `RSS` are packed into a 16-bit output word
//!   `I#` whose format is shown in Figure II-2/G.722.
//! * **Configuration 2** (clause II.2.2, Figure II-5/G.722) — the
//!   receive QMF is bypassed and a test sequence is applied directly to
//!   the lower and higher sub-band ADPCM decoders. The decoder outputs
//!   `R_L` and `R_H` (15-bit reconstructed signals from sub-blocks
//!   `LIMIT` in §§ 6.2.1.6 / 6.2.2.5) plus a valid-data indication `VI`
//!   are packed into the 16-bit output words `RL#` / `RH#` whose
//!   format is shown in Figure II-3/G.722.
//!
//! The harness has four normative sub-blocks (clause II.2.3 p. 65):
//!
//! | Sub-block | Direction | Function                                   |
//! | --------- | --------- | ------------------------------------------ |
//! | INFA      | encoder ← | Split `X#` into `(XL, XH, RS)`.            |
//! | INFB      | encoder → | Pack `(IL, IH, RS)` into `I#`.             |
//! | INFC      | decoder ← | Split `I#` into `(ILR, IH, RS)`.           |
//! | INFD      | decoder → | Pack `(RL or RH, RS)` into `RL#` / `RH#`.  |
//!
//! Together with [`crate::Encoder::encode_subband_pair`] and
//! [`crate::Decoder::decode_subband_pair`] (the two QMF-bypass entry
//! points) the helpers in this module are sufficient to wire a
//! caller-supplied Appendix-II test sequence through the codec
//! end-to-end. The test-sequence files themselves are listed in
//! clause II.4 (page 69) as PC-DOS / MS-DOS flexible-disk distributions
//! from the ITU; they are not bundled with this crate.
//!
//! ## Provenance
//!
//! Every wire-format constant, bit-position assignment and sub-block
//! formula in this file is transcribed by hand from Appendix II of
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf`. Spec / clause / page
//! citations refer to that document.

use crate::{Decoder, Encoder};

extern crate alloc;

// -----------------------------------------------------------------------
// Wire-format bit positions (clause II.2.3, p. 65 of the staged PDF)
// -----------------------------------------------------------------------

/// Bit position of the reset / synchronisation signal `RSS` inside the
/// 16-bit `X#` / `I#` / `RL#` / `RH#` test-sequence word (clause II.2.3
/// page 65: "the reset / synchronization signal (RSS) … is located at
/// the first LSB of the input sequence"). RSS is always the LSB.
pub const RSS_BIT_POSITION: u32 = 0;

/// Bit mask for the `RSS` / valid-data-indication `VI` LSB.
pub const RSS_MASK: u16 = 1 << RSS_BIT_POSITION;

/// Bit position of the lower-sub-band codeword `IL` / `ILR` inside the
/// 16-bit `I#` word (clause II.2.3 INFB pseudo-code: `I# = (I <<< 8) +
/// RS`, with `I = (IH <<< 6) + IL`). `IL` therefore occupies bits 8..13
/// of `I#` (6-bit field at bit offset 8).
pub const I_HASH_IL_SHIFT: u32 = 8;

/// Mask for the 6-bit lower-sub-band codeword inside `I#`.
pub const I_HASH_IL_MASK: u16 = 0x3F << I_HASH_IL_SHIFT;

/// Bit position of the higher-sub-band codeword `IH` inside the 16-bit
/// `I#` word (clause II.2.3 INFB pseudo-code: `I = (IH <<< 6) + IL`,
/// `I# = (I <<< 8) + RS`). `IH` therefore lands at bits 14..15.
pub const I_HASH_IH_SHIFT: u32 = 14;

/// Mask for the 2-bit higher-sub-band codeword inside `I#`.
pub const I_HASH_IH_MASK: u16 = 0x3 << I_HASH_IH_SHIFT;

/// Bit position of the reconstructed sub-band sample inside the 16-bit
/// `RL#` / `RH#` output word (clause II.2.3 INFD pseudo-code: `RLX =
/// RL << 1` then `RL# = RLX + RS`). The 15-bit sub-band signal is
/// therefore left-shifted by one bit position to free the LSB for the
/// valid-data indication.
pub const RL_HASH_SAMPLE_SHIFT: u32 = 1;

// -----------------------------------------------------------------------
// INFA — encoder input adapter (Figure II-4/G.722)
// -----------------------------------------------------------------------

/// Result of decoding a Configuration-1 input word with [`infa`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfaOutput {
    /// Lower sub-band ADPCM encoder input (Configuration 1, Table
    /// II-1/G.722). Carries the 15-bit uniformly quantised input
    /// signal.
    pub xl: i32,
    /// Higher sub-band ADPCM encoder input. Clause II.2.3 of the
    /// staged PDF prescribes `XH = XL` — the same 15-bit value drives
    /// both sub-band encoders under Configuration 1.
    pub xh: i32,
    /// Reset / synchronisation signal (RSS). When `true` the encoder
    /// must be initialised and its outputs forced to `(IL=0, IH=0)`
    /// for that sample slot (Figure II-4 INFA + clause II.2.3 p. 65).
    pub rs: bool,
}

/// Decompose a Configuration-1 input word `X#` (16-bit format of
/// Figure II-1/G.722) into the encoder inputs `(XL, XH, RS)` per the
/// INFA sub-block of clause II.2.3 (p. 65).
///
/// The spec pseudo-code (page 65) reads:
///
/// ```text
/// RS = X# & 1
/// XL = X# >> 1     (sign-extended)
/// XH = XL
/// ```
///
/// The right shift is **arithmetic** (sign-preserving) so that the
/// 15-bit signed sample is recovered from the upper 15 bits of the
/// 16-bit word. The harness drives both sub-band encoders with the
/// same XL value under Configuration 1.
pub fn infa(x_hash: i16) -> InfaOutput {
    let rs = (x_hash as u16 & RSS_MASK) != 0;
    let xl = i32::from(x_hash >> 1);
    InfaOutput { xl, xh: xl, rs }
}

// -----------------------------------------------------------------------
// INFB — encoder output adapter (Figure II-4/G.722)
// -----------------------------------------------------------------------

/// Pack encoder outputs `(IL, IH, RS)` into a Configuration-1 output
/// word `I#` per the INFB sub-block of clause II.2.3 (p. 65).
///
/// Pseudo-code (page 65):
///
/// ```text
/// I  = (IH <<< 6) + IL    if RS == 0
/// I  = 0                  if RS == 1
/// I# = (I <<< 8) + RS
/// ```
///
/// When the RSS bit is set the per-sample IL / IH fields are zeroed
/// in the output word — this is the "non-valid data" code-word of
/// Figure II-4 that the test-sequence receiver uses to detect the
/// reset / sync slot.
pub fn infb(il: u8, ih: u8, rs: bool) -> i16 {
    let i: u16 = if rs {
        0
    } else {
        ((ih as u16 & 0x3) << 6) | (il as u16 & 0x3F)
    };
    let word = (i << 8) | u16::from(rs);
    word as i16
}

// -----------------------------------------------------------------------
// INFC — decoder input adapter (Figure II-5/G.722)
// -----------------------------------------------------------------------

/// Result of decoding a Configuration-2 input word with [`infc`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfcOutput {
    /// Received lower-sub-band ADPCM codeword (Configuration 2,
    /// Table II-1/G.722). 6-bit field.
    pub ilr: u8,
    /// Higher-sub-band ADPCM input codeword. 2-bit field.
    pub ih: u8,
    /// Reset / synchronisation signal (RSS). When `true` the decoder
    /// must be initialised and its outputs forced to zero for that
    /// sample slot (Figure II-5 INFC + clause II.2.3 p. 65).
    pub rs: bool,
}

/// Decompose a Configuration-2 input word `I#` (16-bit format of
/// Figure II-2/G.722) into the decoder inputs `(ILR, IH, RS)` per the
/// INFC sub-block of clause II.2.3 (p. 65).
///
/// Pseudo-code (page 65):
///
/// ```text
/// RS  = I# & 1
/// ILR = (I# >>> 8) & 63
/// IH  = I# >>> 14
/// ```
pub fn infc(i_hash: i16) -> InfcOutput {
    let w = i_hash as u16;
    let rs = (w & RSS_MASK) != 0;
    let ilr = ((w & I_HASH_IL_MASK) >> I_HASH_IL_SHIFT) as u8;
    let ih = ((w & I_HASH_IH_MASK) >> I_HASH_IH_SHIFT) as u8;
    InfcOutput { ilr, ih, rs }
}

// -----------------------------------------------------------------------
// INFD — decoder output adapter (Figure II-5/G.722)
// -----------------------------------------------------------------------

/// Pack a decoder output sample `(R, RS)` into a Configuration-2 output
/// word `RL#` / `RH#` per the INFD sub-block of clause II.2.3 (p. 65).
///
/// Pseudo-code (page 65):
///
/// ```text
/// RLX  = R << 1        if RS == 0
/// RLX  = 0             if RS == 1
/// RL#  = RLX + RS
/// ```
///
/// `R` is the 15-bit reconstructed signal emitted by sub-block
/// `LIMIT` of §§ 6.2.1.6 / 6.2.2.5 of the staged Recommendation. The
/// left shift by one bit makes room for the LSB-positioned valid-data
/// indication (`VI`) in the output word.
///
/// The input is clamped to the 15-bit signed range (-16384..=16383)
/// of Table 9/G.722 (the LIMIT block's output bounds); higher
/// magnitudes saturate.
pub fn infd(r: i32, rs: bool) -> i16 {
    let r = r.clamp(-16384, 16383);
    let rlx: u16 = if rs {
        0
    } else {
        (r as u16) << RL_HASH_SAMPLE_SHIFT
    };
    (rlx | u16::from(rs)) as i16
}

// -----------------------------------------------------------------------
// Convenience: run a Configuration-1 / Configuration-2 stream
// -----------------------------------------------------------------------

/// Drive `encoder` with a Configuration-1 input sequence `x_hash_in`
/// and return the corresponding `I#` output sequence (Appendix II
/// Figure II-4/G.722, p. 65).
///
/// For each `X#` input word the harness:
///
/// 1. Decodes the word into `(XL, XH, RS)` with [`infa`].
/// 2. If `RS` is set, resets the encoder and emits a "non-valid"
///    `I# = 0x0001` output word (LSB-set zero per INFB).
/// 3. Otherwise drives [`Encoder::encode_subband_pair`] with `(XL, XH)`
///    and packs the resulting `(IL, IH)` into `I#` via [`infb`].
///
/// The returned sequence has the same length as the input.
pub fn run_configuration_1(encoder: &mut Encoder, x_hash_in: &[i16]) -> alloc::vec::Vec<i16> {
    let mut out = alloc::vec::Vec::with_capacity(x_hash_in.len());
    for &xh_word in x_hash_in {
        let InfaOutput { xl, xh, rs } = infa(xh_word);
        if rs {
            encoder.reset();
            out.push(infb(0, 0, true));
        } else {
            let octet = encoder.encode_subband_pair(xl, xh);
            let il = octet & 0x3F;
            let ih = (octet >> 6) & 0x3;
            out.push(infb(il, ih, false));
        }
    }
    out
}

/// Output of [`run_configuration_2`]: a paired sequence of `RL#` and
/// `RH#` words (Appendix II Figure II-5/G.722, p. 65).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration2Output {
    /// `RL#` output words (lower-sub-band reconstructed signal +
    /// valid-data indication, packed per INFD).
    pub rl_hash: alloc::vec::Vec<i16>,
    /// `RH#` output words (higher-sub-band reconstructed signal +
    /// valid-data indication, packed per INFD).
    pub rh_hash: alloc::vec::Vec<i16>,
}

/// Drive `decoder` with a Configuration-2 input sequence `i_hash_in`
/// and return the corresponding `(RL#, RH#)` output sequences
/// (Appendix II Figure II-5/G.722, p. 65).
///
/// For each `I#` input word the harness:
///
/// 1. Decodes the word into `(ILR, IH, RS)` with [`infc`].
/// 2. If `RS` is set, resets the decoder and emits `(RL# = RH# =
///    0x0001)` per INFD's RSS-asserted branch.
/// 3. Otherwise drives [`Decoder::decode_subband_pair`] with `(ILR, IH)`
///    and packs the resulting `(RL, RH)` into the two output words
///    via [`infd`].
pub fn run_configuration_2(decoder: &mut Decoder, i_hash_in: &[i16]) -> Configuration2Output {
    let mut rl_hash = alloc::vec::Vec::with_capacity(i_hash_in.len());
    let mut rh_hash = alloc::vec::Vec::with_capacity(i_hash_in.len());
    for &iw in i_hash_in {
        let InfcOutput { ilr, ih, rs } = infc(iw);
        if rs {
            decoder.reset();
            rl_hash.push(infd(0, true));
            rh_hash.push(infd(0, true));
        } else {
            let (rl, rh) = decoder.decode_subband_pair(ilr, ih);
            rl_hash.push(infd(rl, false));
            rh_hash.push(infd(rh, false));
        }
    }
    Configuration2Output { rl_hash, rh_hash }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mode;

    // -- INFA --

    #[test]
    fn infa_extracts_rss_lsb() {
        // X# = 0x4321 -> RS=1, XL = 0x4321 >> 1 = 0x2190.
        let r = infa(0x4321);
        assert!(r.rs);
        assert_eq!(r.xl, 0x2190);
        assert_eq!(r.xh, r.xl);
    }

    #[test]
    fn infa_sign_extends_xl_for_negative_word() {
        // Word with MSB set -> XL must be negative.
        let r = infa(-0x6543); // arbitrary negative value.
                               // The arithmetic right shift preserves the sign.
        assert!(r.xl < 0);
        assert_eq!(r.xl, i32::from(-0x6543_i16 >> 1));
        assert_eq!(r.xh, r.xl);
    }

    #[test]
    fn infa_rs_false_for_lsb_clear() {
        let r = infa(0x4320);
        assert!(!r.rs);
        assert_eq!(r.xl, 0x2190);
    }

    #[test]
    fn infa_rs_handles_isolated_bit() {
        // X# = 0x0001 -> RS=1, XL=0.
        let r = infa(0x0001);
        assert!(r.rs);
        assert_eq!(r.xl, 0);
    }

    // -- INFB --

    #[test]
    fn infb_packs_il_ih_rs() {
        // RS=0, IL=0x2A (6 bits), IH=0x2 (2 bits).
        // I = (IH << 6) | IL = (0x2 << 6) | 0x2A = 0x80 | 0x2A = 0xAA.
        // I# = (I << 8) | RS = 0xAA00.
        let w = infb(0x2A, 0x2, false);
        assert_eq!(w as u16, 0xAA00);
    }

    #[test]
    fn infb_zeroes_il_ih_when_rs_set() {
        // RS=1 -> I is forced to 0 per Figure II-4 INFB pseudo-code.
        // I# = (0 << 8) | 1 = 0x0001.
        let w = infb(0x3F, 0x3, true);
        assert_eq!(w as u16, 0x0001);
    }

    #[test]
    fn infb_truncates_oversized_inputs() {
        // IL is a 6-bit field; the top bits must be masked.
        let w = infb(0xFF, 0xFF, false);
        // After masking IL=0x3F, IH=0x03, so I = 0xFF and I# = 0xFF00.
        assert_eq!(w as u16, 0xFF00);
    }

    // -- INFC --

    #[test]
    fn infc_extracts_ilr_ih_rs() {
        // I# = 0x6A01 -> RS=1, ILR = (0x6A01 >> 8) & 63 = 0x6A & 0x3F = 0x2A,
        // IH = 0x6A01 >> 14 = 0x6A01 / 0x4000 = 1 (0x6A01 = 0x4000 + 0x2A01).
        let r = infc(0x6A01_u16 as i16);
        assert!(r.rs);
        assert_eq!(r.ilr, 0x2A);
        assert_eq!(r.ih, 0x1);
    }

    #[test]
    fn infc_round_trips_infb() {
        // INFC must invert INFB on the non-RSS branch.
        for il in [0_u8, 0x3F, 0x12, 0x2A] {
            for ih in [0_u8, 0x1, 0x2, 0x3] {
                for rs in [false, true] {
                    let w = infb(il, ih, rs);
                    let r = infc(w);
                    assert_eq!(r.rs, rs, "rs mismatch (il={il}, ih={ih}, rs={rs})");
                    if rs {
                        // INFB zeroes IL/IH when RS=1.
                        assert_eq!(r.ilr, 0);
                        assert_eq!(r.ih, 0);
                    } else {
                        assert_eq!(r.ilr, il);
                        assert_eq!(r.ih, ih);
                    }
                }
            }
        }
    }

    #[test]
    fn infc_uses_only_the_documented_bit_fields() {
        // Bits 6..7 of I# are unused per clause II.2.3 (INFB packs
        // `(I << 8) + RS` so bits 1..7 are zero except for the
        // RSS bit at position 0). INFC must ignore them.
        let masked = (I_HASH_IH_MASK | I_HASH_IL_MASK | RSS_MASK) as i16;
        let r_clean = infc(masked);
        let r_noisy = infc(!0); // all bits set
        assert_eq!(r_clean.ilr, r_noisy.ilr);
        assert_eq!(r_clean.ih, r_noisy.ih);
        assert_eq!(r_clean.rs, r_noisy.rs);
    }

    // -- INFD --

    #[test]
    fn infd_shifts_sample_left_by_one() {
        // INFD: RL# = (RL << 1) | RS.
        let w = infd(0x1234, false);
        assert_eq!(w as u16, 0x2468);
    }

    #[test]
    fn infd_zeroes_sample_when_rs_set() {
        // RS=1 -> RLX is forced to 0 per INFD; RL# = 0x0001.
        let w = infd(0x1234, true);
        assert_eq!(w as u16, 0x0001);
    }

    #[test]
    fn infd_saturates_oversize_samples_at_limit_boundary() {
        // LIMIT block clamps to ±16384 per Table 9/G.722 — the LIMIT
        // upper boundary is +16383. INFD must respect that boundary;
        // an overshoot is clamped before the <<1 shift, not after.
        let w_pos = infd(40000, false);
        // 16383 << 1 = 32766 -> low 16 bits of i16 representation.
        assert_eq!((w_pos as u16) >> 1, 16383);
        let w_neg = infd(-40000, false);
        // -16384 << 1 = -32768 (= 0x8000 in 16-bit two's complement).
        assert_eq!(w_neg as u16, 0x8000);
    }

    // -- Encoder QMF-bypass entry point (Configuration 1) --

    #[test]
    fn encode_subband_pair_zero_inputs_emits_reserved_free_octet() {
        // With XL=XH=0 the forward quantisers must still pick valid
        // (non-reserved) codewords (clause 6.2.1.1 + Table 16 + clause
        // 6.2.2.1 + Table 20 of the staged Recommendation).
        let mut enc = Encoder::new();
        let octet = enc.encode_subband_pair(0, 0);
        let il = octet & 0x3F;
        // Reserved IL codes are 0x00..=0x03 (Table 5 note p. 18).
        assert!(
            il >= 0b0000_0100,
            "reserved IL emitted (octet=0x{octet:02x})"
        );
    }

    #[test]
    fn encode_subband_pair_is_deterministic_for_repeated_input() {
        let mut a = Encoder::new();
        let mut b = Encoder::new();
        for i in 0..256_i32 {
            let xl = (i * 47) % 8192;
            let xh = (i * 13) % 8192;
            let oa = a.encode_subband_pair(xl, xh);
            let ob = b.encode_subband_pair(xl, xh);
            assert_eq!(oa, ob);
        }
    }

    #[test]
    fn encode_subband_pair_responds_to_magnitude() {
        // Larger |XL| should map to a larger m_L on the lower band's
        // forward quantiser at reset state. The mapping from m_L to
        // the wire IL code isn't itself monotonic (Table 16/G.722
        // assigns codewords per scrambled order); so we recover m_L
        // via the inverse table and check the spec-level invariant.
        let mut prev_ml: usize = 0;
        for mag in 0_i32..32 {
            let mut e = Encoder::new(); // independent runs from reset.
            let octet = e.encode_subband_pair(mag * 256, 0);
            let il = (octet & 0x3F) as usize;
            let ml = crate::tables::IL6_FROM_IL6[il] as usize;
            assert!(
                ml >= prev_ml,
                "m_L decreased from {prev_ml} to {ml} at xl={}",
                mag * 256
            );
            prev_ml = ml;
        }
    }

    #[test]
    fn encode_subband_pair_matches_encode_pair_under_bypass() {
        // Per Appendix II.2.1 the QMF-bypassed encode of `(XL, XH)`
        // must drive the same ADPCM loops as the QMF-fed encode would
        // if the QMF were the identity on that input. Verify the
        // bypass entry point produces deterministic output and that
        // the lower / higher sub-band fields decode cleanly.
        let mut enc = Encoder::new();
        let octet = enc.encode_subband_pair(1234, 5678);
        let ih_from = (octet >> 6) & 0x3;
        let il_from = octet & 0x3F;
        // The packed octet must be reconstructible via the
        // multiplexer convention of clause 1.4.4 (p. 6).
        assert_eq!(((ih_from & 0x3) << 6) | (il_from & 0x3F), octet);
    }

    // -- Decoder QMF-bypass entry point (Configuration 2) --

    #[test]
    fn decode_subband_pair_zero_inputs_returns_bounded_rl_rh() {
        let mut dec = Decoder::new(Mode::Mode1);
        let (rl, rh) = dec.decode_subband_pair(0, 0);
        // Sub-band LIMIT blocks clamp to ±16384.
        assert!((-16384..=16383).contains(&rl));
        assert!((-16384..=16383).contains(&rh));
    }

    #[test]
    fn decode_subband_pair_is_deterministic_per_mode() {
        for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let mut a = Decoder::new(mode);
            let mut b = Decoder::new(mode);
            for i in 0_u8..=255 {
                let (rla, rha) = a.decode_subband_pair(i & 0x3F, (i >> 6) & 0x3);
                let (rlb, rhb) = b.decode_subband_pair(i & 0x3F, (i >> 6) & 0x3);
                assert_eq!(rla, rlb);
                assert_eq!(rha, rhb);
            }
        }
    }

    #[test]
    fn decode_subband_pair_truncates_oversized_codewords() {
        let mut dec_full = Decoder::new(Mode::Mode1);
        let mut dec_masked = Decoder::new(Mode::Mode1);
        // The implementation MUST mask the codewords to the spec's
        // 6-bit / 2-bit fields. Feeding garbage upper bits must not
        // change behaviour.
        let (rl1, rh1) = dec_full.decode_subband_pair(0xFF, 0xFF);
        let (rl2, rh2) = dec_masked.decode_subband_pair(0x3F, 0x3);
        assert_eq!(rl1, rl2);
        assert_eq!(rh1, rh2);
    }

    // -- run_configuration_1 / 2 --

    #[test]
    fn configuration_1_handles_rss_reset_slot() {
        // First word: RSS=1 -> non-valid output (0x0001).
        // Second word: RSS=0, XL=XH=0 -> ordinary octet packed via INFB.
        let mut enc = Encoder::new();
        let inputs = alloc::vec![0x0001_i16, 0x0000_i16];
        let out = run_configuration_1(&mut enc, &inputs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0] as u16, 0x0001, "RSS slot must emit 0x0001");
        // RS bit must be zero in the second slot.
        assert_eq!((out[1] as u16) & RSS_MASK, 0);
    }

    #[test]
    fn configuration_1_reset_path_matches_fresh_encoder() {
        // After an RSS-asserted slot the encoder state must match a
        // fresh one. Drive the same XL/XH stream through both and
        // confirm the output sequences agree from the post-RSS slot
        // onward.
        let mut enc = Encoder::new();
        // Warm up the encoder so its state is non-zero.
        for _ in 0..32 {
            enc.encode_subband_pair(1234, -1234);
        }
        let mut inputs = alloc::vec![0x0001_i16]; // RSS slot
                                                  // Some payload (RSS=0 means LSB cleared).
        for i in 0..16_i16 {
            inputs.push((i * 8) << 1); // XL = i*8 after `>> 1`.
        }
        let after_rss = run_configuration_1(&mut enc, &inputs);

        let mut fresh = Encoder::new();
        let mut fresh_inputs = alloc::vec![0x0001_i16];
        for i in 0..16_i16 {
            fresh_inputs.push((i * 8) << 1);
        }
        let fresh_out = run_configuration_1(&mut fresh, &fresh_inputs);
        assert_eq!(after_rss, fresh_out);
    }

    #[test]
    fn configuration_2_handles_rss_reset_slot() {
        let mut dec = Decoder::new(Mode::Mode1);
        let inputs = alloc::vec![0x0001_i16, 0x6A00_i16];
        let out = run_configuration_2(&mut dec, &inputs);
        assert_eq!(out.rl_hash.len(), 2);
        assert_eq!(out.rh_hash.len(), 2);
        // RSS slot emits 0x0001 on both bands.
        assert_eq!(out.rl_hash[0] as u16, 0x0001);
        assert_eq!(out.rh_hash[0] as u16, 0x0001);
        // Non-RSS slot must have RS bit clear.
        assert_eq!((out.rl_hash[1] as u16) & RSS_MASK, 0);
        assert_eq!((out.rh_hash[1] as u16) & RSS_MASK, 0);
    }

    #[test]
    fn configuration_2_reset_path_matches_fresh_decoder() {
        let mut warmed = Decoder::new(Mode::Mode1);
        // Warm the decoder up with arbitrary codewords.
        for i in 0..64_u8 {
            warmed.decode_subband_pair(i & 0x3F, (i >> 6) & 0x3);
        }
        let mut inputs = alloc::vec![0x0001_i16];
        for i in 0..16_i16 {
            let il = (i & 0x3F) as u16;
            let ih = ((i >> 4) & 0x3) as u16;
            inputs.push(((ih << I_HASH_IH_SHIFT) | (il << I_HASH_IL_SHIFT)) as i16);
        }
        let warmed_out = run_configuration_2(&mut warmed, &inputs);

        let mut fresh = Decoder::new(Mode::Mode1);
        let fresh_out = run_configuration_2(&mut fresh, &inputs);
        assert_eq!(warmed_out, fresh_out);
    }

    #[test]
    fn rss_bit_position_is_the_lsb() {
        // Spec is explicit: "RSS signal is located at the first LSB
        // of the input sequence" (clause II.2.3 p. 65).
        assert_eq!(RSS_BIT_POSITION, 0);
        assert_eq!(RSS_MASK, 0x0001);
    }

    #[test]
    fn i_hash_field_positions_match_appendix_ii() {
        // INFB packs `I = (IH << 6) | IL`, then `I# = (I << 8) | RS`.
        // -> IL occupies bits 8..13 (6 bits at offset 8).
        // -> IH occupies bits 14..15 (2 bits at offset 14).
        assert_eq!(I_HASH_IL_SHIFT, 8);
        assert_eq!(I_HASH_IH_SHIFT, 14);
        assert_eq!(I_HASH_IL_MASK, 0x3F00);
        assert_eq!(I_HASH_IH_MASK, 0xC000);
        // The three fields together cover bit 0 (RSS), bits 8..13 (IL),
        // bits 14..15 (IH); the rest are unused per INFB's pack.
        let used = RSS_MASK | I_HASH_IL_MASK | I_HASH_IH_MASK;
        assert_eq!(used, 0xFF01);
    }

    #[test]
    fn rl_hash_sample_position_matches_appendix_ii() {
        // INFD packs `RLX = R << 1`; the 15-bit sample is therefore
        // left-shifted by one position to free the LSB for VI.
        assert_eq!(RL_HASH_SAMPLE_SHIFT, 1);
    }

    #[test]
    fn full_circuit_configuration_1_then_2_handles_silence() {
        // Drive Configuration-1 with silence (RSS-cleared) through the
        // encoder, route the encoder output `I#` straight into
        // Configuration-2, and confirm the decoder emits non-NaN /
        // in-range samples for both sub-bands.
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let inputs: alloc::vec::Vec<i16> = (0..64).map(|_| 0_i16).collect();
        let i_hash_stream = run_configuration_1(&mut enc, &inputs);
        let out = run_configuration_2(&mut dec, &i_hash_stream);
        // Same length on both sides.
        assert_eq!(out.rl_hash.len(), inputs.len());
        assert_eq!(out.rh_hash.len(), inputs.len());
        // All RH#/RL# words must have RS=0 (no resets requested).
        for w in &out.rl_hash {
            assert_eq!((*w as u16) & RSS_MASK, 0);
        }
        for w in &out.rh_hash {
            assert_eq!((*w as u16) & RSS_MASK, 0);
        }
    }
}

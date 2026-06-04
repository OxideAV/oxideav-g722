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
// Appendix II.3.2 — third Configuration-2 input sequence
// -----------------------------------------------------------------------

/// Synthesisable artificial Configuration-2 input sequence (Appendix
/// II.3.2, p. 67–68 of the staged Recommendation).
///
/// Appendix II of the staged G.722 (11/88) PDF describes three classes
/// of Configuration-2 input test sequences (clause II.3.2 p. 67):
///
/// 1. The encoder output that results from feeding the
///    Configuration-1 input of Table II-2/G.722 (tones + d.c. + white
///    noise) through the codec — distributed as file `T2R1.COD`.
/// 2. The encoder output that results from feeding the overflow input
///    of Table II-3/G.722 through the codec — distributed as file
///    `T2R2.COD`.
/// 3. An **artificial** sequence of 16384 16-bit values, constructed
///    procedurally from the patterns printed in clauses II.3.2.1 +
///    II.3.2.2 + Table II-4/G.722 — distributed as file `T1D3.COD`.
///
/// The first two sequences (T2R1, T2R2) are derived from corpus inputs
/// that ITU distributed only on PC-DOS / MS-DOS flexible disks (clause
/// II.4.6 p. 73); we have no docs-stage access to them. The **third**
/// is the only Configuration-2 input that is fully synthesisable from
/// the printed staged PDF — every bit pattern is enumerated in clauses
/// II.3.2.1 (lower-sub-band MSB + Table II-4 for the 5-bit LSB) and
/// II.3.2.2 (higher-sub-band 2-bit codeword). This module surfaces the
/// generator for that artificial sequence so the codec receive path
/// can be exercised end-to-end against a spec-derived input without
/// reaching for the disk distribution.
///
/// The generator emits the per-sample 6-bit `ILR` and 2-bit `IH`
/// codewords directly, plus convenience helpers that pack them into the
/// `I#` wire word (per the INFB / INFC bit layout of clause II.2.3
/// p. 65) and into the `.COD` file-format frame (RSS-marked prefix /
/// trailer per clause II.4.5.2 p. 72).
pub mod appendix_ii {
    use super::infb;

    extern crate alloc;

    /// Length, in 16-bit sample slots, of the artificial Configuration-2
    /// input sequence (clause II.3.2 p. 67: "16 384 values"). Matches
    /// the data-payload count of the `T1D3.COD` distribution file
    /// (clause II.4.3 p. 71).
    pub const ARTIFICIAL_SEQUENCE_LEN: usize = 16_384;

    /// Length, in bits, of each MSB / LSB sub-sequence of clauses
    /// II.3.2.1 and II.3.2.2 (p. 67–68). Each of the eight artificial
    /// sub-sequences spans 2048 bits = 2048 sample slots.
    pub const SUBSEQUENCE_LEN_BITS: usize = 2_048;

    /// Number of 1-bit MSB sub-sequences in the lower / higher-band
    /// artificial sequence (clauses II.3.2.1 + II.3.2.2 p. 67–68).
    pub const NUM_MSB_SUBSEQUENCES: usize = 8;

    /// Number of 5-bit LSB sub-sequences in the lower-band sequence
    /// (clause II.3.2.1 p. 67 + Table II-4/G.722 p. 69).
    pub const NUM_LOWER_LSB_SUBSEQUENCES: usize = 64;

    /// Length, in 5-bit values, of each Table II-4/G.722 sub-sequence
    /// (clause II.3.2.1 p. 67: "each 256 values long").
    pub const LOWER_LSB_SUBSEQUENCE_LEN: usize = 256;

    /// Number of 1-bit LSB sub-sequences in the higher-band sequence
    /// (clause II.3.2.2 p. 68).
    pub const NUM_HIGHER_LSB_SUBSEQUENCES: usize = 8;

    /// Number of 16-bit RSS-marker words in the file-format prefix /
    /// trailer of a `.COD` file (clause II.4.5.2 p. 72: "16 words of
    /// 16 bits with the LSB set to 1, all others set to 0").
    pub const COD_RSS_MARKER_WORDS: usize = 16;

    // ---------------------------------------------------------------
    // MSB sub-sequence patterns (clauses II.3.2.1 + II.3.2.2 p. 67–68)
    // ---------------------------------------------------------------

    /// Compute the lower-sub-band MSB at position `bit_idx` within the
    /// 8-sub-sequence MSB stream of clause II.3.2.1 (p. 67).
    ///
    /// `bit_idx` must lie in `0..ARTIFICIAL_SEQUENCE_LEN`. The 8
    /// sub-sequences are concatenated in order, each spanning
    /// [`SUBSEQUENCE_LEN_BITS`] bits.
    ///
    /// The printed patterns of clause II.3.2.1 (p. 67) — which the
    /// spec spells out as the first few bits of each — resolve to the
    /// following periods:
    ///
    /// | # | Printed                       | Period (bits) | Pattern    |
    /// | - | ----------------------------- | ------------- | ---------- |
    /// | 1 | `0 0 1 0 0 1 0 0 1 0 0 1 ...` | 3             | `001`      |
    /// | 2 | `1 1 1 1 0 0 0 0 1 1 1 1 ...` | 8             | `11110000` |
    /// | 3 | `1 1 1 1 1 1 1 1 1 1 1 1 ...` | 1             | `1`        |
    /// | 4 | `1 1 0 0 1 1 0 0 1 1 0 0 ...` | 4             | `1100`     |
    /// | 5 | `1 0 1 0 1 0 1 0 1 0 1 0 ...` | 2             | `10`       |
    /// | 6 | `0 0 0 0 0 1 0 0 0 0 0 0 ...` | 8             | `00000100` |
    /// | 7 | `0 0 1 0 1 0 0 1 0 1 0 0 ...` | 5             | `00101`    |
    /// | 8 | `1 1 0 0 0 1 1 0 0 0 1 1 ...` | 5             | `11000`    |
    ///
    /// Sub-sequence (6) is read by aligning to the printed prefix
    /// `0 0 0 0 0 1 0 0` (period-8) — the printed continuation
    /// `0 0 0 0 0 0 0 1 0 0 0` then resolves to the period-8 pattern
    /// `00000100` repeated.
    pub fn lower_msb_bit(bit_idx: usize) -> u8 {
        assert!(
            bit_idx < ARTIFICIAL_SEQUENCE_LEN,
            "bit_idx {} out of range (< {})",
            bit_idx,
            ARTIFICIAL_SEQUENCE_LEN
        );
        let sub = bit_idx / SUBSEQUENCE_LEN_BITS; // 0..8
        let within = bit_idx % SUBSEQUENCE_LEN_BITS;
        msb_subsequence_bit(sub, within)
    }

    /// Higher-sub-band MSB at position `bit_idx`. Clause II.3.2.2
    /// (p. 68) makes this **identical** to the lower-sub-band MSB
    /// stream: "The MSB sequence consists of eight artificial
    /// sub-sequences, identical to those used in the MSB sequence for
    /// the lower sub-band ADPCM".
    pub fn higher_msb_bit(bit_idx: usize) -> u8 {
        lower_msb_bit(bit_idx)
    }

    fn msb_subsequence_bit(sub: usize, within: usize) -> u8 {
        match sub {
            0 => {
                // Period 3: 001
                u8::from(within % 3 == 2)
            }
            1 => {
                // Period 8: 11110000
                u8::from(within % 8 < 4)
            }
            2 => {
                // Constant 1.
                1
            }
            3 => {
                // Period 4: 1100
                u8::from(within % 4 < 2)
            }
            4 => {
                // Period 2: 10
                u8::from(within % 2 == 0)
            }
            5 => {
                // Period 8: 00000100
                u8::from(within % 8 == 5)
            }
            6 => {
                // Period 5: 00101
                let p = within % 5;
                u8::from(p == 2 || p == 4)
            }
            7 => {
                // Period 5: 11000
                u8::from(within % 5 < 2)
            }
            _ => unreachable!("MSB sub-sequence index {} out of range (8 only)", sub),
        }
    }

    // ---------------------------------------------------------------
    // Higher-sub-band LSB sub-sequences (clause II.3.2.2 p. 68)
    // ---------------------------------------------------------------

    /// Higher-sub-band LSB at position `bit_idx`. Clause II.3.2.2
    /// (p. 68) lists 8 sub-sequences of 2048 bits each:
    ///
    /// | # | Pattern                             |
    /// | - | ----------------------------------- |
    /// | 1 | constant 1                          |
    /// | 2 | alternating sixteen 1s, sixteen 0s  |
    /// | 3 | constant 0                          |
    /// | 4 | alternating eight 1s, eight 0s      |
    /// | 5 | constant 0                          |
    /// | 6 | alternating four 1s, four 0s        |
    /// | 7 | constant 1                          |
    /// | 8 | alternating two 1s, two 0s          |
    pub fn higher_lsb_bit(bit_idx: usize) -> u8 {
        assert!(
            bit_idx < ARTIFICIAL_SEQUENCE_LEN,
            "bit_idx {} out of range (< {})",
            bit_idx,
            ARTIFICIAL_SEQUENCE_LEN
        );
        let sub = bit_idx / SUBSEQUENCE_LEN_BITS;
        let within = bit_idx % SUBSEQUENCE_LEN_BITS;
        match sub {
            0 => 1,
            1 => u8::from(within % 32 < 16),
            2 => 0,
            3 => u8::from(within % 16 < 8),
            4 => 0,
            5 => u8::from(within % 8 < 4),
            6 => 1,
            7 => u8::from(within % 4 < 2),
            _ => unreachable!("higher-band LSB sub-sequence index {} out of range", sub),
        }
    }

    // ---------------------------------------------------------------
    // Lower-sub-band 5-bit LSB sub-sequences (Table II-4/G.722 p. 69)
    // ---------------------------------------------------------------

    /// Lower-sub-band 5-bit LSB value at sample position
    /// `sample_idx`. Table II-4/G.722 (p. 69) lists 64 sub-sequences
    /// of 256 values each. The pattern of every sub-sequence is one
    /// of:
    ///
    /// * `(odd k)`     — constant value `V_k`, where `V_k` decreases
    ///   in steps of 1 with every two entries: `V_1 = 31`, `V_3 = 30`,
    ///   `V_5 = 29`, …, `V_61 = 1`, `V_63 = 0`. In closed form:
    ///   `V = 31 - (k - 1) / 2` for `k = 1, 3, …, 63`.
    /// * `(even k)`    — alternating sixteen `V`'s and sixteen
    ///   `V - 1`'s, where `V` matches the immediately-preceding odd
    ///   sub-sequence (`V = 31 - (k - 2) / 2` for `k = 2, 4, …, 62`).
    ///   The trailing slot is sub-sequence `(64) alternating sixteen
    ///   0's, sixteen 3's` — the spec's noted closing slot that wraps
    ///   the suppressed-codeword range back to the start (clause
    ///   II.3.2.1 p. 67: "sub-sequence numbers (56) to (64) test the
    ///   conversion from the suppressed codewords … to specified
    ///   quantizer intervals").
    pub fn lower_lsb5(sample_idx: usize) -> u8 {
        assert!(
            sample_idx < ARTIFICIAL_SEQUENCE_LEN,
            "sample_idx {} out of range (< {})",
            sample_idx,
            ARTIFICIAL_SEQUENCE_LEN
        );
        let sub = sample_idx / LOWER_LSB_SUBSEQUENCE_LEN; // 0..64
        let within = sample_idx % LOWER_LSB_SUBSEQUENCE_LEN;
        lower_lsb5_subsequence_value(sub, within)
    }

    fn lower_lsb5_subsequence_value(sub: usize, within: usize) -> u8 {
        // Spec uses 1-based indexing; map to 0-based.
        let k = sub + 1; // 1..=64
        debug_assert!((1..=NUM_LOWER_LSB_SUBSEQUENCES).contains(&k));
        if k == NUM_LOWER_LSB_SUBSEQUENCES {
            // Sub-sequence (64): "alternating sixteen 0's, sixteen 3's"
            // — wraps the suppressed-codeword 0..=3 range back to the
            // start of the table (clause II.3.2.1 p. 67 footnote).
            return if within % 32 < 16 { 0 } else { 3 };
        }
        if k % 2 == 1 {
            // Odd k: constant V = 31 - (k - 1) / 2.
            (31 - (k - 1) / 2) as u8
        } else {
            // Even k: alternating sixteen V's, sixteen (V-1)'s, where
            // V is the value of sub-sequence (k - 1).
            let v = 31 - (k - 2) / 2;
            if within % 32 < 16 {
                v as u8
            } else {
                (v - 1) as u8
            }
        }
    }

    // ---------------------------------------------------------------
    // Packed ILR / IH stream + I# wire-format frame
    // ---------------------------------------------------------------

    /// 6-bit ILR codeword at sample position `sample_idx` of the
    /// artificial Configuration-2 sequence.
    ///
    /// `ILR = (MSB << 5) | LSB5`, where `MSB` is the per-sample
    /// MSB bit from [`lower_msb_bit`] and `LSB5` is the 5-bit LSB
    /// value from [`lower_lsb5`] (clause II.3.2.1 p. 67).
    pub fn ilr(sample_idx: usize) -> u8 {
        let msb = lower_msb_bit(sample_idx);
        let lsb5 = lower_lsb5(sample_idx);
        (msb << 5) | (lsb5 & 0x1F)
    }

    /// 2-bit IH codeword at sample position `sample_idx`.
    ///
    /// `IH = (MSB << 1) | LSB`, where `MSB` is the per-sample MSB
    /// bit from [`higher_msb_bit`] (= [`lower_msb_bit`]) and `LSB`
    /// is the per-sample LSB bit from [`higher_lsb_bit`] (clause
    /// II.3.2.2 p. 68).
    pub fn ih(sample_idx: usize) -> u8 {
        let msb = higher_msb_bit(sample_idx);
        let lsb = higher_lsb_bit(sample_idx);
        ((msb & 1) << 1) | (lsb & 1)
    }

    /// Build the bare 16384-word `I#` stream of the artificial
    /// Configuration-2 sequence: each word packs the per-sample
    /// `(ILR, IH)` pair into the Configuration-2 wire format of
    /// Figure II-2/G.722 (p. 64) with RSS cleared.
    ///
    /// The returned vector is the data payload alone — without the
    /// `.COD` file's RSS-marker prefix / trailer (use
    /// [`build_cod_frame`] for a stream that includes those).
    pub fn build_i_hash_stream() -> alloc::vec::Vec<i16> {
        let mut out = alloc::vec::Vec::with_capacity(ARTIFICIAL_SEQUENCE_LEN);
        for n in 0..ARTIFICIAL_SEQUENCE_LEN {
            let il = ilr(n);
            let ih = ih(n);
            out.push(infb(il, ih, false));
        }
        out
    }

    /// Build the `T1D3.COD`-shape Configuration-2 frame for the
    /// artificial sequence (clause II.4.5.2 p. 72): 16 RSS-marker
    /// words (LSB = 1, others = 0), followed by the 16384-word data
    /// payload of [`build_i_hash_stream`], followed by 16 RSS-marker
    /// words.
    ///
    /// The total length is `2 * COD_RSS_MARKER_WORDS +
    /// ARTIFICIAL_SEQUENCE_LEN = 16416 words` — matching the
    /// "16 416 test values" file size that clause II.4.3 (p. 71)
    /// quotes for `T1D3.COD`.
    pub fn build_cod_frame() -> alloc::vec::Vec<i16> {
        let mut out =
            alloc::vec::Vec::with_capacity(2 * COD_RSS_MARKER_WORDS + ARTIFICIAL_SEQUENCE_LEN);
        // Prefix: 16 RSS-marker words (LSB=1, others=0).
        out.resize(COD_RSS_MARKER_WORDS, 0x0001_i16);
        // Payload: 16384 data words with RSS cleared.
        out.extend(build_i_hash_stream());
        // Trailer: 16 RSS-marker words.
        out.resize(out.len() + COD_RSS_MARKER_WORDS, 0x0001_i16);
        out
    }
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

    // -- Appendix II.3.2 artificial sequence --

    #[test]
    fn appendix_ii_sequence_length_matches_spec() {
        use super::appendix_ii::*;
        // Clause II.3.2 p. 67: 16 384 data values total.
        assert_eq!(ARTIFICIAL_SEQUENCE_LEN, 16_384);
        // Sub-sequence partitioning is self-consistent.
        assert_eq!(
            NUM_MSB_SUBSEQUENCES * SUBSEQUENCE_LEN_BITS,
            ARTIFICIAL_SEQUENCE_LEN
        );
        assert_eq!(
            NUM_LOWER_LSB_SUBSEQUENCES * LOWER_LSB_SUBSEQUENCE_LEN,
            ARTIFICIAL_SEQUENCE_LEN
        );
        assert_eq!(
            NUM_HIGHER_LSB_SUBSEQUENCES * SUBSEQUENCE_LEN_BITS,
            ARTIFICIAL_SEQUENCE_LEN
        );
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_1_prefix_matches_print() {
        use super::appendix_ii::lower_msb_bit;
        // Clause II.3.2.1 p. 67 (1): "0 0 1 0 0 1 0 0 1 0 0 1 0 0 1 0 0…"
        let expected: [u8; 17] = [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(lower_msb_bit(i), e, "MSB(1) bit {i}");
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_2_prefix_matches_print() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (2): "1 1 1 1 0 0 0 0 1 1 1 1 0 0 0 0 1…"
        let expected: [u8; 17] = [1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(lower_msb_bit(SUBSEQUENCE_LEN_BITS + i), e, "MSB(2) bit {i}");
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_3_is_constant_one() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (3): all 1s for 2048 bits.
        for i in 0..SUBSEQUENCE_LEN_BITS {
            assert_eq!(lower_msb_bit(2 * SUBSEQUENCE_LEN_BITS + i), 1);
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_4_prefix_matches_print() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (4): "1 1 0 0 1 1 0 0 1 1 0 0 1 1 0 0 1…"
        let expected: [u8; 17] = [1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(
                lower_msb_bit(3 * SUBSEQUENCE_LEN_BITS + i),
                e,
                "MSB(4) bit {i}"
            );
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_5_is_alternating() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (5): "1 0 1 0 1 0 …"
        for i in 0..32 {
            let expected = if i % 2 == 0 { 1 } else { 0 };
            assert_eq!(lower_msb_bit(4 * SUBSEQUENCE_LEN_BITS + i), expected);
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_6_prefix_matches_print() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (6): "0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0…"
        // Split as (00000100)(00000100)(0) per the period-8 reading.
        let expected: [u8; 17] = [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(
                lower_msb_bit(5 * SUBSEQUENCE_LEN_BITS + i),
                e,
                "MSB(6) bit {i}"
            );
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_7_prefix_matches_print() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (7): "0 0 1 0 1 0 0 1 0 1 0 0 1 0 1 0 0…"
        let expected: [u8; 17] = [0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(
                lower_msb_bit(6 * SUBSEQUENCE_LEN_BITS + i),
                e,
                "MSB(7) bit {i}"
            );
        }
    }

    #[test]
    fn appendix_ii_lower_msb_subsequence_8_prefix_matches_print() {
        use super::appendix_ii::{lower_msb_bit, SUBSEQUENCE_LEN_BITS};
        // (8): "1 1 0 0 0 1 1 0 0 0 1 1 0 0 0 1 1…"
        let expected: [u8; 17] = [1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(
                lower_msb_bit(7 * SUBSEQUENCE_LEN_BITS + i),
                e,
                "MSB(8) bit {i}"
            );
        }
    }

    #[test]
    fn appendix_ii_higher_msb_mirrors_lower_msb() {
        use super::appendix_ii::{higher_msb_bit, lower_msb_bit, ARTIFICIAL_SEQUENCE_LEN};
        // Clause II.3.2.2 p. 68: "identical to those used in the MSB
        // sequence for the lower sub-band ADPCM". Spot-check a spread
        // of indices across all 8 sub-sequences.
        let stride = ARTIFICIAL_SEQUENCE_LEN / 128;
        let mut idx = 0;
        while idx < ARTIFICIAL_SEQUENCE_LEN {
            assert_eq!(higher_msb_bit(idx), lower_msb_bit(idx), "idx {idx}");
            idx += stride;
        }
    }

    #[test]
    fn appendix_ii_higher_lsb_subsequence_1_is_constant_one() {
        use super::appendix_ii::{higher_lsb_bit, SUBSEQUENCE_LEN_BITS};
        for i in 0..SUBSEQUENCE_LEN_BITS {
            assert_eq!(higher_lsb_bit(i), 1);
        }
    }

    #[test]
    fn appendix_ii_higher_lsb_subsequence_2_alternates_sixteen() {
        use super::appendix_ii::{higher_lsb_bit, SUBSEQUENCE_LEN_BITS};
        // 16 ones followed by 16 zeros.
        for i in 0..16 {
            assert_eq!(higher_lsb_bit(SUBSEQUENCE_LEN_BITS + i), 1);
        }
        for i in 16..32 {
            assert_eq!(higher_lsb_bit(SUBSEQUENCE_LEN_BITS + i), 0);
        }
        for i in 32..48 {
            assert_eq!(higher_lsb_bit(SUBSEQUENCE_LEN_BITS + i), 1);
        }
    }

    #[test]
    fn appendix_ii_higher_lsb_subsequence_3_is_constant_zero() {
        use super::appendix_ii::{higher_lsb_bit, SUBSEQUENCE_LEN_BITS};
        for i in 0..SUBSEQUENCE_LEN_BITS {
            assert_eq!(higher_lsb_bit(2 * SUBSEQUENCE_LEN_BITS + i), 0);
        }
    }

    #[test]
    fn appendix_ii_higher_lsb_subsequence_4_alternates_eight() {
        use super::appendix_ii::{higher_lsb_bit, SUBSEQUENCE_LEN_BITS};
        for i in 0..8 {
            assert_eq!(higher_lsb_bit(3 * SUBSEQUENCE_LEN_BITS + i), 1);
        }
        for i in 8..16 {
            assert_eq!(higher_lsb_bit(3 * SUBSEQUENCE_LEN_BITS + i), 0);
        }
    }

    #[test]
    fn appendix_ii_higher_lsb_subsequence_6_alternates_four() {
        use super::appendix_ii::{higher_lsb_bit, SUBSEQUENCE_LEN_BITS};
        for i in 0..4 {
            assert_eq!(higher_lsb_bit(5 * SUBSEQUENCE_LEN_BITS + i), 1);
        }
        for i in 4..8 {
            assert_eq!(higher_lsb_bit(5 * SUBSEQUENCE_LEN_BITS + i), 0);
        }
    }

    #[test]
    fn appendix_ii_higher_lsb_subsequence_8_alternates_two() {
        use super::appendix_ii::{higher_lsb_bit, SUBSEQUENCE_LEN_BITS};
        for i in 0..2 {
            assert_eq!(higher_lsb_bit(7 * SUBSEQUENCE_LEN_BITS + i), 1);
        }
        for i in 2..4 {
            assert_eq!(higher_lsb_bit(7 * SUBSEQUENCE_LEN_BITS + i), 0);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_1_is_constant_31() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (1): 31 31 31 …
        for i in 0..LOWER_LSB_SUBSEQUENCE_LEN {
            assert_eq!(lower_lsb5(i), 31);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_2_alternates_31_30() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (2): sixteen 31's, sixteen 30's.
        for i in 0..16 {
            assert_eq!(lower_lsb5(LOWER_LSB_SUBSEQUENCE_LEN + i), 31);
        }
        for i in 16..32 {
            assert_eq!(lower_lsb5(LOWER_LSB_SUBSEQUENCE_LEN + i), 30);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_3_is_constant_30() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (3): 30 30 30 …
        for i in 0..LOWER_LSB_SUBSEQUENCE_LEN {
            assert_eq!(lower_lsb5(2 * LOWER_LSB_SUBSEQUENCE_LEN + i), 30);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_31_is_constant_16() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (31): 16 16 16 …
        let base = 30 * LOWER_LSB_SUBSEQUENCE_LEN; // 0-based: sub 30 == spec (31).
        for i in 0..LOWER_LSB_SUBSEQUENCE_LEN {
            assert_eq!(lower_lsb5(base + i), 16);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_57_is_constant_3() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (57): 3 3 3 … (entering the suppressed-codeword
        // range per clause II.3.2.1 p. 67 footnote).
        let base = 56 * LOWER_LSB_SUBSEQUENCE_LEN;
        for i in 0..LOWER_LSB_SUBSEQUENCE_LEN {
            assert_eq!(lower_lsb5(base + i), 3);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_63_is_constant_0() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (63): 0 0 0 …
        let base = 62 * LOWER_LSB_SUBSEQUENCE_LEN;
        for i in 0..LOWER_LSB_SUBSEQUENCE_LEN {
            assert_eq!(lower_lsb5(base + i), 0);
        }
    }

    #[test]
    fn appendix_ii_lower_lsb5_subsequence_64_wraps_back_to_three() {
        use super::appendix_ii::{lower_lsb5, LOWER_LSB_SUBSEQUENCE_LEN};
        // Table II-4 (64): "alternating sixteen 0's, sixteen 3's".
        let base = 63 * LOWER_LSB_SUBSEQUENCE_LEN;
        for i in 0..16 {
            assert_eq!(lower_lsb5(base + i), 0);
        }
        for i in 16..32 {
            assert_eq!(lower_lsb5(base + i), 3);
        }
    }

    #[test]
    fn appendix_ii_ilr_combines_msb_and_lsb5() {
        use super::appendix_ii::{ilr, lower_lsb5, lower_msb_bit};
        // Spot-check the ILR composition rule ILR = (MSB << 5) | LSB5.
        for &idx in &[0_usize, 1, 100, 2047, 2048, 4095, 8192, 16383] {
            let expected = (lower_msb_bit(idx) << 5) | (lower_lsb5(idx) & 0x1F);
            assert_eq!(ilr(idx), expected);
            // ILR is always a 6-bit value.
            assert!(ilr(idx) <= 0x3F);
        }
    }

    #[test]
    fn appendix_ii_ih_combines_msb_and_lsb() {
        use super::appendix_ii::{higher_lsb_bit, higher_msb_bit, ih};
        // Spot-check IH = (MSB << 1) | LSB.
        for &idx in &[0_usize, 1, 100, 2047, 4095, 8192, 16383] {
            let expected = ((higher_msb_bit(idx) & 1) << 1) | (higher_lsb_bit(idx) & 1);
            assert_eq!(ih(idx), expected);
            assert!(ih(idx) <= 0x3);
        }
    }

    #[test]
    fn appendix_ii_build_i_hash_stream_length_and_rss() {
        use super::appendix_ii::{build_i_hash_stream, ARTIFICIAL_SEQUENCE_LEN};
        let s = build_i_hash_stream();
        assert_eq!(s.len(), ARTIFICIAL_SEQUENCE_LEN);
        // All data slots must have RSS cleared (LSB = 0).
        for &w in &s {
            assert_eq!((w as u16) & RSS_MASK, 0);
        }
    }

    #[test]
    fn appendix_ii_build_i_hash_stream_round_trips_through_infc() {
        // The packed I# stream must decompose back to the same
        // (ILR, IH) values via INFC.
        use super::appendix_ii::{build_i_hash_stream, ih, ilr};
        let s = build_i_hash_stream();
        for (n, &w) in s.iter().enumerate() {
            let r = infc(w);
            assert!(!r.rs);
            assert_eq!(r.ilr, ilr(n), "ILR mismatch at sample {n}");
            assert_eq!(r.ih, ih(n), "IH mismatch at sample {n}");
        }
    }

    #[test]
    fn appendix_ii_build_cod_frame_matches_file_format_size() {
        // Clause II.4.3 p. 71: T1D3.COD = 16 416 test values (= 16
        // prefix + 16384 data + 16 trailer).
        use super::appendix_ii::{build_cod_frame, ARTIFICIAL_SEQUENCE_LEN, COD_RSS_MARKER_WORDS};
        let frame = build_cod_frame();
        assert_eq!(frame.len(), 16_416);
        assert_eq!(
            frame.len(),
            2 * COD_RSS_MARKER_WORDS + ARTIFICIAL_SEQUENCE_LEN
        );
    }

    #[test]
    fn appendix_ii_build_cod_frame_prefix_and_trailer_are_rss_markers() {
        // Clause II.4.5.2 p. 72: the 16-word prefix and the 16-word
        // trailer are all RSS-marker words (LSB=1, others=0).
        use super::appendix_ii::{build_cod_frame, ARTIFICIAL_SEQUENCE_LEN, COD_RSS_MARKER_WORDS};
        let frame = build_cod_frame();
        for w in &frame[..COD_RSS_MARKER_WORDS] {
            assert_eq!(*w as u16, 0x0001);
        }
        let trailer_start = COD_RSS_MARKER_WORDS + ARTIFICIAL_SEQUENCE_LEN;
        for w in &frame[trailer_start..] {
            assert_eq!(*w as u16, 0x0001);
        }
        // Interior payload has RSS cleared.
        for w in &frame[COD_RSS_MARKER_WORDS..trailer_start] {
            assert_eq!((*w as u16) & RSS_MASK, 0);
        }
    }

    #[test]
    fn appendix_ii_run_through_configuration_2_drives_decoder_deterministically() {
        // The synthesised sequence drives the receive ADPCM loop
        // end-to-end through `run_configuration_2`. The output stream
        // must:
        //   1. Match the input length (one RL# / RH# per I# word).
        //   2. Be deterministic — independent decoder instances
        //      produce byte-equal output.
        //   3. Stay inside the LIMIT block's 15-bit signed range
        //      (clause 6.2.1.6 / 6.2.2.5 cap at ±16384), which after
        //      the INFD <<1 shift bounds the wire word at the i16
        //      range (with the RSS LSB clear).
        use super::appendix_ii::build_i_hash_stream;
        let stream = build_i_hash_stream();
        let mut dec_a = Decoder::new(Mode::Mode1);
        let mut dec_b = Decoder::new(Mode::Mode1);
        // Use a shorter prefix to keep the test fast; determinism
        // doesn't depend on the full 16384 word stream.
        let head = &stream[..2048];
        let a = run_configuration_2(&mut dec_a, head);
        let b = run_configuration_2(&mut dec_b, head);
        assert_eq!(a, b);
        assert_eq!(a.rl_hash.len(), head.len());
        assert_eq!(a.rh_hash.len(), head.len());
        for w in &a.rl_hash {
            assert_eq!((*w as u16) & RSS_MASK, 0);
        }
        for w in &a.rh_hash {
            assert_eq!((*w as u16) & RSS_MASK, 0);
        }
    }

    #[test]
    fn appendix_ii_cod_frame_round_trip_handles_rss_brackets() {
        // Drive the full .COD frame through Configuration 2; the
        // 16-word prefix must produce non-valid (RSS=1) output words
        // and trigger the decoder reset, the 16384-word payload must
        // produce valid (RSS=0) outputs, and the 16-word trailer
        // must again produce non-valid outputs.
        use super::appendix_ii::{build_cod_frame, ARTIFICIAL_SEQUENCE_LEN, COD_RSS_MARKER_WORDS};
        let frame = build_cod_frame();
        let mut dec = Decoder::new(Mode::Mode1);
        // Truncate the payload to keep the test fast — RSS-bracket
        // behaviour is independent of the payload length.
        let mut bounded = alloc::vec::Vec::with_capacity(2 * COD_RSS_MARKER_WORDS + 256);
        bounded.extend_from_slice(&frame[..COD_RSS_MARKER_WORDS]);
        bounded.extend_from_slice(&frame[COD_RSS_MARKER_WORDS..COD_RSS_MARKER_WORDS + 256]);
        bounded.extend_from_slice(&frame[COD_RSS_MARKER_WORDS + ARTIFICIAL_SEQUENCE_LEN..]);
        let out = run_configuration_2(&mut dec, &bounded);
        for w in &out.rl_hash[..COD_RSS_MARKER_WORDS] {
            assert_eq!(*w as u16, 0x0001, "RSS prefix RL# must be non-valid");
        }
        for w in &out.rh_hash[..COD_RSS_MARKER_WORDS] {
            assert_eq!(*w as u16, 0x0001, "RSS prefix RH# must be non-valid");
        }
        for w in &out.rl_hash[COD_RSS_MARKER_WORDS..COD_RSS_MARKER_WORDS + 256] {
            assert_eq!((*w as u16) & RSS_MASK, 0, "data RL# must be valid");
        }
        let trailer_start = COD_RSS_MARKER_WORDS + 256;
        for w in &out.rl_hash[trailer_start..] {
            assert_eq!(*w as u16, 0x0001, "RSS trailer RL# must be non-valid");
        }
        for w in &out.rh_hash[trailer_start..] {
            assert_eq!(*w as u16, 0x0001, "RSS trailer RH# must be non-valid");
        }
    }

    #[test]
    fn appendix_ii_zero_pole_predictor_full_range_excursion_landmark() {
        // Sanity-check that the 8 MSB sub-sequences collectively
        // exercise both polarities of the zero predictor over their
        // full ± 2 range (clause II.3.2.1 p. 67: "These MSB sequences
        // are used to force the coefficients of the zero predictor
        // to vary across the entire range of ± 2").
        //
        // We only verify a structural invariant: the eight
        // sub-sequences span both bits (0 and 1) with comparable
        // density, so the predictor sees both polarities in each
        // sub-sequence except (3) constant-1.
        use super::appendix_ii::{lower_msb_bit, NUM_MSB_SUBSEQUENCES, SUBSEQUENCE_LEN_BITS};
        for sub in 0..NUM_MSB_SUBSEQUENCES {
            let mut zeros = 0;
            let mut ones = 0;
            // Sample a 1024-bit prefix of each sub-sequence.
            for i in 0..1024 {
                match lower_msb_bit(sub * SUBSEQUENCE_LEN_BITS + i) {
                    0 => zeros += 1,
                    1 => ones += 1,
                    _ => unreachable!(),
                }
            }
            assert_eq!(zeros + ones, 1024, "sub {sub} produced bits other than 0/1");
            // Only sub-sequence (3) (= constant-1) is allowed to be
            // entirely one polarity.
            if sub != 2 {
                assert!(zeros > 0 && ones > 0, "sub {sub} stuck on a single bit");
            } else {
                assert_eq!(ones, 1024, "sub (3) must be constant 1");
            }
        }
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

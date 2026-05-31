//! Numeric tables transcribed from ITU-T Recommendation G.722.
//!
//! Every constant in this file is sourced directly from the staged
//! Recommendation PDF `docs/audio/g722/T-REC-G.722-198811-S.pdf`
//! (Blue-Book base edition, 1988-11). Spec table / page citations
//! below refer to that document so a reader can audit each value
//! against the printed normative tables. No external reference
//! implementation, no FFmpeg / libav* source, no third-party G.722
//! source distribution, and no online resources were consulted.

// -----------------------------------------------------------------------
// Table 11/G.722 — QMF coefficient values (scaled by 2^13). PDF p. 27.
//
// The spec lists only the 12 distinct values H0..H11 = H23..H12; the
// 24-tap filter is the symmetric extension `H_i = H_{23 - i}`.
// -----------------------------------------------------------------------

/// QMF filter taps `H_0 .. H_23` (Table 11/G.722, p. 27). Used by both
/// the receive QMF (clause 4.4) and the transmit QMF (clause 3.1).
pub const QMF_TAPS: [i32; 24] = [
    3, -11, -11, 53, 12, -156, 32, 362, -210, -805, 951, 3876, 3876, 951, -805, -210, 362, 32,
    -156, 12, 53, -11, -11, 3,
];

// -----------------------------------------------------------------------
// Table 14/G.722 — Quantizer decision levels and output values.
// PDF p. 35. All four arrays below transcribe single columns of the
// lower-sub-band block of this table:
//
//   * `Q6`  = column "Q6"  — the 6-bit decision levels Q6(1..29)
//             scaled per the QUANTL pseudo-code on p. 42.
//   * `QQ6` = column "QQ6" — the 6-bit inverse-quantizer outputs.
//   * `QQ5` = column "QQ5" — the 5-bit inverse-quantizer outputs.
//   * `QQ4` = column "QQ4" — the 4-bit inverse-quantizer outputs.
//   * `WL`  = column "WL"  — the lower-sub-band log-scale multiplier.
//
// Address 0 entries are unused placeholders in the spec; this code
// mirrors the spec's 1-based addressing with a leading sentinel.
// -----------------------------------------------------------------------

/// Lower sub-band 6-bit quantizer decision levels `Q6(1..29)` (Table
/// 14/G.722, p. 35). Index 0 / 30 / 31 are sentinels; the upper
/// decision boundary `LDU` of m_L = 30 is implicitly +∞ (per the
/// "otherwise" row of the QUANTL decision table on p. 42). The
/// QUANTL pseudo-code applies these as `(Q6(k) << 3) * DETL`, so the
/// values here are already pre-scaled for that combination.
pub const Q6: [i32; 32] = [
    0, 35, 72, 110, 150, 190, 233, 276, 323, 370, 422, 473, 530, 587, 650, 714, 786, 858, 940,
    1023, 1121, 1219, 1339, 1458, 1612, 1765, 1980, 2195, 2557, 2919, 0, 0,
];

/// Lower sub-band 6-bit inverse-quantizer output magnitudes
/// `QQ6(1..30)` (Table 14/G.722, p. 35). Index 0 is a sentinel.
pub const QQ6: [i32; 31] = [
    0, 17, 54, 91, 130, 170, 211, 254, 300, 347, 396, 447, 501, 558, 618, 682, 750, 822, 899, 982,
    1072, 1170, 1279, 1399, 1535, 1689, 1873, 2088, 2376, 2738, 3101,
];

/// Lower sub-band 5-bit inverse-quantizer output magnitudes
/// `QQ5(1..15)` (Table 14/G.722, p. 35). Index 0 is a sentinel.
pub const QQ5: [i32; 16] = [
    0, 35, 110, 190, 276, 370, 473, 587, 714, 858, 1023, 1219, 1458, 1765, 2195, 2919,
];

/// Lower sub-band 4-bit inverse-quantizer output magnitudes
/// `QQ4(1..7)` (Table 14/G.722, p. 35). Index 0 is a sentinel;
/// `QQ4(1) = 0`.
pub const QQ4: [i32; 8] = [0, 0, 150, 323, 530, 786, 1121, 1612];

/// Lower sub-band logarithmic-scale-factor multipliers `WL` (Table
/// 14/G.722, p. 35). Indexed by the truncated 4-bit codeword
/// magnitude `IL4` in the range 0..=7.
pub const WL: [i32; 8] = [-60, -30, 58, 172, 334, 538, 1198, 3042];

/// Higher sub-band 2-bit quantizer decision level `Q2(1)` (Table
/// 14/G.722, p. 35). This is the only non-trivial decision level in
/// the 2-bit quantizer; the upper boundary is implicitly +∞.
pub const Q2_LEVEL_1: i32 = 564;

// -----------------------------------------------------------------------
// Table 16/G.722 — Conversion from quantizer intervals to 6-bit output
// codewords. PDF p. 38. The encoder uses this table to pack `(SIL,
// MIL)` from the QUANTL output into the wire-format 6-bit `IL`
// codeword. We split the table into two arrays keyed on the magnitude
// `m_L = 1..=30`: one for each sign (positive / negative).
// -----------------------------------------------------------------------

/// 6-bit code-word `IL` emitted for `(SIL = 0, MIL = m_L)`, i.e. the
/// non-negative-difference column of Table 16/G.722 (p. 38). Index 0
/// is a sentinel.
pub const ILP6_FROM_ML: [u8; 31] = [
    0,        // sentinel
    0b111101, // m_L = 1
    0b111100, // 2
    0b111011, // 3
    0b111010, // 4
    0b111001, // 5
    0b111000, // 6
    0b110111, // 7
    0b110110, // 8
    0b110101, // 9
    0b110100, // 10
    0b110011, // 11
    0b110010, // 12
    0b110001, // 13
    0b110000, // 14
    0b101111, // 15
    0b101110, // 16
    0b101101, // 17
    0b101100, // 18
    0b101011, // 19
    0b101010, // 20
    0b101001, // 21
    0b101000, // 22
    0b100111, // 23
    0b100110, // 24
    0b100101, // 25
    0b100100, // 26
    0b100011, // 27
    0b100010, // 28
    0b100001, // 29
    0b100000, // 30
];

/// 6-bit code-word `IL` emitted for `(SIL = -1, MIL = m_L)`, i.e. the
/// negative-difference column of Table 16/G.722 (p. 38). Index 0 is a
/// sentinel.
pub const ILN6_FROM_ML: [u8; 31] = [
    0,        // sentinel
    0b111111, // m_L = 1
    0b111110, // 2
    0b011111, // 3
    0b011110, // 4
    0b011101, // 5
    0b011100, // 6
    0b011011, // 7
    0b011010, // 8
    0b011001, // 9
    0b011000, // 10
    0b010111, // 11
    0b010110, // 12
    0b010101, // 13
    0b010100, // 14
    0b010011, // 15
    0b010010, // 16
    0b010001, // 17
    0b010000, // 18
    0b001111, // 19
    0b001110, // 20
    0b001101, // 21
    0b001100, // 22
    0b001011, // 23
    0b001010, // 24
    0b001001, // 25
    0b001000, // 26
    0b000111, // 27
    0b000110, // 28
    0b000101, // 29
    0b000100, // 30
];

// -----------------------------------------------------------------------
// Table 20/G.722 — Conversion from quantizer intervals to 2-bit output
// codewords. PDF p. 40. Mirrors Table 16 for the higher sub-band.
// -----------------------------------------------------------------------

/// 2-bit code-word `IH` emitted for `(SIH = 0, MIH = m_H)` (Table
/// 20/G.722, p. 40). Index 0 is a sentinel.
pub const IHP2_FROM_MH: [u8; 3] = [
    0,    // sentinel
    0b11, // m_H = 1
    0b10, // m_H = 2
];

/// 2-bit code-word `IH` emitted for `(SIH = -1, MIH = m_H)` (Table
/// 20/G.722, p. 40). Index 0 is a sentinel.
pub const IHN2_FROM_MH: [u8; 3] = [
    0,    // sentinel
    0b01, // m_H = 1
    0b00, // m_H = 2
];

/// Higher sub-band 2-bit inverse-quantizer output magnitudes
/// `QQ2(1 .. 2)` (Table 14, page 30). Index 0 is a sentinel.
pub const QQ2: [i32; 3] = [0, 202, 926];

/// Higher sub-band logarithmic-scale-factor multipliers `WH(1 .. 2)`
/// (Table 14, page 30). Index 0 is a sentinel; the index is the
/// 2-bit code-word magnitude `IH2`.
pub const WH: [i32; 3] = [0, -214, 798];

// -----------------------------------------------------------------------
// Table 15 ILB — 32-entry log-to-linear conversion table. PDF page 32.
//
// Used by SCALEL (Method 2) / SCALEH (Method 2) at clause 6.2.1.3 and
// 6.2.2.3. The 353-entry ILA table is omitted here as the codec uses
// Method 2 throughout for the smaller footprint.
// -----------------------------------------------------------------------

/// Log-to-linear conversion table `ILB(0 .. 31)` (Table 15, page 32).
/// Address is `i + j` per spec note 1.
pub const ILB: [i32; 32] = [
    2048, 2093, 2139, 2186, 2233, 2282, 2332, 2383, 2435, 2489, 2543, 2599, 2656, 2714, 2774, 2834,
    2896, 2960, 3025, 3091, 3158, 3228, 3298, 3371, 3444, 3520, 3597, 3676, 3756, 3838, 3922, 4008,
];

// -----------------------------------------------------------------------
// Table 17/G.722 — Conversion from 4-bit codewords to quantizer
// intervals. PDF p. 38. The lower-sub-band 4-bit inverse quantizer
// (used as INVQAL during predictor adaptation, and also as the
// Mode-3 receive path when the two LSBs of the wire 6-bit codeword
// have been replaced by auxiliary data) recovers `(SIL, IL4)` from
// this table.
// -----------------------------------------------------------------------

/// Sign bit `SIL` per 4-bit `RIL` codeword (Table 17/G.722, p. 38).
pub const SIL_FROM_IL4: [i32; 16] = [
    0, -1, -1, -1, -1, -1, -1, -1, // RIL = 0000 .. 0111
    0, 0, 0, 0, 0, 0, 0, 0, // RIL = 1000 .. 1111
];

/// Magnitude `IL4` per 4-bit `RIL` codeword (Table 17/G.722, p. 38).
pub const IL4_FROM_IL4: [u8; 16] = [
    0, 7, 6, 5, 4, 3, 2, 1, // RIL = 0000 .. 0111
    7, 6, 5, 4, 3, 2, 1, 0, // RIL = 1000 .. 1111
];

// -----------------------------------------------------------------------
// Table 18/G.722 — Conversion from 6-bit codewords to quantizer
// intervals. PDF p. 39. The receive-side INVQBL / INVQAL blocks index
// the (`SIL`, `IL6`) pair out of this table from the 6-bit `RIL`
// codeword extracted by the demultiplexer.
//
// Per the table's footnote (p. 39): "it is possible for the decoder
// to receive the codewords 000000, 000001, 000010 and 000011 due to
// transmission errors." Those four reserved entries are mapped to
// `(SIL = -1, IL6 = 1)` in the spec table, matching the Mode-1
// substitution rule of Table 5 (page 18).
// -----------------------------------------------------------------------

/// Sign bit `SIL` per 6-bit `RIL` codeword (Table 18/G.722, p. 39).
pub const SIL_FROM_IL6: [i32; 64] = build_sil6();

/// Magnitude `IL6` (= `m_L`) per 6-bit `RIL` codeword (Table
/// 18/G.722, p. 39).
pub const IL6_FROM_IL6: [u8; 64] = build_il6();

const fn build_sil6() -> [i32; 64] {
    // Default: top bit of RIL clear ⇒ SIL = -1 (negative half); top
    // bit set ⇒ SIL = 0 (positive half). Two exceptions: RIL =
    // 111110 / 111111 are listed in the SIL = -1 column of Table 18
    // even though their top bit is 1 (they are the small-magnitude
    // negative codewords ILN(2) = 0b111110 and ILN(1) = 0b111111
    // from Table 16/G.722).
    let mut a = [0_i32; 64];
    let mut i = 0;
    while i < 64 {
        if (i >> 5) & 1 == 0 {
            a[i] = -1;
        } else {
            a[i] = 0;
        }
        i += 1;
    }
    a[0b111110] = -1;
    a[0b111111] = -1;
    a
}

const fn build_il6() -> [u8; 64] {
    // Transcribe Table 18/G.722 (p. 39) row by row. The codeword
    // address space is split into two halves: top-bit-clear (SIL = -1,
    // the ILN-style entries) and top-bit-set (SIL = 0, ILP-style).
    let mut a = [0_u8; 64];

    // SIL = -1 half (top bit of RIL is 0):
    //   RIL = 000000..000011 substitute to IL6 = 1 per footnote.
    a[0b000000] = 1;
    a[0b000001] = 1;
    a[0b000010] = 1;
    a[0b000011] = 1;
    a[0b000100] = 30;
    a[0b000101] = 29;
    a[0b000110] = 28;
    a[0b000111] = 27;
    a[0b001000] = 26;
    a[0b001001] = 25;
    a[0b001010] = 24;
    a[0b001011] = 23;
    a[0b001100] = 22;
    a[0b001101] = 21;
    a[0b001110] = 20;
    a[0b001111] = 19;
    a[0b010000] = 18;
    a[0b010001] = 17;
    a[0b010010] = 16;
    a[0b010011] = 15;
    a[0b010100] = 14;
    a[0b010101] = 13;
    a[0b010110] = 12;
    a[0b010111] = 11;
    a[0b011000] = 10;
    a[0b011001] = 9;
    a[0b011010] = 8;
    a[0b011011] = 7;
    a[0b011100] = 6;
    a[0b011101] = 5;
    a[0b011110] = 4;
    a[0b011111] = 3;
    // The two entries 111110 / 111111 belong to the "SIL = -1" column
    // of Table 18 even though their top bit is 1; this is the spec's
    // explicit handling for the small-magnitude negative codewords.
    a[0b111110] = 2;
    a[0b111111] = 1;

    // SIL = 0 half (top bit of RIL is 1):
    a[0b111101] = 1;
    a[0b111100] = 2;
    a[0b111011] = 3;
    a[0b111010] = 4;
    a[0b111001] = 5;
    a[0b111000] = 6;
    a[0b110111] = 7;
    a[0b110110] = 8;
    a[0b110101] = 9;
    a[0b110100] = 10;
    a[0b110011] = 11;
    a[0b110010] = 12;
    a[0b110001] = 13;
    a[0b110000] = 14;
    a[0b101111] = 15;
    a[0b101110] = 16;
    a[0b101101] = 17;
    a[0b101100] = 18;
    a[0b101011] = 19;
    a[0b101010] = 20;
    a[0b101001] = 21;
    a[0b101000] = 22;
    a[0b100111] = 23;
    a[0b100110] = 24;
    a[0b100101] = 25;
    a[0b100100] = 26;
    a[0b100011] = 27;
    a[0b100010] = 28;
    a[0b100001] = 29;
    a[0b100000] = 30;
    a
}

// -----------------------------------------------------------------------
// Table 19/G.722 — Conversion from 5-bit codewords to quantizer
// intervals. PDF p. 40. Used by the Mode-2 lower-sub-band inverse
// quantizer (the encoder transmits a 6-bit codeword from which the
// data-extraction device drops 1 LSB per Table 2/G.722).
// -----------------------------------------------------------------------

/// Sign bit `SIL` per truncated 5-bit `RIL` codeword (Mode 2).
pub const SIL_FROM_IL5: [i32; 32] = build_sil5();

/// Magnitude `IL5` per truncated 5-bit `RIL` codeword (Mode 2).
pub const IL5_FROM_IL5: [u8; 32] = build_il5();

const fn build_sil5() -> [i32; 32] {
    let mut a = [0_i32; 32];
    let mut i = 0;
    while i < 32 {
        // Top bit of RIL clear => SIL = -1, top bit set => SIL = 0.
        if (i >> 4) & 1 == 0 {
            a[i] = -1;
        } else {
            a[i] = 0;
        }
        i += 1;
    }
    a
}

const fn build_il5() -> [u8; 32] {
    let mut a = [0_u8; 32];
    // SIL = -1 half (RIL top bit = 0)
    a[0b00000] = 1; // substituted
    a[0b00001] = 1; // substituted
    a[0b00010] = 15;
    a[0b00011] = 14;
    a[0b00100] = 13;
    a[0b00101] = 12;
    a[0b00110] = 11;
    a[0b00111] = 10;
    a[0b01000] = 9;
    a[0b01001] = 8;
    a[0b01010] = 7;
    a[0b01011] = 6;
    a[0b01100] = 5;
    a[0b01101] = 4;
    a[0b01110] = 3;
    a[0b01111] = 2;
    // SIL = 0 half (RIL top bit = 1)
    a[0b11111] = 1;
    a[0b11110] = 1;
    a[0b11101] = 2;
    a[0b11100] = 3;
    a[0b11011] = 4;
    a[0b11010] = 5;
    a[0b11001] = 6;
    a[0b11000] = 7;
    a[0b10111] = 8;
    a[0b10110] = 9;
    a[0b10101] = 10;
    a[0b10100] = 11;
    a[0b10011] = 12;
    a[0b10010] = 13;
    a[0b10001] = 14;
    a[0b10000] = 15;
    a
}

// -----------------------------------------------------------------------
// Table 6/G.722 — Decision levels and output codes for the 2-bit
// higher sub-band quantizer. PDF p. 19.
//
// The IHN column gives the 2-bit codes for negative differences,
// IHP for non-negative. We split that into the (sign, magnitude)
// pair used by `INVQAH`.
// -----------------------------------------------------------------------

/// Sign bit `SIH` per 2-bit `IH` codeword (Table 6/G.722, p. 19).
pub const SIH_FROM_IH: [i32; 4] = [
    -1, // IH = 00
    -1, // IH = 01
    0,  // IH = 10
    0,  // IH = 11
];

/// Magnitude `IH2` (= `m_H`) per 2-bit `IH` codeword (Table 6/G.722,
/// p. 19). Index 0 / 1 of the codeword space carry the IHN column
/// (negative differences); index 2 / 3 carry the IHP column.
pub const IH2_FROM_IH: [u8; 4] = [
    2, // IH = 00
    1, // IH = 01
    2, // IH = 10
    1, // IH = 11
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qmf_taps_symmetric() {
        // QMF is a symmetric FIR; verify the printed table satisfies it.
        for i in 0..12 {
            assert_eq!(
                QMF_TAPS[i],
                QMF_TAPS[23 - i],
                "QMF tap pair {i}/{} mismatch",
                23 - i
            );
        }
    }

    #[test]
    fn ilb_monotonic() {
        // Log-to-linear table is monotonically increasing in the spec.
        for i in 1..ILB.len() {
            assert!(ILB[i] > ILB[i - 1]);
        }
        // First entry is 2048 (= 2^11) and last entry is 4008 — the
        // table covers exactly one octave of the inverse-log2 curve.
        assert_eq!(ILB[0], 2048);
        assert_eq!(ILB[31], 4008);
    }

    #[test]
    fn q6_monotonic_in_active_range() {
        for i in 2..=29 {
            assert!(
                Q6[i] > Q6[i - 1],
                "Q6 not monotonic at {i}: {} vs {}",
                Q6[i],
                Q6[i - 1]
            );
        }
    }

    #[test]
    fn qq_tables_monotonic() {
        for i in 2..QQ6.len() {
            assert!(QQ6[i] > QQ6[i - 1]);
        }
        for i in 2..QQ5.len() {
            assert!(QQ5[i] > QQ5[i - 1]);
        }
        // QQ4 starts at index 2 because QQ4(1) = 0 in the spec.
        for i in 3..QQ4.len() {
            assert!(QQ4[i] > QQ4[i - 1]);
        }
    }

    #[test]
    fn il6_table_covers_all_codewords() {
        // Every 6-bit codeword must map to a non-zero magnitude in
        // 1..=30 (Table 18 has no zero entries).
        for (code, &m) in IL6_FROM_IL6.iter().enumerate() {
            assert!(
                (1..=30).contains(&m),
                "IL6 codeword {code:#06b} -> {m} out of range"
            );
        }
    }

    #[test]
    fn il4_table_covers_all_codewords() {
        for (code, &m) in IL4_FROM_IL4.iter().enumerate() {
            assert!(m <= 7, "IL4 codeword {code:#04b} -> {m} out of range");
        }
    }

    #[test]
    fn ih_table_consistent() {
        // SIH and IH2 must agree on the encoding: the two negative
        // codewords are 00/01, positive are 10/11.
        assert_eq!(SIH_FROM_IH, [-1, -1, 0, 0]);
        assert_eq!(IH2_FROM_IH, [2, 1, 2, 1]);
    }

    #[test]
    fn forward_quantizer_codewords_invert_inverse_tables() {
        // Each `m_L` choice must round-trip through the inverse
        // demultiplexer tables (SIL_FROM_IL6 + IL6_FROM_IL6).
        // ILP corresponds to sign = + ⇒ SIL = 0, magnitude index = m_L.
        // ILN corresponds to sign = − ⇒ SIL = -1, magnitude index = m_L.
        for ml in 1..=30 {
            let ilp = ILP6_FROM_ML[ml] as usize;
            assert_eq!(SIL_FROM_IL6[ilp], 0, "ILP6({ml}) decodes to wrong sign");
            assert_eq!(
                IL6_FROM_IL6[ilp] as usize, ml,
                "ILP6({ml}) decodes to wrong magnitude"
            );

            let iln = ILN6_FROM_ML[ml] as usize;
            assert_eq!(SIL_FROM_IL6[iln], -1, "ILN6({ml}) decodes to wrong sign");
            assert_eq!(
                IL6_FROM_IL6[iln] as usize, ml,
                "ILN6({ml}) decodes to wrong magnitude"
            );
        }
    }

    #[test]
    fn higher_forward_quantizer_codewords_invert() {
        for mh in 1..=2 {
            let ihp = IHP2_FROM_MH[mh] as usize;
            assert_eq!(SIH_FROM_IH[ihp], 0);
            assert_eq!(IH2_FROM_IH[ihp] as usize, mh);
            let ihn = IHN2_FROM_MH[mh] as usize;
            assert_eq!(SIH_FROM_IH[ihn], -1);
            assert_eq!(IH2_FROM_IH[ihn] as usize, mh);
        }
    }
}

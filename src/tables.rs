//! ITU-T G.722 reference tables.
//!
//! These are the forward / inverse quantiser and log-step arrays given
//! verbatim in the ITU-T G.722 (09/2012) reference code — the same values
//! that appear in every widely-used implementation (SpanDSP `libg722`,
//! sippy/libg722, NAudio's `G722Codec.cs`, etc.).
//!
//! The ADPCM pipeline in [`crate::band_low`] / [`crate::band_high`] uses
//! these tables directly so the bitstream matches the ITU reference.
//!
//! Bit allocation per mode (low-band):
//!
//! | Mode | Bit rate  | Low-band bits | High-band bits | Aux bits | Table used |
//! |------|-----------|---------------|----------------|----------|------------|
//! | 1    | 64 kbit/s | 6             | 2              | 0        | `QM6`      |
//! | 2    | 56 kbit/s | 5             | 2              | 1        | `QM5`      |
//! | 3    | 48 kbit/s | 4             | 2              | 2        | `QM4`      |
//!
//! The high-band is always 2 bits and uses `QM2`.

#![allow(dead_code)]

/// 64-entry inverse quantiser table for the 6-bit low-band code (64 kbit/s
/// mode). Value at index `code` is the reconstructed quantised difference
/// before multiplication by `det` and a `>> 15` scaling.
pub const QM6: [i32; 64] = [
    -136, -136, -136, -136, -24808, -21904, -19008, -16704, -14984, -13512, -12280, -11192, -10232,
    -9360, -8576, -7856, -7192, -6576, -6000, -5456, -4944, -4464, -4008, -3576, -3168, -2776,
    -2400, -2032, -1688, -1360, -1040, -728, 24808, 21904, 19008, 16704, 14984, 13512, 12280,
    11192, 10232, 9360, 8576, 7856, 7192, 6576, 6000, 5456, 4944, 4464, 4008, 3576, 3168, 2776,
    2400, 2032, 1688, 1360, 1040, 728, 432, 136, -432, -136,
];

/// 32-entry inverse quantiser table for the 5-bit low-band code (56 kbit/s
/// mode).
pub const QM5: [i32; 32] = [
    -280, -280, -23352, -17560, -14120, -11664, -9752, -8184, -6864, -5712, -4696, -3784, -2960,
    -2208, -1520, -880, 23352, 17560, 14120, 11664, 9752, 8184, 6864, 5712, 4696, 3784, 2960, 2208,
    1520, 880, 280, -280,
];

/// 16-entry inverse quantiser table for the 4-bit low-band code (48 kbit/s
/// mode).
pub const QM4: [i32; 16] = [
    0, -20456, -12896, -8968, -6288, -4240, -2584, -1200, 20456, 12896, 8968, 6288, 4240, 2584,
    1200, 0,
];

/// 4-entry inverse quantiser table for the high-band 2-bit code (same at all
/// three rates).
pub const QM2: [i32; 4] = [-7408, -1616, 7408, 1616];

/// Low-band forward-quantiser decision-level table `Q6[32]`. The encoder
/// compares `(Q6[i] * det) >> 12` against the signed-magnitude prediction
/// error to find the 6-bit quantiser index. Only entries 1..=29 are used
/// for searching; entries 0, 30, 31 are sentinels.
pub const Q6: [i32; 32] = [
    0, 35, 72, 110, 150, 190, 233, 276, 323, 370, 422, 473, 530, 587, 650, 714, 786, 858, 940,
    1023, 1121, 1219, 1339, 1458, 1612, 1765, 1980, 2195, 2557, 2919, 0, 0,
];

/// Negative-sign index table for the low-band forward quantiser. Indexed by
/// the interval number from the `Q6` decision-level search (1..=29), with
/// sentinels at 0 and 31.
pub const ILN: [u8; 32] = [
    0, 63, 62, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11,
    10, 9, 8, 7, 6, 5, 4, 0,
];

/// Positive-sign index table for the low-band forward quantiser. Indexed
/// by the interval number from the `Q6` decision-level search.
pub const ILP: [u8; 32] = [
    0, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39,
    38, 37, 36, 35, 34, 33, 32, 0,
];

/// Low-band scale-factor log-step table `WL[8]`, shared across all three
/// rates. Indexed by `RL42[il >> 2]` where `il` is the 6-bit code.
pub const WL: [i32; 8] = [-60, -30, 58, 172, 334, 538, 1198, 3042];

/// Low-band re-mapping table: turns the 4-bit `il >> 2` value into a 3-bit
/// index into `WL`. `rl42[ril]` — used by the scale-factor adapter.
pub const RL42: [usize; 16] = [0, 7, 6, 5, 4, 3, 2, 1, 7, 6, 5, 4, 3, 2, 1, 0];

/// Step-size mantissa table used by `SCALEL` / `SCALEH`. `ilb[wd1]` is
/// looked up with `wd1 = (nb >> 6) & 31` and then shifted by `8 - (nb >>
/// 11)` (low band) or `10 - (nb >> 11)` (high band) to produce the new
/// `det`.
pub const ILB: [i32; 32] = [
    2048, 2093, 2139, 2186, 2233, 2282, 2332, 2383, 2435, 2489, 2543, 2599, 2656, 2714, 2774, 2834,
    2896, 2960, 3025, 3091, 3158, 3228, 3298, 3371, 3444, 3520, 3597, 3676, 3756, 3838, 3922, 4008,
];

/// High-band scale-factor log-step table `WH[3]`, indexed by `RH2[ih]`.
pub const WH: [i32; 3] = [0, -214, 798];

/// High-band 2-bit code → `WH` index mapping.
pub const RH2: [usize; 4] = [2, 1, 2, 1];

/// Negative-sign index for the high-band forward quantiser.
pub const IHN: [u8; 3] = [0, 1, 0];

/// Positive-sign index for the high-band forward quantiser.
pub const IHP: [u8; 3] = [0, 3, 2];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_lengths_match_bit_widths() {
        assert_eq!(QM6.len(), 64);
        assert_eq!(QM5.len(), 32);
        assert_eq!(QM4.len(), 16);
        assert_eq!(QM2.len(), 4);
    }

    #[test]
    fn tables_are_sign_balanced() {
        // Each QM table roughly splits into negatives (first half) and
        // positives (second half). Verify that indexes 0..N/2 are <= 0 for
        // the large-magnitude entries, which is a lightweight sanity check
        // that we transcribed the halves in the right order.
        assert!(QM6[4] < 0 && QM6[32] > 0);
        assert!(QM5[2] < 0 && QM5[16] > 0);
        assert!(QM4[1] < 0 && QM4[8] > 0);
        assert!(QM2[0] < 0 && QM2[2] > 0);
    }

    #[test]
    fn ancillary_table_lengths_match_spec() {
        assert_eq!(Q6.len(), 32);
        assert_eq!(ILN.len(), 32);
        assert_eq!(ILP.len(), 32);
        assert_eq!(ILB.len(), 32);
        assert_eq!(RL42.len(), 16);
        assert_eq!(RH2.len(), 4);
        assert_eq!(IHN.len(), 3);
        assert_eq!(IHP.len(), 3);
    }

    #[test]
    fn rl42_folds_to_3_bit_range() {
        for &r in RL42.iter() {
            assert!(r < 8);
        }
    }
}

//! ITU-T G.722 reference inverse-quantiser tables.
//!
//! These are the `qm6` / `qm5` / `qm4` / `qm2` arrays given verbatim in the
//! ITU-T G.722 (09/2012) reference code — the same values that appear in
//! every widely-used implementation (SpanDSP `libg722`, sippy/libg722,
//! NAudio's `G722Codec.cs`, etc.).
//!
//! They are defined here so that the crate ships the normative spec tables
//! alongside the current "equivalent-shape" pipeline in [`crate::band_low`].
//! The shipping low-band quantiser is a bit-width-parametric uniform
//! quantiser (the decoder is the exact inverse of the encoder for all three
//! rates, which is what our roundtrip tests verify) but is **not**
//! bit-compatible with the ITU reference. Porting the pipeline to index
//! these tables directly is tracked as a follow-up — the constants below
//! mean that work can start from the authoritative values without having
//! to pull the ITU tarball again.
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

/// Low-band scale-factor log-step table `WL[8]`, shared across all three
/// rates. Indexed by the top 3 bits of the low-band code magnitude.
pub const WL: [i32; 8] = [-60, -30, 58, 172, 334, 538, 1198, 3042];

/// High-band scale-factor log-step table `WH[3]`.
pub const WH: [i32; 3] = [0, -214, 798];

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
}

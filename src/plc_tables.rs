//! Appendix IV packet-loss-concealment numeric tables.
//!
//! The six fixed-point data tables the Appendix IV PLC algorithm adds
//! to the base G.722 decoder — the tables enumerated by
//! **Table IV.5/G.722** of the staged 2012 consolidated Recommendation
//! — transcribed from the staged CSV extractions under
//! `docs/audio/g722/tables/` (`appendix-IV-*.csv` + `.meta` sidecars,
//! provenance chain in `docs/audio/g722/tables/README.md`). Numeric
//! data tables staged under `docs/` are clean-room-safe to transcribe
//! (workspace ruling); no algorithmic source was read.
//!
//! Also carried here: the four eq (IV-19) higher-band post-filter
//! constants, which Table IV.5 does **not** list — they are printed in
//! full inside the Recommendation's own equation (`7303/8192`,
//! `3207/4096`) and are held in Q13 with the same feedback-sign
//! convention as [`A_HP`] (see `docs/audio/g722/tables/README.md`,
//! "One table that is not here").

/// Eq (IV-2) asymmetrical Hamming LPC analysis window, Q15
/// (Table IV.5 `G722PLC_lpc_win_80`, 80 entries; staged
/// `appendix-IV-lpc-window-Q15.csv`). Index k holds `w_lp(k − 80)`,
/// i.e. the window over `zl(−80 … −1)`; the scale is ×32767 (not
/// ×32768 — the staged README pins the peak `w = 1.0 → 32767` at
/// index 69, the eq (IV-2) breakpoint `n = −11`).
pub(crate) const LPC_WIN_80: [i32; 80] = [
    2621, 2637, 2684, 2762, 2871, 3010, 3180, 3380, 3610, 3869, 4157, 4473, 4816, 5185, 5581, 6002,
    6447, 6915, 7406, 7918, 8451, 9002, 9571, 10158, 10760, 11376, 12005, 12647, 13298, 13959,
    14628, 15302, 15982, 16666, 17351, 18037, 18723, 19406, 20086, 20761, 21429, 22090, 22742,
    23383, 24012, 24629, 25231, 25817, 26386, 26938, 27470, 27982, 28473, 28941, 29386, 29807,
    30203, 30573, 30916, 31231, 31519, 31778, 32008, 32208, 32378, 32518, 32627, 32705, 32751,
    32767, 32029, 29888, 26554, 22352, 17694, 13036, 8835, 5500, 3359, 2621,
];

/// Clause IV.6.1.2.1 lag window for 60-Hz bandwidth expansion, high
/// half (Table IV.5 `G722PLC_lag_h`; staged
/// `appendix-IV-lag-window-high.csv`). Entry k (k = 0..7) is the high
/// word of the Q31 window value for autocorrelation lag k + 1, in the
/// `(hi << 16) + (lo << 1)` double-precision encoding pinned by the
/// staged data. The 40-dB white-noise correction is folded into the
/// window (values = `exp(−½(2π·60·k/8000)²) / 1.0001`), so `r(0)`
/// itself is left untouched.
pub(crate) const LAG_H: [i32; 8] = [32728, 32619, 32438, 32187, 31867, 31480, 31029, 30517];

/// Low half of the lag window (Table IV.5 `G722PLC_lag_l`; staged
/// `appendix-IV-lag-window-low.csv`). See [`LAG_H`].
pub(crate) const LAG_L: [i32; 8] = [11904, 17280, 30720, 25856, 24192, 28992, 24384, 7360];

/// Eq (IV-5) quarter-band decimation FIR `H_dec(z)`, Q16
/// (Table IV.5 `G722PLC_fir_lp`; staged
/// `appendix-IV-decimation-fir-lowpass-Q16.csv`). Symmetric 8th-order
/// low-pass ahead of the 4:1 decimation to 2 kHz. Deliberately not
/// unity-DC-gain (Σ taps = 67973 > 65536) — the staged README pins
/// this; do not normalise.
pub(crate) const FIR_LP: [i32; 9] = [3692, 6190, 8525, 10186, 10787, 10186, 8525, 6190, 3692];

/// Eq (IV-4) lower-band pre-processing high-pass `H_pre(z)` numerator,
/// Q14 (Table IV.5 `G722PLC_b_hp`; staged
/// `appendix-IV-preproc-highpass-numerator-Q14.csv`): `(1 − z⁻¹)`,
/// exact zero at DC.
pub(crate) const B_HP: [i32; 2] = [16384, -16384];

/// Eq (IV-4) `H_pre(z)` denominator, Q14 (Table IV.5 `G722PLC_a_hp`;
/// staged `appendix-IV-preproc-highpass-denominator-Q14.csv`).
/// **Feedback-sign convention** (staged README ⚠): entry 1 is stored
/// positive as the *feedback* coefficient of the recurrence
/// `y(n) = x(n) − x(n−1) + (123/128)·y(n−1)` — i.e. the negated z⁻¹
/// coefficient of the eq (IV-4) denominator polynomial. `15744/16384 =
/// 123/128` exactly.
pub(crate) const A_HP: [i32; 2] = [16384, 15744];

/// Eq (IV-19) higher-band post-filter `H_post(z)` numerator, Q13:
/// `(7303/8192)(1 − z⁻¹)`. Not in Table IV.5 — the constants are
/// printed in the equation itself (derivation, not extraction).
pub(crate) const B_HP_POST: [i32; 2] = [7303, -7303];

/// Eq (IV-19) `H_post(z)` denominator, Q13, in the same feedback-sign
/// convention as [`A_HP`]: `6414 = 3207/4096 × 8192` exactly, stored
/// positive as the feedback coefficient of
/// `v(n) = (7303/8192)(u(n) − u(n−1)) + (3207/4096)·v(n−1)`.
pub(crate) const A_HP_POST: [i32; 2] = [8192, 6414];

#[cfg(test)]
mod tests {
    use super::*;

    /// Eq (IV-2): the Q15 window must reproduce the closed form
    /// `round(w_lp(n) × 32767)` entry for entry (staged README:
    /// max deviation 0 over all 80 entries).
    #[test]
    fn lpc_window_reproduces_eq_iv2_exactly() {
        for (k, &w) in LPC_WIN_80.iter().enumerate() {
            let n = k as f64 - 80.0; // n = −80 … −1
            let ideal = if n <= -11.0 {
                0.54 - 0.46 * ((n + 80.0) * core::f64::consts::PI / 69.0).cos()
            } else {
                0.54 + 0.46 * ((n + 11.0) * core::f64::consts::PI / 10.0).cos()
            };
            assert_eq!(w, (ideal * 32767.0).round() as i32, "window index {k}");
        }
        // Structural pins from the staged README: peak 32767 at index
        // 69 (the eq IV-2 breakpoint), endpoints 2621 = round(0.08 ×
        // 32767), monotone rising 0..=69 and falling 69..=79.
        assert_eq!(LPC_WIN_80[69], 32_767);
        assert_eq!(LPC_WIN_80[0], 2621);
        assert_eq!(LPC_WIN_80[79], 2621);
        assert!(LPC_WIN_80.windows(2).take(69).all(|w| w[0] < w[1]));
        assert!(LPC_WIN_80.windows(2).skip(69).all(|w| w[0] > w[1]));
    }

    /// The recombined Q31 lag window must match
    /// `exp(−½(2π·60·k/8000)²) / 1.0001` (60-Hz bandwidth expansion
    /// with the 40-dB white-noise correction folded in) to the staged
    /// README's 2.3e−8 bound, and be strictly decreasing in (0, 1).
    #[test]
    fn lag_window_matches_the_conditioning_closed_form() {
        let mut prev = f64::INFINITY;
        for k in 0..8 {
            let l = (i64::from(LAG_H[k]) << 16) + (i64::from(LAG_L[k]) << 1);
            let got = l as f64 / (1u64 << 31) as f64;
            let f = 2.0 * core::f64::consts::PI * 60.0 * (k as f64 + 1.0) / 8000.0;
            let want = (-0.5 * f * f).exp() / 1.0001;
            assert!(
                (got - want).abs() < 2.4e-8,
                "lag window k={k}: {got} vs {want}"
            );
            assert!(got > 0.0 && got < 1.0 && got < prev);
            prev = got;
        }
    }

    /// Eq (IV-5) prints every numerator term; the staged FIR must
    /// equal them term for term, be symmetric, and keep its
    /// non-normalised DC gain (Σ = 67973).
    #[test]
    fn decimation_fir_matches_eq_iv5() {
        assert_eq!(
            FIR_LP,
            [3692, 6190, 8525, 10186, 10787, 10186, 8525, 6190, 3692]
        );
        for j in 0..9 {
            assert_eq!(FIR_LP[j], FIR_LP[8 - j], "symmetry at {j}");
        }
        assert_eq!(FIR_LP.iter().sum::<i32>(), 67_973);
    }

    /// Eq (IV-4): exact DC zero in the numerator; the denominator pole
    /// is 123/128 exactly, stored in the feedback-sign convention.
    #[test]
    fn preproc_highpass_matches_eq_iv4() {
        assert_eq!(B_HP[0] + B_HP[1], 0);
        assert_eq!(A_HP[1] * 128, 123 * A_HP[0]);
    }

    /// Eq (IV-19): 7303/8192 and 3207/4096 carried exactly in Q13.
    #[test]
    fn post_filter_matches_eq_iv19() {
        assert_eq!(B_HP_POST[0] + B_HP_POST[1], 0);
        assert_eq!(B_HP_POST[0], 7303);
        assert_eq!(A_HP_POST[0], 8192);
        assert_eq!(A_HP_POST[1] * 4096, 3207 * A_HP_POST[0]);
    }
}

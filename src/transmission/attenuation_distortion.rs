//! Clause 2.4.2 / Figure 10/G.722 — codec end-to-end attenuation /
//! frequency-distortion mask.
//!
//! Clause 2.4.2 (p. 9) of the staged ITU-T G.722 (11/88) Recommendation
//! requires the **encoder + decoder loop** (the back-to-back / looped
//! configuration of Figure 9/G.722 p. 10) to satisfy the
//! attenuation/frequency-response limits drawn in Figure 10/G.722
//! (p. 11, "Attenuation distortion versus frequency"). The clause is
//! evaluated at test point B (Figure 2/G.722 p. 2) with a sine input
//! at test point A whose level is the nominal in-band test level of
//! −10 dBm0 referenced to the 1000 Hz nominal reference frequency.
//!
//! Unlike [`super::anti_aliasing_filter`] (clause 2.5.1 / Figure 11) and
//! [`super::reconstructing_filter`] (clause 2.5.2 / Figure 12), which
//! pin the **filter-only** masks of the transmit and receive audio
//! parts respectively, Figure 10/G.722 is the **end-to-end codec**
//! mask: it accounts for the SB-ADPCM coder *plus* both audio parts
//! together. The corridor it draws is therefore wider than the
//! filter-only corridors (Figures 11 / 12 leave ±0.5 dB of ripple
//! room per filter; Figure 10 leaves a −1 to +1 dB tight corridor and
//! a −1 to +3 dB relaxed corridor across the full passband).
//!
//! Figure 10/G.722 prints the mask as a pair of piecewise-constant
//! curves on a linear-on-log frequency axis with attenuation in dB.
//! The shape is read from the figure as follows:
//!
//! | Frequency band      | Lower bound (dB) | Upper bound (dB) |
//! | ------------------- | ---------------- | ---------------- |
//! | 0 Hz to 50 Hz       | (out-of-band)                                                                          | (out-of-band)   |
//! | 50 Hz to 100 Hz     | −1                                                                                     | +3 (relaxed)    |
//! | 100 Hz to 6.4 kHz   | −1                                                                                     | +1 (tight)      |
//! | 6.4 kHz to 7 kHz    | −1                                                                                     | +3 (relaxed)    |
//! | 7 kHz to 8 kHz      | −1 — only the lower bound is printed; the upper bound is open above the corridor       | (open)          |
//! | 8 kHz and above     | (out-of-band; right wall of the mask)                                                  | (out-of-band)   |
//!
//! The mask is symmetric in the same sense the filter masks are: a
//! measurement is *inside the mask* (the spec admits the codec) when
//! it sits inside the corridor printed for that band. The spec follows
//! the attenuation-positive sign convention, so a measurement of
//! `+0.5` dB attenuation lies between the `−1` lower bound and the
//! `+1` upper bound and therefore meets the mask in the 100 Hz –
//! 6.4 kHz tight in-band region.
//!
//! ## Difference from Figures 11 and 12
//!
//! Figures 11 and 12 are *filter-only* masks (each evaluated at the
//! interface between their respective filter and the SB-ADPCM
//! interface; clause 2.5.1 / 2.5.2). Figure 10 is the *codec
//! end-to-end* mask (encoder + decoder + both audio parts), the
//! quantity an integrator measures between test points A and B of
//! Figure 2/G.722 in the looped configuration.
//!
//! In particular Figure 10 has no stopband shoulder — the mask's right
//! wall sits at 8 kHz, the Nyquist edge of the codec's 16 kHz sample
//! clock. Above 8 kHz the codec cannot produce signal at all, so the
//! mask simply doesn't extend. The corridor inside the passband is
//! wider than the per-filter corridors because both filters'
//! tolerances and the SB-ADPCM quantizer's frequency response stack
//! together at test point B.
//!
//! Figure 10's in-band ripple bounds (`−1` / `+1` dB tight, `+3` dB
//! relaxed) are exactly twice the filter masks' (`−0.5` / `+0.5` dB
//! tight, `+1.5` dB relaxed) — i.e. each filter's printed corridor
//! contributes ½ of the codec's printed corridor, which matches the
//! geometric intuition (anti-aliasing + reconstructing contributions
//! add in dB and the SB-ADPCM coder's contribution sits inside the
//! resulting envelope).
//!
//! ## Provenance
//!
//! Every breakpoint and dB value below is transcribed from the
//! printed Figure 10/G.722 of
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf` (page 11). No external
//! reference implementation of the codec mask was consulted.

use crate::transmission::{
    NOMINAL_PASSBAND_HIGH_HZ, NOMINAL_PASSBAND_LOW_HZ, SUBBAND_SAMPLE_CLOCK_HZ,
};

// -----------------------------------------------------------------------
// Mask breakpoints (Figure 10/G.722 page 11)
// -----------------------------------------------------------------------

/// In-band passband low edge of Figure 10/G.722 (p. 11). The mask's
/// tight `+1` dB upper bound begins at 100 Hz; the 50–100 Hz strip is
/// the relaxed transition where the upper bound rises to `+3` dB.
pub const PASSBAND_LOW_HZ: u32 = 100;

/// In-band passband high edge for the tight `+1` dB upper-bound region
/// of Figure 10/G.722 (p. 11). Above this and up to
/// [`PASSBAND_RELAXED_HIGH_HZ`] the upper bound relaxes to `+3` dB.
pub const PASSBAND_TIGHT_HIGH_HZ: u32 = 6_400;

/// Upper edge of the relaxed in-band corridor of Figure 10/G.722
/// (p. 11). Beyond this the upper bound is open — only the `−1` dB
/// lower bound continues out to the [`MASK_HIGH_EDGE_HZ`] right wall.
/// This matches the [`NOMINAL_PASSBAND_HIGH_HZ`] of clause 2.4.1.
pub const PASSBAND_RELAXED_HIGH_HZ: u32 = NOMINAL_PASSBAND_HIGH_HZ;

/// Right wall of the Figure 10/G.722 mask (p. 11). The mask's
/// frequency axis ends at 8 kHz — the Nyquist edge of the codec's
/// 16 kHz sample clock (= [`SUBBAND_SAMPLE_CLOCK_HZ`]). Above this
/// the codec cannot synthesise signal and no mask is printed.
pub const MASK_HIGH_EDGE_HZ: u32 = SUBBAND_SAMPLE_CLOCK_HZ;

/// Lower bound of the in-band corridor (Figure 10/G.722 p. 11). The
/// printed value is `−1` dB; sign convention is attenuation-positive,
/// so this means up to 1 dB of *gain* is allowed end-to-end. The
/// bound extends from 50 Hz all the way to 8 kHz (i.e. across the
/// full mask, including the wide-roof transition strips).
pub const IN_BAND_LOWER_BOUND_DB: f64 = -1.0;

/// Tight upper bound of the in-band corridor on 100 Hz – 6.4 kHz
/// (Figure 10/G.722 p. 11). The printed value is `+1` dB.
pub const IN_BAND_TIGHT_UPPER_BOUND_DB: f64 = 1.0;

/// Relaxed upper bound of the in-band corridor on the two transition
/// strips 50 – 100 Hz and 6.4 – 7 kHz (Figure 10/G.722 p. 11). The
/// printed value is `+3` dB.
pub const IN_BAND_RELAXED_UPPER_BOUND_DB: f64 = 3.0;

// -----------------------------------------------------------------------
// Mask evaluation
// -----------------------------------------------------------------------

/// Outcome of evaluating a single (frequency, attenuation) measurement
/// against the Figure 10/G.722 codec end-to-end mask.
///
/// The variants follow the printed piecewise-constant structure of
/// Figure 10. Unlike the filter masks of [`super::anti_aliasing_filter`]
/// and [`super::reconstructing_filter`], Figure 10 has no stopband
/// floor — the mask ends at the 8 kHz right wall (the Nyquist edge of
/// the codec's 16 kHz sample clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskBand {
    /// Frequency is below the 50 Hz mask anchor — outside the codec's
    /// normative coverage. The spec assigns no constraint here.
    BelowMask,
    /// Frequency sits in the 50 Hz – 100 Hz low-transition strip.
    /// Lower bound `−1` dB; upper bound `+3` dB (relaxed corridor).
    LowTransition,
    /// Frequency is in the tight in-band ripple region
    /// [`PASSBAND_LOW_HZ`] – [`PASSBAND_TIGHT_HIGH_HZ`]
    /// (lower bound `−1` dB, tight upper bound `+1` dB).
    InBandTight,
    /// Frequency is in the upper relaxed in-band region
    /// [`PASSBAND_TIGHT_HIGH_HZ`] – [`PASSBAND_RELAXED_HIGH_HZ`]
    /// (lower bound `−1` dB, relaxed upper bound `+3` dB).
    InBandRelaxed,
    /// Frequency sits in the [`PASSBAND_RELAXED_HIGH_HZ`] –
    /// [`MASK_HIGH_EDGE_HZ`] high-transition strip. Only the lower
    /// bound `−1` dB is printed; the upper bound is open (the codec
    /// can roll off as steeply as the implementation chooses).
    HighTransition,
    /// Frequency sits above the [`MASK_HIGH_EDGE_HZ`] right wall —
    /// outside the codec's normative coverage. The codec's 16 kHz
    /// sample clock cannot represent signal here.
    AboveMask,
}

/// Evaluate a measured `(frequency_hz, attenuation_db)` pair against
/// the Figure 10/G.722 mask.
///
/// Returns the [`MaskBand`] the frequency falls into and a `bool` that
/// is `true` when the measured attenuation lies inside the printed
/// mask for that band. The mapping for each band:
///
/// | Band              | Constraint checked                                                |
/// | ----------------- | ----------------------------------------------------------------- |
/// | `BelowMask`       | always `true` (outside normative coverage)                        |
/// | `LowTransition`   | `−1 dB ≤ atten ≤ +3 dB`                                           |
/// | `InBandTight`     | `−1 dB ≤ atten ≤ +1 dB`                                           |
/// | `InBandRelaxed`   | `−1 dB ≤ atten ≤ +3 dB`                                           |
/// | `HighTransition`  | `−1 dB ≤ atten` (lower bound only — upper bound is open)          |
/// | `AboveMask`       | always `true` (outside normative coverage)                        |
pub fn evaluate(frequency_hz: f64, attenuation_db: f64) -> (MaskBand, bool) {
    let band = classify(frequency_hz);
    let ok = match band {
        MaskBand::BelowMask => true,
        MaskBand::LowTransition => {
            (IN_BAND_LOWER_BOUND_DB..=IN_BAND_RELAXED_UPPER_BOUND_DB).contains(&attenuation_db)
        }
        MaskBand::InBandTight => {
            (IN_BAND_LOWER_BOUND_DB..=IN_BAND_TIGHT_UPPER_BOUND_DB).contains(&attenuation_db)
        }
        MaskBand::InBandRelaxed => {
            (IN_BAND_LOWER_BOUND_DB..=IN_BAND_RELAXED_UPPER_BOUND_DB).contains(&attenuation_db)
        }
        MaskBand::HighTransition => attenuation_db >= IN_BAND_LOWER_BOUND_DB,
        MaskBand::AboveMask => true,
    };
    (band, ok)
}

/// Classify a frequency into the [`MaskBand`] it belongs to. Frequencies
/// are sampled from Figure 10/G.722's printed log-axis breakpoints
/// (p. 11).
pub fn classify(frequency_hz: f64) -> MaskBand {
    // Use the spec-printed passband-low anchor of 50 Hz (the figure
    // shows 0.050 kHz as the first mask labelled point). Below that
    // we fall outside the mask's normative coverage.
    let f_low_anchor = NOMINAL_PASSBAND_LOW_HZ as f64;
    if !frequency_hz.is_finite() || frequency_hz < f_low_anchor {
        return MaskBand::BelowMask;
    }
    if frequency_hz < PASSBAND_LOW_HZ as f64 {
        return MaskBand::LowTransition;
    }
    if frequency_hz <= PASSBAND_TIGHT_HIGH_HZ as f64 {
        return MaskBand::InBandTight;
    }
    if frequency_hz <= PASSBAND_RELAXED_HIGH_HZ as f64 {
        return MaskBand::InBandRelaxed;
    }
    if frequency_hz < MASK_HIGH_EDGE_HZ as f64 {
        return MaskBand::HighTransition;
    }
    MaskBand::AboveMask
}

/// Lower-bound attenuation (in dB) the codec end-to-end must satisfy
/// at `frequency_hz`. The bound is `IN_BAND_LOWER_BOUND_DB` (`−1` dB)
/// across the printed mask (50 Hz – 8 kHz) and `f64::NEG_INFINITY`
/// outside it (no constraint).
pub fn lower_bound_db(frequency_hz: f64) -> f64 {
    match classify(frequency_hz) {
        MaskBand::BelowMask | MaskBand::AboveMask => f64::NEG_INFINITY,
        _ => IN_BAND_LOWER_BOUND_DB,
    }
}

/// Upper-bound attenuation (in dB) the codec end-to-end must satisfy
/// at `frequency_hz`. Returns `f64::INFINITY` where the spec doesn't
/// print an upper bound (the `BelowMask`, `HighTransition` and
/// `AboveMask` bands).
pub fn upper_bound_db(frequency_hz: f64) -> f64 {
    match classify(frequency_hz) {
        MaskBand::BelowMask => f64::INFINITY,
        MaskBand::LowTransition => IN_BAND_RELAXED_UPPER_BOUND_DB,
        MaskBand::InBandTight => IN_BAND_TIGHT_UPPER_BOUND_DB,
        MaskBand::InBandRelaxed => IN_BAND_RELAXED_UPPER_BOUND_DB,
        MaskBand::HighTransition => f64::INFINITY,
        MaskBand::AboveMask => f64::INFINITY,
    }
}

// -----------------------------------------------------------------------
// Compile-time invariants
// -----------------------------------------------------------------------

const _: () = {
    // Breakpoints must be strictly increasing across the printed
    // Figure 10/G.722 axis.
    assert!(NOMINAL_PASSBAND_LOW_HZ < PASSBAND_LOW_HZ);
    assert!(PASSBAND_LOW_HZ < PASSBAND_TIGHT_HIGH_HZ);
    assert!(PASSBAND_TIGHT_HIGH_HZ < PASSBAND_RELAXED_HIGH_HZ);
    assert!(PASSBAND_RELAXED_HIGH_HZ < MASK_HIGH_EDGE_HZ);
    // The right wall sits at the QMF Nyquist (8 kHz =
    // SUBBAND_SAMPLE_CLOCK_HZ), the same edge as the input
    // anti-aliasing filter's stopband entry of Figure 11. The codec
    // can't synthesise above this.
    assert!(MASK_HIGH_EDGE_HZ == SUBBAND_SAMPLE_CLOCK_HZ);
    // The relaxed-corridor high edge matches the nominal 3-dB passband
    // of clause 2.4.1.
    assert!(PASSBAND_RELAXED_HIGH_HZ == NOMINAL_PASSBAND_HIGH_HZ);
};

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transmission::{anti_aliasing_filter, reconstructing_filter};

    #[test]
    fn breakpoints_match_figure_10() {
        // The printed Figure 10/G.722 (p. 11) shows the labelled
        // frequencies 0.050 / 0.100 / 1 / 6.4 / 7 / 8 kHz on the log
        // frequency axis. The mask's anchor points must match.
        assert_eq!(NOMINAL_PASSBAND_LOW_HZ, 50);
        assert_eq!(PASSBAND_LOW_HZ, 100);
        assert_eq!(PASSBAND_TIGHT_HIGH_HZ, 6_400);
        assert_eq!(PASSBAND_RELAXED_HIGH_HZ, 7_000);
        assert_eq!(MASK_HIGH_EDGE_HZ, 8_000);
    }

    #[test]
    fn ripple_bounds_match_figure_10() {
        // Figure 10/G.722 (p. 11) prints −1 / +1 / +3 dB on the
        // attenuation axis for the corridor rectangles.
        assert_eq!(IN_BAND_LOWER_BOUND_DB, -1.0);
        assert_eq!(IN_BAND_TIGHT_UPPER_BOUND_DB, 1.0);
        assert_eq!(IN_BAND_RELAXED_UPPER_BOUND_DB, 3.0);
    }

    #[test]
    fn classify_below_low_anchor_is_below_mask() {
        assert_eq!(classify(0.0), MaskBand::BelowMask);
        assert_eq!(classify(10.0), MaskBand::BelowMask);
        assert_eq!(classify(49.999), MaskBand::BelowMask);
    }

    #[test]
    fn classify_low_transition_region() {
        // Between 50 Hz and 100 Hz the corridor uses the wide ±3 dB
        // relaxed upper bound (the small "shelf" rectangle that the
        // figure shows above the in-band rectangle on its left side).
        assert_eq!(classify(50.0), MaskBand::LowTransition);
        assert_eq!(classify(75.0), MaskBand::LowTransition);
        assert_eq!(classify(99.999), MaskBand::LowTransition);
    }

    #[test]
    fn classify_tight_in_band() {
        // 100 Hz to 6.4 kHz inclusive is the tight [−1, +1] dB
        // corridor. Anchor at the printed 1000 Hz nominal reference
        // frequency.
        assert_eq!(classify(100.0), MaskBand::InBandTight);
        assert_eq!(classify(1_000.0), MaskBand::InBandTight);
        assert_eq!(classify(3_500.0), MaskBand::InBandTight);
        assert_eq!(classify(6_400.0), MaskBand::InBandTight);
    }

    #[test]
    fn classify_relaxed_in_band() {
        // 6.4 kHz < f ≤ 7 kHz: relaxed upper bound of +3 dB applies
        // (matching the right-hand transition "shelf" of the figure).
        assert_eq!(classify(6_500.0), MaskBand::InBandRelaxed);
        assert_eq!(classify(6_800.0), MaskBand::InBandRelaxed);
        assert_eq!(classify(7_000.0), MaskBand::InBandRelaxed);
    }

    #[test]
    fn classify_high_transition() {
        // 7 kHz < f < 8 kHz: only the lower bound is printed; the
        // upper bound is open (the codec is free to roll off here).
        assert_eq!(classify(7_500.0), MaskBand::HighTransition);
        assert_eq!(classify(7_999.999), MaskBand::HighTransition);
    }

    #[test]
    fn classify_above_mask_above_right_wall() {
        // 8 kHz and above sits past the printed right wall — the codec
        // cannot synthesise signal here (Nyquist of 16 kHz sample
        // rate).
        assert_eq!(classify(8_000.0), MaskBand::AboveMask);
        assert_eq!(classify(10_000.0), MaskBand::AboveMask);
        assert_eq!(classify(50_000.0), MaskBand::AboveMask);
    }

    #[test]
    fn evaluate_tight_in_band_passes_zero_db() {
        // 0 dB attenuation at 1 kHz must pass the [−1, +1] dB
        // corridor.
        let (band, ok) = evaluate(1_000.0, 0.0);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(ok);
    }

    #[test]
    fn evaluate_tight_in_band_rejects_outside_corridor() {
        // +1.1 dB attenuation breaks the +1 dB upper bound of the
        // tight in-band region.
        let (band, ok) = evaluate(1_000.0, 1.1);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(!ok);
        // −1.1 dB (= 1.1 dB of gain) breaks the −1 dB lower bound.
        let (band, ok) = evaluate(1_000.0, -1.1);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(!ok);
        // Exactly at the printed bounds the measurement passes (the
        // corridor is a closed interval).
        let (_, ok) = evaluate(1_000.0, 1.0);
        assert!(ok);
        let (_, ok) = evaluate(1_000.0, -1.0);
        assert!(ok);
    }

    #[test]
    fn evaluate_relaxed_in_band_admits_two_db() {
        // 2.0 dB attenuation at 6.8 kHz is inside the relaxed [−1, +3]
        // dB region and outside the tight [−1, +1] dB one.
        let (band, ok) = evaluate(6_800.0, 2.0);
        assert_eq!(band, MaskBand::InBandRelaxed);
        assert!(ok);
        // 3.1 dB violates the +3 dB upper bound.
        let (band, ok) = evaluate(6_800.0, 3.1);
        assert_eq!(band, MaskBand::InBandRelaxed);
        assert!(!ok);
        // The exact corridor edges pass.
        let (_, ok) = evaluate(6_800.0, 3.0);
        assert!(ok);
        let (_, ok) = evaluate(6_800.0, -1.0);
        assert!(ok);
    }

    #[test]
    fn evaluate_low_transition_uses_relaxed_corridor() {
        // 75 Hz, +2 dB: inside the wide [−1, +3] dB low-transition
        // corridor; passes.
        let (band, ok) = evaluate(75.0, 2.0);
        assert_eq!(band, MaskBand::LowTransition);
        assert!(ok);
        // 75 Hz, +3.1 dB: violates the +3 dB upper bound.
        let (_, ok) = evaluate(75.0, 3.1);
        assert!(!ok);
        // 75 Hz, +1.5 dB: would exceed the in-band tight corridor's
        // +1 dB bound, but the low-transition strip uses the relaxed
        // bound so this passes.
        let (_, ok) = evaluate(75.0, 1.5);
        assert!(ok);
    }

    #[test]
    fn evaluate_high_transition_lower_bound_only() {
        // 7.5 kHz, +20 dB attenuation: meets the lower bound (which is
        // all the spec prints for this strip).
        let (band, ok) = evaluate(7_500.0, 20.0);
        assert_eq!(band, MaskBand::HighTransition);
        assert!(ok);
        // 7.5 kHz, −1.1 dB (= 1.1 dB of gain): violates the −1 dB
        // lower bound.
        let (band, ok) = evaluate(7_500.0, -1.1);
        assert_eq!(band, MaskBand::HighTransition);
        assert!(!ok);
        // 7.5 kHz, −1 dB: exactly meets the lower bound.
        let (_, ok) = evaluate(7_500.0, -1.0);
        assert!(ok);
    }

    #[test]
    fn evaluate_below_mask_always_passes() {
        // Outside the mask's normative coverage the result is "no
        // constraint applies"; the caller has to evaluate that band
        // separately.
        let (band, ok) = evaluate(10.0, -100.0);
        assert_eq!(band, MaskBand::BelowMask);
        assert!(ok);
    }

    #[test]
    fn evaluate_above_mask_always_passes() {
        // Past the 8 kHz right wall the codec can't synthesise signal;
        // no constraint applies.
        let (band, ok) = evaluate(10_000.0, -100.0);
        assert_eq!(band, MaskBand::AboveMask);
        assert!(ok);
    }

    #[test]
    fn passband_in_band_relaxed_meets_tight_at_anchor() {
        // The 6.4 kHz breakpoint is the high edge of the tight region.
        // A measurement exactly at 6.4 kHz must classify as InBandTight
        // (i.e. the tight ±1 dB corridor still applies at the
        // closed-interval endpoint).
        let (band, _) = evaluate(6_400.0, 0.0);
        assert_eq!(band, MaskBand::InBandTight);
        // Just past 6.4 kHz, the relaxed corridor applies.
        let (band, _) = evaluate(6_400.001, 0.0);
        assert_eq!(band, MaskBand::InBandRelaxed);
    }

    #[test]
    fn nan_and_negative_handled_safely() {
        assert_eq!(classify(f64::NAN), MaskBand::BelowMask);
        assert_eq!(classify(-100.0), MaskBand::BelowMask);
        assert_eq!(lower_bound_db(f64::NAN), f64::NEG_INFINITY);
        assert_eq!(upper_bound_db(f64::NAN), f64::INFINITY);
    }

    #[test]
    fn lower_bound_db_is_minus_one_across_passband() {
        // Sample the passband on a 100 Hz step grid; the lower bound
        // must be the printed −1 dB everywhere.
        let mut f = 50.0_f64;
        while f < 8_000.0 {
            let lb = lower_bound_db(f);
            assert!(
                (lb - IN_BAND_LOWER_BOUND_DB).abs() < 1e-12,
                "lower bound at {f} Hz = {lb}, expected {IN_BAND_LOWER_BOUND_DB}"
            );
            f += 100.0;
        }
    }

    #[test]
    fn lower_bound_db_neg_infinity_outside_mask() {
        assert_eq!(lower_bound_db(10.0), f64::NEG_INFINITY);
        assert_eq!(lower_bound_db(49.999), f64::NEG_INFINITY);
        assert_eq!(lower_bound_db(8_000.0), f64::NEG_INFINITY);
        assert_eq!(lower_bound_db(10_000.0), f64::NEG_INFINITY);
    }

    #[test]
    fn upper_bound_db_step_shape() {
        // Verify the printed step shape: +3 dB on the two transition
        // strips, +1 dB on the tight in-band, open elsewhere.
        assert_eq!(upper_bound_db(75.0), 3.0);
        assert_eq!(upper_bound_db(1_000.0), 1.0);
        assert_eq!(upper_bound_db(6_400.0), 1.0);
        assert!((upper_bound_db(6_500.0) - 3.0).abs() < 1e-12);
        assert!((upper_bound_db(7_000.0) - 3.0).abs() < 1e-12);
        assert_eq!(upper_bound_db(7_500.0), f64::INFINITY);
        assert_eq!(upper_bound_db(10.0), f64::INFINITY);
        assert_eq!(upper_bound_db(10_000.0), f64::INFINITY);
    }

    #[test]
    fn upper_bound_step_at_100hz_breakpoint() {
        // At exactly 100 Hz the tight corridor takes over (closed
        // interval at the left edge); just below 100 Hz the relaxed
        // corridor still applies.
        assert!((upper_bound_db(100.0) - 1.0).abs() < 1e-12);
        assert!((upper_bound_db(99.999) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn corridor_is_twice_filter_corridor() {
        // Figure 10 is the codec end-to-end mask; Figures 11 and 12
        // are the individual filter masks. The codec corridor is
        // exactly twice the filter corridor's printed bounds (each
        // filter contributes ½ of the codec budget):
        // - Tight upper: filter ±0.5 dB → codec ±1 dB.
        // - Relaxed upper: filter +1.5 dB → codec +3 dB.
        // - Lower: filter −0.5 dB → codec −1 dB.
        assert!(
            (IN_BAND_TIGHT_UPPER_BOUND_DB
                - 2.0 * anti_aliasing_filter::IN_BAND_TIGHT_UPPER_BOUND_DB)
                .abs()
                < 1e-12
        );
        assert!(
            (IN_BAND_RELAXED_UPPER_BOUND_DB
                - 2.0 * anti_aliasing_filter::IN_BAND_RELAXED_UPPER_BOUND_DB)
                .abs()
                < 1e-12
        );
        assert!(
            (IN_BAND_LOWER_BOUND_DB - 2.0 * anti_aliasing_filter::IN_BAND_LOWER_BOUND_DB).abs()
                < 1e-12
        );
        // And the reconstructing filter shares the same per-filter
        // corridor — same expectation against Figure 12's anchors.
        assert!(
            (IN_BAND_TIGHT_UPPER_BOUND_DB
                - 2.0 * reconstructing_filter::IN_BAND_TIGHT_UPPER_BOUND_DB)
                .abs()
                < 1e-12
        );
        assert!(
            (IN_BAND_RELAXED_UPPER_BOUND_DB
                - 2.0 * reconstructing_filter::IN_BAND_RELAXED_UPPER_BOUND_DB)
                .abs()
                < 1e-12
        );
        assert!(
            (IN_BAND_LOWER_BOUND_DB - 2.0 * reconstructing_filter::IN_BAND_LOWER_BOUND_DB).abs()
                < 1e-12
        );
    }

    #[test]
    fn breakpoints_match_filter_breakpoints() {
        // Figures 10 / 11 / 12 share their in-band breakpoint set
        // exactly (100 Hz / 6.4 kHz / 7 kHz). Pin that explicitly so
        // a future edit to any of the three masks flags the drift.
        assert_eq!(PASSBAND_LOW_HZ, anti_aliasing_filter::PASSBAND_LOW_HZ);
        assert_eq!(
            PASSBAND_TIGHT_HIGH_HZ,
            anti_aliasing_filter::PASSBAND_TIGHT_HIGH_HZ
        );
        assert_eq!(
            PASSBAND_RELAXED_HIGH_HZ,
            anti_aliasing_filter::PASSBAND_RELAXED_HIGH_HZ
        );
        assert_eq!(PASSBAND_LOW_HZ, reconstructing_filter::PASSBAND_LOW_HZ);
        assert_eq!(
            PASSBAND_TIGHT_HIGH_HZ,
            reconstructing_filter::PASSBAND_TIGHT_HIGH_HZ
        );
        assert_eq!(
            PASSBAND_RELAXED_HIGH_HZ,
            reconstructing_filter::PASSBAND_RELAXED_HIGH_HZ
        );
    }

    #[test]
    fn right_wall_matches_anti_aliasing_stopband_entry() {
        // The 8 kHz right wall of Figure 10 sits at the same place as
        // the stopband entry of Figure 11's input anti-aliasing
        // mask — the Nyquist of the codec's 16 kHz sample clock.
        assert_eq!(MASK_HIGH_EDGE_HZ, anti_aliasing_filter::STOPBAND_ENTRY_HZ);
    }

    #[test]
    fn evaluate_matches_classify_for_passing_measurement_at_each_anchor() {
        // Sanity sweep: for each printed anchor, a measurement that
        // sits exactly on the printed bound must classify as the
        // expected band AND register `ok = true`.
        let pairs: [(f64, f64, MaskBand); 8] = [
            (75.0, 3.0, MaskBand::LowTransition),
            (75.0, -1.0, MaskBand::LowTransition),
            (1_000.0, 0.0, MaskBand::InBandTight),
            (6_400.0, 1.0, MaskBand::InBandTight),
            (6_400.0, -1.0, MaskBand::InBandTight),
            (7_000.0, 3.0, MaskBand::InBandRelaxed),
            (7_000.0, -1.0, MaskBand::InBandRelaxed),
            (7_500.0, -1.0, MaskBand::HighTransition),
        ];
        for (f, a, expected) in pairs {
            let (band, ok) = evaluate(f, a);
            assert_eq!(band, expected, "band mismatch at ({f}, {a})");
            assert!(ok, "anchor measurement ({f}, {a}) failed mask");
        }
    }

    #[test]
    fn evaluate_anchor_misses() {
        // Each anchor +1 dB outside the corridor must fail.
        let misses: [(f64, f64); 5] = [
            (75.0, 3.1),
            (1_000.0, 1.01),
            (6_400.0, 1.01),
            (7_000.0, 3.01),
            (7_500.0, -1.01),
        ];
        for (f, a) in misses {
            let (_, ok) = evaluate(f, a);
            assert!(!ok, "miss at ({f}, {a}) was reported as passing");
        }
    }

    #[test]
    fn classify_at_breakpoints_takes_lower_band() {
        // Closed-interval semantics: the high endpoint of each band
        // belongs to that band (the next band picks up just past the
        // breakpoint). Pin this explicitly.
        assert_eq!(
            classify(PASSBAND_TIGHT_HIGH_HZ as f64),
            MaskBand::InBandTight
        );
        assert_eq!(
            classify(PASSBAND_RELAXED_HIGH_HZ as f64),
            MaskBand::InBandRelaxed
        );
        // The right wall (8 kHz) sits in `AboveMask` per the half-open
        // semantics of "< MASK_HIGH_EDGE_HZ" — the codec cannot
        // produce signal *at* the Nyquist either.
        assert_eq!(classify(MASK_HIGH_EDGE_HZ as f64), MaskBand::AboveMask);
    }

    #[test]
    fn nominal_reference_frequency_passes_zero_db() {
        // The 1000 Hz nominal reference frequency of clause 2.3 (the
        // anchor at which the spec quotes "test level −10 dBm0" for
        // clause 2.4.2) must classify InBandTight and accept a 0 dB
        // measurement (the canonical "passes the mask" example).
        let (band, ok) = evaluate(1_000.0, 0.0);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(ok);
    }
}

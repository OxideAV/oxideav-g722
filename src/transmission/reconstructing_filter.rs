//! Clause 2.5.2 / Figure 12/G.722 — output reconstructing filter mask.
//!
//! Clause 2.5.2 (p. 11) of the staged ITU-T G.722 (11/88) Recommendation
//! requires the receive audio part's output reconstructing filter to
//! satisfy the attenuation/frequency-response limits drawn in
//! Figure 12/G.722 (p. 12, "Attenuation/frequency response of the output
//! reconstructing filter (including x/sin x correction)"). The clause is
//! evaluated at test point B of Figure 2/G.722 (p. 2) with the nominal
//! reference frequency at 1000 Hz and the in-band test level at
//! −10 dBm0.
//!
//! Figure 12/G.722 prints the mask as a pair of piecewise-constant
//! curves on a logarithmic frequency axis with attenuation in dB. Both
//! curves are anchored at the same frequencies; only the dB values
//! differ. The shape is read from the figure as follows:
//!
//! | Frequency band | Lower bound (dB) | Upper bound (dB) |
//! | -------------- | ---------------- | ---------------- |
//! | 0 Hz to 50 Hz       | stopband — no upper bound on attenuation; lower bound undefined (out-of-band)       | (out-of-band)   |
//! | 50 Hz to 100 Hz     | transition (out-of-band → in-band) — no normative constraint inside the slope range | (transition)    |
//! | 100 Hz to 6.4 kHz   | −0.5                                                                                | +0.5            |
//! | 6.4 kHz to 7 kHz    | −0.5                                                                                | +1.5            |
//! | 7 kHz to 8 kHz      | transition (in-band → stopband) — only the lower bound stays at −0.5                | (transition)    |
//! | 8 kHz               | ≥ +25 (stopband entry — attenuation has *floor* of 25 dB)                           | (no upper)      |
//! | 9 kHz               | ≥ +50                                                                               | (no upper)      |
//! | 14 kHz and above    | ≥ +70                                                                               | (no upper)      |
//!
//! In Figure 12 the *upper* bound is the lower line on the dB axis (more
//! gain) and the *lower* bound is the upper line (more attenuation).
//! This module follows the spec's "attenuation is positive" sign
//! convention: a measurement of `+0.3` dB attenuation lies between the
//! `−0.5` lower bound and the `+0.5` upper bound and therefore meets
//! the mask in the 100 Hz – 6.4 kHz in-band region.
//!
//! The mask is a *normative* response specification on the analogue
//! receive audio part — not a digital filter the SB-ADPCM coder itself
//! implements. The data surfaced here is the constraint a host system's
//! reconstructing filter must satisfy *downstream* of the SB-ADPCM
//! decoder. The companion [`evaluate`] helper lets a caller that has
//! measured `(frequency, attenuation_dB)` at test point B verify the
//! result against the mask; clause-2.4.4 idle-noise checks that previously
//! could only sit under the wideband −60 dBm0 bound can be tightened to
//! the narrow-band −66 dBm0 bound once the host's reconstructing filter
//! is verified against this mask.
//!
//! ## Provenance
//!
//! Every breakpoint and dB value below is transcribed from the printed
//! Figure 12/G.722 of `docs/audio/g722/T-REC-G.722-198811-S.pdf` (page
//! 12). No external reference implementation of the filter mask was
//! consulted.

use crate::transmission::{NOMINAL_PASSBAND_LOW_HZ, SUBBAND_SAMPLE_CLOCK_HZ};

// -----------------------------------------------------------------------
// Mask breakpoints (Figure 12/G.722 page 12)
// -----------------------------------------------------------------------

/// In-band passband low edge of Figure 12/G.722 (p. 12). The mask's
/// passband ripple constraints begin at 100 Hz; the 50–100 Hz strip is
/// a transition region between the out-of-band stopband and the
/// in-band ripple.
pub const PASSBAND_LOW_HZ: u32 = 100;

/// In-band passband high edge for the tight ±0.5 dB ripple region of
/// Figure 12/G.722 (p. 12). Above this the upper bound relaxes to
/// +1.5 dB up to [`PASSBAND_RELAXED_HIGH_HZ`].
pub const PASSBAND_TIGHT_HIGH_HZ: u32 = 6_400;

/// Upper edge of the relaxed in-band ripple region (Figure 12/G.722
/// p. 12). Beyond this the mask transitions into the stopband.
pub const PASSBAND_RELAXED_HIGH_HZ: u32 = 7_000;

/// Out-of-band stopband entry (Figure 12/G.722 p. 12). At 8 kHz the
/// reconstructing filter must already provide ≥ 25 dB of attenuation.
pub const STOPBAND_ENTRY_HZ: u32 = 8_000;

/// First stopband shoulder (Figure 12/G.722 p. 12). At 9 kHz the
/// mask requires ≥ 50 dB of attenuation.
pub const STOPBAND_SHOULDER_HZ: u32 = 9_000;

/// Far-band stopband floor (Figure 12/G.722 p. 12). At 14 kHz and
/// above the mask requires ≥ 70 dB of attenuation.
pub const STOPBAND_FAR_HZ: u32 = 14_000;

/// Lower bound of the tight in-band ripple band (100 Hz – 6.4 kHz) in dB
/// (Figure 12/G.722 p. 12). Sign convention: attenuation positive, so
/// −0.5 dB means up to 0.5 dB of *gain* is allowed.
pub const IN_BAND_LOWER_BOUND_DB: f64 = -0.5;

/// Upper bound of the tight in-band ripple band (100 Hz – 6.4 kHz) in dB
/// (Figure 12/G.722 p. 12).
pub const IN_BAND_TIGHT_UPPER_BOUND_DB: f64 = 0.5;

/// Upper bound of the relaxed in-band band (6.4 kHz – 7 kHz) in dB
/// (Figure 12/G.722 p. 12).
pub const IN_BAND_RELAXED_UPPER_BOUND_DB: f64 = 1.5;

/// Minimum attenuation at [`STOPBAND_ENTRY_HZ`] (Figure 12/G.722 p. 12).
pub const STOPBAND_ENTRY_MIN_ATTEN_DB: f64 = 25.0;

/// Minimum attenuation at [`STOPBAND_SHOULDER_HZ`] (Figure 12/G.722
/// p. 12).
pub const STOPBAND_SHOULDER_MIN_ATTEN_DB: f64 = 50.0;

/// Minimum attenuation at [`STOPBAND_FAR_HZ`] and above (Figure 12/G.722
/// p. 12).
pub const STOPBAND_FAR_MIN_ATTEN_DB: f64 = 70.0;

// -----------------------------------------------------------------------
// Mask evaluation
// -----------------------------------------------------------------------

/// Outcome of evaluating a single (frequency, attenuation) measurement
/// against the Figure 12/G.722 mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskBand {
    /// Frequency is below the 50 Hz mask anchor — outside the
    /// reconstructing filter's normative coverage.
    BelowMask,
    /// Frequency sits inside the 50 Hz – [`PASSBAND_LOW_HZ`] transition
    /// strip. No tight numeric bound is printed in Figure 12/G.722; the
    /// host filter design picks a roll-on shape inside this strip.
    LowTransition,
    /// Frequency is in the tight in-band ripple region
    /// [`PASSBAND_LOW_HZ`] – [`PASSBAND_TIGHT_HIGH_HZ`] (±0.5 dB).
    InBandTight,
    /// Frequency is in the relaxed in-band ripple region
    /// [`PASSBAND_TIGHT_HIGH_HZ`] – [`PASSBAND_RELAXED_HIGH_HZ`]
    /// (−0.5 dB to +1.5 dB).
    InBandRelaxed,
    /// Frequency sits in the [`PASSBAND_RELAXED_HIGH_HZ`] –
    /// [`STOPBAND_ENTRY_HZ`] transition. Only the lower bound is
    /// printed in Figure 12/G.722 (−0.5 dB); the upper bound rolls up
    /// to the [`STOPBAND_ENTRY_MIN_ATTEN_DB`] floor.
    HighTransition,
    /// Frequency is in the stopband
    /// (≥ [`STOPBAND_ENTRY_HZ`]). Only a lower bound on attenuation
    /// applies; the floor depends on the frequency.
    Stopband,
}

/// Evaluate a measured `(frequency_hz, attenuation_db)` pair against the
/// Figure 12/G.722 mask.
///
/// Returns the [`MaskBand`] the frequency falls into and a `bool` that
/// is `true` when the measured attenuation lies inside the printed mask
/// for that band, or — for bands with only a one-sided constraint — when
/// it meets the printed bound. The mapping for each band:
///
/// | Band              | Constraint checked                                 |
/// | ----------------- | -------------------------------------------------- |
/// | `BelowMask`       | always `true` (outside normative coverage)         |
/// | `LowTransition`   | always `true` (no printed numeric bound)           |
/// | `InBandTight`     | `−0.5 dB ≤ atten ≤ +0.5 dB`                        |
/// | `InBandRelaxed`   | `−0.5 dB ≤ atten ≤ +1.5 dB`                        |
/// | `HighTransition`  | `−0.5 dB ≤ atten` (lower bound only)               |
/// | `Stopband`        | `atten ≥ floor(frequency)` (see [`stopband_floor_db`]) |
pub fn evaluate(frequency_hz: f64, attenuation_db: f64) -> (MaskBand, bool) {
    let band = classify(frequency_hz);
    let ok = match band {
        MaskBand::BelowMask => true,
        MaskBand::LowTransition => true,
        MaskBand::InBandTight => {
            (IN_BAND_LOWER_BOUND_DB..=IN_BAND_TIGHT_UPPER_BOUND_DB).contains(&attenuation_db)
        }
        MaskBand::InBandRelaxed => {
            (IN_BAND_LOWER_BOUND_DB..=IN_BAND_RELAXED_UPPER_BOUND_DB).contains(&attenuation_db)
        }
        MaskBand::HighTransition => attenuation_db >= IN_BAND_LOWER_BOUND_DB,
        MaskBand::Stopband => attenuation_db >= stopband_floor_db(frequency_hz),
    };
    (band, ok)
}

/// Classify a frequency into the [`MaskBand`] it belongs to. Frequencies
/// are sampled from the figure's printed log-axis breakpoints
/// (Figure 12/G.722 p. 12).
pub fn classify(frequency_hz: f64) -> MaskBand {
    // Use the spec-printed passband-low anchor of 50 Hz (the figure shows
    // 0.050 kHz as the first mask labelled point). Below that we fall
    // outside the mask's normative coverage.
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
    if frequency_hz < STOPBAND_ENTRY_HZ as f64 {
        return MaskBand::HighTransition;
    }
    MaskBand::Stopband
}

/// Minimum attenuation (in dB) the reconstructing filter must provide
/// at `frequency_hz` once the frequency lies in the stopband
/// (≥ [`STOPBAND_ENTRY_HZ`]). Figure 12/G.722 (p. 12) prints three
/// anchor points: 25 dB at 8 kHz, 50 dB at 9 kHz, 70 dB at 14 kHz. The
/// 8–9 kHz and 9–14 kHz segments are piecewise-linear on a log-frequency
/// axis (the printed mask shows two straight diagonal segments on the
/// log scale).
///
/// For frequencies below [`STOPBAND_ENTRY_HZ`] the function returns
/// `f64::NEG_INFINITY` — no stopband floor applies.
pub fn stopband_floor_db(frequency_hz: f64) -> f64 {
    if !frequency_hz.is_finite() || frequency_hz < STOPBAND_ENTRY_HZ as f64 {
        return f64::NEG_INFINITY;
    }
    let f8 = STOPBAND_ENTRY_HZ as f64;
    let f9 = STOPBAND_SHOULDER_HZ as f64;
    let f14 = STOPBAND_FAR_HZ as f64;
    if frequency_hz <= f9 {
        // Log-linear between 8 kHz (25 dB) and 9 kHz (50 dB).
        return interp_log(
            f8,
            STOPBAND_ENTRY_MIN_ATTEN_DB,
            f9,
            STOPBAND_SHOULDER_MIN_ATTEN_DB,
            frequency_hz,
        );
    }
    if frequency_hz <= f14 {
        // Log-linear between 9 kHz (50 dB) and 14 kHz (70 dB).
        return interp_log(
            f9,
            STOPBAND_SHOULDER_MIN_ATTEN_DB,
            f14,
            STOPBAND_FAR_MIN_ATTEN_DB,
            frequency_hz,
        );
    }
    // ≥ 14 kHz — the mask's flat 70 dB floor extends to the band edge.
    STOPBAND_FAR_MIN_ATTEN_DB
}

/// Linear interpolation between `(f1, y1)` and `(f2, y2)` on a log
/// frequency axis. Returns `y1` if `f1 == f2`.
fn interp_log(f1: f64, y1: f64, f2: f64, y2: f64, f: f64) -> f64 {
    if f1 == f2 {
        return y1;
    }
    let l1 = f1.log10();
    let l2 = f2.log10();
    let l = f.log10();
    let t = (l - l1) / (l2 - l1);
    y1 + t * (y2 - y1)
}

// -----------------------------------------------------------------------
// Compile-time invariants
// -----------------------------------------------------------------------

const _: () = {
    // Breakpoints must be strictly increasing across the printed
    // Figure 12/G.722 axis.
    assert!(NOMINAL_PASSBAND_LOW_HZ < PASSBAND_LOW_HZ);
    assert!(PASSBAND_LOW_HZ < PASSBAND_TIGHT_HIGH_HZ);
    assert!(PASSBAND_TIGHT_HIGH_HZ < PASSBAND_RELAXED_HIGH_HZ);
    assert!(PASSBAND_RELAXED_HIGH_HZ < STOPBAND_ENTRY_HZ);
    assert!(STOPBAND_ENTRY_HZ < STOPBAND_SHOULDER_HZ);
    assert!(STOPBAND_SHOULDER_HZ < STOPBAND_FAR_HZ);
    // The stopband begins at the QMF sample-rate boundary (8 kHz =
    // SUBBAND_SAMPLE_CLOCK_HZ, Figure 12/G.722's vertical line at 8 kHz
    // marks exactly that point).
    assert!(STOPBAND_ENTRY_HZ == SUBBAND_SAMPLE_CLOCK_HZ);
};

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_match_figure_12() {
        // The printed Figure 12/G.722 (p. 12) shows the labelled
        // frequencies 0.050 / 0.100 / 1 / 6.4 / 7 / 8 / 9 / 14 kHz on
        // the log frequency axis. The mask's anchor points must match.
        assert_eq!(NOMINAL_PASSBAND_LOW_HZ, 50);
        assert_eq!(PASSBAND_LOW_HZ, 100);
        assert_eq!(PASSBAND_TIGHT_HIGH_HZ, 6_400);
        assert_eq!(PASSBAND_RELAXED_HIGH_HZ, 7_000);
        assert_eq!(STOPBAND_ENTRY_HZ, 8_000);
        assert_eq!(STOPBAND_SHOULDER_HZ, 9_000);
        assert_eq!(STOPBAND_FAR_HZ, 14_000);
    }

    #[test]
    fn ripple_bounds_match_figure_12() {
        // Figure 12/G.722 (p. 12) prints −0.5 / +0.5 / +1.5 dB on the
        // attenuation axis for the in-band rectangles. The stopband
        // labels are 25 / 50 / 70 dB.
        assert_eq!(IN_BAND_LOWER_BOUND_DB, -0.5);
        assert_eq!(IN_BAND_TIGHT_UPPER_BOUND_DB, 0.5);
        assert_eq!(IN_BAND_RELAXED_UPPER_BOUND_DB, 1.5);
        assert_eq!(STOPBAND_ENTRY_MIN_ATTEN_DB, 25.0);
        assert_eq!(STOPBAND_SHOULDER_MIN_ATTEN_DB, 50.0);
        assert_eq!(STOPBAND_FAR_MIN_ATTEN_DB, 70.0);
    }

    #[test]
    fn classify_below_low_anchor_is_below_mask() {
        assert_eq!(classify(0.0), MaskBand::BelowMask);
        assert_eq!(classify(10.0), MaskBand::BelowMask);
        assert_eq!(classify(49.999), MaskBand::BelowMask);
    }

    #[test]
    fn classify_low_transition_region() {
        // Between 50 Hz and 100 Hz the mask sits in the low-transition
        // strip (the slant from the out-of-band stopband into the
        // in-band passband).
        assert_eq!(classify(50.0), MaskBand::LowTransition);
        assert_eq!(classify(75.0), MaskBand::LowTransition);
        assert_eq!(classify(99.999), MaskBand::LowTransition);
    }

    #[test]
    fn classify_tight_in_band() {
        // 100 Hz to 6.4 kHz inclusive is the tight ±0.5 dB ripple band.
        // Anchor at the printed 1000 Hz nominal reference frequency.
        assert_eq!(classify(100.0), MaskBand::InBandTight);
        assert_eq!(classify(1_000.0), MaskBand::InBandTight);
        assert_eq!(classify(3_500.0), MaskBand::InBandTight);
        assert_eq!(classify(6_400.0), MaskBand::InBandTight);
    }

    #[test]
    fn classify_relaxed_in_band() {
        // 6.4 kHz < f ≤ 7 kHz: relaxed upper bound of +1.5 dB.
        assert_eq!(classify(6_500.0), MaskBand::InBandRelaxed);
        assert_eq!(classify(6_800.0), MaskBand::InBandRelaxed);
        assert_eq!(classify(7_000.0), MaskBand::InBandRelaxed);
    }

    #[test]
    fn classify_high_transition() {
        // 7 kHz < f < 8 kHz: high transition where the upper bound
        // rolls up toward the stopband floor.
        assert_eq!(classify(7_500.0), MaskBand::HighTransition);
        assert_eq!(classify(7_999.999), MaskBand::HighTransition);
    }

    #[test]
    fn classify_stopband() {
        assert_eq!(classify(8_000.0), MaskBand::Stopband);
        assert_eq!(classify(9_000.0), MaskBand::Stopband);
        assert_eq!(classify(14_000.0), MaskBand::Stopband);
        assert_eq!(classify(50_000.0), MaskBand::Stopband);
    }

    #[test]
    fn evaluate_tight_in_band_passes_zero_db() {
        // 0 dB attenuation at 1 kHz must pass the ±0.5 dB ripple band.
        let (band, ok) = evaluate(1_000.0, 0.0);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(ok);
    }

    #[test]
    fn evaluate_tight_in_band_rejects_outside_pm_half_db() {
        // 0.6 dB attenuation breaks the +0.5 dB upper bound of the
        // tight in-band region.
        let (band, ok) = evaluate(1_000.0, 0.6);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(!ok);
        // −0.6 dB (= 0.6 dB of gain) breaks the −0.5 dB lower bound.
        let (band, ok) = evaluate(1_000.0, -0.6);
        assert_eq!(band, MaskBand::InBandTight);
        assert!(!ok);
    }

    #[test]
    fn evaluate_relaxed_in_band_admits_one_db() {
        // 1.0 dB attenuation at 6.8 kHz is inside the relaxed
        // [−0.5, +1.5] dB region and outside the tight one.
        let (band, ok) = evaluate(6_800.0, 1.0);
        assert_eq!(band, MaskBand::InBandRelaxed);
        assert!(ok);
        // A measurement at the same frequency that exceeds +1.5 dB is
        // a violation.
        let (band, ok) = evaluate(6_800.0, 1.6);
        assert_eq!(band, MaskBand::InBandRelaxed);
        assert!(!ok);
    }

    #[test]
    fn evaluate_stopband_anchor_25_db_at_8khz() {
        // Exactly the printed stopband anchor (25 dB at 8 kHz, Figure
        // 12/G.722 p. 12) must just meet the mask.
        let (band, ok) = evaluate(8_000.0, 25.0);
        assert_eq!(band, MaskBand::Stopband);
        assert!(ok);
        // 24 dB at 8 kHz fails.
        let (band, ok) = evaluate(8_000.0, 24.0);
        assert_eq!(band, MaskBand::Stopband);
        assert!(!ok);
    }

    #[test]
    fn evaluate_stopband_anchor_50_db_at_9khz() {
        let (_, ok) = evaluate(9_000.0, 50.0);
        assert!(ok);
        let (_, ok) = evaluate(9_000.0, 49.0);
        assert!(!ok);
    }

    #[test]
    fn evaluate_stopband_anchor_70_db_at_14khz() {
        let (_, ok) = evaluate(14_000.0, 70.0);
        assert!(ok);
        let (_, ok) = evaluate(14_000.0, 69.0);
        assert!(!ok);
    }

    #[test]
    fn stopband_floor_below_8khz_is_neg_infinity() {
        assert_eq!(stopband_floor_db(0.0), f64::NEG_INFINITY);
        assert_eq!(stopband_floor_db(1_000.0), f64::NEG_INFINITY);
        assert_eq!(stopband_floor_db(7_999.9), f64::NEG_INFINITY);
    }

    #[test]
    fn stopband_floor_anchor_values() {
        // Exact anchor points from Figure 12/G.722 (p. 12).
        let f8 = stopband_floor_db(8_000.0);
        let f9 = stopband_floor_db(9_000.0);
        let f14 = stopband_floor_db(14_000.0);
        assert!((f8 - 25.0).abs() < 1e-9);
        assert!((f9 - 50.0).abs() < 1e-9);
        assert!((f14 - 70.0).abs() < 1e-9);
    }

    #[test]
    fn stopband_floor_is_monotone_non_decreasing_with_frequency() {
        // Figure 12/G.722's printed stopband is a rising step/slope.
        // Sample the 8 kHz – 20 kHz range; the floor must never drop.
        let mut prev = stopband_floor_db(8_000.0);
        let mut f = 8_100.0_f64;
        while f <= 20_000.0 {
            let cur = stopband_floor_db(f);
            assert!(
                cur >= prev - 1e-12,
                "non-monotone stopband floor at {f} Hz: {cur} < {prev}"
            );
            prev = cur;
            f += 100.0;
        }
    }

    #[test]
    fn stopband_floor_above_14khz_is_flat_70db() {
        // The mask shows a flat 70 dB ceiling extending past 14 kHz.
        assert_eq!(stopband_floor_db(15_000.0), 70.0);
        assert_eq!(stopband_floor_db(20_000.0), 70.0);
        assert_eq!(stopband_floor_db(40_000.0), 70.0);
    }

    #[test]
    fn stopband_floor_intermediate_is_between_anchors() {
        // 8.5 kHz sits between the 25 dB and 50 dB anchors and must
        // produce a floor strictly between them.
        let mid = stopband_floor_db(8_500.0);
        assert!(mid > 25.0 && mid < 50.0, "got {mid}");
        // 12 kHz sits between the 50 dB and 70 dB anchors.
        let mid = stopband_floor_db(12_000.0);
        assert!(mid > 50.0 && mid < 70.0, "got {mid}");
    }

    #[test]
    fn evaluate_below_mask_always_passes() {
        // Outside the mask's normative coverage the result is "no
        // constraint applies"; the caller has to evaluate that band
        // separately (e.g. the analogue input filter mask of clause
        // 2.5.1 / Figure 11).
        let (band, ok) = evaluate(10.0, -100.0);
        assert_eq!(band, MaskBand::BelowMask);
        assert!(ok);
    }

    #[test]
    fn evaluate_low_transition_always_passes() {
        // Figure 12/G.722 doesn't pin a numeric upper bound inside the
        // 50–100 Hz slant; the host filter picks its own roll-on.
        let (band, ok) = evaluate(70.0, 0.0);
        assert_eq!(band, MaskBand::LowTransition);
        assert!(ok);
    }

    #[test]
    fn evaluate_high_transition_lower_bound_only() {
        // 7.5 kHz, +20 dB attenuation: meets the lower bound (which is
        // all the spec pins for this strip).
        let (band, ok) = evaluate(7_500.0, 20.0);
        assert_eq!(band, MaskBand::HighTransition);
        assert!(ok);
        // 7.5 kHz, -1 dB (i.e. 1 dB of *gain*): violates the −0.5 dB
        // lower bound.
        let (band, ok) = evaluate(7_500.0, -1.0);
        assert_eq!(band, MaskBand::HighTransition);
        assert!(!ok);
    }

    #[test]
    fn passband_in_band_relaxed_meets_tight_at_anchor() {
        // The 6.4 kHz breakpoint is the high edge of the tight region.
        // A measurement exactly at 6.4 kHz must classify as InBandTight
        // (i.e. the tight ±0.5 dB ripple constraint still applies at
        // the closed-interval endpoint).
        let (band, _) = evaluate(6_400.0, 0.0);
        assert_eq!(band, MaskBand::InBandTight);
        // Just past 6.4 kHz, the relaxed upper bound applies.
        let (band, _) = evaluate(6_400.001, 0.0);
        assert_eq!(band, MaskBand::InBandRelaxed);
    }

    #[test]
    fn nan_and_negative_handled_safely() {
        assert_eq!(classify(f64::NAN), MaskBand::BelowMask);
        assert_eq!(classify(-100.0), MaskBand::BelowMask);
        assert_eq!(stopband_floor_db(f64::NAN), f64::NEG_INFINITY);
        assert_eq!(stopband_floor_db(-1_000.0), f64::NEG_INFINITY);
    }

    #[test]
    fn stopband_floor_log_linear_at_geometric_mean() {
        // On a log axis the geometric mean of two breakpoints sits at
        // the arithmetic midpoint between their dB values. 8 kHz ↔
        // 9 kHz has geometric mean ≈ 8485.28 Hz; expected floor is
        // (25 + 50) / 2 = 37.5 dB.
        let gm = (8_000.0_f64 * 9_000.0).sqrt();
        let v = stopband_floor_db(gm);
        assert!((v - 37.5).abs() < 1e-9, "got {v}");
        // Same trick for the 9 kHz ↔ 14 kHz segment: geometric mean is
        // (9_000 × 14_000)^½ ≈ 11_224.97 Hz; expected ≈ (50 + 70)/2.
        let gm = (9_000.0_f64 * 14_000.0).sqrt();
        let v = stopband_floor_db(gm);
        assert!((v - 60.0).abs() < 1e-9, "got {v}");
    }
}

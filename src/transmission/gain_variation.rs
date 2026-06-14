//! Clause 2.5.7 / Figure 16/G.722 — variation of gain with input level.
//!
//! Clause 2.5.7 (p. 14) of the staged ITU-T G.722 (11/88)
//! Recommendation requires that, "with a sine wave signal at the
//! nominal reference frequency of 1000 Hz, but excluding the
//! sub-multiple of the 16 kHz sampling frequency, applied to test
//! point A, the gain variation as a function of input level relative
//! to the gain at an input level of −10 dBm0 measured selectively at
//! test point B" satisfies the limits of the mask drawn in Figure
//! 16/G.722 (p. 14, "Variation of gain with input level").
//!
//! Like the clause 2.5.1–2.5.6 masks ([`super::anti_aliasing_filter`]
//! / [`super::reconstructing_filter`] /
//! [`super::group_delay_distortion`] / [`super::signal_to_distortion`]
//! / [`super::signal_to_distortion_frequency`]), this is an
//! audio-parts requirement measured in the looped audio-to-audio
//! configuration of Figure 9b)/G.722 (p. 10; clause 2.5 p. 11).
//!
//! Unlike the single-sided distortion *floors* of Figures 14 / 15 and
//! the single-sided attenuation masks of Figures 11 / 12, Figure 16 is
//! a **two-sided corridor**: the measured gain variation must sit
//! between a lower and an upper bound, the hatching in the figure
//! filling the forbidden region both above the upper bold edge and
//! below the lower bold edge. The corridor is symmetric about 0 dB
//! (`±bound`) and **widens monotonically toward lower input levels** —
//! the gain tracks the −10 dBm0 reference tightly across the operating
//! range and is allowed to drift as the signal falls toward the
//! noise floor.
//!
//! Figure 16/G.722 prints input level in dBm0 on the abscissa
//! (labelled anchors −61 / −56 / −46 / −10 / +9 dBm0) and gain
//! variation in dB on the ordinate (labelled gridlines ±1.5 / ±0.5 /
//! ±0.3 dB about the 0 dB axis). The bold corridor reads as a
//! three-step symmetric staircase:
//!
//! | Input level (dBm0) | Corridor half-width (dB) |
//! | ------------------ | ------------------------ |
//! | below −61          | (no mask printed)        |
//! | −61 to −56         | ±1.5                     |
//! | −56 to −46         | ±0.5                     |
//! | −46 to +9          | ±0.3                     |
//! | above +9           | (no mask printed)        |
//!
//! The −10 dBm0 reference level (the gain that every measurement is
//! taken relative to) sits inside the tightest ±0.3 dB band — the gain
//! variation there is identically 0 dB by construction, comfortably
//! inside the corridor. The +9 dBm0 right wall is the clause 2.2
//! overload point itself ([`super::OVERLOAD_POINT_DBM0`]); above it the
//! converters clip and no mask is printed.
//!
//! Breakpoint ownership follows the sibling masks: each printed
//! transition belongs to the **stricter** (tighter) band, so −56 dBm0
//! is in the ±0.5 dB band and −46 dBm0 is in the ±0.3 dB band. The
//! −61 dBm0 left wall belongs to the ±1.5 dB band (the mask's loosest,
//! and outermost-printed, edge).
//!
//! ## Provenance
//!
//! Every anchor and dB value below is transcribed from the printed
//! Figure 16/G.722 and clause 2.5.7 text of
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf` (page 14). No external
//! reference implementation was consulted.

// -----------------------------------------------------------------------
// Measurement conditions (clause 2.5.7 page 14)
// -----------------------------------------------------------------------

/// Nominal frequency of the gain-variation measurement tone (clause
/// 2.5.7 p. 14: "a sine wave signal at the nominal reference frequency
/// of 1000 Hz, but excluding the sub-multiple of the 16 kHz sampling
/// frequency"). The 1000 Hz figure is the clause-2.5.7 label; the
/// clause-2.3 nominal reference frequency ([`super::NOMINAL_REFERENCE_FREQUENCY_HZ`]
/// = 1020 Hz) is the canonical realisation that excludes simple
/// harmonic relationships with the sampling clock.
pub const MEASUREMENT_TONE_HZ: u32 = 1_000;

/// Reference input level the gain variation is measured **relative
/// to** (clause 2.5.7 p. 14: "relative to the gain at an input level
/// of −10 dBm0"). At this level the gain variation is identically
/// 0 dB by definition; it is the same −10 dBm0 nominal test level used
/// by clauses 2.4.2 / 2.4.3 / 2.5.6.
pub const REFERENCE_LEVEL_DBM0: f64 = -10.0;

// -----------------------------------------------------------------------
// Mask anchors (Figure 16/G.722 page 14)
// -----------------------------------------------------------------------

/// Left wall of the Figure 16/G.722 mask (p. 14) — the printed
/// −61 dBm0 input-level anchor. Below it no mask is printed.
pub const INPUT_LEVEL_LOW_DBM0: f64 = -61.0;

/// First printed corridor step (Figure 16/G.722 p. 14) — the −56 dBm0
/// input-level anchor where the corridor tightens from ±1.5 dB to
/// ±0.5 dB.
pub const STEP_WIDE_DBM0: f64 = -56.0;

/// Second printed corridor step (Figure 16/G.722 p. 14) — the
/// −46 dBm0 input-level anchor where the corridor tightens from
/// ±0.5 dB to its ±0.3 dB operating-range value.
pub const STEP_TIGHT_DBM0: f64 = -46.0;

/// Right wall of the Figure 16/G.722 mask (p. 14) — the printed
/// +9 dBm0 input-level anchor, coincident with the clause 2.2 overload
/// point ([`super::OVERLOAD_POINT_DBM0`]). Above it no mask is printed.
pub const INPUT_LEVEL_HIGH_DBM0: f64 = 9.0;

/// Corridor half-width on −61 … −56 dBm0 (Figure 16/G.722 p. 14): the
/// printed ±1.5 dB gridlines — the loosest band, nearest the noise
/// floor.
pub const HALF_WIDTH_WIDE_DB: f64 = 1.5;

/// Corridor half-width on −56 … −46 dBm0 (Figure 16/G.722 p. 14): the
/// printed ±0.5 dB gridlines.
pub const HALF_WIDTH_MID_DB: f64 = 0.5;

/// Corridor half-width on −46 … +9 dBm0 (Figure 16/G.722 p. 14): the
/// printed ±0.3 dB gridlines — the tightest band, holding across the
/// whole operating range that brackets the −10 dBm0 reference level.
pub const HALF_WIDTH_TIGHT_DB: f64 = 0.3;

// -----------------------------------------------------------------------
// Mask evaluation
// -----------------------------------------------------------------------

/// One of the three corridor segments of the Figure 16/G.722 mask, or
/// the open regions beyond the printed input-level walls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskBand {
    /// Input level is below the −61 dBm0 left wall — outside the
    /// printed mask. No constraint applies.
    BelowMask,
    /// −61 … −56 dBm0: corridor half-width [`HALF_WIDTH_WIDE_DB`]
    /// (±1.5 dB).
    Wide,
    /// −56 … −46 dBm0: corridor half-width [`HALF_WIDTH_MID_DB`]
    /// (±0.5 dB).
    Mid,
    /// −46 … +9 dBm0: corridor half-width [`HALF_WIDTH_TIGHT_DB`]
    /// (±0.3 dB) — the operating-range band that brackets the
    /// −10 dBm0 reference level.
    Tight,
    /// Input level is above the +9 dBm0 right wall (the clause 2.2
    /// overload point) — outside the printed mask. No constraint
    /// applies.
    AboveMask,
}

impl MaskBand {
    /// Corridor half-width (dB) of this band, or `f64::INFINITY` for
    /// the open regions beyond the printed walls.
    pub const fn half_width_db(self) -> f64 {
        match self {
            Self::BelowMask | Self::AboveMask => f64::INFINITY,
            Self::Wide => HALF_WIDTH_WIDE_DB,
            Self::Mid => HALF_WIDTH_MID_DB,
            Self::Tight => HALF_WIDTH_TIGHT_DB,
        }
    }
}

/// Classify an input level (dBm0) into the [`MaskBand`] it occupies on
/// the Figure 16/G.722 corridor.
///
/// Each printed step belongs to the stricter (tighter) band: −56 dBm0
/// classifies as [`MaskBand::Mid`] and −46 dBm0 as [`MaskBand::Tight`].
/// The −61 dBm0 left wall belongs to the loosest printed band
/// ([`MaskBand::Wide`]); the +9 dBm0 right wall belongs to
/// [`MaskBand::Tight`]. A non-finite level classifies as
/// [`MaskBand::BelowMask`].
pub fn classify(input_level_dbm0: f64) -> MaskBand {
    if !input_level_dbm0.is_finite() || input_level_dbm0 < INPUT_LEVEL_LOW_DBM0 {
        return MaskBand::BelowMask;
    }
    if input_level_dbm0 > INPUT_LEVEL_HIGH_DBM0 {
        return MaskBand::AboveMask;
    }
    if input_level_dbm0 < STEP_WIDE_DBM0 {
        return MaskBand::Wide;
    }
    if input_level_dbm0 < STEP_TIGHT_DBM0 {
        return MaskBand::Mid;
    }
    MaskBand::Tight
}

/// Corridor half-width (dB) at `input_level_dbm0` — the magnitude of
/// the largest admissible gain variation in either direction. Returns
/// `f64::INFINITY` outside the printed −61 … +9 dBm0 span (no
/// constraint printed there).
pub fn half_width_db(input_level_dbm0: f64) -> f64 {
    classify(input_level_dbm0).half_width_db()
}

/// Upper bound (dB) of the admissible gain variation at
/// `input_level_dbm0`. The corridor is symmetric about 0 dB, so this
/// is `+`[`half_width_db`]. Returns `f64::INFINITY` outside the
/// printed span.
pub fn upper_bound_db(input_level_dbm0: f64) -> f64 {
    half_width_db(input_level_dbm0)
}

/// Lower bound (dB) of the admissible gain variation at
/// `input_level_dbm0`. The corridor is symmetric about 0 dB, so this
/// is `−`[`half_width_db`]. Returns `f64::NEG_INFINITY` outside the
/// printed span.
pub fn lower_bound_db(input_level_dbm0: f64) -> f64 {
    -half_width_db(input_level_dbm0)
}

/// Evaluate a measured `(input_level_dbm0, gain_variation_db)` pair
/// against the Figure 16/G.722 corridor.
///
/// Returns the [`MaskBand`] the input level falls into and a `bool`
/// that is `true` when the measured gain variation sits within the
/// corridor `[`[`lower_bound_db`]`, `[`upper_bound_db`]`]` for that
/// level. Outside the printed span (`BelowMask` / `AboveMask`) the
/// result is `true`: no constraint applies. A NaN gain variation fails
/// every in-mask band.
pub fn evaluate(input_level_dbm0: f64, gain_variation_db: f64) -> (MaskBand, bool) {
    let band = classify(input_level_dbm0);
    let ok = match band {
        MaskBand::BelowMask | MaskBand::AboveMask => true,
        _ => {
            gain_variation_db >= lower_bound_db(input_level_dbm0)
                && gain_variation_db <= upper_bound_db(input_level_dbm0)
        }
    };
    (band, ok)
}

// -----------------------------------------------------------------------
// Compile-time invariants
// -----------------------------------------------------------------------

const _: () = {
    // The printed input-level anchors are strictly increasing along the
    // Figure 16/G.722 abscissa.
    assert!(INPUT_LEVEL_LOW_DBM0 < STEP_WIDE_DBM0);
    assert!(STEP_WIDE_DBM0 < STEP_TIGHT_DBM0);
    assert!(STEP_TIGHT_DBM0 < REFERENCE_LEVEL_DBM0);
    assert!(REFERENCE_LEVEL_DBM0 < INPUT_LEVEL_HIGH_DBM0);
    // The corridor tightens with rising input level.
    assert!(HALF_WIDTH_WIDE_DB > HALF_WIDTH_MID_DB);
    assert!(HALF_WIDTH_MID_DB > HALF_WIDTH_TIGHT_DB);
    assert!(HALF_WIDTH_TIGHT_DB > 0.0);
};

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transmission::{
        signal_to_distortion_frequency, NOMINAL_PASSBAND_HIGH_HZ, NOMINAL_PASSBAND_LOW_HZ,
        NOMINAL_REFERENCE_FREQUENCY_HZ, OVERLOAD_POINT_DBM0,
    };

    #[test]
    fn anchors_match_figure_16() {
        // Figure 16/G.722 (p. 14) labels −61 / −56 / −46 / −10 / +9
        // dBm0 on its input-level axis and ±1.5 / ±0.5 / ±0.3 dB on
        // its gain-variation axis.
        assert_eq!(INPUT_LEVEL_LOW_DBM0, -61.0);
        assert_eq!(STEP_WIDE_DBM0, -56.0);
        assert_eq!(STEP_TIGHT_DBM0, -46.0);
        assert_eq!(REFERENCE_LEVEL_DBM0, -10.0);
        assert_eq!(INPUT_LEVEL_HIGH_DBM0, 9.0);
        assert_eq!(HALF_WIDTH_WIDE_DB, 1.5);
        assert_eq!(HALF_WIDTH_MID_DB, 0.5);
        assert_eq!(HALF_WIDTH_TIGHT_DB, 0.3);
    }

    #[test]
    fn measurement_tone_matches_clause_2_5_7() {
        // Clause 2.5.7 p. 14: "a sine wave signal at the nominal
        // reference frequency of 1000 Hz".
        assert_eq!(MEASUREMENT_TONE_HZ, 1_000);
    }

    #[test]
    fn classify_below_left_wall_is_below_mask() {
        assert_eq!(classify(-90.0), MaskBand::BelowMask);
        assert_eq!(classify(-61.001), MaskBand::BelowMask);
        assert_eq!(classify(f64::NAN), MaskBand::BelowMask);
        assert_eq!(classify(f64::NEG_INFINITY), MaskBand::BelowMask);
        assert_eq!(classify(f64::INFINITY), MaskBand::BelowMask);
    }

    #[test]
    fn classify_wide_band() {
        // The −61 dBm0 left wall belongs to the loosest printed band.
        assert_eq!(classify(-61.0), MaskBand::Wide);
        assert_eq!(classify(-58.0), MaskBand::Wide);
        // Just left of the first step.
        assert_eq!(classify(-56.001), MaskBand::Wide);
    }

    #[test]
    fn classify_mid_band() {
        // The −56 dBm0 step belongs to the stricter (±0.5 dB) band.
        assert_eq!(classify(-56.0), MaskBand::Mid);
        assert_eq!(classify(-50.0), MaskBand::Mid);
        // Just left of the second step.
        assert_eq!(classify(-46.001), MaskBand::Mid);
    }

    #[test]
    fn classify_tight_band() {
        // The −46 dBm0 step belongs to the stricter (±0.3 dB) band,
        // which holds across the operating range up to the right wall.
        assert_eq!(classify(-46.0), MaskBand::Tight);
        assert_eq!(classify(REFERENCE_LEVEL_DBM0), MaskBand::Tight);
        assert_eq!(classify(0.0), MaskBand::Tight);
        assert_eq!(classify(9.0), MaskBand::Tight);
    }

    #[test]
    fn classify_above_right_wall_is_above_mask() {
        assert_eq!(classify(9.001), MaskBand::AboveMask);
        assert_eq!(classify(20.0), MaskBand::AboveMask);
    }

    #[test]
    fn half_width_per_band() {
        assert_eq!(half_width_db(-58.0), 1.5);
        assert_eq!(half_width_db(-50.0), 0.5);
        assert_eq!(half_width_db(-10.0), 0.3);
        assert_eq!(half_width_db(-90.0), f64::INFINITY);
        assert_eq!(half_width_db(20.0), f64::INFINITY);
    }

    #[test]
    fn corridor_is_symmetric_about_zero() {
        for level in [-58.0, -50.0, -10.0, 0.0, 8.0] {
            assert_eq!(upper_bound_db(level), -lower_bound_db(level));
            assert!(upper_bound_db(level) > 0.0);
            assert!(lower_bound_db(level) < 0.0);
        }
    }

    #[test]
    fn bounds_match_each_printed_band() {
        // Wide band ±1.5 dB.
        assert_eq!(upper_bound_db(-58.0), 1.5);
        assert_eq!(lower_bound_db(-58.0), -1.5);
        // Mid band ±0.5 dB.
        assert_eq!(upper_bound_db(-50.0), 0.5);
        assert_eq!(lower_bound_db(-50.0), -0.5);
        // Tight band ±0.3 dB.
        assert_eq!(upper_bound_db(0.0), 0.3);
        assert_eq!(lower_bound_db(0.0), -0.3);
    }

    #[test]
    fn bounds_open_outside_the_printed_span() {
        assert_eq!(upper_bound_db(-90.0), f64::INFINITY);
        assert_eq!(lower_bound_db(-90.0), f64::NEG_INFINITY);
        assert_eq!(upper_bound_db(20.0), f64::INFINITY);
        assert_eq!(lower_bound_db(20.0), f64::NEG_INFINITY);
    }

    #[test]
    fn reference_level_gain_variation_is_inside_the_tight_band() {
        // By definition the gain variation at the −10 dBm0 reference
        // level is 0 dB; the corridor there is ±0.3 dB so the
        // reference sits comfortably inside.
        let (band, ok) = evaluate(REFERENCE_LEVEL_DBM0, 0.0);
        assert_eq!(band, MaskBand::Tight);
        assert!(ok);
        assert_eq!(half_width_db(REFERENCE_LEVEL_DBM0), 0.3);
    }

    #[test]
    fn corridor_tightens_monotonically_with_rising_level() {
        // Sweep the printed span on a 0.25 dB grid: the corridor
        // half-width is monotone non-increasing as input level rises.
        let mut level = INPUT_LEVEL_LOW_DBM0;
        let mut prev = half_width_db(level);
        while level <= INPUT_LEVEL_HIGH_DBM0 {
            let cur = half_width_db(level);
            assert!(
                cur <= prev,
                "corridor widened at {level} dBm0 ({cur} > {prev})"
            );
            prev = cur;
            level += 0.25;
        }
    }

    #[test]
    fn evaluate_corridor_boundary_semantics() {
        // Exactly on each edge passes; 0.01 dB beyond fails. Spot-check
        // one point per band.
        for level in [-58.0, -50.0, 0.0] {
            let hw = half_width_db(level);
            let (_, ok) = evaluate(level, hw);
            assert!(ok, "{level} dBm0: exact upper edge must pass");
            let (_, ok) = evaluate(level, -hw);
            assert!(ok, "{level} dBm0: exact lower edge must pass");
            let (_, ok) = evaluate(level, hw + 0.01);
            assert!(!ok, "{level} dBm0: above corridor must fail");
            let (_, ok) = evaluate(level, -hw - 0.01);
            assert!(!ok, "{level} dBm0: below corridor must fail");
            let (_, ok) = evaluate(level, 0.0);
            assert!(ok, "{level} dBm0: zero variation must pass");
        }
    }

    #[test]
    fn evaluate_step_transition_tightens_the_corridor() {
        // A +1.0 dB variation is admissible at −58 dBm0 (±1.5 band)
        // but not at −50 dBm0 (±0.5 band); a +0.4 dB variation is
        // admissible at −50 dBm0 but not at 0 dBm0 (±0.3 band).
        assert!(evaluate(-58.0, 1.0).1);
        assert!(!evaluate(-50.0, 1.0).1);
        assert!(evaluate(-50.0, 0.4).1);
        assert!(!evaluate(0.0, 0.4).1);
    }

    #[test]
    fn evaluate_outside_mask_always_passes() {
        let (band, ok) = evaluate(-90.0, 5.0);
        assert_eq!(band, MaskBand::BelowMask);
        assert!(ok);
        let (band, ok) = evaluate(20.0, 5.0);
        assert_eq!(band, MaskBand::AboveMask);
        assert!(ok);
    }

    #[test]
    fn evaluate_nan_variation_fails_in_mask() {
        for level in [-58.0, -50.0, 0.0] {
            let (_, ok) = evaluate(level, f64::NAN);
            assert!(!ok, "{level} dBm0: NaN variation must fail");
        }
        // Outside the mask, even a NaN passes (no constraint).
        assert!(evaluate(-90.0, f64::NAN).1);
    }

    #[test]
    fn right_wall_is_the_overload_point() {
        // Figure 16's +9 dBm0 right wall is the clause 2.2 overload
        // point itself — unlike Figure 14, whose right wall sits 1 dB
        // below it.
        assert_eq!(INPUT_LEVEL_HIGH_DBM0, OVERLOAD_POINT_DBM0);
    }

    #[test]
    fn reference_level_shared_with_sibling_clauses() {
        // The −10 dBm0 reference level is the same nominal test level
        // clause 2.5.6's frequency-swept distortion mask fixes its
        // input at (clauses 2.4.2 / 2.4.3 use it too).
        assert_eq!(
            REFERENCE_LEVEL_DBM0,
            signal_to_distortion_frequency::TEST_LEVEL_DBM0
        );
    }

    #[test]
    fn measurement_tone_sits_in_the_nominal_passband() {
        // 1000 Hz is inside the 50 – 7000 Hz nominal passband of
        // clause 2.4.1, and within 2% of the clause 2.3 nominal
        // reference frequency (1020 Hz) the clause text invokes.
        const _: () = assert!(MEASUREMENT_TONE_HZ > NOMINAL_PASSBAND_LOW_HZ);
        const _: () = assert!(MEASUREMENT_TONE_HZ < NOMINAL_PASSBAND_HIGH_HZ);
        let delta = NOMINAL_REFERENCE_FREQUENCY_HZ.abs_diff(MEASUREMENT_TONE_HZ);
        assert!(delta * 50 <= MEASUREMENT_TONE_HZ);
    }
}

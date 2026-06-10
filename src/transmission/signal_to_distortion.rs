//! Clause 2.5.5 / Figure 14/G.722 — signal-to-total distortion ratio
//! as a function of input level.
//!
//! Clause 2.5.5 (p. 13) of the staged ITU-T G.722 (11/88)
//! Recommendation requires that, "with a sine wave signal at a
//! frequency excluding simple harmonic relationships with the 16 kHz
//! sampling frequency, applied to test point A, the ratio of
//! signal-to-total distortion power as a function of input level
//! measured unweighted in the frequency range 50 to 7000 Hz at test
//! point B" satisfies the limits of the mask drawn in Figure 14/G.722
//! (p. 13, "Signal-to-total distortion ratio as a function of input
//! level"). The clause prescribes **two** measurements: "one at a
//! frequency of about 1 kHz and the other at a frequency of about
//! 6 kHz" — and the printed figure draws a separate curve for each.
//!
//! Like the clause 2.5.1–2.5.3 masks ([`super::anti_aliasing_filter`]
//! / [`super::reconstructing_filter`] /
//! [`super::group_delay_distortion`]), this is an audio-parts
//! requirement measured in the looped audio-to-audio configuration of
//! Figure 9b)/G.722 (p. 10; clause 2.5 p. 11). Unlike the attenuation
//! masks it is a **floor**: the measured signal-to-total distortion
//! ratio must sit at or *above* the printed line (the hatching in the
//! figure sits below/right of each curve, marking the forbidden side).
//!
//! Figure 14/G.722 prints both axes linear-in-dB: input level in dBm0
//! on the abscissa (labelled anchors −56 / −21 / −11 / +8 dBm0) and
//! signal-to-total distortion ratio in dB on the ordinate (labelled
//! gridlines 15 / 50 / 60 dB). Both curves share one rising diagonal
//! from the (−56 dBm0, 15 dB) left corner; each then breaks onto its
//! own horizontal plateau, met where the diagonal crosses its printed
//! gridline:
//!
//! | Input level (dBm0) | 1 kHz floor (dB)    | 6 kHz floor (dB)    |
//! | ------------------ | ------------------- | ------------------- |
//! | below −56          | (no mask printed)   | (no mask printed)   |
//! | −56 to −21         | rising diagonal     | rising diagonal     |
//! | −21 to −11         | rising diagonal     | 50 (plateau)        |
//! | −11 to +8          | 60 (plateau)        | 50 (plateau)        |
//! | above +8           | (no mask printed)   | (no mask printed)   |
//!
//! The three printed corners (−56, 15), (−21, 50) and (−11, 60) are
//! collinear with slope exactly 1 dB of required ratio per dB of
//! input level: 15 = −56 + 71, 50 = −21 + 71 and 60 = −11 + 71. The
//! shared diagonal is therefore `ratio_floor = level + 71 dB`
//! ([`DIAGONAL_OFFSET_DB`]) — every dB of extra input level buys one
//! dB of required signal-to-distortion until the curve's plateau.
//! Each plateau knee is where the diagonal meets the plateau, so the
//! floor is **continuous** across the whole printed span.
//!
//! Structural alignments worth pinning: the mask's right wall sits at
//! +8 dBm0 — 1 dB below the +9 dBm0 overload point of clause 2.2
//! ([`super::OVERLOAD_POINT_DBM0`]); the "about 1 kHz" tone is the
//! clause 2.3 nominal-reference-frequency regime (1020 Hz, chosen to
//! avoid sub-harmonic relationships with the 16 kHz sampling clock —
//! the same exclusion clause 2.5.5 repeats) and exercises the *lower*
//! sub-band, while the "about 6 kHz" tone falls above the 4 kHz QMF
//! band split and exercises the *higher* sub-band, whose printed
//! plateau is 10 dB lower; and the unweighted measurement window is
//! the familiar 50 – 7000 Hz band of clauses 2.4.1 / 2.4.4.
//!
//! ## Provenance
//!
//! Every anchor and dB value below is transcribed from the printed
//! Figure 14/G.722 and clause 2.5.5 text of
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf` (page 13).
//! [`DIAGONAL_OFFSET_DB`] is derived from the three printed corner
//! coordinates as documented above. No external reference
//! implementation was consulted.

// -----------------------------------------------------------------------
// Measurement tones (clause 2.5.5 page 13)
// -----------------------------------------------------------------------

/// Nominal frequency of the first prescribed measurement (clause
/// 2.5.5 p. 13: "one at a frequency of about 1 kHz"). The figure
/// labels its 60 dB plateau "1 kHz". The tone sits in the lower
/// sub-band (below the 4 kHz QMF band split of clause 1.4.1).
pub const MEASUREMENT_TONE_LOW_HZ: u32 = 1_000;

/// Nominal frequency of the second prescribed measurement (clause
/// 2.5.5 p. 13: "the other at a frequency of about 6 kHz"). The
/// figure labels its 50 dB plateau "6 kHz". The tone sits in the
/// higher sub-band (above the 4 kHz QMF band split of clause 1.4.1).
pub const MEASUREMENT_TONE_HIGH_HZ: u32 = 6_000;

// -----------------------------------------------------------------------
// Mask anchors (Figure 14/G.722 page 13)
// -----------------------------------------------------------------------

/// Left wall of the Figure 14/G.722 mask (p. 13) — the printed
/// −56 dBm0 input-level anchor. Below it no mask is printed.
pub const INPUT_LEVEL_LOW_DBM0: f64 = -56.0;

/// Right wall of the Figure 14/G.722 mask (p. 13) — the printed
/// +8 dBm0 input-level anchor, 1 dB below the +9 dBm0 overload point
/// of clause 2.2. Above it no mask is printed.
pub const INPUT_LEVEL_HIGH_DBM0: f64 = 8.0;

/// Signal-to-total distortion floor at the left wall (Figure
/// 14/G.722 p. 13): the printed 15 dB gridline met at −56 dBm0.
pub const FLOOR_AT_LOW_EDGE_DB: f64 = 15.0;

/// Input level at which the 1 kHz curve's diagonal meets its plateau
/// (Figure 14/G.722 p. 13): the printed −11 dBm0 anchor, where the
/// diagonal crosses the 60 dB gridline.
pub const KNEE_TONE_LOW_DBM0: f64 = -11.0;

/// Input level at which the 6 kHz curve's diagonal meets its plateau
/// (Figure 14/G.722 p. 13): the printed −21 dBm0 anchor, where the
/// diagonal crosses the 50 dB gridline.
pub const KNEE_TONE_HIGH_DBM0: f64 = -21.0;

/// Plateau of the 1 kHz curve (Figure 14/G.722 p. 13): the printed
/// 60 dB gridline, holding from −11 dBm0 to the +8 dBm0 right wall.
pub const PLATEAU_TONE_LOW_DB: f64 = 60.0;

/// Plateau of the 6 kHz curve (Figure 14/G.722 p. 13): the printed
/// 50 dB gridline, holding from −21 dBm0 to the +8 dBm0 right wall.
pub const PLATEAU_TONE_HIGH_DB: f64 = 50.0;

/// Offset of the shared rising diagonal: the required ratio on the
/// diagonal is `input_level_dbm0 + 71` dB. Derived from the three
/// printed corner coordinates of Figure 14/G.722 (p. 13), which are
/// collinear with slope 1: 15 = −56 + 71 (left corner), 50 = −21 + 71
/// (6 kHz knee) and 60 = −11 + 71 (1 kHz knee).
pub const DIAGONAL_OFFSET_DB: f64 = 71.0;

// -----------------------------------------------------------------------
// Mask evaluation
// -----------------------------------------------------------------------

/// One of the two measurement tones prescribed by clause 2.5.5
/// (p. 13). Figure 14/G.722 draws a separate mask curve for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementTone {
    /// The "about 1 kHz" measurement ([`MEASUREMENT_TONE_LOW_HZ`]) —
    /// lower sub-band; 60 dB plateau from −11 dBm0.
    About1KHz,
    /// The "about 6 kHz" measurement ([`MEASUREMENT_TONE_HIGH_HZ`]) —
    /// higher sub-band; 50 dB plateau from −21 dBm0.
    About6KHz,
}

impl MeasurementTone {
    /// Nominal tone frequency in Hz (the figure's curve label).
    pub const fn nominal_frequency_hz(self) -> u32 {
        match self {
            Self::About1KHz => MEASUREMENT_TONE_LOW_HZ,
            Self::About6KHz => MEASUREMENT_TONE_HIGH_HZ,
        }
    }

    /// Input level (dBm0) at which this curve's diagonal meets its
    /// plateau.
    pub const fn knee_dbm0(self) -> f64 {
        match self {
            Self::About1KHz => KNEE_TONE_LOW_DBM0,
            Self::About6KHz => KNEE_TONE_HIGH_DBM0,
        }
    }

    /// Plateau value (dB) this curve holds from its knee to the
    /// +8 dBm0 right wall.
    pub const fn plateau_db(self) -> f64 {
        match self {
            Self::About1KHz => PLATEAU_TONE_LOW_DB,
            Self::About6KHz => PLATEAU_TONE_HIGH_DB,
        }
    }
}

/// Outcome of classifying an input level against one curve of the
/// Figure 14/G.722 mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskBand {
    /// Input level is below the −56 dBm0 left wall — outside the
    /// printed mask. No constraint applies.
    BelowMask,
    /// On the shared rising diagonal: floor =
    /// `level + `[`DIAGONAL_OFFSET_DB`].
    Diagonal,
    /// On the curve's horizontal plateau (60 dB for the 1 kHz curve,
    /// 50 dB for the 6 kHz curve).
    Plateau,
    /// Input level is above the +8 dBm0 right wall — outside the
    /// printed mask (1 dB further sits clause 2.2's overload point).
    /// No constraint applies.
    AboveMask,
}

/// Classify an input level (dBm0) into the [`MaskBand`] it occupies
/// on the given tone's curve.
///
/// Edge ownership follows the same convention as the sibling masks:
/// the printed walls belong to the mask (−56 dBm0 classifies as
/// `Diagonal`, +8 dBm0 as `Plateau`). The knee is assigned to the
/// `Plateau`; because the diagonal meets the plateau exactly there
/// (the floor is continuous), the assignment does not change the
/// required value at the knee.
pub fn classify(tone: MeasurementTone, input_level_dbm0: f64) -> MaskBand {
    if !input_level_dbm0.is_finite() || input_level_dbm0 < INPUT_LEVEL_LOW_DBM0 {
        return MaskBand::BelowMask;
    }
    if input_level_dbm0 > INPUT_LEVEL_HIGH_DBM0 {
        return MaskBand::AboveMask;
    }
    if input_level_dbm0 < tone.knee_dbm0() {
        return MaskBand::Diagonal;
    }
    MaskBand::Plateau
}

/// Minimum admissible signal-to-total distortion ratio (in dB) at
/// `input_level_dbm0` on the given tone's curve. Returns
/// `f64::NEG_INFINITY` outside the printed −56 … +8 dBm0 span (no
/// constraint printed there — for a floor, "no constraint" is an
/// infinitely low bar).
pub fn min_ratio_db(tone: MeasurementTone, input_level_dbm0: f64) -> f64 {
    match classify(tone, input_level_dbm0) {
        MaskBand::BelowMask | MaskBand::AboveMask => f64::NEG_INFINITY,
        MaskBand::Diagonal => input_level_dbm0 + DIAGONAL_OFFSET_DB,
        MaskBand::Plateau => tone.plateau_db(),
    }
}

/// Evaluate a measured `(input_level_dbm0, ratio_db)` pair against
/// the given tone's curve of the Figure 14/G.722 mask.
///
/// Returns the [`MaskBand`] the input level falls into and a `bool`
/// that is `true` when the measured signal-to-total distortion ratio
/// sits at or above the printed floor for that level. The mask only
/// prints a floor — more signal-to-distortion than required is always
/// admissible. Outside the printed span (`BelowMask` / `AboveMask`)
/// the result is `true`: no constraint applies. A NaN ratio fails
/// every in-mask band.
pub fn evaluate(tone: MeasurementTone, input_level_dbm0: f64, ratio_db: f64) -> (MaskBand, bool) {
    let band = classify(tone, input_level_dbm0);
    let ok = match band {
        MaskBand::BelowMask | MaskBand::AboveMask => true,
        _ => ratio_db >= min_ratio_db(tone, input_level_dbm0),
    };
    (band, ok)
}

// -----------------------------------------------------------------------
// Compile-time invariants
// -----------------------------------------------------------------------

const _: () = {
    // The printed input-level anchors are strictly increasing along
    // the Figure 14/G.722 abscissa.
    assert!(INPUT_LEVEL_LOW_DBM0 < KNEE_TONE_HIGH_DBM0);
    assert!(KNEE_TONE_HIGH_DBM0 < KNEE_TONE_LOW_DBM0);
    assert!(KNEE_TONE_LOW_DBM0 < INPUT_LEVEL_HIGH_DBM0);
    // The three printed corners are collinear on the slope-1 diagonal
    // `ratio = level + 71`.
    assert!(INPUT_LEVEL_LOW_DBM0 + DIAGONAL_OFFSET_DB == FLOOR_AT_LOW_EDGE_DB);
    assert!(KNEE_TONE_HIGH_DBM0 + DIAGONAL_OFFSET_DB == PLATEAU_TONE_HIGH_DB);
    assert!(KNEE_TONE_LOW_DBM0 + DIAGONAL_OFFSET_DB == PLATEAU_TONE_LOW_DB);
    // The higher-sub-band tone's plateau is the lower of the two.
    assert!(PLATEAU_TONE_HIGH_DB < PLATEAU_TONE_LOW_DB);
    // The two prescribed tones straddle the 4 kHz QMF band split.
    assert!(MEASUREMENT_TONE_LOW_HZ < 4_000);
    assert!(MEASUREMENT_TONE_HIGH_HZ > 4_000);
};

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transmission::{
        IDLE_NOISE_BAND_LOW_HZ, IDLE_NOISE_NARROWBAND_HIGH_HZ, NOMINAL_PASSBAND_HIGH_HZ,
        NOMINAL_REFERENCE_FREQUENCY_HZ, OVERLOAD_POINT_DBM0, SUBBAND_SAMPLE_CLOCK_HZ,
    };
    use MeasurementTone::{About1KHz, About6KHz};

    #[test]
    fn anchors_match_figure_14() {
        // Figure 14/G.722 (p. 13) labels −56 / −21 / −11 / +8 dBm0 on
        // its input-level axis and 15 / 50 / 60 dB on its ratio axis.
        assert_eq!(INPUT_LEVEL_LOW_DBM0, -56.0);
        assert_eq!(KNEE_TONE_HIGH_DBM0, -21.0);
        assert_eq!(KNEE_TONE_LOW_DBM0, -11.0);
        assert_eq!(INPUT_LEVEL_HIGH_DBM0, 8.0);
        assert_eq!(FLOOR_AT_LOW_EDGE_DB, 15.0);
        assert_eq!(PLATEAU_TONE_HIGH_DB, 50.0);
        assert_eq!(PLATEAU_TONE_LOW_DB, 60.0);
    }

    #[test]
    fn measurement_tones_match_clause_2_5_5() {
        // Clause 2.5.5 p. 13: "one at a frequency of about 1 kHz and
        // the other at a frequency of about 6 kHz".
        assert_eq!(MEASUREMENT_TONE_LOW_HZ, 1_000);
        assert_eq!(MEASUREMENT_TONE_HIGH_HZ, 6_000);
        assert_eq!(About1KHz.nominal_frequency_hz(), 1_000);
        assert_eq!(About6KHz.nominal_frequency_hz(), 6_000);
    }

    #[test]
    fn diagonal_passes_through_all_three_printed_corners() {
        // The slope-1 diagonal `ratio = level + 71` reproduces every
        // printed corner of Figure 14/G.722 exactly.
        assert_eq!(
            INPUT_LEVEL_LOW_DBM0 + DIAGONAL_OFFSET_DB,
            FLOOR_AT_LOW_EDGE_DB
        );
        assert_eq!(
            KNEE_TONE_HIGH_DBM0 + DIAGONAL_OFFSET_DB,
            PLATEAU_TONE_HIGH_DB
        );
        assert_eq!(KNEE_TONE_LOW_DBM0 + DIAGONAL_OFFSET_DB, PLATEAU_TONE_LOW_DB);
    }

    #[test]
    fn tone_accessors_match_figure_curves() {
        assert_eq!(About1KHz.knee_dbm0(), -11.0);
        assert_eq!(About1KHz.plateau_db(), 60.0);
        assert_eq!(About6KHz.knee_dbm0(), -21.0);
        assert_eq!(About6KHz.plateau_db(), 50.0);
    }

    #[test]
    fn classify_below_left_wall_is_below_mask() {
        for tone in [About1KHz, About6KHz] {
            assert_eq!(classify(tone, -90.0), MaskBand::BelowMask);
            assert_eq!(classify(tone, -56.001), MaskBand::BelowMask);
            assert_eq!(classify(tone, f64::NAN), MaskBand::BelowMask);
            assert_eq!(classify(tone, f64::NEG_INFINITY), MaskBand::BelowMask);
            assert_eq!(classify(tone, f64::INFINITY), MaskBand::BelowMask);
        }
    }

    #[test]
    fn classify_diagonal_region_per_tone() {
        // The left wall belongs to the mask (stricter side).
        assert_eq!(classify(About1KHz, -56.0), MaskBand::Diagonal);
        assert_eq!(classify(About6KHz, -56.0), MaskBand::Diagonal);
        // Interior diagonal points.
        assert_eq!(classify(About1KHz, -30.0), MaskBand::Diagonal);
        assert_eq!(classify(About6KHz, -30.0), MaskBand::Diagonal);
        // The 1 kHz diagonal extends past the 6 kHz knee.
        assert_eq!(classify(About1KHz, -15.0), MaskBand::Diagonal);
        assert_eq!(classify(About6KHz, -15.0), MaskBand::Plateau);
    }

    #[test]
    fn classify_plateau_region_per_tone() {
        // Knees belong to the plateau (the floor is continuous there
        // so the choice is presentational).
        assert_eq!(classify(About1KHz, -11.0), MaskBand::Plateau);
        assert_eq!(classify(About6KHz, -21.0), MaskBand::Plateau);
        // Interior + right wall.
        for tone in [About1KHz, About6KHz] {
            assert_eq!(classify(tone, 0.0), MaskBand::Plateau);
            assert_eq!(classify(tone, 8.0), MaskBand::Plateau);
        }
    }

    #[test]
    fn classify_above_right_wall_is_above_mask() {
        for tone in [About1KHz, About6KHz] {
            assert_eq!(classify(tone, 8.001), MaskBand::AboveMask);
            assert_eq!(classify(tone, OVERLOAD_POINT_DBM0), MaskBand::AboveMask);
            assert_eq!(classify(tone, 20.0), MaskBand::AboveMask);
        }
    }

    #[test]
    fn floor_on_the_diagonal_is_level_plus_71() {
        for tone in [About1KHz, About6KHz] {
            assert_eq!(min_ratio_db(tone, -56.0), 15.0);
            assert_eq!(min_ratio_db(tone, -40.0), 31.0);
            assert_eq!(min_ratio_db(tone, -25.0), 46.0);
        }
    }

    #[test]
    fn floor_on_each_plateau_is_the_printed_gridline() {
        assert_eq!(min_ratio_db(About1KHz, -11.0), 60.0);
        assert_eq!(min_ratio_db(About1KHz, 0.0), 60.0);
        assert_eq!(min_ratio_db(About1KHz, 8.0), 60.0);
        assert_eq!(min_ratio_db(About6KHz, -21.0), 50.0);
        assert_eq!(min_ratio_db(About6KHz, 0.0), 50.0);
        assert_eq!(min_ratio_db(About6KHz, 8.0), 50.0);
    }

    #[test]
    fn floor_is_continuous_at_each_knee() {
        // Approaching each knee from the diagonal side converges on
        // the plateau value — the printed curves have no jump.
        let eps = 1e-9;
        let just_left_1k = min_ratio_db(About1KHz, KNEE_TONE_LOW_DBM0 - eps);
        assert!((just_left_1k - PLATEAU_TONE_LOW_DB).abs() < 1e-6);
        let just_left_6k = min_ratio_db(About6KHz, KNEE_TONE_HIGH_DBM0 - eps);
        assert!((just_left_6k - PLATEAU_TONE_HIGH_DB).abs() < 1e-6);
    }

    #[test]
    fn floor_is_neg_infinity_outside_the_mask() {
        for tone in [About1KHz, About6KHz] {
            assert_eq!(min_ratio_db(tone, -56.001), f64::NEG_INFINITY);
            assert_eq!(min_ratio_db(tone, -90.0), f64::NEG_INFINITY);
            assert_eq!(min_ratio_db(tone, 8.001), f64::NEG_INFINITY);
            assert_eq!(min_ratio_db(tone, 20.0), f64::NEG_INFINITY);
            assert_eq!(min_ratio_db(tone, f64::NAN), f64::NEG_INFINITY);
        }
    }

    #[test]
    fn floor_is_monotone_non_decreasing_across_the_span() {
        // More input level never relaxes the requirement: sweep both
        // curves on a 0.5 dB grid across the printed span.
        for tone in [About1KHz, About6KHz] {
            let mut level = INPUT_LEVEL_LOW_DBM0;
            let mut prev = min_ratio_db(tone, level);
            while level <= INPUT_LEVEL_HIGH_DBM0 {
                let cur = min_ratio_db(tone, level);
                assert!(
                    cur >= prev,
                    "{tone:?} floor dipped at {level} dBm0 ({cur} < {prev})"
                );
                prev = cur;
                level += 0.5;
            }
        }
    }

    #[test]
    fn plateau_is_the_global_maximum_of_each_curve() {
        for tone in [About1KHz, About6KHz] {
            let mut level = INPUT_LEVEL_LOW_DBM0;
            while level <= INPUT_LEVEL_HIGH_DBM0 {
                assert!(
                    min_ratio_db(tone, level) <= tone.plateau_db(),
                    "{tone:?} floor at {level} dBm0 exceeds its plateau"
                );
                level += 0.5;
            }
        }
    }

    #[test]
    fn one_khz_requirement_is_stricter_between_the_knees() {
        // Between −21 and −11 dBm0 the 1 kHz curve keeps rising while
        // the 6 kHz curve has already capped at 50 dB.
        for level in [-20.0, -16.0, -12.0] {
            let low = min_ratio_db(About1KHz, level);
            let high = min_ratio_db(About6KHz, level);
            assert!(
                low > high,
                "1 kHz floor {low} not stricter than 6 kHz floor {high} at {level} dBm0"
            );
            assert_eq!(high, PLATEAU_TONE_HIGH_DB);
        }
        // On the shared diagonal (at or below −21 dBm0) both curves
        // require the same ratio.
        for level in [-56.0, -40.0, -21.0 - 1e-9] {
            assert_eq!(
                min_ratio_db(About1KHz, level),
                min_ratio_db(About6KHz, level)
            );
        }
    }

    #[test]
    fn evaluate_floor_boundary_semantics() {
        // Exactly at the printed floor the measurement passes; 0.01 dB
        // under it fails. Spot-check one diagonal and one plateau
        // point per curve.
        for (tone, level) in [
            (About1KHz, -30.0),
            (About1KHz, 0.0),
            (About6KHz, -30.0),
            (About6KHz, 0.0),
        ] {
            let floor = min_ratio_db(tone, level);
            let (_, ok) = evaluate(tone, level, floor);
            assert!(ok, "{tone:?} at {level} dBm0: exact floor must pass");
            let (_, ok) = evaluate(tone, level, floor - 0.01);
            assert!(!ok, "{tone:?} at {level} dBm0: under-floor must fail");
            let (_, ok) = evaluate(tone, level, floor + 30.0);
            assert!(ok, "{tone:?} at {level} dBm0: headroom must pass");
        }
    }

    #[test]
    fn evaluate_outside_mask_always_passes() {
        for tone in [About1KHz, About6KHz] {
            let (band, ok) = evaluate(tone, -90.0, 0.0);
            assert_eq!(band, MaskBand::BelowMask);
            assert!(ok);
            let (band, ok) = evaluate(tone, 9.0, 0.0);
            assert_eq!(band, MaskBand::AboveMask);
            assert!(ok);
        }
    }

    #[test]
    fn evaluate_nan_ratio_fails_in_mask() {
        for tone in [About1KHz, About6KHz] {
            let (band, ok) = evaluate(tone, -30.0, f64::NAN);
            assert_eq!(band, MaskBand::Diagonal);
            assert!(!ok);
            let (band, ok) = evaluate(tone, 0.0, f64::NAN);
            assert_eq!(band, MaskBand::Plateau);
            assert!(!ok);
        }
    }

    #[test]
    fn right_wall_sits_one_db_under_the_overload_point() {
        // Figure 14's +8 dBm0 right wall is 1 dB below the +9 dBm0
        // overload point of clause 2.2 — the highest input level the
        // mask constrains before the converters clip.
        assert_eq!(INPUT_LEVEL_HIGH_DBM0 + 1.0, OVERLOAD_POINT_DBM0);
    }

    #[test]
    fn tones_straddle_the_qmf_band_split() {
        // 1 kHz exercises the lower sub-band, 6 kHz the higher one
        // (the QMF split of clause 1.4.1 sits at 4 kHz = half the
        // 8 kHz sub-band sample clock), and both sit inside the
        // 50 – 7000 Hz nominal passband of clause 2.4.1.
        let split = SUBBAND_SAMPLE_CLOCK_HZ / 2;
        assert!(MEASUREMENT_TONE_LOW_HZ < split);
        assert!(MEASUREMENT_TONE_HIGH_HZ > split);
        const _: () = assert!(MEASUREMENT_TONE_HIGH_HZ < NOMINAL_PASSBAND_HIGH_HZ);
    }

    #[test]
    fn about_one_khz_admits_the_nominal_reference_frequency() {
        // Clause 2.5.5 repeats clause 2.3's exclusion of "simple
        // harmonic relationships with the 16 kHz sampling frequency";
        // the 1020 Hz nominal reference frequency of clause 2.3 is
        // the canonical "about 1 kHz" tone satisfying it (within
        // 2% of the curve's 1 kHz label).
        let delta = NOMINAL_REFERENCE_FREQUENCY_HZ.abs_diff(MEASUREMENT_TONE_LOW_HZ);
        assert!(delta * 50 <= MEASUREMENT_TONE_LOW_HZ);
    }

    #[test]
    fn measurement_window_is_the_familiar_unweighted_band() {
        // Clause 2.5.5 measures "unweighted in the frequency range 50
        // to 7000 Hz" — the same window the clause 2.4.4 narrow-band
        // idle-noise measurement uses.
        assert_eq!(IDLE_NOISE_BAND_LOW_HZ, 50);
        assert_eq!(IDLE_NOISE_NARROWBAND_HIGH_HZ, 7_000);
    }
}

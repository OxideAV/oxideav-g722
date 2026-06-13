//! Clause 2.5.6 / Figure 15/G.722 — signal-to-total distortion ratio
//! as a function of frequency.
//!
//! Clause 2.5.6 (p. 14) of the staged ITU-T G.722 (11/88)
//! Recommendation requires that, "with a sine wave signal at a level
//! of −10 dBm0 applied to test point A, the ratio of signal-to-total
//! distortion power as a function of frequency measured unweighted in
//! the frequency range 50 to 7000 Hz at test point B" satisfies the
//! limits of the mask drawn in Figure 15/G.722 (p. 14,
//! "Signal-to-total distortion ratio as a function of frequency").
//!
//! It is the *frequency-swept* companion of clause 2.5.5's
//! *level-swept* mask ([`super::signal_to_distortion`], Figure
//! 14/G.722): clause 2.5.5 fixes the frequency at "about 1 kHz" /
//! "about 6 kHz" and sweeps the input level, while clause 2.5.6 fixes
//! the input level at the clause 2.3 nominal test level (−10 dBm0 —
//! the same level clauses 2.4.2 / 2.4.3 use) and sweeps the frequency.
//! Like the clause 2.5.1–2.5.3 masks
//! ([`super::anti_aliasing_filter`] / [`super::reconstructing_filter`]
//! / [`super::group_delay_distortion`]) it is an audio-parts
//! requirement measured in the looped audio-to-audio configuration of
//! Figure 9b)/G.722 (p. 10; clause 2.5 p. 11), and like Figure 14 it
//! is a **floor**: the measured ratio must sit at or *above* the
//! printed line (the hatching in the figure marks the forbidden side,
//! below/left of the bold staircase outline).
//!
//! Figure 15/G.722 prints the frequency on a log axis (labelled
//! anchors 0.050 / 0.100 / 4 / 6 / 7 kHz) and the signal-to-total
//! distortion ratio in dB on the ordinate (labelled gridlines 46.2 /
//! 50 / 60 dB). The bold floor outline reads:
//!
//! | Frequency band   | Signal-to-total distortion floor (dB)       |
//! | ---------------- | ------------------------------------------- |
//! | below 50 Hz      | (no mask printed)                           |
//! | 50 Hz to 100 Hz  | 50 (plateau)                                |
//! | 100 Hz to 4 kHz  | 60 (plateau — the global maximum)           |
//! | 4 kHz to 6 kHz   | log-linear ramp 60 → 46.2                    |
//! | 6 kHz to 7 kHz   | 46.2 (plateau)                              |
//! | above 7 kHz      | (no mask printed; right wall)               |
//!
//! The 60 → 46.2 dB ramp between 4 kHz and 6 kHz is drawn as a
//! straight line on the figure's log frequency axis, so it is
//! interpolated log-linearly in frequency (matching the 8–9 kHz
//! stopband ramps of the [`super::reconstructing_filter`] /
//! [`super::anti_aliasing_filter`] masks).
//!
//! Structural alignments worth pinning. The mask's right wall sits at
//! 7 kHz — the clause 2.4.1 nominal 3-dB passband high edge
//! ([`super::NOMINAL_PASSBAND_HIGH_HZ`]) — and its left plateau enters
//! at the 50 Hz passband low edge ([`super::NOMINAL_PASSBAND_LOW_HZ`]);
//! the 0.050 / 0.100 / 7 kHz anchors are the same passband-edge anchors
//! the Figure 11 / 12 / 13 masks use. The 60 dB plateau ends at 4 kHz,
//! the QMF band-split frequency (the lower sub-band of clause 1.4.1
//! spans 0 – 4000 Hz; 4 kHz = [`super::SUBBAND_SAMPLE_CLOCK_HZ`] / 2):
//! above the split the coarser higher sub-band budget pulls the floor
//! down toward the 46.2 dB that the clause 2.5.5 "about 6 kHz" curve's
//! plateau (50 dB) sits just above. The unweighted measurement window
//! is the familiar 50 – 7000 Hz band of clauses 2.4.1 / 2.4.4 / 2.5.5.
//!
//! ## Provenance
//!
//! Every breakpoint and dB value below is transcribed from the printed
//! Figure 15/G.722 and clause 2.5.6 text of
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf` (page 14). No external
//! reference implementation was consulted.

use crate::transmission::{
    NOMINAL_PASSBAND_HIGH_HZ, NOMINAL_PASSBAND_LOW_HZ, SUBBAND_SAMPLE_CLOCK_HZ,
};

// -----------------------------------------------------------------------
// Measurement conditions (clause 2.5.6 page 14)
// -----------------------------------------------------------------------

/// Input level at which the frequency sweep is performed (clause 2.5.6
/// p. 14: "with a sine wave signal at a level of −10 dBm0"). This is
/// the same nominal test level clauses 2.4.2 / 2.4.3 use.
pub const TEST_LEVEL_DBM0: f64 = -10.0;

// -----------------------------------------------------------------------
// Mask anchors (Figure 15/G.722 page 14)
// -----------------------------------------------------------------------

/// Low edge of the mask (Figure 15/G.722 p. 14) — the printed 0.050 kHz
/// = 50 Hz frequency anchor, the clause 2.4.1 nominal passband low edge.
/// Below it no mask is printed.
pub const PASSBAND_LOW_HZ: f64 = 50.0;

/// Frequency at which the floor steps up from the 50 dB left plateau to
/// the 60 dB main plateau (Figure 15/G.722 p. 14): the printed
/// 0.100 kHz = 100 Hz anchor.
pub const PLATEAU_LOW_KNEE_HZ: f64 = 100.0;

/// Frequency at which the 60 dB main plateau ends and the descending
/// ramp begins (Figure 15/G.722 p. 14): the printed 4 kHz anchor — the
/// QMF band-split frequency.
pub const RAMP_START_HZ: f64 = 4_000.0;

/// Frequency at which the descending ramp meets the 46.2 dB right
/// plateau (Figure 15/G.722 p. 14): the printed 6 kHz anchor.
pub const RAMP_END_HZ: f64 = 6_000.0;

/// High edge of the mask (Figure 15/G.722 p. 14) — the printed 7 kHz
/// frequency anchor, the clause 2.4.1 nominal passband high edge. Above
/// it no mask is printed (the figure's right wall).
pub const PASSBAND_HIGH_HZ: f64 = 7_000.0;

/// Signal-to-total distortion floor on the 50 Hz – 100 Hz left plateau
/// (Figure 15/G.722 p. 14): the printed 50 dB gridline.
pub const FLOOR_LOW_PLATEAU_DB: f64 = 50.0;

/// Signal-to-total distortion floor on the 100 Hz – 4 kHz main plateau
/// (Figure 15/G.722 p. 14): the printed 60 dB gridline — the global
/// maximum of the mask.
pub const FLOOR_MAIN_PLATEAU_DB: f64 = 60.0;

/// Signal-to-total distortion floor on the 6 kHz – 7 kHz right plateau,
/// and the foot of the 4 – 6 kHz ramp (Figure 15/G.722 p. 14): the
/// printed 46.2 dB gridline.
pub const FLOOR_HIGH_PLATEAU_DB: f64 = 46.2;

// -----------------------------------------------------------------------
// Mask evaluation
// -----------------------------------------------------------------------

/// One band of the Figure 15/G.722 floor mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskBand {
    /// Frequency is below the 50 Hz left wall — outside the printed
    /// mask. No constraint applies.
    BelowMask,
    /// 50 Hz – 100 Hz: floor = [`FLOOR_LOW_PLATEAU_DB`] (50 dB).
    LowPlateau,
    /// 100 Hz – 4 kHz: floor = [`FLOOR_MAIN_PLATEAU_DB`] (60 dB) — the
    /// global maximum of the mask.
    MainPlateau,
    /// 4 kHz – 6 kHz: log-linear ramp from [`FLOOR_MAIN_PLATEAU_DB`]
    /// (60 dB) down to [`FLOOR_HIGH_PLATEAU_DB`] (46.2 dB).
    Ramp,
    /// 6 kHz – 7 kHz: floor = [`FLOOR_HIGH_PLATEAU_DB`] (46.2 dB).
    HighPlateau,
    /// Frequency is above the 7 kHz right wall — outside the printed
    /// mask. No constraint applies.
    AboveMask,
}

/// Classify a frequency (Hz) into the [`MaskBand`] it occupies on the
/// Figure 15/G.722 mask.
///
/// Edge ownership follows the same convention as the sibling masks: at
/// a printed breakpoint the band carrying the *stricter* (higher) floor
/// owns the breakpoint. So 100 Hz classifies as `MainPlateau` (60 dB,
/// stricter than the 50 dB `LowPlateau`), 4 kHz as `MainPlateau`
/// (60 dB, the ramp foot at 4 kHz also equals 60 dB so this is exact),
/// and 6 kHz as `Ramp` (the ramp meets the 46.2 dB plateau there, so
/// the choice is presentational). The 50 Hz and 7 kHz walls belong to
/// the mask.
pub fn classify(frequency_hz: f64) -> MaskBand {
    if !frequency_hz.is_finite() || frequency_hz < PASSBAND_LOW_HZ {
        return MaskBand::BelowMask;
    }
    if frequency_hz > PASSBAND_HIGH_HZ {
        return MaskBand::AboveMask;
    }
    if frequency_hz < PLATEAU_LOW_KNEE_HZ {
        return MaskBand::LowPlateau;
    }
    if frequency_hz <= RAMP_START_HZ {
        return MaskBand::MainPlateau;
    }
    if frequency_hz <= RAMP_END_HZ {
        return MaskBand::Ramp;
    }
    MaskBand::HighPlateau
}

/// Minimum admissible signal-to-total distortion ratio (in dB) at
/// `frequency_hz` on the Figure 15/G.722 mask. Returns
/// `f64::NEG_INFINITY` outside the printed 50 Hz … 7 kHz span (no
/// constraint printed there — for a floor, "no constraint" is an
/// infinitely low bar).
///
/// The 4 – 6 kHz ramp is interpolated log-linearly in frequency
/// between the two printed anchors (60 dB at 4 kHz, 46.2 dB at 6 kHz),
/// matching the straight line the figure draws on its log axis.
pub fn min_ratio_db(frequency_hz: f64) -> f64 {
    match classify(frequency_hz) {
        MaskBand::BelowMask | MaskBand::AboveMask => f64::NEG_INFINITY,
        MaskBand::LowPlateau => FLOOR_LOW_PLATEAU_DB,
        MaskBand::MainPlateau => FLOOR_MAIN_PLATEAU_DB,
        MaskBand::HighPlateau => FLOOR_HIGH_PLATEAU_DB,
        MaskBand::Ramp => interp_log(
            RAMP_START_HZ,
            FLOOR_MAIN_PLATEAU_DB,
            RAMP_END_HZ,
            FLOOR_HIGH_PLATEAU_DB,
            frequency_hz,
        ),
    }
}

/// Evaluate a measured `(frequency_hz, ratio_db)` pair against the
/// Figure 15/G.722 mask.
///
/// Returns the [`MaskBand`] the frequency falls into and a `bool` that
/// is `true` when the measured signal-to-total distortion ratio sits at
/// or above the printed floor for that frequency. The mask only prints
/// a floor — more signal-to-distortion than required is always
/// admissible. Outside the printed span (`BelowMask` / `AboveMask`) the
/// result is `true`: no constraint applies. A NaN ratio fails every
/// in-mask band.
pub fn evaluate(frequency_hz: f64, ratio_db: f64) -> (MaskBand, bool) {
    let band = classify(frequency_hz);
    let ok = match band {
        MaskBand::BelowMask | MaskBand::AboveMask => true,
        _ => ratio_db >= min_ratio_db(frequency_hz),
    };
    (band, ok)
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
    // The printed frequency anchors are strictly increasing along the
    // Figure 15/G.722 abscissa.
    assert!(PASSBAND_LOW_HZ < PLATEAU_LOW_KNEE_HZ);
    assert!(PLATEAU_LOW_KNEE_HZ < RAMP_START_HZ);
    assert!(RAMP_START_HZ < RAMP_END_HZ);
    assert!(RAMP_END_HZ < PASSBAND_HIGH_HZ);
    // The 60 dB main plateau is the global maximum; both side plateaus
    // sit below it.
    assert!(FLOOR_LOW_PLATEAU_DB < FLOOR_MAIN_PLATEAU_DB);
    assert!(FLOOR_HIGH_PLATEAU_DB < FLOOR_MAIN_PLATEAU_DB);
    // The right (high-frequency) plateau is the lowest floor printed.
    assert!(FLOOR_HIGH_PLATEAU_DB < FLOOR_LOW_PLATEAU_DB);
    // The mask walls coincide with the clause 2.4.1 nominal passband.
    assert!(PASSBAND_LOW_HZ == NOMINAL_PASSBAND_LOW_HZ as f64);
    assert!(PASSBAND_HIGH_HZ == NOMINAL_PASSBAND_HIGH_HZ as f64);
    // The 60 dB plateau ends at the QMF band-split frequency.
    assert!(RAMP_START_HZ == (SUBBAND_SAMPLE_CLOCK_HZ / 2) as f64);
};

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transmission::{
        signal_to_distortion, IDLE_NOISE_BAND_LOW_HZ, IDLE_NOISE_NARROWBAND_HIGH_HZ,
    };

    #[test]
    fn test_level_matches_clause_2_5_6() {
        // Clause 2.5.6 p. 14: "with a sine wave signal at a level of
        // −10 dBm0". Same nominal test level as clauses 2.4.2 / 2.4.3.
        assert_eq!(TEST_LEVEL_DBM0, -10.0);
    }

    #[test]
    fn anchors_match_figure_15() {
        // Figure 15/G.722 (p. 14) labels 0.050 / 0.100 / 4 / 6 / 7 kHz
        // on its frequency axis and 46.2 / 50 / 60 dB on its ratio axis.
        assert_eq!(PASSBAND_LOW_HZ, 50.0);
        assert_eq!(PLATEAU_LOW_KNEE_HZ, 100.0);
        assert_eq!(RAMP_START_HZ, 4_000.0);
        assert_eq!(RAMP_END_HZ, 6_000.0);
        assert_eq!(PASSBAND_HIGH_HZ, 7_000.0);
        assert_eq!(FLOOR_LOW_PLATEAU_DB, 50.0);
        assert_eq!(FLOOR_MAIN_PLATEAU_DB, 60.0);
        assert_eq!(FLOOR_HIGH_PLATEAU_DB, 46.2);
    }

    #[test]
    fn classify_below_left_wall_is_below_mask() {
        assert_eq!(classify(49.999), MaskBand::BelowMask);
        assert_eq!(classify(0.0), MaskBand::BelowMask);
        assert_eq!(classify(-1.0), MaskBand::BelowMask);
        assert_eq!(classify(f64::NAN), MaskBand::BelowMask);
        assert_eq!(classify(f64::NEG_INFINITY), MaskBand::BelowMask);
        assert_eq!(classify(f64::INFINITY), MaskBand::BelowMask);
    }

    #[test]
    fn classify_low_plateau_band() {
        // 50 Hz left wall belongs to the mask; band runs up to (but not
        // including) the 100 Hz knee, which the stricter 60 dB plateau
        // owns.
        assert_eq!(classify(50.0), MaskBand::LowPlateau);
        assert_eq!(classify(75.0), MaskBand::LowPlateau);
        assert_eq!(classify(99.999), MaskBand::LowPlateau);
    }

    #[test]
    fn classify_main_plateau_band() {
        // 100 Hz knee belongs to the 60 dB plateau (stricter), as does
        // the 4 kHz ramp start (the ramp foot at 4 kHz is also 60 dB).
        assert_eq!(classify(100.0), MaskBand::MainPlateau);
        assert_eq!(classify(1_000.0), MaskBand::MainPlateau);
        assert_eq!(classify(4_000.0), MaskBand::MainPlateau);
    }

    #[test]
    fn classify_ramp_band() {
        // Just above 4 kHz through the 6 kHz ramp end, which is
        // assigned to the ramp (continuous with the 46.2 dB plateau).
        assert_eq!(classify(4_000.1), MaskBand::Ramp);
        assert_eq!(classify(5_000.0), MaskBand::Ramp);
        assert_eq!(classify(6_000.0), MaskBand::Ramp);
    }

    #[test]
    fn classify_high_plateau_band() {
        // Above the 6 kHz ramp end through the 7 kHz right wall.
        assert_eq!(classify(6_000.1), MaskBand::HighPlateau);
        assert_eq!(classify(6_500.0), MaskBand::HighPlateau);
        assert_eq!(classify(7_000.0), MaskBand::HighPlateau);
    }

    #[test]
    fn classify_above_right_wall_is_above_mask() {
        assert_eq!(classify(7_000.1), MaskBand::AboveMask);
        assert_eq!(classify(8_000.0), MaskBand::AboveMask);
        assert_eq!(classify(20_000.0), MaskBand::AboveMask);
    }

    #[test]
    fn floor_on_each_plateau_is_the_printed_gridline() {
        assert_eq!(min_ratio_db(50.0), 50.0);
        assert_eq!(min_ratio_db(75.0), 50.0);
        assert_eq!(min_ratio_db(100.0), 60.0);
        assert_eq!(min_ratio_db(1_000.0), 60.0);
        assert_eq!(min_ratio_db(4_000.0), 60.0);
        assert_eq!(min_ratio_db(6_000.0), 46.2);
        assert_eq!(min_ratio_db(6_500.0), 46.2);
        assert_eq!(min_ratio_db(7_000.0), 46.2);
    }

    #[test]
    fn floor_on_ramp_endpoints_matches_the_plateaus() {
        // The ramp is anchored at the two printed gridlines.
        assert!((min_ratio_db(4_000.0) - 60.0).abs() < 1e-9);
        // Approaching 4 kHz from the ramp side converges on 60 dB.
        let just_above = min_ratio_db(4_000.0 + 1e-6);
        assert!(
            (just_above - 60.0).abs() < 1e-3,
            "ramp start drift: {just_above}"
        );
        // The ramp foot at 6 kHz equals the right plateau.
        assert!((min_ratio_db(6_000.0) - 46.2).abs() < 1e-9);
    }

    #[test]
    fn ramp_is_log_linear_between_the_anchors() {
        // The geometric-mean frequency of 4 kHz and 6 kHz sits at the
        // arithmetic midpoint of the two dB anchors (the defining
        // property of a straight line on a log axis).
        let f_mid = (4_000.0_f64 * 6_000.0).sqrt();
        let db_mid = min_ratio_db(f_mid);
        let expected = (60.0 + 46.2) / 2.0;
        assert!(
            (db_mid - expected).abs() < 1e-9,
            "log-mid floor {db_mid} != arithmetic-mid dB {expected}"
        );
    }

    #[test]
    fn ramp_is_strictly_decreasing() {
        // Across 4 – 6 kHz the floor falls monotonically from 60 to
        // 46.2 dB.
        let mut f = 4_000.0;
        let mut prev = min_ratio_db(f);
        while f < 6_000.0 {
            f += 50.0;
            let cur = min_ratio_db(f.min(6_000.0));
            assert!(
                cur <= prev,
                "ramp not decreasing at {f} Hz ({cur} > {prev})"
            );
            prev = cur;
        }
    }

    #[test]
    fn floor_is_neg_infinity_outside_the_mask() {
        assert_eq!(min_ratio_db(49.999), f64::NEG_INFINITY);
        assert_eq!(min_ratio_db(0.0), f64::NEG_INFINITY);
        assert_eq!(min_ratio_db(7_000.1), f64::NEG_INFINITY);
        assert_eq!(min_ratio_db(20_000.0), f64::NEG_INFINITY);
        assert_eq!(min_ratio_db(f64::NAN), f64::NEG_INFINITY);
    }

    #[test]
    fn main_plateau_is_the_global_maximum() {
        // Sweep the whole printed span on a fine grid; nowhere does the
        // floor exceed the 60 dB main plateau.
        let mut f = PASSBAND_LOW_HZ;
        while f <= PASSBAND_HIGH_HZ {
            assert!(
                min_ratio_db(f) <= FLOOR_MAIN_PLATEAU_DB + 1e-9,
                "floor at {f} Hz exceeds the 60 dB main plateau"
            );
            f += 25.0;
        }
    }

    #[test]
    fn floor_has_no_upward_step_after_the_main_plateau() {
        // From the 60 dB plateau onward the floor never rises again:
        // ramp down to 46.2 dB then flat. Sweep 100 Hz → 7 kHz and
        // confirm monotone non-increasing.
        let mut f = PLATEAU_LOW_KNEE_HZ;
        let mut prev = min_ratio_db(f);
        while f <= PASSBAND_HIGH_HZ {
            let cur = min_ratio_db(f);
            assert!(
                cur <= prev + 1e-9,
                "floor rose at {f} Hz ({cur} > {prev}) past the main plateau"
            );
            prev = cur;
            f += 25.0;
        }
    }

    #[test]
    fn evaluate_floor_boundary_semantics() {
        // Exactly at the printed floor the measurement passes; 0.01 dB
        // under it fails. Spot-check one point per in-mask band.
        for f in [60.0, 1_000.0, 5_000.0, 6_500.0] {
            let floor = min_ratio_db(f);
            let (_, ok) = evaluate(f, floor);
            assert!(ok, "at {f} Hz: exact floor must pass");
            let (_, ok) = evaluate(f, floor - 0.01);
            assert!(!ok, "at {f} Hz: under-floor must fail");
            let (_, ok) = evaluate(f, floor + 20.0);
            assert!(ok, "at {f} Hz: headroom must pass");
        }
    }

    #[test]
    fn evaluate_outside_mask_always_passes() {
        let (band, ok) = evaluate(40.0, 0.0);
        assert_eq!(band, MaskBand::BelowMask);
        assert!(ok);
        let (band, ok) = evaluate(8_000.0, 0.0);
        assert_eq!(band, MaskBand::AboveMask);
        assert!(ok);
    }

    #[test]
    fn evaluate_nan_ratio_fails_in_mask() {
        let (band, ok) = evaluate(1_000.0, f64::NAN);
        assert_eq!(band, MaskBand::MainPlateau);
        assert!(!ok);
        let (band, ok) = evaluate(5_000.0, f64::NAN);
        assert_eq!(band, MaskBand::Ramp);
        assert!(!ok);
    }

    #[test]
    fn walls_align_with_the_nominal_passband() {
        // Figure 15's 50 Hz / 7 kHz walls are exactly the clause 2.4.1
        // nominal 3-dB passband edges.
        assert_eq!(PASSBAND_LOW_HZ as u32, NOMINAL_PASSBAND_LOW_HZ);
        assert_eq!(PASSBAND_HIGH_HZ as u32, NOMINAL_PASSBAND_HIGH_HZ);
    }

    #[test]
    fn main_plateau_edge_is_the_qmf_band_split() {
        // The 60 dB plateau ends where the QMF splits the band (4 kHz =
        // half the 8 kHz sub-band sample clock); above it the floor
        // falls toward the coarser higher-sub-band budget.
        assert_eq!(RAMP_START_HZ as u32, SUBBAND_SAMPLE_CLOCK_HZ / 2);
    }

    #[test]
    fn high_plateau_floor_brackets_the_clause_2_5_5_six_khz_plateau() {
        // Clause 2.5.6's 6 – 7 kHz floor (46.2 dB) sits just below the
        // clause 2.5.5 "about 6 kHz" curve's level-swept plateau
        // (50 dB) — both describe the higher-sub-band distortion budget
        // near the band edge, the frequency-swept one being the looser
        // of the two.
        const _: () = assert!(FLOOR_HIGH_PLATEAU_DB < signal_to_distortion::PLATEAU_TONE_HIGH_DB);
    }

    #[test]
    fn measurement_window_is_the_familiar_unweighted_band() {
        // Clause 2.5.6 measures "unweighted in the frequency range 50
        // to 7000 Hz" — the same window clauses 2.4.4 / 2.5.5 use, and
        // exactly the mask's two walls.
        assert_eq!(IDLE_NOISE_BAND_LOW_HZ, 50);
        assert_eq!(IDLE_NOISE_NARROWBAND_HIGH_HZ, 7_000);
        assert_eq!(PASSBAND_LOW_HZ as u32, IDLE_NOISE_BAND_LOW_HZ);
        assert_eq!(PASSBAND_HIGH_HZ as u32, IDLE_NOISE_NARROWBAND_HIGH_HZ);
    }
}

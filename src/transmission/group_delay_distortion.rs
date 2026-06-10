//! Clause 2.5.3 / Figure 13/G.722 — group-delay-distortion versus
//! frequency mask.
//!
//! Clause 2.5.3 (p. 13) of the staged ITU-T G.722 (11/88)
//! Recommendation requires the group-delay distortion of the audio
//! parts — "taking the minimum value of group delay as a reference" —
//! to satisfy the limits of the mask drawn in Figure 13/G.722 (p. 13,
//! "Group-delay distortion versus frequency"). Like the clause 2.5.1 /
//! 2.5.2 filter masks ([`super::anti_aliasing_filter`] /
//! [`super::reconstructing_filter`]), this is an audio-parts
//! requirement measured in the looped audio-to-audio configuration of
//! Figure 9b)/G.722 (p. 10; clause 2.5 p. 11).
//!
//! It is the *distortion* companion of clause 2.4.3's *absolute*
//! group-delay limit (already surfaced as
//! [`super::ABSOLUTE_GROUP_DELAY_MAX_MS`]): clause 2.4.3 caps the
//! minimum group delay itself at 4 ms, while Figure 13 caps how far
//! the group delay at any in-band frequency may exceed that minimum.
//!
//! Figure 13/G.722 prints the mask as a piecewise-constant staircase
//! on a log frequency axis with group-delay distortion in ms. The
//! printed gridlines on the distortion axis are 0.25 / 1 / 2 / 4 ms
//! and the labelled frequencies are 0.050 / 0.100 / 0.300 / 4 / 6.4 /
//! 7 kHz. The staircase reads:
//!
//! | Frequency band      | Max group-delay distortion (ms) |
//! | ------------------- | ------------------------------- |
//! | 0 Hz to 50 Hz       | (out-of-band)                   |
//! | 50 Hz to 100 Hz     | 4                               |
//! | 100 Hz to 300 Hz    | 1                               |
//! | 300 Hz to 4 kHz     | 0.25                            |
//! | 4 kHz to 6.4 kHz    | 1                               |
//! | 6.4 kHz to 7 kHz    | 2                               |
//! | above 7 kHz         | (out-of-band; right wall)       |
//!
//! Unlike the attenuation masks there is only an upper bound: the
//! reference is the *minimum* group delay across the band, so the
//! distortion is non-negative by construction and the printed mask is
//! a ceiling.
//!
//! Two structural alignments with the rest of clause 2 are worth
//! pinning. First, the mask's right wall sits at 7 kHz — the clause
//! 2.4.1 nominal 3-dB passband high edge
//! ([`super::NOMINAL_PASSBAND_HIGH_HZ`]) — and the 100 Hz / 6.4 kHz
//! breakpoints are the same in-band anchors Figures 10 / 11 / 12 use.
//! Second, the tight 0.25 ms core ends at 4 kHz, which is the QMF
//! band-split frequency (the lower sub-band of clause 1.4.1 spans
//! 0 – 4000 Hz; 4 kHz = [`super::SUBBAND_SAMPLE_CLOCK_HZ`] / 2) — the
//! shoulder above it is where the band-split filtering dominates the
//! delay budget.
//!
//! ## Provenance
//!
//! Every breakpoint and ms value below is transcribed from the
//! printed Figure 13/G.722 of
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf` (page 13). No external
//! reference implementation was consulted.

use crate::transmission::{
    NOMINAL_PASSBAND_HIGH_HZ, NOMINAL_PASSBAND_LOW_HZ, SUBBAND_SAMPLE_CLOCK_HZ,
};

// -----------------------------------------------------------------------
// Mask breakpoints (Figure 13/G.722 page 13)
// -----------------------------------------------------------------------

/// Low edge of the 1 ms low shoulder of Figure 13/G.722 (p. 13). The
/// printed 0.100 kHz label; below it (down to the 50 Hz mask anchor)
/// the ceiling relaxes to the 4 ms low-transition value. This is the
/// same 100 Hz in-band anchor Figures 10 / 11 / 12 print.
pub const LOW_SHOULDER_LOW_HZ: u32 = 100;

/// Low edge of the tight 0.25 ms core of Figure 13/G.722 (p. 13) —
/// the printed 0.300 kHz label.
pub const CORE_LOW_HZ: u32 = 300;

/// High edge of the tight 0.25 ms core of Figure 13/G.722 (p. 13) —
/// the printed 4 kHz label. This is the QMF band-split frequency: the
/// lower sub-band of clause 1.4.1 spans 0 – 4000 Hz, i.e. 4 kHz =
/// [`SUBBAND_SAMPLE_CLOCK_HZ`] / 2.
pub const CORE_HIGH_HZ: u32 = 4_000;

/// High edge of the 1 ms high shoulder of Figure 13/G.722 (p. 13) —
/// the printed 6.4 kHz label, shared with the
/// `PASSBAND_TIGHT_HIGH_HZ` anchor of Figures 10 / 11 / 12.
pub const HIGH_SHOULDER_HIGH_HZ: u32 = 6_400;

/// Right wall of the Figure 13/G.722 mask (p. 13) — the printed 7 kHz
/// label, matching the clause 2.4.1 nominal 3-dB passband high edge
/// ([`NOMINAL_PASSBAND_HIGH_HZ`]). Above it no mask is printed.
pub const MASK_HIGH_EDGE_HZ: u32 = NOMINAL_PASSBAND_HIGH_HZ;

// -----------------------------------------------------------------------
// Mask ceilings (Figure 13/G.722 page 13, distortion axis)
// -----------------------------------------------------------------------

/// Ceiling on the 50 – 100 Hz low-transition strip (Figure 13/G.722
/// p. 13): 4 ms — the topmost printed gridline of the distortion
/// axis. Numerically this is the same 4 ms the clause 2.4.3 absolute
/// group-delay limit prints ([`super::ABSOLUTE_GROUP_DELAY_MAX_MS`]).
pub const LOW_TRANSITION_MAX_MS: f64 = 4.0;

/// Ceiling on the two 1 ms shoulders — 100 – 300 Hz and 4 – 6.4 kHz
/// (Figure 13/G.722 p. 13).
pub const SHOULDER_MAX_MS: f64 = 1.0;

/// Ceiling on the tight 300 Hz – 4 kHz core (Figure 13/G.722 p. 13):
/// 0.25 ms — the lowest printed gridline of the distortion axis and
/// the global minimum of the staircase.
pub const CORE_MAX_MS: f64 = 0.25;

/// Ceiling on the 6.4 – 7 kHz high-transition strip (Figure 13/G.722
/// p. 13): 2 ms.
pub const HIGH_TRANSITION_MAX_MS: f64 = 2.0;

// -----------------------------------------------------------------------
// Mask evaluation
// -----------------------------------------------------------------------

/// Outcome of classifying a frequency against the Figure 13/G.722
/// group-delay-distortion staircase.
///
/// The variants follow the printed piecewise-constant structure of
/// Figure 13: a 4 ms low transition, a 1 ms low shoulder, the tight
/// 0.25 ms core, a 1 ms high shoulder and a 2 ms high transition,
/// with no constraint printed outside the 50 Hz – 7 kHz span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskBand {
    /// Frequency is below the 50 Hz mask anchor — outside the
    /// audio parts' normative coverage. No constraint printed.
    BelowMask,
    /// 50 Hz – 100 Hz low-transition strip: ceiling
    /// [`LOW_TRANSITION_MAX_MS`] (4 ms).
    LowTransition,
    /// 100 Hz – 300 Hz low shoulder: ceiling [`SHOULDER_MAX_MS`]
    /// (1 ms).
    LowShoulder,
    /// 300 Hz – 4 kHz tight core: ceiling [`CORE_MAX_MS`] (0.25 ms).
    Core,
    /// 4 kHz – 6.4 kHz high shoulder: ceiling [`SHOULDER_MAX_MS`]
    /// (1 ms).
    HighShoulder,
    /// 6.4 kHz – 7 kHz high-transition strip: ceiling
    /// [`HIGH_TRANSITION_MAX_MS`] (2 ms).
    HighTransition,
    /// Frequency is above the [`MASK_HIGH_EDGE_HZ`] right wall —
    /// outside the audio parts' normative coverage. No constraint
    /// printed.
    AboveMask,
}

/// Classify a frequency into the [`MaskBand`] it belongs to.
///
/// Breakpoint ownership follows the same closed-interval convention
/// as the sibling attenuation masks: at every printed breakpoint the
/// *stricter* (lower-ceiling) band owns the point. So 100 Hz / 300 Hz
/// belong to the band on their right (the staircase steps *down*
/// going up in frequency there) while 4 kHz / 6.4 kHz / 7 kHz belong
/// to the band on their left (the staircase steps *up* there).
pub fn classify(frequency_hz: f64) -> MaskBand {
    // The 50 Hz mask anchor is the printed 0.050 kHz label —
    // identical to the clause 2.4.1 nominal passband low edge.
    let f_low_anchor = NOMINAL_PASSBAND_LOW_HZ as f64;
    if !frequency_hz.is_finite() || frequency_hz < f_low_anchor {
        return MaskBand::BelowMask;
    }
    if frequency_hz < LOW_SHOULDER_LOW_HZ as f64 {
        return MaskBand::LowTransition;
    }
    if frequency_hz < CORE_LOW_HZ as f64 {
        return MaskBand::LowShoulder;
    }
    if frequency_hz <= CORE_HIGH_HZ as f64 {
        return MaskBand::Core;
    }
    if frequency_hz <= HIGH_SHOULDER_HIGH_HZ as f64 {
        return MaskBand::HighShoulder;
    }
    if frequency_hz <= MASK_HIGH_EDGE_HZ as f64 {
        return MaskBand::HighTransition;
    }
    MaskBand::AboveMask
}

/// Maximum admissible group-delay distortion (in ms, relative to the
/// minimum group delay per clause 2.5.3) at `frequency_hz`. Returns
/// `f64::INFINITY` outside the printed 50 Hz – 7 kHz mask span (no
/// constraint printed there).
pub fn max_distortion_ms(frequency_hz: f64) -> f64 {
    match classify(frequency_hz) {
        MaskBand::BelowMask | MaskBand::AboveMask => f64::INFINITY,
        MaskBand::LowTransition => LOW_TRANSITION_MAX_MS,
        MaskBand::LowShoulder | MaskBand::HighShoulder => SHOULDER_MAX_MS,
        MaskBand::Core => CORE_MAX_MS,
        MaskBand::HighTransition => HIGH_TRANSITION_MAX_MS,
    }
}

/// Evaluate a measured `(frequency_hz, distortion_ms)` pair against
/// the Figure 13/G.722 mask.
///
/// Returns the [`MaskBand`] the frequency falls into and a `bool`
/// that is `true` when the measured group-delay distortion sits at or
/// under the printed ceiling for that band. The mask only prints a
/// ceiling — the distortion is non-negative by construction (the
/// clause 2.5.3 reference is the *minimum* group delay), so no lower
/// bound is checked. Outside the printed span (`BelowMask` /
/// `AboveMask`) the result is `true`: no constraint applies. A NaN
/// distortion measurement fails every in-mask band.
pub fn evaluate(frequency_hz: f64, distortion_ms: f64) -> (MaskBand, bool) {
    let band = classify(frequency_hz);
    let ok = match band {
        MaskBand::BelowMask | MaskBand::AboveMask => true,
        _ => distortion_ms <= max_distortion_ms(frequency_hz),
    };
    (band, ok)
}

// -----------------------------------------------------------------------
// Compile-time invariants
// -----------------------------------------------------------------------

const _: () = {
    // Breakpoints must be strictly increasing across the printed
    // Figure 13/G.722 axis.
    assert!(NOMINAL_PASSBAND_LOW_HZ < LOW_SHOULDER_LOW_HZ);
    assert!(LOW_SHOULDER_LOW_HZ < CORE_LOW_HZ);
    assert!(CORE_LOW_HZ < CORE_HIGH_HZ);
    assert!(CORE_HIGH_HZ < HIGH_SHOULDER_HIGH_HZ);
    assert!(HIGH_SHOULDER_HIGH_HZ < MASK_HIGH_EDGE_HZ);
    // The right wall sits at the clause 2.4.1 nominal 3-dB passband
    // high edge.
    assert!(MASK_HIGH_EDGE_HZ == NOMINAL_PASSBAND_HIGH_HZ);
    // The tight core's high edge is the QMF band-split frequency
    // (clause 1.4.1's 0 – 4000 Hz lower sub-band; half the 8 kHz
    // sub-band sample clock).
    assert!(CORE_HIGH_HZ * 2 == SUBBAND_SAMPLE_CLOCK_HZ);
    // The staircase ceilings are ordered: core < shoulders < high
    // transition < low transition, matching the printed 0.25 / 1 / 2
    // / 4 ms gridlines.
    assert!(CORE_MAX_MS < SHOULDER_MAX_MS);
    assert!(SHOULDER_MAX_MS < HIGH_TRANSITION_MAX_MS);
    assert!(HIGH_TRANSITION_MAX_MS < LOW_TRANSITION_MAX_MS);
};

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transmission::{attenuation_distortion, ABSOLUTE_GROUP_DELAY_MAX_MS};

    #[test]
    fn breakpoints_match_figure_13() {
        // Figure 13/G.722 (p. 13) labels 0.050 / 0.100 / 0.300 / 4 /
        // 6.4 / 7 kHz on its log frequency axis.
        assert_eq!(NOMINAL_PASSBAND_LOW_HZ, 50);
        assert_eq!(LOW_SHOULDER_LOW_HZ, 100);
        assert_eq!(CORE_LOW_HZ, 300);
        assert_eq!(CORE_HIGH_HZ, 4_000);
        assert_eq!(HIGH_SHOULDER_HIGH_HZ, 6_400);
        assert_eq!(MASK_HIGH_EDGE_HZ, 7_000);
    }

    #[test]
    fn ceilings_match_figure_13() {
        // Figure 13/G.722 (p. 13) prints 0.25 / 1 / 2 / 4 ms on its
        // group-delay-distortion axis.
        assert_eq!(CORE_MAX_MS, 0.25);
        assert_eq!(SHOULDER_MAX_MS, 1.0);
        assert_eq!(HIGH_TRANSITION_MAX_MS, 2.0);
        assert_eq!(LOW_TRANSITION_MAX_MS, 4.0);
    }

    #[test]
    fn classify_below_low_anchor_is_below_mask() {
        assert_eq!(classify(0.0), MaskBand::BelowMask);
        assert_eq!(classify(10.0), MaskBand::BelowMask);
        assert_eq!(classify(49.999), MaskBand::BelowMask);
    }

    #[test]
    fn classify_low_transition_region() {
        // 50 Hz up to (not including) 100 Hz: the 4 ms strip.
        assert_eq!(classify(50.0), MaskBand::LowTransition);
        assert_eq!(classify(75.0), MaskBand::LowTransition);
        assert_eq!(classify(99.999), MaskBand::LowTransition);
    }

    #[test]
    fn classify_low_shoulder_region() {
        // 100 Hz up to (not including) 300 Hz: the 1 ms low shoulder.
        // The staircase steps down at 100 Hz, so the stricter band
        // owns the breakpoint.
        assert_eq!(classify(100.0), MaskBand::LowShoulder);
        assert_eq!(classify(200.0), MaskBand::LowShoulder);
        assert_eq!(classify(299.999), MaskBand::LowShoulder);
    }

    #[test]
    fn classify_core_region() {
        // 300 Hz to 4 kHz inclusive: the tight 0.25 ms core. Both
        // edges belong to the core (it is the strictest band of the
        // staircase). Anchor at 1000 Hz, the clause 2.3 nominal
        // reference frequency.
        assert_eq!(classify(300.0), MaskBand::Core);
        assert_eq!(classify(1_000.0), MaskBand::Core);
        assert_eq!(classify(4_000.0), MaskBand::Core);
    }

    #[test]
    fn classify_high_shoulder_region() {
        // Just past 4 kHz up to 6.4 kHz inclusive: the 1 ms high
        // shoulder (stricter than the 2 ms strip on its right, so it
        // owns the 6.4 kHz breakpoint).
        assert_eq!(classify(4_000.001), MaskBand::HighShoulder);
        assert_eq!(classify(5_000.0), MaskBand::HighShoulder);
        assert_eq!(classify(6_400.0), MaskBand::HighShoulder);
    }

    #[test]
    fn classify_high_transition_region() {
        // Just past 6.4 kHz up to 7 kHz inclusive: the 2 ms strip
        // (stricter than the open region above the right wall, so it
        // owns the 7 kHz breakpoint).
        assert_eq!(classify(6_400.001), MaskBand::HighTransition);
        assert_eq!(classify(6_800.0), MaskBand::HighTransition);
        assert_eq!(classify(7_000.0), MaskBand::HighTransition);
    }

    #[test]
    fn classify_above_mask_above_right_wall() {
        assert_eq!(classify(7_000.001), MaskBand::AboveMask);
        assert_eq!(classify(8_000.0), MaskBand::AboveMask);
        assert_eq!(classify(20_000.0), MaskBand::AboveMask);
    }

    #[test]
    fn evaluate_core_corridor() {
        // 0.2 ms at 1 kHz sits under the 0.25 ms core ceiling.
        let (band, ok) = evaluate(1_000.0, 0.2);
        assert_eq!(band, MaskBand::Core);
        assert!(ok);
        // 0.26 ms breaks it.
        let (band, ok) = evaluate(1_000.0, 0.26);
        assert_eq!(band, MaskBand::Core);
        assert!(!ok);
        // Exactly at the printed ceiling the measurement passes (the
        // mask is a closed interval at the boundary).
        let (_, ok) = evaluate(1_000.0, 0.25);
        assert!(ok);
    }

    #[test]
    fn evaluate_low_transition_corridor() {
        // 3.5 ms at 75 Hz sits under the 4 ms ceiling.
        let (band, ok) = evaluate(75.0, 3.5);
        assert_eq!(band, MaskBand::LowTransition);
        assert!(ok);
        // 4.01 ms breaks it; exactly 4 ms passes.
        let (_, ok) = evaluate(75.0, 4.01);
        assert!(!ok);
        let (_, ok) = evaluate(75.0, 4.0);
        assert!(ok);
    }

    #[test]
    fn evaluate_shoulder_corridors() {
        // Both shoulders share the printed 1 ms ceiling.
        for f in [200.0, 5_000.0] {
            let (_, ok) = evaluate(f, 1.0);
            assert!(ok, "1 ms at {f} Hz must pass the shoulder ceiling");
            let (_, ok) = evaluate(f, 1.01);
            assert!(!ok, "1.01 ms at {f} Hz must fail the shoulder ceiling");
            // A value admissible in the low transition (e.g. 2 ms)
            // fails on the shoulders.
            let (_, ok) = evaluate(f, 2.0);
            assert!(!ok, "2 ms at {f} Hz must fail the shoulder ceiling");
        }
    }

    #[test]
    fn evaluate_high_transition_corridor() {
        // 2 ms at 6.8 kHz exactly meets the printed ceiling.
        let (band, ok) = evaluate(6_800.0, 2.0);
        assert_eq!(band, MaskBand::HighTransition);
        assert!(ok);
        // 2.01 ms breaks it.
        let (_, ok) = evaluate(6_800.0, 2.01);
        assert!(!ok);
    }

    #[test]
    fn evaluate_outside_mask_always_passes() {
        // No constraint is printed below 50 Hz or above 7 kHz.
        let (band, ok) = evaluate(10.0, 100.0);
        assert_eq!(band, MaskBand::BelowMask);
        assert!(ok);
        let (band, ok) = evaluate(8_000.0, 100.0);
        assert_eq!(band, MaskBand::AboveMask);
        assert!(ok);
    }

    #[test]
    fn evaluate_zero_distortion_passes_everywhere() {
        // The frequency at which the group delay attains its minimum
        // measures 0 ms distortion by definition (clause 2.5.3 takes
        // the minimum as the reference) — it must pass in every band.
        let mut f = 10.0_f64;
        while f < 20_000.0 {
            let (_, ok) = evaluate(f, 0.0);
            assert!(ok, "0 ms distortion at {f} Hz must pass");
            f += 50.0;
        }
    }

    #[test]
    fn evaluate_nan_distortion_fails_in_mask() {
        let (band, ok) = evaluate(1_000.0, f64::NAN);
        assert_eq!(band, MaskBand::Core);
        assert!(!ok);
    }

    #[test]
    fn nan_and_negative_frequency_handled_safely() {
        assert_eq!(classify(f64::NAN), MaskBand::BelowMask);
        assert_eq!(classify(-100.0), MaskBand::BelowMask);
        assert_eq!(max_distortion_ms(f64::NAN), f64::INFINITY);
    }

    #[test]
    fn max_distortion_ms_staircase_shape() {
        // Sample each printed step at an interior frequency.
        assert_eq!(max_distortion_ms(75.0), 4.0);
        assert_eq!(max_distortion_ms(200.0), 1.0);
        assert_eq!(max_distortion_ms(1_000.0), 0.25);
        assert_eq!(max_distortion_ms(5_000.0), 1.0);
        assert_eq!(max_distortion_ms(6_800.0), 2.0);
    }

    #[test]
    fn max_distortion_ms_infinite_outside_mask() {
        assert_eq!(max_distortion_ms(10.0), f64::INFINITY);
        assert_eq!(max_distortion_ms(49.999), f64::INFINITY);
        assert_eq!(max_distortion_ms(7_000.001), f64::INFINITY);
        assert_eq!(max_distortion_ms(20_000.0), f64::INFINITY);
    }

    #[test]
    fn stricter_band_owns_every_breakpoint() {
        // At each printed breakpoint the lower ceiling applies.
        assert_eq!(max_distortion_ms(100.0), SHOULDER_MAX_MS);
        assert_eq!(max_distortion_ms(300.0), CORE_MAX_MS);
        assert_eq!(max_distortion_ms(4_000.0), CORE_MAX_MS);
        assert_eq!(max_distortion_ms(6_400.0), SHOULDER_MAX_MS);
        assert_eq!(max_distortion_ms(7_000.0), HIGH_TRANSITION_MAX_MS);
        // ... and the 50 Hz mask anchor belongs to the mask (4 ms),
        // not to the unconstrained region below it.
        assert_eq!(max_distortion_ms(50.0), LOW_TRANSITION_MAX_MS);
    }

    #[test]
    fn core_is_global_minimum_of_staircase() {
        // The 0.25 ms core ceiling is the strictest value anywhere on
        // the printed mask: sweep on a 25 Hz grid.
        let mut f = 50.0_f64;
        while f <= 7_000.0 {
            assert!(
                max_distortion_ms(f) >= CORE_MAX_MS,
                "ceiling at {f} Hz dips under the core minimum"
            );
            f += 25.0;
        }
    }

    #[test]
    fn breakpoints_shared_with_attenuation_masks() {
        // The 100 Hz / 6.4 kHz / 7 kHz anchors are the same printed
        // in-band anchors as Figures 10 / 11 / 12. Pin them so a
        // future edit to either mask flags the drift.
        assert_eq!(LOW_SHOULDER_LOW_HZ, attenuation_distortion::PASSBAND_LOW_HZ);
        assert_eq!(
            HIGH_SHOULDER_HIGH_HZ,
            attenuation_distortion::PASSBAND_TIGHT_HIGH_HZ
        );
        assert_eq!(
            MASK_HIGH_EDGE_HZ,
            attenuation_distortion::PASSBAND_RELAXED_HIGH_HZ
        );
    }

    #[test]
    fn core_high_edge_is_qmf_band_split() {
        // 4 kHz is the QMF band-split frequency: the lower sub-band
        // of clause 1.4.1 spans 0 – 4000 Hz (half the 8 kHz sub-band
        // sample clock of clause 1.6).
        assert_eq!(CORE_HIGH_HZ * 2, SUBBAND_SAMPLE_CLOCK_HZ);
    }

    #[test]
    fn low_transition_ceiling_matches_absolute_group_delay_limit() {
        // Numerical alignment: the 4 ms top step of Figure 13 is the
        // same printed value as the clause 2.4.3 absolute group-delay
        // limit (the topmost gridline of the figure's distortion
        // axis).
        assert!((LOW_TRANSITION_MAX_MS - ABSOLUTE_GROUP_DELAY_MAX_MS).abs() < 1e-12);
    }

    #[test]
    fn nominal_reference_frequency_sits_in_core() {
        // The 1000 Hz nominal reference frequency of clause 2.3 falls
        // inside the tight 0.25 ms core.
        let (band, ok) = evaluate(1_000.0, 0.1);
        assert_eq!(band, MaskBand::Core);
        assert!(ok);
    }
}

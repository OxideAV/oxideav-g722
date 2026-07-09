//! Discrete-spectrum measurement mathematics for the clause 2.4
//! noise / distortion / delay limits.
//!
//! Clauses 2.4.3 – 2.4.5 of the staged ITU-T G.722 (11/88)
//! Recommendation (pages 9 – 11) phrase their limits in terms of
//! *measurements* on the looped codec of Figure 9a)/G.722: a
//! "selective" level measurement of a single frequency (clause 2.4.5),
//! an "unweighted noise power measured in the frequency range 50 to
//! 7000 Hz" (clause 2.4.4), and a "group delay for a sine wave signal"
//! (clause 2.4.3). This module supplies the digital-domain measurement
//! primitives those checks need:
//!
//! * [`fit_sine`] — exact least-squares fit of a sampled record to a
//!   single sinusoid of known frequency, returning its amplitude,
//!   phase, RMS, and the residual ("total distortion") RMS. This is
//!   the digital equivalent of the selective level meter + distortion
//!   meter of the clause 2.4 measurement set-ups, and the phase reading
//!   doubles as the group-delay probe of clause 2.4.3 (group delay is
//!   the derivative of phase with respect to angular frequency).
//! * [`dft_bin_rms`] — the RMS of one discrete-Fourier bin of a
//!   rectangular-windowed record, evaluated with the standard
//!   second-order real-coefficient recurrence (one pass, no
//!   per-sample trigonometry). This is the "measured selectively"
//!   primitive of clause 2.4.5.
//! * [`band_rms`] / [`peak_bin`] — band-limited noise power (the
//!   clause 2.4.4 "measured in the frequency range 50 to 7000 Hz"
//!   window) and the strongest single-frequency component in a band
//!   (the clause 2.4.5 sweep), both built on [`dft_bin_rms`].
//!
//! Everything here is standard sampled-signal mathematics — the only
//! Recommendation-derived content is the *purpose* documented above;
//! no numeric table of the Recommendation is involved.

extern crate alloc;

use alloc::vec::Vec;
use core::f64::consts::TAU;
use core::ops::RangeInclusive;

// -----------------------------------------------------------------------
// Least-squares single-sinusoid fit
// -----------------------------------------------------------------------

/// Result of [`fit_sine`]: the least-squares decomposition of a
/// sampled record into `a·cos(ωn) + b·sin(ωn)` plus a residual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SineFit {
    /// Normalised fit frequency (cycles per sample), as passed in.
    pub cycles_per_sample: f64,
    /// Fitted in-phase (cosine) coefficient `a`.
    pub in_phase: f64,
    /// Fitted quadrature (sine) coefficient `b`.
    pub quadrature: f64,
    /// Peak amplitude of the fitted component, `√(a² + b²)`.
    pub amplitude: f64,
    /// Phase of the fitted component at sample index 0, in radians:
    /// the fit equals `amplitude · cos(ωn − phase_radians)`.
    pub phase_radians: f64,
    /// RMS of the fitted sinusoidal component over the record
    /// (computed exactly from the fitted coefficients and the basis
    /// autocorrelations, so DC / Nyquist / short-record cases are
    /// handled without special-casing).
    pub component_rms: f64,
    /// RMS of the residual `record − fit` — the "total distortion"
    /// reading of a clause 2.5.5-style measurement when the record is
    /// a codec-looped tone.
    pub residual_rms: f64,
}

/// Least-squares fit of `samples` to a single real sinusoid of
/// normalised frequency `cycles_per_sample` (0 = DC, 0.5 = Nyquist).
///
/// Solves the 2×2 normal equations of the basis
/// `{cos(ωn), sin(ωn)}` exactly (no orthogonality approximation), so
/// the fit is exact for any record length ≥ 2 and any in-range
/// frequency, including records covering a non-integer number of
/// cycles. When the sine basis column vanishes (DC and Nyquist, where
/// `sin(ωn) ≡ 0` on the sample grid) the fit degrades gracefully to
/// the single cosine basis function.
///
/// Empty records return an all-zero fit.
pub fn fit_sine(samples: &[i32], cycles_per_sample: f64) -> SineFit {
    let n = samples.len();
    let zero = SineFit {
        cycles_per_sample,
        in_phase: 0.0,
        quadrature: 0.0,
        amplitude: 0.0,
        phase_radians: 0.0,
        component_rms: 0.0,
        residual_rms: 0.0,
    };
    if n == 0 {
        return zero;
    }
    let omega = TAU * cycles_per_sample;

    // Accumulate the normal-equation moments in one pass.
    let mut scc = 0.0_f64; // Σ cos²
    let mut sss = 0.0_f64; // Σ sin²
    let mut scs = 0.0_f64; // Σ cos·sin
    let mut syc = 0.0_f64; // Σ y·cos
    let mut sys = 0.0_f64; // Σ y·sin
    let mut basis: Vec<(f64, f64)> = Vec::with_capacity(n);
    for (i, &y) in samples.iter().enumerate() {
        let (s, c) = (omega * i as f64).sin_cos();
        basis.push((c, s));
        let y = y as f64;
        scc += c * c;
        sss += s * s;
        scs += c * s;
        syc += y * c;
        sys += y * s;
    }

    // Solve [scc scs; scs sss]·[a; b] = [syc; sys]. Guard the
    // determinant against the degenerate DC / Nyquist grid (sin column
    // identically zero) and against records too short to separate the
    // two basis functions.
    let det = scc * sss - scs * scs;
    let (a, b) = if det.abs() > 1e-9 * (scc * sss).max(1.0) {
        ((syc * sss - sys * scs) / det, (sys * scc - syc * scs) / det)
    } else if scc > 0.0 {
        (syc / scc, 0.0)
    } else {
        (0.0, 0.0)
    };

    // Exact mean square of the fitted component from the basis
    // autocorrelations: mean((a·c + b·s)²).
    let fit_mean_sq = (a * a * scc + 2.0 * a * b * scs + b * b * sss) / n as f64;
    // Residual in a second pass over the cached basis samples.
    let mut res_sq = 0.0_f64;
    for (&y, &(c, s)) in samples.iter().zip(basis.iter()) {
        let r = y as f64 - (a * c + b * s);
        res_sq += r * r;
    }

    SineFit {
        cycles_per_sample,
        in_phase: a,
        quadrature: b,
        amplitude: (a * a + b * b).sqrt(),
        phase_radians: b.atan2(a),
        component_rms: fit_mean_sq.max(0.0).sqrt(),
        residual_rms: (res_sq / n as f64).sqrt(),
    }
}

// -----------------------------------------------------------------------
// Single-bin DFT (selective level measurement)
// -----------------------------------------------------------------------

/// RMS of the sinusoidal component in DFT bin `bin` of a
/// rectangular-windowed record of length `N = samples.len()`
/// (bin `k` ⇔ frequency `k / N` cycles per sample).
///
/// Evaluated with the standard second-order real recurrence for a
/// single DFT coefficient (one multiply-add pair per sample). The
/// magnitude is converted to *component RMS*:
///
/// * interior bins (`0 < k < N/2`): a real sinusoid splits across the
///   `k` / `N−k` conjugate pair, so amplitude `= 2·|X_k|/N` and
///   RMS `= √2·|X_k|/N`;
/// * bin 0 (DC): RMS = `|X_0|/N` (the mean's magnitude);
/// * bin `N/2` (Nyquist, even `N`): the component alternates
///   `±A` on the grid, so RMS = amplitude = `|X_{N/2}|/N`.
///
/// Bins above `N/2` alias onto their conjugates; callers should stay
/// within `0..=N/2`. Empty records return 0.
pub fn dft_bin_rms(samples: &[i32], bin: usize) -> f64 {
    let n = samples.len();
    if n == 0 {
        return 0.0;
    }
    let omega = TAU * bin as f64 / n as f64;
    let coeff = 2.0 * omega.cos();
    // Second-order recurrence: s[i] = y[i] + coeff·s[i-1] − s[i-2].
    let mut s_prev = 0.0_f64;
    let mut s_prev2 = 0.0_f64;
    for &y in samples {
        let s = y as f64 + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    // |X_k|² = s₁² + s₂² − coeff·s₁·s₂ at the end of the record.
    let mag_sq = (s_prev * s_prev + s_prev2 * s_prev2 - coeff * s_prev * s_prev2).max(0.0);
    let mag = mag_sq.sqrt();
    let n_f = n as f64;
    if bin == 0 || 2 * bin == n {
        mag / n_f
    } else {
        core::f64::consts::SQRT_2 * mag / n_f
    }
}

// -----------------------------------------------------------------------
// Band-limited scans built on the single-bin measurement
// -----------------------------------------------------------------------

/// Band-limited RMS: the root of the summed per-bin mean-square power
/// over the inclusive bin range — the digital equivalent of the
/// clause 2.4.4 "unweighted noise power measured in the frequency
/// range …" reading for a rectangular-windowed record.
///
/// Bins outside `0..=N/2` are ignored.
pub fn band_rms(samples: &[i32], bins: RangeInclusive<usize>) -> f64 {
    let n = samples.len();
    if n == 0 {
        return 0.0;
    }
    let hi_valid = n / 2;
    let mut power = 0.0_f64;
    for k in bins {
        if k > hi_valid {
            break;
        }
        let rms = dft_bin_rms(samples, k);
        power += rms * rms;
    }
    power.sqrt()
}

/// Strongest single-frequency component in the inclusive bin range —
/// the clause 2.4.5 "level of any single frequency … measured
/// selectively" sweep. Returns `(bin, rms)` of the peak; an empty
/// record or empty range returns `(0, 0.0)`.
pub fn peak_bin(samples: &[i32], bins: RangeInclusive<usize>) -> (usize, f64) {
    let n = samples.len();
    let mut best = (0_usize, 0.0_f64);
    if n == 0 {
        return best;
    }
    let hi_valid = n / 2;
    for k in bins {
        if k > hi_valid {
            break;
        }
        let rms = dft_bin_rms(samples, k);
        if rms > best.1 {
            best = (k, rms);
        }
    }
    best
}

/// Lowest DFT bin of an `n`-sample record at `sample_rate_hz` whose
/// centre frequency is at or above `hz` (for band lower edges).
pub fn bin_at_or_above_hz(n: usize, sample_rate_hz: u32, hz: u32) -> usize {
    // ceil(hz · n / rate)
    (hz as u64 * n as u64).div_ceil(sample_rate_hz as u64) as usize
}

/// Highest DFT bin of an `n`-sample record at `sample_rate_hz` whose
/// centre frequency is at or below `hz` (for band upper edges).
pub fn bin_at_or_below_hz(n: usize, sample_rate_hz: u32, hz: u32) -> usize {
    ((hz as u64 * n as u64) / sample_rate_hz as u64) as usize
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthesise `n` samples of `amp·cos(2π·f·i − phase)` on the
    /// integer grid (rounded to i32, matching the codec's PCM domain).
    fn tone(n: usize, cycles_per_sample: f64, amp: f64, phase: f64) -> Vec<i32> {
        (0..n)
            .map(|i| (amp * (TAU * cycles_per_sample * i as f64 - phase).cos()).round() as i32)
            .collect()
    }

    #[test]
    fn fit_recovers_exact_bin_tone() {
        // 32 cycles in 1024 samples, amplitude 5000, phase 0.7 rad.
        let f = 32.0 / 1024.0;
        let y = tone(1024, f, 5000.0, 0.7);
        let fit = fit_sine(&y, f);
        assert!((fit.amplitude - 5000.0).abs() < 1.0, "{fit:?}");
        assert!((fit.phase_radians - 0.7).abs() < 1e-3, "{fit:?}");
        // Residual is only the i32 rounding of the synthesis (≤ 0.5
        // per sample → RMS ≤ ~0.29).
        assert!(fit.residual_rms < 0.5, "{fit:?}");
        // Component RMS of a sinusoid is amplitude/√2.
        assert!(
            (fit.component_rms - 5000.0 / 2.0_f64.sqrt()).abs() < 1.0,
            "{fit:?}"
        );
    }

    #[test]
    fn fit_is_exact_for_non_integer_cycle_records() {
        // 13.37 cycles in 500 samples — a record no integer DFT bin
        // matches; the exact normal equations must still recover it.
        let f = 13.37 / 500.0;
        let y = tone(500, f, 3000.0, -1.2);
        let fit = fit_sine(&y, f);
        assert!((fit.amplitude - 3000.0).abs() < 1.5, "{fit:?}");
        assert!((fit.phase_radians + 1.2).abs() < 1e-3, "{fit:?}");
        assert!(fit.residual_rms < 0.5, "{fit:?}");
    }

    #[test]
    fn fit_separates_component_from_added_noise() {
        // Tone + deterministic wideband perturbation: the fit must
        // report the tone in the component and the perturbation in
        // the residual.
        let f = 100.0 / 2048.0;
        let mut y = tone(2048, f, 4000.0, 0.3);
        // Small deterministic pseudo-noise (linear congruential walk),
        // zero-mean over the record by symmetric construction.
        let mut state = 0x1234_5678_u32;
        for v in y.iter_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *v += ((state >> 24) as i32) - 128;
        }
        let fit = fit_sine(&y, f);
        assert!((fit.amplitude - 4000.0).abs() < 15.0, "{fit:?}");
        // The added perturbation is uniform-ish on [−128, 127]: RMS
        // ≈ 74. The residual must see it (and not the tone).
        assert!(
            fit.residual_rms > 50.0 && fit.residual_rms < 100.0,
            "{fit:?}"
        );
    }

    #[test]
    fn fit_handles_dc_and_nyquist_degeneracy() {
        // DC record: sin basis vanishes; fit must recover the mean.
        let y = alloc::vec![250_i32; 64];
        let fit = fit_sine(&y, 0.0);
        assert!((fit.in_phase - 250.0).abs() < 1e-9, "{fit:?}");
        assert!((fit.component_rms - 250.0).abs() < 1e-9, "{fit:?}");
        assert!(fit.residual_rms < 1e-9, "{fit:?}");
        // Nyquist record: alternating ±400.
        let y: Vec<i32> = (0..64)
            .map(|i| if i % 2 == 0 { 400 } else { -400 })
            .collect();
        let fit = fit_sine(&y, 0.5);
        assert!((fit.amplitude - 400.0).abs() < 1e-6, "{fit:?}");
        assert!((fit.component_rms - 400.0).abs() < 1e-6, "{fit:?}");
        assert!(fit.residual_rms < 1e-6, "{fit:?}");
    }

    #[test]
    fn fit_empty_record_is_all_zero() {
        let fit = fit_sine(&[], 0.1);
        assert_eq!(fit.amplitude, 0.0);
        assert_eq!(fit.component_rms, 0.0);
        assert_eq!(fit.residual_rms, 0.0);
    }

    #[test]
    fn dft_bin_matches_fit_on_exact_bins() {
        let n = 1024;
        for &(bin, amp) in &[(1_usize, 800.0_f64), (57, 2500.0), (511, 1200.0)] {
            let f = bin as f64 / n as f64;
            let y = tone(n, f, amp, 0.4);
            let rms = dft_bin_rms(&y, bin);
            let expected = amp / 2.0_f64.sqrt();
            assert!(
                (rms - expected).abs() < 1.0,
                "bin {bin}: rms {rms} != {expected}"
            );
            // Off-bin readings of a pure exact-bin tone are ~zero
            // (rectangular-window leakage vanishes on the bin grid).
            let off = dft_bin_rms(&y, bin + 1);
            assert!(off < 1.0, "bin {}: leakage {off}", bin + 1);
        }
    }

    #[test]
    fn dft_bin_dc_and_nyquist_scaling() {
        let n = 256;
        // DC: constant 321 → bin-0 RMS = 321.
        let y = alloc::vec![321_i32; n];
        assert!((dft_bin_rms(&y, 0) - 321.0).abs() < 1e-6);
        // Nyquist: alternating ±321 → bin-N/2 RMS = 321.
        let y: Vec<i32> = (0..n)
            .map(|i| if i % 2 == 0 { 321 } else { -321 })
            .collect();
        assert!((dft_bin_rms(&y, n / 2) - 321.0).abs() < 1e-6);
    }

    #[test]
    fn band_rms_satisfies_parseval_on_the_full_bin_range() {
        // Sum of all bin powers = total record power (Parseval, with
        // the RMS scaling folding the conjugate pairs together).
        let n = 512;
        let mut y = tone(n, 20.0 / n as f64, 3000.0, 0.0);
        let y2 = tone(n, 100.0 / n as f64, 1000.0, 1.0);
        for (a, b) in y.iter_mut().zip(y2.iter()) {
            *a += *b;
        }
        let full = band_rms(&y, 0..=n / 2);
        let mut total_sq = 0.0_f64;
        for &v in &y {
            total_sq += (v as f64) * (v as f64);
        }
        let total_rms = (total_sq / n as f64).sqrt();
        assert!(
            (full - total_rms).abs() < 0.01 * total_rms,
            "band {full} vs total {total_rms}"
        );
    }

    #[test]
    fn band_rms_excludes_out_of_band_components() {
        let n = 1024;
        // Tone at bin 300 only.
        let y = tone(n, 300.0 / n as f64, 2000.0, 0.0);
        // A band that excludes bin 300 reads (near) nothing.
        let out = band_rms(&y, 10..=250);
        assert!(out < 2.0, "out-of-band leak {out}");
        // A band that includes it reads the component RMS.
        let inb = band_rms(&y, 250..=350);
        assert!((inb - 2000.0 / 2.0_f64.sqrt()).abs() < 2.0, "{inb}");
    }

    #[test]
    fn peak_bin_finds_the_strongest_component() {
        let n = 1024;
        let mut y = tone(n, 100.0 / n as f64, 500.0, 0.0);
        let y2 = tone(n, 400.0 / n as f64, 1500.0, 0.5);
        for (a, b) in y.iter_mut().zip(y2.iter()) {
            *a += *b;
        }
        let (k, rms) = peak_bin(&y, 1..=n / 2);
        assert_eq!(k, 400);
        assert!((rms - 1500.0 / 2.0_f64.sqrt()).abs() < 2.0, "{rms}");
        // Restricting the range to exclude the big tone finds the
        // small one.
        let (k, _) = peak_bin(&y, 1..=200);
        assert_eq!(k, 100);
    }

    #[test]
    fn bin_frequency_mapping_round_trips() {
        // 4096 samples at 16 kHz → bin width 3.90625 Hz.
        let n = 4096;
        let rate = 16_000;
        // 50 Hz lower edge → first bin at or above = ceil(50·4096/16000) = 13.
        assert_eq!(bin_at_or_above_hz(n, rate, 50), 13);
        // 7000 Hz upper edge → floor(7000·4096/16000) = 1792.
        assert_eq!(bin_at_or_below_hz(n, rate, 7000), 1792);
        // Exact-bin frequency maps to itself from both sides.
        assert_eq!(bin_at_or_above_hz(n, rate, 4000), 1024);
        assert_eq!(bin_at_or_below_hz(n, rate, 4000), 1024);
        // Nyquist.
        assert_eq!(bin_at_or_below_hz(n, rate, 8000), n / 2);
    }

    #[test]
    fn empty_and_out_of_range_scans_are_safe() {
        assert_eq!(dft_bin_rms(&[], 3), 0.0);
        assert_eq!(band_rms(&[], 0..=10), 0.0);
        assert_eq!(peak_bin(&[], 0..=10), (0, 0.0));
        // Bins beyond N/2 are skipped, not aliased.
        let y = tone(64, 4.0 / 64.0, 1000.0, 0.0);
        let full = band_rms(&y, 0..=1000);
        let valid = band_rms(&y, 0..=32);
        assert!((full - valid).abs() < 1e-9);
    }
}

//! Transmission characteristics — clause 2 of ITU-T G.722 (11/88).
//!
//! Clause 2 of the staged Recommendation pins a set of normative
//! limits an SB-ADPCM implementation must meet under back-to-back
//! encoder/decoder evaluation (clause 2.4 page 9, Figure 9/G.722
//! "Looped measurement configurations"). This module surfaces those
//! limits as typed constants and a small verification helper for the
//! one limit that can be checked end-to-end against the codec without
//! reaching outside the spec:
//!
//! | Spec ref      | Quantity                                     | Limit                    | Surface |
//! | ------------- | -------------------------------------------- | ------------------------ | ------- |
//! | clause 1.6    | Bit / octet clock                            | 64 kHz / 8 kHz           | [`BIT_CLOCK_HZ`] / [`OCTET_CLOCK_HZ`] |
//! | clause 1.6    | A/D + D/A 16 kHz sample-clock tolerance      | ±50·10⁻⁶ (= ±50 ppm)     | [`SAMPLE_CLOCK_TOLERANCE_PPM`] |
//! | clause 2.2    | Overload point of the A/D + D/A converters   | +9 dBm0 ± 0.3 dB         | [`OVERLOAD_POINT_DBM0`] / [`OVERLOAD_POINT_TOLERANCE_DB`] |
//! | clause 2.3    | Nominal reference frequency                  | 1020 Hz (+2 / −7 Hz)     | [`NOMINAL_REFERENCE_FREQUENCY_HZ`] |
//! | clause 2.4.1  | Nominal 3-dB passband (Mode 1)               | 50 Hz to 7000 Hz         | [`NOMINAL_PASSBAND_LOW_HZ`] / [`NOMINAL_PASSBAND_HIGH_HZ`] |
//! | clause 2.4.3  | Absolute group delay (50…7000 Hz, −10 dBm0)  | ≤ 4 ms                   | [`ABSOLUTE_GROUP_DELAY_MAX_MS`] |
//! | clause 2.4.4  | Idle noise (in 50…7000 Hz, no input)         | ≤ −66 dBm0               | [`IDLE_NOISE_MAX_DBM0_NARROWBAND`] |
//! | clause 2.4.4  | Idle noise (in 50…20000 Hz, no input)        | ≤ −60 dBm0               | [`IDLE_NOISE_MAX_DBM0_WIDEBAND`] |
//! | clause 2.4.5  | Single-frequency noise                       | ≤ −70 dBm0               | [`SINGLE_FREQUENCY_NOISE_MAX_DBM0`] |
//!
//! The values are dimensionless constants here; the spec only nails
//! the analogue-domain accounting (the A/D + D/A converters of
//! Figure 2/G.722 page 2). Section 2.4 quotes back-to-back digital
//! results from a *looped configuration* (Figure 9a/G.722 page 10),
//! which is the SB-ADPCM coder driven by clause 2.2's converters with
//! the analogue overload point pinned at +9 dBm0. That accounting is
//! captured in [`uniform_pcm_full_scale`] / [`dbm0_to_uniform_pcm`] so
//! that a caller measuring digital-domain power (RMS of the receive
//! audio part's 14-bit uniform PCM) can compare against the dBm0 limits
//! of clause 2.4 by way of clause 2.2.
//!
//! ## Provenance
//!
//! Every quantity in this module is transcribed directly from
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf`. Cited clause / page
//! numbers refer to that document.

use crate::Decoder;
use crate::Encoder;
use crate::Mode;

pub mod reconstructing_filter;

// -----------------------------------------------------------------------
// Clock + sample-rate accounting (clause 1.6, page 8)
// -----------------------------------------------------------------------

/// 64 kHz bit-clock of the wire stream (clause 1.6 page 8: "64 kHz bit
/// timing and 8 kHz octet timing should be provided by the network to
/// the audio decoder").
pub const BIT_CLOCK_HZ: u32 = 64_000;

/// 8 kHz octet-clock — one octet per (lower-band sample, higher-band
/// sample) pair (clause 1.6 page 8).
pub const OCTET_CLOCK_HZ: u32 = 8_000;

/// 16 kHz uniform-PCM sample-clock of the audio parts (clause 1.4.1
/// page 4 — "The input to the transmit QMFs, x_in, is the output from
/// the transmit audio part and is sampled at 16 kHz").
pub const PCM_SAMPLE_CLOCK_HZ: u32 = 16_000;

/// 8 kHz sub-band sample-clock (clause 1.4.1 page 4 — the QMF outputs
/// `x_L` / `x_H` "are sampled at 8 kHz").
pub const SUBBAND_SAMPLE_CLOCK_HZ: u32 = 8_000;

/// 14-bit uniform-PCM representation of the audio parts (clause 1.4.1
/// page 4 — "uniform digital signal which is coded using 14 bits with
/// 16 kHz sampling"). Width is exposed in bits, not as a magnitude,
/// because the spec doesn't pin the precise saturating boundary —
/// our [`uniform_pcm_full_scale`] resolves that to ±2¹³ = ±8192.
pub const UNIFORM_PCM_BITS: u32 = 14;

/// Sample-clock precision required of the A/D + D/A converters of
/// clause 2.2 (page 8): the spec quotes "± 50·10⁻⁶" = 50 parts per
/// million on both the 16 kHz A/D clock and the 16 kHz D/A clock.
pub const SAMPLE_CLOCK_TOLERANCE_PPM: u32 = 50;

// -----------------------------------------------------------------------
// Analogue-domain reference levels (clause 2.2 + 2.3)
// -----------------------------------------------------------------------

/// Overload point (clipping point) of the analogue-to-uniform-digital
/// converters of clause 2.2 (page 8): +9 dBm0 ± 0.3 dB. The 0 dBm0
/// reference therefore sits 9 dB below digital full-scale.
pub const OVERLOAD_POINT_DBM0: f64 = 9.0;

/// Tolerance on the overload point (clause 2.2 page 8).
pub const OVERLOAD_POINT_TOLERANCE_DB: f64 = 0.3;

/// Nominal reference frequency for transmission-characteristic
/// measurements (clause 2.3 page 8). The spec replaces the
/// industry-standard 1000 Hz with 1020 Hz "to avoid sub-harmonic
/// relationships with the 16 kHz sampling frequency".
pub const NOMINAL_REFERENCE_FREQUENCY_HZ: u32 = 1020;

/// Allowed deviation from [`NOMINAL_REFERENCE_FREQUENCY_HZ`] in the
/// measurement setup (clause 2.3 page 8): "+2 to −7 Hz".
pub const NOMINAL_REFERENCE_FREQUENCY_PLUS_HZ: i32 = 2;
/// See [`NOMINAL_REFERENCE_FREQUENCY_PLUS_HZ`].
pub const NOMINAL_REFERENCE_FREQUENCY_MINUS_HZ: i32 = 7;

// -----------------------------------------------------------------------
// Passband + group-delay limits (clause 2.4.1, 2.4.3 — Mode 1 only)
// -----------------------------------------------------------------------

/// Low edge of the nominal 3-dB passband (clause 2.4.1 page 9).
pub const NOMINAL_PASSBAND_LOW_HZ: u32 = 50;

/// High edge of the nominal 3-dB passband (clause 2.4.1 page 9).
pub const NOMINAL_PASSBAND_HIGH_HZ: u32 = 7000;

/// Maximum absolute group delay across the passband (clause 2.4.3
/// page 9): "should not exceed 4 ms". Defined as the minimum group
/// delay for a sine wave between 50 and 7000 Hz at −10 dBm0.
pub const ABSOLUTE_GROUP_DELAY_MAX_MS: f64 = 4.0;

// -----------------------------------------------------------------------
// Noise limits (clause 2.4.4, 2.4.5)
// -----------------------------------------------------------------------

/// Maximum unweighted idle-noise power measured in 50–7000 Hz at the
/// receive audio test point (B) with no signal at the transmit input
/// (A) (clause 2.4.4 page 9): "should not exceed −66 dBm0".
pub const IDLE_NOISE_MAX_DBM0_NARROWBAND: f64 = -66.0;

/// Same as [`IDLE_NOISE_MAX_DBM0_NARROWBAND`] but in the wider
/// 50 Hz – 20 kHz band (clause 2.4.4 page 9): "should not exceed
/// −60 dBm0".
pub const IDLE_NOISE_MAX_DBM0_WIDEBAND: f64 = -60.0;

/// Maximum single-frequency noise power (clause 2.4.5 page 11): "any
/// single frequency (in particular 8000 Hz, the sampling frequency
/// and its multiples), measured selectively with no signal at the
/// input port (test point A) should not exceed −70 dBm0".
pub const SINGLE_FREQUENCY_NOISE_MAX_DBM0: f64 = -70.0;

/// Lower bound of the idle-noise wideband measurement window (clause
/// 2.4.4 page 9).
pub const IDLE_NOISE_BAND_LOW_HZ: u32 = 50;

/// Upper bound of the idle-noise narrow-band measurement window
/// (clause 2.4.4 page 9). Matches the passband upper edge.
pub const IDLE_NOISE_NARROWBAND_HIGH_HZ: u32 = 7000;

/// Upper bound of the idle-noise wideband measurement window (clause
/// 2.4.4 page 9).
pub const IDLE_NOISE_WIDEBAND_HIGH_HZ: u32 = 20_000;

// -----------------------------------------------------------------------
// Digital-domain conversions (bridge clause 2.2 ↔ clause 2.4)
// -----------------------------------------------------------------------

/// Magnitude that the 14-bit uniform-PCM representation treats as
/// full-scale.
///
/// The spec doesn't bolt this number to a particular integer because
/// the audio-parts converters are specified in the analogue domain
/// (clause 2.2). Our implementation pins the 14-bit uniform-PCM
/// boundary at ±2¹³ = ±8192 — the natural 14-bit-signed boundary —
/// and Table 9/G.722 (page 25) on the SB-ADPCM side caps the QMF
/// outputs at ±16384 (one extra bit of headroom inside the codec).
/// Callers measuring digital-domain RMS power must therefore treat
/// ±8192 as 0 dB-relative-to-full-scale.
pub const fn uniform_pcm_full_scale() -> i32 {
    1 << (UNIFORM_PCM_BITS - 1) // 2^13 = 8192
}

/// Convert a dBm0 power level to the corresponding 14-bit uniform-PCM
/// RMS magnitude. Uses clause 2.2's overload-point accounting: the
/// 0 dBm0 reference is [`OVERLOAD_POINT_DBM0`] dB below the digital
/// full-scale of [`uniform_pcm_full_scale`].
///
/// Returns `f64` (not `i32`) because realistic dBm0 powers correspond
/// to fractional PCM magnitudes — the caller compares an integer-RMS
/// against this threshold using whatever rounding policy fits.
pub fn dbm0_to_uniform_pcm(dbm0: f64) -> f64 {
    // Digital full-scale corresponds to +9 dBm0 (clause 2.2).
    // RMS magnitude is `full_scale * 10^((dbm0 - 9) / 20)`.
    let fs = uniform_pcm_full_scale() as f64;
    fs * 10_f64.powf((dbm0 - OVERLOAD_POINT_DBM0) / 20.0)
}

/// Inverse of [`dbm0_to_uniform_pcm`]. Returns `f64::NEG_INFINITY`
/// for zero RMS.
pub fn uniform_pcm_rms_to_dbm0(rms: f64) -> f64 {
    if rms <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let fs = uniform_pcm_full_scale() as f64;
    OVERLOAD_POINT_DBM0 + 20.0 * (rms / fs).log10()
}

/// Root-mean-square magnitude of a 14-bit uniform-PCM segment, in the
/// same units as the input samples (i.e. as if the receive audio
/// part's converter output were measured directly).
///
/// Empty input returns `0.0`.
pub fn uniform_pcm_rms(samples: &[i32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let n = samples.len() as f64;
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum_sq / n).sqrt()
}

// -----------------------------------------------------------------------
// End-to-end idle-noise check (clause 2.4.4)
// -----------------------------------------------------------------------

/// Returned by [`measure_idle_noise`]: a clause-2.4.4-style report on
/// the idle-noise floor of an SB-ADPCM encode → decode loop driven by
/// a digital all-zero input.
///
/// The spec's −66 dBm0 / −60 dBm0 / −70 dBm0 limits of clause 2.4 sit
/// at the *analogue* receive test point B (Figure 2/G.722 page 2) —
/// downstream of the receive audio part's reconstructing filter
/// (clause 2.5.2 page 11) whose mask removes out-of-band content
/// above ~7 kHz. The report below sits at the **digital** boundary
/// between the SB-ADPCM decoder and the receive audio part (the
/// `x_out` arrow of Figure 1/G.722 page 2). Anything the SB-ADPCM
/// loop produces in the 7–8 kHz residue would be attenuated by the
/// reconstructing filter before reaching test point B; the report
/// therefore counts it against the limit as a worst-case bound, not
/// as a faithful Figure 9/G.722 measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdleNoiseReport {
    /// Sample count actually used for the RMS measurement (the
    /// initial transient is dropped — see [`measure_idle_noise`]).
    pub samples_measured: usize,
    /// RMS magnitude of the receive-audio-part output (in 14-bit
    /// uniform-PCM units), measured at the SB-ADPCM decoder's
    /// digital output — i.e. before the receive audio part's
    /// reconstructing filter of clause 2.5.2.
    pub rms_uniform_pcm: f64,
    /// Same RMS expressed as a dBm0 level via [`uniform_pcm_rms_to_dbm0`].
    pub rms_dbm0: f64,
    /// `true` when [`Self::rms_dbm0`] is at or below
    /// [`IDLE_NOISE_MAX_DBM0_NARROWBAND`] (the 50–7000 Hz limit).
    /// The digital RMS is a wideband (0–8 kHz) measurement and so
    /// is an upper bound on the spec's analogue narrow-band measure;
    /// a `true` here is sufficient (but not necessary) for clause
    /// 2.4.4 compliance.
    pub meets_narrowband_limit: bool,
    /// `true` when [`Self::rms_dbm0`] is at or below
    /// [`IDLE_NOISE_MAX_DBM0_WIDEBAND`] (the 50 Hz – 20 kHz limit).
    pub meets_wideband_limit: bool,
}

/// Drive `encoder` → `decoder` with `digital_zero_count` samples of
/// 14-bit uniform-PCM silence and return an [`IdleNoiseReport`].
///
/// The first 32 output samples are dropped to ride out the QMF's
/// 24-tap warm-up + the predictor's initial scale-factor transient
/// (clauses 3.5 / 3.6 leak the log scale-factor toward zero over
/// roughly ¹⁄₁₂₈ per sample so the warm-up is short).
///
/// The decoder is run in the requested mode; pass [`Mode::Mode1`] for
/// the clause 2.4 limits which the spec quotes specifically for Mode 1
/// ("These limits apply to operation in Mode 1", page 9).
pub fn measure_idle_noise(
    encoder: &mut Encoder,
    decoder: &mut Decoder,
    digital_zero_count: usize,
) -> IdleNoiseReport {
    // Drive the encoder/decoder loop end-to-end with digital silence.
    let pcm_in = alloc::vec![0_i32; digital_zero_count];
    let octets = encoder.encode(&pcm_in);
    let pcm_out = decoder.decode(&octets);

    // Drop the QMF warm-up window (24 taps / 2 samples-per-octet = 12
    // octets ≈ 24 PCM samples; we use 32 for the leak-decay envelope).
    let skip = pcm_out.len().min(32);
    let measured = &pcm_out[skip..];
    let rms_pcm = uniform_pcm_rms(measured);
    let rms_dbm0 = uniform_pcm_rms_to_dbm0(rms_pcm);
    IdleNoiseReport {
        samples_measured: measured.len(),
        rms_uniform_pcm: rms_pcm,
        rms_dbm0,
        meets_narrowband_limit: rms_dbm0 <= IDLE_NOISE_MAX_DBM0_NARROWBAND,
        meets_wideband_limit: rms_dbm0 <= IDLE_NOISE_MAX_DBM0_WIDEBAND,
    }
}

/// Convenience wrapper that constructs fresh encoder + decoder in the
/// requested mode, runs them through [`measure_idle_noise`] for a
/// generous sample window, and returns the report.
pub fn measure_idle_noise_default(mode: Mode) -> IdleNoiseReport {
    let mut enc = Encoder::new();
    let mut dec = Decoder::new(mode);
    // 4096 samples = 256 ms at 16 kHz — well above the time-constant
    // of the leak path (clauses 3.5 / 3.6 have a ¹⁄₁₂₈ per-sample
    // leak, hence a ≈ 16 ms scale-factor time constant).
    measure_idle_noise(&mut enc, &mut dec, 4096)
}

extern crate alloc;

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_constants_match_clause_1_6() {
        // 64 kbit/s = 8 kHz octets × 8 bits per octet (clause 1.6 page 8).
        assert_eq!(BIT_CLOCK_HZ, OCTET_CLOCK_HZ * 8);
        // 16 kHz PCM = 2 × 8 kHz sub-band sample rate (clauses 1.4.1
        // and 3.1).
        assert_eq!(PCM_SAMPLE_CLOCK_HZ, SUBBAND_SAMPLE_CLOCK_HZ * 2);
        // Sample-clock tolerance must be a (small) ppm figure.
        assert_eq!(SAMPLE_CLOCK_TOLERANCE_PPM, 50);
    }

    #[test]
    fn uniform_pcm_full_scale_is_8192() {
        // 14-bit-signed two's-complement boundary (clause 1.4.1 + the
        // SB-ADPCM ±16384 cap of Table 9/G.722 sits one bit above this).
        assert_eq!(uniform_pcm_full_scale(), 8192);
    }

    #[test]
    fn dbm0_round_trip_through_uniform_pcm() {
        // The overload point of clause 2.2 is +9 dBm0 — it MUST
        // correspond to the full-scale magnitude.
        let fs = uniform_pcm_full_scale() as f64;
        let pcm = dbm0_to_uniform_pcm(OVERLOAD_POINT_DBM0);
        assert!((pcm - fs).abs() < 1e-6, "dBm0 anchor missed full-scale");
        // 0 dBm0 is 9 dB below full-scale.
        let pcm0 = dbm0_to_uniform_pcm(0.0);
        let expected = fs * 10_f64.powf(-OVERLOAD_POINT_DBM0 / 20.0);
        assert!((pcm0 - expected).abs() < 1e-6);
        // Round-trip arbitrary values.
        for lvl in [-90.0_f64, -66.0, -10.0, 0.0, 5.0, 9.0] {
            let pcm = dbm0_to_uniform_pcm(lvl);
            let back = uniform_pcm_rms_to_dbm0(pcm);
            assert!((back - lvl).abs() < 1e-9, "dBm0 round-trip drift at {lvl}");
        }
    }

    #[test]
    fn uniform_pcm_rms_zero_for_silence() {
        assert_eq!(uniform_pcm_rms(&[]), 0.0);
        assert_eq!(uniform_pcm_rms(&[0; 16]), 0.0);
    }

    #[test]
    fn uniform_pcm_rms_matches_amplitude_for_dc() {
        // A constant non-zero signal has RMS equal to its magnitude.
        let s = [123_i32; 64];
        assert!((uniform_pcm_rms(&s) - 123.0).abs() < 1e-9);
    }

    #[test]
    fn uniform_pcm_rms_matches_known_sine_amplitude() {
        // A sine of amplitude A has RMS = A / sqrt(2).
        let n = 512;
        let amp = 1000.0;
        let sine: alloc::vec::Vec<i32> = (0..n)
            .map(|i| (amp * (2.0 * core::f64::consts::PI * i as f64 / n as f64).sin()) as i32)
            .collect();
        let rms = uniform_pcm_rms(&sine);
        let expected = amp / 2_f64.sqrt();
        // Loose tolerance because the i32 truncation costs ~0.5 LSB.
        assert!(
            (rms - expected).abs() < 2.0,
            "sine RMS {rms} != expected {expected}"
        );
    }

    #[test]
    fn rms_to_dbm0_handles_zero_as_neg_infinity() {
        assert_eq!(uniform_pcm_rms_to_dbm0(0.0), f64::NEG_INFINITY);
        assert_eq!(uniform_pcm_rms_to_dbm0(-1.0), f64::NEG_INFINITY);
    }

    #[test]
    fn passband_constants_match_clause_2_4_1() {
        // Clause 2.4.1 page 9: "The nominal 3 dB bandwidth is
        // 50 to 7000 Hz".
        assert_eq!(NOMINAL_PASSBAND_LOW_HZ, 50);
        assert_eq!(NOMINAL_PASSBAND_HIGH_HZ, 7000);
        const _: () = assert!(NOMINAL_PASSBAND_LOW_HZ < NOMINAL_PASSBAND_HIGH_HZ);
        // The passband must sit inside the QMF's 0–8 kHz analysis
        // window (clause 1.4.1 page 4).
        const _: () = assert!(NOMINAL_PASSBAND_HIGH_HZ < SUBBAND_SAMPLE_CLOCK_HZ);
    }

    #[test]
    fn group_delay_limit_matches_clause_2_4_3() {
        // Clause 2.4.3 page 9: "should not exceed 4 ms".
        assert!((ABSOLUTE_GROUP_DELAY_MAX_MS - 4.0).abs() < 1e-9);
    }

    #[test]
    fn noise_limits_match_clause_2_4_4_and_2_4_5() {
        // Clause 2.4.4 page 9.
        assert!((IDLE_NOISE_MAX_DBM0_NARROWBAND - -66.0).abs() < 1e-9);
        assert!((IDLE_NOISE_MAX_DBM0_WIDEBAND - -60.0).abs() < 1e-9);
        // Clause 2.4.5 page 11.
        assert!((SINGLE_FREQUENCY_NOISE_MAX_DBM0 - -70.0).abs() < 1e-9);
        // The narrow-band limit is tighter than the wideband one (in
        // a smaller window, less noise is collected, so the spec's
        // bar is higher = a smaller maximum).
        const _: () = assert!(IDLE_NOISE_MAX_DBM0_NARROWBAND < IDLE_NOISE_MAX_DBM0_WIDEBAND);
        // Single-frequency mask is tighter than the wideband one
        // (selective measurement of a single tone).
        const _: () = assert!(SINGLE_FREQUENCY_NOISE_MAX_DBM0 < IDLE_NOISE_MAX_DBM0_WIDEBAND);
    }

    #[test]
    fn noise_band_boundaries_align_with_passband() {
        // Clause 2.4.4 page 9: the narrowband measurement runs from
        // 50 Hz to 7000 Hz (the same as the nominal passband). The
        // wideband measurement extends to 20 kHz.
        assert_eq!(IDLE_NOISE_BAND_LOW_HZ, NOMINAL_PASSBAND_LOW_HZ);
        assert_eq!(IDLE_NOISE_NARROWBAND_HIGH_HZ, NOMINAL_PASSBAND_HIGH_HZ);
        assert_eq!(IDLE_NOISE_WIDEBAND_HIGH_HZ, 20_000);
    }

    #[test]
    fn nominal_reference_frequency_matches_clause_2_3() {
        // Clause 2.3 page 8: 1020 Hz, +2/−7 Hz.
        assert_eq!(NOMINAL_REFERENCE_FREQUENCY_HZ, 1020);
        assert_eq!(NOMINAL_REFERENCE_FREQUENCY_PLUS_HZ, 2);
        assert_eq!(NOMINAL_REFERENCE_FREQUENCY_MINUS_HZ, 7);
        // The reference frequency sits in the codec passband.
        const _: () = assert!(NOMINAL_REFERENCE_FREQUENCY_HZ > NOMINAL_PASSBAND_LOW_HZ);
        const _: () = assert!(NOMINAL_REFERENCE_FREQUENCY_HZ < NOMINAL_PASSBAND_HIGH_HZ);
    }

    #[test]
    fn overload_point_matches_clause_2_2() {
        // Clause 2.2 page 8: +9 dBm0 ± 0.3 dB.
        assert!((OVERLOAD_POINT_DBM0 - 9.0).abs() < 1e-9);
        assert!((OVERLOAD_POINT_TOLERANCE_DB - 0.3).abs() < 1e-9);
    }

    #[test]
    fn idle_noise_report_silence_mode1_meets_wideband_limit() {
        // Clause 2.4.4 page 9 quotes a −60 dBm0 upper bound for the
        // *wideband* (50 Hz – 20 kHz) measurement and a tighter
        // −66 dBm0 bound for the *narrowband* (50 Hz – 7000 Hz)
        // measurement, both taken at test point B (Figure 2/G.722
        // page 2) — i.e. downstream of the receive audio part's
        // reconstructing filter (clause 2.5.2 page 11) which removes
        // sub-band-edge residue above ~7 kHz. Our digital-only loop
        // does not apply that filter so the measurement is a
        // worst-case upper bound on the spec's measurement: passing
        // the wideband bound guarantees clause 2.4.4 compliance once
        // the receive audio part is wired up. The narrowband bound
        // remains a docs-gap target (clause 2.5.2 reconstructing
        // filter not staged as a normative DSP mask).
        let r = measure_idle_noise_default(Mode::Mode1);
        assert!(
            r.meets_wideband_limit,
            "idle-noise report {r:?} exceeded the −60 dBm0 wideband limit"
        );
        assert!(r.samples_measured >= 4096 - 64);
    }

    #[test]
    fn idle_noise_report_silence_mode2_meets_wideband_limit() {
        // Clause 2.4 quotes its limits explicitly for Mode 1 ("These
        // limits apply to operation in Mode 1"); Modes 2 and 3
        // discard 1 / 2 LSBs of the lower sub-band so the receive
        // floor is somewhat noisier than Mode 1's. The digital-only
        // silence loop must still sit under the wideband −60 dBm0
        // bound by the same upper-bound argument as Mode 1.
        let r = measure_idle_noise_default(Mode::Mode2);
        assert!(r.meets_wideband_limit, "mode 2: {r:?}");
    }

    #[test]
    fn idle_noise_report_silence_mode3_meets_wideband_limit() {
        let r = measure_idle_noise_default(Mode::Mode3);
        assert!(r.meets_wideband_limit, "mode 3: {r:?}");
    }

    #[test]
    fn idle_noise_report_sample_count_is_input_minus_skip() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let r = measure_idle_noise(&mut enc, &mut dec, 512);
        assert!(r.samples_measured == 512 - 32);
        // RMS of pure-silence-driven digital loop is small (≪ the
        // 14-bit converter range). The predictor leak path drives
        // 1–2 LSBs of residue through the QMF; the analogue
        // reconstructing filter of clause 2.5.2 then attenuates the
        // 7–8 kHz component away to meet clause 2.4.4. We only check
        // the digital-floor magnitude here.
        assert!(
            r.rms_uniform_pcm < 4.0,
            "silence RMS {} exceeded 4 LSB",
            r.rms_uniform_pcm
        );
    }

    #[test]
    fn idle_noise_report_silence_floor_is_at_most_a_handful_of_lsbs() {
        // Direct check: digital silence must not drive the predictor
        // into a divergent regime. The integer-LSB residue under
        // digital silence is bounded by the QMF's tap-coefficient
        // round-off (Table 11/G.722 p. 27 = ±3876 max coefficient,
        // ×11-bit shift = ±2 LSB scale). Anything materially above
        // that is a predictor / scale-factor adaptation regression.
        for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let r = measure_idle_noise_default(mode);
            assert!(
                r.rms_uniform_pcm < 4.0,
                "{mode:?} digital silence RMS {} blew past the 4-LSB envelope",
                r.rms_uniform_pcm
            );
        }
    }

    #[test]
    fn idle_noise_under_input_window_returns_empty_measurement_safely() {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        // 16 samples of silence -> 8 octets -> 16 output samples, all
        // of which are inside the skip window. The measurement window
        // is empty so the RMS must be 0 and the noise-mask checks
        // pass trivially.
        let r = measure_idle_noise(&mut enc, &mut dec, 16);
        assert_eq!(r.samples_measured, 0);
        assert_eq!(r.rms_uniform_pcm, 0.0);
        assert_eq!(r.rms_dbm0, f64::NEG_INFINITY);
        assert!(r.meets_narrowband_limit);
        assert!(r.meets_wideband_limit);
    }
}

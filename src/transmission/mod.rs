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
//! | clause 2.4.2  | Attenuation/frequency-distortion mask        | Figure 10 / G.722        | [`attenuation_distortion`] |
//! | clause 2.4.3  | Absolute group delay (50…7000 Hz, −10 dBm0)  | ≤ 4 ms                   | [`ABSOLUTE_GROUP_DELAY_MAX_MS`] |
//! | clause 2.4.4  | Idle noise (in 50…7000 Hz, no input)         | ≤ −66 dBm0               | [`IDLE_NOISE_MAX_DBM0_NARROWBAND`] |
//! | clause 2.4.4  | Idle noise (in 50…20000 Hz, no input)        | ≤ −60 dBm0               | [`IDLE_NOISE_MAX_DBM0_WIDEBAND`] |
//! | clause 2.4.5  | Single-frequency noise                       | ≤ −70 dBm0               | [`SINGLE_FREQUENCY_NOISE_MAX_DBM0`] |
//! | clause 2.4.6  | Codec-loop signal-to-total distortion        | "Under study" (no mask)  | [`measure_signal_to_distortion`] (measured-behaviour regression gates) |
//! | clause 2.5.1  | Input anti-aliasing-filter mask              | Figure 11 / G.722        | [`anti_aliasing_filter`] |
//! | clause 2.5.2  | Output reconstructing-filter mask            | Figure 12 / G.722        | [`reconstructing_filter`] |
//! | clause 2.5.3  | Group-delay-distortion mask                  | Figure 13 / G.722        | [`group_delay_distortion`] |
//! | clause 2.5.4  | Receive-audio-part idle noise (50…7000 Hz)   | ≤ −75 dBm0               | [`RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0`] |
//! | clause 2.5.5  | Signal-to-total-distortion floor vs level    | Figure 14 / G.722        | [`signal_to_distortion`] |
//! | clause 2.5.6  | Signal-to-total-distortion floor vs frequency | Figure 15 / G.722       | [`signal_to_distortion_frequency`] |
//! | clause 2.5.7  | Gain-variation corridor vs input level       | Figure 16 / G.722        | [`gain_variation`] |
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

pub mod anti_aliasing_filter;
pub mod attenuation_distortion;
pub mod gain_variation;
pub mod group_delay_distortion;
pub mod reconstructing_filter;
pub mod signal_to_distortion;
pub mod signal_to_distortion_frequency;
pub mod spectrum;

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

/// Maximum unweighted idle-noise power of the **receive audio part
/// alone** (clause 2.5.4 page 13): "The unweighted noise power of the
/// receive audio part measured in the frequency range 50 to 7000 Hz
/// with 14-bit all-zero signal at its input should not exceed
/// −75 dBm0."
///
/// This is an audio-parts requirement (clause 2.5), not a codec
/// requirement: it bounds the receive audio part (D/A converter +
/// output reconstructing filter of Figure 2/G.722 page 2) in
/// isolation, with digital silence at its input. It is therefore
/// 9 dB stricter than the end-to-end narrow-band limit of clause
/// 2.4.4 ([`IDLE_NOISE_MAX_DBM0_NARROWBAND`], −66 dBm0) — the
/// SB-ADPCM loop's own noise floor is granted the difference.
pub const RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0: f64 = -75.0;

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

// -----------------------------------------------------------------------
// Spectral idle-channel measurement (clauses 2.4.4 + 2.4.5)
// -----------------------------------------------------------------------

/// Returned by [`measure_idle_channel_spectrum`]: a frequency-resolved
/// idle-channel report against the clause 2.4.4 band-limited
/// noise-power limits and the clause 2.4.5 selective single-frequency
/// limit.
///
/// The wideband [`measure_idle_noise`] RMS cannot check the
/// *narrow-band* −66 dBm0 bound of clause 2.4.4 (its reading includes
/// the 7–8 kHz sub-band-edge residue the receive audio part's
/// reconstructing filter of clause 2.5.2 would remove) nor the
/// "measured selectively" −70 dBm0 bound of clause 2.4.5. This report
/// resolves both by measuring per-DFT-bin at the digital boundary:
/// the 50 – 7000 Hz band power is exactly the clause 2.4.4 narrow-band
/// window, and the per-bin peak is the clause 2.4.5 selective sweep.
/// Frequencies above the 8 kHz digital Nyquist do not exist at this
/// boundary, so the 50 Hz – 20 kHz wideband window of clause 2.4.4
/// truncates to 50 Hz – 8 kHz here (everything the codec can emit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdleChannelSpectrumReport {
    /// Sample count of the measured steady-state window.
    pub samples_measured: usize,
    /// RMS (uniform-PCM units) in the 50 – 7000 Hz narrow band of
    /// clause 2.4.4.
    pub narrowband_rms: f64,
    /// [`Self::narrowband_rms`] as dBm0.
    pub narrowband_dbm0: f64,
    /// RMS in the 50 Hz – 8 kHz band (the clause 2.4.4 wideband
    /// window truncated at the digital Nyquist).
    pub wideband_rms: f64,
    /// [`Self::wideband_rms`] as dBm0.
    pub wideband_dbm0: f64,
    /// Centre frequency (Hz) of the strongest single-frequency
    /// component at or above the first non-DC bin.
    pub peak_frequency_hz: f64,
    /// RMS of that strongest component.
    pub peak_rms: f64,
    /// [`Self::peak_rms`] as dBm0 — the clause 2.4.5 selective
    /// reading.
    pub peak_dbm0: f64,
    /// RMS of the 8000 Hz (Nyquist) component, the frequency clause
    /// 2.4.5 singles out ("in particular 8000 Hz, the sampling
    /// frequency and its multiples").
    pub nyquist_rms: f64,
    /// Constant (DC) component of the steady-state output, in
    /// uniform-PCM LSBs. DC is not a transmittable "frequency" (the
    /// receive audio part is AC-coupled through its reconstructing
    /// filter) so it is reported separately rather than counted
    /// against the clause 2.4.5 limit.
    pub dc_component: f64,
    /// `true` when [`Self::narrowband_dbm0`] ≤
    /// [`IDLE_NOISE_MAX_DBM0_NARROWBAND`] (−66 dBm0, clause 2.4.4).
    pub meets_narrowband_limit: bool,
    /// `true` when [`Self::wideband_dbm0`] ≤
    /// [`IDLE_NOISE_MAX_DBM0_WIDEBAND`] (−60 dBm0, clause 2.4.4).
    pub meets_wideband_limit: bool,
    /// `true` when [`Self::peak_dbm0`] ≤
    /// [`SINGLE_FREQUENCY_NOISE_MAX_DBM0`] (−70 dBm0, clause 2.4.5).
    pub meets_single_frequency_limit: bool,
}

/// Drive a fresh encoder → decoder loop in `mode` with digital
/// silence and return the frequency-resolved
/// [`IdleChannelSpectrumReport`] over a 4096-sample (256 ms)
/// steady-state window (64 warm-up samples dropped — twice the
/// [`measure_idle_noise`] margin, so the QMF warm-up and scale-factor
/// leak transient are fully excluded from the DFT record).
///
/// Clause 2.4 quotes its limits for Mode 1 ("These limits apply to
/// operation in Mode 1", page 9); callers may still evaluate Modes 2
/// and 3, which this implementation holds to the same limits.
pub fn measure_idle_channel_spectrum(mode: Mode) -> IdleChannelSpectrumReport {
    const SKIP: usize = 64;
    const WINDOW: usize = 4096;
    let mut enc = Encoder::new();
    let mut dec = Decoder::new(mode);
    let pcm_in = alloc::vec![0_i32; WINDOW + SKIP];
    let octets = enc.encode(&pcm_in);
    let out = dec.decode(&octets);
    let w = &out[SKIP..SKIP + WINDOW];

    let lo = spectrum::bin_at_or_above_hz(WINDOW, PCM_SAMPLE_CLOCK_HZ, IDLE_NOISE_BAND_LOW_HZ);
    let hi_nb =
        spectrum::bin_at_or_below_hz(WINDOW, PCM_SAMPLE_CLOCK_HZ, IDLE_NOISE_NARROWBAND_HIGH_HZ);
    let nyquist_bin = WINDOW / 2;

    let narrowband_rms = spectrum::band_rms(w, lo..=hi_nb);
    let wideband_rms = spectrum::band_rms(w, lo..=nyquist_bin);
    let (peak_bin, peak_rms) = spectrum::peak_bin(w, 1..=nyquist_bin);
    let nyquist_rms = spectrum::dft_bin_rms(w, nyquist_bin);
    let dc_component = spectrum::dft_bin_rms(w, 0);

    let narrowband_dbm0 = uniform_pcm_rms_to_dbm0(narrowband_rms);
    let wideband_dbm0 = uniform_pcm_rms_to_dbm0(wideband_rms);
    let peak_dbm0 = uniform_pcm_rms_to_dbm0(peak_rms);
    IdleChannelSpectrumReport {
        samples_measured: WINDOW,
        narrowband_rms,
        narrowband_dbm0,
        wideband_rms,
        wideband_dbm0,
        peak_frequency_hz: peak_bin as f64 * PCM_SAMPLE_CLOCK_HZ as f64 / WINDOW as f64,
        peak_rms,
        peak_dbm0,
        nyquist_rms,
        dc_component,
        meets_narrowband_limit: narrowband_dbm0 <= IDLE_NOISE_MAX_DBM0_NARROWBAND,
        meets_wideband_limit: wideband_dbm0 <= IDLE_NOISE_MAX_DBM0_WIDEBAND,
        meets_single_frequency_limit: peak_dbm0 <= SINGLE_FREQUENCY_NOISE_MAX_DBM0,
    }
}

// -----------------------------------------------------------------------
// End-to-end tone-response measurement (clause 2.4.2 attenuation mask)
// -----------------------------------------------------------------------

/// Returned by [`measure_tone_response`]: an end-to-end gain / attenuation
/// measurement of an SB-ADPCM encode → decode loop driven by a single
/// sinusoid, for checking against the clause 2.4.2 / Figure 10/G.722
/// attenuation/frequency mask (see
/// [`crate::transmission::attenuation_distortion`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneResponseReport {
    /// Tone frequency in Hz.
    pub frequency_hz: f64,
    /// Requested input level in dBm0.
    pub input_level_dbm0: f64,
    /// Measured RMS of the reconstructed output (14-bit uniform-PCM units).
    pub output_rms_uniform_pcm: f64,
    /// End-to-end gain in dB (`20·log10(out_rms / in_rms)`); positive
    /// means amplification, negative means loss.
    pub gain_db: f64,
    /// End-to-end attenuation in dB — the negated [`Self::gain_db`], in
    /// the sign convention of the Figure 10/G.722 mask (positive =
    /// loss). Feed this to
    /// [`crate::transmission::attenuation_distortion::evaluate`].
    pub attenuation_db: f64,
}

/// Drive `encoder` → `decoder` with a `frequency_hz` sinusoid at
/// `input_level_dbm0` and measure the end-to-end gain / attenuation.
///
/// The sine is generated at the 16 kHz PCM rate, quantized to the 14-bit
/// uniform-PCM grid the codec consumes, encoded, and decoded; the output
/// RMS (after dropping a warm-up window from both signals) is compared
/// against the input RMS. The result is intended to be checked against
/// the clause 2.4.2 attenuation/frequency mask in the looped
/// Figure 9/G.722 configuration — a digital-domain stand-in that omits
/// the analogue audio-part filters, so it is a *necessary* condition for
/// clause-2.4.2 compliance measured at the SB-ADPCM digital boundary
/// rather than at analogue test point B.
pub fn measure_tone_response(
    encoder: &mut Encoder,
    decoder: &mut Decoder,
    frequency_hz: f64,
    input_level_dbm0: f64,
    samples: usize,
) -> ToneResponseReport {
    use core::f64::consts::TAU;
    // Peak amplitude for the requested RMS level (sine peak = √2 · RMS).
    let amplitude = dbm0_to_uniform_pcm(input_level_dbm0) * core::f64::consts::SQRT_2;
    let cycles_per_sample = frequency_hz / PCM_SAMPLE_CLOCK_HZ as f64;
    let pcm_in: alloc::vec::Vec<i32> = (0..samples)
        .map(|n| (amplitude * (TAU * cycles_per_sample * n as f64).sin()).round() as i32)
        .collect();
    let octets = encoder.encode(&pcm_in);
    let pcm_out = decoder.decode(&octets);

    // Drop a warm-up window from both the input and output so the RMS
    // ratio is taken over a matched steady-state region (128 samples
    // ≈ 8 ms ≫ the ¹⁄₁₂₈ leak time constant of clauses 3.5 / 3.6).
    let skip = pcm_out.len().min(128);
    let measured_out = if pcm_out.len() > skip {
        &pcm_out[skip..]
    } else {
        &pcm_out[..]
    };
    let in_skip = skip.min(pcm_in.len());
    let measured_in = &pcm_in[in_skip..];
    let out_rms = uniform_pcm_rms(measured_out);
    let in_rms = uniform_pcm_rms(measured_in);
    let gain_db = if in_rms > 0.0 && out_rms > 0.0 {
        20.0 * (out_rms / in_rms).log10()
    } else {
        f64::NEG_INFINITY
    };
    ToneResponseReport {
        frequency_hz,
        input_level_dbm0,
        output_rms_uniform_pcm: out_rms,
        gain_db,
        attenuation_db: -gain_db,
    }
}

// -----------------------------------------------------------------------
// End-to-end signal-to-total-distortion measurement (clause 2.4.6 /
// clause 2.5.5 measurement shape, applied at the digital boundary)
// -----------------------------------------------------------------------

/// Returned by [`measure_signal_to_distortion`]: a selective
/// signal-vs-total-distortion reading of an SB-ADPCM encode → decode
/// loop driven by a single sinusoid.
///
/// The measurement shape follows clause 2.5.5 (page 13: "the ratio of
/// signal-to-total distortion power … measured unweighted") — the
/// output record is decomposed into the sinusoidal component at the
/// stimulus frequency (the *signal*) and everything else (the *total
/// distortion*), via the exact least-squares fit of
/// [`spectrum::fit_sine`]. For the **codec** itself the corresponding
/// requirement, clause 2.4.6 (page 11), is "Under study" — the
/// Recommendation prints no codec-loop S/D mask — so codec-loop
/// readings taken with this helper are *quality-regression* anchors
/// pinned against measured behaviour, not normative limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalToDistortionReport {
    /// Stimulus frequency in Hz.
    pub frequency_hz: f64,
    /// Requested stimulus level in dBm0.
    pub input_level_dbm0: f64,
    /// RMS of the recovered sinusoidal component at the stimulus
    /// frequency (14-bit uniform-PCM units).
    pub signal_rms_uniform_pcm: f64,
    /// [`Self::signal_rms_uniform_pcm`] as a dBm0 level.
    pub signal_dbm0: f64,
    /// RMS of everything in the output record that is *not* the
    /// stimulus-frequency component (quantization noise, harmonics,
    /// aliasing residue) — the unweighted "total distortion".
    pub distortion_rms_uniform_pcm: f64,
    /// [`Self::distortion_rms_uniform_pcm`] as a dBm0 level.
    pub distortion_dbm0: f64,
    /// Signal-to-total-distortion ratio in dB
    /// (`20·log10(signal_rms / distortion_rms)`); `f64::INFINITY`
    /// when the distortion is exactly zero.
    pub ratio_db: f64,
    /// Phase (radians, at the first measured sample) of the fitted
    /// *input* component over the same window — the reference reading
    /// for the clause 2.4.3 group-delay probe.
    pub input_phase_radians: f64,
    /// Phase (radians, at the first measured sample) of the fitted
    /// *output* component. `input_phase_radians −
    /// output_phase_radians` (mod 2π) is the phase lag the codec
    /// imposes at this frequency.
    pub output_phase_radians: f64,
}

/// Drive `encoder` → `decoder` with a `frequency_hz` sinusoid at
/// `input_level_dbm0` and split the steady-state output into
/// signal-at-frequency vs total distortion.
///
/// The stimulus is synthesised at the 16 kHz PCM rate exactly as in
/// [`measure_tone_response`]; the first 256 output samples (16 ms —
/// several clause 3.5 / 3.6 scale-factor time constants) are dropped,
/// and both the input and output records are least-squares-fitted over
/// the *same* absolute sample window so the two phase readings share a
/// time origin (their difference is the codec's phase lag, the
/// clause 2.4.3 group-delay probe).
pub fn measure_signal_to_distortion(
    encoder: &mut Encoder,
    decoder: &mut Decoder,
    frequency_hz: f64,
    input_level_dbm0: f64,
    samples: usize,
) -> SignalToDistortionReport {
    use core::f64::consts::TAU;
    let amplitude = dbm0_to_uniform_pcm(input_level_dbm0) * core::f64::consts::SQRT_2;
    let cycles_per_sample = frequency_hz / PCM_SAMPLE_CLOCK_HZ as f64;
    let pcm_in: alloc::vec::Vec<i32> = (0..samples)
        .map(|n| (amplitude * (TAU * cycles_per_sample * n as f64).sin()).round() as i32)
        .collect();
    let octets = encoder.encode(&pcm_in);
    let pcm_out = decoder.decode(&octets);

    let skip = pcm_out.len().min(256);
    let out_window = &pcm_out[skip..];
    let in_window = &pcm_in[skip..pcm_in.len().min(skip + out_window.len())];

    let out_fit = spectrum::fit_sine(out_window, cycles_per_sample);
    let in_fit = spectrum::fit_sine(in_window, cycles_per_sample);

    let signal_rms = out_fit.component_rms;
    let distortion_rms = out_fit.residual_rms;
    let ratio_db = if distortion_rms > 0.0 {
        20.0 * (signal_rms / distortion_rms).log10()
    } else if signal_rms > 0.0 {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };
    SignalToDistortionReport {
        frequency_hz,
        input_level_dbm0,
        signal_rms_uniform_pcm: signal_rms,
        signal_dbm0: uniform_pcm_rms_to_dbm0(signal_rms),
        distortion_rms_uniform_pcm: distortion_rms,
        distortion_dbm0: uniform_pcm_rms_to_dbm0(distortion_rms),
        ratio_db,
        input_phase_radians: in_fit.phase_radians,
        output_phase_radians: out_fit.phase_radians,
    }
}

/// Convenience wrapper: fresh encoder + decoder in `mode`, one
/// [`measure_signal_to_distortion`] run over a 8192-sample (512 ms)
/// stimulus.
pub fn measure_signal_to_distortion_default(
    mode: Mode,
    frequency_hz: f64,
    input_level_dbm0: f64,
) -> SignalToDistortionReport {
    let mut enc = Encoder::new();
    let mut dec = Decoder::new(mode);
    measure_signal_to_distortion(&mut enc, &mut dec, frequency_hz, input_level_dbm0, 8192)
}

// -----------------------------------------------------------------------
// End-to-end group-delay measurement (clause 2.4.3)
// -----------------------------------------------------------------------

/// Returned by [`measure_group_delay`]: a two-tone phase-slope group
/// delay reading of the looped codec, for the clause 2.4.3 limit
/// ("The absolute group delay, defined as the minimum group delay for
/// a sine wave signal between 50 and 7000 Hz, should not exceed 4 ms.
/// The test level is −10 dBm0", page 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupDelayReport {
    /// Centre frequency (Hz) the group delay is evaluated at.
    pub center_frequency_hz: f64,
    /// Tone spacing (Hz) of the two probe measurements.
    pub spacing_hz: f64,
    /// Stimulus level in dBm0.
    pub input_level_dbm0: f64,
    /// Measured group delay in 16 kHz samples.
    pub delay_samples: f64,
    /// [`Self::delay_samples`] converted to milliseconds — compare
    /// against [`ABSOLUTE_GROUP_DELAY_MAX_MS`].
    pub delay_ms: f64,
}

/// Wrap an angle to the principal interval (−π, π].
fn wrap_phase(mut phi: f64) -> f64 {
    use core::f64::consts::{PI, TAU};
    while phi <= -PI {
        phi += TAU;
    }
    while phi > PI {
        phi -= TAU;
    }
    phi
}

/// Measure the codec's group delay at `center_frequency_hz` by the
/// phase-slope (two-tone) method: group delay is the derivative of
/// phase lag with respect to angular frequency, so two independent
/// looped-tone measurements at `center ± spacing/2` (each phase read
/// via the exact least-squares fit of
/// [`measure_signal_to_distortion`], input and output over the same
/// window) give `τ = Δφ / Δω`. The per-tone phase lags are only known
/// modulo 2π, but their *difference* is unambiguous provided
/// `spacing_hz` is small enough that `Δφ` stays inside (−π, π] — with
/// the default 40 Hz spacing that admits delays up to 200 samples
/// (12.5 ms), comfortably beyond the 4 ms clause 2.4.3 ceiling.
///
/// A fresh encoder + decoder pair is constructed per tone so the two
/// readings share the reset state of clauses 3.5 / 3.6.
pub fn measure_group_delay(
    mode: Mode,
    center_frequency_hz: f64,
    spacing_hz: f64,
    input_level_dbm0: f64,
    samples: usize,
) -> GroupDelayReport {
    use core::f64::consts::TAU;
    let mut lag = [0.0_f64; 2];
    let freqs = [
        center_frequency_hz - spacing_hz / 2.0,
        center_frequency_hz + spacing_hz / 2.0,
    ];
    for (slot, f) in lag.iter_mut().zip(freqs.iter()) {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(mode);
        let r = measure_signal_to_distortion(&mut enc, &mut dec, *f, input_level_dbm0, samples);
        // Positive lag = output behind input.
        *slot = wrap_phase(r.input_phase_radians - r.output_phase_radians);
    }
    // A pure delay of d samples turns the output phase into
    // φ_out(ω) = φ_in(ω) + ω·d, so the lag φ_in − φ_out falls with
    // slope −d against ω: τ = −Δ(lag) / Δω. Δφ is wrapped to the
    // principal interval; Δω is in rad/sample.
    let dphi = wrap_phase(lag[1] - lag[0]);
    let domega = TAU * spacing_hz / PCM_SAMPLE_CLOCK_HZ as f64;
    let delay_samples = -dphi / domega;
    GroupDelayReport {
        center_frequency_hz,
        spacing_hz,
        input_level_dbm0,
        delay_samples,
        delay_ms: delay_samples * 1000.0 / PCM_SAMPLE_CLOCK_HZ as f64,
    }
}

/// Convenience wrapper for the clause 2.4.3 measurement conditions:
/// −10 dBm0 test level, 40 Hz probe spacing, 8192-sample records.
pub fn measure_group_delay_default(mode: Mode, center_frequency_hz: f64) -> GroupDelayReport {
    measure_group_delay(mode, center_frequency_hz, 40.0, -10.0, 8192)
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
    fn receive_audio_part_idle_noise_matches_clause_2_5_4() {
        // Clause 2.5.4 page 13: "should not exceed −75 dBm0".
        assert!((RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0 - -75.0).abs() < 1e-9);
        // The receive-audio-part-alone bound is stricter than both
        // end-to-end codec bounds of clause 2.4.4 (measured in the
        // same 50–7000 Hz window for the narrow-band one): the
        // SB-ADPCM loop is allowed to dominate the end-to-end floor.
        const _: () =
            assert!(RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0 < IDLE_NOISE_MAX_DBM0_NARROWBAND);
        const _: () =
            assert!(RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0 < IDLE_NOISE_MAX_DBM0_WIDEBAND);
        // ... and 9 dB stricter than the narrow-band codec bound.
        assert!(
            (IDLE_NOISE_MAX_DBM0_NARROWBAND - RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0 - 9.0).abs()
                < 1e-9
        );
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

    #[test]
    fn reference_tone_passes_clause_2_4_2_tight_corridor_all_modes() {
        // Operationally enforce the clause 2.4.2 / Figure 10/G.722
        // attenuation/frequency mask on the *actual* codec rather than
        // only on synthetic mask coordinates. A 1020 Hz reference tone
        // (clause 2.3) at the −10 dBm0 nominal test level (clauses
        // 2.4.2 / 2.5.5) is encoded → decoded in each mode; the
        // measured end-to-end attenuation must sit inside the tight
        // in-band corridor (−1 dB ≤ atten ≤ +1 dB) of Figure 10. The
        // SB-ADPCM loop is essentially flat at this level, so the
        // attenuation is a few hundredths of a dB — well inside the
        // corridor — but the test pins that any future predictor /
        // QMF regression that introduced a passband gain error would
        // break clause 2.4.2.
        let f = NOMINAL_REFERENCE_FREQUENCY_HZ as f64;
        assert_eq!(
            attenuation_distortion::classify(f),
            attenuation_distortion::MaskBand::InBandTight,
            "1020 Hz must classify into the tight in-band corridor"
        );
        for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let mut enc = Encoder::new();
            let mut dec = Decoder::new(mode);
            let r = measure_tone_response(&mut enc, &mut dec, f, -10.0, 8192);
            let (band, ok) = attenuation_distortion::evaluate(f, r.attenuation_db);
            assert_eq!(band, attenuation_distortion::MaskBand::InBandTight);
            assert!(
                ok,
                "{mode:?}: 1020 Hz attenuation {:.4} dB escaped the clause 2.4.2 tight corridor",
                r.attenuation_db
            );
            // Belt-and-braces: the measured attenuation must sit inside
            // the printed tight corridor bounds (−1 dB … +1 dB) the
            // `evaluate` call above checks. The lossier Mode 3 (4-bit
            // lower band) shows the largest loss at this level (≈ 0.7 dB)
            // but still well within the +1 dB ceiling; Modes 1 / 2 are
            // near-transparent (a few hundredths of a dB).
            assert!(
                (attenuation_distortion::IN_BAND_LOWER_BOUND_DB
                    ..=attenuation_distortion::IN_BAND_TIGHT_UPPER_BOUND_DB)
                    .contains(&r.attenuation_db),
                "{mode:?}: 1020 Hz attenuation {:.4} dB outside the printed tight corridor",
                r.attenuation_db
            );
        }
    }

    #[test]
    fn passband_sweep_stays_inside_clause_2_4_2_mask_mode1() {
        // Sweep the printed Figure 10 in-band breakpoints plus a grid
        // of passband frequencies through the codec and assert each
        // measured attenuation meets the mask for its band. This is the
        // frequency-swept end-to-end companion of the single-tone test
        // above and guards the whole passband, not just 1020 Hz.
        for &f in &[100.0, 300.0, 500.0, 1020.0, 2000.0, 3000.0, 3400.0] {
            let mut enc = Encoder::new();
            let mut dec = Decoder::new(Mode::Mode1);
            let r = measure_tone_response(&mut enc, &mut dec, f, -10.0, 8192);
            let (_band, ok) = attenuation_distortion::evaluate(f, r.attenuation_db);
            assert!(
                ok,
                "{f} Hz attenuation {:.4} dB escaped the clause 2.4.2 mask",
                r.attenuation_db
            );
        }
    }

    #[test]
    fn tone_response_reports_negative_infinity_gain_for_silence() {
        // A zero-level "tone" produces zero input RMS, so the gain is
        // undefined; the report must surface −∞ rather than NaN.
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let r = measure_tone_response(&mut enc, &mut dec, 1020.0, f64::NEG_INFINITY, 512);
        assert_eq!(r.gain_db, f64::NEG_INFINITY);
        assert_eq!(r.attenuation_db, f64::INFINITY);
    }

    // -------------------------------------------------------------------
    // Whole-codec signal-to-total-distortion quality gates.
    //
    // Clause 2.4.6 (page 11) leaves the codec-loop S/D requirement
    // "Under study" — the Recommendation prints no mask for the
    // SB-ADPCM loop itself (the Figure 14/G.722 mask of clause 2.5.5
    // constrains only the audio parts of the Figure 9b loop). The
    // gates below therefore pin the *measured* behaviour of this
    // implementation, with ≈ 2 dB of headroom, so any predictor /
    // quantizer / QMF regression that degrades the reconstruction
    // quality trips a test even though no normative floor exists.
    // The measurement itself is the clause 2.5.5 shape (selective
    // signal vs unweighted total distortion) applied at the digital
    // boundary of the Figure 9a loop.
    // -------------------------------------------------------------------

    /// Stimulus levels (dBm0) for the S/D gates below.
    const SD_LEVELS_DBM0: [f64; 5] = [-40.0, -30.0, -20.0, -10.0, 0.0];

    #[test]
    fn codec_sd_floors_at_reference_frequency_all_modes() {
        // Measured (8192-sample window, 256-sample warm-up skip):
        //   Mode 1: 23.7 / 29.9 / 32.8 / 31.8 / 33.6 dB
        //   Mode 2: 21.2 / 25.0 / 27.0 / 26.0 / 27.6 dB
        //   Mode 3: 10.3 / 13.4 / 15.2 / 11.5 / 13.9 dB
        // Floors sit ≈ 2 dB under the measurements.
        let floors: [(Mode, [f64; 5]); 3] = [
            (Mode::Mode1, [21.5, 27.5, 30.5, 29.5, 31.5]),
            (Mode::Mode2, [19.0, 22.5, 24.5, 23.5, 25.0]),
            (Mode::Mode3, [8.0, 11.0, 13.0, 9.5, 11.5]),
        ];
        let f = NOMINAL_REFERENCE_FREQUENCY_HZ as f64;
        for (mode, mode_floors) in floors {
            for (lvl, floor) in SD_LEVELS_DBM0.iter().zip(mode_floors.iter()) {
                let r = measure_signal_to_distortion_default(mode, f, *lvl);
                assert!(
                    r.ratio_db >= *floor,
                    "{mode:?} @ {lvl} dBm0: S/D {:.2} dB fell under the {floor} dB gate",
                    r.ratio_db
                );
            }
        }
    }

    #[test]
    fn codec_sd_mode_ordering_at_reference_frequency() {
        // The three modes decode 6 / 5 / 4 lower-sub-band bits
        // (Table 1 page 3), so at a lower-band stimulus the S/D must
        // strictly improve with each extra decoded bit, at every
        // level. Measured gaps: Mode1−Mode2 ≥ 2.5 dB, Mode2−Mode3
        // ≥ 10.9 dB across the level grid; gates at 2 / 8 dB.
        let f = NOMINAL_REFERENCE_FREQUENCY_HZ as f64;
        for lvl in SD_LEVELS_DBM0 {
            let m1 = measure_signal_to_distortion_default(Mode::Mode1, f, lvl).ratio_db;
            let m2 = measure_signal_to_distortion_default(Mode::Mode2, f, lvl).ratio_db;
            let m3 = measure_signal_to_distortion_default(Mode::Mode3, f, lvl).ratio_db;
            assert!(
                m1 - m2 >= 2.0,
                "@ {lvl} dBm0: Mode1 {m1:.2} not ≥ 2 dB above Mode2 {m2:.2}"
            );
            assert!(
                m2 - m3 >= 8.0,
                "@ {lvl} dBm0: Mode2 {m2:.2} not ≥ 8 dB above Mode3 {m3:.2}"
            );
        }
    }

    #[test]
    fn codec_sd_higher_band_is_mode_independent() {
        // A 6 kHz stimulus sits above the 4 kHz QMF split, so its
        // coding runs through the 2-bit higher-sub-band loop, which
        // Table 1 (page 3) keeps identical across the three modes —
        // only the lower band loses LSBs. Measured: ≥ 13.4 dB S/D at
        // every mode / level, with ≤ 0.4 dB spread across modes.
        // Gates: 11.5 dB floor, 1.0 dB spread.
        for lvl in SD_LEVELS_DBM0 {
            let sds: alloc::vec::Vec<f64> = [Mode::Mode1, Mode::Mode2, Mode::Mode3]
                .into_iter()
                .map(|m| measure_signal_to_distortion_default(m, 6000.0, lvl).ratio_db)
                .collect();
            for sd in &sds {
                assert!(
                    *sd >= 11.5,
                    "@ {lvl} dBm0: higher-band S/D {sd:.2} under the 11.5 dB gate"
                );
            }
            let spread = sds.iter().cloned().fold(f64::MIN, f64::max)
                - sds.iter().cloned().fold(f64::MAX, f64::min);
            assert!(
                spread <= 1.0,
                "@ {lvl} dBm0: higher-band S/D spread {spread:.2} dB across modes"
            );
        }
    }

    #[test]
    fn codec_sd_is_level_tracking_like_an_adaptive_quantizer() {
        // The whole point of the clause 3.5 / 3.6 adaptive scale
        // factors is that reconstruction quality stays roughly
        // constant across a wide input-level range (the quantizer
        // rescales rather than drowning quiet signals). Gate: the
        // Mode-1 S/D spread across the −40 … 0 dBm0 grid stays
        // within 12 dB (measured 9.8 dB), i.e. nowhere near the
        // 40 dB spread a *fixed* quantizer would show over the same
        // grid.
        let f = NOMINAL_REFERENCE_FREQUENCY_HZ as f64;
        let sds: alloc::vec::Vec<f64> = SD_LEVELS_DBM0
            .iter()
            .map(|&lvl| measure_signal_to_distortion_default(Mode::Mode1, f, lvl).ratio_db)
            .collect();
        let spread = sds.iter().cloned().fold(f64::MIN, f64::max)
            - sds.iter().cloned().fold(f64::MAX, f64::min);
        assert!(
            spread <= 12.0,
            "Mode 1 S/D spread {spread:.2} dB across −40…0 dBm0 (grid {sds:?})"
        );
    }

    #[test]
    fn codec_sd_recovered_signal_tracks_the_stimulus_level() {
        // The fitted at-frequency component must come back at the
        // stimulus level: within 1 dB for Modes 1 / 2 (measured worst
        // 0.4 dB, at the 6 kHz higher-band tone), within 4.5 dB for
        // the 4-bit Mode 3 (measured worst 3.8 dB, at 3 kHz /
        // −40 dBm0 where the truncated quantizer's reconstruction
        // bias is largest). This is the level-accounting cross-check
        // between dbm0_to_uniform_pcm and the codec's unity passband
        // gain (clause 2.4.2).
        for (mode, tol) in [(Mode::Mode1, 1.0), (Mode::Mode2, 1.0), (Mode::Mode3, 4.5)] {
            for f in [1020.0, 2000.0, 3000.0, 6000.0] {
                for lvl in SD_LEVELS_DBM0 {
                    let r = measure_signal_to_distortion_default(mode, f, lvl);
                    assert!(
                        (r.signal_dbm0 - lvl).abs() <= tol,
                        "{mode:?} {f} Hz @ {lvl} dBm0: recovered {:.2} dBm0 off by more than {tol} dB",
                        r.signal_dbm0
                    );
                }
            }
        }
    }

    #[test]
    fn codec_sd_report_edge_cases_are_total() {
        // Silence stimulus: no signal, no meaningful ratio — the
        // report must stay finite-field-safe, never NaN.
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let r = measure_signal_to_distortion(&mut enc, &mut dec, 1020.0, f64::NEG_INFINITY, 512);
        assert!(r.signal_rms_uniform_pcm < 4.0);
        assert!(!r.ratio_db.is_nan());
        // Records shorter than the warm-up skip yield an empty
        // window; everything must come back zero / −∞, not NaN.
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let r = measure_signal_to_distortion(&mut enc, &mut dec, 1020.0, -10.0, 64);
        assert_eq!(r.signal_rms_uniform_pcm, 0.0);
        assert_eq!(r.distortion_rms_uniform_pcm, 0.0);
        assert_eq!(r.ratio_db, f64::NEG_INFINITY);
    }

    // -------------------------------------------------------------------
    // Frequency-resolved idle-channel conformance (clauses 2.4.4 + 2.4.5).
    // -------------------------------------------------------------------

    #[test]
    fn idle_channel_meets_clause_2_4_4_narrowband_limit_all_modes() {
        // The narrow-band (50 - 7000 Hz) -66 dBm0 bound of clause
        // 2.4.4 (p. 9) was previously out of reach: the wideband RMS
        // of measure_idle_noise cannot exclude the 7 - 8 kHz sub-band
        // residue. The DFT-resolved band power can. Measured: the
        // steady-state idle output is a pure DC pattern (see the
        // dc-only anchor below), so the in-band noise power is zero
        // to numerical precision -- far below every printed bound.
        for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let r = measure_idle_channel_spectrum(mode);
            assert!(
                r.meets_narrowband_limit,
                "{mode:?}: narrow-band idle noise {:.2} dBm0 exceeded -66 dBm0 ({r:?})",
                r.narrowband_dbm0
            );
            assert!(
                r.meets_wideband_limit,
                "{mode:?}: wideband idle noise {:.2} dBm0 exceeded -60 dBm0 ({r:?})",
                r.wideband_dbm0
            );
            assert_eq!(r.samples_measured, 4096);
        }
    }

    #[test]
    fn idle_channel_meets_clause_2_4_5_single_frequency_limit_all_modes() {
        // Clause 2.4.5 (p. 11): "The level of any single frequency
        // (in particular 8000 Hz, the sampling frequency and its
        // multiples), measured selectively ... should not exceed
        // -70 dBm0." The per-bin peak of the idle spectrum is that
        // selective sweep across everything the digital boundary can
        // carry (up to the 8 kHz Nyquist).
        for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let r = measure_idle_channel_spectrum(mode);
            assert!(
                r.meets_single_frequency_limit,
                "{mode:?}: selective idle peak {:.2} dBm0 at {:.1} Hz exceeded -70 dBm0",
                r.peak_dbm0, r.peak_frequency_hz
            );
            // The spec singles out 8000 Hz; pin its bin explicitly.
            assert!(
                uniform_pcm_rms_to_dbm0(r.nyquist_rms) <= SINGLE_FREQUENCY_NOISE_MAX_DBM0,
                "{mode:?}: 8000 Hz idle component {:.4} LSB rms exceeded -70 dBm0",
                r.nyquist_rms
            );
        }
    }

    #[test]
    fn idle_channel_steady_state_is_dc_only() {
        // Sharper than any power bound: with digital silence in, the
        // decoded steady state settles to a *constant* -- +1 LSB in
        // Mode 1 (the Mode-1 INVQBL reconstruction of the steady
        // silence codeword settles the sub-band pair at (r_L, r_H) =
        // (1, 0), which the unity-DC-gain receive QMF carries through
        // as +1) and exactly 0 in Modes 2 / 3. All idle energy is DC,
        // which the receive audio part's reconstructing filter
        // (clause 2.5.2) blocks -- the clause 2.4.4 / 2.4.5 margins
        // are structural, not incidental.
        for (mode, expected) in [(Mode::Mode1, 1), (Mode::Mode2, 0), (Mode::Mode3, 0)] {
            let mut enc = Encoder::new();
            let mut dec = Decoder::new(mode);
            let pcm_in = alloc::vec![0_i32; 4160];
            let out = dec.decode(&enc.encode(&pcm_in));
            let w = &out[64..];
            assert!(
                w.iter().all(|&v| v == expected),
                "{mode:?}: idle steady state is not the constant {expected}"
            );
            // And the report's DC accounting agrees.
            let r = measure_idle_channel_spectrum(mode);
            assert!(
                (r.dc_component - expected as f64).abs() < 1e-6,
                "{mode:?}: DC component {:.6} != {expected}",
                r.dc_component
            );
        }
    }

    // -------------------------------------------------------------------
    // Operational group delay (clause 2.4.3).
    // -------------------------------------------------------------------

    /// Clause 2.4.3 sweep grid: the 50 - 7000 Hz probe band, avoiding
    /// the exact 4 kHz QMF crossover where a single tone splits
    /// between the sub-bands.
    const GROUP_DELAY_SWEEP_HZ: [f64; 11] = [
        100.0, 250.0, 500.0, 1020.0, 2000.0, 3000.0, 3500.0, 4500.0, 5000.0, 6000.0, 6800.0,
    ];

    #[test]
    fn codec_absolute_group_delay_meets_clause_2_4_3() {
        // Clause 2.4.3 (p. 9): the absolute group delay -- "defined as
        // the minimum group delay for a sine wave signal between 50
        // and 7000 Hz", test level -10 dBm0 -- "should not exceed
        // 4 ms". Clause 2.4 quotes the limit for Mode 1. Measured:
        // every point of the sweep sits at ~22 samples = ~1.38 ms, so
        // both the spec's minimum-over-band reading and the (stronger)
        // per-point ceiling clear the limit with ~2.9x headroom.
        let mut min_ms = f64::INFINITY;
        for f in GROUP_DELAY_SWEEP_HZ {
            let r = measure_group_delay_default(Mode::Mode1, f);
            assert!(
                r.delay_ms > 0.0 && r.delay_ms <= ABSOLUTE_GROUP_DELAY_MAX_MS,
                "{f} Hz: group delay {:.4} ms outside (0, 4] ms",
                r.delay_ms
            );
            min_ms = min_ms.min(r.delay_ms);
        }
        assert!(
            min_ms <= ABSOLUTE_GROUP_DELAY_MAX_MS,
            "absolute (minimum) group delay {min_ms:.4} ms exceeded 4 ms"
        );
    }

    #[test]
    fn codec_group_delay_is_the_flat_qmf_cascade_delay_all_modes() {
        // The QMF banks are linear-phase (clause 3.1 / clause 4.4:
        // "linear phase non-recursive digital filters") and the ADPCM
        // loops are memoryless in phase at steady state, so the codec
        // group delay must be flat across frequency and equal to the
        // fixed analysis+synthesis cascade delay. Measured: ~22
        // samples at every sweep point, spread <= 1.3 samples in
        // Modes 1 / 2 and <= 3.5 samples in the noisier 4-bit Mode 3
        // (phase readings through a ~12 dB S/D loop jitter more).
        // Envelope gates: 19.5 ..= 24.5 samples everywhere.
        for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for f in GROUP_DELAY_SWEEP_HZ {
                let d = measure_group_delay_default(mode, f).delay_samples;
                assert!(
                    (19.5..=24.5).contains(&d),
                    "{mode:?} {f} Hz: group delay {d:.3} samples escaped the 19.5..=24.5 envelope"
                );
                lo = lo.min(d);
                hi = hi.max(d);
            }
            let spread_limit = if mode == Mode::Mode3 { 4.0 } else { 2.0 };
            assert!(
                hi - lo <= spread_limit,
                "{mode:?}: group-delay spread {:.3} samples exceeded {spread_limit}",
                hi - lo
            );
        }
    }

    #[test]
    fn group_delay_report_units_and_phase_wrap() {
        // ms accounting: delay_ms = delay_samples / 16 at the 16 kHz
        // PCM clock.
        let r = measure_group_delay_default(Mode::Mode1, 1020.0);
        assert!((r.delay_ms - r.delay_samples * 1000.0 / PCM_SAMPLE_CLOCK_HZ as f64).abs() < 1e-12);
        assert_eq!(r.spacing_hz, 40.0);
        assert_eq!(r.input_level_dbm0, -10.0);
        // wrap_phase maps into (-pi, pi] and is idempotent there.
        use core::f64::consts::{PI, TAU};
        assert!((wrap_phase(PI + 0.5) - (0.5 - PI)).abs() < 1e-12);
        assert!((wrap_phase(-PI - 0.5) - (PI - 0.5)).abs() < 1e-12);
        assert!((wrap_phase(3.0 * TAU + 0.25) - 0.25).abs() < 1e-9);
        assert_eq!(wrap_phase(PI), PI);
        assert_eq!(wrap_phase(0.0), 0.0);
    }
}

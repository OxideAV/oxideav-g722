# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- **Round-218 clause-2 transmission characteristics.** New
  `transmission` module surfaces the normative limits of clause 2 of
  the staged ITU-T G.722 (11/88) Recommendation as typed constants:
  bit/octet/PCM clock rates (clause 1.6 page 8), A/D + D/A
  sample-clock tolerance (clause 2.2 ±50 ppm), overload-point dBm0
  reference + tolerance (clause 2.2 +9 dBm0 ± 0.3 dB), nominal
  reference frequency (clause 2.3 1020 Hz +2/−7), nominal 3-dB
  passband (clause 2.4.1 50–7000 Hz), absolute group-delay maximum
  (clause 2.4.3 ≤ 4 ms), idle-noise limits (clause 2.4.4 narrow-band
  −66 dBm0 / wideband −60 dBm0), and the selective single-frequency
  noise limit (clause 2.4.5 −70 dBm0). `dbm0_to_uniform_pcm` /
  `uniform_pcm_rms_to_dbm0` / `uniform_pcm_rms` bridge the dBm0
  domain (anchored on clause 2.2) and the 14-bit uniform-PCM domain
  of clause 1.4.1. New `IdleNoiseReport` + `measure_idle_noise`
  drive an end-to-end encoder → decoder digital-silence test that
  confirms the receive-side RMS sits under clause 2.4.4's −60 dBm0
  wideband bound for all three modes. 18 new unit tests covering
  constant traceability, dBm0 ↔ uniform-PCM round-trip, RMS-on-sine
  / RMS-on-DC sanity, and silence-floor envelope per mode.

- **Round-212 auxiliary-data channel** — clean-room implementation
  of Figure 1/G.722's data-insertion / data-extraction devices
  (clause 1.3, Table 1/G.722) covering Modes 2 (8 kbit/s aux) and 3
  (16 kbit/s aux). New `aux_data` module exposes `DataInserter` /
  `DataExtractor` plus const helpers `aux_bits_per_octet` /
  `aux_bit_rate_kbps`. Substitution lands at `I_L6` (Mode 2) or
  `I_L5 + I_L6` (Mode 3) in MSB-first wire order per clause 1.4.4;
  Mode 1 is a pass-through. Caller-supplied padding bit on queue
  underflow; mode-switch preserves the queue. 21 unit tests cover
  Table 1 rates, LSB position, queue bookkeeping, mode-switch
  semantics, end-to-end inserter↔extractor round-trip, and
  audio-side silence-envelope round-trip with Mode-3 aux active.

### Fixed

- **Round-207 Table 19/G.722 transcription anomaly.** `SIL_FROM_IL5`
  now matches the printed Table 19 (p. 40) at `RIL = 11111`: the
  entry resolves to `(SIL = -1, IL5 = 1)` despite a set top bit, the
  same structural shape as `Table 18`'s `111110 / 111111` entries.
  The previous implementation used a pure top-bit-as-sign convention
  which would have flipped the sign of the small-magnitude negative
  decision interval in Mode-2 reception and driven the predictor in
  the wrong direction whenever a `11111` truncated codeword arrived.

### Added

- Round-207 Mode-2 and Mode-3 encoder → decoder round-trip
  silence-envelope tests, plus three new Table 19 unit tests
  covering the `11111` anomaly, the substituted-codeword footnote
  (`00000 / 00001`), and the `IL5 ∈ 1..=15` range invariant.

### Added

- **Round-200 clean-room encoder bring-up.** SB-ADPCM transmit path
  against the staged ITU-T G.722 (11/88) Blue-Book Recommendation:
  - 24-tap transmit QMF (clause 3.1, eqs 3-1..3-4) that splits a
    16 kHz uniform-PCM input into 8 kHz lower / higher sub-band
    streams using the Table 11/G.722 symmetric coefficients.
  - 60-level lower-sub-band forward adaptive log quantizer
    (BLOCK 1L QUANTL, clause 6.2.1.1 p. 42 pseudo-code) with the
    Note-2 LDL == LDU row-exclusion rule.
  - 4-level higher-sub-band forward adaptive quantizer
    (BLOCK 1H QUANTH, clause 6.2.2.1).
  - Multiplexer producing the 64 kbit/s octet layout of clause 1.4.4
    (page 6).
  - Internal refactor: lifted the shared SB-ADPCM predictor and
    scale-factor adaptation into a new `predictor` module so the
    encoder and decoder drive a single source of truth for the
    bit-exact pole / zero / log-scale update equations (clauses
    3.5 / 3.6).
- New `Encoder` public type and `make_encoder()` factory (replacing
  the previous `NotImplemented` stub).
- Spec Tables 16 / 20 (encoder forward output codes) transcribed by
  hand into `ILP6_FROM_ML` / `ILN6_FROM_ML` / `IHP2_FROM_MH` /
  `IHN2_FROM_MH`; existing Table-18 inverse tables corrected to give
  `SIL = -1` for codewords `111110` / `111111` per the spec.
- 13 additional unit tests covering encoder determinism, odd-length
  pending-sample buffering, multiplexer bit-layout round-trip,
  encoder → decoder silence-envelope round-trip, monotonic m_L choice
  with growing magnitude, and the QUANTL / QUANTH bit-exact behaviour
  at reset scale factors.

### Added (round 185)

- **Round-185 clean-room decoder bring-up.** SB-ADPCM receive path:
  lower sub-band ADPCM decoder (4/5/6-bit modes via INVQAL +
  INVQBL), higher sub-band ADPCM decoder (2-bit INVQAH), the
  symmetric pole + zero adaptive predictor with UPPOL1 / UPPOL2 /
  UPZERO updates, logarithmic scale-factor adaptation (Method 2,
  32-entry ILB), and the 24-tap receive QMF that interleaves the
  sub-bands into a 16 kHz output stream.
- `Decoder` / `Mode` public API and `make_decoder` factory.
- 18 unit tests + 1 doc-test covering table shape / monotonicity,
  decoder determinism, mode-switch idempotence, reset symmetry, and
  envelope behaviour with all-zero input.

### Changed

- **Reset to orphan-rebuild scaffold (2026-05-25).** The prior
  implementation was retired under the workspace clean-room policy: its
  data tables were documented as having been copied from an external
  reference implementation of the codec, whose provenance the clean-room
  policy does not permit. All public APIs now return
  `Error::NotImplemented` pending a clean-room rebuild against a staged
  ITU-T G.722 Recommendation.

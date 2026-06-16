# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed

- **Round-322 transmit-QMF normalisation off by a factor of four.** The
  encoder transmit (analysis) QMF (`encoder::TransmitQmf::step`)
  right-shifted the ACCUMA / ACCUMB accumulators by 11 bits, producing
  sub-band signals four times larger than the Recommendation
  prescribes (so any non-trivial input clamped at the LOWT / HIGHT
  ±16384 limit). This is the analysis-side counterpart of the r313
  receive-QMF fix. Per the staged 1988 base edition
  (`docs/audio/g722/T-REC-G.722-198811-S.pdf`) the analysis outputs are
  `xL = xA + xB` and `xH = xA − xB` (eqs 3-1 / 3-2, p. 15) with **no**
  leading "× 2" factor (unlike the receive eqs 4-3 / 4-4), and the
  LOWT / HIGHT sub-blocks (clause 5.2.1, p. 28) define
  `XL = (XA + XB) >> (y − 15)`, `XH = (XA − XB) >> (y − 15)`. With the
  QMF coefficients stored as `h · 2^13` (Table 10/G.722 note, p. 26)
  the accumulators equal `2^13 · Σ h·x`, so the correct normalisation
  is `>> (y − 15) = >> 13` (the decoder's matching `>> (y − 16) = >> 12`
  fixes `y = 28`) — exactly one bit *more* than the receive QMF, not
  two bits *less*. The shift is now `>> 13`. A new spec-derived
  conformance test (`transmit_qmf_dc_splits_with_unity_lower_band_gain`)
  pins the fix: the even and odd QMF half-band branches each sum to
  exactly 0.5 (4096 in Q13), so a constant 16 kHz input must split into
  the lower sub-band with unity DC gain (`XL = D`, `XH = 0`) — an exact
  invariant the previous `>> 11` (which gave `XL = 4·D`) violated. The
  earlier range-only encoder tests did not catch the gain error because
  the saturating LOWT / HIGHT clamp masked it on non-DC inputs.
- **Round-313 receive-QMF normalisation off by a factor of two.** The
  decoder receive QMF (`decoder::ReceiveQmf::step`) right-shifted the
  ACCUMC / ACCUMD accumulator by 11 bits, producing reconstructed
  output samples twice as loud as the Recommendation prescribes. Per
  the staged 1988 base edition (`docs/audio/g722/T-REC-G.722-198811-S.pdf`)
  the synthesis output is `xout = 2 · Σ h·x` (eqs 4-3 / 4-4, p. 24)
  and sub-blocks ACCUMC / ACCUMD (p. 29-30) define
  `XOUT = WD >> (y-16)`. With the QMF coefficients stored as `h · 2^13`
  (Table 10/G.722, p. 26) the integer accumulator `WD = 2^13 · Σ h·x`,
  so `xout = 2 · WD / 2^13 = WD >> 12` — not `>> 11`. The shift is now
  `>> 12`. A new spec-derived conformance test
  (`receive_qmf_lower_band_dc_has_unity_gain`) pins the fix: the even
  and odd QMF half-band branches each sum to exactly 0.5 (4096 in Q13),
  so a constant lower-sub-band level must pass through the receive QMF
  with unity DC gain — an exact, mask-free invariant the previous
  shift violated. The earlier range-only decoder tests did not catch
  the gain error because the 2× output still fell inside the ±16384
  LIMIT range.

### Added

- **Round-304 clause 2.5.7 / Figure 16 gain-variation-vs-input-level
  mask.** New `transmission::gain_variation` sub-module surfaces the
  variation-of-gain-with-input-level mask of Figure 16/G.722 (p. 14)
  referenced by clause 2.5.7 (p. 14) of the staged ITU-T G.722 (11/88)
  Recommendation. The clause measures gain at a 1000 Hz
  (`MEASUREMENT_TONE_HZ`) sine **relative to the gain at −10 dBm0**
  (`REFERENCE_LEVEL_DBM0`); unlike the single-sided distortion floors
  of r277 / r287 and the attenuation masks of r237 / r258, Figure 16
  is a **two-sided symmetric corridor** that widens toward lower input
  levels. The printed input-level anchors (−61 / −56 / −46 / +9 dBm0)
  sit in `INPUT_LEVEL_LOW_DBM0` / `STEP_WIDE_DBM0` / `STEP_TIGHT_DBM0`
  / `INPUT_LEVEL_HIGH_DBM0`, and the corridor half-widths (±1.5 /
  ±0.5 / ±0.3 dB) in `HALF_WIDTH_WIDE_DB` / `HALF_WIDTH_MID_DB` /
  `HALF_WIDTH_TIGHT_DB`. The corridor reads: ±1.5 dB on −61…−56 dBm0,
  ±0.5 dB on −56…−46 dBm0, ±0.3 dB on −46…+9 dBm0, open outside the
  −61 / +9 dBm0 walls (the right wall being the clause 2.2 overload
  point itself). The helper API is `classify(level)` /
  `evaluate(level, gain_variation_db)` / `half_width_db(level)` /
  `upper_bound_db(level)` / `lower_bound_db(level)` plus a `MaskBand`
  enum (`BelowMask` / `Wide` / `Mid` / `Tight` / `AboveMask`), with
  each printed step owned by the stricter band. 20 new unit tests
  anchor every breakpoint and half-width at its printed value,
  exercise classification across all five bands, pin corridor symmetry
  and per-band bounds, sweep the monotone-tightening invariant on a
  0.25 dB grid, pin corridor / NaN / outside-mask boundary semantics,
  confirm the −10 dBm0 reference sits at 0 dB variation inside the
  tight band, and lock the structural alignments (right wall = clause
  2.2 overload point; reference level = clause 2.5.6 test level; tone
  inside the clause 2.4.1 passband and within 2 % of the clause 2.3
  nominal reference frequency).

- **Round-287 clause 2.5.6 / Figure 15 signal-to-total-distortion-vs-frequency
  mask.** New `transmission::signal_to_distortion_frequency`
  sub-module surfaces the signal-to-total-distortion-ratio versus
  frequency mask of Figure 15/G.722 (p. 14) referenced by clause
  2.5.6 (p. 14) of the staged ITU-T G.722 (11/88) Recommendation —
  the frequency-swept companion of the r277 level-swept Figure 14
  mask. The clause fixes the input at the −10 dBm0 nominal test level
  (`TEST_LEVEL_DBM0`, shared with clauses 2.4.2 / 2.4.3) and sweeps
  frequency; like Figure 14 it is a **floor**. The printed frequency
  anchors (0.050 / 0.100 / 4 / 6 / 7 kHz) sit in `PASSBAND_LOW_HZ` /
  `PLATEAU_LOW_KNEE_HZ` / `RAMP_START_HZ` / `RAMP_END_HZ` /
  `PASSBAND_HIGH_HZ`, and the printed ratio gridlines (50 / 60 /
  46.2 dB) in `FLOOR_LOW_PLATEAU_DB` / `FLOOR_MAIN_PLATEAU_DB` /
  `FLOOR_HIGH_PLATEAU_DB`. The floor reads: 50 dB on 50–100 Hz,
  60 dB plateau (global maximum) on 100 Hz–4 kHz, a log-linear ramp
  60 → 46.2 dB on 4–6 kHz, 46.2 dB plateau on 6–7 kHz, open outside
  the 50 Hz / 7 kHz passband walls. The helper trio
  `classify(f)` / `evaluate(f, ratio_db)` / `min_ratio_db(f)`
  mirrors the sibling masks; `min_ratio_db` interpolates the ramp
  log-linearly in frequency. 22 new unit tests anchor every
  breakpoint and gridline at its printed value, exercise
  classification across all six bands, pin the ramp's log-linear
  geometric-mean midpoint and strict descent, sweep the 60 dB main
  plateau as the global maximum and the monotone-non-increasing
  post-plateau floor, pin the floor boundary / NaN / outside-mask
  semantics, and lock the structural alignments (50 Hz / 7 kHz walls
  = clause 2.4.1 passband = the 50–7000 Hz measurement window;
  4 kHz plateau edge = QMF band split; 46.2 dB high plateau sits
  below the clause 2.5.5 "about 6 kHz" 50 dB plateau).
- **Round-277 clause 2.5.4 receive-audio-part idle-noise bound +
  clause 2.5.5 / Figure 14 signal-to-total-distortion-vs-input-level
  mask.** New `transmission::signal_to_distortion` sub-module
  surfaces the signal-to-total-distortion-ratio versus input-level
  mask of Figure 14/G.722 (p. 13) referenced by clause 2.5.5 (p. 13)
  of the staged ITU-T G.722 (11/88) Recommendation. Clause 2.5.5
  prescribes two measurements — "about 1 kHz" and "about 6 kHz",
  modeled as the `MeasurementTone` enum with
  `nominal_frequency_hz()` / `knee_dbm0()` / `plateau_db()`
  accessors — and the figure draws one mask curve per tone. The
  printed input-level anchors (−56 / −21 / −11 / +8 dBm0) sit in
  `INPUT_LEVEL_LOW_DBM0` / `KNEE_TONE_HIGH_DBM0` /
  `KNEE_TONE_LOW_DBM0` / `INPUT_LEVEL_HIGH_DBM0` and the printed
  ratio gridlines (15 / 50 / 60 dB) in `FLOOR_AT_LOW_EDGE_DB` /
  `PLATEAU_TONE_HIGH_DB` / `PLATEAU_TONE_LOW_DB`. The three printed
  corners are collinear on a slope-1 diagonal
  (`ratio = level + 71 dB`, `DIAGONAL_OFFSET_DB`), so each curve's
  floor is continuous: shared diagonal from (−56, 15), then a 60 dB
  plateau from −11 dBm0 (1 kHz) or a 50 dB plateau from −21 dBm0
  (6 kHz), both ending at the +8 dBm0 right wall — 1 dB under the
  clause 2.2 overload point. The helper trio
  `classify(tone, level)` / `evaluate(tone, level, ratio_db)` /
  `min_ratio_db(tone, level)` mirrors the sibling masks,
  floor-flavoured (`NEG_INFINITY` = no constraint outside the span;
  measurements must sit at or above the floor). The `MaskBand` enum
  partitions the level axis into `BelowMask` / `Diagonal` /
  `Plateau` / `AboveMask`. Also surfaces the clause 2.5.4 (p. 13)
  receive-audio-part idle-noise bound as
  `transmission::RECEIVE_AUDIO_PART_IDLE_NOISE_MAX_DBM0` (−75 dBm0,
  unweighted 50–7000 Hz, 14-bit all-zero input — 9 dB stricter than
  the end-to-end clause 2.4.4 narrow-band limit). 23 new unit tests
  (22 in the new sub-module plus a `transmission` mod-level test)
  anchor every printed coordinate, pin the corner collinearity and
  knee continuity, sweep floor monotonicity / plateau-maximality on
  a 0.5 dB grid for both tones, check the strict 1 kHz-over-6 kHz
  ordering between the knees, exercise exact-floor boundary
  semantics and NaN handling, and lock the structural alignments
  (right wall = overload − 1 dB; tones straddle the 4 kHz QMF band
  split; the 1020 Hz clause 2.3 reference qualifies as "about
  1 kHz"; the unweighted window is the familiar 50–7000 Hz band;
  the clause 2.5.4 bound is stricter than both clause 2.4.4 codec
  bounds).

- **Round-269 clause 2.5.3 / Figure 13 group-delay-distortion mask.**
  New `transmission::group_delay_distortion` sub-module surfaces the
  group-delay-distortion versus frequency mask of Figure 13/G.722
  (p. 13) referenced by clause 2.5.3 (p. 13) of the staged ITU-T
  G.722 (11/88) Recommendation — the audio-parts companion of clause
  2.4.3's absolute group-delay limit, measured with the minimum group
  delay as reference in the looped audio-to-audio configuration of
  Figure 9b)/G.722 (p. 10). The printed frequency anchors (50 Hz /
  100 Hz / 300 Hz / 4 kHz / 6.4 kHz / 7 kHz) are exposed as
  `LOW_SHOULDER_LOW_HZ` / `CORE_LOW_HZ` / `CORE_HIGH_HZ` /
  `HIGH_SHOULDER_HIGH_HZ` / `MASK_HIGH_EDGE_HZ` (the 50 Hz anchor
  reuses `NOMINAL_PASSBAND_LOW_HZ`); the printed ms ceilings (0.25 /
  1 / 2 / 4 ms) sit in `CORE_MAX_MS` / `SHOULDER_MAX_MS` /
  `HIGH_TRANSITION_MAX_MS` / `LOW_TRANSITION_MAX_MS`. The `MaskBand`
  enum partitions the frequency axis into seven bands (`BelowMask`,
  `LowTransition`, `LowShoulder`, `Core`, `HighShoulder`,
  `HighTransition`, `AboveMask`) and the helper trio `classify(f)` /
  `evaluate(f, distortion_ms)` / `max_distortion_ms(f)` mirrors the
  sibling masks, ceiling-only (the distortion is non-negative by
  construction since the reference is the band minimum). 25 new unit
  tests anchor every printed breakpoint and ceiling, exercise
  classification across all seven bands, pin the
  stricter-band-owns-the-breakpoint convention at every printed
  edge, sweep the 0.25 ms core as the global staircase minimum, lock
  the 100 Hz / 6.4 kHz / 7 kHz anchors against the Figure 10 mask's
  breakpoint set, and pin the structural alignments (4 kHz core edge
  = QMF band-split = `SUBBAND_SAMPLE_CLOCK_HZ` / 2; 7 kHz right wall
  = `NOMINAL_PASSBAND_HIGH_HZ`; 4 ms top step printed equal to
  `ABSOLUTE_GROUP_DELAY_MAX_MS`).

- **Round-262 clause 2.4.2 / Figure 10 codec end-to-end
  attenuation/frequency-distortion mask.** New
  `transmission::attenuation_distortion` sub-module surfaces the
  attenuation/frequency mask of Figure 10/G.722 (p. 11) referenced by
  clause 2.4.2 (p. 9) of the staged ITU-T G.722 (11/88) Recommendation
  — the **end-to-end codec** mask measured at test point B
  (Figure 2/G.722 p. 2) with a sine input at test point A in the
  looped configuration of Figure 9/G.722 (p. 10), distinct from the
  filter-only masks of clauses 2.5.1 / 2.5.2 already pinned by
  r258 / r237. The printed frequency anchors (50 Hz / 100 Hz /
  6.4 kHz / 7 kHz / 8 kHz) are exposed as `PASSBAND_LOW_HZ` /
  `PASSBAND_TIGHT_HIGH_HZ` / `PASSBAND_RELAXED_HIGH_HZ` /
  `MASK_HIGH_EDGE_HZ`; the dB anchors (`−1` lower / `+1` tight upper /
  `+3` relaxed upper) sit in matching `IN_BAND_LOWER_BOUND_DB` /
  `IN_BAND_TIGHT_UPPER_BOUND_DB` / `IN_BAND_RELAXED_UPPER_BOUND_DB`.
  The `MaskBand` enum partitions the frequency axis into six bands
  (`BelowMask`, `LowTransition`, `InBandTight`, `InBandRelaxed`,
  `HighTransition`, `AboveMask`). `classify(f_hz)` /
  `evaluate(f_hz, atten_db)` mirror the receive-side helper trio with
  the addition of `lower_bound_db(f_hz)` / `upper_bound_db(f_hz)`
  accessors that surface the corridor edges directly so a host
  measuring `(frequency, attenuation_dB)` at test point B can read off
  the printed envelope at any frequency. Unlike Figures 11 / 12 the
  mask has no stopband — the right wall sits at the codec's 16 kHz
  sample-clock Nyquist (8 kHz) above which the codec cannot synthesise
  signal; below that wall the `HighTransition` strip (7 – 8 kHz) leaves
  only the `−1` dB lower bound printed and lets the implementation
  pick its own roll-off shape. 28 new unit tests anchor every printed
  breakpoint and ripple bound at its printed value, exercise
  classification across all six bands, pin the corridor edges
  (`+1.1 dB` at 1 kHz fails; `1.0 dB` passes; `−1.0 dB` passes;
  `−1.1 dB` fails), verify the relaxed corridor admits `2.0 dB` at
  6.8 kHz while the tight corridor rejects it, check the
  `lower_bound_db` is `−1` dB across the full 50 Hz – 8 kHz axis,
  pin the corridor-twice-filter-corridor invariant against Figures
  11 / 12 (each filter mask printed corridor is exactly half the codec
  printed corridor on every bound), assert the shared breakpoint set
  with Figures 11 / 12 (100 Hz / 6.4 kHz / 7 kHz), align the right
  wall with the input anti-aliasing filter's stopband entry (8 kHz =
  `SUBBAND_SAMPLE_CLOCK_HZ`), and pin the closed-interval semantics
  at every band boundary.

- **Round-258 clause 2.5.1 / Figure 11 input anti-aliasing filter
  mask.** New `transmission::anti_aliasing_filter` sub-module surfaces
  the attenuation/frequency mask of Figure 11/G.722 (p. 12) referenced
  by clause 2.5.1 (p. 11) of the staged ITU-T G.722 (11/88)
  Recommendation, the transmit-side counterpart of the receive-side
  Figure 12 / `reconstructing_filter` mask landed in r237. Frequency
  anchors (50 Hz / 100 Hz / 6.4 kHz / 7 kHz / 8 kHz / 9 kHz) are
  exposed as `PASSBAND_LOW_HZ` / `PASSBAND_TIGHT_HIGH_HZ` /
  `PASSBAND_RELAXED_HIGH_HZ` / `STOPBAND_ENTRY_HZ` /
  `STOPBAND_SHOULDER_HZ`; the dB anchors match Figure 11's printed
  values (`IN_BAND_LOWER_BOUND_DB` = −0.5,
  `IN_BAND_TIGHT_UPPER_BOUND_DB` = +0.5,
  `IN_BAND_RELAXED_UPPER_BOUND_DB` = +1.5,
  `STOPBAND_ENTRY_MIN_ATTEN_DB` = 25, `STOPBAND_SHOULDER_MIN_ATTEN_DB`
  = 50). Unlike Figure 12 the mask has no 14 kHz / 70 dB anchor — the
  50 dB ceiling extends flat beyond 9 kHz to the band edge — so the
  `MaskBand` enum splits the stopband into `StopbandRamp` (8–9 kHz
  log-linear ramp 25 → 50 dB) and `StopbandFlat` (≥ 9 kHz, flat
  50 dB). `classify(f_hz)` / `evaluate(f_hz, atten_db)` /
  `stopband_floor_db(f_hz)` mirror the receive-side helper trio so a
  caller measuring `(frequency, attenuation_dB)` at test point A
  (Figure 2/G.722 p. 2) can verify the result against the printed
  mask. 29 new unit tests anchor every breakpoint and ripple bound at
  the printed value, exercise classification across all seven bands
  (including the new ramp/flat split), pin the stopband anchor values
  (24 dB at 8 kHz fails, 25 dB passes; 49 dB at 9 kHz fails, 50 dB
  passes), assert monotone non-decreasing behaviour of the floor on a
  100 Hz step grid across 8 kHz – 20 kHz, verify the flat 50 dB
  ceiling above 9 kHz, check the log-linear interpolation invariant on
  the 8–9 kHz ramp, lock the shared-corridor invariant against the
  Figure 12 mask (in-band ripple corridor + 100 Hz / 6.4 kHz / 7 kHz /
  8 kHz / 9 kHz breakpoints + 25 dB / 50 dB anchors), and pin the
  divergence above 9 kHz (Figure 11's 50 dB vs Figure 12's 70 dB at
  14 kHz).

- **Round-237 clause 2.5.2 / Figure 12 output reconstructing filter
  mask.** New `transmission::reconstructing_filter` sub-module surfaces
  the attenuation/frequency mask of Figure 12/G.722 (p. 12) referenced
  by clause 2.5.2 (p. 11) of the staged ITU-T G.722 (11/88)
  Recommendation. The mask's printed frequency anchors (50 Hz / 100 Hz
  / 6.4 kHz / 7 kHz / 8 kHz / 9 kHz / 14 kHz) are exposed as
  `PASSBAND_LOW_HZ` / `PASSBAND_TIGHT_HIGH_HZ` /
  `PASSBAND_RELAXED_HIGH_HZ` / `STOPBAND_ENTRY_HZ` /
  `STOPBAND_SHOULDER_HZ` / `STOPBAND_FAR_HZ`; the dB anchors
  (−0.5 / +0.5 / +1.5 dB in-band, 25 / 50 / 70 dB stopband floor) sit
  in matching `IN_BAND_LOWER_BOUND_DB`, `IN_BAND_TIGHT_UPPER_BOUND_DB`,
  `IN_BAND_RELAXED_UPPER_BOUND_DB`, `STOPBAND_ENTRY_MIN_ATTEN_DB`,
  `STOPBAND_SHOULDER_MIN_ATTEN_DB`, `STOPBAND_FAR_MIN_ATTEN_DB`. The
  `MaskBand` enum partitions the frequency axis into six bands
  matching the figure's piecewise structure (`BelowMask`,
  `LowTransition`, `InBandTight`, `InBandRelaxed`, `HighTransition`,
  `Stopband`); `classify(f_hz)` returns the band; `evaluate(f_hz,
  atten_db)` returns `(MaskBand, bool)` recording whether the printed
  bounds are met; `stopband_floor_db(f_hz)` returns the minimum
  attenuation floor as a log-linear interpolation between the three
  printed stopband anchors (with `NEG_INFINITY` below the stopband
  entry and a flat 70 dB ceiling above 14 kHz). 19 new unit tests
  anchor every breakpoint and ripple bound at the printed value,
  exercise the six bands across `evaluate` (admit / reject), pin the
  stopband anchor values (24 dB at 8 kHz fails, 25 dB passes; same for
  50 / 70 dB at 9 / 14 kHz), assert monotone non-decreasing behaviour
  of the floor on a 100 Hz step grid across 8 kHz – 20 kHz, verify the
  flat 70 dB ceiling above 14 kHz, and check the log-linear
  interpolation invariant (geometric-mean frequency between two
  anchors yields arithmetic-midpoint dB).

- **Round-231 Appendix II.3.2 synthesisable Configuration-2 input
  sequence.** New `test_harness::appendix_ii` sub-module surfaces the
  procedurally-buildable "third" Configuration-2 input sequence of
  Appendix II.3.2 of the staged ITU-T G.722 (11/88) Recommendation —
  the only Appendix-II input sequence fully transcribable from the
  printed PDF (the other two, `T2R1.COD` / `T2R2.COD`, are derived
  from corpus inputs distributed only on PC-DOS / MS-DOS flexible
  disks per clause II.4.6 p. 73). Per-sample helpers
  `lower_msb_bit` / `higher_msb_bit` / `higher_lsb_bit` build the
  eight 2048-bit MSB / LSB sub-sequences whose patterns are spelled
  out in clauses II.3.2.1 (p. 67) and II.3.2.2 (p. 68); `lower_lsb5`
  surfaces the 64-sub-sequence 5-bit-word stream of Table II-4/G.722
  (p. 69) including the wrap-back sub-sequence `(64)` that closes
  the suppressed-codeword range back to the table start (clause
  II.3.2.1 p. 67 footnote). `ilr(idx)` / `ih(idx)` combine the MSB
  and LSB streams into the 6-bit / 2-bit per-sample codewords;
  `build_i_hash_stream()` returns the bare 16 384-word `I#` data
  payload; `build_cod_frame()` wraps it in the 16-word RSS-marker
  prefix / trailer of the `.COD` file-format layout (clause II.4.5.2
  p. 72), yielding the 16 416-word stream whose length matches the
  `T1D3.COD` file-size figure of clause II.4.3 (p. 71). 25 new unit
  tests cover the printed lead-in of each MSB / LSB sub-sequence
  (the 17-bit prefix that the PDF spells out for each), Table II-4
  anchor entries (sub-sequences 1 / 2 / 3 / 31 / 57 / 63 / 64), the
  ILR / IH composition rules, the data-payload + `.COD`-frame
  length / RSS-mask invariants, an INFC round-trip on the packed
  `I#` stream, `run_configuration_2` determinism across two
  independent decoders, full `.COD`-frame RSS-bracket behaviour
  (prefix → reset → valid payload → trailer → reset), and a
  structural invariant on the eight MSB sub-sequences confirming
  both polarities appear in every sub-sequence except the
  constant-1 sub-sequence `(3)` (clause II.3.2.1 p. 67's ±2
  zero-predictor excursion remark).

- **Round-225 Appendix II test-sequence harness.** New `test_harness`
  module surfaces Configuration 1 / Configuration 2 of Appendix II of
  the staged ITU-T G.722 (11/88) Recommendation. Adds QMF-bypass
  entry points on the encoder (`encode_subband_pair`) and decoder
  (`decode_subband_pair`) per clauses II.2.1 and II.2.2 (p. 64), plus
  the four normative sub-blocks INFA / INFB / INFC / INFD of clause
  II.2.3 (p. 65) that translate between the 16-bit `X#` / `I#` /
  `RL#` / `RH#` test-sequence words and the codec's per-sample
  inputs and outputs. Bit-position constants (`RSS_BIT_POSITION`,
  `I_HASH_IL_SHIFT`, `I_HASH_IH_SHIFT`, `RL_HASH_SAMPLE_SHIFT`,
  matching masks) match Figures II-1 / II-2 / II-3 of the staged
  Recommendation. Two convenience walkers `run_configuration_1` /
  `run_configuration_2` thread a caller-supplied test sequence
  through the appropriate codec and handle the RSS reset slot by
  re-initialising the codec and emitting the spec's "non-valid
  data" output word. 28 new unit tests cover each sub-block's
  pseudo-code, INFB ↔ INFC round-trip across all 6+2-bit codeword
  combinations + the RSS bit, encoder QMF-bypass determinism and
  m_L monotonicity at reset, decoder QMF-bypass determinism and
  oversize-codeword field-masking, an end-to-end Configuration-1 →
  Configuration-2 silence walk, and post-RSS state-equivalence
  with a fresh codec for both directions. The Appendix-II
  test-sequence files themselves remain a docs gap (clause II.4
  lists them as PC-DOS / MS-DOS flexible-disk distributions from
  the ITU; they are not staged under `docs/`).

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

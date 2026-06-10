# oxideav-g722

Pure-Rust SB-ADPCM codec for ITU-T G.722 wideband speech / audio at
64 / 56 / 48 kbit/s.

## Status

Round-200 brought the **encoder side** of G.722 online against the
staged Recommendation ITU-T G.722 (11/88) Blue-Book edition. Both
sub-band ADPCM loops are now exercised end-to-end and an encode →
decode round-trip against silence stays inside a tight envelope.

Round-207 closes the open follow-up on `Table 19/G.722` by making
the 5-bit `(SIL, IL5)` inverse-quantizer table bit-faithful to the
printed spec, including the structural anomaly at `RIL = 11111`
where the spec assigns `SIL = -1` despite a set top bit (mirroring
the `111110 / 111111` anomaly that Table 18 already encodes). The
Mode-2 / Mode-3 round-trip silence envelopes are now part of the
test set so the receive path is exercised for all three modes.

Round-218 surfaces clause 2 of the Recommendation as a typed
`transmission` module: clock-rate / sample-clock-tolerance / overload
/ passband / group-delay / idle-noise / single-frequency-noise
constants citing their normative clause numbers, plus dBm0 ↔ uniform
PCM conversion (anchored on clause 2.2's +9 dBm0 overload point) and
an `IdleNoiseReport` end-to-end check that drives encoder → decoder
with digital silence and confirms the resulting PCM-domain RMS sits
under the clause 2.4.4 wideband −60 dBm0 bound for all three modes.

Round-225 surfaces Appendix II of the Recommendation as a typed
`test_harness` module: QMF-bypass entry points on the encoder
(`encode_subband_pair`) and decoder (`decode_subband_pair`) together
with the four normative sub-blocks INFA / INFB / INFC / INFD of
clause II.2.3 (p. 65), the bit-position constants of the `X#` / `I#`
/ `RL#` / `RH#` 16-bit wire-format words, and `run_configuration_1` /
`run_configuration_2` helpers that walk a caller-supplied test
sequence through the appropriate codec configuration.

Round-269 surfaces **clause 2.5.3 / Figure 13/G.722** — the
audio-parts **group-delay-distortion versus frequency** mask — as a
typed `transmission::group_delay_distortion` sub-module: the
*distortion* companion of clause 2.4.3's absolute group-delay limit
(`ABSOLUTE_GROUP_DELAY_MAX_MS`, r218). The printed staircase, with the
minimum group delay as reference, reads 4 ms on 50–100 Hz, 1 ms on
100–300 Hz, 0.25 ms on 300 Hz–4 kHz, 1 ms again on 4–6.4 kHz, 2 ms on
6.4–7 kHz, open outside; ceiling only (the distortion is non-negative
by construction). The helper trio is `classify(f)` /
`evaluate(f, distortion_ms)` / `max_distortion_ms(f)`, with the
stricter band owning every printed breakpoint as in the sibling
masks. 25 new unit tests anchor every breakpoint and ceiling at its
printed value, exercise classification across all seven bands, pin
the breakpoint-ownership convention, sweep the 0.25 ms core as the
staircase's global minimum, lock the shared 100 Hz / 6.4 kHz / 7 kHz
anchors against the Figure 10 mask, and pin the structural
alignments (4 kHz core edge = QMF band-split = half the 8 kHz
sub-band sample clock; 7 kHz right wall = clause 2.4.1 passband
edge; 4 ms top step = the clause 2.4.3 printed value).

Round-262 surfaces **clause 2.4.2 / Figure 10/G.722** — the codec
end-to-end **attenuation/frequency-distortion** mask — as a typed
`transmission::attenuation_distortion` sub-module. Unlike the
filter-only masks of r237 / r258, this is the back-to-back
encoder + decoder + audio-parts mask the spec quotes for the looped
configuration of Figure 9/G.722 (p. 10): the corridor is `[−1, +1]` dB
tight on 100 Hz – 6.4 kHz, `[−1, +3]` dB relaxed on the two
transition strips (50 – 100 Hz and 6.4 – 7 kHz), `≥ −1` dB lower bound
only on 7 – 8 kHz (open above), and the mask's right wall sits at
8 kHz (the Nyquist of the 16 kHz sample clock). The same
`classify(f)` / `evaluate(f, atten_db)` helpers ship alongside
`lower_bound_db(f)` / `upper_bound_db(f)` accessors that surface the
corridor edges directly. 28 new unit tests anchor every printed
breakpoint at the printed value, pin the corridor-twice-filter-corridor
invariant against Figures 11 / 12 (each filter mask printed corridor
is exactly half the codec corridor on every bound), share the
breakpoint set, and align the right wall with the input
anti-aliasing-filter stopband entry.

Round-258 surfaces **clause 2.5.1 / Figure 11/G.722** — the
input-anti-aliasing-filter attenuation-vs-frequency mask — as a typed
`transmission::anti_aliasing_filter` sub-module: the transmit-side
counterpart to the receive-side Figure 12 mask landed in r237. The two
masks share their in-band ripple corridor exactly (±0.5 dB tight on
100 Hz – 6.4 kHz, +1.5 dB relaxed on 6.4 kHz – 7 kHz, −0.5 dB lower)
and their 8 kHz / 25 dB and 9 kHz / 50 dB stopband anchors, but
Figure 11 has no 14 kHz / 70 dB anchor — its 50 dB ceiling extends
flat past 9 kHz to the band edge. The `MaskBand` enum therefore splits
the stopband into `StopbandRamp` (8–9 kHz log-linear ramp) and
`StopbandFlat` (≥ 9 kHz), and the helper trio `classify(f)` /
`evaluate(f, atten_db)` / `stopband_floor_db(f)` mirrors the
receive-side API so a caller measuring `(frequency, attenuation_dB)`
at test point A (Figure 2/G.722 p. 2) can verify the result against
the printed mask. 29 new unit tests anchor every breakpoint at the
printed dB value, exercise classification across all seven bands,
confirm the 8–9 kHz log-linear ramp via geometric-mean checks against
the arithmetic midpoint dB, lock the shared-corridor invariant against
the Figure 12 mask, and pin the divergence above 9 kHz (Figure 11's
50 dB vs Figure 12's 70 dB at 14 kHz).

Round-237 surfaces **clause 2.5.2 / Figure 12/G.722** — the
output-reconstructing-filter attenuation-vs-frequency mask — as a
typed `transmission::reconstructing_filter` sub-module: every printed
breakpoint of the figure (50 Hz / 100 Hz / 6.4 kHz / 7 kHz / 8 kHz /
9 kHz / 14 kHz) and ripple bound (±0.5 dB tight, +1.5 dB relaxed,
25 dB / 50 dB / 70 dB stopband floor) is exposed as a named constant,
plus a `classify(f)` / `evaluate(f, atten_db)` / `stopband_floor_db(f)`
helper trio that lets a caller measuring `(frequency, attenuation_dB)`
at test point B (Figure 2/G.722 p. 2) verify the result against the
printed mask. The previously-open clause-2.4.4 narrow-band-noise
follow-up of round-218 — which sat above the −60 dBm0 wideband bound
because the reconstructing-filter shape was not yet pinned — now has a
typed mask to bolt onto a host filter implementation. 19 new unit tests
anchor every breakpoint at the printed dB value, exercise the
log-linear interpolation of the 8–9 kHz / 9–14 kHz stopband segments
(including geometric-mean midpoint checks against the arithmetic
midpoint dB), confirm classification across all six bands, and verify
the mask rejects measurements outside the ripple corridor.

Round-231 surfaces the **synthesisable Appendix II.3.2 third
Configuration-2 input sequence** as a typed `test_harness::appendix_ii`
sub-module: every per-sample 6-bit `ILR` and 2-bit `IH` codeword of
the 16 384-value artificial sequence is generated procedurally from
the bit patterns printed in clauses II.3.2.1 + II.3.2.2 (p. 67–68) and
Table II-4/G.722 (p. 69), and `build_cod_frame` wraps the payload in
the 16-word RSS-marker prefix / trailer of the `.COD` file format
(clause II.4.5.2 p. 72) for a 16 416-word stream matching the
file-size figure quoted in clause II.4.3 (p. 71). This unblocks the
spec-derived end-to-end exercise of the receive path that did not
require the ITU disk distribution; the two encoder-derived sequences
of clause II.3.2 (`T2R1.COD`, `T2R2.COD`) and the comparison output
files (`T3L*.RC*`, `T3H*.RC*`) remain a docs gap (they are not
synthesisable from the printed PDF).

Coverage:

| Path     | Spec coverage | Notes                                                                                              |
| -------- | ------------- | -------------------------------------------------------------------------------------------------- |
| Encoder  | structural    | Transmit QMF (clause 3.1), BLOCK 1L QUANTL + BLOCK 1H QUANTH forward quantizers, shared predictor. |
| Decoder  | structural    | Lower (4/5/6-bit modes) + higher (2-bit) inverse ADPCM, 24-tap receive QMF.                        |
| Test vectors | partial   | Synthesised Appendix II.3.2 third sequence (`test_harness::appendix_ii`); ITU disk corpus not staged. |

### Implemented in r258

- New `transmission::anti_aliasing_filter` sub-module surfacing the
  attenuation/frequency mask of Figure 11/G.722 (p. 12, clause 2.5.1
  page 11) for the transmit audio part's input anti-aliasing filter —
  the transmit-side counterpart to the receive-side
  `transmission::reconstructing_filter` mask landed in r237:
  - `PASSBAND_LOW_HZ` / `PASSBAND_TIGHT_HIGH_HZ` /
    `PASSBAND_RELAXED_HIGH_HZ` / `STOPBAND_ENTRY_HZ` /
    `STOPBAND_SHOULDER_HZ` — the 100 Hz / 6.4 kHz / 7 kHz / 8 kHz /
    9 kHz frequency anchors printed on the figure's log axis (the
    50 Hz low anchor reuses the existing `NOMINAL_PASSBAND_LOW_HZ`).
  - `IN_BAND_LOWER_BOUND_DB` (−0.5 dB), `IN_BAND_TIGHT_UPPER_BOUND_DB`
    (+0.5 dB), `IN_BAND_RELAXED_UPPER_BOUND_DB` (+1.5 dB),
    `STOPBAND_ENTRY_MIN_ATTEN_DB` (25 dB), `STOPBAND_SHOULDER_MIN_ATTEN_DB`
    (50 dB) — the printed dB values on the attenuation axis.
  - `MaskBand` enum with seven variants matching the figure's
    piecewise structure: `BelowMask`, `LowTransition`, `InBandTight`,
    `InBandRelaxed`, `HighTransition`, `StopbandRamp` (8–9 kHz
    log-linear ramp 25 → 50 dB), `StopbandFlat` (≥ 9 kHz flat 50 dB).
  - `classify(f_hz)` / `evaluate(f_hz, atten_db)` /
    `stopband_floor_db(f_hz)` — the same helper trio as the receive
    side; `stopband_floor_db` log-linearly interpolates the 8–9 kHz
    ramp and returns the flat 50 dB ceiling above 9 kHz.
- 29 new unit tests anchoring every breakpoint and ripple bound to its
  printed value, exercising classification across all seven bands,
  pinning the stopband anchor checks (24 dB at 8 kHz fails, 25 dB
  passes; 49 dB at 9 kHz fails, 50 dB passes; 50 dB at 14 kHz passes),
  confirming the floor is monotone non-decreasing on a 100 Hz step
  grid, verifying the flat 50 dB ceiling above 9 kHz, checking the
  geometric-mean log-linear interpolation invariant on the 8–9 kHz
  ramp, locking the shared in-band ripple corridor + 100 Hz / 6.4 kHz
  / 7 kHz / 8 kHz / 9 kHz breakpoints + 25 dB / 50 dB stopband anchors
  against the Figure 12 mask, and pinning the 14 kHz divergence
  (50 dB Figure 11 vs 70 dB Figure 12 = 20 dB headroom difference
  matching the missing 14 kHz / 70 dB anchor).

### Implemented in r237

- New `transmission::reconstructing_filter` sub-module surfacing the
  attenuation/frequency mask of Figure 12/G.722 (p. 12, clause 2.5.2
  page 11) for the receive audio part's output reconstructing filter:
  - `PASSBAND_LOW_HZ` / `PASSBAND_TIGHT_HIGH_HZ` /
    `PASSBAND_RELAXED_HIGH_HZ` / `STOPBAND_ENTRY_HZ` /
    `STOPBAND_SHOULDER_HZ` / `STOPBAND_FAR_HZ` — the 100 Hz / 6.4 kHz
    / 7 kHz / 8 kHz / 9 kHz / 14 kHz frequency anchors printed on the
    figure's log axis (the 50 Hz low anchor reuses the existing
    `NOMINAL_PASSBAND_LOW_HZ`).
  - `IN_BAND_LOWER_BOUND_DB` (−0.5 dB), `IN_BAND_TIGHT_UPPER_BOUND_DB`
    (+0.5 dB), `IN_BAND_RELAXED_UPPER_BOUND_DB` (+1.5 dB),
    `STOPBAND_ENTRY_MIN_ATTEN_DB` (25 dB), `STOPBAND_SHOULDER_MIN_ATTEN_DB`
    (50 dB), `STOPBAND_FAR_MIN_ATTEN_DB` (70 dB) — the printed dB
    values on the attenuation axis (sign convention: attenuation
    positive).
  - `MaskBand` enum — six bands matching the figure's piecewise
    constant / piecewise log-linear segments: `BelowMask`,
    `LowTransition`, `InBandTight`, `InBandRelaxed`, `HighTransition`,
    `Stopband`.
  - `classify(f_hz)` — drops a measured frequency into the matching
    `MaskBand`.
  - `evaluate(f_hz, atten_db)` — returns `(MaskBand, bool)` where the
    `bool` records whether the printed mask is met at that frequency.
  - `stopband_floor_db(f_hz)` — log-linear interpolation between the
    three printed stopband anchors (25 dB @ 8 kHz, 50 dB @ 9 kHz,
    70 dB @ 14 kHz), with `f64::NEG_INFINITY` returned below the
    stopband entry and a flat 70 dB ceiling above 14 kHz.
- 19 new unit tests anchoring every breakpoint and ripple bound to its
  printed value, exercising classification across all six bands,
  asserting the tight ripple region rejects ±0.6 dB and admits 0 dB,
  asserting the relaxed region admits 1.0 dB and rejects 1.6 dB,
  pinning the stopband anchor checks (24 dB at 8 kHz fails, 25 dB
  passes; same for 50 / 70 dB at 9 / 14 kHz), confirming the floor is
  monotone non-decreasing on a 100 Hz step grid, verifying the flat
  70 dB ceiling above 14 kHz, and checking the geometric-mean of two
  log-axis anchors maps to the arithmetic midpoint of their dB values
  (the log-linear interpolation invariant).

### Implemented in r231

- New `test_harness::appendix_ii` sub-module exposing the
  synthesisable third Configuration-2 input sequence of clause II.3.2
  (p. 67–68):
  - `lower_msb_bit(bit_idx)` — the 8-sub-sequence MSB stream of the
    lower-sub-band 6-bit `ILR` codeword (clause II.3.2.1 p. 67). Each
    sub-sequence is 2048 bits (`SUBSEQUENCE_LEN_BITS`); the eight
    patterns of the spec text resolve to periods 3 / 8 / 1 / 4 / 2 /
    8 / 5 / 5.
  - `higher_msb_bit(bit_idx)` — clause II.3.2.2 (p. 68) makes this
    identical to `lower_msb_bit`.
  - `higher_lsb_bit(bit_idx)` — the 8-sub-sequence LSB stream of the
    higher-sub-band 2-bit `IH` codeword (clause II.3.2.2 p. 68).
  - `lower_lsb5(sample_idx)` — the 64-sub-sequence 5-bit-word stream
    of the lower-sub-band 6-bit codeword's lower five bits, derived
    from Table II-4/G.722 (p. 69). Sub-sequence `(64)` reads
    "alternating sixteen 0's, sixteen 3's" — the spec's wrap that
    closes the suppressed-codeword range back to the table start
    (clause II.3.2.1 p. 67 footnote).
  - `ilr(sample_idx)` — packs `(MSB << 5) | LSB5` into the 6-bit
    `ILR` codeword. `ih(sample_idx)` — packs `(MSB << 1) | LSB` into
    the 2-bit `IH` codeword.
  - `build_i_hash_stream()` — the bare 16384-word `I#` data payload
    with RSS cleared (matches `ARTIFICIAL_SEQUENCE_LEN`).
  - `build_cod_frame()` — the 16 416-word `T1D3.COD`-shape frame: a
    16-word RSS-marker prefix (LSB=1, others=0), 16384 data words
    with RSS cleared, and a 16-word RSS-marker trailer (clause
    II.4.5.2 p. 72; clause II.4.3 p. 71 file-size figure).
- 25 new unit tests covering the printed-prefix sanity of each MSB /
  LSB sub-sequence (the 17-bit lead-in spelled out in the PDF for each
  pattern), Table II-4 anchor entries (1 / 2 / 3 / 31 / 57 / 63 / 64),
  the `ILR` / `IH` composition rules, the data-payload + `.COD`-frame
  length / RSS-mask invariants, an INFC round-trip through the packed
  `I#` stream, a `run_configuration_2` determinism check across two
  independent decoders, full `.COD`-frame RSS-bracket round-trip
  (prefix → reset → valid payload → trailer → reset), and a structural
  invariant on the eight MSB sub-sequences confirming both polarities
  appear in every sub-sequence except the constant-1 sub-sequence (3)
  (clause II.3.2.1 p. 67's ±2 zero-predictor excursion remark).

- New `test_harness` module exposing Appendix II of the staged
  Recommendation:
  - `Encoder::encode_subband_pair(x_l, x_h)` — Configuration-1
    QMF-bypass entry point on the encoder (clause II.2.1 p. 64):
    drives the two sub-band ADPCM encoders directly with already-
    split sub-band inputs and emits the multiplexed octet.
  - `Decoder::decode_subband_pair(i_lr, i_h)` — Configuration-2
    QMF-bypass entry point on the decoder (clause II.2.2 p. 64):
    drives the two inverse quantiser / predictor loops directly
    and returns the per-sub-band `LIMIT`-bounded reconstructed
    signals `(R_L, R_H)`.
  - `test_harness::{infa, infb, infc, infd}` — the four normative
    sub-blocks of clause II.2.3 (p. 65) that translate between the
    16-bit `X#` / `I#` / `RL#` / `RH#` test-sequence words and the
    per-sample encoder / decoder inputs and outputs. The reset /
    sync signal `RSS` (LSB of every test-sequence word) is decoded
    and propagated through the harness.
  - `RSS_BIT_POSITION` / `RSS_MASK` / `I_HASH_IL_SHIFT` /
    `I_HASH_IH_SHIFT` / `I_HASH_IL_MASK` / `I_HASH_IH_MASK` /
    `RL_HASH_SAMPLE_SHIFT` — wire-format bit-position constants
    matching the INFA / INFB / INFC / INFD packs of Figures II-1 /
    II-2 / II-3 of G.722.
  - `run_configuration_1` and `run_configuration_2` — convenience
    walkers that thread an `X#` or `I#` input sequence through the
    appropriate codec and return the matching output sequence(s),
    handling the RSS reset slot by re-initialising the codec and
    emitting the "non-valid data" output word per the spec.
- 28 new unit tests covering each sub-block's pseudo-code (INFA
  arithmetic right shift, INFB zero-fill on RSS, INFC field
  extraction, INFD shifted sample + clamp), INFB↔INFC round-trip
  across all 6+2-bit codeword combinations + the RSS bit, bit-field
  position constants matching Appendix II Figures II-1..3, encoder
  QMF-bypass determinism and m_L monotonicity at reset, decoder
  QMF-bypass determinism and oversize-codeword masking, end-to-end
  Configuration-1 → Configuration-2 silence walk, and post-RSS
  state-equivalence with a fresh codec for both directions.

### Implemented in r218

- New `transmission` module exposing the normative limits of clause 2
  of the Recommendation: `BIT_CLOCK_HZ` / `OCTET_CLOCK_HZ` /
  `PCM_SAMPLE_CLOCK_HZ` / `SUBBAND_SAMPLE_CLOCK_HZ` (clause 1.6
  page 8), `SAMPLE_CLOCK_TOLERANCE_PPM` (clause 2.2),
  `OVERLOAD_POINT_DBM0` + `OVERLOAD_POINT_TOLERANCE_DB` (clause 2.2),
  `NOMINAL_REFERENCE_FREQUENCY_HZ` (clause 2.3),
  `NOMINAL_PASSBAND_LOW_HZ` / `NOMINAL_PASSBAND_HIGH_HZ` (clause
  2.4.1), `ABSOLUTE_GROUP_DELAY_MAX_MS` (clause 2.4.3),
  `IDLE_NOISE_MAX_DBM0_NARROWBAND` / `_WIDEBAND` (clause 2.4.4) and
  `SINGLE_FREQUENCY_NOISE_MAX_DBM0` (clause 2.4.5).
- `uniform_pcm_full_scale` / `dbm0_to_uniform_pcm` /
  `uniform_pcm_rms_to_dbm0` / `uniform_pcm_rms` bridging dBm0
  (clause 2.2's +9 dBm0 overload-point reference) and the 14-bit
  uniform-PCM domain of clause 1.4.1.
- `measure_idle_noise` + `IdleNoiseReport` driving encoder → decoder
  with digital silence and reporting the receive-side RMS in both
  PCM and dBm0 terms. The digital floor sits at ≈ −63 dBm0 across
  all three modes — passes the clause 2.4.4 wideband (−60 dBm0)
  bound; the narrow-band (−66 dBm0) bound is a follow-up that
  depends on clause 2.5.2's reconstructing-filter mask (not yet
  surfaced).

### Implemented in r207

- Table 19/G.722 (5-bit `(SIL, IL5)` inverse quantizer used by
  Mode-2 reception) made bit-faithful to the printed spec on p. 40:
  `RIL = 11111` now resolves to `(SIL = -1, IL5 = 1)` per the
  printed table (it sits in the SIL = -1 column despite a set top
  bit, the same structural shape as `Table 18`'s `111110 / 111111`
  entries).
- Mode-2 and Mode-3 encoder → decoder round-trip silence-envelope
  tests added, exercising the previously-uncovered reception modes.
- New tests covering the Table-19 substituted-codeword footnote
  (`00000 / 00001`), the 11111 anomaly, and the `IL5 ∈ 1..=15`
  range invariant.

### Implemented in r200

- §3.1 / clause 5.2.1 Transmit QMF — 24-tap analysis bank that splits a
  16 kHz uniform-PCM input into 8 kHz lower / higher sub-band streams
  per eqs 3-1..3-4 using the Table 11/G.722 symmetric coefficients.
- §3.3 BLOCK 1L QUANTL — 60-level lower-sub-band forward adaptive log
  quantizer, transcribed from the p. 42 pseudo-code (including Note 2
  exclusion of LDL == LDU rows).
- §3.3 BLOCK 1H QUANTH — 4-level higher-sub-band forward adaptive
  quantizer.
- §1.4.4 Multiplexer — packs (I_H, I_L) into the 64 kbit/s octet
  format `I_H1 I_H2 I_L1 I_L2 I_L3 I_L4 I_L5 I_L6` (I_H1 = MSB).
- Table 16/G.722 (`ILP6_FROM_ML` / `ILN6_FROM_ML`) and Table 20/G.722
  (`IHP2_FROM_MH` / `IHN2_FROM_MH`) — encoder forward output codes.
- Internal refactor: lifted the shared SB-ADPCM predictor + scale-factor
  adaptation into `predictor.rs` so the encoder and decoder drive a
  single source of truth (clauses 3.5 / 3.6).

### Implemented previously (r185)

- §1.3 Modes 1 / 2 / 3 (Table 1, page 3) with mid-stream mode switching.
- §6.2.1.2 / 6.2.1.5 INVQAL + INVQBL inverse adaptive quantizers.
- §6.2.1.3 LOGSCL + SCALEL Method 2 (32-entry log-to-linear table).
- §6.2.1.4 PARREC + FILTEZ + FILTEP + PREDIC + UPPOL1 + UPPOL2 +
  UPZERO lower-sub-band adaptive predictor.
- §6.2.1.6 LIMIT output saturation.
- §6.2.2 the symmetric higher-sub-band ADPCM blocks 2H / 3H / 4H / 5H
  including the 2-bit inverse quantizer and SCALEH Method 2.
- §5.2.2 receive QMF with the 24-tap symmetric filter (Table 11/G.722).

### Not yet implemented

- Appendix III / IV packet-loss concealment.
- Annex B superwideband extension (50–14 000 Hz).
- Annex D stereo extension.
- Bit-exact validation of the receive path against the
  encoder-derived Configuration-2 input sequences `T2R1.COD` /
  `T2R2.COD` and the comparison output files `T3L*.RC*` / `T3H*.RC*`
  — round-225 wires the Configuration-1 / Configuration-2 harness
  (`test_harness` module) and round-231 surfaces the synthesisable
  Appendix II.3.2 third input sequence (`test_harness::appendix_ii`),
  but the disk-distributed corpora of clause II.4 (PC-DOS / MS-DOS
  flexible-disk distributions from the ITU) are not staged under
  `docs/` and are necessary for byte-exact comparison.
- Clause 2.4.3 absolute-group-delay measurement, clause 2.4.5
  selective-single-frequency-noise check and clause 2.4.6
  signal-to-total-distortion ratio (marked "under study" in the
  staged 1988 base edition). Clause 2.4.2 / Figure 10 codec
  attenuation/frequency-distortion mask is now typed by r262
  (`transmission::attenuation_distortion`) and clause 2.5.3 /
  Figure 13 group-delay-distortion mask by r269
  (`transmission::group_delay_distortion`); end-to-end measurement
  against the printed masks still requires both audio parts to be
  brought into the loop.
- The remaining audio-parts clauses of 2.5: clause 2.5.4 receive-part
  idle noise (−75 dBm0 constant), clause 2.5.5 / Figure 14
  signal-to-total-distortion vs input level, clause 2.5.6 / Figure 15
  signal-to-total-distortion vs frequency, clause 2.5.7 / Figure 16
  gain variation vs input level, and the clause 2.5.9 go/return
  crosstalk limits (clause 2.5.8 intermodulation is "under study").
- Both clause 2.5.1 / Figure 11 (input anti-aliasing) and clause
  2.5.2 / Figure 12 (output reconstructing filter) are now typed
  (r258 `transmission::anti_aliasing_filter` and r237
  `transmission::reconstructing_filter`); the host system's actual
  analogue filter implementations still have to be brought into the
  picture for end-to-end clause-2.4.4 idle-noise verification (see
  the next bullet).
- Bringing the host system's reconstructing filter into the
  end-to-end clause 2.4.4 idle-noise check — the r237 mask gives the
  shape; an actual digital reconstructing filter implementation is
  needed to apply it before the RMS measurement so the narrow-band
  −66 dBm0 bound can be checked rather than the looser wideband
  −60 dBm0 bound.

### Open follow-ups

- Both `IL5_FROM_IL5` and `IL4_FROM_IL4` are now strictly bit-faithful
  to the printed Table 19/G.722 and Table 17/G.722 respectively (the
  r200 caveat was closed by the round-207 fix above). Mode-1, Mode-2
  and Mode-3 encoder → decoder round-trip silence envelopes are
  green; bit-exact validation against Appendix-II digital test
  sequences still awaits a staged G.191 fixture corpus.

## Usage

```rust
use oxideav_g722::{Decoder, Encoder, Mode};

// Encode 16 kHz uniform-PCM samples into G.722 octets.
let mut encoder = Encoder::new();
let pcm: Vec<i32> = read_pcm_samples();
let bitstream: Vec<u8> = encoder.encode(&pcm);

// Decode them back to 16 kHz samples.
let mut decoder = Decoder::new(Mode::Mode1);
let samples = decoder.decode(&bitstream);
assert_eq!(samples.len(), bitstream.len() * 2);
```

Both directions are also reachable via the historical factory entry
points: `oxideav_g722::make_encoder()` and
`oxideav_g722::make_decoder(Mode::Mode1)`.

## Provenance

All numeric tables, decision rules and adaptation arithmetic in this
crate were transcribed by hand from the printed normative tables of
`docs/audio/g722/T-REC-G.722-198811-S.pdf` (the Blue-Book base edition
of the Recommendation). Per-table provenance citations sit next to
each constant in `src/tables.rs`. No external source code, no
external reference implementation, and no online resources were
consulted during the rebuild.

## License

MIT — see [LICENSE](LICENSE).

# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.0.8](https://github.com/OxideAV/oxideav-g722/compare/v0.0.7...v0.0.8) - 2026-07-10

### Other

- pcm16_convention fuzz target + CHANGELOG entry for the pcm16 hardening
- robustness totality coverage for the 16-bit PCM entry points
- README + CHANGELOG — r405 ITU-corpus bit-exactness, pcm16 API, corrected-arithmetic notes
- ITU conformance corpus wired bit-exact both directions; three arithmetic bugs fixed
- README + CHANGELOG — full-domain 2.4.2 sweep, 2.4.1 bandwidth, Figure 16 gain-variation gates
- codec-loop gain-variation vs input level pinned against the Figure 16 corridor (selective meter, all three modes characterized)
- clause 2.4.2 sweep across the full Figure 10 mask domain + operational clause 2.4.1 nominal 3-dB bandwidth
- README + CHANGELOG — r401 operational clause-2 completion, robustness fixes, fuzz scaffold
- decoder header cited the wrong Recommendation edition
- cargo-fuzz scaffold — four targets over the bitstream-facing surface (decode / encode-roundtrip / QMF-bypass / aux channel)
- fix clippy unused-parens in robustness closures
- bitstream-surface robustness — two latent out-of-domain fixes + 8 adversarial totality tests
- operational clause 2.4.3 absolute group delay — two-tone phase-slope measurement, <= 4 ms enforced on the looped codec
- frequency-resolved idle-channel conformance — clause 2.4.4 narrow-band + clause 2.4.5 selective limits enforced operationally
- whole-codec signal-to-total-distortion quality gates (clause 2.4.6 'Under study' pinned empirically)
- transmission::spectrum — selective / band-limited measurement primitives for the clause 2.4 limits

### Fixed

- **Three bit-exactness bugs exposed by the ITU-T G.191 G.722
  conformance corpus** (round 405; all three affected every mode on
  every octet once signal was present):
  1. **Table 14 QQ4 addressing off by one.** The printed address
     column is 1-based row numbering, but Table 17's `IL4` output is
     0-based — `QQ4(IL4)` maps to `row = IL4 + 1` exactly like the
     adjacent `WL` column. The old reading shifted every 4-bit
     inverse-quantizer magnitude one row low *and dropped the top
     output 2557 entirely*, biasing the INVQAL predictor-update
     difference `DLT` on every octet (INVQAL feeds adaptation in all
     three modes, and Mode 3's INVQBL directly).
  2. **FILTEP pole-section delay-line timing.** The reconstructed
     signal latch routed `RLT` through an extra delay slot, so the
     pole predictor saw `r(n−2)/r(n−3)` instead of the clause 6.2.1.4
     `RLT1 = r(n−1)` / `RLT2 = r(n−2)`.
  3. **UPPOL1 stability window input.** The `|APL1| ≤ 15360 − APL2`
     bound (eq 3-36) was computed against the *delayed* `AL2`; the
     UPPOL1 sub-block's named input is the freshly updated `APL2`.

  With the fixes the codec is bit-exact against the full corpus:
  encoder 48 768 / 48 768 octets, decoder 97 536 / 97 536 samples in
  each of Modes 1 / 2 / 3. The spec-pseudo-code golden vectors,
  Appendix-II harness anchors and operational-measurement gates were
  re-derived accordingly (the idle channel now *hunts* within ±3 LSB
  of digital silence instead of freezing on a constant — the corpus
  corroborates the drift, which follows from `QQ4(1) = 150` making the
  silence code-word's `DLT = +1`).

### Added

- **ITU-T G.191 conformance corpus wiring** (`tests/itu_conformance.rs`):
  bit-exact encoder and per-mode decoder legs against
  `docs/audio/g722/conformance/` (graceful skip when `docs/` is absent),
  plus committed ~68 KiB prefix excerpts (`tests/data/`) that keep a
  genuine corpus prefix bit-exact in standalone CI, an
  encode→decode chain leg, container-framing assertions
  (word-per-octet `.cod` high bytes), fixture-drift guards, and a
  pinned documentation of the corpus's `codsp.cod` anomaly (it does
  *not* carry the same codewords as `codspw.cod`).
- **16-bit PCM entry points**: `Encoder::encode_pcm16` /
  `encode_pcm16_pair` / `encode_pcm16_into` and
  `Decoder::decode_pcm16` / `decode_octet_pcm16` / `decode_pcm16_into`.
  Full-scale 16-bit uniform PCM in/out with the QMF normalisation
  rescaled by one bit (clause 5.2 Note 2 freedom) — the convention the
  conformance corpus is bit-exact under, and *not* equivalent to
  shifting samples at the API boundary (the extra bit participates in
  all 24 filter products).
- **pcm16 hardening**: robustness totality tests for the new entry
  points (arbitrary-octet decode totality + determinism per mode,
  decode shadowing within one fine LSB of 2× the Table 9 path,
  full-`i16`-domain encode with rail-polarity sanity, wire-invisible
  odd chunking, reset-to-fresh equality) and a fifth cargo-fuzz target
  `pcm16_convention` asserting the shadowing and chunk-transparency
  invariants under fuzz-chosen streams.

- **Round-401 operational clause-2 conformance: the complete measurable
  set.** New `transmission::spectrum` module (exact least-squares
  single-sinusoid fit, one-bin DFT via the second-order recurrence,
  band-limited / peak-bin scans, Hz↔bin edge mapping — 12 unit tests)
  plus three codec-loop measurement surfaces built on it: (1)
  `measure_signal_to_distortion` — clause 2.4.6 prints the codec-loop
  signal-to-total-distortion requirement as "Under study" (no mask),
  so six quality gates pin the *measured* behaviour with ≈ 2 dB
  headroom: per-mode/per-level S/D floors at the 1020 Hz clause 2.3
  reference (Mode 1: 23.7–33.6 dB, Mode 2: 21.2–27.6 dB, Mode 3:
  10.3–15.2 dB across −40…0 dBm0), strict Table 1 mode ordering,
  higher-band (6 kHz) mode independence within 1 dB, adaptive-quantizer
  level tracking (≤ 12 dB spread over a 40 dB stimulus range), and
  recovered-component level accuracy; (2) `measure_idle_channel_spectrum`
  — the clause 2.4.4 **narrow-band** (50–7000 Hz ≤ −66 dBm0) idle bound
  and the clause 2.4.5 **selective** single-frequency bound (≤ −70 dBm0,
  the called-out 8000 Hz bin pinned explicitly) enforced per DFT bin at
  the digital boundary for all three modes, previously unreachable via
  the wideband RMS; with a structural anchor that the idle steady state
  is a pure constant ((r_L, r_H) = (1, 0) → +1 LSB in Mode 1, exactly 0
  in Modes 2/3), so all idle energy is DC and the margins are
  structural; (3) `measure_group_delay` — two-tone phase-slope reading:
  clause 2.4.3 absolute group delay ≤ 4 ms enforced (measured ~22
  samples ≈ 1.38 ms — the QMF cascade delay, matching the joint-QMF
  impulse test's fixed delay index 22 — flat across an 11-frequency
  50–7000 Hz sweep in all three modes, ≈ 2.9× headroom). Additionally:
  the clause 2.4.2 operational sweep now walks the **entire Figure 10
  mask domain** (previously 100–3400 Hz only): the 50–100 Hz low
  transition, both in-band corridors across the 4 kHz QMF split, the
  printed breakpoints, and the 7–8 kHz high transition (measured worst
  −0.47 dB at 4.5 kHz vs the −1 dB bound); a clause 2.4.1 gate pins the
  **nominal 3-dB bandwidth** operationally (end-to-end loss at the 50 /
  7000 Hz band edges within 3 dB of the 1020 Hz reference — the loop is
  flat to hundredths of a dB, leaving the full budget to the audio-part
  filters); and the clause 2.5.7 / Figure 16 **gain-variation corridor**
  is pinned on the codec loop with the selective meter: Modes 1/2 meet
  the printed audio-parts corridor across −61…+8 dBm0 except a
  characterized ≤ 1.0 dB positive-bias window at −56…−53 dBm0, and the
  4-bit Mode 3 (which trades this quality per Table 1) is enveloped at
  ≤ 2.5 dB / ≤ 6 dB. Test count 355→392.
- **Round-401 bitstream-surface robustness + fuzz.** New
  `src/robustness.rs` (deterministic xorshift driver, 8 tests) drives
  the public surface adversarially and asserts the LIMIT / Table 9
  saturation invariants: decoder totality over arbitrary octet streams
  (every `u8` is a valid clause 1.4.4 octet), adversarial mid-stream
  `set_mode`/`reset` interleaving, all 65 536 raw codeword byte pairs
  through `decode_subband_pair`, encoder totality + determinism over
  full-range `i32` PCM, chunked-vs-one-shot encode identity (the
  `pending` path), and reset-equals-fresh stream identity. New `fuzz/`
  cargo-fuzz scaffold (standalone workspace, four targets:
  `decode_stream`, `encode_roundtrip`, `subband_bypass`, `aux_channel`)
  asserting the same spec-side invariants plus the Figure 1/G.722
  auxiliary-channel round-trip contract; initial soak ~4.1 M
  executions, zero findings.

### Fixed

- **Transmit-QMF output clamp saturated after narrowing.** The
  analysis-QMF accumulator was cast `i64 → i32` *before* the Table
  9/G.722 ±16384 clamp, so PCM input beyond the documented 14-bit
  domain wrapped through the cast and came back sign-flipped (a
  constant positive-rail input encoded as an alternating ±full-scale
  lower sub-band) instead of pinning at the rail. The clamp now takes
  the full-width accumulator, matching the receive-side clamp.
  Bit-exact for every in-domain input; regression-pinned in
  `robustness.rs`.
- **Clause 5.2 saturation operators overflowed `i32` on out-of-domain
  sub-band input.** The spec `+`/`−` operators computed the `i32`
  intermediate before clamping to the 16-bit rails, so the Appendix-II
  QMF-bypass entry points could overflow (debug panic) when driven
  past the 15-bit Table II-1 domain. Now saturating at the `i32` rails
  first — bit-identical for every 16-bit operand pair.
- **Decoder module header cited a stale Recommendation edition.** The
  doc comment named "(09/2012)"; the staged clean-room reference is the
  11/88 edition, whose figure numbering the header already used. Also
  completed the higher-band figure span (BLOCKs 2H–5H = Figures
  28–31/G.722).

- **Round-367 joint analysis↔synthesis QMF near-perfect-reconstruction
  conformance.** The transmit (analysis) and receive (synthesis) QMF
  banks share the single 24-tap symmetric Table 11/G.722 coefficient set
  — the defining property of a quadrature mirror filter bank. Until now
  each bank was pinned only by its own *isolated* DC-gain test
  (`transmit_qmf_dc_splits_with_unity_lower_band_gain` /
  `receive_qmf_lower_band_dc_has_unity_gain`); neither pinned the
  **joint** arithmetic, so a transpose of the even/odd delay-line
  assignment, an error in the RECA/RECB sign convention, or a one-bit
  error in *either* `>> 13` (analysis) / `>> 12` (synthesis) shift could
  leave both isolated DC gains correct while destroying the
  reconstruction. New test-only QMF-only accessors
  `Encoder::analysis_qmf_step` / `Decoder::synthesis_qmf_step` (mirroring
  the existing `#[cfg(test)] predictor_snapshots` pattern; no production
  surface) expose the raw analysis `(x_L, x_H)` and raw synthesis
  `(x_out1, x_out2)` so a Kronecker impulse can be cascaded through both
  banks with the sub-band pair passed straight through — no ADPCM
  quantization. Four new `conformance` tests pin: (1) the **bit-exact
  48-sample golden impulse response** (peak `4096` at the fixed
  reconstruction delay index 22, ringed by `0/±1/±2` rounding-noise
  sidelobes); (2) **unity gain within the ±2 two-stage truncation band**
  across `±100 … ±16383` input amplitudes (both shift counts pinned
  jointly — a one-bit error would scale the peak by 2 or ½); (3) **1:1
  linear-phase delay tracking** (shifting the input impulse by an even
  number of samples shifts the reconstructed peak by exactly the same
  amount, peak holding unity `4096`); and (4) the **bounded
  rounding-noise sidelobe budget** (every off-peak sample `≤ ±2`, total
  absolute sidelobe energy `35`). Every golden value is the
  deterministic output of the production QMF integer arithmetic on a
  fully spec-enumerable input; no external reference, disk corpus, or
  online resource was consulted. Test count 351→355.
- **Round-367 analysis-QMF band-selectivity conformance.** The analysis
  QMF splits the 0–8 kHz wideband into a lower (0–4 kHz) and higher
  (4–8 kHz) sub-band (decode-trace §1 / §3.3). The existing isolated
  DC-gain test pins only the lower-band routing; the new
  `analysis_qmf_routes_bands_by_frequency` adds the complementary
  higher-band routing and the mutual aliasing-cancellation: a pure d.c.
  (0 Hz) input lands entirely in `x_L` at unity gain with `x_H = 0`,
  while a Nyquist-rate alternation (8 kHz, the top of the band) lands
  entirely in `x_H` at unity magnitude with `x_L = 0`, the two routings
  mirror-imaging each other. Driven through the QMF-only
  `Encoder::analysis_qmf_step` accessor to steady state. Test count
  355→356.
- **Round-362 Table II-2/G.722 Configuration-1 conformance — segment
  structure + bit-exact "d.c., value of zero" anchor.** Table II-2 is
  the *primary* Configuration-1 encoder conformance input (tones across
  the predictor-pole operating range, three d.c. segments, two
  white-noise levels). The printed 11/88 Recommendation enumerates each
  segment's signal kind + length but not the per-sample amplitudes (the
  tone peaks, low-level d.c. magnitudes, and white-noise seed live only
  on the unstaged disk file `T1C1.XMT`); only the "d.c., value of zero"
  segment is fully sample-enumerable (512 literal zeros). New
  `appendix_ii` API: a `SegmentKind` enum + `TABLE_II_2_SEGMENTS` table
  capturing the 14 printed segments, plus generators for the DcZero
  segment's `XL` / `X#` streams. Four new tests pin (1) the printed
  segment structure — ordering, frequencies, lengths, and the 16384-word
  total cross-checked against the offset constants; (2) the DcZero
  X#-word INFA round-trip; (3) the **bit-exact encoder I# output** for
  the DcZero segment from reset (constant silence code-word `0xFA00`:
  I_H=3 / I_L=58, full-segment FNV-1a checksum), the deterministic
  silence response of the quantizer / predictor feedback loop; and (4)
  its **full-circuit transmit→receive** behaviour across all three
  modes (RH# mode-independent, RL# settles per-mode). Test count
  347→351.
- **Round-359 per-sub-sequence-boundary bit-exact RL#/RH# anchors for
  the synthesisable Appendix-II.3.2 Configuration-2 sequence.** The
  full 16384-sample artificial receive corpus was previously protected
  only by a single opaque FNV-1a fingerprint (a regression anywhere
  flipped one 64-bit value with no localisation) plus a human-readable
  512-sample window that reaches only the first two Table II-4/G.722
  lower-LSB sub-sequences. New `appendix_ii_modeN_table_ii4_boundaries_
  are_bit_exact` tests pin the reconstructed RL#/RH# wire word at every
  one of the 64 Table II-4 sub-sequence boundaries (sample `n * 256`),
  per mode — walking the decoder across the deep adaptive states the
  spec designed the sequence to exercise: the logarithmic quantizer
  scale factor over its entire range (LSB magnitude ramps 31→0), the
  pole predictor coefficients across their allowable range, and the
  **suppressed-codeword conversion** of sub-sequences (56)–(64) that
  clause II.3.2.1 p. 67 explicitly calls out (the four substituted
  INVQBL code-words arising from transmission errors). A companion test
  pins the structural invariant that the higher sub-band loop is
  **mode-independent** (RH# byte-identical across all three modes —
  only the lower band drops LSBs for the auxiliary channel, clause 1.3)
  on the deep-adaptation corpus, plus RSS-clear / LIMIT-range and
  lower-band mode-distinctness checks. +6 tests (341 → 347). No
  external reference, disk corpus, or online resource was consulted;
  every golden value is the deterministic output of the production
  receive path on the fully spec-enumerated input.
- **Round-349 bit-exact ENCODER conformance on the synthesisable
  Table II-3/G.722 overflow Configuration-1 vector.** Every prior
  Appendix-II conformance anchor targeted the *receive* path
  (Configuration 2): the artificial II.3.2 sequence bypasses the
  forward quantizer and difference computation, so the encoder's
  overflow / saturation control was untested against a spec-derived
  vector. Table II-3 (clause II.3.2 p. 67, "sequence for testing
  overflow controls in the ADPCM encoders") is the one Configuration-1
  input that is *fully enumerated* in the printed 11/88 PDF — 768
  full-scale words (`-16384, +16383` ×639 / `0, -10000, -8192` /
  `-16384, +16383, -16384` ×126) — and therefore synthesisable without
  the disk-distributed corpus. `test_harness::appendix_ii` now exposes
  `build_overflow_xl_sequence` / `build_overflow_x_hash_stream`
  (X# = XL << 1, the exact inverse of INFA's `XL = X# >> 1`), and the
  sequence is driven through the encoder via `run_configuration_1` with
  a 32-word leading golden vector + a full-768-word FNV-1a checksum
  (`0x21ba1840cd7af612`). The full-scale ±16384 swings force the
  largest prediction errors, exercising the saturating pole/zero-section
  output computations of clauses 3.6.1 / 3.6.2 (BLOCK 4L / 4H). A
  companion test pins the Configuration-1 RSS reset-slot behaviour
  (non-valid `I# = 0x0001`, post-reset encoder matches a fresh one).
  No external reference, disk corpus, or online resource was consulted.
- **Round-344 bit-exact RL#/RH# conformance for the synthesisable
  Appendix-II.3.2 sequence.** The artificial Configuration-2 input
  sequence (clause II.3.2 — the only spec-derivable ITU receive-path
  test sequence, since the disk-distributed `T2R1.COD` / `T2R2.COD`
  corpus is not staged) was previously only checked for *determinism*
  end-to-end; a decoder regression that altered the output identically
  across runs would pass that check. It is now anchored bit-exact:
  per-mode golden RL#/RH# wire-word vectors for the leading 512-sample
  window (the window crosses the first Table II-4 lower-LSB
  sub-sequence boundary so the lower-band predictor genuinely adapts,
  and the higher band sweeps the full LIMIT range), plus a per-mode
  FNV-1a checksum anchor over the *entire* 16384-sample sequence
  (reaching the suppressed-codeword wrap sub-sequences (56)–(64) of
  Table II-4 that the short window does not cover). The three modes are
  asserted pairwise-distinct on both the windowed vectors and the
  full-sequence fingerprints. Every value is the deterministic output
  of the receive path driven by the spec's own `lower_msb_bit` /
  `lower_lsb5` / `higher_*` generators; no external reference, no disk
  corpus, no online resource was consulted.
- **Round-338 bit-exact conformance vectors + clause-2.4.2 mask driven
  on the real codec.** New `conformance` test module pins the codec's
  exact integer output against golden vectors hand-derived from the
  staged ITU-T G.722 (11/88) Recommendation pseudo-code (sub-blocks
  INVQAL / INVQBL / INVQAH / PARREC / UPPOL1 / UPPOL2 / UPZERO /
  FILTEP / FILTEZ / LOGSCL / SCALEL / SCALEH / RECONS / LIMIT plus the
  analysis / synthesis QMF of clauses 5.2.1 / 5.2.2): per-mode golden
  decode PCM vectors, a golden encode octet stream, per-codeword
  reset-state inverse-quantizer anchors covering every Table 14 / 17 /
  18 / 19 / 6 row, and a single-step hand-derivation anchor (octet 0x7F
  → DL/DH). Until now the only end-to-end checks were loose silence /
  energy envelopes and a predictor-state lockstep invariant; none
  pinned actual sample values, so a sign or shift-count regression that
  stayed inside the envelope (the r313 / r322 QMF-normalisation and
  r326 QUANTL off-by-one defect class) could pass unnoticed.
  Additionally, `transmission::measure_tone_response` +
  `ToneResponseReport` drive a sinusoid end-to-end through encode →
  decode and the clause 2.4.2 / Figure 10/G.722 attenuation/frequency
  mask is now enforced on the production paths across all three modes
  (1020 Hz reference tone + a passband sweep). Every golden integer was
  produced by stepping the Recommendation's own printed pseudo-code; no
  external reference implementation, reference C source, or online
  resource was consulted. Suite: 314 → 328 tests.

- **Round-332 transmit↔receive predictor-state lockstep conformance
  test.** New `encoder_local_decoder_tracks_standalone_decoder_in_lockstep`
  pins the structural identity that the SB-ADPCM block diagrams (Figures
  4 / 6 / 7 of the staged `docs/audio/g722/T-REC-G.722-198811-S.pdf`)
  mandate: the transmit path embeds a *local decoder* whose adaptive
  predictor + scale-factor loop (clauses 3.4 / 3.5 / 3.6) is the **same**
  loop the standalone receive decoder runs, driven by the **identical**
  truncated code-word. In Mode 1 the decoder's predictor-update path
  uses INVQAL on the 4-bit-truncated `I_L` (eq 3-11) — bit-for-bit what
  the encoder feeds its own embedded loop — and the higher band feeds
  its 2-bit `I_H` back untruncated, so after processing the same
  `(I_L, I_H)` stream the encoder's and decoder's predictor states must
  be bit-identical at every step. The test drives both through the
  Appendix-II QMF-bypass entry points (`encode_subband_pair` /
  `decode_subband_pair`) on a 4096-step wide-range pseudo-random
  sub-band signal and asserts equality of the full lower- and
  higher-sub-band predictor snapshots (`dlt` / `plt` / `rlt` / `al1` /
  `al2` / `bl` / `nbl` / `detl`) every step. This guards the shared
  `predictor` module (PARREC / UPPOL1 / UPPOL2 / UPZERO / LOGSCL /
  SCALEL) against silent divergences the loose silence/energy-envelope
  tests cannot see — the same class of latent defect the r326 QUANTL
  off-by-one and r313 / r322 QMF normalisation fixes were. Adds a
  test-only `PredictorSnapshot` + `SubBandState::snapshot` plus
  `#[cfg(test)]` snapshot accessors on the encoder / decoder sub-band
  states; no change to the production decode/encode paths.

### Fixed

- **Round-326 lower-sub-band forward quantizer (QUANTL) off-by-one in the
  decision-level index.** `encoder::LowerEncoderState::quantize_lower`
  evaluated the upper decision level for candidate row `m_L = k` as
  `(Q6(k−1) << 3) * DETL` instead of the Recommendation's
  `(Q6(k) << 3) * DETL`. The QUANTL decision table (clause 6.2.1.1,
  p. 42 of the staged `docs/audio/g722/T-REC-G.722-198811-S.pdf`) gives
  row `m_L = k` the *upper* decision level `LDU(k) = (Q6(k) << 3) * DETL`
  for `k = 1..29` (with `LDU(30) = +∞`) and lower level
  `LDL(k) = LDU(k−1)`, `LDL(1) = 0`, where `Q6(k)` is the 1-indexed
  Table 14/G.722 entry (`Q6(1) = 35`, p. 35). The off-by-one shifted
  every selected `m_L` one row too high and made `m_L = 1` unreachable,
  so the encoder emitted a wrong 6-bit `I_L` codeword (and a wrong
  `W_L` scale-factor-adaptation input) — invisible to encode→decode
  self-round-trip tests but a bit-exactness defect against a conformant
  decoder. The loop now indexes `Q6[k]` directly; at reset
  (`DETL = 32`) a zero-magnitude difference correctly selects `m_L = 4`
  (rows 1..3 collapse to `LDL == LDU == 0` and are excluded by Note 2),
  not `m_L = 5`. Two new spec-derived tests pin the corrected boundary:
  `lower_forward_quantizer_emits_mil_1_when_ldu_1_does_not_collapse`
  (at `DETL = 128`, `LDU(1) = 1` so `WD = 0` reaches `m_L = 1`) and
  `lower_forward_quantizer_boundary_is_strict_below_ldu` (Note 1: `WD`
  exactly on `LDU(1)` advances to `m_L = 2`). The pre-existing reset
  test was updated from the wrong `m_L = 5` to the spec-correct
  `m_L = 4`. The higher-sub-band QUANTH decision level is unaffected:
  it correctly uses the Table 14 higher-band entry `Q2(1) = 564`
  (`src/tables.rs::Q2_LEVEL_1`), not `Q6(1)` — the QUANTH body text's
  "(Q6(1) << 3)" (p. 51) is a known spec misprint contradicted by its
  own "Q2 is obtained from Table 14" note and the Table 14 higher-band
  sub-table.

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

# oxideav-g722

[![CI](https://github.com/OxideAV/oxideav-g722/actions/workflows/ci.yml/badge.svg)](https://github.com/OxideAV/oxideav-g722/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/oxideav-g722.svg)](https://crates.io/crates/oxideav-g722) [![docs.rs](https://docs.rs/oxideav-g722/badge.svg)](https://docs.rs/oxideav-g722) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Pure-Rust SB-ADPCM codec for ITU-T G.722 wideband speech / audio at
64 / 56 / 48 kbit/s. Zero C dependencies, no FFI, no `*-sys` crates.

## Status

Both the **encoder** and **decoder** sides of G.722 are implemented
against the staged Recommendation ITU-T G.722 (11/88) and are
**bit-exact against the ITU-T G.191 G.722 conformance corpus**
(`docs/audio/g722/conformance/` in the workspace): the encoder
reproduces the reference 64 kbit/s bitstream on all 48 768 octets of
the speech vector, and the decoder reproduces the reference PCM on all
97 536 samples in **each of the three modes** (`tests/itu_conformance.rs`;
committed ~0.5 s prefix excerpts keep the check alive in standalone
CI). Landing that corpus exposed and fixed three real divergences in
the earlier spec reading — the Table 14 **QQ4 addressing** (rows map to
`IL4 = row − 1`; the old 1-based reading shifted every 4-bit
inverse-quantizer magnitude one row low and dropped the top output
2557, biasing the predictor-update DLT on *every* octet in *every*
mode), the **FILTEP delay-line timing** (RLT1 is the reconstruction of
one sample earlier, not two), and **UPPOL1's stability window**
(computed against the freshly updated APL2, not the delayed AL2). The
corpus also pinned the 16-bit PCM I/O convention now exposed as
`Encoder::encode_pcm16` / `Decoder::decode_pcm16` (the QMF rescaling
freedom of clause 5.2 Note 2 absorbs the extra bit; *not* equivalent to
shifting the samples).

The sub-band ADPCM loops run end-to-end across all three modes and are
additionally pinned against golden vectors derived independently
from the Recommendation's own per-block pseudo-code (sub-blocks
INVQAL / INVQBL / INVQAH / PARREC / UPPOL1 / UPPOL2 / UPZERO /
FILTEP / FILTEZ / LOGSCL / SCALEL / SCALEH and the analysis /
synthesis QMF). Decode and encode golden vectors, per-codeword
reset-state inverse-quantizer anchors (every Table 14 / 17 / 18 /
19 / 6 row), and a single-step hand-derivation anchor are all pinned
in `src/conformance.rs`. The clause-2.4.2 attenuation/frequency mask
is **operationally enforced on the real codec** across all three
modes via the `transmission::measure_tone_response` helper, and the
rest of the measurable clause-2.4 set is now enforced the same way:
the clause-2.4.3 **absolute group delay** (two-tone phase-slope
reading: ~22 samples ≈ 1.38 ms ≤ 4 ms, flat across an 11-frequency
50–7000 Hz sweep, all modes), the clause-2.4.4 **narrow-band idle
noise** (50–7000 Hz ≤ −66 dBm0) and clause-2.4.5 **selective
single-frequency noise** (≤ −70 dBm0 per DFT bin, 8000 Hz pinned
explicitly) — with the idle steady state proven to hug the deepest LSB
floor (±2–3 LSB hunting with a sub-LSB DC mean; the corpus-corrected
QQ4 makes the silence code-word's DLT = +1, so the predictor slowly
hunts instead of freezing), keeping the margins structural —
and the clause-2.4.6 **signal-to-total-distortion** quantity (printed
"Under study": no normative mask exists) pinned as measured-behaviour
quality gates with ≈ 2 dB headroom per mode / level / sub-band. The
clause-2.4.2 sweep covers the **entire Figure 10 mask domain** (50 Hz
low transition through the 7–8 kHz high transition), the
clause-2.4.1 **nominal 3-dB bandwidth** is checked at both printed
band edges, and the clause-2.5.7 / Figure 16 **gain-variation
corridor** — an audio-parts requirement — is met outright by the
Mode 1 / 2 codec loops under the selective meter (except a
characterized ≤ 1 dB window at −56…−53 dBm0), with the 4-bit Mode 3
enveloped empirically. Both
spec-enumerated synthesisable Appendix-II sequences are now driven
bit-exact end-to-end. The **joint analysis↔synthesis QMF filter bank**
is additionally pinned to its near-perfect-reconstruction property: a
Kronecker impulse cascaded through the transmit and receive QMFs (no
ADPCM quantization between) reconstructs the input as a unity-gain,
fixed-delay copy, anchored bit-exact (golden 48-sample impulse
response, ±2 two-stage-truncation unity-gain band across `±100 …
±16383`, 1:1 linear-phase delay tracking, bounded `±2` rounding-noise
sidelobes) — a check the two earlier per-bank DC-gain tests cannot make
because each pins only one bank in isolation. The II.3.2 artificial
Configuration-2 sequence
through the **receive** path and the Table II-3 overflow Configuration-1
sequence through the **transmit** path (the latter exercising the
pole/zero-section overflow controls). The II.3.2 receive sequence is
anchored at **three depths**: a human-readable 512-sample leading
golden window, a per-mode bit-exact RL#/RH# anchor at **every one of
the 64 Table II-4 sub-sequence boundaries** (walking the codec across
the full scale-factor / pole-coefficient range and the
suppressed-codeword conversion of sub-sequences 56–64), and a
full-16384-sample FNV-1a checksum; the per-boundary anchors also pin
the structural invariant that the higher sub-band loop is
**mode-independent** (identical RH# across all three modes). The two
sequences are also **chained
full-circuit**: the Table II-3 overflow input is encoded then the
resulting `I#` stream decoded per mode, pinning the round-trip RL#/RH#
bit-exact across all three modes; and **reset behaviour** is anchored on
both sides — a mid-stream RSS marker resets the receive decoder and the
continuation matches a fresh decode (all three modes), and the
transmit↔receive predictor lockstep is proven to survive a simultaneous
mid-stream reset. The **Table II-2/G.722** primary Configuration-1
input (tones / d.c. / white noise) now has its printed segment
structure pinned and its only fully sample-enumerable segment — the
512-word "d.c., value of zero" — driven bit-exact through the encoder
and full-circuit through the decoder across all three modes. The one
remaining external-corpus gap is the ITU disk-distributed **QMF-bypass
Configuration-1/2 digital test sequences** (`T2R1.COD` / `T2R2.COD` +
`*.RC*` comparison files, plus the non-enumerated Table II-2 tone /
low-level-d.c. / white-noise sample amplitudes carried only on
`T1C1.XMT`), which are not staged under `docs/` (see *Test vectors*
below); the staged G.191 corpus already pins the full QMF-included
codec in both directions.

The crate is reachable through its direct `Encoder` / `Decoder` API
only; it does not register a trait-surface codec in the runtime
registry.

| Path         | Coverage              | Notes                                                                                  |
| ------------ | --------------------- | -------------------------------------------------------------------------------------- |
| Encoder      | bit-exact (ITU corpus) | Transmit 24-tap QMF (§3.1; unity-DC-gain normalised per the LOWT/HIGHT `>> (y−15)` shift of §5.2.1, with a 16-bit-PCM `>> 14` variant per Note 2), BLOCK 1L QUANTL (decision level `LDU(k) = (Q6(k) << 3)·DETL`, 1-indexed per Table 14) + BLOCK 1H QUANTH (decision level `Q2(1) = 564`) forward quantizers, shared predictor. Bit-exact on all 48 768 corpus octets; also pinned against a spec-pseudo-code golden octet stream. |
| Decoder      | bit-exact (ITU corpus) | Lower (4/5/6-bit modes 1/2/3) + higher (2-bit) inverse ADPCM, 24-tap receive QMF (unity-DC-gain normalised per eqs 4-3/4-4, with a 16-bit-PCM `>> 11` variant per Note 2), LIMIT saturation. Bit-exact on all 3 × 97 536 corpus samples; also pinned against per-mode golden PCM vectors + per-codeword reset-state inverse-quantizer anchors. |
| Test vectors | ITU corpus + spec | **`tests/itu_conformance.rs`: the staged ITU-T G.191 conformance corpus bit-exact in both directions and all three modes (encoder 48 768/48 768 octets; decoder 3 × 97 536/97 536 samples), with committed prefix excerpts for standalone CI and a documented anomaly note on the corpus's second `.cod` container**; `src/conformance.rs` golden decode + encode vectors (all modes), per-codeword inverse-quantizer anchors, single-step hand derivation; the **synthesisable Appendix II.3.2 artificial Configuration-2 sequence** driven end-to-end through the receive path with **bit-exact RL#/RH# golden vectors** at three depths — the leading 512-sample window, **per-mode anchors at all 64 Table II-4 sub-sequence boundaries** (covering the full scale-factor / pole-coefficient range + suppressed-codeword sub-sequences 56–64, with the higher-band loop pinned mode-independent), and a full-16384-sample per-mode FNV-1a checksum; the **synthesisable Table II-3/G.722 overflow Configuration-1 sequence** (768 full-scale words) driven through the **encoder** with a bit-exact I# golden window + full-sequence checksum, exercising the pole/zero-section overflow controls; the **Table II-2/G.722 segment structure** (14 tones/d.c./white-noise segments summing to 16384 words) pinned against the printed table, with its **only fully sample-enumerable segment — the 512-word "d.c., value of zero" — driven bit-exact through the encoder** (44-word constant silence code-word `0xFA00` run — I_H=3 / I_L=58 — then the corpus-corroborated predictor-drift hunting tail, fingerprint-pinned) and **full-circuit transmit→receive** across all three modes (`test_harness`); transmit↔receive predictor-state lockstep over a 4096-step sweep; clause-2.4.2 mask driven on the real codec. The QMF-bypass ITU disk corpus (`T2R1.COD` / `T2R2.COD` / `T1C1.XMT` tone/noise samples / `*.RC*`) is not staged. |

### Implemented

- §1.3 Modes 1 / 2 / 3 (Table 1) with mid-stream mode switching.
- §3.1 / §5.2.1 transmit + §5.2.2 receive 24-tap symmetric QMF
  (Table 11/G.722 coefficients) splitting / recombining the 16 kHz
  uniform-PCM stream and the two 8 kHz sub-bands. Both banks carry the
  spec-exact, mask-free unity-DC-gain normalisation: the analysis
  LOWT/HIGHT shift is `>> (y−15) = >> 13` (one bit more than the
  synthesis `>> (y−16) = >> 12`, reflecting the receive QMF's leading
  factor of 2 in eqs 4-3/4-4 that the transmit QMF lacks).
- §3.3 BLOCK 1L QUANTL (60-level lower-sub-band forward adaptive log
  quantizer) + BLOCK 1H QUANTH (4-level higher-sub-band quantizer).
- §1.4.4 multiplexer packing `(I_H, I_L)` into the 64 kbit/s octet.
- §6.2.1 INVQAL / INVQBL inverse quantizers, LOGSCL + SCALEL Method 2,
  the lower-sub-band adaptive predictor (PARREC + FILTEZ + FILTEP +
  PREDIC + UPPOL1 + UPPOL2 + UPZERO), and LIMIT output saturation.
- §6.2.2 symmetric higher-sub-band ADPCM (blocks 2H–5H, 2-bit inverse
  quantizer, SCALEH Method 2).
- The Table 17 / 18 / 19 / 20 inverse-quantizer tables made
  bit-faithful to the printed spec, including the documented
  structural anomalies.
- A transmit↔receive **predictor-state lockstep** conformance check.
  Per the SB-ADPCM block diagrams (Figures 4 / 6 / 7) the transmit path
  embeds a local decoder whose predictor + scale-factor adaptation
  (clauses 3.4 / 3.5 / 3.6) is the same loop the standalone receive
  decoder runs, driven by the identical truncated code-word; the test
  drives both via the Appendix-II QMF-bypass entry points on a
  4096-step wide-range pseudo-random sub-band signal and asserts the
  two lower- and higher-sub-band predictor states stay bit-identical at
  every step. This guards the entire shared adaptation loop (PARREC /
  UPPOL1 / UPPOL2 / UPZERO / LOGSCL / SCALEL) against the silent
  divergences the loose silence/energy-envelope tests cannot see.
- `transmission::spectrum` + three codec-loop measurement surfaces:
  `measure_signal_to_distortion` (exact least-squares signal /
  total-distortion split with matched-window phase readings),
  `measure_idle_channel_spectrum` (per-DFT-bin idle sweep against the
  clause 2.4.4 narrow-band + clause 2.4.5 selective limits), and
  `measure_group_delay` (two-tone phase-slope, clause 2.4.3), all
  driven as conformance tests on the real codec in every mode.
- Bitstream-surface robustness: the public API is total (and pinned
  deterministic) over arbitrary octet streams, full-range `i32` PCM,
  raw codeword bytes, and adversarial mid-stream `set_mode` / `reset`
  interleavings, asserting the LIMIT / Table 9 saturation invariants
  (`src/robustness.rs`); two latent out-of-domain bugs found and fixed
  in the process (transmit-QMF clamp-after-narrowing sign flip;
  clause 5.2 saturation operators overflowing `i32` on out-of-domain
  sub-band input).
- `fuzz/`: four cargo-fuzz targets (`decode_stream`,
  `encode_roundtrip`, `subband_bypass`, `aux_channel`) asserting the
  same spec-side invariants plus the Figure 1/G.722 auxiliary-channel
  round-trip contract.
- `transmission` module: the normative limits of clause 2 (clock
  rates, sample-clock tolerance, overload point, passband, group
  delay, idle / single-frequency noise) as typed constants citing
  their clause numbers, dBm0 ↔ uniform-PCM conversion, an idle-noise
  end-to-end check, plus typed read views of the figure masks
  (attenuation/frequency, group-delay-distortion, signal-to-distortion
  vs level / frequency, gain-variation, input anti-aliasing + output
  reconstructing filter masks).
- `test_harness` module: Appendix II Configuration-1 / Configuration-2
  QMF-bypass entry points (`encode_subband_pair` / `decode_subband_pair`),
  the four INFA / INFB / INFC / INFD sub-blocks, the wire-format
  bit-position constants, and `run_configuration_1` /
  `run_configuration_2` walkers. `test_harness::appendix_ii`
  synthesises the third Configuration-2 input sequence procedurally
  from the printed bit patterns (the two encoder-derived sequences and
  the comparison output files are not synthesisable from the PDF), and
  the **Table II-3/G.722 overflow Configuration-1 input** (the one
  encoder-side test sequence fully enumerated in the printed PDF: 768
  full-scale words) via `build_overflow_xl_sequence` /
  `build_overflow_x_hash_stream`, driven through the encoder with a
  bit-exact I# golden anchor.

### Not yet implemented

- Validation against the **ITU disk-distributed QMF-bypass** sequences
  and their comparison output files: the Table II-2 tone / d.c. /
  white-noise Configuration-1 input (whose per-sample amplitudes are
  *not* enumerated in the printed PDF — only the segment frequencies /
  lengths), the encoder-derived Configuration-2 inputs (`T2R1.COD` /
  `T2R2.COD`), and the comparison files (`T3L*.RC*` / `T3H*.RC*`).
  These corpora are not staged under `docs/`
  (`docs/audio/g722/conformance/README.md` documents them as a GAP).
  The full QMF-included codec is already bit-exact in both directions
  against the staged G.191 corpus; the disk set would additionally pin
  the sub-band arithmetic in isolation.
- Appendix III / IV packet-loss concealment.
- Annex B superwideband extension (50–14 000 Hz).
- Annex D stereo extension.
- The audio-parts clauses whose masks are typed but require an actual
  analogue/digital filter implementation in the loop for end-to-end
  measurement (the narrow-band −66 dBm0 idle-noise bound, the
  go/return crosstalk limits, the "under study" clauses).

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

Real-world 16 kHz audio is usually full-scale **16-bit PCM** rather
than the Recommendation's Table 9 sign-extended 15-bit wire format;
the `*_pcm16` entry points take and produce it directly, keeping the
extra bit alive through the QMF accumulations (this is the convention
the ITU conformance corpus is bit-exact under):

```rust
use oxideav_g722::{Decoder, Encoder, Mode};

let mut encoder = Encoder::new();
let pcm16: Vec<i16> = read_pcm16_samples();
let bitstream = encoder.encode_pcm16(&pcm16);

let mut decoder = Decoder::new(Mode::Mode1);
let samples: Vec<i16> = decoder.decode_pcm16(&bitstream);
```

Both directions are also reachable via the historical factory entry
points `oxideav_g722::make_encoder()` and
`oxideav_g722::make_decoder(Mode::Mode1)`.

## Provenance

All numeric tables, decision rules and adaptation arithmetic were
transcribed by hand from the printed normative tables of the staged
Recommendation (`docs/audio/g722/`). Per-table provenance citations
sit next to each constant in `src/tables.rs`. The ITU-T G.191
conformance vectors are consumed as opaque black-box input↔output
data only (`docs/audio/g722/conformance/README.md` documents their
provenance); no reference source code was consulted.

## License

MIT — see [LICENSE](LICENSE).

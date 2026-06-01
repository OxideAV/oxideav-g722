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

Coverage:

| Path     | Spec coverage | Notes                                                                                              |
| -------- | ------------- | -------------------------------------------------------------------------------------------------- |
| Encoder  | structural    | Transmit QMF (clause 3.1), BLOCK 1L QUANTL + BLOCK 1H QUANTH forward quantizers, shared predictor. |
| Decoder  | structural    | Lower (4/5/6-bit modes) + higher (2-bit) inverse ADPCM, 24-tap receive QMF.                        |
| Test vectors | none      | Appendix II digital test sequences are not yet staged under `docs/`.                               |

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
- Bit-exact validation against the ITU-T G.191 digital test sequences
  (Appendix II) — the test-sequence corpus is not staged under `docs/`.

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
each constant in `src/tables.rs`. No external reference implementation
of the codec, no FFmpeg / libav* source, no third-party G.722 source
distribution, and no online resources were consulted during the
rebuild.

## License

MIT — see [LICENSE](LICENSE).

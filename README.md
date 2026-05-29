# oxideav-g722

Pure-Rust decoder for ITU-T G.722 wideband sub-band ADPCM speech coding
at 64 / 56 / 48 kbit/s.

## Status

Round-185 clean-room rebuild against the staged Recommendation ITU-T
G.722 (09/2012). The decoder is wired up and self-test green; the
encoder is not yet implemented.

Coverage:

| Path     | Spec coverage | Notes                                                                                  |
| -------- | ------------- | -------------------------------------------------------------------------------------- |
| Decoder  | structural    | Lower (4/5/6-bit modes) + higher (2-bit) sub-band ADPCM + 24-tap receive QMF (Method 2). |
| Encoder  | none          | Forward-quantizer tables are present but the encode path is not wired up.              |
| Test vectors | none      | Appendix II digital test sequences are not yet staged under `docs/`.                   |

### Implemented

- §1.3 Modes 1 / 2 / 3 (Table 1, page 3) with mid-stream mode switching.
- §6.2.1.2 / 6.2.1.5 INVQAL + INVQBL inverse adaptive quantizers.
- §6.2.1.3 LOGSCL + SCALEL Method 2 (32-entry log-to-linear table).
- §6.2.1.4 PARREC + FILTEZ + FILTEP + PREDIC + UPPOL1 + UPPOL2 +
  UPZERO lower-sub-band adaptive predictor.
- §6.2.1.6 LIMIT output saturation.
- §6.2.2 the symmetric higher-sub-band ADPCM blocks 2H / 3H / 4H / 5H
  including the 2-bit inverse quantizer and SCALEH Method 2.
- §5.2.2 receive QMF with the 24-tap symmetric filter (Table 11,
  page 23).

### Not yet implemented

- Encode path (§6.2.1.1 BLOCK 1L QUANTL, §6.2.2.1 BLOCK 1H QUANTH and
  the matching adaptation feedback).
- Appendix III / IV packet-loss concealment.
- Annex B superwideband extension (50–14 000 Hz).
- Annex D stereo extension.
- Bit-exact validation against the ITU-T G.191 digital test sequences
  (Appendix II) — the test-sequence corpus is not staged under `docs/`.

## Usage

```rust
use oxideav_g722::{Decoder, Mode};

let mut decoder = Decoder::new(Mode::Mode1);
let bitstream: &[u8] = read_g722_octets();
let pcm_16khz = decoder.decode(bitstream);
// pcm_16khz.len() == bitstream.len() * 2
```

The decoder can also be constructed via the historical factory entry
point: `oxideav_g722::make_decoder(Mode::Mode1)`.

## Provenance

All numeric tables and adaptation arithmetic in this crate were
transcribed by hand from the printed normative tables of
`docs/audio/adpcm/g722/itu-t.G.722.2012.pdf`. The per-table provenance
citations sit next to each constant in `src/tables.rs`. No external
reference C implementation, no FFmpeg / libav* source, no spandsp
source, and no online resources were consulted during the rebuild.

## License

MIT — see [LICENSE](LICENSE).

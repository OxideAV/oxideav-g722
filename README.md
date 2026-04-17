# oxideav-g722

Pure-Rust **ITU-T G.722** wideband sub-band ADPCM codec — 7 kHz speech /
audio at 16 kHz mono S16, three operating modes (64 / 56 / 48 kbit/s),
with the 8 / 16 kbit/s auxiliary side-channel exposed. Zero C
dependencies.

Part of the [oxideav](https://github.com/OxideAV/oxideav-workspace)
framework but usable standalone.

## Installation

```toml
[dependencies]
oxideav-core = "0.0"
oxideav-codec = "0.0"
oxideav-g722 = "0.0"
```

## Operating modes

| Mode | Bit rate  | Low-band bits | Aux bits / rate | `bit_rate` hint |
|------|-----------|---------------|-----------------|-----------------|
| 1    | 64 kbit/s | 6             | 0 / 0           | `Some(64000)` or `None` |
| 2    | 56 kbit/s | 5             | 1 / 8 kbit/s    | `Some(56000)`   |
| 3    | 48 kbit/s | 4             | 2 / 16 kbit/s   | `Some(48000)`   |

The high-band always uses 2 bits. Every packed byte covers two PCM
samples at 16 kHz (one low/high pair at 8 kHz). Mode is selected by
the `bit_rate` field on `CodecParameters`; an unrecognised rate is
rejected with `Error::Unsupported`.

## Quick use

```rust
use oxideav_codec::{CodecRegistry, Decoder, Encoder};
use oxideav_core::{
    AudioFrame, CodecId, CodecParameters, Frame, SampleFormat, TimeBase,
};

let mut codecs = CodecRegistry::new();
oxideav_g722::register(&mut codecs);

let mut params = CodecParameters::audio(CodecId::new("g722"));
params.sample_rate = Some(16_000);
params.channels = Some(1);
params.sample_format = Some(SampleFormat::S16);
params.bit_rate = Some(64_000); // 56000 or 48000 also valid; None = 64k

let mut enc = codecs.make_encoder(&params)?;
let mut dec = codecs.make_decoder(&params)?;

// Pack S16 mono samples (interleaved LE bytes) into an AudioFrame and
// feed the encoder. Output is one packet per send_frame, samples / 2 bytes.
let pcm: Vec<i16> = (0..3200).map(|n| ((n as f32 * 0.05).sin() * 8_000.0) as i16).collect();
let mut bytes = Vec::with_capacity(pcm.len() * 2);
for &s in &pcm { bytes.extend_from_slice(&s.to_le_bytes()); }
enc.send_frame(&Frame::Audio(AudioFrame {
    format: SampleFormat::S16,
    channels: 1,
    sample_rate: 16_000,
    samples: pcm.len() as u32,
    pts: Some(0),
    time_base: TimeBase::new(1, 16_000),
    data: vec![bytes],
}))?;
enc.flush()?;

while let Ok(pkt) = enc.receive_packet() {
    dec.send_packet(&pkt)?;
    while let Ok(Frame::Audio(_af)) = dec.receive_frame() { /* ... */ }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Auxiliary side-channel (modes 2 + 3)

Mode 2 reserves 1 bit per packed byte (8 kbit/s side-channel) and Mode 3
reserves 2 (16 kbit/s). The default is to write zeros and discard them,
but the concrete `G722Encoder` / `G722Decoder` types let you carry data
through them:

```rust
use oxideav_g722::{decoder::G722Decoder, encoder::G722Encoder, mode::Mode};

let mut enc = G722Encoder::new(out_params, Mode::Mode2);
enc.push_aux(&[0b0, 0b1, 0b1, 0b0]); // LSB of each byte = aux bit
enc.send_frame(&frame)?;             // pads with 0 if queue empties

let mut dec = G722Decoder::with_mode(Mode::Mode2);
dec.send_packet(&pkt)?;
let aux: Vec<u8> = dec.take_aux();   // one entry per decoded byte
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Mode::aux_bits()` and `Mode::aux_rate()` give the per-byte width and
the side-channel data rate.

## Status

- Encoder + decoder cover all three modes; QMF analysis / synthesis match
  the SpanDSP / libg722 24-tap polyphase reference (>30 dB roundtrip on
  pure tone with no ADPCM in between).
- ADPCM pipeline is shape-equivalent to G.722 (sign+magnitude quantiser,
  log-domain scale-factor adapter, 2-pole / 6-zero low-band predictor,
  2-pole / 1-zero high-band predictor) with encoder + decoder sharing the
  exact same update rule, so they stay locked. The on-wire bytes are
  internally consistent and pass encode-decode PSNR > 20 dB at every
  rate, but are not bit-exact with other G.722 implementations.
- The normative ITU-T inverse-quantiser tables (`QM6` / `QM5` / `QM4` /
  `QM2`) are present in [`tables`] for an eventual bit-exact port.
- No sync-word framing is emitted; this crate is transparent on byte
  boundaries and leaves framing to the surrounding container (RTP / SIP
  payload type 9, etc.).
- Stereo and non-16 kHz inputs are rejected with `Error::Unsupported`.

## Codec id

- `"g722"` — registers both directions; capability advertises mono S16
  audio at 16 kHz max.

## License

MIT — see [LICENSE](LICENSE).

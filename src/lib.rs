//! ITU-T G.722 wideband sub-band ADPCM codec.
//!
//! G.722 compresses 7 kHz-bandwidth speech / audio sampled at 16 kHz mono
//! into a nominal 64 / 56 / 48 kbit/s bitstream. The codec works by
//! splitting the input into two 4 kHz sub-bands via a 24-tap QMF, and
//! running a backward-adaptive ADPCM quantiser on each band independently:
//!
//! - Low-band (0..4 kHz): 4 / 5 / 6-bit quantiser (rate-dependent) with a
//!   2-pole / 6-zero predictor.
//! - High-band (4..8 kHz): 2-bit quantiser with the same predictor topology
//!   at every rate.
//!
//! On encode the two bands are packed into an 8-bit sample-serial byte:
//! low-band in the high bits, high-band in the low 2 bits. One byte
//! therefore describes two PCM samples worth of audio — hence 64 kbit/s
//! at 16 kHz (8 bits × 8000 packed pairs / sec). At the lower rates one
//! or two auxiliary bits (always zero on encode, discarded on decode) sit
//! between the low-band and high-band fields; see [`mode::Mode`] for the
//! exact bit layout.
//!
//! # Operating modes
//!
//! | Mode | Bit rate  | Low-band bits | Aux bits | Selected via `CodecParameters::bit_rate` |
//! |------|-----------|---------------|----------|------------------------------------------|
//! | 1    | 64 kbit/s | 6             | 0        | `Some(64000)` or `None`                  |
//! | 2    | 56 kbit/s | 5             | 1        | `Some(56000)`                            |
//! | 3    | 48 kbit/s | 4             | 2        | `Some(48000)`                            |
//!
//! Both encoder and decoder default to Mode 1 (64 kbit/s) when no bit rate
//! is specified.
//!
//! # Scope
//!
//! - The optional G.722 sync word / framing is **not** emitted — this crate
//!   is transparent on byte boundaries, leaving framing to the surrounding
//!   container.
//! - Explicit reset-on-keyframe — the decoder runs continuously across
//!   packet boundaries, which matches how G.722 is typically carried in
//!   RTP / SIP.
//! - Bit-exact compatibility with the ITU-T Table 6 / 7 / 8 quantiser and
//!   log-scale adapter is not yet implemented. The QMF coefficients and
//!   structure match SpanDSP/libg722 exactly (and pass a pure-QMF >30 dB
//!   roundtrip), but the ADPCM predictor + scale adapter use a simpler,
//!   self-consistent rule ([`band_low`] / [`band_high`]). Encoder and
//!   decoder share the same update rule so they stay in lock-step, giving
//!   >20 dB PSNR round trips at all three rates, but the on-wire bytes are
//!   not directly interchangeable with other G.722 implementations yet. See
//!   the module docs of [`band_low`] and [`band_high`] for details. The
//!   normative ITU inverse-quantiser tables are available as constants in
//!   [`tables`] for the eventual bit-exact port.
//!
//! Reference: ITU-T Recommendation G.722 (09/2012) and its Annex I public-domain
//! reference implementation, plus SpanDSP / libg722 (Steve Underwood, public
//! domain) for the QMF coefficient convention.

#![allow(
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::doc_lazy_continuation,
    clippy::doc_overindented_list_items
)]

pub mod band_high;
pub mod band_low;
pub mod decoder;
pub mod encoder;
pub mod mode;
pub mod qmf;
pub mod tables;

use oxideav_codec::CodecRegistry;
use oxideav_core::{CodecCapabilities, CodecId};

pub const CODEC_ID_STR: &str = "g722";

/// Register the G.722 decoder + encoder under the single codec id `"g722"`.
pub fn register(reg: &mut CodecRegistry) {
    let caps = CodecCapabilities::audio("g722_sw")
        .with_lossy(true)
        .with_intra_only(false)
        .with_max_channels(1)
        .with_max_sample_rate(16_000);
    reg.register_both(
        CodecId::new(CODEC_ID_STR),
        caps,
        decoder::make_decoder,
        encoder::make_encoder,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use oxideav_codec::Encoder;
    use oxideav_core::{CodecParameters, SampleFormat};

    fn params() -> CodecParameters {
        let mut p = CodecParameters::audio(CodecId::new(CODEC_ID_STR));
        p.sample_rate = Some(16_000);
        p.channels = Some(1);
        p.sample_format = Some(SampleFormat::S16);
        p
    }

    #[test]
    fn registers_both_directions() {
        let mut reg = CodecRegistry::new();
        register(&mut reg);
        let id = CodecId::new(CODEC_ID_STR);
        assert!(reg.has_decoder(&id));
        assert!(reg.has_encoder(&id));
    }

    #[test]
    fn rejects_wrong_sample_rate() {
        let mut p = params();
        p.sample_rate = Some(8_000);
        assert!(decoder::make_decoder(&p).is_err());
        assert!(encoder::make_encoder(&p).is_err());
    }

    #[test]
    fn rejects_stereo() {
        let mut p = params();
        p.channels = Some(2);
        assert!(decoder::make_decoder(&p).is_err());
        assert!(encoder::make_encoder(&p).is_err());
    }

    #[test]
    fn accepts_all_standard_bit_rates() {
        for br in [48_000u64, 56_000, 64_000] {
            let mut p = params();
            p.bit_rate = Some(br);
            assert!(
                decoder::make_decoder(&p).is_ok(),
                "decoder must accept {br} bit/s"
            );
            assert!(
                encoder::make_encoder(&p).is_ok(),
                "encoder must accept {br} bit/s"
            );
        }
    }

    #[test]
    fn rejects_unknown_bit_rate() {
        let mut p = params();
        p.bit_rate = Some(32_000u64);
        assert!(matches!(
            decoder::make_decoder(&p),
            Err(oxideav_core::Error::Unsupported(_))
        ));
        assert!(matches!(
            encoder::make_encoder(&p),
            Err(oxideav_core::Error::Unsupported(_))
        ));
    }

    #[test]
    fn default_bit_rate_is_mode_1_64k() {
        // No bit_rate set → encoder should report 64 kbit/s as its output.
        let enc = encoder::make_encoder(&params()).expect("encoder");
        assert_eq!(enc.output_params().bit_rate, Some(64_000));
    }
}

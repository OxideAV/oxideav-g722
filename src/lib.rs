//! # oxideav-g722
//!
//! Pure-Rust decoder for ITU-T G.722 wideband sub-band adaptive
//! differential PCM speech coding at 64 / 56 / 48 kbit/s
//! (Recommendation ITU-T G.722, 09/2012).
//!
//! ## Scope
//!
//! This crate currently provides the **receive (decoder) path** of
//! G.722 SB-ADPCM, structured around the bit-exact integer
//! computational details of clause 6.2 of the recommendation. Both
//! the lower (4 kHz) and the higher (4–8 kHz) sub-band adaptive
//! quantizers are implemented along with the 24-tap receive QMF that
//! interleaves the two sub-band signals into a 16 kHz output stream.
//!
//! Three bit-rate modes are supported (Table 1, clause 1.3):
//!
//! | Mode | Audio-coding rate | Lower sub-band code width | Aux-data |
//! | ---- | ----------------- | ------------------------- | -------- |
//! | 1    | 64 kbit/s         | 6 bit                     | 0 kbit/s |
//! | 2    | 56 kbit/s         | 5 bit                     | 8 kbit/s |
//! | 3    | 48 kbit/s         | 4 bit                     | 16 kbit/s|
//!
//! The encoder is not yet implemented; the registry slot only carries
//! a decoder factory.
//!
//! ## Provenance
//!
//! The implementation derives exclusively from the staged
//! `docs/audio/adpcm/g722/itu-t.G.722.2012.pdf` recommendation
//! (227 pages). Tables 4, 11, 14, 15-ILB, 17, 18, 19 and 21 of the
//! recommendation were transcribed by hand from the printed normative
//! tables; see `src/tables.rs` for the per-table provenance citation.
//! No external reference C implementation, no FFmpeg / libav* source,
//! no spandsp source, and no online resources were consulted during
//! this round.
//!
//! ## Usage
//!
//! ```
//! use oxideav_g722::{Decoder, Mode};
//!
//! let mut decoder = Decoder::new(Mode::Mode1);
//! let bitstream: &[u8] = &[0x80, 0x40, 0xC0, 0x10];
//! let samples = decoder.decode(bitstream);
//! assert_eq!(samples.len(), bitstream.len() * 2);
//! ```

#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

mod decoder;
mod tables;

pub use decoder::{Decoder, Mode};

use oxideav_core::RuntimeContext;

/// Crate-local error type.
///
/// At present every fallible API in the crate is the decoder itself,
/// which is infallible at the per-octet level (the spec is designed
/// so any 8-bit value is a valid octet, including the four substituted
/// codewords that arise from transmission errors). `Error` is kept in
/// the public API so future encoder / framing additions can return it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The requested operation is not yet implemented. Returned by
    /// the encoder factory: only decode is wired up so far.
    NotImplemented,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotImplemented => f.write_str("oxideav-g722: feature not implemented"),
        }
    }
}

impl std::error::Error for Error {}

/// Direct decoder factory mirroring the `make_decoder` convention used
/// by sibling codec crates.
///
/// Returns a decoder configured for the requested bit-rate mode. The
/// caller drives it sample-by-sample via [`Decoder::decode_octet`] /
/// [`Decoder::decode`].
pub fn make_decoder(mode: Mode) -> Decoder {
    Decoder::new(mode)
}

/// Encoder factory placeholder — encode path is not yet implemented.
pub fn make_encoder() -> Result<(), Error> {
    Err(Error::NotImplemented)
}

/// Registry entry-point. Currently a no-op since the workspace
/// registry contract is in flux for ADPCM codecs that carry mode
/// metadata out-of-band. Callers should use [`make_decoder`] directly.
pub fn register(_ctx: &mut RuntimeContext) {}

oxideav_core::register!("g722", register);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_is_non_empty() {
        assert!(!format!("{}", Error::NotImplemented).is_empty());
    }

    #[test]
    fn make_decoder_round_trips_mode() {
        let d = make_decoder(Mode::Mode2);
        assert_eq!(d.mode(), Mode::Mode2);
    }

    #[test]
    fn make_encoder_returns_not_implemented() {
        assert_eq!(make_encoder(), Err(Error::NotImplemented));
    }
}

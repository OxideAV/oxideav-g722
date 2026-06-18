//! # oxideav-g722
//!
//! Pure-Rust SB-ADPCM codec for ITU-T G.722 wideband audio at
//! 64 / 56 / 48 kbit/s.
//!
//! ## Scope
//!
//! Both directions of the SB-ADPCM coder are implemented in this
//! crate:
//!
//! * The **transmit path** ([`Encoder`]) packs 16 kHz / 14-bit PCM
//!   into the 64 kbit/s wire octet stream by way of the 24-tap
//!   transmit QMF (clause 3.1) and the 60- and 4-level forward
//!   adaptive quantizers of clauses 3.3 / 6.2.1.1 / 6.2.2.1.
//! * The **receive path** ([`Decoder`]) unpacks the same octet stream
//!   back into 16 kHz PCM via the 24-tap receive QMF (clause 4.4)
//!   and the symmetric inverse quantizers / pole-zero predictors of
//!   clauses 4 / 6.2.
//!
//! Three bit-rate modes are supported (Table 1, clause 1.3):
//!
//! | Mode | Audio-coding rate | Lower sub-band code width | Aux-data |
//! | ---- | ----------------- | ------------------------- | -------- |
//! | 1    | 64 kbit/s         | 6 bit                     | 0 kbit/s |
//! | 2    | 56 kbit/s         | 5 bit                     | 8 kbit/s |
//! | 3    | 48 kbit/s         | 4 bit                     | 16 kbit/s|
//!
//! The encoder is mode-agnostic: it always emits the full 6-bit
//! lower-sub-band codeword and leaves any auxiliary-data LSB
//! substitution to the optional "data insertion device" downstream
//! (Figure 1/G.722).
//!
//! ## Provenance
//!
//! The implementation derives exclusively from the staged
//! `docs/audio/g722/T-REC-G.722-198811-S.pdf` (the Blue-Book base
//! edition of the Recommendation). Tables 4, 11, 14, 15-ILB, 16,
//! 17, 18, 19, 20 and 21 of the Recommendation were transcribed by
//! hand from the printed normative tables; see `src/tables.rs` for
//! the per-table provenance citation. No external source code, no
//! external reference implementation, and no online resources were
//! consulted during the rebuild.
//!
//! ## Usage
//!
//! ```
//! use oxideav_g722::{Decoder, Encoder, Mode};
//!
//! // Encode 16 kHz PCM (here a four-sample silence) into G.722 octets.
//! let mut encoder = Encoder::new();
//! let octets = encoder.encode(&[0_i32; 4]);
//! assert_eq!(octets.len(), 2);
//!
//! // Decode those octets back to two 16 kHz PCM samples per octet.
//! let mut decoder = Decoder::new(Mode::Mode1);
//! let samples = decoder.decode(&octets);
//! assert_eq!(samples.len(), octets.len() * 2);
//! ```

#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

mod aux_data;
#[cfg(test)]
mod conformance;
mod decoder;
mod encoder;
mod predictor;
mod tables;
pub mod test_harness;
pub mod transmission;

pub use aux_data::{aux_bit_rate_kbps, aux_bits_per_octet, DataExtractor, DataInserter};
pub use decoder::{Decoder, Mode};
pub use encoder::Encoder;

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

/// Direct encoder factory mirroring the `make_encoder` convention used
/// by sibling codec crates.
///
/// Returns an SB-ADPCM encoder operating at the 64 kbit/s wire rate
/// (the lower-band code-word is always packed as 6 bits per clause 1.3;
/// the auxiliary-data substitution only affects the receive side, so
/// the transmit path has no mode parameter).
pub fn make_encoder() -> Encoder {
    Encoder::new()
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
    fn make_encoder_returns_a_fresh_encoder() {
        let mut enc = make_encoder();
        // An encoder fed all-zero input must emit valid octets and
        // pair the multiplexer order described in clause 1.4.4.
        let out = enc.encode(&[0_i32; 4]);
        assert_eq!(out.len(), 2, "encoder emits one octet per 2 PCM samples");
    }
}

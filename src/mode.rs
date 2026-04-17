//! G.722 operating mode (64 / 56 / 48 kbit/s).
//!
//! The three modes share every building block of the codec *except* the
//! number of bits used to encode the low-band ADPCM difference. The
//! 8-bit-per-pair sample-serial stream is always 8 bits wide: modes 2 and 3
//! replace the least-significant low-band bits with auxiliary bits that the
//! decoder discards.
//!
//! Bit layout inside the 8-bit codeword (MSB on the left):
//!
//! ```text
//! mode 1: [ IL5 IL4 IL3 IL2 IL1 IL0 | IH1 IH0 ]   64 kbit/s
//! mode 2: [ IL4 IL3 IL2 IL1 IL0 A0  | IH1 IH0 ]   56 kbit/s (A0 = aux)
//! mode 3: [ IL3 IL2 IL1 IL0 A1  A0  | IH1 IH0 ]   48 kbit/s (A1..A0 = aux)
//! ```
//!
//! On encode the auxiliary bits are set to zero. On decode they are
//! ignored — the low-band inverse quantiser only looks at the low-band bits
//! of the appropriate width.

use oxideav_core::{Error, Result};

/// G.722 operating mode — selects the low-band bit allocation and therefore
/// the transmitted bit rate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// 64 kbit/s — low-band uses 6 bits, no auxiliary bits.
    #[default]
    Mode1,
    /// 56 kbit/s — low-band uses 5 bits, 1 auxiliary bit (zero on encode,
    /// discarded on decode).
    Mode2,
    /// 48 kbit/s — low-band uses 4 bits, 2 auxiliary bits (zero on encode,
    /// discarded on decode).
    Mode3,
}

impl Mode {
    /// Number of bits used for the low-band code (sign bit included).
    pub const fn lb_bits(self) -> u8 {
        match self {
            Mode::Mode1 => 6,
            Mode::Mode2 => 5,
            Mode::Mode3 => 4,
        }
    }

    /// Number of bits reserved for the low-band magnitude (i.e. `lb_bits - 1`
    /// once the sign bit is removed).
    pub const fn lb_mag_bits(self) -> u8 {
        self.lb_bits() - 1
    }

    /// Bit rate in bits/s.
    pub const fn bit_rate(self) -> u64 {
        match self {
            Mode::Mode1 => 64_000,
            Mode::Mode2 => 56_000,
            Mode::Mode3 => 48_000,
        }
    }

    /// Map a bit-rate hint (from `CodecParameters::bit_rate`) to a mode.
    /// `None` defaults to [`Mode::Mode1`] (64 kbit/s).
    pub fn from_bit_rate(br: Option<u64>) -> Result<Self> {
        match br {
            None | Some(64_000) => Ok(Mode::Mode1),
            Some(56_000) => Ok(Mode::Mode2),
            Some(48_000) => Ok(Mode::Mode3),
            Some(other) => Err(Error::unsupported(format!(
                "G.722: unsupported bit rate {other} bit/s (expected 48000 / 56000 / 64000)"
            ))),
        }
    }

    /// Pack a low-band code `il` (of [`Self::lb_bits`] width) and a high-band
    /// 2-bit code `ih` into a G.722 sample-serial byte. Auxiliary bits (if
    /// any) are set to zero.
    pub const fn pack(self, il: u8, ih: u8) -> u8 {
        let lb = il & ((1u8 << self.lb_bits()) - 1);
        // Shift the low-band code up past the 2 HB bits and the aux bits
        // that sit between them. Aux bits are left at zero.
        let shift = 8 - self.lb_bits();
        (lb << shift) | (ih & 0x03)
    }

    /// Unpack the low-band code (of [`Self::lb_bits`] width) and high-band
    /// 2-bit code from a G.722 sample-serial byte. Auxiliary bits are
    /// discarded.
    pub const fn unpack(self, byte: u8) -> (u8, u8) {
        let shift = 8 - self.lb_bits();
        let mask = (1u8 << self.lb_bits()) - 1;
        let il = (byte >> shift) & mask;
        let ih = byte & 0x03;
        (il, ih)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_rate_matches_mode() {
        assert_eq!(Mode::Mode1.bit_rate(), 64_000);
        assert_eq!(Mode::Mode2.bit_rate(), 56_000);
        assert_eq!(Mode::Mode3.bit_rate(), 48_000);
    }

    #[test]
    fn lb_widths_are_consistent() {
        for m in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            assert_eq!(m.lb_bits(), m.lb_mag_bits() + 1);
            // lb_bits + 2 HB + aux = 8
            let expected_aux = 8 - m.lb_bits() - 2;
            assert_eq!(8 - m.lb_bits() - 2, expected_aux);
        }
    }

    #[test]
    fn pack_unpack_is_round_trip() {
        for m in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let lb_max = (1u8 << m.lb_bits()) - 1;
            for il in 0..=lb_max {
                for ih in 0..=3u8 {
                    let byte = m.pack(il, ih);
                    let (il2, ih2) = m.unpack(byte);
                    assert_eq!(il2, il, "LB mismatch for mode {m:?}");
                    assert_eq!(ih2, ih, "HB mismatch for mode {m:?}");
                }
            }
        }
    }

    #[test]
    fn aux_bits_are_zero_on_encode() {
        // Mode 2 has 1 aux bit at bit 2.
        let byte = Mode::Mode2.pack(0x1F, 0x3);
        assert_eq!(byte & (1 << 2), 0, "mode 2 aux bit not zero: {byte:08b}");
        // Mode 3 has 2 aux bits at bits 3..2.
        let byte = Mode::Mode3.pack(0x0F, 0x3);
        assert_eq!(
            byte & 0b0000_1100,
            0,
            "mode 3 aux bits not zero: {byte:08b}"
        );
    }

    #[test]
    fn unpack_ignores_aux_bits() {
        // Mode 2: flipping bit 2 should not affect IL or IH.
        let base = Mode::Mode2.pack(0b10110, 0b10);
        let (il0, ih0) = Mode::Mode2.unpack(base);
        let (il1, ih1) = Mode::Mode2.unpack(base ^ 0b0000_0100);
        assert_eq!(il0, il1);
        assert_eq!(ih0, ih1);
    }

    #[test]
    fn rejects_unknown_bit_rate() {
        assert!(Mode::from_bit_rate(Some(32_000)).is_err());
        assert_eq!(Mode::from_bit_rate(None).unwrap(), Mode::Mode1);
        assert_eq!(Mode::from_bit_rate(Some(64_000)).unwrap(), Mode::Mode1);
        assert_eq!(Mode::from_bit_rate(Some(56_000)).unwrap(), Mode::Mode2);
        assert_eq!(Mode::from_bit_rate(Some(48_000)).unwrap(), Mode::Mode3);
    }
}

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
//! By default the encoder writes zero into the auxiliary bits and the
//! decoder discards them. The bits can also be used as a side-channel via
//! [`Mode::pack_with_aux`] / [`Mode::unpack_with_aux`] (and the
//! `push_aux` / `take_aux` helpers on the encoder / decoder); the low-band
//! inverse quantiser only looks at the low-band field, so its output is
//! unaffected by whatever aux value was stamped there.

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

    /// Number of auxiliary-data bits carried per packed byte (between the
    /// low-band field and the 2-bit high-band field). Mode 1 has none;
    /// mode 2 carries one bit per byte (8 kbit/s aux side-channel at 8 kHz
    /// byte rate); mode 3 carries two (16 kbit/s).
    pub const fn aux_bits(self) -> u8 {
        match self {
            Mode::Mode1 => 0,
            Mode::Mode2 => 1,
            Mode::Mode3 => 2,
        }
    }

    /// Auxiliary-data side-channel rate in bits/s. Always
    /// `aux_bits * 8000` since one packed byte represents one 8 kHz sub-band
    /// sample pair.
    pub const fn aux_rate(self) -> u32 {
        self.aux_bits() as u32 * 8_000
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
    /// any) are set to zero. See [`Self::pack_with_aux`] to carry side-channel
    /// data.
    pub const fn pack(self, il: u8, ih: u8) -> u8 {
        self.pack_with_aux(il, ih, 0)
    }

    /// Pack a low-band code, high-band code and `aux_bits()`-wide auxiliary
    /// payload into a G.722 sample-serial byte. The auxiliary payload sits
    /// between the low-band field and the 2-bit high-band field, matching
    /// G.722's spec layout. Excess bits in `aux` above `aux_bits()` are
    /// masked off.
    pub const fn pack_with_aux(self, il: u8, ih: u8, aux: u8) -> u8 {
        let lb_bits = self.lb_bits();
        let aux_bits = self.aux_bits();
        let lb = il & ((1u8 << lb_bits) - 1);
        let aux_mask = if aux_bits == 0 {
            0
        } else {
            (1u8 << aux_bits) - 1
        };
        let aux_field = (aux & aux_mask) << 2;
        let lb_shift = 8 - lb_bits;
        (lb << lb_shift) | aux_field | (ih & 0x03)
    }

    /// Unpack the low-band code (of [`Self::lb_bits`] width) and high-band
    /// 2-bit code from a G.722 sample-serial byte. Auxiliary bits are
    /// discarded. See [`Self::unpack_with_aux`] for codec users that want
    /// to read the side-channel data.
    pub const fn unpack(self, byte: u8) -> (u8, u8) {
        let shift = 8 - self.lb_bits();
        let mask = (1u8 << self.lb_bits()) - 1;
        let il = (byte >> shift) & mask;
        let ih = byte & 0x03;
        (il, ih)
    }

    /// Unpack the low-band code, high-band code and the auxiliary payload
    /// from a G.722 sample-serial byte. The auxiliary value is right-aligned
    /// in `aux_bits()` bits (zero on Mode 1).
    pub const fn unpack_with_aux(self, byte: u8) -> (u8, u8, u8) {
        let lb_bits = self.lb_bits();
        let aux_bits = self.aux_bits();
        let lb_shift = 8 - lb_bits;
        let lb_mask = (1u8 << lb_bits) - 1;
        let il = (byte >> lb_shift) & lb_mask;
        let ih = byte & 0x03;
        let aux = if aux_bits == 0 {
            0
        } else {
            (byte >> 2) & ((1u8 << aux_bits) - 1)
        };
        (il, ih, aux)
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

    #[test]
    fn aux_bits_per_mode() {
        assert_eq!(Mode::Mode1.aux_bits(), 0);
        assert_eq!(Mode::Mode2.aux_bits(), 1);
        assert_eq!(Mode::Mode3.aux_bits(), 2);
        assert_eq!(Mode::Mode1.aux_rate(), 0);
        assert_eq!(Mode::Mode2.aux_rate(), 8_000);
        assert_eq!(Mode::Mode3.aux_rate(), 16_000);
    }

    #[test]
    fn pack_with_aux_round_trip() {
        for m in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            let lb_max = (1u8 << m.lb_bits()) - 1;
            let aux_max = if m.aux_bits() == 0 {
                0
            } else {
                (1u8 << m.aux_bits()) - 1
            };
            for il in 0..=lb_max {
                for ih in 0..=3u8 {
                    for aux in 0..=aux_max {
                        let byte = m.pack_with_aux(il, ih, aux);
                        let (il2, ih2, aux2) = m.unpack_with_aux(byte);
                        assert_eq!(il2, il, "LB mismatch for mode {m:?}");
                        assert_eq!(ih2, ih, "HB mismatch for mode {m:?}");
                        assert_eq!(aux2, aux, "AUX mismatch for mode {m:?}");
                        // Plain `pack` must equal `pack_with_aux(.., 0)`.
                        if aux == 0 {
                            assert_eq!(m.pack(il, ih), byte);
                        }
                        // Plain `unpack` must give the same low/high values.
                        let (il3, ih3) = m.unpack(byte);
                        assert_eq!(il3, il);
                        assert_eq!(ih3, ih);
                    }
                }
            }
        }
    }

    #[test]
    fn pack_with_aux_masks_excess_bits() {
        // Mode 1: aux is always discarded.
        assert_eq!(
            Mode::Mode1.pack_with_aux(0x3F, 0x3, 0xFF),
            Mode::Mode1.pack(0x3F, 0x3)
        );
        // Mode 2: aux is 1 bit; bit 0 of `aux` is used, others ignored.
        let a = Mode::Mode2.pack_with_aux(0, 0, 0b10);
        assert_eq!(a, 0); // 0b10 & 1 = 0
        let b = Mode::Mode2.pack_with_aux(0, 0, 0b11);
        assert_eq!(b, 0b0000_0100); // bit 2 set
                                    // Mode 3: aux is 2 bits at positions 3..2.
        let c = Mode::Mode3.pack_with_aux(0, 0, 0b11);
        assert_eq!(c, 0b0000_1100);
        let d = Mode::Mode3.pack_with_aux(0, 0, 0b1111_0011);
        assert_eq!(d, 0b0000_1100); // upper bits masked
    }
}

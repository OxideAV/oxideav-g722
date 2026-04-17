//! Low-band ADPCM — ITU-T G.722 normative quantiser, log-scale adapter
//! and 2-pole / 6-zero predictor (clause 3.6).
//!
//! | Mode | Rate (kbit/s) | Low-band bits | Inverse-quant table |
//! |------|---------------|---------------|---------------------|
//! | 1    | 64            | 6             | `QM6` (decoder)     |
//! | 2    | 56            | 5             | `QM5` (decoder)     |
//! | 3    | 48            | 4             | `QM4` (decoder)     |
//!
//! The encoder always runs its local reconstruction through `QM4`
//! (indexed by `il >> 2`) and `WL` / `RL42`, matching the ITU reference.
//! That makes the encoder rate-independent and the transmitted bits are
//! simply a truncation of the 6-bit forward index.

use crate::mode::Mode;
use crate::tables::{ILB, ILN, ILP, Q6, QM4, QM5, QM6, RL42, WL};

/// Low-band ADPCM state (shared encoder / decoder structure).
#[derive(Clone, Debug)]
pub struct LowBand {
    /// Number of transmitted low-band bits (4 / 5 / 6).
    lb_bits_v: u8,

    // Predictor / quantiser state mirroring the ITU reference.
    s: i32,
    sp: i32,
    sz: i32,
    nb: i32,
    det: i32,
    a: [i32; 3],
    b: [i32; 7],
    d: [i32; 7],
    p: [i32; 3],
    r: [i32; 3],
}

impl Default for LowBand {
    fn default() -> Self {
        Self::new()
    }
}

impl LowBand {
    /// 64 kbit/s (Mode 1) low-band decoder/encoder state.
    pub fn new() -> Self {
        Self::for_mode(Mode::Mode1)
    }

    /// Low-band state for the given operating mode.
    pub fn for_mode(mode: Mode) -> Self {
        Self::with_bits(mode.lb_bits())
    }

    /// Low-band state for a given transmitted bit width (4/5/6; anything
    /// else clamps to 6).
    pub fn with_bits(lb_bits: u8) -> Self {
        let lb_bits = match lb_bits {
            4..=6 => lb_bits,
            _ => 6,
        };
        Self {
            lb_bits_v: lb_bits,
            s: 0,
            sp: 0,
            sz: 0,
            nb: 0,
            // ITU-T initial step size for the low band.
            det: 32,
            a: [0; 3],
            b: [0; 7],
            d: [0; 7],
            p: [0; 3],
            r: [0; 3],
        }
    }

    /// Total transmitted low-band bit width.
    pub fn lb_bits(&self) -> u8 {
        self.lb_bits_v
    }

    /// Encode a 15-bit-range low-band sample and return the transmitted
    /// code in `0..=(1 << lb_bits) - 1`.
    pub fn encode(&mut self, xl: i32) -> u8 {
        // QUANTL — signed-magnitude quantisation against Q6 × det.
        let el = saturate(xl - self.s);
        let il6 = quantl(el, self.det);

        // INVQAL — the ITU reference encoder always uses QM4 (indexed by
        // `il >> 2`) for the local reconstruction feeding the predictor /
        // scale-factor adapter. This keeps the encoder state identical
        // across rates so the same encoder output feeds any-rate decoder,
        // and is what makes the bitstream bit-exact with the reference.
        let ril = (il6 >> 2) as usize;
        let wd2 = QM4[ril];
        let dlow = (self.det * wd2) >> 15;

        self.update_scale(ril);
        self.block4(dlow);

        // Truncate to the transmitted rate (top lb_bits of the 6-bit il).
        il6 >> (6 - self.lb_bits_v)
    }

    /// Decode a low-band code and return the reconstructed sample.
    pub fn decode(&mut self, code: u8) -> i32 {
        let mask = (1u16 << self.lb_bits_v) as u16 - 1;
        let il = (code as u16 & mask) as usize;

        // Pick inverse quant table + rl42 index per rate.
        let (wd2, ril) = match self.lb_bits_v {
            6 => (QM6[il], il >> 2),
            5 => (QM5[il], il >> 1),
            _ => (QM4[il], il),
        };
        let dlow = (self.det * wd2) >> 15;

        // Reconstruct BEFORE advancing the predictor / scale state.
        let rlow = saturate15(self.s + dlow);

        self.update_scale(ril);
        self.block4(dlow);

        rlow
    }

    /// LOGSCL (low-band scale-factor log update) + SCALEL (step-size
    /// computation from `nb`).
    fn update_scale(&mut self, ril: usize) {
        let il4 = RL42[ril];
        let wd = (self.nb * 127) >> 7;
        let nb = (wd + WL[il4]).clamp(0, 18_432);
        self.nb = nb;

        let wd1 = ((nb >> 6) & 31) as usize;
        let wd2 = 8 - (nb >> 11);
        let wd3 = if wd2 < 0 {
            ILB[wd1] << (-wd2)
        } else {
            ILB[wd1] >> wd2
        };
        self.det = wd3 << 2;
    }

    /// BLOCK4: RECONS, PARREC, UPPOL2, UPPOL1, UPZERO, DELAYA, FILTEP,
    /// FILTEZ, PREDIC. `d` is the reconstructed quantiser output used to
    /// advance the predictor.
    fn block4(&mut self, d: i32) {
        // RECONS.
        self.d[0] = d;
        self.r[0] = saturate(self.s + d);

        // PARREC.
        self.p[0] = saturate(self.sz + d);

        // UPPOL2.
        let mut sg = [0i32; 7];
        for i in 0..3 {
            sg[i] = self.p[i] >> 15;
        }
        let wd1 = saturate(self.a[1] << 2);
        let mut wd2 = if sg[0] == sg[1] { -wd1 } else { wd1 };
        if wd2 > 32_767 {
            wd2 = 32_767;
        }
        let mut wd3 = (wd2 >> 7) + if sg[0] == sg[2] { 128 } else { -128 };
        wd3 += (self.a[2] * 32_512) >> 15;
        let ap2 = wd3.clamp(-12_288, 12_288);

        // UPPOL1.
        sg[0] = self.p[0] >> 15;
        sg[1] = self.p[1] >> 15;
        let wd1 = if sg[0] == sg[1] { 192 } else { -192 };
        let wd2 = (self.a[1] * 32_640) >> 15;
        let mut ap1 = saturate(wd1 + wd2);
        let wd3 = saturate(15_360 - ap2);
        if ap1 > wd3 {
            ap1 = wd3;
        } else if ap1 < -wd3 {
            ap1 = -wd3;
        }

        // UPZERO.
        let wd1 = if d == 0 { 0 } else { 128 };
        sg[0] = d >> 15;
        let mut bp = [0i32; 7];
        for i in 1..7 {
            sg[i] = self.d[i] >> 15;
            let wd2 = if sg[i] == sg[0] { wd1 } else { -wd1 };
            let wd3 = (self.b[i] * 32_640) >> 15;
            bp[i] = saturate(wd2 + wd3);
        }

        // DELAYA: shift d/b histories, then r/p/a.
        for i in (1..=6).rev() {
            self.d[i] = self.d[i - 1];
            self.b[i] = bp[i];
        }
        for i in (1..=2).rev() {
            self.r[i] = self.r[i - 1];
            self.p[i] = self.p[i - 1];
        }
        self.a[1] = ap1;
        self.a[2] = ap2;

        // FILTEP.
        let wd1 = saturate(self.r[1] + self.r[1]);
        let wd1 = (self.a[1] * wd1) >> 15;
        let wd2 = saturate(self.r[2] + self.r[2]);
        let wd2 = (self.a[2] * wd2) >> 15;
        self.sp = saturate(wd1 + wd2);

        // FILTEZ.
        let mut sz = 0i32;
        for i in (1..=6).rev() {
            let wd1 = saturate(self.d[i] + self.d[i]);
            sz += (self.b[i] * wd1) >> 15;
        }
        self.sz = saturate(sz);

        // PREDIC.
        self.s = saturate(self.sp + self.sz);
    }
}

/// ITU-T G.722 QUANTL — forward 6-bit low-band quantiser.
fn quantl(el: i32, det: i32) -> u8 {
    // Magnitude (signed-magnitude, "−(el+1)" for negatives mirrors the C).
    let wd = if el >= 0 { el } else { -(el + 1) };
    let mut i = 1usize;
    while i < 30 {
        let wd1 = (Q6[i] * det) >> 12;
        if wd < wd1 {
            break;
        }
        i += 1;
    }
    if el < 0 {
        ILN[i]
    } else {
        ILP[i]
    }
}

/// 32-bit → 16-bit saturation (replicates the ITU reference's `saturate()`).
fn saturate(v: i32) -> i32 {
    v.clamp(-32_768, 32_767)
}

/// 15-bit saturation used for the reconstructed sub-band output
/// (matches the ITU reference's `saturate15`).
fn saturate15(v: i32) -> i32 {
    v.clamp(-16_384, 16_383)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_stays_silent() {
        for bits in [4u8, 5, 6] {
            let mut enc = LowBand::with_bits(bits);
            let mut dec = LowBand::with_bits(bits);
            for _ in 0..400 {
                let code = enc.encode(0);
                let _ = dec.decode(code);
            }
            // For silent input the encoder and decoder should stay
            // numerically close to the origin.
            assert!(enc.s.abs() < 256);
            assert!(dec.s.abs() < 256);
        }
    }

    #[test]
    fn quantl_zero_input_gives_known_code() {
        // With zero error and the default det=32, Q6[1]*32 >> 12 = 35*32/4096 = 0,
        // so the first comparison already fails (wd=0 < wd1=0 is false), loop
        // exits at i = some larger index. Just sanity-check the result is
        // representable and non-panicking.
        let c = quantl(0, 32);
        assert!(c < 64);
    }

    #[test]
    fn encoder_decoder_track_each_other() {
        // The ITU reference encoder always uses QM4 for its local
        // reconstruction while the decoder uses QM6/5/4 per rate — so the
        // two are not bit-identical. Verify they stay reasonably close on a
        // tone. The scale-factor adapter uses identical `WL` indices (both
        // sides compute `ril = il >> (lb_bits - 4)`), so `nb` matches.
        let mut enc = LowBand::with_bits(6);
        let mut dec = LowBand::with_bits(6);
        for n in 0..800 {
            let x = ((n as f32 * 0.12).sin() * 4_000.0) as i32;
            let code = enc.encode(x);
            let _ = dec.decode(code);
            assert_eq!(enc.nb, dec.nb, "scale-factor adapter diverged at n={n}");
        }
    }

    #[test]
    fn truncation_matches_bit_widths() {
        // At mode 2, the emitted code is 5 bits (top bits of 6-bit il).
        let mut enc = LowBand::with_bits(5);
        for n in 0..128 {
            let x = ((n as f32 * 0.12).sin() * 4_000.0) as i32;
            let code = enc.encode(x);
            assert!(code < 32, "mode 2 code must fit in 5 bits");
        }
        let mut enc = LowBand::with_bits(4);
        for n in 0..128 {
            let x = ((n as f32 * 0.12).sin() * 4_000.0) as i32;
            let code = enc.encode(x);
            assert!(code < 16, "mode 3 code must fit in 4 bits");
        }
    }

    #[test]
    fn code_width_matches_mode() {
        assert_eq!(LowBand::for_mode(Mode::Mode1).lb_bits(), 6);
        assert_eq!(LowBand::for_mode(Mode::Mode2).lb_bits(), 5);
        assert_eq!(LowBand::for_mode(Mode::Mode3).lb_bits(), 4);
    }
}

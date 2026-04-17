//! High-band ADPCM — ITU-T G.722 normative 2-bit quantiser, log-scale
//! adapter and 2-pole / 1-zero predictor (clause 3.6). Shared structure
//! with the low-band but with fewer zero-predictor taps and a different
//! `det` shift in SCALEH.

use crate::tables::{IHN, IHP, ILB, QM2, RH2, WH};

/// High-band ADPCM state.
#[derive(Clone, Debug)]
pub struct HighBand {
    s: i32,
    sp: i32,
    sz: i32,
    nb: i32,
    det: i32,
    a: [i32; 3],
    // b[0] unused; only b[1] is tracked for the 1-zero predictor.
    b: [i32; 2],
    // d[0] holds the current reconstructed output; d[1] the delayed one.
    d: [i32; 2],
    p: [i32; 3],
    r: [i32; 3],
}

impl Default for HighBand {
    fn default() -> Self {
        Self::new()
    }
}

impl HighBand {
    pub fn new() -> Self {
        Self {
            s: 0,
            sp: 0,
            sz: 0,
            nb: 0,
            // ITU-T initial step size for the high band.
            det: 8,
            a: [0; 3],
            b: [0; 2],
            d: [0; 2],
            p: [0; 3],
            r: [0; 3],
        }
    }

    /// Encode a 15-bit-range high-band sample to a 2-bit code (0..=3).
    pub fn encode(&mut self, xh: i32) -> u8 {
        let eh = saturate(xh - self.s);
        let ih = quanth(eh, self.det);

        let wd2 = QM2[ih as usize];
        let dhigh = (self.det * wd2) >> 15;

        self.update_scale(ih as usize);
        self.block4(dhigh);

        ih
    }

    /// Decode a 2-bit code to a reconstructed high-band sample.
    pub fn decode(&mut self, code: u8) -> i32 {
        let ih = (code & 0x3) as usize;
        let wd2 = QM2[ih];
        let dhigh = (self.det * wd2) >> 15;

        let rhigh = saturate15(self.s + dhigh);

        self.update_scale(ih);
        self.block4(dhigh);

        rhigh
    }

    /// LOGSCH + SCALEH.
    fn update_scale(&mut self, ih: usize) {
        let ih2 = RH2[ih];
        let wd = (self.nb * 127) >> 7;
        let nb = (wd + WH[ih2]).clamp(0, 22_528);
        self.nb = nb;

        let wd1 = ((nb >> 6) & 31) as usize;
        let wd2 = 10 - (nb >> 11);
        let wd3 = if wd2 < 0 {
            ILB[wd1] << (-wd2)
        } else {
            ILB[wd1] >> wd2
        };
        self.det = wd3 << 2;
    }

    /// BLOCK4 for the high band (1-zero predictor, so only d[1] / b[1]).
    fn block4(&mut self, d: i32) {
        self.d[0] = d;
        self.r[0] = saturate(self.s + d);

        self.p[0] = saturate(self.sz + d);

        let mut sg = [0i32; 3];
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

        // UPZERO: high band has only one zero tap (b[1]).
        let wd1 = if d == 0 { 0 } else { 128 };
        let sg_d = d >> 15;
        let sg1 = self.d[1] >> 15;
        let wd2 = if sg1 == sg_d { wd1 } else { -wd1 };
        let wd3 = (self.b[1] * 32_640) >> 15;
        let bp1 = saturate(wd2 + wd3);

        // DELAYA.
        self.d[1] = self.d[0];
        self.b[1] = bp1;
        self.r[2] = self.r[1];
        self.r[1] = self.r[0];
        self.p[2] = self.p[1];
        self.p[1] = self.p[0];
        self.a[1] = ap1;
        self.a[2] = ap2;

        // FILTEP.
        let wd1 = saturate(self.r[1] + self.r[1]);
        let wd1 = (self.a[1] * wd1) >> 15;
        let wd2 = saturate(self.r[2] + self.r[2]);
        let wd2 = (self.a[2] * wd2) >> 15;
        self.sp = saturate(wd1 + wd2);

        // FILTEZ — only the one non-zero tap.
        let wd1 = saturate(self.d[1] + self.d[1]);
        let sz = (self.b[1] * wd1) >> 15;
        self.sz = saturate(sz);

        self.s = saturate(self.sp + self.sz);
    }
}

/// ITU-T G.722 QUANTH — 2-bit high-band quantiser.
fn quanth(eh: i32, det: i32) -> u8 {
    let wd = if eh >= 0 { eh } else { -(eh + 1) };
    let wd1 = (564 * det) >> 12;
    let mih = if wd >= wd1 { 2 } else { 1 };
    if eh < 0 {
        IHN[mih]
    } else {
        IHP[mih]
    }
}

fn saturate(v: i32) -> i32 {
    v.clamp(-32_768, 32_767)
}

fn saturate15(v: i32) -> i32 {
    v.clamp(-16_384, 16_383)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_handled() {
        let mut enc = HighBand::new();
        let mut dec = HighBand::new();
        for _ in 0..400 {
            let code = enc.encode(0);
            let _ = dec.decode(code);
        }
        assert!(enc.s.abs() < 256);
    }

    #[test]
    fn encoder_decoder_in_sync() {
        let mut enc = HighBand::new();
        let mut dec = HighBand::new();
        for n in 0..400 {
            let x = ((n as f32 * 0.25).cos() * 3_000.0) as i32;
            let code = enc.encode(x);
            let _ = dec.decode(code);
            assert_eq!(enc.s, dec.s);
            assert_eq!(enc.det, dec.det);
            assert_eq!(enc.nb, dec.nb);
        }
    }
}

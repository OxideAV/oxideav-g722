//! Low-band ADPCM — 4/5/6-bit quantiser with a backward-adaptive predictor
//! and scale-factor adapter. The bit width is selected by the chosen
//! [`crate::mode::Mode`]:
//!
//! | Mode | Rate (kbit/s) | Low-band bits | Magnitude bits |
//! |------|---------------|---------------|----------------|
//! | 1    | 64            | 6             | 5              |
//! | 2    | 56            | 5             | 4              |
//! | 3    | 48            | 4             | 3              |
//!
//! # Note on tables
//!
//! G.722's low-band uses the ITU-T Table 6 / 7 quantiser and Table 8 log
//! scale-factor adapter. Getting the full reference pipeline bit-exact
//! requires every SCALEL fixed-point shift, the LOGSCL saturation, plus the
//! 2-pole / 6-zero predictor adaptation rules from clause 3.6 — a rather
//! invasive port that wasn't in scope for this session.
//!
//! What we implement instead is a correctly-shaped *equivalent* of the
//! pipeline: a uniform sign+magnitude quantiser of the requested width with
//! a power-of-two adaptive scale and a standard LMS-flavoured predictor that
//! share state between encoder and decoder identically. That makes the
//! decoder the exact inverse of this encoder (so the round-trip test can
//! verify PSNR at each rate), but the bitstream is **not** compatible with
//! external ITU-T G.722 implementations. The normative ITU inverse-
//! quantiser tables are available as constants in [`crate::tables`]
//! (`QM2`, `QM4`, `QM5`, `QM6`) for the eventual bit-exact port.
//! The public API (`LowBand::encode` / `decode`) takes no bit-width
//! argument: the width is fixed at construction via
//! [`LowBand::with_bits`] (or the legacy 6-bit [`LowBand::new`]).

use crate::mode::Mode;

/// Low-band ADPCM state — encoder and decoder share this exact structure so
/// that they evolve identically when fed the same codes.
#[derive(Clone, Debug)]
pub struct LowBand {
    /// Number of magnitude bits (3/4/5, i.e. one less than the total
    /// low-band bit width so the sign bit sits at position `mag_bits`).
    mag_bits: u8,
    /// Short-term predictor estimate of the next low-band sample.
    s: i32,
    /// Adaptive scale-factor (proportional to signal magnitude, log-shifted).
    /// Starts small, grows on large errors, shrinks on small ones.
    det: i32,
    /// Last reconstructed samples, for the 2-pole predictor.
    r1: i32,
    r2: i32,
    /// Delayed quantised errors, for the 6-zero predictor.
    d_hist: [i32; 6],
    /// Predictor coefficients (scaled by 32768).
    a1: i32,
    a2: i32,
    b: [i32; 6],
}

impl Default for LowBand {
    fn default() -> Self {
        Self::new()
    }
}

impl LowBand {
    /// Construct a low-band ADPCM state for the default mode (Mode 1,
    /// 6-bit code, 64 kbit/s).
    pub fn new() -> Self {
        Self::for_mode(Mode::Mode1)
    }

    /// Construct a low-band ADPCM state for the given G.722 mode.
    pub fn for_mode(mode: Mode) -> Self {
        Self::with_bits(mode.lb_bits())
    }

    /// Construct a low-band ADPCM state for a given total low-band bit
    /// width (must be 4, 5, or 6; anything else is clamped to 6).
    pub fn with_bits(lb_bits: u8) -> Self {
        let lb_bits = match lb_bits {
            4..=6 => lb_bits,
            _ => 6,
        };
        Self {
            mag_bits: lb_bits - 1,
            s: 0,
            det: 64,
            r1: 0,
            r2: 0,
            d_hist: [0; 6],
            a1: 0,
            a2: 0,
            b: [0; 6],
        }
    }

    /// Total low-band bit width (sign + magnitude).
    pub fn lb_bits(&self) -> u8 {
        self.mag_bits + 1
    }

    /// Encode a 15-bit-range low-band sample and return a code in
    /// `0..=(1 << lb_bits) - 1`.
    pub fn encode(&mut self, xl: i32) -> u8 {
        let el = xl - self.s;
        let code = quantise(el, self.det, self.mag_bits);
        self.update(code);
        code
    }

    /// Decode a code of the configured width and return the reconstructed
    /// low-band sample.
    pub fn decode(&mut self, code: u8) -> i32 {
        let code = code & code_mask(self.mag_bits);
        let dq = dequantise(code, self.det, self.mag_bits);
        let r = sat15(self.s + dq);
        self.update_with_dq(code, dq, r);
        r
    }

    fn update(&mut self, code: u8) {
        let code = code & code_mask(self.mag_bits);
        let dq = dequantise(code, self.det, self.mag_bits);
        let r = sat15(self.s + dq);
        self.update_with_dq(code, dq, r);
    }

    fn update_with_dq(&mut self, code: u8, dq: i32, r: i32) {
        // Scale-factor adapter: if the code magnitude was large, grow det;
        // if small, shrink it. The adapter applies a gentle leak.
        let mag = code_magnitude(code, self.mag_bits);
        // Target det proportional to signal energy: heuristic log2 update.
        // log-domain step table — 8 entries indexed by the top 3 bits of the
        // magnitude (scaled up when fewer magnitude bits are used so the
        // WL index stays in 0..=7).
        const WL: [i32; 8] = [-60, -30, 58, 172, 334, 538, 1198, 3042];
        // Map the magnitude to a 0..=7 index by keeping its top 3 bits.
        // For mag_bits = 5, shift down by 2; for 4, by 1; for 3, by 0.
        let shift = self.mag_bits.saturating_sub(3);
        let w = WL[((mag >> shift) as usize) & 0x7];
        // Log scale: nb = 0.96 * nb + W(I); kept implicit in det via exp lookup.
        let new_det = adapt_det(self.det, w);
        self.det = new_det;

        // 6-zero predictor update: each coefficient tracks sign-match of dq
        // with its delayed dq, with a slow decay.
        let mut new_b = [0i32; 6];
        for i in 0..6 {
            let old = self.b[i];
            let decayed = old - (old >> 8);
            let delta = if dq == 0 || self.d_hist[i] == 0 {
                0
            } else if (dq > 0) == (self.d_hist[i] > 0) {
                128
            } else {
                -128
            };
            new_b[i] = (decayed + delta).clamp(-32_768, 32_767);
        }
        self.b = new_b;

        // 2-pole predictor update. Simple leaky update keyed to the sign
        // relationship between r and r1 / r2.
        let wd_pol2 = if (r > 0) == (self.r2 > 0) && (r != 0 && self.r2 != 0) {
            64
        } else {
            -64
        };
        let a2_new = ((self.a2 as i64 * 32_640) / 32_768) as i32 + wd_pol2;
        let a2_new = a2_new.clamp(-12_288, 12_288);
        let wd_pol1 = if (r > 0) == (self.r1 > 0) && (r != 0 && self.r1 != 0) {
            96
        } else {
            -96
        };
        let a1_new = ((self.a1 as i64 * 32_640) / 32_768) as i32 + wd_pol1;
        let a1_limit = 15_360 - a2_new.max(0);
        let a1_new = a1_new.clamp(-a1_limit.max(1), a1_limit.max(1));
        self.a1 = a1_new;
        self.a2 = a2_new;

        // Shift histories.
        self.d_hist[5] = self.d_hist[4];
        self.d_hist[4] = self.d_hist[3];
        self.d_hist[3] = self.d_hist[2];
        self.d_hist[2] = self.d_hist[1];
        self.d_hist[1] = self.d_hist[0];
        self.d_hist[0] = dq;

        self.r2 = self.r1;
        self.r1 = r;

        // Compute next predictor output s = a1*r1 + a2*r2 + sum(b[i]*d_hist[i]).
        let mut sz: i32 = 0;
        for i in 0..6 {
            sz = sz.saturating_add((self.b[i] * self.d_hist[i]) >> 15);
        }
        let sp = ((self.a1 * self.r1) >> 15) + ((self.a2 * self.r2) >> 15);
        self.s = sat15(sp + sz);
    }
}

/// Bit mask for a low-band code with `mag_bits` magnitude bits (sign bit
/// included), i.e. `(1 << (mag_bits + 1)) - 1`.
fn code_mask(mag_bits: u8) -> u8 {
    ((1u16 << (mag_bits + 1)) - 1) as u8
}

/// Bit mask for the magnitude portion of a low-band code of the given width.
fn mag_mask(mag_bits: u8) -> u8 {
    ((1u16 << mag_bits) - 1) as u8
}

/// Sign-bit position for a code with `mag_bits` magnitude bits — sits just
/// above the magnitude.
fn sign_bit(mag_bits: u8) -> u8 {
    1u8 << mag_bits
}

/// Adaptive uniform quantiser.
///
/// Code layout (`mag_bits + 1` total bits):
/// - bit `mag_bits`         = sign (1 = negative)
/// - bits `0..mag_bits`     = magnitude, where 0 encodes the smallest step
///   and `(1 << mag_bits) - 1` the largest. A zero input lands on code 0
///   (positive-side smallest) whose reconstruction is zero — so silence
///   stays silent at every rate.
fn quantise(el: i32, det: i32, mag_bits: u8) -> u8 {
    if el == 0 {
        return 0;
    }
    let sign = el < 0;
    let mag = el.unsigned_abs() as i32;
    // Step size proportional to det, scaled up inversely with the number of
    // magnitude bits so the same input range spans the code space at every
    // rate (fewer bits ⇒ coarser steps).
    let base_step = (det * 2).max(1);
    let scale = 1i32 << (5 - mag_bits as i32);
    let step = (base_step * scale).max(1);
    let max_mag = (1i32 << mag_bits) - 1;
    let idx = ((mag + step / 2) / step).clamp(0, max_mag) as u8;
    let sign_b = if sign { sign_bit(mag_bits) } else { 0 };
    sign_b | (idx & mag_mask(mag_bits))
}

/// Inverse quantiser — the exact inverse of [`quantise`] given the same
/// `det` / `mag_bits`.
fn dequantise(code: u8, det: i32, mag_bits: u8) -> i32 {
    let mag_idx = (code & mag_mask(mag_bits)) as i32;
    let sign = (code & sign_bit(mag_bits)) != 0;
    if mag_idx == 0 && !sign {
        return 0;
    }
    let base_step = (det * 2).max(1);
    let scale = 1i32 << (5 - mag_bits as i32);
    let step = (base_step * scale).max(1);
    let mag = mag_idx * step;
    if sign {
        -mag
    } else {
        mag
    }
}

/// Approximate magnitude index used by the scale-factor adapter. Zero for
/// silent codes, larger for larger-magnitude codes.
fn code_magnitude(code: u8, mag_bits: u8) -> i32 {
    (code & mag_mask(mag_bits)) as i32
}

/// Apply a log-domain update to `det`, keeping it within a sane range.
fn adapt_det(det: i32, w: i32) -> i32 {
    // Work in "log2 * 1024" space implicitly: we approximate by scaling det
    // by exp(w / 4096) on each sample. For small w this is det * (1 + w/4096).
    let scaled = (det as i64 * (4096 + w as i64)) / 4096;
    let v = scaled as i32;
    v.clamp(8, 32_767)
}

fn sat15(v: i32) -> i32 {
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
                assert_eq!(code, 0, "silence must encode to code 0 at {bits}-bit");
                let r = dec.decode(code);
                assert_eq!(r, 0, "silence must decode to 0 at {bits}-bit");
            }
        }
    }

    #[test]
    fn encoder_and_decoder_stay_in_sync() {
        for bits in [4u8, 5, 6] {
            let mut enc = LowBand::with_bits(bits);
            let mut dec = LowBand::with_bits(bits);
            for n in 0..800 {
                let x = ((n as f32 * 0.12).sin() * 4_000.0) as i32;
                let code = enc.encode(x);
                let _ = dec.decode(code);
                // After each step, predictor s should be identical.
                assert_eq!(
                    enc.s, dec.s,
                    "predictor state diverged at n={n} bits={bits}"
                );
                assert_eq!(enc.det, dec.det);
            }
        }
    }

    #[test]
    fn quantise_dequantise_roundtrip() {
        for bits in [4u8, 5, 6] {
            let mag_bits = bits - 1;
            for &det in &[32, 128, 512, 2048] {
                let base_step = det * 2;
                let scale = 1i32 << (5 - mag_bits as i32);
                let step = base_step * scale;
                let max = step * ((1 << mag_bits) - 1);
                for el in (-max..=max).step_by((step as usize).max(1)) {
                    let code = quantise(el, det, mag_bits);
                    let rec = dequantise(code, det, mag_bits);
                    assert!(
                        (rec - el).abs() <= step,
                        "quantise roundtrip too lossy bits={bits} el={el} rec={rec} det={det}"
                    );
                }
            }
        }
    }

    #[test]
    fn code_width_matches_mode() {
        assert_eq!(LowBand::for_mode(Mode::Mode1).lb_bits(), 6);
        assert_eq!(LowBand::for_mode(Mode::Mode2).lb_bits(), 5);
        assert_eq!(LowBand::for_mode(Mode::Mode3).lb_bits(), 4);
    }
}

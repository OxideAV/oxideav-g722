//! ITU-T G.722 SB-ADPCM decoder.
//!
//! Implements the receive path of clauses 4 and 5 of the staged
//! ITU-T G.722 (11/88) Recommendation, using the bit-exact integer
//! computational details of clause 6.2.
//!
//! Layout:
//!   * `LowerDecoderState` — lower sub-band ADPCM decoder
//!     (BLOCKs 2L/3L/4L/5L/6L; Figures 21–25/G.722).
//!   * `HigherDecoderState` — higher sub-band ADPCM decoder
//!     (BLOCKs 2H/3H/4H/5H; Figures 28–31/G.722).
//!   * `Decoder` — pairs the two sub-bands with the 24-tap receive
//!     QMF (Figure 18/G.722 / clause 5.2.2).

use crate::predictor::{add, mul, sub, SubBandState};
use crate::tables::{
    IH2_FROM_IH, IL4_FROM_IL4, IL5_FROM_IL5, IL6_FROM_IL6, QMF_TAPS, QQ2, QQ4, QQ5, QQ6,
    SIH_FROM_IH, SIL_FROM_IL4, SIL_FROM_IL5, SIL_FROM_IL6, WH, WL,
};

/// Bit-rate mode of the received G.722 stream.
///
/// Mirrors Table 1 of the recommendation (page 3): the audio-coding
/// bit rate at the decoder input is always 64 kbit/s but a varying
/// number of LSBs of the lower-sub-band code-word are usable depending
/// on the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// Mode 1 — 64 kbit/s audio (6-bit lower sub-band, 2-bit upper).
    Mode1,
    /// Mode 2 — 56 kbit/s audio (5-bit lower sub-band, 2-bit upper).
    /// The LSB of the lower-sub-band code-word is data, not audio.
    Mode2,
    /// Mode 3 — 48 kbit/s audio (4-bit lower sub-band, 2-bit upper).
    /// The two LSBs of the lower-sub-band code-word are data.
    Mode3,
}

impl Mode {
    /// Number of LSBs of the lower-sub-band code-word that must be
    /// discarded (Table 2, page 7).
    pub const fn lsbs_to_discard(self) -> u8 {
        match self {
            Self::Mode1 => 0,
            Self::Mode2 => 1,
            Self::Mode3 => 2,
        }
    }
}

/// Lower sub-band ADPCM decoder (clause 4 receive path, clauses
/// 3.4 / 3.6 for predictor + inverse quantizer).
#[derive(Debug, Clone)]
pub(crate) struct LowerDecoderState {
    s: SubBandState,
}

impl LowerDecoderState {
    pub(crate) fn new() -> Self {
        Self {
            s: SubBandState::new_lower(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.s.reset();
    }

    /// Snapshot the predictor + scale-factor state (clauses 3.4 / 3.5 /
    /// 3.6) for the transmit↔receive lockstep invariant.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> crate::predictor::PredictorSnapshot {
        self.s.snapshot()
    }

    /// INVQAL — clause 6.2.1.2 (page 37). Compute DLT from the
    /// truncated 4-bit IL using Q4 / scale factor DETL.
    fn invqal(il: u32, detl: i32) -> i32 {
        let ril = il >> 2; // delete 2 LSBs (page 37)
        let il4 = IL4_FROM_IL4[(ril & 0xF) as usize] as usize;
        let sil = SIL_FROM_IL4[(ril & 0xF) as usize];
        let wd1 = QQ4[il4] << 3;
        let wd2 = if sil == 0 { wd1 } else { -wd1 };
        mul(detl, wd2)
    }

    /// INVQBL — clause 6.2.1.5 / Mode 1/2/3 inverse quantizer for the
    /// decoder output. Returns DL.
    fn invqbl(ilr: u32, detl: i32, mode: Mode) -> i32 {
        let (sil, wd1) = match mode {
            Mode::Mode1 => {
                let ril = ilr & 0x3F;
                let il6 = IL6_FROM_IL6[ril as usize] as usize;
                let sil = SIL_FROM_IL6[ril as usize];
                (sil, QQ6[il6] << 3)
            }
            Mode::Mode2 => {
                let ril = (ilr >> 1) & 0x1F;
                let il5 = IL5_FROM_IL5[ril as usize] as usize;
                let sil = SIL_FROM_IL5[ril as usize];
                (sil, QQ5[il5] << 3)
            }
            Mode::Mode3 => {
                let ril = (ilr >> 2) & 0xF;
                let il4 = IL4_FROM_IL4[ril as usize] as usize;
                let sil = SIL_FROM_IL4[ril as usize];
                (sil, QQ4[il4] << 3)
            }
        };
        let wd2 = if sil == 0 { wd1 } else { -wd1 };
        mul(detl, wd2)
    }

    /// LIMIT — clause 6.2.1.6 / Block 6L (page 44).
    fn limit_output(yl: i32) -> i32 {
        yl.clamp(-16384, 16383)
    }

    /// Run one 8-kHz sample of the lower sub-band decoder, returning
    /// the saturated reconstructed signal `RL`.
    pub(crate) fn step(&mut self, ilr: u32, mode: Mode) -> i32 {
        // (1) Predictor estimate from previous state.
        let (sl, szl) = self.s.predict();

        // (2) INVQAL on truncated 4-bit code-word -> DLT (predictor
        //     update path).
        let dlt = Self::invqal(ilr, self.s.detl);

        // (3) INVQBL on the mode-appropriate code-word -> DL (decode
        //     output path).
        let dl = Self::invqbl(ilr, self.s.detl, mode);

        // (4) RECONS: YL = SL + DL  -> LIMIT  -> RL.
        let yl = add(sl, dl);
        let rl = Self::limit_output(yl);

        // (5) RECONS on the predictor side: RLT = SL + DLT.
        let rlt = add(sl, dlt);

        // (6) Adaptation (PARREC -> UPPOL2 -> UPPOL1 -> UPZERO ->
        //     LOGSCL -> SCALEL Method 2).
        self.s.update_partial_reconstructed(dlt, szl);
        let new_apl2 = self.s.update_pole_coeff_2();
        let new_apl1 = self.s.update_pole_coeff_1();
        let new_bl = self.s.update_zero_coeffs(dlt);

        // LOGSCL uses the 4-bit truncated code-word's WL(il4).
        let il4 = IL4_FROM_IL4[((ilr >> 2) & 0xF) as usize] as usize;
        let nbpl = self.s.update_log_scale(WL[il4]);
        let depl = SubBandState::linear_scale_method2(nbpl, 8);

        // (7) Shift delay lines & latch new coefficients (DELAYA blocks).
        self.s.shift_dlt(dlt);
        self.s.rlt[2] = self.s.rlt[1];
        self.s.rlt[1] = self.s.rlt[0];
        self.s.rlt[0] = rlt;
        self.s.al2 = new_apl2;
        self.s.al1 = new_apl1;
        self.s.bl = new_bl;
        self.s.nbl = nbpl;
        self.s.detl = depl;

        rl
    }
}

/// Higher sub-band ADPCM decoder (clause 6.2.2 / Blocks 2H/3H/4H/5H).
#[derive(Debug, Clone)]
pub(crate) struct HigherDecoderState {
    s: SubBandState,
}

impl HigherDecoderState {
    pub(crate) fn new() -> Self {
        Self {
            s: SubBandState::new_higher(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.s.reset();
    }

    /// Snapshot the predictor + scale-factor state (clauses 3.4 / 3.5 /
    /// 3.6) for the transmit↔receive lockstep invariant.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> crate::predictor::PredictorSnapshot {
        self.s.snapshot()
    }

    /// INVQAH — higher sub-band inverse quantizer (page 46).
    fn invqah(ih: u32, deth: i32) -> i32 {
        let ih = (ih & 0x3) as usize;
        let ih2 = IH2_FROM_IH[ih] as usize;
        let sih = SIH_FROM_IH[ih];
        let wd1 = QQ2[ih2] << 3;
        let wd2 = if sih == 0 { wd1 } else { -wd1 };
        mul(deth, wd2)
    }

    /// Run one 8-kHz sample of the higher sub-band decoder.
    pub(crate) fn step(&mut self, ih: u32) -> i32 {
        // (1) Predictor estimate.
        let (sh, szh) = self.s.predict();
        // (2) INVQAH -> DH.
        let dh = Self::invqah(ih, self.s.detl);
        // (3) RECONS: YH = SH + DH.
        let yh = add(sh, dh);
        let rh = LowerDecoderState::limit_output(yh);
        // RH for the QMF is YH limited; the spec passes YH directly
        // to the QMF (Figure 6) but Block 6 limiting is implicit
        // because RH is also fed into the receive QMF as a 16-bit
        // signal (Table 9 page 21).

        // (4) PARREC and adaptation.
        self.s.update_partial_reconstructed(dh, szh);
        let new_apl2 = self.s.update_pole_coeff_2();
        let new_apl1 = self.s.update_pole_coeff_1();
        let new_bl = self.s.update_zero_coeffs(dh);

        let ih_idx = (ih & 0x3) as usize;
        let ih2 = IH2_FROM_IH[ih_idx] as usize;
        let nbph = self.s.update_log_scale(WH[ih2]);
        // SCALEH uses `>> (10 - WD2)` per spec page 47.
        let deph = SubBandState::linear_scale_method2(nbph, 10);

        // (5) Shift state.
        self.s.shift_dlt(dh);
        self.s.rlt[2] = self.s.rlt[1];
        self.s.rlt[1] = self.s.rlt[0];
        self.s.rlt[0] = add(sh, dh);
        self.s.al2 = new_apl2;
        self.s.al1 = new_apl1;
        self.s.bl = new_bl;
        self.s.nbl = nbph;
        self.s.detl = deph;

        rh
    }
}

/// Receive QMF state — 24-tap delay line for the alternating odd/even
/// sub-sample reconstruction (clause 5.2.2 / Figure 18, page 25).
#[derive(Debug, Clone)]
struct ReceiveQmf {
    // xd[0..12] = even-tap delay line (RECA output history, 12 taps).
    xd: [i32; 12],
    // xs[0..12] = odd-tap delay line (RECB output history).
    xs: [i32; 12],
}

impl ReceiveQmf {
    fn new() -> Self {
        Self {
            xd: [0; 12],
            xs: [0; 12],
        }
    }

    fn reset(&mut self) {
        self.xd = [0; 12];
        self.xs = [0; 12];
    }

    /// Process one paired (rl, rh) sample, returning two 16-kHz output
    /// samples `(xout1, xout2)` per the SELECT block (page 27).
    fn step(&mut self, rl: i32, rh: i32) -> (i32, i32) {
        // RECA + RECB at page 25.
        let xd_new = sub(rl, rh);
        let xs_new = add(rl, rh);
        // Push into the delay lines.
        self.xd.copy_within(0..11, 1);
        self.xs.copy_within(0..11, 1);
        self.xd[0] = xd_new;
        self.xs[0] = xs_new;
        // ACCUMC: WD = (XD*H0)+(XD1*H2)+...+(XD11*H22)  (clause 5.2.2,
        //         sub-block ACCUMC, p. 29).
        // ACCUMD: WD = (XS*H1)+(XS1*H3)+...+(XS11*H23)  (sub-block
        //         ACCUMD, p. 30).
        // The QMF coefficients H0..H23 carry binary representation
        // `S,-2,...,-13` (Table 10/G.722, p. 26) i.e. they are stored as
        // `h * 2^13`; XD/XS are the integer sub-band signals. WD must
        // be kept to `>= 2^-23` precision (ACCUMC Note 2) so we
        // accumulate in i64.
        let mut wd_c: i64 = 0;
        let mut wd_d: i64 = 0;
        for (i, (xd_i, xs_i)) in self.xd.iter().zip(self.xs.iter()).enumerate() {
            wd_c += i64::from(*xd_i) * i64::from(QMF_TAPS[2 * i]);
            wd_d += i64::from(*xs_i) * i64::from(QMF_TAPS[2 * i + 1]);
        }
        // XOUT1/XOUT2 = WD >> (y - 16) with y >= 23 (sub-blocks ACCUMC /
        // ACCUMD). The receive QMF carries an explicit factor of 2
        // *outside* the sum (eqs 4-3 / 4-4, p. 24):
        //   xout(j)   = 2 * sum_i H2i  * xd(i)
        //   xout(j+1) = 2 * sum_i H2i+1 * xs(i)
        // With H stored as `h * 2^13`, the raw integer accumulator WD
        // equals `2^13 * sum h*x`, so
        //   xout = 2 * sum h*x = 2 * WD / 2^13 = WD / 2^12 = WD >> 12.
        // (Equivalently WD >> (y-16) with the spec's free parameter
        // y = 28 >= 23.) XOUT1/XOUT2 then saturate to the 15-bit
        // 2's-complement output range -16384..=16383 (Table 9/G.722,
        // p. 25).
        let xout1 = clamp_qmf(wd_c >> 12);
        let xout2 = clamp_qmf(wd_d >> 12);
        (xout1, xout2)
    }
}

fn clamp_qmf(v: i64) -> i32 {
    if v > 16383 {
        16383
    } else if v < -16384 {
        -16384
    } else {
        v as i32
    }
}

/// Full G.722 SB-ADPCM decoder.
///
/// Consumes 64 kbit/s octets per the multiplexer convention of clause
/// 1.4.4 (page 5: bit order is `IH1 IH2 IL1 IL2 IL3 IL4 IL5 IL6` with
/// IH1 as the MSB transmitted first). Each 8-bit octet drives one
/// 8 kHz sub-band pair and the receive QMF emits two 16 kHz PCM
/// samples per octet.
#[derive(Debug, Clone)]
pub struct Decoder {
    lower: LowerDecoderState,
    higher: HigherDecoderState,
    qmf: ReceiveQmf,
    mode: Mode,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new(Mode::Mode1)
    }
}

impl Decoder {
    /// Construct a decoder operating in the requested bit-rate mode.
    pub fn new(mode: Mode) -> Self {
        Self {
            lower: LowerDecoderState::new(),
            higher: HigherDecoderState::new(),
            qmf: ReceiveQmf::new(),
            mode,
        }
    }

    /// Reset all decoder state to the spec's post-RS condition
    /// (clauses 6.2.1.3 / 6.2.2.3 plus QMF delay-line zeroing).
    pub fn reset(&mut self) {
        self.lower.reset();
        self.higher.reset();
        self.qmf.reset();
    }

    /// Decode one 64-kbit/s octet, producing two 14-bit-uniform PCM
    /// samples at 16 kHz.
    ///
    /// The octet layout is `IH1 IH2 IL1 IL2 IL3 IL4 IL5 IL6` with IH1
    /// at bit 7 (MSB-first transmission, clause 1.4.4 page 5).
    pub fn decode_octet(&mut self, octet: u8) -> (i32, i32) {
        let ih = ((octet >> 6) & 0x3) as u32;
        let ilr = (octet & 0x3F) as u32;
        let rl = self.lower.step(ilr, self.mode);
        let rh = self.higher.step(ih);
        self.qmf.step(rl, rh)
    }

    /// Drive the two sub-band ADPCM decoders directly with already-
    /// separated codewords `(I_LR, I_H)` and return the per-sub-band
    /// reconstructed signals `(R_L, R_H)` *without* running the receive
    /// QMF.
    ///
    /// This is the **receive-QMF-bypass** entry point of Configuration 2
    /// (Appendix II / clause II.2.2 p. 64 of the staged Recommendation
    /// PDF: "By-passing the QMF, the output signals, RL and RH, are
    /// separately obtained from the lower and higher sub-band ADPCM
    /// decoders, respectively").
    ///
    /// `i_lr` is the 6-bit lower-sub-band codeword (range 0..=63);
    /// the decoder's current [`Mode`] decides how many LSBs it actually
    /// consumes (Table 1/G.722 p. 3). `i_h` is the 2-bit
    /// higher-sub-band codeword (range 0..=3). Returns the signed LIMIT
    /// block output of each sub-band as defined by sub-blocks `LIMIT`
    /// in §§ 6.2.1.6 and 6.2.2.5 of the staged Recommendation.
    pub fn decode_subband_pair(&mut self, i_lr: u8, i_h: u8) -> (i32, i32) {
        let rl = self.lower.step((i_lr & 0x3F) as u32, self.mode);
        let rh = self.higher.step((i_h & 0x3) as u32);
        (rl, rh)
    }

    /// Decode a slice of octets, writing 2 samples per octet into
    /// the supplied output buffer.
    ///
    /// # Panics
    /// Panics if `out` is shorter than `2 * input.len()`.
    pub fn decode_into(&mut self, input: &[u8], out: &mut [i32]) {
        assert!(
            out.len() >= input.len() * 2,
            "G.722 output buffer must hold 2 samples per input octet"
        );
        for (i, &octet) in input.iter().enumerate() {
            let (a, b) = self.decode_octet(octet);
            out[2 * i] = a;
            out[2 * i + 1] = b;
        }
    }

    /// Decode a slice of octets, returning a freshly allocated vector
    /// of 16-kHz samples.
    pub fn decode(&mut self, input: &[u8]) -> alloc::vec::Vec<i32> {
        let mut out = alloc::vec![0_i32; input.len() * 2];
        self.decode_into(input, &mut out);
        out
    }

    /// Snapshot the lower- and higher-sub-band predictor + scale-factor
    /// state (clauses 3.4 / 3.5 / 3.6). Used by the transmit↔receive
    /// lockstep conformance test; not part of the public bitstream API.
    #[cfg(test)]
    pub(crate) fn predictor_snapshots(
        &self,
    ) -> (
        crate::predictor::PredictorSnapshot,
        crate::predictor::PredictorSnapshot,
    ) {
        (self.lower.snapshot(), self.higher.snapshot())
    }

    /// Run *only* the receive (synthesis) QMF on one already-
    /// reconstructed sub-band pair `(R_L, R_H)`, returning the two
    /// 16 kHz output samples `(x_out1, x_out2)` of eqs 4-3 / 4-4
    /// (clause 4.4 / 5.2.2) **without** running any ADPCM decoding.
    ///
    /// Used by the joint analysis↔synthesis QMF reconstruction test to
    /// pin the filter bank's near-perfect-reconstruction property in
    /// isolation from the ADPCM loop; not part of the public bitstream
    /// API.
    #[cfg(test)]
    pub(crate) fn synthesis_qmf_step(&mut self, rl: i32, rh: i32) -> (i32, i32) {
        self.qmf.step(rl, rh)
    }

    /// Read-only access to the current bit-rate mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Reconfigure the decoder's mode. The spec permits mode switching
    /// at any octet boundary (page 3, clause 1.3 "From an algorithm
    /// viewpoint, the variant used in the SB-ADPCM decoder can be
    /// changed in any octet during the transmission"). State is
    /// preserved.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}

extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_decoder_starts_silent_with_zero_input() {
        // Spec page 30 (DELAYL minimum value of 32 for DETL) and the
        // adaptation feedback together guarantee that with all-zero
        // input the decoder cannot drift away from zero, but it does
        // ramp up the scale factor via leakage. We just verify the
        // first few samples sit within a tiny envelope around zero.
        let mut dec = Decoder::new(Mode::Mode1);
        let mut out = [0_i32; 32];
        let input = [0_u8; 16];
        dec.decode_into(&input, &mut out);
        for &s in &out {
            assert!(s.abs() <= 1024, "early sample {s} out of expected band");
        }
    }

    #[test]
    fn mode_switch_round_trips() {
        let mut dec = Decoder::new(Mode::Mode1);
        assert_eq!(dec.mode(), Mode::Mode1);
        dec.set_mode(Mode::Mode2);
        assert_eq!(dec.mode(), Mode::Mode2);
        dec.set_mode(Mode::Mode3);
        assert_eq!(dec.mode(), Mode::Mode3);
    }

    #[test]
    fn reset_clears_state() {
        let mut dec = Decoder::new(Mode::Mode1);
        // Push some random-ish octets to give the predictor non-zero
        // state.
        let xs: [u8; 32] = [
            0x47, 0x12, 0xA3, 0x7F, 0x00, 0x55, 0xAA, 0x33, 0x91, 0x6C, 0x18, 0xE5, 0x4B, 0x77,
            0x21, 0x09, 0xC0, 0xF1, 0x82, 0x3D, 0x5E, 0x6A, 0xBB, 0x14, 0x29, 0x47, 0x88, 0xD1,
            0x16, 0x52, 0x73, 0xFE,
        ];
        let _ = dec.decode(&xs);
        dec.reset();
        // After reset the state must match a fresh decoder for any
        // subsequent input.
        let mut fresh = Decoder::new(Mode::Mode1);
        let a = dec.decode(&xs);
        let b = fresh.decode(&xs);
        assert_eq!(a, b, "post-reset decoder did not match a fresh one");
    }

    #[test]
    fn decode_into_produces_two_samples_per_octet() {
        let mut dec = Decoder::new(Mode::Mode1);
        let mut buf = [0_i32; 200];
        let input = [0x55_u8; 100];
        dec.decode_into(&input, &mut buf);
        // All samples must respect the LIMIT block range.
        for &s in &buf {
            assert!((-16384..=16383).contains(&s));
        }
    }

    #[test]
    fn modes_produce_distinct_output_for_random_input() {
        // The three modes use different numbers of LSBs from the
        // lower-sub-band code-word; on non-trivial input they cannot
        // produce identical sample streams.
        let input: alloc::vec::Vec<u8> = (0..256).map(|i| (i ^ 0x5A) as u8).collect();
        let a = Decoder::new(Mode::Mode1).decode(&input);
        let b = Decoder::new(Mode::Mode2).decode(&input);
        let c = Decoder::new(Mode::Mode3).decode(&input);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn decoder_is_deterministic() {
        let input: alloc::vec::Vec<u8> = (0..512).map(|i| (i * 37 + 11) as u8).collect();
        let a = Decoder::new(Mode::Mode1).decode(&input);
        let b = Decoder::new(Mode::Mode1).decode(&input);
        assert_eq!(a, b);
    }

    #[test]
    fn lower_invqal_zero_codeword_returns_zero() {
        // RIL = 0000 yields IL4=0, sign=0, QQ4[0]=0 -> DLT must be 0.
        let dlt = LowerDecoderState::invqal(0b0000_0000, 32);
        assert_eq!(dlt, 0);
    }

    #[test]
    fn receive_qmf_lower_band_dc_has_unity_gain() {
        // Spec-derived conformance check for the receive-QMF
        // normalisation (clause 5.2.2 / eqs 4-3, 4-4, p. 24; Table 9
        // output range, p. 25).
        //
        // The two QMF half-band branches each sum to exactly 0.5: the
        // even-indexed taps H0,H2,...,H22 and the odd-indexed taps
        // H1,H3,...,H23 each total 4096 in the Q13 representation of
        // Table 10/G.722 (= 0.5). Feeding a constant lower-sub-band
        // level R_L = D with R_H = 0, after the 12-tap delay line fills
        // both branches give WD = D * 4096, and the factor-of-2 receive
        // gain yields xout = 2 * 0.5 * D = D on both output sub-samples.
        // A unity DC gain is therefore the exact, spec-mandated result;
        // any other QMF shift count breaks it.
        let even: i64 = (0..12).map(|i| i64::from(QMF_TAPS[2 * i])).sum();
        let odd: i64 = (0..12).map(|i| i64::from(QMF_TAPS[2 * i + 1])).sum();
        assert_eq!(even, 4096, "even QMF taps must sum to 0.5 (Q13)");
        assert_eq!(odd, 4096, "odd QMF taps must sum to 0.5 (Q13)");

        let d = 4096_i32;
        let mut qmf = ReceiveQmf::new();
        // Run long enough to fill the 12-deep delay lines.
        let mut last = (0, 0);
        for _ in 0..16 {
            last = qmf.step(d, 0);
        }
        assert_eq!(
            last,
            (d, d),
            "receive QMF must have unity DC gain on the lower sub-band"
        );

        // The higher sub-band alone (R_L = 0) splits with the QMF
        // band-difference sign: xout(j) = -R_H, xout(j+1) = +R_H.
        let mut qmf_h = ReceiveQmf::new();
        let mut last_h = (0, 0);
        for _ in 0..16 {
            last_h = qmf_h.step(0, d);
        }
        assert_eq!(last_h, (-d, d), "receive QMF higher-band DC split");
    }

    #[test]
    fn higher_invqah_zero_does_not_panic() {
        // IH = 11 (sign +, mag 1) with DETH=8 must produce a small
        // positive predictor update.
        let dh = HigherDecoderState::invqah(0b11, 8);
        assert!(dh >= 0);
    }
}

//! ITU-T G.722 SB-ADPCM encoder.
//!
//! Implements the transmit path of clause 3 of the staged ITU-T G.722
//! (11/88) Recommendation:
//!
//!   * `TransmitQmf` — 24-tap analysis quadrature mirror filter
//!     (clause 3.1, eqs 3-1..3-4 with the symmetric `H_{2i}/H_{2i+1}`
//!     split of Table 4/G.722).
//!   * `LowerEncoderState` — 60-level adaptive log quantizer
//!     (clauses 3.2 / 3.3 / 3.4.1) paired with the shared SB-ADPCM
//!     predictor in [`crate::predictor`] (clauses 3.5 / 3.6).
//!   * `HigherEncoderState` — 4-level adaptive log quantizer
//!     (clauses 3.2 / 3.3 / 3.4.2) paired with the same predictor.
//!   * `Encoder` — pairs the two sub-band loops with the transmit QMF
//!     and emits the multiplexer octet of clause 1.4.4 (page 6:
//!     `I_H1 I_H2 I_L1 I_L2 I_L3 I_L4 I_L5 I_L6` with `I_H1` as the
//!     first transmitted / most-significant bit).

use crate::predictor::{add, mul, sub, SubBandState};
use crate::tables::{
    IHN2_FROM_MH, IHP2_FROM_MH, IL4_FROM_IL4, ILN6_FROM_ML, ILP6_FROM_ML, Q2_LEVEL_1, Q6, QMF_TAPS,
    QQ4, WH, WL,
};

extern crate alloc;

// -----------------------------------------------------------------------
// Transmit QMF (clause 3.1, eqs 3-1..3-4)
// -----------------------------------------------------------------------

/// 24-tap analysis QMF delay line. Splits a 16 kHz input stream into a
/// pair of 8 kHz sub-band signals `(x_L, x_H)` per eqs 3-1..3-4.
///
/// Internal layout follows the spec's ACCUMA / ACCUMB partitioning of
/// the 24 input delay slots: `even` holds `x_in(j), x_in(j-2), ...,
/// x_in(j-22)` (12 even-indexed delays consumed by ACCUMA), and `odd`
/// holds `x_in(j-1), x_in(j-3), ..., x_in(j-23)` (12 odd-indexed delays
/// consumed by ACCUMB).
#[derive(Debug, Clone)]
struct TransmitQmf {
    even: [i32; 12],
    odd: [i32; 12],
}

impl TransmitQmf {
    fn new() -> Self {
        Self {
            even: [0; 12],
            odd: [0; 12],
        }
    }

    fn reset(&mut self) {
        self.even = [0; 12];
        self.odd = [0; 12];
    }

    /// Push the pair `(x_in(j-1), x_in(j))` through the analysis bank
    /// and return `(x_L, x_H)`. The two 16 kHz input samples are taken
    /// in the spec's "first-arrived-first" order so `x_first` is
    /// `x_in(j-1)` (older) and `x_second` is `x_in(j)` (newer).
    ///
    /// Eqs 3-3 / 3-4 give x_A and x_B; eqs 3-1 / 3-2 then form the
    /// sub-band outputs. The right-shift by 13 follows the LOWT / HIGHT
    /// sub-blocks of clause 5.2.1 (p. 28): `XL = (XA + XB) >> (y - 15)`,
    /// `XH = (XA - XB) >> (y - 15)`. The QMF coefficients are scaled by
    /// 2^13 (Table 10 note, p. 26), so the ACCUMA / ACCUMB accumulators
    /// equal `2^13 · Σ h·x`. With the spec's free parameter y = 28
    /// (matching the decoder's receive-QMF `(y - 16) = 12`-bit shift in
    /// clause 5.2.2, ACCUMC / ACCUMD) the analysis normalisation is the
    /// `(y - 15) = 13`-bit right shift below. That is exactly one bit
    /// more than the receive QMF, because eqs 3-1 / 3-2 have no leading
    /// "× 2" factor whereas the receive-side eqs 4-3 / 4-4 do.
    fn step(&mut self, x_first: i32, x_second: i32) -> (i32, i32) {
        // Shift the delay lines by one 8 kHz period (= two 16 kHz
        // samples). The freshest 16 kHz sample x_in(j) lands in
        // even[0]; the previous 16 kHz sample x_in(j-1) lands in
        // odd[0]. Older samples shift toward higher indices.
        self.even.copy_within(0..11, 1);
        self.odd.copy_within(0..11, 1);
        self.even[0] = x_second; // x_in(j)
        self.odd[0] = x_first; // x_in(j-1)

        // x_A = Σ_{i=0..11} H_{2i}   · x_in(j   - 2i)
        // x_B = Σ_{i=0..11} H_{2i+1} · x_in(j-1 - 2i)
        let mut xa: i64 = 0;
        let mut xb: i64 = 0;
        for (i, (ev, od)) in self.even.iter().zip(self.odd.iter()).enumerate() {
            xa += i64::from(*ev) * i64::from(QMF_TAPS[2 * i]);
            xb += i64::from(*od) * i64::from(QMF_TAPS[2 * i + 1]);
        }
        // x_L = x_A + x_B  (eq 3-1), x_H = x_A − x_B  (eq 3-2), then the
        // LOWT / HIGHT normalisation `>> (y - 15) = >> 13` (clause 5.2.1,
        // p. 28) undoes the 2^13 coefficient scale of Table 10. The sum
        // / difference are formed *before* the shift, exactly as the
        // LOWT / HIGHT sub-blocks specify, then saturated to the
        // 14-bit-uniform Table 9 range.
        let xl = clamp_qmf((xa + xb) >> 13);
        let xh = clamp_qmf((xa - xb) >> 13);
        (xl, xh)
    }
}

/// Limit a QMF output to the 14-bit-uniform range described in
/// Table 9/G.722 (page 25: signals are limited to ± 16384/16383 in
/// 2's-complement).
///
/// Takes the full-width accumulator value (matching the decoder-side
/// receive-QMF clamp): saturation must happen *before* any narrowing,
/// otherwise a caller feeding samples beyond the documented 14-bit
/// input domain would see the accumulator wrap through the narrowing
/// cast and come back with the wrong sign instead of pinned at the
/// Table 9 rail.
fn clamp_qmf(v: i64) -> i32 {
    if v > 16383 {
        16383
    } else if v < -16384 {
        -16384
    } else {
        v as i32
    }
}

// -----------------------------------------------------------------------
// Lower sub-band encoder (clauses 3.2, 3.3, 3.4.1)
// -----------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct LowerEncoderState {
    s: SubBandState,
}

impl LowerEncoderState {
    pub(crate) fn new() -> Self {
        Self {
            s: SubBandState::new_lower(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.s.reset();
    }

    /// Snapshot the embedded local-decoder predictor state (clauses
    /// 3.4 / 3.5 / 3.6) for the transmit↔receive lockstep invariant.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> crate::predictor::PredictorSnapshot {
        self.s.snapshot()
    }

    /// 60-level forward quantizer (BLOCK 1L / QUANTL, clause 6.2.1.1
    /// pseudo-code, p. 42).
    ///
    /// The spec walks the decision table top-down, emitting `MIL` =
    /// the m_L row whose `(LDL, LDU)` window straddles `WD =
    /// |e_L|`. Per the table's two notes: (1) if `WD` lands exactly
    /// on `LDU`, the larger `MIL` wins; (2) if `LDL == LDU` for a
    /// given m_L (which happens at low scale factors after the
    /// `>> 15` of the `*` operator collapses small thresholds to 0),
    /// that row is excluded from selection.
    ///
    /// Per the QUANTL decision table (clause 6.2.1.1, p. 42), row
    /// `m_L = k` has upper decision level `LDU(k) = (Q6(k) << 3) *
    /// DETL` for `k = 1..29` (with `LDU(30) = +∞`, the "otherwise"
    /// row) and lower decision level `LDL(k) = LDU(k-1)` (with
    /// `LDL(1) = 0`). `Q6(k)` is the 1-indexed Table 14/G.722 entry,
    /// so the array access is `Q6[k]` (index 0 is the sentinel).
    ///
    /// `IL` is then taken from Table 16/G.722 using `(SIL, MIL)`.
    fn quantize_lower(el: i32, detl: i32) -> u8 {
        // SIL = EL >> 15  (sign bit; -1 for negative, 0 otherwise).
        let sil = el >> 15;
        // WD = EL if SIL == 0 else (32767 - EL) & 32767  (magnitude in
        // the spec's S.-1.-2... representation).
        let wd = if sil == 0 { el } else { (32767 - el) & 32767 };

        // Walk LDU(k) upward and pick the first non-excluded row whose
        // LDU(k) > WD. The LDL == LDU exclusion of Note 2 is handled by
        // `ldu != prev_ldu`, where `prev_ldu` is the previous row's
        // LDU(k-1) = the current row's LDL(k) (so equal adjacent
        // thresholds are skipped). The strict `wd < ldu` implements
        // Note 1 (WD exactly on LDU advances to the larger MIL).
        let mut mil: usize = 30; // "otherwise" row of the decision table.
        let mut prev_ldu = 0_i32; // LDL(1) = (Q6(0) << 3) * DETL = 0.

        // Iterate Q6(1..=29) (index 0 is the sentinel); `k` is the
        // 1-based row m_L whose upper decision level is LDU(k).
        for (k, &q6_k) in Q6.iter().enumerate().take(30).skip(1) {
            let ldu = mul(q6_k << 3, detl);
            if ldu != prev_ldu && wd < ldu {
                mil = k;
                break;
            }
            prev_ldu = ldu;
        }
        // Special-case row m_L = 30: only chosen if WD ≥ all earlier
        // LDUs (the "otherwise" branch); the loop above already
        // leaves `mil = 30` in that case.

        if sil == 0 {
            ILP6_FROM_ML[mil]
        } else {
            ILN6_FROM_ML[mil]
        }
    }

    /// Run one 8 kHz lower-sub-band step:
    ///
    /// 1. Form the predicted value `s_L` from the past state
    ///    (eq 3-23).
    /// 2. `e_L = x_L − s_L` (eq 3-5).
    /// 3. Forward-quantize `e_L` to the 6-bit codeword `I_L`
    ///    (eq 3-9).
    /// 4. Inverse-quantize the truncated 4-bit `I_Lt` to `d_Lt`
    ///    (eq 3-11) — bit-identical to the decoder's INVQAL path.
    /// 5. Reconstruct `r_Lt` and advance the predictor / scale-factor
    ///    state (clauses 3.5 / 3.6) identically to the decoder.
    pub(crate) fn step(&mut self, xl: i32) -> u8 {
        // (1) Predictor estimate.
        let (sl, szl) = self.s.predict();

        // (2) Difference signal e_L.
        let el = sub(xl, sl);

        // (3) Forward-quantize e_L → 6-bit I_L.
        let il = Self::quantize_lower(el, self.s.detl);

        // (4) Inverse-quantize the truncated 4-bit code (used for
        //     adaptation, eq 3-11 with QL4^-1 of Table 7/G.722).
        let ril = (il as u32) >> 2;
        let il4 = IL4_FROM_IL4[(ril & 0xF) as usize] as usize;
        let sil = crate::tables::SIL_FROM_IL4[(ril & 0xF) as usize];
        let wd1 = QQ4[il4] << 3;
        let wd2 = if sil == 0 { wd1 } else { -wd1 };
        let dlt = mul(self.s.detl, wd2);

        // (5) Reconstruct: r_Lt = s_L + d_Lt (eq 3-25 with d = d_Lt).
        let rlt = add(sl, dlt);

        // (6) Adaptation — identical to decoder path.
        self.s.update_partial_reconstructed(dlt, szl);
        let new_apl2 = self.s.update_pole_coeff_2();
        let new_apl1 = self.s.update_pole_coeff_1();
        let new_bl = self.s.update_zero_coeffs(dlt);

        let nbpl = self.s.update_log_scale(WL[il4]);
        let depl = SubBandState::linear_scale_method2(nbpl, 8);

        // (7) Shift delay lines and latch new coefficients.
        self.s.shift_dlt(dlt);
        self.s.rlt[2] = self.s.rlt[1];
        self.s.rlt[1] = self.s.rlt[0];
        self.s.rlt[0] = rlt;
        self.s.al2 = new_apl2;
        self.s.al1 = new_apl1;
        self.s.bl = new_bl;
        self.s.nbl = nbpl;
        self.s.detl = depl;

        il
    }
}

// -----------------------------------------------------------------------
// Higher sub-band encoder (clauses 3.2, 3.3, 3.4.2)
// -----------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct HigherEncoderState {
    s: SubBandState,
}

impl HigherEncoderState {
    pub(crate) fn new() -> Self {
        Self {
            s: SubBandState::new_higher(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.s.reset();
    }

    /// Snapshot the embedded local-decoder predictor state (clauses
    /// 3.4 / 3.5 / 3.6) for the transmit↔receive lockstep invariant.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> crate::predictor::PredictorSnapshot {
        self.s.snapshot()
    }

    /// 4-level forward quantizer (BLOCK 1H / QUANTH, clause 6.2.2.1).
    ///
    /// Mirrors QUANTL's structure for the 2-bit higher-sub-band
    /// quantizer: one non-trivial decision boundary at `(Q2(1) << 3) *
    /// DETH`, with the spec's `*` operator (15-bit right-shift) used
    /// for bit-exact arithmetic. Per the LDL == LDU exclusion
    /// (analogous to QUANTL Note 2), `m_H = 1` is chosen only when
    /// the threshold is non-zero; otherwise we fall through to
    /// `m_H = 2` (the "otherwise" row).
    fn quantize_higher(eh: i32, deth: i32) -> u8 {
        let sih = eh >> 15;
        let wd = if sih == 0 { eh } else { (32767 - eh) & 32767 };
        let ldu = mul(Q2_LEVEL_1 << 3, deth);
        let mih: usize = if ldu != 0 && wd < ldu { 1 } else { 2 };
        if sih == 0 {
            IHP2_FROM_MH[mih]
        } else {
            IHN2_FROM_MH[mih]
        }
    }

    /// Run one 8 kHz higher-sub-band step. Same structure as
    /// `LowerEncoderState::step`; the 2-bit code-word feeds straight
    /// back into adaptation (no truncation, unlike the lower band).
    pub(crate) fn step(&mut self, xh: i32) -> u8 {
        // (1) Predictor estimate.
        let (sh, szh) = self.s.predict();
        // (2) e_H = x_H − s_H.
        let eh = sub(xh, sh);
        // (3) Forward-quantize.
        let ih = Self::quantize_higher(eh, self.s.detl);

        // (4) Inverse-quantize (eq 3-12, Q2^-1 of Table 8/G.722).
        let ih_u = ih as usize;
        let ih2 = crate::tables::IH2_FROM_IH[ih_u] as usize;
        let sih = crate::tables::SIH_FROM_IH[ih_u];
        let wd1 = crate::tables::QQ2[ih2] << 3;
        let wd2 = if sih == 0 { wd1 } else { -wd1 };
        let dh = mul(self.s.detl, wd2);
        let rh = add(sh, dh);

        // (5) Adaptation.
        self.s.update_partial_reconstructed(dh, szh);
        let new_apl2 = self.s.update_pole_coeff_2();
        let new_apl1 = self.s.update_pole_coeff_1();
        let new_bl = self.s.update_zero_coeffs(dh);

        let nbph = self.s.update_log_scale(WH[ih2]);
        let deph = SubBandState::linear_scale_method2(nbph, 10);

        self.s.shift_dlt(dh);
        self.s.rlt[2] = self.s.rlt[1];
        self.s.rlt[1] = self.s.rlt[0];
        self.s.rlt[0] = rh;
        self.s.al2 = new_apl2;
        self.s.al1 = new_apl1;
        self.s.bl = new_bl;
        self.s.nbl = nbph;
        self.s.detl = deph;

        ih
    }
}

// -----------------------------------------------------------------------
// Top-level encoder
// -----------------------------------------------------------------------

/// G.722 SB-ADPCM encoder.
///
/// Consumes pairs of 16 kHz uniform-PCM samples (14-bit signed) and
/// emits 64 kbit/s octets in the multiplexer order of clause 1.4.4
/// (page 6: `I_H1 I_H2 I_L1 I_L2 I_L3 I_L4 I_L5 I_L6` with `I_H1`
/// transmitted first as the MSB of the octet).
///
/// The wire rate is always 64 kbit/s regardless of mode: the auxiliary
/// data-channel substitution of clause 1.3 is performed by an external
/// "data insertion device" downstream of the encoder (Figure 1/G.722,
/// page 2), so the encoder itself is mode-agnostic.
#[derive(Debug, Clone)]
pub struct Encoder {
    qmf: TransmitQmf,
    lower: LowerEncoderState,
    higher: HigherEncoderState,
    /// Holds the older 16 kHz sample when an odd-length call to
    /// [`Encoder::encode`] leaves us with a leftover. The QMF needs
    /// an even-length window to emit one octet, so a single trailing
    /// sample is buffered until the next call provides its partner.
    pending: Option<i32>,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    /// Construct a fresh encoder with all delay lines zeroed.
    pub fn new() -> Self {
        Self {
            qmf: TransmitQmf::new(),
            lower: LowerEncoderState::new(),
            higher: HigherEncoderState::new(),
            pending: None,
        }
    }

    /// Reset all encoder state (delay lines, predictor, scale factors)
    /// to the post-reset condition of clauses 3.5 / 3.6.
    pub fn reset(&mut self) {
        self.qmf.reset();
        self.lower.reset();
        self.higher.reset();
        self.pending = None;
    }

    /// Encode a single 8 kHz step from two paired 16 kHz samples
    /// `(x_in(j-1), x_in(j))` and emit one 64 kbit/s octet.
    pub fn encode_pair(&mut self, x_first: i32, x_second: i32) -> u8 {
        let (xl, xh) = self.qmf.step(x_first, x_second);
        let il = self.lower.step(xl);
        let ih = self.higher.step(xh);
        // Multiplexer: bit 7 = IH1 (MSB of IH), bit 6 = IH2,
        // bits 5..0 = IL1..IL6.
        ((ih & 0x3) << 6) | (il & 0x3F)
    }

    /// Drive the two sub-band ADPCM encoders directly with already-split
    /// sub-band samples `(x_L, x_H)` and return the multiplexed octet.
    ///
    /// This is the **transmit-QMF-bypass** entry point of Configuration 1
    /// (Appendix II / clause II.2.1, page 65 of the staged Recommendation
    /// PDF: "the QMFs are by-passed and the test sequences are applied
    /// directly to the ADPCM encoders or decoders"). It is functionally
    /// equivalent to skipping the transmit QMF stage of [`Encoder::encode_pair`].
    ///
    /// `x_L` / `x_H` must already be in the 15-bit-signed range of the
    /// Configuration-1 input signals `XL` / `XH` (Table II-1/G.722 p. 63);
    /// the spec's `XL` and `XH` are described as 15-bit uniformly
    /// quantized signals so the caller is responsible for any
    /// normalisation that the QMF would otherwise have applied.
    ///
    /// The output octet has the same `I_H1 I_H2 I_L1..I_L6` layout as
    /// [`Encoder::encode_pair`] (clause 1.4.4 p. 6).
    pub fn encode_subband_pair(&mut self, x_l: i32, x_h: i32) -> u8 {
        let il = self.lower.step(x_l);
        let ih = self.higher.step(x_h);
        ((ih & 0x3) << 6) | (il & 0x3F)
    }

    /// Encode a 16 kHz PCM slice into a freshly allocated octet
    /// vector.
    ///
    /// If `input.len()` is odd the trailing sample is buffered and
    /// will be paired with the first sample of the next call. The
    /// returned vector therefore has length `(prev_pending + new) / 2`.
    pub fn encode(&mut self, input: &[i32]) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec::Vec::with_capacity(input.len() / 2 + 1);
        self.encode_into(input, &mut out);
        out
    }

    /// Append encoded octets to `out`. See [`Encoder::encode`] for
    /// odd-length handling.
    pub fn encode_into(&mut self, input: &[i32], out: &mut alloc::vec::Vec<u8>) {
        let mut iter = input.iter().copied();
        // Drain any buffered sample first.
        if let Some(prev) = self.pending.take() {
            if let Some(s) = iter.next() {
                out.push(self.encode_pair(prev, s));
            } else {
                self.pending = Some(prev);
                return;
            }
        }
        while let Some(a) = iter.next() {
            let Some(b) = iter.next() else {
                self.pending = Some(a);
                break;
            };
            out.push(self.encode_pair(a, b));
        }
    }

    /// Number of samples held over for the next call (0 or 1).
    pub fn pending_samples(&self) -> usize {
        usize::from(self.pending.is_some())
    }

    /// Snapshot the embedded local-decoder lower- and higher-sub-band
    /// predictor + scale-factor state (clauses 3.4 / 3.5 / 3.6). Used by
    /// the transmit↔receive lockstep conformance test; not part of the
    /// public bitstream API.
    #[cfg(test)]
    pub(crate) fn predictor_snapshots(
        &self,
    ) -> (
        crate::predictor::PredictorSnapshot,
        crate::predictor::PredictorSnapshot,
    ) {
        (self.lower.snapshot(), self.higher.snapshot())
    }

    /// Run *only* the transmit (analysis) QMF on one paired 16 kHz input
    /// `(x_in(j-1), x_in(j))`, returning the raw `(x_L, x_H)` sub-band
    /// pair of eqs 3-1 / 3-2 (clause 3.1) **without** quantizing them.
    ///
    /// Used by the joint analysis↔synthesis QMF reconstruction test to
    /// pin the filter bank's near-perfect-reconstruction property in
    /// isolation from the ADPCM loop; not part of the public bitstream
    /// API.
    #[cfg(test)]
    pub(crate) fn analysis_qmf_step(&mut self, x_first: i32, x_second: i32) -> (i32, i32) {
        self.qmf.step(x_first, x_second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Decoder;
    use crate::Mode;

    #[test]
    fn transmit_qmf_dc_splits_with_unity_lower_band_gain() {
        // Spec-derived conformance check for the transmit (analysis) QMF
        // normalisation (clause 5.2.1, LOWT / HIGHT sub-blocks, p. 28:
        // `XL = (XA + XB) >> (y - 15)`, `XH = (XA - XB) >> (y - 15)`;
        // Table 10 note p. 26: coefficients scaled by 2^13).
        //
        // The even-indexed taps H0,H2,...,H22 and the odd-indexed taps
        // H1,H3,...,H23 each sum to 4096 in Q13 (= 0.5). For a constant
        // 16 kHz input x_in = D, once the 12-deep delay lines fill, the
        // ACCUMA / ACCUMB accumulators are XA = XB = 4096·D, so
        //   XL = (XA + XB) >> 13 = (8192·D) >> 13 = D   (DC → lower band)
        //   XH = (XA - XB) >> 13 = 0                    (no higher-band DC)
        // A unity lower-sub-band DC gain is the exact, spec-mandated
        // result; the previous `>> 11` produced 4·D, saturating XL at the
        // LOWT clamp. The shift must be `>> (y - 15) = >> 13`, exactly one
        // bit more than the receive QMF's `>> (y - 16) = >> 12`.
        let even: i64 = (0..12).map(|i| i64::from(QMF_TAPS[2 * i])).sum();
        let odd: i64 = (0..12).map(|i| i64::from(QMF_TAPS[2 * i + 1])).sum();
        assert_eq!(even, 4096, "even QMF taps must sum to 0.5 (Q13)");
        assert_eq!(odd, 4096, "odd QMF taps must sum to 0.5 (Q13)");

        let d = 4096_i32;
        let mut qmf = TransmitQmf::new();
        // Run long enough to fill the 12-deep delay lines.
        let mut last = (0, 0);
        for _ in 0..16 {
            last = qmf.step(d, d);
        }
        assert_eq!(
            last,
            (d, 0),
            "transmit QMF must split DC into the lower band with unity gain"
        );
    }

    #[test]
    fn encoder_emits_one_octet_per_two_samples() {
        let mut enc = Encoder::new();
        let octets = enc.encode(&[0_i32; 16]);
        assert_eq!(octets.len(), 8);
        assert_eq!(enc.pending_samples(), 0);
    }

    #[test]
    fn encoder_handles_odd_length_input() {
        let mut enc = Encoder::new();
        // 5 samples → 2 octets + 1 buffered sample.
        let octets = enc.encode(&[1_i32, 2, 3, 4, 5]);
        assert_eq!(octets.len(), 2);
        assert_eq!(enc.pending_samples(), 1);
        // Feeding one more sample drains the buffer and emits an
        // additional octet.
        let octets2 = enc.encode(&[6_i32]);
        assert_eq!(octets2.len(), 1);
        assert_eq!(enc.pending_samples(), 0);
    }

    #[test]
    fn encoder_zero_input_never_emits_a_reserved_codeword() {
        // The four codewords 0b000000..0b000011 are reserved for the
        // receiver's transmission-error replacement (Table 5 note,
        // page 18) and MUST NOT be produced by the encoder.
        let mut enc = Encoder::new();
        let octets = enc.encode(&[0_i32; 4096]);
        for &o in &octets {
            let il = o & 0x3F;
            assert!(il >= 0b000100, "reserved I_L codeword 0x{:02x} emitted", il);
        }
    }

    #[test]
    fn encoder_is_deterministic() {
        let input: alloc::vec::Vec<i32> = (0..512_i32).map(|i| (i * 47 + 13) % 8192).collect();
        let a = Encoder::new().encode(&input);
        let b = Encoder::new().encode(&input);
        assert_eq!(a, b);
    }

    #[test]
    fn lower_forward_quantizer_zero_difference_at_reset_picks_first_unambiguous_row() {
        // Per the QUANTL Note 2 of clause 6.2.1.1 (p. 42), when LDL ==
        // LDU for a given m_L the row is excluded. At reset
        // (DETL = 32) the spec's `*` operator collapses several
        // small Q6 thresholds to 0; QUANTL therefore must skip those
        // m_L rows and pick the first one whose LDU > 0. The picked
        // code must decode (via Table 18/G.722) back to the same
        // magnitude/sign pair we packed.
        let code = LowerEncoderState::quantize_lower(0, 32);
        let m = crate::tables::IL6_FROM_IL6[code as usize] as usize;
        let s = crate::tables::SIL_FROM_IL6[code as usize];
        // Sign is `+` (SIL = 0) because e_L >= 0.
        assert_eq!(s, 0);
        // The chosen m_L must be a row Table 16 actually assigns.
        assert_eq!(code, crate::tables::ILP6_FROM_ML[m]);
        // At DETL = 32 the QUANTL upper decision level LDU(k) =
        // (Q6(k) << 3) * DETL evaluates (with the spec `*` operator's
        // `>> 15` truncation) to 0 for k = 1..3 and to 1 for k = 4:
        //   k=1: (35  << 3) * 32 >> 15 = 8960  >> 15 = 0
        //   k=2: (72  << 3) * 32 >> 15 = 18432 >> 15 = 0
        //   k=3: (110 << 3) * 32 >> 15 = 28160 >> 15 = 0
        //   k=4: (150 << 3) * 32 >> 15 = 38400 >> 15 = 1
        // Rows m_L = 1..3 collapse to LDL == LDU == 0 and are excluded
        // by Note 2; row m_L = 4 is the first with LDL(4)=0 != LDU(4)=1,
        // so WD = 0 (0 <= 0 < 1) selects m_L = 4.
        assert_eq!(m, 4);
    }

    #[test]
    fn lower_forward_quantizer_emits_mil_1_when_ldu_1_does_not_collapse() {
        // Regression guard for the QUANTL decision-level indexing
        // (clause 6.2.1.1, p. 42): row m_L = k uses the *upper* level
        // LDU(k) = (Q6(k) << 3) * DETL, i.e. the 1-indexed Table 14
        // entry Q6(k). With a large enough DETL the first level no
        // longer collapses to 0 under the spec `*` operator's >> 15
        // truncation, so a zero-magnitude difference must select the
        // smallest row m_L = 1 — which an off-by-one (Q6(k-1)) would
        // wrongly report as m_L = 2.
        //
        //   DETL = 128: LDU(1) = (35 << 3) * 128 >> 15
        //                      = 280 * 128 >> 15 = 35840 >> 15 = 1
        // WD = 0 satisfies 0 <= 0 < 1, so MIL = 1.
        let code = LowerEncoderState::quantize_lower(0, 128);
        let m = crate::tables::IL6_FROM_IL6[code as usize] as usize;
        assert_eq!(crate::tables::SIL_FROM_IL6[code as usize], 0);
        assert_eq!(m, 1, "QUANTL must reach m_L = 1 once LDU(1) > 0");
        assert_eq!(code, crate::tables::ILP6_FROM_ML[1]);
    }

    #[test]
    fn lower_forward_quantizer_boundary_is_strict_below_ldu() {
        // Note 1 of the QUANTL table (p. 42): "If WD falls exactly on a
        // higher decision level, LDU, the larger adjacent MIL is used."
        // At DETL = 128, LDU(1) = 1 (see the test above). A magnitude
        // whose WD lands exactly on LDU(1) = 1 must therefore advance to
        // m_L = 2 rather than staying on m_L = 1. WD = 1 is produced by
        // e_L = 1 (SIL = 0, WD = EL = 1).
        let on_boundary = LowerEncoderState::quantize_lower(1, 128);
        let m_on = crate::tables::IL6_FROM_IL6[on_boundary as usize] as usize;
        assert_eq!(m_on, 2, "WD exactly on LDU(1) must advance to m_L = 2");
        // Just below the boundary (WD = 0) stays on m_L = 1.
        let below = LowerEncoderState::quantize_lower(0, 128);
        assert_eq!(crate::tables::IL6_FROM_IL6[below as usize] as usize, 1);
    }

    #[test]
    fn higher_forward_quantizer_zero_difference_at_reset_picks_mh_2() {
        // QUANTH at DETH = 8: (Q2_LEVEL_1 << 3) * DETH = (564*8) * 8 / 32768
        // = 36096/32768 = 1; for WD = 0 the LDU=1 row m_H = 1 wins …
        // but for DETH = 8 the integer threshold rounds to 1, not 0,
        // so m_H = 1 IS unambiguous and chosen.
        let code = HigherEncoderState::quantize_higher(0, 8);
        let m = crate::tables::IH2_FROM_IH[code as usize] as usize;
        assert_eq!(crate::tables::SIH_FROM_IH[code as usize], 0);
        assert_eq!(m, 1);
        assert_eq!(code, crate::tables::IHP2_FROM_MH[1]);
    }

    #[test]
    fn higher_forward_quantizer_sign_branching() {
        // Just above the decision boundary, sign decides ILP vs ILN.
        let pos = HigherEncoderState::quantize_higher(10_000, 8);
        let neg = HigherEncoderState::quantize_higher(-10_000, 8);
        // Positive ⇒ top bit of 2-bit code set; negative ⇒ clear.
        assert_eq!(pos & 0b10, 0b10);
        assert_eq!(neg & 0b10, 0b00);
    }

    #[test]
    fn reset_clears_pending_buffer_and_state() {
        let mut enc = Encoder::new();
        let _ = enc.encode(&[1_i32, 2, 3]); // leaves 1 pending
        assert_eq!(enc.pending_samples(), 1);
        enc.reset();
        assert_eq!(enc.pending_samples(), 0);

        let mut fresh = Encoder::new();
        let input: alloc::vec::Vec<i32> = (0..256_i32).map(|i| i * 3).collect();
        let a = enc.encode(&input);
        let b = fresh.encode(&input);
        assert_eq!(a, b, "post-reset encoder must match a fresh one");
    }

    #[test]
    fn multiplexer_bit_layout_matches_decoder() {
        // The encoder emits IH1 in bit 7 and IL6 in bit 0 (clause
        // 1.4.4). Verify by feeding a non-trivial input and
        // confirming the decoder's bit-field extraction matches the
        // encoder's packing.
        let mut enc = Encoder::new();
        let octets = enc.encode(&[1234, -1234, 4567, -4567, 8192, -8192, 100, -100]);
        for &o in &octets {
            let ih_dec = (o >> 6) & 0x3;
            let il_dec = o & 0x3F;
            assert_eq!(
                ((ih_dec & 0x3) << 6) | (il_dec & 0x3F),
                o,
                "octet 0x{o:02x} did not round-trip through the multiplexer split"
            );
        }
    }

    #[test]
    fn encode_then_decode_sane_envelope_for_silence() {
        // An all-zero input encoded then decoded must yield an
        // all-zero (within a small adaptation envelope) output.
        let mut enc = Encoder::new();
        let octets = enc.encode(&[0_i32; 128]);
        let mut dec = Decoder::new(Mode::Mode1);
        let pcm = dec.decode(&octets);
        // After the initial transient (≈ 4 QMF-delay periods), the
        // output must sit close to zero. We allow a small envelope
        // because the encoder produces small non-zero codewords at
        // boot due to the scale factor's leakage path.
        for &s in &pcm[16..] {
            assert!(s.abs() <= 1024, "silence sample {s} outside envelope");
        }
    }

    #[test]
    fn forward_quantizer_climbs_with_magnitude() {
        // m_L must be monotonically non-decreasing in |e_L|.
        let detl = 32;
        let mut prev: usize = 0;
        for mag in 0..2000 {
            let code = LowerEncoderState::quantize_lower(mag * 8, detl);
            // Recover m_L from the codeword via the inverse table.
            let ml = crate::tables::IL6_FROM_IL6[code as usize] as usize;
            assert!(
                ml >= prev,
                "m_L decreased from {prev} to {ml} at |e_L|={}",
                mag * 8
            );
            prev = ml;
        }
    }

    #[test]
    fn encode_then_decode_mode2_envelope_for_silence() {
        // With the round-207 Table-19 fix the Mode-2 inverse quantiser
        // matches the spec at RIL = 11111, so silence must still decode
        // to an envelope around zero (the encoder is unchanged — it
        // always emits Mode-1 octets — but the receiver discards the
        // lower-band LSB).
        let mut enc = Encoder::new();
        let octets = enc.encode(&[0_i32; 128]);
        let mut dec = Decoder::new(Mode::Mode2);
        let pcm = dec.decode(&octets);
        for &s in &pcm[16..] {
            assert!(
                s.abs() <= 1024,
                "Mode-2 silence sample {s} outside envelope"
            );
        }
    }

    #[test]
    fn encode_then_decode_mode3_envelope_for_silence() {
        // Same idea for Mode 3 (two LSBs of the lower band are
        // discarded).
        let mut enc = Encoder::new();
        let octets = enc.encode(&[0_i32; 128]);
        let mut dec = Decoder::new(Mode::Mode3);
        let pcm = dec.decode(&octets);
        for &s in &pcm[16..] {
            assert!(
                s.abs() <= 1024,
                "Mode-3 silence sample {s} outside envelope"
            );
        }
    }

    #[test]
    fn mode2_round_trip_signal_envelope() {
        // For a non-trivial tonal-ish input, the Mode-2 reconstruction
        // must stay inside a generous envelope. This guards against
        // regressions in the IL5 inverse quantiser path that the
        // round-207 transcription fix corrected: a wrong sign on
        // RIL = 11111 would drive the predictor into an entirely
        // wrong polarity and blow past the LIMIT block's ±16384 cap.
        let pcm: alloc::vec::Vec<i32> = (0..512_i32).map(|i| 2000 * ((i % 16) - 8)).collect();
        let mut enc = Encoder::new();
        let octets = enc.encode(&pcm);
        let mut dec = Decoder::new(Mode::Mode2);
        let out = dec.decode(&octets);
        for &s in &out {
            assert!(
                (-16384..=16383).contains(&s),
                "Mode-2 reconstructed sample {s} escaped LIMIT block"
            );
        }
        // Output must also have non-trivial energy (the codec is not
        // degenerately mapping everything to zero).
        let energy: i64 = out.iter().map(|&s| i64::from(s).pow(2)).sum();
        assert!(
            energy > 1_000_000,
            "Mode-2 output energy {energy} too low — predictor likely dead"
        );
    }

    #[test]
    fn encoder_local_decoder_tracks_standalone_decoder_in_lockstep() {
        // Structural conformance invariant of SB-ADPCM (Figures 4/6/7
        // of the staged G.722 Recommendation): the transmit path embeds
        // a *local decoder* whose adaptive predictor + scale-factor loop
        // (clauses 3.4 / 3.5 / 3.6) is the SAME loop the standalone
        // receive decoder runs, driven by the IDENTICAL truncated
        // code-word. In Mode 1 the decoder's predictor-update path uses
        // INVQAL on the 4-bit-truncated I_L (eq 3-11) — bit-for-bit what
        // the encoder feeds its own embedded loop — and the higher band
        // feeds its 2-bit I_H back with no truncation at all. So after
        // processing the same (I_L, I_H) stream, the encoder's internal
        // predictor state and the decoder's predictor state MUST be
        // bit-identical at every step. This catches any divergence in
        // the shared `predictor` module (PARREC / UPPOL1 / UPPOL2 /
        // UPZERO / LOGSCL / SCALEL) that the loose silence/energy
        // envelope tests cannot see.
        //
        // The QMF-bypass entry points (Appendix II Configuration 1 / 2)
        // let the test drive the two ADPCM loops directly with chosen
        // sub-band sample pairs, isolating the predictor identity from
        // the QMF.
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);

        // A deterministic pseudo-random sub-band signal that sweeps a
        // wide magnitude range so the scale factor climbs and falls and
        // the quantizer visits many decision rows. Kept inside the
        // ±16384 sub-band range of the Configuration-1 inputs.
        let mut state: u32 = 0x1234_5678;
        for n in 0..4096 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let xl = ((state >> 17) as i32 & 0x7FFF) - 16384;
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let xh = ((state >> 17) as i32 & 0x7FFF) - 16384;

            // Encoder produces the multiplexed octet; recover I_L / I_H.
            let octet = enc.encode_subband_pair(xl, xh);
            let il = octet & 0x3F;
            let ih = (octet >> 6) & 0x3;

            // Standalone decoder consumes the same code-words.
            let _ = dec.decode_subband_pair(il, ih);

            let (enc_lo, enc_hi) = enc.predictor_snapshots();
            let (dec_lo, dec_hi) = dec.predictor_snapshots();

            assert_eq!(
                enc_lo, dec_lo,
                "lower-sub-band predictor state diverged at step {n}"
            );
            assert_eq!(
                enc_hi, dec_hi,
                "higher-sub-band predictor state diverged at step {n}"
            );
        }
    }

    #[test]
    fn encoder_decoder_lockstep_survives_a_mid_stream_reset() {
        // Reset-behaviour conformance built on the lockstep invariant
        // above. The transmit-side local decoder and the standalone
        // receive decoder are driven in lockstep, reset simultaneously
        // mid-stream (clauses 3.5 / 3.6 post-RS condition), and required
        // to (a) both land on the exact fresh-instance state immediately
        // after reset and (b) re-synchronise and stay bit-identical for
        // the remainder of the run. A reset that left any predictor /
        // scale-factor / delay-line field stale on either side would
        // diverge here even though each side's own `reset()` "matches a
        // fresh instance" unit test passes — this is the cross-side
        // consistency those single-side tests cannot see.
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let fresh_enc = Encoder::new();
        let fresh_dec = Decoder::new(Mode::Mode1);

        let mut state: u32 = 0x0bad_f00d;
        let reset_at = 1500;
        for n in 0..3000 {
            if n == reset_at {
                enc.reset();
                dec.reset();
                // Both sides must match a brand-new instance exactly.
                assert_eq!(
                    enc.predictor_snapshots(),
                    fresh_enc.predictor_snapshots(),
                    "encoder predictor state not fresh after reset"
                );
                assert_eq!(
                    dec.predictor_snapshots(),
                    fresh_dec.predictor_snapshots(),
                    "decoder predictor state not fresh after reset"
                );
            }

            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let xl = ((state >> 17) as i32 & 0x7FFF) - 16384;
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let xh = ((state >> 17) as i32 & 0x7FFF) - 16384;

            let octet = enc.encode_subband_pair(xl, xh);
            let il = octet & 0x3F;
            let ih = (octet >> 6) & 0x3;
            let _ = dec.decode_subband_pair(il, ih);

            assert_eq!(
                enc.predictor_snapshots(),
                dec.predictor_snapshots(),
                "transmit/receive predictor state diverged at step {n} (reset at {reset_at})"
            );
        }
    }
}

//! Shared SB-ADPCM predictor + scale-factor adaptation.
//!
//! Implements the bit-exact integer arithmetic of clauses 3.4 / 3.5 /
//! 3.6 of the staged ITU-T G.722 (11/88) Recommendation. Both the
//! receive path (decoder) and the transmit path (encoder) drive an
//! identical sub-band predictor/quantizer-adaptation loop; this module
//! is the single source of truth for that loop and is consumed by
//! both [`crate::decoder`] and [`crate::encoder`].

use crate::tables::ILB;

/// Saturate a possibly oversized intermediate result back into the
/// 16-bit signed range used by the spec's `+` / `-` operators
/// (clause 5.2 "saturation control" prelude, page 26).
#[inline]
pub(crate) const fn sat16(x: i32) -> i32 {
    if x > 32767 {
        32767
    } else if x < -32768 {
        -32768
    } else {
        x
    }
}

/// Saturating add (spec `+` operator with overflow / underflow clamping).
///
/// The intermediate sum saturates at the i32 rails before [`sat16`]
/// narrows it: bit-identical for every in-range (16-bit) operand pair
/// — where `a + b` cannot overflow i32 — while staying total when a
/// caller drives the QMF-bypass entry points with sub-band samples
/// beyond the documented 15-bit Appendix-II domain (any i32 overflow
/// would land past the 16-bit rails anyway, so saturating first
/// yields the same clamped result).
#[inline]
pub(crate) const fn add(a: i32, b: i32) -> i32 {
    sat16(a.saturating_add(b))
}

/// Saturating subtract (spec `−` operator). Same totality note as
/// [`add`].
#[inline]
pub(crate) const fn sub(a: i32, b: i32) -> i32 {
    sat16(a.saturating_sub(b))
}

/// Multiplication per the spec's `*` operator: `(A * B) >> 15` using
/// full 32-bit intermediate precision (clause 5.2 prelude).
#[inline]
pub(crate) const fn mul(a: i32, b: i32) -> i32 {
    (a * b) >> 15
}

/// Per-sub-band ADPCM predictor + scale-factor state. Reused by both
/// the lower and the higher sub-band — they differ in code-word width,
/// scale-factor limits, and the inverse-quantizer / W-table values, but
/// the predictor structure (2-pole + 6-zero) and the scale-factor
/// update (clauses 3.5 / 3.6) are identical (clauses 3.4 / 4.1).
#[derive(Debug, Clone)]
pub(crate) struct SubBandState {
    // Quantized difference signal delay line. Index 0 is the "current"
    // d_Lt / d_H (DLT in the spec's variable naming); 1..=6 are
    // d_Lt(n-1) .. d_Lt(n-6) (spec eq 3-21 / 3-22).
    pub(crate) dlt: [i32; 7],
    // Partially reconstructed signal delay line (p_Lt / p_H from
    // eq 3-27 / 3-28). Index 0 = current, 1 = (n-1), 2 = (n-2).
    pub(crate) plt: [i32; 3],
    // Reconstructed signal (predictor input) — r_Lt, r_Lt(n-1),
    // r_Lt(n-2) from eq 3-19.
    pub(crate) rlt: [i32; 3],
    // Pole-section predictor coefficients a_L1, a_L2 (eq 3-29 / 3-30).
    pub(crate) al1: i32,
    pub(crate) al2: i32,
    // Zero-section predictor coefficients b_L1..b_L6 (eq 3-37).
    // Index 0 unused, 1..=6 active.
    pub(crate) bl: [i32; 7],
    // Logarithmic quantizer scale factor ∇ (delayed copy); the current
    // update value is returned by [`update_log_scale`].
    pub(crate) nbl: i32,
    // Linear quantizer scale factor Δ (delayed); the current
    // computation is returned by [`linear_scale_method2`].
    pub(crate) detl: i32,
    /// Cap for ∇ after the eq 3-15 / 3-16 limit. Expressed in the
    /// internal fixed-point: 18432 for the lower sub-band (max ∇ = 9)
    /// and 22528 for the higher sub-band (max ∇ = 11).
    pub(crate) nbpl_cap: i32,
    /// Initial Δ after a reset (32 for lower, 8 for higher band — the
    /// "Δ_min" floor of eq 3-17 / 3-18 in the chosen fixed-point scale).
    pub(crate) detl_reset: i32,
}

impl SubBandState {
    /// Lower-sub-band state at the spec's reset condition.
    pub(crate) fn new_lower() -> Self {
        Self {
            dlt: [0; 7],
            plt: [0; 3],
            rlt: [0; 3],
            al1: 0,
            al2: 0,
            bl: [0; 7],
            nbl: 0,
            detl: 32,
            nbpl_cap: 18432,
            detl_reset: 32,
        }
    }

    /// Higher-sub-band state at the spec's reset condition.
    pub(crate) fn new_higher() -> Self {
        Self {
            dlt: [0; 7],
            plt: [0; 3],
            rlt: [0; 3],
            al1: 0,
            al2: 0,
            bl: [0; 7],
            nbl: 0,
            detl: 8,
            nbpl_cap: 22528,
            detl_reset: 8,
        }
    }

    /// Reset every state variable to the post-reset condition described
    /// alongside eqs 3-13 / 3-14 (∇ = 0, Δ = Δ_min, all delay-line and
    /// coefficient memory zeroed).
    pub(crate) fn reset(&mut self) {
        self.dlt = [0; 7];
        self.plt = [0; 3];
        self.rlt = [0; 3];
        self.al1 = 0;
        self.al2 = 0;
        self.bl = [0; 7];
        self.nbl = 0;
        self.detl = self.detl_reset;
    }

    /// Compute the predicted signal `s_L` (eq 3-23) and the
    /// zero-section output `s_Lz` (eq 3-21). The zero-section output
    /// is also returned because it is needed to form the partially
    /// reconstructed signal `p_Lt` (eq 3-27).
    pub(crate) fn predict(&self) -> (i32, i32) {
        // s_Lz = Σ_{i=1..6} b_Li * d_Lt(n-i).  In the spec's
        // fixed-point convention the b_Li * d_Lt product is doubled
        // before the right-shift implicit in `mul`, matching the
        // "<< 1" implicit gain documented at eq 3-21.
        let mut szl: i32 = 0;
        for (dlt_i, bl_i) in self.dlt.iter().zip(self.bl.iter()).skip(1).take(6) {
            let wd = add(*dlt_i, *dlt_i);
            szl = add(szl, mul(*bl_i, wd));
        }
        // s_Lp = a_L1 * r_Lt(n-1) + a_L2 * r_Lt(n-2)  (eq 3-19).
        let wd1 = add(self.rlt[1], self.rlt[1]);
        let wd1 = mul(self.al1, wd1);
        let wd2 = add(self.rlt[2], self.rlt[2]);
        let wd2 = mul(self.al2, wd2);
        let spl = add(wd1, wd2);
        // s_L = s_Lp + s_Lz   (eq 3-23).
        let sl = add(spl, szl);
        (sl, szl)
    }

    /// Update the log scale factor ∇(n) per eq 3-13 / 3-14, including
    /// the limit step of eq 3-15 / 3-16. `w` is the matching W_L / W_H
    /// table entry indexed by the (truncated, lower-band) magnitude.
    pub(crate) fn update_log_scale(&self, w: i32) -> i32 {
        // WD = β · ∇(n-1) with β = 127/128 in fixed-point ≡ × 32512.
        let wd = mul(self.nbl, 32512);
        let mut nbpl = add(wd, w);
        if nbpl < 0 {
            nbpl = 0;
        }
        if nbpl > self.nbpl_cap {
            nbpl = self.nbpl_cap;
        }
        nbpl
    }

    /// Convert ∇ to Δ via the 32-entry ILB log-to-linear table
    /// (eq 3-17 / 3-18 evaluated through the Method-2 lookup of
    /// clause 6.2.1.3 / 6.2.2.3 in the consolidated edition).
    ///
    /// `base_shift` is 8 for the lower sub-band (∇ ranges 0..=9) and
    /// 10 for the higher (∇ ranges 0..=11). The two extra bits of
    /// shift on the higher band compensate for its wider ∇ range so
    /// the same 32-entry table covers both.
    pub(crate) fn linear_scale_method2(nbpl: i32, base_shift: i32) -> i32 {
        let wd1 = (nbpl >> 6) & 31;
        let wd2 = nbpl >> 11;
        let shift = base_shift - wd2;
        let ilb = ILB[wd1 as usize];
        let wd3 = if shift >= 0 {
            ilb >> shift
        } else {
            ilb << (-shift)
        };
        wd3 << 2
    }

    /// Form p_Lt = d_Lt + s_Lz (eq 3-27) and shift the p delay line.
    pub(crate) fn update_partial_reconstructed(&mut self, dlt: i32, szl: i32) {
        let plt = add(dlt, szl);
        self.plt[2] = self.plt[1];
        self.plt[1] = self.plt[0];
        self.plt[0] = plt;
    }

    /// Update a_L1 per eq 3-29.
    pub(crate) fn update_pole_coeff_1(&self) -> i32 {
        let sg0 = self.plt[0] >> 15;
        let sg1 = self.plt[1] >> 15;
        // 3·2^-8 · p_A where p_A = sgn2(p_Lt(n)) · sgn2(p_Lt(n-1)).
        let wd1 = if sg0 == sg1 { 192 } else { -192 };
        // (1 − 2^-8) · a_L1(n-1) ≡ a_L1 * 32640/32768 in fixed-point.
        let wd2 = mul(self.al1, 32640);
        let mut apl1 = add(wd1, wd2);
        // Stability constraint eq 3-36: |a_L1| ≤ 1 − 2^-4 − a_L2.
        let wd3 = sub(15360, self.al2);
        if apl1 > wd3 {
            apl1 = wd3;
        }
        if apl1 < -wd3 {
            apl1 = -wd3;
        }
        apl1
    }

    /// Update a_L2 per eq 3-30 with the f(a_L1) of eq 3-34.
    pub(crate) fn update_pole_coeff_2(&self) -> i32 {
        let sg0 = self.plt[0] >> 15;
        let sg1 = self.plt[1] >> 15;
        let sg2 = self.plt[2] >> 15;
        // Compute f per eq 3-34. The fixed-point gain on the |a_L1| ≤
        // 1/2 branch is 4·a_L1; the |a_L1| > 1/2 branch is 2·sgn(a_L1).
        // In the spec's S.0.-1.-2... representation a magnitude of 1/2
        // corresponds to |a_L1| ≥ 8192 (== 0.25 in the doubled-coef
        // packing, which is 1/2 of the saturating 16384 ceiling that
        // eq 3-36 enforces on a_L1 alone).
        let mut wd1 = add(self.al1, self.al1);
        wd1 = add(wd1, wd1);
        let wd2 = if sg0 == sg1 { sub(0, wd1) } else { wd1 };
        let wd2 = wd2 >> 7; // gain 1/128 per eq 3-30 leading "2^-7 · p_B".
        let wd3 = if sg0 == sg2 { 128 } else { -128 };
        let wd4 = add(wd2, wd3);
        let wd5 = mul(self.al2, 32512);
        let apl2 = add(wd4, wd5);
        // Stability constraint eq 3-35: |a_L2| ≤ 0.75 ≡ 12288.
        apl2.clamp(-12288, 12288)
    }

    /// Update b_L1..b_L6 per eq 3-37.
    pub(crate) fn update_zero_coeffs(&self, dlt: i32) -> [i32; 7] {
        let wd1 = if dlt == 0 { 0 } else { 128 };
        let sg0 = dlt >> 15;
        let mut new_bl = self.bl;
        for (i, slot) in new_bl.iter_mut().enumerate().take(7).skip(1) {
            let sgi = self.dlt[i] >> 15;
            let wd2 = if sg0 == sgi { wd1 } else { -wd1 };
            let wd3 = mul(self.bl[i], 32640); // (1 − 2^-8) leak per eq 3-37.
            *slot = add(wd2, wd3);
        }
        new_bl
    }

    /// Shift the d_Lt delay line so that the incoming d_Lt becomes
    /// d_Lt(n-1) on the next sample.
    pub(crate) fn shift_dlt(&mut self, dlt: i32) {
        self.dlt.copy_within(1..6, 2);
        self.dlt[1] = dlt;
        self.dlt[0] = dlt;
    }

    /// Capture the full adaptive predictor + scale-factor state.
    ///
    /// Used to assert the spec's structural identity between the
    /// transmit-path *local decoder* and the receive-path decoder: per
    /// the SB-ADPCM block diagrams (Figures 4/6/7/G.722) the encoder
    /// embeds a decoder whose predictor / scale-factor adaptation is the
    /// **same** loop the standalone decoder runs (clauses 3.4 / 3.5 /
    /// 3.6), driven by the identical truncated code-word. Two states that
    /// have processed the same code-word stream must therefore be
    /// bit-identical; this snapshot makes that invariant checkable
    /// without exposing the private fields outside the crate.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> PredictorSnapshot {
        PredictorSnapshot {
            dlt: self.dlt,
            plt: self.plt,
            rlt: self.rlt,
            al1: self.al1,
            al2: self.al2,
            bl: self.bl,
            nbl: self.nbl,
            detl: self.detl,
        }
    }
}

/// An immutable copy of a [`SubBandState`]'s adaptive state, used to
/// compare the encoder's embedded local-decoder loop against the
/// standalone decoder's loop (see [`SubBandState::snapshot`]).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PredictorSnapshot {
    pub(crate) dlt: [i32; 7],
    pub(crate) plt: [i32; 3],
    pub(crate) rlt: [i32; 3],
    pub(crate) al1: i32,
    pub(crate) al2: i32,
    pub(crate) bl: [i32; 7],
    pub(crate) nbl: i32,
    pub(crate) detl: i32,
}

//! Appendix IV lower-band analysis — fixed-point LP / LTP machinery.
//!
//! The clause IV.6.1.2.1–IV.6.1.2.3 analysis stages of the Appendix IV
//! packet-loss concealment, rebuilt on the staged fixed-point numeric
//! tables (`src/plc_tables.rs`, from `docs/audio/g722/tables/`) and
//! the ITU-T G.191 STL basic-operator semantics (`src/basicop.rs`,
//! from `docs/audio/g722/basic-operators/`). Clause IV.7 makes the
//! 16-bit fixed-point realisation normative over the prose, so every
//! multiply/add here is an explicit basic-operator call with its
//! saturation and rounding pinned.
//!
//! Where the staged material stops — the exact *instruction sequence*
//! of the reference realisation is not staged, only the operator
//! semantics and the data tables — the constructions below are our
//! own, built exclusively from the staged operators, and are
//! documented as such at each site (the autocorrelation overflow
//! rescaling schedule, the double-precision reflection-coefficient
//! division, the correlation normalisation). The one *algorithmic*
//! gap, the clause IV.6.1.2.3 "procedure favouring the smaller pitch
//! values", is documented as unobtainable
//! (`docs/audio/g722/appendix-IV-ltp-smaller-pitch-gap.md`); the
//! search below therefore runs a **fitted** preference rule —
//! calibrated black-box against the staged Appendix IV vectors, and
//! never to be mistaken for the ITU recipe — documented at
//! [`TDS_SMALLER_PITCH_MARGIN_Q15`].

use crate::basicop::{
    add, div_s, extract_h, l_abs, l_add, l_comp, l_deposit_h, l_extract, l_mac, l_mac0, l_msu,
    l_mult, l_shl, l_shr, l_sub, mpy_32, mpy_32_16, mult, mult_r, norm_l, round_fx, shr,
};
use crate::plc_tables::{FIR_LP, LAG_H, LAG_L, LPC_WIN_80};

extern crate alloc;
use alloc::vec::Vec;

/// LP order of the lower-band analysis/synthesis filters (eq IV-1).
pub(crate) const LP_ORDER: usize = 8;

/// Q15 weighting constant γ = 0.94 of the clause IV.6.1.2.3 filter
/// B(z/γ): `round(0.94 × 32768) = 30802`.
const GAMMA_Q15: i32 = 30_802;

/// **FITTED, not ITU-specified** — Q15 relative margin of the
/// smaller-pitch preference in the eq (IV-7) `Tds` search.
///
/// Clause IV.6.1.2.3 requires "a procedure favouring the smaller pitch
/// values to avoid choosing pitch multiples" but specifies no rule
/// (documented unobtainable —
/// `docs/audio/g722/appendix-IV-ltp-smaller-pitch-gap.md`). Following
/// that note's §9 experiment, the search below iterates lags upward
/// and lets a longer lag displace the incumbent only when its
/// correlation exceeds the incumbent's by this relative margin:
/// `r(i) > r(best) + margin·r(best)` (i.e. `r(i) > α·r(best)` with
/// `α = 1 + margin/32768`). The value was **fitted black-box against
/// the staged Appendix IV vectors** (ground-truth pitch decisions of
/// `tests/appendix_iv_plc.rs`); it is *not* the ITU recipe, whose
/// text lives only in the unstaged reference realisation. Margin 0
/// degenerates to the plain arg max.
///
/// Fit (2026-08-13, methodology + residuals in README §Appendix IV
/// and `tests/appendix_iv_plc.rs`): sweeping the margin over the
/// staged vectors, the reference reproduces 17 of the 18 ground-truth
/// pitch decisions of `test10.bst` for every margin in `[5504, 6272]`
/// (grid step 32) — all four pitch-multiple misses of the plain arg
/// max close — and within that plateau the full-corpus bit-exact
/// scores are maximised on `[5504, 5568]`. The value here is that
/// sub-interval's midpoint, `α ≈ 1.169`. Validated held-out on
/// `test20.bst`'s ground truths (no part of the fit): 7/8 vs the
/// plain arg max's 6/8, closing that set's one pitch-multiple miss
/// and inducing no new one.
pub(crate) const TDS_SMALLER_PITCH_MARGIN_Q15: i32 = 5536;

/// **FITTED negative result** — Q15 relative margin of the same
/// smaller-pitch preference applied to the eq (IV-8) refinement
/// search over `[4·Tds − 2, 4·Tds + 2]`; 0 keeps the plain arg max.
///
/// Fitted independently of [`TDS_SMALLER_PITCH_MARGIN_Q15`] and kept
/// at 0: the one remaining ground-truth miss (frame 570 of
/// `test10.bst`, `T0` 83 vs 82) does not close for any margin up to
/// 128/32768, and by 256/32768 three previously correct one-lag
/// refinements (frames 149 / 251 / 576) break instead — so the
/// residual divergence there is a correlation-arithmetic difference,
/// not a missing smaller-pitch preference.
pub(crate) const REFINE_SMALLER_PITCH_MARGIN_Q15: i32 = 0;

/// Maximum-correlation search with the smaller-pitch preference: walk
/// `values` upward from index 0 and let a later (longer-lag) candidate
/// displace the incumbent only when it clears the incumbent's
/// correlation by the Q15 relative margin (`v > best + margin·best`,
/// saturating 16-bit arithmetic, `mult_r` rounding). Returns
/// `(index_of_best, best_value)`; with `margin_q15 = 0` this is the
/// plain arg max keeping the smaller lag on ties.
fn preferred_max(values: &[i32], margin_q15: i32) -> (usize, i32) {
    debug_assert!(!values.is_empty());
    let mut best = 0;
    for (i, &v) in values.iter().enumerate().skip(1) {
        let threshold = add(values[best], mult_r(values[best], margin_q15));
        if v > threshold {
            best = i;
        }
    }
    (best, values[best])
}

/// Windowed autocorrelation with the clause IV.6.1.2.1 conditioning.
///
/// Applies the Q15 window to `x`, computes `r(0..=order)`, normalises
/// them to full 32-bit scale (the Levinson recursion is
/// scale-invariant), and applies the staged double-precision lag
/// window to `r(1..=order)` — the 40-dB white-noise correction is
/// folded into the staged window values, so `r(0)` is left untouched
/// (staged `tables/README.md`).
///
/// Overflow handling: the STL accumulator saturates on every `L_mac`
/// (semantics doc §2.3), so a fixed-point autocorrelation must
/// pre-scale its windowed input until the energy fits 32 bits. The
/// rescaling schedule (halve until `r(0)` fits) is our own
/// construction; the partial sums of `r(k)` are bounded by `r(0)`
/// (Cauchy-Schwarz), so once `r(0)` fits no `L_mac` step saturates
/// and the i64 evaluation below is bit-identical to the `L_mac`
/// chain.
///
/// Returns `None` when the windowed signal has zero energy.
fn autocorr(x: &[i32], win: &[i32], order: usize) -> Option<Vec<i32>> {
    debug_assert_eq!(x.len(), win.len());
    let n = x.len();
    // Window application: y(i) = mult_r(x(i), w(i)) — the rounding
    // Q15 multiply (§2.2; the truncating `mult` variant scores
    // slightly further from the reference PLC vectors).
    let mut y: Vec<i32> = x
        .iter()
        .zip(win.iter())
        .map(|(&s, &w)| crate::basicop::mult_r(s, w))
        .collect();
    // Rescale until r(0) fits the 32-bit accumulator.
    let mut r0: i64 = y.iter().map(|&v| 2 * i64::from(v) * i64::from(v)).sum();
    while r0 > i64::from(i32::MAX) {
        for v in y.iter_mut() {
            *v = shr(*v, 1);
        }
        r0 = y.iter().map(|&v| 2 * i64::from(v) * i64::from(v)).sum();
    }
    if r0 == 0 {
        return None;
    }
    let r0 = r0 as i32;
    let norm = norm_l(r0);
    let mut r = Vec::with_capacity(order + 1);
    r.push(l_shl(r0, norm));
    for k in 1..=order {
        let mut acc: i32 = 0;
        for i in k..n {
            acc = l_mac(acc, y[i], y[i - k]);
        }
        let rk = l_shl(acc, norm);
        // 60-Hz bandwidth-expansion lag window (+ folded 40-dB
        // white-noise correction), double precision.
        let (hi, lo) = l_extract(rk);
        r.push(mpy_32(hi, lo, LAG_H[k - 1], LAG_L[k - 1]));
    }
    Some(r)
}

/// Q31 fractional division `num / den` for `0 ≤ num < den`, built from
/// `div_s` with one double-precision refinement step (our own
/// construction on the staged operators; the reference's division
/// sequence is not staged). The first `div_s` gives the Q15 head; the
/// remainder is scaled back up and divided again for 15 more bits:
/// `q = q15·2^16 + 2·((rem·2^15)/den)` in the `(hi << 16) + (lo << 1)`
/// encoding.
fn div32_q31(num: i32, den: i32) -> i32 {
    debug_assert!((0..den).contains(&num) && den > 0);
    let sh = norm_l(den);
    let d = l_shl(den, sh);
    let n = l_shl(num, sh);
    let dh = extract_h(d);
    let nh = extract_h(n);
    if dh == 0 {
        return 0;
    }
    let q15 = div_s(nh.min(dh), dh);
    // rem = n − den·q15/2^15 (double precision estimate of the
    // consumed part).
    let (dhi, dlo) = l_extract(d);
    let est = mpy_32_16(dhi, dlo, q15);
    let rem = l_sub(n, est);
    let rem_up = l_shl(l_abs(rem), 15);
    let q2 = {
        let rh = extract_h(rem_up);
        if rh <= 0 {
            0
        } else {
            div_s(rh.min(dh), dh)
        }
    };
    if rem < 0 {
        l_sub(l_deposit_h(q15), l_shl(q2, 1))
    } else {
        l_comp(q15, q2)
    }
}

/// Levinson-Durbin recursion in double precision (clause IV.6.1.2.1;
/// the recursion itself is the one named by that clause, realised in
/// `(hi, lo)` 32-bit arithmetic — our own construction on the staged
/// operator set). Input: conditioned autocorrelations from
/// [`autocorr`]. Output: `a_1..a_order` of eq (IV-1) in Q12.
///
/// Internally the coefficients are held in Q27 (range ±16). The
/// recursion stops early — keeping the coefficients found so far — if
/// the prediction error becomes non-positive or a reflection
/// coefficient leaves the open unit interval (an unstable fit on
/// pathological input).
fn levinson(r: &[i32], order: usize) -> [i32; LP_ORDER] {
    let mut a_q27 = [0i32; LP_ORDER + 1];
    let mut err = r[0];
    for i in 1..=order {
        // acc(Q27) = r(i)/2^4 + Σ_{j=1..i−1} a_j · r(i−j).
        let mut acc = l_shr(r[i], 4);
        for j in 1..i {
            let (ah, al) = l_extract(a_q27[j]);
            let (rh, rl) = l_extract(r[i - j]);
            acc = l_add(acc, mpy_32(ah, al, rh, rl));
        }
        let acc_q31 = l_shl(acc, 4);
        if err <= 0 || l_abs(acc_q31) >= err {
            break;
        }
        // Reflection coefficient k = −acc/err in Q31.
        let mag = div32_q31(l_abs(acc_q31), err);
        let k_q31 = if acc_q31 > 0 { -mag } else { mag };
        let (kh, kl) = l_extract(k_q31);
        // a_j ← a_j + k·a_{i−j};  a_i ← k.
        let mut new_a = a_q27;
        for j in 1..i {
            let (ah, al) = l_extract(a_q27[i - j]);
            new_a[j] = l_add(a_q27[j], mpy_32(kh, kl, ah, al));
        }
        new_a[i] = l_shr(k_q31, 4);
        a_q27 = new_a;
        // err ← err · (1 − k²).
        let kk = mpy_32(kh, kl, kh, kl);
        let om = l_sub(i32::MAX, kk);
        let (eh, el) = l_extract(err);
        let (oh, ol) = l_extract(om);
        err = mpy_32(eh, el, oh, ol);
    }
    let mut a_q12 = [0i32; LP_ORDER];
    for (out, &q27) in a_q12.iter_mut().zip(a_q27.iter().skip(1)) {
        // Q27 → Q12 with the round-half-up primitive (§2.5).
        *out = round_fx(l_shl(q27, 1));
    }
    a_q12
}

/// Eighth-order LP analysis on the last 10 ms of `zl` (clause
/// IV.6.1.2.1): staged eq (IV-2) Q15 window, conditioned
/// autocorrelation, Levinson-Durbin. `hist` is `zl(−80 … −1)` oldest
/// first. Returns `a_1..a_8` in Q12.
pub(crate) fn lp_analysis(hist: &[i32; 80]) -> [i32; LP_ORDER] {
    match autocorr(hist, &LPC_WIN_80, LP_ORDER) {
        Some(r) => levinson(&r, LP_ORDER),
        None => [0; LP_ORDER],
    }
}

/// Eq (IV-5): low-pass filter `zlpre` by the staged Q16 decimation FIR
/// and decimate 4:1 to 2 kHz. Filter memories initialised to 0 (the
/// clause says "each time"); `t(k)` is the filter output at the last
/// sample of each block of four. The Q16 product sum is truncated to
/// Q0 by taking the high word.
pub(crate) fn decimate(zlpre: &[i32; 288]) -> [i32; 72] {
    let mut t = [0i32; 72];
    for (k, slot) in t.iter_mut().enumerate() {
        let m = 4 * k + 3;
        let mut acc: i32 = 0;
        for (j, &h) in FIR_LP.iter().enumerate() {
            let idx = m as isize - j as isize;
            let x = if idx >= 0 { zlpre[idx as usize] } else { 0 };
            acc = l_mac0(acc, h, x);
        }
        *slot = extract_h(acc);
    }
    t
}

/// Clause IV.6.1.2.3 weighting: 2nd-order LP of `t` (windowed by the
/// last 72 entries of the staged eq (IV-2) window, same conditioning
/// as the 8th-order analysis) giving `B(z) = 1 − b1·z⁻¹ − b2·z⁻²`,
/// then `tw = B(z/γ)·t` with γ = 0.94. The filter runs as an FIR with
/// zero initial state (`t(−73), t(−74)` are outside the analysis
/// buffer).
pub(crate) fn weight(t: &[i32; 72]) -> [i32; 72] {
    let a2 = match autocorr(t, &LPC_WIN_80[8..], 2) {
        Some(r) => levinson(&r, 2),
        None => [0; LP_ORDER],
    };
    // B(z) = 1 − b1 z⁻¹ − b2 z⁻² with (b1, b2) = (−a1, −a2), so
    // tw(n) = t(n) + γ·a1·t(n−1) + γ²·a2·t(n−2). Weighted
    // coefficients in Q12: c_i = mult(γ^i, a_i).
    let g2 = mult(GAMMA_Q15, GAMMA_Q15);
    let c1 = mult(GAMMA_Q15, a2[0]);
    let c2 = mult(g2, a2[1]);
    let mut tw = [0i32; 72];
    for (k, slot) in tw.iter_mut().enumerate() {
        let t1 = if k >= 1 { t[k - 1] } else { 0 };
        let t2 = if k >= 2 { t[k - 2] } else { 0 };
        // Q13 accumulator: L_mult(x, 4096) deposits x in Q13.
        let mut acc = l_mult(t[k], 4096);
        acc = l_mac(acc, c1, t1);
        acc = l_mac(acc, c2, t2);
        *slot = round_fx(l_shl(acc, 3));
    }
    tw
}

/// Normalised cross-correlation in Q15 (eqs IV-6 / IV-9):
/// `Σ x·y / max(Σ x², Σ y²)`, sign carried on the numerator. The
/// energies are accumulated exactly and jointly renormalised before a
/// single `div_s` (our own normalisation construction; `|Σ x·y| ≤
/// max(Σ x², Σ y²)` by Cauchy-Schwarz, so the `div_s` precondition
/// holds structurally).
fn norm_corr(xy: i64, xx: i64, yy: i64) -> i32 {
    let den64 = xx.max(yy);
    if den64 <= 0 {
        return 0;
    }
    let mut sh = 0;
    while (den64 >> sh) > i64::from(i32::MAX) {
        sh += 1;
    }
    let den = (den64 >> sh) as i32;
    let num = (xy >> sh) as i32;
    let nsh = norm_l(den);
    let dh = extract_h(l_shl(den, nsh));
    let nh = extract_h(l_shl(num.abs(), nsh));
    if dh <= 0 {
        return 0;
    }
    let q = div_s(nh.min(dh), dh);
    if num < 0 {
        -q
    } else {
        q
    }
}

/// LTP open-loop pitch analysis (clause IV.6.1.2.3, Figure IV.3) on
/// the pre-processed signal. Returns `(T0, Rmax)` with `Rmax` in Q15.
///
/// Steps a)–e) are implemented as printed: `Tds = 18` unless some
/// `r(i) < 0` exists, then the maximum search over `[max(i0, 4), 35]`.
/// The additional "procedure favouring the smaller pitch values" has
/// no obtainable specification (staged gap note
/// `appendix-IV-ltp-smaller-pitch-gap.md`); both the `Tds` search and
/// the refinement therefore run through [`preferred_max`] with the
/// **fitted** margins [`TDS_SMALLER_PITCH_MARGIN_Q15`] /
/// [`REFINE_SMALLER_PITCH_MARGIN_Q15`] (smaller lag kept on ties),
/// and the residual divergence against the reference vectors is
/// characterised in `tests/appendix_iv_plc.rs`.
pub(crate) fn ltp_analysis(zlpre: &[i32; 288]) -> (usize, i32) {
    ltp_analysis_with_margins(
        zlpre,
        TDS_SMALLER_PITCH_MARGIN_Q15,
        REFINE_SMALLER_PITCH_MARGIN_Q15,
    )
}

/// [`ltp_analysis`] with explicit preference margins — the fitting /
/// characterisation surface used by the calibration measurements
/// (`margin = 0` is the plain arg max on both stages).
pub(crate) fn ltp_analysis_with_margins(
    zlpre: &[i32; 288],
    tds_margin_q15: i32,
    refine_margin_q15: i32,
) -> (usize, i32) {
    let t = decimate(zlpre);
    let tw = weight(&t);

    // Eq (IV-6) over the last 35 weighted-decimated samples:
    // j = −35..−1 ↔ tw[37..72].
    let r_at = |i: usize| -> i32 {
        let mut xy = 0i64;
        let mut xx = 0i64;
        let mut yy = 0i64;
        for j in 0..35 {
            let x = i64::from(tw[37 + j]);
            let y = i64::from(tw[37 + j - i]);
            xy += x * y;
            xx += x * x;
            yy += y * y;
        }
        norm_corr(xy, xx, yy)
    };
    let r_vals: Vec<i32> = (0..=35).map(|i| if i == 0 { 0 } else { r_at(i) }).collect();
    let mut tds: usize = 18; // step a) initialisation.
    if let Some(i0) = (1..=35).find(|&i| r_vals[i] < 0) {
        // Steps c)–e), with the fitted smaller-pitch preference.
        let i1 = i0.max(4);
        let (off, _) = preferred_max(&r_vals[i1..=35], tds_margin_q15);
        tds = i1 + off;
    }

    // Refinement (eqs IV-8 / IV-9): T = 4·Tds, window length T,
    // search i = T−2 … T+2 (bounded by the 288-sample buffer).
    let t_mid = 4 * tds;
    let big_r = |i: usize| -> i32 {
        let win = t_mid;
        let mut xy = 0i64;
        let mut xx = 0i64;
        let mut yy = 0i64;
        for jj in 0..win {
            let x = i64::from(zlpre[288 - win + jj]);
            let y = i64::from(zlpre[288 - win + jj - i]);
            xy += x * y;
            xx += x * x;
            yy += y * y;
        }
        norm_corr(xy, xx, yy)
    };
    let lo = t_mid.saturating_sub(2).max(1);
    let hi = (t_mid + 2).min(288 - t_mid);
    let big_r_vals: Vec<i32> = (lo..=hi).map(big_r).collect();
    let (off, best) = preferred_max(&big_r_vals, refine_margin_q15);
    (lo + off, best)
}

/// One sample of the eq (IV-4) pre-processing high-pass on the staged
/// Q14 constants (feedback-sign convention of the staged table):
/// `y(n) = x(n) − x(n−1) + (123/128)·y(n−1)`, realised as a Q14
/// `L_mult`/`L_mac` chain rounded back to Q0.
pub(crate) fn hpre_step(x: i32, prev_x: i32, prev_y: i32) -> i32 {
    use crate::plc_tables::{A_HP, B_HP};
    let mut acc = l_mult(x, B_HP[0]);
    acc = l_mac(acc, prev_x, B_HP[1]);
    acc = l_mac(acc, prev_y, A_HP[1]);
    round_fx(l_shl(acc, 1))
}

/// One sample of the eq (IV-19) higher-band post filter on its Q13
/// constants (feedback-sign convention):
/// `v(n) = (7303/8192)·(u(n) − u(n−1)) + (3207/4096)·v(n−1)`.
///
/// Realisation (vector-calibrated): each Q13 product is rounded back
/// to Q0 **separately** — `(c·x + 2^12) >> 13`, the `mult_r` shape at
/// Q13 — and the two terms combine through the saturating 16-bit
/// `add`. This filter conditions every higher-band sample for 4 s
/// after each erasure, so its realisation dominates the post-erasure
/// sample-exactness against the staged Appendix IV vectors: the
/// round-once single-accumulator variant drops the fleet-wide exact
/// count by roughly a factor of eight, and a truncating variant
/// (biased one LSB low over the whole hold window) collapses it
/// outright (`tests/appendix_iv_plc.rs`).
pub(crate) fn hpost_step(u: i32, prev_u: i32, prev_v: i32) -> i32 {
    use crate::basicop::{add, sub};
    use crate::plc_tables::{A_HP_POST, B_HP_POST};
    let diff = sub(u, prev_u);
    add(
        (B_HP_POST[0] * diff + 4096) >> 13,
        (A_HP_POST[1] * prev_v + 4096) >> 13,
    )
}

/// Eq (IV-3) LP residual through `A(z)` in Q12:
/// `e(n) = zl(n) + Σ a_i·zl(n−i)`, realised as a Q13 accumulator
/// (`L_mult(x, 4096)` + `L_mac` per tap) rounded back to Q0.
pub(crate) fn residual_step(a: &[i32; LP_ORDER], x: i32, past: impl Fn(usize) -> i32) -> i32 {
    let mut acc = l_mult(x, 4096);
    for (i, &ai) in a.iter().enumerate() {
        acc = l_mac(acc, ai, past(i + 1));
    }
    round_fx(l_shl(acc, 3))
}

/// Eq (IV-16) LP synthesis in Q12:
/// `yl_pre(n) = e(n) − Σ a_i·yl(n−i)`, same accumulator idiom as
/// [`residual_step`] with `L_msu` taps.
pub(crate) fn synthesis_step(a: &[i32; LP_ORDER], e: i32, mem: &[i32; LP_ORDER]) -> i32 {
    let mut acc = l_mult(e, 4096);
    for (i, &ai) in a.iter().enumerate() {
        acc = l_msu(acc, ai, mem[i]);
    }
    round_fx(l_shl(acc, 3))
}

/// Eq (IV-10) zero-crossing rate of `zl(−80 … −1)`; `hist` holds
/// `zl(−81 … −1)` oldest first (one extra sample for the `n − 1`
/// look-back).
pub(crate) fn zero_crossing_rate(hist: &[i32; 81]) -> u32 {
    let mut zcr = 0;
    for n in 1..81 {
        if hist[n] <= 0 && hist[n - 1] > 0 {
            zcr += 1;
        }
    }
    zcr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lp_analysis_of_silence_is_zero() {
        assert_eq!(lp_analysis(&[0; 80]), [0; LP_ORDER]);
    }

    #[test]
    fn lp_analysis_whitens_a_strong_ar1_signal() {
        // x(n) = 0.9 x(n−1) + w(n): the optimal one-tap predictor has
        // a1 ≈ −0.9 (eq IV-1 sign convention: A(z) = 1 + Σ a_i z^-i
        // and e = zl + Σ a_i·zl(n−i), so a1 ≈ −0.9 → Q12 ≈ −3686).
        let mut hist = [0i32; 80];
        let mut state = 0i64;
        let mut seed = 0x1234_5678_u32;
        for slot in hist.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let w = i64::from((seed >> 16) as i16) / 8;
            state = (state * 9) / 10 + w;
            *slot = state.clamp(-30_000, 30_000) as i32;
        }
        let a = lp_analysis(&hist);
        assert!(
            (-4300..=-3000).contains(&a[0]),
            "a1 = {} not near −0.9 in Q12",
            a[0]
        );
    }

    #[test]
    fn levinson_matches_a_double_precision_reference() {
        // Cross-check the fixed-point recursion against a straight
        // f64 evaluation of the same conditioned autocorrelations.
        // The signal carries a noise floor so the normal equations
        // stay well-conditioned: on a near-singular (pure multi-tone)
        // input any 32-bit recursion legitimately departs from f64
        // once the prediction error underflows the Q31 grid.
        let mut hist = [0i32; 80];
        let mut seed = 0x9e37_79b9_u32;
        for (i, slot) in hist.iter_mut().enumerate() {
            let ph = i as f64 / 7.3;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = i32::from((seed >> 16) as i16) / 16;
            *slot = (ph.sin() * 9000.0 + (ph * 2.1).cos() * 3000.0) as i32 + noise;
        }
        let r = autocorr(&hist, &LPC_WIN_80, LP_ORDER).unwrap();
        let got = levinson(&r, LP_ORDER);
        // f64 Levinson on the identical (already conditioned) r.
        let rf: Vec<f64> = r.iter().map(|&v| f64::from(v)).collect();
        let mut af = [0f64; LP_ORDER];
        let mut err = rf[0];
        for i in 1..=LP_ORDER {
            let mut acc = rf[i];
            for j in 1..i {
                acc += af[j - 1] * rf[i - j];
            }
            let k = -acc / err;
            let mut na = af;
            na[i - 1] = k;
            for j in 1..i {
                na[j - 1] = af[j - 1] + k * af[i - j - 1];
            }
            af = na;
            err *= 1.0 - k * k;
        }
        for i in 0..LP_ORDER {
            let want = (af[i] * 4096.0).round() as i32;
            assert!(
                (got[i] - want).abs() <= 4,
                "a{} fixed {} vs float {}",
                i + 1,
                got[i],
                want
            );
        }
    }

    #[test]
    fn div32_q31_is_accurate_to_a_few_lsb30() {
        for (num, den) in [
            (1, 3),
            (100, 101),
            (0x1234_5678, 0x2345_6789),
            (1, i32::MAX),
            (i32::MAX - 2, i32::MAX),
        ] {
            let got = i64::from(div32_q31(num, den));
            let want = (i64::from(num) << 31) / i64::from(den);
            assert!(
                (want - got).abs() <= 8,
                "div32_q31({num}, {den}) = {got}, want {want}"
            );
        }
    }

    #[test]
    fn decimation_fir_dc_gain_matches_the_staged_sum() {
        // A DC input at 1024 must come out at 1024·67973/65536 = 1062
        // once the FIR memory is filled — pinning the deliberately
        // non-unity gain of the staged table.
        let zlpre = [1024i32; 288];
        let t = decimate(&zlpre);
        assert!(
            t[8..].iter().all(|&v| v == 1062),
            "DC gain drifted: {:?}",
            &t[6..12]
        );
    }

    #[test]
    fn hpre_removes_dc_and_hpost_matches_its_pole() {
        // Constant input decays toward zero through both filters, up
        // to the latch point of each round-half-up recursion: for
        // H_pre's 123/128 pole the largest v with round(v·123/128) =
        // v is 12; H_post's 3207/4096 pole latches at 2. (An exact
        // filter would reach 0; the latch is the fixed-point floor,
        // and for H_pre it only biases the LTP pre-conditioning
        // input, not any output path.)
        let mut y = 0;
        let mut py = 0;
        let mut px = 0;
        for _ in 0..600 {
            y = hpre_step(1000, px, py);
            px = 1000;
            py = y;
        }
        assert!((0..=12).contains(&y), "H_pre DC residue {y}");
        let mut v = 0;
        let mut pv = 0;
        let mut pu = 0;
        for _ in 0..600 {
            v = hpost_step(1000, pu, pv);
            pu = 1000;
            pv = v;
        }
        assert!((0..=2).contains(&v), "H_post DC residue {v}");
    }

    #[test]
    fn preferred_max_margin_zero_is_the_plain_arg_max() {
        // Degenerate case: margin 0 must reproduce the strict arg max
        // keeping the smaller index on ties.
        let v = [3, -7, 12, 12, 5, 12, -1];
        assert_eq!(preferred_max(&v, 0), (2, 12));
        let w = [-4, -9, -2, -2];
        assert_eq!(preferred_max(&w, 0), (2, -2));
    }

    #[test]
    fn preferred_max_holds_the_smaller_lag_inside_the_margin() {
        // A longer lag whose correlation exceeds the incumbent's by
        // less than the relative margin must NOT displace it — the
        // pitch-multiple case the fitted rule exists for. Threshold at
        // margin 5536 for best = 10000: 10000 + round(10000·5536/2^15)
        // = 11689.
        let v = [10_000, 2_000, 500, 11_600, 300];
        assert_eq!(preferred_max(&v, TDS_SMALLER_PITCH_MARGIN_Q15), (0, 10_000));
        // Margin 0 would have taken the longer lag.
        assert_eq!(preferred_max(&v, 0), (3, 11_600));
        // A clear winner still displaces the incumbent.
        let w = [10_000, 2_000, 500, 11_700, 300];
        assert_eq!(preferred_max(&w, TDS_SMALLER_PITCH_MARGIN_Q15), (3, 11_700));
    }

    #[test]
    fn preferred_max_margin_scales_negative_incumbents_toward_zero() {
        // For a negative incumbent the α·r(best) threshold moves DOWN
        // (α > 1 makes a negative number more negative), so any later
        // value above it displaces — pinning the multiplicative
        // reading of the fitted rule on the r(i) < 0 side.
        let v = [-5_000, -4_990];
        assert_eq!(preferred_max(&v, TDS_SMALLER_PITCH_MARGIN_Q15), (1, -4_990));
    }

    #[test]
    fn ltp_margins_reproduce_the_fitted_and_plain_searches() {
        // On a two-periodicity signal (fundamental + a stronger-looking
        // multiple within the margin) the fitted search and the plain
        // arg max may legitimately differ; on a clean single pitch they
        // must agree.
        let mut zlpre = [0i32; 288];
        for (i, slot) in zlpre.iter_mut().enumerate() {
            let ph = (i % 60) as f64 / 60.0 * core::f64::consts::TAU;
            *slot = (ph.sin() * 8000.0 + (2.0 * ph).cos() * 2500.0) as i32;
        }
        let fitted = ltp_analysis(&zlpre);
        let plain = ltp_analysis_with_margins(&zlpre, 0, 0);
        assert_eq!(fitted, plain, "clean single pitch must be unaffected");
        assert_eq!(fitted.0, 60);
    }

    #[test]
    fn ltp_finds_a_planted_pitch() {
        // A 60-sample periodic signal in the pre-processed domain must
        // yield T0 = 60 (Tds = 15 in the decimated domain, refined ±2)
        // with a high Rmax.
        let mut zlpre = [0i32; 288];
        for (i, slot) in zlpre.iter_mut().enumerate() {
            let ph = (i % 60) as f64 / 60.0 * core::f64::consts::TAU;
            *slot = (ph.sin() * 8000.0 + (2.0 * ph).cos() * 2500.0) as i32;
        }
        let (t0, rmax) = ltp_analysis(&zlpre);
        assert_eq!(t0, 60, "pitch missed (Rmax {rmax})");
        assert!(rmax > 29_000, "Rmax {rmax} too low for a periodic signal");
    }
}

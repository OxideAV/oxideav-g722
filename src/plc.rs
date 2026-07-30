//! Appendix IV — low-complexity packet-loss concealment.
//!
//! Implements the PLC-extended G.722 decoder of Appendix IV of the
//! staged consolidated Recommendation (`docs/audio/g722/
//! T-REC-G.722-201209-I.pdf`, clauses IV.5 / IV.6): the standard
//! SB-ADPCM decoder plus a concealment path that, on erased frames,
//! extrapolates the lower sub-band with an LPC-based pitch repetition
//! (clause IV.6.1.2), the higher sub-band with a pitch-synchronous
//! repetition (clause IV.6.2.2), applies class-driven adaptive muting
//! (clause IV.6.1.2.7 / Table IV.3), updates the ADPCM decoder states
//! (clauses IV.6.1.4 / IV.6.2.4), and cross-fades back into decoded
//! audio on the first good frame (clause IV.6.1.5 / Table IV.4). A
//! 50-Hz remove-DC filter conditions the higher band during erasures
//! and for four seconds afterwards (clause IV.6.2.3).
//!
//! The algorithm operates on 10-ms or 20-ms frames (clause IV.4:
//! 2L samples at 16 kHz with L = 80 or 160 per sub-band).
//!
//! ## Numerical conventions
//!
//! The Recommendation publishes the concealment algorithm as
//! mathematical prose (clauses IV.5 / IV.6) and notes that it "can be
//! implemented in several other fashions" (clause IV.7). This
//! implementation follows the prose: the signal path (extrapolation,
//! muting, filters, state updates) runs in the same saturating 16-bit
//! integer arithmetic as the base codec (clause 5.2 operators), while
//! the analysis stage (LP / LTP / classification), whose only outputs
//! are the coefficients `a_i`, the pitch delay `T0`, the correlation
//! `Rmax` and the derived class, is computed in double precision from
//! the printed equations (IV-1 … IV-11).
//!
//! ## Provenance
//!
//! Clause / equation / figure citations refer to Appendix IV of the
//! staged 2012 consolidated Recommendation. The autocorrelation
//! conditioning follows the appendix's own description (clause
//! IV.6.1.2.1): a 60-Hz bandwidth-expansion lag window and a 40-dB
//! white-noise correction applied to `r(0)`. Validation against the
//! staged Appendix IV test vectors lives in `tests/appendix_iv_plc.rs`.

use crate::decoder::{HigherDecoderState, LowerDecoderState, Mode, ReceiveQmf};
use crate::predictor::{add, mul, sub};

extern crate alloc;
use alloc::vec::Vec;

/// Sub-band history kept for the lower-band analysis (clause
/// IV.6.1.2): 288 samples of signal (twice the maximal pitch delay of
/// 144), one sample of pitch jitter and eight samples of LPC memory.
const ZL_HISTORY: usize = 297;

/// Sub-band history kept for the higher-band repetition (clause
/// IV.6.2.2): 160 samples.
const ZH_HISTORY: usize = 160;

/// LP order of the lower-band analysis/synthesis filters (clause
/// IV.6.1.2.1, eq IV-1).
const LP_ORDER: usize = 8;

/// Number of sub-band samples over which the higher-band post filter
/// stays engaged after the last erasure (clause IV.6.2.3: "the first
/// 4 s following the erasure" at 8 kHz).
const POST_FILTER_HOLD: u32 = 32_000;

/// Q15 unity for the muting factors (clause IV.6.1.2.7: g_mute
/// initialised to 1).
const Q15_ONE: i32 = 32_767;

/// Extra synthesis length (sub-band samples) generated per erased
/// frame for cross-fading (clause IV.6.1.2.5: "80 extra samples
/// (10 ms)").
const XFADE: usize = 80;

/// Signal class of the frame preceding an erasure (clause IV.6.1.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Transient,
    Unvoiced,
    VuvTransition,
    WeaklyVoiced,
    Voiced,
}

/// Adaptive-muting parameters (Table IV.3).
#[derive(Debug, Clone, Copy)]
struct MutingParams {
    inc_mute: i32,
    fac1: i32,
    fac2p: i32,
    fac3p: i32,
    cf10: i32,
}

impl Class {
    /// Table IV.3 – Adaptive muting parameters.
    fn muting_params(self) -> MutingParams {
        match self {
            Class::Transient => MutingParams {
                inc_mute: 4,
                fac1: 409,
                fac2p: 409,
                fac3p: 409,
                cf10: 0,
            },
            Class::VuvTransition => MutingParams {
                inc_mute: 2,
                fac1: 10,
                fac2p: 10,
                fac3p: 399,
                cf10: 399,
            },
            _ => MutingParams {
                inc_mute: 1,
                fac1: 10,
                fac2p: 20,
                fac3p: 190,
                cf10: 20,
            },
        }
    }
}

/// Per-band adaptive muting state (clause IV.6.1.2.7 for the lower
/// band; clause IV.6.2.2.2 keeps a separate counter/factor pair for
/// the higher band).
#[derive(Debug, Clone, Copy)]
struct MuteState {
    g: i32,
    cnt: i32,
}

impl MuteState {
    fn reset(&mut self) {
        self.g = Q15_ONE;
        self.cnt = 0;
    }

    /// One sample of the "other cases" schedule (20-ms frames or
    /// consecutive erased 10-ms frames): threshold-accelerated
    /// decrements plus the per-sample counter increment.
    fn step_general(&mut self, p: &MutingParams) -> i32 {
        self.g -= p.fac1;
        if self.cnt >= 80 {
            self.g -= p.fac2p;
        }
        if self.cnt >= 160 {
            self.g -= p.fac3p;
        }
        if self.cnt >= 320 {
            self.g = 0;
        }
        if self.g < 0 {
            self.g = 0;
        }
        self.cnt += p.inc_mute;
        self.g
    }

    /// One sample of the first-erased-10-ms-frame schedule: a plain
    /// per-sample decrement by `fac` (fac1 for the frame proper, cf10
    /// for the extra cross-fade synthesis); the counter is bumped by
    /// the caller once per 80-sample block.
    fn step_first10(&mut self, fac: i32) -> i32 {
        self.g -= fac;
        if self.g < 0 {
            self.g = 0;
        }
        self.g
    }
}

/// Analysis results of the first erased frame, kept across consecutive
/// erasures (clause IV.6.1.3).
#[derive(Debug, Clone)]
struct Analysis {
    /// LP coefficients a_1..a_8 in Q12 (eq IV-1).
    a: [i32; LP_ORDER],
    /// Pitch delay T0 (clause IV.6.1.2.3, after the clause IV.6.1.2.4
    /// adjustments).
    t0: usize,
    /// Signal class (clause IV.6.1.2.4).
    class: Class,
}

/// G.722 decoder with the Appendix IV packet-loss concealment.
///
/// Frames are decoded through [`PlcDecoder::decode_good_frame`] /
/// [`PlcDecoder::conceal_erased_frame`]; both return `2 * L` samples
/// of 16-kHz PCM in the crate's 16-bit convention (the same output
/// convention as [`crate::Decoder::decode_pcm16`], which the Appendix
/// IV reference vectors use).
#[derive(Debug, Clone)]
pub struct PlcDecoder {
    mode: Mode,
    /// Sub-band frame length L (80 for 10-ms frames, 160 for 20-ms).
    l: usize,
    lower: LowerDecoderState,
    higher: HigherDecoderState,
    qmf: ReceiveQmf,
    /// Past lower-band reconstruction zl(-297..-1); index 0 is the
    /// oldest sample.
    zl_hist: [i32; ZL_HISTORY],
    /// Past higher-band reconstruction zh(-160..-1).
    zh_hist: [i32; ZH_HISTORY],
    /// Lower/higher-band muting (clauses IV.6.1.2.7 / IV.6.2.2.2).
    mute_lb: MuteState,
    mute_hb: MuteState,
    /// Whether the previous frame was erased.
    prev_erased: bool,
    /// Consecutive-erasure analysis snapshot (clause IV.6.1.3).
    analysis: Option<Analysis>,
    /// Muted extrapolated lower-band signal generated past the frame
    /// end for cross-fading, yl(L..L+79) (clause IV.6.1.2.5).
    yl_extra: [i32; XFADE],
    /// Higher-band post-filter memory (clause IV.6.2.3) as
    /// (previous input uh, previous output vh).
    hpost_mem: (i32, i32),
    /// Remaining sub-band samples of post-filter engagement (clause
    /// IV.6.2.3); refreshed to [`POST_FILTER_HOLD`] by every erased
    /// frame.
    post_hold: u32,
    /// Remaining sub-band samples of cross-fade on the next good
    /// frame (clause IV.6.1.5: first 10 ms only).
    xfade_pending: bool,
}

impl PlcDecoder {
    /// Create a PLC decoder for `frame_samples` 16-kHz samples per
    /// frame (160 = 10 ms or 320 = 20 ms, clause IV.4).
    ///
    /// # Panics
    /// Panics if `frame_samples` is not 160 or 320.
    pub fn new(mode: Mode, frame_samples: usize) -> Self {
        assert!(
            frame_samples == 160 || frame_samples == 320,
            "Appendix IV frames are 10 ms (160) or 20 ms (320) at 16 kHz"
        );
        Self {
            mode,
            l: frame_samples / 2,
            lower: LowerDecoderState::new(),
            higher: HigherDecoderState::new(),
            qmf: ReceiveQmf::new(),
            zl_hist: [0; ZL_HISTORY],
            zh_hist: [0; ZH_HISTORY],
            mute_lb: MuteState { g: Q15_ONE, cnt: 0 },
            mute_hb: MuteState { g: Q15_ONE, cnt: 0 },
            prev_erased: false,
            analysis: None,
            yl_extra: [0; XFADE],
            hpost_mem: (0, 0),
            post_hold: 0,
            xfade_pending: false,
        }
    }

    /// Sub-band frame length L.
    pub fn subband_frame_len(&self) -> usize {
        self.l
    }

    /// Number of 16-kHz samples per frame.
    pub fn frame_samples(&self) -> usize {
        self.l * 2
    }

    /// Decode one good frame of `L` octets (two 16-kHz samples per
    /// octet), applying the post-erasure reconvergence steps of
    /// clauses IV.6.1.5 (cross-fade) and IV.6.2.3 (post filter).
    ///
    /// # Panics
    /// Panics if `octets.len() != frame_samples() / 2`.
    pub fn decode_good_frame(&mut self, octets: &[u8]) -> Vec<i16> {
        assert_eq!(octets.len(), self.l, "one octet per two 16-kHz samples");
        let mut out = Vec::with_capacity(self.l * 2);
        let do_xfade = core::mem::take(&mut self.xfade_pending);
        for (n, &octet) in octets.iter().enumerate() {
            let ih = ((octet >> 6) & 0x3) as u32;
            let ilr = (octet & 0x3F) as u32;
            let xl = self.lower.step(ilr, self.mode);
            let xh = self.higher.step(ih);

            // Cross-fade (Table IV.4): during the first 10 ms after
            // the last erasure the ADPCM output is faded in against
            // the extrapolated signal.
            let zl = if do_xfade && n < XFADE {
                let n_i = n as i32;
                let w_up = (n_i * Q15_ONE) / 79;
                let w_dn = Q15_ONE - w_up;
                add(mul(w_up, xl), mul(w_dn, self.yl_extra[n]))
            } else {
                xl
            };

            // Higher band: post filter for 4 s after erasure (clause
            // IV.6.2.3).
            let zh = if self.post_hold > 0 {
                self.post_hold -= 1;
                self.hpost_step(xh)
            } else {
                xh
            };

            self.push_zl(zl);
            self.push_zh(zh);
            let (a, b) = self.qmf.step_pcm16(zl, zh);
            out.push(a);
            out.push(b);
        }
        // Clause IV.6.1.1 / IV.6.2.1: reset muting on good frames.
        self.mute_lb.reset();
        self.mute_hb.reset();
        self.prev_erased = false;
        self.analysis = None;
        out
    }

    /// Conceal one erased frame (clauses IV.6.1.2 / IV.6.1.3 /
    /// IV.6.2.2), returning the extrapolated 16-kHz PCM.
    pub fn conceal_erased_frame(&mut self) -> Vec<i16> {
        let l = self.l;
        let first_erased = !self.prev_erased;

        // ---- Lower band ----------------------------------------
        if first_erased {
            self.analysis = Some(self.analyse_lower());
        }
        let analysis = self.analysis.clone().expect("analysis set above");

        // Residual of the past signal through A(z) (eq IV-3), for the
        // repetition period plus the jitter/modification margin.
        let e_hist = self.residual_history(&analysis.a);

        // Extrapolate the residual and synthesise yl (muted), plus
        // the 80-sample cross-fade extension.
        let (yl, yl_extra) = self.synthesise_lower(&analysis, &e_hist, first_erased);

        // ---- Higher band (clause IV.6.2.2) ---------------------
        let th = if analysis.class == Class::Voiced {
            analysis.t0
        } else {
            80
        };
        let p = analysis.class.muting_params();
        let mut yh = Vec::with_capacity(l);
        {
            // Pitch-synchronous repetition over a rolling buffer.
            let mut ring: Vec<i32> = self.zh_hist.to_vec();
            for n in 0..l {
                let idx = ring.len() - th;
                let sample = ring[idx];
                let g = if first_erased && l == 80 {
                    self.mute_hb.step_first10(p.fac1)
                } else {
                    self.mute_hb.step_general(&p)
                };
                let muted = sat16((g * sample + 16_384) >> 15);
                // uh = yh; vh = Hpost(uh); zh = vh (clause IV.6.2.3).
                let vh = self.hpost_step(muted);
                yh.push(vh);
                ring.push(vh);
                let _ = n;
            }
            if first_erased && l == 80 {
                self.mute_hb.cnt += 80 * p.inc_mute;
            }
        }
        self.post_hold = POST_FILTER_HOLD;

        // ---- ADPCM state updates -------------------------------
        self.update_lower_adpcm_state(&yl, &yl_extra);
        self.update_higher_adpcm_state();

        // ---- Histories, QMF, bookkeeping -----------------------
        let mut out = Vec::with_capacity(l * 2);
        for n in 0..l {
            let zl = yl[n];
            let zh = yh[n];
            self.push_zl(zl);
            self.push_zh(zh);
            let (a, b) = self.qmf.step_pcm16(zl, zh);
            out.push(a);
            out.push(b);
        }
        self.yl_extra = yl_extra;
        self.prev_erased = true;
        self.xfade_pending = true;
        out
    }

    // ------------------------------------------------------------
    // History plumbing
    // ------------------------------------------------------------

    fn push_zl(&mut self, v: i32) {
        self.zl_hist.copy_within(1.., 0);
        self.zl_hist[ZL_HISTORY - 1] = v;
    }

    fn push_zh(&mut self, v: i32) {
        self.zh_hist.copy_within(1.., 0);
        self.zh_hist[ZH_HISTORY - 1] = v;
    }

    /// zl(n) for n in -297..=-1 (clause IV.6.1.2 indexing).
    fn zl(&self, n: isize) -> i32 {
        debug_assert!((-(ZL_HISTORY as isize)..0).contains(&n));
        self.zl_hist[(ZL_HISTORY as isize + n) as usize]
    }

    // ------------------------------------------------------------
    // Higher-band post filter (clause IV.6.2.3, eq IV-19)
    // ------------------------------------------------------------

    /// One sample of Hpost(z) = (7303/8192)(1 - z^-1) /
    /// (1 - (3207/4096) z^-1).
    ///
    /// The eq IV-19 coefficients are applied at their printed
    /// precisions (numerator Q13, denominator Q12) with
    /// round-to-nearest on each term — the convention that best
    /// matches the staged Appendix IV vectors among the rounding /
    /// width variants surveyed (see `tests/appendix_iv_plc.rs` for the
    /// residual-divergence characterisation).
    fn hpost_step(&mut self, uh: i32) -> i32 {
        let (prev_in, prev_out) = self.hpost_mem;
        let diff = sub(uh, prev_in);
        let vh = sat16(((7303 * diff + 4096) >> 13) + ((3207 * prev_out + 2048) >> 12));
        self.hpost_mem = (uh, vh);
        vh
    }

    // ------------------------------------------------------------
    // Lower-band analysis (clause IV.6.1.2, first erased frame only)
    // ------------------------------------------------------------

    fn analyse_lower(&self) -> Analysis {
        // --- LP analysis (clause IV.6.1.2.1) -------------------
        let a_f = self.lp_analysis();
        let mut a = [0_i32; LP_ORDER];
        for (q, &f) in a.iter_mut().zip(a_f.iter()) {
            *q = (f * 4096.0).round() as i32;
        }

        // --- Pre-processing (clause IV.6.1.2.2, eq IV-4) -------
        // zlpre(n), n = -288..-1 through Hpre(z) =
        // (1 - z^-1)/(1 - (123/128) z^-1); memory starts at zero.
        let mut zlpre = [0_i32; 288];
        let mut prev_in = 0_i32;
        let mut prev_out = 0_i32;
        for (k, slot) in zlpre.iter_mut().enumerate() {
            let x = self.zl(k as isize - 288);
            let y = add(sub(x, prev_in), mul(31_488, prev_out));
            prev_in = x;
            prev_out = y;
            *slot = y;
        }

        // --- LTP analysis (clause IV.6.1.2.3) ------------------
        let (t0_raw, rmax) = self.ltp_analysis(&zlpre);

        // --- Classification (clause IV.6.1.2.4, Figure IV.4) ---
        let nbl = self.lower.log_scale_factor();
        let nbh = self.higher.log_scale_factor();
        let zcr = self.zero_crossing_rate();

        let mut t0 = t0_raw;
        let mut class = Class::WeaklyVoiced;
        if rmax > 0.7 {
            class = Class::Voiced;
            if nbh > nbl {
                class = Class::WeaklyVoiced;
            }
        } else if nbh > nbl {
            class = Class::VuvTransition;
        }
        if zcr >= 20 {
            class = Class::Unvoiced;
            if t0 < 32 {
                t0 *= 2;
            }
        }
        // cnt_peak needs the residual (eq IV-11); compute it against
        // the un-modified residual history.
        if class != Class::Voiced {
            let e_hist = self.residual_history(&a);
            if self.count_peaks(&e_hist, t0) > 0 {
                class = Class::Transient;
            }
        }
        // "if class is not VOICED, and T0 is even, T0 is increased
        // by 1" (clause IV.6.1.2.4).
        if class != Class::Voiced && t0 % 2 == 0 {
            t0 += 1;
        }

        Analysis { a, t0, class }
    }

    /// Eighth-order LP analysis on the last 10 ms of zl (clause
    /// IV.6.1.2.1): asymmetrical Hamming window (eq IV-2),
    /// autocorrelation with 60-Hz bandwidth expansion and 40-dB
    /// white-noise correction, Levinson-Durbin recursion. Returns
    /// a_1..a_8 of eq IV-1 as doubles.
    fn lp_analysis(&self) -> [f64; LP_ORDER] {
        // Window (eq IV-2) over zl(-80..-1).
        let mut wx = [0.0_f64; 80];
        for (k, slot) in wx.iter_mut().enumerate() {
            let n = k as isize - 80; // n = -80..-1
            let w = lpc_window(n);
            *slot = w * f64::from(self.zl(n));
        }
        let mut r = [0.0_f64; LP_ORDER + 1];
        for (kk, slot) in r.iter_mut().enumerate() {
            let mut acc = 0.0;
            for j in kk..80 {
                acc += wx[j] * wx[j - kk];
            }
            *slot = acc;
        }
        conditioned_levinson(&mut r)
    }

    /// LTP open-loop pitch analysis (clause IV.6.1.2.3, Figure IV.3).
    /// Returns (T0, Rmax).
    fn ltp_analysis(&self, zlpre: &[i32; 288]) -> (usize, f64) {
        // Low-pass + 4:1 decimation (eq IV-5). Filter memory starts
        // at zero; t(n) is the filter output at the last sample of
        // each block of four.
        const FIR: [i64; 9] = [3692, 6190, 8525, 10186, 10787, 10186, 8525, 6190, 3692];
        let mut t = [0.0_f64; 72];
        for (k, slot) in t.iter_mut().enumerate() {
            // t(k) taps zlpre at m = 4k+3 (the last sample of block k).
            let m = 4 * k + 3;
            let mut acc: i64 = 0;
            for (j, &h) in FIR.iter().enumerate() {
                let idx = m as isize - j as isize;
                let x = if idx >= 0 { zlpre[idx as usize] } else { 0 };
                acc += h * i64::from(x);
            }
            *slot = (acc >> 16) as f64;
        }

        // 2nd-order LP of t weighted by gamma = 0.94 (clause
        // IV.6.1.2.3). The window is the last 72 samples of wlp.
        let (b1, b2) = {
            let mut wt = [0.0_f64; 72];
            for (k, slot) in wt.iter_mut().enumerate() {
                let n = k as isize - 72; // n = -72..-1
                *slot = lpc_window(n) * t[k];
            }
            let mut r = [0.0_f64; 3];
            for (kk, slot) in r.iter_mut().enumerate() {
                let mut acc = 0.0;
                for j in kk..72 {
                    acc += wt[j] * wt[j - kk];
                }
                *slot = acc;
            }
            let a = conditioned_levinson_order2(&mut r);
            // B(z) = 1 - b1 z^-1 - b2 z^-2 while the recursion
            // returns A(z) = 1 + a1 z^-1 + a2 z^-2.
            (-a[0], -a[1])
        };
        const GAMMA: f64 = 0.94;
        let mut tw = [0.0_f64; 72];
        for k in 0..72 {
            let t1 = if k >= 1 { t[k - 1] } else { 0.0 };
            let t2 = if k >= 2 { t[k - 2] } else { 0.0 };
            tw[k] = t[k] - GAMMA * b1 * t1 - GAMMA * GAMMA * b2 * t2;
        }

        // Normalized cross-correlation (eq IV-6) over the last 35
        // weighted-decimated samples: j = -35..-1 maps to tw[37..72].
        let r_at = |i: usize| -> f64 {
            let mut num = 0.0;
            let mut d1 = 0.0;
            let mut d2 = 0.0;
            for j in 0..35 {
                let x = tw[37 + j];
                let y = tw[37 + j - i];
                num += x * y;
                d1 += x * x;
                d2 += y * y;
            }
            let den = d1.max(d2);
            if den <= 0.0 {
                0.0
            } else {
                num / den
            }
        };
        let mut tds: usize = 18; // initialization (clause IV.6.1.2.3 a).
        let r_vals: Vec<f64> = (0..=35)
            .map(|i| if i == 0 { 0.0 } else { r_at(i) })
            .collect();
        if (1..=35).any(|i| r_vals[i] < 0.0) {
            let i0 = (1..=35).find(|&i| r_vals[i] < 0.0).unwrap();
            let i1 = i0.max(4);
            let mut best = i1;
            for i in i1..=35 {
                if r_vals[i] > r_vals[best] {
                    best = i;
                }
            }
            tds = best;
        }

        // Refinement in the pre-processed domain (eqs IV-8 / IV-9):
        // T = 4 Tds, search i = T-2 .. T+2 with a window of length T.
        let t_mid = 4 * tds;
        let big_r = |i: usize| -> f64 {
            let win = t_mid;
            let mut num = 0.0;
            let mut d1 = 0.0;
            let mut d2 = 0.0;
            for jj in 0..win {
                // j = -T..-1 maps to zlpre[288 - T + jj].
                let x = f64::from(zlpre[288 - win + jj]);
                let yidx = 288 - win + jj - i;
                let y = f64::from(zlpre[yidx]);
                num += x * y;
                d1 += x * x;
                d2 += y * y;
            }
            let den = d1.max(d2);
            if den <= 0.0 {
                0.0
            } else {
                num / den
            }
        };
        let lo = t_mid.saturating_sub(2).max(1);
        let hi = (t_mid + 2).min(288 - t_mid);
        let mut t0 = lo;
        let mut best = f64::MIN;
        for i in lo..=hi {
            let v = big_r(i);
            if v > best {
                best = v;
                t0 = i;
            }
        }
        (t0, best)
    }

    /// Zero-crossing rate of zl(-80..-1) (eq IV-10).
    fn zero_crossing_rate(&self) -> u32 {
        let mut zcr = 0;
        for n in -80..0_isize {
            if self.zl(n) <= 0 && self.zl(n - 1) > 0 {
                zcr += 1;
            }
        }
        zcr
    }

    /// Residual history e(n), n = -289..-1 (eq IV-3), returned as a
    /// slice indexed by `[289 + n]`.
    fn residual_history(&self, a: &[i32; LP_ORDER]) -> Vec<i32> {
        let mut e = Vec::with_capacity(289);
        for n in -289..0_isize {
            let mut acc: i64 = 0;
            for (i, &ai) in a.iter().enumerate() {
                acc += i64::from(ai) * i64::from(self.zl(n - 1 - i as isize));
            }
            let pred = (acc >> 12) as i32;
            e.push(add(self.zl(n), sat16(pred)));
        }
        e
    }

    /// Number of large residual peaks in the last pitch period
    /// (eq IV-11).
    fn count_peaks(&self, e_hist: &[i32], t0: usize) -> u32 {
        let mut cnt = 0;
        for n in -(t0 as isize)..0 {
            let cur = e_hist[(289 + n) as usize].abs() >> 2;
            let mut prev_max = 0;
            for i in -2..=2_isize {
                let idx = 289 + n - t0 as isize + i;
                if (0..289).contains(&idx) {
                    prev_max = prev_max.max(e_hist[idx as usize].abs());
                }
            }
            if cur > prev_max {
                cnt += 1;
            }
        }
        cnt
    }

    // ------------------------------------------------------------
    // Lower-band synthesis (clauses IV.6.1.2.5 – IV.6.1.2.7,
    // IV.6.1.3)
    // ------------------------------------------------------------

    /// Extrapolate the residual, run the synthesis filter and apply
    /// the adaptive muting; returns (yl frame, yl cross-fade extra).
    fn synthesise_lower(
        &mut self,
        analysis: &Analysis,
        e_hist: &[i32],
        first_erased: bool,
    ) -> (Vec<i32>, [i32; XFADE]) {
        let l = self.l;
        let t0 = analysis.t0;
        let p = analysis.class.muting_params();

        // Repetition-period residual, modified per eq IV-12 when the
        // class is not VOICED.
        let mut rep: Vec<i32> = Vec::with_capacity(t0 + 1);
        // Keep one jitter sample before the period (index -T0-1).
        let jitter_pre = e_hist[289 - t0 - 1];
        rep.push(jitter_pre);
        for n in -(t0 as isize)..0 {
            let raw = e_hist[(289 + n) as usize];
            let v = if analysis.class == Class::Voiced {
                raw
            } else {
                let mut prev_max = 0;
                for i in -2..=2_isize {
                    let idx = 289 + n - t0 as isize + i;
                    if (0..289).contains(&idx) {
                        prev_max = prev_max.max(e_hist[idx as usize].abs());
                    }
                }
                let mag = raw.abs().min(prev_max);
                if raw < 0 {
                    -mag
                } else {
                    mag
                }
            };
            rep.push(v);
        }
        // rep[1 + k] = e(-T0 + k); rep[0] = e(-T0 - 1).

        // Extrapolated residual e(0..count-1) via pitch repetition
        // (eqs IV-14 / IV-15); generation extends its own buffer.
        let count = if first_erased || l == 160 {
            l + XFADE
        } else {
            // Consecutive 10-ms erasure: first 80 samples are copied
            // from the previous frame's extra synthesis (clause
            // IV.6.1.3); only L more are newly synthesised — but the
            // residual indexing continues, so generate the full span
            // and use the tail.
            l + XFADE
        };
        let mut e_ext: Vec<i32> = Vec::with_capacity(count);
        {
            // e(n) for n >= 0: index into the virtual sequence where
            // e(m) for m in -T0-1..-1 is rep[m + T0 + 1] and generated
            // samples append after.
            let at = |e_ext: &Vec<i32>, m: isize| -> i32 {
                if m >= 0 {
                    e_ext[m as usize]
                } else {
                    rep[(m + t0 as isize + 1) as usize]
                }
            };
            for n in 0..count as isize {
                let m = if analysis.class == Class::Voiced {
                    n - t0 as isize
                } else {
                    let j = if n % 2 == 0 { 1 } else { -1 };
                    n - t0 as isize + j
                };
                let v = at(&e_ext, m);
                e_ext.push(v);
            }
        }

        // LP synthesis (eq IV-16) + muting (eq IV-17). The filter
        // memory is the muted output yl; it starts from the stored
        // reconstruction zl(-1..-8).
        let mut mem: [i32; LP_ORDER] = core::array::from_fn(|i| self.zl(-1 - i as isize));
        let mut yl_all: Vec<i32> = Vec::with_capacity(count);
        for (n, &e_n) in e_ext.iter().enumerate() {
            let mut acc: i64 = 0;
            for (i, &ai) in analysis.a.iter().enumerate() {
                acc += i64::from(ai) * i64::from(mem[i]);
            }
            let pred = sat16((acc >> 12) as i32);
            let ylpre = sub(e_n, pred);
            let g = if first_erased && l == 80 {
                if n < 80 {
                    self.mute_lb.step_first10(p.fac1)
                } else {
                    if n == 80 {
                        self.mute_lb.cnt += 80 * p.inc_mute;
                    }
                    self.mute_lb.step_first10(p.cf10)
                }
            } else {
                self.mute_lb.step_general(&p)
            };
            let yl_n = sat16((g * ylpre + 16_384) >> 15);
            mem.rotate_right(1);
            mem[0] = yl_n;
            yl_all.push(yl_n);
        }
        if first_erased && l == 80 {
            self.mute_lb.cnt += 80 * p.inc_mute;
        }

        let (frame, extra) = if first_erased || l == 160 {
            let frame = yl_all[..l].to_vec();
            let mut ex = [0_i32; XFADE];
            ex.copy_from_slice(&yl_all[l..l + XFADE]);
            (frame, ex)
        } else {
            // Consecutive 10-ms erasure: frame = previous extra;
            // freshly synthesised span provides the new extra.
            let mut frame = self.yl_extra.to_vec();
            frame.truncate(l);
            let mut ex = [0_i32; XFADE];
            ex.copy_from_slice(&yl_all[l..l + XFADE]);
            (frame, ex)
        };
        (frame, extra)
    }

    // ------------------------------------------------------------
    // ADPCM state updates (clauses IV.6.1.4 / IV.6.2.4)
    // ------------------------------------------------------------

    fn update_lower_adpcm_state(&mut self, yl: &[i32], yl_extra: &[i32; XFADE]) {
        let l = self.l;
        let s = self.lower.state_mut();
        // DLT_i = 0, i = 1..6.
        s.dlt = [0; 7];
        // PLT_i = yl(L - i)/2, i = 1, 2.
        s.plt[1] = yl[l - 1] >> 1;
        s.plt[2] = yl[l - 2] >> 1;
        s.plt[0] = s.plt[1];
        // RLT_1 = yl(L - 1).
        s.rlt[1] = yl[l - 1];
        s.rlt[0] = s.rlt[1];
        // SL = yl(L), SZL = yl(L)/2 — one-shot prediction override
        // consumed by the next decoded sample.
        s.pending_prediction = Some((yl_extra[0], yl_extra[0] >> 1));
        if self.mute_hb.cnt > 160 {
            s.detl = 32;
            s.nbl = 0;
        }
    }

    fn update_higher_adpcm_state(&mut self) {
        let cnt = self.mute_hb.cnt;
        let s = self.higher.state_mut();
        // NBH = NBH / 2; DETH = scaleh(NBH).
        s.nbl >>= 1;
        s.detl = crate::predictor::SubBandState::linear_scale_method2(s.nbl, 10);
        if cnt > 160 {
            s.nbl = 0;
            s.detl = 8;
        }
    }
}

/// 16-bit saturation matching the clause 5.2 operators.
fn sat16(x: i32) -> i32 {
    x.clamp(-32_768, 32_767)
}

/// The asymmetrical Hamming LP window of eq IV-2, evaluated at
/// n = -80..-1.
fn lpc_window(n: isize) -> f64 {
    debug_assert!((-80..0).contains(&n));
    let n = n as f64;
    if n <= -11.0 {
        0.54 - 0.46 * (core::f64::consts::PI * (n + 80.0) / 69.0).cos()
    } else {
        0.54 + 0.46 * (core::f64::consts::PI * (n + 11.0) / 10.0).cos()
    }
}

/// Levinson-Durbin with the clause IV.6.1.2.1 conditioning: 40-dB
/// white-noise correction on r(0) and 60-Hz bandwidth-expansion lag
/// window. Returns a_1..a_8 of A(z) = 1 + Σ a_i z^-i.
fn conditioned_levinson(r: &mut [f64; LP_ORDER + 1]) -> [f64; LP_ORDER] {
    condition_autocorrelation(r);
    let mut a = [0.0_f64; LP_ORDER];
    if r[0] <= 0.0 {
        return a;
    }
    let mut err = r[0];
    for i in 1..=LP_ORDER {
        let mut acc = r[i];
        for j in 1..i {
            acc += a[j - 1] * r[i - j];
        }
        let k = -acc / err;
        let mut new_a = a;
        new_a[i - 1] = k;
        for j in 1..i {
            new_a[j - 1] = a[j - 1] + k * a[i - j - 1];
        }
        a = new_a;
        err *= 1.0 - k * k;
        if err <= 0.0 {
            break;
        }
    }
    a
}

/// Order-2 variant for the LTP weighting filter (clause IV.6.1.2.3).
fn conditioned_levinson_order2(r: &mut [f64; 3]) -> [f64; 2] {
    let mut full = [0.0_f64; LP_ORDER + 1];
    full[..3].copy_from_slice(r);
    condition_autocorrelation_n(&mut full, 2);
    let r = &full;
    let mut a = [0.0_f64; 2];
    if r[0] <= 0.0 {
        return a;
    }
    let k1 = -r[1] / r[0];
    let e1 = r[0] * (1.0 - k1 * k1);
    if e1 <= 0.0 {
        a[0] = k1;
        return a;
    }
    let k2 = -(r[2] + k1 * r[1]) / e1;
    a[0] = k1 + k2 * k1;
    a[1] = k2;
    a
}

/// White-noise correction (40 dB → ×1.0001 on r(0)) and 60-Hz
/// bandwidth-expansion lag window (clause IV.6.1.2.1, following the
/// autocorrelation conditioning it cites).
fn condition_autocorrelation(r: &mut [f64; LP_ORDER + 1]) {
    condition_autocorrelation_n(r, LP_ORDER);
}

fn condition_autocorrelation_n(r: &mut [f64; LP_ORDER + 1], order: usize) {
    r[0] *= 1.0001;
    if r[0] <= 0.0 {
        r[0] = 1.0;
    }
    for (k, slot) in r.iter_mut().enumerate().take(order + 1).skip(1) {
        let f = 2.0 * core::f64::consts::PI * 60.0 * k as f64 / 8000.0;
        *slot *= (-0.5 * f * f).exp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_frames_match_the_plain_decoder() {
        // With no erasures the PLC decoder must be transparent: same
        // output as the plain decoder for any stream.
        let octets: Vec<u8> = (0..800).map(|i| (i * 37 + 11) as u8).collect();
        let mut plain = crate::Decoder::new(Mode::Mode1);
        let expect = plain.decode_pcm16(&octets);
        let mut plc = PlcDecoder::new(Mode::Mode1, 160);
        let mut got = Vec::new();
        for f in octets.chunks(80) {
            got.extend(plc.decode_good_frame(f));
        }
        assert_eq!(got, expect);
    }

    #[test]
    fn concealment_output_is_bounded_and_correct_length() {
        let mut plc = PlcDecoder::new(Mode::Mode1, 160);
        // Prime with an arbitrary good frame, then conceal.
        let octets: Vec<u8> = (0..80).map(|i| (i * 13 + 7) as u8).collect();
        let _ = plc.decode_good_frame(&octets);
        for _ in 0..5 {
            let out = plc.conceal_erased_frame();
            assert_eq!(out.len(), 160);
        }
        // Recovery frame cross-fades without panicking.
        let out = plc.decode_good_frame(&octets);
        assert_eq!(out.len(), 160);
    }

    #[test]
    fn twenty_ms_frames_work_end_to_end() {
        let mut plc = PlcDecoder::new(Mode::Mode1, 320);
        let octets: Vec<u8> = (0..160).map(|i| (i * 29 + 3) as u8).collect();
        let _ = plc.decode_good_frame(&octets);
        let out = plc.conceal_erased_frame();
        assert_eq!(out.len(), 320);
        let out = plc.decode_good_frame(&octets);
        assert_eq!(out.len(), 320);
    }

    #[test]
    fn muting_schedules_match_the_figure_iv6_iv7_trajectories() {
        // Figure IV.6 / IV.7 (clause IV.6.1.2.7): under the general
        // (20-ms / consecutive) schedule the gain must reach zero by
        // ~80 samples for TRANSIENT, ~160 for VUV_TRANSITION and ~300
        // for the other classes, and never rebound.
        for (class, zero_by) in [
            (Class::Transient, 81),
            (Class::VuvTransition, 165),
            (Class::WeaklyVoiced, 305),
        ] {
            let p = class.muting_params();
            let mut m = MuteState { g: Q15_ONE, cnt: 0 };
            let mut reached = None;
            let mut prev = Q15_ONE;
            for n in 0..400 {
                let g = m.step_general(&p);
                assert!(g <= prev, "{class:?}: gain rebounded at {n}");
                assert!((0..=Q15_ONE).contains(&g));
                if g == 0 && reached.is_none() {
                    reached = Some(n);
                }
                prev = g;
            }
            let reached = reached.expect("gain must reach zero");
            assert!(
                reached <= zero_by,
                "{class:?}: gain reached zero at {reached}, expected <= {zero_by}"
            );
            // The hard cutoff of the schedule: once cnt >= 320 the
            // gain is forced to zero outright.
            assert_eq!(m.g, 0);
        }
        // First-erased-10-ms special case, TRANSIENT: fac1 = 409 for
        // 80 samples leaves the printed near-zero residue 32767 -
        // 80*409 = 47, and the cf10 = 0 extra keeps it there.
        let p = Class::Transient.muting_params();
        let mut m = MuteState { g: Q15_ONE, cnt: 0 };
        for _ in 0..80 {
            m.step_first10(p.fac1);
        }
        assert_eq!(m.g, 47);
        for _ in 0..80 {
            m.step_first10(p.cf10);
        }
        assert_eq!(m.g, 47);
    }

    #[test]
    fn hpost_removes_a_dc_step() {
        // Clause IV.6.2.3: Hpost is a 50-Hz remove-DC filter. A
        // constant input must decay towards zero (a latched rounding
        // residue of at most one LSB is the fixed-point floor).
        let mut plc = PlcDecoder::new(Mode::Mode1, 160);
        let mut last = i32::MAX;
        for n in 0..400 {
            let v = plc.hpost_step(1000);
            if n == 0 {
                // First sample passes the full step scaled by b0.
                assert!((880..=900).contains(&v), "step response head {v}");
            }
            last = v;
        }
        // The per-term round-to-nearest recursion latches at
        // (3207*2 + 2048) >> 12 = 2, so the reachable floor is two
        // LSB of DC on this step size.
        assert!(last.abs() <= 2, "DC not removed: residue {last}");
    }

    #[test]
    fn voiced_concealment_continues_a_periodic_signal() {
        // Feed a strongly periodic lower-band signal (pitch well
        // inside the 288-sample window) through good frames, then
        // conceal: the classifier must pick a pitch that keeps the
        // extrapolation aligned with the true continuation.
        let period = 57usize; // odd, in range after refinement
                              // Build a periodic 16-kHz waveform and encode it so the
                              // decoder history is realistic.
        let mut enc = crate::Encoder::new();
        let pcm: Vec<i16> = (0..4800)
            .map(|i| {
                let ph = (i % (2 * period)) as f64 / (2 * period) as f64;
                ((ph * 2.0 * core::f64::consts::PI).sin() * 6000.0) as i16
            })
            .collect();
        let octets = enc.encode_pcm16(&pcm);
        let mut plc = PlcDecoder::new(Mode::Mode1, 160);
        let mut decoded: Vec<i16> = Vec::new();
        for f in octets.chunks(80) {
            if f.len() == 80 {
                decoded.extend(plc.decode_good_frame(f));
            }
        }
        let concealed = plc.conceal_erased_frame();
        // The concealed frame must correlate strongly with the
        // continuation of the periodic waveform: compare against the
        // last decoded period repeated.
        let hist = &decoded[decoded.len() - 2 * period..];
        let mut num = 0f64;
        let mut d1 = 0f64;
        let mut d2 = 0f64;
        for i in 0..160 {
            let a = f64::from(concealed[i]);
            let b = f64::from(hist[i % (2 * period)]);
            num += a * b;
            d1 += a * a;
            d2 += b * b;
        }
        let corr = num / (d1.sqrt() * d2.sqrt()).max(1.0);
        // The Table IV.3 muting walks the gain down through the frame,
        // so the correlation against the unmuted continuation sits
        // below unity even for a perfect pitch; an octave/period error
        // would collapse it far further.
        assert!(
            corr > 0.7,
            "voiced concealment lost the waveform periodicity: corr {corr:.3}"
        );
    }

    #[test]
    fn erasure_from_reset_state_is_silent() {
        // ovfl.bst opens with an erased frame: concealing from the
        // reset state must extrapolate silence, not noise.
        let mut plc = PlcDecoder::new(Mode::Mode1, 320);
        let out = plc.conceal_erased_frame();
        assert!(out.iter().all(|&s| s == 0), "reset-state concealment");
    }
}

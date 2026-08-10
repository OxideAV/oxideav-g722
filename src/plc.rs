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
//! Clause IV.7 makes the 16-bit fixed-point realisation normative
//! over the prose of clauses IV.5 / IV.6. Both the signal path
//! (extrapolation, muting, filters, state updates) and the analysis
//! stage (LP / LTP / classification) therefore run entirely in the
//! ITU-T G.191 STL basic-operator arithmetic ([`crate::basicop`],
//! semantics from the staged `docs/audio/g722/basic-operators/`
//! notes) on the staged Appendix IV Q-format data tables
//! ([`crate::plc_tables`], from `docs/audio/g722/tables/`). The
//! fixed-point analysis machinery lives in [`crate::plc_analysis`].
//!
//! ## Provenance
//!
//! Clause / equation / figure citations refer to Appendix IV of the
//! staged 2012 consolidated Recommendation. The autocorrelation
//! conditioning uses the staged double-precision lag-window pair
//! (60-Hz bandwidth expansion with the 40-dB white-noise correction
//! folded in). Validation against the staged Appendix IV test vectors
//! lives in `tests/appendix_iv_plc.rs`. Two pieces remain outside the
//! staged material and are documented at their sites: the exact
//! instruction sequence of the reference realisation (only the
//! operator semantics and the data tables are staged) and the clause
//! IV.6.1.2.3 "procedure favouring the smaller pitch values"
//! (`docs/audio/g722/appendix-IV-ltp-smaller-pitch-gap.md`).

use crate::basicop::{l_mac, l_mult, mult_r, round_fx};
use crate::decoder::{HigherDecoderState, LowerDecoderState, Mode, ReceiveQmf};
use crate::plc_analysis::{
    self, hpre_step, lp_analysis, ltp_analysis, residual_step, synthesis_step, LP_ORDER,
};

extern crate alloc;
use alloc::vec::Vec;

/// Sub-band history kept for the lower-band analysis (clause
/// IV.6.1.2): 288 samples of signal (twice the maximal pitch delay of
/// 144), one sample of pitch jitter and eight samples of LPC memory.
const ZL_HISTORY: usize = 297;

/// Sub-band history kept for the higher-band repetition (clause
/// IV.6.2.2): 160 samples.
const ZH_HISTORY: usize = 160;

/// Q15 rendering of the Figure IV.4 voicing threshold: "Rmax > 0.7"
/// holds for a Q15 correlation strictly above `round(0.7 × 32768) −
/// 1 = 22937`. The exact fixed-point constant the reference compares
/// against is not staged; this rendering is the assumption in use.
const RMAX_VOICED_Q15: i32 = 22_937;

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

/// Residual-extrapolation continuation state, carried across the
/// erased frames of one erasure run (clause IV.6.1.3: consecutive bad
/// frames keep the first frame's parameters and *continue* the
/// synthesis — only the last L samples of each further frame are newly
/// generated, the first 80 being the previous frame's cross-fade
/// extra).
#[derive(Debug, Clone)]
struct ConcealState {
    /// The last `T0 + 2` extrapolated residual samples, ending at
    /// absolute index `next_abs − 1` (initially the modified
    /// repetition period `e(−T0−1 … −1)` of eqs IV-12/IV-13). The
    /// generation rules (eqs IV-14 / IV-15) only ever reach back
    /// `T0 + 1` samples, so this window is sufficient.
    e_recent: Vec<i32>,
    /// Absolute index (from the start of the erasure run) of the next
    /// residual sample to generate; the eq IV-15 jitter parity
    /// `(−1)^n` follows this continuing index.
    next_abs: usize,
    /// Eq IV-16 synthesis-filter memory: yl(n−1) … yl(n−8) of the
    /// *muted* output, continued across frames.
    mem: [i32; LP_ORDER],
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
    /// Residual/synthesis continuation for the current erasure run
    /// (clause IV.6.1.3).
    conceal: Option<ConcealState>,
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
            conceal: None,
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
            // the extrapolated signal. The Bartlett ramp is carried in
            // Q15 and both legs land through one rounded L_mac
            // accumulation.
            let zl = if do_xfade && n < XFADE {
                let n_i = n as i32;
                let w_up = (n_i * Q15_ONE) / 79;
                let w_dn = Q15_ONE - w_up;
                round_fx(l_mac(l_mult(w_up, xl), w_dn, self.yl_extra[n]))
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
        self.conceal = None;
        out
    }

    /// Conceal one erased frame (clauses IV.6.1.2 / IV.6.1.3 /
    /// IV.6.2.2), returning the extrapolated 16-kHz PCM.
    pub fn conceal_erased_frame(&mut self) -> Vec<i16> {
        let l = self.l;
        let first_erased = !self.prev_erased;

        // ---- Lower band ----------------------------------------
        if first_erased {
            let analysis = self.analyse_lower();
            // Residual of the past signal through A(z) (eq IV-3) and
            // the modified repetition period (eqs IV-12 / IV-13) seed
            // the continuation state for this erasure run.
            let e_hist = self.residual_history(&analysis.a);
            let e_recent = build_repetition_period(&analysis, &e_hist);
            let mem = core::array::from_fn(|i| self.zl(-1 - i as isize));
            self.conceal = Some(ConcealState {
                e_recent,
                next_abs: 0,
                mem,
            });
            self.analysis = Some(analysis);
        }
        let analysis = self.analysis.clone().expect("analysis set above");

        // Extrapolate the residual and synthesise yl (muted), plus
        // the 80-sample cross-fade extension (clauses IV.6.1.2.5 –
        // IV.6.1.2.7 for the first erased frame; clause IV.6.1.3
        // continuation afterwards).
        let (yl, yl_extra) = self.synthesise_lower(&analysis, first_erased);

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
                // Clause IV.6.2.2.2: the higher band runs the
                // clause IV.6.1.2.7 muting on its own counter/factor
                // pair. It uses the threshold ("other cases")
                // schedule from the first erased frame on — during
                // the first 80 samples cnt_mute_hb is still below
                // every threshold, so the trajectory matches the
                // lower band's flat first-10-ms schedule, and the
                // vector scores pin this reading over a mirrored
                // first-10-ms special case.
                let g = self.mute_hb.step_general(&p);
                let muted = mult_r(g, sample);
                // uh = yh; vh = Hpost(uh); zh = vh (clause IV.6.2.3).
                let vh = self.hpost_step(muted);
                yh.push(vh);
                ring.push(vh);
                let _ = n;
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
    /// (1 - (3207/4096) z^-1), on the eq IV-19 constants held in Q13
    /// (`plc_tables::{B_HP_POST, A_HP_POST}`) through the basic-
    /// operator chain of [`plc_analysis::hpost_step`].
    fn hpost_step(&mut self, uh: i32) -> i32 {
        let (prev_in, prev_out) = self.hpost_mem;
        let vh = plc_analysis::hpost_step(uh, prev_in, prev_out);
        self.hpost_mem = (uh, vh);
        vh
    }

    // ------------------------------------------------------------
    // Lower-band analysis (clause IV.6.1.2, first erased frame only)
    // ------------------------------------------------------------

    fn analyse_lower(&self) -> Analysis {
        // --- LP analysis (clause IV.6.1.2.1) -------------------
        // Staged Q15 window + conditioned autocorrelation +
        // double-precision Levinson-Durbin (`plc_analysis`).
        let mut last80 = [0_i32; 80];
        for (k, slot) in last80.iter_mut().enumerate() {
            *slot = self.zl(k as isize - 80);
        }
        let a = lp_analysis(&last80);

        // --- Pre-processing (clause IV.6.1.2.2, eq IV-4) -------
        // zlpre(n), n = -288..-1 through Hpre(z) on the staged Q14
        // constants; memory starts at zero.
        let mut zlpre = [0_i32; 288];
        let mut prev_in = 0_i32;
        let mut prev_out = 0_i32;
        for (k, slot) in zlpre.iter_mut().enumerate() {
            let x = self.zl(k as isize - 288);
            let y = hpre_step(x, prev_in, prev_out);
            prev_in = x;
            prev_out = y;
            *slot = y;
        }

        // --- LTP analysis (clause IV.6.1.2.3) ------------------
        let (t0_raw, rmax) = ltp_analysis(&zlpre);

        // --- Classification (clause IV.6.1.2.4, Figure IV.4) ---
        let nbl = self.lower.log_scale_factor();
        let nbh = self.higher.log_scale_factor();
        let mut zhist = [0_i32; 81];
        for (k, slot) in zhist.iter_mut().enumerate() {
            *slot = self.zl(k as isize - 81);
        }
        let zcr = plc_analysis::zero_crossing_rate(&zhist);

        let mut t0 = t0_raw;
        let mut class = Class::WeaklyVoiced;
        if rmax > RMAX_VOICED_Q15 {
            class = Class::Voiced;
            if nbh > nbl {
                class = Class::WeaklyVoiced;
            }
        } else if nbh > nbl {
            class = Class::VuvTransition;
        }
        // The zcr test is applied to the running class regardless of
        // the earlier outcomes (the Figure IV.4 arrows rejoin the
        // spine): of the two defensible flowchart readings, this one
        // scores measurably closer to the reference PLC vectors
        // (tests/appendix_iv_plc.rs).
        if zcr >= 20 {
            class = Class::Unvoiced;
            // "if class is set to UNVOICED, the pitch delay T0 may be
            // modified to avoid artefacts due to low-pitch delay
            // values" (clause IV.6.1.2.4 / Figure IV.4).
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

    /// Residual history e(n), n = -289..-1 (eq IV-3), returned as a
    /// slice indexed by `[289 + n]`.
    fn residual_history(&self, a: &[i32; LP_ORDER]) -> Vec<i32> {
        let mut e = Vec::with_capacity(289);
        for n in -289..0_isize {
            e.push(residual_step(a, self.zl(n), |i| self.zl(n - i as isize)));
        }
        e
    }

    /// Number of large residual peaks in the last pitch period
    /// (eq IV-11).
    fn count_peaks(&self, e_hist: &[i32], t0: usize) -> u32 {
        let mut cnt = 0;
        for n in -(t0 as isize)..0 {
            let cur = crate::basicop::shr(crate::basicop::abs_s(e_hist[(289 + n) as usize]), 2);
            let mut prev_max = 0;
            for i in -2..=2_isize {
                let idx = 289 + n - t0 as isize + i;
                if (0..289).contains(&idx) {
                    prev_max = prev_max.max(crate::basicop::abs_s(e_hist[idx as usize]));
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

    /// Synthesise the lower band for one erased frame: extrapolate
    /// the residual (eqs IV-14 / IV-15, continuing across consecutive
    /// erasures per clause IV.6.1.3), run the eq IV-16 synthesis
    /// filter and apply the eq IV-17 adaptive muting. Returns
    /// (yl frame, yl cross-fade extra).
    ///
    /// For the first erased frame the whole span `yl(0 … L+79)` is
    /// newly generated; for every further erased frame `yl(0 … 79)` is
    /// the previous frame's extra synthesis and only the last `L`
    /// samples are newly generated — so the muting factor advances by
    /// exactly the number of newly synthesised samples, and the
    /// residual pattern, the jitter parity and the synthesis-filter
    /// memory all continue from the [`ConcealState`].
    fn synthesise_lower(
        &mut self,
        analysis: &Analysis,
        first_erased: bool,
    ) -> (Vec<i32>, [i32; XFADE]) {
        let l = self.l;
        let p = analysis.class.muting_params();

        let count = if first_erased { l + XFADE } else { l };
        let first10 = first_erased && l == 80;
        let mut fresh: Vec<i32> = Vec::with_capacity(count);
        let mut st = self.conceal.take().expect("conceal state seeded");
        for n in 0..count {
            // Residual continuation (eqs IV-14 / IV-15): reach back
            // T0 samples, ±1 alternating when the class is not
            // VOICED; `e_recent` ends at absolute index next_abs − 1
            // and always covers the reach-back window.
            let abs = st.next_abs as isize;
            let m = if analysis.class == Class::Voiced {
                abs - analysis.t0 as isize
            } else {
                let j = if abs % 2 == 0 { 1 } else { -1 };
                abs - analysis.t0 as isize + j
            };
            let base = st.next_abs as isize - st.e_recent.len() as isize;
            let e_n = st.e_recent[(m - base) as usize];
            st.e_recent.push(e_n);
            st.next_abs += 1;
            if st.e_recent.len() > analysis.t0 + 2 {
                let excess = st.e_recent.len() - (analysis.t0 + 2);
                st.e_recent.drain(..excess);
            }

            // Eq IV-16 synthesis on the continued muted-output
            // memory, then eq IV-17 muting through the rounding
            // multiply (`mult_r`, semantics doc §2.2).
            let ylpre = synthesis_step(&analysis.a, e_n, &st.mem);
            let g = if first10 {
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
            let yl_n = mult_r(g, ylpre);
            st.mem.rotate_right(1);
            st.mem[0] = yl_n;
            fresh.push(yl_n);
        }
        if first10 {
            self.mute_lb.cnt += 80 * p.inc_mute;
        }
        self.conceal = Some(st);

        let mut ex = [0_i32; XFADE];
        if first_erased {
            // fresh = yl(0 … L+79).
            ex.copy_from_slice(&fresh[l..]);
            fresh.truncate(l);
            (fresh, ex)
        } else {
            // fresh = yl(80 … L+79): the frame starts with the
            // previous extra synthesis (clause IV.6.1.3).
            let mut frame = self.yl_extra.to_vec();
            frame.extend_from_slice(&fresh[..l - XFADE]);
            ex.copy_from_slice(&fresh[l - XFADE..]);
            (frame, ex)
        }
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

/// The repetition period seeding an erasure run's residual
/// extrapolation: `e(−T0−1 … −1)` from the eq IV-3 residual history,
/// magnitude-limited per eqs IV-12 / IV-13 when the class is not
/// VOICED (each sample clamped to the largest magnitude in a ±2
/// neighbourhood one period earlier). `e_hist` is indexed `[289 + n]`
/// for `n = −289 … −1`.
fn build_repetition_period(analysis: &Analysis, e_hist: &[i32]) -> Vec<i32> {
    let t0 = analysis.t0;
    let mut rep: Vec<i32> = Vec::with_capacity(t0 + 1);
    rep.push(e_hist[289 - t0 - 1]);
    for n in -(t0 as isize)..0 {
        let raw = e_hist[(289 + n) as usize];
        let v = if analysis.class == Class::Voiced {
            raw
        } else {
            let mut prev_max = 0;
            for i in -2..=2_isize {
                let idx = 289 + n - t0 as isize + i;
                if (0..289).contains(&idx) {
                    prev_max = prev_max.max(crate::basicop::abs_s(e_hist[idx as usize]));
                }
            }
            let mag = crate::basicop::abs_s(raw).min(prev_max);
            if raw < 0 {
                -mag
            } else {
                mag
            }
        };
        rep.push(v);
    }
    rep
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

    /// Prime a decoder with a pseudo-random-speech-shaped stream and
    /// return it (shared by the continuation tests below).
    fn primed(frame_samples: usize) -> PlcDecoder {
        let mut enc = crate::Encoder::new();
        let pcm: Vec<i16> = (0..4800)
            .map(|i| {
                let t = i as f64 / 16_000.0;
                let v = (2.0 * core::f64::consts::PI * 220.0 * t).sin() * 7000.0
                    + (2.0 * core::f64::consts::PI * 1_760.0 * t).sin() * 2500.0;
                v as i16
            })
            .collect();
        let octets = enc.encode_pcm16(&pcm);
        let mut plc = PlcDecoder::new(Mode::Mode1, frame_samples);
        for f in octets.chunks(frame_samples / 2) {
            if f.len() == frame_samples / 2 {
                let _ = plc.decode_good_frame(f);
            }
        }
        plc
    }

    #[test]
    fn long_erasure_run_mutes_to_the_floor() {
        // Table IV.3 / clause IV.6.1.2.7: once cnt_mute reaches 320
        // the gain is forced to zero outright, so a long consecutive
        // erasure run must decay to (near-)silence through the
        // clause IV.6.1.3 continuation — the lower band to exactly
        // zero, the higher band down to the eq IV-19 post filter's
        // ±2 LSB rounding latch, QMF-scaled.
        for frame_samples in [160, 320] {
            let mut plc = primed(frame_samples);
            let mut last = Vec::new();
            for _ in 0..12 {
                last = plc.conceal_erased_frame();
            }
            let peak = last.iter().map(|&s| i32::from(s).abs()).max().unwrap();
            assert!(
                peak <= 8,
                "{frame_samples}-sample frames: erasure run not muted (peak {peak})"
            );
        }
    }

    #[test]
    fn consecutive_erasures_continue_without_frame_seams() {
        // Clause IV.6.1.3: each further erased frame starts with the
        // previous frame's extra synthesis, so the concealed signal
        // must be seamless across frame boundaries — the first-order
        // difference at each boundary must be no larger than the
        // biggest step inside the neighbouring frames (a restarted
        // extrapolation shows up as a boundary spike).
        let mut plc = primed(160);
        let mut all: Vec<i16> = Vec::new();
        for _ in 0..4 {
            all.extend(plc.conceal_erased_frame());
        }
        let diffs: Vec<i32> = all
            .windows(2)
            .map(|w| (i32::from(w[1]) - i32::from(w[0])).abs())
            .collect();
        for b in [160usize, 320, 480] {
            let seam = diffs[b - 1];
            // Local slope scale: the largest step in the 40 samples
            // on either side of the boundary, seam excluded.
            let local = diffs[b - 41..b - 1]
                .iter()
                .chain(diffs[b..b + 40].iter())
                .copied()
                .max()
                .unwrap()
                .max(1);
            assert!(
                seam <= 4 * local,
                "seam at boundary {b}: step {seam} vs local scale {local}"
            );
        }
    }
}

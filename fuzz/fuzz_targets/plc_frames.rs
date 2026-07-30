#![no_main]

//! Arbitrary good/erased frame sequences through the Appendix IV
//! packet-loss-concealment decoder.
//!
//! The PLC path (clauses IV.5 / IV.6 of the staged 2012
//! Recommendation) layers an analysis/synthesis pipeline — LP
//! analysis, LTP pitch search, classification, residual repetition,
//! adaptive muting, ADPCM state rewriting, cross-fade — on top of the
//! total base decoder, and every stage carries indexing over history
//! buffers whose bounds depend on fuzz-influenced quantities (the
//! pitch delay after the UNVOICED doubling and odd-forcing, the
//! consecutive-erasure continuation, the 10-ms vs 20-ms frame length).
//! The target drives both frame lengths with adversarial erasure
//! patterns and asserts, per frame:
//!
//! - exactly `2 L` output samples (the 16-kHz accounting of clause
//!   IV.4);
//! - totality: no panic from any history/pitch indexing, for any
//!   erasure pattern including an erasure as the very first frame
//!   (the reset-state extrapolation) and arbitrarily long erasure
//!   bursts (the muting floor);
//! - the mute-to-silence invariant: after 16 consecutive erased
//!   frames the concealment output must be muted down to (at most)
//!   the remove-DC filter's rounding floor (Table IV.3 drives every
//!   class's `g_mute` to zero well before 320 sub-band samples and 16
//!   frames is ≥ 1280; the eq IV-19 recursion's round-to-nearest can
//!   latch a ±1 LSB higher-band residue, so the bound is a few LSB,
//!   not exact zero).

use libfuzzer_sys::fuzz_target;
use oxideav_g722::{Mode, PlcDecoder};

const MODES: [Mode; 3] = [Mode::Mode1, Mode::Mode2, Mode::Mode3];

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, rest)) = data.split_first() else {
        return;
    };
    let frame_samples = if ctl & 1 == 0 { 160 } else { 320 };
    let octets_per_frame = frame_samples / 2;
    let mut plc = PlcDecoder::new(MODES[((ctl >> 1) % 3) as usize], frame_samples);
    assert_eq!(plc.frame_samples(), frame_samples);
    assert_eq!(plc.subband_frame_len(), octets_per_frame);

    // Each input frame consumes one pattern byte (erasure control)
    // plus the frame octets (zero-padded at the tail of the input).
    let mut it = rest.iter().copied();
    let mut erased_run = 0u32;
    for _ in 0..24 {
        let Some(pattern) = it.next() else {
            break;
        };
        // Bias towards erasures so bursts and recovery frames are both
        // common: 0..=127 good, 128..=255 erased.
        if pattern < 128 {
            let mut octets = vec![0u8; octets_per_frame];
            for slot in octets.iter_mut() {
                let Some(b) = it.next() else {
                    break;
                };
                *slot = b;
            }
            let out = plc.decode_good_frame(&octets);
            assert_eq!(out.len(), frame_samples, "good-frame sample accounting");
            erased_run = 0;
        } else {
            let out = plc.conceal_erased_frame();
            assert_eq!(out.len(), frame_samples, "erased-frame sample accounting");
            erased_run += 1;
            if erased_run >= 16 {
                assert!(
                    out.iter().all(|&s| s.abs() <= 4),
                    "concealment not muted after {erased_run} erased frames: {:?}",
                    out.iter().map(|s| s.abs()).max()
                );
            }
        }
    }
});

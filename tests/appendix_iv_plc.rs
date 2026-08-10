//! Appendix IV packet-loss-concealment vectors — characterisation
//! against the staged reference set
//! (`docs/audio/g722/conformance/appendix-IV-plc/`).
//!
//! # Container and bit mapping
//!
//! The `.bst` files are ITU-T G.192 bitstreams (one 16-bit LE word per
//! payload bit, referenced normatively by clause IV.2 of the staged
//! 2012 Recommendation): each frame is a sync word (`0x6B21` = good
//! frame, `0x6B20` = erased frame), a length word (640 bits for 10-ms
//! frames, 1280 for 20-ms), then one word per bit (`0x0081` = 1,
//! `0x007F` = 0; erased frames carry `0x0000` filler). These constants
//! were confirmed empirically against the staged data (the frame
//! arithmetic closes exactly: `test10.bst` = 1257 × (2 + 640) words for
//! 1257 × 160 output samples).
//!
//! The bit-to-codeword mapping is carried by the reference software
//! ("the mapping table of the encoded bit stream is contained in the
//! simulation software", clause IV.7.1) and is **not** printed in the
//! Recommendation; it was recovered *black-box* from the staged
//! vectors themselves by decoding the erasure-free prefix under
//! candidate layouts until one reproduced the reference PCM exactly
//! (no reference source was consulted). The stream layout is
//! **bit-plane-major**: the 8L bits of a frame are eight consecutive
//! planes of L/8 bits in the order `IL4 IL3 IL2 IL1 IH2 IH1 IL5 IL6`
//! (core bits first, the two mode-2/3-droppable enhancement LSBs
//! last). Under this mapping the reference PCM is **bit-exact** over
//! every erasure-free prefix (verified below), which pins both the
//! mapping and the output convention (the crate's `decode_pcm16`
//! 16-bit convention, as with the G.191 corpus).
//!
//! # Comparison status
//!
//! The PLC runs entirely in fixed point on the staged Appendix IV
//! numeric tables (`docs/audio/g722/tables/appendix-IV-*`) and the
//! staged ITU-T G.191 STL basic-operator semantics
//! (`docs/audio/g722/basic-operators/`); realisation choices the
//! staged material leaves open (per-term vs per-sum rounding sites,
//! muting-schedule readings) were calibrated black-box against these
//! vectors. The result is:
//!
//! * bit-exact on every erasure-free prefix (6 880 / 4 160 samples),
//! * bit-exact re-convergence on sufficiently separated good stretches
//!   (tens of frames beyond the prefix are exact; ≈ 32–39 % of all
//!   samples match the reference bit for bit across the three files),
//! * waveform-accurate concealment elsewhere (the first concealed
//!   frame tracks the reference at zero lag; overall SNR vs the
//!   reference output ≈ 13–14 dB per file).
//!
//! # Residual divergence, characterised
//!
//! The dominant remaining divergence is the clause IV.6.1.2.3
//! "procedure favouring the smaller pitch values", which no ITU
//! document specifies (see
//! `docs/audio/g722/appendix-IV-ltp-smaller-pitch-gap.md`; the
//! reference C is the only holder of the rule and is barred). This
//! implementation uses the plain eq (IV-7) arg max, keeping the
//! smaller lag on ties. Measured against the gap note's §8
//! ground-truth pitch table (the 18 confidently-periodic erasures of
//! `test10.bst`), the plain arg max reproduces the reference `T0` on
//! 13 of 18; four of the five misses select a pitch **multiple**
//! (frame 91: 66 vs 34, frame 110: 92 vs 30, frame 188: 141 vs 34,
//! frame 256: 122 vs 40) — precisely the case the unspecified rule
//! exists to prevent — and the fifth (frame 570: 83 vs 82) is a
//! one-lag refinement tie. Secondary divergence sources are the
//! unstaged instruction sequences of the reference realisation (the
//! autocorrelation scaling schedule, the reflection-coefficient
//! division, rounding-site placement), which the fixed-point rebuild
//! approximates on the staged operator set; the floors asserted here
//! pin the achieved level against regression.

use std::path::PathBuf;

use oxideav_g722::{Mode, PlcDecoder};

fn plc_dir() -> PathBuf {
    PathBuf::from("../../docs/audio/g722/conformance/appendix-IV-plc")
}

fn read_words(name: &str) -> Option<Vec<u16>> {
    let path = plc_dir().join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(
            bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
        ),
        Err(_) => {
            eprintln!("skip: {} not in this checkout", path.display());
            None
        }
    }
}

/// G.192 good-frame sync word.
const G192_SYNC_GOOD: u16 = 0x6B21;
/// G.192 erased-frame sync word.
const G192_SYNC_BAD: u16 = 0x6B20;

struct G192Frame {
    good: bool,
    bits: Vec<u8>,
}

fn parse_g192(words: &[u16]) -> Vec<G192Frame> {
    let mut frames = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let sync = words[i];
        assert!(
            sync == G192_SYNC_GOOD || sync == G192_SYNC_BAD,
            "bad sync {sync:#x} at word {i}"
        );
        let len = words[i + 1] as usize;
        assert!(i + 2 + len <= words.len(), "truncated G.192 frame at {i}");
        let bits = words[i + 2..i + 2 + len]
            .iter()
            .map(|&w| u8::from(w == 0x0081))
            .collect();
        frames.push(G192Frame {
            good: sync == G192_SYNC_GOOD,
            bits,
        });
        i += 2 + len;
    }
    frames
}

/// Rebuild the per-sample G.722 octets of one good G.192 frame under
/// the empirically recovered plane-major layout (see module docs):
/// stream plane k carries octet bit `PLANE_TO_BIT[k]` for all L/8
/// codewords, planes ordered `IL4 IL3 IL2 IL1 IH2 IH1 IL5 IL6`.
fn frame_octets(bits: &[u8]) -> Vec<u8> {
    let n = bits.len() / 8;
    const PLANE_TO_BIT: [u8; 8] = [2, 3, 4, 5, 6, 7, 1, 0];
    (0..n)
        .map(|j| {
            let mut o = 0u8;
            for (plane, &bit) in PLANE_TO_BIT.iter().enumerate() {
                o |= bits[plane * n + j] << bit;
            }
            o
        })
        .collect()
}

/// Per-file structural expectations and characterisation floors.
struct VectorCase {
    bst: &'static str,
    out: &'static str,
    /// 16-kHz samples per frame (the out-of-band `-fsize` parameter).
    frame_samples: usize,
    frames: usize,
    bad_frames: usize,
    /// Erasure-free prefix length in samples (bit-exact requirement).
    exact_prefix: usize,
    /// Regression floors for the fixed-point implementation (staged
    /// tables + basic operators; measured 63 965 / 77 654 / 44 557
    /// exact samples and 14.3 / 14.2 / 12.7 dB at calibration time).
    min_exact_samples: usize,
    min_exact_frames: usize,
    min_snr_db: f64,
}

const CASES: [VectorCase; 3] = [
    VectorCase {
        bst: "test10.bst",
        out: "test10.out",
        frame_samples: 160,
        frames: 1257,
        bad_frames: 136,
        exact_prefix: 6880,
        min_exact_samples: 63_500,
        min_exact_frames: 44,
        min_snr_db: 14.0,
    },
    VectorCase {
        bst: "test20.bst",
        out: "test20.out",
        frame_samples: 320,
        frames: 628,
        bad_frames: 65,
        exact_prefix: 4160,
        min_exact_samples: 77_000,
        min_exact_frames: 13,
        min_snr_db: 14.0,
    },
    VectorCase {
        bst: "ovfl.bst",
        out: "ovfl.out",
        frame_samples: 320,
        frames: 401,
        bad_frames: 37,
        exact_prefix: 0,
        min_exact_samples: 44_000,
        min_exact_frames: 1,
        min_snr_db: 12.5,
    },
];

#[test]
fn g192_container_structure_matches_the_staged_readme() {
    for case in &CASES {
        let Some(words) = read_words(case.bst) else {
            return;
        };
        let frames = parse_g192(&words);
        assert_eq!(frames.len(), case.frames, "{}: frame count", case.bst);
        let bad = frames.iter().filter(|f| !f.good).count();
        assert_eq!(bad, case.bad_frames, "{}: erased-frame count", case.bst);
        for (i, f) in frames.iter().enumerate() {
            assert_eq!(
                f.bits.len(),
                case.frame_samples * 4,
                "{}: frame {i} bit count (64 kbit/s)",
                case.bst
            );
        }
        let out = read_words(case.out).unwrap();
        assert_eq!(
            out.len(),
            case.frames * case.frame_samples,
            "{}: output sample count",
            case.out
        );
    }
}

#[test]
fn erasure_free_prefixes_decode_bit_exactly() {
    // Pins the recovered G.192 bit mapping and the output convention:
    // before the first erasure the PLC decoder is the plain decoder,
    // and the reference must match sample for sample.
    for case in &CASES {
        if case.exact_prefix == 0 {
            continue;
        }
        let (Some(bst), Some(out)) = (read_words(case.bst), read_words(case.out)) else {
            return;
        };
        let reference: Vec<i16> = out.iter().map(|&w| w as i16).collect();
        let frames = parse_g192(&bst);
        let first_bad = frames.iter().position(|f| !f.good).unwrap();
        assert_eq!(
            first_bad * case.frame_samples,
            case.exact_prefix,
            "{}: prefix length",
            case.bst
        );
        let mut plc = PlcDecoder::new(Mode::Mode1, case.frame_samples);
        let mut ours: Vec<i16> = Vec::new();
        for f in &frames[..first_bad] {
            ours.extend(plc.decode_good_frame(&frame_octets(&f.bits)));
        }
        if let Some(i) = (0..ours.len()).find(|&i| ours[i] != reference[i]) {
            panic!(
                "{}: prefix diverges at sample {i}: ours {} vs reference {}",
                case.bst, ours[i], reference[i]
            );
        }
    }
}

#[test]
fn plc_characterisation_floors_hold_on_all_three_vectors() {
    for case in &CASES {
        let (Some(bst), Some(out)) = (read_words(case.bst), read_words(case.out)) else {
            return;
        };
        let reference: Vec<i16> = out.iter().map(|&w| w as i16).collect();
        let frames = parse_g192(&bst);
        let mut plc = PlcDecoder::new(Mode::Mode1, case.frame_samples);
        let mut ours: Vec<i16> = Vec::new();
        for f in &frames {
            if f.good {
                ours.extend(plc.decode_good_frame(&frame_octets(&f.bits)));
            } else {
                ours.extend(plc.conceal_erased_frame());
            }
        }
        assert_eq!(ours.len(), reference.len(), "{}: length", case.bst);

        let exact = ours
            .iter()
            .zip(reference.iter())
            .filter(|(a, b)| a == b)
            .count();
        let frames_exact = ours
            .chunks(case.frame_samples)
            .zip(reference.chunks(case.frame_samples))
            .filter(|(a, b)| a == b)
            .count();
        let err_energy: f64 = ours
            .iter()
            .zip(reference.iter())
            .map(|(a, b)| {
                let d = f64::from(*a) - f64::from(*b);
                d * d
            })
            .sum();
        let sig_energy: f64 = reference.iter().map(|&b| f64::from(b) * f64::from(b)).sum();
        let snr = 10.0 * (sig_energy / err_energy.max(1.0)).log10();
        eprintln!(
            "{}: {exact}/{} samples exact; {frames_exact}/{} frames exact; SNR {snr:.1} dB",
            case.bst,
            ours.len(),
            frames.len(),
        );

        // The erasure-free prefix must stay bit-exact inside the full
        // run as well.
        assert!(
            ours[..case.exact_prefix] == reference[..case.exact_prefix],
            "{}: erasure-free prefix regressed",
            case.bst
        );
        assert!(
            exact >= case.min_exact_samples,
            "{}: exact samples {exact} < floor {}",
            case.bst,
            case.min_exact_samples
        );
        assert!(
            frames_exact >= case.min_exact_frames,
            "{}: exact frames {frames_exact} < floor {}",
            case.bst,
            case.min_exact_frames
        );
        assert!(
            snr >= case.min_snr_db,
            "{}: SNR {snr:.2} dB < floor {}",
            case.bst,
            case.min_snr_db
        );
    }
}

#[test]
fn concealment_tracks_the_reference_at_zero_lag() {
    // Waveform-level sanity on the first concealed frame of test10:
    // the extrapolation must be time-aligned with the reference
    // concealment (a pitch/class mismatch would maximise correlation
    // at a non-zero lag or destroy it altogether).
    let (Some(bst), Some(out)) = (read_words("test10.bst"), read_words("test10.out")) else {
        return;
    };
    let reference: Vec<i16> = out.iter().map(|&w| w as i16).collect();
    let frames = parse_g192(&bst);
    let first_bad = frames.iter().position(|f| !f.good).unwrap();
    let mut plc = PlcDecoder::new(Mode::Mode1, 160);
    let mut ours: Vec<i16> = Vec::new();
    for f in &frames[..=first_bad] {
        if f.good {
            ours.extend(plc.decode_good_frame(&frame_octets(&f.bits)));
        } else {
            ours.extend(plc.conceal_erased_frame());
        }
    }
    let lo = first_bad * 160;
    let r = &reference[lo..lo + 160];
    let o = &ours[lo..lo + 160];
    let corr_at = |lag: i32| -> f64 {
        let mut num = 0f64;
        let mut d1 = 0f64;
        let mut d2 = 0f64;
        for i in 0..160i32 {
            let j = i + lag;
            if (0..160).contains(&j) {
                let a = f64::from(r[i as usize]);
                let b = f64::from(o[j as usize]);
                num += a * b;
                d1 += a * a;
                d2 += b * b;
            }
        }
        num / (d1.sqrt() * d2.sqrt()).max(1.0)
    };
    let c0 = corr_at(0);
    assert!(c0 > 0.75, "zero-lag correlation {c0:.3} too low");
    for lag in [-4, -3, -2, -1, 1, 2, 3, 4] {
        let c = corr_at(lag);
        assert!(
            c < c0,
            "lag {lag} correlation {c:.3} beats zero-lag {c0:.3}"
        );
    }
}

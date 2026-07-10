//! Totality / robustness tests for the bitstream-facing entry points.
//!
//! The conformance suites (`conformance`, `test_harness`) pin the
//! codec against Recommendation-derived golden vectors — well-formed
//! inputs by construction. This module drives the *public API
//! surface* with adversarial inputs instead: arbitrary octet streams
//! into the decoder (every `u8` is a syntactically valid G.722 octet
//! per the clause 1.4.4 multiplexer layout, so the decoder must be
//! total over them), PCM far outside the documented 14-bit input
//! domain into the encoder, arbitrary raw codeword bytes into the
//! Appendix-II QMF-bypass entry points, and adversarial interleavings
//! of `set_mode` / `reset` (clause 1.3 page 3 permits mode switching
//! "in any octet during the transmission"). Every test asserts the
//! spec-side output invariants — the LIMIT / Table 9 saturation
//! ranges — rather than merely "no panic".
//!
//! The pseudo-random driver is a self-contained xorshift walk so the
//! streams are deterministic across runs and platforms.

use crate::{Decoder, Encoder, Mode};

extern crate alloc;
use alloc::vec::Vec;

/// Deterministic xorshift64* pseudo-random stream.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }

    fn next_i32(&mut self) -> i32 {
        (self.next_u64() >> 32) as u32 as i32
    }
}

/// Receive-QMF output range: the decoder's PCM output saturates to
/// the 15-bit two's-complement window of Table 9/G.722 (page 25).
const OUT_RANGE: core::ops::RangeInclusive<i32> = -16384..=16383;

const ALL_MODES: [Mode; 3] = [Mode::Mode1, Mode::Mode2, Mode::Mode3];

#[test]
fn decoder_is_total_over_arbitrary_octet_streams() {
    // 8192 pseudo-random octets per mode. Every u8 is a valid
    // multiplexer octet (2-bit I_H + 6-bit I_L, clause 1.4.4), so
    // decode must produce exactly two in-range samples per octet no
    // matter the sequence.
    for (i, mode) in ALL_MODES.into_iter().enumerate() {
        let mut rng = Rng::new(0xD0DE_C0DE + i as u64);
        let octets: Vec<u8> = (0..8192).map(|_| rng.next_u8()).collect();
        let mut dec = Decoder::new(mode);
        let out = dec.decode(&octets);
        assert_eq!(out.len(), octets.len() * 2);
        for (n, &s) in out.iter().enumerate() {
            assert!(
                OUT_RANGE.contains(&s),
                "{mode:?}: sample {n} = {s} escaped the Table 9 output range"
            );
        }
    }
}

#[test]
fn decoder_survives_adversarial_mode_switch_and_reset_interleaving() {
    // Clause 1.3 (page 3): the decoder variant "can be changed in any
    // octet during the transmission". Drive 16384 steps where every
    // step decodes one arbitrary octet and randomly reconfigures /
    // resets the decoder, and assert the output invariants never
    // break. This exercises the mode-dependent INVQBL ladder against
    // predictor state built up under a *different* mode — the
    // switching case the per-mode golden vectors cannot reach.
    let mut rng = Rng::new(0x5EED_0001);
    let mut dec = Decoder::new(Mode::Mode1);
    for n in 0..16384_u32 {
        let (a, b) = dec.decode_octet(rng.next_u8());
        assert!(
            OUT_RANGE.contains(&a) && OUT_RANGE.contains(&b),
            "step {n}: ({a}, {b}) escaped the Table 9 output range"
        );
        match rng.next_u64() % 16 {
            0 => dec.set_mode(Mode::Mode1),
            1 => dec.set_mode(Mode::Mode2),
            2 => dec.set_mode(Mode::Mode3),
            3 => dec.reset(),
            _ => {}
        }
    }
}

#[test]
fn decode_subband_pair_is_total_over_all_raw_codeword_bytes() {
    // The Appendix-II Configuration-2 bypass takes raw codeword
    // bytes; the decoder masks them to the 6-bit / 2-bit fields, so
    // all 256 × 256 raw byte pairs must be accepted. Sweep every
    // pair once against evolving state, in every mode, and assert
    // the LIMIT ranges of §§ 6.2.1.6 / 6.2.2.5.
    for mode in ALL_MODES {
        let mut dec = Decoder::new(mode);
        for i_lr in 0..=255_u8 {
            for i_h in 0..=255_u8 {
                let (rl, rh) = dec.decode_subband_pair(i_lr, i_h);
                assert!(
                    OUT_RANGE.contains(&rl) && OUT_RANGE.contains(&rh),
                    "{mode:?}: ({i_lr:#04x}, {i_h:#04x}) → ({rl}, {rh}) escaped LIMIT"
                );
            }
        }
    }
}

#[test]
fn encoder_is_total_over_full_range_pcm() {
    // The encoder documents a 14-bit input domain, but the API takes
    // i32 — it must stay total (and deterministic) over the whole
    // type. 8192 full-range pseudo-random samples: one octet per
    // sample pair, and the round-trip decode stays inside Table 9.
    let mut rng = Rng::new(0xE4C0_DE00);
    let pcm: Vec<i32> = (0..8192).map(|_| rng.next_i32()).collect();
    let mut enc = Encoder::new();
    let octets = enc.encode(&pcm);
    assert_eq!(octets.len(), pcm.len() / 2);
    // Deterministic: a fresh encoder over the same input produces the
    // identical stream.
    let mut enc2 = Encoder::new();
    assert_eq!(enc2.encode(&pcm), octets);
    for mode in ALL_MODES {
        let mut dec = Decoder::new(mode);
        let out = dec.decode(&octets);
        assert!(out.iter().all(|s| OUT_RANGE.contains(s)));
    }
}

#[test]
fn encoder_qmf_saturates_extreme_input_without_sign_flip() {
    // Regression for the transmit-QMF output clamp: saturation must
    // happen on the full-width accumulator *before* narrowing.
    // A constant maximal-positive input must therefore decode to a
    // steady *positive* rail — if the accumulator wrapped through a
    // narrowing cast first (the pre-fix behaviour), the lower
    // sub-band alternated ±full-scale and the decoded steady state
    // flipped sign every sample.
    for (input, expect_positive) in [(i32::MAX, true), (i32::MIN, false)] {
        let mut enc = Encoder::new();
        let mut dec = Decoder::new(Mode::Mode1);
        let pcm = alloc::vec![input; 512];
        let out = dec.decode(&enc.encode(&pcm));
        // Skip the QMF + scale-factor ramp-up; the plateau must be
        // deep into the correct half of the output range.
        for (n, &s) in out[128..].iter().enumerate() {
            assert!(OUT_RANGE.contains(&s));
            if expect_positive {
                assert!(
                    s > 8192,
                    "sample {n} = {s}: positive-rail input decoded below half-scale"
                );
            } else {
                assert!(
                    s < -8192,
                    "sample {n} = {s}: negative-rail input decoded above negative half-scale"
                );
            }
        }
    }
}

#[test]
fn encode_subband_pair_is_total_over_extreme_subband_inputs() {
    // The Configuration-1 bypass documents a 15-bit input domain
    // (Table II-1), but the saturating spec operators (clause 5.2
    // prelude) make the ADPCM loops well-defined for any i32 — the
    // difference signal clamps to the 16-bit rails before the
    // quantizer sees it. Drive the rails, the ±15-bit boundary, and
    // full-range noise; every emitted octet is trivially valid, so
    // assert the local-decoder state keeps producing octets and the
    // stream is deterministic.
    let extremes = [i32::MAX, i32::MIN, 16384, -16385, 32767, -32768, 0, 1, -1];
    let mut enc = Encoder::new();
    let mut first = Vec::new();
    for &xl in &extremes {
        for &xh in &extremes {
            first.push(enc.encode_subband_pair(xl, xh));
        }
    }
    let mut rng = Rng::new(0xAB0C_ADE5);
    for _ in 0..4096 {
        first.push(enc.encode_subband_pair(rng.next_i32(), rng.next_i32()));
    }
    // Determinism across a fresh encoder.
    let mut enc2 = Encoder::new();
    let mut second = Vec::new();
    for &xl in &extremes {
        for &xh in &extremes {
            second.push(enc2.encode_subband_pair(xl, xh));
        }
    }
    let mut rng = Rng::new(0xAB0C_ADE5);
    for _ in 0..4096 {
        second.push(enc2.encode_subband_pair(rng.next_i32(), rng.next_i32()));
    }
    assert_eq!(first, second);
}

#[test]
fn encoder_chunked_encoding_matches_one_shot() {
    // The odd-length `pending` buffering of Encoder::encode must be
    // invisible on the wire: any chunking of the same PCM stream
    // (including empty and single-sample chunks) yields the identical
    // octet sequence.
    let mut rng = Rng::new(0xC407_0001);
    let pcm: Vec<i32> = (0..4097).map(|_| rng.next_i32() >> 18).collect();

    let mut whole_enc = Encoder::new();
    let whole = whole_enc.encode(&pcm);

    let mut chunked_enc = Encoder::new();
    let mut chunked = Vec::new();
    let mut pos = 0;
    while pos < pcm.len() {
        let take = (rng.next_u64() % 7) as usize; // 0..=6 samples
        let end = (pos + take).min(pcm.len());
        chunked_enc.encode_into(&pcm[pos..end], &mut chunked);
        pos = end;
    }
    assert_eq!(chunked, whole);
    // 4097 samples: exactly one sample must still be pending.
    assert_eq!(chunked_enc.pending_samples(), 1);
    assert_eq!(whole_enc.pending_samples(), 1);
    // Draining one more sample flushes the pair identically.
    let tail_whole = whole_enc.encode(&[42]);
    let tail_chunked = chunked_enc.encode(&[42]);
    assert_eq!(tail_whole, tail_chunked);
    assert_eq!(tail_whole.len(), 1);
    assert_eq!(whole_enc.pending_samples(), 0);
}

#[test]
fn encoder_reset_returns_to_the_fresh_stream() {
    // After arbitrary full-range abuse, reset() must restore the
    // exact post-reset condition of clauses 3.5 / 3.6: the stream
    // after a reset is bit-identical to a fresh encoder's.
    let mut rng = Rng::new(0x0BAD_F00D);
    let noise: Vec<i32> = (0..2048).map(|_| rng.next_i32()).collect();
    let probe: Vec<i32> = (0..2048).map(|_| rng.next_i32() >> 17).collect();

    let mut abused = Encoder::new();
    let _ = abused.encode(&noise);
    abused.reset();
    let after_reset = abused.encode(&probe);

    let mut fresh = Encoder::new();
    let fresh_stream = fresh.encode(&probe);
    assert_eq!(after_reset, fresh_stream);
}

// ---------------------------------------------------------------------
// 16-bit PCM convention entry points (encode_pcm16 / decode_pcm16).
// ---------------------------------------------------------------------

/// The 16-bit-convention decoder output saturates to the full 16-bit
/// two's-complement window (the `WD >> 11` output stage of clause
/// 5.2.2 under the Note 2 rescaling; see `Decoder::decode_octet_pcm16`).
const OUT_RANGE_PCM16: core::ops::RangeInclusive<i16> = i16::MIN..=i16::MAX;

#[test]
fn decoder_pcm16_is_total_over_arbitrary_octet_streams() {
    // Same totality contract as the 15-bit path: every u8 is a valid
    // multiplexer octet, so decode_pcm16 must be total and its output
    // confined to the (trivially full, but pinned) i16 window, with
    // deterministic replay.
    for mode in ALL_MODES {
        let mut rng = Rng::new(0x1600_D1CE);
        let stream: Vec<u8> = (0..8192).map(|_| rng.next_u8()).collect();
        let mut dec_a = Decoder::new(mode);
        let mut dec_b = Decoder::new(mode);
        let out_a = dec_a.decode_pcm16(&stream);
        let out_b = dec_b.decode_pcm16(&stream);
        assert_eq!(out_a, out_b, "{mode:?}: decode_pcm16 not deterministic");
        assert_eq!(out_a.len(), stream.len() * 2);
        for &s in &out_a {
            assert!(OUT_RANGE_PCM16.contains(&s));
        }
    }
}

#[test]
fn pcm16_decode_shares_the_subband_state_machine_with_the_native_path() {
    // The pcm16 output stage must be a pure output-domain rescaling of
    // the same sub-band decode: on the same arbitrary octet stream the
    // two paths' outputs stay within one fine LSB of the 2x relation
    // (the extra bit is true accumulator content) at every sample, in
    // every mode.
    for mode in ALL_MODES {
        let mut rng = Rng::new(0xD0C1_6B17_u64 ^ 0x5EED);
        let stream: Vec<u8> = (0..4096).map(|_| rng.next_u8()).collect();
        let mut d15 = Decoder::new(mode);
        let mut d16 = Decoder::new(mode);
        let coarse = d15.decode(&stream);
        let fine = d16.decode_pcm16(&stream);
        for (i, (c, f)) in coarse.iter().zip(fine.iter()).enumerate() {
            let twice = *c * 2;
            let diff = i32::from(*f) - twice;
            assert!(
                (-1..=1).contains(&diff),
                "{mode:?} sample {i}: fine {f} vs 2x coarse {twice}"
            );
        }
    }
}

#[test]
fn encoder_pcm16_is_total_over_full_range_pcm() {
    // encode_pcm16 must be total over the entire i16 domain (its
    // documented input domain — no out-of-domain values exist) and
    // emit syntactically valid octets whose decode stays in range.
    let mut rng = Rng::new(0xE16C_0DE5);
    let pcm: Vec<i16> = (0..8192).map(|_| (rng.next_u64() >> 48) as i16).collect();
    let mut enc = Encoder::new();
    let octets = enc.encode_pcm16(&pcm);
    assert_eq!(octets.len(), pcm.len() / 2);
    let mut dec = Decoder::new(Mode::Mode1);
    for &s in &dec.decode_pcm16(&octets) {
        assert!(OUT_RANGE_PCM16.contains(&s));
    }
    // Extremes: constant rails must not wrap anywhere in the loop.
    let mut enc = Encoder::new();
    let rails: Vec<i16> = core::iter::repeat_n(i16::MIN, 256)
        .chain(core::iter::repeat_n(i16::MAX, 256))
        .collect();
    let octets = enc.encode_pcm16(&rails);
    let mut dec = Decoder::new(Mode::Mode1);
    let out = dec.decode_pcm16(&octets);
    // The decoded rail regions must land near the rails with correct
    // polarity once the adaptive loop has charged (structural sanity
    // that no sign flip / wraparound occurred in the widened path).
    let head = &out[128..240];
    let tail = &out[320..];
    assert!(head.iter().all(|&v| v < 0), "negative rail lost polarity");
    assert!(
        tail.iter().any(|&v| v > 8192),
        "positive rail never tracked"
    );
}

#[test]
fn encoder_pcm16_chunking_is_stream_transparent() {
    // Odd-length chunked pcm16 feeding equals one-shot encoding: the
    // pending-sample buffer is shared with the 15-bit path and must be
    // convention-transparent within a single-convention stream.
    let mut rng = Rng::new(0xC41F_EED5);
    let pcm: Vec<i16> = (0..4097).map(|_| (rng.next_u64() >> 48) as i16).collect();

    let mut whole_enc = Encoder::new();
    let whole = whole_enc.encode_pcm16(&pcm);

    let mut chunked_enc = Encoder::new();
    let mut chunked = Vec::new();
    let mut rest = pcm.as_slice();
    let mut n = 1;
    while !rest.is_empty() {
        let take = n.min(rest.len());
        chunked_enc.encode_pcm16_into(&rest[..take], &mut chunked);
        rest = &rest[take..];
        n = (n * 3 + 1) % 17 + 1;
    }
    assert_eq!(chunked, whole);
    assert_eq!(chunked_enc.pending_samples(), 1);
    assert_eq!(whole_enc.pending_samples(), 1);
}

#[test]
fn encoder_pcm16_reset_returns_to_the_fresh_stream() {
    let mut rng = Rng::new(0x0BAD_F00D);
    let noise: Vec<i16> = (0..2048).map(|_| (rng.next_u64() >> 48) as i16).collect();
    let probe: Vec<i16> = (0..2048).map(|_| (rng.next_u64() >> 50) as i16).collect();

    let mut abused = Encoder::new();
    let _ = abused.encode_pcm16(&noise);
    abused.reset();
    let after_reset = abused.encode_pcm16(&probe);

    let mut fresh = Encoder::new();
    let fresh_stream = fresh.encode_pcm16(&probe);
    assert_eq!(after_reset, fresh_stream);
}

//! Bit-exact conformance against the ITU-T G.191 G.722 test-vector
//! corpus staged at `docs/audio/g722/conformance/` (workspace umbrella
//! checkout; see that directory's `README.md` for provenance).
//!
//! # Corpus contents and container framing
//!
//! Six raw, header-less payloads (all little-endian):
//!
//! * `inpsp.bin` — encoder input: 97 536 × 16-bit LE PCM samples at
//!   16 kHz (195 072 bytes).
//! * `codspw.cod` — the reference 64 kbit/s bitstream in a
//!   *word-per-octet* container: each G.722 octet is zero-extended
//!   into one 16-bit LE word (high byte always `0x00`; asserted
//!   below). 48 768 octets.
//! * `outsp1.bin` / `outsp2.bin` / `outsp3.bin` — reference decoder
//!   output PCM for Mode 1 / 2 / 3 (16-bit LE, 97 536 samples each),
//!   produced by decoding the `codspw.cod` codewords.
//! * `codsp.cod` — a byte-packed `.cod` file of the same length
//!   (48 768 octets, one per byte). **Empirical finding:** its
//!   payload is *not* the same codeword stream as `codspw.cod` — the
//!   two agree only over the leading silence (first 36 octets) and
//!   then diverge in both the `I_L` and `I_H` fields, and no
//!   input-scaling / QMF-phase / rounding variant of the encoder
//!   reproduces it (best alignment of its decode against `inpsp.bin`
//!   is ≈4.6 dB at a 23-sample delay, versus bit-exactness for
//!   `codspw.cod`). It is therefore *not* used as an oracle here; the
//!   staging README's claim that the two files carry identical
//!   codewords appears to be wrong.
//!
//! # Sample-word convention (why `*_pcm16`)
//!
//! Table 9/G.722 (clause 5.1, p. 25 of the staged 11/88 PDF) defines
//! the codec's native XIN/XOUT wire format as a sign-extended 15-bit
//! word — "the most significant magnitude bit ... appears at the
//! third bit location", "the LSB is set to 0 for 14-bit converters".
//! The corpus instead uses ordinary full-scale 16-bit PCM: `inpsp.bin`
//! contains odd sample values and values above +16383, and the
//! `outspN.bin` references contain odd values, so neither side can be
//! the 15-bit format. The Recommendation explicitly permits this: the
//! ACCUMA/ACCUMB and ACCUMC/ACCUMD Note 2 (clauses 5.2.1 / 5.2.2)
//! allows the QMF operands to be shifted "if so desired" provided the
//! result is rescaled accordingly, and the clause 5.2 prelude leaves
//! the accumulation word length free (≥ 24 bits). Treating the 16-bit
//! samples as `2 × XIN` and rescaling the analysis bank by one extra
//! bit (`>> 14`), and symmetrically emitting `WD >> 11` from the
//! synthesis bank, is exactly that freedom — and is **not** the same
//! as pre-shifting the input (`x >> 1`) or post-shifting the output
//! (`x << 1`), because the extra bit participates in all 24 filter
//! products. With that convention (the crate's `encode_pcm16` /
//! `decode_pcm16` entry points) this implementation reproduces the
//! corpus **bit-exactly**: 48 768 / 48 768 encoder octets and
//! 97 536 / 97 536 decoder samples in each of the three modes. Every
//! other scaling that was tried (input `>>1`/`>>2`, truncating or
//! rounding halving, 15-bit QMF, output `<<1`/`<<2`) diverges within
//! the first ~1 000 octets of signal.
//!
//! The ADPCM loops themselves are the plain clause 6.2 integer
//! arithmetic — truncating `*` operator, saturating `+`/`-` (the
//! UPPOL2 `0 − WD1` negation *must* saturate `-32768 → +32767`, which
//! the corpus genuinely exercises), no rounding anywhere.
//!
//! # Committed prefix fixtures vs. docs-gated full corpus
//!
//! `oxideav-g722` is its own repository; `docs/` exists only in the
//! umbrella checkout. Following the sibling-crate convention, the
//! full-corpus tests locate `../../docs/audio/g722/conformance/` and
//! skip gracefully (logging) when absent, so standalone CI stays
//! meaningful via committed *prefix excerpts* under `tests/data/`:
//! the first 4 096 octets / 8 192 samples of each leg (~68 KiB total,
//! covering the leading silence, the speech onset and the first
//! voiced stretch). Prefix comparison is sound because both
//! directions are causal, streaming, octet-aligned state machines: an
//! output octet/sample pair depends only on earlier input.
//!
//! Corpus semantics per `docs/audio/g722/conformance/README.md`:
//! encode `inpsp.bin` → `codspw.cod`; decode `codspw.cod` in mode N →
//! `outspN.bin`.

use std::path::PathBuf;

use oxideav_g722::{Decoder, Encoder, Mode};

/// Number of octets in the committed prefix fixtures (= 8 192 PCM
/// samples at two samples per octet).
const PREFIX_OCTETS: usize = 4096;

const INPSP_PREFIX: &[u8] = include_bytes!("data/inpsp-prefix.pcm");
const CODSPW_PREFIX: &[u8] = include_bytes!("data/codspw-prefix.g722");
const OUTSP1_PREFIX: &[u8] = include_bytes!("data/outsp1-prefix.pcm");
const OUTSP2_PREFIX: &[u8] = include_bytes!("data/outsp2-prefix.pcm");
const OUTSP3_PREFIX: &[u8] = include_bytes!("data/outsp3-prefix.pcm");

fn pcm16_le(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// First index at which two slices differ, with a short context dump —
/// so a regression points at the sample, not just "not equal".
fn assert_bit_exact<T: PartialEq + core::fmt::Debug>(ours: &[T], reference: &[T], what: &str) {
    assert_eq!(ours.len(), reference.len(), "{what}: length mismatch");
    if let Some(i) = (0..ours.len()).find(|&i| ours[i] != reference[i]) {
        let lo = i.saturating_sub(4);
        let hi = (i + 4).min(ours.len());
        panic!(
            "{what}: first divergence at index {i}: ours[{lo}..{hi}] = {:?}, reference = {:?}",
            &ours[lo..hi],
            &reference[lo..hi]
        );
    }
}

// ---------------------------------------------------------------------
// Committed-prefix legs (always run, including standalone CI).
// ---------------------------------------------------------------------

#[test]
fn encoder_pcm16_is_bit_exact_on_corpus_prefix() {
    let input = pcm16_le(INPSP_PREFIX);
    assert_eq!(input.len(), PREFIX_OCTETS * 2);
    let mut enc = Encoder::new();
    let octets = enc.encode_pcm16(&input);
    assert_bit_exact(&octets, CODSPW_PREFIX, "encoder vs codspw prefix");
}

#[test]
fn decoder_pcm16_mode1_is_bit_exact_on_corpus_prefix() {
    let mut dec = Decoder::new(Mode::Mode1);
    let out = dec.decode_pcm16(CODSPW_PREFIX);
    assert_bit_exact(
        &out,
        &pcm16_le(OUTSP1_PREFIX),
        "mode-1 decode vs outsp1 prefix",
    );
}

#[test]
fn decoder_pcm16_mode2_is_bit_exact_on_corpus_prefix() {
    let mut dec = Decoder::new(Mode::Mode2);
    let out = dec.decode_pcm16(CODSPW_PREFIX);
    assert_bit_exact(
        &out,
        &pcm16_le(OUTSP2_PREFIX),
        "mode-2 decode vs outsp2 prefix",
    );
}

#[test]
fn decoder_pcm16_mode3_is_bit_exact_on_corpus_prefix() {
    let mut dec = Decoder::new(Mode::Mode3);
    let out = dec.decode_pcm16(CODSPW_PREFIX);
    assert_bit_exact(
        &out,
        &pcm16_le(OUTSP3_PREFIX),
        "mode-3 decode vs outsp3 prefix",
    );
}

#[test]
fn encode_then_decode_reproduces_reference_output_on_prefix() {
    // End-to-end: because the encoder reproduces the reference
    // bitstream bit-exactly, chaining our own encoder into our own
    // mode-1 decoder must land exactly on the reference decoded PCM.
    let input = pcm16_le(INPSP_PREFIX);
    let mut enc = Encoder::new();
    let octets = enc.encode_pcm16(&input);
    let mut dec = Decoder::new(Mode::Mode1);
    let out = dec.decode_pcm16(&octets);
    assert_bit_exact(
        &out,
        &pcm16_le(OUTSP1_PREFIX),
        "encode→decode chain vs outsp1 prefix",
    );
}

#[test]
fn pcm16_and_native_entry_points_share_state_machines() {
    // The 16-bit entry points must drive the *same* sub-band ADPCM
    // state as the Table 9 15-bit ones — only the QMF normalisation
    // differs. Cheap structural check: decoding the same octets with
    // both output stages yields sample pairs whose 16-bit value is
    // within one LSB of twice the 15-bit value (the extra bit is true
    // accumulator content, so it is not always exactly 2×).
    let mut d15 = Decoder::new(Mode::Mode1);
    let mut d16 = Decoder::new(Mode::Mode1);
    let o15 = d15.decode(CODSPW_PREFIX);
    let o16 = d16.decode_pcm16(CODSPW_PREFIX);
    for (i, (a, b)) in o15.iter().zip(o16.iter()).enumerate() {
        let twice = *a * 2;
        let fine = i32::from(*b);
        assert!(
            (fine - twice).abs() <= 1,
            "sample {i}: 15-bit {a} vs 16-bit {b} not within one fine LSB"
        );
    }
}

// ---------------------------------------------------------------------
// Full-corpus legs (umbrella checkout only; skip gracefully without
// docs/, mirroring the sibling-crate docs-corpus convention).
// ---------------------------------------------------------------------

fn corpus_file(name: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from("../../docs/audio/g722/conformance").join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            eprintln!(
                "skip: corpus file {} not present in this checkout",
                path.display()
            );
            None
        }
    }
}

/// Unpack the word-per-octet `.cod` container, asserting the framing
/// (every high byte zero) along the way.
fn unpack_word_container(words: &[u8]) -> Vec<u8> {
    assert_eq!(
        words.len() % 2,
        0,
        "word container must be whole 16-bit words"
    );
    words
        .chunks_exact(2)
        .enumerate()
        .map(|(i, w)| {
            assert_eq!(w[1], 0, "codspw.cod word {i} has a non-zero high byte");
            w[0]
        })
        .collect()
}

#[test]
fn full_corpus_encoder_is_bit_exact() {
    let (Some(inp), Some(cod)) = (corpus_file("inpsp.bin"), corpus_file("codspw.cod")) else {
        return;
    };
    let input = pcm16_le(&inp);
    assert_eq!(input.len(), 97_536, "inpsp.bin sample count");
    let octets_ref = unpack_word_container(&cod);
    assert_eq!(octets_ref.len(), 48_768, "codspw.cod octet count");

    let mut enc = Encoder::new();
    let octets = enc.encode_pcm16(&input);
    assert_bit_exact(&octets, &octets_ref, "encoder vs full codspw.cod");
}

#[test]
fn full_corpus_decoder_is_bit_exact_in_all_three_modes() {
    let Some(cod) = corpus_file("codspw.cod") else {
        return;
    };
    let octets = unpack_word_container(&cod);
    for (mode, refname) in [
        (Mode::Mode1, "outsp1.bin"),
        (Mode::Mode2, "outsp2.bin"),
        (Mode::Mode3, "outsp3.bin"),
    ] {
        let Some(refout) = corpus_file(refname) else {
            return;
        };
        let reference = pcm16_le(&refout);
        assert_eq!(reference.len(), 97_536, "{refname} sample count");
        let mut dec = Decoder::new(mode);
        let out = dec.decode_pcm16(&octets);
        assert_bit_exact(&out, &reference, refname);
    }
}

#[test]
fn committed_prefixes_match_the_staged_corpus() {
    // Guard against fixture drift: the committed excerpts must be
    // verbatim prefixes of the staged files.
    let Some(inp) = corpus_file("inpsp.bin") else {
        return;
    };
    assert_eq!(&inp[..INPSP_PREFIX.len()], INPSP_PREFIX, "inpsp prefix");
    let cod = corpus_file("codspw.cod").unwrap();
    let octets = unpack_word_container(&cod);
    assert_eq!(
        &octets[..CODSPW_PREFIX.len()],
        CODSPW_PREFIX,
        "codspw prefix"
    );
    for (name, prefix) in [
        ("outsp1.bin", OUTSP1_PREFIX),
        ("outsp2.bin", OUTSP2_PREFIX),
        ("outsp3.bin", OUTSP3_PREFIX),
    ] {
        let full = corpus_file(name).unwrap();
        assert_eq!(&full[..prefix.len()], prefix, "{name} prefix");
    }
}

#[test]
fn byte_packed_container_is_a_distinct_stream() {
    // Documented anomaly (see module docs): codsp.cod is *shaped* like
    // a byte-packed variant of codspw.cod (same octet count) but does
    // not carry the same codewords. Pin the shape and the fact of the
    // divergence so a future corpus re-staging that fixes or explains
    // the file will surface here.
    let (Some(byte_packed), Some(word_packed)) =
        (corpus_file("codsp.cod"), corpus_file("codspw.cod"))
    else {
        return;
    };
    let words = unpack_word_container(&word_packed);
    assert_eq!(byte_packed.len(), words.len(), "same octet count");
    assert_eq!(
        &byte_packed[..36],
        &words[..36],
        "the two containers agree over the leading silence"
    );
    assert_ne!(
        byte_packed, words,
        "codsp.cod unexpectedly became identical to codspw.cod — \
         re-examine the container documentation in the corpus README"
    );
}

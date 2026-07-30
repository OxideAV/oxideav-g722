//! Bit-exact conformance against the ITU-T G.722 Appendix II digital
//! test sequences (the "T-series"), staged at
//! `docs/audio/g722/conformance/tseries-appendix-II/` in the umbrella
//! checkout (see that directory's `README.md` for provenance).
//!
//! # What the T-series pins
//!
//! Unlike the G.191 demo corpus (`tests/itu_conformance.rs`), which
//! drives the full QMF-included codec end to end, Appendix II runs the
//! sub-band ADPCM arithmetic **in isolation** (clause II.2 of the
//! staged 2012 consolidated Recommendation: "When the TEST signal is
//! provided, the QMFs are by-passed and the test sequences are applied
//! directly to the ADPCM encoders or decoders"). A mismatch therefore
//! localises to the quantizer / predictor adaptation rather than the
//! filter bank. Eleven legs are defined (clause II.4.3, Figure II.6):
//!
//! * Configuration 1 (encoder): `T1C1.XMT → T2R1.COD` and
//!   `T1C2.XMT → T2R2.COD`.
//! * Configuration 2 (decoder): each of `T2R1.COD` / `T2R2.COD` /
//!   `T1D3.COD` decoded to the lower-band references `.RC1/.RC2/.RC3`
//!   (Modes 1/2/3) and the higher-band reference `.RC0` (the higher
//!   band has no mode variation).
//!
//! The comparison rule is **bit-exact 16-bit word streams**: the
//! Appendix II word formats (Figures II.1–II.3) and the INFA / INFB /
//! INFC / INFD sub-blocks (clause II.2.3) define exactly how the
//! `X#` / `I#` / `RL#` / `RH#` words map onto codec inputs/outputs,
//! and the crate's `test_harness` module implements them.
//!
//! # File framing (clause II.4.5)
//!
//! Every distribution file is `16 RSS-marker words (0x0001)` +
//! `16 384 or 768 data words (LSB = 0)` + `16 RSS-marker words`. The
//! harness output for an RSS-marked input word is itself the `0x0001`
//! marker word (INFB / INFD with RS = 1), so whole files are compared
//! word for word — markers included.
//!
//! # Integrity
//!
//! The staged binary files are checked against the CRC-32 / size table
//! that the Recommendation itself publishes (Table II.6 of the 2012
//! edition), and the three staged byte-representations (1986 hex-ASCII
//! with per-line checksums, big-endian, little-endian) are
//! cross-checked word-for-word against each other, so the oracle data
//! cannot silently drift.
//!
//! All tests skip gracefully (with a log line) when `docs/` is not
//! present, keeping standalone CI green; the two spec-synthesisable
//! sequences (`T1D3.COD`, `T1C2.XMT`) remain pinned unconditionally in
//! `src/test_harness.rs` / `src/conformance.rs`.

use std::path::PathBuf;

use oxideav_g722::test_harness::{
    appendix_ii, run_configuration_1, run_configuration_2, Configuration2Output,
};
use oxideav_g722::{Decoder, Encoder, Mode};

/// Number of RSS-marker words framing each file (clause II.4.5).
const MARKER_WORDS: usize = 16;

/// The RSS marker word: LSB set, all other bits clear.
const MARKER: i16 = 0x0001;

fn tseries_dir() -> PathBuf {
    PathBuf::from("../../docs/audio/g722/conformance/tseries-appendix-II")
}

/// Read a little-endian binary T-series file into 16-bit words, or
/// `None` (with a log line) when the corpus is not present.
fn read_le_words(name: &str) -> Option<Vec<i16>> {
    let path = tseries_dir().join("le").join(name);
    match std::fs::read(&path) {
        Ok(bytes) => {
            assert_eq!(bytes.len() % 2, 0, "{name}: odd byte count");
            Some(
                bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect(),
            )
        }
        Err(_) => {
            eprintln!(
                "skip: T-series file {} not in this checkout",
                path.display()
            );
            None
        }
    }
}

/// Assert the clause II.4.5 frame structure and return the data
/// payload (words between the marker blocks).
fn payload<'a>(words: &'a [i16], name: &str) -> &'a [i16] {
    assert!(
        words.len() > 2 * MARKER_WORDS,
        "{name}: too short for the marker framing"
    );
    let n = words.len() - 2 * MARKER_WORDS;
    assert!(
        n == 16_384 || n == 768,
        "{name}: unexpected payload length {n} (clause II.4.5 allows 16384 or 768)"
    );
    for (i, &w) in words[..MARKER_WORDS].iter().enumerate() {
        assert_eq!(w, MARKER, "{name}: leading marker word {i}");
    }
    for (i, &w) in words[words.len() - MARKER_WORDS..].iter().enumerate() {
        assert_eq!(w, MARKER, "{name}: trailing marker word {i}");
    }
    let data = &words[MARKER_WORDS..words.len() - MARKER_WORDS];
    for (i, &w) in data.iter().enumerate() {
        assert_eq!(w & 1, 0, "{name}: data word {i} has RSS/VI set");
    }
    data
}

/// First-divergence assertion with context, mirroring the G.191 leg.
fn assert_bit_exact(ours: &[i16], reference: &[i16], what: &str) {
    assert_eq!(ours.len(), reference.len(), "{what}: length mismatch");
    if let Some(i) = (0..ours.len()).find(|&i| ours[i] != reference[i]) {
        let lo = i.saturating_sub(4);
        let hi = (i + 4).min(ours.len());
        panic!(
            "{what}: first divergence at word {i}: ours[{lo}..{hi}] = {:?}, reference = {:?}",
            &ours[lo..hi],
            &reference[lo..hi]
        );
    }
}

// ---------------------------------------------------------------------
// Integrity: the Recommendation's own CRC-32 table (Table II.6) plus
// cross-representation agreement (ascii == le == be).
// ---------------------------------------------------------------------

/// IEEE CRC-32 (reflected, polynomial 0xEDB88320) — the convention the
/// binary T-series distributions use for their published integrity
/// table (Table II.6 of the staged 2012 Recommendation; also reproduced
/// in the staging README).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = !0;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// (file stem, extension, LE CRC-32, BE CRC-32, total words) — the
/// seventeen sequences of Table II.6 of the staged Recommendation.
const TABLE_II_6: [(&str, u32, u32, usize); 17] = [
    ("bt1c1.xmt", 0x0C3B_FCA7, 0x015A_CCE4, 16_416),
    ("bt1c2.xmt", 0x2D60_4685, 0xEAFC_99B4, 800),
    ("bt1d3.cod", 0x7398_964F, 0x1C85_BE45, 16_416),
    ("bt2r1.cod", 0xD1DA_A1D1, 0x0B90_4231, 16_416),
    ("bt2r2.cod", 0x344E_A5D0, 0xF928_980D, 800),
    ("bt3h1.rc0", 0xE925_0851, 0x0BDE_9C9C, 16_416),
    ("bt3h2.rc0", 0x5330_AE2E, 0x3A54_C7DF, 800),
    ("bt3h3.rc0", 0x3731_AD7F, 0x8E8D_EE65, 16_416),
    ("bt3l1.rc1", 0xED1B_3993, 0x90DD_2D72, 16_416),
    ("bt3l1.rc2", 0x8E8C_4E2B, 0xFE7C_4611, 16_416),
    ("bt3l1.rc3", 0xB7AA_5569, 0x20B5_FFC4, 16_416),
    ("bt3l2.rc1", 0xAF00_F31F, 0xD259_9DE8, 800),
    ("bt3l2.rc2", 0x9143_E92C, 0x8404_1F43, 800),
    ("bt3l2.rc3", 0xAE85_5C07, 0xF326_28D9, 800),
    ("bt3l3.rc1", 0xA537_4659, 0x8C12_ED04, 16_416),
    ("bt3l3.rc2", 0x687B_250A, 0x5505_34A7, 16_416),
    ("bt3l3.rc3", 0x3605_736B, 0x9354_E9CF, 16_416),
];

#[test]
fn staged_binaries_match_the_recommendations_crc32_table() {
    let dir = tseries_dir();
    if !dir.join("le").is_dir() {
        eprintln!("skip: {} not in this checkout", dir.display());
        return;
    }
    for (name, crc_le, crc_be, words) in TABLE_II_6 {
        let le = std::fs::read(dir.join("le").join(name)).unwrap();
        let be = std::fs::read(dir.join("be").join(name)).unwrap();
        assert_eq!(le.len(), words * 2, "{name}: LE size");
        assert_eq!(be.len(), words * 2, "{name}: BE size");
        assert_eq!(crc32(&le), crc_le, "{name}: LE CRC-32 (Table II.6)");
        assert_eq!(crc32(&be), crc_be, "{name}: BE CRC-32 (Table II.6)");
        // LE and BE must be byte-swapped images of the same words.
        let swapped: Vec<u8> = be.chunks_exact(2).flat_map(|c| [c[1], c[0]]).collect();
        assert_eq!(le, swapped, "{name}: LE/BE word disagreement");
    }
}

/// Parse a 1986 hex-ASCII distribution file (clause II.4.4.1): two
/// comment header lines, then lines of 16 four-hex-digit words plus a
/// 1-byte checksum ("two's complement of the least significant 8 bits
/// of the summation of all the preceding characters in the line"),
/// CRLF line endings, a trailing `/* END OF FILE ... */` comment line.
fn parse_ascii(bytes: &[u8], name: &str) -> Vec<i16> {
    let mut words = Vec::new();
    let mut data_lines = 0usize;
    for (li, line) in bytes.split(|&b| b == b'\n').enumerate() {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() || line == [0x1A] {
            continue; // trailing EOF byte in the 1986 files
        }
        if line.starts_with(b"/*") {
            continue; // header / end-of-file comment lines
        }
        assert_eq!(
            line.len(),
            66,
            "{name}: line {li} is not 64 hex chars + 2 checksum chars"
        );
        let sum: u32 = line[..64].iter().map(|&b| u32::from(b)).sum();
        let expected = (!sum + 1) & 0xFF; // two's complement of low 8 bits
        let actual = u32::from_str_radix(std::str::from_utf8(&line[64..66]).unwrap(), 16).unwrap();
        assert_eq!(actual, expected, "{name}: line {li} checksum");
        for w in line[..64].chunks_exact(4) {
            let v = u16::from_str_radix(std::str::from_utf8(w).unwrap(), 16).unwrap();
            words.push(v as i16);
        }
        data_lines += 1;
    }
    assert!(data_lines > 0, "{name}: no data lines parsed");
    words
}

#[test]
fn ascii_files_carry_the_same_words_as_the_binaries() {
    let dir = tseries_dir();
    if !dir.join("ascii").is_dir() {
        eprintln!("skip: {} not in this checkout", dir.display());
        return;
    }
    for (name, ..) in TABLE_II_6 {
        // ASCII names lack the leading 'b' and are upper-case.
        let ascii_name = name[1..].to_uppercase();
        let ascii = std::fs::read(dir.join("ascii").join(&ascii_name)).unwrap();
        let words_ascii = parse_ascii(&ascii, &ascii_name);
        let words_le = read_le_words(name).unwrap();
        assert_bit_exact(&words_ascii, &words_le, &format!("{ascii_name} vs {name}"));
    }
}

// ---------------------------------------------------------------------
// The two spec-synthesisable sequences must equal the ITU-distributed
// bytes (closing the loop on the procedural generators pinned in
// src/test_harness.rs).
// ---------------------------------------------------------------------

#[test]
fn artificial_sequence_generator_reproduces_bt1d3() {
    let Some(reference) = read_le_words("bt1d3.cod") else {
        return;
    };
    payload(&reference, "bt1d3.cod");
    let built = appendix_ii::build_cod_frame();
    assert_bit_exact(
        &built,
        &reference,
        "II.3.2 artificial sequence vs bt1d3.cod",
    );
}

#[test]
fn overflow_sequence_generator_reproduces_bt1c2() {
    let Some(reference) = read_le_words("bt1c2.xmt") else {
        return;
    };
    payload(&reference, "bt1c2.xmt");
    let mut built = vec![MARKER; MARKER_WORDS];
    built.extend(appendix_ii::build_overflow_x_hash_stream());
    built.extend(vec![MARKER; MARKER_WORDS]);
    assert_bit_exact(&built, &reference, "Table II-3 overflow input vs bt1c2.xmt");
}

#[test]
fn t1c1_payload_matches_the_printed_table_ii_2_structure() {
    // The printed Table II-2 enumerates only segment kinds / lengths;
    // the one fully sample-enumerable segment is the 512-word
    // "d.c., value of zero". Pin it inside the real stimulus, plus the
    // adjacent segments being non-zero d.c. of constant value.
    let Some(words) = read_le_words("bt1c1.xmt") else {
        return;
    };
    let data = payload(&words, "bt1c1.xmt");
    let zero_seg =
        &data[appendix_ii::TABLE_II_2_DC_ZERO_OFFSET..][..appendix_ii::TABLE_II_2_DC_ZERO_LEN];
    assert!(
        zero_seg.iter().all(|&w| w == 0),
        "d.c.-zero segment is not all-zero"
    );
    // d.c. positive low (512 words before) and d.c. negative low (512
    // words after): constant, non-zero, correct signs (Table II-2).
    let pos = &data[appendix_ii::TABLE_II_2_DC_ZERO_OFFSET - 512..][..512];
    let neg = &data[appendix_ii::TABLE_II_2_DC_ZERO_OFFSET + 512..][..512];
    assert!(
        pos.iter().all(|&w| w == pos[0]) && pos[0] > 0,
        "d.c. positive segment"
    );
    assert!(
        neg.iter().all(|&w| w == neg[0]) && neg[0] < 0,
        "d.c. negative segment"
    );
}

// ---------------------------------------------------------------------
// Configuration 1 — encoder legs (Figure II.6).
// ---------------------------------------------------------------------

fn run_encoder_leg(input: &str, reference: &str) {
    let (Some(x_hash), Some(i_hash_ref)) = (read_le_words(input), read_le_words(reference)) else {
        return;
    };
    payload(&x_hash, input);
    payload(&i_hash_ref, reference);
    let mut enc = Encoder::new();
    let i_hash = run_configuration_1(&mut enc, &x_hash);
    assert_bit_exact(&i_hash, &i_hash_ref, &format!("{input} -> {reference}"));
}

#[test]
fn encoder_leg_t1c1_is_bit_exact() {
    run_encoder_leg("bt1c1.xmt", "bt2r1.cod");
}

#[test]
fn encoder_leg_t1c2_is_bit_exact() {
    run_encoder_leg("bt1c2.xmt", "bt2r2.cod");
}

// ---------------------------------------------------------------------
// Configuration 2 — decoder legs (Figure II.6): three code-word inputs
// by three modes for the lower band, plus the mode-independent higher
// band.
// ---------------------------------------------------------------------

fn run_decoder_legs(input: &str, rc_lower: [&str; 3], rc_higher: &str) {
    let Some(i_hash) = read_le_words(input) else {
        return;
    };
    payload(&i_hash, input);
    let Some(rh_ref) = read_le_words(rc_higher) else {
        return;
    };
    for (mode, rc) in [Mode::Mode1, Mode::Mode2, Mode::Mode3]
        .into_iter()
        .zip(rc_lower)
    {
        let Some(rl_ref) = read_le_words(rc) else {
            return;
        };
        payload(&rl_ref, rc);
        let mut dec = Decoder::new(mode);
        let Configuration2Output { rl_hash, rh_hash } = run_configuration_2(&mut dec, &i_hash);
        assert_bit_exact(&rl_hash, &rl_ref, &format!("{input} mode {mode:?} -> {rc}"));
        // The higher band has no mode variation (clause II.4.3: one
        // .RC0 per input): every mode must land on the same reference.
        assert_bit_exact(
            &rh_hash,
            &rh_ref,
            &format!("{input} mode {mode:?} -> {rc_higher}"),
        );
    }
}

#[test]
fn decoder_legs_t2r1_are_bit_exact() {
    run_decoder_legs(
        "bt2r1.cod",
        ["bt3l1.rc1", "bt3l1.rc2", "bt3l1.rc3"],
        "bt3h1.rc0",
    );
}

#[test]
fn decoder_legs_t2r2_are_bit_exact() {
    run_decoder_legs(
        "bt2r2.cod",
        ["bt3l2.rc1", "bt3l2.rc2", "bt3l2.rc3"],
        "bt3h2.rc0",
    );
}

#[test]
fn decoder_legs_t1d3_are_bit_exact() {
    run_decoder_legs(
        "bt1d3.cod",
        ["bt3l3.rc1", "bt3l3.rc2", "bt3l3.rc3"],
        "bt3h3.rc0",
    );
}

// ---------------------------------------------------------------------
// Full circuit: our encoder's output for the two .XMT stimuli, decoded
// by our decoder, must equal the decode of the ITU reference .COD —
// because the encoder legs above are bit-exact, this chains the two
// configurations into one spec-shaped round trip.
// ---------------------------------------------------------------------

#[test]
fn full_circuit_t1c1_through_both_configurations() {
    let (Some(x_hash), Some(rl_ref), Some(rh_ref)) = (
        read_le_words("bt1c1.xmt"),
        read_le_words("bt3l1.rc1"),
        read_le_words("bt3h1.rc0"),
    ) else {
        return;
    };
    let mut enc = Encoder::new();
    let i_hash = run_configuration_1(&mut enc, &x_hash);
    let mut dec = Decoder::new(Mode::Mode1);
    let Configuration2Output { rl_hash, rh_hash } = run_configuration_2(&mut dec, &i_hash);
    assert_bit_exact(&rl_hash, &rl_ref, "t1c1 full circuit lower band");
    assert_bit_exact(&rh_hash, &rh_ref, "t1c1 full circuit higher band");
}

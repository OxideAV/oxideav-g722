# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- **Round-185 clean-room decoder bring-up.** SB-ADPCM receive path
  against the staged ITU-T G.722 (09/2012) recommendation: lower
  sub-band ADPCM decoder (4/5/6-bit modes via INVQAL + INVQBL),
  higher sub-band ADPCM decoder (2-bit INVQAH), the symmetric pole +
  zero adaptive predictor with UPPOL1 / UPPOL2 / UPZERO updates,
  logarithmic scale-factor adaptation (Method 2, 32-entry ILB), and
  the 24-tap receive QMF that interleaves the sub-bands into a 16 kHz
  output stream.
- New `Decoder` / `Mode` public API and `make_decoder` factory.
  Encoder factory is a `NotImplemented` stub.
- Spec tables 4, 11, 14, 15-ILB, 17, 18, 19 and 21 transcribed by hand
  from the staged PDF with per-table provenance citations in
  `src/tables.rs`.
- 18 unit tests + 1 doc-test covering table shape / monotonicity,
  decoder determinism, mode-switch idempotence, reset symmetry, and
  envelope behaviour with all-zero input.

### Changed

- **Reset to orphan-rebuild scaffold (2026-05-25).** The prior
  implementation was retired under the workspace clean-room policy: its
  data tables were documented as having been copied from an external
  reference implementation of the codec, whose provenance the clean-room
  policy does not permit. All public APIs now return
  `Error::NotImplemented` pending a clean-room rebuild against a staged
  ITU-T G.722 Recommendation.

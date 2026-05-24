# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- **Reset to orphan-rebuild scaffold (2026-05-25).** The prior
  implementation was retired under the workspace clean-room policy: its
  data tables were documented as having been copied from an external
  reference implementation of the codec, whose provenance the clean-room
  policy does not permit. All public APIs now return
  `Error::NotImplemented` pending a clean-room rebuild against a staged
  ITU-T G.722 Recommendation.

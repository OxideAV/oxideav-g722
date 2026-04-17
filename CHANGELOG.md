# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.4](https://github.com/OxideAV/oxideav-g722/compare/v0.0.3...v0.0.4) - 2026-04-17

### Other

- wire normative ITU-T quantiser + log-scale tables into ADPCM pipeline
- rewrite to match reality — modes, aux side-channel, status caveats
- expose 8 / 16 kbit/s auxiliary side-channel via push_aux / take_aux

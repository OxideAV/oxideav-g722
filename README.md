# oxideav-g722

Pure-Rust ITU-T G.722 wideband sub-band ADPCM speech codec (64 / 56 / 48 kbit/s).

## Status: orphan-rebuild scaffold (reset 2026-05-25)

The previous implementation was retired under the OxideAV clean-room
policy. Its quantiser / predictor data tables were documented as having been
copied from an external reference implementation of the codec, and the
clean-room policy does not permit consulting any external
implementation's source for any reason. Because that provenance could
not be defended, the implementation was removed and the crate reset to
this scaffold.

The crate will be re-built from scratch against a staged ITU-T G.722
Recommendation in a future clean-room round, once that document is staged
under `docs/audio/g722/`. Until then every public API returns
`Error::NotImplemented`.

## License

MIT — see [LICENSE](LICENSE).

//! # oxideav-g722
//!
//! **Status:** orphan-rebuild scaffold (reset 2026-05-25).
//!
//! The prior implementation was retired under the workspace clean-room
//! policy: its data tables were documented as having been copied from an
//! external reference implementation of the codec, whose provenance the
//! clean-room policy does not permit. The policy forbids consulting any
//! external implementation's source for any reason, so the provenance
//! could not be defended. The crate will be re-implemented from scratch
//! against a staged ITU-T G.722 Recommendation in a future clean-room
//! round, once that document is staged under `docs/audio/g722/`.
//!
//! Every public API currently returns [`Error::NotImplemented`].

#![warn(missing_debug_implementations)]

use oxideav_core::RuntimeContext;

/// Crate-local error type. Until the clean-room rebuild lands every
/// public API path returns [`Error::NotImplemented`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The crate has been reset to a scaffold pending clean-room
    /// rebuild; no decoder or encoder functionality is wired up yet.
    NotImplemented,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "oxideav-g722: orphan-rebuild scaffold — no codec wired up"
        )
    }
}

impl std::error::Error for Error {}

/// No-op codec registration — the orphan-rebuild scaffold registers
/// nothing into the runtime context.
pub fn register(_ctx: &mut RuntimeContext) {}

oxideav_core::register!("g722", register);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_error_displays() {
        assert_eq!(Error::NotImplemented, Error::NotImplemented);
        assert!(!format!("{}", Error::NotImplemented).is_empty());
    }
}

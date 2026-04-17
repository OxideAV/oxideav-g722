//! Determinism / reference-vector test for the ADPCM pipeline.
//!
//! The low-band and high-band ADPCM states are defined in terms of the
//! normative ITU-T G.722 quantiser / log-scale tables (`QM4` / `QM5` /
//! `QM6` / `QM2`, `WL` / `WH`, `Q6`, `ILN` / `ILP`, `IHN` / `IHP`, `RL42` /
//! `RH2`, `ILB`). Feeding a known input sequence through the `LowBand` /
//! `HighBand` encoders must always produce the same code sequence — this
//! pins the algorithm so any accidental regression shows up immediately.

use oxideav_g722::band_high::HighBand;
use oxideav_g722::band_low::LowBand;
use oxideav_g722::mode::Mode;

/// Helper: run a slice of "low-band samples" (as i32, 15-bit range) through
/// a LowBand encoder configured for `mode` and return the 6 / 5 / 4-bit
/// codes.
fn encode_lb(samples: &[i32], mode: Mode) -> Vec<u8> {
    let mut enc = LowBand::for_mode(mode);
    samples.iter().map(|&x| enc.encode(x)).collect()
}

#[test]
fn mode1_low_band_ramp_codes_are_stable() {
    // Feed a linear ramp from -8000 to +8000 into a fresh Mode-1 LowBand.
    // The encoder state + ITU tables must produce this exact code sequence;
    // any drift would flag a regression in the forward quantiser, scale
    // adapter or BLOCK4 predictor update.
    let samples: Vec<i32> = (0..32).map(|n| (n * 500) - 8000).collect();
    let codes = encode_lb(&samples, Mode::Mode1);
    let expected: [u8; 32] = [
        0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0B, 0x0F, 0x12, 0x14, 0x16, 0x18, 0x1A, 0x1D, 0x3E,
        0x3F, 0x3B, 0x39, 0x36, 0x33, 0x32, 0x31, 0x2F, 0x30, 0x2E, 0x2E, 0x2F, 0x30, 0x2E, 0x2F,
        0x2F, 0x2F,
    ];
    assert_eq!(codes.as_slice(), &expected, "low-band codes drifted");
}

#[test]
fn high_band_impulse_codes_are_stable() {
    let mut enc = HighBand::new();
    // An impulse followed by silence: HB should react and then settle.
    let samples: [i32; 16] = [0, 0, 4000, 0, -4000, 0, 0, 0, 2000, 0, -1000, 0, 0, 0, 0, 0];
    let codes: Vec<u8> = samples.iter().map(|&x| enc.encode(x)).collect();
    let expected: [u8; 16] = [3, 3, 2, 3, 0, 2, 2, 3, 2, 3, 0, 3, 3, 3, 3, 2];
    assert_eq!(codes.as_slice(), &expected, "high-band codes drifted");
}

#[test]
fn encoder_is_deterministic_across_calls() {
    // Two fresh encoders must produce identical output for identical input.
    let samples: Vec<i32> = (0..128)
        .map(|n| ((n as f32 * 0.1).sin() * 4000.0) as i32)
        .collect();
    for mode in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
        let a = encode_lb(&samples, mode);
        let b = encode_lb(&samples, mode);
        assert_eq!(a, b, "non-deterministic output at {mode:?}");
    }
}

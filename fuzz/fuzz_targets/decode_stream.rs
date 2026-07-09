#![no_main]

//! Arbitrary octet streams through the G.722 decoder with fuzz-driven
//! mid-stream mode switches and resets.
//!
//! Every `u8` is a syntactically valid multiplexer octet (clause
//! 1.4.4: 2-bit `I_H` + 6-bit `I_L`), so the decoder must be **total**
//! over arbitrary byte streams — there is no reject path. What can
//! break is the arithmetic inside the two ADPCM loops and the receive
//! QMF when the predictor state is driven somewhere golden-vector
//! streams never reach, and the state-machine handling of clause
//! 1.3's "the variant used in the SB-ADPCM decoder can be changed in
//! any octet during the transmission" (page 3) plus a reset landing
//! between any two octets. The target asserts, per octet:
//!
//! - exactly two output samples;
//! - both inside the Table 9/G.722 (page 25) receive saturation range
//!   `−16384 ..= 16383` (the §§ 6.2.1.6 / 6.2.2.5 LIMIT blocks feed
//!   the receive QMF whose output clamp is the same window).

use libfuzzer_sys::fuzz_target;
use oxideav_g722::{Decoder, Mode};

const MODES: [Mode; 3] = [Mode::Mode1, Mode::Mode2, Mode::Mode3];

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, stream)) = data.split_first() else {
        return;
    };
    let mut dec = Decoder::new(MODES[(ctl % 3) as usize]);
    for (i, &octet) in stream.iter().enumerate() {
        let (a, b) = dec.decode_octet(octet);
        assert!(
            (-16384..=16383).contains(&a) && (-16384..=16383).contains(&b),
            "octet {i} ({octet:#04x}) decoded outside the Table 9 range: ({a}, {b})"
        );
        // Fuzz-driven control-plane churn: the low bits of the octet
        // combined with the position choose an occasional mode switch
        // or reset, so the mode-dependent INVQBL ladder runs against
        // predictor state accumulated under a different mode.
        match (octet as usize).wrapping_add(i) % 29 {
            0 => dec.set_mode(Mode::Mode1),
            1 => dec.set_mode(Mode::Mode2),
            2 => dec.set_mode(Mode::Mode3),
            3 => dec.reset(),
            _ => {}
        }
    }
});

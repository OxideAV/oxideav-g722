#![no_main]

//! The clause 1.3 / Figure 1/G.722 auxiliary-data channel devices.
//!
//! The data-insertion device substitutes 0 / 1 / 2 LSBs of each wire
//! octet (Table 1/G.722 bit-rate accounting: `I_L6` in Mode 2,
//! `I_L5` + `I_L6` in Mode 3) with queued auxiliary bits, padding
//! when the queue runs dry; the data-extraction device recovers the
//! same positions. This target drives a fuzz-chosen mode, payload
//! and carrier stream through an insert → extract round trip and
//! asserts the wire-format contract:
//!
//! - every audio bit **above** the substituted LSBs is untouched;
//! - the extractor recovers exactly the inserted bit sequence
//!   (queued payload first, then the padding bit), in order;
//! - the drained / padded accounting sums to the number of
//!   substituted positions.

use libfuzzer_sys::fuzz_target;
use oxideav_g722::{aux_bits_per_octet, DataExtractor, DataInserter, Mode};

const MODES: [Mode; 3] = [Mode::Mode1, Mode::Mode2, Mode::Mode3];

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, rest)) = data.split_first() else {
        return;
    };
    let mode = MODES[(ctl % 3) as usize];
    let padding = ctl & 0x40 != 0;
    // First half of the remaining input feeds the aux queue (one bit
    // per byte), second half is the carrier octet stream.
    let (payload, carrier) = rest.split_at(rest.len() / 2);

    let mut inserter = DataInserter::new(mode);
    inserter.set_padding_bit(padding);
    let bits: Vec<bool> = payload.iter().map(|&b| b & 1 != 0).collect();
    inserter.push_aux_bits(&bits);

    let wire = inserter.insert_slice(carrier);
    assert_eq!(wire.len(), carrier.len());

    let n = aux_bits_per_octet(mode) as usize;
    let mask: u8 = (1u8 << n) - 1;
    for (i, (&before, &after)) in carrier.iter().zip(wire.iter()).enumerate() {
        assert_eq!(
            before & !mask,
            after & !mask,
            "octet {i}: audio bits above the aux LSBs were modified"
        );
    }

    // Accounting: every substituted position was either a queued bit
    // or a padding bit.
    let substituted = (carrier.len() * n) as u64;
    assert_eq!(
        inserter.aux_bits_drained() + inserter.padding_bits_inserted(),
        substituted
    );
    assert_eq!(
        inserter.aux_bits_drained(),
        (bits.len() as u64).min(substituted)
    );

    // Round trip: the extractor must recover the inserted sequence
    // (payload prefix, then padding) exactly and in order.
    let mut extractor = DataExtractor::new(mode);
    let recovered_stream = extractor.extract_slice(&wire);
    assert_eq!(recovered_stream, &wire[..], "extractor altered the octets");
    assert_eq!(extractor.aux_bits_extracted(), substituted);
    let recovered = extractor.drain_aux_bits(substituted as usize);
    for (i, &bit) in recovered.iter().enumerate() {
        let expected = if i < bits.len() { bits[i] } else { padding };
        assert_eq!(bit, expected, "aux bit {i} corrupted in transit");
    }
});

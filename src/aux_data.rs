//! Auxiliary-data channel — data insertion / extraction devices
//! (Figure 1/G.722, clause 1.3).
//!
//! Per clause 1.1 of the staged ITU-T G.722 (11/88) Recommendation
//! ("The latter two modes allow an auxiliary data channel of 8 or
//! 16 kbit/s respectively to be provided within the 64 kbit/s by
//! making use of bits from the lower sub-band") the auxiliary-data
//! path is implemented by two devices that sit on the wire side of
//! the SB-ADPCM coder (Figure 1/G.722, page 2):
//!
//! * **Data insertion device** (transmit side) — substitutes "1 or 2
//!   audio bits per octet depending on the mode of operation" with
//!   bits from the auxiliary data channel before transmission. The
//!   substituted positions are the LSBs of `I_L` (clause 1.4.2:
//!   "the two least significant bits of `I_L` are deleted to produce
//!   a 4-bit signal `I_Lt`" — those same LSBs are the audio bits
//!   replaced by the auxiliary data on the wire). The substitution
//!   is downstream of the SB-ADPCM encoder so the encoder's predictor
//!   feedback loop is unaffected by what data the substitution
//!   carries.
//! * **Data extraction device** (receive side) — recovers the
//!   auxiliary bits from the same LSB positions and forwards the
//!   octet to the SB-ADPCM decoder, additionally signalling the
//!   operating mode (Note 4 of Figure 1/G.722: "64 kbit/s signal
//!   comprising 64, 56 or 48 kbit/s for audio coding depending on
//!   the mode of operation").
//!
//! The bit-rate accounting (Table 1/G.722, page 3) is:
//!
//! | Mode | Audio | Aux  | Aux LSB count | Aux bits per octet |
//! | ---- | ----- | ---- | ------------- | ------------------ |
//! | 1    | 64 k  | 0 k  | 0             | 0                  |
//! | 2    | 56 k  | 8 k  | 1 (`I_L6`)    | 1                  |
//! | 3    | 48 k  | 16 k | 2 (`I_L5`, `I_L6`) | 2             |
//!
//! Per clause 1.4.4 page 6 the octet layout is `I_H1 I_H2 I_L1 I_L2
//! I_L3 I_L4 I_L5 I_L6` with `I_H1` as the first transmitted bit
//! (MSB of the octet). The auxiliary LSBs are therefore at bit
//! position 0 (`I_L6`) and bit position 1 (`I_L5`) of the wire
//! octet.

use crate::decoder::Mode;

extern crate alloc;

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Number of auxiliary bits carried in each transmitted octet for the
/// given mode (0 / 1 / 2 — matching the row count of Table 1/G.722).
///
/// This is the per-octet equivalent of [`Mode::lsbs_to_discard`]; the
/// two are intentionally identical (the data-insertion device steals
/// exactly the LSBs that the decoder is going to discard anyway in
/// the matching mode).
pub const fn aux_bits_per_octet(mode: Mode) -> u8 {
    mode.lsbs_to_discard()
}

/// Channel bit-rate (kbit/s) of the auxiliary data path for the
/// given mode. Matches Table 1/G.722 column 3.
pub const fn aux_bit_rate_kbps(mode: Mode) -> u32 {
    // 8 kHz octet rate × auxiliary bits per octet.
    8 * aux_bits_per_octet(mode) as u32
}

/// Mask of the LSB positions overwritten by the data-insertion device
/// for `mode`. Returns 0 in Mode 1 (no substitution), 1 in Mode 2
/// (I_L6 only) and 3 in Mode 3 (I_L5 + I_L6).
const fn aux_lsb_mask(mode: Mode) -> u8 {
    match mode {
        Mode::Mode1 => 0,
        Mode::Mode2 => 0b1,
        Mode::Mode3 => 0b11,
    }
}

// -----------------------------------------------------------------------
// Data-insertion device (transmit side, Figure 1/G.722)
// -----------------------------------------------------------------------

/// Data-insertion device per Figure 1/G.722.
///
/// Sits between the SB-ADPCM encoder output `I` and the wire
/// (transmit side of Note 3 of Figure 1: "Comprises 64, 56 or 48
/// kbit/s for audio coding and 0, 8 or 16 kbit/s for data"). Holds
/// a small bit reservoir of auxiliary bits queued by the caller and
/// substitutes them into the LSBs of each octet's `I_L` field.
///
/// Mode 1 is a pass-through (no substitution); for Modes 2 and 3 the
/// device draws 1 or 2 auxiliary bits per octet respectively. If the
/// reservoir is empty when an octet arrives, the device fills the
/// substituted bit(s) with the value selected by
/// [`Self::set_padding_bit`] (default `0`); per clause 1.3 a missing
/// auxiliary bit cannot stall the audio path so a padding value is
/// always supplied.
///
/// The first auxiliary bit pushed into [`Self::push_aux_bit`] /
/// [`Self::push_aux_bits`] is the first one transmitted; for Mode 3
/// the bit-pair maps to `(I_L5, I_L6)` in MSB-first wire order so the
/// first queued bit lands at `I_L5` and the second at `I_L6`.
///
/// The data-insertion device is purely a wire-side device — it does
/// **not** feed back into the encoder's local decoder loop. The
/// substitution at the wire happens after the encoder has already
/// quantised, multiplexed, and locally adapted on the full 6-bit
/// `I_L`; this matches Figure 1/G.722's data-insertion device sitting
/// outside the dashed "64 kbit/s (7 kHz) audio encoder" box.
#[derive(Debug, Clone)]
pub struct DataInserter {
    mode: Mode,
    /// FIFO of queued auxiliary bits, oldest at index 0.
    queue: alloc::collections::VecDeque<bool>,
    /// Padding bit used when the queue is empty.
    padding: bool,
    /// Number of auxiliary bits drained so far (cumulative).
    drained: u64,
    /// Number of padding bits substituted so far (cumulative).
    padded: u64,
}

impl DataInserter {
    /// Construct a new device in the requested mode with an empty
    /// queue and a `0` padding bit.
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            queue: alloc::collections::VecDeque::new(),
            padding: false,
            drained: 0,
            padded: 0,
        }
    }

    /// Current mode of the device.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Reconfigure the device mid-stream. Per clause 1.3 the mode may
    /// be switched on any octet boundary; the queue is preserved so a
    /// caller that switched from Mode 3 to Mode 2 will keep draining
    /// the already-queued bits at the new (slower) rate.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Padding bit value used when an octet arrives with an empty
    /// auxiliary queue. Default is `false` (= `0`).
    pub fn set_padding_bit(&mut self, bit: bool) {
        self.padding = bit;
    }

    /// Number of auxiliary bits currently queued for transmission.
    pub fn queued_bits(&self) -> usize {
        self.queue.len()
    }

    /// Total number of queued auxiliary bits that have been drained
    /// into transmitted octets (excluding padding).
    pub fn aux_bits_drained(&self) -> u64 {
        self.drained
    }

    /// Total number of padding bits substituted into transmitted
    /// octets because the queue was empty at the moment the octet
    /// went out.
    pub fn padding_bits_inserted(&self) -> u64 {
        self.padded
    }

    /// Auxiliary channel bit-rate currently in use (kbit/s); matches
    /// [`aux_bit_rate_kbps`] of the active mode.
    pub fn aux_bit_rate_kbps(&self) -> u32 {
        aux_bit_rate_kbps(self.mode)
    }

    /// Push a single auxiliary bit onto the back of the queue.
    pub fn push_aux_bit(&mut self, bit: bool) {
        self.queue.push_back(bit);
    }

    /// Push a slice of auxiliary bits in MSB-first transmission order
    /// (the first element will be the first bit transmitted).
    pub fn push_aux_bits(&mut self, bits: &[bool]) {
        self.queue.extend(bits.iter().copied());
    }

    /// Substitute the auxiliary bits into a single octet emitted by
    /// the SB-ADPCM encoder.
    ///
    /// Mode 1 returns the octet unchanged. Mode 2 substitutes 1 bit
    /// at position `I_L6` (bit 0). Mode 3 substitutes 2 bits at
    /// positions `I_L5` (bit 1) + `I_L6` (bit 0) with the first
    /// dequeued bit going to `I_L5` (the MSB-most of the substituted
    /// pair, transmitted earlier on the wire than `I_L6`).
    pub fn insert(&mut self, octet: u8) -> u8 {
        let n = aux_bits_per_octet(self.mode) as usize;
        if n == 0 {
            return octet;
        }
        let mask = aux_lsb_mask(self.mode);
        let mut substitute: u8 = 0;
        // The bit closest to I_L1 ships first on the wire, so the
        // first dequeued bit must end up in the highest of the
        // substituted positions.  For Mode 2 there is only I_L6 (bit
        // 0).  For Mode 3 the order is I_L5 (bit 1) then I_L6 (bit 0).
        for shift in (0..n).rev() {
            let bit = match self.queue.pop_front() {
                Some(b) => {
                    self.drained += 1;
                    b
                }
                None => {
                    self.padded += 1;
                    self.padding
                }
            };
            substitute |= (bit as u8) << shift;
        }
        (octet & !mask) | substitute
    }

    /// Apply [`Self::insert`] to every octet in an in-place slice.
    pub fn insert_in_place(&mut self, octets: &mut [u8]) {
        for o in octets.iter_mut() {
            *o = self.insert(*o);
        }
    }

    /// Apply [`Self::insert`] to every octet of `input`, appending
    /// the substituted octets to `out`.
    pub fn insert_into(&mut self, input: &[u8], out: &mut alloc::vec::Vec<u8>) {
        out.reserve(input.len());
        for &o in input {
            out.push(self.insert(o));
        }
    }

    /// Apply [`Self::insert`] to every octet of `input`, returning a
    /// freshly allocated vector of substituted octets.
    pub fn insert_slice(&mut self, input: &[u8]) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec::Vec::with_capacity(input.len());
        self.insert_into(input, &mut out);
        out
    }

    /// Drop every queued auxiliary bit (the audio side is unaffected).
    pub fn flush_queue(&mut self) {
        self.queue.clear();
    }
}

// -----------------------------------------------------------------------
// Data-extraction device (receive side, Figure 1/G.722)
// -----------------------------------------------------------------------

/// Data-extraction device per Figure 1/G.722.
///
/// Sits between the wire input and the SB-ADPCM decoder input
/// (receive side of Note 3/Note 4 of Figure 1). Splits each
/// 64 kbit/s octet into:
///
/// * an auxiliary-data tail of 0, 1 or 2 bits (Modes 1 / 2 / 3),
///   pushed onto an internal FIFO from which the caller can drain
///   them via [`Self::pop_aux_bit`] / [`Self::drain_aux_bits`];
/// * the octet itself, forwarded unchanged to the SB-ADPCM decoder.
///
/// Per Figure 1/G.722 the device also determines the mode in use and
/// signals it to the decoder. In this crate the mode is supplied by
/// the caller (mirroring the [`Decoder::set_mode`] convention) — the
/// spec does not pin an in-band mode-detection algorithm and refers
/// out to Rec. G.725 for the in-channel handshake.
///
/// Pass-through note: the **wire octet is forwarded unchanged** to
/// the SB-ADPCM decoder. The auxiliary LSBs are only "discarded" in
/// the sense that the decoder's INVQBL inverse quantiser ignores
/// them per the mode-specific bit-width selection of clause 6.2.1.2
/// (see also [`Decoder::set_mode`]'s "two LSBs of the lower band are
/// discarded" doc-comment for Mode 3). The data-extraction device
/// itself never edits the octet.
///
/// [`Decoder`]: crate::Decoder
/// [`Decoder::set_mode`]: crate::Decoder::set_mode
#[derive(Debug, Clone)]
pub struct DataExtractor {
    mode: Mode,
    queue: alloc::collections::VecDeque<bool>,
    extracted: u64,
}

impl DataExtractor {
    /// Construct a new device in the requested mode.
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            queue: alloc::collections::VecDeque::new(),
            extracted: 0,
        }
    }

    /// Current mode of the device.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Reconfigure the device mid-stream. The queue is preserved.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Total number of auxiliary bits extracted from the wire so far.
    pub fn aux_bits_extracted(&self) -> u64 {
        self.extracted
    }

    /// Auxiliary channel bit-rate currently in use (kbit/s).
    pub fn aux_bit_rate_kbps(&self) -> u32 {
        aux_bit_rate_kbps(self.mode)
    }

    /// Process a single octet from the wire. The octet itself is
    /// returned unchanged (to be fed into the SB-ADPCM decoder); the
    /// auxiliary bits, if any, are appended to the internal FIFO.
    pub fn extract(&mut self, octet: u8) -> u8 {
        let n = aux_bits_per_octet(self.mode) as usize;
        if n == 0 {
            return octet;
        }
        // First-transmitted bit (highest substituted bit position)
        // pushes to the queue first.
        for shift in (0..n).rev() {
            let bit = ((octet >> shift) & 1) != 0;
            self.queue.push_back(bit);
            self.extracted += 1;
        }
        octet
    }

    /// Apply [`Self::extract`] to every octet of `input`. The octets
    /// are returned unchanged.
    pub fn extract_slice<'a>(&mut self, input: &'a [u8]) -> &'a [u8] {
        for &o in input {
            // Pull the bits; the octet itself is forwarded by value.
            let _ = self.extract(o);
        }
        input
    }

    /// Pop the next queued auxiliary bit (FIFO; `None` if empty).
    pub fn pop_aux_bit(&mut self) -> Option<bool> {
        self.queue.pop_front()
    }

    /// Drain up to `n` queued auxiliary bits into a freshly allocated
    /// vector. Returns the actual number of bits drained.
    pub fn drain_aux_bits(&mut self, n: usize) -> alloc::vec::Vec<bool> {
        let take = n.min(self.queue.len());
        let mut out = alloc::vec::Vec::with_capacity(take);
        for _ in 0..take {
            out.push(self.queue.pop_front().expect("queue length was checked"));
        }
        out
    }

    /// Number of auxiliary bits currently queued for the caller to
    /// drain.
    pub fn queued_bits(&self) -> usize {
        self.queue.len()
    }

    /// Drop every queued auxiliary bit.
    pub fn flush_queue(&mut self) {
        self.queue.clear();
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decoder, Encoder};

    #[test]
    fn aux_bits_per_octet_matches_table_1() {
        // Table 1/G.722 page 3: aux channel of 0 / 8 / 16 kbit/s for
        // Modes 1 / 2 / 3. The per-octet bit count is the channel
        // rate divided by the 8 kHz octet rate.
        assert_eq!(aux_bits_per_octet(Mode::Mode1), 0);
        assert_eq!(aux_bits_per_octet(Mode::Mode2), 1);
        assert_eq!(aux_bits_per_octet(Mode::Mode3), 2);
    }

    #[test]
    fn aux_bit_rate_kbps_matches_table_1() {
        assert_eq!(aux_bit_rate_kbps(Mode::Mode1), 0);
        assert_eq!(aux_bit_rate_kbps(Mode::Mode2), 8);
        assert_eq!(aux_bit_rate_kbps(Mode::Mode3), 16);
    }

    #[test]
    fn aux_bits_per_octet_agrees_with_decoder_mode_discard() {
        // The data-insertion device replaces exactly the LSBs the
        // decoder is going to discard in the matching mode (clause
        // 1.4.2 + Table 2 page 7).
        for m in [Mode::Mode1, Mode::Mode2, Mode::Mode3] {
            assert_eq!(aux_bits_per_octet(m), m.lsbs_to_discard());
        }
    }

    #[test]
    fn mode1_inserter_is_pass_through() {
        let mut ins = DataInserter::new(Mode::Mode1);
        // Even with bits queued, Mode 1 must not touch the octet.
        ins.push_aux_bits(&[true, false, true, true, false]);
        for raw in 0u8..=255 {
            assert_eq!(ins.insert(raw), raw, "mode-1 must not alter octets");
        }
        // Mode 1 does not drain the queue.
        assert_eq!(ins.queued_bits(), 5);
        assert_eq!(ins.aux_bits_drained(), 0);
        assert_eq!(ins.padding_bits_inserted(), 0);
    }

    #[test]
    fn mode2_inserter_substitutes_il6_only() {
        // Mode 2: 1 aux bit per octet at I_L6 = bit 0.
        let mut ins = DataInserter::new(Mode::Mode2);
        ins.push_aux_bits(&[true, false, true]);
        // Audio octet with every bit set:
        let o = ins.insert(0b1111_1111);
        // I_L6 -> 1, other bits preserved.
        assert_eq!(o, 0b1111_1111);
        let o = ins.insert(0b1111_1110);
        // Aux bit '0' lands in I_L6.
        assert_eq!(o, 0b1111_1110);
        let o = ins.insert(0b0000_0000);
        // Aux bit '1' replaces I_L6.
        assert_eq!(o, 0b0000_0001);
        assert_eq!(ins.aux_bits_drained(), 3);
        assert_eq!(ins.padding_bits_inserted(), 0);
        // No bit 1 (I_L5) touched by Mode 2.
        let o = ins.insert(0b0000_0010);
        // Queue exhausted, defaults to 0 padding for I_L6 only.
        assert_eq!(o & 0b1, 0);
        // I_L5 unchanged.
        assert_eq!(o & 0b10, 0b10);
    }

    #[test]
    fn mode3_inserter_substitutes_il5_and_il6_msb_first() {
        // Mode 3: 2 aux bits per octet. First queued bit -> I_L5
        // (bit 1, transmitted earlier), second -> I_L6 (bit 0).
        let mut ins = DataInserter::new(Mode::Mode3);
        ins.push_aux_bits(&[true, false]); // I_L5 = 1, I_L6 = 0
        let o = ins.insert(0b0000_0000);
        assert_eq!(o, 0b0000_0010, "I_L5 = 1, I_L6 = 0");
        ins.push_aux_bits(&[false, true]); // I_L5 = 0, I_L6 = 1
        let o = ins.insert(0b1111_1111);
        assert_eq!(o, 0b1111_1101, "I_L5 = 0, I_L6 = 1; rest preserved");
        assert_eq!(ins.aux_bits_drained(), 4);
    }

    #[test]
    fn inserter_pads_with_chosen_bit_when_queue_empty() {
        let mut ins = DataInserter::new(Mode::Mode3);
        ins.set_padding_bit(true);
        let o = ins.insert(0b0000_0000);
        // Both substituted positions filled with `1`.
        assert_eq!(o, 0b0000_0011);
        assert_eq!(ins.padding_bits_inserted(), 2);
        ins.set_padding_bit(false);
        let o = ins.insert(0b1111_1111);
        // Both substituted positions filled with `0`.
        assert_eq!(o, 0b1111_1100);
        assert_eq!(ins.padding_bits_inserted(), 4);
    }

    #[test]
    fn inserter_preserves_upper_six_bits() {
        // For an end-to-end set of octets the I_H1 I_H2 I_L1..I_L4
        // bits (positions 7..2) MUST survive the substitution.
        let mut ins = DataInserter::new(Mode::Mode3);
        ins.push_aux_bits(&alloc::vec![true; 512]);
        for raw in 0u8..=255 {
            let after = ins.insert(raw);
            assert_eq!(
                after & 0b1111_1100,
                raw & 0b1111_1100,
                "upper 6 bits of 0x{raw:02x} altered to 0x{after:02x}"
            );
        }
    }

    #[test]
    fn extractor_pops_bits_in_msb_first_order_for_mode3() {
        // 0b...10 in the LSBs => I_L5=1, I_L6=0 on the wire => first
        // pop is `true`, second is `false`.
        let mut ext = DataExtractor::new(Mode::Mode3);
        let _ = ext.extract(0b0000_0010);
        assert_eq!(ext.pop_aux_bit(), Some(true));
        assert_eq!(ext.pop_aux_bit(), Some(false));
        assert_eq!(ext.pop_aux_bit(), None);
        assert_eq!(ext.aux_bits_extracted(), 2);
    }

    #[test]
    fn extractor_pass_through_for_mode1() {
        let mut ext = DataExtractor::new(Mode::Mode1);
        for raw in 0u8..=255 {
            let o = ext.extract(raw);
            assert_eq!(o, raw);
        }
        assert_eq!(ext.queued_bits(), 0);
        assert_eq!(ext.aux_bits_extracted(), 0);
    }

    #[test]
    fn extractor_extracts_il6_in_mode2() {
        let mut ext = DataExtractor::new(Mode::Mode2);
        let _ = ext.extract(0b1111_1111);
        let _ = ext.extract(0b1111_1110);
        let _ = ext.extract(0b0000_0001);
        let bits = ext.drain_aux_bits(4);
        assert_eq!(bits, alloc::vec![true, false, true]);
    }

    #[test]
    fn extract_octet_is_unchanged() {
        // The wire octet is forwarded to the SB-ADPCM decoder
        // unchanged (the decoder discards the LSBs itself in INVQBL).
        let mut ext = DataExtractor::new(Mode::Mode3);
        for raw in [0u8, 0x55, 0xAA, 0xFF, 0x3C, 0x80, 0x7F] {
            assert_eq!(ext.extract(raw), raw);
        }
    }

    #[test]
    fn inserter_extractor_round_trip_mode2() {
        // The bits put in by the inserter must come out of the
        // extractor in the same order.
        let mut ins = DataInserter::new(Mode::Mode2);
        let mut ext = DataExtractor::new(Mode::Mode2);
        let payload: alloc::vec::Vec<bool> = (0..64).map(|i| i % 3 == 0).collect();
        ins.push_aux_bits(&payload);
        // The audio side: feed a tone-ish input through the encoder.
        let mut enc = Encoder::new();
        let pcm: alloc::vec::Vec<i32> = (0..128_i32).map(|i| 1000 * ((i % 8) - 4)).collect();
        let octets = enc.encode(&pcm);
        let wire = ins.insert_slice(&octets);
        for &o in &wire {
            let _ = ext.extract(o);
        }
        let out_bits = ext.drain_aux_bits(payload.len());
        assert_eq!(out_bits, payload);
        assert_eq!(ins.aux_bits_drained(), payload.len() as u64);
        assert_eq!(ext.aux_bits_extracted(), payload.len() as u64);
        // Padding kicks in once the payload is exhausted (octets - 64
        // remaining octets, each with 1 padded LSB).
        let leftover_octets = octets.len() as u64 - payload.len() as u64;
        assert_eq!(ins.padding_bits_inserted(), leftover_octets);
    }

    #[test]
    fn inserter_extractor_round_trip_mode3_lots_of_payload() {
        let mut ins = DataInserter::new(Mode::Mode3);
        let mut ext = DataExtractor::new(Mode::Mode3);
        let mut payload = alloc::vec::Vec::with_capacity(256);
        for i in 0..256 {
            payload.push(((i * 31) ^ 0x5A) & 1 == 1);
        }
        ins.push_aux_bits(&payload);
        let mut enc = Encoder::new();
        // 256 aux bits / 2 per octet = 128 octets minimum.
        let pcm: alloc::vec::Vec<i32> = (0..512_i32).map(|i| (i * 47 + 13) % 8000).collect();
        let octets = enc.encode(&pcm);
        let wire = ins.insert_slice(&octets);
        for &o in &wire {
            let _ = ext.extract(o);
        }
        let out_bits = ext.drain_aux_bits(payload.len());
        assert_eq!(out_bits, payload);
    }

    #[test]
    fn audio_side_round_trip_with_data_insertion_active() {
        // Substituting LSBs is supposed to be inaudible — the decoder
        // tracks the mode, so silence with random aux data on top
        // must still decode to a small envelope around zero.
        let mut enc = Encoder::new();
        let mut ins = DataInserter::new(Mode::Mode3);
        // Aux payload: 1024 alternating bits.
        let payload: alloc::vec::Vec<bool> = (0..1024).map(|i| i % 2 == 0).collect();
        ins.push_aux_bits(&payload);
        let octets = enc.encode(&[0_i32; 1024]);
        let wire = ins.insert_slice(&octets);
        let mut dec = Decoder::new(Mode::Mode3);
        let pcm = dec.decode(&wire);
        // Past the initial transient the LSB substitution must not
        // drive the predictor outside the silence envelope.
        for &s in &pcm[16..] {
            assert!(
                s.abs() <= 2048,
                "Mode-3 silence + aux substitution drifted: {s}"
            );
        }
    }

    #[test]
    fn inserter_queue_persists_across_mode_switch() {
        let mut ins = DataInserter::new(Mode::Mode3);
        ins.push_aux_bits(&[true, false, true, false]);
        // Switch to Mode 2 -> only 1 bit drained per octet.
        ins.set_mode(Mode::Mode2);
        let o = ins.insert(0b0000_0000);
        assert_eq!(o, 0b0000_0001, "Mode-2 drained 1 bit (true)");
        assert_eq!(ins.queued_bits(), 3);
        // Switch to Mode 1 -> no bits drained.
        ins.set_mode(Mode::Mode1);
        let _ = ins.insert(0xFF);
        assert_eq!(ins.queued_bits(), 3);
        // Switch back to Mode 3 -> 2 bits per octet drained.
        ins.set_mode(Mode::Mode3);
        let o = ins.insert(0b0000_0000);
        // Remaining queue: [false, true, false]; pop 2 -> false, true.
        assert_eq!(o, 0b0000_0001, "I_L5 = 0, I_L6 = 1");
        assert_eq!(ins.queued_bits(), 1);
    }

    #[test]
    fn inserter_padding_default_is_zero() {
        let mut ins = DataInserter::new(Mode::Mode2);
        // Queue empty; octet I_L6 forced to 0.
        let o = ins.insert(0b0000_0001);
        assert_eq!(o, 0b0000_0000);
        assert_eq!(ins.padding_bits_inserted(), 1);
    }

    #[test]
    fn flush_queue_drops_pending_bits() {
        let mut ins = DataInserter::new(Mode::Mode2);
        ins.push_aux_bits(&[true; 16]);
        assert_eq!(ins.queued_bits(), 16);
        ins.flush_queue();
        assert_eq!(ins.queued_bits(), 0);
        let _ = ins.insert(0x00);
        // Drains padding (= 0) because queue is empty.
        assert_eq!(ins.padding_bits_inserted(), 1);
    }

    #[test]
    fn insert_in_place_and_insert_into_match_insert_slice() {
        let mut ins_a = DataInserter::new(Mode::Mode3);
        let mut ins_b = DataInserter::new(Mode::Mode3);
        let mut ins_c = DataInserter::new(Mode::Mode3);
        let payload: alloc::vec::Vec<bool> = (0..256).map(|i| (i / 7) & 1 == 0).collect();
        ins_a.push_aux_bits(&payload);
        ins_b.push_aux_bits(&payload);
        ins_c.push_aux_bits(&payload);
        let mut buf = alloc::vec![0u8; 128];
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i * 37) as u8;
        }
        let by_slice = ins_a.insert_slice(&buf);
        let mut by_into = alloc::vec::Vec::new();
        ins_b.insert_into(&buf, &mut by_into);
        let mut by_inplace = buf.clone();
        ins_c.insert_in_place(&mut by_inplace);
        assert_eq!(by_slice, by_into);
        assert_eq!(by_slice, by_inplace);
    }

    #[test]
    fn data_extractor_mode_round_trips_set_mode() {
        let mut ext = DataExtractor::new(Mode::Mode1);
        assert_eq!(ext.mode(), Mode::Mode1);
        ext.set_mode(Mode::Mode2);
        assert_eq!(ext.mode(), Mode::Mode2);
        ext.set_mode(Mode::Mode3);
        assert_eq!(ext.mode(), Mode::Mode3);
    }

    #[test]
    fn drain_more_than_available_returns_what_is_there() {
        let mut ext = DataExtractor::new(Mode::Mode3);
        let _ = ext.extract(0b11);
        let bits = ext.drain_aux_bits(10);
        // Only 2 bits were extracted.
        assert_eq!(bits.len(), 2);
        assert!(bits[0]);
        assert!(bits[1]);
        assert_eq!(ext.queued_bits(), 0);
    }
}

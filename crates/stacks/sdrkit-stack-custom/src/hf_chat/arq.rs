//! The UTC slot grid of `hf_chat` (section 12), the one-way plan of a message, the acknowledged session at both ends (the
//! sender of blocks and the station that answers with an acknowledgement), and the profile suggestion.

use super::frame::{self, Ack};
use super::{Profile, PROFILES};

/// Seconds from a slot start to the first sample of a burst.
pub const START_DELAY_S: f64 = 1.5;
/// Sends of one block after which a sender gives up.
pub const MAX_ATTEMPTS: u32 = 8;
/// The SNR of a block error rate of 10 % on the moderate fading channel of each slotted profile, dB in 2.4 kHz (the
/// protocol's own table; profile 4 has the value of profile 5, so it is never picked over 5).
pub const THRESHOLDS_DB: [f64; 6] = [-19.1, -16.1, -12.6, -6.8, 0.0, 0.0];
/// The margin of the suggestion over the threshold, dB.
pub const MARGIN_DB: f64 = 2.0;
/// The slot of the push-to-talk profiles when they carry data, s.
pub const VOICE_DATA_SLOT_S: f64 = 10.0;

/// The slot length of profile `p`, s.
pub fn slot_s(p: &Profile) -> f64 {
    p.slot_s.unwrap_or(VOICE_DATA_SLOT_S)
}

/// The index of the slot of profile `p` that holds `utc` (seconds since 1970).
pub fn slot_index(p: &Profile, utc: f64) -> i64 {
    (utc / slot_s(p)).floor() as i64
}

/// The start of slot `index` of profile `p`, UTC s.
pub fn slot_start(p: &Profile, index: i64) -> f64 {
    index as f64 * slot_s(p)
}

/// The first slot of profile `p` of the caller (even) or the responder (odd) whose burst start (slot start + 1.5 s) is not
/// before `utc`: its index.
pub fn next_slot(p: &Profile, utc: f64, caller: bool) -> i64 {
    let mut i = slot_index(p, utc - START_DELAY_S);
    while slot_start(p, i) + START_DELAY_S < utc - 1e-9 || (i.rem_euclid(2) == 0) != caller {
        i += 1;
    }
    i
}

/// The clock error of a burst that starts at `utc`, s: its start less the nearest burst start of the slot grid of `p`.
/// Positive is late.
pub fn clock_error(p: &Profile, utc: f64) -> f64 {
    let s = slot_s(p);
    let e = (utc - START_DELAY_S).rem_euclid(s);
    if e > s / 2.0 { e - s } else { e }
}

/// The message id of a message sent at `utc`: `(utc / 7) mod 16`.
pub fn message_id(utc: f64) -> u8 {
    ((utc.max(0.0) as u64 / 7) % 16) as u8
}

/// The profile to suggest for a burst heard at `snr_db`: the fastest of profiles 0 to 5 with the SNR at least its threshold
/// plus 2 dB, else 0.
pub fn suggest(snr_db: f64) -> u8 {
    (0..6u8).rev().find(|&p| snr_db >= THRESHOLDS_DB[usize::from(p)] + MARGIN_DB).unwrap_or(0)
}

/// One burst of a plan: the slot index and the `K`-bit blocks it carries.
pub type PlannedBurst = (i64, Vec<Vec<u8>>);

/// The one-way plan of the message `bits` in profile `p` as message `id`, sent `passes` times: every caller slot from the
/// first one at or after `utc` carries up to blocks-per-slot consecutive blocks. `None` when the message needs more than 16
/// blocks.
pub fn one_way(bits: &[u8], p: &Profile, id: u8, passes: usize, utc: f64) -> Option<Vec<PlannedBurst>> {
    let blocks = frame::blocks(bits, id, p)?;
    let per = p.blocks_per_slot();
    let mut slot = next_slot(p, utc, true);
    let mut out = Vec::new();
    for _ in 0..passes.max(1) {
        for chunk in blocks.chunks(per) {
            out.push((slot, chunk.to_vec()));
            slot += 2;
        }
    }
    Some(out)
}

/// The sending end of an acknowledged session (section 12.4): each caller slot carries the lowest blocks not yet
/// acknowledged, up to blocks-per-slot; an acknowledgement marks blocks; a suggested profile that differs cuts the message
/// again in the new profile and forgets the marks; a block sent 8 times without an acknowledgement ends the session.
#[derive(Clone, Debug)]
pub struct Sender {
    bits: Vec<u8>,
    id: u8,
    profile: u8,
    blocks: Vec<Vec<u8>>,
    acked: Vec<bool>,
    attempts: Vec<u32>,
    /// Bursts sent.
    pub bursts: u32,
}

impl Sender {
    /// A session for the message `bits` as message `id` in profile `profile` (0 to 5). `None` when it needs more than 16
    /// blocks.
    pub fn new(bits: &[u8], id: u8, profile: u8) -> Option<Sender> {
        let p = PROFILES.get(usize::from(profile))?;
        let blocks = frame::blocks(bits, id, p)?;
        let n = blocks.len();
        Some(Sender { bits: bits.to_vec(), id, profile, blocks, acked: vec![false; n], attempts: vec![0; n], bursts: 0 })
    }

    /// The profile in use.
    pub fn profile(&self) -> &'static Profile {
        &PROFILES[usize::from(self.profile)]
    }

    /// The message id.
    pub fn id(&self) -> u8 {
        self.id
    }

    /// Every block is acknowledged.
    pub fn done(&self) -> bool {
        self.acked.iter().all(|&a| a)
    }

    /// A block went 8 times without an acknowledgement.
    pub fn failed(&self) -> bool {
        self.acked.iter().zip(&self.attempts).any(|(&a, &n)| !a && n >= MAX_ATTEMPTS)
    }

    /// The blocks of the next caller slot, each counted as one attempt; `None` when the session is over.
    pub fn next_burst(&mut self) -> Option<Vec<Vec<u8>>> {
        if self.done() || self.failed() {
            return None;
        }
        let per = self.profile().blocks_per_slot();
        let pick: Vec<usize> = (0..self.blocks.len()).filter(|&i| !self.acked[i]).take(per).collect();
        for &i in &pick {
            self.attempts[i] += 1;
        }
        self.bursts += 1;
        Some(pick.iter().map(|&i| self.blocks[i].clone()).collect())
    }

    /// Takes an acknowledgement. One of another message id is ignored. Returns true when it was taken.
    pub fn on_ack(&mut self, ack: &Ack) -> bool {
        if ack.id != self.id {
            return false;
        }
        if ack.profile <= 5 && ack.profile != self.profile {
            if let Some(blocks) = PROFILES.get(usize::from(ack.profile)).and_then(|p| frame::blocks(&self.bits, self.id, p)) {
                self.profile = ack.profile;
                self.acked = vec![false; blocks.len()];
                self.attempts = vec![0; blocks.len()];
                self.blocks = blocks;
                return true;
            }
        }
        for (i, a) in self.acked.iter_mut().enumerate() {
            *a |= ack.bitmap >> i & 1 == 1;
        }
        true
    }
}

/// The answering end of a session: the blocks held of each message, and the acknowledgement to send for one.
#[derive(Clone, Debug, Default)]
pub struct Responder {
    held: [(usize, u16); 16],
}

impl Responder {
    /// Takes block `index` of message `id`, of a profile with `k` information bits; blocks of another `k` replace those held.
    pub fn take(&mut self, id: u8, index: u8, k: usize) {
        let h = &mut self.held[usize::from(id & 15)];
        if h.0 != k {
            *h = (k, 0);
        }
        h.1 |= 1 << (index & 15);
    }

    /// The acknowledgement of message `id` from `callsign` for a burst heard at `snr_db`: every block held, the SNR, the
    /// suggested profile.
    pub fn ack(&self, callsign: &str, id: u8, snr_db: f64) -> Ack {
        Ack { callsign: callsign.to_string(), id: id & 15, bitmap: self.held[usize::from(id & 15)].1, snr_db: snr_db.round() as i32, profile: suggest(snr_db) }
    }

    /// Forgets every block.
    pub fn clear(&mut self) {
        *self = Responder::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hf_chat::frame::{parse_block, Block};

    #[test]
    fn slot_grid() {
        let p3 = &PROFILES[3];
        // Slot 5 of the 30 s grid starts at 150 s.
        assert_eq!(slot_index(p3, 151.5), 5);
        assert_eq!(slot_start(p3, 5), 150.0);
        assert_eq!(next_slot(p3, 151.0, false), 5);
        assert_eq!(next_slot(p3, 151.6, false), 7);
        assert_eq!(next_slot(p3, 151.0, true), 6);
        assert!((clock_error(p3, 151.75) - 0.25).abs() < 1e-9);
        assert!((clock_error(p3, 181.0) + 0.5).abs() < 1e-9);
        assert_eq!(message_id(70.0), 10);
    }

    #[test]
    fn suggestion_is_monotonic() {
        assert_eq!(suggest(-40.0), 0);
        assert_eq!(suggest(40.0), 5);
        let mut last = 0;
        for i in -400..400 {
            let s = suggest(f64::from(i) / 10.0);
            assert!(s >= last);
            last = s;
        }
        assert_eq!(suggest(-4.8), 3);
    }

    #[test]
    fn plan_of_a_chat_in_profile_0() {
        let chat = frame::Message::Chat(frame::Chat {
            from: "LY1ABC".into(),
            to: frame::Recipient::Call("CQ".into()),
            raw: false,
            more: false,
            seq: 0,
            ack: 0,
            text: "CQ".into(),
        })
        .bits()
        .unwrap();
        let plan = one_way(&chat, &PROFILES[0], 3, 2, 1000.0).unwrap();
        // 139 bits: 2 blocks of profile 0, one a caller slot, twice: 4 bursts 240 s apart.
        assert_eq!(plan.len(), 4);
        assert_eq!(plan[0].0 % 2, 0);
        assert_eq!(plan[1].0 - plan[0].0, 2);
        assert!(slot_start(&PROFILES[0], plan[0].0) + START_DELAY_S >= 1000.0);
    }

    #[test]
    fn a_session_moves_to_the_suggested_profile_and_completes() {
        let data = frame::Message::Data((0..40u8).collect()).bits().unwrap();
        let mut tx = Sender::new(&data, 5, 0).unwrap();
        assert_eq!(tx.blocks.len(), 4);
        let mut rx = Responder::default();
        let mut asm = frame::Assembler::default();
        let mut done = None;
        let snr = -4.0;
        for round in 0..20 {
            let Some(sent) = tx.next_burst() else {
                break;
            };
            for b in &sent {
                let Some(Block::Part { id, index, payload }) = parse_block(b) else { panic!("block") };
                rx.take(id, index, tx.profile().k);
                if let frame::Taken::Complete(_, m) = asm.take(id, index, &payload, tx.profile().k, f64::from(round)) {
                    done = m;
                }
            }
            // The answer suggests profile 3 at -4 dB.
            let ack = rx.ack("DL2XYZ", 5, snr);
            assert_eq!(ack.profile, 3);
            tx.on_ack(&ack);
        }
        assert!(tx.done());
        assert_eq!(tx.profile().id, 3);
        assert_eq!(done, Some(frame::Message::Data((0..40u8).collect())));
        // A sender with no acknowledgements gives up after 8 attempts.
        let mut lone = Sender::new(&data, 5, 3).unwrap();
        let mut n = 0;
        while lone.next_burst().is_some() {
            n += 1;
        }
        assert_eq!(n, 8);
        assert!(lone.failed());
    }
}

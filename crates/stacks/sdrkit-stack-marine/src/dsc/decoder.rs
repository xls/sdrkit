//! The DSC bit decoder: the phasing hunt over ten bit phases, slot collection, the abort rules, and the merge of the DX and RX
//! copies with the repair the error-check character allows. ATIS uses it too.
//!
//! Standard: ITU-R M.493-15 section 3 and Annex 1

use super::symbols::{self, EOS};

/// A received call before parsing: the merged symbols and what the merge did.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawCall {
    pub format: Option<u8>,
    /// Message symbols between the format specifier and the EOS. `None` for an unknown one.
    pub message: Vec<Option<u8>>,
    pub eos: Option<u8>,
    pub ecc: Option<u8>,
    pub check_ok: bool,
    /// Symbols unreadable in both copies.
    pub erasures: usize,
    /// Symbols taken from the second copy or rebuilt.
    pub repaired: usize,
    /// Symbols rebuilt with the ECC.
    pub recovered: usize,
    /// The bit index (of the decoder's input) where the phasing locked.
    pub start_bit: u64,
    /// The bit index of the end.
    pub end_bit: u64,
    /// The call was cut short.
    pub cut_short: bool,
    /// Decoded from inverted bits.
    pub inverted: bool,
}

impl RawCall {
    /// The message symbols with unknowns as 0xFF.
    pub fn symbols(&self) -> Vec<u8> {
        self.message.iter().map(|s| s.unwrap_or(0xFF)).collect()
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Sym {
    value: Option<u8>,
    certainty: f32,
}

#[derive(Clone, Debug, Default)]
struct Locked {
    /// The bit phase (bit count mod 10 at which a symbol ends).
    phase: usize,
    /// Slots received so far.
    slot: usize,
    dx: Vec<Sym>,
    rx: Vec<Sym>,
    unreadable: std::collections::VecDeque<bool>,
    end_dx: Option<usize>,
    start_bit: u64,
}

/// Symbols kept per bit phase while hunting.
const HUNT: usize = 24;

/// The decoder of one polarity.
#[derive(Clone, Debug)]
pub struct DscDecoder {
    bits: [f32; 10],
    count: u64,
    /// Per bit phase, the last symbols (0xFF when unreadable) with their certainty.
    hist: Vec<std::collections::VecDeque<(u8, f32)>>,
    locked: Option<Locked>,
    invert: bool,
    calls: Vec<RawCall>,
    /// Calls that decoded with a good check, so far.
    pub good: u64,
}

impl DscDecoder {
    /// A decoder. `invert` decodes the bits inverted (tones swapped).
    pub fn new(invert: bool) -> Self {
        DscDecoder {
            bits: [0.0; 10],
            count: 0,
            hist: vec![std::collections::VecDeque::with_capacity(HUNT + 1); 10],
            locked: None,
            invert,
            calls: Vec::new(),
            good: 0,
        }
    }

    /// Whether the phasing is locked.
    pub fn locked(&self) -> bool {
        self.locked.is_some()
    }

    /// Starts again.
    pub fn reset(&mut self) {
        *self = DscDecoder::new(self.invert);
    }

    /// Takes the calls finished so far.
    pub fn take_calls(&mut self) -> Vec<RawCall> {
        std::mem::take(&mut self.calls)
    }

    /// Feeds soft bits: positive is 1 (Y), negative 0 (B).
    pub fn push(&mut self, soft: &[f32]) {
        for &s in soft {
            let s = if self.invert { -s } else { s };
            self.bits.copy_within(1.., 0);
            self.bits[9] = if s.is_finite() { s } else { 0.0 };
            self.count += 1;
            if self.count < 10 {
                continue;
            }
            let hard: [u8; 10] = std::array::from_fn(|i| u8::from(self.bits[i] > 0.0));
            let sym = symbols::decode(&hard);
            let cert = self.bits.iter().fold(f32::INFINITY, |a, b| a.min(b.abs()));
            let phase = (self.count % 10) as usize;
            if let Some(l) = &mut self.locked {
                if l.phase != phase {
                    continue;
                }
                let done = Self::slot(
                    l,
                    Sym {
                        value: sym,
                        certainty: cert,
                    },
                );
                if let Some(cut) = done {
                    let l = self.locked.take();
                    let call = l
                        .as_ref()
                        .and_then(|l| Self::finish(l, cut, self.count, self.invert));
                    if let Some(c) = call {
                        if c.check_ok {
                            self.good += 1;
                        }
                        self.calls.push(c);
                    }
                    for h in &mut self.hist {
                        h.clear();
                    }
                }
                continue;
            }
            let h = &mut self.hist[phase];
            h.push_back((sym.unwrap_or(0xFF), cert));
            if h.len() > HUNT {
                h.pop_front();
            }
            if let Some(start_slot) = Self::hunt(h) {
                // The newest symbol is slot `start_slot`. Collect from there.
                let n = h.len();
                let mut l = Locked {
                    phase,
                    slot: 0,
                    start_bit: self.count.saturating_sub(10 * n as u64),
                    ..Default::default()
                };
                // Replay the symbols of this phase from the first slot found, with their own certainty.
                let first = n - 1 - start_slot;
                for &(v, c) in h.iter().skip(first) {
                    let s = Sym {
                        value: (v != 0xFF).then_some(v),
                        certainty: c,
                    };
                    let _ = Self::slot(&mut l, s);
                }
                self.locked = Some(l);
                for hh in &mut self.hist {
                    hh.clear();
                }
            }
        }
    }

    /// Looks for at least 3 phasing symbols in their places relative to each other. Returns the slot number of the newest symbol.
    fn hunt(h: &std::collections::VecDeque<(u8, f32)>) -> Option<usize> {
        let n = h.len();
        let mut best: Option<(usize, usize)> = None;
        // The newest symbol is at slot s_new. Slot 0 is at position n-1-s_new.
        for s_new in 0..(n + 16).min(40) {
            if s_new + 1 > n + 16 {
                break;
            }
            let mut count = 0;
            let mut rx = 0;
            for (q, &(v, _)) in h.iter().enumerate() {
                let slot = s_new as isize - (n - 1 - q) as isize;
                if !(0..=15).contains(&slot) {
                    continue;
                }
                if let Some(p) = symbols::phasing(slot as usize) {
                    if (slot >= 12 && slot % 2 == 0) || p != v {
                        continue;
                    }
                    count += 1;
                    if slot % 2 == 1 {
                        rx += 1;
                    }
                }
            }
            // A run of DX phasing alone fits many alignments: require an RX phasing symbol, or the later ones.
            if count >= 3 && rx >= 1 && best.is_none_or(|b| count > b.1) {
                best = Some((s_new, count));
            }
        }
        best.map(|b| b.0)
    }

    /// Takes one slot. Returns `Some(cut_short)` when the call is complete or aborted.
    fn slot(l: &mut Locked, s: Sym) -> Option<bool> {
        let slot = l.slot;
        l.slot += 1;
        l.unreadable.push_back(s.value.is_none());
        if l.unreadable.len() > 30 {
            l.unreadable.pop_front();
        }
        if slot >= 12 && slot.is_multiple_of(2) {
            let i = (slot - 12) / 2;
            if l.dx.len() <= i {
                l.dx.resize(i + 1, Sym::default());
            }
            l.dx[i] = s;
        } else if slot >= 17 && slot % 2 == 1 {
            let i = (slot - 17) / 2;
            if l.rx.len() <= i {
                l.rx.resize(i + 1, Sym::default());
            }
            l.rx[i] = s;
        }
        // Find the end: the first EOS among the message symbols (index 2 on).
        if l.end_dx.is_none() {
            let n = l.dx.len().max(l.rx.len());
            for i in 2..n {
                let v =
                    l.dx.get(i)
                        .and_then(|s| s.value)
                        .or_else(|| l.rx.get(i).and_then(|s| s.value));
                if let Some(v) = v {
                    if EOS.contains(&v) {
                        // Confirm with a second EOS two DX places later when it has arrived, or with the RX copy.
                        l.end_dx = Some(i);
                        break;
                    }
                }
            }
        }
        if let Some(e) = l.end_dx {
            // Wait for the RX copy of the ECC: slot 17 + 2 (e + 1).
            if slot >= 17 + 2 * (e + 1) {
                return Some(false);
            }
        }
        if l.slot > 12 + 280 {
            return Some(true);
        }
        if l.slot >= 60 && l.unreadable.iter().filter(|u| **u).count() >= 24 {
            return Some(true);
        }
        None
    }

    fn finish(l: &Locked, cut: bool, end_bit: u64, inverted: bool) -> Option<RawCall> {
        let (e, cut) = match l.end_dx {
            Some(e) if !cut => (e, false),
            _ => {
                if l.slot < 12 + 16 + 30 {
                    return None;
                }
                // Cut short: the last good slot.
                let n = l.dx.len().max(l.rx.len());
                let mut last = 2;
                for i in 2..n {
                    if l.dx.get(i).and_then(|s| s.value).is_some()
                        || l.rx.get(i).and_then(|s| s.value).is_some()
                    {
                        last = i;
                    }
                }
                (last + 1, true)
            }
        };
        let get = |v: &Vec<Sym>, i: usize| v.get(i).copied().unwrap_or_default();
        // Positions: 0 format (4 copies), 1..e-2 message (indices 2..e), e-1 EOS, e ECC.
        struct Pos {
            cands: Vec<(u8, f32)>,
        }
        let mut pos: Vec<Pos> = Vec::new();
        let cands_of = |copies: &[Sym]| {
            let mut c: Vec<(u8, f32)> = Vec::new();
            for s in copies {
                if let Some(v) = s.value {
                    match c.iter_mut().find(|x| x.0 == v) {
                        Some(x) => x.1 += s.certainty,
                        None => c.push((v, s.certainty)),
                    }
                }
            }
            c
        };
        pos.push(Pos {
            cands: cands_of(&[get(&l.dx, 0), get(&l.dx, 1), get(&l.rx, 0), get(&l.rx, 1)]),
        });
        for i in 2..e {
            pos.push(Pos {
                cands: cands_of(&[get(&l.dx, i), get(&l.rx, i)]),
            });
        }
        if !cut {
            pos.push(Pos {
                cands: cands_of(&[
                    get(&l.dx, e),
                    get(&l.rx, e),
                    get(&l.dx, e + 2),
                    get(&l.dx, e + 3),
                ]),
            });
            pos.push(Pos {
                cands: cands_of(&[get(&l.dx, e + 1), get(&l.rx, e + 1)]),
            });
        }
        // First choices: the DX copy when readable (for the format: the most common), else the RX copy.
        let first: Vec<Option<u8>> = pos
            .iter()
            .enumerate()
            .map(|(k, p)| {
                if k == 0 {
                    p.cands
                        .iter()
                        .max_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|c| c.0)
                } else {
                    p.cands.first().map(|c| c.0)
                }
            })
            .collect();
        let erasures = first.iter().filter(|f| f.is_none()).count();
        // Second copy used when the DX copy was unreadable.
        let mut repaired = 0usize;
        for i in 2..e {
            if l.dx.get(i).and_then(|s| s.value).is_none()
                && l.rx.get(i).and_then(|s| s.value).is_some()
            {
                repaired += 1;
            }
        }
        let mut chosen = first.clone();
        let mut ok = false;
        let mut recovered = 0;
        if !cut {
            let parity = |c: &[Option<u8>]| -> Option<u8> {
                let mut x = 0u8;
                for v in c {
                    x ^= (*v)?;
                }
                Some(x & 0x7F)
            };
            // ECC: format ^ message ^ eos ^ ecc == 0.
            if parity(&chosen) == Some(0) {
                ok = true;
            } else {
                let unknown: Vec<usize> =
                    (0..chosen.len()).filter(|&k| chosen[k].is_none()).collect();
                if unknown.len() == 1 {
                    let k = unknown[0];
                    let mut x = 0u8;
                    for (j, v) in chosen.iter().enumerate() {
                        if j != k {
                            x ^= v.unwrap_or(0);
                        }
                    }
                    chosen[k] = Some(x & 0x7F);
                    ok = true;
                    recovered = 1;
                    repaired += 1;
                } else if unknown.is_empty() {
                    // Fewest changes, least certain positions first: a change at position k moves the parity by `first ^ other`.
                    let mut alt: Vec<(usize, u8, u8)> = (0..pos.len())
                        .filter_map(|k| {
                            let f = chosen[k]?;
                            let o = pos[k].cands.iter().find(|x| x.0 != f)?.0;
                            Some((k, o, f ^ o))
                        })
                        .collect();
                    let least = |k: usize| {
                        pos[k]
                            .cands
                            .iter()
                            .map(|c| c.1)
                            .fold(f32::INFINITY, f32::min)
                    };
                    alt.sort_by(|a, b| least(a.0).total_cmp(&least(b.0)));
                    alt.truncate(16);
                    let p = parity(&chosen).unwrap_or(0);
                    if let Some(mask) = fewest_changes(&alt, p) {
                        for (i, &(k, o, _)) in alt.iter().enumerate() {
                            if mask & (1 << i) != 0 {
                                chosen[k] = Some(o);
                            }
                        }
                        ok = true;
                        repaired += mask.count_ones() as usize;
                    }
                }
            }
        }
        let n_msg = e.saturating_sub(2);
        Some(RawCall {
            format: chosen[0],
            message: chosen[1..1 + n_msg.min(chosen.len().saturating_sub(1))].to_vec(),
            eos: if cut {
                None
            } else {
                chosen.get(1 + n_msg).copied().flatten()
            },
            ecc: if cut {
                None
            } else {
                chosen.get(2 + n_msg).copied().flatten()
            },
            check_ok: ok && !cut,
            erasures,
            repaired,
            recovered,
            start_bit: l.start_bit,
            end_bit,
            cut_short: cut,
            inverted,
        })
    }
}

/// The set of at most three changes, fewest first and then in increasing mask order, whose parity moves (`alt[i].2`) together
/// cancel the parity `p`. `None` when no such set exists.
fn fewest_changes(alt: &[(usize, u8, u8)], p: u8) -> Option<u32> {
    let n = alt.len();
    for changes in 1..=n.min(3) as u32 {
        for mask in 0u32..(1u32 << n) {
            if mask.count_ones() != changes {
                continue;
            }
            let mut x = p;
            for (i, a) in alt.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    x ^= a.2;
                }
            }
            if x & 0x7F == 0 {
                return Some(mask);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsc::symbols::{call_bits, encode, mmsi_symbols};

    fn soft(bits: &[u8]) -> Vec<f32> {
        bits.iter()
            .map(|&b| if b == 1 { 1.0 } else { -1.0 })
            .collect()
    }

    fn distress() -> Vec<u8> {
        let mut m = mmsi_symbols("232123456");
        m.extend_from_slice(&[103, 15, 5, 10, 1, 18, 12, 34, 109]);
        m
    }

    #[test]
    fn a_call_round_trips_at_any_offset() {
        let m = distress();
        for off in 0..10 {
            let mut b: Vec<u8> = (0..off).map(|i| (i % 2) as u8).collect();
            b.extend(call_bits(200, 112, &m, 127));
            b.extend((0..40).map(|i| (i % 2) as u8));
            let mut d = DscDecoder::new(false);
            d.push(&soft(&b));
            let c = d.take_calls();
            assert_eq!(c.len(), 1, "offset {off}");
            assert!(c[0].check_ok);
            assert_eq!(c[0].format, Some(112));
            assert_eq!(c[0].symbols(), m);
            // The inverted decoder finds nothing in these bits.
            let mut di = DscDecoder::new(true);
            di.push(&soft(&b));
            assert!(di.take_calls().iter().all(|c| !c.check_ok));
            // Inverted bits decode only with the inverted decoder.
            let inv: Vec<u8> = b.iter().map(|x| 1 - x).collect();
            let mut di = DscDecoder::new(true);
            di.push(&soft(&inv));
            assert!(di.take_calls()[0].check_ok);
        }
    }

    #[test]
    fn repair_behaviour() {
        let m = distress();
        let base = call_bits(20, 112, &m, 127);
        let bit_of_slot = |s: usize| 20 + 10 * s;
        // Message symbol index 3 (DX slot 16 + 2*3) and its RX copy (slot 21 + 2*3).
        let dx = 16 + 2 * 3;
        let rx = dx + 5;
        // A single bit error in both copies: rebuilt by the ECC.
        let mut b = base.clone();
        b[bit_of_slot(dx) + 2] ^= 1;
        b[bit_of_slot(rx) + 4] ^= 1;
        let mut d = DscDecoder::new(false);
        d.push(&soft(&b));
        let c = &d.take_calls()[0];
        assert!(c.check_ok && c.repaired >= 1 && c.recovered == 1, "{c:?}");
        assert_eq!(c.symbols(), m);
        // Two lost in both copies: failed check with erasures.
        let dx2 = 16 + 2 * 5;
        b[bit_of_slot(dx2)] ^= 1;
        b[bit_of_slot(dx2 + 5)] ^= 1;
        let mut d = DscDecoder::new(false);
        d.push(&soft(&b));
        let c = &d.take_calls()[0];
        assert!(!c.check_ok && c.erasures >= 1);
        // Two conflicting copies (each valid, different), resolved by the ECC.
        let mut b = base.clone();
        let wrong = encode(m[3] ^ 0x03);
        b[bit_of_slot(dx)..bit_of_slot(dx) + 10].copy_from_slice(&wrong);
        let wrong2 = encode(m[6] ^ 0x05);
        b[bit_of_slot(16 + 2 * 6)..bit_of_slot(16 + 2 * 6) + 10].copy_from_slice(&wrong2);
        let mut d = DscDecoder::new(false);
        d.push(&soft(&b));
        let c = &d.take_calls()[0];
        assert!(c.check_ok, "{c:?}");
        assert_eq!(c.repaired, 2);
        assert_eq!(c.symbols(), m);
        // An error in both copies the copies cannot see (both valid and equal, wrong): flagged by the ECC.
        let mut b = base.clone();
        let w = encode(m[3] ^ 0x01);
        b[bit_of_slot(dx)..bit_of_slot(dx) + 10].copy_from_slice(&w);
        b[bit_of_slot(rx)..bit_of_slot(rx) + 10].copy_from_slice(&w);
        let mut d = DscDecoder::new(false);
        d.push(&soft(&b));
        assert!(!d.take_calls()[0].check_ok);
    }
}

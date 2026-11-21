//! The 2-FSK signals of the stack: the tone pair from the averaged spectrum with its lock, one demodulator per candidate speed
//! with its own symbol clock, and the framers of `bee` (CIS-36-50 and CIS-50-50) and `cis14` (CIS-14).

use super::{Message, Out, Sub};
use crate::typed::{CisMessageEvent, CIS_MESSAGE};
use num_complex::Complex64;
use sdrkit_algo_dsp::bandscan::BandScan;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The rate of the channel.
const RATE: f64 = super::RATE;

/// The end-of-message character.
pub const EOM: u8 = 0x77;

/// Whether a 7-bit character has exactly three 1 bits.
pub fn valid(c: u8) -> bool {
    c < 0x80 && c.count_ones() == 3
}

/// The tone pair with its lock (9.2): looks on every new frame of the averaged spectrum.
pub struct TonePair {
    scan: BandScan,
    shift_set: f64,
    /// The lock: centre and shift, Hz.
    pub lock: Option<(f64, f64)>,
    last: Option<(f64, f64)>,
    far: Vec<(f64, f64)>,
    fails: u32,
    /// The noise floor of the newest look.
    pub noise: f32,
    /// The SNR of the pair, dB.
    pub snr_db: Option<f64>,
}

impl TonePair {
    /// A tone pair with a known shift `shift_hz` (0 for none).
    pub fn new(shift_hz: f64) -> Self {
        TonePair {
            scan: BandScan::new(4096, RATE, 0.25).expect("a valid spectrum"),
            shift_set: shift_hz,
            lock: None,
            last: None,
            far: Vec::new(),
            fails: 0,
            noise: 0.0,
            snr_db: None,
        }
    }

    /// One look at the spectrum: the centre and the shift, if a pair stands out.
    fn look(&mut self) -> Option<(f64, f64)> {
        let noise = self.scan.noise(-5900.0, 5900.0);
        self.noise = noise;
        let (f1, p1) = self.scan.peak(150.0, 5500.0)?;
        if p1 < 12.0 * noise {
            return None;
        }
        if self.shift_set > 0.0 {
            let s = self.shift_set;
            let step = self.scan.bin_hz();
            let mut best = (f1, 0.0f64);
            let mut c = f1 - s - 60.0;
            while c <= f1 + s + 60.0 {
                let p = self.scan.band_power(c - s / 2.0 - 30.0, c - s / 2.0 + 30.0)
                    + self.scan.band_power(c + s / 2.0 - 30.0, c + s / 2.0 + 30.0);
                if p > best.1 {
                    best = (c, p);
                }
                c += step;
            }
            return Some((best.0, s));
        }
        let lo = (f1 - 1100.0).max(100.0);
        let hi = (f1 + 1100.0).min(5900.0);
        let below = self.scan.peak(lo, f1 - 55.0);
        let above = self.scan.peak(f1 + 55.0, hi);
        let (f2, p2) = match (below, above) {
            (Some(a), Some(b)) => {
                if a.1 >= b.1 {
                    a
                } else {
                    b
                }
            }
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => return None,
        };
        if p2 < 6.0 * noise || p2 < 0.08 * p1 {
            return None;
        }
        Some((0.5 * (f1 + f2), (f1 - f2).abs()))
    }

    /// Takes samples; returns true when a look was made.
    pub fn push(&mut self, x: &[Complex32]) -> bool {
        let frames = self.scan.push(x);
        if frames == 0 || self.scan.frames() < 3 {
            return false;
        }
        let look = self.look();
        match (self.lock, look) {
            (_, None) => {
                self.fails += 1;
                self.last = None;
                if self.fails >= 7 {
                    self.lock = None;
                    self.far.clear();
                }
            }
            (None, Some(l)) => {
                self.fails = 0;
                if let Some(p) = self.last {
                    if (p.0 - l.0).abs() <= 6.0 && (p.1 - l.1).abs() <= 8.0 {
                        self.lock = Some(l);
                    }
                }
                self.last = Some(l);
            }
            (Some(k), Some(l)) => {
                self.fails = 0;
                self.last = Some(l);
                if (k.0 - l.0).abs() > 12.0 || (k.1 - l.1).abs() > 15.0 {
                    if self
                        .far
                        .last()
                        .is_some_and(|f| (f.0 - l.0).abs() > 6.0 || (f.1 - l.1).abs() > 8.0)
                    {
                        self.far.clear();
                    }
                    self.far.push(l);
                    if self.far.len() >= 3 {
                        self.lock = Some(l);
                        self.far.clear();
                    }
                } else {
                    self.far.clear();
                    self.lock = Some((k.0 + 0.1 * (l.0 - k.0), k.1 + 0.1 * (l.1 - k.1)));
                }
            }
        }
        if let Some((c, s)) = self.lock {
            let lo = c - s / 2.0 - 40.0;
            let hi = c + s / 2.0 + 40.0;
            self.snr_db = Some(self.scan.snr_db(lo, hi, self.noise));
        } else {
            self.snr_db = None;
        }
        true
    }
}

/// A demodulator at one speed: sliding sums of the two mixed tones and a symbol clock.
pub struct Demod {
    /// The speed, Bd.
    pub baud: f64,
    len: usize,
    lo: Vec<Complex64>,
    hi: Vec<Complex64>,
    at: usize,
    sum_lo: Complex64,
    sum_hi: Complex64,
    filled: usize,
    phase: f64,
    last_d: f64,
    changes: u32,
    /// The eye: `|d|` averaged.
    pub eye: f64,
}

impl Demod {
    /// A demodulator at `baud`.
    pub fn new(baud: f64) -> Self {
        let len = ((RATE / baud).round() as usize).max(2);
        Demod {
            baud,
            len,
            lo: vec![Complex64::default(); len],
            hi: vec![Complex64::default(); len],
            at: 0,
            sum_lo: Complex64::default(),
            sum_hi: Complex64::default(),
            filled: 0,
            phase: 0.0,
            last_d: 0.0,
            changes: 0,
            eye: 0.0,
        }
    }

    /// Takes the two mixed samples; returns a bit (1 when the higher tone is stronger) when the clock wraps.
    pub fn push(&mut self, lo: Complex64, hi: Complex64) -> Option<u8> {
        self.sum_lo += lo - self.lo[self.at];
        self.sum_hi += hi - self.hi[self.at];
        self.lo[self.at] = lo;
        self.hi[self.at] = hi;
        self.at = (self.at + 1) % self.len;
        if self.at == 0 {
            // Sum again from the window now and then, so rounding does not build up.
            self.sum_lo = self.lo.iter().sum();
            self.sum_hi = self.hi.iter().sum();
        }
        if self.filled < self.len {
            self.filled += 1;
            return None;
        }
        let (el, eh) = (self.sum_lo.norm_sqr(), self.sum_hi.norm_sqr());
        let d = (eh - el) / (eh + el).max(1e-30);
        self.eye += 0.03 * (d.abs() - self.eye);
        if d * self.last_d < 0.0 {
            let g = if self.changes < 30 { 0.35 } else { 0.08 };
            self.changes += 1;
            self.phase += g * (0.5 - self.phase);
        }
        self.last_d = d;
        self.phase += self.baud / RATE;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            return Some(u8::from(d > 0.0));
        }
        None
    }
}

/// The 2-FSK front: the tone pair, the mixers and the demodulators.
pub struct FskFront {
    /// The tone pair.
    pub pair: TonePair,
    /// The demodulators.
    pub demods: Vec<Demod>,
    n: u64,
    rot_lo: Complex64,
    rot_hi: Complex64,
}

impl FskFront {
    /// A front with demodulators at `bauds` and a known shift `shift_hz` (0 for none).
    pub fn new(bauds: &[f64], shift_hz: f64) -> Self {
        FskFront {
            pair: TonePair::new(shift_hz),
            demods: bauds.iter().map(|&b| Demod::new(b)).collect(),
            n: 0,
            rot_lo: Complex64::new(1.0, 0.0),
            rot_hi: Complex64::new(1.0, 0.0),
        }
    }

    /// Takes one sample; appends `(demodulator, bit)` to `bits` for every bit decided.
    pub fn push(&mut self, x: Complex32, bits: &mut Vec<(usize, u8)>) {
        self.pair.push(std::slice::from_ref(&x));
        self.n += 1;
        let Some((c, s)) = self.pair.lock else {
            return;
        };
        let x = Complex64::new(f64::from(x.re), f64::from(x.im));
        let wl = -TAU * (c - s / 2.0) / RATE;
        let wh = -TAU * (c + s / 2.0) / RATE;
        self.rot_lo *= Complex64::from_polar(1.0, wl);
        self.rot_hi *= Complex64::from_polar(1.0, wh);
        if self.n.is_multiple_of(1024) {
            self.rot_lo /= self.rot_lo.norm();
            self.rot_hi /= self.rot_hi.norm();
        }
        let (lo, hi) = (x * self.rot_lo, x * self.rot_hi);
        for (i, d) in self.demods.iter_mut().enumerate() {
            if let Some(b) = d.push(lo, hi) {
                bits.push((i, b));
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// bee: CIS-36-50 and CIS-50-50

/// The bits of a message start: the start word, then the 70-bit key twice.
const START_BITS: usize = 44 + 140;

#[derive(Clone, Debug, PartialEq)]
enum BeeState {
    Search,
    /// A start passed: the best (score, bits after it, polarity) and the bits still to wait.
    Held {
        best: (i32, usize, bool),
        wait: u32,
    },
    Message,
    After,
}

/// The framer of one demodulator.
struct BeeFramer {
    baud: f64,
    bits: Vec<u8>,
    state: BeeState,
    inverted: bool,
    msg: Option<Message>,
    eom: u32,
    invalid: u32,
    after_invalid: u32,
}

/// The key characters of a start: per place, the valid copy or the first in brackets.
fn key_text(k1: &[u8], k2: &[u8]) -> (String, String) {
    let mut shown = Vec::new();
    let mut hex = String::new();
    for i in 0..10 {
        let (a, b) = (k1[i], k2[i]);
        let c = if valid(a) { a } else { b };
        hex.push_str(&format!("{c:02X}"));
        if valid(a) || valid(b) {
            shown.push(format!("{c:02X}"));
        } else {
            shown.push(format!("[{a:02X}]"));
        }
    }
    (shown.join(" "), hex)
}

/// The 7-bit characters (most significant bit first) of `bits`.
fn chars_of(bits: &[u8]) -> Vec<u8> {
    bits.as_chunks::<7>()
        .0
        .iter()
        .map(|c| c.iter().fold(0u8, |v, &b| (v << 1) | b))
        .collect()
}

impl BeeFramer {
    fn new(baud: f64) -> Self {
        BeeFramer {
            baud,
            bits: Vec::new(),
            state: BeeState::Search,
            inverted: false,
            msg: None,
            eom: 0,
            invalid: 0,
            after_invalid: 0,
        }
    }

    /// The score of a start in the 184 bits that end the buffer, with polarity `inv`.
    fn start_score(&self, inv: bool) -> Option<i32> {
        let n = self.bits.len();
        if n < START_BITS {
            return None;
        }
        let w: Vec<u8> = self.bits[n - START_BITS..]
            .iter()
            .map(|&b| b ^ u8::from(inv))
            .collect();
        let ones = w[..44].iter().filter(|&&b| b == 1).count() as i32;
        if !(20..=22).contains(&ones) {
            return None;
        }
        let diff = w[44..114]
            .iter()
            .zip(&w[114..184])
            .filter(|(a, b)| a != b)
            .count() as i32;
        if diff > 2 {
            return None;
        }
        let k1 = chars_of(&w[44..114]);
        let k2 = chars_of(&w[114..184]);
        let either = (0..10).filter(|&i| valid(k1[i]) || valid(k2[i])).count();
        if either < 8 {
            return None;
        }
        let both = (0..10)
            .map(|i| i32::from(valid(k1[i])) + i32::from(valid(k2[i])))
            .sum::<i32>();
        Some(4 * both - 3 * diff - (ones - 21).abs())
    }

    fn begin(&mut self, out: &mut Out, follow: bool, start: &[u8]) {
        let mut m = Message::new(out.time, self.baud);
        if follow {
            m.text.push_str("[follow-on]");
        } else {
            let w: Vec<u8> = start.to_vec();
            let word = w[..44].iter().fold(0u64, |v, &b| (v << 1) | u64::from(b));
            m.start_word = format!("{word:011X}");
            let (shown, hex) = key_text(&chars_of(&w[44..114]), &chars_of(&w[114..184]));
            m.key = hex;
            m.text.push_str(&format!("[start, key {shown}]"));
        }
        out.chars.push_str(&m.text);
        self.msg = Some(m);
        self.state = BeeState::Message;
        self.eom = 0;
        self.invalid = 0;
    }

    fn end(&mut self, out: &mut Out, by: &str) {
        if let Some(mut m) = self.msg.take() {
            let tail = if by == "end of message" {
                " <EOM>\n"
            } else {
                " <signal lost>\n"
            };
            out.chars.push_str(tail);
            m.ended_by = by.to_string();
            let text = format!(
                "CIS-36-50 message at {:.0} Bd: {} characters, {} failing the 3:4 check, {by}",
                self.baud, m.chars, m.bad
            );
            out.events.push(CIS_MESSAGE.event(
                out.time,
                out.dial,
                &CisMessageEvent {
                    text: &text,
                    start_word: &m.start_word,
                    session_key: &m.key,
                    characters: m.chars,
                    failing: m.bad,
                },
            ));
            out.messages.push(m);
        }
        self.state = BeeState::After;
        self.after_invalid = 0;
        self.bits.clear();
    }

    fn char(&mut self, c: u8, out: &mut Out) {
        let Some(m) = self.msg.as_mut() else { return };
        if c == EOM {
            self.eom += 1;
            if self.eom >= 3 {
                self.end(out, "end of message");
            }
            return;
        }
        for _ in 0..self.eom {
            out.chars.push_str(" [77]");
            m.chars += 1;
            m.bad += 1;
            out.bad += 1;
        }
        self.eom = 0;
        m.chars += 1;
        if valid(c) {
            out.chars.push_str(&format!(" {c:02X}"));
            out.good += 1;
            self.invalid = 0;
        } else {
            out.chars.push_str(&format!(" [{c:02X}]"));
            m.bad += 1;
            out.bad += 1;
            self.invalid += 1;
        }
        if self.invalid >= 12 || m.chars >= 20_000 {
            self.end(out, "signal lost");
        }
    }

    fn push(&mut self, b: u8, out: &mut Out) {
        self.bits.push(b);
        match self.state.clone() {
            BeeState::Search => {
                if self.bits.len() > START_BITS + 4 {
                    self.bits.remove(0);
                }
                let best = [false, true]
                    .into_iter()
                    .filter_map(|inv| self.start_score(inv).map(|s| (s, inv)))
                    .max_by_key(|s| s.0);
                if let Some((s, inv)) = best {
                    self.state = BeeState::Held {
                        best: (s, 0, inv),
                        wait: 3,
                    };
                }
            }
            BeeState::Held { mut best, wait } => {
                best.1 += 1;
                if self.bits.len() > START_BITS + 8 {
                    self.bits.remove(0);
                }
                for inv in [false, true] {
                    if let Some(s) = self.start_score(inv) {
                        if s > best.0 {
                            best = (s, 0, inv);
                        }
                    }
                }
                if wait > 1 {
                    self.state = BeeState::Held {
                        best,
                        wait: wait - 1,
                    };
                    return;
                }
                // The start ends `best.1` bits before the newest bit.
                let n = self.bits.len();
                let end = n - best.1;
                let start: Vec<u8> = self.bits[end - START_BITS..end]
                    .iter()
                    .map(|&b| b ^ u8::from(best.2))
                    .collect();
                let rest: Vec<u8> = self.bits[end..].to_vec();
                self.inverted = best.2;
                self.bits.clear();
                self.begin(out, false, &start);
                for b in rest {
                    self.push(b, out);
                }
            }
            BeeState::Message => {
                if self.bits.len() >= 7 {
                    let c = self
                        .bits
                        .drain(..7)
                        .fold(0u8, |v, b| (v << 1) | (b ^ u8::from(self.inverted)));
                    self.char(c, out);
                }
            }
            BeeState::After => {
                if self.bits.len() >= 7 {
                    let c = self
                        .bits
                        .drain(..7)
                        .fold(0u8, |v, b| (v << 1) | (b ^ u8::from(self.inverted)));
                    if c == EOM {
                        return;
                    }
                    if valid(c) {
                        self.begin(out, true, &[]);
                        self.char(c, out);
                    } else {
                        self.after_invalid += 1;
                        if self.after_invalid >= 6 {
                            self.state = BeeState::Search;
                            self.bits.clear();
                        }
                    }
                }
            }
        }
    }
}

/// The `bee` receiver.
pub struct Bee {
    front: FskFront,
    framers: Vec<BeeFramer>,
    bits: Vec<(usize, u8)>,
}

/// The speeds `bee` tries.
pub const BEE_BAUDS: [f64; 5] = [36.0, 50.0, 75.0, 100.0, 150.0];

impl Bee {
    /// The receiver at the speed `baud` (0 tries every speed of [`BEE_BAUDS`]) and the shift `shift_hz` (0 finds it).
    pub fn new(baud: f64, shift_hz: f64) -> Self {
        let bauds: Vec<f64> = if baud > 0.0 {
            vec![baud]
        } else {
            BEE_BAUDS.to_vec()
        };
        Bee {
            front: FskFront::new(&bauds, shift_hz),
            framers: bauds.iter().map(|&b| BeeFramer::new(b)).collect(),
            bits: Vec::new(),
        }
    }
}

impl Sub for Bee {
    fn push(&mut self, x: &[Complex32], out: &mut Out) {
        for &v in x {
            self.bits.clear();
            self.front.push(v, &mut self.bits);
            for &(i, b) in &self.bits {
                self.framers[i].push(b, out);
            }
        }
    }
    fn state(&self) -> u8 {
        if self
            .framers
            .iter()
            .any(|f| matches!(f.state, BeeState::Message))
        {
            2
        } else if self.front.pair.lock.is_some() {
            1
        } else {
            0
        }
    }
    fn summary(&self) -> String {
        match (
            self.front.pair.lock,
            self.framers.iter().find(|f| f.state == BeeState::Message),
        ) {
            (_, Some(f)) => format!("CIS-36-50 message at {:.0} Bd", f.baud),
            (Some((c, s)), None) => {
                format!("2-FSK at {c:.0} Hz, shift {s:.0} Hz, no message start")
            }
            (None, None) => "searching".to_string(),
        }
    }
    fn measure<'a>(&'a self, m: &mut super::Measures<'a>) {
        fsk_measure(&self.front, m);
    }
    fn snr_db(&self) -> Option<f64> {
        self.front.pair.snr_db
    }
}

/// The tone fields of a 2-FSK front.
fn fsk_measure(front: &FskFront, m: &mut super::Measures<'_>) {
    m.centre_hz = front.pair.lock.map(|l| (l.0 * 10.0).round() / 10.0);
    m.shift_hz = front.pair.lock.map(|l| (l.1 * 10.0).round() / 10.0);
    let eye = front.demods.iter().map(|d| d.eye).fold(0.0, f64::max);
    m.eye = front.pair.lock.map(|_| (eye * 100.0).round() / 100.0);
}

// ------------------------------------------------------------------------------------------------------------------------------
// cis14: CIS-14

/// The `cis14` receiver.
pub struct Cis14 {
    front: FskFront,
    new_bits: Vec<(usize, u8)>,
    bits: Vec<u8>,
    frames: Vec<[u8; 14]>,
    align: Option<usize>,
    count: i64,
    state_bits: String,
}

impl Cis14 {
    /// The receiver at `baud` (0 takes 96 Bd) and the shift `shift_hz` (0 finds it).
    pub fn new(baud: f64, shift_hz: f64) -> Self {
        let b = if baud > 0.0 { baud } else { 96.0 };
        Cis14 {
            front: FskFront::new(&[b], shift_hz),
            new_bits: Vec::new(),
            bits: Vec::new(),
            frames: Vec::new(),
            align: None,
            count: 0,
            state_bits: String::new(),
        }
    }

    /// The alignment of the kept frames: the pair of adjacent places (the state bits) whose smaller mean is largest, that value,
    /// and the median of the other places.
    fn alignment(&self) -> (usize, f64, f64) {
        let n = self.frames.len() as f64;
        let means: Vec<f64> = (0..14)
            .map(|p| {
                (self
                    .frames
                    .iter()
                    .map(|f| if f[p] == 1 { 1.0 } else { -1.0 })
                    .sum::<f64>()
                    / n)
                    .abs()
            })
            .collect();
        let (best, v) = (0..14)
            .map(|p| (p, means[p].min(means[(p + 1) % 14])))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        let mut rest: Vec<f64> = (0..14)
            .filter(|&p| p != best && p != (best + 1) % 14)
            .map(|p| means[p])
            .collect();
        rest.sort_by(f64::total_cmp);
        (best, v, rest[rest.len() / 2])
    }

    fn print(&mut self, f: &[u8], out: &mut Out) {
        let word = |places: [usize; 5]| places.iter().fold(0u8, |v, &p| (v << 1) | f[p]);
        let a = word([2, 4, 6, 8, 10]);
        let b = word([3, 5, 7, 9, 11]);
        let ss = format!("{}{}", f[0], f[1]);
        out.chars
            .push_str(&format!("{ss} {a:02X} {b:02X} {}{}\n", f[12], f[13]));
        out.good += 1;
        self.count += 1;
        self.state_bits = ss;
    }
}

impl Sub for Cis14 {
    fn push(&mut self, x: &[Complex32], out: &mut Out) {
        for &v in x {
            self.new_bits.clear();
            self.front.push(v, &mut self.new_bits);
            for k in 0..self.new_bits.len() {
                let b = self.new_bits[k].1;
                self.bits.push(b);
                if self.bits.len() < 14 {
                    continue;
                }
                let mut f = [0u8; 14];
                f.copy_from_slice(&self.bits[..14]);
                self.bits.drain(..14);
                self.frames.push(f);
                if self.frames.len() > 64 {
                    self.frames.remove(0);
                }
                if self.frames.len() < 24 {
                    continue;
                }
                let (p, v, med) = self.alignment();
                match self.align {
                    None if v >= 0.85 && med <= 0.5 => {
                        self.align = Some(p);
                        // Frames that start at place p: realign the kept frames and print them.
                        let flat: Vec<u8> = self.frames.iter().flatten().copied().collect();
                        let kept: Vec<[u8; 14]> = flat[p..].as_chunks::<14>().0.to_vec();
                        let tail = flat[p + 14 * kept.len()..].to_vec();
                        for f in &kept {
                            self.print(f, out);
                        }
                        self.frames = kept;
                        self.bits.splice(0..0, tail);
                    }
                    // Lost, or the frames drifted to another place: aligned again from the kept frames.
                    Some(q) if v < 0.6 || q != 0 => self.align = None,
                    Some(_) => self.print(&f, out),
                    None => {}
                }
            }
        }
    }
    fn state(&self) -> u8 {
        if self.align.is_some() {
            2
        } else if self.front.pair.lock.is_some() {
            1
        } else {
            0
        }
    }
    fn summary(&self) -> String {
        match (self.align, self.front.pair.lock) {
            (Some(_), _) => format!("CIS-14 frames, state bits {}", self.state_bits),
            (None, Some((c, s))) => {
                format!("2-FSK at {c:.0} Hz, shift {s:.0} Hz, no frame alignment")
            }
            _ => "searching".to_string(),
        }
    }
    fn measure<'a>(&'a self, m: &mut super::Measures<'a>) {
        fsk_measure(&self.front, m);
        m.frames = Some(self.count);
        m.state_bits = (!self.state_bits.is_empty()).then_some(self.state_bits.as_str());
    }
    fn snr_db(&self) -> Option<f64> {
        self.front.pair.snr_db
    }
}

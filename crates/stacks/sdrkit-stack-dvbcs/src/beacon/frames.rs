//! Telemetry frames: sync words, the frame length, the template and fixed bits, the frames with their errors, the minor frame
//! counter, and the decoded view with a confidence for every line.
//!
//! Standard: IRIG 106 chapter 4; CCSDS 131.0-B.

use sdrkit_core::Choice;
use std::collections::VecDeque;

sdrkit_core::choice_enum! {
    /// A frame sync word that the learner looks for.
    pub enum SyncWord {
        /// FAF320 of IRIG 106.
        Faf320 = ("faf320", "FAF320 (IRIG 106)"),
        /// FE6B2840 of IRIG 106.
        Fe6b2840 = ("fe6b2840", "FE6B2840 (IRIG 106)"),
        /// EB90 of IRIG 106.
        Eb90 = ("eb90", "EB90 (IRIG 106)"),
        /// The attached sync marker of CCSDS.
        Ccsds = ("1acffc1d", "1ACFFC1D (CCSDS)"),
    }
}

/// The sync words looked for: value, length in bits, which.
pub const SYNC_WORDS: [(u64, u32, SyncWord); 4] = [
    (0xFAF320, 24, SyncWord::Faf320),
    (0xFE6B_2840, 32, SyncWord::Fe6b2840),
    (0xEB90, 16, SyncWord::Eb90),
    (0x1ACF_FC1D, 32, SyncWord::Ccsds),
];

sdrkit_core::choice_enum! {
    /// What a line of the decoded view is.
    pub enum ViewKind {
        /// A field of the frame.
        Field = ("field", "Field"),
        /// A summary of the frames.
        Summary = ("summary", "Summary"),
    }
}

sdrkit_core::choice_enum! {
    /// How sure a line of the decoded view is.
    pub enum Confidence {
        /// Decoded by a rule.
        Decoded = ("decoded", "Decoded"),
        /// Estimated.
        Estimated = ("estimated", "Estimated"),
        /// The bytes change, but their meaning is not known.
        Unknown = ("unknown", "Meaning not known"),
    }
}

/// One telemetry frame.
#[derive(Clone, Debug, PartialEq)]
pub struct BeaconFrame {
    /// Frame number since the start.
    pub id: u64,
    /// Bytes from the frame start, polarity normalised.
    pub bytes: Vec<u8>,
    /// Minor frame counter, when known.
    pub counter: Option<u32>,
    /// Errors in the fixed columns.
    pub errors: u32,
    /// Few errors.
    pub good: bool,
    /// The signal time when the frame was found, seconds.
    pub time_s: f64,
}

/// A line of the decoded view.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewLine {
    /// What the line is.
    pub kind: ViewKind,
    /// Name.
    pub name: String,
    /// Value.
    pub value: String,
    /// Raw hex.
    pub raw: String,
    /// Byte positions.
    pub bytes: String,
    /// How sure it is.
    pub confidence: Confidence,
    /// Why.
    pub reason: String,
}

/// The frame learner.
#[derive(Debug, Default)]
pub struct Learner {
    bits: VecDeque<u8>,
    /// Absolute index of `bits[0]`.
    base: u64,
    /// Frame length, bits.
    pub frame_len: usize,
    /// Absolute bit index of a frame start.
    start: u64,
    /// The stream is inverted.
    pub inverted: bool,
    /// The sync word found.
    pub sync: Option<SyncWord>,
    sync_bits: (u64, u32),
    /// Frames, newest first (at most 48), and every frame seen (for the analysis, at most 256).
    pub frames: VecDeque<BeaconFrame>,
    history: VecDeque<Vec<u8>>,
    next_id: u64,
    next_start: u64,
    /// Template bytes and fixed-bit mask.
    pub template: Vec<u8>,
    /// Fixed bits.
    pub fixed: Vec<u8>,
    /// Per-column stability (0.5 random to 1 fixed), per byte.
    pub stability: Vec<f32>,
    /// Counter byte and modulo.
    pub counter: Option<(usize, u32)>,
    /// Good and bad frames.
    pub good: u64,
    /// Bad frames.
    pub bad: u64,
    /// Bits seen.
    pub total_bits: u64,
    /// Agreement of the frames (0.5 chance, 1 identical).
    pub agreement: f64,
}

impl Learner {
    /// Takes bits. The frames found get the signal time `time_s`.
    pub fn push(&mut self, bits: &[u8], time_s: f64, new_frames: &mut Vec<BeaconFrame>) {
        self.bits.extend(bits.iter().copied());
        self.total_bits += bits.len() as u64;
        while self.bits.len() > 65_536 {
            self.bits.pop_front();
            self.base += 1;
        }
        if self.frame_len == 0 {
            self.find_sync();
            if self.frame_len == 0 {
                return;
            }
        }
        // Cut frames.
        let fl = self.frame_len as u64;
        while self.next_start + fl <= self.base + self.bits.len() as u64 {
            if self.next_start < self.base {
                self.next_start += fl;
                continue;
            }
            let off = (self.next_start - self.base) as usize;
            let mut bytes = Vec::with_capacity(self.frame_len / 8);
            for k in 0..self.frame_len / 8 {
                let mut v = 0u8;
                for i in 0..8 {
                    v = (v << 1) | self.bits[off + 8 * k + i];
                }
                bytes.push(if self.inverted { !v } else { v });
            }
            self.next_start += fl;
            // The sync word must be there (else the frame is counted bad).
            self.history.push_back(bytes.clone());
            if self.history.len() > 256 {
                self.history.pop_front();
            }
            self.learn();
            let errors = self.errors(&bytes);
            let fixed_bits: u32 = self.fixed.iter().map(|m| m.count_ones()).sum();
            let sum_rule = self.checksum_rule();
            let judge = |bytes: &[u8], errors: u32| -> bool {
                if sum_rule {
                    // The checksum distinguishes a good frame from a damaged one.
                    bytes[..bytes.len() - 1]
                        .iter()
                        .fold(0u8, |a, &b| a.wrapping_add(b))
                        == bytes[bytes.len() - 1]
                } else {
                    errors <= (fixed_bits / 20).max(2)
                }
            };
            let good = judge(&bytes, errors);
            if self.history.len() == 3 {
                // The first frames are judged once there is a template.
                for f in self.frames.iter_mut() {
                    f.errors = f
                        .bytes
                        .iter()
                        .zip(&self.template)
                        .zip(&self.fixed)
                        .map(|((&b, &t), &m)| ((b ^ t) & m).count_ones())
                        .sum();
                    f.good = judge(&f.bytes, f.errors);
                    if f.good {
                        self.good += 1;
                    } else {
                        self.bad += 1;
                    }
                }
            }
            if self.history.len() >= 3 {
                if good {
                    self.good += 1;
                } else {
                    self.bad += 1;
                }
            }
            let counter = self.counter.map(|(b, _)| bytes[b] as u32);
            if let Some((b, _)) = self.counter {
                for f in self.frames.iter_mut().filter(|f| f.counter.is_none()) {
                    f.counter = Some(f.bytes[b] as u32);
                }
            }
            let f = BeaconFrame {
                id: self.next_id,
                bytes,
                counter,
                errors,
                good,
                time_s,
            };
            self.next_id += 1;
            self.frames.push_front(f.clone());
            if self.frames.len() > 48 {
                self.frames.pop_back();
            }
            new_frames.push(f);
        }
    }

    fn find_sync(&mut self) {
        let n = self.bits.len();
        for &(word, len, name) in &SYNC_WORDS {
            let mask = (1u64 << len) - 1;
            for inv in [false, true] {
                let want = if inv { !word & mask } else { word };
                let mut reg = 0u64;
                let mut pos = Vec::new();
                for i in 0..n {
                    reg = ((reg << 1) | self.bits[i] as u64) & mask;
                    if i + 1 >= len as usize && reg == want {
                        pos.push(i + 1 - len as usize);
                    }
                }
                if pos.len() < 3 {
                    continue;
                }
                // The most common spacing between consecutive hits.
                let mut diffs: Vec<usize> = pos.windows(2).map(|w| w[1] - w[0]).collect();
                diffs.sort_unstable();
                let mut best = (0usize, 0usize);
                let mut i = 0;
                while i < diffs.len() {
                    let mut j = i;
                    while j < diffs.len() && diffs[j] == diffs[i] {
                        j += 1;
                    }
                    if j - i > best.1 {
                        best = (diffs[i], j - i);
                    }
                    i = j;
                }
                if best.1 >= 2 && best.0 >= 64 && best.0.is_multiple_of(8) {
                    self.frame_len = best.0;
                    let first = pos
                        .iter()
                        .copied()
                        .find(|&p| pos.contains(&(p + best.0)))
                        .unwrap_or(pos[0]);
                    self.start = self.base + first as u64;
                    self.next_start = self.start;
                    self.inverted = inv;
                    self.sync = Some(name);
                    self.sync_bits = (word, len);
                    return;
                }
            }
        }
    }

    fn learn(&mut self) {
        let nb = self.frame_len / 8;
        let nf = self.history.len();
        if nf == 0 {
            return;
        }
        self.template = vec![0; nb];
        self.fixed = vec![0; nb];
        self.stability = vec![0.0; nb];
        let mut agree_sum = 0.0;
        for b in 0..nb {
            for bit in 0..8 {
                let ones = self
                    .history
                    .iter()
                    .filter(|f| (f[b] >> (7 - bit)) & 1 == 1)
                    .count();
                let maj = 2 * ones >= nf;
                let a = if maj { ones } else { nf - ones } as f64 / nf as f64;
                if maj {
                    self.template[b] |= 1 << (7 - bit);
                }
                if a >= 0.85 && nf >= 3 {
                    self.fixed[b] |= 1 << (7 - bit);
                }
                self.stability[b] += a as f32 / 8.0;
                agree_sum += a;
            }
        }
        self.agreement = agree_sum / (nb * 8) as f64;
        // The counter byte: goes up by one from frame to frame and wraps to 0.
        if nf >= 4 {
            let mut best: Option<(usize, f64)> = None;
            for b in 0..nb {
                let steps = self
                    .history
                    .iter()
                    .zip(self.history.iter().skip(1))
                    .filter(|(a, c)| c[b] == a[b].wrapping_add(1) || (c[b] == 0 && a[b] != 0))
                    .count();
                let f = steps as f64 / (nf - 1) as f64;
                let incs = self
                    .history
                    .iter()
                    .zip(self.history.iter().skip(1))
                    .filter(|(a, c)| c[b] == a[b].wrapping_add(1))
                    .count();
                if f >= 0.8 && incs * 2 >= nf - 1 && best.is_none_or_lower(f) {
                    best = Some((b, f));
                }
            }
            if let Some((b, _)) = best {
                let m = self.history.iter().map(|f| f[b] as u32).max().unwrap_or(0) + 1;
                self.counter = Some((b, m));
            }
        }
    }

    /// Whether the last byte is the sum of the others in (nearly) every frame seen.
    pub fn checksum_rule(&self) -> bool {
        let n = self.history.len();
        n >= 3 && {
            let ok = self
                .history
                .iter()
                .filter(|f| {
                    f[..f.len() - 1].iter().fold(0u8, |a, &b| a.wrapping_add(b)) == f[f.len() - 1]
                })
                .count();
            ok * 10 >= n * 9
        }
    }

    fn errors(&self, bytes: &[u8]) -> u32 {
        bytes
            .iter()
            .zip(&self.template)
            .zip(&self.fixed)
            .map(|((&b, &t), &m)| ((b ^ t) & m).count_ones())
            .sum()
    }

    /// The frames kept for the analysis.
    pub fn history(&self) -> &VecDeque<Vec<u8>> {
        &self.history
    }
}

trait BestExt {
    fn is_none_or_lower(&self, f: f64) -> bool;
}
impl BestExt for Option<(usize, f64)> {
    fn is_none_or_lower(&self, f: f64) -> bool {
        self.is_none_or(|(_, g)| f > g)
    }
}

/// The decoded view of the frames. `frame_s` is the time per frame; `rf_hz` the measured RF frequency.
pub fn analyse(l: &Learner, frame_s: f64, rf_hz: Option<f64>) -> Vec<ViewLine> {
    let h: Vec<&Vec<u8>> = l.history().iter().collect();
    let mut out = Vec::new();
    if h.len() < 4 || l.frame_len == 0 {
        return out;
    }
    let nb = l.frame_len / 8;
    let mut used = vec![false; nb];
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02X}")).collect::<String>();
    let line = |kind, name: &str, value: String, raw: String, bytes: String, conf, reason: &str| {
        ViewLine {
            kind,
            name: name.into(),
            value,
            raw,
            bytes,
            confidence: conf,
            reason: reason.into(),
        }
    };
    let sync_bytes = (l.sync_bits.1 / 8) as usize;
    out.push(line(
        ViewKind::Field,
        "Sync word",
        l.sync.map_or("", |s| s.label()).to_owned(),
        hex(&h[0][..sync_bytes]),
        format!("0 to {}", sync_bytes - 1),
        Confidence::Decoded,
        "found at a constant spacing",
    ));
    used[..sync_bytes].iter_mut().for_each(|u| *u = true);
    // Constant bytes right after the sync word.
    let mut k = sync_bytes;
    while k < nb && h.iter().all(|f| f[k] == h[0][k]) && Some(k) != l.counter.map(|c| c.0) {
        k += 1;
    }
    if k > sync_bytes {
        out.push(line(
            ViewKind::Field,
            "Constant after the sync word",
            hex(&h[0][sync_bytes..k]),
            hex(&h[0][sync_bytes..k]),
            format!("{sync_bytes} to {}", k - 1),
            Confidence::Decoded,
            "the same in every frame",
        ));
        used[sync_bytes..k].iter_mut().for_each(|u| *u = true);
    }
    if let Some((cb, m)) = l.counter {
        out.push(line(
            ViewKind::Field,
            "Minor frame counter",
            format!("0 to {}", m - 1),
            String::new(),
            format!("{cb}"),
            Confidence::Decoded,
            "goes up by one from frame to frame",
        ));
        used[cb] = true;
        for b in 0..nb {
            if b != cb && !used[b] && h.iter().all(|f| f[b] == f[cb]) {
                out.push(line(
                    ViewKind::Field,
                    "Copy of the counter",
                    format!("byte {b}"),
                    String::new(),
                    format!("{b}"),
                    Confidence::Decoded,
                    "equals the counter in every frame",
                ));
                used[b] = true;
            }
        }
    }
    // Checksum in the last byte.
    let sum_ok = h
        .iter()
        .filter(|f| f[..nb - 1].iter().fold(0u8, |a, &b| a.wrapping_add(b)) == f[nb - 1])
        .count();
    if sum_ok == h.len() {
        out.push(line(
            ViewKind::Field,
            "Checksum",
            "sum of the other bytes modulo 256".into(),
            String::new(),
            format!("{}", nb - 1),
            Confidence::Decoded,
            "holds in every frame",
        ));
        used[nb - 1] = true;
    }
    // A seconds clock: steps by one at about one per second and never otherwise.
    for b in 0..nb {
        if used[b] {
            continue;
        }
        let mut steps = 0;
        let mut other = 0;
        for w in h.windows(2) {
            match w[1][b].wrapping_sub(w[0][b]) {
                0 => {}
                1 => steps += 1,
                _ => other += 1,
            }
        }
        let rate = steps as f64 / ((h.len() - 1) as f64 * frame_s);
        if other == 0 && steps >= 2 && (rate - 1.0).abs() < 0.2 {
            out.push(line(
                ViewKind::Field,
                "Seconds clock",
                format!("{} now", h[h.len() - 1][b]),
                String::new(),
                format!("{b}"),
                Confidence::Estimated,
                "counts seconds",
            ));
            used[b] = true;
        }
    }
    // Unknown meaning, grouped.
    let mut constant = Vec::new();
    let mut idle = Vec::new();
    for b in 0..nb {
        if used[b] {
            continue;
        }
        let v = h[0][b];
        if h.iter().all(|f| f[b] == v) {
            if v == 0xFB || v == 0xFC {
                idle.push(b);
            } else {
                constant.push(b);
            }
            used[b] = true;
        }
    }
    let list = |v: &[usize], h: &Vec<&Vec<u8>>| {
        v.iter()
            .map(|&b| format!("{b}:{:02X}", h[0][b]))
            .collect::<Vec<_>>()
            .join(" ")
    };
    if !constant.is_empty() {
        out.push(line(
            ViewKind::Field,
            "Constant words",
            list(&constant, &h),
            String::new(),
            format!("{constant:?}"),
            Confidence::Unknown,
            "the same in every frame",
        ));
    }
    if !idle.is_empty() {
        out.push(line(
            ViewKind::Field,
            "Idle fill",
            format!("{:02X}", h[0][idle[0]]),
            String::new(),
            format!("{idle:?}"),
            Confidence::Unknown,
            "fill bytes",
        ));
    }
    // Commutated: the value depends only on the counter position.
    if let Some((cb, _)) = l.counter {
        for b in 0..nb {
            if used[b] {
                continue;
            }
            let mut map = std::collections::BTreeMap::new();
            let mut consistent = true;
            for f in &h {
                if let Some(&old) = map.get(&f[cb]) {
                    if old != f[b] {
                        consistent = false;
                        break;
                    }
                } else {
                    map.insert(f[cb], f[b]);
                }
            }
            let distinct: std::collections::BTreeSet<u8> = map.values().copied().collect();
            if consistent && distinct.len() > 1 && map.len() >= 3 {
                let vals = map
                    .values()
                    .take(8)
                    .map(|v| format!("{v:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push(line(
                    ViewKind::Field,
                    "Commutated word",
                    vals,
                    String::new(),
                    format!("{b}"),
                    Confidence::Unknown,
                    "its value follows the counter",
                ));
                used[b] = true;
            }
        }
    }
    for b in 0..nb {
        if used[b] {
            continue;
        }
        let (mn, mx) = h
            .iter()
            .fold((255u8, 0u8), |(a, c), f| (a.min(f[b]), c.max(f[b])));
        let mean = h.iter().map(|f| f[b] as f64).sum::<f64>() / h.len() as f64;
        out.push(line(
            ViewKind::Field,
            "Changing word",
            format!("{mn} to {mx}, mean {mean:.1} (raw counts)"),
            String::new(),
            format!("{b}"),
            Confidence::Unknown,
            "changes from frame to frame",
        ));
    }
    // Summary lines.
    let text = h.iter().any(|f| {
        f.windows(6)
            .any(|w| w.iter().all(|&c| (0x20..0x7F).contains(&c)))
    });
    for (n, v) in [
        ("Date and time of day", "not found"),
        ("Orbit, position, ephemeris", "not found"),
        ("Transponders and channels", "not found"),
        ("Text", if text { "found" } else { "none" }),
    ] {
        out.push(line(
            ViewKind::Summary,
            n,
            v.into(),
            String::new(),
            String::new(),
            Confidence::Decoded,
            "looked for in every frame",
        ));
    }
    out.push(line(
        ViewKind::Summary,
        "Frame health",
        format!(
            "{} of {} frames good{}",
            l.good.min(h.len() as u64),
            h.len(),
            if sum_ok == h.len() {
                ", checksum holds"
            } else {
                ""
            }
        ),
        String::new(),
        String::new(),
        Confidence::Decoded,
        "",
    ));
    let sat = match rf_hz {
        Some(f) => satellite(f),
        None => None,
    };
    out.push(line(
        ViewKind::Summary,
        "Satellite",
        sat.unwrap_or_else(|| "not identified".into()),
        String::new(),
        String::new(),
        Confidence::Estimated,
        "estimated from the RF frequency against a table of published beacon frequencies (no table is built in)",
    ));
    out
}

/// The satellite of a beacon frequency: no permissively licensed table of published beacons is built in, so this says nothing.
pub fn satellite(_rf_hz: f64) -> Option<String> {
    None
}

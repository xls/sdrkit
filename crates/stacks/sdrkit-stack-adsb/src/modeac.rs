//! Mode A/C replies on 1090 MHz: the framing pulses, the 12 code pulses and the SPI pulse. Each code shows as a squawk and as a
//! Mode C altitude, because the receiver does not hear which interrogation it answers. [`ModeAcDecoder`] keeps the codes heard.
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.1

use crate::codes::{gillham_ft, squawk, squawk_text};
use crate::frontend::ChainDelay;
use crate::settings::{EXPIRY_S, MIN_REPLIES, MODEAC};
use crate::typed::{ModeAcFields, ModeAcReplyEvent, ModeAcRow, MODEAC_CODES, MODEAC_REPLY};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Time};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    Complex32, ConfigError, EventDef, LockState, Record, Results, SettingError, SettingId,
    SettingSet, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_util_geo::Text;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
use std::collections::VecDeque;
use std::fmt::Write;

/// One Mode A/C reply.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ModeAcReply {
    /// Signal time of F1, s.
    pub time: f64,
    /// The 13 code bits C1 A1 C2 A2 C4 A4 X B1 D1 B2 D2 B4 D4 (X is 0).
    pub code: u16,
    /// The code read as a squawk.
    pub squawk: u16,
    /// The code read as a Mode C altitude, when it is one.
    pub altitude_ft: Option<i32>,
    /// The SPI pulse was present.
    pub spi: bool,
    /// Level, dBFS.
    pub level_dbfs: f32,
}

impl Item for ModeAcReply {
    const KIND: &'static str = "modeac.reply";
    fn event_def() -> Option<&'static EventDef> {
        Some(MODEAC_REPLY.def())
    }
    /// The code in hex, the squawk, the altitude when the code is one, the SPI pulse and the level.
    fn write_fields(&self, r: &mut Record) {
        let mut code = Text::<4>::new();
        // Four hex digits fit.
        let _ = write!(code, "{:04X}", self.code);
        ModeAcReplyEvent {
            code: &code,
            squawk: squawk_text(self.squawk).as_str(),
            altitude_ft: self.altitude_ft.map(i64::from),
            spi: self.spi,
            level_dbfs: f64::from(self.level_dbfs),
            family: keys::Family::Aircraft,
        }
        .write(r);
    }
}

/// The Mode C altitude of a 13-bit reply code (D1 must be 0).
pub fn mode_c_altitude(code: u16) -> Option<i32> {
    let c = code as u32;
    if c & (1 << 4) != 0 {
        return None; // D1
    }
    // C1 A1 C2 A2 C4 A4 (X) B1 (D1) B2 D2 B4 D4 -> C1 A1 C2 A2 C4 A4 B1 B2 D2 B4 D4
    let g = ((c >> 7) << 5) | (((c >> 5) & 1) << 4) | (c & 0xF);
    gillham_ft(g)
}

/// A 13-bit reply code as a table key. Its text is the code read as a squawk, four octal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ReplyCode(pub u16);

impl std::fmt::Display for ReplyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&squawk_text(squawk(self.0 as u32)))
    }
}

/// What the table keeps of a code beside when it was heard.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CodeEntry {
    code: u16,
    spi: bool,
    level: f32,
}

impl Shaped for CodeEntry {
    type Row = ModeAcRow<'static>;
    /// The row of [`MODEAC_CODES`].
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut code = Text::<4>::new();
        // Four hex digits fit.
        let _ = write!(code, "{:04X}", self.code);
        out.put(&ModeAcRow {
            code: &code,
            squawk: squawk_text(squawk(u32::from(self.code))).as_str(),
            alt_ft: mode_c_altitude(self.code).map(i64::from),
            replies: columns::heard(seen),
            spi: self.spi,
            level_dbfs: f64::from(self.level),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// The Mode A/C detector and table.
pub struct ModeAcDecoder {
    enabled: bool,
    standalone: bool,
    fs: f64,
    us: f64,
    epoch: f64,
    delay: f64,
    base: u64,
    p: Vec<f32>,
    cum: Vec<f64>,
    z: Vec<Complex32>,
    total: u64,
    next: u64,
    skip_until: u64,
    blocks: VecDeque<f32>,
    /// Scratch for the noise quantile (no allocation while decoding).
    sorted: Vec<f32>,
    blk_sum: f64,
    blk_n: usize,
    noise: Option<f32>,
    codes: ObjectTable<ReplyCode, CodeEntry>,
    replies: u64,
    expiry_s: f64,
    min_replies: u64,
    published: Published,
    out: Vec<ModeAcReply>,
    last_reply: f64,
    summary: String,
}

impl Default for ModeAcDecoder {
    fn default() -> Self {
        ModeAcDecoder {
            enabled: false,
            standalone: false,
            fs: 4e6,
            us: 4.0,
            epoch: 0.0,
            delay: 0.0,
            base: 0,
            p: Vec::new(),
            cum: vec![0.0],
            z: Vec::new(),
            total: 0,
            next: 0,
            skip_until: 0,
            blocks: VecDeque::new(),
            sorted: Vec::with_capacity(64),
            blk_sum: 0.0,
            blk_n: 0,
            noise: None,
            codes: ObjectTable::new(4096),
            replies: 0,
            expiry_s: 60.0,
            min_replies: 2,
            published: Published::new(ModeAcFields::SHAPE.fields).with_tables(&[&MODEAC_CODES]),
            summary: String::new(),
            out: Vec::new(),
            last_reply: f64::MIN,
        }
    }
}

impl ModeAcDecoder {
    /// A decoder that is on (the Mode A/C mode's).
    pub fn standalone() -> Self {
        ModeAcDecoder {
            enabled: true,
            standalone: true,
            ..Default::default()
        }
    }

    fn reset_stream(&mut self, fs: f64, delay: f64) {
        if self.fs > 0.0 {
            self.epoch += self.total as f64 / self.fs;
        }
        self.fs = fs;
        self.us = fs * 1e-6;
        self.delay = delay;
        self.base = 0;
        self.p.clear();
        self.cum.clear();
        self.cum.push(0.0);
        self.z.clear();
        self.total = 0;
        self.next = 0;
        self.skip_until = 0;
        self.blocks.clear();
        self.blk_sum = 0.0;
        self.blk_n = 0;
        self.noise = None;
    }

    /// Mean power over [a, a + len) in samples (absolute positions, sample-based).
    fn mean(&self, a: f64, len: f64) -> f32 {
        let i0 = (a.round() as u64).saturating_sub(self.base) as usize;
        let n = len.round().max(1.0) as usize;
        let i1 = (i0 + n).min(self.cum.len() - 1);
        if i1 <= i0 {
            return 0.0;
        }
        ((self.cum[i1] - self.cum[i0]) / (i1 - i0) as f64) as f32
    }

    /// Feeds samples, appending decoded replies.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Vec<ModeAcReply>) {
        let mut c = *self
            .cum
            .last()
            .expect("the running sum always holds its starting entry");
        for &s in iq {
            let pw = s.norm_sqr();
            c += pw as f64;
            self.cum.push(c);
            self.p.push(pw);
            self.z.push(s);
            self.blk_sum += pw as f64;
            self.blk_n += 1;
            if self.blk_n == 256 {
                let m = (self.blk_sum / 256.0) as f32;
                if self.blocks.len() == 64 {
                    self.blocks.pop_front();
                }
                self.blocks.push_back(m);
                if self.blocks.len() >= 8 {
                    self.sorted.clear();
                    self.sorted.extend(self.blocks.iter().copied());
                    self.sorted.sort_by(f32::total_cmp);
                    self.noise = Some((self.sorted[self.sorted.len() / 4] / 0.93).max(1e-9));
                }
                self.blk_sum = 0.0;
                self.blk_n = 0;
            }
        }
        self.total += iq.len() as u64;
        self.search(out);
        // Trim.
        let keep = self.next.saturating_sub((4.0 * self.us) as u64);
        if keep > self.base + 65536 {
            let d = (keep - self.base) as usize;
            self.p.drain(..d);
            self.z.drain(..d);
            self.cum.drain(..d);
            self.base = keep;
        }
    }

    fn search(&mut self, out: &mut Vec<ModeAcReply>) {
        let us = self.us;
        let span = (27.0 * us) as u64 + 4;
        let w = 0.45 * us;
        while self.next + span < self.total {
            let n = self.next;
            self.next += 1;
            let Some(noise) = self.noise else { continue };
            if n < self.skip_until || (n as f64) < self.base as f64 + 2.0 * us {
                continue;
            }
            let x = n as f64;
            let f1 = self.mean(x, w);
            let th = 10.0 * noise;
            if f1 <= th {
                continue;
            }
            let f2 = self.mean(x + 20.3 * us, w);
            if f2 <= th || f1 > 2.5 * f2 || f2 > 2.5 * f1 {
                continue;
            }
            // A leading edge: quiet just before F1, and F1 rising here (not inside a pulse already).
            if self.mean(x - 1.0 * us, 0.5 * us) > 0.25 * f1 || self.mean(x - 1.0, 1.0) > 0.6 * f1 {
                continue;
            }
            // Refine to the best start within 0.25 µs.
            let mut best = (f1 + f2, x);
            for k in 1..=((0.25 * us) as usize).max(1) {
                let xx = x + k as f64;
                let v = self.mean(xx, w) + self.mean(xx + 20.3 * us, w);
                if v > best.0 {
                    best = (v, xx);
                }
            }
            let x = best.1;
            let lvl = 0.5 * (self.mean(x, w) + self.mean(x + 20.3 * us, w));
            // The X position and the half positions between code pulses must be quiet (a Mode S reply has pulses everywhere).
            if self.mean(x + 7.0 * 1.45 * us, w) > 0.3 * lvl {
                continue;
            }
            let mut quiet_ok = true;
            for k in 0..14 {
                let t = x + (k as f64 * 1.45 + 0.725) * us;
                if self.mean(t, 0.3 * us) > 0.3 * lvl {
                    quiet_ok = false;
                    break;
                }
            }
            if !quiet_ok {
                continue;
            }
            let mut code = 0u16;
            for i in 0..13 {
                let v = self.mean(x + (i as f64 + 1.0) * 1.45 * us, w);
                if v > 0.5 * lvl {
                    code |= 1 << (12 - i);
                }
            }
            code &= !(1 << 6);
            let spi = self.mean(x + 24.65 * us, w) > 0.5 * lvl;
            let time = self.epoch + x / self.fs - self.delay;
            let r = ModeAcReply {
                time,
                code,
                squawk: squawk(code as u32),
                altitude_ft: mode_c_altitude(code),
                spi,
                level_dbfs: 10.0 * lvl.max(1e-12).log10(),
            };
            self.skip_until = (x + 21.0 * us) as u64;
            out.push(r);
        }
    }

    fn take(&mut self, r: &ModeAcReply) {
        self.replies += 1;
        self.last_reply = r.time;
        if let Ok(up) = self
            .codes
            .upsert(&ReplyCode(r.code), Stamp::at(r.time), || CodeEntry {
                code: r.code,
                ..Default::default()
            })
        {
            up.row.spi |= r.spi;
            up.row.level = r.level_dbfs;
        }
    }

    fn publish(&mut self, now: f64) {
        self.codes.expire(now, self.expiry_s);
        let min = self.min_replies;
        let n = self.codes.iter().filter(|r| r.seen.count >= min).count();
        let p = &mut self.published;
        sdrkit_util_objects::shaped::rows_where(&self.codes, &MODEAC_CODES, now, p, |r| {
            r.seen.count >= min
        });
        p.put(&ModeAcFields {
            modeac_replies: self.replies as i64,
            modeac_codes: n as i64,
        });
        if self.standalone {
            let lock_state = if now - self.last_reply <= 5.0 {
                LockState::Locked
            } else {
                LockState::Searching
            };
            let level = self.noise.map(|x| 10.0 * x.log10());
            self.summary.clear();
            let _ = write!(self.summary, "{} replies, {} codes", self.replies, n);
            let summary = &self.summary;
            p.results_mut().update_status(|s: &mut Status| {
                let changed =
                    s.lock_state != lock_state || s.level_dbfs != level || s.summary != *summary;
                if changed {
                    s.lock_state = lock_state;
                    s.level_dbfs = level;
                    s.summary.clone_from(summary);
                }
                changed
            });
        }
    }
}

sdrkit_core::settings! {
    /// The settings of [`ModeAcDecoder`].
    pub enum ModeAcDecoderSetting {
        /// Decode Mode A/C replies.
        Enabled = MODEAC,
        /// Expiry time.
        ExpiryS = EXPIRY_S,
        /// Replies before a code is shown.
        MinReplies = MIN_REPLIES,
    }
}

sdrkit_core::commands! {
    /// The commands of [`ModeAcDecoder`].
    pub enum ModeAcCommand {
        /// Forgets the codes heard.
        Clear = sdrkit_util_objects::commands::CLEAR,
    }
}

impl Component for ModeAcDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.modeac",
            vec![
                Port::iq("in"),
                Port::typed::<ChainDelay>("delay").optional("no delay"),
            ],
            vec![Port::typed::<ModeAcReply>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ModeAcDecoderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 3.9e6 {
            return Err(ConfigError::RateTooLow {
                need: 4e6,
                got: f.rate,
            });
        }
        self.reset_stream(f.rate, self.delay);
        Ok(vec![Format::new(Kind::typed::<ModeAcReply>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(d) = io.items::<ChainDelay>(1).last() {
            self.delay = d.value.delay_s;
        }
        if !self.enabled {
            return;
        }
        let iq = io.iq(0);
        let mut out = std::mem::take(&mut self.out);
        out.clear();
        self.feed(iq, &mut out);
        for r in &out {
            self.take(r);
        }
        let o = io.items_out::<ModeAcReply>(0);
        for r in &out {
            o.push(Timed {
                time: r.time,
                value: r.clone(),
            });
        }
        self.out = out;
        if io.tick() {
            let now = self.epoch + self.total as f64 / self.fs - self.delay;
            self.publish(now);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ModeAcDecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ModeAcDecoderSetting::Enabled => self.enabled = MODEAC.read(id, v)?,
            ModeAcDecoderSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
            ModeAcDecoderSetting::MinReplies => {
                self.min_replies = MIN_REPLIES.read(id, v)?.max(1) as u64
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ModeAcDecoderSetting::Enabled, Value::Bool(self.enabled)),
            (ModeAcDecoderSetting::ExpiryS, Value::F64(self.expiry_s)),
            (
                ModeAcDecoderSetting::MinReplies,
                Value::I64(self.min_replies as i64),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        ModeAcCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match ModeAcCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ModeAcCommand::Clear => {
                self.codes.clear();
                self.replies = 0;
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// The Mode A/C test signal: transponders answering Mode A and Mode C interrogations (two Mode A replies for each Mode C, as
/// interleaved interrogation patterns do), with their own levels, carrier offsets and timing.
pub struct ModeAcGenerator {
    r: crate::generator::Renderer,
    rng: crate::generator::Rng,
    /// The transponders: (squawk, altitude ft, amplitude, carrier offset).
    pub targets: Vec<(u16, i32, f32, f64)>,
    next: Vec<f64>,
    turn: Vec<u8>,
    /// Every reply sent, when `keep_log` is set: (time, 13-bit code, SPI).
    pub log: Vec<(f64, u16, bool)>,
    /// Keep the log.
    pub keep_log: bool,
}

/// The reply code of an altitude (Mode C), D1 = 0 and X = 0.
pub fn mode_c_code(alt_ft: i32) -> Option<u16> {
    let g = crate::codes::encode_gillham(alt_ft)?;
    Some((((g >> 5) << 7) | (((g >> 4) & 1) << 5) | (g & 0xF)) as u16)
}

impl ModeAcGenerator {
    /// A generator at `rate` (2 Msps or more) with `n` transponders.
    pub fn new(rate: f64, n: usize, seed: u64) -> ModeAcGenerator {
        let mut rng = crate::generator::Rng::new(seed);
        let targets: Vec<(u16, i32, f32, f64)> = (0..n)
            .map(|_| {
                let sq = (rng.below(8) << 9 | rng.below(8) << 6 | rng.below(8) << 3 | rng.below(8))
                    as u16;
                let alt = (rng.range(1000.0, 39000.0) / 100.0).round() as i32 * 100;
                (sq, alt, rng.range(0.02, 0.3) as f32, rng.normal() * 50e3)
            })
            .collect();
        let next = (0..n).map(|_| rng.range(0.0, 0.004)).collect();
        let filt = (0.875 * rate).clamp(1.75e6, 5e6);
        ModeAcGenerator {
            r: crate::generator::Renderer::new(rate, filt),
            rng,
            targets,
            next,
            turn: vec![0; n],
            log: Vec::new(),
            keep_log: false,
        }
    }
    /// From a stack configuration.
    pub fn from_config(cfg: &sdrkit_core::GenConfig) -> ModeAcGenerator {
        let n = cfg.settings.get(&crate::settings::GEN_TARGETS).clamp(1, 50) as usize;
        ModeAcGenerator::new(cfg.rate_hz, n, cfg.seed)
    }
}

impl sdrkit_core::Generator for ModeAcGenerator {
    fn rate_hz(&self) -> f64 {
        self.r.rate()
    }
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(crate::generator::signal_info())
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_end = self.r.time_of(self.r.now_index() + out.len() as u64) + 30e-6;
        loop {
            let (i, t) = self
                .next
                .iter()
                .enumerate()
                .fold((0, f64::MAX), |b, (i, &t)| if t < b.1 { (i, t) } else { b });
            if t > t_end || self.targets.is_empty() {
                break;
            }
            let (sq, alt, amp, freq) = self.targets[i];
            self.turn[i] = (self.turn[i] + 1) % 3;
            let code = if self.turn[i] == 0 {
                mode_c_code(alt).unwrap_or(0)
            } else {
                crate::codes::encode_squawk(sq) as u16
            };
            let spi = false;
            if self.keep_log {
                self.log.push((t, code, spi));
            }
            let ph = self.rng.range(0.0, std::f64::consts::TAU);
            self.r.add(crate::generator::Burst::mode_ac(
                t,
                code as u32,
                spi,
                amp,
                freq,
                ph,
            ));
            self.next[i] = t + 0.004 + self.rng.range(0.0, 0.002);
        }
        self.r.render(out);
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codes::{encode_gillham, encode_squawk};

    #[test]
    fn mode_c_codes() {
        // An altitude code: Gillham bits placed into the reply layout with X = 0 and D1 = 0.
        let g = encode_gillham(12300).unwrap();
        let code = (((g >> 5) << 7) | (((g >> 4) & 1) << 5) | (g & 0xF)) as u16;
        assert_eq!(mode_c_altitude(code), Some(12300));
        assert_eq!(squawk(encode_squawk(0o1200)), 0o1200);
    }
}

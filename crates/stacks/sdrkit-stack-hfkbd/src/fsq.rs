//! FSQ and IFKP: 33-tone incremental keying with the base-32 varicodes, and FSQ's callsign check and line parser. The receiver
//! finds the band, the speed and the symbol timing by itself and decodes the tone steps. The generator sends the clean signal.
//!
//! Standard: M. Greenman ZL1BPU and C. Pidgeon ZL2AFP, "FSQ - Fast Simple QSO" and "IFKP" (published descriptions).

use crate::decoder::HeardCommand;
use crate::decoder::{
    self, last_call, put_text, Heard, HfGenerator, Modulator, GEN_CENTRE_HZ, GEN_MESSAGE,
};
use crate::kinds::FsqMessage;
use crate::typed::{FsqFields, FsqMessageRow, FSQ_MESSAGES};
use crate::util::Tone;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Schema, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port, RateFrom, RunContext, Timed};
use sdrkit_util_bits::crc::catalog::CRC8_SMBUS;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use sdrkit_util_text::store::TextStore;
use sdrkit_util_text::varicode::fsq::{self as varicode, Alphabet};
use std::sync::Arc;

/// FSQ or IFKP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// FSQ: 12 kHz, 8.789 Hz spacing.
    Fsq,
    /// IFKP: 16 kHz, 11.719 Hz spacing.
    Ifkp,
}

impl Kind {
    /// The stack's own sample rate.
    pub fn rate(self) -> f64 {
        match self {
            Kind::Fsq => 12000.0,
            Kind::Ifkp => 16000.0,
        }
    }
    /// Tone spacing: 3 bins of a 4096-point transform.
    pub fn spacing(self) -> f64 {
        3.0 * self.rate() / 4096.0
    }
    /// The 33-tone band, Hz.
    pub fn band(self) -> f64 {
        33.0 * self.spacing()
    }
    /// The speeds: (name, samples per symbol).
    pub fn speeds(self) -> &'static [(&'static str, usize)] {
        match self {
            Kind::Fsq => &[
                ("1.5", 8192),
                ("2", 6144),
                ("3", 4096),
                ("4.5", 3072),
                ("6", 2048),
            ],
            Kind::Ifkp => &[("0.5", 8192), ("1.0", 4096), ("2.0", 2048)],
        }
    }
    /// The name.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Fsq => "FSQ",
            Kind::Ifkp => "IFKP",
        }
    }
}

impl Kind {
    /// The varicode alphabet of the stack.
    pub fn alphabet(self) -> Alphabet {
        match self {
            Kind::Fsq => Alphabet::Fsq,
            Kind::Ifkp => Alphabet::Ifkp,
        }
    }
}

/// The check of an FSQ callsign: CRC-8/SMBUS (polynomial 0x07, initial 0, no reflection, no final XOR).
pub fn callsign_check(s: &[u8]) -> u8 {
    CRC8_SMBUS.checksum(s)
}

sdrkit_core::choice_enum! {
    /// The trigger of a directed FSQ message: what the sender requests.
    pub enum Trigger {
        /// " ": text.
        Text = ("text", "Text"),
        /// "?": requests the signal report.
        Report = ("report", "Asks for the signal report"),
        /// "*": requests an acknowledgement.
        Ack = ("ack", "Asks for an acknowledgement"),
        /// "+": requests a file.
        File = ("file", "Asks for a file"),
        /// ";": relay this message.
        Relay = ("relay", "Relay this message"),
        /// "!": repeat now.
        Repeat = ("repeat", "Repeat now"),
        /// "~": repeat in 15 s.
        RepeatLater = ("repeat_later", "Repeat in 15 s"),
        /// "#": store this text.
        Store = ("store", "Store this text"),
        /// "$": requests the heard list.
        Heard = ("heard", "Asks for the heard list"),
        /// "@": requests the location.
        Location = ("location", "Asks for the location"),
        /// "&": requests the message of the day.
        Motd = ("motd", "Asks for the message of the day"),
        /// "^": requests the software version.
        Version = ("version", "Asks for the software version"),
        /// "%": picture follows.
        Picture = ("picture", "Picture follows"),
        /// "|": alert.
        Alert = ("alert", "Alert"),
        /// ">": speed up.
        Faster = ("faster", "Speed up"),
        /// "<": speed down.
        Slower = ("slower", "Speed down"),
        /// "[": relayed message.
        Relayed = ("relayed", "Relayed message"),
    }
}

/// The trigger characters, in the order of [`Trigger`].
/// The trigger characters, in the order of [`Trigger`].
const TRIGGERS: &str = " ?*+;!~#$@&^%|><[";

impl Trigger {
    /// The trigger of character `c`.
    pub fn from_char(c: char) -> Option<Trigger> {
        let i = TRIGGERS.chars().position(|t| t == c)?;
        Trigger::from_index(i as u16)
    }
    /// Its character.
    pub fn char(self) -> char {
        TRIGGERS
            .chars()
            .nth(usize::from(self.index()))
            .unwrap_or(' ')
    }
}

/// An FSQ line: " \n" + call + ":" + CRC + body + "  \b  " for a directed message (`to` given), + text + "\n " otherwise.
/// The body of a directed message is the addressee, the trigger, a space and the text (the trigger of plain text is itself
/// a space, so then one space parts the addressee and the text).
pub fn frame(call: &str, to: Option<(&str, char)>, text: &str) -> String {
    let crc = callsign_check(call.as_bytes());
    match to {
        Some((to, ' ')) => format!(" \n{call}:{crc:02x}{to} {text}  \x08  "),
        Some((to, trig)) => format!(" \n{call}:{crc:02x}{to}{trig} {text}  \x08  "),
        None => format!(" \n{call}:{crc:02x}{text}\n "),
    }
}

/// Parses one FSQ line. `my_call`: empty for none.
pub fn parse_line(line: &str, my_call: &str, time: f64) -> FsqMessage {
    let mut m = FsqMessage {
        time,
        ..Default::default()
    };
    let line = line.trim_start_matches([' ', '\n']);
    let mut body = line;
    if let Some(colon) = line.find(':').filter(|&c| c > 0) {
        let call = &line[..colon];
        if !call.is_empty()
            && call.len() <= 12
            && call.chars().all(|c| c.is_ascii_alphanumeric() || c == '/')
        {
            let rest = &line[colon + 1..];
            if rest.len() >= 2 && rest.is_char_boundary(2) {
                let hex = &rest[..2];
                let crc_ok = u8::from_str_radix(hex, 16)
                    .map(|v| {
                        v == callsign_check(call.as_bytes())
                            || v == callsign_check(call.to_ascii_uppercase().as_bytes())
                    })
                    .unwrap_or(false);
                m.from = Some(call.to_ascii_uppercase());
                m.crc_ok = crc_ok;
                body = rest[2..].trim_start_matches(' ');
            }
        }
    }
    let mut text = body;
    if m.from.is_some() {
        // An addressee directly followed by a trigger.
        let end = body
            .char_indices()
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '/'));
        if let Some((i, trig)) = end {
            let to = &body[..i];
            if let Some(t) = Trigger::from_char(trig).filter(|_| !to.is_empty()) {
                let lower = to.to_ascii_lowercase();
                m.to = Some(if lower == "allcall" || lower == "cqcqcq" {
                    lower
                } else {
                    to.to_ascii_uppercase()
                });
                m.trigger = Some(t);
                text = &body[i + trig.len_utf8()..];
            }
        }
    }
    m.text = text
        .trim_end_matches(['\x08', ' ', '\n'])
        .trim_start()
        .to_string();
    if !my_call.is_empty() {
        if let Some(to) = &m.to {
            m.to_me = to.eq_ignore_ascii_case(my_call) || to == "allcall" || to == "cqcqcq";
        }
    }
    m
}

/// The base-32 varicode of a stack: a character is a symbol (0 to 28) or a symbol and a marker (29, 30 or 31).
pub fn code(kind: Kind, b: u8) -> Option<(u32, Option<u32>)> {
    let c = kind.alphabet().code(b).ok()?;
    Some((c.first as u32, c.marker.map(u32::from)))
}

sdrkit_core::choice_enum! {
    /// The FSQ speeds, baud.
    pub enum FsqSpeed {
        /// Found by itself.
        Auto = ("auto", "Auto"),
        /// 1.5.
        S1_5 = ("1.5", "1.5"),
        /// 2.
        S2 = ("2", "2"),
        /// 3.
        S3 = ("3", "3"),
        /// 4.5.
        S4_5 = ("4.5", "4.5"),
        /// 6.
        S6 = ("6", "6"),
    }
}

sdrkit_core::choice_enum! {
    /// The IFKP speeds, baud.
    pub enum IfkpSpeed {
        /// Found by itself.
        Auto = ("auto", "Auto"),
        /// 0.5.
        S0_5 = ("0.5", "0.5"),
        /// 1.0.
        S1 = ("1.0", "1.0"),
        /// 2.0.
        S2 = ("2.0", "2.0"),
    }
}

/// The `speed` setting of FSQ: auto, or an entry of [`Kind::speeds`] after it.
pub const FSQ_SPEED: Setting<field::Choice<FsqSpeed>> =
    Setting::choice("speed", "Speed", FsqSpeed::Auto);
/// The `speed` setting of IFKP: auto, or an entry of [`Kind::speeds`] after it.
pub const IFKP_SPEED: Setting<field::Choice<IfkpSpeed>> =
    Setting::choice("speed", "Speed", IfkpSpeed::Auto);

sdrkit_core::settings! {
    /// The settings of FSQ.
    pub enum FsqSetting {
        /// The speed.
        Speed = FSQ_SPEED,
    }
}

sdrkit_core::settings! {
    /// The settings of IFKP.
    pub enum IfkpSetting {
        /// The speed.
        Speed = IFKP_SPEED,
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of FSQ and IFKP.
    pub enum FsqGenSetting {
        /// The text sent. Empty: a directed message (FSQ) or a plain line (IFKP).
        Message = GEN_MESSAGE,
        /// The audio centre.
        CentreHz = GEN_CENTRE_HZ,
    }
}

impl Kind {
    /// The speed setting's choice index in `v` (0 for auto) of this kind.
    fn speed_index(self, id: SettingId, v: &Value) -> Result<u16, SettingError> {
        Ok(match self {
            Kind::Fsq => FSQ_SPEED.read(id, v)?.index(),
            Kind::Ifkp => IFKP_SPEED.read(id, v)?.index(),
        })
    }
}

/// The transmitter.
pub struct FsqModulator {
    kind: Kind,
    rate: f64,
    lo: f64,
    symlen: usize,
    syms: Vec<u32>,
    i: usize,
    tone: u32,
    osc: Tone,
    left: f64,
}

impl FsqModulator {
    /// Sends `text` repeated (with the idle symbols between repeats), the band centred on `centre`.
    pub fn new(kind: Kind, rate: f64, centre: f64, speed: usize, text: &str) -> Self {
        let mut syms = vec![];
        let idle: &[u32] = match kind {
            Kind::Fsq => &[28, 30],
            Kind::Ifkp => &[0],
        };
        syms.extend(idle);
        syms.extend(idle);
        for b in text.bytes() {
            if let Some((a, m)) = code(kind, b) {
                syms.push(a);
                if let Some(m) = m {
                    syms.push(m);
                }
            }
        }
        syms.extend(idle);
        syms.extend(idle);
        FsqModulator {
            kind,
            rate,
            lo: centre - 16.0 * kind.spacing(),
            symlen: speed,
            syms,
            i: 0,
            tone: 0,
            osc: Tone::new(rate),
            left: 0.0,
        }
    }
}

impl Modulator for FsqModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out {
            if self.left <= 0.0 {
                let s = self.syms[self.i % self.syms.len()];
                self.i += 1;
                self.tone = (self.tone + s + 1) % 33;
                self.left += self.symlen as f64 / self.kind.rate() * self.rate;
            }
            *o = self
                .osc
                .next(self.lo + self.tone as f64 * self.kind.spacing(), 1.0);
            self.left -= 1.0;
        }
    }
}

const HOPS: usize = 8;

/// One speed's symbol decoder.
struct SpeedRx {
    symlen: usize,
    spec: WindowedFft,
    nf: usize,
    hop: usize,
    hop_n: u64,
    last_dec: u64,
    sharp: [f32; HOPS],
    phase: usize,
    prev_bin: Option<f64>,
    quality: f32,
    syms: Vec<u32>,
    snr: f32,
    use_count: [f32; 73],
    lowest: f64,
    highest: f64,
    bad: u64,
    repeats: f32,
    held: Vec<u32>,
    decisions: u32,
}

impl SpeedRx {
    /// How well this speed fits: sharp tones that never repeat.
    fn score(&self) -> f32 {
        self.quality * (1.0 - 2.0 * self.repeats).max(0.0)
    }
    fn new(symlen: usize) -> Self {
        // A multiple of 4096 points, so a tone spacing is a whole number of bins (3 per 4096); 6144 samples take 8192.
        let nf = symlen.next_multiple_of(4096);
        SpeedRx {
            symlen,
            spec: WindowedFft::new(nf, symlen, Window::Rectangular)
                .expect("a window that fits the transform"),
            nf,
            hop: 0,
            hop_n: 0,
            last_dec: 0,
            sharp: [0.0; HOPS],
            phase: 0,
            prev_bin: None,
            quality: 0.0,
            syms: Vec::with_capacity(64),
            snr: 0.0,
            use_count: [0.0; 73],
            lowest: f64::MAX,
            highest: f64::MIN,
            bad: 0,
            repeats: 0.0,
            held: Vec::with_capacity(128),
            decisions: 0,
        }
    }
}

impl Shaped for FsqMessage {
    type Row = FsqMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&FsqMessageRow {
            time_s: self.time,
            from: self.from.as_deref(),
            crc_ok: self.crc_ok,
            to: self.to.as_deref(),
            trigger: self.trigger,
            text: &self.text,
            to_me: self.to_me,
        });
    }
}

/// The receiver.
pub struct FsqDecoder {
    kind: Kind,
    forced: Option<usize>,
    my_call: String,
    dial: f64,
    rs: Resampler<Complex32>,
    buf: Vec<Complex32>,
    hist: SampleRing<Complex32>,
    rx: Vec<SpeedRx>,
    chosen: usize,
    vari: varicode::Decoder,
    syms: Vec<u32>,
    line: String,
    line_t: f64,
    messages: LogRing<FsqMessage>,
    new_msgs: Vec<FsqMessage>,
    store: TextStore,
    heard: Heard,
    status: Status,
    tone_use: Option<SeriesId>,
    t_in: u64,
    state: u8,
}

impl FsqDecoder {
    /// A receiver at the default settings: the speed found by itself.
    pub fn new(kind: Kind) -> Self {
        FsqDecoder {
            kind,
            forced: None,
            my_call: String::new(),
            dial: 0.0,
            rs: Resampler::new(8000.0, kind.rate()).expect("the variant has a fixed positive rate"),
            buf: Vec::with_capacity(8192),
            hist: SampleRing::new(8192).expect("a ring of at least one sample"),
            rx: kind
                .speeds()
                .iter()
                .map(|&(_, l)| SpeedRx::new(l))
                .collect(),
            chosen: 0,
            vari: varicode::Decoder::new(kind.alphabet()),
            syms: Vec::with_capacity(128),
            line: String::with_capacity(512),
            line_t: 0.0,
            messages: LogRing::new(FSQ_MESSAGES.cap),
            new_msgs: Vec::new(),
            store: decoder::store(4000, 80),
            heard: Heard::new(kind.name()).with_not_calls(&["ALLCALL", "CQCQCQ"]),
            status: Status::default(),
            tone_use: None,
            t_in: 0,
            state: 0,
        }
    }

    fn reset(&mut self) {
        self.rx = self
            .kind
            .speeds()
            .iter()
            .map(|&(_, l)| SpeedRx::new(l))
            .collect();
        self.hist.reset();
        self.rs.reset();
        self.vari = varicode::Decoder::new(self.kind.alphabet());
        self.state = 0;
    }

    fn bin_range(&self, nf: usize) -> (usize, usize) {
        let fs = self.kind.rate();
        (
            (100.0 / fs * nf as f64) as usize,
            (3900.0 / fs * nf as f64).min(nf as f64 / 2.0 - 1.0) as usize,
        )
    }

    fn hop(&mut self, k: usize) {
        let (lo, hi) = self.bin_range(self.rx[k].nf);
        let r = &mut self.rx[k];
        let n = r.symlen;
        r.spec.run_power(self.hist.last(n));
        let p = r.spec.power();
        let (mut best, mut bi, mut tot) = (0.0f32, lo, 0.0f32);
        for (i, &v) in p.iter().enumerate().take(hi).skip(lo) {
            tot += v;
            if v > best {
                best = v;
                bi = i;
            }
        }
        let h = (r.hop_n % HOPS as u64) as usize;
        let s = if tot > 0.0 { best / tot } else { 0.0 };
        r.sharp[h] += 0.1 * (s - r.sharp[h]);
        r.hop_n += 1;
        if h != r.phase || r.hop_n < r.last_dec + HOPS as u64 / 2 {
            return;
        }
        r.last_dec = r.hop_n;
        let bp = (0..HOPS)
            .max_by(|&a, &b| r.sharp[a].total_cmp(&r.sharp[b]))
            .expect("the range is not empty");
        if bp != r.phase && r.sharp[bp] > 1.03 * r.sharp[r.phase] {
            let d = (bp + HOPS - r.phase) % HOPS;
            r.phase = if d <= HOPS / 2 {
                (r.phase + 1) % HOPS
            } else {
                (r.phase + HOPS - 1) % HOPS
            };
        }
        // Quality: the share of the symbol's energy in its strongest tone, over the 33-tone neighbourhood.
        let per = 3 * r.nf / 4096;
        let span = 33 * per;
        let a = bi.saturating_sub(span).max(lo);
        let b = (bi + span).min(hi);
        let local: f32 = p[a..b].iter().sum();
        let peak: f32 = p[bi.saturating_sub(per / 2).max(lo)..=(bi + per / 2).min(hi)]
            .iter()
            .sum();
        let q = if local > 0.0 { peak / local } else { 0.0 };
        r.decisions += 1;
        let alpha = (1.0 / r.decisions as f32).max(0.1);
        r.quality += alpha * (q - r.quality);
        let others = (local - peak) / (b - a).max(1) as f32 * per as f32;
        let snr = 10.0 * (peak / others.max(1e-20)).max(1e-6).log10() - 10.0 * (33.0f32).log10();
        r.snr += 0.1 * (snr - r.snr);
        // Parabolic peak.
        let (y0, y1, y2) = (p[bi - 1] as f64, p[bi] as f64, p[bi + 1] as f64);
        let den = y0 - 2.0 * y1 + y2;
        let fb = bi as f64
            + if den.abs() > 1e-30 {
                (0.5 * (y0 - y2) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
        let fhz = fb * self.kind.rate() / r.nf as f64;
        if q < 0.12 {
            // Too weak to trust: left out, the character starts again.
            r.bad += 1;
            r.prev_bin = None;
            r.syms.push(u32::MAX);
            return;
        }
        r.lowest = r.lowest.min(fhz);
        r.highest = r.highest.max(fhz);
        if let Some(pb) = r.prev_bin {
            let step = (fb - pb) / per as f64;
            let st = step.round();
            let rep = if st == 0.0 { 1.0 } else { 0.0 };
            r.repeats += 0.1 * (rep - r.repeats);
            if (step - st).abs() > 0.3 || st == 0.0 {
                r.syms.push(u32::MAX);
            } else {
                let sym = ((st as i64 - 1).rem_euclid(33)) as u32;
                r.syms.push(if sym > 31 { u32::MAX } else { sym });
            }
        }
        r.prev_bin = Some(fb);
    }

    fn symbol(&mut self, s: u32, out: &mut Outbox) {
        if s == u32::MAX {
            // A symbol too weak to trust: the character being received is dropped.
            self.vari = varicode::Decoder::new(self.kind.alphabet());
            return;
        }
        if let Some(Ok(b)) = self.vari.push(s as u8) {
            self.out(b, out);
        }
    }

    fn parse(&mut self, line: &str) {
        if self.kind != Kind::Fsq {
            return;
        }
        let m = parse_line(line, &self.my_call, self.line_t);
        if m.from.is_some() {
            self.heard.logs.push(format!(
                "message from {} to {}: {}",
                m.from.as_deref().unwrap_or("?"),
                m.to.as_deref().unwrap_or("-"),
                m.text
            ));
            self.messages.push(m.clone());
            self.new_msgs.push(m);
        }
    }

    fn out(&mut self, b: u8, out: &mut Outbox) {
        let time = self.t_in as f64 / 8000.0;
        let r = &self.rx[self.chosen];
        let snr = r.snr;
        let f = self.dial + 0.5 * (r.lowest + r.highest).max(0.0);
        self.heard
            .char(&mut self.store, out, b, time, 100, 0, f, snr);
        // FSQ lines: a line feed starts one (a line with text before it is a plain message), "  \b  " ends a directed one.
        if b == b'\n' {
            if !self.line.trim().is_empty() {
                let line = std::mem::take(&mut self.line);
                self.parse(&line);
            }
            self.line.clear();
            self.line.push('\n');
            self.line_t = time;
            return;
        }
        if self.line.is_empty() {
            self.line_t = time;
        }
        self.line.push(b as char);
        if self.line.ends_with("  \x08  ") {
            let line = std::mem::take(&mut self.line);
            self.parse(&line);
        }
        if self.line.len() > 400 {
            self.line.clear();
        }
    }
}

impl sdrkit_graph::Decoder for FsqDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        match self.kind {
            Kind::Fsq => "hfkbd.fsq",
            Kind::Ifkp => "hfkbd.ifkp",
        }
    }
    fn outputs(&self) -> Vec<Port> {
        let mut o = decoder::outputs();
        o.push(Port::typed::<FsqMessage>("messages").rate_from(RateFrom::Fixed(0.0)));
        o
    }
    fn settings(&self) -> Arc<Schema> {
        match self.kind {
            Kind::Fsq => FsqSetting::schema(),
            Kind::Ifkp => IfkpSetting::schema(),
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.my_call = ctx.station.callsign.trim().to_string();
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        // FSQ and IFKP declare their one setting at the same place.
        match FsqSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FsqSetting::Speed => {
                let i = self.kind.speed_index(id, value)?;
                self.forced = i.checked_sub(1).map(usize::from);
                if let Some(f) = self.forced {
                    self.chosen = f;
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let i = Value::Choice(self.forced.map_or(0, |f| f as u16 + 1));
        match self.kind {
            Kind::Fsq => values_of([(FsqSetting::Speed, i)]),
            Kind::Ifkp => values_of([(IfkpSetting::Speed, i)]),
        }
    }
    fn results(&self) -> Published {
        crate::typed::results(match self.kind {
            Kind::Fsq => "fsq",
            Kind::Ifkp => "ifkp",
        })
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.reset();
        let mut f = decoder::formats();
        f.push(Format::new(sdrkit_graph::Kind::typed::<FsqMessage>(), 0.0));
        Ok(f)
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.buf.clear();
        self.rs.process(x, &mut self.buf);
        let buf = std::mem::take(&mut self.buf);
        for &s in &buf {
            self.hist.push(s);
            for k in 0..self.rx.len() {
                if self.forced.is_some_and(|f| f != k) {
                    continue;
                }
                self.rx[k].hop += 1;
                if self.rx[k].hop == self.rx[k].symlen / HOPS {
                    self.rx[k].hop = 0;
                    self.hop(k);
                }
            }
            // The speed: the one with the sharpest tones.
            if self.forced.is_none() {
                let best = (0..self.rx.len())
                    .max_by(|&a, &b| self.rx[a].score().total_cmp(&self.rx[b].score()))
                    .expect("the receiver has one detector per speed");
                if best != self.chosen && self.rx[best].score() > 1.2 * self.rx[self.chosen].score()
                {
                    self.chosen = best;
                    self.vari = varicode::Decoder::new(self.kind.alphabet());
                }
            }
            // Each speed keeps its newest symbols, so the one chosen starts with what it heard before it was chosen.
            for r in &mut self.rx {
                let SpeedRx { syms, held, .. } = r;
                held.append(syms);
                if held.len() > 64 {
                    let cut = held.len() - 64;
                    held.drain(..cut);
                }
            }
            let q = self.rx[self.chosen].score();
            let decoding = q > 0.3;
            let mut syms = std::mem::take(&mut self.syms);
            syms.clear();
            if decoding {
                syms.append(&mut self.rx[self.chosen].held);
            }
            if decoding != (self.state == 2) {
                self.state = if decoding { 2 } else { 0 };
                if decoding {
                    let name = self.kind.speeds()[self.chosen].0;
                    self.heard
                        .logs
                        .push(format!("{} {name}: decoding", self.kind.name()));
                }
            }
            for &s in &syms {
                self.symbol(s, out);
            }
            self.syms = syms;
        }
        self.buf = buf;
        self.t_in += x.len() as u64;
        if !self.new_msgs.is_empty() {
            let o = out.items::<FsqMessage>(decoder::EXTRA);
            for m in self.new_msgs.drain(..) {
                o.push(Timed {
                    time: m.time,
                    value: m,
                });
            }
        }
        if self.state == 2 {
            let r = &mut self.rx[self.chosen];
            let lo = 100.0f64.max(r.lowest - 20.0 * self.kind.spacing());
            if r.highest > r.lowest {
                let k = ((r.highest - lo) / self.kind.spacing()).round() as usize;
                if k < 73 {
                    r.use_count[k] += 1.0;
                }
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        let r = &self.rx[self.chosen];
        let (name, l) = self.kind.speeds()[self.chosen];
        let baud = self.kind.rate() / l as f64;
        let decoding = self.state == 2;
        let band = self.kind.band();
        let st = &mut self.status;
        st.lock_state = if decoding {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = decoding.then_some(r.snr);
        st.offset_hz = Some(0.0);
        if decoding {
            put_text(
                &mut st.summary,
                format_args!(
                    "{} {baud:.1} baud: decoding, {band:.0} Hz band, {:.0} dB, {} chars",
                    self.kind.name(),
                    r.snr,
                    self.store.total
                ),
            );
        } else {
            put_text(
                &mut st.summary,
                format_args!("searching for {}", self.kind.name()),
            );
        }
        res.results_mut().set_status(st);
        let total: f32 = r.use_count.iter().sum();
        let heard = r.lowest < f64::MAX;
        self.heard.update(&self.store);
        res.put(&FsqFields {
            speed: name,
            band_hz: Hertz(band),
            symbol_rate_hz: Hertz(baud),
            lowest_hz: heard.then_some(Hertz(r.lowest)),
            highest_hz: heard.then_some(Hertz(r.highest)),
            quality_pct: (f64::from(r.quality.clamp(0.0, 1.0)) * 100.0).round(),
            snr_db: f64::from(r.snr),
            characters: self.store.total as i64,
            dropped_symbols: r.bad as i64,
            text: self.heard.text(),
            callsign: last_call(&self.store),
        });
        if self.tone_use.is_none() {
            self.tone_use = res.results().decl().series_id("tone_use");
        }
        if let Some(id) = self.tone_use {
            res.results_mut().update_series(id, |sr| {
                sr.points.clear();
                for (i, &c) in r.use_count.iter().enumerate() {
                    sr.points
                        .push([i as f32, if total > 0.0 { c / total } else { 0.0 }]);
                }
                true
            });
        }
        if self.kind == Kind::Fsq {
            shaped::log_rows(&self.messages, &FSQ_MESSAGES, 0.0, res);
        }
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
        self.messages.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
        if !kept.table(FSQ_MESSAGES.key) {
            self.messages.clear();
        }
    }
}

/// The generator. It reads the settings `speed` (auto sends 3 baud for FSQ, 1.0 for IFKP), `gen_message` and
/// `gen_centre_hz`. Without `gen_message` FSQ sends a directed message and IFKP a plain line.
pub fn generator(kind: Kind, cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let i = match kind {
        Kind::Fsq => v.get(&FSQ_SPEED).index(),
        Kind::Ifkp => v.get(&IFKP_SPEED).index(),
    };
    let l = usize::from(i)
        .checked_sub(1)
        .and_then(|i| kind.speeds().get(i))
        .map_or(4096, |x| x.1);
    let centre = v.get(&GEN_CENTRE_HZ);
    let default = match kind {
        Kind::Fsq => frame("k0tst", Some(("n0xyz", '?')), "hello from the test signal"),
        Kind::Ifkp => " \nK0TST: ifkp test message 0123456789\n ".into(),
    };
    let text = decoder::message(cfg, &default);
    let m = FsqModulator::new(kind, cfg.rate_hz, centre, l, &text);
    let baud = kind.rate() / l as f64;
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(kind.band()).with_symbol_rate(baud),
    ))
}

/// The FSQ generator.
pub fn fsq_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    generator(Kind::Fsq, cfg)
}

/// The IFKP generator.
pub fn ifkp_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    generator(Kind::Ifkp, cfg)
}

/// The audio centre of the FSQ and IFKP senders.
pub const TX_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("tx_centre_hz", "Audio centre", "Hz", 1500.0)
        .range(300.0, 3000.0, 1.0)
        .section(crate::sender::TX_SECTION)
        .help("The middle of the 33 tones sent.");

sdrkit_core::settings! {
    /// The settings of the FSQ sender.
    pub enum FsqTxSetting {
        /// The speed; auto sends 3 baud.
        Speed = FSQ_SPEED,
        /// The audio centre.
        CentreHz = TX_CENTRE_HZ,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

sdrkit_core::settings! {
    /// The settings of the IFKP sender.
    pub enum IfkpTxSetting {
        /// The speed; auto sends 1.0 baud.
        Speed = IFKP_SPEED,
        /// The audio centre.
        CentreHz = TX_CENTRE_HZ,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

sdrkit_core::commands! {
    /// The commands of the FSQ sender.
    pub enum FsqTxCommand {
        /// Sends a line.
        Send = crate::typed::FSQ_SEND.command,
        /// Stops.
        Stop = crate::typed::KEYBOARD_STOP,
        /// The tune carrier.
        Tune = crate::typed::KEYBOARD_TUNE,
    }
}

/// The tone that comes before the first symbol of a burst; it carries no data.
pub const FIRST_TONE: u32 = 5;

/// The coder of FSQ and IFKP: 33 tones, tone k (0 to 32) at centre + (k - 16) spacing, phase continuous. A burst is the first
/// tone 5, then each character's symbols of the stack's alphabet, each symbol s moving the tone to (previous + s + 1) mod 33;
/// then two idle characters (FSQ 28 30, IFKP 0).
pub struct FsqCoder {
    kind: Kind,
    centre: f64,
    symlen: usize,
    tone: u32,
}

impl FsqCoder {
    /// A coder of `kind` with symbols of `symlen` samples at the stack's own rate, around the audio centre `centre`.
    pub fn new(kind: Kind, centre: f64, symlen: usize) -> Self {
        FsqCoder {
            kind,
            centre,
            symlen,
            tone: FIRST_TONE,
        }
    }
    fn tone_element(&self) -> crate::sender::Element {
        let hz = self.centre + (f64::from(self.tone) - 16.0) * self.kind.spacing();
        crate::sender::Element::tone(hz, self.symlen as f64 / self.kind.rate())
    }
    fn symbol(&mut self, s: u32, out: &mut Vec<crate::sender::Element>) {
        self.tone = (self.tone + s + 1) % 33;
        out.push(self.tone_element());
    }
    fn idle(&self) -> &'static [u32] {
        match self.kind {
            Kind::Fsq => &[28, 30],
            Kind::Ifkp => &[0],
        }
    }
}

impl crate::sender::Coder for FsqCoder {
    fn begin(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.tone = FIRST_TONE;
        out.push(self.tone_element());
    }
    fn char(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        let Some((a, m)) = u8::try_from(c).ok().and_then(|b| code(self.kind, b)) else {
            return;
        };
        self.symbol(a, out);
        if let Some(m) = m {
            self.symbol(m, out);
        }
    }
    fn end(&mut self, out: &mut Vec<crate::sender::Element>) {
        for _ in 0..2 {
            for &s in self.idle() {
                self.symbol(s, out);
            }
        }
    }
}

/// The FSQ or IFKP side of the sender.
#[derive(Clone, Debug)]
pub struct FsqModem {
    kind: Kind,
    /// The speed: an entry of [`Kind::speeds`], `None` for auto.
    speed: Option<usize>,
    centre: f64,
    common: crate::sender::Common,
}

impl FsqModem {
    /// The sender of `kind` at the default settings.
    pub fn new(kind: Kind) -> Self {
        FsqModem {
            kind,
            speed: None,
            centre: TX_CENTRE_HZ.default,
            common: crate::sender::Common::default(),
        }
    }
    /// The samples of a symbol at the stack's own rate: the speed set, else 4096 (FSQ 3, IFKP 1.0).
    pub fn symlen(&self) -> usize {
        self.speed
            .and_then(|i| self.kind.speeds().get(i))
            .map_or(4096, |x| x.1)
    }
    /// Characters of the stack's alphabet as typed; CR LF and a lone CR are one line end.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        let start = out.len();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let c = match c {
                '\r' if chars.peek() == Some(&'\n') => continue,
                '\r' => '\n',
                c => c,
            };
            if u8::try_from(c)
                .ok()
                .and_then(|b| code(self.kind, b))
                .is_none()
            {
                out.truncate(start);
                return Err(crate::sender::TxError::Character(c));
            }
            out.push(c);
        }
        Ok(())
    }
    /// The 33 tones and half a spacing each side.
    fn band(&self) -> (f64, f64) {
        let half = 16.5 * self.kind.spacing();
        (self.centre - half, self.centre + half)
    }
    fn apply_speed(&mut self, i: u16) {
        self.speed = usize::from(i).checked_sub(1);
    }
}

/// The FSQ side of the sender.
#[derive(Clone, Debug)]
pub struct FsqTxModem(FsqModem);

impl Default for FsqTxModem {
    fn default() -> Self {
        FsqTxModem(FsqModem::new(Kind::Fsq))
    }
}

/// The IFKP side of the sender.
#[derive(Clone, Debug)]
pub struct IfkpTxModem(FsqModem);

impl Default for IfkpTxModem {
    fn default() -> Self {
        IfkpTxModem(FsqModem::new(Kind::Ifkp))
    }
}

/// The modem trait for a wrapper of [`FsqModem`] with the settings enum `$set` and the speed setting `$speed`.
macro_rules! fsq_modem {
    ($t:ty, $set:ident, $speed:ident, $name:literal) => {
        impl crate::sender::Modem for $t {
            const NAME: &'static str = $name;
            fn schema() -> Arc<Schema> {
                $set::schema()
            }
            fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
                let m = &mut self.0;
                match $set::from_id(id).ok_or(SettingError::NoSuchId { id })? {
                    $set::Speed => m.apply_speed($speed.read(id, v)?.index()),
                    $set::CentreHz => m.centre = TX_CENTRE_HZ.read(id, v)?,
                    $set::Level => m.common.level = crate::sender::TX_LEVEL.read(id, v)?,
                    $set::TuneLimit => {
                        m.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
                    }
                }
                Ok(())
            }
            fn values(&self) -> Values {
                let m = &self.0;
                let speed = m.speed.map_or(0, |i| i as u16 + 1);
                values_of([
                    ($set::Speed, Value::Choice(speed)),
                    ($set::CentreHz, Value::Hz(m.centre)),
                    ($set::Level, Value::F64(m.common.level)),
                    ($set::TuneLimit, Value::Duration(m.common.tune_s)),
                ])
            }
            fn common(&self) -> crate::sender::Common {
                self.0.common
            }
            fn accept(
                &self,
                text: &str,
                out: &mut Vec<char>,
            ) -> Result<(), crate::sender::TxError> {
                self.0.accept(text, out)
            }
            fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
                let m = &self.0;
                Ok(crate::sender::Coding::Elements(Box::new(FsqCoder::new(
                    m.kind,
                    m.centre,
                    m.symlen(),
                ))))
            }
            fn centre_hz(&self) -> f64 {
                self.0.centre
            }
            fn band(&self) -> (f64, f64) {
                self.0.band()
            }
        }
    };
}

fsq_modem!(IfkpTxModem, IfkpTxSetting, IFKP_SPEED, "hfkbd.ifkptx");

/// The FSQ line that `send` with `args` asks for from the station `station`: the callsigns in lower case, as FSQ stations
/// send them, the check over the call as sent.
///
/// # Errors
///
/// [`crate::sender::TxError::NoCall`] when the station callsign is not set; the errors of the arguments.
pub fn fsq_line(args: &Values, station: &str) -> Result<String, CommandError> {
    let a: crate::typed::FsqSendArgs = crate::typed::FSQ_SEND.args(args)?;
    if station.is_empty() {
        return Err(CommandError::Failed {
            source: sdrkit_core::StageError::new(crate::sender::TxError::NoCall),
        });
    }
    let call = station.to_ascii_lowercase();
    let to = a.to.unwrap_or("").trim().to_ascii_lowercase();
    let trigger = a.trigger.unwrap_or(Trigger::Text).char();
    let to = (!to.is_empty()).then_some((to.as_str(), trigger));
    Ok(frame(&call, to, a.text))
}

impl crate::sender::Modem for FsqTxModem {
    const NAME: &'static str = "hfkbd.fsqtx";
    fn schema() -> Arc<Schema> {
        FsqTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let m = &mut self.0;
        match FsqTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FsqTxSetting::Speed => m.apply_speed(FSQ_SPEED.read(id, v)?.index()),
            FsqTxSetting::CentreHz => m.centre = TX_CENTRE_HZ.read(id, v)?,
            FsqTxSetting::Level => m.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            FsqTxSetting::TuneLimit => {
                m.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let m = &self.0;
        let speed = m.speed.map_or(0, |i| i as u16 + 1);
        values_of([
            (FsqTxSetting::Speed, Value::Choice(speed)),
            (FsqTxSetting::CentreHz, Value::Hz(m.centre)),
            (FsqTxSetting::Level, Value::F64(m.common.level)),
            (FsqTxSetting::TuneLimit, Value::Duration(m.common.tune_s)),
        ])
    }
    fn common(&self) -> crate::sender::Common {
        self.0.common
    }
    fn commands(&self) -> &'static sdrkit_core::command::Commands {
        FsqTxCommand::commands()
    }
    fn action(
        &self,
        id: CommandId,
        args: &Values,
        station: &str,
    ) -> Result<crate::sender::Action, CommandError> {
        Ok(
            match FsqTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
                FsqTxCommand::Send => crate::sender::Action::Send(fsq_line(args, station)?),
                FsqTxCommand::Stop => crate::sender::Action::Stop,
                FsqTxCommand::Tune => crate::sender::Action::Tune,
            },
        )
    }
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        self.0.accept(text, out)
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        let m = &self.0;
        Ok(crate::sender::Coding::Elements(Box::new(FsqCoder::new(
            m.kind,
            m.centre,
            m.symlen(),
        ))))
    }
    fn centre_hz(&self) -> f64 {
        self.0.centre
    }
    fn band(&self) -> (f64, f64) {
        self.0.band()
    }
}

/// The FSQ sender component.
pub fn fsq_sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<FsqTxModem>::default())
}

/// The IFKP sender component.
pub fn ifkp_sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<IfkpTxModem>::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_frames() {
        assert_eq!(callsign_check(b"123456789"), 0xF4);
        assert_eq!(callsign_check(b"k0tst"), 0xA0);
        let crc = callsign_check(b"LY2ABC");
        assert_eq!(
            frame("LY2ABC", Some(("LY2XYZ", '?')), "hi"),
            format!(" \nLY2ABC:{crc:02x}LY2XYZ? hi  \x08  ")
        );
        assert!((Kind::Fsq.spacing() - 8.7890625).abs() < 1e-9);
        assert!((Kind::Ifkp.spacing() - 11.71875).abs() < 1e-9);
        assert!((Kind::Fsq.band() - 290.0).abs() < 1.0);
        assert!((Kind::Ifkp.band() - 386.7).abs() < 1.0);
    }

    #[test]
    fn the_line_parser() {
        let crc = callsign_check(b"LY2ABC");
        let m = parse_line(
            &format!("LY2ABC:{crc:02x}LY2XYZ? snr please"),
            "LY2XYZ",
            0.0,
        );
        assert_eq!(m.from.as_deref(), Some("LY2ABC"));
        assert!(m.crc_ok);
        assert_eq!(m.to.as_deref(), Some("LY2XYZ"));
        assert_eq!(m.trigger, Some(Trigger::Report));
        assert_eq!(m.text, "snr please");
        assert!(m.to_me);
        let m = parse_line(&format!("LY2ABC:{crc:02x}allcall$"), "", 0.0);
        assert_eq!(m.to.as_deref(), Some("allcall"));
        assert_eq!(m.trigger, Some(Trigger::Heard));
        assert_eq!(m.text, "");
        assert!(!m.to_me);
        let m = parse_line("LY2ABC:00LY2XYZ hi", "LY2XYZ", 0.0);
        assert!(!m.crc_ok);
        assert_eq!(m.from.as_deref(), Some("LY2ABC"));
        let m = parse_line("cq cq cq de ly2abc", "LY2XYZ", 0.0);
        assert!(m.from.is_none() && m.to.is_none());
    }
}

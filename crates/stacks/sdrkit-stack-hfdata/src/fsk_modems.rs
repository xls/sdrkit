//! Telephone FSK modems heard as audio in an SSB channel: Bell 103 (originate and answer), ITU-T V.21 (channels 1 and 2), ITU-T
//! V.23 mode 2 and Bell 202. Bursts of asynchronous bytes (a start bit, eight data bits least significant first, stop bits)
//! are found, the standard, the sideband and the dial error that fit are chosen, and each burst is shown as bytes. The meaning
//! of the bytes is not decoded (the time code of the Canadian time station CHU, sent in Bell 103 answer tones, is shown as
//! bytes).
//!
//! Standard: ITU-T V.21 (11/1988) and V.23 for the tones and speeds of their channels; the Bell 103 and Bell 202 tone plans
//! as publicly described. The framing of the bytes is that of start-stop serial links.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [fsk_modems: low pass to 8 kHz, the burst detector on 32 ms spectra, per burst: the sideband,
//!        the standards and dial errors that fit, a start-stop trial for each, the winner by captured energy] -> rows
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, Lcg, Playback, RowLog, Sender, StatusLine, Tone};
use crate::typed::{FskModemsFields, ModemMessageEvent, SenderRow, MODEM_MESSAGE, SENDER_ROWS};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::charframe::CharFramer;
use sdrkit_algo_modem::stretchburst::{StretchConfig, StretchDetector};
use sdrkit_core::interface::{Fields as _, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, SubMode, Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 8000.0;
/// The frame of the burst detector.
const FRAME: usize = 256;

sdrkit_core::choice_enum! {
    /// The standard.
    pub enum Standard {
        /// Every standard is tried.
        Auto = ("auto", "Find it"),
        /// Bell 103, originate.
        Bell103Orig = ("bell103_orig", "Bell 103 originate"),
        /// Bell 103, answer.
        Bell103Ans = ("bell103_ans", "Bell 103 answer"),
        /// V.21 channel 1.
        V21Ch1 = ("v21_ch1", "V.21 channel 1"),
        /// V.21 channel 2.
        V21Ch2 = ("v21_ch2", "V.21 channel 2"),
        /// V.23 mode 2.
        V23Mode2 = ("v23_mode2", "V.23 mode 2"),
        /// Bell 202.
        Bell202 = ("bell202", "Bell 202"),
    }
}

impl Standard {
    /// The six standards.
    pub const ALL: [Standard; 6] = [
        Standard::Bell103Orig,
        Standard::Bell103Ans,
        Standard::V21Ch1,
        Standard::V21Ch2,
        Standard::V23Mode2,
        Standard::Bell202,
    ];

    /// The mark tone, the space tone and the speed.
    pub fn plan(self) -> (f64, f64, f64) {
        match self {
            Standard::Auto | Standard::Bell103Orig => (1270.0, 1070.0, 300.0),
            Standard::Bell103Ans => (2225.0, 2025.0, 300.0),
            Standard::V21Ch1 => (980.0, 1180.0, 300.0),
            Standard::V21Ch2 => (1650.0, 1850.0, 300.0),
            Standard::V23Mode2 => (1300.0, 2100.0, 1200.0),
            Standard::Bell202 => (1200.0, 2200.0, 1200.0),
        }
    }

    /// The name of the row.
    pub fn name(self) -> &'static str {
        match self {
            Standard::Auto => "",
            Standard::Bell103Orig => "Bell 103 originate, 300 baud",
            Standard::Bell103Ans => "Bell 103 answer, 300 baud",
            Standard::V21Ch1 => "V.21 channel 1, 300 baud",
            Standard::V21Ch2 => "V.21 channel 2, 300 baud",
            Standard::V23Mode2 => "V.23 mode 2, 1200 baud",
            Standard::Bell202 => "Bell 202, 1200 baud",
        }
    }
}

sdrkit_core::choice_enum! {
    /// The sideband.
    pub enum Sideband {
        /// The stronger.
        Auto = ("auto", "The stronger"),
        /// Upper.
        Upper = ("upper", "Upper"),
        /// Lower.
        Lower = ("lower", "Lower"),
    }
}

sdrkit_core::choice_enum! {
    /// The standard of the test signal.
    pub enum GenStandard {
        /// Bell 103, originate.
        Bell103Orig = ("bell103_orig", "Bell 103 originate"),
        /// Bell 103, answer.
        Bell103Ans = ("bell103_ans", "Bell 103 answer"),
        /// V.21 channel 1.
        V21Ch1 = ("v21_ch1", "V.21 channel 1"),
        /// V.21 channel 2.
        V21Ch2 = ("v21_ch2", "V.21 channel 2"),
        /// V.23 mode 2.
        V23Mode2 = ("v23_mode2", "V.23 mode 2"),
        /// Bell 202.
        Bell202 = ("bell202", "Bell 202"),
    }
}

impl GenStandard {
    /// The standard.
    pub fn standard(self) -> Standard {
        Standard::ALL[usize::from(self.index())]
    }
}

/// The standard read.
pub const SUBMODE: Setting<field::Choice<Standard>> =
    Setting::choice("submode", "Standard", Standard::Auto)
        .help("Find it tries the six standards and names the one that fits.");
/// The dial error searched.
pub const DIAL_SEARCH_HZ: Setting<field::Hz> = Setting::new(
    "dial_search_hz",
    "Dial error searched",
    "Hz",
    40.0,
)
.range(0.0, 80.0, 10.0)
.help(
    "The dial error is tried over plus and minus this, in 10 Hz steps, then refined in 2 Hz steps.",
);
/// The sideband.
pub const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("sideband", "Sideband", Sideband::Auto);
/// The standard of the test signal.
pub const GEN_STANDARD: Setting<field::Choice<GenStandard>> =
    Setting::choice("gen_standard", "Standard", GenStandard::Bell103Ans)
        .section(GEN_SECTION)
        .advanced();
/// The bytes of the test signal.
pub const GEN_COUNT: Setting<field::I64> = Setting::new("gen_count", "Bytes", "", 24)
    .range(1.0, 1000.0, 1.0)
    .help("Random printable bytes from the seed.")
    .section(GEN_SECTION)
    .advanced();
/// The stop bits of the test signal.
pub const GEN_STOP_BITS: Setting<field::I64> = Setting::new("gen_stop_bits", "Stop bits", "", 1)
    .range(1.0, 2.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// Idle bits between the bytes of the test signal.
pub const GEN_GAP_BITS: Setting<field::I64> =
    Setting::new("gen_gap_bits", "Idle bits between bytes", "", 0)
        .range(0.0, 100.0, 1.0)
        .section(GEN_SECTION)
        .advanced();
/// The idle mark before and after the bytes.
pub const GEN_IDLE_S: Setting<field::Duration> = Setting::new("gen_idle_s", "Idle mark", "s", 0.4)
    .range(0.0, 10.0, 0.01)
    .section(GEN_SECTION)
    .advanced();
/// The silence before.
pub const GEN_LEAD_S: Setting<field::Duration> =
    Setting::new("gen_lead_s", "Silence before", "s", 1.0)
        .range(0.0, 10.0, 0.1)
        .section(GEN_SECTION)
        .advanced();
/// The silence after.
pub const GEN_TAIL_S: Setting<field::Duration> =
    Setting::new("gen_tail_s", "Silence after", "s", 1.0)
        .range(0.0, 10.0, 0.1)
        .section(GEN_SECTION)
        .advanced();
/// The test signal in the lower sideband.
pub const GEN_LOWER: Setting<field::Bool> = Setting::new("gen_lower", "Lower sideband", "", false)
    .section(GEN_SECTION)
    .advanced();
/// A wrong speed of the test signal.
pub const GEN_BAUD_SCALE: Setting<field::F64> =
    Setting::new("gen_baud_scale", "Speed factor", "", 1.0)
        .range(0.25, 4.0, 0.01)
        .help(
            "The speed of the test signal times this (a wrong speed for a test of false decodes).",
        )
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the modem receiver.
    pub enum FskModemsRxSetting {
        /// Standard.
        Submode = SUBMODE,
        /// Dial error searched.
        DialSearchHz = DIAL_SEARCH_HZ,
        /// Sideband.
        Sideband = SIDEBAND,
    }
}

sdrkit_core::settings! {
    /// The settings of the modem stack.
    pub enum FskModemsSetting {
        /// Standard.
        Submode = SUBMODE,
        /// Dial error searched.
        DialSearchHz = DIAL_SEARCH_HZ,
        /// Sideband.
        Sideband = SIDEBAND,
        /// Standard of the test signal.
        GenStandard = GEN_STANDARD,
        /// Bytes of the test signal.
        GenCount = GEN_COUNT,
        /// Stop bits.
        GenStopBits = GEN_STOP_BITS,
        /// Idle bits between bytes.
        GenGapBits = GEN_GAP_BITS,
        /// Idle mark.
        GenIdleS = GEN_IDLE_S,
        /// Silence before.
        GenLeadS = GEN_LEAD_S,
        /// Silence after.
        GenTailS = GEN_TAIL_S,
        /// Lower sideband.
        GenLower = GEN_LOWER,
        /// Speed factor.
        GenBaudScale = GEN_BAUD_SCALE,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "The tones and speeds are those of the standards. Not confirmed by a document read: the framing \
of the bytes (one start bit, eight data bits least significant first, one or more stop bits). The meaning of the bytes, the \
CHU time code included, is not decoded. A burst that starts without idle mark loses its first byte.";

/// A message read from one burst.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// The standard.
    pub standard: Standard,
    /// The signal is in the lower sideband.
    pub lower: bool,
    /// The dial error, Hz.
    pub dial_hz: f64,
    /// The bytes.
    pub bytes: Vec<u8>,
    /// Framing errors.
    pub framing: usize,
    /// The length of the burst, seconds.
    pub secs: f64,
    /// The SNR in the band, dB.
    pub snr_db: f64,
}

impl Message {
    /// The text of the row.
    pub fn text(&self) -> String {
        let (m, s, _) = self.standard.plan();
        let mut hex: Vec<String> = self
            .bytes
            .iter()
            .take(48)
            .map(|b| format!("{b:02X}"))
            .collect();
        if self.bytes.len() > 48 {
            hex.push("...".into());
        }
        let ascii: String = self
            .bytes
            .iter()
            .map(|&b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        format!(
            "{} bytes: {} | {} | tones {:.0} / {:.0} Hz ({} sideband, offset {:+.0} Hz), {} framing errors, {:.2} s",
            self.bytes.len(),
            hex.join(" "),
            ascii,
            m,
            s,
            if self.lower { "lower" } else { "upper" },
            self.dial_hz,
            self.framing,
            self.secs
        )
    }
}

/// The result of one trial.
#[derive(Clone, Debug, Default)]
struct Trial {
    bytes: Vec<u8>,
    framing: usize,
    decisive: f64,
    energy: f64,
}

/// A start-stop trial of `x` with the mark and space at `mark_hz` and `space_hz` (signed: negative
/// in the lower sideband).
fn trial(x: &[Complex32], mark_hz: f64, space_hz: f64, baud: f64) -> Trial {
    let l = ((RATE / baud).round() as usize).max(4);
    let spb = RATE / baud;
    let n = x.len();
    let mut d = vec![0f32; n];
    let mut e = vec![0f32; n];
    let (mut sm, mut ss) = (Complex32::default(), Complex32::default());
    let wm = Complex32::from_polar(1.0, (-TAU * mark_hz / RATE) as f32);
    let ws = Complex32::from_polar(1.0, (-TAU * space_hz / RATE) as f32);
    let mut rm = Vec::with_capacity(n);
    let mut rs = Vec::with_capacity(n);
    let (mut pm, mut ps) = (Complex32::new(1.0, 0.0), Complex32::new(1.0, 0.0));
    for i in 0..n {
        let a = x[i] * pm;
        let b = x[i] * ps;
        pm *= wm;
        ps *= ws;
        if i % 256 == 255 {
            pm /= pm.norm();
            ps /= ps.norm();
        }
        rm.push(a);
        rs.push(b);
        sm += a;
        ss += b;
        if i >= l {
            sm -= rm[i - l];
            ss -= rs[i - l];
        }
        let (em, es) = (sm.norm_sqr(), ss.norm_sqr());
        d[i] = if em + es > 0.0 {
            (em - es) / (em + es)
        } else {
            0.0
        };
        e[i] = em.max(es);
    }
    if n <= 2 * l {
        return Trial::default();
    }
    let mut sorted = e[2 * l..].to_vec();
    sorted.sort_by(f32::total_cmp);
    let thr = 0.3 * sorted[(sorted.len() * 8 / 10).min(sorted.len() - 1)];
    let Some(begin) = (2 * l..n).find(|&i| d[i] >= 0.5) else {
        return Trial::default();
    };
    let mut f = CharFramer::new(spb, 8, false).expect("valid framer");
    let mut t = Trial::default();
    for (i, &di) in d.iter().enumerate().skip(begin) {
        let c = f.push(di);
        let bad = f.framing_error();
        if c.is_none() && !bad {
            continue;
        }
        let at = |k: f64| (begin as f64 + k).round() as usize;
        let read: Vec<usize> = f.bits().iter().map(|b| at(b.0).min(n - 1)).collect();
        if read.iter().any(|&j| e[j] <= thr) {
            continue;
        }
        let _ = i;
        match c {
            Some(v) => {
                t.bytes.push(v);
                t.decisive += read[..9]
                    .iter()
                    .map(|&j| f64::from(d[j].abs()))
                    .sum::<f64>()
                    / 9.0;
                t.energy += read[..9].iter().map(|&j| f64::from(e[j])).sum::<f64>();
            }
            None => t.framing += 1,
        }
    }
    t
}

/// Reads one burst of channel samples: the sideband, the standards and dial errors that fit, a
/// trial for each, the winner, its refinement.
pub fn read_burst(
    x: &[Complex32],
    standard: Standard,
    sideband: Sideband,
    search_hz: f64,
) -> Option<Message> {
    let n = 1024;
    if x.len() < 2 * n {
        return None;
    }
    let mut fft = Fft::forward(n);
    let win: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32)
        .collect();
    let mut p = vec![0f64; n];
    let mut buf = vec![Complex32::default(); n];
    let mut segs = 0;
    let mut at = 0;
    while at + n <= x.len() {
        for i in 0..n {
            buf[i] = x[at + i] * win[i];
        }
        fft.process(&mut buf);
        for (k, v) in buf.iter().enumerate() {
            p[k] += f64::from(v.norm_sqr());
        }
        segs += 1;
        at += n / 2;
    }
    let bw = RATE / n as f64;
    let freq = |k: usize| {
        if k < n / 2 {
            k as f64 * bw
        } else {
            (k as f64 - n as f64) * bw
        }
    };
    let band = |lo: f64, hi: f64, neg: bool| -> f64 {
        (0..n)
            .filter(|&k| {
                let f = freq(k);
                let f = if neg { -f } else { f };
                f >= lo && f <= hi
            })
            .map(|k| p[k])
            .sum()
    };
    let mut noise_bins: Vec<f64> = (0..n)
        .filter(|&k| (3050.0..=3400.0).contains(&freq(k).abs()))
        .map(|k| p[k])
        .collect();
    if noise_bins.len() < 8 || segs < 2 {
        return None;
    }
    noise_bins.sort_by(f64::total_cmp);
    let noise = noise_bins[noise_bins.len() / 2];
    let (up, lo) = (band(600.0, 2700.0, false), band(600.0, 2700.0, true));
    if up + lo < 8.0 * noise * (2100.0 / bw) {
        return None;
    }
    let lower = match sideband {
        Sideband::Auto => lo > up,
        Sideband::Upper => false,
        Sideband::Lower => true,
    };
    let (sig, other) = if lower { (lo, up) } else { (up, lo) };
    if other > 0.25 * sig {
        return None;
    }
    let snr = 10.0
        * ((sig - noise * 2100.0 / bw) / (noise * 2100.0 / bw))
            .max(1e-6)
            .log10();
    let sign = if lower { -1.0 } else { 1.0 };
    let standards: Vec<Standard> = if standard == Standard::Auto {
        Standard::ALL.to_vec()
    } else {
        vec![standard]
    };
    let fits = |st: Standard, dial: f64| -> bool {
        let (m, s, baud) = st.plan();
        let h = baud / 2.0 + 40.0;
        let pm = band(m + dial - h, m + dial + h, lower);
        let ps = band(s + dial - h, s + dial + h, lower);
        pm >= 0.06 * sig && ps >= 0.06 * sig && pm + ps >= 0.6 * sig
    };
    let run = |st: Standard, dial: f64| -> Trial {
        let (m, s, baud) = st.plan();
        trial(x, sign * (m + dial), sign * (s + dial), baud)
    };
    let counts = |t: &Trial| {
        t.bytes.len() >= 4
            && t.framing <= t.bytes.len() * 15 / 100
            && t.decisive / t.bytes.len().max(1) as f64 >= 0.45
    };
    let mut best: Option<(Standard, f64, Trial)> = None;
    let steps = (search_hz / 10.0).floor() as i64;
    for &st in &standards {
        for k in -steps..=steps {
            let dial = 10.0 * k as f64;
            if !fits(st, dial) {
                continue;
            }
            let t = run(st, dial);
            if counts(&t) && best.as_ref().is_none_or(|b| t.energy > b.2.energy) {
                best = Some((st, dial, t));
            }
        }
    }
    let (st, dial0, mut t) = best?;
    let mut dial = dial0;
    for k in -4..=4 {
        let dd = dial0 + 2.0 * f64::from(k);
        let c = run(st, dd);
        if counts(&c) && c.bytes.len() >= t.bytes.len() && c.energy > t.energy {
            t = c;
            dial = dd;
        }
    }
    Some(Message {
        standard: st,
        lower,
        dial_hz: dial,
        bytes: t.bytes,
        framing: t.framing,
        secs: x.len() as f64 / RATE,
        snr_db: snr,
    })
}

/// The burst detector: a stretch as wide as the widest standard (Bell 202, 2200 Hz) in either sideband, three frames over 3
/// times the noise (noise passes it in 0.001 % of frames) open a burst and six under 1.6 (0.2 s) close it, after
/// 0.3 s of settling of the filters.
fn detector() -> StretchDetector {
    let mut c = StretchConfig::new(
        RATE,
        FRAME,
        vec![(-2800.0, -350.0), (350.0, 2800.0)],
        2400.0,
    );
    c.open_ratio = 3.0;
    c.open_frames = 3;
    c.close_frames = 6;
    c.settle = (0.3 * RATE) as usize;
    c.min_len = (0.2 * RATE) as usize;
    c.max_len = (25.0 * RATE) as usize;
    StretchDetector::new(c).expect("valid detector")
}

/// The modem layer.
pub struct FskModems {
    standard: Standard,
    search_hz: f64,
    sideband: Sideband,
    detector: StretchDetector,
    rows: RowLog<(f64, Message)>,
    messages: i64,
    last: Option<Message>,
}

impl Default for FskModems {
    fn default() -> Self {
        Self::new()
    }
}

impl FskModems {
    /// A layer at the default settings.
    pub fn new() -> Self {
        FskModems {
            standard: Standard::Auto,
            search_hz: 40.0,
            sideband: Sideband::Auto,
            detector: detector(),
            rows: RowLog::default(),
            messages: 0,
            last: None,
        }
    }

    fn burst(&mut self, b: &[Complex32], cx: &mut Cx) {
        let Some(m) = read_burst(b, self.standard, self.sideband, self.search_hz) else {
            return;
        };
        let (mk, sp, _) = m.standard.plan();
        let hex: String = m.bytes.iter().map(|b| format!("{b:02X}")).collect();
        cx.events.push(MODEM_MESSAGE.event(
            cx.time,
            cx.dial,
            &ModemMessageEvent {
                text: &m.text(),
                standard: m.standard.name(),
                sideband: if m.lower { "lower" } else { "upper" },
                tones: &format!("{mk:.0} / {sp:.0} Hz"),
                bytes: &hex,
                framing_errors: m.framing as i64,
                duration: Seconds(m.secs),
            },
        ));
        self.messages += 1;
        self.last = Some(m.clone());
        self.rows.push((cx.time, m));
    }
}

impl Layer for FskModems {
    const NAME: &'static str = "hfdata.fsk_modems";
    const DECIM: usize = 3;
    const PASS_HZ: f64 = 3400.0;
    fn schema() -> Arc<Schema> {
        FskModemsRxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(FskModemsFields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&MODEM_MESSAGE)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FskModemsRxSetting::Submode,
                Value::Choice(self.standard.index()),
            ),
            (FskModemsRxSetting::DialSearchHz, Value::Hz(self.search_hz)),
            (
                FskModemsRxSetting::Sideband,
                Value::Choice(self.sideband.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match FskModemsRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FskModemsRxSetting::Submode => self.standard = SUBMODE.read(id, value)?,
            FskModemsRxSetting::DialSearchHz => self.search_hz = DIAL_SEARCH_HZ.read(id, value)?,
            FskModemsRxSetting::Sideband => self.sideband = SIDEBAND.read(id, value)?,
        }
        Ok(())
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        let mut at = 0;
        while at < x.len() {
            at += self.detector.process(&x[at..]);
            if let Some(b) = self.detector.take() {
                let b = b.samples.to_vec();
                self.burst(&b, cx);
            }
        }
    }
    fn finish(&mut self, cx: &mut Cx) {
        self.detector.end();
        if let Some(b) = self.detector.take() {
            let b = b.samples.to_vec();
            self.burst(&b, cx);
        }
    }
    fn reset(&mut self) {
        self.detector.reset();
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.messages = 0;
        self.last = None;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        self.messages = 0;
        self.last = None;
    }
    fn report(&mut self, res: &mut Published, level_dbfs: f64) -> StatusLine {
        self.rows.report(
            res,
            |r| r.clear_rows(&SENDER_ROWS),
            |r, key, (time, m)| {
                SENDER_ROWS.put(
                    r,
                    key,
                    &SenderRow {
                        time: Time(*time),
                        kind: "Message",
                        id: m.standard.name(),
                        text: &m.text(),
                        snr: Some((m.snr_db * 10.0).round() / 10.0),
                    },
                );
            },
        );
        let last = self.last.as_ref();
        let tones = last.map(|m| {
            let (mk, sp, _) = m.standard.plan();
            format!(
                "{mk:.0} / {sp:.0} Hz, {} sideband, offset {:+.0} Hz",
                if m.lower { "lower" } else { "upper" },
                m.dial_hz
            )
        });
        let summary_last = last.map(|m| {
            format!(
                "{} bytes, {} framing errors, {:.2} s",
                m.bytes.len(),
                m.framing,
                m.secs
            )
        });
        let state: u8 = if self.messages > 0 {
            2
        } else if self.detector.is_open() {
            1
        } else {
            0
        };
        res.put(&FskModemsFields {
            note: NOTE_TEXT,
            state: i64::from(state),
            standard: last.map(|m| m.standard.name()),
            tones: tones.as_deref(),
            last_message: summary_last.as_deref(),
            messages: self.messages,
            level_dbfs,
        });
        StatusLine {
            state,
            summary: match last {
                Some(m) => format!("{}: {} messages", m.standard.name(), self.messages),
                None => "no modem burst read".to_string(),
            },
            snr_db: last.map(|m| m.snr_db),
            offset_hz: last.map(|m| m.dial_hz),
        }
    }
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    FskModemsSetting::schema()
}

/// `n` random printable bytes from `seed`.
pub fn test_bytes(n: usize, seed: u64) -> Vec<u8> {
    let mut r = Lcg(seed ^ 0xB3);
    (0..n).map(|_| 0x20 + r.below(95) as u8).collect()
}

/// The tone sequence of `bytes` in `standard` (amplitude 0.5): `lead_s` of silence, `idle_s` of mark, each byte with its start
/// bit, eight data bits least significant first, `stops` stop bits and `gap` idle bits, `idle_s` of mark, `tail_s` of silence.
#[allow(clippy::too_many_arguments)]
pub fn test_sequence(
    standard: Standard,
    bytes: &[u8],
    stops: usize,
    gap: usize,
    idle_s: f64,
    lead_s: f64,
    tail_s: f64,
    lower: bool,
    baud_scale: f64,
) -> Vec<Tone> {
    let (m, s, baud) = standard.plan();
    let unit = 1.0 / (baud * baud_scale);
    let sign = if lower { -1.0 } else { 1.0 };
    let tone = |secs: f64, f: f64| Tone {
        secs,
        hz: Some(sign * f),
        amp: 0.5,
    };
    let mut seq = vec![Tone::silence(lead_s), tone(idle_s, m)];
    for &b in bytes {
        seq.push(tone(unit, s));
        for k in 0..8 {
            seq.push(tone(unit, if (b >> k) & 1 == 1 { m } else { s }));
        }
        seq.push(tone(unit * (stops + gap) as f64, m));
    }
    seq.push(tone(idle_s, m));
    seq.push(Tone::silence(tail_s));
    seq
}

/// The test signal.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let st = s.get(&GEN_STANDARD).standard();
    let seq = test_sequence(
        st,
        &test_bytes(s.get(&GEN_COUNT).clamp(1, 1000) as usize, cfg.seed),
        s.get(&GEN_STOP_BITS).clamp(1, 2) as usize,
        s.get(&GEN_GAP_BITS).clamp(0, 100) as usize,
        s.get(&GEN_IDLE_S),
        s.get(&GEN_LEAD_S),
        s.get(&GEN_TAIL_S),
        s.get(&GEN_LOWER),
        s.get(&GEN_BAUD_SCALE),
    );
    Ok(Box::new(Playback::new(
        &seq,
        cfg.rate_hz,
        SignalInfo {
            symbol_rate_hz: Some(st.plan().2),
            power: 0.25,
            ..SignalInfo::new(3000.0)
        },
    )))
}

/// `cfg` with its submode written into the `submode` setting.
fn with_submode(cfg: &StackConfig) -> Result<StackConfig, ConfigError> {
    let mut cfg = cfg.clone();
    if let Some(sub) = cfg.submode.as_deref().and_then(Standard::from_key) {
        if let Ok(id) = schema().id(SUBMODE.field.key) {
            cfg.settings.set(id, &Value::Choice(sub.index()))?;
        }
    }
    Ok(cfg)
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("fsk_modems"),
        "FSK modems",
        Sender::new(FskModems::new()),
        &with_submode(cfg)?,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

const fn sub(id: &'static str, name: &'static str) -> SubMode {
    SubMode {
        id,
        name,
        default_hz: TEST_SIGNAL_HZ,
    }
}

/// The telephone FSK modems.
pub static FSK_MODEMS: StackDescriptor = StackDescriptor {
    id: "fsk_modems",
    name: "Telephone FSK modems",
    group: "HF utility",
    blurb: "Bell 103, V.21, V.23 mode 2 and Bell 202 bursts heard in an SSB channel: the standard, sideband and dial error found, the bytes of each burst (CHU's time code as bytes).",
    tuning: crate::ms110::hf_tuning(TEST_SIGNAL_HZ),
    submodes: &[
        sub("auto", "Find it"),
        sub("bell103_orig", "Bell 103 originate"),
        sub("bell103_ans", "Bell 103 answer"),
        sub("v21_ch1", "V.21 channel 1"),
        sub("v21_ch2", "V.21 channel 2"),
        sub("v23_mode2", "V.23 mode 2"),
        sub("bell202", "Bell 202"),
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FSK_MODEMS),
    benchmarks: Some(crate::benchmarks),
};

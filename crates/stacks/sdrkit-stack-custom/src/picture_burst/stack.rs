//! The `picture_burst` stack: its settings, results, live receiver (a capture of each burst found by its start sweep), transmit
//! branch, test signal and descriptor.

use super::rx::{decode, xcorr_power, Decoded};
use super::{make, test_picture, up_sweep, FS, SWEEP};
use crate::family::DEMO_CALLSIGN;
use crate::typed::{
    PictureBurstEvent, PictureBurstFields, PictureBurstPictureRow, TxEvent, PICTURE_BURST_PICTURE,
    PICTURE_BURST_PICTURES, PICTURE_BURST_TX,
};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::sinc_table::SincConverter;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Codec, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Picture, PixelFormat,
    PixelLayout, Record, Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor,
    StageError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format, Io, Kept, Kind, Outbox, Port,
    RateFrom, RunContext, TagKind, Timed,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "picture_burst";
/// The lowest input rate.
pub const MIN_RATE: f64 = 400_000.0;
/// What the stack says about its source.
pub const NOTE: &str = crate::slotted_voice::stack::NOTE;
/// Seconds kept before a sweep in a capture.
pub const BEFORE_S: f64 = 0.3;
/// Seconds captured after a sweep start.
pub const AFTER_S: f64 = 10.45;

sdrkit_core::choice_enum! {
    /// The profile of the transmitter.
    #[derive(Default)]
    pub enum TxProfile {
        /// Balanced.
        #[default]
        Balanced = ("0", "0: Balanced"),
        /// Robust.
        Robust = ("1", "1: Robust"),
        /// Quality.
        Quality = ("2", "2: Quality"),
        /// Direct.
        Direct = ("3", "3: Direct"),
    }
}

const TX: &str = "Transmit";

/// The station callsign.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Station callsign", "", "")
    .section(TX)
    .help("Up to 9 characters of A-Z, 0-9, /, - and . Empty sends the demonstration callsign TEST1.");
/// The transmit profile.
pub const PROFILE: Setting<field::Choice<TxProfile>> = Setting::choice("profile", "Profile", TxProfile::Balanced).section(TX);
/// The picture that `send` transmits.
pub const PICTURE: Setting<field::Text> = Setting::new("picture", "Picture", "", "")
    .section(TX)
    .help("A binary PPM (P6, maxval 255) picture. Empty sends a test picture. A picture above 1920 x 1080 is shrunk.");

sdrkit_core::settings! {
    /// The transmitter settings.
    pub enum TxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Profile.
        Profile = PROFILE,
        /// Picture.
        Picture = PICTURE,
    }
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Schema::of(&[TxSetting::SETTINGS].concat()))).clone()
}

/// The `picture_burst.rx` decoder.
pub struct PbDecoder {
    rate: f64,
    rs: Option<Resampler<Complex32>>,
    buf: Vec<Complex32>,
    /// Samples at 632 ksps; `ring[0]` is sample `base`.
    ring: Vec<Complex32>,
    base: u64,
    searched_to: u64,
    capture: Option<(u64, u64)>,
    last: Option<Decoded>,
    /// Decoded bursts not sent out yet, each with its number.
    pending: Vec<(u64, Decoded)>,
    /// Rows of the picture table not written yet: the number of the burst, the callsign, the size, the layers and the
    /// time.
    rows: Vec<(u64, String, String, i64, f64)>,
    bursts: u64,
    decoded: u64,
    now: f64,
    cleared: bool,
    started: bool,
}

impl Default for PbDecoder {
    fn default() -> Self {
        PbDecoder {
            rate: 0.0,
            rs: None,
            buf: Vec::new(),
            ring: Vec::new(),
            base: 0,
            searched_to: 0,
            capture: None,
            last: None,
            pending: Vec::new(),
            rows: Vec::new(),
            bursts: 0,
            decoded: 0,
            now: 0.0,
            cleared: false,
            started: false,
        }
    }
}

impl PbDecoder {
    fn build(&mut self) -> Result<(), ConfigError> {
        self.rs = Some(Resampler::new(self.rate, FS).map_err(|e| ConfigError::Stage(StageError::new(e)))?);
        // A burst that starts on the first sample is still found.
        self.ring = vec![Complex32::new(0.0, 0.0); (BEFORE_S * FS) as usize];
        self.base = 0;
        self.searched_to = 0;
        self.capture = None;
        Ok(())
    }

    fn end(&self) -> u64 {
        self.base + self.ring.len() as u64
    }

    /// Looks for a start sweep in the samples not searched yet.
    fn search(&mut self) {
        let end = self.end();
        if end < self.searched_to + FS as u64 {
            return;
        }
        let from = self.searched_to.saturating_sub(2 * SWEEP as u64).max(self.base);
        let seg = &self.ring[(from - self.base) as usize..];
        let c = xcorr_power(seg, up_sweep());
        self.searched_to = end;
        if c.is_empty() {
            return;
        }
        let mean = c.iter().map(|&v| f64::from(v)).sum::<f64>() / c.len() as f64;
        if let Some((p, &v)) = c.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) {
            if f64::from(v) >= 60.0 * mean {
                let at = from + p as u64;
                self.bursts += 1;
                self.capture = Some((at.saturating_sub((BEFORE_S * FS) as u64).max(self.base), at + (AFTER_S * FS) as u64));
            }
        }
    }

    fn run_capture(&mut self, to: u64) {
        let Some((a, _)) = self.capture.take() else {
            return;
        };
        let lo = (a.max(self.base) - self.base) as usize;
        let hi = ((to.min(self.end())) - self.base) as usize;
        if let Some(d) = decode(&self.ring[lo..hi]) {
            self.decoded += 1;
            self.pending.push((self.decoded, d));
        }
        self.searched_to = to;
    }
}

impl Decoder for PbDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "picture_burst.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<Picture>("pictures").rate_from(RateFrom::Fixed(0.0)),
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Published {
        Published::new(PictureBurstFields::SHAPE.fields).with_tables(&[PICTURE_BURST_PICTURES.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.build()?;
        Ok(vec![
            Format {
                rate: 0.0,
                frame_len: 0,
                media: None,
                ..input.with_kind(Kind::typed::<Picture>())
            },
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn retune(&mut self, _input: &Format) {
        let _ = self.build();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        self.buf.clear();
        if let Some(rs) = self.rs.as_mut() {
            rs.process(x, &mut self.buf);
        }
        self.ring.extend_from_slice(&self.buf);
        match self.capture {
            Some((_, to)) if self.end() >= to => self.run_capture(to),
            Some(_) => {}
            None => self.search(),
        }
        // Keep 16 s.
        let cap = (16.0 * FS) as usize;
        if self.ring.len() > cap {
            let keep_from = self.capture.map_or(u64::MAX, |c| c.0);
            let drop = (self.ring.len() - cap).min(keep_from.saturating_sub(self.base) as usize);
            self.ring.drain(..drop);
            self.base += drop as u64;
        }
        self.emit(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if self.capture.is_some() {
            let e = self.end();
            self.run_capture(e);
        }
        self.emit(out);
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&PICTURE_BURST_PICTURES);
        }
        let receiving = self.capture.is_some();
        let last = self.last.as_ref();
        p.put(&PictureBurstFields {
            header_ok: last.map(|_| true),
            profile: last.map(|d| i64::from(d.header.profile)),
            layers: last.map(|d| d.layers_complete as i64),
            layers_sent: last.map(|d| d.header.layers.len() as i64),
            snr_db: last.map(|d| d.snr_db),
            offset_hz: last.map(|d| Hertz(d.cfo_hz)),
            clock_ppm: last.and_then(|d| d.clock_ppm),
            callsign: last.map(|d| d.header.callsign.as_str()),
            bursts: self.bursts as i64,
            bursts_decoded: self.decoded as i64,
            locked: receiving || last.is_some(),
            note: NOTE,
        });
        for (n, callsign, size, layers, time) in std::mem::take(&mut self.rows) {
            PICTURE_BURST_PICTURES.put(
                p,
                &n.to_string(),
                &PictureBurstPictureRow { callsign: &callsign, size: &size, layers, time: Time(time) },
            );
        }
        let (lock, summary) = if receiving {
            (LockState::Tracking, "Receiving a burst".to_string())
        } else if let Some(d) = &self.last {
            (
                LockState::Locked,
                format!("Decoded {} x {} from {}, {} of {} layers", d.header.width, d.header.height, d.header.callsign, d.layers_complete, d.header.layers.len()),
            )
        } else if self.started {
            (LockState::Searching, "Listening for a burst".to_string())
        } else {
            (LockState::Off, String::new())
        };
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            s.lock_state = lock;
            s.summary.clone_from(&summary);
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.5
    }
    fn clear(&mut self) {
        self.last = None;
        self.bursts = 0;
        self.decoded = 0;
        self.rows.clear();
        self.cleared = true;
    }
    fn forget(&mut self, kept: &Kept) {
        if self.rate > 0.0 {
            let _ = self.build();
        }
        self.pending.clear();
        self.last = None;
        self.bursts = 0;
        if !kept.field("bursts_decoded") {
            self.decoded = 0;
        }
        if !kept.table(PICTURE_BURST_PICTURES.key) {
            self.rows.clear();
            self.cleared = true;
        }
    }
}

impl PbDecoder {
    fn emit(&mut self, out: &mut Outbox) {
        for (n, d) in std::mem::take(&mut self.pending) {
            let size = format!("{} x {}", d.header.width, d.header.height);
            out.messages(1).push(PICTURE_BURST_PICTURE.event(
                self.now,
                0.0,
                &PictureBurstEvent { callsign: &d.header.callsign, size: &size, layers: d.layers_complete as i64 },
            ));
            self.rows.push((n, d.header.callsign.clone(), size, d.layers_complete as i64, self.now));
            if let Some(rgb) = &d.picture {
                out.items::<Picture>(0).push(Timed {
                    time: self.now,
                    value: Picture {
                        codec: Codec::Pixels(PixelFormat::Rgb8),
                        layout: Some(PixelLayout::packed(PixelFormat::Rgb8, d.header.width, d.header.height)),
                        name: Some(d.header.callsign.clone()),
                        data: Arc::from(&rgb[..]),
                    },
                });
            }
            self.last = Some(d);
        }
    }
}

/// Reads a binary PPM (P6, maxval 255): width, height and RGB bytes.
pub fn read_ppm(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 && i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) == Some(&b'#') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(std::str::from_utf8(&bytes[s..i]).ok()?.to_string());
    }
    if fields.len() < 4 || fields[0] != "P6" || fields[3] != "255" {
        return None;
    }
    let (w, h): (u32, u32) = (fields[1].parse().ok()?, fields[2].parse().ok()?);
    let data = bytes.get(i + 1..i + 1 + (w * h * 3) as usize)?.to_vec();
    Some((w, h, data))
}

/// Shrinks a picture above 1920 x 1080 to fit, by the rounded mean of each box.
pub fn fit(w: u32, h: u32, rgb: &[u8]) -> (u32, u32, Vec<u8>) {
    if w <= 1920 && h <= 1080 {
        return (w, h, rgb.to_vec());
    }
    let s = (1920.0 / f64::from(w)).min(1080.0 / f64::from(h));
    let (nw, nh) = (((f64::from(w) * s) as u32).max(1), ((f64::from(h) * s) as u32).max(1));
    let mut out = Vec::with_capacity((nw * nh * 3) as usize);
    for y in 0..nh {
        let y0 = (y * h / nh) as usize;
        let y1 = (((y + 1) * h / nh) as usize).max(y0 + 1);
        for x in 0..nw {
            let x0 = (x * w / nw) as usize;
            let x1 = (((x + 1) * w / nw) as usize).max(x0 + 1);
            for c in 0..3 {
                let mut sum = 0usize;
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        sum += usize::from(rgb[(yy * w as usize + xx) * 3 + c]);
                    }
                }
                let n = (y1 - y0) * (x1 - x0);
                out.push(((sum + n / 2) / n) as u8);
            }
        }
    }
    (nw, nh, out)
}

/// Why `send` was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// A transmission is under way.
    Busy,
    /// The output rate is below 400 ksps.
    Rate,
    /// The picture cannot be read.
    Picture,
    /// The burst cannot be made.
    Make(String),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Busy => f.write_str("a transmission is under way"),
            TxError::Rate => f.write_str("the transmit rate must be at least 400 ksps"),
            TxError::Picture => f.write_str("the picture is not a binary PPM file"),
            TxError::Make(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for TxError {}

/// Starts a transmission.
pub const SEND: Command = Command::new("send", "Send")
    .help("Sends the picture (or the test picture) once in a burst of 10 s. Transmit must be on.")
    .transmits(sdrkit_core::TxContent::File)
    .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop").help("Ends the transmission at once.");

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Send.
        Send = SEND,
        /// Stop.
        Stop = STOP,
    }
}

/// The burst of the transmit settings at `rate`: peak of I or Q 0.9.
pub fn tx_burst(call: &str, profile: usize, picture: &str, rate: f64) -> Result<Vec<Complex32>, TxError> {
    if rate < MIN_RATE {
        return Err(TxError::Rate);
    }
    let (w, h, rgb) = if picture.is_empty() {
        (1920, 1080, test_picture(1920, 1080))
    } else {
        let b = std::fs::read(picture).map_err(|_| TxError::Picture)?;
        let (w, h, rgb) = read_ppm(&b).ok_or(TxError::Picture)?;
        fit(w, h, &rgb)
    };
    let call = if call.trim().is_empty() { DEMO_CALLSIGN } else { call };
    let m = make(&rgb, w, h, profile, call).map_err(|e| TxError::Make(e.to_string()))?;
    let peak = m.samples.iter().map(|v| v.re.abs().max(v.im.abs())).fold(0f32, f32::max).max(1e-12);
    let x: Vec<Complex32> = m.samples.iter().map(|v| v * (0.9 / peak)).collect();
    if (rate - FS).abs() < 1e-6 {
        return Ok(x);
    }
    SincConverter::convert(&x, FS, rate).map_err(|_| TxError::Rate)
}

/// The sender (`picture_burst.tx`).
#[derive(Default)]
pub struct PbSender {
    callsign: String,
    profile: TxProfile,
    picture: String,
    rate: f64,
    transmit: bool,
    pending: Vec<Complex32>,
    at: usize,
    events: Vec<sdrkit_core::Event>,
}

impl PbSender {
    fn event(&mut self, state: &str, reason: &str) {
        self.events.push(PICTURE_BURST_TX.event(0.0, 0.0, &TxEvent { state, reason }));
    }

    /// Starts a transmission.
    pub fn send(&mut self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.at < self.pending.len() {
            return Err(TxError::Busy);
        }
        self.pending = tx_burst(&self.callsign, usize::from(self.profile.index()), &self.picture, self.rate)?;
        self.at = 0;
        Ok(())
    }
}

impl Component for PbSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "picture_burst.tx",
            vec![Port::iq("in")],
            vec![Port::iq("out"), Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let dial = if f.tuned_hz > 0.0 { f.tuned_hz } else { f.centre_hz };
        Ok(vec![Format::iq(self.rate, dial, dial), Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        if !ctx.transmit && self.at < self.pending.len() {
            self.pending.clear();
            self.at = 0;
            self.event("finished", "the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if self.at < self.pending.len() {
            let n = room.min(self.pending.len() - self.at);
            let chunk = self.pending[self.at..self.at + n].to_vec();
            io.iq_out(0).extend(chunk);
            self.at += n;
            if self.at >= self.pending.len() {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                self.pending.clear();
                self.at = 0;
                self.event("finished", "");
            }
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => self.callsign = CALLSIGN.read(id, v)?.to_owned(),
            TxSetting::Profile => self.profile = PROFILE.read(id, v)?,
            TxSetting::Picture => self.picture = PICTURE.read(id, v)?.to_owned(),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (TxSetting::Callsign, Value::Text(self.callsign.clone())),
            (TxSetting::Profile, Value::Choice(self.profile.index())),
            (TxSetting::Picture, Value::Text(self.picture.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Send => match self.send() {
                Ok(()) => self.event("started", ""),
                Err(e) => {
                    self.event("refused", &e.to_string());
                    return Err(CommandError::Failed {
                        source: StageError::new(e),
                    });
                }
            },
            TxCommand::Stop => {
                if self.at < self.pending.len() {
                    self.pending.clear();
                    self.at = 0;
                    self.event("finished", "stopped");
                }
            }
        }
        Ok(None)
    }
}

/// The test signal: a burst of a 960 x 540 test picture, profile 0, the station TEST1, 0.1 s after the start and then every
/// 26 s (16 s of silence after each burst), at unit power.
pub struct PbGenerator {
    rate: f64,
    burst: Vec<Complex32>,
    period: usize,
    at: usize,
}

impl Generator for PbGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(190_000.0).with_symbol_rate(158_000.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let lead = (0.1 * self.rate) as usize;
        for o in out.iter_mut() {
            let v = self
                .at
                .checked_sub(lead)
                .and_then(|i| self.burst.get(i).copied())
                .unwrap_or_default();
            self.at = (self.at + 1) % self.period;
            *o = v;
        }
        out.len()
    }
}

/// The generator of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let m = make(&test_picture(960, 540), 960, 540, 0, DEMO_CALLSIGN).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    let burst = if (cfg.rate_hz - FS).abs() < 1e-6 {
        m.samples
    } else {
        SincConverter::convert(&m.samples, FS, cfg.rate_hz).map_err(|e| ConfigError::Stage(StageError::new(e)))?
    };
    let period = burst.len() + (16.1 * cfg.rate_hz) as usize;
    Ok(Box::new(PbGenerator {
        rate: cfg.rate_hz,
        burst,
        period,
        at: 0,
    }))
}

/// The receiver and transmitter graph.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(cfg: &StackConfig) -> Result<sdrkit_graph::StackGraph, sdrkit_graph::GraphError> {
    use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, StackGraph};
    let b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("rx", DecoderComponent::new(PbDecoder::default()))
        .add("sender", PbSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("rx")
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .output("rx.pictures")
        .output("rx.events")
        .output("sender.events")
        .status_from("rx")
        .clock(0.5);
    let mut g = StackGraph::of(crate::typed::shape(ID), b.build()?)
        .min_rate(MIN_RATE, "picture_burst needs a sample rate of at least 400 ksps");
    for s in TxSetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("sender.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("send", CommandTarget::of("sender", "send"))
        .command("stop", CommandTarget::of("sender", "stop"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    Ok(Box::new(graph(cfg)?))
}

/// The stack.
pub static PICTURE_BURST: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Picture in a burst",
    group: crate::GROUP,
    blurb: "One picture up to 1920 x 1080 in a burst of 10 s at 158 ksymbols a second, in up to 8 layers from robust to \
            fast; the stack shows the picture that the good layers give. Receive and transmit.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6000e6,
        ..Tuning::anywhere(433.0e6, 1.0e6, MIN_RATE, 200e3)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::PICTURE_BURST_VERIFY),
    benchmarks: None,
};

//! HF packet (`hfpacket`): AX.25 frames in 300 Bd AFSK (1600 and 1800 Hz) on a sideband.
//! Standard: AX.25 Link Access Protocol for Amateur Packet Radio v2.2 (flag, bit stuffing, NRZI, FCS).
//!
//! ```text
//! iq -> [guard] -> [front, 12 kHz, dial at 0 Hz] -> [hfpacket: tone correlators, tuning, bit clock, HDLC, AX.25] -> events
//! ```

use crate::common::{self, Placer, Report, Stage, MIN_RATE_HZ, RATE};
use crate::typed::{
    HfpacketFields, HfpacketFrameEvent, HfpacketFrameRow, HfpacketStationEvent, HFPACKET_FRAME,
    HFPACKET_FRAMES, HFPACKET_STATION,
};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, Record, Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Kept, Kind, Port, RunContext, StackGraph,
};
use sdrkit_proto_link::ax25::{Address, Frame};
use sdrkit_proto_link::hdlc::{Line, Transmitter};
use sdrkit_proto_link::modem::{AfskHeard, AfskReceiver};
use std::collections::BTreeSet;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "hfpacket";
/// The name in a status line.
pub const NAME: &str = "HF packet";
/// The symbol rate.
pub const BAUD: f64 = 300.0;
/// The tone of line level 1.
pub const MARK_HZ: f64 = 1600.0;
/// The tone of line level 0.
pub const SPACE_HZ: f64 = 1800.0;
/// The most rows of the frame table.
pub const FRAME_CAP: usize = 200;
/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The results of the detector component: the shared fields, its own fields and the frame list.
fn detector_declaration() -> ResultsDecl {
    common::declaration(HfpacketFields::SHAPE.fields).with_table(HFPACKET_FRAMES.def())
}

sdrkit_core::choice_enum! {
    /// The sideband that carries the tones.
    pub enum Sideband {
        /// The tones are above the dial.
        Usb = ("usb", "Upper sideband"),
        /// The tones are below the dial.
        Lsb = ("lsb", "Lower sideband"),
    }
}

/// The sideband.
pub const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("sideband", "Sideband", Sideband::Usb).help(
        "Where the dial sits: below the tones (upper sideband) or above them (lower sideband).",
    );
/// The frames of the test signal.
pub const GEN_FRAMES: Setting<field::I64> =
    Setting::new("gen_frames", "Test signal: frames", "", 4)
        .range(1.0, 64.0, 1.0)
        .section("Test signal")
        .advanced();
/// The flags before the first frame of the test signal, as a time.
pub const GEN_LEAD_S: Setting<field::Duration> =
    Setting::new("gen_lead_s", "Test signal: lead", "s", 0.3)
        .range(0.0, 5.0, 0.0)
        .section("Test signal")
        .advanced();
/// The silence after the last frame of the test signal.
pub const GEN_TAIL_S: Setting<field::Duration> =
    Setting::new("gen_tail_s", "Test signal: tail", "s", 0.3)
        .range(0.0, 5.0, 0.0)
        .section("Test signal")
        .advanced();
/// The tone of line level 1 of the test signal.
pub const GEN_MARK_HZ: Setting<field::Hz> =
    Setting::new("gen_mark_hz", "Test signal: mark", "Hz", MARK_HZ)
        .range(300.0, 3000.0, 0.0)
        .section("Test signal")
        .advanced();
/// The tone of line level 0 of the test signal.
pub const GEN_SPACE_HZ: Setting<field::Hz> =
    Setting::new("gen_space_hz", "Test signal: space", "Hz", SPACE_HZ)
        .range(300.0, 3000.0, 0.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the stack.
    pub enum PacketSetting {
        /// The sideband.
        Sideband = SIDEBAND,
        /// Frames of the test signal.
        GenFrames = GEN_FRAMES,
        /// Lead of the test signal.
        GenLead = GEN_LEAD_S,
        /// Tail of the test signal.
        GenTail = GEN_TAIL_S,
        /// Mark tone of the test signal.
        GenMark = GEN_MARK_HZ,
        /// Space tone of the test signal.
        GenSpace = GEN_SPACE_HZ,
    }
}

sdrkit_core::settings! {
    /// The settings of [`HfPacket`].
    pub enum DecoderSetting {
        /// The sideband.
        Sideband = SIDEBAND,
    }
}

/// The decoder (`voice.hfpacket`): the 12 kHz channel to frames, the frame table and the events.
pub struct HfPacket {
    rx: AfskReceiver,
    report: Report,
    sideband: Sideband,
    good: u64,
    bad: u64,
    /// The frames decoded since the last change of frequency.
    here: u64,
    stations: BTreeSet<String>,
    last: String,
    /// The newest frame in the monitor form.
    last_frame: Option<String>,
    buf: Vec<Complex32>,
    row: String,
}

impl Default for HfPacket {
    fn default() -> Self {
        HfPacket {
            rx: AfskReceiver::new(RATE, BAUD, MARK_HZ, SPACE_HZ)
                .expect("12 kHz holds 40 samples for a bit"),
            report: Report::new(NAME, detector_declaration()),
            sideband: Sideband::Usb,
            good: 0,
            bad: 0,
            here: 0,
            stations: BTreeSet::new(),
            last: String::new(),
            last_frame: None,
            buf: Vec::new(),
            row: String::new(),
        }
    }
}

/// The addresses of the digipeaters of `f`, comma separated.
fn path_of(f: &Frame<'_>) -> String {
    let mut s = String::new();
    for (i, a) in f.repeaters().iter().enumerate() {
        let _ = write!(s, "{}{a}", if i > 0 { "," } else { "" });
    }
    s
}

impl HfPacket {
    /// Takes the bytes of a frame with a good FCS.
    fn frame(&mut self, bytes: &[u8]) {
        let Ok(f) = Frame::parse(bytes) else {
            self.bad += 1;
            return;
        };
        self.good += 1;
        self.here += 1;
        let source = f.source().to_string();
        let destination = f.destination().to_string();
        let path = path_of(&f);
        let info = String::from_utf8_lossy(f.info).into_owned();
        let kind = f.control.name();
        let line = format!("{f}  [{kind}]");
        let offset = self.rx.tone_offset_hz();
        self.row.clear();
        let _ = write!(self.row, "{}", self.good);
        let wall = self.report.wall();
        HFPACKET_FRAMES.put(
            &mut self.report.p,
            &self.row,
            &HfpacketFrameRow {
                time_utc: wall.map(Time),
                source: &source,
                destination: &destination,
                path: &path,
                r#type: kind,
                pid: f.pid.map(i64::from),
                info: &info,
                tone_offset_hz: Hertz(offset),
            },
        );
        let t = self.report.note(&line);
        let e = HFPACKET_FRAME.event(
            t,
            0.0,
            &HfpacketFrameEvent {
                text: &line,
                source: &source,
                destination: &destination,
                path: &path,
                r#type: kind,
                info: &info,
                tone_offset_hz: Hertz(offset),
            },
        );
        self.report.send(e);
        if self.stations.insert(source.clone()) {
            let text = format!("station {source}");
            let t = self.report.note(&text);
            let e = HFPACKET_STATION.event(
                t,
                0.0,
                &HfpacketStationEvent {
                    text: &text,
                    source: &source,
                },
            );
            self.report.send(e);
        }
        self.last = format!("{source}>{destination}");
        self.last_frame = Some(line);
    }

    fn publish(&mut self) {
        let (stage, text) = if self.here > 0 {
            (
                Stage::Confirmed,
                format!("{} frame(s), last {}", self.good, self.last),
            )
        } else if self.rx.signal_blocks() > 3 {
            (Stage::Signal, "AFSK tones found, no frame yet".to_string())
        } else {
            (Stage::Searching, "searching".to_string())
        };
        self.report.p.put(&HfpacketFields {
            tone_offset_hz: Hertz(self.rx.tone_offset_hz()),
            stations: self.stations.len() as i64,
            last_frame: self.last_frame.as_deref(),
        });
        self.report
            .publish(stage, &text, self.rx.quality_db(), self.good, self.bad);
    }

    fn clear(&mut self) {
        self.good = 0;
        self.bad = 0;
        self.here = 0;
        self.stations.clear();
        self.last.clear();
        self.report.clear();
        self.report.p.clear_rows(&HFPACKET_FRAMES);
        self.last_frame = None;
        self.rx.reset();
    }

    /// Forgets the signal of the frequency before: the tuning, the quality, the level and the state. The frame
    /// table, the counts, the stations and the last frame stay where `kept` holds them.
    fn forget_signal(&mut self, kept: &Kept) {
        self.rx.reset();
        self.here = 0;
        self.report.forget(kept);
        if !kept.table(HFPACKET_FRAMES.key) {
            self.report.p.clear_rows(&HFPACKET_FRAMES);
        }
        if !kept.field("good") {
            self.good = 0;
        }
        if !kept.field("bad") {
            self.bad = 0;
        }
        if !kept.field("stations") {
            self.stations.clear();
        }
        if !kept.field("last_frame") {
            self.last.clear();
            self.last_frame = None;
        }
    }
}

impl Component for HfPacket {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "voice.hfpacket",
            vec![Port::iq("in"), Port::iq("raw")],
            vec![Port::new("events", Kind::Messages)],
        )
        .with_settings(DecoderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if (inputs[0].rate - RATE).abs() > 1e-6 {
            return Err(ConfigError::WrongRate {
                need: RATE,
                got: inputs[0].rate,
            });
        }
        self.rx.reset();
        self.report.configure(inputs[1].rate);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.report.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut buf = std::mem::take(&mut self.buf);
        buf.clear();
        buf.extend_from_slice(x);
        if self.sideband == Sideband::Lsb {
            buf.iter_mut().for_each(|s| *s = s.conj());
        }
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let mut bad = 0;
        self.rx.process(&buf, |h| match h {
            AfskHeard::Frame(b) => frames.push(b.to_vec()),
            AfskHeard::Bad => bad += 1,
        });
        self.report.take(&buf);
        self.buf = buf;
        self.bad += bad;
        for f in &frames {
            self.frame(f);
        }
        self.report.drain(io.messages_out(0));
        if io.tick() || !frames.is_empty() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::Sideband => self.sideband = SIDEBAND.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            DecoderSetting::Sideband,
            Value::Choice(self.sideband.index()),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.report.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.clear();
                self.publish();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        self.forget_signal(kept);
        self.publish();
    }
}

/// The graph of the stack.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    common::add_front(GraphBuilder::new())
        .add("decoder", HfPacket::default())
        .connect("front", "decoder.in")
        .input("decoder.raw")
        .output("decoder")
        .clock(0.25)
        .status_from("decoder")
        .catalog(catalog)
        .build()
}

/// The stack graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "HF packet: the sample rate is too low (250 kHz at least)",
        )
        .route(SIDEBAND.key(), &["decoder.sideband"])?
        .command("clear", sdrkit_graph::CommandTarget::of("decoder", "clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("decoder", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The frames of the test signal: UI frames from `TEST-1` to `BEACON`, each with its number in the text.
pub fn test_frames(n: usize) -> Vec<Vec<u8>> {
    let source = Address::parse("TEST-1").expect("a valid address");
    let destination = Address::parse("BEACON").expect("a valid address");
    (1..=n)
        .map(|i| {
            let text = format!("sdrkit HF packet test frame {i}");
            let mut bytes = Vec::new();
            Frame::ui(destination, source, &[], text.as_bytes())
                .and_then(|f| f.encode(&mut bytes))
                .expect("the frame fits AX.25");
            bytes
        })
        .collect()
}

/// The line levels of a burst of `frames`: the first frame after flags for `lead_s` plus 2, each later frame after 3 flags,
/// 6 flags at the end.
pub fn burst_levels(frames: &[Vec<u8>], lead_s: f64) -> Vec<u8> {
    let mut tx = Transmitter::new(Line::Nrzi);
    let mut levels = Vec::new();
    let lead = (lead_s * BAUD / 8.0).ceil() as usize + 2;
    for (i, f) in frames.iter().enumerate() {
        // The closing flag of a frame is the first of the 3 flags before the next one.
        let flags = if i == 0 { lead } else { 2 };
        tx.encode(&[f.as_slice()], flags, &mut levels);
    }
    tx.flags(5, &mut levels);
    levels
}

/// The test signal: a burst of UI frames as complex tones above the dial, then silence, again and again. [`PacketGenerator::new`]
/// puts the dial at 0 Hz, [`PacketGenerator::from_config`] 20 kHz below the centre, where the radio of the stack sees it.
pub struct PacketGenerator {
    rate: f64,
    levels: Vec<u8>,
    tones: [f64; 2],
    tail: u64,
    n: u64,
    phase: f64,
    placer: Placer,
}

/// The amplitude of the test signal.
pub const GEN_AMPLITUDE: f32 = 0.5;

impl PacketGenerator {
    /// A generator at `rate` of `frames` frames with level 1 at `mark_hz` and level 0 at `space_hz`.
    pub fn new(
        rate: f64,
        frames: usize,
        lead_s: f64,
        tail_s: f64,
        mark_hz: f64,
        space_hz: f64,
    ) -> Self {
        PacketGenerator {
            rate,
            levels: burst_levels(&test_frames(frames), lead_s),
            tones: [space_hz, mark_hz],
            tail: (tail_s * rate).round() as u64,
            n: 0,
            phase: 0.0,
            placer: Placer::new(rate, 0.0),
        }
    }
    /// The generator of the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut g = PacketGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_FRAMES) as usize,
            s.get(&GEN_LEAD_S),
            s.get(&GEN_TAIL_S),
            s.get(&GEN_MARK_HZ),
            s.get(&GEN_SPACE_HZ),
        );
        g.placer = Placer::below_centre(cfg.rate_hz);
        g
    }
    /// The samples of one burst with its tail.
    pub fn period(&self) -> u64 {
        (self.levels.len() as f64 * self.rate / BAUD).round() as u64 + self.tail
    }
}

impl Generator for PacketGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(3000.0).with_power(f64::from(GEN_AMPLITUDE * GEN_AMPLITUDE)))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let per_bit = self.rate / BAUD;
        let period = self.period();
        for o in out.iter_mut() {
            let k = self.n % period;
            self.n += 1;
            // Bit `i` ends at sample round((i + 1) * rate / 300).
            let bit = ((k as f64 + 0.5) / per_bit) as usize;
            *o = match self.levels.get(bit) {
                Some(&l) => {
                    self.phase = (self.phase + self.tones[usize::from(l)] / self.rate).fract();
                    let (s, c) = (TAU * self.phase).sin_cos();
                    Complex32::new(c as f32, s as f32) * GEN_AMPLITUDE
                }
                None => Complex32::default(),
            };
        }
        self.placer.apply(out);
        out.len()
    }
}

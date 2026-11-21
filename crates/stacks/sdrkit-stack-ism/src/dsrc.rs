//! DSRC road toll (`dsrc_toll`): the 5.8 GHz downlink of roadside beacons, counted by frame class. Structure only: the
//! envelope of the amplitude-keyed carrier gives FM0 bits, which give HDLC frames with a good FCS; each frame is counted as a
//! beacon broadcast or a private link frame, and its length and control byte are logged. No link identifier and no content is
//! shown, logged or kept. The uplink (the backscatter of the on-board unit) is not read.
//! Standard: ISO/IEC 13239 (HDLC framing and FCS); the physical values follow the description of CEN EN 12253, which was not
//! read.

use crate::family::{min_rate_message, set_status, REPORT_S};
use crate::typed::{DsrcFields, DsrcFrameEvent, DsrcFrameRow, DSRC_FRAME, DSRC_FRAMES};
use sdrkit_algo_modem::fm0::{Fm0Edges, Fm0Out};
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::interface::{Fields, ReadFields};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, EventDef, GenConfig, Generator, LockProfile, LockState,
    Receiver, Record, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept,
    Kind, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::hdlc::{hdlc_encode, HdlcDeframer};
use sdrkit_util_bits::line::{Biphase, BiphaseKind};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dsrc_toll";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 2e6;
/// The default frequency of the test signal, Hz.
pub const TEST_DIAL_HZ: f64 = 5802.5e6;
/// The most frames kept in the log.
pub const FRAME_CAP: usize = 300;
/// A frame within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 5.0;
/// Bytes of a frame, without the FCS.
pub const MIN_BYTES: usize = 4;
/// The most bytes of a frame.
pub const MAX_BYTES: usize = 300;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: every physical-layer value (500 kbit/s, amplitude keying, FM0), since EN 12253 and EN 12795 were not read; the broadcast rule (a frame that starts with FF FF FF FF), which conflicts with the link-identifier rule (the first FF, whose bit 0 is set, ends the identifier).";

/// The bit rate.
pub const BIT_RATE: Setting<field::Hz> = Setting::new("bit_rate", "Bit rate", "Hz", 500e3)
    .range(100e3, 1e6, 1e3)
    .help("The downlink bit rate. 500 kbit/s is the usual value; no public document was read to confirm it.");

sdrkit_core::settings! {
    /// The settings of `ism.dsrc_toll` and of the `dsrc_toll` stack.
    pub enum DsrcSetting {
        /// The bit rate.
        BitRate = BIT_RATE,
    }
}

/// The class of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameKind {
    /// A frame that starts with FF FF FF FF.
    Broadcast,
    /// Any other frame.
    Private,
}

impl FrameKind {
    /// The text shown.
    pub fn text(self) -> &'static str {
        match self {
            FrameKind::Broadcast => "Beacon broadcast",
            FrameKind::Private => "Private link",
        }
    }
}

/// The class and control byte of a frame (without its FCS). A frame that starts with FF FF FF FF is a broadcast and its
/// control byte follows those four bytes; in another frame the link identifier runs to the first byte whose bit 0 is set and
/// the control byte follows it.
pub fn classify(frame: &[u8]) -> (FrameKind, Option<u8>) {
    if frame.len() >= 4 && frame[..4] == [0xFF; 4] {
        return (FrameKind::Broadcast, frame.get(4).copied());
    }
    let end = frame.iter().position(|b| b & 1 == 1);
    (
        FrameKind::Private,
        end.and_then(|e| frame.get(e + 1).copied()),
    )
}

/// One line of the log: no identifier and no content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DsrcFrame {
    /// Signal time, s.
    pub t: f64,
    /// The class.
    pub kind: FrameKind,
    /// Bytes without the FCS.
    pub bytes: usize,
    /// The control byte.
    pub control: Option<u8>,
}

impl Shift for DsrcFrame {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Frames with a good FCS.
    pub frames: u64,
    /// Broadcast frames.
    pub broadcast: u64,
    /// Private link frames.
    pub private: u64,
    /// Frames with a bad FCS.
    pub bad: u64,
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct DsrcHeld {
    /// The log.
    pub frames: LogRing<DsrcFrame>,
    /// The counters.
    pub counters: Counters,
}

impl Default for DsrcHeld {
    fn default() -> Self {
        DsrcHeld {
            frames: LogRing::new(FRAME_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for DsrcHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
    }
}

impl Tables for DsrcHeld {
    const FORMAT: StateFormat = StateFormat::new("ism.dsrc_toll", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.counters = saved.counters;
    }
}

impl Shaped for DsrcFrame {
    type Row = DsrcFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let control = self.control.map(|c| format!("{c:02X}"));
        out.put(&DsrcFrameRow {
            time_s: self.t,
            kind: self.kind.text(),
            bytes: self.bytes as i64,
            control: control.as_deref(),
            identifier: "link identifier not shown",
        });
    }
}

impl Item for DsrcFrame {
    const KIND: &'static str = "dsrc_toll.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(DSRC_FRAME.def())
    }
    fn write_fields(&self, f: &mut Record) {
        DsrcFrameEvent {
            kind: self.kind.text(),
            bytes: self.bytes as i64,
            family: Family::Other,
        }
        .write(f);
    }
}

/// The `ism.dsrc_toll` decoder: envelope, FM0 from the level changes, HDLC.
pub struct DsrcDecoder {
    bit_rate: f64,
    rate: f64,
    mean: f32,
    level: bool,
    since: f64,
    fm0: Fm0Edges,
    hdlc: HdlcDeframer,
    held: DsrcHeld,
    out: Vec<Timed<DsrcFrame>>,
    level_acc: f64,
    level_n: u64,
    level_dbfs: Option<f64>,
    n: u64,
    now: f64,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for DsrcDecoder {
    fn default() -> Self {
        DsrcDecoder {
            bit_rate: 500e3,
            rate: 0.0,
            mean: 0.0,
            level: false,
            since: 0.0,
            fm0: Fm0Edges::new(2.0),
            hdlc: HdlcDeframer::x25(MIN_BYTES, MAX_BYTES),
            held: DsrcHeld::default(),
            out: Vec::new(),
            level_acc: 0.0,
            level_n: 0,
            level_dbfs: None,
            n: 0,
            now: 0.0,
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

impl DsrcDecoder {
    /// The log and the counters.
    pub fn held(&self) -> &DsrcHeld {
        &self.held
    }

    fn bit(&mut self, b: u8) {
        let Some(end) = self.hdlc.push(b) else {
            return;
        };
        if !end.fcs_ok {
            self.held.counters.bad += 1;
            self.dirty = true;
            return;
        }
        let (kind, control) = classify(self.hdlc.frame());
        let f = DsrcFrame {
            t: self.n as f64 / self.rate,
            kind,
            bytes: end.len,
            control,
        };
        let c = &mut self.held.counters;
        c.frames += 1;
        match kind {
            FrameKind::Broadcast => c.broadcast += 1,
            FrameKind::Private => c.private += 1,
        }
        self.held.frames.push(f.clone());
        self.out.push(Timed {
            time: f.t,
            value: f,
        });
        self.dirty = true;
    }
}

impl Decoder for DsrcDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ism.dsrc_toll"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<DsrcFrame>("frames")]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        DsrcSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(DsrcSetting::BitRate, Value::Hz(self.bit_rate))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match DsrcSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DsrcSetting::BitRate => self.bit_rate = BIT_RATE.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.fm0 = Fm0Edges::new(self.rate / self.bit_rate);
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.fm0 = Fm0Edges::new(input.rate / self.bit_rate);
        self.hdlc.reset();
        Ok(vec![Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(Kind::typed::<DsrcFrame>())
        }])
    }
    fn retune(&mut self, _input: &Format) {
        self.fm0.reset();
        self.hdlc.reset();
    }
    fn attach(&mut self, _ctx: &RunContext) {}
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        for z in x {
            let p = z.norm_sqr();
            self.level_acc += f64::from(p);
            self.level_n += 1;
            self.n += 1;
            let e = p.sqrt();
            let k = if e > 2.0 * self.mean { 0.05 } else { 0.002 };
            self.mean += k * (e - self.mean);
            let level = e > self.mean;
            self.since += 1.0;
            if level != self.level {
                self.level = level;
                let samples = std::mem::replace(&mut self.since, 0.0);
                match self.fm0.edge(samples) {
                    Fm0Out::Bit(b) => self.bit(b),
                    Fm0Out::Lost => self.hdlc.reset(),
                    Fm0Out::Nothing => {}
                }
            }
        }
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        out.items::<DsrcFrame>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        if self.level_n > 0 {
            self.level_dbfs =
                Some(10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        let c = self.held.counters;
        p.put(&DsrcFields {
            frames_ok: c.frames as i64,
            broadcast_frames: c.broadcast as i64,
            private_frames: c.private as i64,
            frames_bad: c.bad as i64,
            level_dbfs: self.level_dbfs,
            note: NOT_CONFIRMED,
        });
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&DSRC_FRAMES);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.frames, &DSRC_FRAMES, self.now, p);
        }
        let newest = self.held.frames.newest_first().next();
        let recent = newest.is_some_and(|f| self.now - f.t <= RECENT_S);
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let summary = if c.frames > 0 {
            format!(
                "DSRC structure only: {} frames ({} broadcast, {} private link), {} bad",
                c.frames, c.broadcast, c.private, c.bad
            )
        } else if self.started {
            "DSRC structure only: listening for beacon frames".to_string()
        } else {
            "DSRC: waiting for samples".to_string()
        };
        set_status(p, lock, &summary);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &Kept) {
        self.level_dbfs = None;
        self.level_acc = 0.0;
        self.level_n = 0;
        if !kept.table(DSRC_FRAMES.key) {
            self.held.frames.clear();
            self.cleared = true;
        }
        let c = &mut self.held.counters;
        if !kept.field("frames_ok") {
            c.frames = 0;
        }
        if !kept.field("broadcast_frames") {
            c.broadcast = 0;
        }
        if !kept.field("private_frames") {
            c.private = 0;
        }
        if !kept.field("frames_bad") {
            c.bad = 0;
        }
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The `dsrc_toll` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(DsrcDecoder::default()))
        .input("rx")
        .output("rx")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("DSRC", MIN_RATE_HZ))
        .route(BIT_RATE.key(), &["rx.bit_rate"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The broadcast frame of the test signal (the worked frame, without its FCS).
pub const TEST_BROADCAST: [u8; 13] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0x03, 0x50, 0x12, 0x34, 0x56, 0xF8, 0xF8, 0xF8, 0x01,
];
/// The private link frame of the test signal (link identifier 2A 5B, control 77).
pub const TEST_PRIVATE: [u8; 9] = [0x2A, 0x5B, 0x77, 0x01, 0x02, 0x7E, 0x7E, 0xFF, 0x10];

/// The envelope of one burst: 64 run-in ones, the two test frames with flags, FM0 at `bit_rate`, `depth` of keying, at `rate`.
pub fn burst(rate: f64, bit_rate: f64, depth: f32) -> Vec<Complex32> {
    let mut bits = vec![1u8; 64];
    hdlc_encode(
        &[&TEST_BROADCAST, &TEST_PRIVATE],
        &CRC16_IBM_SDLC,
        2,
        &mut bits,
    );
    bits.extend([1u8; 8]);
    let mut chips = Vec::new();
    Biphase::new(BiphaseKind::Space, 0).encode(&bits, &mut chips);
    let per_chip = rate / bit_rate / 2.0;
    let n = (chips.len() as f64 * per_chip).ceil() as usize;
    let low = 1.0 - depth.clamp(0.0, 1.0);
    (0..n)
        .map(|i| {
            let c = chips[((i as f64 / per_chip) as usize).min(chips.len() - 1)];
            Complex32::new(if c == 1 { 1.0 } else { low }, 0.0)
        })
        .collect()
}

/// The keying depth of the test signal.
pub const GEN_DEPTH: Setting<field::F64> =
    Setting::new("gen_depth", "Test signal keying depth", "", 1.0)
        .range(0.3, 1.0, 0.05)
        .section("Test signal")
        .advanced();

struct DsrcGenerator {
    scene: Scene,
    rate: f64,
}

impl Generator for DsrcGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(1e6).with_bit_rate(500e3).with_power(0.5))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}

/// The test signal at `rate`: a burst of the two frames every 10 ms.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(rate: f64, depth: f32) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate)?;
    let b = burst(rate, 500e3, depth);
    let mut t = 0.002;
    scene.add_source(Box::new(move || {
        let x = Burst {
            start_s: t,
            offset_hz: 0.0,
            level_db: 0.0,
            samples: b.clone(),
        };
        t += 0.010;
        Some(x)
    }));
    Ok(scene)
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let depth = cfg.settings.get(&GEN_DEPTH) as f32;
    Ok(Box::new(DsrcGenerator {
        scene: scene(cfg.rate_hz, depth)?,
        rate: cfg.rate_hz,
    }))
}

sdrkit_core::settings! {
    /// The settings of the `dsrc_toll` stack: the bit rate and the test signal.
    pub enum DsrcStackSetting {
        /// The bit rate.
        BitRate = BIT_RATE,
        /// The keying depth of the test signal.
        GenDepth = GEN_DEPTH,
    }
}

/// DSRC road toll. The default frequency is that of the test signal; the session gives the real one.
pub static DSRC_TOLL: StackDescriptor = StackDescriptor {
    id: ID,
    name: "DSRC road toll",
    group: "Road toll, 5.8 GHz",
    blurb: "The 5.8 GHz downlink of road toll beacons, structure only: HDLC frames counted as beacon broadcasts or private link frames; no identifier or content is shown.",
    tuning: Tuning::anywhere(TEST_DIAL_HZ, 4e6, MIN_RATE_HZ, 1e6),
    submodes: &[],
    settings: DsrcStackSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal, beacon frames", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = DsrcFields::read(r.fields());
        vec![
            crate::family::at_least_value(
                "broadcast_frames",
                f.as_ref().map_or(0, |f| f.broadcast_frames),
                20,
            ),
            crate::family::at_least_value(
                "private_frames",
                f.as_ref().map_or(0, |f| f.private_frames),
                20,
            ),
        ]
    });
    vec![c]
}

/// The verify case of `dsrc_toll`.
pub static VERIFY: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the receiver: 0.5 s of the test signal at 4 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "dsrc_toll.receiver_4m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    if let Ok(mut s) = scene(rate, 1.0) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(
        &DsrcStackSetting::schema(),
    ))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ =
        rx.configure(&sdrkit_core::InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.5, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worked_fcs_and_the_classes() {
        let mut f = TEST_BROADCAST.to_vec();
        assert_eq!(CRC16_IBM_SDLC.checksum(&f), 0x87EA);
        f.extend([0xEA, 0x87]);
        assert!(CRC16_IBM_SDLC.verify(&f, sdrkit_util_bits::crc::Trailer::Little));
        assert_eq!(
            classify(&TEST_BROADCAST),
            (FrameKind::Broadcast, Some(0x03))
        );
        assert_eq!(classify(&TEST_PRIVATE), (FrameKind::Private, Some(0x77)));
    }
}

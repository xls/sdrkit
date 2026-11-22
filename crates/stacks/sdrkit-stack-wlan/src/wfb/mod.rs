//! wfb-ng and OpenHD (`wfb`): video links made of 802.11n frames that a transmitter injects without joining a network. The
//! HT-mixed PPDUs (one stream, 20 MHz, MCS 0 to 7) are decoded by [`crate::ht`] behind the non-HT receiver of
//! `sdrkit_proto_wlan_phy::ofdm`; the frames give the link id, the radio port, the FEC block and fragment, and the frames
//! lost. The payload stays encrypted: only the structure is shown.
//! Standard: IEEE Std 802.11-2020 clauses 17 and 19. The frame layouts of wfb-ng and OpenHD are not in a public document.

pub mod frames;
pub mod gen;

use crate::ht::{HtDecoder, HtResult};
use crate::report::{hex, wall};
use crate::typed::{WfbFields, WfbFrameRow, WfbLinkRow, WFB_FRAMES, WFB_LINKS};
use frames::{Kind as FrameKind, LinkId, LossCount};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_core::interface::{Fields as _, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::value::Choice as _;
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState,
    Receiver, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor,
    StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kept, Outbox,
    Port, Registry, RunContext, StackGraph,
};
use sdrkit_proto_wlan_phy::ofdm::{OfdmReceiver, SAMPLE_RATE};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "wfb";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 20e6;
/// The width the channel filter keeps flat, Hz. The HT subcarriers span 17.5 MHz; the HT-LTF channel takes in the filter's
/// edge.
const CHAIN_WIDTH_HZ: f64 = 17e6;
/// Links kept.
pub const LINK_CAP: usize = 64;
/// Frames kept in the log.
pub const FRAME_CAP: usize = 400;
/// Payload bytes shown as hex.
pub const HEX_BYTES: usize = 32;
/// A frame within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 5.0;
/// The report period, s.
pub const REPORT_S: f64 = 0.25;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the wfb-ng and OpenHD frame layouts (the WB tag and channel id, the packet types, the nonce of FEC block and fragment, the OpenHD addresses and nonce). The HT data pilots are taken as fixed or moving one place each symbol, whichever fits better. Structure only: the payload is encrypted and not opened. Not tried on a real signal.";

sdrkit_core::choice_enum! {
    /// The links shown.
    pub enum Links {
        /// wfb-ng only.
        Wfb = ("wfb-ng", "wfb-ng"),
        /// OpenHD only.
        OpenHd = ("openhd", "OpenHD"),
        /// Both.
        Both = ("both", "Both"),
    }
}

/// The links shown.
pub const LINKS: Setting<field::Choice<Links>> =
    Setting::choice("links", "Links shown", Links::Both).help("Which systems the tables show.");

sdrkit_core::settings! {
    /// The settings of `wfb`.
    pub enum WfbSetting {
        /// The links shown.
        Links = LINKS,
    }
}

/// One link of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkRow {
    /// "wfb-ng" or "OpenHD".
    pub system: String,
    /// The link id as text.
    pub id: String,
    /// Frames heard.
    pub frames: u64,
    /// The loss count (wfb-ng data).
    pub loss: LossCount,
    /// The SNR of the last frame, dB.
    pub snr_db: f64,
    /// The type of the last frame.
    pub last_kind: String,
    /// The detail of the last frame.
    pub last_detail: String,
}

impl Shift for LinkRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// One frame of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Signal time, s.
    pub t: f64,
    /// Wall time, when live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// "wfb-ng" or "OpenHD".
    pub system: String,
    /// The link id as text.
    pub id: String,
    /// The type.
    pub kind: String,
    /// The detail.
    pub detail: String,
    /// The first payload bytes.
    pub bytes: Vec<u8>,
    /// SNR, dB.
    pub snr_db: f64,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// HT frames with a good FCS.
    pub ht_frames: u64,
    /// HT-mixed PPDUs that this decoder does not take.
    pub not_decodable: u64,
    /// HT frames with a bad FCS.
    pub bad_fcs: u64,
    /// Frames of wfb-ng or OpenHD.
    pub link_frames: u64,
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct WfbHeld {
    /// The links.
    pub links: ObjectTable<String, LinkRow>,
    /// The frame log.
    pub frames: LogRing<FrameRow>,
    /// The counters.
    pub counters: Counters,
}

impl Default for WfbHeld {
    fn default() -> Self {
        WfbHeld {
            links: ObjectTable::new(LINK_CAP),
            frames: LogRing::new(FRAME_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for WfbHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.links.shift_times(by_s);
        self.frames.shift_times(by_s);
    }
}

impl Tables for WfbHeld {
    const FORMAT: StateFormat = StateFormat::new("wlan.wfb", 1);
    fn clear(&mut self) {
        self.links.clear();
        self.frames.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.links.adopt(saved.links);
        self.frames.adopt(saved.frames);
        self.counters = saved.counters;
    }
}

impl Shaped for LinkRow {
    type Row = WfbLinkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WfbLinkRow {
            link: &self.system,
            id: &self.id,
            frames: i64::try_from(self.frames).unwrap_or(i64::MAX),
            frames_lost: i64::try_from(self.loss.lost).unwrap_or(i64::MAX),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
            snr_db: self.snr_db,
            last_type: &self.last_kind,
            last_detail: &self.last_detail,
        });
    }
}

impl Shaped for FrameRow {
    type Row = WfbFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WfbFrameRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            link: &self.system,
            id: &self.id,
            r#type: &self.kind,
            detail: &self.detail,
            hex: &hex(&self.bytes),
            snr_db: self.snr_db,
        });
    }
}

/// An HT frame as the decoder hands it to the tables.
#[derive(Clone, Debug, PartialEq)]
struct Got {
    sample: u64,
    mpdu: Vec<u8>,
    mcs: u8,
    short_gi: bool,
    snr_db: f64,
}

/// The `wlan.wfb` decoder: the channel at 20 Msps, the non-HT receiver with the HT decoder behind it, and the tables.
pub struct WfbDecoder {
    links: Links,
    rate: f64,
    ch: Option<Channelizer>,
    buf: Vec<Complex32>,
    rx: Box<OfdmReceiver>,
    ht: Box<HtDecoder>,
    got: Vec<Got>,
    origin: u64,
    held: WfbHeld,
    snr_db: Option<f64>,
    now: f64,
    last_frame: f64,
    live: bool,
    clock: Option<Clock>,
    summary: String,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for WfbDecoder {
    fn default() -> Self {
        WfbDecoder {
            links: Links::Both,
            rate: 0.0,
            ch: None,
            buf: Vec::new(),
            rx: Box::new(OfdmReceiver::new()),
            ht: Box::new(HtDecoder::new()),
            got: Vec::with_capacity(8),
            origin: 0,
            held: WfbHeld::default(),
            snr_db: None,
            now: 0.0,
            last_frame: f64::NEG_INFINITY,
            live: false,
            clock: None,
            summary: String::with_capacity(160),
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

impl WfbDecoder {
    /// The tables and counters.
    pub fn held(&self) -> &WfbHeld {
        &self.held
    }

    fn build(&mut self, f: &Format) -> Result<(), ConfigError> {
        let mut cfg = ChannelConfig::new(f.rate, f.signal_offset_hz(), CHAIN_WIDTH_HZ, SAMPLE_RATE);
        cfg.exact_rate = true;
        self.ch = Some(Channelizer::new(cfg).map_err(ConfigError::from)?);
        self.rx.reset();
        Ok(())
    }

    fn stamp(&self, t: f64) -> Stamp {
        match self.clock.as_ref().and_then(|c| wall(self.live, c)) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    fn shown(&self, l: &LinkId) -> bool {
        matches!(
            (self.links, l),
            (Links::Both, _)
                | (Links::Wfb, LinkId::Wfb { .. })
                | (Links::OpenHd, LinkId::OpenHd { .. })
        )
    }

    fn frame(&mut self, g: &Got) {
        self.held.counters.ht_frames += 1;
        let Some(f) = frames::parse(&g.mpdu) else {
            return;
        };
        if !self.shown(&f.link) {
            return;
        }
        self.held.counters.link_frames += 1;
        let t = (self.origin + g.sample) as f64 / SAMPLE_RATE;
        let at = self.stamp(t);
        let gi = if g.short_gi { "short GI" } else { "long GI" };
        let payload = &g.mpdu[frames::PAYLOAD.min(g.mpdu.len())..];
        let detail = match f.kind {
            FrameKind::Data { block, fragment } => format!(
                "MCS {}, {gi}, block {block} fragment {fragment}, {} bytes",
                g.mcs,
                g.mpdu.len()
            ),
            FrameKind::Session => {
                format!("MCS {}, {gi}, session key, {} bytes", g.mcs, g.mpdu.len())
            }
            FrameKind::Frame { nonce } => {
                format!("MCS {}, {gi}, nonce {nonce}, {} bytes", g.mcs, g.mpdu.len())
            }
        };
        let system = f.link.system().to_string();
        let id = f.link.text();
        self.held.frames.push(FrameRow {
            t,
            wall: at.wall,
            system: system.clone(),
            id: id.clone(),
            kind: f.kind.text().to_string(),
            detail: detail.clone(),
            bytes: payload[..payload.len().min(HEX_BYTES)].to_vec(),
            snr_db: g.snr_db,
        });
        let key = format!("{system}/{id}");
        if let Ok(up) = self.held.links.upsert(&key, at, || LinkRow {
            system: system.clone(),
            id: id.clone(),
            frames: 0,
            loss: LossCount::default(),
            snr_db: 0.0,
            last_kind: String::new(),
            last_detail: String::new(),
        }) {
            let r = up.row;
            r.frames += 1;
            r.snr_db = g.snr_db;
            r.last_kind = f.kind.text().to_string();
            r.last_detail = detail;
            if let FrameKind::Data { block, fragment } = f.kind {
                r.loss.take(block, fragment);
            }
        }
        self.snr_db = Some(g.snr_db);
        self.last_frame = t;
        self.dirty = true;
    }

    fn summary(&self, out: &mut String) {
        use std::fmt::Write as _;
        out.clear();
        let c = &self.held.counters;
        // Writing into a `String` cannot fail.
        let _ = if !self.started {
            write!(out, "wfb-ng / OpenHD: waiting for samples")
        } else if self.held.links.is_empty() {
            write!(
                out,
                "wfb-ng / OpenHD: no link yet; {} HT frames, {} not decodable here",
                c.ht_frames, c.not_decodable
            )
        } else {
            write!(
                out,
                "wfb-ng / OpenHD: {} links, {} frames; structure only, payload encrypted",
                self.held.links.len(),
                c.link_frames
            )
        };
    }
}

impl Decoder for WfbDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "wlan.wfb"
    }
    fn outputs(&self) -> Vec<Port> {
        Vec::new()
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        WfbSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(WfbSetting::Links, Value::Choice(self.links.index()))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match WfbSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            WfbSetting::Links => self.links = LINKS.read(id, v)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(WfbFields::SHAPE.fields).with_tables(&[WFB_LINKS.def(), WFB_FRAMES.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.build(input)?;
        Ok(Vec::new())
    }
    fn retune(&mut self, input: &Format) {
        let _ = self.build(input);
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = Some(ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, _out: &mut Outbox) {
        if !self.started {
            self.origin = (first as f64 * SAMPLE_RATE / self.rate.max(1.0)) as u64;
        }
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        self.buf.clear();
        if let Some(ch) = self.ch.as_mut() {
            ch.process(x, &mut self.buf);
        }
        let (ht, got, counters) = (&mut self.ht, &mut self.got, &mut self.held.counters);
        self.rx.process_with(
            &self.buf,
            |_| {},
            |v| match ht.decode(v) {
                HtResult::NotHt => false,
                HtResult::NotDecodable(_) => {
                    counters.not_decodable += 1;
                    true
                }
                HtResult::BadFcs(_) => {
                    counters.bad_fcs += 1;
                    true
                }
                HtResult::Frame(f) => {
                    got.push(Got {
                        sample: v.sample,
                        mpdu: f.mpdu.to_vec(),
                        mcs: f.sig.mcs,
                        short_gi: f.sig.short_gi,
                        snr_db: f.snr_db,
                    });
                    true
                }
            },
        );
        let got = std::mem::take(&mut self.got);
        for g in &got {
            self.frame(g);
        }
        self.got = got;
        self.got.clear();
    }
    fn report(&mut self, p: &mut Published) {
        let c = self.held.counters;
        let recent = self.now - self.last_frame <= RECENT_S;
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        p.put(&WfbFields {
            frames_ok: n(c.link_frames),
            ht_frames: n(c.ht_frames),
            ht_not_decodable: n(c.not_decodable),
            bad_fcs: n(c.bad_fcs),
            links_heard: self.held.links.len() as i64,
            frames_lost: n(self.held.links.iter().map(|r| r.record.loss.lost).sum()),
            snr_db: self.snr_db,
            note: NOT_CONFIRMED,
        });
        if std::mem::take(&mut self.cleared) {
            p.results_mut().clear_log();
            p.clear_rows(WFB_LINKS.def());
            p.clear_rows(WFB_FRAMES.def());
        }
        if std::mem::take(&mut self.dirty) {
            shaped::rows(&self.held.links, &WFB_LINKS, self.now, p);
            shaped::log_rows(&self.held.frames, &WFB_FRAMES, self.now, p);
        }
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let mut summary = std::mem::take(&mut self.summary);
        self.summary(&mut summary);
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            if changed {
                s.lock_state = lock;
                s.summary.clone_from(&summary);
            }
            changed
        });
        self.summary = summary;
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.cleared = true;
        self.dirty = true;
    }
    /// The counters and the SNR start again; the links and the frame log stay if `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        self.held.counters = Counters::default();
        self.snr_db = None;
        self.last_frame = f64::NEG_INFINITY;
        if !kept.table(WFB_LINKS.key) {
            self.held.links.clear();
        }
        if !kept.table(WFB_FRAMES.key) {
            self.held.frames.clear();
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

/// The `wfb` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(WfbDecoder::default()))
        .input("rx")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "wfb-ng / OpenHD needs 20 Msps")
        .route(LINKS.key(), &["rx.links"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the component of this stack to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.wfb", || {
        Box::new(DecoderComponent::new(WfbDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::from_config(cfg)?))
}

/// The `wfb` stack. Its default frequency is a 5 GHz channel of the test signal; the session gives the real one.
pub static WFB: StackDescriptor = StackDescriptor {
    id: ID,
    name: "wfb-ng / OpenHD",
    group: "Data and IoT",
    blurb: "Drone video links of injected 802.11n frames: link id, radio port, FEC block and fragment, frames lost. The payload stays encrypted: structure only.",
    tuning: Tuning {
        no_offset_tuning: true,
        radio_filter_hz: Some(20e6),
        ..Tuning::anywhere(5805e6, 20e6, MIN_RATE_HZ, 20e6)
    },
    submodes: &[],
    settings: WfbSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::WFB),
    benchmarks: Some(benchmarks),
};

/// The benchmark of the receiver: 0.5 s of the test signal at 20 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "wfb.receiver_20m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 20e6;
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    if let Ok(mut s) = gen::scene(rate) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&WfbSetting::schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let c = WFB.tuning.default_hz;
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, c, c).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.5, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

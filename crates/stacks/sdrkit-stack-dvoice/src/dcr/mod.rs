//! DCR, the digital convenience radio of Japan (ARIB STD-T98 volume 3, four-level FSK at 2400 symbols/s): the RICH, the
//! SACCH units and their messages, the call sign of the synchronous burst, the data blocks, the calls, and a scan of the
//! plan channels in one capture. The physical layer and the channel coding are similar to NXDN48 and use its whitening and
//! its channel coders. Voice frames are counted; speech is not decoded.

pub mod gen;
pub mod link;
pub mod scan;

use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::front::{FskChain, Pulse};
use crate::nxdn::{scrambler, ChannelCoder, FACCH1, FSW, SACCH, SCRAMBLED};
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::{DcrEventRow, DcrFields, DCR_EVENTS};
use link::Link;
use scan::Scan;
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::{field, Declare, Published, TableDef};
use sdrkit_core::{Complex32, DeclareSetting, Setting, SettingError, SettingId, Value};
use sdrkit_graph::Format;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};

/// The stack id.
pub const ID: &str = "dcr";
/// What the stack says about speech.
pub const NOTE: &str = "Voice frames are counted; speech is not decoded, because the order of the AMBE+2 bits in a DCR traffic half is not confirmed.";
/// The most rows kept.
pub const ROW_CAP: usize = 500;
/// The most plan channels.
pub const PLAN_CAP: usize = 512;
/// A frequency maps to a plan channel within this distance, Hz.
pub const MATCH_HZ: f64 = 1_500.0;
/// The call key of the channel at the tuned frequency. A scanned channel has 1 plus its place in the plan.
const MAIN_KEY: u32 = 0;

sdrkit_core::choice_enum! {
    /// What a row of the DCR list reports.
    pub enum DcrRowKind {
        /// A call opened.
        Call = ("call", "Call"),
        /// A call ended.
        CallEnd = ("call_end", "Call end"),
        /// A call sign from a synchronous burst.
        CallSign = ("call_sign", "Call sign"),
        /// A coded data block.
        Data = ("data", "Data"),
        /// A message in a format of a manufacturer.
        ManufacturerMessage = ("manufacturer_message", "Manufacturer message"),
        /// An idle message.
        Idle = ("idle", "Idle"),
        /// A message of a reserved type.
        UnknownMessage = ("unknown_message", "Unknown message"),
    }
}

/// One row of the DCR list.
#[derive(Clone, Debug, PartialEq)]
pub struct DcrRow {
    /// Signal time, s.
    pub time_s: f64,
    /// What it reports.
    pub kind: DcrRowKind,
    /// Who: "UC n", a call sign, "TCH1" or "TCH2", after the plan label of a scanned channel.
    pub identity: String,
    /// The text.
    pub text: String,
    /// The plan label of a scanned channel.
    pub channel: Option<String>,
}

impl Shaped for DcrRow {
    type Row = DcrEventRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&DcrEventRow {
            time_s: self.time_s,
            kind: self.kind,
            identity: &self.identity,
            text: &self.text,
            channel: self.channel.as_deref(),
        });
    }
}

/// The counters of all channels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    /// Frames read after a sync.
    pub frames: u64,
    /// SACCH units that passed their CRC.
    pub sacch_ok: u64,
    /// SACCH units that failed their CRC.
    pub sacch_bad: u64,
    /// Frames whose RICH parity failed.
    pub rich_bad: u64,
    /// Call signs read from a PICH.
    pub pich_ok: u64,
    /// Data blocks that passed their CRC.
    pub data_ok: u64,
    /// Data blocks that failed their CRC.
    pub data_bad: u64,
    /// Voice frames of 20 ms in frames whose SACCH decoded.
    pub voice_frames: u64,
    /// Calls opened.
    pub calls: u64,
    /// Coded bits of the blocks that passed their CRC.
    pub bits: u64,
    /// The bits that the Viterbi decoder repaired in them.
    pub fixed: u64,
}

impl Counts {
    /// A block of `bits` coded bits passed its CRC with `fixed` bits repaired.
    pub fn checked(&mut self, bits: usize, fixed: usize) {
        self.bits += bits as u64;
        self.fixed += fixed as u64;
    }

    /// The repaired bits as a percentage of the coded bits.
    pub fn ber_pct(&self) -> f64 {
        if self.bits == 0 {
            0.0
        } else {
            100.0 * self.fixed as f64 / self.bits as f64
        }
    }
}

/// What the channels of a receiver share: the coders, the counters, the rows and the last identities heard.
pub struct Shared {
    /// The SACCH coder.
    pub sacch: ChannelCoder,
    /// The coder of the PICH and the data blocks.
    pub facch1: ChannelCoder,
    /// The whitening sequence.
    pub pn: [u8; SCRAMBLED],
    /// The counters.
    pub counts: Counts,
    /// The rows, oldest first.
    pub rows: LogRing<DcrRow>,
    /// The user code of the last message that counted.
    pub last_user_code: Option<u16>,
    /// The last call sign read.
    pub last_call_sign: Option<String>,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            sacch: ChannelCoder::new(SACCH),
            facch1: ChannelCoder::new(FACCH1),
            pn: scrambler(),
            counts: Counts::default(),
            rows: LogRing::new(ROW_CAP),
            last_user_code: None,
            last_call_sign: None,
        }
    }
}

impl Shared {
    /// Adds a row.
    pub fn row(&mut self, time_s: f64, kind: DcrRowKind, identity: String, text: String, channel: Option<String>) {
        self.rows.push(DcrRow {
            time_s,
            kind,
            identity,
            text,
            channel,
        });
    }
}

/// A channel of the plan that the consumer gives.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanChannel {
    /// The label ("351 MHz ch 15").
    pub label: String,
    /// The frequency, Hz.
    pub hz: f64,
}

/// The channels of the text of the setting `plan`: entries separated by ";", each a label, "=" and the frequency in Hz.
/// `None` when an entry does not parse or there are more than 512.
pub fn parse_plan(text: &str) -> Option<Vec<PlanChannel>> {
    let mut out = Vec::new();
    for e in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let (label, hz) = e.rsplit_once('=')?;
        let hz: f64 = hz.trim().parse().ok()?;
        let label = label.trim();
        if !(hz.is_finite() && hz > 0.0) || label.is_empty() {
            return None;
        }
        out.push(PlanChannel {
            label: label.to_string(),
            hz,
        });
    }
    (out.len() <= PLAN_CAP).then_some(out)
}

/// The place in `plan` of the channel nearest to `hz`, when it lies within 1500 Hz.
pub fn nearest(plan: &[PlanChannel], hz: f64) -> Option<usize> {
    plan.iter()
        .enumerate()
        .map(|(i, c)| (i, (c.hz - hz).abs()))
        .filter(|&(_, d)| d <= MATCH_HZ)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The channel plan: the channels to watch.
pub const PLAN: Setting<field::Text> = Setting::new("plan", "Channel plan", "", "").help(
    "The channels to watch: entries separated by \";\", each a label, \"=\" and the frequency in Hz, for example \"351 MHz ch 15=351287500\". At most 512 entries. The host fills it from the DCR plan file. Empty: the tuned frequency only.",
);
/// Watch every plan channel in the span.
pub const SCAN: Setting<field::Bool> = Setting::new("scan", "Watch every plan channel in the span", "", true)
    .help("Decode every plan channel inside 42 % of the sample rate around the centre that carries a signal. It needs two plan channels in the span and 100 ksps or more.");
/// The most channels decoded at once in a scan.
pub const MAX_CHANNELS: Setting<field::I64> =
    Setting::new("max_channels", "Channels decoded at once", "", 8).range(1.0, 8.0, 1.0);

/// The settings that the decoder of the stack takes.
pub const OWN: &[&dyn DeclareSetting] = &[&PLAN, &SCAN, &MAX_CHANNELS];
/// The settings of the stack beside those of the voice monitor: the decoder's and the test signal's.
pub const SETTINGS: &[&dyn DeclareSetting] = &[
    &PLAN,
    &SCAN,
    &MAX_CHANNELS,
    &gen::GEN_CONTENT,
    &gen::GEN_DEVIATION_SCALE,
];

/// The demodulation chain: a 6.25 kHz channel, the FM discriminator and a low-pass for the raised cosine pulses of 2400
/// symbols/s.
pub const fn chain_spec() -> FskChain {
    FskChain {
        raster_hz: 6_250.0,
        chan_pass_hz: 3_400.0,
        chan_stop_hz: 5_400.0,
        pulse: Pulse::LowPass(1_600.0),
        baud: gen::BAUD,
        dc_removal: true,
        lane2: None,
    }
}

/// The frame sync: the NXDN sync word, one wrong symbol allowed.
pub fn patterns() -> Vec<SyncPattern> {
    vec![SyncPattern::from_hex_dibits(0, FSW, 0).with_max_err(1)]
}

/// The DCR protocol: the channel at the tuned frequency, or in a scan the plan channels of the span, with the rows and
/// counters they share.
pub struct Dcr {
    sh: Shared,
    main: Link,
    plan_text: String,
    plan: Vec<PlanChannel>,
    scan_on: bool,
    max_channels: usize,
    scan: Scan,
    format: Option<Format>,
    channel: Option<String>,
}

impl Default for Dcr {
    fn default() -> Self {
        Dcr {
            sh: Shared::default(),
            main: Link::new(MAIN_KEY, None),
            plan_text: String::new(),
            plan: Vec::new(),
            scan_on: true,
            max_channels: 8,
            scan: Scan::default(),
            format: None,
            channel: None,
        }
    }
}

impl Dcr {
    /// Works out the plan channel of the tuned frequency and the scan from the format and the settings.
    fn refresh(&mut self) {
        let Some(f) = self.format.clone() else {
            return;
        };
        self.channel = if self.plan.is_empty() {
            None
        } else {
            Some(match nearest(&self.plan, f.tuned_hz) {
                Some(i) => self.plan[i].label.clone(),
                None => "not on the plan".into(),
            })
        };
        self.scan.configure(&f, &self.plan, self.scan_on);
    }

    /// The rows, oldest first.
    pub fn rows(&self) -> &LogRing<DcrRow> {
        &self.sh.rows
    }
}

impl Protocol for Dcr {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: ID,
            name: "DCR",
            baud: gen::BAUD,
            two_level: false,
            channel_hz: 6_250.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: true,
            extra_baud: None,
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        patterns()
    }

    fn on_sync(
        &mut self,
        _id: u8,
        _inverted: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        // In a scan every plan channel, the tuned one too, has a decoder of the scan.
        if self.scan.active {
            return SyncAnswer::Ignore;
        }
        self.main.sync(pre, tracking)
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        self.main.symbol(s, &mut self.sh, ctx)
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        self.main.tick(&mut self.sh, ctx);
        self.scan.tick(&mut self.sh, ctx);
    }

    fn on_reset(&mut self, _ctx: &mut Ctx) {
        self.main.reset();
        self.scan.drop_units();
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <DcrFields as Fields>::SHAPE.fields.to_vec()
    }

    fn tables(&self) -> Vec<&'static TableDef> {
        vec![DCR_EVENTS.def()]
    }

    fn publish(&self, r: &mut Published) {
        let c = &self.sh.counts;
        let n = |v: u64| v as i64;
        r.put(&DcrFields {
            dcr_frames: n(c.frames),
            sacch_ok: n(c.sacch_ok),
            sacch_bad: n(c.sacch_bad),
            rich_bad: n(c.rich_bad),
            pich_ok: n(c.pich_ok),
            data_ok: n(c.data_ok),
            data_bad: n(c.data_bad),
            voice_frames: n(c.voice_frames),
            calls: n(c.calls),
            ber_pct: c.ber_pct(),
            last_user_code: self.sh.last_user_code.map(i64::from),
            last_call_sign: self.sh.last_call_sign.as_deref(),
            channel: self.channel.as_deref(),
            scan_channels: if self.scan.active { self.scan.in_span.len() as i64 } else { 0 },
            scan_decoders: self.scan.decoders() as i64,
            scan_decoding: self.scan.decoding() as i64,
            note: NOTE,
        });
        shaped::log_rows(&self.sh.rows, &DCR_EVENTS, 0.0, r);
    }

    fn codec(&self) -> Codec {
        Codec::Ambe2
    }

    fn own_settings(&self) -> &'static [&'static dyn DeclareSetting] {
        OWN
    }

    fn apply_own(&mut self, index: usize, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match index {
            0 => {
                let text = PLAN.read(id, value)?;
                self.plan = parse_plan(text).ok_or(SettingError::Malformed { id, at: None })?;
                self.plan_text = text.to_string();
            }
            1 => self.scan_on = SCAN.read(id, value)?,
            2 => self.max_channels = MAX_CHANNELS.read(id, value)?.clamp(1, 8) as usize,
            _ => return Err(SettingError::NoSuchId { id }),
        }
        self.refresh();
        Ok(())
    }

    fn own_value(&self, index: usize) -> Value {
        match index {
            0 => Value::Text(self.plan_text.clone()),
            1 => Value::Bool(self.scan_on),
            _ => Value::I64(self.max_channels as i64),
        }
    }

    fn on_format(&mut self, input: &Format) {
        self.format = Some(input.clone());
        self.refresh();
    }

    fn on_input(&mut self, x: &[Complex32], ctx: &mut Ctx) {
        self.scan.process(x, &self.plan, &mut self.sh, ctx, self.max_channels);
    }

    fn on_clear(&mut self) {
        self.sh.rows.clear();
        self.sh.counts = Counts::default();
        self.sh.last_user_code = None;
        self.sh.last_call_sign = None;
    }
}

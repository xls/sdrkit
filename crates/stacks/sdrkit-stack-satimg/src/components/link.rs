//! The link components of the coded links: front end, PSK demodulator, code synchroniser and decoder, frame synchroniser with
//! Reed-Solomon, and the AOS frame and packet layer.
//! Standard: CCSDS 131.0-B-5, CCSDS 732.0-B-4, CCSDS 133.0-B-2

use super::{controls, Control};
use crate::files;
use crate::typed::{FileArgs, OPEN_CADU};
use crate::items::{Cadu, SpacePacket, Vcdu, CADU_LEN, MAX_ZONE};
use crate::psk::{front_end, DemodState, LinkParams, PskDemod};
use sdrkit_algo_fec::rs::Basis;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::{
    ConfigError, Record, SettingError, SettingId, StageError, Storage, Value, Values,
};
use sdrkit_graph::{
    no_values, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::channel::{BasisPolicy, ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::codesync::{CodeConfig, CodeLock, CodeSync};
use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
use sdrkit_proto_ccsds::packet::{Assembler, AssemblerConfig};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A lower crate's error as a configuration error.
pub(crate) fn stage(e: impl std::error::Error + Send + Sync + 'static) -> ConfigError {
    ConfigError::Stage(StageError::new(e))
}

/// A port of reports or other items with no sample rate.
pub(crate) fn item_port<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// The input of the control items of a display.
pub(crate) fn control_port() -> Port {
    Port::typed::<Control>("control").optional("nothing changes the link")
}

/// Two code symbols for each decoded bit (the rate 1/2 code).
const HALF: std::num::NonZeroU32 = match std::num::NonZeroU32::new(2) {
    Some(n) => n,
    None => unreachable!(),
};

/// The format of an item stream.
pub(crate) fn item_format<T: Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The front end's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrontReport {
    /// Mean input power since the last report, dBFS.
    pub level_dbfs: f32,
    /// Samples near full scale.
    pub overload: bool,
    /// The input rate.
    pub input_rate: f64,
    /// The output rate.
    pub output_rate: f64,
    /// The move of the energy location, Hz.
    pub located_hz: f64,
    /// The offset of the signal that the user set (the tuning of the radio), Hz.
    pub user_offset_hz: f64,
    /// Seconds of signal.
    pub time_s: f64,
    /// The rate is below the least rate of the stack.
    pub rate_too_low: bool,
}
impl Item for FrontReport {
    const KIND: &'static str = "satimg.front_report";
}

/// The front end: bad samples to zero, level, energy location, mixer, anti-alias decimation. Its `control` input takes a new
/// link ([`Control::Link`]) and a new least rate ([`Control::MinRate`]).
pub struct FrontEndComponent {
    link: LinkParams,
    decim_rs: Option<f64>,
    sink: Vec<sdrkit_core::Complex32>,
    blind_iq: bool,
    min_rate: f64,
    fe: Option<sdrkit_algo_modem::psk::PskFrontEnd>,
    rate: f64,
    offset: f64,
    samples: u64,
}

impl FrontEndComponent {
    /// A front end for `link`. Below `min_rate` it measures the level only.
    pub fn new(link: LinkParams, min_rate: f64, blind_iq: bool) -> Self {
        FrontEndComponent {
            link,
            decim_rs: None,
            sink: Vec::new(),
            blind_iq,
            min_rate,
            fe: None,
            rate: 0.0,
            offset: 0.0,
            samples: 0,
        }
    }
    /// Decimates as for a symbol rate of `rs` (the fastest of the links a display can switch to at run time). The output rate
    /// then stays when the link changes.
    pub fn with_decimation_for(mut self, rs: f64) -> Self {
        self.decim_rs = Some(rs);
        self
    }
}

impl Component for FrontEndComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.frontend",
            vec![Port::iq("in"), control_port()],
            vec![Port::iq("out"), item_port::<FrontReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.offset = f.signal_offset_hz();
        let mut dl = self.link.clone();
        if let Some(rs) = self.decim_rs {
            dl.symbol_rate = rs;
        }
        let fe = front_end(f.rate, self.offset, &dl, self.blind_iq)?;
        let out_rate = fe.output_rate();
        self.fe = Some(fe);
        self.samples = 0;
        let mut o = Format::iq(out_rate, f.tuned_hz, f.tuned_hz);
        o.bandwidth_hz = self.link.symbol_rate * (1.0 + self.link.roll_off);
        Ok(vec![o, item_format::<FrontReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let mut reformat = false;
        for c in controls(io, 1) {
            match c {
                Control::Link(l) if *l != self.link => {
                    self.link = l.clone();
                    reformat |= self.decim_rs.is_none();
                }
                Control::MinRate(hz) => self.min_rate = *hz,
                _ => {}
            }
        }
        if reformat {
            io.request_reformat();
        }
        let x = io.iq(0);
        self.samples += x.len() as u64;
        let too_low = self.rate < self.min_rate;
        if let Some(fe) = &mut self.fe {
            if too_low {
                // Below the least rate: measure the level only.
                self.sink.clear();
                fe.process(x, &mut self.sink);
            } else {
                let out = io.iq_out(0);
                fe.process(x, out);
            }
        }
        if io.tick() {
            if let Some(fe) = &mut self.fe {
                let (level, over) = fe.take_level();
                let r = FrontReport {
                    level_dbfs: level,
                    overload: over,
                    input_rate: self.rate,
                    output_rate: fe.output_rate(),
                    located_hz: fe.located_hz(),
                    user_offset_hz: self.offset,
                    time_s: self.samples as f64 / self.rate.max(1.0),
                    rate_too_low: too_low,
                };
                io.items_out::<FrontReport>(1).push(Timed {
                    time: r.time_s,
                    value: r,
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The demodulator's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemodReport {
    /// The state.
    pub state: DemodState,
    /// The nominal symbol rate of the link in use.
    pub symbol_rate_nominal: f64,
    /// The constellation (normalised).
    pub constellation: Vec<[f32; 2]>,
    /// Seconds of signal.
    pub time_s: f64,
}
impl Item for DemodReport {
    const KIND: &'static str = "satimg.demod_report";
}

/// The PSK demodulator: soft code symbols out as LLRs (positive means 0).
///
/// Control items: [`Control::Link`], [`Control::RestartCarrier`], [`Control::RejectCarrier`].
pub struct DemodComponent {
    link: LinkParams,
    demod: Option<PskDemod>,
    rate: f64,
    soft: Vec<i8>,
    samples: u64,
}

impl DemodComponent {
    /// A demodulator that starts on `link`.
    pub fn new(link: LinkParams) -> Self {
        DemodComponent {
            link,
            demod: None,
            rate: 0.0,
            soft: Vec::new(),
            samples: 0,
        }
    }
}

impl Default for DemodComponent {
    /// A demodulator that starts on the Meteor-M OQPSK 72k link.
    fn default() -> Self {
        DemodComponent::new(crate::modes::meteor::MeteorModulation::Oqpsk72.link())
    }
}

impl Component for DemodComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.psk_demod",
            vec![Port::iq("in"), control_port()],
            vec![Port::llr("soft"), item_port::<DemodReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.demod = Some(PskDemod::new(f.rate, &self.link)?);
        let bits_rate = self.link.symbol_rate * self.link.modulation.bits_per_symbol() as f64;
        self.samples = 0;
        let mut o = Format::llr(bits_rate, None);
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o, item_format::<DemodReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for c in controls(io, 1) {
            match c {
                Control::Link(l) if *l != self.link => {
                    // A link that the present rate cannot serve waits for the front end to configure the chain again.
                    self.demod = PskDemod::new(self.rate, l).ok();
                    self.link = l.clone();
                }
                Control::RestartCarrier | Control::RejectCarrier => {
                    if let Some(d) = &mut self.demod {
                        d.restart(*c == Control::RejectCarrier);
                    }
                }
                _ => {}
            }
        }
        let x = io.iq(0);
        self.samples += x.len() as u64;
        let Some(d) = &mut self.demod else { return };
        self.soft.clear();
        d.process(x, &mut self.soft);
        io.llrs_out(0).extend_from_slice(&self.soft);
        if io.tick() {
            let r = DemodReport {
                state: d.report().clone(),
                symbol_rate_nominal: d.params().symbol_rate,
                constellation: d.constellation().collect(),
                time_s: self.samples as f64 / self.rate.max(1.0),
            };
            io.items_out::<DemodReport>(1).push(Timed {
                time: r.time_s,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The code decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeReport {
    /// The lock.
    pub lock: CodeLock,
}
impl Item for CodeReport {
    const KIND: &'static str = "satimg.code_report";
}

/// The code synchroniser and Viterbi decoder: LLRs of code symbols in, hard decoded bits out.
///
/// Control items: a new [`Control::Link`] searches again, as do [`Control::CodeSearch`] and a carrier restart.
pub struct CodeComponent {
    cfg: CodeConfig,
    link: Option<LinkParams>,
    sync: Option<CodeSync>,
}

impl CodeComponent {
    /// A decoder with these rules.
    pub fn new(cfg: CodeConfig) -> Self {
        CodeComponent {
            cfg,
            link: None,
            sync: None,
        }
    }
    fn search_again(&mut self) {
        if let Some(s) = &mut self.sync {
            s.search_again();
        }
    }
}

impl Component for CodeComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.ccsds_decoder",
            vec![Port::llr("in"), control_port()],
            vec![
                Port::hard_bits("out").rate_from(RateFrom::Divided { input: 0, by: HALF }),
                item_port::<CodeReport>("report"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sync = Some(CodeSync::new(self.cfg).map_err(stage)?);
        let f = &inputs[0];
        let mut o = Format::hard_bits(f.rate / 2.0);
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o, item_format::<CodeReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let mut again = false;
        for c in controls(io, 1) {
            match c {
                Control::Link(l) => {
                    // The first link is the one the chain started on: only a change searches again.
                    if self.link.as_ref() != Some(l) {
                        again |= self.link.is_some();
                        self.link = Some(l.clone());
                    }
                }
                Control::CodeSearch | Control::RestartCarrier | Control::RejectCarrier => {
                    again = true
                }
                Control::MinRate(_) | Control::Measure(_) => {}
            }
        }
        if again {
            self.search_again();
        }
        let Some(s) = &mut self.sync else { return };
        s.push(io.llrs(0), io.hard_bits_out(0));
        if io.tick() {
            let lock = *s.lock();
            io.items_out::<CodeReport>(1).push(Timed {
                time: 0.0,
                value: CodeReport { lock },
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The frame layer of a link: how frames are found and how their channel coding is removed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameLink {
    /// Finding the frames.
    pub sync: SyncConfig,
    /// The randomiser and Reed-Solomon.
    pub channel: ChannelConfig,
    /// Search again after this many frames in a row that fail Reed-Solomon (0: never).
    pub max_rs_fails: u32,
}

impl FrameLink {
    /// Meteor-M: the low-orbit rules, Reed-Solomon in the conventional basis.
    pub const METEOR: FrameLink = FrameLink {
        sync: SyncConfig::LEO,
        channel: ChannelConfig::rs_223(4).with_basis(BasisPolicy::Fixed(Basis::Conventional)),
        max_rs_fails: 3,
    };
    /// MetOp AHRPT and FY-3: the low-orbit rules, Reed-Solomon in the dual basis.
    pub const LEO_DUAL: FrameLink = FrameLink {
        sync: SyncConfig::LEO,
        channel: ChannelConfig::rs_223(4),
        max_rs_fails: 3,
    };
    /// GOES, GK-2A and Elektro-L: the geostationary rules, Reed-Solomon in the basis that decodes.
    pub const GEO: FrameLink = FrameLink {
        sync: SyncConfig::GEO,
        channel: ChannelConfig::rs_223(4).with_basis(BasisPolicy::Auto(Basis::Dual)),
        max_rs_fails: 0,
    };

    /// The basis the decoder starts with.
    pub fn start_basis(&self) -> Basis {
        match self.channel.basis {
            BasisPolicy::Fixed(b) | BasisPolicy::Auto(b) => b,
        }
    }
}

/// Counters of the frame layer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Frames that passed Reed-Solomon.
    pub good: u64,
    /// Frames that failed.
    pub failed: u64,
    /// Bytes corrected.
    pub corrected: u64,
    /// Bytes checked (1020 for each frame).
    pub checked: u64,
    /// Markers found (acquisitions and tracked ones).
    pub syncs: u64,
    /// The lock is held.
    pub locked: bool,
    /// The basis in use.
    pub basis: Option<Basis>,
    /// The basis changed this many times.
    pub basis_changes: u32,
}

/// The frame layer's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameReport {
    /// Counters.
    pub stats: FrameStats,
    /// Good frames since the last report.
    pub good_now: u64,
    /// Failed frames since the last report.
    pub failed_now: u64,
    /// Bytes corrected over the last second, percent.
    pub byte_error_pct: f64,
    /// Seconds since the last good frame (signal time).
    pub since_good_s: f64,
    /// New log lines.
    pub log: Vec<String>,
    /// CADUs imported from a file.
    pub imported: u64,
    /// The message of the import.
    pub import_note: String,
}
impl Item for FrameReport {
    const KIND: &'static str = "satimg.frame_report";
}

/// Bits given to the synchroniser at a time while a lock can be dropped after Reed-Solomon failures. The pieces end at whole
/// multiples of this count of input bits, so the drop acts at the same bit whatever the cuts of the input.
const DROP_STEP: u64 = 64;

/// The frame layer as a plain type: frame synchronisation, the randomiser and Reed-Solomon, with its counters. Each frame found
/// leaves as a [`Cadu`], good or failed.
#[derive(Debug)]
pub struct FrameDecoder {
    link: FrameLink,
    sync: FrameSync,
    dec: ChannelDecoder,
    stats: FrameStats,
    rs_fails: u32,
    drop: bool,
    log: Vec<String>,
    bit_rate: f64,
    bits_in: u64,
}

impl FrameDecoder {
    /// A frame layer for `link`. If the link's rules are not valid, it returns the error.
    pub fn new(link: FrameLink) -> Result<Self, sdrkit_proto_ccsds::Error> {
        Ok(FrameDecoder {
            link,
            sync: FrameSync::new(link.sync)?,
            dec: ChannelDecoder::new(link.channel)?,
            stats: FrameStats {
                basis: Some(link.start_basis()),
                ..FrameStats::default()
            },
            rs_fails: 0,
            drop: false,
            log: Vec::new(),
            bit_rate: 1.0,
            bits_in: 0,
        })
    }

    /// The bits per second of the input, for the times of the frames.
    pub fn set_bit_rate(&mut self, hz: f64) {
        self.bit_rate = hz.max(1.0);
    }

    /// Drops the lock and the counts of failures (a retune or a new rate). The counters stay.
    pub fn restart(&mut self) {
        self.sync.drop_lock();
        self.rs_fails = 0;
        self.stats.locked = false;
    }

    /// The counters.
    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    /// The bits taken since the start.
    pub fn bits_in(&self) -> u64 {
        self.bits_in
    }

    /// Takes the log lines written since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// Runs hard bits, appending a [`Cadu`] for each frame found.
    pub fn process(&mut self, bits: &[u8], out: &mut Vec<Cadu>) {
        if self.link.max_rs_fails == 0 {
            self.run(bits, out);
            self.bits_in += bits.len() as u64;
            return;
        }
        let mut rest = bits;
        while !rest.is_empty() {
            let to_step = (DROP_STEP - self.bits_in % DROP_STEP) as usize;
            let n = to_step.min(rest.len());
            self.run(&rest[..n], out);
            self.bits_in += n as u64;
            rest = &rest[n..];
        }
    }

    fn run(&mut self, bits: &[u8], cadus: &mut Vec<Cadu>) {
        let link = self.link;
        let dec = &mut self.dec;
        let stats = &mut self.stats;
        let log = &mut self.log;
        let rs_fails = &mut self.rs_fails;
        let drop = &mut self.drop;
        let rate = self.bit_rate;
        self.sync.push_bits(bits, |f| {
            let mut c = Cadu {
                bytes: [0; CADU_LEN],
                corrected: None,
                checked: link.channel.rs.is_some(),
                basis: dec.basis(),
                time: f.start_bit as f64 / rate,
            };
            c.bytes[..4].copy_from_slice(link.sync.marker.bytes());
            c.bytes[4..].copy_from_slice(f.bytes);
            stats.syncs += 1;
            match dec.decode(&mut c.bytes[4..]) {
                Ok(d) => {
                    c.basis = d.basis;
                    if d.basis_changed {
                        stats.basis_changes += 1;
                        stats.basis = Some(d.basis);
                        log.push(format!(
                            "Reed-Solomon symbols are in the {} basis",
                            if d.basis == Basis::Dual {
                                "dual (CCSDS)"
                            } else {
                                "conventional"
                            }
                        ));
                    }
                    if d.ok() {
                        c.corrected = Some(d.corrected.min(u16::MAX as u32) as u16);
                        stats.good += 1;
                        stats.corrected += d.corrected as u64;
                        *rs_fails = 0;
                    } else {
                        stats.failed += 1;
                        *rs_fails += 1;
                        if link.max_rs_fails > 0 && *rs_fails >= link.max_rs_fails {
                            *rs_fails = 0;
                            *drop = true;
                        }
                    }
                    if d.checked {
                        stats.checked += (CADU_LEN - 4) as u64;
                    }
                }
                Err(_) => stats.failed += 1,
            }
            cadus.push(c);
        });
        if std::mem::take(&mut self.drop) {
            self.sync.drop_lock();
        }
        self.stats.locked = self.sync.locked();
    }
}

sdrkit_core::commands! {
    /// The commands of [`FrameComponent`].
    pub enum FrameCommand {
        /// Imports a file of CADUs.
        OpenCadu = OPEN_CADU.command,
    }
}

/// The frame synchroniser with derandomisation and Reed-Solomon, and the CADU import (the command `open_cadu`). Good and failed
/// frames leave on `out` as [`Cadu`] items: the host records the good ones from a tap on this output.
pub struct FrameComponent {
    link: FrameLink,
    frames: Option<FrameDecoder>,
    cadus: Vec<Cadu>,
    last_good: u64,
    last_failed: u64,
    window: std::collections::VecDeque<(u64, u64)>,
    bit_rate: f64,
    last_good_bit: u64,
    import: Vec<u8>,
    imported: u64,
    import_note: String,
    storage: Arc<dyn Storage>,
}

impl FrameComponent {
    /// A frame layer for `link`.
    pub fn new(link: FrameLink) -> Self {
        FrameComponent {
            link,
            frames: None,
            cadus: Vec::new(),
            last_good: 0,
            last_failed: 0,
            window: Default::default(),
            bit_rate: 1.0,
            last_good_bit: 0,
            import: Vec::new(),
            imported: 0,
            import_note: String::new(),
            storage: files::in_memory(),
        }
    }

    /// Returns the bytes of a CADU file to import at the next input.
    pub fn import(&mut self, bytes: Vec<u8>) {
        self.import = bytes;
        self.imported = 0;
    }

    /// Imports a file of 1024-byte CADUs (derandomised or as sent): Reed-Solomon (where the link has it) and the layers above.
    fn run_import(&mut self, out: &mut Vec<Timed<Cadu>>) {
        let data = std::mem::take(&mut self.import);
        if data.len() < CADU_LEN {
            self.import_note = "not a CADU file".into();
            return;
        }
        let frames: &[[u8; CADU_LEN]] = data.as_chunks::<CADU_LEN>().0;
        let channel = self.link.channel;
        // The randomiser sequence is itself a code word: try both choices on the first frames and keep the one whose frame
        // headers say AOS more often.
        let score = |cfg: ChannelConfig| -> usize {
            let Ok(mut d) = ChannelDecoder::new(cfg) else {
                return 0;
            };
            frames
                .iter()
                .take(16)
                .filter(|f| {
                    let mut c = **f;
                    d.decode(&mut c[4..]).is_ok_and(|r| r.ok()) && c[4] >> 6 == 1
                })
                .count()
        };
        let plain = channel.without_randomiser();
        let cfg = if score(channel) > score(plain) {
            channel
        } else {
            plain
        };
        let Ok(mut dec) = ChannelDecoder::new(cfg) else {
            self.import_note = "not a CADU file".into();
            return;
        };
        for f in frames {
            let mut c = Cadu {
                bytes: *f,
                corrected: None,
                checked: cfg.rs.is_some(),
                basis: dec.basis(),
                time: 0.0,
            };
            if let Ok(d) = dec.decode(&mut c.bytes[4..]) {
                c.basis = d.basis;
                if d.ok() {
                    c.corrected = Some(d.corrected.min(u16::MAX as u32) as u16);
                }
            }
            self.imported += 1;
            out.push(Timed {
                time: 0.0,
                value: c,
            });
        }
        self.import_note = format!("{} CADUs imported", self.imported);
    }
}

impl Component for FrameComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.frame_sync",
            vec![Port::hard_bits("in")],
            vec![item_port::<Cadu>("out"), item_port::<FrameReport>("report")],
        )
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn commands(&self) -> &Commands {
        FrameCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match FrameCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FrameCommand::OpenCadu => {
                let a: FileArgs = OPEN_CADU.args(args)?;
                let bytes = files::read(self.storage.as_ref(), args, a.path)?;
                self.import(bytes);
            }
        }
        Ok(None)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if self.frames.is_none() {
            self.frames = Some(FrameDecoder::new(self.link).map_err(stage)?);
        }
        self.bit_rate = inputs[0].rate.max(1.0);
        if let Some(f) = &mut self.frames {
            f.restart();
            f.set_bit_rate(self.bit_rate);
        }
        Ok(vec![item_format::<Cadu>(), item_format::<FrameReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(frames) = &mut self.frames else {
            return;
        };
        self.cadus.clear();
        frames.process(io.hard_bits(0), &mut self.cadus);
        if self.cadus.iter().any(|c| c.good()) {
            self.last_good_bit = frames.bits_in();
        }
        let out = io.items_out::<Cadu>(0);
        for c in self.cadus.drain(..) {
            out.push(Timed {
                time: c.time,
                value: c,
            });
        }
        if !self.import.is_empty() {
            let mut v = Vec::new();
            self.run_import(&mut v);
            io.items_out::<Cadu>(0).extend(v);
        }
        if io.tick() {
            let Some(frames) = &mut self.frames else {
                return;
            };
            let s = *frames.stats();
            let good_now = s.good - self.last_good;
            let failed_now = s.failed - self.last_failed;
            self.last_good = s.good;
            self.last_failed = s.failed;
            self.window.push_back((s.corrected, s.checked));
            while self.window.len() > 5 {
                self.window.pop_front();
            }
            let (c0, k0) = *self.window.front().unwrap_or(&(0, 0));
            let pct = if s.checked > k0 {
                100.0 * (s.corrected - c0) as f64 / (s.checked - k0) as f64
            } else {
                0.0
            };
            let r = FrameReport {
                stats: s,
                good_now,
                failed_now,
                byte_error_pct: pct,
                since_good_s: (frames.bits_in() - self.last_good_bit) as f64 / self.bit_rate,
                log: frames.take_log(),
                imported: self.imported,
                import_note: std::mem::take(&mut self.import_note),
            };
            io.items_out::<FrameReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// Counters of the packet layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PacketStats {
    /// Packets given out.
    pub packets: u64,
    /// Packets lost (cut by a frame gap, or with a damaged header).
    pub lost: u64,
    /// Packets with a wrong CRC.
    pub crc_errors: u64,
    /// Frames with only idle data (first header pointer 0x7FE, CCSDS 732.0-B-4 clause 4.1.4.2.2).
    pub idle_frames: u64,
    /// Frames refused: a version that is not AOS, or a first header pointer beyond the zone.
    pub refused: u64,
    /// Fill frames (virtual channel 63).
    pub fill_frames: u64,
    /// The last frame: spacecraft, virtual channel, frame count.
    pub last: Option<(u8, u8, u32)>,
    /// Packets by APID.
    pub per_apid: BTreeMap<u16, u64>,
    /// Frames by virtual channel (fill frames not counted).
    pub per_vc: BTreeMap<u8, u64>,
}

/// The packet layer's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PacketReport {
    /// Counters.
    pub stats: PacketStats,
}
impl Item for PacketReport {
    const KIND: &'static str = "satimg.packet_report";
}

/// The AOS frame parser and packet reassembly.
pub struct VcduComponent {
    layout: Layout,
    asm: Assembler,
    stats: PacketStats,
    bad_pointers: u64,
    packets: Vec<SpacePacket>,
}

impl VcduComponent {
    /// For frames of `layout`. With `crc`, each packet ends with the CRC-16 of the xRIT transport layer.
    pub fn new(layout: Layout, crc: bool) -> Self {
        VcduComponent {
            layout,
            asm: Assembler::new(AssemblerConfig {
                crc,
                ..AssemblerConfig::default()
            }),
            stats: PacketStats::default(),
            bad_pointers: 0,
            packets: Vec::new(),
        }
    }

    /// Takes one transfer frame apart: appends its frame item to `frames` and the packets it completes to `packets`. A frame
    /// that is too short or not AOS is counted as refused.
    pub fn push_frame(
        &mut self,
        bytes: &[u8],
        time: f64,
        frames: &mut Vec<Vcdu>,
        packets: &mut Vec<SpacePacket>,
    ) {
        self.packets.clear();
        self.frame(bytes, time, frames);
        packets.append(&mut self.packets);
    }

    fn frame(&mut self, bytes: &[u8], time: f64, out: &mut Vec<Vcdu>) {
        let Ok(f) = self.layout.parse(bytes) else {
            self.stats.refused += 1;
            return;
        };
        let h = f.header;
        if h.version != 1 {
            self.stats.refused += 1;
            return;
        }
        self.stats.last = Some((h.spacecraft, h.vcid, h.counter));
        if h.is_idle() {
            self.stats.fill_frames += 1;
            return;
        }
        *self.stats.per_vc.entry(h.vcid).or_insert(0) += 1;
        if f.first_header == sdrkit_proto_ccsds::tm::FirstHeader::IdleOnly {
            self.stats.idle_frames += 1;
        }
        let mut v = Vcdu {
            version: h.version,
            spacecraft: h.spacecraft,
            vcid: h.vcid,
            counter: h.counter,
            first_header: f.first_header.field(),
            zone: [0; MAX_ZONE],
            zone_len: f.zone.len().min(MAX_ZONE) as u16,
            time,
        };
        v.zone[..v.zone_len as usize].copy_from_slice(&f.zone[..v.zone_len as usize]);
        out.push(v);
        let stats = &mut self.stats;
        let packets = &mut self.packets;
        self.asm
            .push_mpdu(h.vcid, Some(h.counter), f.first_header, f.zone, |p| {
                if p.crc_ok == Some(false) {
                    stats.crc_errors += 1;
                }
                stats.packets += 1;
                *stats.per_apid.entry(p.header.apid).or_insert(0) += 1;
                packets.push(SpacePacket {
                    vcid: p.vcid,
                    spacecraft: h.spacecraft,
                    apid: p.header.apid,
                    flags: p.header.flags.bits(),
                    seq: p.header.count,
                    data: Arc::from(p.data),
                    crc_ok: p.crc_ok,
                    gap_before: p.gap_before,
                    time,
                });
            });
        self.stats.lost = self.asm.stats().lost;
        self.stats.refused += self.asm.stats().bad_pointers - self.bad_pointers;
        self.bad_pointers = self.asm.stats().bad_pointers;
    }

    /// The counters.
    pub fn stats(&self) -> &PacketStats {
        &self.stats
    }
}

impl Component for VcduComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.vcdu",
            vec![Port::typed::<Cadu>("in")],
            vec![
                item_port::<Vcdu>("vcdu"),
                item_port::<SpacePacket>("packets"),
                item_port::<PacketReport>("report"),
            ],
        )
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.asm.reset();
        Ok(vec![
            item_format::<Vcdu>(),
            item_format::<SpacePacket>(),
            item_format::<PacketReport>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut frames = Vec::new();
        for c in io.items::<Cadu>(0) {
            if !c.value.good() {
                continue;
            }
            self.packets.clear();
            frames.clear();
            self.frame(c.value.vcdu(), c.value.time, &mut frames);
            io.items_out::<Vcdu>(0)
                .extend(frames.drain(..).map(|v| Timed {
                    time: v.time,
                    value: v,
                }));
            io.items_out::<SpacePacket>(1)
                .extend(self.packets.drain(..).map(|p| Timed {
                    time: p.time,
                    value: p,
                }));
        }
        if io.tick() {
            io.items_out::<PacketReport>(2).push(Timed {
                time: 0.0,
                value: PacketReport {
                    stats: self.stats.clone(),
                },
            });
        }
    }
}

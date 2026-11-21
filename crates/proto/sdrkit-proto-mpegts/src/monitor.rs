//! The transport stream monitor: it counts the output of a receiver (packets, packets with the transport_error_indicator, scrambled
//! packets, numbered test packets, the bit rate) and finds the service names in the SDT. It does not change the stream.
//!
//! [`TsMonitor`] is the plain type. [`TsMonitorComponent`] (`mpegts.ts_monitor`) runs it in a graph: one or more transport
//! stream inputs, merged in input order, one output with the same packets, and the counts as results.

use crate::demux::SectionAssembler;
use crate::packet::{self, TS_SIZE};
use crate::source::{Fill, Numbered, NumberedCheck};
use crate::stream::{self, TsPacket};
use crate::tables;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, StackShape};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{
    Choice, ConfigError, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, TagKind};

/// The most service names kept.
pub const MAX_SERVICES: usize = 64;

/// The PID of the SDT (EN 300 468 5.1.3).
const SDT_PID: u16 = 0x0011;

/// What the monitor counted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonitorStats {
    /// Packets seen.
    pub packets: u64,
    /// Packets with the transport_error_indicator.
    pub flagged: u64,
    /// Scrambled packets (transport_scrambling_control not 0).
    pub scrambled: u64,
    /// Packets that did not start with the sync byte.
    pub bad_sync: u64,
    /// A PAT was seen.
    pub pat_seen: bool,
    /// The service names of the SDT actual, in the order of the SDT (at most [`MAX_SERVICES`]).
    pub services: Vec<String>,
    /// The check of the numbered packets.
    pub numbered: NumberedCheck,
    /// The bit rate over the last second of signal, bit/s (packets of 188 bytes).
    pub bitrate_bps: f64,
    /// The signal time of the first packet.
    pub first_time: Option<f64>,
}

/// Ten buckets of 0.1 s of signal time: a bit rate over the last second.
#[derive(Clone, Debug, Default)]
struct Rate {
    counts: [u64; 10],
    /// The bucket number (time / 0.1 s) of the newest bucket.
    head: Option<i64>,
    first: f64,
}

impl Rate {
    fn push(&mut self, time: f64) -> f64 {
        let b = (time * 10.0).floor() as i64;
        match self.head {
            None => {
                self.head = Some(b);
                self.first = time;
            }
            Some(h) if b > h => {
                for k in (h + 1)..=(b.min(h + 10)) {
                    self.counts[k.rem_euclid(10) as usize] = 0;
                }
                self.head = Some(b);
            }
            Some(h) if b < h - 9 => {
                // Time went back (a new recording, a retune): start again.
                *self = Rate::default();
                return self.push(time);
            }
            _ => {}
        }
        self.counts[b.rem_euclid(10) as usize] += 1;
        let span = (time - self.first).clamp(0.1, 1.0);
        self.counts.iter().sum::<u64>() as f64 * (TS_SIZE as f64 * 8.0) / span
    }
}

/// The transport stream monitor. It does not change the stream.
#[derive(Debug)]
pub struct TsMonitor {
    stats: MonitorStats,
    pattern: Option<Numbered>,
    rate: Rate,
    sdt: SectionAssembler,
    /// The CRC of the last SDT section decoded, by section_number.
    sdt_crc: [Option<u32>; 4],
    sdt_scratch: Vec<u8>,
}

impl TsMonitor {
    /// A monitor that checks the numbered packets of `pattern` (none when `None`).
    pub fn new(pattern: Option<Numbered>) -> TsMonitor {
        TsMonitor {
            stats: MonitorStats::default(),
            pattern,
            rate: Rate::default(),
            sdt: SectionAssembler::default(),
            sdt_crc: [None; 4],
            sdt_scratch: Vec::with_capacity(1024),
        }
    }

    /// The numbered pattern it checks.
    pub fn pattern(&self) -> Option<Numbered> {
        self.pattern
    }

    /// Checks another pattern from now on (the counts of the old one are cleared).
    pub fn set_pattern(&mut self, pattern: Option<Numbered>) {
        self.pattern = pattern;
        self.stats.numbered = NumberedCheck::default();
    }

    /// The counters.
    pub fn stats(&self) -> &MonitorStats {
        &self.stats
    }

    /// Forgets every count and name.
    pub fn clear(&mut self) {
        *self = TsMonitor::new(self.pattern);
    }

    /// Counts one transport packet at signal time `time`.
    pub fn push(&mut self, p: &[u8; TS_SIZE], time: f64) {
        let st = &mut self.stats;
        st.packets += 1;
        st.first_time.get_or_insert(time);
        st.bitrate_bps = self.rate.push(time);
        let Some(parsed) = packet::parse(p) else {
            st.bad_sync += 1;
            return;
        };
        let h = parsed.header;
        if h.tei {
            st.flagged += 1;
        }
        if h.scrambling != 0 {
            st.scrambled += 1;
        }
        if let Some(pat) = &self.pattern {
            st.numbered.push(pat, p, time);
        }
        if h.tei {
            return;
        }
        match h.pid {
            0 => st.pat_seen = true,
            SDT_PID => {
                let (sdt, crcs, scratch, names) = (
                    &mut self.sdt,
                    &mut self.sdt_crc,
                    &mut self.sdt_scratch,
                    &mut st.services,
                );
                sdt.push(parsed.payload, h.pusi, &mut |s| {
                    sdt_names(s, crcs, scratch, names)
                });
            }
            _ => {}
        }
    }
}

/// Reads the service names of an SDT actual section (EN 300 468 5.2.3, service_descriptor 6.2.33) when its CRC is new.
fn sdt_names(
    s: &[u8],
    crcs: &mut [Option<u32>; 4],
    scratch: &mut Vec<u8>,
    names: &mut Vec<String>,
) {
    let Ok(h) = tables::check(s) else { return };
    if h.table_id != 0x42 || h.total_len < 4 {
        return;
    }
    let crc = u32::from_be_bytes([
        s[h.total_len - 4],
        s[h.total_len - 3],
        s[h.total_len - 2],
        s[h.total_len - 1],
    ]);
    let slot = (h.number as usize).min(3);
    if crcs[slot] == Some(crc) {
        return;
    }
    crcs[slot] = Some(crc);
    scratch.clear();
    scratch.extend_from_slice(&s[..h.total_len]);
    let Some(sdt) = tables::sdt(scratch, &h, &Default::default()) else {
        return;
    };
    for sv in sdt.services {
        if !sv.name.is_empty() && !names.contains(&sv.name) && names.len() < MAX_SERVICES {
            names.push(sv.name);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// Packets seen.
pub const PACKETS: Field<field::I64> = Field::new("packets", "Packets", "");
/// Packets with the transport_error_indicator.
pub const FLAGGED: Field<field::I64> = Field::new("flagged", "Uncorrectable packets", "");
/// Scrambled packets.
pub const SCRAMBLED: Field<field::I64> = Field::new("scrambled", "Scrambled packets", "");
/// Numbered packets that match.
pub const NUMBERED_GOOD: Field<field::I64> = Field::new("numbered_good", "Test packets good", "");
/// Numbered packets that do not match.
pub const NUMBERED_WRONG: Field<field::I64> =
    Field::new("numbered_wrong", "Test packets wrong", "");
/// Breaks in the numbering.
pub const NUMBERED_GAPS: Field<field::I64> = Field::new("numbered_gaps", "Test packet gaps", "");
/// Bit rate over the last second.
pub const BITRATE_BPS: Field<field::F64> = Field::new("bitrate_bps", "Bit rate", "bit/s");
/// A PAT was seen.
pub const PAT_SEEN: Field<field::Bool> = Field::new("pat_seen", "PAT seen", "");
/// The service names of the SDT, one per line.
pub const SERVICES: Field<field::Text> = Field::new("services", "Services", "");

/// The result fields of `mpegts.ts_monitor`: the counts of the transport stream, which a stack shows under its own keys.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsMonitorFields<'a> {
    /// Packets seen.
    #[field(shared = PACKETS)]
    pub packets: i64,
    /// Packets with the transport_error_indicator.
    #[field(shared = FLAGGED)]
    pub flagged: i64,
    /// Scrambled packets.
    #[field(shared = SCRAMBLED)]
    pub scrambled: i64,
    /// Numbered packets that match.
    #[field(shared = NUMBERED_GOOD)]
    pub numbered_good: i64,
    /// Numbered packets that do not match.
    #[field(shared = NUMBERED_WRONG)]
    pub numbered_wrong: i64,
    /// Breaks in the numbering.
    #[field(shared = NUMBERED_GAPS)]
    pub numbered_gaps: i64,
    /// Bit rate over the last second.
    #[field(shared = BITRATE_BPS)]
    pub bitrate_bps: f64,
    /// A PAT was seen.
    #[field(shared = PAT_SEEN)]
    pub pat_seen: bool,
    /// The service names of the SDT, one per line.
    #[field(shared = SERVICES)]
    pub services: &'a str,
}

/// The results that `mpegts.ts_monitor` publishes, for a stack's results declaration.
pub fn results_decl() -> ResultsDecl {
    ResultsDecl::new().with_fields(TsMonitorFields::SHAPE.fields)
}

/// `shape` with the result set [`TsMonitorFields`] as a set of the stack, written by the monitor that the graph of the
/// stack holds under the name `component`. Call it after the result sets of the stack that come before the counts of
/// the monitor.
#[must_use]
pub fn describe(shape: StackShape, component: &'static str) -> StackShape {
    shape
        .fields::<TsMonitorFields<'static>>()
        .component(component, |c| {
            c.fields::<TsMonitorFields<'static>>().shares_declared()
        })
}

sdrkit_core::choice_enum! {
    /// The fill of the numbered test packets that the monitor checks.
    pub enum CheckFill {
        /// No check.
        Off = ("off", "Off"),
        /// 7 n + 13 i (satellite, cable, ISDB-T, ATSC).
        N7I13 = ("n7i13", "7 n + 13 i (satellite, cable, ISDB-T, ATSC)"),
        /// 31 i + 7 n (DVB-T).
        I31N7 = ("i31n7", "31 i + 7 n (DVB-T)"),
    }
}

impl CheckFill {
    fn fill(self) -> Option<Fill> {
        match self {
            CheckFill::Off => None,
            CheckFill::N7I13 => Some(Fill::N7I13),
            CheckFill::I31N7 => Some(Fill::I31N7),
        }
    }
}

const CHECK_PID: Setting<field::I64> =
    Setting::new("check_pid", "Test packet PID", "", 32).range(0.0, 8191.0, 1.0);
const CHECK_FILL: Setting<field::Choice<CheckFill>> =
    Setting::choice("check_fill", "Test packet fill", CheckFill::N7I13);

sdrkit_core::settings! {
    /// The settings of [`TsMonitorComponent`].
    pub enum TsMonitorSetting {
        /// The PID of the numbered test packets.
        CheckPid = CHECK_PID,
        /// The fill of the numbered test packets.
        CheckFill = CHECK_FILL,
    }
}

/// Forgets every count.
pub const CLEAR: Command = Command::new("clear", "Clear").help("Forgets every count.");

sdrkit_core::commands! {
    /// The commands of [`TsMonitorComponent`].
    pub enum TsMonitorCommand {
        /// Forgets every count.
        Clear = CLEAR,
    }
}

/// `mpegts.ts_monitor`: [`TsMonitor`] as a component.
pub struct TsMonitorComponent {
    inputs: usize,
    mon: TsMonitor,
    pid: u16,
    fill: CheckFill,
    published: Published,
    services_shown: usize,
    services: String,
}

impl Default for TsMonitorComponent {
    fn default() -> Self {
        TsMonitorComponent::new(1)
    }
}

impl TsMonitorComponent {
    /// A monitor of `inputs` transport streams (at least 1), merged in input order.
    pub fn new(inputs: usize) -> Self {
        TsMonitorComponent {
            inputs: inputs.max(1),
            mon: TsMonitor::new(Some(Numbered::new(CHECK_PID.default as u16, Fill::N7I13))),
            pid: CHECK_PID.default as u16,
            fill: CHECK_FILL.default,
            published: Published::new(TsMonitorFields::SHAPE.fields),
            services_shown: usize::MAX,
            services: String::new(),
        }
    }

    /// The monitor.
    pub fn monitor(&self) -> &TsMonitor {
        &self.mon
    }

    fn publish(&mut self) {
        let s = self.mon.stats();
        if s.services.len() != self.services_shown {
            self.services_shown = s.services.len();
            self.services.clear();
            for (i, name) in s.services.iter().enumerate() {
                if i > 0 {
                    self.services.push('\n');
                }
                self.services.push_str(name);
            }
        }
        self.published.put(&TsMonitorFields {
            packets: s.packets as i64,
            flagged: s.flagged as i64,
            scrambled: s.scrambled as i64,
            numbered_good: s.numbered.good as i64,
            numbered_wrong: s.numbered.wrong as i64,
            numbered_gaps: s.numbered.gaps as i64,
            bitrate_bps: s.bitrate_bps,
            pat_seen: s.pat_seen,
            services: &self.services,
        });
    }
}

impl Component for TsMonitorComponent {
    fn info(&self) -> ComponentInfo {
        let ins = (0..self.inputs)
            .map(|i| {
                stream::port(&if i == 0 {
                    "in".to_string()
                } else {
                    format!("in{i}")
                })
            })
            .collect();
        ComponentInfo::processor(
            "mpegts.ts_monitor",
            ins,
            vec![stream::port("out").same_format_as(0)],
        )
        .with_settings(TsMonitorSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }

    fn process(&mut self, io: &mut Io) {
        let mut at = io.output(0).first();
        for i in 0..self.inputs {
            let input = io.input(i);
            let first = input.first();
            let lost: [Option<u64>; 4] = {
                let mut l = [None; 4];
                for (k, t) in input
                    .tags()
                    .iter()
                    .filter(|t| matches!(t.kind, TagKind::Discontinuity { .. }))
                    .take(4)
                    .enumerate()
                {
                    l[k] = Some(t.at);
                }
                l
            };
            let items = io.items::<TsPacket>(i);
            for (k, it) in items.iter().enumerate() {
                if lost.contains(&Some(first + k as u64)) {
                    io.output(0)
                        .push_tag(at, TagKind::Discontinuity { lost: 0 });
                }
                self.mon.push(it.value.bytes(), it.time);
                io.items_out::<TsPacket>(0).push(it.clone());
                at += 1;
            }
        }
        self.publish();
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TsMonitorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TsMonitorSetting::CheckPid => self.pid = CHECK_PID.read(id, v)? as u16,
            TsMonitorSetting::CheckFill => self.fill = CHECK_FILL.read(id, v)?,
        }
        self.mon
            .set_pattern(self.fill.fill().map(|f| Numbered::new(self.pid, f)));
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (TsMonitorSetting::CheckPid, Value::I64(i64::from(self.pid))),
            (
                TsMonitorSetting::CheckFill,
                Value::Choice(self.fill.index()),
            ),
        ])
    }

    fn commands(&self) -> &Commands {
        TsMonitorCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match TsMonitorCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TsMonitorCommand::Clear => {
                self.mon.clear();
                self.services_shown = usize::MAX;
                self.publish();
            }
        }
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

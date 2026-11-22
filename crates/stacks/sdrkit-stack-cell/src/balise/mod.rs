//! The Eurobalise and EBICab up-link stack (`ebicab`): telegrams around 4.234 MHz, and the bursts of other balise systems
//! (listed, not decoded).
//!
//! Standard: UNISIG SUBSET-036.

pub mod gen;
pub mod telegram;
pub mod uplink;

use crate::decoder::WallClock;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::publish::keys::Family;
use crate::typed::{
    EbicabBaliseRow, EbicabFields, EbicabOtherBurstRow, EbicabRemoveArgs, EbicabTelegramEvent, EbicabTelegramRow,
    EBICAB_BALISES, EBICAB_OTHER_BURSTS, EBICAB_REMOVE, EBICAB_TELEGRAM, EBICAB_TELEGRAMS,
};
use sdrkit_core::interface::{Fields, Hertz, Seconds, Time};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, EventDef, GenConfig, Generator, LockState, Receiver, Record,
    ResultsDecl, Schema, StackConfig, StageError, StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Kept, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::rows::remove_row;
use sdrkit_util_objects::{Format as StateFormat, LogLine, LogRing, ObjectTable, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use telegram::{Header, StreamReceiver};
use uplink::{BaliseBurst, Uplink, UplinkItem};

/// The stack id.
pub const ID: &str = "ebicab";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 4e6;
/// The most telegram rows kept.
pub const TELEGRAM_CAP: usize = 200;
/// The most balises kept.
pub const BALISE_CAP: usize = 100;
/// The most other bursts kept.
pub const OTHER_CAP: usize = 100;
/// The most lines the event log keeps.
pub const LOG_LINES: usize = 300;
/// A telegram (or a burst) within this many seconds of signal sets the lock state.
pub const RECENT_S: f64 = 30.0;
/// A burst shorter than this many bits is noise: it sets no bit rate and is not listed.
const LEAST_BITS: usize = 24;
/// The first bits of an other burst that are kept.
const HEAD_BITS: usize = 256;
/// The line the log starts with.
const LISTENING: &str = "Balise: listening for the Eurobalise up-link (FSK around 4.234 MHz, 564.48 kbit/s); EBICab formats are not decoded";

/// A telegram row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelegramRow {
    /// Wall time, seconds since 1970, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// Signal time of its burst, seconds.
    pub t: f64,
    /// 1023 or 341.
    pub length: u32,
    /// The bits were inverted.
    pub inverted: bool,
    /// The header fields.
    #[serde(flatten)]
    pub header: Header,
    /// The user bits as hex.
    pub user_bits_hex: String,
    /// Copies in this burst.
    pub copies: u32,
    /// SNR of the burst, dB.
    pub snr_db: f64,
    #[serde(skip)]
    burst: u64,
}

impl TelegramRow {
    /// The key of its balise.
    fn balise(&self) -> BaliseKey {
        BaliseKey {
            nid_c: self.header.nid_c,
            nid_bg: self.header.nid_bg,
            n_pig: self.header.n_pig,
        }
    }
    /// The log line of the telegram.
    fn line(&self) -> String {
        let kind = if self.length == 1023 { "long" } else { "short" };
        format!(
            "Balise: {kind} telegram, NID_C {} NID_BG {}",
            self.header.nid_c, self.header.nid_bg
        )
    }
}

impl Item for TelegramRow {
    const KIND: &'static str = "balise.telegram";
    fn event_def() -> Option<&'static EventDef> {
        Some(EBICAB_TELEGRAM.def())
    }
    fn write_fields(&self, fields: &mut Record) {
        EbicabTelegramEvent {
            source: &self.balise().to_string(),
            text: &self.line(),
            family: Family::Other,
            length_bits: i64::from(self.length),
            nid_c: i64::from(self.header.nid_c),
            nid_bg: i64::from(self.header.nid_bg),
            n_pig: i64::from(self.header.n_pig),
        }
        .write(fields);
    }
}

impl Shift for TelegramRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The key of a balise: country, group, and place in the group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BaliseKey {
    /// NID_C.
    pub nid_c: u16,
    /// NID_BG.
    pub nid_bg: u16,
    /// N_PIG.
    pub n_pig: u8,
}

impl std::fmt::Display for BaliseKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.nid_c, self.nid_bg, self.n_pig)
    }
}

/// A balise row. When it was heard, and how often, is the row's [`Seen`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaliseRow {
    /// NID_C.
    pub nid_c: u16,
    /// NID_BG.
    pub nid_bg: u16,
    /// N_PIG (raw).
    pub n_pig: u8,
    /// N_TOTAL.
    pub n_total: u8,
    /// N_TOTAL + 1.
    pub group_size_shown: u8,
    /// M_VERSION.
    pub m_version: u8,
    /// M_VERSION as "major.minor".
    pub m_version_shown: String,
}

impl Shift for BaliseRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

sdrkit_core::choice_enum! {
    /// What the length of a burst without a telegram suggests.
    #[derive(Serialize, Deserialize)]
    pub enum BurstHint {
        /// About the length of an EBICab 700 telegram (32 bits).
        #[serde(rename = "about the length of an EBICab 700 telegram (32 bits)")]
        Ebicab700 = ("ebicab_700", "About the length of an EBICab 700 telegram (32 bits)"),
        /// About the length of an EBICab 900 telegram (255 bits).
        #[serde(rename = "about the length of an EBICab 900 telegram (255 bits)")]
        Ebicab900 = ("ebicab_900", "About the length of an EBICab 900 telegram (255 bits)"),
        /// The length of a short Eurobalise telegram, but no valid telegram.
        #[serde(rename = "short Eurobalise telegram length, but no valid telegram (errors?)")]
        ShortEurobalise = ("short_eurobalise", "Short Eurobalise telegram length, but no valid telegram"),
        /// The length of a long Eurobalise telegram, but no valid telegram.
        #[serde(rename = "long Eurobalise telegram length, but no valid telegram (errors?)")]
        LongEurobalise = ("long_eurobalise", "Long Eurobalise telegram length, but no valid telegram"),
    }
}

/// A burst that produced no telegram.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OtherBurst {
    /// Signal time, seconds.
    pub t: f64,
    /// Wall time, seconds since 1970, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// Duration, microseconds.
    pub duration_us: f64,
    /// Bits.
    pub bits: usize,
    /// The first bits as hex.
    pub head_hex: String,
    /// What the length suggests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<BurstHint>,
    /// SNR, dB.
    pub snr_db: f64,
}

impl Shift for OtherBurst {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters, and the measures of the newest burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BaliseCounters {
    /// Bursts.
    pub bursts: u64,
    /// Telegrams (every decode).
    pub telegrams: u64,
    /// Long telegrams.
    pub long: u64,
    /// Short telegrams.
    pub short: u64,
    /// Inverted telegrams.
    pub inverted: u64,
    /// Bursts of 24 bits or more without a telegram.
    pub bursts_without_telegram: u64,
    /// Parity hits.
    pub parity_hits: u64,
    /// The bit rate of the newest burst of 24 bits or more, bit/s.
    pub bit_rate_hz: f64,
    /// Its carrier offset, Hz.
    pub carrier_offset_hz: f64,
    /// Its SNR, dB.
    pub snr_db: Option<f64>,
    /// Signal time of the newest burst, seconds.
    pub last_burst_s: Option<f64>,
    /// Signal time of the newest telegram, seconds.
    pub last_telegram_s: Option<f64>,
}

impl Shift for BaliseCounters {
    fn shift_times(&mut self, by_s: f64) {
        for t in [&mut self.last_burst_s, &mut self.last_telegram_s]
            .into_iter()
            .flatten()
        {
            *t += by_s;
        }
    }
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaliseHeld {
    /// Telegram rows, newest last.
    pub telegrams: LogRing<TelegramRow>,
    /// The balises heard.
    pub balises: ObjectTable<BaliseKey, BaliseRow>,
    /// Other bursts, newest last.
    pub other_bursts: LogRing<OtherBurst>,
    /// The event log, newest last.
    pub log: LogRing<LogLine>,
    /// The counters.
    pub counters: BaliseCounters,
}

impl Default for BaliseHeld {
    fn default() -> Self {
        BaliseHeld {
            telegrams: LogRing::new(TELEGRAM_CAP),
            balises: ObjectTable::new(BALISE_CAP),
            other_bursts: LogRing::new(OTHER_CAP),
            log: LogRing::new(LOG_LINES),
            counters: BaliseCounters::default(),
        }
    }
}

impl Shift for BaliseHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.telegrams.shift_times(by_s);
        self.balises.shift_times(by_s);
        self.other_bursts.shift_times(by_s);
        self.log.shift_times(by_s);
        self.counters.shift_times(by_s);
    }
}

impl Tables for BaliseHeld {
    const FORMAT: StateFormat = StateFormat::new("cell.balise_telegrams", 1);
    fn clear(&mut self) {
        self.telegrams.clear();
        self.balises.clear();
        self.other_bursts.clear();
        self.log.clear();
        self.counters = BaliseCounters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.telegrams.adopt(saved.telegrams);
        self.balises.adopt(saved.balises);
        self.other_bursts.adopt(saved.other_bursts);
        self.log.adopt(saved.log);
        self.counters = saved.counters;
    }
}

/// Returns the hint that the length suggests for a burst of `bits` bits that produced no telegram.
pub fn hint(bits: usize) -> Option<BurstHint> {
    match bits {
        26..=40 => Some(BurstHint::Ebicab700),
        235..=275 => Some(BurstHint::Ebicab900),
        300..=360 => Some(BurstHint::ShortEurobalise),
        b if b >= 900 => Some(BurstHint::LongEurobalise),
        _ => None,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Packs bits given as hexadecimal digits into bytes.
fn hex_bytes(hex: &str) -> std::sync::Arc<[u8]> {
    let digit = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
    hex.as_bytes()
        .chunks(2)
        .map(|p| digit(p[0]) << 4 | p.get(1).map_or(0, |&c| digit(c)))
        .collect()
}

impl Shaped for TelegramRow {
    type Row = EbicabTelegramRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let h = &self.header;
        out.put(&EbicabTelegramRow {
            received: self.wall.map(Time),
            length_bits: i64::from(self.length),
            nid_c: i64::from(h.nid_c),
            nid_bg: i64::from(h.nid_bg),
            n_pig: i64::from(h.n_pig),
            n_total: i64::from(h.n_total),
            m_version: i64::from(h.m_version),
            copies: i64::from(self.copies),
            user_bits: hex_bytes(&self.user_bits_hex),
        });
    }
}

impl Shaped for BaliseRow {
    type Row = EbicabBaliseRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EbicabBaliseRow {
            nid_c: i64::from(self.nid_c),
            nid_bg: i64::from(self.nid_bg),
            balise_in_group: i64::from(self.n_pig) + 1,
            group_size: i64::from(self.group_size_shown),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
}

impl Shaped for OtherBurst {
    type Row = EbicabOtherBurstRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EbicabOtherBurstRow {
            received: self.wall.map(Time),
            bits: self.bits as i64,
            duration: Seconds(self.duration_us * 1e-6),
            snr_db: self.snr_db,
            first_bits: hex_bytes(&self.head_hex),
            hint: self.hint,
        });
    }
}

/// The `balise.telegrams` decoder: the telegram receiver over the bits of each burst, and the tables, log and counters.
pub struct BaliseTelegrams {
    rx: StreamReceiver,
    held: BaliseHeld,
    pending_log: Vec<String>,
    clock: WallClock,
    /// The tables changed since the last report.
    dirty: bool,
    /// The results log must be emptied at the next report.
    cleared: bool,
    now: f64,
    burst_no: u64,
    /// Telegram rows added by the burst being read.
    fresh: usize,
}

impl Default for BaliseTelegrams {
    fn default() -> Self {
        BaliseTelegrams {
            rx: StreamReceiver::default(),
            held: BaliseHeld::default(),
            pending_log: vec![LISTENING.to_string()],
            clock: WallClock::default(),
            dirty: true,
            cleared: false,
            now: 0.0,
            burst_no: 0,
            fresh: 0,
        }
    }
}

impl BaliseTelegrams {
    /// The tables, the event log and the counters.
    pub fn held(&self) -> &BaliseHeld {
        &self.held
    }

    fn line(&mut self, t: f64, text: String) {
        self.pending_log.push(text.clone());
        self.held.log.push(LogLine {
            at: self.clock.stamp(t),
            text,
        });
    }

    fn burst(&mut self, b: &BaliseBurst) {
        self.burst_no += 1;
        self.rx.new_burst();
        let before = self.rx.parity_hits;
        let mut tels = Vec::new();
        for &bit in &b.bits {
            self.rx.push(bit, &mut tels);
        }
        self.dirty = true;
        let c = &mut self.held.counters;
        c.bursts += 1;
        c.parity_hits += self.rx.parity_hits - before;
        c.last_burst_s = Some(b.time);
        if b.bits.len() >= LEAST_BITS {
            c.bit_rate_hz = b.bit_rate_hz;
            c.carrier_offset_hz = b.carrier_offset_hz;
            c.snr_db = Some(b.snr_db);
        }
        for t in &tels {
            let c = &mut self.held.counters;
            c.telegrams += 1;
            match t.format {
                telegram::Format::Long => c.long += 1,
                telegram::Format::Short => c.short += 1,
            }
            if t.inverted {
                c.inverted += 1;
            }
            c.last_telegram_s = Some(b.time);
            let Some(h) = telegram::header(&t.user) else {
                continue;
            };
            let hex = telegram::hex(&t.user);
            let burst_no = self.burst_no;
            let same = self
                .held
                .telegrams
                .newest_mut()
                .filter(|r| r.burst == burst_no && r.user_bits_hex == hex);
            match same {
                Some(r) => r.copies += 1,
                None => {
                    let row = TelegramRow {
                        wall: self.clock.wall(b.time),
                        t: b.time,
                        length: t.format.n() as u32,
                        inverted: t.inverted,
                        header: h.clone(),
                        user_bits_hex: hex,
                        copies: 1,
                        snr_db: b.snr_db,
                        burst: burst_no,
                    };
                    self.line(b.time, row.line());
                    self.fresh += 1;
                    self.held.telegrams.push(row);
                }
            }
            let key = BaliseKey {
                nid_c: h.nid_c,
                nid_bg: h.nid_bg,
                n_pig: h.n_pig,
            };
            let row = BaliseRow {
                nid_c: h.nid_c,
                nid_bg: h.nid_bg,
                n_pig: h.n_pig,
                n_total: h.n_total,
                group_size_shown: h.n_total + 1,
                m_version: h.m_version,
                m_version_shown: telegram::version_text(h.m_version),
            };
            // A time that is not finite is refused, and the balise is then left out of the table only.
            if let Ok(up) = self
                .held
                .balises
                .upsert(&key, self.clock.stamp(b.time), || row.clone())
            {
                *up.row = row;
            }
        }
        if tels.is_empty() && b.bits.len() >= LEAST_BITS {
            self.held.counters.bursts_without_telegram += 1;
            let head = &b.bits[..b.bits.len().min(HEAD_BITS)];
            self.held.other_bursts.push(OtherBurst {
                t: b.time,
                wall: self.clock.wall(b.time),
                duration_us: b.duration_us,
                bits: b.bits.len(),
                head_hex: telegram::hex(head),
                hint: hint(b.bits.len()),
                snr_db: b.snr_db,
            });
        }
    }
}

sdrkit_core::commands! {
    /// The commands of [`BaliseTelegrams`].
    pub enum TelegramsCommand {
        /// Forgets one balise.
        Remove = EBICAB_REMOVE.command,
    }
}

impl Decoder for BaliseTelegrams {
    type Input = Timed<UplinkItem>;
    fn name(&self) -> &'static str {
        "balise.telegrams"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<TelegramRow>("telegrams")]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rx.new_burst();
        Ok(vec![Format::new(Kind::typed::<TelegramRow>(), 0.0)])
    }
    fn process(&mut self, x: &[Timed<UplinkItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.now = self.now.max(it.time);
            if let UplinkItem::Burst(b) = &it.value {
                self.fresh = 0;
                self.burst(b);
                let skip = self.held.telegrams.len() - self.fresh.min(self.held.telegrams.len());
                out.items::<TelegramRow>(0)
                    .extend(self.held.telegrams.iter().skip(skip).map(|r| Timed {
                        time: r.t,
                        value: r.clone(),
                    }));
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let recent = |t: Option<f64>| t.is_some_and(|t| now - t <= RECENT_S);
        let lock_state = {
            let c = &self.held.counters;
            if recent(c.last_telegram_s) {
                LockState::Locked
            } else if recent(c.last_burst_s) {
                LockState::Tracking
            } else {
                LockState::Searching
            }
        };
        let dirty = std::mem::take(&mut self.dirty);
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        for line in self.pending_log.drain(..) {
            p.results_mut().log(line);
        }
        if !dirty && p.results().status().lock_state == lock_state {
            return;
        }
        let h = &self.held;
        let c = &h.counters;
        if dirty {
            let measured = c.snr_db.is_some();
            p.put(&EbicabFields {
                bursts: c.bursts as i64,
                telegrams: c.telegrams as i64,
                long: c.long as i64,
                short: c.short as i64,
                inverted: c.inverted as i64,
                bursts_without_telegram: c.bursts_without_telegram as i64,
                parity_hits: c.parity_hits as i64,
                bit_rate_hz: measured.then_some(Hertz(c.bit_rate_hz)),
                offset_hz: measured.then_some(Hertz(c.carrier_offset_hz)),
                snr_db: c.snr_db,
            });
            shaped::log_rows(&h.telegrams, &EBICAB_TELEGRAMS, now, p);
            shaped::rows(&h.balises, &EBICAB_BALISES, now, p);
            shaped::log_rows(&h.other_bursts, &EBICAB_OTHER_BURSTS, now, p);
        }
        let summary = if c.telegrams > 0 {
            format!(
                "Eurobalise: {} telegrams ({} long, {} short)",
                c.telegrams, c.long, c.short
            )
        } else if c.bursts > 0 {
            format!("Balise band: {} bursts, no telegram decoded", c.bursts)
        } else {
            "Balise: no signal".into()
        };
        let measured = c.snr_db.is_some();
        let offset = measured.then_some(c.carrier_offset_hz);
        let snr = c.snr_db.map(|s| s as f32);
        p.results_mut().update_status(|s| {
            let same = s.lock_state == lock_state
                && s.summary == summary
                && s.offset_hz == offset
                && s.snr_db == snr;
            if !same {
                s.lock_state = lock_state;
                s.summary = summary;
                s.offset_hz = offset;
                s.snr_db = snr;
            }
            !same
        });
    }
    fn commands(&self) -> &'static [&'static Command] {
        TelegramsCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TelegramsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TelegramsCommand::Remove => {
                let row = EBICAB_REMOVE.args::<EbicabRemoveArgs>(args)?.row;
                if !remove_row(&mut self.held.balises, row) {
                    return Err(self.results().no_such_row(EBICAB_BALISES.def(), row));
                }
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        self.held.clear();
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
    }
    /// Keeps the telegrams, the balises, the other bursts and their counts while the stack keeps the telegram table;
    /// forgets the measurements of the newest burst.
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(EBICAB_TELEGRAMS.key) {
            self.clear();
        }
        self.rx.new_burst();
        let c = &mut self.held.counters;
        c.snr_db = None;
        c.last_burst_s = None;
        c.last_telegram_s = None;
        self.dirty = true;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock.attach(ctx.wall_clock(), &ctx.clock, self.now);
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
        for l in self.held.log.iter() {
            self.pending_log.push(l.text.clone());
        }
        Ok(())
    }
}

/// The settings of the `ebicab` stack: the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The Eurobalise receiver: the up-link front end and the telegram decoder, with the `clear` command.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("uplink", DecoderComponent::new(Uplink::default()))
        .add(
            "telegrams",
            DecoderComponent::new(BaliseTelegrams::default()),
        )
        .input("uplink")
        .connect("uplink", "telegrams")
        .status_from("telegrams")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE,
            "Balise: the sample rate is too low (4 Msps at least)",
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("telegrams", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the generator of the test signal settings.
///
/// # Errors
///
/// [`ConfigError::Stage`] if the modulator cannot serve the rate.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        gen::BaliseGenerator::from_config(cfg)
            .map_err(|e| ConfigError::Stage(StageError::new(e)))?,
    ))
}

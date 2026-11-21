//! The frame table component (`satimg.frame_table`) of links whose frames are listed but not decoded: soft symbols to
//! frames behind a fixed sync word, with an optional K = 7 code, randomiser and Reed-Solomon code, and the ids of a CCSDS
//! header.
//!
//! Standard: CCSDS 131.0-B-5 (K = 7 code, randomiser, Reed-Solomon, virtual fill); CCSDS 132.0-B-3 and 732.0-B-4 (frame
//! headers).

use super::link::{control_port, item_format, item_port, stage};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::{ConfigError, Record, SettingError, SettingId, Value, Values};
use sdrkit_graph::{
    no_values, Applied, Component, ComponentInfo, Format, Io, Item, Pool, Port, RunContext, Ticket, Timed,
};
use sdrkit_proto_ccsds::channel::{ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::codesync::{CodeConfig, CodeLock, CodeSync};
use sdrkit_proto_ccsds::multisync::{QuadSync, StreamEvent};
use sdrkit_proto_ccsds::quaddiff::QuadDiffDecoder;
use sdrkit_proto_ccsds::wordsync::{WordFrame, WordSync, WordSyncConfig};
use sdrkit_util_bits::line::NrzM;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// Rows of the frame table, and frames in one report at the most.
pub const TABLE_ROWS: usize = 64;
/// Bytes of a frame that the table shows.
pub const HEX_BYTES: usize = 24;
/// Seconds without a good frame after which a coded link searches its code again.
pub const CODE_TIMEOUT_S: f64 = 6.0;

/// The `clear` command of a frame table: the rows and the counters go, the synchronisation stays.
pub const CLEAR: Command = Command::new("clear", "Clear")
    .help("Empties the frame table and the counters; the synchronisation stays.");

sdrkit_core::commands! {
    /// The commands of the frame table: its component and its display.
    pub enum TableCommand {
        /// Empties the table and the counters.
        Clear = CLEAR,
    }
}

/// How the symbols become bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// One bit a symbol as it is.
    Nrz,
    /// One bit a symbol, NRZ-M: a 1 is a change of the level.
    NrzM,
    /// Uncoded QPSK: two bits a symbol, I then Q, read as the four streams of [`QuadSync`].
    Quad,
    /// Uncoded QPSK with four-phase differential coding: two bits a symbol, I then Q, decoded by [`QuadDiffDecoder`].
    QuadDiff,
    /// The rate 1/2, K = 7 code: code symbols to bits by [`CodeSync`] on the sync word of the link, which also undoes NRZ-M
    /// when the configuration says so.
    Coded(CodeConfig),
    /// Uncoded offset QPSK or BPSK with NRZ-M: the hard bits under each sign pattern of the I and Q rails (BPSK: one), NRZ-M
    /// decoded on each rail apart (`per_rail`) or on the whole stream, each into its own synchroniser; the first pattern that
    /// holds a confirmed lock gives the frames.
    Rails {
        /// NRZ-M on each rail before the rails are interleaved.
        per_rail: bool,
        /// Two rails (I then Q of each symbol).
        quad: bool,
    },
}

/// The rules of a link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableLink {
    /// The frame synchroniser.
    pub sync: WordSyncConfig,
    /// How the symbols become bits.
    pub line: Line,
    /// The randomiser and the Reed-Solomon code of the bytes after the sync word, if any.
    pub code: Option<ChannelConfig>,
    /// The randomiser covers the data bytes inside the Reed-Solomon code: it is removed from the data after decoding.
    pub data_randomised: bool,
    /// Every byte of a frame, the sync word too, is inverted before anything else.
    pub invert: bool,
    /// A frame whose check fails is tried again without the randomiser.
    pub retry_plain: bool,
    /// Soft values a second at the input (symbols times bits a symbol).
    pub llr_rate: f64,
}

/// The check of a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Check {
    /// Every code word decoded.
    #[default]
    Good,
    /// A code word did not decode.
    Failed,
    /// The link has no check.
    None,
}

impl Check {
    /// The text of the table.
    pub fn text(self) -> &'static str {
        match self {
            Check::Good => "good",
            Check::Failed => "check failed",
            Check::None => "none",
        }
    }
}

/// The ids a CCSDS frame header gives: (AOS, spacecraft, virtual channel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderIds {
    /// An AOS header (version 01); otherwise a TM header (version 00).
    pub aos: bool,
    /// The spacecraft id: 8 bits (AOS) or 10 bits (TM).
    pub spacecraft: u16,
    /// The virtual channel: 6 bits (AOS) or 3 bits (TM).
    pub vcid: u8,
}

/// The ids of the two bytes `h0`, `h1` after the sync word: an AOS header when the version bits are 01, a TM header when
/// they are 00, nothing else.
pub fn header_ids(h0: u8, h1: u8) -> Option<HeaderIds> {
    match h0 >> 6 {
        1 => Some(HeaderIds {
            aos: true,
            spacecraft: u16::from(h0 & 0x3F) << 2 | u16::from(h1 >> 6),
            vcid: h1 & 0x3F,
        }),
        0 => Some(HeaderIds {
            aos: false,
            spacecraft: u16::from(h0 & 0x3F) << 4 | u16::from(h1 >> 4),
            vcid: (h1 >> 1) & 7,
        }),
        _ => None,
    }
}

/// One row of the frame table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameRow {
    /// Signal time at the end of the frame, s.
    pub time_s: f64,
    /// The header ids.
    pub ids: Option<HeaderIds>,
    /// Bytes of the frame with its sync word.
    pub bytes: u32,
    /// The check.
    pub check: Check,
    /// The first bytes after decoding, upper-case hexadecimal.
    pub hex: String,
}

/// A good frame after decoding, with its signal time: the sync word, then the data bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameBytes {
    /// Signal time at the end of the frame, s.
    pub time_s: f64,
    /// The bytes.
    pub bytes: Arc<[u8]>,
}
impl Item for FrameBytes {
    const KIND: &'static str = "satimg.frame";
}

/// Counters of [`FrameTableComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableStats {
    /// Good frames.
    pub good: u64,
    /// Frames whose check failed.
    pub bad: u64,
    /// Reed-Solomon symbols corrected.
    pub corrected: u64,
    /// Sync words found.
    pub sync_hits: u64,
    /// Good frames for each virtual channel of a header.
    pub per_vc: BTreeMap<u8, u64>,
}

/// The report of [`FrameTableComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableReport {
    /// Counters.
    pub stats: TableStats,
    /// A synchroniser holds a confirmed lock.
    pub locked: bool,
    /// The stream in use is inverted.
    pub inverted: bool,
    /// The QPSK stream in use, 0 to 3.
    pub stream: Option<u8>,
    /// The ids of the last good frame.
    pub ids: Option<HeaderIds>,
    /// The code synchroniser of a coded link.
    pub code: Option<CodeLock>,
    /// Frames since the last report, oldest first, at most [`TABLE_ROWS`].
    pub rows: Vec<FrameRow>,
    /// Good frames since the last report.
    pub good_now: u64,
    /// Seconds of signal since the last good frame.
    pub since_good_s: f64,
    /// New log lines.
    pub log: Vec<String>,
}
impl Item for TableReport {
    const KIND: &'static str = "satimg.table_report";
}

/// The frame state that the `clear` command resets.
#[derive(Default)]
struct Counters {
    stats: TableStats,
    ids: Option<HeaderIds>,
    rows: Vec<FrameRow>,
}

impl Counters {
    fn new_picture(&mut self) {
        *self = Counters::default();
    }
}

/// No code word of the frame took more corrections than the code guarantees: (n - k) / 2 bytes. `received` and `decoded` are
/// the interleaved code words before and after decoding; byte i belongs to code word i mod the depth.
fn within_bound(cfg: &ChannelConfig, received: &[u8], decoded: &[u8]) -> bool {
    let Some(rs) = cfg.rs else { return true };
    let bound = usize::from(rs.n - rs.k) / 2;
    let depth = cfg.depth.max(1);
    let mut changed = vec![0usize; depth];
    for (i, (a, b)) in received.iter().zip(decoded).enumerate() {
        changed[i % depth] += usize::from(a != b);
    }
    changed.iter().all(|&c| c <= bound)
}

/// One sign pattern of [`Line::Rails`]: the signs of the I and Q rails, the NRZ-M decoders (one, or one a rail) and its
/// synchroniser.
struct Pattern {
    signs: [u8; 2],
    nrzm: [NrzM; 2],
    sync: WordSync,
}

enum Sync {
    One(Box<WordSync>, Option<NrzM>),
    Rails(Vec<Pattern>, Option<usize>),
    Quad(Box<QuadSync>),
    QuadDiff(Box<WordSync>, QuadDiffDecoder),
    Coded(Box<CodeSync>, Box<WordSync>),
}

/// The work of [`FrameTableComponent`] and its state.
struct Table {
    link: TableLink,
    sync: Option<Sync>,
    channel: Option<ChannelDecoder>,
    bits: Vec<u8>,
    received: Vec<u8>,
    found: Vec<(Vec<u8>, bool)>,
    plain: Option<ChannelDecoder>,
    llrs: u64,
    last_good: u64,
    code_lock: CodeLock,
    good_reported: u64,
    dec: Counters,
    log: Vec<String>,
}

impl Table {
    fn new(link: TableLink) -> Self {
        Table {
            link,
            sync: None,
            channel: None,
            bits: Vec::new(),
            received: Vec::new(),
            found: Vec::new(),
            plain: None,
            llrs: 0,
            last_good: 0,
            code_lock: CodeLock::default(),
            good_reported: 0,
            dec: Counters::default(),
            log: Vec::new(),
        }
    }

    /// Takes one frame as the synchroniser gave it (sync word included) and returns its row and, for a good frame, its bytes.
    fn push_frame(&mut self, frame: &[u8], time_s: f64) -> (FrameRow, Option<Vec<u8>>) {
        let mut b = frame.to_vec();
        if self.link.invert {
            b.iter_mut().for_each(|x| *x = !*x);
        }
        let mut check = Check::None;
        if let Some(c) = &mut self.channel {
            let coded = c.config().coded_len();
            let n = c.config().frame_len();
            check = Check::Failed;
            if b.len() >= 4 + coded {
                let cfg = *c.config();
                // The received word, without the randomiser, to count the corrections of each code word.
                self.received.clear();
                self.received.extend_from_slice(&b[4..4 + coded]);
                if cfg.randomise {
                    sdrkit_proto_ccsds::randomiser::tm_bytes(&mut self.received);
                }
                let keep = self.link.retry_plain.then(|| b.clone());
                if let Ok(d) = c.decode(&mut b[4..4 + coded]) {
                    if d.ok() && within_bound(&cfg, &self.received, &b[4..4 + coded]) {
                        check = Check::Good;
                        self.dec.stats.corrected += u64::from(d.corrected);
                    }
                }
                if let (Check::Failed, Some(orig), Some(p)) = (check, keep, &mut self.plain) {
                    // The same frame without the randomiser.
                    b = orig;
                    self.received.clear();
                    self.received.extend_from_slice(&b[4..4 + coded]);
                    if let Ok(d) = p.decode(&mut b[4..4 + coded]) {
                        if d.ok() && within_bound(p.config(), &self.received, &b[4..4 + coded]) {
                            check = Check::Good;
                            self.dec.stats.corrected += u64::from(d.corrected);
                        }
                    }
                }
                b.truncate(4 + n);
                if self.link.data_randomised {
                    sdrkit_proto_ccsds::randomiser::tm_bytes(&mut b[4..]);
                }
            }
        }
        let good = check != Check::Failed;
        let ids = (b.len() >= 6).then(|| header_ids(b[4], b[5])).flatten();
        let mut hex = String::with_capacity(3 * HEX_BYTES);
        for (i, x) in b.iter().take(HEX_BYTES).enumerate() {
            // A write to a String cannot fail.
            let _ = write!(hex, "{}{x:02X}", if i > 0 { " " } else { "" });
        }
        let row = FrameRow {
            time_s,
            ids,
            bytes: frame.len() as u32,
            check,
            hex,
        };
        if good {
            self.dec.stats.good += 1;
            if let Some(i) = ids {
                self.dec.ids = Some(i);
                *self.dec.stats.per_vc.entry(i.vcid).or_insert(0) += 1;
            }
        } else {
            self.dec.stats.bad += 1;
        }
        (row, good.then_some(b))
    }

    fn take(found: &mut Vec<(Vec<u8>, bool)>, f: WordFrame<'_>) {
        found.push((f.bytes.to_vec(), f.inverted));
    }

    /// Logs a confirmed frame lock that starts or ends.
    fn log_lock(log: &mut Vec<String>, was: bool, now: bool) {
        if now && !was {
            log.push("frame sync".into());
        } else if was && !now {
            log.push("frame sync lost".into());
        }
    }

    /// A coded link without a good frame for [`CODE_TIMEOUT_S`] since its code lock (or its last good frame) searches the
    /// code again.
    fn code_timeout(&mut self) {
        let Some(Sync::Coded(c, s)) = &mut self.sync else {
            return;
        };
        if !c.lock().locked {
            return;
        }
        let rate = self.link.llr_rate.max(1.0);
        let since_lock = c.lock().symbols as f64 / rate;
        let since_good = (self.llrs - self.last_good) as f64 / rate;
        if since_lock > CODE_TIMEOUT_S && since_good > CODE_TIMEOUT_S {
            c.search_again();
            s.drop_lock();
            self.code_lock.locked = false;
            self.log.push("code lock lost".into());
        }
    }

    /// Builds the synchroniser and the decoders of the link, and forgets the signal.
    fn configure(&mut self) -> Result<(), ConfigError> {
        let cfg = self.link.sync;
        self.sync = Some(match self.link.line {
            Line::Quad => Sync::Quad(Box::new(QuadSync::new(cfg).map_err(stage)?)),
            Line::Nrz => Sync::One(Box::new(WordSync::new(cfg).map_err(stage)?), None),
            Line::NrzM => Sync::One(
                Box::new(WordSync::new(cfg).map_err(stage)?),
                Some(NrzM::new(0)),
            ),
            Line::QuadDiff => Sync::QuadDiff(
                Box::new(WordSync::new(cfg).map_err(stage)?),
                QuadDiffDecoder::default(),
            ),
            Line::Coded(c) => Sync::Coded(
                Box::new(CodeSync::with_word(c, cfg.word as u32).map_err(stage)?),
                Box::new(WordSync::new(cfg).map_err(stage)?),
            ),
            Line::Rails { quad, .. } => {
                let signs: &[[u8; 2]] = if quad {
                    &[[0, 0], [0, 1], [1, 0], [1, 1]]
                } else {
                    &[[0, 0]]
                };
                let mut v = Vec::with_capacity(signs.len());
                for &signs in signs {
                    v.push(Pattern {
                        signs,
                        nrzm: [NrzM::new(0), NrzM::new(0)],
                        sync: WordSync::new(cfg).map_err(stage)?,
                    });
                }
                Sync::Rails(v, None)
            }
        });
        self.plain = match (self.link.retry_plain, self.link.code) {
            (true, Some(c)) => Some(ChannelDecoder::new(c.without_randomiser()).map_err(stage)?),
            _ => None,
        };
        self.code_lock = CodeLock::default();
        self.channel = match self.link.code {
            Some(c) => Some(ChannelDecoder::new(c).map_err(stage)?),
            None => None,
        };
        Ok(())
    }

    /// Takes soft symbols and appends the good frames they complete to `frames`.
    fn run(&mut self, llrs: &[i8], frames: &mut Vec<Timed<FrameBytes>>) {
        self.llrs += llrs.len() as u64;
        self.found.clear();
        let found = &mut self.found;
        match &mut self.sync {
            Some(Sync::One(s, line)) => {
                self.bits.clear();
                self.bits.extend(llrs.iter().map(|&l| u8::from(l < 0)));
                if let Some(l) = line {
                    l.decode(&mut self.bits);
                }
                let was = s.locked();
                s.push_bits(&self.bits, |f| Self::take(found, f));
                Self::log_lock(&mut self.log, was, s.locked());
            }
            Some(Sync::Rails(pats, chosen)) => {
                let per_rail = matches!(self.link.line, Line::Rails { per_rail: true, .. });
                let rails = if matches!(self.link.line, Line::Rails { quad: true, .. }) { 2 } else { 1 };
                for (k, pt) in pats.iter_mut().enumerate() {
                    if chosen.is_some_and(|c| c != k) {
                        continue;
                    }
                    self.bits.clear();
                    self.bits.extend(
                        llrs.iter()
                            .enumerate()
                            .map(|(i, &l)| u8::from(l < 0) ^ pt.signs[i % rails]),
                    );
                    if per_rail {
                        for (i, b) in self.bits.iter_mut().enumerate() {
                            let n = &mut pt.nrzm[i % 2];
                            let mut one = [*b];
                            n.decode(&mut one);
                            *b = one[0];
                        }
                    } else {
                        pt.nrzm[0].decode(&mut self.bits);
                    }
                    let was = pt.sync.locked();
                    let take = chosen.is_some() || !was;
                    let mut got = Vec::new();
                    pt.sync.push_bits(&self.bits, |f| got.push((f.bytes.to_vec(), f.inverted)));
                    let now = pt.sync.locked();
                    if now && chosen.is_none() {
                        *chosen = Some(k);
                        self.log.push("frame sync".into());
                    } else if !now && *chosen == Some(k) {
                        *chosen = None;
                        self.log.push("frame sync lost".into());
                    }
                    if take && *chosen == Some(k) {
                        found.extend(got);
                    }
                }
            }
            Some(Sync::Quad(q)) => {
                q.push_llrs(llrs, |_, f| Self::take(found, f));
                for e in q.take_events() {
                    self.log.push(match e {
                        StreamEvent::Chosen(w) => format!("frame sync (stream {w})"),
                        StreamEvent::Lost => "frame sync lost".into(),
                    });
                }
            }
            Some(Sync::QuadDiff(s, d)) => {
                self.bits.clear();
                self.bits.extend(llrs.iter().map(|&l| u8::from(l < 0)));
                d.decode_pairs(&mut self.bits);
                let was = s.locked();
                s.push_bits(&self.bits, |f| Self::take(found, f));
                Self::log_lock(&mut self.log, was, s.locked());
            }
            Some(Sync::Coded(c, s)) => {
                self.bits.clear();
                c.push(llrs, &mut self.bits);
                let was = s.locked();
                s.push_bits(&self.bits, |f| Self::take(found, f));
                Self::log_lock(&mut self.log, was, s.locked());
                let lock = *c.lock();
                if lock.locked && (lock.locks != self.code_lock.locks || !self.code_lock.locked) {
                    self.log.push(format!(
                        "coded sync found, ambiguity {}{}",
                        lock.transform,
                        if lock.shifted { " (half slip)" } else { "" }
                    ));
                }
                self.code_lock = lock;
            }
            None => {}
        }
        let t = self.llrs as f64 / self.link.llr_rate.max(1.0);
        let found = std::mem::take(&mut self.found);
        for (f, _) in &found {
            let (row, good) = self.push_frame(f, t);
            if self.dec.rows.len() < TABLE_ROWS {
                self.dec.rows.push(row);
            }
            if let Some(bytes) = good {
                self.last_good = self.llrs;
                frames.push(Timed {
                    time: t,
                    value: FrameBytes {
                        time_s: t,
                        bytes: Arc::from(bytes),
                    },
                });
            }
        }
        self.found = found;
        self.code_timeout();
    }

    /// The report since the last one, and its time.
    fn report(&mut self) -> Timed<TableReport> {
        let t = self.llrs as f64 / self.link.llr_rate.max(1.0);
        let (locked, inverted, stream, hits) = match &self.sync {
            Some(Sync::One(s, _)) => (s.locked(), s.inverted(), None, s.stats().sync_hits),
            Some(Sync::Rails(p, c)) => {
                let s = &p[c.unwrap_or(0)].sync;
                (c.is_some(), s.inverted(), c.map(|c| c as u8), p.iter().map(|p| p.sync.stats().sync_hits).sum())
            }
            Some(Sync::Quad(q)) => (
                q.locked(),
                q.inverted(),
                q.chosen().map(|c| c as u8),
                q.sync_hits(),
            ),
            Some(Sync::QuadDiff(s, _) | Sync::Coded(_, s)) => {
                (s.locked(), s.inverted(), None, s.stats().sync_hits)
            }
            None => (false, false, None, 0),
        };
        self.dec.stats.sync_hits = hits;
        let r = TableReport {
            stats: self.dec.stats.clone(),
            locked,
            inverted,
            stream,
            ids: self.dec.ids,
            code: matches!(self.sync, Some(Sync::Coded(..))).then_some(self.code_lock),
            rows: std::mem::take(&mut self.dec.rows),
            good_now: self.dec.stats.good - self.good_reported.min(self.dec.stats.good),
            since_good_s: (self.llrs - self.last_good) as f64 / self.link.llr_rate.max(1.0),
            log: std::mem::take(&mut self.log),
        };
        self.good_reported = self.dec.stats.good;
        Timed { time: t, value: r }
    }
}

/// What a helper thread gives back: the table, the good frames, and the buffer of the soft symbols it took.
struct Done {
    table: Box<Table>,
    frames: Vec<Timed<FrameBytes>>,
    llrs: Vec<i8>,
}

/// Soft symbols that may wait for the helper thread, at the most; more are given up.
const QUEUE_MAX: usize = 1 << 27;

/// Soft symbols to the frames of a [`TableLink`]: hard bits, the line code, the frame synchroniser, the randomiser and the
/// Reed-Solomon code, and the header ids. It reports each frame as a table row and gives each good frame as [`FrameBytes`].
///
/// With [`FrameTableComponent::on_helper`] and live samples the work runs on a helper thread of the run context: the soft
/// symbols of the passes wait in a queue, one batch at a time goes to the thread, and its frames leave in the first pass
/// after it is done, in order. The `clear` command waits for the batch in work, and a report that is due meanwhile
/// repeats the counts of the last one. Recordings and generators run the work in line, so their output does not depend on
/// timing.
pub struct FrameTableComponent {
    link: TableLink,
    /// The table; `None` while a helper thread has it.
    table: Option<Box<Table>>,
    helper: bool,
    live: bool,
    pool: Option<Pool>,
    job: Option<Ticket<Done>>,
    queued: Vec<i8>,
    frames: Vec<Timed<FrameBytes>>,
    report_due: bool,
    clear_due: bool,
    /// The last report, without its rows and log lines.
    last: Option<Timed<TableReport>>,
    /// Soft symbols given up because the helper thread was too slow, since the last report.
    lost: u64,
}

impl FrameTableComponent {
    /// A component for `link`.
    pub fn new(link: TableLink) -> Self {
        FrameTableComponent {
            link,
            table: Some(Box::new(Table::new(link))),
            helper: false,
            live: false,
            pool: None,
            job: None,
            queued: Vec::new(),
            frames: Vec::new(),
            report_due: false,
            clear_due: false,
            last: None,
            lost: 0,
        }
    }

    /// Runs the work on a helper thread when the samples are live.
    pub fn on_helper(mut self) -> Self {
        self.helper = true;
        self
    }

    /// Takes one frame as the synchroniser gave it (sync word included) and returns its row and, for a good frame, its bytes.
    /// While a helper thread has the table, it waits for it.
    pub fn push_frame(&mut self, frame: &[u8], time_s: f64) -> (FrameRow, Option<Vec<u8>>) {
        self.table_back(true);
        match &mut self.table {
            Some(t) => t.push_frame(frame, time_s),
            None => (FrameRow::default(), None),
        }
    }

    /// Takes the table back from the helper thread when its batch is done, or with `wait` waits for it. The frames of the
    /// batch go to `self.frames`. A helper that failed loses the table: a new table of the link takes its place.
    fn table_back(&mut self, wait: bool) {
        let Some(mut job) = self.job.take() else {
            return;
        };
        let done = if wait {
            Some(job.wait())
        } else {
            let r = job.try_take();
            if r.is_none() {
                self.job = Some(job);
            }
            r
        };
        match done {
            Some(Ok(d)) => {
                self.frames.extend(d.frames);
                self.table = Some(d.table);
                if self.queued.is_empty() {
                    self.queued = d.llrs;
                    self.queued.clear();
                }
            }
            Some(Err(_)) => {
                let mut t = Table::new(self.link);
                t.log.push("the frame decoder failed and starts again".into());
                if t.configure().is_ok() {
                    self.table = Some(Box::new(t));
                }
            }
            None => {}
        }
    }

    /// One pass. `llrs` go through the table in line; with live samples and a helper they wait in the queue, and the
    /// table takes the whole queue to the helper thread when it is here. Before the table leaves, the report and the
    /// `clear` command that are due are served. `wait` (the end of the input) waits for the helper and runs the rest in
    /// line.
    fn pass(&mut self, io: &mut Io, wait: bool) {
        self.report_due |= io.tick();
        let llrs = io.llrs(0);
        let helped = self.helper && self.live && !wait;
        self.table_back(!helped);
        if helped {
            if self.queued.len() + llrs.len() > QUEUE_MAX {
                self.lost += self.queued.len() as u64;
                self.queued.clear();
            }
            self.queued.extend_from_slice(llrs);
        } else if let Some(t) = &mut self.table {
            if !self.queued.is_empty() {
                let queued = std::mem::take(&mut self.queued);
                t.run(&queued, &mut self.frames);
                self.queued = queued;
                self.queued.clear();
            }
            t.run(llrs, &mut self.frames);
        }
        io.items_out::<FrameBytes>(0).append(&mut self.frames);
        self.serve(io);
        if !helped || self.queued.is_empty() {
            return;
        }
        if let (Some(pool), Some(mut table)) = (&self.pool, self.table.take()) {
            let llrs = std::mem::take(&mut self.queued);
            self.job = Some(pool.spawn(move || {
                let mut frames = Vec::new();
                table.run(&llrs, &mut frames);
                Done { table, frames, llrs }
            }));
        }
    }

    /// Serves the `clear` command and the report that are due. While the helper thread has the table, the report is the
    /// last one again, without its rows and log lines, so that a display sees the counts stand and not fall to 0.
    fn serve(&mut self, io: &mut Io) {
        if let Some(t) = &mut self.table {
            if std::mem::take(&mut self.clear_due) {
                t.dec.new_picture();
            }
        }
        if !std::mem::take(&mut self.report_due) {
            return;
        }
        match &mut self.table {
            Some(t) => {
                if self.lost > 0 {
                    t.log.push(format!("{} soft symbols given up: the frame decoder is too slow", self.lost));
                    self.lost = 0;
                }
                let r = t.report();
                self.last = Some(Timed {
                    time: r.time,
                    value: TableReport {
                        rows: Vec::new(),
                        good_now: 0,
                        log: Vec::new(),
                        ..r.value.clone()
                    },
                });
                io.items_out::<TableReport>(1).push(r);
            }
            None => {
                if let Some(r) = &self.last {
                    io.items_out::<TableReport>(1).push(r.clone());
                }
            }
        }
    }
}

impl Component for FrameTableComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.frame_table",
            vec![Port::llr("in"), control_port()],
            vec![
                item_port::<FrameBytes>("frames"),
                item_port::<TableReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The batch of the signal before is given up.
        self.table_back(true);
        self.frames.clear();
        self.queued.clear();
        self.lost = 0;
        self.last = None;
        if self.table.is_none() {
            self.table = Some(Box::new(Table::new(self.link)));
        }
        if let Some(t) = &mut self.table {
            t.configure()?;
        }
        Ok(vec![
            item_format::<FrameBytes>(),
            item_format::<TableReport>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        self.pass(io, false);
    }
    fn finish(&mut self, io: &mut Io) {
        self.pass(io, true);
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.pool = Some(ctx.pool.clone());
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        TableCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match TableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TableCommand::Clear => match &mut self.table {
                Some(t) => t.dec.new_picture(),
                None => self.clear_due = true,
            },
        }
        Ok(None)
    }
}

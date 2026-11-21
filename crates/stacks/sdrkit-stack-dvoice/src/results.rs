//! What every digital voice stack publishes: the shared measurements, the verdict, the pictures, the lists as tables, and the
//! call, message and position events.

use crate::calls::Tracker;
use crate::items::{end_seen, CallRecord, Ident};
use crate::typed::{CallDetailRow, CallRow, SharedFields};
use sdrkit_core::interface::{Fields, Hertz, Seconds, Table};
use sdrkit_core::publish::{Declare, Published, TableDef};
use sdrkit_core::results::Series;
use sdrkit_core::{ResultsDecl, SeriesId};
use sdrkit_util_objects::calls::{CallEnd, CallLog};
use sdrkit_util_objects::publish as objpub;
use sdrkit_util_objects::shaped;
use std::collections::VecDeque;
use std::fmt::Write as _;

sdrkit_core::choice_enum! {
    /// Why a stack decodes or does not.
    pub enum Verdict {
        /// No signal at the channel.
        NoInput = ("no_input", "No signal"),
        /// Power, but no frame sync.
        NoSync = ("no_sync", "No frame sync"),
        /// Frames, but their identity does not decode.
        Identity = ("identity", "Identity fails"),
        /// Frames, but the payload fails its error check.
        Fec = ("fec", "Error check fails"),
        /// Decoding, with more than a third of the blocks lost.
        Marginal = ("marginal", "Marginal"),
        /// Decoding.
        Decoding = ("decoding", "Decoding"),
    }
}

impl Verdict {
    /// The status line for people.
    pub fn text(self) -> &'static str {
        match self {
            Verdict::NoInput => "No signal at the channel",
            Verdict::NoSync => "Power, but no frame sync: check the frequency, the stack and the deviation",
            Verdict::Identity => {
                "Frames found, but their identity does not decode: too noisy or distorted"
            }
            Verdict::Fec => {
                "Frames found, but the payload fails its error check: weak signal, fading or clock error"
            }
            Verdict::Marginal => "Decoding, but more than a third of the blocks are lost",
            Verdict::Decoding => "Decoding",
        }
    }
}

pub use crate::typed::{
    BER_HISTORY, CALL, CALLS_100, CALLS_300, CALL_DETAILS, EYE, GRANTS, LEVELS, MESSAGE, MESSAGES,
    POSITION, POSITIONS, SITES, SNR_HISTORY, SPECTRUM, TALKGROUPS,
};

/// What the decoder of a stack of the family publishes: the shared fields, the stack's own `fields` and `tables`, the
/// lists, the pictures and the events. `calls` is the call log of the stack.
pub fn declaration(
    calls: &'static Table<CallRow<'static>>,
    fields: &[&'static dyn Declare],
    tables: &[&'static TableDef],
) -> ResultsDecl {
    let mut d = ResultsDecl::new()
        .with_fields(SharedFields::SHAPE.fields)
        .with_fields(fields)
        .with_table(calls)
        .with_table(&CALL_DETAILS)
        .with_table(&MESSAGES)
        .with_table(&POSITIONS)
        .with_table(&SITES)
        .with_table(&GRANTS)
        .with_table(&TALKGROUPS)
        .series(EYE, "Eye", "", "")
        .series(LEVELS, "Level histogram", "", "")
        .series(SPECTRUM, "Channel spectrum", "Hz", "dB")
        .series(SNR_HISTORY, "SNR history", "", "dB")
        .series(BER_HISTORY, "Bit error history", "", "")
        .points("Positions")
        .event(&CALL)
        .event(&MESSAGE)
        .event(&POSITION);
    for t in tables {
        d = d.with_table(t);
    }
    d
}

/// The shared measurements of one report.
#[derive(Clone, Copy, Debug)]
pub struct Measured {
    /// The channel level, dBFS.
    pub level_dbfs: f64,
    /// The noise floor, dBFS.
    pub noise_dbfs: f64,
    /// The carrier to noise ratio, dB.
    pub cnr_db: Option<f64>,
    /// The SNR on the symbols, dB.
    pub snr_db: Option<f64>,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The deviation, Hz.
    pub deviation_hz: f64,
    /// The symbol clock error, ppm.
    pub clock_ppm: f64,
    /// The bit error rate.
    pub ber: f64,
    /// The signal is inverted.
    pub inverted: bool,
    /// Blocks that passed their check.
    pub blocks_ok: u64,
    /// Blocks that failed their check.
    pub blocks_bad: u64,
    /// Frames whose identity was read.
    pub frames: u64,
    /// Frame syncs.
    pub syncs: u64,
    /// Reports written.
    pub reports: u64,
    /// The verdict.
    pub verdict: Verdict,
}

/// Writes the shared measurements.
pub fn put_measured(p: &mut Published, m: &Measured) {
    p.put(&SharedFields {
        level_dbfs: m.level_dbfs,
        noise_dbfs: m.noise_dbfs,
        cnr_db: m.cnr_db,
        snr_db: m.snr_db,
        offset_hz: Hertz(m.offset_hz),
        deviation_hz: Hertz(m.deviation_hz),
        clock_ppm: m.clock_ppm,
        ber: m.ber,
        inverted: m.inverted,
        blocks_ok: m.blocks_ok as i64,
        blocks_bad: m.blocks_bad as i64,
        frames: m.frames as i64,
        syncs: m.syncs as i64,
        reports: m.reports as i64,
        verdict: m.verdict,
    });
}

/// The ids of the series in a stack's declaration, found once.
#[derive(Clone, Copy, Debug)]
pub struct SeriesIds {
    /// The channel spectrum.
    pub spectrum: SeriesId,
    eye: SeriesId,
    levels: SeriesId,
    snr: SeriesId,
    ber: SeriesId,
}

impl SeriesIds {
    /// The ids in `d`, or `None` when it lacks a series of the family.
    pub fn of(d: &ResultsDecl) -> Option<SeriesIds> {
        Some(SeriesIds {
            spectrum: d.series_id(SPECTRUM)?,
            eye: d.series_id(EYE)?,
            levels: d.series_id(LEVELS)?,
            snr: d.series_id(SNR_HISTORY)?,
            ber: d.series_id(BER_HISTORY)?,
        })
    }
}

/// Makes `s` hold `points`, drawn as a line when `line`. It returns true if that changed it.
pub fn set_series(s: &mut Series, line: bool, points: impl Iterator<Item = [f32; 2]>) -> bool {
    let mut changed = s.line != line;
    s.line = line;
    let mut n = 0;
    for p in points {
        match s.points.get_mut(n) {
            Some(q) if *q == p => {}
            Some(q) => {
                *q = p;
                changed = true;
            }
            None => {
                s.points.push(p);
                changed = true;
            }
        }
        n += 1;
    }
    if s.points.len() > n {
        s.points.truncate(n);
        changed = true;
    }
    changed
}

/// Writes the eye (oldest first), the level histogram (40 bins of 0.25 from -5) and the SNR and bit error histories.
pub fn put_pictures(
    p: &mut Published,
    ids: &SeriesIds,
    eye: &[f32],
    hist: &[u32; 40],
    snr: &VecDeque<f32>,
    ber: &VecDeque<f32>,
) {
    let r = p.results_mut();
    r.update_series(ids.eye, |s| {
        set_series(
            s,
            false,
            eye.iter().enumerate().map(|(i, &v)| [i as f32, v]),
        )
    });
    r.update_series(ids.levels, |s| {
        set_series(
            s,
            true,
            hist.iter()
                .enumerate()
                .map(|(i, &c)| [-5.0 + 0.25 * i as f32 + 0.125, c as f32]),
        )
    });
    r.update_series(ids.snr, |s| {
        set_series(s, true, snr.iter().enumerate().map(|(i, &v)| [i as f32, v]))
    });
    r.update_series(ids.ber, |s| {
        set_series(s, true, ber.iter().enumerate().map(|(i, &v)| [i as f32, v]))
    });
}

/// Writes `id` into `buf` (emptied first) and returns it.
pub fn ident_text<'a>(buf: &'a mut String, id: &Ident) -> &'a str {
    buf.clear();
    // Writing to a `String` cannot fail.
    let _ = write!(buf, "{id}");
    buf
}

/// Writes the call log into `table` and the details of its calls into [`CALL_DETAILS`]. Only cells that changed are
/// written, and rows of calls that left the log are removed.
fn put_calls(
    p: &mut Published,
    table: &'static Table<CallRow<'static>>,
    log: &CallLog<u32, CallRecord>,
    buf: &mut String,
) {
    let mut key = String::new();
    let mut dst = String::new();
    let mut details = 0;
    for call in log.iter() {
        let c = &call.record;
        let e = &c.encryption;
        key.clear();
        // Writing to a `String` cannot fail.
        let _ = write!(key, "{}", call.id);
        ident_text(&mut dst, &c.dst);
        let row = CallRow {
            id: call.id as i64,
            start_s: call.start.t,
            duration: Seconds(call.duration_s()),
            slot: i64::from(c.slot),
            kind: c.kind,
            src: ident_text(buf, &c.src),
            dst: &dst,
            group: c.group,
            emergency: c.emergency,
            access_kind: c.access.map(|a| a.kind),
            access_code: c.access.map(|a| i64::from(a.code)),
            encryption: e.state,
            algorithm: &e.alg,
            alg_id: e.alg_id.map(i64::from),
            enc_key_id: e.key_id.map(i64::from),
            alias: &c.alias,
            text: &c.text,
            voice_frames: i64::from(c.voice_frames),
            media_frames: i64::from(c.media_frames),
            ber: f64::from(c.ber()),
            active: call.is_active(),
            late_entry: call.late_entry,
            end_seen: end_seen(call.end),
            signal_lost: call.end == CallEnd::HangTime,
            carrier_hz: Hertz(c.carrier_hz),
            end: call.end,
        };
        table.put(p, &key, &row);
        for (d, v) in &c.details {
            let call_key_len = key.len();
            let _ = write!(key, "/{}", d.key());
            let row = CallDetailRow {
                call: &key[..call_key_len],
                detail: *d,
                value: v,
            };
            CALL_DETAILS.put(p, &key, &row);
            key.truncate(call_key_len);
            details += 1;
        }
    }
    objpub::keep_newest_rows(table, log.len(), p);
    objpub::keep_newest_rows(&CALL_DETAILS, details, p);
}

/// Writes the lists: the call log `calls` and its details, messages, positions (with their map points), the site, grants,
/// talkgroups and the lines of the event log not yet written.
pub fn put_lists(p: &mut Published, t: &mut Tracker, calls: &'static Table<CallRow<'static>>) {
    let mut buf = String::new();
    if t.log_cleared {
        t.log_cleared = false;
        p.results_mut().clear_log();
        p.results_mut().clear_points();
    }
    let l = &t.lists;
    put_calls(p, calls, &l.calls, &mut buf);
    shaped::log_rows(&l.messages, &MESSAGES, 0.0, p);
    shaped::log_rows(&l.positions, &POSITIONS, 0.0, p);
    let r = p.results_mut();
    for pos in l.positions.iter() {
        let id = ident_text(&mut buf, &pos.src);
        r.update_point(id, pos.lat, pos.lon, id, pos.course_deg.map(|c| c as f32));
    }
    match &l.site {
        Some(s) => {
            SITES.put(p, t.mode, &s.row());
        }
        None => {
            p.remove_row(&SITES, t.mode);
        }
    }
    shaped::log_rows(&l.grants, &GRANTS, 0.0, p);
    shaped::rows(&l.talkgroups, &TALKGROUPS, 0.0, p);
    let r = p.results_mut();
    for line in t.pending_log.drain(..) {
        r.log(line);
    }
}

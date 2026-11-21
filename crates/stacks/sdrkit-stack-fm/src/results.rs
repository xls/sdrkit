//! How the FM receiver publishes the structs of [`crate::typed`], and [`Reading`], the same results read back as one
//! typed value. The RadioText and its RadioText+ tags are published through the broadcast data publisher of
//! `sdrkit-proto-bcdata`.

use crate::bands::BandPlan;
use crate::rds::RdsView;
use crate::typed::{
    AfRow, BandPart, EonAfRow, EonRow, FmFields, LabelTagRow, OdaRow, RdsPart, TmcRow, AF, EON,
    EON_AF, LABEL_TAGS, ODA, TMC,
};
use sdrkit_core::interface::{Column, Hertz, Seconds, Table, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{LockState, Results, SeriesId, Status};
use sdrkit_proto_bcdata::publish::Publisher;
use std::sync::Arc;

/// The series of the MPX spectrum.
pub const MPX_SERIES: SeriesId = SeriesId(0);
/// The series of the RDS symbols.
pub const RDS_SYMBOLS_SERIES: SeriesId = SeriesId(1);

/// A store that can write the FM fields, tables and series.
pub fn store() -> Published {
    Published::from_decl(Arc::new(crate::typed::shape().results_decl()))
}

/// What the receiver's reports measured, beside the RDS view.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Figures {
    /// A carrier is present.
    pub carrier: bool,
    /// Stereo is reported.
    pub stereo: bool,
    /// SNR measure, dB.
    pub snr_db: f32,
    /// Channel level, dBFS.
    pub level_dbfs: f32,
    /// Peak deviation, Hz.
    pub deviation_hz: f64,
    /// Multipath ripple, %.
    pub multipath_pct: f32,
    /// Pilot injection, %.
    pub pilot_pct: f32,
    /// Stereo blend, %.
    pub stereo_pct: f32,
    /// The squelch is closed.
    pub squelched: bool,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// The equaliser is in use.
    pub equaliser: bool,
    /// The pilot PLL is locked.
    pub pilot_locked: bool,
    /// Channel filter width, Hz.
    pub channel_width_hz: f64,
    /// Seconds of signal decoded.
    pub signal_s: f64,
}

/// What depends on RDS against RBDS: the flag, the programme type's name and the call letters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meanings<'a> {
    /// RBDS meanings apply.
    pub rbds: bool,
    /// The programme type's name.
    pub programme: &'a str,
    /// The call letters (RBDS only).
    pub call_letters: Option<&'a str>,
}

/// The lists last written as tables, so that a table is written again only when its list changed, and the publisher of the
/// RadioText.
#[derive(Debug, Default)]
pub struct Shown {
    view: RdsView,
    done: bool,
    label: Publisher,
    key: String,
}

/// Writes the band plan's fields.
pub fn publish_band(p: &mut Published, band: &BandPlan) {
    p.put(&BandPart {
        band_start_hz: Hertz(band.min_hz),
        band_end_hz: Hertz(band.max_hz),
        band_step_hz: Hertz(band.step_hz),
        band_first_hz: Hertz(band.first_hz),
    });
}

/// Writes `key` as the two-digit place `i`.
fn place(key: &mut String, i: usize) {
    use std::fmt::Write;
    key.clear();
    let _ = write!(key, "{i:02}");
}

/// Writes the measurements and the RDS fields as typed fields and tables. A cell is written only when its value changed, and
/// nothing is allocated once the texts have their size, unless a list changed.
pub fn publish(p: &mut Published, shown: &mut Shown, v: &RdsView, f: &Figures, m: &Meanings) {
    use std::fmt::Write;
    shown.label.label(p, &v.text, &v.tags);
    let pi = (v.pi != 0).then(|| hex4(v.pi));
    let mut pin = [0u8; 8];
    let t = &v.tmc_service;
    p.put(&RdsPart {
        station: &v.station,
        station_partial: &v.station_partial,
        text_complete: v.text_complete,
        programme: v.pty.map(|_| m.programme),
        pty: v.pty.map(i64::from),
        pi: pi.as_ref().map(Hex4::as_str),
        call_letters: m.call_letters,
        rbds: m.rbds,
        tp: v.tp,
        ta: v.ta,
        music: v.music,
        ptyn: &v.ptyn,
        ecc: v.ecc.map(i64::from),
        language: v.language.map(i64::from),
        pin: v.pin.as_ref().map(|x| pin_text(x, &mut pin)),
        clock: v.clock.map(|c| Time(c.utc_s as f64)),
        clock_offset: v.clock.map(|c| Seconds(c.offset_min as f64 * 60.0)),
        ert: &v.ert,
        rds_sync: v.sync,
        rds_ok_pct: v.ok_pct as f64,
        groups: v.groups as i64,
        blocks_good: v.blocks.good as i64,
        blocks_corrected: v.blocks.corrected as i64,
        blocks_bad: v.blocks.bad as i64,
        blocks_voted: v.blocks.voted as i64,
        snr_db: f.snr_db as f64,
        level_dbfs: f.level_dbfs as f64,
        offset_hz: Hertz(f.offset_hz),
        carrier: f.carrier,
        stereo: f.stereo,
        stereo_pct: f.stereo_pct as f64,
        pilot_pct: f.pilot_pct as f64,
        pilot_locked: f.pilot_locked,
        deviation_hz: Hertz(f.deviation_hz),
        multipath_pct: f.multipath_pct as f64,
        equaliser: f.equaliser,
        squelched: f.squelched,
        channel_width_hz: Hertz(f.channel_width_hz),
        mpx_top_hz: Hertz(crate::measure::MPX_TOP_HZ),
        signal_time: Seconds(f.signal_s),
        tmc_ltn: t.ltn.map(i64::from),
        tmc_sid: t.sid.map(i64::from),
        tmc_afi: t.afi,
        tmc_enhanced_mode: t.enhanced_mode,
        tmc_scope: t.scope as i64,
    });

    let first = !shown.done;
    shown.done = true;
    let s = &mut shown.view;
    let key = &mut shown.key;
    if first || s.af_codes != v.af_codes {
        for (i, hz) in v.af_hz().enumerate() {
            place(key, i);
            AF.put(p, key, &AfRow { freq_hz: Hertz(hz) });
        }
        for i in v.af_codes.len()..s.af_codes.len() {
            place(key, i);
            p.remove_row(&AF, key);
        }
        s.af_codes.clone_from(&v.af_codes);
    }
    if first || s.oda != v.oda {
        for o in &v.oda {
            key.clear();
            let _ = write!(
                key,
                "{}{}",
                o.group >> 1,
                if o.group & 1 == 1 { "B" } else { "A" }
            );
            let row = OdaRow {
                aid: o.aid as i64,
                group: key.as_str(),
            };
            ODA.put(p, &hex4(o.aid), &row);
        }
        for o in s
            .oda
            .iter()
            .filter(|o| v.oda.iter().all(|n| n.aid != o.aid))
        {
            p.remove_row(&ODA, &hex4(o.aid));
        }
        s.oda.clone_from(&v.oda);
    }
    if first || s.eon != v.eon {
        for e in &v.eon {
            let pi = hex4(e.pi);
            let mut b = [0u8; 8];
            let row = EonRow {
                pi: pi.as_str(),
                name: &e.name,
                tp: e.tp,
                ta: e.ta,
                pty: e.pty.map(i64::from),
                pin: e.pin.as_ref().map(|pin| pin_text(pin, &mut b)),
            };
            EON.put(p, &pi, &row);
            let old = s
                .eon
                .iter()
                .find(|o| o.pi == e.pi)
                .map_or(0, |o| o.af_mhz.len());
            for (i, mhz) in e.af_mhz.iter().enumerate() {
                eon_af_key(key, e.pi, i);
                let row = EonAfRow {
                    network: pi.as_str(),
                    freq_hz: Hertz(mhz * 1e6),
                };
                EON_AF.put(p, key, &row);
            }
            for i in e.af_mhz.len()..old {
                eon_af_key(key, e.pi, i);
                p.remove_row(&EON_AF, key);
            }
        }
        for o in s.eon.iter().filter(|o| v.eon.iter().all(|n| n.pi != o.pi)) {
            p.remove_row(&EON, &hex4(o.pi));
            for i in 0..o.af_mhz.len() {
                eon_af_key(key, o.pi, i);
                p.remove_row(&EON_AF, key);
            }
        }
        s.eon.clone_from(&v.eon);
    }
    if first || s.tmc != v.tmc {
        for (i, m) in v.tmc.iter().enumerate().rev() {
            place(key, i);
            let row = TmcRow {
                event: m.event as i64,
                location: m.location as i64,
                negative_direction: m.negative_direction,
                extent: m.extent as i64,
                diversion: m.diversion,
                duration: m.duration as i64,
                repeats: m.repeats as i64,
                first: Time(m.first_s),
                last: Time(m.last_s),
                expires: Time(m.expires_s),
            };
            TMC.put(p, key, &row);
        }
        for i in v.tmc.len()..s.tmc.len() {
            place(key, i);
            p.remove_row(&TMC, key);
        }
        s.tmc.clone_from(&v.tmc);
    }
}

/// Writes the key of place `i` of the frequencies of the other network `pi`.
fn eon_af_key(key: &mut String, pi: u16, i: usize) {
    use std::fmt::Write;
    key.clear();
    let _ = write!(key, "{}/{i:02}", hex4(pi).as_str());
}

/// Writes series `id` in place: it changes only when the points or the line flag changed.
pub fn write_series(r: &mut Results, id: SeriesId, line: bool, points: &[[f32; 2]]) {
    r.update_series(id, |s| {
        if s.line == line && s.points == points {
            return false;
        }
        s.line = line;
        s.points.clear();
        s.points.extend_from_slice(points);
        true
    });
}

/// Writes the status lock state, measures and summary in place.
pub fn write_status(
    r: &mut Results,
    lock_state: LockState,
    snr_db: f32,
    level_dbfs: f32,
    offset_hz: f64,
    summary: &str,
) {
    r.update_status(|st| {
        let same = st.lock_state == lock_state
            && st.snr_db == Some(snr_db)
            && st.level_dbfs == Some(level_dbfs)
            && st.offset_hz == Some(offset_hz)
            && st.summary == summary;
        if !same {
            st.lock_state = lock_state;
            st.snr_db = Some(snr_db);
            st.level_dbfs = Some(level_dbfs);
            st.offset_hz = Some(offset_hz);
            st.summary.clear();
            st.summary.push_str(summary);
        }
        !same
    });
}

/// The status of a receiver that is searching.
pub fn searching() -> Status {
    Status::new(LockState::Searching).with_summary("Searching")
}

/// Four upper-case hexadecimal digits, without allocating.
#[derive(Clone, Copy)]
pub struct Hex4([u8; 4]);

impl Hex4 {
    /// The digits as text.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("")
    }
}

impl std::ops::Deref for Hex4 {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

/// `v` as four hexadecimal digits.
pub fn hex4(v: u16) -> Hex4 {
    const D: &[u8; 16] = b"0123456789ABCDEF";
    Hex4([
        D[(v >> 12) as usize],
        D[(v >> 8 & 15) as usize],
        D[(v >> 4 & 15) as usize],
        D[(v & 15) as usize],
    ])
}

/// A programme item number as `DD HH:MM`, written into `buf`.
fn pin_text<'a>(p: &crate::rds::group::Pin, buf: &'a mut [u8; 8]) -> &'a str {
    let d = |v: u8| [b'0' + v / 10 % 10, b'0' + v % 10];
    let [a, b] = d(p.day);
    let [c, e] = d(p.hour);
    let [f, g] = d(p.minute);
    *buf = [a, b, b' ', c, e, b':', f, g];
    std::str::from_utf8(buf).unwrap_or("")
}

/// The FM results read back as one typed value: what a band scan or a test reads from [`Results`] (built by
/// [`Reading::from_results`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// 0 searching, 1 tracking, 2 locked (the lock state).
    pub state: u8,
    /// The PS, or empty.
    pub station: String,
    /// The RadioText.
    pub text: String,
    /// The RadioText is complete.
    pub text_complete: bool,
    /// The programme type's name, or empty.
    pub programme: String,
    /// The programme type code.
    pub pty: Option<u8>,
    /// The PI (0 = unknown).
    pub pi: u16,
    /// The call letters, or empty.
    pub call_letters: String,
    /// RBDS meanings apply.
    pub rbds: bool,
    /// Traffic programme.
    pub traffic: bool,
    /// Traffic announcement.
    pub ta: bool,
    /// Music.
    pub music: bool,
    /// Stereo is reported.
    pub stereo: bool,
    /// Stereo blend, %.
    pub stereo_pct: f64,
    /// Pilot injection, %.
    pub pilot_pct: f64,
    /// The pilot PLL is locked.
    pub pilot_locked: bool,
    /// SNR measure, dB.
    pub snr_db: f64,
    /// Channel level, dBFS.
    pub level_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// A carrier is present.
    pub carrier: bool,
    /// Peak deviation, Hz.
    pub deviation_hz: f64,
    /// Multipath ripple, %.
    pub multipath_pct: f64,
    /// The equaliser is in use.
    pub equaliser: bool,
    /// The squelch is closed.
    pub squelched: bool,
    /// RDS block sync.
    pub rds: bool,
    /// RDS usable blocks, %.
    pub rds_ok: f64,
    /// Accepted RDS groups.
    pub groups: u64,
    /// Good blocks.
    pub blocks_good: u64,
    /// Corrected blocks (voted ones too).
    pub blocks_corrected: u64,
    /// Bad blocks.
    pub blocks_bad: u64,
    /// Blocks recovered by voting.
    pub blocks_voted: u64,
    /// Programme item number, `DD HH:MM`, or empty.
    pub pin: String,
    /// The station's clock, seconds since 1970.
    pub clock: Option<f64>,
    /// The station's local time offset, seconds.
    pub clock_offset_s: Option<f64>,
    /// The RadioText+ item is running.
    pub rtplus_running: bool,
    /// The RadioText+ tags, oldest row first: content type, its name, the text.
    pub rtplus: Vec<(u8, String, String)>,
    /// The open data applications, oldest row first: AID and group.
    pub oda: Vec<(u16, String)>,
    /// The other networks, oldest row first: PI and name.
    pub eon: Vec<(u16, String)>,
    /// The alternative frequencies, Hz, oldest row first.
    pub af_hz: Vec<f64>,
    /// The traffic messages kept.
    pub tmc_messages: usize,
    /// Extended country code.
    pub ecc: Option<u8>,
    /// Programme type name.
    pub ptyn: String,
    /// The channel filter width, Hz.
    pub channel_width_hz: f64,
    /// The band.
    pub band: BandPlan,
}

/// The rows of table `t` of `r`, oldest first.
fn rows<'a, S>(r: &'a Results, t: &Table<S>) -> Vec<&'a sdrkit_core::Record> {
    let table = r.decl().table_id(t.key).and_then(|id| r.table(id));
    let mut v: Vec<_> = table
        .into_iter()
        .flat_map(|t| t.rows_newest_first().map(|(_, row)| row))
        .collect();
    v.reverse();
    v
}

impl Reading {
    /// Reads the FM fields of `r`. A field that is absent or has no value reads as its default.
    pub fn from_results(r: &Results) -> Reading {
        let f = r.fields();
        let text = |k: &Column<field::Text>| f.at(k).unwrap_or_default().to_owned();
        let yes = |k: &Column<field::Bool>| f.at(k).unwrap_or(false);
        let num = |k: &Column<field::F64>| f.at(k).unwrap_or(0.0);
        let count = |k: &Column<field::I64>| f.at(k).map_or(0, |x| x.max(0) as u64);
        let hex = |t: &str| u16::from_str_radix(t, 16).unwrap_or(0);
        Reading {
            state: match r.status().lock_state {
                LockState::Locked => 2,
                LockState::Tracking => 1,
                _ => 0,
            },
            station: text(&FmFields::STATION),
            text: text(&FmFields::TEXT),
            text_complete: yes(&FmFields::TEXT_COMPLETE),
            programme: text(&FmFields::PROGRAMME),
            pty: f.at(&FmFields::PTY).map(|x| x as u8),
            pi: hex(f.at(&FmFields::PI).unwrap_or_default()),
            call_letters: text(&FmFields::CALL_LETTERS),
            rbds: yes(&FmFields::RBDS),
            traffic: yes(&FmFields::TP),
            ta: yes(&FmFields::TA),
            music: yes(&FmFields::MUSIC),
            stereo: yes(&FmFields::STEREO),
            stereo_pct: num(&FmFields::STEREO_PCT),
            pilot_pct: num(&FmFields::PILOT_PCT),
            pilot_locked: yes(&FmFields::PILOT_LOCKED),
            snr_db: num(&FmFields::SNR_DB),
            level_dbfs: num(&FmFields::LEVEL_DBFS),
            offset_hz: f.at(&FmFields::OFFSET_HZ).unwrap_or(0.0),
            carrier: yes(&FmFields::CARRIER),
            deviation_hz: f.at(&FmFields::DEVIATION_HZ).unwrap_or(0.0),
            multipath_pct: num(&FmFields::MULTIPATH_PCT),
            equaliser: yes(&FmFields::EQUALISER),
            squelched: yes(&FmFields::SQUELCHED),
            rds: yes(&FmFields::RDS_SYNC),
            rds_ok: num(&FmFields::RDS_OK_PCT),
            groups: count(&FmFields::GROUPS),
            blocks_good: count(&FmFields::BLOCKS_GOOD),
            blocks_corrected: count(&FmFields::BLOCKS_CORRECTED),
            blocks_bad: count(&FmFields::BLOCKS_BAD),
            blocks_voted: count(&FmFields::BLOCKS_VOTED),
            pin: text(&FmFields::PIN),
            clock: f.at(&FmFields::CLOCK),
            clock_offset_s: f.at(&FmFields::CLOCK_OFFSET),
            rtplus_running: f.at(&FmFields::ITEM_RUNNING).unwrap_or(false),
            rtplus: rows(r, &LABEL_TAGS)
                .iter()
                .map(|c| {
                    (
                        c.at(&LabelTagRow::CODE).unwrap_or(0) as u8,
                        c.at(&LabelTagRow::TYPE).unwrap_or_default().to_owned(),
                        c.at(&LabelTagRow::TEXT).unwrap_or_default().to_owned(),
                    )
                })
                .collect(),
            oda: rows(r, &ODA)
                .iter()
                .map(|c| {
                    (
                        c.at(&OdaRow::AID).unwrap_or(0) as u16,
                        c.at(&OdaRow::GROUP).unwrap_or_default().to_owned(),
                    )
                })
                .collect(),
            eon: rows(r, &EON)
                .iter()
                .map(|c| {
                    (
                        hex(c.at(&EonRow::PI).unwrap_or_default()),
                        c.at(&EonRow::NAME).unwrap_or_default().to_owned(),
                    )
                })
                .collect(),
            af_hz: rows(r, &AF)
                .iter()
                .map(|c| c.at(&AfRow::FREQ_HZ).unwrap_or(0.0))
                .collect(),
            tmc_messages: rows(r, &TMC).len(),
            ecc: f.at(&FmFields::ECC).map(|x| x as u8),
            ptyn: text(&FmFields::PTYN),
            channel_width_hz: f.at(&FmFields::CHANNEL_WIDTH_HZ).unwrap_or(0.0),
            band: BandPlan {
                min_hz: f.at(&FmFields::BAND_START_HZ).unwrap_or(0.0),
                max_hz: f.at(&FmFields::BAND_END_HZ).unwrap_or(0.0),
                step_hz: f.at(&FmFields::BAND_STEP_HZ).unwrap_or(0.0),
                first_hz: f.at(&FmFields::BAND_FIRST_HZ).unwrap_or(0.0),
            },
        }
    }
}

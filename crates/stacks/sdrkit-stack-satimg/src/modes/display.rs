//! What the displays of every stack share: the lock state of a state, the histories, the status line and the writer of the
//! fields of the link layers.

use crate::components::link::{CodeReport, DemodReport, FrameReport, FrontReport, PacketReport};
use sdrkit_codec_image::component::{Layout, Pictures, PicturesSetting};
use crate::typed::{self, LinkFields};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::results::{Series, SeriesId};
use sdrkit_core::{ChoiceDecl, LockState, Results, ResultsDecl};
use sdrkit_graph::Component;
use std::collections::VecDeque;
use std::sync::Arc;

/// Points kept in a history (30 s of reports).
pub const HISTORY: usize = 120;

/// The series `esn0` (Es/N0 in dB over the last 30 s).
pub const ESN0: SeriesId = SeriesId(0);
/// The series `byte_errors` (byte errors in percent).
pub const BYTE_ERRORS_SERIES: SeriesId = SeriesId(1);
/// The series `frames_per_s`.
pub const FRAMES_PER_S: SeriesId = SeriesId(2);
/// The series `constellation` (the newest symbols).
pub const CONSTELLATION: SeriesId = SeriesId(3);

/// Adds the series that every stack publishes, in the order of [`ESN0`], [`BYTE_ERRORS_SERIES`], [`FRAMES_PER_S`] and
/// [`CONSTELLATION`]. A declaration must have no other series before them.
pub fn with_series(d: ResultsDecl) -> ResultsDecl {
    d.with_series(&typed::ESN0_SERIES)
        .with_series(&typed::BYTE_ERRORS_SERIES)
        .with_series(&typed::FRAMES_PER_S_SERIES)
        .with_series(&typed::CONSTELLATION_SERIES)
}

/// The results writer of a display: the fields and tables of `decl`, with a log of 200 lines.
pub fn published(decl: ResultsDecl) -> Published {
    let mut p = Published::from_decl(Arc::new(decl));
    p.results_mut().set_log_cap(200);
    p
}

/// Writes the link-layer fields from the newest reports ([`link_fields`]).
pub fn put_link(
    p: &mut Published,
    t: f64,
    f: Option<&FrontReport>,
    d: Option<&DemodReport>,
    c: Option<&CodeReport>,
    fr: Option<&FrameReport>,
    pk: Option<&PacketReport>,
) {
    p.put(&link_fields(t, f, d, c, fr, pk));
}

/// The link-layer fields at signal time `t` from the newest reports. A field whose report has not come has no value.
pub fn link_fields(
    t: f64,
    f: Option<&FrontReport>,
    d: Option<&DemodReport>,
    c: Option<&CodeReport>,
    fr: Option<&FrameReport>,
    pk: Option<&PacketReport>,
) -> LinkFields {
    let last = pk.and_then(|p| p.stats.last);
    LinkFields {
        snr_db: d.map(|d| f64::from(d.state.es_n0_db)),
        level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
        offset_hz: d.map(|_| Hertz(carrier_hz(f, d))),
        clock_ppm: d.map(|d| d.state.clock_ppm),
        symbol_rate_hz: d.map(|d| Hertz(d.symbol_rate_nominal)),
        frames_ok: fr.map(|f| f.stats.good as i64),
        frames_bad: fr.map(|f| f.stats.failed as i64),
        ber: c.map(|c| c.lock.ber),
        signal_time: Seconds(t),
        input_rate_hz: f.map(|f| Hertz(f.input_rate)),
        lock_metric: d.map(|d| f64::from(d.state.lock_metric)),
        phase_ambiguity: c.map(|c| c.lock.ambiguity() as i64),
        code_locked: c.map(|c| c.lock.locked),
        nrzm: c.map(|c| c.lock.nrzm),
        marker_errors: c.map(|c| c.lock.last_marker_errors as i64),
        bytes_corrected: fr.map(|f| f.stats.corrected as i64),
        markers: fr.map(|f| f.stats.syncs as i64),
        spacecraft: last.map(|l| l.0 as i64),
        vcid: last.map(|l| l.1 as i64),
        frame_count: last.map(|l| l.2 as i64),
        packets: pk.map(|p| p.stats.packets as i64),
        packets_lost: pk.map(|p| p.stats.lost as i64),
    }
}

/// The status lock state of a state (0 searching, 1 carrier, 2 frames found, 3 frames decoding).
pub fn lock_state(state: u8) -> LockState {
    match state {
        0 => LockState::Searching,
        1 | 2 => LockState::Tracking,
        _ => LockState::Locked,
    }
}

/// A capped history.
#[derive(Clone, Debug, Default)]
pub struct History(pub VecDeque<f32>);

impl History {
    /// Adds a point.
    pub fn push(&mut self, v: f32) {
        if self.0.len() == HISTORY {
            self.0.pop_front();
        }
        self.0.push_back(v);
    }
    /// Writes the points into `s` as a line, one each 0.25 s. Returns true (the history moves at each report).
    pub fn write(&self, s: &mut Series) -> bool {
        s.points.clear();
        s.points.extend(
            self.0
                .iter()
                .enumerate()
                .map(|(i, &v)| [i as f32 * 0.25, v]),
        );
        s.line = true;
        true
    }
}

/// The histories every stack keeps.
#[derive(Clone, Debug, Default)]
pub struct Histories {
    /// Es/N0, dB.
    pub esn0: History,
    /// Byte errors (or BER), percent.
    pub errors: History,
    /// Good frames per second.
    pub fps: History,
}

impl Histories {
    /// Adds the points of a report and publishes the series and the constellation. The results must be declared with
    /// [`with_series`].
    pub fn publish(&mut self, r: &mut Results, d: Option<&DemodReport>, f: Option<&FrameReport>) {
        let es = d.map_or(-10.0, |d| d.state.es_n0_db);
        self.esn0.push(es);
        self.errors.push(f.map_or(0.0, |f| f.byte_error_pct as f32));
        self.fps.push(f.map_or(0.0, |f| f.good_now as f32 * 4.0));
        r.update_series(ESN0, |s| self.esn0.write(s));
        r.update_series(BYTE_ERRORS_SERIES, |s| self.errors.write(s));
        r.update_series(FRAMES_PER_S, |s| self.fps.write(s));
        if let Some(d) = d {
            r.update_series(CONSTELLATION, |s| {
                s.points.clear();
                s.points.extend_from_slice(&d.constellation);
                s.line = false;
                true
            });
        }
    }
}

/// The total carrier offset: the energy location, the search and the loop (the offset the user set is not included).
pub fn carrier_hz(f: Option<&FrontReport>, d: Option<&DemodReport>) -> f64 {
    f.map_or(0.0, |f| f.located_hz) + d.map_or(0.0, |d| d.state.carrier_hz)
}

/// Writes the status: the lock state of `state`, the Es/N0 while tracking, the level, the carrier offset and the summary.
pub fn status(
    r: &mut Results,
    state: u8,
    f: Option<&FrontReport>,
    d: Option<&DemodReport>,
    summary: &str,
) {
    let lock_state = lock_state(state);
    let snr_db = d.filter(|d| d.state.tracking).map(|d| d.state.es_n0_db);
    let level_dbfs = f.map(|f| f.level_dbfs);
    let offset_hz = Some(carrier_hz(f, d) + f.map_or(0.0, |f| f.user_offset_hz));
    r.update_status(|s| {
        let changed = s.lock_state != lock_state
            || s.snr_db != snr_db
            || s.level_dbfs != level_dbfs
            || s.offset_hz != offset_hz
            || s.summary != summary;
        s.lock_state = lock_state;
        s.snr_db = snr_db;
        s.level_dbfs = level_dbfs;
        s.offset_hz = offset_hz;
        if s.summary != summary {
            s.summary.clear();
            s.summary.push_str(summary);
        }
        changed
    });
}

/// Adds log lines.
pub fn log_lines(r: &mut Results, lines: &[String]) {
    for l in lines {
        r.log(l.as_str());
    }
}

/// The choices of the `view` setting of a stack whose `pictures` component has `layouts`: the choices of the component's own
/// `view` setting, so a choice index means the same view in both.
pub fn view_choices(layouts: Vec<Layout>) -> Vec<ChoiceDecl> {
    picture_choices(layouts, PicturesSetting::View)
}

/// The choices of the `instrument` setting of a stack whose `pictures` component has `layouts`.
pub fn instrument_choices(layouts: Vec<Layout>) -> Vec<ChoiceDecl> {
    picture_choices(layouts, PicturesSetting::Instrument)
}

fn picture_choices(layouts: Vec<Layout>, s: PicturesSetting) -> Vec<ChoiceDecl> {
    Pictures::new(layouts)
        .ok()
        .and_then(|p| {
            p.info()
                .settings
                .setting(s.id())
                .map(|d| d.field.choices.to_vec())
        })
        .unwrap_or_default()
}

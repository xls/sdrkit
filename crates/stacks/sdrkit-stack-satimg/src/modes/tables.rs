//! The display of the stacks that list frames without decoding them (`goes_cda`, `goes_mdl`, `gk2a_cdas`): states, fields,
//! the frame table and the table of virtual channels.

use super::display::{carrier_hz, log_lines, published, status, with_series, Histories};
use crate::components::link::{item_format, item_port, DemodReport, FrontReport};
use crate::components::table::{TableCommand, TableReport};
use crate::components::{last, Control};
use sdrkit_core::command::{CommandError, CommandId, CommandSet as _, Commands};
use crate::typed::{FrameRow, FrameTableFields, VirtualChannelRow, FRAMES, VIRTUAL_CHANNELS};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, Record, Results, ResultsDecl, SettingError, SettingId, Value, Values,
};
use sdrkit_graph::{no_values, Applied, Component, ComponentInfo, Format, Io, Port, Timed};
use std::fmt::Write as _;

/// What a frame-table stack says about itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableStack {
    /// The name in the summary.
    pub name: &'static str,
    /// Seconds from one frame to the next.
    pub frame_period_s: f64,
    /// The demodulator is the residual-carrier PM demodulator, which the display restarts without a frame lock.
    pub pm: bool,
    /// The least input rate.
    pub min_rate: f64,
    /// The note: what is not confirmed and what is not decoded.
    pub note: &'static str,
}

impl TableStack {
    /// Seconds after the last good frame during which the state stays 3: the longer of 3 s and 3 frames.
    pub fn hold_s(&self) -> f64 {
        (3.0 * self.frame_period_s).max(3.0)
    }
    /// Seconds without a frame lock after which the PM carrier is searched again: the longer of 2.5 s and 4 frames.
    pub fn research_s(&self) -> f64 {
        (4.0 * self.frame_period_s).max(2.5)
    }
}

/// The declaration of the display of a frame-table stack: the fields of the frame table, the two tables and the series.
fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(FrameTableFields::SHAPE.fields)
            .with_table(&FRAMES)
            .with_table(&VIRTUAL_CHANNELS),
    )
}

/// The display: states, fields, the tables, summary, histories and, for a PM link, the new carrier search.
pub struct TableDisplay {
    stack: &'static TableStack,
    published: Published,
    hist: Histories,
    searches: u32,
    search_end_s: f64,
    row: u64,
    summary: String,
    key: String,
}

impl TableDisplay {
    /// A display for `stack`.
    pub fn new(stack: &'static TableStack) -> Self {
        TableDisplay {
            stack,
            published: published(display_declaration()),
            hist: Histories::default(),
            searches: 0,
            search_end_s: 0.0,
            row: 0,
            summary: String::new(),
            key: String::new(),
        }
    }
}

impl Component for TableDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.table_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<TableReport>("table"),
            ],
            vec![item_port::<Control>("control")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.searches = 0;
        self.search_end_s = 0.0;
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let none = TableReport::default();
        let r = last::<TableReport>(io, 2).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        let st = self.stack;
        log_lines(self.published.results_mut(), &r.log);
        let low = f.is_some_and(|f| f.rate_too_low);
        let good_recent = r.stats.good > 0 && r.since_good_s < st.hold_s();
        let state = if low {
            0
        } else if good_recent && r.locked {
            3
        } else if r.locked {
            2
        } else if d.is_some_and(|d| d.state.locked && d.state.tracking) {
            1
        } else {
            0
        };
        if let (true, Some(d)) = (st.pm, d) {
            // Without a frame lock the carrier is searched again, after the longer of 2.5 s and 4 frames since the last
            // search ended and since the last good frame.
            if d.state.searches != self.searches {
                self.searches = d.state.searches;
                self.search_end_s = t;
            }
            let wait = st.research_s();
            if d.state.tracking
                && !r.locked
                && t - self.search_end_s >= wait
                && (r.stats.good == 0 || r.since_good_s >= wait)
            {
                self.search_end_s = t;
                io.items_out::<Control>(0).push(Timed {
                    time: t,
                    value: Control::RestartCarrier,
                });
            }
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(self.summary, "{}, {} frames", st.name, r.stats.good),
            2 => write!(self.summary, "{}, frame sync, frames failing", st.name),
            1 => write!(self.summary, "{}, symbols, no frame sync", st.name),
            _ if low => write!(
                self.summary,
                "this link needs {} Msps or more (a recording works)",
                st.min_rate / 1e6
            ),
            _ => write!(self.summary, "{}, searching", st.name),
        };
        let p = &mut self.published;
        status(p.results_mut(), state, f, d, &self.summary);
        p.put(&frame_table_fields(t, f, d, r, st.note));
        publish_tables(p, r, &mut self.row, &mut self.key);
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        TableCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match TableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TableCommand::Clear => {
                self.published.clear_rows(&FRAMES);
                self.published.clear_rows(&VIRTUAL_CHANNELS);
            }
        }
        Ok(None)
    }
}

/// The fields of the frame table at signal time `t` from the newest reports, with `note`.
pub fn frame_table_fields<'a>(
    t: f64,
    f: Option<&FrontReport>,
    d: Option<&DemodReport>,
    r: &TableReport,
    note: &'a str,
) -> FrameTableFields<'a> {
    FrameTableFields {
        snr_db: d.map(|d| f64::from(d.state.es_n0_db)),
        level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
        offset_hz: d.map(|_| Hertz(carrier_hz(f, d))),
        clock_ppm: d.map(|d| d.state.clock_ppm),
        symbol_rate_hz: d.map(|d| Hertz(d.symbol_rate_nominal)),
        frames_ok: r.stats.good as i64,
        frames_bad: r.stats.bad as i64,
        signal_time: Seconds(t),
        input_rate_hz: f.map(|f| Hertz(f.input_rate)),
        lock_metric: d.map(|d| f64::from(d.state.lock_metric)),
        frame_lock: r.locked,
        sync_hits: r.stats.sync_hits as i64,
        bytes_corrected: r.stats.corrected as i64,
        spacecraft: r.ids.map(|i| i64::from(i.spacecraft)),
        vcid: r.ids.map(|i| i64::from(i.vcid)),
        stream: r.stream.map(i64::from),
        inverted: r.inverted,
        note,
    }
}

/// Publishes the new frames of `r` as rows of [`FRAMES`] (keys counted on from `row`) and its counts as rows of
/// [`VIRTUAL_CHANNELS`]. `key` is scratch space.
pub fn publish_tables(p: &mut Published, r: &TableReport, row: &mut u64, key: &mut String) {
    for fr in &r.rows {
        key.clear();
        // A write to a String cannot fail.
        let _ = write!(key, "{row}");
        *row += 1;
        FRAMES.put(
            p,
            key,
            &FrameRow {
                time: Seconds(fr.time_s),
                spacecraft: fr.ids.map(|i| i64::from(i.spacecraft)),
                vcid: fr.ids.map(|i| i64::from(i.vcid)),
                bytes: i64::from(fr.bytes),
                check: fr.check.text(),
                hex: fr.hex.as_str(),
            },
        );
    }
    for (vc, n) in &r.stats.per_vc {
        key.clear();
        // A write to a String cannot fail.
        let _ = write!(key, "{vc}");
        VIRTUAL_CHANNELS.put(
            p,
            key,
            &VirtualChannelRow {
                vcid: i64::from(*vc),
                frames: *n as i64,
            },
        );
    }
}

/// The data bytes of a test frame: 44 85 (an AOS header of spacecraft 18, virtual channel 5), a 24-bit frame counter, then
/// bytes of `rng`.
pub fn test_data(
    counter: u32,
    len: usize,
    rng: &mut sdrkit_util_bits::xorshift::Xorshift64,
) -> Vec<u8> {
    let mut d = vec![0u8; len];
    d[..2].copy_from_slice(&[0x44, 0x85]);
    d[2..5].copy_from_slice(&counter.to_be_bytes()[1..]);
    for b in &mut d[5..] {
        *b = rng.step() as u8;
    }
    d
}

/// The schema of a stack without settings of its own.
pub fn no_settings() -> std::sync::Arc<sdrkit_core::Schema> {
    static SCHEMA: std::sync::OnceLock<std::sync::Arc<sdrkit_core::Schema>> =
        std::sync::OnceLock::new();
    SCHEMA
        .get_or_init(|| std::sync::Arc::new(sdrkit_core::Schema::of(&[])))
        .clone()
}

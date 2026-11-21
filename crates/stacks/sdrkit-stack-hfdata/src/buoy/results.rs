//! The wave-buoy results: the carrier, the symbols, the HXV structure and the frame analysis,
//! published as typed fields, tables, series and the `wavebuoy.vector` event.

use super::hxv::{system_values, BlockReader, SpecPoint};
use super::rx::{Analysis, Carrier, FrameRow, LinkFormat, Symbol, VectorEvent, SPECTRUM_SERIES};
use crate::typed::{
    WavebuoyFields, WavebuoyFrameRow, WavebuoySpectrumRow, WavebuoySystemWordRow,
    WavebuoyVectorEvent, WAVEBUOY_FRAMES, WAVEBUOY_SPECTRUM, WAVEBUOY_SYSTEM_WORDS,
    WAVEBUOY_VECTOR,
};
use crate::util::update_points;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::results::SeriesId;
use sdrkit_core::{ConfigError, LockState, Record, Results, ResultsDecl, Status, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::collections::VecDeque;
use std::sync::Arc;

/// The buoy channels (sigidwiki list): name and MHz.
pub const CHANNELS: &[(&str, f64)] = &[
    ("English Channel, UK", 35.3875),
    ("Lyme Bay, UK", 35.2875),
    ("South UK", 34.7375),
    ("Minehead, Bristol Channel, UK", 31.9275),
    ("Weston, UK", 31.2290),
    ("Default", 30.4750),
    ("Australia", 29.9250),
    ("Morocco", 29.5250),
];

/// The channel within 6 kHz of an absolute frequency.
pub fn channel(hz: f64) -> Option<&'static str> {
    CHANNELS
        .iter()
        .find(|c| (c.1 * 1e6 - hz).abs() <= 6e3)
        .map(|c| c.0)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Fields, tables, series and events.

/// The most frames listed.
pub const FRAMES_CAP: usize = 40;

/// The mean frequency of the newest 256 symbols, Hz about the centre.
pub const SCATTER_SERIES: &str = "scatter";
/// The newest 128 heave samples, one every 0.78 s.
pub const HEAVE_SERIES: &str = "heave";
/// The newest 128 north samples.
pub const NORTH_SERIES: &str = "north";
/// The newest 128 west samples.
pub const WEST_SERIES: &str = "west";

/// What the results component publishes: all but the spectrum series of the carrier finder.
fn own_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(WavebuoyFields::SHAPE.fields)
        .with_table(&WAVEBUOY_SPECTRUM)
        .with_table(&WAVEBUOY_SYSTEM_WORDS)
        .with_table(&WAVEBUOY_FRAMES)
        .series(SCATTER_SERIES, "Symbol means", "symbol", "Hz")
        .series(HEAVE_SERIES, "Heave", "sample", "cm")
        .series(NORTH_SERIES, "North", "sample", "cm")
        .series(WEST_SERIES, "West", "sample", "cm")
        .event(&WAVEBUOY_VECTOR)
}

/// What the wave-buoy stack publishes.
pub fn declaration() -> ResultsDecl {
    own_declaration().series(SPECTRUM_SERIES, "Spectrum", "column", "")
}

impl Shaped for FrameRow {
    type Row = WavebuoyFrameRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WavebuoyFrameRow {
            start: Seconds(self.time),
            hex: &self.hex,
            accepted: self.accepted,
        });
    }
}

/// Writes the cells of a spectrum point.
fn spectrum_row(p: &SpecPoint) -> WavebuoySpectrumRow {
    WavebuoySpectrumRow {
        entry: i64::from(p.n),
        frequency: Hertz(p.hz),
        psd_m2_hz: p.psd,
        direction_deg: p.dir_deg,
        spread_deg: p.spread_deg,
        m2: p.m2,
        n2: p.n2,
        k: p.k,
        relative_psd: p.rpsd,
    }
}

/// The row key of entry or word `n`.
fn key2(n: usize) -> String {
    format!("{n:02}")
}

/// Forgets the bits, frames, vectors and system words.
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets the bits, frames, vectors and system words.");

sdrkit_core::commands! {
    /// The commands of [`BuoyResults`].
    pub enum BuoyCommand {
        /// Forget what was received.
        Clear = CLEAR,
    }
}

/// What the HXV decoder reported last.
#[derive(Clone, Debug, Default, PartialEq)]
struct HxvState {
    locked: bool,
    inverted: bool,
    vectors: u64,
    clean: u64,
    repaired: u64,
    lost: u64,
}

/// `hfdata.buoy_results`: carrier, symbols, vectors and analyses in, `wavebuoy.vector` events out.
pub struct BuoyResults {
    p: Published,
    series: [SeriesId; 4],
    entries_shown: Vec<bool>,
    tuned_hz: f64,
    carrier: Option<Carrier>,
    sym: Option<Symbol>,
    scatter: VecDeque<f32>,
    bits: VecDeque<u8>,
    reader: BlockReader,
    hxv: HxvState,
    frames: LogRing<FrameRow>,
    analysis: Option<Analysis>,
    accepted: u64,
    refused: u64,
    heard: bool,
    now: f64,
}

impl Default for BuoyResults {
    fn default() -> Self {
        let decl = Arc::new(own_declaration());
        let id = |k| decl.series_id(k).expect("the declaration holds the series");
        let series = [
            id(SCATTER_SERIES),
            id(HEAVE_SERIES),
            id(NORTH_SERIES),
            id(WEST_SERIES),
        ];
        BuoyResults {
            p: Published::from_decl(decl),
            series,
            entries_shown: vec![false; 64],
            tuned_hz: 0.0,
            carrier: None,
            sym: None,
            scatter: VecDeque::new(),
            bits: VecDeque::new(),
            reader: BlockReader::default(),
            hxv: HxvState::default(),
            frames: LogRing::new(FRAMES_CAP),
            analysis: None,
            accepted: 0,
            refused: 0,
            heard: false,
            now: 0.0,
        }
    }
}

impl BuoyResults {
    fn format(&self) -> LinkFormat {
        self.carrier.as_ref().map_or(LinkFormat::Fsk2, |c| c.format)
    }

    fn summary(&self, found: bool, sym_locked: bool) -> String {
        let fmt = self.format();
        let sys = system_values(&self.reader.system);
        let baud = self.sym.as_ref().map_or(0.0, |s| s.baud);
        if !found {
            format!(
                "listening on {:.4} MHz for a buoy FSK link",
                self.tuned_hz / 1e6
            )
        } else if fmt == LinkFormat::Fsk2 && self.hxv.locked {
            match (sys.hs_m, sys.tz_s) {
                (Some(hs), Some(tz)) => format!(
                    "HXV buoy data, Hs {hs:.2} m, Tz {tz:.1} s, {} vectors, {} repaired",
                    self.hxv.vectors, self.hxv.repaired
                ),
                _ => format!(
                    "HXV buoy data, {} vectors, {} repaired, waiting for the sea state words",
                    self.hxv.vectors, self.hxv.repaired
                ),
            }
        } else if let (true, Some(a)) = (
            sym_locked,
            self.analysis.as_ref().filter(|a| a.attempt.is_some()),
        ) {
            format!(
                "{}, {baud:.2} baud, frames of {} bits, CRC attempt fits",
                fmt.name(),
                a.period.unwrap_or(0)
            )
        } else if sym_locked {
            format!("{}, {baud:.2} baud, symbols locked", fmt.name())
        } else {
            let (w, snr) = self
                .carrier
                .as_ref()
                .map_or((0.0, 0.0), |c| (c.width_hz, c.snr_100_db));
            format!("{}-like signal, {w:.0} Hz wide, {snr:.0} dB", fmt.name())
        }
    }

    fn publish(&mut self) {
        let found = self.carrier.as_ref().is_some_and(|c| c.found);
        let fmt = self.format();
        let sym_locked = self.sym.as_ref().is_some_and(|s| s.locked);
        let data_valid = (fmt == LinkFormat::Fsk2 && self.hxv.locked)
            || (fmt == LinkFormat::Mfsk4 && sym_locked);
        let lock_state = if data_valid {
            LockState::Locked
        } else if found {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let sys = system_values(&self.reader.system);
        let c = self.carrier.clone();
        let seen = c.as_ref().filter(|c| c.found);
        let off = seen.map(|c| c.centre_hz);
        let abs = seen.map(|c| self.tuned_hz + c.centre_hz);
        let chan = abs.and_then(channel);
        let summary = self.summary(found, sym_locked);
        let bits: String = self
            .bits
            .iter()
            .map(|b| if *b == 1 { '1' } else { '0' })
            .collect();
        let points = self.reader.points();
        let last = self.reader.last;
        let p = &mut self.p;
        let s = self.sym.as_ref();
        let h = &self.hxv;
        let words = self.reader.system.iter().filter(|w| w.is_some()).count();
        let a = self.analysis.as_ref();
        let att = a.and_then(|a| a.attempt.as_ref());
        p.put(&WavebuoyFields {
            format: fmt,
            format_forced: c.as_ref().is_some_and(|c| c.forced),
            signal_found: found,
            data_valid,
            offset_hz: off.map(Hertz),
            frequency: abs.map(Hertz),
            channel: chan,
            peak_db: c.as_ref().map(|c| c.peak_db),
            width: c.as_ref().map(|c| Hertz(c.width_hz)),
            snr_db: c.as_ref().map(|c| c.snr_100_db),
            symbol_rate_hz: s.map(|s| Hertz(s.baud)),
            clock_ppm: s.map(|s| s.clock_ppm),
            tone_step: s.map(|s| Hertz(s.step_hz)),
            symbol_error_steps: s.map(|s| s.error),
            symbol_count: s.map_or(0, |s| s.count as i64),
            locked: sym_locked,
            bits: &bits,
            hxv_locked: h.locked,
            hxv_inverted: h.inverted,
            vector_count: h.vectors as i64,
            clean_count: h.clean as i64,
            repaired_count: h.repaired as i64,
            lost_count: h.lost as i64,
            block_count: self.reader.blocks as i64,
            last_block: self.reader.last_block.map(|b| b as i64),
            wrsg: last.is_some_and(|v| v.wrsg),
            heave_cm: last.map(|v| v.heave as i64),
            north_cm: last.map(|v| v.north as i64),
            west_cm: last.map(|v| v.west as i64),
            system_word_count: words as i64,
            spectrum_point_count: points.len() as i64,
            tn: sys.tn.map(i64::from),
            gps_ok: sys.gps_ok,
            gps_fix: sys.gps_fix,
            hrms_m: sys.hrms_m,
            hs_m: sys.hs_m,
            fz: sys.fz_hz.map(Hertz),
            tz_s: sys.tz_s,
            psd_max_m2_hz: sys.psd_max,
            tr_c: sys.tr_c,
            tw_c: sys.tw_c,
            life_left_weeks: sys.weeks_left.map(i64::from),
            battery_status: sys.battery.map(i64::from),
            av0_m_s2: sys.av0,
            ax0_m_s2: sys.ax0,
            ay0_m_s2: sys.ay0,
            position: sys.lat.zip(sys.lon),
            orientation_deg: sys.orientation_deg,
            inclination_deg: sys.inclination_deg,
            period_bits: a.and_then(|a| a.period).map(|n| n as i64),
            period_z: a.filter(|a| a.period.is_some()).map(|a| a.z),
            sync: a
                .filter(|a| !a.sync_hex.is_empty())
                .map(|a| a.sync_hex.as_str()),
            sync_start_bit: a.and_then(|a| a.sync_start).map(|n| n as i64),
            crc_attempt: att.map(|x| x.name.as_str()),
            crc_passes: att.map_or(0, |x| x.passes as i64),
            frames_ok: self.accepted as i64,
            frames_bad: self.refused as i64,
        });
        let mut shown = [false; 64];
        for pt in &points {
            let n = usize::from(pt.n).min(63);
            shown[n] = true;
            WAVEBUOY_SPECTRUM.put(p, &key2(n), &spectrum_row(pt));
        }
        for (n, was) in self.entries_shown.iter_mut().enumerate() {
            if *was && !shown[n] {
                p.remove_row(&WAVEBUOY_SPECTRUM, &key2(n));
            }
            *was = shown[n];
        }
        for (i, w) in self.reader.system.iter().enumerate() {
            match w {
                Some(w) => {
                    WAVEBUOY_SYSTEM_WORDS.put(
                        p,
                        &key2(i),
                        &WavebuoySystemWordRow {
                            word: i as i64,
                            value: i64::from(*w),
                        },
                    );
                }
                None => {
                    p.remove_row(&WAVEBUOY_SYSTEM_WORDS, &key2(i));
                }
            }
        }
        shaped::log_rows(&self.frames, &WAVEBUOY_FRAMES, self.now, p);
        let r = p.results_mut();
        let mut st = Status::new(lock_state).with_summary(&summary);
        if let Some(c) = seen {
            st = st
                .with_snr_db(c.snr_100_db as f32)
                .with_offset_hz(c.centre_hz);
        }
        r.set_status(&st);
        let [scatter, heave, north, west] = self.series;
        update_points(
            r,
            scatter,
            false,
            self.scatter.iter().enumerate().map(|(i, &f)| [i as f32, f]),
        );
        let recent = &self.reader.recent;
        let track = |f: fn(&(i32, i32, i32)) -> i32| {
            recent
                .iter()
                .enumerate()
                .map(move |(i, v)| [i as f32, f(v) as f32])
        };
        update_points(r, heave, true, track(|v| v.0));
        update_points(r, north, true, track(|v| v.1));
        update_points(r, west, true, track(|v| v.2));
        if !self.heard && found {
            self.heard = true;
            r.log(format!(
                "Wave buoy: {} signal at {:.4} MHz",
                fmt.name(),
                abs.unwrap_or(0.0) / 1e6
            ));
        }
    }
}

impl Component for BuoyResults {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.buoy_results",
            vec![
                Port::typed::<Carrier>("carrier"),
                Port::typed::<Symbol>("symbols"),
                Port::typed::<VectorEvent>("vectors"),
                Port::typed::<Analysis>("frames"),
            ],
            vec![Port::new("events", Kind::Messages)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned_hz = inputs[0].tuned_hz;
        self.reader.restart();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for c in io.items::<Carrier>(0) {
            self.now = self.now.max(c.value.time);
            self.carrier = Some(c.value.clone());
        }
        for s in io.items::<Symbol>(1) {
            let s = &s.value;
            self.now = self.now.max(s.time);
            if self.scatter.len() >= 256 {
                self.scatter.pop_front();
            }
            self.scatter.push_back(s.freq);
            if s.locked {
                let bits: &[u8] = if s.format == LinkFormat::Mfsk4 {
                    &[(s.tone >> 1) & 1, s.tone & 1]
                } else {
                    &[s.tone & 1]
                };
                for &b in bits {
                    if self.bits.len() >= 320 {
                        self.bits.pop_front();
                    }
                    self.bits.push_back(b);
                }
            }
            self.sym = Some(s.clone());
        }
        for e in io.items::<VectorEvent>(2) {
            match &e.value {
                VectorEvent::Vector(v) => {
                    if self.reader.take(v) {
                        self.reader.restart();
                    }
                    io.messages_out(0).push(WAVEBUOY_VECTOR.event(
                        v.time,
                        self.tuned_hz,
                        &WavebuoyVectorEvent {
                            heave_cm: i64::from(v.heave),
                            north_cm: i64::from(v.north),
                            west_cm: i64::from(v.west),
                            cyclic: i64::from(v.cyclic),
                            family: Family::Sensor,
                        },
                    ));
                }
                VectorEvent::Lost(i) => self.reader.lost(*i),
                VectorEvent::Restart => self.reader.restart(),
                VectorEvent::State {
                    locked,
                    inverted,
                    vectors,
                    clean,
                    repaired,
                    lost,
                } => {
                    self.hxv = HxvState {
                        locked: *locked,
                        inverted: *inverted,
                        vectors: *vectors,
                        clean: *clean,
                        repaired: *repaired,
                        lost: *lost,
                    };
                }
            }
        }
        for a in io.items::<Analysis>(3) {
            let a = &a.value;
            for f in &a.new_frames {
                if a.attempt.is_some() {
                    if f.accepted {
                        self.accepted += 1;
                    } else {
                        self.refused += 1;
                    }
                }
                self.frames.push(f.clone());
            }
            self.analysis = Some(a.clone());
        }
        if io.tick() {
            self.publish();
        }
    }
    fn commands(&self) -> &Commands {
        BuoyCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match BuoyCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BuoyCommand::Clear => {
                self.bits.clear();
                self.frames.clear();
                self.analysis = None;
                self.accepted = 0;
                self.refused = 0;
                self.reader.clear();
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

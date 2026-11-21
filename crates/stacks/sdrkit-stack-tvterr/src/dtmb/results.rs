//! What the DTMB receiver publishes, and `dtmb.report`, the component that writes it.

use super::fec::FecState;
use super::front::FrontState;
use super::profile::Carriers;
use super::si::SiState;
use super::track::TrackState;
use super::{width_values, DtmbSetting, Width, BODY, WIDTH};
use crate::status::{write_status, StatusLine, StepColour};
use crate::typed::{DtmbFields, DTMB_CIR};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::Choice;
use sdrkit_core::{
    ConfigError, LockState, Results, ResultsDecl, SeriesId, SettingError, SettingId, SettingSet,
    Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Kept, Port};
use std::fmt::Write;
use std::sync::Arc;

/// What the stack decodes and which of its values no public document confirms.
pub const NOTE_TEXT: &str = "Decodes the frame body to the transport stream: both carrier forms, every profile of GB \
    20600-2006 (DTMB-A is not supported). Not compared with a printed copy of the standard: the PN generators and the phase \
    schedule, the 22 system-information words and their places, the frequency interleaver of C = 3780, the constellations, \
    the Nordstrom-Robinson table and the three LDPC matrices. Not confirmed: 4QAM-NR is taken as the time interleaver acting \
    on the NR input bits (not on the 4QAM symbols after the NR code), and no on-air 4QAM-NR signal was decoded.";

/// What `dtmb.report` publishes: the result set and the impulse response.
pub fn report_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(DtmbFields::SHAPE.fields)
        .with_series(&DTMB_CIR)
}

/// What the DTMB stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(super::ID).results_decl()
}

/// `dtmb.report`: gathers the front end's and the tracker's states into the results.
pub struct Report {
    width: Width,
    p: Published,
    cir: SeriesId,
    front: FrontState,
    track: TrackState,
    si: SiState,
    fec: FecState,
    summary: String,
}

impl Default for Report {
    fn default() -> Self {
        Report::new(Width::W8)
    }
}

impl Report {
    /// For a channel width.
    pub fn new(width: Width) -> Report {
        let decl = Arc::new(report_declaration());
        let cir = decl
            .series_id(DTMB_CIR.key)
            .unwrap_or_else(|| unreachable!("the report declares its plot"));
        let p = Published::from_decl(decl);
        Report {
            width,
            p,
            cir,
            front: FrontState::default(),
            track: TrackState::default(),
            si: SiState::default(),
            fec: FecState::default(),
            summary: String::with_capacity(160),
        }
    }

    /// Forgets the signal and publishes the empty state.
    fn reset(&mut self) {
        self.track = TrackState::default();
        self.si = SiState::default();
        self.fec = FecState::default();
        self.publish();
    }

    fn publish(&mut self) {
        let fsym = self.width.symbol_rate();
        let t = &self.track;
        let f = &self.front;
        let si = &self.si;
        let fec = &self.fec;
        let p = &mut self.p;
        let decoding = t.tracking && si.index.is_some();
        let locked = decoding && fec.ts_lock;
        // The MER stands for the SNR once the profile is known and it lies between 8 dB and the header's C/N.
        let mer = fec.mer_db.filter(|_| decoding);
        let snr = match mer {
            Some(m) if m > 8.0 && m < t.cn_db => m,
            _ => t.cn_db,
        };
        let header = t.header.filter(|_| t.tracking);
        let profile = si
            .index
            .and_then(super::profile::Profile::from_index)
            .filter(|_| decoding);
        let c = &fec.counters;
        p.put(&DtmbFields {
            detection: t.tracking as i64 + decoding as i64 + locked as i64,
            tracking: t.tracking,
            header,
            pn_phase_rotates: header.map(|_| t.rotates),
            width: Hertz(self.width.hz()),
            frame_length: header.map(|h| Seconds((h.len() + BODY) as f64 / fsym)),
            cn_db: header.map(|_| t.cn_db),
            echo_span: header.map(|_| Seconds(t.echo_span_s)),
            frames: t.frames as i64,
            frames_erased: t.erased as i64,
            no_header_pct: t.no_header_pct,
            rate_ok: f.rate_ok,
            step_pn_sync: StepColour::of(decoding, t.tracking),
            si_index: profile.map(|pr| pr.index() as i64),
            si_corr: t.tracking.then_some(si.corr),
            mapping: profile.map(|pr| pr.mapping),
            code_rate: profile.map(|pr| pr.rate),
            interleaver: profile.map(|pr| pr.interleaver),
            carriers: si.carriers.filter(|_| decoding),
            mer_db: mer,
            ldpc_ok: c.good as i64,
            ldpc_bad: c.bad as i64,
            ldpc_iterations: c.iterations,
            bch_corrected: c.bch_corrected as i64,
            codewords_dropped: 0,
            codewords_skipped: c.skipped as i64,
            packets: c.packets as i64,
            ts_lock: locked,
            payload_bps: profile.and_then(|pr| Some(pr.net_rate(t.header?, fsym))),
            note: NOTE_TEXT,
            offset_hz: header.map(|_| Hertz(f.coarse_hz + t.fine_hz)),
            clock_ppm: t.tracking.then_some(t.clock_ppm),
            level_dbfs: f.level_dbfs,
            snr_db: header.map(|_| snr),
        });
        let r = p.results_mut();
        let offset = f.coarse_hz + t.fine_hz;
        self.summary.clear();
        match (t.tracking, t.header, profile) {
            (true, Some(h), Some(pr)) => {
                let single = si.carriers == Some(Carriers::Single);
                let _ = write!(
                    self.summary,
                    "DTMB{}: {} {} {} mode {}  C/N {:.1} dB",
                    if single { " C=1" } else { "" },
                    h.name(),
                    pr.mapping.label(),
                    pr.rate.key(),
                    pr.interleaver.key(),
                    t.cn_db
                );
                if let Some(m) = mer {
                    let _ = write!(self.summary, "  MER {m:.1} dB");
                }
                let _ = write!(
                    self.summary,
                    "  CFO {offset:.0} Hz  LDPC ok {} bad {}  packets {}{}",
                    c.good,
                    c.bad,
                    c.packets,
                    if locked { "  TS lock" } else { "" }
                );
            }
            (true, Some(h), None) => {
                let _ = write!(
                    self.summary,
                    "DTMB: {} frames tracked, system information not decoded  C/N {:.1} dB  CFO {:.0} Hz",
                    h.name(),
                    t.cn_db,
                    offset
                );
            }
            _ if !f.rate_ok => self.summary.push_str("too low for DTMB (8 Msps or more)"),
            _ => self.summary.push_str("DTMB: searching"),
        }
        write_status(
            r,
            &StatusLine {
                lock_state: if locked {
                    LockState::Locked
                } else if t.tracking {
                    LockState::Tracking
                } else {
                    LockState::Searching
                },
                snr_db: t.tracking.then_some(snr as f32),
                offset_hz: t.tracking.then_some(offset),
                level_dbfs: Some(f.level_dbfs as f32),
                summary: &self.summary,
            },
        );
        let cir: &[f32] = &t.cir_db;
        r.update_series(self.cir, |ser| {
            let same = ser.line
                && ser.points.len() == cir.len()
                && ser
                    .points
                    .iter()
                    .zip(cir)
                    .enumerate()
                    .all(|(i, (p, &v))| *p == [i as f32, v]);
            if !same {
                ser.line = true;
                ser.points.clear();
                ser.points
                    .extend(cir.iter().enumerate().map(|(i, &v)| [i as f32, v]));
            }
            !same
        });
    }
}

impl Component for Report {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "dtmb.report",
            vec![
                Port::typed::<FrontState>("front"),
                Port::typed::<TrackState>("track"),
                Port::typed::<SiState>("si"),
                Port::typed::<FecState>("fec"),
            ],
        )
        .with_settings(DtmbSetting::schema())
    }
    fn values(&self) -> Values {
        width_values(self.width)
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.front = FrontState::default();
        self.reset();
        Ok(vec![])
    }
    /// Every result of the signal is of the frequency before. The level and the rate rule are of the input and stay.
    fn forget(&mut self, _: &Kept) {
        self.reset();
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        if let Some(f) = io.items::<FrontState>(0).last() {
            self.front = f.value;
            changed = true;
        }
        for t in io.items::<TrackState>(1) {
            for l in &t.value.log {
                self.p.results_mut().log(l.clone());
            }
            self.track.clone_from(&t.value);
            changed = true;
        }
        self.track.log.clear();
        for s in io.items::<SiState>(2) {
            for l in &s.value.log {
                self.p.results_mut().log(l.clone());
            }
            self.si.clone_from(&s.value);
            changed = true;
        }
        self.si.log.clear();
        for s in io.items::<FecState>(3) {
            for l in &s.value.log {
                self.p.results_mut().log(l.clone());
            }
            self.fec.clone_from(&s.value);
            changed = true;
        }
        self.fec.log.clear();
        if changed {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DtmbSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DtmbSetting::Width => self.width = WIDTH.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

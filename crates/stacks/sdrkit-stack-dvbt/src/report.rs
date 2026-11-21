//! `dvbt.report`: gathers what the DVB-T components measure into the stack's results: the status lock state and line, the typed
//! fields and the quality score.

use crate::items::Link;
use crate::params::{Bandwidth, CodeRate, Constellation, Guard, Mode};
use crate::results::{quality_of, RxState};
use crate::settings::{StreamChoice, STREAM};
use crate::typed::DvbtFields;
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, ConfigError, LockState, Results, ResultsDecl, SettingError, SettingId, SettingSet,
    Status, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Port};
use sdrkit_proto_mpegts::stream::{self, TsPacket};
use sdrkit_proto_mpegts::ServiceList;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// The results are brought up to date at most this often, in signal seconds.
const REPORT_EVERY_S: f64 = 0.033;
/// The RS history of the quality score, in reports.
const RS_HISTORY: usize = 40;

/// The C/N in dB that a constellation and code rate need on a Gaussian channel.
///
/// Standard: ETSI EN 300 744 Annex A, Table A.1.
pub fn required_cn(constellation: Constellation, rate: CodeRate) -> f32 {
    let row = match constellation {
        Constellation::Qpsk => [3.1, 4.9, 5.9, 6.9, 7.7],
        Constellation::Qam16 => [8.8, 11.1, 12.5, 13.5, 13.9],
        Constellation::Qam64 => [14.4, 16.5, 18.0, 19.3, 20.1],
    };
    row[match rate {
        CodeRate::R12 => 0,
        CodeRate::R23 => 1,
        CodeRate::R34 => 2,
        CodeRate::R56 => 3,
        CodeRate::R78 => 4,
    }]
}

sdrkit_core::settings! {
    /// The settings of `dvbt.report`.
    pub enum ReportSetting {
        /// The stream of a hierarchical multiplex (for the net bit rate).
        Stream = STREAM,
    }
}

/// `dvbt.report`.
pub struct DvbtReport {
    link: Arc<Link>,
    p: Published,
    lp: bool,
    now: f64,
    report_t: f64,
    quality: f32,
    rs_hist: VecDeque<(u64, u64)>,
    last_rs: (u64, u64),
    services: usize,
    summary: String,
    unsupported: String,
}

impl Default for DvbtReport {
    fn default() -> Self {
        DvbtReport::new(Link::new())
    }
}

impl DvbtReport {
    /// A report of the components that share `link`.
    pub fn new(link: Arc<Link>) -> Self {
        DvbtReport {
            link,
            p: Published::from_decl(Arc::new(
                ResultsDecl::new().with_fields(DvbtFields::SHAPE.fields),
            )),
            lp: false,
            now: 0.0,
            report_t: f64::NEG_INFINITY,
            quality: 0.0,
            rs_hist: VecDeque::with_capacity(RS_HISTORY + 1),
            last_rs: (0, 0),
            services: 0,
            summary: String::with_capacity(128),
            unsupported: String::new(),
        }
    }

    fn publish(&mut self) {
        let Ok(t) = self.link.telemetry.lock() else {
            return;
        };
        let link = &self.link;
        let mut state = link.state.load(Ordering::Relaxed);
        let tps_locked = link.tps_locked.load(Ordering::Relaxed);
        if state >= 2 && !tps_locked {
            state = 1;
        }
        let rx_state = match state {
            0 => RxState::Searching,
            1 => RxState::Tracking,
            2 => RxState::Locked,
            _ => RxState::Streaming,
        };
        let detect = link.detect.load(Ordering::Relaxed);
        let detect = if state == 0 && link.candidate.load(Ordering::Relaxed) {
            detect.max(1)
        } else {
            detect
        };
        let w = &t.width;
        let bw = w.bandwidth.unwrap_or(Bandwidth::B8);
        let nat = bw.natural_rate();
        let level = t.level_dbfs.unwrap_or(-120.0);
        let mode = t.fft.and_then(|(n, _)| Mode::from_fft(n));
        let guard = t
            .fft
            .and_then(|(n, g)| Guard::ALL.into_iter().find(|x| x.samples(n) == g));
        let offset_hz = t.fft.map(|(n, _)| {
            let spacing = nat / n as f64;
            let int = f64::from(t.int_offset.unwrap_or(0));
            let frac = if t.mirrored { -t.frac } else { t.frac };
            (frac + int) * spacing + link.locator_hz()
        });
        let locked = state >= 2 && tps_locked;
        let tps = t.tps.filter(|_| locked);
        let required = tps.map(|(tp, _, _)| required_cn(tp.constellation, tp.rate_hp));
        let snr = t.eq.snr_db;
        let data_valid = t.eq.data_valid && locked;
        let f = &t.fec;
        let o = &f.outer;
        let fec_sync = f.fec_sync && state >= 2;
        if let Some(u) = t.unsupported {
            if !self.unsupported.split(", ").any(|x| x == u) {
                if !self.unsupported.is_empty() {
                    self.unsupported.push_str(", ");
                }
                self.unsupported.push_str(u);
            }
        }
        // The quality score: the SNR margin, weighted by the share of good RS blocks.
        let nb = o.words.saturating_sub(self.last_rs.0);
        let nf = o.failed.saturating_sub(self.last_rs.1);
        self.last_rs = (o.words, o.failed);
        self.rs_hist.push_back((nb - nf.min(nb), nb));
        while self.rs_hist.len() > RS_HISTORY {
            self.rs_hist.pop_front();
        }
        let margin = snr - required.unwrap_or(0.0);
        if locked {
            let mut score = (25.0 + 12.5 * margin).clamp(0.0, 100.0);
            if fec_sync {
                let (g, n) = self
                    .rs_hist
                    .iter()
                    .fold((0u64, 0u64), |a, b| (a.0 + b.0, a.1 + b.1));
                let frac = if n > 0 { g as f32 / n as f32 } else { 1.0 };
                score *= frac * frac;
            } else {
                score *= 0.5;
            }
            self.quality += 0.25 * (score - self.quality);
        } else {
            self.quality = 0.0;
        }
        self.p.put(&DvbtFields {
            detect_level: i64::from(detect),
            channel_width: Hertz(f64::from(bw.mhz()) * 1e6),
            bandwidth_auto: w.auto,
            bandwidth_from: w.source,
            measured_width: w.measured_hz.map(Hertz),
            occupancy_db: w.occupancy_db,
            input_rate: Hertz(w.input_rate),
            natural_rate: Hertz(nat),
            resampling: (w.input_rate - nat).abs() > 1e-6 * nat || link.clock_ppm() != 0.0,
            rate_ok: w.input_rate >= bw.min_rate() * 0.9999,
            fft: mode,
            fft_size: t.fft.map(|(n, _)| n as i64),
            guard,
            guard_samples: t.fft.map(|(_, g)| g as i64),
            carriers: t.fft.map(|_| mode.map_or(0, |m| m.carriers() as i64)),
            constellation: tps.map(|(tp, _, _)| tp.constellation),
            hierarchy: tps.map(|(tp, _, _)| tp.hierarchy),
            code_rate_hp: tps.map(|(tp, _, _)| tp.rate_hp),
            code_rate_lp: tps.map(|(tp, _, _)| tp.rate_lp),
            stream: if self.lp {
                StreamChoice::Lp
            } else {
                StreamChoice::Hp
            },
            cell_id: tps
                .filter(|_| t.cell_known)
                .map(|(tp, _, _)| i64::from(tp.cell_id)),
            tps_length: tps.map(|(tp, _, _)| i64::from(tp.length)),
            dvbh_flags: tps.map(|(tp, _, _)| i64::from(tp.dvbh)),
            frame: tps.map(|(_, frame, _)| i64::from(frame)),
            symbol: tps.map(|(_, _, sym)| sym as i64),
            frame_duration: tps.map(|(tp, _, _)| Seconds(tp.frame_s(bw))),
            net_bitrate_bps: tps.map(|(tp, _, _)| tp.net_bitrate(bw, self.lp)),
            offset_hz: offset_hz.map(Hertz),
            integer_offset: t.fft.map(|_| i64::from(t.int_offset.unwrap_or(0))),
            locator_hz: Hertz(link.locator_hz()),
            clock_ppm: link.clock_ppm(),
            spectrum_inverted: t.mirrored,
            snr_db: data_valid.then_some(f64::from(snr)),
            mer_db: data_valid.then_some(f64::from(t.eq.mer_db)),
            ber_pre_viterbi: f.inner.ber_pre,
            ber_post_viterbi: f.ber_post,
            viterbi_margin: f64::from(f.inner.margin),
            puncture_phase: f.inner.phase.map(|x| x as i64),
            rs_blocks: o.words as i64,
            rs_clean: o.clean as i64,
            rs_corrected: o.corrected as i64,
            rs_failed: o.failed as i64,
            rs_bytes_corrected: o.bytes_corrected as i64,
            packets: o.delivered as i64,
            fec_sync,
            resyncs: t.resyncs as i64,
            since_tps: (t.tps_frames.0 > 0).then(|| Seconds((t.now - t.tps_good_time).max(0.0))),
            quality_pct: f64::from(self.quality),
            quality_label: quality_of(self.quality, locked),
            required_cn_db: required.map(f64::from),
            margin_db: locked.then_some(f64::from(margin)),
            unsupported: (!self.unsupported.is_empty()).then_some(self.unsupported.as_str()),
            data_valid,
            first_packet: t.first_packet.map(Seconds),
            level_dbfs: t.level_dbfs.map(f64::from),
        });
        // The status lock state and line.
        let s = &mut self.summary;
        s.clear();
        let _ = write!(
            s,
            "{}",
            RxState::CHOICES[usize::from(rx_state.index())].label
        );
        if let (Some(m), Some(g)) = (mode, guard) {
            if matches!(rx_state, RxState::Searching | RxState::Tracking) {
                let _ = write!(s, ", {} {}", m.text(), g.text());
            } else {
                let _ = write!(s, " {} {}", m.text(), g.text());
            }
        }
        if let Some((tp, _, _)) = tps {
            let _ = write!(
                s,
                " {} {}, {:.1} dB, {} services",
                tp.constellation.text(),
                tp.rate_hp.text(),
                snr,
                self.services
            );
        }
        let lock_state = match rx_state {
            RxState::Searching => LockState::Searching,
            RxState::Tracking => LockState::Tracking,
            RxState::Locked | RxState::Streaming => LockState::Locked,
        };
        let snr_db = data_valid.then_some(snr);
        let summary = &self.summary;
        self.p.results_mut().update_status(|x| {
            let same = x.lock_state == lock_state
                && x.snr_db == snr_db
                && x.level_dbfs == Some(level)
                && x.offset_hz == offset_hz
                && x.summary == *summary;
            if !same {
                x.lock_state = lock_state;
                x.snr_db = snr_db;
                x.level_dbfs = Some(level);
                x.offset_hz = offset_hz;
                x.summary.clear();
                x.summary.push_str(summary);
            }
            !same
        });
    }
}

impl Component for DvbtReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "dvbt.report",
            vec![stream::port("ts"), Port::typed::<ServiceList>("services")],
        )
        .with_settings(ReportSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.now = 0.0;
        self.report_t = f64::NEG_INFINITY;
        self.quality = 0.0;
        self.rs_hist.clear();
        self.last_rs = (0, 0);
        self.unsupported.clear();
        self.p
            .results_mut()
            .set_status(&Status::new(LockState::Searching).with_summary("Searching"));
        Ok(Vec::new())
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(l) = io.items::<ServiceList>(1).last() {
            self.services = l.value.services.len();
        }
        let _ = io.items::<TsPacket>(0);
        self.now += io.pass_seconds();
        if self.now - self.report_t >= REPORT_EVERY_S {
            self.report_t = self.now;
            self.publish();
        }
    }

    fn finish(&mut self, _io: &mut Io) {
        self.publish();
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReportSetting::Stream => self.lp = STREAM.read(id, v)? == StreamChoice::Lp,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let stream = if self.lp {
            StreamChoice::Lp
        } else {
            StreamChoice::Hp
        };
        values_of([(ReportSetting::Stream, Value::Choice(stream.index()))])
    }

    /// The fields and the services belong to the multiplex of the frequency before. A change of the sample rate keeps
    /// the services: the service list comes again only when it changes.
    fn forget(&mut self, _kept: &Kept) {
        self.services = 0;
        self.p.results_mut().fields_mut().clear();
        self.quality = 0.0;
        self.rs_hist.clear();
        self.unsupported.clear();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

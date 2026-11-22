//! What the ATSC 3.0 receiver publishes, and `atsc3.report`, the component that writes it.

use super::bootstrap::{min_time_ms, post_rate};
use super::components::{BootstrapFound, FrameState, LlsState, MediaState};
use super::frame::NotDecoded;
use super::layout::Pattern;
use super::rx::FrameInfo;
use super::svc::SltService;
use crate::status::{write_status, StatusLine, StepColour};
use crate::typed::{
    Atsc3ComponentRow, Atsc3Fields, Atsc3FrameFields, Atsc3PlpRow, Atsc3ServiceRow,
    ATSC3_COMPONENTS, ATSC3_PLPS, ATSC3_SERVICES,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::value::Choice;
use sdrkit_core::{ConfigError, LockState, Record, Results, ResultsDecl, TableId, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kept, Port};
use sdrkit_proto_mpegts::publish::{SERVICE_ROW, SERVICE_SELECT_KIND};
use std::fmt::Write;
use std::net::IpAddr;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The system bandwidth that a bootstrap signals.
    pub enum SystemBandwidth {
        /// 6 MHz.
        W6 = ("6", "6 MHz"),
        /// 7 MHz.
        W7 = ("7", "7 MHz"),
        /// 8 MHz.
        W8 = ("8", "8 MHz"),
        /// More than 8 MHz.
        More = ("more", "more than 8 MHz"),
    }
}

impl SystemBandwidth {
    /// The bandwidth of a bootstrap code (ATSC A/321 table 7.4).
    pub fn of(code: u8) -> SystemBandwidth {
        match code & 3 {
            0 => SystemBandwidth::W6,
            1 => SystemBandwidth::W7,
            2 => SystemBandwidth::W8,
            _ => SystemBandwidth::More,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The category of a service (ATSC A/331 table 6.4).
    pub enum ServiceCategory {
        /// Linear audio and video.
        Tv = ("tv", "TV"),
        /// Linear audio only.
        Radio = ("radio", "Radio"),
        /// An app-based service.
        App = ("app", "App"),
        /// The electronic service guide.
        Guide = ("guide", "Guide"),
        /// Emergency alerts.
        Alerts = ("alerts", "Alerts"),
        /// Rights management data.
        Drm = ("drm", "DRM"),
        /// A value the receiver does not know.
        Other = ("other", "Other"),
    }
}

impl ServiceCategory {
    /// The category of a `serviceCategory` code.
    pub fn of(code: u8) -> ServiceCategory {
        match code {
            1 => ServiceCategory::Tv,
            2 => ServiceCategory::Radio,
            3 => ServiceCategory::App,
            4 => ServiceCategory::Guide,
            5 => ServiceCategory::Alerts,
            6 => ServiceCategory::Drm,
            _ => ServiceCategory::Other,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How the service signalling is delivered (ATSC A/331 table 6.5).
    pub enum Delivery {
        /// ROUTE.
        Route = ("route", "ROUTE"),
        /// MMTP.
        Mmtp = ("mmtp", "MMTP"),
        /// A value the receiver does not know.
        Other = ("other", "Other"),
    }
}

impl Delivery {
    /// The delivery of an `slsProtocol` code.
    pub fn of(code: u8) -> Delivery {
        match code {
            1 => Delivery::Route,
            2 => Delivery::Mmtp,
            _ => Delivery::Other,
        }
    }
}

sdrkit_core::choice_enum! {
    /// What the receiver does with a service.
    pub enum ServiceAction {
        /// The service can be received.
        Receive = ("receive", "Receive"),
        /// The service is received, waiting for its signalling.
        Waiting = ("waiting", "Waiting for its signalling"),
        /// The service is received.
        Receiving = ("receiving", "Receiving"),
        /// The receiver cannot receive this delivery.
        Unsupported = ("unsupported", "Not supported"),
    }
}

/// What `atsc3.report` publishes: the result set and the two tables of the stack.
pub fn report_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(Atsc3Fields::SHAPE.fields)
        .with_fields(Atsc3FrameFields::SHAPE.fields)
        .with_table(ATSC3_SERVICES.def())
        .with_table(ATSC3_COMPONENTS.def())
        .with_table(ATSC3_PLPS.def())
}

/// What the ATSC 3.0 stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(super::ID).results_decl()
}

/// The service received: `chosen` when it is in the list, else (with no choice) the first visible linear TV service
/// delivered by ROUTE.
pub fn service_to_receive(chosen: Option<u16>, services: &[SltService]) -> Option<u16> {
    if let Some(c) = chosen {
        return services.iter().find(|s| s.id == c).map(|s| s.id);
    }
    services
        .iter()
        .find(|s| !s.hidden && s.category == 1 && s.sls_protocol == 1)
        .map(|s| s.id)
}

/// The signalling received for service `s` on TSI 0 sent to its SLS destination address and port: objects, documents read,
/// signed packages.
pub fn sls_counts(media: &MediaState, s: &SltService) -> (u64, u64, u64) {
    let Ok(dst) = s.sls_dst.parse::<IpAddr>() else {
        return (0, 0, 0);
    };
    media
        .sls
        .iter()
        .filter(|o| o.dst == dst && o.port == s.sls_port)
        .fold((0, 0, 0), |a, o| {
            (a.0 + o.objects, a.1 + o.documents, a.2 + o.signed_packages)
        })
}

sdrkit_core::commands! {
    /// The commands of `atsc3.report`.
    pub enum ReportCommand {
        /// Receive a service of the `services` table. An empty row receives the first visible linear TV service.
        ServiceSelect = SERVICE_SELECT_KIND.command,
    }
}

/// The period of the clock of the receiver graph, seconds of signal: the report ages the bootstraps and the frames by it.
pub const REPORT_S: f64 = 0.1;

/// The least time without a bootstrap (or without a decoded frame) after which the signal counts as lost, seconds. A longer
/// frame holds for [`HOLD_FRAMES`] of its length.
pub const HOLD_S: f64 = 0.5;
/// The frame lengths without a bootstrap after which the signal counts as lost.
pub const HOLD_FRAMES: f64 = 2.5;

/// `atsc3.report`: gathers the bootstraps, frames, service list and media into the results. Command `service.select`
/// chooses the service to receive. The state is Locked while frames decode, Tracking while bootstraps are found, and
/// Searching again when none came for [`HOLD_S`].
pub struct Report {
    p: Published,
    services_table: TableId,
    service: Option<u16>,
    last: Option<BootstrapFound>,
    bootstraps: u64,
    frame: FrameState,
    lls: LlsState,
    media: MediaState,
    signalled: Option<u16>,
    key: String,
    /// Clock periods since the last bootstrap and since the last decoded frame.
    quiet: (u32, u32),
    /// The state published last.
    seen: (bool, bool),
    /// The reason logged last, and whether the lock is logged.
    logged: (Option<NotDecoded>, bool),
    /// The signalling and the decoded PLP that the rows of the PLP table show.
    rows: (Option<Arc<FrameInfo>>, Option<u8>),
}

impl Default for Report {
    fn default() -> Self {
        let p = Published::from_decl(Arc::new(report_declaration()));
        let services_table = p
            .results()
            .decl()
            .table_id(ATSC3_SERVICES.key)
            .unwrap_or_else(|| unreachable!("the report declares the services table"));
        Report {
            p,
            services_table,
            service: None,
            last: None,
            bootstraps: 0,
            frame: FrameState::default(),
            lls: LlsState::default(),
            media: MediaState::default(),
            signalled: None,
            key: String::with_capacity(8),
            quiet: (0, 0),
            seen: (false, false),
            logged: (None, false),
            rows: (None, None),
        }
    }
}

impl Report {
    /// Starts again and publishes the empty state.
    fn reset(&mut self) {
        self.last = None;
        self.bootstraps = 0;
        self.frame = FrameState::default();
        self.lls = LlsState::default();
        self.media = MediaState::default();
        self.signalled = None;
        self.quiet = (0, 0);
        self.logged = (None, false);
        self.publish();
    }

    /// Whether a bootstrap came lately, and whether a frame decoded lately.
    fn recent(&self) -> (bool, bool) {
        let frame_s = self.last.map_or(0.0, |b| b.frame_s);
        let hold = (HOLD_S.max(HOLD_FRAMES * frame_s) / REPORT_S).ceil() as u32;
        (
            self.last.is_some() && self.quiet.0 <= hold,
            // A frame counts as decoded while a time de-interleaver fills: only a FEC block that came out shows a lock.
            self.frame.decoded > 0 && self.frame.packets > 0 && self.quiet.1 <= hold,
        )
    }

    fn publish(&mut self) {
        let (locked, decoding) = self.recent();
        self.seen = (locked, decoding);
        let receiving = service_to_receive(self.service, &self.lls.services);
        let p = &mut self.p;
        let bootstrap = self.last.map(|b| (b, b.fields));
        let info = self.frame.info.as_deref();
        let pattern = info.and_then(|i| Pattern::from_code(i.pilot_pattern));
        // The PLP that is decoded, when the last frame got as far as its cells.
        let coding = self
            .frame
            .coding
            .filter(|_| self.frame.packets + self.frame.bad_packets > 0);
        let plp = self.frame.plp_id.filter(|_| coding.is_some());
        let blocks = self.frame.packets + self.frame.bad_packets;
        // The ids of the PLPs of every subframe, one bit each.
        let ids = info
            .filter(|i| !i.plps.is_empty())
            .map(|i| i.plps.iter().fold(0u64, |m, p| m | 1 << (p.id & 63)));
        p.put(&Atsc3Fields {
            bootstrap_seen: locked,
            bootstraps: self.bootstraps as i64,
            frames_taken: self.frame.frames as i64,
            frames_ok: self.frame.decoded as i64,
            offset_hz: bootstrap.map(|(b, _)| Hertz(b.cfo_hz)),
            minor_version: bootstrap.map(|(_, f)| f.minor as i64),
            system_bandwidth: bootstrap.map(|(_, f)| SystemBandwidth::of(f.bandwidth)),
            preamble_structure: bootstrap.map(|(_, f)| f.preamble as i64),
            min_time_next: bootstrap.map(|(_, f)| Seconds(min_time_ms(f.min_time) as f64 / 1e3)),
            post_rate: bootstrap.map(|(_, f)| Hertz(post_rate(f.rate_n))),
            ea_wake_up: bootstrap.map(|(_, f)| f.ea_wake),
            frame_length: bootstrap.map(|(b, _)| Seconds(b.frame_s)),
            lls_tables: self.lls.tables as i64,
            signed_signalling: self.lls.signed || self.media.signed,
            leap_seconds: self
                .lls
                .system_time
                .as_ref()
                .map(|t| i64::from(t.utc_offset_s)),
            service: self.service.map(i64::from),
            service_received: receiving.map(i64::from),
            route_objects: self.media.objects as i64,
            media_samples: self.media.samples as i64,
            step_bootstrap: StepColour::of(locked, false),
            step_l1: StepColour::of(decoding, locked),
            step_service: StepColour::of(self.media.inits > 0, !self.lls.services.is_empty()),
        });
        p.put(&Atsc3FrameFields {
            frames_failed: (self.frame.frames - self.frame.decoded) as i64,
            not_decoded: self.frame.failed,
            fft_size: info.map(|i| i.fft.size() as i64),
            guard_samples: info.map(|i| i.guard as i64),
            l1_basic_mode: info.map(|i| i64::from(i.l1_basic_mode)),
            l1_detail_mode: info.map(|i| i64::from(i.l1_detail_mode)),
            preamble_symbols: info.map(|i| i64::from(i.preamble_symbols)),
            subframes: info.map(|i| i64::from(i.subframes)),
            subframe_symbols: info.map(|i| i64::from(i.symbols)),
            pilot_dx: pattern.map(|p| p.dx() as i64),
            pilot_dy: pattern.map(|p| p.dy() as i64),
            plps: ids.map(|n| i64::from(n.count_ones())),
            plp_id: plp.map(i64::from),
            plp_points: coding.map(|c| 1 << c.bits),
            plp_code_rate: coding.map(|c| i64::from(c.rate)),
            plp_ldpc_bits: coding.map(|c| c.ldpc_bits as i64),
            fec_blocks: i64::from(self.frame.last_blocks.0),
            fec_blocks_good: i64::from(self.frame.last_blocks.1),
            baseband_packets: self.frame.packets as i64,
            baseband_packets_bad: self.frame.bad_packets as i64,
            outer_corrected: self.frame.outer_corrected as i64,
            ldpc_iterations: (self.frame.last_blocks.0 > 0)
                .then_some(f64::from(self.frame.iterations)),
            mer_db: self.frame.mer_db.map(f64::from),
            pilot_snr_db: self.frame.pilot_snr_db.map(f64::from),
            step_plp: if blocks == 0 {
                StepColour::Off
            } else if self.frame.packets * 100 > blocks * 99 {
                StepColour::Green
            } else if self.frame.packets * 2 > blocks {
                StepColour::Amber
            } else {
                StepColour::Red
            },
        });
        // The rows change only with the signalling or the PLP that is decoded.
        let shown = (self.frame.info.clone(), plp);
        let same = match (&self.rows.0, &shown.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        } && self.rows.1 == shown.1;
        if !same {
            p.clear_rows(ATSC3_PLPS.def());
        }
        self.rows = shown;
        for row in info
            .filter(|_| !same)
            .map_or(&[][..], |i| &i.plps[..])
        {
            self.key.clear();
            let _ = write!(self.key, "{}.{}", row.subframe, row.id);
            let row = Atsc3PlpRow {
                subframe: i64::from(row.subframe),
                id: i64::from(row.id),
                layer: i64::from(row.layer),
                points: row.coding.map(|c| 1 << c.bits),
                code_rate: row.coding.map(|c| i64::from(c.rate)),
                ldpc_bits: row.coding.map(|c| c.ldpc_bits as i64),
                cells: i64::from(row.cells),
                time_interleaver: i64::from(row.ti_mode),
                decoded: plp == Some(row.id),
                bonding: row.bonding.map(i64::from),
                bonded_bsid: row.bonded_bsid.map(i64::from),
            };
            ATSC3_PLPS.put(p, &self.key, &row);
        }
        for (tsi, fourcc) in &self.media.components {
            self.key.clear();
            let _ = write!(self.key, "{tsi}");
            let fourcc = String::from_utf8_lossy(fourcc);
            let row = Atsc3ComponentRow {
                tsi: *tsi as i64,
                fourcc: &fourcc,
            };
            ATSC3_COMPONENTS.put(p, &self.key, &row);
        }
        for s in &self.lls.services {
            let delivery = Delivery::of(s.sls_protocol);
            let action = if delivery == Delivery::Mmtp {
                ServiceAction::Unsupported
            } else if Some(s.id) == receiving {
                if self.media.inits > 0 {
                    ServiceAction::Receiving
                } else {
                    ServiceAction::Waiting
                }
            } else {
                ServiceAction::Receive
            };
            self.key.clear();
            let _ = write!(self.key, "{}", s.id);
            let (objects, documents, signed) = sls_counts(&self.media, s);
            let row = Atsc3ServiceRow {
                major: i64::from(s.major),
                minor: i64::from(s.minor),
                name: &s.short_name,
                hidden: s.hidden,
                id: i64::from(s.id),
                category: ServiceCategory::of(s.category),
                delivery,
                action,
                sls_objects: objects as i64,
                sls_documents: documents as i64,
                sls_signed_packages: signed as i64,
            };
            ATSC3_SERVICES.put(p, &self.key, &row);
        }
        let r = p.results_mut();
        if receiving.is_some() && self.media.inits > 0 && self.signalled != receiving {
            if let Some(id) = receiving {
                r.log(format!("ATSC 3.0: signaling of service {id} received"));
            }
            self.signalled = receiving;
        }
        if self.frame.failed != self.logged.0 {
            if let Some(why) = self.frame.failed {
                r.log(format!("ATSC 3.0: frame not decoded: {}", why.label()));
            }
            self.logged.0 = self.frame.failed;
        }
        if decoding && !self.logged.1 {
            if let (Some(i), Some(plp), Some(c)) = (info, plp, coding) {
                let rest = if ids.is_some_and(|n| n.count_ones() > 1) {
                    "; the other PLPs are not decoded"
                } else {
                    ""
                };
                r.log(format!(
                    "ATSC 3.0: locked, {}K FFT, guard {} samples, PLP {}: {}{rest}",
                    i.fft.size() / 1024,
                    i.guard,
                    plp,
                    c.name()
                ));
            }
        }
        self.logged.1 = decoding;
        let (lock_state, summary) = if decoding {
            (LockState::Locked, "Locked - ATSC 3.0")
        } else if locked {
            (LockState::Tracking, "syncing - ATSC 3.0")
        } else {
            (LockState::Searching, "searching - ATSC 3.0")
        };
        write_status(
            r,
            &StatusLine {
                lock_state,
                snr_db: None,
                offset_hz: self.last.map(|b| b.cfo_hz),
                level_dbfs: None,
                summary,
            },
        );
    }

    /// Chooses the service of row `row` of the services table, or the automatic choice for an empty row.
    fn select(&mut self, row: &str) -> Result<(), CommandError> {
        self.service = if row.is_empty() {
            None
        } else {
            let id = row
                .parse::<u16>()
                .ok()
                .filter(|&id| self.lls.services.iter().any(|s| s.id == id))
                .ok_or_else(|| CommandError::NoSuchRow {
                    table: self.services_table,
                    key: row.to_owned(),
                })?;
            Some(id)
        };
        self.publish();
        Ok(())
    }
}

impl Component for Report {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "atsc3.report",
            vec![
                Port::typed::<BootstrapFound>("sync")
                    .optional("no bootstraps: the report shows the delivery layers only"),
                Port::typed::<FrameState>("frame").optional("no frames counted"),
                Port::typed::<LlsState>("lls"),
                Port::typed::<MediaState>("media"),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![])
    }
    /// The bootstraps, the frames, the services and the media are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.service = None;
        self.p.clear_rows(ATSC3_SERVICES.def());
        self.p.clear_rows(ATSC3_COMPONENTS.def());
        self.p.clear_rows(ATSC3_PLPS.def());
        self.reset();
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        if io.tick() {
            self.quiet = (
                self.quiet.0.saturating_add(1),
                self.quiet.1.saturating_add(1),
            );
            changed = self.recent() != self.seen;
        }
        for b in io.items::<BootstrapFound>(0) {
            self.quiet.0 = 0;
            if self.last.map(|l| l.fields) != Some(b.value.fields) {
                let f = b.value.fields;
                self.p.results_mut().log(format!(
                    "ATSC 3.0: bootstrap found, minor version {}, preamble structure {}, carrier offset {:+.1} Hz",
                    f.minor, f.preamble, b.value.cfo_hz
                ));
            }
            self.last = Some(b.value);
            self.bootstraps += 1;
            changed = true;
        }
        if let Some(f) = io.items::<FrameState>(1).last() {
            if f.value.decoded > self.frame.decoded {
                self.quiet.1 = 0;
            }
            self.frame.clone_from(&f.value);
            changed = true;
        }
        if let Some(l) = io.items::<LlsState>(2).last() {
            self.lls.clone_from(&l.value);
            changed = true;
        }
        if let Some(m) = io.items::<MediaState>(3).last() {
            self.media.clone_from(&m.value);
            changed = true;
        }
        if changed {
            self.publish();
        }
    }
    fn commands(&self) -> &Commands {
        ReportCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match ReportCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ReportCommand::ServiceSelect => self.select(args.arg(&SERVICE_ROW)?)?,
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

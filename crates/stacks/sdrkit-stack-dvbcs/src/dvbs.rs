//! The satellite TV stack `dvbs` (DVB-S and DVB-S2): the receiver of `sdrkit-proto-dvbs2`, the DVB-S FEC, the stream
//! adaptation and IP of `sdrkit-proto-ipcast` and the transport stream layer of `sdrkit-proto-mpegts`. The DVB-S and
//! DVB-S2 sender (`transmit`, [`crate::dvbs_transmit`]) works on the session's samples into the transmit sink (`tx`).
//!
//! Standard: ETSI EN 300 421; ETSI EN 302 307-1.

use crate::dvbs_transmit::{DvbsTransmit, TxSetting, COMMANDS};
use crate::TS_LAYER;
use sdrkit_algo_fec::conv::{Depuncturer, Viterbi};
use sdrkit_core::results::ResultsDecl;
use sdrkit_core::{
    Choice, ConfigError, Receiver, SettingSet, SignalType, StackConfig, Value, Values,
};
use sdrkit_graph::components::TxSink;
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Kind, Port,
    RateFrom, StackGraph, Stage, Timed,
};
use sdrkit_proto_dvbs2::inner::{inner_decoder, PunctureRate};
use sdrkit_proto_dvbs2::receive::Lost;
use sdrkit_proto_dvbs2::settings::DvbsSetting;
use sdrkit_proto_dvbs2::sync::S1Soft;
use sdrkit_proto_dvbs2::{sync_rule, Dvbs2Fec, DvbsFront, DvbsReport, DvbsSync};
use sdrkit_proto_ipcast::components::{BbFrameAdapt, IpComponent, SystemChoice};
use sdrkit_proto_mpegts::layer;
use sdrkit_proto_mpegts::monitor::TsMonitorComponent;
use sdrkit_proto_mpegts::outer::{sync_search, Dispersal, OuterDecoder, OuterStats};
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};

/// The stack id.
pub const ID: &str = "dvbs";

/// The lowest input rate.
pub const MIN_RATE: f64 = 2e6;

/// The period of the results, seconds of signal.
pub const REPORT_S: f64 = 0.25;

/// Input samples per graph pass.
pub const PASS: usize = 8192;

// ---------------------------------------------------------------------------------------------------------------------------------
// The DVB-S FEC.

/// `dvbs.fec1`: depuncturing and the Viterbi decoder of `sdrkit-algo-fec`, the packet alignment and the outer code. Output `ts`
/// is the transport stream, `lost` reports to the sync stage that the stream is lost, and `stats` carries the counters.
pub struct Dvbs1Fec {
    dep: Option<(PunctureRate, Depuncturer<'static>)>,
    /// The stream starts with a Y of the mother code, and its first value has not come yet.
    lead: bool,
    vit: Viterbi,
    mother: Vec<i8>,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    align: Option<(usize, bool)>,
    outer: OuterDecoder,
    searching: bool,
    ts: TsOut,
    log: Vec<String>,
}

impl Default for Dvbs1Fec {
    fn default() -> Self {
        Dvbs1Fec {
            dep: None,
            lead: false,
            vit: inner_decoder(),
            mother: Vec::new(),
            bits: Vec::new(),
            bytes: Vec::new(),
            align: None,
            outer: OuterDecoder::new(Dispersal::DvbGroups),
            searching: false,
            ts: TsOut::default(),
            log: Vec::new(),
        }
    }
}

impl Component for Dvbs1Fec {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbs.fec1",
            vec![Port::typed::<S1Soft>("in")],
            vec![
                stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Lost>("lost").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OuterStats>("stats").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = Dvbs1Fec::default();
        Ok(vec![
            inputs[0].with_kind(stream::kind()),
            Format::new(Kind::typed::<Lost>(), 0.0),
            Format::new(Kind::typed::<OuterStats>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut lost = false;
        let softs = io.items::<S1Soft>(0);
        let ts = io.items_out::<TsPacket>(0);
        for s in softs {
            let time = s.time;
            let s = &s.value;
            if s.restart {
                let d = Depuncturer::with_phase(s.rate.puncture(), s.rate.pattern_index(s.phase));
                self.dep = Some((s.rate, d));
                self.lead = s.rate.starts_with_y(s.phase);
                self.vit.reset();
                self.bits.clear();
                self.align = None;
                self.searching = true;
            }
            let Some((_, dep)) = self.dep.as_mut() else {
                continue;
            };
            self.mother.clear();
            // A stream that starts with a Y of the mother code: one erased X before it.
            if std::mem::take(&mut self.lead) {
                self.mother.push(0);
            }
            dep.push(&s.soft, &mut self.mother);
            self.vit.push(&self.mother, &mut self.bits);
            if self.searching {
                if self.bits.len() < 22 * 204 * 8 {
                    continue;
                }
                match sync_search(&self.bits, &sync_rule(20, true)) {
                    Some(h) => {
                        self.align = Some((h.start, h.inverted));
                        self.outer.restart();
                        self.searching = false;
                    }
                    None => {
                        lost = true;
                        self.dep = None;
                        self.searching = false;
                        self.log
                            .push("DVB-S: the packet alignment was not found".into());
                        continue;
                    }
                }
            }
            let Some((skip, inv)) = self.align else {
                continue;
            };
            // Whole bytes from the aligned bit stream.
            let start = skip.min(self.bits.len());
            let n = (self.bits.len() - start) / 8;
            self.bytes.clear();
            for k in 0..n {
                let v = self.bits[start + 8 * k..start + 8 * k + 8]
                    .iter()
                    .fold(0u8, |a, &b| (a << 1) | b);
                self.bytes.push(if inv { !v } else { v });
            }
            self.bits.drain(..start + 8 * n);
            self.align = Some((0, inv));
            let out = &mut self.ts;
            // A word that does not start with the sync byte is not a packet, and the monitor counts it as lost.
            self.outer.push(&self.bytes, &mut |w| {
                out.push(ts, &w.packet, time, false);
            });
            if self.outer.run_failed() >= 48 {
                lost = true;
                self.dep = None;
                self.align = None;
            }
        }
        if lost {
            self.log
                .push("DVB-S: no packet sync, searching again".into());
            io.items_out::<Lost>(1).push(Timed {
                time: 0.0,
                value: Lost,
            });
        }
        if io.tick() || lost {
            io.items_out::<OuterStats>(2).push(Timed {
                time: 0.0,
                value: self.outer.stats,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// What the DVB-S/S2 stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The graph of the satellite receiver at its default settings. The monitor merges the DVB-S stream (`fec1.ts`) and the DVB-S2
/// stream (`bbframe.ts`). The DVB-S2 FEC begins the second stage; the DVB-S FEC stays with the sync stage, which reads its
/// feedback.
pub fn builder() -> GraphBuilder {
    let b = GraphBuilder::new()
        .add("front", DvbsFront::default())
        .add("sync", DvbsSync::default())
        .add("fec2", Dvbs2Fec::default())
        .add("bbframe", BbFrameAdapt::default())
        .add("ip", IpComponent::default())
        .add("fec1", Dvbs1Fec::default())
        .add("monitor", TsMonitorComponent::new(2))
        .add("report", DvbsReport::default())
        .add("transmit", DvbsTransmit::default())
        .add("tx", TxSink::default())
        .input("front.in")
        .input("transmit.in")
        .connect("transmit.out", "tx.in")
        .connect("front.symbols", "sync.in")
        .feedback("sync.feedback", "front.feedback")
        .connect("sync.frames", "fec2.in")
        .stage("sync", "fec2", Stage::new(sdrkit_proto_dvbs2::FEC_STAGE))
        .connect("fec2.out", "bbframe.in")
        .connect("bbframe.ip", "ip.in")
        .connect("sync.soft", "fec1.in")
        .feedback("fec1.lost", "sync.lost")
        .connect("fec1.ts", "monitor.in")
        .connect("bbframe.ts", "monitor.in1")
        .connect("front.state", "report.front")
        .connect("sync.state", "report.sync")
        .connect("fec2.state", "report.fec2")
        .connect("fec1.stats", "report.fec1")
        .set("bbframe.system", Value::Choice(SystemChoice::DvbS2.index()));
    layer::attach(b, "monitor.out", &TS_LAYER)
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("report")
}

/// Where each stack setting goes.
const ROUTES: &[(&str, &[&str])] = &[
    ("symbol_rate", &["front.symbol_rate", "report.symbol_rate"]),
    ("roll_off", &["front.roll_off"]),
    ("carrier_pick", &["front.carrier_pick"]),
    ("standard", &["sync.standard"]),
    ("modcod_rule", &["fec2.modcod_rule"]),
    ("modcod", &["fec2.modcod"]),
    ("modcods", &["fec2.modcods"]),
    ("frame_length", &["fec2.frame_length"]),
    ("pilots", &["fec2.pilots"]),
    (
        "pl_scrambling",
        &["sync.pl_scrambling", "report.pl_scrambling"],
    ),
    ("isi", &["bbframe.stream"]),
    ("ip_pcap", &["ip.pcap"]),
    ("tx_polarisation", &["report.tx_polarisation"]),
    ("lnb_supply", &["report.lnb_supply"]),
    layer::DECODE_VIDEO_ROUTE,
];

/// The satellite receiver with the settings of `cfg`: the graph in fixed passes of [`PASS`] samples.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder()
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration())
        .build()?;
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "too low for DVB-S (2 Msps or more)")
        .fixed_passes(PASS)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for c in COMMANDS {
        b = b.command(c, sdrkit_graph::CommandTarget::of("transmit", c));
    }
    for (key, targets) in ROUTES {
        b = b.route(key, targets)?;
    }
    for s in TxSetting::schema().settings() {
        let key = &s.field.key;
        b = b.route(key, &[&format!("transmit.{key}")])?;
    }
    b.with_settings(&cfg.settings).build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, sdrkit_core::ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(
    cfg: &sdrkit_core::GenConfig,
) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
    Ok(Box::new(
        sdrkit_proto_dvbs2::gen::DvbsGenerator::from_config(cfg),
    ))
}

/// The satellite TV stack.
pub static DVBS: sdrkit_core::StackDescriptor = sdrkit_core::StackDescriptor {
    id: ID,
    name: "DVB-S/S2",
    group: "TV",
    blurb: "Satellite television and data: DVB-S and DVB-S2 (transport streams, GSE and IP), services, guide and media. Transmits a DVB-S or DVB-S2 transport stream that the host gives.",
    tuning: sdrkit_core::Tuning {
        min_hz: 10e6,
        max_hz: 6000e6,
        radio_filter_hz: Some(8e6),
        no_offset_tuning: true,
        ..sdrkit_core::Tuning::anywhere(1500e6, 10e6, MIN_RATE, 8e6)
    },
    submodes: &[],
    settings: DvbsSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::DvbS, SignalType::DvbS2],
    auto_gain: sdrkit_core::AutoGain::Radio,
    verify: Some(&crate::verify::DVBS_HOOKS),
    benchmarks: None,
};

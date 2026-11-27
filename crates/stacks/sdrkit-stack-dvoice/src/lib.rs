//! The digital voice stacks: DMR, NXDN, dPMR, TETRA (trunked and direct mode), P25 Phase 1 and 2, TETRAPOL, D-STAR, Yaesu System Fusion, M17, FreeDV, DCR and
//! the automatic detector. Each stack is a decoder and a voice monitor in one graph. Speech leaves as compressed frames for
//! the codec plug-ins of the host; [`mbe::register`] adds the decoders of the half-rate, full-rate and D-STAR coders to a
//! codec set. Encrypted content is never decoded. [`descriptors`] lists the stacks.

// Bit and sample loops index several slices at once. Plain index loops read more clearly there.
#![allow(clippy::needless_range_loop)]

pub mod auto;
pub mod bench;
pub mod bits;
pub mod calls;
pub mod conv;
pub mod dcr;
pub mod decoder;
pub mod dmr;
pub mod dmr_tx;
pub mod dpmr;
pub mod dqpsk;
pub mod dstar;
pub mod dstar_tx;
pub mod freedv;
pub mod freedv_ofdm;
pub mod freedv_tx;
pub mod front;
pub mod gen;
pub mod items;
pub mod m17;
pub mod m17_tx;
pub mod mbe;
pub mod monitor;
pub mod nxdn;
pub mod p25;
pub mod p25p2;
pub mod receiver;
pub mod results;
pub mod sender;
pub mod symbols;
pub mod tetra;
pub mod tetra_dmo;
pub mod tetrapol;
pub mod typed;
pub mod verify;
pub mod voice;
pub mod ysf;

use decoder::Protocol;
use front::{ChainSpec, DqpskChain, FskChain, Pulse};
use monitor::MonitorSetting;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, ResultsDecl, Schema,
    SettingSet, SignalType, StackConfig, StackDescriptor, SubMode, Tuning,
};
use sdrkit_graph::{GraphError, StackGraph};
use std::sync::{Arc, OnceLock};

const GROUP: &str = "Digital voice";

/// The tuning of a narrowband voice stack.
const fn narrow(
    min_mhz: f64,
    max_mhz: f64,
    default_hz: f64,
    rate: f64,
    bandwidth: f64,
    min_rate: f64,
    offset: f64,
) -> Tuning {
    Tuning {
        min_hz: min_mhz * 1e6,
        max_hz: max_mhz * 1e6,
        default_hz,
        rate_hz: rate,
        min_rate_hz: min_rate,
        bandwidth_hz: bandwidth,
        tune_offset_hz: offset,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    }
}

/// The C4FM chain of P25, System Fusion and NXDN: the transmitter shapes with a raised cosine, so the receiver takes a low-pass.
const fn c4fm() -> FskChain {
    FskChain {
        raster_hz: 12_500.0,
        chan_pass_hz: 5_000.0,
        chan_stop_hz: 9_000.0,
        pulse: Pulse::LowPass(3_200.0),
        baud: 4800.0,
        dc_removal: true,
        lane2: None,
    }
}

/// The demodulation chain of stack `id`, or `None` for an id of another family.
pub fn chain(id: &str) -> Option<ChainSpec> {
    let fsk = |raster_hz, chan_pass_hz, chan_stop_hz, pulse, baud, dc_removal| FskChain {
        raster_hz,
        chan_pass_hz,
        chan_stop_hz,
        pulse,
        baud,
        dc_removal,
        lane2: None,
    };
    Some(match id {
        "dmr" => ChainSpec::Fsk(fsk(
            12_500.0,
            5_400.0,
            9_500.0,
            Pulse::RootRaisedCosine(0.2),
            4800.0,
            true,
        )),
        "p25" | "ysf" => ChainSpec::Fsk(c4fm()),
        "nxdn" => ChainSpec::Fsk(FskChain {
            lane2: Some((Pulse::LowPass(1_600.0), 2400.0)),
            ..c4fm()
        }),
        "m17" => ChainSpec::Fsk(fsk(
            12_500.0,
            5_400.0,
            9_500.0,
            Pulse::RootRaisedCosine(0.5),
            4800.0,
            true,
        )),
        "dstar" => ChainSpec::Fsk(fsk(
            6_250.0,
            3_200.0,
            6_000.0,
            Pulse::IntegrateDump,
            4800.0,
            true,
        )),
        "dpmr" => ChainSpec::Fsk(fsk(
            6_250.0,
            3_000.0,
            5_000.0,
            Pulse::RootRaisedCosine(0.2),
            2400.0,
            true,
        )),
        "tetrapol" => ChainSpec::Fsk(fsk(
            12_500.0,
            4_500.0,
            7_000.0,
            Pulse::IntegrateDump,
            8000.0,
            false,
        )),
        "p25p2" => ChainSpec::Dqpsk(DqpskChain {
            raster_hz: 12_500.0,
            chan_pass_hz: 4_000.0,
            chan_stop_hz: 7_000.0,
            rate: 48_000.0,
            alpha: 0.2,
            baud: 6000.0,
        }),
        "tetra" | "tetra_dmo" => ChainSpec::Dqpsk(DqpskChain {
            raster_hz: 25_000.0,
            chan_pass_hz: 11_000.0,
            chan_stop_hz: 14_000.0,
            rate: tetra::STREAM_RATE,
            alpha: 0.35,
            baud: 18_000.0,
        }),
        "freedv" => ChainSpec::Fdmdv,
        "dcr" => ChainSpec::Fsk(dcr::chain_spec()),
        "dvoice-auto" => ChainSpec::Fsk(FskChain {
            lane2: Some((Pulse::RootRaisedCosine(0.2), 2400.0)),
            ..c4fm()
        }),
        _ => return None,
    })
}

fn build<P: Protocol>(proto: P, id: &str, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let spec = chain(id).unwrap_or(ChainSpec::Fsk(c4fm()));
    receiver::build(proto, spec, min_rate(id), cfg, branch(id, cfg))
}

/// The transmit branch of stack `id` with the configuration `cfg`, or `None` for a stack that does not send.
fn branch(id: &str, cfg: &StackConfig) -> Option<receiver::Branch> {
    match id {
        "m17" => Some(receiver::Branch {
            sender: Box::new(m17_tx::M17Transmit::default()),
            settings: <m17_tx::TxSetting as SettingSet>::SETTINGS,
            commands: &m17_tx::COMMANDS,
        }),
        "dmr" => Some(receiver::Branch {
            sender: Box::new(dmr_tx::DmrTransmit::default()),
            settings: <dmr_tx::TxSetting as SettingSet>::SETTINGS,
            commands: &dmr_tx::COMMANDS,
        }),
        "dstar" => Some(receiver::Branch {
            sender: Box::new(dstar_tx::DstarTransmit::default()),
            settings: <dstar_tx::TxSetting as SettingSet>::SETTINGS,
            commands: &dstar_tx::COMMANDS,
        }),
        "freedv" => Some(receiver::Branch {
            sender: Box::new(freedv_tx::FreedvTransmit::new(freedv::Submode::of(
                cfg.submode.as_deref(),
            ))),
            settings: <freedv_tx::TxSetting as SettingSet>::SETTINGS,
            commands: &freedv_tx::COMMANDS,
        }),
        _ => None,
    }
}

/// The receiver graph of stack `id`, or `None` for an id of another family. The voice monitor takes the codec plug-ins of
/// the host from the run context ([`StackGraph::set_codecs`]).
pub fn receiver_graph(id: &str, cfg: &StackConfig) -> Option<Result<StackGraph, GraphError>> {
    Some(match id {
        "dmr" => build(dmr::Dmr::default(), id, cfg),
        "nxdn" => build(nxdn::Nxdn::default(), id, cfg),
        "dpmr" => build(dpmr::Dpmr::default(), id, cfg),
        "tetra" => build(tetra::Tetra::default(), id, cfg),
        "tetra_dmo" => build(tetra_dmo::TetraDmo::default(), id, cfg),
        "p25" => build(p25::P25::default(), id, cfg),
        "p25p2" => build(p25p2::P25p2::default(), id, cfg),
        "tetrapol" => build(tetrapol::Tetrapol::default(), id, cfg),
        "dstar" => build(dstar::DStar::default(), id, cfg),
        "ysf" => build(ysf::Ysf::default(), id, cfg),
        "m17" => build(m17::M17::default(), id, cfg),
        "freedv" => build(freedv::FreeDv::new(freedv::Submode::of(cfg.submode.as_deref())), id, cfg),
        "dvoice-auto" => build(auto::Auto::default(), id, cfg),
        "dcr" => build(dcr::Dcr::default(), id, cfg),
        _ => return None,
    })
}

macro_rules! stack_fns {
    ($make:ident, $decl:ident, $id:literal, $proto:ty) => {
        fn $make(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
            Ok(Box::new(build(<$proto>::default(), $id, cfg)?))
        }
        fn $decl() -> ResultsDecl {
            monitor::declare(decoder::declaration(&<$proto>::default()))
        }
    };
}

fn min_rate(id: &str) -> f64 {
    descriptors()
        .iter()
        .find(|d| d.id == id)
        .map_or(96_000.0, |d| d.tuning.min_rate_hz)
}

stack_fns!(make_dmr, decl_dmr, "dmr", dmr::Dmr);
stack_fns!(make_nxdn, decl_nxdn, "nxdn", nxdn::Nxdn);
stack_fns!(make_dpmr, decl_dpmr, "dpmr", dpmr::Dpmr);
stack_fns!(make_tetra, decl_tetra, "tetra", tetra::Tetra);
stack_fns!(
    make_tetra_dmo,
    decl_tetra_dmo,
    "tetra_dmo",
    tetra_dmo::TetraDmo
);
stack_fns!(make_p25, decl_p25, "p25", p25::P25);
stack_fns!(make_p25p2, decl_p25p2, "p25p2", p25p2::P25p2);
stack_fns!(make_tetrapol, decl_tetrapol, "tetrapol", tetrapol::Tetrapol);
stack_fns!(make_dstar, decl_dstar, "dstar", dstar::DStar);
stack_fns!(make_ysf, decl_ysf, "ysf", ysf::Ysf);
stack_fns!(make_m17, decl_m17, "m17", m17::M17);
fn make_freedv(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    let proto = freedv::FreeDv::new(freedv::Submode::of(cfg.submode.as_deref()));
    Ok(Box::new(build(proto, "freedv", cfg)?))
}
fn decl_freedv() -> ResultsDecl {
    monitor::declare(decoder::declaration(&freedv::FreeDv::default()))
}
stack_fns!(make_auto, decl_auto, "dvoice-auto", auto::Auto);
stack_fns!(make_dcr, decl_dcr, "dcr", dcr::Dcr);

/// The settings of the DMR stack: the monitor's, the colour code of the test signal and the sender's.
fn dmr_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[
                MonitorSetting::SETTINGS,
                &[&dmr::GEN_CC],
                <dmr_tx::TxSetting as SettingSet>::SETTINGS,
            ]
            .concat(),
        ))
    }))
}

/// The settings of the P25 stack: the monitor's and the NAC of the test signal.
fn p25_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[MonitorSetting::SETTINGS, &[&p25::GEN_NAC]].concat(),
        ))
    }))
}

/// The settings of the System Fusion stack: the monitor's and the voice type of the test signal.
fn ysf_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[MonitorSetting::SETTINGS, &[&ysf::GEN_VOICE]].concat(),
        ))
    }))
}

/// DMR (Tiers I, II and III).
pub static DMR: StackDescriptor = StackDescriptor {
    id: "dmr",
    name: "DMR",
    group: GROUP,
    blurb: "DMR two-slot TDMA: calls with their speech, talkgroups, talker aliases, text messages and positions. Transmits voice from the microphone and text messages as short data in direct mode.",
    tuning: sdrkit_core::Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(30.0, 1000.0, 0.0, 2.4e6, 12_500.0, 96_000.0, 0.0)
    },
    submodes: &[],
    settings: dmr_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dmr],
    auto_gain: AutoGain::Radio,
    make_receiver: make_dmr,
    make_generator: Some(gen_dmr),
    verify: Some(&verify::HOOKS),
    benchmarks: Some(verify::benchmarks),
};

/// P25 Phase 1.
pub static P25: StackDescriptor = StackDescriptor {
    id: "p25",
    name: "P25",
    group: GROUP,
    blurb: "APCO P25 Phase 1: voice calls with their speech, talkgroups, encryption IDs, and the trunking control channel (sites, grants).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(30.0, 1000.0, 0.0, 2e6, 12_500.0, 96_000.0, 50_000.0)
    },
    submodes: &[],
    settings: p25_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::P25Phase1],
    auto_gain: AutoGain::Radio,
    make_receiver: make_p25,
    make_generator: Some(gen_p25),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// The settings of the M17 stack: the monitor's and the sender's.
fn m17_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[
                MonitorSetting::SETTINGS,
                <m17_tx::TxSetting as SettingSet>::SETTINGS,
            ]
            .concat(),
        ))
    }))
}

/// M17.
pub static M17: StackDescriptor = StackDescriptor {
    id: "m17",
    name: "M17",
    group: GROUP,
    blurb: "M17 amateur digital voice: callsigns, text, positions, SMS packets; Codec 2 speech through a plug-in. Transmits voice from the microphone (Codec 2 3200) and SMS or data packets.",
    tuning: sdrkit_core::Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(28.0, 1300.0, 0.0, 2e6, 12_500.0, 96_000.0, 50_000.0)
    },
    submodes: &[],
    settings: m17_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::M17],
    auto_gain: AutoGain::Radio,
    make_receiver: make_m17,
    make_generator: Some(gen_m17),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// The settings of the D-STAR stack: the monitor's and the sender's.
fn dstar_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[MonitorSetting::SETTINGS, <dstar_tx::TxSetting as SettingSet>::SETTINGS].concat(),
        ))
    }))
}

/// D-STAR.
pub static DSTAR: StackDescriptor = StackDescriptor {
    id: "dstar",
    name: "D-STAR",
    group: GROUP,
    blurb: "D-STAR digital voice: voice calls with their speech, callsigns and repeaters from the header, slow-data messages and positions. Transmits voice from the microphone and slow-data texts, with the radio header.",
    tuning: sdrkit_core::Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(28.0, 1300.0, 0.0, 2e6, 6_250.0, 96_000.0, 50_000.0)
    },
    submodes: &[],
    settings: dstar_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::DStar],
    auto_gain: AutoGain::Radio,
    make_receiver: make_dstar,
    make_generator: Some(gen_dstar),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// NXDN (NXDN48 and NXDN96).
pub static NXDN: StackDescriptor = StackDescriptor {
    id: "nxdn",
    name: "NXDN",
    group: GROUP,
    blurb: "NXDN 6.25 and 12.5 kHz: calls with their speech, status and short data, RAN, and the site of a trunked control channel.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(30.0, 1000.0, 0.0, 2e6, 12_500.0, 96_000.0, 50_000.0)
    },
    submodes: &[],
    settings: MonitorSetting::schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Nxdn96, SignalType::Nxdn48],
    auto_gain: AutoGain::Radio,
    make_receiver: make_nxdn,
    make_generator: Some(gen_nxdn),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// dPMR (including PMR446 digital).
pub static DPMR: StackDescriptor = StackDescriptor {
    id: "dpmr",
    name: "dPMR",
    group: GROUP,
    blurb: "dPMR and PMR446 digital: calls with their speech, colour codes, status, short messages and packet data.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(
        30.0,
        1000.0,
        0.0,
        2e6,
        6_250.0,
        96_000.0,
        50_000.0,
    )
    },
    submodes: &[],
    settings: MonitorSetting::schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dpmr],
    auto_gain: AutoGain::Radio,
    make_receiver: make_dpmr,
    make_generator: Some(gen_dpmr),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// Yaesu System Fusion.
pub static YSF: StackDescriptor = StackDescriptor {
    id: "ysf",
    name: "Yaesu System Fusion",
    group: GROUP,
    blurb: "System Fusion (C4FM): calls with their speech in V/D mode 1, V/D mode 2 and voice full rate, callsigns, data type, text and GPS from the data channel.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(28.0, 1300.0, 0.0, 2e6, 12_500.0, 96_000.0, 50_000.0)
    },
    submodes: &[],
    settings: ysf_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::YaesuFusion],
    auto_gain: AutoGain::Radio,
    make_receiver: make_ysf,
    make_generator: Some(gen_ysf),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// P25 Phase 2.
pub static P25P2: StackDescriptor = StackDescriptor {
    id: "p25p2",
    name: "P25 Phase 2",
    group: GROUP,
    blurb: "APCO P25 Phase 2 TDMA: two voice channels per carrier with their speech, talkgroups, encryption IDs and the network status.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        ..narrow(30.0, 1000.0, 0.0, 2e6, 12_500.0, 96_000.0, 50_000.0)
    },
    submodes: &[],
    settings: MonitorSetting::schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::P25Phase2],
    auto_gain: AutoGain::Radio,
    make_receiver: make_p25p2,
    make_generator: Some(gen_p25p2),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// TETRAPOL.
pub static TETRAPOL: StackDescriptor = StackDescriptor {
    id: "tetrapol",
    name: "TETRAPOL",
    group: GROUP,
    blurb: "TETRAPOL (GMSK 8 kbit/s): system identity and calls; encrypted calls are shown without identities.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 68e6,
        max_hz: 520e6,
        default_hz: 0.0,
        rate_hz: 250_000.0,
        min_rate_hz: 64_000.0,
        bandwidth_hz: 200_000.0,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: Some(200_000.0),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: MonitorSetting::schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Tetrapol],
    auto_gain: AutoGain::Radio,
    make_receiver: make_tetrapol,
    make_generator: Some(gen_tetrapol),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// TETRA (downlink).
pub static TETRA: StackDescriptor = StackDescriptor {
    id: "tetra",
    name: "TETRA",
    group: GROUP,
    blurb: "TETRA downlink: the cell (MCC, MNC, colour code, location area) and its calls; encrypted calls without identities.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 136e6,
        max_hz: 960e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 200_000.0,
        bandwidth_hz: 25_000.0,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: MonitorSetting::schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Tetra],
    auto_gain: AutoGain::Radio,
    make_receiver: make_tetra,
    make_generator: Some(gen_tetra),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};


/// TETRA Direct Mode Operation.
pub static TETRA_DMO: StackDescriptor = StackDescriptor {
    id: tetra_dmo::ID,
    name: "TETRA DMO",
    group: GROUP,
    blurb: "TETRA direct mode on one channel: synchronisation and normal bursts, DM messages, addresses and calls, and the speech of clear calls; encrypted calls are marked, never decoded.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 30e6,
        max_hz: 1000e6,
        default_hz: 0.0,
        rate_hz: 240e3,
        min_rate_hz: 72e3,
        bandwidth_hz: 25_000.0,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: tetra_dmo_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    make_receiver: make_tetra_dmo,
    make_generator: Some(gen_tetra_dmo),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// The settings of the FreeDV stack: the monitor's, the clipper of the test signal and the sender's.
fn freedv_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[
                MonitorSetting::SETTINGS,
                &[&freedv_ofdm::GEN_CLIP],
                <freedv_tx::TxSetting as SettingSet>::SETTINGS,
            ]
            .concat(),
        ))
    }))
}

/// FreeDV.
pub static FREEDV: StackDescriptor = StackDescriptor {
    id: "freedv",
    name: "FreeDV",
    group: GROUP,
    blurb: "FreeDV on HF (upper sideband): the 1600, 700D and 700E waveforms with their callsign text; Codec 2 speech through a plug-in. RADE is not received. Transmits voice from the microphone on 1600 (Codec 2 1300), 700D or 700E (Codec 2 700C) with the callsign text.",
    tuning: Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        tune_offset_hz: 20_000.0,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(0.0, 2e6, 96_000.0, 3_000.0)
    },
    submodes: &[
        SubMode { id: "auto", name: "Automatic (700D, 700E and 1600)", default_hz: 0.0, ..SubMode::DEFAULT },
        SubMode { id: "1600", name: "FreeDV 1600", default_hz: 0.0, ..SubMode::DEFAULT },
        SubMode { id: "700D", name: "FreeDV 700D", default_hz: 0.0, ..SubMode::DEFAULT },
        SubMode { id: "700E", name: "FreeDV 700E", default_hz: 0.0, ..SubMode::DEFAULT },
        SubMode { id: "rade", name: "FreeDV RADE (not received)", default_hz: 0.0, ..SubMode::DEFAULT },
    ],
    settings: freedv_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::FreeDv],
    auto_gain: AutoGain::Radio,
    make_receiver: make_freedv,
    make_generator: Some(gen_freedv),
    verify: Some(&verify::FREEDV_HOOKS),
    benchmarks: Some(verify::freedv_benchmarks),
};

fn gen_dmr(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(dmr::generator(cfg)))
}
fn gen_dpmr(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(dpmr::generator(cfg)))
}
fn gen_nxdn(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(nxdn::generator(cfg)))
}
fn gen_p25(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(p25::generator(cfg)))
}
fn gen_p25p2(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(p25p2::generator(cfg)))
}
fn gen_m17(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(m17::generator(cfg)))
}
fn gen_ysf(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(ysf::generator(cfg)))
}
fn gen_dstar(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(dstar::generator(cfg)))
}
fn gen_tetra(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(tetra::generator(cfg)))
}
fn gen_tetra_dmo(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(tetra_dmo::DmoGenerator::new(
        cfg,
        -TETRA_DMO.tuning.tune_offset_hz,
    )))
}

/// The settings of the TETRA DMO stack: the voice monitor's and the test signal's.
pub fn tetra_dmo_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[MonitorSetting::SETTINGS, tetra_dmo::settings::SETTINGS].concat(),
        ))
    }))
}

fn gen_tetrapol(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(tetrapol::generator(cfg)))
}
fn gen_freedv(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(freedv::generator(cfg)))
}

/// The automatic detector's test signal: a DMR, a P25 and a dPMR transmission one after the other.
fn auto_generator(cfg: &GenConfig) -> gen::Prepared {
    let mut sig = dmr::DmrGen::default_schedule(1).render(cfg.seed);
    sig.extend(p25::render(0x293, cfg.seed));
    sig.extend(dpmr::render());
    gen::Prepared::new(sig, cfg.rate_hz, true).with_info(12_500.0, 4800.0, 9600.0)
}

fn make_auto_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(auto_generator(cfg)))
}

/// One pass of the test signal of stack `id` (it does not repeat), for tests and tools that need its exact length. `None` for
/// an id of another family.
pub fn test_signal(id: &str, cfg: &GenConfig) -> Option<gen::Prepared> {
    let g = match id {
        "dmr" => dmr::generator(cfg),
        "nxdn" => nxdn::generator(cfg),
        "dpmr" => dpmr::generator(cfg),
        "tetra" => tetra::generator(cfg),
        "tetra_dmo" => tetra_dmo::prepared(cfg),
        "p25" => p25::generator(cfg),
        "p25p2" => p25p2::generator(cfg),
        "tetrapol" => tetrapol::generator(cfg),
        "dstar" => dstar::generator(cfg),
        "ysf" => ysf::generator(cfg),
        "m17" => m17::generator(cfg),
        "freedv" => freedv::generator(cfg),
        "dvoice-auto" => auto_generator(cfg),
        "dcr" => dcr::gen::generator(cfg),
        _ => return None,
    };
    Some(g.once())
}

/// Digital voice (auto).
pub static AUTO: StackDescriptor = StackDescriptor {
    id: "dvoice-auto",
    name: "Digital voice (auto)",
    group: GROUP,
    blurb: "Finds which digital voice mode is on the channel (DMR, NXDN, dPMR, P25, Fusion, M17) and decodes it.",
    tuning: Tuning { kind: Some(sdrkit_core::TuningKind::Free), radio_filter_hz: None, ..Tuning::anywhere(0.0, 2.4e6, 96_000.0, 12_500.0) },
    submodes: &[],
    settings: MonitorSetting::schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dmr, SignalType::Nxdn96, SignalType::Nxdn48, SignalType::Dpmr, SignalType::P25Phase1, SignalType::YaesuFusion, SignalType::M17],
    auto_gain: AutoGain::Radio,
    make_receiver: make_auto,
    make_generator: Some(make_auto_gen),
    verify: Some(&verify::HOOKS),
    benchmarks: None,
};

/// The settings of the DCR stack: the voice monitor's, the decoder's and the test signal's.
fn dcr_schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(S.get_or_init(|| {
        Arc::new(Schema::of(
            &[MonitorSetting::SETTINGS, dcr::SETTINGS].concat(),
        ))
    }))
}

fn gen_dcr(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(dcr::gen::generator(cfg)))
}

/// DCR, the digital convenience radio of Japan (ARIB STD-T98 volume 3). The host sets the frequency; the plan is a data file
/// that the host reads.
pub static DCR: StackDescriptor = StackDescriptor {
    id: dcr::ID,
    name: "DCR (Japan digital convenience radio)",
    group: GROUP,
    blurb: "Japanese digital convenience radio (four-level FSK): calls by user code, call signs, manufacturer messages and data blocks, on one channel or every plan channel in the span; voice frames are counted, not played.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 150e6,
        max_hz: 470e6,
        default_hz: 0.0,
        rate_hz: 250e3,
        min_rate_hz: 48e3,
        bandwidth_hz: 12_500.0,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(200e3),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: dcr_schema,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    make_receiver: make_dcr,
    make_generator: Some(gen_dcr),
    verify: Some(&verify::HOOKS),
    benchmarks: Some(verify::dcr_benchmarks),
};

/// Every digital voice stack.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &DMR, &NXDN, &DPMR, &TETRA, &P25, &P25P2, &TETRAPOL, &DSTAR, &YSF, &M17, &FREEDV, &AUTO,
        &TETRA_DMO, &DCR,
    ]
}

/// The declaration of stack `id` (empty for an id of another family).
pub fn declaration(id: &str) -> ResultsDecl {
    match id {
        "dmr" => decl_dmr(),
        "nxdn" => decl_nxdn(),
        "dpmr" => decl_dpmr(),
        "tetra" => decl_tetra(),
        "tetra_dmo" => decl_tetra_dmo(),
        "p25" => decl_p25(),
        "p25p2" => decl_p25p2(),
        "tetrapol" => decl_tetrapol(),
        "dstar" => decl_dstar(),
        "ysf" => decl_ysf(),
        "m17" => decl_m17(),
        "freedv" => decl_freedv(),
        "dvoice-auto" => decl_auto(),
        "dcr" => decl_dcr(),
        _ => ResultsDecl::default(),
    }
}

//! HF data and signals: HF ALE (2G decoded), 3G-ALE, PACTOR-I (decoded), PACTOR-II, -III and -IV and Robust Packet (physical
//! layer), the serial-tone
//! modems of MIL-STD-188-110, STANAG 4285 and STANAG 4539, the naval broadcast FSK of STANAG 4481, the structure of Link 11, the
//! HF waveform identifier, the HF beacon propagation monitor, the HF ocean radar monitor and the wave-buoy HF link.
//!
//! | Id | Stack | Module |
//! |---|---|---|
//! | `ale` | HF ALE (2G) | [`ale`] |
//! | `ale3g` | 3G-ALE | [`ale3g`] |
//! | `pactor1` | PACTOR-I (experimental) | [`pactor1`] |
//! | `pactor2`, `pactor3`, `pactor4` | PACTOR-II, -III, -IV (physical layer only) | [`pactorhf`] |
//! | `rpr` | Robust Packet (physical layer only) | [`rpr`] |
//! | `ms110` | MIL-STD-188-110 serial tone | [`ms110`] |
//! | `s4285` | STANAG 4285 | [`s4285`] |
//! | `s4539` | STANAG 4539 high-rate serial tone | [`s4539`] |
//! | `s4481` | STANAG 4481 FSK | [`s4481`] |
//! | `link11` | Link 11 CLEW (structure only) | [`link11`] |
//! | `arqe` | ARQ-E / ARQ-N | [`arqe`] |
//! | `autospec` | AUTOSPEC / SPREAD | [`autospec`] |
//! | `efr` | EFR teleswitch | [`efr`] |
//! | `piccolo` | Piccolo Mk VI | [`piccolo`] |
//! | `coquelet` | Coquelet-8 | [`coquelet`] |
//! | `cis_hf` | CIS HF data signals (three read, four identified only) | [`cis`] |
//! | `gtor` | G-TOR monitor | [`gtor`] |
//! | `hfid` | HF waveform ID (identification only) | [`hfid`] |
//! | `hfbeacons` | HF beacons and the propagation dashboard | [`beacons`] |
//! | `hfradar` | CODAR / HF ocean radar | [`radar`] |
//! | `wavebuoy` | Wave-buoy HF link (Datawell) | [`buoy`] |
//! | `crowd36` | CROWD36 34-tone MFSK | [`crowd36`] |
//! | `xpa` | XPA and XPA2 number senders | [`xpa`] |
//! | `fsk200` | FSK200/500 (F01) and FSK200/1000 (F06, F06a) | [`fsk200`] |
//! | `gwfsk` | GW FSK | [`gwfsk`] |
//! | `fsk_modems` | Telephone FSK modems (Bell 103, V.21, V.23, Bell 202) | [`fsk_modems`] |
//! | `uvb76` | UVB-76 buzzer monitor (timing only) | [`uvb76`] |
//! | `vlf_msk` | VLF MSK transmitter monitor (measures only) | [`vlf_msk`] |
//!
//! Parts that several stacks share (the HF channel front end, sideband audio, Listen, the serial-tone parts, the teleprinter
//! decoder) are in [`frontend`], [`sideband`], [`listen`], [`serial`] and [`teleprinter`]; the HF sender stacks share [`senders`];
//! the transmitters of `ale` and `hfbeacons` share [`transmit`]. Every receiver is a stack graph, and every stack has a test
//! signal.

pub mod ale;
pub mod ale3g;
pub mod arqe;
pub mod autospec;
pub mod beacons;
pub mod buoy;
pub mod cis;
pub mod coquelet;
pub mod crowd36;
pub mod efr;
pub mod frontend;
pub mod fsk200;
pub mod fsk_modems;
pub mod gtor;
pub mod gwfsk;
pub mod hfid;
pub mod link11;
pub mod listen;
pub mod ms110;
pub mod pactor1;
pub mod pactorhf;
pub mod piccolo;
pub mod radar;
pub mod receiver;
pub mod rpr;
pub mod s4285;
pub mod s4481;
pub mod s4539;
pub mod senders;
pub mod serial;
pub mod sideband;
pub mod teleprinter;
pub mod transmit;
pub mod typed;
pub mod util;
pub mod uvb76;
pub mod verify;
pub mod vlf_msk;
pub mod xpa;

use sdrkit_core::{
    BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor,
    Values,
};
use sdrkit_graph::Registry;

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &ale::ALE,
        &pactor1::PACTOR1,
        &buoy::WAVEBUOY,
        &radar::HFRADAR,
        &beacons::HFBEACONS,
        &pactorhf::PACTOR2,
        &pactorhf::PACTOR3,
        &pactorhf::PACTOR4,
        &hfid::HFID,
        &ms110::MS110,
        &s4285::S4285,
        &s4539::S4539,
        &ale3g::ALE3G,
        &s4481::S4481,
        &link11::LINK11,
        &efr::EFR,
        &gtor::GTOR,
        &arqe::ARQE,
        &autospec::AUTOSPEC,
        &piccolo::PICCOLO,
        &coquelet::COQUELET,
        &cis::CIS_HF,
        &crowd36::CROWD36,
        &xpa::XPA,
        &fsk200::FSK200,
        &gwfsk::GWFSK,
        &fsk_modems::FSK_MODEMS,
        &uvb76::UVB76,
        &vlf_msk::VLF_MSK,
        &rpr::RPR,
    ]
}

/// Adds every component kind of the crate to `r` as `hfdata.<name>`, so a graph can be built from a description.
pub fn register(r: &mut Registry) {
    use crate::frontend::HfChannelFrontEnd;
    use crate::listen::Listen;
    use crate::sideband::SidebandAudio;
    r.add("hfdata.hf_front", || Box::new(HfChannelFrontEnd::default()));
    r.add("hfdata.sideband_audio", || {
        Box::new(SidebandAudio::default())
    });
    r.add("hfdata.listen", || Box::new(Listen::default()));
    r.add("hfdata.ale_word_sync", || {
        Box::new(ale::sync::AleWordSync::default())
    });
    r.add("hfdata.ale_call_assembler", || {
        Box::new(ale::calls::CallAssembler::default())
    });
    r.add("hfdata.ale_tables", || {
        Box::new(ale::tables::AleTables::default())
    });
    r.add("hfdata.ale_tx", || {
        Box::new(ale::tx::AleTransmitter::default())
    });
    r.add("hfdata.hfb_tx", || {
        Box::new(beacons::tx::BeaconTransmitter::default())
    });
    r.add("hfdata.pactor_pair_finder", || {
        Box::new(pactor1::rx::TonePairFinder::default())
    });
    r.add("hfdata.pactor_burst_detector", || {
        Box::new(pactor1::rx::BurstDetectorComponent::default())
    });
    r.add("hfdata.pactor_burst_reader", || {
        Box::new(pactor1::rx::BurstReaderComponent::default())
    });
    r.add("hfdata.pactor_link", || {
        Box::new(pactor1::link::PactorLink::default())
    });
    r.add("hfdata.pactorhf_buffer", || {
        Box::new(pactorhf::rx::AudioBuffer::new(2))
    });
    r.add("hfdata.pactorhf_results", || {
        Box::new(pactorhf::rx::PactorHfResults::new(2))
    });
    r.add("hfdata.rpr_buffer", || {
        Box::new(rpr::rx::RprBuffer::default())
    });
    r.add("hfdata.rpr_results", || {
        Box::new(rpr::rx::RprResults::default())
    });
    r.add("hfdata.buoy_front", || {
        Box::new(buoy::rx::BuoyFront::default())
    });
    r.add("hfdata.buoy_carrier", || {
        Box::new(buoy::rx::CarrierFinder::default())
    });
    r.add("hfdata.buoy_fsk_demod", || {
        Box::new(buoy::rx::FskDemod::default())
    });
    r.add("hfdata.hxv_decoder", || {
        Box::new(buoy::rx::HxvDecoder::default())
    });
    r.add("hfdata.buoy_frame_analyser", || {
        Box::new(buoy::rx::FrameAnalyser::default())
    });
    r.add("hfdata.buoy_results", || {
        Box::new(buoy::results::BuoyResults::default())
    });
    r.add("hfdata.hfid_front", || Box::new(hfid::HfidFront::default()));
    r.add("hfdata.hfid_analyser", || {
        Box::new(hfid::HfidAnalyser::default())
    });
    r.add("hfdata.hfid_view", || Box::new(hfid::HfidView::default()));
    r.add("hfdata.hfradar_detector", || {
        Box::new(radar::detect::SweepDetector::default())
    });
    r.add("hfdata.hfradar_radars", || {
        Box::new(radar::radars::RadarTable::default())
    });
    r.add("hfdata.hfb_stage1", || {
        Box::new(beacons::rx::BeaconBank::default())
    });
    r.add("hfdata.hfb_stage2", || {
        Box::new(beacons::rx::BeaconMeter::default())
    });
    r.add("hfdata.hfb_runs", || {
        Box::new(beacons::rx::RunDetector::default())
    });
    r.add("hfdata.hfb_judges", || {
        Box::new(beacons::rx::KeyingJudges::default())
    });
    r.add("hfdata.ms110", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            ms110::rx::Ms110Decoder::new(),
        ))
    });
    r.add("hfdata.s4285", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            s4285::rx::S4285Decoder::new(),
        ))
    });
    r.add("hfdata.s4539", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            s4539::rx::S4539Decoder::new(),
        ))
    });
    r.add("hfdata.ale3g", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            ale3g::rx::Ale3gDecoder::new(),
        ))
    });
    r.add("hfdata.s4481", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            s4481::rx::S4481Decoder::new(),
        ))
    });
    r.add("hfdata.link11", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            link11::rx::Link11Decoder::new(),
        ))
    });
    r.add("hfdata.efr", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            teleprinter::TtyDecoder::new(efr::Efr::new()),
        ))
    });
    r.add("hfdata.gtor", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            teleprinter::TtyDecoder::new(gtor::Gtor::new()),
        ))
    });
    r.add("hfdata.arqe", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            teleprinter::TtyDecoder::new(arqe::Arqe::new()),
        ))
    });
    r.add("hfdata.autospec", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            teleprinter::TtyDecoder::new(autospec::Autospec::new()),
        ))
    });
    r.add("hfdata.piccolo", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            teleprinter::TtyDecoder::new(piccolo::Piccolo::new()),
        ))
    });
    r.add("hfdata.coquelet", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            teleprinter::TtyDecoder::new(coquelet::Coquelet::new()),
        ))
    });
    r.add("hfdata.cis_hf", || {
        Box::new(sdrkit_graph::DecoderComponent::new(cis::CisDecoder::new()))
    });
    r.add("hfdata.crowd36", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            crowd36::Crowd36::new(),
        )))
    });
    r.add("hfdata.xpa", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            xpa::Xpa::new(),
        )))
    });
    r.add("hfdata.fsk200", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            fsk200::Fsk200::new(),
        )))
    });
    r.add("hfdata.gwfsk", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            gwfsk::Gwfsk::new(),
        )))
    });
    r.add("hfdata.fsk_modems", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            fsk_modems::FskModems::new(),
        )))
    });
    r.add("hfdata.uvb76", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            uvb76::Uvb76::new(),
        )))
    });
    r.add("hfdata.vlf_msk", || {
        Box::new(sdrkit_graph::DecoderComponent::new(senders::Sender::new(
            vlf_msk::VlfMsk::new(),
        )))
    });
    r.add("hfdata.hfb_stats", || {
        Box::new(beacons::stats::BeaconStats::default())
    });
}

fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)())
        .expect("the settings of every stack take the generator settings");
    let settings = Values::defaults(&schema);
    let mut g = (desc.make_generator.expect("every stack has a generator"))(
        &GenConfig::new(rate, settings.clone()).with_seed(1),
    )
    .expect("the generator builds");
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    let k = g.generate(&mut x);
    x.truncate(k);
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    let centre = desc.tuning.default_hz;
    let _ = rx.configure(&InputInfo::new(rate, centre, centre).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// Benchmarks a receiver of this crate whose test signal puts the dial where a tuned radio would (the tune offset below the
/// centre of the samples).
fn bench_tuned(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)())
        .expect("the settings of every stack take the generator settings");
    let settings = Values::defaults(&schema);
    let mut g = (desc.make_generator.expect("every stack has a generator"))(
        &GenConfig::new(rate, settings.clone()).with_seed(1),
    )
    .expect("the generator builds");
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    let k = g.generate(&mut x);
    x.truncate(k);
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    let centre = desc.tuning.default_hz;
    let dial = centre + serial::gen::test_dial(rate);
    let _ = rx.configure(&InputInfo::new(rate, centre, dial).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// Benchmarks of the receivers (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "ms110.receiver_2m",
            run: |s| bench_tuned(s, &ms110::MS110, 2e6, 3.0),
        },
        Benchmark {
            name: "s4285.receiver_2m",
            run: |s| bench_tuned(s, &s4285::S4285, 2e6, 3.0),
        },
        Benchmark {
            name: "s4539.receiver_2m",
            run: |s| bench_tuned(s, &s4539::S4539, 2e6, 3.0),
        },
        Benchmark {
            name: "ale3g.receiver_2m",
            run: |s| bench_tuned(s, &ale3g::ALE3G, 2e6, 3.0),
        },
        Benchmark {
            name: "s4481.receiver_2m",
            run: |s| bench_tuned(s, &s4481::S4481, 2e6, 3.0),
        },
        Benchmark {
            name: "link11.receiver_2m",
            run: |s| bench_tuned(s, &link11::LINK11, 2e6, 3.0),
        },
        Benchmark {
            name: "efr.receiver_24k",
            run: |s| bench_receiver(s, &efr::EFR, 24e3, 30.0),
        },
        Benchmark {
            name: "gtor.receiver_24k",
            run: |s| bench_receiver(s, &gtor::GTOR, 24e3, 30.0),
        },
        Benchmark {
            name: "arqe.receiver_24k",
            run: |s| bench_receiver(s, &arqe::ARQE, 24e3, 30.0),
        },
        Benchmark {
            name: "autospec.receiver_24k",
            run: |s| bench_receiver(s, &autospec::AUTOSPEC, 24e3, 30.0),
        },
        Benchmark {
            name: "piccolo.receiver_24k",
            run: |s| bench_receiver(s, &piccolo::PICCOLO, 24e3, 30.0),
        },
        Benchmark {
            name: "coquelet.receiver_24k",
            run: |s| bench_receiver(s, &coquelet::COQUELET, 24e3, 30.0),
        },
        Benchmark {
            name: "cis_hf.receiver_24k",
            run: |s| bench_receiver(s, &cis::CIS_HF, 24e3, 30.0),
        },
        Benchmark {
            name: "crowd36.receiver_24k",
            run: |s| bench_receiver(s, &crowd36::CROWD36, 24e3, 12.0),
        },
        Benchmark {
            name: "xpa.receiver_24k",
            run: |s| bench_receiver(s, &xpa::XPA, 24e3, 30.0),
        },
        Benchmark {
            name: "fsk200.receiver_24k",
            run: |s| bench_receiver(s, &fsk200::FSK200, 24e3, 20.0),
        },
        Benchmark {
            name: "gwfsk.receiver_24k",
            run: |s| bench_receiver(s, &gwfsk::GWFSK, 24e3, 10.0),
        },
        Benchmark {
            name: "fsk_modems.receiver_24k",
            run: |s| bench_receiver(s, &fsk_modems::FSK_MODEMS, 24e3, 4.0),
        },
        Benchmark {
            name: "uvb76.receiver_24k",
            run: |s| bench_receiver(s, &uvb76::UVB76, 24e3, 20.0),
        },
        Benchmark {
            name: "vlf_msk.receiver_24k",
            run: |s| bench_receiver(s, &vlf_msk::VLF_MSK, 24e3, 20.0),
        },
        Benchmark {
            name: "ale.receiver_250k",
            run: |s| bench_receiver(s, &ale::ALE, 250e3, 2.0),
        },
        Benchmark {
            name: "pactor1.receiver_250k",
            run: |s| bench_receiver(s, &pactor1::PACTOR1, 250e3, 2.0),
        },
        Benchmark {
            name: "wavebuoy.receiver_250k",
            run: |s| bench_receiver(s, &buoy::WAVEBUOY, 250e3, 2.0),
        },
        Benchmark {
            name: "hfradar.receiver_2m",
            run: |s| bench_receiver(s, &radar::HFRADAR, 2e6, 3.0),
        },
        Benchmark {
            name: "hfbeacons.receiver_2m",
            run: |s| bench_receiver(s, &beacons::HFBEACONS, 2e6, 0.5),
        },
        Benchmark {
            name: "pactor2.receiver_250k",
            run: |s| bench_receiver(s, &pactorhf::PACTOR2, 250e3, 2.0),
        },
        Benchmark {
            name: "rpr.receiver_2m",
            run: |s| bench_tuned(s, &rpr::RPR, 2e6, 6.0),
        },
        Benchmark {
            name: "hfid.receiver_250k",
            run: |s| bench_receiver(s, &hfid::HFID, 250e3, 2.0),
        },
    ]
}

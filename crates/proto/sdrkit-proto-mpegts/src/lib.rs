//! The MPEG transport stream: packets, the demultiplexer, PSI and DVB SI, the programme guide, elementary streams for host
//! decoders, teletext pages and DVB subtitles, as plain types and as graph components. [`register`] adds the components and
//! the conversions of a transport stream that the graph inserts. [`publish`] declares what a host reads of it.
//!
//! Standard: ISO/IEC 13818-1, ETSI EN 300 468, ETSI TS 101 211, ETSI TS 101 154, ETSI EN 300 472, ETS 300 706, ETSI EN 300 743,
//! ATSC A/65.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod codec;
pub mod components;
pub mod demux;
pub mod epg;
mod error;
pub mod es;
pub mod font;
pub mod input;
pub mod layer;
pub mod monitor;
pub mod mpeg4;
pub mod nal;
pub mod outer;
pub mod packet;
pub mod pcr;
pub mod pes;
pub mod publish;
pub mod sendqueue;
pub mod si;
pub mod source;
pub mod stream;
pub mod subtitle;
pub mod tables;
pub mod teletext;
pub mod time;
mod video;

pub use components::register;
pub use demux::{Demultiplexer, DemuxOutput, Route, RouteSet, Section, TsDemux, TsFilter};
pub use epg::{Epg, EpgEvent};
pub use error::{Error, Mpeg4Fault, Mpeg4Field, MuxFault, SectionFault};
pub use es::EsPacket;
pub use layer::{attach, TsLayer};
pub use monitor::{TsMonitor, TsMonitorComponent};
pub use pes::PesPacket;
pub use publish::{ServiceList, SiTables, TeletextTables, TsNetwork, TsTime};
pub use si::{Service, SiDb, StreamEntry};
pub use source::{Numbered, NumberedCheck, PacketSource, TestMux};
pub use subtitle::{SubtitleDisplay, SubtitlePresenter};
pub use teletext::{TeletextDecoder, TeletextPage};

/// The benchmarks of this crate: the demultiplexer over a generated multiplex, the monitor over the test multiplex, teletext
/// decoding, and SL packets joined into access units.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![
        sdrkit_core::Benchmark {
            name: "mpegts.demux_20mbit",
            run: bench_demux,
        },
        sdrkit_core::Benchmark {
            name: "mpegts.monitor_20mbit",
            run: bench_monitor,
        },
        sdrkit_core::Benchmark {
            name: "mpegts.teletext",
            run: bench_teletext,
        },
        sdrkit_core::Benchmark {
            name: "mpegts.sl_packets",
            run: bench_sl,
        },
    ]
}

fn bench_sl(seconds: f64) -> sdrkit_core::BenchResult {
    use mpeg4::{SlAssembler, SlConfig, SlHeader};
    // DMB video: access units of 2,000 bytes, each in one SL packet with a 33-bit time stamp, at 512 kbit/s.
    let c = SlConfig {
        use_au_start: true,
        use_au_end: true,
        use_rap: true,
        use_timestamps: true,
        ts_resolution: 90_000,
        ts_len: 33,
        ..SlConfig::default()
    };
    let packets: Vec<Vec<u8>> = (0..1000u64)
        .map(|i| {
            let mut p = Vec::new();
            let h = SlHeader {
                au_start: true,
                au_end: true,
                rap: i % 25 == 0,
                cts: Some(i * 3600),
                ..SlHeader::default()
            };
            h.write(&c, &mut p).expect("a header of this configuration");
            p.resize(p.len() + 2000, 0x5A);
            p
        })
        .collect();
    let n = packets.len();
    let signal_s = n as f64 * 2000.0 * 8.0 / 512e3;
    let mut sl = SlAssembler::new(1 << 16);
    let mut bytes = 0usize;
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for p in &packets {
            let _ = sl.push(p, &c, |_, d| bytes += d.len());
        }
    })
}

fn bench_monitor(seconds: f64) -> sdrkit_core::BenchResult {
    use source::PacketSource;
    let mut mux = TestMux::satellite_cable(100).expect("a valid test multiplex");
    let pkts: Vec<[u8; 188]> = (0..20_000)
        .map(|_| {
            let mut p = [0u8; 188];
            mux.next_packet(&mut p);
            p
        })
        .collect();
    let n = pkts.len();
    let signal_s = n as f64 * 1504.0 / 20e6;
    let mut m = TsMonitor::new(mux.numbered());
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for (i, p) in pkts.iter().enumerate() {
            m.push(p, i as f64 * 1504.0 / 20e6);
        }
    })
}

fn bench_teletext(seconds: f64) -> sdrkit_core::BenchResult {
    use sdrkit_util_text::teletext::{packet, PageNo};
    // One page of 24 rows per PES, 100 pages: 2,500 packets.
    let mut pes = Vec::new();
    for k in 0..100u16 {
        let page = PageNo::from_decimal(100 + k).expect("a decimal page");
        let mut pk = vec![packet::build_header(
            page,
            0,
            Default::default(),
            0,
            b"SDRKIT",
        )];
        for r in 1..=24 {
            pk.push(packet::build_row(
                page.magazine(),
                r,
                b"The quick brown fox jumps over the dog",
            ));
        }
        let mut v = Vec::new();
        packet::build_pes(&pk, &mut v);
        pes.push(v);
    }
    let n = 100 * 25;
    // A teletext stream carries about 16 packets per field: 800 packets per second.
    let signal_s = n as f64 / 800.0;
    let mut d = TeletextDecoder::new();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for p in &pes {
            d.push_pes(p);
            d.drain_completed().for_each(drop);
        }
    })
}

fn bench_demux(seconds: f64) -> sdrkit_core::BenchResult {
    // A multiplex of PAT, PMT and one video PID with PES, at about 20 Mbit/s.
    let mut cc = 0u8;
    let mut pkts: Vec<[u8; 188]> = Vec::new();
    let pat = tables::build_pat(1, 0, &[(1, 0x100)]);
    let pmt = tables::build_pmt(1, 0, 0x101, &[], &[(0x02, 0x101, vec![])]);
    pkts.extend(packet::packetize_section(&pat, 0, &mut cc));
    let mut c2 = 0u8;
    pkts.extend(packet::packetize_section(&pmt, 0x100, &mut c2));
    let mut c3 = 0u8;
    for i in 0..2000i64 {
        let pes = pes::build_pes(0xE0, Some(i * 3600), None, &[0u8; 2000], false);
        for (k, chunk) in pes.chunks(184).enumerate() {
            let mut p = [0xFFu8; 188];
            packet::write_header(
                &mut p,
                &packet::Header {
                    pusi: k == 0,
                    pid: 0x101,
                    afc: 1,
                    cc: c3,
                    ..Default::default()
                },
            );
            c3 = (c3 + 1) & 15;
            p[4..4 + chunk.len()].copy_from_slice(chunk);
            pkts.push(p);
        }
    }
    let n = pkts.len();
    let signal_s = n as f64 * 1504.0 / 20e6;
    let mut d = Demultiplexer::new();
    d.auto_select = true;
    let mut out = DemuxOutput::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for (i, p) in pkts.iter().enumerate() {
            d.push(p, Some(i as f64 * 1504.0 / 20e6), false, &mut out);
            out.clear();
        }
    })
}

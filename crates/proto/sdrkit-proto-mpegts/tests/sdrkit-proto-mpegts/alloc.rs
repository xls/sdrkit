//! Steady state allocates nothing: after a warm-up, 1,000 more calls of every streaming path are counted. This module holds the
//! counting allocator of the whole test program. The other modules allocate as they like, and the count looks only at its own
//! thread.

use sdrkit_proto_mpegts::monitor::TsMonitor;
use sdrkit_proto_mpegts::source::{NumberedCheck, PacketSource, TestMux};
use sdrkit_proto_mpegts::stream::TsOut;
use sdrkit_proto_mpegts::teletext::TeletextDecoder;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|a| a.set(a.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Runs `f` 200 times to warm up, then 1,000 times, and returns the allocations of the 1,000.
fn count(mut f: impl FnMut()) -> usize {
    for _ in 0..200 {
        f();
    }
    let before = ALLOCS.with(|a| a.get());
    for _ in 0..1000 {
        f();
    }
    ALLOCS.with(|a| a.get()) - before
}

#[test]
fn streaming_paths_do_not_allocate() {
    let mut mux = TestMux::satellite_cable(40).unwrap();
    let pattern = mux.numbered().unwrap();
    let mut p = [0u8; 188];
    let mut mon = TsMonitor::new(Some(pattern));
    let mut check = NumberedCheck::default();
    let mut t = 0.0;
    let n = count(|| {
        mux.next_packet(&mut p);
        t += 1e-4;
        mon.push(&p, t);
        check.push(&pattern, &p, t);
    });
    assert_eq!(n, 0, "test multiplex, monitor and check");
    assert_eq!(mon.stats().services, ["sdrkit test"]);

    let mut out = TsOut::default();
    let mut items = Vec::with_capacity(4);
    let n = count(|| {
        items.clear();
        out.push(&mut items, &p, 0.0, false);
    });
    assert_eq!(n, 0, "transport stream output");

    let pes = {
        use sdrkit_util_text::teletext::{packet, PageNo};
        let page = PageNo::from_decimal(100).unwrap();
        let mut v = Vec::new();
        packet::build_pes(
            &[
                packet::build_header(page, 0, Default::default(), 0, b"SDRKIT"),
                packet::build_row(1, 1, b"Hello"),
                packet::build_row(1, 2, b"World"),
            ],
            &mut v,
        );
        v
    };
    let mut tt = TeletextDecoder::new();
    let n = count(|| {
        tt.push_pes(&pes);
        tt.drain_completed().for_each(drop);
    });
    assert_eq!(n, 0, "teletext decoding");

    // The SL layer of DMB: one access unit in two SL packets, read and joined.
    use sdrkit_proto_mpegts::mpeg4::{SlAssembler, SlConfig, SlHeader};
    let c = SlConfig {
        use_au_start: true,
        use_au_end: true,
        use_rap: true,
        use_timestamps: true,
        ts_resolution: 90_000,
        ts_len: 33,
        ..SlConfig::default()
    };
    let mut first = Vec::new();
    let h = SlHeader {
        au_start: true,
        rap: true,
        cts: Some(3600),
        ..SlHeader::default()
    };
    h.write(&c, &mut first).unwrap();
    first.extend_from_slice(&[0x55; 1000]);
    let mut last = vec![0x40];
    last.extend_from_slice(&[0xAA; 500]);
    let mut sl = SlAssembler::new(1 << 16);
    let mut bytes = 0usize;
    let n = count(|| {
        sl.push(&first, &c, |_, d| bytes += d.len()).unwrap();
        sl.push(&last, &c, |_, d| bytes += d.len()).unwrap();
    });
    assert_eq!(n, 0, "SL packets to access units");
    assert_eq!(bytes, 1200 * 1500);

    // NAL units of an access unit, and the payload of one without its emulation prevention bytes.
    let au = [
        0, 0, 0, 1, 0x67, 0x42, 0, 0, 3, 1, 0, 0, 1, 0x68, 0xCE, 0, 0, 1, 0x65, 0x88,
    ];
    let mut rbsp = Vec::with_capacity(64);
    let n = count(|| {
        for (s, e) in sdrkit_proto_mpegts::nal::NalUnits::new(&au) {
            rbsp.clear();
            sdrkit_proto_mpegts::nal::unescape_rbsp_into(&au[s..e], &mut rbsp);
        }
    });
    assert_eq!(n, 0, "NAL units");
}

/// A transport stream of two services with MPEG audio: a PAT, two PMTs and PES of one layer II frame on each audio PID, as
/// many packets of each PID as make whole cycles of the continuity counter.
fn audio_service() -> Vec<[u8; 188]> {
    use sdrkit_proto_mpegts::packet::{packetize_section, write_header, Header};
    use sdrkit_proto_mpegts::{pes, tables};
    let mut pkts = Vec::new();
    let (mut c0, mut c1, mut c3) = (0u8, 0u8, 0u8);
    let mut cc = [0u8; 2];
    let pat = tables::build_pat(1, 0, &[(1, 0x100), (2, 0x200)]);
    let pmt = tables::build_pmt(1, 0, 0x1FFF, &[], &[(0x03, 0x101, vec![])]);
    let pmt2 = tables::build_pmt(2, 0, 0x1FFF, &[], &[(0x03, 0x201, vec![])]);
    // MPEG-1 layer II, 256 kbit/s, 48 kHz: 768 bytes a frame (ISO/IEC 11172-3 2.4.3.1).
    let mut frame = vec![0u8; 768];
    frame[..4].copy_from_slice(&[0xFF, 0xFC, 0xC4, 0x00]);
    for k in 0..16 {
        pkts.extend(packetize_section(&pat, 0, &mut c0));
        pkts.extend(packetize_section(&pmt, 0x100, &mut c1));
        pkts.extend(packetize_section(&pmt2, 0x200, &mut c3));
        let pes = pes::build_pes(0xC0, Some(k * 2160), None, &frame, true);
        for (n, pid) in [0x101u16, 0x201].into_iter().enumerate() {
            let c2 = &mut cc[n];
            for (i, chunk) in pes.chunks(184).enumerate() {
                let mut p = [0xFFu8; 188];
                write_header(
                    &mut p,
                    &Header {
                        pusi: i == 0,
                        pid,
                        afc: 1,
                        cc: *c2,
                        ..Default::default()
                    },
                );
                *c2 = (*c2 + 1) & 15;
                p[188 - chunk.len()..].copy_from_slice(chunk);
                if chunk.len() < 184 {
                    // Stuffing in an adaptation field fills a short last packet.
                    p[3] = 0x30 | c2.wrapping_sub(1) & 15;
                    p[4] = (183 - chunk.len()) as u8;
                    if p[4] > 0 {
                        p[5] = 0;
                    }
                }
                pkts.push(p);
            }
        }
    }
    pkts
}

#[test]
fn transport_stream_to_media_packets_does_not_allocate() {
    use sdrkit_proto_mpegts::{Demultiplexer, DemuxOutput};
    let pkts = audio_service();
    let mut d = Demultiplexer::new();
    d.probe_all = true;
    d.select(Some(1));
    let mut out = DemuxOutput::default();
    let (mut i, mut t, mut frames) = (0usize, 0.0, 0usize);
    let n = count(|| {
        d.push(&pkts[i % pkts.len()], Some(t), false, &mut out);
        frames += out.media.len();
        out.clear();
        i += 1;
        t += 7.5e-5;
    });
    assert_eq!(n, 0, "transport stream to media packets");
    assert!(frames > 50, "{frames} media packets");
}

/// A stand-in for a stack decoder: outputs `packets` repeatedly, 200 per call, 75 µs apart.
struct Packets {
    packets: Vec<[u8; 188]>,
    next: usize,
    t: f64,
    out: TsOut,
}

impl sdrkit_graph::Component for Packets {
    fn info(&self) -> sdrkit_graph::ComponentInfo {
        use sdrkit_graph::{Kind, Port};
        sdrkit_graph::ComponentInfo::processor(
            "test.packets",
            vec![Port::new("in", Kind::Iq)],
            vec![sdrkit_proto_mpegts::stream::port("out")],
        )
    }
    fn configure(
        &mut self,
        _: &[sdrkit_graph::Format],
    ) -> Result<Vec<sdrkit_graph::Format>, sdrkit_core::ConfigError> {
        Ok(vec![sdrkit_graph::Format::new(
            sdrkit_proto_mpegts::stream::kind(),
            0.0,
        )])
    }
    fn process(&mut self, io: &mut sdrkit_graph::Io) {
        use sdrkit_proto_mpegts::stream::TsPacket;
        for _ in 0..200 {
            let p = &self.packets[self.next % self.packets.len()];
            self.next += 1;
            self.t += 7.5e-5;
            self.out.push(io.items_out::<TsPacket>(0), p, self.t, false);
        }
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
}

/// A steady transport stream of two services goes through `ts_demux` and `psi_si`, which publish their results and tables,
/// and allocates nothing once warm.
#[test]
fn the_transport_stream_layer_does_not_allocate_once_warm() {
    use sdrkit_core::{Complex32, InputInfo, Output, Receiver};
    use sdrkit_proto_mpegts::layer::{self, TsLayer};
    let b = sdrkit_graph::GraphBuilder::new()
        .add(
            "fec",
            Packets {
                packets: audio_service(),
                next: 0,
                t: 0.0,
                out: TsOut::default(),
            },
        )
        .input("fec");
    let mut g = layer::attach(b, "fec.out", &TsLayer::basic())
        .build()
        .unwrap();
    Receiver::configure(&mut g, &InputInfo::new(1e6, 0.0, 0.0).recording()).unwrap();
    let mut out = Output::default();
    let x = [Complex32::new(0.0, 0.0); 16];
    // Warm up for 30 s of signal: the PID reports go out every 0.5 s.
    for _ in 0..2000 {
        out.clear();
        g.feed(&x, &mut out);
    }
    let before = ALLOCS.with(|a| a.get());
    for _ in 0..2000 {
        out.clear();
        g.feed(&x, &mut out);
    }
    let n = ALLOCS.with(|a| a.get()) - before;
    let demux = g
        .component_as::<sdrkit_proto_mpegts::components::TsDemuxComponent>(layer::DEMUX)
        .unwrap();
    assert_eq!(demux.demux().si.services().len(), 2);
    // The PID reports carried the codec of each audio stream to the service information.
    let si = g
        .component_as::<sdrkit_proto_mpegts::components::PsiSi>(layer::SI)
        .unwrap();
    for sid in [1, 2] {
        let s = si.db().service(sid).unwrap();
        assert_eq!(
            s.streams[0].codec,
            Some(sdrkit_proto_mpegts::codec::EsCodec::Mp2)
        );
    }
    assert_eq!(n, 0, "the transport stream layer");
}

//! The graph components, run in a graph.

use sdrkit_components::testing::Source;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, GraphBuilder, Kind, Packet, Registry};
use sdrkit_proto_ipcast::adapt::TsFramer;
use sdrkit_proto_ipcast::alp::{write_bbp_header, write_ts, BbpExtension, NO_POINTER};
use sdrkit_proto_ipcast::bbheader::BbHeader;
use sdrkit_proto_ipcast::components::{AlpComponent, BbFrameAdapt};
use sdrkit_proto_mpegts::components::TsDemuxComponent;
use sdrkit_proto_mpegts::stream::TsPacket;
use std::sync::Arc;

#[test]
fn every_component_is_registered_under_ipcast() {
    let mut r = Registry::new();
    sdrkit_proto_ipcast::register(&mut r);
    for name in ["ipcast.bbframe", "ipcast.alp", "ipcast.ip", "ipcast.route"] {
        let c = r.make(name).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(c.info().name, name);
    }
}

/// The bytes of the transport packets in `d`.
fn ts_bytes(d: &Data) -> Vec<Vec<u8>> {
    match d {
        Data::Items(Some(items)) => items
            .as_any()
            .downcast_ref::<Vec<Timed<TsPacket>>>()
            .unwrap()
            .iter()
            .map(|p| p.value.bytes().to_vec())
            .collect(),
        Data::Items(None) => Vec::new(),
        other => panic!("{other:?}"),
    }
}

/// Ten BBFRAMEs of a transport stream of numbered packets.
fn numbered_bbframes() -> Vec<Packet> {
    let mut f = TsFramer::default();
    let mut n = 0u32;
    let mut frames = Vec::new();
    for _ in 0..10 {
        let mut df = vec![0u8; 2000];
        let syncd = f.fill(&mut df, &mut || {
            n += 1;
            crate::adapt::numbered(n - 1)
        });
        let mut fr = [0u8; 10].to_vec();
        let mut hb = [0u8; 10];
        BbHeader::transport(16_000, syncd).write(&mut hb);
        fr.copy_from_slice(&hb);
        fr.extend_from_slice(&df);
        frames.push(Packet::new(Arc::from(fr), 0.0));
    }
    frames
}

#[test]
fn bbframes_to_a_transport_stream_that_the_demultiplexer_takes() {
    let frames = numbered_bbframes();
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Bytes, 0.0), Data::Bytes(frames), 3),
        )
        .add("adapt", BbFrameAdapt::default())
        .add("demux", TsDemuxComponent::default())
        .connect("src", "adapt")
        .connect("adapt.ts", "demux.in")
        .build()
        .unwrap();
    let ts = run_ts(&mut g, 5, "adapt.ts").0;
    assert!(ts.len() > 100);
    let demux = g.component_as::<TsDemuxComponent>("demux").unwrap();
    assert_eq!(demux.demux().ts.stats.packets, ts.len() as u64);
    for (i, p) in ts.iter().enumerate() {
        assert_eq!(p, &crate::adapt::numbered(i as u32).to_vec());
    }
}

/// A change of frequency clears the counts of the mode adaptation: the frames and the transport packets are of the carrier
/// before.
#[test]
fn a_change_of_frequency_clears_the_bbframe_counts() {
    use sdrkit_graph::Kept;
    use sdrkit_proto_ipcast::components::BbFrameFields;
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source::new(
                Format::new(Kind::Bytes, 0.0),
                Data::Bytes(numbered_bbframes()),
                3,
            ),
        )
        .add("adapt", BbFrameAdapt::default())
        .connect("src", "adapt")
        .build()
        .unwrap();
    let ts = run_ts(&mut g, 5, "adapt.ts").0;
    assert!(ts.len() > 100);
    let counts = |g: &Graph| {
        let f = g.component_results("adapt").unwrap().fields();
        (
            f.at(&BbFrameFields::FRAMES_OK),
            f.at(&BbFrameFields::TS_PACKETS),
        )
    };
    assert_eq!(counts(&g), (Some(10), Some(ts.len() as i64)));
    g.component_mut("adapt")
        .unwrap()
        .forget(&Kept::of(&[], &[]));
    assert_eq!(counts(&g), (Some(0), Some(0)));
}

#[test]
fn alp_transport_packets_leave_on_the_ts_output() {
    let mut s = Vec::new();
    let pk: Vec<[u8; 188]> = (0..4).map(crate::adapt::numbered).collect();
    write_ts(&pk, 0, false, &mut s).unwrap();
    let mut bbps = Vec::new();
    for (k, c) in s.chunks(300).enumerate() {
        let mut b = Vec::new();
        write_bbp_header(
            if k == 0 { 0 } else { NO_POINTER },
            BbpExtension::None,
            &mut b,
        )
        .unwrap();
        b.extend_from_slice(c);
        bbps.push(Packet::new(Arc::from(b), 0.0));
    }
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Bytes, 0.0), Data::Bytes(bbps), 1),
        )
        .add("alp", AlpComponent::default())
        .connect("src", "alp")
        .build()
        .unwrap();
    let ts = run_ts(&mut g, 5, "alp.ts").0;
    assert_eq!(ts, pk.iter().map(|p| p.to_vec()).collect::<Vec<_>>());
}

use sdrkit_core::{Complex32, Output};
use sdrkit_graph::{Graph, TagKind, Timed};
use sdrkit_proto_ipcast::components::{IpPacket, RouteObjectItem, UdpDatagram};

/// Runs `g` for `passes` and returns, for `port`, every transport packet and the indexes of the items with a `Discontinuity`
/// tag.
fn run_ts(g: &mut Graph, passes: usize, port: &str) -> (Vec<Vec<u8>>, Vec<u64>) {
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let (mut pk, mut tags) = (Vec::new(), Vec::new());
    for _ in 0..passes {
        out.clear();
        g.process(&[] as &[Complex32], &mut out);
        let b = g.buffer(port).unwrap();
        pk.extend(ts_bytes(&b.data));
        tags.extend(
            b.tags()
                .iter()
                .filter(|t| matches!(t.kind, TagKind::Discontinuity { .. }))
                .map(|t| t.at),
        );
    }
    (pk, tags)
}

/// Runs `g` for `passes` and returns every item of typed output `port`.
fn run_items<T: sdrkit_graph::Item>(g: &mut Graph, passes: usize, port: &str) -> Vec<T> {
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let mut v = Vec::new();
    for _ in 0..passes {
        out.clear();
        g.process(&[] as &[Complex32], &mut out);
        if let Data::Items(Some(items)) = &g.buffer(port).unwrap().data {
            let items = items.as_any().downcast_ref::<Vec<Timed<T>>>().unwrap();
            v.extend(items.iter().map(|t| t.value.clone()));
        }
    }
    v
}

#[test]
fn a_bad_frame_breaks_the_chain_and_tags_the_next_packet() {
    let mut f = TsFramer::default();
    let mut n = 0u32;
    let mut frames = Vec::new();
    for k in 0..10 {
        let mut df = vec![0u8; 2000];
        let syncd = f.fill(&mut df, &mut || {
            n += 1;
            crate::adapt::numbered(n - 1)
        });
        let mut hb = [0u8; 10];
        BbHeader::transport(16_000, syncd).write(&mut hb);
        if k == 4 {
            hb[9] ^= 0x55; // a bad CRC-8
        }
        let mut fr = hb.to_vec();
        fr.extend_from_slice(&df);
        frames.push(Packet::new(Arc::from(fr), 0.0));
    }
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Bytes, 0.0), Data::Bytes(frames), 1),
        )
        .add("adapt", BbFrameAdapt::default())
        .connect("src", "adapt")
        .build()
        .unwrap();
    let (ts, tags) = run_ts(&mut g, 12, "adapt.ts");
    // The first packet of the stream, and the first after the bad frame, are tagged.
    assert_eq!(tags.len(), 2, "{tags:?}");
    let after = tags[1] as usize;
    let before = crate::adapt::numbered(0);
    assert_eq!(ts[0], before.to_vec());
    // Every packet is one that was sent, and the numbers jump only at the tag.
    let num = |p: &[u8]| u32::from_be_bytes([p[4], p[5], p[6], p[7]]);
    for w in 1..ts.len() {
        let step = num(&ts[w]) - num(&ts[w - 1]);
        assert!(step == 1 || w == after, "packet {w}");
    }
}

#[test]
fn gse_frames_to_route_objects_through_three_components() {
    use sdrkit_proto_ipcast::components::{IpComponent, RouteComponent};
    use sdrkit_proto_ipcast::gse::{GseWriter, Label};
    use sdrkit_proto_ipcast::lct::LctBuilder;
    use std::net::Ipv4Addr;
    // One ROUTE object of 3000 bytes in 3 packets, each a UDP datagram in IPv4 in GSE.
    let obj: Vec<u8> = (0..3000u32).map(|i| (i * 13) as u8).collect();
    let mut w = GseWriter::default();
    for k in 0..3 {
        let mut p = Vec::new();
        LctBuilder {
            tsi: 1,
            toi: 7,
            codepoint: 8,
            transfer_len: Some(3000),
            close_object: k == 2,
        }
        .write(&mut p)
        .unwrap();
        p.extend_from_slice(&((k * 1000) as u32).to_be_bytes());
        p.extend_from_slice(&obj[k * 1000..(k + 1) * 1000]);
        let mut ip = Vec::new();
        sdrkit_proto_ipcast::ip::write_ipv4_udp(
            &mut ip,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(239, 255, 10, 1),
            k as u16,
            1500,
            1234,
            3000,
            &p,
            |_| {},
        )
        .unwrap();
        w.push_pdu(0x0800, Label::Six([1, 2, 3, 4, 5, 6]), &ip)
            .unwrap();
    }
    let mut frames = Vec::new();
    for _ in 0..4 {
        let mut df = vec![0u8; 1200];
        let n = w.fill(&mut df);
        let mut h = BbHeader::transport((n * 8) as u16, 0);
        h.format = sdrkit_proto_ipcast::bbheader::StreamFormat::GenericContinuous;
        h.upl = 0;
        h.sync = 0;
        let mut hb = [0u8; 10];
        h.write(&mut hb);
        let mut fr = hb.to_vec();
        fr.extend_from_slice(&df[..n]);
        frames.push(Packet::new(Arc::from(fr), 0.0));
    }
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Bytes, 0.0), Data::Bytes(frames), 1),
        )
        .add("adapt", BbFrameAdapt::default())
        .add("ip", IpComponent::default())
        .add("route", RouteComponent::default())
        .connect("src", "adapt")
        .connect("adapt.ip", "ip")
        .connect("ip.udp", "route")
        .set("ip.pcap", sdrkit_core::Value::Bool(true))
        .build()
        .unwrap();
    let objects: Vec<RouteObjectItem> = run_items(&mut g, 8, "route.out");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].data.bytes(), &obj[..]);
    assert_eq!((objects[0].tsi, objects[0].toi), (1, 7));
    let _ = (
        std::any::type_name::<IpPacket>(),
        std::any::type_name::<UdpDatagram>(),
    );
}

#[test]
fn the_pcap_output_starts_a_file_then_adds_records() {
    use sdrkit_proto_ipcast::components::{IpComponent, PcapRecord};
    use std::net::Ipv4Addr;
    let mut packets = Vec::new();
    for k in 0..3u16 {
        let mut ip = Vec::new();
        sdrkit_proto_ipcast::ip::write_ipv4_udp(
            &mut ip,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(10, 0, 0, 2),
            k,
            1500,
            1,
            2,
            &[k as u8; 20],
            |_| {},
        )
        .unwrap();
        packets.push(sdrkit_graph::Timed {
            time: f64::from(k),
            value: IpPacket {
                packet: Packet::new(Arc::from(ip), f64::from(k)),
                stream: 0,
                label: [0; 6],
                label_len: 0,
            },
        });
    }
    use sdrkit_graph::stream::Buffer;
    use sdrkit_graph::{Component, Io};
    let mut c = IpComponent::default();
    let schema = c.info().settings;
    c.apply(schema.id("pcap").unwrap(), &sdrkit_core::Value::Bool(true))
        .unwrap();
    let formats = c
        .configure(&[Format::new(Kind::typed::<IpPacket>(), 0.0)])
        .unwrap();
    let mut input = Buffer::new(&Kind::typed::<IpPacket>());
    input.data = Data::Items(Some(Box::new(packets)));
    let inputs = [input];
    let all = [
        Format::new(Kind::typed::<IpPacket>(), 0.0),
        formats[0].clone(),
        formats[1].clone(),
    ];
    let mut outs = [Buffer::new(&all[1].kind), Buffer::new(&all[2].kind)];
    {
        let mut io = Io::standalone(&inputs, &all, &[0], &[1, 2], &mut outs);
        c.process(&mut io);
    }
    let records: Vec<PcapRecord> = match &outs[1].data {
        Data::Items(Some(b)) => b
            .as_any()
            .downcast_ref::<Vec<sdrkit_graph::Timed<PcapRecord>>>()
            .unwrap()
            .iter()
            .map(|t| t.value.clone())
            .collect(),
        other => panic!("{other:?}"),
    };
    assert_eq!(records.len(), 3);
    assert!(records[0].starts_file && !records[1].starts_file && !records[2].starts_file);
    // The file header (24 bytes) and one record header (16 bytes) come before the first packet.
    assert_eq!(&records[0].data.bytes()[..4], &0xA1B2_C3D4u32.to_le_bytes());
    assert_eq!(records[0].data.bytes().len(), 24 + 16 + 48);
    assert_eq!(records[1].data.bytes().len(), 16 + 48);
}

#[test]
fn compressed_ip_packets_are_counted_and_noted() {
    use sdrkit_graph::stream::Buffer;
    use sdrkit_graph::{Component, Io};
    use sdrkit_proto_ipcast::alp::{write_single, PacketType};
    use sdrkit_proto_ipcast::components::COMPRESSED_IP_NOTE;
    // One baseband packet: an IPv4 packet and two packets of compressed IP.
    let mut alp = Vec::new();
    write_single(PacketType::Ipv4, None, &[0x45; 40], &mut alp).unwrap();
    write_single(PacketType::CompressedIp, None, &[0xFD; 30], &mut alp).unwrap();
    write_single(PacketType::CompressedIp, None, &[0xFD; 30], &mut alp).unwrap();
    let mut b = Vec::new();
    write_bbp_header(0, BbpExtension::None, &mut b).unwrap();
    b.extend_from_slice(&alp);
    let mut c = AlpComponent::default();
    let formats = c.configure(&[Format::new(Kind::Bytes, 0.0)]).unwrap();
    let mut input = Buffer::new(&Kind::Bytes);
    input.data = Data::Bytes(vec![Packet::new(Arc::from(b), 1.0)]);
    let inputs = [input];
    let all = [
        Format::new(Kind::Bytes, 0.0),
        formats[0].clone(),
        formats[1].clone(),
        formats[2].clone(),
    ];
    let mut outs = [
        Buffer::new(&all[1].kind),
        Buffer::new(&all[2].kind),
        Buffer::new(&all[3].kind),
    ];
    {
        let mut io = Io::standalone(&inputs, &all, &[0], &[1, 2, 3], &mut outs);
        c.process(&mut io);
    }
    let r = c.results().unwrap();
    let f = r.fields();
    let id = f.decl().id("compressed_ip").unwrap();
    assert_eq!(f.value(id), Some(&sdrkit_core::Value::I64(2)));
    assert_eq!(
        r.log_lines()
            .iter()
            .filter(|l| l.as_str() == COMPRESSED_IP_NOTE)
            .count(),
        1
    );
}

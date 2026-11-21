//! Runs the benchmarks of the hot paths: `cargo run --release -p sdrkit-proto-ipcast --example bench`.

use sdrkit_proto_ipcast::adapt::{TsFramer, UpDeframer};
use sdrkit_proto_ipcast::alp::{
    write_bbp_header, write_single, write_ts, AlpReader, BbpExtension, PacketType, NO_POINTER,
};
use sdrkit_proto_ipcast::bbheader::BbHeader;
use sdrkit_proto_ipcast::gse::{GseReader, GseWriter, Label};
use sdrkit_proto_ipcast::ip::{write_ipv4_udp, Reassembler};
use std::hint::black_box;
use std::net::Ipv4Addr;
use std::time::Instant;

/// Calls `f` (which handles `bytes` bytes) for about half a second and prints the rate.
fn bench(name: &str, bytes: usize, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    let mut n = 0usize;
    while start.elapsed().as_secs_f64() < 0.5 {
        f();
        n += bytes;
    }
    let rate = n as f64 * 8.0 / start.elapsed().as_secs_f64();
    println!("{name:40} {:10.0} Mbit/s", rate / 1e6);
}

fn main() {
    // DVB-S2 normal frames of 64800 bits at rate 9/10: data fields of 7264 bytes.
    let mut f = TsFramer::default();
    let mut n = 0u8;
    let frames: Vec<(BbHeader, Vec<u8>)> = (0..16)
        .map(|_| {
            let mut df = vec![0u8; 7264];
            let syncd = f.fill(&mut df, &mut || {
                n = n.wrapping_add(1);
                let mut p = [n; 188];
                p[0] = 0x47;
                p
            });
            (BbHeader::transport(7264 * 8, syncd), df)
        })
        .collect();
    let mut d = UpDeframer::new();
    bench("mode adaptation (TS out of data fields)", 16 * 7264, || {
        for (h, df) in &frames {
            let _ = d.push(h, df, &mut |p| {
                black_box(p.bytes[3]);
            });
        }
    });

    let mut w = GseWriter::default();
    for i in 0..400usize {
        w.push_pdu(
            0x0800,
            Label::Six([1, 2, 3, 4, 5, 6]),
            &vec![i as u8; 200 + (i * 37) % 1300],
        )
        .unwrap();
    }
    let fields: Vec<Vec<u8>> = (0..64)
        .map(|_| {
            let mut df = vec![0u8; 7264];
            w.fill(&mut df);
            df
        })
        .collect();
    let mut g = GseReader::default();
    bench("GSE reader (with reassembly)", 64 * 7264, || {
        for df in &fields {
            g.push(df, &mut |p| {
                black_box(p.pdu.len());
            });
        }
    });

    let mut stream = Vec::new();
    for i in 0..200usize {
        write_single(
            PacketType::Ipv4,
            None,
            &vec![i as u8; 100 + (i * 53) % 1400],
            &mut stream,
        )
        .unwrap();
        let ts = [[0x47u8; 188]; 4];
        write_ts(&ts, 0, false, &mut stream).unwrap();
    }
    let bbps: Vec<Vec<u8>> = stream
        .chunks(5000)
        .enumerate()
        .map(|(k, c)| {
            let mut b = Vec::new();
            write_bbp_header(
                if k == 0 { 0 } else { NO_POINTER },
                BbpExtension::None,
                &mut b,
            )
            .unwrap();
            b.extend_from_slice(c);
            b
        })
        .collect();
    let total: usize = bbps.iter().map(|b| b.len()).sum();
    let mut a = AlpReader::new();
    bench("ALP reader (baseband packets)", total, || {
        for b in &bbps {
            a.push_bbp(b, &mut |e| {
                black_box(e);
            });
        }
    });

    println!(
        "  (ALP packets {}, dropped {}, skipped bytes {})",
        a.stats.packets, a.stats.dropped, a.stats.skipped
    );

    let mut out = Vec::new();
    let mut ranges = Vec::new();
    for id in 0..64u16 {
        write_ipv4_udp(
            &mut out,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(239, 0, 0, 1),
            id,
            1500,
            1,
            2,
            &vec![id as u8; 4000],
            |r| ranges.push(r),
        )
        .unwrap();
    }
    let mut r = Reassembler::default();
    bench("IPv4 reassembly and UDP checksum", out.len(), || {
        for rg in &ranges {
            let _ = r.push(&out[rg.clone()], 0.0, &mut |dg| {
                black_box(dg.udp());
            });
        }
    });
}

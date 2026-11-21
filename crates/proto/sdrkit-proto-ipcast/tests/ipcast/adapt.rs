//! Mode adaptation: EN 302 307-1 clauses 5.1.4 to 5.1.6 and annex D, EN 302 755 clause 5.1.8.

use sdrkit_proto_ipcast::adapt::{Issy, TsFramer, UpDeframer, NULL_PACKET};
use sdrkit_proto_ipcast::bbheader::{BbHeader, Mode, StreamFormat};
use sdrkit_util_bits::crc::catalog::CRC8_DVB_S2;

/// A transport packet that carries its number in its payload, PID 0x100.
pub fn numbered(n: u32) -> [u8; 188] {
    let mut p = [0u8; 188];
    p[0] = 0x47;
    p[1] = 0x01;
    p[2] = 0x00;
    p[3] = 0x10 | (n & 15) as u8;
    p[4..8].copy_from_slice(&n.to_be_bytes());
    for (i, b) in p[8..].iter_mut().enumerate() {
        *b = (i as u32 ^ n) as u8;
    }
    p
}

fn number(p: &[u8]) -> u32 {
    u32::from_be_bytes([p[4], p[5], p[6], p[7]])
}

#[test]
fn packets_cross_frames_and_come_back_in_order() {
    let mut f = TsFramer::default();
    let mut d = UpDeframer::new();
    let mut n = 0u32;
    let mut got = Vec::new();
    for _ in 0..20 {
        let mut df = vec![0u8; 4826];
        let syncd = f.fill(&mut df, &mut || {
            n += 1;
            numbered(n - 1)
        });
        let h = BbHeader::transport(4826 * 8, syncd);
        d.push(&h, &df, &mut |p| {
            assert!(p.crc_ok);
            got.push(number(p.bytes));
            assert_eq!(p.bytes, &numbered(number(p.bytes))[..]);
        })
        .unwrap();
    }
    assert!(got.len() > 500);
    assert!(got.windows(2).all(|w| w[1] == w[0] + 1));
    assert_eq!(got[0], 0);
}

/// The framer of user packets of another length (a GFPS of 96 bytes, sync byte 0xB8) in normal mode, and of transport packets
/// in high efficiency mode (no sync byte, no CRC-8), each through the deframer: every packet back, in order.
#[test]
fn the_framer_takes_any_packet_length_and_high_efficiency_mode() {
    let gfps = |n: u32| {
        let mut p = [0u8; 96];
        p[0] = 0xB8;
        p[1..5].copy_from_slice(&n.to_be_bytes());
        for (i, b) in p[5..].iter_mut().enumerate() {
            *b = (i as u32 ^ n.wrapping_mul(29)) as u8;
        }
        p
    };
    let cases = [(TsFramer::normal(96), Mode::Normal), (TsFramer::high_efficiency(188), Mode::HighEfficiency)];
    for (mut f, mode) in cases {
        let upl = f.upl();
        let mut d = UpDeframer::new();
        let (mut n, mut got) = (0u32, Vec::new());
        for _ in 0..20 {
            let mut df = vec![0u8; 3001];
            let syncd = f.fill(&mut df, &mut || {
                n += 1;
                if upl == 96 { gfps(n - 1).to_vec() } else { numbered(n - 1).to_vec() }
            });
            let h = BbHeader {
                format: if upl == 96 { StreamFormat::GenericPacketised } else { StreamFormat::Transport },
                upl: if mode == Mode::Normal { (upl * 8) as u16 } else { 0 },
                sync: if upl == 96 { 0xB8 } else { 0 },
                mode,
                ..BbHeader::transport(3001 * 8, syncd)
            };
            d.push(&h, &df, &mut |p| {
                assert!(p.crc_ok);
                let k = u32::from_be_bytes(p.bytes[if upl == 96 { 1 } else { 4 }..][..4].try_into().unwrap());
                let want = if upl == 96 { gfps(k).to_vec() } else { numbered(k).to_vec() };
                assert_eq!(p.bytes, &want[..]);
                got.push(k);
            })
            .unwrap();
        }
        assert!(got.len() > 300, "{upl}: {}", got.len());
        assert!(got.windows(2).all(|w| w[1] == w[0] + 1), "{upl}");
    }
}

/// Builds one data field of normal-mode units by hand: [CRC-8 of the previous unit][the unit after the sync byte].
fn units(bodies: &[Vec<u8>]) -> Vec<u8> {
    let mut df = Vec::new();
    let mut crc = 0u8;
    for b in bodies {
        df.push(crc);
        df.extend_from_slice(b);
        crc = CRC8_DVB_S2.checksum(b);
    }
    df.push(crc); // the sync slot of the next unit closes the last one
    df
}

#[test]
fn null_packets_go_back_before_the_packet_that_counts_them() {
    // EN 302 307-1 clause D.3: the DNP after a useful packet counts the null packets deleted before it.
    let mut a = numbered(1)[1..].to_vec();
    a.push(0); // DNP: none before packet 1
    let mut b = numbered(2)[1..].to_vec();
    b.push(2); // DNP: two null packets between packet 1 and packet 2
    let df = units(&[a, b]);
    let mut h = BbHeader::transport((df.len() * 8) as u16, 0);
    h.npd = true;
    h.upl = (188 + 1) * 8;
    let mut seq = Vec::new();
    UpDeframer::new()
        .push(&h, &df, &mut |p| {
            seq.push(if p.null { 0 } else { number(p.bytes) })
        })
        .unwrap();
    assert_eq!(seq, [1, 0, 0, 2]);
}

#[test]
fn crc8_covers_issy_and_dnp() {
    // EN 302 307-1 clause 5.1.4: the CRC-8 covers the UPL - 8 bits after the sync byte, ISSY and DNP included.
    let mut a = numbered(7)[1..].to_vec();
    a.extend_from_slice(&[0x81, 0x23, 0x45]); // ISSY: ISCRlong (first bits 10)
    let df = units(&[a.clone()]);
    let mut h = BbHeader::transport((df.len() * 8) as u16, 0);
    h.issyi = true;
    h.upl = (188 + 3) * 8;
    let mut got = Vec::new();
    UpDeframer::new()
        .push(&h, &df, &mut |p| {
            got.push((p.crc_ok, p.issy, p.bytes.to_vec()))
        })
        .unwrap();
    assert_eq!(got.len(), 1);
    assert!(
        got[0].0,
        "a CRC-8 over the 187 packet bytes only would fail here"
    );
    assert_eq!(got[0].1, Some(Issy::IscrLong(0x01_2345)));
    assert_eq!(got[0].2, numbered(7));
}

#[test]
fn issy_codes_of_table_d1() {
    assert_eq!(Issy::parse(&[0x12, 0x34]).unwrap(), Issy::IscrShort(0x1234));
    assert_eq!(
        Issy::parse(&[0xBF, 0xFF, 0xFF]).unwrap(),
        Issy::IscrLong(0x3F_FFFF)
    );
    assert_eq!(
        Issy::parse(&[0xC6, 0x10]).unwrap(),
        Issy::Bufs {
            unit: 1,
            value: 0x210
        }
    );
    assert_eq!(
        Issy::parse(&[0xE9, 0x01]).unwrap(),
        Issy::Bufstat {
            unit: 2,
            value: 0x101
        }
    );
}

#[test]
fn a_bad_crc_sets_the_transport_error_indicator() {
    let a = numbered(3)[1..].to_vec();
    let mut df = units(&[a]);
    df[50] ^= 1;
    let h = BbHeader::transport((df.len() * 8) as u16, 0);
    let mut got = Vec::new();
    let mut d = UpDeframer::new();
    d.push(&h, &df, &mut |p| got.push((p.crc_ok, p.bytes[1])))
        .unwrap();
    assert_eq!(got, [(false, 0x81)]);
    assert_eq!(d.stats.crc_errors, 1);
}

#[test]
fn high_efficiency_mode_transport_stream() {
    // EN 302 755 figure 6: [187 bytes][DNP], no CRC-8, SYNCD at the byte after the sync byte.
    let mut df = Vec::new();
    df.extend_from_slice(&[9u8; 20]); // the tail of a packet from the last frame
    for n in 0..3 {
        df.extend_from_slice(&numbered(n)[1..]);
        df.push(if n == 1 { 1 } else { 0 });
    }
    let mut h = BbHeader::transport((df.len() * 8) as u16, 20 * 8);
    h.mode = Mode::HighEfficiency;
    h.npd = true;
    let mut seq = Vec::new();
    UpDeframer::new()
        .push(&h, &df, &mut |p| {
            assert!(p.crc_ok);
            seq.push(if p.null { 99 } else { number(p.bytes) });
            if !p.null {
                assert_eq!(p.bytes[0], 0x47);
            } else {
                assert_eq!(p.bytes, &NULL_PACKET[..]);
            }
        })
        .unwrap();
    assert_eq!(seq, [0, 99, 1, 2]);
}

#[test]
fn a_lost_frame_is_found_again_at_syncd() {
    let mut f = TsFramer::default();
    let mut n = 0u32;
    let mut frames = Vec::new();
    for _ in 0..6 {
        let mut df = vec![0u8; 1000];
        let syncd = f.fill(&mut df, &mut || {
            n += 1;
            numbered(n - 1)
        });
        frames.push((BbHeader::transport(8000, syncd), df));
    }
    let mut d = UpDeframer::new();
    let mut got = Vec::new();
    for (i, (h, df)) in frames.iter().enumerate() {
        if i == 2 {
            continue; // lost
        }
        d.push(h, df, &mut |p| {
            if p.crc_ok {
                got.push(number(p.bytes))
            }
        })
        .unwrap();
    }
    assert!(d.stats.resyncs >= 1);
    // Every packet delivered with a good CRC is one that was sent, in order.
    assert!(got.windows(2).all(|w| w[1] > w[0]));
    assert!(got.contains(&30));
}

#[test]
fn bad_headers_are_refused_without_panics() {
    let mut d = UpDeframer::new();
    let mut h = BbHeader::transport(800, 0);
    assert!(
        d.push(&h, &[0u8; 50], &mut |_| {}).is_err(),
        "DFL past the field"
    );
    h.upl = 0;
    assert!(d.push(&h, &[0u8; 100], &mut |_| {}).is_err());
    h.upl = 1504;
    h.syncd = 3;
    assert!(d.push(&h, &[0u8; 100], &mut |_| {}).is_err());
    assert_eq!(d.stats.refused, 3);
}

#[test]
fn the_deframer_does_not_allocate() {
    let mut f = TsFramer::default();
    let mut n = 0u32;
    let mut frames = Vec::new();
    for _ in 0..8 {
        let mut df = vec![0u8; 4826];
        let syncd = f.fill(&mut df, &mut || {
            n += 1;
            numbered(n)
        });
        frames.push((BbHeader::transport(4826 * 8, syncd), df));
    }
    let mut d = UpDeframer::new();
    let mut k = 0;
    let mut sink = 0u64;
    let a = crate::alloc::count(|| {
        let (h, df) = &frames[k % frames.len()];
        k += 1;
        let _ = d.push(h, df, &mut |p| sink += p.bytes[7] as u64);
    });
    assert_eq!(a, 0);
    assert!(sink > 0);
}

#[test]
fn a_change_of_stream_shape_does_not_panic() {
    // Half a unit of a stream with ISSY (UPL 191 x 8), then a frame of a generic packetised stream of 100-byte packets.
    let mut d = UpDeframer::new();
    let mut a = numbered(1)[1..].to_vec();
    a.extend_from_slice(&[0x81, 0, 0]);
    let df = units(&[a]);
    let mut h = BbHeader::transport((100 * 8) as u16, 0);
    h.issyi = true;
    h.upl = 191 * 8;
    d.push(&h, &df[..100], &mut |_| {}).unwrap();
    let mut g = BbHeader::transport(800, 0);
    g.format = sdrkit_proto_ipcast::bbheader::StreamFormat::GenericPacketised;
    g.upl = 100 * 8;
    g.sync = 0;
    let r = d.push(&g, &[0u8; 100], &mut |_| {});
    assert!(r.is_ok());
    assert_eq!(d.stats.resyncs, 1);
}

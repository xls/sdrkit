//! The synthetic video test units against synthetic_packets.bin. The test joins the fragments
//! itself only to read the vector.

mod common;

use sdrkit_codec_custom::video_units::{Synthetic, SyntheticParams, UnitKind};

#[test]
fn units_match_the_packet_vector() {
    let Some(dir) = common::vectors() else { return };
    let bin = std::fs::read(common::folder_with(&dir, "synthetic_packets.bin").join("synthetic_packets.bin")).unwrap();
    // Packet: type and stream byte, sequence (2), length (2), payload, CRC-32 (4).
    let mut units: Vec<(u8, u32, bool, Vec<u8>)> = Vec::new();
    let mut cur: [Option<(u32, bool, Vec<u8>)>; 2] = [None, None];
    let mut i = 0;
    let mut packets = 0;
    while i < bin.len() {
        let t = bin[i] >> 4;
        let len = u16::from_be_bytes([bin[i + 3], bin[i + 4]]) as usize;
        let pl = &bin[i + 5..i + 5 + len];
        i += 9 + len;
        packets += 1;
        let flags = pl[0];
        let slot = &mut cur[t as usize];
        let mut data = &pl[1..];
        if flags & 1 != 0 {
            let time = u32::from_be_bytes([pl[1], pl[2], pl[3], pl[4]]);
            data = &pl[5..];
            *slot = Some((time, flags & 4 != 0, Vec::new()));
        }
        slot.as_mut().unwrap().2.extend_from_slice(data);
        if flags & 2 != 0 {
            let (time, key, bytes) = slot.take().unwrap();
            units.push((t, time, key, bytes));
        }
    }
    assert_eq!(packets, 56);
    let mut g = Synthetic::new(SyntheticParams::default()).unwrap();
    let mut buf = vec![0u8; 30000];
    let mut made = Vec::new();
    while g.media_time() < 0.25 {
        made.push(g.next_unit());
    }
    // The packet order puts each unit's fragments together, in the order the generator makes the units. The source requests
    // packets while the media time is below 0.25 s, so the fragments of the last unit can be cut off.
    assert!(made.len() == units.len() || made.len() == units.len() + 1);
    assert!(cur.iter().filter(|c| c.is_some()).count() + units.len() >= made.len());
    for (u, (t, time, key, bytes)) in made.iter().zip(&units) {
        assert_eq!(u.kind.packet_type(), *t);
        assert_eq!((u.time_ms, u.key, u.len), (*time, *key, bytes.len()));
        u.fill(&mut buf).unwrap();
        assert_eq!(&buf[..u.len], &bytes[..]);
        if u.kind == UnitKind::Audio {
            assert_eq!(u.len, 240);
        }
    }
}

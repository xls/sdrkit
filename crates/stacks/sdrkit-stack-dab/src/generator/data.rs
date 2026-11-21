//! The generator's data services: packet-mode sub-channels (with the packet FEC), SPI and Journaline carousels in them, a DMB video
//! stream sub-channel, and the FIGs that signal them.

use super::fic::fig;
use super::services::LOOP_FRAMES;
use crate::data::dmb::write as dmbw;
use sdrkit_proto_bcdata::datagroup::Header;
use sdrkit_proto_bcdata::journaline::{self, Kind, Object};
use sdrkit_proto_bcdata::mot::{self, HeaderWriter};
use sdrkit_proto_bcdata::packet::{dab_padding, DabFec, DabPacketWriter, FEC_ADT};
use sdrkit_proto_bcdata::spi;
use sdrkit_proto_mpegts::outer::{Dispersal, OuterEncoder, RS_N};
use sdrkit_proto_mpegts::packet::{packetize_pes, packetize_section};
use sdrkit_proto_mpegts::tables::{build_pat, build_pmt, descriptor};

/// An MSC data group of `group_type` with a CRC, one segment (number 0, last) and transport id `tid`, holding `data` behind its
/// MOT segmentation header.
pub fn mot_group(group_type: u8, tid: u16, data: &[u8]) -> Vec<u8> {
    let mut seg = Vec::with_capacity(data.len() + 2);
    // Test objects are far below the 8191-byte segment limit.
    let _ = mot::write_segment(0, data, &mut seg);
    let h = Header {
        group_type,
        segment: Some((true, 0)),
        transport_id: Some(tid),
        ..Header::default()
    };
    let mut out = Vec::with_capacity(seg.len() + 9);
    let _ = h.write(&seg, &mut out);
    out
}

/// The bytes of a MOT header: core, ContentName (UTF-8) and nothing else.
pub fn mot_header(body_size: u32, content_type: u8, subtype: u16, name: &str) -> Vec<u8> {
    let mut v = Vec::new();
    let _ = HeaderWriter::new(body_size, content_type, subtype)
        .content_name(name)
        .write(&mut v);
    v
}

/// Packet-mode logical frames for the loop: the data groups (address, bytes) repeated as a carousel, packetized, padded, with the
/// packet FEC when requested.
pub fn packet_frames(bitrate: u16, groups: &[(u16, Vec<u8>)], fec: bool) -> Vec<Vec<u8>> {
    let lf = bitrate as usize * 3;
    let total = lf * LOOP_FRAMES;
    let mut writers = std::collections::BTreeMap::<u16, DabPacketWriter>::new();
    let fec_coder = DabFec::new();
    let mut stream = Vec::with_capacity(total);
    let mut set = Vec::new();
    let mut packets = Vec::new();
    let mut g = 0;
    while stream.len() < total {
        let (addr, data) = &groups[g % groups.len().max(1)];
        g += 1;
        let w = match writers.entry(*addr) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => match DabPacketWriter::new(*addr) {
                Ok(w) => e.insert(w),
                Err(_) => continue,
            },
        };
        packets.clear();
        if w.write_group(data, &mut packets).is_err() {
            continue;
        }
        if fec {
            // Packets of 24 to 96 bytes, each kept whole in one application data table.
            let mut at = 0;
            while at < packets.len() {
                let len = sdrkit_proto_bcdata::packet::DAB_LENGTHS[(packets[at] >> 6) as usize];
                if set.len() + len > FEC_ADT {
                    while set.len() < FEC_ADT {
                        dab_padding(&mut set);
                    }
                    stream.extend_from_slice(&set);
                    let _ = fec_coder.encode(&set, &mut stream);
                    set.clear();
                }
                set.extend_from_slice(&packets[at..at + len]);
                at += len;
            }
        } else {
            stream.extend_from_slice(&packets);
        }
    }
    stream.truncate(total);
    // Packets do not cross logical frames: re-cut on packet boundaries is not needed when frames are multiples of 24 bytes and
    // packets are back to back; the decoder treats the sub-channel as one byte stream.
    stream.chunks(lf).map(|c| c.to_vec()).collect()
}

/// MSC data groups of an MOT object in directory mode: the directory (type 6) and the bodies (type 4), each in one segment.
pub fn mot_directory(objects: &[(u16, Vec<u8>, Vec<u8>)]) -> Vec<Vec<u8>> {
    // objects: (transport id, header bytes, body)
    let mut entries = Vec::new();
    for (tid, h, _) in objects {
        entries.extend_from_slice(&tid.to_be_bytes());
        entries.extend_from_slice(h);
    }
    let mut dir = Vec::new();
    let size = 13 + entries.len();
    dir.extend_from_slice(&((size as u32) & 0x3FFF_FFFF).to_be_bytes());
    dir.extend_from_slice(&(objects.len() as u16).to_be_bytes());
    dir.extend_from_slice(&[0, 0, 0]);
    dir.extend_from_slice(&[0, 0]);
    dir.extend_from_slice(&[0, 0]);
    dir.extend(entries);
    let mut v = vec![mot_group(6, 0x0001, &dir)];
    for (tid, _, body) in objects {
        v.push(mot_group(4, *tid, body));
    }
    v
}

/// The SPI carousel of the default ensemble: an SI object naming the services and a PI object with two programmes.
pub fn spi_objects(eid: u16, services: &[(u16, &str)], t0: i64) -> Vec<(u16, Vec<u8>, Vec<u8>)> {
    use spi::write::*;
    let svcs: Vec<Vec<u8>> = services
        .iter()
        .map(|&(sid, name)| {
            element(
                0x28,
                &[],
                &[
                    text(0x12, name),
                    element(0x29, &[(0x80, bearer(0xE1, eid, sid, 0))], &[]),
                ],
            )
        })
        .collect();
    let si = element(
        0x03,
        &[],
        &[element(
            0x26,
            &[(0x80, vec![0xE1, (eid >> 8) as u8, eid as u8])],
            &svcs,
        )],
    );
    let mk = |id: &str, name: &str, start: i64| {
        let t = element(
            0x2C,
            &[
                (0x80, timepoint(start)),
                (0x81, 1800u16.to_be_bytes().to_vec()),
            ],
            &[],
        );
        element(
            0x1C,
            &[(0x80, id.as_bytes().to_vec())],
            &[text(0x11, name), element(0x19, &[], &[t])],
        )
    };
    let pi = element(
        0x02,
        &[],
        &[element(
            0x21,
            &[],
            &[
                mk("crid://test/1", "Morning Tones", t0),
                mk("crid://test/2", "Noon Melody", t0 + 1800),
            ],
        )],
    );
    vec![
        (0x10, mot_header(si.len() as u32, 7, 0, "SI.dat"), si),
        (0x11, mot_header(pi.len() as u32, 7, 1, "PI.dat"), pi),
    ]
}

/// The Journaline carousel: a menu and two messages, in data groups of type 0 (no session header).
pub fn journaline_groups() -> Vec<Vec<u8>> {
    let objs = [
        Object {
            id: 0,
            kind: Kind::Menu,
            is_static: true,
            title: "Test News".into(),
            links: vec![(0x10, "Weather".into()), (0x11, "Sport".into())],
            ..Object::default()
        },
        Object {
            id: 0x10,
            kind: Kind::Text,
            title: "Weather".into(),
            body: "Sunny, 21 degrees".into(),
            ..Object::default()
        },
        Object {
            id: 0x11,
            kind: Kind::List,
            title: "Sport".into(),
            items: vec!["Home\t2:1".into(), "Away\t0:0".into()],
            ..Object::default()
        },
    ];
    objs.iter()
        .enumerate()
        .map(|(i, o)| {
            let mut body = Vec::new();
            journaline::encode(o, i == 1, &mut body);
            let h = Header {
                user_access: false,
                ..Header::default()
            };
            let mut v = Vec::with_capacity(body.len() + 4);
            let _ = h.write(&body, &mut v);
            v
        })
        .collect()
}

/// A DMB video stream for the loop: PAT, PMT with the IOD, H.264 I_PCM pictures (`fps`) and AAC-LC access units, RS(204, 188) and
/// the byte interleaver, cut into logical frames of `bitrate`.
pub fn dmb_frames(bitrate: u16, fps: u32, aac_units: &[Vec<u8>], asc: &[u8]) -> Vec<Vec<u8>> {
    let lf = bitrate as usize * 3;
    let total = lf * LOOP_FRAMES;
    let (pmt_pid, vpid, apid) = (0x100u16, 0x101u16, 0x102u16);
    let mut cc = [0u8; 4];
    let pat = build_pat(1, 0, &[(1, pmt_pid)]);
    let es = vec![
        dmbw::es_descriptor(1, 0x21, 4, &[]),
        dmbw::es_descriptor(2, 0x40, 5, asc),
    ];
    let iod = dmbw::iod_descriptor(&es);
    // SL packets in PES (stream_type 0x12), each with an SL_descriptor (tag 0x1E) that carries its ES_ID.
    let streams: Vec<(u8, u16, Vec<u8>)> = [(vpid, 1u16), (apid, 2u16)]
        .iter()
        .map(|&(pid, id)| (0x12, pid, descriptor(0x1E, &id.to_be_bytes())))
        .collect();
    let pmt = build_pmt(1, 0, vpid, &iod, &streams);
    let mut ts: Vec<[u8; 188]> = Vec::new();
    let bytes_per_s = bitrate as f64 * 1000.0 / 8.0 * 188.0 / 204.0;
    let packets_per_s = bytes_per_s / 188.0;
    let need = (total as f64 / 204.0).ceil() as usize + 20;
    let mut t = 0f64;
    let mut frame = 0u32;
    let mut au = 0usize;
    while ts.len() < need {
        if ts.len().is_multiple_of(50) {
            ts.extend(packetize_section(&pat, 0, &mut cc[0]));
            ts.extend(packetize_section(&pmt, pmt_pid, &mut cc[1]));
        }
        let now = ts.len() as f64 / packets_per_s;
        if now >= t {
            let pic = dmbw::h264_idr(64, 48, 16 + (frame % 200) as u8, frame % 16);
            let pts = (t * 90_000.0) as u64;
            ts.extend(packetize_pes(
                &dmbw::sl_pes(&pic, pts, true),
                vpid,
                &mut cc[2],
            ));
            for _ in 0..(48_000 / fps as usize / 960).max(1) {
                if let Some(a) = aac_units.get(au % aac_units.len().max(1)) {
                    let apts = (au as f64 * 960.0 / 48_000.0 * 90_000.0) as u64;
                    ts.extend(packetize_pes(
                        &dmbw::sl_pes(a, apts, true),
                        apid,
                        &mut cc[3],
                    ));
                }
                au += 1;
            }
            frame += 1;
            t += 1.0 / fps as f64;
        } else {
            let mut null = [0xFFu8; 188];
            null[0] = 0x47;
            null[1] = 0x1F;
            null[2] = 0xFF;
            null[3] = 0x10;
            ts.push(null);
        }
    }
    let mut outer = OuterEncoder::new(Dispersal::None);
    let mut s = Vec::with_capacity(ts.len() * RS_N);
    let mut w = [0u8; RS_N];
    for p in &ts {
        outer.encode(p, &mut w);
        s.extend_from_slice(&w);
    }
    s.truncate(total);
    s.chunks(lf).map(|c| c.to_vec()).collect()
}

/// FIG 0/3: a packet mode component (SCId, data groups, DSCTy, sub-channel, packet address).
pub fn fig0_3(scid: u16, data_groups: bool, dscty: u8, subch: u8, address: u16) -> Vec<u8> {
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x03, 8);
    w.put(scid as u64, 12);
    w.put(0, 3);
    w.put(0, 1);
    w.put((!data_groups) as u64 ^ 1, 1);
    w.put(0, 1);
    w.put(dscty as u64, 6);
    w.put(subch as u64, 6);
    w.put(address as u64, 10);
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// FIG 0/8: SId, SCIdS and sub-channel (short form) or SCId (long form).
pub fn fig0_8(sid: u32, scids: u8, subch: Option<u8>, scid: Option<u16>) -> Vec<u8> {
    let pd = sid > 0xFFFF;
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x08 | if pd { 0x20 } else { 0 }, 8);
    w.put(sid as u64, (if pd { 32 } else { 16 }) as u32);
    w.put(0, 1);
    w.put(0, 3);
    w.put(scids as u64, 4);
    match (subch, scid) {
        (Some(sc), _) => {
            w.put(0, 1);
            w.put(0, 1);
            w.put(sc as u64, 6);
        }
        (_, Some(id)) => {
            w.put(1, 1);
            w.put(0, 3);
            w.put(id as u64, 12);
        }
        _ => {}
    }
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// FIG 0/13: a user application of a component.
pub fn fig0_13(sid: u32, scids: u8, app: u16, data: &[u8]) -> Vec<u8> {
    let pd = sid > 0xFFFF;
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x0D | if pd { 0x20 } else { 0 }, 8);
    w.put(sid as u64, (if pd { 32 } else { 16 }) as u32);
    w.put(scids as u64, 4);
    w.put(1, 4);
    w.put(app as u64, 11);
    w.put(data.len() as u64, 5);
    w.bytes(data);
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// FIG 0/14: the FEC scheme of a sub-channel.
pub fn fig0_14(subch: u8, scheme: u8) -> Vec<u8> {
    fig(0, &[0x0E, (subch << 2) | (scheme & 3)])
}

/// FIG 0/9: LTO in half hours, ECC, international table.
pub fn fig0_9(lto: i8, ecc: u8, table: u8) -> Vec<u8> {
    let b = (((lto < 0) as u8) << 5) | (lto.unsigned_abs() & 31);
    fig(0, &[0x09, b, ecc, table])
}

/// FIG 0/21: one DAB frequency (kHz) for an EId, and one FM frequency (kHz) for a PI code.
pub fn fig0_21(eid: u16, dab_khz: u32, pi: u16, fm_khz: u32) -> Vec<u8> {
    let mut lists = Vec::new();
    lists.extend_from_slice(&eid.to_be_bytes());
    lists.push(3);
    let fa = dab_khz / 16;
    lists.extend_from_slice(&[((fa >> 16) & 7) as u8, (fa >> 8) as u8, fa as u8]);
    lists.extend_from_slice(&pi.to_be_bytes());
    lists.push((8 << 4) | 1);
    lists.push(((fm_khz - 87_500) / 100) as u8);
    let mut v = vec![0x15, 0x00, lists.len() as u8];
    v.extend(lists);
    fig(0, &v)
}

/// FIG 0/22 entries: a main identifier's position and its sub-identifiers (SubId, delay µs, latitude and longitude offsets).
pub fn fig0_22(main: u8, lat: f64, lon: f64, subs: &[(u8, u16, f64, f64)]) -> Vec<u8> {
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x16, 8);
    w.put(0, 1);
    w.put(main as u64, 7);
    let latc = (lat / 90.0 * 32768.0).floor();
    let lonc = (lon / 180.0 * 32768.0).floor();
    let latf = ((lat - latc * 90.0 / 32768.0) / (90.0 / 524_288.0))
        .round()
        .clamp(0.0, 15.0);
    let lonf = ((lon - lonc * 180.0 / 32768.0) / (180.0 / 524_288.0))
        .round()
        .clamp(0.0, 15.0);
    w.put((latc as i16) as u16 as u64, 16);
    w.put((lonc as i16) as u16 as u64, 16);
    w.put(latf as u64, 4);
    w.put(lonf as u64, 4);
    if !subs.is_empty() {
        w.put(1, 1);
        w.put(main as u64, 7);
        w.put(0, 5);
        w.put(subs.len() as u64, 3);
        for &(s, td, dlat, dlon) in subs {
            w.put(s as u64, 5);
            w.put(td as u64, 11);
            w.put(
                ((dlat / (90.0 / 524_288.0)).round() as i16) as u16 as u64,
                16,
            );
            w.put(
                ((dlon / (180.0 / 524_288.0)).round() as i16) as u16 as u64,
                16,
            );
        }
    }
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// FIG 0/24: other ensembles that carry a service.
pub fn fig0_24(sid: u16, eids: &[u16]) -> Vec<u8> {
    let mut v = vec![0x18 | 0x40];
    v.extend_from_slice(&sid.to_be_bytes());
    v.push(eids.len() as u8 & 15);
    for e in eids {
        v.extend_from_slice(&e.to_be_bytes());
    }
    fig(0, &v)
}

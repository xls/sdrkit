//! 802.11 MAC frames other than beacons: control and data headers, management bodies, and A-MPDUs (IEEE Std 802.11-2020
//! clause 9).

use sdrkit_proto_wlan_phy::mac::{self, control, Ampdu, Frame, FrameType, MgmtBody};

const AP: [u8; 6] = [0x02, 0, 0, 0, 0, 1];
const STA: [u8; 6] = [0x06, 1, 2, 3, 4, 5];

fn header(ftype: u8, subtype: u8, flags: u8, a1: [u8; 6], a2: [u8; 6], a3: [u8; 6]) -> Vec<u8> {
    let mut f = vec![ftype << 2 | subtype << 4, flags, 0x2C, 0];
    f.extend_from_slice(&a1);
    f.extend_from_slice(&a2);
    f.extend_from_slice(&a3);
    f.extend_from_slice(&[0x30, 0x12]);
    f
}

#[test]
fn control_frames_and_their_addresses() {
    // Ack and CTS: frame control D4 00 and C4 00, the receiver address only.
    let mut ack = vec![0xD4, 0, 0, 0];
    ack.extend_from_slice(&STA);
    let f = Frame::parse(&ack).unwrap();
    assert_eq!((f.fc.ftype, f.fc.subtype), (FrameType::Control, control::ACK));
    assert_eq!(f.parties().transmitter, None);
    assert_eq!(f.parties().receiver, STA);
    // RTS: B4 00, receiver and transmitter.
    let mut rts = vec![0xB4, 0, 0x80, 0];
    rts.extend_from_slice(&AP);
    rts.extend_from_slice(&STA);
    let f = Frame::parse(&rts).unwrap();
    assert_eq!(f.fc.subtype, control::RTS);
    assert_eq!((f.parties().transmitter, f.duration), (Some(STA), 0x80));
    assert_eq!(control::name(f.fc.subtype), "RTS");
    // A compressed BlockAck for TID 5 from sequence number 291.
    let mut ba = vec![0x94, 0, 0, 0];
    ba.extend_from_slice(&STA);
    ba.extend_from_slice(&AP);
    ba.extend_from_slice(&[0x04, 0x50]);
    ba.extend_from_slice(&(291u16 << 4).to_le_bytes());
    ba.extend_from_slice(&[0xFF; 8]);
    let b = Frame::parse(&ba).unwrap().block_ack().unwrap();
    assert_eq!((b.request, b.tid, b.start_seq, b.variant), (false, 5, 291, 2));
    // PS-Poll: address 1 is the BSSID.
    let mut ps = vec![0xA4, 0, 0x01, 0xC0];
    ps.extend_from_slice(&AP);
    ps.extend_from_slice(&STA);
    let p = Frame::parse(&ps).unwrap().parties();
    assert_eq!((p.bssid, p.to_ap), (Some(AP), true));
}

#[test]
fn data_frames_by_their_ds_bits() {
    let dst = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    // To DS: a station to its access point. QoS data with TID 6, protected.
    let mut up = header(2, 8, 0x41, AP, STA, dst);
    up.extend_from_slice(&[0x06, 0x00]);
    up.extend_from_slice(&[0xAA; 12]);
    let f = Frame::parse(&up).unwrap();
    let p = f.parties();
    assert_eq!((p.transmitter, p.bssid, p.to_ap, p.from_ap), (Some(STA), Some(AP), true, false));
    assert_eq!((f.tid(), f.fc.protected, f.is_null_data()), (Some(6), true, false));
    assert_eq!(f.body.len(), 12);
    assert_eq!(f.seq, Some((0x123, 0)));
    // From DS: the access point to a station.
    let down = header(2, 0, 0x02, STA, AP, dst);
    let p = Frame::parse(&down).unwrap().parties();
    assert_eq!((p.receiver, p.bssid, p.to_ap, p.from_ap), (STA, Some(AP), false, true));
    // Null and QoS Null carry no body.
    assert!(Frame::parse(&header(2, 4, 0x11, AP, STA, AP)).unwrap().is_null_data());
    let mut qn = header(2, 12, 0x01, AP, STA, AP);
    qn.extend_from_slice(&[0, 0]);
    assert!(Frame::parse(&qn).unwrap().is_null_data());
    assert_eq!(mac::data_subtype_name(12), "QoS Null");
    assert!(mac::is_group(&mac::BROADCAST) && !mac::is_group(&STA));
    assert!(mac::is_local(&STA) && !mac::is_local(&dst));
}

#[test]
fn management_bodies() {
    // A probe request for the SSID "lab" with HT, VHT and HE capabilities.
    let mut pr = header(0, 4, 0, mac::BROADCAST, STA, mac::BROADCAST);
    mac::push_element(&mut pr, mac::element::SSID, b"lab").unwrap();
    mac::push_element(&mut pr, mac::element::SUPPORTED_RATES, &[0x02, 0x04, 0x0B, 0x16]).unwrap();
    mac::push_element(&mut pr, mac::element::EXTENDED_RATES, &[0x30, 0x48, 0x60, 0x6C]).unwrap();
    mac::push_element(&mut pr, mac::element::DS_PARAMETER_SET, &[11]).unwrap();
    mac::push_element(&mut pr, mac::element::HT_CAPABILITIES, &[0; 26]).unwrap();
    mac::push_element(&mut pr, mac::element::VHT_CAPABILITIES, &[0; 12]).unwrap();
    mac::push_element(&mut pr, mac::element::EXTENSION, &[mac::element::EXT_HE_CAPABILITIES, 0, 0]).unwrap();
    let Some(MgmtBody::ProbeRequest(r)) = Frame::parse(&pr).unwrap().mgmt() else {
        panic!("not a probe request");
    };
    assert_eq!((r.ssid(), r.generation(), r.max_rate_kbps(), r.channel()), (Some(&b"lab"[..]), 6, Some(54_000), Some(11)));
    // Authentication: open system, transaction 2, success.
    let mut au = header(0, 11, 0, STA, AP, AP);
    au.extend_from_slice(&[0, 0, 2, 0, 0, 0]);
    let f = Frame::parse(&au).unwrap();
    assert_eq!(f.mgmt(), Some(MgmtBody::Authentication { algorithm: 0, transaction: 2, status: 0 }));
    assert!(f.parties().from_ap);
    // Reassociation request with the current access point.
    let mut ra = header(0, 2, 0, AP, STA, AP);
    ra.extend_from_slice(&[0x31, 0x04, 10, 0]);
    ra.extend_from_slice(&[9; 6]);
    mac::push_element(&mut ra, mac::element::SSID, b"lab").unwrap();
    let Some(MgmtBody::AssocRequest { current_ap, listen_interval, elements, .. }) = Frame::parse(&ra).unwrap().mgmt() else {
        panic!("not a reassociation request");
    };
    assert_eq!((current_ap, listen_interval), (Some([9; 6]), 10));
    assert_eq!(mac::find(elements, mac::element::SSID).unwrap().data, b"lab");
    // Association response: status 0, AID 5 (sent with its two top bits set).
    let mut ar = header(0, 1, 0, STA, AP, AP);
    ar.extend_from_slice(&[0x31, 0x04, 0, 0, 0x05, 0xC0]);
    assert!(matches!(Frame::parse(&ar).unwrap().mgmt(), Some(MgmtBody::AssocResponse { status: 0, aid: 5, .. })));
    // Deauthentication with reason 7; a block ack action; a protected action frame gives no body.
    let mut de = header(0, 12, 0, AP, STA, AP);
    de.extend_from_slice(&[7, 0]);
    assert_eq!(Frame::parse(&de).unwrap().mgmt(), Some(MgmtBody::Deauthentication { reason: 7 }));
    assert_eq!(mac::reason_text(7), "class 3 frame from a station that is not associated");
    let mut ac = header(0, 13, 0, AP, STA, AP);
    ac.extend_from_slice(&[3, 0, 1]);
    assert_eq!(Frame::parse(&ac).unwrap().mgmt(), Some(MgmtBody::Action { category: 3, action: Some(0) }));
    assert_eq!(mac::action_category_name(3), "Block Ack");
    ac[1] = 0x40;
    assert_eq!(Frame::parse(&ac).unwrap().mgmt(), None);
    // A truncated body gives nothing.
    assert_eq!(Frame::parse(&de[..25]).unwrap().mgmt(), None);
}

fn mpdu(n: usize, tag: u8) -> Vec<u8> {
    let mut f = header(2, 0, 0x01, AP, STA, AP);
    f.extend((0..n).map(|i| tag.wrapping_add(i as u8)));
    mac::push_fcs(&mut f);
    f
}

#[test]
fn ampdu_delimiters_and_the_walk() {
    // The delimiter of a 100-octet MPDU: length in bits 4 to 15, the CRC-8 of the first two octets, the signature 4E.
    let m = mpdu(72, 1);
    assert_eq!(m.len(), 100);
    let mut a = Vec::new();
    mac::push_ampdu_subframe(&mut a, &m, false);
    assert_eq!(&a[..2], &(100u16 << 4).to_le_bytes());
    assert_eq!(a[3], mac::DELIMITER_SIGNATURE);
    assert_eq!(a.len(), 104);
    // Three subframes with padding between them; the last has EOF.
    let (m2, m3) = (mpdu(33, 9), mpdu(250, 77));
    mac::push_ampdu_subframe(&mut a, &m2, false);
    mac::push_ampdu_subframe(&mut a, &m3, true);
    assert_eq!(a.len() % 4, 0);
    let got: Vec<_> = Ampdu::new(&a).collect();
    assert_eq!(got.len(), 3);
    assert_eq!((got[0].mpdu, got[1].mpdu, got[2].mpdu), (&m[..], &m2[..], &m3[..]));
    assert_eq!((got[0].eof, got[2].eof), (false, true));
    assert!(got.iter().all(|s| mac::fcs_ok(s.mpdu)));
    // A damaged first delimiter: the walk passes over it and finds the other two.
    let mut bad = a.clone();
    bad[0] ^= 0x10;
    let mut w = Ampdu::new(&bad);
    let got: Vec<_> = w.by_ref().collect();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].mpdu, &m2[..]);
    assert!(w.bad_delimiters() >= 26);
    // Padding delimiters of length 0 and trailing octets give no subframe; a length past the end is not followed.
    let mut pad = Vec::new();
    mac::push_ampdu_subframe(&mut pad, &[], true);
    mac::push_ampdu_subframe(&mut pad, &m2, true);
    pad.truncate(pad.len() - 8);
    assert_eq!(Ampdu::new(&pad).count(), 0);
    // The VHT length extension: 5000 octets use the two high bits.
    let big = vec![0x5A; 5000];
    let mut v = Vec::new();
    mac::push_ampdu_subframe(&mut v, &big, false);
    assert_eq!(u16::from_le_bytes([v[0], v[1]]), (5000u16 & 0xFFF) << 4 | 1 << 2);
    assert_eq!(Ampdu::new(&v).next().unwrap().mpdu.len(), 5000);
}

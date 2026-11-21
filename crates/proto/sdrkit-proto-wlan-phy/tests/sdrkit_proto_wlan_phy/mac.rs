//! 802.11 MAC framing (IEEE Std 802.11-2020 clause 9).

use sdrkit_proto_wlan_phy::mac::{
    self, element, ElementValue, Frame, FrameType, MgmtSubtype, Rsn, Suite,
};

#[test]
fn fcs_check_value() {
    // The CRC-32/ISO-HDLC known answer. The FCS goes least significant byte first.
    assert_eq!(mac::crc32(b"123456789"), 0xCBF4_3926);
    let mut f = b"123456789".to_vec();
    mac::push_fcs(&mut f);
    assert_eq!(&f[9..], &[0x26, 0x39, 0xF4, 0xCB]);
    assert!(mac::fcs_ok(&f));
    f[0] ^= 1;
    assert!(!mac::fcs_ok(&f));
    assert!(!mac::fcs_ok(&[1, 2, 3]));
}

#[test]
fn frame_control_bits() {
    // Beacon: type 0, subtype 8: first byte 0x80.
    let fc = mac::FrameControl::from_bytes([0x80, 0x00]);
    assert_eq!(
        (fc.ftype, fc.mgmt_subtype()),
        (FrameType::Management, Some(MgmtSubtype::Beacon))
    );
    assert_eq!(fc.bytes(), [0x80, 0x00]);
    // QoS data from the DS, protected: 0x88 0x42.
    let fc = mac::FrameControl::from_bytes([0x88, 0x42]);
    assert_eq!(fc.ftype, FrameType::Data);
    assert_eq!(fc.subtype, 8);
    assert!(fc.from_ds && fc.protected && !fc.to_ds);
    assert_eq!(fc.bytes(), [0x88, 0x42]);
    for c in 0..16 {
        assert_eq!(MgmtSubtype::from_code(c).code(), c);
    }
    assert_eq!(MgmtSubtype::ProbeResponse.name(), "Probe Response");
}

#[test]
fn beacon_build_and_parse() {
    let bssid = [0x02, 0x11, 0x22, 0x33, 0x44, 0x55];
    let mut f = Vec::new();
    mac::push_mgmt_header(
        &mut f,
        MgmtSubtype::Beacon,
        mac::BROADCAST,
        bssid,
        bssid,
        1234,
    );
    mac::push_beacon_fields(
        &mut f,
        0x0102_0304_0506_0708,
        100,
        mac::CAP_ESS | mac::CAP_PRIVACY,
    );
    mac::push_element(&mut f, element::SSID, b"cafe").unwrap();
    mac::push_element(
        &mut f,
        element::SUPPORTED_RATES,
        &[0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24],
    )
    .unwrap();
    mac::push_element(&mut f, element::DS_PARAMETER_SET, &[11]).unwrap();
    // WPA2-Personal with CCMP (clause 9.4.2.24): version 1, group CCMP, one pairwise CCMP, one AKM PSK, capabilities 0x000C.
    let rsn = [
        1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 2, 0x0C, 0,
    ];
    mac::push_element(&mut f, element::RSN, &rsn).unwrap();
    // ASTM F3411 Remote ID vendor element: OUI FA-0B-BC, type 0x0D.
    mac::push_vendor(&mut f, [0xFA, 0x0B, 0xBC], 0x0D, &[7, 1, 2, 3]).unwrap();
    mac::push_element(&mut f, element::EXTENSION, &[35, 9, 9]).unwrap();
    mac::push_fcs(&mut f);
    let fr = Frame::parse_with_fcs(&f).unwrap();
    assert_eq!(fr.seq, Some((1234, 0)));
    assert_eq!(fr.bssid(), Some(bssid));
    let b = fr.beacon().unwrap();
    assert_eq!(
        (b.timestamp, b.interval_tu, b.capability),
        (0x0102_0304_0506_0708, 100, 0x0011)
    );
    let els: Vec<_> = mac::elements(b.elements).collect();
    assert_eq!(els.len(), 6);
    assert_eq!(els[0].value(), ElementValue::Ssid(b"cafe"));
    match els[1].value() {
        ElementValue::Rates(r) => {
            let kbps: Vec<_> = r.iter().filter_map(|&x| mac::rate_kbps(x)).collect();
            assert_eq!(kbps[0], (1000, true));
            assert_eq!(kbps[2], (5500, true));
            assert_eq!(kbps[4], (6000, false));
        }
        v => panic!("{v:?}"),
    }
    assert_eq!(els[2].value(), ElementValue::DsChannel(11));
    let r = match els[3].value() {
        ElementValue::Rsn(d) => Rsn::parse(d).unwrap(),
        v => panic!("{v:?}"),
    };
    let ccmp = Suite {
        oui: mac::IEEE_OUI,
        kind: 4,
    };
    assert_eq!(r.group, Some(ccmp));
    assert_eq!(r.pairwise_suites().collect::<Vec<_>>(), [ccmp]);
    assert_eq!(
        r.akm_suites().collect::<Vec<_>>(),
        [Suite {
            oui: mac::IEEE_OUI,
            kind: 2
        }]
    );
    assert_eq!(r.capabilities, Some(0x000C));
    assert_eq!(
        mac::vendor_elements(b.elements, [0xFA, 0x0B, 0xBC], 0x0D).next(),
        Some(&[7, 1, 2, 3][..])
    );
    assert_eq!(
        (els[5].id, els[5].ext, els[5].data),
        (255, Some(35), &[9, 9][..])
    );
    // A beacon is not a probe request.
    assert!(fr.probe_request().is_none());
    // A membership selector is not a rate.
    assert_eq!(mac::rate_kbps(0xFF), None);
}

#[test]
fn truncated_and_bad_frames() {
    let body = [element::SSID, 4, b'a', b'b'];
    let mut it = mac::elements(&body);
    assert!(it.next().is_none());
    assert!(it.truncated());
    assert!(Frame::parse(&[0x80, 0, 0, 0, 1, 2, 3]).is_err());
    assert!(Frame::parse(&[0x81, 0, 0, 0, 1, 2, 3, 4, 5, 6]).is_err()); // version 1
                                                                        // Ack: frame control, duration, receiver address only.
    let ack = Frame::parse(&[0xD4, 0, 0, 0, 1, 2, 3, 4, 5, 6]).unwrap();
    assert_eq!(ack.fc.ftype, FrameType::Control);
    assert_eq!(ack.addr2, None);
    // A data frame between distribution systems has four addresses and no BSSID.
    let mut wds = vec![0x08, 0x03, 0, 0];
    wds.extend_from_slice(&[1; 18]);
    wds.extend_from_slice(&[0, 0]);
    wds.extend_from_slice(&[4; 6]);
    let f = Frame::parse(&wds).unwrap();
    assert_eq!(f.addr4, Some([4; 6]));
    assert_eq!(f.bssid(), None);
    assert!(Rsn::parse(&[1, 0, 0, 0x0F, 0xAC, 4, 9, 0]).is_err());
}

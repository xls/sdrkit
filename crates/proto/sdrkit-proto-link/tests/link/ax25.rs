use sdrkit_proto_link::ax25::{
    pid_name, xid, Address, CommandResponse, Control, Frame, Frmr, Modulo, Supervisory, Tnc2,
    Unnumbered, Xid,
};

fn u(kind: Unnumbered, pf: bool) -> Control {
    Control::U { kind, pf }
}

fn byte(c: Control, m: Modulo) -> Vec<u8> {
    let mut o = [0u8; 2];
    let n = c.write(m, &mut o).unwrap();
    o[..n].to_vec()
}

#[test]
fn unnumbered_control_fields_are_those_of_ax25_2_2_figure_4_4() {
    // AX.25 2.2 clause 4.3.3: the U frame control fields with P/F 0 and 1.
    let table = [
        (Unnumbered::Sabme, 0x6F),
        (Unnumbered::Sabm, 0x2F),
        (Unnumbered::Disc, 0x43),
        (Unnumbered::Dm, 0x0F),
        (Unnumbered::Ua, 0x63),
        (Unnumbered::Frmr, 0x87),
        (Unnumbered::Ui, 0x03),
        (Unnumbered::Xid, 0xAF),
        (Unnumbered::Test, 0xE3),
    ];
    for (k, code) in table {
        for pf in [false, true] {
            let b = code | if pf { 0x10 } else { 0 };
            assert_eq!(byte(u(k, pf), Modulo::Eight), [b]);
            for m in [Modulo::Eight, Modulo::OneTwentyEight] {
                assert_eq!(Control::parse(&[b], m).unwrap(), (u(k, pf), 1));
            }
        }
    }
    assert_eq!(u(Unnumbered::Ui, false).name(), "UI");
    assert_eq!(
        Control::parse(&[0x07], Modulo::Eight).unwrap().0,
        u(Unnumbered::Other(0x07), false)
    );
}

#[test]
fn i_and_s_control_fields_in_modulo_8_and_128() {
    // Modulo 8 (figure 4.1): I = N(R) P N(S) 0, S = N(R) P/F S S 0 1.
    let i = Control::I {
        ns: 3,
        nr: 6,
        poll: false,
    };
    assert_eq!(byte(i, Modulo::Eight), [0b1100_0110]);
    let rr = Control::S {
        kind: Supervisory::Rr,
        nr: 5,
        pf: true,
    };
    assert_eq!(byte(rr, Modulo::Eight), [0b1011_0001]);
    for (k, ss) in [
        (Supervisory::Rr, 0),
        (Supervisory::Rnr, 1),
        (Supervisory::Rej, 2),
        (Supervisory::Srej, 3),
    ] {
        let c = Control::S {
            kind: k,
            nr: 2,
            pf: false,
        };
        let b = 0b0100_0001 | (ss << 2);
        assert_eq!(byte(c, Modulo::Eight), [b]);
        assert_eq!(Control::parse(&[b], Modulo::Eight).unwrap(), (c, 1));
    }
    // Modulo 128 (figure 4.2): two bytes, N(S) 0 then N(R) P.
    let i = Control::I {
        ns: 100,
        nr: 27,
        poll: true,
    };
    assert_eq!(byte(i, Modulo::OneTwentyEight), [200, 55]);
    assert_eq!(
        Control::parse(&[200, 55], Modulo::OneTwentyEight).unwrap(),
        (i, 2)
    );
    let s = Control::S {
        kind: Supervisory::Srej,
        nr: 99,
        pf: true,
    };
    assert_eq!(byte(s, Modulo::OneTwentyEight), [0x0D, 199]);
    assert_eq!(
        Control::parse(&[0x0D, 199], Modulo::OneTwentyEight).unwrap(),
        (s, 2)
    );
    // Sequence numbers outside the modulo are refused, short input is an error.
    assert!(Control::I {
        ns: 8,
        nr: 0,
        poll: false
    }
    .write(Modulo::Eight, &mut [0; 2])
    .is_err());
    assert!(Control::parse(&[0x00], Modulo::OneTwentyEight).is_err());
    assert!(Control::parse(&[], Modulo::Eight).is_err());
}

#[test]
fn address_bytes_follow_clause_3_12() {
    // "APRS" as a command destination: characters shifted left, blank padded, SSID byte C R R SSID E = 1 11 0000 0.
    let mut d = Address::parse("APRS").unwrap();
    d.flag = true;
    assert_eq!(
        d.to_bytes(false),
        [0x82, 0xA0, 0xA4, 0xA6, 0x40, 0x40, 0xE0]
    );
    let s = Address::parse("N0CALL-9").unwrap();
    assert_eq!(s.to_bytes(true), [0x9C, 0x60, 0x86, 0x82, 0x98, 0x98, 0x73]);
    let (a, last) = Address::from_bytes(&[0x9C, 0x60, 0x86, 0x82, 0x98, 0x98, 0x73]).unwrap();
    assert_eq!(
        (a.to_string(), a.ssid, last, a.flag),
        ("N0CALL-9".to_string(), 9, true, false)
    );
    for bad in ["", "TOOLONG", "AB-16", "AB-", "A.B", "AB-1x"] {
        assert!(Address::parse(bad).is_err(), "{bad}");
    }
    // A blank inside a callsign, or the extension bit in a character, is refused.
    assert!(Address::from_bytes(&[0x82, 0x40, 0xA4, 0x40, 0x40, 0x40, 0x60]).is_err());
    assert!(Address::from_bytes(&[0x83, 0x40, 0x40, 0x40, 0x40, 0x40, 0x60]).is_err());
}

#[test]
fn a_ui_frame_from_tnc2_has_the_bytes_of_the_standard() {
    let f = Frame::from_tnc2("N0CALL-9>APRS,WIDE1-1*,WIDE2-1:!test").unwrap();
    let mut b = Vec::new();
    f.encode(&mut b).unwrap();
    assert_eq!(b.len(), f.len());
    assert_eq!(&b[..7], &[0x82, 0xA0, 0xA4, 0xA6, 0x40, 0x40, 0xE0]); // destination, command
    assert_eq!(b[13], 0x72); // source SSID 9, C = 0, not last
    assert_eq!(b[20], 0x80 | 0x60 | (1 << 1)); // WIDE1-1 repeated
    assert_eq!(b[27], 0x60 | (1 << 1) | 1); // WIDE2-1 last
    assert_eq!(&b[28..30], &[0x03, 0xF0]);
    assert_eq!(&b[30..], b"!test");
    let g = Frame::parse(&b).unwrap();
    assert_eq!(g.command_response(), CommandResponse::Command);
    assert_eq!(g.to_string(), "N0CALL-9>APRS,WIDE1-1*,WIDE2-1:!test");
    assert_eq!(g, f);
    // Repeaters before the starred one get H too.
    let f = Frame::from_tnc2("A>B,C,D*,E:x").unwrap();
    let h: Vec<bool> = f.repeaters().iter().map(|r| r.flag).collect();
    assert_eq!(h, [true, true, false]);
    assert_eq!(f.to_string(), "A>B,C,D*,E:x");
    // APRS-IS constructs cannot go on the air, but Tnc2 splits them.
    assert!(Frame::from_tnc2("A>B,qAR,TCPIP*:x").is_err());
    let t = Tnc2::parse("VERYLONGCALL>APRS,TCPIP*,qAC,T2:hello\r\n").unwrap();
    assert_eq!(
        (t.source, t.destination, t.info),
        ("VERYLONGCALL", "APRS", "hello")
    );
    assert_eq!(t.path().collect::<Vec<_>>(), ["TCPIP*", "qAC", "T2"]);
    assert!(Tnc2::parse("no header").is_err());
    assert!(Frame::from_tnc2("A>B,1,2,3,4,5,6,7,8,9:x").is_err());
}

#[test]
fn frames_of_every_kind_parse_and_encode() {
    let d = Address::parse("DEST").unwrap();
    let s = Address::parse("SRC-1").unwrap();
    let cases = [
        (
            Control::I {
                ns: 1,
                nr: 2,
                poll: true,
            },
            Some(0xCC),
            &b"ip"[..],
        ),
        (
            Control::S {
                kind: Supervisory::Rnr,
                nr: 4,
                pf: false,
            },
            None,
            &b""[..],
        ),
        (u(Unnumbered::Sabm, true), None, &b""[..]),
        (u(Unnumbered::Test, false), None, &b"echo"[..]),
    ];
    for (c, pid, info) in cases {
        let f = Frame::new(d, s, &[], c, pid, info).unwrap();
        let mut b = Vec::new();
        f.encode(&mut b).unwrap();
        let g = Frame::parse(&b).unwrap();
        assert_eq!((g.control, g.pid, g.info), (c, pid, info));
        assert_eq!(g.command_response(), CommandResponse::Version1);
    }
    // Modulo 128 needs the modulo to parse.
    let mut f = Frame::new(
        d,
        s,
        &[],
        Control::I {
            ns: 90,
            nr: 3,
            poll: false,
        },
        Some(0xF0),
        b"x",
    )
    .unwrap();
    f.modulo = Modulo::OneTwentyEight;
    let mut b = Vec::new();
    f.encode(&mut b).unwrap();
    assert_eq!(
        Frame::parse_with(&b, Modulo::OneTwentyEight)
            .unwrap()
            .control,
        f.control
    );
    // A PID against the frame type is refused.
    assert!(Frame::new(d, s, &[], u(Unnumbered::Ua, false), Some(0xF0), b"").is_err());
    assert!(Frame::new(d, s, &[d; 9], Control::UI, Some(0xF0), b"").is_err());
    assert_eq!(pid_name(0xF0), Some("no layer 3"));
    assert_eq!(pid_name(0xCF), Some("NET/ROM"));
    assert_eq!(pid_name(0x00), None);
}

#[test]
fn xid_and_frmr_information_fields() {
    // XID (clause 4.3.3.7): FI 0x82, GI 0x80, group length, then PI PL PV. Window size receive 7, I field length receive 2048 bits.
    let info = [
        0x82,
        0x80,
        0x00,
        0x07,
        xid::WINDOW_RX,
        1,
        7,
        xid::I_LENGTH_RX,
        2,
        0x08,
        0x00,
    ];
    let x = Xid::parse(&info).unwrap();
    assert_eq!(x.params().count(), 2);
    assert_eq!(x.value(xid::WINDOW_RX), Some(7));
    assert_eq!(x.value(xid::I_LENGTH_RX), Some(2048));
    assert_eq!(x.value(xid::RETRIES), None);
    assert!(Xid::parse(&[0x82, 0x81, 0, 0]).is_err());
    assert!(Xid::parse(&[0x82, 0x80, 0, 9, 1]).is_err());
    // FRMR modulo 8 (figure 4.6): rejected control, V(R) C/R V(S) 0, then 0000 Z Y X W.
    let f = Frmr::parse(&[0x0F, (5 << 5) | 0x10 | (3 << 1), 0b1001], Modulo::Eight).unwrap();
    assert_eq!(
        (f.rejected_control, f.vr, f.vs, f.response),
        (0x0F, 5, 3, true)
    );
    assert_eq!((f.w, f.x, f.y, f.z), (true, false, false, true));
    let f = Frmr::parse(
        &[0x02, 0x00, 100 << 1, (90 << 1) | 1, 0b0110],
        Modulo::OneTwentyEight,
    )
    .unwrap();
    assert_eq!(
        (f.vs, f.vr, f.response, f.x, f.y),
        (100, 90, true, true, true)
    );
    assert!(Frmr::parse(&[1, 2], Modulo::Eight).is_err());
}

#[test]
fn parsing_a_frame_does_not_allocate() {
    let f = Frame::from_tnc2("N0CALL-9>APRS,WIDE1-1*,WIDE2-1:!4903.50N/07201.75W-Test").unwrap();
    let mut b = Vec::new();
    f.encode(&mut b).unwrap();
    let mut n = 0;
    let allocs = crate::allocations(|| {
        for _ in 0..1000 {
            n += Frame::parse(&b).unwrap().repeaters().len();
        }
    });
    assert_eq!((allocs, n), (0, 2000));
}

#[test]
fn bad_frames_give_errors_not_panics() {
    let mut seed = 12345u32;
    let mut buf = [0u8; 80];
    for n in 0..2000 {
        for b in buf.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (seed >> 24) as u8;
        }
        let len = n % 80;
        let _ = Frame::parse(&buf[..len]);
        let _ = Frame::parse_with(&buf[..len], Modulo::OneTwentyEight);
    }
    // Eleven addresses.
    let mut b = Vec::new();
    for _ in 0..11 {
        b.extend_from_slice(&Address::parse("AB").unwrap().to_bytes(false));
    }
    assert!(Frame::parse(&b).is_err());
    assert!(Frame::parse(&Address::parse("AB").unwrap().to_bytes(true)).is_err());
}

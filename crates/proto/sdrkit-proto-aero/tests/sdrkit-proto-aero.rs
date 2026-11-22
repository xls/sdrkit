//! The tests of sdrkit-proto-aero, one module per part of the crate.

/// A real uplink received on air (an ADS-C periodic contract request from Prestwick to HB-JHM). Its ARINC 622 CRC is good.
const PIKCPYA: &str = "/PIKCPYA.ADS.HB-JHM07040B000C000D010E011000440D";

mod acars_blocks {
    use sdrkit_proto_aero::acars::{self, syndrome, Block, BlockSpec, Check, End, NAK};
    use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;

    #[test]
    fn characters_carry_odd_parity() {
        // ARINC 618: every character has odd parity in bit 7. The bit sync is "+*", the suffix DEL.
        assert_eq!(acars::with_parity(b'+'), 0xAB);
        assert_eq!(acars::with_parity(b'*'), 0x2A);
        assert_eq!(acars::with_parity(acars::ETX_CHAR), acars::ETX);
        assert_eq!(acars::ETX, 0x83);
        assert_eq!(acars::with_parity(acars::ETB_CHAR), acars::ETB);
        assert_eq!(acars::ETB, 0x97);
        assert_eq!(acars::with_parity(acars::ACK), 0x86);
        assert_eq!(acars::with_parity(acars::STX), 0x02);
        assert_eq!(acars::with_parity(acars::SOH), 0x01);
        assert_eq!(acars::with_parity(acars::SYN), 0x16);
        assert_eq!(acars::with_parity(acars::DEL), 0x7F);
        assert_eq!(acars::with_parity(NAK), 0x15);
        assert_eq!(acars::BIT_SYNC_SENT, [0xAB, 0x2A]);
    }

    #[test]
    fn bcs_is_crc16_kermit() {
        // The CRC-16/KERMIT check value of "123456789" is 0x2189, sent low byte first.
        assert_eq!(CRC16_KERMIT.checksum(b"123456789"), 0x2189);
        assert_eq!(acars::bcs(b"123456789"), [0x89, 0x21]);
    }

    #[test]
    fn syndromes_match_rfc1662_table() {
        // RFC 1662 Appendix C, fcstab[1], [2], [4] ... [128].
        let t = [
            0x1189, 0x2312, 0x4624, 0x8C48, 0x1081, 0x2102, 0x4204, 0x8408,
        ];
        for (k, &v) in t.iter().enumerate() {
            assert_eq!(syndrome(k), Some(v), "bit {k}");
        }
        assert_eq!(syndrome((acars::MAX_BODY + 2) * 8), None);
    }

    pub fn h1() -> (Vec<u8>, [u8; 2]) {
        let mut b = Vec::new();
        let c = BlockSpec::new('2', "N123AB", b'2', End::Etx)
            .label("H1")
            .text("M01AUA0901#DFB TEST MESSAGE FROM ACARS 1234567")
            .body(&mut b)
            .unwrap();
        (b, c)
    }

    #[test]
    fn a_received_uplink_frame_is_rebuilt_byte_for_byte() {
        // The uplink that carried PIKCPYA, as an independent receiver framed it: 67 bytes, BCS 0x93 0xAB.
        let text = &super::PIKCPYA;
        let mut f = Vec::new();
        BlockSpec::new('2', "HB-JHM", b'Q', End::Etx)
            .label("A6")
            .text(text)
            .frame(2, false, &mut f)
            .unwrap();
        assert_eq!(f.len(), 67);
        assert_eq!(&f[f.len() - 3..f.len() - 1], &[0x93, 0xAB]);
        let fr = acars::split_frame(&f).unwrap();
        assert_eq!(fr.len, f.len());
        let mut body = fr.body.to_vec();
        assert_eq!(
            acars::check_and_repair(&mut body, fr.bcs),
            Check::Good { bits_fixed: 0 }
        );
        assert_eq!(acars::strip_parity(&mut body), 0);
        let b = Block::parse(&body).unwrap();
        assert_eq!(
            (b.mode, b.address, b.tak, b.label_str(), b.block_id),
            (b'2', ".HB-JHM", NAK, "A6", b'Q')
        );
        assert_eq!(b.registration(), "HB-JHM");
        assert_eq!(b.text, Some(*text));
        assert!(!b.downlink());
        assert_eq!(b.app_text(), *text);
    }

    #[test]
    fn frames_with_sync_and_without_text() {
        let mut f = Vec::new();
        BlockSpec::new('2', "N12345", b'A', End::Etx)
            .frame(16, true, &mut f)
            .unwrap();
        let fr = acars::split_frame(&f).unwrap();
        let mut body = fr.body.to_vec();
        acars::strip_parity(&mut body);
        let b = Block::parse(&body).unwrap();
        assert_eq!(b.label_str(), "_d");
        assert_eq!(b.text, None);
        assert_eq!(b.app_text(), "");
        // A short or broken frame is an error, never a panic.
        for n in 0..f.len() - 1 {
            let _ = acars::split_frame(&f[..n]);
        }
        assert!(acars::split_frame(&[0x55; 30]).is_err());
        // The longest body is MAX_BODY bytes, from the mode to the ETX.
        for (n, ok) in [(acars::MAX_BODY, true), (acars::MAX_BODY + 1, false)] {
            let mut f = vec![acars::SOH];
            f.extend(std::iter::repeat_n(b'A', n - 1));
            f.extend_from_slice(&[acars::ETX, 0, 0]);
            assert_eq!(acars::split_frame(&f).is_ok(), ok, "body of {n}");
        }
        assert!(Block::parse(&[0; 5]).is_err());
    }

    #[test]
    fn the_builder_refuses_what_a_block_cannot_hold() {
        let mut out = Vec::new();
        let long = "X".repeat(221);
        let spec = BlockSpec::new('2', "N1", b'1', End::Etx);
        assert!(spec.text(&long).body(&mut out).is_err());
        assert!(spec.text("A\u{3}B").body(&mut out).is_err());
        assert!(BlockSpec::new('2', "TOOLONGREG", b'1', End::Etx)
            .body(&mut out)
            .is_err());
        assert!(spec.label("H").body(&mut out).is_err());
        assert!(out.is_empty());
        assert!(spec.text(&long[..220]).body(&mut out).is_ok());
    }

    #[test]
    fn every_single_bit_error_is_repaired() {
        let (b, c) = h1();
        for i in 0..b.len() {
            for bit in 0..8 {
                let mut x = b.clone();
                x[i] ^= 1 << bit;
                assert_eq!(
                    acars::check_and_repair(&mut x, c),
                    Check::Good { bits_fixed: 1 },
                    "byte {i} bit {bit}"
                );
                assert_eq!(x, b);
            }
        }
        for j in 0..2 {
            for bit in 0..8 {
                let mut cc = c;
                cc[j] ^= 1 << bit;
                let mut x = b.clone();
                assert_eq!(
                    acars::check_and_repair(&mut x, cc),
                    Check::Good { bits_fixed: 1 }
                );
                assert_eq!(x, b);
            }
        }
    }

    #[test]
    fn two_bits_in_a_byte_and_three_bytes_are_repaired_four_are_not() {
        let (b, c) = h1();
        for i in [0usize, 5, 20, b.len() - 1] {
            let mut x = b.clone();
            x[i] ^= 0b0010_0100;
            assert_eq!(
                acars::check_and_repair(&mut x, c),
                Check::Good { bits_fixed: 2 }
            );
            assert_eq!(x, b);
        }
        let mut x = b.clone();
        x[3] ^= 1;
        x[17] ^= 8;
        x[30] ^= 64;
        assert_eq!(
            acars::check_and_repair(&mut x, c),
            Check::Good { bits_fixed: 3 }
        );
        assert_eq!(x, b);
        x[3] ^= 1;
        x[17] ^= 8;
        x[30] ^= 64;
        x[40] ^= 2;
        assert_eq!(acars::check_and_repair(&mut x, c), Check::Bad);
        // One wrong bit in a byte and one in the BCS.
        let mut x = b.clone();
        x[9] ^= 4;
        let mut cc = c;
        cc[1] ^= 0x10;
        assert_eq!(
            acars::check_and_repair(&mut x, cc),
            Check::Good { bits_fixed: 2 }
        );
        assert_eq!(x, b);
    }

    #[test]
    fn downlink_prefix_and_sublabels() {
        let mut body = Vec::new();
        BlockSpec::new('2', "N1", b'1', End::Etx)
            .label("H1")
            .text("M01AXX0001#M1B/PS POSITION")
            .body(&mut body)
            .unwrap();
        acars::strip_parity(&mut body);
        let b = Block::parse(&body).unwrap();
        let p = b.downlink_prefix().unwrap();
        assert_eq!(
            (p.msn, p.sequence, p.flight, p.rest),
            ("M01", b'A', "XX0001", "#M1B/PS POSITION")
        );
        let s = b.sublabel().unwrap();
        assert_eq!((s.sublabel, s.mfi), ("M1", Some("PS")));
        let s = acars::sublabel("- #MD/AT CLEARANCE", false).unwrap();
        assert_eq!((s.sublabel, s.mfi), ("MD", Some("AT")));
        assert_eq!(acars::next_block_id(b'9'), b'0');
        assert_eq!(acars::next_block_id(b'Z'), b'A');
        assert_eq!(acars::next_block_id(b'C'), b'D');
        let mut s = String::new();
        assert!(!acars::printable_into(b"A\r\nB\rC\x01\x7F", 100, &mut s));
        assert_eq!(s, "A\nB\nC..");
    }
}

mod assembly {
    use sdrkit_proto_aero::acars::assemble::{Assembler, Joined, Quality, MAX_JOINED};
    use sdrkit_proto_aero::acars::{self, Block, BlockSpec, End, Message};

    #[allow(clippy::too_many_arguments)]
    fn feed(
        a: &mut Assembler<()>,
        t: f64,
        reg: &str,
        label: &str,
        id: u8,
        text: &str,
        end: End,
        out: &mut Vec<Joined<()>>,
    ) {
        let mut body = Vec::new();
        BlockSpec::new('2', reg, id, end)
            .label(label)
            .text(text)
            .body(&mut body)
            .unwrap();
        acars::strip_parity(&mut body);
        let b = Block::parse(&body).unwrap();
        a.push(t, &b, Quality::GOOD, 0, (), out);
    }

    #[test]
    fn three_blocks_join() {
        let mut a = Assembler::new();
        let mut out = Vec::new();
        feed(
            &mut a,
            1.0,
            "N1",
            "H1",
            b'1',
            "M01AXX0001WX OMDB 1200Z ",
            End::Etb,
            &mut out,
        );
        feed(
            &mut a,
            2.0,
            "N1",
            "H1",
            b'2',
            "M01BXX0001WIND 270/12 ",
            End::Etb,
            &mut out,
        );
        assert!(out.is_empty());
        assert_eq!(a.open(), 1);
        feed(
            &mut a,
            3.0,
            "N1",
            "H1",
            b'3',
            "M01CXX0001QNH 1008",
            End::Etx,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        let m = Message::from_joined(&out[0]);
        assert_eq!(m.text, "WX OMDB 1200Z WIND 270/12 QNH 1008");
        assert_eq!(
            (m.blocks, m.incomplete, m.flight.as_str(), m.msn.as_str()),
            (3, false, "XX0001", "M01")
        );
    }

    #[test]
    fn a_repeat_is_dropped_and_a_gap_marks_the_message() {
        let mut a = Assembler::new();
        let mut out = Vec::new();
        feed(
            &mut a,
            1.0,
            "N1",
            "H1",
            b'1',
            "M01AXX0001AAA",
            End::Etb,
            &mut out,
        );
        feed(
            &mut a,
            1.5,
            "N1",
            "H1",
            b'1',
            "M01AXX0001AAA",
            End::Etb,
            &mut out,
        );
        feed(
            &mut a,
            2.0,
            "N1",
            "H1",
            b'3',
            "M01CXX0001CCC",
            End::Etx,
            &mut out,
        );
        let m = Message::from_joined(&out[0]);
        assert_eq!(
            (m.text.as_str(), m.blocks, m.incomplete),
            ("AAACCC", 2, true)
        );
    }

    #[test]
    fn an_uplink_resend_is_dropped() {
        let mut a = Assembler::new();
        let mut out = Vec::new();
        feed(
            &mut a,
            1.0,
            "N1",
            "80",
            b'A',
            "FPL PART ONE ",
            End::Etb,
            &mut out,
        );
        feed(
            &mut a,
            1.5,
            "N1",
            "80",
            b'A',
            "FPL PART ONE ",
            End::Etb,
            &mut out,
        );
        feed(
            &mut a,
            2.0,
            "N1",
            "80",
            b'B',
            "PART TWO",
            End::Etx,
            &mut out,
        );
        let m = Message::from_joined(&out[0]);
        assert_eq!(
            (m.text.as_str(), m.blocks, m.incomplete),
            ("FPL PART ONE PART TWO", 2, false)
        );
    }

    #[test]
    fn caps_and_timeouts() {
        let mut a = Assembler::new();
        let mut out = Vec::new();
        for i in 0..100 {
            feed(
                &mut a,
                1.0 + i as f64 * 0.01,
                &format!("N{i}"),
                "H1",
                b'1',
                "M01AXX0001PART",
                End::Etb,
                &mut out,
            );
        }
        assert_eq!(a.open(), 64);
        assert_eq!(out.len(), 36);
        assert!(out.iter().all(|j| !j.complete));
        out.clear();
        a.expire(100.0, &mut out);
        assert_eq!((a.open(), out.len()), (0, 64));
        // A long message stops growing at the cap.
        let mut out = Vec::new();
        let chunk = "X".repeat(200);
        for i in 0..60u8 {
            let end = if i == 59 { End::Etx } else { End::Etb };
            let seq = (b'A' + (i % 26)) as char;
            feed(
                &mut a,
                i as f64,
                "N9",
                "H1",
                b'1' + (i % 9),
                &format!("M01{seq}XX0001{chunk}"),
                end,
                &mut out,
            );
        }
        let m = Message::from_joined(out.last().unwrap());
        assert!(m.text.len() <= MAX_JOINED);
    }
}

mod readings_and_labels {
    use sdrkit_proto_aero::labels::{self, LABELS};
    use sdrkit_proto_aero::readings::{media_advisory, oooi};

    #[test]
    fn oooi_reports() {
        assert_eq!(
            oooi("QA", "OMDB1030").unwrap().to_string(),
            "from OMDB out 1030"
        );
        assert_eq!(
            oooi("Q1", "OMDB07140722145114592500EGLL")
                .unwrap()
                .to_string(),
            "from OMDB out 0714 off 0722 on 1451 in 1459 to EGLL"
        );
        assert_eq!(
            oooi("QM", "EGLL    OMDB").unwrap().to_string(),
            "from OMDB to EGLL"
        );
        assert!(oooi("QB", "EGLL07").is_none());
        assert!(oooi("QB", "egll0722").is_none());
        assert!(oooi("QB", "EGLL2961").is_none());
        assert!(oooi("H1", "EGLL1200").is_none());
    }

    #[test]
    fn media_advisories() {
        assert_eq!(
            media_advisory("0EV123456SH").unwrap().to_string(),
            "link established: VHF ACARS at 12:34:56, available: default SATCOM, HF"
        );
        assert_eq!(
            media_advisory("0L2235959/FREE TEXT").unwrap().to_string(),
            "link lost: VDL2 at 23:59:59"
        );
        assert!(media_advisory("1EV123456SH").is_none());
        assert!(media_advisory("0EV126045SH").is_none());
        assert!(media_advisory("0EV123456SHVS2XIGCV").is_none());
    }

    #[test]
    fn the_label_table_is_sorted_unique_and_merged() {
        assert!(LABELS.windows(2).all(|w| w[0].code < w[1].code));
        assert!(LABELS.iter().all(|l| l.code.len() == 2 && !l.verified));
        assert_eq!(LABELS.len(), 118);
        assert_eq!(labels::meaning("A6"), Some("ADS-C uplink"));
        assert_eq!(labels::meaning("_d"), Some("General response"));
        assert_eq!(labels::meaning("2N"), Some("Takeoff report"));
        assert_eq!(
            labels::find("Q1").unwrap().alternative,
            Some("OOOI report (out, off, on, in)")
        );
        assert_eq!(labels::meaning("ZZ"), None);
    }
}

mod text_positions {
    use sdrkit_proto_aero::textpos::{read, sane, Form};

    #[test]
    fn pos_reports() {
        let p = read("H1", "#M1BPOSN25312E055214,PARAR,125000,350,XYZ").unwrap();
        assert!((p.position.lat_deg - (25.0 + 31.2 / 60.0)).abs() < 1e-9);
        assert!((p.position.lon_deg - (55.0 + 21.4 / 60.0)).abs() < 1e-9);
        assert_eq!(
            (p.alt_ft, p.sec_of_day, p.form),
            (Some(35000.0), Some(12 * 3600 + 50 * 60), Form::PosReport)
        );
        let p = read("H1", "POSN43312W123174,EASON,215754,370").unwrap();
        assert!(
            (p.position.lat_deg - 43.52).abs() < 1e-9 && (p.position.lon_deg + 123.29).abs() < 1e-9
        );
        let p = read("H1", "#M1BPOS/ID12345,/PSN42579W108090,173207,320,").unwrap();
        assert!((p.position.lat_deg - 42.965).abs() < 1e-9);
        assert_eq!(p.alt_ft, Some(32000.0));
        assert!(read("H1", "POSN25612E055214,X,125000,350").is_none());
    }

    #[test]
    fn label_20_and_decimal_forms() {
        let q = read("20", "POSN25312W055214,,125000,350").unwrap();
        assert!(
            (q.position.lat_deg - 25.312).abs() < 1e-9
                && (q.position.lon_deg + 55.214).abs() < 1e-9
        );
        assert_eq!(q.form, Form::Thousandths);
        let r = read("16", "N 44.203,W 86.546,31965,6, 290").unwrap();
        assert!(
            (r.position.lat_deg - 44.203).abs() < 1e-9
                && (r.position.lon_deg + 86.546).abs() < 1e-9
        );
        assert_eq!((r.alt_ft, r.form), (Some(31965.0), Form::Decimal));
        let s = read("16", "N 28.177/W 96.055").unwrap();
        assert!((s.position.lon_deg + 96.055).abs() < 1e-9);
        assert_eq!(
            read("12", "S 33.946,E 151.177,GRD").unwrap().alt_ft,
            Some(0.0)
        );
        for t in [
            "POS Bogus message",
            "#M1BPOS Bogus message",
            "METAR EGLL 121220Z 24012KT",
            "POSUNKNOWN",
            "POS",
            "é POSN",
        ] {
            assert!(read("H1", t).is_none(), "{t}");
        }
    }

    #[test]
    fn the_position_test() {
        let p = sane(10.0, 190.0).unwrap();
        assert_eq!((p.lat_deg, p.lon_deg), (10.0, -170.0));
        assert!(sane(0.0, 0.0).is_none());
        assert!(sane(91.0, 0.0).is_none());
        assert!(sane(10.0, 361.0).is_none());
        assert!(sane(f64::NAN, 0.0).is_none());
    }
}

mod envelopes {
    use sdrkit_proto_aero::arinc622::{self, Imi};
    use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;

    #[test]
    fn the_crc_is_crc16_genibus() {
        // CRC-16/GENIBUS: check value of "123456789" is 0xD64E.
        assert_eq!(CRC16_GENIBUS.checksum(b"123456789"), 0xD64E);
    }

    #[test]
    fn a_received_envelope_is_found_and_rebuilt() {
        let e = arinc622::find(super::PIKCPYA, "HB-JHM").unwrap();
        assert_eq!(
            (e.ground, e.imi, e.address),
            ("PIKCPYA", Imi::Ads, ".HB-JHM")
        );
        assert!(e.crc_ok);
        let mut buf = [0u8; 32];
        let data = e.binary(&mut buf).unwrap().to_vec();
        let mut s = String::from("/");
        arinc622::build("PIKCPYA", Imi::Ads, "HB-JHM", &data, &mut s).unwrap();
        assert_eq!(s, super::PIKCPYA);
        // One changed hex digit fails the CRC.
        let bad = super::PIKCPYA.replacen("0B00", "0B01", 1);
        assert!(!arinc622::find(&bad, "").unwrap().crc_ok);
    }

    #[test]
    fn an_address_sent_without_its_dots() {
        let mut s = String::new();
        arinc622::build("ABCDEFG", Imi::Ads, "N12", &[7, 1, 2], &mut s).unwrap();
        assert!(s.starts_with("ABCDEFG.ADS....N12"));
        let short = s.replacen("....N12", "N12", 1);
        let e = arinc622::find(&short, "N12").unwrap();
        assert_eq!(e.address, "N12");
        assert!(e.crc_ok);
        // The extra separator dot that an earlier envelope builder wrote.
        let extra = super::PIKCPYA.replacen("ADS.HB", "ADS..HB", 1);
        assert!(arinc622::find(&extra, "HB-JHM").unwrap().crc_ok);
        assert!(arinc622::build("AB", Imi::Ads, "N1", &[], &mut s).is_err());
        assert!(arinc622::find("no envelope here", "").is_none());
        assert!(arinc622::unhex("0G", &mut [0u8; 4]).is_err());
    }
}

mod adsc_groups {
    use sdrkit_proto_aero::adsc::{
        self, groups, request, Content, Fom, Group, Report, ReportKind, Request, RequestGroup,
        Waypoint,
    };
    use sdrkit_proto_aero::arinc622::{self, Imi};

    #[test]
    fn a_received_contract_request_reads_completely() {
        let a = adsc::decode(super::PIKCPYA, false, "HB-JHM").unwrap();
        assert!(a.crc_ok);
        assert_eq!(
            (a.ground.as_str(), a.aircraft.as_str()),
            ("PIKCPYA", "HB-JHM")
        );
        let Content::Uplink {
            request,
            groups,
            complete,
        } = a.content
        else {
            panic!()
        };
        assert!(complete);
        assert_eq!(request, Request::Periodic { contract: 4 });
        assert_eq!(
            groups,
            [
                RequestGroup::ReportingInterval {
                    scaling: 0,
                    rate: 0
                },
                RequestGroup::Modulus {
                    tag: 12,
                    modulus: 0
                },
                RequestGroup::Modulus {
                    tag: 13,
                    modulus: 1
                },
                RequestGroup::Modulus {
                    tag: 14,
                    modulus: 1
                },
                RequestGroup::Modulus {
                    tag: 16,
                    modulus: 0
                },
            ]
        );
        let mut bin = Vec::new();
        request.encode(&mut bin);
        for g in &groups {
            g.encode(&mut bin).unwrap();
        }
        assert_eq!(bin, [7, 4, 0x0B, 0, 0x0C, 0, 0x0D, 1, 0x0E, 1, 0x10, 0]);
        assert_eq!(
            RequestGroup::ReportingInterval {
                scaling: 2,
                rate: 5
            }
            .interval_s(),
            Some(320)
        );
    }

    #[test]
    fn the_basic_report_bit_layout() {
        // Fields MSB first: lat 21, lon 21, alt 16, time 15, figure of merit 7 (TCAS bit 4, accuracy bits 1 to 3, redundancy
        // bit 0), 80 bits in all.
        let lsb = 90.0 / f64::from(1 << 19);
        let r = Report {
            kind: ReportKind::Basic,
            lat_deg: 1000.0 * lsb,
            lon_deg: -2.0 * lsb,
            alt_ft: 36000.0,
            sec_past_hour: 1234.5,
            fom: Fom {
                accuracy: 6,
                redundancy: true,
                tcas: true,
            },
        };
        let want: u128 = (1000u128 << 59)
            | (((1u128 << 21) - 2) << 38)
            | (9000u128 << 22)
            | (9876u128 << 7)
            | 0b001_1101;
        let mut bin = Vec::new();
        Group::Report(r).encode(&mut bin).unwrap();
        assert_eq!(bin[0], 7);
        assert_eq!(&bin[1..], &want.to_be_bytes()[6..]);
        assert_eq!(groups(&bin).next().unwrap().unwrap(), Group::Report(r));
    }

    #[test]
    fn every_downlink_group_round_trips() {
        let w = |lat, lon, alt| Waypoint {
            lat_deg: lat,
            lon_deg: lon,
            alt_ft: alt,
        };
        let all = [
            Group::Ack { contract: 3 },
            Group::Nack {
                contract: 3,
                reason: 2,
                extended: Some(9),
            },
            Group::Nack {
                contract: 3,
                reason: 4,
                extended: None,
            },
            Group::Noncompliance {
                contract: 5,
                groups: 0,
            },
            Group::CancelEmergency,
            Group::Report(Report {
                kind: ReportKind::Emergency,
                lat_deg: -33.946,
                lon_deg: 151.177,
                alt_ft: 37004.0,
                sec_past_hour: 1234.375,
                fom: Fom {
                    accuracy: 5,
                    redundancy: false,
                    tcas: true,
                },
            }),
            Group::flight_id("QFA1").unwrap(),
            Group::PredictedRoute {
                next: w(-33.5, 151.0, 37000.0),
                eta_s: 600,
                then: w(-33.0, 148.8, 39000.0),
            },
            Group::EarthReference {
                track_deg: 270.0,
                valid: true,
                gs_kt: 471.5,
                vs_fpm: -1200.0,
            },
            Group::AirReference {
                heading_deg: 45.0,
                valid: false,
                mach: 0.82,
                vs_fpm: 64.0,
            },
            Group::Meteo {
                wind_kt: 42.5,
                valid: true,
                wind_dir_deg: 265.78125,
                temp_c: -54.25,
            },
            Group::Airframe { icao: 0x7C1234 },
            Group::IntermediateIntent {
                distance_nm: 12.5,
                valid: true,
                track_deg: 90.0,
                alt_ft: 1000.0,
                eta_s: 300,
            },
            Group::FixedIntent {
                point: w(10.0, 20.0, 3000.0),
                eta_s: 77,
            },
        ];
        let mut bin = Vec::new();
        for g in &all {
            g.encode(&mut bin).unwrap();
        }
        let back: Vec<Group> = groups(&bin).map(Result::unwrap).collect();
        assert_eq!(back.len(), all.len());
        for (a, b) in all.iter().zip(&back) {
            match (a, b) {
                (Group::Report(x), Group::Report(y)) => {
                    assert!(
                        (x.lat_deg - y.lat_deg).abs() < 2e-4
                            && (x.lon_deg - y.lon_deg).abs() < 2e-4
                    );
                    assert_eq!(
                        (x.alt_ft, x.sec_past_hour, x.fom, x.kind),
                        (y.alt_ft, y.sec_past_hour, y.fom, y.kind)
                    );
                }
                (
                    Group::PredictedRoute {
                        next: n1, then: t1, ..
                    },
                    Group::PredictedRoute {
                        next: n2, then: t2, ..
                    },
                ) => {
                    assert!(
                        (n1.lat_deg - n2.lat_deg).abs() < 2e-4
                            && (t1.lon_deg - t2.lon_deg).abs() < 2e-4
                    );
                }
                (Group::FixedIntent { point: p1, .. }, Group::FixedIntent { point: p2, .. }) => {
                    assert!((p1.lat_deg - p2.lat_deg).abs() < 2e-4);
                }
                (Group::AirReference { mach: m1, .. }, Group::AirReference { mach: m2, .. }) => {
                    assert!((m1 - m2).abs() < 1e-9);
                }
                _ => assert_eq!(a, b),
            }
        }
        assert_eq!(back[6].flight_id_text(), Some("QFA1"));
    }

    #[test]
    fn a_downlink_report_in_an_envelope() {
        let mut bin = Vec::new();
        Group::Report(Report {
            kind: ReportKind::Basic,
            lat_deg: 51.5,
            lon_deg: -0.4,
            alt_ft: 36000.0,
            sec_past_hour: 100.0,
            fom: Fom {
                accuracy: 6,
                redundancy: true,
                tcas: true,
            },
        })
        .encode(&mut bin)
        .unwrap();
        Group::EarthReference {
            track_deg: 90.0,
            valid: true,
            gs_kt: 450.0,
            vs_fpm: 0.0,
        }
        .encode(&mut bin)
        .unwrap();
        let mut t = String::from("/");
        arinc622::build("TESTGND", Imi::Ads, "G-EUUA", &bin, &mut t).unwrap();
        let a = adsc::decode(&t, true, "G-EUUA").unwrap();
        assert!(a.crc_ok);
        let p = a.position().unwrap();
        assert!((p.lat_deg - 51.5).abs() < 2e-4);
        assert_eq!(a.earth_reference(), Some((90.0, 450.0, 0.0)));
        // A broken CRC returns no position.
        let bad = t.replacen("TESTGND.ADS.G-EUUA0", "TESTGND.ADS.G-EUUA1", 1);
        assert!(adsc::decode(&bad, true, "").unwrap().position().is_none());
    }

    #[test]
    fn bad_groups_end_the_reading() {
        let mut bin = vec![99, 1, 2];
        assert!(groups(&bin).next().unwrap().is_err());
        bin = vec![7, 1, 2];
        let mut it = groups(&bin);
        assert!(it.next().unwrap().is_err());
        assert!(it.next().is_none());
        assert!(request(&[]).is_err());
        assert!(request(&[55]).is_err());
        assert!(Group::Report(Report {
            kind: ReportKind::Basic,
            lat_deg: 0.0,
            lon_deg: 0.0,
            alt_ft: 1e9,
            sec_past_hour: 0.0,
            fom: Fom {
                accuracy: 0,
                redundancy: false,
                tcas: false
            }
        })
        .encode(&mut bin)
        .is_err());
        assert!(Group::flight_id("BAD!").is_err());
    }
}

mod cpdlc_messages {
    use sdrkit_proto_aero::arinc622::Imi;
    use sdrkit_proto_aero::cpdlc::{
        self, element_text, read, write, Body, Element, Header, Message,
    };
    use sdrkit_util_bits::per_schema::Value;

    /// The real envelopes of FANS-1/A messages: four AT1 downlinks and a CR1.
    pub const AT1: [&str; 4] = [
        "/SOUCAYA.AT1.HL8251243F880C3D903BB412903604FE326C2479F4A64F7F62528B1A9CF8382738186AC28B16668E013DF464D8A7F0",
        "/MSTEC7X.AT1.VT-ANKA094D88C3D903BB465D0723053B2E5123CFA53279400014B0894A2C6A73CBD8F52447AF1244CB4C9B94600089D65C84314892694587510528B1A9CF41169D440C1AB36A08B42",
        "/MELCAYA.AT1.ZK-OKC253C21CC3D903BA178F96618F0B28024B83127CD7886A12E9D85266B927584A9169C1A8EEB2800EEA7",
        "/MSTEC7X.AT1.VT-ANE21409DCC3DD03BB52350490502B2E5129D5A15692BA009A08892E7CC831E210A4C06EEBC28B1662BC02360165C80E1F7",
    ];
    const CR1: &str = "- #MD/AA ATLTWXA.CR1.N856DN203A3AA8E5C1A9323EDD";

    fn header(id: u8) -> Header {
        Header {
            msg_id: id,
            msg_ref: None,
            timestamp: None,
        }
    }

    fn climb(fl: i64) -> Message {
        Message {
            header: header(7),
            elements: vec![Element {
                number: 20,
                param: Value::Choice(6, Box::new(Value::Int(fl))),
            }],
        }
    }

    fn free_text(id: u8, s: &str) -> Message {
        Message {
            header: header(id),
            elements: vec![Element {
                number: 67,
                param: Value::Seq(vec![Some(Value::Text(s.into()))]),
            }],
        }
    }

    #[test]
    fn the_worked_uplink_and_downlink_bodies() {
        let mut b = Vec::new();
        write(&climb(350), true, &mut b).unwrap();
        assert_eq!(b, [0x03, 0x8A, 0x65, 0x00]);
        let mut b = Vec::new();
        write(
            &free_text(12, "REQUEST HIGHER DUE TURBULENCE"),
            false,
            &mut b,
        )
        .unwrap();
        assert_eq!(&b[..3], [0x06, 0x21, 0x8E]);
        // 1 + 2 + 6 + 8 + 8 bits, then 29 characters of 7 bits, padded to octets.
        assert_eq!(b.len(), (25 + 29 * 7usize).div_ceil(8));
        assert_eq!(
            read(&b, false).unwrap(),
            free_text(12, "REQUEST HIGHER DUE TURBULENCE")
        );
    }

    #[test]
    fn the_downlink_element_number_has_8_bits() {
        // The first real envelope: identification 8, timestamp 15:56:32, element 48 (POSITION REPORT).
        let c = cpdlc::decode(AT1[0], true, "").unwrap();
        assert!(c.crc_ok);
        let m = c.message().expect("decodes");
        assert_eq!(
            m.header,
            Header {
                msg_id: 8,
                msg_ref: None,
                timestamp: Some((15, 56, 32))
            }
        );
        assert_eq!(m.elements[0].number, 48);
        assert_eq!(c.summary(), "POSITION REPORT [positionreport]");
        let t = c.text();
        assert!(t.contains("Timestamp: 15:56:32"), "{t}");
        assert!(t.contains("Flight level: 340"), "{t}");
        assert!(t.contains("Fix: GORLO"), "{t}");
        assert_eq!(cpdlc::DOWNLINK_ELEMENTS, 129);
        assert_eq!(
            element_text(false, 80),
            Some("DEVIATING [distanceoffset][direction] OF ROUTE")
        );
        assert_eq!(element_text(false, 81), None);
        assert_eq!(element_text(true, 182), Some("CONFIRM ATIS CODE"));
    }

    #[test]
    fn real_envelopes_decode_completely() {
        for t in AT1 {
            let c = cpdlc::decode(t, true, "").unwrap();
            assert_eq!(c.imi, Imi::At1);
            assert!(c.crc_ok, "{t}");
            assert!(c.parsed(), "{t}");
            assert!(c.text().len() > 40, "{t}");
        }
        let c = cpdlc::decode(CR1, false, "").unwrap();
        assert_eq!(
            (c.imi, c.ground.as_str(), c.aircraft.as_str()),
            (Imi::Cr1, "ATLTWXA", "N856DN")
        );
        assert!(c.crc_ok && c.parsed(), "{:?}", c.body);
        assert!(c
            .description()
            .starts_with("FANS-1/A CPDLC connect request (CR1)"));
    }

    #[test]
    fn uplink_climb_clearances() {
        for fl in [290, 350, 410] {
            let mut t = String::new();
            cpdlc::build("EGTTYGA", "G-ABCD", &climb(fl), true, &mut t).unwrap();
            assert!(t.starts_with("/EGTTYGA.AT1.G-ABCD"));
            let c = cpdlc::decode(&t, false, "").unwrap();
            assert!(c.crc_ok && c.parsed());
            assert_eq!(
                (c.ground.as_str(), c.aircraft.as_str()),
                ("EGTTYGA", "G-ABCD")
            );
            let text = c.text();
            assert!(text.contains("CLIMB TO AND MAINTAIN"), "{text}");
            assert!(text.contains(&format!("Flight level: {fl}")), "{text}");
            // Read as a downlink: the check stays good.
            let d = cpdlc::decode(&t, true, "").unwrap();
            assert!(d.crc_ok);
            let _ = d.text();
            // Lower-case hex.
            let hex_at = "/EGTTYGA.AT1.G-ABCD".len();
            let lower = format!("{}{}", &t[..hex_at], t[hex_at..].to_lowercase());
            let l = cpdlc::decode(&lower, false, "").unwrap();
            assert!(l.crc_ok && l.parsed());
            // One hex digit changed.
            let mut bad = t.clone().into_bytes();
            bad[hex_at + 1] = if bad[hex_at + 1] == b'0' { b'1' } else { b'0' };
            let bad = String::from_utf8(bad).unwrap();
            let b = cpdlc::decode(&bad, false, "").unwrap();
            assert!(!b.crc_ok);
            assert!(b.description().ends_with(", CRC FAILED"));
            // Every cut: no fault, never a long text from a broken body.
            for n in (0..t.len()).step_by(5) {
                if let Some(c) = cpdlc::decode(&t[..n], false, "") {
                    assert!(c.text().len() < 200, "{n}");
                }
            }
        }
    }

    #[test]
    fn downlink_free_text() {
        let mut t = String::new();
        cpdlc::build(
            "KZNYCAY",
            "N123AB",
            &free_text(12, "REQUEST HIGHER DUE TURBULENCE"),
            false,
            &mut t,
        )
        .unwrap();
        let c = cpdlc::decode(&t, true, "").unwrap();
        assert!(c.crc_ok && c.parsed());
        assert!(c.text().contains("REQUEST HIGHER DUE TURBULENCE"));
        assert!(c.description().contains("FANS-1/A CPDLC message"));
        assert_eq!(c.summary(), "[freetext]");
    }

    #[test]
    fn other_texts_are_not_cpdlc() {
        assert!(cpdlc::decode("/AKLCDYA.ADS.9V-SVG21D0755D8", true, "").is_none());
        assert!(cpdlc::decode("RANDOM TEXT .AT1. no address", true, "").is_none());
        // A 7-character ground address preceded by an eighth address character is not an address.
        assert!(cpdlc::decode("XEGTTYGA.AT1.G-ABCD038A6500", false, "").is_none());
        let c = cpdlc::decode("/EGTTYGA.CC1.G-ABCD", false, "").unwrap();
        assert_eq!(c.body, Body::Empty);
        assert_eq!(c.text(), "(no message body)");
    }

    #[test]
    fn header_bounds_and_element_range() {
        let full = Message {
            header: Header {
                msg_id: 63,
                msg_ref: Some(63),
                timestamp: Some((23, 59, 59)),
            },
            elements: (0..5)
                .map(|n| Element {
                    number: n,
                    param: Value::Null,
                })
                .collect(),
        };
        let mut b = Vec::new();
        write(&full, true, &mut b).unwrap();
        assert_eq!(read(&b, true).unwrap(), full);
        let mut m = climb(350);
        m.header.msg_id = 64;
        assert!(write(&m, true, &mut b).is_err());
        let wilco = |n| Message {
            header: header(1),
            elements: vec![Element {
                number: n,
                param: Value::Null,
            }],
        };
        assert!(write(&wilco(128), false, &mut b).is_ok());
        assert!(write(&wilco(129), false, &mut b).is_err());
        // Flight level 20 is below the range.
        assert!(write(&climb(20), true, &mut b).is_err());
    }
}

mod messages {
    use sdrkit_proto_aero::acars::assemble::Quality;
    use sdrkit_proto_aero::acars::{self, Block, BlockSpec, End, Message, Reading, Tak};

    fn message(reg: &str, label: &str, id: u8, text: &str) -> Message {
        let mut body = Vec::new();
        BlockSpec::new('2', reg, id, End::Etx)
            .label(label)
            .text(text)
            .body(&mut body)
            .unwrap();
        acars::strip_parity(&mut body);
        Message::from_block(&Block::parse(&body).unwrap(), Quality::GOOD)
    }

    #[test]
    fn an_oooi_downlink() {
        let m = message("A6-EDA", "QA", b'3', "M01AEK0201OMDB0714");
        assert_eq!(
            (m.mode, m.registration.as_str(), m.tak, m.block_id),
            ('2', "A6-EDA", Tak::Nak, '3')
        );
        assert_eq!(
            (m.msn.as_str(), m.flight.as_str(), m.text.as_str()),
            ("M01", "EK0201", "OMDB0714")
        );
        assert_eq!(m.label_meaning, Some("Out / fuel report"));
        let Reading::Oooi(o) = &m.reading else {
            panic!("{:?}", m.reading)
        };
        assert_eq!(o.to_string(), "from OMDB out 0714");
    }

    #[test]
    fn adsc_cpdlc_and_text_positions_are_read() {
        let m = message("HB-JHM", "A6", b'Q', super::PIKCPYA);
        assert!(matches!(&m.reading, Reading::Adsc(a) if a.crc_ok));
        let m = message(
            "N1",
            "H1",
            b'1',
            "M01AXX0001#M1BPOSN43312W123174,EASON,215754,370",
        );
        assert_eq!(
            (m.sublabel.as_deref(), m.mfi.as_deref()),
            (Some("M1"), None)
        );
        let (p, alt) = m.position().unwrap();
        assert!((p.lat_deg - 43.52).abs() < 1e-9);
        assert_eq!(alt, Some(37000.0));
        let m = message("N1", "SA", b'1', "M01AXX00010EV123456SH");
        assert!(matches!(m.reading, Reading::MediaAdvisory(_)));
        // An uplink is read, but what it says is not the aircraft's position.
        let m = message("N1", "H1", b'A', "POSN43312W123174,EASON,215754,370");
        assert!(matches!(m.reading, Reading::Format(_)));
        assert_eq!(m.position(), None);
    }
}

mod message_log {
    use sdrkit_proto_aero::msglog::{
        classify, flight_key, redact, reg_key, Category::*, MessageLog, Offer,
    };

    #[test]
    fn classification() {
        assert_eq!(classify("_d", "ANYTHING HERE"), Ignore);
        assert_eq!(classify("Q0", "LINK TEST TEXT"), Ignore);
        assert_eq!(classify("SA", "0EV123456SH"), Ignore);
        assert_eq!(classify("H1", "ab"), Ignore);
        assert_eq!(classify("H1", &"0123456789ABCDEF".repeat(3)), Ignore);
        assert_eq!(classify("H1", "..?..;..:.,.,.;.."), Ignore);
        assert_eq!(classify("80", "PLEASE ADVISE GATE FOR ARRIVAL"), Free);
        assert_eq!(classify("ZZ", "REQUEST CATERING FOR TURNAROUND"), Free);
        assert_eq!(classify("ZZ", "XJ91KQ"), Ignore);
        assert_eq!(classify("Q1", "OMDB07140722145114592500EGLL"), Ops);
        assert_eq!(classify("5Z", "COMPANY REQUEST FOR GATE"), Company);
        assert_eq!(classify("H1", "CFDS REPORT 32-41 FAULT L MLG"), Fault);
        assert_eq!(classify("ZZ", "MAINT MESSAGE: CFDS PRINT REQUESTED"), Maint);
        assert_eq!(classify("ZZ", "CAPT REQUESTS WHEELCHAIR AT GATE"), Crew);
    }

    #[test]
    fn redaction_and_keys() {
        assert_eq!(redact("CALL +971 4 123 4567 NOW"), "CALL [number] NOW");
        assert_eq!(redact("TEL 0044 20 7946 0958"), "TEL [number]");
        assert_eq!(redact("TEL 370-612-34567 OK"), "TEL [number] OK");
        assert_eq!(
            redact("EK0201 GATE B12 0540 0545 0550"),
            "EK0201 GATE B12 0540 0545 0550"
        );
        assert_eq!(redact("MAIL ops.desk@example.com NOW"), "MAIL [email] NOW");
        assert_eq!(
            redact("POSN25312E055214,125000,350"),
            "POSN25312E055214,125000,350"
        );
        assert_eq!(reg_key("a6-e.da "), "A6EDA");
        assert_eq!(flight_key("EK0201"), flight_key("EK201"));
    }

    fn offer<'a>(
        t: f64,
        icao: Option<u32>,
        reg: &'a str,
        flight: &'a str,
        text: &'a str,
    ) -> Offer<'a> {
        Offer {
            t,
            link: "VDL2",
            freq_hz: Some(136.975e6),
            icao,
            reg,
            flight,
            label: "80",
            text,
        }
    }

    #[test]
    fn attachment_and_bounds() {
        let mut l = MessageLog::new();
        assert!(l.offer(&offer(0.0, None, "A6-EDA", "", "PLEASE ADVISE GATE ONE")));
        assert_eq!(l.for_aircraft(1.0, None, "A6EDA", "", false).len(), 1);
        assert!(l.offer(&offer(
            1.0,
            Some(0x896123),
            "A6-EDA",
            "",
            "PLEASE ADVISE GATE TWO"
        )));
        assert_eq!(l.for_aircraft(2.0, Some(0x896123), "", "", false).len(), 2);
        assert!(l.offer(&offer(2.0, None, "A6-EDA", "", "PLEASE ADVISE GATE THREE")));
        assert_eq!(l.for_aircraft(3.0, Some(0x896123), "", "", false).len(), 3);
        assert!(!l.offer(&offer(10.0, None, "A6-EDA", "", "PLEASE ADVISE GATE THREE")));
        assert!(l.offer(&offer(3.0, None, "", "EK0201", "PLEASE ADVISE GATE FOUR")));
        assert_eq!(l.for_aircraft(4.0, None, "", "EK201", true).len(), 1);
        assert_eq!(l.for_aircraft(4.0, None, "", "EK201", false).len(), 0);
        assert!(!l.offer(&offer(3.0, None, "", "", "PLEASE ADVISE GATE FIVE")));
        for i in 0..80 {
            let t = format!("PLEASE ADVISE GATE {i} {}", "WORD ".repeat(120));
            l.offer(&offer(100.0 + i as f64 * 40.0, None, "N9", "", &t));
        }
        let v = l.for_aircraft(5000.0, None, "N9", "", false);
        assert_eq!(v.len(), 50);
        assert!(v.iter().all(|e| e.text.chars().count() <= 500));
        assert!(l
            .for_aircraft(100.0 + 49.0 * 3600.0 + 4000.0, None, "N9", "", false)
            .is_empty());
        l.prune(1e7);
        assert_eq!(l.aircraft(), 0);
        for i in 0..450 {
            l.offer(&offer(
                i as f64,
                None,
                &format!("N{i}"),
                "",
                "PLEASE ADVISE GATE NOW",
            ));
        }
        assert_eq!(l.aircraft(), 400);
    }
}

mod aircraft_table {
    use sdrkit_proto_aero::aircraft::{report_time, AircraftTable, Fix, Motion, Source};
    use sdrkit_util_geo::Position;
    use sdrkit_util_objects::Stamp;

    fn fix(lat: f64, lon: f64) -> Fix {
        Fix {
            position: Position::new(lat, lon),
            alt_ft: None,
            on_ground: false,
            source: Source::Text,
        }
    }

    #[test]
    fn rows_fold_from_registration_to_address() {
        let mut t: AircraftTable = AircraftTable::new(10);
        let a = t.heard(None, ".VH-OQA", Stamp::at(5.0)).unwrap();
        a.message("QFA1", "H1", "FIRST TEXT");
        a.position(5.0, fix(-33.0, 151.0), None).unwrap();
        let a = t.heard(Some(0x7C1234), "VH-OQA", Stamp::at(20.0)).unwrap();
        a.message("", "B6", "SECOND TEXT");
        a.position(
            20.0,
            fix(-33.5, 151.5),
            Some(Motion {
                track_deg: 200.0,
                gs_kt: 450.0,
                vs_fpm: None,
            }),
        )
        .unwrap();
        assert_eq!(t.len(), 1);
        let a = t.get(Some(0x7C1234), "").unwrap();
        assert_eq!(
            (a.messages, a.flight.as_str(), a.last_label.as_str()),
            (2, "QFA1", "B6")
        );
        assert_eq!(a.registration, "VH-OQA");
        assert_eq!(a.country, Some("Australia"));
        assert_eq!(a.track_deg, Some(200.0));
        assert_eq!(a.trail.len(), 2);
        assert_eq!(t.search("qfa").count(), 1);
        assert_eq!(t.get(Some(0x7C1234), "").unwrap().last_text, "SECOND TEXT");
        // A later message with only the registration finds the address row.
        t.heard(None, "VH-OQA", Stamp::at(30.0))
            .unwrap()
            .message("", "H1", "THIRD");
        assert_eq!(t.len(), 1);
        assert_eq!(t.get(None, "VH-OQA").unwrap().messages, 3);
        assert!(t.heard(None, "", Stamp::at(1.0)).is_none());
        assert!(t.heard(Some(1), "", Stamp::at(f64::NAN)).is_none());
    }

    #[test]
    fn a_reported_track_stays_until_two_newer_positions_replace_it() {
        let mut t: AircraftTable = AircraftTable::new(10);
        let a = t.heard(Some(0x7C1234), "", Stamp::at(0.0)).unwrap();
        let m = Motion {
            track_deg: 45.0,
            gs_kt: 450.0,
            vs_fpm: None,
        };
        a.position(0.0, fix(0.0, 0.0), Some(m)).unwrap();
        // The bearing from the reported fix to the next point (east) does not replace the reported track.
        a.position(60.0, fix(0.0, 0.2), None).unwrap();
        assert_eq!(a.track_deg, Some(45.0));
        // Two points after the report give the bearing between them (north).
        a.position(120.0, fix(0.2, 0.2), None).unwrap();
        assert!((a.track_deg.unwrap() - 0.0).abs() < 0.1);
        assert_eq!(a.track_reported_at, Some(0.0));
    }

    #[test]
    fn trail_settings_and_change_without_hearing() {
        use sdrkit_proto_aero::aircraft::{TrailSettings, TRAIL_POINTS};
        use sdrkit_util_objects::Spacing;
        let mut t: AircraftTable = AircraftTable::new(10);
        assert_eq!(t.trail().points, TRAIL_POINTS);
        let a = t.heard(Some(0x7C1234), "", Stamp::at(0.0)).unwrap();
        for i in 0..10 {
            a.position(i as f64 * 2.0, fix(0.0, i as f64 * 0.01), None)
                .unwrap();
        }
        assert_eq!(a.trail.len(), 10);
        // 80 points 5 s apart: the rows keep the points the new spacing allows.
        let s = TrailSettings {
            points: 80,
            spacing: Spacing::seconds(5.0),
        };
        t.set_trail(s);
        let times: Vec<f64> = t
            .get(Some(0x7C1234), "")
            .unwrap()
            .trail
            .points()
            .map(|p| p.t)
            .collect();
        assert_eq!(times, [0.0, 6.0, 12.0, 18.0]);
        // A new row gets the new settings.
        let b = t.heard(Some(0x7C0001), "", Stamp::at(30.0)).unwrap();
        b.position(30.0, fix(1.0, 1.0), None).unwrap();
        b.position(32.0, fix(1.0, 1.1), None).unwrap();
        assert_eq!(b.trail.len(), 1);
        // A change through get_mut does not count as hearing: the older row stays the older.
        t.get_mut(Some(0x7C1234), "")
            .unwrap()
            .flight
            .push_str("QFA1");
        let order: Vec<_> = t.newest_first().map(|r| r.record.icao).collect();
        assert_eq!(order, [Some(0x7C0001), Some(0x7C1234)]);
        assert_eq!(t.get(Some(0x7C1234), "").unwrap().flight, "QFA1");
        assert!(t.get_mut(Some(0x123456), "").is_none());
        // A restored table keeps the settings of the table that adopts it.
        let saved: AircraftTable =
            serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        let mut fresh: AircraftTable = AircraftTable::new(10);
        fresh.adopt(saved);
        assert_eq!(fresh.trail(), TrailSettings::default());
        assert_eq!(fresh.get(Some(0x7C1234), "").unwrap().trail.len(), 4);
    }

    #[test]
    fn registration_from_the_address_and_motion() {
        let mut t: AircraftTable = AircraftTable::new(10);
        let a = t.heard(Some(0xA00001), "", Stamp::at(0.0)).unwrap();
        assert_eq!(a.registration, "N1");
        a.position(0.0, fix(50.0, 0.0), None).unwrap();
        a.position(600.0, fix(50.0, 0.5), None).unwrap();
        assert!((a.track_deg.unwrap() - 90.0).abs() < 1.0);
        let d = a.derived(3.0, 7200.0).unwrap();
        assert!((d.gs_kt - 19.3 * 6.0).abs() < 2.0);
        a.gs_kt = Some(d.gs_kt);
        let e = a.estimate(1200.0).unwrap();
        assert!(e.lon_deg > 0.9 && e.lon_deg < 1.1);
        assert!(a.position(f64::INFINITY, fix(1.0, 1.0), None).is_err());
    }

    #[test]
    fn tables_of_two_windows_merge() {
        let mut a: AircraftTable<f64> = AircraftTable::new(10);
        let mut b: AircraftTable<f64> = AircraftTable::new(10);
        let r = a.heard(Some(0x7C1234), "VH-OQA", Stamp::at(10.0)).unwrap();
        r.message("QFA1", "H1", "X");
        r.position(10.0, fix(-33.0, 151.0), None).unwrap();
        r.extra = 1.5e9;
        let r = b.heard(Some(0x7C1234), "", Stamp::at(20.0)).unwrap();
        r.message("", "B6", "Y");
        r.position(20.0, fix(-33.5, 151.5), None).unwrap();
        b.heard(Some(0x7C9999), "VH-ABC", Stamp::at(21.0)).unwrap();
        a.merge_from(&b);
        assert_eq!(a.len(), 2);
        let r = a.get(Some(0x7C1234), "").unwrap();
        assert_eq!(
            (r.messages, r.registration.as_str(), r.extra),
            (2, "VH-OQA", 1.5e9)
        );
        assert_eq!(r.fix.unwrap().1.position.lat_deg, -33.5);
    }

    #[test]
    fn an_adsc_report_updates_the_row() {
        use sdrkit_proto_aero::adsc::{self, Fom, Group, Report, ReportKind, Waypoint};
        use sdrkit_proto_aero::arinc622::{self, Imi};
        let w = Waypoint {
            lat_deg: -33.0,
            lon_deg: 150.0,
            alt_ft: 37000.0,
        };
        let mut bin = Vec::new();
        for g in [
            Group::Report(Report {
                kind: ReportKind::Basic,
                lat_deg: -33.9,
                lon_deg: 151.2,
                alt_ft: 36000.0,
                sec_past_hour: 10.0,
                fom: Fom {
                    accuracy: 6,
                    redundancy: true,
                    tcas: true,
                },
            }),
            Group::flight_id("QFA1").unwrap(),
            Group::EarthReference {
                track_deg: 270.0,
                valid: true,
                gs_kt: 450.0,
                vs_fpm: 0.0,
            },
            Group::PredictedRoute {
                next: w,
                eta_s: 60,
                then: w,
            },
        ] {
            g.encode(&mut bin).unwrap();
        }
        let mut text = String::new();
        arinc622::build("SYDCAYA", Imi::Ads, "VH-OQA", &bin, &mut text).unwrap();
        let m = adsc::decode(&text, true, "VH-OQA").unwrap();
        let mut t: AircraftTable = AircraftTable::new(10);
        let r = t.heard(None, "VH-OQA", Stamp::at(5.0)).unwrap();
        assert!(r.adsc(5.0, &m));
        assert_eq!(
            (r.flight.as_str(), r.track_deg, r.gs_kt),
            ("QFA1", Some(270.0), Some(450.0))
        );
        assert_eq!(r.fix.unwrap().1.alt_ft, Some(36000.0));
        assert!(r.route.is_some());
    }

    #[test]
    fn a_saved_table_restores_with_its_trails_links_and_countries() {
        use sdrkit_util_objects::Shift;
        let mut t: AircraftTable = AircraftTable::new(10);
        let heard_at = Stamp::at(10.0).with_wall(1_762_646_410.2);
        let a = t.heard(Some(0x7C1234), "VH-OQA", heard_at).unwrap();
        a.message("QFA1", "H1", "TEXT");
        a.position(10.0, fix(-33.0, 151.0), None).unwrap();
        a.position(70.0, fix(-33.0, 151.2), None).unwrap();
        t.heard(None, "N12345", Stamp::at(30.0)).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let mut back: AircraftTable = serde_json::from_str(&json).unwrap();
        back.shift_times(-60.0);
        let mut u: AircraftTable = AircraftTable::new(1);
        u.adopt(back);
        // The cap of 1 keeps the most recently heard row: the address row, heard at 10 s, left. N12345 stays.
        assert_eq!(u.len(), 1);
        assert!(u.get(None, "N12345").is_some());
        let mut back: AircraftTable = serde_json::from_str(&json).unwrap();
        back.shift_times(-60.0);
        let a = back.get(None, "VH-OQA").unwrap(); // found through the saved registration link
                                                   // ICAO Annex 10 Vol. III Table 9-1: 7C0000 to 7FFFFF is Australia.
        assert_eq!(a.country, Some("Australia"));
        assert_eq!(a.fix.unwrap().0, 10.0);
        assert_eq!(
            a.trail.points().map(|p| p.t).collect::<Vec<_>>(),
            [-50.0, 10.0]
        );
        assert_eq!(
            back.rows()
                .seen(&sdrkit_proto_aero::aircraft::Key::Address(0x7C1234))
                .unwrap()
                .last_s,
            -50.0
        );
        assert!(serde_json::from_str::<AircraftTable>("{\"rows\":1}").is_err());
    }

    #[test]
    fn rows_publish_the_standard_columns_then_the_link_data() {
        use sdrkit_core::publish::field;
        use sdrkit_core::publish::{Declare, Field, Published, TableDef};
        use sdrkit_core::Value;
        use sdrkit_proto_aero::aircraft::{columns, Key};
        use sdrkit_util_objects::publish::{Cells, Object};
        use sdrkit_util_objects::{publish, Seen, Shift};

        /// A link's own data: the carrier the aircraft was last heard on.
        #[derive(Clone, Default)]
        struct Carrier(f64);
        impl Object for Carrier {
            fn write(&self, _: Option<&Seen>, _: f64, out: &mut impl Cells) {
                out.set(&FREQUENCY, self.0);
            }
        }
        impl Shift for Carrier {
            fn shift_times(&mut self, _: f64) {}
        }
        const FREQUENCY: Field<field::Hz> = Field::new("frequency", "Frequency", "Hz");
        const COLUMNS: &[&dyn Declare] = &[
            &columns::ADDRESS,
            &columns::REGISTRATION,
            &columns::COUNTRY,
            &columns::FLIGHT,
            &columns::LABEL,
            &columns::MESSAGES,
            &columns::POSITION,
            &columns::ALT_FT,
            &columns::TRACK_DEG,
            &columns::GS_KT,
            &columns::VS_FPM,
            &sdrkit_util_objects::publish::columns::LAST_HEARD,
            &FREQUENCY,
        ];
        const AIRCRAFT: TableDef = TableDef {
            key: "aircraft",
            label: "Aircraft",
            columns: COLUMNS,
            cap: 10,
            marks: sdrkit_core::RowMarks::NONE,
        };
        assert_eq!(columns::STANDARD.len() + 1, COLUMNS.len());
        let mut t: AircraftTable<Carrier> = AircraftTable::new(10);
        let heard_at = Stamp::at(10.0).with_wall(1_762_646_410.2);
        let a = t.heard(Some(0x7C1234), "VH-OQA", heard_at).unwrap();
        a.message("QFA1", "H1", "TEXT");
        a.extra = Carrier(1_545_100_000.0);
        let m = Motion {
            track_deg: 90.0,
            gs_kt: 480.0,
            vs_fpm: Some(-500.0),
        };
        a.position(
            10.0,
            Fix {
                alt_ft: Some(37000.0),
                ..fix(-33.0, 151.0)
            },
            Some(m),
        )
        .unwrap();
        t.heard(None, "N12345", Stamp::at(12.0)).unwrap();
        assert_eq!(Key::Address(0x00ABCD).to_string(), "00ABCD");
        assert_eq!(Key::Registration("N12345".into()).to_string(), "N12345");

        let mut p = Published::new(&[]).with_tables(&[&AIRCRAFT]);
        publish::rows(t.rows(), &AIRCRAFT, 25.0, &mut p);
        publish::points(t.rows(), "aircraft/", p.results_mut());
        let r = p.results();
        let table = r.table(r.decl().table_id("aircraft").unwrap()).unwrap();
        let row = |id: &str| {
            table
                .row(id)
                .unwrap()
                .iter()
                .map(|(_, v)| Some(v.clone()))
                .collect::<Vec<_>>()
        };
        let text = |s: &str| Some(Value::Text(s.into()));
        assert_eq!(
            row("7C1234"),
            [
                text("7C1234"),
                text("VH-OQA"),
                text("Australia"),
                text("QFA1"),
                text("H1"),
                Some(Value::I64(1)),
                Some(Value::Position {
                    lat: -33.0,
                    lon: 151.0
                }),
                Some(Value::F64(37000.0)),
                Some(Value::F64(90.0)),
                Some(Value::F64(480.0)),
                Some(Value::F64(-500.0)),
                Some(Value::Time(1_762_646_410.0)),
                Some(Value::Hz(1_545_100_000.0)),
            ]
        );
        // A row without an address, flight or position has no value in those cells, and no map point.
        assert_eq!(row("N12345")[0], Some(Value::None));
        assert_eq!(row("N12345")[6], Some(Value::None));
        let pt = &r.points()["aircraft/7C1234"];
        assert_eq!((pt.label.as_str(), pt.heading), ("QFA1", Some(90.0)));
        assert_eq!(r.points().len(), 1);

        // Heard again within the same second with the same values: no counter moves.
        let before = p.results().change_counter();
        t.heard(
            Some(0x7C1234),
            "VH-OQA",
            Stamp::at(10.5).with_wall(1_762_646_410.7),
        )
        .unwrap();
        publish::rows(t.rows(), &AIRCRAFT, 26.0, &mut p);
        assert_eq!(p.results().change_counter(), before);
        // In the next second, only the heard time changes.
        t.heard(
            Some(0x7C1234),
            "VH-OQA",
            Stamp::at(11.0).with_wall(1_762_646_411.2),
        )
        .unwrap();
        publish::rows(t.rows(), &AIRCRAFT, 27.0, &mut p);
        let r = p.results();
        let table = r.table(r.decl().table_id("aircraft").unwrap()).unwrap();
        let changed: Vec<&str> = table.rows_changed_since(before).map(|(k, _)| k).collect();
        assert_eq!(changed, ["7C1234"]);
        assert_eq!(
            table
                .row("7C1234")
                .unwrap()
                .get(&sdrkit_util_objects::publish::columns::LAST_HEARD),
            Some(1_762_646_411.0)
        );
    }

    #[test]
    fn report_times() {
        // Wall clock 10:00:30: 3500 s past the hour is in the future. Thus the report is from 09:58:20.
        assert_eq!(report_time(None, Some(3500.0), 36_030.0), 32_400.0 + 3500.0);
        assert_eq!(report_time(Some(100.0), Some(5.0), 36_030.0), 100.0);
        assert_eq!(report_time(None, None, 86_400.0 + 30.0), 30.0);
    }
}

mod no_allocation {
    use sdrkit_proto_aero::acars::{self, Block, BlockSpec, End};
    use sdrkit_proto_aero::{adsc, arinc622, readings, textpos};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    struct Counting;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    // Counts the allocations of the current thread only, so the other tests of this program can run beside it.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            unsafe { System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static A: Counting = Counting;

    fn count(f: impl FnOnce()) -> usize {
        let before = COUNT.with(Cell::get);
        f();
        COUNT.with(Cell::get) - before
    }

    #[test]
    fn the_receive_path_does_not_allocate() {
        let mut frame = Vec::new();
        BlockSpec::new('2', "HB-JHM", b'Q', End::Etx)
            .label("A6")
            .text(super::PIKCPYA)
            .frame(2, true, &mut frame)
            .unwrap();
        let mut body = [0u8; acars::MAX_BODY];
        let mut bin = [0u8; 64];
        let _ = acars::syndrome(0);
        let n = count(|| {
            for i in 0..1000 {
                let f = acars::split_frame(&frame).unwrap();
                let body = &mut body[..f.body.len()];
                body.copy_from_slice(f.body);
                body[i % body.len()] ^= 1 << (i % 8);
                assert!(matches!(
                    acars::check_and_repair(body, f.bcs),
                    acars::Check::Good { .. }
                ));
                acars::strip_parity(body);
                let b = Block::parse(body).unwrap();
                let e = arinc622::find(b.app_text(), b.registration()).unwrap();
                let data = e.binary(&mut bin).unwrap();
                let (_, groups) = adsc::request(data).unwrap();
                assert_eq!(groups.count(), 5);
                assert!(adsc::groups(&[7; 11]).next().is_some());
                assert!(textpos::read("H1", "POSN43312W123174,EASON,215754,370").is_some());
                assert!(readings::oooi("QA", "OMDB1030").is_some());
                assert!(readings::media_advisory("0EV123456SH").is_some());
            }
        });
        assert_eq!(n, 0);
    }
}

mod no_panic {
    use sdrkit_proto_aero::acars::{self, Block};
    use sdrkit_proto_aero::{adsc, arinc622, cpdlc, readings, textpos};

    /// A small seeded generator (xorshift64*), so the inputs are the same on every run.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn bytes(&mut self, out: &mut Vec<u8>, max: usize) {
            out.clear();
            let n = (self.next() as usize) % (max + 1);
            out.extend((0..n).map(|_| self.next() as u8));
        }
        fn text(&mut self, out: &mut String, max: usize) {
            const PARTS: [&str; 12] = [
                ".",
                "ADS",
                ".AT1.",
                "/",
                "POS",
                "N",
                "E",
                "W",
                "S",
                ",",
                "0123456789",
                "ABCDEF.DIS.",
            ];
            out.clear();
            let n = (self.next() as usize) % max;
            for _ in 0..n {
                let k = self.next() as usize;
                if k.is_multiple_of(3) {
                    out.push((b' ' + (k % 95) as u8) as char);
                } else {
                    out.push_str(PARTS[k % PARTS.len()]);
                }
            }
        }
    }

    #[test]
    fn random_bytes_and_texts_never_panic() {
        let mut r = Rng(0x5EED_1234_ABCD_0001);
        let mut b = Vec::new();
        let mut t = String::new();
        let mut buf = [0u8; 512];
        for _ in 0..5000 {
            r.bytes(&mut b, 300);
            if let Ok(f) = acars::split_frame(&b) {
                let mut body = f.body.to_vec();
                let _ = acars::check_and_repair(&mut body, f.bcs);
                acars::strip_parity(&mut body);
                if let Ok(blk) = Block::parse(&body) {
                    let _ = (blk.downlink_prefix(), blk.sublabel(), blk.app_text());
                }
            }
            let mut body = b.clone();
            let _ = acars::check_and_repair(&mut body, [b.first().copied().unwrap_or(0), 7]);
            acars::strip_parity(&mut body);
            let _ = Block::parse(&body);
            let _ = adsc::groups(&b).count();
            if let Ok((_, g)) = adsc::request(&b) {
                let _ = g.count();
            }
            let _ = cpdlc::read(&b, true);
            let _ = cpdlc::read(&b, false);
            r.text(&mut t, 60);
            if let Some(e) = arinc622::find(&t, "N1") {
                let _ = e.binary(&mut buf);
            }
            let _ = adsc::decode(&t, true, "");
            let _ = cpdlc::decode(&t, false, "");
            let _ = textpos::read("H1", &t);
            let _ = textpos::read("20", &t);
            let _ = textpos::read("16", &t);
            let _ = readings::oooi("Q1", &t);
            let _ = readings::media_advisory(&t);
            let _ = acars::read_text("H1", "M01AXX0001", &t, true, "N1");
        }
    }
}

mod air_data {
    use sdrkit_proto_aero::air::{
        declination_deg, isa_ft, isa_m, sat_from_tas_mach, wind_triangle, A0_KT,
    };

    #[test]
    fn standard_atmosphere_matches_doc_7488_tables() {
        // ICAO Doc 7488/3 (and the US Standard Atmosphere 1976 table 1): geopotential altitude (m), temperature (K), pressure (Pa).
        for (h, t_k, p_pa) in [
            (0.0, 288.15, 101_325.0),
            (1_000.0, 281.65, 89_874.6),
            (5_000.0, 255.65, 54_019.9),
            (11_000.0, 216.65, 22_632.1),
            (15_000.0, 216.65, 12_044.6),
            (20_000.0, 216.65, 5_474.89),
            (32_000.0, 228.65, 868.019),
        ] {
            let a = isa_m(h).unwrap();
            assert!((a.temp_c + 273.15 - t_k).abs() < 1e-6, "{h}: {}", a.temp_c);
            let rel = (a.pressure_hpa * 100.0 - p_pa).abs() / p_pa;
            assert!(rel < 2e-5, "{h}: {}", a.pressure_hpa);
        }
        let a = isa_ft(10_000.0).unwrap();
        assert!((a.temp_c + 4.812).abs() < 1e-3 && (a.pressure_hpa - 696.82).abs() < 0.05);
        assert!(isa_m(32_001.0).is_err() && isa_m(-5_001.0).is_err() && isa_m(f64::NAN).is_err());
    }

    #[test]
    fn temperature_from_airspeed_and_mach() {
        // At the ISA sea level speed of sound, Mach 1 is 15 degrees C.
        assert!((sat_from_tas_mach(A0_KT, 1.0).unwrap() - 15.0).abs() < 1e-9);
        // ISA at FL350: -54.3 degrees C, speed of sound 576.4 kt, Mach 0.8 is 461.1 kt TAS.
        let t = sat_from_tas_mach(0.8 * 576.4, 0.8).unwrap();
        assert!((t + 54.3).abs() < 0.2, "{t}");
        assert!(sat_from_tas_mach(400.0, 0.0).is_err());
        assert!(sat_from_tas_mach(f64::NAN, 0.8).is_err());
    }

    #[test]
    fn wind_triangle_cases() {
        // A head wind: track and heading east, 100 kt over the ground at 120 kt TAS: 20 kt from the east.
        let w = wind_triangle(90.0, 100.0, 90.0, 120.0).unwrap();
        assert!((w.from_deg - 90.0).abs() < 1e-9 && (w.speed_kt - 20.0).abs() < 1e-9);
        // A cross wind from the west: heading north at 100 kt, ground vector 50 kt east and 100 kt north.
        let gs = 50f64.hypot(100.0);
        let tr = 50f64.atan2(100.0).to_degrees();
        let w = wind_triangle(tr, gs, 0.0, 100.0).unwrap();
        assert!((w.from_deg - 270.0).abs() < 1e-9 && (w.speed_kt - 50.0).abs() < 1e-9);
        assert!(wind_triangle(0.0, -1.0, 0.0, 100.0).is_err());
        assert!(wind_triangle(f64::NAN, 1.0, 0.0, 100.0).is_err());
    }

    #[test]
    fn declination_near_published_values() {
        // Published IGRF-13 declinations for 2020 (NOAA calculator). The degree-3 cut keeps them within 6 degrees.
        for (lat, lon, d) in [
            (54.687, 25.28, 8.0),
            (37.77, -122.42, 13.0),
            (40.71, -74.0, -12.8),
            (51.5, -0.12, 1.0),
            (-33.95, 151.18, 12.8),
        ] {
            let got = declination_deg(lat, lon).unwrap();
            assert!((got - d).abs() < 6.0, "{lat} {lon}: {got}");
        }
        assert!(declination_deg(91.0, 0.0).is_err());
        assert!(declination_deg(0.0, f64::INFINITY).is_err());
    }
}

mod non_icao_rows {
    use sdrkit_proto_aero::aircraft::{AircraftTable, Key};
    use sdrkit_util_objects::Stamp;

    #[test]
    fn a_non_icao_address_has_its_own_row() {
        let mut t: AircraftTable = AircraftTable::new(10);
        t.heard_address(0x7C1234, true, Stamp::at(1.0))
            .unwrap()
            .messages += 1;
        t.heard_address(0x7C1234, false, Stamp::at(2.0))
            .unwrap()
            .messages += 2;
        assert_eq!(t.len(), 2);
        let other = t.get_address(0x7C1234, true).unwrap();
        assert_eq!(
            (other.icao, other.non_icao, other.messages),
            (None, Some(0x7C1234), 1)
        );
        assert_eq!(other.address_text().as_deref(), Some("~7C1234"));
        assert!(other.country.is_none() && other.registration.is_empty());
        let icao = t.get_address(0x7C1234, false).unwrap();
        assert_eq!((icao.icao, icao.messages), (Some(0x7C1234), 2));
        assert_eq!(Key::NonIcao(0x7C1234).to_string(), "~7C1234");
        assert_eq!(t.search("~7C").count(), 1);
        assert!(t.heard_address(0x100_0000, true, Stamp::at(3.0)).is_none());
        t.get_address_mut(0x7C1234, true).unwrap().flight = "TISB".into();
        assert_eq!(t.get(None, "").map(|a| a.messages), None);
    }
}

mod trails_pending_rows_and_the_log {
    use sdrkit_proto_aero::aircraft::{AircraftTable, Fix, Key, Source};
    use sdrkit_proto_aero::msglog::{MessageLog, Offer};
    use sdrkit_proto_aero::trail::{self, TrailMark};
    use sdrkit_proto_aero::Error;
    use sdrkit_util_geo::Position;
    use sdrkit_util_objects::{Shift, Stamp};

    fn fix(lat: f64, lon: f64, alt_ft: Option<f64>, on_ground: bool) -> Fix {
        Fix {
            position: Position::new(lat, lon),
            alt_ft,
            on_ground,
            source: Source::ModeS,
        }
    }

    #[test]
    fn a_trail_packs_and_unpacks_with_the_on_ground_bit() {
        let mut t: AircraftTable = AircraftTable::new(4);
        let a = t.heard(Some(0x7C1234), "", Stamp::at(0.0)).unwrap();
        a.position(0.0, fix(-33.9, 151.17, None, true), None)
            .unwrap();
        a.position(5.0, fix(-33.91, 151.18, Some(1200.0), false), None)
            .unwrap();
        // Far from the point before: a full point again.
        a.position(10.0, fix(10.0, -20.0, Some(37000.0), false), None)
            .unwrap();
        let mut bytes = Vec::new();
        trail::encode(a.trail.points(), &mut bytes);
        assert_eq!(bytes.len(), 10 + 5 + 10);
        let mut back = Vec::new();
        trail::decode(&bytes, &mut back).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(
            back[0].1,
            TrailMark {
                alt_ft: None,
                on_ground: true
            }
        );
        assert_eq!(back[1].1.alt_ft, Some(1200.0));
        assert!(!back[1].1.on_ground);
        assert!((back[1].0.lat_deg + 33.91).abs() < 1e-4);
        assert!((back[2].0.lon_deg + 20.0).abs() < 1e-4);
        assert_eq!(trail::decode(&bytes[..12], &mut back), Err(Error::Short));
        assert_eq!(trail::decode(&bytes[10..15], &mut back), Err(Error::Short));
        assert_eq!(trail::decode(&[], &mut back), Ok(()));
        assert!(back.is_empty());
    }

    #[test]
    fn a_pending_row_folds_into_the_address_and_rows_can_be_removed() {
        let mut t: AircraftTable = AircraftTable::new(4);
        let p = t.heard_pending(3, 0x12, Stamp::at(1.0)).unwrap();
        p.message("QFA1", "H1", "TEXT");
        let pending = Key::Pending {
            ground_station: 3,
            aircraft_id: 0x12,
        };
        assert_eq!(pending.to_string(), "@0312");
        assert!(t.rows().contains(&pending));
        let a = t
            .resolve_pending(3, 0x12, 0x7C1234, Stamp::at(2.0))
            .unwrap();
        assert_eq!((a.messages, a.flight.as_str()), (1, "QFA1"));
        assert!(!t.rows().contains(&pending));
        assert!(t.remove(&Key::Address(0x7C1234)).is_some());
        assert!(t.remove(&Key::Address(0x7C1234)).is_none());
        assert!(t.is_empty());
    }

    #[test]
    fn the_log_saves_and_moves_its_times() {
        let mut log = MessageLog::new();
        let o = Offer {
            t: 100.0,
            link: "HFDL",
            label: "H1",
            reg: "VH-OQA",
            text: "GATE B12",
            ..Offer::default()
        };
        assert!(log.offer(&o));
        let json = serde_json::to_string(&log).unwrap();
        let mut back: MessageLog = serde_json::from_str(&json).unwrap();
        back.shift_times(-50.0);
        let v = back.for_aircraft(60.0, None, "VHOQA", "", false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].t, 50.0);
    }
}

mod plain_text_formats {
    use sdrkit_proto_aero::acars::assemble::Quality;
    use sdrkit_proto_aero::acars::{self, Block, BlockSpec, End, Message, Reading};
    use sdrkit_proto_aero::formats::{self, Category, LABELS};

    type Ex = (
        &'static str,
        &'static str,
        Option<(f64, f64)>,
        Option<f64>,
        &'static str,
        &'static str,
        &'static str,
    );

    /// Label, text, latitude and longitude, altitude, origin, destination, flight ("" not stated).
    const EXAMPLES: [Ex; 39] = [
        ("12", "N 42.150,W121.187,39000,161859, 109,.C-GWSO,1742", Some((42.15, -121.187)), Some(39000.0), "", "", ""),
        ("16", "283806/AUTPOS/LLD N400547 W0774954\r\n/ALT 12932/SAT ****\r\n/WND ******/TAT ****/TAS ****/CRZ ***\r\n/FOB 065120\r\n/DAT 260228/TIM 150742", Some((40.096, -77.832)), Some(12932.0), "", "", "3806"),
        ("16", "(2AAABN39211W 77144KTEBMMTO-/A(Z", Some((39.352, -77.24)), None, "KTEB", "MMTO", ""),
        ("16", "N 44.203,W 86.546,31965,6, 290", Some((44.203, -86.546)), Some(31965.0), "", "", ""),
        ("16", "N 28.177/W 96.055", Some((28.177, -96.055)), None, "", "", ""),
        ("1J", "FTX/ID50007B,RCH4086,ABB02R70E037/MR6,/FX4 QTR PHILLY UP 37-6307A", None, None, "", "", "RCH4086"),
        ("1L", "000000070LOWW,KEWR,0932,1744,N 49.223,E 12.038,0659", Some((49.223, 12.038)), None, "LOWW", "KEWR", ""),
        ("1L", "00018213200/GS 411500/DEP MDPC/DES CYYZ/ETA 0120/GW 479/ALT 39002\r\nCAS 229/SAT - 59.0/FN SWG9040/TFQ 48/DAY 22OCT24/UTC 002714\r\nLON W 78.289/LAT N 39.556/WD  20/WS  13", Some((39.556, -78.289)), Some(39002.0), "MDPC", "CYYZ", "SWG9040"),
        ("1L", "000000660N50442E005566,100444359SOG-06 ,,--- 21-,83617441", Some((50.73667, 5.94333)), Some(35900.0), "", "", ""),
        ("1L", "+ 39.126/- 77.358/UTC 085208/FOB   8.2/ALT  3997/CAS  239/ETA 0903", Some((39.126, -77.358)), Some(3997.0), "", "", ""),
        ("1M", "/BA0843/ETA01/230822/LDSP/EGLL/EGSS/2JK0\n1940/EGLL27L/10", None, None, "LDSP", "EGLL", "BA0843"),
        ("20", "POSN38160W077075,,211733,360,OTT,212041,,N42,19689,40,544", Some((38.16, -77.075)), None, "", "", ""),
        ("22", "OFF01YX3661/25251712KIADKPWM171207  92", None, None, "KIAD", "KPWM", "YX3661"),
        ("22", "N 370824W 760010,-------,194936,30418, ,      , ,M 42,27335  42, 107,", Some((37.0824, -76.001)), Some(30418.0), "", "", ""),
        ("24", "/241710/1021/04WM/34962/N53.13/E001.33/3374/1056/", Some((53.13, 1.33)), Some(34962.0), "", "", "04WM"),
        ("2A", "SUM/ID53147T,RCH140,/DC03032026,213916/MR7,/SM031612,420,032139,031623,416,031631,413,032130,212,032139,209/VR329519", None, None, "", "", "RCH140"),
        ("2P", "M80AMC4086POS/ID50007B,RCH4086,ABB02R70E037/DC10022025,051804/MR103,/ET090738/PSN56012W013273,051804,350,,,,,084081,/CG,,/FB0857/VR0322B89", Some((56.02, -13.455)), Some(35000.0), "", "", "RCH4086"),
        ("30", "/EA1719/DSKSFO/SK23", None, None, "", "KSFO", ""),
        ("44", "00ETA03,N38241W081357,330,KBNA,KBWI,1107,0123,0208,008.1", Some((38.40167, -81.595)), Some(33000.0), "KBNA", "KBWI", ""),
        ("44", "IN01,N33528W084181,KCLT,KPDK,1106,0045,---.-", Some((33.88, -84.30167)), None, "KCLT", "KPDK", ""),
        ("44", "OFF02,N39247W077226,KFDK,KSNA,1106,2124,0248,011.1", Some((39.41167, -77.37667)), None, "KFDK", "KSNA", ""),
        ("44", "ON01,N33522W084181,KCLT,KPDK,1106,004023,---.-,", Some((33.87, -84.30167)), None, "KCLT", "KPDK", ""),
        ("44", " /FB 0160/AD KORH/N 38.655,W 75.325,JBU2834,INA03,KORH,2043", Some((38.655, -75.325)), None, "", "KORH", "JBU2834"),
        ("4J", "POS/ID91459S,BANKR31,/DC03032024,142813/MR64,0/ET31539/PSN39277W077359,142800,240,N39300W077110,031430,N38560W077150,M28,27619,MT370/CG311,160,350/FB732/VR329071", Some((39.462, -77.598)), Some(24000.0), "", "", "BANKR31"),
        ("58", "OG0704/06/230942/N39.214/W76.106/22683/N/", Some((39.214, -76.106)), Some(22683.0), "", "", "OG0704"),
        ("5Z", "/B3 TO DATA REQ    / KIAH KBOS 14 152532 R4R /---- BOPT/OFF C0.000/1 LNO  G1600", None, None, "KIAH", "KBOS", ""),
        ("80", "3N01 POSRPT 5891/04 KIAH/MMGL .XA-VOI\r\n/POS N29395W095133/ALT +15608/MCH 558/FOB 0100/ETA 0410", Some((29.395, -95.133)), Some(15608.0), "KIAH", "MMGL", "5891"),
        ("83", "KLAX,KEWR,220103, 40.53,- 74.47, 3836,212, 140.0, 19700", Some((40.53, -74.47)), Some(3836.0), "KLAX", "KEWR", ""),
        ("8E", "EGSS,1618", None, None, "", "EGSS", ""),
        ("H1", "76401\r\n02E23KDJTKBWI\r\nN35551W07767100084099M050276030G1801230030YY,\r\n", None, None, "KDJT", "KBWI", ""),
        ("H1", "FPN/FNAAL1956/RP:DA:KPHL:AA:KPHX:CR:PHLPHX61:R:27L(26O):D:PHL3:A:EAGUL6.ZUN:AP:ILS26..AIR,N40010W080490.J110.BOWRR..VLA,...", None, None, "", "", "AAL1956"),
        ("H1", "INI/ID70045B,RCH2050,AJM363201271/MR2,000/AFKDOV,KBHM/TD271115,131545EE", None, None, "KDOV", "KBHM", "RCH2050"),
        ("H1", "M85AQF0073YSSY,KSFO,101621,- 4.9985,-169.9820,35003,290,  35.1, 44100,S05W169,S02W167,1645", Some((-4.9985, -169.982)), Some(35003.0), "YSSY", "KSFO", "QF0073"),
        ("H1", "POSN43312W123174,EASON,215754,370,EBINY,220601,ELENN,M48,02216,185/TS215754,0921227A40", Some((43.52, -123.29)), Some(37000.0), "", "", ""),
        ("H1", "(POS-KLM296  -3911N07600W/234212 F250\r\nRMK/FUEL  37.0 M0.69)", Some((39.183, -76.0)), Some(25000.0), "", "", "KLM296"),
        ("H1", "/HDQDLUA.POSN38332W080082,RONZZ,135753,320,LEVII,140454,WISTA,M45,20967,194/GAHDQDLUA/CA/TS135753,1411240721", Some((38.553, -80.137)), Some(32000.0), "", "", ""),
        ("H1", "*POS10300950N3954W07759363312045802M5230175", Some((39.9, -77.98333)), Some(36331.0), "", "", ""),
        ("HX", "RA FMT LOCATION N4009.6 W07540.8", Some((40.16, -75.68)), None, "", "", ""),
        ("QQ", "KSDLKLAS0025\r\n001FE09002543N3336.6W11155.90281750042", Some((33.61, -111.93167)), None, "KSDL", "KLAS", ""),
    ];

    fn matches(e: &Ex) -> Result<(), String> {
        let (label, text, pos, alt, o, d, fl) = *e;
        let r = if label == "QQ" {
            formats::read_qq(text)
        } else {
            formats::read(label, text)
        };
        let r = r.ok_or("no reading")?;
        if let Some((la, lo)) = pos {
            let p = r.position.ok_or("no position")?;
            if (p.lat_deg - la).abs() > 0.002 || (p.lon_deg - lo).abs() > 0.002 {
                return Err(format!("position {p:?}"));
            }
        }
        if let Some(a) = alt {
            if r.alt_ft.is_none_or(|x| (x - a).abs() > 1.0) {
                return Err(format!("altitude {:?}", r.alt_ft));
            }
        }
        for (want, have, what) in [
            (o, &r.origin, "origin"),
            (d, &r.destination, "destination"),
            (fl, &r.flight, "flight"),
        ] {
            if !want.is_empty() && have.as_deref() != Some(want) {
                return Err(format!("{what} {have:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn the_published_examples_read() {
        let mut misses = Vec::new();
        for e in &EXAMPLES {
            if let Err(m) = matches(e) {
                misses.push(format!("{} {:?}: {m}", e.0, e.1));
            }
        }
        // At least 85 % of the examples, within 0.002 degree and 1 ft.
        assert!(misses.len() * 100 <= EXAMPLES.len() * 15, "{misses:#?}");
        assert!(misses.is_empty(), "{misses:#?}");
    }

    #[test]
    fn one_line_form_and_refusals() {
        let r = formats::read(
            "44",
            "00ETA03,N38241W081357,330,KBNA,KBWI,1107,0123,0208,008.1",
        )
        .unwrap();
        assert!(r.line().starts_with("ETA report"));
        assert!(r.line().contains("KBNA"));
        assert_eq!(r.category, Category::Eta);
        assert!(formats::read("44", "GOOD MORNING CAPTAIN").is_none());
        assert!(formats::read(
            "44",
            "00ETA03,N99999W999999,330,KBNA,KBWI,1107,0123,0208,008.1"
        )
        .is_none());
    }

    #[test]
    fn a_downlink_is_read_with_its_prefix() {
        let mut body = Vec::new();
        BlockSpec::new('2', "VH-QPA", b'1', End::Etx)
            .label("H1")
            .text("M85AQF0073YSSY,KSFO,101621,- 4.9985,-169.9820,35003,290,  35.1, 44100,S05W169,S02W167,1645")
            .body(&mut body)
            .unwrap();
        acars::strip_parity(&mut body);
        let m = Message::from_block(&Block::parse(&body).unwrap(), Quality::GOOD);
        let Reading::Format(f) = &m.reading else {
            panic!("{:?}", m.reading)
        };
        assert_eq!(
            (f.flight.as_deref(), f.origin.as_deref()),
            (Some("QF0073"), Some("YSSY"))
        );
        let (p, alt) = m.position().unwrap();
        assert!((p.lat_deg + 4.9985).abs() < 1e-6 && alt == Some(35003.0));
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn check(label: &str, t: &str) {
        let r = if label == "QQ" {
            formats::read_qq(t)
        } else {
            formats::read(label, t)
        };
        if let Some(p) = r.and_then(|r| r.position) {
            assert!(
                p.lat_deg.abs() <= 90.0 && p.lon_deg.abs() <= 180.0,
                "{label} {t:?}"
            );
        }
    }

    #[test]
    fn cut_changed_and_random_texts_never_fail() {
        let mut r = Rng(0x9E37_79B9_7F4A_7C15);
        for e in &EXAMPLES {
            let b = e.1.as_bytes();
            for _ in 0..40 {
                let mut v = b[..(r.next() as usize) % (b.len() + 1)].to_vec();
                for _ in 0..(r.next() % 4) {
                    if !v.is_empty() {
                        let i = (r.next() as usize) % v.len();
                        v[i] = b' ' + (r.next() % 95) as u8;
                    }
                }
                check(e.0, &String::from_utf8_lossy(&v));
            }
        }
        let labels = LABELS;
        let mut t = String::new();
        for _ in 0..300_000 {
            t.clear();
            for _ in 0..(r.next() % 81) {
                let k = r.next();
                t.push(match k % 8 {
                    0 => ',',
                    1 => '/',
                    2 => char::from(b'0' + (k >> 8) as u8 % 10),
                    3 => ['N', 'S', 'E', 'W', ' ', '.', '\n', '#'][(k >> 8) as usize % 8],
                    _ => char::from(b' ' + ((k >> 8) % 95) as u8),
                });
            }
            check(labels[(r.next() as usize) % labels.len()], &t);
        }
    }
}

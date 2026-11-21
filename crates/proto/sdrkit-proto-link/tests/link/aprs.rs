//! Known answers from APRS 1.0.1, the APRS101 document (the chapter of each example is named) and the 1.1 and 1.2 addenda.

use sdrkit_proto_link::aprs::{
    self, Body, Content, Extension, Located, MicEMessage, NmeaCheck, Packet, Symbol, TelemetryMeta,
    Timestamp,
};
use sdrkit_proto_link::dprs;

fn p(s: &str) -> Packet {
    aprs::parse(s.as_bytes(), "APRS").unwrap()
}

fn pos(s: &str) -> Located {
    match p(s).body {
        Body::Position { report, .. } => report,
        b => panic!("{s}: {b:?}"),
    }
}

fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} against {b}");
}

#[test]
fn chapter_8_uncompressed_positions() {
    let r = pos("!4903.50N/07201.75W-Test 001234");
    near(r.position.lat_deg, 49.0 + 3.5 / 60.0, 1e-9);
    near(r.position.lon_deg, -(72.0 + 1.75 / 60.0), 1e-9);
    assert_eq!(
        r.symbol,
        Symbol {
            table: '/',
            code: '-'
        }
    );
    assert_eq!((r.comment.as_str(), r.ambiguity), ("Test 001234", 0));
    // Altitude in the comment.
    let r = pos("!4903.50N/07201.75W-Test /A=001234");
    near(r.altitude_m.unwrap(), 1234.0 * 0.3048, 1e-9);
    assert_eq!(r.comment, "Test ");
    // Time stamps: DHM zulu, DHM local, HMS.
    let q = p("/092345z4903.50N/07201.75W>Test1234");
    let Body::Position {
        timestamp,
        messaging,
        ..
    } = q.body
    else {
        panic!()
    };
    assert_eq!(
        timestamp,
        Some(Timestamp::DayHourMinute {
            day: 9,
            hour: 23,
            minute: 45,
            utc: true
        })
    );
    assert!(!messaging);
    let q = p("@092345/4903.50N/07201.75W>Test1234");
    let Body::Position {
        timestamp,
        messaging,
        ..
    } = q.body
    else {
        panic!()
    };
    assert_eq!(
        timestamp,
        Some(Timestamp::DayHourMinute {
            day: 9,
            hour: 23,
            minute: 45,
            utc: false
        })
    );
    assert!(messaging);
    let q = p("/234517h4903.50N/07201.75W>");
    let Body::Position { timestamp, .. } = q.body else {
        panic!()
    };
    assert_eq!(
        timestamp,
        Some(Timestamp::HourMinuteSecond {
            hour: 23,
            minute: 45,
            second: 17
        })
    );
    assert!(aprs::parse(b"/999999z4903.50N/07201.75W>", "APRS").is_err());
}

#[test]
fn chapter_8_position_ambiguity() {
    // Each blank widens the box: 0.1', 1', 10', 1 degree. The position is the box centre.
    let cases = [
        ("!4903.5 N/07201.75W-", 1, 49.0 + 3.55 / 60.0, 0.1),
        ("!4903.  N/07201.75W-", 2, 49.0 + 3.5 / 60.0, 1.0),
        ("!490 .  N/07201.75W-", 3, 49.0 + 5.0 / 60.0, 10.0),
        ("!49  .  N/07201.75W-", 4, 49.5, 60.0),
    ];
    for (s, amb, lat, box_min) in cases {
        let r = pos(s);
        assert_eq!(r.ambiguity, amb, "{s}");
        near(r.position.lat_deg, lat, 1e-9);
        near(r.resolution_deg, box_min / 60.0, 1e-12);
    }
    // The longitude takes the latitude's box: 72 deg 01.75' in a 1-degree box is 72.5.
    near(pos("!49  .  N/07201.75W-").position.lon_deg, -72.5, 1e-9);
    // A blank left of a digit is not a position.
    assert_eq!(p("!49 3.50N/07201.75W-").body.kind(), "Other");
}

#[test]
fn positions_encode_as_the_chapters_show() {
    let p = sdrkit_util_geo::Position {
        lat_deg: 49.0 + 3.5 / 60.0,
        lon_deg: -(72.0 + 1.75 / 60.0),
        alt_m: 0.0,
    };
    let mut s = String::new();
    aprs::encode_uncompressed(
        &p,
        Symbol {
            table: '/',
            code: '-',
        },
        0,
        &mut s,
    )
    .unwrap();
    assert_eq!(s, "4903.50N/07201.75W-");
    let mut s = String::new();
    aprs::encode_uncompressed(
        &p,
        Symbol {
            table: '/',
            code: '-',
        },
        2,
        &mut s,
    )
    .unwrap();
    assert_eq!(s, "4903.  N/07201.  W-");
    // The rounding carries: 10.999999 N is 11 00.00.
    let q = sdrkit_util_geo::Position {
        lat_deg: 10.999_999,
        lon_deg: -1.0,
        alt_m: 0.0,
    };
    let mut s = String::new();
    aprs::encode_uncompressed(
        &q,
        Symbol {
            table: '/',
            code: '-',
        },
        0,
        &mut s,
    )
    .unwrap();
    assert_eq!(s, "1100.00N/00100.00W-");
    // Chapter 9: 49.5 N, 72.75 W, course 88, speed 36.2 knots, T = "[".
    let c = sdrkit_util_geo::Position {
        lat_deg: 49.5,
        lon_deg: -72.75,
        alt_m: 0.0,
    };
    let mut s = String::new();
    aprs::encode_compressed(
        &c,
        Symbol {
            table: '/',
            code: '>',
        },
        Some((88, 36.2)),
        b'[' - 33,
        &mut s,
    )
    .unwrap();
    assert_eq!(s, "/5L!!<*e7>7P[");
    assert!(aprs::encode_uncompressed(&p, Symbol::default(), 5, &mut String::new()).is_err());
    assert!(aprs::encode_compressed(&p, Symbol::default(), None, 64, &mut String::new()).is_err());
}

#[test]
fn chapter_9_compressed_positions() {
    // "/5L!!<*e7>7P[": 49.5 N, 72.75 W, course 88, speed 36.2 knots.
    // The spec rounds: the base-91 grid is 1/380926 degree of latitude and 1/190463 of longitude.
    let r = pos("=/5L!!<*e7>7P[");
    near(r.position.lat_deg, 49.5, 1e-5);
    near(r.position.lon_deg, -72.75, 1e-5);
    assert!(r.compressed);
    assert_eq!(r.course_deg, Some(88));
    near(r.speed_kn.unwrap(), 36.2, 0.05);
    // Altitude: cs "S]" with T "S" (NMEA source GGA): 1.002^4610 = 10004 feet.
    let r = pos("!/5L!!<*e7OS]S");
    near(r.altitude_m.unwrap() / 0.3048, 10004.0, 1.0);
    // Range: "{?" is 2 x 1.08^30 = 20.12 miles.
    let r = pos("!/5L!!<*e7>{?!");
    match r.extension {
        Some(Extension::Range { miles }) => near(miles, 20.12, 0.01),
        e => panic!("{e:?}"),
    }
    // An overlay digit is sent as a to j.
    assert_eq!(pos("!c5L!!<*e7>   ").symbol.table, '2');
    // No course, speed or range when c is a blank.
    let r = pos("!/5L!!<*e7>   comment");
    assert_eq!(
        (r.course_deg, r.altitude_m, r.comment.as_str()),
        (None, None, "comment")
    );
}

#[test]
fn chapter_7_data_extensions() {
    // PHG5132: 25 W, 20 ft, 3 dB, 90 degrees.
    match pos("=4903.50N/07201.75W#PHG5132").extension {
        Some(Extension::Phg(s)) => {
            assert_eq!(
                (s.power, s.height_ft, s.gain_db, s.directivity_deg),
                (25.0, 20.0, 3, 90)
            );
        }
        e => panic!("{e:?}"),
    }
    match pos("!4903.50N/07201.75W-RNG0050").extension {
        Some(Extension::Range { miles }) => assert_eq!(miles, 50.0),
        e => panic!("{e:?}"),
    }
    // DFS2360: S2, 80 ft, 6 dB, omni.
    match pos("!4903.50N/07201.75W\\DFS2360").extension {
        Some(Extension::Dfs(s)) => {
            assert_eq!(
                (s.power, s.height_ft, s.gain_db, s.directivity_deg),
                (2.0, 80.0, 6, 0)
            );
        }
        e => panic!("{e:?}"),
    }
    // Course and speed, then a DF bearing and NRQ.
    let r = pos("!4903.50N/07201.75W\\088/036/270/729");
    assert_eq!((r.course_deg, r.speed_kn), (Some(88), Some(36.0)));
    let b = r.bearing.unwrap();
    assert_eq!((b.bearing_deg, b.nrq), (270, [7, 2, 9]));
    // Area object shape.
    assert!(matches!(
        pos("!4903.50N/07201.75W\\T15/C04 text").extension,
        Some(Extension::Area {
            kind: 15,
            colour: 4
        })
    ));
}

#[test]
fn aprs_1_2_dao_and_base91_comment_telemetry() {
    // !W52!: one more digit of the minutes, 5 for the latitude and 2 for the longitude.
    let r = pos("!4903.50N/07201.75W-Test!W52!");
    near(r.position.lat_deg, 49.0 + 3.505 / 60.0, 1e-9);
    near(r.position.lon_deg, -(72.0 + 1.752 / 60.0), 1e-9);
    assert_eq!((r.datum, r.comment.as_str()), (Some('W'), "Test"));
    // !w!!! base-91: zero extra.
    let r = pos("!4903.50N/07201.75W-!w\"\"!");
    near(r.position.lat_deg, 49.0 + (3.5 + 0.0001) / 60.0, 1e-9);
    // |ss1122| : sequence 1, analog 185.
    let r = pos("!4903.50N/07201.75W-hi |!\"#$|");
    let t = r.telemetry.unwrap();
    assert_eq!(
        (t.sequence.as_str(), t.analog[0], t.analog[1]),
        ("1", Some(185.0), None)
    );
    assert_eq!(r.comment, "hi ");
}

#[test]
fn chapter_10_mic_e() {
    // Destination S32U6T (the chapter's example): 33 25.64 N, West, no longitude offset, message M3 "Returning".
    let info = [
        b'`',
        72 + 28,
        4 + 28,
        31 + 28,
        8 + 28,
        61 + 28,
        23 + 28,
        b'>',
        b'/',
    ];
    let m = match aprs::parse(&info, "S32U6T").unwrap().body {
        Body::MicE(m) => m,
        b => panic!("{b:?}"),
    };
    near(m.position.lat_deg, 33.0 + 25.64 / 60.0, 1e-9);
    near(m.position.lon_deg, -(72.0 + 4.31 / 60.0), 1e-9);
    assert_eq!((m.speed_kn, m.course_deg), (86.0, 123));
    assert_eq!(m.message, MicEMessage::Standard(3));
    assert_eq!(m.message.name(), "Returning");
    assert_eq!(
        m.symbol,
        Symbol {
            table: '/',
            code: '>'
        }
    );
    // Longitude offset +100: 112 07.74 is d = 12 + 28, m = 7 + 28, h = 74 + 28. 5 degrees is d = 5 + 90 + 28 (190 to 199).
    let info = [b'`', 12 + 28, 7 + 28, 74 + 28, 28, 28, 28, b'k', b'/'];
    let m = match aprs::parse(&info, "S32UPT").unwrap().body {
        Body::MicE(m) => m,
        b => panic!("{b:?}"),
    };
    near(m.position.lon_deg, -(112.0 + 7.74 / 60.0), 1e-9);
    let info = [b'`', 5 + 90 + 28, 60 + 28, 28, 28, 28, 28, b'k', b'/'];
    let m = match aprs::parse(&info, "S32UPT").unwrap().body {
        Body::MicE(m) => m,
        b => panic!("{b:?}"),
    };
    near(m.position.lon_deg, -5.0, 1e-9);
    // Message codes: all standard bits 1 is M0, custom A-K letters give C codes, all 0 is emergency, a mix is unknown.
    let base = [b'`', 28, 28, 28, 28, 28, 28, b'k', b'/'];
    let code = |d: &str| match aprs::parse(&base, d).unwrap().body {
        Body::MicE(m) => m.message,
        _ => unreachable!(),
    };
    assert_eq!(code("PPP000"), MicEMessage::Standard(0));
    assert_eq!(code("AAA000"), MicEMessage::Custom(0));
    assert_eq!(code("000000"), MicEMessage::Emergency);
    assert_eq!(code("PA0000"), MicEMessage::Unknown);
    // Ambiguity: L in place of the hundredths.
    let m = match aprs::parse(&base, "S32U6L").unwrap().body {
        Body::MicE(m) => m,
        _ => unreachable!(),
    };
    assert_eq!(m.ambiguity, 1);
    // Status text: Kenwood type code, altitude "xxx}" (1000 m), comment, and five-channel hex telemetry.
    let mut info = base.to_vec();
    info.extend_from_slice(b"]\">q}Hello=");
    let m = match aprs::parse(&info, "S32U6T").unwrap().body {
        Body::MicE(m) => m,
        _ => unreachable!(),
    };
    assert_eq!(
        (m.device.as_deref(), m.altitude_m, m.comment.as_str()),
        (Some("Kenwood TM-D710"), Some(1000.0), "Hello")
    );
    let mut info = base.to_vec();
    info.extend_from_slice(b"'0102030405 x");
    let Body::MicE(m) = aprs::parse(&info, "S32U6T").unwrap().body else {
        unreachable!()
    };
    assert_eq!(
        m.telemetry,
        Some([Some(1), Some(2), Some(3), Some(4), Some(5)])
    );
    let mut info = base.to_vec();
    info.extend_from_slice(b"`0A0B");
    let Body::MicE(m) = aprs::parse(&info, "S32U6T").unwrap().body else {
        unreachable!()
    };
    assert_eq!(m.telemetry, Some([Some(10), None, Some(11), None, None]));
    let mut info = base.to_vec();
    info.extend_from_slice(b"`Comment_%");
    let Body::MicE(m) = aprs::parse(&info, "S32U6T").unwrap().body else {
        unreachable!()
    };
    assert_eq!(
        (m.device.as_deref(), m.comment.as_str()),
        (Some("Yaesu FTM-400DR"), "Comment")
    );
    // Bad destinations and short fields are errors.
    assert!(aprs::parse(&base, "S32U6").is_err());
    assert!(aprs::parse(&base, "S3!U6T").is_err());
    assert!(aprs::parse(&base[..8], "S32U6T").is_err());
}

#[test]
fn chapter_11_objects_and_items() {
    let q = p(";LEADER   *092345z4903.50N/07201.75W>088/036");
    let Body::Object {
        name,
        live,
        timestamp,
        report,
    } = q.body
    else {
        panic!()
    };
    assert_eq!((name.as_str(), live), ("LEADER", true));
    assert!(timestamp.is_some());
    assert_eq!((report.course_deg, report.speed_kn), (Some(88), Some(36.0)));
    let Body::Object { live, .. } = p(";LEADER   _092345z4903.50N/07201.75W>088/036").body else {
        panic!()
    };
    assert!(!live);
    let Body::Item { name, live, report } = p(")AID #2!4903.50N/07201.75WA").body else {
        panic!()
    };
    assert_eq!(
        (name.as_str(), live, report.symbol.code),
        ("AID #2", true, 'A')
    );
    let Body::Item { live, .. } = p(")MOBIL_/5L!!<*e7>7P[").body else {
        panic!()
    };
    assert!(!live);
    assert!(aprs::parse(b";SHORT", "APRS").is_err());
    assert!(aprs::parse(b")AB!4903.50N/07201.75WA", "APRS").is_err());
}

#[test]
fn chapter_12_weather() {
    let q = p("_10090556c220s004g005t077r000p000P000h50b09900wRSW");
    let Body::Weather {
        timestamp,
        weather: w,
    } = q.body
    else {
        panic!()
    };
    assert_eq!(
        timestamp,
        Some(Timestamp::MonthDayHourMinute {
            month: 10,
            day: 9,
            hour: 5,
            minute: 56
        })
    );
    assert_eq!(
        (w.wind_dir_deg, w.wind_mph, w.gust_mph, w.temp_f),
        (Some(220), Some(4.0), Some(5.0), Some(77.0))
    );
    assert_eq!(
        (w.rain_1h_in, w.humidity_pct, w.pressure_hpa),
        (Some(0.0), Some(50), Some(990.0))
    );
    assert_eq!(w.software, "wRSW");
    let r = pos("!4903.50N/07201.75W_220/004g005t-07r001p002P003h00b10132L456");
    let w = r.weather.unwrap();
    assert_eq!(
        (w.wind_dir_deg, w.wind_mph, w.temp_f, w.humidity_pct),
        (Some(220), Some(4.0), Some(-7.0), Some(100))
    );
    assert_eq!(
        (w.rain_1h_in, w.rain_24h_in, w.rain_midnight_in),
        (Some(0.01), Some(0.02), Some(0.03))
    );
    assert_eq!(
        (w.pressure_hpa, w.luminosity_w_m2),
        (Some(1013.2), Some(456))
    );
    assert_eq!(
        p("!4903.50N/07201.75W_220/004g...t077").body.kind(),
        "Weather"
    );
    let w = pos("!4903.50N/07201.75W_.../...g...t077s012l010")
        .weather
        .unwrap();
    assert_eq!(
        (w.wind_dir_deg, w.gust_mph, w.snow_in, w.luminosity_w_m2),
        (None, None, Some(12.0), Some(1010))
    );
}

#[test]
fn chapter_13_telemetry() {
    let Body::Telemetry(t) = p("T#005,199,000,255,073,123,01101001").body else {
        panic!()
    };
    assert_eq!(t.sequence, "005");
    assert_eq!(
        t.analog,
        [Some(199.0), Some(0.0), Some(255.0), Some(73.0), Some(123.0)]
    );
    assert_eq!(t.digital, Some(0b0110_1001));
    let Body::Telemetry(t) = p("T#MIC,1.5,-2.25").body else {
        panic!()
    };
    assert_eq!(
        (t.sequence.as_str(), t.analog[1], t.digital),
        ("MIC", Some(-2.25), None)
    );
    let meta = |s: &str| match p(s).body {
        Body::Message(m) => match m.content {
            Content::Telemetry(t) => t,
            c => panic!("{c:?}"),
        },
        b => panic!("{b:?}"),
    };
    let TelemetryMeta::Parameters(n) =
        meta(":N0QBF-11 :PARM.Battery,Btemp,ATemp,Pres,Alt,Camra,Chute,Sun,10m,ATV")
    else {
        panic!()
    };
    assert_eq!(
        (n.len(), n[0].as_str(), n[9].as_str()),
        (10, "Battery", "ATV")
    );
    let TelemetryMeta::Units(u) =
        meta(":N0QBF-11 :UNIT.Volts,deg.F,deg.F,Mbar,Kft,Click,OPEN!,on,on,hi")
    else {
        panic!()
    };
    assert_eq!(u[1], "deg.F");
    let TelemetryMeta::Equations(e) =
        meta(":N0QBF-11 :EQNS.0,5.2,0,0,.53,-32,3,4.39,49,-32,3,18,1,2,3")
    else {
        panic!()
    };
    assert_eq!(e[0], [0.0, 5.2, 0.0]);
    assert_eq!(e[1], [0.0, 0.53, -32.0]);
    assert_eq!(e[4], [1.0, 2.0, 3.0]);
    let TelemetryMeta::Bits { sense, title } = meta(":N0QBF-11 :BITS.10110000,N0QBF's Big Balloon")
    else {
        panic!()
    };
    assert_eq!(
        (sense, title.as_str()),
        (0b1011_0000, "N0QBF's Big Balloon")
    );
}

#[test]
fn chapter_14_messages() {
    let m = |s: &str| match p(s).body {
        Body::Message(m) => m,
        b => panic!("{b:?}"),
    };
    let x = m(":WU2Z     :Testing{003");
    assert_eq!(x.addressee, "WU2Z");
    assert_eq!(
        x.content,
        Content::Text {
            text: "Testing".into(),
            id: Some("003".into()),
            reply_ack: None
        }
    );
    assert_eq!(
        m(":KB2ICI-14:ack003").content,
        Content::Ack {
            id: "003".into(),
            reply_ack: None
        }
    );
    assert_eq!(
        m(":KB2ICI-14:rej003").content,
        Content::Rej { id: "003".into() }
    );
    assert_eq!(
        m(":BLN3     :Snow expected in Tampa RSN").content,
        Content::Bulletin {
            id: '3',
            group: String::new(),
            text: "Snow expected in Tampa RSN".into()
        }
    );
    assert_eq!(
        m(":BLNQ     :Mt St Helen digi will be QRT this weekend").content,
        Content::Announcement {
            id: 'Q',
            text: "Mt St Helen digi will be QRT this weekend".into()
        }
    );
    assert!(
        matches!(m(":BLN4WX   :group").content, Content::Bulletin { id: '4', ref group, .. } if group == "WX")
    );
    assert!(matches!(
        m(":NWS-WARN :Tornado").content,
        Content::Weather { .. }
    ));
    assert_eq!(
        m(":KH2Z     :?APRSP").content,
        Content::Query {
            query: "APRSP".into()
        }
    );
    // APRS 1.1 reply-ack.
    assert_eq!(
        m(":N0CALL   :Hello{MM}AA").content,
        Content::Text {
            text: "Hello".into(),
            id: Some("MM".into()),
            reply_ack: Some("AA".into())
        }
    );
    assert_eq!(
        m(":N0CALL   :ackMM}AA").content,
        Content::Ack {
            id: "MM".into(),
            reply_ack: Some("AA".into())
        }
    );
    // "ack" or "rej" followed by something that is not a message number is text.
    assert!(matches!(
        m(":N0CALL   :acknowledged").content,
        Content::Text { .. }
    ));
    assert!(matches!(
        m(":N0CALL   :rej this").content,
        Content::Text { .. }
    ));
    assert!(aprs::parse(b":SHORT:x", "APRS").is_err());
    // Encoding.
    let mut s = String::new();
    aprs::message::encode_text("WU2Z", "Testing", Some("003"), &mut s).unwrap();
    assert_eq!(s, ":WU2Z     :Testing{003");
    let mut s = String::new();
    aprs::message::encode_ack("KB2ICI-14", "003", &mut s).unwrap();
    assert_eq!(s, ":KB2ICI-14:ack003");
    assert!(aprs::message::encode_text("TOOLONGNAME", "x", None, &mut String::new()).is_err());
    assert!(aprs::message::encode_text("A", "x{y", None, &mut String::new()).is_err());
}

#[test]
fn chapters_15_to_18_and_other_types() {
    let Body::Status {
        timestamp,
        locator,
        text,
    } = p(">Net Control Center").body
    else {
        panic!()
    };
    assert_eq!(
        (timestamp, locator, text.as_str()),
        (None, None, "Net Control Center")
    );
    let Body::Status {
        timestamp, text, ..
    } = p(">092345zNet Control Center").body
    else {
        panic!()
    };
    assert!(timestamp.is_some());
    assert_eq!(text, "Net Control Center");
    let Body::Status { locator, text, .. } = p(">IO91SX/G My house").body else {
        panic!()
    };
    assert_eq!(
        locator,
        Some((
            "IO91SX".into(),
            Symbol {
                table: '/',
                code: 'G'
            }
        ))
    );
    assert_eq!(text, "My house");
    let Body::Status { locator, .. } = p(">IO91/G").body else {
        panic!()
    };
    assert_eq!(locator.unwrap().0, "IO91");
    let Body::Capabilities(c) = p("<IGATE,MSG_CNT=43,LOC_CNT=14").body else {
        panic!()
    };
    assert_eq!(c[0], ("IGATE".into(), None));
    assert_eq!(c[1], ("MSG_CNT".into(), Some("43".into())));
    let Body::Query { query, target } = p("?APRS? 34.02,-117.15,0200").body else {
        panic!()
    };
    assert_eq!(
        (query.as_str(), target.as_str()),
        ("APRS", "34.02,-117.15,0200")
    );
    let Body::ThirdParty { header, packet } =
        p("}WB2OSZ-5>APDW17,TCPIP,WB2OSZ*:!4903.50N/07201.75W-").body
    else {
        panic!()
    };
    assert_eq!(header, "WB2OSZ-5>APDW17,TCPIP,WB2OSZ*");
    assert_eq!(packet.body.kind(), "Position");
    assert!(p("}WB2OSZ-5>APDW17:!4903.50N/07201.75W-")
        .body
        .position()
        .is_some());
    let Body::UserDefined { user, kind, data } = p("{Q1qwerty").body else {
        panic!()
    };
    assert_eq!((user, kind, data.as_str()), ('Q', '1', "qwerty"));
    let Body::Maidenhead { locator, comment } = p("[IO91SX] 35 miles NNW of London").body else {
        panic!()
    };
    assert_eq!(
        (locator.as_str(), comment.as_str()),
        ("IO91SX", "35 miles NNW of London")
    );
    // A "!" position after other text (chapter 5).
    let r = pos("TheNet X1J4 (BFLD)!4903.50N/07201.75Wn");
    assert_eq!(r.comment, "TheNet X1J4 (BFLD)");
    assert_eq!(p("hello world").body, Body::Other("hello world".into()));
    assert!(aprs::parse(b"", "APRS").is_err());
}

#[test]
fn raw_nmea_has_its_checksum_checked() {
    // The *hh checksum of raw NMEA is checked. A good GGA, a good RMC, a damaged one, and one with no checksum.
    let Body::RawGps {
        check,
        position,
        altitude_m,
        ..
    } = p("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47").body
    else {
        panic!()
    };
    assert_eq!(check, NmeaCheck::Good);
    near(position.unwrap().lat_deg, 48.1173, 1e-9);
    assert_eq!(altitude_m, Some(545.4));
    let Body::RawGps {
        check,
        speed_kn,
        course_deg,
        ..
    } = p("$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A").body
    else {
        panic!()
    };
    assert_eq!(
        (check, speed_kn, course_deg),
        (NmeaCheck::Good, Some(22.4), Some(84.4))
    );
    let Body::RawGps {
        check, position, ..
    } = p("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*48").body
    else {
        panic!()
    };
    assert_eq!((check, position), (NmeaCheck::Bad, None));
    let Body::RawGps {
        check, position, ..
    } = p("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,").body
    else {
        panic!()
    };
    assert_eq!(check, NmeaCheck::Missing);
    assert!(position.is_some());
}

#[test]
fn frames_and_tnc2_lines_parse_and_summaries_print() {
    let f = sdrkit_proto_link::ax25::Frame::from_tnc2(
        "N0CALL-9>S32U6T,WIDE1-1:`\x64\x20\x3b\x24\x59\x33>/",
    )
    .unwrap();
    assert_eq!(aprs::parse_frame(&f).unwrap().body.kind(), "Mic-E");
    let q = aprs::parse_tnc2("N0CALL>APRS-1,TCPIP*,qAC,T2:!4903.50N/07201.75W-Test").unwrap();
    assert_eq!(q.to_string(), "49.0583 -72.0292 [/-] Test");
    assert!(aprs::parse_tnc2("garbage").is_err());
}

#[test]
fn bad_fields_give_errors_not_panics() {
    let mut seed = 99u32;
    let alphabet = b"!=/@`';)>:_T#$}{[<?0123456789.NSEW/\\ zh|{}ABCDKLPZ";
    let mut buf = [0u8; 60];
    for n in 0..20_000 {
        for b in buf.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = if seed >> 31 == 0 {
                alphabet[(seed >> 8) as usize % alphabet.len()]
            } else {
                (seed >> 16) as u8
            };
        }
        let len = n % 60;
        let _ = aprs::parse(&buf[..len], "S32U6T");
        let _ = aprs::parse(&buf[..len], "APRS");
    }
}

#[test]
fn d_prs_lines_carry_a_crc() {
    let mut line = String::new();
    dprs::encode("JA1ABC-A>API51,DSTAR*:!3541.50N/13945.00E>test", &mut line);
    assert!(line.starts_with("$$CRC") && line.ends_with('\r'));
    let d = dprs::parse(&line).unwrap();
    assert!(d.crc_ok);
    assert!(aprs::parse_tnc2(d.tnc2).unwrap().body.position().is_some());
    // Without the closing carriage return the CRC still holds. A changed byte fails it.
    assert!(dprs::parse(line.trim_end()).unwrap().crc_ok);
    let bad = line.replace("test", "tesT");
    assert!(!dprs::parse(&bad).unwrap().crc_ok);
    assert!(dprs::parse("$$CRCZZZZ,x").is_err());
    assert!(dprs::parse("hello").is_err());
}

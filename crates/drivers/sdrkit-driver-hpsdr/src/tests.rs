//! Tests of the rules without a network, and of protocol 1 against a fake radio on a fake network.

use crate::fake1::{Fake1, Pace};
use crate::plan::{self, Gain};
use crate::profile::{self, Model, Profile};
use crate::{
    descriptor, p1, p2, parse_address, HpsdrDriver, HpsdrError, HpsdrKindCommand, HpsdrSetting,
    Timing, ADDRESS, BUSY, FREQUENCY_SENT, HERMES_LITE2, HPSDR1, HPSDR2, RADIOS, RED_PITAYA,
    R_ADDRESS, R_BOARD, R_BUSY, R_MAC, R_PROTOCOL, R_RECEIVERS, R_VERSION, WAIT,
};
use sdrkit_core::net::{FakeTransport, NetError, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandSet, Complex32, Field, Owner, Record, Results, SettingSet, Value, Values,
};
use sdrkit_driver_core::address::ADDRESS as ADDRESS_TEXT;
use sdrkit_driver_core::{
    own_id, ring, tx_schema, Capabilities, DriverError, DriverKind, LossReport, RadioSetting,
    Retuned, RingReader, Source, SourceDriver, Transmitter, TxSetting, TxState,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---- Helpers ----

/// A driver of one profile on a fake network.
pub struct Drv {
    d: HpsdrDriver,
}

impl std::ops::Deref for Drv {
    type Target = HpsdrDriver;
    fn deref(&self) -> &HpsdrDriver {
        &self.d
    }
}

/// The kind owner of `d`, for its commands.
pub fn kind(d: Drv) -> DriverKind {
    DriverKind::new(Box::new(d.d))
}

/// Runs command `c` of kind `k`: a probe of `address`, or a search, each waiting `wait`. Returns the radios found, newest first.
pub fn run(
    k: &mut DriverKind,
    c: HpsdrKindCommand,
    address: &str,
    wait: Duration,
) -> Result<Vec<Record>, DriverError> {
    let commands = HpsdrKindCommand::commands();
    let mut given = commands.def(c.id()).unwrap().new_args();
    given.set(&WAIT.field, wait.as_secs_f64());
    if c == HpsdrKindCommand::Probe {
        given.set(&ADDRESS.field, address);
    }
    let args = commands.check(c.id(), &given).unwrap();
    if let Err(e) = k.invoke(c.id(), &args) {
        let CommandError::Failed { source } = e else {
            panic!("{e}")
        };
        return Err(source.downcast_ref::<DriverError>().unwrap().clone());
    }
    let id = k.results_decl().table_id(RADIOS.key).unwrap();
    let t = k.results().table(id).unwrap();
    Ok(t.rows_newest_first().map(|(_, r)| r.clone()).collect())
}

/// The cells of a row of [`RADIOS`]: address, busy, board, receivers, MAC, version and protocol.
pub fn cells(r: &Record) -> (String, bool, i64, Option<i64>, String, String, Option<i64>) {
    (
        r.get(&R_ADDRESS).unwrap().to_string(),
        r.get(&R_BUSY).unwrap(),
        r.get(&R_BOARD).unwrap(),
        r.get(&R_RECEIVERS),
        r.get(&R_MAC).unwrap().to_string(),
        r.get(&R_VERSION).unwrap().to_string(),
        r.get(&R_PROTOCOL),
    )
}

pub fn drv(p: Profile, net: Arc<dyn Transport>) -> Drv {
    Drv {
        d: HpsdrDriver::new(p, net),
    }
}

impl Drv {
    pub fn with_timing(self, t: Timing) -> Drv {
        Drv {
            d: self.d.with_timing(t),
        }
    }

    /// The values of a radio at `centre`, asking `rate`, with the hand gain `gain` (`None`: automatic) and the settings `kv`.
    pub fn tune(&self, centre: f64, rate: f64, gain: Option<f64>, kv: &[(&str, Value)]) -> Values {
        let schema = self.d.descriptor().decl().schema().clone();
        let mut v = Values::defaults(&schema);
        let mut all = vec![
            ("centre_hz", Value::Hz(centre)),
            ("rate", Value::Hz(rate)),
            ("agc", Value::Bool(gain.is_none())),
        ];
        if let Some(g) = gain {
            all.push(("gain", Value::F64(g)));
        }
        all.extend(kv.iter().cloned());
        for (key, value) in &all {
            v.set(schema.id(key).unwrap(), value).unwrap();
        }
        v
    }

    pub fn open_source(&self, address: &str, v: &Values) -> Result<Rx, DriverError> {
        let src = self.d.open(address, v)?;
        Ok(Rx {
            src,
            decl_of: self.d.p.model,
            lines: Vec::new(),
            res: results(self.d.p.model),
        })
    }

    pub fn live(&self) -> usize {
        self.d.live_streams()
    }
}

fn results(m: Model) -> Results {
    Results::new(descriptor(m).decl().results().clone())
}

/// An open receiver, with the results it writes.
pub struct Rx {
    pub src: Box<dyn Source>,
    decl_of: Model,
    lines: Vec<String>,
    res: Results,
}

impl Rx {
    /// Starts into a ring of the type of the radio and returns the rate and the ring.
    pub fn start(&mut self) -> (f64, RingReader) {
        let (w, r) = ring(1 << 21, self.src.sample_type());
        let st = self.src.start(w).unwrap();
        (st.rate_hz, r)
    }

    pub fn retune(&mut self, v: &Values) -> Result<Retuned, DriverError> {
        self.src.apply(v)
    }

    pub fn poll(&mut self) -> &Results {
        self.res = results(self.decl_of);
        self.src.poll(&mut self.res);
        self.lines.extend(self.res.log_lines().iter().cloned());
        &self.res
    }

    /// The log lines since the last call.
    pub fn lines(&mut self) -> Vec<String> {
        self.poll();
        std::mem::take(&mut self.lines)
    }

    pub fn status(&mut self) -> String {
        self.poll().status().summary.clone()
    }

    pub fn int(&mut self, f: &Field<field::I64>) -> Option<i64> {
        self.poll().fields().get(f)
    }

    pub fn num(&mut self, f: &Field<field::F64>) -> Option<f64> {
        self.poll().fields().get(f)
    }

    pub fn flag(&mut self, f: &Field<field::Bool>) -> Option<bool> {
        self.poll().fields().get(f)
    }

    pub fn text(&mut self, f: &Field<field::Text>) -> Option<String> {
        self.poll().fields().get(f).map(str::to_string)
    }

    pub fn capabilities(&self) -> Capabilities {
        self.src.capabilities().clone()
    }

    pub fn loss(&self) -> Option<LossReport> {
        self.src.loss()
    }

    pub fn transmitter(&mut self, v: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        self.src.transmitter(v)
    }

    pub fn stop(&mut self) {
        self.src.stop();
    }
}

/// Drops what the ring holds now.
pub fn drain(r: &mut RingReader) {
    while r.read_with(1 << 16, |_| {}) > 0 {}
}

/// Reads at least `min` samples (up to 3 s).
pub fn take(r: &mut RingReader, min: usize) -> Vec<Complex32> {
    let end = Instant::now() + Duration::from_secs(3);
    let mut v = Vec::new();
    let mut buf = vec![Complex32::new(0.0, 0.0); 65536];
    while v.len() < min && Instant::now() < end {
        r.wait(1, Duration::from_millis(50));
        let n = r.read_iq(&mut buf);
        v.extend_from_slice(&buf[..n]);
    }
    v
}

pub fn all(v: &[Complex32], re: f32, im: f32, tol: f32) -> bool {
    !v.is_empty()
        && v.iter()
            .all(|s| (s.re - re).abs() <= tol && (s.im - im).abs() <= tol)
}

pub fn has(lines: &[String], text: &str) -> bool {
    lines.iter().any(|l| l.contains(text))
}

pub fn on() -> Value {
    Value::Bool(true)
}

/// The values of a transmitter at `f` with the attenuation `att_db` (the power -`att_db`).
pub fn tx_values(f: f64, att_db: f64) -> Values {
    let limits = sdrkit_driver_core::TxLimits {
        power_db: crate::tx::POWER_DB,
        ..Default::default()
    };
    let schema = tx_schema(&[], &limits);
    let mut v = Values::defaults(&schema);
    v.set(TxSetting::Frequency.id(), &Value::Hz(f)).unwrap();
    v.set(TxSetting::Power.id(), &Value::F64(-att_db)).unwrap();
    v
}

pub fn hpsdr_error(e: &DriverError) -> Option<HpsdrError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<HpsdrError>().copied(),
        _ => None,
    }
}

pub fn refused(s: RadioSetting) -> DriverError {
    DriverError::Refused { id: s.id() }
}

fn frame_bytes(head: [u8; 8]) -> Vec<u8> {
    let mut v = head.to_vec();
    v.extend(std::iter::repeat_n(0, 504));
    v
}

/// Full scale of a 32-bit word times `x`.
fn w(x: f64) -> i32 {
    (x * 2_147_483_648.0) as i32
}

// ---- Rules without a network ----

#[test]
fn rates_are_chosen_as_the_table_says() {
    let full = profile::RATES_P2_FULL;
    let cases: [(f64, Option<u32>, u8, Option<u32>); 9] = [
        (0.0, Some(384_000), 3, Some(1_536_000)),
        (48_000.0, Some(48_000), 0, Some(48_000)),
        (47_996.0, Some(48_000), 0, Some(48_000)),
        (50_000.0, Some(96_000), 1, Some(96_000)),
        (100_000.0, Some(192_000), 2, Some(192_000)),
        (150_000.0, Some(192_000), 2, Some(192_000)),
        (384_000.0, Some(384_000), 3, Some(384_000)),
        (500_000.0, None, 0, Some(768_000)),
        (2_000_000.0, None, 0, None),
    ];
    for (asked, r1, code, r2) in cases {
        let got1 = plan::choose_rate(asked, &profile::RATES_P1, false);
        let got2 = plan::choose_rate(asked, &full, false);
        assert_eq!(got1.as_ref().ok().copied(), r1, "asked {asked}");
        assert_eq!(got2.as_ref().ok().copied(), r2, "asked {asked}");
        match got1 {
            Ok(r) => assert_eq!(plan::rate_code(r), code),
            Err(e) => assert_eq!(e, refused(RadioSetting::Rate)),
        }
    }
    assert_eq!(
        plan::choose_rate(96_000.0, &profile::RATES_P1, true),
        Ok(96_000)
    );
    assert_eq!(
        plan::choose_rate(100_000.0, &profile::RATES_P1, true),
        Err(refused(RadioSetting::Rate))
    );
    assert_eq!(HPSDR2.rates(Some(1)), &[48_000, 96_000, 192_000, 384_000]);
    assert_eq!(HPSDR2.rates(Some(5)).len(), 6);
}

#[test]
fn frequency_gain_and_filters_follow_the_rules() {
    assert_eq!(plan::freq_sent(7_074_000.0, 0.0), Some(7_074_000));
    assert_eq!(
        plan::freq_sent(10e6, 10.0),
        Some((10e6 / 1.00001f64).round() as u32)
    );
    assert_eq!(plan::freq_sent(-5.0, 0.0), None);
    assert_eq!(plan::freq_sent(9e9, 0.0), None);
    assert_eq!(plan::freq_sent(4.29e9, 0.0), Some(4_290_000_000));
    assert_eq!(
        plan::freq_note(&HERMES_LITE2, 100_000_000),
        "the Hermes Lite 2 is specified for up to 38.4 MHz; the driver may not reach 100.000 MHz or the signal may be weak or absent"
    );
    assert!(plan::freq_note(&HERMES_LITE2, 38_400_001).is_empty());
    assert!(plan::freq_note(&RED_PITAYA, 70_000_000).contains("specified for up to 61.4 MHz"));
    let g = |p: &Profile, has: bool, db: Option<f64>| plan::gain(p, has, db);
    assert_eq!(g(&HERMES_LITE2, true, Some(20.0)), Ok(Gain::Lna(20)));
    assert_eq!(
        g(&HERMES_LITE2, true, None),
        Ok(Gain::Lna(plan::LNA_DEFAULT_DB))
    );
    assert_eq!(plan::lna_byte(20), 0x60);
    assert_eq!(
        g(&HERMES_LITE2, true, Some(60.0)),
        Err(refused(RadioSetting::Gain))
    );
    assert_eq!(g(&HERMES_LITE2, true, Some(48.0)), Ok(Gain::Lna(48)));
    assert_eq!(plan::lna_byte(48), 0x7C);
    assert_eq!(g(&HPSDR1, true, Some(-10.0)), Ok(Gain::Atten(10)));
    assert_eq!(g(&HPSDR1, true, Some(-31.0)), Ok(Gain::Atten(31)));
    assert_eq!(
        g(&HPSDR1, true, Some(-50.0)),
        Err(refused(RadioSetting::Gain))
    );
    assert_eq!(
        g(&HPSDR1, true, Some(5.0)),
        Err(refused(RadioSetting::Gain))
    );
    assert_eq!(g(&HPSDR1, true, None), Ok(Gain::Atten(0)));
    assert_eq!(
        g(&HPSDR1, HPSDR1.has_gain(Some(0)), Some(-5.0)),
        Ok(Gain::None)
    );
    assert!(RED_PITAYA.has_gain(None));
    assert_eq!(g(&RED_PITAYA, true, Some(20.0)), Ok(Gain::Words(655_360)));
    assert_eq!(g(&RED_PITAYA, true, None), Ok(Gain::Words(plan::UNITY)));
    assert_eq!(
        g(&RED_PITAYA, true, Some(41.0)),
        Err(refused(RadioSetting::Gain))
    );
    assert_eq!(plan::scale_word(1 << 20, 655_360), 10 << 20);
    assert_eq!(plan::scale_word(i32::MAX / 2, 655_360), i32::MAX);
    assert_eq!(plan::scale_word(i32::MIN / 2, 655_360), i32::MIN);
    for (f, b) in [
        (1_900_000, 0x02),
        (3_573_000, 0x04),
        (7_074_000, 0x08),
        (14_200_000, 0x10),
        (21_000_000, 0x20),
        (28_000_000, 0x40),
        (30_000_000, 0x00),
    ] {
        assert_eq!(plan::n2adr_bits(f), b, "{f}");
    }
    assert_eq!(plan::phase_word(7_074_000), 0x0EBC_CCCD);
    assert_eq!(plan::phase_word(14_200_000), 0x1D95_5555);
    assert_eq!(plan::phase_word(1_000_000), 0x0215_5555);
    assert_eq!(plan::phase_word(50_100_000), 0x6860_0000);
    assert_eq!(plan::phase_word(7_100_000), 0x0ECA_AAAB);
    assert_eq!(plan::phase_word(130_000_000), 0xFFFF_FFFF);
    assert!((plan::hl2_temperature(798) - 13.5).abs() < 0.1);
    assert!(plan::swr(100, 0).is_none() && plan::swr(10, 20).is_none());
    assert!((plan::swr(400, 100).unwrap() - 3.0).abs() < 1e-9);
    assert_eq!(plan::drive_byte(50.0), 128);
    assert_eq!(crate::tx::drive(-6.0), 128);
    assert_eq!(crate::tx::drive(0.0), 255);
    assert_eq!(plan::tx16(0.5), 0x4000);
    assert_eq!(plan::tx16(-0.25), -0x2000);
    assert_eq!(plan::alex_hpf(7_074_000), plan::alex::HPF_6_5);
    assert_eq!(plan::alex_hpf(1_000_000), plan::alex::HPF_BYPASS);
    assert_eq!(plan::alex_hpf(51_000_000), plan::alex::PREAMP_6M);
}

#[test]
fn settings_are_declared_per_profile() {
    let keys = |m: Model| {
        descriptor(m)
            .decl()
            .schema()
            .settings()
            .iter()
            .skip(RadioSetting::SETTINGS.len())
            .map(|s| s.field.key.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keys(Model::HermesLite2),
        ["ppm", "swapiq", "txtimeout", "n2adr", "hl2pa"]
    );
    assert_eq!(keys(Model::RedPitaya), ["ppm", "swapiq", "txtimeout"]);
    assert_eq!(keys(Model::Hpsdr2), ["ppm", "swapiq", "txtimeout"]);
    let schema = descriptor(Model::HermesLite2).decl().schema().clone();
    let n2adr = schema.setting(schema.id("n2adr").unwrap()).unwrap();
    assert_eq!(n2adr.default, Value::Bool(true));
    assert!(schema.settings().iter().all(|x| !x.restart));
    assert_eq!(
        schema.id("txtimeout").unwrap(),
        own_id(HpsdrSetting::TxTimeout),
        "the own id of txtimeout"
    );
    // A time limit below 10 s is refused by the schema.
    let mut v = Values::defaults(&schema);
    assert!(v
        .set(schema.id("txtimeout").unwrap(), &Value::F64(5.0))
        .is_err());
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let keys: Vec<_> = crate::drivers(net)
        .iter()
        .map(|d| d.descriptor().key)
        .collect();
    assert_eq!(keys, ["hermeslite2", "redpitaya", "hpsdr1", "hpsdr2"]);
    for m in [Model::HermesLite2, Model::Hpsdr2] {
        assert_eq!(
            descriptor(m).produces[0].sample,
            sdrkit_graph::stream::SampleType::Cs32
        );
    }
}

#[test]
fn addresses_name_a_receiver() {
    let a = parse_address(&HERMES_LITE2, "127.0.0.1:1024/rx3").unwrap();
    assert_eq!(
        (a.host.as_str(), a.port, a.receiver),
        ("127.0.0.1", 1024, 3)
    );
    assert_eq!(
        parse_address(&HERMES_LITE2, "10.0.0.2:50123")
            .unwrap()
            .receiver,
        1
    );
    assert_eq!(
        parse_address(&HERMES_LITE2, "127.0.0.1").unwrap_err(),
        DriverError::Address
    );
    assert!(parse_address(&HERMES_LITE2, ":1024").is_err());
    assert!(parse_address(&HERMES_LITE2, "h:0").is_err());
    assert!(parse_address(&HERMES_LITE2, "h:1024/rx0").is_err());
    let e = parse_address(&HERMES_LITE2, "127.0.0.1:1024/rx13").unwrap_err();
    assert_eq!(e, DriverError::Streams { given: 13, has: 12 });
    assert!(!e.retryable());
}

#[test]
fn the_address_command_builds_an_address_that_the_parser_reads() {
    let drivers = crate::drivers(Arc::new(FakeTransport::new()));
    for (driver, p) in drivers.into_iter().zip(profile::ALL) {
        let mut k = DriverKind::new(driver);
        let key = k.descriptor().key;
        let id = k.commands().id("address").unwrap();
        let mut build = |receiver: Option<usize>| {
            let mut given = k.commands().def(id).unwrap().new_args();
            let parts = [
                ("host", Some(Value::Text("192.0.2.1".into()))),
                ("port", Some(Value::I64(5555))),
                ("receiver", receiver.map(|n| Value::I64(n as i64))),
            ];
            for (part, value) in parts {
                if let Some(v) = value {
                    let slot = given.decl().id(part).unwrap();
                    given.set_value(slot, &v).unwrap();
                }
            }
            let reply = sdrkit_core::owner::invoke(&mut k, id, &given)?;
            let text = reply.unwrap();
            Ok::<_, CommandError>(text.get(&ADDRESS_TEXT).unwrap().to_string())
        };
        let last = usize::from(p.max_receivers);
        let first = build(None).unwrap();
        assert_eq!(first, format!("{key}:192.0.2.1:5555"));
        for (text, receiver) in [(first, 1), (build(Some(last)).unwrap(), last)] {
            let rest = text.strip_prefix(key).unwrap().strip_prefix(':').unwrap();
            let a = parse_address(&p, rest).unwrap();
            assert_eq!(
                (a.host.as_str(), a.port, a.receiver),
                ("192.0.2.1", 5555, receiver),
                "{text}"
            );
        }
        assert!(matches!(
            build(Some(last + 1)),
            Err(CommandError::BadArgument { .. })
        ));
    }
}

#[test]
fn ep6_samples_are_words_and_are_laid_out_by_receiver_count() {
    let slots: Vec<usize> = (1..=12).map(p1::slots).collect();
    assert_eq!(slots, [63, 36, 25, 19, 15, 13, 11, 10, 9, 8, 7, 6]);
    let s = |b: [u8; 6]| p1::sample(&b[..3], &b[3..], false);
    assert_eq!(s([0xe0, 0, 0, 0x40, 0, 0]), [w(0.5), w(-0.25)]);
    assert_eq!(s([0xff, 0xff, 0xff, 0x80, 0, 0]), [i32::MIN, -256]);
    assert_eq!(s([0x7f, 0xff, 0xff, 0, 0, 1]), [256, 0x7fff_ff00]);
    let x = p1::sample(&[0xe0, 0, 0], &[0x40, 0, 0], true);
    assert_eq!(x, [w(-0.25), w(0.5)]);
    // A packet of four receivers: 38 per receiver, frame 0's slots then frame 1's.
    let mut pkt = vec![0u8; 1032];
    for f in 0..2 {
        let o = 8 + f * 512;
        pkt[o..o + 3].copy_from_slice(&[0x7f; 3]);
        for sl in 0..19 {
            for k in 0..4 {
                let wo = 8 + o + sl * 26 + 6 * k;
                // word B = slot index + 19 x frame, receiver k in word A.
                pkt[wo + 2] = k as u8;
                pkt[wo + 5] = (sl + 19 * f) as u8;
            }
        }
    }
    assert_eq!(p1::per_packet(4), 38);
    assert!(p1::packet_fits(&pkt, 4));
    for i in 0..38 {
        for k in 0..4 {
            let [re, im] = p1::rx_sample(&pkt, 4, k, i, false);
            assert_eq!((re >> 8, im >> 8), (i as i32, k as i32));
        }
    }
    assert_eq!(p1::per_packet(1), 126);
    // Protocol 2: I then Q.
    let mut b = vec![0u8; 22];
    b[16..22].copy_from_slice(&[0x40, 0, 0, 0xe0, 0, 0]);
    assert_eq!(p2::ddc_sample(&b, 0, false), [w(0.5), w(-0.25)]);
    assert_eq!(p2::ddc_sample(&b, 0, true), [w(-0.25), w(0.5)]);
}

#[test]
fn discovery_replies_are_read() {
    let mut r = vec![
        0xef, 0xfe, 0x02, 0x00, 0x1c, 0xc0, 0xa2, 0x12, 0x34, 0x49, 0x06, 0, 0, 0, 0, 0, 0, 0, 0,
        0x04, 0x00, 0x02,
    ];
    r.extend(std::iter::repeat_n(0, 38));
    assert_eq!(r.len(), 60);
    let x = p1::Reply::parse(&r).unwrap();
    assert_eq!(
        (x.board, x.gateware, x.minor, x.receivers, x.busy),
        (6, 73, Some(2), Some(4), false)
    );
    assert_eq!(p1::mac_text(&x.mac), "00:1C:C0:A2:12:34");
    r[2] = 3;
    assert!(p1::Reply::parse(&r).unwrap().busy);
    r[2] = 4;
    assert!(p1::Reply::parse(&r).is_none());
    r[2] = 2;
    r[0x13] = 1;
    assert_eq!(p1::Reply::parse(&r).unwrap().receivers, Some(1));
    let short = [
        0xef, 0xfe, 0x02, 0x00, 0x1c, 0xc0, 0xa2, 0x12, 0x34, 0x49, 0x06,
    ];
    let x = p1::Reply::parse(&short).unwrap();
    assert_eq!(
        (x.board, x.gateware, x.receivers, x.minor),
        (6, 73, None, None)
    );
    assert!(p1::Reply::parse(&short[..10]).is_none());
    let mut ep = short;
    ep[2] = 1;
    assert!(p1::Reply::parse(&ep).is_none());
    assert_eq!(p1::discovery_request().len(), 63);
    assert_eq!(p1::discovery_request()[..3], [0xef, 0xfe, 0x02]);
    assert!(p1::discovery_request()[3..].iter().all(|&b| b == 0));
    assert_eq!(p1::command(1)[..4], [0xef, 0xfe, 0x04, 0x01]);
    assert_eq!(p1::command(0).len(), 64);
    // Protocol 2 reply.
    let mut r2 = vec![
        0, 0, 0, 0, 0x02, 0x00, 0x1c, 0xc0, 0xa2, 0x56, 0x78, 0x05, 0x26, 0x15, 0, 0, 0, 0, 0, 0,
        0x04, 0x01, 0x00,
    ];
    r2.extend(std::iter::repeat_n(0, 37));
    let x = p2::Reply::parse(&r2).unwrap();
    assert_eq!(
        (x.board, x.protocol, x.firmware, x.receivers),
        (5, 38, 21, Some(4))
    );
    assert_eq!(profile::board_name(2, 5), "Orion MkII");
}

#[test]
fn status_bytes_are_read() {
    let mut f = [0u8; 512];
    f[..3].copy_from_slice(&[0x7f; 3]);
    f[3] = 0x08;
    f[4] = 0x03;
    f[5] = 0x1e;
    let s = p1::frame_status(&f, true);
    assert_eq!((s.address, s.ack), (1, false));
    assert_eq!(((s.c[0] as u16) << 8) | s.c[1] as u16, 798);
    f[3] = 0x88;
    assert!(p1::frame_status(&f, true).ack);
    assert!(!p1::frame_status(&f, false).ack);
}

// ---- Discovery and probe against the fake ----

#[test]
fn discovery_finds_the_fake_and_says_its_receivers() {
    let fake = Fake1::hl2();
    let mut k = kind(drv(HERMES_LITE2, fake.transport()));
    let ms = Duration::from_millis;
    let found = run(&mut k, HpsdrKindCommand::Discover, "", ms(200)).unwrap();
    assert_eq!(found.len(), 1);
    let (address, busy, board, receivers, mac, version, _) = cells(&found[0]);
    assert_eq!((board, receivers, busy), (6, Some(4), false));
    assert_eq!(address, fake.addr());
    assert_eq!(
        (mac.as_str(), version.as_str()),
        ("00:1C:C0:A2:12:34", "73.2")
    );
    // Busy, two receivers, a cut reply.
    fake.with(|s| {
        s.busy = true;
        s.byte13 = 2;
    });
    let f = cells(&run(&mut k, HpsdrKindCommand::Probe, &fake.addr(), ms(200)).unwrap()[0]);
    assert!(f.1);
    assert_eq!(f.3, Some(2));
    fake.with(|s| {
        s.reply = Some(vec![
            0xef, 0xfe, 0x02, 0x00, 0x1c, 0xc0, 0xa2, 0x12, 0x34, 0x49, 0x06,
        ])
    });
    let f = cells(&run(&mut k, HpsdrKindCommand::Probe, &fake.addr(), ms(200)).unwrap()[0]);
    assert_eq!((f.3, f.5.as_str()), (None, "73"));
    // Ignored replies.
    for bad in [
        vec![0xef, 0xfe, 0x02],
        vec![0xef, 0xfe, 0x01, 0, 0, 0, 0, 0, 0, 0, 6],
        vec![0xef, 0xfe, 0x04, 0, 0, 0, 0, 0, 0, 0, 6],
    ] {
        fake.with(|s| s.reply = Some(bad.clone()));
        assert_eq!(
            run(&mut k, HpsdrKindCommand::Probe, &fake.addr(), ms(150)),
            Err(NetError::TimedOut.into())
        );
    }
}

#[test]
fn a_probe_checks_the_board() {
    let net = FakeTransport::new();
    let rp = Fake1::on(&net, "127.0.0.1", 1024, 1, 32, 0);
    let hl = Fake1::on(&net, "127.0.0.2", 1024, 6, 73, 4);
    let t = Duration::from_millis(200);
    let probe = |p: Profile, addr: &str| {
        run(
            &mut kind(drv(p, rp.transport())),
            HpsdrKindCommand::Probe,
            addr,
            t,
        )
    };
    assert_eq!(
        probe(HERMES_LITE2, &rp.addr()).unwrap_err(),
        NetError::TimedOut.into()
    );
    assert_eq!(
        probe(RED_PITAYA, &hl.addr()).unwrap_err(),
        NetError::TimedOut.into()
    );
    let f = cells(&probe(RED_PITAYA, &rp.addr()).unwrap()[0]);
    assert_eq!((f.2, f.3), (1, None));
    // Discovery lists only the boards of the profile.
    let found = run(
        &mut kind(drv(HERMES_LITE2, rp.transport())),
        HpsdrKindCommand::Discover,
        "",
        t,
    )
    .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(cells(&found[0]).0, hl.addr());
}

// ---- Open, stream, retune ----

#[test]
fn open_sends_the_transcript_and_streams() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let v = d.tune(7_074_000.0, 150_000.0, Some(20.0), &[("n2adr", on())]);
    let mut src = d.open_source(&fake.addr(), &v).unwrap();
    let (rate, mut r) = src.start();
    assert_eq!(rate, 192_000.0);
    assert!(fake.wait(Duration::from_secs(1), |s| s.ep2 >= 10));
    let got = fake.with(|s| s.got.clone());
    // 1: discovery, from the same socket as everything else.
    assert_eq!(got[0].bytes, p1::discovery_request().to_vec());
    let from = got[0].from;
    assert!(got.iter().all(|g| g.from == from));
    // 2, 3: two EP2 packets.
    let mut ep2_0 = vec![0xef, 0xfe, 0x01, 0x02, 0, 0, 0, 0];
    ep2_0.extend(frame_bytes([
        0x7f, 0x7f, 0x7f, 0x00, 0x02, 0x08, 0x00, 0x04,
    ]));
    ep2_0.extend(frame_bytes([
        0x7f, 0x7f, 0x7f, 0x02, 0x00, 0x6b, 0xf0, 0xd0,
    ]));
    assert_eq!(got[1].bytes, ep2_0);
    let mut ep2_1 = vec![0xef, 0xfe, 0x01, 0x02, 0, 0, 0, 1];
    ep2_1.extend(frame_bytes([
        0x7f, 0x7f, 0x7f, 0x04, 0x00, 0x6b, 0xf0, 0xd0,
    ]));
    ep2_1.extend(frame_bytes([
        0x7f, 0x7f, 0x7f, 0x14, 0x00, 0x00, 0x00, 0x60,
    ]));
    assert_eq!(got[2].bytes, ep2_1);
    // 4: start.
    let mut start_cmd = vec![0xef, 0xfe, 0x04, 0x01];
    start_cmd.extend([0u8; 60]);
    assert_eq!(got[3].bytes, start_cmd);
    // 5: sequence 2 and 3 with registers 0x09, 0x00 and 0x01, 0x02. 6: the cycle goes on.
    let log = fake.with(|s| s.ep2_log.clone());
    assert_eq!(
        &log[..4],
        &[
            (0, 0x00, 0x01),
            (1, 0x02, 0x0a),
            (2, 0x09, 0x00),
            (3, 0x01, 0x02)
        ]
    );
    let cyc = [0x00, 0x01, 0x02, 0x0a, 0x09];
    for (i, e) in log.iter().enumerate() {
        assert_eq!(e.0, i as u32);
        assert_eq!((e.1, e.2), (cyc[(2 * i) % 5], cyc[(2 * i + 1) % 5]));
    }
    assert_eq!(fake.reg(0x00), Some(0x0208_0004));
    assert_eq!(fake.reg(0x01), Some(7_074_000));
    assert_eq!(fake.reg(0x02), Some(7_074_000));
    assert_eq!(fake.reg(0x0a), Some(0x60));
    assert_eq!(r.sample_type(), sdrkit_graph::stream::SampleType::Cs32);
    let v = take(&mut r, 2000);
    assert!(all(&v, 0.5, -0.25, 1e-6), "{:?}", &v[..4]);
    // swapiq on.
    src.retune(&d.tune(7_074_000.0, 150_000.0, Some(20.0), &[("swapiq", on())]))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    drain(&mut r);
    let v = take(&mut r, 1000);
    assert!(all(&v, -0.25, 0.5, 1e-6));
    let lines = src.lines();
    assert!(
        lines.iter().any(
            |l| l.starts_with("Hermes Lite 2: streaming from 127.0.0.1:")
                && l.ends_with("at 192 ksps, 1 receiver")
        ),
        "{lines:?}"
    );
    assert!(lines.iter().any(
        |l| l.contains("Hermes Lite 2 RX1: receiving from 127.0.0.1:")
            && l.ends_with("at 192 ksps")
    ));
    assert!(!has(&lines, "specified for up to"), "{lines:?}");
    src.stop();
}

#[test]
fn a_live_retune_changes_registers_without_a_restart() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let v = d.tune(7_074_000.0, 150_000.0, Some(20.0), &[("n2adr", on())]);
    let mut src = d.open_source(&fake.addr(), &v).unwrap();
    let (_, mut r) = src.start();
    take(&mut r, 500);
    let starts = fake.with(|s| s.starts);
    let off = [("n2adr", Value::Bool(false))];
    let rt = src
        .retune(&d.tune(14_200_000.0, 384_000.0, Some(48.0), &off))
        .unwrap();
    match rt {
        Retuned::Live(s) => assert_eq!(s.rate_hz, 384_000.0),
        x => panic!("{x:?}"),
    }
    assert!(fake.wait_reg(0x00, 0x0300_0004));
    assert!(fake.wait_reg(0x02, 14_200_000));
    assert!(fake.wait_reg(0x0a, 0x7C));
    // A gain above the LNA range is refused, and nothing changes.
    assert_eq!(
        src.retune(&d.tune(14_200_000.0, 384_000.0, Some(60.0), &off)),
        Err(refused(RadioSetting::Gain))
    );
    let _ = src.lines();
    let rt = src
        .retune(&d.tune(100e6, 384_000.0, Some(48.0), &off))
        .unwrap();
    assert!(matches!(rt, Retuned::Live(_)), "{rt:?}");
    let l = src.lines();
    assert!(has(&l, "the Hermes Lite 2 is specified for up to 38.4 MHz; the driver may not reach 100.000 MHz or the signal may be weak or absent"), "{l:?}");
    assert!(fake.wait_reg(0x02, 100_000_000));
    assert_eq!(fake.with(|s| s.starts), starts);
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(200),
        "{:?}",
        t.elapsed()
    );
    assert!(fake.wait(Duration::from_millis(500), |s| s.stops == 1 && !s.streaming));
    src.stop();
    assert_eq!(d.live(), 0);
}

#[test]
fn red_pitaya_cycle_gain_and_range() {
    let fake = Fake1::red_pitaya();
    let d = drv(RED_PITAYA, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(14_074_000.0, 100_000.0, None, &[]))
        .unwrap();
    let (rate, mut r) = src.start();
    assert_eq!(rate, 192_000.0);
    assert!(fake.wait(Duration::from_secs(1), |s| s.ep2 >= 12));
    assert_eq!(fake.reg(0x00), Some(0x0200_0004));
    assert_eq!(fake.reg(0x02), Some(14_074_000));
    assert_eq!(fake.reg(0x01), Some(14_074_000));
    let log = fake.with(|s| s.ep2_log.clone());
    assert_eq!(
        &log[..3],
        &[(0, 0x00, 0x01), (1, 0x02, 0x09), (2, 0x00, 0x01)]
    );
    assert!(all(&take(&mut r, 1000), 0.5, -0.25, 1e-6));
    let _ = src.lines();
    src.retune(&d.tune(50_100_000.0, 384_000.0, None, &[]))
        .unwrap();
    let l = src.lines();
    assert!(!has(&l, "specified for up to"), "{l:?}");
    assert!(fake.wait_reg(0x00, 0x0300_0004));
    assert!(fake.wait_reg(0x02, 50_100_000));
    let swap = [("swapiq", on())];
    src.retune(&d.tune(50_100_000.0, 384_000.0, None, &swap))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), -0.25, 0.5, 1e-6));
    // The gain is a factor on the sample words: 6 dB is about 2 times, and a word stays within full scale.
    src.retune(&d.tune(50_100_000.0, 384_000.0, Some(6.0), &swap))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), -0.498_8, 0.997_6, 1e-4));
    src.retune(&d.tune(50_100_000.0, 384_000.0, Some(20.0), &swap))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), -1.0, 1.0, 1e-6));
    assert_eq!(
        src.retune(&d.tune(50_100_000.0, 384_000.0, Some(41.0), &swap)),
        Err(DriverError::Refused {
            id: RadioSetting::Gain.id()
        })
    );
    src.retune(&d.tune(70e6, 384_000.0, None, &swap)).unwrap();
    let l = src.lines();
    assert!(has(&l, "specified for up to 61.4 MHz"), "{l:?}");
    assert!(fake.wait_reg(0x02, 70_000_000));
    assert!(!fake.with(|s| s.regs_seen.contains(&0x0a)));
    assert_eq!(fake.reg(0x09), Some(0));
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), -0.25, 0.5, 1e-6));
    let caps = src.capabilities();
    assert_eq!(caps.gain_db, Some((0.0, 40.0, 1.0)));
    assert_eq!(caps.max_freq_hz, 61.44e6);
    src.stop();
}

#[test]
fn several_receivers_share_one_stream() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let a = fake.addr();
    let base = d.tune(7_074_000.0, 150_000.0, Some(20.0), &[]);
    // 1. RX2 alone.
    let mut rx2 = d.open_source(&format!("{a}/rx2"), &base).unwrap();
    let (rate, mut r2) = rx2.start();
    assert_eq!(rate, 192_000.0);
    assert!(fake.wait_reg(0x00, 0x0208_000C));
    assert!(fake.wait_reg(0x03, 7_074_000));
    assert!(fake.wait_reg(0x02, 7_074_000));
    let v = take(&mut r2, 2000);
    assert!(all(&v, 0.375, -0.25, 1e-6), "{:?}", &v[..3]);
    let log = fake.with(|s| s.ep2_log.clone());
    let cyc = [0x00, 0x01, 0x02, 0x03, 0x0a, 0x09];
    for (i, e) in log.iter().enumerate().take(20) {
        assert_eq!(
            (e.1, e.2),
            (cyc[(2 * i) % 6], cyc[(2 * i + 1) % 6]),
            "packet {i}"
        );
    }
    let starts = fake.with(|s| s.starts);
    // 2. RX1 joins asking 384 ksps at 14.2 MHz.
    let mut rx1 = d
        .open_source(&a, &d.tune(14_200_000.0, 384_000.0, Some(20.0), &[]))
        .unwrap();
    let (rate, mut r1) = rx1.start();
    assert_eq!(rate, 192_000.0);
    let l = rx1.lines();
    assert!(has(&l, "share one sample rate"), "{l:?}");
    assert!(
        has(
            &l,
            "RX1 runs at the 192 ksps another receiver of it set, 384 ksps were asked for"
        ),
        "{l:?}"
    );
    assert!(fake.wait_reg(0x00, 0x0210_000C));
    assert!(fake.wait_reg(0x02, 14_200_000));
    assert!(fake.wait_reg(0x01, 14_200_000));
    assert_eq!(fake.reg(0x03), Some(7_074_000));
    assert_eq!(fake.with(|s| s.starts), starts);
    assert!(all(&take(&mut r1, 1000), 0.5, -0.25, 1e-6));
    assert!(all(&take(&mut r2, 1000), 0.375, -0.25, 1e-6));
    // 3. RX4 joins at 3.573 MHz.
    let mut rx4 = d
        .open_source(
            &format!("{a}/rx4"),
            &d.tune(3_573_000.0, 150_000.0, Some(20.0), &[]),
        )
        .unwrap();
    let (_, mut r4) = rx4.start();
    assert!(fake.wait_reg(0x00, 0x0210_001C));
    assert!(fake.wait_reg(0x05, 3_573_000));
    assert!(fake.wait_reg(0x04, 14_200_000));
    assert_eq!(fake.with(|s| s.starts), starts);
    std::thread::sleep(Duration::from_millis(100));
    assert!(all(&take(&mut r1, 2000), 0.5, -0.25, 1e-6));
    assert!(all(&take(&mut r2, 2000), 0.375, -0.25, 1e-6));
    assert!(all(&take(&mut r4, 2000), 0.125, -0.25, 1e-6));
    // Settling loss is counted at the new layout, for the receivers that had samples.
    let s1 = rx1.int(&crate::SETTLE_LOST).unwrap();
    let s2 = rx2.int(&crate::SETTLE_LOST).unwrap();
    assert_eq!(rx4.int(&crate::SETTLE_LOST), Some(0));
    // The fake switches four packets after it sees the new count. How many it sends before register 0x00 reaches it depends
    // on the load of the machine, so only the lower bound and the 300 ms limit (300 packets at 1 ms) are firm.
    for s in [s1, s2] {
        assert!((38 * 3..=38 * 300).contains(&s) && s % 38 == 0, "{s}");
    }
    // 4. RX4 requests 384 ksps.
    let _ = rx4.lines();
    let rt = rx4
        .retune(&d.tune(3_573_000.0, 384_000.0, Some(20.0), &[]))
        .unwrap();
    let Retuned::Live(st) = rt else { panic!() };
    assert_eq!(st.rate_hz, 192_000.0);
    assert!(has(&rx4.lines(), "share one sample rate"));
    // 5. RX1 retunes.
    rx1.retune(&d.tune(10_100_000.0, 384_000.0, Some(20.0), &[]))
        .unwrap();
    assert!(fake.wait_reg(0x02, 10_100_000));
    assert_eq!(fake.reg(0x03), Some(7_074_000));
    // 6. RX4 stops.
    rx4.stop();
    assert!(fake.wait_reg(0x00, 0x0210_000C));
    std::thread::sleep(Duration::from_millis(100));
    drain(&mut r1);
    drain(&mut r2);
    assert!(all(&take(&mut r1, 1000), 0.5, -0.25, 1e-6));
    assert!(all(&take(&mut r2, 1000), 0.375, -0.25, 1e-6));
    assert_eq!(fake.with(|s| s.starts), starts);
    // 7. RX1 stops: still streaming. RX2 stops: a stop.
    rx1.stop();
    std::thread::sleep(Duration::from_millis(50));
    assert!(fake.with(|s| s.streaming && s.stops == 0));
    rx2.stop();
    assert!(fake.wait(Duration::from_millis(500), |s| s.stops == 1 && !s.streaming));
    assert_eq!(d.live(), 0);
    // 8. RX2 alone again.
    let mut rx2 = d
        .open_source(
            &format!("{a}/rx2"),
            &d.tune(7_074_000.0, 384_000.0, Some(20.0), &[]),
        )
        .unwrap();
    let (rate, _r) = rx2.start();
    assert_eq!(rate, 384_000.0);
    // 9. RX5 of a four-receiver radio.
    let e = d.open_source(&format!("{a}/rx5"), &base).err().unwrap();
    assert_eq!(e, DriverError::Streams { given: 5, has: 4 });
    assert!(!e.retryable());
    rx2.stop();
}

#[test]
fn sequence_gaps_are_counted_and_restarts_ignored() {
    let fake = Fake1::hl2();
    fake.with(|s| {
        s.skip.push(1000..=1004);
        s.jump = Some((1100, 201_100));
    });
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, Some(0.0), &[]))
        .unwrap();
    let (_, mut r) = src.start();
    assert!(fake.wait(Duration::from_secs(4), |s| s.seq > 201_200));
    take(&mut r, 100);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(src.int(&crate::SEQ_LOST), Some(630));
    src.stop();
}

#[test]
fn open_failures_say_why() {
    let t1 = Timing { open_attempts: 1 };
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport()).with_timing(t1);
    let v = d.tune(7e6, 48_000.0, None, &[]);
    // Nothing on the port.
    let t = Instant::now();
    let e = d.open_source("127.0.0.1:1025", &v).err().unwrap();
    let took = t.elapsed();
    assert_eq!(e, NetError::TimedOut.into());
    assert!(e.retryable());
    assert!(
        took >= Duration::from_millis(1500) && took < Duration::from_millis(2300),
        "{took:?}"
    );
    // Answers discovery, never streams.
    fake.with(|s| s.never_stream = true);
    let e = d.open_source(&fake.addr(), &v).err().unwrap();
    assert_eq!(e, NetError::TimedOut.into());
    let times = fake.with(|s| s.start_times.clone());
    assert!((3..=4).contains(&times.len()), "{}", times.len());
    // The fake stamps a start when its loop gets to it, which a loaded machine can delay by some tens of milliseconds. The
    // driver spaces them 400 ms apart.
    for w in times.windows(2) {
        assert!(
            w[1] - w[0] >= Duration::from_millis(340),
            "{:?}",
            w[1] - w[0]
        );
    }
    // Bad address: final, nothing sent.
    let e = d.open_source("127.0.0.1", &v).err().unwrap();
    assert_eq!(e, DriverError::Address);
    // RX13.
    let e = d
        .open_source(&format!("{}/rx13", fake.addr()), &v)
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Streams { given: 13, has: 12 });
    // A frequency above what protocol 1 can say.
    let e = d
        .open_source(&fake.addr(), &d.tune(5e9, 48_000.0, None, &[]))
        .err()
        .unwrap();
    assert_eq!(e, refused(RadioSetting::Centre));
    // A rate above every rate of the radio.
    let e = d
        .open_source(&fake.addr(), &d.tune(7e6, 2e6, None, &[]))
        .err()
        .unwrap();
    assert_eq!(e, refused(RadioSetting::Rate));
    // The same receiver twice.
    fake.with(|s| s.never_stream = false);
    let mut a = d.open_source(&fake.addr(), &v).unwrap();
    let e = d.open_source(&fake.addr(), &v).err().unwrap();
    assert_eq!(
        hpsdr_error(&e),
        Some(HpsdrError::ReceiverInUse { receiver: 1 })
    );
    a.stop();
}

#[test]
fn keep_alive_answers_every_packet() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    std::thread::sleep(Duration::from_millis(1000));
    // Let what is in flight arrive (a short quiet, well below the 500 ms start resend), then compare the totals: four EP2
    // packets at open, then one for every EP6.
    fake.with(|s| s.never_stream = true);
    std::thread::sleep(Duration::from_millis(150));
    let (e2, e6) = fake.with(|s| (s.ep2 as i64, s.ep6 as i64));
    fake.with(|s| s.never_stream = false);
    std::thread::sleep(Duration::from_millis(200));
    assert!(e6 > 500, "{e6}");
    assert!((e2 - 4 - e6).abs() <= 2, "EP2 {e2}, EP6 {e6}");
    // A stall without a stop: start again after 500 ms, every 500 ms. The second one brings it back.
    fake.with(|s| s.paused = Some(2));
    let paused_at = Instant::now();
    let starts0 = fake.with(|s| s.starts);
    assert!(fake.wait(Duration::from_secs(3), |s| s.paused.is_none()));
    let times: Vec<_> = fake
        .with(|s| s.start_times.clone())
        .into_iter()
        .skip(starts0 as usize)
        .collect();
    assert_eq!(times.len(), 2);
    let first = times[0] - paused_at;
    assert!(
        first >= Duration::from_millis(450) && first < Duration::from_millis(800),
        "{first:?}"
    );
    assert!(
        times[1] - times[0] >= Duration::from_millis(480),
        "{:?}",
        times[1] - times[0]
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(src.int(&crate::SEQ_LOST).unwrap() > 0);
    let l = src.lines();
    assert!(!has(&l, "for 3 s"), "{l:?}");
    src.stop();
}

#[test]
fn a_lost_stream_is_reopened() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &[]))
        .unwrap();
    let (_, mut r) = src.start();
    take(&mut r, 500);
    let _ = src.lines();
    fake.with(|s| s.paused = Some(u32::MAX));
    let t0 = Instant::now();
    let want = format!("Hermes Lite 2: no samples from {} for 3 s", fake.addr());
    let mut seen = Vec::new();
    let lost_after = loop {
        seen.extend(src.lines());
        if seen.iter().any(|l| l == &want) {
            break t0.elapsed();
        }
        assert!(t0.elapsed() < Duration::from_secs(5), "{seen:?}");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        lost_after >= Duration::from_millis(2700) && lost_after <= Duration::from_millis(3400),
        "{lost_after:?}"
    );
    assert_eq!(fake.with(|s| s.stops), 0);
    // Reopens fail while the fake is silent, then succeed.
    std::thread::sleep(Duration::from_millis(2500));
    let discoveries = fake.with(|s| s.discoveries);
    assert!(discoveries >= 2, "{discoveries}");
    fake.with(|s| s.paused = None);
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        seen.extend(src.lines());
        if seen
            .iter()
            .any(|l| l.contains("Hermes Lite 2 RX1: back after") && l.contains("samples lost"))
        {
            break;
        }
        assert!(Instant::now() < end, "{seen:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(src.loss().unwrap().link_down > 0);
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), 0.5, -0.25, 1e-6));
    assert_eq!(fake.with(|s| s.stops), 0);
    src.stop();
    src.stop();
    assert!(fake.wait(Duration::from_millis(500), |s| s.stops == 1));
    assert_eq!(d.live(), 0);
}

#[test]
fn status_lights_overload_and_shows_temperature() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    fake.with(|s| s.status = [0x00, 0x01, 0, 0, 0]);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(src.flag(&crate::OVERLOAD), Some(true));
    assert!(src.status().contains("ADC overload"));
    fake.with(|s| s.status = [0x08, 0x03, 0x1e, 0, 0]);
    std::thread::sleep(Duration::from_millis(100));
    assert!((src.num(&crate::TEMPERATURE).unwrap() - 13.5).abs() < 0.1);
    // An acknowledgement is not status.
    fake.with(|s| s.status = [0x88, 0x0f, 0xff, 0, 0]);
    std::thread::sleep(Duration::from_millis(700));
    assert!((src.num(&crate::TEMPERATURE).unwrap() - 13.5).abs() < 0.1);
    assert_eq!(src.flag(&crate::OVERLOAD), Some(false));
    std::thread::sleep(Duration::from_millis(600));
    assert!(src.num(&crate::THROUGHPUT).unwrap() > 0.0);
    assert!(src.num(&crate::ACHIEVED).unwrap() > 0.0);
    assert_eq!(src.flag(&BUSY), Some(false));
    assert_eq!(src.text(&crate::MAC).as_deref(), Some("00:1C:C0:A2:12:34"));
    assert_eq!(src.text(&crate::VERSION).as_deref(), Some("73.2"));
    assert_eq!(
        src.text(&crate::BOARD_NAME).as_deref(),
        Some("Hermes-Lite 2")
    );
    src.stop();
}

// ---- Transmit (fake radio only) ----

fn feed(tx: &mut dyn Transmitter, d: Duration) {
    let chunk = vec![Complex32::new(0.5, -0.25); 2048];
    let end = Instant::now() + d;
    while Instant::now() < end {
        let _ = tx.push(&chunk, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn an_open_transmitter_keys_nothing_until_it_is_armed() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let hl2pa = [("hl2pa", on())];
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &hl2pa))
        .unwrap();
    let (_, _r) = src.start();
    assert!(src.capabilities().transmit.is_some());
    assert!(src.src.tx_schema().is_some());
    let mut tx = src.transmitter(&tx_values(7.1e6, 6.0)).unwrap();
    assert_eq!(tx.state(), TxState::Disarmed);
    assert!(fake.wait(Duration::from_secs(1), |s| s.regs_seen.contains(&0x09)));
    std::thread::sleep(Duration::from_millis(100));
    // No MOX, no drive, no PA.
    assert!(!fake.with(|s| s.mox_seen));
    assert_eq!(fake.reg(0x09), Some(0));
    tx.close();
    src.stop();
}

#[test]
fn protocol1_transmit_keys_paces_and_ends() {
    let fake = Fake1::hl2();
    fake.with(|s| s.pace = Pace::Rate);
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    let _ = src.lines();
    assert_eq!(
        src.capabilities().transmit.as_ref().map(|t| (
            t.min_freq_hz,
            t.rates.max_hz,
            t.power_db
        )),
        Some((10e3, 48_000.0, crate::tx::POWER_DB))
    );
    assert_eq!(
        src.src.tx_schema().map(|s| s.settings().len()),
        Some(TxSetting::SETTINGS.len())
    );
    // The radio drives an amplifier: it starts at the bottom of its drive range.
    let schema = src.src.tx_schema().unwrap();
    assert_eq!(
        schema.setting(TxSetting::Power.id()).map(|d| d.default.clone()),
        Some(Value::F64(crate::tx::POWER_DB.0))
    );
    // 0x09 is in the cycle.
    assert!(fake.wait(Duration::from_secs(1), |s| s.regs_seen.contains(&0x09)));
    let mut tx = src.transmitter(&tx_values(7.1e6, 6.0)).unwrap();
    let e = src.transmitter(&tx_values(7.1e6, 6.0)).err().unwrap();
    assert_eq!(hpsdr_error(&e), Some(HpsdrError::TransmitterTaken));
    assert_eq!(
        tx.configure(&tx_values(50e6, 6.0)),
        Err(DriverError::Refused {
            id: TxSetting::Frequency.id()
        })
    );
    tx.configure(&tx_values(7.1e6, 6.0)).unwrap();
    tx.arm().unwrap();
    assert_eq!(tx.state(), TxState::Armed);
    let a = fake.with(|s| s.ep2);
    let t = Instant::now();
    feed(tx.as_mut(), Duration::from_secs(2));
    let rate = (fake.with(|s| s.ep2) - a) as f64 / t.elapsed().as_secs_f64();
    assert!((rate - 381.0).abs() < 381.0 * 0.05, "{rate}");
    assert!(fake.with(|s| s.mox_seen));
    assert_eq!(fake.reg(0x01), Some(7_100_000));
    assert_eq!(fake.reg(0x09).map(|v| v >> 24), Some(0x80));
    assert_eq!(
        fake.with(|s| s.first_slot),
        Some([0, 0, 0, 0, 0x40, 0x00, 0xe0, 0x00])
    );
    let got = fake.with(|s| s.got.clone());
    let last = got
        .iter()
        .rev()
        .find(|g| g.bytes.len() == 1032 && g.bytes[11] >> 1 == 0)
        .unwrap();
    assert_eq!(last.bytes[8..12], [0x7f, 0x7f, 0x7f, 0x01]);
    // The host stops supplying samples: once the queue is empty, zeros, then MOX cleared after 2 s.
    let end = Instant::now() + Duration::from_secs(4);
    while tx.stats().queued > 0 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(1));
    }
    let stopped = Instant::now();
    assert!(fake.wait(Duration::from_secs(4), |s| !s.mox_last));
    let after = stopped.elapsed();
    assert!(
        after >= Duration::from_millis(1900) && after < Duration::from_millis(2600),
        "{after:?}"
    );
    assert_eq!(tx.state(), TxState::Disarmed);
    assert!(tx.stats().underrun_samples > 0);
    tx.close();
    // 384 ksps: one EP2 per eight EP6.
    src.retune(&d.tune(7e6, 384_000.0, None, &[])).unwrap();
    let mut tx = src.transmitter(&tx_values(7.1e6, 20.0)).unwrap();
    tx.arm().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let (a2, a6) = fake.with(|s| (s.ep2, s.ep6));
    feed(tx.as_mut(), Duration::from_secs(2));
    let (b2, b6) = fake.with(|s| (s.ep2, s.ep6));
    let (d2, d6) = ((b2 - a2) as i64, (b6 - a6) as i64);
    assert!((d2 - d6 / 8).abs() <= 2, "EP2 {d2}, EP6 {d6}");
    tx.disarm();
    // The driver clears MOX at once. At 384 ksps the fake takes a while to work through what is in flight when the other
    // tests load the machine, so it gets more time.
    assert_eq!(tx.state(), TxState::Disarmed);
    assert!(fake.wait(Duration::from_millis(1000), |s| !s.mox_last));
    drop(tx);
    src.stop();
}

#[test]
fn protocol1_transmit_times_out() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let set = [("txtimeout", Value::F64(10.0))];
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &set))
        .unwrap();
    let (_, _r) = src.start();
    let mut tx = src.transmitter(&tx_values(7.1e6, 26.0)).unwrap();
    tx.arm().unwrap();
    let t = Instant::now();
    let chunk = vec![Complex32::new(0.1, 0.0); 1024];
    while fake.with(|s| !s.mox_seen || s.mox_last) && t.elapsed() < Duration::from_secs(12) {
        let _ = tx.push(&chunk, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let took = t.elapsed();
    assert!(
        took >= Duration::from_millis(9800) && took <= Duration::from_millis(10_300),
        "{took:?}"
    );
    let mut res = results(Model::HermesLite2);
    tx.poll(&mut res);
    let l: Vec<_> = res.log_lines().iter().cloned().collect();
    assert!(
        l.iter()
            .any(|x| x == "Hermes Lite 2: transmit timed out after 10 s"),
        "{l:?}"
    );
    src.stop();
}

#[test]
fn protocol1_transmit_ends_when_the_stream_is_lost() {
    let fake = Fake1::hl2();
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    let mut tx = src.transmitter(&tx_values(7.1e6, 26.0)).unwrap();
    tx.arm().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert!(fake.with(|s| s.mox_last));
    let from = fake.with(|s| s.got.last().unwrap().from);
    fake.with(|s| s.paused = Some(u32::MAX));
    let chunk = vec![Complex32::new(0.1, 0.0); 512];
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(3500) {
        let _ = tx.push(&chunk, Duration::ZERO);
        std::thread::sleep(Duration::from_millis(5));
    }
    let got = fake.with(|s| s.got.clone());
    let mine: Vec<_> = got
        .iter()
        .filter(|g| g.from == from && g.bytes.len() == 1032)
        .collect();
    let last = mine.last().unwrap();
    assert_eq!(last.bytes[11] & 1, 0, "MOX still set in the last packet");
    let n = mine.len();
    std::thread::sleep(Duration::from_millis(500));
    let later = fake.with(|s| {
        s.got
            .iter()
            .filter(|g| g.from == from && g.bytes.len() == 1032)
            .count()
    });
    assert_eq!(later, n, "packets after the loss");
    assert_eq!(tx.state(), TxState::Closed);
    src.stop();
}

#[test]
fn hpsdr_protocol1_cycle_has_the_drive_and_attenuator_registers() {
    let fake = Fake1::new(1, 30, 0);
    let d = drv(HPSDR1, fake.transport());
    let mut src = d
        .open_source(
            &fake.addr(),
            &d.tune(14_074_000.0, 96_000.0, Some(-10.0), &[]),
        )
        .unwrap();
    let (rate, mut r) = src.start();
    assert_eq!(rate, 96_000.0);
    assert!(fake.wait(Duration::from_secs(1), |s| s.ep2 >= 12));
    let log = fake.with(|s| s.ep2_log.clone());
    let cyc = [0x00, 0x01, 0x02, 0x0a, 0x09];
    for (i, e) in log.iter().enumerate().take(10) {
        assert_eq!(
            (e.1, e.2),
            (cyc[(2 * i) % 5], cyc[(2 * i + 1) % 5]),
            "packet {i}"
        );
    }
    assert_eq!(fake.reg(0x00), Some(0x0100_0004));
    assert_eq!(fake.reg(0x0a), Some(0x2A));
    assert_eq!(fake.reg(0x09), Some(0));
    assert!(all(&take(&mut r, 500), 0.5, -0.25, 1e-6));
    assert_eq!(src.capabilities().gain_db, Some((-31.0, 0.0, 1.0)));
    src.retune(&d.tune(14_074_000.0, 96_000.0, Some(-31.0), &[]))
        .unwrap();
    assert!(fake.wait_reg(0x0a, 0x3F));
    assert_eq!(
        src.retune(&d.tune(14_074_000.0, 96_000.0, Some(-40.0), &[])),
        Err(refused(RadioSetting::Gain))
    );
    src.stop();
    // Metis (board 0) has no attenuator, and Griffin defaults to two receivers.
    assert_eq!(HPSDR1.gain_range(Some(0)), None);
    assert_eq!(HPSDR1.default_receivers(Some(2)), 2);
    assert_eq!(HPSDR1.default_receivers(Some(5)), 7);
}

#[test]
fn settling_starts_with_the_first_samples_of_an_open() {
    // The radio misses the first start: the samples come only after the start sent again 400 ms later, still in the old
    // one-receiver layout for a few packets. None of those can reach the ring of RX2: settling runs from the start of the
    // stream.
    let fake = Fake1::hl2();
    fake.with(|s| s.paused = Some(2));
    let d = drv(HERMES_LITE2, fake.transport());
    let mut rx2 = d
        .open_source(
            &format!("{}/rx2", fake.addr()),
            &d.tune(7_074_000.0, 48_000.0, Some(20.0), &[]),
        )
        .unwrap();
    assert!(fake.with(|s| s.starts) >= 2);
    let (_, mut r2) = rx2.start();
    let v = take(&mut r2, 3000);
    assert!(all(&v, 0.375, -0.25, 1e-6), "{:?}", &v[..4]);
    rx2.stop();
}

#[test]
fn a_host_that_does_not_resolve_is_retried() {
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let host = "no-such-radio.invalid";
    let d = drv(HERMES_LITE2, net.clone()).with_timing(Timing { open_attempts: 1 });
    let t = Instant::now();
    let e = d
        .open_source(&format!("{host}:1024"), &d.tune(7e6, 48_000.0, None, &[]))
        .err()
        .unwrap();
    assert_eq!(e, NetError::NoSuchHost.into());
    assert!(e.retryable());
    assert!(t.elapsed() < Duration::from_millis(500));
    assert_eq!(
        run(
            &mut kind(drv(HERMES_LITE2, net.clone())),
            HpsdrKindCommand::Probe,
            &format!("{host}:1024"),
            Duration::from_millis(50)
        ),
        Err(NetError::NoSuchHost.into())
    );
    // Protocol 2 says the same.
    let d2 = drv(HPSDR2, net).with_timing(Timing { open_attempts: 1 });
    let e = d2
        .open_source(&format!("{host}:1024"), &d2.tune(7e6, 48_000.0, None, &[]))
        .err()
        .unwrap();
    assert_eq!(e, NetError::NoSuchHost.into());
    assert_eq!(d.live(), 0);
}

#[test]
fn a_stop_while_streaming_is_quick_and_throughput_is_measured() {
    // At 48 ksps with one receiver: about 381 packets per second of 1032 bytes (3.15 Mbit/s), 48 000 samples per second.
    let fake = Fake1::hl2();
    fake.with(|s| s.pace = Pace::Rate);
    let d = drv(HERMES_LITE2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7e6, 48_000.0, None, &[]))
        .unwrap();
    let (_, mut r) = src.start();
    let end = Instant::now() + Duration::from_millis(2600);
    while Instant::now() < end {
        take(&mut r, 1);
        drain(&mut r);
    }
    let pps = src.num(&crate::PACKETS).unwrap();
    assert!((pps - 381.0).abs() < 381.0 * 0.08, "{pps}");
    let mbps = src.num(&crate::THROUGHPUT).unwrap();
    assert!((mbps - 3.15).abs() < 3.15 * 0.08, "{mbps}");
    let got = src.num(&crate::ACHIEVED).unwrap();
    assert!((got - 48_000.0).abs() < 48_000.0 * 0.08, "{got}");
    assert_eq!(src.int(&crate::GAPS), Some(0));
    assert_eq!(src.int(&crate::RECEIVERS), Some(1));
    assert_eq!(src.poll().fields().get(&FREQUENCY_SENT), Some(7e6));
    let st = src.status();
    assert!(
        st.contains("RX1 at 48 ksps, 1 receiver in the stream, 7.000 MHz"),
        "{st}"
    );
    // A stop while streaming returns within 200 ms, and the radio gets one stop command.
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(200),
        "{:?}",
        t.elapsed()
    );
    assert!(fake.wait(Duration::from_millis(300), |s| s.stops == 1 && !s.streaming));
    src.stop();
    assert_eq!(fake.with(|s| s.stops), 1);
    assert_eq!(d.live(), 0);
}

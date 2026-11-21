//! The acceptance tests of DVB-SH (the stack's specification, section 4.11): the format layers without a radio, then round
//! trips of the OFDM test signal through impairments and the receiver. The turbo interleaver addresses and the time
//! interleaver's branch delays are tested where they live, in `sdrkit-algo-fec`.

use sdrkit_algo_fec::branchdelay::ShTimeInterleaver;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, SettingSet, StackConfig, Value, Values};
use sdrkit_dev_test::{Awgn, ClockError, Echo, FrequencyOffset};
use sdrkit_graph::{StackGraph, TapReader};
use sdrkit_stack_dvbt::dvbsh::coding::{
    build_eframe, dispersal, parse_eframe, Eframe, ShCodec, EFRAME_BITS, PACKETS, TS,
};
use sdrkit_stack_dvbt::dvbsh::gen::{OfdmGenerator, PATTERN_HP, PATTERN_LP};
use sdrkit_stack_dvbt::dvbsh::ofdm::{OfdmParams, Width};
use sdrkit_stack_dvbt::dvbsh::receiver;
use sdrkit_stack_dvbt::dvbsh::settings::DvbshSetting;
use sdrkit_stack_dvbt::params::{Constellation, Guard, Hierarchy, Mode};
use sdrkit_stack_dvbt::typed::DvbshFields;
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

use super::common::{drain_packets, value};

fn packet(n: u32) -> [u8; TS] {
    PATTERN_HP.packet(n)
}

#[test]
fn crc_and_dispersal_sequence() {
    assert_eq!(CRC16_IBM_3740.checksum(b"123456789"), 0x29B1);
    let w = dispersal(16);
    let v = w.iter().fold(0u32, |a, &b| a << 1 | b as u32);
    assert_eq!(v, 0x03F6);
}

#[test]
fn eframe_round_trip() {
    let prbs = dispersal(EFRAME_BITS);
    let packets: Vec<[u8; TS]> = (0..PACKETS as u32).map(packet).collect();
    let mut bits = vec![0u8; EFRAME_BITS];
    build_eframe(&packets, 17, &prbs, &mut bits);
    let plain: Vec<u8> = bits.iter().zip(&prbs).map(|(a, b)| a ^ b).collect();
    let mut e = Eframe::default();
    parse_eframe(&plain, &mut e);
    let h = e.header.expect("header CRC");
    assert_eq!(
        (h.tis, h.upl, h.dfl, h.sync, h.counter & 1023),
        (3, 1504, 12_096, 0x47, 17)
    );
    assert_eq!(e.ok, [true; PACKETS]);
    assert_eq!(&e.packets[..], &packets[..]);
    // The 72 padding bits are zero.
    assert!(plain[114 + 12_096..].iter().all(|&b| b == 0));
    // One corrupted bit in packet 3: seven good packets, packet 3 flagged.
    let mut bad = plain.clone();
    bad[114 + 3 * 1512 + 100] ^= 1;
    parse_eframe(&bad, &mut e);
    assert_eq!(e.ok.iter().filter(|&&x| x).count(), 7);
    assert!(!e.ok[3] && e.packets[3][1] & 0x80 != 0);
}

#[test]
fn coded_lengths_and_frame_arithmetic() {
    let n_tcb = [
        61_440, 55_296, 49_152, 43_008, 36_864, 36_864, 30_720, 30_720, 24_576, 24_576, 18_432,
        18_432,
    ];
    let data_cu = [810, 810, 816, 798, 810, 810, 810, 810, 816, 816, 810, 810];
    for id in 0..12u8 {
        let c = ShCodec::new(id).unwrap();
        assert_eq!(c.n_tcb(), n_tcb[id as usize], "pattern {id}");
        assert_eq!(c.n_bil(), c.n_tcb() / 128 * 126);
        let iu = c.n_bil() / 126;
        assert_eq!(iu * 126, c.n_bil());
        assert_eq!(iu % 48, 0, "pattern {id}");
        let ncw = 816 * 16 / iu;
        assert_eq!(ncw * c.n_bil(), data_cu[id as usize] * 2016, "pattern {id}");
    }
    assert_eq!(ShCodec::signalling().unwrap().n_tcb(), 5760);
    assert!(ShCodec::new(12).is_err());
}

#[test]
fn noise_free_code_words_of_every_pattern() {
    let mut rng = Rng::new(5);
    for id in 0..12u8 {
        let mut c = ShCodec::new(id).unwrap();
        let bits: Vec<u8> = (0..EFRAME_BITS).map(|_| rng.bit()).collect();
        let mut coded = vec![0u8; c.n_bil()];
        c.encode(&bits, &mut coded).unwrap();
        let llr: Vec<i8> = coded
            .iter()
            .map(|&b| if b == 0 { 40 } else { -40 })
            .collect();
        let mut out = vec![0u8; EFRAME_BITS];
        let d = c.decode(&llr, 8, |h| h == &bits[..], &mut out).unwrap();
        assert_eq!(out, bits, "pattern {id}");
        assert!(d.iterations <= 2);
        let zeros = vec![0u8; EFRAME_BITS];
        c.encode(&zeros, &mut coded).unwrap();
        assert!(coded.iter().all(|&b| b == 0), "pattern {id}");
    }
}

/// One round trip of the OFDM test signal.
#[derive(Clone)]
struct Trip {
    p: OfdmParams,
    width: Width,
    seconds: f64,
    snr_db: Option<f64>,
    cfo_hz: f64,
    ppm: f64,
    echo: Option<(usize, f64)>,
    /// The signal is off (zero) from this fraction of the run for this many seconds.
    off: Option<(f64, f64)>,
    lp: bool,
    lead: usize,
    rate: Option<f64>,
    settings: Vec<(&'static str, Value)>,
}

impl Trip {
    fn new(p: OfdmParams, width: Width, seconds: f64) -> Self {
        Trip {
            p,
            width,
            seconds,
            snr_db: None,
            cfo_hz: 0.0,
            ppm: 0.0,
            echo: None,
            off: None,
            lp: false,
            lead: 0,
            rate: None,
            settings: Vec::new(),
        }
    }
}

/// What a round trip delivered.
struct Delivered {
    good: usize,
    /// Numbered packets missing between the first and the last good one.
    lost: usize,
    flagged: usize,
    wrong: usize,
    codewords_ok: i64,
    codewords_bad: i64,
    rx: StackGraph,
}

fn config(settings: &[(&str, Value)]) -> StackConfig {
    let schema = DvbshSetting::schema();
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        v.set(schema.id(k).unwrap(), x).unwrap();
    }
    StackConfig::new(v)
}

fn signal(t: &Trip) -> (Vec<Complex32>, f64) {
    let mut g = OfdmGenerator::new(t.p, t.width, None).unwrap();
    let nat = g.natural_rate();
    let mut x = Vec::new();
    let n = (t.seconds * nat) as usize;
    while x.len() < n {
        x.extend_from_slice(g.next_symbol());
    }
    // A random start: some way into the first symbol.
    let mut x = x.split_off(t.lead % 1000);
    if t.ppm != 0.0 {
        let mut y = Vec::new();
        ClockError::new(nat, t.ppm).process(&x, &mut y);
        x = y;
    }
    if let Some((d, db)) = t.echo {
        Echo::new(d, db).process(&mut x);
    }
    let rate = t.rate.unwrap_or(nat);
    if (rate - nat).abs() > 1.0 {
        let mut rs =
            sdrkit_algo_dsp::resample::FractionalResampler::<Complex32>::with_rates(nat, rate)
                .unwrap();
        let mut y = Vec::new();
        rs.process(&x, &mut y);
        x = y;
    }
    if t.cfo_hz != 0.0 {
        FrequencyOffset::new(rate, t.cfo_hz).process(&mut x);
    }
    let m = g.params().mode;
    let occupied = m.carriers() as f64 * nat / m.fft() as f64;
    let power = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    if let Some((at, secs)) = t.off {
        let a = (at * x.len() as f64) as usize;
        let b = (a + (secs * rate) as usize).min(x.len());
        x[a..b].fill(Complex32::default());
    }
    if let Some(snr) = t.snr_db {
        Awgn::new(power, snr, occupied, rate, 77).process(&mut x);
    }
    (x, rate)
}

fn run(t: &Trip) -> Delivered {
    let (x, rate) = signal(t);
    run_signal(t, &x, rate)
}

fn run_signal(t: &Trip, x: &[Complex32], rate: f64) -> Delivered {
    let mut settings = vec![("low_priority", Value::Bool(t.lp))];
    settings.extend(t.settings.iter().cloned());
    let mut rx = receiver(&config(&settings)).unwrap();
    rx.configure(&InputInfo::new(rate, 2177.5e6, 2177.5e6).recording())
        .unwrap();
    let mut tap: TapReader = rx.graph_mut().tap("decode.out", 1 << 14).unwrap();
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut packets = Vec::new();
    let mut out = Output::default();
    for c in x.chunks(20_000) {
        rx.feed(c, &mut out);
        out.clear();
        drain_packets(&mut tap, &mut buf, &mut packets);
    }
    rx.finish(&mut out);
    drain_packets(&mut tap, &mut buf, &mut packets);
    let pattern = if t.lp { PATTERN_LP } else { PATTERN_HP };
    let (mut good, mut flagged, mut wrong) = (0, 0, 0);
    let mut numbers = Vec::new();
    for p in &packets {
        if p[1] & 0x80 != 0 {
            flagged += 1;
        } else if let Some(n) = pattern.number_of(p) {
            numbers.push(n);
            good += 1;
        } else if p[1] & 0x1F == 0x1F && p[2] == 0xFF {
            // A null packet of the test stream.
        } else {
            wrong += 1;
        }
    }
    let res = Owner::results(&rx);
    let get = |k: &str| match value(res, k) {
        Some(Value::I64(v)) => *v,
        _ => 0,
    };
    numbers.sort_unstable();
    numbers.dedup();
    let lost = match (numbers.first(), numbers.last()) {
        (Some(&a), Some(&b)) => (b - a + 1) as usize - numbers.len(),
        _ => 0,
    };
    Delivered {
        good,
        lost,
        flagged,
        wrong,
        codewords_ok: get(DvbshFields::CODEWORDS_OK.key),
        codewords_bad: get(DvbshFields::CODEWORDS_BAD.key),
        rx,
    }
}

fn params(mode: Mode, c: Constellation, h: Hierarchy, guard: Guard, hp: u8, lp: u8) -> OfdmParams {
    OfdmParams {
        mode,
        guard,
        constellation: c,
        hierarchy: h,
        hp,
        lp,
        ..Default::default()
    }
}

fn summary(d: &Delivered) -> String {
    let s = Owner::results(&d.rx).status().summary.clone();
    format!(
        "good {} lost {} flagged {} wrong {} codewords {} ok {} bad; {s}",
        d.good, d.lost, d.flagged, d.wrong, d.codewords_ok, d.codewords_bad
    )
}

#[test]
fn clean_2k_qpsk_one_third() {
    let p = params(
        Mode::K2,
        Constellation::Qpsk,
        Hierarchy::None,
        Guard::G8,
        4,
        0,
    );
    let d = run(&Trip::new(p, Width::W5, 2.0));
    assert!(
        d.good > 300 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        summary(&d)
    );
}

#[test]
fn qpsk_2k_one_third_at_4_db() {
    let p = params(
        Mode::K2,
        Constellation::Qpsk,
        Hierarchy::None,
        Guard::G8,
        4,
        0,
    );
    let mut t = Trip::new(p, Width::W5, 3.0);
    t.snr_db = Some(4.0);
    t.cfo_hz = 7.3e3;
    t.ppm = 15.0;
    t.lead = 777;
    let d = run(&t);
    assert!(
        d.good > 300 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        summary(&d)
    );
}

#[test]
fn qam16_8k_one_half_with_an_echo() {
    let p = params(
        Mode::K8,
        Constellation::Qam16,
        Hierarchy::None,
        Guard::G4,
        8,
        0,
    );
    let mut t = Trip::new(p, Width::W8, 2.5);
    t.snr_db = Some(12.0);
    t.cfo_hz = -23e3;
    t.echo = Some((300, -6.0));
    let d = run(&t);
    assert!(
        d.good > 1000 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        summary(&d)
    );
}

#[test]
fn qam16_1k_two_fifths_complementary_at_1_7_mhz() {
    let p = params(
        Mode::K1,
        Constellation::Qam16,
        Hierarchy::None,
        Guard::G32,
        7,
        0,
    );
    let mut t = Trip::new(p, Width::W1_7, 4.0);
    t.snr_db = Some(13.0);
    t.cfo_hz = 1.2e3;
    t.ppm = -30.0;
    let d = run(&t);
    assert!(
        d.good > 500 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        summary(&d)
    );
}

#[test]
fn hierarchical_2k_high_and_low_priority() {
    let p = params(
        Mode::K2,
        Constellation::Qam16,
        Hierarchy::A2,
        Guard::G8,
        4,
        9,
    );
    for lp in [false, true] {
        let mut t = Trip::new(p, Width::W5, 3.0);
        t.snr_db = Some(16.0);
        t.cfo_hz = 2.5e3;
        t.lp = lp;
        let d = run(&t);
        assert!(
            d.good > 300 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
            "lp {lp}: {}",
            summary(&d)
        );
    }
}

#[test]
fn blockage_with_the_late_interleaver() {
    // 2K 16-QAM: an SH frame is 4 OFDM frames, 272 symbols of 252 us at 5 MHz with guard 1/8.
    let sh_frame_s = 272.0 * 2304.0 / (40e6 / 7.0);
    let mut p = params(
        Mode::K2,
        Constellation::Qam16,
        Hierarchy::None,
        Guard::G8,
        4,
        0,
    );
    p.ti = ShTimeInterleaver {
        common_multiplier: 2,
        late_taps: 24,
        slices: 2,
        slice_distance: 1,
        non_late_increment: 1,
    };
    let mut t = Trip::new(p, Width::W5, 4.0);
    t.snr_db = Some(14.0);
    t.off = Some((0.7, 0.25 * sh_frame_s));
    let d = run(&t);
    assert!(
        d.good > 300 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        summary(&d)
    );
    // The same blockage with the uniform profile: some code words lost, none passed as good.
    p.ti = ShTimeInterleaver::default();
    t.p = p;
    let d = run(&t);
    assert!(d.codewords_bad > 0 && d.wrong == 0, "{}", summary(&d));
}

#[test]
fn qpsk_4k_two_sevenths_at_6_mhz() {
    let p = params(
        Mode::K4,
        Constellation::Qpsk,
        Hierarchy::None,
        Guard::G16,
        3,
        0,
    );
    let mut t = Trip::new(p, Width::W6, 2.0);
    t.snr_db = Some(8.0);
    t.cfo_hz = 3.1e3;
    t.ppm = 5.0;
    let d = run(&t);
    assert!(
        d.good > 300 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        summary(&d)
    );
}

#[test]
fn qpsk_2k_two_thirds_at_2_db_passes_no_wrong_packet() {
    let p = params(
        Mode::K2,
        Constellation::Qpsk,
        Hierarchy::None,
        Guard::G8,
        10,
        0,
    );
    let mut t = Trip::new(p, Width::W5, 2.0);
    t.snr_db = Some(2.0);
    let d = run(&t);
    assert!(
        d.good == 0 && d.wrong == 0 && d.codewords_ok == 0 && d.codewords_bad > 0,
        "{}",
        summary(&d)
    );
}

#[test]
fn automatic_width_finds_each_channel() {
    for (width, rate) in [(Width::W5, 8e6), (Width::W8, 10e6), (Width::W1_7, 2.4e6)] {
        let mut p = params(
            Mode::K2,
            Constellation::Qpsk,
            Hierarchy::None,
            Guard::G8,
            4,
            0,
        );
        if width == Width::W1_7 {
            p.mode = Mode::K1;
        }
        let mut t = Trip::new(p, width, 4.0);
        t.snr_db = Some(18.0);
        t.ppm = 8.0;
        t.rate = Some(rate);
        let d = run(&t);
        assert!(
            d.good > 100 && d.wrong == 0,
            "{} MHz: {}",
            width.mhz(),
            summary(&d)
        );
        let w = match value(Owner::results(&d.rx), DvbshFields::WIDTH_MHZ.key) {
            Some(Value::F64(v)) => *v,
            _ => 0.0,
        };
        assert_eq!(w, width.mhz());
    }
}

#[test]
fn the_s_band_raster_is_a_data_file_the_registry_takes() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans");
    let text = std::fs::read_to_string(dir.join("dvbsh.json")).expect("the data file");
    let file = sdrkit_data_plans::data::parse("dvbsh.json", &text).expect("a plan file");
    let mut plans = (*sdrkit_dev_test::plans()).clone();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    let band = &file.groups[0].bands[0];
    assert_eq!(band.channels.len(), 6);
    assert!(band.modes.iter().any(|m| m == "dvbsh"));
    // Every channel lies inside the band, 5 MHz wide.
    for c in &band.channels {
        assert!(c.centre_hz - 2.5e6 >= band.start_hz && c.centre_hz + 2.5e6 <= band.end_hz);
    }
}

#[test]
fn a_code_word_with_fewer_soft_values_than_data_bits_gives_the_packets_of_an_erased_word() {
    use sdrkit_stack_dvbt::dvbsh::coding::{parse_eframe, Eframe, ShCodec};
    let mut codec = ShCodec::new(8).unwrap();
    let (k, n) = (codec.k(), codec.n_bil());
    // The decisions of the decoder for a word that is all erased, descrambled.
    let mut hard = vec![0u8; k];
    assert!(codec.decode(&vec![0i8; n], 2, |_| false, &mut hard).is_err());
    let bits: Vec<u8> = hard.iter().zip(codec.prbs()).map(|(b, p)| b ^ p).collect();
    let mut erased = Eframe::default();
    parse_eframe(&bits, &mut erased);
    for known in [0, k - 1] {
        let mut llr = vec![0i8; n];
        llr[..known].fill(-9);
        let mut out = Eframe::default();
        assert!(codec.decode_eframe(&llr, 8, &mut out).is_err(), "{known} soft values");
        assert_eq!(out.packets, erased.packets, "{known} soft values");
        assert_eq!(out.ok, [false; 8], "{known} soft values");
    }
}

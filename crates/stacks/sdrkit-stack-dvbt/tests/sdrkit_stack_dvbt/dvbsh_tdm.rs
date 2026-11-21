//! Round trips of the TDM test signal of DVB-SH (SH-B) through impairments and the stack. Es/N0 is the power of the signal
//! over the noise in a bandwidth of the symbol rate.

use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Complex32, Generator, InputInfo, LockState, Output, Receiver, SettingSet, StackConfig, Value,
    Values,
};
use sdrkit_dev_test::{Awgn, ClockError, FrequencyOffset};
use sdrkit_graph::{StackGraph, TapReader};
use sdrkit_stack_dvbt::dvbsh::gen::PATTERN_HP;
use sdrkit_stack_dvbt::dvbsh::receiver;
use sdrkit_stack_dvbt::dvbsh::settings::DvbshSetting;
use sdrkit_stack_dvbt::dvbsh::tdm::{Modulation, RollOff};
use sdrkit_stack_dvbt::params::Guard;
use sdrkit_stack_dvbt::dvbsh::tdmgen::{TdmGenerator, TdmParams};
use sdrkit_stack_dvbt::typed::DvbshFields;

use super::common::{drain_packets, value};

/// The sample rate of the round trips.
const RATE: f64 = 8e6;

/// One round trip of the TDM test signal.
#[derive(Clone)]
struct Trip {
    p: TdmParams,
    seconds: f64,
    esn0_db: Option<f64>,
    cfo_hz: f64,
    ppm: f64,
    /// The signal is off (zero) from this second for this many seconds.
    off: Option<(f64, f64)>,
    seed: u64,
    settings: Vec<(&'static str, Value)>,
}

impl Trip {
    fn new(p: TdmParams, seconds: f64) -> Self {
        Trip {
            p,
            seconds,
            esn0_db: None,
            cfo_hz: 0.0,
            ppm: 0.0,
            off: None,
            seed: 77,
            settings: vec![("waveform", Value::Choice(2))],
        }
    }
}

/// What a round trip delivered.
struct Delivered {
    good: usize,
    lost: usize,
    flagged: usize,
    wrong: usize,
    rx: StackGraph,
}

impl Delivered {
    fn int(&self, key: &str) -> i64 {
        match value(Owner::results(&self.rx), key) {
            Some(Value::I64(v)) => *v,
            _ => 0,
        }
    }

    fn real(&self, key: &str) -> Option<f64> {
        match value(Owner::results(&self.rx), key) {
            Some(Value::F64(v) | Value::Duration(v) | Value::Hz(v)) => Some(*v),
            _ => None,
        }
    }

    fn text(&self, key: &str) -> String {
        match value(Owner::results(&self.rx), key) {
            Some(Value::Text(v)) => v.clone(),
            Some(Value::Choice(v)) => v.to_string(),
            _ => String::new(),
        }
    }

    fn summary(&self) -> String {
        let s = Owner::results(&self.rx).status().summary.clone();
        format!(
            "good {} lost {} flagged {} wrong {}; code words {} ok {} bad; first packet {:?} s; {s}",
            self.good,
            self.lost,
            self.flagged,
            self.wrong,
            self.int(DvbshFields::CODEWORDS_OK.key),
            self.int(DvbshFields::CODEWORDS_BAD.key),
            self.real(DvbshFields::FIRST_PACKET_S.key),
        )
    }
}

fn config(settings: &[(&str, Value)]) -> StackConfig {
    let schema = DvbshSetting::schema();
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        v.set(schema.id(k).unwrap(), x).unwrap();
    }
    StackConfig::new(v)
}

/// The test signal of `t` without noise: the clock error, the carrier offset and the time the signal is off.
fn clean(t: &Trip) -> Vec<Complex32> {
    let mut g = TdmGenerator::new(t.p, Some(RATE)).unwrap();
    let mut x = vec![Complex32::default(); (t.seconds * RATE) as usize];
    g.generate(&mut x);
    // A start some way into the first slot.
    let mut x = x.split_off(777);
    if t.ppm != 0.0 {
        let mut y = Vec::new();
        ClockError::new(RATE, t.ppm).process(&x, &mut y);
        x = y;
    }
    if t.cfo_hz != 0.0 {
        FrequencyOffset::new(RATE, t.cfo_hz).process(&mut x);
    }
    if let Some((at, secs)) = t.off {
        let a = ((at * RATE) as usize).min(x.len());
        let b = (a + (secs * RATE) as usize).min(x.len());
        x[a..b].fill(Complex32::default());
    }
    x
}

/// The signal of `t` with its noise.
fn signal(t: &Trip) -> Vec<Complex32> {
    let mut x = clean(t);
    if let Some(db) = t.esn0_db {
        Awgn::new(1.0, db, t.p.symbol_rate(), RATE, t.seed).process(&mut x);
    }
    x
}

fn run(t: &Trip) -> Delivered {
    run_signal(t, &signal(t))
}

fn run_signal(t: &Trip, x: &[Complex32]) -> Delivered {
    let mut rx = receiver(&config(&t.settings)).unwrap();
    rx.configure(&InputInfo::new(RATE, 2177.5e6, 2177.5e6).recording())
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
    let (mut good, mut flagged, mut wrong) = (0, 0, 0);
    let mut numbers = Vec::new();
    for p in &packets {
        if p[1] & 0x80 != 0 {
            flagged += 1;
        } else if let Some(n) = PATTERN_HP.number_of(p) {
            numbers.push(n);
            good += 1;
        } else if p[1] & 0x1F == 0x1F && p[2] == 0xFF {
            // A null packet of the test stream.
        } else {
            wrong += 1;
        }
    }
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
        rx,
    }
}

fn params(modulation: Modulation, id: u8) -> TdmParams {
    TdmParams {
        modulation,
        id,
        ..TdmParams::default()
    }
}

#[test]
fn clean_qpsk_one_third_returns_every_packet() {
    let d = run(&Trip::new(params(Modulation::Qpsk, 4), 1.5));
    assert!(
        d.good > 1000 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
        "{}",
        d.summary()
    );
    // The signalling field gives what the generator was set to; the rate and the roll-off are those of table 5.13.
    assert_eq!(d.text(DvbshFields::WAVEFORM.key), "1");
    assert_eq!(d.text(DvbshFields::TDM_MODULATION.key), "QPSK");
    assert_eq!(d.text(DvbshFields::RATE_HP.key), "1/3");
    assert_eq!(d.int(DvbshFields::TDM_FRAME_WIDTH_CU.key), 784);
    assert_eq!(d.text(DvbshFields::INTERLEAVER.key), "4 / 48 / 1 / 0 / 0");
    assert_eq!(d.real(DvbshFields::TDM_ROLLOFF.key), Some(0.25));
    assert_eq!(d.real(DvbshFields::WIDTH_MHZ.key), Some(5.0));
    let rs = d.real(DvbshFields::TDM_SYMBOL_RATE.key).unwrap();
    assert!((rs - 35e6 / 9.0).abs() < 1.0, "{rs}");
    assert_eq!(d.int(DvbshFields::LOCK.key), 4);
    assert_eq!(Owner::results(&d.rx).status().lock_state, LockState::Locked);
    assert!(d.real(DvbshFields::TDM_MER_DB.key).unwrap() > 25.0, "{}", d.summary());
    let first = d.real(DvbshFields::FIRST_PACKET_S.key).unwrap();
    assert!(first < 1.0, "first packet at {first} s");
}

/// The symbol rate of the default test signal: 5 MHz, guard 1/8, roll-off 0.25.
const RS: f64 = 35e6 / 9.0;

/// The settings of a receiver of the TDM form at the symbol rate of the default test signal.
fn set_rate() -> Vec<(&'static str, Value)> {
    vec![
        ("waveform", Value::Choice(2)),
        ("tdm_symbol_rate", Value::F64(RS)),
    ]
}

#[test]
fn clean_8psk_and_16apsk_return_every_packet() {
    for (m, id, name, rate) in [
        (Modulation::Psk8, 8, "8PSK", "1/2"),
        (Modulation::Apsk16, 10, "16APSK", "2/3"),
    ] {
        let d = run(&Trip::new(params(m, id), 1.3));
        assert!(
            d.good > 1500 && d.lost == 0 && d.flagged == 0 && d.wrong == 0,
            "{name}: {}",
            d.summary()
        );
        assert_eq!(d.text(DvbshFields::TDM_MODULATION.key), name);
        assert_eq!(d.text(DvbshFields::RATE_HP.key), rate);
        assert_eq!(
            d.int(DvbshFields::TDM_FRAME_WIDTH_CU.key),
            392 * m.bits() as i64
        );
        assert_eq!(d.int(DvbshFields::CODEWORDS_BAD.key), 0);
    }
}

#[test]
fn a_set_symbol_rate_gives_the_first_packet_within_half_a_second() {
    let mut t = Trip::new(params(Modulation::Qpsk, 4), 0.8);
    t.settings = set_rate();
    let d = run(&t);
    let first = d.real(DvbshFields::FIRST_PACKET_S.key).unwrap_or(f64::MAX);
    assert!(first < 0.5 && d.wrong == 0, "{}", d.summary());
}

/// Runs `t` and checks that no code word failed, and the carrier offset and the clock error that the stack shows.
fn check_offsets(t: &Trip, good: usize, ppm_within: f64) -> Delivered {
    let d = run(t);
    assert!(
        d.good > good && d.wrong == 0 && d.int(DvbshFields::CODEWORDS_BAD.key) == 0,
        "{}",
        d.summary()
    );
    let cfo = d.real(DvbshFields::CFO_HZ.key).unwrap();
    assert!((cfo - t.cfo_hz).abs() < 300.0, "carrier offset {cfo}");
    let ppm = d.real(DvbshFields::CLOCK_PPM.key).unwrap();
    assert!((ppm - t.ppm).abs() < ppm_within, "clock {ppm} ppm");
    d
}

#[test]
fn qpsk_one_third_at_3_db_roll_off_0_15_with_carrier_and_clock_offsets() {
    // 5 MHz, guard 1/4, roll-off 0.15: 4.25 Msym/s and 952 units; the symbol rate is found.
    let p = TdmParams {
        guard: Guard::G4,
        rolloff: RollOff::R15,
        ..params(Modulation::Qpsk, 4)
    };
    let mut t = Trip::new(p, 1.5);
    (t.esn0_db, t.cfo_hz, t.ppm) = (Some(3.0), 12.3e3, 20.0);
    let d = check_offsets(&t, 800, 15.0);
    assert_eq!(d.int(DvbshFields::TDM_FRAME_WIDTH_CU.key), 952);
    assert_eq!(d.real(DvbshFields::TDM_ROLLOFF.key), Some(0.15));
    let mer = d.real(DvbshFields::TDM_MER_DB.key).unwrap();
    assert!((mer - 3.0).abs() < 1.0, "MER {mer} dB");
}

#[test]
fn psk8_one_half_at_10_db_with_carrier_and_clock_offsets() {
    let mut t = Trip::new(params(Modulation::Psk8, 8), 1.3);
    (t.esn0_db, t.cfo_hz, t.ppm) = (Some(10.0), -7e3, -15.0);
    check_offsets(&t, 1500, 10.0);
}

#[test]
fn apsk16_two_thirds_at_14_db_roll_off_0_35_with_carrier_and_clock_offsets() {
    // 5 MHz, guard 1/32, roll-off 0.35, the frame of OFDM 16-QAM: 40/11 Msym/s and 672 units.
    let p = TdmParams {
        guard: Guard::G32,
        rolloff: RollOff::R35,
        ofdm_qam16: true,
        ..params(Modulation::Apsk16, 10)
    };
    let mut t = Trip::new(p, 2.2);
    (t.esn0_db, t.cfo_hz, t.ppm) = (Some(14.0), 2.5e3, 8.0);
    let d = check_offsets(&t, 1500, 10.0);
    assert_eq!(d.int(DvbshFields::TDM_FRAME_WIDTH_CU.key), 672);
}

#[test]
fn the_largest_carrier_and_clock_offsets_are_followed() {
    // 90 kHz is 1/43 of the symbol rate; the slot search takes 1/40.
    for (cfo, ppm) in [(90e3, 100.0), (-90e3, -100.0)] {
        let mut t = Trip::new(params(Modulation::Psk8, 8), 1.3);
        t.settings = set_rate();
        (t.esn0_db, t.cfo_hz, t.ppm) = (Some(10.0), cfo, ppm);
        check_offsets(&t, 1500, 10.0);
    }
}

/// Runs the signal of `p` at `esn0_db` with each noise seed and checks that every run decodes every code word.
fn every_seed_decodes(p: TdmParams, esn0_db: f64) {
    let mut t = Trip::new(p, 1.2);
    t.settings = set_rate();
    let x = clean(&t);
    for seed in [11u64, 12, 13] {
        let mut y = x.clone();
        Awgn::new(1.0, esn0_db, p.symbol_rate(), RATE, seed).process(&mut y);
        let d = run_signal(&t, &y);
        let ok = d.int(DvbshFields::CODEWORDS_OK.key);
        assert!(
            ok > 100 && d.int(DvbshFields::CODEWORDS_BAD.key) == 0 && d.wrong == 0 && d.lost == 0,
            "seed {seed}: {}",
            d.summary()
        );
    }
}

#[test]
fn qpsk_one_third_decodes_at_1_db_with_every_seed() {
    every_seed_decodes(params(Modulation::Qpsk, 4), 1.0);
}

#[test]
fn psk8_one_half_decodes_at_6_5_db_with_every_seed() {
    every_seed_decodes(params(Modulation::Psk8, 8), 6.5);
}

#[test]
fn apsk16_two_thirds_decodes_at_11_5_db_with_every_seed() {
    every_seed_decodes(params(Modulation::Apsk16, 10), 11.5);
}

#[test]
fn the_signalling_field_decodes_at_minus_2_db_and_no_wrong_packet_passes() {
    // QPSK 2/3 at -2 dB: the signalling field (rate 1/5) decodes, the code words do not.
    let mut t = Trip::new(params(Modulation::Qpsk, 10), 1.2);
    t.settings = set_rate();
    t.esn0_db = Some(-2.0);
    let d = run(&t);
    assert_eq!(d.int(DvbshFields::TDM_FRAME_WIDTH_CU.key), 784, "{}", d.summary());
    assert_eq!(d.text(DvbshFields::RATE_HP.key), "2/3");
    assert!(d.good == 0 && d.wrong == 0 && d.flagged > 0, "{}", d.summary());
    assert!(d.int(DvbshFields::LOCK.key) == 3, "{}", d.summary());
    // 8PSK 2/3 complementary at 2 dB: every packet is flagged.
    let mut t = Trip::new(params(Modulation::Psk8, 11), 1.2);
    t.settings = set_rate();
    t.esn0_db = Some(2.0);
    let d = run(&t);
    assert!(d.good == 0 && d.wrong == 0 && d.flagged > 0, "{}", d.summary());
    // At -8 dB no slot is found and nothing is read.
    let mut t = Trip::new(params(Modulation::Qpsk, 10), 0.6);
    t.settings = set_rate();
    t.esn0_db = Some(-8.0);
    let d = run(&t);
    assert!(d.int(DvbshFields::LOCK.key) == 0 && d.flagged == 0 && d.wrong == 0, "{}", d.summary());
    assert_eq!(d.text(DvbshFields::TDM_FRAME_WIDTH_CU.key), "");
}

#[test]
fn a_short_blockage_with_the_late_interleaver_loses_no_code_word() {
    let mut p = params(Modulation::Qpsk, 4);
    p.ti = sdrkit_algo_fec::branchdelay::ShTimeInterleaver {
        common_multiplier: 2,
        late_taps: 24,
        slices: 2,
        slice_distance: 1,
        non_late_increment: 1,
    };
    let mut t = Trip::new(p, 2.4);
    t.settings = set_rate();
    (t.esn0_db, t.off) = (Some(6.0), Some((1.2, 0.05)));
    let d = run(&t);
    assert_eq!(d.text(DvbshFields::INTERLEAVER.key), "2 / 24 / 2 / 1 / 1");
    assert!(
        d.int(DvbshFields::CODEWORDS_BAD.key) == 0 && d.wrong == 0 && d.lost == 0 && d.good > 1000,
        "{}",
        d.summary()
    );
    // The slots of the 50 ms without signal were erased, and the grids ran on.
    let erased = d.int(DvbshFields::TDM_SLOTS_ERASED.key);
    assert!((60..=120).contains(&erased), "{erased} slots erased");
}

#[test]
fn the_automatic_waveform_and_symbol_rate_find_the_tdm_form() {
    let mut t = Trip::new(params(Modulation::Psk8, 8), 3.2);
    t.settings.clear();
    (t.esn0_db, t.cfo_hz, t.ppm) = (Some(18.0), 6e3, 8.0);
    let d = check_offsets(&t, 1500, 5.0);
    assert_eq!(d.text(DvbshFields::WAVEFORM.key), "1");
    // The OFDM form has its turn first: 0.3 s on each of the four widths that 8 Msps allows.
    let first = d.real(DvbshFields::FIRST_PACKET_S.key).unwrap();
    assert!((1.2..2.4).contains(&first), "first packet at {first} s");
}

#[test]
fn loss_of_the_signal_returns_to_searching_and_the_signal_locks_again() {
    let mut t = Trip::new(params(Modulation::Qpsk, 4), 1.0);
    t.settings = set_rate();
    t.esn0_db = Some(10.0);
    let x = signal(&t);
    let mut rx = receiver(&config(&t.settings)).unwrap();
    rx.configure(&InputInfo::new(RATE, 2177.5e6, 2177.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut feed = |rx: &mut StackGraph, x: &[Complex32]| {
        for c in x.chunks(20_000) {
            rx.feed(c, &mut out);
            out.clear();
        }
    };
    let state = |rx: &StackGraph| {
        let r = Owner::results(rx);
        let int = |k: &str| match value(r, k) {
            Some(Value::I64(v)) => *v,
            _ => 0,
        };
        (
            r.status().lock_state,
            int(DvbshFields::LOCK.key),
            int(DvbshFields::CODEWORDS_OK.key),
            int(DvbshFields::CODEWORDS_BAD.key),
        )
    };
    feed(&mut rx, &x);
    let before = state(&rx);
    assert!(
        before.0 == LockState::Locked && before.1 == 4 && before.2 > 50,
        "{before:?}"
    );
    // Noise alone for 2.6 s: longer than the 2 s that the grids run on.
    let mut noise = vec![Complex32::default(); (0.2 * RATE) as usize];
    Awgn::new(1.0, 10.0, RS, RATE, 5).process(&mut noise);
    for _ in 0..13 {
        feed(&mut rx, &noise);
    }
    let lost = state(&rx);
    assert!(lost.0 == LockState::Searching && lost.1 == 0, "{lost:?}");
    feed(&mut rx, &x);
    let again = state(&rx);
    assert!(
        again.0 == LockState::Locked && again.1 == 4 && again.2 > lost.2 + 50,
        "{again:?} after {lost:?}"
    );
}

#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_share_of_a_core_of_each_dvbsh_benchmark() {
    for b in sdrkit_stack_dvbt::dvbsh::benchmarks() {
        let r = (b.run)(4.0);
        eprintln!(
            "{}: {:.3} of a core, {:.1} Msps",
            b.name,
            r.cpu_fraction_of_realtime,
            r.items_per_second / 1e6
        );
    }
}

#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_lowest_es_n0_at_which_8psk_one_half_decodes_with_every_seed() {
    let p = params(Modulation::Psk8, 8);
    let mut t = Trip::new(p, 1.2);
    t.settings = set_rate();
    let x = clean(&t);
    let mut db = 7.0;
    loop {
        let mut all = true;
        for seed in [11u64, 12, 13] {
            let mut y = x.clone();
            Awgn::new(1.0, db, p.symbol_rate(), RATE, seed).process(&mut y);
            let d = run_signal(&t, &y);
            let ok = d.int(DvbshFields::CODEWORDS_OK.key);
            let good = ok > 100 && d.int(DvbshFields::CODEWORDS_BAD.key) == 0 && d.wrong == 0 && d.lost == 0;
            eprintln!(
                "{db:.2} dB seed {seed}: {} iterations {:?}",
                d.summary(),
                d.real(DvbshFields::TURBO_ITERATIONS.key)
            );
            all &= good;
        }
        if !all || db < 3.0 {
            break;
        }
        db -= 0.25;
    }
}

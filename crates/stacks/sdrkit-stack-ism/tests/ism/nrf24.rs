//! `nrf24`: the CRC and the sniffer, the generator round trip, random bursts, the length check, the channel request, refused
//! values and the events.

use crate::util::set;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_EBN0_DB, GEN_NOISE, GEN_OFFSET_HZ};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{
    Choice, Complex32, HostRequest, InputInfo, Output, Receiver, StackConfig, Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_graph::{Component, StackGraph};
use sdrkit_stack_ism::family::hex;
use sdrkit_stack_ism::nrf24::{self, Demod, NrfBurst, NrfTables, NRF24, RATES};
use sdrkit_stack_ism::typed::{NrfFields, NRF_ADDRESS};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;

const RATE: f64 = 8e6;

fn feed(h: &Harness, x: &[Complex32]) -> StackGraph {
    let mut r = nrf24::receiver(&StackConfig::new(h.values().unwrap())).unwrap();
    r.configure(&InputInfo::new(RATE, 2402e6, 2402e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        out.clear();
    }
    r.finish(&mut out);
    r
}

fn store(r: &StackGraph) -> &nrf24::Store {
    r.graph()
        .component_as::<NrfTables>("tables")
        .unwrap()
        .store()
}

fn h(rate: &str, off: f64) -> Harness {
    Harness::new(&NRF24)
        .rate(RATE)
        .set(&nrf24::GEN_RATE, nrf24::GenRate::from_key(rate).unwrap())
        .set(&GEN_NOISE, NoiseKind::EbN0)
        .set(&GEN_EBN0_DB, 15.0)
        .set(&GEN_OFFSET_HZ, off)
        .seed(3)
}

#[test]
fn crc_and_sniff_round_trip() {
    let mut bits = Vec::new();
    unpack(b"123456789", BitOrder::Msb, &mut bits);
    assert_eq!(CRC16_IBM_3740.bits(&bits), 0x29B1);
    for (a, pid, na, p, c16) in nrf24::test_frames() {
        let mut b = vec![1, 0, 1, 0];
        b.extend(nrf24::frame_bits(&a, pid, na, &p, c16));
        b.extend([0, 1, 1, 0, 1]);
        let f = nrf24::sniff(&b);
        let g = f
            .iter()
            .find(|x| x.addr == a)
            .unwrap_or_else(|| panic!("{} {:?}", hex(&a), f));
        assert_eq!(
            (g.pid, g.no_ack, &g.payload, g.crc_bits),
            (pid, na, &p, if c16 { 16 } else { 8 })
        );
    }
}

#[test]
fn generator_round_trip() {
    for (rate, rates, off) in [
        ("1000", vec![1e6], 100e3),
        ("2000", vec![2e6], -100e3),
        ("250", vec![250e3], 100e3),
        ("all", vec![1e6, 2e6, 250e3], -100e3),
    ] {
        let h = h(rate, off).seconds(0.25);
        let r = feed(&h, &h.generate().unwrap());
        let s = store(&r);
        for (a, _, _, p, c16) in nrf24::test_frames() {
            if c16 {
                assert!(
                    s.frames.iter().any(|f| f.addr == a && f.payload == p),
                    "{} at {rate}: {:?}",
                    hex(&a),
                    s.frames.iter().map(|f| hex(&f.addr)).collect::<Vec<_>>()
                );
            }
        }
        // CRC-8 frames appear from their third sight.
        assert!(s.frames.iter().any(|f| f.crc_bits == 8), "{rate}");
        for f in s.frames.iter() {
            assert!(rates.contains(&f.rate), "rate {}", f.rate);
        }
    }
}

#[test]
fn no_false_frames_from_random_bursts() {
    let h = h("all", 0.0).set(&nrf24::GEN_RANDOM, true).seconds(2.0);
    let r = feed(&h, &h.generate().unwrap());
    let f = NrfFields::read(r.results().fields()).expect("the nRF24 fields");
    assert_eq!(f.crc16_frames, 0);
    assert_eq!(f.crc8_frames, 0);
    assert!(f.bad_bursts > 0);
}

/// A burst whose length does not fit the rate it is read at is refused by the length check.
#[test]
fn wrong_rate_rejected_by_length_check() {
    let (a, pid, na, p, c16) = nrf24::test_frames().remove(0);
    let mut d = Demod::default();
    for (ri, &(br, _, _, _)) in RATES.iter().enumerate() {
        let bits = nrf24::frame_bits(&a, pid, na, &p, c16);
        let levels: Vec<f32> = bits
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        let pre = 400;
        let mut s = vec![Complex32::default(); pre];
        let mut m = nrf24::modulator(br, RATE).unwrap();
        m.push(&levels, &mut s);
        m.flush(&mut s);
        let end = s.len();
        s.extend(vec![Complex32::default(); end + 400]);
        let burst = |end: usize| NrfBurst {
            samples: s.clone().into(),
            rate: RATE,
            start: pre,
            end,
            snr_db: 30.0,
        };
        let f = d.burst_frames(&burst(end));
        assert!(
            f.iter()
                .any(|x| x.addr == a && x.payload == p && x.rate == br),
            "rate {ri}: {f:?}"
        );
        for k in [2.0, 0.5] {
            let e = (pre + ((end - pre) as f64 * k) as usize).min(s.len());
            let f = d.burst_frames(&burst(e));
            assert!(
                !f.iter().any(|x| x.rate == br),
                "rate {ri}, length x{k}: {f:?}"
            );
        }
    }
}

#[test]
fn a_new_channel_asks_the_host_to_tune() {
    let mut r = nrf24::receiver(&StackConfig::new(Harness::new(&NRF24).values().unwrap())).unwrap();
    r.configure(&InputInfo::new(RATE, 2402e6, 2402e6).recording())
        .unwrap();
    let mut out = Output::default();
    r.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert!(out.requests.is_empty());
    set(&mut r, "channel", Value::I64(76)).unwrap();
    out.clear();
    r.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert!(matches!(
        out.requests.as_slice(),
        [HostRequest::Retune { tuned_hz, .. }] if *tuned_hz == 2476e6
    ));
}

/// A value of the wrong kind or out of range is refused.
#[test]
fn a_bad_setting_value_is_refused() {
    let schema = NrfTables::default().info().settings;
    let id = schema.id("channel").unwrap();
    assert!(schema.check(id, &Value::Text("two".into())).is_err());
    assert!(schema.check(id, &Value::I64(200)).is_err());
    let mut r = nrf24::receiver(&StackConfig::new(Harness::new(&NRF24).values().unwrap())).unwrap();
    assert!(set(&mut r, "channel", Value::I64(200)).is_err());
    let id = r.schema().id("channel").unwrap();
    assert!(owner::apply(&mut r, id, &Value::Text("two".into())).is_err());
}

/// The events of new addresses carry the frequency of the channel.
#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let run = Harness::new(&NRF24).rate(RATE).seconds(0.25).run().unwrap();
    assert!(!run.events.is_empty());
    for e in &run.events {
        assert!(e.is(&NRF_ADDRESS), "{}", e.kind());
        assert!(
            (e.freq_hz - 2402e6).abs() < 400e3,
            "{} at {} Hz",
            e.kind(),
            e.freq_hz
        );
    }
}

/// A change of frequency keeps the addresses, the frame log and the counts, and drops the lock on the present signal.
#[test]
fn a_change_of_frequency_keeps_the_addresses_and_drops_the_lock() {
    let h = Harness::new(&NRF24).rate(RATE).seconds(0.25);
    let mut r = feed(&h, &h.generate().unwrap());
    let addresses = crate::util::table(r.results(), "addresses").len();
    let frames = crate::util::number(r.results(), "frames");
    assert!(addresses >= 1 && frames > 0.0);
    assert_eq!(
        r.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
    r.forget();
    assert_eq!(
        crate::util::table(r.results(), "addresses").len(),
        addresses
    );
    assert_eq!(crate::util::number(r.results(), "frames"), frames);
    assert_ne!(
        r.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
}

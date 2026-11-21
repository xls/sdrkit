//! `thread`: the test network on the air with carrier and clock errors, at 5 Msps and in noise, the `zigbee` stack on the
//! same signal, the channel scan and the channel setting.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Event, HostRequest, InputInfo, Output, Receiver, Results, StackDescriptor, Station,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_ism::thread::{self, THREAD};
use sdrkit_stack_ism::typed::{
    PanDeviceRow, ThreadFrameRow, ThreadNetworkRow, ZigbeeFrameRow, ZigbeeNetworkRow,
    THREAD_DEVICE, THREAD_NETWORK,
};
use sdrkit_stack_ism::zigbee::{self, channel_hz, ZIGBEE};

fn rows<'a>(r: &'a Results, t: &str) -> Vec<&'a sdrkit_core::Record> {
    crate::util::table(r, t)
        .rows_newest_first()
        .map(|(_, c)| c)
        .collect()
}

fn field(r: &Results, k: &str) -> f64 {
    crate::util::number(r, k)
}

/// The test signal of `thread` at `rate` with these settings: the network is 1 MHz below the centre of the samples.
fn signal(rate: f64, seconds: f64, s: &[CaseSetting]) -> Vec<Complex32> {
    Harness::new(&THREAD)
        .rate(rate)
        .seconds(seconds)
        .with_settings(s)
        .generate()
        .unwrap()
}

/// Feeds `x`, a capture 1 MHz above channel 15, to a receiver of `d`. Returns the receiver and what it sent.
fn receive(
    d: &'static StackDescriptor,
    x: &[Complex32],
    rate: f64,
) -> (Box<dyn Receiver>, Vec<Event>, Vec<HostRequest>) {
    let mut rx = Harness::new(d).receiver().unwrap();
    let tuned = channel_hz(15);
    rx.configure(&InputInfo::new(rate, tuned + 1e6, tuned).recording())
        .unwrap();
    let mut out = Output::default();
    let (mut events, mut requests) = (Vec::new(), Vec::new());
    for c in x.chunks(65536) {
        rx.feed(c, &mut out);
        events.append(&mut out.events);
        requests.append(&mut out.requests);
        out.clear();
    }
    (rx, events, requests)
}

/// The network of the test signal is there with its name, and the encrypted and MLE rows.
fn check_network(r: &Results, what: &str) {
    assert!(
        field(r, "frames_ok") >= 20.0,
        "{what}: {}",
        field(r, "frames_ok")
    );
    assert_eq!(field(r, "channel"), 15.0, "{what}");
    assert!(field(r, "encrypted_frames") >= 6.0, "{what}");
    assert!(
        field(r, "ch15") >= 20.0 && field(r, "ch11") == 0.0,
        "{what}"
    );
    let nets = rows(r, "networks");
    let n = nets
        .iter()
        .find(|c| c.at(&ThreadNetworkRow::PAN) == Some("0x7A31"))
        .unwrap_or_else(|| panic!("{what}: {nets:?}"));
    assert_eq!(
        n.at(&ThreadNetworkRow::NETWORK_NAME),
        Some("HomeThread"),
        "{what}"
    );
    assert_eq!(
        n.at(&ThreadNetworkRow::EXT_PAN),
        Some("11:22:33:44:55:66:77:88")
    );
    assert_eq!(n.at(&ThreadNetworkRow::JOINING_OPEN), Some(true));
    assert_eq!(n.at(&ThreadNetworkRow::VERSION), Some(2));
    assert_eq!(n.at(&ThreadNetworkRow::THREAD_SEEN), Some(true));
    assert_eq!(n.at(&ThreadNetworkRow::CH), Some(15));
    assert_eq!(n.at(&ThreadNetworkRow::COORDINATOR), Some("0x5000"));
    let frames = rows(r, "frames");
    let details = |want: &str| {
        frames.iter().any(|c| {
            c.at(&ThreadFrameRow::DETAILS)
                .is_some_and(|d| d.contains(want))
        })
    };
    assert!(
        details("link-layer AES-CCM encrypted (level 5, key index 1"),
        "{what}"
    );
    assert!(details("MLE Discovery Request (not secured)"), "{what}");
    assert!(details("Thread beacon, network \"HomeThread\""), "{what}");
    assert!(frames
        .iter()
        .any(|c| c.at(&ThreadFrameRow::ENCRYPTED) == Some(true)
            && c.at(&ThreadFrameRow::ACK_REQUEST) == Some(true)));
    let devs = rows(r, "devices");
    assert_eq!(devs.len(), 3, "{what}: {devs:?}");
    let role = |short: &str| {
        devs.iter()
            .find(|c| c.at(&PanDeviceRow::SHORT) == Some(short))
            .and_then(|c| c.at(&PanDeviceRow::ROLE))
    };
    assert_eq!(role("0x5000"), Some("coordinator"), "{what}");
    assert_eq!(role("0x5401"), Some("device"), "{what}");
    assert!(devs
        .iter()
        .any(|c| c.at(&PanDeviceRow::EXTENDED) == Some("01:23:45:67:89:AB:CD:EF")));
}

#[test]
fn the_test_network_is_read_clean_and_with_carrier_and_clock_errors() {
    let snr = |db: f64| {
        vec![
            CaseSetting::new(&GEN_NOISE, NoiseKind::Snr),
            CaseSetting::new(&GEN_SNR_DB, db),
        ]
    };
    let with = |db: f64, off: f64, ppm: f64| {
        let mut s = snr(db);
        s.push(CaseSetting::new(&GEN_OFFSET_HZ, off));
        s.push(CaseSetting::new(&GEN_CLOCK_PPM, ppm));
        s
    };
    let cases: [(&str, f64, f64, Vec<CaseSetting>); 5] = [
        ("clean", 4e6, 0.5, snr(30.0)),
        ("+100 kHz", 4e6, 0.5, with(25.0, 100e3, 0.0)),
        ("-100 kHz, +40 ppm", 4e6, 0.5, with(25.0, -100e3, 40.0)),
        ("10 dB", 4e6, 1.0, with(10.0, 30e3, -20.0)),
        ("5 Msps", 5e6, 0.5, with(25.0, 50e3, 0.0)),
    ];
    for (what, rate, seconds, s) in cases {
        let x = signal(rate, seconds, &s);
        let (rx, events, requests) = receive(&THREAD, &x, rate);
        check_network(rx.results(), what);
        assert_eq!(
            events.iter().filter(|e| e.is(&THREAD_NETWORK)).count(),
            1,
            "{what}"
        );
        assert_eq!(
            events.iter().filter(|e| e.is(&THREAD_DEVICE)).count(),
            3,
            "{what}"
        );
        assert!(crate::util::declared(rx.results().decl(), &events));
        // A recording gives no retune request.
        assert!(requests.is_empty(), "{what}");
    }
}

#[test]
fn noise_gives_no_network_and_no_device() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(8);
    let x: Vec<Complex32> = (0..2_000_000).map(|_| rng.complex_normal(0.1)).collect();
    let (rx, events, _) = receive(&THREAD, &x, 4e6);
    let r = rx.results();
    assert!(rows(r, "networks").is_empty() && rows(r, "devices").is_empty());
    assert!(events.is_empty());
}

#[test]
fn the_zigbee_stack_shows_the_pan_without_a_network_name() {
    let x = signal(4e6, 0.5, &[]);
    let (rx, _, _) = receive(&ZIGBEE, &x, 4e6);
    let r = rx.results();
    let nets = rows(r, "networks");
    assert!(nets
        .iter()
        .any(|c| c.at(&ZigbeeNetworkRow::PAN) == Some("0x7A31")));
    assert!(r.decl().table_id("networks").is_some());
    let frames = rows(r, "frames");
    assert!(!frames.is_empty());
    assert!(!frames.iter().any(|c| c
        .at(&ZigbeeFrameRow::DETAILS)
        .is_some_and(|d| d.contains("MLE") || d.contains("HomeThread"))));
}

#[test]
fn the_scan_steps_through_the_channels_of_a_live_radio() {
    // Quiet samples for 35 s of signal: with a dwell of 2 s the stack asks for each channel in turn, and starts again.
    let scan = |live: bool| -> Vec<f64> {
        let mut h = Harness::new(&THREAD)
            .set(&thread::SCAN, true)
            .set(&zigbee::DWELL, 2.0);
        if live {
            h = h.live();
        }
        let mut rx = h.receiver().unwrap();
        let info = InputInfo::new(4e6, 2406e6, 2405e6);
        rx.configure(&if live { info } else { info.recording() })
            .unwrap();
        let quiet = vec![Complex32::default(); 1_000_000];
        let mut out = Output::default();
        let mut asked = Vec::new();
        for _ in 0..140 {
            rx.feed(&quiet, &mut out);
            for r in out.requests.drain(..) {
                if let HostRequest::Retune { tuned_hz, .. } = r {
                    asked.push(tuned_hz);
                }
            }
            out.clear();
        }
        asked
    };
    let asked = scan(true);
    let want: Vec<f64> = (0..17).map(|k| 2405e6 + 5e6 * f64::from(k % 16)).collect();
    assert_eq!(asked[..17], want[..], "{asked:?}");
    assert_eq!(asked.len(), 18);
    assert!(scan(false).is_empty());
}

#[test]
fn the_channel_setting_asks_for_its_frequency_whatever_the_station() {
    for locator in ["QF56", "JO62", "FN30", "RF70"] {
        let mut rx = Harness::new(&THREAD)
            .station(Station {
                locator: locator.into(),
                ..Station::default()
            })
            .receiver()
            .unwrap();
        rx.configure(&InputInfo::new(4e6, 2406e6, 2405e6).recording())
            .unwrap();
        let mut out = Output::default();
        rx.feed(&vec![Complex32::default(); 100_000], &mut out);
        // The default channel 11 is where the stack starts: nothing to request.
        assert!(out.requests.is_empty(), "{locator}");
        crate::util::set(rx.as_mut(), "channel", sdrkit_core::Value::I64(20)).unwrap();
        rx.feed(&vec![Complex32::default(); 100_000], &mut out);
        assert!(
            out.requests
                .iter()
                .any(|r| matches!(r, HostRequest::Retune { tuned_hz, .. } if *tuned_hz == 2450e6)),
            "{locator}"
        );
        // The channels 11 to 26 are constants of IEEE 802.15.4: the stack has no region setting.
        assert!(Owner::schema(&*rx).id("region").is_err());
    }
    assert!(crate::util::set(
        Harness::new(&THREAD).receiver().unwrap().as_mut(),
        "channel",
        sdrkit_core::Value::I64(27)
    )
    .is_err());
}

#[test]
fn the_network_name_of_the_test_signal_is_a_setting_and_survives_a_restart() {
    let x = signal(
        4e6,
        0.3,
        &[CaseSetting::new(&thread::GEN_NETWORK_NAME, "sdrkit lab")],
    );
    let (rx, _, _) = receive(&THREAD, &x, 4e6);
    let name = |r: &Results| {
        rows(r, "networks")
            .first()
            .and_then(|c| c.at(&ThreadNetworkRow::NETWORK_NAME))
            .map(str::to_owned)
    };
    assert_eq!(name(rx.results()).as_deref(), Some("sdrkit lab"));
    let state = rx.save_state().unwrap();
    let (mut again, _, _) = receive(&THREAD, &[], 4e6);
    again.restore_state(&state).unwrap();
    assert_eq!(name(again.results()).as_deref(), Some("sdrkit lab"));
    crate::util::clear(again.as_mut());
    assert!(rows(again.results(), "networks").is_empty());
}

/// A change of frequency keeps the networks, the devices and the counts, and drops the level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_networks_and_drops_the_level() {
    let x = signal(4e6, 0.5, &[]);
    let (mut rx, _, _) = receive(&THREAD, &x, 4e6);
    let networks = rows(rx.results(), "networks").len();
    let frames = field(rx.results(), "frames_ok");
    assert!(networks >= 1 && frames > 0.0);
    assert!(crate::util::has(rx.results(), "level_dbfs"));
    rx.forget();
    assert_eq!(rows(rx.results(), "networks").len(), networks);
    assert_eq!(field(rx.results(), "frames_ok"), frames);
    assert!(!crate::util::has(rx.results(), "level_dbfs"));
    assert!(!crate::util::has(rx.results(), "snr_db"));
    assert_eq!(rx.results().status().level_dbfs, None);
}

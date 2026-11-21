//! VDL Mode 4: the link layer, the generator through the receiver at many rates, local decoding at the station, and bad rates.

use crate::common::{self, num, rows, text, Row};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, StackConfig, Station};
use sdrkit_stack_avdata::typed::{VDL4_CHANNELS, VDL4_MESSAGES};
use sdrkit_stack_avdata::vdl4::generator::AMPLITUDE;
use sdrkit_stack_avdata::vdl4::{self, GenParams, Vdl4Generator};
use sdrkit_util_geo::Position;

/// The six channels of the test scene, MHz, around the centre 136.85 MHz.
const SCENE: [f64; 6] = [136.725, 136.775, 136.825, 136.875, 136.900, 136.975];

fn rx(rate: f64) -> (Box<dyn Receiver>, bool) {
    let list: Vec<String> = SCENE.iter().map(|f| format!("{f:.3}")).collect();
    let list = list.join(" ");
    let cfg = StackConfig::new(crate::common::values(
        &vdl4::DESCRIPTOR,
        &[("channels", list.as_str())],
    ));
    let mut rx: Box<dyn Receiver> = Box::new(vdl4::receiver(&cfg).unwrap());
    let ok = rx
        .configure(&InputInfo::new(rate, 136.85e6, 136.85e6).recording())
        .is_ok();
    (rx, ok)
}

fn signal(rate: f64, offset: f64, secs: f64, snr: f64) -> Vec<Complex32> {
    let mut g = Vdl4Generator::new(GenParams {
        rate_hz: rate,
        centre_hz: 136.85e6,
        channels: SCENE.iter().map(|f| (f * 1e6).round()).collect(),
        ..Default::default()
    });
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    for c in x.chunks_mut(65_536) {
        g.generate(c);
    }
    if offset != 0.0 {
        common::offset(&mut x, rate, offset);
    }
    common::noise(&mut x, AMPLITUDE * AMPLITUDE, snr, 25e3, rate, 4);
    x
}

fn messages(r: &dyn Receiver) -> Vec<Row> {
    rows(r.results(), &VDL4_MESSAGES)
}

#[test]
fn a_dls_burst_through_the_link_layer() {
    let mut info = vec![0x10, 0x00];
    info.extend(b"HELLO VDL4");
    let f = vdl4::reader::write(true, 1, 0x4CA2B1, 3, 3, &info);
    let lb = vdl4::generator::line_bits(&f);
    assert!(lb.len() > 80);
    let mut rd = vdl4::reader::Reader::default();
    let m = rd
        .read(&vdl4::Burst {
            frame: f,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(m.msg_id, 3);
    assert!(m.dls_text.contains("HELLO"));
}

#[test]
fn rates_and_offsets() {
    for &rate in &[
        0.1e6, 0.25e6, 0.5e6, 1e6, 2e6, 2.4e6, 3e6, 4e6, 5e6, 6e6, 8e6, 10e6, 20e6,
    ] {
        let (_, ok) = rx(rate);
        assert_eq!(ok, rate >= 400e3, "{rate}");
        if rate < 400e3 {
            let (r, _) = rx(rate);
            assert!(
                r.results().status().summary.contains("400 kHz"),
                "{}",
                r.results().status().summary
            );
            continue;
        }
        let offs: &[f64] = if rate <= 6e6 {
            &[0.0, 2500.0, -2500.0]
        } else {
            &[0.0]
        };
        for &off in offs {
            let x = signal(rate, off, 1.8, 30.0);
            let (mut r, _) = rx(rate);
            common::run(r.as_mut(), &x, 65_536);
            let frames = common::field(r.results(), "frames");
            if rate >= 1e6 {
                assert!(frames >= 2.0, "{rate} {off}: {frames}");
            }
        }
    }
}

#[test]
fn bad_rates_do_not_crash() {
    for &rate in &[0.0, -5.0, 1.0, 0.001, 1e12, f64::NAN] {
        let (mut r, ok) = rx(rate);
        assert!(!ok || rate >= 400e3);
        let mut out = Output::default();
        r.feed(&[], &mut out);
        r.feed(&[Complex32::new(0.1, 0.0); 1000], &mut out);
    }
}

#[test]
fn any_cut_returns_the_same_bursts() {
    let rate = 1e6;
    let x = signal(rate, 0.0, 1.6, 25.0);
    let mut got = Vec::new();
    for chunk in [65_536usize, 1000, 7] {
        let (mut r, _) = rx(rate);
        common::run(r.as_mut(), &x, chunk);
        got.push(
            messages(r.as_ref())
                .iter()
                .map(|m| {
                    (
                        (num(m, "signal_s") * 1e4).round() as i64,
                        text(m, "address").to_owned(),
                        text(m, "summary").to_owned(),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    assert!(got[0].len() >= 2, "{:?}", got[0]);
    assert_eq!(got[0], got[1]);
    assert_eq!(got[0], got[2]);
}

#[test]
fn one_sync_burst_decodes_at_the_receiving_station() {
    // Without the station, one even report produces no position. With it, the report is decoded
    // locally.
    let x = signal(1e6, 0.0, 0.5, 30.0);
    for (station, want) in [(None, false), (Some(Position::new(51.45, -0.45)), true)] {
        let (mut r, _) = rx(1e6);
        if let Some(p) = station {
            r.set_station(&Station::at(p));
        }
        common::run(r.as_mut(), &x, 65_536);
        let ms = messages(r.as_ref());
        let sync = ms
            .iter()
            .find(|m| text(m, "message") == "SYNC")
            .expect("a SYNC burst");
        assert_eq!(common::position(sync, "position").is_some(), want);
        if want {
            let (lat, lon) = common::position(sync, "position").unwrap();
            assert!(
                (lat - 51.4).abs() < 0.01 && (lon + 0.5).abs() < 0.01,
                "{lat} {lon}"
            );
        }
    }
}

#[test]
fn a_change_of_frequency_keeps_the_lists_and_forgets_the_channels() {
    let shape = sdrkit_stack_avdata::typed::shape("vdl4");
    let kept = shape.kept_keys().unwrap();
    assert!(kept.iter().any(|k| k.contains("messages")), "{kept:?}");
    assert!(!kept.iter().any(|k| k.contains("channels")), "{kept:?}");
    let rate = 1e6;
    let x = signal(rate, 0.0, 1.8, 30.0);
    let (mut r, ok) = rx(rate);
    assert!(ok);
    common::run(r.as_mut(), &x, 65_536);
    let n = messages(r.as_ref()).len();
    let frames = common::field(r.results(), "frames");
    assert!(n > 0 && frames >= 2.0, "{n} {frames}");
    assert!(!rows(r.results(), &VDL4_CHANNELS).is_empty());
    r.forget();
    assert_eq!(messages(r.as_ref()).len(), n);
    assert_eq!(common::field(r.results(), "frames"), frames);
    assert!(rows(r.results(), &VDL4_CHANNELS).is_empty());
    assert_eq!(common::field(r.results(), "bad"), 0.0);
}

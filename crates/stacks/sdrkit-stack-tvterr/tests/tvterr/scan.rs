//! The channel scan driven over generated channels: an ATSC channel, an ISDB-T channel and an empty one,
//! each through its receiver, judged by `ChannelScan`.

use super::common::{self, chain, impair};
use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver};
use sdrkit_stack_tvterr::band::{AvgSpectrum, BandFft};
use sdrkit_stack_tvterr::dtmb::pn::Header;
use sdrkit_stack_tvterr::isdbt::results::{CodeRate, LayerName, Modulation};
use sdrkit_stack_tvterr::isdbt::Guard;
use sdrkit_stack_tvterr::scan::{
    ChannelDetail, ChannelScan, LayerSummary, Phase, ScanNote, ScanSettings, Standard,
};

fn receiver(id: &str) -> Box<dyn Receiver> {
    let d = sdrkit_stack_tvterr::descriptors()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("stack {id}"));
    common::receiver(d, &[])
}

/// Scans one channel holding `x`: spectrum, then the receiver's reports every 0.1 s of signal.
fn scan(
    standard: Standard,
    x: &[Complex32],
    rate: f64,
) -> sdrkit_stack_tvterr::scan::ChannelResult {
    scan_width(standard, x, rate, 6.0)
}

fn scan_width(
    standard: Standard,
    x: &[Complex32],
    rate: f64,
    width_mhz: f64,
) -> sdrkit_stack_tvterr::scan::ChannelResult {
    let mut s = ChannelScan::new(standard, 539e6, width_mhz, ScanSettings::default());
    let mut sp = AvgSpectrum::new(BandFft::N1024);
    sp.add_all(&x[..64 * 1024]);
    let level =
        10.0 * (x[..65536].iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / 65536.0).log10();
    s.on_spectrum(&sp.mean(), rate, level, 0.0);
    if s.phase() == Phase::Done {
        return s.result().unwrap().clone();
    }
    let mut rx = receiver(standard.stack_id());
    rx.configure(&InputInfo::new(rate, 539e6, 539e6).recording())
        .unwrap();
    let mut out = Output::default();
    let step = (0.1 * rate) as usize;
    for (k, c) in x.chunks(step).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        let t = (k + 1) as f64 * 0.1;
        s.on_results(Owner::results(rx.as_ref()), None, t);
        if s.phase() == Phase::Names {
            s.on_services(&["TEST [TV]".to_string()], true, t);
        }
        if s.phase() == Phase::Done {
            break;
        }
    }
    let end = x.len() as f64 / rate;
    s.tick(end + 10.0);
    s.result().expect("done").clone()
}

#[test]
fn scan_finds_atsc_isdbt_and_empty_channels() {
    // ATSC at 8 Msps.
    let x = common::generate(
        sdrkit_stack_tvterr::atsc::gen::AtscGenerator::new(Default::default()).unwrap(),
        1.0,
    );
    let x = impair(&x, 8e6, &chain(0.0, 0.0, Some(40.0), 5.38e6));
    let r = scan(Standard::Atsc, &x, 8e6);
    assert!(r.locked, "{r:?}");
    assert_eq!(r.standard, Some(Standard::Atsc));
    assert!(
        matches!(
            r.detail,
            ChannelDetail::Atsc {
                field_parity: 1 | 2
            }
        ),
        "{r:?}"
    );

    // ISDB-T at 8 Msps, mode 1 (short frames).
    use sdrkit_stack_tvterr::isdbt::gen::{config, layer, IsdbtGenParams, IsdbtGenerator};
    use sdrkit_stack_tvterr::isdbt::tmcc::LayerParams;
    let p = IsdbtGenParams {
        mode: 1,
        config: config(
            true,
            [layer(1, 1, 0, 1), layer(3, 2, 0, 12), LayerParams::UNUSED],
        ),
        ..Default::default()
    };
    let x = impair(
        &common::generate(IsdbtGenerator::new(p).unwrap(), 1.2),
        8e6,
        &chain(0.0, 0.0, Some(30.0), 5.57e6),
    );
    let r = scan(Standard::Isdbt, &x, 8e6);
    assert!(r.locked, "{r:?}");
    assert_eq!(
        r.detail,
        ChannelDetail::Isdbt {
            mode: 1,
            guard: Some(Guard::G8),
            segments: 13,
            partial: true,
            layers: vec![
                LayerSummary {
                    layer: LayerName::A,
                    segments: 1,
                    modulation: Some(Modulation::Qpsk),
                    code_rate: Some(CodeRate::R23),
                },
                LayerSummary {
                    layer: LayerName::B,
                    segments: 12,
                    modulation: Some(Modulation::Qam64),
                    code_rate: Some(CodeRate::R34),
                },
            ],
        }
    );
    assert_eq!(r.services, vec!["TEST [TV]".to_string()]);

    // An empty channel.
    let x = common::noise(200_000, 1e-4, 9);
    let r = scan(Standard::Isdbt, &x, 8e6);
    assert_eq!(r.notes, vec![ScanNote::Empty]);
    assert!(!r.locked);
}

#[test]
fn scan_reports_dtmb_and_atsc3_channels() {
    // DTMB at 10 Msps, 8 MHz: frame headers are tracked. Listed as seen until the body decodes.
    let x = common::generate(
        sdrkit_stack_tvterr::dtmb::gen::DtmbGenerator::new(Default::default()).unwrap(),
        1.0,
    );
    let r = scan_width(Standard::Dtmb, &x, 10e6, 8.0);
    assert_eq!(r.width_mhz, 8.0);
    assert!(r.occupancy_db > 5.0, "{r:?}");
    assert_eq!(r.standard, Some(Standard::Dtmb), "{r:?}");
    if !r.locked {
        assert!(r.seen, "{r:?}");
        assert_eq!(r.notes, vec![ScanNote::DtmbFramesOnly]);
        assert!(
            matches!(
                r.detail,
                ChannelDetail::Dtmb {
                    header: Some(Header::Pn420 | Header::Pn595 | Header::Pn945)
                }
            ),
            "{r:?}"
        );
        assert!(r.snr_db.unwrap() > 15.0, "{r:?}");
    }

    // ATSC 3.0 at 8 Msps: bootstraps are found. Without a decoded frame the channel says so.
    let x = common::generate(
        sdrkit_stack_tvterr::atsc3::Atsc3Generator::new(
            sdrkit_stack_tvterr::atsc3::Atsc3GenParams {
                rate_hz: 8e6,
                ..Default::default()
            },
        )
        .unwrap(),
        1.0,
    );
    let r = scan(Standard::Atsc3, &x, 8e6);
    if !r.locked {
        assert_eq!(
            r.notes,
            vec![ScanNote::SignalNoLock, ScanNote::Atsc3BootstrapsOnly],
            "{r:?}"
        );
    }
}

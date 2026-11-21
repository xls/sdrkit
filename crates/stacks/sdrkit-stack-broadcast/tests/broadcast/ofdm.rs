//! The layouts of the DRM, HamDRM, HD Radio and CDR stacks are valid framed OFDM layouts of `sdrkit_algo_modem::ofdm`.

use sdrkit_algo_modem::ofdm::{FrameDemodulator, FrameLayout};

/// Every layout of the family is a valid shared OFDM layout, and its demodulator builds on the shared pieces.
#[test]
fn the_mode_layouts_are_shared_ofdm_layouts() {
    use sdrkit_stack_broadcast::{cdr, drm, hamdrm, hd};
    let mut all: Vec<(String, FrameLayout)> = Vec::new();
    for r in [
        drm::params::Robustness::A,
        drm::params::Robustness::B,
        drm::params::Robustness::C,
        drm::params::Robustness::D,
        drm::params::Robustness::E,
    ] {
        for occ in 0..6 {
            if let Some(l) = drm::params::layout(r, occ) {
                all.push((format!("DRM {r:?} {occ}"), l));
            }
        }
    }
    for m in hamdrm::HamMode::ALL {
        all.push((format!("HamDRM {m:?}"), hamdrm::layout(m)));
    }
    for b in [hd::l1::Band::Fm, hd::l1::Band::Am] {
        all.push((format!("HD {b:?}"), hd::l1::plan(b).layout));
    }
    for t in 1..=3 {
        for inner in [false, true] {
            all.push((
                format!("CDR {t} {inner}"),
                cdr::phy::layout(cdr::phy::TxMode(t), inner),
            ));
        }
    }
    for (name, l) in all {
        let o = l.ofdm().unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(o.used(), l.ncarriers(), "{name}");
        assert_eq!(o.period(), l.nsym, "{name}");
        assert_eq!(o.cp(0), l.cp0, "{name}");
        let data: usize = (0..l.nsym as u64).map(|s| o.data(s).len()).sum();
        assert_eq!(data, l.data_cells().len(), "{name}");
        assert!(FrameDemodulator::try_new(l).is_ok(), "{name}");
    }
}

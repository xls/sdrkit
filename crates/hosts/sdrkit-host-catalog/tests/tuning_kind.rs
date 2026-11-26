//! Every stack declares where it listens ([`sdrkit_core::Tuning::kind`]), so that a host knows whether a change to the
//! stack keeps the present frequency. A plain demodulation, where a stack declares one, has a filter width, and a pitch
//! for CW only.

use sdrkit_core::{DemodMode, PlainDemod, TuningKind};

fn demod_problem(id: &str, d: &PlainDemod) -> Option<String> {
    if !(d.width_hz > 0.0 && d.width_hz <= 250e3) {
        return Some(format!("{id}: the filter width {} Hz is not a channel width", d.width_hz));
    }
    match (d.mode, d.cw_pitch_hz) {
        (DemodMode::Cw, p) if !(200.0..=1500.0).contains(&p) => Some(format!("{id}: CW without a pitch")),
        (DemodMode::Cw, _) | (_, 0.0) => None,
        _ => Some(format!("{id}: a pitch for a demodulator other than CW")),
    }
}

#[test]
fn every_stack_declares_where_it_listens() {
    let stacks = sdrkit_host_catalog::all();
    assert!(stacks.len() > 200, "{} stacks", stacks.len());
    let mut problems = Vec::new();
    for s in &stacks {
        let t = &s.tuning;
        match t.kind {
            None => problems.push(format!("{}: no tuning kind", s.id)),
            Some(TuningKind::Fixed) if t.default_hz <= 0.0 => {
                problems.push(format!("{}: one frequency, but no default frequency", s.id));
            }
            Some(_) => {}
        }
        if let Some(d) = &t.demod {
            problems.extend(demod_problem(s.id, d));
        }
        for m in s.submodes {
            if let Some(d) = &m.demod {
                problems.extend(demod_problem(&format!("{}/{}", s.id, m.id), d));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The stacks that the app review named: CW, SSTV, DMR and the general stack listen anywhere; FT8 and WSPR on their
/// dials; ADS-B on 1090 MHz. CW is heard as CW at its pitch, airband as AM, marine VHF as NFM.
#[test]
fn the_named_stacks_listen_where_they_should() {
    let stacks = sdrkit_host_catalog::all();
    let of = |id: &str| stacks.iter().find(|s| s.id == id).map(|s| &s.tuning);
    for id in ["cw", "sstv", "dmr", "general"] {
        assert_eq!(of(id).and_then(|t| t.kind), Some(TuningKind::Free), "{id}");
    }
    for id in ["ft8", "wspr", "fm", "dvbt"] {
        assert_eq!(of(id).and_then(|t| t.kind), Some(TuningKind::Channels), "{id}");
    }
    assert_eq!(of("adsb").and_then(|t| t.kind), Some(TuningKind::Fixed));
    let mode = |id: &str| of(id).and_then(|t| t.demod).map(|d| d.mode);
    assert_eq!(mode("cw"), Some(DemodMode::Cw));
    assert_eq!(mode("airband"), Some(DemodMode::Am));
    assert_eq!(mode("marinevhf"), Some(DemodMode::Nfm));
    assert_eq!(mode("dvbt"), None);
}

//! Service following: the other frequencies of the ensemble (FIG 0/21) and of the ensembles that carry the
//! service (FIG 0/24), limited to the channels that the host permits.

use crate::channels::Channel;
use crate::ensemble::Ensemble;

/// An alternative to try when the signal is lost.
#[derive(Clone, Debug, PartialEq)]
pub struct Alternative {
    /// Frequency in Hz.
    pub hz: f64,
    /// The channel name.
    pub channel: &'static str,
    /// The ensemble there.
    pub eid: u16,
}

/// The alternatives for service `sid` on the channels of `table`, in the order to try them.
pub fn alternatives(db: &Ensemble, sid: u32, table: &[Channel]) -> Vec<Alternative> {
    let mut eids: Vec<u16> = Vec::new();
    if let Some(e) = db.eid {
        eids.push(e);
    }
    if let Some(s) = db.services.get(&sid) {
        for &e in &s.other_ensembles {
            if !eids.contains(&e) {
                eids.push(e);
            }
        }
    }
    let mut v: Vec<Alternative> = Vec::new();
    for &eid in &eids {
        for l in db.frequencies.values() {
            if l.id as u16 != eid || l.rm > 1 {
                continue;
            }
            for &(khz, _) in &l.freqs_khz {
                let hz = khz as f64 * 1e3;
                if let Some(c) = table.iter().find(|c| (c.hz - hz).abs() <= 50e3) {
                    if !v.iter().any(|a| a.hz == c.hz) {
                        v.push(Alternative {
                            hz: c.hz,
                            channel: c.name,
                            eid,
                        });
                    }
                }
            }
        }
    }
    v
}

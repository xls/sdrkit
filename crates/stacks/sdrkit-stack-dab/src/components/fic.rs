//! The FIC components: the FIC decoder (`dab.fic`), the FIG parser (`dab.figs`) and the ensemble database (`dab.ensemble`).
//!
//! Standard: EN 300 401 clauses 5.2, 6 and 8.

use super::items::{EnsembleItem, FibItem, FicSoft, FicStats, FigEvent};
use crate::ensemble::{Changes, Ensemble, FicRecords};
use crate::fic::{walk, FicDecoder};
use crate::fig::{FigLists, FigRecord};
use crate::tables::FIC_BITS;
use sdrkit_core::ConfigError;
use sdrkit_core::Values;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use std::sync::Arc;

fn typed<T: sdrkit_graph::Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

/// The FIC decoder component.
#[derive(Default)]
pub struct FicComponent {
    dec: FicDecoder,
    fibs: Vec<crate::fic::Fib>,
}

impl Component for FicComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.fic",
            vec![Port::typed::<FicSoft>("in")],
            vec![Port::typed::<FibItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec = FicDecoder::new();
        Ok(vec![typed::<FibItem>()])
    }
    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        for f in io.items::<FicSoft>(0) {
            self.fibs.clear();
            let viterbi = io.section("viterbi");
            self.dec
                .decode(f.value.frame, &f.value.soft[..FIC_BITS], &mut self.fibs);
            drop(viterbi);
            for fib in self.fibs.drain(..) {
                io.items_out::<FibItem>(0).push(Timed {
                    time: f.time,
                    value: FibItem {
                        fib,
                        start: f.value.start,
                    },
                });
            }
        }
    }
}

/// The FIG parser component.
#[derive(Default)]
pub struct FigParser {
    ok: u8,
    bytes: [i16; 12],
    totals: (u64, u64),
}

impl Component for FigParser {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.figs",
            vec![Port::typed::<FibItem>("in")],
            vec![Port::typed::<FigEvent>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = FigParser::default();
        Ok(vec![typed::<FigEvent>()])
    }
    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        for it in io.items::<FibItem>(0) {
            let fib = &it.value.fib;
            let frame = fib.frame;
            let pos = fib.pos;
            if pos == 0 {
                self.ok = 0;
                self.bytes = [-1; 12];
            }
            if fib.ok {
                self.ok += 1;
                self.totals.0 += 1;
                let out = io.items_out::<FigEvent>(0);
                let used = walk(&fib.data, |t, d| {
                    out.push(Timed {
                        time: it.time,
                        value: FigEvent::Fig {
                            frame,
                            pos,
                            rec: FigRecord::new(t, d),
                        },
                    });
                });
                self.bytes[pos as usize] = used as i16;
            } else {
                self.totals.1 += 1;
                self.bytes[pos as usize] = -1;
            }
            if pos == 11 {
                io.items_out::<FigEvent>(0).push(Timed {
                    time: it.time,
                    value: FigEvent::FrameDone {
                        frame,
                        start: it.value.start,
                        ok: self.ok,
                        bytes: self.bytes,
                        totals: self.totals,
                    },
                });
            }
        }
    }
}

/// The ensemble database component.
#[derive(Default)]
pub struct EnsembleComponent {
    db: Ensemble,
    lists: FigLists,
    records: FicRecords,
    shared: Arc<Ensemble>,
    stats: Arc<FicStats>,
    spare: Vec<Arc<FicStats>>,
    cif_count: Option<u16>,
    changed: bool,
    new_ensemble: bool,
}

impl EnsembleComponent {
    /// The database.
    pub fn db(&self) -> &Ensemble {
        &self.db
    }
    /// The FIC records.
    pub fn records(&self) -> &FicRecords {
        &self.records
    }
}

impl Component for EnsembleComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.ensemble",
            vec![Port::typed::<FigEvent>("in")],
            vec![Port::typed::<EnsembleItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = EnsembleComponent::default();
        Ok(vec![typed::<EnsembleItem>()])
    }
    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        for it in io.items::<FigEvent>(0) {
            match &it.value {
                FigEvent::Fig { frame, pos, rec } => {
                    self.records.add(rec, *frame, *pos);
                    let mut ch = Changes::default();
                    self.db.apply(rec, &mut self.lists, *frame, &mut ch);
                    if let Some(c) = ch.cif_count {
                        // The count belongs to the CIF of the FIB group that carried it.
                        self.cif_count = Some((c + 5000 - (*pos as u16 / 3)) % 5000);
                    }
                    if ch.new_ensemble {
                        self.records = FicRecords::default();
                        self.new_ensemble = true;
                    }
                    self.changed |= ch.services || ch.subchannels || ch.new_ensemble;
                }
                FigEvent::FrameDone {
                    frame,
                    start,
                    ok,
                    bytes,
                    totals,
                } => {
                    self.db.age_announcements(*frame);
                    self.records.age(*frame);
                    self.records.fib_bytes = *bytes;
                    for &b in bytes {
                        if b >= 0 {
                            self.records.used.0 += b as u64;
                            self.records.used.1 += 1;
                        }
                    }
                    if Arc::get_mut(&mut self.stats).is_none() {
                        // An item still holds the last statistics: take a buffer that no item holds.
                        let free = self.spare.iter().position(|a| Arc::strong_count(a) == 1);
                        let next = match free {
                            Some(i) => self.spare.swap_remove(i),
                            None => Arc::new(FicStats::default()),
                        };
                        let last = std::mem::replace(&mut self.stats, next);
                        if self.spare.len() < 4 {
                            self.spare.push(last);
                        }
                    }
                    if let Some(st) = Arc::get_mut(&mut self.stats) {
                        st.fib_ok = totals.0;
                        st.fib_bad = totals.1;
                        st.fib_ok_last_frame = *ok;
                        st.fic_fill_pct = self.records.fill_pct();
                        st.fib_bytes = *bytes;
                        st.records = self.records.records.len();
                        st.per_type.clear();
                        st.per_type.extend(
                            self.records
                                .per_type
                                .iter()
                                .map(|(&(t, e), &(c, b))| (t, e, c, b)),
                        );
                    }
                    let changed =
                        self.changed || self.shared.announcements != self.db.announcements;
                    if changed {
                        self.shared = Arc::new(self.db.clone());
                    }
                    io.items_out::<EnsembleItem>(0).push(Timed {
                        time: it.time,
                        value: EnsembleItem {
                            frame: *frame,
                            start: *start,
                            cif_count: self.cif_count.take(),
                            db: self.shared.clone(),
                            changed,
                            new_ensemble: std::mem::take(&mut self.new_ensemble),
                            fic: self.stats.clone(),
                            utc: (self.db.utc_seconds, self.db.utc_ms),
                        },
                    });
                    self.changed = false;
                }
            }
        }
    }
}

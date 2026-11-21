//! Demodulators of FSK, GMSK, MSK, chirp, OFDM and spread signals, the sync search and tone detection.

mod chirp;
mod despread;
mod fsk;
mod gmsk;
mod msk;
mod ofdm;
mod sync_search;
mod tone;

pub use chirp::{ChirpSymbol, DechirpDemod};
pub use despread::Despread;
pub use fsk::{FskDemod, FskEnergies};
pub use gmsk::GmskDemod;
pub use msk::MskDemod;
pub use ofdm::{OfdmCells, OfdmDemod, OfdmSync};
pub use sync_search::SyncSearch;
pub use tone::{ToneDetect, ToneEvent};

use sdrkit_graph::{Buffer, Registry, Tag, TagKind};

/// Adds these components.
pub fn register(r: &mut Registry) {
    r.add("modem.fsk_demod", || Box::new(FskDemod::default()));
    r.add("modem.fsk_demod_real", || Box::new(FskDemod::on_real()));
    r.add("modem.gmsk_demod", || Box::new(GmskDemod::default()));
    r.add("modem.msk_demod", || Box::new(MskDemod::default()));
    r.add("modem.dechirp", || Box::new(DechirpDemod::default()));
    r.add("modem.ofdm_sync", || Box::new(OfdmSync::default()));
    r.add("modem.ofdm_demod", || Box::new(OfdmDemod::default()));
    r.add("modem.despread", || Box::new(Despread::default()));
    r.add("modem.sync_search", || Box::new(SyncSearch::default()));
    r.add("modem.sync_search_symbols", || {
        Box::new(SyncSearch::on_symbols())
    });
    r.add("modem.tone_detect", || Box::new(ToneDetect::default()));
}

/// The pieces of an input of `len` items between its `Retuned` and `Discontinuity` tags, as `(start, end, reset)`: `reset` is
/// true when the demodulator must start afresh at `start` (the piece may then be empty, for a tag just past the last item).
fn pieces(input: &Buffer, len: usize) -> Pieces<'_> {
    Pieces {
        tags: input.tags().iter(),
        first: input.first(),
        len,
        at: 0,
        reset: false,
        done: false,
    }
}

/// See [`pieces`].
struct Pieces<'a> {
    tags: std::slice::Iter<'a, Tag>,
    first: u64,
    len: usize,
    at: usize,
    reset: bool,
    done: bool,
}

impl Iterator for Pieces<'_> {
    type Item = (usize, usize, bool);
    fn next(&mut self) -> Option<Self::Item> {
        for t in self.tags.by_ref() {
            if !matches!(
                t.kind,
                TagKind::Retuned { .. } | TagKind::Discontinuity { .. }
            ) {
                continue;
            }
            let p = (t.at.saturating_sub(self.first) as usize).min(self.len);
            if p > self.at {
                let piece = (self.at, p, self.reset);
                self.at = p;
                self.reset = true;
                return Some(piece);
            }
            self.reset = true;
        }
        if self.done {
            return None;
        }
        self.done = true;
        (self.len > self.at || self.reset).then_some((self.at, self.len, self.reset))
    }
}

/// Parses a list of numbers separated by commas or spaces (`"1200, 2200"`); `None` when an entry is not a finite number.
fn parse_list(s: &str) -> Option<Vec<f64>> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_split_at_resets() {
        let mut b = Buffer::new(&sdrkit_graph::Kind::Real);
        b.push_tag(
            3,
            TagKind::Retuned {
                centre_hz: 0.0,
                tuned_hz: 0.0,
            },
        );
        b.push_tag(5, TagKind::Lock { locked: true });
        b.push_tag(10, TagKind::Discontinuity { lost: 0 });
        let v: Vec<_> = pieces(&b, 10).collect();
        assert_eq!(v, [(0, 3, false), (3, 10, true), (10, 10, true)]);
        let e = Buffer::new(&sdrkit_graph::Kind::Real);
        assert_eq!(pieces(&e, 4).collect::<Vec<_>>(), [(0, 4, false)]);
        assert_eq!(pieces(&e, 0).count(), 0);
    }

    #[test]
    fn lists_parse_strictly() {
        assert_eq!(parse_list("1200, 2200"), Some(vec![1200.0, 2200.0]));
        assert_eq!(parse_list(" 1 2 3 "), Some(vec![1.0, 2.0, 3.0]));
        assert_eq!(parse_list("1, x"), None);
        assert_eq!(parse_list(""), Some(vec![]));
    }
}

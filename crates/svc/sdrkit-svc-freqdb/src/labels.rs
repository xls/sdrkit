//! Labels on a spectrum: names from live decoding, scan results, memories, database entries and plan channels, placed so
//! that they do not overlap, the most useful first.

use crate::db::FreqDb;
use sdrkit_data_plans::ChannelPlan;
use serde::{Deserialize, Serialize};

sdrkit_core::choice_enum! {
    /// Where a label comes from. Earlier sources win a place first.
    #[derive(PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum LabelSource {
        /// A name the receiver decoded now (an FM station name, a DAB ensemble).
        Live = ("live", "Live"),
        /// A scan result.
        Scan = ("scan", "Scan"),
        /// A memory or bookmark.
        Memory = ("memory", "Memory"),
        /// A starred database entry.
        Starred = ("starred", "Starred"),
        /// A database entry.
        Database = ("database", "Database"),
        /// A plan channel or segment.
        Plan = ("plan", "Plan"),
    }
}

/// One name to place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelInput {
    /// Frequency (or lower edge of a range), Hz.
    pub hz: f64,
    /// Upper edge of a range, Hz.
    #[serde(default)]
    pub end_hz: Option<f64>,
    /// Text shown.
    pub text: String,
    /// Where it comes from.
    pub source: LabelSource,
    /// An id the host can act on (a database entry, a memory id), empty for none.
    #[serde(default)]
    pub id: String,
}

/// The view the labels are for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// Left edge, Hz.
    pub lo_hz: f64,
    /// Right edge, Hz.
    pub hi_hz: f64,
    /// Width in pixels (or points).
    pub width_px: f64,
    /// Width of one character, pixels.
    #[serde(default = "default_char_px")]
    pub char_px: f64,
    /// Rows of labels available.
    #[serde(default = "default_rows")]
    pub rows: usize,
}

fn default_char_px() -> f64 {
    7.0
}

fn default_rows() -> usize {
    3
}

/// A placed label.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Label {
    /// Frequency, Hz (the lower edge of a span).
    pub hz: f64,
    /// Upper edge of a span.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_hz: Option<f64>,
    /// Text.
    pub text: String,
    /// Where it comes from.
    pub source: LabelSource,
    /// The id given with the input.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Horizontal position of the frequency (the left edge of a span), pixels.
    pub x_px: f64,
    /// Width of the span, pixels (0 for a point).
    pub span_px: f64,
    /// Row, 0 nearest the spectrum.
    pub row: usize,
}

/// The labels of a view: points (one frequency) and spans (bands and ranges, drawn as bars).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Labels {
    /// Point labels, placed without overlap.
    pub points: Vec<Label>,
    /// Span labels (at most 32, widest last).
    pub spans: Vec<Label>,
}

/// Most points considered before layout.
const MAX_INPUTS: usize = 4000;
/// Spacing in pixels below which plan channels are left out (too dense to read).
const MIN_CHANNEL_PX: f64 = 24.0;

/// Gathers the names of a view, for [`spectrum_labels`]: the entries of the database that apply in `scope` (see
/// [`Entry::applies`](crate::Entry::applies)) and the names of `plans`. The channels of a plan are added only when they
/// are at least 24 pixels apart; the segments of a plan are added as spans.
pub fn gather(db: &FreqDb, plans: &[&ChannelPlan], scope: &[String], view: &View) -> Vec<LabelInput> {
    let mut v = Vec::new();
    let span = (view.hi_hz - view.lo_hz).max(1.0);
    for e in db.in_span(view.lo_hz, view.hi_hz, scope) {
        if v.len() >= MAX_INPUTS {
            break;
        }
        // Plan-channel entries come from the plans below, with the density rule.
        if e.id.starts_with("p:") {
            continue;
        }
        v.push(LabelInput {
            hz: e.hz,
            end_hz: e.end_hz,
            text: e.name.clone(),
            source: if db.is_starred(&e.id) {
                LabelSource::Starred
            } else {
                LabelSource::Database
            },
            id: e.id.clone(),
        });
    }
    {
        {
            for p in plans {
                let inside: Vec<_> = p
                    .channels
                    .iter()
                    .filter(|c| {
                        !c.reallocated && c.centre_hz >= view.lo_hz && c.centre_hz <= view.hi_hz
                    })
                    .collect();
                let min_gap = inside
                    .windows(2)
                    .map(|w| (w[1].centre_hz - w[0].centre_hz).abs())
                    .filter(|g| *g > 0.0)
                    .fold(f64::INFINITY, f64::min)
                    .min(inside.first().map(|c| c.width_hz).unwrap_or(f64::INFINITY));
                if !inside.is_empty() && min_gap / span * view.width_px >= MIN_CHANNEL_PX {
                    for c in inside.into_iter().take(MAX_INPUTS) {
                        v.push(LabelInput {
                            hz: c.centre_hz,
                            end_hz: None,
                            text: c.name.clone(),
                            source: LabelSource::Plan,
                            id: String::new(),
                        });
                    }
                }
                for s in &p.segments {
                    if s.hi_hz >= view.lo_hz && s.lo_hz <= view.hi_hz {
                        v.push(LabelInput {
                            hz: s.lo_hz,
                            end_hz: Some(s.hi_hz),
                            text: s.name.clone(),
                            source: LabelSource::Plan,
                            id: String::new(),
                        });
                    }
                }
            }
        }
    }
    v
}

/// Lays out labels: points in rows without overlap (the best sources first, then nearest the view's centre), and spans.
/// Labels outside the view are dropped; the same text on the same frequency is kept once.
pub fn spectrum_labels(view: &View, inputs: &[LabelInput]) -> Labels {
    let span = view.hi_hz - view.lo_hz;
    if !(span > 0.0 && view.width_px > 0.0) {
        return Labels::default();
    }
    let x_of = |hz: f64| (hz - view.lo_hz) / span * view.width_px;
    let centre = (view.lo_hz + view.hi_hz) / 2.0;
    let mut points: Vec<&LabelInput> = inputs
        .iter()
        .filter(|i| {
            i.end_hz.is_none()
                && i.hz >= view.lo_hz
                && i.hz <= view.hi_hz
                && !i.text.trim().is_empty()
        })
        .collect();
    points.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then((a.hz - centre).abs().total_cmp(&(b.hz - centre).abs()))
    });
    let rows = view.rows.max(1);
    let mut taken: Vec<Vec<(f64, f64)>> = vec![Vec::new(); rows];
    let mut out = Labels::default();
    for p in points {
        if out
            .points
            .iter()
            .any(|l| (l.hz - p.hz).abs() < 1.0 && l.text == p.text)
        {
            continue;
        }
        let x = x_of(p.hz);
        let w = p.text.chars().count() as f64 * view.char_px + view.char_px;
        let (l, r) = (x - w / 2.0, x + w / 2.0);
        if let Some(row) = (0..rows).find(|&row| taken[row].iter().all(|&(a, b)| r <= a || l >= b))
        {
            taken[row].push((l, r));
            out.points.push(Label {
                hz: p.hz,
                end_hz: None,
                text: p.text.clone(),
                source: p.source,
                id: p.id.clone(),
                x_px: x,
                span_px: 0.0,
                row,
            });
        }
    }
    let mut spans: Vec<&LabelInput> = inputs
        .iter()
        .filter(|i| i.end_hz.is_some_and(|e| e >= view.lo_hz) && i.hz <= view.hi_hz)
        .collect();
    spans.sort_by(|a, b| {
        let wa = a.end_hz.unwrap_or(a.hz) - a.hz;
        let wb = b.end_hz.unwrap_or(b.hz) - b.hz;
        a.source.cmp(&b.source).then(wa.total_cmp(&wb))
    });
    spans.dedup_by(|a, b| a.text == b.text && (a.hz - b.hz).abs() < 1.0);
    for s in spans.into_iter().take(32) {
        let lo = s.hz.max(view.lo_hz);
        let hi = s.end_hz.unwrap_or(s.hz).min(view.hi_hz);
        out.spans.push(Label {
            hz: s.hz,
            end_hz: s.end_hz,
            text: s.text.clone(),
            source: s.source,
            id: s.id.clone(),
            x_px: x_of(lo),
            span_px: x_of(hi) - x_of(lo),
            row: 0,
        });
    }
    out
}

/// Both steps: the database and plan names of the view plus the `live` names, laid out.
pub fn auto_labels(
    db: &FreqDb,
    plans: &[&ChannelPlan],
    scope: &[String],
    view: &View,
    live: &[LabelInput],
) -> Labels {
    let mut inputs = live.to_vec();
    inputs.extend(gather(db, plans, scope, view));
    spectrum_labels(view, &inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(lo: f64, hi: f64) -> View {
        View {
            lo_hz: lo,
            hi_hz: hi,
            width_px: 1000.0,
            char_px: 7.0,
            rows: 2,
        }
    }

    #[test]
    fn layout_without_overlap() {
        let v = view(100e6, 101e6);
        let input = |hz: f64, text: &str, source| LabelInput {
            hz,
            end_hz: None,
            text: text.into(),
            source,
            id: String::new(),
        };
        let inputs = vec![
            input(100.5e6, "DATABASE", LabelSource::Database),
            input(100.5e6, "LIVE FM", LabelSource::Live),
            input(100.501e6, "THIRD", LabelSource::Plan),
            input(100.502e6, "FOURTH", LabelSource::Plan),
            input(100.2e6, "FAR", LabelSource::Memory),
            input(99e6, "OUTSIDE", LabelSource::Live),
            input(100.5e6, "LIVE FM", LabelSource::Live),
        ];
        let l = spectrum_labels(&v, &inputs);
        let texts: Vec<&str> = l.points.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["LIVE FM", "FAR", "DATABASE"]);
        assert_eq!((l.points[0].row, l.points[2].row), (0, 1));
        assert!((l.points[0].x_px - 500.0).abs() < 1e-6);
        // In each row no two labels overlap.
        for row in 0..2 {
            let mut iv: Vec<(f64, f64)> = l
                .points
                .iter()
                .filter(|p| p.row == row)
                .map(|p| {
                    let w = p.text.len() as f64 * 7.0 + 7.0;
                    (p.x_px - w / 2.0, p.x_px + w / 2.0)
                })
                .collect();
            iv.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert!(iv.windows(2).all(|w| w[0].1 <= w[1].0));
        }
    }

    #[test]
    fn database_and_plans_on_a_view() {
        let plans = sdrkit_dev_test::plans();
        let mut db = FreqDb::new();
        db.set_supplied(crate::data::tests::supplied());
        db.star("b:ft8-20", true);
        let scope = |t: &str| -> Vec<String> { t.split(' ').map(String::from).collect() };
        let plan = |id: &str| vec![plans.get(id).unwrap()];
        let (us, de) = (scope("US itu2"), scope("DE itu1"));
        // 20 m: FT8 starred, other entries from the database, the band as a span.
        let l = auto_labels(
            &db,
            &plan("ham_r2"),
            &us,
            &view(14.0e6, 14.35e6),
            &[],
        );
        let ft8 = l.points.iter().find(|p| p.text == "FT8 20 m").unwrap();
        assert_eq!(ft8.source, LabelSource::Starred);
        assert!(l.points.iter().any(|p| p.text == "NCDXF/IARU beacons 20 m"));
        assert!(l.spans.iter().any(|s| s.text == "20 m"));
        // PMR446 zoomed in: channel names appear; zoomed out over UHF they are too dense.
        let near = auto_labels(
            &db,
            &plan("pmr446"),
            &de,
            &view(446.0e6, 446.2e6),
            &[],
        );
        assert!(near.points.iter().any(|p| p.text.starts_with("PMR ")));
        let far = auto_labels(
            &db,
            &plan("pmr446"),
            &de,
            &view(400e6, 500e6),
            &[],
        );
        assert!(far.points.iter().all(|p| p.source != LabelSource::Plan));
        // FM band: TV and FM plans give spans; live names win.
        let live = vec![LabelInput {
            hz: 100.1e6,
            end_hz: None,
            text: "RADIO 1".into(),
            source: LabelSource::Live,
            id: String::new(),
        }];
        let fm = auto_labels(
            &db,
            &plan("fm_europe"),
            &de,
            &view(87.5e6, 108e6),
            &live,
        );
        assert_eq!(fm.points[0].text, "RADIO 1");
        assert!(fm.spans.iter().any(|s| s.text == "FM"));
        assert!(spectrum_labels(&view(1.0, 1.0), &live).points.is_empty());
    }
}

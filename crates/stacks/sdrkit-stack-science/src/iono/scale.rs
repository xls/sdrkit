//! Ionogram scaling: traces from an ionogram, the URSI parameters of a vertical ionogram, the modes and MUFs of an oblique one, the
//! hop geometry over a spherical earth, the list of known chirp sounders and the amateur and broadcast bands.
//! Standard: URSI Handbook of Ionogram Interpretation and Reduction (UAG-23A)

use sdrkit_util_geo::Position;
use serde::{Deserialize, Serialize};

/// The radius of the earth for the hop geometry, km.
pub const EARTH_KM: f64 = 6371.0;
/// The speed of light, km per second.
pub const C_KM_S: f64 = 299_792.458;
/// Rows of an ionogram.
pub const ROWS: usize = 200;
/// Columns of an ionogram.
pub const COLS: usize = 600;
/// The width of a column, Hz.
pub const COL_HZ: f64 = 50e3;
/// The height of a row, seconds of delay.
pub const ROW_S: f64 = 50e-6;
/// The group range of one row, km.
pub const ROW_KM: f64 = C_KM_S * ROW_S;

/// An ionogram in dB over each column's median. Cell `(row, col)` is at `db[row * COLS + col]`. A column that was never swept has
/// `swept[col] == false` and its cells mean nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct IonogramDb {
    /// The cells, row by row.
    pub db: Vec<f32>,
    /// The columns with data.
    pub swept: Vec<bool>,
}

impl IonogramDb {
    /// An empty ionogram: every cell 0 dB and no column swept.
    pub fn new() -> IonogramDb {
        IonogramDb {
            db: vec![0.0; ROWS * COLS],
            swept: vec![false; COLS],
        }
    }
    /// The cell at `row`, `col`.
    pub fn at(&self, row: usize, col: usize) -> f32 {
        self.db[row * COLS + col]
    }
}

impl Default for IonogramDb {
    fn default() -> Self {
        IonogramDb::new()
    }
}

/// The centre frequency of column `col`, Hz.
pub fn col_hz(col: usize) -> f64 {
    (col as f64 + 0.5) * COL_HZ
}

/// One point of a trace: a column and a row.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TracePoint {
    /// The column.
    pub col: usize,
    /// The row (fractional, the peak's centre).
    pub row: f64,
    /// The level, dB over the column's median.
    pub db: f32,
}

impl TracePoint {
    /// The frequency, Hz.
    pub fn freq_hz(&self) -> f64 {
        col_hz(self.col)
    }
    /// The group range, km.
    pub fn range_km(&self) -> f64 {
        self.row * ROW_KM
    }
}

/// A trace: echoes of neighbouring columns at close delays, lowest frequency first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    /// The points.
    pub points: Vec<TracePoint>,
}

impl Trace {
    /// The lowest and highest frequencies, Hz.
    pub fn freq_range_hz(&self) -> (f64, f64) {
        let lo = self.points.first().map_or(0.0, |p| p.freq_hz());
        let hi = self.points.last().map_or(0.0, |p| p.freq_hz());
        (lo, hi)
    }
    /// The shortest group range, km.
    pub fn min_range_km(&self) -> f64 {
        self.points
            .iter()
            .map(|p| p.range_km())
            .fold(f64::INFINITY, f64::min)
    }
}

/// How traces are found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceOptions {
    /// A peak stands at least this far over the column's median, dB.
    pub peak_db: f32,
    /// A column with more than this share of its rows over `peak_db` is an interfering carrier and is dropped.
    pub lit_share: f64,
    /// Peaks of neighbouring columns join a trace when their rows differ by at most this.
    pub link_rows: f64,
    /// Columns a trace can skip.
    pub gap_cols: usize,
    /// The fewest points of a trace; shorter runs are noise.
    pub min_points: usize,
}

impl Default for TraceOptions {
    fn default() -> Self {
        TraceOptions {
            peak_db: 8.0,
            lit_share: 0.3,
            link_rows: 4.0,
            gap_cols: 2,
            min_points: 4,
        }
    }
}

/// The delay peaks of every swept column: rows over `peak_db` that are local maxima, with the row refined by a parabola. Columns lit
/// at most delays are left out.
pub fn column_peaks(ion: &IonogramDb, o: &TraceOptions) -> Vec<Vec<TracePoint>> {
    let mut out = vec![Vec::new(); COLS];
    for (col, peaks) in out.iter_mut().enumerate() {
        if !ion.swept[col] {
            continue;
        }
        let lit = (0..ROWS).filter(|&r| ion.at(r, col) >= o.peak_db).count();
        if lit as f64 > o.lit_share * ROWS as f64 {
            continue;
        }
        for row in 0..ROWS {
            let v = ion.at(row, col);
            if v < o.peak_db {
                continue;
            }
            let below = if row > 0 {
                ion.at(row - 1, col)
            } else {
                f32::MIN
            };
            let above = if row + 1 < ROWS {
                ion.at(row + 1, col)
            } else {
                f32::MIN
            };
            if v < below || v <= above {
                continue;
            }
            let frac = if row > 0 && row + 1 < ROWS {
                sdrkit_algo_dsp::peak::parabolic(below as f64, v as f64, above as f64).offset
            } else {
                0.0
            };
            peaks.push(TracePoint {
                col,
                row: row as f64 + frac.clamp(-0.5, 0.5),
                db: v,
            });
        }
    }
    out
}

/// The traces of an ionogram: peaks of neighbouring columns at close delays are linked; runs shorter than `min_points` are dropped.
pub fn traces(ion: &IonogramDb, o: &TraceOptions) -> Vec<Trace> {
    let peaks = column_peaks(ion, o);
    let mut open: Vec<Trace> = Vec::new();
    let mut done: Vec<Trace> = Vec::new();
    for (col, ps) in peaks.iter().enumerate() {
        let mut used = vec![false; ps.len()];
        // Each open trace takes the closest free peak of this column.
        for t in open.iter_mut() {
            let last = *t.points.last().expect("a trace has points");
            let best = ps
                .iter()
                .enumerate()
                .filter(|(i, p)| !used[*i] && (p.row - last.row).abs() <= o.link_rows)
                .min_by(|a, b| {
                    (a.1.row - last.row)
                        .abs()
                        .total_cmp(&(b.1.row - last.row).abs())
                });
            if let Some((i, p)) = best {
                used[i] = true;
                t.points.push(*p);
            }
        }
        let (keep, ended): (Vec<Trace>, Vec<Trace>) = open
            .into_iter()
            .partition(|t| col - t.points.last().expect("a trace has points").col <= o.gap_cols);
        done.extend(ended);
        open = keep;
        for (i, p) in ps.iter().enumerate() {
            if !used[i] {
                open.push(Trace { points: vec![*p] });
            }
        }
    }
    done.extend(open);
    done.retain(|t| t.points.len() >= o.min_points);
    done.sort_by(|a, b| a.min_range_km().total_cmp(&b.min_range_km()));
    done
}

/// A frequency, with a mark when the trace ran to the edge of the swept data (URSI qualifying letter D: a lower bound).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Freq {
    /// Hz.
    pub hz: f64,
    /// The true value is at least this.
    pub lower_bound: bool,
}

impl Freq {
    /// "≥ 7.05 MHz" or "7.05 MHz".
    pub fn text(&self) -> String {
        format!(
            "{}{:.2} MHz",
            if self.lower_bound { "≥ " } else { "" },
            self.hz / 1e6
        )
    }
}

/// The URSI parameters of a vertical ionogram. A value not found is `None`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VerticalScaling {
    /// The critical frequency of the E layer.
    pub fo_e: Option<Freq>,
    /// The critical frequency of the F1 layer.
    pub fo_f1: Option<Freq>,
    /// The critical frequency of the F2 layer (ordinary wave).
    pub fo_f2: Option<Freq>,
    /// The end of the extraordinary F2 trace.
    pub fx_f2: Option<Freq>,
    /// The lowest virtual height of the E trace, km.
    pub hp_e_km: Option<f64>,
    /// The lowest virtual height of the F trace, km.
    pub hp_f_km: Option<f64>,
    /// The lowest virtual height of the F2 trace, km.
    pub hp_f2_km: Option<f64>,
    /// The height of the F2 peak, h'(0.834 foF2), km.
    pub hm_f2_km: Option<f64>,
    /// The propagation factor M(3000)F2.
    pub m3000: Option<f64>,
    /// MUF(3000)F2, Hz.
    pub muf3000_hz: Option<f64>,
    /// The lowest frequency of any echo.
    pub fmin_hz: Option<f64>,
    /// A multiple reflection was found.
    pub multiple: bool,
}

/// Echoes under this virtual height are the ground wave, km.
const GROUND_KM: f64 = 40.0;
/// The E region of a vertical ionogram, km.
const E_KM: (f64, f64) = (85.0, 160.0);

/// Scales a vertical ionogram (virtual height = group range / 2). The ground wave and multiple reflections are left out; the E trace
/// is the echoes at 85 to 160 km and its end is foE; the F echoes give foF2 (the O trace end) and fxF2 (the X trace end, 0.3 to
/// 1.0 MHz above); foF1 is a cusp of the F trace; hmF2 = h'(0.834 foF2); M(3000)F2 = 1490 / (hmF2 + 176) after Shimazaki;
/// MUF(3000)F2 = foF2 × M(3000)F2. A trace that runs to the edge of the swept data returns a lower bound and no MUF.
pub fn scale_vertical(ion: &IonogramDb, o: &TraceOptions) -> VerticalScaling {
    let mut s = VerticalScaling::default();
    let peaks = column_peaks(ion, o);
    let in_trace = trace_mask(ion, o);
    let last_swept = (0..COLS).rev().find(|&c| ion.swept[c]);
    // First-order echoes per column: not the ground wave, not twice the height of another echo, part of a trace.
    let mut first: Vec<Vec<f64>> = vec![Vec::new(); COLS];
    for (col, ps) in peaks.iter().enumerate() {
        let hs: Vec<f64> = ps
            .iter()
            .filter(|p| in_trace.contains(&(p.col, p.row.round() as usize)))
            .map(|p| p.range_km() / 2.0)
            .filter(|&h| h > GROUND_KM)
            .collect();
        for &h in &hs {
            let multiple = hs.iter().any(|&g| g < h && (h / g - 2.0).abs() < 0.08);
            if multiple {
                s.multiple = true;
            } else {
                first[col].push(h);
            }
        }
    }
    let freq = |col: usize| Freq {
        hz: col_hz(col),
        lower_bound: Some(col) == last_swept,
    };
    s.fmin_hz = (0..COLS).find(|&c| !first[c].is_empty()).map(col_hz);
    // The E layer.
    let e_cols: Vec<usize> = (0..COLS)
        .filter(|&c| first[c].iter().any(|&h| (E_KM.0..=E_KM.1).contains(&h)))
        .collect();
    if let Some(&top) = e_cols.last() {
        // Two E echoes in a column below the top: the O trace ends there and the X trace runs on.
        let e_of = |c: usize| -> Vec<f64> {
            first[c]
                .iter()
                .copied()
                .filter(|h| (E_KM.0..=E_KM.1).contains(h))
                .collect()
        };
        let c = o_end(&e_cols, top, e_of).unwrap_or(top);
        s.fo_e = Some(Freq {
            hz: col_hz(c) + 0.5 * COL_HZ,
            lower_bound: Some(c) == last_swept,
        });
        s.hp_e_km = e_cols
            .iter()
            .flat_map(|&c| first[c].iter().copied())
            .filter(|h| (E_KM.0..=E_KM.1).contains(h))
            .reduce(f64::min);
    }
    // The F layer: echoes above the E region, at frequencies above the E echoes.
    let e_end = e_cols.last().copied();
    let f_of = |c: usize| -> Vec<f64> {
        if e_end.is_some_and(|e| c <= e) {
            return Vec::new();
        }
        first[c].iter().copied().filter(|&h| h > E_KM.1).collect()
    };
    let f_cols: Vec<usize> = (0..COLS).filter(|&c| !f_of(c).is_empty()).collect();
    let Some(&top) = f_cols.last() else {
        return s;
    };
    s.hp_f_km = f_cols.iter().flat_map(|&c| f_of(c)).reduce(f64::min);
    let fo_col = match o_end(&f_cols, top, f_of) {
        Some(c) => {
            s.fx_f2 = Some(freq(top));
            c
        }
        None => top,
    };
    let fo = freq(fo_col);
    s.fo_f2 = Some(fo);
    // The O trace: the highest first-order F echo of each column up to foF2.
    let o_trace: Vec<(usize, f64)> = f_cols
        .iter()
        .filter(|&&c| c <= fo_col)
        .map(|&c| (c, f_of(c).into_iter().fold(0.0, f64::max)))
        .collect();
    // foF1: a cusp of the O trace well between foE and foF2 (the height rises, then falls by 15 km or more).
    let lo = e_end.map_or(0.0, |e| col_hz(e) + 0.3e6);
    for i in 1..o_trace.len().saturating_sub(1) {
        let (c, h) = o_trace[i];
        let f = col_hz(c);
        if f < lo || f > fo.hz - 0.5e6 {
            continue;
        }
        let left = o_trace[..i].iter().rev().take(10).all(|&(_, g)| g <= h);
        let right = o_trace[i + 1..].iter().take(10).all(|&(_, g)| g <= h);
        let drop = o_trace[i + 1..]
            .iter()
            .take(20)
            .map(|&(_, g)| h - g)
            .fold(0.0, f64::max);
        if left && right && drop >= 15.0 {
            s.fo_f1 = Some(Freq {
                hz: f,
                lower_bound: false,
            });
            break;
        }
    }
    let f2_from = s.fo_f1.map_or(0.0, |f| f.hz);
    s.hp_f2_km = o_trace
        .iter()
        .filter(|&&(c, _)| col_hz(c) > f2_from)
        .map(|&(_, h)| h)
        .reduce(f64::min);
    if !fo.lower_bound {
        let f = 0.834 * fo.hz;
        if let Some(&(_, h)) = o_trace
            .iter()
            .min_by(|a, b| (col_hz(a.0) - f).abs().total_cmp(&(col_hz(b.0) - f).abs()))
        {
            s.hm_f2_km = Some(h);
            let m = 1490.0 / (h + 176.0);
            s.m3000 = Some(m);
            s.muf3000_hz = Some(fo.hz * m);
        }
    }
    s
}

/// The end of the ordinary trace of a layer whose echoes (heights per column from `of`) are in `cols`, highest `top`: the last
/// column with two echoes at different heights, when it is 0.3 to 1.0 MHz below `top` (fxF − foF ≈ fH / 2). `None` when there is
/// no such column: the trace at `top` is then the ordinary one.
fn o_end(cols: &[usize], top: usize, of: impl Fn(usize) -> Vec<f64>) -> Option<usize> {
    let two = cols.iter().rev().find(|&&c| {
        let h = of(c);
        let hi = h.iter().copied().fold(f64::MIN, f64::max);
        let lo = h.iter().copied().fold(f64::MAX, f64::min);
        h.len() >= 2 && hi - lo > 5.0
    })?;
    let d = col_hz(top) - col_hz(*two);
    (0.3e6..=1.0e6).contains(&d).then_some(*two)
}

/// The (column, row) cells that belong to a trace.
fn trace_mask(ion: &IonogramDb, o: &TraceOptions) -> std::collections::HashSet<(usize, usize)> {
    traces(ion, o)
        .iter()
        .flat_map(|t| t.points.iter().map(|p| (p.col, p.row.round() as usize)))
        .collect()
}

/// The group path of an `hops`-hop mode over the ground distance `distance_km` with mirror height `height_km`, km.
pub fn group_path_km(hops: u32, distance_km: f64, height_km: f64) -> f64 {
    let n = hops.max(1) as f64;
    let theta = distance_km / (2.0 * n * EARTH_KM);
    let r2 = EARTH_KM + height_km;
    let s = (EARTH_KM * EARTH_KM + r2 * r2 - 2.0 * EARTH_KM * r2 * theta.cos()).sqrt();
    2.0 * n * s
}

/// The ground distance of an `hops`-hop mode with group path `path_km` and mirror height `height_km`, km. `None` when the path is
/// too short for that height (shorter than 2 × hops × height) or the geometry has no solution.
pub fn distance_km(hops: u32, path_km: f64, height_km: f64) -> Option<f64> {
    let n = hops.max(1) as f64;
    let s = path_km / (2.0 * n);
    if s < height_km {
        return None;
    }
    let r2 = EARTH_KM + height_km;
    let c = (EARTH_KM * EARTH_KM + r2 * r2 - s * s) / (2.0 * EARTH_KM * r2);
    if !(-1.0..=1.0).contains(&c) {
        return None;
    }
    Some(2.0 * n * EARTH_KM * c.acos())
}

/// The mirror height of an `hops`-hop mode with group path `path_km` over `distance_km`, km. `None` when the path is shorter than
/// the ground distance allows.
pub fn mirror_height_km(hops: u32, path_km: f64, distance_km: f64) -> Option<f64> {
    let n = hops.max(1) as f64;
    let s = path_km / (2.0 * n);
    let theta = distance_km / (2.0 * n * EARTH_KM);
    let d = s * s - (EARTH_KM * theta.sin()).powi(2);
    if d < 0.0 {
        return None;
    }
    let h = EARTH_KM * theta.cos() + d.sqrt() - EARTH_KM;
    (h >= 0.0).then_some(h)
}

/// The angle of incidence at the mirror of an `hops`-hop mode over `distance_km` with mirror height `height_km`, radians.
pub fn incidence_rad(hops: u32, distance_km: f64, height_km: f64) -> f64 {
    let n = hops.max(1) as f64;
    let theta = distance_km / (2.0 * n * EARTH_KM);
    let s = group_path_km(hops, distance_km, height_km) / (2.0 * n);
    (EARTH_KM * theta.sin() / s).clamp(-1.0, 1.0).asin()
}

/// One propagation mode of an oblique ionogram.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    /// "1E", "1F", "2F" ...
    pub name: String,
    /// Hops.
    pub hops: u32,
    /// The MUF of the mode: the nose of its trace, Hz.
    pub muf: Freq,
    /// The shortest group path, km.
    pub min_path_km: f64,
    /// The high (Pedersen) ray was seen above the nose.
    pub high_ray: bool,
}

/// How the ground distance was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceFrom {
    /// From the sounder's site and the receiver position.
    Sites,
    /// From the lowest group path.
    Delay,
}

/// The modes and MUF of an oblique ionogram.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObliqueScaling {
    /// The ground distance, km.
    pub distance_km: f64,
    /// How it was found.
    pub from: DistanceFrom,
    /// The modes, fewest hops first.
    pub modes: Vec<Mode>,
    /// The highest MUF of the modes, Hz.
    pub path_muf: Option<Freq>,
}

/// The mirror height assumed for a flat lowest trace (1E), km.
const ASSUMED_E_KM: f64 = 100.0;
/// The mirror height assumed for a curved lowest trace (1F), km.
const ASSUMED_F_KM: f64 = 250.0;
/// The E region of an oblique path, km.
const OBLIQUE_E_KM: (f64, f64) = (85.0, 150.0);
/// The F region of an oblique path, km.
const OBLIQUE_F_KM: (f64, f64) = (150.0, 500.0);

/// A trace is flat when the group path of its lower 70 % in frequency changes by less than this share.
fn is_flat(t: &Trace) -> bool {
    let (lo, hi) = t.freq_range_hz();
    let cut = lo + 0.7 * (hi - lo);
    let ps: Vec<f64> = t
        .points
        .iter()
        .filter(|p| p.freq_hz() <= cut)
        .map(|p| p.range_km())
        .collect();
    let (a, b) = ps
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(a, b), &v| (a.min(v), b.max(v)));
    ps.is_empty() || (b - a) / a.max(1.0) < 0.05
}

/// Scales an oblique ionogram. `known_km` is the ground distance from the sounder's site and the receiver position when both are
/// known; else the distance comes from the lowest group path (the lowest trace taken as 1E when flat, else 1F). Each trace gets the
/// fewest hops whose mirror height is in the E region (85 to 150 km) or the F region (150 to 500 km); F is ruled out below the E MUF
/// of as many hops. The MUF of a mode is the nose of its trace.
pub fn scale_oblique(
    ion: &IonogramDb,
    o: &TraceOptions,
    known_km: Option<f64>,
) -> Option<ObliqueScaling> {
    let ts = traces(ion, o);
    let lowest = ts.first()?;
    let last_swept = (0..COLS).rev().find(|&c| ion.swept[c]);
    let (distance_km, from) = match known_km {
        Some(d) => (d, DistanceFrom::Sites),
        None => {
            let h = if is_flat(lowest) {
                ASSUMED_E_KM
            } else {
                ASSUMED_F_KM
            };
            (
                distance_km(1, lowest.min_range_km(), h)?,
                DistanceFrom::Delay,
            )
        }
    };
    let mut modes: Vec<Mode> = Vec::new();
    for t in &ts {
        let path = t.min_range_km();
        let nose = t
            .points
            .iter()
            .max_by(|a, b| a.col.cmp(&b.col))
            .copied()
            .expect("a trace has points");
        for hops in 1..=4u32 {
            let Some(h) = mirror_height_km(hops, path, distance_km) else {
                continue;
            };
            let layer = if (OBLIQUE_E_KM.0..OBLIQUE_E_KM.1).contains(&h) {
                'E'
            } else if (OBLIQUE_F_KM.0..=OBLIQUE_F_KM.1).contains(&h) {
                'F'
            } else {
                continue;
            };
            let e_muf = modes
                .iter()
                .find(|m| m.hops == hops && m.name.ends_with('E'))
                .map(|m| m.muf.hz);
            if layer == 'F' && e_muf.is_some_and(|e| nose.freq_hz() < e) {
                continue;
            }
            let name = format!("{hops}{layer}");
            let high_ray = t
                .points
                .iter()
                .any(|p| p.col + 1 >= nose.col && p.range_km() > nose.range_km() + 2.0 * ROW_KM);
            let muf = Freq {
                hz: nose.freq_hz(),
                lower_bound: Some(nose.col) == last_swept,
            };
            match modes.iter_mut().find(|m| m.name == name) {
                Some(m) => {
                    if muf.hz > m.muf.hz {
                        m.muf = muf;
                    }
                    m.min_path_km = m.min_path_km.min(path);
                    m.high_ray |= high_ray;
                }
                None => modes.push(Mode {
                    name,
                    hops,
                    muf,
                    min_path_km: path,
                    high_ray,
                }),
            }
            break;
        }
    }
    modes.sort_by(|a, b| a.hops.cmp(&b.hops).then(a.name.cmp(&b.name)));
    let path_muf = modes
        .iter()
        .map(|m| m.muf)
        .max_by(|a, b| a.hz.total_cmp(&b.hz));
    Some(ObliqueScaling {
        distance_km,
        from,
        modes,
        path_muf,
    })
}

/// The geometry of a scaling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Geometry {
    /// Choose from the known sounder's distance, else from the height of the lowest trace.
    Auto,
    /// A local ionosonde.
    Vertical,
    /// A distant sounder.
    Oblique,
}

/// A scaling of either geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Scaling {
    /// A vertical ionogram.
    Vertical(VerticalScaling),
    /// An oblique ionogram.
    Oblique(ObliqueScaling),
}

impl Scaling {
    /// The usable MUF: the path MUF of an oblique ionogram, MUF(3000)F2 of a vertical one.
    pub fn muf_hz(&self) -> Option<f64> {
        match self {
            Scaling::Vertical(v) => v.muf3000_hz,
            Scaling::Oblique(o) => o.path_muf.filter(|f| !f.lower_bound).map(|f| f.hz),
        }
    }
    /// The lowest echo, Hz.
    pub fn fmin_hz(&self) -> Option<f64> {
        match self {
            Scaling::Vertical(v) => v.fmin_hz,
            Scaling::Oblique(_) => None,
        }
    }
    /// One line of text.
    pub fn text(&self) -> String {
        let f = |x: &Option<Freq>| x.map_or("-".into(), |f| f.text());
        let k = |x: Option<f64>| x.map_or("-".into(), |v| format!("{v:.0} km"));
        match self {
            Scaling::Vertical(v) => format!(
                "vertical: foE {}, foF1 {}, foF2 {}, fxF2 {}, h'E {}, h'F {}, h'F2 {}, hmF2 {}, M(3000)F2 {}, MUF(3000)F2 {}, fmin {}",
                f(&v.fo_e),
                f(&v.fo_f1),
                f(&v.fo_f2),
                f(&v.fx_f2),
                k(v.hp_e_km),
                k(v.hp_f_km),
                k(v.hp_f2_km),
                k(v.hm_f2_km),
                v.m3000.map_or("-".into(), |m| format!("{m:.2}")),
                v.muf3000_hz.map_or("-".into(), |m| format!("{:.2} MHz", m / 1e6)),
                v.fmin_hz.map_or("-".into(), |m| format!("{:.2} MHz", m / 1e6)),
            ),
            Scaling::Oblique(o) => {
                let modes: Vec<String> = o
                    .modes
                    .iter()
                    .map(|m| format!("{} {} ({:.0} km)", m.name, m.muf.text(), m.min_path_km))
                    .collect();
                format!(
                    "oblique: {:.0} km ({}), path MUF {}, modes {}",
                    o.distance_km,
                    match o.from {
                        DistanceFrom::Sites => "from the sites",
                        DistanceFrom::Delay => "from the delay",
                    },
                    f(&o.path_muf),
                    modes.join(", ")
                )
            }
        }
    }
}

/// The vertical geometry is chosen below this distance, km.
const VERTICAL_KM: f64 = 100.0;
/// Without a distance, the vertical geometry is chosen when the lowest trace is under this virtual height, km.
const VERTICAL_LOWEST_KM: f64 = 350.0;

/// Scales an ionogram in the requested geometry (`Auto` chooses from `known_km`, else from the lowest trace). `None` when it
/// has no trace.
pub fn scale(
    ion: &IonogramDb,
    o: &TraceOptions,
    geometry: Geometry,
    known_km: Option<f64>,
) -> Option<Scaling> {
    let ts = traces(ion, o);
    let lowest = ts
        .iter()
        .map(|t| t.min_range_km())
        .find(|&r| r / 2.0 > GROUND_KM)?;
    let vertical = match geometry {
        Geometry::Vertical => true,
        Geometry::Oblique => false,
        Geometry::Auto => match known_km {
            Some(d) => d < VERTICAL_KM,
            None => lowest / 2.0 < VERTICAL_LOWEST_KM,
        },
    };
    if vertical {
        Some(Scaling::Vertical(scale_vertical(ion, o)))
    } else {
        scale_oblique(ion, o, known_km.filter(|&d| d >= VERTICAL_KM)).map(Scaling::Oblique)
    }
}

/// A chirp sounder of the list of known sounders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnownSounder {
    /// The name.
    pub name: String,
    /// The sweep rate, Hz per second.
    pub rate_hz_s: f64,
    /// The repetition period, s, when known.
    pub period_s: Option<f64>,
    /// The start offset in the period (UTC seconds modulo the period), when known.
    pub offset_s: Option<f64>,
    /// The site.
    pub site: Position,
    /// The entry has been checked on the air.
    pub verified: bool,
    /// A note.
    pub note: String,
}

fn known(name: &str, rate: f64, lat: f64, lon: f64) -> KnownSounder {
    KnownSounder {
        name: name.into(),
        rate_hz_s: rate,
        period_s: None,
        offset_s: None,
        site: Position::new(lat, lon),
        verified: false,
        note: "rate and site from public reports".into(),
    }
}

/// The built-in list of known chirp sounders. No entry is verified.
pub fn known_sounders() -> Vec<KnownSounder> {
    vec![
        known("Sodankylä (SGO), Finland", 500e3, 67.367, 26.629),
        known("Inskip, UK", 100e3, 53.830, -2.840),
        known("Akrotiri, Cyprus", 100e3, 34.585, 32.987),
        known("Chesapeake, Virginia (ROTHR)", 100e3, 36.56, -76.30),
        known("Kingsville, Texas (ROTHR)", 100e3, 27.5, -97.8),
        known("Juana Díaz, Puerto Rico (ROTHR)", 100e3, 18.02, -66.5),
    ]
}

/// The result of matching a sounder against the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Match {
    /// 0 no match, 1 the rate (and the distance), 2 the rate and the schedule.
    pub level: u8,
    /// The index of the entry matched.
    pub index: Option<usize>,
    /// The entries of the same rate.
    pub candidates: Vec<usize>,
    /// The distance from the matched site to the receiver, km.
    pub distance_km: Option<f64>,
}

/// Matches a sounder heard at `rate_hz_s` with the schedule `period_s` and `offset_s` (when known) against `list`. With a receiver
/// position `rx`, the candidate whose site distance best agrees with `delay_km` (a distance from the delay, when known) wins, else the
/// nearest. Without a position, a single candidate of the rate matches; several do not. A schedule agreeing within 0.1 s
/// returns level 2.
pub fn match_sounder(
    list: &[KnownSounder],
    rate_hz_s: f64,
    period_s: Option<f64>,
    offset_s: Option<f64>,
    rx: Option<Position>,
    delay_km: Option<f64>,
) -> Match {
    let candidates: Vec<usize> = (0..list.len())
        .filter(|&i| (list[i].rate_hz_s - rate_hz_s).abs() <= 0.01 * rate_hz_s)
        .collect();
    let mut m = Match {
        candidates: candidates.clone(),
        ..Match::default()
    };
    let schedule_ok = |k: &KnownSounder| match (k.period_s, k.offset_s, period_s, offset_s) {
        (Some(kp), Some(ko), Some(p), Some(o)) => {
            (kp - p).abs() < 0.1 && {
                let d = (ko - o).rem_euclid(p);
                d.min(p - d) < 0.1
            }
        }
        _ => false,
    };
    let dist = |i: usize| rx.map(|r| list[i].site.distance_m(&r) / 1e3);
    let scheduled: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&i| schedule_ok(&list[i]))
        .collect();
    let pool = if scheduled.is_empty() {
        &candidates
    } else {
        &scheduled
    };
    let pick = if rx.is_some() {
        pool.iter().copied().min_by(|&a, &b| {
            let key = |i: usize| {
                let d = dist(i).unwrap_or(f64::INFINITY);
                delay_km.map_or(d, |k| (d - k).abs())
            };
            key(a).total_cmp(&key(b))
        })
    } else if pool.len() == 1 {
        Some(pool[0])
    } else {
        None
    };
    if let Some(i) = pick {
        m.index = Some(i);
        m.level = if scheduled.contains(&i) { 2 } else { 1 };
        m.distance_km = dist(i);
    }
    m
}

/// A band of the "bands open now" list: name and edges, Hz.
pub const BANDS: &[(&str, f64, f64)] = &[
    ("160 m", 1.810e6, 2.000e6),
    ("120 m bc", 2.300e6, 2.495e6),
    ("90 m bc", 3.200e6, 3.400e6),
    ("80 m", 3.500e6, 3.800e6),
    ("75 m bc", 3.900e6, 4.000e6),
    ("60 m bc", 4.750e6, 5.060e6),
    ("60 m", 5.3515e6, 5.3665e6),
    ("49 m bc", 5.900e6, 6.200e6),
    ("40 m", 7.000e6, 7.200e6),
    ("41 m bc", 7.200e6, 7.450e6),
    ("31 m bc", 9.400e6, 9.900e6),
    ("30 m", 10.100e6, 10.150e6),
    ("25 m bc", 11.600e6, 12.100e6),
    ("22 m bc", 13.570e6, 13.870e6),
    ("20 m", 14.000e6, 14.350e6),
    ("19 m bc", 15.100e6, 15.800e6),
    ("16 m bc", 17.480e6, 17.900e6),
    ("17 m", 18.068e6, 18.168e6),
    ("15 m bc", 18.900e6, 19.020e6),
    ("15 m", 21.000e6, 21.450e6),
    ("13 m bc", 21.450e6, 21.850e6),
    ("12 m", 24.890e6, 24.990e6),
    ("11 m bc", 25.670e6, 26.100e6),
    ("10 m", 28.000e6, 29.700e6),
];

/// Whether a band is open now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BandState {
    /// It starts below the MUF and not below the lowest echo.
    Open,
    /// It starts above the MUF.
    Closed,
    /// It starts below the lowest echo.
    Absorbed,
    /// No MUF is known.
    Unknown,
}

/// The state of every band of [`BANDS`] for a usable MUF and the lowest echo.
pub fn bands_open(muf_hz: Option<f64>, fmin_hz: Option<f64>) -> Vec<(&'static str, BandState)> {
    BANDS
        .iter()
        .map(|&(name, lo, _)| {
            let st = match muf_hz {
                None => BandState::Unknown,
                Some(m) if lo >= m => BandState::Closed,
                Some(_) if fmin_hz.is_some_and(|f| lo < f) => BandState::Absorbed,
                Some(_) => BandState::Open,
            };
            (name, st)
        })
        .collect()
}

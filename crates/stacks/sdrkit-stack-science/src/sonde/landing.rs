//! Landing prediction: an instantaneous model (exponential atmosphere, constant drift) and a smoothed model (least-squares drift
//! and descent over a window, with the ascent winds of the layers below as an option). Positions move on the WGS-84 ellipsoid.

use sdrkit_util_geo::ellipsoid::{Ellipsoid, Observer};
use sdrkit_util_geo::Position;

/// One track point used by the predictor.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrackPoint {
    /// Signal (or sonde) time, s.
    pub t: f64,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Altitude, m.
    pub alt: f64,
}

/// A predicted landing.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Landing {
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Seconds left.
    pub seconds: f64,
}

/// Options of the smoothed model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandingOptions {
    /// Window, s (0: instantaneous).
    pub window_s: f64,
    /// Use the layer winds of the ascent.
    pub layers: bool,
    /// Layer thickness, m.
    pub layer_m: f64,
    /// Weight of the ascent wind.
    pub ascent_weight: f64,
    /// Ground height, m.
    pub ground_m: f64,
}

impl Default for LandingOptions {
    fn default() -> Self {
        LandingOptions {
            window_s: 10.0,
            layers: true,
            layer_m: 500.0,
            ascent_weight: 0.6,
            ground_m: 0.0,
        }
    }
}

/// The point `east_m` and `north_m` metres from `(lat, lon)`, along the WGS-84 geodesic with that bearing.
pub fn moved(lat: f64, lon: f64, east_m: f64, north_m: f64) -> (f64, f64) {
    let d = east_m.hypot(north_m);
    if d == 0.0 {
        return (lat, lon);
    }
    let bearing = east_m.atan2(north_m).to_degrees();
    match Ellipsoid::WGS84.direct(&Position::new(lat, lon), bearing, d) {
        Ok((p, _)) => (p.lat_deg, p.lon_deg),
        Err(_) => (lat, lon),
    }
}

/// The instantaneous model: descent from `alt` at `vv` (negative) in 1 s steps with v(h) = v_now·exp((h − h_now)/(2·7000)), and
/// drift at `(ve, vn)` (or per layer from `wind`). The drift of each stretch of constant wind moves the position on the WGS-84
/// ellipsoid. Returns `None` unless the sonde falls faster than 1 m/s more than 10 m above `ground`.
#[allow(clippy::too_many_arguments)]
pub fn descend(
    lat: f64,
    lon: f64,
    alt: f64,
    vv: f64,
    ve: f64,
    vn: f64,
    ground: f64,
    wind: Option<&dyn Fn(f64) -> Option<(f64, f64)>>,
) -> Option<Landing> {
    if vv >= -1.0 || alt - ground <= 10.0 {
        return None;
    }
    let (mut la, mut lo, mut h) = (lat, lon, alt);
    let (mut de, mut dn) = (0.0, 0.0);
    let mut current: Option<(f64, f64)> = None;
    let mut s = 0.0;
    for _ in 0..36_000 {
        if h <= ground {
            break;
        }
        let v = vv * ((h - alt) / 14_000.0).exp();
        let (e, n) = wind.and_then(|w| w(h)).unwrap_or((ve, vn));
        if current.is_some_and(|c| c != (e, n)) {
            (la, lo) = moved(la, lo, de, dn);
            (de, dn) = (0.0, 0.0);
        }
        current = Some((e, n));
        de += e;
        dn += n;
        h += v;
        s += 1.0;
    }
    (la, lo) = moved(la, lo, de, dn);
    Some(Landing {
        lat: la,
        lon: lo,
        seconds: s,
    })
}

fn slope(xs: &[f64], ys: &[f64]) -> Option<f64> {
    let n = xs.len() as f64;
    if xs.len() < 2 {
        return None;
    }
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
    }
    (sxx > 0.0).then(|| sxy / sxx)
}

/// East and north metres of `p` in the WGS-84 local frame of `origin`.
fn en(p: &TrackPoint, origin: &Observer) -> (f64, f64) {
    let v = origin.enu(&Position::new(p.lat, p.lon).to_ecef());
    (v.east_m, v.north_m)
}

/// The prediction from a track (oldest first) and the latest speeds.
pub fn predict(
    track: &[TrackPoint],
    now: TrackPoint,
    ve: f64,
    vn: f64,
    vv: f64,
    opt: &LandingOptions,
) -> Option<Landing> {
    let inst = || descend(now.lat, now.lon, now.alt, vv, ve, vn, opt.ground_m, None);
    if opt.window_s <= 0.0 || track.len() < 2 {
        return inst();
    }
    let start = now.t - opt.window_s;
    let win: Vec<&TrackPoint> = track
        .iter()
        .filter(|p| p.t >= start && p.t <= now.t)
        .collect();
    if (win.len() as f64) < opt.window_s / 2.0 || win.len() < 3 {
        return inst();
    }
    let ts: Vec<f64> = win.iter().map(|p| p.t).collect();
    let (lat0, lon0) = (now.lat, now.lon);
    let origin = Observer::new(Position::new(lat0, lon0));
    let es: Vec<f64> = win.iter().map(|p| en(p, &origin).0).collect();
    let ns: Vec<f64> = win.iter().map(|p| en(p, &origin).1).collect();
    let hs: Vec<f64> = win.iter().map(|p| p.alt).collect();
    // The whole window must be falling at more than 1 m/s.
    for w in win.windows(2) {
        let dt = w[1].t - w[0].t;
        if dt > 0.0 && (w[1].alt - w[0].alt) / dt > -1.0 {
            return inst();
        }
    }
    let (se, sn, sv) = (slope(&ts, &es)?, slope(&ts, &ns)?, slope(&ts, &hs)?);
    // The smoothed present position: the fit's value at the newest time.
    let n = ts.len() as f64;
    let mt = ts.iter().sum::<f64>() / n;
    let fit = |ys: &[f64], s: f64| ys.iter().sum::<f64>() / n + s * (now.t - mt);
    let (e0, n0, h0) = (fit(&es, se), fit(&ns, sn), fit(&hs, sv));
    let (la, lo) = moved(lat0, lon0, e0, n0);
    if !opt.layers {
        return descend(la, lo, h0, sv, se, sn, opt.ground_m, None);
    }
    // Ascent winds per layer: the track before its highest point.
    let top = track
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.alt.total_cmp(&b.1.alt))
        .map(|x| x.0)
        .unwrap_or(0);
    let ascent = &track[..=top];
    let lm = opt.layer_m.max(1.0);
    let nlayers = ((h0 / lm).ceil().max(0.0) as usize) + 1;
    let mut winds: Vec<Option<(f64, f64)>> = vec![None; nlayers];
    for (k, w) in winds.iter_mut().enumerate() {
        let (lo_h, hi_h) = (k as f64 * lm, (k + 1) as f64 * lm);
        let pts: Vec<&TrackPoint> = ascent
            .iter()
            .filter(|p| p.alt >= lo_h && p.alt < hi_h)
            .collect();
        let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
            continue;
        };
        if pts.len() < 2 || last.t - first.t < 20.0 {
            continue;
        }
        let t: Vec<f64> = pts.iter().map(|p| p.t).collect();
        let e: Vec<f64> = pts.iter().map(|p| en(p, &origin).0).collect();
        let nn: Vec<f64> = pts.iter().map(|p| en(p, &origin).1).collect();
        if let (Some(a), Some(b)) = (slope(&t, &e), slope(&t, &nn)) {
            *w = Some((a, b));
        }
    }
    let wgt = opt.ascent_weight.clamp(0.0, 1.0);
    let wind = move |h: f64| -> Option<(f64, f64)> {
        let k = (h / lm).floor().max(0.0) as usize;
        winds
            .get(k)
            .copied()
            .flatten()
            .map(|(a, b)| (wgt * a + (1.0 - wgt) * se, wgt * b + (1.0 - wgt) * sn))
    };
    descend(la, lo, h0, sv, se, sn, opt.ground_m, Some(&wind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_down_and_drift() {
        let l = descend(25.0, 55.0, 10_000.0, -20.0, 0.0, 0.0, 0.0, None).unwrap();
        assert!((l.lat - 25.0).abs() < 1e-9 && (l.lon - 55.0).abs() < 1e-9);
        assert!(l.seconds > 500.0 && l.seconds < 2500.0, "{}", l.seconds);
        let d = descend(25.0, 55.0, 10_000.0, -20.0, 10.0, 0.0, 0.0, None).unwrap();
        let km = Position::new(25.0, 55.0).distance_m(&Position::new(d.lat, d.lon)) / 1000.0;
        assert!(d.lon > 55.0 && km > 5.0 && km < 25.0, "{km}");
        assert!(descend(25.0, 55.0, 10_000.0, 5.0, 0.0, 0.0, 0.0, None).is_none());
        assert!(descend(25.0, 55.0, 5.0, -5.0, 0.0, 0.0, 0.0, None).is_none());
    }

    #[test]
    fn window_zero_is_instantaneous() {
        let track: Vec<TrackPoint> = (0..20)
            .map(|i| TrackPoint {
                t: i as f64,
                lat: 25.0,
                lon: 55.0,
                alt: 9000.0 - 20.0 * i as f64,
            })
            .collect();
        let now = *track.last().unwrap();
        let opt = LandingOptions {
            window_s: 0.0,
            ..Default::default()
        };
        assert_eq!(
            predict(&track, now, 3.0, 1.0, -20.0, &opt),
            descend(now.lat, now.lon, now.alt, -20.0, 3.0, 1.0, 0.0, None)
        );
        assert_eq!(
            predict(&[], now, 3.0, 1.0, -20.0, &LandingOptions::default()),
            descend(now.lat, now.lon, now.alt, -20.0, 3.0, 1.0, 0.0, None)
        );
    }
}

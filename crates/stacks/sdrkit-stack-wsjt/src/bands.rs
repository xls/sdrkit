//! The bands and dials that the host gives, and the pure functions of whole-span decoding, the multi-band monitor and band
//! hopping.

use crate::mode::Mode;

/// The most bands of the multi-band monitor: the selection mask has four bits for each band.
pub const MAX_BANDS: usize = 11;

/// The modes of the monitor's columns and of whole-span decoding.
pub const MONITOR_MODES: [Mode; 4] = [Mode::Ft8, Mode::Ft4, Mode::Ft2, Mode::Wspr];

/// One band of the setting `band_edges`.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    /// The name ("20m").
    pub name: String,
    /// The lower edge, Hz.
    pub low_hz: f64,
    /// The upper edge, Hz.
    pub high_hz: f64,
}

/// The bands and dials that the host gives: the settings `band_edges` and `dials`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BandPlan {
    /// The bands in monitor order.
    pub bands: Vec<Band>,
    /// The dials (upper sideband, Hz) of each mode.
    pub dials: Vec<(Mode, f64)>,
}

/// A frequency of a band or dial entry: positive and finite.
fn hz(text: &str) -> Option<f64> {
    let v: f64 = text.trim().parse().ok()?;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// The entries of a text of entries separated by ";", each split at its "=": (left, right), both trimmed.
fn entries(text: &str) -> impl Iterator<Item = Option<(&str, &str)>> {
    text.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| e.split_once('=').map(|(a, b)| (a.trim(), b.trim())))
}

/// The bands of the text of the setting `band_edges`: entries `name=low_hz-high_hz` separated by ";". `None` when an
/// entry does not parse, a name is empty or given twice, or a lower edge is not below its upper edge.
pub fn parse_band_edges(text: &str) -> Option<Vec<Band>> {
    let mut out: Vec<Band> = Vec::new();
    for e in entries(text) {
        let (name, range) = e?;
        let (lo, hi) = range.split_once('-')?;
        let (low_hz, high_hz) = (hz(lo)?, hz(hi)?);
        if name.is_empty() || low_hz >= high_hz || out.iter().any(|b| b.name == name) {
            return None;
        }
        out.push(Band {
            name: name.to_owned(),
            low_hz,
            high_hz,
        });
    }
    Some(out)
}

/// The dials of the text of the setting `dials`: entries `mode=frequency_hz` separated by ";", the mode by its id or name
/// in any case. `None` when an entry does not parse or names no mode.
pub fn parse_dials(text: &str) -> Option<Vec<(Mode, f64)>> {
    entries(text)
        .map(|e| {
            let (mode, f) = e?;
            Some((Mode::from_id(&mode.to_ascii_lowercase())?, hz(f)?))
        })
        .collect()
}

impl BandPlan {
    /// The band index of a frequency (Hz): from 3 kHz below the lower edge to the upper edge.
    pub fn band_of(&self, hz: f64) -> Option<usize> {
        self.bands
            .iter()
            .position(|b| hz >= b.low_hz - 3000.0 && hz <= b.high_hz)
    }

    /// The band name of a frequency (Hz).
    pub fn band_name(&self, hz: f64) -> Option<&str> {
        self.band_of(hz).map(|b| self.bands[b].name.as_str())
    }

    /// The dial (Hz) of a (band, monitor mode) pair; `None` when the mode has no dial there.
    pub fn monitor_dial(&self, band: usize, mode: usize) -> Option<f64> {
        let m = *MONITOR_MODES.get(mode)?;
        self.dials
            .iter()
            .find(|&&(dm, d)| dm == m && self.band_of(d) == Some(band))
            .map(|d| d.1)
    }

    /// The dials of whole-span decoding: every FT8, FT4, FT2 and WSPR dial inside the span around `tuned_hz`, except the
    /// dial itself. Returns (mode, dial Hz).
    pub fn span_dials(&self, tuned_hz: f64, span_hz: f64) -> Vec<(Mode, f64)> {
        let mut out = Vec::new();
        for m in MONITOR_MODES {
            for &(dm, d) in &self.dials {
                let off = (d - tuned_hz).abs();
                if dm == m && off <= span_hz / 2.0 - AUDIO_SPAN && off >= 150.0 {
                    out.push((m, d));
                }
            }
        }
        out
    }

    /// The dial (Hz) of `mode` on the band named `band`.
    pub fn dial_of(&self, mode: Mode, band: &str) -> Option<f64> {
        let b = self.bands.iter().position(|x| x.name == band)?;
        self.dials
            .iter()
            .find(|&&(m, d)| m == mode && self.band_of(d) == Some(b))
            .map(|d| d.1)
    }
}

/// The audio a dial needs, Hz.
pub const AUDIO_SPAN: f64 = 3300.0;

/// Whether a dial is covered by a capture (80 % of the rate usable).
pub fn covered(dial_hz: f64, centre_hz: f64, rate: f64) -> bool {
    rate > 0.0 && (dial_hz - centre_hz).abs() + AUDIO_SPAN <= 0.4 * rate
}

/// The capture width that holds an uncovered dial, Hz (0 for a pair with no dial).
pub fn width_needed(dial_hz: Option<f64>, centre_hz: f64) -> f64 {
    match dial_hz {
        Some(d) => 2.0 * ((d - centre_hz).abs() + AUDIO_SPAN) / 0.8,
        None => 0.0,
    }
}

/// The best centre for these dials at `rate`: the middle of the widest group within 2 x (0.4 rate - 3.3 kHz), ties to the lowest.
/// Returns (centre, dials covered).
pub fn best_centre(dials: &[f64], rate: f64) -> Option<(f64, usize)> {
    let mut d: Vec<f64> = dials.to_vec();
    d.sort_by(|a, b| a.total_cmp(b));
    if d.is_empty() {
        return None;
    }
    let w = 2.0 * (0.4 * rate - AUDIO_SPAN);
    let mut best = (d[0], 0usize);
    for i in 0..d.len() {
        let mut j = i;
        while j + 1 < d.len() && d[j + 1] - d[i] <= w {
            j += 1;
        }
        let n = j - i + 1;
        if n > best.1 {
            best = ((d[i] + d[j]) / 2.0, n);
        }
    }
    (best.1 > 0).then_some(best)
}

/// The rate needed for these dials, samples a second.
pub fn needed_rate(dials: &[f64]) -> f64 {
    if dials.is_empty() {
        return 0.0;
    }
    let lo = dials.iter().cloned().fold(f64::MAX, f64::min);
    let hi = dials.iter().cloned().fold(f64::MIN, f64::max);
    2.0 * ((hi - lo) / 2.0 + AUDIO_SPAN) / 0.8
}

/// A selection of (band, mode) pairs as a 64-bit mask, bit `band x 4 + mode`; out-of-range pairs are dropped.
pub fn mask_of(pairs: &[(usize, usize)]) -> u64 {
    pairs
        .iter()
        .filter(|&&(b, m)| b < MAX_BANDS && m < 4)
        .fold(0u64, |a, &(b, m)| a | 1 << (b * 4 + m))
}

/// The pairs of a mask.
pub fn pairs_of(mask: u64) -> Vec<(usize, usize)> {
    (0..MAX_BANDS * 4)
        .filter(|i| mask >> i & 1 == 1)
        .map(|i| (i / 4, i % 4))
        .collect()
}

/// The CPU guard of the multi-band monitor.
#[derive(Clone, Debug, Default)]
pub struct CpuGuard {
    /// The smoothed load.
    pub load: f64,
    over: u32,
    /// Shown while the load is over the limit.
    pub too_slow: bool,
}

impl CpuGuard {
    /// The limit.
    pub const LIMIT: f64 = 0.85;
    /// Takes the load of one buffer (processing time / signal time). Returns true when one selection must be dropped.
    pub fn update(&mut self, load: f64) -> bool {
        self.load = 0.8 * self.load + 0.2 * load;
        if self.load > Self::LIMIT {
            self.too_slow = true;
            self.over += 1;
            if self.over >= 8 {
                self.over = 0;
                return true;
            }
        } else {
            self.too_slow = false;
            self.over = 0;
        }
        false
    }
    /// Which band to drop: the one with the fewest decodes (ties: the highest band); never the last.
    pub fn victim(bands: &[(usize, u64)]) -> Option<usize> {
        if bands.len() <= 1 {
            return None;
        }
        bands
            .iter()
            .min_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|b| b.0)
    }
}

/// The length of a slot of band hopping, seconds.
pub const HOP_SLOT_S: f64 = 120.0;

/// A plan of band hopping. The band numbering and the cycle come from the host (a data file), the masks from the user.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HopPlan {
    /// The band names by number: band `b` is bit `b` of a mask. At most 16.
    pub bands: Vec<String>,
    /// The coordinated cycle: band numbers in their order.
    pub cycle: Vec<usize>,
    /// The bands switched on in each UTC hour, hour 0 first.
    pub hours: [u16; 24],
    /// Every station hops the same cycle at the same time.
    pub coordinated: bool,
}

/// Why a text of band hopping is refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HopTextError {
    /// The hour masks are not 24 hexadecimal numbers of at most 16 bits separated by commas.
    #[error("not 24 hexadecimal masks separated by commas")]
    Hours,
    /// A band list has an empty name, more than 16 bands, or a band twice.
    #[error("a band list needs at most 16 different names separated by commas")]
    Bands,
    /// The cycle names a band that the band list does not have.
    #[error("the cycle names a band that is not in the band list")]
    Cycle,
}

/// The hour masks of a text of 24 hexadecimal masks separated by commas, hour 0 first (spaces around a mask are ignored).
pub fn parse_hours(text: &str) -> Result<[u16; 24], HopTextError> {
    let mut out = [0u16; 24];
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    if parts.len() != 24 {
        return Err(HopTextError::Hours);
    }
    for (o, p) in out.iter_mut().zip(parts) {
        if p.is_empty() || !p.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(HopTextError::Hours);
        }
        *o = u16::from_str_radix(p, 16).map_err(|_| HopTextError::Hours)?;
    }
    Ok(out)
}

/// The text of hour masks: 24 upper-case hexadecimal masks without leading zeros, separated by commas.
pub fn hours_text(hours: &[u16; 24]) -> String {
    hours
        .iter()
        .map(|h| format!("{h:X}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The band names of a text of names separated by commas (empty text: none).
pub fn parse_bands(text: &str) -> Result<Vec<String>, HopTextError> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let names: Vec<String> = text.split(',').map(|s| s.trim().to_owned()).collect();
    let unique = names.iter().enumerate().all(|(i, n)| !names[..i].contains(n));
    if names.len() > 16 || names.iter().any(String::is_empty) || !unique {
        return Err(HopTextError::Bands);
    }
    Ok(names)
}

/// The band numbers of a cycle text, by the names of `bands`.
pub fn parse_cycle(text: &str, bands: &[String]) -> Result<Vec<usize>, HopTextError> {
    parse_bands(text)?
        .iter()
        .map(|n| bands.iter().position(|b| b == n).ok_or(HopTextError::Cycle))
        .collect()
}

impl HopPlan {
    /// The hop slot holding UTC second `t` (negative times round down).
    pub fn slot_of(t: f64) -> i64 {
        (t / HOP_SLOT_S).floor() as i64
    }

    /// The UTC second of the next change of band after `t`.
    pub fn next_change(t: f64) -> f64 {
        (Self::slot_of(t) + 1) as f64 * HOP_SLOT_S
    }

    /// The band number of the slot holding UTC second `t`; `None` when the hour has no band. Coordinated: band `slot mod n` of
    /// the cycle of `n` bands, or when that band is off the next band of the cycle (in cycle order, wrapping) that is on, or
    /// when no band of the cycle is on the lowest numbered band that is on. Not coordinated: the bands that are on, in number
    /// order, one per slot.
    pub fn band_at(&self, t: f64) -> Option<usize> {
        let slot = Self::slot_of(t);
        let hour = (t.rem_euclid(86_400.0) / 3600.0).floor() as usize;
        let mask = self.hours[hour.min(23)];
        if mask == 0 {
            return None;
        }
        let on = |b: usize| b < 16 && mask >> b & 1 == 1;
        if self.coordinated {
            let n = self.cycle.len() as i64;
            if n > 0 {
                let start = slot.rem_euclid(n) as usize;
                if let Some(b) = (0..self.cycle.len())
                    .map(|i| self.cycle[(start + i) % self.cycle.len()])
                    .find(|&b| on(b))
                {
                    return Some(b);
                }
            }
            return (0..16).find(|&b| on(b));
        }
        let bands: Vec<usize> = (0..16).filter(|&b| on(b)).collect();
        Some(bands[slot.rem_euclid(bands.len() as i64) as usize])
    }

    /// The name of band `b`, when the band list has one.
    pub fn name(&self, b: usize) -> Option<&str> {
        self.bands.get(b).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The plan of the repository's data files, through the texts of the settings.
    pub(crate) fn plan() -> BandPlan {
        use sdrkit_presets::wsjt;
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        let dials = wsjt::read_dials(&data.join(wsjt::DIALS)).unwrap();
        let bands = wsjt::read_bands(&data.join(wsjt::BANDS)).unwrap();
        BandPlan {
            bands: parse_band_edges(&wsjt::bands_text(&bands)).unwrap(),
            dials: parse_dials(&wsjt::dials_text(&dials)).unwrap(),
        }
    }

    fn dial(b: &str, m: usize) -> Option<f64> {
        let p = plan();
        let i = p.bands.iter().position(|x| x.name == b).unwrap();
        p.monitor_dial(i, m)
    }

    fn band_of(hz: f64) -> Option<String> {
        plan().band_name(hz).map(str::to_owned)
    }

    #[test]
    fn known_answers() {
        assert_eq!(dial("20m", 0), Some(14.074e6));
        assert_eq!(dial("20m", 1), Some(14.080e6));
        assert_eq!(dial("20m", 2), Some(14.084e6));
        assert!((dial("20m", 3).unwrap() - 14.0956e6).abs() < 1.0);
        assert_eq!(dial("160m", 1), None);
        assert_eq!(dial("160m", 2), None);
        assert!((0..11).filter(|&b| plan().monitor_dial(b, 2).is_some()).count() == 1);
        // 2.4 Msps around 14.000 MHz: the four 20 m pairs covered, 40 m and 17 m not.
        for m in 0..4 {
            assert!(covered(dial("20m", m).unwrap(), 14.0e6, 2.4e6));
        }
        assert!(!covered(dial("40m", 0).unwrap(), 14.0e6, 2.4e6));
        assert!(!covered(dial("17m", 0).unwrap(), 14.0e6, 2.4e6));
        assert!(covered(14.074e6, 14.074e6 + 950e3, 2.4e6));
        assert!(!covered(14.074e6, 14.074e6 + 970e3, 2.4e6));
        // 10 Msps: 40, 30 and 20 m FT8 together.
        let (c, n) = best_centre(&[7.074e6, 10.136e6, 14.074e6], 10e6).unwrap();
        assert_eq!(n, 3);
        assert!(c > 7.0e6 && c < 14.2e6);
        // 2.4 Msps: the most is 20 m FT8 + FT4 at 14.077 MHz.
        let (c, n) = best_centre(&[14.074e6, 14.080e6, 7.074e6], 2.4e6).unwrap();
        assert_eq!(n, 2);
        assert!((c - 14.077e6).abs() < 1.0);
        // Two dials on 40 m beat one on 20 m.
        let (c, n) = best_centre(&[7.074e6, 7.0475e6, 14.074e6], 2.4e6).unwrap();
        assert_eq!(n, 2);
        assert!(c < 8e6);
        let r = needed_rate(&[7.074e6, 18.100e6]);
        assert!((13.5e6..=14.0e6).contains(&r), "{r}");
        assert!(needed_rate(&[14.074e6]) < 20e3);
        // 10 Msps around 16 MHz with 40, 20 and 17 m.
        assert!(!covered(7.074e6, 16e6, 10e6));
        assert!(covered(14.074e6, 16e6, 10e6));
        assert!(covered(18.100e6, 16e6, 10e6));
        assert_eq!(band_of(14.074e6).as_deref(), Some("20m"));
        assert_eq!(band_of(6.9985e6).as_deref(), Some("40m"));
        assert_eq!(width_needed(None, 14e6), 0.0);
        assert!(!covered(14.074e6, 14.0e6, 0.0));
    }

    #[test]
    fn masks_and_the_guard() {
        let m = mask_of(&[(5, 0), (5, 1), (20, 0), (3, 7)]);
        assert_eq!(pairs_of(m), vec![(5, 0), (5, 1)]);
        let mut g = CpuGuard::default();
        let mut fired = None;
        for i in 0..60 {
            if g.update(2.0) {
                fired = Some(i);
                break;
            }
        }
        let f = fired.unwrap();
        assert!((7..=30).contains(&f), "{f}");
        let mut g = CpuGuard::default();
        assert!(!(0..100).any(|_| g.update(0.5)));
        assert_eq!(CpuGuard::victim(&[(3, 5), (5, 2), (9, 2)]), Some(9));
        assert_eq!(CpuGuard::victim(&[(3, 5)]), None);
    }

    #[test]
    fn span_dials_skip_the_dial_itself() {
        let d = plan().span_dials(14.074e6, 250e3);
        assert!(d
            .iter()
            .any(|&(m, f)| m == Mode::Ft4 && (f - 14.080e6).abs() < 1.0));
        assert!(!d
            .iter()
            .any(|&(m, f)| m == Mode::Ft8 && (f - 14.074e6).abs() < 1.0));
    }

    #[test]
    fn the_setting_texts_parse() {
        let b = parse_band_edges("20m=14000000-14350000; 40m = 7000000-7200000;").unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b[1].name, "40m");
        assert_eq!(parse_band_edges(""), Some(Vec::new()));
        for bad in ["20m", "20m=14350000-14000000", "=1-2", "a=1-2;a=3-4", "20m=x-2"] {
            assert_eq!(parse_band_edges(bad), None, "{bad}");
        }
        let d = parse_dials("ft8=14074000;FT4=14080000; wspr=14095600").unwrap();
        assert_eq!(d, vec![(Mode::Ft8, 14.074e6), (Mode::Ft4, 14.080e6), (Mode::Wspr, 14.0956e6)]);
        for bad in ["ft8", "xx=1", "ft8=0"] {
            assert_eq!(parse_dials(bad), None, "{bad}");
        }
        let p = BandPlan::default();
        assert!(p.span_dials(14.074e6, 250e3).is_empty());
        assert_eq!((p.band_name(14.074e6), p.dial_of(Mode::Ft8, "20m")), (None, None));
    }
}

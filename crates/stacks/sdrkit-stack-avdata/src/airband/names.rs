//! Airband channels: the 25 kHz and 8.33 kHz rasters, parsing what the user types, the channel list as a setting value, and the
//! capture plan. The 8.33 kHz names come from sdrkit-data-plans.
//! Standard: ICAO Annex 10 Volume V clause 4.1.2.4 (8.33 kHz channel naming).

use serde::{Deserialize, Serialize};

/// The 8.33 kHz step, Hz.
pub const STEP_833: f64 = 25e3 / 3.0;

/// A channel of the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// Hz.
    pub freq_hz: f64,
    /// On the 8.33 kHz raster.
    pub is833: bool,
    /// The user's tag.
    pub label: String,
    /// Muted.
    pub mute: bool,
    /// Solo.
    pub solo: bool,
    /// Priority (one at most).
    pub priority: bool,
    /// Its own squelch (dB C/N), else the global one.
    pub squelch_db: Option<f64>,
}

impl Channel {
    /// The channel's name ("118.035").
    pub fn name(&self) -> String {
        name(self.freq_hz, self.is833)
    }
}

/// The name of a channel: a 25 kHz channel by its frequency, an 8.33 kHz one by its 25 kHz block and 5, 10 or 15 kHz.
pub fn name(freq_hz: f64, is833: bool) -> String {
    if is833 {
        sdrkit_data_plans::naming::icao_833_name(freq_hz)
    } else {
        format!("{:.3}", (freq_hz / 1e3).round() / 1e3)
    }
}

fn on_833(f: f64) -> bool {
    on_833_within(f, 10.0)
}

fn on_833_within(f: f64, tol_hz: f64) -> bool {
    let n = (f - 118e6) / STEP_833;
    (n - n.round()).abs() * STEP_833 <= tol_hz
}

/// Parses a channel name or an 8.33 kHz raster frequency in MHz: (frequency Hz, 8.33 flag). `None` for anything else.
pub fn parse(text: &str) -> Option<(f64, bool)> {
    let t = text.trim();
    let v: f64 = t.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    let decimals = t.split('.').nth(1).map_or(0, |d| d.len());
    let hz = v * 1e6;
    if !(118e6 - 1.0..=136.990e6 + 1.0).contains(&hz) {
        return None;
    }
    if decimals <= 3 {
        let khz = (v * 1e3).round() as i64;
        let last = khz % 100;
        let block_khz = khz - last % 25;
        match last % 25 {
            0 => return Some((khz as f64 * 1e3, false)),
            5 | 10 | 15 => {
                let idx = (last % 25) / 5 - 1;
                let f = block_khz as f64 * 1e3 + idx as f64 * STEP_833;
                return Some((f, true));
            }
            _ => {}
        }
    }
    // A typed frequency is on the raster within 10 Hz, or within the rounding of its last digit ("118.0083").
    let tol = if decimals >= 4 {
        (0.5e6 * 10f64.powi(-(decimals as i32))).max(10.0)
    } else {
        10.0
    };
    if on_833_within(hz, tol) {
        let n = ((hz - 118e6) / STEP_833).round();
        let f = 118e6 + n * STEP_833;
        let on25 = ((f / 25e3) - (f / 25e3).round()).abs() < 1e-6;
        return Some((if on25 { (f / 1e3).round() * 1e3 } else { f }, true));
    }
    None
}

fn esc(label: &str) -> String {
    let mut s = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() || " -_./()+!&'".contains(c) {
            s.push(c);
        } else {
            let mut b = [0u8; 4];
            for x in c.encode_utf8(&mut b).bytes() {
                s.push_str(&format!("%{x:02X}"));
            }
        }
    }
    s
}

fn unesc(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    let t = String::from_utf8(out).ok()?;
    Some(
        t.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect(),
    )
}

/// The channel list as a setting value ("v3:...").
pub fn list_to_text(list: &[Channel]) -> String {
    let recs: Vec<String> = list
        .iter()
        .map(|c| {
            let mut r = format!(
                "{:.3},{},{},{},{},{}",
                c.freq_hz,
                c.is833 as u8,
                c.mute as u8,
                c.solo as u8,
                c.priority as u8,
                esc(&c.label)
            );
            if let Some(s) = c.squelch_db {
                r.push_str(&format!(",{s:.1}"));
            }
            r
        })
        .collect();
    format!("v3:{}", recs.join(";"))
}

/// Reads a channel list ("v3:" or the older "v2:" form). Damaged or off-raster records are skipped. Sorted by frequency.
pub fn list_from_text(text: &str) -> Vec<Channel> {
    let t = text.trim();
    let (v3, body) = if let Some(b) = t.strip_prefix("v3:") {
        (true, b)
    } else if let Some(b) = t.strip_prefix("v2:") {
        (false, b)
    } else {
        return Vec::new();
    };
    let mut out: Vec<Channel> = Vec::new();
    for rec in body.split(';') {
        let f: Vec<&str> = rec.split(',').collect();
        if f.len() < 6 {
            continue;
        }
        let Ok(hz) = f[0].trim().parse::<f64>() else {
            continue;
        };
        let flag = |s: &str| match s.trim() {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        };
        let (Some(is833), Some(mute), Some(solo), Some(prio)) =
            (flag(f[1]), flag(f[2]), flag(f[3]), flag(f[4]))
        else {
            continue;
        };
        let Some(label) = unesc(f[5]) else { continue };
        if !(118e6 - 1.0..=136.990e6 + 1.0).contains(&hz) {
            continue;
        }
        let ok = if is833 {
            on_833(hz)
        } else {
            ((hz / 25e3) - (hz / 25e3).round()).abs() * 25e3 <= 10.0
        };
        if !ok {
            continue;
        }
        let squelch_db = if v3 {
            f.get(6)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| (-5.0..=45.0).contains(v))
        } else {
            None
        };
        if out.iter().any(|c| (c.freq_hz - hz).abs() < 10.0) {
            continue;
        }
        out.push(Channel {
            freq_hz: hz,
            is833,
            label,
            mute,
            solo,
            priority: prio,
            squelch_db,
        });
    }
    out.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
    // One priority at most.
    let mut seen = false;
    for c in out.iter_mut() {
        if c.priority {
            if seen {
                c.priority = false;
            }
            seen = true;
        }
    }
    out
}

/// Checks a channel list setting: empty, or a list in which every record reads.
///
/// # Errors
///
/// The byte position of the first record that is not a channel on the airband raster (0 when the text does not start with
/// `v3:` or `v2:`).
pub fn list_check(text: &str) -> Result<(), usize> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(());
    }
    let lead = text.len() - text.trim_start().len();
    let Some(body) = t.strip_prefix("v3:").or_else(|| t.strip_prefix("v2:")) else {
        return Err(lead);
    };
    let mut at = lead + 3;
    for r in body.split(';') {
        let start = at;
        at += r.len() + 1;
        if r.trim().is_empty() {
            continue;
        }
        if list_from_text(&format!("{}:{r}", &t[..2])).len() != 1 {
            return Err(start);
        }
    }
    Ok(())
}

/// The usable span of a capture at `rate`, for planning: 2 x (0.45 rate - 12.5 kHz - 12 kHz - 12.5 kHz - tune offset).
pub fn usable_span(rate: f64, tune_offset: f64) -> f64 {
    2.0 * (0.45 * rate - 12.5e3 - 12e3 - 12.5e3 - tune_offset)
}

/// The capture plan: groups of frequencies one capture can hold, with each group's centre.
pub fn capture_plan(freqs: &[f64], rate: f64, tune_offset: f64) -> Vec<(f64, Vec<f64>)> {
    let mut f: Vec<f64> = freqs.to_vec();
    f.sort_by(|a, b| a.total_cmp(b));
    f.dedup();
    let span = usable_span(rate, tune_offset);
    let mut groups: Vec<Vec<f64>> = Vec::new();
    for x in f {
        match groups.last_mut() {
            Some(g) if x - g[0] <= span => g.push(x),
            _ => groups.push(vec![x]),
        }
    }
    groups
        .into_iter()
        .map(|g| ((g[0] + g[g.len() - 1]) / 2.0, g))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b1_names_and_parsing() {
        assert_eq!(parse("118.005"), Some((118.000e6, true)));
        let (f, e) = parse("118.010").unwrap();
        assert!((f - 118.008_333_3e6).abs() < 1.0 && e);
        let (f, e) = parse("132.015").unwrap();
        assert!((f - 132.016_666_7e6).abs() < 1.0 && e);
        assert_eq!(parse("118.025"), Some((118.025e6, false)));
        assert_eq!(parse("121.5"), Some((121.5e6, false)));
        let (f, e) = parse("118.0083").unwrap();
        assert!((f - 118.008_333e6).abs() < 1.0 && e);
        assert!(parse("118.008333").is_some());
        assert!(parse("136.990").is_some());
        for bad in ["118.020", "118.012", "117.9", "137.000", "abc"] {
            assert!(parse(bad).is_none(), "{bad}");
        }
        assert_eq!(name(118.025e6, true), "118.030");
        assert_eq!(name(118.025e6 + STEP_833, true), "118.035");
        assert_eq!(name(118.025e6 + 2.0 * STEP_833, true), "118.040");
        // Every 8.33 kHz frequency's name parses back to it.
        for n in 0..(19e6 / STEP_833) as i64 {
            let f = 118e6 + n as f64 * STEP_833;
            if f > 136.990e6 {
                break;
            }
            let (g, e) = parse(&name(f, true)).unwrap();
            assert!((g - f).abs() < 1.0 && e, "{f}");
        }
    }

    #[test]
    fn b19_list_text() {
        let l = vec![
            Channel {
                freq_hz: 118.7e6,
                label: "Tower".into(),
                ..Default::default()
            },
            Channel {
                freq_hz: 118.025e6 + STEP_833,
                is833: true,
                label: "App, 1;2\n%".into(),
                mute: true,
                squelch_db: Some(12.5),
                ..Default::default()
            },
            Channel {
                freq_hz: 121.5e6,
                label: "Guard".into(),
                priority: true,
                ..Default::default()
            },
        ];
        let t = list_to_text(&l);
        assert!(!t.contains('\n'));
        let back = list_from_text(&t);
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].label, "App, 1;2 %");
        assert_eq!(back[0].squelch_db, Some(12.5));
        assert!(back[2].priority && back[0].mute);
        assert!(list_from_text("").is_empty());
        assert!(list_from_text("v2:").is_empty());
        assert!(list_from_text(";;;").is_empty());
        let v2 = list_from_text("v2:118700000,0,0,0,0,Tower;bad;118012000,0,0,0,0,Off");
        assert_eq!(v2.len(), 1);
        assert!(v2[0].squelch_db.is_none());
        let sq = list_from_text("v3:118700000,0,0,0,0,A,12.5;121500000,0,0,0,0,B,zz");
        assert_eq!(sq[0].squelch_db, Some(12.5));
        assert_eq!(sq[1].squelch_db, None);
    }

    #[test]
    fn b18_capture_plan() {
        let p = capture_plan(&[118.7e6, 118.8e6, 119.0e6], 2.4e6, 54_166.67);
        assert_eq!(p.len(), 1);
        let p = capture_plan(&[118.0e6, 130.0e6], 2.4e6, 0.0);
        assert_eq!(p.len(), 2);
        assert!(capture_plan(&[], 2.4e6, 0.0).is_empty());
        let fs = [
            118.1e6, 118.7e6, 119.9e6, 121.5e6, 124.0e6, 127.3e6, 132.0e6, 135.0e6,
        ];
        let p = capture_plan(&fs, 2.4e6, 0.0);
        let n: usize = p.iter().map(|g| g.1.len()).sum();
        assert_eq!(n, fs.len());
        for (c, g) in &p {
            assert!(g.iter().all(|f| (f - c).abs() <= 0.5 * 2.4e6 * 0.8));
        }
    }
}

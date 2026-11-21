//! The channels that the host gives in the setting `channels` of `atv` and `fpv`: a label and a frequency each, and the
//! FPV view of them (band letter, number, scan centres).

use sdrkit_core::publish::field;
use sdrkit_core::{Setting, SettingError, SettingId};
use sdrkit_data_plans::normalise_name;

/// The most channels of `atv`.
pub const ATV_CHANNELS_CAP: usize = 256;
/// The most channels of `fpv`.
pub const FPV_CHANNELS_CAP: usize = 64;

/// The channels.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "The channels: entries separated by \";\", each a label, \"=\" and the frequency in Hz, for example \"E21=474000000\" \
     or \"R5=5806000000\". At most 256 entries for atv and 64 for fpv. The host fills it from a channel plan file. The \
     table channels lists them and channel.tune takes a label. Empty: no channel.",
);

/// One channel.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// The label ("E21", "5A", "R5").
    pub label: String,
    /// The frequency, Hz.
    pub hz: f64,
}

/// The channels of the text of [`CHANNELS`]. `None` when an entry does not parse or there are more than `cap`.
pub fn parse(text: &str, cap: usize) -> Option<Vec<Channel>> {
    let mut out = Vec::new();
    for e in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let (label, hz) = e.split_once('=')?;
        let label = label.trim();
        let hz: f64 = hz.trim().parse().ok()?;
        if label.is_empty() || !hz.is_finite() || hz <= 0.0 {
            return None;
        }
        out.push(Channel {
            label: label.to_owned(),
            hz,
        });
    }
    (out.len() <= cap).then_some(out)
}

/// As [`parse`], with the error of setting `id`.
///
/// # Errors
///
/// [`SettingError::Malformed`] when the text does not parse.
pub fn read(id: SettingId, text: &str, cap: usize) -> Result<Vec<Channel>, SettingError> {
    parse(text, cap).ok_or(SettingError::Malformed { id, at: None })
}

/// The channel of `channels` named `label` ("21", "e21", "ch 21", "5A"). A label without the band letter also finds the
/// channel with it ("21" finds "E21").
pub fn find<'a>(channels: &'a [Channel], label: &str) -> Option<&'a Channel> {
    let want = normalise_name(label);
    let digits = |s: &str| {
        s.trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .to_string()
    };
    channels
        .iter()
        .find(|c| normalise_name(&c.label) == want)
        .or_else(|| {
            let w = digits(&want);
            if w.is_empty() {
                return None;
            }
            channels
                .iter()
                .find(|c| digits(&normalise_name(&c.label)) == w)
        })
}

/// One FPV channel.
#[derive(Clone, Debug, PartialEq)]
pub struct FpvChannel {
    /// "R5".
    pub name: String,
    /// The band letter: the label without its trailing digits.
    pub band: String,
    /// The trailing number of the label, 0 when it has none.
    pub number: u8,
    /// MHz.
    pub mhz: f64,
}

/// The FPV view of `channels`, in their order.
pub fn fpv_channels(channels: &[Channel]) -> Vec<FpvChannel> {
    channels
        .iter()
        .map(|c| {
            let band = c.label.trim_end_matches(|ch: char| ch.is_ascii_digit());
            FpvChannel {
                name: c.label.clone(),
                band: band.to_owned(),
                number: c.label[band.len()..].parse().unwrap_or(0),
                mhz: c.hz / 1e6,
            }
        })
        .collect()
}

/// The nearest of `channels` to `mhz` within `limit_mhz`.
pub fn fpv_nearest(channels: &[FpvChannel], mhz: f64, limit_mhz: f64) -> Option<&FpvChannel> {
    channels
        .iter()
        .filter(|c| (c.mhz - mhz).abs() <= limit_mhz)
        .min_by(|a, b| (a.mhz - mhz).abs().total_cmp(&(b.mhz - mhz).abs()))
}

/// The names of the `channels` within 2 MHz of `mhz`, joined ("R5 / A4").
pub fn fpv_names_at(channels: &[FpvChannel], mhz: f64) -> String {
    let mut v: Vec<&FpvChannel> = channels
        .iter()
        .filter(|c| (c.mhz - mhz).abs() <= 2.0)
        .collect();
    v.sort_by(|a, b| (a.mhz - mhz).abs().total_cmp(&(b.mhz - mhz).abs()));
    v.iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>()
        .join(" / ")
}

/// The boundary between the 1.2 GHz and the 5.8 GHz FPV channels, MHz.
const G12_BELOW_MHZ: f64 = 3000.0;

/// The centres a scan visits: every channel of the 1.2 GHz band (`g12`) or of the 5.8 GHz bands within 4.5 MHz of one
/// (sorted unique frequencies grouped from the first uncovered one up to 8 MHz above it; the centre the middle of the
/// group, rounded to 0.1 MHz).
pub fn fpv_scan_centres(channels: &[FpvChannel], g12: bool) -> Vec<f64> {
    let mut f: Vec<f64> = channels
        .iter()
        .filter(|c| (c.mhz < G12_BELOW_MHZ) == g12)
        .map(|c| c.mhz)
        .collect();
    f.sort_by(|a, b| a.total_cmp(b));
    f.dedup();
    let mut out = Vec::new();
    let mut i = 0;
    while i < f.len() {
        let first = f[i];
        let mut j = i;
        while j + 1 < f.len() && f[j + 1] - first <= 8.0 {
            j += 1;
        }
        let mid = (first + f[j]) / 2.0;
        out.push((mid * 10.0).round() / 10.0);
        i = j + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_channel_list_parses_and_finds_labels() {
        let ch = parse("E5=177500000; E21=474000000 ;5A=140500000;", 256).unwrap();
        assert_eq!(ch.len(), 3);
        assert_eq!(find(&ch, "21").unwrap().hz, 474e6);
        assert_eq!(find(&ch, "e21").unwrap().hz, 474e6);
        assert_eq!(find(&ch, "ch 21").unwrap().hz, 474e6);
        assert_eq!(find(&ch, "E5").unwrap().hz, 177.5e6);
        assert_eq!(find(&ch, "5A").unwrap().hz, 140.5e6);
        assert!(find(&ch, "99").is_none());
        assert_eq!(parse("", 256), Some(Vec::new()));
        for bad in ["21", "=474000000", "21=x", "21=-1", "21=inf"] {
            assert_eq!(parse(bad, 256), None, "{bad}");
        }
        let many: String = (0..=4).map(|i| format!("{i}=474000000;")).collect();
        assert_eq!(parse(&many, 4), None);
    }

    #[test]
    fn fpv_channels_take_band_and_number_from_the_label() {
        let ch = fpv_channels(&parse("R5=5806000000;A4=5805000000;G4=1200000000", 64).unwrap());
        assert_eq!(
            (ch[0].band.as_str(), ch[0].number, ch[0].mhz),
            ("R", 5, 5806.0)
        );
        assert_eq!(fpv_nearest(&ch, 5806.4, 4.0).unwrap().name, "R5");
        assert!(fpv_nearest(&ch, 3000.0, 4.0).is_none());
        let n = fpv_names_at(&ch, 5806.0);
        assert!(n.contains("R5") && n.contains("A4"), "{n}");
        assert_eq!(fpv_scan_centres(&ch, true), vec![1200.0]);
        assert_eq!(fpv_scan_centres(&ch, false), vec![5805.5]);
    }
}

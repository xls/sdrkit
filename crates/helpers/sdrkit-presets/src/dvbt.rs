//! The settings of the `dvbt` and `dvb_terrestrial` stacks from the TV band groups of the plan files (`plans/tv.json`):
//! the channel list and the channel width of a country.

use crate::data::Data;
use crate::preset::PlanChannel;
use sdrkit_core::Value;

/// The `dvbt` settings for `channels`: `channels`, the centre frequencies in Hz separated by ";", and `bandwidth`, the
/// width in MHz that every channel has ("8", "7", "6" or "5"), or "auto" when the widths differ or one is unknown.
pub fn dvbt_settings(channels: &[PlanChannel]) -> Vec<(String, Value)> {
    let hz: Vec<String> = channels
        .iter()
        .map(|c| format!("{:.0}", c.frequency_hz))
        .collect();
    let mhz = |c: &PlanChannel| c.width_hz.map(|w| (w / 1e6).round() as u32);
    let first = channels.first().and_then(mhz);
    let width = match first {
        Some(w) if (5..=8).contains(&w) && channels.iter().all(|c| mhz(c) == first) => {
            w.to_string()
        }
        _ => "auto".to_owned(),
    };
    vec![
        ("channels".to_owned(), Value::Text(hz.join(";"))),
        ("bandwidth".to_owned(), Value::Text(width)),
    ]
}

impl Data {
    /// The `dvbt` settings of the country `code` (ISO 3166 alpha-2, any case), see [`dvbt_settings`]. `None` if the
    /// country has no plan that lists DVB-T.
    pub fn dvbt_settings(&self, code: &str) -> Option<Vec<(String, Value)>> {
        let channels = self.tv_channels("dvbt", code);
        (!channels.is_empty()).then(|| dvbt_settings(&channels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
    }

    fn label_hz(d: &Data, code: &str, label: &str) -> Option<f64> {
        d.tv_channels("dvbt", code)
            .into_iter()
            .find(|c| c.label == label)
            .map(|c| c.frequency_hz)
    }

    #[test]
    fn the_dvbt_channels_of_a_country() {
        let d = data();
        let au = d.tv_channels("dvbt", "AU");
        assert_eq!(au.len(), 32);
        assert_eq!(au.first().map(|c| c.frequency_hz), Some(177.5e6));
        assert_eq!(label_hz(&d, "au", "9A"), Some(205.5e6));
        assert_eq!(label_hz(&d, "au", "28"), Some(529.5e6));
        assert_eq!(label_hz(&d, "au", "51"), Some(690.5e6));
        assert_eq!(label_hz(&d, "au", "52"), None);
        assert!(au.iter().all(|c| c.width_hz == Some(7e6)));
        // Germany uses Band III for DAB only (BNetzA): its channels are the UHF part, with no E5 to E12.
        let de = d.tv_channels("dvbt", "DE");
        assert_eq!(de.len(), 28);
        assert_eq!(de.first().map(|c| c.frequency_hz), Some(474e6));
        assert_eq!(label_hz(&d, "de", "48"), Some(690e6));
        assert_eq!(label_hz(&d, "de", "49"), None, "700 MHz band: mobile");
        assert_eq!(label_hz(&d, "de", "E12"), None);
        // France keeps both parts of the European plan: Band III at 7 MHz and UHF at 8 MHz.
        assert_eq!(label_hz(&d, "fr", "E12"), Some(226.5e6));
        assert_eq!(label_hz(&d, "fr", "21"), Some(474e6));
        assert!(!d.tv_channels("dvbt", "NZ").is_empty());
        assert!(d.tv_channels("dvbt", "US").is_empty(), "no DVB-T plan");
    }

    #[test]
    fn the_settings_of_a_country() {
        let d = data();
        let text = |code: &str, key: &str| {
            d.dvbt_settings(code)
                .and_then(|s| s.into_iter().find(|(k, _)| k == key))
                .map(|(_, v)| v)
        };
        assert_eq!(text("AU", "bandwidth"), Some(Value::Text("7".into())));
        assert_eq!(text("DE", "bandwidth"), Some(Value::Text("8".into())));
        assert_eq!(text("FR", "bandwidth"), Some(Value::Text("auto".into())));
        let Some(Value::Text(au)) = text("AU", "channels") else {
            panic!("a channel list")
        };
        assert_eq!(au.split(';').count(), 32);
        assert!(au.starts_with("177500000;"));
        assert_eq!(d.dvbt_settings("US"), None);
        // Mixed widths leave the width to the measurement.
        let mixed = [7e6, 8e6].map(|w| PlanChannel {
            label: String::new(),
            frequency_hz: w * 30.0,
            width_hz: Some(w),
        });
        assert_eq!(dvbt_settings(&mixed)[1].1, Value::Text("auto".into()));
    }
}

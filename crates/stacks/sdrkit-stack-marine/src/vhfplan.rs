//! The marine VHF channel plan that the host gives in the setting `plan`: the label and the frequencies of each channel.

use sdrkit_core::publish::field;
use sdrkit_core::Setting;

/// The most channels of a plan.
pub const PLAN_CAP: usize = 200;

/// The channel plan.
pub const PLAN: Setting<field::Text> = Setting::new("plan", "Channel plan", "", "").help(
    "The channels: entries separated by \";\", each a label, \"=\" and the frequency in Hz, or the ship and the coast frequency of a duplex pair separated by \"/\", for example \"16=156800000\" or \"1=156050000/160650000\". An entry that ends in \":data\" is a channel of data, left out of an empty scan list. At most 200 entries. The host fills it from a channel plan file. Empty: no channel has a label.",
);

/// One channel of the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// The label ("16", "22A").
    pub label: String,
    /// Where a ship transmits, Hz.
    pub ship_hz: f64,
    /// Where a coast station transmits, Hz. Equal to `ship_hz` for a simplex channel.
    pub coast_hz: f64,
    /// A voice channel (not DSC or AIS).
    pub voice: bool,
}

impl Channel {
    /// Simplex (one frequency).
    pub fn simplex(&self) -> bool {
        (self.ship_hz - self.coast_hz).abs() < 1.0
    }
    /// The frequencies to listen on (Hz): both halves of a duplex pair, or the one simplex frequency.
    pub fn listen_hz(&self) -> Vec<f64> {
        if self.simplex() {
            vec![self.ship_hz]
        } else {
            vec![self.ship_hz, self.coast_hz]
        }
    }
}

/// A channel plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// The channels in the order of the setting.
    pub channels: Vec<Channel>,
}

/// A frequency of a plan entry: positive and finite.
fn hz(text: &str) -> Option<f64> {
    let v: f64 = text.trim().parse().ok()?;
    (v.is_finite() && v > 0.0).then_some(v)
}

impl Plan {
    /// The plan of the text of the setting [`PLAN`]. `None` when an entry does not parse or there are more than
    /// [`PLAN_CAP`].
    pub fn parse(text: &str) -> Option<Plan> {
        let mut channels = Vec::new();
        for e in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
            let (label, rest) = e.split_once('=')?;
            let label = label.trim();
            let (freqs, voice) = match rest.rsplit_once(':') {
                Some((f, kind)) if kind.trim() == "data" => (f, false),
                Some(_) => return None,
                None => (rest, true),
            };
            let (ship_hz, coast_hz) = match freqs.split_once('/') {
                Some((s, c)) => (hz(s)?, hz(c)?),
                None => (hz(freqs)?, hz(freqs)?),
            };
            if label.is_empty() {
                return None;
            }
            channels.push(Channel {
                label: label.to_string(),
                ship_hz,
                coast_hz,
                voice,
            });
        }
        (channels.len() <= PLAN_CAP).then_some(Plan { channels })
    }

    /// The channel of frequency `hz` (ship or coast side, within 0.5 kHz) and whether it is the coast side.
    pub fn by_frequency(&self, hz: f64) -> Option<(&Channel, bool)> {
        let near = |f: f64| (f - hz).abs() <= 500.0;
        self.channels
            .iter()
            .find(|c| near(c.ship_hz))
            .map(|c| (c, false))
            .or_else(|| {
                self.channels
                    .iter()
                    .find(|c| near(c.coast_hz))
                    .map(|c| (c, true))
            })
    }

    /// The label of the channel of frequency `hz`.
    pub fn label_of(&self, hz: f64) -> Option<&str> {
        self.by_frequency(hz).map(|(c, _)| c.label.as_str())
    }

    /// The frequencies to listen on for `hz`: both halves of the plan channel that holds it, else `hz` alone.
    pub fn listen_hz(&self, hz: f64) -> Vec<f64> {
        self.by_frequency(hz)
            .map_or_else(|| vec![hz], |(c, _)| c.listen_hz())
    }

    /// The voice channels in plan order.
    pub fn voice_channels(&self) -> impl Iterator<Item = &Channel> {
        self.channels.iter().filter(|c| c.voice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_text_parses() {
        let p = Plan::parse("16=156800000; 1=156050000/160650000 ;70=156525000:data;").unwrap();
        assert_eq!(p.channels.len(), 3);
        assert!(p.channels[0].simplex() && p.channels[0].voice);
        assert_eq!(p.channels[1].listen_hz(), vec![156.05e6, 160.65e6]);
        assert!(!p.channels[2].voice);
        assert_eq!(
            p.by_frequency(160.65e6)
                .map(|(c, coast)| (c.label.as_str(), coast)),
            Some(("1", true))
        );
        assert_eq!(p.label_of(156.8003e6), Some("16"));
        assert_eq!(p.listen_hz(156.05e6), vec![156.05e6, 160.65e6]);
        assert_eq!(p.listen_hz(157.0e6), vec![157.0e6]);
        assert_eq!(p.voice_channels().count(), 2);
        assert_eq!(Plan::parse(""), Some(Plan::default()));
        for bad in [
            "16",
            "=156800000",
            "16=x",
            "16=-1",
            "16=156800000:voice",
            "1=156050000/",
        ] {
            assert_eq!(Plan::parse(bad), None, "{bad}");
        }
        let many: String = (0..=PLAN_CAP).map(|i| format!("{i}=156000000;")).collect();
        assert_eq!(Plan::parse(&many), None);
    }
}

//! The setting `channels` of the analogue video stacks from the plan files: the channels of an analogue TV plan
//! (`plans/analog_tv.json`, stack `atv`) and the FPV video channels (`plans/fpv.json`, stack `fpv`).

use crate::data::Data;
use crate::preset::PlanChannel;
use sdrkit_core::Value;
use sdrkit_data_plans::{band_channels, BandGroup, PlanKind};

/// The id of the group of the FPV video channels.
const FPV_GROUP: &str = "fpv_video";

/// The channels of one band group, for the setting `channels`.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelList {
    /// The id of the group (`an_bg`, `fpv_video`).
    pub id: String,
    /// The name of the group.
    pub name: String,
    /// The television system letters of an analogue TV plan ("B/G", "I", "M"); empty for FPV.
    pub system: String,
    /// The colour systems of an analogue TV plan ("PAL, SECAM"); empty for FPV.
    pub colour: String,
    /// The channels in use, in the order of the group.
    pub channels: Vec<PlanChannel>,
}

impl ChannelList {
    /// The channels in use of every band of `g`, in order.
    fn of(g: &BandGroup) -> ChannelList {
        let channels = g
            .bands
            .iter()
            .flat_map(band_channels)
            .filter(|c| !c.reallocated)
            .map(|c| PlanChannel {
                label: c.name,
                frequency_hz: c.centre_hz,
                width_hz: (c.width_hz > 0.0).then_some(c.width_hz),
            })
            .collect();
        let (system, colour) = g
            .analog
            .as_ref()
            .map(|a| (a.system.clone(), a.colour.clone()))
            .unwrap_or_default();
        ChannelList {
            id: g.id.clone(),
            name: g.name.clone(),
            system,
            colour,
            channels,
        }
    }

    /// The text of the setting `channels`: entries `label=Hz` separated by ";".
    pub fn text(&self) -> String {
        self.channels
            .iter()
            .map(|c| format!("{}={}", c.label, c.frequency_hz))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The settings of the list: `channels`.
    pub fn settings(&self) -> Vec<(String, Value)> {
        vec![("channels".to_owned(), Value::Text(self.text()))]
    }
}

impl Data {
    /// Every analogue TV plan of the plan files, in their order.
    pub fn atv_plans(&self) -> Vec<ChannelList> {
        self.plans
            .groups()
            .filter(|g| g.kind == PlanKind::AnalogTv)
            .map(ChannelList::of)
            .collect()
    }

    /// The analogue TV plan with the group id `id`.
    pub fn atv_plan(&self, id: &str) -> Option<ChannelList> {
        self.plans
            .group(id.trim())
            .filter(|g| g.kind == PlanKind::AnalogTv)
            .map(ChannelList::of)
    }

    /// The analogue TV plan of the country `code` (ISO 3166 alpha-2, any case). `None` if the country has none.
    pub fn atv_plan_of_country(&self, code: &str) -> Option<ChannelList> {
        let found = self
            .plans
            .find("atv", &code.trim().to_ascii_uppercase(), true)?;
        self.atv_plan(&found.id)
    }

    /// The FPV video channels. `None` if the plan files lack the group `fpv_video`.
    pub fn fpv_channels(&self) -> Option<ChannelList> {
        self.plans.group(FPV_GROUP).map(ChannelList::of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
    }

    fn hz(list: &ChannelList, label: &str) -> f64 {
        list.channels
            .iter()
            .find(|c| c.label == label)
            .unwrap_or_else(|| panic!("{}: no channel {label}", list.id))
            .frequency_hz
    }

    #[test]
    fn every_analogue_tv_plan_of_the_file_loads() {
        let d = data();
        let all = d.atv_plans();
        assert_eq!(all.len(), 11);
        for p in &all {
            assert!(!p.channels.is_empty() && !p.system.is_empty(), "{}", p.id);
            assert!(
                p.channels
                    .iter()
                    .all(|c| c.frequency_hz > 40e6 && c.frequency_hz < 1e9),
                "{}",
                p.id
            );
            assert_eq!(p.text().split(';').count(), p.channels.len(), "{}", p.id);
        }
        let bg = d.atv_plan("an_bg").unwrap();
        assert_eq!(hz(&bg, "21"), 474e6);
        assert_eq!(hz(&bg, "E5"), 177.5e6);
        assert_eq!(bg.system, "B/G");
        assert_eq!(hz(&d.atv_plan("an_au").unwrap(), "5A"), 140.5e6);
        assert!(d.atv_plan("an_j").is_some());
        assert!(d.atv_plan("fpv_video").is_none());
        for (cc, id) in [
            ("IE", "an_i"),
            ("fr", "an_l"),
            ("GB", "an_i"),
            ("US", "an_m"),
        ] {
            assert_eq!(d.atv_plan_of_country(cc).unwrap().id, id, "{cc}");
        }
        assert!(d.atv_plan_of_country("zz").is_none());
        let s = bg.settings();
        assert_eq!(s.len(), 1);
        assert!(
            matches!(&s[0].1, Value::Text(t) if t.contains("E21=474000000") || t.contains("21=474000000"))
        );
    }

    #[test]
    fn every_fpv_channel_of_the_file_loads() {
        let f = data().fpv_channels().unwrap();
        assert_eq!(f.channels.len(), 56);
        let mut names: Vec<&str> = f.channels.iter().map(|c| c.label.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 56);
        for (n, mhz) in [
            ("R1", 5658.0),
            ("R5", 5806.0),
            ("R8", 5917.0),
            ("A1", 5865.0),
            ("A8", 5725.0),
            ("B1", 5733.0),
            ("B8", 5866.0),
            ("E1", 5705.0),
            ("E5", 5885.0),
            ("E8", 5945.0),
            ("F1", 5740.0),
            ("F8", 5880.0),
            ("L1", 5362.0),
            ("L8", 5621.0),
            ("G1", 1080.0),
            ("G8", 1360.0),
        ] {
            assert_eq!(hz(&f, n), mhz * 1e6, "{n}");
        }
        for k in 1..8 {
            assert!(
                (hz(&f, &format!("R{}", k + 1)) - hz(&f, &format!("R{k}")) - 37e6).abs() <= 1e6
            );
            assert_eq!(
                hz(&f, &format!("F{}", k + 1)) - hz(&f, &format!("F{k}")),
                20e6
            );
        }
        assert!(f.text().contains("R5=5806000000"));
    }
}

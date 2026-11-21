//! The channels of the digital television stacks (`dvbt`, `dvbt2`, `isdbt`, `atsc`, `atsc3`, `dtmb`) in a country, from
//! the TV band groups of the plan files (`plans/tv.json`). A host tunes a stack to a channel of the list.

use crate::data::Data;
use crate::preset::PlanChannel;

impl Data {
    /// The channels that the country `code` (ISO 3166 alpha-2, any case) has for the television stack `stack`, in the
    /// order of the plan. Empty if the country has no plan that lists the stack.
    pub fn tv_channels(&self, stack: &str, code: &str) -> Vec<PlanChannel> {
        self.plans
            .find(stack, &code.trim().to_ascii_uppercase(), true)
            .map(|f| f.channels)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ISDB-T signal is this far above the centre of its channel.
    const ISDBT_OFFSET_HZ: f64 = 1e6 / 7.0;

    fn channel(d: &Data, stack: &str, code: &str, label: &str) -> f64 {
        d.tv_channels(stack, code)
            .into_iter()
            .find(|c| c.label == label)
            .unwrap_or_else(|| panic!("{stack} {code}: no channel {label}"))
            .frequency_hz
    }

    #[test]
    fn the_tv_plans_serve_every_terrestrial_stack() {
        let d = Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
        // ISDB-T channel 14 of Brazil and Japan's channel 13 carry the 1/7 MHz offset.
        assert!((channel(&d, "isdbt", "br", "14") - (473e6 + ISDBT_OFFSET_HZ)).abs() < 1.0);
        assert!((channel(&d, "isdbt", "jp", "13") - (473e6 + ISDBT_OFFSET_HZ)).abs() < 1.0);
        // ATSC channel 14 of the United States, DTMB channel 21 of China.
        assert_eq!(channel(&d, "atsc", "us", "14"), 473e6);
        assert_eq!(channel(&d, "dtmb", "cn", "DS21"), 538e6);
        assert!(d.tv_channels("atsc", "zz").is_empty());
    }
}

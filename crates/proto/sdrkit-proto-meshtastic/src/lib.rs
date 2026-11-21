//! Meshtastic modem presets and frequency slots: the presets, the band of the slots, the slot that a channel takes, and the
//! list of slots of a preset in a band. The stacks and the host helpers share this code; it holds no regional data.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// A modem preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    /// Its name.
    pub name: &'static str,
    /// Spreading factor.
    pub sf: u8,
    /// Bandwidth, Hz (the 2.4 GHz band changes it).
    pub bw: f64,
    /// Coding rate 5 to 8.
    pub cr: u8,
    /// The name of its default channel.
    pub channel: &'static str,
}

/// The presets, in the firmware's order of the preset search, EdgeFastLow last.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "LongFast",
        sf: 11,
        bw: 250e3,
        cr: 5,
        channel: "LongFast",
    },
    Preset {
        name: "LongSlow",
        sf: 12,
        bw: 125e3,
        cr: 8,
        channel: "LongSlow",
    },
    Preset {
        name: "MediumSlow",
        sf: 10,
        bw: 250e3,
        cr: 5,
        channel: "MediumSlow",
    },
    Preset {
        name: "MediumFast",
        sf: 9,
        bw: 250e3,
        cr: 5,
        channel: "MediumFast",
    },
    Preset {
        name: "ShortSlow",
        sf: 8,
        bw: 250e3,
        cr: 5,
        channel: "ShortSlow",
    },
    Preset {
        name: "ShortFast",
        sf: 7,
        bw: 250e3,
        cr: 5,
        channel: "ShortFast",
    },
    Preset {
        name: "LongModerate",
        sf: 11,
        bw: 125e3,
        cr: 8,
        channel: "LongMod",
    },
    Preset {
        name: "ShortTurbo",
        sf: 7,
        bw: 500e3,
        cr: 5,
        channel: "ShortTurbo",
    },
    Preset {
        name: "LongTurbo",
        sf: 11,
        bw: 500e3,
        cr: 8,
        channel: "LongTurbo",
    },
    Preset {
        name: "EdgeFastLow",
        sf: 8,
        bw: 62.5e3,
        cr: 8,
        channel: "EdgeFastLow",
    },
];

/// The preset of a name, matched without case, blanks, "_" or "-" ("LongMod" is LongModerate).
pub fn preset(name: &str) -> Option<&'static Preset> {
    let k = norm(name);
    if k == "longmod" {
        return PRESETS.iter().find(|p| p.name == "LongModerate");
    }
    PRESETS.iter().find(|p| norm(p.name) == k)
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The band in which the name of the primary channel chooses a frequency slot: the band of the network, which the host
/// sets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlotBand {
    /// The lower edge, Hz.
    pub start_hz: f64,
    /// The upper edge, Hz.
    pub end_hz: f64,
}

/// The lowest frequency of the 2.4 GHz LoRa modems, whose presets take wider bandwidths.
pub const WIDE_LORA_FROM_HZ: f64 = 2.4e9;

impl SlotBand {
    /// The band lies in the 2.4 GHz LoRa band.
    pub fn wide(&self) -> bool {
        self.start_hz >= WIDE_LORA_FROM_HZ
    }
}

/// The preset's bandwidth in a band (2.4 GHz: 1625, 406.25 or 812.5 kHz).
pub fn preset_bw(p: &Preset, band: &SlotBand) -> f64 {
    if band.wide() {
        if p.bw == 500e3 {
            1625e3
        } else if p.bw == 125e3 {
            406.25e3
        } else {
            812.5e3
        }
    } else {
        p.bw
    }
}

/// djb2 of a name.
pub fn djb2(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for &c in s.as_bytes() {
        h = h.wrapping_mul(33).wrapping_add(c as u32);
    }
    h
}

/// The number of frequency slots of a preset in a band: the band width over the preset's bandwidth, rounded down. 0 when
/// the band is narrower than one channel.
pub fn slot_count(p: &Preset, band: &SlotBand) -> u32 {
    let n = ((band.end_hz - band.start_hz) / preset_bw(p, band) + 1e-6).floor();
    if n.is_nan() || n < 1.0 {
        0
    } else {
        n.min(f64::from(u32::MAX)) as u32
    }
}

/// The 0-based slot that a channel takes among `count` slots: the user's channel number (1-based, 0 automatic) modulo the
/// count, else the djb2 hash of the channel name modulo the count. An empty name takes the preset's default channel name.
/// `None` when `count` is 0.
pub fn slot_index(p: &Preset, count: u32, channel_name: &str, channel_num: u32) -> Option<u32> {
    if count == 0 {
        return None;
    }
    let name = if channel_name.is_empty() {
        p.channel
    } else {
        channel_name
    };
    Some(if channel_num > 0 {
        (channel_num - 1) % count
    } else {
        djb2(name) % count
    })
}

/// The centre of the 0-based slot `index` of a preset in a band, Hz, not rounded.
pub fn slot_centre_hz(p: &Preset, band: &SlotBand, index: u32) -> f64 {
    let bw = preset_bw(p, band);
    band.start_hz + bw / 2.0 + f64::from(index) * bw
}

/// The slot frequency (Hz, rounded to the hertz) of a preset in a band with the primary channel's name (empty: the preset's
/// default channel name), the user's channel number (1-based, 0 automatic), the frequency offset and an override frequency (Hz, 0
/// none). `None` when the band is narrower than one channel.
pub fn slot_hz(
    p: &Preset,
    band: &SlotBand,
    channel_name: &str,
    channel_num: u32,
    offset_hz: f64,
    override_hz: f64,
) -> Option<f64> {
    if override_hz > 0.0 {
        return Some(override_hz.round());
    }
    let ch = slot_index(p, slot_count(p, band), channel_name, channel_num)?;
    Some((slot_centre_hz(p, band, ch) + offset_hz).round())
}

/// The frequency slots of a Meshtastic preset in a band, with the slot of the channel settings and the preset's default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SlotGrid {
    /// The index of the preset in [`PRESETS`].
    pub preset: usize,
    /// The lower edge of the band, Hz.
    pub start_hz: f64,
    /// The upper edge of the band, Hz.
    pub end_hz: f64,
    /// The frequency offset added to every slot, Hz.
    pub offset_hz: f64,
    /// The 0-based slot that the stack listens on: that of the channel settings, or with an override frequency the slot
    /// whose frequency it is; `None` for an override frequency that is no slot's.
    pub current: Option<u32>,
    /// The 0-based slot of the preset's default channel name.
    pub preset_default: Option<u32>,
}

/// The most Meshtastic slots that a band lists, and the largest value of the setting `mt_channel_num`.
pub const SLOTS_CAP: usize = 1024;

/// One slot of a [`SlotGrid`], as a row of the table `slots` gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotRow {
    /// The slot number, from 1.
    pub slot: u32,
    /// The frequency of the slot with the offset, Hz.
    pub freq_hz: f64,
    /// The stack listens on the slot.
    pub current: bool,
    /// The slot of the preset's default channel name.
    pub preset_default: bool,
}

/// The channel settings that choose a slot: the primary channel's name (empty: the preset's), the channel number (from 1,
/// 0: from the name), the frequency offset and an override frequency (Hz, 0 none).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlotChoice<'a> {
    /// The primary channel's name; empty takes the preset's.
    pub channel_name: &'a str,
    /// The channel number, from 1; 0 takes the slot from the name.
    pub channel_num: u32,
    /// The frequency offset added to every slot, Hz.
    pub offset_hz: f64,
    /// A frequency in place of the slot's, Hz; 0 for none.
    pub override_hz: f64,
}

/// A channel number above the slots of a preset in a band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelNumTooHigh {
    /// The channel number given.
    pub channel_num: u32,
    /// The number of slots.
    pub slots: u32,
}

impl std::fmt::Display for ChannelNumTooHigh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "channel number {} is above the {} slots of the band",
            self.channel_num, self.slots
        )
    }
}

impl std::error::Error for ChannelNumTooHigh {}

/// Checks that the channel number `channel_num` (from 1, 0: from the name) names a slot of preset `p` in `band`.
///
/// # Errors
///
/// [`ChannelNumTooHigh`] for a number above the slots.
pub fn check_channel_num(
    p: &Preset,
    band: &SlotBand,
    channel_num: u32,
) -> Result<(), ChannelNumTooHigh> {
    let slots = slot_count(p, band);
    if channel_num > slots {
        return Err(ChannelNumTooHigh { channel_num, slots });
    }
    Ok(())
}

impl SlotGrid {
    /// The slots of preset `p` in `band` with the channel settings `c`. The current slot is that of the settings; with an
    /// override frequency it is the slot whose frequency the override is, if one is.
    pub fn new(p: &Preset, band: &SlotBand, c: &SlotChoice<'_>) -> SlotGrid {
        let count = slot_count(p, band);
        let mut g = SlotGrid {
            preset: PRESETS.iter().position(|x| x.name == p.name).unwrap_or(0),
            start_hz: band.start_hz,
            end_hz: band.end_hz,
            offset_hz: c.offset_hz,
            current: slot_index(p, count, c.channel_name, c.channel_num),
            preset_default: slot_index(p, count, "", 0),
        };
        if c.override_hz > 0.0 {
            g.current = g.slot_at(c.override_hz);
        }
        g
    }

    fn preset(&self) -> &'static Preset {
        PRESETS.get(self.preset).unwrap_or(&PRESETS[0])
    }

    fn band(&self) -> SlotBand {
        SlotBand {
            start_hz: self.start_hz,
            end_hz: self.end_hz,
        }
    }

    /// The number of slots.
    pub fn count(&self) -> u32 {
        slot_count(self.preset(), &self.band())
    }

    /// The frequency of the 0-based slot `index`, Hz, rounded to the hertz as [`slot_hz`] rounds.
    pub fn freq_hz(&self, index: u32) -> f64 {
        (slot_centre_hz(self.preset(), &self.band(), index) + self.offset_hz).round()
    }

    /// The 0-based slot whose frequency is `hz` after rounding to the hertz; `None` if no slot has it.
    pub fn slot_at(&self, hz: f64) -> Option<u32> {
        let bw = preset_bw(self.preset(), &self.band());
        let centre0 = slot_centre_hz(self.preset(), &self.band(), 0);
        let i = ((hz - self.offset_hz - centre0) / bw).round();
        if !(0.0..f64::from(self.count())).contains(&i) {
            return None;
        }
        let i = i as u32;
        (self.freq_hz(i) == hz.round()).then_some(i)
    }

    /// The slots, lowest first, at most [`SLOTS_CAP`].
    pub fn rows(&self) -> impl Iterator<Item = SlotRow> + '_ {
        (0..self.count().min(SLOTS_CAP as u32)).map(|i| SlotRow {
            slot: i + 1,
            freq_hz: self.freq_hz(i),
            current: self.current == Some(i),
            preset_default: self.preset_default == Some(i),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const US: SlotBand = SlotBand {
        start_hz: 902e6,
        end_hz: 928e6,
    };

    #[test]
    fn the_default_channel_of_longfast_takes_slot_20_of_104_in_the_us_band() {
        let p = preset("LongFast").unwrap();
        let g = SlotGrid::new(p, &US, &SlotChoice::default());
        assert_eq!(g.count(), 104);
        assert_eq!((g.current, g.preset_default), (Some(19), Some(19)));
        assert_eq!(g.freq_hz(19), 906.875e6);
        assert_eq!(slot_hz(p, &US, "", 0, 0.0, 0.0), Some(906.875e6));
        let rows: Vec<_> = g.rows().collect();
        assert_eq!(rows.len(), 104);
        assert!(rows[19].current && rows[19].preset_default && rows[19].slot == 20);
    }

    #[test]
    fn a_channel_number_moves_the_slot_and_one_above_the_slots_is_too_high() {
        let p = preset("LongSlow").unwrap();
        assert_eq!(slot_count(p, &US), 208);
        assert_eq!(check_channel_num(p, &US, 208), Ok(()));
        assert_eq!(
            check_channel_num(p, &US, 209),
            Err(ChannelNumTooHigh {
                channel_num: 209,
                slots: 208
            })
        );
        let c = SlotChoice {
            channel_num: 7,
            ..SlotChoice::default()
        };
        assert_eq!(SlotGrid::new(p, &US, &c).current, Some(6));
    }

    #[test]
    fn an_override_at_a_slot_centre_makes_that_slot_current() {
        let p = preset("LongFast").unwrap();
        let at = |hz: f64| {
            let c = SlotChoice {
                override_hz: hz,
                ..SlotChoice::default()
            };
            SlotGrid::new(p, &US, &c).current
        };
        assert_eq!(at(902.625e6), Some(2));
        assert_eq!(at(902.6e6), None);
        assert_eq!(at(950e6), None);
    }
}

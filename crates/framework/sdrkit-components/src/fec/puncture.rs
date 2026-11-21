//! `fec.puncture` and `fec.depuncture`: a repeating keep pattern over a code stream, and the inverse that puts an erasure back in
//! every removed place.

use crate::bits::BitItem;
use sdrkit_algo_fec::conv::{codes, puncture, Depuncturer, Puncture};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, ChannelsFrom, Component, ComponentInfo, Format, Io, Port, TagKind};
use std::marker::PhantomData;

/// The DAB puncturing vectors by name (`dab_pi_1` is `PI_1`).
const DAB_PI_NAMES: [&str; 24] = [
    "dab_pi_1",
    "dab_pi_2",
    "dab_pi_3",
    "dab_pi_4",
    "dab_pi_5",
    "dab_pi_6",
    "dab_pi_7",
    "dab_pi_8",
    "dab_pi_9",
    "dab_pi_10",
    "dab_pi_11",
    "dab_pi_12",
    "dab_pi_13",
    "dab_pi_14",
    "dab_pi_15",
    "dab_pi_16",
    "dab_pi_17",
    "dab_pi_18",
    "dab_pi_19",
    "dab_pi_20",
    "dab_pi_21",
    "dab_pi_22",
    "dab_pi_23",
    "dab_pi_24",
];
/// The DRM tail patterns by name (`drm_tail_0` is `r_p = 0`).
const DRM_TAIL_NAMES: [&str; 12] = [
    "drm_tail_0",
    "drm_tail_1",
    "drm_tail_2",
    "drm_tail_3",
    "drm_tail_4",
    "drm_tail_5",
    "drm_tail_6",
    "drm_tail_7",
    "drm_tail_8",
    "drm_tail_9",
    "drm_tail_10",
    "drm_tail_11",
];

/// Every named puncturing pattern of `sdrkit_algo_fec::conv::codes`, by the constant's name in snake case, and `custom`.
pub fn patterns() -> Vec<(&'static str, Option<Puncture<'static>>)> {
    let mut v: Vec<(&'static str, Option<Puncture<'static>>)> = vec![
        ("none_1_2", Some(codes::NONE_1_2)),
        ("dvb_2_3", Some(codes::DVB_2_3)),
        ("dvb_3_4", Some(codes::DVB_3_4)),
        ("dvb_5_6", Some(codes::DVB_5_6)),
        ("dvb_7_8", Some(codes::DVB_7_8)),
        ("wifi_2_3", Some(codes::WIFI_2_3)),
        ("wifi_3_4", Some(codes::WIFI_3_4)),
        ("wifi_5_6", Some(codes::WIFI_5_6)),
        ("ccsds_2_3", Some(codes::CCSDS_2_3)),
        ("ccsds_3_4", Some(codes::CCSDS_3_4)),
        ("ccsds_5_6", Some(codes::CCSDS_5_6)),
        ("ccsds_7_8", Some(codes::CCSDS_7_8)),
        ("drm_1_6", Some(codes::DRM_1_6)),
        ("drm_1_4", Some(codes::DRM_1_4)),
        ("drm_3_10", Some(codes::DRM_3_10)),
        ("drm_1_3", Some(codes::DRM_1_3)),
        ("drm_4_11", Some(codes::DRM_4_11)),
        ("drm_2_5", Some(codes::DRM_2_5)),
        ("drm_1_2", Some(codes::DRM_1_2)),
        ("drm_4_7", Some(codes::DRM_4_7)),
        ("drm_3_5", Some(codes::DRM_3_5)),
        ("drm_2_3", Some(codes::DRM_2_3)),
        ("drm_8_11", Some(codes::DRM_8_11)),
        ("drm_3_4", Some(codes::DRM_3_4)),
        ("drm_4_5", Some(codes::DRM_4_5)),
        ("drm_7_8", Some(codes::DRM_7_8)),
        ("drm_8_9", Some(codes::DRM_8_9)),
        ("dab_tail", Some(codes::DAB_TAIL)),
        ("m17_p1", Some(codes::M17_P1)),
        ("m17_p2", Some(codes::M17_P2)),
        ("m17_p3", Some(codes::M17_P3)),
        ("tetra_2_3", Some(codes::TETRA_2_3)),
        ("tetra_1_3", Some(codes::TETRA_1_3)),
        ("tetra_292_432", Some(codes::TETRA_292_432)),
        ("tetra_148_432", Some(codes::TETRA_148_432)),
        ("hd_fm_2_5", Some(codes::HD_FM_2_5)),
        ("hd_fm_1_2", Some(codes::HD_FM_1_2)),
        ("hd_fm_2_7", Some(codes::HD_FM_2_7)),
        ("hd_am_e1_5_12", Some(codes::HD_AM_E1_5_12)),
        ("hd_am_e2_2_3", Some(codes::HD_AM_E2_2_3)),
    ];
    v.extend(
        DAB_PI_NAMES
            .iter()
            .zip(codes::DAB_PI)
            .map(|(n, p)| (*n, Some(p))),
    );
    v.extend(
        DRM_TAIL_NAMES
            .iter()
            .zip(codes::DRM_TAIL)
            .map(|(n, p)| (*n, Some(p))),
    );
    v.push(("custom", None));
    v
}

sdrkit_core::choice_enum! {
    /// A puncturing pattern: a named pattern of [`patterns`], or `custom`.
    pub enum PunctureName {
        /// `none_1_2`.
        None12 = ("none_1_2", "none_1_2"),
        /// `dvb_2_3`.
        Dvb23 = ("dvb_2_3", "dvb_2_3"),
        /// `dvb_3_4`.
        Dvb34 = ("dvb_3_4", "dvb_3_4"),
        /// `dvb_5_6`.
        Dvb56 = ("dvb_5_6", "dvb_5_6"),
        /// `dvb_7_8`.
        Dvb78 = ("dvb_7_8", "dvb_7_8"),
        /// `wifi_2_3`.
        Wifi23 = ("wifi_2_3", "wifi_2_3"),
        /// `wifi_3_4`.
        Wifi34 = ("wifi_3_4", "wifi_3_4"),
        /// `wifi_5_6`.
        Wifi56 = ("wifi_5_6", "wifi_5_6"),
        /// `ccsds_2_3`.
        Ccsds23 = ("ccsds_2_3", "ccsds_2_3"),
        /// `ccsds_3_4`.
        Ccsds34 = ("ccsds_3_4", "ccsds_3_4"),
        /// `ccsds_5_6`.
        Ccsds56 = ("ccsds_5_6", "ccsds_5_6"),
        /// `ccsds_7_8`.
        Ccsds78 = ("ccsds_7_8", "ccsds_7_8"),
        /// `drm_1_6`.
        Drm16 = ("drm_1_6", "drm_1_6"),
        /// `drm_1_4`.
        Drm14 = ("drm_1_4", "drm_1_4"),
        /// `drm_3_10`.
        Drm310 = ("drm_3_10", "drm_3_10"),
        /// `drm_1_3`.
        Drm13 = ("drm_1_3", "drm_1_3"),
        /// `drm_4_11`.
        Drm411 = ("drm_4_11", "drm_4_11"),
        /// `drm_2_5`.
        Drm25 = ("drm_2_5", "drm_2_5"),
        /// `drm_1_2`.
        Drm12 = ("drm_1_2", "drm_1_2"),
        /// `drm_4_7`.
        Drm47 = ("drm_4_7", "drm_4_7"),
        /// `drm_3_5`.
        Drm35 = ("drm_3_5", "drm_3_5"),
        /// `drm_2_3`.
        Drm23 = ("drm_2_3", "drm_2_3"),
        /// `drm_8_11`.
        Drm811 = ("drm_8_11", "drm_8_11"),
        /// `drm_3_4`.
        Drm34 = ("drm_3_4", "drm_3_4"),
        /// `drm_4_5`.
        Drm45 = ("drm_4_5", "drm_4_5"),
        /// `drm_7_8`.
        Drm78 = ("drm_7_8", "drm_7_8"),
        /// `drm_8_9`.
        Drm89 = ("drm_8_9", "drm_8_9"),
        /// `dab_tail`.
        DabTail = ("dab_tail", "dab_tail"),
        /// `m17_p1`.
        M17P1 = ("m17_p1", "m17_p1"),
        /// `m17_p2`.
        M17P2 = ("m17_p2", "m17_p2"),
        /// `m17_p3`.
        M17P3 = ("m17_p3", "m17_p3"),
        /// `tetra_2_3`.
        Tetra23 = ("tetra_2_3", "tetra_2_3"),
        /// `tetra_1_3`.
        Tetra13 = ("tetra_1_3", "tetra_1_3"),
        /// `tetra_292_432`.
        Tetra292432 = ("tetra_292_432", "tetra_292_432"),
        /// `tetra_148_432`.
        Tetra148432 = ("tetra_148_432", "tetra_148_432"),
        /// `hd_fm_2_5`.
        HdFm25 = ("hd_fm_2_5", "hd_fm_2_5"),
        /// `hd_fm_1_2`.
        HdFm12 = ("hd_fm_1_2", "hd_fm_1_2"),
        /// `hd_fm_2_7`.
        HdFm27 = ("hd_fm_2_7", "hd_fm_2_7"),
        /// `hd_am_e1_5_12`.
        HdAmE1512 = ("hd_am_e1_5_12", "hd_am_e1_5_12"),
        /// `hd_am_e2_2_3`.
        HdAmE223 = ("hd_am_e2_2_3", "hd_am_e2_2_3"),
        /// `dab_pi_1`.
        DabPi1 = ("dab_pi_1", "dab_pi_1"),
        /// `dab_pi_2`.
        DabPi2 = ("dab_pi_2", "dab_pi_2"),
        /// `dab_pi_3`.
        DabPi3 = ("dab_pi_3", "dab_pi_3"),
        /// `dab_pi_4`.
        DabPi4 = ("dab_pi_4", "dab_pi_4"),
        /// `dab_pi_5`.
        DabPi5 = ("dab_pi_5", "dab_pi_5"),
        /// `dab_pi_6`.
        DabPi6 = ("dab_pi_6", "dab_pi_6"),
        /// `dab_pi_7`.
        DabPi7 = ("dab_pi_7", "dab_pi_7"),
        /// `dab_pi_8`.
        DabPi8 = ("dab_pi_8", "dab_pi_8"),
        /// `dab_pi_9`.
        DabPi9 = ("dab_pi_9", "dab_pi_9"),
        /// `dab_pi_10`.
        DabPi10 = ("dab_pi_10", "dab_pi_10"),
        /// `dab_pi_11`.
        DabPi11 = ("dab_pi_11", "dab_pi_11"),
        /// `dab_pi_12`.
        DabPi12 = ("dab_pi_12", "dab_pi_12"),
        /// `dab_pi_13`.
        DabPi13 = ("dab_pi_13", "dab_pi_13"),
        /// `dab_pi_14`.
        DabPi14 = ("dab_pi_14", "dab_pi_14"),
        /// `dab_pi_15`.
        DabPi15 = ("dab_pi_15", "dab_pi_15"),
        /// `dab_pi_16`.
        DabPi16 = ("dab_pi_16", "dab_pi_16"),
        /// `dab_pi_17`.
        DabPi17 = ("dab_pi_17", "dab_pi_17"),
        /// `dab_pi_18`.
        DabPi18 = ("dab_pi_18", "dab_pi_18"),
        /// `dab_pi_19`.
        DabPi19 = ("dab_pi_19", "dab_pi_19"),
        /// `dab_pi_20`.
        DabPi20 = ("dab_pi_20", "dab_pi_20"),
        /// `dab_pi_21`.
        DabPi21 = ("dab_pi_21", "dab_pi_21"),
        /// `dab_pi_22`.
        DabPi22 = ("dab_pi_22", "dab_pi_22"),
        /// `dab_pi_23`.
        DabPi23 = ("dab_pi_23", "dab_pi_23"),
        /// `dab_pi_24`.
        DabPi24 = ("dab_pi_24", "dab_pi_24"),
        /// `drm_tail_0`.
        DrmTail0 = ("drm_tail_0", "drm_tail_0"),
        /// `drm_tail_1`.
        DrmTail1 = ("drm_tail_1", "drm_tail_1"),
        /// `drm_tail_2`.
        DrmTail2 = ("drm_tail_2", "drm_tail_2"),
        /// `drm_tail_3`.
        DrmTail3 = ("drm_tail_3", "drm_tail_3"),
        /// `drm_tail_4`.
        DrmTail4 = ("drm_tail_4", "drm_tail_4"),
        /// `drm_tail_5`.
        DrmTail5 = ("drm_tail_5", "drm_tail_5"),
        /// `drm_tail_6`.
        DrmTail6 = ("drm_tail_6", "drm_tail_6"),
        /// `drm_tail_7`.
        DrmTail7 = ("drm_tail_7", "drm_tail_7"),
        /// `drm_tail_8`.
        DrmTail8 = ("drm_tail_8", "drm_tail_8"),
        /// `drm_tail_9`.
        DrmTail9 = ("drm_tail_9", "drm_tail_9"),
        /// `drm_tail_10`.
        DrmTail10 = ("drm_tail_10", "drm_tail_10"),
        /// `drm_tail_11`.
        DrmTail11 = ("drm_tail_11", "drm_tail_11"),
        /// `custom`.
        Custom = ("custom", "custom"),
    }
}

impl PunctureName {
    /// The named pattern, or `None` for `custom`. The variants are in the order of [`patterns`].
    fn pattern(self) -> Option<Puncture<'static>> {
        patterns()
            .into_iter()
            .nth(usize::from(self.index()))
            .and_then(|(_, p)| p)
    }
}

const PATTERN: Setting<field::Choice<PunctureName>> =
    Setting::choice("pattern", "Pattern", PunctureName::None12)
        .help("A named puncturing pattern, or custom (the keep setting).");
const KEEP: Setting<field::Text> = Setting::new("keep", "Custom pattern", "", "").help(
    "With pattern custom: 1 for each value sent and 0 for each removed, one period of the serial code stream.",
);
const PHASE: Setting<field::I64> = Setting::new("phase", "Phase", "", 0)
    .range(0.0, 4096.0, 1.0)
    .help("Position in the pattern of the first value (and of the value at each frame start).");

sdrkit_core::settings! {
    /// The settings of [`PunctureComponent`] and [`DepunctureComponent`].
    pub enum PunctureSetting {
        /// The pattern.
        Pattern = PATTERN,
        /// The custom pattern.
        Keep = KEEP,
        /// The phase.
        Phase = PHASE,
    }
}

/// Reads a custom keep pattern: `0` and `1`, with spaces and commas allowed between them.
fn parse_keep(id: SettingId, s: &str) -> Result<Vec<u8>, SettingError> {
    let mut keep = Vec::with_capacity(s.len());
    for (at, c) in s.chars().enumerate() {
        match c {
            '0' => keep.push(0),
            '1' => keep.push(1),
            ' ' | ',' => {}
            _ => return Err(SettingError::Malformed { id, at: Some(at) }),
        }
    }
    if !keep.is_empty() && Puncture::try_new(&keep).is_err() {
        return Err(SettingError::NotPermitted { id });
    }
    Ok(keep)
}

/// The settings shared by both directions.
#[derive(Clone, Debug)]
struct PatternSettings {
    name: PunctureName,
    custom: Vec<u8>,
    phase: usize,
    /// The pattern in use (built at configure).
    keep: Vec<u8>,
}

impl Default for PatternSettings {
    fn default() -> Self {
        PatternSettings {
            name: PATTERN.default,
            custom: Vec::new(),
            phase: PHASE.default as _,
            keep: vec![1, 1],
        }
    }
}

impl PatternSettings {
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PunctureSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PunctureSetting::Pattern => self.name = PATTERN.read(id, v)?,
            PunctureSetting::Keep => self.custom = parse_keep(id, KEEP.read(id, v)?)?,
            PunctureSetting::Phase => self.phase = PHASE.read(id, v)? as usize,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        let custom: String = self
            .custom
            .iter()
            .map(|&b| if b == 1 { '1' } else { '0' })
            .collect();
        values_of([
            (PunctureSetting::Pattern, Value::Choice(self.name.index())),
            (PunctureSetting::Keep, Value::Text(custom)),
            (PunctureSetting::Phase, Value::I64(self.phase as i64)),
        ])
    }

    /// Builds the pattern; returns (period, kept).
    fn build(&mut self) -> Result<(usize, usize), ConfigError> {
        self.keep = match self.name.pattern() {
            Some(p) => p.keep().to_vec(),
            None => self.custom.clone(),
        };
        let p = Puncture::try_new(&self.keep).map_err(|_| SettingError::Required {
            id: PunctureSetting::Keep.id(),
        })?;
        if self.phase >= p.period() {
            return Err(SettingError::Conflict {
                id: PunctureSetting::Phase.id(),
                with: PunctureSetting::Pattern.id(),
            }
            .into());
        }
        Ok((p.period(), p.kept()))
    }

    /// The positions (relative to `first`, below `len`) of the `FrameStart` tags in `tags`.
    fn frame_starts<'a>(
        tags: &'a [sdrkit_graph::Tag],
        first: u64,
        len: usize,
    ) -> impl Iterator<Item = usize> + 'a {
        tags.iter()
            .filter(|t| matches!(t.kind, TagKind::FrameStart { .. }))
            .map(move |t| t.at.saturating_sub(first) as usize)
            .filter(move |&r| r < len)
    }
}

/// Removes the values a puncturing pattern does not send (the transmit side, for generators and tests): `fec.puncture` on hard
/// bits and `fec.puncture_llr` on LLRs, the same kind out, at `kept / period` of the input rate. A `FrameStart` tag restarts the
/// pattern at `phase`.
#[derive(Debug, Default)]
pub struct PunctureComponent<T: BitItem> {
    s: PatternSettings,
    pos: usize,
    bits: PhantomData<T>,
}

impl<T: BitItem> PunctureComponent<T> {
    const NAMES: [&'static str; 2] = ["fec.puncture", "fec.puncture_llr"];
}

impl<T: BitItem> Component for PunctureComponent<T> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            Self::NAMES[T::INDEX],
            vec![Port::new("in", T::KIND)],
            vec![Port::new("out", T::KIND).channels_from(ChannelsFrom::Input(0))],
        )
        .with_settings(PunctureSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let (period, kept) = self.s.build()?;
        self.pos = self.s.phase;
        Ok(vec![
            inputs[0].with_rate(inputs[0].rate * kept as f64 / period as f64)
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let Ok(p) = Puncture::try_new(&self.s.keep) else {
            return;
        };
        let period = p.period();
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let len = io.input_len(0);
        let mut a = 0;
        let starts = PatternSettings::frame_starts(tags, first, len).chain(std::iter::once(len));
        for b in starts {
            if b > a {
                let x = T::read(io, 0);
                puncture(&p, self.pos, &x[a..b], T::write(io, 0));
                self.pos = (self.pos + (b - a)) % period;
            }
            if b < len {
                self.pos = self.s.phase;
            }
            a = b;
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.s.apply(id, v)
    }

    fn values(&self) -> Values {
        self.s.values()
    }
}

/// Puts an erasure (0) back in every place a puncturing pattern removed: `Llr` in and out, at `period / kept` of the input rate.
/// Each received value goes to the next kept place and the removed places after it are filled at once, so any cut of the stream
/// produces the same output. A `FrameStart` tag restarts the pattern at `phase` at its item.
#[derive(Debug, Default)]
pub struct Depuncture {
    s: PatternSettings,
    phase: usize,
}

impl Component for Depuncture {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fec.depuncture",
            vec![Port::llr("in")],
            vec![Port::llr("out")],
        )
        .with_settings(PunctureSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let (period, kept) = self.s.build()?;
        self.phase = self.s.phase;
        Ok(vec![
            inputs[0].with_rate(inputs[0].rate * period as f64 / kept as f64)
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let Ok(p) = Puncture::try_new(&self.s.keep) else {
            return;
        };
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = io.llrs(0);
        let out = io.llrs_out(0);
        let mut a = 0;
        let starts =
            PatternSettings::frame_starts(tags, first, x.len()).chain(std::iter::once(x.len()));
        for b in starts {
            if b > a {
                let mut d = Depuncturer::with_phase(p, self.phase);
                d.push(&x[a..b], out);
                self.phase = d.phase();
            }
            if b < x.len() {
                self.phase = self.s.phase;
            }
            a = b;
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.s.apply(id, v)
    }

    fn values(&self) -> Values {
        self.s.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_the_patterns() {
        let keys: Vec<&str> = PunctureName::CHOICES.iter().map(|c| &*c.key).collect();
        let names: Vec<&str> = patterns().into_iter().map(|(n, _)| n).collect();
        assert_eq!(keys, names);
    }
}

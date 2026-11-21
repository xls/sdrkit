//! The pulse analyser of `ism` (submode `pulse_analyser`, component `ism.pulse_analyser`): each OOK train of the burst front end
//! is cut into frames at long gaps (`sdrkit_proto_ookdev::guess::frame_ranges`), and each frame of enough pulses gets the line
//! code, the timings and the bits of `sdrkit_proto_ookdev::guess::Guesser::frame`. The analyser measures and guesses: it names a
//! line code, not a device, and checks no preamble, address or checksum.

use crate::typed::{IsmPulseFrameRow, PulseCode};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    Timed,
};
use sdrkit_proto_ookdev::guess::{bits_bin, bits_hex, frame_ranges, Guesser, LineCode};
use sdrkit_proto_ookdev::{Modulation, Train};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{Seen, Shift};
use serde::{Deserialize, Serialize};
use std::ops::Range;

sdrkit_core::choice_enum! {
    /// What `ism` does with the pulse trains it finds.
    pub enum Submode {
        /// The device protocols read the trains.
        Devices = ("devices", "Device decoders"),
        /// The pulse analyser reads the trains in place of the device protocols.
        PulseAnalyser = ("pulse_analyser", "OOK / ASK pulse analyser"),
    }
}

/// What `ism` does with the pulse trains it finds.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Submode", Submode::Devices).help(
        "The pulse analyser measures the frames of an unknown on-off keyed signal and guesses their line code; \
         it decodes no device.",
    );
/// The longest gap inside a frame.
pub const FRAME_GAP_MS: Setting<field::F64> =
    Setting::new("frame_gap_ms", "Longest gap inside a frame", "ms", 5.0)
        .range(1.0, 50.0, 0.5)
        .help("Pulse analyser: a longer gap ends a frame. After 4 pulses the limit is 12 times the median width (at least 1.2 ms), if that is shorter.");
/// The fewest pulses of a frame.
pub const MIN_PULSES: Setting<field::I64> =
    Setting::new("min_pulses", "Fewest pulses in a frame", "", 8)
        .range(4.0, 64.0, 1.0)
        .help("Pulse analyser: a frame with fewer pulses is not analysed.");
/// The split ratio of the clusters of widths and gaps.
pub const CLUSTER_RATIO: Setting<field::F64> =
    Setting::new("cluster_ratio", "Cluster split ratio", "", 1.5)
        .range(1.2, 2.5, 0.05)
        .help("Pulse analyser: sorted widths or gaps start a new cluster where a value is this many times the one before.");

sdrkit_core::settings! {
    /// The setting of the components that only follow the submode.
    pub enum SubmodeSetting {
        /// The submode.
        Submode = SUBMODE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Analyser`].
    pub enum AnalyserSetting {
        /// The submode.
        Submode = SUBMODE,
        /// The longest gap inside a frame.
        FrameGapMs = FRAME_GAP_MS,
        /// The fewest pulses of a frame.
        MinPulses = MIN_PULSES,
        /// The split ratio of the clusters.
        ClusterRatio = CLUSTER_RATIO,
    }
}

/// The bits a text field shows.
pub const SHOWN_BITS: usize = 96;
/// A frame with the same bits that ends within this time of the one before is a repeat, seconds.
pub const REPEAT_S: f64 = 0.5;

/// One analysed frame (`ism.pulse_frame`). A frame without a line code has the code [`LineCode::Unknown`], no bits and no
/// timings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PulseFrame {
    /// The signal time of the first pulse, seconds.
    pub time_s: f64,
    /// Hz from the input centre.
    pub offset_hz: f64,
    /// The frequency, Hz (the input centre plus the offset).
    #[serde(default)]
    pub freq_hz: f64,
    /// The level of the marks of the train, dBFS.
    pub level_dbfs: f32,
    /// The line code.
    pub code: LineCode,
    /// The bits, one value 0 or 1 each.
    pub bits: Vec<u8>,
    /// PWM: the short width. PPM: the short gap. µs.
    pub short_us: Option<f32>,
    /// PWM: the long width. PPM: the long gap. µs.
    pub long_us: Option<f32>,
    /// PPM: the pulse width, µs.
    pub pulse_us: Option<f32>,
    /// PWM: the bit period, µs.
    pub period_us: Option<f32>,
    /// Manchester: the half-bit. PCM: the unit. µs.
    pub unit_us: Option<f32>,
    /// Manchester and PCM: the bit rate, bit/s.
    pub bit_rate: Option<f64>,
    /// The pulses of the frame.
    pub pulses: usize,
    /// From the start of the first pulse to the end of the last, ms.
    pub frame_ms: f64,
    /// Frames in a row with the same bits, this one included (0 without a line code).
    pub repeats: u32,
}

impl Item for PulseFrame {
    const KIND: &'static str = "ism.pulse_frame";
}

impl Shift for PulseFrame {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

/// The choice of a line code with bits; `None` for [`LineCode::Unknown`].
pub fn pulse_code(c: LineCode) -> Option<PulseCode> {
    match c {
        LineCode::Pwm => Some(PulseCode::Pwm),
        LineCode::Ppm => Some(PulseCode::Ppm),
        LineCode::Manchester => Some(PulseCode::Manchester),
        LineCode::Nrz => Some(PulseCode::Pcm),
        LineCode::Unknown => None,
    }
}

impl PulseFrame {
    /// Whether the frame has a line code.
    pub fn good(&self) -> bool {
        self.code != LineCode::Unknown
    }

    /// The first 96 bits as 0 and 1, with "..." when there are more.
    pub fn bin(&self) -> String {
        bits_bin(&self.bits, SHOWN_BITS)
    }

    /// The first 96 bits as hex.
    pub fn hex(&self) -> String {
        bits_hex(&self.bits, SHOWN_BITS)
    }

    /// The Manchester half-bit, µs.
    pub fn half_bit_us(&self) -> Option<f64> {
        (self.code == LineCode::Manchester)
            .then_some(self.unit_us)
            .flatten()
            .map(f64::from)
    }

    /// The PCM unit, µs.
    pub fn pcm_unit_us(&self) -> Option<f64> {
        (self.code == LineCode::Nrz)
            .then_some(self.unit_us)
            .flatten()
            .map(f64::from)
    }

    /// "PWM, 24 bits: B2CA53, repeated 4 time(s)".
    pub fn headline(&self) -> String {
        let name = pulse_code(self.code).map_or("?", |c| c.label());
        format!(
            "{name}, {} bits: {}, repeated {} time(s)",
            self.bits.len(),
            self.hex(),
            self.repeats
        )
    }
}

impl Shaped for PulseFrame {
    type Row = IsmPulseFrameRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let Some(line_code) = pulse_code(self.code) else {
            return;
        };
        out.put(&IsmPulseFrameRow {
            time_s: self.time_s,
            frequency: Hertz(self.freq_hz),
            line_code,
            bits: self.bits.len() as i64,
            bits_bin: &self.bin(),
            bits_hex: &self.hex(),
            short_us: self.short_us.map(f64::from),
            long_us: self.long_us.map(f64::from),
            pulse_us: self.pulse_us.map(f64::from),
            period_us: self.period_us.map(f64::from),
            half_bit_us: self.half_bit_us(),
            unit_us: self.pcm_unit_us(),
            bit_rate: self.bit_rate,
            pulses: self.pulses as i64,
            frame_ms: self.frame_ms,
            repeats: i64::from(self.repeats),
            level_dbfs: f64::from(self.level_dbfs),
        });
    }
}

/// `ism.pulse_analyser`: the frames of each OOK train in the submode `pulse_analyser` (module docs); in the other submode it
/// reads nothing. A frame of at least `min_pulses` pulses leaves as a [`PulseFrame`], with or without a line code. Repeats:
/// a frame with the same bits as the frame with a line code before it, ending within 0.5 s of it, adds one; another
/// frame with a line code starts at 1.
pub struct Analyser {
    submode: Submode,
    frame_gap_ms: f64,
    min_pulses: i64,
    guesser: Guesser,
    ranges: Vec<Range<usize>>,
    scratch: Vec<f32>,
    pulses: Vec<(f32, f32)>,
    bits: Vec<u8>,
    /// The bits, the end time and the repeats of the last frame with a line code.
    last: Option<(Vec<u8>, f64, u32)>,
    out: Vec<Timed<PulseFrame>>,
}

impl Default for Analyser {
    fn default() -> Self {
        Analyser {
            submode: SUBMODE.default,
            frame_gap_ms: FRAME_GAP_MS.default,
            min_pulses: MIN_PULSES.default,
            guesser: Guesser::with_cluster_ratio(CLUSTER_RATIO.default as f32),
            ranges: Vec::new(),
            scratch: Vec::new(),
            pulses: Vec::new(),
            bits: Vec::new(),
            last: None,
            out: Vec::new(),
        }
    }
}

impl Analyser {
    /// Analyses the frames of `t` (type docs); each goes to `f`.
    pub fn run(&mut self, t: &Train, mut f: impl FnMut(PulseFrame)) {
        if t.modulation != Modulation::Ook || !t.is_valid() {
            return;
        }
        frame_ranges(
            t,
            (self.frame_gap_ms * 1e3) as f32,
            &mut self.scratch,
            &mut self.ranges,
        );
        let n = t.pulses.len();
        let mut at = 0.0f64;
        let mut next = 0usize;
        for r in std::mem::take(&mut self.ranges) {
            at += t.pulses[next..r.start]
                .iter()
                .map(|p| f64::from(p.mark) + f64::from(p.space))
                .sum::<f64>();
            next = r.start;
            if (r.len() as i64) < self.min_pulses {
                continue;
            }
            self.pulses.clear();
            self.pulses.extend(r.clone().map(|i| {
                let space = if i + 1 == n {
                    f32::INFINITY
                } else {
                    t.us(t.pulses[i].space)
                };
                (t.us(t.pulses[i].mark), space)
            }));
            let time_s = (t.start + at) / t.rate_hz;
            let mut frame = PulseFrame {
                time_s,
                offset_hz: t.offset_hz,
                freq_hz: 0.0,
                level_dbfs: t.level_dbfs,
                code: LineCode::Unknown,
                bits: Vec::new(),
                short_us: None,
                long_us: None,
                pulse_us: None,
                period_us: None,
                unit_us: None,
                bit_rate: None,
                pulses: r.len(),
                frame_ms: 0.0,
                repeats: 0,
            };
            if let Some(g) = self.guesser.frame(&self.pulses, &mut self.bits) {
                let end_s = time_s + f64::from(g.frame_us) * 1e-6;
                let repeats = match &self.last {
                    Some((b, e, k)) if *b == self.bits && end_s - e <= REPEAT_S => k + 1,
                    _ => 1,
                };
                self.last = Some((self.bits.clone(), end_s, repeats));
                frame = PulseFrame {
                    code: g.code,
                    bits: self.bits.clone(),
                    short_us: g.short_us,
                    long_us: g.long_us,
                    pulse_us: g.pulse_us,
                    period_us: g.period_us,
                    unit_us: g.unit_us,
                    bit_rate: g.bit_rate(),
                    frame_ms: f64::from(g.frame_us) * 1e-3,
                    repeats,
                    ..frame
                };
            } else {
                frame.frame_ms = self.pulses.iter().map(|p| f64::from(p.0)).sum::<f64>() * 1e-3
                    + self.pulses[..self.pulses.len() - 1]
                        .iter()
                        .map(|p| f64::from(p.1))
                        .sum::<f64>()
                        * 1e-3;
            }
            f(frame);
        }
    }
}

impl Component for Analyser {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ism.pulse_analyser",
            vec![Port::typed::<Train>("trains")],
            vec![Port::typed::<PulseFrame>("frames").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(AnalyserSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last = None;
        Ok(vec![inputs[0].with_kind(Kind::typed::<PulseFrame>())])
    }
    fn process(&mut self, io: &mut Io) {
        if self.submode != Submode::PulseAnalyser {
            return;
        }
        let trains = io.items::<Train>(0);
        if trains.is_empty() {
            return;
        }
        let mut out = std::mem::take(&mut self.out);
        for t in trains {
            self.run(&t.value, |f| {
                out.push(Timed {
                    time: f.time_s,
                    value: f,
                })
            });
        }
        if !out.is_empty() {
            io.items_out::<PulseFrame>(0).append(&mut out);
        }
        self.out = out;
    }
    /// Drops the frame that the next one is compared with for its repeats.
    fn forget(&mut self, _kept: &Kept) {
        self.last = None;
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match AnalyserSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AnalyserSetting::Submode => {
                self.submode = SUBMODE.read(id, value)?;
                self.last = None;
            }
            AnalyserSetting::FrameGapMs => self.frame_gap_ms = FRAME_GAP_MS.read(id, value)?,
            AnalyserSetting::MinPulses => self.min_pulses = MIN_PULSES.read(id, value)?,
            AnalyserSetting::ClusterRatio => {
                self.guesser
                    .set_cluster_ratio(CLUSTER_RATIO.read(id, value)? as f32);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                AnalyserSetting::Submode,
                Value::Choice(self.submode.index()),
            ),
            (AnalyserSetting::FrameGapMs, Value::F64(self.frame_gap_ms)),
            (AnalyserSetting::MinPulses, Value::I64(self.min_pulses)),
            (
                AnalyserSetting::ClusterRatio,
                Value::F64(f64::from(self.guesser.cluster_ratio())),
            ),
        ])
    }
}

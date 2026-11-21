//! The channels of `ogn`: a plan of equally spaced channels around the tuned frequency, each a configuration of the chip
//! receiver of the crate, and the reader of the packet after a sync with its checks and repair.

use super::proto::{self, Repair, BITS, PACKET};
use crate::chips::{ChipChannel, ChipConfig, ChipStats, ChipView, Detector, Outcome, Reader};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::field;
use sdrkit_core::{
    ConfigError, Record, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, TagKind,
    Timed,
};

/// The chips of the sync word and the packet.
const AIR_CHIPS: f64 = 64.0 + 416.0;
/// The room that a channel needs at each side of its centre, Hz.
pub const CHANNEL_ROOM_HZ: f64 = 90e3;

/// The count of channels. The default is the plan of the test signal.
pub const CHANNELS: Setting<field::I64> = Setting::new("channels", "Channels", "", 2)
    .range(1.0, 80.0, 1.0)
    .help(
        "The count of channels of the plan of your area. The tuned frequency is the middle of the plan. The default is \
         the plan of the test signal.",
    );
/// The distance between two channels. The default is the plan of the test signal.
pub const CHANNEL_STEP_HZ: Setting<field::Hz> =
    Setting::new("channel_step_hz", "Channel spacing", "Hz", 200e3)
        .range(100e3, 2e6, 1.0)
        .help("The distance between two channels. The default is the plan of the test signal.");

sdrkit_core::settings! {
    /// The settings of `ogn.channels`.
    pub enum OgnChannelsSetting {
        /// Channels.
        Channels = CHANNELS,
        /// Channel spacing.
        ChannelStepHz = CHANNEL_STEP_HZ,
    }
}

/// The signal of one channel: 100 000 chips a second, ±50 kHz, Manchester bits, the 32-bit sync word.
pub fn chip_config() -> ChipConfig {
    ChipConfig {
        chip_rate: 100e3,
        samples_per_chip: 4,
        deviation_hz: 50e3,
        pass_hz: 85e3,
        stop_hz: 140e3,
        detector: Detector::Discriminator,
        manchester: true,
        sync: proto::SYNC,
        sync_bits: 32,
        skip_samples: 200,
        ring_chips: 1024,
    }
}

/// The frequencies of a plan of `n` channels `step_hz` apart whose middle is `dial_hz`.
pub fn plan_frequencies(dial_hz: f64, n: usize, step_hz: f64) -> Vec<f64> {
    (0..n)
        .map(|i| dial_hz + (i as f64 - (n as f64 - 1.0) / 2.0) * step_hz)
        .collect()
}

/// True if a channel `offset_hz` from the centre of a capture at `rate` is inside the capture.
pub fn fits(offset_hz: f64, rate: f64) -> bool {
    offset_hz.abs() + CHANNEL_ROOM_HZ < 0.49 * rate
}

/// One packet with no failed check and a good header parity.
#[derive(Clone, Debug, PartialEq)]
pub struct OgnPacket {
    /// The 26 bytes, after the repair.
    pub bytes: [u8; PACKET],
    /// The index of the channel in the plan.
    pub channel: u16,
    /// The frequency of the channel, Hz.
    pub freq_hz: f64,
    /// The level of the burst over the noise floor, dB.
    pub level_db: f64,
    /// The power of the burst in the channel, dBFS.
    pub power_dbfs: f64,
    /// The bits that the repair changed.
    pub repaired: u32,
}

/// The state of one channel at a report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChannelState {
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The channel is inside the capture and received.
    pub on: bool,
    /// The counters.
    pub stats: ChipStats,
    /// The noise floor, dBFS for each sample.
    pub noise_dbfs: Option<f64>,
}

/// What the channels send to the tracker (graph item `ogn.item`).
#[derive(Clone, Debug, PartialEq)]
pub enum OgnItem {
    /// A packet. The time of the item is the end of the packet.
    Packet(OgnPacket),
    /// The state of every channel of the plan, at each report and when the plan changes.
    Channels(Vec<ChannelState>),
}

impl Item for OgnItem {
    const KIND: &'static str = "ogn.item";
}

/// Reads the 208 bits after a sync: the checks, the repair from the soft bits, the header parity.
pub struct OgnReader {
    channel: u16,
    freq_hz: f64,
    packets: Vec<(f64, OgnPacket)>,
    repair: Repair,
}

impl OgnReader {
    /// The reader of channel `channel` of the plan.
    pub fn new(channel: u16, freq_hz: f64) -> Self {
        OgnReader {
            channel,
            freq_hz,
            packets: Vec::new(),
            repair: Repair::default(),
        }
    }
}

impl Reader for OgnReader {
    fn first_need(&self) -> u64 {
        2 * BITS as u64
    }

    fn read(&mut self, view: &ChipView<'_>) -> Outcome {
        let mut soft = [0.0f32; BITS];
        let mut p = [0u8; PACKET];
        for (j, s) in soft.iter_mut().enumerate() {
            let (bit, margin) = view.bit(j, 0.0);
            *s = if bit { margin } else { -margin };
            p[j / 8] |= u8::from(bit) << (7 - j % 8);
        }
        let mut repaired = 0;
        if proto::failed_checks(&p) != 0 {
            let Some((fixed, changed)) = self.repair.run(&soft) else {
                return Outcome::Bad;
            };
            p = fixed;
            repaired = changed;
        }
        if proto::parse(&p).is_none() {
            return Outcome::Bad;
        }
        let end_s = view.sync_time_s() + AIR_CHIPS / chip_config().chip_rate;
        self.packets.push((
            end_s,
            OgnPacket {
                bytes: p,
                channel: self.channel,
                freq_hz: self.freq_hz,
                level_db: view.level_db(-63, 2 * BITS as i64),
                power_dbfs: view.power_dbfs(-63, 2 * BITS as i64),
                repaired,
            },
        ));
        Outcome::Good
    }
}

/// The channels component (`ogn.channels`).
pub struct OgnChannels {
    channels: Vec<ChipChannel<OgnReader>>,
    count: usize,
    step_hz: f64,
    rate: f64,
    centre_hz: f64,
    tuned_hz: f64,
    changed: bool,
}

impl Default for OgnChannels {
    fn default() -> Self {
        OgnChannels {
            channels: Vec::new(),
            count: CHANNELS.default as usize,
            step_hz: CHANNEL_STEP_HZ.default,
            rate: 0.0,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            changed: false,
        }
    }
}

impl OgnChannels {
    /// Makes the channels of the plan for the input. The middle of the plan is the tuned frequency, or the centre of the
    /// capture when that is not known. The counters of a plan with the same count of channels stay.
    fn start(&mut self) -> Result<(), ConfigError> {
        if self.rate <= 0.0 {
            return Ok(());
        }
        let dial = if self.tuned_hz > 0.0 {
            self.tuned_hz
        } else {
            self.centre_hz
        };
        let centre = if self.centre_hz > 0.0 {
            self.centre_hz
        } else {
            dial
        };
        let freqs = plan_frequencies(dial, self.count, self.step_hz);
        if self.channels.len() != freqs.len() {
            self.channels = freqs
                .iter()
                .enumerate()
                .map(|(i, &f)| ChipChannel::new(chip_config(), OgnReader::new(i as u16, f)))
                .collect();
        }
        for (ch, f) in self.channels.iter_mut().zip(freqs) {
            ch.reader.freq_hz = f;
            if fits(f - centre, self.rate) {
                ch.configure(self.rate, f - centre)?;
            } else {
                ch.stop();
            }
        }
        self.changed = true;
        Ok(())
    }

    fn states(&self) -> Vec<ChannelState> {
        self.channels
            .iter()
            .map(|ch| ChannelState {
                freq_hz: ch.reader.freq_hz,
                on: ch.running(),
                stats: ch.stats(),
                noise_dbfs: ch.noise_dbfs(),
            })
            .collect()
    }
}

impl Component for OgnChannels {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ogn.channels",
            vec![Port::iq("in")],
            vec![Port::typed::<OgnItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(OgnChannelsSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        self.start()?;
        Ok(vec![Format::new(Kind::typed::<OgnItem>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let retuned = io.input(0).tags().iter().rev().find_map(|t| match t.kind {
            TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } => Some((centre_hz, tuned_hz)),
            _ => None,
        });
        if let Some((c, t)) = retuned {
            self.centre_hz = c;
            self.tuned_hz = t;
            // A plan that the filters refuse stays as it was.
            let _ = self.start();
        }
        let x = io.iq(0);
        for ch in &mut self.channels {
            ch.feed(x);
        }
        let tick = io.tick();
        let states = (tick || std::mem::take(&mut self.changed)).then(|| self.states());
        let o = io.items_out::<OgnItem>(0);
        for ch in &mut self.channels {
            for (t, p) in ch.reader.packets.drain(..) {
                o.push(Timed {
                    time: t,
                    value: OgnItem::Packet(p),
                });
            }
        }
        if let Some(s) = states {
            o.push(Timed {
                time: 0.0,
                value: OgnItem::Channels(s),
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match OgnChannelsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OgnChannelsSetting::Channels => {
                self.count = usize::try_from(CHANNELS.read(id, v)?).unwrap_or(1).max(1);
            }
            OgnChannelsSetting::ChannelStepHz => self.step_hz = CHANNEL_STEP_HZ.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (OgnChannelsSetting::Channels, Value::I64(self.count as i64)),
            (OgnChannelsSetting::ChannelStepHz, Value::Hz(self.step_hz)),
        ])
    }

    fn commands(&self) -> &Commands {
        OgnChannelsCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match OgnChannelsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            OgnChannelsCommand::Clear => {
                for ch in &mut self.channels {
                    ch.reset_stats();
                }
            }
        }
        Ok(None)
    }
}

sdrkit_core::commands! {
    /// The commands of `ogn.channels`.
    pub enum OgnChannelsCommand {
        /// Sets the counters of the channels to zero.
        Clear = sdrkit_util_objects::commands::CLEAR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sync_word_is_its_chips() {
        let chips: Vec<u8> = (0..32)
            .rev()
            .flat_map(|k| {
                let bit = (proto::SYNC >> k & 1) as u8;
                [1 - bit, bit]
            })
            .collect();
        let bytes: Vec<u8> = chips
            .chunks(8)
            .map(|c| c.iter().fold(0, |v, b| v << 1 | b))
            .collect();
        assert_eq!(bytes, [0xAA, 0x66, 0x55, 0xA5, 0x96, 0x99, 0x96, 0x5A]);
    }

    #[test]
    fn the_default_plan_is_two_channels_beside_the_dial() {
        let f = plan_frequencies(868.3e6, CHANNELS.default as usize, CHANNEL_STEP_HZ.default);
        assert!((f[0] - 868.2e6).abs() < 1e-3 && (f[1] - 868.4e6).abs() < 1e-3);
        assert!(f.iter().all(|f| fits(f - 868.3e6, 1e6)));
        // A capture centred on 868.875 MHz at 2 Msps holds the lower channel.
        assert!(fits(868.2e6 - 868.875e6, 2e6));
    }
}

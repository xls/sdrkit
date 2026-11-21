//! One FLARM channel: the chip receiver of the crate at 800 ksps (8 samples per chip) with the FM discriminator and the
//! Manchester sync word, and the reader of the 26-byte frame with its CRC.

use super::proto::{crc_ok, CHIP_RATE, DEVIATION, FRAME, SYNC};
use crate::chips::{ChipChannel, ChipConfig, ChipView, Detector, Outcome, Reader};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};

/// The rate the chips are read at.
pub const CHAN_RATE: f64 = 800e3;
const SPC: usize = 8;
const DATA_CHIPS: usize = FRAME * 16;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 85e3;

/// The signal of a FLARM channel: Manchester chips at 100 kchip/s, ±50 kHz, the 32-bit sync word.
pub fn chip_config() -> ChipConfig {
    ChipConfig {
        chip_rate: CHIP_RATE,
        samples_per_chip: SPC,
        deviation_hz: DEVIATION,
        pass_hz: PASS_HZ,
        stop_hz: 140e3,
        detector: Detector::Discriminator,
        manchester: true,
        sync: SYNC,
        sync_bits: 32,
        skip_samples: 64 * SPC as u64,
        ring_chips: 1024,
    }
}

sdrkit_core::choice_enum! {
    /// The two FLARM channels around 868.3 MHz.
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    pub enum Channel {
        /// 868.2 MHz.
        #[default]
        Low = ("0", "868.2 MHz"),
        /// 868.4 MHz.
        High = ("1", "868.4 MHz"),
    }
}

impl Channel {
    /// Channel 0 is [`Channel::Low`], any other number [`Channel::High`].
    pub fn of(n: u8) -> Channel {
        if n == 0 {
            Channel::Low
        } else {
            Channel::High
        }
    }
    /// 0 for [`Channel::Low`], 1 for [`Channel::High`].
    pub fn slot(self) -> usize {
        usize::from(self.index())
    }
    /// The offset from 868.3 MHz, Hz.
    pub fn offset_hz(self) -> f64 {
        match self {
            Channel::Low => -100e3,
            Channel::High => 100e3,
        }
    }
    /// The radio frequency, Hz.
    pub fn frequency_hz(self) -> f64 {
        868.3e6 + self.offset_hz()
    }
}

/// A frame whose CRC passed.
#[derive(Clone, Debug, PartialEq)]
pub struct FlarmFrame {
    /// The 26 bytes (payload and CRC).
    pub bytes: [u8; FRAME],
    /// The channel.
    pub channel: Channel,
    /// Level over the channel's noise floor, dB.
    pub level_db: f64,
    /// Bit errors in the sync.
    pub sync_errors: u32,
}

/// What a channel reports to the tracker.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelItem {
    /// A frame with a good CRC.
    Frame(FlarmFrame),
    /// The channel's counters, at each report.
    Stats(ChannelStats),
}

impl Item for ChannelItem {
    const KIND: &'static str = "flarm.frame";
}

/// The counters of a channel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelStats {
    /// The channel.
    pub channel: Channel,
    /// Syncs taken.
    pub syncs: u64,
    /// Good frames.
    pub good: u64,
    /// CRC failures after a sync with at most one error.
    pub bad: u64,
    /// The noise floor, dBFS per sample (None until measured).
    pub noise_dbfs: Option<f64>,
    /// Signal time of the last block with energy (seconds since configure).
    pub energy_at: Option<f64>,
}

/// Reads the 26 bytes after a sync and keeps the frames whose CRC is right.
#[derive(Clone, Debug)]
pub struct FlarmReader {
    channel: Channel,
    frames: Vec<(f64, FlarmFrame)>,
}

impl Reader for FlarmReader {
    fn first_need(&self) -> u64 {
        DATA_CHIPS as u64
    }

    fn read(&mut self, view: &ChipView<'_>) -> Outcome {
        let mut bytes = [0u8; FRAME];
        for (j, b) in bytes.iter_mut().enumerate() {
            *b = view.byte(j, 0.0);
        }
        if !crc_ok(&bytes) {
            // A sync that matched one way with all chips the other way.
            let inv = bytes.map(|b| !b);
            if !crc_ok(&inv) {
                return Outcome::Bad;
            }
            bytes = inv;
        }
        // The mean sample power over the sync and the data.
        let level_db = view.level_db(-63, DATA_CHIPS as i64);
        self.frames.push((
            view.sync_time_s(),
            FlarmFrame {
                bytes,
                channel: self.channel,
                level_db,
                sync_errors: view.sync_errors,
            },
        ));
        Outcome::Good
    }
}

/// The DSP of one channel.
#[derive(Clone, Debug)]
pub struct FlarmChannel {
    channel: Channel,
    chips: ChipChannel<FlarmReader>,
}

impl FlarmChannel {
    /// The DSP of `channel`.
    pub fn new(channel: Channel) -> Self {
        FlarmChannel {
            channel,
            chips: ChipChannel::new(
                chip_config(),
                FlarmReader {
                    channel,
                    frames: Vec::new(),
                },
            ),
        }
    }

    /// Starts again at `rate` with the channel pair `pair_offset` Hz from the centre of the input (filters, discriminator,
    /// floor and sync search restart; counters stay).
    ///
    /// # Errors
    ///
    /// The errors of the channeliser and of the filter design for a rate that they refuse.
    pub fn configure(
        &mut self,
        rate: f64,
        pair_offset: f64,
    ) -> Result<(), sdrkit_algo_dsp::DspError> {
        self.chips
            .configure(rate, self.channel.offset_hz() + pair_offset)
    }

    /// Stops the channel until the next configure.
    pub fn stop(&mut self) {
        self.chips.stop();
    }

    /// Counters so far.
    pub fn stats(&self) -> ChannelStats {
        let s = self.chips.stats();
        ChannelStats {
            channel: self.channel,
            syncs: s.syncs,
            good: s.good,
            bad: s.bad,
            noise_dbfs: self.chips.noise_dbfs(),
            energy_at: self.chips.energy_at(),
        }
    }

    /// Feeds input samples; good frames (with their times, seconds since configure) are appended.
    pub fn feed(&mut self, x: &[Complex32], frames: &mut Vec<(f64, FlarmFrame)>) {
        self.chips.feed(x);
        frames.append(&mut self.chips.reader.frames);
    }
}

/// The channel of a [`FlarmChannelComponent`].
pub const CHANNEL: Setting<field::Choice<Channel>> =
    Setting::choice("channel", "Channel", Channel::Low);

sdrkit_core::settings! {
    /// The settings of [`FlarmChannelComponent`].
    pub enum FlarmChannelSetting {
        /// The channel.
        Channel = CHANNEL,
    }
}

/// The channel component (`drones.flarm_channel`): IQ in, frames and counters out.
pub struct FlarmChannelComponent {
    chan: FlarmChannel,
    frames: Vec<(f64, FlarmFrame)>,
}

impl FlarmChannelComponent {
    /// The component of `channel`.
    pub fn new(channel: Channel) -> Self {
        FlarmChannelComponent {
            chan: FlarmChannel::new(channel),
            frames: Vec::new(),
        }
    }
}

impl Component for FlarmChannelComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.flarm_channel",
            vec![Port::iq("in")],
            vec![Port::typed::<ChannelItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FlarmChannelSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.chan.configure(f.rate, f.signal_offset_hz())?;
        Ok(vec![Format::new(Kind::typed::<ChannelItem>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.frames.clear();
        self.chan.feed(x, &mut self.frames);
        let tick = io.tick();
        let stats = tick.then(|| self.chan.stats());
        let o = io.items_out::<ChannelItem>(0);
        for (t, f) in self.frames.drain(..) {
            o.push(Timed {
                time: t,
                value: ChannelItem::Frame(f),
            });
        }
        if let Some(s) = stats {
            o.push(Timed {
                time: 0.0,
                value: ChannelItem::Stats(s),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FlarmChannelSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FlarmChannelSetting::Channel => {
                let c = CHANNEL.read(id, v)?;
                if c != self.chan.channel {
                    self.chan = FlarmChannel::new(c);
                }
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(
            FlarmChannelSetting::Channel,
            Value::Choice(self.chan.channel.index()),
        )])
    }
}

//! The stack contract: a [`StackDescriptor`] says what a stack needs and how to make its [`Receiver`] and [`Generator`].

use crate::bench::Benchmark;
use crate::event::Event;
use crate::facilities::Facilities;
use crate::media::FourCc;
use crate::media::{ChannelLayout, MediaFormat, Packet, PictureState};
use crate::owner::Owner;
use crate::request::HostRequest;
use crate::settings::{Schema, SettingError, SettingId, Values};
use crate::signal::SignalType;
use crate::state::{StateBlob, StateError};
use crate::station::Station;
use crate::storage::{StorageError, StorePath};
use crate::verify::VerifyHooks;
use crate::{Complex32, Hz, LockState};
use std::sync::Arc;

/// What a stack requires from the radio.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    pub min_hz: Hz,
    pub max_hz: Hz,
    pub default_hz: Hz,
    /// The rate the stack would like.
    pub rate_hz: Hz,
    /// Below this the receiver refuses to run.
    pub min_rate_hz: Hz,
    /// The width of the signal, for the box on the spectrum.
    pub bandwidth_hz: Hz,
    /// Tune the radio this far from the signal, so the radio's DC spike stays off it.
    pub tune_offset_hz: Hz,
    /// The signal has a carrier at its centre (AM): the front end must not remove DC there.
    pub carrier_at_centre: bool,
    /// The signal itself is not circular around the centre (analog TV carriers, pulses of one phase): the front end's blind IQ
    /// correction would take it for an imbalance, so it is skipped.
    pub not_circular: bool,
    /// The bandwidth of the radio's analogue (baseband) filter that the stack requires, in Hz; `None` leaves it to the radio
    /// (automatic).
    pub radio_filter_hz: Option<Hz>,
    /// The stack must not be offset-tuned: the radio is tuned onto the signal even when the user requested
    /// automatic offset tuning (a stack that watches several channels, or handles the DC spike itself).
    pub no_offset_tuning: bool,
    /// Where the stack listens: anywhere in its range, only on its listed frequencies, or on one frequency. `None` when
    /// the stack does not declare it.
    pub kind: Option<TuningKind>,
    /// The plain demodulation that the general stack takes to listen to the signal of the stack by ear. `None` for a
    /// signal that no plain demodulation suits (OFDM, cells, most digital modes).
    pub demod: Option<PlainDemod>,
    /// The most channels of host audio that the graph of the stack takes (a `host.in` component), over its submodes: 1
    /// for mono, 2 for stereo, 0 for none. The stacks table reads it without making the stack.
    pub host_audio_channels: u16,
}

crate::choice_enum! {
    /// Where a stack listens. A host keeps the present frequency on a change to a stack that listens anywhere, and asks
    /// before it moves to a listed frequency of another stack.
    pub enum TuningKind {
        /// Anywhere in its range: the user tunes freely (CW, SSTV, DMR, the general stack).
        Free = ("free", "Anywhere"),
        /// Only on the frequencies that the data lists for it: a plan, channels or dials (FM, FT8, TV).
        Channels = ("channels", "Listed frequencies"),
        /// On the one frequency of its system, its default frequency (ADS-B on 1090 MHz, GNSS L1).
        Fixed = ("fixed", "One frequency"),
    }
}

crate::choice_enum! {
    /// A plain demodulation, as the demodulators of the general stack name them.
    pub enum DemodMode {
        /// Amplitude modulation.
        Am = ("am", "AM"),
        /// Narrow FM.
        Nfm = ("nfm", "NFM"),
        /// Wide FM (broadcast).
        Wfm = ("wfm", "WFM"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
        /// Morse code: a beat tone at a pitch.
        Cw = ("cw", "CW"),
    }
}

/// How the general stack is set to listen to the signal of a stack by ear: the demodulator, its filter width and, for
/// CW, the pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlainDemod {
    /// The demodulator.
    pub mode: DemodMode,
    /// The usual width of the channel filter, Hz.
    pub width_hz: Hz,
    /// The pitch of the beat tone of CW, Hz. 0 for another demodulator.
    pub cw_pitch_hz: Hz,
}

impl PlainDemod {
    /// `mode` with the filter width `width_hz`, for a demodulator other than CW.
    pub const fn new(mode: DemodMode, width_hz: Hz) -> Self {
        PlainDemod {
            mode,
            width_hz,
            cw_pitch_hz: 0.0,
        }
    }

    /// CW with the filter width `width_hz` and the pitch `pitch_hz`.
    pub const fn cw(width_hz: Hz, pitch_hz: Hz) -> Self {
        PlainDemod {
            mode: DemodMode::Cw,
            width_hz,
            cw_pitch_hz: pitch_hz,
        }
    }
}

impl Tuning {
    /// The value of every field that a descriptor does not state: the whole range, no rates, no width, every flag off.
    /// A descriptor ends its literal with `..Tuning::DEFAULT`, so that a field added later needs no edit of every stack.
    pub const DEFAULT: Tuning = Tuning {
        min_hz: 0.0,
        max_hz: 1e12,
        default_hz: 0.0,
        rate_hz: 0.0,
        min_rate_hz: 0.0,
        bandwidth_hz: 0.0,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        kind: None,
        demod: None,
        host_audio_channels: 0,
    };

    /// A stack that can be tuned anywhere, with every flag off. A `const fn`, so a descriptor can start from it:
    /// `Tuning { carrier_at_centre: true, ..Tuning::anywhere(100e6, 2e6, 1e6, 200e3) }`.
    pub const fn anywhere(default_hz: Hz, rate_hz: Hz, min_rate_hz: Hz, bandwidth_hz: Hz) -> Self {
        Tuning {
            default_hz,
            rate_hz,
            min_rate_hz,
            bandwidth_hz,
            ..Tuning::DEFAULT
        }
    }
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning::DEFAULT
    }
}

/// A variant of a stack that shares its receiver (FT8 and WSPR under HF digital).
#[derive(Clone, Debug, PartialEq)]
pub struct SubMode {
    pub id: &'static str,
    pub name: &'static str,
    pub default_hz: Hz,
    /// The plain demodulation of the submode, where it differs from that of the stack ([`Tuning::demod`]).
    pub demod: Option<PlainDemod>,
}

impl SubMode {
    /// The value of every field that a submode does not state. A submode literal ends with `..SubMode::DEFAULT`, so that
    /// a field added later needs no edit of every stack.
    pub const DEFAULT: SubMode = SubMode {
        id: "",
        name: "",
        default_hz: 0.0,
        demod: None,
    };
}

/// Everything a receiver is made with.
///
/// Build it with [`StackConfig::new`] and the `with_*` methods.
///
/// ```
/// use sdrkit_core::{Schema, StackConfig, Values};
/// let cfg = StackConfig::new(Values::defaults(&Default::default())).with_submode("ft8");
/// assert_eq!(cfg.submode.as_deref(), Some("ft8"));
/// ```
#[derive(Clone, Debug)]
pub struct StackConfig {
    pub submode: Option<String>,
    pub settings: Values,
    /// The host's conversions between stream formats. `None` uses the standard set of sdrkit-graph.
    pub adapters: Option<Arc<dyn AdapterCatalog>>,
}

/// The host's conversions between stream formats, as a stack's build function receives them in [`StackConfig::adapters`].
/// The adapter catalog of sdrkit-graph is the one implementation. A stack passes it to its graph builder.
pub trait AdapterCatalog: std::any::Any + Send + Sync + std::fmt::Debug {
    /// The catalog as a shared [`Any`](std::any::Any), so the graph crate can take back its own type.
    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync>;
}

impl StackConfig {
    /// A configuration with these settings, no submode and the standard adapters. Build configurations with it, so fields
    /// can be added without changing every caller.
    pub fn new(settings: Values) -> Self {
        StackConfig {
            submode: None,
            settings,
            adapters: None,
        }
    }
    /// The same configuration with the host's adapter catalog.
    pub fn with_adapters(mut self, adapters: Arc<dyn AdapterCatalog>) -> Self {
        self.adapters = Some(adapters);
        self
    }
    /// The same configuration for `submode`.
    pub fn with_submode(mut self, submode: &str) -> Self {
        self.submode = Some(submode.into());
        self
    }
    /// The same configuration for `submode`, or for the stack's default when `None`.
    pub fn with_submode_opt(mut self, submode: Option<String>) -> Self {
        self.submode = submode;
        self
    }
}

/// Everything a generator is made with: the test signal's own options use the stack's settings schema.
#[derive(Clone, Debug)]
pub struct GenConfig {
    pub rate_hz: Hz,
    pub submode: Option<String>,
    pub settings: Values,
    /// Makes the signal repeatable.
    pub seed: u64,
    /// The station of the host: position, callsign and time.
    pub station: crate::Station,
    /// The frequency where a session of the stack listens, Hz ([`GenConfig::tuned_hz`]).
    tuned_hz: Option<Hz>,
    /// Set when the generator reads [`GenConfig::tuned_hz`].
    tuned_read: Arc<std::sync::atomic::AtomicBool>,
}

impl GenConfig {
    /// A generator at `rate_hz` with these settings, no submode and seed 0. Build configurations with it, so fields can be added
    /// without changing every caller.
    ///
    /// ```
    /// use sdrkit_core::{GenConfig, Values};
    /// let cfg = GenConfig::new(48e3, Values::defaults(&Default::default())).with_submode("ft8").with_seed(7);
    /// assert_eq!((cfg.rate_hz, cfg.submode.as_deref(), cfg.seed), (48e3, Some("ft8"), 7));
    /// ```
    pub fn new(rate_hz: Hz, settings: Values) -> Self {
        GenConfig {
            rate_hz,
            submode: None,
            settings,
            seed: 0,
            station: crate::Station::default(),
            tuned_hz: None,
            tuned_read: Arc::default(),
        }
    }
    /// The same configuration for `submode`.
    pub fn with_submode(mut self, submode: &str) -> Self {
        self.submode = Some(submode.into());
        self
    }
    /// The same configuration for `submode`, or for the stack's default when `None`.
    pub fn with_submode_opt(mut self, submode: Option<String>) -> Self {
        self.submode = submode;
        self
    }
    /// The same configuration with the seed `seed`.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// The same configuration for the station `station`.
    #[must_use]
    pub fn with_station(mut self, station: crate::Station) -> Self {
        self.station = station;
        self
    }

    /// The same configuration for a signal that a session tuned to `tuned_hz` reads, or for no session when `None`.
    #[must_use]
    pub fn with_tuned_hz(mut self, tuned_hz: Option<Hz>) -> Self {
        self.tuned_hz = tuned_hz;
        self.tuned_read = Arc::default();
        self
    }

    /// The frequency where a session of the stack listens: the radio centre less the tune offset of the stack. `None`
    /// when the signal is not made for a radio. A generator that places its channels against the radio centre reads it
    /// when its own centre setting is 0. A signal that has read it is made again when it changes.
    pub fn tuned_hz(&self) -> Option<Hz> {
        self.tuned_read
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.tuned_hz
    }

    /// True once a generator made with this configuration has read [`GenConfig::tuned_hz`].
    pub fn reads_tuned(&self) -> bool {
        self.tuned_read.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// The samples a receiver gets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputInfo {
    pub rate_hz: Hz,
    /// The frequency at the centre of the samples (0 Hz in them).
    pub centre_hz: Hz,
    /// Where the user tuned: the signal is at `tuned_hz - centre_hz` in the samples.
    pub tuned_hz: Hz,
    /// The samples come from a radio as they are received (`true`), or from a recording or a generator (`false`), which replays at
    /// its own pace and may be off its stated centre.
    pub live: bool,
}

impl InputInfo {
    /// Live samples at `rate_hz`, centred on `centre_hz`, tuned to `tuned_hz`.
    pub fn new(rate_hz: Hz, centre_hz: Hz, tuned_hz: Hz) -> Self {
        InputInfo {
            rate_hz,
            centre_hz,
            tuned_hz,
            live: true,
        }
    }
    /// The same samples, from a recording or a generator.
    pub fn recording(mut self) -> Self {
        self.live = false;
        self
    }
    pub fn signal_offset_hz(&self) -> Hz {
        self.tuned_hz - self.centre_hz
    }
}

/// The kind of compressed media a receiver passes to the host's decoders (see
/// [`Codec::stream_kind`](crate::media::Codec::stream_kind)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    H264,
    Hevc,
    Mpeg2Video,
    Aac,
    /// HE-AAC, v1 or v2.
    HeAac,
    /// xHE-AAC (USAC).
    XheAac,
    Ac3,
    EAc3,
    /// MPEG-1 Layer I.
    Mp1,
    /// MPEG-1 Layer II.
    Mp2,
    /// MPEG-1 Layer III.
    Mp3,
    /// A still picture: compressed (JPEG, PNG, JPEG 2000, HEIC, GIF; the format's code says which) or raw pixels (a
    /// waterfall in grey; the format's `pixels` returns the layout).
    Picture,
    /// An MPEG transport stream. One [`Elementary`] packet holds whole 188-byte transport packets: its length is a multiple
    /// of 188.
    MpegTs,
    /// Decoded video: one [`Elementary`] packet holds one frame of raw pixels in display order. The format's `pixels`
    /// returns the layout, and its `video` the aspect ratio, the frame rate and the field order.
    RawVideo,
}

/// One packet of compressed media for the host's decoders, as a receiver passes it in [`Output::media`]. The bytes are shared
/// with the receiver (reference-counted), never copied, and the stream's format (codec, packaging, configuration, timebase,
/// properties) comes with every packet, also shared.
#[derive(Clone, Debug, PartialEq)]
pub struct Elementary {
    /// What the host decodes it with.
    pub kind: StreamKind,
    /// The bytes, with pts, dts (in `format.timebase`) and the keyframe flag.
    pub packet: Packet,
    /// The stream's format.
    pub format: Arc<MediaFormat>,
    /// For a picture: which picture it is and whether more of it can come. `None` for media, and for a picture of a
    /// receiver that states nothing: the host then takes each packet as a whole picture of its own.
    pub picture: Option<PictureTag>,
}

/// Which picture a picture packet holds, for the host.
#[derive(Clone, Debug, PartialEq)]
pub struct PictureTag {
    /// The number of the picture in its receiver. Packets with the same number are states of one picture. A number
    /// whose low 32 bits are [`Picture::WHOLE`](crate::Picture::WHOLE) is a whole picture of its own.
    pub id: u64,
    /// Whether more of the picture can come.
    pub state: PictureState,
    /// The name of the picture, if it has one.
    pub name: Option<Arc<str>>,
}

/// What a receiver can produce from one call of [`Receiver::feed`]. The engine drains it after the call. Nothing here blocks.
#[derive(Debug, Default)]
pub struct Output {
    audio: Vec<f32>,
    runs: Vec<AudioRun>,
    pub events: Vec<Event>,
    /// Compressed media for the host's decoders, in the order produced.
    pub media: Vec<Elementary>,
    /// The receiver's requests to the host (retune, hop, follow a channel), in the order they were made.
    pub requests: Vec<HostRequest>,
}

/// Where a run of one audio format starts in [`Output::audio`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct AudioRun {
    start: usize,
    rate_hz: Hz,
    channels: u16,
    layout: ChannelLayout,
}

/// A part of [`Output::audio`] with one rate and one channel count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioPart<'a> {
    /// The frames per second.
    pub rate_hz: Hz,
    /// The samples per frame: 1 for mono, 2 for stereo (left then right).
    pub channels: u16,
    /// The order of the channels.
    pub layout: ChannelLayout,
    /// The interleaved samples.
    pub samples: &'a [f32],
}

impl Output {
    /// Empties it and keeps the buffers.
    pub fn clear(&mut self) {
        self.audio.clear();
        self.runs.clear();
        self.events.clear();
        self.media.clear();
        self.requests.clear();
    }
    /// Moves everything of `other` to the end of this output, in order. `other` is left empty and keeps its buffers.
    pub fn append(&mut self, other: &mut Output) {
        let mut start = 0;
        for (k, r) in other.runs.iter().enumerate() {
            let end = other.runs.get(k + 1).map_or(other.audio.len(), |n| n.start);
            self.push_audio_in(
                &other.audio[start.max(r.start)..end],
                r.rate_hz,
                r.channels,
                r.layout,
            );
            start = end;
        }
        self.events.append(&mut other.events);
        self.media.append(&mut other.media);
        self.requests.append(&mut other.requests);
        other.clear();
    }
    /// Asks the host for something (see [`HostRequest`]).
    pub fn request(&mut self, r: HostRequest) {
        self.requests.push(r);
    }
    /// Appends interleaved audio `samples` of `channels` channels at `rate_hz`: mono or stereo, or channels in an order
    /// that the stack does not know. If the rate or the channel count differs from the audio before it, a new run starts
    /// at the first of these samples.
    pub fn push_audio(&mut self, samples: &[f32], rate_hz: Hz, channels: u16) {
        self.push_audio_in(
            samples,
            rate_hz,
            channels,
            ChannelLayout::of_count(channels),
        );
    }
    /// [`Output::push_audio`] for channels in the order `layout`.
    pub fn push_audio_in(
        &mut self,
        samples: &[f32],
        rate_hz: Hz,
        channels: u16,
        layout: ChannelLayout,
    ) {
        if samples.is_empty() {
            return;
        }
        let same = self
            .runs
            .last()
            .is_some_and(|r| r.rate_hz == rate_hz && r.channels == channels && r.layout == layout);
        if !same {
            self.runs.push(AudioRun {
                start: self.audio.len(),
                rate_hz,
                channels,
                layout,
            });
        }
        self.audio.extend_from_slice(samples);
    }
    /// Every audio sample, interleaved, of every run ([`Output::audio_runs`]).
    pub fn audio(&self) -> &[f32] {
        &self.audio
    }
    /// Every audio sample, to change in place (a level, a filter). The runs stay as they are.
    pub fn audio_mut(&mut self) -> &mut [f32] {
        &mut self.audio
    }
    /// The audio in runs of one rate and one channel count, in order.
    pub fn audio_runs(&self) -> impl Iterator<Item = AudioPart<'_>> + '_ {
        self.runs.iter().enumerate().map(|(i, r)| {
            let end = self.runs.get(i + 1).map_or(self.audio.len(), |n| n.start);
            AudioPart {
                rate_hz: r.rate_hz,
                channels: r.channels,
                layout: r.layout,
                samples: &self.audio[r.start..end],
            }
        })
    }
    /// The rate and the channel count of the last audio run. `None` when there is no audio.
    pub fn audio_format(&self) -> Option<(Hz, u16)> {
        self.runs.last().map(|r| (r.rate_hz, r.channels))
    }
    /// Hands a packet to the host's decoders (shares its bytes and the format; nothing is copied).
    pub fn push_media(&mut self, kind: StreamKind, packet: Packet, format: &Arc<MediaFormat>) {
        self.media.push(Elementary {
            kind,
            packet,
            format: format.clone(),
            picture: None,
        });
    }
    /// Hands a picture to the host with what `tag` states about it (shares its bytes and the format).
    pub fn push_picture(&mut self, packet: Packet, format: &Arc<MediaFormat>, tag: PictureTag) {
        self.media.push(Elementary {
            kind: StreamKind::Picture,
            packet,
            format: format.clone(),
            picture: Some(tag),
        });
    }
}

/// A rate as a refusal names what to ask for: `4 Msps`, `2.048 Msps`, `250 ksps`.
fn msps(hz: Hz) -> String {
    let (v, unit) = if hz >= 1e6 { (hz / 1e6, "Msps") } else { (hz / 1e3, "ksps") };
    let t = format!("{v:.3}");
    format!("{} {unit}", t.trim_end_matches('0').trim_end_matches('.'))
}

/// Why a receiver or a component cannot work with its input and settings.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ConfigError {
    /// The sample rate is below what the stack needs.
    #[error("the stack needs at least {need} samples a second, the input gives {got}: ask for {} or more", msps(*need))]
    RateTooLow {
        /// The lowest rate, Hz.
        need: Hz,
        /// The rate given, Hz.
        got: Hz,
    },
    /// The sample rate is above what the stack can take.
    #[error("the stack takes at most {max} samples a second, the input gives {got}: ask for {} or less", msps(*max))]
    RateTooHigh {
        /// The highest rate, Hz.
        max: Hz,
        /// The rate given, Hz.
        got: Hz,
    },
    /// The stage works at one sample rate only.
    #[error("the stage works at {need} samples a second, not {got}")]
    WrongRate {
        /// The rate needed, Hz.
        need: Hz,
        /// The rate given, Hz.
        got: Hz,
    },
    /// The sample rate must be a whole multiple of another rate.
    #[error("{got} samples a second is not a whole multiple of {of} Hz")]
    RateNotMultiple {
        /// The rate given, Hz.
        got: Hz,
        /// The rate it must be a multiple of, Hz.
        of: Hz,
    },
    /// The input has no sample rate yet.
    #[error("the input has no sample rate")]
    NoRate,
    /// The stage has no channel of this bandwidth.
    #[error("there is no channel {got} Hz wide")]
    BandwidthNotSupported {
        /// The requested bandwidth, Hz.
        got: Hz,
    },
    /// A setting refuses the configuration.
    #[error(transparent)]
    Setting(#[from] SettingError),
    /// The value of a setting cannot work at this sample rate.
    #[error("setting number {} cannot work at {rate_hz} samples a second", .id.0)]
    SettingAtRate {
        /// The setting.
        id: SettingId,
        /// The sample rate, Hz.
        rate_hz: Hz,
    },
    /// No codec is registered for this code.
    #[error("no codec {codec:?} is registered")]
    MissingCodec {
        /// The codec.
        codec: FourCc,
    },
    /// A data file cannot be read.
    #[error("data file {path}: {source}")]
    Data {
        /// The file.
        path: StorePath,
        /// What went wrong.
        source: StorageError,
    },
    /// A data file does not have the size, the checksum or the layout that is needed.
    #[error("data file {path} is damaged")]
    BadData {
        /// The file.
        path: StorePath,
    },
    /// A signal processing stage refused its parameters.
    #[error("{0}")]
    Stage(StageError),
}

/// A stack's decoder. It runs on the engine's decoding thread only. Its settings, results, events and commands are those of
/// [`Owner`].
pub trait Receiver: Owner {
    /// Takes the rate and centre (at start, and after a change of rate).
    ///
    /// # Errors
    ///
    /// [`ConfigError`] if the receiver cannot work with this input.
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError>;
    /// Decodes a block of any length. Never blocks.
    fn feed(&mut self, iq: &[Complex32], out: &mut Output);
    /// The input ended. The receiver decodes what it still holds, appends the outputs to `out` and brings its results up to
    /// date. The host calls it once, after the last [`Receiver::feed`]. A later `feed` or `configure` starts a new stream. The
    /// default does nothing.
    fn finish(&mut self, out: &mut Output) {
        let _ = out;
    }
    /// A new centre or tuned frequency at the same rate: drop sync. A receiver whose results belong to one frequency clears
    /// them.
    fn retuned(&mut self, input: &InputInfo) {
        let _ = input;
    }
    /// The frequency changed for another reason than the receiver's own order: forget every result of the frequency
    /// before, except what the description of the stack keeps ([`StackShape::kept`](crate::interface::StackShape::kept)).
    /// The host calls it after [`Receiver::retuned`] or [`Receiver::configure`] for a change that the host made, for a
    /// tune of a scan service (also to the frequency that the session has), and for a new centre of the input of a
    /// session without a frequency of its own. It does not call it for a change that the receiver ordered or for another
    /// retune of a service. The default does nothing.
    fn forget(&mut self) {}
    /// The station context (position, callsign, locator, time zone), given right after the receiver is made and again
    /// whenever the user changes it, between two blocks. The default ignores it.
    fn set_station(&mut self, station: &Station) {
        let _ = station;
    }
    /// True if the receiver makes transmissions (a station of a network, a beacon). The host connects a transmitter
    /// only to such a receiver. The default returns false.
    fn transmits(&self) -> bool {
        false
    }
    /// Turns the transmitter of the receiver on or off. The host calls it with `on` true while its one transmit switch
    /// is on and the radio of the receiver can transmit, and with false otherwise. While it is off the receiver makes
    /// no transmission and each own message fails. A receiver starts with it off. The default ignores the call.
    fn set_transmit(&mut self, on: bool) {
        let _ = on;
    }
    /// The host's facilities (files, clock, random source, codec plug-ins), given right after the receiver is made and
    /// before the first [`Receiver::configure`]. The default ignores them.
    fn set_facilities(&mut self, facilities: &Facilities) {
        let _ = facilities;
    }
    /// What the receiver has gathered (lists of things heard, logs), for the host to keep. `None` (the default) when it keeps
    /// nothing.
    fn save_state(&self) -> Option<StateBlob> {
        None
    }
    /// Restores what [`Receiver::save_state`] returned.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the blob is of another owner or version, or damaged. Nothing changes. The default returns
    /// [`StateError::NoState`].
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let _ = state;
        Err(StateError::NoState)
    }
    /// Tells the receive chain that `lost` samples are missing before the next sample that it is fed. The default does
    /// nothing.
    fn discontinuity(&mut self, lost: u64) {
        let _ = lost;
    }

    /// The stages of the receive chain, for the engine to place on threads. `None` for a receive chain that runs on one
    /// thread only. A stack may return the stages of another receive chain from one block to the next, for example at a
    /// hand-over between two standards. It then ends the placement of the chain it leaves inside that block: it moves
    /// the finished output of that chain into the output of the block ([`Staged::drain`](crate::Staged::drain)) and
    /// calls [`Staged::cut`](crate::Staged::cut) with mask 0, so no unit is lost or doubled and every stage thread of
    /// that chain returns. The engine sees the other [`Staged::stage_set`](crate::Staged::stage_set) at its next look
    /// and places the new stages.
    fn staged(&mut self) -> Option<&mut dyn crate::Staged> {
        None
    }
    /// The receiver as `Any`, so a host that knows the concrete type can reach it (the engine finds a stack's graph this way, to tap
    /// a port inside it: `rx.as_any_mut()?.downcast_mut::<sdrkit_graph::Graph>()`). `None` (the default) for a receiver that offers
    /// nothing more.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

/// The error of a lower crate (a filter, a modem, a code), kept with its type.
#[derive(Clone, Debug)]
pub struct StageError(std::sync::Arc<dyn std::error::Error + Send + Sync>);

impl StageError {
    /// Wraps `error`.
    pub fn new(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        StageError(std::sync::Arc::new(error))
    }
    /// The wrapped error, if it is a `T`.
    pub fn downcast_ref<T: std::error::Error + 'static>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl PartialEq for StageError {
    /// Two values are equal if they share one wrapped error.
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The source is the source of the wrapped error.
impl std::error::Error for StageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// A stack's transmitter: makes its signal at baseband. Used for tests, for a demo with no radio, and for transmit.
///
/// A generator makes the clean signal only, at unit mean power while it transmits. Noise, offset, drift, clock error, fading and
/// level are added by the framework from the standard `gen_*` settings (`sdrkit_components::sim`, through `sdrkit-algo-sim`), using
/// what [`Generator::signal_info`] declares.
pub trait Generator: Send {
    fn rate_hz(&self) -> Hz;
    /// Fills `out`. Returns how many samples were written: fewer than `out.len()` once a finite signal has ended.
    fn generate(&mut self, out: &mut [Complex32]) -> usize;
    /// What the signal is: its bandwidth, symbol rate, bit rate and power, so noise can be set as SNR, Es/N0 or Eb/N0. `None` (the
    /// default) for a generator that has not declared it; the framework then accepts only noise set as an absolute floor or an SNR
    /// over the whole sample rate.
    fn signal_info(&self) -> Option<SignalInfo> {
        None
    }
    /// The station's frequency in the samples, Hz from their centre, for a stack tuned `tune_offset_hz` above the station
    /// ([`Tuning::tune_offset_hz`]). The default is `-tune_offset_hz`: the samples are what a radio tuned with the stack's offset
    /// sees. A generator that makes its station at 0 Hz returns 0. The framework moves the signal by the difference, so the
    /// station lands on the frequency the receiver is tuned to.
    fn station_offset_hz(&self, tune_offset_hz: Hz) -> Hz {
        -tune_offset_hz
    }
}

impl<G: Generator + ?Sized> Generator for Box<G> {
    fn rate_hz(&self) -> Hz {
        (**self).rate_hz()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        (**self).generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        (**self).signal_info()
    }
    fn station_offset_hz(&self, tune_offset_hz: Hz) -> Hz {
        (**self).station_offset_hz(tune_offset_hz)
    }
}

/// What a generator declares about its clean signal (see [`Generator::signal_info`]).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SignalInfo {
    /// The occupied bandwidth an SNR is measured in, Hz.
    pub bandwidth_hz: Hz,
    /// Symbols per second, for Es/N0; `None` for a signal without symbols (analogue).
    pub symbol_rate_hz: Option<Hz>,
    /// Information bits per second, for Eb/N0; `None` when not meaningful.
    pub bit_rate_hz: Option<Hz>,
    /// Mean power while transmitting. sdrkit's generators make 1 (0 dBFS); a generator that cannot (a fixed-level recording) says
    /// what it makes.
    pub power: f64,
}

impl SignalInfo {
    /// A unit-power signal `bandwidth_hz` wide, with no symbol or bit rate.
    pub const fn new(bandwidth_hz: Hz) -> Self {
        SignalInfo {
            bandwidth_hz,
            symbol_rate_hz: None,
            bit_rate_hz: None,
            power: 1.0,
        }
    }
    /// With this symbol rate.
    pub const fn with_symbol_rate(mut self, hz: Hz) -> Self {
        self.symbol_rate_hz = Some(hz);
        self
    }
    /// With this information bit rate.
    pub const fn with_bit_rate(mut self, hz: Hz) -> Self {
        self.bit_rate_hz = Some(hz);
        self
    }
    /// With this mean power while transmitting.
    pub const fn with_power(mut self, power: f64) -> Self {
        self.power = power;
        self
    }
}

/// Makes a stack's receiver.
///
/// # Errors
///
/// [`ConfigError`] if the receiver cannot be built with this configuration.
pub type MakeReceiver = fn(&StackConfig) -> Result<Box<dyn Receiver>, ConfigError>;

/// Makes a stack's generator.
///
/// # Errors
///
/// [`ConfigError`] if the generator cannot make a signal with this configuration (a rate it cannot serve).
pub type MakeGenerator = fn(&GenConfig) -> Result<Box<dyn Generator>, ConfigError>;

crate::choice_enum! {
    /// What "automatic gain" means for a stack while the user leaves the radio gain on automatic.
    #[derive(Default)]
    pub enum AutoGain {
        /// The radio's own automatic gain control. Right for a signal of constant envelope (FM).
        #[default]
        Radio = ("radio", "Radio AGC"),
        /// A fixed gain near the top of the radio's range. Right for short pulses (ADS-B).
        High = ("high", "High fixed gain"),
        /// The engine finds a gain once and holds it. Right for OFDM (DVB-T, DAB).
        Hold = ("hold", "Found once and held"),
    }
}

/// How a scan of channels decides that a channel of a stack holds a station, and where it reads the name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LockProfile {
    /// The time after the retune before the channel is judged, seconds.
    pub settle_s: f64,
    /// The time after which a channel with no signal is left. `None` waits for the timeout.
    pub quiet_after_s: Option<f64>,
    /// The SNR that counts as some signal, dB.
    pub signal_snr_db: f32,
    /// The time after which a channel with no lock is left, seconds.
    pub timeout_s: f64,
    /// What a lock is.
    pub lock: LockCondition,
    /// The time to wait for a name after the lock, seconds.
    pub collect_s: f64,
    /// Where the name is read. The first that is not empty is used.
    pub name_from: &'static [NameSource],
    /// The keys of result fields that a scan keeps for each channel found.
    pub capture: &'static [&'static str],
}

impl LockProfile {
    /// The profile of a stack that declares none: a locked lock state within 2 seconds.
    pub const DEFAULT: LockProfile = LockProfile {
        settle_s: 0.15,
        quiet_after_s: Some(0.5),
        signal_snr_db: 6.0,
        timeout_s: 2.0,
        lock: LockCondition::LockState {
            at_least: LockState::Locked,
        },
        collect_s: 0.0,
        name_from: &[],
        capture: &[],
    };
}

impl Default for LockProfile {
    fn default() -> Self {
        LockProfile::DEFAULT
    }
}

/// A condition on what a stack publishes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockCondition {
    /// The lock state is at least this.
    LockState {
        /// The lowest lock state that passes.
        at_least: LockState,
    },
    /// The SNR is at least `min_db`.
    Snr {
        /// dB.
        min_db: f32,
    },
    /// The number in result field `key` is above `min`.
    FieldAbove {
        /// The key of the result field.
        key: &'static str,
        /// The number it must exceed.
        min: f64,
    },
    /// Result field `key` holds a value that is not empty text.
    FieldPresent {
        /// The key of the result field.
        key: &'static str,
    },
    /// One of these holds.
    Any(&'static [LockCondition]),
    /// All of these hold.
    All(&'static [LockCondition]),
}

/// Where a scan reads the name of a station.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameSource {
    /// The text of result field `key`.
    Field(&'static str),
    /// The summary of the status.
    Summary,
}

/// A stack, as the engine and the stack list know it. One per stack crate; `sdrkit-host-catalog` lists them all.
pub struct StackDescriptor {
    /// Stable: saved in settings and recordings. Lower case, no spaces ("fm", "adsb").
    pub id: &'static str,
    pub name: &'static str,
    /// The heading in the stack list.
    pub group: &'static str,
    /// One plain sentence for the stack list.
    pub blurb: &'static str,
    pub tuning: Tuning,
    pub submodes: &'static [SubMode],
    /// The settings schema.
    pub settings: fn() -> Arc<Schema>,
    pub make_receiver: MakeReceiver,
    pub make_generator: Option<MakeGenerator>,
    /// How a scan of channels judges a channel of this stack. [`LockProfile::DEFAULT`] for most stacks.
    pub lock_profile: LockProfile,
    /// The signal types that the stack receives. The engine starts this stack for a request that names one of them.
    pub signal_types: &'static [SignalType],
    /// What automatic gain means for this stack.
    pub auto_gain: AutoGain,
    /// How a host tests the stack: its test-signal runs and its recorded vectors. `None` keeps the generic test run.
    pub verify: Option<&'static VerifyHooks>,
    /// The stack crate's benchmarks (a crate with several stacks lists them once, with its first stack).
    pub benchmarks: Option<fn() -> Vec<Benchmark>>,
}

impl StackDescriptor {
    /// The sample rate that the stack would like with the settings `values`: the rate that one of its settings gives
    /// ([`Setting::sets_rate`](crate::Setting::sets_rate)), else the rate of its table. `values` may be of the schema of
    /// the stack or of an owner that holds the settings of the stack under the same keys.
    pub fn rate_hz(&self, values: &Values) -> Hz {
        (self.settings)()
            .rate_hz(values)
            .unwrap_or(self.tuning.rate_hz)
    }
}

/// Finds a declared item by its key: a stack descriptor by stack id, a band plan by plan key. The engine supplies a lookup to
/// a service when it makes the service. The lookup can hold state, such as the stack list of one engine.
pub type Lookup<T> = Arc<dyn Fn(&str) -> Option<T> + Send + Sync>;

/// Finds the descriptor of a stack by its id.
pub type StackLookup = Lookup<&'static StackDescriptor>;

impl std::fmt::Debug for StackDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackDescriptor")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::{field, Field, Published};
    use crate::settings::{Setting, SettingSet};
    use crate::value::Value;
    use crate::EventDef;

    const POWER: Field<field::F64> = Field::new("power", "Power", "");
    static BLOCK: EventDef = EventDef::new("meter.block", "Block", &[&POWER]);
    const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 10.0, 0.0);

    crate::settings! {
        /// The meter's settings.
        enum MeterSetting {
            Gain = GAIN,
        }
    }

    // A small stack that reports the mean power of what it is fed.
    struct Meter {
        results: Published,
        gain: f32,
    }
    impl Receiver for Meter {
        fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
            if input.rate_hz < 1000.0 {
                Err(ConfigError::RateTooLow {
                    need: 1000.0,
                    got: input.rate_hz,
                })
            } else {
                Ok(())
            }
        }
        fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
            let p =
                iq.iter().map(|s| s.norm_sqr()).sum::<f32>() / iq.len().max(1) as f32 * self.gain;
            self.results.set(&POWER, f64::from(p));
            out.events
                .push(BLOCK.event(0.0, 0.0).set(&POWER, f64::from(p)));
        }
    }
    impl Owner for Meter {
        fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
            match MeterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
                MeterSetting::Gain => self.gain = GAIN.read(id, value)? as f32,
            }
            Ok(())
        }
        fn results(&self) -> &crate::Results {
            self.results.results()
        }
    }

    static METER: StackDescriptor = StackDescriptor {
        id: "meter",
        name: "Meter",
        group: "Tools",
        blurb: "Shows the power of the signal.",
        tuning: Tuning::anywhere(100e6, 48e3, 1e3, 10e3),
        submodes: &[],
        settings: MeterSetting::schema,
        make_receiver: |cfg| {
            Ok(Box::new(Meter {
                results: Published::new(&[&POWER]),
                gain: cfg.settings.get(&GAIN) as f32,
            }))
        },
        make_generator: None,
        lock_profile: LockProfile::DEFAULT,
        signal_types: &[SignalType::Carrier],
        auto_gain: AutoGain::Radio,
        verify: None,
        benchmarks: None,
    };

    fn lookup(id: &str) -> Option<&'static StackDescriptor> {
        (id == METER.id).then_some(&METER)
    }

    #[test]
    fn a_stack_is_made_from_its_descriptor_and_fed() {
        let schema = (METER.settings)();
        let find: StackLookup = Arc::new(lookup);
        let desc = find("meter").unwrap();
        assert!(find("fm").is_none());
        assert_eq!(desc.signal_types, &[SignalType::Carrier]);
        let mut rx = (desc.make_receiver)(&StackConfig::new(Values::defaults(&schema))).unwrap();
        assert!(rx.configure(&InputInfo::new(10.0, 0.0, 0.0)).is_err());
        rx.configure(&InputInfo::new(48e3, 100e6, 100e6).recording())
            .unwrap();
        let gain = schema.id("gain").unwrap();
        let two = Value::F64(2.0);
        schema.check(gain, &two).unwrap();
        rx.apply(gain, &two).unwrap();
        assert!(schema.check(gain, &Value::F64(11.0)).is_err());
        assert_eq!(
            rx.apply(SettingId(7), &two),
            Err(SettingError::NoSuchId { id: SettingId(7) })
        );
        assert_eq!(
            rx.restore_state(&StateBlob::new("meter", 1, vec![])),
            Err(StateError::NoState)
        );
        let mut out = Output::default();
        rx.feed(&[Complex32::new(1.0, 0.0); 16], &mut out);
        assert_eq!(rx.results().fields().get(&POWER), Some(2.0));
        assert_eq!(out.events.len(), 1);
        assert!(out.events[0].is(&BLOCK));
    }

    #[test]
    fn audio_of_a_changed_channel_count_starts_a_new_run() {
        let mut out = Output::default();
        out.push_audio(&[0.1, 0.2, 0.3, 0.4], 48e3, 2);
        out.push_audio(&[0.5, 0.6], 48e3, 2);
        out.push_audio(&[], 48e3, 1);
        out.push_audio(&[0.7, 0.8, 0.9], 48e3, 1);
        let runs: Vec<_> = out.audio_runs().collect();
        assert_eq!(runs.len(), 2);
        assert_eq!((runs[0].channels, runs[0].samples.len()), (2, 6));
        assert_eq!(
            (runs[1].channels, runs[1].samples),
            (1, &[0.7, 0.8, 0.9][..])
        );
        assert_eq!(out.audio_format(), Some((48e3, 1)));
        out.clear();
        assert_eq!(out.audio_runs().count(), 0);
        assert_eq!(out.audio_format(), None);
    }
}

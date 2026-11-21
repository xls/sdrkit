//! The senders of `atv` and `fpv` (`atv.transmit`, `fpv.transmit`): the modulator of the stack's test signal
//! ([`AtvGenerator`], [`FpvGenerator`]) fed with the pictures and the sound of the host and written continuously to the
//! transmit sink.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is the host's audio, mono at any rate. Output
//! `out` is complex baseband at the session's rate for `tx.sink`, the channel centred on the dial. Output `keyed` mutes the
//! receive audio of the stack from key down to a short hold after the end mark. Each pass writes as many samples as the
//! pass brings in.
//!
//! `start` begins at the start of a field with a rise of [`RAMP_S`]; `stop` ends at the end of a field with a fall of
//! [`RAMP_S`] and the end mark. The sum of the peaks of the carriers is [`PEAK`] of full scale. A picture given by `frame`
//! is fitted to the picture of the system ([`fit_frame`]) and taken at the start of the next field; without one the test
//! picture goes out. On a live radio whose host times bursts (`HostStreams::transmit_lead`), the first sample states its
//! time on air (`TagKind::BurstStart`). The transmit setting of the engine is the only switch.

use crate::card::{Pattern, Planes};
use crate::fpv::{FpvGenParams, FpvGenerator, MIN_RATE_HZ as FPV_MIN_RATE_HZ};
use crate::gen_atv::{AtvGenParams, AtvGenerator};
use crate::gen_video::SecamIdent;
use crate::settings::{
    AtvTxSetting, FpvGenSystem, FpvPattern, FpvTxSetting, GenColour, GenSetup, TxSubcarrier,
    TxSystem, TX_AC_MS, TX_COLOUR, TX_COMPRESSION, TX_DEVIATION_MHZ, TX_FPV_PICTURE,
    TX_FPV_SYSTEM, TX_GROUP_DELAY, TX_NYQUIST, TX_PICTURE, TX_SECAM_ID, TX_SETUP, TX_SOUND,
    TX_SUBCARRIER, TX_SYNC_HIGH, TX_SYSTEM,
};
use crate::sound_gen::{Programme, SecondSound, PROGRAMME_STEP};
use crate::systems::{Colour, System, SystemId};
use crate::typed::{TvFrameArgs, TvPixels, TvTxFields, TV_FRAME, TV_START, TV_STOP};
use sdrkit_codec_image::colour::yuv_from_rgb;
use sdrkit_codec_image::geom::resize_box;
use sdrkit_codec_image::{PixelFormat as ImageFormat, Raster};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, MicInput, TxMute};
use sdrkit_core::interface::Fields;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Generator,
    Published, Record, Results, Schema, SettingError, SettingId, SettingSet, StageError, Value,
    Values,
};
use sdrkit_graph::components::{HostIn, TxSink};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, GraphBuilder, Io, Port, RunContext};
use std::sync::Arc;

/// The sum of the peaks of the carriers, of full scale.
pub const PEAK: f32 = 0.9;
/// The peak of the vision carrier over its sync tip: the overshoot of the vestigial-sideband filter at the sync edges.
pub const VSB_OVERSHOOT: f64 = 1.03;
/// The rise at the start and the fall at the end (raised cosine), seconds.
pub const RAMP_S: f64 = 0.005;
/// The largest width or height of a picture given to `frame`.
pub const MAX_SIDE: u32 = 8192;
/// The host's audio kept while it waits, seconds; older audio is dropped.
const MIC_KEEP_S: f64 = 0.5;
/// The host's audio that waits before the first is read, seconds: room for the passes to come unevenly.
const MIC_PRIME_S: f64 = 0.05;
/// The samples the modulator makes at a time.
const CHUNK: usize = 16_384;

sdrkit_core::commands! {
    /// The commands of a sender.
    pub enum TvTxCommand {
        /// Start.
        Start = TV_START,
        /// Stop.
        Stop = TV_STOP,
        /// Set the picture.
        Frame = TV_FRAME.command,
    }
}

/// Why a sender refuses a command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TvTxError {
    /// The session's rate is below what the system needs (Hz).
    Rate {
        /// The rate the system needs, Hz.
        need_hz: f64,
    },
    /// The system and the colour never existed together (colour on A or E, NTSC on a positive system).
    Combination,
    /// The width or the height is outside 1 to [`MAX_SIDE`].
    Size,
    /// The pixel bytes do not match the width, the height and the pixel format.
    Length,
    /// `start` came while the transmission ends.
    Ending,
}

impl std::fmt::Display for TvTxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TvTxError::Rate { need_hz } => write!(
                f,
                "the system needs a sample rate of at least {} Msps",
                need_hz / 1e6
            ),
            TvTxError::Combination => f.write_str("this colour does not exist on this system"),
            TvTxError::Size => f.write_str("the width and the height must be 1 to 8192 pixels"),
            TvTxError::Length => f.write_str(
                "the pixel bytes do not match the width, the height and the pixel format",
            ),
            TvTxError::Ending => f.write_str("the transmission is ending"),
        }
    }
}

impl std::error::Error for TvTxError {}

fn failed(e: TvTxError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The bytes of one pixel of `format`.
fn bytes_per_pixel(format: TvPixels) -> usize {
    match format {
        TvPixels::Rgb8 => 3,
        TvPixels::Bgra => 4,
        TvPixels::Grey8 => 1,
    }
}

/// Checks the size of a picture: `width` x `height` pixels of `format` in `len` bytes.
fn check_size(len: usize, width: i64, height: i64, format: TvPixels) -> Result<(u32, u32), TvTxError> {
    let side = |v: i64| {
        u32::try_from(v)
            .ok()
            .filter(|v| (1..=MAX_SIDE).contains(v))
            .ok_or(TvTxError::Size)
    };
    let (w, h) = (side(width)?, side(height)?);
    if len != w as usize * h as usize * bytes_per_pixel(format) {
        return Err(TvTxError::Length);
    }
    Ok((w, h))
}

/// The picture `pixels` (`width` x `height`, rows top first, no padding, in `format`) fitted into a picture of `w` x `h`
/// pixels as Y, U and V: scaled by the smaller of the two ratios of the sizes and centred, the rest black, so a wider
/// picture gets bars above and below and a narrower one bars at the sides. Where the picture is larger than its place it is
/// reduced by the area mean of the pixels each place covers; where it is smaller, place pixel (x, y) takes pixel
/// (floor(x W / w'), floor(y H / h')) of it, W x H being the picture and w' x h' its place.
///
/// # Errors
///
/// [`TvTxError::Size`] or [`TvTxError::Length`].
pub fn fit_frame(
    pixels: &[u8],
    width: u32,
    height: u32,
    format: TvPixels,
    w: usize,
    h: usize,
) -> Result<Planes, TvTxError> {
    let (width, height) = check_size(pixels.len(), i64::from(width), i64::from(height), format)?;
    let (fw, fh) = (f64::from(width), f64::from(height));
    let s = (w as f64 / fw).min(h as f64 / fh);
    let dw = ((fw * s).round() as usize).clamp(1, w);
    let dh = ((fh * s).round() as usize).clamp(1, h);
    let (dx, dy) = ((w - dw) / 2, (h - dh) / 2);
    let smaller;
    let (data, sw, sh, layout) = if width as usize > dw || height as usize > dh {
        let (fmt, bytes) = match format {
            TvPixels::Rgb8 => (
                ImageFormat::Bgra,
                pixels
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|p| [p[2], p[1], p[0], 255])
                    .collect(),
            ),
            TvPixels::Bgra => (ImageFormat::Bgra, pixels.to_vec()),
            TvPixels::Grey8 => (ImageFormat::Grey8, pixels.to_vec()),
        };
        let r = Raster::from_vec(fmt, width, height, bytes).map_err(|_| TvTxError::Length)?;
        smaller = resize_box(&r, (dw as u32).min(width), (dh as u32).min(height));
        let layout = if format == TvPixels::Grey8 {
            TvPixels::Grey8
        } else {
            TvPixels::Bgra
        };
        (
            smaller.data(),
            smaller.width() as usize,
            smaller.height() as usize,
            layout,
        )
    } else {
        (pixels, width as usize, height as usize, format)
    };
    let bpp = bytes_per_pixel(layout);
    let mut out = Planes::new(w, h, 0.0);
    for y in 0..dh {
        let sy = y * sh / dh;
        for x in 0..dw {
            let sx = x * sw / dw;
            let p = &data[(sy * sw + sx) * bpp..][..bpp];
            let [r, g, b] = match layout {
                TvPixels::Rgb8 => [p[0], p[1], p[2]],
                TvPixels::Bgra => [p[2], p[1], p[0]],
                TvPixels::Grey8 => [p[0]; 3],
            };
            let [yy, u, v] = yuv_from_rgb(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
            );
            let i = (dy + y) * w + dx + x;
            out.y[i] = yy;
            out.u[i] = u;
            out.v[i] = v;
        }
    }
    Ok(out)
}

/// What a sender needs of a modulator.
pub trait TvModulator: Send {
    /// Makes the next `out.len()` samples.
    fn make(&mut self, out: &mut [Complex32]);
    /// Shows a picture of the host from the next field on (`None`: the test picture).
    fn show(&mut self, picture: Option<Planes>);
    /// The size of the picture: width, height.
    fn picture_size(&self) -> (usize, usize);
    /// The pictures of the host taken so far.
    fn pictures_taken(&self) -> u64;
    /// The programme samples that the next `n` output samples read.
    fn programme_due(&self, n: usize) -> usize;
    /// Adds host audio at the output rate over [`PROGRAMME_STEP`].
    fn push_programme(&mut self, x: &[f32]);
    /// Fields a second.
    fn field_hz(&self) -> f64;
}

impl TvModulator for AtvGenerator {
    fn make(&mut self, out: &mut [Complex32]) {
        self.generate(out);
    }
    fn show(&mut self, picture: Option<Planes>) {
        AtvGenerator::show(self, picture);
    }
    fn picture_size(&self) -> (usize, usize) {
        let r = self.system().raster;
        (r.width as usize, r.height as usize)
    }
    fn pictures_taken(&self) -> u64 {
        AtvGenerator::pictures_taken(self)
    }
    fn programme_due(&self, n: usize) -> usize {
        AtvGenerator::programme_due(self, n)
    }
    fn push_programme(&mut self, x: &[f32]) {
        AtvGenerator::push_programme(self, x);
    }
    fn field_hz(&self) -> f64 {
        self.system().raster.field_hz()
    }
}

impl TvModulator for FpvGenerator {
    fn make(&mut self, out: &mut [Complex32]) {
        self.generate(out);
    }
    fn show(&mut self, picture: Option<Planes>) {
        FpvGenerator::show(self, picture);
    }
    fn picture_size(&self) -> (usize, usize) {
        let r = self.raster();
        (r.width as usize, r.height as usize)
    }
    fn pictures_taken(&self) -> u64 {
        FpvGenerator::pictures_taken(self)
    }
    fn programme_due(&self, n: usize) -> usize {
        FpvGenerator::programme_due(self, n)
    }
    fn push_programme(&mut self, x: &[f32]) {
        FpvGenerator::push_programme(self, x);
    }
    fn field_hz(&self) -> f64 {
        self.raster().field_hz()
    }
}

/// The settings of a sender and the modulator they make.
pub trait TxStack: Default + Send + 'static {
    /// The name of the component.
    const ID: &'static str;
    /// The settings.
    fn schema() -> Arc<Schema>;
    /// Takes the value of setting `id`.
    ///
    /// # Errors
    ///
    /// A [`SettingError`] for an unknown setting or a value of the wrong kind.
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError>;
    /// The values of the settings.
    fn values(&self) -> Values;
    /// Whether the host's audio goes out.
    fn sound(&self) -> bool;
    /// The modulator at `rate` and the name of the system it sends.
    ///
    /// # Errors
    ///
    /// [`TvTxError::Rate`] or [`TvTxError::Combination`].
    fn build(&self, rate: f64) -> Result<(Box<dyn TvModulator>, String), TvTxError>;
}

/// The settings of the `atv` sender.
#[derive(Clone, Debug, PartialEq)]
pub struct AtvTx {
    system: TxSystem,
    colour: GenColour,
    picture: Pattern,
    sound: bool,
    compression: f64,
    group_delay: bool,
    nyquist: bool,
    setup: GenSetup,
    secam_id: SecamIdent,
}

impl Default for AtvTx {
    fn default() -> Self {
        AtvTx {
            system: TX_SYSTEM.default,
            colour: TX_COLOUR.default,
            picture: TX_PICTURE.default,
            sound: TX_SOUND.default,
            compression: TX_COMPRESSION.default,
            group_delay: TX_GROUP_DELAY.default,
            nyquist: TX_NYQUIST.default,
            setup: TX_SETUP.default,
            secam_id: TX_SECAM_ID.default,
        }
    }
}

impl AtvTx {
    /// The generator parameters at `rate`: the carrier level puts the sum of the peaks of the vision carrier (the sync tip,
    /// or super white at 1.3 on a positive system, with [`VSB_OVERSHOOT`]) and of the sound carrier (1.8 times its level
    /// with AM sound) at [`PEAK`].
    pub fn params(&self, rate: f64) -> AtvGenParams {
        let id = self.system.system();
        let sys = System::get(id);
        let s = sys.raster.sync_level;
        let vision = VSB_OVERSHOOT
            * if sys.positive {
                (1.3 - s) / (1.0 - s)
            } else {
                1.0
            };
        let sound = if self.sound {
            10f64.powf(sys.sound_db / 20.0) * if sys.am_sound { 1.8 } else { 1.0 }
        } else {
            0.0
        };
        AtvGenParams {
            rate_hz: rate,
            system: id,
            colour: self.colour.colour(id),
            pattern: self.picture,
            sound: if self.sound {
                Programme::Unmodulated
            } else {
                Programme::NoCarrier
            },
            host_sound: self.sound,
            second: SecondSound::None,
            sync_compression: self.compression / 100.0,
            group_delay: self.group_delay,
            nyquist_slope: self.nyquist,
            setup: self.setup == GenSetup::Ire75,
            secam_ident: self.secam_id,
            level: f64::from(PEAK) / (vision + sound),
            ..AtvGenParams::default()
        }
    }
}

impl TxStack for AtvTx {
    const ID: &'static str = "atv.transmit";
    fn schema() -> Arc<Schema> {
        AtvTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match AtvTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AtvTxSetting::System => self.system = TX_SYSTEM.read(id, v)?,
            AtvTxSetting::Colour => self.colour = TX_COLOUR.read(id, v)?,
            AtvTxSetting::Picture => self.picture = TX_PICTURE.read(id, v)?,
            AtvTxSetting::Sound => self.sound = TX_SOUND.read(id, v)?,
            AtvTxSetting::Compression => self.compression = TX_COMPRESSION.read(id, v)?,
            AtvTxSetting::GroupDelay => self.group_delay = TX_GROUP_DELAY.read(id, v)?,
            AtvTxSetting::Nyquist => self.nyquist = TX_NYQUIST.read(id, v)?,
            AtvTxSetting::Setup => self.setup = TX_SETUP.read(id, v)?,
            AtvTxSetting::SecamId => self.secam_id = TX_SECAM_ID.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (AtvTxSetting::System, Value::Choice(self.system.index())),
            (AtvTxSetting::Colour, Value::Choice(self.colour.index())),
            (AtvTxSetting::Picture, Value::Choice(self.picture.index())),
            (AtvTxSetting::Sound, Value::Bool(self.sound)),
            (AtvTxSetting::Compression, Value::F64(self.compression)),
            (AtvTxSetting::GroupDelay, Value::Bool(self.group_delay)),
            (AtvTxSetting::Nyquist, Value::Bool(self.nyquist)),
            (AtvTxSetting::Setup, Value::Choice(self.setup.index())),
            (AtvTxSetting::SecamId, Value::Choice(self.secam_id.index())),
        ])
    }
    fn sound(&self) -> bool {
        self.sound
    }
    fn build(&self, rate: f64) -> Result<(Box<dyn TvModulator>, String), TvTxError> {
        let p = self.params(rate);
        let sys = System::get(p.system);
        let need_hz = sys.channel_mhz * 1e6;
        if rate < need_hz {
            return Err(TvTxError::Rate { need_hz });
        }
        if !p.valid() {
            return Err(TvTxError::Combination);
        }
        let name = sys.name(p.colour());
        Ok((Box::new(AtvGenerator::new(p)), name))
    }
}

/// The settings of the `fpv` sender.
#[derive(Clone, Debug, PartialEq)]
pub struct FpvTx {
    system: FpvGenSystem,
    picture: FpvPattern,
    sound: bool,
    subcarrier: TxSubcarrier,
    deviation_mhz: f64,
    sync_high: bool,
    ac_ms: f64,
}

impl Default for FpvTx {
    fn default() -> Self {
        FpvTx {
            system: TX_FPV_SYSTEM.default,
            picture: TX_FPV_PICTURE.default,
            sound: TX_SOUND.default,
            subcarrier: TX_SUBCARRIER.default,
            deviation_mhz: TX_DEVIATION_MHZ.default,
            sync_high: TX_SYNC_HIGH.default,
            ac_ms: TX_AC_MS.default,
        }
    }
}

impl FpvTx {
    /// The generator parameters at `rate`: a constant envelope at [`PEAK`].
    pub fn params(&self, rate: f64) -> FpvGenParams {
        FpvGenParams {
            rate_hz: rate,
            ntsc: self.system == FpvGenSystem::Ntsc,
            bars: self.picture == FpvPattern::Bars,
            sound_hz: match (self.sound, self.subcarrier) {
                (false, _) => 0.0,
                (true, TxSubcarrier::S65) => 6.5e6,
                (true, TxSubcarrier::S60) => 6.0e6,
            },
            host_sound: self.sound,
            none: false,
            centre_hz: 0.0,
            deviation_hz: self.deviation_mhz * 1e6,
            sync_high: self.sync_high,
            ac_s: self.ac_ms / 1e3,
            amplitude: f64::from(PEAK),
        }
    }
}

impl TxStack for FpvTx {
    const ID: &'static str = "fpv.transmit";
    fn schema() -> Arc<Schema> {
        FpvTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match FpvTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FpvTxSetting::System => self.system = TX_FPV_SYSTEM.read(id, v)?,
            FpvTxSetting::Picture => self.picture = TX_FPV_PICTURE.read(id, v)?,
            FpvTxSetting::Sound => self.sound = TX_SOUND.read(id, v)?,
            FpvTxSetting::Subcarrier => self.subcarrier = TX_SUBCARRIER.read(id, v)?,
            FpvTxSetting::DeviationMhz => self.deviation_mhz = TX_DEVIATION_MHZ.read(id, v)?,
            FpvTxSetting::SyncHigh => self.sync_high = TX_SYNC_HIGH.read(id, v)?,
            FpvTxSetting::AcMs => self.ac_ms = TX_AC_MS.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (FpvTxSetting::System, Value::Choice(self.system.index())),
            (FpvTxSetting::Picture, Value::Choice(self.picture.index())),
            (FpvTxSetting::Sound, Value::Bool(self.sound)),
            (FpvTxSetting::Subcarrier, Value::Choice(self.subcarrier.index())),
            (FpvTxSetting::DeviationMhz, Value::F64(self.deviation_mhz)),
            (FpvTxSetting::SyncHigh, Value::Bool(self.sync_high)),
            (FpvTxSetting::AcMs, Value::F64(self.ac_ms)),
        ])
    }
    fn sound(&self) -> bool {
        self.sound
    }
    fn build(&self, rate: f64) -> Result<(Box<dyn TvModulator>, String), TvTxError> {
        if rate < FPV_MIN_RATE_HZ {
            return Err(TvTxError::Rate {
                need_hz: FPV_MIN_RATE_HZ,
            });
        }
        let p = self.params(rate);
        let (id, colour) = if p.ntsc {
            (SystemId::M, Colour::Ntsc)
        } else {
            (SystemId::G, Colour::Pal)
        };
        Ok((
            Box::new(FpvGenerator::new(p)),
            System::get(id).name(colour),
        ))
    }
}

/// A picture of the host, kept to be fitted to the system at each start.
struct Frame {
    pixels: Arc<[u8]>,
    width: u32,
    height: u32,
    format: TvPixels,
}

impl Frame {
    fn fit(&self, (w, h): (usize, usize)) -> Result<Planes, TvTxError> {
        fit_frame(&self.pixels, self.width, self.height, self.format, w, h)
    }
}

/// One transmission: the modulator, the output, the samples made and the end.
struct Burst {
    m: Box<dyn TvModulator>,
    out: BurstOut,
    /// Samples made.
    made: u64,
    /// Samples of the rise and of the fall.
    ramp: u64,
    /// The sample at which the transmission ends.
    end_at: Option<u64>,
    /// Samples a field.
    field_len: f64,
    sound: bool,
    /// The host's audio is read.
    primed: bool,
    system: String,
}

/// The gain of a raised-cosine rise at sample `k` of `n`.
fn rise(k: u64, n: u64) -> f32 {
    (0.5 - 0.5 * (std::f64::consts::PI * (k as f64 + 0.5) / n as f64).cos()) as f32
}

impl Burst {
    /// Makes samples until at least `room` output samples wait, or until the transmission is made.
    fn fill(&mut self, room: usize, mic: &mut MicInput, made: &mut Vec<Complex32>, audio: &mut Vec<f32>) {
        let prime = (MIC_PRIME_S * mic.rate()) as usize;
        while self.out.wants(room) {
            let n = self
                .end_at
                .map_or(CHUNK, |e| (e - self.made).min(CHUNK as u64) as usize);
            if n == 0 {
                self.out.finish(made);
                return;
            }
            if self.sound {
                audio.clear();
                audio.resize(self.m.programme_due(n), 0.0);
                self.primed |= mic.waiting() >= prime;
                if self.primed {
                    mic.read(audio);
                }
                self.m.push_programme(audio);
            }
            made.clear();
            made.resize(n, Complex32::default());
            self.m.make(made);
            self.shape(made);
            self.out.push(made);
            self.made += n as u64;
        }
    }

    /// The rise at the start and the fall before the end on the samples `x` that follow the ones made.
    fn shape(&self, x: &mut [Complex32]) {
        let (k0, r) = (self.made, self.ramp);
        let end = self.end_at.unwrap_or(u64::MAX);
        if k0 >= r && end.saturating_sub(k0 + x.len() as u64) >= r {
            return;
        }
        for (i, z) in x.iter_mut().enumerate() {
            let k = k0 + i as u64;
            let mut g = 1.0;
            if k < r {
                g *= rise(k, r);
            }
            let left = end - k;
            if left <= r {
                g *= rise(left - 1, r);
            }
            *z *= g;
        }
    }

    /// Ends the transmission at the first end of a field that leaves room for the fall.
    fn stop(&mut self) {
        if self.end_at.is_none() {
            let f = ((self.made + self.ramp) as f64 / self.field_len).ceil();
            self.end_at = Some((f * self.field_len).round() as u64);
        }
    }

    /// The fields sent.
    fn fields(&self) -> u64 {
        (self.out.sent() as f64 / self.field_len) as u64
    }
}

/// The sender of a stack (`atv.transmit`, `fpv.transmit`). See the module.
pub struct TvTransmit<S: TxStack> {
    cfg: S,
    rate: f64,
    dial: f64,
    transmit: bool,
    mic: MicInput,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    timing: BurstTiming,
    frame: Option<Frame>,
    /// The pictures shown and the fields sent of the last transmission.
    last: (u64, u64),
    results: Published,
    made: Vec<Complex32>,
    audio: Vec<f32>,
}

impl<S: TxStack> Default for TvTransmit<S> {
    fn default() -> Self {
        let mut t = TvTransmit {
            cfg: S::default(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            mic: MicInput::new(1.0, MIC_KEEP_S),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            timing: BurstTiming::default(),
            frame: None,
            last: (0, 0),
            results: Published::new(TvTxFields::SHAPE.fields),
            made: Vec::new(),
            audio: Vec::new(),
        };
        t.publish();
        t
    }
}

impl<S: TxStack> TvTransmit<S> {
    /// Whether a transmission is under way.
    pub fn sending(&self) -> bool {
        self.burst.is_some()
    }

    /// Starts sending; a transmission under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or a [`TvTxError`]: the rate, the combination, a picture that does not fit, or a
    /// transmission that ends.
    pub fn start(&mut self) -> Result<(), CommandError> {
        if !self.transmit {
            return Err(CommandError::TransmitOff);
        }
        if let Some(b) = &self.burst {
            return match b.end_at {
                Some(_) => Err(failed(TvTxError::Ending)),
                None => Ok(()),
            };
        }
        let (mut m, system) = self.cfg.build(self.rate).map_err(failed)?;
        if let Some(f) = &self.frame {
            m.show(Some(f.fit(m.picture_size()).map_err(failed)?));
        }
        let out = BurstOut::new(self.rate, self.rate, PEAK)
            .map_err(|_| failed(TvTxError::Rate { need_hz: 1.0 }))?;
        self.mic.restart();
        self.burst = Some(Burst {
            field_len: self.rate / m.field_hz(),
            m,
            out,
            made: 0,
            ramp: ((RAMP_S * self.rate).round() as u64).max(1),
            end_at: None,
            sound: self.cfg.sound(),
            primed: false,
            system,
        });
        Ok(())
    }

    /// Ends the transmission at the end of a field.
    pub fn stop(&mut self) {
        if let Some(b) = &mut self.burst {
            b.stop();
        }
    }

    /// Sets the picture: shown from the next field, kept for the next start. Width and height 0 with no pixels: the test
    /// picture again.
    ///
    /// # Errors
    ///
    /// [`TvTxError::Size`] or [`TvTxError::Length`].
    pub fn set_frame(&mut self, a: &TvFrameArgs) -> Result<(), TvTxError> {
        if a.width == 0 && a.height == 0 && a.pixels.is_empty() {
            self.frame = None;
            if let Some(b) = &mut self.burst {
                b.m.show(None);
            }
            return Ok(());
        }
        let (width, height) = check_size(a.pixels.len(), a.width, a.height, a.pixel_format)?;
        let f = Frame {
            pixels: Arc::clone(&a.pixels),
            width,
            height,
            format: a.pixel_format,
        };
        if let Some(b) = &mut self.burst {
            b.m.show(Some(f.fit(b.m.picture_size())?));
        }
        self.frame = Some(f);
        Ok(())
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if let Some(b) = self.burst.take() {
            self.last = (b.m.pictures_taken(), b.fields());
            self.end_owed = true;
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let (frames, fields) = self
            .burst
            .as_ref()
            .map_or(self.last, |b| (b.m.pictures_taken(), b.fields()));
        let f = TvTxFields {
            tx_sending: self.burst.is_some(),
            tx_system: self.burst.as_ref().map(|b| b.system.as_str()),
            tx_frames: frames as i64,
            tx_fields: fields as i64,
        };
        self.results.put(&f);
    }

    /// One pass: the samples of the transmission, the end mark and the keyed state.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        let sound = self.burst.as_ref().is_some_and(|b| b.sound);
        self.mic.take(io, 1, sound);
        let mut ended = std::mem::take(&mut self.end_owed);
        if ended {
            end_mark(io, 0);
        }
        if let Some(b) = &mut self.burst {
            b.fill(room, &mut self.mic, &mut self.made, &mut self.audio);
            if b.out.write(io, 0, room, &self.timing) {
                self.last = (b.m.pictures_taken(), b.fields());
                self.burst = None;
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        if io.tick() || ended {
            self.publish();
        }
    }
}

impl<S: TxStack> Component for TvTransmit<S> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            S::ID,
            vec![
                Port::iq("in"),
                Port::audio("mic").optional("silence: an unmodulated sound carrier"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(S::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.mic.set_rate(self.rate / PROGRAMME_STEP as f64, MIC_KEEP_S);
        self.mic.configure(&inputs[1]);
        self.cut();
        self.keyed.reset();
        self.publish();
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            KeyedOut::format(),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        self.mic.attach(ctx);
        if !ctx.transmit {
            self.cut();
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        self.cut();
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.cfg.apply(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.cfg.values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TvTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TvTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TvTxCommand::Start => self.start()?,
            TvTxCommand::Stop => self.stop(),
            TvTxCommand::Frame => {
                let a: TvFrameArgs = TV_FRAME.args(args)?;
                self.set_frame(&a).map_err(failed)?;
            }
        }
        self.publish();
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// The sender of `atv`.
pub type AtvTransmit = TvTransmit<AtvTx>;
/// The sender of `fpv`.
pub type FpvTransmit = TvTransmit<FpvTx>;

/// The keys of the commands of a sender, which a stack routes to its component `transmit`.
pub const COMMANDS: [&str; 3] = ["start", "stop", "frame"];

/// Adds the transmit branch to `b`: the host's audio (`mic`), the sender `sender` (`transmit`) on the session's samples, the
/// transmit sink (`tx`), and, with `audio` (the receive audio output of the graph), the mute (`mute`) that silences it while
/// the sender is keyed; the graph's audio output is then `mute.out`.
pub fn branch(b: GraphBuilder, sender: impl Component + 'static, audio: Option<&str>) -> GraphBuilder {
    let b = b
        .add("mic", HostIn::default())
        .add("transmit", sender)
        .add("tx", TxSink::default())
        .input("mic.in")
        .input("transmit.in")
        .connect("mic.out", "transmit.mic")
        .connect("transmit.out", "tx.in");
    match audio {
        Some(a) => b
            .add("mute", TxMute::default())
            .connect(a, "mute.in")
            .connect("transmit.keyed", "mute.keyed")
            .output("mute.out"),
        None => b,
    }
}

/// Adds the senders to a graph registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add(AtvTx::ID, || Box::new(AtvTransmit::default()));
    r.add(FpvTx::ID, || Box::new(FpvTransmit::default()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wider picture gets black bars above and below, a narrower one at the sides; a picture of the size passes unchanged.
    #[test]
    fn pictures_keep_their_shape() {
        let white = vec![255u8; 1536 * 576 * 3];
        let p = fit_frame(&white, 1536, 576, TvPixels::Rgb8, 768, 576).unwrap();
        // 768 x 288 in the middle.
        assert_eq!(p.y[100 * 768 + 400], 0.0);
        assert!((p.y[144 * 768 + 400] - 1.0).abs() < 1e-6);
        assert!((p.y[431 * 768 + 400] - 1.0).abs() < 1e-6);
        assert_eq!(p.y[432 * 768 + 400], 0.0);
        let tall = vec![200u8; 100 * 300];
        let p = fit_frame(&tall, 100, 300, TvPixels::Grey8, 768, 576).unwrap();
        // 192 x 576 in the middle: columns 288 to 479.
        assert_eq!(p.y[300 * 768 + 287], 0.0);
        assert!((p.y[300 * 768 + 288] - 200.0 / 255.0).abs() < 1e-6);
        assert!((p.y[300 * 768 + 479] - 200.0 / 255.0).abs() < 1e-6);
        assert_eq!(p.y[300 * 768 + 480], 0.0);
        assert_eq!(
            fit_frame(&tall, 100, 301, TvPixels::Grey8, 768, 576).unwrap_err(),
            TvTxError::Length
        );
    }
}

//! The front end of every stack: the dial to 0 Hz, 24 kHz complex, the sideband as 8 kHz complex and real audio (USB, LSB, NFM
//! or AM), the slow AGC, Listen, and the level measurements. A front end with a span ([`HfFront::with_span`]) gives the wide
//! stream only, at the rate of the span that the setting `span` picks.

use crate::typed::FrontFields;
use sdrkit_algo_dsp::agc::Agc;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::fir::{hilbert_design, lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::ring::DelayLine;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom,
    TagKind,
};

/// The rate of the wide complex stream.
pub const WIDE_RATE: f64 = 24_000.0;
/// The rate of the sideband audio.
pub const AUDIO_RATE: f64 = 8_000.0;
/// No stack of this family runs below this input rate.
pub const MIN_RATE: f64 = 12_000.0;
/// Centre of the sideband band in the wide stream.
const BAND_CENTRE: f64 = 2000.0;

/// Power in dB, with -200 for no power.
pub fn db(p: f64) -> f64 {
    if p > 1e-20 {
        10.0 * p.log10()
    } else {
        -200.0
    }
}

sdrkit_core::choice_enum! {
    /// The demodulator in front of the decoders.
    pub enum Demod {
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband, mirrored.
        Lsb = ("lsb", "LSB"),
        /// A discriminator on a ±6 kHz channel (SSTV on 2 m).
        Nfm = ("nfm", "NFM"),
        /// The envelope.
        Am = ("am", "AM"),
    }
}

sdrkit_core::choice_enum! {
    /// The width of the band of the wide stream of a front end with a span.
    pub enum Span {
        /// 14 kHz at 24 kHz.
        Khz14 = ("14k", "14 kHz"),
        /// 48 kHz at 64 kHz.
        Khz48 = ("48k", "48 kHz"),
        /// 96 kHz at 128 kHz.
        Khz96 = ("96k", "96 kHz"),
        /// 192 kHz at 256 kHz.
        Khz192 = ("192k", "192 kHz"),
    }
}

impl Span {
    /// The width of the band, Hz.
    pub fn width_hz(self) -> f64 {
        match self {
            Span::Khz14 => 14e3,
            Span::Khz48 => 48e3,
            Span::Khz96 => 96e3,
            Span::Khz192 => 192e3,
        }
    }

    /// The rate of the wide stream, Hz.
    pub fn wide_rate(self) -> f64 {
        match self {
            Span::Khz14 => WIDE_RATE,
            Span::Khz48 => 64e3,
            Span::Khz96 => 128e3,
            Span::Khz192 => 256e3,
        }
    }

    /// The input rate that the span asks of the radio, Hz: 0 (the rate of the tuning table) for 14 kHz. The radio is 20 kHz
    /// above the dial, so the band reaches 20 kHz plus half the span below the centre; the rate keeps that inside 0.4 of it.
    pub fn radio_rate(self) -> f64 {
        match self {
            Span::Khz14 => 0.0,
            Span::Khz48 => 128e3,
            Span::Khz96 => 192e3,
            Span::Khz192 => 320e3,
        }
    }
}

/// The input rate of a value of the `span` setting ([`Span::radio_rate`]).
fn span_rate(v: &Value) -> f64 {
    match v {
        Value::Choice(i) => Span::from_index(*i).map_or(0.0, Span::radio_rate),
        _ => 0.0,
    }
}

sdrkit_core::choice_enum! {
    /// The demodulators of a stack that takes a sideband only. The indices equal those of [`Demod`].
    pub enum Sideband {
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband, mirrored.
        Lsb = ("lsb", "LSB"),
    }
}

/// The `demod` setting.
pub const DEMOD: Setting<field::Choice<Demod>> =
    Setting::choice("demod", "Demodulator", Demod::Usb)
        .help("How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.");
/// The `demod` setting of a stack that takes a sideband only.
pub const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("demod", "Demodulator", Sideband::Usb)
        .help("How the sideband audio is made. USB is usual on HF.");
/// The `listen` setting.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false)
    .help("Plays the sideband audio. Decoding is the same either way.");

/// The `span` setting of a front end with a span.
pub const SPAN: Setting<field::Choice<Span>> = Setting::choice("span", "Span", Span::Khz14)
    .help("The width of the band that is watched, centred on the dial. A wider span asks the radio for a higher rate.")
    .sets_rate(span_rate);

sdrkit_core::settings! {
    /// The settings of [`HfFront`].
    pub enum FrontSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// Listen.
        Listen = LISTEN,
        /// The span.
        Span = SPAN,
    }
}

/// The front end component: `in` (Iq, any rate of at least 12 ksps) to `wide` (Iq, 24 kHz, dial at 0), `caudio` (Iq, 8 kHz, the
/// sideband 100 to 3900 Hz with the dial at 0), `audio` (Real, 8 kHz) and `listen` (Audio, 8 kHz, only while Listen is on). With
/// a span, `wide` is at the rate of the span and the other three outputs stay empty.
pub struct HfFront {
    demod: Demod,
    span: Option<Span>,
    allow_fm_am: bool,
    listen: bool,
    rate: f64,
    offset: f64,
    nco: Nco,
    rs: Option<Resampler<Complex32>>,
    mixed: Vec<Complex32>,
    wide: Vec<Complex32>,
    // USB/LSB path
    nco_down: Nco,
    dec: Decimator<Complex32>,
    nco_up: Nco,
    side: Vec<Complex32>,
    // NFM / AM path
    chan: Fir<Complex32>,
    prev: Complex32,
    dc: f32,
    rdec: Decimator<f32>,
    real24: Vec<f32>,
    real8: Vec<f32>,
    hil: Fir<f32>,
    hdelay: DelayLine<f32>,
    agc: Agc,
    // levels
    in_pow: f64,
    in_n: u64,
    au_pow: f64,
    au_n: u64,
    results: Published,
    since: f64,
}

impl Default for HfFront {
    fn default() -> Self {
        Self::new()
    }
}

impl HfFront {
    /// A front end with USB and Listen off.
    pub fn new() -> Self {
        let h = hilbert_design(AUDIO_RATE, 150.0, 50.0).expect("the filter settings are valid");
        let hl = h.len();
        HfFront {
            demod: Demod::Usb,
            span: None,
            allow_fm_am: true,
            listen: false,
            rate: 0.0,
            offset: 0.0,
            nco: Nco::new(1.0, 0.0),
            rs: None,
            mixed: Vec::new(),
            wide: Vec::new(),
            nco_down: Nco::new(WIDE_RATE, -BAND_CENTRE),
            dec: Decimator::new(WIDE_RATE, 3, 1950.0, 75.0),
            nco_up: Nco::new(AUDIO_RATE, BAND_CENTRE),
            side: Vec::new(),
            chan: Fir::new(
                &lowpass(WIDE_RATE, 6000.0, 8000.0, 60.0).expect("the filter settings are valid"),
            ),
            prev: Complex32::default(),
            dc: 0.0,
            rdec: Decimator::new(WIDE_RATE, 3, 3800.0, 60.0),
            real24: Vec::new(),
            real8: Vec::new(),
            hil: Fir::new(&h),
            hdelay: DelayLine::new(hl / 2),
            agc: Agc::new(AUDIO_RATE, 0.7, 0.05, 2.0, 7e6),
            in_pow: 0.0,
            in_n: 0,
            au_pow: 0.0,
            au_n: 0,
            results: Published::new(FrontFields::SHAPE.fields),
            since: 0.0,
        }
    }

    /// The wide stream only, at the rate of the span of the setting `span` (14 kHz to start).
    pub fn with_span(mut self) -> Self {
        self.span = Some(Span::Khz14);
        self
    }

    /// The rate of the wide stream.
    fn wide_rate(&self) -> f64 {
        self.span.map_or(WIDE_RATE, Span::wide_rate)
    }

    /// Only USB and LSB (ROS).
    pub fn sideband_only(mut self) -> Self {
        self.allow_fm_am = false;
        self
    }

    fn set_offset(&mut self, offset: f64) {
        self.offset = offset;
        self.nco = Nco::new(self.rate, -offset);
    }

    fn reset_audio(&mut self) {
        self.dec.reset();
        self.rdec.reset();
        self.chan.reset();
        self.hil.reset();
        self.hdelay.reset();
        self.prev = Complex32::default();
        self.dc = 0.0;
        self.nco_down = Nco::new(
            WIDE_RATE,
            if self.demod == Demod::Lsb {
                BAND_CENTRE
            } else {
                -BAND_CENTRE
            },
        );
        self.nco_up = Nco::new(AUDIO_RATE, BAND_CENTRE);
    }

    /// Makes the sideband from `self.wide` into `self.side` (complex 8 kHz, before the AGC).
    fn sideband(&mut self) {
        self.side.clear();
        match self.demod {
            Demod::Usb | Demod::Lsb => {
                self.mixed.clear();
                self.mixed.resize(self.wide.len(), Complex32::default());
                self.nco_down.mix(&self.wide, &mut self.mixed);
                self.dec.process(&self.mixed, &mut self.side);
                if self.demod == Demod::Lsb {
                    for s in &mut self.side {
                        *s = s.conj();
                    }
                }
                self.nco_up.mix_in_place(&mut self.side);
            }
            Demod::Nfm | Demod::Am => {
                self.real24.clear();
                let k = (WIDE_RATE / (std::f64::consts::TAU * 3000.0) * 2.0) as f32;
                for &x in &self.wide {
                    let y = self.chan.process_sample(x);
                    let v = if self.demod == Demod::Nfm {
                        let d = (y * self.prev.conj()).arg();
                        self.prev = y;
                        d * k * 0.5
                    } else {
                        let e = y.norm();
                        self.dc += 0.0005 * (e - self.dc);
                        e - self.dc
                    };
                    self.real24.push(v);
                }
                self.real8.clear();
                self.rdec.process(&self.real24, &mut self.real8);
                for &v in &self.real8 {
                    let i = self.hdelay.push(v);
                    let q = self.hil.process_sample(v);
                    self.side.push(Complex32::new(i, q));
                }
            }
        }
    }
}

impl Component for HfFront {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfkbd.front",
            vec![Port::iq("in")],
            vec![
                Port::iq("wide").rate_from(if self.span.is_some() {
                    RateFrom::Configure
                } else {
                    RateFrom::Fixed(WIDE_RATE)
                }),
                Port::iq("caudio").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::real("audio").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::audio("listen")
                    .rate_from(RateFrom::Fixed(AUDIO_RATE))
                    .channels_from(ChannelsFrom::Fixed(1)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < MIN_RATE - 1e-6 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.set_offset(f.signal_offset_hz());
        let wide_rate = self.wide_rate();
        self.rs = if (f.rate - wide_rate).abs() < 1e-6 {
            None
        } else {
            Some(Resampler::with_quality(f.rate, wide_rate, 0.42, 72.0)?)
        };
        self.reset_audio();
        let bandwidth = match self.span {
            Some(s) => s.width_hz().min(f.rate * 0.9),
            None => (WIDE_RATE * 0.84).min(f.rate * 0.9),
        };
        let wide = Format {
            bandwidth_hz: bandwidth,
            ..Format::iq(wide_rate, f.tuned_hz, f.tuned_hz)
        };
        let caudio = Format {
            bandwidth_hz: 3800.0,
            ..Format::iq(AUDIO_RATE, f.tuned_hz, f.tuned_hz)
        };
        let mut audio = Format::new(Kind::Real, AUDIO_RATE);
        audio.bandwidth_hz = 3800.0;
        audio.tuned_hz = f.tuned_hz;
        let mut listen = Format::new(Kind::Audio, AUDIO_RATE);
        listen.bandwidth_hz = 3800.0;
        Ok(vec![wide, caudio, audio, listen])
    }

    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                self.set_offset(tuned_hz - centre_hz);
                self.reset_audio();
            }
        }
        let x = io.iq(0);
        for s in x {
            self.in_pow += s.norm_sqr() as f64;
        }
        self.in_n += x.len() as u64;
        self.mixed.clear();
        self.mixed.resize(x.len(), Complex32::default());
        if self.offset != 0.0 {
            self.nco.mix(x, &mut self.mixed);
        } else {
            self.mixed.copy_from_slice(x);
        }
        self.wide.clear();
        match &mut self.rs {
            Some(r) => r.process(&self.mixed, &mut self.wide),
            None => self.wide.extend_from_slice(&self.mixed),
        }
        io.iq_out(0).extend_from_slice(&self.wide);
        if self.span.is_none() {
            self.sideband();
        } else {
            self.side.clear();
        }
        for s in &self.side {
            self.au_pow += s.norm_sqr() as f64;
        }
        // The slow AGC: one gain for the whole audio.
        self.agc.process(&mut self.side);
        self.au_n += self.side.len() as u64;
        io.iq_out(1).extend_from_slice(&self.side);
        let au = io.real_out(2);
        au.extend(self.side.iter().map(|s| s.re));
        if self.listen {
            let side = &self.side;
            io.audio_out(3)
                .samples
                .extend(side.iter().map(|s| 0.5 * s.re));
        }
        self.since += x.len() as f64 / self.rate.max(1.0);
        if self.since >= 0.1 || io.tick() {
            self.since = 0.0;
            let lin = if self.in_n > 0 {
                db(self.in_pow / self.in_n as f64)
            } else {
                -200.0
            };
            // A front end with a span makes no sideband audio: its level is absent.
            let lau = (self.au_n > 0).then(|| db(self.au_pow / self.au_n as f64));
            if self.in_n > 0 {
                self.results.put(&FrontFields {
                    level_dbfs: Some((lin * 10.0).round() / 10.0),
                    audio_dbfs: lau.map(|l| (l * 10.0).round() / 10.0),
                });
            }
            self.in_pow = 0.0;
            self.in_n = 0;
            self.au_pow = 0.0;
            self.au_n = 0;
        }
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Demod => {
                let d = DEMOD.read(id, value)?;
                if !self.allow_fm_am && !matches!(d, Demod::Usb | Demod::Lsb) {
                    return Err(SettingError::NotPermitted { id });
                }
                if d != self.demod {
                    self.demod = d;
                    self.reset_audio();
                }
            }
            FrontSetting::Listen => self.listen = LISTEN.read(id, value)?,
            FrontSetting::Span => {
                let s = SPAN.read(id, value)?;
                if self.span.is_none() {
                    return Err(SettingError::NotPermitted { id });
                }
                if self.span != Some(s) {
                    self.span = Some(s);
                    return Ok(Applied::Reformat);
                }
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Demod, Value::Choice(self.demod.index())),
            (FrontSetting::Listen, Value::Bool(self.listen)),
            (
                FrontSetting::Span,
                Value::Choice(self.span.unwrap_or(Span::Khz14).index()),
            ),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

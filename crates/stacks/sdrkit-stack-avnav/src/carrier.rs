//! The AM carrier stage that NDB, VOR and ILS share: the envelope divided by the carrier level (so the output is the modulation
//! itself), and four times a second the carrier's offset, level and carrier to noise density from a windowed spectrum. The beat
//! oscillator makes a keyed carrier audible as a tone.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::spectrum::CarrierSpectrum;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};

/// The carrier as last measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CarrierInfo {
    /// Offset of the carrier from the centre of the channel, Hz.
    pub offset_hz: f64,
    /// Carrier level, dBFS.
    pub level_dbfs: f32,
    /// Carrier to noise density, dB-Hz.
    pub cn0_dbhz: f32,
    /// A carrier stands clear of the noise (C/N0 at least the threshold; with synchronous detection it also stands out in
    /// the detector's carrier average).
    pub present: bool,
    /// The carrier is enough for an identification: with synchronous detection its power in the detector's 3 Hz average is
    /// more than 5 times the noise there (a C/N0 of about 17 dB-Hz); without, it is `present`.
    pub stands_out: bool,
}

impl Item for CarrierInfo {
    const KIND: &'static str = "carrier.info";
}

const MAX_OFFSET: Setting<field::Hz> = Setting::new("max_offset", "Carrier search", "Hz", 1000.0)
    .range(10.0, 1e5, 0.0)
    .help("How far from the tuned frequency the carrier is looked for.");
const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Carrier threshold", "dB-Hz", 30.0)
        .range(0.0, 90.0, 0.0)
        .help("The least carrier to noise density that counts as a station.");
const TAU: Setting<field::Duration> =
    Setting::new("tau", "Carrier average", "s", 0.5).range(0.05, 5.0, 0.0);
const SYNC: Setting<field::Bool> = Setting::new("sync", "Synchronous detection", "", false)
    .help("The modulation is the in-phase part against a carrier that a frequency loop follows, not the envelope.");

sdrkit_core::settings! {
    /// The settings of [`AmCarrier`].
    pub enum AmCarrierSetting {
        /// How far from the centre the carrier is looked for.
        MaxOffset = MAX_OFFSET,
        /// The least C/N0 that counts as a carrier.
        Threshold = THRESHOLD,
        /// The time constant of the carrier level.
        Tau = TAU,
        /// Synchronous detection of the modulation.
        Sync = SYNC,
    }
}

/// The synchronous detector of [`AmCarrier`]: the channel turned by a frequency estimate `fo`; a 30 Hz one-pole average of the
/// turned stream whose phase steps (clipped to +-5 Hz) move `fo` with weight 0.002, within +-200 Hz; the carrier as a 3 Hz
/// one-pole average, its magnitude averaged with 0.001. The modulation is the in-phase part `Re(z conj(c)) / max(|c|, m)` over
/// the slow magnitude `m`, less 1, so a tone's amplitude is its modulation depth. A carrier that the spectrum finds more than
/// 10 Hz from `fo` sets `fo` ([`SyncDetector::seed`]), so a carrier far off is followed at once; until the carrier average
/// holds the carrier, the envelope stands in.
#[derive(Clone, Copy, Debug, Default)]
struct SyncDetector {
    rate: f64,
    a30: f32,
    a3: f32,
    fo: f64,
    phase: f64,
    v: Complex32,
    carrier: Complex32,
    slow: f32,
    /// The magnitude of the 30 Hz average, slowly averaged.
    wide: f32,
    /// The power of the 3 Hz carrier average and of the stream, each averaged with [`SYNC_POWER_WEIGHT`].
    carrier_power: f32,
    total_power: f32,
    /// The share of the stream's noise power that the 3 Hz average passes.
    noise_share: f32,
}

/// Largest step of the frequency loop per sample, Hz, and its weight.
const SYNC_STEP_HZ: f64 = 5.0;
const SYNC_WEIGHT: f64 = 0.002;
/// The frequency loop stays within this many Hz.
const SYNC_RANGE_HZ: f64 = 200.0;
/// A measured carrier farther than this from the loop's frequency sets it, Hz.
const SYNC_SEED_HZ: f64 = 10.0;
/// The share of the 30 Hz average the 3 Hz carrier average must hold for the synchronous output.
const SYNC_HOLD: f32 = 0.7;
/// The weight of a sample in the averaged powers of the carrier test.
const SYNC_POWER_WEIGHT: f32 = 0.0005;
/// A carrier stands out when the power of the 3 Hz average is more than this many times the noise that the average passes.
const SYNC_STANDS_OUT: f32 = 5.0;

impl SyncDetector {
    /// A detector for a stream at `rate` whose noise fills `width_hz`. A one-pole average of weight `a` passes `a / (2 - a)` of
    /// the power of white noise; noise that fills only `width_hz` of the rate is denser by `rate / width_hz`.
    fn new(rate: f64, width_hz: f64) -> Self {
        let pole = |hz: f64| (1.0 - (-std::f64::consts::TAU * hz / rate).exp()) as f32;
        let a3 = pole(3.0);
        let fill = if width_hz > 0.0 && width_hz < rate {
            rate / width_hz
        } else {
            1.0
        };
        SyncDetector {
            rate,
            a30: pole(30.0),
            a3,
            noise_share: a3 / (2.0 - a3) * fill as f32,
            ..SyncDetector::default()
        }
    }

    /// Whether the carrier stands out of the noise in the 3 Hz average: its power is more than 5 times the share of the
    /// stream's power that noise alone would leave there.
    fn stands_out(&self) -> bool {
        self.carrier_power > SYNC_STANDS_OUT * self.noise_share * self.total_power
    }

    /// Takes the carrier offset the spectrum measured.
    fn seed(&mut self, offset_hz: f64) {
        if (offset_hz - self.fo).abs() > SYNC_SEED_HZ {
            self.fo = offset_hz.clamp(-SYNC_RANGE_HZ, SYNC_RANGE_HZ);
        }
    }

    /// The modulation of one sample, or `None` while the 3 Hz carrier average holds less than 0.7 of the magnitude of the 30 Hz
    /// average (both slowly averaged): the loop is not on the carrier yet, so the 3 Hz average turns.
    fn push(&mut self, z: Complex32) -> Option<f32> {
        let turned = z * Complex32::from_polar(1.0, -self.phase as f32);
        self.phase =
            (self.phase + std::f64::consts::TAU * self.fo / self.rate) % std::f64::consts::TAU;
        let prev = self.v;
        self.v += (turned - self.v) * self.a30;
        let step = (self.v * prev.conj()).arg() as f64 * self.rate / std::f64::consts::TAU;
        if step.is_finite() {
            self.fo = (self.fo + SYNC_WEIGHT * step.clamp(-SYNC_STEP_HZ, SYNC_STEP_HZ))
                .clamp(-SYNC_RANGE_HZ, SYNC_RANGE_HZ);
        }
        self.carrier += (turned - self.carrier) * self.a3;
        self.carrier_power += SYNC_POWER_WEIGHT * (self.carrier.norm_sqr() - self.carrier_power);
        self.total_power += SYNC_POWER_WEIGHT * (z.norm_sqr() - self.total_power);
        let c = self.carrier.norm();
        self.slow += 0.001 * (c - self.slow);
        self.wide += 0.001 * (self.v.norm() - self.wide);
        let m = c.max(self.slow);
        if m <= 1e-12 || self.slow <= 1e-12 || self.slow < SYNC_HOLD * self.wide {
            return None;
        }
        Some(((turned * self.carrier.conj()).re / m / self.slow - 1.0).clamp(-2.0, 2.0))
    }
}

/// The AM carrier stage (`avnav.am_carrier`, iq -> real `am` + [`CarrierInfo`] `info`). The envelope is averaged over 1/30 s
/// (which nulls 30 Hz and its harmonics, so VOR and ILS tones do not move the level), then by a one-pole average. The output is
/// the envelope divided by that level, less 1.
pub struct AmCarrier {
    max_offset: f64,
    threshold: f64,
    tau: f64,
    sync: bool,
    detector: SyncDetector,
    rate: f64,
    alpha: f32,
    level: f32,
    seen: f32,
    boxcar: Vec<f32>,
    bpos: usize,
    bsum: f64,
    ring: Vec<Complex32>,
    pos: usize,
    filled: usize,
    nfft: usize,
    fft: Option<Fft>,
    work: Vec<Complex32>,
    powers: Vec<f32>,
    spectrum: CarrierSpectrum,
    every: u64,
    count: u64,
    info: CarrierInfo,
}

impl Default for AmCarrier {
    fn default() -> Self {
        AmCarrier::make(MAX_OFFSET.default)
    }
}

impl AmCarrier {
    /// Looking for the carrier within `max_offset_hz` of the centre.
    ///
    /// # Errors
    ///
    /// The [`SettingError`] of the `max_offset` setting for an offset outside its range.
    pub fn new(max_offset_hz: f64) -> Result<Self, SettingError> {
        AmCarrierSetting::MaxOffset.check(&Value::Hz(max_offset_hz))?;
        Ok(AmCarrier::make(max_offset_hz))
    }

    fn make(max_offset_hz: f64) -> Self {
        AmCarrier {
            max_offset: max_offset_hz,
            threshold: THRESHOLD.default,
            tau: TAU.default,
            sync: SYNC.default,
            detector: SyncDetector::default(),
            rate: 0.0,
            alpha: 0.0,
            level: 0.0,
            seen: 0.0,
            boxcar: vec![0.0],
            bpos: 0,
            bsum: 0.0,
            ring: Vec::new(),
            pos: 0,
            filled: 0,
            nfft: 0,
            fft: None,
            work: Vec::new(),
            powers: Vec::new(),
            spectrum: CarrierSpectrum::default(),
            every: 1,
            count: 0,
            info: CarrierInfo::default(),
        }
    }

    /// The last measurement.
    pub fn info(&self) -> CarrierInfo {
        self.info
    }

    fn measure(&mut self) {
        let n = self.nfft;
        let Some(fft) = &mut self.fft else { return };
        let w = self.spectrum.window();
        for i in 0..n {
            self.work[i] = self.ring[(self.pos + i) % n] * w[i];
        }
        fft.process(&mut self.work);
        for (p, w) in self.powers.iter_mut().zip(&self.work) {
            *p = w.norm_sqr();
        }
        let n0 = self
            .spectrum
            .noise_density(&self.powers, 1)
            .unwrap_or(f64::INFINITY);
        let bin_hz = self.rate / n as f64;
        let reach = ((self.max_offset / bin_hz).ceil() as isize).min(n as isize / 2 - 2);
        let at = |k: isize| self.powers[k.rem_euclid(n as isize) as usize] as f64;
        let k = (-reach..=reach)
            .max_by(|&a, &b| at(a).total_cmp(&at(b)))
            .unwrap_or(0);
        let (l, pk, r) = (at(k - 1), at(k), at(k + 1));
        let frac = log_parabolic(l, pk, r).offset;
        let p_c = self.spectrum.carrier_power(l, pk, r);
        let cn0 = 10.0 * (p_c / n0.max(1e-30)).log10();
        self.info = CarrierInfo {
            offset_hz: (k as f64 + frac) * bin_hz,
            level_dbfs: (10.0 * p_c.max(1e-20).log10()) as f32,
            cn0_dbhz: cn0 as f32,
            // With synchronous detection the carrier must also stand out in the detector's 3 Hz average.
            present: cn0 >= self.threshold && (!self.sync || self.detector.stands_out()),
            stands_out: if self.sync {
                self.detector.stands_out()
            } else {
                cn0 >= self.threshold
            },
        };
    }
}

impl Component for AmCarrier {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.am_carrier",
            vec![Port::iq("in")],
            vec![
                Port::real("am").same_format_as(0),
                Port::typed::<CarrierInfo>("info").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(AmCarrierSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.rate = f.rate;
        self.alpha = (1.0 - (-1.0 / (self.tau * f.rate)).exp()) as f32;
        self.detector = SyncDetector::new(f.rate, f.bandwidth_hz);
        self.seen = 0.0;
        self.level = 0.0;
        self.boxcar = vec![0.0; ((f.rate / 30.0).round() as usize).max(1)];
        self.bpos = 0;
        self.bsum = 0.0;
        // About 0.5 s of signal, a power of two.
        let mut n = 256usize;
        while (n as f64) < 0.5 * f.rate && n < 1 << 16 {
            n *= 2;
        }
        self.nfft = n;
        self.ring = vec![Complex32::default(); n];
        self.pos = 0;
        self.filled = 0;
        self.fft = Some(Fft::forward(n));
        self.work = vec![Complex32::default(); n];
        self.powers = vec![0.0; n];
        self.spectrum = CarrierSpectrum::new(n, f.rate, f.bandwidth_hz.min(f.rate))?;
        self.every = (0.25 * f.rate).round().max(1.0) as u64;
        self.count = 0;
        self.info = CarrierInfo::default();
        Ok(vec![
            Format {
                bandwidth_hz: f.bandwidth_hz / 2.0,
                ..f.with_kind(Kind::Real)
            },
            Format {
                rate: 0.0,
                ..f.with_kind(Kind::typed::<CarrierInfo>())
            },
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.iq(0);
        {
            let am = io.real_out(0);
            am.reserve(x.len());
            let mut level = self.level;
            let a = self.alpha;
            // The average starts as a running mean, so a quiet start (a filter filling up) does not leave a wrong level.
            let mut seen = self.seen;
            let len = self.boxcar.len();
            for s in x {
                let m = s.norm();
                // A mean over 1/30 s first: it nulls 30 Hz and its harmonics (VOR, ILS), so the level does not follow them.
                self.bsum += (m - self.boxcar[self.bpos]) as f64;
                self.boxcar[self.bpos] = m;
                self.bpos += 1;
                if self.bpos == len {
                    self.bpos = 0;
                    self.bsum = self.boxcar.iter().map(|&v| v as f64).sum();
                }
                seen += 1.0;
                let mean = (self.bsum / (seen as f64).min(len as f64)) as f32;
                level += a.max(1.0 / seen) * (mean - level);
                let sync = if self.sync {
                    self.detector.push(*s)
                } else {
                    None
                };
                // The first 1/30 s only fills the average. The envelope stands in until the loop holds the carrier.
                am.push(if level > 1e-12 && seen > len as f32 {
                    sync.unwrap_or((m / level - 1.0).min(2.0))
                } else {
                    0.0
                });
            }
            self.seen = seen;
            self.level = level;
        }
        let n = self.nfft;
        for (i, &s) in x.iter().enumerate() {
            self.ring[self.pos] = s;
            self.pos += 1;
            if self.pos == n {
                self.pos = 0;
            }
            self.filled = (self.filled + 1).min(n);
            self.count += 1;
            if self.count >= self.every {
                self.count = 0;
                if self.filled == n {
                    self.measure();
                    if self.sync && self.info.present {
                        self.detector.seed(self.info.offset_hz);
                    }
                    let time = (first + i as u64 + 1) as f64 / self.rate;
                    let info = self.info;
                    io.items_out::<CarrierInfo>(1)
                        .push(Timed { time, value: info });
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AmCarrierSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AmCarrierSetting::MaxOffset => self.max_offset = MAX_OFFSET.read(id, v)?,
            AmCarrierSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            AmCarrierSetting::Tau => {
                self.tau = TAU.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            AmCarrierSetting::Sync => self.sync = SYNC.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (AmCarrierSetting::MaxOffset, Value::Hz(self.max_offset)),
            (AmCarrierSetting::Threshold, Value::F64(self.threshold)),
            (AmCarrierSetting::Tau, Value::Duration(self.tau)),
            (AmCarrierSetting::Sync, Value::Bool(self.sync)),
        ])
    }
}

sdrkit_core::choice_enum! {
    /// What [`Bfo`] makes audible.
    pub enum BfoAudio {
        /// The envelope.
        Am = ("am", "AM"),
        /// The carrier as a beat note.
        Cw = ("cw", "Beat note"),
    }
}

const AUDIO: Setting<field::Choice<BfoAudio>> = Setting::choice("mode", "Audio", BfoAudio::Cw);
const PITCH: Setting<field::Hz> =
    Setting::new("pitch", "Beat pitch", "Hz", 800.0).range(100.0, 3000.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`Bfo`].
    pub enum BfoSetting {
        /// The envelope or the beat note.
        Mode = AUDIO,
        /// The pitch of the beat note.
        Pitch = PITCH,
    }
}

/// A beat oscillator (`avnav.bfo`): iq + [`CarrierInfo`] -> real. The carrier, wherever it was measured, is heard as a tone of
/// `pitch` Hz. [`BfoAudio::Am`] passes the envelope instead (relative to the carrier, as [`AmCarrier`] makes it).
pub struct Bfo {
    pitch: f64,
    mode: BfoAudio,
    rate: f64,
    offset: f64,
    nco: Nco,
    level: f32,
    alpha: f32,
}

impl Default for Bfo {
    fn default() -> Self {
        Bfo::new(AUDIO.default)
    }
}

impl Bfo {
    /// Making `mode` audible.
    pub fn new(mode: BfoAudio) -> Self {
        Bfo {
            pitch: PITCH.default,
            mode,
            rate: 0.0,
            offset: 0.0,
            nco: Nco::new(1.0, 0.0),
            level: 0.0,
            alpha: 0.0,
        }
    }
}

impl Component for Bfo {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.bfo",
            vec![Port::iq("in"), Port::typed::<CarrierInfo>("info")],
            vec![Port::real("out").same_format_as(0)],
        )
        .with_settings(BfoSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.nco = Nco::new(f.rate, self.pitch - self.offset);
        self.alpha = (1.0 - (-1.0 / (0.05 * f.rate)).exp()) as f32;
        Ok(vec![Format {
            bandwidth_hz: f.bandwidth_hz / 2.0,
            ..f.with_kind(Kind::Real)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(i) = io.items::<CarrierInfo>(1).last() {
            if i.value.present {
                self.offset = i.value.offset_hz;
            }
        }
        let x = io.iq(0);
        let out = io.real_out(0);
        out.reserve(x.len());
        let am = self.mode == BfoAudio::Am;
        // The carrier, at the measured offset, is heard at the pitch.
        self.nco.set_freq(self.pitch - self.offset);
        for s in x {
            let m = s.norm();
            self.level += self.alpha * (m - self.level);
            let g = if self.level > 1e-9 {
                0.5 / self.level
            } else {
                0.0
            };
            if am {
                out.push((m * g * 2.0 - 1.0) * 0.5);
            } else {
                out.push((s * self.nco.next_sample()).re * g);
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BfoSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BfoSetting::Mode => self.mode = AUDIO.read(id, v)?,
            BfoSetting::Pitch => self.pitch = PITCH.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BfoSetting::Mode, Value::Choice(self.mode.index())),
            (BfoSetting::Pitch, Value::Hz(self.pitch)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_offset_and_density() {
        let rate = 8000.0;
        let mut c = AmCarrier::new(1000.0).unwrap();
        let f = Format::new(Kind::Iq, rate);
        // The whole band carries noise.
        c.configure(&[f]).unwrap();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(1);
        // Carrier 0.1 at +123 Hz, noise of power 1e-4 over 8 kHz.
        for i in 0..8000 {
            let p = std::f64::consts::TAU * 123.0 * i as f64 / rate;
            let s = Complex32::new((0.1 * p.cos()) as f32, (0.1 * p.sin()) as f32)
                + rng.complex_normal(1e-4);
            c.ring[c.pos] = s;
            c.pos = (c.pos + 1) % c.nfft;
            c.filled = (c.filled + 1).min(c.nfft);
        }
        c.measure();
        let want = 10.0 * (0.01f64 / (1e-4 / rate)).log10();
        assert!((c.info.offset_hz - 123.0).abs() < 1.0, "{:?}", c.info);
        assert!(
            (c.info.cn0_dbhz as f64 - want).abs() < 1.5,
            "{:?} want {want}",
            c.info
        );
        assert!((c.info.level_dbfs + 20.0).abs() < 1.0);
    }
}

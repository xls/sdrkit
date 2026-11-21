//! The QO-100 receiver: the beacon lock on the narrowband transponder, its averaged spectrum and signals, and the wideband
//! spectrum with its DATV carriers.

use sdrkit_algo_dsp::meter::power_to_db as db;
use super::{
    converter_of, LoChoice, BEACON_HIGH, BEACON_LOW, BEACON_PSK, DATV_BEACON, MIN_RATE, NB_HI,
    NB_LO, NB_RATE,
};
use crate::display::slope_of;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::converter::Converter;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::spectrum::{percentile, Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Schema, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Item, Kind, Outbox, Port, SlicePool, Timed};
use std::collections::VecDeque;
use std::sync::Arc;

/// The local oscillator of the LNB.
pub const LO: Setting<field::Choice<LoChoice>> =
    Setting::choice("lo", "LNB oscillator", LoChoice::L9750).restart();
/// The custom local oscillator.
pub const LO_CUSTOM_MHZ: Setting<field::F64> =
    Setting::new("lo_custom_mhz", "Custom LO", "MHz", 9750.0)
        .range(1.0, 20000.0, 0.001)
        .restart();
/// Apply the LNB error measured on the beacons.
pub const AUTO_CORRECTION: Setting<field::Bool> =
    Setting::new("auto_correction", "Automatic correction", "", true)
        .help("Apply the LNB error measured on the beacons.");
/// The calibration of the LNB.
pub const LNB_PPM: Setting<field::F64> = Setting::new("lnb_ppm", "LNB calibration", "ppm", 0.0)
    .range(-100.0, 100.0, 0.01)
    .restart();

sdrkit_core::settings! {
    /// The settings of [`QoReceiver`].
    pub enum QoReceiverSetting {
        /// LNB oscillator.
        Lo = LO,
        /// Custom LO.
        LoCustomMhz = LO_CUSTOM_MHZ,
        /// Automatic correction.
        AutoCorrection = AUTO_CORRECTION,
        /// LNB calibration.
        LnbPpm = LNB_PPM,
    }
}

/// Points of a fine estimate of the beacons.
pub const FINE_N: usize = 4096;
/// Points of a frame of the narrowband spectrum.
pub const NARROW_N: usize = 8192;
/// Frames averaged into one narrowband spectrum.
pub const NARROW_FRAMES: usize = 8;
/// Points of a frame of the wideband spectrum.
pub const WIDE_N: usize = 2048;
/// A CW beacon counts when its peak stands this far over the median bin, dB.
pub const BEACON_SNR_DB: f64 = 15.0;
/// Fine estimates in a row without a beacon before the lock is held.
pub const MISSES_TO_HOLD: u32 = 6;
/// The drift is measured over this many seconds of LNB errors.
pub const DRIFT_WINDOW_S: f64 = 30.0;

sdrkit_core::choice_enum! {
    /// The phase of the beacon lock.
    #[derive(Default)]
    pub enum LockPhase {
        /// The narrowband transponder is outside the radio's band, or nothing was measured yet.
        #[default]
        Outside = ("outside", "Outside the band"),
        /// Searching for the beacons.
        Searching = ("searching", "Searching"),
        /// Locked to the CW beacons.
        Locked = ("locked", "Locked"),
        /// The beacons were lost, and the last correction is held.
        Held = ("held", "Held"),
    }
}

sdrkit_core::choice_enum! {
    /// The class of a tracked signal, from its width and how steady it is.
    #[derive(Default)]
    pub enum SignalClass {
        /// Not classed.
        #[default]
        Unknown = ("unknown", "Unknown"),
        /// One of the beacons.
        Beacon = ("beacon", "Beacon"),
        /// A steady carrier.
        Carrier = ("carrier", "Carrier"),
        /// Morse.
        Cw = ("cw", "CW"),
        /// A narrow digital signal.
        Psk = ("psk", "PSK"),
        /// Single sideband speech.
        Ssb = ("ssb", "SSB"),
        /// Slow-scan television.
        Sstv = ("sstv", "SSTV"),
        /// A wide digital signal.
        WideDigital = ("wide_digital", "Wide digital"),
    }
}

/// One of the three narrowband beacons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Beacon {
    /// "lower CW", "PSK" or "upper CW".
    pub name: &'static str,
    /// Its nominal RF, Hz.
    pub nominal_hz: f64,
    /// Whether it stands over the floor by its limit.
    pub found: bool,
    /// The RF where it shows with the nominal LO and no correction, Hz.
    pub apparent_hz: f64,
    /// Its peak over the median bin, dB.
    pub snr_db: f64,
}

/// The beacons by name and nominal RF.
pub const BEACONS: [(&str, f64); 3] = [
    ("lower CW", BEACON_LOW),
    ("PSK", BEACON_PSK),
    ("upper CW", BEACON_HIGH),
];

/// The beacon lock's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BeaconLock {
    pub phase: LockPhase,
    /// The LNB's LO error (actual minus nominal), Hz.
    pub error_hz: f64,
    /// The same in parts per million of the nominal LO.
    pub error_ppm: f64,
    /// The drift of the error over the last 30 s, Hz/s.
    pub drift_hz_s: f64,
    /// Seconds since the lock was taken.
    pub since_lock_s: f64,
    /// Fine estimates that found a CW beacon.
    pub measurements: u64,
    pub beacons: [Beacon; 3],
    /// Whether the error is applied to the frequencies shown.
    pub applied: bool,
}

/// A signal on the narrowband transponder.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Signal {
    /// The corrected RF of its top (CW: the carrier), Hz.
    pub rf_hz: f64,
    /// Its width at 6 dB under the top, Hz.
    pub width_hz: f64,
    /// Its peak level, dBFS per bin.
    pub level_dbfs: f64,
    /// Its peak over the floor, dB.
    pub snr_db: f64,
    /// The beacon it is, or "".
    pub beacon: &'static str,
    /// The tracker's class (the display sets it).
    pub class: SignalClass,
    /// The tracker's id (the display sets it).
    pub track: u64,
}

/// A DATV carrier on the wideband transponder.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Datv {
    /// Its corrected centre RF, Hz.
    pub rf_hz: f64,
    /// Its centre in the input, Hz (what a DVB-S receiver tunes to).
    pub if_hz: f64,
    /// Its width at 3 dB over the floor, Hz.
    pub width_hz: f64,
    /// The symbol rate derived from its width (roll-off 0.35), Hz.
    pub symbol_rate_hz: f64,
    /// Its mean level over the floor, dB.
    pub snr_db: f64,
    /// Whether its top is flat, as DVB-S and DVB-S2 carriers look.
    pub flat_topped: bool,
    /// Whether it is the DATV beacon.
    pub beacon: bool,
}

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum QoItem {
    /// The beacon lock, at each report.
    Lock(BeaconLock),
    /// An averaged narrowband spectrum: bins lowest RF first, from `f0_hz` in steps of `bin_hz`.
    Narrow {
        bins_dbfs: Arc<[f32]>,
        f0_hz: f64,
        bin_hz: f64,
        floor_dbfs: f64,
        signals: Arc<[Signal]>,
    },
    /// An averaged wideband spectrum, lowest RF first.
    Wide {
        bins_dbfs: Arc<[f32]>,
        f0_hz: f64,
        bin_hz: f64,
        datv: Arc<[Datv]>,
        outside: bool,
    },
}

impl Item for QoItem {
    const KIND: &'static str = "satcom.qo100_item";
}

/// The receiver: both transponders from the one input.
pub struct QoReceiver {
    rate: f64,
    centre_if: f64,
    lo_choice: LoChoice,
    lo_custom_mhz: f64,
    conv: Converter,
    auto: bool,
    lnb_ppm: f64,
    t: f64,
    narrow: Option<Narrow>,
    wide: Option<Wide>,
    lock: BeaconLock,
    misses: u32,
    lock_since: f64,
    recent: VecDeque<(f64, f64)>,
    floats: SlicePool<f32>,
    signals: SlicePool<Signal>,
    datvs: SlicePool<Datv>,
    found: Vec<Signal>,
    found_datv: Vec<Datv>,
    scratch: Vec<f32>,
}

/// The narrowband channel and its spectra.
struct Narrow {
    chan: Channelizer,
    mixed: Vec<Complex32>,
    fine: Vec<Complex32>,
    fine_welch: Welch,
    fine_pow: Vec<f32>,
    frame: Vec<Complex32>,
    welch: Welch,
    pow: Vec<f32>,
    acc: Vec<f32>,
    frames: usize,
}

/// The wideband spectrum.
struct Wide {
    welch: Welch,
    frame: Vec<Complex32>,
    pow: Vec<f32>,
    acc: Vec<f32>,
    frames: usize,
    skip: u64,
}

impl Default for QoReceiver {
    fn default() -> Self {
        QoReceiver {
            rate: 0.0,
            centre_if: 0.0,
            lo_choice: LoChoice::L9750,
            lo_custom_mhz: 9750.0,
            conv: converter_of(LoChoice::L9750, 9750.0),
            auto: true,
            lnb_ppm: 0.0,
            t: 0.0,
            narrow: None,
            wide: None,
            lock: BeaconLock::default(),
            misses: 0,
            lock_since: 0.0,
            recent: VecDeque::new(),
            floats: SlicePool::default(),
            signals: SlicePool::default(),
            datvs: SlicePool::default(),
            found: Vec::new(),
            found_datv: Vec::new(),
            scratch: Vec::new(),
        }
    }
}

impl QoReceiver {
    /// The LO with the calibration and the LO error `err`.
    fn lo_with(&self, err: f64) -> f64 {
        self.conv.lo_hz * (1.0 + self.lnb_ppm * 1e-6) + err
    }
    /// The offset in the input of an RF, with the LO error `err`.
    fn offset_of(&self, rf: f64, err: f64) -> f64 {
        self.conv.with_lo(self.lo_with(err)).if_of(rf) - self.centre_if
    }
    /// The error applied to the frequencies shown.
    fn correction(&self) -> f64 {
        if self.auto && self.lock.phase == LockPhase::Locked {
            self.lock.error_hz
        } else {
            0.0
        }
    }

    /// Builds the channels for the input rate, the centre and the LO. Drops the lock.
    fn build(&mut self) -> Result<(), ConfigError> {
        self.conv = converter_of(self.lo_choice, self.lo_custom_mhz);
        self.lock = BeaconLock::default();
        self.misses = 0;
        self.recent.clear();
        let mid = self.offset_of(BEACON_PSK, 0.0);
        let outside = mid.abs() + 300e3 > 0.5 * self.rate;
        let welch = |n| Welch::new(n, Window::Hann, 0.0, Scaling::Tone);
        self.narrow = if outside {
            None
        } else {
            let mut cfg = ChannelConfig::new(self.rate, mid, 0.875 * NB_RATE, NB_RATE);
            cfg.exact_rate = true;
            Some(Narrow {
                chan: Channelizer::new(cfg)?,
                mixed: Vec::new(),
                fine: Vec::with_capacity(FINE_N),
                fine_welch: welch(FINE_N)?,
                fine_pow: vec![0.0; FINE_N],
                frame: Vec::with_capacity(NARROW_N),
                welch: welch(NARROW_N)?,
                pow: vec![0.0; NARROW_N],
                acc: vec![0.0; NARROW_N],
                frames: 0,
            })
        };
        self.wide = Some(Wide {
            welch: welch(WIDE_N)?,
            frame: Vec::with_capacity(WIDE_N),
            pow: vec![0.0; WIDE_N],
            acc: vec![0.0; WIDE_N],
            frames: 0,
            // About 32 frames in each report of 0.25 s.
            skip: ((self.rate * 0.25 / WIDE_N as f64) / 32.0).floor().max(1.0) as u64,
        });
        self.lock.phase = if outside {
            LockPhase::Outside
        } else {
            LockPhase::Searching
        };
        Ok(())
    }

    /// One fine estimate: the beacons' peaks within 60 kHz of their nominal place.
    fn estimate(&mut self) {
        let Some(nb) = &mut self.narrow else { return };
        let mut pw = std::mem::take(&mut nb.fine_pow);
        let ok = nb.fine_welch.compute(&nb.fine, &mut pw).is_ok();
        if ok {
            self.measure(&pw);
        }
        if let Some(nb) = &mut self.narrow {
            nb.fine_pow = pw;
        }
    }

    /// Reads the beacons from the power spectrum `pw` of a fine estimate, and updates the lock.
    fn measure(&mut self, pw: &[f32]) {
        let n = FINE_N;
        let bin = NB_RATE / n as f64;
        let med = percentile(pw, 0.5, &mut self.scratch).max(1e-30) as f64;
        let mid = self.offset_of(BEACON_PSK, 0.0);
        let (mut err_sum, mut err_n) = (0.0, 0usize);
        let mut beacons = [Beacon::default(); 3];
        for (b, &(name, nominal)) in beacons.iter_mut().zip(BEACONS.iter()) {
            *b = Beacon {
                name,
                nominal_hz: nominal,
                ..Default::default()
            };
            let expect = self.offset_of(nominal, 0.0) - mid;
            let k0 = ((expect - 60e3) / bin + (n / 2) as f64).max(1.0) as usize;
            let k1 = (((expect + 60e3) / bin + (n / 2) as f64) as usize).min(n - 2);
            if k1 <= k0 {
                continue;
            }
            let k = (k0..=k1)
                .max_by(|&a, &c| pw[a].total_cmp(&pw[c]))
                .unwrap_or(k0);
            let snr = db(pw[k] as f64 / med);
            let pk = log_parabolic(pw[k - 1] as f64, pw[k] as f64, pw[k + 1] as f64);
            let off = (k as f64 + pk.offset - (n / 2) as f64) * bin + mid;
            b.apparent_hz = self
                .conv
                .with_lo(self.lo_with(0.0))
                .rf_of(off + self.centre_if);
            b.snr_db = snr;
            if name == "PSK" {
                b.found = snr >= 10.0;
            } else if snr >= BEACON_SNR_DB {
                b.found = true;
                err_sum += nominal - b.apparent_hz;
                err_n += 1;
            }
        }
        self.lock.beacons = beacons;
        if err_n == 0 {
            self.misses += 1;
            if self.misses >= MISSES_TO_HOLD && self.lock.phase == LockPhase::Locked {
                self.lock.phase = LockPhase::Held;
            }
            if matches!(self.lock.phase, LockPhase::Outside) {
                self.lock.phase = LockPhase::Searching;
            }
            return;
        }
        self.misses = 0;
        // A LO that is high shows every RF low by as much, below the downlink and above it (mirrored).
        let lo_err = err_sum / err_n as f64;
        self.lock.error_hz = lo_err;
        self.lock.error_ppm = lo_err / self.conv.lo_hz * 1e6;
        self.lock.measurements += 1;
        if self.lock.phase != LockPhase::Locked {
            self.lock_since = self.t;
        }
        self.lock.phase = LockPhase::Locked;
        self.lock.since_lock_s = self.t - self.lock_since;
        self.recent.push_back((self.t, lo_err));
        while self
            .recent
            .front()
            .is_some_and(|p| self.t - p.0 > DRIFT_WINDOW_S)
        {
            self.recent.pop_front();
        }
        self.lock.drift_hz_s = if self.recent.len() >= 3 {
            slope_of(self.recent.make_contiguous())
        } else {
            0.0
        };
    }

    /// The averaged narrowband spectrum (in corrected RF) and its signals.
    fn narrow_spectrum(&mut self, out: &mut Vec<Timed<QoItem>>) {
        let corr = self.correction();
        let mid = self.offset_of(BEACON_PSK, 0.0);
        let (conv, centre_if) = (self.conv.with_lo(self.lo_with(corr)), self.centre_if);
        let inv = conv.inverted();
        let Some(nb) = &mut self.narrow else { return };
        let n = NARROW_N;
        let bin = NB_RATE / n as f64;
        let frames = nb.frames.max(1) as f32;
        for a in nb.acc.iter_mut() {
            *a /= frames;
        }
        let p = &nb.acc;
        let floor = percentile(p, 0.5, &mut self.scratch).max(1e-30);
        // Bins lowest RF first.
        let at = |i: usize| if inv { n - 1 - i } else { i };
        let rf_of_bin = |k: f64| conv.rf_of((k - (n / 2) as f64) * bin + mid + centre_if);
        let bins = self.floats.exact(n, |s| {
            for (i, v) in s.iter_mut().enumerate() {
                *v = db(p[at(i)] as f64) as f32;
            }
        });
        let f0 = rf_of_bin(at(0) as f64);
        // Signals: regions 6 dB over the floor inside the transponder.
        self.found.clear();
        let thr = floor * 4.0;
        let mut i = 0;
        while i < n {
            let f = rf_of_bin(at(i) as f64);
            if p[at(i)] > thr && (NB_LO - 2e3..=NB_HI + 2e3).contains(&f) {
                let s = i;
                let mut e = i;
                while e + 1 < n && p[at(e + 1)] > thr {
                    e += 1;
                }
                let top = (s..=e)
                    .max_by(|&a, &b| p[at(a)].total_cmp(&p[at(b)]))
                    .unwrap_or(s);
                let pk = p[at(top)];
                let half = pk / 4.0;
                let a = (s..=top)
                    .rev()
                    .take_while(|&j| p[at(j)] > half)
                    .last()
                    .unwrap_or(top);
                let b = (top..=e)
                    .take_while(|&j| p[at(j)] > half)
                    .last()
                    .unwrap_or(top);
                let kt = at(top);
                let frac = if kt > 0 && kt + 1 < n {
                    log_parabolic(p[kt - 1] as f64, p[kt] as f64, p[kt + 1] as f64).offset
                } else {
                    0.0
                };
                let rf_top = rf_of_bin(kt as f64 + frac);
                let beacon = BEACONS
                    .iter()
                    .find(|(_, f)| (rf_top - f).abs() < 1.5e3 + (e - s) as f64 * bin)
                    .map_or("", |b| b.0);
                self.found.push(Signal {
                    rf_hz: rf_top,
                    width_hz: (b - a + 1) as f64 * bin,
                    level_dbfs: db(pk as f64),
                    snr_db: db((pk / floor) as f64),
                    beacon,
                    ..Default::default()
                });
                i = e + 1;
            } else {
                i += 1;
            }
        }
        let found = &self.found;
        let signals = self
            .signals
            .exact(found.len(), |s| s.copy_from_slice(found));
        out.push(Timed {
            time: self.t,
            value: QoItem::Narrow {
                bins_dbfs: bins,
                f0_hz: f0,
                bin_hz: bin,
                floor_dbfs: db(floor as f64),
                signals,
            },
        });
        nb.acc.iter_mut().for_each(|v| *v = 0.0);
        nb.frames = 0;
    }

    /// The averaged wideband spectrum and its DATV carriers.
    fn wide_spectrum(&mut self, out: &mut Vec<Timed<QoItem>>) {
        let Some(w) = &mut self.wide else { return };
        if w.frames == 0 {
            return;
        }
        let n = WIDE_N;
        let bin = self.rate / n as f64;
        let frames = w.frames as f32;
        for a in w.acc.iter_mut() {
            *a /= frames;
        }
        let p = &w.acc;
        let floor = percentile(p, 0.5, &mut self.scratch).max(1e-30);
        let (conv, centre_if) = (self.conv, self.centre_if);
        let inv = conv.inverted();
        let rf = |k: usize| conv.rf_of((k as f64 - (n / 2) as f64) * bin + centre_if);
        let at = |i: usize| if inv { n - 1 - i } else { i };
        let bins = self.floats.exact(n, |s| {
            for (i, v) in s.iter_mut().enumerate() {
                *v = db(p[at(i)] as f64) as f32;
            }
        });
        // DATV carriers: regions at least 100 kHz wide, 3 dB over the floor, inside the wideband transponder.
        self.found_datv.clear();
        let edge = (0.05 * n as f64) as usize;
        let mut i = edge;
        while i < n - edge {
            if p[i] > floor * 2.0 {
                let s = i;
                while i < n - edge && p[i] > floor * 2.0 {
                    i += 1;
                }
                let width = (i - s) as f64 * bin;
                let rfc = rf((s + i) / 2);
                if width >= 100e3 && (10491e6..=10499e6).contains(&rfc) {
                    let top = p[s..i].iter().copied().fold(0.0f32, f32::max);
                    let mean = p[s..i].iter().sum::<f32>() / (i - s) as f32;
                    self.found_datv.push(Datv {
                        rf_hz: rfc,
                        if_hz: ((s + i) as f64 / 2.0 - (n / 2) as f64) * bin + centre_if,
                        width_hz: width,
                        symbol_rate_hz: width / 1.35,
                        snr_db: db((mean / floor) as f64),
                        flat_topped: mean > 0.5 * top,
                        beacon: (rfc - DATV_BEACON).abs() < 200e3,
                    });
                }
            } else {
                i += 1;
            }
        }
        let lo_rf = rf(if inv { n - 1 } else { 0 });
        let hi_rf = rf(if inv { 0 } else { n - 1 });
        // Outside: less than 0.5 MHz of the wideband transponder in the usable band.
        let (ulo, uhi) = (lo_rf + 0.05 * self.rate, hi_rf - 0.05 * self.rate);
        let outside = (uhi.min(10499e6) - ulo.max(10491e6)).max(0.0) < 0.5e6;
        let found = &self.found_datv;
        let datv = self.datvs.exact(found.len(), |s| s.copy_from_slice(found));
        out.push(Timed {
            time: self.t,
            value: QoItem::Wide {
                bins_dbfs: bins,
                f0_hz: lo_rf,
                bin_hz: bin,
                datv,
                outside,
            },
        });
        w.acc.iter_mut().for_each(|v| *v = 0.0);
        w.frames = 0;
    }
}

impl Decoder for QoReceiver {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.qo100_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<QoItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        QoReceiverSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (QoReceiverSetting::Lo, Value::Choice(self.lo_choice.index())),
            (
                QoReceiverSetting::LoCustomMhz,
                Value::F64(self.lo_custom_mhz),
            ),
            (QoReceiverSetting::AutoCorrection, Value::Bool(self.auto)),
            (QoReceiverSetting::LnbPpm, Value::F64(self.lnb_ppm)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let rebuild = match QoReceiverSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            QoReceiverSetting::Lo => {
                self.lo_choice = LO.read(id, v)?;
                true
            }
            QoReceiverSetting::LoCustomMhz => {
                self.lo_custom_mhz = LO_CUSTOM_MHZ.read(id, v)?;
                true
            }
            QoReceiverSetting::AutoCorrection => {
                self.auto = AUTO_CORRECTION.read(id, v)?;
                false
            }
            QoReceiverSetting::LnbPpm => {
                self.lnb_ppm = LNB_PPM.read(id, v)?;
                true
            }
        };
        if self.rate > 0.0 && rebuild {
            self.build().map_err(|e| SettingError::Config {
                id,
                source: Box::new(e),
            })?;
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre_if = f.centre_hz;
        self.t = 0.0;
        self.build()?;
        Ok(vec![f.with_kind(Kind::typed::<QoItem>())])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_if = f.centre_hz;
        let _ = self.build();
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        let out = out.items::<QoItem>(0);
        self.t = (first + iq.len() as u64) as f64 / self.rate;
        if let Some(w) = &mut self.wide {
            let nn = WIDE_N as u64;
            for (k, &v) in iq.iter().enumerate() {
                if ((first + k as u64) / nn).is_multiple_of(w.skip) {
                    w.frame.push(v);
                    if w.frame.len() == WIDE_N {
                        if w.welch.compute(&w.frame, &mut w.pow).is_ok() {
                            for (a, p) in w.acc.iter_mut().zip(&w.pow) {
                                *a += *p;
                            }
                            w.frames += 1;
                        }
                        w.frame.clear();
                    }
                }
            }
        }
        let Some(nb) = &mut self.narrow else { return };
        let mut mixed = std::mem::take(&mut nb.mixed);
        mixed.clear();
        nb.chan.process(iq, &mut mixed);
        for &v in &mixed {
            let Some(nb) = &mut self.narrow else { break };
            nb.fine.push(v);
            let fine_full = nb.fine.len() == FINE_N;
            nb.frame.push(v);
            if nb.frame.len() == NARROW_N {
                if nb.welch.compute(&nb.frame, &mut nb.pow).is_ok() {
                    for (a, p) in nb.acc.iter_mut().zip(&nb.pow) {
                        *a += *p;
                    }
                    nb.frames += 1;
                }
                nb.frame.clear();
            }
            if fine_full {
                self.estimate();
                if let Some(nb) = &mut self.narrow {
                    nb.fine.clear();
                }
            }
        }
        if let Some(nb) = &mut self.narrow {
            nb.mixed = mixed;
            if nb.frames >= NARROW_FRAMES {
                self.narrow_spectrum(out);
            }
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let out = out.items::<QoItem>(0);
        let mut l = self.lock;
        if self.narrow.is_none() {
            l.phase = LockPhase::Outside;
        }
        l.applied = self.auto;
        out.push(Timed {
            time: self.t,
            value: QoItem::Lock(l),
        });
        self.wide_spectrum(out);
    }
}

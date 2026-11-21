//! PSK31 and its variants: every signal in the passband decoded at once. The receiver finds the carriers in the window
//! spectrum and runs one channel decoder per carrier (mixer, matched filter, symbol clock, AFC, carrier detect and the
//! varicode; for BPSK the shared carrier loop, sequence detection over the overlap of the pulses and differential decoding; for
//! QPSK differential detection and the K = 5 Viterbi decoder). With Auto on it also chooses the modulation: it keeps the last
//! 10 s of the band, ranks trial decodes of the stretch with every modulation and applies the best. The generator sends any
//! number of clean signals.
//!
//! Standard: P. Martinez G3PLX, "PSK31: a new radio-teletype mode" (RadCom 1998/1999), and its varicode table.

use crate::auto::{Due, Trials};
use crate::decoder::HeardCommand;
use crate::decoder::{
    self, put_text, retain_rows, text_into, Heard, HfGenerator, Modulator, GEN_MESSAGE, GEN_SECTION,
};
use crate::fec::{SoftViterbi, QPSK_K5};
use crate::typed::{PskFields, PskSignalRow, PSK_SIGNALS};
use crate::util::{db, median};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Schema, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Kind, Outbox, Port, RunContext};
use sdrkit_util_text::score::{is_readable, readable_share};
use sdrkit_util_text::store::TextStore;
use sdrkit_util_text::varicode::psk as varicode;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

/// An angle wrapped into (-pi, pi].
#[inline]
fn wrap(a: f32) -> f32 {
    PI - (PI - a).rem_euclid(TAU)
}

/// The result of one bit in a channel's varicode decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariOut {
    /// Nothing yet.
    None,
    /// A character.
    Char(u8),
    /// A code the table does not have.
    Bad,
}

/// One bit to the varicode decoder.
fn vari_push(d: &mut varicode::Decoder, bit: bool) -> VariOut {
    match d.push(bit as u8) {
        None => VariOut::None,
        Some(Ok(b)) => VariOut::Char(b),
        Some(Err(_)) => VariOut::Bad,
    }
}

sdrkit_core::choice_enum! {
    /// A PSK variant.
    pub enum Variant {
        /// BPSK31.
        Bpsk31 = ("BPSK31", "BPSK31"),
        /// BPSK63.
        Bpsk63 = ("BPSK63", "BPSK63"),
        /// QPSK31.
        Qpsk31 = ("QPSK31", "QPSK31"),
        /// QPSK63.
        Qpsk63 = ("QPSK63", "QPSK63"),
        /// BPSK125.
        Bpsk125 = ("BPSK125", "BPSK125"),
        /// BPSK250.
        Bpsk250 = ("BPSK250", "BPSK250"),
        /// QPSK125.
        Qpsk125 = ("QPSK125", "QPSK125"),
    }
}

impl Variant {
    /// The name ("BPSK31").
    pub fn name(self) -> &'static str {
        match self {
            Variant::Bpsk31 => "BPSK31",
            Variant::Bpsk63 => "BPSK63",
            Variant::Qpsk31 => "QPSK31",
            Variant::Qpsk63 => "QPSK63",
            Variant::Bpsk125 => "BPSK125",
            Variant::Bpsk250 => "BPSK250",
            Variant::Qpsk125 => "QPSK125",
        }
    }
    /// Symbols per second.
    pub fn baud(self) -> f64 {
        match self {
            Variant::Bpsk31 | Variant::Qpsk31 => 31.25,
            Variant::Bpsk63 | Variant::Qpsk63 => 62.5,
            Variant::Bpsk125 | Variant::Qpsk125 => 125.0,
            Variant::Bpsk250 => 250.0,
        }
    }
    /// Quadrature (convolutionally coded).
    pub fn qpsk(self) -> bool {
        matches!(self, Variant::Qpsk31 | Variant::Qpsk63 | Variant::Qpsk125)
    }
}

/// The phase advance in quarter turns of a QPSK code symbol (bit 0 from mask 0x17, bit 1 from mask 0x19): (symbol + 2) mod 4.
/// An advance turns the complex envelope counter-clockwise, so on the upper sideband the audio tone's phase advances. This is
/// the designer's published phase table: the window 00001 (the newest bit 1) advances by 90 degrees, 00000 reverses.
pub fn qpsk_turns(sym: u32) -> u32 {
    (sym + 2) % 4
}

/// The code symbol of a phase advance in quarter turns.
pub fn qpsk_symbol(turns: u32) -> u32 {
    (turns + 2) % 4
}

/// The `modulation` setting.
pub const MODULATION_SETTING: Setting<field::Choice<Variant>> =
    Setting::choice("modulation", "Modulation", Variant::Bpsk31);
/// The `threshold` setting.
pub const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Threshold", "dB", 5.0)
    .range(4.0, 20.0, 0.5)
    .help("How far above the noise floor a signal must be to be decoded.");
/// The `focus_hz` setting.
pub const FOCUS_HZ: Setting<field::Hz> = Setting::new("focus_hz", "Focus", "Hz", -1.0)
    .range(-1.0, 4000.0, 0.0)
    .help("The signal nearest this offset is the focus (-1 for none).");

/// The `auto` setting.
pub const AUTO: Setting<field::Bool> = Setting::new("auto", "Automatic", "", true)
    .help("Chooses the modulation from the signals. Choosing the modulation by hand turns it off.");

sdrkit_core::settings! {
    /// The settings of [`PskDecoder`].
    pub enum PskSetting {
        /// Automatic modulation. It comes first: a modulation given with it turns it off.
        Auto = AUTO,
        /// The variant.
        Modulation = MODULATION_SETTING,
        /// The threshold above the noise floor.
        Threshold = THRESHOLD,
        /// The focus.
        FocusHz = FOCUS_HZ,
    }
}

/// The audio frequency of the test signal that sends `gen_message`.
pub const GEN_FREQ_HZ: Setting<field::Hz> =
    Setting::new("gen_freq_hz", "Audio frequency", "Hz", 1000.0)
        .range(100.0, 3900.0, 0.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the PSK stack.
    pub enum PskGenSetting {
        /// The text sent. Empty: the demo band.
        Message = GEN_MESSAGE,
        /// The audio frequency.
        FreqHz = GEN_FREQ_HZ,
    }
}

const RATE: f64 = 24000.0;
const SPS: usize = 16;
const NFFT: usize = 4096;
const LO_HZ: f64 = 100.0;
const HI_HZ: f64 = 3900.0;
const MAX_SIGNALS: usize = 24;
/// The rows of the signals table: the signals heard and those gone whose text stays.
pub const SIGNALS_CAP: usize = MAX_SIGNALS + 48;
const REPLAY_S: f64 = 1.5;
/// The band kept for the trials of Auto, seconds.
const KEEP_S: f64 = 10.0;
/// While locked, Auto tries again this often, seconds.
const RERUN_S: f64 = 10.0;
/// Auto loses its lock after this long without good text, seconds.
const LOST_S: f64 = 10.0;
/// The lock health reads this many of the newest characters of each signal.
const HEALTH_CHARS: usize = 80;

/// The share of the carrier loop's frequency that the mixer takes over each symbol.
const MIXER_SHARE: f64 = 0.1;
/// The share of the new symbol that the QPSK phase estimate takes each symbol.
const CREF_GAIN: f32 = 0.1;
/// The share of the phase error that the turn per symbol of the QPSK phase estimate takes: (CREF_GAIN / (2 x 0.707))^2, a
/// damping of 0.707.
const CREF_FREQ_GAIN: f32 = 0.005;
/// The text ends when the quality stays low for more than this many symbols.
const DCD_HOLD_SYMBOLS: u32 = 16;
/// The symbols the sequence detector waits before it decides one.
const SEQUENCE_DEPTH: u32 = 12;
/// A signal with text coming and a quality above this reads well with the present modulation.
const READING_QUALITY: f32 = 0.7;
/// A carrier is on only while its smoothed magnitude is at least this many times the noise level. The smoothed magnitude of
/// noise alone stays well under 1.5 times its mean; a signal at the 5 dB threshold has about twice the noise level.
const NOISE_MARGIN: f32 = 1.5;
/// A symbol of the replay of a new signal is before the burst when its power is under this share of the strongest symbol.
const QUIET_SHARE: f32 = 0.1;

/// The carrier loop of a BPSK signal: a noise bandwidth of 0.03 of the symbol rate, the lock test over 32 symbols.
fn carrier_loop() -> CarrierSync<Costas<2>> {
    let mut cfg = CarrierConfig::new(0.03);
    cfg.lock_updates = 32.0;
    cfg.max_freq = 0.02;
    CarrierSync::new(Costas::<2>, cfg).expect("the loop settings are constants in range")
}

/// Sequence detection of BPSK symbols behind the matched filter of the two-symbol cosine pulse. The filter output of a symbol
/// holds a sixth of each neighbour; the detector finds the most likely symbol sequence for that overlap (two states, the
/// metric of Ungerboeck's form) instead of removing the overlap with a linear filter, which costs 0.25 dB.
#[derive(Clone, Copy, Debug, Default)]
struct PulseSequence {
    /// The metric of the best path that ends in each symbol value (-1, +1).
    metric: [f32; 2],
    /// The symbols of those paths, the newest in bit 0.
    path: [u64; 2],
    /// The mean magnitude of the input: the level of a symbol.
    level: f32,
    /// The symbol decided last.
    last: bool,
}

impl PulseSequence {
    /// Takes the in-phase part `u` of the matched filter output at a symbol instant. Returns whether the symbol decided now
    /// (`SEQUENCE_DEPTH` symbols back) equals the one before it: the differentially coded bit.
    fn push(&mut self, u: f32) -> bool {
        self.level += 0.05 * (u.abs() - self.level);
        let overlap = self.level / 6.0;
        let mut metric = [0.0f32; 2];
        let mut path = [0u64; 2];
        for (new, sign) in [(0usize, -1.0f32), (1, 1.0)] {
            let from_low = self.metric[0] + sign * (u + overlap);
            let from_high = self.metric[1] + sign * (u - overlap);
            let (m, old) = if from_low >= from_high {
                (from_low, 0)
            } else {
                (from_high, 1)
            };
            metric[new] = m;
            path[new] = (self.path[old] << 1) | new as u64;
        }
        let top = metric[0].max(metric[1]);
        self.metric = [metric[0] - top, metric[1] - top];
        self.path = path;
        let best = usize::from(metric[1] >= metric[0]);
        let symbol = (path[best] >> SEQUENCE_DEPTH) & 1 == 1;
        let same = symbol == self.last;
        self.last = symbol;
        same
    }
}

/// One signal's decoder.
pub struct PskChannel {
    /// Stable id.
    pub id: u32,
    variant: Variant,
    /// Where it was found, Hz from the dial.
    pub found_hz: f64,
    /// Where the AFC is now.
    pub freq_hz: f64,
    ph: Complex32,
    rot: Complex32,
    nrm: u32,
    dec: usize,
    acc: Complex32,
    cnt: usize,
    mf: SampleRing<Complex32>,
    taps: Vec<f32>,
    n: u64,
    env: [f32; SPS],
    next_at: u64,
    symbols: u64,
    prev: Complex32,
    amp: f32,
    /// 0 to 1.
    pub quality: f32,
    /// Text is coming.
    pub dcd: bool,
    mse: f32,
    vit: Option<SoftViterbi>,
    vari: varicode::Decoder,
    /// Zero bits in a row since the text started, up to 2; the varicode takes bits only after two.
    gap: u8,
    /// The carrier detect of the newest symbols, the newest in bit 0.
    dcd_hist: u64,
    /// The steps the Viterbi decoder of QPSK holds a bit before it gives it.
    vit_lag: u32,
    /// The Viterbi decoder has given a bit.
    vit_out: bool,
    /// The text.
    pub store: TextStore,
    /// Bad codes.
    pub bad: u64,
    /// Picked by the user.
    pub manual: bool,
    /// The signal was heard on a frequency before the present one. Its text is a log entry: no new signal takes it.
    pub past: bool,
    /// Signal time first and last active.
    pub first: f64,
    /// See `first`.
    pub last: f64,
    t: u64,
    y1: Complex32,
    y2: Complex32,
    acq: SampleRing<Complex32>,
    acq_n: u64,
    low: u32,
    cref: Complex32,
    /// The turn of `cref` per symbol, radians.
    cstep: f32,
    state: Complex32,
    carrier: bool,
    on_level: f32,
    /// The mean magnitude of the symbols while no carrier is on (0 until the first such symbol).
    noise: f32,
    acq_last: f64,
    acq_fft: WindowedFft,
    /// Symbols decided while text was coming, those with no phase change (QPSK), and those far below the carrier level.
    fit: [u32; 3],
    /// The carrier loop of BPSK: the symbols turned onto the real axis.
    carrier_loop: CarrierSync<Costas<2>>,
    /// The sequence detector of BPSK.
    sequence: PulseSequence,
    /// The newest differential decisions of BPSK, the last in bit 0.
    steps: u64,
    /// Symbols in a row with a quality under the level that ends the text.
    weak: u32,
    /// Every decided bit of BPSK, for the error counts of the tests.
    #[cfg(test)]
    bits: Vec<bool>,
}

const ACQ_N: usize = 512;

impl PskChannel {
    /// A decoder for a signal at `hz` from the dial.
    pub fn new(id: u32, variant: Variant, hz: f64, t: u64) -> Self {
        let dec = (RATE / (SPS as f64 * variant.baud())).round() as usize;
        let taps: Vec<f32> = (0..=2 * SPS)
            .map(|i| 0.5 * (1.0 + (PI * (i as f32 - SPS as f32) / SPS as f32).cos()))
            .collect();
        let mut c = PskChannel {
            id,
            variant,
            found_hz: hz,
            freq_hz: hz,
            ph: Complex32::new(1.0, 0.0),
            rot: Complex32::new(1.0, 0.0),
            nrm: 0,
            dec,
            acc: Complex32::default(),
            cnt: 0,
            mf: SampleRing::new(taps.len()).expect("a ring of at least one sample"),
            taps,
            n: 0,
            env: [0.0; SPS],
            next_at: SPS as u64,
            symbols: 0,
            prev: Complex32::default(),
            amp: 0.0,
            quality: 0.0,
            dcd: false,
            mse: 1.0,
            vit: variant.qpsk().then(|| SoftViterbi::new(QPSK_K5, 40)),
            vari: varicode::Decoder::new(),
            gap: 0,
            dcd_hist: 0,
            vit_lag: 0,
            vit_out: false,
            store: decoder::store(2000, 60),
            bad: 0,
            manual: false,
            past: false,
            first: t as f64 / RATE,
            last: t as f64 / RATE,
            t,
            y1: Complex32::default(),
            y2: Complex32::default(),
            acq: SampleRing::new(ACQ_N).expect("a ring of at least one sample"),
            acq_n: 0,
            low: 0,
            cref: Complex32::default(),
            cstep: 0.0,
            state: Complex32::new(1.0, 0.0),
            // A new channel starts on a carrier: the noise level is measured only after it ends.
            carrier: true,
            on_level: 0.0,
            noise: 0.0,
            acq_last: 0.0,
            acq_fft: WindowedFft::new(ACQ_N, ACQ_N, Window::Hann)
                .expect("a window that fits the transform"),
            fit: [0; 3],
            carrier_loop: carrier_loop(),
            sequence: PulseSequence::default(),
            steps: 0,
            weak: 0,
            #[cfg(test)]
            bits: Vec::new(),
        };
        c.set_freq(hz);
        c
    }

    fn set_freq(&mut self, hz: f64) {
        self.freq_hz = hz;
        let w = -std::f64::consts::TAU * hz / RATE;
        self.rot = Complex32::new(w.cos() as f32, w.sin() as f32);
    }

    /// SNR in the signal's bandwidth, dB.
    pub fn snr_db(&self) -> f32 {
        let m = self.mse.max(1e-4);
        (10.0 * (1.0 / (2.0 * m)).log10()).clamp(-10.0, 40.0)
    }

    /// One 24 kHz sample. Returns a decoded byte (or a bad code) when one completes.
    #[inline]
    pub fn step(&mut self, x: Complex32) -> VariOut {
        self.t += 1;
        self.acc += x * self.ph;
        self.ph *= self.rot;
        self.nrm += 1;
        if self.nrm >= 1024 {
            self.nrm = 0;
            self.ph /= self.ph.norm();
        }
        self.cnt += 1;
        if self.cnt < self.dec {
            return VariOut::None;
        }
        self.cnt = 0;
        let y0 = self.acc / self.dec as f32;
        self.acc = Complex32::default();
        self.mf.push(y0);
        let w = self.mf.last(self.taps.len());
        let mut y = Complex32::default();
        for (h, v) in self.taps.iter().zip(w) {
            y += *v * *h;
        }
        let k = (self.n % SPS as u64) as usize;
        let ea = if self.symbols < 40 { 0.15 } else { 0.03 };
        self.env[k] += ea * (y.norm() - self.env[k]);
        if self.carrier && (!self.variant.qpsk() || !self.dcd) {
            self.acquire(y);
        }
        let out = if self.n == self.next_at {
            let r = self.symbol(y);
            // Move the decision one step a symbol towards the envelope peak (straight to it for the first symbols).
            let best = (0..SPS)
                .max_by(|&a, &b| self.env[a].total_cmp(&self.env[b]))
                .expect("the range is not empty");
            let cur = k;
            let mut d = best as i64 - cur as i64;
            if d > SPS as i64 / 2 {
                d -= SPS as i64;
            }
            if d < -(SPS as i64) / 2 {
                d += SPS as i64;
            }
            let step = if self.symbols < 8 { d } else { d.signum() };
            let step = if self.env[best] > 1.05 * self.env[cur] || self.symbols < 8 {
                step
            } else {
                // Near the peak the envelope is flat: for BPSK the vertex of the parabola through the instant and its two
                // neighbours moves the decision when it lies more than 0.6 of a step away.
                let (before, at, after) = (
                    self.env[(cur + SPS - 1) % SPS],
                    self.env[cur],
                    self.env[(cur + 1) % SPS],
                );
                let bend = 2.0 * at - before - after;
                let vertex = if bend > 0.0 && !self.variant.qpsk() {
                    0.5 * (after - before) / bend
                } else {
                    0.0
                };
                if vertex > 0.6 {
                    1
                } else if vertex < -0.6 {
                    -1
                } else {
                    0
                }
            };
            self.next_at = (self.n as i64 + SPS as i64 + step) as u64;
            r
        } else {
            VariOut::None
        };
        self.n += 1;
        out
    }

    /// Frequency acquisition while nothing decodes: the modulation is removed by squaring (BPSK) or the fourth power (QPSK), and
    /// the line that remains is at that multiple of the tuning error.
    fn acquire(&mut self, y: Complex32) {
        // Squaring removes BPSK and the idle reversals of QPSK alike.
        let p = 2;
        let a = y.norm().max(1e-12);
        let u = y / a;
        let v = if p == 2 { u * u } else { (u * u) * (u * u) };
        self.acq.push(v);
        self.acq_n += 1;
        if self.acq_n < ACQ_N as u64 / 2 || !self.acq_n.is_multiple_of(ACQ_N as u64 / 4) {
            return;
        }
        let bins = self.acq_fft.run(self.acq.last(ACQ_N));
        let fs = SPS as f64 * self.variant.baud();
        // A QPSK signal off by a quarter of the baud rate looks like one in tune after squaring, so the search stays inside it.
        let span = if self.variant.qpsk() { 0.4 } else { 0.6 };
        let max_bin = ((self.variant.baud() * p as f64 * span) / fs * ACQ_N as f64) as usize;
        let mut best = (0usize, 0.0f32);
        let mut sum = 0.0f32;
        for k in 0..ACQ_N {
            let m = bins[k].norm_sqr();
            sum += m;
            let kk = if k > ACQ_N / 2 { ACQ_N - k } else { k };
            if kk <= max_bin && m > best.1 {
                best = (k, m);
            }
        }
        let mean = sum / ACQ_N as f32;
        let need = if self.dcd { 25.0 } else { 12.0 };
        if best.1 > need * mean {
            let k = best.0 as f64;
            let k = if k > ACQ_N as f64 / 2.0 {
                k - ACQ_N as f64
            } else {
                k
            };
            let df = k * fs / ACQ_N as f64 / p as f64;
            let f = self.freq_hz + df;
            // While text comes, only a lock half the symbol rate off (reversals look like a carrier there) is left.
            let (min, max) = if self.dcd {
                (0.4, 0.6)
            } else {
                (0.08, f64::MAX)
            };
            let off = df.abs() / self.variant.baud();
            if off > min && off < max && (f - self.acq_last).abs() < 0.1 * self.variant.baud() {
                self.found_hz = f;
                self.set_freq(f);
            }
            self.acq_last = f;
        }
    }

    fn symbol(&mut self, y_new: Complex32) -> VariOut {
        // Remove the one-sixth leak of the neighbours (the two-symbol pulse through its matched filter), one symbol late.
        let y = self.y1 - (self.y2 + y_new) * (1.0 / 6.0);
        self.y2 = self.y1;
        self.y1 = y_new;
        self.symbols += 1;
        let a = y.norm();
        let z = y * self.prev.conj();
        self.prev = y;
        let strong = self.amp;
        self.amp += 0.1 * (a - self.amp);
        // The level while a carrier is on: rises fast, falls slowly.
        if self.amp > self.on_level {
            self.on_level += 0.2 * (self.amp - self.on_level);
        } else {
            self.on_level *= 0.998;
        }
        // The noise level: the mean magnitude while no carrier is on. A symbol counts at most twice the level, so that the
        // rise of a carrier lifts the level more slowly than the carrier lifts its own magnitude.
        if !self.carrier {
            self.noise = if self.noise > 0.0 {
                self.noise + 0.05 * (a.min(2.0 * self.noise) - self.noise)
            } else {
                a
            };
        }
        if self.amp < 0.3 * self.on_level || self.amp < NOISE_MARGIN * self.noise {
            self.low += 1;
        } else {
            self.low = 0;
        }
        let carrier = self.low < 6;
        if carrier && !self.carrier {
            // A carrier starts: find the symbol clock again at once.
            self.symbols = 0;
            self.env = [0.0; SPS];
            self.acq_n = 0;
            self.carrier_loop.reset();
            self.sequence = PulseSequence::default();
            self.cstep = 0.0;
        }
        self.carrier = carrier;
        let qpsk = self.variant.qpsk();
        let ang = z.arg();
        // The quality is the fit of the phase step to the points of the modulation: four for QPSK, whose reversals before
        // the text are two of them. Before text comes, the AFC of QPSK takes the BPSK phase error of those reversals, which
        // pulls in from twice as far, so that it does not settle a quarter of the baud rate away.
        let (fit, lim) = if qpsk {
            (wrap(4.0 * ang) / 4.0, PI / 4.0)
        } else {
            (wrap(2.0 * ang) / 2.0, FRAC_PI_2)
        };
        let q = 1.0 - (fit * fit) / (lim * lim / 3.0);
        self.quality += 0.07 * (q - self.quality);
        self.mse += 0.05 * (fit * fit - self.mse);
        let pulling = !self.dcd;
        let e = if qpsk && pulling {
            wrap(2.0 * ang) / 2.0
        } else {
            fit
        };
        // AFC from the remaining phase step.
        let g = if pulling { 0.06 } else { 0.01 };
        let t_sym = 1.0 / self.variant.baud();
        let df =
            g * e as f64 / (std::f64::consts::TAU * t_sym) * (62.5 / self.variant.baud()).sqrt();
        // The phase steps of noise have a quality near 0: they move a BPSK channel little.
        let df = if qpsk {
            df
        } else {
            df * f64::from(self.quality.clamp(0.0, 1.0))
        };
        let df = if carrier { df } else { 0.0 };
        // BPSK: the carrier loop turns the symbols at this instant. While text comes and the loop is locked, the mixer takes
        // over a tenth of the loop's frequency each symbol, and the phase steps move it no more.
        let coherent = (!qpsk).then(|| self.carrier_loop.push(y_new).re);
        let locked = !qpsk && self.carrier_loop.locked();
        if !qpsk && !self.dcd {
            // Before the text the phase steps hold the frequency; the loop follows the phase alone.
            self.carrier_loop.set_freq(0.0);
        }
        let df = if locked && self.dcd {
            let f = self.carrier_loop.freq() * self.variant.baud();
            let take = MIXER_SHARE * f;
            self.carrier_loop.set_freq((f - take) / self.variant.baud());
            take
        } else {
            df
        };
        let nf = self.freq_hz + df;
        if (nf - self.found_hz).abs() > 2.0 * self.variant.baud() {
            let f = self.found_hz;
            self.set_freq(f);
        } else {
            self.set_freq(nf);
        }
        // DCD.
        let _ = strong;
        let gone = !carrier;
        self.weak = if self.quality < 0.15 {
            self.weak + 1
        } else {
            0
        };
        // The text goes on over a short dip of the quality.
        if self.dcd && (self.weak > DCD_HOLD_SYMBOLS || gone) {
            self.dcd = false;
            self.vari.reset();
            self.gap = 0;
        } else if !self.dcd && self.quality > 0.35 && !gone {
            self.dcd = true;
        }
        self.dcd_hist = (self.dcd_hist << 1) | u64::from(self.dcd);
        // QPSK: detection against a phase estimate averaged over the last symbols (decision feedback): close to coherent
        // detection, with the plain differential product while the estimate builds up. The estimate turns by the remaining
        // frequency error each symbol, so that a small error of the AFC does not move it off the points.
        self.cref *= Complex32::from_polar(1.0, self.cstep);
        let r = self.cref * self.state;
        let zd = if self.cref.norm() > 0.3 * self.amp {
            y * r.conj()
        } else {
            z
        };
        if self.dcd && carrier {
            self.fit[0] += 1;
            self.fit[2] += u32::from(a < 0.5 * self.on_level);
        }
        if let Some(u) = coherent {
            // The sequence detector's bit is `SEQUENCE_DEPTH` symbols late; the phase step of the same pair of symbols stands
            // in until the loop is locked.
            self.steps = (self.steps << 1) | u64::from(z.re > 0.0);
            let same = self.sequence.push(u);
            let bit = if locked {
                same
            } else {
                (self.steps >> (SEQUENCE_DEPTH - 1)) & 1 == 1
            };
            #[cfg(test)]
            self.bits.push(bit);
            if !self.dcd {
                return VariOut::None;
            }
            return self.text_bit(bit, SEQUENCE_DEPTH);
        }
        let Some(v) = self.vit.as_mut() else {
            return VariOut::None;
        };
        let sc = 1.0 / self.amp.max(1e-9).powi(2);
        let mut bm = [0.0f32; 4];
        let mut best = (f32::MIN, 0u32);
        for (sym, m) in bm.iter_mut().enumerate() {
            let turns = qpsk_turns(sym as u32);
            let th = turns as f32 * FRAC_PI_2;
            *m = (zd.re * th.cos() + zd.im * th.sin()) * sc / self.amp.max(1e-9)
                * zd.norm().max(1e-12).recip()
                * y.norm();
            if *m > best.0 {
                best = (*m, turns);
            }
        }
        if self.dcd && carrier && best.1 == 0 {
            self.fit[1] += 1;
        }
        self.state *= Complex32::new(0.0, 1.0).powu(best.1);
        if self.cref.norm() > 0.3 * self.amp {
            // The second-order part of the loop: the phase error of the decided point moves the turn per symbol.
            let err = (y * (self.cref * self.state).conj()).arg();
            self.cstep = (self.cstep + CREF_FREQ_GAIN * err).clamp(-0.3, 0.3);
        }
        self.cref += CREF_GAIN * (y * self.state.conj() - self.cref);
        // Max-log soft values of the two code bits (positive means 1) from the four label metrics, scaled to about 1.
        let top = bm.iter().fold(1e-12f32, |m, v| m.max(v.abs()));
        let soft = |bit: usize| {
            let (mut one, mut zero) = (f32::MIN, f32::MIN);
            for (sym, &m) in bm.iter().enumerate() {
                if (sym >> bit) & 1 == 1 {
                    one = one.max(m);
                } else {
                    zero = zero.max(m);
                }
            }
            0.5 * (one - zero) / top
        };
        let (s0, s1) = (soft(0), soft(1));
        let out = v.push(s0, s1);
        self.vit_out |= out.is_some();
        if !self.vit_out {
            self.vit_lag += 1;
        }
        match out {
            Some(bit) if self.dcd => self.text_bit(bit == 1, self.vit_lag),
            _ => VariOut::None,
        }
    }

    /// One decided bit while text comes, of the symbol `lag` symbols back. A bit is text only when the carrier detect was on
    /// at its symbol and the four symbols before it, and the varicode takes bits only after the first two zero bits in a row
    /// of text (the gap before every character): the noise before a burst and the rise of its carrier give no character.
    fn text_bit(&mut self, bit: bool, lag: u32) -> VariOut {
        if (self.dcd_hist >> lag.min(59)) & 0x1f != 0x1f {
            self.gap = 0;
            return VariOut::None;
        }
        if self.gap < 2 {
            self.gap = if bit { 0 } else { self.gap + 1 };
            if self.gap == 2 {
                self.vari.reset();
            }
            return VariOut::None;
        }
        vari_push(&mut self.vari, bit)
    }
}

/// Where a burst at `hz` from the dial starts in the stretch `x` (24 kHz) that ends inside it: the index of the first
/// sample after the last symbol whose power in four times the symbol rate around `hz` is under [`QUIET_SHARE`] of the
/// strongest symbol. 0 when no symbol is that quiet: the signal was on over the whole stretch, or the noise is too near it.
fn burst_start(x: &[Complex32], hz: f64, baud: f64) -> usize {
    let part = ((RATE / (4.0 * baud)).round() as usize).max(1);
    let w = -std::f64::consts::TAU * hz / RATE;
    let rot = Complex32::new(w.cos() as f32, w.sin() as f32);
    let mut ph = Complex32::new(1.0, 0.0);
    let (mut top, mut quiet_end) = (0.0f32, 0usize);
    let mut powers = Vec::with_capacity(x.len() / (4 * part) + 1);
    for symbol in x.chunks_exact(4 * part) {
        let mut p = 0.0;
        for q in symbol.chunks_exact(part) {
            let mut acc = Complex32::default();
            for &v in q {
                acc += v * ph;
                ph *= rot;
            }
            ph /= ph.norm();
            p += acc.norm_sqr();
        }
        top = top.max(p);
        powers.push(p);
    }
    for (i, &p) in powers.iter().enumerate() {
        if p < QUIET_SHARE * top {
            quiet_end = (i + 1) * 4 * part;
        }
    }
    quiet_end
}

/// The PSK receiver: every signal in the window.
pub struct PskDecoder {
    variant: Variant,
    threshold: f64,
    focus_hz: Option<f64>,
    rate: f64,
    dial: f64,
    spec: WindowedFft,
    buf: Vec<Complex32>,
    held: Vec<f32>,
    avg: Vec<f32>,
    frames: u64,
    smooth: Vec<f32>,
    scratch: Vec<f32>,
    ring: SampleRing<Complex32>,
    t: u64,
    chans: Vec<PskChannel>,
    gone: Vec<PskChannel>,
    next_id: u32,
    floor: f32,
    chars: u64,
    bad: u64,
    seq: u64,
    heard: Heard,
    status: Status,
    key: String,
    text: String,
    stale: Vec<String>,
    spectrum: Option<SeriesId>,
    peaks: Vec<(f64, f32)>,
    cands: Vec<(f64, u32, u32)>,
    // Auto
    auto: bool,
    trials: Trials<Option<Variant>>,
    found: Option<Variant>,
    pending: Option<Variant>,
    good_at: u64,
}

/// Every modulation Auto tries. The 8PSK forms would also give text from a BPSK or QPSK signal and need about 8 dB more.
pub const VARIANTS: [Variant; 7] = [
    Variant::Bpsk31,
    Variant::Bpsk63,
    Variant::Qpsk31,
    Variant::Qpsk63,
    Variant::Bpsk125,
    Variant::Bpsk250,
    Variant::Qpsk125,
];

/// The score of a trial decode of the stretch `x` (24 kHz) with modulation `v` and threshold `threshold` (dB): with `ok` good
/// characters, `bad` bad codes, `r` the readable share of the last two thirds of all its text and `fit` the symbol fit (0 to
/// 1), 0 unless `ok` is at least 16 and 4 times `bad` and `r` is at least 0.85; else `1000 (ok - 3 bad) r^2 fit / baud`. The
/// symbol fit is `(1 - 4 max(0, s - 0.25)) (1 - 4 w)`, with `s` the share of decided QPSK symbols without a phase change and
/// `w` the share of symbols under half the carrier level.
pub fn trial_score(x: &[Complex32], v: Variant, threshold: f64) -> f64 {
    let mut d = PskDecoder::keeping(REPLAY_S);
    d.auto = false;
    d.variant = v;
    d.threshold = threshold;
    let kinds: Vec<Kind> = decoder::formats().iter().map(|f| f.kind).collect();
    let mut out = Outbox::new(&kinds);
    for c in x.chunks(2400) {
        sdrkit_graph::Decoder::process(&mut d, c, 0, &mut out);
        out.items::<crate::kinds::TextChar>(decoder::CHARS).clear();
        out.messages(decoder::EVENTS).clear();
    }
    let (ok, bad) = (d.chars as f64, d.bad as f64);
    let text: Vec<char> = d
        .gone
        .iter()
        .chain(&d.chans)
        .flat_map(|c| c.store.text())
        .collect();
    let r = readable_share(text[text.len() / 3..].iter().copied());
    // How well the symbols fit the modulation: a decoder faster than the signal sees many symbols with no phase change
    // (beyond the quarter that QPSK text has), one slower than the signal sees symbols that cancel. Its letters would read as
    // text.
    let f = d.gone.iter().chain(&d.chans).fold([0u32; 3], |f, c| {
        [f[0] + c.fit[0], f[1] + c.fit[1], f[2] + c.fit[2]]
    });
    let n = f64::from(f[0].max(1));
    let (still, weak) = (f64::from(f[1]) / n, f64::from(f[2]) / n);
    let fit = ((1.0 - 4.0 * (still - 0.25).max(0.0)) * (1.0 - 4.0 * weak)).clamp(0.0, 1.0);
    if ok >= 16.0 && ok >= 4.0 * bad && r >= 0.85 {
        1000.0 * (ok - 3.0 * bad) * r * r * fit / v.baud()
    } else {
        0.0
    }
}

/// The modulation of the best [`trial_score`] of the stretch `x` above 0, if any.
pub fn choose(x: &[Complex32], threshold: f64) -> Option<Variant> {
    VARIANTS
        .iter()
        .map(|&v| (v, trial_score(x, v, threshold)))
        .filter(|s| s.1 > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|s| s.0)
}

impl PskDecoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        Self::keeping(KEEP_S)
    }

    /// A receiver that keeps `seconds` of the band.
    fn keeping(seconds: f64) -> Self {
        let mut trials = Trials::new();
        trials.configure(RATE);
        if AUTO.default {
            trials.on();
        }
        PskDecoder {
            variant: Variant::Bpsk31,
            threshold: 5.0,
            focus_hz: None,
            rate: RATE,
            dial: 0.0,
            spec: WindowedFft::new(NFFT, NFFT, Window::Hann)
                .expect("a window that fits the transform"),
            buf: Vec::with_capacity(NFFT),
            held: vec![0.0; NFFT],
            avg: vec![0.0; NFFT],
            frames: 0,
            smooth: vec![0.0; NFFT],
            scratch: Vec::new(),
            ring: SampleRing::new((seconds.max(REPLAY_S) * RATE) as usize)
                .expect("a ring of at least one sample"),
            t: 0,
            chans: Vec::new(),
            gone: Vec::new(),
            next_id: 1,
            floor: 0.0,
            chars: 0,
            bad: 0,
            seq: 0,
            heard: Heard::new("BPSK31"),
            status: Status::default(),
            key: String::new(),
            text: String::new(),
            stale: Vec::new(),
            spectrum: None,
            peaks: Vec::new(),
            cands: Vec::new(),
            auto: AUTO.default,
            trials,
            found: None,
            pending: None,
            good_at: 0,
        }
    }

    /// The signals being followed.
    pub fn channels(&self) -> &[PskChannel] {
        &self.chans
    }

    fn forget_signals(&mut self) {
        let chans = std::mem::take(&mut self.chans);
        for c in chans {
            self.retire(c);
        }
        self.held.iter_mut().for_each(|h| *h = 0.0);
        self.frames = 0;
        self.buf.clear();
        self.cands.clear();
    }

    /// Samples of the band held, at most the ring.
    fn held(&self) -> usize {
        self.ring.end().min(self.ring.capacity() as u64) as usize
    }

    /// Forgets the signals and the band kept, and drops a running trial.
    fn reset(&mut self) {
        self.forget_signals();
        self.ring.reset();
        self.trials.reset();
        self.pending = None;
    }

    /// Changes the modulation.
    fn set_variant(&mut self, v: Variant) {
        if v != self.variant {
            self.variant = v;
            self.heard.set_stack(v.name());
            self.forget_signals();
        }
    }

    /// Takes a finished trial, and starts one that is due while a carrier is in the window.
    fn run_auto(&mut self) {
        if let Some((v, _)) = self.trials.poll() {
            self.take(v);
        }
        if self.chans.is_empty() && self.peaks.is_empty() {
            return;
        }
        if let Some(due) = self.trials.due(self.auto, Some(RERUN_S)) {
            self.start_trial(due);
        }
    }

    fn start_trial(&mut self, due: Due) {
        let n = ((due.seconds * RATE) as usize).min(self.held());
        let stretch = self.ring.last(n).to_vec();
        let threshold = self.threshold;
        self.trials
            .start(due, stretch, move |x| choose(&x, threshold));
        if let Some((v, _)) = self.trials.poll() {
            self.take(v);
        }
    }

    /// The answer `v` of a trial. The present modulation locks at once. Another one is applied at once while not locked and
    /// no signal reads well with the present modulation; else it must come twice in a row: one trial on the first seconds
    /// of a burst can name a faster modulation. No answer while not locked clears the modulation found.
    fn take(&mut self, v: Option<Variant>) {
        if !self.auto {
            return;
        }
        let locked = self.trials.locked();
        let reading = self
            .chans
            .iter()
            .any(|c| c.dcd && c.quality > READING_QUALITY);
        match v {
            None if !locked => {
                self.pending = None;
                self.found = None;
                self.trials.nothing();
            }
            None => self.pending = None,
            Some(v) if locked && v == self.variant => self.pending = None,
            Some(v) if v != self.variant && self.pending != Some(v) && (locked || reading) => {
                self.pending = Some(v);
            }
            Some(v) => {
                self.pending = None;
                self.found = Some(v);
                if v != self.variant {
                    self.heard.logs.push(format!("automatic: {}", v.name()));
                }
                self.set_variant(v);
                self.trials.lock();
                self.good_at = self.t;
            }
        }
    }

    /// The health of a lock: when characters came, the newest 80 of each signal are good text when they are fewer than 24 or
    /// at least 0.8 of them read as text. 10 s without good text loses the lock.
    fn watch(&mut self, new_chars: bool) {
        if !self.trials.locked() {
            return;
        }
        if new_chars {
            let (mut n, mut good) = (0usize, 0usize);
            for c in &self.chans {
                let len = c.store.text().count();
                for ch in c.store.text().skip(len.saturating_sub(HEALTH_CHARS)) {
                    n += 1;
                    good += usize::from(is_readable(ch));
                }
            }
            if n < 24 || good * 5 >= n * 4 {
                self.good_at = self.t;
            }
        }
        if (self.t - self.good_at.min(self.t)) as f64 / RATE > LOST_S {
            self.trials.unlock();
            self.pending = None;
            self.heard.logs.push("automatic lock lost".into());
        }
    }

    fn retire(&mut self, c: PskChannel) {
        if c.store.total > 0 {
            self.gone.retain(|g| {
                g.past || (g.freq_hz - c.freq_hz).abs() > 0.5 * self.variant.baud() || g.id == c.id
            });
            self.gone.push(c);
            if self.gone.len() > 48 {
                self.gone.remove(0);
            }
        }
    }

    fn bin_hz(&self) -> f64 {
        RATE / NFFT as f64
    }

    fn find(&mut self) {
        let p = self.spec.run_power(&self.buf);
        let first = self.frames == 0;
        self.frames += 1;
        for ((h, a), &v) in self.held.iter_mut().zip(self.avg.iter_mut()).zip(p) {
            *h = if v > *h { v } else { *h + 0.15 * (v - *h) };
            *a = if first { v } else { *a + 0.2 * (v - *a) };
        }
        let bin = self.bin_hz();
        let lo = (LO_HZ / bin).ceil() as usize;
        let hi = (HI_HZ / bin).floor() as usize;
        let baud = self.variant.baud();
        let half = ((0.5 * baud) / bin).round().max(1.0) as usize;
        let n = (2 * half + 1) as f32;
        for i in lo..=hi {
            let mut s = 0.0;
            for k in i.saturating_sub(half)..=(i + half).min(NFFT / 2) {
                s += self.avg[k];
            }
            self.smooth[i] = s;
        }
        self.floor = median(&self.avg[lo..=hi], &mut self.scratch).max(1e-20);
        let top = self.smooth[lo..=hi].iter().copied().fold(0.0f32, f32::max);
        // The SNR in the signal's bandwidth: the power above the floor in its width, over the floor in that width.
        let need = 10f32.powf(self.threshold as f32 / 10.0);
        let noise = n * self.floor;
        self.peaks.clear();
        for i in lo + 1..hi {
            let v = self.smooth[i];
            let snr = (v - noise) / noise;
            if snr > need && v > top * 1e-4 && v >= self.smooth[i - 1] && v > self.smooth[i + 1] {
                // Parabolic interpolation of the peak.
                let (a, b, c) = (
                    self.smooth[i - 1] as f64,
                    v as f64,
                    self.smooth[i + 1] as f64,
                );
                let den = a - 2.0 * b + c;
                let d = if den.abs() > 1e-30 {
                    (0.5 * (a - c) / den).clamp(-0.5, 0.5)
                } else {
                    0.0
                };
                self.peaks.push(((i as f64 + d) * bin, snr));
            }
        }
        // Strongest first. At least 1.5 widths apart. Not the keying sidebands or IMD of a much stronger signal.
        self.peaks
            .sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.total_cmp(&b.0)));
        let mut kept = 0;
        for i in 0..self.peaks.len() {
            let (f, v) = self.peaks[i];
            let clash = self.peaks[..kept].iter().any(|&(g, w)| {
                let d = (f - g).abs();
                d < 1.5 * baud || (d < 4.0 * baud && w > 30.0 * v)
            });
            if !clash {
                self.peaks[kept] = (f, v);
                kept += 1;
            }
        }
        self.peaks.truncate(kept);
        let peaks = std::mem::take(&mut self.peaks);
        // A peak becomes a signal once it has stayed put for three spectra.
        let mut cands = std::mem::take(&mut self.cands);
        cands.retain_mut(|(cf, n, miss)| {
            if let Some(&(f, _)) = peaks.iter().find(|p| (p.0 - *cf).abs() < 0.5 * baud) {
                *cf = (*cf * *n as f64 + f) / (*n + 1) as f64;
                *n += 1;
                *miss = 0;
            } else {
                *miss += 1;
            }
            *miss <= 6
        });
        for &(f, _) in &peaks {
            if !cands.iter().any(|c| (c.0 - f).abs() < 0.5 * baud) {
                cands.push((f, 1, 0));
            }
        }
        for c in &cands {
            let near = self
                .chans
                .iter()
                .any(|ch| (ch.freq_hz - c.0).abs() < 1.5 * baud);
            if c.1 >= 3 && !near && self.chans.len() < MAX_SIGNALS {
                self.start(c.0, false);
            }
        }
        cands.truncate(64);
        self.cands = cands;
        self.peaks = peaks;
    }

    fn start(&mut self, f: f64, manual: bool) {
        // Replay the last 1.5 s so the idle reversals lock it before the text, from the start of the burst when the burst
        // starts inside them: the noise before a burst moves the frequency and hides the start of the carrier.
        let n = self.held().min((REPLAY_S * RATE) as usize);
        let n = n - burst_start(self.ring.last(n), f, self.variant.baud());
        let begin = self.t.saturating_sub(n as u64);
        let mut c = PskChannel::new(self.next_id, self.variant, f, begin);
        c.manual = manual;
        self.next_id += 1;
        let hist = self.ring.last(n);
        let mut ctx_dummy = Vec::new();
        for &x in hist {
            if let VariOut::Char(b) = c.step(x) {
                ctx_dummy.push((c.t, b));
            }
        }
        // A new signal where a gone one was carries its text on.
        if let Some(i) = self
            .gone
            .iter()
            .position(|g| !g.past && (g.freq_hz - f).abs() < 0.8 * self.variant.baud())
        {
            let g = self.gone.remove(i);
            c.store = g.store;
            c.id = g.id;
            c.first = g.first;
        }
        for (t, b) in ctx_dummy {
            c.store.push_byte(b, t as f64 / RATE, self.dial + f, c.id);
            self.chars += 1;
        }
        self.chans.push(c);
    }

    fn focus_id(&self) -> Option<u32> {
        let f = self.focus_hz?;
        self.chans
            .iter()
            .min_by(|a, b| (a.freq_hz - f).abs().total_cmp(&(b.freq_hz - f).abs()))
            .map(|c| c.id)
    }

    /// Writes the row of signal `i` (of the gone ones when `gone`) into the signals table.
    fn signal_row(&mut self, i: usize, gone: bool, p: &mut Published) {
        let c = if gone { &self.gone[i] } else { &self.chans[i] };
        put_text(&mut self.key, format_args!("{}", c.id));
        text_into(&c.store, &mut self.text);
        PSK_SIGNALS.put(
            p,
            &self.key,
            &PskSignalRow {
                id: i64::from(c.id),
                offset_hz: Hertz(c.freq_hz),
                afc_hz: Hertz(c.freq_hz - c.found_hz),
                snr_db: f64::from(c.snr_db()),
                quality_pct: (f64::from(c.quality) * 100.0).round(),
                dcd: c.dcd,
                manual: c.manual,
                first_s: c.first,
                last_s: c.last,
                characters: c.store.total as i64,
                text: &self.text,
                gone,
            },
        );
    }
}

impl Default for PskDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl sdrkit_graph::Decoder for PskDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.psk"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        PskSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match PskSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PskSetting::Modulation => {
                let v = MODULATION_SETTING.read(id, value)?;
                if v != self.variant {
                    // A modulation chosen by hand turns Auto off.
                    if self.auto {
                        self.auto = false;
                        self.trials.off();
                        self.found = None;
                        self.pending = None;
                    }
                    self.set_variant(v);
                }
            }
            PskSetting::Auto => {
                let a = AUTO.read(id, value)?;
                if a != self.auto {
                    self.auto = a;
                    self.pending = None;
                    if a {
                        self.trials.on();
                    } else {
                        self.trials.off();
                        self.found = None;
                    }
                }
            }
            PskSetting::Threshold => self.threshold = THRESHOLD.read(id, value)?,
            PskSetting::FocusHz => {
                let f = FOCUS_HZ.read(id, value)?;
                self.focus_hz = (f >= 0.0).then_some(f);
                if let Some(f) = self.focus_hz {
                    let near = self
                        .chans
                        .iter()
                        .any(|c| (c.freq_hz - f).abs() < self.variant.baud());
                    if !near {
                        if self.chans.len() >= MAX_SIGNALS {
                            let c = self.chans.remove(0);
                            self.retire(c);
                        }
                        self.start(f, true);
                    }
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (PskSetting::Modulation, Value::Choice(self.variant.index())),
            (PskSetting::Threshold, Value::F64(self.threshold)),
            (
                PskSetting::FocusHz,
                Value::Hz(self.focus_hz.unwrap_or(-1.0)),
            ),
            (PskSetting::Auto, Value::Bool(self.auto)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("psk")
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.trials.attach(ctx);
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = input.rate;
        self.dial = input.tuned_hz;
        self.reset();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let focus = self.focus_id();
        let chars = self.chars;
        for &s in x {
            self.ring.push(s);
            self.buf.push(s);
            self.t += 1;
            if self.buf.len() == NFFT {
                self.find();
                self.buf.drain(..NFFT / 2);
            }
            for c in &mut self.chans {
                match c.step(s) {
                    VariOut::Char(b) => {
                        let time = self.t as f64 / RATE;
                        c.last = time;
                        self.chars += 1;
                        let freq = self.dial + c.freq_hz;
                        let snr = c.snr_db();
                        self.heard.char(
                            &mut c.store,
                            out,
                            b,
                            time,
                            (c.quality.clamp(0.0, 1.0) * 100.0) as u8,
                            c.id,
                            freq,
                            snr,
                        );
                    }
                    VariOut::Bad => {
                        c.bad += 1;
                        self.bad += 1;
                    }
                    VariOut::None => {}
                }
            }
        }
        // Drop signals silent for 40 s (not the focus) and duplicates that drifted onto another.
        let now = self.t as f64 / RATE;
        let mut i = 0;
        while i < self.chans.len() {
            let c = &self.chans[i];
            let silent = now - c.last.max(c.first) > 40.0 && Some(c.id) != focus && !c.manual;
            let dup = self.chans[..i]
                .iter()
                .any(|o| (o.freq_hz - c.freq_hz).abs() < 0.4 * self.variant.baud());
            if silent || dup {
                let c = self.chans.remove(i);
                self.retire(c);
            } else {
                i += 1;
            }
        }
        self.trials.advance(x.len());
        self.watch(self.chars != chars);
        self.run_auto();
    }
    fn report(&mut self, res: &mut Published) {
        let focus = self.focus_id();
        let decoding = self.chans.iter().filter(|c| c.dcd).count();
        let state = if decoding > 0 {
            2
        } else if !self.peaks.is_empty() || !self.chans.is_empty() {
            1
        } else {
            0
        };
        let fc = focus.and_then(|id| self.chans.iter().find(|c| c.id == id));
        let strongest = self
            .chans
            .iter()
            .filter(|c| c.dcd)
            .max_by(|a, b| a.snr_db().total_cmp(&b.snr_db()));
        let st = &mut self.status;
        st.lock_state =
            [LockState::Searching, LockState::Tracking, LockState::Locked][state as usize];
        st.offset_hz = Some(fc.map_or(0.0, |c| c.freq_hz - c.found_hz));
        st.snr_db = fc.or(strongest).map(|c| c.snr_db());
        let name = self.variant.name();
        match (self.chans.is_empty(), fc) {
            (true, _) => put_text(&mut st.summary, format_args!("searching for {name}")),
            (false, None) => put_text(
                &mut st.summary,
                format_args!("{name}: {} signals, {decoding} decoding", self.chans.len()),
            ),
            (false, Some(c)) => put_text(
                &mut st.summary,
                format_args!(
                    "{name}: {} signals, {decoding} decoding; focus {:.0} Hz, {:.0} dB",
                    self.chans.len(),
                    c.freq_hz,
                    c.snr_db()
                ),
            ),
        }
        res.results_mut().set_status(st);
        for i in 0..self.gone.len() {
            self.signal_row(i, true, res);
        }
        for i in 0..self.chans.len() {
            self.signal_row(i, false, res);
        }
        let (chans, gone) = (&self.chans, &self.gone);
        retain_rows(res, &PSK_SIGNALS, &mut self.stale, |k| {
            chans.iter().chain(gone).any(|c| k.parse() == Ok(c.id))
        });
        // The window spectrum for the waterfall.
        let bin = self.bin_hz();
        let lo = (LO_HZ / bin).ceil() as usize;
        let hi = (HI_HZ / bin).floor() as usize;
        if self.spectrum.is_none() {
            self.spectrum = res.results().decl().series_id("spectrum");
        }
        if let Some(id) = self.spectrum {
            let held = &self.held;
            res.results_mut().update_series(id, |sp| {
                sp.line = true;
                sp.points.clear();
                for i in lo..=hi {
                    sp.points
                        .push([(i as f64 * bin) as f32, db(held[i] as f64) as f32]);
                }
                true
            });
        }
        self.seq += 1;
        res.put(&PskFields {
            modulation: self.variant,
            floor_db: db(self.floor as f64),
            focus: focus.map(i64::from),
            characters: self.chars as i64,
            errors: self.bad as i64,
            auto: self.auto,
            auto_locked: self.trials.locked(),
            auto_modulation: self.found.map(Variant::name),
        });
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        for c in &mut self.chans {
            c.store.clear();
        }
        self.gone.clear();
        self.heard.clear();
    }
    /// The signals are of the frequency before. Their texts stay as rows of gone signals if the stack keeps the
    /// table.
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if kept.table(PSK_SIGNALS.key) {
            self.gone.iter_mut().for_each(|g| g.past = true);
        } else {
            self.gone.clear();
        }
        self.heard.forget(kept);
    }
}

/// One PSK signal of the generator.
#[derive(Clone, Debug)]
pub struct PskSignalSpec {
    /// Audio frequency, Hz.
    pub freq_hz: f64,
    /// Variant.
    pub variant: Variant,
    /// The text (repeated).
    pub text: String,
    /// Amplitude.
    pub amp: f64,
    /// Seconds before it starts.
    pub start_s: f64,
    /// Idle reversals before and after the text, seconds.
    pub idle_s: f64,
    /// Carrier off between repeats, seconds.
    pub pause_s: f64,
    /// Drift, Hz per minute.
    pub drift_hz_min: f64,
    /// Symbol clock error, ppm.
    pub clock_ppm: f64,
    /// Start phase, radians.
    pub phase: f64,
}

impl PskSignalSpec {
    /// A signal with the usual timing (1 s idle, 2 s pause), amplitude 1.
    pub fn new(freq_hz: f64, variant: Variant, text: &str) -> Self {
        PskSignalSpec {
            freq_hz,
            variant,
            text: text.into(),
            amp: 1.0,
            start_s: 0.0,
            idle_s: 1.0,
            pause_s: 2.0,
            drift_hz_min: 0.0,
            clock_ppm: 0.0,
            phase: 0.0,
        }
    }
}

struct PskTx {
    s: PskSignalSpec,
    /// Phasors of the symbols of one repeat (0 = carrier off).
    syms: Vec<Complex32>,
    t: f64,
    carrier: f64,
}

impl PskTx {
    fn new(s: PskSignalSpec) -> Self {
        let baud = s.variant.baud();
        let idle = (s.idle_s * baud).round() as usize;
        let pause = (s.pause_s * baud).round() as usize;
        let mut bits: Vec<bool> = vec![];
        for b in s.text.bytes() {
            let _ = varicode::encode(&[b], &mut |x| bits.push(x == 1));
        }
        let mut syms = vec![];
        let mut ph = Complex32::new(1.0, 0.0);
        let mut enc = Encoder::new(QPSK_K5);
        let mut push_bit = |bit: bool, syms: &mut Vec<Complex32>, ph: &mut Complex32| {
            if s.variant.qpsk() {
                let sym = enc.push_bit(bit as u8) as u32;
                let turns = qpsk_turns(sym);
                *ph *= Complex32::new(0.0, 1.0).powu(turns);
            } else if !bit {
                *ph = -*ph;
            }
            syms.push(*ph);
        };
        for _ in 0..idle {
            push_bit(false, &mut syms, &mut ph);
        }
        for &b in &bits {
            push_bit(b, &mut syms, &mut ph);
        }
        // Flush the code, then idle.
        for _ in 0..idle {
            push_bit(false, &mut syms, &mut ph);
        }
        syms.extend(std::iter::repeat_n(Complex32::default(), pause.max(1)));
        PskTx {
            s,
            syms,
            t: 0.0,
            carrier: 0.0,
        }
    }

    fn sample(&mut self, rate: f64) -> Complex32 {
        let t = self.t;
        self.t += 1.0 / rate;
        if t < self.s.start_s {
            return Complex32::default();
        }
        let tt = t - self.s.start_s;
        let f = self.s.freq_hz + self.s.drift_hz_min * tt / 60.0;
        self.carrier += f / rate;
        self.carrier -= self.carrier.floor();
        let ts = tt * self.s.variant.baud() * (1.0 + self.s.clock_ppm * 1e-6);
        let k = ts.floor() as usize;
        let frac = (ts - ts.floor()) as f32;
        let n = self.syms.len();
        let a = self.syms[(k + n - 1) % n];
        let b = self.syms[k % n];
        // Before the first symbol the carrier is off.
        let a = if k == 0 { Complex32::default() } else { a };
        let w = 0.5 * (1.0 + (PI * frac).cos());
        let env = a * w + b * (1.0 - w);
        let p = std::f64::consts::TAU * self.carrier + self.s.phase;
        env * Complex32::new(p.cos() as f32, p.sin() as f32) * self.s.amp as f32
    }
}

/// The PSK generator: any number of signals.
pub struct PskModulator {
    rate: f64,
    tx: Vec<PskTx>,
}

impl PskModulator {
    /// Signals at `rate`.
    pub fn new(rate: f64, signals: Vec<PskSignalSpec>) -> Self {
        PskModulator {
            rate,
            tx: signals.into_iter().map(PskTx::new).collect(),
        }
    }
}

impl Modulator for PskModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out {
            let mut v = Complex32::default();
            for t in &mut self.tx {
                v += t.sample(self.rate);
            }
            *o = v;
        }
    }
}

/// The demo band: four signals (700, 1050, 1500, 2200 Hz. One drifting 3 Hz/min, one with an 80 ppm symbol clock).
pub fn demo_band(variant: Variant) -> Vec<PskSignalSpec> {
    let mut v = vec![
        PskSignalSpec::new(700.0, variant, "CQ CQ CQ DE LY2ABC LY2ABC PSE K\n"),
        PskSignalSpec::new(1050.0, variant, "TEST DE K0TST K0TST 73\n"),
        PskSignalSpec::new(1500.0, variant, "QRZ? DE DL1XYZ DL1XYZ K\n"),
        PskSignalSpec::new(2200.0, variant, "CQ DX DE N0XYZ N0XYZ K\n"),
    ];
    v[1].drift_hz_min = 3.0;
    v[1].amp = 0.48;
    v[2].clock_ppm = 80.0;
    v[2].amp = 0.4;
    v[3].amp = 0.6;
    v[3].start_s = 0.7;
    v
}

/// The generator. It reads the settings `modulation`, `gen_message` and `gen_freq_hz`. Without `gen_message` it sends the demo
/// band.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = cfg.settings.get(&MODULATION_SETTING);
    let text = Some(cfg.settings.get(&GEN_MESSAGE)).filter(|t| !t.is_empty());
    let signals = match text {
        Some(t) => vec![PskSignalSpec::new(cfg.settings.get(&GEN_FREQ_HZ), v, t)],
        None => demo_band(v),
    };
    // Raised-cosine shaping of reversals: the mean power is 3/4 of the peak.
    let p: f64 = signals.iter().map(|s| s.amp * s.amp * 0.75).sum();
    let baud = v.baud();
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        PskModulator::new(cfg.rate_hz, signals),
        SignalInfo::new(2.0 * baud)
            .with_symbol_rate(baud)
            .with_bit_rate(baud)
            .with_power(p),
    ))
}

/// The audio frequency of the PSK sender.
pub const TX_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("tx_centre_hz", "Audio frequency", "Hz", 1000.0)
        .range(200.0, 3000.0, 1.0)
        .section(crate::sender::TX_SECTION)
        .help("The audio frequency of the carrier sent.");

sdrkit_core::settings! {
    /// The settings of the PSK sender.
    pub enum PskTxSetting {
        /// The variant.
        Modulation = MODULATION_SETTING,
        /// The audio frequency.
        CentreHz = TX_CENTRE_HZ,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

/// The PSK coder. Each bit or code symbol is one element of the PSK31 pulse: the carrier state goes from the last one to the
/// new one by a raised cosine over the symbol, so a reversal passes through zero at mid symbol. BPSK: bit 0 reverses the
/// phase, bit 1 keeps it. QPSK: each bit enters the K = 5 code and its symbol turns the phase by [`qpsk_turns`]. A burst:
/// the carrier rises from zero over the first symbol of a preamble of max(32, baud) zero bits (reversals), each character's
/// varicode with two zero bits after it, a postamble of ones (a steady carrier, the end the designer's description gives:
/// max(16, preamble / 2) bits, for QPSK max(48, preamble / 2) and 8 more), and the carrier falls to zero over one symbol.
pub struct PskCoder {
    variant: Variant,
    hz: f64,
    z: Complex32,
    enc: Encoder,
}

impl PskCoder {
    /// A coder of `variant` at the audio frequency `hz`.
    pub fn new(variant: Variant, hz: f64) -> Self {
        PskCoder {
            variant,
            hz,
            z: Complex32::new(1.0, 0.0),
            enc: Encoder::new(QPSK_K5),
        }
    }
    /// The preamble, bits.
    pub fn preamble(&self) -> usize {
        (self.variant.baud().round() as usize).max(32)
    }
    /// The postamble, bits.
    pub fn postamble(&self) -> usize {
        let half = self.preamble() / 2;
        if self.variant.qpsk() {
            half.max(48) + 8
        } else {
            half.max(16)
        }
    }
    fn element(&self, from: Complex32, to: Complex32) -> crate::sender::Element {
        crate::sender::Element {
            hz: self.hz,
            s: 1.0 / self.variant.baud(),
            env: crate::sender::Envelope::Cosine(from, to),
        }
    }
    fn bit(&mut self, bit: bool, out: &mut Vec<crate::sender::Element>) {
        let from = self.z;
        if self.variant.qpsk() {
            let sym = u32::from(self.enc.push_bit(u8::from(bit)));
            self.z *= Complex32::new(0.0, 1.0).powu(qpsk_turns(sym));
        } else if !bit {
            self.z = -self.z;
        }
        out.push(self.element(from, self.z));
    }
}

impl crate::sender::Coder for PskCoder {
    fn begin(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.z = Complex32::new(1.0, 0.0);
        self.enc = Encoder::new(QPSK_K5);
        out.push(self.element(Complex32::default(), self.z));
        for _ in 0..self.preamble() {
            self.bit(false, out);
        }
    }
    fn char(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        let Ok(code) = u8::try_from(c)
            .map_err(|_| ())
            .and_then(|b| varicode::code(b).map_err(|_| ()))
        else {
            return;
        };
        for b in code.iter() {
            self.bit(b == 1, out);
        }
        self.bit(false, out);
        self.bit(false, out);
    }
    fn end(&mut self, out: &mut Vec<crate::sender::Element>) {
        for _ in 0..self.postamble() {
            self.bit(true, out);
        }
        out.push(self.element(self.z, Complex32::default()));
    }
}

/// The PSK side of the sender.
#[derive(Clone, Debug)]
pub struct PskModem {
    variant: Variant,
    centre: f64,
    common: crate::sender::Common,
}

impl Default for PskModem {
    fn default() -> Self {
        PskModem {
            variant: MODULATION_SETTING.default,
            centre: TX_CENTRE_HZ.default,
            common: crate::sender::Common::default(),
        }
    }
}

impl crate::sender::Modem for PskModem {
    const NAME: &'static str = "hfkbd.psktx";
    fn schema() -> Arc<Schema> {
        PskTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match PskTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PskTxSetting::Modulation => self.variant = MODULATION_SETTING.read(id, v)?,
            PskTxSetting::CentreHz => self.centre = TX_CENTRE_HZ.read(id, v)?,
            PskTxSetting::Level => self.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            PskTxSetting::TuneLimit => {
                self.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (
                PskTxSetting::Modulation,
                Value::Choice(self.variant.index()),
            ),
            (PskTxSetting::CentreHz, Value::Hz(self.centre)),
            (PskTxSetting::Level, Value::F64(self.common.level)),
            (PskTxSetting::TuneLimit, Value::Duration(self.common.tune_s)),
        ])
    }
    fn common(&self) -> crate::sender::Common {
        self.common
    }
    /// Bytes 0 to 127 as typed; CR LF in the text is one line end (LF). Any other character is refused.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        let start = out.len();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\r' && chars.peek() == Some(&'\n') {
                continue;
            }
            if !c.is_ascii() {
                out.truncate(start);
                return Err(crate::sender::TxError::Character(c));
            }
            out.push(c);
        }
        Ok(())
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        Ok(crate::sender::Coding::Elements(Box::new(PskCoder::new(
            self.variant,
            self.centre,
        ))))
    }
    fn centre_hz(&self) -> f64 {
        self.centre
    }
    /// The carrier and twice the baud each side.
    fn band(&self) -> (f64, f64) {
        let b = 2.0 * self.variant.baud();
        (self.centre - b, self.centre + b)
    }
}

/// The PSK sender component.
pub fn sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<PskModem>::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qpsk_map_round_trips() {
        for s in 0..4 {
            assert_eq!(qpsk_symbol(qpsk_turns(s)), s);
        }
        assert_eq!(qpsk_turns(0), 2);
        assert_eq!(qpsk_turns(1), 3);
        assert_eq!(qpsk_turns(2), 0);
        assert_eq!(qpsk_turns(3), 1);
    }

    /// The bit errors of `got` against `want`, with the best delay for each block of 256 bits, and the bits compared. The first
    /// 512 bits are left out.
    fn bit_errors(got: &[bool], want: &[bool]) -> (usize, usize) {
        let (mut errors, mut bits) = (0, 0);
        let mut i = 512;
        while i + 256 + 8 < got.len().min(want.len()) {
            errors += (-24i64..=6)
                .map(|lag| {
                    (0..256)
                        .filter(|&k| got[i + k] != want[(i as i64 + k as i64 + lag) as usize])
                        .count()
                })
                .min()
                .unwrap_or(256);
            bits += 256;
            i += 256;
        }
        (errors, bits)
    }

    /// BPSK31 with random bits at Eb/N0 = 6 dB (random bits have 0.75 of the carrier power), a clock 50 ppm fast and the
    /// channel 1.5 Hz off. Coherent detection with differential decoding has a symbol error rate of Q(sqrt(2 Eb/N0)) and two
    /// bit errors for each: 0.48 % of the bits at 6 dB, 0.64 % at 5.7 dB. Differential detection has 0.93 %. The count over
    /// five seeds stays under that of 5.7 dB: the receiver is within 0.3 dB of coherent detection.
    #[test]
    fn bpsk31_bit_errors_at_6_db_are_those_of_coherent_detection() {
        const SYMBOLS: usize = 20_000;
        let per_symbol = (RATE / 31.25) as usize;
        let (mut errors, mut compared) = (0, 0);
        for seed in 1..=5u64 {
            let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
            let bits: Vec<bool> = (0..SYMBOLS).map(|_| rng.next_u64() & 1 == 1).collect();
            let mut phasor = Complex32::new(1.0, 0.0);
            let syms: Vec<Complex32> = bits
                .iter()
                .map(|&b| {
                    if !b {
                        phasor = -phasor;
                    }
                    phasor
                })
                .collect();
            let mut spec = PskSignalSpec::new(1000.0, Variant::Bpsk31, "");
            spec.clock_ppm = 50.0;
            let mut tx = PskTx {
                s: spec,
                syms,
                t: 0.0,
                carrier: 0.0,
            };
            let noise = 0.75 / 10f64.powf(0.6) * (RATE / 31.25);
            let mut ch = PskChannel::new(1, Variant::Bpsk31, 1001.5, 0);
            for _ in 0..SYMBOLS * per_symbol {
                ch.step(tx.sample(RATE) + rng.complex_normal(noise));
            }
            let (e, n) = bit_errors(&ch.bits, &bits);
            errors += e;
            compared += n;
        }
        assert!(compared > 90_000, "{compared} bits");
        let limit = 0.006_42 * compared as f64;
        assert!(
            (errors as f64) < limit,
            "{errors} errors in {compared} bits, limit {limit:.0}"
        );
    }

    /// The start of a burst is found within two symbols behind its first sample; a carrier over the whole stretch has no start.
    #[test]
    fn burst_start_is_where_the_carrier_starts() {
        let mut spec = PskSignalSpec::new(1000.0, Variant::Bpsk31, "CQ\n");
        spec.start_s = 0.9;
        let mut tx = PskTx::new(spec);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        // 10 dB in 2.5 kHz.
        let noise = 0.75 / 10.0 * (RATE / 2500.0);
        let x: Vec<Complex32> = (0..(1.5 * RATE) as usize)
            .map(|_| tx.sample(RATE) + rng.complex_normal(noise))
            .collect();
        let at = burst_start(&x, 1000.0, 31.25);
        let first = (0.9 * RATE) as usize;
        assert!(
            at <= first + 2 * 768 && at + 768 >= first,
            "{at} vs {first}"
        );
        assert_eq!(burst_start(&x[first + 4 * 768..], 1000.0, 31.25), 0);
    }
}

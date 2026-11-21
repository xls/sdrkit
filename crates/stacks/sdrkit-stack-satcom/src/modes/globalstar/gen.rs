//! The Globalstar test signal: band-limited noise 1.23 MHz wide (the look of a CDMA terminal) on a channel, with a Doppler curve.
//! It is the clean signal at unit power for each terminal. Noise, offset and clock error come from the standard `gen_*` settings.

use super::{channel_hz, nearest_channel, BAND_CENTRE_HZ, CHANNELS};
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};

sdrkit_core::choice_enum! {
    /// The scene of the test signal.
    pub enum Scenario {
        /// A pass.
        Pass = ("pass", "A pass"),
        /// One steady terminal.
        Steady = ("steady", "Steady"),
        /// No terminal.
        Silence = ("silence", "Silence"),
    }
}

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The scene of the test signal.
pub const GEN_SCENARIO: Setting<field::Choice<Scenario>> = Setting::choice("gen_scenario", "Scenario", Scenario::Pass)
    .help("A pass: one channel sweeps from +30 to -30 kHz in 40 s, and the next channel sends 300 ms bursts every 800 ms. Steady: one channel at a constant frequency. Silence: no terminal.")
    .section(TEST_SIGNAL)
    .advanced();
/// The channel of the test signal (0: the channel nearest the centre).
pub const GEN_CHANNEL: Setting<field::I64> = Setting::new("gen_channel", "Channel", "", 0)
    .range(0.0, 13.0, 1.0)
    .help("0: the channel nearest the centre.")
    .section(TEST_SIGNAL)
    .advanced();
/// How much faster than real time a pass plays.
pub const GEN_TIME_SCALE: Setting<field::F64> =
    Setting::new("gen_time_scale", "Time scale", "", 1.0)
        .range(1.0, 100.0, 1.0)
        .help("10 plays a pass in a tenth of the time.")
        .section(TEST_SIGNAL)
        .advanced();
/// Where the radio is tuned.
pub const GEN_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("gen_centre_hz", "Radio centre", "Hz", BAND_CENTRE_HZ)
        .range(1600e6, 1640e6, 1e3)
        .help("Where the radio is tuned, so the generator puts the channels in the right place.")
        .section(TEST_SIGNAL)
        .advanced();

/// What the test signal holds.
#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    /// 0 a pass, 1 steady, 2 silence.
    pub scenario: u8,
    /// 0: the channel nearest the centre.
    pub channel: usize,
    pub time_scale: f64,
    pub centre_hz: f64,
    pub seed: u64,
}

impl GenParams {
    /// A pass at real speed around the band centre.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            scenario: 0,
            channel: 0,
            time_scale: 1.0,
            centre_hz: BAND_CENTRE_HZ,
            seed: 1,
        }
    }
}

/// Band-limited noise of one terminal at unit power: white noise at 2 Msps through a low-pass of +-615 kHz, resampled to the output
/// rate.
struct Terminal {
    rng: Rng,
    fir: Fir<Complex32>,
    rs: Resampler<Complex32>,
    work: Vec<Complex32>,
    out: Vec<Complex32>,
    pos: usize,
    gain: f32,
}

const WORK_RATE: f64 = 2e6;

impl Terminal {
    fn new(rate: f64, seed: u64) -> Self {
        let taps = lowpass(WORK_RATE, 600e3, 640e3, 50.0).expect("the filter settings are valid");
        // The filter passes this share of white noise power.
        let pass = taps.iter().map(|t| (*t as f64).powi(2)).sum::<f64>();
        Terminal {
            rng: Rng::new(seed),
            fir: Fir::new(&taps),
            rs: Resampler::new(WORK_RATE, rate).expect("a positive rate"),
            work: Vec::with_capacity(1024),
            out: Vec::new(),
            pos: 0,
            gain: (1.0 / pass).sqrt() as f32,
        }
    }
    fn next(&mut self) -> Complex32 {
        while self.pos >= self.out.len() {
            self.work.clear();
            for _ in 0..1024 {
                let x = self.rng.complex_normal(1.0);
                self.work.push(self.fir.process_sample(x));
            }
            self.out.clear();
            self.rs.process(&self.work, &mut self.out);
            self.pos = 0;
        }
        self.pos += 1;
        self.out[self.pos - 1] * self.gain
    }
}

/// The Globalstar test signal.
pub struct GsGenerator {
    p: GenParams,
    main: Terminal,
    second: Terminal,
    main_nco: Nco,
    second_nco: Nco,
    main_off: f64,
    n: u64,
}

/// The pass's Doppler at signal time `t` (seconds), for a time scale.
pub fn pass_doppler(t: f64, scale: f64) -> f64 {
    let leg = 40.0 / scale;
    let x = (t % leg) / leg;
    30e3 - 60e3 * x
}

impl GsGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.scenario = match s.get(&GEN_SCENARIO) {
            Scenario::Pass => 0,
            Scenario::Steady => 1,
            Scenario::Silence => 2,
        };
        p.channel = usize::try_from(s.get(&GEN_CHANNEL)).unwrap_or(0);
        p.time_scale = s.get(&GEN_TIME_SCALE).max(1.0);
        p.centre_hz = s.get(&GEN_CENTRE_HZ);
        p.seed = cfg.seed;
        Self::new(p)
    }

    /// The generator of these parameters.
    pub fn new(p: GenParams) -> Self {
        let ch = if p.channel == 0 {
            nearest_channel(p.centre_hz)
        } else {
            p.channel.min(CHANNELS)
        };
        let ch2 = if ch < CHANNELS { ch + 1 } else { ch - 1 };
        let main_off = channel_hz(ch) - p.centre_hz;
        GsGenerator {
            main: Terminal::new(p.rate, p.seed ^ 0x1111),
            second: Terminal::new(p.rate, p.seed ^ 0x2222),
            main_nco: Nco::new(p.rate, main_off),
            second_nco: Nco::new(p.rate, channel_hz(ch2) - p.centre_hz),
            main_off,
            n: 0,
            p,
        }
    }

    /// The main channel.
    pub fn channel(&self) -> usize {
        nearest_channel(self.main_off + self.p.centre_hz)
    }

    /// The main terminal's Doppler at signal time `t`.
    pub fn doppler_at(&self, t: f64) -> f64 {
        match self.p.scenario {
            0 => pass_doppler(t, self.p.time_scale),
            _ => 0.0,
        }
    }
}

impl Generator for GsGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let dt = 1.0 / self.p.rate;
        for o in out.iter_mut() {
            let t = self.n as f64 * dt;
            let mut v = Complex32::new(0.0, 0.0);
            if self.p.scenario != 2 {
                if self.p.scenario == 0 && self.n.is_multiple_of(64) {
                    self.main_nco.set_freq(self.main_off + self.doppler_at(t));
                }
                let s = self.main.next();
                v += s * self.main_nco.next_sample();
            }
            if self.p.scenario == 0 {
                let s = self.second.next();
                let c = self.second_nco.next_sample();
                if t % 0.8 < 0.3 {
                    v += s * c;
                }
            }
            *o = v;
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Each terminal is at unit power while it sends. The SNR is stated in one channel of 1.23 MHz.
        Some(SignalInfo::new(1.23e6))
    }
}

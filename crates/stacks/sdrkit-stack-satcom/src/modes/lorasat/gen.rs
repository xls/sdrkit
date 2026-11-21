//! The LoRa satellite test signal: beacon frames of a pass, with the carrier on the straight-line pass model. Noise and the
//! radio's errors come from the standard `gen_*` settings.

use super::{pass_rate, pass_shift, LoraBw, Sf};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Complex32, GenConfig, Generator, Setting, SignalInfo};
use sdrkit_proto_lora_phy::{LoraParams, TxFrame};

/// A pass the test signal can make.
#[derive(Clone, Copy, Debug)]
pub struct Pass {
    /// Its setting name.
    pub name: &'static str,
    /// The carrier, Hz.
    pub freq: f64,
    /// Spreading factor.
    pub sf: u8,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// Coding rate denominator (4/cr).
    pub cr: u8,
    /// Preamble symbols.
    pub preamble: u16,
}

/// The passes the test signal can make.
pub const PASSES: &[Pass] = &[
    Pass {
        name: "436.8",
        freq: 436.8e6,
        sf: 11,
        bw: 125e3,
        cr: 8,
        preamble: 8,
    },
    Pass {
        name: "868.1",
        freq: 868.1e6,
        sf: 10,
        bw: 125e3,
        cr: 5,
        preamble: 8,
    },
    Pass {
        name: "137.055",
        freq: 137.055e6,
        sf: 8,
        bw: 125e3 / 3.0,
        cr: 5,
        preamble: 15,
    },
];

sdrkit_core::choice_enum! {
    /// The pass of the test signal.
    pub enum GenBand {
        /// 436.8 MHz, SF11.
        P436 = ("436.8", "436.8 MHz, SF11"),
        /// 868.1 MHz, SF10.
        P868 = ("868.1", "868.1 MHz, SF10"),
        /// 137.055 MHz, SF8, 41.7 kHz.
        P137 = ("137.055", "137.055 MHz, SF8, 41.7 kHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The spreading factor of the test signal.
    pub enum GenSf {
        /// The pass's.
        Pass = ("0", "The pass's"),
        /// SF7.
        Sf7 = ("7", "7"),
        /// SF8.
        Sf8 = ("8", "8"),
        /// SF9.
        Sf9 = ("9", "9"),
        /// SF10.
        Sf10 = ("10", "10"),
        /// SF11.
        Sf11 = ("11", "11"),
        /// SF12.
        Sf12 = ("12", "12"),
    }
}

sdrkit_core::choice_enum! {
    /// The bandwidth of the test signal.
    pub enum GenBw {
        /// The pass's.
        Pass = ("0", "The pass's"),
        /// 41.7 kHz.
        B41k7 = ("41.7", "41.7 kHz"),
        /// 62.5 kHz.
        B62k5 = ("62.5", "62.5 kHz"),
        /// 125 kHz.
        B125k = ("125", "125 kHz"),
        /// 250 kHz.
        B250k = ("250", "250 kHz"),
        /// 500 kHz.
        B500k = ("500", "500 kHz"),
    }
}

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The pass of the test signal.
pub const GEN_BAND: Setting<field::Choice<GenBand>> =
    Setting::choice("gen_band", "Pass", GenBand::P436)
        .section(TEST_SIGNAL)
        .advanced();
/// The spreading factor of the test signal.
pub const GEN_SF: Setting<field::Choice<GenSf>> = Setting::choice("gen_sf", "SF", GenSf::Pass)
    .section(TEST_SIGNAL)
    .advanced();
/// The bandwidth of the test signal.
pub const GEN_BW_KHZ: Setting<field::Choice<GenBw>> =
    Setting::choice("gen_bw_khz", "Bandwidth, kHz", GenBw::Pass)
        .section(TEST_SIGNAL)
        .advanced();
/// The test signal has the Doppler of the pass.
pub const GEN_PASS_DOPPLER: Setting<field::Bool> =
    Setting::new("gen_pass_doppler", "Doppler of the pass", "", true)
        .section(TEST_SIGNAL)
        .advanced();
/// Where in the pass the test signal starts.
pub const GEN_START_S: Setting<field::Duration> =
    Setting::new("gen_start_s", "Seconds into the pass", "s", 0.0)
        .range(0.0, 600.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The sync word of the test signal.
pub const GEN_SYNC: Setting<field::I64> = Setting::new("gen_sync", "Sync word", "", 18)
    .range(0.0, 255.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();

/// What the test signal sends.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Sample rate, Hz.
    pub rate: f64,
    /// The carrier frequency, Hz.
    pub freq: f64,
    /// Where a receiver puts its centre, Hz (the dial plus the stack's tune offset). The generator does not use it: its 0 Hz is
    /// the carrier frequency.
    pub centre: f64,
    /// Spreading factor.
    pub sf: u8,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// Coding rate denominator (4/cr).
    pub cr: u8,
    /// Preamble symbols.
    pub preamble: u16,
    /// Sync word.
    pub sync: u8,
    /// The carrier follows the pass.
    pub doppler: bool,
    /// The carrier shifts with the pass, but stays fixed during each frame.
    pub no_rate: bool,
    /// Seconds into the pass at the start.
    pub start_s: f64,
    /// Seconds between beacons.
    pub period_s: f64,
    /// The time of closest approach in the pass, seconds.
    pub tca_s: f64,
}

impl GenParams {
    /// A pass at `freq` with its usual LoRa setting.
    pub fn new(rate: f64, freq: f64) -> Self {
        let (sf, bw, cr, pre) = PASSES
            .iter()
            .find(|p| (freq - p.freq).abs() < 1e6)
            .map_or((11, 125e3, 8, 8), |p| (p.sf, p.bw, p.cr, p.preamble));
        GenParams {
            rate,
            freq,
            centre: freq + super::DESCRIPTOR.tuning.tune_offset_hz,
            sf,
            bw,
            cr,
            preamble: pre,
            sync: 0x12,
            doppler: true,
            no_rate: false,
            start_s: 0.0,
            period_s: 6.0,
            tca_s: 300.0,
        }
    }
    /// The LoRa parameters of the beacons.
    pub fn params(&self) -> LoraParams {
        LoraParams::new(self.sf, self.bw, self.cr, self.preamble, self.sync)
    }
}

/// The beacon text of frame `k`.
pub fn beacon(k: u64) -> String {
    format!("SDRKIT TEST BEACON {k:05}")
}

/// The test signal generator.
pub struct LoraSatGenerator {
    p: GenParams,
    frames: Vec<TxFrame>,
    next: u64,
    n: u64,
}

impl LoraSatGenerator {
    /// The generator of a stack configuration. Settings the schema refused never arrive here, so a missing one is its default.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let band = usize::from(s.get(&GEN_BAND).index());
        let f = PASSES.get(band).map_or(PASSES[0].freq, |p| p.freq);
        let mut p = GenParams::new(cfg.rate_hz, f);
        let sf = s.get(&GEN_SF).index();
        if sf > 0 {
            p.sf = Sf::from_index(sf - 1).map_or(p.sf, Sf::value);
        }
        let bw = s.get(&GEN_BW_KHZ).index();
        if bw > 0 {
            p.bw = LoraBw::from_index(bw - 1).map_or(p.bw, LoraBw::hz);
        }
        p.doppler = s.get(&GEN_PASS_DOPPLER);
        p.start_s = s.get(&GEN_START_S);
        p.sync = u8::try_from(s.get(&GEN_SYNC)).unwrap_or(0x12);
        Self::new(p)
    }

    /// The generator of these parameters.
    pub fn new(p: GenParams) -> Self {
        LoraSatGenerator {
            frames: Vec::new(),
            next: 0,
            n: 0,
            p,
        }
    }

    /// The pass time of output time `t`.
    pub fn pass_time(&self, t: f64) -> f64 {
        (self.p.start_s + t) % 600.0
    }

    /// The carrier's Doppler shift at output time `t` (0 with Doppler off).
    pub fn shift(&self, t: f64) -> f64 {
        if self.p.doppler {
            pass_shift(self.p.freq, self.pass_time(t), self.p.tca_s)
        } else {
            0.0
        }
    }

    /// The start time of beacon `k`.
    pub fn frame_start(&self, k: u64) -> f64 {
        0.3 + k as f64 * self.p.period_s
    }

    fn frame(&self, k: u64) -> Option<TxFrame> {
        let t0 = self.frame_start(k);
        let mut f = TxFrame::new(&self.p.params(), beacon(k).as_bytes())
            .ok()?
            .at(t0)
            .amplitude(1.0);
        let dur = f.duration_s();
        let shift = self.shift(t0);
        let rate = if self.p.doppler && !self.p.no_rate {
            pass_rate(self.p.freq, self.pass_time(t0 + dur / 2.0), self.p.tca_s)
        } else {
            0.0
        };
        f.carrier_hz = shift;
        f.drift_hz_s = rate;
        // The Doppler compresses the symbol clock too.
        f.ppm = -shift / self.p.freq * 1e6;
        f.phase = (k as f64 * 1.7) % std::f64::consts::TAU;
        Some(f)
    }
}

impl Generator for LoraSatGenerator {
    /// The dial frequency is at 0 Hz.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_end = (self.n + out.len() as u64) as f64 / self.p.rate;
        while self.frame_start(self.next) < t_end {
            if let Some(f) = self.frame(self.next) {
                self.frames.push(f);
            }
            self.next += 1;
        }
        out.fill(Complex32::default());
        for f in &self.frames {
            f.add_into(out, self.p.rate, self.n);
        }
        self.frames.retain(|f| f.end_s() >= t_end);
        self.n += out.len() as u64;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let sym = self.p.bw / (1u64 << self.p.sf) as f64;
        let bits = sym * self.p.sf as f64 * 4.0 / self.p.cr as f64;
        Some(
            SignalInfo::new(self.p.bw)
                .with_symbol_rate(sym)
                .with_bit_rate(bits),
        )
    }
}

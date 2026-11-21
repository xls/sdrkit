//! The LBJ test signal: POCSAG bursts as continuous-phase FSK, bit 1 the lower frequency, at unit mean power.

use super::message::pocsag_config;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::SimError;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use sdrkit_proto_pager::pocsag::{
    encode_batches, transmission_bits, Content, PocsagTx, PREAMBLE_BITS,
};

/// The bit rate, bit/s.
pub const BIT_RATE: f64 = 1200.0;
/// The deviation the generator uses unless told otherwise, Hz.
pub const DEVIATION_HZ: f64 = 4200.0;
/// The range of the deviation setting, Hz.
pub const DEVIATION_RANGE: (f64, f64) = (1000.0, 8000.0);

/// A page to send.
#[derive(Clone, Debug)]
pub struct TxPage {
    /// Address.
    pub ric: u32,
    /// Function bits.
    pub function: u32,
    /// Numeric text.
    pub text: String,
}

/// The options of the generator.
#[derive(Clone, Debug)]
pub struct LbjGenOptions {
    /// Deviation, Hz.
    pub deviation_hz: f64,
    /// The channel relative to the radio centre, Hz.
    pub channel_hz: f64,
    /// Silence before the first burst, seconds.
    pub lead_s: f64,
    /// Silence between bursts, seconds.
    pub gap_s: f64,
    /// Repeat the bursts for ever.
    pub repeat: bool,
}

impl Default for LbjGenOptions {
    fn default() -> Self {
        LbjGenOptions {
            deviation_hz: DEVIATION_HZ,
            channel_hz: -50e3,
            lead_s: 0.3,
            gap_s: 0.5,
            repeat: true,
        }
    }
}

impl LbjGenOptions {
    /// The options that the test signal settings of `cfg` give.
    pub fn from_config(cfg: &GenConfig) -> Self {
        LbjGenOptions {
            deviation_hz: cfg.settings.get(&GEN_DEVIATION_HZ),
            ..LbjGenOptions::default()
        }
    }
}

/// The deviation of the test signal.
pub const GEN_DEVIATION_HZ: Setting<field::Hz> = Setting::new(
    "gen_deviation_hz",
    "Test signal deviation",
    "Hz",
    DEVIATION_HZ,
)
.range(DEVIATION_RANGE.0, DEVIATION_RANGE.1, 10.0)
.section("Test signal")
.advanced();

/// The test signal settings of the stack. The framework adds the standard `gen_*` settings (noise, offsets, impairments).
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_DEVIATION_HZ];

/// The basic message text: the train left-aligned in 5, a space, the speed right-aligned in 3, a space, the kilometre post
/// (0.1 km) right-aligned in 5.
pub fn basic_text(train: &str, speed: u32, km_tenths: i32) -> String {
    format!("{train:<5} {speed:>3} {km_tenths:>5}")
}

/// The demo loop: four basic messages.
pub fn demo_bursts() -> Vec<Vec<TxPage>> {
    let m = |train: &str, speed, km, up: bool| {
        vec![TxPage {
            ric: 1234000,
            function: if up { 3 } else { 1 },
            text: basic_text(train, speed, km),
        }]
    };
    vec![
        m("19533", 67, 382, false),
        m("10482", 112, 12046, true),
        m("23017", 0, 50, false),
        m("41206", 86, 3119, true),
    ]
}

/// The bits of one burst: a 576-bit preamble, then batches of the sync word and 16 code words. A page whose text holds a
/// character outside the numeric set is left out.
pub fn burst_bits(pages: &[TxPage]) -> Vec<u8> {
    let set = pocsag_config().numeric;
    let tx: Vec<PocsagTx> = pages
        .iter()
        .map(|p| PocsagTx {
            ric: p.ric,
            function: (p.function & 3) as u8,
            content: Content::Numeric(&p.text),
        })
        .filter(|t| encode_batches(std::slice::from_ref(t), set, &mut Vec::new()).is_ok())
        .collect();
    let mut words = Vec::new();
    if encode_batches(&tx, set, &mut words).is_err() {
        words.clear();
    }
    let mut bits = Vec::new();
    transmission_bits(&words, PREAMBLE_BITS, &mut bits);
    bits
}

/// What the generator makes next.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Segment {
    /// This many samples of silence.
    Quiet(usize),
    /// Burst `i` of the list.
    Burst(usize),
    /// Nothing more.
    End,
}

/// The LBJ generator.
pub struct LbjGenerator {
    rate: f64,
    opt: LbjGenOptions,
    bursts: Vec<Vec<u8>>,
    /// None for a generator that makes nothing (a rate that the modulator cannot serve).
    modulator: Option<Cpm>,
    osc: Nco,
    /// The burst being sent and the next sample of it.
    samples: Vec<Complex32>,
    at: usize,
    /// Silence left before the next segment.
    quiet: usize,
    next: Segment,
    /// The burst sent last.
    last: Option<usize>,
}

impl LbjGenerator {
    /// A generator of these bursts at `rate`. An error for a rate or a deviation the modulator refuses.
    pub fn new(rate: f64, bursts: Vec<Vec<TxPage>>, opt: LbjGenOptions) -> Result<Self, SimError> {
        let modulator =
            Cpm::cpfsk(2.0 * opt.deviation_hz / BIT_RATE, rate / BIT_RATE)?.with_held_edges();
        let lead = (opt.lead_s * rate).round() as usize;
        Ok(LbjGenerator {
            rate,
            bursts: bursts.iter().map(|b| burst_bits(b)).collect(),
            modulator: Some(modulator),
            osc: Nco::new(rate, opt.channel_hz),
            opt,
            samples: Vec::new(),
            at: 0,
            quiet: 0,
            next: Segment::Quiet(lead),
            last: None,
        })
    }

    /// The demo loop of the test signal settings in `cfg`.
    ///
    /// # Errors
    ///
    /// The [`SimError`] of a rate that the modulator cannot serve.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, SimError> {
        LbjGenerator::new(cfg.rate_hz, demo_bursts(), LbjGenOptions::from_config(cfg))
    }

    /// Modulates burst `i` into the sample buffer.
    fn start_burst(&mut self, i: usize) {
        // Bit 1 is the lower frequency.
        let levels: Vec<f32> = self.bursts[i]
            .iter()
            .map(|&b| if b == 1 { -1.0 } else { 1.0 })
            .collect();
        self.samples.clear();
        if let Some(m) = self.modulator.as_mut() {
            m.push(&levels, &mut self.samples);
            m.flush(&mut self.samples);
        }
        self.osc.mix_in_place(&mut self.samples);
        self.at = 0;
    }

    /// Moves on to the next segment; false at the end of the signal.
    fn advance(&mut self) -> bool {
        match self.next {
            Segment::End => return false,
            Segment::Quiet(n) => {
                self.quiet = n;
                self.next = match self.last {
                    None if !self.bursts.is_empty() => Segment::Burst(0),
                    Some(i) if i + 1 < self.bursts.len() => Segment::Burst(i + 1),
                    Some(_) if self.opt.repeat => Segment::Burst(0),
                    _ => Segment::End,
                };
            }
            Segment::Burst(i) => {
                self.start_burst(i);
                self.last = Some(i);
                let more = i + 1 < self.bursts.len() || self.opt.repeat;
                let gap = if more { self.opt.gap_s } else { 0.1 };
                self.next = Segment::Quiet((gap * self.rate).round() as usize);
            }
        }
        true
    }
}

impl Generator for LbjGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut written = 0;
        while written < out.len() {
            if self.at < self.samples.len() {
                let n = (self.samples.len() - self.at).min(out.len() - written);
                out[written..written + n].copy_from_slice(&self.samples[self.at..self.at + n]);
                self.at += n;
                written += n;
            } else if self.quiet > 0 {
                let n = self.quiet.min(out.len() - written);
                out[written..written + n].fill(Complex32::new(0.0, 0.0));
                self.quiet -= n;
                written += n;
            } else if !self.advance() {
                break;
            }
        }
        written
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of LBJ is the carrier over the noise in 25 kHz.
        Some(
            SignalInfo::new(25e3)
                .with_symbol_rate(BIT_RATE)
                .with_bit_rate(BIT_RATE),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_and_bits() {
        assert_eq!(basic_text("19533", 67, 382), "19533  67   382");
        assert_eq!(basic_text("123", 23, 123), "123    23   123");
        let b = burst_bits(&demo_bursts()[0]);
        assert_eq!(b.len(), 576 + 544);
        assert_eq!(&b[..4], &[1, 0, 1, 0]);
    }

    #[test]
    fn the_signal_has_unit_power_and_the_bursts_in_order() {
        let opt = LbjGenOptions {
            repeat: false,
            ..Default::default()
        };
        let mut g = LbjGenerator::new(960e3, demo_bursts(), opt).unwrap();
        let mut x = vec![Complex32::new(0.0, 0.0); 960_000 * 6];
        let n = g.generate(&mut x);
        // 0.3 s lead, four bursts of 1120 bits, three gaps of 0.5 s and 0.1 s at the end.
        let burst = (1120.0 * 800.0) as usize;
        assert_eq!(n, 288_000 + 4 * burst + 3 * 480_000 + 96_000);
        let on: Vec<f32> = x[..n]
            .iter()
            .map(|s| s.norm_sqr())
            .filter(|&p| p > 0.0)
            .collect();
        assert_eq!(on.len(), 4 * burst);
        assert!(on.iter().all(|p| (p - 1.0).abs() < 1e-3));
    }
}

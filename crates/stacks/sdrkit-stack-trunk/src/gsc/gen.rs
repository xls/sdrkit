//! The GSC test signal, made from the format: pages as single calls (preamble, start code, address, then data blocks, or the
//! activation word, a voice-like tone pair and the deactivation word) or as one batch, on 2-FSK with the frequency smoothed
//! by a one-pole low-pass of 900 Hz, mark the upper frequency, at unit amplitude.

use super::code::{
    self, encode, inverse, push_data_block, push_preamble, push_two_words, text_fields, PagerCode,
    BLOCK_BITS, TEST_ACTIVATION, TEST_DEACTIVATION,
};
use num_complex::Complex32;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The grid rate, bit/s.
pub const GRID_RATE: f64 = 600.0;

/// What a page carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxKind {
    /// No data.
    Tone,
    /// Numeric data blocks.
    Numeric,
    /// Alphanumeric data blocks.
    Alpha,
    /// The activation word, voice and the deactivation word.
    Voice,
}

/// One page to send.
#[derive(Clone, Debug, PartialEq)]
pub struct TxPage {
    /// The six-digit pager code.
    pub code: u32,
    /// The address function, 1 to 4.
    pub function: u8,
    /// What the page carries.
    pub kind: TxKind,
    /// The text of a data page.
    pub text: String,
    /// The voice length, s.
    pub voice_s: f64,
}

impl TxPage {
    /// A page of `kind` to `code` with function 1, a voice of 2 s.
    pub fn new(code: u32, kind: TxKind, text: &str) -> TxPage {
        TxPage {
            code,
            function: 1,
            kind,
            text: text.to_string(),
            voice_s: 2.0,
        }
    }
}

/// The options of the signal.
#[derive(Clone, Debug, PartialEq)]
pub struct GscOptions {
    /// The pages.
    pub pages: Vec<TxPage>,
    /// One batch (an inverted preamble, the start code, then every page) instead of single calls.
    pub batch: bool,
    /// The battery-saver preamble and start code; without them an alternating pattern comes first.
    pub preamble: bool,
    /// The deviation, Hz.
    pub deviation_hz: f64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The drift of the carrier, Hz/s.
    pub drift_hz_s: f64,
    /// The sender's clock error, ppm.
    pub clock_ppm: f64,
    /// The drift of the sender's clock, ppm/s.
    pub clock_drift_ppm_s: f64,
    /// The spectrum is inverted.
    pub inverted: bool,
    /// Unmodulated carrier before and after the calls, s.
    pub carrier_s: f64,
    /// No carrier after the signal, s, before it starts again.
    pub gap_s: f64,
}

impl Default for GscOptions {
    /// The test pages as single calls, ±4500 Hz, 0.2 s of carrier around them and 0.3 s without a carrier.
    fn default() -> Self {
        GscOptions {
            pages: test_pages(),
            batch: false,
            preamble: true,
            deviation_hz: 4500.0,
            offset_hz: 0.0,
            drift_hz_s: 0.0,
            clock_ppm: 0.0,
            clock_drift_ppm_s: 0.0,
            inverted: false,
            carrier_s: 0.2,
            gap_s: 0.3,
        }
    }
}

/// The test pages: a tone page to 954853, a numeric page "5551234" and an alphanumeric page "CALL HQ" to 123456, and a
/// voice page of 2 s to 954853.
pub fn test_pages() -> Vec<TxPage> {
    vec![
        TxPage::new(954_853, TxKind::Tone, ""),
        TxPage::new(123_456, TxKind::Numeric, "5551234"),
        TxPage::new(123_456, TxKind::Alpha, "CALL HQ"),
        TxPage::new(954_853, TxKind::Voice, ""),
    ]
}

/// A piece of the signal.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    /// Grid bits at 600 bit/s.
    Bits(Vec<u8>),
    /// Unmodulated carrier, s.
    Carrier(f64),
    /// Voice-like audio, s.
    Voice(f64),
    /// No carrier, s.
    Off(f64),
}

/// The address block and what follows it, for one page.
fn push_page(p: &TxPage, bits: &mut Vec<u8>, segs: &mut Vec<Segment>) {
    let c = PagerCode::from_digits(p.code);
    let (inv1, inv2) = code::function_senses(p.function);
    let mut w1 = encode(c.word1());
    let mut w2 = encode(c.word2());
    if inv1 {
        w1 = inverse(w1);
    }
    if inv2 {
        w2 = inverse(w2);
    }
    push_two_words(w1, w2, bits);
    match p.kind {
        TxKind::Tone => {}
        TxKind::Numeric | TxKind::Alpha => {
            for f in text_fields(&p.text, p.kind == TxKind::Numeric) {
                push_data_block(&code::block_info(f), bits);
            }
        }
        TxKind::Voice => {
            let a = encode(TEST_ACTIVATION);
            push_two_words(a, inverse(a), bits);
            segs.push(Segment::Bits(std::mem::take(bits)));
            segs.push(Segment::Voice(p.voice_s));
            let d = encode(TEST_DEACTIVATION);
            push_two_words(d, inverse(d), bits);
        }
    }
}

/// The segments of the options, carrier first, the gap last.
pub fn segments(o: &GscOptions) -> Vec<Segment> {
    let mut segs = vec![Segment::Carrier(o.carrier_s)];
    let mut bits = Vec::new();
    let start = encode(code::START_CODE);
    let preamble_of = |p: &TxPage| {
        PagerCode::from_digits(p.code)
            .preamble_index()
            .map_or(0, |i| code::preamble_words()[i])
    };
    if !o.preamble {
        for k in 0..BLOCK_BITS {
            bits.push((k & 1) as u8);
        }
        for p in &o.pages {
            push_page(p, &mut bits, &mut segs);
        }
    } else if o.batch {
        if let Some(first) = o.pages.first() {
            push_preamble(inverse(preamble_of(first)), &mut bits);
            push_two_words(start, inverse(start), &mut bits);
            for p in &o.pages {
                push_page(p, &mut bits, &mut segs);
            }
        }
    } else {
        for (k, p) in o.pages.iter().enumerate() {
            if k > 0 {
                segs.push(Segment::Bits(std::mem::take(&mut bits)));
                segs.push(Segment::Carrier(0.1));
            }
            push_preamble(preamble_of(p), &mut bits);
            push_two_words(start, inverse(start), &mut bits);
            push_page(p, &mut bits, &mut segs);
        }
    }
    segs.push(Segment::Bits(bits));
    segs.push(Segment::Carrier(o.carrier_s));
    segs.push(Segment::Off(o.gap_s));
    segs.retain(|s| !matches!(s, Segment::Bits(b) if b.is_empty()));
    segs
}

/// The length of the segments at the nominal clock, s.
pub fn length_s(segs: &[Segment]) -> f64 {
    segs.iter()
        .map(|s| match s {
            Segment::Bits(b) => b.len() as f64 / GRID_RATE,
            Segment::Carrier(t) | Segment::Voice(t) | Segment::Off(t) => *t,
        })
        .sum()
}

/// The generator: the segments, repeated without end.
pub struct GscGenerator {
    rate: f64,
    channel_hz: f64,
    o: GscOptions,
    segs: Vec<Segment>,
    at: usize,
    /// Grid bits or seconds into the segment.
    pos: f64,
    t: f64,
    level: f64,
    alpha: f64,
    phase: f64,
}

impl GscGenerator {
    /// A generator of `o` at `rate`, with the channel `channel_hz` from the centre.
    pub fn new(rate: f64, channel_hz: f64, o: GscOptions) -> GscGenerator {
        let segs = segments(&o);
        GscGenerator {
            rate,
            channel_hz,
            o,
            segs,
            at: 0,
            pos: 0.0,
            t: 0.0,
            level: 0.0,
            alpha: 1.0 - (-TAU * 900.0 / rate).exp(),
            phase: 0.0,
        }
    }

    /// The test signal of the settings of `cfg`, for a stack tuned `tune_offset_hz` above the channel.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] if the rate cannot hold the channel.
    pub fn from_config(cfg: &GenConfig, tune_offset_hz: f64) -> Result<GscGenerator, ConfigError> {
        let need = 2.0 * (tune_offset_hz + 12.5e3);
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        let pages = match cfg.settings.get(&GEN_PAGES) {
            TestPages::All => test_pages(),
            TestPages::Tone => vec![test_pages().swap_remove(0)],
            TestPages::Numeric => vec![test_pages().swap_remove(1)],
            TestPages::Alpha => vec![test_pages().swap_remove(2)],
            TestPages::Voice => vec![test_pages().swap_remove(3)],
        };
        let o = GscOptions {
            pages,
            batch: cfg.settings.get(&GEN_BATCH),
            preamble: !cfg.settings.get(&GEN_NO_PREAMBLE),
            deviation_hz: cfg.settings.get(&GEN_DEVIATION_HZ),
            ..GscOptions::default()
        };
        Ok(GscGenerator::new(cfg.rate_hz, -tune_offset_hz, o))
    }

    /// The modulation level of this sample (-1 to 1), and whether the carrier is on. Moves the position by one sample.
    fn step(&mut self) -> (f64, bool) {
        let dt = 1.0 / self.rate;
        let grid =
            GRID_RATE * (1.0 + (self.o.clock_ppm + self.o.clock_drift_ppm_s * self.t) * 1e-6);
        loop {
            let Some(seg) = self.segs.get(self.at) else {
                self.at = 0;
                self.pos = 0.0;
                continue;
            };
            let (level, on, len, step) = match seg {
                Segment::Bits(b) => {
                    let k = self.pos as usize;
                    let Some(&bit) = b.get(k) else {
                        self.at += 1;
                        self.pos = 0.0;
                        continue;
                    };
                    (
                        if bit != 0 { 1.0 } else { -1.0 },
                        true,
                        b.len() as f64,
                        grid * dt,
                    )
                }
                Segment::Carrier(s) => (0.0, true, *s, dt),
                Segment::Off(s) => (0.0, false, *s, dt),
                Segment::Voice(s) => {
                    let u = self.pos;
                    let env = 0.6 + 0.4 * (TAU * 3.0 * u).sin();
                    let a = 0.45 * (TAU * 470.0 * u).sin() + 0.35 * (TAU * 1130.0 * u).sin();
                    (env * a, true, *s, dt)
                }
            };
            if self.pos >= len {
                self.at += 1;
                self.pos = 0.0;
                continue;
            }
            self.pos += step;
            return (level, on);
        }
    }
}

impl Generator for GscGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let dt = 1.0 / self.rate;
        for z in out.iter_mut() {
            let (target, on) = self.step();
            self.t += dt;
            if !on {
                self.level = 0.0;
                *z = Complex32::new(0.0, 0.0);
                continue;
            }
            self.level += self.alpha * (target - self.level);
            let f = self.channel_hz
                + self.o.offset_hz
                + self.o.drift_hz_s * self.t
                + self.o.deviation_hz * self.level;
            self.phase = (self.phase + TAU * f * dt) % TAU;
            let (s, c) = self.phase.sin_cos();
            *z = if self.o.inverted {
                Complex32::new(c as f32, -s as f32)
            } else {
                Complex32::new(c as f32, s as f32)
            };
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the carrier over the noise in 12.5 kHz.
        Some(
            SignalInfo::new(12.5e3)
                .with_symbol_rate(GRID_RATE)
                .with_bit_rate(GRID_RATE),
        )
    }
}

sdrkit_core::choice_enum! {
    /// The pages of the test signal.
    pub enum TestPages {
        /// A tone, a numeric, an alphanumeric and a voice page.
        All = ("all", "Tone, numeric, alphanumeric and voice pages"),
        /// A tone page.
        Tone = ("tone", "A tone page"),
        /// A numeric page.
        Numeric = ("numeric", "A numeric page"),
        /// An alphanumeric page.
        Alpha = ("alpha", "An alphanumeric page"),
        /// A voice page.
        Voice = ("voice", "A voice page"),
    }
}

/// The pages of the test signal.
pub const GEN_PAGES: Setting<field::Choice<TestPages>> =
    Setting::choice("gen_pages", "Test signal pages", TestPages::All)
        .section("Test signal")
        .advanced();
/// Send the pages as one batch after an inverted preamble.
pub const GEN_BATCH: Setting<field::Bool> =
    Setting::new("gen_batch", "Test signal batch", "", false)
        .section("Test signal")
        .advanced();
/// Send no preamble and no start code (pagers without battery saving).
pub const GEN_NO_PREAMBLE: Setting<field::Bool> =
    Setting::new("gen_no_preamble", "Test signal without preamble", "", false)
        .section("Test signal")
        .advanced();
/// The deviation of the test signal.
pub const GEN_DEVIATION_HZ: Setting<field::F64> =
    Setting::new("gen_deviation_hz", "Test signal deviation", "Hz", 4500.0)
        .range(500.0, 8000.0, 100.0)
        .section("Test signal")
        .advanced();

/// The test signal settings. The framework adds the standard `gen_*` settings.
pub const SETTINGS: &[&dyn DeclareSetting] =
    &[&GEN_PAGES, &GEN_BATCH, &GEN_NO_PREAMBLE, &GEN_DEVIATION_HZ];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_single_calls_have_their_lengths() {
        let segs = segments(&GscOptions::default());
        let bits: usize = segs
            .iter()
            .map(|s| match s {
                Segment::Bits(b) => b.len(),
                _ => 0,
            })
            .sum();
        // Four calls of preamble (856), start and address (242); data 121 + 121; voice words 242.
        assert_eq!(bits, 4 * (856 + 242) + 121 + 121 + 242);
        assert!(
            (length_s(&segs) - (bits as f64 / 600.0 + 0.2 + 0.2 + 0.3 + 0.3 + 2.0)).abs() < 1e-9
        );
    }
}

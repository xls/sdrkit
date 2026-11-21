//! The ERMES test signal: one batch (batch E of country 40, operator 2, area 5, cycle 12, subsequence 3; three initial
//! addresses and two messages), or the last batch P with no address and no message, as 4-level FSK on FM with the level
//! smoothed by a one-pole low-pass at 3000 Hz, at unit amplitude, then 0.3 s without a carrier, repeated without end.

use super::code::{
    self, alpha_words, apt_words, encode, headers, interleave, numeric_words, Category, SystemInfo,
    APT, CODEBLOCK, DELIMITER, LEVELS_HZ, PREAMBLE, SYMBOL_RATE, SYNC,
};
use num_complex::Complex32;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// Send the last batch of a subsequence (batch P, 190 words) with no address and no message.
pub const GEN_LAST_BATCH: Setting<field::Bool> =
    Setting::new("gen_last_batch", "Test signal last batch", "", false)
        .section("Test signal")
        .advanced();

/// The test signal settings. The framework adds the standard `gen_*` settings.
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_LAST_BATCH];

/// A message to send.
#[derive(Clone, Debug, PartialEq)]
pub struct TxMessage {
    /// The 22-bit local address.
    pub address: u32,
    /// The category.
    pub category: Category,
    /// The text.
    pub text: String,
}

/// The system information of the test batch.
pub fn test_system(batch: u8) -> SystemInfo {
    SystemInfo {
        country: 40,
        operator: 2,
        area: 5,
        cycle: 12,
        subsequence: 3,
        batch,
        ssi_field: SystemInfo::ssi_time(3, 18, 21),
        ..SystemInfo::default()
    }
}

/// The messages of the test batch.
pub fn test_messages() -> Vec<TxMessage> {
    vec![
        TxMessage {
            address: 0x2ABCD5,
            category: Category::Alphanumeric,
            text: "HELLO WORLD".into(),
        },
        TxMessage {
            address: 0x123454,
            category: Category::Numeric,
            text: "1234567890".into(),
        },
    ]
}

/// The sent words of a batch, preamble first.
pub fn batch_words(si: &SystemInfo, addresses: &[u32], messages: &[TxMessage]) -> Vec<u32> {
    let total = si.batch_words();
    let mut w = vec![PREAMBLE, SYNC];
    w.extend(si.words().iter().map(|&i| encode(i)));
    w.extend(addresses.iter().map(|&a| encode(a)));
    w.extend(std::iter::repeat_n(APT, apt_words(addresses.len())));
    let mut rows = vec![DELIMITER];
    for m in messages {
        let [h1, h2] = headers(m.address, m.category, false);
        rows.push(encode(h1));
        rows.push(encode(h2));
        let data = match m.category {
            Category::Numeric => numeric_words(&m.text),
            Category::Alphanumeric => alpha_words(&m.text),
            Category::Tone | Category::Transparent => Vec::new(),
        };
        rows.extend(data.into_iter().map(encode));
        rows.push(DELIMITER);
    }
    let room = total - w.len();
    rows.resize(room, DELIMITER);
    rows.truncate(room);
    for block in rows.chunks(CODEBLOCK) {
        let b: [u32; CODEBLOCK] =
            std::array::from_fn(|i| block.get(i).copied().unwrap_or(DELIMITER));
        w.extend_from_slice(&interleave(&b));
    }
    w
}

/// The generator.
pub struct ErmesGenerator {
    rate: f64,
    channel_hz: f64,
    /// The level index of every symbol of the batch.
    symbols: Vec<usize>,
    at: usize,
    pos: f64,
    quiet: usize,
    gap: usize,
    level: f64,
    alpha: f64,
    phase: f64,
}

impl ErmesGenerator {
    /// A generator of `words` at `rate`, the channel `channel_hz` from the centre, `gap_s` without a carrier after each copy.
    pub fn new(rate: f64, words: &[u32], channel_hz: f64, gap_s: f64) -> ErmesGenerator {
        let symbols = words.iter().flat_map(|&w| code::word_levels(w)).collect();
        ErmesGenerator {
            rate,
            channel_hz,
            symbols,
            at: 0,
            pos: 0.0,
            quiet: (0.05 * rate) as usize,
            gap: (gap_s * rate) as usize,
            level: 0.0,
            alpha: 1.0 - (-TAU * 3000.0 / rate).exp(),
            phase: 0.0,
        }
    }

    /// The test batch of the settings of `cfg`, for a stack tuned `tune_offset_hz` above the channel.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] if the rate cannot hold the channel.
    pub fn from_config(
        cfg: &GenConfig,
        tune_offset_hz: f64,
    ) -> Result<ErmesGenerator, ConfigError> {
        let need = 2.0 * (tune_offset_hz + 12.5e3);
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        let words = if cfg.settings.get(&GEN_LAST_BATCH) {
            batch_words(&test_system(code::LAST_BATCH), &[], &[])
        } else {
            batch_words(&test_system(4), &[1000, 200, 30], &test_messages())
        };
        Ok(ErmesGenerator::new(
            cfg.rate_hz,
            &words,
            -tune_offset_hz,
            0.3,
        ))
    }
}

impl Generator for ErmesGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = SYMBOL_RATE / self.rate;
        for z in out.iter_mut() {
            if self.quiet > 0 {
                self.quiet -= 1;
                *z = Complex32::new(0.0, 0.0);
                continue;
            }
            let target = LEVELS_HZ[self.symbols[self.at]];
            self.level += self.alpha * (target - self.level);
            self.phase = (self.phase + TAU * (self.channel_hz + self.level) / self.rate) % TAU;
            *z = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
            self.pos += step;
            if self.pos >= 1.0 {
                self.pos -= 1.0;
                self.at += 1;
                if self.at >= self.symbols.len() {
                    self.at = 0;
                    self.quiet = self.gap;
                    self.level = 0.0;
                }
            }
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the carrier over the noise in 25 kHz.
        Some(
            SignalInfo::new(25e3)
                .with_symbol_rate(SYMBOL_RATE)
                .with_bit_rate(2.0 * SYMBOL_RATE),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ermes::rx::read_batch;

    #[test]
    fn the_test_batch_reads_back() {
        let w = batch_words(&test_system(4), &[1000, 200, 30], &test_messages());
        assert_eq!(w.len(), code::BATCH_WORDS);
        let b = read_batch(&w[2..]).expect("a good batch");
        assert_eq!(b.addresses, [1000, 200, 30]);
        assert_eq!(b.messages.len(), 2);
        assert_eq!(b.messages[0].address, 0x2ABCD5);
        assert_eq!(b.messages[0].text, "HELLO WORLD");
        assert_eq!(b.messages[1].text, "1234567890");
        assert_eq!(b.si, test_system(4));
        let p = batch_words(&test_system(15), &[], &[]);
        assert_eq!(p.len(), code::LAST_BATCH_WORDS);
        let b = read_batch(&p[2..]).expect("a good batch");
        assert!(b.addresses.is_empty() && b.messages.is_empty());
    }
}

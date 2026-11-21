//! The identification keying of the test signals: Morse text sent again and again with a pause between, as an envelope from 0 to
//! 1 with shaped edges. The keying comes from the shared Morse keyer of `sdrkit-algo-sim`.
//! Standard: ITU-R M.1677-1 Part I section 2 (timing).

use sdrkit_algo_sim::modulate::Morse;
use sdrkit_core::{Complex32, Generator};

/// The envelope of a repeated identification.
pub struct IdentEnvelope {
    morse: Option<Morse>,
    buf: Vec<Complex32>,
}

impl IdentEnvelope {
    /// `text` at `wpm` words a minute for samples at `rate`, after `lead_s` seconds of silence, with `pause_s` seconds between
    /// repeats and edges of `rise_s` seconds. An empty text (or a text that Morse cannot send) never keys.
    pub fn new(text: &str, wpm: f64, rate: f64, lead_s: f64, pause_s: f64, rise_s: f64) -> Self {
        let morse = Morse::new(text, wpm, rate, 0.0, rise_s)
            .and_then(|m| m.repeating(pause_s))
            .and_then(|m| m.with_lead(lead_s))
            .ok();
        IdentEnvelope {
            morse,
            buf: Vec::new(),
        }
    }

    /// The envelope of the next `n` samples, each from 0 (key up) to 1 (key down). Allocates nothing once the buffer has grown to
    /// `n`.
    pub fn next(&mut self, n: usize) -> impl Iterator<Item = f32> + '_ {
        self.buf.clear();
        self.buf.resize(n, Complex32::default());
        if let Some(m) = &mut self.morse {
            m.generate(&mut self.buf);
        }
        self.buf.iter().map(|z| z.re)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_repeats() {
        // "E" at 12 wpm is one 100 ms mark. With a pause of 1 s, one cycle is 1.1 s.
        let mut s = IdentEnvelope::new("E", 12.0, 1000.0, 0.0, 1.0, 0.0);
        let on = s.next(2200).filter(|&v| v > 0.5).count();
        assert_eq!(on, 200);
        let mut quiet = IdentEnvelope::new("", 12.0, 1000.0, 0.0, 1.0, 0.0);
        assert_eq!(quiet.next(100).filter(|&v| v > 0.0).count(), 0);
    }
}

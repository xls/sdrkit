//! The presets of the general receiver: a frequency with the demodulator and the filter width that suit it. The host
//! sets them with the setting `presets`; the `presets` table lists them, and the command `preset.apply` applies one.

use crate::squelch::SquelchKind;
use crate::typed::{PresetRow, PRESETS as TABLE};
use crate::types::Demod;
use sdrkit_core::interface::Hertz;
use sdrkit_core::{Choice as _, Published};

/// The most presets the setting holds (the capacity of the `presets` table).
pub const MAX_PRESETS: usize = 64;

/// One preset.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// The row key in the `presets` table.
    pub key: String,
    /// The name for people.
    pub name: String,
    /// The frequency (Hz).
    pub freq_hz: f64,
    /// The demodulator.
    pub demod: Demod,
    /// The filter width (Hz).
    pub width_hz: f64,
    /// The squelch.
    pub squelch: SquelchKind,
}

impl Preset {
    /// Writes the row of this preset into the `presets` table of `p`.
    pub fn write_row(&self, p: &mut Published) {
        TABLE.put(
            p,
            &self.key,
            &PresetRow {
                name: &self.name,
                freq_hz: Hertz(self.freq_hz),
                demod: self.demod,
                width: Hertz(self.width_hz),
                squelch: self.squelch,
            },
        );
    }
}

/// The presets of the text of the setting `presets`: entries separated by `;`, each a key, `=`, the frequency in Hz,
/// the demodulator key, the filter width in Hz and the name, separated by `,` (the name is the rest of the entry).
/// `None` for text that has an entry without this form, two entries with one key, or more than [`MAX_PRESETS`]
/// entries. Empty text has no presets.
pub fn parse(text: &str) -> Option<Vec<Preset>> {
    let mut out: Vec<Preset> = Vec::new();
    for e in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let (key, rest) = e.split_once('=')?;
        let key = key.trim();
        let mut parts = rest.splitn(4, ',').map(str::trim);
        let freq_hz: f64 = parts.next()?.parse().ok()?;
        let demod = Demod::from_key(parts.next()?)?;
        let width_hz: f64 = parts.next()?.parse().ok()?;
        let name = parts.next().unwrap_or("").trim();
        let valid = !key.is_empty()
            && freq_hz.is_finite()
            && freq_hz > 0.0
            && width_hz.is_finite()
            && width_hz > 0.0
            && out.iter().all(|p| p.key != key);
        if !valid {
            return None;
        }
        out.push(Preset {
            key: key.to_owned(),
            name: if name.is_empty() { key } else { name }.to_owned(),
            freq_hz,
            demod,
            width_hz,
            squelch: SquelchKind::Auto,
        });
    }
    (out.len() <= MAX_PRESETS).then_some(out)
}

/// The preset of `presets` with row key `key`.
pub fn preset<'a>(presets: &'a [Preset], key: &str) -> Option<&'a Preset> {
    presets.iter().find(|p| p.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_read_from_text_and_bad_text_is_refused() {
        let p = parse("cw=7030000,cw,500,CW; pmr = 446006250 , nfm , 8500 , PMR446, channel 1")
            .unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "CW");
        assert_eq!(
            (p[1].key.as_str(), p[1].freq_hz, p[1].demod, p[1].width_hz),
            ("pmr", 446_006_250.0, Demod::Nfm, 8500.0)
        );
        assert_eq!(p[1].name, "PMR446, channel 1");
        assert_eq!(parse("x=7030000,cw,500").unwrap()[0].name, "x");
        assert_eq!(parse("").unwrap(), Vec::new());
        for bad in [
            "x=7030000,dsb,500,X",
            "x=abc,cw,500,X",
            "=7030000,cw,500,X",
            "x=7030000,cw",
            "x=0,cw,500,X",
            "x=1,cw,1,A;x=2,cw,1,B",
        ] {
            assert!(parse(bad).is_none(), "{bad}");
        }
        let many: Vec<String> = (0..=MAX_PRESETS)
            .map(|i| format!("k{i}=1,am,1,K"))
            .collect();
        assert!(parse(&many.join(";")).is_none());
        assert!(preset(&p, "pmr").is_some() && preset(&p, "nowhere").is_none());
    }
}

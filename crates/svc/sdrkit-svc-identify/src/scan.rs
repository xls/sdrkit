//! Scan and identify in one step: every signal in the capture, identified, as rows that a list or a CSV file can show.

use crate::identify::{Identifier, Sureness, Verdict};
use crate::verify::Check;
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};

/// One signal found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScanRow {
    /// Where (Hz).
    pub freq_hz: f64,
    /// Width (Hz).
    pub width_hz: f64,
    /// Power (dBFS).
    pub level_dbfs: f64,
    /// SNR (dB).
    pub snr_db: f64,
    /// The measurement in a line.
    pub summary: String,
    /// The beginner line ("Sure: DMR").
    pub line: String,
    /// The best guess, when there is one.
    pub best: Option<String>,
    /// Its calibrated probability.
    pub confidence: f64,
    /// How sure.
    pub sureness: Sureness,
    /// Its own detector confirmed it.
    pub verified: bool,
    /// The key of the signal type of the best guess, when there is one.
    pub signal: Option<String>,
    /// The ranked guesses (name, probability).
    pub ranked: Vec<(String, f64)>,
}

/// Finds every signal in the capture (at least 8 dB over the floor, at most 16, strongest first) and identifies each.
pub fn scan_and_identify(id: &Identifier, x: &[Complex32], rate: f64, rf_hz: f64) -> Vec<ScanRow> {
    id.identify_all(x, rate, rf_hz)
        .into_iter()
        .filter(|r| r.features.class != crate::signature::ModClass::Noise)
        .map(|r| {
            let named = r.verdict != Verdict::Unknown;
            let top = r.candidates.first();
            ScanRow {
                freq_hz: r.features.rf_hz,
                width_hz: r.features.width_99_hz,
                level_dbfs: r.features.power_dbfs,
                snr_db: r.features.snr_db,
                summary: r.summary.clone(),
                line: r.line.clone(),
                best: top.filter(|_| named).map(|c| c.name.clone()),
                confidence: top.filter(|_| named).map_or(0.0, |c| c.probability),
                sureness: r.sureness,
                verified: top
                    .and_then(|c| c.verification.as_ref())
                    .is_some_and(|v| v.check == Check::Confirmed),
                signal: top.filter(|_| named).map(|c| c.signal.clone()),
                ranked: r
                    .candidates
                    .iter()
                    .filter(|c| c.fits)
                    .take(3)
                    .map(|c| (c.name.clone(), c.probability))
                    .collect(),
            }
        })
        .collect()
}

/// The rows as CSV (frequency, width, level, SNR, best guess, confidence, verified, signal type, summary).
pub fn to_csv(rows: &[ScanRow]) -> String {
    let mut s = String::from(
        "freq_hz,width_hz,level_dbfs,snr_db,best,confidence,verified,signal,summary\n",
    );
    for r in rows {
        let q = |t: &str| format!("\"{}\"", t.replace('"', "\"\""));
        s.push_str(&format!(
            "{:.0},{:.0},{:.1},{:.1},{},{:.2},{},{},{}\n",
            r.freq_hz,
            r.width_hz,
            r.level_dbfs,
            r.snr_db,
            q(r.best.as_deref().unwrap_or("")),
            r.confidence,
            r.verified,
            q(r.signal.as_deref().unwrap_or("")),
            q(&r.summary)
        ));
    }
    s
}

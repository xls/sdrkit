//! Telemetry (APRS 1.0.1 chapter 13): the `T#` report of five analog values and eight bits, and the four messages that name and
//! scale the channels (`PARM.`, `UNIT.`, `EQNS.`, `BITS.`).

/// One telemetry report.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Telemetry {
    /// The sequence number as sent (three digits, or `MIC` from a Mic-E unit).
    pub sequence: String,
    /// The five analog values as sent (APRS 1.0.1 sends 0 to 255. APRS 1.2 allows any decimal number).
    pub analog: [Option<f64>; 5],
    /// The eight digital bits, B1 in the most significant place.
    pub digital: Option<u8>,
    /// Text after the values.
    pub comment: String,
}

impl Telemetry {
    /// Reads the text after `T#`.
    pub(crate) fn parse(s: &str) -> Option<Telemetry> {
        let mut t = Telemetry::default();
        let mut parts = s.splitn(7, ',');
        t.sequence = parts.next()?.trim().to_string();
        if t.sequence.is_empty() {
            return None;
        }
        for k in 0..5 {
            let Some(p) = parts.next() else {
                return Some(t);
            };
            let p = p.trim();
            if !p.is_empty() {
                t.analog[k] = Some(p.parse().ok()?);
            }
        }
        if let Some(rest) = parts.next() {
            let b = rest.as_bytes();
            let n = b
                .iter()
                .take(8)
                .take_while(|c| matches!(c, b'0' | b'1'))
                .count();
            if n == 8 {
                t.digital = Some(b[..8].iter().fold(0u8, |a, &c| (a << 1) | (c - b'0')));
                t.comment = rest[8..].trim().to_string();
            } else {
                t.comment = rest.trim().to_string();
            }
        }
        Some(t)
    }
}

/// The messages that describe a station's telemetry. A station sends them to itself as APRS messages.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TelemetryMeta {
    /// `PARM.`: the names of the channels (A1 to A5, then B1 to B8).
    Parameters(Vec<String>),
    /// `UNIT.`: the units or labels of the channels.
    Units(Vec<String>),
    /// `EQNS.`: for each analog channel the coefficients a, b, c of `a x² + b x + c`.
    Equations([[f64; 3]; 5]),
    /// `BITS.`: the value of each bit that means "on", and the project title.
    Bits {
        /// The sense bits, B1 in the most significant place.
        sense: u8,
        /// The project title.
        title: String,
    },
}

impl TelemetryMeta {
    /// Reads a message text that starts with `PARM.`, `UNIT.`, `EQNS.` or `BITS.`.
    pub(crate) fn parse(text: &str) -> Option<TelemetryMeta> {
        let list = |s: &str| {
            s.split(',')
                .map(|v| v.trim().to_string())
                .collect::<Vec<_>>()
        };
        if let Some(s) = text.strip_prefix("PARM.") {
            return Some(TelemetryMeta::Parameters(list(s)));
        }
        if let Some(s) = text.strip_prefix("UNIT.") {
            return Some(TelemetryMeta::Units(list(s)));
        }
        if let Some(s) = text.strip_prefix("EQNS.") {
            let mut e = [[0.0, 1.0, 0.0]; 5];
            for (k, v) in s.split(',').enumerate().take(15) {
                let v = v.trim();
                if !v.is_empty() {
                    e[k / 3][k % 3] = v.parse().ok()?;
                }
            }
            return Some(TelemetryMeta::Equations(e));
        }
        if let Some(s) = text.strip_prefix("BITS.") {
            let b = s.as_bytes();
            if b.len() < 8 || !b[..8].iter().all(|c| matches!(c, b'0' | b'1')) {
                return None;
            }
            let sense = b[..8].iter().fold(0u8, |a, &c| (a << 1) | (c - b'0'));
            let title = s[8..].trim_start_matches(',').trim().to_string();
            return Some(TelemetryMeta::Bits { sense, title });
        }
        None
    }
}

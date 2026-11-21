//! What a signal type looks like on the air: a [`Signature`] that the identifier compares with a measurement.

use serde::{Deserialize, Serialize};

/// The modulation class the identifier measures, and the classes a signature may show as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModClass {
    /// Nothing could be said.
    Unknown,
    /// No signal above the noise floor.
    Noise,
    /// An unmodulated carrier (or a single tone: CW, SSB with one tone).
    Carrier,
    /// Amplitude modulation with a carrier.
    Am,
    /// Analogue frequency modulation (constant envelope, no symbol structure).
    Fm,
    /// Two-level frequency shift keying.
    Fsk2,
    /// Four-level frequency shift keying.
    Fsk4,
    /// Gaussian minimum shift keying (two levels, modulation index about 0.5).
    Gmsk,
    /// Binary phase shift keying.
    Bpsk,
    /// Quadrature phase shift keying.
    Qpsk,
    /// Differential QPSK with a π/4 rotation per symbol.
    Pi4Dqpsk,
    /// Eight-level phase shift keying.
    Psk8,
    /// Quadrature amplitude modulation (16 or more points).
    Qam,
    /// Orthogonal frequency division multiplexing.
    Ofdm,
    /// Linear chirps (LoRa and others).
    Chirp,
    /// Short pulses (on-off keying, pulse position).
    Pulse,
}

impl ModClass {
    /// The word shown to a user ("4FSK", "π/4-DQPSK").
    pub fn word(self) -> &'static str {
        match self {
            ModClass::Unknown => "unknown",
            ModClass::Noise => "noise",
            ModClass::Carrier => "carrier",
            ModClass::Am => "AM",
            ModClass::Fm => "FM",
            ModClass::Fsk2 => "2FSK",
            ModClass::Fsk4 => "4FSK",
            ModClass::Gmsk => "GMSK",
            ModClass::Bpsk => "BPSK",
            ModClass::Qpsk => "QPSK",
            ModClass::Pi4Dqpsk => "π/4-DQPSK",
            ModClass::Psk8 => "8PSK",
            ModClass::Qam => "QAM",
            ModClass::Ofdm => "OFDM",
            ModClass::Chirp => "chirp",
            ModClass::Pulse => "pulse",
        }
    }

    /// A rough family, for partial credit when the measured class is a near neighbour of the one expected (2FSK and GMSK, QPSK and
    /// π/4-DQPSK).
    pub fn family(self) -> u8 {
        match self {
            ModClass::Unknown | ModClass::Noise => 0,
            ModClass::Carrier | ModClass::Am => 1,
            ModClass::Fm | ModClass::Fsk2 | ModClass::Fsk4 | ModClass::Gmsk => 2,
            ModClass::Bpsk
            | ModClass::Qpsk
            | ModClass::Pi4Dqpsk
            | ModClass::Psk8
            | ModClass::Qam => 3,
            ModClass::Ofdm => 4,
            ModClass::Chirp => 5,
            ModClass::Pulse => 6,
        }
    }
}

/// Whether a signal is on all the time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keying {
    /// On all the time (a broadcast, a repeater's downlink).
    Continuous,
    /// On in bursts (TDMA, packets, a handheld's calls are not counted: a burst is shorter than a second).
    Bursty,
    /// Either.
    Either,
}

/// A frame sync word a cheap detector can search for on the symbols of an FSK signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameSync {
    /// A name for the evidence line ("BS voice sync").
    pub name: String,
    /// The symbols, as levels: ±1 for two levels, ±1 and ±3 for four.
    pub symbols: Vec<i8>,
    /// The bits are NRZI coded (a 1 keeps the level, a 0 changes it): the search runs on the decoded bits.
    pub nrzi: bool,
}

impl FrameSync {
    /// A four-level sync from its bits, two bits a symbol, most significant first, with the usual mapping
    /// (01 → +3, 00 → +1, 10 → −1, 11 → −3).
    pub fn fsk4(name: &str, bits: u64, symbols: usize) -> Self {
        let mut v = Vec::with_capacity(symbols);
        for i in (0..symbols).rev() {
            let d = (bits >> (2 * i)) & 3;
            v.push(match d {
                0b01 => 3,
                0b00 => 1,
                0b10 => -1,
                _ => -3,
            });
        }
        FrameSync {
            name: name.into(),
            symbols: v,
            nrzi: false,
        }
    }

    /// A two-level sync from its bits, most significant first (1 → +1, 0 → −1).
    pub fn fsk2(name: &str, bits: u64, len: usize) -> Self {
        let symbols = (0..len)
            .rev()
            .map(|i| if (bits >> i) & 1 == 1 { 1 } else { -1 })
            .collect();
        FrameSync {
            name: name.into(),
            symbols,
            nrzi: false,
        }
    }

    /// A two-level sync searched on NRZI-decoded bits.
    pub fn nrzi(name: &str, bits: u64, len: usize) -> Self {
        FrameSync {
            nrzi: true,
            ..Self::fsk2(name, bits, len)
        }
    }

    /// Two or four levels.
    pub fn levels(&self) -> u8 {
        if self.symbols.iter().any(|s| s.abs() > 1) {
            4
        } else {
            2
        }
    }
}

/// What a signal type looks like: everything the identifier can compare with a measurement. Every field but the names is
/// optional (empty, `None`): a rule with nothing to compare adds no points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    /// What the user reads ("P25 Phase 1").
    pub name: String,
    /// The key of the signal type (`"dmr"`). A stack declares the signal types that it opens by this key.
    pub signal: String,
    /// A group for group-level scoring ("Digital voice", "Broadcast", "Aviation").
    pub family: String,
    /// The classes it may show as.
    pub classes: Vec<ModClass>,
    /// The class it should show as.
    pub main_class: ModClass,
    /// Symbol rates (Bd).
    pub symbol_rates: Vec<f64>,
    /// OFDM useful symbol lengths (s).
    pub ofdm_tu_s: Vec<f64>,
    /// OFDM guard lengths (s), when they are fixed.
    pub ofdm_tg_s: Vec<f64>,
    /// The usual 99 % width (Hz), low and high.
    pub width_hz: (f64, f64),
    /// The width is only an upper limit (AM, FM, carriers: their width follows the programme).
    pub width_is_limit: bool,
    /// Two or four FSK levels.
    pub fsk_levels: Option<u8>,
    /// The deviation of the outer FSK level (Hz).
    pub fsk_outer_dev_hz: Option<f64>,
    /// Continuous or bursty.
    pub keying: Keying,
    /// A fixed burst length (s).
    pub burst_s: Option<f64>,
    /// A fixed burst period (s).
    pub period_s: Option<f64>,
    /// Frame syncs a cheap detector can find.
    pub frame_sync: Vec<FrameSync>,
    /// Bands where it is found (Hz, low and high).
    pub bands: Vec<(f64, f64)>,
    /// MFSK: the number of tones and their spacing (Hz).
    pub tones: Option<(u32, f64)>,
    /// The usual frequency (Hz).
    pub usual_hz: Option<f64>,
    /// Chirp bandwidths (Hz).
    pub chirp_bw_hz: Vec<f64>,
    /// Chirp spreading factors.
    pub chirp_sf: Vec<u8>,
    /// A 19 kHz stereo pilot in the FM multiplex.
    pub fm_pilot: bool,
}

impl Signature {
    /// A signature with a name, a signal type key, a family and the class it shows as. The builder methods add the rest.
    pub fn new(name: &str, signal: &str, family: &str, class: ModClass) -> Self {
        Signature {
            name: name.into(),
            signal: signal.into(),
            family: family.into(),
            classes: vec![class],
            main_class: class,
            symbol_rates: Vec::new(),
            ofdm_tu_s: Vec::new(),
            ofdm_tg_s: Vec::new(),
            width_hz: (0.0, 0.0),
            width_is_limit: false,
            fsk_levels: None,
            fsk_outer_dev_hz: None,
            keying: Keying::Either,
            burst_s: None,
            period_s: None,
            frame_sync: Vec::new(),
            bands: Vec::new(),
            tones: None,
            usual_hz: None,
            chirp_bw_hz: Vec::new(),
            chirp_sf: Vec::new(),
            fm_pilot: false,
        }
    }
    /// More classes it may show as.
    pub fn also(mut self, classes: &[ModClass]) -> Self {
        for c in classes {
            if !self.classes.contains(c) {
                self.classes.push(*c);
            }
        }
        self
    }
    /// Its symbol rates.
    pub fn rates(mut self, bd: &[f64]) -> Self {
        self.symbol_rates = bd.to_vec();
        self
    }
    /// Its usual 99 % width.
    pub fn width(mut self, lo_hz: f64, hi_hz: f64) -> Self {
        self.width_hz = (lo_hz, hi_hz);
        self.width_is_limit = false;
        self
    }
    /// A width that is only an upper limit.
    pub fn width_up_to(mut self, hi_hz: f64) -> Self {
        self.width_hz = (0.0, hi_hz);
        self.width_is_limit = true;
        self
    }
    /// FSK levels and the outer deviation.
    pub fn fsk(mut self, levels: u8, outer_dev_hz: f64) -> Self {
        self.fsk_levels = Some(levels);
        self.fsk_outer_dev_hz = Some(outer_dev_hz);
        self
    }
    /// OFDM useful symbol lengths and fixed guards (s).
    pub fn ofdm(mut self, tu_s: &[f64], tg_s: &[f64]) -> Self {
        self.ofdm_tu_s = tu_s.to_vec();
        self.ofdm_tg_s = tg_s.to_vec();
        self
    }
    /// Continuous.
    pub fn continuous(mut self) -> Self {
        self.keying = Keying::Continuous;
        self
    }
    /// Bursty, with a fixed burst length and period when they are known.
    pub fn bursty(mut self, burst_s: Option<f64>, period_s: Option<f64>) -> Self {
        self.keying = Keying::Bursty;
        self.burst_s = burst_s;
        self.period_s = period_s;
        self
    }
    /// A frame sync.
    pub fn sync(mut self, s: FrameSync) -> Self {
        self.frame_sync.push(s);
        self
    }
    /// A band where it is found.
    pub fn band(mut self, lo_hz: f64, hi_hz: f64) -> Self {
        self.bands.push((lo_hz, hi_hz));
        self
    }
    /// The usual frequency.
    pub fn usual(mut self, hz: f64) -> Self {
        self.usual_hz = Some(hz);
        self
    }
    /// MFSK tones.
    pub fn tones(mut self, count: u32, spacing_hz: f64) -> Self {
        self.tones = Some((count, spacing_hz));
        self
    }
    /// Chirps.
    pub fn chirp(mut self, bw_hz: &[f64], sf: &[u8]) -> Self {
        self.chirp_bw_hz = bw_hz.to_vec();
        self.chirp_sf = sf.to_vec();
        self
    }
    /// A stereo pilot.
    pub fn pilot(mut self) -> Self {
        self.fm_pilot = true;
        self
    }
    /// Whether `rf_hz` is in one of its bands (true when it lists none).
    pub fn in_band(&self, rf_hz: f64) -> Option<bool> {
        if self.bands.is_empty() {
            None
        } else {
            Some(
                self.bands
                    .iter()
                    .any(|&(lo, hi)| rf_hz >= lo && rf_hz <= hi),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_level_sync_from_bits() {
        // 0x7 = 01 11 -> +3 -3
        let s = FrameSync::fsk4("t", 0x7, 2);
        assert_eq!(s.symbols, vec![3, -3]);
        assert_eq!(s.levels(), 4);
        let b = FrameSync::fsk2("b", 0b101, 3);
        assert_eq!(b.symbols, vec![1, -1, 1]);
        assert_eq!(b.levels(), 2);
    }

    #[test]
    fn builder_and_band() {
        let s = Signature::new("X", "x", "Test", ModClass::Fsk2)
            .rates(&[1200.0])
            .band(100e6, 200e6)
            .also(&[ModClass::Gmsk, ModClass::Fsk2]);
        assert_eq!(s.classes, vec![ModClass::Fsk2, ModClass::Gmsk]);
        assert_eq!(s.in_band(150e6), Some(true));
        assert_eq!(s.in_band(50e6), Some(false));
        let json = serde_json::to_string(&s).unwrap();
        let back: Signature = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}

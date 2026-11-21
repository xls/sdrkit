//! `modem.dechirp`: chirp spread spectrum symbols to their FFT bins.

use super::pieces;
use sdrkit_algo_modem::chirp::{Dechirp, Direction};
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::EventDef;
use sdrkit_core::Field;
use sdrkit_core::Record;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::SlicePool;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Item, Kind, Port, Timed};
use std::sync::Arc;

/// One dechirped symbol (the output of `modem.dechirp`): the peak and the power of every bin, for hard or soft decisions
/// (`sdrkit_algo_modem::fsk::FskLlr` over [`bins`](Self::bins) with the stack's bit map returns the LLRs).
#[derive(Clone, Debug, PartialEq)]
pub struct ChirpSymbol {
    /// The symbol's number since the start (or the last reset).
    pub symbol: u64,
    /// The strongest bin, `0 .. 2^SF`: the symbol value when the timing and frequency are right.
    pub bin: u32,
    /// The peak's fractional offset from `bin` (Candan's estimator): timing plus frequency error, in bins.
    pub frac: f32,
    /// The timing error in chips measured at the frequency wrap (positive: read late); `None` for a down-chirp or symbol 0.
    pub timing: Option<f32>,
    /// The power of the strongest bin.
    pub power: f32,
    /// The mean power of the other bins.
    pub noise: f32,
    /// The power of every bin (shared, reused once every reader has dropped it).
    pub bins: Arc<[f32]>,
}

impl Item for ChirpSymbol {
    const KIND: &'static str = "chirp.bins";
    fn event_def() -> Option<&'static EventDef> {
        Some(&CHIRP_SYMBOL)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&SYMBOL, self.symbol as i64);
        r.set(&BIN, i64::from(self.bin));
        r.set(&FRAC, f64::from(self.frac));
        r.opt(&TIMING, self.timing.map(f64::from));
        r.set(&POWER, f64::from(self.power));
        r.set(&NOISE, f64::from(self.noise));
    }
}

const SYMBOL: Field<field::I64> = Field::new("symbol", "Symbol", "");
const BIN: Field<field::I64> = Field::new("bin", "Peak bin", "");
const FRAC: Field<field::F64> = Field::new("frac", "Fractional offset", "bins");
const TIMING: Field<field::F64> = Field::new("timing", "Timing error", "chips");
const POWER: Field<field::F64> = Field::new("power", "Peak power", "");
const NOISE: Field<field::F64> = Field::new("noise", "Noise power", "");

static CHIRP_SYMBOL: EventDef = EventDef::new(
    "chirp.bins",
    "Chirp symbol",
    &[&SYMBOL, &BIN, &FRAC, &TIMING, &POWER, &NOISE],
);

sdrkit_core::choice_enum! {
    /// The chirp direction of [`DechirpDemod`].
    pub enum ChirpDirection {
        /// Up-chirps.
        Up = ("up", "Up-chirps"),
        /// Down-chirps.
        Down = ("down", "Down-chirps"),
    }
}

const SF: Setting<field::I64> = Setting::new("sf", "Spreading factor", "", 7)
    .range(5.0, 12.0, 1.0)
    .help("2^SF chips (and bins) per symbol.");
const BW_HZ: Setting<field::Hz> = Setting::new("bw_hz", "Bandwidth", "Hz", 125_000.0)
    .range(1.0, 1e8, 0.0)
    .help("The chip rate; the sample rate must be a whole multiple of it (1 to 64).");
const DIRECTION: Setting<field::Choice<ChirpDirection>> =
    Setting::choice("direction", "Chirp", ChirpDirection::Up);

sdrkit_core::settings! {
    /// The settings of [`DechirpDemod`].
    pub enum DechirpDemodSetting {
        /// Spreading factor.
        Sf = SF,
        /// Bandwidth.
        BwHz = BW_HZ,
        /// Chirp.
        Direction = DIRECTION,
    }
}

/// `modem.dechirp`: chirp symbols (`Iq` at `os` times `bw_hz`, filtered to the bandwidth) to [`ChirpSymbol`]s, one per `2^SF os`
/// samples, wrapping `sdrkit_algo_modem::chirp::Dechirp` (base chirp removed, FFT planned once, Candan fraction, wrap timing).
///
/// Symbols are read back to back from the start of the stream (and from every `Retuned` or `Discontinuity` tag): aligning them to
/// a preamble is the receiver's work (the LoRa PHY finds the preamble, then feeds this component from the first data symbol, or
/// calls `Dechirp::dechirp_at` on the burst). Publishes `snr_db`, the last symbol's peak over the mean of the other bins.
#[derive(Debug)]
pub struct DechirpDemod {
    sf: u8,
    bw_hz: f64,
    direction: ChirpDirection,
    dechirp: Option<Box<Dechirp>>,
    buf: Vec<Complex32>,
    /// Absolute input index of `buf[0]`.
    start: u64,
    rate: f64,
    symbols: u64,
    pool: SlicePool<f32>,
    results: Published,
    every: Every,
    ratio: f32,
}

impl Default for DechirpDemod {
    fn default() -> Self {
        DechirpDemod {
            sf: SF.default as u8,
            bw_hz: BW_HZ.default,
            direction: DIRECTION.default,
            dechirp: None,
            buf: Vec::new(),
            start: 0,
            rate: 0.0,
            symbols: 0,
            pool: SlicePool::default(),
            results: Published::new(&[&keys::SNR_DB]),
            every: Every::default(),
            ratio: 0.0,
        }
    }
}

impl Component for DechirpDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.dechirp",
            vec![Port::iq("in")],
            vec![Port::typed::<ChirpSymbol>("out")],
        )
        .with_settings(DechirpDemodSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let os = f.rate / self.bw_hz;
        if !(os >= 0.5 && (os - os.round()).abs() < 1e-6 * os.max(1.0)) {
            return Err(ConfigError::RateNotMultiple {
                got: f.rate,
                of: self.bw_hz,
            });
        }
        let d = Dechirp::new(self.sf, os.round() as usize).map_err(ConfigError::from)?;
        self.buf = Vec::with_capacity(d.symbol_len());
        self.pool = SlicePool::with_ready(d.chips(), 16);
        self.dechirp = Some(Box::new(d));
        self.start = 0;
        self.rate = f.rate;
        self.symbols = 0;
        self.every = Every::new(f.rate, 0.25);
        Ok(vec![Format::new(Kind::typed::<ChirpSymbol>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let first = io.input(0).first();
        let x = io.iq(0);
        let dir = if self.direction == ChirpDirection::Down {
            Direction::Down
        } else {
            Direction::Up
        };
        let Some(d) = self.dechirp.as_mut() else {
            return;
        };
        let len = d.symbol_len();
        for (a, b, reset) in pieces(io.input(0), n) {
            if reset {
                self.buf.clear();
                self.symbols = 0;
            }
            let mut i = a;
            while i < b {
                if self.buf.is_empty() {
                    self.start = first + i as u64;
                }
                let take = (len - self.buf.len()).min(b - i);
                self.buf.extend_from_slice(&x[i..i + take]);
                i += take;
                if self.buf.len() == len {
                    // A sample that is not a number makes the whole symbol unreadable: it is skipped (its number still counts).
                    if let Ok(p) = d.dechirp(&self.buf, dir, 0.0) {
                        self.ratio = p.ratio();
                        let item = ChirpSymbol {
                            symbol: self.symbols,
                            bin: p.bin as u32,
                            frac: p.frac as f32,
                            timing: p.timing.map(|t| t as f32),
                            power: p.power,
                            noise: p.noise,
                            bins: self
                                .pool
                                .exact(d.bins().len(), |s| s.copy_from_slice(d.bins())),
                        };
                        let time = self.start as f64 / self.rate;
                        io.items_out::<ChirpSymbol>(0)
                            .push(Timed { time, value: item });
                    }
                    self.symbols += 1;
                    self.buf.clear();
                }
            }
        }
        if self.every.tick(n) && self.ratio > 0.0 {
            if self.ratio.is_finite() {
                self.results
                    .set(&keys::SNR_DB, 10.0 * (self.ratio as f64).log10());
            } else {
                self.results.none(&keys::SNR_DB);
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DechirpDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DechirpDemodSetting::Sf => self.sf = SF.read(id, v)? as u8,
            DechirpDemodSetting::BwHz => self.bw_hz = BW_HZ.read(id, v)?,
            DechirpDemodSetting::Direction => self.direction = DIRECTION.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (DechirpDemodSetting::Sf, Value::I64(self.sf as i64)),
            (DechirpDemodSetting::BwHz, Value::Hz(self.bw_hz)),
            (
                DechirpDemodSetting::Direction,
                Value::Choice(self.direction.index()),
            ),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}

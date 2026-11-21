//! Transmitter frequency stability meter (`tx_stability`): for every transmission of 1 s or more on one channel, the carrier
//! frequency against time: start and final frequency, key-up step, settling time, drift over the second half, mean offset
//! and scatter. TIA-603 and EN 300 086 name frequency stability; the rules here are confirmed by no public document. It
//! measures and decodes nothing; voice averages out of a reading only when it is symmetrical.

pub mod gen;

use crate::instrument::{Front, Log, SILENCE_DB};
use crate::txtest::{index_percentile, tx_stack, Ctx, TxTest};
use crate::typed::{
    TxStabilityFields, TxStabilityTransmissionEvent, TxStabilityTransmissionRow,
    TX_STABILITY_TRANSMISSION, TX_STABILITY_TRANSMISSIONS,
};
use sdrkit_algo_dsp::peak::line_fit;
use sdrkit_algo_dsp::plateau::settling_time;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "tx_stability";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 24e3;
/// The stretch dropped at the start while the filter fills, s; the clock starts there.
pub const SKIP_S: f64 = 0.1;
/// The length of a power block, s.
pub const BLOCK_S: f64 = 0.02;
/// The block powers the floor reads, s.
pub const FLOOR_WINDOW_S: f64 = 60.0;
/// The floor is first taken at this time, s.
pub const FLOOR_FROM_S: f64 = 1.0;
/// No decision before this time, s.
pub const DECIDE_FROM_S: f64 = 3.0;
/// The start frequency is the mean of the readings of this stretch after key-up, s.
pub const START_SPAN: (f64, f64) = (0.05, 0.2);
/// The final frequency is the mean of the readings of this last stretch, s.
pub const FINAL_SPAN_S: f64 = 0.4;
/// The least readings of an analysed transmission.
pub const MIN_READINGS: usize = 12;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures, per transmission of at least the shortest length, the carrier frequency against time: \
start and final frequency, key-up step, settling time, drift over the second half, mean offset and scatter; decodes nothing. \
Best with a carrier, a tone or a quiet transmission. The ppm value needs the tuned frequency. The rules are confirmed by no \
public document.";

/// The key-up level over the floor.
pub const ON_DB: Setting<field::F64> =
    Setting::new("on_db", "Key-up over the floor", "dB", 10.0).range(3.0, 30.0, 0.0);
/// The key-down level over the floor.
pub const OFF_DB: Setting<field::F64> =
    Setting::new("off_db", "Key-down under", "dB", 5.0).range(3.0, 30.0, 0.0);
/// The hang before key-down.
pub const HANG_S: Setting<field::F64> =
    Setting::new("hang_s", "Hang before key-down", "s", 0.3).range(0.05, 2.0, 0.0);
/// The shortest transmission analysed.
pub const MIN_TX_S: Setting<field::F64> =
    Setting::new("min_tx_s", "Shortest transmission analysed", "s", 1.0).range(0.5, 60.0, 0.0);
/// The least reading length.
pub const READING_S: Setting<field::F64> =
    Setting::new("reading_s", "Least reading length", "s", 0.05).range(0.02, 0.5, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The key-up level.
        OnDb = ON_DB,
        /// The key-down level.
        OffDb = OFF_DB,
        /// The hang.
        HangS = HANG_S,
        /// The shortest transmission.
        MinTxS = MIN_TX_S,
        /// The reading length.
        ReadingS = READING_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `tx_stability` stack.
    pub enum TxStabilitySetting {
        /// The key-up level.
        OnDb = ON_DB,
        /// The key-down level.
        OffDb = OFF_DB,
        /// The hang.
        HangS = HANG_S,
        /// The shortest transmission.
        MinTxS = MIN_TX_S,
        /// The reading length.
        ReadingS = READING_S,
        /// The transmissions of the test signal.
        GenKeyed = gen::GEN_KEYED,
        /// The final frequency.
        GenFinalHz = gen::GEN_FINAL_HZ,
        /// The start offset.
        GenStartOffsetHz = gen::GEN_START_OFFSET_HZ,
        /// The settling time constant.
        GenTauS = gen::GEN_TAU_S,
        /// The drift in a transmission.
        GenTxDriftHzS = gen::GEN_TX_DRIFT_HZ_S,
        /// The modulating tone.
        GenModHz = gen::GEN_MOD_HZ,
        /// The key ramps.
        GenRampS = gen::GEN_RAMP_S,
    }
}

/// A transmission in progress.
#[derive(Clone, Debug, Default)]
struct Tx {
    start: f64,
    hang_from: Option<f64>,
    sum: f64,
    count: usize,
    from: f64,
    readings: Vec<(f64, f64)>,
}

/// The measurement of `tx_stability`.
#[derive(Default)]
pub struct TxStability {
    front: Option<Front>,
    rate: f64,
    block: usize,
    on_db: f64,
    off_db: f64,
    hang: f64,
    min_tx: f64,
    reading: usize,
    t: f64,
    blocks: u64,
    acc: f64,
    fill: usize,
    step_sum: f64,
    prev: Option<Complex32>,
    powers: VecDeque<(f64, f64)>,
    floor: Option<f64>,
    tx: Option<Tx>,
    transmissions: i64,
    last: Option<TxStabilityTransmissionRow>,
    log: Log<TxStabilityTransmissionRow>,
}

impl TxStability {
    fn analyse(&mut self, tx: Tx, end: f64, c: &mut Ctx) {
        self.transmissions += 1;
        let len = end - tx.start;
        let r: Vec<(f64, f64)> = tx
            .readings
            .into_iter()
            .filter(|&(t, _)| t + 0.05 <= len)
            .collect();
        if len < self.min_tx || r.len() < MIN_READINGS {
            return;
        }
        let mean_of = |sel: &dyn Fn(f64) -> bool| {
            let v: Vec<f64> = r.iter().filter(|x| sel(x.0)).map(|x| x.1).collect();
            (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
        };
        let (Some(start), Some(fin)) = (
            mean_of(&|t| t >= START_SPAN.0 && t <= START_SPAN.1),
            mean_of(&|t| t >= len - FINAL_SPAN_S),
        ) else {
            return;
        };
        let mean = r.iter().map(|x| x.1).sum::<f64>() / r.len() as f64;
        let band = 10f64.max(0.1 * (start - fin).abs());
        let settle = settling_time(&r, fin, band, 0.05);
        let half: Vec<(f64, f64)> = r.iter().copied().filter(|x| x.0 >= len / 2.0).collect();
        let line = (half.len() > 3)
            .then(|| {
                let (x, y): (Vec<f64>, Vec<f64>) = half.iter().copied().unzip();
                line_fit(&x, &y, None).ok()
            })
            .flatten();
        let scatter = line.map(|l| {
            (half
                .iter()
                .map(|&(x, y)| (y - l.at(x)).powi(2))
                .sum::<f64>()
                / half.len() as f64)
                .sqrt()
        });
        let row = TxStabilityTransmissionRow {
            time_s: Seconds(tx.start),
            length_s: Seconds(len),
            readings: r.len() as i64,
            start_hz: Hertz(start),
            final_hz: Hertz(fin),
            keyup_step_hz: Hertz(start - fin),
            settle_s: Seconds(settle),
            settle_band_hz: Hertz(band),
            drift_hz_s: line.map(|l| l.slope),
            mean_hz: Hertz(mean),
            mean_ppm: c.tuned_hz.map(|f| mean / f * 1e6),
            scatter_hz: scatter.map(Hertz),
        };
        c.events.push(TX_STABILITY_TRANSMISSION.event(
            tx.start,
            c.dial(),
            &TxStabilityTransmissionEvent {
                time_s: row.time_s,
                length_s: row.length_s,
                readings: row.readings,
                start_hz: row.start_hz,
                final_hz: row.final_hz,
                keyup_step_hz: row.keyup_step_hz,
                settle_s: row.settle_s,
                settle_band_hz: row.settle_band_hz,
                drift_hz_s: row.drift_hz_s,
                mean_hz: row.mean_hz,
                mean_ppm: row.mean_ppm,
                scatter_hz: row.scatter_hz,
            },
        ));
        self.last = Some(row.clone());
        self.log.push(row);
    }

    fn block_done(&mut self, p_db: f64, steps: f64, n: usize, c: &mut Ctx) {
        let block_s = n as f64 / self.rate;
        let begin = self.t;
        self.t += block_s;
        self.blocks += 1;
        let t = self.t;
        if p_db > SILENCE_DB {
            self.powers.push_back((t, p_db));
        }
        while self
            .powers
            .front()
            .is_some_and(|&(a, _)| t - a > FLOOR_WINDOW_S)
        {
            self.powers.pop_front();
        }
        if t >= FLOOR_FROM_S
            && self.tx.is_none()
            && (self.floor.is_none() || self.blocks.is_multiple_of(50))
        {
            let mut v: Vec<f64> = self.powers.iter().map(|p| p.1).collect();
            if let Some(q) = index_percentile(&mut v, 0.1) {
                self.floor = Some(self.floor.map_or(q, |f| 0.9 * f + 0.1 * q));
            }
        }
        let Some(floor) = self.floor else { return };
        if t < DECIDE_FROM_S {
            return;
        }
        let over = p_db - floor;
        if self.tx.is_none() && over > self.on_db {
            self.tx = Some(Tx {
                start: begin,
                from: 0.0,
                ..Tx::default()
            });
        }
        let Some(tx) = self.tx.as_mut() else {
            if over > self.off_db {
                c.tally.energy(t);
            }
            return;
        };
        c.tally.good(t);
        tx.sum += steps;
        tx.count += n;
        if tx.count >= self.reading {
            let len = tx.count as f64 / self.rate;
            let f = tx.sum / tx.count as f64 * self.rate / TAU;
            tx.readings.push((tx.from + len / 2.0, f));
            tx.from += len;
            tx.sum = 0.0;
            tx.count = 0;
        }
        if over < self.off_db {
            let from = *tx.hang_from.get_or_insert(begin);
            if t - from >= self.hang {
                let tx = self.tx.take().unwrap_or_default();
                self.analyse(tx, from, c);
            }
        } else {
            tx.hang_from = None;
        }
    }
}

impl TxTest for TxStability {
    const ID: &'static str = ID;
    const NAME: &'static str = "tx_stability.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let d = (rate_hz / WORK_HZ).floor().max(1.0);
        let front = Front::new(rate_hz, WORK_HZ, 8000f64.min(0.45 * rate_hz / d), SKIP_S);
        self.rate = front.rate();
        self.block = ((BLOCK_S * self.rate).round() as usize).max(8);
        self.front = Some(front);
        self.on_db = v.get(&ON_DB);
        self.off_db = v.get(&OFF_DB);
        self.hang = v.get(&HANG_S);
        self.min_tx = v.get(&MIN_TX_S);
        self.reading = (v.get(&READING_S) * self.rate).round() as usize;
        self.t = self.t.max(SKIP_S);
        self.blocks = 0;
        self.acc = 0.0;
        self.fill = 0;
        self.step_sum = 0.0;
        self.prev = None;
        self.powers.clear();
        self.floor = None;
        self.tx = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(mut front) = self.front.take() else {
            return;
        };
        for &z in front.process(x) {
            self.acc += f64::from(z.norm_sqr());
            if let Some(p) = self.prev {
                let d = z * p.conj();
                self.step_sum += f64::from(d.im).atan2(f64::from(d.re));
            }
            self.prev = Some(z);
            self.fill += 1;
            if self.fill == self.block {
                let p = 10.0 * (self.acc / self.block as f64).max(1e-30).log10();
                let s = self.step_sum;
                self.acc = 0.0;
                self.step_sum = 0.0;
                self.fill = 0;
                self.block_done(p, s, self.block, c);
            }
        }
        self.front = Some(front);
    }
    fn clear(&mut self) {
        self.transmissions = 0;
        self.last = None;
        self.tx = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&TX_STABILITY_TRANSMISSIONS, p);
        let l = self.last.as_ref();
        p.put(&TxStabilityFields {
            transmitting: self.tx.is_some(),
            length_s: l.map(|r| r.length_s),
            readings: l.map(|r| r.readings),
            start_hz: l.map(|r| r.start_hz),
            final_hz: l.map(|r| r.final_hz),
            keyup_step_hz: l.map(|r| r.keyup_step_hz),
            settle_s: l.map(|r| r.settle_s),
            settle_band_hz: l.map(|r| r.settle_band_hz),
            drift_hz_s: l.and_then(|r| r.drift_hz_s),
            mean_hz: l.map(|r| r.mean_hz),
            mean_ppm: l.and_then(|r| r.mean_ppm),
            scatter_hz: l.and_then(|r| r.scatter_hz),
            transmissions: self.transmissions,
        });
        let now = if self.tx.is_some() {
            "transmission on"
        } else {
            "idle"
        };
        match l {
            Some(r) => format!(
                "{now}; last: {:.1} s, start {:+.1} Hz, final {:+.1} Hz, settled in {:.2} s",
                r.length_s.0, r.start_hz.0, r.final_hz.0, r.settle_s.0
            ),
            None if self.floor.is_none() => "measuring the floor".to_string(),
            None => format!("{now}; no transmission analysed"),
        }
    }
}

const KEYS: [&str; 5] = ["on_db", "off_db", "hang_s", "min_tx_s", "reading_s"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("three transmissions, two analysed", o, 0.0);
    c.seconds = o.seconds.max(24.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::instrument::number(r, TxStabilityFields::FINAL_HZ.key);
        vec![CheckLine::new(
            TxStabilityFields::FINAL_HZ.key,
            (f - 500.0).abs() < 25.0,
            format!("{f:.1} Hz, 500 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: TxStability,
    settings: TxStabilitySetting,
    keys: KEYS,
    name: "Transmitter frequency stability meter",
    blurb: "Per transmission: start and final frequency, key-up step, settling time, drift, mean offset and scatter.",
    tuning: (26e6, 6e9, 48e3, 16e3, true),
    rate_text: "The stability meter needs at least 24 ksps",
    generator: |cfg| gen::TxStabilityGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}

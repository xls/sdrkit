//! The LBJ modem: 1200 bit/s FSK on an FM channel, demodulated at 48 kHz, with the POCSAG sync search over all
//! 40 sample phases and the batch framing. The `lbj.modem` decoder.

use sdrkit_algo_dsp::fm::fast_atan2;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4};
use sdrkit_algo_dsp::kmeans::kmeans_1d_fixed;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::RationalResampler;
use sdrkit_core::Values;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{no_values, Decoder, Format, Item, Kind, Outbox, Port, Timed};
use sdrkit_proto_pager::pocsag::SYNC;
use std::f64::consts::TAU;

/// The demodulation rate, Hz.
pub const RATE: f64 = 48_000.0;
/// Samples per bit at [`RATE`].
pub const SPB: u64 = 40;
/// Samples at [`RATE`] between two level measurements.
pub const LEVEL_EVERY: u64 = 1024;
const HIST: usize = 1 << 16;
const BATCH_BITS: u64 = 544;

/// The codewords of one burst.
#[derive(Clone, Debug, PartialEq)]
pub struct LbjBurst {
    /// Signal time of the lock, seconds.
    pub time: f64,
    /// Level at the lock, dBFS.
    pub level_dbfs: f32,
    /// Carrier offset from the threshold of the first batch, Hz.
    pub carrier_offset_hz: f32,
    /// Errors in the sync word that locked.
    pub sync_errors: u32,
    /// The codewords of every batch, with the polarity corrected.
    pub codewords: Vec<u32>,
    /// The reliability of each bit (32 per codeword), 1.0 for a clean bit.
    pub rel: Vec<f32>,
}

/// What the modem passes on (`lbj.modem_item`): a burst, or the channel level at a moment of the signal.
#[derive(Clone, Debug, PartialEq)]
pub enum ModemItem {
    /// A burst that ended.
    Burst(LbjBurst),
    /// The channel level and SNR, measured every [`LEVEL_EVERY`] samples at [`RATE`].
    Level {
        /// Level, dBFS.
        level_dbfs: f32,
        /// SNR over the noise floor, dB.
        snr_db: f32,
    },
}

impl Item for ModemItem {
    const KIND: &'static str = "lbj.modem_item";
}

/// The place of the first hit and the hits of the window: (sample, errors, inverted).
type Pending = (u64, Vec<(u64, u32, bool)>);

#[derive(Clone, Debug)]
struct Lock {
    pos: u64,
    inverted: bool,
    thr: f32,
    level: f32,
    time: f64,
    sync_errors: u32,
    codewords: Vec<u32>,
    rel: Vec<f32>,
    offset: f32,
    batches: usize,
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The `lbj.modem` decoder: IQ in, [`ModemItem`]s out.
pub struct LbjModem {
    in_rate: f64,
    osc: Nco,
    resampler: Option<RationalResampler<Complex32>>,
    mixed: Vec<Complex32>,
    rs_out: Vec<Complex32>,
    lp_i: Butterworth4,
    lp_q: Butterworth4,
    smooth: Biquad,
    thr: f32,
    thr_a: f32,
    prev: Complex32,
    power: f64,
    floor: f64,
    level_db: f32,
    snr_db: f32,
    n: u64,
    hist: Vec<f32>,
    regs: [u32; 40],
    before: [u32; 40],
    pending: Option<Pending>,
    lock: Option<Lock>,
}

impl Default for LbjModem {
    fn default() -> Self {
        LbjModem {
            in_rate: 0.0,
            osc: Nco::new(1.0, 0.0),
            resampler: None,
            mixed: Vec::new(),
            rs_out: Vec::new(),
            lp_i: Butterworth4::lowpass(RATE, 6000.0),
            lp_q: Butterworth4::lowpass(RATE, 6000.0),
            smooth: Biquad::new(BiquadCoeffs::lowpass(
                RATE,
                1500.0,
                std::f64::consts::FRAC_1_SQRT_2,
            )),
            thr: 0.0,
            thr_a: (1.0 - (-1.0 / (RATE * 0.03)).exp()) as f32,
            prev: Complex32::new(0.0, 0.0),
            power: 0.0,
            floor: 1.0,
            level_db: -200.0,
            snr_db: 0.0,
            n: 0,
            hist: vec![0.0; HIST],
            regs: [0; 40],
            before: [0; 40],
            pending: None,
            lock: None,
        }
    }
}

impl LbjModem {
    /// The level (dBFS) and SNR (dB) measured last.
    pub fn level(&self) -> (f32, f32) {
        (self.level_db, self.snr_db)
    }

    fn mean5(&self, m: u64) -> f32 {
        let mut s = 0.0;
        for k in 0..5 {
            s += self.hist[((m - k) as usize) & (HIST - 1)];
        }
        s / 5.0
    }

    /// Bit 1 is the lower frequency.
    fn bit(v: f32, thr: f32, inverted: bool) -> u32 {
        ((v < thr) != inverted) as u32
    }

    fn word_at(&self, end: u64, thr: f32, inverted: bool) -> u32 {
        let mut w = 0u32;
        for i in 0..32u64 {
            let m = end - SPB * (31 - i);
            w = (w << 1) | Self::bit(self.mean5(m), thr, inverted);
        }
        w
    }

    fn end_burst(&mut self, out: &mut Vec<Timed<ModemItem>>) {
        if let Some(l) = self.lock.take() {
            if l.batches > 0 {
                out.push(Timed {
                    time: l.time,
                    value: ModemItem::Burst(LbjBurst {
                        time: l.time,
                        level_dbfs: l.level,
                        carrier_offset_hz: l.offset,
                        sync_errors: l.sync_errors,
                        codewords: l.codewords,
                        rel: l.rel,
                    }),
                });
            }
        }
        self.regs = [0; 40];
        self.before = [0; 40];
        self.pending = None;
    }

    fn read_batch(&mut self, out: &mut Vec<Timed<ModemItem>>) {
        let Some(l) = self.lock.as_ref() else { return };
        let (pos, inverted, lock_thr) = (l.pos, l.inverted, l.thr);
        let mut v = [0f32; 512];
        for (k, x) in v.iter_mut().enumerate() {
            *x = self.mean5(pos + SPB * (k as u64 + 1));
        }
        // The threshold of the batch: the midpoint of the two clusters, by up to 3 rounds of two-means while each holds 20
        // values.
        let mut thr = lock_thr;
        let (mut hi_m, mut lo_m) = (thr + 0.1, thr - 0.1);
        for _ in 0..3 {
            let Some(r) = kmeans_1d_fixed(&v, [f64::from(lo_m), f64::from(hi_m)], 1) else {
                break;
            };
            if r.counts[0] < 20 || r.counts[1] < 20 {
                break;
            }
            lo_m = r.centres[0] as f32;
            hi_m = r.centres[1] as f32;
            thr = (hi_m + lo_m) / 2.0;
        }
        let half = ((hi_m - lo_m) / 2.0).abs().max(1e-6);
        let Some(l) = self.lock.as_mut() else { return };
        if l.batches == 0 {
            l.offset = (thr as f64 * RATE / TAU) as f32;
        }
        for c in 0..16 {
            let mut w = 0u32;
            for b in 0..32 {
                let x = v[c * 32 + b];
                w = (w << 1) | Self::bit(x, thr, inverted);
                l.rel.push(((x - thr).abs() / half).min(2.0));
            }
            l.codewords.push(w);
        }
        l.batches += 1;
        let batches = l.batches;
        // The next sync word, within 3 samples of where it must end.
        let base = pos + SPB * BATCH_BITS;
        let mut best = (u32::MAX, base);
        for d in -3i64..=3 {
            let q = (base as i64 + d) as u64;
            let e = (self.word_at(q, thr, inverted) ^ SYNC).count_ones();
            if e < best.0 {
                best = (e, q);
            }
        }
        match self.lock.as_mut() {
            Some(l) if best.0 <= 6 && batches < 6 => l.pos = best.1,
            _ => self.end_burst(out),
        }
    }

    fn sample(&mut self, y: Complex32, out: &mut Vec<Timed<ModemItem>>) {
        // Channel filter.
        let z = Complex32::new(
            self.lp_i.process_sample(y.re),
            self.lp_q.process_sample(y.im),
        );
        let p = z * self.prev.conj();
        let d = fast_atan2(p.im, p.re);
        self.prev = z;
        let s = self.smooth.process_sample(d);
        self.thr += self.thr_a * (s - self.thr);
        // Level and SNR.
        let pw = z.norm_sqr() as f64;
        self.power += (pw - self.power) / 2400.0;
        if self.power < self.floor {
            self.floor = self.power;
        } else {
            self.floor += 1e-5 * (self.power - self.floor);
        }
        let n = self.n;
        if n.is_multiple_of(LEVEL_EVERY) {
            self.level_db = sdrkit_algo_dsp::meter::power_to_db(self.power) as f32;
            self.snr_db =
                sdrkit_algo_dsp::meter::power_to_db(self.power / self.floor.max(1e-30)) as f32;
            out.push(Timed {
                time: n as f64 / RATE,
                value: ModemItem::Level {
                    level_dbfs: self.level_db,
                    snr_db: self.snr_db,
                },
            });
        }
        self.hist[(n as usize) & (HIST - 1)] = s;
        if n >= 4 {
            // The slicer of this sample phase.
            let ph = (n % SPB) as usize;
            let bit = Self::bit(self.mean5(n), self.thr, false);
            self.before[ph] = (self.before[ph] << 1) | (self.regs[ph] >> 31);
            self.regs[ph] = (self.regs[ph] << 1) | bit;
            if self.lock.is_none() && n >= 4000 {
                self.search(n, ph);
            }
        }
        let due = matches!(&self.pending, Some((start, _)) if n >= start + 12);
        if due && self.lock.is_none() {
            if let Some((_, hits)) = self.pending.take() {
                self.lock_on(hits);
            }
        }
        if let Some(l) = self.lock.as_ref() {
            if n >= l.pos + SPB * BATCH_BITS + 3 + 4 {
                self.read_batch(out);
            }
        }
        self.n += 1;
    }

    /// The sync search at sample `n` of phase `ph`.
    fn search(&mut self, n: u64, ph: usize) {
        let r = self.regs[ph];
        let e = (r ^ SYNC).count_ones();
        let pre = |w: u32| {
            (w ^ 0xAAAA_AAAA)
                .count_ones()
                .min((w ^ 0x5555_5555).count_ones())
                <= 6
        };
        let pre_ok = pre(self.before[ph]);
        let hit = |e: u32| e <= 2 || (e <= 5 && pre_ok);
        let found = if hit(e) {
            Some((e, false))
        } else if hit(32 - e) {
            Some((32 - e, true))
        } else {
            None
        };
        if let Some((e, inv)) = found {
            match self.pending.as_mut() {
                None => self.pending = Some((n, vec![(n, e, inv)])),
                Some((_, v)) => v.push((n, e, inv)),
            }
        }
    }

    /// Locks on the fewest-error hits of one polarity at the end of the pending window.
    fn lock_on(&mut self, hits: Vec<(u64, u32, bool)>) {
        let Some(best) = hits.iter().min_by_key(|h| h.1).copied() else {
            return;
        };
        let mut pos: Vec<u64> = hits
            .iter()
            .filter(|h| h.1 == best.1 && h.2 == best.2)
            .map(|h| h.0)
            .collect();
        pos.sort_unstable();
        let centre = pos[pos.len() / 2];
        self.lock = Some(Lock {
            pos: centre,
            inverted: best.2,
            thr: self.thr,
            level: self.level_db,
            time: centre as f64 / RATE,
            sync_errors: best.1,
            codewords: Vec::new(),
            rel: Vec::new(),
            offset: 0.0,
            batches: 0,
        });
    }

    fn reset_sync(&mut self) {
        self.lock = None;
        self.pending = None;
        self.regs = [0; 40];
        self.before = [0; 40];
    }
}

impl Decoder for LbjModem {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "lbj.modem"
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<ModemItem>("out")]
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        *self = LbjModem::default();
        self.in_rate = f.rate;
        self.osc = Nco::new(f.rate, f.centre_hz - f.tuned_hz);
        let r = f.rate.round() as u64;
        if r != RATE as u64 {
            let g = gcd(48_000, r);
            self.resampler = Some(RationalResampler::with_spec(
                (48_000 / g) as usize,
                (r / g) as usize,
                0.2,
                0.8,
                40.0,
            )?);
        }
        Ok(vec![Format::new(Kind::typed::<ModemItem>(), 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        // The channel follows the dial. A burst half received belongs to the old frequency.
        self.osc = Nco::new(self.in_rate, f.centre_hz - f.tuned_hz);
        self.reset_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.osc.mix_in_place(&mut self.mixed);
        let mut rs = std::mem::take(&mut self.rs_out);
        rs.clear();
        match self.resampler.as_mut() {
            Some(r) => r.process(&self.mixed, &mut rs),
            None => rs.extend_from_slice(&self.mixed),
        }
        let items = out.items::<ModemItem>(0);
        for &y in &rs {
            self.sample(y, items);
        }
        self.rs_out = rs;
    }
}

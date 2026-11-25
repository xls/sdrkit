//! The test signal generator: the station lists of every mode, a single-station source and the JS8 stations, as clean
//! complex baseband with the dial at 0 Hz (upper sideband: an audio tone at f Hz lies f Hz above it). The strongest station has
//! unit power. Noise, offsets and fading come from the framework's `gen_*` settings.

use crate::encode::{self, frame_tones, EncodeError};
use crate::mode::FskSpec;
use crate::mode::{Mode, Sub};
use crate::proto::js8msg::{self, Outgoing};
use num_complex::Complex32;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_core::{GenConfig, Generator, Hz, SignalInfo};

/// A station of a list: text, audio Hz of tone 0, level dB (relative to the strongest), DT s.
pub type Station = (&'static str, f64, f64, f64);

/// The station list of a mode.
pub fn stations(mode: Mode) -> &'static [Station] {
    match mode {
        Mode::Ft8 => &[
            ("CQ K1ABC FN42", 600.0, 0.0, 0.0),
            ("K1ABC W9XYZ -12", 900.0, -6.0, 0.3),
            ("W9XYZ K1ABC R-05", 1250.0, -10.0, -0.4),
            ("CQ PJ4/K1ABC", 1500.0, -3.0, 0.1),
            ("TNX BOB 73 GL", 1800.0, -12.0, 0.6),
            ("G4ABC DL1XYZ RR73", 2100.0, -8.0, 0.0),
            ("CQ JA1XYZ PM95", 2450.0, -14.0, 1.0),
            ("7123456789ABCDEF01", 2800.0, -9.0, -0.2),
        ],
        Mode::Ft4 => &[
            ("CQ K1ABC FN42", 700.0, 0.0, 0.0),
            ("K1ABC W9XYZ EN37", 1100.0, -5.0, 0.2),
            ("W9XYZ K1ABC +03", 1600.0, -8.0, -0.3),
            ("CQ TEST DL1XYZ JO62", 2100.0, -3.0, 0.0),
            ("VK2ABC ZL1XYZ 73", 2600.0, -10.0, 0.5),
        ],
        Mode::Ft2 => &[
            ("CQ K1ABC FN42", 800.0, 0.0, 0.0),
            ("K1ABC W9XYZ -10", 1400.0, -4.0, 0.1),
            ("W9XYZ K1ABC RRR", 2200.0, -7.0, -0.1),
        ],
        Mode::Wspr => &[
            ("K1ABC FN42 37", 1450.0, 0.0, 0.0),
            ("G4XYZ IO91 23", 1480.0, -6.0, 0.5),
            ("PJ4/K1ABC 30", 1520.0, -10.0, -0.5),
            ("DL1XYZ/P 10", 1550.0, -4.0, 1.0),
            ("<K1ABC> FN42AX 37", 1580.0, -8.0, 0.2),
        ],
        Mode::Jt65 | Mode::Jt9 => &[
            ("CQ K1ABC FN42", 600.0, 0.0, 0.0),
            ("K1ABC W9XYZ -12", 1000.0, -4.0, 0.3),
            ("W9XYZ K1ABC R-05", 1400.0, -8.0, -0.4),
            ("CQ DX G4ABC IO91", 1800.0, -3.0, 0.1),
            ("TNX BOB 73 GL", 2200.0, -10.0, 0.6),
        ],
        Mode::Jt4 => &[
            ("CQ K1ABC FN42", 600.0, 0.0, 0.0),
            ("K1ABC W9XYZ -12", 1000.0, -4.0, 0.3),
            ("W9XYZ K1ABC R-05", 1400.0, -8.0, -0.4),
            ("CQ DX G4ABC IO91", 1800.0, -3.0, 0.1),
        ],
        Mode::Q65 => &[
            ("CQ K1ABC FN42", 700.0, 0.0, 0.0),
            ("K1ABC W9XYZ -12", 1200.0, -4.0, 0.3),
            ("W9XYZ K1ABC RRR", 1700.0, -8.0, -0.4),
            ("CQ DX G4ABC IO91", 2200.0, -3.0, 0.1),
        ],
        Mode::Fst4 => &[
            ("CQ K1ABC FN42", 700.0, 0.0, 0.0),
            ("K1ABC W9XYZ -12", 1100.0, -4.0, 0.3),
            ("W9XYZ K1ABC RRR", 1500.0, -8.0, -0.4),
            ("CQ DX G4ABC IO91", 2000.0, -3.0, 0.1),
            ("TNX BOB 73 GL", 2500.0, -6.0, 0.2),
        ],
        Mode::Fst4w => &[
            ("K1ABC FN42 37", 1400.0, 0.0, 0.2),
            ("G4XYZ IO91 23", 1700.0, -3.0, -0.4),
        ],
        Mode::Msk144 => &[("CQ K1ABC FN42", 1500.0, 0.0, 0.0)],
        Mode::Js8 => &[],
    }
}

/// One transmission repeated every period, made as it streams by the shared CPM modulator at a base rate.
struct Tx {
    tones: Vec<u8>,
    f0: f64,
    amp: f32,
    /// Start in the period, samples at the base rate (negative: it began in the previous period).
    start: i64,
    len: usize,
    /// The modulator at the start of a transmission, and the one in use.
    fresh: Option<Cpm>,
    cpm: Option<Cpm>,
    /// Samples made and not yet taken: `buf[0]` is sample `buf_at` of the transmission.
    buf: Vec<Complex32>,
    buf_at: usize,
    /// Symbols pushed so far, and whether the burst was flushed.
    pushed: usize,
    flushed: bool,
    mirrored: bool,
    /// Hz per second of drift (JS8).
    drift: f64,
    /// Periods it sends in (JS8): (first, every; 0 = once).
    first: i64,
    every: i64,
}

impl Tx {
    #[allow(clippy::too_many_arguments)]
    fn new(
        spec: FskSpec,
        tones: Vec<u8>,
        f0: f64,
        amp: f32,
        start_s: f64,
        rate: f64,
        mirrored: bool,
        msk_len_s: f64,
    ) -> Tx {
        let len = if msk_len_s > 0.0 {
            (msk_len_s * rate) as usize
        } else {
            (tones.len() as f64 * spec.symbol_s * rate).round() as usize
        };
        // A spec out of range makes a silent transmission.
        let fresh = spec.modulator(rate).ok();
        Tx {
            tones,
            f0,
            amp,
            start: (start_s * rate).round() as i64,
            len,
            cpm: fresh.clone(),
            fresh,
            buf: Vec::new(),
            buf_at: 0,
            pushed: 0,
            flushed: false,
            mirrored,
            drift: 0.0,
            first: 0,
            every: 1,
        }
    }

    /// The sample at position `u` of the transmission (sequential calls), at `rate`.
    fn at(&mut self, u: usize, rate: f64) -> Complex32 {
        if u == 0 {
            self.cpm.clone_from(&self.fresh);
            self.buf.clear();
            self.buf_at = 0;
            self.pushed = 0;
            self.flushed = false;
        }
        let Some(cpm) = self.cpm.as_mut() else {
            return Complex32::default();
        };
        while u >= self.buf_at + self.buf.len() {
            if self.flushed {
                return Complex32::default();
            }
            self.buf_at += self.buf.len();
            self.buf.clear();
            if self.pushed < self.tones.len() {
                cpm.push(&[self.tones[self.pushed] as f32], &mut self.buf);
                self.pushed += 1;
            } else {
                cpm.flush(&mut self.buf);
                self.flushed = true;
            }
        }
        // The carrier, with a frequency drift of `drift` Hz per second: phase 2 pi (f0 t + drift t^2 / 2).
        let t = u as f64 / rate;
        let phase = std::f64::consts::TAU * (self.f0 * t + 0.5 * self.drift * t * t);
        let (s, c) = phase.sin_cos();
        let z = self.buf[u - self.buf_at] * Complex32::new(c as f32, s as f32);
        let z = if self.mirrored { z.conj() } else { z };
        z * self.amp
    }
}

/// The stations of a mode, as complex baseband at a base rate, slot after slot from sample 0 = the start of a slot.
pub struct StationSource {
    sub: Sub,
    txs: Vec<Tx>,
    rate: f64,
    pos: u64,
    period_n: u64,
}

impl StationSource {
    /// The base rate.
    pub const RATE: f64 = 12000.0;

    /// The station list of `sub` (levels relative to the strongest at amplitude `peak`), upside down when `mirrored`. An error
    /// when a text of the list cannot be packed.
    pub fn new(sub: Sub, peak: f32, mirrored: bool) -> Result<StationSource, EncodeError> {
        let list: Vec<(String, f64, f64, f64)> = stations(sub.mode)
            .iter()
            .map(|s| (s.0.to_string(), s.1, s.2, s.3))
            .collect();
        Self::from_list(sub, &list, peak, mirrored, false)
    }

    /// A list of (text, Hz, level dB, DT). An MSK144 short message `<CALL1 CALL2> RPT` is sent as the 40-bit short
    /// frame when `short` is set. Refuses a text that cannot be packed.
    pub fn from_list(
        sub: Sub,
        list: &[(String, f64, f64, f64)],
        peak: f32,
        mirrored: bool,
        short: bool,
    ) -> Result<StationSource, EncodeError> {
        let rate = Self::RATE;
        let mut txs = Vec::new();
        for (text, hz, db, dt) in list {
            let syms = if sub.mode == Mode::Msk144 {
                encode::msk144_symbols(text, short)?
            } else {
                encode::channel_symbols(&sub, text)?
            };
            let amp = peak * 10f32.powf(*db as f32 / 20.0);
            let msk = if sub.mode == Mode::Msk144 {
                sub.period - 0.1
            } else {
                0.0
            };
            let mut spec = sub.fsk();
            if sub.mode == Mode::Msk144 {
                spec.spacing_hz = 1000.0;
            }
            let tones = frame_tones(&sub, &syms);
            let f0 = if sub.mode == Mode::Msk144 {
                hz - 500.0
            } else {
                *hz
            };
            let tones = if sub.mode == Mode::Msk144 {
                let reps = encode::msk_frames(&sub, tones.len(), msk);
                tones
                    .iter()
                    .cycle()
                    .take(reps * tones.len())
                    .copied()
                    .collect()
            } else {
                tones
            };
            txs.push(Tx::new(
                spec,
                tones,
                f0,
                amp,
                sub.t0() + dt,
                rate,
                mirrored,
                msk,
            ));
        }
        Ok(StationSource {
            sub,
            txs,
            rate,
            pos: 0,
            period_n: (sub.period * rate).round() as u64,
        })
    }

    /// Starts at `offset_s` seconds into a slot (a live demonstration in step with the clock).
    pub fn skip_to(&mut self, offset_s: f64) {
        self.pos = (offset_s.rem_euclid(self.sub.period) * self.rate) as u64;
    }

    /// The next sample.
    pub fn next_sample(&mut self) -> Complex32 {
        let n = self.period_n as i64;
        let period = (self.pos / self.period_n) as i64;
        let q = (self.pos % self.period_n) as i64;
        let mut acc = Complex32::default();
        for tx in self.txs.iter_mut() {
            let u = (q - tx.start).rem_euclid(n);
            if (u as usize) < tx.len {
                // Which period's transmission this is (it can begin in the previous one).
                let p = if q - tx.start < 0 { period - 1 } else { period };
                if p >= tx.first
                    && (tx.every > 0 && (p - tx.first) % tx.every == 0
                        || tx.every == 0 && p == tx.first)
                {
                    acc += tx.at(u as usize, self.rate);
                }
            }
        }
        self.pos += 1;
        acc
    }

    /// Audio (the real part) of `n` samples at the base rate, for tests.
    pub fn audio(&mut self, n: usize) -> Vec<f32> {
        (0..n)
            .map(|_| self.next_sample().re.clamp(-1.0, 1.0))
            .collect()
    }
}

/// A JS8 station of the generator.
#[derive(Clone, Debug)]
pub struct Js8Station {
    /// What it sends.
    pub msg: Outgoing,
    /// Sub-mode.
    pub submode: u8,
    /// Audio frequency of tone 0.
    pub hz: f64,
    /// Level, dB (the strongest station of the list is sent at unit power).
    pub snr: f64,
    /// DT.
    pub dt: f64,
    /// Drift, Hz per minute.
    pub drift: f64,
    /// First slot.
    pub first: i64,
    /// Again every N slots after its last frame (0: once).
    pub every: i64,
}

/// The default JS8 stations.
pub fn js8_stations() -> Vec<Js8Station> {
    let st = |msg, submode, hz, snr, dt, first, every| Js8Station {
        msg,
        submode,
        hz,
        snr,
        dt,
        drift: 0.0,
        first,
        every,
    };
    vec![
        st(
            Outgoing::Heartbeat {
                call: "LY2ABC".into(),
                grid: "KO24".into(),
                cq: None,
            },
            0,
            600.0,
            -8.0,
            0.0,
            0,
            1,
        ),
        st(
            Outgoing::Heartbeat {
                call: "DL1XYZ".into(),
                grid: "JO62".into(),
                cq: Some(0),
            },
            0,
            1100.0,
            -12.0,
            0.4,
            0,
            4,
        ),
        st(
            Outgoing::Command {
                from: "OK1XX".into(),
                to: "LY2ABC".into(),
                cmd: " SNR".into(),
                num: -12,
            },
            0,
            1500.0,
            -6.0,
            0.0,
            1,
            4,
        ),
        st(
            Outgoing::Text {
                from: "G4ABC".into(),
                to: "DL1XYZ".into(),
                text: "HELLO FROM LONDON. HOW COPY?".into(),
            },
            0,
            1900.0,
            -4.0,
            -0.3,
            0,
            2,
        ),
        st(
            Outgoing::Relay {
                from: "EA3XYZ".into(),
                via: "OK1XX".into(),
                dest: "SP5AB".into(),
                text: "PLEASE RELAY. TNX".into(),
            },
            0,
            2400.0,
            -6.0,
            0.0,
            0,
            3,
        ),
        st(
            Outgoing::Heartbeat {
                call: "YL2XYZ".into(),
                grid: "KO26".into(),
                cq: None,
            },
            1,
            2800.0,
            -2.0,
            0.0,
            0,
            1,
        ),
        st(
            Outgoing::Heartbeat {
                call: "ES1ABC".into(),
                grid: "KO29".into(),
                cq: Some(7),
            },
            2,
            350.0,
            0.0,
            0.0,
            1,
            1,
        ),
    ]
}

/// The JS8 generator: the stations, each sending its message in its sub-mode's slots and again every N slots after its last frame.
pub struct Js8Source {
    srcs: Vec<StationSource>,
}

impl Js8Source {
    /// The stations, the strongest at amplitude `peak` and the others at their levels below it.
    pub fn new(list: &[Js8Station], peak: f32) -> Result<Js8Source, EncodeError> {
        let rate = StationSource::RATE;
        let top = list.iter().map(|s| s.snr).fold(f64::MIN, f64::max);
        let mut srcs = Vec::new();
        for s in list {
            let sub = Sub::new(Mode::Js8).with_sub(s.submode);
            let frames = js8msg::frames_of(&s.msg)?;
            let amp = peak * 10f64.powf((s.snr - top) / 20.0) as f32;
            // A message of n frames repeats every n - 1 + every slots.
            let n = frames.len() as i64;
            let cycle = if s.every > 0 { n - 1 + s.every } else { 0 };
            let mut txs = Vec::new();
            for (i, (f, flags)) in frames.iter().enumerate() {
                let tones = encode::js8_tones(s.submode, &f.pack()?, *flags);
                let mut tx = Tx::new(
                    sub.fsk(),
                    tones,
                    s.hz,
                    amp,
                    sub.t0() + s.dt,
                    rate,
                    false,
                    0.0,
                );
                tx.drift = s.drift / 60.0;
                tx.first = s.first + i as i64;
                tx.every = cycle;
                txs.push(tx);
            }
            srcs.push(StationSource {
                sub,
                txs,
                rate,
                pos: 0,
                period_n: (sub.period * rate).round() as u64,
            });
        }
        Ok(Js8Source { srcs })
    }

    /// The next sample.
    pub fn next_sample(&mut self) -> Complex32 {
        let mut acc = Complex32::default();
        for s in self.srcs.iter_mut() {
            acc += s.next_sample();
        }
        acc
    }
}

enum Src {
    Stations(StationSource),
    Js8(Js8Source),
    /// Nothing: the mode cannot be encoded (its code is missing).
    Silent,
}

impl Src {
    fn next(&mut self) -> Complex32 {
        match self {
            Src::Stations(s) => s.next_sample(),
            Src::Js8(s) => s.next_sample(),
            Src::Silent => Complex32::default(),
        }
    }
}

/// The generator of a mode: complex baseband at any rate (the dial at 0 Hz), from the base-rate stations through a resampler.
pub struct WsjtGenerator {
    src: Src,
    sub: Sub,
    rate: Hz,
    res: Option<Resampler<Complex32>>,
    base: Vec<Complex32>,
    out: Vec<Complex32>,
    pos: usize,
    /// Why nothing is sent (empty when the signal is made).
    refusal: String,
}

impl WsjtGenerator {
    /// The station list of `sub` at `rate`, upside down when `mirrored`; the first sample is a slot start (see
    /// [`WsjtGenerator::start_at`]). An error when a text of the list cannot be packed.
    pub fn new(sub: Sub, rate: Hz, mirrored: bool) -> Result<WsjtGenerator, EncodeError> {
        let s = StationSource::new(sub, 1.0, mirrored)?;
        Ok(Self::with(Src::Stations(s), sub, rate))
    }

    /// Puts the first sample at `utc` (seconds since 1970), so the slots follow that clock as on a real band. Call it
    /// before the first [`Generator::generate`].
    pub fn start_at(&mut self, utc: f64) {
        match &mut self.src {
            Src::Stations(s) => s.skip_to(utc),
            Src::Js8(j) => j.srcs.iter_mut().for_each(|s| s.skip_to(utc)),
            Src::Silent => {}
        }
    }

    /// A single station repeating one message every slot; an MSK144 short message is sent as the short frame when
    /// `short` is set. Refuses a text that cannot be packed.
    pub fn single(
        sub: Sub,
        rate: Hz,
        text: &str,
        hz: f64,
        short: bool,
    ) -> Result<WsjtGenerator, EncodeError> {
        let list = [(text.to_string(), hz, 0.0, 0.0)];
        let s = StationSource::from_list(sub, &list, 1.0, false, short)?;
        Ok(Self::with(Src::Stations(s), sub, rate))
    }

    /// The JS8 generator.
    pub fn js8(rate: Hz, list: &[Js8Station]) -> Result<WsjtGenerator, EncodeError> {
        Ok(Self::with(
            Src::Js8(Js8Source::new(list, 1.0)?),
            Sub::new(Mode::Js8),
            rate,
        ))
    }

    /// A generator that sends nothing, and why.
    pub fn silent(sub: Sub, rate: Hz, why: String) -> WsjtGenerator {
        let mut g = Self::with(Src::Silent, sub, rate);
        g.refusal = why;
        g
    }

    fn with(src: Src, sub: Sub, rate: Hz) -> WsjtGenerator {
        let res = ((rate - StationSource::RATE).abs() > 1e-6)
            .then(|| Resampler::new(StationSource::RATE, rate).ok())
            .flatten();
        WsjtGenerator {
            src,
            sub,
            rate,
            res,
            base: Vec::new(),
            out: Vec::new(),
            pos: 0,
            refusal: String::new(),
        }
    }

    /// Why nothing is sent (empty when the signal is made).
    pub fn refusal(&self) -> &str {
        &self.refusal
    }

    /// From a generator configuration: the mode's sub-mode and the `gen_*` settings of the mode.
    pub fn from_config(mode: Mode, cfg: &GenConfig) -> Box<dyn Generator> {
        use crate::settings::{GEN_HZ, GEN_LIVE, GEN_MIRRORED, GEN_SINGLE, MSK144_SHORT};
        let s = &cfg.settings;
        let sub = crate::settings::sub_of(mode, s, cfg.submode.as_deref());
        let made = if mode == Mode::Js8 {
            WsjtGenerator::js8(cfg.rate_hz, &js8_stations())
        } else {
            let single = s.get(&GEN_SINGLE);
            if single.is_empty() {
                WsjtGenerator::new(sub, cfg.rate_hz, s.get(&GEN_MIRRORED))
            } else {
                let short = mode == Mode::Msk144 && s.get(&MSK144_SHORT);
                WsjtGenerator::single(sub, cfg.rate_hz, single, s.get(&GEN_HZ), short)
            }
        };
        match made {
            Ok(mut g) => {
                if s.get(&GEN_LIVE) {
                    // The band of a live test signal follows the system clock, as the air does.
                    g.start_at(crate::slots::system_clock().now());
                }
                Box::new(g)
            }
            Err(e) => Box::new(Self::silent(sub, cfg.rate_hz, e.to_string())),
        }
    }
}

impl Generator for WsjtGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    /// The SNR convention of the family is the power of the strongest station in 2500 Hz.
    fn signal_info(&self) -> Option<SignalInfo> {
        if matches!(self.src, Src::Silent) {
            return None;
        }
        let baud = 1.0 / self.sub.symbol_s();
        Some(
            SignalInfo::new(2500.0)
                .with_symbol_rate(baud)
                .with_power(1.0),
        )
    }
    /// The dial frequency is at 0 Hz, the tones above it.
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.pos < self.out.len() {
                let n = (self.out.len() - self.pos).min(out.len() - w);
                out[w..w + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
                self.pos += n;
                w += n;
                continue;
            }
            self.out.clear();
            self.pos = 0;
            self.base.clear();
            for _ in 0..1200 {
                self.base.push(self.src.next());
            }
            match self.res.as_mut() {
                Some(r) => r.process(&self.base, &mut self.out),
                None => self.out.extend_from_slice(&self.base),
            }
        }
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_list_packs() {
        for m in Mode::ALL.into_iter().filter(|&m| m != Mode::Js8) {
            assert!(StationSource::new(Sub::new(m), 1.0, false).is_ok(), "{m:?}");
        }
        assert!(StationSource::from_list(
            Sub::new(Mode::Ft8),
            &[("K1ABC/ABCD W9XYZ".into(), 1000.0, 0.0, 0.0)],
            1.0,
            false,
            false,
        )
        .is_err());
        assert!(Js8Source::new(&js8_stations(), 1.0).is_ok());
    }

    #[test]
    fn the_generator_makes_any_rate() {
        let mut g = WsjtGenerator::new(Sub::new(Mode::Ft8), 48000.0, false).unwrap();
        let mut buf = vec![Complex32::default(); 48000];
        assert_eq!(g.generate(&mut buf), 48000);
        let p: f32 = buf[30000..].iter().map(|c| c.norm_sqr()).sum::<f32>() / 18000.0;
        assert!(p > 0.5, "{p}");
    }
}

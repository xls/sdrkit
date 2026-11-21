//! The 2G ALE test signal generator: calls and their answers as continuous-phase 8-FSK on the upper sideband
//! of a dial at 0 Hz, with channel bit errors when requested.

use super::word::{self, address_words, amd_words, Lqa, THRU, TIS, TO, TWS};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use std::f64::consts::{PI, TAU};

/// One transmission: its words.
#[derive(Clone, Debug, PartialEq)]
pub struct Transmission {
    pub words: Vec<u32>,
}

fn scan(t: u8, addr: &str, times: usize) -> Vec<u32> {
    let mut v = Vec::new();
    for _ in 0..times {
        v.extend(address_words(t, addr));
    }
    v
}

/// A sounding: the caller in TWS (or TIS) words, repeated.
pub fn sounding(caller: &str, tis: bool, repeats: usize) -> Vec<Transmission> {
    vec![Transmission {
        words: scan(if tis { TIS } else { TWS }, caller, repeats),
    }]
}

/// One transmission of a call or an answer: the address `to` in TO words `times` times, the LQA word, the AMD words,
/// then the address `from` in words of type `conclusion` (TIS or TWS).
pub fn frame_words(
    to: &str,
    times: usize,
    lqa: Option<Lqa>,
    amd: Option<&str>,
    conclusion: u8,
    from: &str,
) -> Vec<u32> {
    let mut w = scan(TO, to, times);
    if let Some(q) = lqa {
        w.push(q.word());
    }
    if let Some(t) = amd {
        w.extend(amd_words(t));
    }
    w.extend(address_words(conclusion, from));
    w
}

/// An individual call (or AMD, or allcall with `to` = "@?@"), with its answer and acknowledgement when `answered`.
pub fn individual(
    caller: &str,
    to: &str,
    lqa: bool,
    amd: Option<&str>,
    answered: bool,
    scan_words: usize,
) -> Vec<Transmission> {
    let allcall = to == "@?@" || to == "@@?";
    let q = |ber, sinad, mp| lqa.then_some(Lqa { ber, sinad, mp });
    let concl = if allcall { TWS } else { TIS };
    let mut v = vec![Transmission {
        words: frame_words(to, scan_words, q(2, 24, 1), amd, concl, caller),
    }];
    if answered && !allcall {
        v.push(Transmission {
            words: frame_words(caller, 2, q(1, 26, 0), None, TIS, to),
        });
        v.push(Transmission {
            words: frame_words(to, 2, None, None, TWS, caller),
        });
    }
    v
}

/// A net call: the call to the net address, each member's answer, the caller's closing.
pub fn net_call(caller: &str, net: &str, members: &[&str], scan_words: usize) -> Vec<Transmission> {
    let mut v = individual(caller, net, false, None, false, scan_words);
    for m in members {
        let mut r = scan(TO, caller, 2);
        r.extend(address_words(TIS, m));
        v.push(Transmission { words: r });
    }
    let mut c = scan(TO, net, 2);
    c.extend(address_words(TWS, caller));
    v.push(Transmission { words: c });
    v
}

/// A group call: THRU words for each member, repeated scan / 2 times, then TIS caller.
pub fn group_call(caller: &str, members: &[&str], scan_words: usize) -> Vec<Transmission> {
    let mut w = Vec::new();
    for _ in 0..(scan_words / 2).max(1) {
        for m in members {
            w.extend(address_words(THRU, m));
        }
    }
    w.extend(address_words(TIS, caller));
    vec![Transmission { words: w }]
}

/// The test calls of the built-in source (neutral test addresses).
pub fn test_calls() -> Vec<Transmission> {
    let mut v = sounding("TSTA", false, 4);
    v.extend(individual("TSTA", "TSTB", true, None, true, 6));
    v.extend(net_call("NETC", "NET", &["MEMA", "MEMB"], 6));
    v.extend(individual(
        "TSTD",
        "TSTA",
        false,
        Some("QSY 14109 USB PSE"),
        false,
        6,
    ));
    v.extend(group_call("GRPE", &["GRPF", "GRPG"], 4));
    v.extend(sounding("LONGADDR1", true, 3));
    v
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct AleGenOptions {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The share of channel bits turned over before they become tones (0 to 0.2).
    pub ber: f64,
    /// The seed of the bit errors.
    pub seed: u64,
    /// Starts again after 3 s of silence.
    pub repeat: bool,
    /// The transmissions, one after another.
    pub calls: Vec<Transmission>,
    /// Seconds of silence after the last transmission when not repeating.
    pub tail_s: f64,
}

impl Default for AleGenOptions {
    fn default() -> Self {
        AleGenOptions {
            rate: 24_000.0,
            ber: 0.0,
            seed: 1,
            repeat: false,
            calls: test_calls(),
            tail_s: 1.0,
        }
    }
}

struct Tx {
    start: f64,
    tones: Vec<u8>,
}

/// The generator: unit mean power while a transmission is on.
pub struct AleGenerator {
    o: AleGenOptions,
    txs: Vec<Tx>,
    total: f64,
    i: u64,
    cur: usize,
    phase: f64,
}

impl AleGenerator {
    /// The generator of `o`.
    pub fn new(o: AleGenOptions) -> AleGenerator {
        let mut rng = Rng::new(o.seed);
        let mut txs = Vec::new();
        let mut t = 0.6;
        for c in &o.calls {
            let mut tones = Vec::with_capacity(c.words.len() * 49);
            for &w in &c.words {
                let tw = word::word_tones(w);
                for &tone in &tw {
                    let mut b = word::tribit(tone);
                    for k in 0..3 {
                        if o.ber > 0.0 && rng.uniform() < o.ber {
                            b ^= 1 << k;
                        }
                    }
                    tones.push(word::tone(b));
                }
            }
            let dur = tones.len() as f64 * SYMBOL_S;
            txs.push(Tx { start: t, tones });
            t += dur + 1.2;
        }
        let total = t - 1.2;
        AleGenerator {
            txs,
            total,
            i: 0,
            cur: 0,
            phase: 0.0,
            o,
        }
    }

    /// The generator of the stack's settings: `gen_calls` and `gen_ber`.
    pub fn from_config(cfg: &GenConfig) -> AleGenerator {
        let s = &cfg.settings;
        AleGenerator::new(AleGenOptions {
            rate: cfg.rate_hz,
            ber: s.get(&super::GEN_BER),
            seed: cfg.seed,
            repeat: true,
            calls: if s.get(&super::GEN_CALLS) == super::GenCalls::Sounding {
                sounding("TSTA", false, 4)
            } else {
                test_calls()
            },
            tail_s: 1.0,
        })
    }

    /// The signal time of each transmission's start, seconds.
    pub fn starts(&self) -> Vec<f64> {
        self.txs.iter().map(|t| t.start).collect()
    }

    /// The length of one pass of the script (without the repeat pause), seconds.
    pub fn length_s(&self) -> f64 {
        self.total + self.o.tail_s
    }
}

impl Generator for AleGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The word SNR is stated in 3 kHz. A word carries 24 bits in 49 symbols.
        Some(
            SignalInfo::new(3000.0)
                .with_symbol_rate(125.0)
                .with_bit_rate(24.0 / (49.0 * SYMBOL_S)),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let rate = self.o.rate;
        let cycle = if self.o.repeat {
            self.total + 3.0
        } else {
            self.total + self.o.tail_s
        };
        for (k, o) in out.iter_mut().enumerate() {
            let mut t = self.i as f64 / rate;
            if t >= cycle {
                if !self.o.repeat {
                    return k;
                }
                self.i = 0;
                self.cur = 0;
                t = 0.0;
            }
            self.i += 1;
            while self.cur < self.txs.len() {
                let tx = &self.txs[self.cur];
                if t >= tx.start + tx.tones.len() as f64 * SYMBOL_S {
                    self.cur += 1;
                } else {
                    break;
                }
            }
            *o = match self.txs.get(self.cur) {
                Some(tx) => {
                    let dur = tx.tones.len() as f64 * SYMBOL_S;
                    tone_sample(&tx.tones, t - tx.start, dur, rate, 1.0, &mut self.phase)
                }
                None => Complex32::new(0.0, 0.0),
            };
        }
        out.len()
    }
}

/// The length of one 8-FSK symbol, s.
pub const SYMBOL_S: f64 = 0.008;
/// The length of one word on the air (49 symbols), s.
pub const WORD_S: f64 = 49.0 * SYMBOL_S;
/// The raised-cosine ramp at the start and the end of a transmission, s.
pub const RAMP_S: f64 = 0.002;
/// The lowest tone (tone 0), Hz; tone k is 250 Hz higher per step.
pub const BASE_HZ: f64 = 750.0;

/// The sample at `dt` seconds after the start of a transmission of `tones` that ends at `end` seconds, for samples at
/// `rate` and peak amplitude `amp`: the upper-sideband tone of the 8 ms symbol at `dt`, its phase run on from `phase`
/// (continuous over the whole transmission), with raised-cosine ramps of 2 ms after the start and before `end`. Zero, and
/// the phase kept, outside `0 <= dt < end`. The test signal and the transmitter both write through this function.
pub fn tone_sample(
    tones: &[u8],
    dt: f64,
    end: f64,
    rate: f64,
    amp: f64,
    phase: &mut f64,
) -> Complex32 {
    if !(dt >= 0.0 && dt < end) || tones.is_empty() {
        return Complex32::new(0.0, 0.0);
    }
    // A sample on a symbol boundary belongs to the symbol that starts there, whatever the rounding of the quotient.
    let tone = tones[((dt / SYMBOL_S + 1e-9) as usize).min(tones.len() - 1)];
    let f = BASE_HZ + 250.0 * f64::from(tone);
    *phase = (*phase + TAU * f / rate) % TAU;
    let rise = if dt < RAMP_S {
        0.5 - 0.5 * (PI * dt / RAMP_S).cos()
    } else {
        1.0
    };
    let fall = if end - dt < RAMP_S {
        0.5 - 0.5 * (PI * (end - dt) / RAMP_S).cos()
    } else {
        1.0
    };
    let a = amp * rise.min(fall);
    Complex32::new((a * phase.cos()) as f32, (a * phase.sin()) as f32)
}

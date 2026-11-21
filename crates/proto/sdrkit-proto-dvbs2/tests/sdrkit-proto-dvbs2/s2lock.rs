//! The lock of the DVB-S2 receiver over seeds: every constellation, normal and short frames, with and without pilots, both
//! spectrum senses, the three roll-offs, the low code rates of QPSK at an Es/N0 near 0 dB, a carrier far off the centre.
//! A seed sets the data, the noise, the carrier phase, a carrier offset and a symbol clock offset. A receiver that
//! locks returns every frame it must, bit for bit.
//!
//! Two ignored tests measure: `threshold` prints the frame error rate against Es/N0, `cost` the processor time of each
//! stage. Both take the carrier from `S2LOCK_CASE` (MODCOD, short, pilots, inverted, dB over the figure of the standard;
//! for example `18,0,1,0,1.5`).

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::{Complex32, Output, Staged, Value};
use sdrkit_graph::{Buffer, Data, Format, GraphBuilder, RunContext};
use sdrkit_proto_dvbs2::fec::FrameDecoder;
use sdrkit_proto_dvbs2::front::{Front, FrontConfig};
use sdrkit_proto_dvbs2::modcod::ModCod;
use sdrkit_proto_dvbs2::s2encode::{roll_off_field, S2Encoder, TsAdapter};
use sdrkit_proto_dvbs2::sync::{Standard, Sync};
use sdrkit_proto_dvbs2::transmit::TxShaper;
use sdrkit_proto_dvbs2::{attach, Dvbs2Parts};
use sdrkit_proto_ipcast::bbheader::{bb_descramble, BBHEADER_LEN};
use std::f64::consts::PI;
use std::time::Instant;

/// The input rate and the nominal symbol rate of the test signals.
const FS: f64 = 2e6;
const RS: f64 = 1e6;
/// The frames of a test signal, at least.
const FRAMES: usize = 8;
/// The frames at the end of a signal that the receiver need not return: it waits for the header after a frame.
const TAIL: usize = 2;
/// The symbols of a signal that the receiver graph may take before it returns its first frame: the survey (48 spectra,
/// 192 000 symbols) and the first search (65 000 symbols).
const LOCK_SYMBOLS: usize = 260_000;
/// The seeds of each case.
const SEEDS: u64 = 20;

/// One carrier.
#[derive(Clone, Copy, Debug)]
struct Case {
    id: u8,
    short: bool,
    pilots: bool,
    inverted: bool,
    roll_off: f64,
    /// Es/N0, dB over the figure of EN 302 307-1 table 13 for the MODCOD.
    margin_db: f64,
    /// The receiver has the nominal symbol rate set.
    rate_set: bool,
    /// The carrier is off the centre by up to this share of the symbol rate, each way.
    offset: f64,
}

impl Case {
    /// MODCOD `id` at roll-off 0.35, 3 dB over its figure.
    fn new(id: u8, short: bool, pilots: bool) -> Case {
        Case {
            id,
            short,
            pilots,
            inverted: false,
            roll_off: 0.35,
            margin_db: 3.0,
            rate_set: false,
            offset: 0.005,
        }
    }
    fn modcod(&self) -> ModCod {
        ModCod::new(self.id).unwrap()
    }
    fn esn0_db(&self) -> f64 {
        self.modcod().qef_esn0(false) + self.margin_db
    }
    fn name(&self) -> String {
        format!(
            "{}{}{}{}{}{} roll-off {} at {:.2} dB",
            self.modcod().name(),
            if self.short { " short" } else { " normal" },
            if self.pilots { " pilots" } else { "" },
            if self.inverted { " inverted" } else { "" },
            if self.rate_set { ", rate set" } else { "" },
            if self.offset > 0.005 { " off the centre" } else { "" },
            self.roll_off,
            self.esn0_db()
        )
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// A number in -1 to 1.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

/// `frames` PLFRAMEs of `c` with data from `rng`: their symbols and the BBFRAMEs they carry.
fn frames(c: &Case, rng: &mut Rng, frames: usize) -> (Vec<Complex32>, Vec<Vec<u8>>) {
    let mut enc = S2Encoder::new(c.modcod(), c.short, c.pilots, 0);
    let code = enc.code().expect("the frame size has a code");
    let dfl = code.kbch / 8 - BBHEADER_LEN;
    let mut adapter = TsAdapter::default();
    let mut sent = Vec::with_capacity(frames);
    let (mut all, mut syms) = (Vec::new(), Vec::new());
    for _ in 0..frames {
        let frame = enc.bbframe();
        adapter.fill(frame, dfl, None, roll_off_field(c.roll_off), &mut || {
            let mut p = [0u8; 188];
            for b in p.iter_mut() {
                *b = rng.next() as u8;
            }
            p[0] = 0x47;
            p
        });
        sent.push(frame.to_vec());
        assert!(enc.encode(&mut syms));
        all.extend_from_slice(&syms);
    }
    (all, sent)
}

/// The signal of `c` for `seed`, of `symbols` symbols at least, and the BBFRAMEs it carries. The carrier is off the
/// centre by up to the share of the symbol rate that `c` holds (0.5 % if not set) at a random phase, the symbol clock up
/// to 100 ppm off.
fn signal(c: &Case, seed: u64, symbols: usize) -> (Vec<Complex32>, Vec<Vec<u8>>) {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ (seed + 1).wrapping_mul(0x2545_f491_4f6c_dd1d));
    for _ in 0..8 {
        rng.next();
    }
    let rs = RS * (1.0 + 1e-4 * rng.unit());
    let mut shaper = TxShaper::new(FS, rs, c.roll_off).unwrap();
    let count = FRAMES.max(symbols.div_ceil(c.modcod().frame_symbols(c.short, c.pilots)));
    let (syms, sent) = frames(c, &mut rng, count);
    shaper.push(&syms);
    let mut x = vec![Complex32::default(); shaper.ready()];
    shaper.pop(&mut x);
    let offset = c.offset * rng.unit();
    let phase = PI * rng.unit();
    let w = 2.0 * PI * offset * RS / FS;
    for (k, z) in x.iter_mut().enumerate() {
        let (s, co) = (phase + w * k as f64).sin_cos();
        *z *= Complex32::new(co as f32, s as f32);
        if c.inverted {
            *z = Complex32::new(z.im, z.re);
        }
    }
    let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
    let level = NoiseLevel::EsN0Db {
        esn0_db: c.esn0_db(),
        symbol_rate_hz: rs,
    };
    Awgn::complex(level, p, FS, seed ^ 0x5bd1_e995).unwrap().process(&mut x);
    (x, sent)
}

/// What a receiver returned of a signal.
#[derive(Clone, Debug, Default)]
struct Got {
    /// The frames sent.
    sent: usize,
    /// The indices of the frames returned, and the count of those returned that were not sent.
    idx: Vec<usize>,
    wrong: usize,
    /// The spectrum sense reported.
    inverted: Option<bool>,
    /// Seconds in the front end, the sync stage and the frame decoder while frames were followed, and the symbols
    /// that each of them worked on in that time.
    cost: [f64; 3],
    symbols: [usize; 3],
    /// The symbols of the signal that the receiver had taken when it returned its first frame.
    first_after: Option<usize>,
}

impl Got {
    fn count(&mut self, sent: &[Vec<u8>], frame: &[u8]) {
        self.first_after.get_or_insert(self.symbols[0]);
        match sent.iter().position(|s| s[..] == *frame) {
            Some(i) => self.idx.push(i),
            None => self.wrong += 1,
        }
    }
    /// The frames returned of those from frame `from` on that the receiver must return, and their count.
    fn returned(&self, from: usize) -> (usize, usize) {
        let end = self.sent - TAIL;
        (self.idx.iter().filter(|&&i| i >= from && i < end).count(), end.saturating_sub(from))
    }
    /// From its first frame on the receiver returned every frame, and no frame that was not sent.
    fn locked(&self) -> bool {
        self.idx.first().is_some_and(|&first| {
            let (good, due) = self.returned(first);
            self.wrong == 0 && good == due
        })
    }
}

/// The signal of `c` for `seed`, of `symbols` symbols, through the receiver graph of [`attach`]: the BBFRAMEs of its FEC
/// stage.
fn through_graph(c: &Case, seed: u64, symbols: usize) -> Got {
    let (x, sent) = signal(c, seed, symbols);
    let mut g = attach(GraphBuilder::new(), Dvbs2Parts { ts: true, field: false })
        .set("s2_front.symbol_rate", Value::Hz(if c.rate_set { RS } else { 0.0 }))
        .clock(0.25)
        .build()
        .expect("the graph builds");
    g.set_context(RunContext::default());
    g.configure_input(Format::iq(FS, 0.0, 0.0)).expect("the rate");
    let mut tap = g.tap("s2_fec.out", 1 << 16).expect("the tapped port");
    let mut buf = Buffer::new(&sdrkit_graph::Kind::Bytes);
    let mut got = Got {
        sent: sent.len(),
        ..Got::default()
    };
    let mut take = |tap: &mut sdrkit_graph::TapReader, got: &mut Got| {
        while tap.pop(&mut buf) {
            if let Data::Bytes(v) = &buf.data {
                for p in v {
                    got.count(&sent, p.bytes());
                }
            }
        }
    };
    let mut out = Output::default();
    for b in x.chunks(16384) {
        g.process(b, &mut out);
        out.clear();
        got.symbols[0] += (b.len() as f64 * RS / FS) as usize;
        take(&mut tap, &mut got);
    }
    g.drain(&mut out);
    g.finish(&mut out);
    take(&mut tap, &mut got);
    got
}

/// The signal of `c` for `seed`, of `symbols` symbols, through the front end, the sync stage and the frame decoder
/// called one after the other, with the time each takes while frames are followed.
fn direct(c: &Case, seed: u64, symbols: usize) -> Got {
    let (x, sent) = signal(c, seed, symbols);
    direct_on(&x, &sent)
}

/// The samples `x`, which carry the BBFRAMEs `sent`, through the three stages as [`direct`] does.
fn direct_on(x: &[Complex32], sent: &[Vec<u8>]) -> Got {
    let mut front = Front::new(FS, FrontConfig::default()).unwrap();
    let mut sync = Sync::new(Standard::S2, 0);
    let mut dec = FrameDecoder::default();
    let mut got = Got {
        sent: sent.len(),
        ..Got::default()
    };
    let (mut syms, mut bytes) = (Vec::new(), Vec::new());
    for b in x.chunks(16384) {
        let followed = sync.locked();
        front.hold_timing(followed);
        syms.clear();
        let t0 = Instant::now();
        front.process(b, &mut syms);
        if std::mem::replace(&mut front.restarted, false) {
            sync.restart();
        }
        let t1 = Instant::now();
        sync.push(&syms);
        if followed {
            got.cost[0] += (t1 - t0).as_secs_f64();
            got.cost[1] += t1.elapsed().as_secs_f64();
            got.symbols[0] += syms.len();
            got.symbols[1] += syms.len();
        }
        for f in sync.frames_out.drain(..) {
            let m = ModCod::new(f.modcod).unwrap();
            let noise_var = 10f32.powf(-(f.esn0_db.clamp(-10.0, 40.0) as f32) / 10.0);
            let t2 = Instant::now();
            if dec.decode(&m, f.short, &f.syms, noise_var, &mut bytes).is_some_and(|r| r.bch_ok) {
                bb_descramble(&mut bytes);
                got.count(sent, &bytes);
            }
            got.cost[2] += t2.elapsed().as_secs_f64();
            got.symbols[2] += m.frame_symbols(f.short, f.pilots);
        }
    }
    got.inverted = sync.report.locked.then_some(sync.report.inverted);
    got
}

/// The receiver graph locks on every case with every seed and returns its first frame within [`LOCK_SYMBOLS`]. Prints
/// the latest first frame over the seeds and the most symbols of the signal that the graph took before it returned a
/// frame.
fn all_lock(cases: &[Case]) {
    let mut failed = Vec::new();
    for c in cases {
        let mut locked = 0;
        let (mut latest, mut after) = (0, 0);
        for seed in 0..SEEDS {
            let g = through_graph(c, seed, 400_000);
            let first = g.idx.first().copied();
            if g.locked() && g.first_after.is_some_and(|n| n <= LOCK_SYMBOLS) {
                locked += 1;
                latest = latest.max(first.unwrap_or(0));
                after = after.max(g.first_after.unwrap_or(0));
            } else {
                eprintln!("{} seed {seed}: frames {:?} of {}, {} not sent", c.name(), g.idx, g.sent, g.wrong);
            }
        }
        eprintln!(
            "{}: locked on {locked} of {SEEDS} seeds, from frame {latest}, first frame out after {after} symbols",
            c.name()
        );
        if locked != SEEDS {
            failed.push(format!("{}: {locked} of {SEEDS}", c.name()));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

fn inverted(c: Case) -> Case {
    Case { inverted: true, ..c }
}

/// `c` at 20 dB over the figure of its MODCOD: the survey then measures the roll-off of the carrier.
fn strong(c: Case) -> Case {
    Case { margin_db: 20.0, ..c }
}

/// `c` at an Es/N0 of `esn0_db`.
fn at_db(c: Case, esn0_db: f64) -> Case {
    Case { margin_db: esn0_db - c.modcod().qef_esn0(false), ..c }
}

/// `c` with the carrier up to 20 % of the symbol rate off the centre: as far as the input of the tests has room.
fn off_centre(c: Case) -> Case {
    Case { offset: 0.2, ..c }
}

/// Noise without a carrier, as long as the test signals.
fn noise_alone(seed: u64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); 800_000];
    let level = NoiseLevel::EsN0Db {
        esn0_db: 0.0,
        symbol_rate_hz: RS,
    };
    Awgn::complex(level, 1.0, FS, seed).unwrap().process(&mut x);
    x
}

#[test]
fn qpsk_locks_on_every_seed() {
    all_lock(&[
        Case::new(7, false, false),
        Case::new(4, false, true),
        Case::new(10, true, false),
    ]);
}

/// The plateau of these carriers is 3 dB over the noise floor or less, and no single header shows in the header metric.
/// QPSK 1/2 is 0.7 dB over its figure.
#[test]
fn the_low_rates_of_qpsk_lock_on_every_seed() {
    all_lock(&[
        at_db(Case::new(1, false, false), 0.0),
        at_db(Case::new(1, false, true), 0.0),
        at_db(Case::new(1, true, false), 0.0),
        inverted(at_db(Case::new(1, false, false), 0.0)),
        at_db(Case::new(2, false, false), 0.0),
        at_db(Case::new(3, false, false), 0.7),
        at_db(Case::new(4, false, false), 1.7),
        Case { roll_off: 0.20, ..at_db(Case::new(4, true, true), 1.95) },
    ]);
}

/// The survey places a carrier that is far off the centre, also one near the noise floor, and the spectrum sense is
/// still told.
#[test]
fn a_carrier_far_off_the_centre_locks_on_every_seed() {
    all_lock(&[
        off_centre(at_db(Case::new(1, false, false), 0.0)),
        off_centre(inverted(at_db(Case::new(4, false, false), 1.7))),
        off_centre(Case::new(13, false, false)),
        off_centre(Case::new(18, false, true)),
    ]);
}

/// The input filter of a radio leaves a step in the noise floor: here the noise of the inner 70 % of the band is 3 dB
/// over that of the edges. The survey takes the carrier, which stands 5 dB over that hump and is off the centre, and
/// the receiver locks.
#[test]
fn a_carrier_on_a_step_in_the_noise_floor_locks() {
    // A low-pass filter to 0.35 of the sample rate.
    let taps: Vec<f32> = (0..127)
        .map(|k| {
            let t = (k as f64 - 63.0) * 0.7 * PI;
            let sinc = if t == 0.0 { 1.0 } else { t.sin() / t };
            (0.7 * sinc * (0.5 - 0.5 * (2.0 * PI * (k as f64 + 0.5) / 127.0).cos())) as f32
        })
        .collect();
    let c = off_centre(at_db(Case::new(4, false, false), 40.0));
    let level = NoiseLevel::EsN0Db {
        esn0_db: 8.0,
        symbol_rate_hz: RS,
    };
    for seed in 0..6 {
        let (mut x, sent) = signal(&c, seed, 400_000);
        let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
        let mut inner = vec![Complex32::default(); x.len() + taps.len()];
        Awgn::complex(level, p, FS, seed + 100).unwrap().process(&mut inner);
        Awgn::complex(level, p, FS, seed + 200).unwrap().process(&mut x);
        for (z, w) in x.iter_mut().zip(inner.windows(taps.len())) {
            *z += w.iter().zip(&taps).map(|(v, t)| *v * *t).sum::<Complex32>();
        }
        let g = direct_on(&x, &sent);
        assert!(g.locked(), "seed {seed}: frames {:?} of {}", g.idx, g.sent);
    }
}

/// Noise alone has no hump with a cyclic line, so the survey takes no carrier and no frame comes.
#[test]
fn noise_alone_gives_no_carrier() {
    for seed in 0..4 {
        let x = noise_alone(seed);
        let mut front = Front::new(FS, FrontConfig::default()).unwrap();
        let mut sync = Sync::new(Standard::S2, 0);
        let mut syms = Vec::new();
        for b in x.chunks(16384) {
            syms.clear();
            front.process(b, &mut syms);
            sync.push(&syms);
        }
        assert!(front.acq.is_none(), "seed {seed}: {:?}", front.acq);
        assert!(front.carriers.is_empty(), "seed {seed}: {:?}", front.carriers);
        assert!(sync.frames_out.is_empty(), "seed {seed}");
    }
}

/// Symbols of noise alone: the search over several frames finds no header in them.
#[test]
fn noise_symbols_give_no_frames() {
    for seed in 0..4 {
        let mut x = vec![Complex32::default(); 400_000];
        let level = NoiseLevel::EsN0Db {
            esn0_db: 0.0,
            symbol_rate_hz: RS,
        };
        Awgn::complex(level, 1.0, RS, seed).unwrap().process(&mut x);
        let mut sync = Sync::new(Standard::S2, 0);
        for b in x.chunks(8192) {
            sync.push(b);
        }
        assert!(!sync.locked(), "seed {seed}");
        assert!(sync.frames_out.is_empty(), "seed {seed}");
        assert!(sync.report.failed_hunts >= 2, "seed {seed}");
    }
}

/// A carrier that ends: the frame follower takes at most two more frames of the same kind, then searches again, and
/// finds nothing in the noise.
#[test]
fn the_end_of_a_carrier_ends_the_frames() {
    for c in [Case::new(7, false, false), at_db(Case::new(1, false, false), 0.0), Case::new(25, true, false)] {
        for seed in 0..4u64 {
            let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ (seed + 1).wrapping_mul(0x2545_f491_4f6c_dd1d));
            let len = c.modcod().frame_symbols(c.short, c.pilots);
            let (mut x, sent) = frames(&c, &mut rng, 200_000usize.div_ceil(len));
            x.resize(x.len() + 12 * len + 400_000, Complex32::default());
            let level = NoiseLevel::EsN0Db {
                esn0_db: c.esn0_db(),
                symbol_rate_hz: RS,
            };
            Awgn::complex(level, 1.0, RS, seed).unwrap().process(&mut x);
            let mut sync = Sync::new(Standard::S2, 0);
            let mut got = Vec::new();
            for b in x.chunks(8192) {
                sync.push(b);
                got.extend(sync.frames_out.drain(..).map(|f| (f.modcod, f.short, f.pilots)));
            }
            let name = c.name();
            assert!(got.len() >= sent.len() - 1 && got.len() <= sent.len() + 2, "{name} seed {seed}: {} frames", got.len());
            assert!(got.iter().all(|&k| k == (c.id, c.short, c.pilots)), "{name} seed {seed}: {got:?}");
            assert!(!sync.locked() && !sync.report.vcm, "{name} seed {seed}");
        }
    }
}

#[test]
fn psk8_locks_on_every_seed() {
    all_lock(&[
        Case::new(13, false, false),
        Case::new(15, false, true),
        Case::new(12, true, false),
    ]);
}

#[test]
fn apsk16_locks_on_every_seed() {
    all_lock(&[
        Case::new(18, false, false),
        Case::new(23, false, false),
        Case::new(19, false, true),
        Case::new(21, true, false),
        Case::new(20, true, true),
        strong(Case::new(18, false, false)),
    ]);
}

#[test]
fn apsk32_locks_on_every_seed() {
    all_lock(&[
        Case::new(24, false, false),
        Case::new(28, false, false),
        Case::new(26, false, true),
        Case::new(25, true, false),
        Case::new(27, true, true),
        strong(Case::new(28, false, false)),
    ]);
}

#[test]
fn an_inverted_spectrum_locks_on_every_seed() {
    all_lock(&[
        inverted(Case::new(6, false, false)),
        inverted(Case::new(13, false, true)),
        inverted(Case::new(19, true, false)),
        inverted(Case::new(25, false, true)),
        inverted(Case::new(24, false, false)),
    ]);
}

#[test]
fn every_roll_off_locks_on_every_seed() {
    let cases = [
        Case { roll_off: 0.25, ..Case::new(13, false, false) },
        Case { roll_off: 0.20, ..Case::new(18, false, false) },
        Case { roll_off: 0.20, ..Case::new(6, true, true) },
        Case { roll_off: 0.25, ..Case::new(26, true, false) },
    ];
    all_lock(&cases);
    all_lock(&cases.map(strong));
}

/// With the symbol rate set the clock loop searches wide, also at the measured roll-off of a strong carrier.
#[test]
fn a_set_symbol_rate_locks_on_every_seed() {
    let cases = [
        Case::new(7, false, false),
        Case::new(13, true, true),
        Case { roll_off: 0.20, ..Case::new(18, false, false) },
        Case { roll_off: 0.25, ..Case::new(26, true, false) },
    ]
    .map(|c| Case { rate_set: true, ..c });
    all_lock(&cases);
    all_lock(&cases.map(strong));
}

#[test]
fn the_spectrum_sense_is_reported() {
    for c in [Case::new(6, true, false), Case::new(21, true, true)] {
        for c in [c, inverted(c)] {
            let g = direct(&c, 3, 200_000);
            assert_eq!(g.inverted, Some(c.inverted), "{}", c.name());
            assert!(g.locked(), "{}: {:?}", c.name(), g.idx);
        }
    }
}

/// The case of `S2LOCK_CASE`.
fn env_case() -> Case {
    let v: Vec<f64> = std::env::var("S2LOCK_CASE")
        .expect("S2LOCK_CASE is set")
        .split(',')
        .map(|x| x.parse().expect("a number"))
        .collect();
    Case {
        inverted: v[3] != 0.0,
        margin_db: v[4],
        ..Case::new(v[0] as u8, v[1] != 0.0, v[2] != 0.0)
    }
}

/// Prints the frame error rate of the case of the environment from its Es/N0 down in steps of 0.25 dB until it passes
/// 1 in 10 or the Es/N0 is 1 dB under the figure: over the seeds, the frames after the first 250 000 symbols of 800 000.
/// With `S2LOCK_GRAPH` set the signal goes through the receiver graph, which also tries DVB-S and dithers the symbol
/// rate after failed searches; else through the three stages called one after the other.
#[test]
#[ignore = "a measurement"]
fn threshold() {
    let c0 = env_case();
    let graph = std::env::var("S2LOCK_GRAPH").is_ok();
    let mut margin = c0.margin_db;
    loop {
        let c = Case { margin_db: margin, ..c0 };
        let runs: Vec<Got> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..SEEDS)
                .map(|seed| {
                    s.spawn(move || {
                        if graph {
                            through_graph(&c, seed, 800_000)
                        } else {
                            direct(&c, seed, 800_000)
                        }
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let from = 250_000usize.div_ceil(c.modcod().frame_symbols(c.short, c.pilots));
        let (mut due, mut good, mut wrong) = (0, 0, 0);
        for g in &runs {
            let r = g.returned(from);
            good += r.0;
            due += r.1;
            wrong += g.wrong;
        }
        let fer = 1.0 - good as f64 / due as f64;
        eprintln!("FER {} ({margin:+.2}): {fer:.4} ({good} of {due}, {wrong} not sent)", c.name());
        if fer > 0.1 || margin < -1.0 {
            break;
        }
        margin -= 0.25;
    }
}

/// Prints the processor time of the stages on the case of the environment while frames are followed, per symbol: the
/// least of 5 runs of 1 600 000 symbols.
#[test]
#[ignore = "a measurement"]
fn cost() {
    let c = env_case();
    let mut best = [f64::MAX; 3];
    for _ in 0..5 {
        let g = direct(&c, 1, 1_600_000);
        for ((b, t), n) in best.iter_mut().zip(g.cost).zip(g.symbols) {
            *b = b.min(t / n.max(1) as f64 * 1e9);
        }
    }
    eprintln!(
        "COST {}: front {:.1} ns, sync {:.1} ns, fec {:.1} ns per symbol",
        c.name(),
        best[0],
        best[1],
        best[2]
    );
}

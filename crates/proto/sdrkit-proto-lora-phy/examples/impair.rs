//! Measures the frame error rate of the receive chain on signals with the faults of real transmitters and radios:
//! `cargo run --profile shipped -p sdrkit-proto-lora-phy --example impair -- [--frames 200] [--only text] [--lens 20,60,200]
//! [--sf 11 --bw 250000 --cr 5 --preamble 16 --sync 43] [--rate 2000000] [--rf 919875000] [--workers 8]`.
//!
//! Each line is one point and one payload length: the judged frames, how many were right, failed the CRC or were not
//! found, the same by place in a group of frames, the frames that a search repaired, and for the frames that failed the
//! CRC the mean number of wrong symbols, the share of those off by one bin and the mean place of the first wrong symbol
//! (0 at the start, 1 at the end).

#[path = "../tests/sdrkit_proto_lora_phy/impair.rs"]
mod impair;

use impair::{Radio, Tx};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{coding, ChannelRx, DetectorConfig, LoraParams};

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// One clean frame.
    Clean,
    /// A crystal error, ppm: carrier and clock together.
    Crystal(f64),
    /// A clock error alone, ppm.
    Clock(f64),
    /// A carrier drift, Hz/s.
    Drift(f64),
    /// The frame this many dB above full scale of a 12-bit converter.
    Clip(f64),
    /// A gain step of this many dB at a random time in the frame.
    Step(f64),
    /// A second frame at this level (dB) that starts inside the first. The stronger frame is judged.
    Overlap(f64),
    /// Noise at this SNR in the LoRa bandwidth, dB.
    Snr(f64),
    /// This many frames with this gap (ms) between them, each with its own crystal error within 30 ppm and level within 3 dB.
    Group(usize, f64),
    /// A frame 40 dB above full scale, then after this gap (ms) a frame at the usual level, which is judged.
    AfterLoud(f64),
    /// A DC offset of this share of the frame amplitude.
    Dc(f64),
    /// IQ imbalance of 1 dB and this many degrees.
    Iq(f64),
    /// A crystal error within 30 ppm, a carrier drift within 100 Hz/s and a frame 6 dB above full scale.
    All,
}

struct Group {
    txs: Vec<Tx>,
    radio: Radio,
    judged: Vec<usize>,
    seconds: f64,
}

struct Setup {
    p: LoraParams,
    rate: f64,
    rf: f64,
}

fn payload(len: usize, rng: &mut Rng) -> Vec<u8> {
    let mut v = vec![0u8; len];
    rng.fill_bytes(&mut v);
    v
}

/// A frame with a random payload, start phase and carrier within one bin, at a start that is on no sample grid.
fn tx(s: &Setup, len: usize, start_s: f64, rng: &mut Rng) -> Tx {
    let mut t = Tx::new(&payload(len, rng), start_s);
    t.phase = rng.range(0.0, std::f64::consts::TAU);
    t.carrier_hz = rng.range(-1.0, 1.0) * s.p.bw / s.p.n() as f64;
    t
}

fn build(kind: Kind, s: &Setup, len: usize, rng: &mut Rng) -> Group {
    let sym = s.p.symbol_s();
    let lead = sym * (2.0 + rng.uniform());
    let mut radio = Radio::new(s.rate);
    radio.rf_hz = s.rf;
    radio.noise_seed = rng.next_u64();
    let mut txs = vec![tx(s, len, lead, rng)];
    let mut judged = vec![0];
    let dur = txs[0].duration_s(&s.p);
    match kind {
        Kind::Clean => {}
        Kind::Crystal(ppm) => txs[0].crystal_ppm = ppm,
        Kind::Clock(ppm) => txs[0].clock_ppm = ppm,
        Kind::Drift(hz_s) => txs[0].drift_hz_s = hz_s,
        Kind::Clip(db) => {
            radio.amp = 10f32.powf(db as f32 / 20.0);
            radio.adc_bits = Some(12);
        }
        Kind::Step(db) => radio.level_step = Some((lead + dur * rng.range(0.1, 0.9), db)),
        Kind::Overlap(db) => {
            let mut b = tx(s, len, lead + dur * rng.range(0.1, 0.9), rng);
            b.level_db = db;
            b.crystal_ppm = rng.range(-30.0, 30.0);
            txs[0].crystal_ppm = rng.range(-30.0, 30.0);
            txs.push(b);
            if db > 0.0 {
                judged = vec![1];
            }
        }
        Kind::Snr(db) => radio.snr_db = Some(db),
        Kind::Group(count, gap_ms) => {
            txs[0].crystal_ppm = rng.range(-30.0, 30.0);
            txs[0].level_db = rng.range(-3.0, 0.0);
            for k in 1..count {
                let end = txs[k - 1].start_s + txs[k - 1].duration_s(&s.p);
                let mut b = tx(s, len, end + gap_ms * 1e-3, rng);
                b.crystal_ppm = rng.range(-30.0, 30.0);
                b.level_db = rng.range(-3.0, 0.0);
                txs.push(b);
                judged.push(k);
            }
        }
        Kind::AfterLoud(gap_ms) => {
            radio.amp = 0.05;
            radio.adc_bits = Some(12);
            txs[0].level_db = 66.0;
            txs[0].crystal_ppm = rng.range(-30.0, 30.0);
            let mut b = tx(s, len, lead + dur + gap_ms * 1e-3, rng);
            b.crystal_ppm = rng.range(-30.0, 30.0);
            txs.push(b);
            judged = vec![1];
        }
        Kind::Dc(share) => {
            radio.dc = Complex32::new(radio.amp * share as f32, -radio.amp * share as f32 * 0.5)
        }
        Kind::Iq(deg) => radio.iq = Some((1.0, deg)),
        Kind::All => {
            txs[0].crystal_ppm = rng.range(-30.0, 30.0);
            txs[0].drift_hz_s = rng.range(-100.0, 100.0);
            radio.amp = 2.0;
            radio.adc_bits = Some(12);
        }
    }
    let end = txs
        .iter()
        .map(|t| t.start_s + t.duration_s(&s.p))
        .fold(0.0, f64::max);
    Group {
        txs,
        radio,
        judged,
        seconds: end + 4.0 * sym,
    }
}

#[derive(Clone, Default)]
struct Stats {
    /// By place in the group: right, CRC failed, not found.
    place: [[u32; 3]; 3],
    repaired: u32,
    wrong_syms: u64,
    off_by_one: u64,
    first_wrong: f64,
    crc_frames: u32,
    extra: u32,
}

impl Stats {
    fn add(&mut self, o: &Stats) {
        for (a, b) in self.place.iter_mut().zip(&o.place) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
        self.repaired += o.repaired;
        self.wrong_syms += o.wrong_syms;
        self.off_by_one += o.off_by_one;
        self.first_wrong += o.first_wrong;
        self.crc_frames += o.crc_frames;
        self.extra += o.extra;
    }
}

fn detector(p: &LoraParams) -> DetectorConfig {
    let mut d = DetectorConfig::new(p.sf, p.bw, p.sync);
    d.preamble = p.preamble;
    d.ldro = p.ldro;
    d
}

/// Runs the groups `first, first + step, ..` below `count` of one point through one receiver, as one stream.
fn worker(
    kind: Kind,
    s: &Setup,
    len: usize,
    seed: u64,
    first: usize,
    step: usize,
    count: usize,
) -> Stats {
    let mut st = Stats::default();
    let mut rx = ChannelRx::new(s.rate, 0.0, s.p.bw, vec![detector(&s.p)]).expect("a receiver");
    let mut base = 0u64;
    let mut out = Vec::new();
    let n = s.p.n() as i64;
    let sym_samples = s.p.symbol_s() * s.rate;
    let mut g = first;
    while g < count {
        let mut rng = Rng::stream(seed, g as u64);
        let grp = build(kind, s, len, &mut rng);
        let x = grp.radio.render(&s.p, &grp.txs, grp.seconds);
        out.clear();
        for c in x.chunks(65536) {
            rx.process(c, &mut out);
        }
        let mut used = vec![false; out.len()];
        for (place, &j) in grp.judged.iter().enumerate() {
            let t = &grp.txs[j];
            let at = base as f64 + t.start_s * s.rate;
            let hit = out
                .iter()
                .position(|(f, _)| (f.start as f64 - at).abs() < 3.0 * sym_samples);
            let slot = &mut st.place[place.min(2)];
            match hit {
                None => slot[2] += 1,
                Some(i) => {
                    used[i] = true;
                    let (f, syms) = &out[i];
                    if f.crc_ok && f.payload == t.payload {
                        slot[0] += 1;
                        if f.list_fixed || f.jumps > 0 {
                            st.repaired += 1;
                        }
                    } else {
                        slot[1] += 1;
                        let want = coding::encode(&t.payload, &s.p).unwrap_or_default();
                        let mut first = None;
                        for (k, (w, got)) in want.iter().zip(&syms.symbols).enumerate() {
                            let d = (got.bin() as i64 - *w as i64).rem_euclid(n);
                            let d = d.min(n - d);
                            // The header symbols carry two bits less: a neighbour bin is no error there.
                            if d == 0 || (k < 8 && d <= 1) {
                                continue;
                            }
                            st.wrong_syms += 1;
                            st.off_by_one += (d == 1) as u64;
                            first.get_or_insert(k);
                        }
                        st.crc_frames += 1;
                        st.first_wrong +=
                            first.map_or(1.0, |k| k as f64 / want.len().max(1) as f64);
                    }
                }
            }
        }
        // Frames that match no transmitted frame.
        for (i, (f, _)) in out.iter().enumerate() {
            let known = grp.txs.iter().any(|t| {
                (f.start as f64 - base as f64 - t.start_s * s.rate).abs() < 3.0 * sym_samples
            });
            if !used[i] && !known {
                st.extra += 1;
            }
        }
        base += x.len() as u64;
        g += step;
    }
    st
}

fn arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    let a: Vec<String> = std::env::args().collect();
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let frames: usize = arg("--frames", 200);
    let only: String = arg("--only", String::new());
    let lens: String = arg("--lens", "20,60,200".to_string());
    let workers: usize = arg("--workers", 8);
    let mut p = LoraParams::new(
        arg("--sf", 11),
        arg("--bw", 250e3),
        arg("--cr", 5),
        arg("--preamble", 16),
        arg("--sync", 0x2b),
    );
    p.crc = true;
    let s = Setup {
        p,
        rate: arg("--rate", 2e6),
        rf: arg("--rf", 919.875e6),
    };
    let mut points: Vec<(String, Kind)> = vec![("clean".into(), Kind::Clean)];
    for k in -6..=6 {
        let ppm = 5.0 * k as f64;
        points.push((format!("crystal {ppm:+} ppm"), Kind::Crystal(ppm)));
    }
    for ppm in [-30.0, 30.0] {
        points.push((format!("clock {ppm:+} ppm"), Kind::Clock(ppm)));
    }
    for d in [-200.0, -100.0, -50.0, 50.0, 100.0, 200.0] {
        points.push((format!("drift {d:+} Hz/s"), Kind::Drift(d)));
    }
    for db in [-1.0, 0.0, 3.0, 6.0, 12.0] {
        points.push((format!("clip {db:+} dBFS"), Kind::Clip(db)));
    }
    for db in [-12.0, -6.0, 6.0, 12.0] {
        points.push((format!("step {db:+} dB"), Kind::Step(db)));
    }
    for db in [-6.0, -10.0, -20.0, 6.0] {
        points.push((format!("overlap {db:+} dB"), Kind::Overlap(db)));
    }
    for db in [-5.0, -10.0, -15.0, -17.0, -18.0, -19.0, -20.0] {
        points.push((format!("snr {db:+} dB"), Kind::Snr(db)));
    }
    for count in [2usize, 3] {
        for gap in [0.0, 5.0, 20.0, 50.0, 100.0, 200.0] {
            points.push((
                format!("group of {count} gap {gap} ms"),
                Kind::Group(count, gap),
            ));
        }
    }
    for gap in [5.0, 50.0, 200.0] {
        points.push((format!("after loud {gap} ms"), Kind::AfterLoud(gap)));
    }
    points.push(("dc 0.3".into(), Kind::Dc(0.3)));
    points.push(("iq 1 dB 5 deg".into(), Kind::Iq(5.0)));
    points.push(("all".into(), Kind::All));

    println!(
        "SF{} {} Hz CR 4/{} preamble {} at {} S/s, {} judged frames per point",
        s.p.sf, s.p.bw, s.p.cr, s.p.preamble, s.rate, frames
    );
    println!("point | bytes | judged | right | crc | lost | bad by place | repaired | extra | wrong syms | off by one | first wrong");
    for (pi, (name, kind)) in points.iter().enumerate() {
        if !only.is_empty() && !name.contains(only.as_str()) {
            continue;
        }
        for len in lens.split(',').filter_map(|l| l.parse::<usize>().ok()) {
            let per_group = match kind {
                Kind::Group(c, _) => *c,
                _ => 1,
            };
            let groups = frames.div_ceil(per_group);
            let seed = 1000 * pi as u64 + len as u64;
            let mut total = Stats::default();
            std::thread::scope(|sc| {
                let hs: Vec<_> = (0..workers)
                    .map(|w| {
                        let s = &s;
                        sc.spawn(move || worker(*kind, s, len, seed, w, workers, groups))
                    })
                    .collect();
                for h in hs {
                    total.add(&h.join().expect("a worker"));
                }
            });
            let sum = |c: usize| total.place.iter().map(|p| p[c]).sum::<u32>();
            let judged = sum(0) + sum(1) + sum(2);
            let by_place: Vec<String> = total
                .place
                .iter()
                .filter(|p| p.iter().sum::<u32>() > 0)
                .map(|p| format!("{}/{}", p[1] + p[2], p.iter().sum::<u32>()))
                .collect();
            let cf = total.crc_frames.max(1) as f64;
            println!(
                "{name} | {len} | {judged} | {} | {} | {} | {} | {} | {} | {:.1} | {:.2} | {:.2}",
                sum(0),
                sum(1),
                sum(2),
                by_place.join(" "),
                total.repaired,
                total.extra,
                total.wrong_syms as f64 / cf,
                total.off_by_one as f64 / total.wrong_syms.max(1) as f64,
                total.first_wrong / cf,
            );
        }
    }
}

//! Plays a recording of 16-bit sample pairs through the DVB-T receiver, faster than real time, and prints the log, the
//! status of each second, the result fields at the end, the decoding time and a digest of the transport packets (all of
//! them, and those of the time spans given as `span=<from>:<to>` in seconds).
//!
//! `cargo run --release -p sdrkit-stack-dvbt --example recorded -- <file.sigmf-data> [rate Hz] [shift Hz] [key=value ...]`
//!
//! With `bench [seconds]` in place of a file it runs the benchmarks of the crate and prints the share of one core of each.

use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, SettingSet, StackConfig, Value, Values};
use sdrkit_graph::Timed;
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_stack_dvbt::settings::DvbtSetting;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: recorded <file.sigmf-data> [rate Hz] [shift Hz] [key=choice index ...]");
        return;
    };
    let nums: Vec<f64> = args[1..].iter().filter_map(|a| a.parse().ok()).collect();
    if path == "bench" {
        for b in sdrkit_stack_dvbt::benchmarks() {
            let r = (b.run)(nums.first().copied().unwrap_or(3.0));
            let status = if r.cpu_fraction_of_realtime > 1.0 {
                "MORE THAN ONE CORE"
            } else {
                "ok"
            };
            println!(
                "{:42} {:6.3} of one core  {status}",
                b.name, r.cpu_fraction_of_realtime
            );
        }
        return;
    }
    let rate = nums.first().copied().unwrap_or(9_142_856.0);
    let shift = nums.get(1).copied().unwrap_or(0.0);
    let raw = std::fs::read(path).expect("the recording reads");
    let step = 2.0 * std::f64::consts::PI * shift / rate;
    let x: Vec<Complex32> = raw
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .map(|(n, c)| {
            let i = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0;
            let q = i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0;
            let a = step * n as f64;
            Complex32::new(i, q) * Complex32::new(a.cos() as f32, a.sin() as f32)
        })
        .collect();
    let schema = DvbtSetting::schema();
    let mut v = Values::defaults(&schema);
    for a in &args[1..] {
        if let Some((k, val)) = a.split_once('=') {
            if k == "span" {
                continue;
            }
            let id = schema.id(k).expect("a setting key");
            let val = match val {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                n => Value::Choice(n.parse().expect("a choice index")),
            };
            v.set(id, &val).expect("a value of the setting");
        }
    }
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&StackConfig::new(v)).expect("builds");
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .expect("rate");
    let spans: Vec<(f64, f64)> = args[1..]
        .iter()
        .filter_map(|a| a.strip_prefix("span="))
        .filter_map(|a| a.split_once(':'))
        .map(|(a, b)| (a.parse().expect("seconds"), b.parse().expect("seconds")))
        .collect();
    let mut tap = rx.graph_mut().tap("fec.out", 1 << 14).expect("the port");
    let mut buf = sdrkit_graph::Buffer::default();
    let mut all = Digest::new();
    let mut inside = Digest::new();
    let mut failed_at = Vec::new();
    let mut out = Output::default();
    let started = std::time::Instant::now();
    let mut seen: Vec<String> = Vec::new();
    let per_s = rate as usize;
    let mut fed = 0usize;
    let mut next = per_s;
    for c in x.chunks(16_384) {
        rx.feed(c, &mut out);
        out.clear();
        fed += c.len();
        drain(&mut tap, &mut buf, &spans, &mut all, &mut inside);
        let r = Owner::results(&rx);
        let lines: Vec<String> = r.log_lines().iter().cloned().collect();
        if lines != seen {
            // The log is grouped by component: a line is new when the lines before held fewer copies of it.
            for (i, l) in lines.iter().enumerate() {
                let now = lines[..=i].iter().filter(|x| *x == l).count();
                let before = seen.iter().filter(|x| *x == l).count();
                if now > before {
                    println!("{:7.3} s log: {l}", fed as f64 / rate);
                }
            }
            seen = lines;
        }
        if fed >= next {
            next += per_s;
            failed_at.push(field(&rx, "rs_failed"));
            println!("{:7.3} s {}", fed as f64 / rate, line(&rx));
        }
    }
    rx.finish(&mut out);
    drain(&mut tap, &mut buf, &spans, &mut all, &mut inside);
    let took = started.elapsed().as_secs_f64();
    println!(
        "decoding: {took:.3} s for {:.3} s of signal, {:.3} of real time",
        fed as f64 / rate,
        took * rate / fed as f64
    );
    println!(
        "packets: {} digest {:016x}; in the spans: {} digest {:016x}; rs_failed at each second: {}",
        all.count,
        all.hash,
        inside.count,
        inside.hash,
        failed_at.join(" ")
    );
    let r = Owner::results(&rx);
    println!("end: {}", line(&rx));
    for (d, v) in r.fields().iter() {
        println!("  {} = {v:?}", d.key);
    }
    for t in r.decl().tables() {
        let n = r
            .decl()
            .table_id(&t.key)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.len());
        if n > 0 {
            println!("  table {}: {n} rows", t.key);
        }
    }
}

/// A count and an FNV-1a hash of transport packets with their error flags.
struct Digest {
    count: u64,
    hash: u64,
}

impl Digest {
    fn new() -> Self {
        Digest {
            count: 0,
            hash: 0xcbf2_9ce4_8422_2325,
        }
    }
    fn add(&mut self, p: &TsPacket) {
        self.count += 1;
        for &b in p.bytes().iter().chain(&[p.has_error() as u8]) {
            self.hash = (self.hash ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Adds the packets that the tap holds to `all`, and those with a time in one of `spans` to `inside`.
fn drain(
    tap: &mut sdrkit_graph::TapReader,
    buf: &mut sdrkit_graph::Buffer,
    spans: &[(f64, f64)],
    all: &mut Digest,
    inside: &mut Digest,
) {
    while tap.pop(buf) {
        let sdrkit_graph::Data::Items(Some(v)) = &buf.data else {
            continue;
        };
        let Some(items) = v.as_any().downcast_ref::<Vec<Timed<TsPacket>>>() else {
            continue;
        };
        for p in items {
            all.add(&p.value);
            if spans.iter().any(|s| (s.0..s.1).contains(&p.time)) {
                inside.add(&p.value);
            }
        }
    }
}

/// The result field `k` of `rx` as text.
fn field(rx: &sdrkit_graph::StackGraph, k: &str) -> String {
    let r = Owner::results(rx);
    r.decl()
        .field_id(k)
        .and_then(|id| r.fields().value(id))
        .map_or("-".to_string(), |v| format!("{v:?}"))
}

/// The status summary and the error counts of `rx`.
fn line(rx: &sdrkit_graph::StackGraph) -> String {
    let r = Owner::results(rx);
    let f = |k: &str| field(rx, k);
    format!(
        "{} | mer {} pre {} post {} rs_failed {} packets {}",
        r.status().summary,
        f("mer_db"),
        f("ber_pre_viterbi"),
        f("ber_post_viterbi"),
        f("rs_failed"),
        f("packets"),
    )
}

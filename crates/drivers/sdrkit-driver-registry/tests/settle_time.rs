//! Measurements on a real radio, receive only: the level of the samples from the start of a stream, and the settle time
//! of a retune by the level step between a strong station and an empty frequency.
//!
//! `SDRKIT_RADIO=<address> SDRKIT_STRONG=<MHz> SDRKIT_EMPTY=<MHz> cargo test --release -p sdrkit-driver-registry --test
//! settle_time -- --ignored --nocapture`. `SDRKIT_RATE` (Msps, default 2.4) and `SDRKIT_GAIN` (dB, default the automatic
//! gain of the radio) are optional.
//!
//! A hop run: `SDRKIT_HOPS=<MHz>,<MHz>,...` (in place of the strong and empty frequencies) visits the frequencies in turn,
//! `SDRKIT_HOP_COUNT` times (default 24) for `SDRKIT_DWELL` ms each (default 600). Per hop it reports the time from the
//! first sample at the new frequency until the samples that pass stay within 1 dB of the level the hop settles at, the
//! samples left out later in the hop, and how steady the level that passes is. The stay of a hop begins when its first
//! sample has passed to the reader, as a hop of the engine does. Then per hop: the time from the request to that sample,
//! the samples left out and those that never came, and the share of the nominal samples of the run that passed.
//! `SDRKIT_TRACE=1` adds the level of each 10 ms of a hop. `SDRKIT_SET=<key>=<n>` sets a choice of the driver.

use sdrkit_core::net::StdNet;
use sdrkit_core::{Clock, Complex32, MemStorage, Owner, SettingSet, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RadioSetting, RING_CAPACITY};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The samples of one level reading.
const PART: usize = 256;

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

fn mhz(key: &str) -> Option<f64> {
    env(key).and_then(|s| s.parse::<f64>().ok()).map(|m| m * 1e6)
}

/// What the reader saw: the level of each part with its place in the stream, and the arrival of each read.
#[derive(Default)]
struct Seen {
    levels: Vec<(u64, f32)>,
    arrivals: Vec<(Instant, u64)>,
    /// The stretch of the start that the engine leaves out, in samples of the ring, once its end is known.
    start: Option<(u64, u64)>,
    /// The arrival of the first sample after that stretch, and the place of the level reading it begins.
    first: Option<(Instant, u64)>,
    /// Each stretch of samples without a gap: its place in the ring and in the signal.
    segments: Vec<(u64, u64)>,
    /// The first sample of each retune, by its number, as a place in the ring.
    moved: Vec<(u64, u64)>,
    /// The stretches that the engine leaves out, as places in the ring.
    unsettled: Vec<(u64, u64)>,
    /// The samples that never came from the radio: the place in the signal and the count.
    gaps: Vec<(u64, u64)>,
}

impl Seen {
    /// The place in the signal of the place `at` in the ring.
    fn signal(&self, at: u64) -> u64 {
        let k = self.segments.partition_point(|s| s.0 <= at);
        self.segments
            .get(k.saturating_sub(1))
            .map_or(at, |s| s.1 + (at - s.0))
    }
}

fn level_db(x: &[Complex32]) -> f32 {
    let p = x.iter().map(|c| c.norm_sqr()).sum::<f32>() / x.len().max(1) as f32;
    10.0 * p.max(1e-12).log10()
}

fn median(v: &mut [f32]) -> f32 {
    v.sort_by(f32::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(f32::NAN)
}

#[test]
#[ignore = "a measurement on a real radio"]
fn measure_the_start_and_the_settle_time_of_a_radio() {
    let Some(address) = env("SDRKIT_RADIO") else {
        return;
    };
    let hops: Vec<f64> = env("SDRKIT_HOPS")
        .map(|s| s.split(',').filter_map(|m| m.trim().parse::<f64>().ok()).map(|m| m * 1e6).collect())
        .unwrap_or_default();
    let hop_count = env("SDRKIT_HOP_COUNT").and_then(|s| s.parse::<usize>().ok()).unwrap_or(24);
    let dwell = env("SDRKIT_DWELL").and_then(|s| s.parse::<u64>().ok()).unwrap_or(600);
    let strong = hops.first().copied().or_else(|| mhz("SDRKIT_STRONG")).expect("SDRKIT_STRONG");
    let empty = mhz("SDRKIT_EMPTY").filter(|_| hops.is_empty());
    let rate = mhz("SDRKIT_RATE").unwrap_or(2.4e6);
    let gain = env("SDRKIT_GAIN").and_then(|s| s.parse::<f64>().ok());
    let key = match address.strip_prefix("usb:") {
        Some(rest) => sdrkit_driver_registry::split_usb(rest).expect("a USB family").0,
        None => address.split(':').next().unwrap(),
    }
    .to_string();
    // A network driver takes the address after its key.
    let address = match address.strip_prefix("usb:") {
        Some(_) => address.clone(),
        None => address[key.len() + 1..].to_string(),
    };
    let usb = sdrkit_driver_usb::platform_bus();
    let driver = sdrkit_driver_registry::all(
        Arc::new(|_: &str, _: &Values, _| None),
        Arc::new(MemStorage::new()),
        Arc::new(StdNet),
        Clock::new(|| 0.0),
        usb,
    )
    .into_iter()
    .find(|d| d.descriptor().key == key)
    .expect("a driver for the address");
    let kind = DriverKind::new(driver);
    let schema = kind.schema_at(&address).unwrap();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Centre.id(), &Value::Hz(strong)).unwrap();
    v.set(RadioSetting::Rate.id(), &Value::Hz(rate)).unwrap();
    v.set(RadioSetting::Agc.id(), &Value::Bool(gain.is_none()))
        .unwrap();
    if let Some(g) = gain {
        v.set(RadioSetting::Gain.id(), &Value::F64(g)).unwrap();
    }
    // A setting of the driver by its key, as the number of one of its choices: `SDRKIT_SET=buffer_size=2`.
    if let Some((key, n)) = env("SDRKIT_SET").as_deref().and_then(|s| s.split_once('=')) {
        let id = schema.id(key).expect("a setting of the driver");
        v.set(id, &Value::Choice(n.parse().expect("a choice number"))).unwrap();
    }
    let before_open = Instant::now();
    let mut radio = kind.open(&address, &v).expect("the radio opens");
    let opened = before_open.elapsed();
    let (w, mut r) = ring(RING_CAPACITY, radio.sample_type());
    let stats = r.stats();
    let t0 = Instant::now();
    let started = radio.start(w).expect("the stream starts");
    let rate = started.rate_hz;
    let seen = Arc::new(Mutex::new(Seen::default()));
    let stop = Arc::new(AtomicBool::new(false));
    // The number of the last retune of which a sample has passed to the reader.
    let passed = Arc::new(AtomicU64::new(0));
    let reader = {
        let (seen, stop, passed) = (seen.clone(), stop.clone(), passed.clone());
        std::thread::spawn(move || {
            let mut pos = 0u64;
            let mut buf = vec![Complex32::default(); 1 << 16];
            while !stop.load(Ordering::Acquire) {
                let n = r.wait(1, Duration::from_millis(20)).min(buf.len());
                if n == 0 {
                    continue;
                }
                let at = Instant::now();
                let mut s = seen.lock().unwrap();
                if s.start.is_none() {
                    s.start = s.unsettled.first().copied();
                }
                if let (None, Some((_, to))) = (s.first, s.start) {
                    if r.position() + n as u64 > to {
                        s.first = Some((at, pos + (to.saturating_sub(r.position()))));
                    }
                }
                // A gap is signal time: the samples after it are later by its length.
                let mut done = 0usize;
                let mut gaps = r.take_gaps(n);
                gaps.push(sdrkit_driver_core::Gap::lost_at(n, 0));
                for g in gaps {
                    let ring_at = r.position();
                    if s.segments.last().is_none_or(|x| x.1 - x.0 != pos - ring_at) {
                        s.segments.push((ring_at, pos));
                    }
                    let k = r.read_iq(&mut buf[..g.at - done]);
                    for part in buf[..k].chunks(PART) {
                        s.levels.push((pos, level_db(part)));
                        pos += part.len() as u64;
                    }
                    done += k;
                    if g.lost > 0 {
                        s.gaps.push((pos, g.lost));
                    }
                    pos += g.lost;
                }
                s.arrivals.push((at, pos));
                let st = r.stats();
                let m = st.moves();
                if m.found > 0 && s.moved.last().is_none_or(|x| x.0 != m.found) {
                    s.moved.push((m.found, m.at));
                }
                // From the end of the last stretch seen, so that none is passed over.
                let q = s.unsettled.last().map_or(0, |u| u.1);
                if let Some(to) = st.unsettled(q).and_then(|u| u.to.map(|to| (u.from, to))) {
                    if s.unsettled.last() != Some(&to) {
                        s.unsettled.push(to);
                    }
                }
                let (here, q) = (r.position(), s.unsettled.last().map_or(0, |u| u.1));
                if m.found == m.begun && here > m.at.max(q) && st.unsettled(q).is_none_or(|u| u.from >= here) {
                    passed.store(m.found, Ordering::Release);
                }
            }
        })
    };
    std::thread::sleep(Duration::from_millis(1500));
    // The retunes: (frequency, end of the request, its start).
    let mut tunes = Vec::new();
    if let Some(empty) = empty {
        for k in 0..12 {
            let f = if k % 2 == 0 { empty } else { strong };
            let began = Instant::now();
            radio
                .apply(RadioSetting::Centre.id(), &Value::Hz(f))
                .unwrap();
            tunes.push((f, Instant::now(), began));
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    // The hops: (frequency, number of the retune, end of the request, its start).
    let mut hopped = Vec::new();
    if hops.len() > 1 {
        for k in 0..hop_count {
            let f = hops[(k + 1) % hops.len()];
            let began = Instant::now();
            radio
                .apply(RadioSetting::Centre.id(), &Value::Hz(f))
                .unwrap();
            let seq = stats.moves().begun;
            hopped.push((f, seq, Instant::now(), began));
            while passed.load(Ordering::Acquire) < seq && began.elapsed() < Duration::from_secs(3) {
                std::thread::sleep(Duration::from_millis(1));
            }
            std::thread::sleep(Duration::from_millis(dwell));
        }
    }
    let hops_ended = Instant::now();
    stop.store(true, Ordering::Release);
    reader.join().unwrap();
    radio.stop();
    let s = seen.lock().unwrap();
    // The least delay from the capture of the last sample of a read to its arrival.
    let delay = s
        .arrivals
        .iter()
        .map(|(at, end)| at.duration_since(t0).as_secs_f64() - *end as f64 / rate)
        .fold(f64::INFINITY, f64::min);
    println!(
        "{address}: {:.3} Msps, least delay {:.1} ms, open {:.0} ms",
        rate / 1e6,
        delay * 1e3,
        opened.as_secs_f64() * 1e3
    );
    // The start: the level every 25 ms over the first 2 s.
    let step = (rate * 0.025) as u64;
    let mut line = String::new();
    for k in 0..80u64 {
        let mut p: Vec<f32> = s
            .levels
            .iter()
            .filter(|(at, _)| *at >= k * step && *at < (k + 1) * step)
            .map(|x| x.1)
            .collect();
        if !p.is_empty() {
            line += &format!("{}:{:.1} ", k * 25, median(&mut p));
        }
    }
    println!("start (ms:dB): {line}");
    // What the engine passes on: the first sample after the stretch of the start, and the level from it on.
    if let (Some((from, to)), Some((at, p))) = (s.start, s.first) {
        let mut m: Vec<f32> = Vec::new();
        for k in 0..20u64 {
            let mut v: Vec<f32> = s
                .levels
                .iter()
                .filter(|(x, _)| *x >= p + k * step && *x < p + (k + 1) * step)
                .map(|x| x.1)
                .collect();
            if !v.is_empty() {
                m.push(median(&mut v));
            }
        }
        let (lo, hi) = m.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        // The parts of the first 10 ms against the level of the next 0.5 s.
        let mut all = m.clone();
        let settled = median(&mut all);
        let worst = s
            .levels
            .iter()
            .filter(|(x, _)| *x >= p && *x < p + (rate * 0.01) as u64)
            .map(|x| (x.1 - settled).abs())
            .fold(0.0f32, f32::max);
        println!(
            "first sample to the stack {:.0} ms after the start; left out {:.0} ms of signal; level over the next 0.5 s \
             within {:.1} dB; the parts of its first 10 ms within {worst:.1} dB of it",
            at.duration_since(t0).as_secs_f64() * 1e3,
            (to - from) as f64 / rate * 1e3,
            hi - lo
        );
    }
    // The settle time of each retune: from the end of the request to the capture of the first part within 1 dB of the
    // level that the new frequency settles at (2 dB on a noise floor).
    let place = |t: Instant| ((t.duration_since(t0).as_secs_f64() - delay) * rate).max(0.0) as u64;
    let mut times = Vec::new();
    for (i, (f, end, began)) in tunes.iter().enumerate() {
        let from = place(*began);
        let to = tunes.get(i + 1).map_or(u64::MAX, |n| place(n.2));
        let parts: Vec<(u64, f32)> = s
            .levels
            .iter()
            .copied()
            .filter(|(at, _)| *at >= from && *at < to)
            .collect();
        let mut tail: Vec<f32> = parts.iter().rev().take(200).map(|x| x.1).collect();
        let settled = median(&mut tail);
        let mut before: Vec<f32> = s
            .levels
            .iter()
            .filter(|(at, _)| *at < from && *at + (rate * 0.1) as u64 >= from)
            .map(|x| x.1)
            .collect();
        let was = median(&mut before);
        // The first part from which on 20 parts in a row stay near the settled level.
        let tol = if settled < was { 2.0 } else { 1.0 };
        let mut first = None;
        for k in 0..parts.len() {
            if parts[k..].iter().take(20).all(|x| (x.1 - settled).abs() <= tol) {
                first = Some(parts[k].0);
                break;
            }
        }
        let Some(p) = first else { continue };
        let after_end = |p: u64| (p as f64 / rate + delay - end.duration_since(t0).as_secs_f64()) * 1e3;
        let ms = after_end(p);
        // The stream that came after the request ended until the level changed: the samples in flight and the settle.
        let read = s
            .arrivals
            .iter()
            .take_while(|(at, _)| at <= end)
            .last()
            .map_or(0, |a| a.1);
        let lag = p.saturating_sub(read) as f64 / rate * 1e3;
        // Where the level passes half of the step, in dB.
        let half = (was + settled) / 2.0;
        let mid = parts
            .iter()
            .find(|x| (x.1 - half) * (settled - was) >= 0.0)
            .map_or(f64::NAN, |x| after_end(x.0));
        let asked = end.duration_since(*began).as_secs_f64() * 1e3;
        println!(
            "to {:.3} MHz: {was:.1} -> {settled:.1} dB, request {asked:.1} ms, half way {mid:.1} ms and settled {ms:.1} ms after it, {lag:.1} ms of stream after it",
            f / 1e6
        );
        if (settled - was).abs() >= 6.0 {
            times.push(ms);
        }
    }
    if !times.is_empty() {
        let n = times.len();
        let mut t = times.clone();
        t.sort_by(f64::total_cmp);
        println!(
            "settle over {n} steps of 6 dB or more: median {:.1} ms, largest {:.1} ms",
            t[n / 2],
            t[n - 1]
        );
    }
    // The place in the signal of the sample captured at `t`.
    let at = |t: Instant| ((t.duration_since(t0).as_secs_f64() - delay) * rate).max(0.0) as u64;
    let delivery = hopped.iter().map(|h| (h.3, h.2)).collect::<Vec<_>>();
    let hopped: Vec<(f64, u64, u64)> = hopped.iter().map(|&(f, seq, end, _)| (f, seq, at(end))).collect();
    report_hops(&s, &hopped, rate);
    report_delivery(&s, &hopped, &delivery, hops_ended, rate);
}

/// Per hop (with the start and the end of its request): the time from the request to the arrival of the first sample
/// that passes, the samples left out between the last read before the request and that sample, and those of them that
/// never came from the radio. Then the share of the nominal samples of the whole run that passed.
fn report_delivery(s: &Seen, hopped: &[(f64, u64, u64)], asked: &[(Instant, Instant)], ended: Instant, rate: f64) {
    if hopped.is_empty() {
        return;
    }
    let left: Vec<(u64, u64)> = s.unsettled.iter().map(|&(a, b)| (s.signal(a), s.signal(b))).collect();
    let read_at = |t: Instant| s.arrivals.iter().take_while(|a| a.0 <= t).last().map_or(0, |a| a.1);
    let missing = |a: u64, b: u64| s.gaps.iter().filter(|g| g.0 >= a && g.0 < b).map(|g| g.1).sum::<u64>();
    let ms = |n: u64| n as f64 / rate * 1e3;
    let run_from = read_at(asked[0].0);
    let run_to = read_at(ended);
    let (mut out_all, mut firsts, mut outs, mut lost_all) = (0u64, Vec::new(), Vec::new(), 0u64);
    let mut steps = Vec::new();
    for (i, &(f, seq, _)) in hopped.iter().enumerate() {
        let (began, end) = asked[i];
        let from = read_at(began);
        let to = asked.get(i + 1).map_or(run_to, |n| read_at(n.0));
        let Some(m) = s.moved.iter().find(|m| m.0 >= seq).map(|m| s.signal(m.1)) else {
            out_all += to - from;
            println!("to {:.3} MHz: nothing passed", f / 1e6);
            continue;
        };
        let mut p = m.max(from);
        while let Some(u) = left.iter().find(|u| u.0 <= p && p < u.1) {
            p = u.1;
        }
        let p = p.min(to);
        let later: u64 = left.iter().filter(|u| u.0 > p && u.0 < to).map(|u| u.1.min(to) - u.0).sum();
        let lost = missing(from, to);
        let first = s
            .arrivals
            .iter()
            .find(|a| a.1 > p)
            .map_or(f64::NAN, |a| a.0.duration_since(began).as_secs_f64() * 1e3);
        println!(
            "to {:.3} MHz: request {:.1} ms, first sample to the reader {first:.1} ms after the request began, \
             {} samples ({:.1} ms) left out before it, {} ({:.1} ms) later, {lost} never came",
            f / 1e6,
            end.duration_since(began).as_secs_f64() * 1e3,
            p - from,
            ms(p - from),
            later,
            ms(later)
        );
        // The level of each 10 ms that passed: the first three against the rest of the hop.
        let bin = (rate * 0.01) as u64;
        let mut bins: Vec<f32> = Vec::new();
        for k in 0..(to.saturating_sub(p)) / bin.max(1) {
            let (a, b) = (p + k * bin, p + (k + 1) * bin);
            if left.iter().any(|u| u.0 < b && a < u.1) {
                continue;
            }
            let mut v: Vec<f32> = s.levels.iter().filter(|l| l.0 >= a && l.0 < b).map(|l| l.1).collect();
            if !v.is_empty() {
                bins.push(median(&mut v));
            }
        }
        if bins.len() >= 8 {
            let mut rest = bins[3..].to_vec();
            let m = median(&mut rest);
            steps.push(bins[..3].iter().map(|x| (x - m).abs()).fold(0.0f32, f32::max));
        }
        if env("SDRKIT_TRACE").is_some() {
            // The level of each 10 ms from the last read before the request; in brackets what is left out.
            let step = (rate * 0.01) as u64;
            let mut line = String::new();
            for k in 0..40u64 {
                let (a, b) = (from + k * step, from + (k + 1) * step);
                let mut v: Vec<f32> = s.levels.iter().filter(|l| l.0 >= a && l.0 < b).map(|l| l.1).collect();
                if a >= to || v.is_empty() {
                    break;
                }
                let out = a < p || left.iter().any(|u| u.0 <= a && a < u.1);
                line += &if out { format!("[{:.0}] ", median(&mut v)) } else { format!("{:.0} ", median(&mut v)) };
            }
            println!("  {line}");
        }
        out_all += p - from + later;
        lost_all += lost;
        firsts.push(first);
        outs.push(p - from);
    }
    let nominal = ended.duration_since(asked[0].0).as_secs_f64() * rate;
    let passed = (run_to - run_from).saturating_sub(out_all);
    firsts.sort_by(f64::total_cmp);
    outs.sort_unstable();
    let n = firsts.len();
    if n == 0 {
        return;
    }
    if !steps.is_empty() {
        let worst = steps.iter().copied().fold(0.0f32, f32::max);
        println!(
            "the level of the first 30 ms that pass against the rest of the hop: median {:.1} dB, largest {worst:.1} dB",
            median(&mut steps)
        );
    }
    println!(
        "{} hops: first sample to the reader median {:.1} ms, largest {:.1} ms; left out per hop median {} ({:.1} ms), \
         largest {} ({:.1} ms); {lost_all} never came; passed {:.1} % of the nominal samples ({:.2} of {:.2} Msps)",
        hopped.len(),
        firsts[n / 2],
        firsts[n - 1],
        outs[n / 2],
        ms(outs[n / 2]),
        outs[n - 1],
        ms(outs[n - 1]),
        100.0 * passed as f64 / nominal,
        passed as f64 / nominal * rate / 1e6,
        rate / 1e6
    );
}

/// Per hop (frequency, number of the retune, place in the signal of the end of the request): the time from the end of the
/// request to the first sample at the new frequency, from there until the samples that pass stay within 1 dB of the level
/// the hop settles at, how far the level that passes strays from it, and the samples left out later in the hop.
fn report_hops(s: &Seen, hopped: &[(f64, u64, u64)], rate: f64) {
    if hopped.is_empty() {
        return;
    }
    let left: Vec<(u64, u64)> = s.unsettled.iter().map(|&(a, b)| (s.signal(a), s.signal(b))).collect();
    let end = s.levels.last().map_or(0, |l| l.0);
    let win = ((rate * 0.001) as u64).max(1);
    let (mut lost, mut strays, mut later_all) = (Vec::new(), Vec::new(), 0.0);
    let mut firsts = Vec::new();
    for (i, &(f, seq, asked)) in hopped.iter().enumerate() {
        let found = |q: u64| s.moved.iter().find(|m| m.0 >= q).map(|m| s.signal(m.1));
        let Some(m) = found(seq) else {
            println!("to {:.3} MHz: no first sample found", f / 1e6);
            continue;
        };
        let to = hopped.get(i + 1).and_then(|h| found(h.1)).unwrap_or(end).max(m);
        // The first sample that passes.
        let mut p = m;
        while let Some(u) = left.iter().find(|u| u.0 <= p && p < u.1) {
            p = u.1;
        }
        // The levels of 1 ms of passed samples each.
        let mut wins: Vec<(u64, Vec<f32>)> = Vec::new();
        for &(at, l) in s.levels.iter().filter(|(at, _)| *at >= p && *at < to) {
            if left.iter().any(|u| u.0 <= at && at < u.1) {
                continue;
            }
            let k = (at - p) / win;
            match wins.last_mut() {
                Some(w) if w.0 == k => w.1.push(l),
                _ => wins.push((k, vec![l])),
            }
        }
        let meds: Vec<(u64, f32)> = wins.into_iter().map(|(k, mut v)| (k, median(&mut v))).collect();
        if meds.len() < 4 {
            println!("to {:.3} MHz: too little passed", f / 1e6);
            continue;
        }
        let mut tail: Vec<f32> = meds[meds.len() / 2..].iter().map(|x| x.1).collect();
        let settled = median(&mut tail);
        let steady = (0..meds.len())
            .find(|&k| meds[k..].iter().all(|x| (x.1 - settled).abs() <= 1.0))
            .map_or(to, |k| p + meds[k].0 * win);
        let ms = |n: u64| n as f64 / rate * 1e3;
        let stray = meds.iter().map(|x| (x.1 - settled).abs()).fold(0.0f32, f32::max);
        let later: u64 = left
            .iter()
            .filter(|u| u.0 >= p && u.1 <= to)
            .map(|u| u.1 - u.0)
            .sum();
        let first = (m as f64 - asked as f64) / rate * 1e3;
        firsts.push(first);
        println!(
            "to {:.3} MHz: {settled:.1} dB, first sample {first:.1} ms after the request, passes {:.1} ms and steady {:.1} ms after it, level within {stray:.1} dB, {:.1} ms left out later",
            f / 1e6,
            ms(p - m),
            ms(steady - m),
            ms(later)
        );
        lost.push(ms(steady - m));
        strays.push(stray);
        later_all += ms(later);
    }
    if lost.is_empty() {
        return;
    }
    let n = lost.len();
    let mut l = lost.clone();
    l.sort_by(f64::total_cmp);
    let worst = strays.iter().copied().fold(0.0f32, f32::max);
    let mut st = strays.clone();
    firsts.sort_by(f64::total_cmp);
    println!(
        "first sample after the request: median {:.1} ms, largest {:.1} ms",
        firsts[firsts.len() / 2],
        firsts[firsts.len() - 1]
    );
    println!(
        "{n} hops: steady after median {:.1} ms, largest {:.1} ms; level within median {:.1} dB, largest {worst:.1} dB; {later_all:.0} ms left out later in all",
        l[n / 2],
        l[n - 1],
        median(&mut st)
    );
}

//! Helpers of the tests of the keyboard senders: a stack set to send, with its transmit stream reached as the engine reaches
//! it (the stack's graph with the host streams in its context), the commands, the bursts written, noise at an SNR in 2.5 kHz
//! against the burst's own power, the 99 % power bandwidth and the checks every sender shares.
#![allow(dead_code)]

use crate::common::{noise_only, RATE};
use hf::typed::KeyboardTxFields;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Clock, Complex32, InputInfo, Output, Owner, Receiver, StackConfig, StackDescriptor,
    Value, Values,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_hfkbd as hf;
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the sending stack.
pub const DIAL: f64 = 14.07e6;
/// The clock of a host that times bursts.
pub const NOW: f64 = 1_800_000_000.0;
/// The station callsign of the tests.
pub const CALL: &str = "N0CALL";

/// One write to the transmit stream: its samples, its end mark and its stated start.
type Write = (Vec<Complex32>, bool, Option<f64>);

/// The transmit stream: each write, and the lead that the host asks for.
#[derive(Default)]
pub struct Sink {
    writes: Mutex<Vec<Write>>,
    lead: Duration,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                end,
                start,
                ..
            },
        ) = (port, data)
        {
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), end, start));
        }
    }
    fn transmit_lead(&self) -> Duration {
        self.lead
    }
}

/// One burst: its samples, whether its end mark came, and the stated starts in it (the place and the UTC).
pub struct Burst {
    pub samples: Vec<Complex32>,
    pub ended: bool,
    pub starts: Vec<(usize, f64)>,
}

impl Sink {
    /// The writes after the first `from`, each its samples and its end mark; `from` moves past them.
    pub fn take_since(&self, from: &mut usize) -> Vec<(Vec<Complex32>, bool)> {
        let w = self.writes.lock().unwrap();
        let new = w[(*from).min(w.len())..]
            .iter()
            .map(|(s, end, _)| (s.clone(), *end))
            .collect();
        *from = w.len();
        new
    }

    pub fn bursts(&self) -> Vec<Burst> {
        let mut out: Vec<Burst> = Vec::new();
        let mut open = false;
        for (s, end, start) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push(Burst {
                    samples: Vec::new(),
                    ended: false,
                    starts: Vec::new(),
                });
            }
            let b = out.last_mut().unwrap();
            if let Some(utc) = start {
                b.starts.push((b.samples.len(), *utc));
            }
            b.samples.extend_from_slice(s);
            b.ended = *end;
            open = !end;
        }
        out
    }
}

/// `set` as the settings of a case.
pub fn cases(set: &[(&'static str, Value)]) -> Vec<sdrkit_core::verify::CaseSetting> {
    set.iter()
        .map(|(k, v)| sdrkit_core::verify::CaseSetting {
            key: k,
            value: v.clone(),
        })
        .collect()
}

/// The settings of `desc` with `set` (key and value) on its defaults.
pub fn values(desc: &'static StackDescriptor, set: &[(&str, Value)]) -> Values {
    let schema = (desc.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in set {
        s.set(schema.id(k).unwrap_or_else(|_| panic!("no setting {k}")), v)
            .unwrap();
    }
    s
}

/// The stack `desc` with `set`, transmit on, the station callsign [`CALL`], at [`RATE`] on [`DIAL`], and its transmit stream.
/// With `lead`, on a live radio whose host times bursts with that lead, at the clock [`NOW`].
pub fn station(
    desc: &'static StackDescriptor,
    set: &[(&str, Value)],
    lead: Option<Duration>,
) -> (Box<dyn Receiver>, Arc<Sink>) {
    station_of(desc, set, lead, CALL, "")
}

/// [`station`] with the station callsign `call` and the locator `locator`.
pub fn station_of(
    desc: &'static StackDescriptor,
    set: &[(&str, Value)],
    lead: Option<Duration>,
    call: &str,
    locator: &str,
) -> (Box<dyn Receiver>, Arc<Sink>) {
    let mut rx = (desc.make_receiver)(&StackConfig::new(values(desc, set))).unwrap();
    let input = InputInfo::new(RATE, DIAL, DIAL);
    rx.configure(&if lead.is_some() {
        input
    } else {
        input.recording()
    })
    .unwrap();
    let sink = Arc::new(Sink {
        lead: lead.unwrap_or_default(),
        ..Sink::default()
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    ctx.clock = Clock::fixed(NOW);
    let mut st = (*ctx.station).clone();
    st.callsign = call.into();
    st.locator = locator.into();
    ctx.station = Arc::new(st);
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, sink)
}

/// Runs command `key` with the arguments `args`.
pub fn command(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<(), sdrkit_core::CommandError> {
    let id = Owner::commands(rx).id(key).unwrap();
    let mut a = Owner::commands(rx).def(id).unwrap().new_args();
    for (k, v) in args {
        let i = a.decl().id(k).unwrap();
        a.set_value(i, v).unwrap();
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

/// Sends `text`.
pub fn send(rx: &mut dyn Receiver, text: &str) -> Result<(), sdrkit_core::CommandError> {
    command(rx, "send", &[("text", Value::Text(text.into()))])
}

/// Feeds `seconds` of silence in pieces of 0.1 s.
pub fn idle(rx: &mut dyn Receiver, seconds: f64) {
    let zeros = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

/// Feeds silence until nothing is sent, at most `limit` seconds.
pub fn until_quiet(rx: &mut dyn Receiver, limit: f64) {
    for _ in 0..(limit * 10.0) as usize {
        idle(rx, 0.1);
        if !fields(rx).tx_sending {
            idle(rx, 0.2);
            return;
        }
    }
    panic!("still sending after {limit} s");
}

/// The transmit fields.
pub fn fields(rx: &dyn Receiver) -> KeyboardTxFields {
    KeyboardTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// The one burst that `desc` with `set` sends for `text`; it must end with its end mark.
pub fn one_burst(
    desc: &'static StackDescriptor,
    set: &[(&str, Value)],
    text: &str,
) -> Vec<Complex32> {
    let (mut rx, sink) = station(desc, set, None);
    send(rx.as_mut(), text).expect("the text is sent");
    until_quiet(rx.as_mut(), 1200.0);
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "one burst");
    assert!(b[0].ended, "the end mark");
    b.remove(0).samples
}

/// The first and the last sample above 1e-6 in magnitude.
pub fn signal_span(x: &[Complex32]) -> (usize, usize) {
    let a = x.iter().position(|z| z.norm() > 1e-6).expect("a signal");
    let b = x.iter().rposition(|z| z.norm() > 1e-6).unwrap();
    (a, b + 1)
}

/// The mean power of the signal between its first and its last sample.
pub fn signal_power(x: &[Complex32]) -> f64 {
    let (a, b) = signal_span(x);
    x[a..b].iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / (b - a) as f64
}

/// `x` between 1 s and `after` s of silence, with white noise at `snr_db` in 2.5 kHz against the power of its signal.
pub fn noisy(x: &[Complex32], snr_db: f64, after: f64, seed: u64) -> Vec<Complex32> {
    let mut y = vec![Complex32::default(); RATE as usize];
    y.extend_from_slice(x);
    y.extend(std::iter::repeat_n(
        Complex32::default(),
        (after * RATE) as usize,
    ));
    let gain = (signal_power(x) / 10f64.powf(snr_db / 10.0)).sqrt() as f32;
    let noise = noise_only(y.len() as f64 / RATE + 1.0, seed);
    for (s, n) in y.iter_mut().zip(noise) {
        *s += n * gain;
    }
    y
}

/// `x` followed by `after` s of silence.
pub fn followed(x: &[Complex32], after: f64) -> Vec<Complex32> {
    let mut y = x.to_vec();
    y.extend(std::iter::repeat_n(
        Complex32::default(),
        (after * RATE) as usize,
    ));
    y
}

/// The level of the power spectral density between `lo` and `hi` Hz under its peak, dB: the mean of Hann-windowed spectra of
/// 8192 points with half overlap.
pub fn psd_below_peak(x: &[Complex32], lo: f64, hi: f64) -> f64 {
    let n = 8192;
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(n);
    let mut acc = vec![0.0f64; n];
    let mut buf = vec![Complex32::default(); n];
    let (a, b) = signal_span(x);
    let mut i = a;
    while i + n <= b {
        for k in 0..n {
            let w = 0.5 - 0.5 * (TAU * k as f64 / n as f64).cos();
            buf[k] = x[i + k] * w as f32;
        }
        fft.process(&mut buf);
        for k in 0..n {
            acc[k] += f64::from(buf[k].norm_sqr());
        }
        i += n / 2;
    }
    let hz = |k: usize| if k < n / 2 { k as f64 } else { k as f64 - n as f64 } * RATE / n as f64;
    let peak = acc.iter().copied().fold(0.0, f64::max);
    let worst = (0..n)
        .filter(|&k| (lo..=hi).contains(&hz(k)))
        .map(|k| acc[k])
        .fold(0.0, f64::max);
    10.0 * (peak / worst.max(1e-300)).log10()
}

/// `x` between 1 s and `after` s of silence.
pub fn padded(x: &[Complex32], after: f64) -> Vec<Complex32> {
    let mut y = vec![Complex32::default(); RATE as usize];
    y.extend_from_slice(x);
    y.extend(std::iter::repeat_n(
        Complex32::default(),
        (after * RATE) as usize,
    ));
    y
}

/// The frequency of each phase step of `x`, Hz: element n is the step from sample n to n + 1.
pub fn steps(x: &[Complex32]) -> Vec<f64> {
    x.windows(2)
        .map(|p| f64::from((p[1] * p[0].conj()).arg()) * RATE / TAU)
        .collect()
}

/// The checks of every burst: the first 0.15 s and the last 0.1 s are silent; the 10 ms fades follow the raised cosine; the
/// largest magnitude is `level` within 0.5 dB.
pub fn check_frame(x: &[Complex32], level: f64) {
    let lead = (0.15 * RATE) as usize;
    let tail = (0.1 * RATE) as usize;
    assert!(x[..lead].iter().all(|z| z.norm() < 1e-6), "silence before");
    assert!(
        x[x.len() - tail..].iter().all(|z| z.norm() < 1e-6),
        "silence after"
    );
    assert!(x[lead].norm() < 1e-6, "the fade starts at 0");
    let peak = x.iter().map(|z| f64::from(z.norm())).fold(0.0, f64::max);
    let db = 20.0 * (peak / level).log10();
    assert!(db.abs() < 0.5, "peak {peak} for a level {level}");
    // The fade: sample i of 120 at the start has at most the gain 0.5 - 0.5 cos(pi i / 120) of the level.
    let f = (0.01 * RATE) as usize;
    for i in 0..f {
        let g = 0.5 - 0.5 * (PI * i as f64 / f as f64).cos();
        assert!(
            f64::from(x[lead + i].norm()) <= g * level + 1e-6,
            "fade at sample {i}"
        );
    }
}

/// The 99 % power bandwidth of `x` (ITU-R SM.328): the mean of Hann-windowed spectra of 8192 points with half overlap,
/// between the frequencies under which 0.5 % and above which 0.5 % of the power lie. Returns (low, high), Hz.
pub fn occupied(x: &[Complex32]) -> (f64, f64) {
    let n = 8192;
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(n);
    let mut acc = vec![0.0f64; n];
    let mut buf = vec![Complex32::default(); n];
    let (a, b) = signal_span(x);
    let mut i = a;
    while i + n <= b {
        for k in 0..n {
            let w = 0.5 - 0.5 * (TAU * k as f64 / n as f64).cos();
            buf[k] = x[i + k] * w as f32;
        }
        fft.process(&mut buf);
        for k in 0..n {
            acc[k] += f64::from(buf[k].norm_sqr());
        }
        i += n / 2;
    }
    // Bins from -rate/2 to +rate/2.
    let mut bins: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let f = if k < n / 2 {
                k as f64
            } else {
                k as f64 - n as f64
            } * RATE
                / n as f64;
            (f, acc[k])
        })
        .collect();
    bins.sort_by(|p, q| p.0.total_cmp(&q.0));
    let total: f64 = bins.iter().map(|b| b.1).sum();
    let mut c = 0.0;
    let (mut lo, mut hi) = (f64::NAN, f64::NAN);
    for (f, p) in &bins {
        c += p;
        if lo.is_nan() && c >= 0.005 * total {
            lo = *f;
        }
        if hi.is_nan() && c >= 0.995 * total {
            hi = *f;
        }
    }
    (lo, hi)
}

/// `stop` during a burst: the burst ends at once with the fade, the end mark, and nothing after; a new text can be sent.
pub fn check_stop(desc: &'static StackDescriptor, set: &[(&str, Value)], text: &str, after_s: f64) {
    let (mut rx, sink) = station(desc, set, None);
    send(rx.as_mut(), text).unwrap();
    idle(rx.as_mut(), after_s);
    assert!(fields(rx.as_ref()).tx_sending);
    let before: usize = sink.bursts()[0].samples.len();
    command(rx.as_mut(), "stop", &[]).unwrap();
    idle(rx.as_mut(), 1.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended, "the end mark");
    assert!(!fields(rx.as_ref()).tx_sending);
    let x = &b[0].samples;
    let f = (0.01 * RATE) as usize;
    assert!(
        x.len() <= before + f + 1,
        "{} after the stop",
        x.len() - before
    );
    assert!(x.last().unwrap().norm() < 0.05, "a clean tail");
    // The magnitude falls over the fade: sample i after the stop has at most the gain 0.5 - 0.5 cos(pi (f - 1 - i) / f) of
    // the level.
    for (i, z) in x[before..].iter().enumerate() {
        let j = (f - 1).saturating_sub(i);
        let g = 0.5 - 0.5 * (PI * j as f64 / f as f64).cos();
        assert!(
            f64::from(z.norm()) <= 0.5 * g + 1e-4,
            "sample {i} after the stop"
        );
    }
    send(rx.as_mut(), text).expect("a new text after the stop");
}

/// With transmit off `send` is refused and nothing is written; with transmit on, a burst states its start on a live radio
/// whose host gives a lead, and transmit going off ends the burst with the end mark.
pub fn check_gate(desc: &'static StackDescriptor, set: &[(&str, Value)], text: &str) {
    let (mut rx, sink) = station(desc, set, None);
    rx.set_transmit(false);
    assert!(send(rx.as_mut(), text).is_err());
    assert!(command(rx.as_mut(), "tune", &[]).is_err());
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
    rx.set_transmit(true);
    send(rx.as_mut(), text).unwrap();
    idle(rx.as_mut(), 0.5);
    rx.set_transmit(false);
    idle(rx.as_mut(), 0.5);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    // The stated start.
    let (mut rx, sink) = station(desc, set, Some(Duration::from_millis(300)));
    send(rx.as_mut(), text).unwrap();
    idle(rx.as_mut(), 0.5);
    let b = sink.bursts();
    assert_eq!(b[0].starts.len(), 1);
    let (at, utc) = b[0].starts[0];
    assert_eq!(at, 0);
    assert!((utc - (NOW + 0.3 + 0.2)).abs() < 1e-6, "{}", utc - NOW);
}

/// The tune carrier: a steady tone at `centre` Hz for the tune time, with the frame of a burst.
pub fn check_tune(desc: &'static StackDescriptor, set: &[(&str, Value)], centre: f64) {
    let mut set = set.to_vec();
    set.push(("tx_tune_limit", Value::Duration(2.0)));
    let (mut rx, sink) = station(desc, &set, None);
    command(rx.as_mut(), "tune", &[]).unwrap();
    assert!(
        send(rx.as_mut(), "E").is_err(),
        "no text during the tune carrier"
    );
    until_quiet(rx.as_mut(), 10.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    let x = &b[0].samples;
    check_frame(x, 0.5);
    let (a, e) = signal_span(x);
    let secs = (e - a) as f64 / RATE;
    assert!((secs - 2.0).abs() < 0.01, "{secs} s");
    let f = steps(&x[a + 200..e - 200]);
    assert!(
        f.iter().all(|v| (v - centre).abs() < 0.5),
        "the carrier at {centre} Hz"
    );
}

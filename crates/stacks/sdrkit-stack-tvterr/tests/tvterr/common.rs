//! Helpers of the tests: making a receiver, running it over samples while taking its transport packets, and impairing a test
//! signal with the shared impairment chain.
#![allow(dead_code)]

use sdrkit_algo_sim::chain::{Chain, ChainConfig, SignalInfo};
use sdrkit_algo_sim::noise::NoiseLevel;
use sdrkit_core::{
    Complex32, Event, Generator, HostRequest, InputInfo, Output, Receiver, Results, StackConfig,
    StackDescriptor, Value, Values,
};
use sdrkit_graph::{Buffer, Data, Graph, Timed};
use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck};
use sdrkit_proto_mpegts::stream::{self, TsPacket};

/// Bytes in a transport packet.
pub const TS: usize = 188;

/// What a run produced.
pub struct Run {
    pub packets: Vec<([u8; TS], f64)>,
    pub results: Results,
    pub events: Vec<Event>,
    pub requests: Vec<HostRequest>,
}

impl Run {
    fn value(&self, key: &str) -> Option<&Value> {
        let f = self.results.fields();
        f.value(f.decl().id(key)?)
    }
    /// A number of the results (NaN when absent).
    pub fn f(&self, key: &str) -> f64 {
        match self.value(key) {
            Some(Value::F64(v) | Value::Hz(v) | Value::Duration(v) | Value::Time(v)) => *v,
            Some(Value::I64(v)) => *v as f64,
            _ => f64::NAN,
        }
    }
    /// A yes-or-no field of the results.
    pub fn b(&self, key: &str) -> bool {
        matches!(self.value(key), Some(Value::Bool(true)))
    }
    /// A text field, or the key of a choice field, of the results.
    pub fn t(&self, key: &str) -> String {
        let f = self.results.fields();
        let Some(id) = f.decl().id(key) else {
            return String::new();
        };
        match f.value(id) {
            Some(Value::Text(t)) => t.to_string(),
            Some(Value::Choice(i)) => f
                .decl()
                .field(id)
                .and_then(|d| d.choices.get(usize::from(*i)))
                .map(|c| c.key.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
    /// The rows of table `key`.
    pub fn rows(&self, key: &str) -> usize {
        self.results
            .decl()
            .table_id(key)
            .and_then(|id| self.results.table(id))
            .map_or(0, |t| t.len())
    }
    /// The numbered packets of `pattern` the run delivered.
    pub fn check(&self, pattern: &Numbered) -> NumberedCheck {
        let mut c = NumberedCheck::default();
        for (p, t) in &self.packets {
            c.push(pattern, p, *t);
        }
        c
    }
}

/// The settings of stack `d`: these values over its defaults. A value is a choice key or `true` or `false`.
pub fn values(d: &StackDescriptor, settings: &[(&str, &str)]) -> Values {
    let schema = (d.settings)();
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        let id = schema.id(k).expect("a setting of the stack");
        let decl = &schema.setting(id).expect("declared").field;
        let value = match *x {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            key => Value::Choice(decl.choice_index(key).expect("a choice of the setting")),
        };
        v.set(id, &value).expect("a value the stack takes");
    }
    v
}

/// The receiver of stack `d` with these settings over its defaults.
pub fn receiver(d: &StackDescriptor, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    (d.make_receiver)(&StackConfig::new(values(d, settings))).expect("the receiver builds")
}

/// Feeds `x` in pieces of `chunk` and collects the transport packets of the graph port `port` (empty for none).
pub fn run(rx: &mut dyn Receiver, rate: f64, x: &[Complex32], chunk: usize, port: &str) -> Run {
    rx.configure(&InputInfo::new(rate, 539e6, 539e6).recording())
        .expect("rate accepted");
    let mut tap = (!port.is_empty()).then(|| {
        let g: &mut Graph = rx
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .expect("a graph");
        g.tap(port, 1 << 16).expect("port")
    });
    let mut out = Output::default();
    let mut packets = Vec::new();
    let mut events = Vec::new();
    let mut requests = Vec::new();
    let mut buf = Buffer::new(&stream::kind());
    for c in x.chunks(chunk.max(1)) {
        rx.feed(c, &mut out);
        events.append(&mut out.events);
        requests.append(&mut out.requests);
        out.clear();
        if let Some(tap) = &mut tap {
            while tap.pop(&mut buf) {
                if let Data::Items(Some(v)) = &buf.data {
                    if let Some(items) = v.as_any().downcast_ref::<Vec<Timed<TsPacket>>>() {
                        packets.extend(items.iter().map(|p| (*p.value.bytes(), p.time)));
                    }
                }
            }
        }
    }
    Run {
        packets,
        results: sdrkit_core::Owner::results(rx).clone(),
        events,
        requests,
    }
}

/// `seconds` of a generator's signal.
pub fn generate<G: Generator>(mut g: G, seconds: f64) -> Vec<Complex32> {
    let mut x = vec![Complex32::new(0.0, 0.0); (seconds * g.rate_hz()) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

/// Impairments for [`impair`]: a carrier offset, a clock error, an SNR in the signal's bandwidth, and the rest of the chain.
pub fn chain(
    offset_hz: f64,
    clock_ppm: f64,
    snr_db: Option<f64>,
    bandwidth_hz: f64,
) -> ChainConfig {
    ChainConfig {
        offset_hz,
        clock_ppm,
        noise: match snr_db {
            Some(snr_db) => NoiseLevel::SnrDb {
                snr_db,
                bandwidth_hz,
            },
            None => NoiseLevel::Off,
        },
        ..Default::default()
    }
}

/// `x` (at `rate`) through the shared impairment chain `cfg`.
pub fn impair(x: &[Complex32], rate: f64, cfg: &ChainConfig) -> Vec<Complex32> {
    let mut c = Chain::new(cfg, &SignalInfo::new(rate)).expect("a valid chain");
    let mut y = Vec::with_capacity(x.len() + 1024);
    c.process(x, &mut y);
    y
}

/// White noise of total power `power`.
pub fn noise(n: usize, power: f64, seed: u64) -> Vec<Complex32> {
    let mut r = sdrkit_algo_sim::rng::Rng::new(seed);
    (0..n).map(|_| r.complex_normal(power)).collect()
}

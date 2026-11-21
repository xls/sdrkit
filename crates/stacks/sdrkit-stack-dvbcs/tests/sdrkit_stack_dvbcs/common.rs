//! Helpers: a receiver with a tap on its transport stream, feeding in pieces, and the check of the numbered packets.

#![allow(dead_code)]

use sdrkit_core::interface::Column;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::Field;
use sdrkit_core::record::Kind as FieldKind;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, InputInfo, Output, Receiver, Results, StackConfig, StackDescriptor, Value, Values,
};
use sdrkit_graph::{Buffer, Data, GraphError, Kind, StackGraph, TapReader, Timed};
use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck};
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_stack_dvbcs::{beacon, dvbc, dvbc2, dvbs, isdbs};

/// The settings of a stack: the defaults with `settings` set.
pub fn config(desc: &StackDescriptor, settings: &[CaseSetting]) -> StackConfig {
    let schema = (desc.settings)();
    let mut v = Values::defaults(&schema);
    for s in settings {
        let id = schema.id(s.key).unwrap();
        v.set(id, &s.value).unwrap();
    }
    StackConfig::new(v)
}

/// A recording's input.
pub fn recording(rate: f64, centre: f64) -> InputInfo {
    InputInfo::new(rate, centre, centre).recording()
}

/// The receiver function of a stack of this crate.
fn receiver_of(desc: &StackDescriptor) -> fn(&StackConfig) -> Result<StackGraph, GraphError> {
    match desc.id {
        dvbc::ID => dvbc::receiver,
        dvbc2::ID => dvbc2::receiver,
        dvbs::ID => dvbs::receiver,
        isdbs::ID => isdbs::receiver,
        _ => beacon::receiver,
    }
}

/// A receiver of a stack with a tap on the transport stream it delivers (the output of the monitor).
pub struct Rx {
    /// The receiver.
    pub rx: StackGraph,
    tap: Option<TapReader>,
    buf: Buffer,
    /// Every transport packet delivered.
    pub packets: Vec<[u8; 188]>,
    /// Every event.
    pub events: Vec<sdrkit_core::Event>,
}

impl Rx {
    /// A receiver of `desc` with these settings, configured for `input`.
    pub fn new(desc: &StackDescriptor, settings: &[CaseSetting], input: &InputInfo) -> Rx {
        let mut rx = receiver_of(desc)(&config(desc, settings)).unwrap();
        rx.configure(input).unwrap();
        let tap = rx.graph_mut().tap("monitor.out", 1 << 14).ok();
        Rx {
            rx,
            tap,
            buf: Buffer::new(&Kind::Bytes),
            packets: Vec::new(),
            events: Vec::new(),
        }
    }

    fn drain(&mut self, out: &mut Output) {
        self.events.append(&mut out.events);
        out.clear();
        let Some(t) = self.tap.as_mut() else { return };
        while t.pop(&mut self.buf) {
            if let Data::Items(Some(v)) = &self.buf.data {
                if let Some(items) = v.as_any().downcast_ref::<Vec<Timed<TsPacket>>>() {
                    self.packets.extend(items.iter().map(|p| *p.value.bytes()));
                }
            }
        }
    }

    /// Feeds `x` in pieces of `piece` samples.
    pub fn feed(&mut self, x: &[Complex32], piece: usize) {
        let mut out = Output::default();
        for c in x.chunks(piece.max(1)) {
            self.rx.feed(c, &mut out);
            self.drain(&mut out);
        }
    }

    /// Feeds `x` in pieces of mixed sizes (1, 7, 4096, 65536, 333, 12345).
    pub fn feed_mixed(&mut self, x: &[Complex32]) {
        let sizes = [1usize, 7, 4096, 65536, 333, 12345];
        let mut out = Output::default();
        let (mut i, mut k) = (0, 0);
        while i < x.len() {
            let n = sizes[k % sizes.len()].min(x.len() - i);
            self.rx.feed(&x[i..i + n], &mut out);
            self.drain(&mut out);
            i += n;
            k += 1;
        }
    }

    /// Ends the stream: the receiver delivers what it still holds.
    pub fn finish(&mut self) {
        let mut out = Output::default();
        self.rx.finish(&mut out);
        self.drain(&mut out);
    }

    /// The check of the numbered packets of the satellite and cable pattern among those delivered.
    pub fn check(&self) -> NumberedCheck {
        let mut c = NumberedCheck::default();
        for p in &self.packets {
            c.push(&Numbered::SATELLITE_CABLE, p, 0.0);
        }
        c
    }

    /// The results.
    pub fn results(&self) -> &Results {
        Owner::results(&self.rx)
    }

    /// The value of result field `f`.
    pub fn get<K: FieldKind>(&self, f: &Field<K>) -> Option<K::Out<'_>> {
        self.results().fields().get(f)
    }

    /// The value of the result field of column `c`.
    pub fn at<K: FieldKind>(&self, c: &Column<K>) -> Option<K::Out<'_>> {
        self.results().fields().at(c)
    }

    /// A number field by key, NaN when it has none.
    pub fn num(&self, key: &str) -> f64 {
        let r = self.results();
        match r.decl().field_id(key).and_then(|id| r.fields().value(id)) {
            Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x)) => *x,
            Some(Value::I64(x)) => *x as f64,
            _ => f64::NAN,
        }
    }

    /// Writes a stack setting through the checked path.
    pub fn apply(&mut self, key: &str, v: &Value) {
        let id = self.rx.schema().id(key).unwrap();
        owner::apply(&mut self.rx, id, v).unwrap();
    }

    /// Prints the log and the summary.
    pub fn dump(&self) {
        for l in self.results().log_lines() {
            println!("{l}");
        }
        println!("{}", self.results().status().summary);
    }
}

/// The rows of table `key` of `r`.
pub fn rows(r: &Results, key: &str) -> usize {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// The generator of `desc` with these settings of the stack and of the standard `gen_*` impairments, `seconds` long at
/// `rate`.
pub fn generate(
    desc: &'static StackDescriptor,
    rate: f64,
    seconds: f64,
    settings: &[CaseSetting],
) -> Vec<Complex32> {
    sdrkit_dev_test::Harness::new(desc)
        .rate(rate)
        .seconds(seconds)
        .with_settings(settings)
        .generate()
        .unwrap()
}

//! Bits to bytes and back as adapters of the graph: validate packs hard bits for an input of packed bytes in a declared bit
//! order, and unpacks bytes that state their bit order for an input of hard bits.

use sdrkit_core::{Complex32, ConfigError, Output, Values};
use sdrkit_graph::*;
use std::sync::{Arc, Mutex};

/// Iq in; out the bits 1, 0, 1, 1, 0, 0, 0, 1 again and again (one per sample), or the byte 0xB1 per 8 samples in bytes
/// that state MSB first.
struct Source(Kind);

impl Component for Source {
    fn info(&self) -> ComponentInfo {
        let mut out = Port::new("out", self.0).rate_from(RateFrom::Fixed(0.0));
        if self.0 == Kind::Bytes {
            out = out.bit_order(BitOrder::MsbFirst);
        }
        ComponentInfo::processor("test.source", vec![Port::iq("in")], vec![out])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            bit_order: (self.0 == Kind::Bytes).then_some(BitOrder::MsbFirst),
            ..Format::new(self.0, 0.0)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.iq(0).len() / 8;
        if self.0 == Kind::HardBits {
            let byte = [1u8, 0, 1, 1, 0, 0, 0, 1];
            io.hard_bits_out(0)
                .extend((0..n).flat_map(|_| byte.iter().copied()));
        } else {
            io.bytes_out(0)
                .push(Packet::new(Arc::from(vec![0xB1u8; n]), 0.0));
        }
    }
}

/// A sink with input `port` that keeps the bytes or bits it sees.
struct Sink(Port, Arc<Mutex<Vec<u8>>>);

impl Component for Sink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.sink", vec![self.0.clone()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut v = self.1.lock().unwrap();
        match io.input_format(0).kind {
            Kind::Bytes => v.extend(io.bytes(0).iter().flat_map(|p| p.bytes().iter().copied())),
            _ => v.extend_from_slice(io.hard_bits(0)),
        }
    }
}

fn run(from: Kind, want: Port) -> (Vec<&'static str>, Vec<u8>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut g = GraphBuilder::new()
        .catalog(sdrkit_components::registry().catalog())
        .add("src", Source(from))
        .add("rx", Sink(want, Arc::clone(&seen)))
        .input("src")
        .connect("src", "rx")
        .build()
        .unwrap();
    g.configure_input(Format::iq(8e3, 0.0, 0.0)).unwrap();
    g.process(&[Complex32::default(); 32], &mut Output::default());
    let kinds = g.report().adapters.iter().map(|a| a.kind).collect();
    let v = seen.lock().unwrap().clone();
    (kinds, v)
}

#[test]
fn hard_bits_are_packed_for_an_input_of_bytes() {
    let (kinds, bytes) = run(
        Kind::HardBits,
        Port::bytes("in").bit_order(BitOrder::MsbFirst),
    );
    assert_eq!(kinds, ["bits.pack"]);
    assert_eq!(bytes, [0xB1; 4]);
    let (_, lsb) = run(
        Kind::HardBits,
        Port::bytes("in").bit_order(BitOrder::LsbFirst),
    );
    assert_eq!(lsb, [0x8D; 4]);
}

#[test]
fn bytes_of_a_stated_bit_order_are_unpacked_for_an_input_of_bits() {
    let (kinds, bits) = run(Kind::Bytes, Port::hard_bits("in"));
    assert_eq!(kinds, ["bits.unpack"]);
    assert_eq!(&bits[..8], [1, 0, 1, 1, 0, 0, 0, 1]);
    assert_eq!(bits.len(), 32);
}

/// Bytes that state no bit order.
struct Plain;

impl Component for Plain {
    fn info(&self) -> ComponentInfo {
        let out = Port::bytes("out").rate_from(RateFrom::Fixed(0.0));
        ComponentInfo::processor("test.plain", vec![Port::iq("in")], vec![out])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }
    fn process(&mut self, _: &mut Io) {}
}

#[test]
fn bytes_of_no_stated_bit_order_fail_at_connect_and_packed_bits_unpack() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = GraphBuilder::new()
        .catalog(sdrkit_components::registry().catalog())
        .add("src", Plain)
        .add("rx", Sink(Port::hard_bits("in"), Arc::clone(&seen)))
        .input("src")
        .connect("src", "rx")
        .build();
    assert!(
        matches!(r, Err(GraphError::Connect { .. })),
        "{:?}",
        r.err()
    );
    // The packer states its bit order, so its bytes unpack again.
    let mut g = GraphBuilder::new()
        .catalog(sdrkit_components::registry().catalog())
        .add("src", Source(Kind::HardBits))
        .add("pack", sdrkit_components::bits::Packer::default())
        .add("rx", Sink(Port::hard_bits("in"), Arc::clone(&seen)))
        .input("src")
        .connect("src", "pack")
        .connect("pack", "rx")
        .build()
        .unwrap();
    g.configure_input(Format::iq(8e3, 0.0, 0.0)).unwrap();
    let kinds: Vec<_> = g.report().adapters.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, ["bits.unpack"]);
}

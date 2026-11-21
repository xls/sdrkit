//! The graph components `link.afsk1200` and `link.g3ruh9600`: discriminator audio in, AX.25 frames without the FCS out.

use crate::modem::{Afsk1200, G3ruh9600};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{ConfigError, Results, Values};
use sdrkit_graph::{
    no_values, walk_tags, Component, ComponentInfo, Every, Format, Io, Kind, PacketPool, Port,
    RateFrom, RateRule, Registry, TagKind, TagStep,
};

/// Adds the components of this crate to a registry.
pub fn register(r: &mut Registry) {
    r.add("link.afsk1200", || {
        Box::new(PacketDemod::<Afsk1200>::default())
    });
    r.add("link.g3ruh9600", || {
        Box::new(PacketDemod::<G3ruh9600>::default())
    });
}

/// What the component needs from a demodulator.
pub trait Demod: Send + 'static {
    /// The registered name.
    const NAME: &'static str;
    /// The lowest audio rate in samples per second.
    const MIN_RATE: f64;
    /// The highest audio rate in samples per second.
    const MAX_RATE: f64;
    /// A demodulator at this rate.
    fn make(rate: f64) -> crate::Result<Self>
    where
        Self: Sized;
    /// Feeds audio.
    fn feed(&mut self, audio: &[f32], on_frame: &mut dyn FnMut(crate::modem::Found<'_>));
    /// Clears state.
    fn clear(&mut self);
    /// Frames reported so far.
    fn frames(&self) -> u64;
}

impl Demod for Afsk1200 {
    const NAME: &'static str = "link.afsk1200";
    const MIN_RATE: f64 = Afsk1200::MIN_RATE;
    const MAX_RATE: f64 = Afsk1200::MAX_RATE;
    fn make(rate: f64) -> crate::Result<Self> {
        Afsk1200::new(rate)
    }
    fn feed(&mut self, audio: &[f32], on_frame: &mut dyn FnMut(crate::modem::Found<'_>)) {
        self.process(audio, on_frame)
    }
    fn clear(&mut self) {
        self.reset()
    }
    fn frames(&self) -> u64 {
        self.stats().frames
    }
}

impl Demod for G3ruh9600 {
    const NAME: &'static str = "link.g3ruh9600";
    const MIN_RATE: f64 = G3ruh9600::MIN_RATE;
    const MAX_RATE: f64 = G3ruh9600::MAX_RATE;
    fn make(rate: f64) -> crate::Result<Self> {
        G3ruh9600::new(rate)
    }
    fn feed(&mut self, audio: &[f32], on_frame: &mut dyn FnMut(crate::modem::Found<'_>)) {
        self.process(audio, on_frame)
    }
    fn clear(&mut self) {
        self.reset()
    }
    fn frames(&self) -> u64 {
        self.stats().frames
    }
}

/// A packet demodulator component: `link.afsk1200` or `link.g3ruh9600`.
pub struct PacketDemod<D: Demod> {
    demod: Option<D>,
    format: Format,
    /// The absolute index of the demodulator's sample 0.
    origin: Option<u64>,
    pool: PacketPool,
    every: Every,
    results: Published,
}

impl<D: Demod> Default for PacketDemod<D> {
    fn default() -> Self {
        PacketDemod {
            demod: None,
            format: Format::new(Kind::Real, 0.0),
            origin: None,
            pool: PacketPool::default(),
            every: Every::default(),
            results: Published::new(&[&keys::FRAMES_OK]),
        }
    }
}

impl<D: Demod> PacketDemod<D> {
    fn publish(&mut self) {
        let n = self.demod.as_ref().map_or(0, |d| d.frames());
        self.results.set(&keys::FRAMES_OK, n as i64);
    }
}

impl<D: Demod> Component for PacketDemod<D> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            D::NAME,
            vec![Port::real("in").rate(RateRule::AtLeast(D::MIN_RATE))],
            vec![Port::bytes("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        if rate > D::MAX_RATE {
            return Err(ConfigError::RateTooHigh {
                max: D::MAX_RATE,
                got: rate,
            });
        }
        self.demod = Some(D::make(rate)?);
        self.format = inputs[0].clone();
        self.origin = None;
        self.every = Every::new(rate, 0.25);
        self.publish();
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = io.real(0);
        let Some(demod) = self.demod.as_mut() else {
            return;
        };
        let origin = *self.origin.get_or_insert(first);
        let (format, pool) = (&self.format, &mut self.pool);
        let out = io.bytes_out(0);
        walk_tags(first, x.len(), tags, |step| match step {
            TagStep::Items(a, b) => demod.feed(&x[a..b], &mut |f| {
                let time = format.time_of(origin + f.at);
                out.push(pool.make(f.bytes.len(), time, |d| d.copy_from_slice(f.bytes)));
            }),
            TagStep::Tag(_, t) => {
                if matches!(
                    t.kind,
                    TagKind::Retuned { .. } | TagKind::Discontinuity { .. }
                ) {
                    demod.clear();
                }
            }
        });
        if self.every.tick(x.len()) {
            self.publish();
        }
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

//! The error correction as components: `dvbt.inner` (cells to bits), `dvbt.outer` (bits to transport packets) and `dvbt.fec`
//! (both together, because the outer decoder tells the inner one to search the puncturing phase again).

use crate::inner::{InnerDecoder, InnerStats, ViterbiChoice};
use crate::items::{DataCells, DecodedBits, Link};
use crate::outer::{OuterDecoder, PacketBytes};
use crate::settings::{StreamChoice, STREAM, VITERBI};
use crate::typed::FecFields;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, ConfigError, Results, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Meter, Pool,
    Port, RateFrom, RunContext, SlicePool, Timed,
};
use sdrkit_proto_mpegts::outer::OuterStats;
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// The inner and outer decoders together.
pub struct FecChain {
    /// The inner decoder.
    pub inner: InnerDecoder,
    /// The outer decoder.
    pub outer: OuterDecoder,
    bits: Vec<u8>,
    meter: Meter,
}

impl Default for FecChain {
    fn default() -> Self {
        FecChain::new()
    }
}

impl FecChain {
    /// A new chain.
    pub fn new() -> Self {
        FecChain {
            inner: InnerDecoder::new(),
            outer: OuterDecoder::new(),
            bits: Vec::new(),
            meter: Meter::default(),
        }
    }
    /// Sets the work counters that the sections of the chain count into.
    pub fn set_meter(&mut self, meter: Meter) {
        self.inner.meter = meter.clone();
        self.meter = meter;
    }
    /// One symbol of cells. Packets out.
    pub fn push(&mut self, c: &DataCells, out: &mut Vec<(PacketBytes, bool, f64)>) {
        if c.fresh {
            self.outer.reset();
        }
        self.bits.clear();
        if self.inner.push(c, &mut self.bits) {
            let timer = self.meter.section("rs");
            self.outer.push(&self.bits, c.time, out);
            drop(timer);
            if std::mem::take(&mut self.outer.rephase) {
                self.inner.search_phase_again();
            }
        }
    }
    /// The report of the chain now.
    pub fn report(&self) -> FecReport {
        FecReport {
            inner: self.inner.stats,
            outer: *self.outer.stats(),
            fec_sync: self.outer.fec_sync,
            ber_post: self.outer.ber_post,
        }
    }
    /// Starts afresh.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.outer.reset();
    }
}

/// What the chain measured.
#[derive(Clone, Copy, Debug, Default)]
pub struct FecReport {
    /// Inner decoder measurements.
    pub inner: InnerStats,
    /// Outer counters.
    pub outer: OuterStats,
    /// Packet alignment holds.
    pub fec_sync: bool,
    /// BER after Viterbi.
    pub ber_post: f64,
}

/// Sends the packets of one call as transport stream items, the uncorrectable ones flagged.
fn packets_out(pk: &[(PacketBytes, bool, f64)], ts: &mut TsOut, out: &mut Vec<Timed<TsPacket>>) {
    for (p, failed, t) in pk {
        // A packet of the outer decoder always starts with the sync byte.
        ts.push(out, p, *t, *failed);
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbt.fec`.
    pub enum FecSetting {
        /// The stream of a hierarchical multiplex.
        Stream = STREAM,
        /// The Viterbi decoder.
        Viterbi = VITERBI,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbt.inner`.
    pub enum InnerSetting {
        /// The stream of a hierarchical multiplex.
        Stream = STREAM,
        /// The Viterbi decoder.
        Viterbi = VITERBI,
    }
}

/// `dvbt.fec`: inner and outer decoding.
pub struct DvbtFec {
    link: Arc<Link>,
    chain: FecChain,
    pool: Option<Pool>,
    pk: Vec<(PacketBytes, bool, f64)>,
    ts: TsOut,
    lp: bool,
    choice: ViterbiChoice,
    /// The latest report.
    pub report: FecReport,
    p: Published,
    first_packet: Option<f64>,
}

impl Default for DvbtFec {
    fn default() -> Self {
        DvbtFec::new(Link::new())
    }
}

impl DvbtFec {
    /// A new stage sharing `link`.
    pub fn new(link: Arc<Link>) -> Self {
        DvbtFec {
            link,
            chain: FecChain::new(),
            pool: None,
            pk: Vec::new(),
            ts: TsOut::default(),
            lp: false,
            choice: ViterbiChoice::Auto,
            report: FecReport::default(),
            p: Published::new(FecFields::SHAPE.fields),
            first_packet: None,
        }
    }

    fn build(&mut self) {
        let mut c = FecChain::new();
        c.inner.lp = self.lp;
        c.inner.choice = self.choice;
        c.inner.pool = self.pool.clone();
        self.chain = c;
    }

    /// The signal time of the first packet.
    pub fn first_packet(&self) -> Option<f64> {
        self.first_packet
    }
}

impl DvbtFec {
    /// Takes what is decoded into the output, and updates the link and the results.
    fn emit(&mut self, io: &mut Io) {
        self.report = self.chain.report();
        if !self.pk.is_empty() && self.first_packet.is_none() {
            self.first_packet = self.pk.iter().find(|p| !p.1).map(|p| p.2);
        }
        let locked = self.link.tps_locked.load(Ordering::Relaxed);
        self.link
            .fec_sync
            .store(self.report.fec_sync && locked, Ordering::Relaxed);
        if locked && self.report.fec_sync && self.pk.iter().any(|p| !p.1) {
            // Only "locked" becomes "streaming": a state that an earlier component lowered since stays.
            let (o, r) = (Ordering::AcqRel, Ordering::Relaxed);
            let _ = self.link.state.compare_exchange(2, 3, o, r);
            let _ = self.link.detect.compare_exchange(2, 3, o, r);
        }
        packets_out(&self.pk, &mut self.ts, io.items_out::<TsPacket>(0));
        if let Ok(mut t) = self.link.telemetry.lock() {
            t.fec = self.report;
            t.first_packet = self.first_packet;
        }
        let o = &self.report.outer;
        self.p.put(&FecFields {
            frames_ok: o.delivered as i64,
            frames_bad: o.failed as i64,
        });
    }
}

impl Component for DvbtFec {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.fec",
            vec![Port::typed::<DataCells>("in")],
            vec![stream::port("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FecSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.build();
        self.first_packet = None;
        Ok(vec![Format::new(stream::kind(), 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.pool = Some(ctx.pool.clone());
        self.chain.inner.pool = self.pool.clone();
    }
    fn process(&mut self, io: &mut Io) {
        self.pk.clear();
        self.chain.set_meter(io.meter());
        for c in io.items::<DataCells>(0) {
            self.chain.push(&c.value, &mut self.pk);
        }
        self.emit(io);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FecSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FecSetting::Stream => {
                self.lp = STREAM.read(id, v)? == StreamChoice::Lp;
                self.chain.inner.lp = self.lp;
                self.chain.reset();
            }
            FecSetting::Viterbi => {
                self.choice = VITERBI.read(id, v)?;
                self.chain.inner.choice = self.choice;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let stream = if self.lp {
            StreamChoice::Lp
        } else {
            StreamChoice::Hp
        };
        values_of([
            (FecSetting::Stream, Value::Choice(stream.index())),
            (FecSetting::Viterbi, Value::Choice(self.choice.index())),
        ])
    }
    /// The counts are of the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.p.results_mut().fields_mut().clear();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

/// `dvbt.inner`: cells to decoded bits.
pub struct DvbtInner {
    dec: InnerDecoder,
    bits: Vec<u8>,
    pool: SlicePool<u8>,
}

impl Default for DvbtInner {
    fn default() -> Self {
        DvbtInner::new()
    }
}

impl DvbtInner {
    /// A new stage.
    pub fn new() -> Self {
        DvbtInner {
            dec: InnerDecoder::new(),
            bits: Vec::new(),
            pool: SlicePool::default(),
        }
    }
}

impl Component for DvbtInner {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.inner",
            vec![Port::typed::<DataCells>("in")],
            vec![Port::typed::<DecodedBits>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(InnerSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec.reset();
        Ok(vec![Format::new(Kind::typed::<DecodedBits>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut out = std::mem::take(io.items_out::<DecodedBits>(0));
        for c in io.items::<DataCells>(0) {
            self.bits.clear();
            if self.dec.push(&c.value, &mut self.bits) {
                let b = &self.bits;
                let bits = self.pool.exact(b.len(), |v| v.copy_from_slice(b));
                out.push(Timed {
                    time: c.time,
                    value: DecodedBits {
                        bits,
                        fresh: c.value.fresh,
                        time: c.time,
                        duration: self.dec.block_seconds(),
                    },
                });
            }
        }
        *io.items_out::<DecodedBits>(0) = out;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match InnerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InnerSetting::Stream => {
                self.dec.lp = STREAM.read(id, v)? == StreamChoice::Lp;
                self.dec.reset();
            }
            InnerSetting::Viterbi => self.dec.choice = VITERBI.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let stream = if self.dec.lp {
            StreamChoice::Lp
        } else {
            StreamChoice::Hp
        };
        values_of([
            (InnerSetting::Stream, Value::Choice(stream.index())),
            (
                InnerSetting::Viterbi,
                Value::Choice(self.dec.choice.index()),
            ),
        ])
    }
}

/// `dvbt.outer`: decoded bits to transport packets.
#[derive(Default)]
pub struct DvbtOuter {
    dec: OuterDecoder,
    pk: Vec<(PacketBytes, bool, f64)>,
    ts: TsOut,
}

impl Component for DvbtOuter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.outer",
            vec![Port::typed::<DecodedBits>("in")],
            vec![stream::port("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec.reset_counters();
        Ok(vec![Format::new(stream::kind(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        self.pk.clear();
        for b in io.items::<DecodedBits>(0) {
            if b.value.fresh {
                self.dec.reset();
            }
            self.dec.push(&b.value.bits, b.time, &mut self.pk);
        }
        packets_out(&self.pk, &mut self.ts, io.items_out::<TsPacket>(0));
    }
    fn values(&self) -> Values {
        no_values()
    }
}

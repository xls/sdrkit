//! The ATSC 1.0 receiver as components: `atsc.pilot_find`, `atsc.front`, `atsc.pilot_loop`, `atsc.symbol_clock`,
//! `atsc.field_sync`, `atsc.equalise` and `atsc.fec`. `atsc.report` (in the stack module) publishes the results.

use super::fec::{FecCounters, FieldDecoder};
use super::rx::*;
use crate::band::{AvgSpectrum, BandFft};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};

macro_rules! item {
    ($t:ty, $k:expr) => {
        impl Item for $t {
            const KIND: &'static str = $k;
        }
    };
}
item!(PilotFound, "atsc.pilot");
item!(PilotLoopState, "atsc.pilot_loop");
item!(ClockState, "atsc.clock");
item!(FieldSyncState, "atsc.field_state");
item!(Segment, "atsc.segment");
item!(Field, "atsc.field");
item!(SoftField, "atsc.soft_field");
item!(FecReport, "atsc.fec_report");

fn typed<T: Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

/// An output port of typed items, which have no sample rate.
fn items<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// `atsc.pilot_find`: averages the spectrum of the whole band, finds the pilot (as it is or mirrored), and mixes the channel so
/// that its pilot is at its nominal frequency. Searches again every 0.5 s while no pilot is found.
pub struct PilotFind {
    rate: f64,
    spec: AvgSpectrum,
    held: Vec<Complex32>,
    found: Option<PilotFound>,
    nco: Option<Nco>,
    since: usize,
}

impl Default for PilotFind {
    fn default() -> Self {
        PilotFind {
            rate: 8e6,
            spec: AvgSpectrum::new(BandFft::N4096),
            held: Vec::new(),
            found: None,
            nco: None,
            since: 0,
        }
    }
}

impl PilotFind {
    const FRAMES: usize = 32;
}

impl Component for PilotFind {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.pilot_find",
            vec![Port::iq("in")],
            vec![
                Port::iq("out").same_format_as(0),
                items::<PilotFound>("state"),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        let n = if self.rate > 12e6 {
            BandFft::N8192
        } else {
            BandFft::N4096
        };
        self.spec = AvgSpectrum::new(n);
        self.held.clear();
        self.found = None;
        self.nco = None;
        self.since = 0;
        Ok(vec![inputs[0].clone(), typed::<PilotFound>()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let n = self.spec.len();
        if self.found.is_none() {
            self.held.extend_from_slice(x);
            if self.held.len() >= n * Self::FRAMES {
                self.spec.clear();
                self.spec.add_all(&self.held[..n * Self::FRAMES]);
                let f = find_pilot(&self.spec.mean(), self.rate);
                let held = std::mem::take(&mut self.held);
                let time = io.input(0).first() as f64 / self.rate;
                match f {
                    Some(f) => {
                        self.found = Some(f);
                        self.nco = Some(Nco::new(self.rate, f.shift_hz));
                        io.items_out::<PilotFound>(1).push(Timed { time, value: f });
                    }
                    None => self.since += held.len(),
                }
                // Everything held goes out (shifted when found, as it is otherwise).
                self.emit(&held, io);
                if self.found.is_none() && self.since < (0.5 * self.rate) as usize {
                    // Keep looking on the next samples.
                }
                if self.found.is_none() {
                    self.since = 0;
                }
            }
            return;
        }
        self.emit(x, io);
    }
}

impl PilotFind {
    fn emit(&mut self, x: &[Complex32], io: &mut Io) {
        let out = io.iq_out(0);
        let start = out.len();
        match (&self.found, &mut self.nco) {
            (Some(f), Some(nco)) => {
                if f.mirrored {
                    out.extend(x.iter().map(|s| s.conj()));
                } else {
                    out.extend_from_slice(x);
                }
                nco.mix_in_place(&mut out[start..]);
            }
            _ => out.extend_from_slice(x),
        }
    }
}

/// `atsc.front`: the matched VSB filter and resampling to the symbol rate, 10.762238 MHz.
#[derive(Default)]
pub struct FrontComp {
    front: Option<Front>,
}

impl Component for FrontComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.front",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(FSYM))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.front = Some(Front::new(inputs[0].rate)?);
        Ok(vec![Format {
            rate: FSYM,
            bandwidth_hz: 6e6,
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if let Some(f) = &mut self.front {
            let out = io.iq_out(0);
            f.process(x, out);
        }
    }
}

/// `atsc.pilot_loop`: the carrier loop on the pilot, DC and gain normalisation. Outputs the channel, the pilot's place at 0 Hz.
pub struct PilotLoopComp {
    lp: PilotLoop,
    report_every: u64,
    since: u64,
}

impl Default for PilotLoopComp {
    fn default() -> Self {
        PilotLoopComp {
            lp: PilotLoop::new(1.0),
            report_every: (0.05 * FSYM) as u64,
            since: 0,
        }
    }
}

impl Component for PilotLoopComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.pilot_loop",
            vec![Port::iq("in")],
            vec![
                Port::iq("out").same_format_as(0),
                items::<PilotLoopState>("state"),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.lp = PilotLoop::new(1.0);
        Ok(vec![inputs[0].clone(), typed::<PilotLoopState>()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let t = io.input(0).first() as f64 / FSYM;
        self.lp.process(x, io.iq_out(0));
        self.since += x.len() as u64;
        if self.since >= self.report_every {
            self.since = 0;
            io.items_out::<PilotLoopState>(1).push(Timed {
                time: t,
                value: self.lp.state,
            });
        }
    }
}

/// `atsc.symbol_clock`: segment sync, timing loop and interpolation. Outputs segments of 832 symbols.
#[derive(Default)]
pub struct ClockComp {
    clk: SymbolClock,
    segs: Vec<Segment>,
    n: u64,
}

impl Component for ClockComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.symbol_clock",
            vec![Port::iq("in")],
            vec![items::<Segment>("out"), items::<ClockState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.clk = SymbolClock::new();
        Ok(vec![typed::<Segment>(), typed::<ClockState>()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.segs.clear();
        self.clk.process(x, FSYM, &mut self.segs);
        let st = self.clk.state;
        let out = io.items_out::<Segment>(0);
        for s in self.segs.drain(..) {
            out.push(Timed {
                time: s.time,
                value: s,
            });
        }
        self.n += x.len() as u64;
        if self.n >= (0.05 * FSYM) as u64 {
            self.n = 0;
            let t = io.input(0).first() as f64 / FSYM;
            io.items_out::<ClockState>(1)
                .push(Timed { time: t, value: st });
        }
    }
}

/// `atsc.field_sync`: matches the field sync and frames fields.
#[derive(Default)]
pub struct FieldComp {
    fr: FieldFramer,
}

impl Component for FieldComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.field_sync",
            vec![Port::typed::<Segment>("in")],
            vec![items::<Field>("out"), items::<FieldSyncState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fr = FieldFramer::default();
        Ok(vec![typed::<Field>(), typed::<FieldSyncState>()])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<Segment>(0) {
            let was = self.fr.state;
            if let Some(f) = self.fr.push(&s.value) {
                io.items_out::<Field>(0).push(Timed {
                    time: f.time,
                    value: f,
                });
                io.items_out::<FieldSyncState>(1).push(Timed {
                    time: s.time,
                    value: self.fr.state,
                });
            } else if was.locked != self.fr.state.locked {
                io.items_out::<FieldSyncState>(1).push(Timed {
                    time: s.time,
                    value: self.fr.state,
                });
            }
        }
    }
}

/// `atsc.equalise`: the per-field equaliser. Whole fields come in and equalised fields go out, so the stack may give it a
/// stage of its own.
#[derive(Default)]
pub struct EqComp {
    eq: Equaliser,
}

impl EqComp {
    fn equalise(&mut self, field: &Field) -> SoftField {
        let sf = self.eq.equalise(field);
        // A field this clean needs no new solution next time (re-solved every fourth field, at once on trouble).
        self.eq.report(sf.snr_db >= 15.0);
        sf
    }
}

impl Component for EqComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.equalise",
            vec![Port::typed::<Field>("in")],
            vec![items::<SoftField>("out")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.eq = Equaliser::new();
        Ok(vec![typed::<SoftField>()])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<Field>(0) {
            let sf = self.equalise(&f.value);
            io.items_out::<SoftField>(0).push(Timed {
                time: f.time,
                value: sf,
            });
        }
    }
}

/// What `atsc.fec` reports after each field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FecReport {
    /// Counters.
    pub counters: FecCounters,
    /// SNR on the known symbols of the field, dB.
    pub snr_db: f64,
    /// SNR of the data against the decoded trellis path, dB.
    pub data_snr_db: f64,
    /// The field was decoded (its estimate was usable).
    pub decoded: bool,
    /// Field parity (1 or 2).
    pub parity: u8,
}

/// `atsc.fec`: trellis decoding of a field with the next field as its look-ahead, de-interleaving, RS, de-randomising. It
/// outputs transport packets. A field comes out when the next one arrives.
#[derive(Default)]
pub struct FecComp {
    dec: FieldDecoder,
    pk: Vec<[u8; TS]>,
    ts: TsOut,
    held: Option<(f64, f64, u8)>,
}

impl Component for FecComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc.fec",
            vec![Port::typed::<SoftField>("in")],
            vec![
                stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                items::<FecReport>("state"),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec = FieldDecoder::new();
        self.held = None;
        Ok(vec![Format::new(stream::kind(), 0.0), typed::<FecReport>()])
    }
    /// The counts are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.dec = FieldDecoder::new();
        self.held = None;
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<SoftField>(0) {
            let sf = &f.value;
            let mut rep = FecReport {
                snr_db: sf.snr_db,
                parity: if sf.second { 2 } else { 1 },
                ..Default::default()
            };
            if sf.gap {
                self.dec.reset();
                self.held = None;
            }
            if sf.usable {
                self.pk.clear();
                if let (Some(err), Some((time, _, _))) =
                    (self.dec.push_field(&sf.levels, &mut self.pk), self.held)
                {
                    rep.decoded = true;
                    rep.data_snr_db = 10.0 * (21.0 / err.max(1e-9)).log10();
                    // One packet per data segment, 77.4 µs each.
                    let seg_t = super::fec::SEG as f64 / FSYM;
                    let out = io.items_out::<TsPacket>(0);
                    for (i, p) in self.pk.iter().enumerate() {
                        // The transport error indicator (ISO/IEC 13818-1 clause 2.4.3.2).
                        let bad = p[1] & 0x80 != 0;
                        self.ts.push(out, p, time + (i + 1) as f64 * seg_t, bad);
                    }
                }
                self.held = Some((f.time, sf.snr_db, rep.parity));
            } else {
                self.dec.reset();
                self.held = None;
            }
            rep.counters = self.dec.counters;
            io.items_out::<FecReport>(1).push(Timed {
                time: f.time,
                value: rep,
            });
        }
    }
}

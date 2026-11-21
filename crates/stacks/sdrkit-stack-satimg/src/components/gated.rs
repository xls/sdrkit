//! The PSK demodulator behind a no-signal gate (`satimg.gated_demod`), for links of tens of megasymbols a second. While no
//! carrier is found the [`Gate`] looks at one piece of samples and lets the next ones go by unread; an open gate starts the
//! demodulator at the carrier of its line. A demodulator whose lock metric stays low after its first symbols closes the
//! gate again, and so does a carrier restart from the display.

use super::link::{control_port, item_format, item_port, DemodReport};
use super::{controls, Control};
use crate::gate::{Gate, GateConfig, GateOutcome};
use crate::psk::{LinkParams, Modulation, PskDemod};
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, Value, Values};
use sdrkit_graph::{no_values, Applied, Component, ComponentInfo, Format, Io, Item, Port, Timed};

/// Symbols after an open gate at which the lock metric is checked.
pub const CONFIRM_SYMBOLS: u64 = 20_000;
/// The least lock metric at that check.
pub const CONFIRM_METRIC: f32 = 0.1;

/// The gate rules of a stack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateRules {
    /// Samples of a piece.
    pub piece: usize,
    /// The least height of the line over the mean that opens the gate.
    pub open_ratio: f64,
    /// The piece goes through the matched filter first.
    pub matched: bool,
    /// Pieces let go by after a piece of noise.
    pub skip: u32,
    /// The largest carrier offset searched, Hz.
    pub span_hz: f64,
}

/// The report of the gate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GateReport {
    /// The demodulator runs.
    pub open: bool,
    /// Pieces looked at.
    pub looks: u64,
    /// Input samples let go by unread.
    pub skipped: u64,
    /// Input samples taken.
    pub samples: u64,
    /// The height of the line of the last look.
    pub last_ratio: f64,
    /// Times the gate opened.
    pub opens: u64,
    /// Seconds of signal.
    pub time_s: f64,
}

impl Item for GateReport {
    const KIND: &'static str = "satimg.gate_report";
}

/// The component. See the [module documentation](self).
pub struct GatedDemodComponent {
    link: LinkParams,
    rules: GateRules,
    gate: Option<Gate>,
    demod: Option<PskDemod>,
    open: bool,
    confirmed: bool,
    rate: f64,
    samples: u64,
    opens: u64,
    first: Vec<Complex32>,
    soft: Vec<i8>,
}

impl GatedDemodComponent {
    /// A demodulator for `link` behind a gate with `rules`.
    pub fn new(link: LinkParams, rules: GateRules) -> Self {
        GatedDemodComponent {
            link,
            rules,
            gate: None,
            demod: None,
            open: false,
            confirmed: false,
            rate: 0.0,
            samples: 0,
            opens: 0,
            first: Vec::new(),
            soft: Vec::new(),
        }
    }

    fn close(&mut self) {
        self.open = false;
        if let Some(g) = &mut self.gate {
            g.reset();
        }
        if let Some(d) = &mut self.demod {
            d.state.tracking = false;
            d.state.locked = false;
        }
    }
}

impl Component for GatedDemodComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.gated_demod",
            vec![Port::iq("in"), control_port()],
            vec![
                Port::llr("soft"),
                item_port::<DemodReport>("report"),
                item_port::<GateReport>("gate"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.demod = Some(PskDemod::short(f.rate, &self.link)?);
        let power = if self.link.modulation == Modulation::Bpsk { 2 } else { 4 };
        self.gate = Some(Gate::new(GateConfig {
            rate: f.rate,
            symbol_rate: self.link.symbol_rate,
            piece: self.rules.piece,
            power,
            oqpsk: self.link.modulation == Modulation::Oqpsk,
            matched: self.rules.matched.then_some(self.link.roll_off),
            open_ratio: self.rules.open_ratio,
            skip: self.rules.skip,
            span_hz: self.rules.span_hz,
        }));
        self.open = false;
        self.samples = 0;
        let bits_rate = self.link.symbol_rate * self.link.modulation.bits_per_symbol() as f64;
        let mut o = Format::llr(bits_rate, None);
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o, item_format::<DemodReport>(), item_format::<GateReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let restart = controls(io, 1).any(|c| matches!(c, Control::RestartCarrier | Control::RejectCarrier));
        if restart {
            self.close();
        }
        let x = io.iq(0);
        self.samples += x.len() as u64;
        let (Some(gate), Some(demod)) = (&mut self.gate, &mut self.demod) else {
            return;
        };
        self.soft.clear();
        let mut i = 0;
        while i < x.len() {
            if self.open {
                demod.process(&x[i..], &mut self.soft);
                break;
            }
            let (used, out) = gate.push(&x[i..]);
            i += used;
            if let Some(GateOutcome::Open { carrier_hz, .. }) = out {
                self.open = true;
                self.confirmed = false;
                self.opens += 1;
                demod.start_at(carrier_hz);
                // The piece that opened the gate is the start of the signal: the demodulator takes it first.
                self.first.clear();
                self.first.extend_from_slice(gate.piece());
                demod.process(&self.first, &mut self.soft);
            }
        }
        if self.open && !self.confirmed && demod.state.symbols >= CONFIRM_SYMBOLS {
            if demod.state.lock_metric < CONFIRM_METRIC {
                self.open = false;
                gate.reset();
                demod.state.tracking = false;
                demod.state.locked = false;
            } else {
                self.confirmed = true;
            }
        }
        io.llrs_out(0).extend_from_slice(&self.soft);
        if io.tick() {
            let t = self.samples as f64 / self.rate.max(1.0);
            let r = DemodReport {
                state: demod.report().clone(),
                symbol_rate_nominal: self.link.symbol_rate,
                constellation: demod.constellation().collect(),
                time_s: t,
            };
            io.items_out::<DemodReport>(1).push(Timed { time: t, value: r });
            let g = GateReport {
                open: self.open,
                looks: gate.looks,
                skipped: gate.skipped,
                samples: self.samples,
                last_ratio: gate.last_ratio,
                opens: self.opens,
                time_s: t,
            };
            io.items_out::<GateReport>(2).push(Timed { time: t, value: g });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

//! The graph side of the LoRa physical layer: the item kinds `lora.frame`, `lora.symbols` and `lora.state`, and the
//! component `lora.bank`, which takes complex samples and returns frames, their symbols and the state of the bank.

use crate::bank::{Bank, BankPlan, DetectorState};
use crate::error::LoraError;
use crate::rx::{LoraFrame, LoraSymbols};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{EventShape, Family, Fields, Hertz, StackShape};
use sdrkit_core::publish::keys;
use sdrkit_core::{ConfigError, EventDef, Record, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Registry, Timed,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The fields of a decoded frame that leaves the graph (`lora.frame`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraFrameFields {
    /// The payload bytes.
    #[field(label = "Payload")]
    pub payload: Arc<[u8]>,
    /// The spreading factor.
    #[field(label = "Spreading factor")]
    pub sf: i64,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw_hz: Hertz,
    /// The coding rate, 5 to 8 (4/5 to 4/8).
    #[field(label = "Coding rate")]
    pub cr: i64,
    /// The frame carries a CRC.
    #[field(label = "CRC present")]
    pub crc_present: bool,
    /// The CRC is right.
    #[field(label = "CRC good")]
    pub crc_ok: bool,
    /// The frame has an implicit header.
    #[field(label = "Implicit header")]
    pub implicit: bool,
    /// The sync word heard.
    #[field(label = "Sync word")]
    pub sync: i64,
    /// The IQ was inverted.
    #[field(label = "Inverted IQ")]
    pub inverted: bool,
    /// The carrier offset at the start of the frame.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The clock offset.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The codewords corrected.
    #[field(label = "Codewords corrected")]
    pub corrected: i64,
    /// The number the plan provided the detector.
    #[field(label = "Detector tag")]
    pub tag: i64,
    /// The centre of the channel.
    #[field(label = "Channel", unit = "Hz")]
    pub channel_hz: Hertz,
}

/// The event of a decoded frame that leaves the graph, with the fields of [`LoraFrameFields`].
pub static FRAME_EVENT: EventDef = EventDef::new(
    "lora.frame",
    "LoRa frame",
    <LoraFrameFields as Fields>::SHAPE.fields,
);

/// The event type `lora.frame` with its shape, for the description of a stack.
pub fn frame() -> EventShape {
    EventShape::of::<LoraFrameFields>(&FRAME_EVENT)
}

/// The interface of the frame event of this crate, which stacks of several families return. A family whose stacks
/// list it names this interface with `Family::uses`.
pub fn interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("lora_phy", 1, 0).stack(StackShape::new("lora_phy").event(frame()))
    })
}

impl Item for LoraFrame {
    const KIND: &'static str = "lora.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(&FRAME_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        LoraFrameFields {
            payload: Arc::from(&self.payload[..]),
            sf: i64::from(self.sf),
            bw_hz: Hertz(self.bw),
            cr: i64::from(self.cr),
            crc_present: self.crc_present,
            crc_ok: self.crc_ok,
            implicit: self.implicit,
            sync: i64::from(self.sync),
            inverted: self.inverted,
            offset_hz: Hertz(self.cfo_hz),
            clock_ppm: self.ppm,
            snr_db: self.snr_db,
            level_dbfs: self.level_db,
            corrected: i64::from(self.corrected),
            tag: i64::from(self.tag),
            channel_hz: Hertz(self.channel_hz),
        }
        .write(r);
    }
}

impl Item for LoraSymbols {
    const KIND: &'static str = "lora.symbols";
}

const RESET: Command =
    Command::new("reset", "Reset").help("Forgets the signal and makes the bank again.");

sdrkit_core::commands! {
    /// The commands of [`LoraBank`].
    pub enum LoraBankCommand {
        /// Forgets the signal.
        Reset = RESET,
    }
}

/// The state of a bank at each tick and at each busy change (graph item `lora.state`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BankState {
    /// The signal time, seconds. For a busy change, the time of the change (never before the item before it).
    pub time: f64,
    /// A detector is inside a frame.
    pub busy: bool,
    /// Every detector of the plan.
    pub detectors: Vec<DetectorState>,
    /// The channels in band.
    pub in_band: usize,
    /// The detectors that run.
    pub demodulators: usize,
    /// The input rate, Hz.
    pub rate: f64,
    /// The centre of the input, Hz.
    pub centre_hz: f64,
    /// The mean power of the input over the last tick period, dBFS (-200 before any input). A busy change repeats the value of
    /// the last tick.
    pub level_db: f64,
    /// The samples seen (for a busy change, the input sample of the change).
    pub samples: u64,
}

impl Item for BankState {
    const KIND: &'static str = "lora.state";
}

/// The `lora.bank` component: complex samples in, and out the frames, their symbols, and the state of the bank. A state item
/// leaves at each tick of the graph clock and at each change of a detector's busy state. The bank is made again at the next
/// piece of samples after a new plan, a retune or the `reset` command.
#[derive(Default)]
pub struct LoraBank {
    plan: BankPlan,
    rate: f64,
    centre: f64,
    bank: Option<Bank>,
    rebuild: bool,
    found: Vec<(LoraFrame, LoraSymbols)>,
    lev: f64,
    lev_n: u64,
    level: f64,
    error: Option<LoraError>,
    /// The busy state of each detector after the last change given.
    det_busy: Vec<bool>,
    /// The time of the last state item given, seconds.
    last_time: f64,
}

impl LoraBank {
    /// A bank with this plan.
    pub fn new(plan: BankPlan) -> Self {
        LoraBank {
            plan,
            rebuild: true,
            level: -200.0,
            ..Default::default()
        }
    }

    /// Replaces the plan. If it differs from the plan in use, the bank is made again at the next piece of samples.
    pub fn set_plan(&mut self, plan: BankPlan) {
        if plan != self.plan {
            self.plan = plan;
            self.rebuild = true;
        }
    }

    /// The reason why the last plan did not build. `None` if it built.
    pub fn error(&self) -> Option<LoraError> {
        self.error
    }
}

impl Component for LoraBank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "lora.bank",
            vec![Port::iq("in")],
            vec![
                Port::typed::<LoraFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<LoraSymbols>("symbols").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<BankState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(f) = inputs.first() {
            self.rate = f.rate;
            self.centre = f.centre_hz;
        }
        self.rebuild = true;
        Ok(vec![
            Format::new(Kind::typed::<LoraFrame>(), 0.0),
            Format::new(Kind::typed::<LoraSymbols>(), 0.0),
            Format::new(Kind::typed::<BankState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { centre_hz, .. } = t.kind {
                self.centre = centre_hz;
                self.rebuild = true;
            }
        }
        if self.rebuild && self.rate > 0.0 {
            match Bank::new(self.rate, self.centre, self.plan.clone(), first) {
                Ok(b) => {
                    self.bank = Some(b);
                    self.error = None;
                    self.det_busy.clear();
                }
                Err(e) => {
                    self.bank = None;
                    self.error = Some(e);
                }
            }
            self.rebuild = false;
        }
        let iq = io.iq(0);
        for s in iq {
            self.lev += s.norm_sqr() as f64;
        }
        self.lev_n += iq.len() as u64;
        self.found.clear();
        if let Some(b) = &mut self.bank {
            b.process(iq, &mut self.found);
        }
        let rate = self.rate.max(1.0);
        let end = first + iq.len() as u64;
        if let Some(b) = &self.bank {
            if !b.busy_changes().is_empty() {
                let states = b.states();
                self.det_busy.resize(states.len(), false);
                for ch in b.busy_changes() {
                    if let Some(x) = self.det_busy.get_mut(ch.detector) {
                        *x = ch.busy;
                    }
                    let at = ch.at.min(end);
                    let time = (at as f64 / rate).max(self.last_time);
                    let mut detectors = states.clone();
                    for (d, &busy) in detectors.iter_mut().zip(&self.det_busy) {
                        d.busy = busy;
                    }
                    let st = BankState {
                        time,
                        busy: self.det_busy.iter().any(|&x| x),
                        detectors,
                        in_band: b.in_band(),
                        demodulators: b.demodulators(),
                        rate: self.rate,
                        centre_hz: self.centre,
                        level_db: self.level,
                        samples: at,
                    };
                    self.last_time = time;
                    io.items_out::<BankState>(2).push(Timed { time, value: st });
                }
            }
        }
        for (f, s) in self.found.drain(..) {
            let time = f.start as f64 / rate;
            io.items_out::<LoraSymbols>(1)
                .push(Timed { time, value: s });
            io.items_out::<LoraFrame>(0).push(Timed { time, value: f });
        }
        if io.tick() {
            if self.lev_n > 0 {
                self.level = 10.0 * (self.lev / self.lev_n as f64).max(1e-20).log10();
            }
            self.lev = 0.0;
            self.lev_n = 0;
            let mut st = BankState {
                time: (end as f64 / rate).max(self.last_time),
                level_db: self.level,
                rate: self.rate,
                centre_hz: self.centre,
                samples: end,
                ..Default::default()
            };
            if let Some(b) = &self.bank {
                st.busy = b.busy();
                st.detectors = b.states();
                st.in_band = b.in_band();
                st.demodulators = b.demodulators();
            }
            self.last_time = st.time;
            io.items_out::<BankState>(2).push(Timed {
                time: st.time,
                value: st,
            });
        }
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn commands(&self) -> &Commands {
        LoraBankCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match LoraBankCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            LoraBankCommand::Reset => self.rebuild = true,
        }
        Ok(None)
    }
}

/// Adds the `lora.*` components of this crate to `r`.
pub fn register(r: &mut Registry) {
    r.add("lora.bank", || Box::new(LoraBank::new(BankPlan::default())));
}

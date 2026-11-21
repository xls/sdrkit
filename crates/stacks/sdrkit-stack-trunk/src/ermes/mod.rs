//! ERMES paging (`ermes`): 4-level FSK at 3125 symbols/s on FM, one batch at a time: the network identity, the initial
//! addresses and the numeric and text messages. Receive only. The stack holds no channel list; the session gives the frequency.
//!
//! Standard: ETS 300 133-4 (air interface). The `note` field names what no public document available here confirms.

pub mod code;
pub mod gen;
pub mod rx;

use crate::front::CarrierState;
use rx::{ErmesBatch, ErmesHeard, ErmesModem};
use crate::typed::{
    ErmesBatchEvent, ErmesEntryRow, ErmesFields, ErmesMessageEvent, ERMES_BATCH, ERMES_MESSAGE,
    ERMES_ROWS,
};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, EventDef, GenConfig, Generator, LockState, Receiver, Record, ResultsDecl, Schema,
    StackConfig, StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Outbox,
    Port, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "ermes";
/// The least input rate, Hz: what the paging front end needs.
pub const MIN_RATE: f64 = 96e3;
/// The most rows kept.
pub const ROW_CAP: usize = 400;
/// A good batch within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 10.0;
/// What no public document available here confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the preamble and sync word values, the SSI type 0 fields, header word 2 (and its bit 9), the numeric codes 10 to 15, the additional-information content, and the cycle and subsequence structure; one batch is read at a time and the time slots of the paging areas are not followed.";

/// What a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowKind {
    /// The initial addresses of a batch.
    Addresses,
    /// A message.
    Message,
}

/// One row of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErmesRow {
    /// Signal time, s.
    pub t: f64,
    /// What it is.
    pub kind: RowKind,
    /// The batch letter.
    pub batch: char,
    /// The local address of a message (22 bits).
    pub address: Option<u32>,
    /// The category of a message.
    pub category: Option<String>,
    /// The addresses (decimal) or the message text.
    pub text: String,
}

impl Shift for ErmesRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ErmesCounters {
    /// Good batches.
    pub batches: u64,
    /// Batches dropped.
    pub bad_batches: u64,
    /// Initial addresses.
    pub addresses: u64,
    /// Messages.
    pub messages: u64,
    /// Code words repaired.
    pub repaired: u64,
    /// Preamble and sync found.
    pub syncs: u64,
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErmesHeld {
    /// The rows, newest last.
    pub rows: LogRing<ErmesRow>,
    /// The counters.
    pub counters: ErmesCounters,
    /// The network of the last good batch.
    pub network: Option<String>,
}

impl Default for ErmesHeld {
    fn default() -> Self {
        ErmesHeld {
            rows: LogRing::new(ROW_CAP),
            counters: ErmesCounters::default(),
            network: None,
        }
    }
}

impl Shift for ErmesHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.shift_times(by_s);
    }
}

impl Tables for ErmesHeld {
    const FORMAT: StateFormat = StateFormat::new("trunk.ermes", 1);
    fn clear(&mut self) {
        self.rows.clear();
        self.counters = ErmesCounters::default();
        self.network = None;
    }
    fn adopt(&mut self, saved: Self) {
        self.rows.adopt(saved.rows);
        self.counters = saved.counters;
        self.network = saved.network;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for ErmesRow {
    type Row = ErmesEntryRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ErmesEntryRow {
            time_s: self.t,
            kind: match self.kind {
                RowKind::Addresses => "Addresses",
                RowKind::Message => "Message",
            },
            batch: self.batch.encode_utf8(&mut [0; 4]),
            address: self.address.map(i64::from),
            category: self.category.as_deref(),
            text: &self.text,
        });
    }
}

/// A good batch as an event item: its letter, network and initial addresses.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchItem {
    /// The batch letter.
    pub batch: char,
    /// The network.
    pub network: String,
    /// The initial addresses, decimal.
    pub addresses: String,
}

impl Item for BatchItem {
    const KIND: &'static str = "ermes.batch";
    fn event_def() -> Option<&'static EventDef> {
        Some(ERMES_BATCH.def())
    }
    fn write_fields(&self, f: &mut Record) {
        ErmesBatchEvent {
            batch: self.batch.encode_utf8(&mut [0; 4]),
            network: &self.network,
            text: &self.addresses,
            family: Family::Pager,
        }
        .write(f);
    }
}

/// A message as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageItem(pub ErmesRow);

impl Item for MessageItem {
    const KIND: &'static str = "ermes.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(ERMES_MESSAGE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let r = &self.0;
        let target = r.address.map(|a| a.to_string());
        ErmesMessageEvent {
            target: target.as_deref(),
            category: r.category.as_deref(),
            text: &r.text,
            family: Family::Pager,
        }
        .write(f);
    }
}

/// The `ermes.batches` decoder: the rows, the counters, the events and the saved state.
#[derive(Default)]
pub struct ErmesBatches {
    held: ErmesHeld,
    now: f64,
    started: bool,
    last_good: Option<f64>,
    carrier: Option<CarrierState>,
    batches_out: Vec<Timed<BatchItem>>,
    messages_out: Vec<Timed<MessageItem>>,
    pending_log: Vec<String>,
    cleared: bool,
    dirty: bool,
    summary: String,
}

impl ErmesBatches {
    /// The rows and counters.
    pub fn held(&self) -> &ErmesHeld {
        &self.held
    }

    fn batch(&mut self, t: f64, b: &ErmesBatch) {
        let letter = b.si.batch_letter();
        let network = b.si.text();
        let c = &mut self.held.counters;
        c.batches += 1;
        c.addresses += b.addresses.len() as u64;
        c.messages += b.messages.len() as u64;
        c.repaired += u64::from(b.repaired);
        self.last_good = Some(t);
        let addresses = b
            .addresses
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        self.pending_log.push(format!(
            "ermes: batch {letter}, {network}, {} addresses, {} messages",
            b.addresses.len(),
            b.messages.len()
        ));
        self.batches_out.push(Timed {
            time: t,
            value: BatchItem {
                batch: letter,
                network: network.clone(),
                addresses: addresses.clone(),
            },
        });
        self.held.network = Some(network);
        if !b.addresses.is_empty() {
            self.held.rows.push(ErmesRow {
                t,
                kind: RowKind::Addresses,
                batch: letter,
                address: None,
                category: None,
                text: addresses,
            });
        }
        for m in &b.messages {
            let row = ErmesRow {
                t,
                kind: RowKind::Message,
                batch: letter,
                address: Some(m.address),
                category: Some(m.category.label().to_string()),
                text: m.text.clone(),
            };
            self.pending_log.push(format!(
                "ermes: {} ({}): {}",
                m.address,
                m.category.label(),
                m.text
            ));
            self.messages_out.push(Timed {
                time: t,
                value: MessageItem(row.clone()),
            });
            self.held.rows.push(row);
        }
        self.dirty = true;
    }
}

impl Decoder for ErmesBatches {
    type Input = Timed<ErmesHeard>;
    fn name(&self) -> &'static str {
        "ermes.batches"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<BatchItem>("batches"),
            Port::typed::<MessageItem>("messages"),
        ]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::new(<ErmesFields<'static> as Fields>::SHAPE.fields).with_tables(&[&ERMES_ROWS])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            crate::typed_format::<BatchItem>(input),
            crate::typed_format::<MessageItem>(input),
        ])
    }
    fn process(&mut self, x: &[Timed<ErmesHeard>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.started = true;
            self.now = self.now.max(it.time);
            match &it.value {
                ErmesHeard::Carrier(s) => self.carrier = Some(*s),
                ErmesHeard::Sync => self.held.counters.syncs += 1,
                ErmesHeard::Bad => {
                    self.held.counters.bad_batches += 1;
                    self.dirty = true;
                }
                ErmesHeard::Batch(b) => self.batch(it.time, b),
            }
        }
        out.items::<BatchItem>(0).append(&mut self.batches_out);
        out.items::<MessageItem>(1).append(&mut self.messages_out);
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let recent = self.last_good.is_some_and(|t| now - t <= RECENT_S);
        let c = &self.held.counters;
        let carrier = self.carrier;
        p.put(&ErmesFields {
            batches: c.batches as i64,
            bad_batches: c.bad_batches as i64,
            addresses: c.addresses as i64,
            messages: c.messages as i64,
            words_repaired: c.repaired as i64,
            syncs: c.syncs as i64,
            network: self.held.network.as_deref(),
            carrier: carrier.map(|s| s.present),
            snr_db: carrier.filter(|s| s.present).map(|s| f64::from(s.snr_db)),
            level_dbfs: carrier.map(|s| f64::from(s.input_dbfs)),
            offset_hz: carrier.map(|s| Hertz(f64::from(s.afc_hz))),
            note: NOT_CONFIRMED,
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.rows, &ERMES_ROWS, now, p);
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
        }
        let lock_state = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let level = carrier.map(|s| s.input_dbfs);
        let (batches, messages) = (c.batches, c.messages);
        let network = self.held.network.as_deref().unwrap_or("");
        let sep = if network.is_empty() { "" } else { ", " };
        let r = p.results_mut();
        if !self.started {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("ERMES: waiting for samples"),
            );
        } else if batches > 0 {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("ERMES: {batches} batches, {messages} messages{sep}{network}"),
            );
        } else {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("ERMES: listening"),
            );
        }
    }
    fn clear(&mut self) {
        self.held.clear();
        self.last_good = None;
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The settings of the `ermes` stack: the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(gen::SETTINGS)))
        .clone()
}

/// The receiver: the modem and the batch decoder, with the `clear` command.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("modem", DecoderComponent::new(ErmesModem::default()))
        .add("batches", DecoderComponent::new(ErmesBatches::default()))
        .input("modem")
        .connect("modem", "batches")
        .output("batches.batches")
        .output("batches.messages")
        .status_from("batches")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "ERMES needs a sample rate of at least 96 ksps")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`receiver`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// Makes the looping test signal, the channel where the receiver looks.
///
/// # Errors
///
/// [`ConfigError`] if the rate cannot hold the signal.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::ErmesGenerator::from_config(
        cfg,
        crate::ERMES.tuning.tune_offset_hz,
    )?))
}

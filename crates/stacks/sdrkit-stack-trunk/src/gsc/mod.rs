//! Golay sequential code paging (`gsc`): 2-FSK on FM, Golay (23,12) words at 300 bit/s and BCH (15,7) data blocks at
//! 600 bit/s. Tone-only, numeric, alphanumeric and tone-and-voice pages, single calls and batches, with and without the
//! battery-saver preamble. The voice of a tone-and-voice page is marked, not decoded. Receive only; the stack holds no channel
//! list.
//!
//! Format: the Golay sequential code as the specification states it, checked against US patent 4423416. The `note` field names
//! what no public document confirms.

pub mod code;
pub mod gen;
pub mod rx;

use crate::front::CarrierState;
use crate::typed::{GscFields, GscPageEvent, GscPageRow, GSC_PAGE, GSC_PAGES};
use rx::{Address, GscHeard, GscModem, GscPage, GscStatus, PageKind, VoiceEnd, VoiceEndBy};
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
pub const ID: &str = "gsc";
/// The least input rate, Hz: what the paging front end needs.
pub const MIN_RATE: f64 = 96e3;
/// The most rows kept.
pub const ROW_CAP: usize = 400;
/// A page within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 10.0;
/// What no public document available here confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the preamble values, the start code 713, the Word 1 values, the rule from a pager code to its words, the illegal addresses, the order of the address functions 2 to 4, the character tables, the place of the continue bit and the BCH generator. The activation and deactivation words are not published: any word of their form is taken, and the test signal uses 3061 and 2457 as stand-ins. A voice page ends at its activation word again, at a deactivation word, or after 16 s. The voice is marked, not decoded.";

/// One row of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GscRow {
    /// Signal time, s.
    pub t: f64,
    /// What the row is ("Tone page", "Numeric page", "Alpha page", "Voice page", "Voice end").
    pub kind: String,
    /// The pager code as shown.
    pub code: String,
    /// The address function, 1 to 4.
    pub function: u8,
    /// The preamble index.
    pub preamble: Option<u8>,
    /// How the call was sent.
    pub call: String,
    /// The text.
    pub text: String,
    /// The data blocks.
    pub blocks: Option<u32>,
    /// A note.
    pub note: Option<String>,
    /// Bits corrected in the address words.
    pub corrected: u32,
    /// The activation or end word.
    pub word: Option<u16>,
    /// The length of the voice, s.
    pub voice_s: Option<f64>,
}

impl Shift for GscRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GscCounters {
    /// Tone pages.
    pub tone: u64,
    /// Numeric pages.
    pub numeric: u64,
    /// Alphanumeric pages.
    pub alpha: u64,
    /// Voice pages.
    pub voice: u64,
    /// Preambles seen.
    pub preambles: u64,
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GscHeld {
    /// The rows, newest last.
    pub rows: LogRing<GscRow>,
    /// The counters.
    pub counters: GscCounters,
}

impl Default for GscHeld {
    fn default() -> Self {
        GscHeld {
            rows: LogRing::new(ROW_CAP),
            counters: GscCounters::default(),
        }
    }
}

impl Shift for GscHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.shift_times(by_s);
    }
}

impl Tables for GscHeld {
    const FORMAT: StateFormat = StateFormat::new("trunk.gsc", 1);
    fn clear(&mut self) {
        self.rows.clear();
        self.counters = GscCounters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.rows.adopt(saved.rows);
        self.counters = saved.counters;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for GscRow {
    type Row = GscPageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&GscPageRow {
            time_s: self.t,
            kind: &self.kind,
            code: &self.code,
            function: i64::from(self.function),
            words: words_text(self.function),
            preamble: self.preamble.map(i64::from),
            call: &self.call,
            text: &self.text,
            blocks: self.blocks.map(i64::from),
            note: self.note.as_deref(),
            bits_corrected: i64::from(self.corrected),
            word: self.word.map(i64::from),
            voice_s: self.voice_s,
        });
    }
}

/// Which words of an address function are inverted.
fn words_text(function: u8) -> &'static str {
    match function {
        2 => "Word 2 inverted",
        3 => "Word 1 inverted",
        4 => "both words inverted",
        _ => "both words normal",
    }
}

/// A page as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct PageItem(pub GscRow);

impl Item for PageItem {
    const KIND: &'static str = "gsc.page";
    fn event_def() -> Option<&'static EventDef> {
        Some(GSC_PAGE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let r = &self.0;
        GscPageEvent {
            target: &r.code,
            kind: &r.kind,
            function: i64::from(r.function),
            text: &r.text,
            family: Family::Pager,
        }
        .write(f);
    }
}

/// The `gsc.pages` decoder: the rows, the counters, the events and the saved state.
#[derive(Default)]
pub struct GscPages {
    held: GscHeld,
    now: f64,
    started: bool,
    last_page: Option<f64>,
    carrier: Option<CarrierState>,
    status: GscStatus,
    pages_out: Vec<Timed<PageItem>>,
    pending_log: Vec<String>,
    cleared: bool,
    dirty: bool,
    summary: String,
}

impl GscPages {
    /// The rows and counters.
    pub fn held(&self) -> &GscHeld {
        &self.held
    }

    fn row_of(t: f64, kind: &str, a: &Address) -> GscRow {
        GscRow {
            t,
            kind: kind.to_string(),
            code: a.code.text(),
            function: a.function,
            preamble: a.preamble.map(|p| p as u8),
            call: a.call.label().to_string(),
            text: String::new(),
            blocks: None,
            note: None,
            corrected: a.corrected,
            word: None,
            voice_s: None,
        }
    }

    fn page(&mut self, t: f64, p: &GscPage) {
        let c = &mut self.held.counters;
        let kind = match p.kind {
            PageKind::Tone => {
                c.tone += 1;
                "Tone page"
            }
            PageKind::Numeric => {
                c.numeric += 1;
                "Numeric page"
            }
            PageKind::Alpha => {
                c.alpha += 1;
                "Alpha page"
            }
            PageKind::Voice => {
                c.voice += 1;
                "Voice page"
            }
        };
        let mut row = Self::row_of(t, kind, &p.address);
        match p.kind {
            PageKind::Numeric | PageKind::Alpha => {
                row.text.clone_from(&p.text);
                row.blocks = Some(p.blocks);
                if p.more {
                    row.note = Some("the last block said that more would follow".into());
                }
            }
            PageKind::Voice => {
                row.text = "tone and voice".into();
                row.word = p.activation;
                row.note = Some("the voice follows on the channel; it is not decoded".into());
            }
            PageKind::Tone => {}
        }
        self.last_page = Some(t);
        self.pending_log.push(format!(
            "gsc: {kind} {} function {}{}{}",
            row.code,
            row.function,
            if row.text.is_empty() { "" } else { ": " },
            row.text
        ));
        self.pages_out.push(Timed {
            time: t,
            value: PageItem(row.clone()),
        });
        self.held.rows.push(row);
        self.dirty = true;
    }

    fn voice_end(&mut self, t: f64, v: &VoiceEnd) {
        let mut row = Self::row_of(t, "Voice end", &v.address);
        row.word = v.word;
        row.voice_s = Some(v.length_s);
        row.note = Some(
            match v.by {
                VoiceEndBy::Activation => "the activation word came again",
                VoiceEndBy::Deactivation => "a deactivation word",
                VoiceEndBy::Timer => "no end word; closed by the 16 s timer",
            }
            .into(),
        );
        self.pending_log.push(format!(
            "gsc: voice end {} after {:.1} s",
            row.code, v.length_s
        ));
        self.held.rows.push(row);
        self.dirty = true;
    }
}

impl Decoder for GscPages {
    type Input = Timed<GscHeard>;
    fn name(&self) -> &'static str {
        "gsc.pages"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<PageItem>("pages")]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::new(<GscFields<'static> as Fields>::SHAPE.fields).with_tables(&[&GSC_PAGES])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![crate::typed_format::<PageItem>(input)])
    }
    fn process(&mut self, x: &[Timed<GscHeard>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.started = true;
            self.now = self.now.max(it.time);
            match &it.value {
                GscHeard::Carrier(s) => self.carrier = Some(*s),
                GscHeard::Status(s) => self.status = *s,
                GscHeard::Preamble(..) => {
                    self.held.counters.preambles += 1;
                    self.dirty = true;
                }
                GscHeard::Page(p) => self.page(it.time, p),
                GscHeard::VoiceEnd(v) => self.voice_end(it.time, v),
            }
        }
        out.items::<PageItem>(0).append(&mut self.pages_out);
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let recent = self.last_page.is_some_and(|t| now - t <= RECENT_S);
        let c = &self.held.counters;
        let carrier = self.carrier;
        let s = self.status;
        let pages = c.tone + c.numeric + c.alpha + c.voice;
        let present = carrier.is_some_and(|c| c.present);
        p.put(&GscFields {
            pages: pages as i64,
            tone_pages: c.tone as i64,
            numeric_pages: c.numeric as i64,
            alpha_pages: c.alpha as i64,
            voice_pages: c.voice as i64,
            preambles: c.preambles as i64,
            locked: s.locked,
            preamble: s.preamble.map(|(i, _)| i as i64),
            call: s
                .preamble
                .and_then(|(_, b)| b)
                .map(|b| if b { "batch" } else { "single call" }),
            inverted: s.inverted,
            deviation_hz: present.then_some(Hertz(f64::from(s.deviation_hz))),
            offset_hz: present.then_some(Hertz(f64::from(s.offset_hz))),
            clock_ppm: present.then_some(f64::from(s.clock_ppm)),
            quality_db: s.quality_db.filter(|_| s.locked).map(f64::from),
            carrier: carrier.map(|s| s.present),
            snr_db: carrier.filter(|s| s.present).map(|s| f64::from(s.snr_db)),
            level_dbfs: carrier.map(|s| f64::from(s.input_dbfs)),
            note: NOT_CONFIRMED,
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.rows, &GSC_PAGES, now, p);
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
        }
        let lock_state = if !self.started {
            LockState::Off
        } else if s.locked || recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let level = carrier.map(|s| s.input_dbfs);
        let r = p.results_mut();
        if !self.started {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("GSC: waiting for samples"),
            );
        } else if pages > 0 {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("GSC: {pages} pages"),
            );
        } else {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("GSC: listening"),
            );
        }
    }
    fn clear(&mut self) {
        self.held.clear();
        self.last_page = None;
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

/// The settings of the `gsc` stack: the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(gen::SETTINGS)))
        .clone()
}

/// The receiver: the modem and the page decoder, with the `clear` command.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("modem", DecoderComponent::new(GscModem::default()))
        .add("pages", DecoderComponent::new(GscPages::default()))
        .input("modem")
        .connect("modem", "pages")
        .output("pages.pages")
        .status_from("pages")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "GSC needs a sample rate of at least 96 ksps")
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
    Ok(Box::new(gen::GscGenerator::from_config(
        cfg,
        crate::GSC.tuning.tune_offset_hz,
    )?))
}

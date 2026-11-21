//! The pagers (`pager`): POCSAG 512, 1200 and 2400 bit/s and FLEX 1600/2, 3200/2, 3200/4 and 6400/4 on one channel, with the
//! sequential calls sent on the same channel as an option. The demodulator is the stack's own: `pager.demod` and `pager.parse`
//! are used twice, on the 25 kHz channel (`demod`, `parse`) and behind a 7 kHz filter (`narrow_demod`, `narrow_parse`). `sdrkit-proto-pager` reads the code
//! words. The stack sends POCSAG pages ([`tx`]) into a transmit sink; it does not send FLEX.

pub mod demod;
pub mod gen;
pub mod parse;
pub mod tables;
pub mod tx;

use crate::front::{PagerDisc, PagerFront, CHANNEL, NARROW_DECODER};
use crate::{typed_format, typed_port};
use demod::{FLEX, POCSAG, POCSAG_1200, POCSAG_2400, POCSAG_512};
use parse::{CHARSET, COMBINE, READ, READ_BY_ADDRESS};
use sdrkit_core::publish::field;
use sdrkit_core::{
    ConfigError, ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Port, StackGraph, Timed,
};
use sdrkit_proto_tones::selcall::{SelcallCall, SelcallConfig, SelcallDecoder};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "pager";

/// A page as the graph carries it: the page `sdrkit-proto-pager` read, with the stack's own fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageItem {
    /// The page.
    pub page: sdrkit_proto_pager::Page,
    /// The carrier offset measured at the lock, Hz.
    pub offset_hz: f64,
}

impl Item for PageItem {
    const KIND: &'static str = "pager.page";
}

/// `pager.selcall`: the sequential calls on the paging channel, read by `sdrkit-proto-tones` from the discriminator. It runs only
/// when the `selcall` setting is on.
pub struct PagerSelcall {
    on: bool,
    dec: Option<SelcallDecoder>,
    rate: f64,
    origin: Option<u64>,
    out: Vec<Timed<SelcallCall>>,
}

impl Default for PagerSelcall {
    fn default() -> Self {
        PagerSelcall {
            on: false,
            dec: None,
            rate: 0.0,
            origin: None,
            out: Vec::new(),
        }
    }
}

/// Also decode the sequential calls.
pub const SELCALL: Setting<field::Bool> = Setting::new("selcall", "Sequential calls", "", false)
    .help("Also decode the sequential tone calls sent on the same channel.");
/// The input is one carrier at 192 ksps.
pub const WIDE_INPUT: Setting<field::Bool> = Setting::new("wide_input", "One carrier", "", false)
    .help("The input is one carrier at 192 ksps (a channel of a bank span).")
    .restart();

sdrkit_core::settings! {
    /// The settings of `pager.selcall`.
    pub enum SelcallSetting {
        /// Also decode the sequential calls.
        Selcall = SELCALL,
    }
}

impl Component for PagerSelcall {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "pager.selcall",
            vec![Port::real("in")],
            vec![typed_port::<SelcallCall>("out")],
        )
        .with_settings(SelcallSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.dec = Some(SelcallDecoder::new(self.rate, SelcallConfig::default())?);
        self.origin = None;
        Ok(vec![typed_format::<SelcallCall>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        if !self.on {
            return;
        }
        let first = io.input(0).first();
        let x = io.real(0);
        let Some(dec) = self.dec.as_mut() else { return };
        let origin = *self.origin.get_or_insert(first);
        let (rate, out) = (self.rate, &mut self.out);
        dec.process(x, |c| {
            out.push(Timed {
                time: (origin + c.start) as f64 / rate,
                value: c,
            })
        });
        io.items_out::<SelcallCall>(0).append(&mut self.out);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SelcallSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SelcallSetting::Selcall => {
                let on = SELCALL.read(id, v)?;
                if on && !self.on {
                    if let Some(d) = self.dec.as_mut() {
                        d.reset();
                    }
                    self.origin = None;
                }
                self.on = on;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SelcallSetting::Selcall, Value::Bool(self.on))])
    }
}

sdrkit_core::settings! {
    /// The settings of the pager stack.
    pub enum PagerSetting {
        /// Look for POCSAG syncs.
        Pocsag = POCSAG,
        /// Look for FLEX syncs.
        Flex = FLEX,
        /// POCSAG at 512 bit/s.
        Pocsag512 = POCSAG_512,
        /// POCSAG at 1200 bit/s.
        Pocsag1200 = POCSAG_1200,
        /// POCSAG at 2400 bit/s.
        Pocsag2400 = POCSAG_2400,
        /// Also decode the sequential calls.
        Selcall = SELCALL,
        /// The channel filter.
        Channel = CHANNEL,
        /// The second decoder behind a 7 kHz filter.
        NarrowDecoder = NARROW_DECODER,
        /// Vote repeated copies.
        Combine = COMBINE,
        /// How POCSAG pages are read.
        Read = READ,
        /// The reading of single addresses.
        ReadByAddress = READ_BY_ADDRESS,
        /// The character set.
        Charset = CHARSET,
        /// The input is one carrier at 192 ksps.
        WideInput = WIDE_INPUT,
        /// The protocol sent.
        TxProtocol = tx::PROTOCOL,
        /// The bit rate sent.
        TxRate = tx::POCSAG_RATE,
        /// The deviation sent.
        TxDeviation = tx::DEVIATION,
        /// The polarity sent.
        TxPolarity = tx::POLARITY,
        /// The level sent.
        TxLevel = tx::TX_LEVEL,
    }
}

/// The stack's graph: the front end, two decoders (demodulator and parser) on the 25 kHz channel and on the same samples
/// behind a 7 kHz filter, and the tables, which merge their pages; the sequential calls leave as their events. The POCSAG
/// sender takes the same input and writes into the transmit sink.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", PagerFront::new(true))
        .add("disc", PagerDisc::default())
        .add("demod", demod::PagerDemodComponent::default())
        .add("parse", parse::PagerParse::default())
        .add("narrow_demod", demod::PagerDemodComponent::default())
        .add("narrow_parse", parse::PagerParse::default())
        .add("selcall", PagerSelcall::default())
        .add("tables", tables::PagerTables::default())
        .add("sender", tx::PagerSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("front.in")
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .connect("front.out", "disc")
        .connect("front.out", "demod.in")
        .connect("front.carrier", "demod.carrier")
        .connect("front.afc", "demod.afc")
        .feedback("demod.busy", "front.busy")
        .connect("demod.words", "parse")
        .connect("front.narrow", "narrow_demod.in")
        .connect("front.carrier", "narrow_demod.carrier")
        .connect("front.afc", "narrow_demod.afc")
        .feedback("narrow_demod.busy", "front.narrow_busy")
        .connect("narrow_demod.words", "narrow_parse")
        .connect("narrow_parse.pages", "tables.narrow_pages")
        .connect("narrow_parse.stats", "tables.narrow_stats")
        .connect("narrow_demod.lock", "tables.narrow_lock")
        .connect("disc", "selcall")
        .connect("parse.pages", "tables.pages")
        .connect("parse.stats", "tables.stats")
        .connect("demod.lock", "tables.lock")
        .connect("front.carrier", "tables.carrier")
        .connect("selcall", "tables.selcall")
        .output("tables")
        .output("selcall")
        .output("sender.events")
        .clock(0.25)
        .status_from("tables")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    tables::declaration()
}

/// The receiver: 1 Msps at least, 192 ksps with the wide-input setting.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let min = if cfg.settings.get(&WIDE_INPUT) {
        192e3
    } else {
        1e6
    };
    let mut b = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            min,
            "the pagers need at least 1 Msps from a radio (192 ksps for one carrier)",
        )
        .route(WIDE_INPUT.key(), &[])?
        .route(SELCALL.key(), &["selcall.selcall"])?
        .route(CHANNEL.key(), &["front.channel"])?
        .route(NARROW_DECODER.key(), &["front.narrow_decoder"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            crate::typed::PAGER_SEND.command.key,
            sdrkit_graph::CommandTarget::of("sender", crate::typed::PAGER_SEND.command.key),
        )
        .command(
            crate::typed::PAGER_HALT.key,
            sdrkit_graph::CommandTarget::of("sender", crate::typed::PAGER_HALT.key),
        );
    for key in [
        tx::PROTOCOL.key(),
        tx::POCSAG_RATE.key(),
        tx::DEVIATION.key(),
        tx::POLARITY.key(),
        tx::TX_LEVEL.key(),
    ] {
        b = b.route(key, &[&format!("sender.{key}")])?;
    }
    for key in [
        POCSAG.key(),
        FLEX.key(),
        POCSAG_512.key(),
        POCSAG_1200.key(),
        POCSAG_2400.key(),
    ] {
        b = b.route(
            key,
            &[&format!("demod.{key}"), &format!("narrow_demod.{key}")],
        )?;
    }
    for key in [
        COMBINE.key(),
        READ.key(),
        READ_BY_ADDRESS.key(),
        CHARSET.key(),
    ] {
        b = b.route(
            key,
            &[&format!("parse.{key}"), &format!("narrow_parse.{key}")],
        )?;
    }
    b.with_settings(&cfg.settings).build()
}

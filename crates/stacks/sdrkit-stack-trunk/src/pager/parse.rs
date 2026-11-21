//! `pager.parse`: POCSAG page assembly and FLEX phase decoding by `sdrkit-proto-pager`, from the demodulator's code words, with the
//! voting of repeated copies, the word counts of each speed and the pages that the acceptance rules of `sdrkit-proto-pager`
//! refused.

use super::demod::{Decoded, PagerWords};
use super::PageItem;
use crate::{typed_format, typed_port};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Item, Port, Timed};
use sdrkit_proto_pager::flex::{self, FlexDecoder, PHASE_BITS};
use sdrkit_proto_pager::pocsag::{self, PocsagAssembler, Rate};
use sdrkit_proto_pager::{Kind, Page, ReadMode};

/// Word counts of one batch or frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WordStat {
    /// "POCSAG 1200", "FLEX 3200/4".
    pub speed: &'static str,
    /// Valid as received.
    pub good: u64,
    /// Repaired.
    pub repaired: u64,
    /// Lost.
    pub lost: u64,
    /// Batches or frames locked with at least one good word.
    pub transmissions: u64,
    /// Pages refused by the acceptance rules.
    pub refused: u64,
    /// Of them, pages refused by a rule that no public document states.
    pub refused_unconfirmed: u64,
    /// The carrier offset of the lock, Hz.
    pub offset_hz: f64,
}

impl Item for WordStat {
    const KIND: &'static str = "pager.stat";
}

/// Reads "123:alpha, 456:numeric" (separated by commas, spaces, semicolons or line breaks). Each item is an address and a
/// reading.
///
/// # Errors
///
/// The byte position of the first item that is not an address and a reading.
pub fn parse_by_address(s: &str) -> Result<Vec<(u32, bool)>, usize> {
    let mut out = Vec::new();
    for item in s.split([',', ' ', ';', '\n']) {
        let at = item.as_ptr() as usize - s.as_ptr() as usize;
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let parsed = item.split_once(':').and_then(|(a, m)| {
            let a: u32 = a.trim().parse().ok()?;
            if a > pocsag::MAX_RIC {
                return None;
            }
            match m.trim() {
                "alpha" => Some((a, true)),
                "numeric" => Some((a, false)),
                _ => None,
            }
        });
        out.push(parsed.ok_or(at)?);
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    good: u64,
    repaired: u64,
    lost: u64,
    refused: u64,
    refused_unconfirmed: u64,
}

impl Counts {
    fn of(c: (u64, u64, u64), r: sdrkit_proto_pager::Refusals) -> Counts {
        Counts {
            good: c.0,
            repaired: c.1,
            lost: c.2,
            refused: r.total,
            refused_unconfirmed: r.unconfirmed,
        }
    }
}

/// `pager.parse` (module documentation).
pub struct PagerParse {
    pocsag: [PocsagAssembler; 3],
    flex: FlexDecoder,
    cfg: pocsag::Config,
    flex_cfg: flex::Config,
    charset: Charset,
    read: ReadChoice,
    by_address: String,
    seen: [Counts; 4],
    phases: Vec<f32>,
    second: Vec<f32>,
    pages: Vec<Timed<PageItem>>,
    stats: Vec<Timed<WordStat>>,
}

const RATES: [Rate; 3] = [Rate::B512, Rate::B1200, Rate::B2400];

fn assembler(cfg: &pocsag::Config, rate: Rate) -> PocsagAssembler {
    let cfg = pocsag::Config {
        rate,
        ..cfg.clone()
    };
    PocsagAssembler::new(cfg).expect("the default settings are in range")
}

impl Default for PagerParse {
    fn default() -> Self {
        let cfg = pocsag::Config::default();
        let flex_cfg = flex::Config::default();
        PagerParse {
            pocsag: RATES.map(|r| assembler(&cfg, r)),
            flex: FlexDecoder::new(flex_cfg).expect("the default settings are in range"),
            cfg,
            flex_cfg,
            charset: Charset::Ascii,
            read: ReadChoice::Auto,
            by_address: String::new(),
            seen: [Counts::default(); 4],
            phases: vec![0.0; 4 * PHASE_BITS],
            second: vec![0.0; 4 * PHASE_BITS],
            pages: Vec::new(),
            stats: Vec::new(),
        }
    }
}

fn rate_index(baud: u16) -> usize {
    match baud {
        512 => 0,
        2400 => 2,
        _ => 1,
    }
}

/// Keeps a page that a decoder returned, with the German characters when they are chosen.
fn keep(pages: &mut Vec<Timed<PageItem>>, german_set: bool, p: &Page, offset_hz: f64) {
    let mut page = p.clone();
    if german_set && page.kind == Kind::Alpha {
        let mut de = String::with_capacity(page.text.len() + 8);
        sdrkit_util_text::ia5::din66003_into(&page.text, &mut de);
        page.text = de;
    }
    pages.push(Timed {
        time: p.time,
        value: PageItem { page, offset_hz },
    });
}

impl PagerParse {
    fn stat(
        &mut self,
        i: usize,
        speed: &'static str,
        c: Counts,
        t: f64,
        transmissions: u64,
        offset_hz: f64,
    ) {
        let before = std::mem::replace(&mut self.seen[i], c);
        self.stats.push(Timed {
            time: t,
            value: WordStat {
                speed,
                good: c.good.saturating_sub(before.good),
                repaired: c.repaired.saturating_sub(before.repaired),
                lost: c.lost.saturating_sub(before.lost),
                refused: c.refused.saturating_sub(before.refused),
                refused_unconfirmed: c
                    .refused_unconfirmed
                    .saturating_sub(before.refused_unconfirmed),
                transmissions,
                offset_hz,
            },
        });
    }
    fn pocsag_counts(&self, i: usize) -> Counts {
        let a = &self.pocsag[i];
        let c = a.counts();
        Counts::of((c.good, c.repaired, c.lost), a.refusals())
    }
    /// Takes one item of code words.
    pub fn take(&mut self, b: &PagerWords) {
        let t = b.time;
        let off = b.offset_hz;
        let ger = self.charset == Charset::German;
        let pages = &mut self.pages;
        match &b.what {
            Decoded::PocsagWords {
                baud,
                first_slot,
                words,
            } => {
                let a = &mut self.pocsag[rate_index(*baud)];
                for (k, (raw, rel)) in words.iter().enumerate() {
                    a.push(first_slot + k, *raw, Some(rel), t, |p| {
                        keep(pages, ger, p, off)
                    });
                }
            }
            Decoded::PocsagBatchEnd { baud, valid } => {
                let i = rate_index(*baud);
                let c = self.pocsag_counts(i);
                self.stat(i, RATES[i].name(), c, t, (*valid > 0) as u64, off);
            }
            Decoded::PocsagEnd { baud } => {
                let i = rate_index(*baud);
                self.pocsag[i].end(t, |p| keep(pages, ger, p, off));
                let c = self.pocsag_counts(i);
                self.stat(i, RATES[i].name(), c, t, 0, off);
            }
            Decoded::FlexFrame {
                speed,
                fiw,
                symbols,
                second,
            } => {
                let np = speed.phases();
                let (bufs, _) = self.phases.split_at_mut(np * PHASE_BITS);
                let mut parts: [&mut [f32]; 4] = Default::default();
                for (slot, chunk) in parts.iter_mut().zip(bufs.chunks_mut(PHASE_BITS)) {
                    *slot = chunk;
                }
                let mut ok = flex::split_phases(*speed, symbols, &mut parts[..np]).is_ok();
                if ok && second.len() == symbols.len() {
                    // The second bit of each 4-level symbol (phases B and D) comes from its own values.
                    let (bufs, _) = self.second.split_at_mut(np * PHASE_BITS);
                    let mut other: [&mut [f32]; 4] = Default::default();
                    for (slot, chunk) in other.iter_mut().zip(bufs.chunks_mut(PHASE_BITS)) {
                        *slot = chunk;
                    }
                    ok = flex::split_phases(*speed, second, &mut other[..np]).is_ok();
                    for p in (1..np).step_by(2) {
                        parts[p].copy_from_slice(other[p]);
                    }
                }
                if ok {
                    for (p, llr) in parts[..np].iter().enumerate() {
                        // Each phase holds `PHASE_BITS` values, the one length the decoder refuses otherwise.
                        let _ = self.flex.decode_llrs(*speed, *fiw, p as u8, llr, t, |pg| {
                            keep(pages, ger, pg, off)
                        });
                    }
                }
                let c = self.flex.counts();
                let now = Counts::of((c.good, c.repaired, c.lost), self.flex.refusals());
                let before = self.seen[3];
                let read = now.good + now.repaired > before.good + before.repaired;
                self.stat(3, speed.name(), now, t, read as u64, off);
            }
        }
    }
}

sdrkit_core::choice_enum! {
    /// How POCSAG pages are read.
    pub enum ReadChoice {
        /// Text when the reading looks like text.
        Auto = ("auto", "Auto"),
        /// By the function code.
        Function = ("function", "By function code"),
        /// Always text.
        Alpha = ("alpha", "Force alphanumeric"),
        /// Always numeric.
        Numeric = ("numeric", "Force numeric"),
    }
}

impl ReadChoice {
    /// The reading of `sdrkit-proto-pager`.
    fn mode(self) -> ReadMode {
        match self {
            ReadChoice::Auto => ReadMode::Auto,
            ReadChoice::Function => ReadMode::Function,
            ReadChoice::Alpha => ReadMode::Alpha,
            ReadChoice::Numeric => ReadMode::Numeric,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The character set of text pages.
    pub enum Charset {
        /// ASCII.
        Ascii = ("ascii", "ASCII"),
        /// The German national set of DIN 66003.
        German = ("german", "German (ä ö ü ß)"),
    }
}

/// Vote repeated copies.
pub const COMBINE: Setting<field::Bool> = Setting::new("combine", "Combine copies", "", true)
    .help("Paging networks repeat messages: vote the copies word by word.");
/// How POCSAG pages are read.
pub const READ: Setting<field::Choice<ReadChoice>> =
    Setting::choice("read", "Read POCSAG pages", ReadChoice::Auto);
/// The reading of single addresses.
pub const READ_BY_ADDRESS: Setting<field::Text> = Setting::new("read_by_address", "Read by address", "", "")
    .help("Rules for single addresses: \"address:alpha\" or \"address:numeric\", separated by commas or spaces.");
/// The character set.
pub const CHARSET: Setting<field::Choice<Charset>> =
    Setting::choice("charset", "Character set", Charset::Ascii);

sdrkit_core::settings! {
    /// The settings of `pager.parse`.
    pub enum ParseSetting {
        /// Vote repeated copies.
        Combine = COMBINE,
        /// How POCSAG pages are read.
        Read = READ,
        /// The reading of single addresses.
        ReadByAddress = READ_BY_ADDRESS,
        /// The character set.
        Charset = CHARSET,
    }
}

impl Component for PagerParse {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "pager.parse",
            vec![Port::typed::<PagerWords>("in")],
            vec![
                typed_port::<PageItem>("pages"),
                typed_port::<WordStat>("stats"),
            ],
        )
        .with_settings(ParseSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The voting history is kept. The open pages are forgotten.
        for a in &mut self.pocsag {
            a.reset();
        }
        Ok(vec![
            typed_format::<PageItem>(&inputs[0]),
            typed_format::<WordStat>(&inputs[0]),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for b in io.items::<PagerWords>(0) {
            self.take(&b.value);
        }
        io.items_out::<PageItem>(0).append(&mut self.pages);
        io.items_out::<WordStat>(1).append(&mut self.stats);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ParseSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ParseSetting::Combine => {
                let on = COMBINE.read(id, v)?;
                let flex_cfg = flex::Config {
                    combine: on,
                    ..self.flex_cfg
                };
                self.flex =
                    FlexDecoder::new(flex_cfg).map_err(|_| SettingError::NotPermitted { id })?;
                self.flex_cfg = flex_cfg;
                self.cfg.combine = on;
                for a in &mut self.pocsag {
                    a.set_combine(on);
                }
            }
            ParseSetting::Read => {
                self.read = READ.read(id, v)?;
                self.cfg.read = self.read.mode();
                for a in &mut self.pocsag {
                    a.set_read(self.cfg.read);
                }
            }
            ParseSetting::ReadByAddress => {
                let s = READ_BY_ADDRESS.read(id, v)?;
                let rules = parse_by_address(s)
                    .map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                self.cfg.by_address = rules;
                self.pocsag = RATES.map(|r| assembler(&self.cfg, r));
                s.clone_into(&mut self.by_address);
            }
            ParseSetting::Charset => self.charset = CHARSET.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ParseSetting::Combine, Value::Bool(self.cfg.combine)),
            (ParseSetting::Read, Value::Choice(self.read.index())),
            (
                ParseSetting::ReadByAddress,
                Value::Text(self.by_address.clone()),
            ),
            (ParseSetting::Charset, Value::Choice(self.charset.index())),
        ])
    }
}

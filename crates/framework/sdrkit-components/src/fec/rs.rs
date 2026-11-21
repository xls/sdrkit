//! `fec.rs_decode`, with and without its erasure input: Reed-Solomon decoding of byte packets, one (interleaved) frame per packet.

use super::common::{Erasures, FrameStatus, PacketPool};
use sdrkit_algo_fec::rs::{codes, Basis, Rs, RsCode, SymbolOrder};
use sdrkit_algo_fec::FecError;
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Packet, Port, Timed};
use std::collections::VecDeque;

/// Packets that arrived with no symbol in error.
pub const PACKETS_CLEAN: Field<field::I64> = Field::new("packets_clean", "Packets clean", "");
/// Packets with symbols corrected.
pub const PACKETS_CORRECTED: Field<field::I64> =
    Field::new("packets_corrected", "Packets corrected", "");
/// Packets with a code word that could not be corrected (or of the wrong length).
pub const PACKETS_FAILED: Field<field::I64> = Field::new("packets_failed", "Packets failed", "");
/// Symbols corrected (errors and erasures).
pub const SYMBOLS_CORRECTED: Field<field::I64> =
    Field::new("symbols_corrected", "Symbols corrected", "");

sdrkit_core::choice_enum! {
    /// Every named Reed-Solomon code of `sdrkit_algo_fec::rs::codes`, by the constant's name in snake case. `vdl2` is the VDL mode 2 code,
    /// whose block length is the `length` setting.
    pub enum RsCodeName {
        /// `dvb_204_188`.
        Dvb204188 = ("dvb_204_188", "dvb_204_188"),
        /// `isdb_t_204_196`.
        IsdbT204196 = ("isdb_t_204_196", "isdb_t_204_196"),
        /// `atsc_207_187`.
        Atsc207187 = ("atsc_207_187", "atsc_207_187"),
        /// `dab_plus_120_110`.
        DabPlus120110 = ("dab_plus_120_110", "dab_plus_120_110"),
        /// `isdb_s_tmcc_64_48`.
        IsdbSTmcc6448 = ("isdb_s_tmcc_64_48", "isdb_s_tmcc_64_48"),
        /// `dmr_12_9`.
        Dmr129 = ("dmr_12_9", "dmr_12_9"),
        /// `hd_radio_96_88`.
        HdRadio9688 = ("hd_radio_96_88", "hd_radio_96_88"),
        /// `rs41_255_231`.
        Rs41255231 = ("rs41_255_231", "rs41_255_231"),
        /// `rs92_234_210`.
        Rs92234210 = ("rs92_234_210", "rs92_234_210"),
        /// `ccsds_255_223`.
        Ccsds255223 = ("ccsds_255_223", "ccsds_255_223"),
        /// `ccsds_255_239`.
        Ccsds255239 = ("ccsds_255_239", "ccsds_255_239"),
        /// `ccsds_255_223_conventional`.
        Ccsds255223Conventional = ("ccsds_255_223_conventional", "ccsds_255_223_conventional"),
        /// `uat_30_18`.
        Uat3018 = ("uat_30_18", "uat_30_18"),
        /// `uat_48_34`.
        Uat4834 = ("uat_48_34", "uat_48_34"),
        /// `uat_92_72`.
        Uat9272 = ("uat_92_72", "uat_92_72"),
        /// `vdl2`.
        Vdl2 = ("vdl2", "vdl2"),
        /// `galileo_fec2_118_58`.
        GalileoFec211858 = ("galileo_fec2_118_58", "galileo_fec2_118_58"),
        /// `p25_24_12`.
        P252412 = ("p25_24_12", "p25_24_12"),
        /// `p25_24_16`.
        P252416 = ("p25_24_16", "p25_24_16"),
        /// `p25_36_20`.
        P253620 = ("p25_36_20", "p25_36_20"),
        /// `p25_45_26`.
        P254526 = ("p25_45_26", "p25_45_26"),
        /// `p25_52_30`.
        P255230 = ("p25_52_30", "p25_52_30"),
        /// `jt65_63_12`.
        Jt656312 = ("jt65_63_12", "jt65_63_12"),
    }
}

impl RsCodeName {
    /// The code.
    pub fn code(self) -> RsCode {
        match self {
            RsCodeName::Dvb204188 => codes::DVB_204_188,
            RsCodeName::IsdbT204196 => codes::ISDB_T_204_196,
            RsCodeName::Atsc207187 => codes::ATSC_207_187,
            RsCodeName::DabPlus120110 => codes::DAB_PLUS_120_110,
            RsCodeName::IsdbSTmcc6448 => codes::ISDB_S_TMCC_64_48,
            RsCodeName::Dmr129 => codes::DMR_12_9,
            RsCodeName::HdRadio9688 => codes::HD_RADIO_96_88,
            RsCodeName::Rs41255231 => codes::RS41_255_231,
            RsCodeName::Rs92234210 => codes::RS92_234_210,
            RsCodeName::Ccsds255223 => codes::CCSDS_255_223,
            RsCodeName::Ccsds255239 => codes::CCSDS_255_239,
            RsCodeName::Ccsds255223Conventional => codes::CCSDS_255_223_CONVENTIONAL,
            RsCodeName::Uat3018 => codes::UAT_30_18,
            RsCodeName::Uat4834 => codes::UAT_48_34,
            RsCodeName::Uat9272 => codes::UAT_92_72,
            RsCodeName::Vdl2 => codes::vdl2(255),
            RsCodeName::GalileoFec211858 => codes::GALILEO_FEC2_118_58,
            RsCodeName::P252412 => codes::P25_24_12,
            RsCodeName::P252416 => codes::P25_24_16,
            RsCodeName::P253620 => codes::P25_36_20,
            RsCodeName::P254526 => codes::P25_45_26,
            RsCodeName::P255230 => codes::P25_52_30,
            RsCodeName::Jt656312 => codes::JT65_63_12,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The symbol basis of [`RsDecode`].
    pub enum SymbolBasis {
        /// As the code says.
        Code = ("code", "As the code says"),
        /// Conventional.
        Conventional = ("conventional", "Conventional"),
        /// CCSDS dual basis.
        Dual = ("dual", "CCSDS dual basis"),
    }
}

/// The most packets (or erasure lists) held waiting for their partner on the other input when `erasures` is connected; beyond
/// that the oldest packet is decoded without erasures.
const MAX_WAITING: usize = 64;

const CODE: Setting<field::Choice<RsCodeName>> =
    Setting::choice("code", "Code", RsCodeName::Dvb204188);
const DEPTH: Setting<field::I64> = Setting::new("depth", "Interleaving depth", "", 1)
    .range(1.0, 32.0, 1.0)
    .help("Code words per packet, interleaved symbol by symbol.");
const BASIS: Setting<field::Choice<SymbolBasis>> =
    Setting::choice("basis", "Symbol basis", SymbolBasis::Code);
const LENGTH: Setting<field::I64> = Setting::new("length", "Code word length", "", 0)
    .range(0.0, 255.0, 1.0)
    .help("Symbols per code word of a shortened code; 0 for the code's own length.");
const KEEP_PARITY: Setting<field::Bool> = Setting::new("keep_parity", "Keep parity", "", false)
    .help("Output the whole corrected frame, not only the data.");
const DROP_FAILED: Setting<field::Bool> =
    Setting::new("drop_failed", "Drop failed packets", "", false);

sdrkit_core::settings! {
    /// The settings of [`RsDecode`].
    pub enum RsDecodeSetting {
        /// Code.
        Code = CODE,
        /// Interleaving depth.
        Depth = DEPTH,
        /// Symbol basis.
        Basis = BASIS,
        /// Code word length.
        Length = LENGTH,
        /// Keep parity.
        KeepParity = KEEP_PARITY,
        /// Drop failed packets.
        DropFailed = DROP_FAILED,
    }
}

/// Decodes Reed-Solomon code words, one frame per `Bytes` packet: a frame is `depth` code words of `n` symbols interleaved symbol
/// by symbol (symbol `i` belongs to code word `i mod depth`), so a packet holds `n × depth` bytes. Symbols are one per byte (codes
/// over fields of up to 8 bits).
///
/// Outputs: `out`, the corrected data (`k × depth` bytes; the whole frame with `keep_parity`), and `status`, one
/// [`FrameStatus`] per input packet, in order. A packet whose code words could not all be corrected is passed on as received
/// (its status says so), or dropped with `drop_failed`; a packet of the wrong length is dropped and counted as failed.
///
/// Settings: `code` (named), `depth` (1 to 32), `basis` (`code`, `conventional`, or `dual` for the CCSDS dual basis on air),
/// `length` (shortened code word length; 0 for the code's own, and the block length for `vdl2`).
///
/// The optional input `erasures` (typed, may be left unconnected) carries one [`Erasures`] list per packet (frame indices of
/// unreliable symbols), paired with the packets in order. Unconnected, every packet is decoded with no erasures.
#[derive(Debug)]
pub struct RsDecode {
    name: RsCodeName,
    code: RsCode,
    depth: usize,
    basis: SymbolBasis,
    length: usize,
    keep_parity: bool,
    drop_failed: bool,
    rs: Option<Rs>,
    frame: Vec<u8>,
    pool: PacketPool,
    waiting: VecDeque<Packet>,
    lists: VecDeque<(usize, usize)>,
    flat: Vec<u16>,
    clean: u64,
    fixed: u64,
    failed: u64,
    symbols: u64,
    since: u32,
    published_at: f64,
    published: Published,
}

impl Default for RsDecode {
    fn default() -> Self {
        RsDecode {
            name: CODE.default,
            code: codes::DVB_204_188,
            depth: DEPTH.default as _,
            basis: BASIS.default,
            length: LENGTH.default as _,
            keep_parity: KEEP_PARITY.default,
            drop_failed: DROP_FAILED.default,
            rs: None,
            frame: Vec::new(),
            pool: PacketPool::default(),
            waiting: VecDeque::new(),
            lists: VecDeque::new(),
            flat: Vec::new(),
            clean: 0,
            fixed: 0,
            failed: 0,
            symbols: 0,
            since: 0,
            published_at: f64::NEG_INFINITY,
            published: Published::new(&[
                &PACKETS_CLEAN,
                &PACKETS_CORRECTED,
                &PACKETS_FAILED,
                &SYMBOLS_CORRECTED,
            ]),
        }
    }
}

impl RsDecode {
    /// The code in use, after `length` and `basis`.
    fn effective(&self) -> Result<RsCode, ConfigError> {
        let mut c = self.code;
        if self.name == RsCodeName::Vdl2 {
            let n = if self.length == 0 { 255 } else { self.length };
            c = codes::vdl2(n as u16);
        } else if self.length > 0 {
            if self.length > c.n as usize || self.length <= c.nroots() {
                return Err(SettingError::Conflict {
                    id: RsDecodeSetting::Length.id(),
                    with: RsDecodeSetting::Code.id(),
                }
                .into());
            }
            c = c.shortened(self.length as u16);
        }
        match self.basis {
            SymbolBasis::Conventional => c.basis = Basis::Conventional,
            SymbolBasis::Dual => c.basis = Basis::Dual,
            SymbolBasis::Code => {}
        }
        if c.m > 8 {
            return Err(SettingError::NotPermitted {
                id: RsDecodeSetting::Code.id(),
            }
            .into());
        }
        Ok(c)
    }

    /// Decodes one packet with the erasure list `er` and emits it.
    fn decode_one(&mut self, p: &Packet, er: &[u16], io: &mut Io) {
        let Some(rs) = self.rs.as_mut() else {
            return;
        };
        let c = *rs.code();
        let (n, k, d) = (c.n as usize, c.k as usize, self.depth);
        let mut status = FrameStatus {
            ok: false,
            corrected: 0,
            iterations: 1,
            failed_words: d as u32,
        };
        let bytes = p.bytes();
        let emit = if bytes.len() != n * d {
            self.failed += 1;
            false
        } else {
            self.frame.clear();
            self.frame.extend_from_slice(bytes);
            let r = if d == 1 {
                match rs.decode(&mut self.frame, er) {
                    Ok(r) => Ok((r.corrected, 0)),
                    Err(FecError::Uncorrectable) => Ok((0, 1)),
                    Err(e) => Err(e),
                }
            } else {
                rs.decode_interleaved(&mut self.frame, d, er)
                    .map(|r| (r.corrected, r.failed))
            };
            match r {
                Ok((corrected, failed)) => {
                    status = FrameStatus {
                        ok: failed == 0,
                        corrected,
                        iterations: 1,
                        failed_words: failed,
                    };
                    self.symbols += corrected as u64;
                    if failed > 0 {
                        self.failed += 1;
                    } else if corrected > 0 {
                        self.fixed += 1;
                    } else {
                        self.clean += 1;
                    }
                    status.ok || !self.drop_failed
                }
                Err(_) => {
                    self.failed += 1;
                    false
                }
            }
        };
        if emit {
            let range = if self.keep_parity {
                0..n * d
            } else if c.order == SymbolOrder::HighFirst {
                0..k * d
            } else {
                (n - k) * d..n * d
            };
            let frame = &self.frame[range];
            let pkt = self
                .pool
                .make(frame.len(), p.time, |b| b.copy_from_slice(frame));
            io.bytes_out(0).push(pkt);
        }
        io.items_out::<FrameStatus>(1).push(Timed {
            time: p.time,
            value: status,
        });
        self.since += 1;
        if p.time - self.published_at >= 0.25 || self.since >= 256 {
            self.publish(p.time);
        }
    }

    /// Queues an erasure list, making room in the flat store by dropping lists already used.
    fn push_list(&mut self, list: &[u16]) {
        if self.flat.len() + list.len() > self.flat.capacity() {
            let keep = self.lists.front().map_or(self.flat.len(), |&(s, _)| s);
            self.flat.drain(..keep);
            for l in self.lists.iter_mut() {
                l.0 -= keep;
            }
        }
        let start = self.flat.len();
        self.flat.extend_from_slice(list);
        self.lists.push_back((start, list.len()));
    }

    fn publish(&mut self, time: f64) {
        self.published.set(&PACKETS_CLEAN, self.clean as i64);
        self.published.set(&PACKETS_CORRECTED, self.fixed as i64);
        self.published.set(&PACKETS_FAILED, self.failed as i64);
        self.published.set(&SYMBOLS_CORRECTED, self.symbols as i64);
        self.since = 0;
        self.published_at = time;
    }
}

impl Component for RsDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fec.rs_decode",
            vec![
                Port::bytes("in"),
                Port::typed::<Erasures>("erasures")
                    .optional("every packet is decoded with no erasures"),
            ],
            vec![Port::bytes("out"), Port::typed::<FrameStatus>("status")],
        )
        .with_settings(RsDecodeSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let c = self.effective()?;
        self.rs = Some(Rs::new(c).map_err(crate::stage)?);
        self.frame.clear();
        self.frame.reserve(c.n as usize * self.depth);
        self.waiting.clear();
        self.waiting.reserve(MAX_WAITING + 1);
        self.lists.clear();
        self.lists.reserve(MAX_WAITING + 1);
        self.flat.clear();
        self.flat.reserve(MAX_WAITING * 64);
        (self.clean, self.fixed, self.failed, self.symbols) = (0, 0, 0, 0);
        self.published_at = f64::NEG_INFINITY;
        Ok(vec![
            Format::new(sdrkit_graph::Kind::Bytes, 0.0),
            Format::new(sdrkit_graph::Kind::typed::<FrameStatus>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let packets = io.bytes(0);
        if !io.is_connected(1) {
            for p in packets {
                self.decode_one(p, &[], io);
            }
        } else {
            let lists = io.items::<Erasures>(1);
            for i in 0..packets.len().max(lists.len()) {
                if let Some(e) = lists.get(i) {
                    self.push_list(&e.value.0);
                }
                if let Some(p) = packets.get(i) {
                    self.waiting.push_back(p.clone());
                }
                while !self.waiting.is_empty() && !self.lists.is_empty() {
                    let (Some(p), Some((s, l))) =
                        (self.waiting.pop_front(), self.lists.pop_front())
                    else {
                        break;
                    };
                    let er = std::mem::take(&mut self.flat);
                    self.decode_one(&p, &er[s..s + l], io);
                    self.flat = er;
                }
                if self.lists.is_empty() {
                    self.flat.clear();
                }
                if self.waiting.len() > MAX_WAITING {
                    if let Some(p) = self.waiting.pop_front() {
                        self.decode_one(&p, &[], io);
                    }
                }
                if self.lists.len() > MAX_WAITING {
                    self.lists.pop_front();
                }
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RsDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RsDecodeSetting::Code => {
                self.name = CODE.read(id, v)?;
                self.code = self.name.code();
            }
            RsDecodeSetting::Depth => self.depth = DEPTH.read(id, v)? as usize,
            RsDecodeSetting::Basis => self.basis = BASIS.read(id, v)?,
            RsDecodeSetting::Length => self.length = LENGTH.read(id, v)? as usize,
            RsDecodeSetting::KeepParity => self.keep_parity = KEEP_PARITY.read(id, v)?,
            RsDecodeSetting::DropFailed => self.drop_failed = DROP_FAILED.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (RsDecodeSetting::Code, Value::Choice(self.name.index())),
            (RsDecodeSetting::Depth, Value::I64(self.depth as i64)),
            (RsDecodeSetting::Basis, Value::Choice(self.basis.index())),
            (RsDecodeSetting::Length, Value::I64(self.length as i64)),
            (RsDecodeSetting::KeepParity, Value::Bool(self.keep_parity)),
            (RsDecodeSetting::DropFailed, Value::Bool(self.drop_failed)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

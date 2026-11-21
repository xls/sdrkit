//! The table receiver's demodulators on one FM channel: the channel and the discriminator with AFC, the AFSK 1200 and G3RUH 9600 packet modems, FSK slicers at 9600 and 4800 bit/s and at the extra bit
//! rates, whose bits go to HDLC receivers, to the AX100 and CCSDS sync word hunters and to the burst framings of
//! [`super::framings`], the Lucky-7 correlator on the discriminator output, and the audio FSK path of U482C. A spectral
//! carrier follow-up retunes the channel for every path. Two narrow channels (+-4.5 kHz for AFSK 1200 and 4800 bit/s,
//! +-8.5 kHz for 9600 bit/s and U482C) run the same framings beside the wide paths. A named submode runs only its own
//! framing.

use super::framings::{self as fr, ConcatConfig, Decoded, MobitexVariant};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_fec::conv::Viterbi;
use sdrkit_algo_fec::rs::Basis;
use sdrkit_algo_modem::follow::{SpectralConfig, SpectralFollow};
use sdrkit_algo_modem::lock::Gears;
use sdrkit_algo_modem::timing::{SymbolSync, SymbolSyncConfig, ZeroCrossing};
use sdrkit_core::{Complex32, ConfigError, StageError};
use sdrkit_proto_ccsds::channel::{BasisPolicy, ChannelConfig as CcsdsConfig, ChannelDecoder};
use sdrkit_proto_link::csp::{Ax100, AX100_SYNC};
use sdrkit_proto_link::hdlc::{Line, Receiver};
use sdrkit_proto_link::hunt::{Found, Framing as HuntFraming, HuntConfig, Hunter};
use sdrkit_proto_link::modem::{Afsk1200, G3ruh9600};
use sdrkit_util_bits::sync::SyncWord;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The rate of the FM channel, Hz.
pub const RATE: f64 = 48_000.0;
/// The width of the FM channel, Hz.
const CHANNEL_HZ: f64 = 30e3;
/// The most sync word bits that can differ in a match of the AX100 and CCSDS RS hunters.
const SYNC_ERRORS: u32 = 2;
/// Two frames of one burst framing with the same bytes whose ends lie within this many bits are one frame.
const SAME_FRAME_BITS: u64 = 600;
/// Two frames of another framing with the same bytes whose ends lie within this many samples (50 ms) are one frame.
const SAME_FRAME_SAMPLES: u64 = 2400;
/// The narrow channels: pass and stop edges, Hz.
const NARROW_4K5: (f64, f64) = (4500.0, 8100.0);
const NARROW_8K5: (f64, f64) = (8500.0, 13_500.0);
/// The largest deviation of the signals of each narrow channel, Hz: a discriminator value beyond it is a click of the noise.
const LIMIT_4K5: f32 = 3600.0;
const LIMIT_8K5: f32 = 5100.0;
/// The most the correction of the channel moves in one sample, Hz (100 kHz a second: a retune sweeps a narrow filter
/// and does not step it).
const TUNE_SLEW_HZ: f64 = 100e3 / RATE;
/// The loop bandwidths of a slicer's bit clock for each bit, before and after lock (0.15 pulls in on a preamble of 32
/// bits), and the bits its lock test averages.
const CLOCK_GEARS: Gears = Gears {
    pull_in_bn_t: 0.15,
    track_bn_t: 0.02,
};
const CLOCK_LOCK_SYMBOLS: f32 = 48.0;
/// A Mobitex frame from a narrow path needs this many good blocks.
const NARROW_MOBITEX_BLOCKS: usize = 2;

sdrkit_core::choice_enum! {
    /// The framing of a frame.
    #[derive(PartialOrd, Ord, Serialize, Deserialize)]
    pub enum Framing {
        /// AX.25 on Bell 202 AFSK.
        Afsk = ("ax25_afsk", "AX.25 AFSK"),
        /// AX.25 with the G3RUH scrambler.
        G3ruh = ("ax25_g3ruh", "AX.25 G3RUH"),
        /// AX.25 on plain NRZI FSK.
        Nrzi = ("ax25_nrzi", "AX.25 NRZI"),
        /// AX100 Mode 5.
        Mode5 = ("ax100_mode5", "AX100 Mode 5"),
        /// AX100 Mode 6.
        Mode6 = ("ax100_mode6", "AX100 Mode 6"),
        /// A CCSDS Reed-Solomon frame.
        Ccsds = ("ccsds_rs", "CCSDS RS"),
        /// Geoscan.
        Geoscan = ("geoscan", "Geoscan"),
        /// Lucky-7.
        Lucky7 = ("lucky7", "Lucky-7"),
        /// Spino.
        Spino = ("spino", "Spino"),
        /// Erminaz (a CCSDS RS frame).
        Erminaz = ("erminaz", "Erminaz"),
        /// USP.
        Usp = ("usp", "USP"),
        /// NGHam.
        Ngham = ("ngham", "NGHam"),
        /// U482C.
        U482c = ("u482c", "U482C"),
        /// Mobitex.
        Mobitex = ("mobitex", "Mobitex"),
        /// Mobitex-NX.
        MobitexNx = ("mobitex_nx", "Mobitex-NX"),
        /// BEESAT-1.
        Beesat1 = ("beesat1", "BEESAT-1"),
        /// BEESAT-9.
        Beesat9 = ("beesat9", "BEESAT-9"),
        /// CCSDS concatenated.
        CcsdsConcat = ("ccsds_concat", "CCSDS concatenated"),
    }
}

impl Framing {
    /// The framing named by `text`: its key or its label, in any case.
    pub fn parse(text: &str) -> Option<Framing> {
        use sdrkit_core::Choice;
        Framing::CHOICES
            .iter()
            .position(|c| c.key.eq_ignore_ascii_case(text) || c.label.eq_ignore_ascii_case(text))
            .and_then(|i| Framing::from_index(i as u16))
    }
    /// The framing is AX.25.
    pub fn is_ax25(self) -> bool {
        matches!(self, Framing::Afsk | Framing::G3ruh | Framing::Nrzi)
    }
    /// The framing is AX100.
    pub fn is_ax100(self) -> bool {
        matches!(self, Framing::Mode5 | Framing::Mode6)
    }
    /// The framing is one of the burst framings of [`super::framings`] (a missing Reed-Solomon count reads −1 for them).
    pub fn is_burst(self) -> bool {
        self >= Framing::Geoscan
    }
    /// The framing of a Mobitex layout name.
    fn of_mobitex(name: Option<&str>) -> Framing {
        match name {
            Some("Mobitex") => Framing::Mobitex,
            Some("BEESAT-1") => Framing::Beesat1,
            Some("BEESAT-9") => Framing::Beesat9,
            _ => Framing::MobitexNx,
        }
    }
}

/// A submode of the stack: every framing at once, or one framing with its physical-layer defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Submode {
    /// Every framing.
    #[default]
    All,
    /// Geoscan.
    Geoscan,
    /// Lucky-7.
    Lucky7,
    /// Spino.
    Spino,
    /// Erminaz.
    Erminaz,
    /// USP.
    Usp,
    /// NGHam.
    Ngham,
    /// U482C.
    U482c,
    /// Mobitex.
    Mobitex,
    /// CCSDS concatenated.
    CcsdsConcat,
}

impl Submode {
    /// Every named submode with its id.
    pub const NAMED: [(Submode, &'static str); 9] = [
        (Submode::Geoscan, "geoscan"),
        (Submode::Lucky7, "lucky7"),
        (Submode::Spino, "spino"),
        (Submode::Erminaz, "erminaz"),
        (Submode::Usp, "usp"),
        (Submode::Ngham, "ngham"),
        (Submode::U482c, "u482c"),
        (Submode::Mobitex, "mobitex"),
        (Submode::CcsdsConcat, "ccsds_concat"),
    ];

    /// The submode of a stack configuration: `None` for the default, `Some(None)` for an id the stack does not have.
    pub fn of(id: Option<&str>) -> Option<Submode> {
        match id {
            None | Some("") => Some(Submode::All),
            Some(m) => Self::NAMED.iter().find(|(_, k)| *k == m).map(|(s, _)| *s),
        }
    }

    /// The bit rate the submode adds to the fixed slicers, if any.
    pub fn own_bitrate(self) -> Option<u32> {
        (self == Submode::Spino).then_some(2400)
    }

    fn runs(self, s: Submode) -> bool {
        self == Submode::All || self == s
    }
}

sdrkit_core::choice_enum! {
    /// The AX.25 receivers on the extra slicers.
    pub enum ExtraAx25 {
        /// None.
        Off = ("off", "Off"),
        /// G3RUH scrambled.
        G3ruh = ("g3ruh", "G3RUH"),
        /// Plain NRZI.
        Nrzi = ("nrzi", "NRZI"),
    }
}

/// How the demodulators of a channel are set up.
#[derive(Clone, Debug, PartialEq)]
pub struct DemodConfig {
    /// The submode.
    pub submode: Submode,
    /// The CCSDS RS frame length, 33 to 255 bytes.
    pub ccsds_len: usize,
    /// The CCSDS RS basis (CCSDS RS and Erminaz).
    pub ccsds_basis: Basis,
    /// Extra slicers, bit/s.
    pub extra_bitrates: Vec<u32>,
    /// The AX.25 receivers of the extra slicers.
    pub extra_ax25: ExtraAx25,
    /// The U482C audio carrier, Hz (0: the path is off).
    pub afsk_carrier_hz: f64,
    /// The U482C audio bit rate.
    pub afsk_bitrate: u32,
    /// Geoscan frame bytes after the sync word.
    pub geoscan_len: usize,
    /// The CCSDS concatenated link.
    pub concat: ConcatConfig,
    /// The Mobitex variant of the Mobitex-NX sync word.
    pub mobitex_variant: MobitexVariant,
}

impl Default for DemodConfig {
    fn default() -> Self {
        DemodConfig {
            submode: Submode::All,
            ccsds_len: 255,
            ccsds_basis: Basis::Dual,
            extra_bitrates: Vec::new(),
            extra_ax25: ExtraAx25::Off,
            afsk_carrier_hz: 3600.0,
            afsk_bitrate: 4800,
            geoscan_len: 66,
            concat: ConcatConfig::default(),
            mobitex_variant: MobitexVariant::Nx,
        }
    }
}

/// What a demodulator found.
#[derive(Clone, Debug, PartialEq)]
pub struct RawFrame {
    /// The framing.
    pub framing: Framing,
    /// The bit rate.
    pub baud: u32,
    /// AX.25: the frame without its FCS. AX100 and U482C: the CSP packet. CCSDS: the data field. The burst framings: the
    /// bytes of section 4 of each.
    pub data: Vec<u8>,
    /// Reed-Solomon symbols corrected, or `None` for a framing without Reed-Solomon.
    pub rs_corrected: Option<u32>,
    /// The channel sample (at [`RATE`]) where it ended.
    pub at: u64,
    /// What passed ("CRC-16", "Golay + Reed-Solomon" ...); empty for the HDLC framings.
    pub validated: &'static str,
    /// The Lucky-7 kind.
    pub kind: Option<&'static str>,
    /// The sync word was found complemented.
    pub inverted: bool,
    /// The spacecraft id of a CCSDS header.
    pub spacecraft: Option<u16>,
    /// The virtual channel of a CCSDS header.
    pub vc: Option<u8>,
    /// The NGHam flags.
    pub flags: Option<u8>,
    /// The Mobitex callsign.
    pub callsign: Option<String>,
}

impl RawFrame {
    /// A frame with nothing but its framing, rate, bytes, corrections and end.
    pub fn new(framing: Framing, baud: u32, data: Vec<u8>, rs: Option<u32>, at: u64) -> Self {
        RawFrame {
            framing,
            baud,
            data,
            rs_corrected: rs,
            at,
            validated: "",
            kind: None,
            inverted: false,
            spacecraft: None,
            vc: None,
            flags: None,
            callsign: None,
        }
    }

    fn burst(framing: Framing, baud: u32, f: Found<Decoded>, at: u64) -> Self {
        let d = f.frame;
        let framing = if framing == Framing::MobitexNx {
            Framing::of_mobitex(d.kind)
        } else {
            framing
        };
        let ids = matches!(framing, Framing::Erminaz | Framing::CcsdsConcat)
            .then(|| fr::header_ids(&d.data))
            .flatten();
        RawFrame {
            framing,
            baud,
            rs_corrected: d.rs_corrected,
            at,
            validated: d.validated,
            kind: if framing == Framing::Lucky7 {
                d.kind
            } else {
                None
            },
            inverted: f.inverted,
            spacecraft: ids.map(|i| i.0),
            vc: ids.map(|i| i.1),
            flags: d.flags,
            callsign: d.callsign,
            data: d.data,
        }
    }
}

/// The sync matches and failed decodes of one framing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramingCounts {
    /// Sync word matches.
    pub sync_hits: u64,
    /// Candidates that failed their checks.
    pub decode_failed: u64,
}

/// A setting whose value the framing cannot take.
#[derive(Debug)]
struct Refused(&'static str);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} does not fit the code", self.0)
    }
}

impl std::error::Error for Refused {}

/// What a sync word hunter collects after a hit.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hunt {
    Mode5,
    Ccsds,
}

/// The sync word hunters of a raw bit stream (AX100 Mode 5, CCSDS), either polarity.
struct AsmPath {
    ax100: SyncWord,
    ccsds: SyncWord,
    collecting: Option<(Hunt, bool, usize)>,
    flags: u16,
    bytes: Vec<u8>,
    byte: u8,
    nbits: u8,
    ccsds_len: usize,
    /// On a narrow path: an AX100 Mode 5 frame only with Reed-Solomon.
    rs_only: bool,
}

impl AsmPath {
    fn new(ccsds_len: usize, rs_only: bool) -> Self {
        AsmPath {
            ax100: SyncWord::new(AX100_SYNC as u128, 32, SYNC_ERRORS)
                .expect("a 32-bit word")
                .with_inverted(true),
            ccsds: SyncWord::new(sdrkit_proto_ccsds::marker::TM.value(), 32, SYNC_ERRORS)
                .expect("a 32-bit word")
                .with_inverted(true),
            collecting: None,
            flags: 0,
            bytes: Vec::with_capacity(258),
            byte: 0,
            nbits: 0,
            ccsds_len,
            rs_only,
        }
    }

    fn push(&mut self, b: u8, baud: u32, at: u64, codes: &mut Codes, out: &mut Vec<RawFrame>) {
        let a = self.ax100.push(b);
        let c = self.ccsds.push(b);
        let Some((hunt, inverted, need)) = self.collecting else {
            let hit = match (a, c) {
                (Some(h), _) => Some((Hunt::Mode5, h.inverted(), 3)),
                (None, Some(h)) => Some((Hunt::Ccsds, h.inverted(), self.ccsds_len)),
                _ => None,
            };
            if let Some(h) = hit {
                self.collecting = Some(h);
                self.bytes.clear();
                self.byte = 0;
                self.nbits = 0;
            }
            return;
        };
        self.byte = (self.byte << 1) | ((b & 1) ^ inverted as u8);
        self.nbits += 1;
        if self.nbits < 8 {
            return;
        }
        self.bytes.push(self.byte);
        self.byte = 0;
        self.nbits = 0;
        if hunt == Hunt::Mode5 && self.bytes.len() == 3 {
            match codes
                .ax100
                .mode5_length([self.bytes[0], self.bytes[1], self.bytes[2]])
            {
                Ok((len, flags, _))
                    if len > 0
                        && (!self.rs_only || flags & sdrkit_proto_link::csp::FLAG_RS != 0) =>
                {
                    self.flags = flags;
                    self.collecting = Some((hunt, inverted, 3 + len));
                }
                _ => self.collecting = None,
            }
            return;
        }
        if self.bytes.len() < need {
            return;
        }
        self.collecting = None;
        let found = match hunt {
            Hunt::Mode5 => codes
                .ax100
                .mode5_decode(&mut self.bytes[3..], self.flags)
                .ok()
                .map(|(len, n)| (Framing::Mode5, self.bytes[3..3 + len].to_vec(), n)),
            Hunt::Ccsds => codes.ccsds_decode(&mut self.bytes),
        };
        if let Some((framing, data, n)) = found {
            let mut f = RawFrame::new(framing, baud, data, Some(n), at);
            f.inverted = inverted;
            if framing == Framing::Ccsds {
                let ids = fr::header_ids(&f.data);
                f.spacecraft = ids.map(|i| i.0);
                f.vc = ids.map(|i| i.1);
            }
            out.push(f);
        }
    }
}

/// The decoders a channel shares between its slicers.
struct Codes {
    ax100: Ax100,
    ccsds: Option<ChannelDecoder>,
}

impl Codes {
    fn ccsds_decode(&mut self, frame: &mut [u8]) -> Option<(Framing, Vec<u8>, u32)> {
        let d = self.ccsds.as_mut()?;
        let n = d.config().frame_len();
        let r = d.decode(frame).ok()?;
        r.ok()
            .then(|| (Framing::Ccsds, frame[..n].to_vec(), r.corrected))
    }
}

/// A sync word hunter of `bits` bits for `framing`.
fn hunter<F: HuntFraming>(
    word: u128,
    bits: u32,
    errors: u32,
    max_bits: usize,
    framing: F,
) -> Hunter<F> {
    let cfg = HuntConfig {
        word,
        word_bits: bits,
        max_errors: errors,
        complement: true,
        max_open: 32,
        max_frame_bits: max_bits,
    };
    Hunter::new(cfg, framing).expect("a sync word of 16 to 64 bits")
}

/// The CCSDS concatenated decoder of a slicer: one streaming Viterbi decoder for each pairing of the code symbols, each
/// followed by NRZ-M decoding (when on) and a sync word hunter on the decoded bits.
struct ConcatPath {
    nrzm: bool,
    viterbi: [Viterbi; 2],
    hunters: [Hunter<fr::ConcatFraming>; 2],
    batch: [Vec<i8>; 2],
    started: bool,
    bits: Vec<u8>,
    prev: [u8; 2],
    mag: f32,
}

impl ConcatPath {
    fn new(cfg: &ConcatConfig) -> Option<ConcatPath> {
        let max = 8 * cfg.block_bytes();
        let make = || -> Option<Hunter<fr::ConcatFraming>> {
            Some(hunter(
                fr::ASM as u128,
                32,
                4,
                max,
                fr::ConcatFraming::new(cfg)?,
            ))
        };
        let v = || Viterbi::new(cfg.code.code(), 0).ok();
        Some(ConcatPath {
            nrzm: cfg.nrzm,
            viterbi: [v()?, v()?],
            hunters: [make()?, make()?],
            batch: [Vec::with_capacity(64), Vec::with_capacity(64)],
            started: false,
            bits: Vec::with_capacity(64),
            prev: [0; 2],
            mag: 0.0,
        })
    }

    fn push(&mut self, soft: f32, found: &mut Vec<Found<Decoded>>) {
        self.mag += 0.01 * (soft.abs() - self.mag);
        let l = if self.mag > 0.0 {
            (soft / self.mag * 32.0).clamp(-127.0, 127.0) as i8
        } else {
            0
        };
        self.batch[0].push(l);
        if self.started {
            self.batch[1].push(l);
        }
        self.started = true;
        if self.batch[0].len() < 64 {
            return;
        }
        for p in 0..2 {
            self.bits.clear();
            self.viterbi[p].push(&self.batch[p], &mut self.bits);
            self.batch[p].clear();
            for &b in &self.bits {
                let d = if self.nrzm { b ^ self.prev[p] } else { b };
                self.prev[p] = b;
                self.hunters[p].push(if d == 1 { -1.0 } else { 1.0 }, found);
            }
        }
    }

    fn counts(&self) -> FramingCounts {
        let mut c = FramingCounts::default();
        for h in &self.hunters {
            c.sync_hits += h.counts().sync_hits;
            c.decode_failed += h.counts().failed;
        }
        c
    }
}

/// The burst framing hunters of one slicer.
#[derive(Default)]
struct Hunters {
    geoscan: Option<Hunter<fr::GeoscanFraming>>,
    spino: Option<Hunter<fr::SpinoFraming>>,
    erminaz: Option<Hunter<fr::CcsdsRsFraming>>,
    usp: Option<Hunter<fr::UspFraming>>,
    ngham: Option<Hunter<fr::NghamFraming>>,
    u482c: Option<Hunter<fr::U482cFraming>>,
    mobitex: Option<Hunter<fr::MobitexFraming>>,
    mobitex_nx: Option<Hunter<fr::MobitexFraming>>,
    concat: Option<Box<ConcatPath>>,
    found: Vec<Found<Decoded>>,
}

impl Hunters {
    /// The hunters of `cfg` on a slicer; `u482c_only` for the audio FSK path; `narrow` on a narrow path (a Mobitex frame
    /// needs two good blocks there).
    fn new(cfg: &DemodConfig, u482c_only: bool, narrow: bool) -> Result<Hunters, ConfigError> {
        let m = cfg.submode;
        let mut h = Hunters::default();
        if u482c_only {
            h.u482c = Some(hunter(
                fr::U482C_SYNC as u128,
                32,
                4,
                8 * 260,
                fr::U482cFraming::default(),
            ));
            return Ok(h);
        }
        if m.runs(Submode::Geoscan) {
            h.geoscan = Some(hunter(
                fr::GEOSCAN_SYNC as u128,
                32,
                4,
                8 * cfg.geoscan_len,
                fr::GeoscanFraming::new(cfg.geoscan_len),
            ));
        }
        if m.runs(Submode::Spino) {
            h.spino = Some(hunter(
                fr::SPINO_SYNC as u128,
                32,
                0,
                8 * (fr::SPINO_MAX_SIZE + 16),
                fr::SpinoFraming::default(),
            ));
        }
        if m == Submode::Erminaz {
            let f = fr::CcsdsRsFraming::new(cfg.ccsds_len, cfg.ccsds_basis)
                .ok_or_else(|| crate::stage_error(Refused("the CCSDS code length")))?;
            h.erminaz = Some(hunter(fr::ASM as u128, 32, 4, 8 * cfg.ccsds_len, f));
        }
        if m.runs(Submode::Usp) {
            h.usp = Some(hunter(
                fr::USP_SYNC as u128,
                64,
                13,
                64 + 16 * 255,
                fr::UspFraming::default(),
            ));
        }
        if m.runs(Submode::Ngham) {
            h.ngham = Some(hunter(
                fr::NGHAM_SYNC as u128,
                32,
                4,
                24 + 8 * 255,
                fr::NghamFraming::default(),
            ));
        }
        if m.runs(Submode::U482c) {
            h.u482c = Some(hunter(
                fr::U482C_SYNC as u128,
                32,
                4,
                8 * 260,
                fr::U482cFraming::default(),
            ));
        }
        if m.runs(Submode::Mobitex) {
            let max = 8 * 11 + 32 * sdrkit_algo_fec::mobitex::BLOCK_BITS;
            let framing = |plain: bool| {
                let mut f = fr::MobitexFraming::new(plain, cfg.mobitex_variant);
                f.min_good = if narrow { NARROW_MOBITEX_BLOCKS } else { 1 };
                f
            };
            h.mobitex = Some(hunter(fr::MOBITEX_SYNC as u128, 16, 1, max, framing(true)));
            h.mobitex_nx = Some(hunter(
                fr::MOBITEX_NX_SYNC as u128,
                16,
                1,
                max,
                framing(false),
            ));
        }
        // A frame size that does not fit the interleave turns the path off until both settings fit.
        if m.runs(Submode::CcsdsConcat) {
            h.concat = ConcatPath::new(&cfg.concat).map(Box::new);
        }
        Ok(h)
    }

    /// Takes one soft bit (positive for 0) and appends the frames that end on it.
    fn push(&mut self, soft: f32, baud: u32, at: u64, out: &mut Vec<RawFrame>) {
        macro_rules! run {
            ($h:expr, $framing:expr) => {
                if let Some(h) = $h.as_mut() {
                    h.push(soft, &mut self.found);
                    for f in self.found.drain(..) {
                        out.push(RawFrame::burst($framing, baud, f, at));
                    }
                }
            };
        }
        run!(self.geoscan, Framing::Geoscan);
        run!(self.spino, Framing::Spino);
        run!(self.erminaz, Framing::Erminaz);
        run!(self.usp, Framing::Usp);
        run!(self.ngham, Framing::Ngham);
        run!(self.u482c, Framing::U482c);
        run!(self.mobitex, Framing::Mobitex);
        run!(self.mobitex_nx, Framing::MobitexNx);
        if let Some(c) = self.concat.as_mut() {
            c.push(soft, &mut self.found);
            for f in self.found.drain(..) {
                out.push(RawFrame::burst(Framing::CcsdsConcat, baud, f, at));
            }
        }
    }

    /// Adds the counts of every hunter to `out`.
    fn counts(&self, out: &mut Vec<(Framing, FramingCounts)>) {
        fn add(out: &mut Vec<(Framing, FramingCounts)>, f: Framing, c: FramingCounts) {
            match out.iter_mut().find(|x| x.0 == f) {
                Some(x) => {
                    x.1.sync_hits += c.sync_hits;
                    x.1.decode_failed += c.decode_failed;
                }
                None => out.push((f, c)),
            }
        }
        fn of<F: HuntFraming>(h: &Option<Hunter<F>>) -> Option<FramingCounts> {
            h.as_ref().map(|h| FramingCounts {
                sync_hits: h.counts().sync_hits,
                decode_failed: h.counts().failed,
            })
        }
        for (f, c) in [
            (Framing::Geoscan, of(&self.geoscan)),
            (Framing::Spino, of(&self.spino)),
            (Framing::Erminaz, of(&self.erminaz)),
            (Framing::Usp, of(&self.usp)),
            (Framing::Ngham, of(&self.ngham)),
            (Framing::U482c, of(&self.u482c)),
            (Framing::Mobitex, of(&self.mobitex)),
            (Framing::Mobitex, of(&self.mobitex_nx)),
            (
                Framing::CcsdsConcat,
                self.concat.as_ref().map(|c| c.counts()),
            ),
        ] {
            if let Some(c) = c {
                add(out, f, c);
            }
        }
    }
}

/// An FSK slicer at one bit rate with its bit paths: a moving average over one bit, read at the bit instants of a
/// zero-crossing timing loop.
struct Slicer {
    baud: u32,
    clock: SymbolSync<f32, ZeroCrossing, Cubic>,
    symbols: Vec<f32>,
    ma: Vec<f32>,
    pos: usize,
    sum: f32,
    nrzi: Option<(Receiver, Framing)>,
    g3ruh: Option<Receiver>,
    /// The G3RUH receiver of AX100 Mode 6, which also reports the frames whose FCS fails, and the bytes of one.
    mode6: Option<Receiver>,
    payload: Vec<u8>,
    asm: Option<AsmPath>,
    hunters: Hunters,
}

impl Slicer {
    /// Adds the AX100 Mode 6 receiver: a G3RUH HDLC frame whose FCS fails is taken when its bytes, with the two read as the
    /// FCS, are a Reed-Solomon code word.
    fn with_mode6(mut self) -> Self {
        self.mode6 = Some(Receiver::ax25(Line::G3ruh).deliver_bad(true));
        self
    }

    /// A slicer at `baud`, with an AX.25 NRZI receiver (reported as `nrzi`), a G3RUH receiver, the AX100 and CCSDS RS hunters
    /// when `asm` is set (with the CCSDS length, and whether AX100 Mode 5 needs Reed-Solomon), and the burst hunters `hunters`.
    fn new(
        baud: u32,
        nrzi: Option<Framing>,
        g3ruh: bool,
        asm: Option<(usize, bool)>,
        hunters: Hunters,
    ) -> Result<Self, ConfigError> {
        let n = ((RATE / baud as f64).round() as usize).max(1);
        let mut clock = SymbolSyncConfig::new(RATE / f64::from(baud));
        clock.gears = Some(CLOCK_GEARS);
        clock.lock_symbols = CLOCK_LOCK_SYMBOLS;
        // A rectangle through the moving average is a triangle; the raised cosine of roll-off 1 is the nearest shape.
        clock.pulse = Shape::Rc { beta: 1.0 };
        Ok(Slicer {
            baud,
            clock: SymbolSync::new(ZeroCrossing, Cubic, clock)
                .map_err(|e| ConfigError::Stage(StageError::new(e)))?,
            symbols: Vec::with_capacity(2),
            ma: vec![0.0; n],
            pos: 0,
            sum: 0.0,
            nrzi: nrzi.map(|f| (Receiver::ax25(Line::Nrzi), f)),
            g3ruh: g3ruh.then(|| Receiver::ax25(Line::G3ruh)),
            mode6: None,
            payload: Vec::with_capacity(256),
            asm: asm.map(|(len, rs_only)| AsmPath::new(len, rs_only)),
            hunters,
        })
    }

    /// Takes one discriminator sample (Hz, a positive value is a 1) and appends the frames that end on it.
    fn push(&mut self, a: f32, at: u64, codes: &mut Codes, out: &mut Vec<RawFrame>) {
        // A moving average over one bit.
        self.sum += a - self.ma[self.pos];
        self.ma[self.pos] = a;
        self.pos = (self.pos + 1) % self.ma.len();
        self.symbols.clear();
        self.clock
            .process(std::slice::from_ref(&self.sum), &mut self.symbols);
        for i in 0..self.symbols.len() {
            let soft = self.symbols[i];
            let bit = u8::from(soft > 0.0);
            if let Some((rx, framing)) = &mut self.nrzi {
                if rx.push(bit).is_some() {
                    push_ax25(out, *framing, self.baud, rx.frame(), at);
                }
            }
            if let Some(g) = &mut self.g3ruh {
                if g.push(bit).is_some() {
                    push_ax25(out, Framing::G3ruh, self.baud, g.frame(), at);
                }
            }
            if let Some(rx) = &mut self.mode6 {
                if rx.push(bit).is_some_and(|end| !end.fcs_ok) {
                    self.payload.clear();
                    self.payload.extend_from_slice(rx.frame_with_fcs());
                    if let Ok((len, n)) = codes.ax100.mode6_decode(&mut self.payload) {
                        let mut f = RawFrame::new(
                            Framing::Mode6,
                            self.baud,
                            self.payload[..len].to_vec(),
                            Some(n),
                            at,
                        );
                        f.validated = "Reed-Solomon";
                        out.push(f);
                    }
                }
            }
            if let Some(a) = &mut self.asm {
                a.push(bit, self.baud, at, codes, out);
            }
            // The hunters take soft bits positive for 0.
            self.hunters.push(-soft, self.baud, at, out);
        }
    }
}

/// The Lucky-7 receiver: the discriminator output correlated on every sample with the 16 sync symbols, and the 312 symbols
/// of a frame taken at the nominal rate from each start found.
struct Lucky7 {
    sps: usize,
    ring: Vec<f32>,
    sums: Vec<(f32, f32)>,
    mask: usize,
    n: u64,
    sum: f32,
    abs: f32,
    pending: Vec<(u64, bool)>,
    was_candidate: bool,
    quiet_until: u64,
    counts: FramingCounts,
}

/// The correlation over the magnitude a Lucky-7 sync word needs.
const LUCKY7_CORR: f32 = 0.72;
/// Symbols of a Lucky-7 frame with its sync word.
const LUCKY7_SYMBOLS: usize = 312;

impl Lucky7 {
    fn new() -> Self {
        let sps = (RATE / 4800.0).round() as usize;
        let size = (2 * (LUCKY7_SYMBOLS + 32) * sps).next_power_of_two();
        Lucky7 {
            sps,
            ring: vec![0.0; size],
            sums: vec![(0.0, 0.0); size],
            mask: size - 1,
            n: 0,
            sum: 0.0,
            abs: 0.0,
            pending: Vec::with_capacity(32),
            was_candidate: false,
            quiet_until: 0,
            counts: FramingCounts::default(),
        }
    }

    fn push(&mut self, a: f32, at: u64, out: &mut Vec<RawFrame>) {
        let m = self.mask;
        let sps = self.sps as u64;
        let old = if self.n >= sps {
            self.ring[(self.n - sps) as usize & m]
        } else {
            0.0
        };
        self.ring[self.n as usize & m] = a;
        self.sum += a - old;
        self.abs += a.abs() - old.abs();
        self.sums[self.n as usize & m] = (self.sum, self.abs);
        self.n += 1;
        if self.n < 16 * sps {
            return;
        }
        let (mut corr, mut mag) = (0f32, 0f32);
        for k in 0..16u64 {
            let (s, ab) = self.sums[(self.n - 1 - (15 - k) * sps) as usize & m];
            if (fr::LUCKY7_SYNC >> (15 - k)) & 1 == 1 {
                corr += s;
            } else {
                corr -= s;
            }
            mag += ab;
        }
        let candidate = mag > 0.0 && corr.abs() >= LUCKY7_CORR * mag;
        if candidate && !self.was_candidate {
            self.counts.sync_hits += 1;
        }
        self.was_candidate = candidate;
        let start = self.n - 16 * sps;
        if candidate && start >= self.quiet_until && self.pending.len() < 32 {
            self.pending.push((start, corr < 0.0));
        }
        let span = LUCKY7_SYMBOLS as u64 * sps;
        let mut i = 0;
        while i < self.pending.len() {
            let (s0, inv) = self.pending[i];
            if self.n < s0 + span {
                i += 1;
                continue;
            }
            self.pending.swap_remove(i);
            if s0 < self.quiet_until {
                continue;
            }
            match self.frame(s0, inv) {
                Some(p) => {
                    self.quiet_until = s0 + span;
                    self.pending.retain(|x| x.0 >= s0 + span);
                    let mut f = RawFrame::new(Framing::Lucky7, 4800, p.to_vec(), None, at);
                    f.kind = Some(fr::lucky7_kind(&p));
                    f.validated = "CRC-16";
                    f.inverted = inv;
                    out.push(f);
                    i = 0;
                }
                None => self.counts.decode_failed += 1,
            }
        }
    }

    /// The payload of the frame that starts at sample `s0`, when its CRC passes.
    fn frame(&self, s0: u64, inverted: bool) -> Option<[u8; 35]> {
        let sps = self.sps as u64;
        let mut v = [0f32; LUCKY7_SYMBOLS];
        for (k, o) in v.iter_mut().enumerate() {
            let from = s0 + k as u64 * sps;
            *o = (from..from + sps)
                .map(|t| self.ring[t as usize & self.mask])
                .sum::<f32>();
        }
        let mean = v.iter().sum::<f32>() / LUCKY7_SYMBOLS as f32;
        let mut bytes = [0u8; LUCKY7_SYMBOLS / 8];
        for (k, x) in v.iter().enumerate() {
            let bit = u8::from(*x > mean) ^ u8::from(inverted);
            bytes[k / 8] |= bit << (7 - k % 8);
        }
        fr::lucky7_decode(&bytes[2..])
    }
}

/// The U482C audio FSK path: the discriminator output mixed down by the audio carrier, filtered, discriminated again, and
/// sliced at the audio bit rate into the U482C hunter.
struct AudioFsk {
    nco: Nco,
    lpf: Fir<Complex32>,
    disc: Discriminator,
    slicer: Slicer,
}

impl AudioFsk {
    fn new(cfg: &DemodConfig) -> Result<Self, ConfigError> {
        let fc = cfg.afsk_carrier_hz;
        let br = f64::from(cfg.afsk_bitrate);
        let pass = (0.9 * fc).min(0.75 * br + 600.0);
        let stop = (1.6 * fc).min(pass + 0.75 * br + 1500.0);
        let taps = lowpass(RATE, pass, stop.max(pass + 200.0), 40.0)?;
        Ok(AudioFsk {
            nco: Nco::new(RATE, -fc),
            lpf: Fir::new(&taps),
            disc: Discriminator::new(RATE, 1.0)?,
            slicer: Slicer::new(
                cfg.afsk_bitrate,
                None,
                false,
                None,
                Hunters::new(cfg, true, false)?,
            )?,
        })
    }

    fn push(&mut self, a: f32, at: u64, codes: &mut Codes, out: &mut Vec<RawFrame>) {
        let z = self.nco.next_sample() * a;
        let y = self.lpf.process_sample(z);
        let f = self.disc.process_sample(y);
        self.slicer.push(f, at, codes, out);
    }
}

/// A narrow channel filter on the retuned channel, with its own discriminator (the slow mean removed, clicks limited) and the paths
/// that read it. Its failed frames are not counted.
struct NarrowPath {
    lpf: Fir<Complex32>,
    /// The discriminator's output is held within this, Hz.
    limit: f32,
    disc: Discriminator,
    afc: f32,
    iq: Vec<Complex32>,
    audio: Vec<f32>,
    afsk: Option<Afsk1200>,
    g3ruh: Option<G3ruh9600>,
    slicer: Slicer,
    audio_fsk: Option<AudioFsk>,
}

impl NarrowPath {
    /// A path behind the low-pass `(pass, stop)` (Hz) with these demodulators, its discriminator held within `limit` Hz.
    fn new(
        (pass, stop): (f64, f64),
        limit: f32,
        afsk: Option<Afsk1200>,
        g3ruh: Option<G3ruh9600>,
        slicer: Slicer,
        audio_fsk: Option<AudioFsk>,
    ) -> Result<Self, ConfigError> {
        Ok(NarrowPath {
            lpf: Fir::new(&lowpass(RATE, pass, stop, 50.0)?),
            limit,
            disc: Discriminator::new(RATE, 1.0)?,
            afc: 0.0,
            iq: Vec::new(),
            audio: Vec::new(),
            afsk,
            g3ruh,
            slicer,
            audio_fsk,
        })
    }

    /// Takes the channel's samples with the retune of each (`first` the channel sample before them) and appends the frames
    /// that end in them.
    fn process(
        &mut self,
        x: &[Complex32],
        tuned: &[f32],
        first: u64,
        codes: &mut Codes,
        out: &mut Vec<RawFrame>,
    ) {
        self.iq.clear();
        self.iq
            .extend(x.iter().map(|&v| self.lpf.process_sample(v)));
        self.audio.clear();
        self.audio.resize(self.iq.len(), 0.0);
        self.disc.process_into(&self.iq, &mut self.audio);
        for (a, t) in self.audio.iter_mut().zip(tuned) {
            *a += t;
            self.afc += 0.0002 * (*a - self.afc);
            // A value beyond the largest deviation is a click of the noise.
            *a = (*a - self.afc).clamp(-self.limit, self.limit);
        }
        if let Some(m) = &mut self.afsk {
            m.process(&self.audio, |f| {
                push_ax25(out, Framing::Afsk, 1200, f.bytes, first + f.at);
            });
        }
        if let Some(m) = &mut self.g3ruh {
            m.process(&self.audio, |f| {
                push_ax25(out, Framing::G3ruh, 9600, f.bytes, first + f.at);
            });
        }
        for (i, &a) in self.audio.iter().enumerate() {
            let at = first + i as u64 + 1;
            self.slicer.push(a, at, codes, out);
            if let Some(p) = &mut self.audio_fsk {
                p.push(a, at, codes, out);
            }
        }
    }
}

/// The table receiver's demodulators on one FM channel.
pub struct FmDemods {
    chan: Channelizer,
    disc: Discriminator,
    iq: Vec<Complex32>,
    audio: Vec<f32>,
    afc: f32,
    /// Channel samples so far.
    pub n: u64,
    afsk: Option<Afsk1200>,
    g3ruh: Option<G3ruh9600>,
    slicers: Vec<Slicer>,
    lucky7: Option<Lucky7>,
    audio_fsk: Option<AudioFsk>,
    /// The carrier follow-up, and the copy of the channel it measures on (turned by its own correction, which moves in
    /// steps).
    follow: SpectralFollow,
    probe: Vec<Complex32>,
    /// The correction on the channel that every path reads (Hz): it slews to the follow-up's. The mixer's phase, and the
    /// correction of each sample of the channel.
    tune_hz: f64,
    tune_phase: f64,
    tuned: Vec<f32>,
    /// The narrow channels and their paths.
    narrow: Vec<NarrowPath>,
    codes: Codes,
    found: Vec<RawFrame>,
    recent: VecDeque<(Framing, u64, Vec<u8>)>,
    /// The channel's offset from where it was placed (the mean frequency before the retune), Hz.
    pub offset_hz: f64,
    /// The mean power of the channel.
    pub level: f64,
}

impl FmDemods {
    /// The demodulators of a channel at `offset` Hz in an input at `rate_in`, set up by `cfg`.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] when the rate cannot hold the channel, the CCSDS code refuses the length or a bit rate is too high
    /// for the channel rate.
    pub fn new(rate_in: f64, offset: f64, cfg: &DemodConfig) -> Result<Self, ConfigError> {
        let mut ccfg = ChannelConfig::new(rate_in, offset, CHANNEL_HZ, RATE);
        ccfg.exact_rate = true;
        let chan = Channelizer::new(ccfg)?;
        let fill = 255usize.saturating_sub(cfg.ccsds_len);
        let ccsds = ChannelDecoder::new(
            CcsdsConfig::rs_223(1)
                .with_virtual_fill(fill)
                .with_basis(BasisPolicy::Fixed(cfg.ccsds_basis)),
        )
        .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
        let all = cfg.submode == Submode::All;
        let asm = all.then_some((cfg.ccsds_len, false));
        let wide_9600 = Slicer::new(
            9600,
            all.then_some(Framing::Nrzi),
            false,
            asm,
            Hunters::new(cfg, false, false)?,
        )?;
        let mut slicers = vec![
            // AX100 Mode 6 is read on the wide path only.
            if all {
                wide_9600.with_mode6()
            } else {
                wide_9600
            },
            Slicer::new(
                4800,
                all.then_some(Framing::Nrzi),
                all,
                asm,
                Hunters::new(cfg, false, false)?,
            )?,
        ];
        let mut extra: Vec<u32> = cfg.extra_bitrates.clone();
        extra.extend(cfg.submode.own_bitrate());
        for b in extra {
            if slicers.iter().any(|s| s.baud == b) || slicers.len() >= 6 {
                continue;
            }
            let (nrzi, g3ruh) = match cfg.extra_ax25 {
                ExtraAx25::Off => (None, false),
                ExtraAx25::G3ruh => (None, true),
                ExtraAx25::Nrzi => (Some(Framing::Nrzi), false),
            };
            slicers.push(Slicer::new(
                b,
                nrzi,
                g3ruh,
                None,
                Hunters::new(cfg, false, false)?,
            )?);
        }
        let audio_fsk = (cfg.submode.runs(Submode::U482c) && cfg.afsk_carrier_hz > 0.0)
            .then(|| AudioFsk::new(cfg))
            .transpose()?;
        let narrow_asm = all.then_some((cfg.ccsds_len, true));
        let narrow = vec![
            NarrowPath::new(
                NARROW_4K5,
                LIMIT_4K5,
                all.then(|| Afsk1200::new(RATE)).transpose()?,
                None,
                Slicer::new(
                    4800,
                    None,
                    false,
                    narrow_asm,
                    Hunters::new(cfg, false, true)?,
                )?,
                None,
            )?,
            NarrowPath::new(
                NARROW_8K5,
                LIMIT_8K5,
                None,
                all.then(|| G3ruh9600::new(RATE)).transpose()?,
                Slicer::new(
                    9600,
                    None,
                    false,
                    narrow_asm,
                    Hunters::new(cfg, false, true)?,
                )?,
                (cfg.submode.runs(Submode::U482c) && cfg.afsk_carrier_hz > 0.0)
                    .then(|| AudioFsk::new(cfg))
                    .transpose()?,
            )?,
        ];
        Ok(FmDemods {
            chan,
            // A deviation of 1 Hz makes the discriminator's output read in Hz.
            disc: Discriminator::new(RATE, 1.0)?,
            iq: Vec::new(),
            audio: Vec::new(),
            afc: 0.0,
            n: 0,
            afsk: all.then(|| Afsk1200::new(RATE)).transpose()?,
            g3ruh: all.then(|| G3ruh9600::new(RATE)).transpose()?,
            slicers,
            lucky7: cfg.submode.runs(Submode::Lucky7).then(Lucky7::new),
            audio_fsk,
            follow: SpectralFollow::new(SpectralConfig::closed_loop(RATE))
                .map_err(|e| ConfigError::Stage(StageError::new(e)))?,
            probe: Vec::new(),
            tune_hz: 0.0,
            tune_phase: 0.0,
            tuned: Vec::new(),
            narrow,
            codes: Codes {
                ax100: Ax100::new()?,
                ccsds: Some(ccsds),
            },
            found: Vec::new(),
            recent: VecDeque::with_capacity(16),
            offset_hz: 0.0,
            level: 0.0,
        })
    }

    /// The sync matches and failed decodes of each burst framing so far.
    pub fn counts(&self, out: &mut Vec<(Framing, FramingCounts)>) {
        for s in &self.slicers {
            s.hunters.counts(out);
        }
        if let Some(a) = &self.audio_fsk {
            a.slicer.hunters.counts(out);
        }
        if let Some(l) = &self.lucky7 {
            out.push((Framing::Lucky7, l.counts));
        }
    }

    /// Demodulates input samples and appends the frames that end in them.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<RawFrame>) {
        self.iq.clear();
        self.chan.process(x, &mut self.iq);
        // Every path reads the channel retuned by the follow-up. The correction slews to the follow-up's value, and each
        // discriminator adds it back, so a retune does not move the frequencies a path reads.
        self.probe.clear();
        self.probe.extend_from_slice(&self.iq);
        self.follow.process(&mut self.probe);
        let target = self.follow.correction_hz();
        self.tuned.clear();
        for v in self.iq.iter_mut() {
            self.tune_hz += (target - self.tune_hz).clamp(-TUNE_SLEW_HZ, TUNE_SLEW_HZ);
            self.tune_phase = (self.tune_phase - std::f64::consts::TAU * self.tune_hz / RATE)
                % std::f64::consts::TAU;
            let (s, c) = self.tune_phase.sin_cos();
            *v *= Complex32::new(c as f32, s as f32);
            self.tuned.push(self.tune_hz as f32);
        }
        self.audio.clear();
        self.audio.resize(self.iq.len(), 0.0);
        self.disc.process_into(&self.iq, &mut self.audio);
        for ((a, s), t) in self.audio.iter_mut().zip(&self.iq).zip(&self.tuned) {
            self.level += 0.0005 * (s.norm_sqr() as f64 - self.level);
            // AFC: a slow mean of the frequency before the retune is the carrier's offset left after the dial.
            *a += t;
            self.afc += 0.0002 * (*a - self.afc);
            *a -= self.afc;
        }
        let first = self.n;
        self.found.clear();
        let found = &mut self.found;
        if let Some(m) = &mut self.afsk {
            m.process(&self.audio, |f| {
                push_ax25(found, Framing::Afsk, 1200, f.bytes, first + f.at);
            });
        }
        if let Some(m) = &mut self.g3ruh {
            m.process(&self.audio, |f| {
                push_ax25(found, Framing::G3ruh, 9600, f.bytes, first + f.at);
            });
        }
        for &a in &self.audio {
            self.n += 1;
            let at = self.n;
            for s in &mut self.slicers {
                s.push(a, at, &mut self.codes, &mut self.found);
            }
            if let Some(l) = &mut self.lucky7 {
                l.push(a, at, &mut self.found);
            }
            if let Some(p) = &mut self.audio_fsk {
                p.push(a, at, &mut self.codes, &mut self.found);
            }
        }
        for p in &mut self.narrow {
            p.process(&self.iq, &self.tuned, first, &mut self.codes, &mut self.found);
        }
        // The frames in the order they ended (the narrow paths add theirs after the wide ones).
        self.found.sort_by_key(|f| f.at);
        for f in self.found.drain(..) {
            // The same bytes from two candidates, two slicers or two paths close together are one frame.
            let window = if f.framing.is_burst() {
                SAME_FRAME_BITS * RATE as u64 / u64::from(f.baud.max(1))
            } else {
                SAME_FRAME_SAMPLES
            };
            let dup = self
                .recent
                .iter()
                .any(|(g, at, d)| *g == f.framing && f.at.abs_diff(*at) < window && *d == f.data);
            if dup {
                continue;
            }
            if self.recent.len() == 16 {
                self.recent.pop_front();
            }
            self.recent.push_back((f.framing, f.at, f.data.clone()));
            out.push(f);
        }
        self.offset_hz = self.afc as f64;
    }
}

/// Adds an HDLC frame that is an AX.25 frame.
fn push_ax25(out: &mut Vec<RawFrame>, framing: Framing, baud: u32, bytes: &[u8], at: u64) {
    if sdrkit_proto_link::ax25::Frame::parse(bytes).is_ok() {
        out.push(RawFrame::new(framing, baud, bytes.to_vec(), None, at));
    }
}

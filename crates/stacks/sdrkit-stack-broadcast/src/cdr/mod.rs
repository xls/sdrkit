//! CDR, China Digital Radio, in the FM band. The LDPC matrices and the bit interleavers of GY/T 268.1 have no free source.
//! The receiver and the generator load them from a file at run time (setting `cdr_tables`) and decode nothing without them.
//!
//! Standard: GY/T 268.1-2013 (physical layer) and GY/T 268.2-2013 (multiplex).

pub mod mux;
pub mod phy;
pub mod results;
pub mod tables;

use crate::component::ClearCommand;
use crate::datafile::{FileError, Loaded};
use crate::qam::Qam;
use mux::{AudioStream, ControlFrame, ServiceFrame, SmctEntry, SubFrame};
use phy::{SpectrumMode, SubframeCells, SystemInfo, TxMode, RATE};
use results as res;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_fec::conv::{ConvCode, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::ldpc::{LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_modem::ofdm::{CellFrame, FrameDemodulator, FrameModulator, FrameState};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::Hertz;
use sdrkit_core::media::{AudioProps, Codec, MediaFormat, Packaging, Packet};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, LockState, Receiver, Record, Results, SeriesId, Setting, SettingError, SettingId,
    SettingSet, SignalInfo, StackConfig, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Kept, Kind, MediaPacket, Port, RateFrom, RateRule, StackGraph, Timed,
};
use sdrkit_proto_mpegts::publish::{SERVICE_ROW, SERVICE_SELECT_KIND};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::seq;
use sdrkit_util_bits::{BitOrder, Pad};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;
use tables::{gather, scatter, CdrTables, LdpcRate, TablesError, N as LDPC_N, SI_BITS};

/// The stack id.
pub const ID: &str = "cdr";
/// The minimum input rate.
pub const MIN_RATE: f64 = 768e3;

/// The rate 1/4 convolutional code of the SI and the SDIS (GY/T 268.1 table 4.3).
const SI_CODE: ConvCode = ConvCode::new(7, &[0o133, 0o171, 0o145, 0o133]);

/// Information bits of the system information.
const SI_INFO: usize = 48;

/// The text of the `tables` field without tables.
pub const NO_TABLES: &str =
    "not loaded: set cdr_tables to a file with the GY/T 268.1 LDPC matrices and interleavers";

/// The tables as a component holds them.
pub type LoadedTables = Loaded<CdrTables, TablesError>;

impl FileError for TablesError {
    fn read_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            TablesError::Read { kind } => Some(*kind),
            _ => None,
        }
    }
}

/// Loads the tables at `path`: `Ok(None)` for an empty path.
///
/// # Errors
///
/// [`TablesError`] if the file cannot be read or is refused.
pub fn load_tables(path: &str) -> Result<Option<Arc<CdrTables>>, TablesError> {
    Loaded::load(path, CdrTables::load).value().clone()
}

/// The tables file of a component.
const COMPONENT_TABLES: Setting<field::Text> = Setting::new("tables", "GY/T 268.1 tables", "", "")
    .help("A JSON file with the LDPC matrices and the bit interleavers of GY/T 268.1.");

sdrkit_core::settings! {
    /// The settings of [`CdrOfdm`] and [`CdrDecoder`].
    pub enum TablesSetting {
        /// The tables file.
        Tables = COMPONENT_TABLES,
    }
}

/// Which of the `steps * n` mother code bits are sent, `keep` of them spread evenly (the rate matching of the SI and SDIS).
fn rate_mask(steps: usize, n: usize, keep: usize) -> Vec<u8> {
    let mut m = vec![0u8; steps * n];
    for i in 0..steps {
        let a = (i * keep) / steps;
        let b = ((i + 1) * keep) / steps;
        for j in 0..(b - a).min(n) {
            m[i * n + j] = 1;
        }
    }
    m
}

/// The convolutional channel of the SI and the SDIS: rate 1/4 with 6 tail bits, rate-matched to `coded` bits, then interleaved
/// by a destination table of the tables file.
#[derive(Clone, Debug)]
pub struct ConvChannel {
    info: usize,
    coded: usize,
    mask: Vec<u8>,
    dest: Vec<u32>,
    vit: Viterbi,
    quant: Quantizer,
    mother: Vec<f32>,
    llr: Vec<Llr8>,
}

impl ConvChannel {
    /// A channel of `info` bits into `coded` bits with the interleaver `dest`; `None` when `dest` is not `coded` long.
    pub fn new(info: usize, coded: usize, dest: &[u32]) -> Option<ConvChannel> {
        let steps = info + 6;
        if dest.len() != coded {
            return None;
        }
        Some(ConvChannel {
            info,
            coded,
            mask: rate_mask(steps, 4, coded.min(steps * 4)),
            dest: dest.to_vec(),
            vit: Viterbi::new(SI_CODE, steps).ok()?,
            quant: Quantizer::default(),
            mother: vec![0.0; steps * 4],
            llr: vec![0; steps * 4],
        })
    }
    /// Encodes the first `info` bits of `bits` into `coded` bits in sending order.
    pub fn encode(&self, bits: &[u8]) -> Vec<u8> {
        let mut x = bits[..self.info.min(bits.len())].to_vec();
        x.resize(self.info + 6, 0);
        let mut m = Vec::with_capacity(x.len() * 4);
        Encoder::new(SI_CODE).encode(&x, &mut m);
        let mut p: Vec<u8> = m
            .iter()
            .zip(&self.mask)
            .filter(|(_, &k)| k == 1)
            .map(|(&b, _)| b)
            .collect();
        p.resize(self.coded, 0);
        let mut out = vec![0u8; self.coded];
        scatter(&self.dest, &p, &mut out);
        out
    }
    /// Adds the LLRs `soft` (sending order) to `acc` in coded order, for the copies to be summed.
    pub fn deinterleave_add(&self, soft: &[f32], acc: &mut [f32]) {
        for (a, &d) in acc.iter_mut().zip(&self.dest) {
            *a += soft.get(d as usize).copied().unwrap_or(0.0);
        }
    }
    /// Decodes LLRs in coded order (positive means 0) into the information bits.
    pub fn decode(&mut self, p: &[f32], out: &mut Vec<u8>) -> bool {
        self.mother.fill(0.0);
        let mut it = p.iter();
        for (m, &k) in self.mother.iter_mut().zip(&self.mask) {
            if k == 1 {
                *m = it.next().copied().unwrap_or(0.0);
            }
        }
        out.clear();
        out.resize(self.info, 0);
        self.quant
            .quantize_auto(&self.mother, &mut self.llr)
            .is_ok()
            && self.vit.decode(&self.llr, Ends::Zero, out).is_ok()
    }
}

/// The logical frame plan of a configuration.
#[derive(Clone, Debug)]
pub struct Plan {
    /// SDIS cells per logical frame.
    pub sdis_cells: usize,
    /// MSDS cells per logical frame.
    pub msds_cells: usize,
    /// SDI information bits.
    pub sdi_bits: usize,
    /// Coded SDIS bits.
    pub sdis_bits: usize,
    /// LDPC code words per logical frame.
    pub codewords: usize,
    /// Service multiplex frame bytes per logical frame.
    pub msds_bytes: usize,
}

/// The plan.
pub fn plan(t: TxMode, m: SpectrumMode, sdi: Qam, data: Qam, rate: LdpcRate) -> Plan {
    let ni = m.ni();
    let msds_cells = t.msds_cells() * ni;
    let codewords = msds_cells * data.bits() / LDPC_N;
    let sdis_cells = t.sdis_cells() * ni;
    Plan {
        sdis_cells,
        msds_cells,
        sdi_bits: t.sdi_q(sdi) * ni / 8 * 8,
        sdis_bits: sdis_cells * sdi.bits(),
        codewords,
        msds_bytes: codewords * rate.k() / 8,
    }
}

/// The transmitter's configuration.
#[derive(Clone, Debug)]
pub struct TxConfig {
    /// Transmission mode.
    pub tx: TxMode,
    /// Spectrum mode.
    pub spectrum: SpectrumMode,
    /// Service data constellation.
    pub data: Qam,
    /// LDPC rate.
    pub rate: LdpcRate,
    /// SDI constellation.
    pub sdi: Qam,
    /// Allocation mode 1..3.
    pub allocation: u8,
    /// The text data service.
    pub text: bool,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            tx: TxMode(1),
            spectrum: SpectrumMode(1),
            data: Qam::Q4,
            rate: LdpcRate::R3_4,
            sdi: Qam::Q4,
            allocation: 1,
            text: true,
        }
    }
}

/// The text the data service sends.
pub const TEST_TEXT: &str = "CDR test signal. CDR sound uses the DRA+ codec.";

fn qam_index(q: Qam) -> u8 {
    (q.bits() / 2 - 1) as u8
}

/// A DRA+ unit of filler bytes (DRA+ is not openly specified; sdrkit has no encoder).
pub fn dra_unit(service: u16, n: u64, len: usize) -> Vec<u8> {
    let mut u = service.to_be_bytes().to_vec();
    u.extend_from_slice(&(n as u32).to_be_bytes());
    let mut r = Rng::new(n ^ (service as u64) << 32);
    while u.len() < len {
        u.push(r.next_u64() as u8);
    }
    u.truncate(len);
    u
}

/// XORs bits with the CDR scrambler, started again for each call.
fn scramble(bits: &mut [u8]) {
    Additive::new(seq::CDR).apply_bits(bits);
}

/// A random QPSK cell of unit power (dummy cells).
fn dummy(r: &mut Rng) -> Complex32 {
    let b = r.next_u64();
    let a = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(
        if b & 1 == 0 { a } else { -a },
        if b & 2 == 0 { a } else { -a },
    )
}

/// The coders of a configuration, made from the tables.
struct Coders {
    si: ConvChannel,
    sdc: ConvChannel,
    code: Option<sdrkit_algo_fec::ldpc::LdpcCode>,
    msds: Vec<u32>,
}

/// The CDR transmitter at 816 ksps.
pub struct CdrTransmitter {
    cfg: TxConfig,
    plan: Plan,
    cells: SubframeCells,
    modu: FrameModulator,
    coders: Option<Coders>,
    note: String,
    history: VecDeque<Vec<Complex32>>,
    rng: Rng,
    lf: u64,
    units: [u64; 2],
    k: usize,
}

impl CdrTransmitter {
    /// A transmitter. Without `tables`, or when they lack what this configuration needs, it sends the beacon, the pilots and
    /// random cells only ([`CdrTransmitter::note`] says why).
    pub fn new(cfg: TxConfig, tables: Option<&CdrTables>) -> CdrTransmitter {
        let plan = plan(cfg.tx, cfg.spectrum, cfg.sdi, cfg.data, cfg.rate);
        let cells = phy::subframe_cells(cfg.tx, cfg.spectrum);
        let l = phy::layout(cfg.tx, true);
        let k = l.ncarriers();
        let (coders, note) = match tables {
            None => (None, NO_TABLES.to_string()),
            Some(t) => match t.missing(plan.sdis_bits, cfg.rate) {
                Some(m) => (None, format!("the tables lack {m}")),
                None => {
                    // The LDPC encoder of sdrkit-algo-fec is dense and stops at 4096 information or parity bits: a code word of
                    // 9216 bits has none, so the service data cells are random.
                    let code = t.code(cfg.rate).filter(|c| c.can_encode()).cloned();
                    let note = if code.is_none() {
                        format!(
                            "the LDPC code of rate {} has no encoder in sdrkit-algo-fec; the service data cells are random",
                            cfg.rate.text()
                        )
                    } else {
                        String::new()
                    };
                    let coders = (|| {
                        Some(Coders {
                            si: ConvChannel::new(SI_INFO, SI_BITS, t.si())?,
                            sdc: ConvChannel::new(
                                plan.sdi_bits,
                                plan.sdis_bits,
                                t.sdis(plan.sdis_bits)?,
                            )?,
                            code,
                            msds: t.msds().to_vec(),
                        })
                    })();
                    (coders, note)
                }
            },
        };
        CdrTransmitter {
            modu: FrameModulator::new(l),
            history: VecDeque::new(),
            rng: Rng::new(0xD0D0),
            lf: 0,
            units: [0; 2],
            k,
            cells,
            plan,
            cfg,
            coders,
            note,
        }
    }
    /// The plan.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    /// Why the transmitter sends no coded data, or empty when it does.
    pub fn note(&self) -> &str {
        &self.note
    }
    fn control(&self) -> Vec<u8> {
        let mut services = vec![0x1001, 0x1002];
        if self.cfg.text {
            services.push(0x1003);
        }
        let cf = ControlFrame {
            smct_update: 1,
            smct: vec![SmctEntry {
                id: 0,
                hierarchical: false,
                high: false,
                mask: 15,
                services,
            }],
            nit: Some(mux::Nit {
                update: 1,
                country: "CHN".into(),
                network_id: 0x100,
                frequencies: vec![10_610_000],
                name: "Test CDR".into(),
            }),
            not_decoded: vec![],
        };
        cf.to_bytes(self.plan.sdi_bits / 8).unwrap_or_default()
    }
    fn service(&mut self) -> Vec<u8> {
        let len = self.plan.msds_bytes;
        let nsub = if self.cfg.text { 3 } else { 2 };
        let text_len = if self.cfg.text {
            TEST_TEXT.len() + 4 + 5 + 5 + 1 + 3 + 3
        } else {
            0
        };
        let room = len.saturating_sub(
            ServiceFrame::header_len(nsub) + text_len + 2 * (4 + 1 + 5 + 5 + 1 + 8),
        );
        let a = room * 55 / 90;
        let b = room * 35 / 90;
        let mut subs = Vec::new();
        for (i, (sid, units, share, rate, ch)) in
            [(0x1001u16, 30usize, a, 8u8, 2u8), (0x1002, 20, b, 6, 1)]
                .iter()
                .enumerate()
        {
            let per = (share / units).saturating_sub(3);
            let mut au = Vec::with_capacity(*units);
            for _ in 0..*units {
                au.push((0u8, dra_unit(*sid, self.units[i], per)));
                self.units[i] += 1;
            }
            subs.push(SubFrame {
                start: (self.lf * 14_400) as u32,
                audio: au,
                data: vec![],
                streams: vec![AudioStream {
                    algorithm: 0,
                    bitrate: ((share * 8) as f64 / 0.64 / 100.0) as u16,
                    rate_code: *rate,
                    channels: *ch,
                    language: "chi".into(),
                }],
            });
        }
        if self.cfg.text {
            subs.push(SubFrame {
                start: (self.lf * 14_400) as u32,
                audio: vec![],
                data: vec![(160, TEST_TEXT.as_bytes().to_vec())],
                streams: vec![],
            });
        }
        let sf = ServiceFrame {
            id: 0,
            emergency: false,
            updates: (1, 1, 0),
            subs,
        };
        sf.to_bytes(len).unwrap_or_default()
    }
    fn logical_cells(&mut self) -> Vec<Complex32> {
        let n = self.plan.sdis_cells + self.plan.msds_cells;
        if self.coders.is_none() {
            return (0..n).map(|_| dummy(&mut self.rng)).collect();
        }
        let c = self.control();
        let s = self.service();
        let Some(cd) = &self.coders else {
            return Vec::new();
        };
        let mut bits = Vec::new();
        unpack(&c, BitOrder::Msb, &mut bits);
        bits.resize(self.plan.sdi_bits, 0);
        scramble(&mut bits);
        let sdc = cd.sdc.encode(&bits);
        let mut bits = Vec::new();
        unpack(&s, BitOrder::Msb, &mut bits);
        let k = self.cfg.rate.k();
        bits.resize(self.plan.codewords * k, 0);
        scramble(&mut bits);
        let mut out = Vec::with_capacity(n);
        let sq = self.cfg.sdi;
        for c in sdc.chunks(sq.bits()) {
            out.push(sq.map(c));
        }
        let Some(code) = &cd.code else {
            out.extend((0..self.plan.msds_cells).map(|_| dummy(&mut self.rng)));
            return out;
        };
        let dq = self.cfg.data;
        let mut w = vec![0u8; LDPC_N];
        let mut sent = vec![0u8; LDPC_N];
        let mut all = Vec::with_capacity(self.plan.codewords * LDPC_N);
        for info in bits.chunks(k) {
            let _ = code.encode(info, &mut w);
            scatter(&cd.msds, &w, &mut sent);
            all.extend_from_slice(&sent);
        }
        for c in all.chunks(dq.bits()) {
            out.push(dq.map(c));
        }
        out
    }
    /// Appends one logical frame (4 sub-frames) of samples.
    pub fn next_logical(&mut self, out: &mut Vec<Complex32>) {
        let span = [1usize, 2, 4][(self.cfg.allocation as usize).clamp(1, 3) - 1];
        let x = self.logical_cells();
        self.history.push_front(x);
        while self.history.len() > span {
            self.history.pop_back();
        }
        let total = 4 * self.cells.data.len();
        let n = self.plan.sdis_cells + self.plan.msds_cells;
        // Dummy cells (the pre-roll of the spread over logical frames, and the cells after the logical frame) are random QPSK: a
        // constant makes a peak in the time signal.
        let mut stream = Vec::with_capacity(total);
        for j in 0..total {
            let v = match self.history.get(j % span) {
                Some(h) if j < n => h[j],
                _ => dummy(&mut self.rng),
            };
            stream.push(v);
        }
        let t = self.cfg.tx;
        let (lo, _) = phy::union(t);
        let per = self.cells.data.len();
        for sf in 0..4u8 {
            let mut grid = vec![Complex32::new(0.0, 0.0); (t.sn() + 1) * self.k];
            for (c, l) in self.cfg.spectrum.halves() {
                for kk in phy::half_carriers(t, c, l) {
                    let i = (kk - lo) as usize;
                    if kk % 2 == 0 {
                        grid[i] = phy::beacon(t, kk);
                    }
                    for s in 1..=t.sn() {
                        if phy::is_pilot(s, kk) {
                            grid[s * self.k + i] = phy::pilot(s, kk);
                        }
                    }
                }
            }
            let si = SystemInfo {
                mf: true,
                next_freq: 511,
                nominal: self.cfg.spectrum.nominal_code(),
                spectrum: self.cfg.spectrum.0,
                frame: (self.lf % 4) as u8,
                subframe: sf,
                allocation: self.cfg.allocation,
                sdi_qam: qam_index(self.cfg.sdi),
                data_qam: qam_index(self.cfg.data),
                hierarchy: 0,
                uniform: true,
                rate_hi: self.cfg.rate.index() as u8,
                rate_lo: self.cfg.rate.index() as u8,
            };
            let coded = self.coders.as_ref().map(|c| c.si.encode(&si.bits()));
            for copy in &self.cells.si {
                for (j, &i) in copy.iter().enumerate() {
                    grid[i] = match &coded {
                        Some(b) => Qam::Q4.map(&b[2 * j..2 * j + 2]),
                        None => dummy(&mut self.rng),
                    };
                }
            }
            for (j, &i) in self.cells.data.iter().enumerate() {
                grid[i] = stream[sf as usize * per + j];
            }
            self.modu.frame(&grid, out);
        }
        self.lf += 1;
    }
}

/// The generator's options.
#[derive(Clone, Debug, Default)]
pub struct GenOptions {
    /// The transmitter.
    pub tx: TxConfig,
    /// An analogue FM programme in the middle (hybrid modes), 10 dB above the digital signal.
    pub fm_programme: bool,
    /// The GY/T 268.1 tables.
    pub tables: Option<Arc<CdrTables>>,
}

/// The CDR test signal: the digital signal at unit power, and the FM programme in the hybrid modes. Impairments are the
/// framework's (`gen_*`).
pub struct CdrGenerator {
    tx: CdrTransmitter,
    o: GenOptions,
    out_rate: f64,
    rs: Option<Resampler<Complex32>>,
    pending: VecDeque<Complex32>,
    buf: Vec<Complex32>,
    tmp: Vec<Complex32>,
    fm_phase: f64,
    n: u64,
}

impl CdrGenerator {
    /// A generator at `out_rate`; `None` when the rate cannot be reached.
    pub fn new(o: GenOptions, out_rate: f64) -> Option<CdrGenerator> {
        let tx = CdrTransmitter::new(o.tx.clone(), o.tables.as_deref());
        let rs = if (out_rate - RATE).abs() > 1e-6 {
            Some(Resampler::with_quality(RATE, out_rate, 0.45, 70.0).ok()?)
        } else {
            None
        };
        Some(CdrGenerator {
            rs,
            tx,
            o,
            out_rate,
            pending: VecDeque::new(),
            buf: Vec::new(),
            tmp: Vec::new(),
            fm_phase: 0.0,
            n: 0,
        })
    }
    /// Why the generator sends no coded data, or empty when it does.
    pub fn note(&self) -> &str {
        self.tx.note()
    }
    /// The options of a generator configuration.
    ///
    /// # Errors
    ///
    /// [`TablesError`] if the tables file does not load.
    pub fn options(cfg: &GenConfig) -> Result<GenOptions, TablesError> {
        let s = &cfg.settings;
        let tx = TxConfig {
            tx: TxMode(s.get(&GEN_TX_MODE).index() as u8 + 1),
            spectrum: SpectrumMode(s.get(&GEN_SPECTRUM_MODE).code()),
            data: s.get(&GEN_DATA_QAM),
            rate: s.get(&GEN_LDPC_RATE),
            sdi: s.get(&GEN_SDI_QAM),
            allocation: s.get(&GEN_ALLOCATION).index() as u8 + 1,
            text: s.get(&GEN_TEXT_SERVICE),
        };
        Ok(GenOptions {
            tx,
            fm_programme: s.get(&GEN_FM_PROGRAMME),
            tables: load_tables(s.get(&TABLES_FILE))?,
        })
    }
    fn power(&self) -> f64 {
        if self.o.fm_programme && self.o.tx.spectrum.hybrid() {
            11.0
        } else {
            1.0
        }
    }
}

impl Generator for CdrGenerator {
    fn rate_hz(&self) -> f64 {
        self.out_rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        while self.pending.len() < out.len() {
            self.buf.clear();
            self.tx.next_logical(&mut self.buf);
            // The digital signal at unit power.
            let p = self.buf.iter().map(|s| s.norm_sqr()).sum::<f32>() / self.buf.len() as f32;
            let g = 1.0 / p.max(1e-12).sqrt();
            for s in &mut self.buf {
                *s *= g;
            }
            if self.o.fm_programme && self.o.tx.spectrum.hybrid() {
                let amp = 10f32.sqrt();
                for s in &mut self.buf {
                    let t = self.n as f64 / RATE;
                    self.fm_phase +=
                        std::f64::consts::TAU * 75e3 * (std::f64::consts::TAU * 1e3 * t).cos()
                            / RATE;
                    *s += Complex32::from_polar(amp, self.fm_phase as f32);
                    self.n += 1;
                }
                self.fm_phase %= std::f64::consts::TAU;
            }
            match &mut self.rs {
                Some(r) => {
                    self.tmp.clear();
                    r.process(&self.buf, &mut self.tmp);
                    self.pending.extend(self.tmp.iter());
                }
                None => self.pending.extend(self.buf.iter()),
            }
        }
        for o in out.iter_mut() {
            *o = self.pending.pop_front().unwrap_or_default();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // An SNR is stated in the digital signal's own bandwidth, for the whole signal's power.
        let t = self.o.tx.tx;
        let bw = self.o.tx.spectrum.halves().len() as f64 * t.half() as f64 * t.spacing();
        let power = self.power();
        Some(SignalInfo::new(bw * power).with_power(power))
    }
}

/// Makes the generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    match CdrGenerator::options(cfg)
        .ok()
        .and_then(|o| CdrGenerator::new(o, cfg.rate_hz))
    {
        Some(g) => Ok(Box::new(g)),
        None => Err(crate::silence::NoSignal {
            rate_hz: cfg.rate_hz,
        }
        .refused()),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Receiver components

/// One sub-frame with its system information.
#[derive(Clone, Debug)]
pub struct CdrSubframe {
    /// Transmission mode.
    pub tx: TxMode,
    /// The cells.
    pub frame: Arc<CellFrame>,
    /// The SI, when it decoded.
    pub si: Option<SystemInfo>,
    /// The SI constellation.
    pub si_points: Vec<[f32; 2]>,
}
impl Item for CdrSubframe {
    const KIND: &'static str = "cdr.subframe";
}

/// Decodes the SI of a sub-frame by trying every spectrum mode's copies.
pub fn decode_si(t: TxMode, f: &CellFrame, ch: &mut ConvChannel) -> Option<SystemInfo> {
    let mut soft = Vec::with_capacity(SI_BITS);
    let mut acc = vec![0.0f32; SI_BITS];
    let mut bits = Vec::new();
    for m in SpectrumMode::ALL {
        let cells = phy::subframe_cells(t, m);
        acc.fill(0.0);
        for copy in &cells.si {
            soft.clear();
            for &i in copy {
                Qam::Q4.demap(f.z[i], f.w[i].min(1e4), &mut soft);
            }
            ch.deinterleave_add(&soft, &mut acc);
        }
        if !ch.decode(&acc, &mut bits) {
            continue;
        }
        if let Some(si) = SystemInfo::from_bits(&bits) {
            if si.spectrum == m.0 {
                return Some(si);
            }
        }
    }
    None
}

/// The OFDM stage (`broadcast.cdr_ofdm`, iq at 816 ksps -> typed sub-frames): every transmission mode searched in the inner and
/// the outer band. With the tables, a lock counts once its SI decodes; without them, once the beacon is tracked.
pub struct CdrOfdm {
    loaded: LoadedTables,
    demods: Vec<(TxMode, FrameDemodulator, u32)>,
    active: Option<usize>,
    si_ch: Option<ConvChannel>,
}

impl CdrOfdm {
    /// The stage with these tables.
    pub fn new(loaded: LoadedTables) -> CdrOfdm {
        let mut demods = Vec::new();
        for t in [1u8, 2, 3] {
            for inner in [true, false] {
                demods.push((
                    TxMode(t),
                    FrameDemodulator::new(phy::layout(TxMode(t), inner)),
                    0,
                ));
            }
        }
        CdrOfdm {
            demods,
            active: None,
            si_ch: loaded
                .get()
                .and_then(|t| ConvChannel::new(SI_INFO, SI_BITS, t.si())),
            loaded,
        }
    }
    fn restart(&mut self) {
        for d in &mut self.demods {
            d.1.restart();
            d.2 = 0;
        }
        self.active = None;
    }
}

impl Component for CdrOfdm {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.cdr_ofdm",
            vec![Port::iq("in").rate(RateRule::Exact(RATE))],
            vec![Port::typed::<CdrSubframe>("subframes").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TablesSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        for d in &mut self.demods {
            d.1.reset();
            d.2 = 0;
        }
        self.active = None;
        Ok(vec![Format::new(Kind::typed::<CdrSubframe>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut out = Vec::new();
        for i in 0..self.demods.len() {
            if self.active.is_some_and(|a| a != i) {
                continue;
            }
            self.demods[i].1.push(x);
            while let Some(f) = self.demods[i].1.next_frame() {
                let t = self.demods[i].0;
                let si = self.si_ch.as_mut().and_then(|ch| decode_si(t, &f, ch));
                let good = match &self.si_ch {
                    Some(_) => si.is_some(),
                    None => self.demods[i].1.state() == FrameState::Tracking,
                };
                if good {
                    self.demods[i].2 = 0;
                    if self.active.is_none() {
                        self.active = Some(i);
                        for (j, d) in self.demods.iter_mut().enumerate() {
                            if j != i {
                                d.1.reset();
                            }
                        }
                    }
                } else {
                    self.demods[i].2 += 1;
                    // A doubtful lock: restart this search after a few sub-frames without SI; a decoded signal holds on longer.
                    let limit = if self.active == Some(i) { 12 } else { 3 };
                    if self.demods[i].2 >= limit {
                        self.demods[i].2 = 0;
                        self.demods[i].1.restart();
                        if self.active == Some(i) {
                            self.active = None;
                        }
                    }
                }
                if self.active == Some(i) {
                    let si_points = si
                        .and_then(|s| SpectrumMode::from_code(s.spectrum))
                        .map(|m| {
                            phy::subframe_cells(t, m)
                                .si
                                .iter()
                                .flatten()
                                .take(1024)
                                .map(|&j| [f.z[j].re, f.z[j].im])
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push((
                        f.time,
                        CdrSubframe {
                            tx: t,
                            frame: f,
                            si,
                            si_points,
                        },
                    ));
                }
                if self.active.is_some_and(|a| a != i) {
                    break;
                }
            }
        }
        for (t, v) in out {
            io.items_out::<CdrSubframe>(0)
                .push(Timed { time: t, value: v });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::Tables => {
                if self
                    .loaded
                    .apply(id, COMPONENT_TABLES.read(id, v)?, CdrTables::load)?
                {
                    self.si_ch = self
                        .loaded
                        .get()
                        .and_then(|t| ConvChannel::new(SI_INFO, SI_BITS, t.si()));
                    self.restart();
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            TablesSetting::Tables,
            Value::Text(self.loaded.path().into()),
        )])
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => self.restart(),
        }
        Ok(None)
    }
}

/// A decoded logical frame: control and service multiplex frames.
#[derive(Clone, Debug)]
pub struct LogicalFrame {
    /// Control multiplex frame bytes.
    pub control: Arc<[u8]>,
    /// Service multiplex frame bytes, `None` when a code word failed.
    pub service: Option<Arc<[u8]>>,
}
impl Item for LogicalFrame {
    const KIND: &'static str = "cdr.logical_frame";
}

/// Decoder counters after each sub-frame.
#[derive(Clone, Debug, Default)]
pub struct DecoderStats {
    /// The SI.
    pub si: Option<SystemInfo>,
    /// What the tables lack, or empty when they hold what the signal needs.
    pub tables: String,
    /// SI good.
    pub si_good: u64,
    /// SI bad.
    pub si_bad: u64,
    /// Sub-frames.
    pub subframes: u64,
    /// LDPC code words good.
    pub cw_good: u64,
    /// LDPC code words bad.
    pub cw_bad: u64,
    /// Mean iterations.
    pub mean_iterations: f64,
    /// Carrier offset.
    pub offset_hz: f64,
    /// SNR (Es/N0).
    pub snr_db: f32,
    /// Level.
    pub level_dbfs: f32,
    /// Transmission mode.
    pub tx: Option<u8>,
    /// Time.
    pub time: f64,
    /// MSC constellation.
    pub msc_points: Vec<[f32; 2]>,
    /// SI constellation.
    pub si_points: Vec<[f32; 2]>,
    /// |H| of one symbol: (frequency, dB).
    pub channel: Vec<[f32; 2]>,
}
impl Item for DecoderStats {
    const KIND: &'static str = "cdr.stats";
}

/// A logical frame being gathered over the allocation span: its number, cells with weights, and which are in.
type Spread = (u64, Vec<(Complex32, f32)>, Vec<bool>);

/// The decoders of a configuration.
struct RxCoders {
    sdc: ConvChannel,
    ldpc: LdpcDecoder,
    k: usize,
    msds: Vec<u32>,
}

/// The decoder (`broadcast.cdr_decoder`, typed sub-frames -> logical frames, counters).
#[derive(Default)]
pub struct CdrDecoder {
    loaded: Option<LoadedTables>,
    tables: Option<Arc<CdrTables>>,
    si: Option<SystemInfo>,
    cfg_key: Option<(u8, u8, u8, u8, u8, u8, u8)>,
    cells: Option<SubframeCells>,
    plan: Option<Plan>,
    coders: Option<RxCoders>,
    quant: Quantizer,
    slot: Vec<(Complex32, f32)>,
    slot_sf: u8,
    have: [bool; 4],
    spread: VecDeque<Spread>,
    lf_count: u64,
    stats: DecoderStats,
    iters: u64,
}

impl CdrDecoder {
    /// The decoder with these tables.
    pub fn new(loaded: LoadedTables) -> CdrDecoder {
        let mut d = CdrDecoder {
            tables: loaded.get(),
            ..Default::default()
        };
        d.stats.tables = match loaded.value() {
            Ok(Some(_)) => String::new(),
            Ok(None) => NO_TABLES.into(),
            Err(e) => format!("not loaded: {}: {e}", loaded.path()),
        };
        d.loaded = Some(loaded);
        d
    }

    fn setup(&mut self, t: TxMode, si: &SystemInfo) {
        let Some(m) = SpectrumMode::from_code(si.spectrum) else {
            return;
        };
        let p = plan(t, m, si.sdi(), si.data(), si.ldpc());
        self.cells = Some(phy::subframe_cells(t, m));
        self.coders = None;
        match &self.tables {
            None => self.stats.tables = NO_TABLES.into(),
            Some(tb) => match tb.missing(p.sdis_bits, si.ldpc()) {
                Some(miss) => self.stats.tables = format!("the tables lack {miss}"),
                None => {
                    self.coders = (|| {
                        let code = tb.code(si.ldpc())?;
                        Some(RxCoders {
                            sdc: ConvChannel::new(p.sdi_bits, p.sdis_bits, tb.sdis(p.sdis_bits)?)?,
                            ldpc: LdpcDecoder::new(code, MinSum::default()),
                            k: code.k(),
                            msds: tb.msds().to_vec(),
                        })
                    })();
                    self.stats.tables.clear();
                }
            },
        }
        self.plan = Some(p);
        self.spread.clear();
        self.slot.clear();
        self.have = [false; 4];
    }

    fn logical(&mut self, cells: &[(Complex32, f32)], out: &mut Vec<LogicalFrame>) {
        let (Some(si), Some(p), Some(cd)) = (self.si, self.plan.as_ref(), self.coders.as_mut())
        else {
            return;
        };
        let sq = si.sdi();
        let mut soft = Vec::with_capacity(p.sdis_bits);
        for &(z, w) in &cells[..p.sdis_cells] {
            sq.demap(z, w.min(1e4), &mut soft);
        }
        let mut acc = vec![0.0f32; p.sdis_bits];
        cd.sdc.deinterleave_add(&soft, &mut acc);
        let mut bits = Vec::new();
        cd.sdc.decode(&acc, &mut bits);
        scramble(&mut bits);
        let mut control = Vec::with_capacity(bits.len() / 8);
        pack(&bits, BitOrder::Msb, Pad::Drop, &mut control);
        let dq = si.data();
        let mcells = &cells[p.sdis_cells..p.sdis_cells + p.msds_cells];
        self.stats.msc_points = mcells
            .iter()
            .step_by((mcells.len() / 1024).max(1))
            .take(1024)
            .map(|c| [c.0.re, c.0.im])
            .collect();
        let mut soft = Vec::with_capacity(p.codewords * LDPC_N);
        for &(z, w) in mcells {
            dq.demap(z, w.min(1e4), &mut soft);
        }
        let mut word = vec![0.0f32; LDPC_N];
        let mut llr = vec![0 as Llr8; LDPC_N];
        let mut hard = vec![0u8; LDPC_N];
        let mut info = Vec::with_capacity(p.codewords * cd.k);
        let mut ok = true;
        for c in soft.as_chunks::<LDPC_N>().0.iter().take(p.codewords) {
            gather(&cd.msds, c, &mut word);
            let _ = self.quant.quantize_auto(&word, &mut llr);
            match cd.ldpc.decode(&llr, 40, &mut hard) {
                Ok(d) => {
                    self.stats.cw_good += 1;
                    self.iters += d.iterations as u64;
                }
                Err(_) => {
                    self.stats.cw_bad += 1;
                    self.iters += 40;
                    ok = false;
                }
            }
            info.extend_from_slice(&hard[..cd.k]);
        }
        let n = (self.stats.cw_good + self.stats.cw_bad).max(1);
        self.stats.mean_iterations = self.iters as f64 / n as f64;
        scramble(&mut info);
        let mut service = Vec::with_capacity(info.len() / 8);
        pack(&info, BitOrder::Msb, Pad::Drop, &mut service);
        out.push(LogicalFrame {
            control: control.into(),
            service: ok.then(|| service.into()),
        });
    }
}

impl Component for CdrDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.cdr_decoder",
            vec![Port::typed::<CdrSubframe>("subframes")],
            vec![
                Port::typed::<LogicalFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DecoderStats>("stats").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TablesSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<LogicalFrame>(), 0.0),
            Format::new(Kind::typed::<DecoderStats>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut frames = Vec::new();
        let mut stats = Vec::new();
        for it in io.items::<CdrSubframe>(0) {
            let c = &it.value;
            let f = &c.frame;
            self.stats.subframes += 1;
            self.stats.offset_hz = f.cfo_hz;
            self.stats.snr_db = f.snr_db;
            self.stats.level_dbfs = f.level_db;
            self.stats.tx = Some(c.tx.0);
            self.stats.time = f.time;
            self.stats.si_points.clone_from(&c.si_points);
            let (lo, _) = phy::union(c.tx);
            self.stats.channel =
                f.h.iter()
                    .enumerate()
                    .step_by(4)
                    .filter(|(_, h)| h.norm_sqr() > 0.0)
                    .map(|(i, h)| {
                        [
                            ((lo + i as i32) as f64 * c.tx.spacing() / 1e3) as f32,
                            10.0 * h.norm_sqr().log10(),
                        ]
                    })
                    .collect();
            if f.first {
                self.slot.clear();
                self.have = [false; 4];
                self.spread.clear();
            }
            let sf = match &c.si {
                Some(si) => {
                    self.stats.si_good += 1;
                    let key = (
                        si.spectrum,
                        si.allocation,
                        si.sdi_qam,
                        si.data_qam,
                        si.rate_hi,
                        si.hierarchy,
                        c.tx.0,
                    );
                    if self.cfg_key != Some(key) {
                        self.cfg_key = Some(key);
                        self.setup(c.tx, si);
                    }
                    self.si = Some(*si);
                    self.stats.si = Some(*si);
                    si.subframe
                }
                None => {
                    if self.tables.is_some() {
                        self.stats.si_bad += 1;
                    }
                    (self.slot_sf + 1) % 4
                }
            };
            self.slot_sf = sf;
            let (Some(cells), Some(p)) = (&self.cells, self.plan.clone()) else {
                stats.push((it.time, self.stats.clone()));
                continue;
            };
            if sf == 0 {
                self.slot.clear();
                self.have = [false; 4];
            }
            if self.have[..sf as usize].iter().all(|h| *h) {
                for &i in &cells.data {
                    self.slot.push((f.z[i], f.w[i]));
                }
                self.have[sf as usize] = true;
            }
            if sf == 3 && self.have.iter().all(|h| *h) && self.coders.is_some() {
                let span = self.si.map_or(1, |si| {
                    [1usize, 2, 4][(si.allocation as usize).clamp(1, 3) - 1]
                });
                let n = p.sdis_cells + p.msds_cells;
                let m = self.lf_count;
                self.lf_count += 1;
                if !self.spread.iter().any(|e| e.0 == m) {
                    self.spread.push_back((
                        m,
                        vec![(Complex32::new(0.0, 0.0), 0.0); n],
                        vec![false; n],
                    ));
                }
                for j in 0..n.min(self.slot.len()) {
                    let target = m.wrapping_sub((j % span) as u64);
                    if let Some(e) = self.spread.iter_mut().find(|e| e.0 == target) {
                        e.1[j] = self.slot[j];
                        e.2[j] = true;
                    }
                }
                let done = m.wrapping_sub(span as u64 - 1);
                let entry = self
                    .spread
                    .iter()
                    .position(|e| e.0 == done)
                    .and_then(|p| self.spread.remove(p));
                while self.spread.len() > span {
                    self.spread.pop_front();
                }
                if let Some((_, cells, have)) = entry {
                    if have.iter().all(|h| *h) {
                        let mut lf = Vec::new();
                        self.logical(&cells, &mut lf);
                        frames.extend(lf.into_iter().map(|x| (it.time, x)));
                    }
                }
            }
            stats.push((it.time, self.stats.clone()));
        }
        for (t, v) in frames {
            io.items_out::<LogicalFrame>(0)
                .push(Timed { time: t, value: v });
        }
        for (t, v) in stats {
            io.items_out::<DecoderStats>(1)
                .push(Timed { time: t, value: v });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::Tables => {
                let mut loaded = self.loaded.take().unwrap_or_else(LoadedTables::none);
                let changed = loaded.apply(id, COMPONENT_TABLES.read(id, v)?, CdrTables::load);
                match changed {
                    Ok(true) => *self = CdrDecoder::new(loaded),
                    Ok(false) => self.loaded = Some(loaded),
                    Err(e) => {
                        self.loaded = Some(loaded);
                        return Err(e);
                    }
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let path = self.loaded.as_ref().map_or("", LoadedTables::path);
        values_of([(TablesSetting::Tables, Value::Text(path.into()))])
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => {
                self.cfg_key = None;
                self.cells = None;
                self.spread.clear();
                self.slot.clear();
            }
        }
        Ok(None)
    }
    /// The frame state and the counts start again. What the tables lack stays.
    fn forget(&mut self, _kept: &Kept) {
        self.cfg_key = None;
        self.cells = None;
        self.spread.clear();
        self.slot.clear();
        let tables = std::mem::take(&mut self.stats.tables);
        self.stats = DecoderStats {
            tables,
            ..DecoderStats::default()
        };
    }
}

/// One row of the services table.
#[derive(Clone, Debug, Default)]
struct ServiceRow {
    seen: bool,
    audio: bool,
    data: bool,
    streams: Vec<AudioStream>,
    sample_rate: u32,
    kbps: f64,
    units: usize,
    data_types: Vec<u8>,
    types_written: usize,
    text: String,
    good: u64,
    bad: u64,
}

sdrkit_core::commands! {
    /// The commands of [`CdrMux`].
    pub enum MuxCommand {
        /// Plays the service whose `services` row key (its service id in hexadecimal) is the argument `row`. "0" plays the
        /// first service of the multiplex.
        ServiceSelect = SERVICE_SELECT_KIND.command,
        /// Forgets the services and the network.
        Clear = crate::component::CLEAR,
    }
}

/// The multiplex parser and results (`broadcast.cdr_mux`, typed logical frames and counters -> the DRA+ units of the service
/// playing, as media packets, and the results).
pub struct CdrMux {
    stats: DecoderStats,
    published: Published,
    status: Status,
    key: String,
    services: BTreeMap<u16, ServiceRow>,
    smct: Vec<SmctEntry>,
    nit: Option<mux::Nit>,
    frequencies_written: Vec<u32>,
    control: (u64, u64),
    service_frames: (u64, u64),
    service: Option<u16>,
    format: Option<(u16, u32, u8, Arc<MediaFormat>)>,
    units_in: u64,
    has_stats: bool,
    next_report: f64,
}

impl Default for CdrMux {
    fn default() -> Self {
        CdrMux {
            stats: DecoderStats {
                tables: NO_TABLES.into(),
                ..Default::default()
            },
            published: Published::from_decl(Arc::new(crate::typed::shape(ID).results_decl())),
            status: Status::new(LockState::Searching),
            key: String::with_capacity(16),
            services: BTreeMap::new(),
            smct: Vec::new(),
            nit: None,
            frequencies_written: Vec::new(),
            control: (0, 0),
            service_frames: (0, 0),
            service: None,
            format: None,
            units_in: 0,
            has_stats: false,
            next_report: 0.0,
        }
    }
}

/// The series of the stack, in their order in its description.
const SERIES: [SeriesId; 3] = [SeriesId(0), SeriesId(1), SeriesId(2)];

impl CdrMux {
    /// The stage, with what the decoder says of the tables before the first sub-frame.
    pub fn new(tables: String) -> CdrMux {
        let mut m = CdrMux::default();
        m.stats.tables = tables;
        m.publish();
        m
    }
    /// The service that plays: the one selected, else the first of the multiplex.
    fn listened(&self) -> Option<u16> {
        self.service
            .or_else(|| self.smct.first().and_then(|e| e.services.first().copied()))
    }
    fn publish(&mut self) {
        let st = &self.stats;
        let state = if self.service_frames.0 > 0 || self.control.0 > 0 {
            3
        } else if st.si_good > 0 {
            2
        } else if st.subframes > 0 {
            1
        } else {
            0
        };
        let listened = self.listened();
        let p = &mut self.published;
        let spectrum = st.si.and_then(|si| SpectrumMode::from_code(si.spectrum));
        let synced = st.subframes > 0;
        let selected = listened.map(|sid| {
            self.key.clear();
            // Writing into a String cannot fail.
            let _ = write!(self.key, "{sid:04X}");
            self.key.as_str()
        });
        p.put(&res::CdrFields {
            tables: if st.tables.is_empty() {
                "loaded"
            } else {
                st.tables.as_str()
            },
            tx_mode: st.tx.map(i64::from),
            spectrum_mode: spectrum.map(|m| i64::from(m.0)),
            spectrum: spectrum.map(|m| m.text()),
            ldpc_rate: st.si.map(|si| si.ldpc().text()),
            offset_hz: synced.then_some(Hertz(st.offset_hz)),
            snr_db: synced.then_some(f64::from(st.snr_db)),
            level_dbfs: self.has_stats.then_some(f64::from(st.level_dbfs)),
            subframes: st.subframes as i64,
            si_ok: st.si_good as i64,
            si_bad: st.si_bad as i64,
            cw_ok: st.cw_good as i64,
            cw_bad: st.cw_bad as i64,
            iterations: (st.cw_good + st.cw_bad > 0).then_some(st.mean_iterations),
            control_ok: self.control.0 as i64,
            control_bad: self.control.1 as i64,
            service_ok: self.service_frames.0 as i64,
            service_bad: self.service_frames.1 as i64,
            network: self.nit.as_ref().map(|n| n.name.as_str()),
            country: self.nit.as_ref().map(|n| n.country.as_str()),
            network_id: self.nit.as_ref().map(|n| n.network_id as i64),
            selected_service: selected,
            units_in: self.units_in as i64,
        });
        let freqs: &[u32] = self.nit.as_ref().map_or(&[], |n| &n.frequencies);
        if self.frequencies_written != freqs {
            p.clear_rows(&res::NETWORK_FREQUENCIES);
            for (i, &f) in freqs.iter().enumerate() {
                self.key.clear();
                let _ = write!(self.key, "{i}");
                // GY/T 268.2: the frequency in units of 10 Hz.
                let row = res::CdrFrequencyRow {
                    freq_hz: Hertz(f64::from(f) * 10.0),
                };
                res::NETWORK_FREQUENCIES.put(p, &self.key, &row);
            }
            self.frequencies_written.clear();
            self.frequencies_written.extend_from_slice(freqs);
        }
        for (sid, s) in &mut self.services {
            let s0 = s.streams.first();
            self.key.clear();
            let _ = write!(self.key, "{sid:04X}");
            let row = res::CdrServiceRow {
                sid: i64::from(*sid),
                audio: s.audio,
                data: s.data,
                sample_rate: (s.seen && s.audio).then_some(i64::from(s.sample_rate)),
                channels: s0.map(|x| i64::from(x.channels)),
                language: s0.map(|x| x.language.as_str()),
                algorithm: s0.map(|x| i64::from(x.algorithm)),
                kbps: s.kbps,
                units: s.units as i64,
                text: (!s.text.is_empty()).then_some(s.text.as_str()),
                good: s.good as i64,
                bad: s.bad as i64,
            };
            res::SERVICES.put(p, &self.key, &row);
            for &t in &s.data_types[s.types_written..] {
                self.key.clear();
                let _ = write!(self.key, "{sid:04X}/{t}");
                let (service, _) = self.key.split_at(4);
                let row = res::CdrDataTypeRow {
                    service,
                    data_type: i64::from(t),
                };
                res::SERVICE_DATA_TYPES.put(p, &self.key, &row);
            }
            s.types_written = s.data_types.len();
        }
        let r = p.results_mut();
        for (id, pts) in SERIES
            .iter()
            .zip([&st.si_points, &st.msc_points, &st.channel])
        {
            if !pts.is_empty() {
                r.update_series(*id, |s| {
                    sdrkit_core::results::copy_if_changed(&mut s.points, pts)
                });
            }
        }
        let status = &mut self.status;
        status.lock_state = match state {
            0 => LockState::Searching,
            1 | 2 => LockState::Tracking,
            _ => LockState::Locked,
        };
        status.snr_db = Some(st.snr_db);
        status.level_dbfs = Some(st.level_dbfs);
        status.offset_hz = Some(st.offset_hz);
        status.summary.clear();
        let w = &mut status.summary;
        let _ = match state {
            0 if !st.tables.is_empty() => write!(w, "searching for a beacon; tables {}", st.tables),
            0 => write!(w, "searching for a beacon"),
            1 if !st.tables.is_empty() => write!(w, "beacon found; tables {}", st.tables),
            1 => write!(w, "synchronised"),
            2 if !st.tables.is_empty() => {
                write!(w, "system information read; tables {}", st.tables)
            }
            2 => write!(w, "reading the system information"),
            _ => write!(w, "decoding the multiplex"),
        };
        r.set_status(&self.status);
    }

    /// The media format of the DRA+ units of service `sid`, made again when its sample rate or channel count changes.
    fn format_of(&mut self, sid: u16, rate: u32, channels: u8) -> Arc<MediaFormat> {
        match &self.format {
            Some((s, r, c, f)) if (*s, *r, *c) == (sid, rate, channels) => Arc::clone(f),
            _ => {
                let mut f = MediaFormat::new(Codec::Dra, Packaging::Raw);
                f.audio = Some(AudioProps {
                    sample_rate: rate,
                    channels,
                });
                let f = Arc::new(f);
                self.format = Some((sid, rate, channels, Arc::clone(&f)));
                f
            }
        }
    }

    fn frame(&mut self, f: &LogicalFrame, time: f64, io: &mut Io) {
        match ControlFrame::parse(&f.control) {
            Some((cf, _, _)) => {
                self.control.0 += 1;
                if cf.nit.is_some() {
                    self.nit = cf.nit;
                }
                if !cf.smct.is_empty() {
                    self.smct = cf.smct;
                    for e in &self.smct {
                        for sid in &e.services {
                            self.services.entry(*sid).or_default();
                        }
                    }
                }
            }
            None => self.control.1 += 1,
        }
        let Some((_, subs)) = f.service.as_deref().and_then(ServiceFrame::parse) else {
            self.service_frames.1 += 1;
            return;
        };
        self.service_frames.0 += 1;
        let ids: Vec<u16> = self
            .smct
            .first()
            .map(|e| e.services.clone())
            .unwrap_or_default();
        let listened = self.service.or(ids.first().copied());
        let mut playing = None;
        for (j, s) in subs.into_iter().enumerate() {
            let Some(&sid) = ids.get(j) else { continue };
            let row = self.services.entry(sid).or_default();
            let Some(rx) = s else {
                row.bad += 1;
                continue;
            };
            row.good += 1;
            row.seen = true;
            if !rx.audio_ok || !rx.data_ok {
                row.bad += 1;
            }
            let sub = rx.sub;
            row.audio = !sub.audio.is_empty();
            row.data = !sub.data.is_empty();
            row.units = sub.audio.len();
            if !sub.streams.is_empty() {
                row.sample_rate = mux::sample_rate(sub.streams[0].rate_code);
                row.streams.clone_from(&sub.streams);
            }
            let bytes: usize = sub.audio.iter().map(|u| u.1.len()).sum::<usize>()
                + sub.data.iter().map(|u| u.1.len()).sum::<usize>();
            row.kbps = bytes as f64 * 8.0 / 0.64 / 1000.0;
            for (t, b) in &sub.data {
                if !row.data_types.contains(t) {
                    row.data_types.push(*t);
                }
                if let Ok(s) = std::str::from_utf8(b) {
                    row.text = s.to_string();
                }
            }
            if Some(sid) == listened {
                let channels = row.streams.first().map_or(2, |s| s.channels);
                playing = Some((sid, row.sample_rate, channels, sub.audio));
            }
        }
        if let Some((sid, rate, channels, units)) = playing {
            let format = self.format_of(sid, rate, channels);
            for (_, b) in units {
                self.units_in += 1;
                io.items_out::<MediaPacket>(0).push(Timed {
                    time,
                    value: MediaPacket::new(Packet::new(b.into(), time), Arc::clone(&format)),
                });
            }
        }
    }
}

impl Component for CdrMux {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.cdr_mux",
            vec![
                Port::typed::<LogicalFrame>("frames"),
                Port::typed::<DecoderStats>("stats"),
            ],
            vec![Port::typed::<MediaPacket>("dra").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.next_report = 0.0;
        Ok(vec![Format::new(Kind::typed::<MediaPacket>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut tmax = 0.0f64;
        for s in io.items::<DecoderStats>(1) {
            self.stats.clone_from(&s.value);
            self.has_stats = true;
            tmax = tmax.max(s.time);
        }
        let frames = io.items::<LogicalFrame>(0);
        for f in frames {
            self.frame(&f.value, f.time, io);
        }
        let t = self.stats.time.max(tmax);
        if t >= self.next_report || !frames.is_empty() {
            while self.next_report <= t {
                self.next_report += 0.25;
            }
            self.publish();
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        MuxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match MuxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MuxCommand::ServiceSelect => {
                let row = args.arg(&SERVICE_ROW)?;
                self.service = if row == "0" {
                    None
                } else {
                    match u16::from_str_radix(row, 16) {
                        Ok(sid) if self.services.is_empty() || self.services.contains_key(&sid) => {
                            Some(sid)
                        }
                        _ => {
                            let table = self
                                .published
                                .results()
                                .decl()
                                .table_id(res::SERVICES.key)
                                .unwrap_or(sdrkit_core::TableId(0));
                            return Err(CommandError::NoSuchRow {
                                table,
                                key: row.to_string(),
                            });
                        }
                    }
                };
                self.publish();
            }
            MuxCommand::Clear => {
                self.services.clear();
                self.smct.clear();
                self.nit = None;
                self.published.clear_rows(&res::SERVICES);
                self.published.clear_rows(&res::SERVICE_DATA_TYPES);
                self.publish();
            }
        }
        Ok(None)
    }
    /// Every result belongs to the multiplex on the frequency before: the services, the network, the counts and the
    /// measurements start again. The service chosen and what the tables lack stay.
    fn forget(&mut self, _kept: &Kept) {
        let tables = std::mem::take(&mut self.stats.tables);
        self.stats = DecoderStats {
            tables,
            ..DecoderStats::default()
        };
        self.has_stats = false;
        self.services.clear();
        self.smct.clear();
        self.nit = None;
        self.control = (0, 0);
        self.service_frames = (0, 0);
        self.units_in = 0;
        self.published.clear_rows(&res::SERVICES);
        self.published.clear_rows(&res::SERVICE_DATA_TYPES);
        let r = self.published.results_mut();
        for id in SERIES {
            r.update_series(id, |s| {
                let changed = !s.points.is_empty();
                s.points.clear();
                changed
            });
        }
        self.publish();
    }
}

/// The CDR graph, with the tables that the components start with.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn graph(loaded: &LoadedTables, catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let decoder = CdrDecoder::new(loaded.clone());
    let decoder_note = decoder.stats.tables.clone();
    let front = crate::component::channel(300e3, RATE).map_err(|error| GraphError::Setting {
        key: "front".into(),
        error,
    })?;
    GraphBuilder::new()
        .add("front", front)
        .add("ofdm", CdrOfdm::new(loaded.clone()))
        .add("decoder", decoder)
        .add("mux", CdrMux::new(decoder_note))
        .input("front")
        .connect("front", "ofdm")
        .connect("ofdm", "decoder")
        .connect("decoder.frames", "mux.frames")
        .connect("decoder.stats", "mux.stats")
        .output("mux")
        .status_from("mux")
        .catalog(catalog)
        .build()
}

/// The CDR receiver. The tables of the `cdr_tables` setting are loaded once and shared by its components.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build. A tables file that does not load is not an error: the `tables` field says why.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let loaded = LoadedTables::load(cfg.settings.get(&TABLES_FILE), CdrTables::load);
    StackGraph::of(crate::typed::shape(ID), graph(&loaded, &Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE, "CDR needs a sample rate of at least 768 ksps")
        .route("cdr_tables", &["ofdm.tables", "decoder.tables"])?
        .command(
            "service.select",
            sdrkit_graph::CommandTarget::of("mux", "service.select"),
        )
        .command("clear", sdrkit_graph::CommandTarget::of("mux", "clear"))
        .command("resync", sdrkit_graph::CommandTarget::of("ofdm", "clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("ofdm", "clear"))?
        .on_retune(sdrkit_graph::CommandTarget::of("decoder", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

sdrkit_core::choice_enum! {
    /// A transmission mode (GY/T 268.1 table 1).
    #[derive(Default)]
    pub enum TxModeChoice {
        /// Mode 1.
        #[default]
        M1 = ("1", "1"),
        /// Mode 2.
        M2 = ("2", "2"),
        /// Mode 3.
        M3 = ("3", "3"),
    }
}

sdrkit_core::choice_enum! {
    /// A spectrum mode (GY/T 268.1 table 2).
    #[derive(Default)]
    pub enum SpectrumModeChoice {
        /// 100 kHz, all digital.
        #[default]
        S1 = ("1", "1"),
        /// 200 kHz, all digital.
        S2 = ("2", "2"),
        /// 2 × 50 kHz beside stereo FM.
        S9 = ("9", "9"),
        /// 2 × 100 kHz beside stereo FM.
        S10 = ("10", "10"),
        /// 2 × 50 kHz beside mono FM.
        S22 = ("22", "22"),
        /// 2 × 100 kHz beside mono FM.
        S23 = ("23", "23"),
    }
}

impl SpectrumModeChoice {
    /// The code of the spectrum mode.
    pub fn code(self) -> u8 {
        match self {
            SpectrumModeChoice::S1 => 1,
            SpectrumModeChoice::S2 => 2,
            SpectrumModeChoice::S9 => 9,
            SpectrumModeChoice::S10 => 10,
            SpectrumModeChoice::S22 => 22,
            SpectrumModeChoice::S23 => 23,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The time span of a logical frame (GY/T 268.1 clause 4.3).
    #[derive(Default)]
    pub enum AllocationChoice {
        /// One sub-frame slot.
        #[default]
        A1 = ("1", "1"),
        /// Two.
        A2 = ("2", "2"),
        /// Four.
        A3 = ("3", "3"),
    }
}

/// The tables file.
pub const TABLES_FILE: Setting<field::Text> = Setting::new("cdr_tables", "GY/T 268.1 tables", "", "")
    .help("A JSON file with the LDPC matrices and the bit interleavers of GY/T 268.1. Without it nothing is decoded.")
    .restart();
/// Test signal: the transmission mode.
pub const GEN_TX_MODE: Setting<field::Choice<TxModeChoice>> =
    Setting::choice("gen_tx_mode", "Transmission mode", TxModeChoice::M1)
        .section("Test signal")
        .advanced();
/// Test signal: the spectrum mode.
pub const GEN_SPECTRUM_MODE: Setting<field::Choice<SpectrumModeChoice>> =
    Setting::choice("gen_spectrum_mode", "Spectrum mode", SpectrumModeChoice::S1)
        .section("Test signal")
        .advanced();
/// Test signal: the constellation of the data.
pub const GEN_DATA_QAM: Setting<field::Choice<Qam>> =
    Setting::choice("gen_data_qam", "Data", Qam::Q4)
        .section("Test signal")
        .advanced();
/// Test signal: the LDPC rate.
pub const GEN_LDPC_RATE: Setting<field::Choice<LdpcRate>> =
    Setting::choice("gen_ldpc_rate", "LDPC rate", LdpcRate::R3_4)
        .section("Test signal")
        .advanced();
/// Test signal: the constellation of the SDIS.
pub const GEN_SDI_QAM: Setting<field::Choice<Qam>> = Setting::choice("gen_sdi_qam", "SDI", Qam::Q4)
    .section("Test signal")
    .advanced();
/// Test signal: the allocation.
pub const GEN_ALLOCATION: Setting<field::Choice<AllocationChoice>> =
    Setting::choice("gen_allocation", "Allocation", AllocationChoice::A1)
        .section("Test signal")
        .advanced();
/// Test signal: an analogue FM programme in the hybrid modes.
pub const GEN_FM_PROGRAMME: Setting<field::Bool> =
    Setting::new("gen_fm_programme", "FM programme", "", true)
        .section("Test signal")
        .advanced();
/// Test signal: a text service.
pub const GEN_TEXT_SERVICE: Setting<field::Bool> =
    Setting::new("gen_text_service", "Text service", "", true)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the CDR stack: the tables file, then the test signal's.
    pub enum CdrSetting {
        /// The tables file.
        Tables = TABLES_FILE,
        /// Test signal: the transmission mode.
        GenTxMode = GEN_TX_MODE,
        /// Test signal: the spectrum mode.
        GenSpectrumMode = GEN_SPECTRUM_MODE,
        /// Test signal: the constellation of the data.
        GenDataQam = GEN_DATA_QAM,
        /// Test signal: the LDPC rate.
        GenLdpcRate = GEN_LDPC_RATE,
        /// Test signal: the constellation of the SDIS.
        GenSdiQam = GEN_SDI_QAM,
        /// Test signal: the allocation.
        GenAllocation = GEN_ALLOCATION,
        /// Test signal: an analogue FM programme.
        GenFmProgramme = GEN_FM_PROGRAMME,
        /// Test signal: a text service.
        GenTextService = GEN_TEXT_SERVICE,
    }
}

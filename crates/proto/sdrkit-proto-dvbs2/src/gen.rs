//! The DVB-S and DVB-S2 test-signal generator: DVB-S (outer code, punctured convolutional code, QPSK)
//! and DVB-S2 (mode adaptation, BB scrambling, BCH, LDPC, bit interleaving, mapping, PLFRAMEs with pilots, PL scrambling, dummy
//! frames, GSE and multistream options), then root raised cosine pulses. The signal is clean: the framework adds the
//! impairments.

use super::modcod::ModCod;
use super::settings::{
    GenStandardChoice, GEN_DUMMY_EVERY, GEN_GSE, GEN_MODULATION, GEN_MULTISTREAM, GEN_PILOTS,
    GEN_PL_SCRAMBLING, GEN_RATE, GEN_ROLLOFF, GEN_SHORT, GEN_STANDARD, GEN_SYMBOL_RATE,
};
use crate::inner::PunctureRate;
use crate::s1encode::{useful_bit_rate, S1Encoder};
use crate::s2encode::{bb_header, roll_off_field, S2Encoder, TsAdapter};
use crate::transmit::{payload, signal_info, Payload, TxShaper, GEN_PAYLOAD};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo, Values};
use sdrkit_proto_ipcast::bbheader::BBHEADER_LEN;
use sdrkit_proto_ipcast::gse::{GseWriter, Label};
use sdrkit_proto_ipcast::ip::write_ipv4_udp;
use sdrkit_proto_mpegts::source::PacketSource;
use std::net::Ipv4Addr;

/// What the generator sends.
#[derive(Clone, Copy, Debug)]
pub enum GenStandard {
    /// DVB-S with a code rate.
    S(PunctureRate),
    /// DVB-S2 with a MODCOD, frame size and pilots.
    S2 {
        /// The MODCOD.
        modcod: ModCod,
        /// Short frames.
        short: bool,
        /// Pilots.
        pilots: bool,
    },
}

/// The generator's parameters.
pub struct DvbsGenParams {
    /// Sample rate.
    pub fs: f64,
    /// Standard.
    pub standard: GenStandard,
    /// Symbol rate, Hz.
    pub symbol_rate: f64,
    /// Roll-off.
    pub rolloff: f64,
    /// PL scrambling code.
    pub gold: u32,
    /// A dummy PLFRAME every N frames (0: none).
    pub dummy_every: u32,
    /// GSE (IP) instead of a transport stream.
    pub gse: bool,
    /// Multistream: ISI 0 transport stream, ISI 1 GSE.
    pub multistream: bool,
    /// What it sends.
    pub source: Box<dyn PacketSource>,
    /// A generic continuous stream instead of a transport stream: the bytes of each data field (TS/GS 01, UPL 0, SYNCD 0).
    pub data: Option<Box<dyn DataFieldSource>>,
}

/// The bytes of a generic continuous stream, data field by data field.
pub trait DataFieldSource: Send {
    /// Fills `df` with the next bytes of the stream.
    fn fill(&mut self, df: &mut [u8]);
}

impl DvbsGenParams {
    /// DVB-S2 QPSK 2/3 normal at 5 Msym/s (or what the rate allows) with the test multiplex.
    pub fn new(fs: f64) -> Self {
        DvbsGenParams {
            fs,
            standard: GenStandard::S2 {
                modcod: ModCod::new(6).unwrap_or_else(|| unreachable!("MODCOD 6 exists")),
                short: false,
                pilots: false,
            },
            symbol_rate: (5e6f64).min(fs / 2.0),
            rolloff: 0.35,
            gold: 0,
            dummy_every: 0,
            gse: false,
            multistream: false,
            source: payload(false),
            data: None,
        }
    }
    /// The parameters of the stack's `gen_` settings. The schema of `s` must be that of [`super::DvbsSetting`].
    pub fn from_settings(fs: f64, s: &Values) -> Self {
        let mut p = DvbsGenParams::new(fs);
        let rate = s.get(&GEN_RATE);
        match s.get(&GEN_STANDARD) {
            GenStandardChoice::Dvbs => {
                p.standard = GenStandard::S(rate.punctured().unwrap_or(PunctureRate::R23))
            }
            GenStandardChoice::Dvbs2 => {
                let modcod = ModCod::find(s.get(&GEN_MODULATION), rate)
                    .or(ModCod::new(6))
                    .unwrap_or_else(|| unreachable!("MODCOD 6 exists"));
                p.standard = GenStandard::S2 {
                    modcod,
                    short: s.get(&GEN_SHORT),
                    pilots: s.get(&GEN_PILOTS),
                };
            }
        }
        let r = s.get(&GEN_SYMBOL_RATE);
        if r > 0.0 {
            p.symbol_rate = r;
        }
        p.rolloff = s.get(&GEN_ROLLOFF).value();
        p.gold = s.get(&GEN_PL_SCRAMBLING) as u32;
        p.dummy_every = s.get(&GEN_DUMMY_EVERY) as u32;
        p.gse = s.get(&GEN_GSE);
        p.multistream = s.get(&GEN_MULTISTREAM);
        p.source = payload(s.get(&GEN_PAYLOAD) == Payload::Numbered);
        p
    }
}

/// The DVB-S and DVB-S2 test-signal generator.
pub struct DvbsGenerator {
    p: DvbsGenParams,
    shaper: Option<TxShaper>,
    // DVB-S.
    s1: Option<S1Encoder>,
    // DVB-S2.
    s2: Option<S2Encoder>,
    adapter: TsAdapter,
    gse: GseWriter,
    frame_no: u64,
    syms: Vec<Complex32>,
    ip_n: u32,
}

impl DvbsGenerator {
    /// A generator with these parameters.
    pub fn new(p: DvbsGenParams) -> Self {
        let s2 = match p.standard {
            GenStandard::S2 {
                modcod,
                short,
                pilots,
            } => Some(S2Encoder::new(modcod, short, pilots, p.gold)),
            GenStandard::S(_) => None,
        };
        let s1 = match p.standard {
            GenStandard::S(r) => Some(S1Encoder::new(r)),
            GenStandard::S2 { .. } => None,
        };
        DvbsGenerator {
            shaper: TxShaper::new(p.fs, p.symbol_rate, p.rolloff).ok(),
            s1,
            s2,
            adapter: TsAdapter::default(),
            gse: GseWriter::default(),
            frame_no: 0,
            syms: Vec::new(),
            ip_n: 0,
            p,
        }
    }
    /// The generator of the stack's generator configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(DvbsGenParams::from_settings(cfg.rate_hz, &cfg.settings))
    }

    fn s1_packet(&mut self) {
        let mut p = [0u8; 188];
        self.p.source.next_packet(&mut p);
        if let Some(e) = self.s1.as_mut() {
            e.packet(&p, &mut self.syms);
        }
    }

    /// Queues GSE packets until the backlog fills two data fields of `len` bytes.
    fn fill_gse(&mut self, len: usize) {
        let mut pkt = Vec::with_capacity(1500);
        while self.gse.backlog() < 2 * len {
            let n = self.ip_n;
            self.ip_n += 1;
            let payload: Vec<u8> = (0..(200 + (n as usize * 53) % 1200))
                .map(|k| (k as u32 ^ n) as u8)
                .collect();
            pkt.clear();
            let src = Ipv4Addr::new(10, 1, 0, (n % 3) as u8 + 1);
            let dst = Ipv4Addr::new(239, 2, 0, 1);
            let mut ranges = Vec::new();
            let r = write_ipv4_udp(
                &mut pkt,
                src,
                dst,
                n as u16,
                1500,
                4000,
                5004 + (n % 3) as u16,
                &payload,
                |r| ranges.push(r),
            );
            if r.is_err() {
                return;
            }
            let label = Label::Six([0, 0x11, 0x22, 0x33, 0x44, (n % 3) as u8]);
            for r in ranges {
                if self.gse.push_pdu(0x0800, label, &pkt[r]).is_err() {
                    return;
                }
            }
        }
    }

    fn s2_frame(&mut self) {
        self.frame_no += 1;
        let code = self.s2.as_ref().and_then(S2Encoder::code);
        let dummy =
            self.p.dummy_every > 0 && self.frame_no.is_multiple_of(self.p.dummy_every as u64);
        let (Some(code), false) = (code, dummy) else {
            if let Some(e) = &self.s2 {
                e.dummy(&mut self.syms);
            }
            return;
        };
        let dfl = code.kbch / 8 - BBHEADER_LEN;
        let isi = self.p.multistream.then_some((self.frame_no % 2) as u8);
        let gse = self.p.gse || isi == Some(1);
        let generic = self.p.data.is_some() && !gse;
        if gse {
            self.fill_gse(dfl);
        }
        let Some(enc) = self.s2.as_mut() else { return };
        let ro = roll_off_field(self.p.rolloff);
        let frame = enc.bbframe();
        if gse || generic {
            let df = &mut frame[BBHEADER_LEN..];
            match self.p.data.as_mut().filter(|_| generic) {
                Some(src) => src.fill(df),
                None => {
                    self.gse.fill(df);
                }
            }
            let mut hb = [0u8; BBHEADER_LEN];
            bb_header(false, isi, ro, (8 * dfl) as u16, 0).write(&mut hb);
            frame[..BBHEADER_LEN].copy_from_slice(&hb);
        } else {
            let src = &mut self.p.source;
            self.adapter.fill(frame, dfl, isi, ro, &mut || {
                let mut p = [0u8; 188];
                src.next_packet(&mut p);
                p
            });
        }
        enc.encode(&mut self.syms);
    }

    /// The information bit rate.
    fn bit_rate(&self) -> f64 {
        let rs = self.p.symbol_rate;
        match self.p.standard {
            GenStandard::S(r) => useful_bit_rate(rs, r),
            GenStandard::S2 { modcod, short, .. } => match modcod.code(short) {
                Some(c) => {
                    let syms = modcod.frame_symbols(short, false) as f64;
                    rs * (c.kbch - 80) as f64 / syms
                }
                None => 0.0,
            },
        }
    }
}

impl Generator for DvbsGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.fs
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let Some(mut shaper) = self.shaper.take() else {
            return 0;
        };
        while shaper.ready() < out.len() {
            match self.p.standard {
                GenStandard::S(_) => self.s1_packet(),
                GenStandard::S2 { .. } => self.s2_frame(),
            }
            shaper.push(&self.syms);
        }
        let n = shaper.pop(out);
        self.shaper = Some(shaper);
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info(
            self.p.symbol_rate,
            self.p.rolloff,
            self.bit_rate(),
        ))
    }
}

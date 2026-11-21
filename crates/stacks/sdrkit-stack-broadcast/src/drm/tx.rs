//! The DRM transmitter: the multiplex content (one audio service, an optional packet-mode data service with a MOT carousel or
//! Journaline), the FAC, the SDC, multilevel coding, interleaving, the frame layout and OFDM modulation at 48 kHz (mode E:
//! 192 kHz), at a mean power of 1.

use super::audio::{build_superframe, overhead, TextSender};
use super::disperse;
use super::fac::{AppInfo, AudioInfo, Entity, Fac, FacService, Sdc};
use super::interleave::CellInterleaver;
use super::mlc::Mlc;
use super::params::{
    fac_rate, frame_pilot, layout, level_rates, sdc_qam, sdc_rates, CellMap, Qam, Robustness,
};
use sdrkit_algo_modem::ofdm::FrameModulator;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_codec_image::{png, Raster};
use sdrkit_core::Complex32;
use sdrkit_proto_bcdata::datagroup::{self, Header};
use sdrkit_proto_bcdata::journaline::{self, Kind};
use sdrkit_proto_bcdata::mot::{self, HeaderWriter, MotTime};
use sdrkit_proto_bcdata::packet::DrmPacketWriter;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;
use std::collections::VecDeque;

/// What the audio frames carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioKind {
    /// A melody (deterministic frames).
    Melody,
    /// A 1 kHz tone.
    Tone,
    /// Silence.
    Silence,
    /// Noise-like: no two frames alike.
    Noise,
    /// Not AAC: random bytes the receiver must count as damaged.
    NotAac,
}

/// Faults injected into the data service's packets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacketFaults {
    /// Every n-th packet not sent.
    pub drop_every: Option<u32>,
    /// Every n-th packet with a bad CRC.
    pub bad_crc_every: Option<u32>,
    /// Every n-th packet replaced by one of another packet id.
    pub foreign_every: Option<u32>,
}

/// The optional data service.
#[derive(Clone, Debug)]
pub struct DataConfig {
    /// DAB user application type (0x002 slide show, 0x003 website, 0x004 TPEG, 0x007 EPG, 0x44A Journaline).
    pub app_type: u16,
    /// Directory mode (else header mode).
    pub directory: bool,
    /// MOT segment size.
    pub segment_size: usize,
    /// Faults.
    pub faults: PacketFaults,
}

impl Default for DataConfig {
    fn default() -> Self {
        DataConfig {
            app_type: 0x002,
            directory: false,
            segment_size: 120,
            faults: PacketFaults::default(),
        }
    }
}

/// The transmitter's configuration.
#[derive(Clone, Debug)]
pub struct TxConfig {
    /// Robustness mode.
    pub mode: Robustness,
    /// Spectrum occupancy.
    pub occupancy: u8,
    /// MSC constellation.
    pub qam: Qam,
    /// Protection level of part B.
    pub protection: u8,
    /// Short interleaver.
    pub short_interleaver: bool,
    /// SDC mode (0 16-QAM, 1 4-QAM).
    pub sdc_mode: u8,
    /// Audio content.
    pub audio: AudioKind,
    /// Text message on.
    pub text: bool,
    /// The data service.
    pub data: Option<DataConfig>,
    /// Seed.
    pub seed: u64,
}

impl TxConfig {
    /// The usual configuration of a mode.
    pub fn usual(mode: Robustness) -> TxConfig {
        let qam = if mode == Robustness::E {
            Qam::Q16
        } else {
            Qam::Q64
        };
        TxConfig {
            mode,
            occupancy: mode.usual_occupancy(),
            qam,
            protection: usual_protection(mode, qam),
            short_interleaver: false,
            sdc_mode: if mode == Robustness::E { 1 } else { 0 },
            audio: AudioKind::Melody,
            text: true,
            data: None,
            seed: 1,
        }
    }
    /// The MSC mode code of the FAC.
    pub fn msc_mode(&self) -> u8 {
        match (self.mode == Robustness::E, self.qam) {
            (false, Qam::Q16) | (true, Qam::Q4) => 3,
            _ => 0,
        }
    }
}

/// The usual protection: 64-QAM level 1; mode E level 2 for 16-QAM, 1 for 4-QAM; 16-QAM level 1.
pub fn usual_protection(mode: Robustness, q: Qam) -> u8 {
    match (mode == Robustness::E, q) {
        (true, Qam::Q16) => 2,
        (true, _) => 1,
        (false, Qam::Q64) => 1,
        _ => 1,
    }
}

/// The text the test signal sends.
pub const TEST_TEXT: &str = "DRM test signal";

/// The bits of a test audio frame that its CRC covers (clause 5.4.1): its number and length, which take the place of the side
/// information and TNS bits of an AAC frame.
pub const TEST_FRAME_CRC_BITS: usize = 48;

/// The audio frame of global number `n` and `len` bytes (its first 4 bytes are `n`, then the length, then content).
pub fn audio_frame(n: u64, len: usize, kind: AudioKind, seed: u64) -> Vec<u8> {
    let mut f = Vec::with_capacity(len);
    f.extend_from_slice(&(n as u32).to_be_bytes());
    f.extend_from_slice(&(len as u16).to_be_bytes());
    let base = match kind {
        AudioKind::Melody => (n % 16) * 977,
        AudioKind::Tone => 1000,
        AudioKind::Silence => 0,
        AudioKind::Noise | AudioKind::NotAac => n.wrapping_mul(0x9E37_79B9),
    };
    let mut r = Rng::new(base ^ seed.wrapping_mul(31) ^ 0xAAC);
    while f.len() < len {
        f.push(r.next_u64() as u8);
    }
    f.truncate(len);
    f
}

/// Whether received audio frame bytes are exactly a frame [`audio_frame`] made.
pub fn audio_frame_exact(b: &[u8], kind: AudioKind, seed: u64) -> bool {
    if b.len() < 6 {
        return false;
    }
    let n = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64;
    let len = u16::from_be_bytes([b[4], b[5]]) as usize;
    len == b.len() && audio_frame(n, len, kind, seed) == b
}

/// Audio frames per audio super frame.
pub fn audio_frames(mode: Robustness) -> usize {
    if mode == Robustness::E {
        5
    } else {
        10
    }
}

/// The multiplex plan: bytes per logical frame and the stream lengths.
#[derive(Clone, Debug)]
pub struct Plan {
    /// Cells per multiplex frame.
    pub mux_cells: usize,
    /// Bytes per multiplex frame.
    pub mux_bytes: usize,
    /// Stream 0 (audio) bytes.
    pub audio_len: usize,
    /// Stream 1 (data) bytes.
    pub data_len: usize,
    /// Packets per logical frame.
    pub packets: usize,
    /// SDC data field bytes.
    pub sdc_len: usize,
}

/// Packet length of the data service (45 data bytes).
pub const PACKET_LEN: usize = 48;

/// The plan of a configuration.
pub fn plan(cfg: &TxConfig) -> Option<Plan> {
    let map = CellMap::new(cfg.mode, cfg.occupancy)?;
    let (rates, _) = level_rates(cfg.mode, cfg.qam, cfg.protection)?;
    let mux_cells = map.mux_cells();
    let m = Mlc::eep(cfg.qam, rates, mux_cells).ok()?;
    let mux_bytes = m.info_bits() / 8;
    let sq = sdc_qam(cfg.mode, cfg.sdc_mode);
    let sm = Mlc::eep(sq, sdc_rates(cfg.mode, cfg.sdc_mode), map.sdc.len()).ok()?;
    // The SDC block is the AFS index (4 bits), the data field and the CRC-16 (clause 6.4.2).
    let sdc_len = super::fac::sdc_data_len(sm.info_bits());
    let (packets, data_len) = if cfg.data.is_some() && cfg.mode != Robustness::E {
        let p = 12usize.min(mux_bytes / 2 / PACKET_LEN).max(1);
        (p, p * PACKET_LEN)
    } else {
        (0, 0)
    };
    let audio_len = mux_bytes - data_len;
    Some(Plan {
        mux_cells,
        mux_bytes,
        audio_len,
        data_len,
        packets,
        sdc_len,
    })
}

/// The test slides: 32 x 24 PNGs of known pixels.
pub fn test_slide(i: usize) -> Vec<u8> {
    let mut px = Vec::with_capacity(32 * 24 * 3);
    for y in 0..24 {
        for x in 0..32 {
            px.push((x * 8 + i * 40) as u8);
            px.push((y * 10) as u8);
            px.push(((x ^ y) * 7 + i * 3) as u8);
        }
    }
    Raster::from_rgb(32, 24, &px)
        .and_then(|r| png::encode(&r))
        .unwrap_or_default()
}

/// The MOT objects of the test carousel: (transport id, header, body).
pub fn carousel(app: u16) -> Vec<(u16, Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    match app {
        0x002 => {
            for i in 0..3usize {
                let body = test_slide(i);
                let (cat, title) = if i % 2 == 0 {
                    (1u8, "News")
                } else {
                    (2u8, "Weather")
                };
                let mut w = HeaderWriter::new(body.len() as u32, 2, 3)
                    .content_name(&format!("slide{i}.png"))
                    .param(mot::param::CATEGORY_SLIDE, &[cat, 10 + i as u8])
                    .param(mot::param::CATEGORY_TITLE, title.as_bytes())
                    .param(
                        mot::param::CLICK_THROUGH_URL,
                        format!("http://example.org/{i}").as_bytes(),
                    );
                match i {
                    0 => w = w.trigger_time(MotTime::Now),
                    1 => {
                        // 2026-10-06 12:30 UTC (MJD 61319).
                        w = w.trigger_time(MotTime::At {
                            unix_s: (61_319 - 40_587) * 86_400 + 12 * 3600 + 30 * 60,
                            ms: 0,
                        })
                    }
                    _ => {}
                }
                let mut h = Vec::new();
                let _ = w.write(&mut h);
                out.push((100 + i as u16, h, body));
            }
        }
        0x003 | 0x007 => {
            let (name, ctype, sub, body): (&str, u8, u16, &[u8]) = if app == 0x003 {
                (
                    "index.html",
                    1,
                    2,
                    b"<html><body><h1>Test</h1><a href=\"x.html\">link</a></body></html>",
                )
            } else {
                (
                    "epg.xml",
                    7,
                    0,
                    b"<?xml version=\"1.0\"?><epg><programme>Test</programme></epg>",
                )
            };
            let mut h = Vec::new();
            let _ = HeaderWriter::new(body.len() as u32, ctype, sub)
                .content_name(name)
                .write(&mut h);
            out.push((200, h, body.to_vec()));
        }
        _ => {}
    }
    out
}

/// The Journaline objects the test signal sends.
pub fn journaline_pages() -> Vec<journaline::Object> {
    vec![
        journaline::Object {
            id: 0,
            kind: Kind::Menu,
            title: "Main menu".into(),
            links: vec![(1, "News".into()), (2, "Weather".into())],
            ..Default::default()
        },
        journaline::Object {
            id: 1,
            kind: Kind::Text,
            title: "News".into(),
            body: "A test signal is on air.".into(),
            ..Default::default()
        },
        journaline::Object {
            id: 2,
            kind: Kind::List,
            title: "Weather".into(),
            items: vec!["Sunny".into(), "21 C".into()],
            ..Default::default()
        },
    ]
}

/// The extended header length the test signal's Journaline service signals.
pub const JOURNALINE_EXT: u8 = 2;

/// The MSC dummy cell `i` of table 46 (clause 7.7): `a (1 + j)`, then `a (1 - j)`.
pub fn dummy_cell(q: Qam, i: usize) -> Complex32 {
    let a = q.scale();
    Complex32::new(a, if i.is_multiple_of(2) { a } else { -a })
}

/// The DRM transmitter.
pub struct DrmTransmitter {
    cfg: TxConfig,
    plan: Plan,
    map: CellMap,
    modu: FrameModulator,
    ncar: usize,
    fac: Mlc,
    sdc: Mlc,
    msc: Mlc,
    cells: CellInterleaver,
    frame: u64,
    mux: u64,
    audio_n: u64,
    sf_cells: Vec<Complex32>,
    sf_pos: usize,
    text: TextSender,
    pk: DrmPacketWriter,
    pk_queue: VecDeque<Vec<u8>>,
    pk_count: u32,
    unit_queue: VecDeque<Vec<u8>>,
    dg_cont: u8,
    rng: Rng,
    pending_audio: Vec<u8>,
    sdc_rot: usize,
    scratch: Vec<Complex32>,
    /// Audio frames sent so far.
    pub frames_sent: u64,
}

impl DrmTransmitter {
    /// A transmitter; `None` for a configuration the standard does not allow.
    pub fn new(cfg: TxConfig) -> Option<DrmTransmitter> {
        let r = cfg.mode;
        r.range(cfg.occupancy)?;
        let ok_qam = match r {
            Robustness::E => matches!(cfg.qam, Qam::Q16 | Qam::Q4),
            _ => matches!(cfg.qam, Qam::Q64 | Qam::Q16),
        };
        if !ok_qam || (cfg.data.is_some() && r == Robustness::E) {
            return None;
        }
        let plan = plan(&cfg)?;
        let map = CellMap::new(r, cfg.occupancy)?;
        let l = layout(r, cfg.occupancy)?;
        let ncar = l.ncarriers();
        let fac = Mlc::fac(fac_rate(r), r.fac_bits(), r.fac_cells()).ok()?;
        let sq = sdc_qam(r, cfg.sdc_mode);
        let sdc = Mlc::eep(sq, sdc_rates(r, cfg.sdc_mode), map.sdc.len()).ok()?;
        let msc = Mlc::eep(
            cfg.qam,
            level_rates(r, cfg.qam, cfg.protection)?.0,
            plan.mux_cells,
        )
        .ok()?;
        let pk = DrmPacketWriter::new(1, PACKET_LEN).ok()?;
        let depth = if cfg.short_interleaver {
            1
        } else {
            r.long_depth()
        };
        Some(DrmTransmitter {
            cells: CellInterleaver::new(plan.mux_cells, depth),
            frame: 0,
            mux: 0,
            audio_n: 0,
            sf_cells: Vec::new(),
            sf_pos: 0,
            text: TextSender::new(TEST_TEXT, false),
            pk,
            pk_queue: VecDeque::new(),
            pk_count: 0,
            unit_queue: VecDeque::new(),
            dg_cont: 0,
            rng: Rng::new(cfg.seed ^ 0xD4),
            pending_audio: Vec::new(),
            sdc_rot: 0,
            scratch: Vec::new(),
            frames_sent: 0,
            modu: FrameModulator::new(l),
            ncar,
            fac,
            sdc,
            msc,
            map,
            plan,
            cfg,
        })
    }
    /// The plan.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    /// The configuration.
    pub fn config(&self) -> &TxConfig {
        &self.cfg
    }
    /// The OFDM rate.
    pub fn rate(&self) -> f64 {
        self.cfg.mode.rate()
    }
    /// The first logical frame a receiver starting at the beginning can decode (the interleaver pre-roll).
    pub fn first_decodable_frame(&self) -> u64 {
        if self.cfg.short_interleaver {
            0
        } else {
            self.cfg.mode.long_depth() as u64 - 1
        }
    }
    /// Samples per frame.
    pub fn frame_len(&self) -> usize {
        self.modu.layout().frame_len()
    }

    fn label(&self) -> &'static str {
        if self.cfg.mode == Robustness::E {
            "Test DRM+"
        } else {
            "Test DRM"
        }
    }

    fn fac_block(&self, identity: u8, idx: u64) -> Fac {
        let audio = FacService {
            id: 0xE12345,
            short_id: 0,
            audio_ca: false,
            language: 5,
            data: false,
            descriptor: 10,
            data_ca: false,
        };
        let data = FacService {
            id: 0xE12346,
            short_id: 1,
            language: 5,
            data: true,
            descriptor: 0,
            ..Default::default()
        };
        let has_data = self.cfg.data.is_some();
        let services = if self.cfg.mode == Robustness::E {
            vec![audio.clone(), audio]
        } else if has_data && idx % 2 == 1 {
            vec![data]
        } else {
            vec![audio]
        };
        Fac {
            enhancement: false,
            identity,
            rm: self.cfg.mode == Robustness::E,
            occupancy: self.cfg.occupancy,
            short_interleaver: self.cfg.short_interleaver,
            msc_mode: self.cfg.msc_mode(),
            sdc_mode: self.cfg.sdc_mode,
            services_code: super::fac::services_code(1, has_data as u8).unwrap_or(4),
            reconfiguration: 0,
            toggle: false,
            services,
        }
    }

    /// The SDC block as sent (clause 6.4.2), one bit per byte.
    fn sdc_block(&mut self) -> Vec<u8> {
        let elapsed_min = (self.frame as f64 * self.cfg.mode.frame_s() / 60.0) as u32;
        let mut streams = vec![(0u16, self.plan.audio_len as u16)];
        if self.plan.data_len > 0 {
            streams.push((0, self.plan.data_len as u16));
        }
        let mut e = vec![
            Entity::Multiplex {
                next: false,
                prot_a: 0,
                prot_b: self.cfg.protection,
                streams,
            },
            Entity::Label {
                short_id: 0,
                text: self.label().into(),
            },
            Entity::Time {
                mjd: 61319,
                hour: 12 + (elapsed_min / 60) as u8,
                minute: (elapsed_min % 60) as u8,
                offset_half_hours: None,
            },
            Entity::Audio(AudioInfo {
                short_id: 0,
                stream: 0,
                coding: 0,
                sbr: false,
                mode: 0,
                rate_code: 3,
                text: self.cfg.text,
                ..Default::default()
            }),
            Entity::Language {
                short_id: 0,
                language: "eng".into(),
                country: "ae".into(),
            },
        ];
        if let Some(d) = &self.cfg.data {
            e.push(Entity::Label {
                short_id: 1,
                text: "Test Data".into(),
            });
            let mut ud = vec![
                ((d.app_type >> 3) & 0xFF) as u8,
                ((d.app_type & 7) << 5) as u8,
            ];
            if d.app_type == 0x44A {
                ud.push(2); // the Journaline extended header length
            }
            e.push(Entity::Application(AppInfo {
                short_id: 1,
                stream: 1,
                packet_mode: true,
                data_units: true,
                packet_id: 1,
                domain: 1,
                packet_len: (PACKET_LEN - 3) as u8,
                data: ud,
            }));
        }
        // The multiplex description goes in every SDC; the other entities follow in turn, as many as fit.
        let mut chosen = vec![e[0].clone()];
        let rest = &e[1..];
        let mut added = 0;
        for k in 0..rest.len() {
            let cand = rest[(self.sdc_rot + k) % rest.len()].clone();
            let mut trial = chosen.clone();
            trial.push(cand);
            if (Sdc {
                afs: 0,
                entities: trial.clone(),
            })
            .to_bits(self.plan.sdc_len)
            .is_some()
            {
                chosen = trial;
                added += 1;
            }
        }
        self.sdc_rot = (self.sdc_rot + added.max(1)) % rest.len().max(1);
        let s = Sdc {
            afs: 0,
            entities: chosen,
        };
        s.to_bits(self.plan.sdc_len)
            .or_else(|| Sdc::default().to_bits(self.plan.sdc_len))
            .unwrap_or_default()
    }

    fn next_units(&mut self) {
        let Some(d) = self.cfg.data.clone() else {
            return;
        };
        let mut cont = self.dg_cont;
        let mut q = std::mem::take(&mut self.unit_queue);
        // One data group: the header, the segment field when the object is cut, and the transport id.
        let mut group =
            |kind: u8, seg: Option<(bool, u16)>, tid: Option<u16>, data: &[u8], cont: &mut u8| {
                let h = Header {
                    group_type: kind,
                    continuity: *cont,
                    segment: seg,
                    transport_id: tid,
                    user_access: tid.is_some(),
                    ..Header::default()
                };
                *cont = (*cont + 1) & 15;
                let mut g = Vec::new();
                if h.write(data, &mut g).is_ok() {
                    q.push_back(g);
                }
            };
        if d.app_type == 0x44A {
            for p in journaline_pages() {
                let mut b = Vec::new();
                journaline::encode(&p, false, &mut b);
                // The extended header that the SDC signals follows the object header (TS 102 979).
                b.splice(3..3, std::iter::repeat_n(0u8, JOURNALINE_EXT as usize));
                group(datagroup::GENERAL, None, None, &b, &mut cont);
            }
        } else if d.app_type == 0x004 {
            // TPEG: opaque frames in general data groups.
            let mut f = vec![0xFF, 0x0F];
            f.extend((0..60).map(|i| i as u8));
            group(datagroup::GENERAL, None, None, &f, &mut cont);
        } else {
            let objs = carousel(d.app_type);
            let mut part = |kind: u8, tid: u16, data: &[u8], cont: &mut u8| {
                let chunks: Vec<&[u8]> = data.chunks(d.segment_size.max(1)).collect();
                let n = chunks.len();
                for (i, c) in chunks.into_iter().enumerate() {
                    let mut seg = Vec::with_capacity(c.len() + 2);
                    let _ = mot::write_segment(0, c, &mut seg);
                    group(kind, Some((i == n - 1, i as u16)), Some(tid), &seg, cont);
                }
            };
            if d.directory {
                let dir: Vec<(u16, &[u8])> =
                    objs.iter().map(|(t, h, _)| (*t, h.as_slice())).collect();
                let mut b = Vec::new();
                if mot::write_directory(&dir, 0, d.segment_size as u16, &mut b).is_ok() {
                    part(datagroup::MOT_DIRECTORY, 1, &b, &mut cont);
                }
            }
            for (tid, h, b) in &objs {
                if !d.directory {
                    part(datagroup::MOT_HEADER, *tid, h, &mut cont);
                }
                part(datagroup::MOT_BODY, *tid, b, &mut cont);
            }
        }
        self.unit_queue = q;
        self.dg_cont = cont;
    }

    fn data_stream(&mut self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.plan.data_len);
        let faults = self.cfg.data.as_ref().map(|d| d.faults).unwrap_or_default();
        while out.len() < self.plan.data_len {
            if self.pk_queue.is_empty() {
                if self.unit_queue.is_empty() {
                    self.next_units();
                }
                let mut b = Vec::new();
                match self.unit_queue.pop_front() {
                    Some(u) => {
                        let _ = self.pk.write_unit(&u, &mut b);
                    }
                    None => self.pk.write_padding(&mut b),
                }
                self.pk_queue
                    .extend(b.as_chunks::<PACKET_LEN>().0.iter().map(|p| p.to_vec()));
            }
            let mut p = self
                .pk_queue
                .pop_front()
                .expect("a data unit or padding fills at least one packet");
            self.pk_count += 1;
            let c = self.pk_count;
            if faults.drop_every.is_some_and(|n| c.is_multiple_of(n)) {
                continue;
            }
            if faults.bad_crc_every.is_some_and(|n| c.is_multiple_of(n)) {
                p[3] ^= 0x5A;
            }
            if faults.foreign_every.is_some_and(|n| c.is_multiple_of(n)) {
                if let Ok(mut f) = DrmPacketWriter::new(3, PACKET_LEN) {
                    let _ = f.write_unit(&[1, 2, 3], &mut out);
                }
                self.pk_queue.push_front(p);
                continue;
            }
            out.extend_from_slice(&p);
        }
        out.truncate(self.plan.data_len);
        out
    }

    fn audio_part(&mut self, logical: u64) -> Vec<u8> {
        let mode = self.cfg.mode;
        let text_len = if self.cfg.text { 4 } else { 0 };
        let per = self.plan.audio_len - text_len;
        let mut out = Vec::with_capacity(self.plan.audio_len);
        if mode != Robustness::E || logical.is_multiple_of(2) {
            // A new audio super frame (DRM+: carried by this and the next logical frame).
            let total = if mode == Robustness::E { 2 * per } else { per };
            let nf = audio_frames(mode);
            let sf = if self.cfg.audio == AudioKind::NotAac {
                (0..total).map(|_| self.rng.next_u64() as u8).collect()
            } else {
                let avail = total - overhead(nf);
                let mut frames = Vec::with_capacity(nf);
                let mut used = 0;
                for i in 0..nf {
                    // The last frame fills the super frame exactly, as an encoder does.
                    let len = if i == nf - 1 {
                        avail - used
                    } else {
                        avail / nf - 2 + (i % 3)
                    };
                    used += len;
                    frames.push(audio_frame(
                        self.audio_n,
                        len.max(8),
                        self.cfg.audio,
                        self.cfg.seed,
                    ));
                    self.audio_n += 1;
                }
                self.frames_sent += nf as u64;
                build_superframe(&frames, &[TEST_FRAME_CRC_BITS; 10][..nf], total)
                    .expect("frames fit")
            };
            if mode == Robustness::E {
                out.extend_from_slice(&sf[..per]);
                self.pending_audio = sf[per..].to_vec();
            } else {
                out.extend_from_slice(&sf);
            }
        } else {
            out.extend_from_slice(&std::mem::take(&mut self.pending_audio));
            out.resize(per, 0);
        }
        if self.cfg.text {
            out.extend_from_slice(&self.text.next_unit());
        }
        out
    }

    fn logical_frame(&mut self) -> Vec<u8> {
        let n = self.mux;
        let mut b = self.audio_part(n);
        if self.plan.data_len > 0 {
            let d = self.data_stream();
            b.extend_from_slice(&d);
        }
        b.resize(self.plan.mux_bytes, 0);
        b
    }

    fn build_superframe_cells(&mut self) {
        let r = self.cfg.mode;
        let nm = r.frames_per_superframe();
        let ncell = self.plan.mux_cells;
        let q = self.cfg.qam;
        self.sf_cells.clear();
        let mut cells = vec![Complex32::new(0.0, 0.0); ncell];
        for _ in 0..nm {
            let bytes = self.logical_frame();
            let mut bits = Vec::with_capacity(self.msc.info_bits());
            unpack(&bytes, BitOrder::Msb, &mut bits);
            bits.resize(self.msc.info_bits(), 0);
            disperse(&mut bits);
            self.msc.encode(&bits, &mut cells);
            // Cells of frames before the first one (the interleaver's pre-roll) are dummy cells.
            self.cells
                .push(&cells, &mut self.sf_cells, |i| dummy_cell(q, i));
            self.mux += 1;
        }
        // The cells left over in the super frame are the dummy cells of table 46.
        let total = self.map.msc_per_superframe();
        let mut k = 0;
        while self.sf_cells.len() < total {
            self.sf_cells.push(dummy_cell(q, k));
            k += 1;
        }
        self.sf_pos = 0;
    }

    /// Appends the next frame's samples (at the OFDM rate).
    pub fn next_frame(&mut self, out: &mut Vec<Complex32>) {
        let r = self.cfg.mode;
        let nf = r.frames_per_superframe() as u64;
        let fi = self.frame % nf;
        if fi == 0 {
            self.build_superframe_cells();
        }
        // Pilots: time, frequency and gain pilots of the occupancy, and the AFS pilots of mode E (clause 8.4).
        let (lo, _) = r.union_range();
        let occ = self.cfg.occupancy;
        let mut grid = vec![Complex32::new(0.0, 0.0); r.nsym() * self.ncar];
        for s in 0..r.nsym() {
            for c in 0..self.ncar {
                if let Some(v) = frame_pilot(r, occ, fi as usize, s, lo + c as i32) {
                    grid[s * self.ncar + c] = v;
                }
            }
        }
        // FAC.
        let identity = match fi {
            0 => 0,
            x if x == nf - 1 => 2,
            _ => 1,
        };
        let fac = self.fac_block(identity, self.frame);
        let mut fb = fac.to_bits(r);
        disperse(&mut fb);
        let mut fc = vec![Complex32::new(0.0, 0.0); r.fac_cells()];
        self.fac.encode(&fb, &mut fc);
        for (&i, &v) in self.map.fac.iter().zip(&fc) {
            grid[i] = v;
        }
        // SDC.
        if fi == 0 {
            let mut bits = self.sdc_block();
            bits.resize(self.sdc.info_bits(), 0);
            disperse(&mut bits);
            let mut sc = vec![Complex32::new(0.0, 0.0); self.map.sdc.len()];
            self.sdc.encode(&bits, &mut sc);
            for (&i, &v) in self.map.sdc.iter().zip(&sc) {
                grid[i] = v;
            }
        }
        // MSC: the super frame's cells in order (clause 8.6.2).
        for &i in &self.map.msc[fi as usize] {
            grid[i] = self.sf_cells[self.sf_pos];
            self.sf_pos += 1;
        }
        self.scratch.clear();
        self.modu.frame(&grid, &mut self.scratch);
        // Normalise to mean power 1 (the pilot cells are boosted; measured over the frame).
        let p = self.scratch.iter().map(|s| s.norm_sqr()).sum::<f32>() / self.scratch.len() as f32;
        let g = 1.0 / p.max(1e-12).sqrt();
        out.extend(self.scratch.iter().map(|s| s * g));
        self.frame += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurations_build_and_refuse() {
        for r in Robustness::ALL {
            let cfg = TxConfig::usual(r);
            let t = DrmTransmitter::new(cfg).expect("usual configuration");
            assert!(t.plan().audio_len > 100, "{r:?} {:?}", t.plan());
        }
        let mut bad = TxConfig::usual(Robustness::C);
        bad.occupancy = 0;
        assert!(DrmTransmitter::new(bad).is_none());
        let mut bad = TxConfig::usual(Robustness::E);
        bad.qam = Qam::Q64;
        assert!(DrmTransmitter::new(bad).is_none());
    }

    #[test]
    fn frames_have_unit_power_and_length() {
        let mut t = DrmTransmitter::new(TxConfig::usual(Robustness::B)).unwrap();
        let mut x = Vec::new();
        t.next_frame(&mut x);
        assert_eq!(x.len(), 19_200);
        let p: f32 = x.iter().map(|s| s.norm_sqr()).sum::<f32>() / x.len() as f32;
        assert!((p - 1.0).abs() < 1e-3);
    }
}

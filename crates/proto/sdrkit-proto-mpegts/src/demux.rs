//! The packet layer: per-PID continuity, TEI, null packets, scrambling, PCR and statistics. Section and PES assembly for the
//! PIDs routed, service and track selection, the single-service transport stream, and [`Demultiplexer`], all of it as one
//! object.
//!
//! Standard: ISO/IEC 13818-1 2.4.3, 2.4.4.

use crate::codec::{EsCodec, EsKind};
use crate::es::{EsPacket, EsState};
use crate::packet::{self, NULL_PID, TS_SIZE};
use crate::pcr::Stc;
use crate::pes::{PesAssembler, PesPacket};
use crate::si::{list_key, Changes, Service, SiDb};
use crate::subtitle::{SubtitleDecoder, SubtitleDisplay};
use crate::tables;
use crate::teletext::{TeletextDecoder, TeletextPage};
use sdrkit_core::Choice;
use sdrkit_graph::{Packet, PacketPool};
use std::collections::BTreeMap;

pub mod sl;

/// What a routed PID carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The selected video.
    Video,
    /// The selected audio.
    Audio,
    /// The selected teletext.
    Teletext,
    /// The selected DVB subtitles.
    Subtitle,
}

/// A set of routes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouteSet(u8);

impl RouteSet {
    /// No route.
    pub const NONE: RouteSet = RouteSet(0);
    /// Every route.
    pub const ALL: RouteSet = RouteSet(0b1111);

    /// The set with `r` added.
    pub const fn with(self, r: Route) -> RouteSet {
        RouteSet(self.0 | 1 << r as u8)
    }

    /// Whether the set holds `r`.
    pub const fn contains(self, r: Route) -> bool {
        self.0 & 1 << r as u8 != 0
    }
}

/// A complete section as output by the demultiplexer (the typed item `mpeg.section`).
#[derive(Clone, Debug)]
pub struct Section {
    /// The PID it came on.
    pub pid: u16,
    /// The section bytes (header to CRC), shared.
    pub data: Packet,
    /// Arrival time (signal seconds).
    pub time: f64,
}

impl Section {
    /// The section bytes (header to CRC).
    pub fn bytes(&self) -> &[u8] {
        self.data.bytes()
    }
}

impl sdrkit_graph::Item for Section {
    const KIND: &'static str = "mpeg.section";
}

/// Per-PID state of the packet layer, as reported (the typed item `ts.pid`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PidReport {
    /// The PID.
    pub pid: u16,
    /// Packets.
    pub packets: u64,
    /// Continuity errors.
    pub cc_errors: u64,
    /// Packets with transport_error_indicator.
    pub tei: u64,
    /// kbit/s.
    pub kbps: f64,
    /// Examined (4 payload packets seen).
    pub examined: bool,
    /// Scrambled.
    pub scrambled: bool,
    /// PES starts (payload_unit_start on a PES PID).
    pub pes_starts: u64,
    /// The first PTS seen (90 kHz).
    pub first_pts: Option<i64>,
    /// PCRs seen.
    pub pcr_count: u64,
    /// The first PCR (27 MHz).
    pub first_pcr: Option<u64>,
    /// What the stream itself says, when it changed since the last report: (codec, packaging, properties).
    pub stream: Option<(
        crate::codec::EsCodec,
        crate::codec::EsPackaging,
        crate::codec::EsProps,
    )>,
}

impl sdrkit_graph::Item for PidReport {
    const KIND: &'static str = "ts.pid";
}

#[derive(Debug, Default)]
struct PidState {
    seen: bool,
    packets: u64,
    cc_errors: u64,
    tei: u64,
    last_cc: Option<u8>,
    dups: u8,
    accept_next: bool,
    scrambled: u32,
    clear: u32,
    payload_packets: u64,
    bytes: u64,
    bytes_at_rate: u64,
    kbps: f64,
    pes_starts: u64,
    first_pts: Option<i64>,
    pcr_count: u64,
    first_pcr: Option<u64>,
    section: Option<SectionAssembler>,
    pes: Option<PesAssembler>,
}

impl PidState {
    fn examined(&self) -> bool {
        self.payload_packets >= 4
    }
    fn is_scrambled(&self) -> bool {
        self.scrambled >= 4 && self.scrambled > self.clear
    }
}

/// Assembles sections from the payloads of one PID.
#[derive(Clone, Debug, Default)]
pub struct SectionAssembler {
    buf: Vec<u8>,
    active: bool,
    /// Sections dropped (too long, corrupt pointer, continuity).
    pub dropped: u64,
}

impl SectionAssembler {
    /// Drops the section in progress.
    pub fn drop_partial(&mut self) {
        if self.active {
            self.dropped += 1;
        }
        self.active = false;
        self.buf.clear();
    }
    /// One packet's payload.
    pub fn push(&mut self, payload: &[u8], pusi: bool, out: &mut impl FnMut(&[u8])) {
        let mut p = payload;
        if pusi {
            let Some(&ptr) = p.first() else { return };
            let ptr = ptr as usize;
            if 1 + ptr > p.len() {
                self.drop_partial();
                return;
            }
            if self.active {
                self.buf.extend_from_slice(&p[1..1 + ptr]);
                self.complete(out);
            }
            self.active = false;
            self.buf.clear();
            p = &p[1 + ptr..];
            // New sections from the pointed position.
            loop {
                if p.is_empty() || p[0] == 0xFF {
                    return;
                }
                if p.len() < 3 {
                    self.buf.extend_from_slice(p);
                    self.active = true;
                    return;
                }
                let total = 3 + ((p[1] as usize & 0x0F) << 8 | p[2] as usize);
                if total > tables::max_len(p[0]) {
                    self.dropped += 1;
                    return;
                }
                if total <= p.len() {
                    out(&p[..total]);
                    p = &p[total..];
                } else {
                    self.buf.extend_from_slice(p);
                    self.active = true;
                    return;
                }
            }
        } else if self.active {
            self.buf.extend_from_slice(p);
            self.complete(out);
        }
    }
    fn complete(&mut self, out: &mut impl FnMut(&[u8])) {
        if self.buf.len() < 3 {
            return;
        }
        let total = 3 + ((self.buf[1] as usize & 0x0F) << 8 | self.buf[2] as usize);
        if total > tables::max_len(self.buf[0]) {
            self.drop_partial();
            return;
        }
        if self.buf.len() >= total {
            out(&self.buf[..total]);
            // What follows in this packet after a section that ends without PUSI is stuffing or a new section, which only a PUSI
            // packet may start.
            self.active = false;
            self.buf.clear();
        }
    }
}

/// Multiplex statistics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MuxStats {
    /// Packets.
    pub packets: u64,
    /// Null packets.
    pub null_packets: u64,
    /// TEI packets.
    pub tei_packets: u64,
    /// Continuity errors.
    pub cc_errors: u64,
    /// Duplicates discarded.
    pub duplicates: u64,
    /// Multiplex kbit/s.
    pub mux_kbps: f64,
    /// Null kbit/s.
    pub null_kbps: f64,
    /// Losses signalled from below.
    pub losses: u64,
}

/// Signal time for packets that carry none: from the PCR of the first PCR PID (packets × 188 × 8 / mux rate).
#[derive(Clone, Debug, Default)]
struct StreamClock {
    pid: Option<u16>,
    first: Option<(u64, f64)>,
    rate_bps: f64,
}

impl StreamClock {
    fn pcr(&mut self, pid: u16, index: u64, pcr_s: f64) {
        if *self.pid.get_or_insert(pid) != pid {
            return;
        }
        match self.first {
            None => self.first = Some((index, pcr_s)),
            Some((i0, p0)) => {
                let dt = pcr_s - p0;
                if dt > 0.05 && index > i0 {
                    self.rate_bps = (index - i0) as f64 * (TS_SIZE * 8) as f64 / dt;
                } else if dt < 0.0 {
                    self.first = Some((index, pcr_s));
                }
            }
        }
    }
    fn time(&self, index: u64) -> f64 {
        if self.rate_bps > 0.0 {
            index as f64 * (TS_SIZE * 8) as f64 / self.rate_bps
        } else {
            0.0
        }
    }
}

/// What the packet layer produces.
pub trait DemuxSink {
    /// A complete section of a routed section PID.
    fn section(&mut self, pid: u16, data: &[u8], time: f64);
    /// A complete PES of a routed PID.
    fn pes(&mut self, route: Route, pes: PesPacket);
    /// A PCR (27 MHz) of `pid` arriving at `time`.
    fn pcr(&mut self, pid: u16, pcr: u64, time: f64, discontinuity: bool) {
        let _ = (pid, pcr, time, discontinuity);
    }
}

/// The packet layer.
#[derive(Debug)]
pub struct TsDemux {
    pids: Vec<PidState>,
    section_pids: Vec<bool>,
    routes: BTreeMap<u16, Route>,
    loss: bool,
    /// Multiplex statistics.
    pub stats: MuxStats,
    clock: StreamClock,
    last_rate_t: f64,
    mux_bytes_at: (u64, u64),
    /// The signal time of the last packet.
    pub now: f64,
}

impl Default for TsDemux {
    fn default() -> Self {
        Self::new()
    }
}

/// The section PIDs always assembled.
pub const FIXED_SECTION_PIDS: [u16; 7] = [0x0000, 0x0001, 0x0010, 0x0011, 0x0012, 0x0014, 0x1FFB];

impl TsDemux {
    /// A new packet layer, assembling the fixed section PIDs (and the ISDB-T one-segment PMTs).
    pub fn new() -> Self {
        let mut d = TsDemux {
            pids: (0..8192).map(|_| PidState::default()).collect(),
            section_pids: vec![false; 8192],
            routes: BTreeMap::new(),
            loss: false,
            stats: MuxStats::default(),
            clock: StreamClock::default(),
            last_rate_t: 0.0,
            mux_bytes_at: (0, 0),
            now: 0.0,
        };
        for p in FIXED_SECTION_PIDS.iter().copied().chain(0x1FC8..=0x1FCF) {
            d.add_section_pid(p);
        }
        d
    }
    /// Assembles sections on `pid` (a PMT PID, a carousel).
    pub fn add_section_pid(&mut self, pid: u16) {
        let p = pid as usize & 0x1FFF;
        self.section_pids[p] = true;
        self.pids[p]
            .section
            .get_or_insert_with(SectionAssembler::default);
    }
    /// Stops assembling sections on a PID that is not fixed.
    pub fn remove_section_pid(&mut self, pid: u16) {
        if FIXED_SECTION_PIDS.contains(&pid) || (0x1FC8..=0x1FCF).contains(&pid) {
            return;
        }
        let p = pid as usize & 0x1FFF;
        self.section_pids[p] = false;
        self.pids[p].section = None;
    }
    /// Sets the PES routes (the selected service's streams). Assembly stops for PIDs that are not in `routes`.
    pub fn set_routes(&mut self, routes: &BTreeMap<u16, Route>) {
        for &pid in self.routes.keys() {
            if !routes.contains_key(&pid) {
                self.pids[pid as usize].pes = None;
            }
        }
        for &pid in routes.keys() {
            self.pids[pid as usize]
                .pes
                .get_or_insert_with(PesAssembler::default);
        }
        self.routes = routes.clone();
    }
    /// The routes.
    pub fn routes(&self) -> &BTreeMap<u16, Route> {
        &self.routes
    }
    /// Data was lost below (a resync): every counter is accepted unchecked next, partial sections dropped, PES marked damaged.
    pub fn loss(&mut self) {
        self.loss = true;
        self.stats.losses += 1;
    }
    /// Forgets all state and statistics (a new multiplex).
    pub fn reset(&mut self) {
        let routes = self.routes.clone();
        let sections: Vec<u16> = (0..8192u16)
            .filter(|&p| self.section_pids[p as usize])
            .collect();
        *self = TsDemux::new();
        for p in sections {
            self.add_section_pid(p);
        }
        self.set_routes(&routes);
    }

    /// The report of one PID.
    pub fn report(&self, pid: u16) -> PidReport {
        let s = &self.pids[pid as usize & 0x1FFF];
        PidReport {
            pid,
            packets: s.packets,
            cc_errors: s.cc_errors,
            tei: s.tei,
            kbps: s.kbps,
            examined: s.examined(),
            scrambled: s.is_scrambled(),
            pes_starts: s.pes_starts,
            first_pts: s.first_pts,
            pcr_count: s.pcr_count,
            first_pcr: s.first_pcr,
            stream: None,
        }
    }
    /// Reports of every PID seen, in PID order.
    pub fn reports(&self) -> impl Iterator<Item = PidReport> + '_ {
        (0..8192u16)
            .filter(|&p| self.pids[p as usize].seen)
            .map(|p| self.report(p))
    }

    /// One aligned 188-byte packet. `time` is its arrival in signal seconds (`None`: derived from the PCR); `loss` marks data lost
    /// before it.
    pub fn push(&mut self, p: &[u8], time: Option<f64>, loss: bool, sink: &mut impl DemuxSink) {
        if loss {
            self.loss();
        }
        let Some(pk) = packet::parse(p) else { return };
        let h = pk.header;
        self.stats.packets += 1;
        let index = self.stats.packets - 1;
        let now = time.unwrap_or_else(|| self.clock.time(index));
        self.now = now;
        let st = &mut self.pids[h.pid as usize];
        st.seen = true;
        st.packets += 1;
        st.bytes += TS_SIZE as u64;
        if h.pid == NULL_PID {
            self.stats.null_packets += 1;
            self.rates(now);
            return;
        }
        if h.tei {
            st.tei += 1;
            self.stats.tei_packets += 1;
            if let Some(s) = st.section.as_mut() {
                s.drop_partial();
            }
            if let Some(pe) = st.pes.as_mut() {
                pe.damage();
            }
            self.rates(now);
            return;
        }
        if self.loss {
            self.loss = false;
            for s in self.pids.iter_mut() {
                s.accept_next = true;
                if let Some(x) = s.section.as_mut() {
                    x.drop_partial();
                }
                if let Some(pe) = s.pes.as_mut() {
                    pe.damage();
                    pe.discontinuity();
                }
            }
        }
        let st = &mut self.pids[h.pid as usize];
        let disc = pk.adaptation.is_some_and(|a| a.discontinuity);
        if let Some(pcr) = pk.adaptation.and_then(|a| a.pcr) {
            st.pcr_count += 1;
            st.first_pcr.get_or_insert(pcr);
            self.clock.pcr(h.pid, index, packet::pcr_seconds(pcr));
            sink.pcr(h.pid, pcr, now, disc);
        }
        let st = &mut self.pids[h.pid as usize];
        if h.has_payload() {
            // Continuity.
            let mut duplicate = false;
            match st.last_cc {
                Some(last) if !st.accept_next && !disc => {
                    if h.cc == last {
                        st.dups = st.dups.saturating_add(1);
                        if st.dups == 1 {
                            duplicate = true;
                            self.stats.duplicates += 1;
                        } else {
                            st.cc_errors += 1;
                            self.stats.cc_errors += 1;
                            cc_error(st);
                        }
                    } else if h.cc == (last + 1) & 15 {
                        st.dups = 0;
                    } else {
                        st.cc_errors += 1;
                        self.stats.cc_errors += 1;
                        st.dups = 0;
                        cc_error(st);
                    }
                }
                _ => st.dups = 0,
            }
            st.last_cc = Some(h.cc);
            st.accept_next = false;
            if duplicate {
                self.rates(now);
                return;
            }
            // Scrambling, on payload packets only.
            st.payload_packets += 1;
            if h.scrambling != 0 {
                st.scrambled += 1;
            } else {
                st.clear += 1;
            }
            if st.scrambled + st.clear >= 512 {
                st.scrambled /= 2;
                st.clear /= 2;
            }
            if h.scrambling == 0 {
                if h.pusi
                    && pk.payload.len() >= 3
                    && pk.payload[..3] == [0, 0, 1]
                    && !self.section_pids[h.pid as usize]
                {
                    st.pes_starts += 1;
                    if st.first_pts.is_none() {
                        if let Some((_, Some(pts), _, _, _)) = crate::pes::parse_header(pk.payload)
                        {
                            st.first_pts = Some(pts);
                        }
                    }
                }
                if let Some(sa) = st.section.as_mut() {
                    let pid = h.pid;
                    sa.push(pk.payload, h.pusi, &mut |s| sink.section(pid, s, now));
                }
                let st = &mut self.pids[h.pid as usize];
                if let (Some(pe), Some(&route)) = (st.pes.as_mut(), self.routes.get(&h.pid)) {
                    let ra = pk.adaptation.is_some_and(|a| a.random_access);
                    pe.push(h.pid, pk.payload, h.pusi, ra, now, &mut |x| {
                        sink.pes(route, x)
                    });
                }
            }
        }
        self.rates(now);
    }

    /// Ends every PES in progress (end of a file).
    pub fn flush(&mut self, sink: &mut impl DemuxSink) {
        for (&pid, &route) in self.routes.iter() {
            if let Some(pe) = self.pids[pid as usize].pes.as_mut() {
                pe.flush(pid, &mut |x| sink.pes(route, x));
            }
        }
    }

    fn rates(&mut self, now: f64) {
        let dt = now - self.last_rate_t;
        if dt < 0.5 {
            if dt < 0.0 {
                self.last_rate_t = now;
            }
            return;
        }
        for s in self.pids.iter_mut().filter(|s| s.seen) {
            let k = (s.bytes - s.bytes_at_rate) as f64 * 8.0 / dt / 1000.0;
            s.kbps = if s.kbps == 0.0 {
                k
            } else {
                0.65 * s.kbps + 0.35 * k
            };
            s.bytes_at_rate = s.bytes;
        }
        let (p0, n0) = self.mux_bytes_at;
        let k = (self.stats.packets - p0) as f64 * 1.504 / dt;
        let kn = (self.stats.null_packets - n0) as f64 * 1.504 / dt;
        self.stats.mux_kbps = if self.stats.mux_kbps == 0.0 {
            k
        } else {
            0.65 * self.stats.mux_kbps + 0.35 * k
        };
        self.stats.null_kbps = if self.stats.null_kbps == 0.0 {
            kn
        } else {
            0.65 * self.stats.null_kbps + 0.35 * kn
        };
        self.mux_bytes_at = (self.stats.packets, self.stats.null_packets);
        self.last_rate_t = now;
    }
}

fn cc_error(st: &mut PidState) {
    if let Some(s) = st.section.as_mut() {
        s.drop_partial();
    }
    if let Some(p) = st.pes.as_mut() {
        p.damage();
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Service selection.

/// The user's choices for a service's tracks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    /// Preferred audio language (ISO 639-2).
    pub audio_lang: String,
    /// Audio description is requested.
    pub audio_description: bool,
    /// Preferred subtitle language; empty: subtitles off.
    pub subtitle_lang: String,
    /// Hard-of-hearing subtitles before normal ones.
    pub hard_of_hearing: bool,
}

/// What is selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// The service wished for (it stays a wish until its PMT is known).
    pub service: Option<u16>,
    /// (onid, tsid, name) of the wish, to find it again on a new multiplex.
    pub wish: Option<(u16, u16, String)>,
    /// The video PID chosen.
    pub video: Option<u16>,
    /// The audio PID chosen.
    pub audio: Option<u16>,
    /// The audio PID requested by the user.
    pub audio_override: Option<u16>,
    /// The subtitle PID and pages (composition, ancillary).
    pub subtitle: Option<(u16, u16, u16)>,
    /// A subtitle choice the user made: Some(None) off, Some(Some(..)) this one.
    pub subtitle_override: Option<Option<(u16, u16, u16)>>,
    /// The teletext PID.
    pub teletext: Option<u16>,
    /// The PCR PID.
    pub pcr: Option<u16>,
    /// The PMT PID.
    pub pmt: Option<u16>,
    /// What the player does.
    pub status: PlayerStatus,
}

sdrkit_core::choice_enum! {
    /// What the player does.
    #[derive(Default)]
    pub enum PlayerStatus {
        /// A service is chosen and its PMT has not come yet.
        Starting = ("starting", "Starting"),
        /// The chosen service plays.
        Playing = ("playing", "Playing"),
        /// The chosen service is scrambled.
        Scrambled = ("scrambled", "Scrambled"),
        /// No service is chosen.
        #[default]
        NoService = ("no_service", "No service"),
    }
}

/// Chooses the tracks of a service.
pub fn choose_tracks(s: &crate::si::Service, prefs: &Preferences, sel: &mut Selection) {
    let videos: Vec<&crate::si::StreamEntry> = s
        .streams
        .iter()
        .filter(|e| e.kind == Some(EsKind::Video))
        .collect();
    sel.video = videos
        .iter()
        .find(|e| videos.len() > 1 && e.pid == s.pcr_pid)
        .or(videos.first())
        .map(|e| e.pid);
    let audios: Vec<&crate::si::StreamEntry> = s
        .streams
        .iter()
        .filter(|e| e.kind == Some(EsKind::Audio))
        .collect();
    let main = |e: &&crate::si::StreamEntry| {
        prefs.audio_description
            || (e.audio_type != 3
                && e.supplementary
                    .as_ref()
                    .is_none_or(|x| x.mix_type == 1 && x.editorial == 0))
    };
    let lang_match = audios.iter().find(|e| {
        main(e)
            && !prefs.audio_lang.is_empty()
            && e.languages
                .iter()
                .any(|l| crate::epg::same_lang(&l.0, &prefs.audio_lang))
    });
    sel.audio = match sel
        .audio_override
        .filter(|p| audios.iter().any(|e| e.pid == *p))
    {
        Some(p) => Some(p),
        None => lang_match
            .or(audios.iter().find(|e| main(e)))
            .or(audios.first())
            .map(|e| e.pid),
    };
    sel.teletext = s
        .streams
        .iter()
        .find(|e| e.kind == Some(EsKind::Teletext))
        .map(|e| e.pid);
    sel.subtitle = match &sel.subtitle_override {
        Some(c) => *c,
        None if prefs.subtitle_lang.is_empty() => None,
        None => {
            let mut best: Option<(u8, (u16, u16, u16))> = None;
            for e in s
                .streams
                .iter()
                .filter(|e| e.kind == Some(EsKind::Subtitles))
            {
                for sub in &e.subtitles {
                    if !crate::epg::same_lang(&sub.lang, &prefs.subtitle_lang) {
                        continue;
                    }
                    let hoh = (0x20..=0x24).contains(&sub.kind);
                    let rank = if hoh == prefs.hard_of_hearing { 2 } else { 1 };
                    if best.is_none_or(|b| rank > b.0) {
                        best = Some((rank, (e.pid, sub.composition_page, sub.ancillary_page)));
                    }
                }
            }
            best.map(|b| b.1)
        }
    };
    sel.pcr = (s.pcr_pid != 0 && s.pcr_pid != 0x1FFF).then_some(s.pcr_pid);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Single-service transport stream.

/// Passes one service as a transport stream with a rewritten PAT.
#[derive(Clone, Debug, Default)]
pub struct TsFilter {
    /// Also pass the SDT, EIT and TDT/TOT (for an external player); off for recording.
    pub with_si: bool,
    pids: Vec<u16>,
    service: u16,
    pmt_pid: u16,
    tsid: u16,
    cc: u8,
    version: u8,
    pat: Vec<[u8; TS_SIZE]>,
}

impl TsFilter {
    /// A filter (nothing passes until a service is set).
    pub fn new(with_si: bool) -> Self {
        TsFilter {
            with_si,
            ..Default::default()
        }
    }
    /// The service to pass: its id, PMT PID, PCR PID and ES PIDs, and the original tsid.
    pub fn set_service(&mut self, sid: u16, pmt_pid: u16, pcr_pid: u16, es: &[u16], tsid: u16) {
        let mut pids = vec![pmt_pid];
        if pcr_pid != 0x1FFF && pcr_pid != 0 {
            pids.push(pcr_pid);
        }
        pids.extend_from_slice(es);
        pids.sort_unstable();
        pids.dedup();
        if (sid, pmt_pid, tsid) != (self.service, self.pmt_pid, self.tsid) {
            self.version = (self.version + 1) & 31;
        }
        self.pids = pids;
        self.service = sid;
        self.pmt_pid = pmt_pid;
        self.tsid = tsid;
        let pat = tables::build_pat(tsid, self.version, &[(sid, pmt_pid)]);
        let mut cc = 0;
        self.pat = packet::packetize_section(&pat, 0, &mut cc);
    }
    /// Filters one packet: appends what passes to `out` (a PAT start becomes a generated single-programme PAT).
    pub fn push(&mut self, p: &[u8], out: &mut Vec<[u8; TS_SIZE]>) {
        let Some(h) = packet::header(p) else { return };
        if self.service == 0 {
            return;
        }
        if h.pid == 0 {
            if h.pusi {
                for pk in &self.pat {
                    let mut pk = *pk;
                    pk[3] = pk[3] & 0xF0 | self.cc;
                    self.cc = (self.cc + 1) & 15;
                    out.push(pk);
                }
            }
            return;
        }
        let si = self.with_si && matches!(h.pid, 0x11 | 0x12 | 0x14);
        if si || self.pids.binary_search(&h.pid).is_ok() {
            let mut q = [0u8; TS_SIZE];
            q.copy_from_slice(&p[..TS_SIZE]);
            out.push(q);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The whole demultiplexer.

/// What one call of [`Demultiplexer::push`] produced.
#[derive(Debug, Default)]
pub struct DemuxOutput {
    /// Media packets of the selected video and audio.
    pub media: Vec<EsPacket>,
    /// Guide events stored.
    pub events: Vec<crate::epg::EpgEvent>,
    /// Teletext pages completed.
    pub pages: Vec<TeletextPage>,
    /// Subtitle display sets.
    pub subtitles: Vec<SubtitleDisplay>,
    /// The single-service transport stream.
    pub filtered: Vec<[u8; TS_SIZE]>,
    /// The arrival time of each packet of `filtered`.
    pub filtered_times: Vec<f64>,
    /// The service list changed.
    pub services_changed: bool,
    /// UTC was received.
    pub time_changed: bool,
    /// Log lines.
    pub log: Vec<String>,
    /// Routed PES of the routes in [`Demultiplexer::forward_pes`] (instead of decoding them).
    pub pes: Vec<(Route, PesPacket)>,
    /// Complete sections, when [`Demultiplexer::forward_sections`] is set.
    pub sections: Vec<Section>,
}

impl DemuxOutput {
    /// Clears everything, keeping the allocations.
    pub fn clear(&mut self) {
        self.media.clear();
        self.events.clear();
        self.pages.clear();
        self.subtitles.clear();
        self.filtered.clear();
        self.filtered_times.clear();
        self.services_changed = false;
        self.time_changed = false;
        self.log.clear();
        self.pes.clear();
        self.sections.clear();
    }
}

/// The demultiplexer as one object: packet layer, service information, selection, elementary streams, teletext, subtitles and the
/// single-service stream.
pub struct Demultiplexer {
    /// The packet layer.
    pub ts: TsDemux,
    /// The service information.
    pub si: SiDb,
    /// Track preferences.
    pub prefs: Preferences,
    /// The selection.
    pub selection: Selection,
    /// Automatically select the first playable service when nothing is selected.
    pub auto_select: bool,
    /// The routes whose PES packets are passed on (in [`DemuxOutput::pes`]) instead of decoded here.
    pub forward_pes: RouteSet,
    /// Hand complete sections on (in [`DemuxOutput::sections`]).
    pub forward_sections: bool,
    /// Parse every audio and video stream of every service (to describe them in the service list), not only the selected ones.
    pub probe_all: bool,
    es: BTreeMap<u16, EsState>,
    stc: BTreeMap<u16, Stc>,
    /// Teletext of the selected service.
    pub teletext: TeletextDecoder,
    /// DVB subtitles of the selected service.
    pub subtitles: Option<SubtitleDecoder>,
    /// The single-service stream.
    pub filter: TsFilter,
    applied_version: u64,
    warned: Vec<u16>,
    /// The PIDs whose codec or properties changed since the last [`Demultiplexer::reports`].
    unreported: Vec<u16>,
    last_tick: f64,
    sections: PacketPool,
    pes_buf: Vec<(Route, PesPacket)>,
    pcr_buf: Vec<(u16, u64, f64, bool)>,
}

impl Default for Demultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

struct Sink<'a> {
    si: &'a mut SiDb,
    out: &'a mut DemuxOutput,
    forward_sections: bool,
    sections: &'a mut PacketPool,
    pes: &'a mut Vec<(Route, PesPacket)>,
    pcr: &'a mut Vec<(u16, u64, f64, bool)>,
}

impl DemuxSink for Sink<'_> {
    fn section(&mut self, pid: u16, data: &[u8], time: f64) {
        if self.forward_sections {
            self.out.sections.push(Section {
                pid,
                data: self
                    .sections
                    .make(data.len(), time, |b| b.copy_from_slice(data)),
                time,
            });
        }
        let ch: Changes = self.si.push_section(pid, data, time);
        self.out.services_changed |= ch.services;
        self.out.time_changed |= ch.time;
        self.out.events.extend(ch.events);
    }
    fn pes(&mut self, route: Route, pes: PesPacket) {
        self.pes.push((route, pes));
    }
    fn pcr(&mut self, pid: u16, pcr: u64, time: f64, d: bool) {
        self.pcr.push((pid, pcr, time, d));
    }
}

impl Demultiplexer {
    /// A new demultiplexer.
    pub fn new() -> Self {
        Demultiplexer {
            ts: TsDemux::new(),
            si: SiDb::new(),
            prefs: Preferences::default(),
            selection: Selection::default(),
            auto_select: false,
            forward_pes: RouteSet::NONE,
            forward_sections: false,
            probe_all: false,
            es: BTreeMap::new(),
            stc: BTreeMap::new(),
            teletext: TeletextDecoder::new(),
            subtitles: None,
            filter: TsFilter::new(false),
            applied_version: u64::MAX,
            warned: Vec::new(),
            unreported: Vec::new(),
            last_tick: 0.0,
            sections: PacketPool::default(),
            pes_buf: Vec::new(),
            pcr_buf: Vec::new(),
        }
    }

    /// Selects a service (`None`: nothing). Media restarts ("starting") and waits for its PMT.
    pub fn select(&mut self, sid: Option<u16>) {
        self.selection.service = sid;
        self.selection.wish = sid
            .and_then(|s| self.si.service(s))
            .map(|s| (s.onid, s.tsid, s.name.clone()));
        self.selection.video = None;
        self.selection.audio = None;
        self.selection.audio_override = None;
        self.selection.subtitle = None;
        self.selection.teletext = None;
        self.selection.status = if sid.is_some() {
            PlayerStatus::Starting
        } else {
            PlayerStatus::NoService
        };
        self.es.clear();
        self.teletext.clear();
        self.subtitles = None;
        self.applied_version = u64::MAX;
        self.apply_selection();
    }

    /// Switches the audio track (only the audio PID changes).
    pub fn set_audio(&mut self, pid: Option<u16>) {
        self.selection.audio_override = pid;
        self.applied_version = u64::MAX;
        self.apply_selection();
    }

    /// Chooses subtitles: `Some((pid, composition, ancillary))`, or `None` for off.
    pub fn set_subtitle(&mut self, choice: Option<(u16, u16, u16)>) {
        self.selection.subtitle_override = Some(choice);
        self.applied_version = u64::MAX;
        self.apply_selection();
    }

    /// A retune: a move of more than 100 kHz (or another stack) is a new multiplex; a smaller one keeps everything.
    pub fn retuned(&mut self, moved_hz: f64, stack_changed: bool) {
        if moved_hz.abs() > 100e3 || stack_changed {
            let wish = self.selection.wish.clone().or_else(|| {
                self.selection
                    .service
                    .map(|s| (0, 0, format!("Service {s}")))
            });
            self.si.clear_multiplex();
            self.ts.reset();
            let keep = self.selection.service;
            self.select(None);
            self.selection.wish = wish;
            self.selection.service = keep;
            self.selection.status = if keep.is_some() {
                PlayerStatus::Starting
            } else {
                PlayerStatus::NoService
            };
            self.es.clear();
        }
        self.ts.loss();
    }

    /// The automatic choice: the first service that can play when nothing is selected, and the next one when the chosen
    /// service turns out scrambled (by free_CA_mode of the SDT or the scrambling bits of its packets). A scrambled service
    /// stays chosen while no other can play.
    fn choose_automatically(&mut self) {
        let scrambled = |sid: u16| self.si.service(sid).is_some_and(|s| s.scrambled);
        match self.selection.service {
            None => self.selection.service = self.si.first_playable(),
            Some(sid) if scrambled(sid) => {
                if let Some(next) = self.si.first_playable() {
                    self.selection.service = Some(next);
                    self.selection.video = None;
                    self.selection.audio = None;
                    self.selection.audio_override = None;
                    self.selection.subtitle = None;
                    self.selection.teletext = None;
                    self.selection.status = PlayerStatus::Starting;
                    self.teletext.clear();
                    self.subtitles = None;
                }
            }
            Some(_) => {}
        }
    }

    fn apply_selection(&mut self) {
        if self.applied_version == self.si.version {
            return;
        }
        self.applied_version = self.si.version;
        // Section PIDs: every PMT PID the PAT announces.
        let pmts: Vec<u16> = self.si.pmt_pids().collect();
        for p in pmts {
            self.ts.add_section_pid(p);
        }
        // A wish from another multiplex: the same onid/tsid/sid, or else the same name.
        if let (Some(sid), Some((onid, tsid, name))) =
            (self.selection.service, self.selection.wish.clone())
        {
            let exact = self.si.service(sid).is_some_and(|s| {
                (onid == 0 || s.onid == onid || s.onid == 0)
                    && (tsid == 0 || s.tsid == tsid || s.tsid == 0)
            });
            if !exact {
                if let Some(s) = self.si.services().into_iter().find(|s| s.name == name) {
                    self.selection.service = Some(s.service_id);
                }
            }
        }
        if self.auto_select {
            self.choose_automatically();
        }
        let mut routes = BTreeMap::new();
        if self.probe_all {
            for s in self.si.services() {
                for e in &s.streams {
                    match e.kind {
                        Some(EsKind::Video) => routes.insert(e.pid, Route::Video),
                        Some(EsKind::Audio) => routes.insert(e.pid, Route::Audio),
                        _ => None,
                    };
                }
            }
        }
        let Some(sid) = self.selection.service else {
            self.ts.set_routes(&routes);
            return;
        };
        let Some(svc) = self.si.service(sid).cloned() else {
            self.ts.set_routes(&routes);
            return;
        };
        self.selection.pmt = (svc.pmt_pid != 0).then_some(svc.pmt_pid);
        if !svc.have_pmt {
            self.selection.status = PlayerStatus::Starting;
            self.ts.set_routes(&routes);
            return;
        }
        let before = self.selection.clone();
        choose_tracks(&svc, &self.prefs, &mut self.selection);
        let s = &self.selection;
        for (p, r) in [
            (s.video, Route::Video),
            (s.audio, Route::Audio),
            (s.teletext, Route::Teletext),
            (s.subtitle.map(|x| x.0), Route::Subtitle),
        ] {
            if let Some(p) = p {
                routes.insert(p, r);
            }
        }
        // New PIDs reset their stream; the same PIDs keep flowing.
        if before.audio != s.audio {
            if let Some(a) = before.audio {
                if self.probe_all {
                    if let Some(e) = self.es.get_mut(&a) {
                        e.reset_time();
                    }
                } else {
                    self.es.remove(&a);
                }
            }
        }
        self.es.retain(|pid, _| routes.contains_key(pid));
        if before.teletext != s.teletext {
            self.teletext.clear();
        }
        if before.subtitle != s.subtitle {
            self.subtitles = s.subtitle.map(|(_, c, a)| SubtitleDecoder::new(c, a));
        }
        let es_pids: Vec<u16> = svc.streams.iter().map(|e| e.pid).collect();
        self.filter
            .set_service(sid, svc.pmt_pid, svc.pcr_pid, &es_pids, self.si.tsid);
        self.selection.status = if svc.scrambled {
            PlayerStatus::Scrambled
        } else {
            PlayerStatus::Playing
        };
        self.ts.set_routes(&routes);
    }

    /// One aligned packet. `time`: arrival in signal seconds (`None` derives it from the PCR); `loss`: data lost before it.
    pub fn push(&mut self, p: &[u8], time: Option<f64>, loss: bool, out: &mut DemuxOutput) {
        let mut pes = std::mem::take(&mut self.pes_buf);
        let mut pcr = std::mem::take(&mut self.pcr_buf);
        let mut sink = Sink {
            si: &mut self.si,
            out,
            forward_sections: self.forward_sections,
            sections: &mut self.sections,
            pes: &mut pes,
            pcr: &mut pcr,
        };
        self.ts.push(p, time, loss, &mut sink);
        for (pid, v, t, d) in pcr.drain(..) {
            self.stc.entry(pid).or_default().push(v, t, d);
        }
        self.pcr_buf = pcr;
        let n = out.filtered.len();
        self.filter.push(p, &mut out.filtered);
        let t = self.ts.now;
        out.filtered_times.resize(out.filtered.len().max(n), t);
        if self.si.version != self.applied_version {
            self.apply_selection();
        }
        for (route, pe) in pes.drain(..) {
            self.route(route, pe, out);
        }
        self.pes_buf = pes;
        let now = self.ts.now;
        if now - self.last_tick >= 1.0 || now < self.last_tick {
            self.last_tick = now;
            self.tick(now);
        }
        out.log.append(&mut self.si.log);
    }

    /// Ends the PES in progress (end of a file).
    pub fn flush(&mut self, out: &mut DemuxOutput) {
        let mut pes = std::mem::take(&mut self.pes_buf);
        let mut pcr = std::mem::take(&mut self.pcr_buf);
        let mut sink = Sink {
            si: &mut self.si,
            out,
            forward_sections: self.forward_sections,
            sections: &mut self.sections,
            pes: &mut pes,
            pcr: &mut pcr,
        };
        self.ts.flush(&mut sink);
        pcr.clear();
        self.pcr_buf = pcr;
        for (route, pe) in pes.drain(..) {
            self.route(route, pe, out);
        }
        self.pes_buf = pes;
        // The access unit each video stream holds back for joining.
        let video = self.selection.video;
        for (pid, es) in self.es.iter_mut() {
            es.flush(&mut |m| {
                if video == Some(*pid) {
                    out.media.push(m)
                }
            });
        }
        self.tick(self.ts.now);
    }

    fn tick(&mut self, now: f64) {
        for r in self.ts.reports() {
            self.si.set_scrambling(r.pid, r.examined, r.scrambled);
            self.si.set_stream(r.pid, None, None, Some(r.kbps));
        }
        self.si.tick(now);
        self.si.refresh_now_next(now);
    }

    fn route(&mut self, route: Route, pe: PesPacket, out: &mut DemuxOutput) {
        let selected = match route {
            Route::Video => self.selection.video == Some(pe.pid),
            Route::Audio => self.selection.audio == Some(pe.pid),
            Route::Teletext => self.selection.teletext == Some(pe.pid),
            Route::Subtitle => self.selection.subtitle.map(|s| s.0) == Some(pe.pid),
        };
        let sid = match self.selection.service {
            Some(s) if selected => s,
            _ => match self.si.service_of_pid(pe.pid) {
                Some(s) => s,
                None => return,
            },
        };
        let pcr_pid = self.selection.pcr;
        if selected && self.forward_pes.contains(route) {
            let mut pe = pe;
            if let Some(st) = self
                .si
                .service(sid)
                .and_then(|s| s.streams.iter().find(|e| e.pid == pe.pid))
            {
                pe.info = Some(st.codec_info());
            }
            pe.aac_profile_level = self
                .si
                .service(sid)
                .and_then(|s| s.streams.iter().find(|e| e.pid == pe.pid))
                .and_then(|e| e.aac_profile_level);
            pe.pages = self
                .selection
                .subtitle
                .filter(|s| s.0 == pe.pid)
                .map(|s| (s.1, s.2));
            pe.stc = pcr_pid
                .and_then(|p| self.stc.get(&p))
                .and_then(|c| c.predict(pe.time));
            pe.discontinuity |= pcr_pid
                .and_then(|p| self.stc.get_mut(&p))
                .is_some_and(|c| std::mem::take(&mut c.jumped));
            out.pes.push((route, pe));
            return;
        }
        match route {
            Route::Video | Route::Audio => {
                let Some(svc) = self.si.service(sid) else {
                    return;
                };
                let Some(st) = svc.streams.iter().find(|e| e.pid == pe.pid) else {
                    return;
                };
                let pid = pe.pid;
                let es = self.es.entry(pid).or_insert_with(|| {
                    let mut e = EsState::new(pid, st.codec_info());
                    e.aac_profile_level = st.aac_profile_level;
                    e
                });
                let stc = pcr_pid
                    .and_then(|p| self.stc.get(&p))
                    .and_then(|c| c.predict(pe.time));
                let jumped = pcr_pid
                    .and_then(|p| self.stc.get_mut(&p))
                    .is_some_and(|c| std::mem::take(&mut c.jumped));
                let mut pe = pe;
                pe.discontinuity |= jumped;
                es.push(&pe, stc, &mut |m| {
                    if selected {
                        out.media.push(m)
                    }
                });
                if let Some((signalled, carried)) = es.warning.take() {
                    if !self.warned.contains(&pid) {
                        self.warned.push(pid);
                        out.log.push(format!(
                            "PID 0x{pid:04X} is signalled as {signalled:?} but carries {carried:?}"
                        ));
                    }
                }
                let changed = self.si.set_stream(
                    pid,
                    Some((es.info.codec, es.info.packaging)),
                    Some(&es.props),
                    None,
                );
                if changed && !self.unreported.contains(&pid) {
                    self.unreported.push(pid);
                }
            }
            Route::Teletext if selected => {
                self.teletext.push_pes(pe.payload.bytes());
                for (n, sub) in self.teletext.take_completed() {
                    if let Some(p) = self.teletext.page(n, Some(sub)) {
                        out.pages.push(p);
                    }
                }
            }
            Route::Subtitle if selected => {
                if let Some(d) = self.subtitles.as_mut() {
                    out.subtitles.extend(d.push_pes(pe.payload.bytes(), pe.pts));
                }
            }
            _ => {}
        }
    }

    /// Reports of every PID seen. A report holds what its stream says only in the first report after the codec or the
    /// properties of the stream changed.
    pub fn reports(&mut self) -> impl Iterator<Item = PidReport> + '_ {
        let (es, unreported) = (&self.es, &mut self.unreported);
        self.ts.reports().map(move |mut r| {
            if let Some(k) = unreported.iter().position(|&p| p == r.pid) {
                unreported.swap_remove(k);
                if let Some(e) = es.get(&r.pid) {
                    r.stream = Some((e.info.codec, e.info.packaging, e.props.clone()));
                }
            }
            r
        })
    }

    /// The STC (seconds) of the selected service at `time`.
    pub fn stc(&self, time: f64) -> Option<f64> {
        self.selection
            .pcr
            .and_then(|p| self.stc.get(&p))
            .and_then(|c| c.predict(time))
    }

    /// The label of a PID for the statistics, for example "PAT" or "News audio (MPEG-1 layer II)".
    pub fn pid_label(&self, pid: u16) -> PidLabel<'_> {
        let fixed = match pid {
            0x0000 => Some("PAT"),
            0x0001 => Some("CAT"),
            0x0010 => Some("NIT"),
            0x0011 => Some("SDT/BAT"),
            0x0012 => Some("EIT"),
            0x0013 => Some("RST"),
            0x0014 => Some("TDT/TOT"),
            0x001E => Some("DIT"),
            0x001F => Some("SIT"),
            0x1FFF => Some("null"),
            _ => None,
        };
        if let Some(t) = fixed {
            return PidLabel::Fixed(t);
        }
        let uses = |s: &&Service| {
            s.pmt_pid == pid || s.pcr_pid == pid || s.streams.iter().any(|e| e.pid == pid)
        };
        if let Some(s) = self.si.known().filter(uses).min_by_key(|s| list_key(s)) {
            if s.pmt_pid == pid {
                return PidLabel::Pmt(&s.name);
            }
            if let Some(e) = s.streams.iter().find(|e| e.pid == pid) {
                return PidLabel::Stream(&s.name, e.kind.unwrap_or(EsKind::Data), e.codec);
            }
            return PidLabel::Pcr(&s.name);
        }
        if self.si.nit_pid == pid {
            return PidLabel::Fixed("NIT");
        }
        PidLabel::Other(pid)
    }
}

/// The label of a PID, from [`Demultiplexer::pid_label`]. It is written out with [`std::fmt::Display`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PidLabel<'a> {
    /// A PID of a table with a fixed PID, or the NIT.
    Fixed(&'static str),
    /// The PMT of the named service.
    Pmt(&'a str),
    /// An elementary stream of the named service: its kind and, when known, its codec.
    Stream(&'a str, EsKind, Option<EsCodec>),
    /// The PCR PID of the named service, when it carries no elementary stream of it.
    Pcr(&'a str),
    /// A PID that no table names.
    Other(u16),
}

impl std::fmt::Display for PidLabel<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = |c: &'static [sdrkit_core::value::ChoiceDecl], i: u16| -> &'static str {
            &c[usize::from(i)].label
        };
        match *self {
            PidLabel::Fixed(t) => f.write_str(t),
            PidLabel::Pmt(name) => write!(f, "PMT {name}"),
            PidLabel::Stream(name, kind, codec) => {
                write!(f, "{name} {}", label(EsKind::CHOICES, kind.index()))?;
                match codec {
                    Some(c) => write!(f, " ({})", label(EsCodec::CHOICES, c.index())),
                    None => Ok(()),
                }
            }
            PidLabel::Pcr(name) => write!(f, "{name} PCR"),
            PidLabel::Other(pid) => write!(f, "PID 0x{pid:04X}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{packetize_section, write_header, Header};
    use crate::tables::{build_pat, build_pmt, descriptor};

    #[derive(Default)]
    struct Collect {
        sections: Vec<(u16, Vec<u8>)>,
    }
    impl DemuxSink for Collect {
        fn section(&mut self, pid: u16, d: &[u8], _: f64) {
            self.sections.push((pid, d.to_vec()));
        }
        fn pes(&mut self, _: Route, _: PesPacket) {}
    }

    fn pkt(pid: u16, cc: u8, pusi: bool, disc: bool, payload: &[u8]) -> [u8; 188] {
        let mut p = [0xFFu8; 188];
        if disc {
            write_header(
                &mut p,
                &Header {
                    pid,
                    cc,
                    pusi,
                    afc: 3,
                    ..Default::default()
                },
            );
            p[4] = 1;
            p[5] = 0x80;
            let n = payload.len().min(182);
            p[6..6 + n].copy_from_slice(&payload[..n]);
        } else {
            write_header(
                &mut p,
                &Header {
                    pid,
                    cc,
                    pusi,
                    afc: 1,
                    ..Default::default()
                },
            );
            let n = payload.len().min(184);
            p[4..4 + n].copy_from_slice(&payload[..n]);
        }
        p
    }

    #[test]
    fn duplicates_discontinuity_and_loss() {
        let mut d = TsDemux::new();
        let mut c = Collect::default();
        let pat = build_pat(1, 0, &[(1, 0x100)]);
        let mut cc = 0u8;
        let pk = packetize_section(&pat, 0, &mut cc)[0];
        d.push(&pk, Some(0.0), false, &mut c);
        d.push(&pk, Some(0.0), false, &mut c); // a duplicate: discarded, not an error
        assert_eq!(d.stats.cc_errors, 0);
        assert_eq!(d.stats.duplicates, 1);
        assert_eq!(c.sections.len(), 1);
        d.push(&pkt(0, 9, true, true, &[0, 0xFF]), Some(0.0), false, &mut c); // discontinuity_indicator
        assert_eq!(d.stats.cc_errors, 0);
        // A section across packets, then a loss: the partial section is discarded, no error.
        let long = build_pmt(
            1,
            0,
            0x101,
            &[0u8; 0],
            &(0..40)
                .map(|i| (0x1B, 0x200 + i, descriptor(0x52, &[i as u8])))
                .collect::<Vec<_>>(),
        );
        d.add_section_pid(0x100);
        let mut cc = 0;
        let pks = packetize_section(&long, 0x100, &mut cc);
        assert!(pks.len() >= 2);
        d.push(&pks[0], Some(0.0), false, &mut c);
        d.push(&pks[1], Some(0.0), true, &mut c);
        assert_eq!(d.stats.cc_errors, 0);
        assert_eq!(c.sections.iter().filter(|s| s.0 == 0x100).count(), 0);
    }

    /// A packet identifier whose continuity counter stands still: the first repeat is a duplicate, each later one an
    /// error, for any number of them.
    #[test]
    fn a_counter_that_stands_still_is_counted_for_any_number_of_packets() {
        let mut d = TsDemux::new();
        let mut c = Collect::default();
        let pat = build_pat(1, 0, &[(1, 0x100)]);
        let mut cc = 0u8;
        let pk = packetize_section(&pat, 0, &mut cc)[0];
        for _ in 0..1000 {
            d.push(&pk, Some(0.0), false, &mut c);
        }
        assert_eq!(d.stats.duplicates, 1);
        assert_eq!(d.stats.cc_errors, 998);
    }

    #[test]
    fn cc_gap_inside_a_section() {
        let mut d = TsDemux::new();
        let mut c = Collect::default();
        d.add_section_pid(0x100);
        let long = build_pmt(
            1,
            0,
            0x101,
            &[],
            &(0..60)
                .map(|i| (0x1B, 0x200 + i, descriptor(0x52, &[i as u8])))
                .collect::<Vec<_>>(),
        );
        let mut cc = 0;
        let pks = packetize_section(&long, 0x100, &mut cc);
        assert!(pks.len() >= 3);
        d.push(&pks[0], Some(0.0), false, &mut c);
        d.push(&pks[2], Some(0.0), false, &mut c);
        for p in &pks[3..] {
            d.push(p, Some(0.0), false, &mut c);
        }
        assert_eq!(d.stats.cc_errors, 1);
        assert!(c.sections.iter().all(|s| s.0 != 0x100));
    }

    #[test]
    fn pointer_past_the_end() {
        let mut d = TsDemux::new();
        let mut c = Collect::default();
        let mut p = pkt(0, 0, true, false, &[200, 0, 0]);
        p[4] = 200;
        d.push(&p, Some(0.0), false, &mut c);
        assert!(c.sections.is_empty());
    }

    #[test]
    fn scrambling_counts() {
        let mut d = TsDemux::new();
        let mut c = Collect::default();
        for i in 0..10u8 {
            let mut p = pkt(0x300, i, false, false, &[1; 10]);
            if i == 5 {
                p[3] |= 0x80; // one stray scrambled packet
            }
            d.push(&p, Some(0.0), false, &mut c);
        }
        assert!(!d.report(0x300).scrambled);
        for i in 0..10u8 {
            let mut p = pkt(0x301, i, false, false, &[1; 10]);
            p[3] |= 0x80;
            d.push(&p, Some(0.0), false, &mut c);
        }
        // A PCR-only packet (adaptation only, scrambling bits 00) does not clear it.
        let mut p = [0xFFu8; 188];
        write_header(
            &mut p,
            &Header {
                pid: 0x301,
                cc: 9,
                afc: 2,
                ..Default::default()
            },
        );
        p[4] = 183;
        p[5] = 0x10;
        d.push(&p, Some(0.0), false, &mut c);
        assert!(d.report(0x301).scrambled);
    }

    #[test]
    fn ts_filter_rewrites_the_pat() {
        let mut f = TsFilter::new(false);
        f.set_service(5, 0x100, 0x101, &[0x101, 0x102], 9);
        let mut out = Vec::new();
        let mut cc = 0;
        let pat = packetize_section(&build_pat(9, 0, &[(5, 0x100), (6, 0x200)]), 0, &mut cc)[0];
        f.push(&pat, &mut out);
        f.push(&pkt(0x102, 0, false, false, &[1]), &mut out);
        f.push(&pkt(0x200, 0, false, false, &[1]), &mut out);
        f.push(&pkt(0x12, 0, false, false, &[1]), &mut out);
        assert_eq!(out.len(), 2);
        let mut c = Collect::default();
        let mut d = TsDemux::new();
        d.push(&out[0], Some(0.0), false, &mut c);
        let (pid, s) = &c.sections[0];
        assert_eq!(*pid, 0);
        let h = tables::check(s).unwrap();
        assert_eq!(tables::pat(s, &h).programs, vec![(5, 0x100)]);
    }
}

//! Transport stream recording: the whole multiplex, or one file per service with a rewritten single-programme PAT. A file that
//! reaches the size limit goes on in a next part.
//!
//! Standard: ISO/IEC 13818-1, 2.4.4 (program association and program map sections).

use crate::error::RecordError;
use crate::format::{iso_utc, stamp};
use crate::spool::{self, Block, Encoding, Live, Op, Sink, Spool, Writer};
use sdrkit_core::{Clock, Packet, Storage, StorePath};
use sdrkit_proto_mpegts::demux::SectionAssembler;
use sdrkit_proto_mpegts::packet::{self, NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_proto_mpegts::{tables, TsFilter};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Packets in one block of the writer.
const BLOCK_PACKETS: usize = 512;

/// A block of a transport stream file: shared packets (the multiplex) or filtered copies (a service).
pub(crate) struct TsBlock {
    packets: Vec<TsPacket>,
    filtered: Vec<[u8; TS_SIZE]>,
}

impl TsBlock {
    fn new() -> TsBlock {
        TsBlock {
            packets: Vec::with_capacity(BLOCK_PACKETS),
            filtered: Vec::with_capacity(BLOCK_PACKETS),
        }
    }
    fn len(&self) -> usize {
        self.packets.len() + self.filtered.len()
    }
}

impl Block for TsBlock {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn write_to(&self, sink: &mut Sink) -> std::io::Result<()> {
        self.packets
            .iter()
            .try_for_each(|p| sink.write(p.bytes()))?;
        self.filtered.iter().try_for_each(|p| sink.write(p))
    }
    fn clear(&mut self) {
        self.packets.clear();
        self.filtered.clear();
    }
}

/// How the stream is recorded.
#[derive(Clone, Debug)]
pub(crate) struct TsSpec {
    pub folder: StorePath,
    pub prefix: String,
    /// The services to record, one file each. Empty records the multiplex.
    pub services: Vec<u16>,
    pub drop_null: bool,
    pub rf_hz: f64,
    pub max_bytes: u64,
    pub sidecar: bool,
    pub utc_offset_min: i32,
    pub queue_bytes: u64,
}

/// The sidecar of a transport stream file: what it holds, as JSON.
#[derive(Serialize)]
struct Sidecar<'a> {
    recorder: &'static str,
    start_utc: &'a str,
    part: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_hz: Option<f64>,
}

/// The control side of a transport stream recording.
pub(crate) struct TsControl {
    pub writer: Writer<TsBlock>,
    pub live: Arc<Live>,
}

struct OutFile {
    service: Option<u16>,
    filter: Option<TsFilter>,
    base: String,
    part: u32,
    part_bytes: u64,
}

/// The transport packets of `media`, a packet of [`Codec::MpegTs`](sdrkit_core::media::Codec::MpegTs), as shared slices of
/// its bytes. Bytes after the last whole transport packet, and a transport packet that does not start with the sync byte, are
/// skipped.
pub fn transport_packets(media: &Packet) -> impl Iterator<Item = TsPacket> + '_ {
    (0..media.len / TS_SIZE).filter_map(move |k| {
        TsPacket::new(Packet::slice(
            &media.data,
            media.start + k * TS_SIZE,
            TS_SIZE,
            media.time,
        ))
    })
}

/// The feed of a transport stream recording. The engine passes it the packets of the stream. [`TsFeed::push`] never waits on
/// storage and allocates nothing once the tables of the chosen services were seen.
pub struct TsFeed {
    spec: TsSpec,
    clock: Clock,
    spool: Spool<TsBlock>,
    live: Arc<Live>,
    session: String,
    files: Vec<OutFile>,
    pat: SectionAssembler,
    pmt: HashMap<u16, SectionAssembler>,
    pmt_pids: HashMap<u16, u16>,
    last_sections: HashMap<u16, Vec<u8>>,
    sections: Vec<u8>,
    ends: Vec<usize>,
    tsid: u16,
    ended: bool,
}

impl std::fmt::Debug for TsFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TsFeed")
            .field("session", &self.session)
            .field("files", &self.files.len())
            .finish()
    }
}

/// Starts a transport stream recording of `session` on `storage`. Each file opens at once.
pub(crate) fn start(
    spec: TsSpec,
    session: &str,
    storage: Arc<dyn Storage>,
    clock: Clock,
) -> Result<(TsFeed, TsControl), RecordError> {
    let slots = spec.services.len().max(1);
    let buffers = (spec.queue_bytes / (BLOCK_PACKETS * TS_SIZE) as u64).max(2) as usize;
    let (spool, writer) = spool::start(storage, slots, buffers, TsBlock::new)?;
    let live = Arc::new(Live::default());
    let now = clock.now();
    let time = stamp(now, spec.utc_offset_min);
    let mhz = if spec.rf_hz > 0.0 {
        format!("_{:.3}MHz", spec.rf_hz / 1e6)
    } else {
        String::new()
    };
    let services: Vec<Option<u16>> = if spec.services.is_empty() {
        vec![None]
    } else {
        spec.services.iter().map(|&s| Some(s)).collect()
    };
    let files = services
        .into_iter()
        .map(|service| {
            let what = service.map_or_else(|| "mux".to_string(), |sid| format!("sid{sid}"));
            OutFile {
                service,
                filter: service.map(|_| TsFilter::new(false)),
                base: format!("{}_{what}{mhz}_{time}", spec.prefix),
                part: 0,
                part_bytes: 0,
            }
        })
        .collect();
    let mut feed = TsFeed {
        spec,
        clock,
        spool,
        live: live.clone(),
        session: session.into(),
        files,
        pat: SectionAssembler::default(),
        pmt: HashMap::new(),
        pmt_pids: HashMap::new(),
        last_sections: HashMap::new(),
        sections: Vec::new(),
        ends: Vec::new(),
        tsid: 0,
        ended: false,
    };
    for i in 0..feed.files.len() {
        feed.open_part(i);
    }
    Ok((feed, TsControl { writer, live }))
}

impl TsFeed {
    /// The session whose stream this records.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// True once the recording has ended. The engine drops an ended feed.
    pub fn is_ended(&self) -> bool {
        self.ended
    }

    /// Adds one packet. A packet of the multiplex is kept by its shared bytes. A packet of a service is copied once by the
    /// filter that rewrites the PAT.
    pub fn push(&mut self, p: &TsPacket) {
        if self.ended {
            return;
        }
        if self.live.stopping() {
            self.end();
            return;
        }
        let bytes = p.bytes();
        if self.spec.services.is_empty() {
            let pid = u16::from_be_bytes([bytes[1], bytes[2]]) & 0x1FFF;
            if !(self.spec.drop_null && pid == NULL_PID) {
                match self.spool.block(0) {
                    Some(b) => {
                        b.packets.push(p.clone());
                        self.after_append(0, 1);
                    }
                    None => self.live.add_dropped(1),
                }
            }
            return;
        }
        self.psi(bytes);
        for i in 0..self.files.len() {
            let Some(b) = self.spool.block(i) else {
                self.live.add_dropped(1);
                continue;
            };
            let before = b.filtered.len();
            if let Some(f) = &mut self.files[i].filter {
                f.push(bytes, &mut b.filtered);
            }
            let n = b.filtered.len() - before;
            if n > 0 {
                self.after_append(i, n);
            }
        }
    }

    fn after_append(&mut self, i: usize, n: usize) {
        self.live.add_frames(n as u64);
        let bytes = (n * TS_SIZE) as u64;
        self.files[i].part_bytes += bytes;
        // A block is sent before it can grow past its capacity: the filter appends at most a few packets per call.
        if self
            .spool
            .block(i)
            .is_some_and(|b| b.len() + 8 > BLOCK_PACKETS)
        {
            self.spool.send(i);
        }
        if self.spec.max_bytes > 0
            && self.files[i].part_bytes + TS_SIZE as u64 > self.spec.max_bytes
        {
            self.spool.send(i);
            self.spool.op(Op::Close {
                slot: i,
                keep: true,
                log: None,
            });
            self.open_part(i);
        }
    }

    /// Follows the PAT and the PMTs of the chosen services, so the filters follow a service whose streams change.
    fn psi(&mut self, p: &[u8]) {
        let Some(parsed) = packet::parse(p) else {
            return;
        };
        let pid = parsed.header.pid;
        let pusi = parsed.header.pusi;
        self.sections.clear();
        self.ends.clear();
        let (sections, ends) = (&mut self.sections, &mut self.ends);
        let mut keep = |s: &[u8]| {
            sections.extend_from_slice(s);
            ends.push(sections.len());
        };
        if pid == 0 {
            self.pat.push(parsed.payload, pusi, &mut keep);
        } else if let Some(a) = self.pmt.get_mut(&pid) {
            a.push(parsed.payload, pusi, &mut keep);
        } else {
            return;
        }
        let mut from = 0;
        for k in 0..self.ends.len() {
            let to = self.ends[k];
            let s = std::mem::take(&mut self.sections);
            self.section(pid, &s[from..to]);
            self.sections = s;
            from = to;
        }
    }

    fn section(&mut self, pid: u16, s: &[u8]) {
        if self
            .last_sections
            .get(&pid)
            .is_some_and(|l| l.as_slice() == s)
        {
            return;
        }
        let Ok(h) = tables::check(s) else {
            return;
        };
        let last = self.last_sections.entry(pid).or_default();
        last.clear();
        last.extend_from_slice(s);
        match h.table_id {
            0x00 => {
                let pat = tables::pat(s, &h);
                self.tsid = pat.tsid;
                for (prog, pmt_pid) in pat.programs {
                    if prog != 0 && self.spec.services.contains(&prog) {
                        self.pmt_pids.insert(prog, pmt_pid);
                        self.pmt.entry(pmt_pid).or_default();
                    }
                }
            }
            0x02 => {
                let Some(pmt) = tables::pmt(s, &h) else {
                    return;
                };
                let Some(&pmt_pid) = self.pmt_pids.get(&pmt.program) else {
                    return;
                };
                let es: Vec<u16> = pmt.streams.iter().map(|s| s.pid).collect();
                for f in &mut self.files {
                    if f.service == Some(pmt.program) {
                        if let Some(flt) = &mut f.filter {
                            flt.set_service(pmt.program, pmt_pid, pmt.pcr_pid, &es, self.tsid);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn open_part(&mut self, i: usize) {
        let now = self.clock.now();
        let f = &mut self.files[i];
        f.part += 1;
        f.part_bytes = 0;
        let name = if f.part == 1 {
            format!("{}.ts", f.base)
        } else {
            format!("{}_part{}.ts", f.base, f.part)
        };
        let meta = self.spec.sidecar.then(|| {
            let start = iso_utc(now);
            serde_json::to_vec_pretty(&Sidecar {
                recorder: "sdrkit",
                start_utc: &start,
                part: f.part,
                service_id: f.service,
                frequency_hz: (self.spec.rf_hz > 0.0).then_some(self.spec.rf_hz),
            })
            .unwrap_or_default()
        });
        match self.spec.folder.join(&name) {
            Ok(path) => self.spool.op(Op::Open {
                slot: i,
                path,
                encoding: Encoding::Plain,
                meta: meta.map(|m| ("json", m)),
            }),
            Err(_) => self.end(),
        }
    }

    fn end(&mut self) {
        if self.ended {
            return;
        }
        self.spool.send_all();
        self.spool.op(Op::Finish);
        self.ended = true;
        self.live.ended.store(true, Ordering::Relaxed);
    }
}

impl Drop for TsFeed {
    fn drop(&mut self) {
        self.end();
    }
}

//! Test material: a record service on storage in memory, command calls, and a transport stream with two services, PCR and
//! null packets.
#![allow(dead_code)]

use sdrkit_core::command::CommandError;
use sdrkit_core::event::Event;
use sdrkit_core::media::Packet;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Clock, MemStorage, Record, Storage, StorePath, Value};
use sdrkit_proto_mpegts::packet::{self, Header, TS_SIZE};
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_proto_mpegts::tables;
use sdrkit_svc_record::{Feed, RecordService};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A record service on storage in memory, with a clock that the test moves.
pub struct Rig {
    pub svc: RecordService,
    pub storage: Arc<MemStorage>,
    pub time: Arc<AtomicU64>,
    pub events: Vec<Event>,
}

/// A clock that reads `t` in milliseconds.
pub fn clock(t: &Arc<AtomicU64>) -> Clock {
    let t = t.clone();
    Clock::new(move || t.load(Ordering::Relaxed) as f64 / 1000.0)
}

impl Rig {
    /// A service at 1 735 732 800 s (2025-01-01 12:00 UTC) with the settings `set`.
    pub fn new(set: &[(&str, Value)]) -> Rig {
        let storage = Arc::new(MemStorage::new());
        let time = Arc::new(AtomicU64::new(1_735_732_800_000));
        let mut svc = RecordService::open(storage.clone(), clock(&time)).unwrap();
        for (k, v) in set {
            let id = svc.schema().id(k).unwrap();
            owner::apply(&mut svc, id, v).unwrap();
        }
        Rig {
            svc,
            storage,
            time,
            events: Vec::new(),
        }
    }

    /// Runs command `key` with the arguments `args`.
    pub fn call(
        &mut self,
        key: &str,
        args: &[(&str, Value)],
    ) -> Result<Option<Record>, CommandError> {
        call(&mut self.svc, key, args)
    }

    /// Runs a start command and returns the row key of the recording and its feed.
    pub fn start(&mut self, key: &str, args: &[(&str, Value)]) -> (String, Feed) {
        let reply = self.call(key, args).unwrap().unwrap();
        let id = reply.decl().id("recording").unwrap();
        let Some(Value::Text(row)) = reply.value(id).cloned() else {
            panic!("no recording in the reply");
        };
        let feed = self.svc.take_feeds().pop().unwrap();
        (row, feed)
    }

    /// Ticks the service until `done` is true, for at most 10 s of real time.
    pub fn tick_until(&mut self, mut done: impl FnMut(&Rig) -> bool) {
        for _ in 0..2000 {
            let now = self.time.load(Ordering::Relaxed) as f64 / 1000.0;
            self.svc.tick(now, &mut self.events);
            if done(self) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("timed out");
    }

    /// Ticks until no recording runs.
    pub fn settle(&mut self) {
        self.tick_until(|r| rows(&r.svc, "running").is_empty());
    }

    /// Moves the clock by `s` seconds.
    pub fn advance(&self, s: f64) {
        self.time.fetch_add((s * 1000.0) as u64, Ordering::Relaxed);
    }

    /// The bytes of file `path`.
    pub fn read(&self, path: &str) -> Vec<u8> {
        self.storage.read(&StorePath::new(path).unwrap()).unwrap()
    }

    /// The files in the recordings folder with extension `ext`, in name order.
    pub fn files(&self, ext: &str) -> Vec<String> {
        let mut v: Vec<String> = self
            .storage
            .list(&StorePath::new("recordings").unwrap())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.name.ends_with(&format!(".{ext}")))
            .map(|e| format!("recordings/{}", e.name))
            .collect();
        v.sort();
        v
    }

    /// The value of result field `key`.
    pub fn field(&self, key: &str) -> Value {
        let r = self.svc.results();
        let id = r.decl().field_id(key).unwrap();
        r.fields().value(id).cloned().unwrap_or(Value::None)
    }

    /// The events of kind `kind`.
    pub fn events_of(&self, kind: &str) -> Vec<&Event> {
        self.events.iter().filter(|e| e.kind() == kind).collect()
    }
}

/// Runs command `key` of `s` with the arguments `args` through the checked path.
pub fn call<O: Owner>(
    s: &mut O,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = s.commands().id(key)?;
    let mut given = s.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let fid = given.decl().id(k).unwrap();
        given.set_value(fid, v).unwrap();
    }
    owner::invoke(s, id, &given)
}

/// The rows of table `key`, as (row key, record), in key order.
pub fn rows<O: Owner>(s: &O, key: &str) -> Vec<(String, Record)> {
    let id = s.results_decl().table_id(key).unwrap();
    let mut v: Vec<(String, Record)> = s
        .results()
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(k, r)| (k.to_string(), r.clone()))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// The value of field `key` of `r`.
pub fn cell(r: &Record, key: &str) -> Value {
    r.value(r.decl().id(key).unwrap())
        .cloned()
        .unwrap_or(Value::None)
}

/// A text value.
pub fn text(v: &str) -> Value {
    Value::Text(v.into())
}

/// The packets of `bytes`, each on its own shared bytes.
pub fn packets(bytes: &[u8]) -> Vec<TsPacket> {
    let data: Arc<[u8]> = bytes.into();
    (0..bytes.len() / TS_SIZE)
        .map(|k| TsPacket::new(Packet::slice(&data, k * TS_SIZE, TS_SIZE, 0.0)).unwrap())
        .collect()
}

/// A PCR (27 MHz) of `t` seconds, plus an offset in 90 kHz ticks.
pub fn pcr_of(t: f64, base0: u64) -> u64 {
    let ticks = (t * 27e6).round() as u64;
    ((base0 + ticks / 300) % (1 << 33)) * 300 + ticks % 300
}

/// A packet of `pid` carrying a PCR (adaptation field only).
pub fn pcr_packet(pid: u16, pcr: u64, cc: u8) -> [u8; TS_SIZE] {
    let mut p = [0xFFu8; TS_SIZE];
    packet::write_header(
        &mut p,
        &Header {
            pid,
            afc: 2,
            cc,
            ..Default::default()
        },
    );
    let base = pcr / 300;
    let ext = pcr % 300;
    p[4] = 183;
    p[5] = 0x10;
    p[6] = (base >> 25) as u8;
    p[7] = (base >> 17) as u8;
    p[8] = (base >> 9) as u8;
    p[9] = (base >> 1) as u8;
    p[10] = ((base & 1) as u8) << 7 | 0x7E | (ext >> 8) as u8;
    p[11] = ext as u8;
    p
}

/// A payload packet of `pid` whose payload starts with a 4-byte serial number.
pub fn payload_packet(pid: u16, cc: u8, serial: u32) -> [u8; TS_SIZE] {
    let mut p = [0u8; TS_SIZE];
    packet::write_header(
        &mut p,
        &Header {
            pid,
            afc: 1,
            cc,
            ..Default::default()
        },
    );
    p[4..8].copy_from_slice(&serial.to_be_bytes());
    p
}

/// The PIDs of the generated multiplex.
pub const PMT1: u16 = 0x100;
pub const VID1: u16 = 0x101;
pub const AUD1: u16 = 0x102;
pub const PMT2: u16 = 0x200;
pub const VID2: u16 = 0x201;

/// A multiplex of `seconds`: two services (1: PMT 0x100, video 0x101 with the PCR, audio 0x102. 2: PMT 0x200, video 0x201 with
/// its PCR), PAT and PMTs every 0.4 s, PCR every 40 ms, and null packets. `per` payload packets of each video PID per 40 ms.
/// `base0` offsets the PCR (in 90 kHz ticks), to test the wrap.
pub fn multiplex(seconds: f64, per: usize, base0: u64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cc: std::collections::HashMap<u16, u8> = Default::default();
    let mut next = |pid: u16| {
        let c = cc.entry(pid).or_insert(0);
        let v = *c;
        *c = (*c + 1) & 15;
        v
    };
    let pat = tables::build_pat(7, 0, &[(1, PMT1), (2, PMT2)]);
    let pmt1 = tables::build_pmt(
        1,
        0,
        VID1,
        &[],
        &[(0x02, VID1, vec![]), (0x04, AUD1, vec![])],
    );
    let pmt2 = tables::build_pmt(2, 0, VID2, &[], &[(0x02, VID2, vec![])]);
    let (mut c0, mut c1, mut c2) = (0u8, 0u8, 0u8);
    let n = (seconds * 25.0).round() as usize;
    let mut serial = 0u32;
    for i in 0..n {
        let t = i as f64 * 0.04;
        if i % 10 == 0 {
            for p in packet::packetize_section(&pat, 0, &mut c0) {
                out.extend_from_slice(&p);
            }
            for p in packet::packetize_section(&pmt1, PMT1, &mut c1) {
                out.extend_from_slice(&p);
            }
            for p in packet::packetize_section(&pmt2, PMT2, &mut c2) {
                out.extend_from_slice(&p);
            }
        }
        out.extend_from_slice(&pcr_packet(VID1, pcr_of(t, base0), next(VID1)));
        out.extend_from_slice(&pcr_packet(VID2, pcr_of(t, base0 + 1000), next(VID2)));
        for _ in 0..per {
            serial += 1;
            out.extend_from_slice(&payload_packet(VID1, next(VID1), serial));
            out.extend_from_slice(&payload_packet(VID2, next(VID2), serial));
        }
        out.extend_from_slice(&payload_packet(AUD1, next(AUD1), serial));
        let mut null = [0xFFu8; TS_SIZE];
        packet::write_header(
            &mut null,
            &Header {
                pid: packet::NULL_PID,
                afc: 1,
                ..Default::default()
            },
        );
        out.extend_from_slice(&null);
        out.extend_from_slice(&null);
    }
    out
}

/// The PIDs of the packets in a file, in order.
pub fn pids(bytes: &[u8]) -> Vec<u16> {
    bytes
        .as_chunks::<TS_SIZE>()
        .0
        .iter()
        .map(|p| {
            assert_eq!(p[0], 0x47);
            u16::from_be_bytes([p[1], p[2]]) & 0x1FFF
        })
        .collect()
}

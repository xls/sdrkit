//! The MPEG-4 elementary streams of one programme in a transport stream, as DMB sends them: the PAT, the PMT with its initial
//! object descriptor, and SL packets in PES, given out as access units.
//!
//! Standard: ISO/IEC 13818-1 2.11 (carriage of ISO/IEC 14496 data), ISO/IEC 14496-1 7.2.6 and 7.3.2.

use super::{DemuxSink, Route, TsDemux};
use crate::mpeg4::{EsDescriptor, IodDescriptor, SlAssembler};
use crate::pes::PesPacket;
use crate::tables;
use std::collections::BTreeMap;

/// stream_type of SL packets in PES (ISO/IEC 13818-1 table 2-34).
pub const SL_IN_PES: u8 = 0x12;
/// The largest access unit kept, bytes.
const MAX_AU: usize = 1 << 20;

/// One elementary stream that the initial object descriptor and the PMT describe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlStream {
    /// The PID that carries it.
    pub pid: u16,
    /// Its ES_Descriptor: ES_ID, decoder configuration and SL configuration.
    pub es: EsDescriptor,
}

/// One access unit out of an SL stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessUnit {
    /// ES_ID of the stream.
    pub es_id: u16,
    /// objectTypeIndication of the stream (0x21 H.264, 0x40 MPEG-4 audio).
    pub object_type: u8,
    /// streamType of the stream (4 visual, 5 audio).
    pub stream_type: u8,
    /// The bytes.
    pub data: Vec<u8>,
    /// The composition time stamp in 90 kHz units, when the SL header provides one.
    pub cts_90k: Option<i64>,
    /// The unit is a random access point.
    pub rap: bool,
}

impl sdrkit_graph::Item for AccessUnit {
    const KIND: &'static str = "mpeg4.access_unit";
}

#[derive(Default)]
struct Collect {
    sections: Vec<(u16, Vec<u8>)>,
    pes: Vec<PesPacket>,
}

impl DemuxSink for Collect {
    fn section(&mut self, pid: u16, data: &[u8], _time: f64) {
        self.sections.push((pid, data.to_vec()));
    }
    fn pes(&mut self, _route: Route, pes: PesPacket) {
        self.pes.push(pes);
    }
}

/// The demultiplexer of the SL streams of the first programme in the PAT.
pub struct SlDemux {
    ts: TsDemux,
    pmt_pid: Option<u16>,
    pmt_version: Option<u8>,
    streams: BTreeMap<u16, (SlStream, SlAssembler)>,
    sink: Collect,
}

impl Default for SlDemux {
    fn default() -> Self {
        Self::new()
    }
}

impl SlDemux {
    /// A demultiplexer with no programme.
    pub fn new() -> Self {
        SlDemux {
            ts: TsDemux::new(),
            pmt_pid: None,
            pmt_version: None,
            streams: BTreeMap::new(),
            sink: Collect::default(),
        }
    }

    /// The streams known, by PID.
    pub fn streams(&self) -> impl Iterator<Item = &SlStream> {
        self.streams.values().map(|(s, _)| s)
    }

    /// The packet layer and its statistics.
    pub fn packet_layer(&self) -> &TsDemux {
        &self.ts
    }

    /// One 188-byte packet in. Each access unit it completes goes to `out`. A packet with the transport_error_indicator is
    /// counted and dropped by the packet layer.
    pub fn push(&mut self, packet: &[u8], out: &mut Vec<AccessUnit>) {
        self.ts.push(packet, None, false, &mut self.sink);
        let sections = std::mem::take(&mut self.sink.sections);
        for (pid, s) in &sections {
            self.section(*pid, s);
        }
        self.sink.sections = sections;
        self.sink.sections.clear();
        let pes = std::mem::take(&mut self.sink.pes);
        for p in &pes {
            self.pes_packet(p, out);
        }
        self.sink.pes = pes;
        self.sink.pes.clear();
    }

    fn section(&mut self, pid: u16, s: &[u8]) {
        let Ok(h) = tables::check(s) else { return };
        match h.table_id {
            0x00 if pid == 0 => {
                let pat = tables::pat(s, &h);
                let Some(&(_, pmt)) = pat.programs.iter().find(|p| p.0 != 0) else {
                    return;
                };
                if self.pmt_pid != Some(pmt) {
                    if let Some(old) = self.pmt_pid {
                        self.ts.remove_section_pid(old);
                    }
                    self.ts.add_section_pid(pmt);
                    self.pmt_pid = Some(pmt);
                    self.pmt_version = None;
                }
            }
            0x02 if Some(pid) == self.pmt_pid && self.pmt_version != Some(h.version) => {
                let Some(pmt) = tables::pmt(s, &h) else {
                    return;
                };
                self.pmt_version = Some(h.version);
                let es: Vec<EsDescriptor> = pmt
                    .program_desc
                    .iod
                    .as_deref()
                    .and_then(|d| IodDescriptor::parse(d).ok())
                    .map(|d| d.iod.es)
                    .unwrap_or_default();
                self.streams.clear();
                let mut routes = BTreeMap::new();
                for st in pmt.streams.iter().filter(|st| st.stream_type == SL_IN_PES) {
                    let Some(id) = st.desc.sl_es_id.or(st.desc.fmc.first().map(|f| f.0)) else {
                        continue;
                    };
                    let Some(e) = es.iter().find(|e| e.es_id == id) else {
                        continue;
                    };
                    let route = if e.decoder.stream_type == 4 {
                        Route::Video
                    } else {
                        Route::Audio
                    };
                    routes.insert(st.pid, route);
                    let stream = SlStream {
                        pid: st.pid,
                        es: e.clone(),
                    };
                    self.streams
                        .insert(st.pid, (stream, SlAssembler::new(MAX_AU)));
                }
                self.ts.set_routes(&routes);
            }
            _ => {}
        }
    }

    fn pes_packet(&mut self, p: &PesPacket, out: &mut Vec<AccessUnit>) {
        let Some((s, asm)) = self.streams.get_mut(&p.pid) else {
            return;
        };
        if p.damaged {
            asm.reset();
        }
        let e = &s.es;
        let c = &e.sl;
        let _ = asm.push(p.payload.bytes(), c, |h, data| {
            out.push(AccessUnit {
                es_id: e.es_id,
                object_type: e.decoder.object_type,
                stream_type: e.decoder.stream_type,
                data: data.to_vec(),
                cts_90k: h.cts.and_then(|t| c.to_90khz(t)),
                rap: h.rap,
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpeg4::{DecoderConfig, ObjectDescriptor, Profiles, SlConfig, SlHeader};
    use crate::packet::{packetize_pes, packetize_section};
    use crate::pes::build_pes;

    fn sl_config() -> SlConfig {
        SlConfig {
            use_au_start: true,
            use_au_end: true,
            use_rap: true,
            use_timestamps: true,
            ts_resolution: 90_000,
            ts_len: 33,
            ..SlConfig::default()
        }
    }

    fn es(es_id: u16, object_type: u8, stream_type: u8) -> EsDescriptor {
        EsDescriptor {
            es_id,
            decoder: DecoderConfig {
                object_type,
                stream_type,
                ..DecoderConfig::default()
            },
            sl: sl_config(),
            ..EsDescriptor::default()
        }
    }

    #[test]
    fn gives_access_units_of_the_iod_streams() {
        let iod = IodDescriptor {
            scope: 0x10,
            label: 1,
            iod: ObjectDescriptor {
                id: 1,
                initial: true,
                include_inline_profiles: true,
                profiles: Some(Profiles::default()),
                es: vec![es(1, 0x21, 4), es(2, 0x40, 5)],
                ..ObjectDescriptor::default()
            },
        };
        let mut iod_bytes = Vec::new();
        iod.write(&mut iod_bytes).unwrap();
        let pat = tables::build_pat(1, 0, &[(1, 0x100)]);
        let streams: Vec<(u8, u16, Vec<u8>)> = [(0x101u16, 1u16), (0x102, 2)]
            .iter()
            .map(|&(pid, id)| (SL_IN_PES, pid, tables::descriptor(0x1E, &id.to_be_bytes())))
            .collect();
        let pmt = tables::build_pmt(1, 0, 0x101, &iod_bytes, &streams);
        let mut cc = [0u8; 4];
        let mut ts = packetize_section(&pat, 0, &mut cc[0]);
        ts.extend(packetize_section(&pmt, 0x100, &mut cc[1]));
        for (k, (pid, n)) in [(0x101u16, 400usize), (0x102, 30)].iter().enumerate() {
            let h = SlHeader {
                au_start: true,
                au_end: true,
                rap: true,
                cts: Some(9000 * (k as u64 + 1)),
                ..SlHeader::default()
            };
            let mut sl = Vec::new();
            h.write(&sl_config(), &mut sl).unwrap();
            sl.extend(std::iter::repeat_n(k as u8 + 7, *n));
            let pes = build_pes(0xFA, None, None, &sl, true);
            ts.extend(packetize_pes(&pes, *pid, &mut cc[2 + k]));
        }
        let mut d = SlDemux::new();
        let mut out = Vec::new();
        for p in &ts {
            d.push(p, &mut out);
        }
        assert_eq!(d.streams().count(), 2);
        assert_eq!(out.len(), 2);
        assert_eq!(
            (out[0].es_id, out[0].object_type, out[0].data.len()),
            (1, 0x21, 400)
        );
        assert_eq!(
            (out[1].es_id, out[1].stream_type, out[1].cts_90k),
            (2, 5, Some(18_000))
        );
        assert!(out.iter().all(|a| a.rap));
    }
}

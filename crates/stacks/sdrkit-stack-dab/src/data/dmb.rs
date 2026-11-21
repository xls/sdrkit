//! DMB video services: the outer code of a stream data sub-channel without energy dispersal (TS 102 427), and the MPEG-4
//! SL streams of its transport stream (TS 102 428): H.264 access units and MPEG-4 audio access units. The outer code is
//! [`sdrkit_proto_mpegts::outer`], and the transport stream demultiplexer is [`sdrkit_proto_mpegts::demux::sl`].
//!
//! Standard: ETSI TS 102 427 clause 5, ETSI TS 102 428 clauses 5 and 6.

use sdrkit_proto_mpegts::codec::{h264_nals, parse_h264_sps};
use sdrkit_proto_mpegts::demux::sl::{AccessUnit, SlDemux, SlStream};
use sdrkit_proto_mpegts::mpeg4::{EsDescriptor, IodDescriptor, SlConfig, SlHeader};
use sdrkit_proto_mpegts::outer::{self, Dispersal, OuterDecoder, SyncRule, RS_N};
use std::collections::BTreeMap;

/// The packet sync rule of a DMB sub-channel: 204-byte words, no dispersal groups, 20 words looked at, 9 in 10 with 0x47.
const SYNC_RULE: SyncRule = SyncRule {
    spacing: RS_N,
    min_places: 20,
    min_fraction: 0.9,
    groups: false,
    allow_inverted: false,
};

/// An elementary stream as the initial object descriptor describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EsInfo {
    /// ES_ID.
    pub es_id: u16,
    /// objectTypeIndication (0x21 H.264, 0x40 MPEG-4 audio).
    pub oti: u8,
    /// streamType (4 visual, 5 audio).
    pub stream_type: u8,
    /// DecoderSpecificInfo.
    pub dsi: Vec<u8>,
}

impl From<&SlStream> for EsInfo {
    fn from(s: &SlStream) -> Self {
        EsInfo {
            es_id: s.es.es_id,
            oti: s.es.decoder.object_type,
            stream_type: s.es.decoder.stream_type,
            dsi: s.es.decoder.specific_info.clone(),
        }
    }
}

/// What the DMB decoder publishes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DmbStatus {
    /// Transport stream packets decoded.
    pub ts_packets: u64,
    /// RS words corrected.
    pub rs_corrected: u64,
    /// RS words beyond repair.
    pub rs_failed: u64,
    /// The elementary streams.
    pub streams: Vec<EsInfo>,
    /// Video access units.
    pub video_units: u64,
    /// Audio access units.
    pub audio_units: u64,
    /// Picture size from the sequence parameter set.
    pub video_size: Option<(u32, u32)>,
}

/// The DMB decoder of one sub-channel.
pub struct DmbDecoder {
    outer: OuterDecoder,
    demux: SlDemux,
    aligned: bool,
    raw: Vec<u8>,
    bits: Vec<u8>,
    es: BTreeMap<u16, EsInfo>,
    /// Status.
    pub status: DmbStatus,
}

impl Default for DmbDecoder {
    fn default() -> Self {
        DmbDecoder {
            outer: OuterDecoder::new(Dispersal::None),
            demux: SlDemux::new(),
            aligned: false,
            raw: Vec::new(),
            bits: Vec::new(),
            es: BTreeMap::new(),
            status: DmbStatus::default(),
        }
    }
}

impl DmbDecoder {
    /// A decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// The elementary streams known, by ES_ID.
    pub fn streams(&self) -> &BTreeMap<u16, EsInfo> {
        &self.es
    }

    /// One logical frame in; access units out.
    pub fn push(&mut self, lf: &[u8], out: &mut Vec<AccessUnit>) {
        if !self.aligned {
            // The sync bytes keep their 204-byte spacing through the interleaver (branch 0).
            self.raw.extend_from_slice(lf);
            if self.raw.len() < RS_N * 30 {
                return;
            }
            self.bits.clear();
            for &b in &self.raw {
                self.bits.extend((0..8).rev().map(|i| (b >> i) & 1));
            }
            match outer::sync_search(&self.bits, &SYNC_RULE).filter(|h| h.start % 8 == 0) {
                Some(h) => {
                    self.aligned = true;
                    self.outer.restart();
                    let raw = std::mem::take(&mut self.raw);
                    self.feed(&raw[h.start / 8..], out);
                }
                None => {
                    let keep = self.raw.len() - RS_N * 20;
                    self.raw.drain(..keep);
                }
            }
            return;
        }
        self.feed(lf, out);
    }

    fn feed(&mut self, bytes: &[u8], out: &mut Vec<AccessUnit>) {
        let start = out.len();
        let demux = &mut self.demux;
        self.outer.push(bytes, &mut |w| demux.push(&w.packet, out));
        let s = &self.outer.stats;
        self.status.ts_packets = s.delivered;
        self.status.rs_corrected = s.corrected;
        self.status.rs_failed = s.failed;
        if self.es.len() != self.demux.streams().count() {
            self.es = self
                .demux
                .streams()
                .map(|s| (s.es.es_id, EsInfo::from(s)))
                .collect();
            self.status.streams = self.es.values().cloned().collect();
        }
        for au in &out[start..] {
            match au.stream_type {
                4 => {
                    self.status.video_units += 1;
                    if self.status.video_size.is_none() {
                        self.status.video_size = sps_size(&au.data);
                    }
                }
                5 => self.status.audio_units += 1,
                _ => {}
            }
        }
    }
}

/// The picture size from the H.264 sequence parameter set in an Annex B access unit.
pub fn sps_size(au: &[u8]) -> Option<(u32, u32)> {
    let (_, start, end) = h264_nals(au).into_iter().find(|n| n.0 == 7)?;
    let sps = parse_h264_sps(au.get(start..end)?)?;
    Some((sps.width, sps.height))
}

/// Writing a DMB stream, for the generator.
pub mod write {
    use super::*;

    use crate::bits::BitWriter;
    use sdrkit_proto_mpegts::mpeg4::{DecoderConfig, ObjectDescriptor, Profiles};

    /// The SL configuration the generator uses: start, end and random access flags, 33-bit composition time stamps at 90 kHz.
    pub fn sl_config() -> SlConfig {
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

    /// The ES descriptor of one stream, with the generator's SL configuration.
    pub fn es_descriptor(es_id: u16, oti: u8, stream_type: u8, dsi: &[u8]) -> EsDescriptor {
        EsDescriptor {
            es_id,
            decoder: DecoderConfig {
                object_type: oti,
                stream_type,
                buffer_size_db: 0x1000,
                max_bitrate: 0x0004_0000,
                avg_bitrate: 0x0002_0000,
                specific_info: dsi.to_vec(),
                ..DecoderConfig::default()
            },
            sl: sl_config(),
            ..EsDescriptor::default()
        }
    }

    /// The IOD descriptor (MPEG-2 descriptor tag 0x1D) holding an initial object descriptor with these ES descriptors. Empty
    /// when the descriptor is longer than 255 bytes.
    pub fn iod_descriptor(es: &[EsDescriptor]) -> Vec<u8> {
        let d = IodDescriptor {
            scope: 0x10,
            label: 1,
            iod: ObjectDescriptor {
                id: 1,
                initial: true,
                include_inline_profiles: true,
                profiles: Some(Profiles::default()),
                es: es.to_vec(),
                ..ObjectDescriptor::default()
            },
        };
        let mut v = Vec::new();
        if d.write(&mut v).is_err() {
            v.clear();
        }
        v
    }

    /// A PES packet (stream id 0xFA, ISO/IEC 14496 SL-packetized stream) holding one SL packet of an access unit with its
    /// composition time stamp (33 bits, 90 kHz).
    pub fn sl_pes(au: &[u8], cts: u64, rap: bool) -> Vec<u8> {
        let h = SlHeader {
            au_start: true,
            au_end: true,
            rap,
            cts: Some(cts & ((1 << 33) - 1)),
            ..SlHeader::default()
        };
        let mut sl = Vec::with_capacity(8 + au.len());
        // The generator's configuration contains every field this header uses, so the write does not fail.
        let _ = h.write(&sl_config(), &mut sl);
        sl.extend_from_slice(au);
        sdrkit_proto_mpegts::pes::build_pes(0xFA, None, None, &sl, true)
    }

    /// A minimal H.264 baseline access unit (SPS, PPS and an IDR picture of I_PCM macroblocks) of `w` × `h` (multiples of 16)
    /// with a flat luma value.
    pub fn h264_idr(w: u32, h: u32, luma: u8, idr_id: u32) -> Vec<u8> {
        fn ue(b: &mut BitWriter<'_>, v: u32) {
            let x = v + 1;
            let n = 32 - x.leading_zeros();
            b.put(0, ((n - 1) as usize) as u32);
            b.put(x as u64, (n as usize) as u32);
        }
        fn nal(t: u8, rbsp: Vec<u8>) -> Vec<u8> {
            let mut v = vec![0, 0, 0, 1, 0x60 | t];
            let mut zeros = 0;
            for &x in &rbsp {
                if zeros >= 2 && x <= 3 {
                    v.push(3);
                    zeros = 0;
                }
                v.push(x);
                zeros = if x == 0 { zeros + 1 } else { 0 };
            }
            v
        }
        let mut s_buf = Vec::new();
        let mut s = crate::bits::BitWriter::new(&mut s_buf);
        s.put(66, 8);
        s.put(0xC0, 8);
        s.put(30, 8);
        ue(&mut s, 0);
        ue(&mut s, 0);
        ue(&mut s, 2);
        ue(&mut s, 0);
        s.put(0, 1);
        ue(&mut s, w / 16 - 1);
        ue(&mut s, h / 16 - 1);
        s.put(1, 1);
        s.put(1, 1);
        s.put(0, 1);
        s.put(0, 1);
        s.put(1, 1);
        let mut p_buf = Vec::new();
        let mut p = crate::bits::BitWriter::new(&mut p_buf);
        ue(&mut p, 0);
        ue(&mut p, 0);
        p.put(0, 1);
        p.put(0, 1);
        ue(&mut p, 0);
        ue(&mut p, 0);
        ue(&mut p, 0);
        p.put(0, 1);
        p.put(0, 2);
        p.put(1, 1); // pic_init_qp_minus26 = 0 (se: "1")
        p.put(1, 1);
        p.put(1, 1);
        p.put(1, 1);
        p.put(0, 1);
        p.put(0, 1);
        p.put(1, 1);
        let mut sl_buf = Vec::new();
        let mut sl = crate::bits::BitWriter::new(&mut sl_buf);
        ue(&mut sl, 0);
        ue(&mut sl, 7);
        ue(&mut sl, 0);
        sl.put(0, 4);
        ue(&mut sl, idr_id);
        sl.put(0, 1);
        sl.put(0, 1);
        sl.put(1, 1); // slice_qp_delta 0
        ue(&mut sl, 1);
        for _ in 0..(w / 16) * (h / 16) {
            ue(&mut sl, 25);
            sl.align();
            for _ in 0..256 {
                sl.put(luma.max(16) as u64, 8);
            }
            for _ in 0..128 {
                sl.put(128, 8);
            }
        }
        sl.put(1, 1);
        let mut out = nal(7, {
            let _ = s.finish();
            s_buf
        });
        out.extend(nal(8, {
            let _ = p.finish();
            p_buf
        }));
        out.extend(nal(5, {
            let _ = sl.finish();
            sl_buf
        }));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::write::*;
    use super::*;

    #[test]
    fn sps_returns_the_size() {
        let au = h264_idr(64, 48, 100, 0);
        assert_eq!(sps_size(&au), Some((64, 48)));
    }
}

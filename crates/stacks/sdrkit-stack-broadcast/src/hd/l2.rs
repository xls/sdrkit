//! HD Radio Layer 2 and above: the PCI and its spread over the transfer frame, audio PDUs, the fixed data bearer, AAS and PSD
//! packets, ID3 program service data and the station information service. The SIG and the LOT header are sdrkit's own layouts.
//!
//! Standard: NRSC-5-E documents 1014s, 1017s, 1019s, 1020s, 1028s and 1085s.

use sdrkit_algo_fec::rs::{codes as rs_codes, Rs, RsCode, SymbolOrder};
use sdrkit_util_bits::crc::catalog::{CRC16_IBM_SDLC, CRC8_NRSC_5};
use sdrkit_util_bits::crc::cyclic::remainder;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::hdlc::{ByteStuffing, Unstuffer, FLAG};
use sdrkit_util_bits::pack::{pack, push_u64, BitOrder, Pad};
use sdrkit_util_bits::{BitReader, BitWriter, HardBits};
use std::collections::BTreeMap;

/// The PCI code words CW0 to CW7 of standard L2 PDUs, h0 in the most significant of 24 bits (1014s table 5-6). Each is a
/// cyclic permutation of CW0.
pub mod pci {
    /// CW0: audio only.
    pub const AUDIO: u32 = 0x38D8D3;
    /// CW1: audio and opportunistic data.
    pub const AUDIO_OPPORTUNISTIC: u32 = 0xCE3634;
    /// CW2: audio and fixed data.
    pub const AUDIO_FIXED: u32 = 0xE3634C;
    /// CW3: audio, fixed data and opportunistic data.
    pub const AUDIO_FIXED_OPPORTUNISTIC: u32 = 0x8D8D33;
    /// CW4: fixed data only.
    pub const FIXED: u32 = 0x3634CE;
    /// CW5 to CW7: reserved.
    pub const RESERVED: [u32; 3] = [0x8D338D, 0xD8D338, 0x634CE3];
    /// The words in use, CW0 to CW4.
    pub const ALL: [u32; 5] = [
        AUDIO,
        AUDIO_OPPORTUNISTIC,
        AUDIO_FIXED,
        AUDIO_FIXED_OPPORTUNISTIC,
        FIXED,
    ];
    /// Whether the word says that the PDU carries audio (1014s table 5-6).
    pub fn has_audio(cw: u32) -> bool {
        cw != FIXED
    }
    /// Whether the word says that the PDU carries fixed data (1014s table 5-6).
    pub fn has_fixed(cw: u32) -> bool {
        matches!(cw, AUDIO_FIXED | AUDIO_FIXED_OPPORTUNISTIC | FIXED)
    }
}

/// Matches a received PCI word against the code words in use with at most 4 bit errors (`mask` marks bits that are known, and missing
/// bits count as zero). The error limit is sdrkit's own choice.
pub fn match_pci(word: u32, mask: u32) -> Option<u32> {
    pci::ALL
        .iter()
        .copied()
        .find(|&c| ((c ^ word) & mask).count_ones() <= 4)
}

/// MIME hashes known by name.
pub fn mime_name(h: u32) -> String {
    match h {
        0xBE4B_7536 => "primary image".into(),
        0xD9C7_2536 => "station logo".into(),
        0x4DC6_6C5A => "HDC audio".into(),
        0xBB49_2AAC => "text".into(),
        0x1E65_3E9C => "JPEG".into(),
        0x4F32_8CA0 => "PNG".into(),
        0xFF84_22D7 => "TTN traffic".into(),
        0xEF04_2E96 => "TTN weather".into(),
        x => format!("MIME {x:08X}"),
    }
}

/// Program type names (an NRSC-5-style list).
pub fn program_type_name(t: u8) -> &'static str {
    const N: [&str; 32] = [
        "Undefined",
        "News",
        "Information",
        "Sports",
        "Talk",
        "Rock",
        "Classic Rock",
        "Adult Hits",
        "Soft Rock",
        "Top 40",
        "Country",
        "Oldies",
        "Soft",
        "Nostalgia",
        "Jazz",
        "Classical",
        "Rhythm and Blues",
        "Soft Rhythm and Blues",
        "Foreign Language",
        "Religious Music",
        "Religious Talk",
        "Personality",
        "Public",
        "College",
        "Spanish Talk",
        "Spanish Music",
        "Hip-Hop",
        "Reserved",
        "Reserved",
        "Weather",
        "Emergency Test",
        "Emergency",
    ];
    N.get(t as usize).copied().unwrap_or("Reserved")
}

// ---------------------------------------------------------------------------------------------------------------------------
// The PCI spread over the transfer frame

/// Where the header bits of a standard L2 PDU of `L` bits sit: bit h0 at `start`, each next one `offset` bits after the one before
/// (the bits between, exclusive of the header bits), and `len` header bits (1014s clause 5.2.1, table 5-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderSpread {
    /// Nstart.
    pub start: usize,
    /// Noffset.
    pub offset: usize,
    /// The header length, 22 to 24 bits.
    pub len: usize,
}

impl HeaderSpread {
    /// The transfer frame bit of header bit `i`.
    pub fn position(&self, i: usize) -> usize {
        self.start + i * (self.offset + 1)
    }
}

/// The header spread parameters of 1014s table 5-7 for an L2 PDU of `l` bits. `None` when `l` is too short to hold the header
/// (below 120 bits plus the header).
pub fn header_spread(l: usize) -> Option<HeaderSpread> {
    let len = match l % 8 {
        0 => 24,
        7 => 23,
        _ => 22,
    };
    let s = if l < 72_000 {
        let start = 120;
        let bytes = l.checked_sub(120)?.div_ceil(8);
        let offset = (8 * (bytes / len)).checked_sub(1)?;
        HeaderSpread { start, offset, len }
    } else {
        let start = 8 * (l - 30_000).div_ceil(8);
        let offset = match len {
            24 => 1247,
            23 => 1303,
            _ => 1359,
        };
        HeaderSpread { start, offset, len }
    };
    (s.position(len - 1) < l).then_some(s)
}

/// The whole payload bytes of an L2 PDU of `l` bits: the bits that are not header bits, in bytes. The excess bits that do not
/// make a byte sit at the end of the payload (1014s clause 5.2.1).
pub fn payload_bytes(l: usize) -> usize {
    header_spread(l).map_or(0, |s| (l - s.len) / 8)
}

/// Writes the `l` bits of a standard L2 PDU into `out`: the payload bytes (bit 0 of each byte first, 1014s clause 5.3 and 1017s
/// clause 5.2.1.5), with the PCI word `cw` spread over them (1014s clause 5.2.1). Missing payload bits are zeros. An empty `out`
/// when `l` is too short.
pub fn spread(cw: u32, payload: &[u8], l: usize, out: &mut Vec<u8>) {
    out.clear();
    let Some(s) = header_spread(l) else {
        return;
    };
    out.reserve(l);
    let mut next = 0usize;
    let mut bit = 0usize;
    for pos in 0..l {
        if next < s.len && pos == s.position(next) {
            out.push(((cw >> (23 - next)) & 1) as u8);
            next += 1;
        } else {
            let b = payload.get(bit / 8).map_or(0, |&x| (x >> (bit % 8)) & 1);
            out.push(b);
            bit += 1;
        }
    }
}

/// A standard L2 PDU taken apart: the PCI word with its known bits, and the payload bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Despread {
    /// The received header bits, h0 in bit 23.
    pub word: u32,
    /// The header bits the PDU carries (22 to 24).
    pub mask: u32,
    /// The whole payload bytes.
    pub payload: Vec<u8>,
}

/// Takes the header bits out of the `bits` of a standard L2 PDU (one bit per byte) and packs the rest into bytes, bit 0 first
/// (1014s clause 5.2.1). `None` when the PDU is too short.
pub fn despread(bits: &[u8]) -> Option<Despread> {
    let s = header_spread(bits.len())?;
    let mut word = 0u32;
    let mut mask = 0u32;
    let mut payload_bits = Vec::with_capacity(bits.len());
    let mut next = 0usize;
    for (pos, &b) in bits.iter().enumerate() {
        if next < s.len && pos == s.position(next) {
            word |= ((b & 1) as u32) << (23 - next);
            mask |= 1 << (23 - next);
            next += 1;
        } else {
            payload_bits.push(b);
        }
    }
    payload_bits.truncate(payload_bits.len() / 8 * 8);
    let mut payload = Vec::with_capacity(payload_bits.len() / 8);
    pack(&payload_bits, BitOrder::Lsb, Pad::Drop, &mut payload);
    Some(Despread {
        word,
        mask,
        payload,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------
// Audio transport PDUs

/// One row of 1017s table 5-2: an audio codec mode and stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodecStream {
    /// The audio codec mode.
    pub mode: u8,
    /// The stream id: 0 core, 1 enhanced.
    pub stream: u8,
    /// PDUs per L1 frame.
    pub pdus_per_frame: u8,
    /// The average number of encoded audio packets per PDU (N).
    pub packets: u8,
    /// The PDU sequence numbers run from 0 to this value.
    pub seq_max: u8,
    /// Bits per locator (Lc).
    pub lc: u32,
    /// The maximum bit rate, kbit/s (for mode 0b1101, the standard service modes).
    pub max_kbps: u16,
}

/// The audio codec modes and streams that 1017s table 5-2 defines.
pub const CODEC_STREAMS: [CodecStream; 9] = [
    CodecStream {
        mode: 0b0000,
        stream: 0,
        pdus_per_frame: 1,
        packets: 32,
        seq_max: 1,
        lc: 16,
        max_kbps: 96,
    },
    CodecStream {
        mode: 0b0001,
        stream: 0,
        pdus_per_frame: 8,
        packets: 4,
        seq_max: 7,
        lc: 12,
        max_kbps: 48,
    },
    CodecStream {
        mode: 0b0001,
        stream: 1,
        pdus_per_frame: 1,
        packets: 32,
        seq_max: 1,
        lc: 16,
        max_kbps: 48,
    },
    CodecStream {
        mode: 0b0010,
        stream: 0,
        pdus_per_frame: 8,
        packets: 4,
        seq_max: 7,
        lc: 12,
        max_kbps: 20,
    },
    CodecStream {
        mode: 0b0010,
        stream: 1,
        pdus_per_frame: 1,
        packets: 32,
        seq_max: 1,
        lc: 16,
        max_kbps: 16,
    },
    CodecStream {
        mode: 0b0011,
        stream: 0,
        pdus_per_frame: 8,
        packets: 4,
        seq_max: 7,
        lc: 12,
        max_kbps: 24,
    },
    CodecStream {
        mode: 0b0011,
        stream: 1,
        pdus_per_frame: 1,
        packets: 32,
        seq_max: 1,
        lc: 16,
        max_kbps: 72,
    },
    CodecStream {
        mode: 0b1010,
        stream: 0,
        pdus_per_frame: 1,
        packets: 32,
        seq_max: 1,
        lc: 12,
        max_kbps: 22,
    },
    CodecStream {
        mode: 0b1101,
        stream: 0,
        pdus_per_frame: 8,
        packets: 4,
        seq_max: 7,
        lc: 12,
        max_kbps: 24,
    },
];

/// The row of 1017s table 5-2 for a codec mode and stream. Mode 0b1010 stream 1 has the values of its own row: 8 PDUs a frame,
/// 4 packets, Lc 12. `None` for a reserved mode.
pub fn codec_stream(mode: u8, stream: u8) -> Option<CodecStream> {
    if mode == 0b1010 && stream == 1 {
        return Some(CodecStream {
            mode,
            stream,
            pdus_per_frame: 8,
            packets: 4,
            seq_max: 7,
            lc: 12,
            max_kbps: 24,
        });
    }
    CODEC_STREAMS
        .iter()
        .copied()
        .find(|c| c.mode == mode && c.stream == stream)
}

/// The header code of the audio PDU: RS(96, 88) over GF(256), `p(x) = x^8 + x^4 + x^3 + x^2 + 1`, eight roots from `a^1`, the
/// last parity byte first in the PDU and PDU byte 95 the highest data power (1017s clause 5.2.3.1).
pub const HEADER_CODE: RsCode = RsCode {
    order: SymbolOrder::LowFirst,
    ..rs_codes::HD_RADIO_96_88
};

/// The header code's generator `g(x) = a^36 + a^203 x + a^3 x^2 + a^220 x^3 + a^253 x^4 + a^211 x^5 + a^240 x^6 + a^176 x^7 + x^8`,
/// as powers of `a` from the constant term (1017s clause 5.2.3.1).
pub const HEADER_GENERATOR_POWERS: [u8; 8] = [36, 203, 3, 220, 253, 211, 240, 176];

/// A codec for [`HEADER_CODE`].
pub fn header_rs() -> Rs {
    Rs::new(HEADER_CODE).unwrap_or_else(|_| unreachable!("a valid catalogue code"))
}

/// Bytes of the RS codeword at the start of each audio PDU (1017s figure 5-1).
pub const HEADER_LEN: usize = 96;
/// RS parity bytes at the start of each audio PDU (1017s table 5-1).
pub const PARITY_LEN: usize = 8;
/// Bytes before the locators: the parity and the 6-byte PDU control word (1017s figure 5-1).
pub const FIXED_HEADER_LEN: usize = 14;

/// The header expansion IDs of 1017s table 5-6.
pub mod expansion {
    /// Class indication.
    pub const CLASS: u8 = 0b000;
    /// Program number indication.
    pub const PROGRAM_NUMBER: u8 = 0b001;
    /// Program type indication (two bytes).
    pub const PROGRAM_TYPE: u8 = 0b010;
    /// Program segment ID (four or five bytes).
    pub const SEGMENT_ID: u8 = 0b011;
    /// PDU marker (two or four bytes).
    pub const PDU_MARKER: u8 = 0b100;
}

/// An audio PDU of one program stream (1017s clause 5.2.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pdu {
    /// Audio codec mode (table 5-2).
    pub codec_mode: u8,
    /// Stream id: 0 core, 1 enhanced.
    pub stream: u8,
    /// PDU sequence number (3 bits).
    pub seq: u8,
    /// Blend control (table 5-4).
    pub blend: u8,
    /// TX digital audio gain (stream 0, table 5-5) or the pre-decoded per-stream delay (5 bits).
    pub gain: u8,
    /// Post-decoded common delay, in four-audio-frame periods (6 bits).
    pub common_delay: u8,
    /// Latency, in two-audio-frame periods (3 bits).
    pub latency: u8,
    /// The first packet continues from the last PDU.
    pub pfirst: bool,
    /// The last packet continues into the next PDU.
    pub plast: bool,
    /// The sequence number of the first whole packet (6 bits).
    pub start_seq: u8,
    /// Program number 0..7 (header expansion 0b001, 0 when absent).
    pub program: u8,
    /// Program type (header expansion 0b010).
    pub program_type: u8,
    /// Restricted access (the Access bit of header expansion 0b010).
    pub restricted: bool,
    /// Program service data bytes.
    pub psd: Vec<u8>,
    /// Audio packets (without their CRC).
    pub packets: Vec<Vec<u8>>,
}

/// One received PDU: packets with their CRC result.
#[derive(Clone, Debug, Default)]
pub struct RxPdu {
    /// The header fields (packets left empty).
    pub header: Pdu,
    /// Each packet, `None` when its CRC failed.
    pub packets: Vec<Option<Vec<u8>>>,
}

/// Writes the 6-byte PDU control word of codec modes 0b0000 to 0b1110 (1017s figure 5-2): the fields packed from bit 0 of byte 0
/// on (codec mode in header bits 43:40, stream id 45:44, PDU sequence 32 and 47:46, blend 34:33, gain or delay 39:35, common
/// delay 29:24, latency 16 and 31:30, Pfirst 17, Plast 18, starting sequence 8 and 23:19, NOP 14:9, HEF 15, La 7:0).
pub fn control_word(p: &Pdu, nop: usize, hef: bool, la: u8) -> [u8; 6] {
    let mut v = Vec::with_capacity(6);
    let mut w = BitWriter::new_lsb(&mut v);
    for (x, n) in [
        (p.codec_mode as u64, 4),
        (p.stream as u64, 2),
        (p.seq as u64, 3),
        (p.blend as u64, 2),
        (p.gain as u64, 5),
        (p.common_delay as u64, 6),
        (p.latency as u64, 3),
        (p.pfirst as u64, 1),
        (p.plast as u64, 1),
        (p.start_seq as u64, 6),
        (nop as u64, 6),
        (hef as u64, 1),
        (la as u64, 8),
    ] {
        w.put(x, n);
    }
    w.finish();
    let mut out = [0u8; 6];
    out.copy_from_slice(&v[..6]);
    out
}

/// The header expansion bytes for a program number and a program type (1017s figures 5-8 and 5-10): the HEF in bit 7 of each
/// byte says that another byte follows.
pub fn expansion_bytes(program: u8, program_type: u8, restricted: bool) -> [u8; 3] {
    [
        0x80 | expansion::PROGRAM_NUMBER << 4 | (program & 7) << 1,
        0x80 | expansion::PROGRAM_TYPE << 4 | (restricted as u8) << 3 | program_type >> 7,
        program_type & 0x7F,
    ]
}

impl Pdu {
    /// The bytes of the PDU (1017s figure 5-1): RS parity, control word, locators (Lc bits each from table 5-2, bit 0 first,
    /// figures 5-4 and 5-5), header expansion (program number and type), PSD, then each packet with its CRC-8. `None` when the
    /// codec mode is reserved, the PSD ends after byte 255, the PDU is shorter than the 96-byte codeword, or a locator does not fit
    /// in Lc bits.
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        let lc = codec_stream(self.codec_mode, self.stream)?.lc;
        let nop = self.packets.len();
        if nop == 0 || nop > 63 {
            return None;
        }
        let locs_len = (nop * lc as usize).div_ceil(8);
        let ex = expansion_bytes(self.program, self.program_type, self.restricted);
        let psd_start = FIXED_HEADER_LEN + locs_len + ex.len();
        let la = psd_start + self.psd.len() - 1;
        if la > 255 {
            return None;
        }
        let mut ends = Vec::with_capacity(nop);
        let mut end = la;
        for p in &self.packets {
            end += p.len() + 1;
            ends.push(end);
        }
        if end + 1 < HEADER_LEN || ends.iter().any(|&e| e >> lc != 0) {
            return None;
        }
        let mut b = vec![0u8; PARITY_LEN];
        b.extend_from_slice(&control_word(self, nop, true, la as u8));
        let mut w = BitWriter::new_lsb(&mut b);
        for &e in &ends {
            w.put(e as u64, lc);
        }
        w.finish();
        b.extend_from_slice(&ex);
        b.extend_from_slice(&self.psd);
        for p in &self.packets {
            b.extend_from_slice(p);
            b.push(CRC8_NRSC_5.checksum(p));
        }
        header_rs().encode_word(&mut b[..HEADER_LEN]).ok()?;
        Some(b)
    }
}

/// Reads the header expansion bytes from `b` (1017s clause 5.2.1.6) into `p`. It returns the number of bytes, at most 16.
fn read_expansion(b: &[u8], p: &mut Pdu) -> usize {
    let mut i = 0;
    while i < b.len() && i < 16 {
        let x = b[i];
        let (hef, id) = (x >> 7 != 0, (x >> 4) & 7);
        let extra = match id {
            expansion::PROGRAM_NUMBER => {
                p.program = (x >> 1) & 7;
                2 * (x & 1) as usize
            }
            expansion::PROGRAM_TYPE => {
                p.restricted = x & 0x08 != 0;
                let lo = b.get(i + 1).copied().unwrap_or(0);
                p.program_type = (x & 1) << 7 | (lo & 0x7F);
                i += 2;
                if lo >> 7 == 0 {
                    return i;
                }
                continue;
            }
            expansion::PDU_MARKER => {
                if x & 0x08 != 0 {
                    3
                } else {
                    1
                }
            }
            _ => 0,
        };
        // The HEF of the last byte of a multi-byte expansion says whether more follow.
        let last = i + extra;
        let more = if extra == 0 {
            hef
        } else {
            b.get(last).is_some_and(|&y| y >> 7 != 0)
        };
        i = last + 1;
        if !more {
            break;
        }
    }
    i.min(b.len())
}

/// Reads an audio PDU at the start of `b` (1017s clause 5.2.1): corrects the 96-byte codeword, reads the control word, the
/// locators, the header expansion, the PSD and the packets. Returns it and its length (the last locator plus one), or `None`
/// when the codeword cannot be corrected, the codec mode is reserved or the codec mode is 0b1111.
pub fn parse_pdu(b: &[u8], rs: &mut Rs) -> Option<(RxPdu, usize)> {
    if b.len() < HEADER_LEN {
        return None;
    }
    let mut h = b[..HEADER_LEN].to_vec();
    rs.decode(&mut h, &[]).ok()?;
    let mut r = BitReader::new_lsb(&h[PARITY_LEN..FIXED_HEADER_LEN]);
    let mut p = Pdu {
        codec_mode: r.u(4) as u8,
        stream: r.u(2) as u8,
        seq: r.u(3) as u8,
        blend: r.u(2) as u8,
        gain: r.u(5) as u8,
        common_delay: r.u(6) as u8,
        latency: r.u(3) as u8,
        pfirst: r.u(1) != 0,
        plast: r.u(1) != 0,
        start_seq: r.u(6) as u8,
        ..Default::default()
    };
    let nop = r.u(6) as usize;
    let hef = r.u(1) != 0;
    let la = r.u(8) as usize;
    let lc = codec_stream(p.codec_mode, p.stream)?.lc;
    // The codeword is corrected: the bytes after it are as received.
    let mut whole = h;
    whole.extend_from_slice(&b[HEADER_LEN..]);
    let locs_len = (nop * lc as usize).div_ceil(8);
    let locs = whole.get(FIXED_HEADER_LEN..FIXED_HEADER_LEN + locs_len)?;
    let mut lr = BitReader::new_lsb(locs);
    let ends: Vec<usize> = (0..nop).map(|_| lr.u(lc) as usize).collect();
    let mut at = FIXED_HEADER_LEN + locs_len;
    if hef {
        at += read_expansion(&whole[at..], &mut p);
    }
    if la + 1 > at {
        p.psd = whole.get(at..=la)?.to_vec();
    }
    let mut packets = Vec::with_capacity(nop);
    let mut start = la + 1;
    for &e in &ends {
        if e < start || e >= whole.len() {
            return None;
        }
        let pk = &whole[start..=e];
        packets.push(
            CRC8_NRSC_5
                .verify(pk, Trailer::Big)
                .then(|| pk[..pk.len() - 1].to_vec()),
        );
        start = e + 1;
    }
    Some((RxPdu { header: p, packets }, start))
}

// ---------------------------------------------------------------------------------------------------------------------------
// PSD (ID3)

/// Program service data.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Psd {
    /// Title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Album.
    pub album: String,
    /// Genre.
    pub genre: String,
    /// Comment.
    pub comment: String,
    /// UFID owner and id.
    pub ufid: String,
    /// Commercial frame: price, seller, description, URL, valid until.
    pub commercial: Option<(String, String, String, String, String)>,
    /// XHDR: LOT id and MIME hash of the album art (sdrkit's own frame layout).
    pub xhdr: Option<(u32, u32)>,
}

/// The ID3v2.3.0 tag header: "ID3", version 3.0, no flags (1028s clause 6).
pub const ID3_HEADER: [u8; 6] = *b"ID3\x03\x00\x00";
/// The largest ID3 tag of PSD, in bytes (1028s clause 6).
pub const ID3_MAX: usize = 1018;

fn id3_frame(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(body);
}

/// ISO-8859-1 bytes of a text (characters above 0xFF become '?').
pub fn latin1(t: &str) -> Vec<u8> {
    t.chars()
        .map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
        .collect()
}

/// The text of ISO-8859-1 bytes.
pub fn from_latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn text_body(t: &str) -> Vec<u8> {
    let mut b = vec![0u8];
    b.extend(latin1(t));
    b
}

impl Psd {
    /// The ID3 tag (1028s clause 5.3 and appendix A): the ID3v2.3.0 header, then the frames. Empty text frames are left out
    /// (1028s clause 6).
    pub fn to_id3(&self) -> Vec<u8> {
        let mut f = Vec::new();
        for (id, t) in [
            (b"TIT2", &self.title),
            (b"TPE1", &self.artist),
            (b"TALB", &self.album),
            (b"TCON", &self.genre),
        ] {
            if !t.is_empty() {
                id3_frame(&mut f, id, &text_body(t));
            }
        }
        if !self.comment.is_empty() {
            let mut c = vec![0u8];
            c.extend_from_slice(b"eng");
            c.push(0);
            c.extend(latin1(&self.comment));
            id3_frame(&mut f, b"COMM", &c);
        }
        if !self.ufid.is_empty() {
            id3_frame(&mut f, b"UFID", &latin1(&self.ufid));
        }
        if let Some((price, seller, desc, url, until)) = &self.commercial {
            let mut c = vec![0u8];
            c.extend(latin1(price));
            c.push(0);
            c.extend(latin1(until));
            c.extend(latin1(url));
            c.push(0);
            c.push(1); // received as: standard CD album with other songs
            c.extend(latin1(seller));
            c.push(0);
            c.extend(latin1(desc));
            c.push(0);
            id3_frame(&mut f, b"COMR", &c);
        }
        if let Some((lot, mime)) = self.xhdr {
            let mut x = mime.to_be_bytes().to_vec();
            x.push(0); // parameter: the LOT id
            x.push(4);
            x.extend_from_slice(&lot.to_be_bytes());
            id3_frame(&mut f, b"XHDR", &x);
        }
        let mut t = ID3_HEADER.to_vec();
        let n = f.len() as u32;
        t.extend_from_slice(&[
            (n >> 21) as u8 & 0x7F,
            (n >> 14) as u8 & 0x7F,
            (n >> 7) as u8 & 0x7F,
            n as u8 & 0x7F,
        ]);
        t.extend_from_slice(&f);
        t
    }
    /// Reads an ID3v2.3.0 tag (1028s appendix A clause 3). `None` when the header or a frame size is wrong.
    pub fn from_id3(b: &[u8]) -> Option<Psd> {
        if b.len() < 10 || b[..3] != ID3_HEADER[..3] {
            return None;
        }
        let n = ((b[6] as usize) << 21)
            | ((b[7] as usize) << 14)
            | ((b[8] as usize) << 7)
            | b[9] as usize;
        let tag = b.get(10..10 + n)?;
        let mut p = 0;
        let mut psd = Psd::default();
        let text = |d: &[u8]| {
            from_latin1(d.get(1..).unwrap_or(&[]))
                .trim_end_matches('\0')
                .to_string()
        };
        while p + 10 <= tag.len() {
            let id = &tag[p..p + 4];
            let len = u32::from_be_bytes([tag[p + 4], tag[p + 5], tag[p + 6], tag[p + 7]]) as usize;
            let body = tag.get(p + 10..p + 10 + len)?;
            match id {
                b"TIT2" => psd.title = text(body),
                b"TPE1" => psd.artist = text(body),
                b"TALB" => psd.album = text(body),
                b"TCON" => psd.genre = text(body),
                b"COMM" if body.len() >= 5 => {
                    let rest = &body[4..];
                    let i = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
                    psd.comment = from_latin1(rest.get(i + 1..).unwrap_or(&[]));
                }
                b"UFID" => psd.ufid = from_latin1(body),
                b"COMR" if !body.is_empty() => {
                    let parts: Vec<String> =
                        body[1..].split(|&c| c == 0).map(from_latin1).collect();
                    // price \0 valid-until(8) url \0 received-as seller \0 desc \0
                    if parts.len() >= 4 {
                        let price = parts[0].clone();
                        let until: String = parts[1].chars().take(8).collect();
                        let url: String = parts[1].chars().skip(8).collect();
                        let seller = parts[2].chars().skip(1).collect();
                        let desc = parts[3].clone();
                        psd.commercial = Some((price, seller, desc, url, until));
                    }
                }
                b"XHDR" if body.len() >= 10 => {
                    let mime = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
                    let lot = u32::from_be_bytes([body[6], body[7], body[8], body[9]]);
                    psd.xhdr = Some((lot, mime));
                }
                _ => {}
            }
            p += 10 + len;
        }
        Some(psd)
    }
}

/// The PSD port of a program: 0x5100 for the main program, 0x5201 to 0x5207 for programs 1 to 7 (1019s table 5-2, 1085s clause
/// 5.2.1).
pub fn psd_port(program: u8) -> u16 {
    if program == 0 {
        0x5100
    } else {
        0x5200 + program as u16
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// HDLC framing of AAS and PSD packets

/// The data transport packet format of the basic packet, and the PSD protocol field (1019s table 6-2, 1085s clause 5.1.1.2).
pub const DTPF_BASIC: u8 = 0x21;

/// Wraps a packet in an HDLC-like frame (1019s clause 6.2, 1085s clause 5.1): flag, DTPF 0x21, the port and the sequence number
/// little-endian, the payload, the FCS-16 of RFC 1662 low byte first, all escaped, and a closing flag.
pub fn hdlc_frame(port: u16, seq: u16, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![DTPF_BASIC];
    f.extend_from_slice(&port.to_le_bytes());
    f.extend_from_slice(&seq.to_le_bytes());
    f.extend_from_slice(payload);
    let _ = CRC16_IBM_SDLC.append(&mut f, Trailer::Little);
    let mut out = Vec::with_capacity(f.len() + 8);
    ByteStuffing::PPP.frame(&f, &mut out);
    out
}

/// An AAS or PSD packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AasPacket {
    /// Port.
    pub port: u16,
    /// Sequence.
    pub seq: u16,
    /// Payload.
    pub payload: Vec<u8>,
}

/// Takes an HDLC byte stream apart into packets (1019s clause 6.2).
#[derive(Clone, Debug)]
pub struct AasReceiver {
    frames: Unstuffer,
    /// Packets with a good FCS.
    pub good: u64,
    /// Bad FCS.
    pub bad: u64,
}

impl Default for AasReceiver {
    fn default() -> Self {
        AasReceiver {
            frames: Unstuffer::new(ByteStuffing::PPP, 4096),
            good: 0,
            bad: 0,
        }
    }
}

impl AasReceiver {
    /// Starts over (sync lost).
    pub fn reset(&mut self) {
        self.frames = Unstuffer::new(ByteStuffing::PPP, 4096);
    }
    /// Feeds bytes. Complete packets of the basic format go to `out`. Frames of other formats are dropped (1085s clause
    /// 5.1.1.2).
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<AasPacket>) {
        for &b in bytes {
            let Some(len) = self.frames.push(b) else {
                continue;
            };
            let f = self.frames.frame();
            if len < 7 {
                continue;
            }
            if CRC16_IBM_SDLC.verify(f, Trailer::Little) && f[0] == DTPF_BASIC {
                self.good += 1;
                out.push(AasPacket {
                    port: u16::from_le_bytes([f[1], f[2]]),
                    seq: u16::from_le_bytes([f[3], f[4]]),
                    payload: f[5..len - 2].to_vec(),
                });
            } else {
                self.bad += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The fixed data bearer

/// A fixed sub-channel: its FEC mode and its bytes in each L2 PDU (1019s figure 7-3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SubChannel {
    /// Reed-Solomon parity bytes per codeword, Np (0: no FEC).
    pub parity: u8,
    /// Interleaver rows, Rw (0: no interleaving).
    pub depth: u8,
    /// Bytes in each L2 PDU.
    pub len: u16,
}

/// The fixed bearer configuration that the configuration control channel sends (1019s table 7-3).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct FixedConfig {
    /// The maximum packet length indicator (1019s clause 7.5.2.1).
    pub mpl: u8,
    /// One to four sub-channels.
    pub subs: Vec<SubChannel>,
}

impl FixedConfig {
    /// The total bytes of the sub-channels.
    pub fn streams_len(&self) -> usize {
        self.subs.iter().map(|s| s.len as usize).sum()
    }
}

/// The CCC bytes of one configuration message for each number of sub-channels, 1 to 4, flag included (1019s table 7-4).
pub const CCC_LENGTH: [usize; 4] = [8, 12, 16, 20];

/// The synchronization channel width byte for a CCC of `width` bytes: 0xNN with N the half width, 0x00 for one byte (1019s
/// table 7-2). `None` for a width the table does not allow.
pub fn sync_width_byte(width: usize) -> Option<u8> {
    match width {
        1 => Some(0),
        2..=30 if width.is_multiple_of(2) => {
            let n = (width / 2) as u8;
            Some(n << 4 | n)
        }
        _ => None,
    }
}

/// Returns the CCC width that a synchronization width byte encodes (1019s table 7-2), or `None` when the two nibbles differ.
pub fn ccc_width(b: u8) -> Option<usize> {
    let (hi, lo) = (b >> 4, b & 15);
    (hi == lo).then_some(if hi == 0 { 1 } else { 2 * hi as usize })
}

/// The bytes of one configuration control channel message: the flag, then MPL, each sub-channel's mode (Np then Rw) and length
/// (little-endian), and the FCS-16, escaped (1019s clause 7.5.2, table 7-3 and figure 7-3). The flag is shared with the next
/// message.
pub fn ccc_message(cfg: &FixedConfig) -> Vec<u8> {
    let mut m = vec![cfg.mpl];
    for s in cfg.subs.iter().take(4) {
        m.extend_from_slice(&[s.parity, s.depth]);
        m.extend_from_slice(&s.len.to_le_bytes());
    }
    let _ = CRC16_IBM_SDLC.append(&mut m, Trailer::Little);
    let mut out = vec![FLAG];
    ByteStuffing::PPP.escape(&m, &mut out);
    out
}

/// Reads one configuration control channel message (unescaped, FCS included, 1019s table 7-3). `None` when the FCS or the
/// length is wrong.
pub fn parse_ccc(m: &[u8]) -> Option<FixedConfig> {
    if m.len() < 7 || !(m.len() - 3).is_multiple_of(4) || !CRC16_IBM_SDLC.verify(m, Trailer::Little)
    {
        return None;
    }
    let subs = m[1..m.len() - 2]
        .chunks(4)
        .map(|c| SubChannel {
            parity: c[0],
            depth: c[1],
            len: u16::from_le_bytes([c[2], c[3]]),
        })
        .collect::<Vec<_>>();
    (subs.len() <= 4).then_some(FixedConfig { mpl: m[0], subs })
}

/// Writes the fixed bearer at the end of each L2 PDU (1019s clause 7.5, figure 7-2): the sub-channel streams, the CCC bytes, then
/// one synchronization byte (count, width, width, width, the count stepping by 4).
#[derive(Clone, Debug)]
pub struct FixedWriter {
    cfg: FixedConfig,
    width: usize,
    ccc: std::collections::VecDeque<u8>,
    frames: u64,
}

impl FixedWriter {
    /// A writer for `cfg` with a CCC of `width` bytes a PDU. `None` for a width that 1019s table 7-2 does not allow.
    pub fn new(cfg: FixedConfig, width: usize) -> Option<FixedWriter> {
        sync_width_byte(width)?;
        Some(FixedWriter {
            cfg,
            width,
            ccc: Default::default(),
            frames: 0,
        })
    }
    /// The bytes of the region at the end of each L2 PDU.
    pub fn region_len(&self) -> usize {
        self.cfg.streams_len() + self.width + 1
    }
    /// The configuration.
    pub fn config(&self) -> &FixedConfig {
        &self.cfg
    }
    /// Appends the region of one L2 PDU to `out`: `streams[i]` fills sub-channel `i` (cut or padded with flags to its length).
    pub fn region(&mut self, streams: &[&[u8]], out: &mut Vec<u8>) {
        for (i, s) in self.cfg.subs.iter().enumerate() {
            let data = streams.get(i).copied().unwrap_or(&[]);
            let n = s.len as usize;
            out.extend_from_slice(&data[..data.len().min(n)]);
            out.extend(std::iter::repeat_n(FLAG, n.saturating_sub(data.len())));
        }
        while self.ccc.len() < self.width {
            self.ccc.extend(ccc_message(&self.cfg));
        }
        out.extend(self.ccc.drain(..self.width));
        let k = self.frames;
        out.push(if k.is_multiple_of(4) {
            ((k / 4) * 4) as u8
        } else {
            sync_width_byte(self.width).unwrap_or(0)
        });
        self.frames += 1;
    }
}

/// Reads the fixed bearer at the end of each L2 PDU (1019s clause 7.5): learns the CCC width from the synchronization channel
/// (the same width byte in two PDUs in a row), then the configuration from the CCC messages, then returns the sub-channel bytes.
#[derive(Clone, Debug)]
pub struct FixedReader {
    last_sync: Option<u8>,
    width: Option<usize>,
    ccc: Unstuffer,
    /// The configuration, once a CCC message is read.
    pub config: Option<FixedConfig>,
}

impl Default for FixedReader {
    fn default() -> Self {
        FixedReader {
            last_sync: None,
            width: None,
            ccc: Unstuffer::new(ByteStuffing::PPP, 64),
            config: None,
        }
    }
}

impl FixedReader {
    /// Starts over.
    pub fn reset(&mut self) {
        *self = FixedReader::default();
    }
    /// The bytes the fixed region takes at the end of a PDU, once known.
    pub fn region_len(&self) -> Option<usize> {
        Some(self.config.as_ref()?.streams_len() + self.width? + 1)
    }
    /// Takes the `payload` of one L2 PDU that carries fixed data. It returns the sub-channel byte ranges of `payload` (index,
    /// start, end).
    pub fn push(&mut self, payload: &[u8]) -> Vec<(usize, usize, usize)> {
        let Some(&sync) = payload.last() else {
            return Vec::new();
        };
        if let Some(w) = ccc_width(sync) {
            if self.last_sync == Some(sync) {
                self.width = Some(w);
            }
        }
        self.last_sync = Some(sync);
        let Some(w) = self.width else {
            return Vec::new();
        };
        let n = payload.len();
        if w + 1 > n {
            return Vec::new();
        }
        for &b in &payload[n - 1 - w..n - 1] {
            if self.ccc.push(b).is_some() {
                if let Some(c) = parse_ccc(self.ccc.frame()) {
                    self.config = Some(c);
                }
            }
        }
        let Some(cfg) = &self.config else {
            return Vec::new();
        };
        let total = cfg.streams_len();
        if total + w + 1 > n {
            return Vec::new();
        }
        let mut at = n - 1 - w - total;
        cfg.subs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let r = (i, at, at + s.len as usize);
                at += s.len as usize;
                r
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// LOT (sdrkit's own layout)

/// The LOT fragment payload: LOT id (2), fragment number (4), then on fragment 0 the header: version (1), size (4), MIME hash (4),
/// expiry MJD (4), name length (1), name. Then the data.
pub fn lot_fragments(
    lot: u16,
    name: &str,
    mime: u32,
    expiry_mjd: u32,
    body: &[u8],
    frag: usize,
) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut head = vec![1u8];
    head.extend_from_slice(&(body.len() as u32).to_be_bytes());
    head.extend_from_slice(&mime.to_be_bytes());
    head.extend_from_slice(&expiry_mjd.to_be_bytes());
    head.push(name.len().min(255) as u8);
    head.extend_from_slice(&name.as_bytes()[..name.len().min(255)]);
    for (i, c) in body.chunks(frag).enumerate() {
        let mut f = lot.to_be_bytes().to_vec();
        f.extend_from_slice(&(i as u32).to_be_bytes());
        if i == 0 {
            f.extend_from_slice(&head);
        }
        f.extend_from_slice(c);
        out.push(f);
    }
    out
}

/// A LOT file being received or complete.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct LotFile {
    /// Port.
    pub port: u16,
    /// LOT id.
    pub lot: u16,
    /// Name.
    pub name: String,
    /// MIME hash.
    pub mime: u32,
    /// Size.
    pub size: u32,
    /// Expiry (MJD).
    pub expiry_mjd: u32,
    /// Bytes received.
    pub received: usize,
    /// Complete.
    pub complete: bool,
    /// Version (changes with the bytes).
    pub version: u32,
    /// Signal time of the last update.
    pub updated_s: f64,
    /// The bytes when complete.
    #[serde(skip)]
    pub bytes: Option<std::sync::Arc<[u8]>>,
    #[serde(skip)]
    frags: BTreeMap<u32, Vec<u8>>,
    #[serde(skip)]
    hash: u64,
}

/// Collects LOT files.
#[derive(Clone, Debug, Default)]
pub struct LotCollector {
    /// Files by (port, LOT id).
    pub files: BTreeMap<(u16, u16), LotFile>,
}

impl LotCollector {
    /// Takes one fragment. It returns the key of a file that completed (or changed and completed again).
    pub fn push(&mut self, port: u16, payload: &[u8], t: f64) -> Option<(u16, u16)> {
        if payload.len() < 6 {
            return None;
        }
        let lot = u16::from_be_bytes([payload[0], payload[1]]);
        let frag = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
        if frag >= 2048 {
            return None;
        }
        let f = self.files.entry((port, lot)).or_insert_with(|| LotFile {
            port,
            lot,
            ..Default::default()
        });
        let mut data = &payload[6..];
        if frag == 0 {
            if data.len() < 14 {
                return None;
            }
            f.size = u32::from_be_bytes([data[1], data[2], data[3], data[4]]).min(1 << 20);
            f.mime = u32::from_be_bytes([data[5], data[6], data[7], data[8]]);
            f.expiry_mjd = u32::from_be_bytes([data[9], data[10], data[11], data[12]]);
            let n = data[13] as usize;
            f.name = String::from_utf8_lossy(data.get(14..14 + n)?).to_string();
            data = &data[14 + n..];
        }
        f.frags.insert(frag, data.to_vec());
        f.received = f.frags.values().map(|v| v.len()).sum();
        f.updated_s = t;
        if f.size > 0 && f.received >= f.size as usize {
            let n = *f.frags.keys().last()? as usize + 1;
            if f.frags.len() == n {
                let mut b = Vec::with_capacity(f.size as usize);
                for v in f.frags.values() {
                    b.extend_from_slice(v);
                }
                b.truncate(f.size as usize);
                let h = b.iter().fold(0xCBF2_9CE4_8422_2325u64, |a, &x| {
                    (a ^ x as u64).wrapping_mul(0x100_0000_01B3)
                });
                if !f.complete || h != f.hash {
                    f.complete = true;
                    f.hash = h;
                    f.version += 1;
                    f.bytes = Some(b.into());
                    // At most 64 files: the oldest complete one goes first.
                    let key = (port, lot);
                    if self.files.len() > 64 {
                        let old = self
                            .files
                            .iter()
                            .filter(|(k, v)| v.complete && **k != key)
                            .min_by(|a, b| a.1.updated_s.total_cmp(&b.1.updated_s))
                            .map(|(k, _)| *k);
                        if let Some(k) = old {
                            self.files.remove(&k);
                        }
                    }
                    return Some(key);
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// SIG (sdrkit's own layout)

/// A component of a SIG service.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SigComponent {
    /// Data (else audio).
    pub data: bool,
    /// Component id.
    pub id: u8,
    /// Port (data) or program number (audio).
    pub port: u16,
    /// Type (AAS 0 stream, 1 packet, 3 LOT).
    pub kind: u8,
    /// MIME hash.
    pub mime: u32,
    /// Service data type.
    pub sdt: u16,
}

/// A SIG service.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SigService {
    /// Data service (else audio).
    pub data: bool,
    /// Service number.
    pub number: u16,
    /// Name.
    pub name: String,
    /// Components.
    pub components: Vec<SigComponent>,
}

/// The SIG port.
pub const SIG_PORT: u16 = 0x0020;

/// Encodes the SIG.
pub fn sig_bytes(services: &[SigService]) -> Vec<u8> {
    let mut b = vec![services.len() as u8];
    for s in services {
        b.push(s.data as u8);
        b.extend_from_slice(&s.number.to_be_bytes());
        b.push(s.name.len().min(255) as u8);
        b.extend_from_slice(&s.name.as_bytes()[..s.name.len().min(255)]);
        b.push(s.components.len() as u8);
        for c in &s.components {
            b.push(c.data as u8);
            b.push(c.id);
            b.extend_from_slice(&c.port.to_be_bytes());
            b.push(c.kind);
            b.extend_from_slice(&c.mime.to_be_bytes());
            b.extend_from_slice(&c.sdt.to_be_bytes());
        }
    }
    b
}

/// Decodes the SIG.
pub fn parse_sig(b: &[u8]) -> Option<Vec<SigService>> {
    let n = *b.first()? as usize;
    let mut p = 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let data = *b.get(p)? != 0;
        let number = u16::from_be_bytes([*b.get(p + 1)?, *b.get(p + 2)?]);
        let nl = *b.get(p + 3)? as usize;
        let name = String::from_utf8_lossy(b.get(p + 4..p + 4 + nl)?).to_string();
        p += 4 + nl;
        let nc = *b.get(p)? as usize;
        p += 1;
        let mut components = Vec::new();
        for _ in 0..nc {
            let c = b.get(p..p + 11)?;
            components.push(SigComponent {
                data: c[0] != 0,
                id: c[1],
                port: u16::from_be_bytes([c[2], c[3]]),
                kind: c[4],
                mime: u32::from_be_bytes([c[5], c[6], c[7], c[8]]),
                sdt: u16::from_be_bytes([c[9], c[10]]),
            });
            p += 11;
        }
        out.push(SigService {
            data,
            number,
            name,
            components,
        });
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------------------------------------
// SIS (PIDS)

/// The station information.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Sis {
    /// Short station name (call sign).
    pub short_name: String,
    /// The "-FM" extension of the short name.
    pub fm_suffix: bool,
    /// Universal short station name.
    pub universal_name: String,
    /// Long station name.
    pub long_name: String,
    /// Station slogan.
    pub slogan: String,
    /// Station message.
    pub message: String,
    /// The station message has high priority.
    pub message_priority: bool,
    /// Country code.
    pub country: String,
    /// FCC facility id.
    pub facility_id: u32,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Altitude, metres.
    pub alt_m: u32,
    /// Audio services: (program, type, restricted, sound experience).
    pub audio_services: Vec<(u8, u8, bool, u8)>,
    /// Data services: (type, MIME 12 bits, restricted).
    pub data_services: Vec<(u16, u16, bool)>,
    /// SIS parameter messages by index.
    pub params: BTreeMap<u8, u16>,
    /// Time locked to GPS.
    pub time_locked: bool,
    /// The absolute L1 frame number, when read.
    pub alfn: Option<u32>,
}

/// A SIS message type (1020s table 4-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    /// 0000 Station ID number.
    StationId,
    /// 0001 Station name, short format.
    ShortName,
    /// 0010 Station name, long format.
    LongName,
    /// 0100 Station location.
    Location,
    /// 0101 Station message.
    Message,
    /// 0110 Service information.
    ServiceInfo,
    /// 0111 SIS parameter.
    Parameter,
    /// 1000 with name type 0: universal short station name.
    UniversalName,
    /// 1000 with name type 1: station slogan.
    Slogan,
}

impl Msg {
    /// The MSG ID (1020s table 4-1).
    pub fn id(self) -> u8 {
        match self {
            Msg::StationId => 0b0000,
            Msg::ShortName => 0b0001,
            Msg::LongName => 0b0010,
            Msg::Location => 0b0100,
            Msg::Message => 0b0101,
            Msg::ServiceInfo => 0b0110,
            Msg::Parameter => 0b0111,
            Msg::UniversalName | Msg::Slogan => 0b1000,
        }
    }
}

/// The payload bits of each MSG ID 0 to 10 (1020s table 4-1). `None` for a reserved ID.
pub fn msg_payload_bits(id: u8) -> Option<usize> {
    const BITS: [Option<usize>; 11] = [
        Some(32),
        Some(22),
        Some(58),
        Some(32),
        Some(27),
        Some(58),
        Some(27),
        Some(22),
        Some(58),
        Some(58),
        Some(27),
    ];
    BITS.get(id as usize).copied().flatten()
}

/// One PIDS block of the schedule: one message, or two messages in an extended PDU.
pub type Slot = (Msg, Option<Msg>);

/// The FM SIS schedule for a station name of at most 4 characters, EA not active (1020s table 5-1).
pub const FM_SCHEDULE: [Slot; 16] = [
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::ServiceInfo, Some(Msg::ServiceInfo)),
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::Slogan, None),
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::ServiceInfo, Some(Msg::ServiceInfo)),
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::Location, Some(Msg::Location)),
    (Msg::Message, None),
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::ServiceInfo, Some(Msg::ServiceInfo)),
    (Msg::LongName, None),
    (Msg::Parameter, Some(Msg::StationId)),
    (Msg::Message, None),
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::ServiceInfo, Some(Msg::ServiceInfo)),
];

/// The AM SIS schedule for a station name of at most 4 characters, EA not active (1020s table 5-5).
pub const AM_SCHEDULE: [Slot; 8] = [
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::Message, None),
    (Msg::ServiceInfo, Some(Msg::ShortName)),
    (Msg::Parameter, Some(Msg::Location)),
    (Msg::ShortName, Some(Msg::StationId)),
    (Msg::Slogan, None),
    (Msg::ServiceInfo, Some(Msg::ShortName)),
    (Msg::LongName, None),
];

/// The short name characters by value: A to Z, space, '?', '-', '*', '$' (1020s tables 4-3 and 4-5).
pub const SHORT_NAME_SET: &[u8; 31] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ ?-*$";

/// The 12-bit SIS CRC generator `g(x) = x^12 + x^11 + x^3 + x + 1` (1020s clause 4.11).
pub const SIS_CRC_POLY: u128 = 0x180B;

/// The CRC of a SIS PDU (1020s clause 4.11): the remainder of PDU bits 79 to 0, bit 79 the highest power and bits 68 to 79 taken
/// as zeros, divided by [`SIS_CRC_POLY`]. Bit 68 of the PDU holds its most significant bit.
pub fn sis_crc(bits: &[u8]) -> u16 {
    let v: Vec<u8> = (0..80)
        .rev()
        .map(|i| {
            if i >= 68 {
                0
            } else {
                bits.get(i).map_or(0, |b| b & 1)
            }
        })
        .collect();
    remainder(&v, 0, SIS_CRC_POLY, 12) as u16
}

/// The two ADV ALFN bits of a PIDS block (1020s clause 5.3): FM block `b` carries ALFN bits `2b + 1` and `2b` (0 the least
/// significant). AM block `b` carries bits `2b + 1` and `2b`, and in a frame whose two lowest bits are 00 it carries bits
/// `2b + 17` and `2b + 16` instead.
pub fn adv_alfn(alfn: u32, block: usize, fm: bool) -> u8 {
    let shift = if !fm && alfn & 3 == 0 {
        16 + 2 * block
    } else {
        2 * block
    };
    ((alfn >> (shift % 32)) & 3) as u8
}

/// Writes a SIS PDU (1020s figure 4-1): Type 0, Ext, the message(s) (each a MSG ID and its payload, as bits), zeros to bit 63,
/// reserved bit 64, time locked bit 65, ADV ALFN bits 66 and 67, and the CRC. `None` when the messages do not fit (58 bits for
/// one payload, 54 for two).
pub fn sis_pdu(m1: &[u8], m2: Option<&[u8]>, time_locked: bool, alfn_bits: u8) -> Option<Vec<u8>> {
    let mut b = Vec::with_capacity(80);
    push_u64(&mut b, 0, 1);
    push_u64(&mut b, m2.is_some() as u64, 1);
    b.extend_from_slice(m1);
    if let Some(m) = m2 {
        b.extend_from_slice(m);
    }
    if b.len() > 64 {
        return None;
    }
    b.resize(64, 0);
    push_u64(&mut b, 0, 1);
    push_u64(&mut b, time_locked as u64, 1);
    push_u64(&mut b, alfn_bits as u64 & 3, 2);
    let crc = sis_crc(&b);
    push_u64(&mut b, crc as u64, 12);
    Some(b)
}

fn short_name_index(c: u8) -> u64 {
    SHORT_NAME_SET
        .iter()
        .position(|&x| x == c.to_ascii_uppercase())
        .unwrap_or(26) as u64
}

/// The two 5-bit letters of a country code (1020s table 4-3: A is 0).
fn country_bits(c: &str) -> u64 {
    let b = c.as_bytes();
    let l = |i: usize| {
        (b.get(i)
            .copied()
            .unwrap_or(b'A')
            .to_ascii_uppercase()
            .saturating_sub(b'A')
            & 31) as u64
    };
    l(0) << 5 | l(1)
}

/// A message: its MSG ID then its payload bits.
fn msg_bits(id: u8, f: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut w = Vec::with_capacity(62);
    push_u64(&mut w, id as u64, 4);
    f(&mut w);
    w
}

/// The station ID number message (1020s figure 4-2): country code 10 bits, 3 reserved bits, facility ID 19 bits.
pub fn station_id_msg(country: &str, facility_id: u32) -> Vec<u8> {
    msg_bits(0, |w| {
        push_u64(w, country_bits(country), 10);
        push_u64(w, 0, 3);
        push_u64(w, facility_id as u64 & 0x7_FFFF, 19);
    })
}

/// The short station name message (1020s figure 4-3, table 4-4): four 5-bit characters, then the extension (01: "-FM").
pub fn short_name_msg(name: &str, fm: bool) -> Vec<u8> {
    msg_bits(1, |w| {
        let b = name.as_bytes();
        for i in 0..4 {
            push_u64(w, short_name_index(b.get(i).copied().unwrap_or(b' ')), 5);
        }
        push_u64(w, fm as u64, 2);
    })
}

/// The two station location messages, high portion first (1020s figure 4-5): latitude and longitude in S9.13 two's complement,
/// the altitude in units of 16 m (0 to 255) split in its high and low nibbles.
pub fn location_msgs(lat: f64, lon: f64, alt_m: f64) -> [Vec<u8>; 2] {
    let alt = (alt_m / 16.0).round().clamp(0.0, 255.0) as u64;
    let fix = |d: f64| ((d * 8192.0).round() as i64 as u64) & 0x3F_FFFF;
    [
        msg_bits(4, |w| {
            push_u64(w, 1, 1);
            push_u64(w, fix(lat), 22);
            push_u64(w, alt >> 4, 4);
        }),
        msg_bits(4, |w| {
            push_u64(w, 0, 1);
            push_u64(w, fix(lon), 22);
            push_u64(w, alt & 15, 4);
        }),
    ]
}

/// The 7-bit checksum of a station message (1020s figure 4-7): the 16-bit sum of the bytes, its high byte without bit 15 added
/// to its low byte, the seven low bits kept.
pub fn message_checksum(b: &[u8]) -> u8 {
    let s = b.iter().fold(0u16, |a, &x| a.wrapping_add(x as u16));
    ((((s >> 8) & 0x7F) + (s & 0xFF)) & 0x7F) as u8
}

/// The frames of a station message in ISO-8859-1 (1020s figure 4-6, tables 4-6 and 4-7): frame 0 carries the frame number,
/// sequence (2 bits), priority, text encoding 000, length, checksum and bytes 0 to 3, the others the frame number, sequence,
/// 3 reserved bits and 6 bytes. At most 190 bytes and 32 frames.
pub fn message_msgs(text: &[u8], seq: u8, priority: bool) -> Vec<Vec<u8>> {
    let t = &text[..text.len().min(190)];
    let n = 1 + t.len().saturating_sub(4).div_ceil(6);
    let byte = |i: usize| t.get(i).copied().unwrap_or(0) as u64;
    (0..n)
        .map(|f| {
            msg_bits(5, |w| {
                push_u64(w, f as u64, 5);
                push_u64(w, seq as u64 & 3, 2);
                if f == 0 {
                    push_u64(w, priority as u64, 1);
                    push_u64(w, 0, 3);
                    push_u64(w, t.len() as u64, 8);
                    push_u64(w, message_checksum(t) as u64, 7);
                    (0..4).for_each(|i| push_u64(w, byte(i), 8));
                } else {
                    push_u64(w, 0, 3);
                    (0..6).for_each(|i| push_u64(w, byte(6 * f - 2 + i), 8));
                }
            })
        })
        .collect()
}

/// The frames of a long station name (1020s figure 4-4): last frame number, current frame number, seven 7-bit characters, the
/// sequence number. At most 56 characters and 8 frames.
pub fn long_name_msgs(text: &[u8], seq: u8) -> Vec<Vec<u8>> {
    let t = &text[..text.len().min(56)];
    let n = t.len().div_ceil(7).max(1);
    (0..n)
        .map(|f| {
            msg_bits(2, |w| {
                push_u64(w, (n - 1) as u64, 3);
                push_u64(w, f as u64, 3);
                for i in 0..7 {
                    push_u64(w, (t.get(7 * f + i).copied().unwrap_or(0) & 0x7F) as u64, 7);
                }
                push_u64(w, seq as u64 & 7, 3);
            })
        })
        .collect()
}

/// The frames of a universal short station name in ISO-8859-1 (1020s figure 4-26): frame 0 carries the frame number, name type
/// 0, text encoding, the "-FM" append bit, the length bit and bytes 0 to 5, frame 1 bytes 6 to 11. At most 12 bytes.
pub fn universal_name_msgs(text: &[u8], fm: bool) -> Vec<Vec<u8>> {
    let t = &text[..text.len().min(12)];
    let two = t.len() > 6;
    let byte = |i: usize| t.get(i).copied().unwrap_or(0) as u64;
    (0..1 + two as usize)
        .map(|f| {
            msg_bits(8, |w| {
                push_u64(w, f as u64, 4);
                push_u64(w, 0, 1);
                if f == 0 {
                    push_u64(w, 0, 3);
                    push_u64(w, fm as u64, 1);
                    push_u64(w, two as u64, 1);
                } else {
                    push_u64(w, 0, 5);
                }
                (0..6).for_each(|i| push_u64(w, byte(6 * f + i), 8));
            })
        })
        .collect()
}

/// The frames of a station slogan in ISO-8859-1 (1020s figure 4-27, tables 4-21 and 4-22): frame 0 carries the frame number,
/// name type 1, text encoding, 3 reserved bits, the length and bytes 0 to 4, the others 5 reserved bits and bytes `6n - 1` to
/// `6n + 4`. At most 95 bytes and 16 frames.
pub fn slogan_msgs(text: &[u8]) -> Vec<Vec<u8>> {
    let t = &text[..text.len().min(95)];
    let n = 1 + t.len().saturating_sub(5).div_ceil(6);
    let byte = |i: usize| t.get(i).copied().unwrap_or(0) as u64;
    (0..n)
        .map(|f| {
            msg_bits(8, |w| {
                push_u64(w, f as u64, 4);
                push_u64(w, 1, 1);
                if f == 0 {
                    push_u64(w, 0, 3);
                    push_u64(w, 0, 3);
                    push_u64(w, t.len() as u64, 7);
                    (0..5).for_each(|i| push_u64(w, byte(i), 8));
                } else {
                    push_u64(w, 0, 5);
                    (0..6).for_each(|i| push_u64(w, byte(6 * f - 1 + i), 8));
                }
            })
        })
        .collect()
}

/// An audio service descriptor message (1020s figure 4-9): category 00, access, program number (6 bits), program type, 5
/// reserved bits, applied sound experience (5 bits).
pub fn audio_service_msg(program: u8, ty: u8, restricted: bool, sound: u8) -> Vec<u8> {
    msg_bits(6, |w| {
        push_u64(w, 0, 2);
        push_u64(w, restricted as u64, 1);
        push_u64(w, program as u64, 6);
        push_u64(w, ty as u64, 8);
        push_u64(w, 0, 5);
        push_u64(w, sound as u64, 5);
    })
}

/// A data service descriptor message (1020s figure 4-10): category 01, access, service data type (9 bits), 3 reserved bits, the
/// 12 low bits of the MIME type hash.
pub fn data_service_msg(ty: u16, mime: u16, restricted: bool) -> Vec<u8> {
    msg_bits(6, |w| {
        push_u64(w, 1, 2);
        push_u64(w, restricted as u64, 1);
        push_u64(w, ty as u64, 9);
        push_u64(w, 0, 3);
        push_u64(w, mime as u64 & 0xFFF, 12);
    })
}

/// A SIS parameter message (1020s figure 4-11): a 6-bit index and a 16-bit parameter.
pub fn parameter_msg(index: u8, value: u16) -> Vec<u8> {
    msg_bits(7, |w| {
        push_u64(w, index as u64, 6);
        push_u64(w, value as u64, 16);
    })
}

/// The leap second parameter (index 0) to send when the station has none: 18 seconds current and pending (1020s table 4-14).
pub const DEFAULT_LEAP_SECONDS: u16 = 0x1212;

/// Sends the station information on the PIDS logical channel, one SIS PDU a block, by the schedules of 1020s tables 5-1 (FM) and
/// 5-5 (AM). Each text message advances one frame each time its slot comes. A slot of an empty text carries the short name and
/// the station ID instead (1020s clause 5.2).
#[derive(Clone, Debug)]
pub struct SisTransmitter {
    fm: bool,
    time_locked: bool,
    frames: [Vec<Vec<u8>>; 9],
    at: [usize; 9],
}

fn msg_index(m: Msg) -> usize {
    match m {
        Msg::StationId => 0,
        Msg::ShortName => 1,
        Msg::LongName => 2,
        Msg::Location => 3,
        Msg::Message => 4,
        Msg::ServiceInfo => 5,
        Msg::Parameter => 6,
        Msg::UniversalName => 7,
        Msg::Slogan => 8,
    }
}

impl SisTransmitter {
    /// A transmitter of `sis` on FM (`fm`) or AM, with text sequence number `seq`.
    pub fn new(sis: &Sis, fm: bool, seq: u8) -> SisTransmitter {
        let text = |t: &str| latin1(t);
        let mut services: Vec<Vec<u8>> = sis
            .audio_services
            .iter()
            .map(|&(p, t, r, s)| audio_service_msg(p, t, r, s))
            .collect();
        services.extend(
            sis.data_services
                .iter()
                .map(|&(t, m, r)| data_service_msg(t, m, r)),
        );
        let mut params: Vec<Vec<u8>> = sis
            .params
            .iter()
            .map(|(&i, &v)| parameter_msg(i, v))
            .collect();
        if params.is_empty() {
            params.push(parameter_msg(0, DEFAULT_LEAP_SECONDS));
        }
        let nonempty = |v: Vec<Vec<u8>>, t: &str| if t.is_empty() { Vec::new() } else { v };
        SisTransmitter {
            fm,
            time_locked: sis.time_locked,
            frames: [
                vec![station_id_msg(&sis.country, sis.facility_id)],
                vec![short_name_msg(&sis.short_name, sis.fm_suffix)],
                nonempty(long_name_msgs(&text(&sis.long_name), seq), &sis.long_name),
                location_msgs(sis.lat, sis.lon, sis.alt_m as f64).to_vec(),
                nonempty(
                    message_msgs(&text(&sis.message), seq, sis.message_priority),
                    &sis.message,
                ),
                services,
                params,
                nonempty(
                    universal_name_msgs(&text(&sis.universal_name), sis.fm_suffix),
                    &sis.universal_name,
                ),
                nonempty(slogan_msgs(&text(&sis.slogan)), &sis.slogan),
            ],
            at: [0; 9],
        }
    }
    fn next(&mut self, m: Msg) -> Option<Vec<u8>> {
        let i = msg_index(m);
        let v = &self.frames[i];
        if v.is_empty() {
            return None;
        }
        let f = v[self.at[i] % v.len()].clone();
        self.at[i] += 1;
        Some(f)
    }
    /// The 80 bits of the SIS PDU of block `block` of the L1 frame with number `alfn`.
    pub fn pdu(&mut self, block: usize, alfn: u32) -> Vec<u8> {
        let (a, b) = if self.fm {
            FM_SCHEDULE[block % 16]
        } else {
            AM_SCHEDULE[block % 8]
        };
        let alfn_bits = adv_alfn(alfn, block, self.fm);
        let first = self.next(a);
        let second = b.and_then(|m| self.next(m));
        let pdu = match (first, second) {
            (Some(x), Some(y)) => sis_pdu(&x, Some(&y), self.time_locked, alfn_bits),
            (Some(x), None) if b.is_none() => sis_pdu(&x, None, self.time_locked, alfn_bits),
            _ => {
                let x = self.next(Msg::ShortName).unwrap_or_default();
                let y = self.next(Msg::StationId).unwrap_or_default();
                sis_pdu(&x, Some(&y), self.time_locked, alfn_bits)
            }
        };
        pdu.unwrap_or_else(|| vec![0; 80])
    }
}

/// The frames of a multi-frame text: sequence number, number of frames once known, bytes by frame.
#[derive(Clone, Debug, Default)]
struct TextFrames {
    seq: Option<u8>,
    count: Option<usize>,
    len: Option<usize>,
    checksum: Option<u8>,
    frames: BTreeMap<usize, Vec<u8>>,
}

impl TextFrames {
    fn frame(&mut self, seq: Option<u8>, f: usize, bytes: Vec<u8>) {
        if seq.is_some() && self.seq.is_some() && seq != self.seq {
            *self = TextFrames::default();
        }
        if seq.is_some() {
            self.seq = seq;
        }
        self.frames.insert(f, bytes);
    }
    /// The bytes when all frames are in.
    fn complete(&self) -> Option<Vec<u8>> {
        let n = self.count?;
        if (0..n).any(|f| !self.frames.contains_key(&f)) {
            return None;
        }
        let mut all: Vec<u8> = (0..n).flat_map(|f| self.frames[&f].clone()).collect();
        if let Some(l) = self.len {
            all.truncate(l);
        }
        while all.last() == Some(&0) {
            all.pop();
        }
        if let Some(c) = self.checksum {
            if message_checksum(&all) != c {
                return None;
            }
        }
        Some(all)
    }
}

/// One half of a station location: the latitude or longitude in 1/8192 degree, and the altitude nibble.
type LocationHalf = (i64, u64);

/// Reassembles the station information from SIS PDUs (1020s clause 4).
#[derive(Clone, Debug, Default)]
pub struct SisReceiver {
    /// What is complete.
    pub sis: Sis,
    long: TextFrames,
    message: TextFrames,
    slogan: TextFrames,
    universal: TextFrames,
    loc: [Option<LocationHalf>; 2],
    alfn_bits: BTreeMap<usize, u8>,
    alfn_lsb: Option<u16>,
    /// PIDS frames good.
    pub good: u64,
    /// PIDS frames with a bad CRC.
    pub bad: u64,
    /// Something changed in the last push.
    pub changed: bool,
}

fn signed22(v: u64) -> i64 {
    if v & (1 << 21) != 0 {
        v as i64 - (1 << 22)
    } else {
        v as i64
    }
}

impl SisReceiver {
    /// Takes one SIS PDU (80 bits) of block `block`, FM (`fm`) or AM.
    pub fn push(&mut self, bits: &[u8], block: usize, fm: bool) {
        self.changed = false;
        if bits.len() < 80 {
            self.bad += 1;
            return;
        }
        let crc = HardBits::at(bits, 68).u(12) as u16;
        if sis_crc(bits) != crc {
            self.bad += 1;
            return;
        }
        self.good += 1;
        let before = self.sis.clone();
        let mut r = HardBits::new(&bits[..68]);
        if r.u(1) != 0 {
            return; // an LLDS PDU
        }
        let ext = r.u(1) != 0;
        let id1 = r.u(4) as u8;
        self.message_payload(id1, &mut r);
        if ext {
            if let Some(n) = msg_payload_bits(id1) {
                let mut r2 = HardBits::at(&bits[..68], 6 + n);
                let id2 = r2.u(4) as u8;
                self.message_payload(id2, &mut r2);
            }
        }
        let mut r = HardBits::at(bits, 65);
        self.sis.time_locked = r.u(1) != 0;
        self.alfn(block, fm, r.u(2) as u8);
        self.changed = self.sis != before;
    }

    fn alfn(&mut self, block: usize, fm: bool, v: u8) {
        if block == 0 {
            self.alfn_bits.clear();
        }
        self.alfn_bits.insert(block, v);
        let n = if fm { 16 } else { 8 };
        if self.alfn_bits.len() < n || block != n - 1 {
            return;
        }
        let word = (0..n).fold(0u32, |a, b| a | (self.alfn_bits[&b] as u32) << (2 * b));
        if fm {
            self.sis.alfn = Some(word);
            return;
        }
        // AM: the frame after one whose low bits are 11 carries the 16 high bits (1020s clause 5.3.2).
        match self.alfn_lsb {
            Some(prev) if prev.wrapping_add(1) & 3 == 0 => {
                let lsb = prev.wrapping_add(1);
                self.sis.alfn = Some(word << 16 | lsb as u32);
                self.alfn_lsb = Some(lsb);
            }
            _ => {
                self.alfn_lsb = Some(word as u16);
                if let Some(a) = self.sis.alfn {
                    self.sis.alfn = Some((a & 0xFFFF_0000) | word);
                }
            }
        }
    }

    fn message_payload(&mut self, id: u8, r: &mut HardBits) {
        let s = &mut self.sis;
        match id {
            0 => {
                let c = r.u(10);
                let ch = |v: u64| (b'A' + (v & 31) as u8) as char;
                s.country = format!("{}{}", ch(c >> 5), ch(c));
                r.u(3);
                s.facility_id = r.u(19) as u32;
            }
            1 => {
                let name: String = (0..4)
                    .map(|_| *SHORT_NAME_SET.get(r.u(5) as usize).unwrap_or(&b' ') as char)
                    .collect();
                s.short_name = name.trim_end().to_string();
                s.fm_suffix = r.u(2) == 1;
            }
            2 => {
                let last = r.u(3) as usize;
                let cur = r.u(3) as usize;
                let chars: Vec<u8> = (0..7).map(|_| r.u(7) as u8).collect();
                let seq = r.u(3) as u8;
                self.long.frame(Some(seq), cur, chars);
                self.long.count = Some(last + 1);
                if let Some(t) = self.long.complete() {
                    s.long_name = from_latin1(&t);
                }
            }
            4 => {
                let high = r.u(1) != 0;
                let v = signed22(r.u(22));
                let alt = r.u(4);
                self.loc[!high as usize] = Some((v, alt));
                if let [Some((lat, ah)), Some((lon, al))] = self.loc {
                    s.lat = lat as f64 / 8192.0;
                    s.lon = lon as f64 / 8192.0;
                    s.alt_m = ((ah << 4 | al) * 16) as u32;
                }
            }
            5 => {
                let f = r.u(5) as usize;
                let seq = r.u(2) as u8;
                if f == 0 {
                    s.message_priority = r.u(1) != 0;
                    r.u(3);
                    let len = r.u(8) as usize;
                    let ck = r.u(7) as u8;
                    let b: Vec<u8> = (0..4).map(|_| r.u(8) as u8).collect();
                    self.message.frame(Some(seq), 0, b);
                    self.message.len = Some(len);
                    self.message.checksum = Some(ck);
                    self.message.count = Some(1 + len.saturating_sub(4).div_ceil(6));
                } else {
                    r.u(3);
                    let b: Vec<u8> = (0..6).map(|_| r.u(8) as u8).collect();
                    self.message.frame(Some(seq), f, b);
                }
                if let Some(t) = self.message.complete() {
                    s.message = from_latin1(&t);
                }
            }
            6 => match r.u(2) {
                0 => {
                    let restricted = r.u(1) != 0;
                    let p = r.u(6) as u8;
                    let ty = r.u(8) as u8;
                    r.u(5);
                    let sound = r.u(5) as u8;
                    s.audio_services.retain(|a| a.0 != p);
                    s.audio_services.push((p, ty, restricted, sound));
                    s.audio_services.sort();
                }
                1 => {
                    let restricted = r.u(1) != 0;
                    let ty = r.u(9) as u16;
                    r.u(3);
                    let mime = r.u(12) as u16;
                    if !s.data_services.contains(&(ty, mime, restricted)) {
                        s.data_services.push((ty, mime, restricted));
                    }
                }
                _ => {}
            },
            7 => {
                let i = r.u(6) as u8;
                let v = r.u(16) as u16;
                s.params.insert(i, v);
            }
            8 => {
                let f = r.u(4) as usize;
                if r.u(1) == 0 {
                    if f == 0 {
                        r.u(3);
                        s.fm_suffix |= r.u(1) != 0;
                        self.universal.count = Some(1 + r.u(1) as usize);
                    } else {
                        r.u(5);
                    }
                    let b: Vec<u8> = (0..6).map(|_| r.u(8) as u8).collect();
                    self.universal.frame(None, f, b);
                    if let Some(t) = self.universal.complete() {
                        s.universal_name = from_latin1(&t);
                    }
                } else {
                    let b: Vec<u8> = if f == 0 {
                        r.u(6);
                        let len = r.u(7) as usize;
                        self.slogan.len = Some(len);
                        self.slogan.count = Some(1 + len.saturating_sub(5).div_ceil(6));
                        (0..5).map(|_| r.u(8) as u8).collect()
                    } else {
                        r.u(5);
                        (0..6).map(|_| r.u(8) as u8).collect()
                    };
                    self.slogan.frame(None, f, b);
                    if let Some(t) = self.slogan.complete() {
                        s.slogan = from_latin1(&t);
                    }
                }
            }
            _ => {}
        }
    }
}

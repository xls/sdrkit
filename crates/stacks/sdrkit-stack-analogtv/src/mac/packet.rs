//! MAC packets: the 751-bit packet with its Golay header, the companded sound block, and the service identification data groups
//! with their commands and parameters.
//!
//! Standard: ETS 300 250 Part 1 clause 3.6, Parts 3 and 5.

use super::codes::{group_crc, header_code, packet_interleave, PACKET_BITS};
use crate::nicam::{compress, expand, parities, recover, shift_of_code};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_text::teletext::hamming::{decode_8_4, encode_8_4};

/// The dummy packet address.
pub const DUMMY: u16 = 1023;
/// The service identification address.
pub const SI_ADDRESS: u16 = 0;
/// Packet type of a first SI packet.
pub const SI_FIRST: u8 = 0xF8;
/// Packet type of a following SI packet, and of a sound packet.
pub const NEXT: u8 = 0xC7;

/// A packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    /// Address (10 bits).
    pub address: u16,
    /// Continuity index (2 bits).
    pub ci: u8,
    /// The 91 data bytes.
    pub data: [u8; 91],
}

impl Packet {
    /// A packet.
    pub fn new(address: u16, ci: u8, data: [u8; 91]) -> Self {
        Packet { address, ci, data }
    }
    /// The 751 transmitted bits (interleaved).
    pub fn bits(&self) -> [u8; PACKET_BITS] {
        let mut s = [0u8; PACKET_BITS];
        let info = (self.address & 0x3FF) | ((self.ci as u16 & 3) << 10);
        let h = header_code().encode(info as u128) as u32;
        for i in 0..23 {
            s[i] = ((h >> (22 - i)) & 1) as u8;
        }
        for (k, &b) in self.data.iter().enumerate() {
            for i in 0..8 {
                s[23 + 8 * k + i] = (b >> i) & 1;
            }
        }
        let mut out = [0u8; PACKET_BITS];
        packet_interleave()
            .apply(&s, &mut out)
            .expect("both are one packet long");
        out
    }
    /// From 751 received bits (more are ignored): the packet and the header bits corrected. The Golay code is perfect, so every
    /// header decodes. If `bits` is shorter than a packet, the missing bits read as zero.
    pub fn from_bits(bits: &[u8], code: &SmallCode) -> (Packet, u32) {
        let mut sent = [0u8; PACKET_BITS];
        let n = bits.len().min(PACKET_BITS);
        sent[..n].copy_from_slice(&bits[..n]);
        let mut s = [0u8; PACKET_BITS];
        packet_interleave()
            .invert(&sent, &mut s)
            .expect("both are one packet long");
        let mut h = 0u128;
        for &b in &s[..23] {
            h = (h << 1) | b as u128;
        }
        let (word, corr) = code.decode(h).unwrap_or((h, 0));
        let d = code.info(word) as u16;
        let corr = corr as u32;
        let mut data = [0u8; 91];
        for (k, byte) in data.iter_mut().enumerate() {
            for i in 0..8 {
                *byte |= s[23 + 8 * k + i] << i;
            }
        }
        (
            Packet {
                address: d & 0x3FF,
                ci: ((d >> 10) & 3) as u8,
                data,
            },
            corr,
        )
    }
}

/// A sound block: 64 samples of 14 bits companded to 10 bits with parity, the scale factor in the parity bits, 16 unallocated bits
/// set to one: 90 bytes after the packet type byte.
pub fn sound_block(samples: &[i32; 64]) -> [u8; 90] {
    let mut words = [0u16; 64];
    let code = compress(samples, &mut words);
    let par: Vec<u8> = parities(&words, code).collect();
    let mut bits = Vec::with_capacity(720);
    for k in 0..64 {
        for i in 0..10 {
            bits.push(((words[k] >> i) & 1) as u8);
        }
        bits.push(par[k]);
    }
    bits.extend(std::iter::repeat_n(1u8, 16));
    let mut out = [0u8; 90];
    for (k, b) in out.iter_mut().enumerate() {
        for i in 0..8 {
            *b |= bits[8 * k + i] << i;
        }
    }
    out
}

/// Decodes a sound block: the samples (a failed word repeats the one before) and the parity errors.
pub fn decode_sound(block: &[u8], prev: &mut i32, out: &mut [i32; 64]) -> u32 {
    let bit = |i: usize| (block[i / 8] >> (i % 8)) & 1;
    let mut words = [0u16; 64];
    let mut par = [0u8; 64];
    for k in 0..64 {
        let mut w = 0u16;
        for i in 0..10 {
            w |= (bit(11 * k + i) as u16) << i;
        }
        words[k] = w;
        par[k] = bit(11 * k + 10);
    }
    let (code, bad) = recover(&words, &par);
    let s = shift_of_code(code);
    for k in 0..64 {
        if bad & (1 << k) != 0 {
            out[k] = *prev;
        } else {
            out[k] = expand(words[k], s);
            *prev = out[k];
        }
    }
    bad.count_ones()
}

/// Command and parameter identifiers (Part 5).
pub mod ids {
    /// The network command.
    pub const CI_NETWORK: u8 = 0x10;
    /// The TV service command.
    pub const CI_TV: u8 = 0x90;
    /// Network origin: satellite channel (BCD), country text from byte 3.
    pub const NWO: u8 = 0x10;
    /// Network name.
    pub const NWNAME: u8 = 0x14;
    /// List of services: type, address.
    pub const LISTX: u8 = 0x18;
    /// The service name parameter (SREF): the name from byte 1.
    pub const SREF: u8 = 0x40;
    /// A parameter group.
    pub const PG: u8 = 0x80;
    /// Conditional access: ECM address (bit 15 set), CA system byte.
    pub const ACCM: u8 = 0x88;
    /// Video configuration (bits 3 to 5 = 6: 16:9).
    pub const VCONF: u8 = 0x90;
    /// The TV sound: its address.
    pub const TV_SOUND: u8 = 0xA4;
    /// The EuroCrypt CA system byte.
    pub const EUROCRYPT: u8 = 0x40;
}

/// A parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    /// Identifier.
    pub pi: u8,
    /// Data (for a group: the parameters inside, encoded).
    pub data: Vec<u8>,
}

/// A command with its parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// Identifier.
    pub ci: u8,
    /// Parameters.
    pub params: Vec<Param>,
}

/// Encodes parameters.
pub fn encode_params(params: &[Param]) -> Vec<u8> {
    let mut v = Vec::new();
    for p in params {
        v.push(p.pi);
        v.push(p.data.len() as u8);
        v.extend_from_slice(&p.data);
    }
    v
}

/// Decodes parameters.
pub fn decode_params(b: &[u8]) -> Vec<Param> {
    let mut v = Vec::new();
    let mut i = 0;
    while i + 2 <= b.len() {
        let (pi, li) = (b[i], b[i + 1] as usize);
        if i + 2 + li > b.len() {
            break;
        }
        v.push(Param {
            pi,
            data: b[i + 2..i + 2 + li].to_vec(),
        });
        i += 2 + li;
    }
    v
}

/// A data group: type, commands, with its Hamming-coded header and CRC.
pub fn encode_group(kind: u8, commands: &[Command]) -> Vec<u8> {
    let mut payload = Vec::new();
    for c in commands {
        let p = encode_params(&c.params);
        payload.push(c.ci);
        payload.push(p.len() as u8);
        payload.extend_from_slice(&p);
    }
    let len = payload.len();
    let mut g = vec![
        encode_8_4(kind),
        encode_8_4((len >> 4) as u8),
        encode_8_4(len as u8),
        encode_8_4((len >> 8) as u8),
    ];
    g.extend_from_slice(&payload);
    let crc = group_crc(&g);
    g.push((crc & 0xFF) as u8);
    g.push((crc >> 8) as u8);
    g
}

/// The total length of a data group from its header (None when the header is damaged).
pub fn group_len(header: &[u8]) -> Option<usize> {
    if header.len() < 4 {
        return None;
    }
    let nibble = |b: u8| decode_8_4(b).map(|d| d.data as usize);
    let hi = nibble(header[1])?;
    let lo = nibble(header[2])?;
    let top = nibble(header[3])?;
    Some(4 + ((top << 8) | (hi << 4) | lo) + 2)
}

/// Decodes a complete data group: (type, commands), or None if its CRC fails.
pub fn decode_group(g: &[u8]) -> Option<(u8, Vec<Command>)> {
    let n = group_len(g)?;
    if g.len() < n {
        return None;
    }
    let crc = group_crc(&g[..n - 2]);
    if g[n - 2] != (crc & 0xFF) as u8 || g[n - 1] != (crc >> 8) as u8 {
        return None;
    }
    let kind = decode_8_4(g[0])?.data;
    let mut cmds = Vec::new();
    let p = &g[4..n - 2];
    let mut i = 0;
    while i + 2 <= p.len() {
        let (ci, li) = (p[i], p[i + 1] as usize);
        if i + 2 + li > p.len() {
            break;
        }
        cmds.push(Command {
            ci,
            params: decode_params(&p[i + 2..i + 2 + li]),
        });
        i += 2 + li;
    }
    Some((kind, cmds))
}

/// Cuts a data group into SI packet payloads (91 bytes each: the type byte, then 88 bytes of the group, then two unused).
pub fn group_packets(g: &[u8]) -> Vec<[u8; 91]> {
    let mut out = Vec::new();
    for (k, chunk) in g.chunks(88).enumerate() {
        let mut d = [0u8; 91];
        d[0] = if k == 0 { SI_FIRST } else { NEXT };
        d[1..1 + chunk.len()].copy_from_slice(chunk);
        out.push(d);
    }
    out
}

/// Text from bytes (EBU Latin with the ASCII part kept).
pub fn text_of(b: &[u8]) -> String {
    b.iter()
        .filter(|&&c| (0x20..0x7F).contains(&c))
        .map(|&c| c as char)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packet_round_trips() {
        let mut d = [0u8; 91];
        for (i, v) in d.iter_mut().enumerate() {
            *v = (i * 7) as u8;
        }
        let p = Packet::new(345, 2, d);
        let mut bits = p.bits();
        bits[5] ^= 1;
        bits[100] ^= 1;
        let (q, corr) = Packet::from_bits(&bits, header_code());
        assert_eq!((q.address, q.ci), (345, 2));
        assert!(corr <= 1);
        let _ = q;
    }

    #[test]
    fn sound_blocks_and_groups_round_trip() {
        let mut x = [0i32; 64];
        for (k, v) in x.iter_mut().enumerate() {
            *v = ((k as f64 * 0.2).sin() * 4000.0) as i32;
        }
        let b = sound_block(&x);
        let mut prev = 0;
        let mut y = [0i32; 64];
        assert_eq!(decode_sound(&b, &mut prev, &mut y), 0);
        let mut se = 0.0;
        let mut ss = 0.0;
        for k in 0..64 {
            se += ((x[k] - y[k]) as f64).powi(2);
            ss += (x[k] as f64).powi(2);
        }
        assert!(10.0 * (ss / se.max(1e-9)).log10() > 45.0);
        let pg = encode_params(&[Param {
            pi: ids::ACCM,
            data: vec![0x5A, 0x81, ids::EUROCRYPT],
        }]);
        let g = encode_group(
            3,
            &[Command {
                ci: ids::CI_TV,
                params: vec![
                    Param {
                        pi: ids::SREF,
                        data: b"\0TEST".to_vec(),
                    },
                    Param {
                        pi: ids::PG,
                        data: pg,
                    },
                ],
            }],
        );
        let (kind, cmds) = decode_group(&g).unwrap();
        assert_eq!(kind, 3);
        assert_eq!(text_of(&cmds[0].params[0].data[1..]), "TEST");
        let inner = decode_params(&cmds[0].params[1].data);
        assert_eq!(inner[0].data[2], ids::EUROCRYPT);
        let mut bad = g.clone();
        bad[6] ^= 0x10;
        assert!(decode_group(&bad).is_none());
    }
}

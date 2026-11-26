//! Basic-rate packets after the access code (Bluetooth Core Specification v5.4 Vol 2 Part B clauses 6.4, 6.5 and 8.4.3):
//! the names of the packet types, the header read with a known UAP over the 64 whitening seeds, and the extended inquiry
//! response (EIR) packet that follows an FHS whose EIR bit is set. The EIR packet is a DM or DH packet whose HEC and CRC are
//! checked with UAP 0, as every inquiry response is; its payload is the EIR data, a list of AD structures.

use crate::baseband::{
    crc16_uap0, fec13_decode, fec13_encode, fec23_decode, fec23_encode, header_bits, header_fields,
    hec, Whitening, HEADER_AIR_BITS,
};

/// The packet types of the ACL and SCO logical transports at basic rate (clause 6.5, table 6.2), by TYPE code.
pub fn type_name(t: u8) -> &'static str {
    const NAMES: [&str; 16] = [
        "NULL", "POLL", "FHS", "DM1", "DH1", "HV1", "HV2", "HV3", "DV", "AUX1", "DM3", "DH3", "EV4", "EV5", "DM5", "DH5",
    ];
    NAMES[usize::from(t & 15)]
}

/// One reading of a packet header: the whitening seed (CLK1..CLK6) whose header passes the HEC, LT_ADDR and TYPE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderReading {
    /// The whitening seed.
    pub clk: u8,
    /// LT_ADDR.
    pub lt_addr: u8,
    /// TYPE.
    pub kind: u8,
}

/// The readings of the header in `bits` (the 54 bits after the access code) with the HEC preset from `uap`: every
/// whitening seed whose header passes. A seed passes by chance with probability 1/256, so a packet can give a false
/// reading next to the true one.
pub fn header_readings(bits: &[u8], uap: u8) -> Vec<HeaderReading> {
    if bits.len() < HEADER_AIR_BITS {
        return Vec::new();
    }
    let mut header = Vec::with_capacity(18);
    let split = fec13_decode(&bits[..HEADER_AIR_BITS], &mut header);
    if 4 * split > 18 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut h = [0u8; 18];
    for clk in 0..64u8 {
        h.copy_from_slice(&header);
        Whitening::new(clk).apply(&mut h);
        if hec(uap, &h[..10]) == h[10..18] {
            let (lt_addr, kind, ..) = header_fields(&h[..10]);
            out.push(HeaderReading { clk, lt_addr, kind });
        }
    }
    out
}

/// The bits of a packet header after the access code: LT_ADDR, TYPE, the flags clear, the HEC with `uap`, whitened with the
/// clock bits `clk` and coded with FEC 1/3. The payload bits that follow are whitened by the same sequence on: `payload`
/// is whitened and appended as it is (uncoded).
pub fn header_air_bits(lt_addr: u8, kind: u8, uap: u8, clk: u8, payload: &[u8], out: &mut Vec<u8>) {
    let mut plain = Vec::with_capacity(18 + payload.len());
    let h = header_bits(lt_addr, kind, false, false, false);
    plain.extend_from_slice(&h);
    plain.extend_from_slice(&hec(uap, &h));
    plain.extend_from_slice(payload);
    Whitening::new(clk).apply(&mut plain);
    fec13_encode(&plain[..18], out);
    out.extend_from_slice(&plain[18..]);
}

/// The TYPE codes of the packets that may carry an EIR (DM1, DH1, DM3, DH3, DM5, DH5).
const EIR_TYPES: [u8; 6] = [3, 4, 10, 11, 14, 15];

/// An extended inquiry response read from the air.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eir {
    /// The packet TYPE.
    pub kind: u8,
    /// The EIR data (AD structures).
    pub data: Vec<u8>,
}

/// The bytes of `bits` (least significant bit first), whole bytes only.
fn bytes(bits: &[u8]) -> Vec<u8> {
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|b| b.iter().enumerate().fold(0u8, |a, (i, &x)| a | (x & 1) << i))
        .collect()
}

/// Reads an EIR packet from the bits after an access code: the header by majority, then for each of the 64 whitening seeds
/// whose header passes the HEC with UAP 0 and has a DM or DH type, the payload (FEC 2/3 for DM), its payload header and
/// its CRC with UAP 0. `None` when no seed gives a good CRC.
pub fn read_eir(bits: &[u8]) -> Option<Eir> {
    if bits.len() < HEADER_AIR_BITS + 8 {
        return None;
    }
    let mut header = Vec::with_capacity(18);
    let split = fec13_decode(&bits[..HEADER_AIR_BITS], &mut header);
    if 4 * split > 18 {
        return None;
    }
    let after = &bits[HEADER_AIR_BITS..];
    let mut decoded_dm: Option<Vec<u8>> = None;
    let mut h = [0u8; 18];
    for clk in 0..64u8 {
        h.copy_from_slice(&header);
        let mut w = Whitening::new(clk);
        w.apply(&mut h);
        if hec(0, &h[..10]) != h[10..18] {
            continue;
        }
        let (_, kind, ..) = header_fields(&h[..10]);
        if !EIR_TYPES.contains(&kind) {
            continue;
        }
        let dm = matches!(kind, 3 | 10 | 14);
        let mut p = if dm {
            if decoded_dm.is_none() {
                let n = after.len() / 15 * 15;
                let mut out = Vec::with_capacity(n * 2 / 3);
                // A block with no single-error syndrome ends the payload that can be read.
                let mut at = 0;
                while at + 15 <= n {
                    if fec23_decode(&after[at..at + 15], &mut out).is_none() {
                        break;
                    }
                    at += 15;
                }
                decoded_dm = Some(out);
            }
            decoded_dm.clone().unwrap_or_default()
        } else {
            after.to_vec()
        };
        w.apply(&mut p);
        // The payload header: 8 bits for a one-slot packet, 16 for the others; LLID, FLOW, then the length.
        let single = matches!(kind, 3 | 4);
        let ph = if single { 8 } else { 16 };
        if p.len() < ph {
            continue;
        }
        let len_bits = if single { 5 } else { 10 };
        let length = p[3..3 + len_bits].iter().enumerate().fold(0usize, |a, (i, &b)| a | usize::from(b & 1) << i);
        let end = ph + 8 * length;
        if p.len() < end + 16 || crc16_uap0(&p[..end]) != p[end..end + 16] {
            continue;
        }
        return Some(Eir {
            kind,
            data: bytes(&p[ph..end]),
        });
    }
    None
}

/// The bits of an EIR packet after the access code for test signals: a DM1 packet (data up to 17 bytes) or a DH1 packet
/// (up to 27), header and CRC with UAP 0, whitened with `clk`. `data` longer than 27 bytes is cut.
pub fn eir_air_bits(data: &[u8], clk: u8, out: &mut Vec<u8>) {
    let data = &data[..data.len().min(27)];
    let dm1 = data.len() <= 17;
    let kind = if dm1 { 3 } else { 4 };
    let mut plain = Vec::with_capacity(18 + 8 * (data.len() + 3));
    let h = header_bits(0, kind, false, false, false);
    plain.extend_from_slice(&h);
    plain.extend_from_slice(&hec(0, &h));
    // LLID 2 (start), FLOW 1, LENGTH.
    let mut payload = vec![0, 1, 1];
    payload.extend((0..5).map(|i| (data.len() >> i & 1) as u8));
    for &b in data {
        payload.extend((0..8).map(|i| b >> i & 1));
    }
    let crc = crc16_uap0(&payload);
    payload.extend_from_slice(&crc);
    plain.extend_from_slice(&payload);
    Whitening::new(clk).apply(&mut plain);
    fec13_encode(&plain[..18], out);
    if dm1 {
        let mut p = plain[18..].to_vec();
        p.resize(p.len().div_ceil(10) * 10, 0);
        fec23_encode(&p, out);
    } else {
        out.extend_from_slice(&plain[18..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eir_and_headers_read_back() {
        for n in [12usize, 24] {
            let data: Vec<u8> = (0..n as u8).map(|i| i.wrapping_mul(37)).collect();
            let mut bits = Vec::new();
            eir_air_bits(&data, 0x2A, &mut bits);
            let e = read_eir(&bits).unwrap();
            assert_eq!((e.kind, e.data), (if n <= 17 { 3 } else { 4 }, data));
        }
        let mut bits = Vec::new();
        header_air_bits(1, 4, 0x7A, 0x11, &[1, 0, 1], &mut bits);
        let r = header_readings(&bits, 0x7A);
        assert!(r.contains(&HeaderReading {
            clk: 0x11,
            lt_addr: 1,
            kind: 4
        }));
        assert_eq!(type_name(4), "DH1");
    }
}

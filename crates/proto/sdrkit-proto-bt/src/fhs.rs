//! The FHS packet: the 144 information bits, the device address as text, and the whole packet after the access code (header
//! and payload, whitened and coded). Standard: Bluetooth Core Specification v5.4, Vol 2 Part B clause 6.5.1.4.

use crate::baseband::{
    crc16_uap0, fec13_decode, fec13_encode, fec23_decode, fec23_encode, header_bits, header_fields,
    hec, Whitening, HEADER_AIR_BITS, TYPE_FHS,
};

/// Information bits of an FHS payload.
pub const INFO_BITS: usize = 144;
/// Bits of an FHS payload on the air: 16 blocks of FEC 2/3 over the information bits and the CRC.
pub const PAYLOAD_AIR_BITS: usize = 240;
/// Bits of an FHS packet after the access code.
pub const PACKET_AIR_BITS: usize = HEADER_AIR_BITS + PAYLOAD_AIR_BITS;

/// The fields of an FHS payload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fhs {
    /// The 34 parity bits of the sender's sync word (bit i = s_i).
    pub parity: u64,
    /// The lower address part.
    pub lap: u32,
    /// Extended inquiry response follows.
    pub eir: bool,
    /// The page scan repetition mode.
    pub sr: u8,
    /// The upper address part.
    pub uap: u8,
    /// The non-significant address part.
    pub nap: u16,
    /// The class of device (24 bits).
    pub class: u32,
    /// The logical transport address.
    pub lt_addr: u8,
    /// CLK27..CLK2 of the sender, units of 1.25 ms.
    pub clock: u32,
    /// The page scan mode.
    pub page_scan_mode: u8,
}

/// The fields of an FHS payload, each least significant bit first: (first bit, width).
const LAYOUT: [(usize, usize); 10] = [
    (0, 34),
    (34, 24),
    (58, 1),
    (60, 2),
    (64, 8),
    (72, 16),
    (88, 24),
    (112, 3),
    (115, 26),
    (141, 3),
];

impl Fhs {
    /// The fields of 144 information bits in air order.
    pub fn from_bits(b: &[u8]) -> Option<Fhs> {
        if b.len() < INFO_BITS {
            return None;
        }
        let v = |k: usize| {
            let (at, n) = LAYOUT[k];
            (0..n).fold(0u64, |a, i| a | u64::from(b[at + i] & 1) << i)
        };
        Some(Fhs {
            parity: v(0),
            lap: v(1) as u32,
            eir: v(2) == 1,
            sr: v(3) as u8,
            uap: v(4) as u8,
            nap: v(5) as u16,
            class: v(6) as u32,
            lt_addr: v(7) as u8,
            clock: v(8) as u32,
            page_scan_mode: v(9) as u8,
        })
    }

    /// The 144 information bits in air order (the reserved bits are 0).
    pub fn to_bits(&self) -> [u8; INFO_BITS] {
        let vals = [
            self.parity,
            u64::from(self.lap),
            u64::from(self.eir),
            u64::from(self.sr),
            u64::from(self.uap),
            u64::from(self.nap),
            u64::from(self.class),
            u64::from(self.lt_addr),
            u64::from(self.clock),
            u64::from(self.page_scan_mode),
        ];
        let mut b = [0u8; INFO_BITS];
        for (&(at, n), v) in LAYOUT.iter().zip(vals) {
            for i in 0..n {
                b[at + i] = (v >> i & 1) as u8;
            }
        }
        b
    }

    /// The device address as text: NAP high and low byte, UAP, LAP high, middle and low byte ("12:34:5E:A1:B2:C3").
    pub fn address_text(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.nap >> 8,
            self.nap & 0xFF,
            self.uap,
            self.lap >> 16 & 0xFF,
            self.lap >> 8 & 0xFF,
            self.lap & 0xFF
        )
    }
}

/// The bits of an FHS packet after the access code: header (LT_ADDR 0, FHS, HEC with UAP 0) and payload with its CRC, whitened
/// with the clock bits `clk` (CLK1..CLK6), then coded with FEC 1/3 and 2/3.
pub fn packet_bits(fhs: &Fhs, clk: u8, out: &mut Vec<u8>) {
    let mut plain = Vec::with_capacity(18 + INFO_BITS + 16);
    let h = header_bits(0, TYPE_FHS, false, false, false);
    plain.extend_from_slice(&h);
    plain.extend_from_slice(&hec(0, &h));
    let info = fhs.to_bits();
    plain.extend_from_slice(&info);
    plain.extend_from_slice(&crc16_uap0(&info));
    Whitening::new(clk).apply(&mut plain);
    fec13_encode(&plain[..18], out);
    fec23_encode(&plain[18..], out);
}

/// Why the bits after an access code are not an FHS packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FhsError {
    /// Fewer bits than an FHS packet.
    Short,
    /// More than a quarter of the 18 header triples disagree.
    Header,
    /// A payload block has more than one bit error.
    Payload,
    /// No whitening seed gives a good HEC, the FHS type and a good CRC.
    Check,
}

/// An FHS packet read from the air.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FhsPacket {
    /// The payload fields.
    pub fhs: Fhs,
    /// The whitening seed (CLK1..CLK6) that checks.
    pub clk: u8,
    /// Header triples that disagreed.
    pub header_split: usize,
    /// Payload blocks corrected.
    pub corrected: usize,
}

/// Reads an FHS packet from the bits after an access code: the header by majority, the payload by FEC 2/3, then the first of
/// the 64 whitening seeds whose header has a good HEC (UAP 0) and the FHS type and whose payload has a good CRC.
///
/// # Errors
///
/// [`FhsError`] for the first test the bits fail.
pub fn read_packet(bits: &[u8]) -> Result<FhsPacket, FhsError> {
    if bits.len() < PACKET_AIR_BITS {
        return Err(FhsError::Short);
    }
    let mut header = Vec::with_capacity(18);
    let split = fec13_decode(&bits[..HEADER_AIR_BITS], &mut header);
    if 4 * split > 18 {
        return Err(FhsError::Header);
    }
    let mut payload = Vec::with_capacity(160);
    let corrected = fec23_decode(&bits[HEADER_AIR_BITS..PACKET_AIR_BITS], &mut payload)
        .ok_or(FhsError::Payload)?;
    let mut h = [0u8; 18];
    let mut p = [0u8; 160];
    for clk in 0..64u8 {
        h.copy_from_slice(&header);
        let mut w = Whitening::new(clk);
        w.apply(&mut h);
        if hec(0, &h[..10]) != h[10..18] || header_fields(&h[..10]).1 != TYPE_FHS {
            continue;
        }
        p.copy_from_slice(&payload);
        w.apply(&mut p);
        if crc16_uap0(&p[..INFO_BITS]) != p[INFO_BITS..] {
            continue;
        }
        let Some(fhs) = Fhs::from_bits(&p) else {
            continue;
        };
        return Ok(FhsPacket {
            fhs,
            clk,
            header_split: split,
            corrected,
        });
    }
    Err(FhsError::Check)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(k: u64) -> Fhs {
        let r = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x1234_5678_9ABC_DEF0;
        Fhs {
            parity: r & ((1 << 34) - 1),
            lap: (r >> 7) as u32 & 0xFF_FFFF,
            eir: r >> 3 & 1 == 1,
            sr: (r >> 11) as u8 & 3,
            uap: (r >> 17) as u8,
            nap: (r >> 29) as u16,
            class: (r >> 37) as u32 & 0xFF_FFFF,
            lt_addr: (r >> 41) as u8 & 7,
            clock: (r >> 33) as u32 & ((1 << 26) - 1),
            page_scan_mode: (r >> 61) as u8 & 7,
        }
    }

    #[test]
    fn random_packets_with_every_seed_come_back() {
        for k in 0..512u64 {
            let f = sample(k);
            let clk = (k % 64) as u8;
            let mut b = Vec::new();
            packet_bits(&f, clk, &mut b);
            assert_eq!(b.len(), PACKET_AIR_BITS);
            let p = read_packet(&b).unwrap();
            assert_eq!(p.fhs, f);
            assert_eq!(p.clk, clk);
        }
    }

    #[test]
    fn damaged_packets_never_give_other_values() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut good = 0;
        for k in 0..4000u64 {
            let f = sample(k + 1000);
            let mut b = Vec::new();
            packet_bits(&f, (k % 64) as u8, &mut b);
            let errors = 1 + next() % 12;
            for _ in 0..errors {
                let i = (next() % b.len() as u64) as usize;
                b[i] ^= 1;
            }
            if let Ok(p) = read_packet(&b) {
                assert_eq!(p.fhs, f, "packet {k}");
                good += 1;
            }
        }
        assert!(good > 500, "{good}");
    }

    #[test]
    fn the_address_text() {
        let f = Fhs {
            lap: 0xA1B2C3,
            uap: 0x5E,
            nap: 0x1234,
            ..Fhs::default()
        };
        assert_eq!(f.address_text(), "12:34:5E:A1:B2:C3");
        assert_eq!(Fhs::from_bits(&f.to_bits()), Some(f));
    }
}

//! ExpressLRS frames on the 900 MHz and 433 MHz LoRa rates: the link id from the binding phrase, the two frame sizes with
//! their CRCs, the sync frame and the four stick values of a radio-control frame.
//! No public document gives the frame layout, the CRC start value or the rate rows. The hash of the phrase is MD5 (RFC 1321).

use sdrkit_util_bits::crc::CrcSpec;

/// The LoRa sync word.
pub const SYNC: u8 = 0x12;
/// The bandwidth of every rate row, Hz.
pub const BW_HZ: f64 = 500e3;
/// The bytes of the short frame.
pub const SHORT: usize = 8;
/// The bytes of the long frame.
pub const LONG: usize = 13;
/// The protocol version, which is part of the CRC start value.
pub const VERSION: u16 = 4;

/// The CRC of the short frame: 14 bits, polynomial 0x2E57. The start value is the one of the link.
pub const CRC14: CrcSpec = CrcSpec::new("CRC-14/ELRS", 14, 0x2E57, 0, false, false, 0, 0x1A4D);
/// The CRC of the long frame: 16 bits, polynomial 0x3D65. The start value is the one of the link.
pub const CRC16: CrcSpec = CrcSpec::new("CRC-16/ELRS", 16, 0x3D65, 0, false, false, 0, 0x3D48);

/// One LoRa setting that a link can use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateRow {
    /// The spreading factor.
    pub sf: u8,
    /// The coding rate, 5 to 8.
    pub cr: u8,
    /// The preamble symbols.
    pub preamble: u16,
    /// The frame bytes.
    pub bytes: u8,
    /// The time between two frames of the test signal, seconds.
    pub interval_s: f64,
    /// The name of the rate.
    pub name: &'static str,
}

/// The rate rows, each at 500 kHz with an implicit header and no LoRa CRC.
pub const ROWS: [RateRow; 5] = [
    RateRow {
        sf: 6,
        cr: 7,
        preamble: 8,
        bytes: 8,
        interval_s: 0.005,
        name: "200 Hz",
    },
    RateRow {
        sf: 7,
        cr: 7,
        preamble: 8,
        bytes: 8,
        interval_s: 0.010,
        name: "100 Hz",
    },
    RateRow {
        sf: 8,
        cr: 7,
        preamble: 10,
        bytes: 8,
        interval_s: 0.020,
        name: "50 Hz",
    },
    RateRow {
        sf: 9,
        cr: 7,
        preamble: 10,
        bytes: 8,
        interval_s: 0.040,
        name: "25 Hz",
    },
    RateRow {
        sf: 6,
        cr: 8,
        preamble: 8,
        bytes: 13,
        interval_s: 0.010,
        name: "100 Hz Full",
    },
];

/// The MD5 hash of `data` (RFC 1321).
pub fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
    let mut state: [u32; 4] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_le_bytes());
    for part in msg.as_chunks::<64>().0 {
        let mut m = [0u32; 16];
        for (w, b) in m.iter_mut().zip(part.as_chunks::<4>().0) {
            *w = u32::from_le_bytes(*b);
        }
        let [mut a, mut b, mut c, mut d] = state;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            // The constant of step i is the whole part of 2^32 |sin(i + 1)|.
            let k = ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32;
            let sum = a.wrapping_add(f).wrapping_add(k).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(sum.rotate_left(S[(i / 16) * 4 + i % 4]));
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d]) {
            *s = s.wrapping_add(v);
        }
    }
    let mut out = [0u8; 16];
    for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        *o = s.to_le_bytes();
    }
    out
}

/// The two bytes of a link id that a sync frame carries and that the CRC start value comes from.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct LinkId(pub u16);

impl std::fmt::Display for LinkId {
    /// Four hex digits, upper case.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04X}", self.0)
    }
}

impl LinkId {
    /// The CRC start value of a sync frame of this link.
    pub fn sync_start(self) -> u16 {
        self.0 ^ (VERSION << 8)
    }
}

/// The six bytes of the link id of a binding phrase.
pub fn uid(phrase: &str) -> [u8; 6] {
    let text = format!("-DMY_BINDING_PHRASE=\"{phrase}\"");
    let h = md5(text.as_bytes());
    [h[0], h[1], h[2], h[3], h[4], h[5]]
}

/// The link of a binding phrase.
pub fn link_of(phrase: &str) -> LinkId {
    let u = uid(phrase);
    LinkId(u16::from_be_bytes([u[4], u[5]]))
}

/// The CRC that the frame `f` must carry for the start value `start`. `None` for a frame of another size than 8 or 13 bytes.
pub fn crc_for(f: &[u8], start: u16) -> Option<u16> {
    match f.len() {
        SHORT => {
            let spec = CrcSpec {
                init: u64::from(start & 0x3FFF),
                ..CRC14
            };
            let mut d = [0u8; 7];
            d.copy_from_slice(&f[..7]);
            d[0] &= 0x03;
            Some(spec.compute(&d) as u16)
        }
        LONG => {
            let spec = CrcSpec {
                init: u64::from(start),
                ..CRC16
            };
            Some(spec.compute(&f[..11]) as u16)
        }
        _ => None,
    }
}

/// The CRC that the frame `f` carries.
pub fn crc_of(f: &[u8]) -> Option<u16> {
    match f.len() {
        SHORT => Some(u16::from(f[0] >> 2) << 8 | u16::from(f[7])),
        LONG => Some(u16::from_le_bytes([f[11], f[12]])),
        _ => None,
    }
}

/// The type of the frame: 0 radio-control data or link statistics, 1 data, 2 sync, 3 other.
pub fn frame_type(f: &[u8]) -> u8 {
    f.first().map_or(3, |b| b & 3)
}

/// The content of a sync frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sync {
    /// The link.
    pub link: LinkId,
    /// Where the link is in its hop sequence.
    pub hop: u8,
    /// The nonce.
    pub nonce: u8,
    /// The byte shown as the rate index.
    pub rate_index: u8,
}

/// Reads a sync frame with a good CRC. `None` for another type, another size or a wrong CRC.
pub fn parse_sync(f: &[u8]) -> Option<Sync> {
    if !matches!(f.len(), SHORT | LONG) || frame_type(f) != 2 {
        return None;
    }
    let link = LinkId(u16::from_be_bytes([f[5], f[6]]));
    (crc_for(f, link.sync_start()) == crc_of(f)).then_some(Sync {
        link,
        hop: f[1],
        nonce: f[2],
        rate_index: f[3],
    })
}

/// The first number n, 0 to 255, for which the CRC of the frame `f` fits the link.
pub fn nonce_of(f: &[u8], link: LinkId) -> Option<u8> {
    let have = crc_of(f)?;
    (0..=255u8).find(|&n| crc_for(f, link.sync_start() ^ u16::from(n)) == Some(have))
}

/// The four stick values (each 0 to 1023) and the armed flag of a radio-control frame.
pub fn sticks(f: &[u8]) -> Option<([u16; 4], bool)> {
    let armed = match f.len() {
        SHORT => f[6] & 0x80 != 0,
        LONG => f[0] & 0x80 != 0,
        _ => return None,
    };
    let v = f[1..6]
        .iter()
        .rev()
        .fold(0u64, |v, &b| v << 8 | u64::from(b));
    Some((
        std::array::from_fn(|i| ((v >> (10 * i)) & 0x3FF) as u16),
        armed,
    ))
}

/// Writes the CRC for `start` into the frame `f` of 8 or 13 bytes.
fn seal(f: &mut [u8], start: u16) {
    if f.len() == SHORT {
        f[0] &= 0x03;
    }
    let Some(crc) = crc_for(f, start) else {
        return;
    };
    if f.len() == SHORT {
        f[0] |= ((crc >> 8) as u8) << 2;
        f[7] = crc as u8;
    } else {
        f[11..13].copy_from_slice(&crc.to_le_bytes());
    }
}

/// A sync frame of `len` bytes (8 or 13) of the link with the id bytes `uid`.
pub fn build_sync(uid: &[u8; 6], hop: u8, nonce: u8, rate_index: u8, len: usize) -> Vec<u8> {
    let mut f = vec![0u8; if len == LONG { LONG } else { SHORT }];
    f[0] = 2;
    f[1..7].copy_from_slice(&[hop, nonce, rate_index, uid[3], uid[4], uid[5]]);
    let link = LinkId(u16::from_be_bytes([uid[4], uid[5]]));
    seal(&mut f, link.sync_start());
    f
}

/// A radio-control frame of `len` bytes (8 or 13) with the CRC start value of the number `n`. The long frame carries `rest`
/// in its bytes 6 to 10.
pub fn build_rc(
    link: LinkId,
    n: u8,
    sticks: [u16; 4],
    armed: bool,
    len: usize,
    rest: [u8; 5],
) -> Vec<u8> {
    let long = len == LONG;
    let mut f = vec![0u8; if long { LONG } else { SHORT }];
    let v = sticks
        .iter()
        .enumerate()
        .fold(0u64, |v, (i, &s)| v | u64::from(s & 0x3FF) << (10 * i));
    f[1..6].copy_from_slice(&v.to_le_bytes()[..5]);
    if long {
        f[0] = u8::from(armed) << 7;
        f[6..11].copy_from_slice(&rest);
    } else {
        f[6] = u8::from(armed) << 7;
    }
    seal(&mut f, link.sync_start() ^ u16::from(n));
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNC8: [u8; 8] = [0x0E, 0x11, 0x2A, 0x03, 0xF5, 0xB1, 0xED, 0x6F];
    const RC8: [u8; 8] = [0xB4, 0x64, 0x00, 0xF8, 0x3F, 0x4B, 0x80, 0x43];
    const RC13: [u8; 13] = [
        0x80, 0x64, 0x00, 0xF8, 0x3F, 0x4B, 0x01, 0x02, 0x03, 0x04, 0x05, 0x65, 0x31,
    ];
    const SYNC13: [u8; 13] = [
        0x02, 0x05, 0x2A, 0x03, 0xF5, 0xB1, 0xED, 0x00, 0x00, 0x00, 0x00, 0xFA, 0x45,
    ];

    #[test]
    fn md5_gives_the_values_of_rfc_1321() {
        let hex = |d: [u8; 16]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex(md5(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            )),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn the_link_id_comes_from_the_phrase() {
        assert_eq!(uid("sdrkit test"), [0xC1, 0x0E, 0x2E, 0xF5, 0xB1, 0xED]);
        let link = link_of("sdrkit test");
        assert_eq!(link.to_string(), "B1ED");
        assert_eq!(link.sync_start(), 0xB5ED);
        assert_eq!(link.sync_start() & 0x3FFF, 0x35ED);
    }

    #[test]
    fn the_crcs_have_their_check_values() {
        assert_eq!(CRC14.compute(b"123456789"), 0x1A4D);
        assert_eq!(CRC16.compute(b"123456789"), 0x3D48);
    }

    #[test]
    fn the_sync_frames_parse_and_one_wrong_bit_is_refused() {
        assert_eq!(crc_of(&SYNC8), Some(0x036F));
        let want = Sync {
            link: LinkId(0xB1ED),
            hop: 17,
            nonce: 42,
            rate_index: 3,
        };
        assert_eq!(parse_sync(&SYNC8), Some(want));
        let mut bad = SYNC8;
        bad[3] = 0x02;
        assert_eq!(parse_sync(&bad), None);
        assert_eq!(parse_sync(&SYNC13), Some(Sync { hop: 5, ..want }));
        for byte in 0..8 {
            for bit in 0..8 {
                let mut f = SYNC8;
                f[byte] ^= 1 << bit;
                // A change of the type bits makes another frame type.
                assert_eq!(parse_sync(&f), None, "byte {byte} bit {bit}");
            }
        }
    }

    #[test]
    fn the_radio_control_frames_fit_one_number_only() {
        let link = LinkId(0xB1ED);
        assert_eq!(frame_type(&RC8), 0);
        assert_eq!(crc_of(&RC8), Some(0x2D43));
        let fits: Vec<u8> = (0..=255u8)
            .filter(|&n| crc_for(&RC8, link.sync_start() ^ u16::from(n)) == crc_of(&RC8))
            .collect();
        assert_eq!(fits, [77]);
        assert_eq!(sticks(&RC8), Some(([100, 512, 1023, 300], true)));
        assert_eq!(crc_of(&RC13), Some(0x3165));
        assert_eq!(nonce_of(&RC13, link), Some(9));
        assert_eq!(sticks(&RC13), Some(([100, 512, 1023, 300], true)));
        assert_eq!(nonce_of(&RC8, LinkId(0x0102)), None);
    }

    #[test]
    fn the_builders_make_the_worked_frames() {
        let u = uid("sdrkit test");
        assert_eq!(build_sync(&u, 17, 42, 3, 8), SYNC8);
        assert_eq!(build_sync(&u, 5, 42, 3, 13), SYNC13);
        let link = link_of("sdrkit test");
        let s = [100, 512, 1023, 300];
        assert_eq!(build_rc(link, 77, s, true, 8, [0; 5]), RC8);
        assert_eq!(build_rc(link, 9, s, true, 13, [1, 2, 3, 4, 5]), RC13);
    }
}

//! Eurobalise telegrams (UNISIG SUBSET-036 4.3): the 10-to-11-bit substitution, the scrambler, the cyclic code,
//! the encoder, the window check and the streaming receiver, and the SUBSET-026 header.

use serde::{Deserialize, Serialize};

/// Telegram formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// 1023 bits, 830 user bits.
    Long,
    /// 341 bits, 210 user bits.
    Short,
}

impl Format {
    /// Telegram length n.
    pub fn n(self) -> usize {
        match self {
            Format::Long => 1023,
            Format::Short => 341,
        }
    }
    /// User bits.
    pub fn user_bits(self) -> usize {
        match self {
            Format::Long => 830,
            Format::Short => 210,
        }
    }
    /// Repeated bits r of the receive window.
    pub fn r(self) -> usize {
        match self {
            Format::Long => 77,
            Format::Short => 121,
        }
    }
    fn f(self) -> u128 {
        let e: &[u32] = match self {
            Format::Long => &[10, 9, 7, 6, 4, 3, 2, 1, 0],
            Format::Short => &[10, 8, 7, 5, 3, 1, 0],
        };
        e.iter().fold(0u128, |a, &k| a | 1u128 << k)
    }
    fn g(self) -> u128 {
        let e: &[u32] = match self {
            Format::Long => &[
                75, 73, 72, 71, 67, 62, 61, 60, 57, 56, 55, 52, 51, 49, 46, 45, 44, 43, 41, 37, 35,
                34, 33, 31, 30, 28, 26, 24, 21, 17, 16, 15, 13, 12, 11, 9, 4, 1, 0,
            ],
            Format::Short => &[
                75, 72, 71, 70, 69, 68, 66, 65, 64, 63, 60, 55, 54, 49, 47, 46, 45, 44, 43, 42, 41,
                39, 38, 37, 36, 34, 33, 32, 31, 30, 27, 25, 22, 19, 17, 13, 12, 11, 10, 6, 3, 1, 0,
            ],
        };
        e.iter().fold(0u128, |a, &k| a | 1u128 << k)
    }
}

fn degree(p: u128) -> u32 {
    127 - p.leading_zeros()
}

/// The remainder of the polynomial of `bits` (first bit the highest power) modulo `p`.
pub fn poly_mod(bits: impl IntoIterator<Item = u8>, p: u128) -> u128 {
    let d = degree(p);
    let top = 1u128 << d;
    let mut r = 0u128;
    for b in bits {
        r = (r << 1) | (b as u128 & 1);
        if r & top != 0 {
            r ^= p;
        }
    }
    r
}

fn clmul(a: u128, b: u128) -> u128 {
    let mut r = 0u128;
    for i in 0..128 {
        if (b >> i) & 1 == 1 {
            r ^= a << i;
        }
    }
    r
}

/// x^k mod p.
fn xpow_mod(k: usize, p: u128) -> u128 {
    poly_mod(std::iter::once(1u8).chain(std::iter::repeat_n(0u8, k)), p)
}

/// The 11-bit word of a 10-bit value.
pub fn word_of(v: u16) -> u16 {
    BALISE_WORDS[v as usize & 1023]
}

/// The 10-bit value of an 11-bit word, or none for an invalid word.
pub fn value_of(w: u16) -> Option<u16> {
    BALISE_WORDS.binary_search(&w).ok().map(|i| i as u16)
}

const H: u32 = (1 << 31) | (1 << 30) | (1 << 29) | (1 << 27) | (1 << 25) | 1;

fn scramble(bits: &mut [u8], b: u16, descramble: bool) {
    let mut s = 2_801_775_573u32.wrapping_mul(b as u32);
    for x in bits.iter_mut() {
        let inp = *x;
        let out = ((s >> 31) as u8) ^ inp;
        let fb = if descramble { inp } else { out };
        s = (s << 1) ^ if fb == 1 { H } else { 0 };
        *x = out;
    }
}

fn blocks_of(bits: &[u8]) -> Vec<u16> {
    bits.chunks(10)
        .map(|c| c.iter().fold(0u16, |a, &b| (a << 1) | b as u16))
        .collect()
}

fn bits_of_blocks(blocks: &[u16]) -> Vec<u8> {
    let mut v = Vec::with_capacity(blocks.len() * 10);
    for &b in blocks {
        for i in (0..10).rev() {
            v.push(((b >> i) & 1) as u8);
        }
    }
    v
}

/// Encodes `user` bits (830 or 210) with scrambling value `b`; none when no extra shaping bits make every word valid.
/// The telegram's bits are in the order sent (b(n-1) first).
pub fn encode(format: Format, user: &[u8], b: u16) -> Option<Vec<u8>> {
    assert_eq!(user.len(), format.user_bits());
    let n = format.n();
    let mut blocks = blocks_of(user);
    let sum: u32 = blocks.iter().map(|&x| x as u32).sum();
    blocks[0] = (sum % 1024) as u16;
    let mut bits = bits_of_blocks(&blocks);
    scramble(&mut bits, b, false);
    let mut t = Vec::with_capacity(n);
    for v in blocks_of(&bits) {
        for i in (0..11).rev() {
            t.push(((word_of(v) >> i) & 1) as u8);
        }
    }
    // Control bits 0 0 1, then B (12 bits).
    t.extend_from_slice(&[0, 0, 1]);
    for i in (0..12).rev() {
        t.push(((b >> i) & 1) as u8);
    }
    let base = t.len();
    let fg = clmul(format.f(), format.g());
    let g = format.g();
    for esb in 0..1024u16 {
        t.truncate(base);
        for i in (0..10).rev() {
            t.push(((esb >> i) & 1) as u8);
        }
        let rem = poly_mod(t.iter().copied().chain(std::iter::repeat_n(0u8, 85)), fg) ^ g;
        for i in (0..85).rev() {
            t.push(((rem >> i) & 1) as u8);
        }
        debug_assert_eq!(t.len(), n);
        if t.chunks(11)
            .all(|w| value_of(w.iter().fold(0u16, |a, &x| (a << 1) | x as u16)).is_some())
        {
            return Some(t);
        }
    }
    None
}

/// Encodes with the first scrambling value B = 1, 2, ... that works.
pub fn encode_first(format: Format, user: &[u8]) -> Option<(Vec<u8>, u16)> {
    (1..4096u16).find_map(|b| encode(format, user, b).map(|t| (t, b)))
}

/// A decoded telegram.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The format.
    pub format: Format,
    /// The bits were inverted.
    pub inverted: bool,
    /// The user bits.
    pub user: Vec<u8>,
    /// The scrambling value.
    pub b: u16,
}

/// Checks a receive window of n + r bits and decodes it.
pub fn decode_window(format: Format, w: &[u8]) -> Option<Decoded> {
    let n = format.n();
    let r = w.len().checked_sub(n)?;
    if poly_mod(w[..n].iter().copied(), format.g()) != 0 {
        return None;
    }
    if w[n..n + r] != w[..r] {
        return None;
    }
    let rem = poly_mod(w[..n].iter().copied(), format.f());
    let f = format.f();
    let mut gx = format.g() % (1u128 << 127);
    gx = poly_mod((0..=75).rev().map(|i| ((gx >> i) & 1) as u8), f);
    let mut s = None;
    let mut cur = gx;
    for k in 0..n {
        if cur == rem {
            s = Some(k);
            break;
        }
        // cur = cur * x mod f
        cur <<= 1;
        if cur & (1 << 10) != 0 {
            cur ^= f;
        }
    }
    let s = s?;
    let start = (n - s) % n;
    let mut t: Vec<u8> = (0..n).map(|j| w[(start + j) % n]).collect();
    let words: Vec<u16> = t
        .chunks(11)
        .map(|c| c.iter().fold(0u16, |a, &x| (a << 1) | x as u16))
        .collect();
    if words.iter().any(|&x| value_of(x).is_none()) {
        return None;
    }
    let at = |t: &[u8], b: usize| t[n - 1 - b];
    let inverted = at(&t, 109) == 1;
    if inverted {
        for x in t.iter_mut() {
            *x ^= 1;
        }
    }
    if at(&t, 108) != 0 || at(&t, 107) != 1 {
        return None;
    }
    let b = (0..12).fold(0u16, |a, i| (a << 1) | at(&t, 106 - i) as u16);
    let uw = format.user_bits() / 10;
    let blocks: Vec<u16> = t[..uw * 11]
        .chunks(11)
        .map(|c| value_of(c.iter().fold(0u16, |a, &x| (a << 1) | x as u16)).unwrap_or(0))
        .collect();
    let mut bits = bits_of_blocks(&blocks);
    scramble(&mut bits, b, true);
    let mut ub = blocks_of(&bits);
    let others: u32 = ub[1..].iter().map(|&x| x as u32).sum();
    ub[0] = ((ub[0] as u32 + 1024 * 64 - others % 1024) % 1024) as u16;
    Some(Decoded {
        format,
        inverted,
        user: bits_of_blocks(&ub),
        b,
    })
}

/// The SUBSET-026 header of a telegram.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// Q_UPDOWN.
    pub q_updown: u8,
    /// M_VERSION.
    pub m_version: u8,
    /// Q_MEDIA.
    pub q_media: u8,
    /// N_PIG.
    pub n_pig: u8,
    /// N_TOTAL.
    pub n_total: u8,
    /// M_DUP.
    pub m_dup: u8,
    /// M_MCOUNT.
    pub m_mcount: u8,
    /// NID_C.
    pub nid_c: u16,
    /// NID_BG.
    pub nid_bg: u16,
    /// Q_LINK.
    pub q_link: u8,
    /// The first NID_PACKET (255: end of information).
    pub first_packet: u8,
}

/// Reads the header from the user bits (at least 58).
pub fn header(user: &[u8]) -> Option<Header> {
    if user.len() < 58 {
        return None;
    }
    let mut p = 0;
    let mut take = |n: usize| {
        let v = user[p..p + n]
            .iter()
            .fold(0u32, |a, &b| (a << 1) | b as u32);
        p += n;
        v
    };
    Some(Header {
        q_updown: take(1) as u8,
        m_version: take(7) as u8,
        q_media: take(1) as u8,
        n_pig: take(3) as u8,
        n_total: take(3) as u8,
        m_dup: take(2) as u8,
        m_mcount: take(8) as u8,
        nid_c: take(10) as u16,
        nid_bg: take(14) as u16,
        q_link: take(1) as u8,
        first_packet: take(8) as u8,
    })
}

/// Writes a header into the first 58 user bits.
pub fn write_header(h: &Header, user: &mut [u8]) {
    let fields: [(u32, usize); 11] = [
        (h.q_updown as u32, 1),
        (h.m_version as u32, 7),
        (h.q_media as u32, 1),
        (h.n_pig as u32, 3),
        (h.n_total as u32, 3),
        (h.m_dup as u32, 2),
        (h.m_mcount as u32, 8),
        (h.nid_c as u32, 10),
        (h.nid_bg as u32, 14),
        (h.q_link as u32, 1),
        (h.first_packet as u32, 8),
    ];
    let mut p = 0;
    for (v, n) in fields {
        for i in (0..n).rev() {
            user[p] = ((v >> i) & 1) as u8;
            p += 1;
        }
    }
}

/// "major.minor" of M_VERSION.
pub fn version_text(m: u8) -> String {
    format!("{}.{}", (m >> 4) & 7, m & 15)
}

/// The user bits as hex, padded with zeros to a whole byte.
pub fn hex(bits: &[u8]) -> String {
    let mut bytes = Vec::new();
    sdrkit_util_bits::pack::pack(
        bits,
        sdrkit_util_bits::BitOrder::Msb,
        sdrkit_util_bits::Pad::Zero,
        &mut bytes,
    );
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// The 32-bit Mersenne Twister (MT19937) with its standard seeding.
pub struct Mt19937 {
    mt: [u32; 624],
    i: usize,
}

impl Mt19937 {
    /// Seeded with `seed`.
    pub fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Mt19937 { mt, i: 624 }
    }
    /// The next output.
    pub fn next_u32(&mut self) -> u32 {
        if self.i >= 624 {
            for k in 0..624 {
                let y = (self.mt[k] & 0x8000_0000) | (self.mt[(k + 1) % 624] & 0x7FFF_FFFF);
                let mut v = self.mt[(k + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908_B0DF;
                }
                self.mt[k] = v;
            }
            self.i = 0;
        }
        let mut y = self.mt[self.i];
        self.i += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^ (y >> 18)
    }
}

/// The user bits of a test telegram of balise `p` in group `g`.
pub fn test_user_bits(format: Format, g: u16, p: u8) -> Vec<u8> {
    let mut user = vec![0u8; format.user_bits()];
    let h = Header {
        q_updown: 1,
        m_version: 33,
        q_media: 0,
        n_pig: p,
        n_total: 2,
        m_dup: 0,
        m_mcount: 255,
        nid_c: 466,
        nid_bg: g,
        q_link: 1,
        first_packet: 255,
    };
    write_header(&h, &mut user);
    let mut mt = Mt19937::new(31 * g as u32 + p as u32);
    for u in user.iter_mut().skip(58) {
        *u = (mt.next_u32() & 1) as u8;
    }
    user
}

/// A streaming receiver for both formats over one burst's bits.
pub struct StreamReceiver {
    hist: Vec<u8>,
    seen: usize,
    states: [FormatState; 2],
    /// Positions where the last n bits were divisible by g(x).
    pub parity_hits: u64,
}

struct FormatState {
    format: Format,
    rem: u128,
    xn: u128,
    marks: std::collections::VecDeque<usize>,
    last_decode: Option<usize>,
}

impl FormatState {
    fn new(format: Format) -> Self {
        FormatState {
            format,
            rem: 0,
            xn: xpow_mod(format.n(), format.g()),
            marks: Default::default(),
            last_decode: None,
        }
    }
}

const HISTORY: usize = 8192;

impl Default for StreamReceiver {
    fn default() -> Self {
        StreamReceiver {
            hist: Vec::with_capacity(HISTORY * 2),
            seen: 0,
            states: [
                FormatState::new(Format::Long),
                FormatState::new(Format::Short),
            ],
            parity_hits: 0,
        }
    }
}

impl StreamReceiver {
    /// A new burst began.
    pub fn new_burst(&mut self) {
        self.hist.clear();
        self.seen = 0;
        for s in self.states.iter_mut() {
            *s = FormatState::new(s.format);
        }
    }
    /// Adds one bit; returns a telegram when a window checks.
    pub fn push(&mut self, bit: u8, out: &mut Vec<Decoded>) {
        if self.hist.len() >= 2 * HISTORY {
            self.hist.drain(..HISTORY);
        }
        self.hist.push(bit & 1);
        self.seen += 1;
        let t = self.seen; // bits seen; the newest is at index t - 1
        let base = t - self.hist.len();
        for s in self.states.iter_mut() {
            let n = s.format.n();
            let g = s.format.g();
            s.rem = (s.rem << 1) | (bit as u128 & 1);
            if s.rem & (1u128 << 75) != 0 {
                s.rem ^= g;
            }
            if t > n && self.hist[t - 1 - n - base] == 1 {
                s.rem ^= s.xn;
            }
            if t >= n && s.rem == 0 {
                self.parity_hits += 1;
                s.marks.push_back(t);
            }
            let r = if t > 7500 { n } else { s.format.r() };
            while let Some(&m) = s.marks.front() {
                if m + r > t {
                    break;
                }
                s.marks.pop_front();
                if s.last_decode.is_some_and(|d| m < d + n) {
                    continue;
                }
                let lo = m - n;
                if lo < base || m + r > t {
                    continue;
                }
                let w = &self.hist[lo - base..m + r - base];
                if let Some(dec) = decode_window(s.format, w) {
                    s.last_decode = Some(m);
                    out.push(dec);
                }
            }
        }
    }
}

/// The 1024 valid 11-bit Eurobalise words, SUBSET-036 Annex B2 (10-bit value v maps to entry v).
pub const BALISE_WORDS: [u16; 1024] = [
    65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88,
    89, 90, 91, 92, 93, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
    112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 129, 134, 137, 140,
    142, 143, 144, 146, 147, 148, 149, 150, 153, 155, 164, 165, 166, 171, 175, 176, 177, 186, 187,
    188, 189, 190, 193, 195, 205, 207, 208, 209, 218, 220, 225, 226, 227, 228, 230, 234, 235, 239,
    240, 252, 254, 257, 259, 260, 261, 262, 263, 264, 265, 266, 267, 270, 271, 272, 276, 277, 278,
    279, 282, 283, 290, 291, 293, 302, 303, 304, 305, 308, 309, 312, 313, 314, 316, 317, 318, 321,
    322, 323, 324, 325, 326, 327, 334, 335, 336, 337, 338, 339, 340, 341, 344, 345, 346, 347, 348,
    349, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 377, 379,
    382, 385, 386, 388, 389, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405,
    406, 407, 408, 412, 413, 420, 421, 422, 423, 424, 425, 426, 427, 428, 429, 430, 431, 432, 433,
    434, 435, 438, 439, 442, 444, 445, 446, 449, 458, 459, 462, 463, 464, 465, 466, 467, 472, 473,
    474, 475, 476, 477, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496,
    497, 500, 501, 502, 503, 506, 507, 510, 513, 516, 517, 526, 527, 528, 529, 530, 531, 532, 533,
    536, 537, 538, 539, 540, 541, 547, 548, 549, 550, 551, 556, 559, 560, 561, 562, 573, 574, 577,
    578, 579, 584, 588, 589, 590, 591, 592, 593, 594, 595, 596, 597, 598, 599, 600, 601, 602, 603,
    610, 611, 612, 613, 614, 615, 617, 618, 619, 620, 621, 622, 623, 624, 628, 630, 631, 638, 641,
    652, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 675, 676, 677, 683, 684,
    685, 686, 687, 688, 689, 698, 699, 700, 701, 702, 705, 706, 707, 709, 710, 711, 719, 720, 721,
    730, 732, 733, 738, 739, 740, 741, 744, 745, 746, 747, 749, 750, 751, 752, 753, 756, 757, 760,
    761, 762, 763, 764, 766, 769, 771, 774, 775, 780, 781, 782, 783, 784, 788, 789, 793, 795, 796,
    797, 803, 805, 814, 815, 816, 818, 828, 829, 830, 833, 834, 835, 836, 837, 846, 847, 848, 852,
    858, 859, 868, 870, 872, 873, 874, 875, 876, 879, 880, 881, 882, 883, 886, 887, 894, 897, 899,
    900, 901, 902, 903, 904, 905, 906, 907, 908, 909, 910, 911, 912, 913, 914, 915, 916, 917, 918,
    920, 921, 922, 923, 925, 931, 932, 933, 936, 937, 938, 939, 940, 941, 942, 943, 944, 945, 954,
    956, 957, 958, 961, 976, 996, 997, 998, 999, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007,
    1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020, 1021, 1026, 1027,
    1028, 1029, 1030, 1031, 1032, 1033, 1034, 1035, 1036, 1037, 1038, 1039, 1040, 1041, 1042, 1043,
    1044, 1045, 1046, 1047, 1048, 1049, 1050, 1051, 1071, 1086, 1089, 1090, 1091, 1093, 1102, 1103,
    1104, 1105, 1106, 1107, 1108, 1109, 1110, 1111, 1114, 1115, 1116, 1122, 1124, 1125, 1126, 1127,
    1129, 1130, 1131, 1132, 1133, 1134, 1135, 1136, 1137, 1138, 1139, 1140, 1141, 1142, 1143, 1144,
    1145, 1146, 1147, 1148, 1150, 1153, 1160, 1161, 1164, 1165, 1166, 1167, 1168, 1171, 1172, 1173,
    1174, 1175, 1177, 1179, 1188, 1189, 1195, 1199, 1200, 1201, 1210, 1211, 1212, 1213, 1214, 1217,
    1218, 1219, 1229, 1231, 1232, 1233, 1242, 1244, 1250, 1251, 1252, 1254, 1258, 1259, 1263, 1264,
    1265, 1266, 1267, 1272, 1273, 1276, 1278, 1281, 1283, 1284, 1285, 1286, 1287, 1290, 1291, 1294,
    1295, 1296, 1297, 1298, 1300, 1301, 1302, 1303, 1306, 1307, 1308, 1309, 1314, 1315, 1317, 1326,
    1327, 1328, 1336, 1337, 1338, 1340, 1341, 1342, 1345, 1346, 1347, 1348, 1349, 1358, 1359, 1360,
    1361, 1362, 1363, 1364, 1370, 1371, 1372, 1380, 1381, 1382, 1383, 1384, 1385, 1386, 1387, 1388,
    1389, 1390, 1391, 1392, 1395, 1406, 1409, 1416, 1417, 1419, 1423, 1424, 1425, 1426, 1427, 1428,
    1429, 1430, 1432, 1433, 1434, 1435, 1436, 1437, 1444, 1445, 1446, 1447, 1448, 1449, 1450, 1451,
    1452, 1453, 1454, 1455, 1456, 1457, 1458, 1459, 1463, 1468, 1469, 1470, 1473, 1474, 1485, 1486,
    1487, 1488, 1491, 1496, 1497, 1498, 1499, 1500, 1506, 1507, 1508, 1509, 1510, 1511, 1514, 1515,
    1516, 1517, 1518, 1519, 1520, 1521, 1530, 1531, 1534, 1537, 1540, 1541, 1544, 1545, 1546, 1547,
    1550, 1551, 1552, 1553, 1554, 1555, 1556, 1557, 1558, 1559, 1560, 1561, 1562, 1563, 1564, 1565,
    1570, 1571, 1572, 1573, 1574, 1575, 1580, 1581, 1582, 1583, 1584, 1585, 1588, 1589, 1598, 1601,
    1602, 1603, 1605, 1608, 1609, 1612, 1613, 1614, 1615, 1616, 1617, 1618, 1619, 1620, 1621, 1622,
    1623, 1624, 1625, 1626, 1627, 1634, 1635, 1639, 1640, 1641, 1642, 1643, 1644, 1645, 1646, 1647,
    1648, 1649, 1650, 1651, 1652, 1653, 1654, 1655, 1658, 1659, 1661, 1662, 1665, 1668, 1670, 1676,
    1677, 1678, 1679, 1680, 1681, 1682, 1683, 1684, 1685, 1686, 1687, 1688, 1689, 1690, 1691, 1698,
    1699, 1700, 1701, 1702, 1703, 1706, 1707, 1708, 1709, 1710, 1711, 1712, 1713, 1720, 1721, 1722,
    1723, 1724, 1725, 1726, 1729, 1730, 1731, 1733, 1734, 1735, 1738, 1739, 1742, 1743, 1744, 1745,
    1754, 1756, 1757, 1764, 1765, 1768, 1769, 1770, 1771, 1775, 1776, 1777, 1780, 1781, 1782, 1783,
    1784, 1785, 1786, 1787, 1788, 1790, 1793, 1795, 1807, 1808, 1812, 1813, 1817, 1819, 1820, 1821,
    1822, 1827, 1829, 1838, 1839, 1840, 1842, 1852, 1854, 1857, 1858, 1859, 1860, 1861, 1870, 1871,
    1872, 1876, 1881, 1882, 1883, 1892, 1894, 1897, 1898, 1899, 1900, 1901, 1903, 1904, 1905, 1907,
    1910, 1913, 1918, 1921, 1922, 1923, 1924, 1925, 1926, 1927, 1928, 1929, 1930, 1931, 1932, 1933,
    1934, 1935, 1936, 1937, 1938, 1939, 1940, 1941, 1942, 1943, 1944, 1945, 1946, 1947, 1948, 1949,
    1950, 1954, 1955, 1956, 1957, 1958, 1959, 1960, 1961, 1962, 1963, 1964, 1965, 1966, 1967, 1968,
    1969, 1970, 1971, 1972, 1973, 1974, 1975, 1976, 1977, 1978, 1979, 1980, 1981, 1982,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_check_values() {
        assert_eq!(BALISE_WORDS.len(), 1024);
        assert_eq!(&BALISE_WORDS[..4], &[0x041, 0x042, 0x043, 0x044]);
        assert_eq!(&BALISE_WORDS[1020..], &[0x7BB, 0x7BC, 0x7BD, 0x7BE]);
        assert_eq!(
            BALISE_WORDS.iter().map(|&w| w as u64).sum::<u64>(),
            1_048_064
        );
        assert_eq!(BALISE_WORDS.iter().fold(0u16, |a, &w| a ^ w), 0);
        assert!(BALISE_WORDS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn mersenne_twister_known_output() {
        // The first output of MT19937 seeded with 5489 is 3499211612.
        assert_eq!(Mt19937::new(5489).next_u32(), 3_499_211_612);
    }

    fn rotations(format: Format, t: &[u8], every: usize) {
        let n = format.n();
        let r = format.r();
        for k in (0..n).step_by(every) {
            let w: Vec<u8> = (0..n + r).map(|i| t[(k + i) % n]).collect();
            let d = decode_window(format, &w).unwrap_or_else(|| panic!("shift {k}"));
            assert!(!d.inverted);
        }
    }

    #[test]
    fn encode_and_decode() {
        for format in [Format::Long, Format::Short] {
            let user = test_user_bits(format, 1201, 0);
            let mut ok = 0;
            for b in 1..=50u16 {
                let Some(t) = encode(format, &user, b) else {
                    continue;
                };
                ok += 1;
                assert_eq!(t.len(), format.n());
                let n = format.n();
                let r = format.r();
                let w: Vec<u8> = (0..n + r).map(|i| t[(i + 17 * b as usize) % n]).collect();
                let d = decode_window(format, &w).expect("decodes");
                assert_eq!(d.user, user);
                assert_eq!(d.b, b);
                // Inverted.
                let wi: Vec<u8> = w.iter().map(|x| x ^ 1).collect();
                let di = decode_window(format, &wi).expect("decodes inverted");
                assert!(di.inverted);
                assert_eq!(di.user, user);
                // One bit error.
                let mut we = w.clone();
                we[5] ^= 1;
                assert!(decode_window(format, &we).is_none());
                if b <= 3 {
                    rotations(format, &t, 1);
                }
            }
            assert!(ok > 0, "{format:?}");
        }
    }

    #[test]
    fn format_bit_is_checked() {
        let user = test_user_bits(Format::Short, 1202, 0);
        let (mut t, _) = encode_first(Format::Short, &user).unwrap();
        let n = t.len();
        t[n - 1 - 108] ^= 1;
        let w: Vec<u8> = (0..n + 121).map(|i| t[i % n]).collect();
        assert!(decode_window(Format::Short, &w).is_none());
    }

    #[test]
    fn streaming_finds_one_per_n_bits() {
        let user = test_user_bits(Format::Long, 1201, 1);
        let (t, _) = encode_first(Format::Long, &user).unwrap();
        let mut rx = StreamReceiver::default();
        rx.new_burst();
        let mut out = Vec::new();
        let total = 10 * 1023;
        for i in 0..total {
            rx.push(t[(i + 300) % 1023], &mut out);
        }
        // r = 77 for the first 7500 bits, then n: one telegram per n bits.
        assert!(out.len() >= 7 && out.len() <= 10, "{}", out.len());
        assert!(out.iter().all(|d| d.user == user));
        let h = header(&out[0].user).unwrap();
        assert_eq!(
            (h.nid_c, h.nid_bg, h.n_pig, h.m_version),
            (466, 1201, 1, 33)
        );
        assert_eq!(version_text(33), "2.1");
    }
}

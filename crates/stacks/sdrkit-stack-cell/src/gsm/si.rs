//! GSM System Information (3GPP TS 44.018): parsing and building SI1, SI2, SI2bis, SI2ter, SI2quater,
//! SI3, SI4 and SI13 frames, and the frequency list formats with the range decoding of Annex J.

use serde::{Deserialize, Serialize};

/// The fill octet of unused frame space.
pub const FILL: u8 = 0x2B;

/// The display name of a System Information type code.
pub fn type_name(code: u8) -> String {
    match code {
        0x19 => "SI1 (0x19)".into(),
        0x1A => "SI2 (0x1A)".into(),
        0x02 => "SI2bis (0x02)".into(),
        0x03 => "SI2ter (0x03)".into(),
        0x07 => "SI2quater (0x07)".into(),
        0x1B => "SI3 (0x1B)".into(),
        0x1C => "SI4 (0x1C)".into(),
        0x00 => "SI13 (0x00)".into(),
        c => format!("type 0x{c:02X}"),
    }
}

/// A location area identity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lai {
    /// MCC.
    pub mcc: u16,
    /// MNC.
    pub mnc: u16,
    /// MNC digits (2 or 3).
    pub mnc_digits: u8,
    /// LAC.
    pub lac: u16,
}

/// The cell parameters of SI3 and SI4, as broadcast (indices, not meanings).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Si3Fields {
    /// Cell identity.
    pub cell_id: u16,
    /// ATT.
    pub attach: bool,
    /// BS_AG_BLKS_RES.
    pub bs_ag_blks_res: u8,
    /// CCCH_CONF.
    pub ccch_conf: u8,
    /// BS_PA_MFRMS (value + 2).
    pub bs_pa_mfrms: u8,
    /// T3212, decihours.
    pub t3212_decihours: u8,
    /// RADIO_LINK_TIMEOUT index.
    pub radio_link_timeout_index: u8,
    /// DTX.
    pub dtx: u8,
    /// PWRC.
    pub pwrc: bool,
    /// CELL_RESELECT_HYSTERESIS x 2 dB.
    pub cell_reselect_hysteresis_db: u8,
    /// MS_TXPWR_MAX_CCH.
    pub ms_txpwr_max_cch: u8,
    /// RXLEV_ACCESS_MIN.
    pub rxlev_access_min: u8,
    /// NECI.
    pub neci: bool,
    /// ACS.
    pub acs: bool,
    /// MAX_RETRANS index.
    pub max_retrans: u8,
    /// TX_INTEGER index.
    pub tx_integer: u8,
    /// CELL_BAR_ACCESS.
    pub cell_bar_access: bool,
    /// Access control classes barred (bit n = class n).
    pub access_classes_barred: u16,
}

/// A frequency list: the ARFCNs, or none when the format is not one this receiver decodes.
pub type FreqList = Option<Vec<u16>>;

/// A parsed System Information message.
#[derive(Clone, Debug, PartialEq)]
pub enum Si {
    /// SI1: the cell allocation.
    Si1 {
        /// Cell channel description.
        cell_allocation: FreqList,
        /// RACH octets.
        rach: [u8; 3],
    },
    /// SI2: BCCH neighbours.
    Si2 {
        /// Neighbour cell description.
        neighbours: FreqList,
        /// NCC permitted.
        ncc_permitted: u8,
        /// RACH octets.
        rach: [u8; 3],
    },
    /// SI3.
    Si3 {
        /// LAI.
        lai: Lai,
        /// The fields.
        fields: Si3Fields,
    },
    /// SI4.
    Si4 {
        /// LAI.
        lai: Lai,
        /// Cell selection and RACH octets.
        sel: [u8; 2],
        /// RACH octets.
        rach: [u8; 3],
    },
    /// A type recognised but not parsed further (SI2bis, SI2ter, SI2quater, SI13).
    Other(u8),
}

impl Si {
    /// The type code.
    pub fn code(&self) -> u8 {
        match self {
            Si::Si1 { .. } => 0x19,
            Si::Si2 { .. } => 0x1A,
            Si::Si3 { .. } => 0x1B,
            Si::Si4 { .. } => 0x1C,
            Si::Other(c) => *c,
        }
    }
}

fn digit(d: u8) -> Option<u16> {
    (d <= 9).then_some(d as u16)
}

/// Reads a LAI (5 octets).
pub fn parse_lai(o: &[u8]) -> Option<Lai> {
    let mcc = digit(o[0] & 15)? * 100 + digit(o[0] >> 4)? * 10 + digit(o[1] & 15)?;
    let m3 = o[1] >> 4;
    let (mnc, digits) = if m3 == 0xF {
        (digit(o[2] & 15)? * 10 + digit(o[2] >> 4)?, 2)
    } else {
        (
            digit(o[2] & 15)? * 100 + digit(o[2] >> 4)? * 10 + digit(m3)?,
            3,
        )
    };
    Some(Lai {
        mcc,
        mnc,
        mnc_digits: digits,
        lac: u16::from_be_bytes([o[3], o[4]]),
    })
}

/// Writes a LAI.
pub fn lai_octets(l: &Lai) -> [u8; 5] {
    let d = |v: u16, i: u32| ((v / 10u16.pow(i)) % 10) as u8;
    let (m1, m2, m3) = (d(l.mcc, 2), d(l.mcc, 1), d(l.mcc, 0));
    let (n1, n2, n3) = if l.mnc_digits == 3 {
        (d(l.mnc, 2), d(l.mnc, 1), d(l.mnc, 0))
    } else {
        (d(l.mnc, 1), d(l.mnc, 0), 0xF)
    };
    let lac = l.lac.to_be_bytes();
    [m2 << 4 | m1, n3 << 4 | m3, n2 << 4 | n1, lac[0], lac[1]]
}

fn rach3(o: &[u8]) -> [u8; 3] {
    [o[0], o[1], o[2]]
}

/// Fills the RACH and cell selection fields into `f` from their octets.
pub fn apply_rach(f: &mut Si3Fields, r: &[u8; 3]) {
    f.max_retrans = r[0] >> 6;
    f.tx_integer = (r[0] >> 2) & 15;
    f.cell_bar_access = (r[0] >> 1) & 1 == 1;
    f.access_classes_barred = u16::from_be_bytes([r[1], r[2]]);
}

/// Fills the cell selection fields.
pub fn apply_selection(f: &mut Si3Fields, s: &[u8; 2]) {
    f.cell_reselect_hysteresis_db = (s[0] >> 5) * 2;
    f.ms_txpwr_max_cch = s[0] & 31;
    f.acs = s[1] >> 7 == 1;
    f.neci = (s[1] >> 6) & 1 == 1;
    f.rxlev_access_min = s[1] & 63;
}

/// Parses a 23-octet frame; none when it is not System Information (or a LAI digit is bad).
pub fn parse(m: &[u8; 23]) -> Option<Si> {
    if m[0] & 3 != 1 || m[1] != 0x06 {
        return None;
    }
    let t = m[2];
    Some(match t {
        0x19 => Si::Si1 {
            cell_allocation: decode_freq_list(&m[3..19]),
            rach: rach3(&m[19..22]),
        },
        0x1A => Si::Si2 {
            neighbours: decode_freq_list(&m[3..19]),
            ncc_permitted: m[19],
            rach: rach3(&m[20..23]),
        },
        0x1B => {
            let lai = parse_lai(&m[5..10])?;
            let mut f = Si3Fields {
                cell_id: u16::from_be_bytes([m[3], m[4]]),
                attach: (m[10] >> 6) & 1 == 1,
                bs_ag_blks_res: (m[10] >> 3) & 7,
                ccch_conf: m[10] & 7,
                bs_pa_mfrms: (m[11] & 7) + 2,
                t3212_decihours: m[12],
                pwrc: (m[13] >> 6) & 1 == 1,
                dtx: (m[13] >> 4) & 3,
                radio_link_timeout_index: m[13] & 15,
                ..Default::default()
            };
            apply_selection(&mut f, &[m[14], m[15]]);
            apply_rach(&mut f, &rach3(&m[16..19]));
            Si::Si3 { lai, fields: f }
        }
        0x1C => Si::Si4 {
            lai: parse_lai(&m[3..8])?,
            sel: [m[8], m[9]],
            rach: rach3(&m[10..13]),
        },
        0x02 | 0x03 | 0x07 | 0x00 => Si::Other(t),
        _ => return None,
    })
}

fn header(len: usize, t: u8) -> [u8; 23] {
    let mut m = [FILL; 23];
    m[0] = ((len as u8) << 2) | 1;
    m[1] = 0x06;
    m[2] = t;
    m
}

fn rach_octets(f: &Si3Fields) -> [u8; 3] {
    let ac = f.access_classes_barred.to_be_bytes();
    [
        (f.max_retrans & 3) << 6 | (f.tx_integer & 15) << 2 | (f.cell_bar_access as u8) << 1,
        ac[0],
        ac[1],
    ]
}

fn selection_octets(f: &Si3Fields) -> [u8; 2] {
    [
        ((f.cell_reselect_hysteresis_db / 2) & 7) << 5 | (f.ms_txpwr_max_cch & 31),
        (f.acs as u8) << 7 | (f.neci as u8) << 6 | (f.rxlev_access_min & 63),
    ]
}

/// Builds SI1.
pub fn build_si1(alloc: &[u16], f: &Si3Fields) -> [u8; 23] {
    let mut m = header(21, 0x19);
    m[3..19].copy_from_slice(&encode_freq_list(alloc));
    m[19..22].copy_from_slice(&rach_octets(f));
    m
}

/// Builds SI2.
pub fn build_si2(neigh: &[u16], ncc_permitted: u8, f: &Si3Fields) -> [u8; 23] {
    let mut m = header(22, 0x1A);
    m[3..19].copy_from_slice(&encode_freq_list(neigh));
    m[19] = ncc_permitted;
    m[20..23].copy_from_slice(&rach_octets(f));
    m
}

/// Builds SI3.
pub fn build_si3(lai: &Lai, f: &Si3Fields) -> [u8; 23] {
    let mut m = header(18, 0x1B);
    m[3..5].copy_from_slice(&f.cell_id.to_be_bytes());
    m[5..10].copy_from_slice(&lai_octets(lai));
    m[10] = (f.attach as u8) << 6 | (f.bs_ag_blks_res & 7) << 3 | (f.ccch_conf & 7);
    m[11] = f.bs_pa_mfrms.saturating_sub(2) & 7;
    m[12] = f.t3212_decihours;
    m[13] = (f.pwrc as u8) << 6 | (f.dtx & 3) << 4 | (f.radio_link_timeout_index & 15);
    m[14..16].copy_from_slice(&selection_octets(f));
    m[16..19].copy_from_slice(&rach_octets(f));
    m
}

/// Builds SI4.
pub fn build_si4(lai: &Lai, f: &Si3Fields) -> [u8; 23] {
    let mut m = header(12, 0x1C);
    m[3..8].copy_from_slice(&lai_octets(lai));
    m[8..10].copy_from_slice(&selection_octets(f));
    m[10..13].copy_from_slice(&rach_octets(f));
    m
}

/// A frame of type `t` with no content (for SI2ter in the test signal).
pub fn build_empty(t: u8) -> [u8; 23] {
    header(0, t)
}

/// The L2 fill frame.
pub fn fill_frame() -> [u8; 23] {
    [FILL; 23]
}

struct BitReader<'a> {
    o: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn read(&mut self, n: usize) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            let b = self
                .o
                .get(self.pos / 8)
                .map_or(0, |x| (x >> (7 - self.pos % 8)) & 1);
            v = (v << 1) | b as u32;
            self.pos += 1;
        }
        v
    }
    fn left(&self) -> usize {
        (self.o.len() * 8).saturating_sub(self.pos)
    }
}

/// Annex J: F(K) of node K from W(1..) for range `r`. The 1024 range returns F directly; the others return N to add to
/// ORIG-ARFCN.
pub fn range_node(w: &[u32], k: usize, r: i64) -> i64 {
    let mut index = k;
    let mut j = 1usize << (usize::BITS - 1 - index.leading_zeros());
    let mut n = w[index] as i64;
    while index > 1 {
        let jj = j as i64;
        if 2 * index < 3 * j {
            index -= j / 2;
            n = (n + w[index] as i64 + r / jj - 2).rem_euclid(2 * r / jj - 1) + 1;
        } else {
            index -= j;
            n = (n + w[index] as i64 + 2 * r / jj - 2).rem_euclid(2 * r / jj - 1) + 1;
        }
        j /= 2;
    }
    n
}

fn width(first: usize, k: usize) -> usize {
    // W(1) has `first` bits; each tree level one fewer.
    let level = usize::BITS as usize - 1 - k.leading_zeros() as usize;
    first.saturating_sub(level)
}

/// Decodes a 16-octet frequency list (cell channel description, neighbour cell description).
pub fn decode_freq_list(o: &[u8]) -> FreqList {
    let fmt_hi = o[0] >> 6;
    let mut out: Vec<u16> = Vec::new();
    if fmt_hi == 0 {
        // Bit map 0: octet 15 bit 1 = ARFCN 1 ... octet 0 bit 4 = ARFCN 124.
        for n in 1..=124u16 {
            let bit = (n - 1) as usize;
            let octet = 15 - bit / 8;
            if (o[octet] >> (bit % 8)) & 1 == 1 {
                out.push(n);
            }
        }
        return Some(out);
    }
    if fmt_hi != 2 {
        return None;
    }
    let sub = (o[0] >> 1) & 7;
    let mut br = BitReader { o, pos: 0 };
    if (o[0] >> 3) & 1 == 0 {
        // 1024 range: F0 at bit 3 (the sixth bit sent), W(1) from bit 2.
        br.pos = 5;
        let f0 = br.read(1);
        if f0 == 1 {
            out.push(0);
        }
        // Every node that fits; a null node produces no channel (an encoder can leave a node empty).
        let mut w = vec![0u32];
        let mut k = 1;
        while br.left() >= width(10, k) && width(10, k) > 0 {
            w.push(br.read(width(10, k)));
            k += 1;
        }
        for k in 1..w.len() {
            if w[k] != 0 {
                out.push(range_node(&w, k, 1024) as u16);
            }
        }
    } else {
        let (r, first) = match sub {
            0b100 => (512, 9),
            0b101 => (256, 8),
            0b110 => (128, 7),
            0b111 => (0, 0),
            _ => return None,
        };
        br.pos = 7;
        let orig = br.read(10) as i64;
        out.push(orig as u16);
        if r == 0 {
            let mut i = 1;
            while br.left() > 0 {
                if br.read(1) == 1 {
                    out.push(((orig + i) % 1024) as u16);
                }
                i += 1;
            }
        } else {
            let mut w = vec![0u32];
            let mut k = 1;
            while width(first, k) > 0 && br.left() >= width(first, k) {
                w.push(br.read(width(first, k)));
                k += 1;
            }
            for k in 1..w.len() {
                if w[k] != 0 {
                    out.push(((orig + range_node(&w, k, r)) % 1024) as u16);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    Some(out)
}

/// Encodes a frequency list in 16 octets: the bit map 0 when every ARFCN is 1 to 124, else the 1024 range format.
pub fn encode_freq_list(arfcns: &[u16]) -> [u8; 16] {
    let mut o = [0u8; 16];
    if arfcns.iter().all(|&a| (1..=124).contains(&a)) {
        for &n in arfcns {
            let bit = (n - 1) as usize;
            o[15 - bit / 8] |= 1 << (bit % 8);
        }
        return o;
    }
    let f0 = arfcns.contains(&0);
    let rest: Vec<i64> = arfcns
        .iter()
        .filter(|&&a| a != 0)
        .map(|&a| a as i64)
        .collect();
    let w = range_assign(&rest, 1024, 10, 16).unwrap_or_else(|| vec![0]);
    o[0] = 0b1000_0000 | (f0 as u8) << 2;
    let mut pos = 6usize;
    for (k, &v) in w.iter().enumerate().skip(1) {
        let n = width(10, k);
        for i in (0..n).rev() {
            if pos < 128 {
                o[pos / 8] |= (((v >> i) & 1) as u8) << (7 - pos % 8);
            }
            pos += 1;
        }
    }
    o
}

// Finds W(1..M) whose nodes give exactly `set` (backtracking over which channel each node takes).
fn range_assign(set: &[i64], r: i64, first: usize, max_nodes: usize) -> Option<Vec<u32>> {
    let mut w = vec![0u32];
    let mut left: Vec<i64> = set.to_vec();
    fn go(w: &mut Vec<u32>, left: &mut Vec<i64>, r: i64, first: usize, max: usize) -> bool {
        if left.is_empty() {
            return true;
        }
        let k = w.len();
        if k > max || width(first, k) == 0 {
            return false;
        }
        let bits = width(first, k);
        let mut options: Vec<(u32, usize)> = Vec::new();
        w.push(0);
        for v in 1..(1u32 << bits) {
            w[k] = v;
            let f = range_node(w, k, r);
            if let Some(i) = left.iter().position(|&x| x == f) {
                options.push((v, i));
            }
        }
        for (v, _) in options {
            w[k] = v;
            let f = range_node(w, k, r);
            let Some(i) = left.iter().position(|&x| x == f) else {
                continue;
            };
            let c = left.remove(i);
            if go(w, left, r, first, max) {
                return true;
            }
            left.insert(i, c);
        }
        w.pop();
        false
    }
    go(&mut w, &mut left, r, first, max_nodes).then_some(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_lists() {
        let b = encode_freq_list(&[1, 62, 124]);
        assert_eq!(decode_freq_list(&b), Some(vec![1, 62, 124]));
        for set in [
            vec![958u16, 960, 962],
            vec![955, 958, 961, 967, 970, 973],
            // 17 channels: ARFCN 0 (F0) and 16 W nodes.
            vec![
                0, 955, 958, 961, 964, 967, 970, 973, 976, 979, 982, 985, 988, 991, 994, 997, 1000,
            ],
            vec![0, 1, 975],
        ] {
            let o = encode_freq_list(&set);
            assert_eq!(o[0] >> 6, 2, "range format");
            let mut want = set.clone();
            want.sort_unstable();
            assert_eq!(decode_freq_list(&o), Some(want), "{set:?}");
        }
        let mut unknown = [0u8; 16];
        unknown[0] = 0b0100_0000;
        assert_eq!(decode_freq_list(&unknown), None);
        // The example formulas of 10.5.2.13.3: F2 = (W1 - 512 + W2) smod 1023, F5 = (W1 + (W3 - 256 + W5) smod 511) smod 1023.
        let w = [0u32, 600, 300, 200, 50, 70];
        let smod = |n: i64, m: i64| (n - 1).rem_euclid(m) + 1;
        assert_eq!(range_node(&w, 2, 1024), smod(600 - 512 + 300, 1023));
        assert_eq!(
            range_node(&w, 5, 1024),
            smod(600 + smod(200 - 256 + 70, 511), 1023)
        );
        // A variable bit map.
        let mut vb = [0u8; 16];
        vb[0] = 0b1000_1110; // format 1 0 1 1 1, ORIG high bit 0
        let orig = 960u32;
        let mut br = |pos: usize, v: u32, n: usize| {
            for i in 0..n {
                let b = ((v >> (n - 1 - i)) & 1) as u8;
                vb[(pos + i) / 8] |= b << (7 - (pos + i) % 8);
            }
        };
        br(7, orig, 10);
        br(17 + 1, 1, 1); // ORIG + 2
        assert_eq!(decode_freq_list(&vb), Some(vec![960, 962]));
        // A range list with an empty node in the middle (W2 null, W3 and W7 set).
        let mut o = [0u8; 16];
        o[..9].copy_from_slice(&[0x83, 0xBE, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80]);
        assert_eq!(decode_freq_list(&o), Some(vec![958, 960, 962]));
    }

    #[test]
    fn system_information() {
        let f = Si3Fields {
            cell_id: 12001,
            attach: true,
            bs_ag_blks_res: 1,
            ccch_conf: 0,
            bs_pa_mfrms: 3,
            t3212_decihours: 10,
            radio_link_timeout_index: 11,
            dtx: 2,
            pwrc: false,
            cell_reselect_hysteresis_db: 4,
            ms_txpwr_max_cch: 5,
            rxlev_access_min: 12,
            neci: true,
            acs: false,
            max_retrans: 3,
            tx_integer: 9,
            cell_bar_access: false,
            access_classes_barred: 0,
        };
        let lai = Lai {
            mcc: 1,
            mnc: 1,
            mnc_digits: 2,
            lac: 4711,
        };
        match parse(&build_si3(&lai, &f)) {
            Some(Si::Si3 { lai: l, fields }) => {
                assert_eq!(l, lai);
                assert_eq!(fields, f);
            }
            x => panic!("{x:?}"),
        }
        match parse(&build_si4(&lai, &f)) {
            Some(Si::Si4 { lai: l, sel, rach }) => {
                assert_eq!(l, lai);
                let mut g = Si3Fields::default();
                apply_selection(&mut g, &sel);
                apply_rach(&mut g, &rach);
                assert_eq!(g.rxlev_access_min, 12);
                assert_eq!(g.tx_integer, 9);
            }
            x => panic!("{x:?}"),
        }
        match parse(&build_si1(&[958, 960, 962], &f)) {
            Some(Si::Si1 {
                cell_allocation, ..
            }) => assert_eq!(cell_allocation, Some(vec![958, 960, 962])),
            x => panic!("{x:?}"),
        }
        match parse(&build_si2(&[955, 958, 961, 967, 970, 973], 0xFF, &f)) {
            Some(Si::Si2 {
                neighbours,
                ncc_permitted,
                ..
            }) => {
                assert_eq!(neighbours, Some(vec![955, 958, 961, 967, 970, 973]));
                assert_eq!(ncc_permitted, 0xFF);
            }
            x => panic!("{x:?}"),
        }
        // A 3-digit MNC.
        let l3 = Lai {
            mcc: 310,
            mnc: 410,
            mnc_digits: 3,
            lac: 7,
        };
        assert_eq!(parse_lai(&lai_octets(&l3)), Some(l3));
        // A digit above 9 fails the frame.
        let mut bad = build_si3(&lai, &f);
        bad[5] = 0xA0;
        assert_eq!(parse(&bad), None);
        // Not RR, and the fill frame: not System Information.
        let mut nrr = build_si3(&lai, &f);
        nrr[1] = 0x05;
        assert_eq!(parse(&nrr), None);
        assert_eq!(parse(&fill_frame()), None);
        assert_eq!(parse(&build_empty(0x03)), Some(Si::Other(0x03)));
        assert_eq!(type_name(0x1A), "SI2 (0x1A)");
        assert_eq!(type_name(0x55), "type 0x55");
    }
}

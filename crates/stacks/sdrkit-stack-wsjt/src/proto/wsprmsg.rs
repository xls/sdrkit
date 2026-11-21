//! The 50-bit WSPR message of WSPR and FST4W: types 1, 2 and 3, the 15-bit callsign hash (R. Jenkins, `lookup3.c`) and the
//! table of calls learnt from decodes.
//! Standard: A. Talbot (G4JNT), "The WSPR Coding Process".

use super::msg72::{pack_call, unpack_call};
use super::msg77::PackError;
use std::collections::{HashMap, VecDeque};

/// `hashlittle` of R. Jenkins's `lookup3.c` (public domain).
pub fn lookup3(key: &[u8], initval: u32) -> u32 {
    fn rot(x: u32, k: u32) -> u32 {
        x.rotate_left(k)
    }
    let mut a: u32 = 0xdead_beef_u32
        .wrapping_add(key.len() as u32)
        .wrapping_add(initval);
    let mut b = a;
    let mut c = a;
    let mut k = key;
    let word = |s: &[u8], i: usize| -> u32 {
        let mut v = 0u32;
        for j in 0..4 {
            if let Some(&x) = s.get(i + j) {
                v |= (x as u32) << (8 * j);
            }
        }
        v
    };
    while k.len() > 12 {
        a = a.wrapping_add(word(k, 0));
        b = b.wrapping_add(word(k, 4));
        c = c.wrapping_add(word(k, 8));
        // mix
        a = a.wrapping_sub(c);
        a ^= rot(c, 4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= rot(a, 6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= rot(b, 8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= rot(c, 16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= rot(a, 19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= rot(b, 4);
        b = b.wrapping_add(a);
        k = &k[12..];
    }
    if k.is_empty() {
        return c;
    }
    a = a.wrapping_add(word(k, 0));
    b = b.wrapping_add(word(k, 4));
    c = c.wrapping_add(word(k, 8));
    // final
    c ^= b;
    c = c.wrapping_sub(rot(b, 14));
    a ^= c;
    a = a.wrapping_sub(rot(c, 11));
    b ^= a;
    b = b.wrapping_sub(rot(a, 25));
    c ^= b;
    c = c.wrapping_sub(rot(b, 16));
    a ^= c;
    a = a.wrapping_sub(rot(c, 4));
    b ^= a;
    b = b.wrapping_sub(rot(a, 14));
    c ^= b;
    c = c.wrapping_sub(rot(b, 24));
    c
}

/// The 15-bit hash of a call.
pub fn hash15(call: &str) -> u32 {
    lookup3(call.trim().as_bytes(), 146) & 32767
}

/// Whether `dbm` is a power WSPR can send (0..60, ending in 0, 3 or 7).
pub fn valid_dbm(dbm: i32) -> bool {
    (0..=60).contains(&dbm) && matches!(dbm % 10, 0 | 3 | 7)
}

/// The WSPR hash table: calls of type 1 and 2 decodes, by their 15-bit hash.
#[derive(Clone, Debug, Default)]
pub struct WsprHashes {
    by15: HashMap<u32, String>,
    order: VecDeque<u32>,
}

impl WsprHashes {
    /// The most calls kept.
    pub const CAP: usize = 4096;
    /// Learns a call.
    pub fn learn(&mut self, call: &str) {
        let h = hash15(call);
        if self.by15.insert(h, call.to_string()).is_none() {
            self.order.push_back(h);
            while self.order.len() > Self::CAP {
                if let Some(o) = self.order.pop_front() {
                    self.by15.remove(&o);
                }
            }
        }
    }
    /// The call of a hash.
    pub fn get(&self, h: u32) -> Option<&str> {
        self.by15.get(&h).map(|s| s.as_str())
    }
    /// Forgets everything.
    pub fn clear(&mut self) {
        self.by15.clear();
        self.order.clear();
    }
    /// Calls known.
    pub fn len(&self) -> usize {
        self.by15.len()
    }
    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.by15.is_empty()
    }
}

fn nc37(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as u32),
        b'A'..=b'Z' => Some((c - b'A') as u32 + 10),
        b' ' => Some(36),
        _ => None,
    }
}

fn ch37(v: u32) -> char {
    match v {
        0..=9 => (b'0' + v as u8) as char,
        10..=35 => (b'A' + (v - 10) as u8) as char,
        _ => ' ',
    }
}

/// Packs a WSPR message into its 50 bits.
pub fn pack50(text: &str) -> Result<[u8; 50], PackError> {
    let t = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();
    let w: Vec<&str> = t.split(' ').collect();
    let bad = || PackError::NotWspr;
    let dbm: i32 = w.last().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    if !valid_dbm(dbm) {
        return Err(PackError::Power);
    }
    let (n1, n2): (u32, u32) = match w.as_slice() {
        [call, grid, _] if call.starts_with('<') && call.ends_with('>') => {
            // Type 3: the hash of the call and a 6-character locator.
            let c = call.trim_matches(|c| c == '<' || c == '>');
            let g = grid.as_bytes();
            if g.len() != 6 || !super::msg77::is_grid6(grid) {
                return Err(bad());
            }
            let rot = format!("{}{}", &grid[1..], &grid[..1]);
            let n1 = pack_call(&rot).ok_or_else(bad)?;
            let ntype = -(dbm + 1);
            (n1, hash15(c) * 128 + (ntype + 64) as u32)
        }
        [call, grid, _] => {
            let n1 = pack_call(call)
                .filter(|&n| n < super::msg72::NBASE)
                .ok_or_else(bad)?;
            let g = grid.as_bytes();
            if !super::msg77::is_grid4(grid) {
                return Err(bad());
            }
            let ng = (179 - 10 * (g[0] - b'A') as u32 - (g[2] - b'0') as u32) * 180
                + 10 * (g[1] - b'A') as u32
                + (g[3] - b'0') as u32;
            (n1, ng * 128 + (dbm + 64) as u32)
        }
        [call, _] if call.contains('/') => {
            // Type 2: a prefix (up to 3 characters) or a suffix (one character or two digits).
            let (a, b) = call.split_once('/').ok_or_else(bad)?;
            let (base, n3) = if b.len() <= 2 && a.len() > b.len() && pack_call(a).is_some() {
                let n = match b.as_bytes() {
                    [c] => nc37(*c).filter(|&v| v < 36).ok_or_else(bad)?,
                    [x, y] if x.is_ascii_digit() && y.is_ascii_digit() => {
                        26 + ((x - b'0') * 10 + (y - b'0')) as u32
                    }
                    _ => return Err(bad()),
                };
                (a, 60000 + n)
            } else if (1..=3).contains(&a.len()) {
                let p = format!("{a:>3}");
                let mut n = 0u32;
                for &c in p.as_bytes() {
                    n = 37 * n + nc37(c).ok_or_else(bad)?;
                }
                (b, n)
            } else {
                return Err(bad());
            };
            let n1 = pack_call(base)
                .filter(|&n| n < super::msg72::NBASE)
                .ok_or_else(bad)?;
            let nadd = (n3 >= 32768) as i32;
            let ng = n3 - 32768 * nadd as u32;
            let ntype = dbm + 1 + nadd;
            (n1, ng * 128 + (ntype + 64) as u32)
        }
        _ => return Err(bad()),
    };
    let mut out = [0u8; 50];
    for (i, o) in out.iter_mut().enumerate() {
        *o = if i < 28 {
            ((n1 >> (27 - i)) & 1) as u8
        } else {
            ((n2 >> (49 - i)) & 1) as u8
        };
    }
    Ok(out)
}

/// A decoded WSPR message.
#[derive(Clone, Debug, PartialEq)]
pub struct Wspr50 {
    /// The text (a type 3 shows `<CALL>` or `<...>`).
    pub text: String,
    /// The message type, 1 to 3.
    pub kind: u8,
    /// The call (for type 3, the call of the hash when known).
    pub call: String,
    /// The locator (4 or 6 characters, empty for type 2).
    pub grid: String,
    /// Power, dBm.
    pub dbm: i32,
    /// The hash of a type 3.
    pub hash: u32,
}

/// Unpacks 50 bits, naming a type 3 call from `hashes`.
pub fn unpack50(bits: &[u8], hashes: &WsprHashes) -> Option<Wspr50> {
    let n1 = bits[..28].iter().fold(0u32, |a, &b| (a << 1) | b as u32);
    let n2 = bits[28..50].iter().fold(0u32, |a, &b| (a << 1) | b as u32);
    let ng = n2 >> 7;
    let ntype = (n2 & 127) as i32 - 64;
    if n1 >= super::msg72::NBASE {
        return None;
    }
    let call = unpack_call(n1)?;
    if ntype >= 0 && valid_dbm(ntype) {
        if ng >= 180 * 180 {
            return None;
        }
        // ng = (179 - 10 a - c) x 180 + 10 b + d for the locator a b c d.
        let lon = 179 - ng / 180;
        let b = (ng % 180) / 10;
        let d = ng % 10;
        let grid = format!(
            "{}{}{}{}",
            (b'A' + (lon / 10) as u8) as char,
            (b'A' + b as u8) as char,
            lon % 10,
            d
        );
        if !super::msg77::is_grid4(&grid) {
            return None;
        }
        return Some(Wspr50 {
            text: format!("{call} {grid} {ntype}"),
            kind: 1,
            call,
            grid,
            dbm: ntype,
            hash: 0,
        });
    }
    if ntype > 0 {
        // Type 2.
        let nu = ntype % 10;
        let (dbm, nadd) = match nu {
            1 | 4 | 8 => (ntype - 1, 0),
            2 | 5 | 9 => (ntype - 2, 1),
            _ => return None,
        };
        if !valid_dbm(dbm) {
            return None;
        }
        let n3 = ng + 32768 * nadd as u32;
        let full = if n3 < 60000 {
            let mut n = n3;
            let mut p = [' '; 3];
            for i in (0..3).rev() {
                p[i] = ch37(n % 37);
                n /= 37;
            }
            let p: String = p.iter().collect::<String>().trim().to_string();
            if p.is_empty() {
                return None;
            }
            format!("{p}/{call}")
        } else {
            let n = n3 - 60000;
            let s = match n {
                0..=35 => ch37(n).to_string(),
                36..=125 => format!("{:02}", n - 26),
                _ => return None,
            };
            format!("{call}/{s}")
        };
        return Some(Wspr50 {
            text: format!("{full} {dbm}"),
            kind: 2,
            call: full,
            grid: String::new(),
            dbm,
            hash: 0,
        });
    }
    // Type 3: ntype = -(dbm + 1).
    let dbm = -ntype - 1;
    if !valid_dbm(dbm) {
        return None;
    }
    if call.len() != 6 {
        return None;
    }
    let grid = format!("{}{}", &call[5..], &call[..5]);
    if !super::msg77::is_grid6(&grid) {
        return None;
    }
    let named = hashes.get(ng).map(|c| c.to_string());
    let shown = named.clone().unwrap_or_else(|| "...".into());
    Some(Wspr50 {
        text: format!("<{shown}> {grid} {dbm}"),
        kind: 3,
        call: named.unwrap_or_default(),
        grid,
        dbm,
        hash: ng,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup3_known_answers() {
        assert_eq!(lookup3(b"", 0), 0xdead_beef);
        assert_eq!(lookup3(b"Four score and seven years ago", 0), 0x1777_0551);
        assert_eq!(lookup3(b"Four score and seven years ago", 1), 0xcd62_8161);
    }

    #[test]
    fn round_trips() {
        let mut h = WsprHashes::default();
        h.learn("K1ABC");
        for t in [
            "K1ABC FN42 37",
            "G4XYZ IO91 23",
            "PJ4/K1ABC 30",
            "DL1XYZ/P 10",
            "K1ABC/12 33",
            "<K1ABC> FN42AX 37",
        ] {
            let b = pack50(t).unwrap_or_else(|e| panic!("{t}: {e}"));
            assert_eq!(unpack50(&b, &h).unwrap().text, t);
        }
        let b = pack50("<K1ABC> FN42AX 37").unwrap();
        assert_eq!(
            unpack50(&b, &WsprHashes::default()).unwrap().text,
            "<...> FN42AX 37"
        );
        assert!(pack50("K1ABC FN42 38").is_err());
    }
}

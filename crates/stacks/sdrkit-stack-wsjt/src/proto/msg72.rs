//! The 72-bit message of JT65, JT9 and JT4: two 28-bit call fields and a 16-bit locator or report field, or 13 characters of
//! free text, carried as 12 six-bit symbols.
//! Standard: J. Taylor, "The JT65 Communications Protocol", QEX September/October 2005.

use super::msg77::PackError;

/// The base of the call field: values from it on are tokens.
pub const NBASE: u32 = 37 * 36 * 10 * 27 * 27 * 27;
const NGBASE: u32 = 180 * 180;
const A42: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ +-./?";

fn nc(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as u32),
        b'A'..=b'Z' => Some((c - b'A') as u32 + 10),
        b' ' => Some(36),
        _ => None,
    }
}

/// A standard call (up to 6 characters, digit in the 2nd or 3rd place) as its 28-bit value.
pub fn pack_call(call: &str) -> Option<u32> {
    match call {
        "CQ" => return Some(NBASE + 1),
        "QRZ" => return Some(NBASE + 2),
        "DE" => return Some(267_796_945),
        _ => {}
    }
    if let Some(f) = call.strip_prefix("CQ ") {
        if f.len() == 3 && f.bytes().all(|c| c.is_ascii_digit()) {
            return Some(NBASE + 3 + f.parse::<u32>().ok()?);
        }
    }
    let mut c = call.to_string();
    if c.starts_with("3DA0") {
        c = format!("3D0{}", &c[4..]);
    }
    if c.len() >= 3 && c.starts_with("3X") && c.as_bytes()[2].is_ascii_uppercase() {
        c = format!("Q{}", &c[2..]);
    }
    let b = c.as_bytes();
    if b.len() > 6 || b.len() < 3 {
        return None;
    }
    let six: Vec<u8> = if b[2].is_ascii_digit() {
        let mut v = b.to_vec();
        v.resize(6, b' ');
        v
    } else if b[1].is_ascii_digit() && b.len() <= 5 {
        let mut v = vec![b' '];
        v.extend_from_slice(b);
        v.resize(6, b' ');
        v
    } else {
        return None;
    };
    let suffix = &six[3..];
    let letters = suffix.iter().take_while(|c| c.is_ascii_uppercase()).count();
    if suffix[letters..].iter().any(|&c| c != b' ') || !six[2].is_ascii_digit() {
        return None;
    }
    let mut n = nc(six[0])?;
    let c1 = nc(six[1])?;
    if c1 == 36 {
        return None;
    }
    n = 36 * n + c1;
    n = 10 * n + (six[2] - b'0') as u32;
    for &ch in &six[3..] {
        n = 27 * n + if ch == b' ' { 26 } else { (ch - b'A') as u32 };
    }
    Some(n)
}

/// The text of a call value.
pub fn unpack_call(n: u32) -> Option<String> {
    if n >= NBASE {
        return match n - NBASE {
            1 => Some("CQ".into()),
            2 => Some("QRZ".into()),
            x if (3..1003).contains(&x) => Some(format!("CQ {:03}", x - 3)),
            _ if n == 267_796_945 => Some("DE".into()),
            _ => None,
        };
    }
    let mut n = n;
    let mut s = [b' '; 6];
    for i in (3..6).rev() {
        let j = n % 27;
        n /= 27;
        s[i] = if j == 26 { b' ' } else { b'A' + j as u8 };
    }
    s[2] = b'0' + (n % 10) as u8;
    n /= 10;
    let j = n % 36;
    n /= 36;
    s[1] = if j < 10 {
        b'0' + j as u8
    } else {
        b'A' + (j - 10) as u8
    };
    s[0] = match n {
        0..=9 => b'0' + n as u8,
        10..=35 => b'A' + (n - 10) as u8,
        36 => b' ',
        _ => return None,
    };
    let t = String::from_utf8_lossy(&s).trim().to_string();
    Some(if let Some(r) = t.strip_prefix("3D0") {
        format!("3DA0{r}")
    } else if t.len() > 1 && t.starts_with('Q') && t.as_bytes()[1].is_ascii_uppercase() {
        format!("3X{}", &t[1..])
    } else {
        t
    })
}

/// The value of a 4-character locator (upper case) as the 72-bit message and JS8 pack it: `((w + 180) / 2) * 180 + s + 90`,
/// where `w` is the longitude of the square's centre counted west-positive (the protocol's convention) and `s` the latitude of
/// its south edge. The square comes from `sdrkit-util-geo`, which counts longitude east-positive.
pub fn grid_value(g: &str) -> Option<u32> {
    let b = g.as_bytes();
    if b.len() != 4
        || !(b'A'..=b'R').contains(&b[0])
        || !(b'A'..=b'R').contains(&b[1])
        || !b[2].is_ascii_digit()
        || !b[3].is_ascii_digit()
    {
        return None;
    }
    let sq = sdrkit_util_geo::maidenhead::decode(g).ok()?;
    let west = (-sq.centre().lon_deg).round() as i32;
    let south = sq.south_deg.round() as i32;
    Some((((west + 180) / 2) * 180 + south + 90) as u32)
}

/// The 4-character locator of a value of [`grid_value`]; `None` beyond the 180 x 180 squares.
pub fn grid_text(v: u32) -> Option<String> {
    if v >= NGBASE {
        return None;
    }
    let west = (v / 180) as f64 * 2.0 - 180.0 + 1.0;
    let lat = (v % 180) as f64 - 90.0 + 0.5;
    let p = sdrkit_util_geo::Position::new(lat, -west);
    Some(
        sdrkit_util_geo::maidenhead::encode(&p, 4)
            .ok()?
            .as_str()
            .to_string(),
    )
}

fn pack_grid(g: &str) -> Option<u32> {
    match g {
        "" => return Some(NGBASE + 1),
        "RO" => return Some(NGBASE + 62),
        "RRR" => return Some(NGBASE + 63),
        "73" => return Some(NGBASE + 64),
        _ => {}
    }
    let report = |s: &str| -> Option<i32> {
        let (sign, d) = match s.as_bytes().first()? {
            b'-' => (-1, &s[1..]),
            b'+' => (1, &s[1..]),
            _ => return None,
        };
        if d.is_empty() || d.len() > 2 || !d.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(sign * d.parse::<i32>().ok()?)
    };
    if let Some(r) = g.strip_prefix('R').and_then(report) {
        if (-30..=-1).contains(&r) {
            return Some(NGBASE + 31 + (-r) as u32);
        }
        // Other reports travel as the locator LA00..LA99 (report + 50).
        return (-50..=49)
            .contains(&r)
            .then(|| grid_value(&format!("LA{:02}", r + 50)))
            .flatten();
    }
    if let Some(r) = report(g) {
        if (-30..=-1).contains(&r) {
            return Some(NGBASE + 1 + (-r) as u32);
        }
        return (-50..=49)
            .contains(&r)
            .then(|| grid_value(&format!("KA{:02}", r + 50)))
            .flatten();
    }
    grid_value(g)
}

fn unpack_grid(ng: u32) -> Option<String> {
    if ng < NGBASE {
        let g = grid_text(ng)?;
        // The report locators.
        if let Some(d) = g.strip_prefix("KA") {
            let v: i32 = d.parse().ok()?;
            let r = v - 50;
            return Some(if r >= 0 {
                format!("+{r:02}")
            } else {
                format!("-{:02}", -r)
            });
        }
        if let Some(d) = g.strip_prefix("LA") {
            let v: i32 = d.parse().ok()?;
            let r = v - 50;
            return Some(if r >= 0 {
                format!("R+{r:02}")
            } else {
                format!("R-{:02}", -r)
            });
        }
        return Some(g);
    }
    Some(match ng - NGBASE {
        1 => String::new(),
        x @ 2..=31 => format!("-{:02}", x - 1),
        x @ 32..=61 => format!("R-{:02}", x - 31),
        62 => "RO".into(),
        63 => "RRR".into(),
        64 => "73".into(),
        _ => return None,
    })
}

fn split_symbols(nc1: u32, nc2: u32, ng: u32) -> [u8; 12] {
    [
        (nc1 >> 22 & 63) as u8,
        (nc1 >> 16 & 63) as u8,
        (nc1 >> 10 & 63) as u8,
        (nc1 >> 4 & 63) as u8,
        (4 * (nc1 & 15) + (nc2 >> 26 & 3)) as u8,
        (nc2 >> 20 & 63) as u8,
        (nc2 >> 14 & 63) as u8,
        (nc2 >> 8 & 63) as u8,
        (nc2 >> 2 & 63) as u8,
        (16 * (nc2 & 3) + (ng >> 12 & 15)) as u8,
        (ng >> 6 & 63) as u8,
        (ng & 63) as u8,
    ]
}

/// Packs a message into its 12 six-bit symbols.
pub fn pack72(text: &str) -> Result<[u8; 12], PackError> {
    let t = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();
    let mut w: Vec<String> = t.split(' ').map(|s| s.to_string()).collect();
    // CQ DX and CQ with two letters are folded into the call field.
    if w.len() >= 3
        && w[0] == "CQ"
        && (1..=2).contains(&w[1].len())
        && w[1].bytes().all(|c| c.is_ascii_uppercase())
    {
        let f = format!("CQ9{}", w[1]);
        w.splice(0..2, [f]);
    }
    if w.len() >= 3 && w[0] == "CQ" && w[1].len() == 3 && w[1].bytes().all(|c| c.is_ascii_digit()) {
        let f = format!("CQ {}", w[1]);
        w.splice(0..2, [f]);
    }
    let standard = match w.as_slice() {
        [a, b] => pack_call(a)
            .zip(pack_call(b))
            .map(|(x, y)| (x, y, NGBASE + 1)),
        [a, b, g] => match (pack_call(a), pack_call(b), pack_grid(g)) {
            (Some(x), Some(y), Some(z)) => Some((x, y, z)),
            _ => None,
        },
        [a, b, r, g] if r == "R" => match (pack_call(a), pack_call(b), grid_value(g)) {
            // "R grid" is not in the 72-bit message; refuse to fold it.
            (Some(_), Some(_), Some(_)) => None,
            _ => None,
        },
        _ => None,
    };
    if let Some((nc1, nc2, ng)) = standard {
        // The second field must be a call.
        if nc2 < NBASE {
            return Ok(split_symbols(nc1, nc2, ng));
        }
    }
    // Free text.
    if t.len() > 13 {
        return Err(PackError::TooLong);
    }
    if !t.chars().all(|c| A42.contains(c)) {
        return Err(PackError::Characters);
    }
    let m = format!("{t:<13}");
    let m = m.as_bytes();
    let idx = |c: u8| {
        A42.find(c as char)
            .expect("every character was checked against the free-text alphabet") as u32
    };
    let (mut nc1, mut nc2, mut ng) = (0u32, 0u32, 0u32);
    for &c in &m[0..5] {
        nc1 = 42 * nc1 + idx(c);
    }
    for &c in &m[5..10] {
        nc2 = 42 * nc2 + idx(c);
    }
    for &c in &m[10..13] {
        ng = 42 * ng + idx(c);
    }
    nc1 += nc1;
    if ng & 32768 != 0 {
        nc1 += 1;
    }
    nc2 += nc2;
    if ng & 65536 != 0 {
        nc2 += 1;
    }
    ng = (ng & 32767) + 32768;
    Ok(split_symbols(nc1, nc2, ng))
}

/// The message is the all-zero word ("000AAA 000AAA RA90"). Each code of the 72-bit modes is linear, so the all-zero word is
/// what a tone that never moves off the lowest data tone decodes to. No station sends it, so a decoder must not report it.
pub fn is_null(d: &[u8; 12]) -> bool {
    d.iter().all(|&s| s == 0)
}

/// Unpacks 12 six-bit symbols. `None` when they are not a valid message.
pub fn unpack72(d: &[u8; 12]) -> Option<String> {
    let d: Vec<u32> = d.iter().map(|&x| x as u32).collect();
    let nc1 = (d[0] << 22) | (d[1] << 16) | (d[2] << 10) | (d[3] << 4) | (d[4] >> 2 & 15);
    let nc2 = ((d[4] & 3) << 26)
        | (d[5] << 20)
        | (d[6] << 14)
        | (d[7] << 8)
        | (d[8] << 2)
        | (d[9] >> 4 & 3);
    let ng = ((d[9] & 15) << 12) | (d[10] << 6) | d[11];
    if ng >= 32768 {
        let mut ng = ng & 32767;
        let mut nc1 = nc1;
        let mut nc2 = nc2;
        if nc1 & 1 != 0 {
            ng += 32768;
        }
        nc1 >>= 1;
        if nc2 & 1 != 0 {
            ng += 65536;
        }
        nc2 >>= 1;
        let mut s = [b' '; 13];
        for (range, mut v) in [(0..5, nc1), (5..10, nc2), (10..13, ng)] {
            for i in range.rev() {
                s[i] = A42.as_bytes()[(v % 42) as usize];
                v /= 42;
            }
            if v != 0 {
                return None;
            }
        }
        let t = String::from_utf8_lossy(&s).trim().to_string();
        return (!t.is_empty()).then_some(t);
    }
    let mut c1 = unpack_call(nc1)?;
    let c2 = unpack_call(nc2)?;
    if nc2 >= NBASE {
        return None;
    }
    if let Some(rest) = c1.strip_prefix("CQ9") {
        if rest.bytes().all(|c| c.is_ascii_uppercase()) && !rest.is_empty() {
            c1 = format!("CQ {rest}");
        }
    }
    let g = unpack_grid(ng)?;
    Some(if g.is_empty() {
        format!("{c1} {c2}")
    } else {
        format!("{c1} {c2} {g}")
    })
}

/// The 72 bits of the symbols, most significant first.
pub fn bits72(d: &[u8; 12]) -> [u8; 72] {
    let mut out = [0u8; 72];
    for (i, &s) in d.iter().enumerate() {
        for b in 0..6 {
            out[6 * i + b] = (s >> (5 - b)) & 1;
        }
    }
    out
}

/// The symbols of 72 bits.
pub fn symbols72(bits: &[u8]) -> [u8; 12] {
    let mut d = [0u8; 12];
    for (i, x) in d.iter_mut().enumerate() {
        *x = bits[6 * i..6 * i + 6].iter().fold(0, |a, &b| (a << 1) | b);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for t in [
            "CQ K1ABC FN42",
            "K1ABC W9XYZ EN37",
            "W9XYZ K1ABC -15",
            "K1ABC W9XYZ R-03",
            "K1ABC W9XYZ RRR",
            "K1ABC W9XYZ 73",
            "CQ DX G4ABC IO91",
            "TNX BOB 73 GL",
            "K1ABC W9XYZ +05",
            "K1ABC W9XYZ R+05",
            "CQ 123 K1ABC FN42",
        ] {
            let d = pack72(t).unwrap();
            assert_eq!(unpack72(&d).as_deref(), Some(t), "{t}");
        }
    }

    #[test]
    fn known_answers() {
        assert_eq!(pack_call("CQ"), Some(262_177_561));
        let n = pack_call("K1ABC").unwrap();
        // " K1ABC": base 37/36/10/27/27/27.
        assert_eq!(n, ((((36 * 36 + 20) * 10 + 1) * 27) * 27 + 1) * 27 + 2);
    }

    #[test]
    fn every_locator_survives() {
        for a in b'A'..=b'R' {
            for b in b'A'..=b'R' {
                for c in 0..10 {
                    for d in 0..10 {
                        let g = format!("{}{}{c}{d}", a as char, b as char);
                        if g.starts_with("KA") || g.starts_with("LA") {
                            continue;
                        }
                        let t = format!("K1ABC W9XYZ {g}");
                        assert_eq!(unpack72(&pack72(&t).unwrap()).as_deref(), Some(t.as_str()));
                    }
                }
            }
        }
    }
}

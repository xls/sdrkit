//! ASTERIX output and input: category 021 (ADS-B target reports), 048 (monoradar target reports, for the Mode S replies heard)
//! and 062 (system track data, from the aircraft table). The reader skips the items it does not use.
//!
//! Standard: EUROCONTROL ASTERIX Part 12 (CAT021 ed. 2.x), Part 4 (CAT048 ed. 1.x), Part 9 (CAT062 ed. 1.x)

use crate::feeds::{RemoteState, TargetReport};
use crate::message::NON_ICAO;

/// How long an item is.
#[derive(Clone, Copy, Debug)]
enum Len {
    /// Fixed, bytes.
    F(usize),
    /// Extended by an FX bit in each byte.
    Var,
    /// A count byte then items of this size.
    Rep(usize),
    /// A one-byte length that includes itself.
    Explicit,
    /// Compound: a primary subfield (FX-extended bitmap), then the subfields present.
    C(&'static [Len]),
    /// Extended in steps of this many bytes (FX in the last bit of each step).
    VarN(usize),
    /// Not used in this UAP.
    Spare,
}

use Len::*;

const UAP021: [Len; 49] = [
    F(2),
    Var,
    F(2),
    F(1),
    F(3),
    F(6),
    F(8),
    F(3),
    F(2),
    F(2),
    F(3),
    F(3),
    F(4),
    F(3),
    F(4),
    F(2),
    Var,
    F(1),
    F(2),
    F(2),
    F(2),
    F(2),
    F(1),
    F(2),
    F(2),
    F(4),
    F(2),
    F(3),
    F(6),
    F(1),
    C(&[F(2), F(2), F(2), F(1)]),
    F(2),
    F(2),
    C(&[Var, Rep(15)]),
    F(1),
    F(1),
    Var,
    F(1),
    Rep(8),
    F(7),
    F(1),
    C(&[F(1); 23]),
    Spare,
    Spare,
    Spare,
    Spare,
    Spare,
    Explicit,
    Explicit,
];

const UAP048: [Len; 28] = [
    F(2),
    F(3),
    Var,
    F(4),
    F(2),
    F(2),
    C(&[F(1); 7]),
    F(3),
    F(6),
    Rep(8),
    F(2),
    F(4),
    F(4),
    Var,
    F(4),
    Var,
    F(2),
    F(4),
    F(2),
    C(&[F(2), Rep(6)]),
    F(2),
    F(7),
    F(1),
    F(2),
    F(1),
    F(2),
    Explicit,
    Explicit,
];

const I062_380: [Len; 28] = [
    F(3),
    F(6),
    F(2),
    F(2),
    F(2),
    F(2),
    F(2),
    F(1),
    Rep(15),
    F(2),
    F(2),
    F(7),
    F(2),
    F(2),
    F(2),
    F(2),
    F(2),
    F(2),
    F(1),
    F(8),
    F(1),
    F(6),
    F(2),
    F(1),
    Rep(8),
    F(2),
    F(2),
    F(2),
];

const UAP062: [Len; 35] = [
    F(2),
    Spare,
    F(1),
    F(3),
    F(8),
    F(6),
    F(4),
    F(2),
    F(2),
    F(7),
    C(&I062_380),
    F(2),
    Var,
    C(&[F(1), F(1), F(1), F(1), F(2), F(1), F(1), F(1), F(1), F(1)]),
    F(1),
    C(&[F(1); 31]),
    F(2),
    F(2),
    F(2),
    F(2),
    C(&[
        F(2),
        F(7),
        F(4),
        F(1),
        F(4),
        F(1),
        F(4),
        F(4),
        F(3),
        F(2),
        F(2),
        Rep(4),
        F(6),
        F(1),
        F(7),
        F(7),
        F(2),
        F(7),
    ]),
    Var,
    F(1),
    C(&[F(1), F(4), F(6), F(2), F(2), F(1), F(1)]),
    F(2),
    VarN(3),
    C(&[F(4), F(2), F(4), F(1), F(1), F(2), F(2), F(1)]),
    C(&[F(2), F(4), F(2), F(2), F(2), F(1)]),
    Spare,
    Spare,
    Spare,
    Spare,
    Spare,
    Explicit,
    Explicit,
];

/// Builds one record from (FRN, item bytes) pairs in increasing FRN order.
fn record(items: &[(usize, Vec<u8>)], out: &mut Vec<u8>) {
    let max = items.iter().map(|i| i.0).max().unwrap_or(1);
    let nbytes = max.div_ceil(7);
    let mut fspec = vec![0u8; nbytes];
    for (frn, _) in items {
        let i = frn - 1;
        fspec[i / 7] |= 0x80 >> (i % 7);
    }
    for b in fspec.iter_mut().take(nbytes - 1) {
        *b |= 1;
    }
    out.extend_from_slice(&fspec);
    for (_, d) in items {
        out.extend_from_slice(d);
    }
}

fn block(cat: u8, records: &[u8]) -> Vec<u8> {
    let len = records.len() + 3;
    let mut v = Vec::with_capacity(len);
    v.push(cat);
    v.extend_from_slice(&(len as u16).to_be_bytes());
    v.extend_from_slice(records);
    v
}

fn be(v: i64, n: usize) -> Vec<u8> {
    (0..n).rev().map(|i| (v >> (8 * i)) as u8).collect()
}

fn tod(t: f64) -> Vec<u8> {
    be(((t.rem_euclid(86400.0)) * 128.0).round() as i64, 3)
}

fn chars6(cs: &str) -> Vec<u8> {
    let v = crate::codes::encode_callsign(cs);
    be(v as i64, 6)
}

fn mode3a(sq: u16) -> Vec<u8> {
    be((sq & 0o7777) as i64, 2)
}

fn ecat(cat: (char, u8)) -> u8 {
    match cat {
        ('A', 1) => 1,
        ('A', 2) => 2,
        ('A', 3) => 3,
        ('A', 4) => 4,
        ('A', 5) => 5,
        ('A', 6) => 6,
        ('A', 7) => 10,
        ('B', 1) => 11,
        ('B', 2) => 12,
        ('B', 3) => 16,
        ('B', 4) => 15,
        ('B', 6) => 13,
        ('B', 7) => 14,
        ('C', 1) => 20,
        ('C', 2) => 21,
        ('C', 3) => 22,
        _ => 0,
    }
}

/// One CAT021 data block for a target report.
pub fn encode_021(t: &TargetReport, sac_sic: (u8, u8)) -> Vec<u8> {
    let mut it: Vec<(usize, Vec<u8>)> = Vec::new();
    it.push((1, vec![sac_sic.0, sac_sic.1]));
    let atp = if t.key & NON_ICAO != 0 { 3 } else { 0 };
    it.push((2, vec![atp << 5]));
    it.push((3, be(t.track_no as i64, 2)));
    if let Some((lat, lon)) = t.pos {
        it.push((5, tod(t.tod_s)));
        let s = (1u64 << 23) as f64 / 180.0;
        let mut v = be((lat * s).round() as i64, 3);
        v.extend(be((lon * s).round() as i64, 3));
        it.push((6, v));
    }
    it.push((11, be((t.key & 0xFF_FFFF) as i64, 3)));
    it.push((12, tod(t.tod_s)));
    if let Some(g) = t.gnss_ft {
        it.push((16, be((g as f64 / 6.25).round() as i64, 2)));
    }
    if let Some(v) = t.version {
        it.push((18, vec![((v & 7) << 3) | 2]));
    }
    if let Some(sq) = t.squawk {
        it.push((19, mode3a(sq)));
    }
    if let Some(a) = t.alt_ft {
        it.push((21, be((a as f64 / 25.0).round() as i64, 2)));
    }
    if let Some(h) = t.heading_deg {
        it.push((22, be((h * 65536.0 / 360.0).round() as i64, 2)));
    }
    it.push((
        23,
        vec![(t.emergency.min(6) << 2) | if t.spi { 3 } else { 0 }],
    ));
    if let Some((r, baro)) = t.vrate {
        it.push((
            if baro { 24 } else { 25 },
            be(((r as f64 / 6.25).round() as i64) & 0x7FFF, 2),
        ));
    }
    if let Some((gs, trk)) = t.ground {
        let mut v = be(((gs / 3600.0 * 16384.0).round() as i64) & 0x7FFF, 2);
        v.extend(be((trk * 65536.0 / 360.0).round() as i64, 2));
        it.push((26, v));
    }
    if let Some(cs) = &t.callsign {
        it.push((29, chars6(cs)));
    }
    if let Some(c) = t.category {
        it.push((30, vec![ecat(c)]));
    }
    if let Some(a) = t.sel_alt_ft {
        it.push((32, be(((a as f64 / 25.0).round() as i64) & 0x1FFF, 2)));
    }
    it.sort_by_key(|i| i.0);
    let mut r = Vec::new();
    record(&it, &mut r);
    block(21, &r)
}

/// One CAT048 data block for a Mode S reply.
pub fn encode_048(t: &TargetReport, sac_sic: (u8, u8)) -> Vec<u8> {
    let mut it: Vec<(usize, Vec<u8>)> = Vec::new();
    it.push((1, vec![sac_sic.0, sac_sic.1]));
    it.push((2, tod(t.tod_s)));
    let typ = if t.df == 11 { 4u8 } else { 5 };
    it.push((3, vec![(typ << 5) | if t.spi { 0x04 } else { 0 }]));
    if let Some((rho, theta)) = t.polar {
        let mut v = be(((rho * 256.0).round() as i64).min(0xFFFF), 2);
        v.extend(be((theta * 65536.0 / 360.0).round() as i64, 2));
        it.push((4, v));
    }
    if let Some(sq) = t.squawk {
        it.push((5, mode3a(sq)));
    }
    if let Some(a) = t.alt_ft {
        it.push((6, be(((a as f64 / 25.0).round() as i64) & 0x3FFF, 2)));
    }
    it.push((8, be((t.key & 0xFF_FFFF) as i64, 3)));
    if let Some(cs) = &t.callsign {
        it.push((9, chars6(cs)));
    }
    if let Some((mb, b1, b2)) = t.mb {
        let mut v = vec![1u8];
        v.extend(be(mb as i64, 7));
        v.push((b1 << 4) | b2);
        it.push((10, v));
    }
    it.push((11, be(t.track_no as i64, 2)));
    if let Some((gs, trk)) = t.ground {
        let mut v = be((gs / 3600.0 * 16384.0).round() as i64, 2);
        v.extend(be((trk * 65536.0 / 360.0).round() as i64, 2));
        it.push((13, v));
    }
    let stat = if t.on_ground { 1u8 } else { 0 };
    it.push((21, vec![stat << 2, 0]));
    it.sort_by_key(|i| i.0);
    let mut r = Vec::new();
    record(&it, &mut r);
    block(48, &r)
}

/// One CAT062 data block with a record per track.
pub fn encode_062(tracks: &[TargetReport], sac_sic: (u8, u8)) -> Vec<u8> {
    let mut recs = Vec::new();
    for t in tracks {
        let mut it: Vec<(usize, Vec<u8>)> = Vec::new();
        it.push((1, vec![sac_sic.0, sac_sic.1]));
        it.push((3, vec![0]));
        it.push((4, tod(t.tod_s)));
        if let Some((lat, lon)) = t.pos {
            let s = (1u64 << 25) as f64 / 180.0;
            let mut v = be((lat * s).round() as i64, 4);
            v.extend(be((lon * s).round() as i64, 4));
            it.push((5, v));
        }
        if let Some((gs, trk)) = t.ground {
            let ms = gs * 0.514_444;
            let (vx, vy) = (ms * trk.to_radians().sin(), ms * trk.to_radians().cos());
            let mut v = be(((vx / 0.25).round() as i64) & 0xFFFF, 2);
            v.extend(be(((vy / 0.25).round() as i64) & 0xFFFF, 2));
            it.push((7, v));
        }
        if let Some(sq) = t.squawk {
            it.push((9, mode3a(sq)));
        }
        if let Some(cs) = &t.callsign {
            let mut v = vec![0u8];
            v.extend(chars6(cs));
            it.push((10, v));
        }
        // I062/380: ADR, and IAS, SAL, MAC when known.
        let mut sub: Vec<(usize, Vec<u8>)> = vec![(1, be((t.key & 0xFF_FFFF) as i64, 3))];
        if let Some(i) = t.ias_kt {
            sub.push((4, be((i.round() as i64) & 0x7FFF, 2)));
        }
        if let Some(a) = t.sel_alt_ft {
            sub.push((6, be(((a as f64 / 25.0).round() as i64) & 0x1FFF, 2)));
        }
        if let Some(m) = t.mach {
            sub.push((27, be((m / 0.008).round() as i64, 2)));
        }
        let mut c = Vec::new();
        record(&sub, &mut c);
        it.push((11, c));
        it.push((12, be(t.track_no as i64, 2)));
        it.push((13, vec![0x80]));
        if let Some(a) = t.alt_ft {
            it.push((17, be(((a as f64 / 25.0).round() as i64) & 0xFFFF, 2)));
        }
        if let Some((r, _)) = t.vrate {
            it.push((20, be(((r as f64 / 6.25).round() as i64) & 0xFFFF, 2)));
        }
        it.sort_by_key(|i| i.0);
        record(&it, &mut recs);
    }
    block(62, &recs)
}

// ---------------------------------------------------------------- reading

fn var_len(d: &[u8], at: usize) -> Option<usize> {
    let mut n = 0;
    loop {
        let b = *d.get(at + n)?;
        n += 1;
        if b & 1 == 0 {
            return Some(n);
        }
    }
}

fn fspec(d: &[u8], at: usize) -> Option<(Vec<usize>, usize)> {
    let n = var_len(d, at)?;
    let mut frns = Vec::new();
    for (k, b) in d[at..at + n].iter().enumerate() {
        for bit in 0..7 {
            if b & (0x80 >> bit) != 0 {
                frns.push(k * 7 + bit + 1);
            }
        }
    }
    Some((frns, n))
}

fn item_len(l: &Len, d: &[u8], at: usize) -> Option<usize> {
    match l {
        F(n) => Some(*n),
        Var => var_len(d, at),
        Rep(n) => Some(1 + *d.get(at)? as usize * n),
        Explicit => {
            let n = *d.get(at)? as usize;
            (n >= 1).then_some(n)
        }
        VarN(step) => {
            let mut n = 0;
            loop {
                let b = *d.get(at + n + step - 1)?;
                n += step;
                if b & 1 == 0 {
                    return Some(n);
                }
            }
        }
        C(subs) => {
            let (present, p) = fspec(d, at)?;
            let mut n = p;
            for frn in present {
                let s = subs.get(frn - 1)?;
                n += item_len(s, d, at + n)?;
            }
            Some(n)
        }
        Spare => None,
    }
}

/// The items of one record: (FRN, offset, length), and the record length.
/// (FRN, offset, length) of each item of a record.
type ItemList = Vec<(usize, usize, usize)>;

fn items(uap: &[Len], d: &[u8], at: usize) -> Option<(ItemList, usize)> {
    let (frns, mut n) = fspec(d, at)?;
    let mut out = Vec::with_capacity(frns.len());
    for frn in frns {
        let l = uap.get(frn - 1)?;
        let len = item_len(l, d, at + n)?;
        if at + n + len > d.len() {
            return None;
        }
        out.push((frn, at + n, len));
        n += len;
    }
    Some((out, n))
}

fn get(d: &[u8], o: usize, n: usize) -> i64 {
    d[o..o + n].iter().fold(0i64, |a, &b| (a << 8) | b as i64)
}

fn sget(d: &[u8], o: usize, n: usize, bits: u32) -> i64 {
    let v = get(d, o, n) & ((1i64 << bits) - 1);
    if v >= 1 << (bits - 1) {
        v - (1 << bits)
    } else {
        v
    }
}

fn callsign_at(d: &[u8], o: usize) -> Option<crate::codes::Callsign> {
    let cs = crate::codes::callsign(get(d, o, 6) as u64);
    (!cs.is_empty()).then_some(cs)
}

fn sq_at(d: &[u8], o: usize) -> u16 {
    (get(d, o, 2) & 0o7777) as u16
}

fn decode_021(d: &[u8], items: &[(usize, usize, usize)]) -> Option<RemoteState> {
    let mut s = RemoteState::default();
    let mut key = None;
    let mut gv = None;
    for &(frn, o, n) in items {
        match frn {
            2 => {
                if (d[o] >> 5) & 7 == 3 {
                    s.key |= NON_ICAO;
                }
                if n >= 2 && d[o + 1] & 0x40 != 0 {
                    s.on_ground = Some(true);
                }
            }
            6 => {
                let k = 180.0 / (1u64 << 23) as f64;
                s.pos = Some((
                    sget(d, o, 3, 24) as f64 * k,
                    sget(d, o + 3, 3, 24) as f64 * k,
                ));
            }
            7 => {
                let k = 180.0 / (1u64 << 30) as f64;
                s.pos = Some((
                    sget(d, o, 4, 32) as f64 * k,
                    sget(d, o + 4, 4, 32) as f64 * k,
                ));
            }
            11 => key = Some(get(d, o, 3) as u32),
            19 => s.squawk = Some(sq_at(d, o)),
            21 => s.alt_ft = Some((sget(d, o, 2, 16) * 25) as i32),
            24 | 25 => s.vrate_fpm = Some((sget(d, o, 2, 15) as f64 * 6.25).round() as i32),
            26 => {
                gv = Some((
                    (get(d, o, 2) & 0x7FFF) as f64 * 3600.0 / 16384.0,
                    get(d, o + 2, 2) as f64 * 360.0 / 65536.0,
                ))
            }
            29 => s.callsign = callsign_at(d, o),
            _ => {}
        }
    }
    if let Some((g, t)) = gv {
        s.speed_kt = Some(g);
        s.track_deg = Some(t);
    }
    s.key |= key?;
    Some(s)
}

fn decode_048(d: &[u8], items: &[(usize, usize, usize)]) -> Option<RemoteState> {
    let mut s = RemoteState::default();
    let mut key = None;
    for &(frn, o, _) in items {
        match frn {
            5 => s.squawk = Some(sq_at(d, o)),
            6 => s.alt_ft = Some((sget(d, o, 2, 14) * 25) as i32),
            8 => key = Some(get(d, o, 3) as u32),
            9 => s.callsign = callsign_at(d, o),
            13 => {
                s.speed_kt = Some(get(d, o, 2) as f64 * 3600.0 / 16384.0);
                s.track_deg = Some(get(d, o + 2, 2) as f64 * 360.0 / 65536.0);
            }
            21 => s.on_ground = Some(matches!((d[o] >> 2) & 7, 1 | 3)),
            _ => {}
        }
    }
    s.key = key?;
    Some(s)
}

fn decode_062(d: &[u8], items: &[(usize, usize, usize)]) -> Option<RemoteState> {
    let mut s = RemoteState::default();
    let mut key = None;
    for &(frn, o, _) in items {
        match frn {
            5 => {
                let k = 180.0 / (1u64 << 25) as f64;
                s.pos = Some((
                    sget(d, o, 4, 32) as f64 * k,
                    sget(d, o + 4, 4, 32) as f64 * k,
                ));
            }
            7 => {
                let vx = sget(d, o, 2, 16) as f64 * 0.25;
                let vy = sget(d, o + 2, 2, 16) as f64 * 0.25;
                s.speed_kt = Some(vx.hypot(vy) / 0.514_444);
                let mut t = vx.atan2(vy).to_degrees();
                if t < 0.0 {
                    t += 360.0;
                }
                s.track_deg = Some(t);
            }
            9 => s.squawk = Some(sq_at(d, o)),
            10 => s.callsign = callsign_at(d, o + 1),
            11 => {
                let (subs, mut n) = fspec(d, o)?;
                for frn in subs {
                    let l = item_len(&I062_380[frn - 1], d, o + n)?;
                    if frn == 1 {
                        key = Some(get(d, o + n, 3) as u32);
                    }
                    n += l;
                }
            }
            17 => s.alt_ft = Some((sget(d, o, 2, 16) * 25) as i32),
            20 => s.vrate_fpm = Some((sget(d, o, 2, 16) as f64 * 6.25).round() as i32),
            _ => {}
        }
    }
    s.key = key?;
    Some(s)
}

/// Decodes one data block (category 021, 048 or 062) into aircraft states.
pub fn decode_block(d: &[u8], out: &mut Vec<RemoteState>) -> bool {
    if d.len() < 3 {
        return false;
    }
    let cat = d[0];
    let len = get(d, 1, 2) as usize;
    if len < 3 || len > d.len() {
        return false;
    }
    let uap: &[Len] = match cat {
        21 => &UAP021,
        48 => &UAP048,
        62 => &UAP062,
        _ => return true,
    };
    let mut at = 3;
    while at < len {
        let Some((its, n)) = items(uap, &d[..len], at) else {
            return false;
        };
        let st = match cat {
            21 => decode_021(d, &its),
            48 => decode_048(d, &its),
            _ => decode_062(d, &its),
        };
        if let Some(s) = st {
            out.push(s);
        }
        at += n;
    }
    true
}

/// Decodes a stream of data blocks; returns the bytes used (a block cut short waits for more).
pub fn decode_stream(d: &[u8], out: &mut Vec<RemoteState>) -> usize {
    let mut at = 0;
    while at + 3 <= d.len() {
        let len = get(d, at + 1, 2) as usize;
        if len < 3 {
            at += 1;
            continue;
        }
        if at + len > d.len() {
            break;
        }
        decode_block(&d[at..at + len], out);
        at += len;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> TargetReport {
        TargetReport {
            key: 0x4840D6,
            tod_s: 43200.5,
            callsign: crate::codes::Callsign::try_from_str("KLM1023").ok(),
            squawk: Some(0o7700),
            alt_ft: Some(38000),
            pos: Some((52.26578, 3.93891)),
            polar: Some((120.0, 45.0)),
            ground: Some((450.0, 182.9)),
            vrate: Some((-832, true)),
            ias_kt: Some(280.0),
            mach: Some(0.78),
            sel_alt_ft: Some(36000),
            category: Some(('A', 3)),
            version: Some(2),
            track_no: 7,
            df: 20,
            ..Default::default()
        }
    }

    #[test]
    fn categories_round_trip() {
        let t = report();
        for (cat, data) in [
            (21, encode_021(&t, (1, 2))),
            (48, encode_048(&t, (1, 2))),
            (62, encode_062(&[t.clone(), t.clone()], (1, 2))),
        ] {
            assert_eq!(data[0], cat);
            let mut out = Vec::new();
            assert_eq!(decode_stream(&data, &mut out), data.len());
            assert!(!out.is_empty(), "{cat}");
            let s = &out[0];
            assert_eq!(s.key, 0x4840D6, "{cat}");
            assert_eq!(s.callsign.as_deref(), Some("KLM1023"), "{cat}");
            assert_eq!(s.squawk, Some(0o7700), "{cat}");
            assert_eq!(s.alt_ft, Some(38000), "{cat}");
            assert!((s.speed_kt.unwrap() - 450.0).abs() < 0.5, "{cat}");
            if cat != 48 {
                let (la, lo) = s.pos.unwrap();
                assert!(
                    (la - 52.26578).abs() < 1e-4 && (lo - 3.93891).abs() < 1e-4,
                    "{cat}"
                );
                assert!((s.vrate_fpm.unwrap() + 832).abs() < 7, "{cat}");
            }
        }
        // A block cut short waits.
        let d = encode_021(&t, (0, 1));
        let mut out = Vec::new();
        assert_eq!(decode_stream(&d[..d.len() - 2], &mut out), 0);
    }
}

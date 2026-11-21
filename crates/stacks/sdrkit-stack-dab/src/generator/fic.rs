//! The generator's FIC: FIGs packed into FIBs, then energy dispersal, convolutional coding and puncturing.

use crate::fec::{crc_of, encode_profile};
use crate::tables::{fic_profile, Protection};

/// A FIG with its header byte.
pub fn fig(fig_type: u8, data: &[u8]) -> Vec<u8> {
    let mut v = vec![(fig_type << 5) | data.len() as u8];
    v.extend_from_slice(data);
    v
}

/// FIG 0/0.
pub fn fig0_0(eid: u16, cif_count: u16) -> Vec<u8> {
    let c = cif_count % 5000;
    fig(
        0,
        &[
            0x00,
            (eid >> 8) as u8,
            eid as u8,
            (c / 250) as u8 & 31,
            (c % 250) as u8,
        ],
    )
}

/// FIG 0/7.
pub fn fig0_7(services: u8, count: u16) -> Vec<u8> {
    fig(
        0,
        &[
            0x07,
            (services << 2) | ((count >> 8) as u8 & 3),
            count as u8,
        ],
    )
}

/// FIG 0/10, long form with milliseconds, UTC.
pub fn fig0_10(unix_s: f64) -> Vec<u8> {
    let secs = unix_s.floor() as i64;
    let ms = ((unix_s - secs as f64) * 1000.0).round().min(999.0) as u64;
    let mjd = (secs.div_euclid(86400) + 40587) as u64;
    let tod = secs.rem_euclid(86400) as u64;
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x0A, 8);
    w.put(0, 1);
    w.put(mjd, 17);
    w.put(0, 1);
    w.put(0, 1);
    w.put(1, 1);
    w.put(tod / 3600, 5);
    w.put((tod / 60) % 60, 6);
    w.put(tod % 60, 6);
    w.put(ms, 10);
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// FIG 0/1 entries (id, start, protection, size).
pub fn fig0_1(entries: &[(u8, u16, Protection, u16)]) -> Vec<u8> {
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x01, 8);
    for &(id, start, p, size) in entries {
        w.put(id as u64, 6);
        w.put(start as u64, 10);
        match p {
            Protection::Uep(i) => {
                w.put(0, 1);
                w.put(0, 1);
                w.put(i as u64, 6);
            }
            Protection::Eep { option, level } => {
                w.put(1, 1);
                w.put(option as u64, 3);
                w.put((level - 1) as u64, 2);
                w.put(size as u64, 10);
            }
        }
    }
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// A FIG 0/2 service with its components (TMId, type, sub-channel or SCId, primary, CA).
#[derive(Clone, Debug)]
pub struct Fig02Service {
    /// SId.
    pub sid: u32,
    /// CAId.
    pub caid: u8,
    /// Components: (TMId, ASCTy or DSCTy, sub-channel or SCId, primary, CA).
    pub components: Vec<(u8, u8, u16, bool, bool)>,
}

/// FIG 0/2 (16-bit SIds when `pd` is false).
pub fn fig0_2(services: &[Fig02Service], pd: bool) -> Vec<u8> {
    let mut w_buf = Vec::new();
    let mut w = crate::bits::BitWriter::new(&mut w_buf);
    w.put(0x02 | if pd { 0x20 } else { 0 }, 8);
    for s in services {
        w.put(s.sid as u64, (if pd { 32 } else { 16 }) as u32);
        w.put(0, 1);
        w.put(s.caid as u64, 3);
        w.put(s.components.len() as u64, 4);
        for &(tmid, ty, sc, primary, ca) in &s.components {
            w.put(tmid as u64, 2);
            if tmid == 3 {
                w.put(sc as u64, 12);
            } else {
                w.put(ty as u64, 6);
                w.put(sc as u64, 6);
            }
            w.put(primary as u64, 1);
            w.put(ca as u64, 1);
        }
    }
    fig(0, &{
        let _ = w.finish();
        w_buf
    })
}

/// FIG 0/17 entries (SId, dynamic, language, programme type).
pub fn fig0_17(entries: &[(u16, bool, Option<u8>, u8)]) -> Vec<u8> {
    let mut v = vec![0x11];
    for &(sid, dynamic, lang, pty) in entries {
        v.extend_from_slice(&sid.to_be_bytes());
        v.push(((dynamic as u8) << 7) | ((lang.is_some() as u8) << 5));
        if let Some(l) = lang {
            v.push(l);
        }
        v.push(pty & 31);
    }
    fig(0, &v)
}

/// FIG 0/18 entries (SId, ASu, clusters).
pub fn fig0_18(entries: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
    let mut v = vec![0x12];
    for (sid, asu, clusters) in entries {
        v.extend_from_slice(&sid.to_be_bytes());
        v.extend_from_slice(&asu.to_be_bytes());
        v.push(clusters.len() as u8 & 7);
        v.extend_from_slice(clusters);
    }
    fig(0, &v)
}

/// FIG 0/19 entries (cluster, ASw, sub-channel).
pub fn fig0_19(entries: &[(u8, u16, u8)]) -> Vec<u8> {
    let mut v = vec![0x13];
    for &(cluster, asw, subch) in entries {
        v.push(cluster);
        v.extend_from_slice(&asw.to_be_bytes());
        v.push(subch & 63);
    }
    fig(0, &v)
}

/// FIG 1: extension (0 ensemble, 1 service, 5 data service), identifier bytes, the label (EBU Latin, padded with spaces to 16)
/// and the character flag field.
pub fn fig1(ext: u8, id: &[u8], label: &str, flags: u16) -> Vec<u8> {
    let mut v = vec![ext & 7];
    v.extend_from_slice(id);
    let mut t = crate::registry::to_ebu_latin(label);
    t.resize(16, b' ');
    v.extend_from_slice(&t[..16]);
    v.extend_from_slice(&flags.to_be_bytes());
    fig(1, &v)
}

/// FIG 2 segments of a UTF-8 label, at most 8 character bytes each, cut at character boundaries.
pub fn fig2_segments(ext: u8, id: &[u8], label: &str, toggle: bool, flags: u16) -> Vec<Vec<u8>> {
    let mut pieces: Vec<Vec<u8>> = Vec::new();
    let mut piece: Vec<u8> = Vec::new();
    for ch in label.chars() {
        let mut b = [0u8; 4];
        let s = ch.encode_utf8(&mut b).as_bytes();
        if piece.len() + s.len() > 8 {
            pieces.push(std::mem::take(&mut piece));
        }
        piece.extend_from_slice(s);
    }
    pieces.push(piece);
    let count = pieces.len();
    pieces
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut v = vec![((toggle as u8) << 7) | ((i as u8 & 7) << 4) | (ext & 7)];
            v.extend_from_slice(id);
            if i == 0 {
                v.push(((count as u8 - 1) & 7) << 4);
                v.extend_from_slice(&flags.to_be_bytes());
            }
            v.extend_from_slice(p);
            fig(2, &v)
        })
        .collect()
}

/// Packs FIGs into 12 FIBs: `first` from FIB 0, `rest` from FIB 1, each into the first FIB with room; groups (FIG 2 segments)
/// go together into one FIB. Returns the 12 FIBs with their CRC.
pub fn pack(first: &[Vec<u8>], rest: &[Vec<Vec<u8>>]) -> Vec<[u8; 32]> {
    let mut fibs: Vec<Vec<u8>> = vec![Vec::new(); 12];
    let mut place = |group: &[Vec<u8>], from: usize| {
        let len: usize = group.iter().map(|g| g.len()).sum();
        if let Some(f) = (from..12).find(|&i| fibs[i].len() + len <= 30) {
            for g in group {
                fibs[f].extend_from_slice(g);
            }
        }
    };
    for f in first {
        place(std::slice::from_ref(f), 0);
    }
    for g in rest {
        let len: usize = g.iter().map(|x| x.len()).sum();
        if len <= 30 {
            place(g, 1);
        } else {
            // A label in more segments than one FIB holds: each segment goes on its own.
            for one in g {
                place(std::slice::from_ref(one), 1);
            }
        }
    }
    fibs.iter()
        .map(|f| {
            let mut b = [0u8; 32];
            b[..f.len()].copy_from_slice(f);
            if f.len() < 30 {
                b[f.len()] = 0xFF;
            }
            let c = crc_of(&b[..30]);
            b[30] = (c >> 8) as u8;
            b[31] = c as u8;
            b
        })
        .collect()
}

/// Codes the 12 FIBs of a frame into the 9216 FIC bits (one bit per byte).
pub fn encode_fic(fibs: &[[u8; 32]]) -> Vec<u8> {
    let p = fic_profile();
    let mut out = Vec::with_capacity(9216);
    for g in fibs.chunks(3) {
        let bytes: Vec<u8> = g.iter().flatten().copied().collect();
        encode_profile(&p, &bytes, &mut out);
    }
    out
}

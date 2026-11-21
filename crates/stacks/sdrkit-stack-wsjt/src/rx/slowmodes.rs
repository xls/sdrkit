//! The slow FSK decoders: WSPR, JT65, JT9, JT4, Q65, FST4 and FST4W.

use super::ft::normalise_llrs;
use super::q65;
use super::slot::*;
use super::slow::{self, SlowCand, SlowSpec, SyncKind};
use crate::encode::{self, sync_bits, GRAY4, JT4_SYNC, JT65_SYNC, JT9_SYNC, Q65_SYNC, WSPR_SYNC};
use crate::mode::{Mode, Sub};
use crate::proto::codes::{CodeId, CodeStore};
use crate::proto::msg77::{unpack77, unpack77_learn, CallHashes, RVEC};
use crate::proto::wsprmsg::{unpack50, WsprHashes};
use crate::proto::{jt65, k32, msg72};
use num_complex::Complex32;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_util_bits::crc::cyclic::FST4_CRC24;

/// Shared state of the slow decoders.
pub struct SlowCtx<'a> {
    /// The 77-bit call hash table.
    pub hashes: &'a mut CallHashes,
    /// The WSPR hash table (WSPR and FST4W).
    pub wspr: &'a mut WsprHashes,
    /// Search upside-down signals too (WSPR).
    pub mirrored: bool,
    /// The codes loaded at run time (FST4, FST4W, Q65).
    pub codes: &'a CodeStore,
}

/// What a mode's decoding of one fitted candidate found.
pub struct Found {
    /// The message.
    pub text: String,
    /// The tones sent (for subtraction and the SNR).
    pub tones: Vec<u8>,
    /// Power, dBm (WSPR, FST4W), else -1.
    pub dbm: i32,
    /// The SNR, when the mode measures it itself.
    pub snr: Option<f64>,
    /// Through deep decoding.
    pub deep: bool,
    /// A WSPR type 3 hash to name later in the slot.
    pub hash15: Option<u32>,
}

/// The spec of a slow mode.
pub fn spec(sub: &Sub) -> SlowSpec {
    let mut s = SlowSpec {
        sub: *sub,
        nsym: sub.num_symbols(),
        ntones: sub.num_tones(),
        spacing: sub.spacing_hz(),
        sym_s: sub.symbol_s(),
        sps2: 32,
        sync: SyncKind::Tone0(Vec::new()),
        drifts: vec![0.0],
        hann: false,
        band: (200.0, 3000.0),
        threshold: 0.7,
        peak_bins: 2,
        max_cand: 40,
        bins_per_tone: 2,
        fit_hz: sub.spacing_hz(),
        guard_hz: sub.spacing_hz(),
    };
    match sub.mode {
        Mode::Wspr => {
            s.sync = SyncKind::Parity(sync_bits(WSPR_SYNC));
            s.band = (1250.0, 1750.0);
            s.threshold = 0.12;
        }
        Mode::Jt4 => {
            s.sync = SyncKind::Parity(sync_bits(JT4_SYNC));
            s.sps2 = 16 << sub.sub.min(6);
            s.threshold = 0.10;
            s.max_cand = 20;
        }
        Mode::Jt9 => {
            s.sync = SyncKind::Tone0((0..85).map(|i| JT9_SYNC.contains(&i)).collect());
            s.drifts = vec![0.0, -3.0, 3.0, -6.0, 6.0];
            s.bins_per_tone = 2;
            s.max_cand = 16;
            s.threshold = 1.0;
        }
        Mode::Jt65 => {
            s.sync = SyncKind::Tone0(sync_bits(JT65_SYNC).iter().map(|&b| b == 1).collect());
            s.sps2 = 512;
            s.drifts = vec![0.0, -1.5, 1.5, -3.0, 3.0];
            s.hann = true;
            s.bins_per_tone = 1;
            s.peak_bins = 3;
            s.max_cand = 16;
            s.threshold = 0.7;
        }
        Mode::Q65 => {
            s.sync = SyncKind::Tone0((0..85).map(|i| Q65_SYNC.contains(&i)).collect());
            s.sps2 = 128 << sub.sub.min(4);
            s.drifts = vec![0.0, -3.0, 3.0, -6.0, 6.0];
            s.peak_bins = 6;
            s.max_cand = 12;
            s.threshold = 1.2;
            // The fit and the track of the sync tone work in symbol rates, whatever the tone spacing.
            s.fit_hz = 1.0 / s.sym_s;
            s.guard_hz = s.spacing.max(10.0 / s.sym_s);
        }
        Mode::Fst4 | Mode::Fst4w => {
            let mut w = Vec::new();
            for k in 0..5 {
                for (i, &t) in encode::FST4_SYNC[k % 2].iter().enumerate() {
                    w.push((38 * k + i, t));
                }
            }
            s.sync = SyncKind::Words(w);
            s.sps2 = 16;
            s.threshold = 1.6;
            s.max_cand = 30;
            if sub.mode == Mode::Fst4w {
                s.band = (1300.0, 1700.0);
            }
        }
        _ => {}
    }
    s
}

/// Runs the passes of a slow mode: search, fit, `try_one`, subtraction.
fn run(
    slot: &Slot,
    s: &SlowSpec,
    passes: usize,
    invert_too: bool,
    mirrored: bool,
    mut try_one: impl FnMut(&mut slow::Fit, &[Complex32], &SlowCand) -> Option<Found>,
) -> (Vec<Decode>, Vec<Option<u32>>) {
    let mut audio: Vec<f32> = slot
        .audio
        .iter()
        .map(|&x| if x.is_finite() { x } else { 0.0 })
        .collect();
    let (n, m) = slow::sizes(s, audio.len(), slot.rate);
    let mut out: Vec<Decode> = Vec::new();
    let mut hashes: Vec<Option<u32>> = Vec::new();
    let mut made: Vec<(f64, f64, bool)> = Vec::new();
    let mut amps = Vec::new();
    let orient: &[bool] = if mirrored { &[false, true] } else { &[false] };
    let span = (s.ntones as f64 - 1.0) * s.spacing;
    for &mir in orient {
        if mir {
            flip(&mut audio);
        }
        let mut ms = s.clone();
        if mir {
            ms.band = (
                slot.rate / 2.0 - s.band.1 - span,
                slot.rate / 2.0 - s.band.0 - span,
            );
        }
        for pass in 0..passes {
            let cands = slow::search(&audio, slot, &ms, invert_too);

            let spec = SlotSpectrum::new(&audio, n);
            let mut found = 0;
            let mut taken: Vec<(f64, f64)> = Vec::new();
            for c in cands {
                // Inside the span of a signal decoded in this pass.
                if taken
                    .iter()
                    .any(|&(lo, hi)| c.hz >= lo - 3.0 * s.spacing && c.hz <= hi + 3.0 * s.spacing)
                {
                    continue;
                }
                let mut f = slow::fit(&spec, slot, &ms, &c, m);
                slow::amplitudes(&f, &ms, &mut amps);
                let Some(d) = try_one(&mut f, &amps, &c) else {
                    continue;
                };
                let snr = d.snr.unwrap_or_else(|| slow::snr(&amps, &ms, &d.tones));
                let hz_f = f.hz();
                let hz = if mir { slot.rate / 2.0 - hz_f } else { hz_f };
                let t_tx = slot.t_first + f.t_audio;
                let dt = t_tx - s.sub.t0();
                slow::subtract_decode(&mut audio, slot, &f, &ms, &d.tones);
                found += 1;
                taken.push((hz_f, hz_f + span));
                let dup = out.iter().any(|o| o.text == d.text)
                    || made.iter().any(|&(fm, tm, mm)| {
                        mm == mir
                            && (fm - hz).abs() < 1.5 * s.spacing
                            && (tm - t_tx).abs() < 2.0 * s.sym_s
                    });
                made.push((hz, t_tx, mir));
                if dup {
                    continue;
                }
                let mut rec = Decode::new(slot, d.text, snr, dt, hz, mir, pass as u32);
                rec.dbm = d.dbm;
                rec.deep = d.deep;
                out.push(rec);
                hashes.push(d.hash15);
            }
            if found == 0 {
                break;
            }
        }
        if mir {
            flip(&mut audio);
        }
    }
    (out, hashes)
}

/// Mirrors the spectrum of 8 kHz audio (f to 4000 - f).
fn flip(a: &mut [f32]) {
    for x in a.iter_mut().skip(1).step_by(2) {
        *x = -*x;
    }
}

/// Soft data bits (positive means 0) of a parity-sync mode (tone = sync + 2 x data): log I0 of tone (sync) against tone
/// (sync + 2), the noise from the two tones of the other sync parity.
fn parity_llrs(c: &[Complex32], sync: &[u8], inverted: bool) -> Vec<f32> {
    let n = sync.len();
    let mut noise = 0f64;
    let mut sig = 0f64;
    for (k, &sb) in sync.iter().enumerate() {
        let sb = (sb ^ inverted as u8) as usize;
        let p: [f32; 4] = std::array::from_fn(|t| c[k * 4 + t].norm_sqr());
        noise += ((p[1 - sb] + p[3 - sb]) / 2.0) as f64;
        sig += p[sb].max(p[sb + 2]) as f64;
    }
    let s2 = (noise / n as f64).max(1e-30);
    let a2 = (sig / n as f64 - s2).max(0.05 * s2);
    let kk = (2.0 * a2.sqrt() / s2) as f32;
    sync.iter()
        .enumerate()
        .map(|(k, &sb)| {
            let sb = (sb ^ inverted as u8) as usize;
            let a0 = c[k * 4 + sb].norm();
            let a1 = c[k * 4 + sb + 2].norm();
            (ln_i0(kk * a0) - ln_i0(kk * a1)).clamp(-25.0, 25.0)
        })
        .collect()
}

/// WSPR.
pub fn decode_wspr(slot: &Slot, ctx: &mut SlowCtx) -> Vec<Decode> {
    let s = spec(&slot.sub);
    let sync = sync_bits(WSPR_SYNC);
    let mirrored = ctx.mirrored;
    let wspr = &mut *ctx.wspr;
    let (mut out, hashes) = run(slot, &s, 2, false, mirrored, |_, c, cand| {
        let llr = parity_llrs(c, &sync, cand.inverted);
        let bits = k32::decode(&llr, 50)?.bits;
        let w = unpack50(&bits, wspr)?;
        if w.kind != 3 {
            wspr.learn(&w.call);
        }
        let cw = k32::encode(&bits);
        let tones: Vec<u8> = cw.iter().zip(&sync).map(|(&d, &sb)| sb + 2 * d).collect();
        Some(Found {
            text: w.text,
            tones,
            dbm: w.dbm,
            snr: None,
            deep: false,
            hash15: (w.kind == 3).then_some(w.hash),
        })
    });
    // Hashes learnt later in the slot name its type 3 decodes.
    for (d, h) in out.iter_mut().zip(hashes) {
        if let (Some(h), true) = (h, d.text.starts_with("<...>")) {
            if let Some(c) = ctx.wspr.get(h) {
                d.text = d.text.replacen("<...>", &format!("<{c}>"), 1);
                d.call = c.to_string();
            }
        }
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

/// JT4.
pub fn decode_jt4(slot: &Slot, _ctx: &mut SlowCtx) -> Vec<Decode> {
    let s = spec(&slot.sub);
    let sync = sync_bits(JT4_SYNC);
    let (mut out, _) = run(slot, &s, 2, true, false, |_, c, cand| {
        let llr = parity_llrs(c, &sync, cand.inverted);
        let bits = k32::decode(&llr, 72)?.bits;
        let dat = msg72::symbols72(&bits);
        if msg72::is_null(&dat) {
            return None;
        }
        let text = msg72::unpack72(&dat)?;
        let cw = k32::encode(&bits);
        let tones: Vec<u8> = cw
            .iter()
            .zip(&sync)
            .map(|(&d, &sb)| (sb ^ cand.inverted as u8) + 2 * d)
            .collect();
        Some(Found {
            text,
            tones,
            dbm: -1,
            snr: None,
            deep: false,
            hash15: None,
        })
    });
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

/// JT9.
pub fn decode_jt9(slot: &Slot, _ctx: &mut SlowCtx) -> Vec<Decode> {
    let s = spec(&slot.sub);
    let mut label = vec![0u8; 8];
    for v in 0..8u8 {
        label[(v ^ (v >> 1)) as usize] = v;
    }
    let data: Vec<usize> = (0..85).filter(|i| !JT9_SYNC.contains(i)).collect();
    let (mut out, _) = run(slot, &s, 2, false, false, |_, c, _| {
        let amps: Vec<f32> = data
            .iter()
            .flat_map(|&k| (1..9).map(move |t| c[k * 9 + t].norm()))
            .collect();
        let mut llr = Vec::with_capacity(207);
        fsk_llrs(&amps, 8, 3, &label, &mut llr);
        let bits = k32::decode(&llr[..206], 72)?.bits;
        let dat = msg72::symbols72(&bits);
        if msg72::is_null(&dat) {
            return None;
        }
        let text = msg72::unpack72(&dat)?;
        let tones = encode::channel_symbols(&s.sub, &text, &CodeStore::default()).ok()?;
        Some(Found {
            text,
            tones,
            dbm: -1,
            snr: None,
            deep: false,
            hash15: None,
        })
    });
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

/// JT65 (A, B, C).
pub fn decode_jt65(slot: &Slot, _ctx: &mut SlowCtx) -> Vec<Decode> {
    let s = spec(&slot.sub);
    let sync = sync_bits(JT65_SYNC);
    let data: Vec<usize> = (0..126).filter(|&i| sync[i] == 0).collect();
    let mut rng = Rng::new(0x1234_5678_9abc_def0u64 ^ (slot.utc as u64));
    let (mut out, _) = run(slot, &s, 3, false, false, |_, c, _| {
        // Symbol probabilities of the 63 data symbols (channel order) from the 64 data-tone amplitudes: Rician likelihoods,
        // normalised.
        let mut prob = [[0f32; 64]; 63];
        let mut best = [0u8; 63];
        let mut rel = [0f32; 63];
        let (mut pmax, mut pother) = (0f64, 0f64);
        for &k in &data {
            let e: Vec<f32> = (0..64).map(|g| c[k * 66 + 2 + g].norm_sqr()).collect();
            let mx = e.iter().cloned().fold(0f32, f32::max);
            pmax += mx as f64;
            pother += ((e.iter().sum::<f32>() - mx) / 63.0) as f64;
        }
        let s2 = (pother / 63.0).max(1e-30);
        let a2 = (pmax / 63.0 - s2).max(0.05 * s2);
        let kk = (2.0 * a2.sqrt() / s2) as f32;
        for (j, &k) in data.iter().enumerate() {
            let mut lp = [0f32; 64];
            let mut mx = f32::MIN;
            for (sym, l) in lp.iter_mut().enumerate() {
                *l = ln_i0(kk * c[k * 66 + 2 + (sym ^ (sym >> 1))].norm());
                mx = mx.max(*l);
            }
            let mut tot = 0f32;
            let mut p = [0f32; 64];
            for (pp, l) in p.iter_mut().zip(&lp) {
                *pp = (l - mx).exp();
                tot += *pp;
            }
            let (mut b1, mut i1, mut b2) = (0f32, 0usize, 0f32);
            for (sym, pp) in p.iter_mut().enumerate() {
                *pp /= tot;
                if *pp > b1 {
                    b2 = b1;
                    b1 = *pp;
                    i1 = sym;
                } else if *pp > b2 {
                    b2 = *pp;
                }
            }
            prob[j] = p;
            best[j] = i1 as u8;
            rel[j] = b1 - b2;
        }
        let rx = jt65::deinterleave(&best);
        let prob_d = jt65::deinterleave(&prob);
        let rel_d = jt65::deinterleave(&rel);
        // Reliability ranks, least reliable first.
        let mut order: Vec<usize> = (0..63).collect();
        order.sort_by(|&a, &b| rel_d[a].total_cmp(&rel_d[b]));
        let score = |cw: &[u8; 63]| -> f32 {
            (0..63).map(|i| prob_d[i][cw[i] as usize]).sum::<f32>() / 63.0
        };
        let mut best_dec: Option<([u8; 12], f32, usize)> = None;
        let consider = |eras: &[usize], best_dec: &mut Option<([u8; 12], f32, usize)>| {
            if let Some((dat, cw, _)) = jt65::decode(&rx, eras) {
                // A codeword far from the hard decisions is what many erasures make of noise.
                if (0..63).filter(|&i| cw[i] != rx[i]).count() > 40 {
                    return;
                }
                let sc = score(&cw);
                if best_dec.as_ref().is_none_or(|b| sc > b.1) {
                    *best_dec = Some((dat, sc, eras.len()));
                }
            }
        };
        consider(&[], &mut best_dec);
        for ne in [10usize, 20, 30, 40, 45] {
            consider(&order[..ne], &mut best_dec);
        }
        let mut eras = Vec::with_capacity(63);
        for _ in 0..3000 {
            if best_dec.as_ref().is_some_and(|b| b.2 == 0) {
                break;
            }
            eras.clear();
            for (rank, &i) in order.iter().enumerate() {
                let u = rng.uniform();
                if u < 1.0 - rank as f64 / 70.0 && eras.len() < 51 {
                    eras.push(i);
                }
            }
            consider(&eras, &mut best_dec);
        }
        let (dat, sc, ne) = best_dec?;
        if msg72::is_null(&dat) {
            return None;
        }
        let text = msg72::unpack72(&dat)?;
        let free = dat[9] & 8 != 0;
        let ok = if free {
            ne == 0 && sc > 0.35
        } else {
            sc > 0.12 || ne == 0
        };
        if !ok {
            return None;
        }
        let tones = encode::channel_symbols(&s.sub, &text, &CodeStore::default()).ok()?;
        Some(Found {
            text,
            tones,
            dbm: -1,
            snr: None,
            deep: ne > 0,
            hash15: None,
        })
    });
    // A decode below -28 dB is an alias of a stronger signal.
    out.retain(|d| d.snr >= -28);
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

/// Coherent soft bits (positive means 0) of groups of `g` neighbouring 4-FSK symbols of FST4.
#[allow(clippy::needless_range_loop)]
fn fst4_group_llrs(c: &[Complex32], data: &[usize], g: usize, label: &[u8], out: &mut Vec<f32>) {
    out.clear();
    for blk in data.chunks(30) {
        for chunk in blk.chunks(g) {
            let gs = chunk.len();
            let nb = 2 * gs;
            let mut m1 = vec![f32::MIN; nb];
            let mut m0 = vec![f32::MIN; nb];
            for seq in 0..4usize.pow(gs as u32) {
                let mut x = seq;
                let mut tones = [0usize; 4];
                for j in (0..gs).rev() {
                    tones[j] = x % 4;
                    x /= 4;
                }
                let mut sum = Complex32::default();
                for j in 0..gs {
                    sum += c[chunk[j] * 4 + tones[j]];
                }
                let mag = sum.norm();
                for j in 0..gs {
                    let lab = label[tones[j]];
                    for b in 0..2 {
                        let i = 2 * j + b;
                        if (lab >> (1 - b)) & 1 == 1 {
                            m1[i] = m1[i].max(mag);
                        } else {
                            m0[i] = m0[i].max(mag);
                        }
                    }
                }
            }
            for i in 0..nb {
                out.push(m0[i] - m1[i]);
            }
        }
    }
    normalise_llrs(out);
}

/// Q65, with its code from `ctx.codes`: nothing is decoded when it is not loaded. Up to three passes; each decoded signal is
/// subtracted before the next pass.
pub fn decode_q65(slot: &Slot, ctx: &mut SlowCtx) -> Vec<Decode> {
    let Some(code) = ctx.codes.symbols(CodeId::Q65).cloned() else {
        return Vec::new();
    };
    let s = spec(&slot.sub);
    let mut dec = q65::Decoder::new(&code);
    let hashes = &mut *ctx.hashes;
    let (mut out, _) = run(slot, &s, 3, false, false, |f, _, _| {
        let mut g = q65::Grid::new(f, &s);
        let (offset, drift) = g.track();
        if offset != 0.0 || drift != 0.0 {
            f.df += offset / s.sym_s;
            f.drift_hz += drift / s.sym_s;
            g = q65::Grid::new(f, &s);
        }
        let w = dec.decode(&g, 1.0 / s.sym_s, |bits| {
            unpack77_learn(bits, hashes).map(|u| u.text)
        })?;
        Some(Found {
            text: w.text,
            tones: w.tones,
            dbm: -1,
            snr: Some(w.snr),
            deep: false,
            hash15: None,
        })
    });
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

/// FST4 and FST4W, with their code from `ctx.codes`: nothing is decoded when it is not loaded.
pub fn decode_fst4(slot: &Slot, ctx: &mut SlowCtx) -> Vec<Decode> {
    let s = spec(&slot.sub);
    let w = slot.sub.mode == Mode::Fst4w;
    let id = if w { CodeId::Fst4w } else { CodeId::Fst4 };
    let Some(code) = ctx.codes.get(id).cloned() else {
        return Vec::new();
    };
    let k = code.k();
    let mut label = vec![0u8; 4];
    for (v, &t) in GRAY4.iter().enumerate() {
        label[t as usize] = v as u8;
    }
    let data: Vec<usize> = (0..4).flat_map(|w| 38 * w + 8..38 * w + 38).collect();
    let mut dec = LdpcDecoders::new(&code);
    let mut cw = vec![0u8; code.n()];
    let hashes = &mut *ctx.hashes;
    let wspr = &mut *ctx.wspr;
    let (mut out, _) = run(slot, &s, 1, false, false, |_, c, cand| {
        let amps: Vec<f32> = data
            .iter()
            .flat_map(|&kk| (0..4).map(move |t| c[kk * 4 + t].norm()))
            .collect();
        let mut l1 = Vec::with_capacity(240);
        fsk_llrs(&amps, 4, 2, &label, &mut l1);
        let mut sets = vec![l1];
        for g in [2usize, 4] {
            let mut v = Vec::new();
            fst4_group_llrs(c, &data, g, &label, &mut v);
            sets.push(v);
        }
        let mut deep = false;
        let mut ok = false;
        for osd in [false, true] {
            if osd && cand.score <= 2.0 {
                break;
            }
            for l in sets.iter().filter(|l| l.len() == code.n()) {
                if let Some(r) = dec.decode(l, 50, osd, &mut cw) {
                    if (!r.osd || r.errors < 70) && FST4_CRC24.check(&cw[..k]) {
                        ok = true;
                        deep = r.osd;
                        break;
                    }
                }
            }
            if ok {
                break;
            }
        }
        if !ok {
            return None;
        }
        let bits = &cw[..k];
        let (text, dbm) = if w {
            let u = unpack50(&bits[..50], wspr)?;
            if u.kind != 3 {
                wspr.learn(&u.call);
            }
            (u.text, u.dbm)
        } else {
            let mut m: Vec<u8> = bits[..77].to_vec();
            for (b, r) in m.iter_mut().zip(RVEC.iter()) {
                *b ^= r;
            }
            if deep {
                let p = unpack77(&m, hashes)?;
                if !matches!(p.i3, 1 | 2 | 4) || p.unknown_hash {
                    // Deep results: plain QSO types only; free text needs belief propagation.
                    if p.i3 != 0 || p.n3 != 0 {
                        return None;
                    }
                }
            }
            (unpack77_learn(&m, hashes)?.text, -1)
        };
        let tones = encode::fst4_tones(&cw);
        Some(Found {
            text,
            tones,
            dbm,
            snr: None,
            deep,
            hash15: None,
        })
    });
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

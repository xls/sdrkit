//! The encoder: one stream, blocks sorted by prefix doubling, one Huffman code a block (sent as two equal tables, the fewest
//! the format allows). It favours plain code over ratio and speed; it is meant for test data.

use crate::{BLOCK_MAGIC, END_MAGIC};
use sdrkit_util_bits::crc::catalog::CRC32_BZIP2;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The longest code the encoder makes.
const MAX_LEN: u8 = 17;

/// A writer of bits, most significant bit first.
#[derive(Default)]
struct BitsOut {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitsOut {
    fn put(&mut self, bits: u32, v: u64) {
        for k in (0..bits).rev() {
            self.acc = (self.acc << 1) | ((v >> k) & 1);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put(pad, 0);
        }
        self.out
    }
}

/// Encodes `data` as one bzip2 stream with block size `level` x 100 000 (`level` is held to 1 to 9). Empty data gives a
/// stream with no block.
pub fn compress(data: &[u8], level: u8) -> Vec<u8> {
    let level = level.clamp(1, 9);
    let max_block = usize::from(level) * 100_000 - 19;
    let mut w = BitsOut::default();
    w.put(24, u64::from_be_bytes([0, 0, 0, 0, 0, b'B', b'Z', b'h']));
    w.put(8, u64::from(b'0' + level));
    let mut combined = 0u32;
    let mut block = Vec::with_capacity(max_block);
    let mut crc = CRC32_BZIP2.digest();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        let mut r = 1;
        while r < 255 && i + r < data.len() && data[i + r] == b {
            r += 1;
        }
        let need = if r >= 4 { 5 } else { r };
        if block.len() + need > max_block {
            let c = crc.finalize();
            write_block(&mut w, &block, c);
            combined = combined.rotate_left(1) ^ c;
            block.clear();
            crc.reset();
        }
        if r >= 4 {
            block.extend_from_slice(&[b; 4]);
            block.push((r - 4) as u8);
        } else {
            block.extend(std::iter::repeat_n(b, r));
        }
        crc.update(&data[i..i + r]);
        i += r;
    }
    if !block.is_empty() {
        let c = crc.finalize();
        write_block(&mut w, &block, c);
        combined = combined.rotate_left(1) ^ c;
    }
    w.put(48, END_MAGIC);
    w.put(32, u64::from(combined));
    w.finish()
}

/// The rows of the sorted rotations of `b`, by prefix doubling: entry k is the start of the k-th smallest rotation.
fn sort_rotations(b: &[u8]) -> Vec<u32> {
    let n = b.len();
    let mut sa: Vec<u32> = (0..n as u32).collect();
    let mut rank: Vec<u32> = b.iter().map(|&x| u32::from(x)).collect();
    let mut next = vec![0u32; n];
    let mut k = 1usize;
    loop {
        let key = |i: u32| (u64::from(rank[i as usize]) << 32) | u64::from(rank[(i as usize + k) % n]);
        sa.sort_unstable_by_key(|&i| key(i));
        let mut r = 0u32;
        next[sa[0] as usize] = 0;
        for j in 1..n {
            if key(sa[j]) != key(sa[j - 1]) {
                r += 1;
            }
            next[sa[j] as usize] = r;
        }
        std::mem::swap(&mut rank, &mut next);
        if r as usize == n - 1 || k >= n {
            break;
        }
        k *= 2;
    }
    sa
}

/// Huffman code lengths of at most [`MAX_LEN`] bits for `freq` (every symbol gets a code).
fn code_lengths(freq: &[u32]) -> Vec<u8> {
    let mut w: Vec<u64> = freq.iter().map(|&f| u64::from(f.max(1))).collect();
    loop {
        let n = w.len();
        let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
        let mut parent = vec![usize::MAX; 2 * n];
        for (s, &x) in w.iter().enumerate() {
            heap.push(Reverse((x, s)));
        }
        let mut id = n;
        while heap.len() > 1 {
            let (Some(Reverse((a, ia))), Some(Reverse((b, ib)))) = (heap.pop(), heap.pop()) else {
                break;
            };
            parent[ia] = id;
            parent[ib] = id;
            heap.push(Reverse((a + b, id)));
            id += 1;
        }
        let lens: Vec<u8> = (0..n)
            .map(|s| {
                let mut d = 0u8;
                let mut p = s;
                while parent[p] != usize::MAX {
                    p = parent[p];
                    d = d.saturating_add(1);
                }
                d.max(1)
            })
            .collect();
        if lens.iter().all(|&l| l <= MAX_LEN) {
            return lens;
        }
        for x in &mut w {
            *x = 1 + *x / 2;
        }
    }
}

/// Writes one block of `b` (after the first run-length step) with block CRC `crc`.
fn write_block(w: &mut BitsOut, b: &[u8], crc: u32) {
    let n = b.len();
    let sa = sort_rotations(b);
    let origin = sa.iter().position(|&s| s == 0).unwrap_or(0);
    let mut in_use = [false; 256];
    for &x in b {
        in_use[usize::from(x)] = true;
    }
    let mut seq = [0u8; 256];
    let mut used = 0usize;
    for (v, &u) in in_use.iter().enumerate() {
        if u {
            seq[v] = used as u8;
            used += 1;
        }
    }
    let alpha = used + 2;
    let eob = (alpha - 1) as u16;
    // Move-to-front, with runs of the front byte as RUNA (0) and RUNB (1).
    let mut mtf: Vec<u8> = (0..used).map(|v| v as u8).collect();
    let mut syms: Vec<u16> = Vec::with_capacity(n + 1);
    let mut zeros = 0usize;
    let flush = |zeros: &mut usize, syms: &mut Vec<u16>| {
        if *zeros == 0 {
            return;
        }
        let mut z = *zeros - 1;
        loop {
            syms.push((z & 1) as u16);
            if z < 2 {
                break;
            }
            z = (z - 2) / 2;
        }
        *zeros = 0;
    };
    for &s in &sa {
        let c = b[(s as usize + n - 1) % n];
        let v = seq[usize::from(c)];
        let j = mtf.iter().position(|&m| m == v).unwrap_or(0);
        if j == 0 {
            zeros += 1;
            continue;
        }
        flush(&mut zeros, &mut syms);
        mtf.copy_within(0..j, 1);
        mtf[0] = v;
        syms.push(j as u16 + 1);
    }
    flush(&mut zeros, &mut syms);
    syms.push(eob);
    let mut freq = vec![0u32; alpha];
    for &s in &syms {
        freq[usize::from(s)] += 1;
    }
    let lens = code_lengths(&freq);
    // Canonical codes: by length, then by symbol.
    let mut codes = vec![0u32; alpha];
    let mut code = 0u32;
    for l in 1..=MAX_LEN {
        for s in 0..alpha {
            if lens[s] == l {
                codes[s] = code;
                code += 1;
            }
        }
        code <<= 1;
    }
    w.put(48, BLOCK_MAGIC);
    w.put(32, u64::from(crc));
    w.put(1, 0);
    w.put(24, origin as u64);
    let mut groups = 0u16;
    for g in 0..16 {
        if in_use[16 * g..16 * g + 16].iter().any(|&u| u) {
            groups |= 1 << (15 - g);
        }
    }
    w.put(16, u64::from(groups));
    for g in 0..16 {
        if groups >> (15 - g) & 1 == 1 {
            let mut m = 0u16;
            for k in 0..16 {
                if in_use[16 * g + k] {
                    m |= 1 << (15 - k);
                }
            }
            w.put(16, u64::from(m));
        }
    }
    let n_sel = syms.len().div_ceil(50);
    w.put(3, 2);
    w.put(15, n_sel as u64);
    for _ in 0..n_sel {
        w.put(1, 0);
    }
    for _ in 0..2 {
        let mut cur = lens[0];
        w.put(5, u64::from(cur));
        for &l in &lens {
            while cur < l {
                w.put(2, 0b10);
                cur += 1;
            }
            while cur > l {
                w.put(2, 0b11);
                cur -= 1;
            }
            w.put(1, 0);
        }
    }
    for &s in &syms {
        w.put(u32::from(lens[usize::from(s)]), u64::from(codes[usize::from(s)]));
    }
}

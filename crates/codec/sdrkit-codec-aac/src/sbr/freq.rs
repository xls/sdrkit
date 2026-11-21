//! The frequency band tables of SBR: the master table from the header, the high and low resolution tables, the noise floor
//! table, the patches of the HF generator and the limiter table.
//!
//! Standard: ISO/IEC 14496-3 clauses 4.6.18.3.2 (frequency tables), 4.6.18.6.3 (patches) and 4.6.18.7.3 (limiter bands).

use super::Header;

/// The offsets of the start band by `bs_start_freq`, by group of output rates (16 kHz, 22.05 kHz, 24 kHz, 32 kHz, 44.1 to
/// 64 kHz, 88.2 and 96 kHz, others), as printed in the PacketVideo AAC decoder (`get_sbr_startfreq.cpp`, Apache License 2.0).
#[rustfmt::skip]
static START_OFFSET: [[i8; 16]; 7] = [
    [-8, -7, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7],
    [-5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13],
    [-5, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16],
    [-6, -4, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16],
    [-4, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16, 20],
    [-2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16, 20, 24],
    [0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16, 20, 24, 28, 33],
];

/// The most bands of the master table.
const MAX_MASTER: usize = 64;

/// The tables derived from one header at one output rate.
#[derive(Clone, Debug)]
pub(crate) struct Freq {
    /// The first band of SBR.
    pub kx: usize,
    /// The number of SBR bands.
    pub m: usize,
    /// The band borders of high and low resolution: `n[r] + 1` entries each.
    pub table: [[u8; MAX_MASTER + 1]; 2],
    /// Bands of low (index 0) and high (index 1) resolution.
    pub n: [usize; 2],
    /// The noise floor band borders.
    pub noise: [u8; 6],
    /// Noise floor bands.
    pub n_q: usize,
    /// The limiter band borders.
    pub lim: [u8; 32],
    /// Limiter bands.
    pub n_lim: usize,
    /// The first source band and the width of each patch.
    pub patch: [(u8, u8); 6],
    /// Patches.
    pub n_patch: usize,
}

impl Default for Freq {
    fn default() -> Self {
        Freq {
            kx: 0,
            m: 0,
            table: [[0; MAX_MASTER + 1]; 2],
            n: [0; 2],
            noise: [0; 6],
            n_q: 0,
            lim: [0; 32],
            n_lim: 0,
            patch: [(0, 0); 6],
            n_patch: 0,
        }
    }
}

fn nint(x: f64) -> i32 {
    (x + 0.5).floor() as i32
}

/// The band widths of `n` bands from `start` to `stop` on a logarithmic scale, sorted.
fn log_widths(start: i32, stop: i32, n: usize, out: &mut [i32]) {
    let mut prev = start;
    for (i, d) in out.iter_mut().enumerate().take(n) {
        let cur = nint(
            f64::from(start) * (f64::from(stop) / f64::from(start)).powf((i + 1) as f64 / n as f64),
        );
        *d = cur - prev;
        prev = cur;
    }
    out[..n].sort_unstable();
}

impl Freq {
    /// The tables of `h` at the SBR rate `fs` in Hz. Returns `None` if the header sets tables outside the limits of the
    /// standard.
    pub(crate) fn new(h: &Header, fs: u32) -> Option<Freq> {
        let (k0, k2) = Self::range(h, fs);
        Self::build(h, fs, k0, k2)
    }

    /// The first and the end band of the master table (k0 and k2).
    fn range(h: &Header, fs: u32) -> (i32, i32) {
        let fsf = f64::from(fs);
        let (start_min, stop_min) = if fs < 32_000 {
            (nint(3000.0 * 128.0 / fsf), nint(6000.0 * 128.0 / fsf))
        } else if fs < 64_000 {
            (nint(4000.0 * 128.0 / fsf), nint(8000.0 * 128.0 / fsf))
        } else {
            (nint(5000.0 * 128.0 / fsf), nint(10000.0 * 128.0 / fsf))
        };
        let group = match fs {
            16_000 => 0,
            22_050 => 1,
            24_000 => 2,
            32_000 => 3,
            44_100..=64_000 => 4,
            88_200 | 96_000 => 5,
            _ => 6,
        };
        let k0 = start_min + i32::from(START_OFFSET[group][usize::from(h.start_freq)]);
        let k2 = match h.stop_freq {
            14 => (2 * k0).min(64),
            15 => (3 * k0).min(64),
            s => {
                let mut dk = [0i32; 13];
                let mut prev = stop_min;
                for (p, d) in dk.iter_mut().enumerate() {
                    let cur = nint(
                        f64::from(stop_min)
                            * (64.0 / f64::from(stop_min)).powf((p + 1) as f64 / 13.0),
                    );
                    *d = cur - prev;
                    prev = cur;
                }
                dk.sort_unstable();
                (stop_min + dk[..usize::from(s)].iter().sum::<i32>()).min(64)
            }
        };
        (k0, k2)
    }

    fn build(h: &Header, fs: u32, k0: i32, k2: i32) -> Option<Freq> {
        if k0 <= 0 || k2 <= k0 {
            return None;
        }
        let mut master = [0i32; MAX_MASTER + 1];
        let n_master = if h.freq_scale == 0 {
            let (dk, n) = if h.alter_scale {
                (2, ((k2 - k0) >> 1) as usize)
            } else {
                (1, ((k2 - k0) & !1) as usize)
            };
            if n == 0 || n > MAX_MASTER {
                return None;
            }
            let mut widths = [dk; MAX_MASTER];
            let mut diff = k2 - (k0 + n as i32 * dk);
            let (incr, mut k): (i32, i32) = if diff < 0 { (1, 0) } else { (-1, n as i32 - 1) };
            while diff != 0 && (0..n as i32).contains(&k) {
                widths[k as usize] -= incr;
                k += incr;
                diff += incr;
            }
            master[0] = k0;
            for i in 0..n {
                master[i + 1] = master[i] + widths[i];
            }
            n
        } else {
            let bands = [12.0, 10.0, 8.0][usize::from(h.freq_scale) - 1];
            let warp = if h.alter_scale { 1.3 } else { 1.0 };
            let two = f64::from(k2) / f64::from(k0) > 2.2449;
            let k1 = if two { 2 * k0 } else { k2 };
            let n0 = 2 * nint(bands * (f64::from(k1) / f64::from(k0)).log2() / 2.0) as usize;
            if n0 == 0 || n0 > MAX_MASTER {
                return None;
            }
            let mut d0 = [0i32; MAX_MASTER];
            log_widths(k0, k1, n0, &mut d0);
            master[0] = k0;
            for i in 0..n0 {
                master[i + 1] = master[i] + d0[i];
            }
            let mut n = n0;
            if two {
                let n1 = 2 * nint(bands * (f64::from(k2) / f64::from(k1)).log2() / (2.0 * warp))
                    as usize;
                if n1 == 0 || n0 + n1 > MAX_MASTER {
                    return None;
                }
                let mut d1 = [0i32; MAX_MASTER];
                log_widths(k1, k2, n1, &mut d1);
                if d0[n0 - 1] > d1[0] {
                    let change = (d0[n0 - 1] - d1[0]).min((d1[n1 - 1] - d1[0]) >> 1);
                    d1[0] += change;
                    d1[n1 - 1] -= change;
                    d1[..n1].sort_unstable();
                }
                for i in 0..n1 {
                    master[n + i + 1] = master[n + i] + d1[i];
                }
                n += n1;
            }
            n
        };
        if master[..=n_master].windows(2).any(|w| w[1] <= w[0]) {
            return None;
        }
        let xover = usize::from(h.xover_band);
        if xover >= n_master {
            return None;
        }
        let mut f = Freq::default();
        let n_high = n_master - xover;
        let n_low = n_high.div_ceil(2);
        for i in 0..=n_high {
            f.table[1][i] = master[xover + i] as u8;
        }
        for i in 0..=n_low {
            let j = if i == 0 { 0 } else { 2 * i - (n_high % 2) };
            f.table[0][i] = f.table[1][j];
        }
        f.n = [n_low, n_high];
        f.kx = usize::from(f.table[1][0]);
        f.m = usize::from(f.table[1][n_high]) - f.kx;
        if f.kx > 32 || f.kx + f.m > 64 || f.m == 0 {
            return None;
        }
        // The noise floor bands.
        f.n_q = if h.noise_bands == 0 {
            1
        } else {
            (nint(f64::from(h.noise_bands) * (f64::from(k2) / f.kx as f64).log2()).max(1) as usize)
                .min(5)
        };
        let mut ik = 0usize;
        f.noise[0] = f.table[0][0];
        for k in 1..=f.n_q {
            ik += (n_low - ik) / (f.n_q + 1 - k);
            f.noise[k] = f.table[0][ik];
        }
        f.patches(&master[..=n_master], k0, fs)?;
        f.limiter(h.limiter_bands);
        Some(f)
    }

    /// Builds the patches of the HF generator (clause 4.6.18.6.3).
    fn patches(&mut self, master: &[i32], k0: i32, fs: u32) -> Option<()> {
        let n_master = master.len() - 1;
        let kx = self.kx as i32;
        let end = kx + self.m as i32;
        let goal = nint(2.048e6 / f64::from(fs));
        let mut k = if goal < end {
            master.iter().position(|&f| f >= goal).unwrap_or(n_master)
        } else {
            n_master
        };
        let (mut msb, mut usb) = (k0, kx);
        let mut n = 0;
        let mut done = false;
        for _ in 0..MAX_MASTER {
            let mut j = k + 1;
            let (mut sb, mut odd);
            loop {
                j -= 1;
                sb = master[j];
                odd = (sb - 2 + k0).rem_euclid(2);
                if sb <= k0 - 1 + msb - odd || j == 0 {
                    break;
                }
            }
            let width = (sb - usb).max(0);
            if width > 0 {
                if n == 5 {
                    return None;
                }
                let start = k0 - odd - width;
                if start < 0 {
                    return None;
                }
                self.patch[n] = (start as u8, width as u8);
                usb = sb;
                msb = sb;
                n += 1;
            } else {
                msb = kx;
            }
            if master[k] - sb < 3 {
                k = n_master;
            }
            if sb == end {
                done = true;
                break;
            }
        }
        if !done {
            return None;
        }
        if n > 1 && self.patch[n - 1].1 < 3 {
            n -= 1;
        }
        self.n_patch = n;
        (n > 0).then_some(())
    }

    /// Builds the limiter band table (clause 4.6.18.7.3).
    fn limiter(&mut self, bands: u8) {
        let low = &self.table[0][..=self.n[0]];
        if bands == 0 {
            self.lim[0] = low[0];
            self.lim[1] = low[self.n[0]];
            self.n_lim = 1;
            return;
        }
        let per_octave = [1.2, 2.0, 3.0][usize::from(bands) - 1];
        let mut borders = [0u8; 6];
        borders[0] = self.kx as u8;
        for i in 0..self.n_patch {
            borders[i + 1] = borders[i] + self.patch[i].1;
        }
        let inner = &borders[1..self.n_patch];
        let mut t = [0u8; 32];
        let mut n = 0;
        for &v in low.iter().chain(inner) {
            t[n] = v;
            n += 1;
        }
        t[..n].sort_unstable();
        let mut last = n - 1;
        let mut k = 1;
        while k <= last {
            let octaves = (f64::from(t[k]) / f64::from(t[k - 1])).log2();
            if octaves * per_octave < 0.49 {
                if t[k] == t[k - 1] || !inner.contains(&t[k]) {
                    t.copy_within(k + 1..=last, k);
                } else if inner.contains(&t[k - 1]) {
                    k += 1;
                    continue;
                } else {
                    t.copy_within(k..=last, k - 1);
                }
                last -= 1;
                continue;
            }
            k += 1;
        }
        self.lim[..=last].copy_from_slice(&t[..=last]);
        self.n_lim = last;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_bands_of_the_formula_match_the_printed_tables() {
        // k2 by bs_stop_freq 0 to 12 at 48, 44.1, 32, 22.05 and 16 kHz, as printed in the PacketVideo decoder
        // (`get_sbr_stopfreq.cpp`).
        let printed: [(u32, [i32; 13]); 5] = [
            (48_000, [21, 23, 25, 27, 29, 32, 35, 38, 41, 45, 49, 54, 59]),
            (44_100, [23, 25, 27, 29, 31, 34, 37, 40, 43, 47, 51, 55, 59]),
            (32_000, [32, 34, 36, 38, 40, 42, 44, 46, 49, 52, 55, 58, 61]),
            (22_050, [35, 36, 38, 40, 42, 44, 46, 48, 50, 52, 55, 58, 61]),
            (16_000, [48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 60, 62]),
        ];
        for (fs, k2s) in printed {
            for (s, &k2) in k2s.iter().enumerate() {
                let h = Header {
                    stop_freq: s as u8,
                    ..Header::default()
                };
                assert_eq!(Freq::range(&h, fs).1, k2, "{fs} {s}");
            }
        }
    }

    #[test]
    fn a_typical_header_returns_consistent_tables() {
        let h = Header {
            start_freq: 5,
            stop_freq: 9,
            ..Header::default()
        };
        let f = Freq::new(&h, 48_000).unwrap();
        assert!(f.kx < 32 && f.kx + f.m <= 64);
        assert_eq!(f.table[0][0], f.table[1][0]);
        assert_eq!(f.table[0][f.n[0]], f.table[1][f.n[1]]);
        assert_eq!(f.noise[f.n_q], f.table[0][f.n[0]]);
        let width: usize = f.patch[..f.n_patch].iter().map(|p| usize::from(p.1)).sum();
        assert_eq!(width, f.m);
        assert_eq!(usize::from(f.lim[0]), f.kx);
        assert_eq!(usize::from(f.lim[f.n_lim]), f.kx + f.m);
    }
}

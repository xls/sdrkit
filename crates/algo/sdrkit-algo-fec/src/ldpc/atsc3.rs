//! The ATSC 3.0 LDPC codes: type A and type B codes as quasi-cyclic layers of 360 checks, and the type A encoder.
//!
//! Standard: ATSC A/322:2026-06 clause 6.1.3 and Annex A.

use super::tables::atsc3::{Atsc3Table, TypeATable, ALL, BCH_PARITY_16K, BCH_PARITY_64K, BCH_T};
use super::tables::dvb_bch::{S2_NORMAL_MINPOLYS, S2_SHORT_MINPOLYS};
use super::{Edge, InfoAt, LdpcCode, NO_MASK};
use crate::bch::Bch;
use crate::error::{FecError, Param, Result};

impl Atsc3Table {
    /// The table of the code of length `n_inner` (16200 or 64800) and rate `rate_fifteenths / 15` (2 to 13), or `None`.
    pub fn find(n_inner: usize, rate_fifteenths: u8) -> Option<Atsc3Table> {
        ALL.iter()
            .find(|(n, r, _)| *n as usize == n_inner && *r == rate_fifteenths)
            .map(|(_, _, t)| *t)
    }

    /// Code word length `N_inner`.
    pub fn n(&self) -> usize {
        match self {
            Atsc3Table::A(t) => t.n as usize,
            Atsc3Table::B(t) => t.n as usize,
        }
    }

    /// Information bits `N_outer` of the LDPC code: the BCH code word, or the payload and its CRC.
    pub fn k(&self) -> usize {
        match self {
            Atsc3Table::A(t) => t.k as usize,
            Atsc3Table::B(t) => t.k as usize,
        }
    }

    /// The BCH outer code of this length (A/322 clause 6.1.2.1): `N_outer` bits, of which `M_outer` = 192 (64800) or 168
    /// (16200) are parity, correcting 12 errors. Its factors `g1` to `g12` (table 6.3) are those of EN 302 307-1 tables 6a
    /// and 6b, so the field is built on their `g1`.
    pub fn bch(&self) -> Result<Bch> {
        let (m, poly, parity) = if self.n() == 64800 {
            (16, S2_NORMAL_MINPOLYS[0], BCH_PARITY_64K)
        } else {
            (14, S2_SHORT_MINPOLYS[0], BCH_PARITY_16K)
        };
        let bch = Bch::new(m, poly, self.k(), BCH_T as usize)?;
        if bch.n() - bch.k() != parity as usize {
            return Param::Parity.conflict(Param::Table);
        }
        Ok(bch)
    }
}

impl TypeATable {
    /// Makes sure that the sizes agree (`m1` and `m2 = n − k − m1` are multiples of 360, `(k + m1) / 360` rows) and that
    /// every row of the parity part only reaches the identity part. Returns `(Q1, Q2)`.
    fn check(&self) -> Result<(usize, usize)> {
        let (n, k, m1) = (self.n as usize, self.k as usize, self.m1 as usize);
        if k == 0
            || k % 360 != 0
            || m1 == 0
            || m1 % 360 != 0
            || n <= k + m1
            || (n - k - m1) % 360 != 0
        {
            return Param::Table.conflict(Param::K);
        }
        if self.rows.len() != (k + m1) / 360 {
            return Param::Rows.conflict(Param::K);
        }
        for (g, row) in self.rows.iter().enumerate() {
            let floor = if g < k / 360 { 0 } else { m1 };
            if row
                .iter()
                .any(|&x| (x as usize) < floor || x as usize >= n - k)
            {
                return Param::Table.degenerate();
            }
        }
        Ok((m1 / 360, (n - k - m1) / 360))
    }

    /// Encodes `info` (`k` bits, one per byte) into `out` (`n` bits) by the procedure of A/322 clause 6.1.3.1 steps i to viii.
    /// The output is the code word `Λ` with the parity bits already in the order of steps vi and viii, which is the order
    /// [`LdpcCode::atsc3`] decodes. An error when a length is wrong or the table is inconsistent.
    pub fn encode(&self, info: &[u8], out: &mut [u8]) -> Result<()> {
        let (q1, q2) = self.check()?;
        let (n, k, m1) = (self.n as usize, self.k as usize, self.m1 as usize);
        if info.len() != k {
            return Err(FecError::InputLength {
                expected: k,
                got: info.len(),
            });
        }
        if out.len() != n {
            return Err(FecError::OutputTooShort { need: n });
        }
        let m2 = n - k - m1;
        // Parity bit p_j is at code word position k + pos(j): steps vi and viii put p_(Q s + t) at group t, lane s.
        let pos = |j: usize| {
            if j < m1 {
                (j % q1) * 360 + j / q1
            } else {
                let j = j - m1;
                m1 + (j % q2) * 360 + j / q2
            }
        };
        let addr = |x: usize, m: usize| {
            if x < m1 {
                (x + m * q1) % m1
            } else {
                m1 + (x - m1 + m * q2) % m2
            }
        };
        out[..k].iter_mut().zip(info).for_each(|(o, &b)| *o = b & 1);
        let (head, p) = out.split_at_mut(k);
        p.fill(0);
        // Steps ii to iv: the information bits.
        for (g, row) in self.rows[..k / 360].iter().enumerate() {
            for m in 0..360 {
                if head[g * 360 + m] == 1 {
                    for &x in row.iter() {
                        p[pos(addr(x as usize, m))] ^= 1;
                    }
                }
            }
        }
        // Step v: the accumulator of the dual-diagonal part.
        for i in 1..m1 {
            let prev = p[pos(i - 1)];
            p[pos(i)] ^= prev;
        }
        // Step vii: the dual-diagonal parity bits, in code word order, feed the identity part.
        for (t, row) in self.rows[k / 360..].iter().enumerate() {
            for m in 0..360 {
                if p[t * 360 + m] == 1 {
                    for &x in row.iter() {
                        p[pos(addr(x as usize, m))] ^= 1;
                    }
                }
            }
        }
        Ok(())
    }
}

impl LdpcCode {
    /// The ATSC 3.0 LDPC code of length `n_inner` (16200 or 64800) and rate `rate_fifteenths / 15` (2 to 13), from A/322
    /// Annex A. Type B codes are built by [`LdpcCode::from_dvb`] and type A codes by [`LdpcCode::from_atsc3_type_a`].
    ///
    /// The code word order is `Λ` of clause 6.1.3, the encoder output before the bit interleaver. For a type B code the
    /// receiver must undo the parity interleaver of clause 6.2.1 after the group-wise de-interleaver. A type A code has no
    /// parity interleaver, so the group-wise de-interleaver output is already `Λ`. An error when no code has this length
    /// and rate.
    ///
    /// ```
    /// use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
    /// let code = LdpcCode::atsc3(16200, 10).unwrap();
    /// assert_eq!((code.n(), code.k()), (16200, 10800));
    /// let info: Vec<u8> = (0..code.k()).map(|i| (i % 5 == 2) as u8).collect();
    /// let mut cw = vec![0u8; code.n()];
    /// code.encode(&info, &mut cw).unwrap();
    /// let mut llr: Vec<i8> = cw.iter().map(|&b| if b == 0 { 12 } else { -12 }).collect();
    /// for i in (0..llr.len()).step_by(400) {
    ///     llr[i] = -llr[i];
    /// }
    /// let mut out = vec![0u8; code.n()];
    /// LdpcDecoder::new(&code, MinSum::default()).decode(&llr, 50, &mut out).unwrap();
    /// assert_eq!(out, cw);
    /// ```
    pub fn atsc3(n_inner: usize, rate_fifteenths: u8) -> Result<Self> {
        let t = Atsc3Table::find(n_inner, rate_fifteenths).ok_or(FecError::Conflict {
            param: Param::N,
            with: Param::Rate,
        })?;
        Self::from_atsc3(t)
    }

    /// The ATSC 3.0 LDPC code of a table: [`LdpcCode::from_dvb`] for type B, [`LdpcCode::from_atsc3_type_a`] for type A.
    /// The code word order is as [`LdpcCode::atsc3`]. An error when the table is inconsistent.
    pub fn from_atsc3(t: Atsc3Table) -> Result<Self> {
        match t {
            Atsc3Table::A(t) => Self::from_atsc3_type_a(t),
            Atsc3Table::B(t) => Self::from_dvb(t),
        }
    }

    /// An ATSC 3.0 type A code from its table (A/322 clause 6.1.3.1). Check `i = Q1 b + a` of the dual-diagonal part is lane
    /// `b` of layer `a`, and check `M1 + Q2 b + a` of the identity part is lane `b` of layer `Q1 + a`. The code word order is
    /// `Λ`, which already holds each parity part as groups of 360 bits, so no permutation is needed.
    ///
    /// [`LdpcCode::encode`] is not available for these codes ([`LdpcCode::can_encode`] is false). Encode with
    /// [`TypeATable::encode`] or with the general [`LdpcEncoder`](super::LdpcEncoder). An error when the table is
    /// inconsistent.
    pub fn from_atsc3_type_a(t: &'static TypeATable) -> Result<Self> {
        let (q1, q2) = t.check()?;
        let (n, k, m1) = (t.n as usize, t.k as usize, t.m1 as usize);
        let ninfo = k / 360;
        let mut layers: Vec<Vec<Edge>> = vec![Vec::new(); q1 + q2];
        // Row g describes column group g: information groups, then the dual-diagonal parity groups.
        for (g, row) in t.rows.iter().enumerate() {
            for &x in row.iter() {
                let x = x as usize;
                let (layer, x1) = if x < m1 {
                    (x % q1, x / q1)
                } else {
                    (q1 + (x - m1) % q2, (x - m1) / q2)
                };
                layers[layer].push(Edge {
                    col: g as u32,
                    shift: ((360 - x1) % 360) as u16,
                    mask: NO_MASK,
                });
            }
        }
        for (a, layer) in layers.iter_mut().enumerate() {
            if a < q1 {
                // p_i and p_(i−1); the first check of all has no p_(−1).
                layer.push(Edge {
                    col: (ninfo + a) as u32,
                    shift: 0,
                    mask: NO_MASK,
                });
                layer.push(if a >= 1 {
                    Edge {
                        col: (ninfo + a - 1) as u32,
                        shift: 0,
                        mask: NO_MASK,
                    }
                } else {
                    Edge {
                        col: (ninfo + q1 - 1) as u32,
                        shift: 359,
                        mask: 0,
                    }
                });
            } else {
                layer.push(Edge {
                    col: (ninfo + a) as u32,
                    shift: 0,
                    mask: NO_MASK,
                });
            }
        }
        Self::assemble(
            t.name.to_string(),
            n,
            k,
            360,
            n / 360,
            layers,
            Vec::new(),
            InfoAt::Start,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::tables::atsc3::*;
    use crate::ldpc::{LdpcDecoder, LdpcEncoder, MinSum};

    struct Rng(u64);
    impl Rng {
        fn bit(&mut self) -> u8 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as u8 & 1
        }
    }

    /// A/322 table 6.4: rates 2/15 to 5/15 are type A at both lengths, and 7/15 at 64800; all others are type B.
    #[test]
    fn code_types_follow_table_6_4() {
        assert_eq!(ALL.len(), 24);
        for &(n, r, t) in ALL {
            let type_a = (2..=5).contains(&r) || (n == 64800 && r == 7);
            assert_eq!(matches!(t, Atsc3Table::A(_)), type_a, "{n} {r}");
        }
        assert!(Atsc3Table::find(16200, 14).is_none() && Atsc3Table::find(32400, 6).is_none());
    }

    /// K = N r / 15, M1 and M2 of tables 6.5 and 6.6, Q_ldpc of table 6.7, row counts and address ranges of clause 6.1.3.
    #[test]
    fn tables_agree_with_tables_6_5_to_6_7() {
        let m12 = [
            (64800, 2, 1800, 54360),
            (64800, 3, 1800, 50040),
            (64800, 4, 1800, 45720),
            (64800, 5, 1440, 41760),
            (64800, 7, 1080, 33480),
            (16200, 2, 3240, 10800),
            (16200, 3, 1080, 11880),
            (16200, 4, 1080, 10800),
            (16200, 5, 720, 10080),
        ];
        let q = [
            (64800, [108, 0, 84, 72, 60, 48, 36, 24]),
            (16200, [27, 24, 21, 18, 15, 12, 9, 6]),
        ];
        for &(n, r, t) in ALL {
            match t {
                Atsc3Table::A(t) => {
                    let &(_, _, m1, m2) = m12.iter().find(|e| e.0 == n && e.1 == r).unwrap();
                    assert_eq!((t.n, t.k, t.m1), (n, n * r as u32 / 15, m1));
                    assert_eq!(t.n - t.k - t.m1, m2);
                    assert!(t.check().is_ok(), "{}", t.name);
                }
                Atsc3Table::B(t) => {
                    let qs = q.iter().find(|e| e.0 == n).unwrap().1;
                    assert_eq!((t.n, t.k, t.q), (n, n * r as u32 / 15, qs[r as usize - 6]));
                    assert_eq!(t.rows.len() as u32, t.k / 360);
                    assert!(t
                        .rows
                        .iter()
                        .all(|row| row.iter().all(|&x| (x as u32) < t.n - t.k)));
                }
            }
        }
        // Annex A table A.1.1 (64800, 2/15): first and last rows as printed.
        assert_eq!(A_64K_2_15.rows[0][..4], [615, 898, 1029, 6129]);
        assert_eq!(A_64K_2_15.rows[28][16], 55973);
    }

    /// The A/322 procedure of clause 6.1.3.1 produces words that every check of the quasi-cyclic layout accepts, the edge count is
    /// the table's, and the general sparse encoder produces the same words.
    #[test]
    fn type_a_encoder_satisfies_every_check() {
        let mut rng = Rng(0x1234_5678_9ABC_DEF1);
        for &(_, _, t) in ALL {
            let Atsc3Table::A(t) = t else { continue };
            let code = LdpcCode::from_atsc3_type_a(t).unwrap();
            let (n, k, m1) = (t.n as usize, t.k as usize, t.m1 as usize);
            assert_eq!(code.checks(), n - k);
            let ones: usize = t.rows.iter().map(|r| r.len()).sum::<usize>() * 360;
            assert_eq!(code.edges(), ones + 2 * m1 - 1 + (n - k - m1), "{}", t.name);
            let info: Vec<u8> = (0..k).map(|_| rng.bit()).collect();
            let mut cw = vec![0u8; n];
            t.encode(&info, &mut cw).unwrap();
            assert!(code.is_codeword(&cw), "{}", t.name);
            assert_eq!(cw[..k], info[..]);
            let mut bad = cw.clone();
            bad[k + 7] ^= 1;
            assert!(!code.is_codeword(&bad));
            let mut cw2 = vec![0u8; n];
            LdpcEncoder::new(&code)
                .unwrap()
                .encode(&info, &mut cw2)
                .unwrap();
            assert_eq!(cw2, cw, "{}", t.name);
        }
    }

    /// Clause 6.1.3.1 step v: a single information bit produces a run of ones in the dual-diagonal part from its first address on.
    #[test]
    fn type_a_unit_vector_follows_the_accumulator() {
        let t = &A_16K_2_15;
        let (k, m1, q1) = (t.k as usize, t.m1 as usize, t.m1 as usize / 360);
        let mut info = vec![0u8; k];
        info[0] = 1;
        let mut cw = vec![0u8; t.n as usize];
        t.encode(&info, &mut cw).unwrap();
        let first = *t.rows[0]
            .iter()
            .filter(|&&x| (x as usize) < m1)
            .min()
            .unwrap() as usize;
        // p_j is at k + (j mod Q1) 360 + j / Q1; the row's addresses below M1 each toggle the run from there on.
        let below: Vec<usize> = t.rows[0]
            .iter()
            .map(|&x| x as usize)
            .filter(|&x| x < m1)
            .collect();
        for j in 0..m1 {
            let want = below.iter().filter(|&&x| x <= j).count() as u8 & 1;
            assert_eq!(cw[k + (j % q1) * 360 + j / q1], want, "p_{j}");
        }
        assert_eq!(cw[k + (first % q1) * 360 + first / q1], 1);
    }

    #[test]
    fn every_code_decodes_with_errors() {
        let mut rng = Rng(77);
        for &(n, r, _) in ALL {
            let code = LdpcCode::atsc3(n as usize, r).unwrap();
            let info: Vec<u8> = (0..code.k()).map(|_| rng.bit()).collect();
            let mut cw = vec![0u8; code.n()];
            match Atsc3Table::find(n as usize, r).unwrap() {
                Atsc3Table::A(t) => t.encode(&info, &mut cw).unwrap(),
                Atsc3Table::B(_) => code.encode(&info, &mut cw).unwrap(),
            }
            assert!(code.is_codeword(&cw), "{}", code.name());
            // One wrong bit in 200 at full strength, the others at a modest level.
            let mut llr: Vec<i8> = cw.iter().map(|&b| if b == 0 { 10 } else { -10 }).collect();
            let mut flipped = 0;
            for i in (0..llr.len()).step_by(200) {
                llr[i] = -llr[i];
                flipped += 1;
            }
            let mut out = vec![0u8; code.n()];
            let d = LdpcDecoder::new(&code, MinSum::default())
                .decode(&llr, 50, &mut out)
                .unwrap_or_else(|e| panic!("{}: {e:?}", code.name()));
            assert_eq!(out, cw, "{}", code.name());
            assert_eq!(d.corrected, flipped);
        }
    }

    /// A/322 tables 6.1 and 6.2: K_payload = N_outer − 192 (64800) or N_outer − 168 (16200). Table 6.3: the generator is
    /// g1(x) g2(x) ... g12(x), the polynomials of EN 302 307-1 tables 6a and 6b.
    #[test]
    fn bch_outer_code_follows_tables_6_1_to_6_3() {
        fn mul(a: &[u64], b: u32) -> Vec<u64> {
            let mut out = vec![0u64; a.len() + 1];
            for i in 0..32 {
                if b >> i & 1 == 1 {
                    for (w, &x) in a.iter().enumerate() {
                        out[w] ^= x << i;
                        if i > 0 {
                            out[w + 1] ^= x >> (64 - i);
                        }
                    }
                }
            }
            while out.len() > 1 && *out.last().unwrap() == 0 {
                out.pop();
            }
            out
        }
        for &(n, r, t) in ALL {
            let nouter = (n * r as u32 / 15) as usize;
            let bch = t.bch().unwrap();
            let (parity, polys) = if n == 64800 {
                (192, &S2_NORMAL_MINPOLYS)
            } else {
                (168, &S2_SHORT_MINPOLYS)
            };
            assert_eq!(
                (bch.n(), bch.k(), bch.t()),
                (nouter, nouter - parity, 12),
                "{n} {r}"
            );
            let g = polys.iter().fold(vec![1u64], |g, &p| mul(&g, p));
            assert_eq!(bch.generator(), &g[..], "{n} {r}");
        }
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(LdpcCode::atsc3(16200, 1).is_err());
        let mut out = vec![0u8; 16200];
        assert!(A_16K_3_15.encode(&[0u8; 10], &mut out).is_err());
        assert!(A_16K_3_15.encode(&[0u8; 3240], &mut out[..100]).is_err());
        static BAD: TypeATable = TypeATable {
            name: "bad",
            n: 16200,
            k: 3240,
            m1: 1080,
            rows: &[&[1]],
        };
        assert!(LdpcCode::from_atsc3_type_a(&BAD).is_err());
        assert!(BAD.encode(&[0u8; 3240], &mut out).is_err());
    }
}

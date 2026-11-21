//! One FEC block of the BGAN forward link (ETSI TS 102 744-2-1 clauses 5.3.7 and 5.3.8): scrambling, the turbo code of
//! `sdrkit-algo-fec` with the interleaver of Annex C.1, and the puncturing and symbol mapping of Annex C.2, in both directions.

use super::tables::{tables, CodeTable, Modulation};
use sdrkit_algo_fec::turbo::{Slot, TurboDecoder};
use sdrkit_algo_fec::turbo_codes::{sl_code, sl_map};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq;
use std::sync::OnceLock;

/// The most information bits of a forward block (F80T5X64 H6, N = 5284).
pub const MAX_K: usize = 5280;

/// The scrambler bits of one FEC block (restarted at each block), at least [`MAX_K`] of them.
pub fn scrambler_bits() -> &'static [u8] {
    static S: OnceLock<Vec<u8>> = OnceLock::new();
    S.get_or_init(|| {
        let mut l: Lfsr = seq::BGAN;
        (0..MAX_K).map(|_| l.bit()).collect()
    })
}

/// How the columns of the 38 tables labelled `I1 I0 Q1 Q0` and `I0 Q0` map to the bits of the symbol value. No public
/// document states it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IqLabels {
    /// In column order: `I1 I0 Q1 Q0` are `b3 b2 b1 b0`, `I0 Q0` are `b1 b0`.
    #[default]
    InOrder,
    /// Swapped within each axis pair: `I1 I0 Q1 Q0` are `b2 b3 b0 b1`, `I0 Q0` are `b0 b1`.
    Swapped,
}

/// The bit of the symbol value (counted from the most significant, 0 = `b[n-1]`) that column `j` of `t` carries.
pub fn column_bit(t: &CodeTable, j: usize, labels: IqLabels) -> usize {
    if t.b_labelled || labels == IqLabels::InOrder {
        j
    } else {
        j ^ 1
    }
}

/// The modulation of a table by its bits per symbol.
pub fn modulation_of(t: &CodeTable) -> Modulation {
    match t.bits {
        2 => Modulation::Qpsk,
        4 => Modulation::Qam16,
        5 => Modulation::Qam32,
        _ => Modulation::Qam64,
    }
}

/// Encodes the `K / 8` octets `data` of a block with table `t`: scrambles them (bit 8 of octet 1 first), turbo encodes and maps
/// them. Returns the symbol values, `t.symbols` of them.
pub fn encode_block(t: &CodeTable, data: &[u8], labels: IqLabels) -> Vec<u16> {
    let k = t.k();
    let n = t.n;
    let scr = scrambler_bits();
    let bits: Vec<u8> = (0..k)
        .map(|i| {
            let b = data.get(i / 8).map_or(0, |o| o >> (7 - i % 8) & 1);
            b ^ scr[i]
        })
        .collect();
    let mut raw = vec![0u8; 3 * n];
    let Ok(code) = sl_code(&t.tci) else {
        return vec![0; t.symbols];
    };
    let _ = code.encode_each(&bits, |stream, period, b| match stream {
        0 => raw[period] = b,
        1 => raw[n + period] = b,
        3 => raw[2 * n + period] = b,
        _ => {}
    });
    let nb = t.bits;
    (0..t.symbols)
        .map(|s| {
            let mut v = 0u16;
            for j in 0..nb {
                let b = raw[t.cipm[s * nb + j] as usize];
                v |= u16::from(b) << (nb - 1 - column_bit(t, j, labels));
            }
            v
        })
        .collect()
}

/// A decoder of the blocks of one table.
pub struct BlockDecoder {
    table: &'static CodeTable,
    map: Vec<Slot>,
    dec: TurboDecoder,
    labels: IqLabels,
    constellation: Constellation,
    llr: Vec<f32>,
    q: Vec<i8>,
    hard: Vec<u8>,
    octets: Vec<u8>,
    /// Turbo iterations of the last decode.
    pub iterations: u32,
}

impl BlockDecoder {
    /// The decoder of table `index` of [`tables`], or `None` for a bad index.
    pub fn new(index: usize, labels: IqLabels) -> Option<Self> {
        let table = tables().get(index)?;
        let map = sl_map(&table.cipm, table.n).ok()?;
        let dec = TurboDecoder::new(sl_code(&table.tci).ok()?);
        let set = sdrkit_algo_dsp::constellation::PointSet::new(
            modulation_of(table).points(),
            sdrkit_algo_dsp::constellation::Shape::Other,
        )
        .ok()?;
        let constellation = Constellation::new(set).ok()?;
        Some(BlockDecoder {
            table,
            map,
            dec,
            labels,
            constellation,
            llr: vec![0.0; table.symbols * table.bits],
            q: vec![0; table.symbols * table.bits],
            hard: vec![0; table.k()],
            octets: vec![0; table.k() / 8],
            iterations: 0,
        })
    }

    /// The table.
    pub fn table(&self) -> &'static CodeTable {
        self.table
    }

    /// Demaps the equalised symbol `z` (symbol `s` of the block) with weight `w = |h|^2 / noise` into the block's LLRs.
    pub fn put(&mut self, s: usize, z: Complex32, w: f32) {
        let nb = self.table.bits;
        let mut l = [0f32; 6];
        if self
            .constellation
            .demap_weighted(z, w, Demap::MaxLog, &mut l[..nb])
            .is_err()
        {
            l = [0.0; 6];
        }
        for j in 0..nb {
            self.llr[s * nb + j] = l[column_bit(self.table, j, self.labels)];
        }
    }

    /// Turbo decodes the symbols put since the last call with at most `iters` iterations, descrambles them and returns the
    /// octets when `check` accepts them (a PDU with a good CRC, or zero fill), else `None`.
    pub fn decode(&mut self, iters: u32, check: impl Fn(&[u8]) -> bool) -> Option<&[u8]> {
        let mean = self.llr.iter().map(|x| x.abs()).sum::<f32>() / self.llr.len().max(1) as f32;
        let scale = if mean > 0.0 { 40.0 / mean } else { 1.0 };
        for (q, &l) in self.q.iter_mut().zip(&self.llr) {
            *q = (l * scale).round().clamp(-127.0, 127.0) as i8;
        }
        self.llr.fill(0.0);
        if self.dec.load(&self.map, &self.q).is_err() {
            return None;
        }
        let scr = scrambler_bits();
        let octets = &mut self.octets;
        let r = self.dec.decode(
            iters,
            |bits| {
                pack(bits, scr, octets);
                check(octets)
            },
            &mut self.hard,
        );
        match r {
            Ok(d) => {
                self.iterations = d.iterations;
                pack(&self.hard, scr, &mut self.octets);
                Some(&self.octets)
            }
            Err(_) => {
                self.iterations = iters;
                None
            }
        }
    }
}

/// Descrambles `bits` with `scr` into octets, bit 8 first.
fn pack(bits: &[u8], scr: &[u8], out: &mut [u8]) {
    for (o, (c, s)) in out
        .iter_mut()
        .zip(bits.as_chunks::<8>().0.iter().zip(scr.as_chunks::<8>().0))
    {
        *o = c
            .iter()
            .zip(s)
            .fold(0u8, |a, (&b, &x)| (a << 1) | ((b ^ x) & 1));
    }
}

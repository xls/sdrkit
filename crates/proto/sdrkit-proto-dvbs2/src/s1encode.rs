//! The DVB-S transmit coding of transport packets into QPSK symbols: energy dispersal with the inverted sync byte at the
//! start of each group of eight packets, Reed-Solomon (204,188, t = 8), convolutional interleaving (I = 12), the K = 7
//! convolutional code punctured to the code rate, and Gray-coded QPSK. The pulse shaping is [`crate::transmit::TxShaper`].
//!
//! Standard: ETSI EN 300 421 clauses 4.4 and 4.5.

use crate::inner::{encoder, PunctureRate};
use sdrkit_algo_fec::conv::{puncture, Encoder};
use sdrkit_core::Complex32;
use sdrkit_proto_mpegts::outer::{Dispersal, OuterEncoder, RS_N, TS};

/// The QPSK symbol of the bits `i` and `q` (EN 300 421 clause 4.5): 0 maps to +1/√2, 1 to −1/√2 on each axis.
pub fn qpsk(i: u8, q: u8) -> Complex32 {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(if i == 0 { s } else { -s }, if q == 0 { s } else { -s })
}

/// The useful bit rate of DVB-S at `symbol_rate` symbols per second and code rate `rate`: the transport stream rate,
/// `symbol_rate × 2 × rate × 188 / 204` bit/s (EN 300 421).
pub fn useful_bit_rate(symbol_rate: f64, rate: PunctureRate) -> f64 {
    symbol_rate * 2.0 * rate.value() * TS as f64 / RS_N as f64
}

/// The coder of one DVB-S stream, from the start of a dispersal group. It allocates nothing once its buffers have grown to
/// one packet.
#[derive(Clone, Debug)]
pub struct S1Encoder {
    rate: PunctureRate,
    outer: OuterEncoder,
    conv: Encoder,
    coded: Vec<u8>,
    coded_n: usize,
    bits: Vec<u8>,
}

impl S1Encoder {
    /// A coder at code rate `rate`.
    pub fn new(rate: PunctureRate) -> Self {
        S1Encoder {
            rate,
            outer: OuterEncoder::new(Dispersal::DvbGroups),
            conv: encoder(),
            coded: Vec::with_capacity(2 * 8 * RS_N),
            coded_n: 0,
            bits: Vec::with_capacity(2 * 8 * RS_N + 1),
        }
    }

    /// The code rate.
    pub fn rate(&self) -> PunctureRate {
        self.rate
    }

    /// Codes `packet` (its sync byte is ignored) and writes its QPSK symbols into `syms`, which is cleared first. An odd
    /// bit left over waits for the next packet, so the count of symbols per packet varies by one at most.
    pub fn packet(&mut self, packet: &[u8; TS], syms: &mut Vec<Complex32>) {
        let mut w = [0u8; RS_N];
        self.outer.encode(packet, &mut w);
        self.coded.clear();
        for &b in &w {
            for i in (0..8).rev() {
                let o = self.conv.push_bit((b >> i) & 1);
                self.coded.extend_from_slice(&[o & 1, o >> 1]);
            }
        }
        let pat = self.rate.puncture();
        puncture(&pat, self.coded_n % pat.period(), &self.coded, &mut self.bits);
        self.coded_n += self.coded.len();
        syms.clear();
        let mut i = 0;
        while i + 1 < self.bits.len() {
            syms.push(qpsk(self.bits[i], self.bits[i + 1]));
            i += 2;
        }
        self.bits.drain(..i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_useful_bit_rate_follows_the_standard() {
        // EN 300 421: 27.5 Msym/s at rate 3/4 carries 38.015 Mbit/s.
        assert!((useful_bit_rate(27.5e6, PunctureRate::R34) - 38.014_706e6).abs() < 1.0);
    }

    #[test]
    fn each_packet_gives_the_symbols_of_its_rate() {
        for r in PunctureRate::ALL {
            let mut e = S1Encoder::new(r);
            let mut syms = Vec::new();
            let mut total = 0;
            let p = [0x47u8; TS];
            let n = 2 * r.period();
            for _ in 0..n {
                e.packet(&p, &mut syms);
                total += syms.len();
            }
            // 204 × 8 bits a packet, coded at the rate, two bits a symbol.
            let want = n * RS_N * 8 * r.coded() / r.period() / 2;
            assert_eq!(total, want, "{r:?}");
        }
    }
}

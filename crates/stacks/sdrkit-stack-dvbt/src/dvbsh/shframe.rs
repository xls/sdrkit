//! The SH frame and the time interleaver (ETSI EN 302 583 clauses 5.4.3 and 5.5.2): code words of whole interleaving units and
//! a padding part filled by the energy-dispersal sequence, passed IU by IU through the 48-branch convolutional interleaver. The
//! transmitter side makes the IU stream of a test signal; the receiver side takes soft IUs and de-interleaves them into the
//! code words whose units all arrived, and decodes code words.

use super::coding::{build_eframe, dispersal, Eframe, ShCodec, EFRAME_BITS, IU_BITS, PACKETS, TS};
use sdrkit_algo_fec::blockinterleave::ConvBlocks;
use sdrkit_algo_fec::branchdelay::{ShTimeInterleaver, SH_BRANCHES};
use sdrkit_algo_fec::error::FecError;
use sdrkit_proto_mpegts::source::PacketSource;

/// The number of whole code words of `iu_per_cw` units in a DATA part of `capacity_iu` units (table 5.11 for the OFDM form).
pub fn codewords(capacity_iu: usize, iu_per_cw: usize) -> usize {
    capacity_iu / iu_per_cw.max(1)
}

/// The layout of the interleaved part of an SH frame for one pattern and one interleaver profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameShape {
    /// Units that pass the time interleaver per SH frame.
    pub capacity_iu: usize,
    /// Units per code word.
    pub iu_per_cw: usize,
    /// Code words per SH frame.
    pub codewords: usize,
    /// The interleaver profile.
    pub ti: ShTimeInterleaver,
    /// The SH frame capacity in units over 48, the unit of the slice offset.
    pub slice_unit: u32,
}

impl FrameShape {
    /// The shape of `capacity_iu` units for a code word of `n_bil` bits; `slice_unit` is the unit of the slice offset.
    pub fn new(capacity_iu: usize, n_bil: usize, ti: ShTimeInterleaver, slice_unit: u32) -> Self {
        let iu_per_cw = n_bil / IU_BITS;
        FrameShape {
            capacity_iu,
            iu_per_cw,
            codewords: codewords(capacity_iu, iu_per_cw),
            ti,
            slice_unit,
        }
    }

    /// The units of the DATA part.
    pub fn data_iu(&self) -> usize {
        self.codewords * self.iu_per_cw
    }
}

/// The transmitter of one stream: packets to EFRAMEs, code words, the SH frame and the time interleaver, one unit at a time.
pub struct FrameTx {
    shape: FrameShape,
    codec: ShCodec,
    source: Box<dyn PacketSource>,
    frame: Vec<u8>,
    padding: Vec<u8>,
    eframe: Vec<u8>,
    packets: [[u8; TS]; PACKETS],
    ti: ConvBlocks<u8>,
    at: usize,
    out: Vec<u8>,
    /// SH frames made.
    pub frames: u64,
}

impl FrameTx {
    /// A transmitter of `shape` with pattern `id` and the packets of `source`.
    ///
    /// # Errors
    ///
    /// [`FecError`] for a reserved pattern or an invalid interleaver profile.
    pub fn new(shape: FrameShape, id: u8, source: Box<dyn PacketSource>) -> Result<Self, FecError> {
        let codec = ShCodec::new(id)?;
        let rx = shape.ti.delays(shape.slice_unit)?;
        let ti = ConvBlocks::new(&ConvBlocks::<u8>::transmit_delays(&rx), IU_BITS, 0)?;
        let pad_bits = (shape.capacity_iu - shape.data_iu()) * IU_BITS;
        Ok(FrameTx {
            shape,
            codec,
            source,
            frame: vec![0; shape.capacity_iu * IU_BITS],
            padding: dispersal(pad_bits),
            eframe: vec![0; EFRAME_BITS],
            packets: [[0; TS]; PACKETS],
            ti,
            at: shape.capacity_iu,
            out: vec![0; IU_BITS],
            frames: 0,
        })
    }

    /// The shape.
    pub fn shape(&self) -> &FrameShape {
        &self.shape
    }

    fn make_frame(&mut self) {
        let n_bil = self.shape.iu_per_cw * IU_BITS;
        for cw in 0..self.shape.codewords {
            for p in self.packets.iter_mut() {
                self.source.next_packet(p);
            }
            build_eframe(
                &self.packets,
                cw as u32,
                self.codec.prbs(),
                &mut self.eframe,
            );
            let at = cw * n_bil;
            // The buffers have the code's sizes.
            let _ = self
                .codec
                .encode(&self.eframe, &mut self.frame[at..at + n_bil]);
        }
        let d = self.shape.data_iu() * IU_BITS;
        self.frame[d..].copy_from_slice(&self.padding);
        self.frames += 1;
    }

    /// The next interleaved unit (126 bits).
    pub fn next_iu(&mut self) -> &[u8] {
        if self.at == self.shape.capacity_iu {
            self.make_frame();
            self.at = 0;
        }
        let a = self.at * IU_BITS;
        self.at += 1;
        self.ti.push(&self.frame[a..a + IU_BITS], &mut self.out);
        &self.out
    }
}

/// What the receiver of a stream has counted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Code words decoded with good CRCs.
    pub codewords_ok: u64,
    /// Code words that did not decode, or that held units the de-interleaver had not filled.
    pub codewords_bad: u64,
    /// Packets with a good CRC.
    pub packets_ok: u64,
    /// Packets passed with the transport_error_indicator.
    pub packets_bad: u64,
    /// Turbo iterations of the decoded code words, summed.
    pub iterations: u64,
}

/// The time de-interleaver of one stream: soft units in step with the SH frame in, the code words whose units all arrived out.
pub struct FrameRx {
    shape: FrameShape,
    ti: ConvBlocks<i8>,
    lag: i64,
    q: i64,
    unit: Vec<i8>,
    cw: Vec<i8>,
    cw_index: Option<usize>,
    cw_valid: bool,
}

impl FrameRx {
    /// A de-interleaver of `shape`.
    ///
    /// # Errors
    ///
    /// [`FecError`] for an invalid interleaver profile.
    pub fn new(shape: FrameShape) -> Result<Self, FecError> {
        let rx = shape.ti.delays(shape.slice_unit)?;
        let lmax = rx.iter().copied().max().unwrap_or(0) as i64;
        Ok(FrameRx {
            shape,
            ti: ConvBlocks::new(&rx, IU_BITS, 0)?,
            lag: SH_BRANCHES as i64 * lmax,
            q: 0,
            unit: vec![0; IU_BITS],
            cw: vec![0; shape.iu_per_cw * IU_BITS],
            cw_index: None,
            cw_valid: false,
        })
    }

    /// The memory the de-interleaver holds, bytes, for `shape`.
    pub fn memory(shape: &FrameShape) -> Result<usize, FecError> {
        let d = shape.ti.delays(shape.slice_unit)?;
        Ok(d.iter().map(|&x| x as usize).sum::<usize>() * IU_BITS)
    }

    /// The shape.
    pub fn shape(&self) -> &FrameShape {
        &self.shape
    }

    /// Takes the next received unit (126 LLRs, the first of an SH frame at a multiple of the capacity since the receiver was
    /// made) and calls `on_word` with the soft values of a code word that completes with every unit filled.
    pub fn push(&mut self, llr: &[i8], on_word: impl FnOnce(&[i8])) {
        let valid = self.ti.push(llr, &mut self.unit);
        let cap = self.shape.capacity_iu as i64;
        let pos = (self.q - self.lag).rem_euclid(cap) as usize;
        self.q += 1;
        let ipc = self.shape.iu_per_cw;
        if pos >= self.shape.data_iu() {
            return;
        }
        let (cw, off) = (pos / ipc, pos % ipc);
        if off == 0 {
            self.cw_index = Some(cw);
            self.cw_valid = valid;
        } else if self.cw_index != Some(cw) {
            // The first unit of this code word went by before the receiver started.
            self.cw_index = Some(cw);
            self.cw_valid = false;
        }
        self.cw_valid &= valid;
        self.cw[off * IU_BITS..(off + 1) * IU_BITS].copy_from_slice(&self.unit);
        if off + 1 < ipc {
            return;
        }
        self.cw_index = None;
        if self.cw_valid {
            on_word(&self.cw);
        }
    }
}

/// The decoder of the code words of a stream, of any puncturing pattern, and its counters.
#[derive(Default)]
pub struct WordDecoder {
    codec: Option<(u8, ShCodec)>,
    eframe: Eframe,
    /// The counters.
    pub stats: FrameStats,
}

impl WordDecoder {
    /// Decodes code word `cw` of pattern `id` with at most `iters` turbo iterations and calls `on_packet(packet, good)` for
    /// every packet of it. A reserved pattern decodes nothing.
    pub fn decode(&mut self, id: u8, cw: &[i8], iters: u32, mut on_packet: impl FnMut(&[u8; TS], bool)) {
        if self.codec.as_ref().map(|c| c.0) != Some(id) {
            self.codec = ShCodec::new(id).ok().map(|c| (id, c));
        }
        let Some((_, codec)) = self.codec.as_mut() else {
            return;
        };
        match codec.decode_eframe(cw, iters, &mut self.eframe) {
            Ok(d) => {
                self.stats.codewords_ok += 1;
                self.stats.iterations += d.iterations as u64;
            }
            Err(_) => {
                self.stats.codewords_bad += 1;
                // The CRC of a packet is 16 bits: in a code word that did not decode, one that holds alone holds by
                // chance once in 65536 packets. Fewer than half of the packets good: none is passed as good.
                let good = self.eframe.ok.iter().filter(|&&ok| ok).count();
                if 2 * good < self.eframe.ok.len() {
                    for (p, ok) in self
                        .eframe
                        .packets
                        .iter_mut()
                        .zip(self.eframe.ok.iter_mut())
                    {
                        p[1] |= 0x80;
                        *ok = false;
                    }
                }
            }
        }
        for (p, &ok) in self.eframe.packets.iter().zip(&self.eframe.ok) {
            if ok {
                self.stats.packets_ok += 1;
            } else {
                self.stats.packets_bad += 1;
            }
            on_packet(p, ok);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dvbsh::coding::OFDM_FRAME_CU;

    #[test]
    fn ofdm_codewords_per_sh_frame() {
        // Table 5.11: N_CW per rate 1/5 .. 2/3 and the DATA CU.
        let ncw = [27, 30, 34, 38, 45, 54, 68, 90];
        let data_cu = [810, 810, 816, 798, 810, 810, 816, 810];
        for (i, id) in [0u8, 1, 2, 3, 4, 6, 8, 10].into_iter().enumerate() {
            let c = ShCodec::new(id).unwrap();
            let s = FrameShape::new(OFDM_FRAME_CU * 16, c.n_bil(), Default::default(), 272);
            assert_eq!(s.codewords, ncw[i], "pattern {id}");
            assert_eq!(s.data_iu() / 16, data_cu[i], "pattern {id}");
            assert_eq!(s.codewords * c.n_bil(), data_cu[i] * 2016);
        }
    }
}

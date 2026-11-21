//! The channel coding common to both DVB-SH forms (ETSI EN 302 583 clauses 5.1 to 5.4): the CRC-16 of each user packet, the
//! EHEADER, the padding and the EScrambler of an EFRAME, and the code word: the turbo code of clause 5.3, the bit-wise interleaver
//! and the rate adaptation of clause 5.4.2. The signalling field of the TDM form uses the same code with K = 1146.

use sdrkit_algo_fec::blockinterleave::KeptPermutation;
use sdrkit_algo_fec::error::{Decoded, FecError};
use sdrkit_algo_fec::turbo::{Slot, TurboDecoder};
use sdrkit_algo_fec::turbo_codes::{sh_code, sh_map, sh_pattern, ShPattern};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// Bits of an EFRAME, the turbo code's input (L_TC-input).
pub const EFRAME_BITS: usize = 12_282;
/// Bits of the signalling field of the TDM form before coding.
pub const SIGNALLING_BITS: usize = 1146;
/// User packets in a DATAFIELD.
pub const PACKETS: usize = 8;
/// Bits of one user packet after mode adaptation (187 bytes and the CRC-16).
pub const PACKET_BITS: usize = 1512;
/// Bits of the EHEADER.
pub const EHEADER_BITS: usize = 114;
/// Bits of the DATAFIELD.
pub const DATAFIELD_BITS: usize = PACKETS * PACKET_BITS;
/// Bits of one interleaving unit (IU).
pub const IU_BITS: usize = 126;
/// Bits of one capacity unit (CU, 16 IU).
pub const CU_BITS: usize = 2016;
/// The bytes of a transport packet.
pub const TS: usize = 188;
/// The bits of the SH frame capacity of the OFDM form: 816 CU.
pub const OFDM_FRAME_CU: usize = 816;

/// The multiplier `m` of the bit-wise interleaver `H(w) = m w mod N_TCB` for a code rate (table 5.7).
pub fn multiplier(rate: (u32, u32)) -> Option<usize> {
    Some(match rate {
        (1, 5) => 247,
        (2, 9) => 245,
        (1, 4) => 221,
        (2, 7) => 197,
        (1, 3) => 185,
        (2, 5) => 167,
        (1, 2) => 157,
        (2, 3) => 125,
        _ => return None,
    })
}

/// The name of a Punct_Pat_ID: the rate, with "c" for the complementary pattern ("1/3c").
pub fn pattern_name(id: u8) -> String {
    match sh_pattern(id) {
        Some(p) => format!(
            "{}/{}{}",
            p.rate.0,
            p.rate.1,
            if p.complementary { "c" } else { "" }
        ),
        None => format!("reserved {id}"),
    }
}

/// The EFRAME energy-dispersal sequence: the PRBS 1 + X^14 + X^15 loaded with 100101010000000, `n` bits.
pub fn dispersal(n: usize) -> Vec<u8> {
    let mut l = sdrkit_util_bits::seq::DVB_DISPERSAL;
    let mut v = vec![0u8; n];
    l.fill_bits(&mut v);
    v
}

fn put(bits: &mut [u8], at: usize, n: usize, v: u64) {
    for k in 0..n {
        bits[at + k] = ((v >> (n - 1 - k)) & 1) as u8;
    }
}

fn get(bits: &[u8], at: usize, n: usize) -> u64 {
    bits[at..at + n]
        .iter()
        .fold(0u64, |a, &b| (a << 1) | (b & 1) as u64)
}

/// The EHEADER of an EFRAME (clause 5.1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EHeader {
    /// TIS, the type of input stream: 3 MPEG-TS, 1 generic stream.
    pub tis: u8,
    /// UPL, the user packet length in bits.
    pub upl: u16,
    /// DFL, the DATAFIELD length in bits.
    pub dfl: u16,
    /// SYNC, a copy of the user packets' sync byte.
    pub sync: u8,
    /// CBCOUNTER: CBCOUNTER_SH in the 14 most significant bits, CBCOUNTER_FB (the EFRAME's index in its SH frame) in the 10
    /// least significant.
    pub counter: u32,
}

impl EHeader {
    /// The header of a transport stream EFRAME with counter `counter`.
    pub fn ts(counter: u32) -> Self {
        EHeader {
            tis: 3,
            upl: 1504,
            dfl: DATAFIELD_BITS as u16,
            sync: 0x47,
            counter: counter & 0xFF_FFFF,
        }
    }

    /// Writes the 114 bits, CRC-16 over bits 0 to 97 included.
    pub fn write(&self, bits: &mut [u8]) {
        put(bits, 0, 2, self.tis as u64);
        put(bits, 2, 16, self.upl as u64);
        put(bits, 18, 16, self.dfl as u64);
        put(bits, 34, 8, self.sync as u64);
        put(bits, 42, 32, 0);
        put(bits, 74, 24, self.counter as u64);
        let crc = CRC16_IBM_3740.bits(&bits[..98]);
        put(bits, 98, 16, crc as u64);
    }

    /// Reads the 114 bits; `None` when the CRC fails.
    pub fn read(bits: &[u8]) -> Option<Self> {
        if CRC16_IBM_3740.bits(&bits[..98]) as u64 != get(bits, 98, 16) {
            return None;
        }
        Some(EHeader {
            tis: get(bits, 0, 2) as u8,
            upl: get(bits, 2, 16) as u16,
            dfl: get(bits, 18, 16) as u16,
            sync: get(bits, 34, 8) as u8,
            counter: get(bits, 74, 24) as u32,
        })
    }
}

/// The CRC-16 of a transport packet: over the 187 bytes after the sync byte (clause 5.1.1).
pub fn packet_crc(p: &[u8; TS]) -> u16 {
    CRC16_IBM_3740.checksum(&p[1..])
}

/// Writes the EFRAME of `packets` (8) with counter `counter`, scrambled by `prbs` ([`dispersal`] of [`EFRAME_BITS`]), into
/// `out` ([`EFRAME_BITS`] bits).
pub fn build_eframe(packets: &[[u8; TS]], counter: u32, prbs: &[u8], out: &mut [u8]) {
    out[..EFRAME_BITS].fill(0);
    EHeader::ts(counter).write(out);
    for (i, p) in packets.iter().take(PACKETS).enumerate() {
        let at = EHEADER_BITS + i * PACKET_BITS;
        for (j, &b) in p[1..].iter().enumerate() {
            put(out, at + 8 * j, 8, b as u64);
        }
        put(out, at + 1496, 16, packet_crc(p) as u64);
    }
    for (o, &s) in out[..EFRAME_BITS].iter_mut().zip(prbs) {
        *o ^= s;
    }
}

/// An EFRAME read back: the header (when its CRC holds) and the packets with their CRC results.
#[derive(Clone, Debug)]
pub struct Eframe {
    /// The EHEADER, `None` when its CRC failed.
    pub header: Option<EHeader>,
    /// The packets, sync byte restored; a packet whose CRC failed has the transport_error_indicator set.
    pub packets: [[u8; TS]; PACKETS],
    /// Whether each packet's CRC held.
    pub ok: [bool; PACKETS],
}

impl Default for Eframe {
    fn default() -> Self {
        Eframe {
            header: None,
            packets: [[0; TS]; PACKETS],
            ok: [false; PACKETS],
        }
    }
}

/// Whether the descrambled EFRAME `bits` has a good EHEADER and 8 good packets.
pub fn eframe_good(bits: &[u8]) -> bool {
    EHeader::read(bits).is_some()
        && (0..PACKETS).all(|i| {
            let at = EHEADER_BITS + i * PACKET_BITS;
            CRC16_IBM_3740.bits(&bits[at..at + 1496]) as u64 == get(bits, at + 1496, 16)
        })
}

/// Reads the descrambled EFRAME `bits` into `out`.
pub fn parse_eframe(bits: &[u8], out: &mut Eframe) {
    out.header = EHeader::read(bits);
    let sync = out.header.map_or(0x47, |h| h.sync);
    for i in 0..PACKETS {
        let at = EHEADER_BITS + i * PACKET_BITS;
        let p = &mut out.packets[i];
        p[0] = sync;
        for j in 0..187 {
            p[1 + j] = get(bits, at + 8 * j, 8) as u8;
        }
        let ok = packet_crc(p) as u64 == get(bits, at + 1496, 16);
        out.ok[i] = ok;
        if !ok {
            p[1] |= 0x80;
        }
    }
}

/// The DVB-SH code word codec of one puncturing pattern: turbo code, bit-wise interleaver and rate adaptation. Every buffer is
/// sized when it is made; encoding and decoding do not allocate.
pub struct ShCodec {
    pattern: &'static ShPattern,
    k: usize,
    map: Vec<Slot>,
    perm: KeptPermutation,
    dec: TurboDecoder,
    prbs: Vec<u8>,
    raw: Vec<u8>,
    coded: Vec<u8>,
    full: Vec<i8>,
    hard: Vec<u8>,
    scratch: Vec<u8>,
}

impl ShCodec {
    /// The codec of the EFRAME code word for Punct_Pat_ID `id` (0 to 11).
    ///
    /// # Errors
    ///
    /// [`FecError::NotAllowed`] for a reserved id.
    pub fn new(id: u8) -> Result<Self, FecError> {
        let p = sh_pattern(id).ok_or(FecError::NotAllowed {
            param: sdrkit_algo_fec::error::Param::Puncturing,
            got: id as i64,
        })?;
        let m = multiplier(p.rate).unwrap_or(1);
        Self::build(p, EFRAME_BITS, m, 126, 128)
    }

    /// The codec of the signalling field of the TDM form: K = 1146, pattern 0, `H(w) = 73 w mod 5760`, no rate adaptation.
    ///
    /// # Errors
    ///
    /// Never for the fixed parameters; the type is that of [`ShCodec::new`].
    pub fn signalling() -> Result<Self, FecError> {
        Self::build(
            &sdrkit_algo_fec::turbo_codes::SH_PATTERNS[0],
            SIGNALLING_BITS,
            73,
            1,
            1,
        )
    }

    fn build(
        pattern: &'static ShPattern,
        k: usize,
        m: usize,
        keep: usize,
        period: usize,
    ) -> Result<Self, FecError> {
        let code = sh_code(k)?;
        let map = sh_map(k, pattern);
        let per = code.periods() * code.streams();
        let perm = KeptPermutation::new(m, map.len(), keep, period)?;
        Ok(ShCodec {
            pattern,
            k,
            perm,
            dec: TurboDecoder::new(code),
            prbs: if k == EFRAME_BITS {
                dispersal(k)
            } else {
                Vec::new()
            },
            raw: vec![0; per],
            coded: vec![0; map.len()],
            full: vec![0; map.len()],
            hard: vec![0; k],
            scratch: vec![0; k],
            map,
        })
    }

    /// The pattern.
    pub fn pattern(&self) -> &'static ShPattern {
        self.pattern
    }

    /// The information bits K.
    pub fn k(&self) -> usize {
        self.k
    }

    /// The coded bits N_TCB.
    pub fn n_tcb(&self) -> usize {
        self.map.len()
    }

    /// The bits sent, N_BIL (N_TCB without the bits the rate adaptation drops).
    pub fn n_bil(&self) -> usize {
        self.perm.kept()
    }

    /// The EFRAME energy-dispersal sequence (empty for the signalling codec).
    pub fn prbs(&self) -> &[u8] {
        &self.prbs
    }

    /// Encodes `bits` (K) into `out` (N_BIL).
    pub fn encode(&mut self, bits: &[u8], out: &mut [u8]) -> Result<(), FecError> {
        let code = self.dec.code();
        code.encode(bits, &mut self.raw)?;
        code.transmit(&self.map, &self.raw, &mut self.coded)?;
        self.perm.apply(&self.coded, out)
    }

    /// Decodes `llr` (N_BIL, positive means 0, 0 erased) with at most `iters` iterations, stopping as soon as `check` passes
    /// on the K hard bits. Writes K bits to `out`.
    pub fn decode(
        &mut self,
        llr: &[i8],
        iters: u32,
        check: impl FnMut(&[u8]) -> bool,
        out: &mut [u8],
    ) -> Result<Decoded, FecError> {
        self.perm.invert(llr, 0, &mut self.full)?;
        self.dec.load(&self.map, &self.full)?;
        self.dec.decode(iters, check, out)
    }

    /// Decodes an EFRAME code word: stops as soon as the EHEADER and the 8 packet CRCs hold, and reads the descrambled EFRAME
    /// into `out`. `Err` when they never held (`out` then holds the last decisions, bad packets flagged). A word with
    /// fewer soft values that are not erased than data bits cannot decode: no iteration runs, and `out` holds the
    /// decisions of a word that is all erased.
    pub fn decode_eframe(
        &mut self,
        llr: &[i8],
        iters: u32,
        out: &mut Eframe,
    ) -> Result<Decoded, FecError> {
        let known = llr.iter().filter(|&&l| l != 0).count();
        if llr.len() == self.n_bil() && self.prbs.len() == self.k() && known < self.k() {
            parse_eframe(&self.prbs, out);
            return Err(FecError::Uncorrectable);
        }
        let (prbs, scratch) = (&self.prbs, &mut self.scratch);
        let check = |h: &[u8]| {
            for ((s, &b), &p) in scratch.iter_mut().zip(h).zip(prbs) {
                *s = b ^ p;
            }
            eframe_good(scratch)
        };
        self.perm.invert(llr, 0, &mut self.full)?;
        self.dec.load(&self.map, &self.full)?;
        let r = self.dec.decode(iters, check, &mut self.hard);
        for ((s, &b), &p) in self.scratch.iter_mut().zip(&self.hard).zip(&self.prbs) {
            *s = b ^ p;
        }
        parse_eframe(&self.scratch, out);
        r
    }
}

//! The ATSC 3.0 L1 signalling: the fields of L1-Basic and L1-Detail, their bit forms, the CRC, and the scrambling sequence that
//! the signalling, the baseband packets and the dummy cells share.
//!
//! Standard: ATSC A/322:2023-03 clauses 5.2.3, 6.1.2.2, 9.2 and 9.3.

/// The bits of L1-Basic.
pub const L1_BASIC_BITS: usize = 200;

/// The fewest bytes of L1-Detail (clause 9.2.2).
pub const L1_DETAIL_MIN_BYTES: usize = 25;

/// The CRC of clause 6.1.2.2 over `bits` (one per byte): the register of x^32 + x^21 + x^16 + x^11 + 1 starts at all ones.
pub fn crc32(bits: &[u8]) -> u32 {
    let mut reg = u32::MAX;
    for &b in bits {
        let feedback = (reg >> 31) ^ u32::from(b & 1);
        reg <<= 1;
        if feedback == 1 {
            reg ^= 0x0021_0801;
        }
    }
    reg
}

/// The scrambling sequence of clause 5.2.3, one byte per step: the register of 1 + X + X^3 + X^6 + X^7 + X^11 + X^12 + X^13 +
/// X^16 starts at 0xF180.
#[derive(Clone, Copy, Debug)]
pub struct Scrambler {
    /// Stage `X^i` is bit `i − 1`.
    reg: u16,
}

impl Default for Scrambler {
    fn default() -> Self {
        Scrambler { reg: 0xF180 }
    }
}

impl Scrambler {
    /// The next byte `D7 .. D0` of the sequence.
    pub fn byte(&mut self) -> u8 {
        let x = |i: u32| (self.reg >> (i - 1) & 1) as u8;
        let out = x(14) << 7
            | x(13) << 6
            | x(12) << 5
            | x(11) << 4
            | x(7) << 3
            | x(4) << 2
            | x(3) << 1
            | x(1);
        let feedback = self.reg >> 15;
        self.reg <<= 1;
        if feedback == 1 {
            // Into X, and after X, X^3, X^6, X^7, X^11, X^12 and X^13.
            self.reg ^= 0b0011_1000_1100_1011;
        }
        out
    }
    /// Scrambles or descrambles `bytes` from the start of the sequence.
    pub fn apply(bytes: &mut [u8]) {
        let mut s = Scrambler::default();
        bytes.iter_mut().for_each(|b| *b ^= s.byte());
    }
    /// Scrambles or descrambles `bits` (one per byte, the first is the most significant of a byte) from the start of the
    /// sequence.
    pub fn apply_bits(bits: &mut [u8]) {
        let mut s = Scrambler::default();
        for chunk in bits.chunks_mut(8) {
            let k = s.byte();
            for (i, b) in chunk.iter_mut().enumerate() {
                *b ^= k >> (7 - i) & 1;
            }
        }
    }
    /// The first `n` bits of the sequence.
    pub fn bits(n: usize) -> Vec<u8> {
        let mut out = vec![0u8; n];
        Scrambler::apply_bits(&mut out);
        out
    }
}

/// Why a signalling block did not parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum L1Error {
    /// The CRC does not agree.
    #[error("the CRC of the L1 signalling is wrong")]
    Crc,
    /// The block ends before its fields do.
    #[error("the L1 signalling is too short")]
    Short,
    /// A field has a value that makes the block impossible to read.
    #[error("the L1 signalling holds a value that cannot be read")]
    Value,
}

fn put(bits: &mut Vec<u8>, value: u64, n: usize) {
    bits.extend((0..n).rev().map(|i| (value >> i & 1) as u8));
}

struct Reader<'a> {
    bits: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn get(&mut self, n: usize) -> Result<u32, L1Error> {
        let end = self.at + n;
        let field = self.bits.get(self.at..end).ok_or(L1Error::Short)?;
        self.at = end;
        Ok(field.iter().fold(0, |v, &b| v << 1 | u32::from(b & 1)))
    }
    fn flag(&mut self) -> Result<bool, L1Error> {
        Ok(self.get(1)? == 1)
    }
}

/// How the length of a frame is given (clause 7.2.2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameLength {
    /// A time-aligned frame: its length in units of 5 ms, and the extra samples in the guard of each symbol after the
    /// Preamble.
    TimeAligned {
        /// `L1B_frame_length`.
        units_5ms: u16,
        /// `L1B_excess_samples_per_symbol`.
        excess_samples: u16,
    },
    /// A symbol-aligned frame: the symbols follow each other with their signalled guards.
    SymbolAligned {
        /// `L1B_time_offset`.
        time_offset: u16,
        /// `L1B_additional_samples`.
        additional_samples: u8,
    },
}

/// The waveform of a subframe (in L1-Basic for the first, in L1-Detail for the others).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubframeParams {
    /// MIMO is used.
    pub mimo: bool,
    /// The MISO option code (table 9.9).
    pub miso: u8,
    /// The FFT size code (table 9.10).
    pub fft_size: u8,
    /// The carrier reduction, 0 to 4.
    pub reduced_carriers: u8,
    /// The guard interval code (table 9.11).
    pub guard_interval: u8,
    /// The data and subframe boundary symbols of the subframe (the field holds one less).
    pub symbols: u16,
    /// The scattered pilot pattern code (table 9.12).
    pub pilot_pattern: u8,
    /// The scattered pilot boost code (table 9.14).
    pub pilot_boost: u8,
    /// The first symbol is a subframe boundary symbol.
    pub sbs_first: bool,
    /// The last symbol is a subframe boundary symbol.
    pub sbs_last: bool,
}

impl SubframeParams {
    fn pack(&self, b: &mut Vec<u8>) {
        put(b, u64::from(self.mimo), 1);
        put(b, u64::from(self.miso), 2);
        put(b, u64::from(self.fft_size), 2);
        put(b, u64::from(self.reduced_carriers), 3);
        put(b, u64::from(self.guard_interval), 4);
        put(b, u64::from(self.symbols.wrapping_sub(1)), 11);
        put(b, u64::from(self.pilot_pattern), 5);
        put(b, u64::from(self.pilot_boost), 3);
        put(b, u64::from(self.sbs_first), 1);
        put(b, u64::from(self.sbs_last), 1);
    }
    fn parse(r: &mut Reader) -> Result<SubframeParams, L1Error> {
        Ok(SubframeParams {
            mimo: r.flag()?,
            miso: r.get(2)? as u8,
            fft_size: r.get(2)? as u8,
            reduced_carriers: r.get(3)? as u8,
            guard_interval: r.get(4)? as u8,
            symbols: r.get(11)? as u16 + 1,
            pilot_pattern: r.get(5)? as u8,
            pilot_boost: r.get(3)? as u8,
            sbs_first: r.flag()?,
            sbs_last: r.flag()?,
        })
    }
}

/// L1-Basic (table 9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct L1Basic {
    /// `L1B_version`.
    pub version: u8,
    /// `L1B_mimo_scattered_pilot_encoding`.
    pub mimo_pilot_encoding: bool,
    /// `L1B_lls_flag`: a PLP of the frame carries low level signalling.
    pub lls: bool,
    /// `L1B_time_info_flag`: 0 none, 1 ms, 2 µs, 3 ns.
    pub time_info: u8,
    /// `L1B_return_channel_flag`.
    pub return_channel: bool,
    /// `L1B_papr_reduction` (table 9.5).
    pub papr: u8,
    /// The frame length mode and its two fields.
    pub frame_length: FrameLength,
    /// The subframes of the frame (the field holds one less).
    pub subframes: u16,
    /// The Preamble symbols (the field holds one less).
    pub preamble_symbols: u8,
    /// The carrier reduction of the Preamble symbols after the first.
    pub preamble_reduced_carriers: u8,
    /// `L1B_L1_Detail_content_tag`.
    pub detail_content_tag: u8,
    /// The size of L1-Detail, bytes.
    pub detail_bytes: u16,
    /// The L1-Detail mode, 1 to 7 (the field holds one less; 8 is the reserved code).
    pub detail_mode: u8,
    /// `L1B_L1_Detail_additional_parity_mode`.
    pub detail_additional_parity: u8,
    /// The cells of L1-Detail with its additional parity.
    pub detail_cells: u32,
    /// The first subframe.
    pub first: SubframeParams,
}

impl L1Basic {
    /// The 200 bits, one per byte, CRC included.
    pub fn pack(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(L1_BASIC_BITS);
        put(&mut b, u64::from(self.version), 3);
        put(&mut b, u64::from(self.mimo_pilot_encoding), 1);
        put(&mut b, u64::from(self.lls), 1);
        put(&mut b, u64::from(self.time_info), 2);
        put(&mut b, u64::from(self.return_channel), 1);
        put(&mut b, u64::from(self.papr), 2);
        match self.frame_length {
            FrameLength::TimeAligned {
                units_5ms,
                excess_samples,
            } => {
                put(&mut b, 0, 1);
                put(&mut b, u64::from(units_5ms), 10);
                put(&mut b, u64::from(excess_samples), 13);
            }
            FrameLength::SymbolAligned {
                time_offset,
                additional_samples,
            } => {
                put(&mut b, 1, 1);
                put(&mut b, u64::from(time_offset), 16);
                put(&mut b, u64::from(additional_samples), 7);
            }
        }
        put(&mut b, u64::from(self.subframes.wrapping_sub(1)), 8);
        put(&mut b, u64::from(self.preamble_symbols.wrapping_sub(1)), 3);
        put(&mut b, u64::from(self.preamble_reduced_carriers), 3);
        put(&mut b, u64::from(self.detail_content_tag), 2);
        put(&mut b, u64::from(self.detail_bytes), 13);
        put(&mut b, u64::from(self.detail_mode.wrapping_sub(1)), 3);
        put(&mut b, u64::from(self.detail_additional_parity), 2);
        put(&mut b, u64::from(self.detail_cells), 19);
        self.first.pack(&mut b);
        put(&mut b, 0, 48);
        let crc = crc32(&b);
        put(&mut b, u64::from(crc), 32);
        b
    }

    /// Reads the 200 bits `bits` (one per byte).
    ///
    /// # Errors
    ///
    /// [`L1Error::Short`] for fewer than 200 bits, [`L1Error::Crc`] when the CRC does not agree.
    pub fn parse(bits: &[u8]) -> Result<L1Basic, L1Error> {
        let bits = bits.get(..L1_BASIC_BITS).ok_or(L1Error::Short)?;
        let mut r = Reader { bits, at: 0 };
        let version = r.get(3)? as u8;
        let mimo_pilot_encoding = r.flag()?;
        let lls = r.flag()?;
        let time_info = r.get(2)? as u8;
        let return_channel = r.flag()?;
        let papr = r.get(2)? as u8;
        let frame_length = if r.flag()? {
            FrameLength::SymbolAligned {
                time_offset: r.get(16)? as u16,
                additional_samples: r.get(7)? as u8,
            }
        } else {
            FrameLength::TimeAligned {
                units_5ms: r.get(10)? as u16,
                excess_samples: r.get(13)? as u16,
            }
        };
        let l1 = L1Basic {
            version,
            mimo_pilot_encoding,
            lls,
            time_info,
            return_channel,
            papr,
            frame_length,
            subframes: r.get(8)? as u16 + 1,
            preamble_symbols: r.get(3)? as u8 + 1,
            preamble_reduced_carriers: r.get(3)? as u8,
            detail_content_tag: r.get(2)? as u8,
            detail_bytes: r.get(13)? as u16,
            detail_mode: r.get(3)? as u8 + 1,
            detail_additional_parity: r.get(2)? as u8,
            detail_cells: r.get(19)?,
            first: SubframeParams::parse(&mut r)?,
        };
        r.get(32)?;
        r.get(16)?;
        let crc = r.get(32)?;
        if crc != crc32(&bits[..L1_BASIC_BITS - 32]) {
            return Err(L1Error::Crc);
        }
        Ok(l1)
    }
}

/// The time of the frame in L1-Detail (clause 9.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct L1Time {
    /// Seconds of TAI.
    pub seconds: u32,
    /// Milliseconds.
    pub milliseconds: u16,
    /// Microseconds, when the precision holds them.
    pub microseconds: Option<u16>,
    /// Nanoseconds, when the precision holds them.
    pub nanoseconds: Option<u16>,
}

/// The hybrid time interleaver fields of a PLP (clause 9.3.9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hti {
    /// `L1D_plp_HTI_inter_subframe`.
    pub inter_subframe: bool,
    /// `L1D_plp_HTI_num_ti_blocks` as signalled.
    pub ti_blocks: u8,
    /// `L1D_plp_HTI_num_fec_blocks_max` as signalled.
    pub fec_blocks_max: u16,
    /// `L1D_plp_HTI_num_fec_blocks`: the first value, or the first `ti_blocks + 1` values with `inter_subframe`.
    pub fec_blocks: [u16; 16],
    /// `L1D_plp_HTI_cell_interleaver`.
    pub cell_interleaver: bool,
}

impl Hti {
    /// The values of `fec_blocks` that are sent.
    pub fn sent(&self) -> usize {
        if self.inter_subframe {
            usize::from(self.ti_blocks) + 1
        } else {
            1
        }
    }
}

/// The channel bonding of a PLP (clause 9.3.6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonding {
    /// `L1D_plp_channel_bonding_format`.
    pub format: u8,
    /// `L1D_plp_bonded_rf_id`: the first `count` values.
    pub rf_ids: [u8; 8],
    /// The RF ids sent: `L1D_plp_num_channel_bonded + 1`.
    pub count: u8,
}

/// One PLP of a subframe in L1-Detail (table 9.8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plp {
    /// `L1D_plp_id`, 0 to 63.
    pub id: u8,
    /// The PLP carries low level signalling.
    pub lls: bool,
    /// The layer: 0 core, above 0 enhanced.
    pub layer: u8,
    /// The index of the first data cell.
    pub start: u32,
    /// The data cells in the subframe.
    pub size: u32,
    /// `L1D_plp_scrambler_type`.
    pub scrambler: u8,
    /// `L1D_plp_fec_type` (table 9.17).
    pub fec_type: u8,
    /// `L1D_plp_mod` (table 9.18), for the FEC types 0 to 5.
    pub modulation: Option<u8>,
    /// `L1D_plp_cod` (table 9.20): the code rate in fifteenths less 2, for the FEC types 0 to 5.
    pub code_rate: Option<u8>,
    /// `L1D_plp_TI_mode` (table 9.21).
    pub ti_mode: u8,
    /// The first cell of the first whole FEC block, without time interleaving.
    pub fec_block_start: Option<u16>,
    /// `L1D_plp_CTI_fec_block_start`, with the convolutional time interleaver.
    pub cti_fec_block_start: Option<u32>,
    /// `L1D_plp_num_channel_bonded`, when channels are bonded.
    pub channels_bonded: Option<u8>,
    /// The bonding format and the RF ids, when the PLP is bonded.
    pub bonding: Option<Bonding>,
    /// Stream combining, IQ interleaving and phase hopping, in a MIMO subframe.
    pub mimo: Option<(bool, bool, bool)>,
    /// Dispersed (true) or non-dispersed, for a core PLP.
    pub dispersed: Option<bool>,
    /// The subslices (the field holds one less) and their interval, for a dispersed PLP.
    pub subslices: Option<(u16, u32)>,
    /// `L1D_plp_TI_extended_interleaving`.
    pub ti_extended: Option<bool>,
    /// The depth code and start row of the convolutional time interleaver.
    pub cti: Option<(u8, u16)>,
    /// The hybrid time interleaver fields.
    pub hti: Option<Hti>,
    /// `L1D_plp_ldm_injection_level`, for an enhanced PLP.
    pub ldm_injection_level: Option<u8>,
}

/// One subframe in L1-Detail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetailSubframe {
    /// The waveform, for every subframe after the first.
    pub params: Option<SubframeParams>,
    /// `L1D_subframe_multiplex`, when the frame has more than one subframe.
    pub multiplex: Option<bool>,
    /// The frequency interleaver is used.
    pub frequency_interleaver: bool,
    /// The null cells of the subframe boundary symbols, when the subframe has one.
    pub sbs_null_cells: Option<u16>,
    /// The PLPs.
    pub plps: Vec<Plp>,
}

/// L1-Detail (table 9.8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct L1Detail {
    /// `L1D_version`.
    pub version: u8,
    /// The BSIDs of the bonded channels.
    pub bonded_bsids: Vec<u16>,
    /// The time, when L1-Basic says it is there.
    pub time: Option<L1Time>,
    /// The subframes.
    pub subframes: Vec<DetailSubframe>,
    /// `L1D_bsid`.
    pub bsid: u16,
}

impl L1Detail {
    /// The bits of L1-Detail for a frame with L1-Basic `basic`, one per byte, with reserved zeros up to a whole number of
    /// bytes (25 at least) and the CRC.
    pub fn pack(&self, basic: &L1Basic) -> Vec<u8> {
        self.pack_with(basic, 0)
    }

    /// As [`L1Detail::pack`], with `reserved` more bytes of reserved zeros before the CRC.
    pub fn pack_with(&self, basic: &L1Basic, reserved: usize) -> Vec<u8> {
        let mut b = Vec::new();
        put(&mut b, u64::from(self.version), 4);
        put(&mut b, self.bonded_bsids.len() as u64, 3);
        for &id in &self.bonded_bsids {
            put(&mut b, u64::from(id), 16);
            put(&mut b, 0, 3);
        }
        if let (Some(t), true) = (&self.time, basic.time_info != 0) {
            put(&mut b, u64::from(t.seconds), 32);
            put(&mut b, u64::from(t.milliseconds), 10);
            if basic.time_info != 1 {
                put(&mut b, u64::from(t.microseconds.unwrap_or(0)), 10);
                if basic.time_info != 2 {
                    put(&mut b, u64::from(t.nanoseconds.unwrap_or(0)), 10);
                }
            }
        }
        let bonded = !self.bonded_bsids.is_empty();
        for (i, s) in self.subframes.iter().enumerate() {
            let params = if i == 0 {
                basic.first
            } else {
                let p = s.params.unwrap_or(basic.first);
                p.pack(&mut b);
                p
            };
            if basic.subframes > 1 {
                put(&mut b, u64::from(s.multiplex.unwrap_or(false)), 1);
            }
            put(&mut b, u64::from(s.frequency_interleaver), 1);
            if params.sbs_first || params.sbs_last {
                put(&mut b, u64::from(s.sbs_null_cells.unwrap_or(0)), 13);
            }
            put(&mut b, (s.plps.len() as u64).wrapping_sub(1), 6);
            for p in &s.plps {
                pack_plp(&mut b, p, bonded, params.mimo);
            }
        }
        put(&mut b, u64::from(self.bsid), 16);
        let bytes = ((b.len() + 32).div_ceil(8) + reserved).max(L1_DETAIL_MIN_BYTES);
        b.resize(bytes * 8 - 32, 0);
        let crc = crc32(&b);
        put(&mut b, u64::from(crc), 32);
        b
    }

    /// Reads L1-Detail from `bits` (one per byte; the size that L1-Basic `basic` gives, or more). Reserved bits before the
    /// CRC are accepted.
    ///
    /// # Errors
    ///
    /// As [`L1Detail::parse_into`].
    pub fn parse(bits: &[u8], basic: &L1Basic) -> Result<L1Detail, L1Error> {
        let mut d = L1Detail::default();
        d.parse_into(bits, basic)?;
        Ok(d)
    }

    /// As [`L1Detail::parse`], into `self`: it keeps the storage of its lists, so reading the L1-Detail of frame after frame
    /// allocates nothing. After an error `self` holds no valid signalling.
    ///
    /// # Errors
    ///
    /// [`L1Error::Short`] when `bits` ends before the fields or the size of `basic`, [`L1Error::Crc`] when the CRC does not
    /// agree, [`L1Error::Value`] when the fields need more bits than the size holds.
    pub fn parse_into(&mut self, bits: &[u8], basic: &L1Basic) -> Result<(), L1Error> {
        let size = usize::from(basic.detail_bytes) * 8;
        if size < 32 {
            return Err(L1Error::Short);
        }
        let bits = bits.get(..size).ok_or(L1Error::Short)?;
        let body = &bits[..size - 32];
        let crc = bits[size - 32..]
            .iter()
            .fold(0u32, |v, &b| v << 1 | u32::from(b & 1));
        if crc != crc32(body) {
            return Err(L1Error::Crc);
        }
        let mut r = Reader { bits: body, at: 0 };
        let value = |e: L1Error| {
            if e == L1Error::Short {
                L1Error::Value
            } else {
                e
            }
        };
        self.fields(&mut r, basic).map_err(value)
    }

    fn fields(&mut self, r: &mut Reader, basic: &L1Basic) -> Result<(), L1Error> {
        self.version = r.get(4)? as u8;
        let num_rf = r.get(3)?;
        self.bonded_bsids.clear();
        for _ in 0..num_rf {
            self.bonded_bsids.push(r.get(16)? as u16);
            r.get(3)?;
        }
        self.time = if basic.time_info != 0 {
            let seconds = r.get(32)?;
            let milliseconds = r.get(10)? as u16;
            let microseconds = if basic.time_info != 1 {
                Some(r.get(10)? as u16)
            } else {
                None
            };
            let nanoseconds = if basic.time_info == 3 {
                Some(r.get(10)? as u16)
            } else {
                None
            };
            Some(L1Time {
                seconds,
                milliseconds,
                microseconds,
                nanoseconds,
            })
        } else {
            None
        };
        let count = usize::from(basic.subframes);
        self.subframes.truncate(count);
        for i in 0..count {
            if i == self.subframes.len() {
                self.subframes.push(DetailSubframe::default());
            }
            let s = &mut self.subframes[i];
            s.plps.clear();
            s.params = if i > 0 {
                Some(SubframeParams::parse(r)?)
            } else {
                None
            };
            let waveform = s.params.unwrap_or(basic.first);
            s.multiplex = if basic.subframes > 1 {
                Some(r.flag()?)
            } else {
                None
            };
            s.frequency_interleaver = r.flag()?;
            s.sbs_null_cells = if waveform.sbs_first || waveform.sbs_last {
                Some(r.get(13)? as u16)
            } else {
                None
            };
            for _ in 0..r.get(6)? + 1 {
                s.plps.push(parse_plp(r, num_rf > 0, waveform.mimo)?);
            }
        }
        self.bsid = r.get(16)? as u16;
        Ok(())
    }
}

fn pack_plp(b: &mut Vec<u8>, p: &Plp, bonded: bool, mimo: bool) {
    put(b, u64::from(p.id), 6);
    put(b, u64::from(p.lls), 1);
    put(b, u64::from(p.layer), 2);
    put(b, u64::from(p.start), 24);
    put(b, u64::from(p.size), 24);
    put(b, u64::from(p.scrambler), 2);
    put(b, u64::from(p.fec_type), 4);
    if p.fec_type <= 5 {
        put(b, u64::from(p.modulation.unwrap_or(0)), 4);
        put(b, u64::from(p.code_rate.unwrap_or(0)), 4);
    }
    put(b, u64::from(p.ti_mode), 2);
    match p.ti_mode {
        0 => put(b, u64::from(p.fec_block_start.unwrap_or(0x7FFF)), 15),
        1 => put(b, u64::from(p.cti_fec_block_start.unwrap_or(0)), 22),
        _ => {}
    }
    if bonded {
        let bonding = p.bonding.unwrap_or_default();
        let n = u64::from(bonding.count.saturating_sub(1));
        put(b, n, 3);
        if n > 0 {
            put(b, u64::from(bonding.format), 2);
            bonding.rf_ids[..usize::from(bonding.count)]
                .iter()
                .for_each(|&id| put(b, u64::from(id), 3));
        }
    }
    if mimo {
        let (a, c, d) = p.mimo.unwrap_or_default();
        put(b, u64::from(a), 1);
        put(b, u64::from(c), 1);
        put(b, u64::from(d), 1);
    }
    if p.layer == 0 {
        let dispersed = p.dispersed.unwrap_or(false);
        put(b, u64::from(dispersed), 1);
        if dispersed {
            let (n, interval) = p.subslices.unwrap_or((2, 0));
            put(b, u64::from(n.wrapping_sub(1)), 14);
            put(b, u64::from(interval), 24);
        }
        if (p.ti_mode == 1 || p.ti_mode == 2) && p.modulation == Some(0) {
            put(b, u64::from(p.ti_extended.unwrap_or(false)), 1);
        }
        if p.ti_mode == 1 {
            let (depth, row) = p.cti.unwrap_or_default();
            put(b, u64::from(depth), 3);
            put(b, u64::from(row), 11);
        } else if p.ti_mode == 2 {
            let h = p.hti.unwrap_or_default();
            put(b, u64::from(h.inter_subframe), 1);
            put(b, u64::from(h.ti_blocks), 4);
            put(b, u64::from(h.fec_blocks_max), 12);
            for &n in &h.fec_blocks[..h.sent()] {
                put(b, u64::from(n), 12);
            }
            put(b, u64::from(h.cell_interleaver), 1);
        }
    } else {
        put(b, u64::from(p.ldm_injection_level.unwrap_or(0)), 5);
    }
}

fn parse_plp(r: &mut Reader, bonded: bool, mimo: bool) -> Result<Plp, L1Error> {
    let mut p = Plp {
        id: r.get(6)? as u8,
        lls: r.flag()?,
        layer: r.get(2)? as u8,
        start: r.get(24)?,
        size: r.get(24)?,
        scrambler: r.get(2)? as u8,
        fec_type: r.get(4)? as u8,
        ..Plp::default()
    };
    if p.fec_type <= 5 {
        p.modulation = Some(r.get(4)? as u8);
        p.code_rate = Some(r.get(4)? as u8);
    }
    p.ti_mode = r.get(2)? as u8;
    match p.ti_mode {
        0 => p.fec_block_start = Some(r.get(15)? as u16),
        1 => p.cti_fec_block_start = Some(r.get(22)?),
        _ => {}
    }
    if bonded {
        let n = r.get(3)? as u8;
        p.channels_bonded = Some(n);
        if n > 0 {
            let mut bonding = Bonding {
                format: r.get(2)? as u8,
                rf_ids: [0; 8],
                count: n + 1,
            };
            for id in &mut bonding.rf_ids[..usize::from(n) + 1] {
                *id = r.get(3)? as u8;
            }
            p.bonding = Some(bonding);
        }
    }
    if mimo {
        p.mimo = Some((r.flag()?, r.flag()?, r.flag()?));
    }
    if p.layer == 0 {
        let dispersed = r.flag()?;
        p.dispersed = Some(dispersed);
        if dispersed {
            p.subslices = Some((r.get(14)? as u16 + 1, r.get(24)?));
        }
        if (p.ti_mode == 1 || p.ti_mode == 2) && p.modulation == Some(0) {
            p.ti_extended = Some(r.flag()?);
        }
        if p.ti_mode == 1 {
            p.cti = Some((r.get(3)? as u8, r.get(11)? as u16));
        } else if p.ti_mode == 2 {
            let mut h = Hti {
                inter_subframe: r.flag()?,
                ti_blocks: r.get(4)? as u8,
                fec_blocks_max: r.get(12)? as u16,
                ..Hti::default()
            };
            for k in 0..h.sent() {
                h.fec_blocks[k] = r.get(12)? as u16;
            }
            h.cell_interleaver = r.flag()?;
            p.hti = Some(h);
        }
    } else {
        p.ldm_injection_level = Some(r.get(5)? as u8);
    }
    Ok(p)
}

/// The coded blocks of an L1-Detail of `bits` bits in a mode whose blocks hold `k_seg` bits at most (clause 6.5.3.1): the
/// number of blocks and the information bits `K_sig` of each (zeros fill the last bits of each block's share).
pub fn detail_segments(bits: usize, k_seg: usize) -> (usize, usize) {
    let blocks = bits.div_ceil(k_seg).max(1);
    let k_sig = bits.div_ceil(blocks * 8) * 8;
    (blocks, k_sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clause 5.2.3: the first values printed are 1100 0000 0110 1101 0011 1111.
    #[test]
    fn the_scrambling_sequence_starts_as_printed() {
        let mut s = Scrambler::default();
        assert_eq!([s.byte(), s.byte(), s.byte()], [0xC0, 0x6D, 0x3F]);
        assert_eq!(
            Scrambler::bits(24),
            [1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1]
        );
        let mut x = [0u8; 3];
        Scrambler::apply(&mut x);
        assert_eq!(x, [0xC0, 0x6D, 0x3F]);
    }

    fn basic() -> L1Basic {
        L1Basic {
            version: 0,
            mimo_pilot_encoding: false,
            lls: true,
            time_info: 0,
            return_channel: false,
            papr: 0,
            frame_length: FrameLength::SymbolAligned {
                time_offset: 12345,
                additional_samples: 0,
            },
            subframes: 1,
            preamble_symbols: 1,
            preamble_reduced_carriers: 0,
            detail_content_tag: 1,
            detail_bytes: 25,
            detail_mode: 2,
            detail_additional_parity: 0,
            detail_cells: 774,
            first: SubframeParams {
                mimo: false,
                miso: 0,
                fft_size: 0,
                reduced_carriers: 0,
                guard_interval: 1,
                symbols: 72,
                pilot_pattern: 10,
                pilot_boost: 1,
                sbs_first: true,
                sbs_last: true,
            },
        }
    }

    fn plp(id: u8) -> Plp {
        Plp {
            id,
            lls: id == 0,
            start: 1000 * u32::from(id),
            size: 453_600,
            fec_type: 1,
            modulation: Some(0),
            code_rate: Some(6),
            fec_block_start: Some(77),
            dispersed: Some(false),
            ..Plp::default()
        }
    }

    #[test]
    fn l1_basic_is_200_bits_and_reads_back() {
        let b = basic();
        let bits = b.pack();
        assert_eq!(bits.len(), 200);
        assert_eq!(L1Basic::parse(&bits), Ok(b));
        // The version is the first field and the mode bit of the frame length the eleventh.
        assert_eq!(bits[..3], [0, 0, 0]);
        assert_eq!(bits[10], 1);
        for at in [0, 57, 150, 199] {
            let mut bad = bits.clone();
            bad[at] ^= 1;
            assert_eq!(L1Basic::parse(&bad), Err(L1Error::Crc), "bit {at}");
        }
        assert_eq!(L1Basic::parse(&bits[..199]), Err(L1Error::Short));
        let t = L1Basic {
            frame_length: FrameLength::TimeAligned {
                units_5ms: 50,
                excess_samples: 3,
            },
            subframes: 256,
            preamble_symbols: 8,
            detail_mode: 7,
            ..b
        };
        assert_eq!(L1Basic::parse(&t.pack()), Ok(t));
    }

    #[test]
    fn l1_detail_reads_back_with_every_branch() {
        let b = basic();
        let d = L1Detail {
            version: 1,
            bonded_bsids: vec![],
            time: None,
            subframes: vec![DetailSubframe {
                params: None,
                multiplex: None,
                frequency_interleaver: true,
                sbs_null_cells: Some(300),
                plps: vec![plp(0)],
            }],
            bsid: 0x1234,
        };
        let bits = d.pack(&b);
        // 7 + 1 + 13 + 6 + 89 + 16 + 32 = 164 bits, sent as the least size of 25 bytes.
        assert_eq!(bits.len(), 200);
        assert_eq!(L1Detail::parse(&bits, &b), Ok(d.clone()));
        let mut bad = bits.clone();
        bad[20] ^= 1;
        assert_eq!(L1Detail::parse(&bad, &b), Err(L1Error::Crc));
        assert_eq!(L1Detail::parse(&bits[..100], &b), Err(L1Error::Short));

        // Two subframes with three and one PLPs, time to the nanosecond, bonding, MIMO, both time interleavers, a dispersed
        // PLP and an enhanced layer.
        let mut b2 = L1Basic {
            subframes: 2,
            time_info: 3,
            ..b
        };
        let second = SubframeParams {
            mimo: true,
            fft_size: 1,
            guard_interval: 5,
            symbols: 20,
            pilot_pattern: 8,
            pilot_boost: 2,
            sbs_first: false,
            sbs_last: false,
            ..b.first
        };
        let cti = Plp {
            ti_mode: 1,
            fec_block_start: None,
            cti_fec_block_start: Some(0x2A_BCDE),
            ti_extended: Some(true),
            cti: Some((3, 1023)),
            channels_bonded: Some(1),
            bonding: Some(Bonding {
                format: 1,
                rf_ids: [1, 2, 0, 0, 0, 0, 0, 0],
                count: 2,
            }),
            ..plp(1)
        };
        let hti = Plp {
            ti_mode: 2,
            fec_block_start: None,
            modulation: Some(2),
            dispersed: Some(true),
            subslices: Some((16384, 250)),
            channels_bonded: Some(0),
            hti: Some(Hti {
                inter_subframe: true,
                ti_blocks: 2,
                fec_blocks_max: 40,
                fec_blocks: [10, 11, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                cell_interleaver: true,
            }),
            ..plp(2)
        };
        let enhanced = Plp {
            layer: 1,
            dispersed: None,
            ldm_injection_level: Some(9),
            channels_bonded: Some(0),
            mimo: Some((true, false, true)),
            ..plp(3)
        };
        let d2 = L1Detail {
            version: 1,
            bonded_bsids: vec![0xBEEF],
            time: Some(L1Time {
                seconds: 1_700_000_000,
                milliseconds: 999,
                microseconds: Some(1),
                nanoseconds: Some(500),
            }),
            subframes: vec![
                DetailSubframe {
                    params: None,
                    multiplex: Some(false),
                    frequency_interleaver: false,
                    sbs_null_cells: Some(0),
                    plps: vec![
                        Plp {
                            channels_bonded: Some(0),
                            ..plp(0)
                        },
                        cti,
                        hti,
                    ],
                },
                DetailSubframe {
                    params: Some(second),
                    multiplex: Some(false),
                    frequency_interleaver: true,
                    sbs_null_cells: None,
                    plps: vec![enhanced],
                },
            ],
            bsid: 77,
        };
        let bits = d2.pack(&b2);
        assert!(bits.len() >= 200 && bits.len().is_multiple_of(8));
        b2.detail_bytes = (bits.len() / 8) as u16;
        assert_eq!(L1Detail::parse(&bits, &b2), Ok(d2.clone()));
        // Reserved bytes before the CRC are accepted.
        let mut padded = bits[..bits.len() - 32].to_vec();
        padded.resize(padded.len() + 24, 0);
        let crc = crc32(&padded);
        put(&mut padded, u64::from(crc), 32);
        b2.detail_bytes += 3;
        assert_eq!(L1Detail::parse(&padded, &b2), Ok(d2));
        // A size that ends inside the fields does not read past it.
        let mut cut = bits[..120].to_vec();
        let crc = crc32(&cut);
        put(&mut cut, u64::from(crc), 32);
        b2.detail_bytes = (cut.len() / 8) as u16;
        assert_eq!(L1Detail::parse(&cut, &b2), Err(L1Error::Value));
        b2.detail_bytes = 2;
        assert_eq!(L1Detail::parse(&cut, &b2), Err(L1Error::Short));
    }

    /// Clause 6.5.3.1.
    #[test]
    fn l1_detail_is_cut_into_equal_blocks() {
        assert_eq!(detail_segments(200, 3072), (1, 200));
        assert_eq!(detail_segments(3072, 3072), (1, 3072));
        assert_eq!(detail_segments(3080, 3072), (2, 1544));
        assert_eq!(detail_segments(2360, 2352), (2, 1184));
        assert_eq!(detail_segments(20000, 6312), (4, 5000));
    }
}

//! The DVB-S2 transmit coding, shared by the test-signal generator and the senders. [`TsAdapter`] is the mode adaptation
//! of one transport stream: the BBHEADER, the data field of user packets whose sync byte carries the CRC-8 of the packet
//! before, and the padding. [`S2Encoder`] makes the PLFRAME symbols of a BBFRAME: BB scrambling, BCH, LDPC, the bit
//! interleaver, the constellation, the PLHEADER, the pilot blocks and the PL scrambling, and the dummy PLFRAME. The pulse
//! shaping is [`crate::transmit::TxShaper`].
//!
//! Standard: ETSI EN 302 307-1 clauses 5.1 to 5.5.

use crate::fec::S2Codec;
use crate::modcod::{Code, ModCod};
use crate::pl::{code7, header_symbols, pl_scrambling, quarter, PILOT};
use sdrkit_core::Complex32;
use sdrkit_proto_ipcast::adapt::TsFramer;
use sdrkit_proto_ipcast::bbheader::{bb_descramble, BbHeader, Mode, StreamFormat, BBHEADER_LEN};

/// The bits of a transport packet.
const UPL: u16 = 188 * 8;
/// The data symbols between two pilot blocks: 16 slots.
const PILOT_EVERY: usize = 16 * 90;
/// The symbols of a pilot block.
const PILOT_BLOCK: usize = 36;
/// The slots of a dummy PLFRAME.
const DUMMY_SLOTS: usize = 36;
/// The most symbols after a PLHEADER: QPSK, normal frame, pilots.
const MAX_BODY: usize = 33_282;

/// The RO field of MATYPE-1 for `roll_off`: 0.35, 0.25 or 0.20 (any other value signals 0.35).
pub fn roll_off_field(roll_off: f64) -> u8 {
    match roll_off {
        r if (r - 0.25).abs() < 1e-6 => 1,
        r if (r - 0.20).abs() < 1e-6 => 2,
        _ => 0,
    }
}

/// The BBHEADER of a CCM frame in normal mode with a data field of `dfl` bits: a transport stream of 188-byte packets
/// with `syncd`, or with `transport` false a generic continuous stream. `isi` is the input stream of a multistream
/// carrier, `None` on a single stream. `ro` is [`roll_off_field`].
pub fn bb_header(transport: bool, isi: Option<u8>, ro: u8, dfl: u16, syncd: u16) -> BbHeader {
    BbHeader {
        format: if transport {
            StreamFormat::Transport
        } else {
            StreamFormat::GenericContinuous
        },
        single_stream: isi.is_none(),
        ccm: true,
        issyi: false,
        npd: false,
        ext: ro,
        stream_id: isi.unwrap_or(0),
        upl: if transport { UPL } else { 0 },
        dfl,
        sync: if transport { 0x47 } else { 0 },
        syncd: if transport { syncd } else { 0 },
        mode: Mode::Normal,
        issy: None,
    }
}

/// The useful bit rate of DVB-S2 at `symbol_rate` symbols per second: the data field of a frame (K_bch − 80 bits) over
/// the symbols of its PLFRAME with the header and the pilots. A transport stream has this rate: each packet takes 188
/// bytes of the data field. `None` when the frame size has no code at the rate (9/10 short).
pub fn useful_bit_rate(symbol_rate: f64, modcod: &ModCod, short: bool, pilots: bool) -> Option<f64> {
    let c = modcod.code(short)?;
    Some(symbol_rate * (c.kbch - 8 * BBHEADER_LEN) as f64 / modcod.frame_symbols(short, pilots) as f64)
}

/// The mode adaptation of one transport stream in normal mode (EN 302 307-1 clause 5.1): packets are cut across data
/// fields, and each BBFRAME gets its header. It does not allocate after it is made.
#[derive(Clone, Debug, Default)]
pub struct TsAdapter {
    framer: TsFramer,
}

impl TsAdapter {
    /// Writes the BBHEADER and the data field into `frame` (a zeroed BBFRAME of K_bch / 8 bytes). The data field takes
    /// `data` bytes, at most the frame less the header; the bytes after it stay as padding. `next` gives the packets.
    pub fn fill(
        &mut self,
        frame: &mut [u8],
        data: usize,
        isi: Option<u8>,
        ro: u8,
        next: &mut impl FnMut() -> [u8; 188],
    ) {
        if frame.len() < BBHEADER_LEN {
            return;
        }
        let data = data.min(frame.len() - BBHEADER_LEN);
        let syncd = self
            .framer
            .fill(&mut frame[BBHEADER_LEN..BBHEADER_LEN + data], next);
        let h = bb_header(true, isi, ro, (8 * data) as u16, syncd);
        let mut hb = [0u8; BBHEADER_LEN];
        h.write(&mut hb);
        frame[..BBHEADER_LEN].copy_from_slice(&hb);
    }
}

/// The coder of the PLFRAMEs of one MODCOD, frame size and pilot choice. It allocates nothing after it is made.
pub struct S2Encoder {
    modcod: ModCod,
    short: bool,
    pilots: bool,
    codec: Option<S2Codec>,
    scr: Vec<u8>,
    frame: Vec<u8>,
    word: Vec<u8>,
    coded: Vec<u8>,
    sent: Vec<u8>,
}

impl S2Encoder {
    /// A coder of `modcod` in short or normal frames with or without pilots, PL scrambling code `gold`. A frame size
    /// without a code at the rate (9/10 short) gives a coder that makes dummy frames only ([`S2Encoder::code`] is `None`).
    pub fn new(modcod: ModCod, short: bool, pilots: bool, gold: u32) -> Self {
        let codec = S2Codec::new(&modcod, short);
        let (k, n, interleaved) = codec.as_ref().map_or((0, 0, false), |c| {
            (c.code.kldpc, c.code.nldpc, c.interleave.is_some())
        });
        let kbch = codec.as_ref().map_or(0, |c| c.code.kbch);
        let mut scr = Vec::new();
        pl_scrambling(gold, MAX_BODY, &mut scr);
        S2Encoder {
            modcod,
            short,
            pilots,
            codec,
            scr,
            frame: vec![0; kbch / 8],
            word: vec![0; k],
            coded: vec![0; n],
            sent: vec![0; if interleaved { n } else { 0 }],
        }
    }

    /// The code of the frames, `None` when the frame size has none at the rate.
    pub fn code(&self) -> Option<Code> {
        self.codec.as_ref().map(|c| c.code)
    }

    /// The symbols of a PLFRAME with data.
    pub fn frame_symbols(&self) -> usize {
        self.modcod.frame_symbols(self.short, self.pilots)
    }

    /// The BBFRAME of the next frame, K_bch / 8 bytes set to zero (empty without a code): the caller writes the BBHEADER
    /// and the data field, and what it leaves is the padding.
    pub fn bbframe(&mut self) -> &mut [u8] {
        self.frame.fill(0);
        &mut self.frame
    }

    /// Writes the PLFRAME of the BBFRAME last given by [`S2Encoder::bbframe`] into `syms` (cleared first), at unit mean
    /// power. False, with `syms` empty, when the coder has no code.
    pub fn encode(&mut self, syms: &mut Vec<Complex32>) -> bool {
        syms.clear();
        let Some(c) = self.codec.as_ref() else {
            return false;
        };
        bb_descramble(&mut self.frame);
        for (k, b) in self.frame.iter().enumerate() {
            for i in 0..8 {
                self.word[8 * k + i] = (b >> (7 - i)) & 1;
            }
        }
        if c.bch.encode(&mut self.word).is_err() || c.ldpc.encode(&self.word, &mut self.coded).is_err() {
            return false;
        }
        let sent = match &c.interleave {
            Some(p) => {
                if p.apply(&self.coded, &mut self.sent).is_err() {
                    return false;
                }
                &self.sent
            }
            None => &self.coded,
        };
        syms.extend_from_slice(&header_symbols(code7(self.modcod.id, self.short, self.pilots)));
        let mut j = 0;
        for (k, bits) in sent.chunks(c.cons.bits()).enumerate() {
            if self.pilots && k > 0 && k.is_multiple_of(PILOT_EVERY) {
                for _ in 0..PILOT_BLOCK {
                    syms.push(quarter(PILOT, self.scr[j]));
                    j += 1;
                }
            }
            let label = bits.iter().fold(0usize, |a, &v| (a << 1) | v as usize);
            syms.push(quarter(c.points[label], self.scr[j]));
            j += 1;
        }
        true
    }

    /// Writes a dummy PLFRAME into `syms` (cleared first): the PLHEADER of MODCOD 0 and 36 slots of the unmodulated
    /// carrier, PL scrambled (EN 302 307-1 clause 5.5.1).
    pub fn dummy(&self, syms: &mut Vec<Complex32>) {
        syms.clear();
        syms.extend_from_slice(&header_symbols(code7(0, false, false)));
        syms.extend(
            self.scr[..DUMMY_SLOTS * 90]
                .iter()
                .map(|&r| quarter(PILOT, r)),
        );
    }
}

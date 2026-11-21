//! TETRA Direct Mode Operation (`tetra_dmo`) on one 25 kHz channel: the synchronisation bursts (DSB) with DMAC-SYNC on SCH/S
//! and SCH/H, the normal bursts (DNB) with DMAC-DATA on SCH/F or STCH, the DM messages, the calls and the clear speech of a
//! call whose set-up was heard. Encrypted calls are reported and never decrypted.
//!
//! Standard: ETSI EN 300 396-2 (bursts, colour code), EN 300 396-3 (DM messages), EN 300 392-2 (coding, scrambling). The PDU
//! layouts here are not confirmed against EN 300 396-3; the `note` field says so.

use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::Prepared;
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::tetra::{
    decode_block, encode_block, rcpc, scrambling, NORMAL_1, NORMAL_2, STREAM_RATE, SYNC_TRAINING,
};
use crate::typed::TetraDmoFields;
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::{Declare, Published};
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo, Value};
use sdrkit_util_bits::pack::{push_u64, to_u64};

/// The stack id.
pub const ID: &str = "tetra_dmo";
/// The rate the bursts are read and made at, Hz (4 samples a symbol).
pub const TETRA_DMO_RATE: f64 = STREAM_RATE;
/// The bits of a burst.
pub const BURST_BITS: usize = 470;
/// The symbols of a slot.
pub const SLOT_SYMBOLS: usize = 255;
/// The symbols of guard before a burst.
pub const GUARD_SYMBOLS: usize = 17;
/// The preamble of a one-channel normal burst.
pub const P1: &str = "001100100011";
/// The preamble of a two-channel normal burst.
pub const P2: &str = "100110101001";
/// The preamble of a synchronisation burst.
pub const P3: &str = "000101000111";
/// No burst for this long drops the slot clock and the call, s.
pub const LOST_S: f64 = 1.5;
/// What no public document available here confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: every PDU layout of the DMAC-SYNC and DMAC-DATA (round-tripped only against this stack's own encoder): whether the SCH/H repeater address is there for every communication type, the meaning of the call type flag, the order of the DM-SETUP elements, and the colour code rule. DM-SDS and DM-INFO are named, not decoded; repeater and gateway operation, dual watch, two-frequency calls and frame 18 signalling are not followed.";

/// The 30-bit DM colour code: the 6 least significant bits of the MNI, then the 24-bit source address (EN 300 396-2 clause
/// 8.2.4.1).
pub fn colour_code(mni: u32, source: u32) -> u32 {
    ((mni & 0x3F) << 24) | (source & 0xFF_FFFF)
}

/// The name of DM message type `t`.
pub fn message_name(t: u8) -> &'static str {
    match t {
        1 => "DM-SDS OCCUPIED",
        2 => "DM-TIMING REQUEST",
        3 => "DM-TIMING ACK",
        8 => "DM-SETUP",
        9 => "DM-SETUP PRES",
        10 => "DM-CONNECT",
        11 => "DM-DISCONNECT",
        12 => "DM-CONNECT ACK",
        13 => "DM-OCCUPIED",
        14 => "DM-RELEASE",
        15 => "DM-TX CEASED",
        16 => "DM-TX REQUEST",
        17 => "DM-TX ACCEPT",
        18 => "DM-PREEMPT",
        19 => "DM-PRE ACCEPT",
        20 => "DM-REJECT",
        21 => "DM-INFO",
        22 => "DM-SDS UDATA",
        23 => "DM-SDS DATA",
        24 => "DM-SDS ACK",
        25 => "gateway message",
        30 | 31 => "proprietary",
        _ => "reserved",
    }
}

/// The DMAC-SYNC PDU of SCH/S (60 bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncS {
    /// System code.
    pub system_code: u8,
    /// Communication type: 0 direct, 1 via DM-REP, 2 via DM-GATE, 3 via DM-REP/GATE.
    pub comm_type: u8,
    /// Master/slave link flag.
    pub master_slave: bool,
    /// Gateway generated message flag.
    pub gateway: bool,
    /// A/B channel usage.
    pub ab_usage: u8,
    /// Slot 1 to 4.
    pub slot: u8,
    /// Frame 1 to 18.
    pub frame: u8,
    /// Air interface encryption state (0 clear).
    pub aie: u8,
    /// Time variant parameter (29 bits).
    pub tvp: u32,
    /// KSG number.
    pub ksg: u8,
    /// Key number.
    pub key: u8,
}

impl SyncS {
    /// The 60 bits.
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(60);
        push_u64(&mut b, u64::from(self.system_code), 4);
        push_u64(&mut b, 0, 2);
        push_u64(&mut b, u64::from(self.comm_type), 2);
        push_u64(&mut b, u64::from(self.master_slave), 1);
        push_u64(&mut b, u64::from(self.gateway), 1);
        push_u64(&mut b, u64::from(self.ab_usage), 2);
        push_u64(&mut b, u64::from(self.slot.saturating_sub(1)), 2);
        push_u64(&mut b, u64::from(self.frame), 5);
        push_u64(&mut b, u64::from(self.aie), 2);
        if self.aie != 0 {
            push_u64(&mut b, u64::from(self.tvp), 29);
            push_u64(&mut b, 0, 1);
            push_u64(&mut b, u64::from(self.ksg), 4);
            push_u64(&mut b, u64::from(self.key), 5);
        } else {
            push_u64(&mut b, 0, 39);
        }
        b
    }

    /// The PDU of 60 bits; `None` for another sync PDU type or a frame number outside 1 to 18.
    pub fn from_bits(b: &[u8]) -> Option<SyncS> {
        let f = |a: usize, n: usize| to_u64(&b[a..a + n]);
        if b.len() < 60 || f(4, 2) != 0 {
            return None;
        }
        let frame = f(14, 5) as u8;
        if !(1..=18).contains(&frame) {
            return None;
        }
        let aie = f(19, 2) as u8;
        Some(SyncS {
            system_code: f(0, 4) as u8,
            comm_type: f(6, 2) as u8,
            master_slave: f(8, 1) == 1,
            gateway: f(9, 1) == 1,
            ab_usage: f(10, 2) as u8,
            slot: f(12, 2) as u8 + 1,
            frame,
            aie,
            tvp: if aie != 0 { f(21, 29) as u32 } else { 0 },
            ksg: if aie != 0 { f(51, 4) as u8 } else { 0 },
            key: if aie != 0 { f(55, 5) as u8 } else { 0 },
        })
    }
}

/// The elements of DM-SETUP and DM-SETUP PRES.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Setup {
    /// Timing flag.
    pub timing: bool,
    /// LCH in frame 18 flag.
    pub lch_frame18: bool,
    /// Pre-emption flag.
    pub preemption: bool,
    /// Power class.
    pub power_class: u8,
    /// Power control flag.
    pub power_control: bool,
    /// Dual watch synchronisation flag.
    pub dual_watch: bool,
    /// Two-frequency call flag.
    pub two_frequency: bool,
    /// Circuit mode type.
    pub circuit_mode: u8,
    /// Priority level.
    pub priority: u8,
    /// End-to-end encryption flag.
    pub e2e: bool,
    /// Call type flag (1: a group call).
    pub group: bool,
    /// External source flag.
    pub external: bool,
}

impl Setup {
    fn push(&self, b: &mut Vec<u8>) {
        push_u64(b, u64::from(self.timing), 1);
        push_u64(b, u64::from(self.lch_frame18), 1);
        push_u64(b, u64::from(self.preemption), 1);
        push_u64(b, u64::from(self.power_class), 3);
        push_u64(b, u64::from(self.power_control), 1);
        push_u64(b, 0, 2);
        push_u64(b, u64::from(self.dual_watch), 1);
        push_u64(b, u64::from(self.two_frequency), 1);
        push_u64(b, u64::from(self.circuit_mode), 4);
        push_u64(b, 0, 4);
        push_u64(b, u64::from(self.priority), 2);
        push_u64(b, u64::from(self.e2e), 1);
        push_u64(b, u64::from(self.group), 1);
        push_u64(b, u64::from(self.external), 1);
        push_u64(b, 0, 2);
    }
    fn read(b: &[u8]) -> Setup {
        let f = |a: usize, n: usize| to_u64(&b[a..a + n]);
        Setup {
            timing: f(0, 1) == 1,
            lch_frame18: f(1, 1) == 1,
            preemption: f(2, 1) == 1,
            power_class: f(3, 3) as u8,
            power_control: f(6, 1) == 1,
            dual_watch: f(9, 1) == 1,
            two_frequency: f(10, 1) == 1,
            circuit_mode: f(11, 4) as u8,
            priority: f(19, 2) as u8,
            e2e: f(21, 1) == 1,
            group: f(22, 1) == 1,
            external: f(23, 1) == 1,
        }
    }
}

/// The bits of the setup elements.
const SETUP_BITS: usize = 26;

/// The addressing and message of SCH/H (124 bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncH {
    /// Repeater address.
    pub repeater: u16,
    /// Fill bit indication.
    pub fill: bool,
    /// Number of SCH/F slots when fragmented.
    pub fragment_slots: Option<u8>,
    /// Frame countdown.
    pub countdown: u8,
    /// Destination address, `None` for address type 2 (no address).
    pub destination: Option<u32>,
    /// Source address, `None` for address type 2.
    pub source: Option<u32>,
    /// MNI: MCC (10 bits) and MNC (14 bits).
    pub mni: u32,
    /// Message type.
    pub message: u8,
    /// The set-up elements of DM-SETUP and DM-SETUP PRES.
    pub setup: Option<Setup>,
}

fn push_address(b: &mut Vec<u8>, a: Option<u32>) {
    match a {
        Some(v) => {
            push_u64(b, 0, 2);
            push_u64(b, u64::from(v), 24);
        }
        None => push_u64(b, 2, 2),
    }
}

/// Reads an address type and address at `*at`.
fn read_address(b: &[u8], at: &mut usize) -> Option<Option<u32>> {
    let t = to_u64(b.get(*at..*at + 2)?);
    *at += 2;
    if t == 2 {
        return Some(None);
    }
    let v = to_u64(b.get(*at..*at + 24)?) as u32;
    *at += 24;
    Some(Some(v))
}

impl SyncH {
    /// The 124 bits.
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(124);
        push_u64(&mut b, u64::from(self.repeater), 10);
        push_u64(&mut b, u64::from(self.fill), 1);
        push_u64(&mut b, u64::from(self.fragment_slots.is_some()), 1);
        if let Some(n) = self.fragment_slots {
            push_u64(&mut b, u64::from(n), 4);
        }
        push_u64(&mut b, u64::from(self.countdown), 2);
        push_address(&mut b, self.destination);
        push_address(&mut b, self.source);
        push_u64(&mut b, u64::from(self.mni), 24);
        push_u64(&mut b, u64::from(self.message), 5);
        if let Some(s) = &self.setup {
            s.push(&mut b);
        }
        b.resize(124, 0);
        b
    }

    /// The fields of 124 bits.
    pub fn from_bits(b: &[u8]) -> Option<SyncH> {
        let f = |a: usize, n: usize| b.get(a..a + n).map(to_u64);
        let mut at = 12;
        let fragment_slots = if f(11, 1)? == 1 {
            at += 4;
            Some(f(12, 4)? as u8)
        } else {
            None
        };
        let countdown = f(at, 2)? as u8;
        at += 2;
        let destination = read_address(b, &mut at)?;
        let source = read_address(b, &mut at)?;
        let mni = f(at, 24)? as u32;
        at += 24;
        let message = f(at, 5)? as u8;
        at += 5;
        let setup = matches!(message, 8 | 9)
            .then(|| b.get(at..at + SETUP_BITS).map(Setup::read))
            .flatten();
        Some(SyncH {
            repeater: f(0, 10)? as u16,
            fill: f(10, 1)? == 1,
            fragment_slots,
            countdown,
            destination,
            source,
            mni,
            message,
            setup,
        })
    }
}

/// The DMAC-DATA header of SCH/F or STCH.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Data {
    /// A null PDU: nothing follows.
    pub null: bool,
    /// Frame countdown.
    pub countdown: u8,
    /// Air interface encryption state.
    pub aie: u8,
    /// Destination address.
    pub destination: Option<u32>,
    /// Source address.
    pub source: Option<u32>,
    /// MNI (communication types 0 and 1).
    pub mni: Option<u32>,
    /// Message type.
    pub message: u8,
}

impl Data {
    /// The bits, filled to `len`.
    pub fn bits(&self, len: usize) -> Vec<u8> {
        let mut b = Vec::with_capacity(len);
        push_u64(&mut b, 0, 2);
        push_u64(&mut b, 0, 3);
        push_u64(&mut b, u64::from(self.null), 1);
        push_u64(&mut b, u64::from(self.countdown), 2);
        push_u64(&mut b, u64::from(self.aie), 2);
        push_address(&mut b, self.destination);
        push_address(&mut b, self.source);
        if let Some(m) = self.mni {
            push_u64(&mut b, u64::from(m), 24);
        }
        push_u64(&mut b, u64::from(self.message), 5);
        b.resize(len, 0);
        b
    }

    /// The header of `b` for communication type `comm_type`; `None` for another MAC PDU type.
    pub fn from_bits(b: &[u8], comm_type: u8) -> Option<Data> {
        let f = |a: usize, n: usize| b.get(a..a + n).map(to_u64);
        if f(0, 2)? != 0 {
            return None;
        }
        if f(5, 1)? == 1 {
            return Some(Data {
                null: true,
                ..Data::default()
            });
        }
        let mut at = 10;
        let destination = read_address(b, &mut at)?;
        let source = read_address(b, &mut at)?;
        let mni = if comm_type <= 1 {
            let m = f(at, 24)? as u32;
            at += 24;
            Some(m)
        } else {
            None
        };
        Some(Data {
            null: false,
            countdown: f(6, 2)? as u8,
            aie: f(8, 2)? as u8,
            destination,
            source,
            mni,
            message: f(at, 5)? as u8,
        })
    }
}

/// The MCC and MNC of an MNI.
pub fn mcc_mnc(mni: u32) -> (u32, u32) {
    (mni >> 14, mni & 0x3FFF)
}

fn bits_of(s: &str) -> Vec<u8> {
    s.bytes().map(|c| c - b'0').collect()
}

/// The phase step of a dibit, in quarters of π: 00 +1, 01 +3, 11 -3, 10 -1.
fn step(d0: u8, d1: u8) -> i32 {
    match (d0, d1) {
        (0, 0) => 1,
        (0, 1) => 3,
        (1, 1) => -3,
        _ => -1,
    }
}

/// Sets the phase adjustment bits 12 and 13 so that the phase steps of symbols 7 to 126 (1-based) sum to a multiple of 2π.
pub fn adjust_phase(b: &mut [u8]) {
    let rest: i32 = (7..126).map(|s| step(b[2 * s], b[2 * s + 1])).sum();
    for (d0, d1) in [(0, 0), (0, 1), (1, 1), (1, 0)] {
        if (rest + step(d0, d1)).rem_euclid(8) == 0 {
            b[12] = d0;
            b[13] = d1;
            return;
        }
    }
}

/// A synchronisation burst (470 bits) of a coded SCH/S (120) and SCH/H (216).
pub fn sync_burst(sch_s: &[u8], sch_h: &[u8]) -> Vec<u8> {
    let mut b = bits_of(P3);
    b.extend([0, 0]);
    b.extend([1u8; 8]);
    b.extend([0u8; 64]);
    b.extend([1u8; 8]);
    b.extend_from_slice(sch_s);
    b.extend(bits_of(SYNC_TRAINING));
    b.extend_from_slice(sch_h);
    b.extend([0, 0]);
    adjust_phase(&mut b);
    b
}

/// A normal burst (470 bits) of two blocks of 216 bits: one channel (training n) or two (training p).
pub fn normal_burst(b1: &[u8], b2: &[u8], two_channels: bool) -> Vec<u8> {
    let mut b = bits_of(if two_channels { P2 } else { P1 });
    b.extend([0, 0]);
    b.extend_from_slice(b1);
    b.extend(bits_of(if two_channels { NORMAL_2 } else { NORMAL_1 }));
    b.extend_from_slice(b2);
    b.extend([0, 0]);
    adjust_phase(&mut b);
    b
}

/// Scrambles (or descrambles) bits with the sequence of colour code `colour`.
pub fn scramble(bits: &mut [u8], colour: u32) {
    let mut pn = vec![0u8; bits.len()];
    scrambling(colour, &mut pn);
    for (b, p) in bits.iter_mut().zip(pn) {
        *b ^= p;
    }
}

// ------------------------------------------------------------------------------------------------------------- results

// ------------------------------------------------------------------------------------------------------------ protocol

const KEY: u32 = 1;
/// The symbols before the training sequence that a burst read needs: from symbol 1 (bit 2).
const DSB_BEFORE: usize = 106;
const DNB_BEFORE: usize = 114;
/// The symbols read of a burst: bits 2 to 469.
const READ_SYMBOLS: usize = 234;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Burst {
    Sync,
    Normal { two_channels: bool },
}

/// The call open now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Call {
    source: u32,
    destination: u32,
    mni: u32,
    /// A set-up was heard (and its elements).
    setup: Option<Setup>,
    aie: u8,
}

/// The TETRA DMO protocol.
pub struct TetraDmo {
    buf: Vec<Sym>,
    burst: Option<Burst>,
    conv: ConvChannel,
    sync: Option<SyncS>,
    colour: Option<u32>,
    call: Option<Call>,
    last_burst_s: f64,
    dsb: u64,
    dnb: u64,
    good: u64,
    bad: u64,
    traffic: u64,
    refused: u64,
    last_source: Option<u32>,
    last_destination: Option<u32>,
    mni: Option<u32>,
}

impl Default for TetraDmo {
    fn default() -> Self {
        TetraDmo {
            buf: Vec::with_capacity(READ_SYMBOLS),
            burst: None,
            conv: rcpc(),
            sync: None,
            colour: None,
            call: None,
            last_burst_s: 0.0,
            dsb: 0,
            dnb: 0,
            good: 0,
            bad: 0,
            traffic: 0,
            refused: 0,
            last_source: None,
            last_destination: None,
            mni: None,
        }
    }
}

fn soft_bits(s: &[Sym]) -> Vec<f32> {
    let mut v = Vec::with_capacity(s.len() * 2);
    for x in s {
        let (a, b) = x.soft_dibit();
        v.push(a);
        v.push(b);
    }
    v
}

fn addr_text(a: Option<u32>) -> String {
    a.map_or_else(|| "-".to_string(), |v| v.to_string())
}

impl TetraDmo {
    fn end_call(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) {
            ctx.end_call(KEY, true);
        }
        self.call = None;
        self.colour = None;
    }

    /// A DM message from a DSB (with its set-up elements) or a DNB.
    #[allow(clippy::too_many_arguments)]
    fn message(
        &mut self,
        burst: &str,
        message: u8,
        source: Option<u32>,
        destination: Option<u32>,
        mni: Option<u32>,
        setup: Option<Setup>,
        aie: u8,
        ctx: &mut Ctx,
    ) {
        let name = message_name(message);
        let s = self.sync.unwrap_or_default();
        self.last_source = source.or(self.last_source);
        self.last_destination = destination.or(self.last_destination);
        if mni.is_some() {
            self.mni = mni;
        }
        let mut text = format!(
            "{name}: from {} to {}",
            addr_text(source),
            addr_text(destination)
        );
        if let Some(m) = mni {
            let (mcc, mnc) = mcc_mnc(m);
            text.push_str(&format!(", MNI {mcc}/{mnc}"));
        }
        if let Some(st) = &setup {
            text.push_str(&format!(
                ", {}, circuit {}, priority {}",
                if st.group { "group" } else { "individual" },
                st.circuit_mode,
                st.priority
            ));
            if st.e2e {
                text.push_str(", end-to-end encrypted");
            }
        }
        if aie != 0 {
            text.push_str(&format!(", air interface encryption {aie}"));
        }
        ctx.control(ControlRecord {
            opcode: u32::from(message),
            name: format!("{burst} {name}"),
            text,
            time: ctx.now,
            slot: s.slot,
            ..Default::default()
        });
        match message {
            8 | 9 | 12 | 13 => {
                let (Some(src), Some(m)) = (source, mni.or(self.mni)) else {
                    return;
                };
                let dst = destination.unwrap_or(0);
                self.colour = Some(colour_code(m, src));
                let same = self
                    .call
                    .is_some_and(|c| c.source == src && c.destination == dst && c.mni == m);
                if !same {
                    if ctx.has_call(KEY) {
                        ctx.end_call(KEY, true);
                    }
                    self.call = Some(Call {
                        source: src,
                        destination: dst,
                        mni: m,
                        setup: None,
                        aie,
                    });
                }
                if let Some(c) = self.call.as_mut() {
                    if setup.is_some() {
                        c.setup = setup;
                    }
                    c.aie = c.aie.max(aie);
                }
                let c = self.call.unwrap_or_default();
                let e2e = c.setup.is_some_and(|s| s.e2e);
                let encrypted = c.aie != 0 || e2e;
                let enc = if encrypted {
                    Encryption::encrypted(
                        Some(u32::from(c.aie)),
                        if e2e { "end-to-end" } else { "air interface" },
                        None,
                    )
                } else if c.setup.is_some() {
                    Encryption::clear()
                } else {
                    Encryption::default()
                };
                let group = c.setup.is_none_or(|s| s.group);
                if !ctx.has_call(KEY) {
                    let (mcc, mnc) = mcc_mnc(m);
                    let mut rec = CallRecord {
                        kind: if group {
                            CallKind::GroupVoice
                        } else {
                            CallKind::PrivateVoice
                        },
                        src: Ident::Number(u64::from(src)),
                        dst: Ident::Number(u64::from(dst)),
                        group,
                        encryption: enc.clone(),
                        slot: s.slot,
                        ..Default::default()
                    };
                    rec.set(CallDetail::Mcc, Value::I64(i64::from(mcc)));
                    rec.set(CallDetail::Mnc, Value::I64(i64::from(mnc)));
                    rec.set(CallDetail::CommMode, Value::I64(i64::from(s.comm_type)));
                    ctx.start_call(KEY, rec, false);
                }
                if let Some(st) = c.setup {
                    ctx.update_call(KEY, |r| {
                        r.group = st.group;
                        r.kind = if st.group {
                            CallKind::GroupVoice
                        } else {
                            CallKind::PrivateVoice
                        };
                        r.set(
                            CallDetail::CallType,
                            Value::Text(format!(
                                "circuit mode {}, priority {}{}",
                                st.circuit_mode,
                                st.priority,
                                if st.e2e {
                                    ", end-to-end encryption"
                                } else {
                                    ""
                                }
                            )),
                        );
                    });
                }
                if enc != Encryption::default() {
                    ctx.set_encryption(KEY, enc);
                }
            }
            14 | 15 => self.end_call(ctx),
            _ => {}
        }
    }

    fn sync_burst(&mut self, ctx: &mut Ctx) {
        let b = soft_bits(&self.buf);
        let s = decode_block(&mut self.conv, &b[92..212], 60, 11, 0)
            .and_then(|(info, _)| SyncS::from_bits(&info));
        let Some(s) = s else {
            self.bad += 1;
            ctx.block(false, 120, 0);
            return;
        };
        ctx.block(true, 120, 0);
        ctx.identity(true);
        self.dsb += 1;
        self.sync = Some(s);
        let Some((info, _)) = decode_block(&mut self.conv, &b[250..466], 124, 101, 0) else {
            self.bad += 1;
            ctx.block(false, 216, 0);
            return;
        };
        ctx.block(true, 216, 0);
        let Some(h) = SyncH::from_bits(&info) else {
            return;
        };
        self.message(
            "DSB",
            h.message,
            h.source,
            h.destination,
            Some(h.mni),
            h.setup,
            s.aie,
            ctx,
        );
    }

    fn data(&mut self, info: &[u8], ctx: &mut Ctx) {
        let comm = self.sync.map_or(0, |s| s.comm_type);
        let Some(d) = Data::from_bits(info, comm) else {
            return;
        };
        if d.null || matches!(d.message, 8 | 9) {
            return;
        }
        self.message(
            "DNB",
            d.message,
            d.source,
            d.destination,
            d.mni,
            None,
            d.aie,
            ctx,
        );
    }

    /// Speech of a traffic burst: the 432 descrambled bits, the first 137 of each block of 216.
    fn traffic(&mut self, hard: &mut [u8], colour: u32, ctx: &mut Ctx) {
        self.traffic += 1;
        scramble(hard, colour);
        let clear = self
            .call
            .is_some_and(|c| c.aie == 0 && c.setup.is_some_and(|s| !s.e2e))
            && self.sync.is_none_or(|s| s.aie == 0);
        if !clear || !ctx.has_call(KEY) {
            self.refused += 1;
            return;
        }
        ctx.update_call(KEY, |c| c.voice_frames += 2);
        let slot = self.sync.map_or(1, |s| s.slot);
        for (i, blk) in hard.chunks(216).take(2).enumerate() {
            let spec = crate::voice::FrameSpec {
                codec: Codec::TetraAcelp,
                packaging: sdrkit_graph::SpeechPackaging::Tchs137,
                codec_mode: None,
                duration_ms: 30,
                stack: ID,
                carrier_hz: ctx.carrier_hz,
                slot,
            };
            ctx.speech(
                KEY,
                spec,
                &blk[..137],
                ctx.now + i as f64 * 0.03,
                false,
                None,
            );
        }
    }

    fn normal_burst(&mut self, two_channels: bool, ctx: &mut Ctx) {
        let Some(colour) = self.colour else {
            return;
        };
        self.dnb += 1;
        let b = soft_bits(&self.buf);
        let (b1, b2) = (&b[12..228], &b[250..466]);
        if !two_channels {
            let mut joined = b1.to_vec();
            joined.extend_from_slice(b2);
            if let Some((info, _)) = decode_block(&mut self.conv, &joined, 268, 103, colour) {
                self.good += 1;
                ctx.block(true, 432, 0);
                self.data(&info, ctx);
                return;
            }
            let mut hard: Vec<u8> = joined.iter().map(|&v| u8::from(v > 0.0)).collect();
            self.traffic(&mut hard, colour, ctx);
            return;
        }
        for (i, half) in [b1, b2].into_iter().enumerate() {
            match decode_block(&mut self.conv, half, 124, 101, colour) {
                Some((info, _)) => {
                    self.good += 1;
                    self.data(&info, ctx);
                }
                None if i == 1 => {
                    self.refused += 1;
                }
                None => ctx.block(false, 216, 0),
            }
        }
    }
}

impl Protocol for TetraDmo {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: ID,
            name: "TETRA DMO",
            baud: 18_000.0,
            two_level: false,
            channel_hz: 25_000.0,
            calls: &crate::results::CALLS_300,
            message_cap: 400,
            polarity_rule: false,
            clock_loop: false,
            extra_baud: None,
            stream_rate: STREAM_RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        let mk = |id: u8, s: &str, before: usize| {
            let mut p = SyncPattern::from_hex_dibits(id, "0", before);
            p.symbols = s
                .as_bytes()
                .chunks(2)
                .map(|c| {
                    sdrkit_algo_fec::conv::tcm::dibit_symbol(((c[0] - b'0') << 1) | (c[1] - b'0'))
                })
                .collect();
            p.max_err = p.symbols.len() / 8;
            p.no_auto_invert()
        };
        vec![
            mk(0, SYNC_TRAINING, DSB_BEFORE),
            mk(1, NORMAL_1, DNB_BEFORE),
            mk(2, NORMAL_2, DNB_BEFORE),
        ]
    }

    fn on_sync(
        &mut self,
        id: u8,
        _inv: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        if tracking
            && self.burst.is_some()
            && self.buf.len() > 5
            && self.buf.len() < READ_SYMBOLS - 10
        {
            return SyncAnswer::Ignore;
        }
        self.burst = Some(match id {
            0 => Burst::Sync,
            1 => Burst::Normal {
                two_channels: false,
            },
            _ => Burst::Normal { two_channels: true },
        });
        self.buf.clear();
        self.buf.extend_from_slice(pre);
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        let Some(b) = self.burst else {
            return SymAnswer::Stop;
        };
        self.buf.push(s);
        if self.buf.len() < READ_SYMBOLS {
            return SymAnswer::Continue;
        }
        self.burst = None;
        self.last_burst_s = ctx.now;
        match b {
            Burst::Sync => self.sync_burst(ctx),
            Burst::Normal { two_channels } => self.normal_burst(two_channels, ctx),
        }
        SymAnswer::Stop
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if self.call.is_some() && ctx.now - self.last_burst_s > LOST_S {
            self.end_call(ctx);
            self.sync = None;
        }
    }

    fn on_reset(&mut self, ctx: &mut Ctx) {
        self.burst = None;
        self.buf.clear();
        self.end_call(ctx);
        self.sync = None;
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <TetraDmoFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        let last_source = self.last_source.map(|v| v.to_string());
        let last_destination = self.last_destination.map(|v| v.to_string());
        let mni = self.mni.map(|m| {
            let (mcc, mnc) = mcc_mnc(m);
            format!("{mcc}/{mnc}")
        });
        r.put(&TetraDmoFields {
            dsb: self.dsb as i64,
            dnb: self.dnb as i64,
            data_blocks: self.good as i64,
            sync_blocks_bad: self.bad as i64,
            traffic_bursts: self.traffic as i64,
            traffic_refused: self.refused as i64,
            comm_type: self.sync.map(|s| i64::from(s.comm_type)),
            frame: self.sync.map(|s| i64::from(s.frame)),
            slot: self.sync.map(|s| i64::from(s.slot)),
            aie_state: self.sync.map(|s| i64::from(s.aie)),
            last_source: last_source.as_deref(),
            last_destination: last_destination.as_deref(),
            mni: mni.as_deref(),
            note: NOT_CONFIRMED,
        });
    }

    fn codec(&self) -> Codec {
        Codec::TetraAcelp
    }
}

// ----------------------------------------------------------------------------------------------------------- generator

/// The test call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestCall {
    /// Communication type.
    pub comm_type: u8,
    /// A group call.
    pub group: bool,
    /// Air interface encryption state.
    pub aie: u8,
    /// End-to-end encryption in the set-up.
    pub e2e: bool,
    /// Traffic bursts.
    pub traffic: usize,
    /// DSBs with DM-SETUP.
    pub setup_repeats: usize,
    /// The source address.
    pub source: u32,
    /// The destination address.
    pub destination: u32,
    /// The MNI.
    pub mni: u32,
}

impl Default for TestCall {
    fn default() -> Self {
        TestCall {
            comm_type: 0,
            group: true,
            aie: 0,
            e2e: false,
            traffic: 40,
            setup_repeats: 4,
            source: 0x1A2B3C,
            destination: 0x4D5E6F,
            mni: (262 << 14) | 1001,
        }
    }
}

/// The scheduled bursts of the test call (`None` for an empty slot), and the 432 type-4 bits of each traffic burst before
/// scrambling.
pub fn test_bursts(c: &TestCall, seed: u64) -> (Vec<Option<Vec<u8>>>, Vec<Vec<u8>>) {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let setup = Setup {
        timing: true,
        power_class: 4,
        circuit_mode: 1,
        priority: 2,
        e2e: c.e2e,
        group: c.group,
        ..Setup::default()
    };
    let colour = colour_code(c.mni, c.source);
    let sch_h = |message: u8, countdown: u8, with_setup: bool| SyncH {
        countdown,
        destination: Some(c.destination),
        source: Some(c.source),
        mni: c.mni,
        message,
        setup: with_setup.then_some(setup),
        ..SyncH::default()
    };
    let dsb = |frame: u8, h: SyncH, rng: &mut sdrkit_algo_sim::rng::Rng| {
        let s = SyncS {
            comm_type: c.comm_type,
            slot: 1,
            frame,
            aie: c.aie,
            tvp: if c.aie != 0 {
                (rng.next_u64() & 0x1FFF_FFFF) as u32
            } else {
                0
            },
            ksg: if c.aie != 0 { 1 } else { 0 },
            key: if c.aie != 0 { 1 } else { 0 },
            ..SyncS::default()
        };
        sync_burst(
            &encode_block(&s.bits(), 11, 0),
            &encode_block(&h.bits(), 101, 0),
        )
    };
    let empty = |v: &mut Vec<Option<Vec<u8>>>, n: usize| v.extend(std::iter::repeat_n(None, n));
    let mut slots = Vec::new();
    let mut sent = Vec::new();
    empty(&mut slots, 4);
    let mut frame = 1u8;
    let next = |f: &mut u8| {
        *f = *f % 18 + 1;
    };
    for k in 0..c.setup_repeats {
        let countdown = (c.setup_repeats - 1 - k).min(3) as u8;
        slots.push(Some(dsb(frame, sch_h(8, countdown, true), &mut rng)));
        empty(&mut slots, 3);
        next(&mut frame);
    }
    let mut done = 0;
    while done < c.traffic {
        if frame == 18 {
            slots.push(Some(dsb(frame, sch_h(13, 0, false), &mut rng)));
        } else {
            let bits: Vec<u8> = (0..432).map(|_| (rng.next_u64() & 1) as u8).collect();
            let mut s = bits.clone();
            scramble(&mut s, colour);
            sent.push(bits);
            slots.push(Some(normal_burst(&s[..216], &s[216..], false)));
            done += 1;
        }
        empty(&mut slots, 3);
        next(&mut frame);
    }
    slots.push(Some(dsb(frame, sch_h(15, 0, false), &mut rng)));
    empty(&mut slots, 7);
    (slots, sent)
}

/// The samples at 72 kHz of scheduled bursts: each burst after the guard of its slot, empty slots silent.
pub fn render_slots(slots: &[Option<Vec<u8>>]) -> Vec<Complex32> {
    let sps = 4;
    let slot_len = SLOT_SYMBOLS * sps;
    let mut out = vec![Complex32::new(0.0, 0.0); slots.len() * slot_len + 64 * sps];
    for (i, s) in slots.iter().enumerate() {
        let Some(bits) = s else { continue };
        let mut m = crate::dqpsk::DqpskModulator::new(18_000.0, STREAM_RATE, 0.35);
        let mut v = Vec::new();
        m.bits(bits, &mut v);
        m.flush(&mut v);
        let at = i * slot_len + GUARD_SYMBOLS * sps;
        for (k, z) in v.into_iter().enumerate() {
            if let Some(o) = out.get_mut(at + k) {
                *o += z * 0.5;
            }
        }
    }
    out
}

/// The settings of the test call.
pub mod settings {
    use sdrkit_core::publish::field;
    use sdrkit_core::{DeclareSetting, Setting};

    /// The communication type.
    pub const GEN_COMM_TYPE: Setting<field::I64> =
        Setting::new("gen_comm_type", "Test signal communication type", "", 0)
            .range(0.0, 3.0, 1.0)
            .section("Test signal")
            .advanced();
    /// A group call.
    pub const GEN_GROUP: Setting<field::Bool> =
        Setting::new("gen_group", "Test signal group call", "", true)
            .section("Test signal")
            .advanced();
    /// The air interface encryption state.
    pub const GEN_AI_ENC: Setting<field::I64> =
        Setting::new("gen_ai_enc", "Test signal encryption state", "", 0)
            .range(0.0, 3.0, 1.0)
            .section("Test signal")
            .advanced();
    /// End-to-end encryption in the set-up.
    pub const GEN_E2E: Setting<field::Bool> =
        Setting::new("gen_e2e", "Test signal end-to-end encryption", "", false)
            .section("Test signal")
            .advanced();
    /// Traffic bursts.
    pub const GEN_TRAFFIC_FRAMES: Setting<field::I64> =
        Setting::new("gen_traffic_frames", "Test signal traffic bursts", "", 40)
            .range(0.0, 1000.0, 1.0)
            .section("Test signal")
            .advanced();
    /// DSBs with DM-SETUP.
    pub const GEN_SETUP_REPEATS: Setting<field::I64> =
        Setting::new("gen_setup_repeats", "Test signal set-up bursts", "", 4)
            .range(1.0, 18.0, 1.0)
            .section("Test signal")
            .advanced();

    /// The test signal settings.
    pub const SETTINGS: &[&dyn DeclareSetting] = &[
        &GEN_COMM_TYPE,
        &GEN_GROUP,
        &GEN_AI_ENC,
        &GEN_E2E,
        &GEN_TRAFFIC_FRAMES,
        &GEN_SETUP_REPEATS,
    ];
}

/// The test call of the settings of `cfg`.
pub fn test_call(cfg: &GenConfig) -> TestCall {
    use settings::*;
    let s = &cfg.settings;
    let has = |key: &str| s.schema().id(key).is_ok();
    TestCall {
        comm_type: if has("gen_comm_type") {
            s.get(&GEN_COMM_TYPE).clamp(0, 3) as u8
        } else {
            0
        },
        group: !has("gen_group") || s.get(&GEN_GROUP),
        aie: if has("gen_ai_enc") {
            s.get(&GEN_AI_ENC).clamp(0, 3) as u8
        } else {
            0
        },
        e2e: has("gen_e2e") && s.get(&GEN_E2E),
        traffic: if has("gen_traffic_frames") {
            s.get(&GEN_TRAFFIC_FRAMES).clamp(0, 1000) as usize
        } else {
            40
        },
        setup_repeats: if has("gen_setup_repeats") {
            s.get(&GEN_SETUP_REPEATS).clamp(1, 18) as usize
        } else {
            4
        },
        ..TestCall::default()
    }
}

/// The looping test signal at the rate of `cfg`, the channel `channel_hz` from the centre.
pub struct DmoGenerator {
    inner: Prepared,
    nco: sdrkit_algo_dsp::nco::Nco,
}

/// The test call of `cfg` at the rate of `cfg`, repeated, the channel at 0 Hz.
pub fn prepared(cfg: &GenConfig) -> Prepared {
    let (slots, _) = test_bursts(&test_call(cfg), cfg.seed);
    Prepared::with_source_rate(render_slots(&slots), STREAM_RATE, cfg.rate_hz, true)
        .with_info(25_000.0, 18_000.0, 36_000.0)
}

impl DmoGenerator {
    /// The test call of `cfg`, repeated, with the channel at `channel_hz`.
    pub fn new(cfg: &GenConfig, channel_hz: f64) -> DmoGenerator {
        let inner = prepared(cfg);
        DmoGenerator {
            inner,
            nco: sdrkit_algo_dsp::nco::Nco::new(cfg.rate_hz, channel_hz),
        }
    }
}

impl Generator for DmoGenerator {
    fn rate_hz(&self) -> Hz {
        self.inner.rate_hz()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The bursts are sent at half amplitude.
        self.inner.signal_info().map(|i| i.with_power(0.25))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = self.inner.generate(out);
        self.nco.mix_in_place(&mut out[..n]);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colour_code_of_the_example() {
        assert_eq!(colour_code((262 << 14) + 1001, 0x1A2B3C), 0x291A_2B3C);
    }

    #[test]
    fn sync_s_round_trip() {
        let s = SyncS {
            comm_type: 1,
            master_slave: true,
            slot: 3,
            frame: 17,
            aie: 2,
            tvp: 0x1F00BA,
            ksg: 5,
            key: 9,
            ab_usage: 1,
            ..SyncS::default()
        };
        let b = s.bits();
        assert_eq!(b.len(), 60);
        assert_eq!(SyncS::from_bits(&b), Some(s));
    }

    #[test]
    fn sync_h_round_trip() {
        let h = SyncH {
            countdown: 2,
            destination: Some(0xABCDEF),
            source: Some(0x123456),
            mni: (262 << 14) | 42,
            message: 8,
            setup: Some(Setup {
                e2e: true,
                group: false,
                circuit_mode: 2,
                priority: 3,
                power_class: 2,
                ..Setup::default()
            }),
            ..SyncH::default()
        };
        let b = h.bits();
        assert_eq!(b.len(), 124);
        assert_eq!(SyncH::from_bits(&b), Some(h));
    }

    #[test]
    fn coding_with_colour_zero_and_another() {
        let mut info = vec![0u8; 60];
        info[5] = 1;
        info[40] = 1;
        let coded = encode_block(&info, 11, 0);
        let soft: Vec<f32> = coded
            .iter()
            .map(|&b| if b == 1 { 1.0 } else { -1.0 })
            .collect();
        let mut conv = rcpc();
        assert_eq!(
            decode_block(&mut conv, &soft, 60, 11, 0).map(|x| x.0),
            Some(info.clone())
        );
        assert!(decode_block(&mut conv, &soft, 60, 11, colour_code(1, 2)).is_none());
    }

    #[test]
    fn bursts_have_their_training_and_phase() {
        let n = normal_burst(&[0; 216], &[1; 216], false);
        assert_eq!(n.len(), BURST_BITS);
        assert_eq!(n[230..252].to_vec(), bits_of(NORMAL_1));
        let s = sync_burst(&[0; 120], &[0; 216]);
        assert_eq!(s.len(), BURST_BITS);
        assert_eq!(s[214..252].to_vec(), bits_of(SYNC_TRAINING));
        for b in [n, s] {
            let sum: i32 = (6..126).map(|k| step(b[2 * k], b[2 * k + 1])).sum();
            assert_eq!(sum.rem_euclid(8), 0);
        }
    }
}

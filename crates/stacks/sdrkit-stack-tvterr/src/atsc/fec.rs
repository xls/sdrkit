//! ATSC 1.0 channel coding: the field encoder turns transport packets into symbol levels and the field decoder turns soft
//! levels back into packets, both continuous from field to field. The sequences, the Reed-Solomon code, the byte interleaver and
//! the trellis come from the shared crates. This module holds the A/53 field layout and the map of the 12 trellis encoders.
//!
//! Standard: ATSC A/53 Part 2 clauses 5.3 and 6.

use sdrkit_algo_fec::conv::{BranchMetrics, Ends, Trellis, TrellisDecoder};
use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_algo_modem::equalizer::DfeDecider;
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_util_bits::interleave::DelayLanes;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq;

/// Data symbols in a data segment.
pub const SEG_DATA: usize = 828;
/// Symbols in a segment.
pub const SEG: usize = 832;
/// Data segments in a field.
pub const DATA_SEGS: usize = 312;
/// Bytes per RS word.
pub const RS_N: usize = 207;
/// Bytes per field in the RS domain.
pub const FIELD_BYTES: usize = DATA_SEGS * RS_N;
/// The byte interleaver's branches.
pub const BRANCHES: usize = 52;
/// Cells per branch step.
pub const CELL: usize = 4;
/// Interleaver plus de-interleaver delay in bytes.
pub const IL_DELAY: usize = (BRANCHES - 1) * CELL * BRANCHES;
/// Symbols per encoder per field.
pub const ENC_SYMS: usize = DATA_SEGS * SEG_DATA / 12;
/// Segment sync symbols.
pub const SEG_SYNC: [f32; 4] = [5.0, -5.0, -5.0, 5.0];
/// The 8 levels.
pub const LEVELS: [f32; 8] = [-7.0, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0];

/// Trellis steps of look-ahead (per encoder) the decoder takes from the next field, and of history from the previous one.
pub const LOOK: usize = 96;

/// The PN511 of the field sync (511 bits, 1 = +5). A/53 Part 2 clause 5.3.2.2.
pub fn pn511() -> [u8; 511] {
    let mut out = [0u8; 511];
    let mut l: Lfsr = seq::ATSC_PN511;
    l.fill_bits(&mut out);
    out
}

/// The PN63 of the field sync (63 bits). A/53 Part 2 clause 5.3.2.2.
pub fn pn63() -> [u8; 63] {
    let mut out = [0u8; 63];
    let mut l: Lfsr = seq::ATSC_PN63;
    l.fill_bits(&mut out);
    out
}

/// The 24 VSB mode symbols of 8-VSB.
pub const VSB_MODE: [u8; 24] = [
    0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 0,
];

/// The first 728 symbols of a field sync segment (segment sync, PN511, three PN63, VSB mode), for field 1 (`second == false`)
/// or 2. These symbols are known to a receiver in advance.
pub fn field_sync_known(second: bool) -> &'static [f32; 728] {
    static KNOWN: std::sync::OnceLock<[[f32; 728]; 2]> = std::sync::OnceLock::new();
    &KNOWN.get_or_init(|| [field_sync_made(false), field_sync_made(true)])[second as usize]
}

fn field_sync_made(second: bool) -> [f32; 728] {
    let mut v = [0f32; 728];
    v[..4].copy_from_slice(&SEG_SYNC);
    let lv = |b: u8| if b != 0 { 5.0 } else { -5.0 };
    for (i, b) in pn511().iter().enumerate() {
        v[4 + i] = lv(*b);
    }
    let p = pn63();
    for k in 0..3 {
        for (i, b) in p.iter().enumerate() {
            let b = if k == 1 && second { 1 - *b } else { *b };
            v[515 + 63 * k + i] = lv(b);
        }
    }
    for (i, b) in VSB_MODE.iter().enumerate() {
        v[704 + i] = lv(*b);
    }
    v
}

/// The randomiser's 58344 bytes (312 × 187), one field. A/53 Part 2 clause 5.3.1.1.
pub fn randomiser() -> Vec<u8> {
    let mut out = vec![0u8; DATA_SEGS * 187];
    seq::atsc_randomiser(&mut out);
    out
}

/// The trellis map of a field: for each field byte (interleaved domain), the encoder that takes it and its index among that
/// encoder's bytes. And for each data symbol, its encoder.
pub struct TrellisMap {
    /// `byte_enc[n]`: (encoder, byte index within the encoder) of field byte `n`.
    pub byte_enc: Vec<(u8, u16)>,
    /// `enc_byte[e][m]`: the field byte that is byte `m` of encoder `e`.
    pub enc_byte: Vec<Vec<u32>>,
    /// `enc_sym[e][t]`: the flat data-symbol index (segment × 828 + i) of encoder `e`'s symbol `t`.
    pub enc_sym: Vec<Vec<u32>>,
}

impl TrellisMap {
    /// Builds the map.
    pub fn new() -> TrellisMap {
        let mut byte_enc = vec![(0u8, 0u16); FIELD_BYTES];
        let mut enc_byte: Vec<Vec<u32>> = (0..12)
            .map(|_| Vec::with_capacity(FIELD_BYTES / 12))
            .collect();
        for (n, slot) in byte_enc.iter_mut().enumerate() {
            let chunk = n / 12;
            let j = n % 12;
            let seg = (48 * chunk) / SEG_DATA;
            let e = (j + 4 * seg) % 12;
            *slot = (e as u8, chunk as u16);
            enc_byte[e].push(n as u32);
        }
        let mut enc_sym: Vec<Vec<u32>> = (0..12).map(|_| Vec::with_capacity(ENC_SYMS)).collect();
        for k in 0..DATA_SEGS {
            for i in 0..SEG_DATA {
                enc_sym[(i + 4 * k) % 12].push((k * SEG_DATA + i) as u32);
            }
        }
        TrellisMap {
            byte_enc,
            enc_byte,
            enc_sym,
        }
    }
}

impl Default for TrellisMap {
    fn default() -> Self {
        TrellisMap::new()
    }
}

/// The byte interleaver of the sender (`receive == false`) or the receiver: 52 branches of 4-byte steps. A/53 Part 2 clause 5.3.3.
pub fn byte_interleaver(receive: bool) -> DelayLanes<u8> {
    DelayLanes::forney(BRANCHES as u32, CELL as u32, receive, 0)
}

/// Encodes transport packets into ATSC fields of symbol levels (without the pilot).
pub struct FieldEncoder {
    rs: Rs,
    rand: Vec<u8>,
    map: TrellisMap,
    il: DelayLanes<u8>,
    trellis: Trellis,
    // Per encoder: the precoder's last Z2 and the trellis state.
    pre: [u8; 12],
    st: [u8; 12],
    second: bool,
    last12: [f32; 12],
    bytes: Vec<u8>,
    sym: Vec<f32>,
}

impl FieldEncoder {
    /// A new encoder, about to start field 1.
    pub fn new() -> FieldEncoder {
        FieldEncoder {
            rs: Rs::new(codes::ATSC_207_187).unwrap_or_else(|_| unreachable!()),
            rand: randomiser(),
            map: TrellisMap::new(),
            il: byte_interleaver(false),
            trellis: Trellis::atsc_8vsb(),
            pre: [0; 12],
            st: [0; 12],
            second: false,
            last12: [-5.0; 12],
            bytes: vec![0; FIELD_BYTES],
            sym: vec![0.0; DATA_SEGS * SEG_DATA],
        }
    }

    /// The interleaved bytes of the last field, in field order (the input of the trellis encoders).
    pub fn last_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Encodes 312 packets (188 bytes, sync byte first) into one field: 313 segments of 832 levels appended to `out`. Fewer
    /// packets leave the field unchanged and append nothing.
    pub fn encode_field(&mut self, packets: &[[u8; TS]], out: &mut Vec<f32>) {
        if packets.len() < DATA_SEGS {
            return;
        }
        // Field sync segment.
        let known = field_sync_known(self.second);
        out.extend_from_slice(known);
        let p = pn63();
        for i in 0..92 {
            out.push(if p[i % 63] != 0 { 5.0 } else { -5.0 });
        }
        out.extend_from_slice(&self.last12);
        // Randomise, RS, interleave.
        let mut word = [0u8; RS_N];
        for (k, pk) in packets.iter().take(DATA_SEGS).enumerate() {
            for j in 0..187 {
                word[j] = pk[j + 1] ^ self.rand[k * 187 + j];
            }
            let _ = self.rs.encode_word(&mut word);
            for j in 0..RS_N {
                self.bytes[k * RS_N + j] = self.il.push(word[j]);
            }
        }
        // Trellis: each encoder's dibits in its byte order. X2 through the precoder, X1 through the trellis code.
        for e in 0..12 {
            let bytes = &self.map.enc_byte[e];
            let syms = &self.map.enc_sym[e];
            for (t, &pos) in syms.iter().enumerate() {
                let byte = self.bytes[bytes[t / 4] as usize];
                let dib = (byte >> (6 - 2 * (t % 4))) & 3;
                let z2 = (dib >> 1) ^ self.pre[e];
                self.pre[e] = z2;
                let x1 = (dib & 1) as usize;
                let s = self.st[e] as usize;
                let zz = self.trellis.label(s, x1);
                self.st[e] = self.trellis.next(s, x1) as u8;
                self.sym[pos as usize] = LEVELS[4 * z2 as usize + zz];
            }
        }
        for k in 0..DATA_SEGS {
            out.extend_from_slice(&SEG_SYNC);
            out.extend_from_slice(&self.sym[k * SEG_DATA..(k + 1) * SEG_DATA]);
        }
        self.last12
            .copy_from_slice(&self.sym[DATA_SEGS * SEG_DATA - 12..]);
        self.second = !self.second;
    }
}

impl Default for FieldEncoder {
    fn default() -> Self {
        FieldEncoder::new()
    }
}

/// Counters of the field decoder.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FecCounters {
    /// Fields decoded.
    pub fields: u64,
    /// RS words without error.
    pub rs_clean: u64,
    /// RS words corrected.
    pub rs_corrected: u64,
    /// RS words with too many errors to correct.
    pub rs_failed: u64,
    /// Packets delivered.
    pub packets: u64,
}

/// The branch metrics of a level against the four trellis labels: minus the squared distance to the nearer of the label's two
/// levels (Z2 = 0 or 1), in 1/64 level units squared, at most 4096 levels squared. A level that is not finite has the same metric
/// for every label.
#[inline]
fn label_metrics(y: f32, m: &mut [i32]) {
    for (m, l0) in m.iter_mut().zip([-7.0f32, -5.0, -3.0, -1.0]) {
        let d0 = y - l0;
        let d1 = d0 - 8.0;
        let d = if y.is_finite() {
            (d0 * d0).min(d1 * d1)
        } else {
            0.0
        };
        *m = -(d.min(4096.0) * 64.0) as i32;
    }
}

/// The branch metrics of a window, four labels per step.
struct StepMetrics<'a>(&'a [i32]);

impl BranchMetrics for StepMetrics<'_> {
    fn labels(&self) -> usize {
        4
    }
    #[inline]
    fn fill(&mut self, step: usize, m: &mut [i32]) {
        for (m, v) in m.iter_mut().zip(&self.0[4 * step..4 * step + 4]) {
            *m = *v;
        }
    }
}

/// Steps of each encoder's survivors that [`TrellisSlicer`] keeps: a decision can change until its encoder has taken 12 more
/// symbols (144 symbols of the field and the segment syncs between them).
const SURVIVOR: usize = 12;

/// Decides the symbols of a field for a decision-feedback equaliser from the survivors of the 12 trellis codes. Each symbol
/// extends the four survivors of its encoder, the symbol of the best survivor is the decision, and the earlier symbols of that
/// survivor replace what was decided before. In a known state the code leaves four of the eight levels, 4 apart, so a decision
/// is wrong far less often than the nearest level, and a wrong one is put right before the far feedback taps read it.
///
/// Symbols are counted from the field sync. Those outside the data segments get the nearest level.
#[derive(Clone, Debug)]
pub struct TrellisSlicer {
    metric: [[f32; 4]; 12],
    /// Per encoder and state: the levels of the survivor, the newest last.
    path: [[[f32; SURVIVOR]; 4]; 12],
    /// Per encoder: the symbols its survivors hold, the newest last, and how many are held.
    at: [[u32; SURVIVOR]; 12],
    held: [u8; 12],
}

impl Default for TrellisSlicer {
    fn default() -> Self {
        TrellisSlicer {
            metric: [[0.0; 4]; 12],
            path: [[[0.0; SURVIVOR]; 4]; 12],
            at: [[0; SURVIVOR]; 12],
            held: [0; 12],
        }
    }
}

impl DfeDecider for TrellisSlicer {
    fn begin(&mut self) {
        self.metric = [[0.0; 4]; 12];
        self.held = [0; 12];
    }

    #[inline]
    fn decide(&mut self, t: usize, y: f32, d: &mut [f32]) {
        let (k, i) = (t / SEG, t % SEG);
        let y = if y.is_finite() { y } else { 0.0 };
        if k == 0 || k > DATA_SEGS || i < 4 {
            d[t] = ((y + 7.0) * 0.5).round().clamp(0.0, 7.0) * 2.0 - 7.0;
            return;
        }
        let e = (i - 4 + 4 * (k - 1)) % 12;
        // Per label: the nearer of its two levels and the squared distance to it.
        let mut cost = [0f32; 4];
        let mut level = [0f32; 4];
        for l in 0..4 {
            let lo = 2.0 * l as f32 - 7.0;
            let (d0, d1) = (y - lo, y - lo - 8.0);
            (cost[l], level[l]) = if d0 * d0 <= d1 * d1 {
                (d0 * d0, lo)
            } else {
                (d1 * d1, lo + 8.0)
            };
        }
        // State (a << 1) | b gives Z0 = a and goes to ((X1 ^ b) << 1) | a: state `n` is reached from (n & 1) << 1 | b.
        let old = self.metric[e];
        let paths = self.path[e];
        let mut best = 0;
        for n in 0..4 {
            let (a1, b1) = (n >> 1, n & 1);
            let from = |b: usize| ((b1 << 1) | b, ((a1 ^ b) << 1) | b1);
            let ((s0, l0), (s1, l1)) = (from(0), from(1));
            let (m0, m1) = (old[s0] + cost[l0], old[s1] + cost[l1]);
            let (s, l, m) = if m0 <= m1 { (s0, l0, m0) } else { (s1, l1, m1) };
            self.metric[e][n] = m;
            let p = &mut self.path[e][n];
            p[..SURVIVOR - 1].copy_from_slice(&paths[s][1..]);
            p[SURVIVOR - 1] = level[l];
            if m < self.metric[e][best] {
                best = n;
            }
        }
        let least = self.metric[e][best];
        for m in &mut self.metric[e] {
            *m -= least;
        }
        let at = &mut self.at[e];
        at.copy_within(1.., 0);
        at[SURVIVOR - 1] = t as u32;
        let held = (self.held[e] as usize + 1).min(SURVIVOR);
        self.held[e] = held as u8;
        for j in SURVIVOR - held..SURVIVOR {
            d[at[j] as usize] = self.path[e][best][j];
        }
    }
}

/// Decodes fields of soft levels into transport packets: trellis (per encoder over the field, with [`LOOK`] steps of the
/// previous and the next field around it), byte de-interleaving, RS(207,187), de-randomising. A field is decoded when the next one
/// arrives (its look-ahead), or at [`FieldDecoder::flush`].
pub struct FieldDecoder {
    rs: Rs,
    rand: Vec<u8>,
    map: TrellisMap,
    dil: DelayLanes<u8>,
    trellis: Trellis,
    dec: TrellisDecoder,
    pre: [u8; 12],
    // The field waiting for its look-ahead, and the last LOOK levels of each encoder in the field before it.
    held: Vec<f32>,
    have_held: bool,
    tail: Vec<f32>,
    have_tail: bool,
    // Output bytes counted since the first field after a reset.
    out_count: u64,
    word: Vec<u8>,
    bytes: Vec<u8>,
    lv: Vec<f32>,
    metrics: Vec<i32>,
    inputs: Vec<u8>,
    decided: Vec<f32>,
    /// Counters.
    pub counters: FecCounters,
}

/// Bytes of the end of a field that a trial replays through the de-interleaver: at least its longest delay and the part of a
/// Reed-Solomon word that lies before the next field, in whole turns of the 52 branches.
const REPLAY: usize = BRANCHES * 208;
/// Bytes of the Reed-Solomon word across a field's start that lie before it.
const WORD_BEFORE: usize = (RS_N - IL_DELAY % RS_N) % RS_N;

/// What a trial of a field needs of the fields before it ([`FieldDecoder::trial`]): the precoders, the history of the trellis
/// and the last bytes of the field before.
#[derive(Clone, Debug)]
pub struct FecState {
    pre: [u8; 12],
    tail: Vec<f32>,
    have_tail: bool,
    bytes: Vec<u8>,
    have_bytes: bool,
}

impl Default for FecState {
    fn default() -> Self {
        FecState {
            pre: [0; 12],
            tail: vec![0.0; 12 * LOOK],
            have_tail: false,
            bytes: vec![0; REPLAY],
            have_bytes: false,
        }
    }
}

impl FecState {
    /// No field before: the next trial is a new start.
    pub fn clear(&mut self) {
        self.pre = [0; 12];
        self.have_tail = false;
        self.have_bytes = false;
    }
}

impl FieldDecoder {
    /// A new decoder.
    pub fn new() -> FieldDecoder {
        let trellis = Trellis::atsc_8vsb();
        let steps = ENC_SYMS + 2 * LOOK;
        FieldDecoder {
            rs: Rs::new(codes::ATSC_207_187).unwrap_or_else(|_| unreachable!()),
            rand: randomiser(),
            map: TrellisMap::new(),
            dil: byte_interleaver(true),
            dec: TrellisDecoder::new(&trellis, steps).unwrap_or_else(|_| unreachable!()),
            trellis,
            pre: [0; 12],
            held: vec![0.0; DATA_SEGS * SEG_DATA],
            have_held: false,
            tail: vec![0.0; 12 * LOOK],
            have_tail: false,
            out_count: 0,
            word: Vec::with_capacity(RS_N),
            bytes: vec![0; FIELD_BYTES],
            lv: vec![0.0; steps],
            metrics: vec![0; 4 * steps],
            inputs: vec![0; steps],
            decided: vec![0.0; DATA_SEGS * SEG_DATA],
            counters: FecCounters::default(),
        }
    }

    /// Decodes `levels` as the field after `state`, without the next field as its look-ahead (the last [`LOOK`] steps of each
    /// encoder are then less sure), and returns the Reed-Solomon words that failed and the words that came out. No packet is
    /// made and the counters stay. The same field can be tried again with other levels; [`FieldDecoder::keep`] makes the
    /// state after it. A decoder that runs trials is not used for [`FieldDecoder::push_field`]. `None` when `levels` is short.
    pub fn trial(&mut self, state: &FecState, levels: &[f32]) -> Option<(u64, u64)> {
        if levels.len() < DATA_SEGS * SEG_DATA {
            return None;
        }
        self.pre = state.pre;
        self.tail.copy_from_slice(&state.tail);
        self.have_tail = state.have_tail;
        self.have_held = false;
        self.held.copy_from_slice(&levels[..DATA_SEGS * SEG_DATA]);
        for e in 0..12 {
            self.trellis_one(e, None);
        }
        // The de-interleaver as the end of the field before leaves it, and the start of the word across the field's start.
        self.dil.reset(0);
        self.word.clear();
        if state.have_bytes {
            for (i, &b) in state.bytes.iter().enumerate() {
                let y = self.dil.push(b);
                if i >= REPLAY - WORD_BEFORE {
                    self.word.push(y);
                }
            }
        }
        let skip = if state.have_bytes { 0 } else { IL_DELAY };
        let (mut failed, mut words) = (0u64, 0u64);
        for n in 0..FIELD_BYTES {
            let y = self.dil.push(self.bytes[n]);
            if n < skip {
                continue;
            }
            self.word.push(y);
            if self.word.len() == RS_N {
                words += 1;
                failed += u64::from(self.rs.decode(&mut self.word, &[]).is_err());
                self.word.clear();
            }
        }
        Some((failed, words))
    }

    /// The state after the field of the last [`FieldDecoder::trial`], into `state`.
    pub fn keep(&self, state: &mut FecState) {
        state.pre = self.pre;
        state.tail.copy_from_slice(&self.tail);
        state.have_tail = true;
        state
            .bytes
            .copy_from_slice(&self.bytes[FIELD_BYTES - REPLAY..]);
        state.have_bytes = true;
    }

    /// The levels the trellis path gave the symbols of the last field decoded (312 × 828, nominal ±1 .. ±7).
    pub fn decided(&self) -> &[f32] {
        &self.decided
    }

    /// Starts again (after a loss of field sync): the next field is a new start of the de-interleaver. A field still waiting is
    /// dropped.
    pub fn reset(&mut self) {
        self.dil.reset(0);
        self.pre = [0; 12];
        self.have_held = false;
        self.have_tail = false;
        self.out_count = 0;
        self.word.clear();
    }

    /// The trellis-decoded bytes of the last field decoded, in field order (before de-interleaving).
    pub fn last_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Takes one field's 312 × 828 soft levels (nominal ±1, ±3, ±5, ±7) and decodes the field before it, with this one as its
    /// look-ahead. Packets go to `packets` (188 bytes, sync byte first, transport_error_indicator set on uncorrectable ones). Returns
    /// the trellis path's squared error per symbol of the field decoded (for the data SNR), or `None` when no field was waiting or
    /// `levels` is short.
    pub fn push_field(&mut self, levels: &[f32], packets: &mut Vec<[u8; TS]>) -> Option<f64> {
        if levels.len() < DATA_SEGS * SEG_DATA {
            return None;
        }
        let r = if self.have_held {
            Some(self.decode_held(Some(levels), packets))
        } else {
            None
        };
        self.held.copy_from_slice(&levels[..DATA_SEGS * SEG_DATA]);
        self.have_held = true;
        r
    }

    /// Decodes the field still waiting, without look-ahead (at the end of the signal).
    pub fn flush(&mut self, packets: &mut Vec<[u8; TS]>) -> Option<f64> {
        if !self.have_held {
            return None;
        }
        let r = self.decode_held(None, packets);
        self.have_held = false;
        Some(r)
    }

    fn decode_held(&mut self, next: Option<&[f32]>, packets: &mut Vec<[u8; TS]>) -> f64 {
        let mut err_sum = 0.0f64;
        for e in 0..12 {
            err_sum += self.trellis_one(e, next);
        }
        self.have_tail = true;
        // De-interleave, cut into RS words, decode.
        for n in 0..FIELD_BYTES {
            let y = self.dil.push(self.bytes[n]);
            let o = self.out_count;
            self.out_count += 1;
            if o < IL_DELAY as u64 {
                continue;
            }
            self.word.push(y);
            if self.word.len() == RS_N {
                let p = (o + 1 - IL_DELAY as u64) / RS_N as u64 - 1;
                let k = (p % DATA_SEGS as u64) as usize;
                let r = self.rs.decode(&mut self.word, &[]);
                let mut pk = [0u8; TS];
                pk[0] = 0x47;
                for j in 0..187 {
                    pk[j + 1] = self.word[j] ^ self.rand[k * 187 + j];
                }
                match r {
                    Ok(d) if d.corrected == 0 => self.counters.rs_clean += 1,
                    Ok(_) => self.counters.rs_corrected += 1,
                    Err(_) => {
                        self.counters.rs_failed += 1;
                        pk[1] |= 0x80;
                    }
                }
                packets.push(pk);
                self.counters.packets += 1;
                self.word.clear();
            }
        }
        self.counters.fields += 1;
        err_sum / (DATA_SEGS * SEG_DATA) as f64
    }

    // The trellis of encoder `e` over the held field, with the history of the previous field and the look-ahead of `next`. Writes
    // its bytes into `self.bytes`. Returns the summed squared error of the path over the held field.
    fn trellis_one(&mut self, e: usize, next: Option<&[f32]>) -> f64 {
        let syms = &self.map.enc_sym[e];
        let before = if self.have_tail { LOOK } else { 0 };
        let after = if next.is_some() { LOOK } else { 0 };
        let steps = before + ENC_SYMS + after;
        // The levels of the window: history, the field, look-ahead.
        self.lv[..before].copy_from_slice(&self.tail[e * LOOK..e * LOOK + before]);
        for (t, &pos) in syms.iter().enumerate() {
            self.lv[before + t] = self.held[pos as usize];
        }
        if let Some(nx) = next {
            for t in 0..after {
                self.lv[before + ENC_SYMS + t] = nx[syms[t] as usize];
            }
        }
        // Keep this field's end as the next window's history.
        for t in 0..LOOK {
            self.tail[e * LOOK + t] = self.lv[before + ENC_SYMS - LOOK + t];
        }
        for (m, &y) in self
            .metrics
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&self.lv[..steps])
        {
            label_metrics(y, m);
        }
        let mut bm = StepMetrics(&self.metrics[..4 * steps]);
        if self
            .dec
            .decode(&mut bm, steps, Ends::Open, None, &mut self.inputs[..steps])
            .is_err()
        {
            self.inputs[..steps].fill(0);
        }
        // The decoder outputs the inputs X1. The start state whose re-encoded path scores best is the path's own.
        let step: [[(usize, usize); 2]; 4] = std::array::from_fn(|s| {
            std::array::from_fn(|x| (self.trellis.label(s, x), self.trellis.next(s, x)))
        });
        let mut at = [0usize, 1, 2, 3];
        let mut score = [0i64; 4];
        for (row, &x) in self
            .metrics
            .as_chunks::<4>()
            .0
            .iter()
            .zip(&self.inputs[..steps])
        {
            for (s, m) in at.iter_mut().zip(&mut score) {
                let (label, next) = step[*s & 3][x as usize & 1];
                *m += row[label & 3] as i64;
                *s = next;
            }
        }
        let mut start = 0usize;
        for s0 in 1..4 {
            if score[s0] > score[start] {
                start = s0;
            }
        }
        let mut s = start;
        for t in 0..before {
            s = step[s & 3][self.inputs[t] as usize & 1].1;
        }
        // Z2 decisions, precoder inverse, bytes.
        let mut err = 0.0f64;
        let mut pre = self.pre[e];
        let field = before..before + ENC_SYMS;
        for (((ys, xs), &at), pos) in self.lv[field.clone()]
            .as_chunks::<4>()
            .0
            .iter()
            .zip(self.inputs[field].as_chunks::<4>().0)
            .zip(&self.map.enc_byte[e])
            .zip(syms.as_chunks::<4>().0)
        {
            let mut byte = 0u8;
            for ((&y, &x1), &p) in ys.iter().zip(xs).zip(pos) {
                let (zz, next) = step[s & 3][x1 as usize & 1];
                s = next;
                let l0 = LEVELS[zz & 3];
                let z2 = ((y - l0 - 8.0).abs() < (y - l0).abs()) as u8;
                let d = y - l0 - 8.0 * z2 as f32;
                self.decided[p as usize] = l0 + 8.0 * z2 as f32;
                if y.is_finite() {
                    err += (d * d) as f64;
                }
                byte = (byte << 2) | ((z2 ^ pre) << 1) | (x1 & 1);
                pre = z2;
            }
            self.bytes[at as usize] = byte;
        }
        self.pre[e] = pre;
        err
    }
}

impl Default for FieldDecoder {
    fn default() -> Self {
        FieldDecoder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck};

    #[test]
    fn randomiser_interleaver_and_trellis_map() {
        // The randomiser is an involution (XOR applied twice restores the data) and starts in the state A/53 specifies.
        let r = randomiser();
        assert_eq!(&r[..8], &[0xC0, 0x6D, 0x3F, 0x99, 0x38, 0x6A, 0x29, 0x52]);
        let data: Vec<u8> = (0..500u32).map(|i| (i * 31) as u8).collect();
        let twice: Vec<u8> = data.iter().zip(&r).map(|(d, k)| d ^ k ^ k).collect();
        assert_eq!(twice, data);
        // Interleaver and de-interleaver restore the stream after exactly their delay.
        let mut il = byte_interleaver(false);
        let mut dl = byte_interleaver(true);
        let n = IL_DELAY + 5000;
        let input: Vec<u8> = (0..n).map(|i| (i as u32 * 7 + 3) as u8).collect();
        let out: Vec<u8> = input.iter().map(|&x| dl.push(il.push(x))).collect();
        assert_eq!(&out[IL_DELAY..], &input[..n - IL_DELAY]);
        // The 12-encoder map is a bijection on bytes and on symbols.
        let m = TrellisMap::new();
        let mut seen = vec![false; FIELD_BYTES];
        for e in 0..12 {
            assert_eq!(m.enc_byte[e].len(), FIELD_BYTES / 12);
            assert_eq!(m.enc_sym[e].len(), ENC_SYMS);
            for &b in &m.enc_byte[e] {
                assert!(!seen[b as usize]);
                seen[b as usize] = true;
            }
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(pn511()[..12], [0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1]);
    }

    /// Encodes `fields` fields of numbered packets, adds noise of `snr_db` (data power 21) and decodes. Returns the packet check
    /// after the warm-up, the packets counted, and the byte errors of the trellis output by place in the field.
    fn run(snr_db: f64, fields: usize, seed: u64) -> (NumberedCheck, usize, Vec<u32>) {
        let pattern = Numbered::TERRESTRIAL;
        let mut enc = FieldEncoder::new();
        let mut dec = FieldDecoder::new();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        let sigma = (21.0f64 / 10f64.powf(snr_db / 10.0)).sqrt();
        let mut delivered = Vec::new();
        let mut sent_bytes: Vec<Vec<u8>> = Vec::new();
        let mut errors = vec![0u32; FIELD_BYTES];
        let mut n = 0u32;
        for f in 0..fields {
            let pk: Vec<[u8; TS]> = (0..DATA_SEGS)
                .map(|_| {
                    n += 1;
                    pattern.packet(n - 1)
                })
                .collect();
            let mut lv = Vec::new();
            enc.encode_field(&pk, &mut lv);
            sent_bytes.push(enc.last_bytes().to_vec());
            let mut data = Vec::with_capacity(DATA_SEGS * SEG_DATA);
            for k in 0..DATA_SEGS {
                for &v in &lv[(k + 1) * SEG + 4..(k + 2) * SEG] {
                    data.push(v + (sigma * rng.normal()) as f32);
                }
            }
            if dec.push_field(&data, &mut delivered).is_some() {
                for (i, (a, b)) in dec.last_bytes().iter().zip(&sent_bytes[f - 1]).enumerate() {
                    errors[i] += (a != b) as u32;
                }
            }
        }
        dec.flush(&mut delivered);
        // The first 52 segments (de-interleaver warm-up) are not counted.
        let skip = 53;
        let mut c = NumberedCheck::default();
        for p in delivered.iter().skip(skip) {
            c.push(&pattern, p, 0.0);
        }
        (c, delivered.len() - skip, errors)
    }

    #[test]
    fn field_encoder_and_decoder_through_noise() {
        for snr in [99.0, 22.0] {
            let (c, n, _) = run(snr, 3, 12345);
            assert_eq!(c.good as usize, n, "{snr} dB: {c:?}");
            assert_eq!(c.wrong, 0);
        }
        let (c, n, _) = run(15.5, 3, 12345);
        assert!((c.good as f64) > 0.95 * n as f64, "15.5 dB: {c:?} of {n}");
        assert!(c.wrong <= 3);
    }

    #[test]
    fn the_end_of_a_field_decodes_as_well_as_its_middle() {
        // At 13 dB the trellis makes byte errors. Without look-ahead the last steps of each encoder in a field were decided with
        // no later symbols, so the last bytes of the field had about twice the error rate of the rest.
        let (_, _, errors) = run(13.0, 40, 99);
        let end: u32 = errors[FIELD_BYTES - 12..].iter().sum();
        let all: u32 = errors.iter().sum();
        let per_12 = all as f64 * 12.0 / FIELD_BYTES as f64;
        assert!(
            per_12 > 20.0,
            "the test needs byte errors to compare: {per_12}"
        );
        assert!(
            (end as f64) < 1.3 * per_12,
            "the last 12 bytes: {end} errors, {per_12:.1} for 12 bytes on average"
        );
    }

    #[test]
    fn the_trellis_slicer_decides_better_than_the_nearest_level() {
        // Two fields at 16 dB: the symbols as the slicer leaves them (with its corrections) against the nearest levels.
        let mut enc = FieldEncoder::new();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(17);
        let mut sent = Vec::new();
        for f in 0..2u32 {
            let pk: Vec<[u8; TS]> = (0..DATA_SEGS as u32)
                .map(|k| Numbered::TERRESTRIAL.packet(f * DATA_SEGS as u32 + k))
                .collect();
            enc.encode_field(&pk, &mut sent);
        }
        let per = (DATA_SEGS + 1) * SEG;
        let sigma = (21.0f64 / 10f64.powf(1.6)).sqrt();
        let nearest = |y: f32| ((y + 7.0) * 0.5).round().clamp(0.0, 7.0) * 2.0 - 7.0;
        let (mut wrong, mut wrong_nearest, mut data) = (0, 0, 0);
        let mut slicer = TrellisSlicer::default();
        for field in sent.chunks(per) {
            let mut d = vec![0f32; per];
            slicer.begin();
            for (t, &s) in field.iter().enumerate() {
                let y = s + (sigma * rng.normal()) as f32;
                if t % SEG < 4 || t < SEG {
                    // The sync symbols are known to the equaliser.
                    d[t] = s;
                    continue;
                }
                slicer.decide(t, y, &mut d);
                wrong_nearest += usize::from(nearest(y) != s);
                data += 1;
            }
            wrong += (SEG..per)
                .filter(|t| t % SEG >= 4 && d[*t] != field[*t])
                .count();
        }
        // Nearest level: 7/8 erfc(1 / (sigma sqrt 2)) = 0.147 of the symbols.
        let rate = wrong_nearest as f64 / data as f64;
        assert!((rate - 0.147).abs() < 0.01, "{rate}");
        assert!(wrong * 8 < wrong_nearest, "{wrong} against {wrong_nearest}");
        // Without noise every symbol is right, also the first of each encoder, whose state is not known yet.
        let mut d = vec![0f32; per];
        slicer.begin();
        for (t, &s) in sent[..per].iter().enumerate() {
            slicer.decide(t, s, &mut d);
        }
        assert_eq!(&d[SEG..], &sent[SEG..per]);
    }

    #[test]
    fn a_trial_counts_the_failures_the_decoder_has() {
        // Trials of each field, twice, against the decoder that runs on from field to field: no failure in clean fields, and in
        // noise near the cliff a count close to the decoder's (a trial has no look-ahead).
        for (snr_db, slack) in [(40.0, 0u64), (14.3, 12)] {
            let pattern = Numbered::TERRESTRIAL;
            let mut enc = FieldEncoder::new();
            let mut dec = FieldDecoder::new();
            let mut judge = FieldDecoder::new();
            let mut state = FecState::default();
            let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
            let sigma = (21.0f64 / 10f64.powf(snr_db / 10.0)).sqrt();
            let mut n = 0u32;
            let mut pk = Vec::new();
            let mut counts = Vec::new();
            for f in 0..5 {
                let packets: Vec<[u8; TS]> = (0..DATA_SEGS)
                    .map(|_| {
                        n += 1;
                        pattern.packet(n - 1)
                    })
                    .collect();
                let mut lv = Vec::new();
                enc.encode_field(&packets, &mut lv);
                let mut data = Vec::with_capacity(DATA_SEGS * SEG_DATA);
                for k in 0..DATA_SEGS {
                    for &v in &lv[(k + 1) * SEG + 4..(k + 2) * SEG] {
                        data.push(v + (sigma * rng.normal()) as f32);
                    }
                }
                let first = judge.trial(&state, &data).unwrap();
                assert_eq!(judge.trial(&state, &data), Some(first), "a trial repeats");
                assert_eq!(first.1, if f == 0 { 260 } else { 312 });
                // The path's levels are the levels sent, where the field decodes.
                if snr_db > 30.0 {
                    assert_eq!(first.0, 0, "field {f}");
                    let sent = (1..=DATA_SEGS).flat_map(|k| lv[k * SEG + 4..(k + 1) * SEG].iter());
                    assert!(judge.decided().iter().zip(sent).all(|(a, b)| a == b));
                }
                judge.keep(&mut state);
                counts.push(first.0);
                // The decoder gives field f - 1 when field f arrives.
                let c0 = dec.counters.rs_failed;
                dec.push_field(&data, &mut pk);
                if f >= 2 {
                    let real = dec.counters.rs_failed - c0;
                    let trial = counts[f - 1];
                    assert!(
                        trial + slack >= real && trial <= real + slack,
                        "{snr_db} dB field {}: trial {trial}, decoder {real}",
                        f - 1
                    );
                }
            }
            assert!(judge.trial(&state, &[0.0; 16]).is_none());
            state.clear();
            assert_eq!(
                judge
                    .trial(&state, &vec![0.0; DATA_SEGS * SEG_DATA])
                    .unwrap()
                    .1,
                260
            );
        }
    }
}

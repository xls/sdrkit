//! The HT and VHT data fields of the OFDM chains, decoded on the helper threads of the run context. A chain reads the signal
//! fields in order and gives each decodable data field, with a copy of its PPDU, to a bounded window ([`InOrder`]). A helper
//! demaps, deinterleaves, depunctures, decodes, descrambles, checks the FCS and takes the A-MPDU apart. The frames, the
//! counters and the decoded fields leave in the order the chains found them, so the output is the same for every number of
//! helpers.
//!
//! With live input a data field is skipped and counted as [`Undecoded::Busy`] when every slot of the window is in use, or, with
//! no helpers, when the chain is more than [`BEHIND_S`](super::BEHIND_S) of work behind its input. With a recording or a test
//! signal nothing is skipped: the chain waits for a slot and drains the window at the end of each piece.

use super::{mpdu, PhyKind, PpduCounters, WifiMpdu};
use crate::wifi::plan::Band;
use sdrkit_graph::{InOrder, Pool, Timed};
use sdrkit_proto_wlan_phy::mac as phymac;
use sdrkit_proto_wlan_phy::ofdm::PpduView;
use sdrkit_proto_wlan_phy::ppdu::{DataPlan, OwnedPpdu, Ppdu, PpduDecoder, PpduInfo, SigFields, Undecoded};
use sdrkit_proto_wlan_phy::vht::VhtMcs;
use std::collections::VecDeque;

/// The most helper threads the data fields use.
const MAX_HELPERS: usize = 4;
/// Slots of the window per helper.
const SLOTS_PER_HELPER: usize = 4;

/// One data field: the PPDU and its plan, where it was heard, and what the helper made of it.
struct FieldJob {
    ppdu: OwnedPpdu,
    plan: Option<DataPlan>,
    time_s: f64,
    channel: u32,
    band: Option<Band>,
    /// `None` when the data was decoded.
    why: Option<Undecoded>,
    psdu: Vec<u8>,
    /// The MPDUs with a good FCS: start and length in `psdu`, the FCS left out.
    mpdus: Vec<(usize, usize)>,
    bad_fcs: u64,
    bad_delimiters: u64,
}

impl Default for FieldJob {
    fn default() -> Self {
        FieldJob {
            ppdu: OwnedPpdu::default(),
            plan: None,
            time_s: 0.0,
            channel: 0,
            band: None,
            why: None,
            psdu: Vec::new(),
            mpdus: Vec::new(),
            bad_fcs: 0,
            bad_delimiters: 0,
        }
    }
}

/// Decodes the data field of `j` with decoder `dec`, checks the FCS of each MPDU and takes an A-MPDU apart.
fn decode(dec: &mut Box<PpduDecoder>, j: &mut FieldJob) {
    j.psdu.clear();
    j.mpdus.clear();
    j.bad_fcs = 0;
    j.bad_delimiters = 0;
    let Some(plan) = j.plan else {
        j.why = Some(Undecoded::Data);
        return;
    };
    let (info, psdu, sig_b_length) = match dec.decode_data(&j.ppdu.view(), &plan) {
        Ppdu::Psdu { info, psdu, sig_b_length, .. } => (info, psdu, sig_b_length),
        Ppdu::Seen { why, .. } => {
            j.why = Some(why);
            return;
        }
        Ppdu::NonHt => {
            j.why = Some(Undecoded::Data);
            return;
        }
    };
    j.why = None;
    j.psdu.extend_from_slice(psdu);
    let all = &j.psdu[..sig_b_length.map_or(j.psdu.len(), |n| n.min(j.psdu.len()))];
    let base = all.as_ptr() as usize;
    let mut take = |frame: &[u8]| {
        if frame.len() < 14 || !phymac::fcs_ok(frame) {
            j.bad_fcs += 1;
        } else {
            j.mpdus.push((frame.as_ptr() as usize - base, frame.len() - 4));
        }
    };
    if info.aggregated {
        let mut walk = phymac::Ampdu::new(all);
        for sub in walk.by_ref() {
            take(sub.mpdu);
        }
        j.bad_delimiters = u64::from(walk.bad_delimiters());
    } else {
        take(&j.psdu);
    }
}

/// What waits to leave in order.
enum Held {
    Frame(Timed<WifiMpdu>),
    Note(PpduInfo, Undecoded),
    /// The oldest data field of the window not yet taken back.
    Field,
}

/// The data fields of every OFDM chain of a decoder, and the PPDU counters (module documentation).
pub(super) struct Fields {
    window: InOrder<FieldJob, Box<PpduDecoder>>,
    held: VecDeque<Held>,
    /// What left in order, for the decoder to send.
    pub ready: Vec<Timed<WifiMpdu>>,
    /// The counters of the HT, VHT, HE and EHT PPDUs.
    pub ppdu: PpduCounters,
    /// The samples are live: skip data fields rather than wait.
    pub live: bool,
}

impl Default for Fields {
    /// No helpers, not live.
    fn default() -> Self {
        Fields::new(&Pool::shared(), 0, false)
    }
}

impl Fields {
    /// Data fields on up to `helpers` helper threads of `pool` (at most [`MAX_HELPERS`]).
    pub fn new(pool: &Pool, helpers: usize, live: bool) -> Fields {
        let helpers = helpers.min(MAX_HELPERS);
        Fields {
            window: InOrder::new(
                pool,
                helpers,
                SLOTS_PER_HELPER * helpers.max(1),
                || Box::new(PpduDecoder::new()),
                FieldJob::default,
                decode,
            ),
            held: VecDeque::with_capacity(64),
            ready: Vec::new(),
            ppdu: PpduCounters::default(),
            live,
        }
    }

    /// The helper threads.
    pub fn helpers(&self) -> usize {
        self.window.helpers()
    }

    /// A non-HT frame, in order.
    pub fn frame(&mut self, m: Timed<WifiMpdu>) {
        if self.held.is_empty() {
            self.ready.push(m);
        } else {
            self.held.push_back(Held::Frame(m));
        }
    }

    /// A PPDU whose data is not decoded, in order.
    pub fn note(&mut self, info: PpduInfo, why: Undecoded) {
        if self.held.is_empty() {
            self.ppdu.note(&info, Some(why));
        } else {
            self.held.push_back(Held::Note(info, why));
        }
    }

    /// A decodable data field of PPDU `v`, heard at `time_s` on `channel`. `behind` says the chain is behind its input, which
    /// skips the field when the input is live and there are no helpers.
    pub fn field(
        &mut self,
        v: &PpduView<'_>,
        plan: DataPlan,
        time_s: f64,
        channel: u32,
        band: Option<Band>,
        behind: bool,
    ) {
        if self.live && (self.helpers() == 0 && behind) {
            self.note(plan.info, Undecoded::Busy);
            return;
        }
        if self.window.is_full() {
            self.flush(false);
        }
        while !self.live && self.window.is_full() {
            self.flush_one_field();
        }
        let given = self.window.give(|j| {
            j.ppdu.copy_from(v);
            j.plan = Some(plan);
            (j.time_s, j.channel, j.band) = (time_s, channel, band);
        });
        if given {
            self.held.push_back(Held::Field);
        } else {
            self.note(plan.info, Undecoded::Busy);
        }
    }

    /// Lets out what is done, in order. With `wait` it waits for every data field.
    pub fn flush(&mut self, wait: bool) {
        while let Some(h) = self.held.front() {
            if matches!(h, Held::Field) {
                let (ready, ppdu) = (&mut self.ready, &mut self.ppdu);
                let deliver = |j: &mut FieldJob, r: Result<(), sdrkit_graph::context::JobPanicked>| {
                    deliver(j, r.is_ok(), ready, ppdu);
                };
                let taken = if wait {
                    self.window.wait_one(deliver)
                } else {
                    self.window.take_one(deliver)
                };
                if !taken {
                    return;
                }
            }
            self.let_out_front();
        }
    }

    /// Waits for every data field and lets everything out. False when nothing was held or ready.
    pub fn flush_all(&mut self) -> bool {
        let any = !self.held.is_empty() || !self.ready.is_empty();
        self.flush(true);
        any
    }

    /// Lets out the held items up to and with the oldest data field, waiting for it.
    fn flush_one_field(&mut self) {
        while let Some(h) = self.held.front() {
            if matches!(h, Held::Field) {
                let (ready, ppdu) = (&mut self.ready, &mut self.ppdu);
                self.window
                    .wait_one(|j, r| deliver(j, r.is_ok(), ready, ppdu));
                self.held.pop_front();
                return;
            }
            self.let_out_front();
        }
    }

    /// Lets out the front item, a data field already delivered.
    fn let_out_front(&mut self) {
        match self.held.pop_front() {
            Some(Held::Frame(m)) => self.ready.push(m),
            Some(Held::Note(info, why)) => self.ppdu.note(&info, Some(why)),
            Some(Held::Field) | None => {}
        }
    }
}

/// Counts a decoded data field and sends its MPDUs.
fn deliver(j: &mut FieldJob, ok: bool, ready: &mut Vec<Timed<WifiMpdu>>, counters: &mut PpduCounters) {
    let Some(plan) = j.plan.take() else {
        return;
    };
    let info = plan.info;
    let why = if ok { j.why } else { Some(Undecoded::Data) };
    counters.note(&info, why);
    if why.is_some() {
        return;
    }
    let rate = match info.sig {
        SigFields::Ht(s) => s.rate_mbps(),
        _ => VhtMcs(info.mcs.unwrap_or(0)).rate_mbps(info.short_gi),
    };
    let v = j.ppdu.view();
    counters.bad_fcs += j.bad_fcs;
    counters.bad_delimiters += j.bad_delimiters;
    for &(at, len) in &j.mpdus {
        counters.mpdus += 1;
        let mut m = mpdu(
            &j.psdu[at..at + len],
            PhyKind::Ofdm,
            rate,
            v.level_dbfs,
            v.offset_hz,
            v.snr_db,
            j.time_s,
            j.channel,
            j.band,
        );
        m.value.format = Some(info.format);
        m.value.mcs = info.mcs;
        m.value.aggregated = info.aggregated;
        ready.push(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wifi::bench::busy_signal;
    use crate::wifi::{Chain, Rx, CHAIN_RATE_HZ, CHAIN_WIDTH_HZ};
    use sdrkit_core::Complex32;
    use sdrkit_proto_wlan_phy::ht::{self, HtSig, Mcs, PilotPattern};
    use sdrkit_proto_wlan_phy::{ofdm, vht};

    fn chain() -> Chain {
        let rx = Rx::Ofdm(Box::new(ofdm::OfdmReceiver::new()), Box::new(PpduDecoder::new()));
        Chain::new(CHAIN_RATE_HZ, 0.0, CHAIN_WIDTH_HZ, CHAIN_RATE_HZ, rx, 0.0, Some(Band::G5), 36).unwrap()
    }

    /// The frames and counters of `x` through one chain with `helpers` helpers, not live.
    fn run(x: &[Complex32], helpers: usize) -> (Vec<Timed<WifiMpdu>>, PpduCounters) {
        let pool = Pool::new(4);
        let mut fields = Fields::new(&pool, helpers, false);
        let mut c = chain();
        let mut out = Vec::new();
        for p in x.chunks(65_536) {
            c.run(p, 1.0, &mut out, &mut fields);
            fields.flush(true);
            out.append(&mut fields.ready);
        }
        (out, fields.ppdu)
    }

    /// The HT and VHT MPDUs of `x` as the receiver and [`PpduDecoder::decode`] give them in line: bytes, format, MCS and time.
    fn in_line(x: &[Complex32]) -> Vec<(Vec<u8>, Option<u8>, f64)> {
        let mut rx = ofdm::OfdmReceiver::new();
        let mut dec = PpduDecoder::new();
        let mut got = Vec::new();
        for p in x.chunks(65_536) {
            rx.process_with(
                p,
                |_| {},
                |v| match dec.decode(v) {
                    Ppdu::NonHt => false,
                    Ppdu::Seen { .. } => true,
                    Ppdu::Psdu { info, psdu, sig_b_length, .. } => {
                        let mut take = |f: &[u8]| {
                            if f.len() >= 14 && phymac::fcs_ok(f) {
                                got.push((f[..f.len() - 4].to_vec(), info.mcs, v.sample as f64 / CHAIN_RATE_HZ));
                            }
                        };
                        if info.aggregated {
                            let all = &psdu[..sig_b_length.map_or(psdu.len(), |n| n.min(psdu.len()))];
                            for sub in phymac::Ampdu::new(all) {
                                take(sub.mpdu);
                            }
                        } else {
                            take(psdu);
                        }
                        true
                    }
                },
            );
        }
        got
    }

    fn mpdu(n: usize, seed: &mut u32) -> Vec<u8> {
        let mut f = vec![0x88, 0x01, 0, 0, 2, 0, 0, 0, 0, 1, 6, 1, 2, 3, 4, 5, 2, 0, 0, 0, 0, 1, 0x10, 0, 0, 0];
        f.extend((0..n).map(|_| {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (*seed >> 24) as u8
        }));
        phymac::push_fcs(&mut f);
        f
    }

    /// Every HT MCS 0 to 7 and VHT MCS 0 to 8 with random payloads, both guard intervals, back to back with a little noise.
    fn every_mcs() -> (Vec<Complex32>, usize) {
        let mut seed = 7u32;
        let mut x = vec![Complex32::default(); 2000];
        let mut mpdus = 0;
        for short_gi in [false, true] {
            for m in 0..8u8 {
                let parts: Vec<Vec<u8>> = (0..3).map(|i| mpdu(100 + 300 * i + 17 * usize::from(m), &mut seed)).collect();
                let mut a = Vec::new();
                for (i, p) in parts.iter().enumerate() {
                    phymac::push_ampdu_subframe(&mut a, p, i == 2);
                }
                let sig = HtSig {
                    mcs: m,
                    smoothing: true,
                    not_sounding: true,
                    aggregation: true,
                    short_gi,
                    ..HtSig::default()
                };
                ht::build_ppdu_sig(&a, sig, Mcs(m), 0x21 + m, PilotPattern::Rotating, &mut x);
                x.extend(std::iter::repeat_n(Complex32::default(), 400));
                let one = mpdu(200 + 40 * usize::from(m), &mut seed);
                ht::build_ppdu(&one, Mcs(m), short_gi, 0x11 + m, PilotPattern::Fixed, &mut x);
                x.extend(std::iter::repeat_n(Complex32::default(), 400));
                mpdus += 4;
            }
            for m in 0..9u8 {
                let parts: Vec<Vec<u8>> = (0..2).map(|i| mpdu(500 + 700 * i + 29 * usize::from(m), &mut seed)).collect();
                let mut a = Vec::new();
                for (i, p) in parts.iter().enumerate() {
                    phymac::push_ampdu_subframe(&mut a, p, i == 1);
                }
                vht::build_ppdu(&a, vht::VhtMcs(m), short_gi, 0x40 + m, &mut x);
                x.extend(std::iter::repeat_n(Complex32::default(), 400));
                mpdus += 2;
            }
        }
        x.extend(std::iter::repeat_n(Complex32::default(), 4000));
        for s in &mut x {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.004;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.004;
            *s += Complex32::new(a, b);
        }
        (x, mpdus)
    }

    fn summary(out: &[Timed<WifiMpdu>]) -> Vec<(Vec<u8>, Option<u8>, f64)> {
        out.iter().map(|m| (m.value.bytes.clone(), m.value.mcs, m.time)).collect()
    }

    #[test]
    fn helpers_give_the_frames_and_counters_of_the_in_line_path_for_every_mcs() {
        let (x, mpdus) = every_mcs();
        let (one, counters) = run(&x, 0);
        assert_eq!(one.len(), mpdus, "every MPDU of every MCS is decoded");
        assert_eq!(summary(&one), in_line(&x));
        assert_eq!((counters.mpdus, counters.bad_fcs), (mpdus as u64, 0));
        for helpers in [1, 2, 4] {
            let (got, c) = run(&x, helpers);
            assert!(got == one, "{helpers} helpers");
            assert_eq!(c, counters, "{helpers} helpers");
        }
    }

    #[test]
    fn helpers_give_the_frames_and_counters_of_the_in_line_path_on_a_busy_channel() {
        let x = busy_signal();
        let (one, counters) = run(&x, 0);
        assert!(one.len() > 100);
        assert_eq!(summary(&one), in_line(&x));
        assert_eq!(counters.not_decoded_for(Undecoded::Busy), 0);
        for helpers in [1, 2, 4] {
            let (got, c) = run(&x, helpers);
            assert!(got == one, "{helpers} helpers");
            assert_eq!(c, counters, "{helpers} helpers");
        }
    }

    #[test]
    fn helpers_give_the_frames_and_counters_of_the_in_line_path_on_the_test_signal_scenes() {
        use sdrkit_core::Generator;
        for band in [Band::G5, Band::G6] {
            let mut g = crate::wifi::gen::WifiGenerator::new(crate::wifi::gen::WifiGenParams {
                rate_hz: CHAIN_RATE_HZ,
                frames: crate::wifi::gen::scene_56(band, if band == Band::G5 { 36 } else { 5 }),
                ..Default::default()
            });
            let mut x = vec![Complex32::default(); (0.3 * CHAIN_RATE_HZ) as usize];
            g.generate(&mut x);
            let (one, counters) = run(&x, 0);
            assert!(!one.is_empty());
            for helpers in [1, 3] {
                let (got, c) = run(&x, helpers);
                assert!(got == one, "{band:?}, {helpers} helpers");
                assert_eq!(c, counters);
            }
        }
    }

    #[test]
    fn a_full_window_skips_live_data_fields_and_counts_them() {
        // Live input fed far faster than the helper decodes: the window fills, and every data field is either decoded or
        // counted as busy. What is decoded is a part of the full result, in order.
        let x = busy_signal();
        let (all, full) = run(&x, 0);
        let pool = Pool::new(1);
        // The pool's one thread is held until the input is in: the window fills whatever the timing.
        let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let g = std::sync::Arc::clone(&gate);
        let held = pool.spawn(move || {
            let mut open = g.0.lock().unwrap();
            while !*open {
                open = g.1.wait(open).unwrap();
            }
        });
        let mut fields = Fields::new(&pool, 1, true);
        let mut c = chain();
        let mut out = Vec::new();
        for p in x.chunks(65_536) {
            c.run(p, 1.0, &mut out, &mut fields);
            fields.flush(false);
            out.append(&mut fields.ready);
        }
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        held.wait().unwrap();
        fields.flush(true);
        out.append(&mut fields.ready);
        let p = fields.ppdu;
        let busy = p.not_decoded_for(Undecoded::Busy);
        assert_eq!(busy, full.decoded - SLOTS_PER_HELPER as u64, "all but the window's slots are skipped");
        assert_eq!(p.decoded + busy, full.decoded);
        assert_eq!(p.seen, full.seen);
        let mut rest = all.iter();
        assert!(out.iter().all(|m| rest.any(|a| a == m)), "the decoded frames keep their order");
    }
}

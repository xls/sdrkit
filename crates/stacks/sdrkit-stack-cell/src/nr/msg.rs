//! The NR SIB1 (3GPP TS 38.331, BCCH-DL-SCH-Message, unaligned PER), described once over [`PerIo`]: the same description reads
//! and writes. The fields kept are the ones the stack publishes. Components that only take room are walked with typical
//! values: a writer sends them present with their first values, a reader skips whatever is there.
//!
//! The description is in parts; a reader keeps every part that was read whole, so a message with a component this
//! description does not expect still gives its first parts.

pub use crate::nbiot::msg::Plmn;
use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};
use serde::{Deserialize, Serialize};

fn flag<P: PerIo>(p: &mut P, b: bool) -> bool {
    let mut x = b;
    p.flag(&mut x);
    x
}

fn int<P: PerIo, T: Copy + TryFrom<i64> + Into<i64>>(p: &mut P, v: &mut T, lo: i64, hi: i64) {
    let mut x: i64 = (*v).into();
    p.int(&mut x, lo, hi);
    if let Ok(t) = T::try_from(x) {
        *v = t;
    }
}

/// An INTEGER (`lo..=hi`) that is not kept: a writer sends `lo`.
fn skip_int<P: PerIo>(p: &mut P, lo: i64, hi: i64) {
    let mut x = lo;
    p.int(&mut x, lo, hi);
}

/// An ENUMERATED of `count` values that is not kept: a writer sends the first.
fn skip_enum<P: PerIo>(p: &mut P, count: usize) {
    let mut i = 0;
    p.enumerated(&mut i, count, false);
}

/// An OPTIONAL value of an INTEGER range.
fn opt_int<P: PerIo, T: Copy + Default + TryFrom<i64> + Into<i64>>(p: &mut P, has: bool, v: &mut Option<T>, lo: i64, hi: i64) {
    if has {
        let mut x = v.unwrap_or_default();
        int(p, &mut x, lo, hi);
        *v = Some(x);
    } else {
        *v = None;
    }
}

/// The end of an extensible SEQUENCE whose extension bit was `e`.
fn ext_end<P: PerIo>(p: &mut P, e: bool) {
    if e {
        p.skip_extensions();
    }
}

/// The cells of one PLMN-IdentityInfo: the networks that share one cell identity and tracking area.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlmnInfo {
    /// The networks.
    pub plmns: Vec<Plmn>,
    /// trackingAreaCode (24 bits).
    pub tac: Option<u32>,
    /// ranac.
    pub ranac: Option<u8>,
    /// cellIdentity (36 bits).
    pub cell_identity: u64,
    /// cellReservedForOperatorUse is reserved.
    pub reserved_operator: bool,
    /// gNB-ID-Length-r17 (22 to 32), when sent.
    pub gnb_id_length: Option<u8>,
}

impl PlmnInfo {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let mut e = flag(p, self.gnb_id_length.is_some());
        let has_tac = flag(p, self.tac.is_some());
        let has_ranac = flag(p, self.ranac.is_some());
        let mut n = self.plmns.len().max(1);
        p.count(&mut n, 1, 12);
        self.plmns.resize(n, Plmn::default());
        for pl in self.plmns.iter_mut() {
            pl.codec(p);
        }
        if has_tac {
            let mut v = u64::from(self.tac.unwrap_or(0));
            p.bits(&mut v, 24);
            self.tac = Some(v as u32);
        } else {
            self.tac = None;
        }
        opt_int(p, has_ranac, &mut self.ranac, 0, 255);
        p.bits(&mut self.cell_identity, 36);
        let mut res = usize::from(!self.reserved_operator);
        p.enumerated(&mut res, 2, false);
        self.reserved_operator = res == 0;
        // Extension additions: group 1 (iab-Support-r16), group 2 (trackingAreaList-r17, gNB-ID-Length-r17).
        if p.reading() {
            self.gnb_id_length = None;
            if !e {
                return;
            }
            let mut big = 0;
            p.bits(&mut big, 1);
            if big != 0 {
                p.fail();
                return;
            }
            let mut count = 0;
            p.bits(&mut count, 6);
            let count = count as usize + 1;
            let mut map = vec![false; count];
            p.optionals(&mut map);
            for (i, _) in map.iter().enumerate().filter(|x| *x.1) {
                let mut body = Vec::new();
                p.open_type(&mut body);
                if i == 1 {
                    let mut r = PerReader::new(&body);
                    let has_list = flag(&mut r, false);
                    let has_len = flag(&mut r, false);
                    if has_list {
                        let mut n = 1;
                        r.count(&mut n, 1, 12);
                        let mut skip = 0;
                        for _ in 0..n {
                            r.bits(&mut skip, 24);
                        }
                    }
                    if has_len && !r.failed() {
                        let mut v = 22u8;
                        int(&mut r, &mut v, 22, 32);
                        if !r.failed() {
                            self.gnb_id_length = Some(v);
                        }
                    }
                }
            }
        } else if let Some(len) = self.gnb_id_length {
            // Two additions known, the second present: its body holds no list and the length.
            let mut body = Vec::new();
            let mut w = PerWriter::new(&mut body);
            flag(&mut w, false);
            flag(&mut w, true);
            let mut v = len;
            int(&mut w, &mut v, 22, 32);
            w.finish();
            let mut x = 0;
            p.bits(&mut x, 1);
            x = 1;
            p.bits(&mut x, 6);
            let mut map = [false, true];
            p.optionals(&mut map);
            p.open_type(&mut body);
            e = true;
        }
        let _ = e;
    }

    /// The gNB identity and the cell identity within the gNB, when the length of the gNB identity was sent.
    pub fn gnb_and_cell(&self) -> Option<(u32, u16)> {
        let len = u32::from(self.gnb_id_length?);
        let local = 36 - len;
        Some(((self.cell_identity >> local) as u32, (self.cell_identity & ((1 << local) - 1)) as u16))
    }
}

/// One SI message of the scheduling list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SiMessage {
    /// si-BroadcastStatus is broadcasting.
    pub broadcasting: bool,
    /// si-Periodicity, radio frames (8 to 512).
    pub period_frames: u16,
    /// The SIBs of the message: the SIB number, its value tag and whether it has an area scope.
    pub sibs: Vec<(u8, Option<u8>, bool)>,
}

impl SiMessage {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let mut st = usize::from(!self.broadcasting);
        p.enumerated(&mut st, 2, false);
        self.broadcasting = st == 0;
        let mut per = i64::from(self.period_frames);
        p.enum_value(&mut per, &[8, 16, 32, 64, 128, 256, 512]);
        self.period_frames = per as u16;
        let mut n = self.sibs.len().max(1);
        p.count(&mut n, 1, 32);
        self.sibs.resize(n, (2, None, false));
        for (sib, tag, area) in self.sibs.iter_mut() {
            let has_tag = flag(p, tag.is_some());
            *area = flag(p, *area);
            // type: sibType2 .. sibType14, three spares, then extension values.
            let mut t = usize::from(sib.saturating_sub(2));
            p.enumerated(&mut t, 16, true);
            *sib = t as u8 + 2;
            opt_int(p, has_tag, tag, 0, 31);
        }
    }
}

/// One carrier of scs-SpecificCarrierList.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScsCarrier {
    /// offsetToCarrier, resource blocks.
    pub offset_to_carrier: u16,
    /// subcarrierSpacing, kHz.
    pub scs_khz: u16,
    /// carrierBandwidth, resource blocks.
    pub bandwidth_rb: u16,
}

/// subcarrierSpacing: kHz15, kHz30, kHz60, kHz120, kHz240 and three spares.
fn scs<P: PerIo>(p: &mut P, khz: &mut u16) {
    let mut i = match *khz {
        30 => 1,
        60 => 2,
        120 => 3,
        240 => 4,
        _ => 0,
    };
    p.enumerated(&mut i, 8, false);
    *khz = [15, 30, 60, 120, 240, 0, 0, 0][i.min(7)];
}

fn carrier_list<P: PerIo>(p: &mut P, list: &mut Vec<ScsCarrier>) {
    let mut n = list.len().max(1);
    p.count(&mut n, 1, 5);
    list.resize(n, ScsCarrier::default());
    for c in list.iter_mut() {
        let e = flag(p, false);
        int(p, &mut c.offset_to_carrier, 0, 2199);
        scs(p, &mut c.scs_khz);
        int(p, &mut c.bandwidth_rb, 1, 275);
        ext_end(p, e);
    }
}

/// MultiFrequencyBandListNR-SIB: the band numbers (0 where an entry has none).
fn band_list<P: PerIo>(p: &mut P, bands: &mut Vec<u16>) {
    let mut n = bands.len().max(1);
    p.count(&mut n, 1, 8);
    bands.resize(n, 0);
    for b in bands.iter_mut() {
        let has_band = flag(p, *b != 0);
        let has_pmax = flag(p, false);
        if has_band {
            int(p, b, 1, 1024);
        } else {
            *b = 0;
        }
        if has_pmax {
            let mut m = 1;
            p.count(&mut m, 1, 8);
            for _ in 0..m {
                if flag(p, false) {
                    skip_int(p, -30, 33);
                }
                skip_int(p, 0, 7);
            }
        }
    }
}

/// BWP: locationAndBandwidth, subcarrierSpacing, cyclicPrefix.
fn bwp<P: PerIo>(p: &mut P, riv: &mut u16, khz: &mut u16) {
    flag(p, false);
    int(p, riv, 0, 37949);
    scs(p, khz);
}

/// RACH-ConfigGeneric.
fn rach_generic<P: PerIo>(p: &mut P) {
    let e = flag(p, false);
    skip_int(p, 0, 255);
    skip_enum(p, 4);
    skip_int(p, 0, 274);
    skip_int(p, 0, 15);
    skip_int(p, -202, -60);
    skip_enum(p, 11);
    skip_enum(p, 4);
    skip_enum(p, 8);
    ext_end(p, e);
}

/// SI-RequestConfig.
fn si_request<P: PerIo>(p: &mut P) {
    let has_occ = flag(p, false);
    let has_period = flag(p, false);
    if has_occ {
        rach_generic(p);
        skip_enum(p, 8);
    }
    if has_period {
        skip_enum(p, 8);
    }
    let mut n = 1;
    p.count(&mut n, 1, 32);
    for _ in 0..n {
        let a = flag(p, false);
        let b = flag(p, false);
        skip_int(p, 0, 63);
        if a {
            skip_int(p, 0, 15);
        }
        if b {
            skip_int(p, 0, 15);
        }
    }
}

/// A list of time domain allocations (PDSCH or PUSCH): k0 or k2, the mapping type, startSymbolAndLength.
fn time_alloc_list<P: PerIo>(p: &mut P) {
    let mut n = 1;
    p.count(&mut n, 1, 16);
    for _ in 0..n {
        if flag(p, false) {
            skip_int(p, 0, 32);
        }
        skip_enum(p, 2);
        skip_int(p, 0, 127);
    }
}

/// ControlResourceSet.
fn coreset<P: PerIo>(p: &mut P) {
    let e = flag(p, false);
    let add = flag(p, false);
    let rel = flag(p, false);
    let tci = flag(p, false);
    let scr = flag(p, false);
    skip_int(p, 0, 11);
    let mut bits = 0;
    p.bits(&mut bits, 45);
    skip_int(p, 1, 3);
    let mut kind = 1;
    p.choice(&mut kind, 2, false);
    if kind == 0 {
        let shift = flag(p, false);
        skip_enum(p, 3);
        skip_enum(p, 3);
        if shift {
            skip_int(p, 0, 274);
        }
    }
    skip_enum(p, 2);
    for has in [add, rel] {
        if has {
            let mut n = 1;
            p.count(&mut n, 1, 64);
            for _ in 0..n {
                skip_int(p, 0, 127);
            }
        }
    }
    let _ = tci;
    if scr {
        skip_int(p, 0, 65535);
    }
    ext_end(p, e);
}

/// SearchSpace. A writer sends a common search space for DCI formats 0_0 and 1_0 in every slot.
fn search_space<P: PerIo>(p: &mut P) {
    let has_coreset = flag(p, true);
    let has_slots = flag(p, true);
    let has_duration = flag(p, false);
    let has_symbols = flag(p, true);
    let has_candidates = flag(p, true);
    let has_type = flag(p, true);
    skip_int(p, 0, 39);
    if has_coreset {
        skip_int(p, 0, 11);
    }
    if has_slots {
        const SLOTS: [i64; 15] = [1, 2, 4, 5, 8, 10, 16, 20, 40, 80, 160, 320, 640, 1280, 2560];
        let mut alt = 0;
        p.choice(&mut alt, 15, false);
        match SLOTS.get(alt) {
            Some(&n) if n > 1 => skip_int(p, 0, n - 1),
            Some(_) => {}
            None => p.fail(),
        }
    }
    if has_duration {
        skip_int(p, 2, 2559);
    }
    if has_symbols {
        let mut bits = 1 << 13;
        p.bits(&mut bits, 14);
    }
    if has_candidates {
        for _ in 0..5 {
            skip_enum(p, 8);
        }
    }
    if has_type {
        let mut kind = 0;
        p.choice(&mut kind, 2, false);
        if kind == 0 {
            let f00 = flag(p, true);
            let f20 = flag(p, false);
            let f21 = flag(p, false);
            let f22 = flag(p, false);
            let f23 = flag(p, false);
            if f00 {
                let e = flag(p, false);
                ext_end(p, e);
            }
            if f20 {
                let e = flag(p, false);
                let mut levels = [false; 5];
                p.optionals(&mut levels);
                for _ in levels.iter().filter(|x| **x) {
                    skip_enum(p, 2);
                }
                ext_end(p, e);
            }
            for has in [f21, f22] {
                if has {
                    let e = flag(p, false);
                    ext_end(p, e);
                }
            }
            if f23 {
                let e = flag(p, false);
                if flag(p, false) {
                    skip_enum(p, 8);
                }
                skip_enum(p, 2);
                ext_end(p, e);
            }
        } else {
            let e = flag(p, false);
            skip_enum(p, 2);
            ext_end(p, e);
        }
    }
}

/// BWP-DownlinkCommon after its generic parameters: pdcch-ConfigCommon and pdsch-ConfigCommon.
fn bwp_downlink_rest<P: PerIo>(p: &mut P, e: bool, has_pdcch: bool, has_pdsch: bool) {
    if has_pdcch && flag(p, true) {
        let e = flag(p, false);
        let has_c0 = flag(p, true);
        let has_coreset = flag(p, false);
        let has_ss0 = flag(p, true);
        let has_list = flag(p, true);
        let ids = [flag(p, true), flag(p, false), flag(p, true), flag(p, true)];
        if has_c0 {
            skip_int(p, 0, 15);
        }
        if has_coreset {
            coreset(p);
        }
        if has_ss0 {
            skip_int(p, 0, 15);
        }
        if has_list {
            let mut n = 1;
            p.count(&mut n, 1, 4);
            for _ in 0..n {
                search_space(p);
            }
        }
        for _ in ids.iter().filter(|x| **x) {
            skip_int(p, 0, 39);
        }
        ext_end(p, e);
    }
    if has_pdsch && flag(p, true) {
        let e = flag(p, false);
        if flag(p, true) {
            time_alloc_list(p);
        }
        ext_end(p, e);
    }
    ext_end(p, e);
}

/// PCCH-Config.
fn pcch<P: PerIo>(p: &mut P) {
    let e = flag(p, false);
    let has_first = flag(p, false);
    skip_enum(p, 4);
    const N: [i64; 5] = [1, 2, 4, 8, 16];
    let mut alt = 0;
    p.choice(&mut alt, 5, false);
    match N.get(alt) {
        Some(&n) if n > 1 => skip_int(p, 0, n - 1),
        Some(_) => {}
        None => p.fail(),
    }
    skip_enum(p, 3);
    if has_first {
        const TOP: [i64; 8] = [139, 279, 559, 1119, 2239, 4479, 8959, 17919];
        let mut alt = 0;
        p.choice(&mut alt, 8, false);
        let mut n = 1;
        p.count(&mut n, 1, 4);
        for _ in 0..n {
            skip_int(p, 0, TOP[alt.min(7)]);
        }
    }
    ext_end(p, e);
}

/// RACH-ConfigCommon.
fn rach_common<P: PerIo>(p: &mut P) {
    let e = flag(p, false);
    let has_total = flag(p, false);
    let has_per = flag(p, true);
    let has_b = flag(p, false);
    let has_thr = flag(p, true);
    let has_thr_sul = flag(p, false);
    let has_scs = flag(p, true);
    flag(p, false);
    rach_generic(p);
    if has_total {
        skip_int(p, 1, 63);
    }
    if has_per {
        let mut alt = 3;
        p.choice(&mut alt, 8, false);
        match alt {
            0..=3 => skip_enum(p, 16),
            4 => skip_enum(p, 8),
            5 => skip_int(p, 1, 16),
            6 => skip_int(p, 1, 8),
            _ => skip_int(p, 1, 4),
        }
    }
    if has_b {
        skip_enum(p, 16);
        skip_enum(p, 8);
        skip_int(p, 1, 64);
    }
    skip_enum(p, 8);
    if has_thr {
        skip_int(p, 0, 127);
    }
    if has_thr_sul {
        skip_int(p, 0, 127);
    }
    let mut alt = 1;
    p.choice(&mut alt, 2, false);
    skip_int(p, 0, if alt == 0 { 837 } else { 137 });
    if has_scs {
        skip_enum(p, 8);
    }
    skip_enum(p, 3);
    ext_end(p, e);
}

/// UplinkConfigCommonSIB.
fn uplink<P: PerIo>(p: &mut P) {
    // FrequencyInfoUL-SIB.
    let e = flag(p, false);
    let has_bands = flag(p, false);
    let has_point_a = flag(p, false);
    let has_pmax = flag(p, true);
    flag(p, false);
    if has_bands {
        let mut bands = Vec::new();
        band_list(p, &mut bands);
    }
    if has_point_a {
        skip_int(p, 0, 3_279_165);
    }
    let mut carriers = vec![ScsCarrier {
        bandwidth_rb: 1,
        ..Default::default()
    }];
    carrier_list(p, &mut carriers);
    if has_pmax {
        skip_int(p, -30, 33);
    }
    ext_end(p, e);
    // BWP-UplinkCommon.
    let e = flag(p, false);
    let has_rach = flag(p, true);
    let has_pusch = flag(p, true);
    let has_pucch = flag(p, true);
    let (mut riv, mut khz) = (0, 15);
    bwp(p, &mut riv, &mut khz);
    if has_rach && flag(p, true) {
        rach_common(p);
    }
    if has_pusch && flag(p, true) {
        let e = flag(p, false);
        flag(p, false);
        let has_list = flag(p, true);
        let has_delta = flag(p, true);
        let has_p0 = flag(p, true);
        if has_list {
            time_alloc_list(p);
        }
        if has_delta {
            skip_int(p, -1, 6);
        }
        if has_p0 {
            skip_int(p, -202, 24);
        }
        ext_end(p, e);
    }
    if has_pucch && flag(p, true) {
        let e = flag(p, false);
        let has_res = flag(p, true);
        let has_hop = flag(p, false);
        let has_p0 = flag(p, true);
        if has_res {
            skip_int(p, 0, 15);
        }
        skip_enum(p, 3);
        if has_hop {
            skip_int(p, 0, 1023);
        }
        if has_p0 {
            skip_int(p, -202, 24);
        }
        ext_end(p, e);
    }
    ext_end(p, e);
    // timeAlignmentTimerCommon.
    skip_enum(p, 8);
}

/// The downlink carrier of servingCellConfigCommon.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Downlink {
    /// frequencyBandList: the band numbers.
    pub bands: Vec<u16>,
    /// offsetToPointA, resource blocks.
    pub offset_to_point_a: u16,
    /// scs-SpecificCarrierList.
    pub carriers: Vec<ScsCarrier>,
    /// locationAndBandwidth of the initial downlink BWP.
    pub bwp_location_and_bandwidth: u16,
    /// subcarrierSpacing of the initial downlink BWP, kHz.
    pub bwp_scs_khz: u16,
}

impl Downlink {
    /// The start and the width of the initial downlink BWP, resource blocks (TS 38.214 clause 5.1.2.2.2 with a grid of 275).
    pub fn bwp_start_and_width(&self) -> (u16, u16) {
        let riv = self.bwp_location_and_bandwidth;
        let (a, b) = (riv / 275, riv % 275);
        if a + b < 275 {
            (b, a + 1)
        } else {
            (274 - b, 277 - a - 1)
        }
    }
}

/// The SSB facts at the end of servingCellConfigCommon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsbInfo {
    /// ssb-PositionsInBurst.inOneGroup: bit 7 is SSB 0.
    pub positions: u8,
    /// ssb-PeriodicityServingCell, ms.
    pub period_ms: u16,
    /// tdd-UL-DL-ConfigurationCommon is present.
    pub tdd: bool,
    /// An uplink is configured.
    pub uplink: bool,
}

/// SIB1, the fields kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib1 {
    /// q-RxLevMin, in steps of 2 dBm.
    pub q_rx_lev_min: Option<i8>,
    /// q-QualMin, dB.
    pub q_qual_min: Option<i8>,
    /// plmn-IdentityInfoList.
    pub networks: Vec<PlmnInfo>,
    /// cellReservedForOtherUse is present.
    pub reserved_other_use: bool,
    /// si-WindowLength, slots, with scheduling information.
    pub si_window_slots: Option<u16>,
    /// schedulingInfoList.
    pub si_messages: Vec<SiMessage>,
    /// The downlink carrier, when servingCellConfigCommon was read to there.
    pub downlink: Option<Downlink>,
    /// The SSB facts, when servingCellConfigCommon was read whole.
    pub ssb: Option<SsbInfo>,
}

/// si-WindowLength values, slots.
const SI_WINDOWS: [i64; 11] = [5, 10, 20, 40, 80, 160, 320, 640, 1280, 2560, 5120];

impl Sib1 {
    /// The description of the BCCH-DL-SCH-Message that carries SIB1. `parts` counts the parts a reader read whole: 1 the access
    /// information, 2 the scheduling, 3 the downlink carrier, 4 the SSB facts.
    fn codec<P: PerIo>(&mut self, p: &mut P, parts: &mut u8) {
        *parts = 0;
        // message: c1, systemInformationBlockType1.
        let mut c1 = 0;
        p.bits(&mut c1, 1);
        let mut alt = 1;
        p.bits(&mut alt, 1);
        if c1 != 0 || alt != 1 {
            p.fail();
            return;
        }
        let has_sel = flag(p, self.q_rx_lev_min.is_some());
        let has_fail = flag(p, false);
        let has_si = flag(p, self.si_window_slots.is_some());
        let has_serving = flag(p, self.downlink.is_some());
        // ims-EmergencySupport, eCallOverIMS-Support, ue-TimersAndConstants, uac-BarringInfo, useFullResumeID,
        // lateNonCriticalExtension, nonCriticalExtension: not read, and a writer sends none.
        let mut rest = [false; 7];
        p.optionals(&mut rest);
        if has_sel {
            let has_off = flag(p, false);
            let has_sul = flag(p, false);
            let has_qual = flag(p, self.q_qual_min.is_some());
            let has_qoff = flag(p, false);
            let mut v = self.q_rx_lev_min.unwrap_or(-70);
            int(p, &mut v, -70, -22);
            self.q_rx_lev_min = Some(v);
            if has_off {
                skip_int(p, 1, 8);
            }
            if has_sul {
                skip_int(p, -70, -22);
            }
            opt_int(p, has_qual, &mut self.q_qual_min, -43, -12);
            if has_qoff {
                skip_int(p, 1, 8);
            }
        } else {
            self.q_rx_lev_min = None;
            self.q_qual_min = None;
        }
        // CellAccessRelatedInfo.
        let e = flag(p, false);
        self.reserved_other_use = flag(p, self.reserved_other_use);
        let mut n = self.networks.len().max(1);
        p.count(&mut n, 1, 12);
        self.networks.resize(n, PlmnInfo::default());
        for info in self.networks.iter_mut() {
            info.codec(p);
        }
        ext_end(p, e);
        if p.failed() {
            return;
        }
        *parts = 1;
        if has_fail {
            let has_offset = flag(p, false);
            skip_enum(p, 4);
            skip_enum(p, 8);
            if has_offset {
                skip_int(p, 0, 15);
            }
        }
        if p.reading() {
            self.si_messages.clear();
        }
        if has_si {
            let e = flag(p, false);
            let has_req = flag(p, false);
            let has_req_sul = flag(p, false);
            let has_area = flag(p, false);
            let mut n = self.si_messages.len().max(1);
            p.count(&mut n, 1, 32);
            self.si_messages.resize(n, SiMessage::default());
            for m in self.si_messages.iter_mut() {
                m.codec(p);
            }
            let mut w = i64::from(self.si_window_slots.unwrap_or(5));
            p.enum_value(&mut w, &SI_WINDOWS);
            self.si_window_slots = Some(w as u16);
            if has_req {
                si_request(p);
            }
            if has_req_sul {
                si_request(p);
            }
            if has_area {
                let mut id = 0;
                p.bits(&mut id, 24);
            }
            ext_end(p, e);
        } else {
            self.si_window_slots = None;
        }
        if p.failed() {
            return;
        }
        *parts = 2;
        if !has_serving {
            self.downlink = None;
            self.ssb = None;
            return;
        }
        // ServingCellConfigCommonSIB.
        let e_serving = flag(p, false);
        let has_uplink = flag(p, self.ssb.is_none_or(|s| s.uplink));
        let has_sul = flag(p, false);
        let has_ta = flag(p, false);
        let has_tdd = flag(p, self.ssb.is_some_and(|s| s.tdd));
        // DownlinkConfigCommonSIB: FrequencyInfoDL-SIB, the initial BWP.
        let e_dl = flag(p, false);
        let mut d = self.downlink.take().unwrap_or_default();
        band_list(p, &mut d.bands);
        int(p, &mut d.offset_to_point_a, 0, 2199);
        carrier_list(p, &mut d.carriers);
        let e_bwp = flag(p, false);
        let has_pdcch = flag(p, true);
        let has_pdsch = flag(p, true);
        bwp(p, &mut d.bwp_location_and_bandwidth, &mut d.bwp_scs_khz);
        if p.failed() {
            return;
        }
        self.downlink = Some(d);
        *parts = 3;
        let mut s = self.ssb.take().unwrap_or(SsbInfo {
            positions: 0x80,
            period_ms: 20,
            tdd: false,
            uplink: true,
        });
        bwp_downlink_rest(p, e_bwp, has_pdcch, has_pdsch);
        // BCCH-Config, PCCH-Config.
        let e = flag(p, false);
        skip_enum(p, 4);
        ext_end(p, e);
        pcch(p);
        ext_end(p, e_dl);
        if has_uplink {
            uplink(p);
        }
        if has_sul {
            uplink(p);
        }
        if has_ta {
            skip_enum(p, 3);
        }
        // ssb-PositionsInBurst, ssb-PeriodicityServingCell.
        let has_groups = flag(p, false);
        let mut pos = u64::from(s.positions);
        p.bits(&mut pos, 8);
        s.positions = pos as u8;
        if has_groups {
            let mut g = 0;
            p.bits(&mut g, 8);
        }
        let mut per = i64::from(s.period_ms);
        p.enum_value(&mut per, &[5, 10, 20, 40, 80, 160]);
        s.period_ms = per as u16;
        s.tdd = has_tdd;
        s.uplink = has_uplink;
        if p.failed() {
            return;
        }
        self.ssb = Some(s);
        *parts = 4;
        // A writer ends the SEQUENCE: no TDD pattern, ss-PBCH-BlockPower. A reader needs nothing after the periodicity.
        if !p.reading() && !has_tdd {
            skip_int(p, -60, 50);
            let _ = e_serving;
        }
    }

    /// The parts that agree with what the cell is known to send, at most `parts`: the downlink carrier (part 3) must have
    /// a carrier and an initial BWP of the common spacing of the MIB, the BWP within the carrier; the SSB facts (part 4) must
    /// mark the SS/PBCH block `ssb` that was received as sent. A part that disagrees was read out of place.
    pub fn plausible_parts(&self, parts: u8, scs_common_khz: u16, ssb: u8) -> u8 {
        let downlink = self.downlink.as_ref().is_some_and(|d| {
            let (start, width) = d.bwp_start_and_width();
            d.bwp_scs_khz == scs_common_khz
                && d.carriers
                    .iter()
                    .any(|c| c.scs_khz == scs_common_khz && start + width <= c.bandwidth_rb)
        });
        let ssb_ok = self.ssb.is_some_and(|s| ssb < 8 && s.positions & (0x80 >> ssb) != 0);
        match parts {
            3.. if !downlink => 2,
            4.. if !ssb_ok => 3,
            p => p,
        }
    }

    /// Reads the message. Returns the fields of the parts read whole and their count (1 to 4), or `None` when the message is
    /// not a SIB1 or its access information is broken.
    pub fn read(bytes: &[u8]) -> Option<(Sib1, u8)> {
        let mut r = PerReader::new(bytes);
        let mut s = Sib1::default();
        let mut parts = 0;
        s.codec(&mut r, &mut parts);
        if parts == 0 || s.networks.iter().any(|n| n.plmns.first().is_none_or(|p| p.mcc.is_none())) {
            return None;
        }
        if parts < 2 {
            s.si_messages.clear();
            s.si_window_slots = None;
        }
        if parts < 3 {
            s.downlink = None;
        }
        if parts < 4 {
            s.ssb = None;
        }
        Some((s, parts))
    }

    /// The message as octets. A SIB1 with a TDD pattern cannot be written.
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut w = PerWriter::new(&mut out);
        let mut s = self.clone();
        if let Some(x) = s.ssb.as_mut() {
            x.tdd = false;
        }
        let mut parts = 0;
        s.codec(&mut w, &mut parts);
        w.finish();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A typical SIB1.
    pub fn sample() -> Sib1 {
        Sib1 {
            q_rx_lev_min: Some(-60),
            q_qual_min: Some(-20),
            networks: vec![PlmnInfo {
                plmns: vec![Plmn::new(1, 1, 2), Plmn::new(999, 70, 3)],
                tac: Some(0x00_12_34),
                ranac: None,
                cell_identity: 0x1_2345_6789,
                reserved_operator: false,
                gnb_id_length: Some(24),
            }],
            reserved_other_use: false,
            si_window_slots: Some(20),
            si_messages: vec![
                SiMessage {
                    broadcasting: true,
                    period_frames: 16,
                    sibs: vec![(2, Some(3), false), (3, Some(1), false)],
                },
                SiMessage {
                    broadcasting: false,
                    period_frames: 64,
                    sibs: vec![(5, None, true)],
                },
            ],
            downlink: Some(Downlink {
                bands: vec![78, 77],
                offset_to_point_a: 86,
                carriers: vec![ScsCarrier {
                    offset_to_carrier: 0,
                    scs_khz: 30,
                    bandwidth_rb: 273,
                }],
                bwp_location_and_bandwidth: 1099,
                bwp_scs_khz: 30,
            }),
            ssb: Some(SsbInfo {
                positions: 0xF0,
                period_ms: 20,
                tdd: false,
                uplink: true,
            }),
        }
    }

    #[test]
    fn first_bits_by_hand() {
        // A SIB1 with the access information only, encoded by hand from the ASN.1:
        // c1 (0), systemInformationBlockType1 (1); eleven OPTIONAL flags, all 0;
        // CellAccessRelatedInfo: extension 0, cellReservedForOtherUse absent 0, list size 1 of 1..12 (0000);
        // PLMN-IdentityInfo: extension 0, trackingAreaCode present 1, ranac absent 0, list size 1 (0000);
        // PLMN-Identity: mcc present 1, digits 2 6 2 (0010 0110 0010), mnc size 2 (0), digits 0 1 (0000 0001);
        // trackingAreaCode 0x00ABCD (24 bits), cellIdentity 0x123456789 (36 bits), notReserved (1).
        let s = Sib1 {
            networks: vec![PlmnInfo {
                plmns: vec![Plmn::new(262, 1, 2)],
                tac: Some(0xABCD),
                cell_identity: 0x1_2345_6789,
                ..Default::default()
            }],
            ..Default::default()
        };
        let bits = concat!(
            "01", "00000000000", "0", "0", "0000", "0", "1", "0", "0000", "1", "001001100010", "0", "00000001",
            "000000001010101111001101", "000100100011010001010110011110001001", "1"
        );
        let mut want = Vec::new();
        for chunk in bits.as_bytes().chunks(8) {
            let mut b = 0u8;
            for (i, c) in chunk.iter().enumerate() {
                b |= (c - b'0') << (7 - i);
            }
            want.push(b);
        }
        assert_eq!(s.write(), want);
        let (r, parts) = Sib1::read(&want).unwrap();
        assert_eq!(parts, 2);
        assert_eq!(r, s);
    }

    #[test]
    fn round_trip_and_parts() {
        let s = sample();
        let bytes = s.write();
        let (r, parts) = Sib1::read(&bytes).unwrap();
        assert_eq!(parts, 4);
        assert_eq!(r, s);
        assert_eq!(r.networks[0].gnb_and_cell(), Some((0x12_3456, 0x789)));
        // The parts are kept when they agree with the cell: 30 kHz common spacing, a block among 0 to 3 sent.
        assert_eq!(r.plausible_parts(4, 30, 2), 4);
        assert_eq!(r.plausible_parts(4, 30, 5), 3);
        assert_eq!(r.plausible_parts(4, 15, 2), 2);
        assert_eq!(r.plausible_parts(2, 15, 2), 2);
        // Initial BWP of 273 RBs from RB 0: RIV = 275 x (275 - 273 + 1) + (275 - 1 - 0) = 1099.
        assert_eq!(r.downlink.as_ref().unwrap().bwp_start_and_width(), (0, 273));
        // Padding after the message changes nothing.
        let mut padded = bytes.clone();
        padded.extend([0u8; 9]);
        assert_eq!(Sib1::read(&padded).unwrap().0, s);
        // A message cut short keeps the parts before the cut.
        let mut seen = [false; 5];
        for n in 0..bytes.len() {
            match Sib1::read(&bytes[..n]) {
                Some((r, parts)) => {
                    seen[parts as usize] = true;
                    assert_eq!(r.networks, s.networks);
                    assert_eq!(r.downlink.is_some(), parts >= 3);
                    assert_eq!(r.ssb.is_some(), parts >= 4);
                }
                None => seen[0] = true,
            }
        }
        assert!(seen[0] && seen[1] && seen[2] && seen[3]);
        // Another message of the channel is not a SIB1.
        assert!(Sib1::read(&[0x00, 0x00, 0x00, 0x00]).is_none());
        // Without the gNB identity length the cell identity is not split.
        let mut t = s.clone();
        t.networks[0].gnb_id_length = None;
        let (r, _) = Sib1::read(&t.write()).unwrap();
        assert_eq!(r.networks[0].gnb_and_cell(), None);
        assert_eq!(r, t);
    }
}

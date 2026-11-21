//! The LTE broadcast messages (3GPP TS 36.331, unaligned PER): the MIB, SystemInformationBlockType1 and SystemInformation with
//! SIB2 to SIB5. Each is described once over [`PerIo`]; the same description reads and writes.

pub use crate::nbiot::msg::Plmn;
use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};
use sdrkit_util_bits::{BitOrder, Pad};
use serde::{Deserialize, Serialize};

/// The MIB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mib {
    /// dl-Bandwidth index (0..5).
    pub bw_index: u8,
    /// phich-Duration extended.
    pub phich_extended: bool,
    /// phich-Resource index (1/6, 1/2, 1, 2).
    pub ng_index: u8,
    /// The 8 most significant SFN bits.
    pub sfn_msb: u8,
    /// The 10 spare bits.
    pub spare: u16,
}

impl Mib {
    /// The 24 bits.
    pub fn to_bits(&self) -> [u8; 24] {
        let v: u32 = (self.bw_index as u32) << 21
            | (self.phich_extended as u32) << 20
            | (self.ng_index as u32 & 3) << 18
            | (self.sfn_msb as u32) << 10
            | (self.spare as u32 & 0x3FF);
        let mut b = [0u8; 24];
        for (i, x) in b.iter_mut().enumerate() {
            *x = ((v >> (23 - i)) & 1) as u8;
        }
        b
    }
    /// From 24 bits; none for a bandwidth index above 5.
    pub fn from_bits(b: &[u8]) -> Option<Mib> {
        let v = b[..24].iter().fold(0u32, |a, &x| (a << 1) | x as u32);
        let m = Mib {
            bw_index: (v >> 21) as u8,
            phich_extended: (v >> 20) & 1 == 1,
            ng_index: ((v >> 18) & 3) as u8,
            sfn_msb: ((v >> 10) & 0xFF) as u8,
            spare: (v & 0x3FF) as u16,
        };
        (m.bw_index <= 5).then_some(m)
    }
}

fn ext_bit<P: PerIo>(p: &mut P) -> bool {
    let mut e = false;
    p.flag(&mut e);
    e
}

fn ext_end<P: PerIo>(p: &mut P, e: bool) {
    if e {
        p.skip_extensions();
    }
}

fn opt<P: PerIo>(p: &mut P, present: bool) -> bool {
    let mut b = present;
    p.flag(&mut b);
    b
}

fn bits<P: PerIo>(p: &mut P, v: &mut u64, n: u32) {
    p.bits(v, n);
}

fn int<P: PerIo, T: Copy + TryFrom<i64> + Into<i64>>(p: &mut P, v: &mut T, lo: i64, hi: i64) {
    let mut x: i64 = (*v).into();
    p.int(&mut x, lo, hi);
    if let Ok(t) = T::try_from(x) {
        *v = t;
    }
}

/// Writes the `n` low bits of `v`.
fn put<P: PerIo>(p: &mut P, v: u64, n: u32) {
    let mut x = v;
    p.bits(&mut x, n);
}

fn plmn_list<P: PerIo>(p: &mut P, list: &mut Vec<(Plmn, bool)>) {
    let mut n = list.len().max(1);
    p.count(&mut n, 1, 6);
    list.resize(n, (Plmn::default(), false));
    for (pl, res) in list.iter_mut() {
        pl.codec(p);
        let mut idx = if *res { 0 } else { 1 };
        p.enumerated(&mut idx, 2, false);
        *res = idx == 0;
    }
}

/// SIB1.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib1 {
    /// PLMNs with the reserved flag.
    pub plmns: Vec<(Plmn, bool)>,
    /// TAC.
    pub tac: u16,
    /// 28-bit cell identity.
    pub cell_id: u32,
    /// Barred.
    pub barred: bool,
    /// Intra-frequency reselection allowed.
    pub intra_freq_allowed: bool,
    /// csg-Indication.
    pub csg_indication: bool,
    /// csg-Identity.
    pub csg_identity: Option<u32>,
    /// q-RxLevMin, dBm.
    pub q_rx_lev_min_dbm: i32,
    /// q-RxLevMinOffset.
    pub q_rx_lev_min_offset: Option<u8>,
    /// p-Max, dBm.
    pub p_max_dbm: Option<i32>,
    /// freqBandIndicator (1..64), replaced by the v9e0 value when that is present.
    pub band: u16,
    /// The scheduling list: (period in frames, SIB numbers).
    pub scheduling: Vec<(u16, Vec<u16>)>,
    /// tdd-Config.
    pub tdd: Option<(u8, u8)>,
    /// si-WindowLength, ms.
    pub si_window_ms: u16,
    /// systemInfoValueTag.
    pub value_tag: u8,
    /// multiBandInfoList (v8h0, v9e0 values applied).
    pub multi_band: Vec<u16>,
    /// ims-EmergencySupport-r9.
    pub ims_emergency: bool,
}

const SI_PERIODS: [i64; 7] = [8, 16, 32, 64, 128, 256, 512];
const SI_WINDOWS: [i64; 7] = [1, 2, 5, 10, 15, 20, 40];
const SIB_EXT: [u16; 6] = [19, 20, 21, 24, 25, 26];

fn sib_type<P: PerIo>(p: &mut P, sib: &mut u16) {
    let mut e = *sib > 18;
    p.flag(&mut e);
    if !e {
        let mut i = (*sib as usize).saturating_sub(3).min(15);
        p.enumerated(&mut i, 16, false);
        *sib = i as u16 + 3;
    } else {
        let mut n = SIB_EXT.iter().position(|&x| x == *sib).unwrap_or(0) as u64;
        p.small(&mut n);
        *sib = SIB_EXT.get(n as usize).copied().unwrap_or(99);
    }
}

impl Sib1 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let has_pmax = opt(p, self.p_max_dbm.is_some());
        let has_tdd = opt(p, self.tdd.is_some());
        let has_nce = opt(
            p,
            !self.multi_band.is_empty() || self.ims_emergency || self.band > 64,
        );
        // cellAccessRelatedInfo.
        let has_csg = opt(p, self.csg_identity.is_some());
        plmn_list(p, &mut self.plmns);
        int(p, &mut self.tac, 0, 65535);
        let mut cid = self.cell_id as u64;
        bits(p, &mut cid, 28);
        self.cell_id = cid as u32;
        let mut b = if self.barred { 0 } else { 1 };
        p.enumerated(&mut b, 2, false);
        self.barred = b == 0;
        let mut ifr = if self.intra_freq_allowed { 0 } else { 1 };
        p.enumerated(&mut ifr, 2, false);
        self.intra_freq_allowed = ifr == 0;
        p.flag(&mut self.csg_indication);
        if has_csg {
            let mut v = self.csg_identity.unwrap_or(0) as u64;
            bits(p, &mut v, 27);
            self.csg_identity = Some(v as u32);
        } else {
            self.csg_identity = None;
        }
        // cellSelectionInfo.
        let has_off = opt(p, self.q_rx_lev_min_offset.is_some());
        let mut q = (self.q_rx_lev_min_dbm / 2) as i64;
        p.int(&mut q, -70, -22);
        self.q_rx_lev_min_dbm = 2 * q as i32;
        if has_off {
            let mut o = self.q_rx_lev_min_offset.unwrap_or(1);
            int(p, &mut o, 1, 8);
            self.q_rx_lev_min_offset = Some(o);
        }
        if has_pmax {
            let mut v = self.p_max_dbm.unwrap_or(0) as i64;
            p.int(&mut v, -30, 33);
            self.p_max_dbm = Some(v as i32);
        } else {
            self.p_max_dbm = None;
        }
        let root: u16 = match self.band {
            b if b > 64 => 64,
            0 => 1,
            b => b,
        };
        let mut band = root as i64;
        p.int(&mut band, 1, 64);
        if p.reading() {
            self.band = band as u16;
        }
        let mut n = self.scheduling.len().max(1);
        p.count(&mut n, 1, 32);
        self.scheduling.resize(n, (16, Vec::new()));
        for (per, sibs) in self.scheduling.iter_mut() {
            let mut v = *per as i64;
            p.enum_value(&mut v, &SI_PERIODS);
            *per = v as u16;
            let mut m = sibs.len();
            p.count(&mut m, 0, 31);
            sibs.resize(m, 3);
            for s in sibs.iter_mut() {
                sib_type(p, s);
            }
        }
        if has_tdd {
            let (mut a, mut s) = self.tdd.unwrap_or((0, 0));
            let mut ai = a as usize;
            p.enumerated(&mut ai, 7, false);
            let mut si = s as usize;
            p.enumerated(&mut si, 9, false);
            a = ai as u8;
            s = si as u8;
            self.tdd = Some((a, s));
        } else {
            self.tdd = None;
        }
        let mut w = self.si_window_ms as i64;
        p.enum_value(&mut w, &SI_WINDOWS);
        self.si_window_ms = w as u16;
        int(p, &mut self.value_tag, 0, 31);
        if has_nce {
            self.v890(p);
        }
    }

    fn v890<P: PerIo>(&mut self, p: &mut P) {
        let want_late = !self.multi_band.is_empty() || self.band > 64;
        let has_late = opt(p, want_late);
        let has_920 = opt(p, self.ims_emergency);
        if has_late {
            if p.reading() {
                let mut bytes = Vec::new();
                p.octets_unconstrained(&mut bytes);
                let mut r = PerReader::new(&bytes);
                self.v8h0(&mut r);
                if r.failed() {
                    p.fail();
                }
            } else {
                let mut bytes = Vec::new();
                let mut w = PerWriter::new(&mut bytes);
                self.clone().v8h0(&mut w);
                w.finish();
                p.octets_unconstrained(&mut bytes);
            }
        }
        if has_920 {
            let ims = opt(p, self.ims_emergency);
            let csi = opt(p, false);
            let nce = opt(p, false);
            self.ims_emergency = ims;
            if csi {
                // cellSelectionInfo-v920: q-QualMin-r9 (-34..-3), offset optional.
                let has = opt(p, false);
                let mut q = 0i64;
                p.int(&mut q, -34, -3);
                if has {
                    let mut o = 1i64;
                    p.int(&mut o, 1, 8);
                }
            }
            let _ = nce;
        }
    }

    fn v8h0<P: PerIo>(&mut self, p: &mut P) {
        // The root list holds 1..64 values; values above 64 go into the v9e0 list as 64 plus the real value.
        let has_mb = opt(p, !self.multi_band.is_empty());
        let need_9e0 = self.band > 64 || self.multi_band.iter().any(|&b| b > 64);
        let has_9e0 = opt(p, need_9e0);
        let mut mb_root: Vec<u16> = self
            .multi_band
            .iter()
            .map(|&b| if b > 64 { 64 } else { b })
            .collect();
        if has_mb {
            let mut n = mb_root.len().max(1);
            p.count(&mut n, 1, 8);
            mb_root.resize(n, 1);
            for b in mb_root.iter_mut() {
                int(p, b, 1, 64);
            }
        }
        let mut band_9e0: Option<u16> = (self.band > 64).then_some(self.band);
        let mut mb_9e0: Vec<Option<u16>> = self
            .multi_band
            .iter()
            .map(|&b| (b > 64).then_some(b))
            .collect();
        if has_9e0 {
            let hb = opt(p, band_9e0.is_some());
            let hm = opt(p, mb_9e0.iter().any(|x| x.is_some()));
            let _nce = opt(p, false);
            if hb {
                let mut v = band_9e0.unwrap_or(65);
                int(p, &mut v, 65, 256);
                band_9e0 = Some(v);
            }
            if hm {
                let mut n = mb_9e0.len().max(1);
                p.count(&mut n, 1, 8);
                mb_9e0.resize(n, None);
                for e in mb_9e0.iter_mut() {
                    let h = opt(p, e.is_some());
                    if h {
                        let mut v = e.unwrap_or(65);
                        int(p, &mut v, 65, 256);
                        *e = Some(v);
                    } else {
                        *e = None;
                    }
                }
            }
        }
        if p.reading() {
            if let Some(b) = band_9e0 {
                if self.band == 64 {
                    self.band = b;
                }
            }
            self.multi_band = mb_root
                .iter()
                .enumerate()
                .map(|(i, &b)| {
                    if b == 64 {
                        mb_9e0.get(i).copied().flatten().unwrap_or(64)
                    } else {
                        b
                    }
                })
                .collect();
        }
    }
}

/// An access class barring configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AcBarring {
    /// Factor index (p00 .. p95).
    pub factor: u8,
    /// Time index.
    pub time: u8,
    /// Special classes.
    pub special: u8,
}

impl AcBarring {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        int(p, &mut self.factor, 0, 15);
        int(p, &mut self.time, 0, 7);
        int(p, &mut self.special, 0, 31);
    }
}

/// SIB2 (the fields read; the rest is carried through for writing).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib2 {
    /// ac-BarringInfo: emergency, signalling, data.
    pub ac_barring: Option<(bool, Option<AcBarring>, Option<AcBarring>)>,
    /// numberOfRA-Preambles.
    pub ra_preambles: u8,
    /// powerRampingStep index.
    pub ramp_step: u8,
    /// preambleInitialReceivedTargetPower index.
    pub target_power: u8,
    /// preambleTransMax.
    pub preamble_trans_max: u16,
    /// ra-ResponseWindowSize, subframes.
    pub ra_window: u8,
    /// mac-ContentionResolutionTimer index.
    pub cr_timer: u8,
    /// maxHARQ-Msg3Tx.
    pub max_harq_msg3: u8,
    /// modificationPeriodCoeff.
    pub mod_period_coeff: u8,
    /// defaultPagingCycle, frames.
    pub paging_cycle: u16,
    /// nB index.
    pub nb: u8,
    /// rootSequenceIndex.
    pub root_sequence: u16,
    /// prach-ConfigIndex.
    pub prach_config: u8,
    /// highSpeedFlag.
    pub high_speed: bool,
    /// zeroCorrelationZoneConfig.
    pub zcz: u8,
    /// prach-FreqOffset.
    pub prach_freq_offset: u8,
    /// referenceSignalPower, dBm.
    pub ref_signal_power: i8,
    /// p-b.
    pub p_b: u8,
    /// n-SB, hopping mode, hopping offset, 64QAM, group hopping, group assignment, sequence hopping, cyclic shift.
    pub pusch: (u8, bool, u8, bool, bool, u8, bool, u8),
    /// deltaPUCCH-Shift index, nRB-CQI, nCS-AN, n1PUCCH-AN.
    pub pucch: (u8, u8, u8, u16),
    /// SRS setup: bandwidth config, subframe config, ack/nack simultaneous, max UpPTS.
    pub srs: Option<(u8, u8, bool, bool)>,
    /// p0-NominalPUSCH.
    pub p0_pusch: i16,
    /// alpha index.
    pub alpha: u8,
    /// p0-NominalPUCCH.
    pub p0_pucch: i16,
    /// deltaF list indices.
    pub delta_f: [u8; 5],
    /// deltaPreambleMsg3.
    pub delta_preamble_msg3: i8,
    /// ul-CyclicPrefixLength len2.
    pub ul_cp_len2: bool,
    /// t300, t301 (ms), t310 (ms), n310, t311 (ms), n311.
    pub timers: (u16, u16, u16, u8, u16, u8),
    /// ul-CarrierFreq.
    pub ul_earfcn: Option<u16>,
    /// ul-Bandwidth in RBs.
    pub ul_bw_rb: Option<u8>,
    /// additionalSpectrumEmission.
    pub add_spectrum_emission: u8,
    /// MBSFN allocations counted.
    pub mbsfn: u8,
    /// timeAlignmentTimerCommon, subframes (none: infinity).
    pub time_alignment: Option<u16>,
    /// Later-release additions present.
    pub extended: bool,
}

const PREAMBLE_TRANS_MAX: [i64; 11] = [3, 4, 5, 6, 7, 8, 10, 20, 50, 100, 200];
const RA_WINDOW: [i64; 8] = [2, 3, 4, 5, 6, 7, 8, 10];
const MOD_COEFF: [i64; 4] = [2, 4, 8, 16];
const PAGING: [i64; 4] = [32, 64, 128, 256];
const T300: [i64; 8] = [100, 200, 300, 400, 600, 1000, 1500, 2000];
const T310: [i64; 7] = [0, 50, 100, 200, 500, 1000, 2000];
const N310: [i64; 8] = [1, 2, 3, 4, 6, 8, 10, 20];
const T311: [i64; 7] = [1000, 3000, 5000, 10000, 15000, 20000, 30000];
const N311: [i64; 8] = [1, 2, 3, 4, 5, 6, 8, 10];
const UL_BW: [i64; 6] = [6, 15, 25, 50, 75, 100];
const TAT: [i64; 8] = [500, 750, 1280, 1920, 2560, 5120, 10240, 0];

fn ev<P: PerIo, T: Copy + TryFrom<i64> + Into<i64>>(p: &mut P, v: &mut T, table: &[i64]) {
    let mut x: i64 = (*v).into();
    p.enum_value(&mut x, table);
    if let Ok(t) = T::try_from(x) {
        *v = t;
    }
}

impl Sib2 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let has_ac = opt(p, self.ac_barring.is_some());
        let has_mbsfn = opt(p, self.mbsfn > 0);
        if has_ac {
            let (mut em, mut sig, mut dat) = self.ac_barring.unwrap_or((false, None, None));
            let hs = opt(p, sig.is_some());
            let hd = opt(p, dat.is_some());
            p.flag(&mut em);
            if hs {
                let mut c = sig.unwrap_or_default();
                c.codec(p);
                sig = Some(c);
            } else {
                sig = None;
            }
            if hd {
                let mut c = dat.unwrap_or_default();
                c.codec(p);
                dat = Some(c);
            } else {
                dat = None;
            }
            self.ac_barring = Some((em, sig, dat));
        } else {
            self.ac_barring = None;
        }
        // radioResourceConfigCommon.
        let e_rr = ext_bit(p);
        {
            // rach-ConfigCommon.
            let e_rach = ext_bit(p);
            let has_ga = opt(p, false);
            let mut np = (self.ra_preambles as usize / 4).saturating_sub(1);
            p.enumerated(&mut np, 16, false);
            self.ra_preambles = (4 * (np + 1)) as u8;
            if has_ga {
                let ge = ext_bit(p);
                let mut x = 0u64;
                p.bits(&mut x, 4 + 2 + 3);
                ext_end(p, ge);
            }
            int(p, &mut self.ramp_step, 0, 3);
            int(p, &mut self.target_power, 0, 15);
            ev(p, &mut self.preamble_trans_max, &PREAMBLE_TRANS_MAX);
            ev(p, &mut self.ra_window, &RA_WINDOW);
            int(p, &mut self.cr_timer, 0, 7);
            int(p, &mut self.max_harq_msg3, 1, 8);
            ext_end(p, e_rach);
        }
        ev(p, &mut self.mod_period_coeff, &MOD_COEFF);
        ev(p, &mut self.paging_cycle, &PAGING);
        int(p, &mut self.nb, 0, 7);
        int(p, &mut self.root_sequence, 0, 837);
        int(p, &mut self.prach_config, 0, 63);
        p.flag(&mut self.high_speed);
        int(p, &mut self.zcz, 0, 15);
        int(p, &mut self.prach_freq_offset, 0, 94);
        int(p, &mut self.ref_signal_power, -60, 50);
        int(p, &mut self.p_b, 0, 3);
        {
            let u = &mut self.pusch;
            int(p, &mut u.0, 1, 4);
            p.flag(&mut u.1);
            int(p, &mut u.2, 0, 98);
            p.flag(&mut u.3);
            p.flag(&mut u.4);
            int(p, &mut u.5, 0, 29);
            p.flag(&mut u.6);
            int(p, &mut u.7, 0, 7);
        }
        {
            let c = &mut self.pucch;
            int(p, &mut c.0, 0, 2);
            int(p, &mut c.1, 0, 98);
            int(p, &mut c.2, 0, 7);
            int(p, &mut c.3, 0, 2047);
        }
        // soundingRS-UL-ConfigCommon: CHOICE release / setup.
        let mut setup = self.srs.is_some() as usize;
        p.enumerated(&mut setup, 2, false);
        if setup == 1 {
            let (mut bw, mut sf, mut an, mut up) = self.srs.unwrap_or((0, 0, false, false));
            let has_up = opt(p, up);
            int(p, &mut bw, 0, 7);
            int(p, &mut sf, 0, 15);
            p.flag(&mut an);
            up = has_up;
            self.srs = Some((bw, sf, an, up));
        } else {
            self.srs = None;
        }
        int(p, &mut self.p0_pusch, -126, 24);
        int(p, &mut self.alpha, 0, 7);
        int(p, &mut self.p0_pucch, -127, -96);
        let df = &mut self.delta_f;
        int(p, &mut df[0], 0, 2);
        int(p, &mut df[1], 0, 2);
        int(p, &mut df[2], 0, 3);
        int(p, &mut df[3], 0, 2);
        int(p, &mut df[4], 0, 2);
        int(p, &mut self.delta_preamble_msg3, -1, 6);
        p.flag(&mut self.ul_cp_len2);
        ext_end(p, e_rr);
        // ue-TimersAndConstants.
        let e_t = ext_bit(p);
        {
            let t = &mut self.timers;
            ev(p, &mut t.0, &T300);
            ev(p, &mut t.1, &T300);
            ev(p, &mut t.2, &T310);
            ev(p, &mut t.3, &N310);
            ev(p, &mut t.4, &T311);
            ev(p, &mut t.5, &N311);
        }
        ext_end(p, e_t);
        // freqInfo.
        let hf = opt(p, self.ul_earfcn.is_some());
        let hb = opt(p, self.ul_bw_rb.is_some());
        if hf {
            let mut v = self.ul_earfcn.unwrap_or(0);
            int(p, &mut v, 0, 65535);
            self.ul_earfcn = Some(v);
        } else {
            self.ul_earfcn = None;
        }
        if hb {
            let mut v = self.ul_bw_rb.unwrap_or(6);
            ev(p, &mut v, &UL_BW);
            self.ul_bw_rb = Some(v);
        } else {
            self.ul_bw_rb = None;
        }
        int(p, &mut self.add_spectrum_emission, 1, 32);
        if has_mbsfn {
            let mut n = self.mbsfn.max(1) as usize;
            p.count(&mut n, 1, 8);
            self.mbsfn = n as u8;
            for _ in 0..n {
                let mut x = 0u64;
                p.bits(&mut x, 3 + 3);
                let mut four = false;
                p.flag(&mut four);
                let mut y = 0u64;
                p.bits(&mut y, if four { 24 } else { 6 });
            }
        } else {
            self.mbsfn = 0;
        }
        let mut tat = self.time_alignment.map(|v| v as i64).unwrap_or(0);
        p.enum_value(&mut tat, &TAT);
        self.time_alignment = (tat != 0).then_some(tat as u16);
        self.extended = e;
        ext_end(p, e);
    }
}

/// SIB3.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib3 {
    /// q-Hyst, dB.
    pub q_hyst_db: u8,
    /// s-NonIntraSearch (raw, x2 dB).
    pub s_non_intra: Option<u8>,
    /// threshServingLow (raw).
    pub thresh_serving_low: u8,
    /// cellReselectionPriority.
    pub priority: u8,
    /// q-RxLevMin, dBm.
    pub q_rx_lev_min_dbm: i32,
    /// p-Max.
    pub p_max: Option<i8>,
    /// s-IntraSearch (raw).
    pub s_intra: Option<u8>,
    /// allowedMeasBandwidth index.
    pub allowed_bw: Option<u8>,
    /// presenceAntennaPort1.
    pub antenna_port1: bool,
    /// neighCellConfig.
    pub neigh_cell_config: u8,
    /// t-ReselectionEUTRA, s.
    pub t_reselection: u8,
}

const Q_HYST: [i64; 16] = [0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24];

impl Sib3 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let has_speed = opt(p, false);
        ev(p, &mut self.q_hyst_db, &Q_HYST);
        if has_speed {
            let mut x = 0u64;
            p.bits(&mut x, 3 + 3 + 4 + 4 + 2 + 2);
        }
        let hn = opt(p, self.s_non_intra.is_some());
        if hn {
            let mut v = self.s_non_intra.unwrap_or(0);
            int(p, &mut v, 0, 31);
            self.s_non_intra = Some(v);
        } else {
            self.s_non_intra = None;
        }
        int(p, &mut self.thresh_serving_low, 0, 31);
        int(p, &mut self.priority, 0, 7);
        let hp = opt(p, self.p_max.is_some());
        let hs = opt(p, self.s_intra.is_some());
        let hb = opt(p, self.allowed_bw.is_some());
        let hsf = opt(p, false);
        let mut q = (self.q_rx_lev_min_dbm / 2) as i64;
        p.int(&mut q, -70, -22);
        self.q_rx_lev_min_dbm = 2 * q as i32;
        if hp {
            let mut v = self.p_max.unwrap_or(0);
            int(p, &mut v, -30, 33);
            self.p_max = Some(v);
        } else {
            self.p_max = None;
        }
        if hs {
            let mut v = self.s_intra.unwrap_or(0);
            int(p, &mut v, 0, 31);
            self.s_intra = Some(v);
        } else {
            self.s_intra = None;
        }
        if hb {
            let mut v = self.allowed_bw.unwrap_or(0);
            int(p, &mut v, 0, 5);
            self.allowed_bw = Some(v);
        } else {
            self.allowed_bw = None;
        }
        p.flag(&mut self.antenna_port1);
        int(p, &mut self.neigh_cell_config, 0, 3);
        int(p, &mut self.t_reselection, 0, 7);
        if hsf {
            let mut x = 0u64;
            p.bits(&mut x, 4);
        }
        ext_end(p, e);
    }
}

/// Q-OffsetRange values, dB.
pub const Q_OFFSET: [i64; 31] = [
    -24, -22, -20, -18, -16, -14, -12, -10, -8, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 8, 10,
    12, 14, 16, 18, 20, 22, 24,
];

fn cell_range_list<P: PerIo>(p: &mut P, list: &mut Vec<(u16, Option<u8>)>) {
    let mut n = list.len().max(1);
    p.count(&mut n, 1, 16);
    list.resize(n, (0, None));
    for (start, range) in list.iter_mut() {
        let hr = opt(p, range.is_some());
        int(p, start, 0, 503);
        if hr {
            let mut r = range.unwrap_or(0);
            int(p, &mut r, 0, 15);
            *range = Some(r);
        } else {
            *range = None;
        }
    }
}

/// SIB4.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib4 {
    /// Intra-frequency neighbours: (PCI, q-OffsetCell dB).
    pub neighbours: Vec<(u16, i8)>,
    /// Black list: (start, range index).
    pub black: Vec<(u16, Option<u8>)>,
    /// csg-PhysCellIdRange.
    pub csg_range: Option<(u16, Option<u8>)>,
}

impl Sib4 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let hn = opt(p, !self.neighbours.is_empty());
        let hb = opt(p, !self.black.is_empty());
        let hc = opt(p, self.csg_range.is_some());
        if hn {
            let mut n = self.neighbours.len().max(1);
            p.count(&mut n, 1, 16);
            self.neighbours.resize(n, (0, 0));
            for (pci, q) in self.neighbours.iter_mut() {
                let ne = ext_bit(p);
                int(p, pci, 0, 503);
                ev(p, q, &Q_OFFSET);
                ext_end(p, ne);
            }
        } else {
            self.neighbours.clear();
        }
        if hb {
            cell_range_list(p, &mut self.black);
        } else {
            self.black.clear();
        }
        if hc {
            let mut v = vec![self.csg_range.unwrap_or((0, None))];
            let (mut s, mut r) = v.remove(0);
            let hr = opt(p, r.is_some());
            int(p, &mut s, 0, 503);
            if hr {
                let mut x = r.unwrap_or(0);
                int(p, &mut x, 0, 15);
                r = Some(x);
            } else {
                r = None;
            }
            self.csg_range = Some((s, r));
        } else {
            self.csg_range = None;
        }
        ext_end(p, e);
    }
}

/// One inter-frequency carrier of SIB5.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InterFreq {
    /// dl-CarrierFreq.
    pub earfcn: u16,
    /// q-RxLevMin, dBm.
    pub q_rx_lev_min_dbm: i32,
    /// p-Max.
    pub p_max: Option<i8>,
    /// t-ReselectionEUTRA.
    pub t_reselection: u8,
    /// threshX-High (raw).
    pub thresh_high: u8,
    /// threshX-Low (raw).
    pub thresh_low: u8,
    /// allowedMeasBandwidth index.
    pub allowed_bw: u8,
    /// presenceAntennaPort1.
    pub antenna_port1: bool,
    /// cellReselectionPriority.
    pub priority: Option<u8>,
    /// neighCellConfig.
    pub neigh_cell_config: u8,
    /// q-OffsetFreq, dB.
    pub q_offset_freq: Option<i8>,
    /// Neighbours: (PCI, q-OffsetCell dB).
    pub neighbours: Vec<(u16, i8)>,
    /// Black list.
    pub black: Vec<(u16, Option<u8>)>,
}

impl InterFreq {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let hp = opt(p, self.p_max.is_some());
        let hsf = opt(p, false);
        let hpr = opt(p, self.priority.is_some());
        let hq = opt(p, self.q_offset_freq.is_some());
        let hn = opt(p, !self.neighbours.is_empty());
        let hb = opt(p, !self.black.is_empty());
        int(p, &mut self.earfcn, 0, 65535);
        let mut q = (self.q_rx_lev_min_dbm / 2) as i64;
        p.int(&mut q, -70, -22);
        self.q_rx_lev_min_dbm = 2 * q as i32;
        if hp {
            let mut v = self.p_max.unwrap_or(0);
            int(p, &mut v, -30, 33);
            self.p_max = Some(v);
        } else {
            self.p_max = None;
        }
        int(p, &mut self.t_reselection, 0, 7);
        if hsf {
            let mut x = 0u64;
            p.bits(&mut x, 4);
        }
        int(p, &mut self.thresh_high, 0, 31);
        int(p, &mut self.thresh_low, 0, 31);
        int(p, &mut self.allowed_bw, 0, 5);
        p.flag(&mut self.antenna_port1);
        if hpr {
            let mut v = self.priority.unwrap_or(0);
            int(p, &mut v, 0, 7);
            self.priority = Some(v);
        } else {
            self.priority = None;
        }
        int(p, &mut self.neigh_cell_config, 0, 3);
        if hq {
            let mut v = self.q_offset_freq.unwrap_or(0);
            ev(p, &mut v, &Q_OFFSET);
            self.q_offset_freq = Some(v);
        } else {
            self.q_offset_freq = None;
        }
        if hn {
            let mut n = self.neighbours.len().max(1);
            p.count(&mut n, 1, 16);
            self.neighbours.resize(n, (0, 0));
            for (pci, q) in self.neighbours.iter_mut() {
                int(p, pci, 0, 503);
                ev(p, q, &Q_OFFSET);
            }
        } else {
            self.neighbours.clear();
        }
        if hb {
            cell_range_list(p, &mut self.black);
        } else {
            self.black.clear();
        }
        ext_end(p, e);
    }
}

/// SIB5.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib5 {
    /// The carriers.
    pub carriers: Vec<InterFreq>,
}

impl Sib5 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let mut n = self.carriers.len().max(1);
        p.count(&mut n, 1, 8);
        self.carriers.resize(n, InterFreq::default());
        for c in self.carriers.iter_mut() {
            c.codec(p);
        }
        ext_end(p, e);
    }
}

/// One entry of a SystemInformation message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SibEntry {
    /// SIB2.
    Sib2(Box<Sib2>),
    /// SIB3.
    Sib3(Sib3),
    /// SIB4.
    Sib4(Sib4),
    /// SIB5.
    Sib5(Sib5),
    /// A later SIB whose content is not parsed (its number; 12 for "12+").
    Other(u16),
}

/// A BCCH-DL-SCH message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Bcch {
    /// SIB1.
    Sib1(Box<Sib1>),
    /// SystemInformation.
    Si(Vec<SibEntry>),
}

impl Bcch {
    /// Parses a transport block; none when it fails (runs past the end or breaks a constraint).
    pub fn parse(tb: &[u8]) -> Option<Bcch> {
        let mut bytes = Vec::new();
        pack(tb, BitOrder::Msb, Pad::Zero, &mut bytes);
        let mut r = PerReader::new(&bytes);
        let mut ext = false;
        r.flag(&mut ext);
        if ext {
            return None;
        }
        let mut c = false;
        r.flag(&mut c);
        let out = if c {
            let mut s = Sib1::default();
            s.codec(&mut r);
            if s.plmns.is_empty() {
                return None;
            }
            Bcch::Sib1(Box::new(s))
        } else {
            let mut ce = false;
            r.flag(&mut ce);
            if ce {
                return None;
            }
            let mut _nce = false;
            r.flag(&mut _nce);
            let mut n = 1usize;
            r.count(&mut n, 1, 32);
            let mut v = Vec::new();
            for _ in 0..n {
                let mut e = false;
                r.flag(&mut e);
                if e {
                    v.push(SibEntry::Other(12));
                    break;
                }
                let mut idx = 0usize;
                r.enumerated(&mut idx, 10, false);
                match idx {
                    0 => {
                        let mut s = Sib2::default();
                        s.codec(&mut r);
                        v.push(SibEntry::Sib2(Box::new(s)));
                    }
                    1 => {
                        let mut s = Sib3::default();
                        s.codec(&mut r);
                        v.push(SibEntry::Sib3(s));
                    }
                    2 => {
                        let mut s = Sib4::default();
                        s.codec(&mut r);
                        v.push(SibEntry::Sib4(s));
                    }
                    3 => {
                        let mut s = Sib5::default();
                        s.codec(&mut r);
                        v.push(SibEntry::Sib5(s));
                    }
                    k => {
                        v.push(SibEntry::Other(k as u16 + 2));
                        break;
                    }
                }
                if r.failed() {
                    return None;
                }
            }
            Bcch::Si(v)
        };
        (!r.failed()).then_some(out)
    }

    /// Encodes the message (bits, not padded).
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut w = PerWriter::new(&mut bytes);
        put(&mut w, 0, 1);
        match self {
            Bcch::Sib1(s) => {
                put(&mut w, 1, 1);
                s.as_ref().clone().codec(&mut w);
            }
            Bcch::Si(entries) => {
                put(&mut w, 0, 1);
                put(&mut w, 0, 1);
                put(&mut w, 0, 1);
                let mut n = entries.len();
                w.count(&mut n, 1, 32);
                for e in entries {
                    put(&mut w, 0, 1);
                    match e {
                        SibEntry::Sib2(s) => {
                            put(&mut w, 0, 4);
                            s.as_ref().clone().codec(&mut w);
                        }
                        SibEntry::Sib3(s) => {
                            put(&mut w, 1, 4);
                            s.clone().codec(&mut w);
                        }
                        SibEntry::Sib4(s) => {
                            put(&mut w, 2, 4);
                            s.clone().codec(&mut w);
                        }
                        SibEntry::Sib5(s) => {
                            put(&mut w, 3, 4);
                            s.clone().codec(&mut w);
                        }
                        SibEntry::Other(_) => {}
                    }
                }
            }
        }
        let n = w.finish();
        let mut b = Vec::new();
        unpack(&bytes, BitOrder::Msb, &mut b);
        b.truncate(n);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mib_bits() {
        let m = Mib {
            bw_index: 2,
            phich_extended: false,
            ng_index: 1,
            sfn_msb: 15,
            spare: 0,
        };
        assert_eq!(Mib::from_bits(&m.to_bits()), Some(m));
        let mut b = m.to_bits();
        b[0] = 1;
        b[1] = 1;
        assert_eq!(Mib::from_bits(&b), None);
    }

    #[test]
    fn sib1_with_bands_and_extensions() {
        let s = Sib1 {
            plmns: vec![(Plmn::new(1, 1, 2), false), (Plmn::new(246, 3, 2), true)],
            tac: 77,
            cell_id: 0x1B5901,
            barred: false,
            intra_freq_allowed: true,
            q_rx_lev_min_dbm: -124,
            p_max_dbm: Some(23),
            band: 66,
            scheduling: vec![(16, vec![]), (32, vec![3]), (64, vec![4, 5, 13])],
            si_window_ms: 20,
            value_tag: 5,
            multi_band: vec![3, 20, 28, 66, 71],
            ims_emergency: true,
            ..Default::default()
        };
        let bits = Bcch::Sib1(Box::new(s.clone())).encode();
        match Bcch::parse(&bits) {
            Some(Bcch::Sib1(b)) => assert_eq!(*b, s),
            x => panic!("{x:?}"),
        }
        assert_eq!(Bcch::parse(&bits[..bits.len() / 2]), None);
    }

    #[test]
    fn si_stops_at_a_later_sib() {
        let mut bytes = Vec::new();
        let mut w = PerWriter::new(&mut bytes);
        put(&mut w, 0, 1);
        put(&mut w, 0, 1);
        put(&mut w, 0, 2);
        put(&mut w, 1, 5); // two entries
        put(&mut w, 1, 1); // an extension (SIB12 or later)
        w.finish();
        let mut b = Vec::new();
        unpack(&bytes, BitOrder::Msb, &mut b);
        b.resize(64, 0);
        assert_eq!(Bcch::parse(&b), Some(Bcch::Si(vec![SibEntry::Other(12)])));
    }
}

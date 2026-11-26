//! The LTE system information on other radio systems (3GPP TS 36.331 clause 6.3.1, unaligned PER): SIB6 (UTRA carriers),
//! SIB7 (GERAN carriers), SIB8 (CDMA2000 carriers and cells) and SIB24 (NR carriers). Each is described once over
//! [`PerIo`]; the same description reads and writes. Extension additions are skipped when read and not written.

use super::msg::{ext_bit, ext_end, int, opt};
use sdrkit_util_bits::per::PerIo;
use serde::{Deserialize, Serialize};

/// A cell reselection priority, INTEGER (0..7), when present.
fn priority<P: PerIo>(p: &mut P, present: bool, v: &mut Option<u8>) {
    if present {
        let mut x = v.unwrap_or(0);
        int(p, &mut x, 0, 7);
        *v = Some(x);
    } else {
        *v = None;
    }
}

/// SpeedStateScaleFactors: sf-Medium and sf-High, two ENUMERATED of 4.
fn speed<P: PerIo>(p: &mut P, present: bool, v: &mut Option<(u8, u8)>) {
    if present {
        let (mut a, mut b) = v.unwrap_or((0, 0));
        int(p, &mut a, 0, 3);
        int(p, &mut b, 0, 3);
        *v = Some((a, b));
    } else {
        *v = None;
    }
}

/// A SEQUENCE OF of `lo..=hi` items, each by `f`.
fn list<P: PerIo, T: Default + Clone>(p: &mut P, v: &mut Vec<T>, lo: usize, hi: usize, mut f: impl FnMut(&mut P, &mut T)) {
    let mut n = v.len().max(lo);
    p.count(&mut n, lo, hi);
    if p.failed() {
        return;
    }
    v.resize(n, T::default());
    for x in v.iter_mut() {
        f(p, x);
    }
}

/// A BIT STRING of `lo..=hi` bits, any length, one bit per byte.
fn bit_list<P: PerIo>(p: &mut P, v: &mut Vec<u8>, lo: usize, hi: usize) {
    list(p, v, lo, hi, |p, b| {
        let mut x = u64::from(*b);
        p.bits(&mut x, 1);
        *b = x as u8;
    });
}

/// A carrier of UTRA FDD or TDD (CarrierFreqUTRA-FDD, CarrierFreqUTRA-TDD).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UtraCarrier {
    /// carrierFreq: the UARFCN of the downlink.
    pub uarfcn: u16,
    /// cellReselectionPriority.
    pub priority: Option<u8>,
    /// threshX-High (raw, x2 dB).
    pub thresh_high: u8,
    /// threshX-Low (raw, x2 dB).
    pub thresh_low: u8,
    /// q-RxLevMin (raw): field x 2 + 1 dBm.
    pub q_rx_lev_min: i8,
    /// p-MaxUTRA, dBm.
    pub p_max: i8,
    /// q-QualMin, dB (FDD only).
    pub q_qual_min: Option<i8>,
}

impl UtraCarrier {
    fn codec<P: PerIo>(&mut self, p: &mut P, fdd: bool) {
        let e = ext_bit(p);
        let hp = opt(p, self.priority.is_some());
        int(p, &mut self.uarfcn, 0, 16383);
        priority(p, hp, &mut self.priority);
        int(p, &mut self.thresh_high, 0, 31);
        int(p, &mut self.thresh_low, 0, 31);
        int(p, &mut self.q_rx_lev_min, -60, -13);
        int(p, &mut self.p_max, -50, 33);
        if fdd {
            let mut q = self.q_qual_min.unwrap_or(0);
            int(p, &mut q, -24, 0);
            self.q_qual_min = Some(q);
        } else {
            self.q_qual_min = None;
        }
        ext_end(p, e);
    }
    /// q-RxLevMin, dBm.
    pub fn q_rx_lev_min_dbm(&self) -> i32 {
        2 * i32::from(self.q_rx_lev_min) + 1
    }
}

/// SystemInformationBlockType6: the UTRA carriers for cell reselection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib6 {
    /// carrierFreqListUTRA-FDD.
    pub fdd: Vec<UtraCarrier>,
    /// carrierFreqListUTRA-TDD.
    pub tdd: Vec<UtraCarrier>,
    /// t-ReselectionUTRA, s.
    pub t_reselection: u8,
    /// t-ReselectionUTRA-SF.
    pub speed: Option<(u8, u8)>,
}

impl Sib6 {
    /// Reads or writes the message.
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let hf = opt(p, !self.fdd.is_empty());
        let ht = opt(p, !self.tdd.is_empty());
        let hs = opt(p, self.speed.is_some());
        if hf {
            list(p, &mut self.fdd, 1, 16, |p, c| c.codec(p, true));
        } else {
            self.fdd.clear();
        }
        if ht {
            list(p, &mut self.tdd, 1, 16, |p, c| c.codec(p, false));
        } else {
            self.tdd.clear();
        }
        int(p, &mut self.t_reselection, 0, 7);
        speed(p, hs, &mut self.speed);
        ext_end(p, e);
    }
}

/// How a group of GERAN carriers names the ARFCNs after the first (CarrierFreqsGERAN followingARFCNs).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GeranFollowing {
    /// explicitListOfARFCNs.
    Explicit(Vec<u16>),
    /// equallySpacedARFCNs: the spacing and the number of ARFCNs after the first.
    Equal(u8, u8),
    /// variableBitMapOfARFCNs: bit i of the map (from the leading bit) is ARFCN first + 1 + i.
    Bitmap(Vec<u8>),
}

impl Default for GeranFollowing {
    fn default() -> Self {
        GeranFollowing::Explicit(Vec::new())
    }
}

/// A group of GERAN carriers with its reselection parameters (CarrierFreqsInfoGERAN).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GeranGroup {
    /// startingARFCN.
    pub first: u16,
    /// bandIndicator is pcs1900 (else dcs1800); it has a meaning for ARFCNs 512 to 810 only.
    pub pcs1900: bool,
    /// followingARFCNs.
    pub following: GeranFollowing,
    /// cellReselectionPriority.
    pub priority: Option<u8>,
    /// ncc-Permitted: bit N-1 (from the leading bit) permits NCC N-1.
    pub ncc_permitted: u8,
    /// q-RxLevMin (raw): field x 2 - 115 dBm.
    pub q_rx_lev_min: u8,
    /// p-MaxGERAN, dBm.
    pub p_max: Option<u8>,
    /// threshX-High (raw, x2 dB).
    pub thresh_high: u8,
    /// threshX-Low (raw, x2 dB).
    pub thresh_low: u8,
}

impl GeranGroup {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        int(p, &mut self.first, 0, 1023);
        p.flag(&mut self.pcs1900);
        let mut idx = match self.following {
            GeranFollowing::Explicit(_) => 0,
            GeranFollowing::Equal(..) => 1,
            GeranFollowing::Bitmap(_) => 2,
        };
        p.choice(&mut idx, 3, false);
        match idx {
            0 => {
                let mut v = match &self.following {
                    GeranFollowing::Explicit(v) => v.clone(),
                    _ => Vec::new(),
                };
                list(p, &mut v, 0, 31, |p, a| int(p, a, 0, 1023));
                self.following = GeranFollowing::Explicit(v);
            }
            1 => {
                let (mut d, mut n) = match self.following {
                    GeranFollowing::Equal(d, n) => (d, n),
                    _ => (1, 0),
                };
                int(p, &mut d, 1, 8);
                int(p, &mut n, 0, 31);
                self.following = GeranFollowing::Equal(d, n);
            }
            _ => {
                let mut v = match &self.following {
                    GeranFollowing::Bitmap(v) => v.clone(),
                    _ => vec![0],
                };
                p.octets(&mut v, 1, 16);
                self.following = GeranFollowing::Bitmap(v);
            }
        }
        let hp = opt(p, self.priority.is_some());
        let hm = opt(p, self.p_max.is_some());
        priority(p, hp, &mut self.priority);
        int(p, &mut self.ncc_permitted, 0, 255);
        int(p, &mut self.q_rx_lev_min, 0, 45);
        if hm {
            let mut x = self.p_max.unwrap_or(0);
            int(p, &mut x, 0, 39);
            self.p_max = Some(x);
        } else {
            self.p_max = None;
        }
        int(p, &mut self.thresh_high, 0, 31);
        int(p, &mut self.thresh_low, 0, 31);
        ext_end(p, e);
    }
    /// The ARFCNs of the group, the first one first (TS 36.331 CarrierFreqsGERAN field descriptions).
    pub fn arfcns(&self) -> Vec<u16> {
        let s = self.first;
        let mut v = vec![s];
        match &self.following {
            GeranFollowing::Explicit(x) => v.extend(x),
            GeranFollowing::Equal(d, n) => v.extend((1..=u16::from(*n)).map(|i| (s + i * u16::from(*d)) % 1024)),
            GeranFollowing::Bitmap(m) => {
                for (i, byte) in m.iter().enumerate() {
                    for b in 0..8u16 {
                        if byte >> (7 - b) & 1 == 1 {
                            v.push((s + 8 * i as u16 + b + 1) % 1024);
                        }
                    }
                }
            }
        }
        v
    }
    /// q-RxLevMin, dBm.
    pub fn q_rx_lev_min_dbm(&self) -> i32 {
        2 * i32::from(self.q_rx_lev_min) - 115
    }
}

/// SystemInformationBlockType7: the GERAN carriers for cell reselection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib7 {
    /// t-ReselectionGERAN, s.
    pub t_reselection: u8,
    /// t-ReselectionGERAN-SF.
    pub speed: Option<(u8, u8)>,
    /// carrierFreqsInfoList.
    pub groups: Vec<GeranGroup>,
}

impl Sib7 {
    /// Reads or writes the message.
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let hs = opt(p, self.speed.is_some());
        let hg = opt(p, !self.groups.is_empty());
        int(p, &mut self.t_reselection, 0, 7);
        speed(p, hs, &mut self.speed);
        if hg {
            list(p, &mut self.groups, 1, 16, |p, g| g.codec(p));
        } else {
            self.groups.clear();
        }
        ext_end(p, e);
    }
}

/// A band class of CDMA2000 with its reselection thresholds (BandClassInfoCDMA2000).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CdmaBand {
    /// bandClass (bc0 is 0).
    pub band_class: u8,
    /// cellReselectionPriority.
    pub priority: Option<u8>,
    /// threshX-High (raw).
    pub thresh_high: u8,
    /// threshX-Low (raw).
    pub thresh_low: u8,
}

/// The cells of one CDMA2000 carrier (NeighCellsPerBandclassCDMA2000).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CdmaCarrier {
    /// The band class.
    pub band_class: u8,
    /// arfcn.
    pub arfcn: u16,
    /// physCellIdList: the PN offsets.
    pub pn: Vec<u16>,
}

/// CellReselectionParametersCDMA2000 of HRPD or 1xRTT.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CdmaReselection {
    /// bandClassList.
    pub bands: Vec<CdmaBand>,
    /// neighCellList, one entry per carrier.
    pub carriers: Vec<CdmaCarrier>,
    /// t-ReselectionCDMA2000, s.
    pub t_reselection: u8,
    /// t-ReselectionCDMA2000-SF.
    pub speed: Option<(u8, u8)>,
}

fn band_class<P: PerIo>(p: &mut P, v: &mut u8) {
    let mut i = usize::from(*v);
    p.enumerated(&mut i, 32, true);
    *v = i.min(255) as u8;
}

impl CdmaReselection {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let hs = opt(p, self.speed.is_some());
        list(p, &mut self.bands, 1, 32, |p, b| {
            let e = ext_bit(p);
            let hp = opt(p, b.priority.is_some());
            band_class(p, &mut b.band_class);
            priority(p, hp, &mut b.priority);
            int(p, &mut b.thresh_high, 0, 63);
            int(p, &mut b.thresh_low, 0, 63);
            ext_end(p, e);
        });
        // NeighCellListCDMA2000: per band class a list of carriers; kept here as one carrier per entry.
        let mut by_band: Vec<(u8, Vec<CdmaCarrier>)> = Vec::new();
        for c in &self.carriers {
            match by_band.iter_mut().find(|x| x.0 == c.band_class) {
                Some(x) => x.1.push(c.clone()),
                None => by_band.push((c.band_class, vec![c.clone()])),
            }
        }
        list(p, &mut by_band, 1, 16, |p, (bc, cs)| {
            band_class(p, bc);
            list(p, cs, 1, 16, |p, c| {
                int(p, &mut c.arfcn, 0, 2047);
                list(p, &mut c.pn, 1, 16, |p, n| int(p, n, 0, 511));
            });
            for c in cs.iter_mut() {
                c.band_class = *bc;
            }
        });
        self.carriers = by_band.into_iter().flat_map(|x| x.1).collect();
        int(p, &mut self.t_reselection, 0, 7);
        speed(p, hs, &mut self.speed);
    }
}

/// SystemInformationBlockType8: the CDMA2000 system time, pre-registration and the carriers and cells of HRPD and 1xRTT.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib8 {
    /// systemTimeInfo: cdma-EUTRA-Synchronisation, and the time (39 bits synchronous, else 49 bits).
    pub system_time: Option<(bool, bool, u64)>,
    /// searchWindowSize.
    pub search_window: Option<u8>,
    /// parametersHRPD: preRegistrationAllowed, preRegistrationZoneId, the secondary zones.
    pub hrpd_pre_registration: Option<(bool, Option<u8>, Vec<u8>)>,
    /// cellReselectionParametersHRPD.
    pub hrpd: Option<CdmaReselection>,
    /// parameters1XRTT is present.
    pub one_x_present: bool,
    /// csfb-RegistrationParam1XRTT (63 bits).
    pub one_x_registration: Option<u64>,
    /// longCodeState1XRTT (42 bits).
    pub one_x_long_code: Option<u64>,
    /// cellReselectionParameters1XRTT.
    pub one_x: Option<CdmaReselection>,
}

impl Sib8 {
    /// Reads or writes the message.
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let ht = opt(p, self.system_time.is_some());
        let hw = opt(p, self.search_window.is_some());
        let hh = opt(p, self.hrpd_pre_registration.is_some());
        let h1 = opt(p, self.one_x_present);
        if ht {
            let (mut sync, mut asynchronous, mut t) = self.system_time.unwrap_or((false, false, 0));
            p.flag(&mut sync);
            let mut idx = usize::from(asynchronous);
            p.choice(&mut idx, 2, false);
            asynchronous = idx == 1;
            p.bits(&mut t, if asynchronous { 49 } else { 39 });
            self.system_time = Some((sync, asynchronous, t));
        } else {
            self.system_time = None;
        }
        if hw {
            let mut w = self.search_window.unwrap_or(0);
            int(p, &mut w, 0, 15);
            self.search_window = Some(w);
        } else {
            self.search_window = None;
        }
        if hh {
            let hr = opt(p, self.hrpd.is_some());
            let (mut allowed, mut zone, mut secondary) = self.hrpd_pre_registration.clone().unwrap_or((false, None, Vec::new()));
            let hz = opt(p, zone.is_some());
            let hs = opt(p, !secondary.is_empty());
            p.flag(&mut allowed);
            if hz {
                let mut z = zone.unwrap_or(0);
                int(p, &mut z, 0, 255);
                zone = Some(z);
            } else {
                zone = None;
            }
            if hs {
                list(p, &mut secondary, 1, 2, |p, z| int(p, z, 0, 255));
            } else {
                secondary.clear();
            }
            self.hrpd_pre_registration = Some((allowed, zone, secondary));
            if hr {
                let mut r = self.hrpd.clone().unwrap_or_default();
                r.codec(p);
                self.hrpd = Some(r);
            } else {
                self.hrpd = None;
            }
        } else {
            self.hrpd_pre_registration = None;
            self.hrpd = None;
        }
        self.one_x_present = h1;
        if h1 {
            let hr = opt(p, self.one_x_registration.is_some());
            let hl = opt(p, self.one_x_long_code.is_some());
            let hc = opt(p, self.one_x.is_some());
            for (present, v, n) in [(hr, &mut self.one_x_registration, 63), (hl, &mut self.one_x_long_code, 42)] {
                if present {
                    let mut x = v.unwrap_or(0);
                    p.bits(&mut x, n);
                    *v = Some(x);
                } else {
                    *v = None;
                }
            }
            if hc {
                let mut r = self.one_x.clone().unwrap_or_default();
                r.codec(p);
                self.one_x = Some(r);
            } else {
                self.one_x = None;
            }
        } else {
            self.one_x_registration = None;
            self.one_x_long_code = None;
            self.one_x = None;
        }
        ext_end(p, e);
    }
}

/// A carrier of NR (CarrierFreqNR-r15).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NrCarrier {
    /// carrierFreq-r15: the NR-ARFCN.
    pub nr_arfcn: u32,
    /// multiBandInfoList-r15: the bands.
    pub bands: Vec<u16>,
    /// multiBandInfoListSUL-r15.
    pub bands_sul: Vec<u16>,
    /// measTimingConfig-r15: the period choice (sf5 .. sf160), the offset, the duration index.
    pub mtc: Option<(u8, u8, u8)>,
    /// subcarrierSpacingSSB-r15, kHz.
    pub ssb_scs_khz: u16,
    /// ss-RSSI-Measurement-r15: the slots (one bit per byte) and endSymbol.
    pub rssi: Option<(Vec<u8>, u8)>,
    /// cellReselectionPriority-r15.
    pub priority: Option<u8>,
    /// cellReselectionSubPriority-r13 index (0.2 x (index + 1)).
    pub sub_priority: Option<u8>,
    /// threshX-High-r15 (raw, x2 dB).
    pub thresh_high: u8,
    /// threshX-Low-r15 (raw, x2 dB).
    pub thresh_low: u8,
    /// threshX-Q-r15.
    pub thresh_q: Option<(u8, u8)>,
    /// q-RxLevMin-r15 (raw, x2 dBm).
    pub q_rx_lev_min: i8,
    /// q-RxLevMinSUL-r15 (raw).
    pub q_rx_lev_min_sul: Option<i8>,
    /// p-MaxNR-r15, dBm.
    pub p_max: i8,
    /// ns-PmaxListNR-r15: additionalPmaxNR and additionalSpectrumEmissionNR.
    pub ns_pmax: Vec<(Option<i8>, u8)>,
    /// q-QualMin-r15, dB.
    pub q_qual_min: Option<i8>,
    /// deriveSSB-IndexFromCell-r15.
    pub derive_ssb_index: bool,
    /// maxRS-IndexCellQual-r15.
    pub max_rs_index: Option<u8>,
    /// threshRS-Index-r15: RSRP, RSRQ and SINR thresholds.
    pub thresh_rs: Option<(Option<u8>, Option<u8>, Option<u8>)>,
}

const SSB_SCS: [i64; 4] = [15, 30, 120, 240];

fn opt_int<P: PerIo, T: Copy + Default + TryFrom<i64> + Into<i64>>(p: &mut P, present: bool, v: &mut Option<T>, lo: i64, hi: i64) {
    if present {
        let mut x = v.unwrap_or_else(|| T::try_from(lo).unwrap_or_default());
        int(p, &mut x, lo, hi);
        *v = Some(x);
    } else {
        *v = None;
    }
}

impl NrCarrier {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let present = [
            !self.bands.is_empty(),
            !self.bands_sul.is_empty(),
            self.mtc.is_some(),
            self.rssi.is_some(),
            self.priority.is_some(),
            self.sub_priority.is_some(),
            self.thresh_q.is_some(),
            self.q_rx_lev_min_sul.is_some(),
            !self.ns_pmax.is_empty(),
            self.q_qual_min.is_some(),
            self.max_rs_index.is_some(),
            self.thresh_rs.is_some(),
        ];
        let mut h = present;
        p.optionals(&mut h);
        int(p, &mut self.nr_arfcn, 0, 3_279_165);
        for (on, v) in [(h[0], &mut self.bands), (h[1], &mut self.bands_sul)] {
            if on {
                list(p, v, 1, 32, |p, b| int(p, b, 1, 1024));
            } else {
                v.clear();
            }
        }
        if h[2] {
            let (mut period, mut offset, mut duration) = self.mtc.unwrap_or((0, 0, 0));
            let mut idx = usize::from(period);
            p.choice(&mut idx, 6, false);
            period = idx as u8;
            let last = [4i64, 9, 19, 39, 79, 159][idx.min(5)];
            int(p, &mut offset, 0, last);
            int(p, &mut duration, 0, 4);
            self.mtc = Some((period, offset, duration));
        } else {
            self.mtc = None;
        }
        let mut scs = i64::from(self.ssb_scs_khz);
        p.enum_value(&mut scs, &SSB_SCS);
        self.ssb_scs_khz = scs as u16;
        if h[3] {
            let (mut slots, mut end) = self.rssi.clone().unwrap_or((vec![0], 0));
            bit_list(p, &mut slots, 1, 80);
            int(p, &mut end, 0, 3);
            self.rssi = Some((slots, end));
        } else {
            self.rssi = None;
        }
        priority(p, h[4], &mut self.priority);
        opt_int(p, h[5], &mut self.sub_priority, 0, 3);
        int(p, &mut self.thresh_high, 0, 31);
        int(p, &mut self.thresh_low, 0, 31);
        if h[6] {
            let (mut a, mut b) = self.thresh_q.unwrap_or((0, 0));
            int(p, &mut a, 0, 31);
            int(p, &mut b, 0, 31);
            self.thresh_q = Some((a, b));
        } else {
            self.thresh_q = None;
        }
        int(p, &mut self.q_rx_lev_min, -70, -22);
        opt_int(p, h[7], &mut self.q_rx_lev_min_sul, -70, -22);
        int(p, &mut self.p_max, -30, 33);
        if h[8] {
            list(p, &mut self.ns_pmax, 1, 8, |p, (pmax, emission)| {
                let hp = opt(p, pmax.is_some());
                opt_int(p, hp, pmax, -30, 33);
                int(p, emission, 0, 7);
            });
        } else {
            self.ns_pmax.clear();
        }
        opt_int(p, h[9], &mut self.q_qual_min, -43, -12);
        p.flag(&mut self.derive_ssb_index);
        opt_int(p, h[10], &mut self.max_rs_index, 1, 16);
        if h[11] {
            let (mut a, mut b, mut c) = self.thresh_rs.unwrap_or((None, None, None));
            let (ha, hb, hc) = (opt(p, a.is_some()), opt(p, b.is_some()), opt(p, c.is_some()));
            opt_int(p, ha, &mut a, 0, 127);
            opt_int(p, hb, &mut b, 0, 127);
            opt_int(p, hc, &mut c, 0, 127);
            self.thresh_rs = Some((a, b, c));
        } else {
            self.thresh_rs = None;
        }
        ext_end(p, e);
    }
    /// q-RxLevMin, dBm.
    pub fn q_rx_lev_min_dbm(&self) -> i32 {
        2 * i32::from(self.q_rx_lev_min)
    }
}

/// SystemInformationBlockType24-r15: the NR carriers for cell reselection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib24 {
    /// carrierFreqListNR-r15.
    pub carriers: Vec<NrCarrier>,
    /// t-ReselectionNR-r15, s.
    pub t_reselection: u8,
    /// t-ReselectionNR-SF-r15.
    pub speed: Option<(u8, u8)>,
    /// lateNonCriticalExtension, not read.
    pub late: Option<Vec<u8>>,
}

impl Sib24 {
    /// Reads or writes the message.
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        let e = ext_bit(p);
        let hc = opt(p, !self.carriers.is_empty());
        let hs = opt(p, self.speed.is_some());
        let hl = opt(p, self.late.is_some());
        if hc {
            list(p, &mut self.carriers, 1, 8, |p, c| c.codec(p));
        } else {
            self.carriers.clear();
        }
        int(p, &mut self.t_reselection, 0, 7);
        speed(p, hs, &mut self.speed);
        if hl {
            let mut v = self.late.clone().unwrap_or_default();
            p.octets_unconstrained(&mut v);
            self.late = Some(v);
        } else {
            self.late = None;
        }
        ext_end(p, e);
    }
}

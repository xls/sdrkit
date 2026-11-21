//! The NB-IoT broadcast messages (3GPP TS 36.331): MasterInformationBlock-NB and SystemInformationBlockType1-NB, each described
//! once over [`PerIo`] for both reading and writing.

use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};
use sdrkit_util_bits::{BitOrder, Pad};
use serde::{Deserialize, Serialize};

/// The operation mode of an NB-IoT carrier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationMode {
    /// In-band, same PCI as the LTE cell (eutra-CRS-SequenceInfo).
    InbandSamePci(u8),
    /// In-band, another PCI: four CRS ports when true; the raster offset index.
    InbandDifferentPci {
        /// eutra-NumCRS-Ports is "four".
        four_ports: bool,
        /// rasterOffset index 0..3.
        raster: u8,
    },
    /// Guard band, raster offset index.
    Guardband(u8),
    /// Standalone.
    #[default]
    Standalone,
}

impl OperationMode {
    /// The name shown.
    pub fn name(&self) -> &'static str {
        match self {
            OperationMode::InbandSamePci(_) => "in-band (same PCI)",
            OperationMode::InbandDifferentPci { .. } => "in-band (other PCI)",
            OperationMode::Guardband(_) => "guard band",
            OperationMode::Standalone => "standalone",
        }
    }
    /// The raster offset (kHz) the MIB states, if any.
    pub fn raster_offset_khz(&self) -> Option<f64> {
        let k = |i: u8| [-7.5, -2.5, 2.5, 7.5][i as usize & 3];
        match self {
            OperationMode::InbandDifferentPci { raster, .. } => Some(k(*raster)),
            OperationMode::Guardband(r) => Some(k(*r)),
            _ => None,
        }
    }
    /// The raster offset index of kHz.
    pub fn raster_index(khz: f64) -> u8 {
        [-7.5, -2.5, 2.5, 7.5]
            .iter()
            .position(|&v| (v - khz).abs() < 0.1)
            .unwrap_or(2) as u8
    }
}

/// MasterInformationBlock-NB.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MibNb {
    /// The 4 MSBs of the SFN.
    pub sfn_msb: u8,
    /// The 2 LSBs of the hyper SFN.
    pub hsfn_lsb: u8,
    /// schedulingInfoSIB1.
    pub scheduling_info_sib1: u8,
    /// systemInfoValueTag.
    pub value_tag: u8,
    /// ab-Enabled.
    pub access_barring: bool,
    /// operationModeInfo.
    pub mode: OperationMode,
    /// additionalTransmissionSIB1.
    pub additional_sib1: bool,
}

fn bits<P: PerIo, T: Copy + Into<u64> + TryFrom<u64>>(p: &mut P, v: &mut T, n: u32) {
    let mut x: u64 = (*v).into();
    p.bits(&mut x, n);
    if let Ok(t) = T::try_from(x) {
        *v = t;
    }
}

/// One decimal digit (INTEGER (0..9)).
fn digit<P: PerIo>(p: &mut P, d: &mut u8) {
    let mut x = *d as i64;
    p.int(&mut x, 0, 9);
    *d = x as u8;
}

impl MibNb {
    /// The description, for reading and writing (34 bits).
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        bits(p, &mut self.sfn_msb, 4);
        bits(p, &mut self.hsfn_lsb, 2);
        bits(p, &mut self.scheduling_info_sib1, 4);
        bits(p, &mut self.value_tag, 5);
        p.flag(&mut self.access_barring);
        let mut choice: usize = match self.mode {
            OperationMode::InbandSamePci(_) => 0,
            OperationMode::InbandDifferentPci { .. } => 1,
            OperationMode::Guardband(_) => 2,
            OperationMode::Standalone => 3,
        };
        p.enumerated(&mut choice, 4, false);
        let mut spare = 0u64;
        match choice {
            0 => {
                let mut s = match self.mode {
                    OperationMode::InbandSamePci(s) => s,
                    _ => 0,
                };
                bits(p, &mut s, 5);
                self.mode = OperationMode::InbandSamePci(s);
            }
            1 => {
                let (mut f, mut r) = match self.mode {
                    OperationMode::InbandDifferentPci { four_ports, raster } => {
                        (four_ports, raster)
                    }
                    _ => (false, 0),
                };
                p.flag(&mut f);
                bits(p, &mut r, 2);
                p.bits(&mut spare, 2);
                self.mode = OperationMode::InbandDifferentPci {
                    four_ports: f,
                    raster: r,
                };
            }
            2 => {
                let mut r = match self.mode {
                    OperationMode::Guardband(r) => r,
                    _ => 0,
                };
                bits(p, &mut r, 2);
                p.bits(&mut spare, 3);
                self.mode = OperationMode::Guardband(r);
            }
            _ => {
                p.bits(&mut spare, 5);
                self.mode = OperationMode::Standalone;
            }
        }
        p.flag(&mut self.additional_sib1);
        let mut sp = 0u64;
        p.bits(&mut sp, 10);
    }
    /// From 34 bits.
    pub fn from_bits(b: &[u8; 34]) -> MibNb {
        let mut bytes = Vec::new();
        pack(b, BitOrder::Msb, Pad::Zero, &mut bytes);
        let mut r = PerReader::new(&bytes);
        let mut m = MibNb::default();
        m.codec(&mut r);
        m
    }
    /// The 34 bits.
    pub fn to_bits(&self) -> [u8; 34] {
        let mut bytes = Vec::new();
        let mut w = PerWriter::new(&mut bytes);
        self.clone().codec(&mut w);
        w.finish();
        let mut v = Vec::new();
        unpack(&bytes, BitOrder::Msb, &mut v);
        let mut out = [0u8; 34];
        out.copy_from_slice(&v[..34]);
        out
    }
}

/// A PLMN identity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Plmn {
    /// MCC digits (none: the previous entry's).
    pub mcc: Option<[u8; 3]>,
    /// MNC digits (2 or 3).
    pub mnc: Vec<u8>,
}

impl Plmn {
    /// A PLMN from numbers.
    pub fn new(mcc: u16, mnc: u16, digits: usize) -> Plmn {
        let d = |v: u16, i: u32| ((v / 10u16.pow(i)) % 10) as u8;
        Plmn {
            mcc: Some([d(mcc, 2), d(mcc, 1), d(mcc, 0)]),
            mnc: (0..digits).rev().map(|i| d(mnc, i as u32)).collect(),
        }
    }
    /// The description (36.331 PLMN-Identity).
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        let mut has = self.mcc.is_some();
        p.flag(&mut has);
        if has {
            let mut m = self.mcc.unwrap_or([0; 3]);
            for d in m.iter_mut() {
                digit(p, d);
            }
            self.mcc = Some(m);
        } else {
            self.mcc = None;
        }
        let mut n = self.mnc.len().max(2);
        p.count(&mut n, 2, 3);
        self.mnc.resize(n, 0);
        for d in self.mnc.iter_mut() {
            digit(p, d);
        }
    }
    /// MCC, MNC and MNC digits (`prev_mcc` when this entry has none).
    pub fn numbers(&self, prev_mcc: Option<[u8; 3]>) -> (u16, u16, u8) {
        let m = self.mcc.or(prev_mcc).unwrap_or([0; 3]);
        let mcc = m.iter().fold(0u16, |a, &d| a * 10 + d as u16);
        let mnc = self.mnc.iter().fold(0u16, |a, &d| a * 10 + d as u16);
        (mcc, mnc, self.mnc.len() as u8)
    }
}

/// The SIB1-NB fields this receiver reads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib1Nb {
    /// hyperSFN-MSB.
    pub hsfn_msb: u8,
    /// PLMNs with the reserved flag.
    pub plmns: Vec<(Plmn, bool)>,
    /// TAC.
    pub tac: u16,
    /// 28-bit cell identity.
    pub cell_id: u32,
    /// cellBarred.
    pub barred: bool,
    /// intraFreqReselection allowed.
    pub intra_freq_reselection: bool,
    /// q-RxLevMin, dBm (x2 applied).
    pub q_rx_lev_min_dbm: i32,
    /// q-QualMin, dB.
    pub q_qual_min_db: i32,
    /// p-Max, dBm.
    pub p_max_dbm: Option<i32>,
    /// freqBandIndicator.
    pub band: u16,
}

impl Sib1Nb {
    /// The description over a BCCH-DL-SCH-Message-NB carrying SIB1-NB. The writer writes the read fields and a minimal rest (one
    /// scheduling entry, the SI window); the reader stops after the band indicator.
    pub fn codec<P: PerIo>(&mut self, p: &mut P) {
        let mut ext = 0u64;
        p.bits(&mut ext, 1);
        let mut c = 1u64;
        p.bits(&mut c, 1);
        if p.reading() && (ext != 0 || c != 1) {
            p.fail();
            return;
        }
        // Presence of the ten OPTIONAL fields.
        let mut pmax = self.p_max_dbm.is_some();
        p.flag(&mut pmax);
        let mut others = 0u64;
        p.bits(&mut others, 9);
        bits(p, &mut self.hsfn_msb, 8);
        let mut n = self.plmns.len().max(1);
        p.count(&mut n, 1, 6);
        self.plmns.resize(n, (Plmn::default(), false));
        for (pl, res) in self.plmns.iter_mut() {
            let mut attach = false;
            p.flag(&mut attach);
            pl.codec(p);
            let mut idx = if *res { 0 } else { 1 };
            p.enumerated(&mut idx, 2, false);
            *res = idx == 0;
        }
        bits(p, &mut self.tac, 16);
        bits(p, &mut self.cell_id, 28);
        let mut b = if self.barred { 0 } else { 1 };
        p.enumerated(&mut b, 2, false);
        self.barred = b == 0;
        let mut ifr = if self.intra_freq_reselection { 0 } else { 1 };
        p.enumerated(&mut ifr, 2, false);
        self.intra_freq_reselection = ifr == 0;
        let mut q = (self.q_rx_lev_min_dbm / 2) as i64;
        p.int(&mut q, -70, -22);
        self.q_rx_lev_min_dbm = q as i32 * 2;
        let mut qq = self.q_qual_min_db as i64;
        p.int(&mut qq, -34, -3);
        self.q_qual_min_db = qq as i32;
        if pmax {
            let mut v = self.p_max_dbm.unwrap_or(0) as i64;
            p.int(&mut v, -30, 33);
            self.p_max_dbm = Some(v as i32);
        } else {
            self.p_max_dbm = None;
        }
        let mut band = self.band as i64;
        p.int(&mut band, 1, 256);
        self.band = band as u16;
        if p.reading() {
            return;
        }
        // schedulingInfoList: one entry (rf512, every2ndRF, no SIBs, b120), si-WindowLength ms160.
        let mut one = 1usize;
        p.count(&mut one, 1, 8);
        let mut v = 3u64;
        p.bits(&mut v, 3);
        let mut v = 0u64;
        p.bits(&mut v, 2);
        let mut nsib = 0usize;
        p.count(&mut nsib, 0, 31);
        let mut v = 1u64;
        p.bits(&mut v, 3);
        let mut v = 0u64;
        p.bits(&mut v, 3);
    }
    /// Parses a transport block.
    pub fn parse(tb: &[u8]) -> Option<Sib1Nb> {
        let mut bytes = Vec::new();
        pack(tb, BitOrder::Msb, Pad::Zero, &mut bytes);
        let mut r = PerReader::new(&bytes);
        let mut s = Sib1Nb::default();
        s.codec(&mut r);
        (!r.failed() && !s.plmns.is_empty()).then_some(s)
    }
    /// Encodes into bits.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut w = PerWriter::new(&mut bytes);
        self.clone().codec(&mut w);
        w.finish();
        let mut v = Vec::new();
        unpack(&bytes, BitOrder::Msb, &mut v);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mib_nb_and_sib1_nb() {
        for mode in [
            OperationMode::Standalone,
            OperationMode::Guardband(0),
            OperationMode::InbandSamePci(9),
            OperationMode::InbandDifferentPci {
                four_ports: true,
                raster: 3,
            },
        ] {
            let m = MibNb {
                sfn_msb: 9,
                hsfn_lsb: 1,
                scheduling_info_sib1: 4,
                value_tag: 5,
                access_barring: false,
                mode,
                additional_sib1: false,
            };
            assert_eq!(MibNb::from_bits(&m.to_bits()), m);
        }
        let s = Sib1Nb {
            hsfn_msb: 3,
            plmns: vec![(Plmn::new(1, 1, 2), false), (Plmn::new(310, 410, 3), true)],
            tac: 0x1A2B,
            cell_id: 0x0123456,
            barred: false,
            intra_freq_reselection: true,
            q_rx_lev_min_dbm: -128,
            q_qual_min_db: -20,
            p_max_dbm: Some(23),
            band: 8,
        };
        let b = s.encode();
        assert_eq!(Sib1Nb::parse(&b), Some(s));
        assert_eq!(Sib1Nb::parse(&b[..20]), None);
    }
}

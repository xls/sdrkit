//! The `constellation` setting of the slicer, the carrier loop, the equaliser and the SNR meter.

use sdrkit_algo_dsp::constellation::{by_name, PointSet};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, ConfigError, Setting};

sdrkit_core::choice_enum! {
    /// A constellation, by the name that `sdrkit_algo_dsp::constellation::by_name` takes.
    pub enum ConstellationName {
        /// BPSK.
        Bpsk = ("bpsk", "BPSK"),
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// 8PSK.
        Psk8 = ("psk8", "8PSK"),
        /// DVB-S2 8PSK.
        Dvbs2Psk8 = ("dvbs2_8psk", "DVB-S2 8PSK"),
        /// 2-PAM.
        Pam2 = ("pam2", "2-PAM"),
        /// 4-PAM.
        Pam4 = ("pam4", "4-PAM"),
        /// 8-PAM.
        Pam8 = ("pam8", "8-PAM"),
        /// 16-QAM, IEEE 802.11 bit map.
        Qam16Ieee = ("qam16_ieee", "16-QAM (IEEE)"),
        /// 64-QAM, IEEE 802.11 bit map.
        Qam64Ieee = ("qam64_ieee", "64-QAM (IEEE)"),
        /// 256-QAM, IEEE 802.11 bit map.
        Qam256Ieee = ("qam256_ieee", "256-QAM (IEEE)"),
        /// 1024-QAM, IEEE 802.11 bit map.
        Qam1024Ieee = ("qam1024_ieee", "1024-QAM (IEEE)"),
        /// 16-QAM, DVB bit map.
        Qam16Dvb = ("qam16_dvb", "16-QAM (DVB)"),
        /// 64-QAM, DVB bit map.
        Qam64Dvb = ("qam64_dvb", "64-QAM (DVB)"),
        /// 256-QAM, DVB bit map.
        Qam256Dvb = ("qam256_dvb", "256-QAM (DVB)"),
        /// 1024-QAM, DVB bit map.
        Qam1024Dvb = ("qam1024_dvb", "1024-QAM (DVB)"),
        /// 16-QAM, 3GPP bit map.
        Qam16Gpp = ("qam16_3gpp", "16-QAM (3GPP)"),
        /// 64-QAM, 3GPP bit map.
        Qam64Gpp = ("qam64_3gpp", "64-QAM (3GPP)"),
        /// 256-QAM, 3GPP bit map.
        Qam256Gpp = ("qam256_3gpp", "256-QAM (3GPP)"),
        /// 1024-QAM, 3GPP bit map.
        Qam1024Gpp = ("qam1024_3gpp", "1024-QAM (3GPP)"),
        /// DVB-T hierarchical 16-QAM, alpha 2.
        DvbtQam16A2 = ("dvbt_qam16_a2", "DVB-T 16-QAM, alpha 2"),
        /// DVB-T hierarchical 16-QAM, alpha 4.
        DvbtQam16A4 = ("dvbt_qam16_a4", "DVB-T 16-QAM, alpha 4"),
        /// DVB-T hierarchical 64-QAM, alpha 2.
        DvbtQam64A2 = ("dvbt_qam64_a2", "DVB-T 64-QAM, alpha 2"),
        /// DVB-T hierarchical 64-QAM, alpha 4.
        DvbtQam64A4 = ("dvbt_qam64_a4", "DVB-T 64-QAM, alpha 4"),
        /// DVB-C 16-QAM.
        DvbcQam16 = ("dvbc_qam16", "DVB-C 16-QAM"),
        /// DVB-C 32-QAM.
        DvbcQam32 = ("dvbc_qam32", "DVB-C 32-QAM"),
        /// DVB-C 64-QAM.
        DvbcQam64 = ("dvbc_qam64", "DVB-C 64-QAM"),
        /// DVB-C 128-QAM.
        DvbcQam128 = ("dvbc_qam128", "DVB-C 128-QAM"),
        /// DVB-C 256-QAM.
        DvbcQam256 = ("dvbc_qam256", "DVB-C 256-QAM"),
        /// DVB-S2 16APSK, rate 2/3.
        Dvbs2Apsk16R2_3 = ("dvbs2_16apsk_2_3", "DVB-S2 16APSK 2/3"),
        /// DVB-S2 16APSK, rate 3/4.
        Dvbs2Apsk16R3_4 = ("dvbs2_16apsk_3_4", "DVB-S2 16APSK 3/4"),
        /// DVB-S2 16APSK, rate 4/5.
        Dvbs2Apsk16R4_5 = ("dvbs2_16apsk_4_5", "DVB-S2 16APSK 4/5"),
        /// DVB-S2 16APSK, rate 5/6.
        Dvbs2Apsk16R5_6 = ("dvbs2_16apsk_5_6", "DVB-S2 16APSK 5/6"),
        /// DVB-S2 16APSK, rate 8/9.
        Dvbs2Apsk16R8_9 = ("dvbs2_16apsk_8_9", "DVB-S2 16APSK 8/9"),
        /// DVB-S2 16APSK, rate 9/10.
        Dvbs2Apsk16R9_10 = ("dvbs2_16apsk_9_10", "DVB-S2 16APSK 9/10"),
        /// DVB-S2 32APSK, rate 3/4.
        Dvbs2Apsk32R3_4 = ("dvbs2_32apsk_3_4", "DVB-S2 32APSK 3/4"),
        /// DVB-S2 32APSK, rate 4/5.
        Dvbs2Apsk32R4_5 = ("dvbs2_32apsk_4_5", "DVB-S2 32APSK 4/5"),
        /// DVB-S2 32APSK, rate 5/6.
        Dvbs2Apsk32R5_6 = ("dvbs2_32apsk_5_6", "DVB-S2 32APSK 5/6"),
        /// DVB-S2 32APSK, rate 8/9.
        Dvbs2Apsk32R8_9 = ("dvbs2_32apsk_8_9", "DVB-S2 32APSK 8/9"),
        /// DVB-S2 32APSK, rate 9/10.
        Dvbs2Apsk32R9_10 = ("dvbs2_32apsk_9_10", "DVB-S2 32APSK 9/10"),
        /// 8-VSB.
        Vsb8 = ("vsb8", "8-VSB"),
    }
}

impl ConstellationName {
    /// The name that `sdrkit_algo_dsp::constellation::by_name` takes.
    pub fn key(self) -> &'static str {
        &Self::CHOICES[usize::from(self.index())].key
    }

    /// The point set.
    pub fn point_set(self) -> PointSet {
        by_name(self.key()).expect("every constellation name has a point set")
    }
}

/// The `constellation` setting, with QPSK as its default.
pub(crate) const CONSTELLATION: Setting<field::Choice<ConstellationName>> =
    Setting::choice("constellation", "Constellation", ConstellationName::Qpsk)
        .help("The point set the symbols are decided on.");

/// The modem constellation `c`.
pub(crate) fn constellation(
    c: ConstellationName,
) -> Result<sdrkit_algo_modem::Constellation, ConfigError> {
    Ok(sdrkit_algo_modem::Constellation::new(c.point_set())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::constellation::NAMES;

    #[test]
    fn the_choices_are_the_point_set_names() {
        let keys: Vec<&str> = ConstellationName::CHOICES.iter().map(|c| &*c.key).collect();
        assert_eq!(keys, NAMES);
        for i in 0..NAMES.len() {
            let c = ConstellationName::from_index(i as u16).unwrap();
            assert!(constellation(c).is_ok(), "{}", c.key());
        }
    }
}

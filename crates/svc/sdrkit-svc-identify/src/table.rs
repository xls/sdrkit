//! The signal types that the identifier recognises, from published parameters. Each signature names a signal type by its key. A
//! stack declares which signal types it opens, and the engine supplies that mapping to the service.

use crate::signature::{FrameSync, Keying, ModClass, Signature};

const VOICE: &str = "Digital voice";
const BROADCAST: &str = "Broadcast";
const LISTEN: &str = "Listening";
const AVIATION: &str = "Aviation";
const TV: &str = "TV";
const DATA: &str = "Data";

/// DVB-T useful symbol lengths for a channel width (MHz) and FFT size: N × 7 / (8 × W) µs.
pub fn dvbt_tu_s(width_mhz: f64, n: usize) -> f64 {
    n as f64 * 7.0 / (8.0 * width_mhz) * 1e-6
}

/// The channel width (MHz) and FFT size that best explain a DVB-T/T2 useful symbol length, with the relative error.
pub fn dvbt_width_for(tu_s: f64) -> (f64, usize, f64) {
    let mut best = (8.0, 2048, f64::MAX);
    for w in [8.0, 7.0, 6.0, 5.0] {
        for n in [1024, 2048, 4096, 8192, 16384, 32768] {
            let e = (dvbt_tu_s(w, n) / tu_s - 1.0).abs();
            if e < best.2 {
                best = (w, n, e);
            }
        }
    }
    best
}

/// The four DMR sync patterns (base station and mobile, voice and data).
pub fn dmr_syncs() -> Vec<FrameSync> {
    vec![
        FrameSync::fsk4("BS voice sync", 0x755F_D7DF_75F7, 24),
        FrameSync::fsk4("BS data sync", 0xDFF5_7D75_DF5D, 24),
        FrameSync::fsk4("MS voice sync", 0x7F7D_5DD5_7DFD, 24),
        FrameSync::fsk4("MS data sync", 0xD5D7_F77F_D757, 24),
    ]
}

/// Every signature written by hand, from published parameters.
pub fn builtin() -> Vec<Signature> {
    let mut v = Vec::new();
    let tu = |w: f64, ns: &[usize]| ns.iter().map(|n| dvbt_tu_s(w, *n)).collect::<Vec<f64>>();

    // Broadcast and listening (the FM and general receivers).
    v.push(
        Signature::new("FM broadcast", "fm_broadcast", BROADCAST, ModClass::Fm)
            .width(80e3, 260e3)
            .pilot()
            .continuous()
            .band(65e6, 108e6)
            .usual(98e6),
    );
    v.push(Signature::new("Wide FM", "wide_fm", LISTEN, ModClass::Fm).width(50e3, 260e3));
    v.push(
        Signature::new("AM broadcast", "am_broadcast", BROADCAST, ModClass::Am)
            .also(&[ModClass::Carrier])
            .width_up_to(12e3)
            .continuous()
            .band(148.5e3, 30e6),
    );
    v.push(
        Signature::new("AM voice", "am_voice", LISTEN, ModClass::Am)
            .also(&[ModClass::Carrier])
            .width_up_to(10e3)
            .band(30e6, 1000e6),
    );
    v.push(
        Signature::new("Narrow FM voice", "narrow_fm_voice", LISTEN, ModClass::Fm)
            .width_up_to(16e3)
            .band(26e6, 1000e6),
    );
    v.push(
        Signature::new(
            "SSB voice (upper)",
            "ssb_voice_upper",
            LISTEN,
            ModClass::Unknown,
        )
        .also(&[ModClass::Carrier])
        .width_up_to(3.2e3)
        .band(10e6, 1000e6),
    );
    v.push(
        Signature::new(
            "SSB voice (lower)",
            "ssb_voice_lower",
            LISTEN,
            ModClass::Unknown,
        )
        .also(&[ModClass::Carrier])
        .width_up_to(3.2e3)
        .band(0.1e6, 10e6),
    );
    v.push(
        Signature::new("CW (Morse)", "cw_morse", LISTEN, ModClass::Carrier)
            .width_up_to(600.0)
            .band(0.1e6, 450e6),
    );

    // Aviation.
    v.push(
        Signature::new("ADS-B / Mode S", "ads_b_mode_s", AVIATION, ModClass::Pulse)
            .rates(&[1e6])
            .width(0.8e6, 4e6)
            .bursty(Some(120e-6), None)
            .band(1085e6, 1095e6)
            .usual(1090e6),
    );
    v.push(
        Signature::new(
            "Mode A/C replies",
            "mode_a_c_replies",
            AVIATION,
            ModClass::Pulse,
        )
        .width(0.8e6, 4e6)
        .bursty(Some(20.3e-6), None)
        .band(1085e6, 1095e6)
        .usual(1090e6),
    );
    v.push(
        Signature::new(
            "Mode S interrogations",
            "mode_s_interrogations",
            AVIATION,
            ModClass::Pulse,
        )
        .also(&[ModClass::Bpsk])
        .width(0.4e6, 6e6)
        .bursty(None, None)
        .band(1025e6, 1035e6)
        .usual(1030e6),
    );
    v.push(
        Signature::new("UAT", "uat", AVIATION, ModClass::Fsk2)
            .also(&[ModClass::Gmsk, ModClass::Pulse])
            .rates(&[1_041_667.0])
            .fsk(2, 312.5e3)
            .width(0.6e6, 1.6e6)
            .bursty(None, None)
            .band(975e6, 981e6)
            .usual(978e6),
    );

    // TV and digital radio.
    let mut dvbt_tu = tu(8.0, &[2048, 8192]);
    dvbt_tu.extend(tu(7.0, &[2048, 8192]));
    dvbt_tu.extend(tu(6.0, &[2048, 8192]));
    dvbt_tu.extend(tu(5.0, &[2048, 8192]));
    v.push(
        Signature::new("DVB-T", "dvb_t", TV, ModClass::Ofdm)
            .ofdm(&dvbt_tu, &[])
            .width(4.0e6, 7.8e6)
            .continuous()
            .band(170e6, 240e6)
            .band(470e6, 870e6),
    );
    let mut t2 = tu(8.0, &[1024, 4096, 16384, 32768]);
    t2.extend(tu(7.0, &[1024, 4096, 16384, 32768]));
    t2.extend(tu(6.0, &[1024, 4096, 16384, 32768]));
    v.push(
        Signature::new("DVB-T2", "dvb_t2", TV, ModClass::Ofdm)
            .ofdm(&t2, &[])
            .width(4.0e6, 7.9e6)
            .continuous()
            .band(170e6, 240e6)
            .band(470e6, 870e6),
    );
    v.push(
        Signature::new("DAB / DAB+", "dab_dab", BROADCAST, ModClass::Ofdm)
            .ofdm(
                &[1e-3, 250e-6, 125e-6, 500e-6],
                &[246e-6, 62e-6, 31e-6, 123e-6],
            )
            .width(1.3e6, 1.6e6)
            .continuous()
            .band(170e6, 240e6)
            .band(1452e6, 1492e6),
    );
    v.push(
        Signature::new("ISDB-T", "isdb_t", TV, ModClass::Ofdm)
            .ofdm(&[252e-6, 504e-6, 1008e-6], &[])
            .width(5.3e6, 5.8e6)
            .continuous()
            .band(90e6, 870e6),
    );
    v.push(
        Signature::new("DTMB", "dtmb", TV, ModClass::Ofdm)
            .ofdm(&[500e-6], &[])
            .width(7.2e6, 7.7e6)
            .continuous()
            .band(470e6, 870e6),
    );
    v.push(
        Signature::new("ATSC 1.0", "atsc_1_0", TV, ModClass::Unknown)
            .also(&[ModClass::Am])
            .rates(&[10.762_238e6])
            .width(5.2e6, 6.1e6)
            .continuous()
            .band(54e6, 700e6),
    );

    // Digital voice.
    let mut dmr = Signature::new("DMR", "dmr", VOICE, ModClass::Fsk4)
        .rates(&[4800.0])
        .fsk(4, 1944.0)
        .width(6.5e3, 10.5e3)
        .bursty(Some(27.5e-3), Some(60e-3))
        .band(30e6, 1000e6);
    dmr.keying = Keying::Either;
    for s in dmr_syncs() {
        dmr = dmr.sync(s);
    }
    v.push(dmr);
    v.push(
        Signature::new("P25 Phase 1", "p25_phase_1", VOICE, ModClass::Fsk4)
            .rates(&[4800.0])
            .fsk(4, 1800.0)
            .width(6.5e3, 10.5e3)
            .sync(FrameSync::fsk4("frame sync", 0x5575_F5FF_77FF, 24))
            .band(30e6, 1000e6),
    );
    v.push(
        Signature::new("NXDN96", "nxdn96", VOICE, ModClass::Fsk4)
            .rates(&[4800.0])
            .fsk(4, 2400.0)
            .width(6.5e3, 10.5e3)
            .sync(FrameSync::fsk4("frame sync word", 0xCDF59, 10))
            .band(30e6, 1000e6),
    );
    v.push(
        Signature::new("NXDN48", "nxdn48", VOICE, ModClass::Fsk4)
            .rates(&[2400.0])
            .fsk(4, 1050.0)
            .width(3.0e3, 5.5e3)
            .sync(FrameSync::fsk4("frame sync word", 0xCDF59, 10))
            .band(30e6, 1000e6),
    );
    v.push(
        Signature::new("dPMR", "dpmr", VOICE, ModClass::Fsk4)
            .rates(&[2400.0])
            .fsk(4, 1050.0)
            .width(3.0e3, 5.5e3)
            .sync(FrameSync::fsk4("FS1", 0x57FF_5F75_D577, 24))
            .band(30e6, 1000e6),
    );
    v.push(
        Signature::new(
            "Yaesu System Fusion",
            "yaesu_system_fusion",
            VOICE,
            ModClass::Fsk4,
        )
        .rates(&[4800.0])
        .fsk(4, 2700.0)
        .width(7e3, 11e3)
        .sync(FrameSync::fsk4("frame sync", 0xD4_71C9_634D, 20))
        .band(28e6, 1300e6),
    );
    v.push(
        Signature::new("M17", "m17", VOICE, ModClass::Fsk4)
            .rates(&[4800.0])
            .fsk(4, 2400.0)
            .width(7e3, 10.5e3)
            .sync(FrameSync::fsk4("LSF sync", 0x55F7, 8))
            .sync(FrameSync::fsk4("stream sync", 0xFF5D, 8))
            .band(28e6, 1300e6),
    );
    v.push(
        Signature::new("D-STAR", "d_star", VOICE, ModClass::Gmsk)
            .also(&[ModClass::Fsk2])
            .rates(&[4800.0])
            .fsk(2, 1200.0)
            .width(3.5e3, 7e3)
            .band(28e6, 1300e6),
    );
    v.push(
        Signature::new("TETRA", "tetra", VOICE, ModClass::Pi4Dqpsk)
            .also(&[ModClass::Qpsk])
            .rates(&[18_000.0])
            .width(18e3, 25e3)
            .band(136e6, 960e6),
    );
    v.push(
        Signature::new("P25 Phase 2", "p25_phase_2", VOICE, ModClass::Pi4Dqpsk)
            .also(&[ModClass::Qpsk, ModClass::Fsk4])
            .rates(&[6000.0])
            .width(7e3, 11e3)
            .band(30e6, 1000e6),
    );

    // Data.
    v.push(
        Signature::new("POCSAG", "pocsag", DATA, ModClass::Fsk2)
            .rates(&[512.0, 1200.0, 2400.0])
            .fsk(2, 4500.0)
            .width(9e3, 16e3)
            .sync(FrameSync::fsk2("sync codeword", 0x7CD2_15D8, 32))
            .band(30e6, 1000e6),
    );
    v.push(
        Signature::new("AIS", "ais", DATA, ModClass::Gmsk)
            .also(&[ModClass::Fsk2])
            .rates(&[9600.0])
            .fsk(2, 2400.0)
            .width(8e3, 16e3)
            .bursty(Some(26.67e-3), None)
            .sync(FrameSync::nrzi("training and start flag", 0x5555_557E, 32))
            .band(161.9e6, 162.1e6),
    );
    v.push(
        Signature::new("LoRa", "lora", DATA, ModClass::Chirp)
            .chirp(
                &[125e3, 250e3, 500e3, 62.5e3, 31.25e3],
                &[7, 8, 9, 10, 11, 12],
            )
            .width(30e3, 520e3)
            .band(430e6, 440e6)
            .band(863e6, 870e6)
            .band(902e6, 928e6),
    );
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dvbt_widths_from_tu() {
        let (w, n, e) = dvbt_width_for(896e-6);
        assert_eq!((w, n), (8.0, 8192));
        assert!(e < 1e-9);
        let (w, n, _) = dvbt_width_for(dvbt_tu_s(6.0, 2048) * 1.002);
        assert_eq!((w, n), (6.0, 2048));
    }

    #[test]
    fn table_has_the_main_signal_types_and_unique_keys() {
        let t = builtin();
        for k in [
            "fm_broadcast",
            "narrow_fm_voice",
            "ads_b_mode_s",
            "dvb_t",
            "dab_dab",
            "dmr",
            "p25_phase_1",
            "nxdn96",
            "tetra",
            "ais",
            "lora",
        ] {
            assert!(t.iter().any(|s| s.signal == k), "{k}");
        }
        let mut keys: Vec<&str> = t.iter().map(|s| s.signal.as_str()).collect();
        keys.sort();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n);
        // DMR's BS voice sync starts +3 -3 (0x7 = 01 11).
        assert_eq!(&dmr_syncs()[0].symbols[..2], &[3, -3]);
    }
}

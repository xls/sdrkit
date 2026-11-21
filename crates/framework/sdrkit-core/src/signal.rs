//! The signal types that a service can tell apart and a stack can receive ([`SignalType`]).

use crate::Choice;

crate::choice_enum! {
    /// What a signal is: a modulation class that a spectrum sweep can guess, or a named system that a signal identifier can
    /// name. A stack declares the types it receives in its descriptor, and the engine maps a type to a stack.
    pub enum SignalType {
        /// An unmodulated carrier.
        Carrier = ("carrier", "Carrier"),
        /// AM voice.
        AmVoice = ("am_voice", "AM voice"),
        /// AM broadcast, with a steady carrier.
        AmBroadcast = ("am_broadcast", "AM broadcast"),
        /// Narrow FM voice, present most of the time.
        NarrowFmVoice = ("narrow_fm_voice", "Narrow FM voice"),
        /// Push-to-talk voice: narrow FM that comes and goes.
        KeyedVoice = ("keyed_voice", "Keyed voice"),
        /// Wide FM that is not a broadcast with a stereo pilot.
        WideFm = ("wide_fm", "Wide FM"),
        /// FM broadcast.
        FmBroadcast = ("fm_broadcast", "FM broadcast"),
        /// Single sideband voice, upper sideband.
        SsbUpper = ("ssb_voice_upper", "SSB voice (upper)"),
        /// Single sideband voice, lower sideband.
        SsbLower = ("ssb_voice_lower", "SSB voice (lower)"),
        /// Morse code (on-off keyed carrier).
        CwMorse = ("cw_morse", "CW (Morse)"),
        /// Narrow data of no known system: under 4 kHz, or narrow and bursty.
        NarrowDigital = ("narrow_digital", "Narrow digital"),
        /// Digital voice or data of no known system, 4 to 100 kHz, flat-topped.
        Digital = ("digital", "Digital"),
        /// Wide digital of no known system.
        WideDigital = ("wide_digital", "Wide digital"),
        /// ADS-B and Mode S replies.
        AdsbModeS = ("ads_b_mode_s", "ADS-B / Mode S"),
        /// Mode A and Mode C replies.
        ModeAcReplies = ("mode_a_c_replies", "Mode A/C replies"),
        /// Mode S interrogations.
        ModeSInterrogations = ("mode_s_interrogations", "Mode S interrogations"),
        /// Universal Access Transceiver.
        Uat = ("uat", "UAT"),
        /// DVB-T.
        DvbT = ("dvb_t", "DVB-T"),
        /// DVB-T2.
        DvbT2 = ("dvb_t2", "DVB-T2"),
        /// DAB and DAB+.
        Dab = ("dab", "DAB / DAB+"),
        /// ISDB-T.
        IsdbT = ("isdb_t", "ISDB-T"),
        /// ISDB-TSB.
        IsdbTsb = ("isdb_tsb", "ISDB-TSB"),
        /// DTMB.
        Dtmb = ("dtmb", "DTMB"),
        /// ATSC 1.0.
        Atsc1 = ("atsc_1_0", "ATSC 1.0"),
        /// DMR.
        Dmr = ("dmr", "DMR"),
        /// P25 Phase 1.
        P25Phase1 = ("p25_phase_1", "P25 Phase 1"),
        /// P25 Phase 2.
        P25Phase2 = ("p25_phase_2", "P25 Phase 2"),
        /// NXDN at 9600 bit/s.
        Nxdn96 = ("nxdn96", "NXDN96"),
        /// NXDN at 4800 bit/s.
        Nxdn48 = ("nxdn48", "NXDN48"),
        /// dPMR.
        Dpmr = ("dpmr", "dPMR"),
        /// Yaesu System Fusion.
        YaesuFusion = ("yaesu_system_fusion", "Yaesu System Fusion"),
        /// M17.
        M17 = ("m17", "M17"),
        /// D-STAR.
        DStar = ("d_star", "D-STAR"),
        /// TETRA.
        Tetra = ("tetra", "TETRA"),
        /// POCSAG paging.
        Pocsag = ("pocsag", "POCSAG"),
        /// AIS.
        Ais = ("ais", "AIS"),
        /// LoRa.
        Lora = ("lora", "LoRa"),
        /// AX.25 packet at 1200 bit/s in AFSK (APRS and others).
        PacketAfsk1200 = ("packet_afsk_1200", "Packet (1200 bit/s AFSK)"),
        /// RTTY.
        Rtty = ("rtty", "RTTY"),
        /// SITOR, NAVTEX included.
        Sitor = ("sitor", "SITOR"),
        /// PSK31 and its faster relatives.
        Psk31 = ("psk31", "PSK31"),
        /// MFSK.
        Mfsk = ("mfsk", "MFSK"),
        /// DominoEX.
        DominoEx = ("dominoex", "DominoEX"),
        /// THOR.
        Thor = ("thor", "THOR"),
        /// Olivia.
        Olivia = ("olivia", "Olivia"),
        /// Contestia.
        Contestia = ("contestia", "Contestia"),
        /// MT63.
        Mt63 = ("mt63", "MT63"),
        /// FSQ.
        Fsq = ("fsq", "FSQ"),
        /// IFKP.
        Ifkp = ("ifkp", "IFKP"),
        /// Hellschreiber.
        Hellschreiber = ("hellschreiber", "Hellschreiber"),
        /// Slow-scan television.
        Sstv = ("sstv", "SSTV"),
        /// ARDOP.
        Ardop = ("ardop", "ARDOP"),
        /// ROS.
        Ros = ("ros", "ROS"),
        /// FT8.
        Ft8 = ("ft8", "FT8"),
        /// FT4.
        Ft4 = ("ft4", "FT4"),
        /// FT2.
        Ft2 = ("ft2", "FT2"),
        /// WSPR.
        Wspr = ("wspr", "WSPR"),
        /// JT65.
        Jt65 = ("jt65", "JT65"),
        /// JT9.
        Jt9 = ("jt9", "JT9"),
        /// JT4.
        Jt4 = ("jt4", "JT4"),
        /// Q65.
        Q65 = ("q65", "Q65"),
        /// FST4.
        Fst4 = ("fst4", "FST4"),
        /// FST4W.
        Fst4w = ("fst4w", "FST4W"),
        /// MSK144.
        Msk144 = ("msk144", "MSK144"),
        /// JS8.
        Js8 = ("js8", "JS8"),
        /// The DroneID downlink of DJI drones.
        DjiDroneId = ("dji_droneid", "DJI DroneID"),
        /// FLARM.
        Flarm = ("flarm", "FLARM"),
        /// ATSC 3.0.
        Atsc3 = ("atsc_3_0", "ATSC 3.0"),
        /// Analogue television with amplitude-modulated vision.
        AnalogTv = ("analog_tv", "Analog TV"),
        /// MAC television (multiplexed analogue components).
        Mac = ("mac", "MAC"),
        /// Frequency-modulated analogue video.
        FmVideo = ("fm_video", "FM video"),
        /// ACARS.
        Acars = ("acars", "ACARS"),
        /// SELCAL tone calls.
        Selcal = ("selcal", "SELCAL"),
        /// HFDL.
        Hfdl = ("hfdl", "HFDL"),
        /// VDL Mode 2.
        Vdl2 = ("vdl_mode_2", "VDL Mode 2"),
        /// VDL Mode 4.
        Vdl4 = ("vdl_mode_4", "VDL Mode 4"),
        /// Satellite navigation signals (GPS, Galileo, BeiDou, GLONASS, QZSS).
        Gnss = ("gnss", "GNSS"),
        /// Satellite-based augmentation signals.
        Sbas = ("sbas", "SBAS"),
        /// LBJ train broadcasts.
        Lbj = ("lbj", "LBJ"),
        /// Railway balise up-link telegrams.
        Balise = ("balise", "Balise"),
        /// GSM and GSM-R.
        Gsm = ("gsm", "GSM"),
        /// NB-IoT.
        NbIot = ("nb_iot", "NB-IoT"),
        /// LTE.
        Lte = ("lte", "LTE"),
        /// 5G NR.
        Nr = ("nr", "5G NR"),
        /// DECT.
        Dect = ("dect", "DECT"),
        /// ANT and ANT+.
        Ant = ("ant", "ANT / ANT+"),
        /// IEEE 802.15.4 (Zigbee and others).
        Ieee802154 = ("ieee_802_15_4", "IEEE 802.15.4"),
        /// NFC and RFID at 13.56 MHz.
        Nfc = ("nfc", "NFC"),
        /// The Enhanced ShockBurst radio of nRF24 chips.
        Nrf24 = ("nrf24", "nRF24"),
        /// Sigfox.
        Sigfox = ("sigfox", "Sigfox"),
        /// Wireless M-Bus.
        WirelessMbus = ("wireless_m_bus", "Wireless M-Bus"),
        /// Wi-SUN.
        WiSun = ("wi_sun", "Wi-SUN"),
        /// Z-Wave.
        ZWave = ("z_wave", "Z-Wave"),
        /// DVB-S.
        DvbS = ("dvb_s", "DVB-S"),
        /// DVB-S2.
        DvbS2 = ("dvb_s2", "DVB-S2"),
        /// DVB-C.
        DvbC = ("dvb_c", "DVB-C"),
        /// DVB-C2.
        DvbC2 = ("dvb_c2", "DVB-C2"),
        /// ISDB-S.
        IsdbS = ("isdb_s", "ISDB-S"),
        /// TETRAPOL.
        Tetrapol = ("tetrapol", "TETRAPOL"),
        /// FreeDV.
        FreeDv = ("freedv", "FreeDV"),
        /// MPT 1327 trunking.
        Mpt1327 = ("mpt1327", "MPT 1327"),
        /// LTR trunking.
        Ltr = ("ltr", "LTR"),
        /// EDACS trunking.
        Edacs = ("edacs", "EDACS"),
        /// Multitone paging.
        MultitonePaging = ("multitone_paging", "Multitone paging"),
        /// Wi-Fi (IEEE 802.11).
        Wifi = ("wifi", "Wi-Fi"),
        /// Wi-Fi HaLow (IEEE 802.11ah).
        WifiHalow = ("wifi_halow", "Wi-Fi HaLow"),
        /// Bluetooth Low Energy.
        BluetoothLe = ("bluetooth_le", "Bluetooth LE"),
        /// A non-directional beacon: a carrier with a keyed tone.
        Ndb = ("ndb", "NDB"),
        /// VOR.
        Vor = ("vor", "VOR"),
        /// ILS localizer and glide slope.
        Ils = ("ils", "ILS"),
        /// DME.
        Dme = ("dme", "DME"),
        /// Loran-C and eLoran.
        LoranC = ("loran_c", "Loran-C"),
        /// DGPS beacons.
        Dgps = ("dgps", "DGPS"),
        /// LRIT weather satellite downlinks.
        Lrit = ("lrit", "LRIT"),
        /// HRIT weather satellite downlinks.
        Hrit = ("hrit", "HRIT"),
        /// HRPT and AHRPT weather satellite downlinks.
        Hrpt = ("hrpt", "HRPT / AHRPT"),
        /// LRPT weather satellite downlinks.
        Lrpt = ("lrpt", "LRPT"),
        /// S-VISSR weather satellite downlinks.
        SVissr = ("s_vissr", "S-VISSR"),
        /// Digital selective calling.
        Dsc = ("dsc", "DSC"),
        /// Weather fax.
        Wefax = ("weather_fax", "Weather fax"),
        /// VDES.
        Vdes = ("vdes", "VDES"),
        /// ATIS identification after an inland waterway transmission.
        Atis = ("atis", "ATIS"),
        /// DRM.
        Drm = ("drm", "DRM"),
        /// HamDRM.
        HamDrm = ("hamdrm", "HamDRM"),
        /// HD Radio.
        HdRadio = ("hd_radio", "HD Radio"),
        /// CDR (China Digital Radio).
        Cdr = ("cdr", "CDR"),
        /// PACTOR-I.
        Pactor1 = ("pactor_1", "PACTOR-I"),
        /// PACTOR-II.
        Pactor2 = ("pactor_2", "PACTOR-II"),
        /// PACTOR-III.
        Pactor3 = ("pactor_3", "PACTOR-III"),
        /// PACTOR-IV.
        Pactor4 = ("pactor_4", "PACTOR-IV"),
        /// HF ocean radar sweeps.
        HfOceanRadar = ("hf_ocean_radar", "HF ocean radar"),
        /// Second-generation HF ALE.
        Ale2g = ("ale_2g", "ALE (2G)"),
        /// Radiosondes.
        Radiosonde = ("radiosonde", "Radiosonde"),
        /// SAME alert headers.
        Same = ("same", "SAME"),
        /// Time signal stations.
        TimeSignal = ("time_signal", "Time signal"),
        /// Data collection platforms of weather satellites.
        Dcp = ("dcp", "Data collection platform"),
        /// Argos platform bursts.
        Argos = ("argos", "Argos"),
        /// COSPAS-SARSAT distress beacons on 406 MHz.
        Cospas406 = ("cospas_sarsat_406", "COSPAS-SARSAT 406 MHz"),
        /// Inmarsat-C.
        InmarsatC = ("inmarsat_c", "Inmarsat-C"),
        /// Inmarsat Aero.
        InmarsatAero = ("inmarsat_aero", "Inmarsat Aero"),
        /// Orbcomm.
        Orbcomm = ("orbcomm", "Orbcomm"),
        /// Iridium.
        Iridium = ("iridium", "Iridium"),
        /// Globalstar.
        Globalstar = ("globalstar", "Globalstar"),
    }
}

impl serde::Serialize for SignalType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.key())
    }
}

impl<'de> serde::Deserialize<'de> for SignalType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let key = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        SignalType::from_key(&key).ok_or_else(|| serde::de::Error::unknown_variant(&key, &[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_round_trip() {
        let all = SignalType::CHOICES;
        for (i, c) in all.iter().enumerate() {
            assert!(all[i + 1..].iter().all(|d| d.key != c.key), "{}", c.key);
            let t = SignalType::from_index(i as u16).unwrap();
            assert_eq!(t.key(), c.key);
        }
        assert_eq!(SignalType::Dab.label(), "DAB / DAB+");
    }
}

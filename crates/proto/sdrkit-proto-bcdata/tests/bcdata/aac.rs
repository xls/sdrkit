use sdrkit_proto_bcdata::aac::*;
use sdrkit_util_bits::BitReader;

fn asc(c: &AudioSpecificConfig) -> Vec<u8> {
    let mut b = Vec::new();
    c.write(&mut b).unwrap();
    b
}

#[test]
fn audio_specific_config_known_answers() {
    // ISO/IEC 14496-3 1.6.2.1. AAC-LC 48 kHz stereo: 00010 0011 0010 000 = 0x11 0x90. With frameLengthFlag 1 it is 0x11 0x94.
    assert_eq!(asc(&AudioSpecificConfig::lc(48_000, 2)), [0x11, 0x90]);
    assert_eq!(
        asc(&AudioSpecificConfig {
            frame_960: true,
            ..AudioSpecificConfig::lc(48_000, 2)
        }),
        [0x11, 0x94]
    );
    // AAC-LC 44.1 kHz stereo: 0x12 0x10.
    assert_eq!(asc(&AudioSpecificConfig::lc(44_100, 2)), [0x12, 0x10]);
    // HE-AAC, explicit hierarchical: object type 5, core 24 kHz stereo, SBR 48 kHz, core type 2: 0x2B 0x11 0x88 0x00.
    let he = AudioSpecificConfig {
        sbr_rate: Some(48_000),
        ..AudioSpecificConfig::lc(24_000, 2)
    };
    assert_eq!(asc(&he), [0x2B, 0x11, 0x88, 0x00]);
    // HE-AAC v2: object type 29, core 24 kHz mono: 0xEB 0x09 0x88 0x00.
    let v2 = AudioSpecificConfig {
        ps: true,
        ..AudioSpecificConfig { channels: 1, ..he }
    };
    assert_eq!(asc(&v2), [0xEB, 0x09, 0x88, 0x00]);
    // A rate outside Table 1.18: index 15 and 24 bits.
    let odd = asc(&AudioSpecificConfig::lc(10_000, 1));
    assert_eq!(odd.len(), 5);
    assert_eq!(
        AudioSpecificConfig::parse(&odd).unwrap().sample_rate,
        10_000
    );
    for c in [he, v2, AudioSpecificConfig::lc(48_000, 2)] {
        assert_eq!(AudioSpecificConfig::parse(&asc(&c)).unwrap(), c);
    }
    assert!(AudioSpecificConfig {
        ps: true,
        ..AudioSpecificConfig::lc(24_000, 1)
    }
    .write(&mut Vec::new())
    .is_err());
}

#[test]
fn dab_plus_superframe_flags_of_ts_102_563() {
    let c = AudioSpecificConfig::dab_plus(true, true, true, true).unwrap();
    assert_eq!(
        (c.sample_rate, c.sbr_rate, c.channels, c.ps, c.frame_960),
        (24_000, Some(48_000), 1, true, true)
    );
    assert_eq!(c.samples_per_unit(), 1920);
    let c = AudioSpecificConfig::dab_plus(false, false, true, false).unwrap();
    assert_eq!(
        (c.sample_rate, c.channels, c.samples_per_unit()),
        (32_000, 2, 960)
    );
    assert!(AudioSpecificConfig::dab_plus(true, false, true, true).is_err());
}

#[test]
fn latm_config_is_not_padded() {
    // The StreamMuxConfig holds the AudioSpecificConfig bit by bit: for HE-AAC (25 bits) frameLengthType and
    // latmBufferFullness 0xFF follow at once. Padding the ASC to whole bytes (as the DAB+ packager this crate replaces did)
    // moves them by 7 bits.
    let c = AudioSpecificConfig::dab_plus(true, true, true, false).unwrap();
    let mut b = Vec::new();
    write_latm(&c, true, &[0xAB; 300], &mut b).unwrap();
    let mut r = BitReader::new(&b);
    assert_eq!(r.u(1), 0); // useSameStreamMux
                           // audioMuxVersion 0, allStreamsSameTimeFraming 1, numSubFrames 0, numProgram 0, numLayer 0.
    assert_eq!(r.u(15), 0x2000);
    let mut asc_bits = Vec::new();
    c.write(&mut asc_bits).unwrap();
    assert_eq!(
        r.u(25),
        u32::from_be_bytes([asc_bits[0], asc_bits[1], asc_bits[2], asc_bits[3]]) as u64 >> 7
    );
    assert_eq!(r.u(3), 0); // frameLengthType
    assert_eq!(r.u(8), 0xFF); // latmBufferFullness
    assert_eq!(r.u(2), 0); // otherDataPresent, crcCheckPresent
    assert_eq!((r.u(8), r.u(8)), (255, 45)); // MuxSlotLengthBytes 300
    assert_eq!(r.u(8), 0xAB);
    // Without the config: useSameStreamMux 1, then the length.
    let mut b = Vec::new();
    write_latm(&c, false, &[1, 2], &mut b).unwrap();
    assert_eq!(b, [0x81, 0x00, 0x81, 0x00]);
}

#[test]
fn loas_sync_and_length() {
    let c = AudioSpecificConfig::lc(48_000, 2);
    let mut b = Vec::new();
    write_loas(&c, false, &[9; 10], &mut b).unwrap();
    // syncword 0x2B7, audioMuxLengthBytes = 12 (one flag bit, one length byte, ten bytes, padding).
    assert_eq!(&b[..3], &[0x56, 0xE0, 12]);
    assert_eq!(b.len(), 15);
}

#[test]
fn adts_header_known_answer() {
    // AAC-LC 44.1 kHz stereo, 371-byte frame: FF F1 50 80 2E 7F FC.
    let mut b = Vec::new();
    write_adts_header(&AudioSpecificConfig::lc(44_100, 2), 364, &mut b).unwrap();
    assert_eq!(b, [0xFF, 0xF1, 0x50, 0x80, 0x2E, 0x7F, 0xFC]);
    let dab = AudioSpecificConfig::dab_plus(true, false, true, false).unwrap();
    assert!(write_adts_header(&dab, 100, &mut b).is_err());
}

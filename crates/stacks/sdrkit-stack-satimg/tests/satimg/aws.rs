//! AWS L-band: the frame and packet layers, the radiometer record and the receiver on its test signal.

use super::common::{impaired, run};
use sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S;
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::LockState;
use sdrkit_dev_test::Harness;
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::packet::PrimaryHeader;
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_stack_satimg::gen::test_value;
use sdrkit_stack_satimg::instruments::mwr::{
    sample, test_record, MwrDecoder, CHANNELS, FIELD_HEADER, RECORD_BYTES, SPOTS,
};
use sdrkit_stack_satimg::modes::aws::{
    test_frame, test_packets, CADU_BYTES, CODED_BYTES, DESCRIPTOR, FRAME,
};

#[test]
fn the_frame_is_a_tm_frame_of_1115_bytes() {
    assert_eq!((CADU_BYTES, CODED_BYTES), (1279, 1275));
    assert_eq!((FRAME.frame_len, FRAME.data_len()), (1115, 1109));
    let f = test_frame(9, FirstHeader::At(5), &[0xAB; 1109]);
    assert_eq!(f[..6], [0x06, 0x86, 9, 9, 0x18, 0x05]);
    let p = FRAME.parse(&f).unwrap();
    assert_eq!(
        (p.header.version, p.header.spacecraft, p.header.vcid),
        (0, 104, 3)
    );
    assert_eq!(p.data, &[0xAB; 1109][..]);
}

#[test]
fn a_coded_frame_takes_16_byte_errors_in_each_code_word() {
    let frame = test_frame(1, FirstHeader::None, &[0x5A; 1109]);
    let mut cadu = Vec::new();
    ChannelCoder::new(ChannelConfig::rs_223(5))
        .unwrap()
        .encode(&frame, &mut cadu)
        .unwrap();
    assert_eq!(cadu.len(), 1279);
    assert_eq!(cadu[..4], [0x1A, 0xCF, 0xFC, 0x1D]);
    let mut dec = ChannelDecoder::new(ChannelConfig::rs_223(5)).unwrap();
    let mut coded = cadu[4..].to_vec();
    for w in 0..5 {
        for j in 0..16 {
            coded[w + 5 * (3 + 13 * j)] ^= 0xA5;
        }
    }
    let mut buf = coded.clone();
    let d = dec.decode(&mut buf).unwrap();
    assert!(d.ok());
    assert_eq!((d.corrected, d.frame_len), (80, 1115));
    assert_eq!(buf[..1115], frame[..]);
    coded[2 + 5 * 250] ^= 0xFF;
    let d = dec.decode(&mut coded).unwrap();
    assert!(!d.ok());
}

#[test]
fn a_scan_record_comes_out_of_its_eight_packets() {
    let record = test_record(3, test_value);
    assert_eq!(record.len(), RECORD_BYTES);
    assert_eq!(
        sample(&record, 144, 18),
        u16::from_le_bytes([record[7595], record[7596]])
    );
    assert_eq!(sample(&record, 144, 18), test_value(18, 8 * 144, 3) * 64);
    let packets = test_packets(3, 16_380);
    assert_eq!(packets.len(), 8);
    let sizes: Vec<usize> = packets
        .iter()
        .map(|p| p.len() - 6 - FIELD_HEADER - 2)
        .collect();
    assert_eq!(sizes, [1000, 1000, 1000, 1000, 1000, 1000, 1000, 599]);
    let mut dec = MwrDecoder::default();
    let mut strips = Vec::new();
    for (i, p) in packets.iter().enumerate() {
        let h = PrimaryHeader::parse(p).unwrap();
        assert_eq!((h.apid, h.count), (100, (16_380 + i as u16) & 0x3FFF));
        assert!(strips.is_empty());
        // The error control field covers the whole packet before it.
        let check = sdrkit_util_bits::crc::catalog::CRC16_IBM_3740.checksum(&p[..p.len() - 2]);
        assert_eq!(check.to_be_bytes(), p[p.len() - 2..]);
        dec.push(h.flags.bits(), &p[6..p.len() - 2], true, &mut strips);
    }
    assert_eq!(strips.len(), CHANNELS);
    assert_eq!(dec.stats.odd_records, 0);
    for s in &strips {
        let c = usize::from(s.channel) - 1;
        let values: Vec<f32> = (0..SPOTS)
            .map(|p| f32::from(sample(&record, p, c)))
            .collect();
        let lo = values.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        for (v, got) in values.iter().zip(s.samples.iter()) {
            assert_eq!(*got, ((v - lo) / (hi - lo) * 255.0).round() as u8);
        }
    }
}

#[test]
fn the_receiver_draws_scans_through_noise_offset_and_drift() {
    let h = impaired(
        Harness::new(&DESCRIPTOR)
            .rate(6e6)
            .seconds(2.0)
            .set(&GEN_DRIFT_HZ_S, -700.0),
        Some(10.0),
        28e3,
        40.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, 6e6, 1707e6, 1707e6, 65_536, Some("frames.out"));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 250.0, "{}", r.num("frames_ok"));
    assert!(r.num("lines") >= 40.0, "{}", r.num("lines"));
    assert_eq!(r.num("spacecraft"), 104.0);
    assert_eq!(r.num("vcid"), 3.0);
    assert_eq!(r.num("packet_crc_errors"), 0.0);
    assert_eq!(r.num("odd_records"), 0.0);
    let strips = r.of::<Strip>();
    let channels: std::collections::BTreeSet<u16> = strips.iter().map(|s| s.channel).collect();
    assert_eq!(channels.len(), 19);
    assert!(strips.iter().all(|s| s.width as usize == SPOTS));
}

#[test]
fn noise_gives_no_frame() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    let x: Vec<sdrkit_core::Complex32> = (0..(3.0 * 6e6) as usize)
        .map(|_| sdrkit_core::Complex32::new(next() + next(), next() + next()) * 0.2)
        .collect();
    let h = Harness::new(&DESCRIPTOR).rate(6e6);
    let r = run::<Strip>(&h, &x, 6e6, 1707e6, 1707e6, 65_536, Some("frames.out"));
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.num("frames_ok"), 0.0);
    assert!(r.of::<Strip>().is_empty());
}

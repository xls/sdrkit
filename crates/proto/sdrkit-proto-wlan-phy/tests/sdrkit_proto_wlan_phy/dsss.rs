//! 802.11b DSSS (IEEE Std 802.11-2020 clause 16).

use super::{cuts, impair};
use sdrkit_proto_wlan_phy::dsss::{self, DsssReceiver, PlcpHeader, Preamble, Rate};
use sdrkit_proto_wlan_phy::{mac, Complex32};
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::{pack, BitOrder};

#[test]
fn barker_and_sfd() {
    // Clause 16.4.6.3: +1, -1, +1, +1, -1, +1, +1, +1, -1, -1, -1, the first chip first.
    assert_eq!(dsss::BARKER_11, [1, -1, 1, 1, -1, 1, 1, 1, -1, -1, -1]);
    // Its aperiodic autocorrelation sidelobes are 0 or -1.
    for lag in 1..11 {
        let s: i32 = (0..11 - lag)
            .map(|i| (dsss::BARKER_11[i] * dsss::BARKER_11[i + lag]) as i32)
            .sum();
        assert!(s == 0 || s == -1, "{lag}");
    }
    // The short SFD is the long SFD sent in time-reversed order.
    assert_eq!(dsss::SFD_LONG.reverse_bits(), dsss::SFD_SHORT);
}

/// The header CRC of clause 16.2.3.7 written as the figure draws it: x^16 + x^12 + x^5 + 1, preset to ones, the bits in the
/// order sent, the remainder ones-complemented and sent from x^15.
fn crc_figure(bits: &[u8]) -> u16 {
    let mut c = 0xFFFFu16;
    for &b in bits {
        let fb = ((c >> 15) as u8 & 1) ^ b;
        c <<= 1;
        if fb == 1 {
            c ^= 0x1021;
        }
    }
    !c
}

#[test]
fn header_crc_and_fields() {
    assert_eq!(CRC16_GENIBUS.checksum(b"123456789"), 0xD64E);
    for (rate, octets) in [
        (Rate::Dbpsk1, 14),
        (Rate::Dqpsk2, 1500),
        (Rate::Cck5_5, 77),
        (Rate::Cck11, 1026),
    ] {
        let h = PlcpHeader::for_psdu(rate, octets).unwrap();
        let mut bits = Vec::new();
        h.push_bits(&mut bits);
        assert_eq!(bits.len(), 48);
        assert_eq!(pack::to_u64(&bits[32..]) as u16, crc_figure(&bits[..32]));
        assert_eq!(PlcpHeader::from_bits(&bits).unwrap(), h);
        assert_eq!(h.psdu_octets(), Some(octets));
        bits[20] ^= 1;
        assert!(PlcpHeader::from_bits(&bits).is_err());
    }
    // SIGNAL values of clause 16.2.3.4 and 16.3.3.4.
    assert_eq!(
        [Rate::Dbpsk1, Rate::Dqpsk2, Rate::Cck5_5, Rate::Cck11].map(|r| r.signal()),
        [0x0A, 0x14, 0x37, 0x6E]
    );
}

#[test]
fn length_extension_example_of_16_3_3_6() {
    // The table of the standard: 11 Mbit/s, 1023 to 1026 octets.
    for (octets, length, ext) in [
        (1023, 744, false),
        (1024, 745, false),
        (1025, 746, false),
        (1026, 747, true),
    ] {
        let h = PlcpHeader::for_psdu(Rate::Cck11, octets).unwrap();
        assert_eq!(h.length_us, length);
        assert_eq!(
            h.service & dsss::SERVICE_LENGTH_EXTENSION != 0,
            ext,
            "{octets}"
        );
        assert_eq!(h.psdu_octets(), Some(octets));
    }
    // 1 and 2 Mbit/s: 8 and 4 µs per octet.
    assert_eq!(
        PlcpHeader::for_psdu(Rate::Dbpsk1, 100).unwrap().length_us,
        800
    );
    assert_eq!(
        PlcpHeader::for_psdu(Rate::Dqpsk2, 100).unwrap().length_us,
        400
    );
    // 5.5 Mbit/s round trips for every length.
    for octets in 1..3000 {
        assert_eq!(
            PlcpHeader::for_psdu(Rate::Cck5_5, octets)
                .unwrap()
                .psdu_octets(),
            Some(octets)
        );
        assert_eq!(
            PlcpHeader::for_psdu(Rate::Cck11, octets)
                .unwrap()
                .psdu_octets(),
            Some(octets)
        );
    }
    assert!(PlcpHeader::for_psdu(Rate::Dbpsk1, 9000).is_err());
}

#[test]
fn ppdu_bits_and_scrambler() {
    let psdu = [0xA5u8; 20];
    let h = PlcpHeader::for_psdu(Rate::Dbpsk1, psdu.len()).unwrap();
    let mut bits = Vec::new();
    dsss::ppdu_bits(Preamble::Long, &h, &psdu, &mut bits).unwrap();
    assert!(bits[..128].iter().all(|&b| b == 1));
    assert_eq!(pack::to_u64_lsb(&bits[128..144]) as u16, 0xF3A0);
    let mut s = bits.clone();
    dsss::scramble(&mut s, Preamble::Long);
    // The receiver's descrambler is self-synchronising: after 7 bits it returns the data back.
    let mut d = s.clone();
    sdrkit_util_bits::seq::ieee80211b(0).descramble(&mut d);
    assert_eq!(&d[7..], &bits[7..]);
    let mut out = Vec::new();
    pack::pack(
        &bits[192..],
        BitOrder::Lsb,
        sdrkit_util_bits::Pad::Zero,
        &mut out,
    );
    assert_eq!(out, psdu);
    assert!(dsss::ppdu_bits(Preamble::Long, &h, &psdu[..3], &mut Vec::new()).is_err());
    // DQPSK quarters of table 16-3.
    assert_eq!(
        [(0, 0), (0, 1), (1, 1), (1, 0)].map(|(a, b)| dsss::dqpsk_quarters(a, b)),
        [0, 1, 2, 3]
    );
}

/// A PPDU at 22 samples per symbol (two per chip).
fn signal(mpdu: &[u8], preamble: Preamble, rate: Rate) -> Vec<Complex32> {
    let mut psdu = mpdu.to_vec();
    mac::push_fcs(&mut psdu);
    let h = PlcpHeader::for_psdu(rate, psdu.len()).unwrap();
    let mut bits = Vec::new();
    dsss::ppdu_bits(preamble, &h, &psdu, &mut bits).unwrap();
    dsss::scramble(&mut bits, preamble);
    let mut sym = Vec::new();
    dsss::symbols(&bits, preamble, rate, &mut sym).unwrap();
    let mut chips = Vec::new();
    dsss::spread(&sym, &mut chips);
    chips.iter().flat_map(|&c| [c, c]).collect()
}

#[test]
fn receiver_decodes_long_and_short_preambles_at_1_and_2_mbits() {
    let cases = [
        (Preamble::Long, Rate::Dbpsk1),
        (Preamble::Long, Rate::Dqpsk2),
        (Preamble::Short, Rate::Dqpsk2),
        (Preamble::Long, Rate::Dbpsk1),
    ];
    let mpdus: Vec<Vec<u8>> = (0..cases.len())
        .map(|i| (0..60 + 10 * i).map(|k| (k * 31 + i) as u8).collect())
        .collect();
    let mut x = Vec::new();
    for (m, &(p, r)) in mpdus.iter().zip(&cases) {
        x.extend(impair(&signal(m, p, r), 22e6, 0.0, 0.0, 2000, 0));
    }
    // 30 kHz offset, SNR about 3 dB per sample (14 dB after despreading).
    let y = impair(&x, 22e6, 30e3, 0.5, 3000, 9);
    let mut a = DsssReceiver::new(22).unwrap();
    let mut whole = Vec::new();
    a.process(&y, |f| {
        whole.push((f.mpdu.to_vec(), f.preamble, f.header.rate))
    });
    assert_eq!(whole.len(), cases.len(), "{:?}", a.stats());
    for ((m, &(p, r)), (gm, gp, gr)) in mpdus.iter().zip(&cases).zip(&whole) {
        assert_eq!((gm, *gp, *gr), (m, p, r));
    }
    let mut b = DsssReceiver::new(22).unwrap();
    let mut cut = Vec::new();
    for piece in cuts(&y, 4000, 1) {
        b.process(piece, |f| {
            cut.push((f.mpdu.to_vec(), f.preamble, f.header.rate))
        });
    }
    assert_eq!(cut, whole);
    assert!(DsssReceiver::new(5).is_err());
}

#[test]
fn receiver_counts_cck_headers() {
    let psdu = [0u8; 50];
    let h = PlcpHeader::for_psdu(Rate::Cck11, psdu.len()).unwrap();
    let mut bits = Vec::new();
    dsss::ppdu_bits(Preamble::Long, &h, &psdu, &mut bits).unwrap();
    bits.truncate(192); // preamble and header only: the CCK part is not built here
    dsss::scramble(&mut bits, Preamble::Long);
    let mut sym = Vec::new();
    dsss::symbols(&bits, Preamble::Long, Rate::Dbpsk1, &mut sym).unwrap();
    let mut chips = Vec::new();
    dsss::spread(&sym, &mut chips);
    let x: Vec<Complex32> = chips.iter().flat_map(|&c| [c, c]).collect();
    let y = impair(&x, 22e6, 0.0, 0.0, 2000, 0);
    let mut rx = DsssReceiver::new(22).unwrap();
    rx.process(&y, |_| panic!("no frame"));
    assert_eq!(rx.stats().other_rate, 1);
    assert!(dsss::symbols(&bits, Preamble::Long, Rate::Cck11, &mut sym).is_err());
}

#[test]
fn frame_level_is_the_power_of_the_preamble() {
    let x = signal(&[0x5A; 40], Preamble::Long, Rate::Dbpsk1);
    let p_db = super::power_db(&x[..2000]);
    for gain in [1.0f32, 0.1, 0.01] {
        let y = impair(&super::scaled(&x, gain), 22e6, 0.0, 1e-8, 3000, 2);
        let mut lv = Vec::new();
        DsssReceiver::new(22)
            .unwrap()
            .process(&y, |f| lv.push(f.level_dbfs));
        let want = p_db + 20.0 * (gain as f64).log10();
        assert_eq!(lv.len(), 1);
        assert!((lv[0] - want).abs() < 0.2, "gain {gain}: {} {want}", lv[0]);
    }
}

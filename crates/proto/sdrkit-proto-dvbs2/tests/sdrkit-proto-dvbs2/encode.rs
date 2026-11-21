//! The transmit coding shared by the test-signal generator and the senders: the generator's samples are pinned, and the
//! DVB-S2 encoder's header, padding and bit rate follow EN 302 307-1.

use sdrkit_core::{Complex32, Generator};
use sdrkit_proto_dvbs2::gen::{DvbsGenParams, DvbsGenerator, GenStandard};
use sdrkit_proto_dvbs2::inner::PunctureRate;
use sdrkit_proto_dvbs2::modcod::ModCod;
use sdrkit_proto_dvbs2::transmit::payload;

/// FNV-1a over the bits of the samples.
fn hash(x: &[Complex32]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for z in x {
        for v in [z.re.to_bits(), z.im.to_bits()] {
            for b in v.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

/// A DVB-S2 generator configuration at 1 Msym/s and 2 Msps with numbered packets.
fn s2(modcod: u8, short: bool, pilots: bool) -> DvbsGenParams {
    let mut p = DvbsGenParams::new(2e6);
    p.symbol_rate = 1e6;
    p.source = payload(true);
    p.standard = GenStandard::S2 {
        modcod: ModCod::new(modcod).unwrap(),
        short,
        pilots,
    };
    p
}

/// The hash of the first 200 000 samples of the generator of `p`, made in blocks of 4096.
fn run(p: DvbsGenParams) -> u64 {
    let mut g = DvbsGenerator::new(p);
    let mut x = vec![Complex32::default(); 200_000];
    for c in x.chunks_mut(4096) {
        assert_eq!(g.generate(c), c.len());
    }
    hash(&x)
}

/// The samples of the generator are those recorded on the machine that made the pins (64-bit ARM): the shared encoders
/// leave every sample as it was.
#[test]
fn the_generator_samples_are_the_recorded_ones() {
    let cases: Vec<(&str, DvbsGenParams, u64)> = vec![
        ("QPSK 2/3 normal", s2(6, false, false), 0x1e85_8df2_0c2d_75c8),
        ("QPSK 1/4 normal, pilots", s2(1, false, true), 0xe255_2b4f_438c_9891),
        ("8PSK 3/5 short, pilots", s2(12, true, true), 0x19d3_611f_7ff4_c69a),
        ("8PSK 9/10 normal", s2(17, false, false), 0x34e2_6dc3_ba62_c0ab),
        ("16APSK 3/4 normal, pilots", s2(19, false, true), 0x5da3_64eb_1a51_1057),
        ("16APSK 2/3 short", s2(18, true, false), 0x1415_0d22_f0d3_2344),
        ("32APSK 4/5 normal", s2(25, false, false), 0x931c_8421_27bc_ae30),
        ("32APSK 8/9 short, pilots", s2(27, true, true), 0xb99c_978c_db3e_6be9),
        ("QPSK 3/4, roll-off 0.25", DvbsGenParams { rolloff: 0.25, ..s2(7, false, false) }, 0x0aec_b9f9_9b0c_e4d6),
        ("8PSK 2/3, roll-off 0.20", DvbsGenParams { rolloff: 0.20, ..s2(13, true, false) }, 0xe685_5f27_9342_6c68),
        ("QPSK 1/2, gold code 5", DvbsGenParams { gold: 5, ..s2(4, true, true) }, 0x4bee_05de_453c_f2c7),
        ("QPSK 2/3, a dummy frame every 2", DvbsGenParams { dummy_every: 2, ..s2(6, true, false) }, 0x98c2_3c96_4b78_1a22),
        ("GSE", DvbsGenParams { gse: true, ..s2(6, true, false) }, 0x3f47_001a_361e_cc7f),
        ("multistream", DvbsGenParams { multistream: true, ..s2(13, true, true) }, 0xada3_373a_98f9_b09d),
        ("9/10 short: dummy frames", s2(11, true, false), 0xa6bd_3156_1130_11ab),
        ("the test multiplex", DvbsGenParams { source: payload(false), ..s2(6, false, false) }, 0x37fc_1095_9c1e_ecd2),
        (
            "DVB-S 3/4",
            DvbsGenParams { standard: GenStandard::S(PunctureRate::R34), ..s2(6, false, false) },
            0xb893_52b7_b365_6cc4,
        ),
    ];
    let mut bad = Vec::new();
    for (name, p, want) in cases {
        let got = run(p);
        if got != want {
            bad.push(format!("(\"{name}\", {got:#018x})"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

use sdrkit_proto_dvbs2::fec::FrameDecoder;
use sdrkit_proto_dvbs2::pl::{code7, header_symbols, pl_scrambling, quarter, PILOT};
use sdrkit_proto_dvbs2::s2encode::{useful_bit_rate, S2Encoder, TsAdapter};
use sdrkit_proto_ipcast::adapt::UpDeframer;
use sdrkit_proto_ipcast::bbheader::{bb_descramble, BbHeader, StreamFormat, BBHEADER_LEN};
use sdrkit_proto_mpegts::source::{Numbered, NumberedSource, PacketSource};

/// The data symbols of the PLFRAME `syms` of `m`: the header is the one of the mode, the PL scrambling of code `gold` is
/// taken off, and the pilot blocks, each 36 unmodulated symbols after 16 slots, are taken out.
fn data_symbols(syms: &[Complex32], m: &ModCod, short: bool, pilots: bool, gold: u32) -> Vec<Complex32> {
    assert_eq!(syms.len(), m.frame_symbols(short, pilots), "{}", m.name());
    assert_eq!(syms[..90], header_symbols(code7(m.id, short, pilots))[..]);
    let mut scr = Vec::new();
    pl_scrambling(gold, syms.len() - 90, &mut scr);
    let body: Vec<Complex32> = syms[90..]
        .iter()
        .zip(&scr)
        .map(|(s, &r)| quarter(*s, (4 - r) & 3))
        .collect();
    if !pilots {
        return body;
    }
    let mut data = Vec::new();
    for (k, c) in body.chunks(1440 + 36).enumerate() {
        data.extend_from_slice(&c[..c.len().min(1440)]);
        for p in c.iter().skip(1440) {
            assert!((*p - PILOT).norm() < 1e-6, "{}: pilot block {k}", m.name());
        }
    }
    data
}

/// The FEC decoder of the receiver and its deframer return the packets that went into the encoder, for every MODCOD,
/// both frame sizes, with and without pilots; the packets cut across frames are joined.
#[test]
fn every_mode_decodes_back_to_its_packets() {
    let mut dec = FrameDecoder::default();
    for m in ModCod::all() {
        for (short, pilots) in [(false, false), (false, true), (true, false), (true, true)] {
            if short && !m.has_short() {
                assert!(S2Encoder::new(m, true, pilots, 0).code().is_none());
                continue;
            }
            let gold = u32::from(m.id) * 1000;
            let mut enc = S2Encoder::new(m, short, pilots, gold);
            let mut adapter = TsAdapter::default();
            let data = enc.code().unwrap().kbch / 8 - BBHEADER_LEN;
            let mut src = NumberedSource::new(Numbered::SATELLITE_CABLE);
            let mut sent = Vec::new();
            let mut got = Vec::new();
            let mut deframer = UpDeframer::new();
            let (mut syms, mut bb) = (Vec::new(), Vec::new());
            for _ in 0..2 {
                adapter.fill(enc.bbframe(), data, None, 0, &mut || {
                    let mut p = [0u8; 188];
                    src.next_packet(&mut p);
                    sent.push(p);
                    p
                });
                assert!(enc.encode(&mut syms));
                let d = data_symbols(&syms, &m, short, pilots, gold);
                let r = dec.decode(&m, short, &d, 0.01, &mut bb).expect("a frame of the mode");
                assert!(r.ldpc_ok && r.bch_ok && r.iterations <= 1, "{} {r:?}", m.name());
                bb_descramble(&mut bb);
                let h = BbHeader::parse(&bb).expect("the CRC-8 of the BBHEADER");
                assert_eq!((h.format, h.single_stream, h.ccm), (StreamFormat::Transport, true, true));
                assert_eq!((h.upl, h.sync, usize::from(h.dfl)), (188 * 8, 0x47, 8 * data));
                deframer
                    .push_frame(&bb, &mut |p| {
                        assert!(p.crc_ok);
                        got.push(p.bytes.to_vec());
                    })
                    .unwrap();
            }
            assert!(got.len() >= 2 * data / 188 - 1, "{}: {} packets", m.name(), got.len());
            for (g, s) in got.iter().zip(&sent) {
                assert_eq!(g[..], s[..], "{} short {short} pilots {pilots}", m.name());
            }
        }
    }
}

/// A data field shorter than the frame: DFL states its length, and zeros pad the BBFRAME up to K_bch.
#[test]
fn a_short_data_field_is_padded() {
    let m = ModCod::new(4).unwrap();
    let mut enc = S2Encoder::new(m, true, false, 0);
    let mut adapter = TsAdapter::default();
    let mut src = NumberedSource::new(Numbered::SATELLITE_CABLE);
    let mut sent = Vec::new();
    let data = 3 * 188;
    adapter.fill(enc.bbframe(), data, None, 2, &mut || {
        let mut p = [0u8; 188];
        src.next_packet(&mut p);
        sent.push(p);
        p
    });
    let mut syms = Vec::new();
    assert!(enc.encode(&mut syms));
    let d = data_symbols(&syms, &m, true, false, 0);
    let mut bb = Vec::new();
    FrameDecoder::default().decode(&m, true, &d, 0.01, &mut bb).unwrap();
    bb_descramble(&mut bb);
    assert_eq!(bb.len(), 7032 / 8);
    let h = BbHeader::parse(&bb).unwrap();
    assert_eq!((usize::from(h.dfl), h.syncd, h.ext), (8 * data, 0, 2));
    assert!(bb[BBHEADER_LEN + data..].iter().all(|&b| b == 0), "the padding is zero");
    let mut got = Vec::new();
    UpDeframer::new()
        .push_frame(&bb, &mut |p| got.push(p.bytes.to_vec()))
        .unwrap();
    assert_eq!(got.len(), 2, "the third packet has no CRC-8 yet");
    assert_eq!((&got[0][..], &got[1][..]), (&sent[0][..], &sent[1][..]));
}

/// A dummy PLFRAME: the header of MODCOD 0 and 36 slots of the unmodulated carrier under the PL scrambling.
#[test]
fn a_dummy_plframe_is_a_header_and_36_slots() {
    let enc = S2Encoder::new(ModCod::new(13).unwrap(), false, true, 9);
    let mut syms = Vec::new();
    enc.dummy(&mut syms);
    assert_eq!(syms.len(), 90 + 36 * 90);
    assert_eq!(syms[..90], header_symbols(code7(0, false, false))[..]);
    let mut scr = Vec::new();
    pl_scrambling(9, 36 * 90, &mut scr);
    for (s, &r) in syms[90..].iter().zip(&scr) {
        assert!((quarter(*s, (4 - r) & 3) - PILOT).norm() < 1e-6);
    }
}

/// The useful bit rate: the worked example of the specification, QPSK 2/3 in normal frames at 1 Msym/s carries
/// 42 960 bits in 32 490 symbols, 1.3222 Mbit/s, and 1.2908 Mbit/s with pilots (33 282 symbols).
#[test]
fn the_useful_bit_rate_counts_the_header_and_the_pilots() {
    let m = ModCod::new(6).unwrap();
    let r = useful_bit_rate(1e6, &m, false, false).unwrap();
    assert!((r - 42_960.0 / 32_490.0 * 1e6).abs() < 1e-6 && (r - 1.3222e6).abs() < 100.0);
    let r = useful_bit_rate(1e6, &m, false, true).unwrap();
    assert!((r - 42_960.0 / 33_282.0 * 1e6).abs() < 1e-6 && (r - 1.2908e6).abs() < 100.0);
    assert!(useful_bit_rate(1e6, &ModCod::new(11).unwrap(), true, false).is_none());
}

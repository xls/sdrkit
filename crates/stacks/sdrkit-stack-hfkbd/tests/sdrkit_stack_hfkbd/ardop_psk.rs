//! ARDOP PSK and QAM data frames: the six worked examples and the short check vectors of the signal specification (bytes in,
//! blocks, check values and symbols out), the decisions and the combining of repeats on measured values, and each frame type
//! sent, put through noise and a tuning error, and read back by the stack's receiver.

use crate::common::*;
use crate::tx_common::*;
use hf::ardop::body::{self, Body, Repeats};
use hf::ardop::tx::{session_id, FecType, FrameWriter};
use hf::ardop::{frame_check, frame_type, type_check, type_symbols, RsCodes, TxFrame};
use sdrkit_core::Complex32;
use sdrkit_stack_hfkbd as hf;

/// One carrier of a worked example: its count, CRC register, block and symbol digits.
struct Carrier {
    count: usize,
    crc: u16,
    block: &'static str,
    digits: &'static str,
}

/// A worked example: the payload, the type code and session id, the ten type symbols and the carriers.
struct Example {
    payload: fn() -> Vec<u8>,
    code: u8,
    session: u8,
    type_symbols: [u8; 10],
    carriers: &'static [Carrier],
}

const EXAMPLES: &[Example] = &[
    Example {
        payload: || b"HELLO".to_vec(),
        code: 0x42, session: 0xFF, type_symbols: [1, 0, 0, 2, 2, 2, 3, 3, 1, 2], carriers: &[
    Carrier { count: 5, crc: 0xBE60, block: "0548454C4C4F0000000000000000000000BE2287D55C6CB219F0AB", digits: "002466224460220022002206666666666666666666666666666666666666666666662062266266064602464462004226604642226264" },
]},
    Example {
        payload: || (0..70u8).collect(),
        code: 0x61, session: 0x9B, type_symbols: [1, 2, 0, 1, 3, 3, 3, 2, 2, 3], carriers: &[
    Carrier { count: 64, crc: 0x092F, block: "40000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F202122232425262728292A2B2C2D2E2F303132333435363738393A3B3C3D3E3F094E3445F2EE00BE86C3D1D0CFC09911658EC102A84B839B888990258EB9C3A8E118", digits: "222222222224444000066600002444622242226666244404440666444424442666422444466002266006602224600240024224000260026224066066606002044642266662244004400662444024402662422622262440400406620004244062264220000660066220066466646006044242204442600626640664222060064006422260220442446602066204266666206266042220600202220064222260466002402400620002222626224406222046204400440260000460442620464442626640022400" },
    Carrier { count: 6, crc: 0x8B40, block: "06404142434445000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008B21EBD378BADCF899D9A510C8695E24AB71E2D2D3A8DBC2BF190CF80D2958237088", digits: "002600002224666244424466002444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444440042266062642442426620406066426624020260402446664400262460622600404242242662022646640400604200040642240222006400006004024622266464440044" },
    Carrier { count: 0, crc: 0x2CE0, block: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002C8175DAF27BC3D2644A0C14D63120EA525F86AC852370316CA22C4F697B44E6226D", digits: "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000422666020242404200464064442022604660040006660220240066004442626022602062240406622466220200006602644044004224420262464060022046226624060" },
    Carrier { count: 0, crc: 0x2CE0, block: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002C8175DAF27BC3D2644A0C14D63120EA525F86AC852370316CA22C4F697B44E6226D", digits: "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000422666020242404200464064442022604660040006660220240066004442626022602062240406622466220200006602644044004224420262464060022046226624060" },
]},
    Example {
        payload: || b"ABC".to_vec(),
        code: 0x44, session: 0xFF, type_symbols: [1, 0, 1, 0, 1, 2, 3, 2, 3, 1], carriers: &[
    Carrier { count: 3, crc: 0xAF20, block: "03414243000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000AF64BB64D35047247DF98B256075E3DF4CB57CBA5F54E3F5D3A3314EC1A40A37396929D0400C", digits: "00622771331111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111243040531714613773626254762014560667521105412630217102431603721616751451376112700055323131516751155562" },
]},
    Example {
        payload: || (0..120u32).map(|i| ((7 * i + 1) % 256) as u8).collect(),
        code: 0x53, session: 0x9B, type_symbols: [1, 1, 0, 3, 2, 3, 0, 2, 0, 2], carriers: &[
    Carrier { count: 108, crc: 0x044E, block: "6C01080F161D242B323940474E555C636A71787F868D949BA2A9B0B7BEC5CCD3DAE1E8EFF6FD040B121920272E353C434A51585F666D747B828990979EA5ACB3BAC1C8CFD6DDE4EBF2F900070E151C232A31383F464D545B626970777E858C939AA1A8AFB6BDC4CBD2D9E0E7EE041D9201133AF23F803D5F54E33704F1042B0C4294C7CAE549FD4EEE3FF55D18BEBF0FF5F367", digits: "36666233364500301224153564622332473027261175231253321775147024727746206630721441723010354475141074106226606700345557263127142336026330335420341616652002603446073271316273252556467321317617141437437333342211300771263562473321340427157531230140072764014424615302205525472430670410242031140761557727373337145307007622253032412015322343377346442335167365607026210572754432160537227636512105070510" },
    Carrier { count: 12, crc: 0xB9B0, block: "0CF5FC030A11181F262D343B42000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000B9E35032149E022EEEDAFBED48B76A276C8DB063C99A390612745842DD6D5AC34B27D98C092F", digits: "03435437775522455334373125724436004444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444504372400344626511227547416103257371761371416261477047572300700156706242267252502747724646103403337307" },
]},
    Example {
        payload: || vec![0x9A, 0x13, 0x51, 0x41, 0x4D],
        code: 0x46, session: 0xFF, type_symbols: [1, 0, 1, 2, 3, 2, 3, 2, 1, 3], carriers: &[
    Carrier { count: 5, crc: 0x7B00, block: "059A1351414D0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007B468FE419D56BC38734C37BC7EF72EB3F2DE0A97711AE098C0A1DD9ECA5A1D4E6F64D4FC8325DE5AC999646D5889032DF648C20F46807A0484A1183A51F1A4A130B", digits: "05E8141267380000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007A64CB956FC17AE19037B658C39871FA5C6B91BC3234EC4DD91B49EFD9B0A384A0F51E29DD027CA79DEF8620D2AAB360DC26EA44B75D54E62A6812A5F45C5F3D611C" },
]},
    Example {
        payload: || (0..130u32).map(|i| (0xA5 ^ (3 * i)) as u8).collect(),
        code: 0x55, session: 0x9B, type_symbols: [1, 1, 1, 1, 1, 3, 0, 3, 2, 1], carriers: &[
    Carrier { count: 128, crc: 0xBB88, block: "80A5A6A3ACA9AAB7B0BDBEBB8481828F889596939C999AE7E0EDEEEBF4F1F2FFF8C5C6C3CCC9CAD7D0DDDEDB2421222F283536333C393A07000D0E0B1411121F186566636C696A77707D7E7B4441424F485556535C595AA7A0ADAEABB4B1B2BFB88586838C898A97909D9E9BE4E1E2EFE8F5F6F3FCF9FAC7C0CDCECBD4D1D2DFD8BBDD2465B5E2E138B1E96A657B7317E39D94360C9A1BB37438B85884FC7710DCBE100991B20F1F45CFA69F21536A3B18DA5F7BC1427649FF5061CF3F107EB712ED78", digits: "80A79794EACDF9C3E69E9FADD192A4CBBBC1A094D9ABCEC391FCA8E984B4B5CBAAE3F5948C89DFC380DAFDAD7356024B5D0506147B6F2C4333380E692670134B4C2753142E4D3D43221E5B2D1512604B7F4164141D2B0AC3D5FCECE9C0B4F1CBEEE3B194C8899BC3C4DAB9ADB7D6C6CB9985C294BFEFE8C3F7B8CAE9E2F0D7CB88BEB82641C1F1F03BE7DE4E410B2565B6FCD1422EF92D832619CC1995C87677C8B9222BC582292952EDF5ED7050683E7FCE3A1C815764098F4423FE18110E90139E5D" },
    Carrier { count: 2, crc: 0x0450, block: "022526000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040514C2440D28366344CAE5BE671EFCDE5AAC6539447D9D89DAF56C6021C2A287188A05744BB1DBEE2A09E2AB1D0859FF46F37BC8E0D55FD7305F64ADCFC8DA7C08", digits: "024131111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111155237B5155A4C753626ACA7A8656CBFCA79BF525E265AB889E8F42E4467B5F190199B30737AD6BECA4E6FD79C5A2A78FE20F21C88E6B05C90330F51B8CBFFCE5919" },
]},
];

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn digit_string(d: &[u8]) -> String {
    d.iter().map(|v| format!("{v:X}")).collect()
}

/// The six worked examples: the type symbols, the block of every carrier, its CRC register and sent CRC bytes, the
/// Reed-Solomon check equation, the symbol digits, and the symbol count of the code table.
#[test]
fn ardop_psk_worked_examples() {
    let rs = &mut RsCodes::new();
    for e in EXAMPLES {
        let ft = frame_type(e.code).unwrap();
        let b = Body::of(&ft).unwrap();
        assert_eq!(
            type_symbols(e.code, e.session),
            e.type_symbols,
            "{:#x}",
            e.code
        );
        let f = TxFrame::data_bytes(rs, e.code, e.session, &(e.payload)());
        assert_eq!(
            f.body.len(),
            ft.block_len() * e.carriers.len(),
            "{:#x}",
            e.code
        );
        for (c, (blk, want)) in f.body.chunks(ft.block_len()).zip(e.carriers).enumerate() {
            let at = format!("{:#x} carrier {c}", e.code);
            assert_eq!(blk, hex(want.block).as_slice(), "{at}");
            assert_eq!(usize::from(blk[0]), want.count, "{at}");
            let crc = frame_check(&blk[..=ft.data]);
            assert_eq!(crc, want.crc, "{at}");
            assert_eq!(blk[ft.data + 1], (crc >> 8) as u8, "{at}");
            assert_eq!(blk[ft.data + 2], (crc as u8) ^ e.code, "{at}");
            let mut w = blk.to_vec();
            assert_eq!(rs.correct(&mut w, ft.rs), Some(0), "{at}");
            let d = body::digits(b, blk);
            assert_eq!(d.len(), body::symbols(&ft), "{at}");
            assert_eq!(digit_string(&d), want.digits, "{at}");
        }
    }
    // E1 with the default leader and trailer lasts 1560 ms.
    let e1 = TxFrame::data(rs, 0x42, 0xFF, "HELLO").with_leader(240);
    assert!((e1.air_s() - 1.56).abs() < 1e-9);
    let mut w = FrameWriter::new(RATE, 0.5, false);
    w.push_frame(&e1);
    assert_eq!(w.end_sample(), (1.56 * RATE).round() as u64);
    // The symbol counts of the code table.
    for (code, n) in [
        (0x40, 396),
        (0x42, 108),
        (0x44, 392),
        (0x46, 390),
        (0x74, 390),
        (0x63, 392),
    ] {
        assert_eq!(body::symbols(&frame_type(code).unwrap()), n, "{code:#x}");
    }
}

/// The short check vectors: the CRC-16, the Reed-Solomon parity, the type parity and the session ids; and samples 0 to 9 of
/// the 1500 Hz phase start symbol, rounded, times 1.2 and truncated.
#[test]
fn ardop_psk_check_vectors() {
    let d: Vec<u8> = (1..=12).collect();
    assert_eq!(frame_check(&d), 0xD58C);
    assert_eq!(frame_check(&[0]), 0x95C0);
    assert_eq!(frame_check(&[]), 0xFFFF);
    assert_eq!(RsCodes::new().parity(&d, 4), [0x2B, 0xEA, 0x74, 0xA3]);
    let p: Vec<u8> = [0x30, 0x00, 0xFF, 0x4A]
        .iter()
        .map(|&b| type_check(b))
        .collect();
    assert_eq!(p, [2, 1, 1, 0]);
    assert_eq!(session_id("N0CALL", "N0CALL-1"), 0x9B);
    assert_eq!(session_id("N0CALL-1", "N0CALL"), 0xC6);
    assert_eq!(session_id("n0call-0", "N0CALL-1"), 0x9B);
    let s: Vec<i64> = (0..10)
        .map(|n| ((26_000.0 * body::carrier_wave(1500.0, 0, f64::from(n)).0).round() * 1.2) as i64)
        .collect();
    assert_eq!(s, [0, 582, 1646, 1746, 0, -2904, -4921, -4053, 0, 5192]);
}

/// The measured values of a frame of `code` carrying `data` with no noise: each carrier at the magnitude 1 on the full ring,
/// 0.5 on the half ring, every phase step turned by `turn_deg`.
fn ideal(code: u8, data: &[u8], turn_deg: f32) -> Vec<Vec<Complex32>> {
    let ft = frame_type(code).unwrap();
    let b = Body::of(&ft).unwrap();
    let f = TxFrame::data_bytes(&mut RsCodes::new(), code, 0xFF, data);
    f.body
        .chunks(ft.block_len())
        .map(|blk| {
            let mut v = vec![Complex32::new(1.0, 0.0)];
            for (k, d) in body::digits(b, blk).into_iter().enumerate() {
                let r = if d & 8 != 0 { 0.5 } else { 1.0 };
                let a = (f32::from(d & 7) * 45.0 + turn_deg * (k + 1) as f32).to_radians();
                v.push(Complex32::from_polar(r, a));
            }
            v
        })
        .collect()
}

/// Decisions on measured values: every modulation reads its frame from ideal values turned by a residual tuning error, with
/// the quality near 100. Over repeats a carrier found good stays good and the others are read from the circular mean of
/// their steps; a change of the type code forgets the copies.
#[test]
fn ardop_psk_decisions_and_repeats() {
    let rs = &mut RsCodes::new();
    let data: Vec<u8> = (0..200u32).map(|i| (i * 37 + 11) as u8).collect();
    for code in [
        0x40u8, 0x42, 0x44, 0x46, 0x50, 0x52, 0x54, 0x60, 0x62, 0x64, 0x70, 0x72, 0x74,
    ] {
        let ft = frame_type(code).unwrap();
        let n = data.len().min(ft.capacity());
        let r = Repeats::new().read(&ft, rs, &ideal(code, &data[..n], 3.0));
        assert!(r.good, "{code:#x}");
        assert_eq!(r.data, &data[..n], "{code:#x}");
        assert!(r.quality >= 98, "{code:#x}: {}", r.quality);
    }
    // 8PSK.500.100, two carriers. Every other symbol of a carrier turned by `deg`: its steps are off by ±deg.
    let ft = frame_type(0x52).unwrap();
    let good = ideal(0x52, &data, 0.0);
    let turned = |c: usize, deg: f32| {
        let mut x = good.clone();
        for z in x[c][1..].iter_mut().step_by(2) {
            *z *= Complex32::from_polar(1.0, deg.to_radians());
        }
        x
    };
    // Copy one: carrier 1 off by 40°, carrier 0 good and kept. Copy two: carrier 0 off by 115°, carrier 1 clean; the mean
    // of carrier 1 is off by 20°, inside the 22.5° margin.
    let mut rep = Repeats::new();
    let r1 = rep.read(&ft, rs, &turned(1, 40.0));
    assert!(!r1.good);
    assert_eq!(r1.good_carriers, 1);
    let r2 = rep.read(&ft, rs, &turned(0, 115.0));
    assert!(r2.good, "{r2:?}");
    assert_eq!(r2.data, &data[..200]);
    // The same two copies with the odd code second: the memory is forgotten and carrier 0 is lost.
    let mut rep = Repeats::new();
    rep.read(&ft, rs, &turned(1, 40.0));
    let other = frame_type(0x53).unwrap();
    let r3 = rep.read(&other, rs, &turned(0, 115.0));
    assert!(!r3.good);
    assert_eq!(r3.good_carriers, 0);
}

/// Every PSK and QAM frame type sent by the frame writer, its tuning off by 4 Hz, with white noise at `snr_db` in 2.5 kHz,
/// over `seeds`: the receiver reads the frame type and the whole payload in every case.
fn loop_case(code: u8, snr_db: f64, seeds: std::ops::Range<u64>) {
    let ft = frame_type(code).unwrap();
    let data: Vec<u8> = (0..ft.capacity() as u32 - 5)
        .map(|i| (i * 29 + code as u32) as u8)
        .collect();
    let f = TxFrame::data_bytes(&mut RsCodes::new(), code, 0xFF, &data).with_leader(240);
    let mut w = FrameWriter::new(RATE, 0.5, false);
    w.push_frame(&f);
    let mut x = vec![Complex32::default(); w.end_sample() as usize];
    w.fill(&mut x);
    let step = std::f64::consts::TAU * 4.0 / RATE;
    for (n, z) in x.iter_mut().enumerate() {
        *z *= Complex32::from_polar(1.0, (step * n as f64) as f32);
    }
    let want: String = data
        .iter()
        .map(|&c| {
            if (32..127).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect();
    for seed in seeds {
        let r = feed(&harness(&hf::ARDOP), &noisy(&x, snr_db, 1.0, seed));
        let rows = rows(&r, "frames");
        let name = &ft.name;
        let row = rows
            .iter()
            .find(|row| &cell_text(row, "name") == name)
            .unwrap_or_else(|| panic!("{name} seed {seed}: no frame"));
        assert_eq!(cell_flag(row, "body_ok"), Some(true), "{name} seed {seed}");
        assert_eq!(cell_text(row, "text"), want, "{name} seed {seed}");
        assert_eq!(num(&r, "data_ok"), 1.0, "{name} seed {seed}");
    }
}

#[test]
fn ardop_psk_loop_4psk() {
    for code in [0x40, 0x42, 0x50, 0x60, 0x70] {
        loop_case(code, 15.0, 1..3);
    }
}

#[test]
fn ardop_psk_loop_8psk() {
    for code in [0x44, 0x52, 0x62, 0x72] {
        loop_case(code, 20.0, 1..3);
    }
}

#[test]
fn ardop_psk_loop_16qam() {
    for code in [0x46, 0x54, 0x64, 0x74] {
        loop_case(code, 25.0, 1..3);
    }
}

/// The test signal with a 16QAM.500.100 last frame, at 25 dB in 2.5 kHz with a 20 Hz tuning error and a 50 ppm clock error:
/// the two 4FSK data frames and the 16QAM frame are read whole.
#[test]
fn ardop_psk_test_signal() {
    let h = snr_2500(harness(&hf::ARDOP), 25.0)
        .set(&hf::ardop::GEN_DATA, hf::ardop::GenData::Qam16_500_100)
        .set(&GEN_OFFSET_HZ, 20.0)
        .set(&GEN_CLOCK_PPM, 50.0)
        .seconds(29.0);
    let r = run(&h);
    let rows = rows(&r, "frames");
    let row = rows
        .iter()
        .find(|row| cell_text(row, "name") == "16QAM.500.100.E")
        .expect("the 16QAM frame");
    assert_eq!(cell_flag(row, "body_ok"), Some(true));
    assert_eq!(cell_text(row, "text"), hf::ardop::GEN_TEXT);
    assert_eq!(num(&r, "data_ok"), 3.0);
}

/// The one burst the stack sends for `text` with the settings `set`; it ends with its end mark.
fn ardop_burst(set: &[(&str, sdrkit_core::Value)], text: &str) -> Vec<Complex32> {
    use sdrkit_core::{Owner, Value};
    let (mut rx, sink) = station(&hf::ARDOP, set, None);
    send(rx.as_mut(), text).expect("the text is sent");
    for _ in 0..600 {
        idle(rx.as_mut(), 0.1);
        let res = Owner::results(rx.as_ref());
        let id = res.decl().field_id("tx_sending").expect("the field");
        if !matches!(res.fields().value(id), Some(Value::Bool(true)))
            && field_num(res, "tx_waiting") == 0.0
        {
            break;
        }
    }
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "one burst");
    assert!(b[0].ended, "the end mark");
    b.remove(0).samples
}

/// The `send` command with a PSK or QAM FEC type: a text longer than one frame goes as frames of that type, even and odd in
/// turn, and the stack's own receiver reads the whole text back at 20 dB in 2.5 kHz.
#[test]
fn ardop_psk_send_command() {
    use sdrkit_core::{Choice as _, Value};
    let text: String = (0..300).map(|i| (b'A' + (i % 26) as u8) as char).collect();
    for t in [FecType::Psk4_500_100, FecType::Qam16_1000_100] {
        let x = ardop_burst(&[("fec_type", Value::Choice(t.index()))], &text);
        let r = feed(&harness(&hf::ARDOP), &noisy(&x, 20.0, 2.0, 7));
        let mut rows = rows(&r, "frames");
        rows.sort_by(|a, b| cell_num(a, "time_s").total_cmp(&cell_num(b, "time_s")));
        let ft = frame_type(t.code()).unwrap();
        let base = ft.name.trim_end_matches(".E");
        assert_eq!(rows.len(), text.len().div_ceil(ft.capacity()), "{t:?}");
        for (i, row) in rows.iter().enumerate() {
            let want = format!("{base}.{}", if i % 2 == 0 { "E" } else { "O" });
            assert_eq!(cell_text(row, "name"), want, "{t:?}");
            assert_eq!(cell_flag(row, "body_ok"), Some(true), "{t:?} {i}");
        }
        let got: String = rows.iter().map(|row| cell_text(row, "text")).collect();
        assert_eq!(got, text, "{t:?}");
    }
}

/// The samples of the frame writer at 12 kHz: E1's phase start symbol (2400 samples after the first type sample of a 240 ms
/// leader) is the specification's samples 0 to 9 within 2 units at the leader peak 28 600; every sample of the first 40 data
/// symbols of E4 is the sum over its two carriers of the symbol formula at the digits of the example, carrier 0 at 1400 Hz,
/// times 0.65.
#[test]
fn ardop_psk_writer_samples() {
    let level = 0.5;
    let unit = 28_600.0 / level;
    let write = |f: &TxFrame| {
        let mut w = FrameWriter::new(RATE, level as f32, false);
        w.push_frame(f);
        let mut x = vec![Complex32::default(); w.end_sample() as usize];
        assert_eq!(w.fill(&mut x), x.len());
        x
    };
    let body_start = 12 * 240 + 2400;
    let rs = &mut RsCodes::new();
    let x = write(&TxFrame::data(rs, 0x42, 0xFF, "HELLO").with_leader(240));
    let want = [0, 582, 1646, 1746, 0, -2904, -4921, -4053, 0, 5192];
    for (n, w) in want.iter().enumerate() {
        let got = f64::from(x[body_start + n].re) * unit;
        assert!((got - f64::from(*w)).abs() < 2.0, "sample {n}: {got} {w}");
    }
    let e4 = &EXAMPLES[3];
    let x = write(&TxFrame::data_bytes(rs, e4.code, e4.session, &(e4.payload)()).with_leader(240));
    let digits: Vec<Vec<u8>> = e4
        .carriers
        .iter()
        .map(|c| {
            c.digits
                .bytes()
                .map(|b| (b as char).to_digit(16).unwrap() as u8)
                .collect()
        })
        .collect();
    let mut worst = 0.0f64;
    for (k, pair) in digits[0].iter().zip(&digits[1]).take(40).enumerate() {
        let a = body_start + 120 * (k + 1);
        for n in 0..120 {
            let (mut re, mut im) = (0.0, 0.0);
            for (hz, d) in [(1400.0, *pair.0), (1600.0, *pair.1)] {
                let (r, i) = body::carrier_wave(hz, d, n as f64);
                re += r * 0.65 * 26_000.0;
                im += i * 0.65 * 26_000.0;
            }
            let z = x[a + n];
            worst = worst.max((f64::from(z.re) * unit - re).hypot(f64::from(z.im) * unit - im));
        }
    }
    assert!(worst < 1.0, "{worst}");
}

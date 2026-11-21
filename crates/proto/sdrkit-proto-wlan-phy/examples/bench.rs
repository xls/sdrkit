//! Times the receivers and the coding chain: `cargo run --release -p sdrkit-proto-wlan-phy --example bench`. Each line is the best of
//! several runs, in millions of input items per second (samples, bytes or frames as the name says).

use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_proto_wlan_phy::{ble, dsss, mac, ofdm, Complex32};
use std::hint::black_box;
use std::time::Instant;

fn measure(name: &str, items: usize, mut step: impl FnMut()) {
    step();
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut runs = 0;
    while runs < 5 || start.elapsed().as_secs_f64() < 0.5 {
        let t = Instant::now();
        step();
        best = best.min(t.elapsed().as_secs_f64());
        runs += 1;
    }
    println!("{name:48} {:10.2} M/s", items as f64 / best / 1e6);
}

fn noise(x: &mut [Complex32], power: f64, seed: u64) {
    sdrkit_algo_sim::noise::Awgn::with_power(power, seed)
        .unwrap()
        .process(x);
}

fn beacon(len: usize) -> Vec<u8> {
    let mut f = Vec::new();
    mac::push_mgmt_header(
        &mut f,
        mac::MgmtSubtype::Beacon,
        mac::BROADCAST,
        [2; 6],
        [2; 6],
        1,
    );
    mac::push_beacon_fields(&mut f, 0, 100, 1);
    while f.len() < len {
        f.push(f.len() as u8);
    }
    mac::push_fcs(&mut f);
    f
}

fn main() {
    // BLE: 20 advertising packets at 4 Msps with gaps.
    let mut x = Vec::new();
    for i in 0..20u8 {
        let mut data = Vec::new();
        ble::ad::push(&mut data, ble::ad::COMPLETE_NAME, b"bench node").unwrap();
        let mut pdu = Vec::new();
        ble::build_adv(
            &mut pdu,
            ble::PduType::AdvInd,
            &ble::Addr::public([i; 6]),
            &data,
        )
        .unwrap();
        let mut bits = Vec::new();
        ble::air_bits(&pdu, 37, &mut bits).unwrap();
        let levels: Vec<f32> = bits.iter().map(|&b| 2.0 * b as f32 - 1.0).collect();
        let mut m = Cpm::gfsk(0.5, 0.5, 4.0).unwrap();
        m.push(&levels, &mut x);
        m.flush(&mut x);
        x.extend(std::iter::repeat_n(Complex32::default(), 4000));
    }
    noise(&mut x, 0.05, 1);
    let mut rx = ble::AdvReceiver::new(4).unwrap();
    measure("ble receiver, 4 Msps (samples)", x.len(), || {
        rx.process(black_box(&x), |p| {
            black_box(p.pdu);
        })
    });

    // 802.11b: 1 Mbit/s frames at 22 Msps.
    let psdu = beacon(200);
    let h = dsss::PlcpHeader::for_psdu(dsss::Rate::Dbpsk1, psdu.len()).unwrap();
    let mut bits = Vec::new();
    dsss::ppdu_bits(dsss::Preamble::Long, &h, &psdu, &mut bits).unwrap();
    dsss::scramble(&mut bits, dsss::Preamble::Long);
    let mut sym = Vec::new();
    dsss::symbols(&bits, dsss::Preamble::Long, dsss::Rate::Dbpsk1, &mut sym).unwrap();
    let mut chips = Vec::new();
    dsss::spread(&sym, &mut chips);
    let mut y: Vec<Complex32> = chips.iter().flat_map(|&c| [c, c]).collect();
    y.extend(std::iter::repeat_n(Complex32::default(), 20_000));
    noise(&mut y, 0.1, 2);
    let mut drx = dsss::DsssReceiver::new(22).unwrap();
    measure("dsss receiver, 22 Msps (samples)", y.len(), || {
        drx.process(black_box(&y), |f| {
            black_box(f.mpdu);
        })
    });

    // 802.11a: 54 and 6 Mbit/s frames at 20 Msps.
    for r in [ofdm::Rate::R6, ofdm::Rate::R54] {
        let mut z = Vec::new();
        ofdm::build_ppdu(&beacon(1500), r, 0x5D, &mut z).unwrap();
        z.extend(std::iter::repeat_n(Complex32::default(), 2000));
        noise(&mut z, 1e-3, 3);
        let mut orx = ofdm::OfdmReceiver::new();
        measure(
            &format!("ofdm receiver, {} Mbit/s frames (samples)", r.mbps()),
            z.len(),
            || {
                orx.process(black_box(&z), |f| {
                    black_box(f.mpdu);
                })
            },
        );
    }
    let psdu = beacon(1500);
    let mut coded = Vec::with_capacity(20_000);
    measure(
        "ofdm encode_data, 54 Mbit/s (PSDU bytes)",
        psdu.len(),
        || {
            coded.clear();
            ofdm::encode_data(black_box(&psdu), ofdm::Rate::R54, 0x5D, &mut coded).unwrap();
        },
    );

    // MAC and BLE parsing.
    let f = beacon(300);
    measure("mac frame parse and element walk (frames)", 1, || {
        let fr = mac::Frame::parse_with_fcs(black_box(&f)).unwrap();
        black_box(mac::elements(fr.body).count());
    });
}

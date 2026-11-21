//! Times the lookups: `cargo run --release -p sdrkit-util-ids --example bench`.

use sdrkit_util_ids::names::{IdKind, IdNames};
use sdrkit_util_ids::tones::{ctcss, dcs};
use sdrkit_util_ids::{bluetooth, icao, language, mcc, mmsi, oui};
use std::hint::black_box;
use std::time::Instant;

fn bench(name: &str, mut f: impl FnMut(u32)) {
    const N: u32 = 2_000_000;
    for i in 0..10_000 {
        f(i);
    }
    let t = Instant::now();
    for i in 0..N {
        f(i);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / N as f64;
    println!("{name:40} {ns:8.1} ns per call");
}

fn main() {
    bench("icao::country", |i| {
        black_box(icao::country(black_box(i.wrapping_mul(2_654_435_761) >> 8)));
    });
    bench("icao::registration (US)", |i| {
        black_box(icao::registration(black_box(0xA0_0001 + i % 0xD_F7C7)));
    });
    bench("icao::address (N-number)", |_| {
        black_box(icao::address(black_box("N628TS")));
    });
    bench("mmsi::classify", |_| {
        black_box(mmsi::classify(black_box("232123456")));
    });
    let plmn = mcc::Plmn::new(262, 1, 2).unwrap();
    bench("mcc::operator", |_| {
        black_box(mcc::operator(black_box(plmn)));
    });
    bench("oui::vendor", |i| {
        black_box(oui::vendor(black_box([
            (i >> 16) as u8,
            (i >> 8) as u8,
            i as u8,
        ])));
    });
    bench("bluetooth::company", |i| {
        black_box(bluetooth::company(black_box(i as u16)));
    });
    bench("language::iso639", |_| {
        black_box(language::iso639(black_box("deu")));
    });
    let w = dcs::Dcs::from_octal("023").unwrap().word();
    bench("dcs::decode (46 trials)", |_| {
        black_box(dcs::decode(black_box(w)));
    });
    bench("ctcss::nearest", |i| {
        black_box(ctcss::nearest(black_box(60.0 + (i % 200) as f32), 1.2));
    });
    let mut names = IdNames::new(IdKind::Mmsi);
    let text: String = (0..10_000)
        .map(|k| format!("{}\tShip {k}\n", 232_000_000 + k))
        .collect();
    names.import(&text);
    bench("IdNames::get_number (10,000 entries)", |i| {
        black_box(names.get_number(black_box(232_000_000 + (i % 20_000) as u64)));
    });
    let t = Instant::now();
    let (n, _) = IdNames::from_text(IdKind::Mmsi, &text);
    println!(
        "{:40} {:8.1} ms for {} rows",
        "IdNames::from_text",
        t.elapsed().as_secs_f64() * 1e3,
        n.len()
    );
}

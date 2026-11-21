//! Times the stages of the 1090 MHz chain on noise and on the default airspace (a tool for tuning).
use sdrkit_core::{Complex32, Generator};
use std::time::Instant;

fn main() {
    let rate = 4e6;
    let mut g = sdrkit_stack_adsb::generator::AdsbGenerator::airspace(rate, Default::default(), 1);
    let mut x = vec![Complex32::new(0.0, 0.0); 4_000_000];
    g.generate(&mut x);
    let t = Instant::now();
    let mut d = sdrkit_stack_adsb::frontend::PulseDcRemoval::new(rate);
    let mut y = Vec::new();
    d.run(&x, &mut y);
    println!("dc {:?}", t.elapsed());
    let t = Instant::now();
    let mut dm = sdrkit_stack_adsb::demod::Demod::new(rate, 0.0);
    let mut out = Vec::new();
    for c in y.chunks(65536) {
        dm.feed(c, &mut out);
    }
    println!("demod {:?} ({} candidates)", t.elapsed(), out.len());
    use sdrkit_core::{Receiver, SettingSet};
    let schema = sdrkit_stack_adsb::settings::AdsbSetting::schema();
    let mut v = sdrkit_core::Values::defaults(&schema);
    v.update_from_json(&serde_json::json!({"effort": "2"}))
        .unwrap();
    let mut rx = sdrkit_stack_adsb::receiver::receiver(&sdrkit_core::StackConfig::new(v)).unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(rate, 1090e6, 1090e6).recording())
        .unwrap();
    let t = Instant::now();
    let mut o = sdrkit_core::Output::default();
    for c in x.chunks(65536) {
        rx.feed(c, &mut o);
    }
    println!("receiver {:?}", t.elapsed());
}

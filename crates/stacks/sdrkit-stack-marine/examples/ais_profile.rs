//! Times the AIS receiver at 2 Msps on the port demo, with and without collision handling, and on noise alone.

use sdrkit_components::sim::{GenSetting, NoiseKind};
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Choice, Complex32, Generator, InputInfo, Output, SettingSet, StackConfig, Value, Values,
};
use sdrkit_stack_marine::ais::generator::{DemoOptions, PortDemo, Render};

fn time(x: &[Complex32], cc: bool) -> (f64, String) {
    let d = &sdrkit_stack_marine::AIS;
    let schema = (d.settings)();
    let mut v = Values::defaults(&schema);
    v.set(schema.id("collision_cancel").unwrap(), &Value::Bool(cc))
        .unwrap();
    let mut rx = (d.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(2e6, 162e6, 162e6).recording())
        .unwrap();
    let mut out = Output::default();
    let t = std::time::Instant::now();
    for c in x.chunks(65_536) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let good = rx.results().fields().get(&keys::FRAMES_OK).unwrap_or(0);
    (t.elapsed().as_secs_f64(), good.to_string())
}

fn main() {
    let rate = 2e6;
    let g = PortDemo::new(DemoOptions {
        render: Render {
            rate,
            duration_s: 0.0,
            ..Default::default()
        },
        ..Default::default()
    });
    // Noise at 30 dB in 48 kHz, as the host's impairments add it.
    let schema = GenSetting::schema();
    let mut v = Values::defaults(&schema);
    let mut set = |k: &str, x: Value| v.set(schema.id(k).unwrap(), &x).unwrap();
    set("gen_noise", Value::Choice(NoiseKind::Snr.index()));
    set("gen_snr_db", Value::F64(30.0));
    set("gen_level_dbfs", Value::F64(-12.0));
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &v).unwrap();
    let mut x = vec![Complex32::default(); (rate * 4.0) as usize];
    g.generate(&mut x);
    let secs = 4.0;
    for cc in [true, false] {
        let (t, good) = time(&x, cc);
        println!(
            "collision handling {cc}: {:.1} % of one core, {good} messages",
            t / secs * 100.0
        );
    }
    let mut n = sdrkit_dev_test::Awgn::with_sigma(0.01, 1);
    let y = n.noise(x.len());
    println!(
        "noise only: {:.1} % of one core",
        time(&y, true).0 / secs * 100.0
    );
}

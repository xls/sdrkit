//! Draw-list, id, rate and gain-plan vectors.

use super::*;
use crate::tests::{hex, REAL_MENU_HEX};

const S: &str = "Airspy Source";

fn real() -> Menu {
    let b = hex(REAL_MENU_HEX);
    assert_eq!(b.len(), 591);
    Menu::parse(&b).expect("readable")
}

#[test]
fn the_real_menu_reads_as_expected() {
    let m = real();
    let src = m.server_source().unwrap();
    assert_eq!(
        (src.entries.clone(), src.selected, src.label.as_str()),
        (vec!["Airspy".to_string()], 0, "Server source")
    );
    assert_eq!(m.source(), Some("Airspy"));
    let dev = m.get(&format!("##_airspy_dev_sel_{S}")).unwrap();
    assert_eq!(dev.kind, Kind::List);
    assert_eq!(dev.entries, ["258C62DC2D43588F"]);
    assert_eq!(dev.label, "Device");
    let sr = m.rate_list().unwrap();
    assert_eq!(sr.id, format!("##_airspy_sr_sel_{S}"));
    assert_eq!(m.rates(), [10e6, 2.5e6]);
    assert_eq!(sr.selected, 0);
    let radios: Vec<_> = m
        .controls
        .iter()
        .filter(|c| c.kind == Kind::Radio)
        .map(|c| (c.visible(), c.on, c.label.as_str()))
        .collect();
    assert_eq!(
        radios,
        [
            ("Sensitive", false, "Gain mode"),
            ("Linear", true, "Gain mode"),
            ("Free", false, "Gain mode")
        ]
    );
    let g = m.get(&format!("##_airspy_lin_gain_{S}")).unwrap();
    assert_eq!(
        (g.kind, g.value, g.min, g.max, g.label.as_str(), g.disabled),
        (Kind::SliderInt, 18.0, 0.0, 21.0, "Gain", false)
    );
    let b = m.get(&format!("Bias T##_airspy_{S}")).unwrap();
    assert_eq!(
        (b.kind, b.on, b.label.as_str()),
        (Kind::CheckBox, false, "Bias T")
    );
    // The Refresh button and the columns give no control: 3 lists, 3 radio buttons, a slider and a check box.
    assert_eq!(m.controls.len(), 8);
    assert!(m
        .controls
        .iter()
        .all(|c| !c.id.contains("Refresh") && !c.id.contains("Columns")));
    let plan = m.gain_plan().unwrap();
    assert_eq!(plan.sliders.len(), 1);
    assert_eq!(plan.sliders[0].id, g.id);
    assert_eq!((plan.lo, plan.hi, plan.drivable()), (0.0, 21.0, true));
}

#[test]
fn broken_lists_are_unreadable() {
    let b = hex(REAL_MENU_HEX);
    assert_eq!(Menu::parse(&b[..b.len() - 3]), Err(Unreadable));
    assert_eq!(Menu::parse(&hex("09 01 02 03")), Err(Unreadable));
    assert_eq!(Menu::parse(&[]), Ok(Menu::default()));
    // The first element must be a step.
    assert_eq!(Menu::parse(&hex("01 01")), Err(Unreadable));
}

#[test]
fn small_lists() {
    let m = Menu::parse(&hex(
        "00 8b 00 04 13 00 42 69 61 73 20 54 23 23 78 5f 62 69 61 73 74 5f 53 72 63 01 01",
    ))
    .unwrap();
    assert_eq!(m.controls.len(), 1);
    let c = &m.controls[0];
    assert_eq!(
        (c.kind, c.on, c.id.as_str(), c.label.as_str()),
        (Kind::CheckBox, true, "Bias T##x_biast_Src", "Bias T")
    );
    assert_eq!(name_from_id(&c.id), "Bias tee");
    let m = Menu::parse(&hex("00 8b 00 04 01 00 78 01 01")).unwrap();
    assert_eq!(
        (
            m.controls[0].id.as_str(),
            m.controls[0].label.as_str(),
            m.controls[0].on
        ),
        ("x", "x", true)
    );
}

/// A small draw-list writer.
#[derive(Default)]
struct W(Vec<u8>);
impl W {
    fn step(mut self, code: u8) -> Self {
        self.0.extend([0, code, 0]);
        self
    }
    fn s(mut self, t: &str) -> Self {
        self.0.push(4);
        self.0.extend((t.len() as u16).to_le_bytes());
        self.0.extend(t.as_bytes());
        self
    }
    fn i(mut self, v: i32) -> Self {
        self.0.push(2);
        self.0.extend(v.to_le_bytes());
        self
    }
    fn f(mut self, v: f32) -> Self {
        self.0.push(3);
        self.0.extend(v.to_le_bytes());
        self
    }
    fn b(mut self, v: bool) -> Self {
        self.0.extend([1, v as u8]);
        self
    }
    fn combo(self, id: &str, entries: &str) -> Self {
        self.step(step::COMBO).s(id).i(0).s(entries).i(-1)
    }
    fn slider(self, id: &str, v: i32, lo: i32, hi: i32) -> Self {
        self.step(step::SLIDER_INT).s(id).i(v).i(lo).i(hi).i(0).i(0)
    }
    fn menu(self) -> Menu {
        Menu::parse(&self.0).unwrap()
    }
}

#[test]
fn disabled_pairs_and_left_labels() {
    let m = W::default()
        .step(step::BEGIN_DISABLED)
        .combo("##a_x_y", "1\0")
        .step(step::END_DISABLED)
        .combo("##a_z_y", "1\0")
        .step(step::END_DISABLED)
        .step(step::END_DISABLED)
        .step(step::BEGIN_DISABLED)
        .combo("##a_w_y", "1\0")
        .menu();
    let d: Vec<bool> = m.controls.iter().map(|c| c.disabled).collect();
    assert_eq!(d, [true, false, true]);
    let m = W::default()
        .step(step::LEFT_LABEL)
        .s("LNA Gain")
        .step(step::FILL_WIDTH)
        .slider("##_x_lna_gain_S", 1, 0, 15)
        .menu();
    assert_eq!(m.controls[0].label, "LNA Gain");
    let m = W::default()
        .step(step::LEFT_LABEL)
        .s("LNA Gain")
        .step(step::BUTTON)
        .s("Go")
        .f(0.0)
        .f(0.0)
        .slider("##_x_lna_gain_S", 1, 0, 15)
        .menu();
    assert_eq!(m.controls.len(), 1);
    assert_eq!(m.controls[0].label, "LNA gain");
}

#[test]
fn steps_with_missing_or_wrong_arguments_are_skipped() {
    let m = W::default()
        .step(step::SLIDER_INT)
        .s("##a_b")
        .i(1)
        .i(0)
        .step(step::CHECKBOX)
        .s("##a_c")
        .i(1)
        .step(0x95)
        .i(1)
        .s("x")
        .step(step::CHECKBOX)
        .s("ok##a")
        .b(true)
        .i(77)
        .step(step::TEXT_COLORED)
        .f(1.0)
        .f(1.0)
        .f(1.0)
        .f(1.0)
        .s("Status: ok")
        .s("")
        .step(step::SLIDER_FLOAT_WITH_STEPS)
        .s("##q_gain_Q")
        .f(1.5)
        .f(0.0)
        .f(10.0)
        .f(0.5)
        .i(0)
        .step(step::INPUT_INT)
        .s("##q_ppm_Q")
        .i(3)
        .i(0)
        .i(10)
        .i(0)
        .menu();
    assert_eq!(m.controls.len(), 4);
    assert_eq!(m.controls[0].id, "ok##a");
    assert_eq!(m.texts(), ["Status: ok"]);
    let f = &m.controls[2];
    assert_eq!((f.kind, f.step, f.value), (Kind::SliderFloat, 0.5, 1.5));
    let p = &m.controls[3];
    assert_eq!(
        (p.kind, p.step, p.value, p.label.as_str()),
        (Kind::InputInt, 1.0, 3.0, "PPM")
    );
}

#[test]
fn combo_entries_split_at_nul() {
    assert_eq!(split_entries("10.0MHz\x002.5MHz\0"), ["10.0MHz", "2.5MHz"]);
    assert_eq!(split_entries("a\0b"), ["a", "b"]);
    assert!(split_entries("").is_empty());
    assert!(split_entries("\0").is_empty());
}

#[test]
fn names_and_words_from_ids() {
    assert_eq!(words("_airspy_lin_gain_Airspy Source"), ["lin", "gain"]);
    assert_eq!(words("_airspy_sr_sel_Airspy Source"), ["sr", "sel"]);
    assert_eq!(words("x_biast_Src"), ["biast"]);
    assert_eq!(words("_x_SR_y"), ["SR", "y"]);
    let n = name_from_id;
    assert_eq!(n("##_airspy_dev_sel_Airspy Source"), "Device");
    assert_eq!(n("##_airspy_lin_gain_Airspy Source"), "Linear gain");
    assert_eq!(n("Linear##_airspy_gm_Airspy Source"), "Gain mode");
    assert_eq!(n("##x_biast_Src"), "Bias tee");
    assert_eq!(n("##_airspy_unknownword_Name"), "Unknownword");
    assert_eq!(n("##_airspy_lna_gain_Airspy Source"), "LNA gain");
    assert_eq!(n("##_rtlsdr_sr_RTL Source"), "Sample rate");
    assert_eq!(n("##_ofs_Foo_bar"), "Foo bar");
    assert_eq!(n("##__a__b__"), "B");
    assert_eq!(n("##"), "");
    assert_eq!(n(SERVER_SOURCE_ID), "Server source");
    let c = |id: &str| {
        let mut c = Control::new(Kind::List, id.into(), String::new(), false);
        c.entries = vec!["1".into()];
        c
    };
    assert!(c("##_x_SR_y").carries("sr"));
    assert!(c("##_AIRSPY_bw_z").carries("bw"));
    assert!(c("##sens_if_x").carries("if"));
    assert!(!c("##sens_if_x").carries("sens"));
}

#[test]
fn rate_texts() {
    let cases: &[(&str, f64)] = &[
        ("10.0MHz", 10e6),
        ("2.5MHz", 2.5e6),
        ("250KHz", 250e3),
        ("2.048 MSPS", 2.048e6),
        ("48000", 48e3),
        ("5 ks/s", 5e3),
        ("1.5GHz", 1.5e9),
        ("Auto", 0.0),
        ("8 MHz BW", 0.0),
        ("5 kk", 0.0),
        ("0", 0.0),
        ("-3", 0.0),
        ("", 0.0),
        ("inf", 0.0),
        ("nan", 0.0),
        ("  1e3 hz", 1e3),
    ];
    for (t, v) in cases {
        assert_eq!(rate_text(t), *v, "{t:?}");
    }
}

#[test]
fn the_rate_list_is_found_by_its_words() {
    // A bandwidth list before the rate list, both all rates: the rate list wins by its words.
    let m = W::default()
        .combo("##_x_bw_S", "1MHz\0")
        .combo("##_x_sr_S", "2MHz\0")
        .menu();
    assert_eq!(m.rate_list().unwrap().id, "##_x_sr_S");
    // No words: the first that is not a bandwidth.
    let m = W::default()
        .combo("##_x_bw_S", "1MHz\0")
        .combo("##_x_rates_S", "2MHz\0")
        .menu();
    assert_eq!(m.rate_list().unwrap().id, "##_x_rates_S");
    // Only a bandwidth list, a list with a non-rate entry, the server-source list: none.
    let m = W::default()
        .combo("##_x_bw_S", "1MHz\0")
        .combo("##_x_sr_S", "2MHz\0Auto\0")
        .combo(SERVER_SOURCE_ID, "48000\0")
        .menu();
    assert!(m.rate_list().is_none());
}

#[test]
fn gain_plans_and_targets() {
    let free = W::default()
        .slider("##_airspy_lna_gain_S", 0, 0, 15)
        .slider("##_airspy_mix_gain_S", 0, 0, 15)
        .slider("##_airspy_vga_gain_S", 0, 0, 15)
        .slider("##_airspy_fixed_gain_S", 3, 3, 3)
        .menu();
    let p = free.gain_plan().unwrap();
    assert_eq!((p.sliders.len(), p.lo, p.hi), (3, 0.0, 45.0));
    let t: Vec<f64> = p
        .sliders
        .iter()
        .map(|c| gain_target(c, 22.5, 0.0, 45.0))
        .collect();
    assert_eq!(t, [8.0, 8.0, 8.0]);
    let t: Vec<f64> = p
        .sliders
        .iter()
        .map(|c| gain_target(c, 45.0, 0.0, 45.0))
        .collect();
    assert_eq!(t, [15.0, 15.0, 15.0]);
    let lin = real();
    let p = lin.gain_plan().unwrap();
    let g = |x| gain_target(p.sliders[0], x, p.lo, p.hi);
    assert_eq!((g(9.6), g(99.0), g(-5.0)), (10.0, 21.0, 0.0));
    // Scores: linear beats sensitive beats a plain gain.
    let m = W::default()
        .slider("##_x_rf_gain_S", 0, 0, 10)
        .slider("##_x_sens_gain_S", 0, 0, 10)
        .slider("##_x_lin_gain_S", 0, 0, 10)
        .menu();
    assert_eq!(m.gain_plan().unwrap().sliders[0].id, "##_x_lin_gain_S");
    let m = W::default()
        .slider("##_x_rf_gain_S", 0, 0, 10)
        .slider("##_x_sens_gain_S", 0, 0, 10)
        .menu();
    assert_eq!(m.gain_plan().unwrap().sliders[0].id, "##_x_sens_gain_S");
    // A float slider with steps snaps.
    let m = W::default()
        .step(step::SLIDER_FLOAT_WITH_STEPS)
        .s("##_x_gain_S")
        .f(0.0)
        .f(0.0)
        .f(10.0)
        .f(0.5)
        .i(0)
        .menu();
    let p = m.gain_plan().unwrap();
    assert_eq!(gain_target(p.sliders[0], 3.3, p.lo, p.hi), 3.5);
    // Not drivable when every slider is disabled; no plan without sliders.
    let m = W::default()
        .step(step::BEGIN_DISABLED)
        .slider("##_x_gain_S", 0, 0, 10)
        .menu();
    assert!(!m.gain_plan().unwrap().drivable());
    assert!(W::default()
        .combo("##a", "x\0")
        .menu()
        .gain_plan()
        .is_none());
}

#[test]
fn the_real_menu_as_settings() {
    let m = real();
    let s = server_settings(&m);
    let keys: Vec<&str> = s.iter().map(|s| s.decl.field.key.as_ref()).collect();
    assert_eq!(
        keys,
        [
            "server.##sdrpp_server_src_sel",
            "server.##_airspy_dev_sel_Airspy Source",
            "server.##_airspy_gm_Airspy Source",
            "server.Bias T##_airspy_Airspy Source",
        ]
    );
    let gm = &s[2].decl;
    assert_eq!(gm.field.label, "Gain mode");
    assert_eq!(gm.default, Value::Choice(1));
    let choices: Vec<&str> = gm.field.choices.iter().map(|c| c.key.as_ref()).collect();
    assert_eq!(choices, ["Sensitive", "Linear", "Free"]);
    assert_eq!(
        s[2].target.action(&Value::Choice(2)),
        Some(("Free##_airspy_gm_Airspy Source", ActionValue::Bool(true)))
    );
    assert_eq!(s[0].decl.field.label, "Server source");
    assert_eq!(s[1].decl.field.label, "Device");
    let devices: Vec<&str> = s[1]
        .decl
        .field
        .choices
        .iter()
        .map(|c| c.label.as_ref())
        .collect();
    assert_eq!(devices, ["258C62DC2D43588F"]);
    assert_eq!(
        s[1].target.action(&Value::Choice(0)),
        Some(("##_airspy_dev_sel_Airspy Source", ActionValue::Int(0)))
    );
    assert_eq!(
        (s[3].decl.field.label.as_ref(), &s[3].decl.default),
        ("Bias T", &Value::Bool(false))
    );
    assert!(s
        .iter()
        .all(|x| x.decl.section == "Server" && !x.decl.restart));
    // A slider that is not in the gain plan is offered, with six significant digits.
    let m = W::default()
        .step(step::SLIDER_FLOAT)
        .s("Offset##_x_off_S")
        .f(1.234_567_8)
        .f(10.0)
        .f(0.0)
        .i(0)
        .i(0)
        .menu();
    let s = server_settings(&m);
    assert_eq!(s[0].decl.default, Value::F64(1.23457));
    let r = s[0].decl.range.unwrap();
    assert!(r.min == 0.0 && r.max == 10.0 && (r.step - 0.1).abs() < 1e-9);
    assert_eq!(
        s[0].target.action(&Value::F64(2.5)),
        Some(("Offset##_x_off_S", ActionValue::Float(2.5)))
    );
    assert_eq!(s[0].target.action(&Value::I64(2)), None);
}

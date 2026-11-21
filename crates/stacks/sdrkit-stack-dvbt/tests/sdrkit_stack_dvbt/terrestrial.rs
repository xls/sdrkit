//! The DVB-T2 / DVB-T stack that the scanner uses: with a DVB-T2 receiver registered, every new channel starts the alternation
//! again with DVB-T2, whatever locked on the last channel.

use sdrkit_core::owner::Owner;
use sdrkit_core::SettingSet;
use sdrkit_core::{
    Complex32, ConfigError, InputInfo, Output, Receiver, Results, StackConfig, Values,
};
use sdrkit_stack_dvbt::results::Decoding;
use sdrkit_stack_dvbt::typed::DvbTerrestrialFields;
use sdrkit_stack_dvbt::{TerrestrialReceiver, TerrestrialSetting};

/// A DVB-T2 receiver that never locks.
struct NoT2(Results);

impl Owner for NoT2 {
    fn results(&self) -> &Results {
        &self.0
    }
}

impl Receiver for NoT2 {
    fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
        Ok(())
    }
    fn feed(&mut self, _: &[Complex32], _: &mut Output) {}
}

#[test]
fn a_retune_starts_the_alternation_with_dvbt2() {
    assert!(sdrkit_stack_dvbt::terrestrial::set_t2_factory(|_| Ok(
        Box::new(NoT2(Results::default()))
    )));
    let cfg = StackConfig::new(Values::defaults(&TerrestrialSetting::schema()));
    let mut rx = TerrestrialReceiver::new(&cfg).unwrap();
    let rate = 8e6;
    rx.configure(&InputInfo::new(rate, 529.5e6, 529.5e6).recording())
        .unwrap();
    assert_eq!(rx.decoding(), Decoding::Dvbt2);
    let x = vec![Complex32::default(); 16384];
    let mut out = Output::default();
    // 1.6 s of DVB-T2, then DVB-T.
    let mut t = 0.0;
    while t < 1.7 {
        rx.feed(&x, &mut out);
        out.clear();
        t += x.len() as f64 / rate;
    }
    assert_eq!(rx.decoding(), Decoding::Dvbt);
    assert_eq!(
        rx.results().fields().at(&DvbTerrestrialFields::STANDARD),
        Some(Decoding::Dvbt)
    );
    rx.retuned(&InputInfo::new(rate, 536.5e6, 536.5e6).recording());
    assert_eq!(rx.decoding(), Decoding::Dvbt2);
    assert!(rx
        .results()
        .log_lines()
        .iter()
        .any(|l| l == "trying DVB-T2"));
}

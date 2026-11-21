//! The cost file of a family of stacks. Each stack records what each of its stages costs in calibration units per
//! second of signal ([`crate::calibrate`]), per variant. The unit does not depend on the machine that measured. The engine reads the cost before a session starts. The numbers come from
//! [`measure`], which runs the stack on its test signal and reads the work counters of its graph: the same counters
//! that a running session shows. Nobody writes the file by hand.

use sdrkit_core::{
    Complex32, CostRow, CostTable, GenConfig, InputInfo, Output, Receiver, StackConfig,
    StackDescriptor, Values,
};
use std::path::Path;

/// Seconds of test signal that are fed before the measurement starts, and that are measured.
const WARM_S: f64 = 1.0;
const MEASURED_S: f64 = 3.0;

/// Runs `stack` in `variant` (a submode, or `-`) on its test signal at its own rate and returns the cost of each stage.
/// `prepare` sets the stack up before it is configured, for example with the command that selects a service.
///
/// # Errors
///
/// A text that names the step that failed: the stack has no test signal, or it refuses its own rate.
pub fn measure(
    stack: &StackDescriptor,
    variant: &str,
    prepare: impl FnOnce(&mut dyn Receiver),
) -> Result<Vec<CostRow>, String> {
    let submode = (variant != "-").then(|| variant.to_owned());
    let rate = stack.tuning.rate_hz;
    let settings = Values::defaults(&(stack.settings)());
    let make = stack
        .make_generator
        .ok_or_else(|| format!("{}: no test signal", stack.id))?;
    let gen_cfg = GenConfig::new(rate, settings.clone()).with_submode_opt(submode.clone());
    let mut gen = make(&gen_cfg).map_err(|e| format!("{}: {e}", stack.id))?;
    let cfg = StackConfig::new(settings).with_submode_opt(submode);
    let mut rx = (stack.make_receiver)(&cfg).map_err(|e| format!("{}: {e}", stack.id))?;
    prepare(rx.as_mut());
    let hz = stack.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, hz, hz).recording())
        .map_err(|e| format!("{}: {e}", stack.id))?;
    let mut block = vec![Complex32::default(); 16_384];
    let mut out = Output::default();
    let mut run = |rx: &mut dyn Receiver, seconds: f64| {
        let mut left = (seconds * rate) as usize;
        while left > 0 {
            let n = gen.generate(&mut block).min(left).max(1);
            out.clear();
            rx.feed(&block[..n.min(block.len())], &mut out);
            left = left.saturating_sub(n);
        }
    };
    let (mut before, mut after) = (Vec::new(), Vec::new());
    run(rx.as_mut(), WARM_S);
    let no_stages = || format!("{}: the receive chain declares no stages", stack.id);
    rx.staged().ok_or_else(no_stages)?.stage_loads(&mut before);
    run(rx.as_mut(), MEASURED_S);
    rx.staged().ok_or_else(no_stages)?.stage_loads(&mut after);
    // The speed of this machine in the same run turns the measured share of a core into calibration units.
    let core = crate::calibrate::units_per_second(std::time::Duration::from_millis(30));
    Ok(after
        .iter()
        .zip(&before)
        .map(|(a, b)| CostRow {
            stack: stack.id.to_owned(),
            variant: variant.to_owned(),
            stage: a.name.clone(),
            units: core * (a.busy_ns - b.busy_ns) as f64 / 1e9 / (a.signal_s - b.signal_s).max(1e-9),
        })
        .collect())
}

/// Compares the cost file at `path` with the measured cost of `stack` in `variant`. The file must hold one row for each
/// stage and no other row of the variant. An optimised build also compares the numbers: a recorded share that is more
/// than `factor` above or below the measured one is stale. A recorded cost that is more than `factor` above or below the measured one is stale.
/// A build with debug assertions is several times slower than the shipped one, so it compares the rows only. With `SDRKIT_WRITE_COST=1`, an optimised build writes the rows of the
/// variant into the file and keeps the other rows.
///
/// # Errors
///
/// A text that names the row that is missing, extra or stale.
pub fn check(
    stack: &StackDescriptor,
    variant: &str,
    path: &Path,
    factor: f64,
    prepare: impl FnOnce(&mut dyn Receiver),
) -> Result<(), String> {
    let measured = measure(stack, variant, prepare)?;
    let optimised = !cfg!(debug_assertions);
    let core = crate::calibrate::units_per_second(std::time::Duration::from_millis(30));
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let recorded = CostTable::parse(&text);
    let ours = |r: &&CostRow| r.stack == stack.id && r.variant == variant;
    if optimised && std::env::var_os("SDRKIT_WRITE_COST").is_some_and(|v| v == "1") {
        let mut rows: Vec<CostRow> = recorded.rows().iter().filter(|r| !ours(r)).cloned().collect();
        rows.extend(measured);
        return std::fs::write(path, CostTable::from_rows(rows).render())
            .map_err(|e| format!("{}: {e}", path.display()));
    }
    let hint = "run the test in a release build with SDRKIT_WRITE_COST=1";
    for m in &measured {
        let Some(units) = recorded.units(&m.stack, &m.variant, &m.stage) else {
            return Err(format!(
                "{} {} {}: no recorded cost; {hint}",
                m.stack, m.variant, m.stage
            ));
        };
        // Below a hundredth of a core of this machine the two numbers are noise.
        let stale = optimised
            && units.max(m.units) > 0.01 * core
            && (units > m.units * factor || units * factor < m.units);
        if stale {
            return Err(format!(
                "{} {} {}: recorded {units:.1}, measured {:.1}, more than a factor of {factor} apart; {hint}",
                m.stack, m.variant, m.stage, m.units
            ));
        }
    }
    if let Some(r) = recorded
        .rows()
        .iter()
        .filter(ours)
        .find(|r| !measured.iter().any(|m| m.stage == r.stage))
    {
        return Err(format!(
            "{} {} {}: a recorded cost of a stage that the stack does not have; {hint}",
            r.stack, r.variant, r.stage
        ));
    }
    Ok(())
}

//! What a scan will do, said before it starts: its kind, the channels or blocks of one pass, the dwell on each, the time of
//! one pass and the passes in the total time. For a survey also its stage one, and the time that span blocks would take,
//! which decides between the two.

use crate::service::ScanKind;

/// The plan of a scan. Times are seconds; a time that does not apply is 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScanPlan {
    /// The kind of scan.
    pub kind: ScanKind,
    /// The channels, or the blocks, of one pass (for a survey: of stage one).
    pub steps: usize,
    /// The time on one channel or block, after the settle time.
    pub dwell_s: f64,
    /// The time of one pass. For an activity scan the least time: a stop on activity adds to it. For a survey the time of
    /// stage one: each candidate visit adds `visit_s`.
    pub pass_s: f64,
    /// The total time. 0: the scan runs until it is stopped, or ends after its passes.
    pub total_s: f64,
    /// The passes in the total time, or the passes set. 0: until the scan is stopped.
    pub passes: f64,
    /// A survey: the time of one candidate visit (settle and dwell).
    pub visit_s: f64,
    /// A survey: the time of one pass of span blocks over the range, which the rule compares with the total time.
    pub span_pass_s: f64,
}

impl Default for ScanPlan {
    /// No scan.
    fn default() -> Self {
        ScanPlan {
            kind: ScanKind::None,
            steps: 0,
            dwell_s: 0.0,
            pass_s: 0.0,
            total_s: 0.0,
            passes: 0.0,
            visit_s: 0.0,
            span_pass_s: 0.0,
        }
    }
}

/// The passes of `pass_s` in `total_s`; 0 without a total time.
fn passes_in(total_s: f64, pass_s: f64) -> f64 {
    if total_s > 0.0 && pass_s > 0.0 {
        total_s / pass_s
    } else {
        0.0
    }
}

impl ScanPlan {
    /// An activity scan of `steps` channels with `settle_s` and `dwell_s` on each, and `passes` (0 until stopped).
    pub fn activity(steps: usize, settle_s: f64, dwell_s: f64, passes: u32) -> ScanPlan {
        let pass_s = steps as f64 * (settle_s + dwell_s);
        ScanPlan {
            kind: ScanKind::Activity,
            steps,
            dwell_s,
            pass_s,
            total_s: pass_s * f64::from(passes),
            passes: f64::from(passes),
            ..ScanPlan::default()
        }
    }

    /// A channel scan of `steps` channels, each settled and tested for at most `test_s`: one pass.
    pub fn channels(steps: usize, settle_s: f64, test_s: f64) -> ScanPlan {
        let pass_s = steps as f64 * (settle_s + test_s);
        ScanPlan {
            kind: ScanKind::Channels,
            steps,
            dwell_s: test_s,
            pass_s,
            total_s: pass_s,
            passes: 1.0,
            ..ScanPlan::default()
        }
    }

    /// A span-block scan of `steps` blocks with `settle_s` and `dwell_s` on each, over `total_s`.
    pub fn span(steps: usize, settle_s: f64, dwell_s: f64, total_s: f64) -> ScanPlan {
        let pass_s = steps as f64 * (settle_s + dwell_s);
        ScanPlan {
            kind: ScanKind::Span,
            steps,
            dwell_s,
            pass_s,
            total_s,
            passes: passes_in(total_s, pass_s),
            span_pass_s: pass_s,
            ..ScanPlan::default()
        }
    }

    /// A survey of the range: stage one in `look_blocks` blocks of `look_s` each, visits of `settle_s` and `dwell_s`;
    /// `span` is the plan of span blocks over the same range. The survey runs span blocks when one pass of them fits into
    /// the total time, or, without a total time, when it is not longer than one pass of stage one.
    pub fn survey(
        look_blocks: usize,
        look_s: f64,
        settle_s: f64,
        dwell_s: f64,
        span: ScanPlan,
    ) -> ScanPlan {
        let pass_s = look_blocks as f64 * (settle_s + look_s);
        let total_s = span.total_s;
        let fits = if total_s > 0.0 {
            span.pass_s <= total_s
        } else {
            span.pass_s <= pass_s
        };
        if fits && span.steps > 0 {
            return span;
        }
        ScanPlan {
            kind: ScanKind::Survey,
            steps: look_blocks,
            dwell_s: look_s,
            pass_s,
            total_s,
            passes: passes_in(total_s, pass_s),
            visit_s: settle_s + dwell_s,
            span_pass_s: span.pass_s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_range_takes_span_blocks_and_a_wide_one_a_survey() {
        // 200 kHz in blocks of 48 kHz with 10 s each: 5 blocks, 51 s a pass, about 70 passes in an hour.
        let span = ScanPlan::span(5, 0.2, 10.0, 3600.0);
        assert!((span.pass_s - 51.0).abs() < 1e-9);
        assert!((span.passes - 3600.0 / 51.0).abs() < 1e-9);
        let narrow = ScanPlan::survey(1, 2.0, 0.2, 10.0, span);
        assert_eq!(narrow.kind, ScanKind::Span);
        // 70 to 450 MHz: 7917 span blocks take 22 hours a pass; 43 blocks of 9 MHz take 95 s.
        let span = ScanPlan::span(7917, 0.2, 10.0, 3600.0);
        let wide = ScanPlan::survey(43, 2.0, 0.2, 10.0, span);
        assert_eq!(wide.kind, ScanKind::Survey);
        assert!((wide.pass_s - 43.0 * 2.2).abs() < 1e-9);
        assert!((wide.passes - 3600.0 / (43.0 * 2.2)).abs() < 1e-9);
        assert!((wide.visit_s - 10.2).abs() < 1e-9);
        assert!((wide.span_pass_s - 7917.0 * 10.2).abs() < 1e-6);
    }
}

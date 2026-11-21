//! Very slow Morse (QRSS and DFCW) read from runs of a key state when the element length is not known.
//!
//! A receiver that measures the key state on windows of half an element lengthens each mark and shortens each gap by about
//! [`BIAS`] of an element. [`refine`] fits the element length to the runs by least squares with that bias, [`error`] scores how
//! well the runs fit whole elements, and [`elements`] turns the runs into dots, dashes and gaps. [`codes`] groups them into the
//! dot-dash codes of letters. The letters themselves come from a Morse table.

/// The share of an element by which a half-element window lengthens marks and shortens gaps.
pub const BIAS: f64 = 0.13;

/// How the key is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keying {
    /// On-off keying of one carrier: a dot is one element on, a dash three; gaps of 1, 3 and 7 elements.
    Qrss,
    /// Two frequencies: a dot is one element on the dot frequency, a dash one element on the dash frequency; gaps of 1 (letter)
    /// and 3 (word) elements.
    Dfcw,
}

/// A key state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// No carrier.
    Up,
    /// The carrier on (QRSS).
    Down,
    /// The lower of the two frequencies (DFCW).
    Lower,
    /// The upper of the two frequencies (DFCW).
    Upper,
}

/// A stretch of one key state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    /// The state.
    pub key: Key,
    /// Its length, seconds.
    pub seconds: f64,
}

/// What a run means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    /// A dot.
    Dot,
    /// A dash.
    Dash,
    /// The end of a letter.
    LetterEnd,
    /// The end of a word.
    WordEnd,
}

/// A letter's code, or a space between words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// The elements of a letter: the first in bit `len − 1`, 1 for a dash.
    Letter {
        /// The elements.
        bits: u16,
        /// Their number.
        len: u8,
    },
    /// A space.
    Space,
}

fn sign(key: Key) -> f64 {
    if key == Key::Up {
        -1.0
    } else {
        1.0
    }
}

/// The run in elements of length `t`, its bias taken off.
fn units(r: &Run, t: f64, bias: f64) -> f64 {
    (r.seconds - sign(r.key) * bias * t) / t
}

/// The whole number of elements a run of `u` elements stands for.
fn whole(keying: Keying, key: Key, u: f64) -> f64 {
    match (keying, key) {
        (Keying::Qrss, Key::Up) => {
            if u < 2.0 {
                1.0
            } else if u < 5.0 {
                3.0
            } else {
                7.0
            }
        }
        (Keying::Qrss, _) => {
            if u < 2.0 {
                1.0
            } else {
                3.0
            }
        }
        (Keying::Dfcw, _) => u.round().max(1.0),
    }
}

/// The element length fitted to `runs` from `t0` seconds: six rounds of a least-squares fit of each run to its whole number of
/// elements plus the bias. Returns `t0` when the runs give no fit.
pub fn refine(runs: &[Run], keying: Keying, t0: f64, bias: f64) -> f64 {
    let mut t = t0;
    for _ in 0..6 {
        let (mut num, mut den) = (0.0, 0.0);
        for r in runs {
            let k = whole(keying, r.key, units(r, t, bias));
            let a = k + sign(r.key) * bias;
            num += r.seconds * a;
            den += a * a;
        }
        if den <= 0.0 || !(num / den).is_finite() || num <= 0.0 {
            return t0;
        }
        t = num / den;
    }
    t
}

/// How badly `runs` fit whole elements of `t` seconds: the mean over the runs of the relative distance to the nearest allowed
/// length (each at most 1), plus 0.5 for DFCW when the run at the 1/6 quantile is not one element long, plus 0.0005 × `t`.
pub fn error(runs: &[Run], keying: Keying, t: f64, bias: f64) -> f64 {
    if runs.is_empty() || t <= 0.0 {
        return f64::MAX;
    }
    let mut e = 0.0;
    for r in runs {
        let u = units(r, t, bias);
        let d = match (keying, r.key) {
            (Keying::Qrss, Key::Up) => [1.0, 3.0, 7.0]
                .iter()
                .map(|&k: &f64| (u - k).abs() / k)
                .fold(f64::MAX, f64::min),
            (Keying::Qrss, _) => [1.0, 3.0]
                .iter()
                .map(|&k: &f64| (u - k).abs() / k)
                .fold(f64::MAX, f64::min),
            (Keying::Dfcw, _) => {
                let k = if u < 0.6 { 1.0 } else { u.round().max(1.0) };
                (u - k).abs() / k
            }
        };
        e += d.min(1.0);
    }
    e /= runs.len() as f64;
    if keying == Keying::Dfcw {
        let mut u: Vec<f64> = runs.iter().map(|r| units(r, t, bias)).collect();
        u.sort_by(f64::total_cmp);
        let q = u[(u.len() - 1) / 6];
        if q.round() != 1.0 {
            e += 0.5;
        }
    }
    e + 0.0005 * t
}

/// The meaning of each run at element length `t`. With `dash_below` a DFCW dash is on the lower frequency.
pub fn elements(runs: &[Run], keying: Keying, t: f64, bias: f64, dash_below: bool) -> Vec<Element> {
    let mut out = Vec::new();
    for r in runs {
        let u = units(r, t, bias);
        match (keying, r.key) {
            (Keying::Qrss, Key::Up) => {
                if u >= 5.0 {
                    out.push(Element::WordEnd);
                } else if u >= 2.0 {
                    out.push(Element::LetterEnd);
                }
            }
            (Keying::Qrss, _) => out.push(if u < 2.0 { Element::Dot } else { Element::Dash }),
            (Keying::Dfcw, Key::Up) => out.push(if u.round() >= 3.0 {
                Element::WordEnd
            } else {
                Element::LetterEnd
            }),
            (Keying::Dfcw, key) => {
                let k = u.round().max(1.0) as usize;
                let dash = (key == Key::Upper) != dash_below;
                let e = if dash { Element::Dash } else { Element::Dot };
                out.extend(std::iter::repeat_n(e, k));
            }
        }
    }
    out
}

/// The letters and word spaces of `elements`. A letter of more than 16 elements is cut.
pub fn codes(elements: &[Element]) -> Vec<Symbol> {
    let mut out = Vec::new();
    let (mut bits, mut len) = (0u16, 0u8);
    let flush = |out: &mut Vec<Symbol>, bits: &mut u16, len: &mut u8| {
        if *len > 0 {
            out.push(Symbol::Letter {
                bits: *bits,
                len: *len,
            });
        }
        (*bits, *len) = (0, 0);
    };
    for e in elements {
        match e {
            Element::Dot | Element::Dash => {
                if len < 16 {
                    bits = (bits << 1) | u16::from(*e == Element::Dash);
                    len += 1;
                }
            }
            Element::LetterEnd => flush(&mut out, &mut bits, &mut len),
            Element::WordEnd => {
                flush(&mut out, &mut bits, &mut len);
                if !matches!(out.last(), None | Some(Symbol::Space)) {
                    out.push(Symbol::Space);
                }
            }
        }
    }
    flush(&mut out, &mut bits, &mut len);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The runs of "CQ" in QRSS at `t` s elements, with the window's bias, without the first and last gaps.
    fn qrss_cq(t: f64) -> Vec<Run> {
        // C: -.-.  Q: --.-
        let pattern: &[(bool, f64)] = &[
            (true, 3.0),
            (false, 1.0),
            (true, 1.0),
            (false, 1.0),
            (true, 3.0),
            (false, 1.0),
            (true, 1.0),
            (false, 3.0),
            (true, 3.0),
            (false, 1.0),
            (true, 3.0),
            (false, 1.0),
            (true, 1.0),
            (false, 1.0),
            (true, 3.0),
        ];
        pattern
            .iter()
            .map(|&(on, k)| Run {
                key: if on { Key::Down } else { Key::Up },
                seconds: (k + if on { BIAS } else { -BIAS }) * t,
            })
            .collect()
    }

    #[test]
    fn qrss_runs_give_the_element_length_and_the_letters() {
        let runs = qrss_cq(6.0);
        let t = refine(&runs, Keying::Qrss, 5.0, BIAS);
        assert!((t - 6.0).abs() < 0.01, "{t}");
        assert!(error(&runs, Keying::Qrss, t, BIAS) < 0.01);
        assert!(error(&runs, Keying::Qrss, 3.0, BIAS) > 0.1);
        let c = codes(&elements(&runs, Keying::Qrss, t, BIAS, false));
        assert_eq!(
            c,
            vec![
                Symbol::Letter {
                    bits: 0b1010,
                    len: 4
                },
                Symbol::Letter {
                    bits: 0b1101,
                    len: 4
                }
            ]
        );
    }

    #[test]
    fn dfcw_runs_count_elements_by_length() {
        // "TE": T is one dash (upper), a letter gap, E one dot (lower); then a word gap and T again.
        let t = 6.0;
        let runs = [
            Run {
                key: Key::Upper,
                seconds: t,
            },
            Run {
                key: Key::Up,
                seconds: t,
            },
            Run {
                key: Key::Lower,
                seconds: 2.0 * t,
            },
            Run {
                key: Key::Up,
                seconds: 3.0 * t,
            },
            Run {
                key: Key::Upper,
                seconds: t,
            },
        ];
        let e = elements(&runs, Keying::Dfcw, t, 0.0, false);
        assert_eq!(
            codes(&e),
            vec![
                Symbol::Letter { bits: 1, len: 1 },
                Symbol::Letter { bits: 0, len: 2 },
                Symbol::Space,
                Symbol::Letter { bits: 1, len: 1 }
            ]
        );
        let below = codes(&elements(&runs, Keying::Dfcw, t, 0.0, true));
        assert_eq!(below[0], Symbol::Letter { bits: 0, len: 1 });
        assert!((refine(&runs, Keying::Dfcw, 5.5, 0.0) - t).abs() < 1e-9);
    }
}

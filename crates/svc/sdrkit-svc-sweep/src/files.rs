//! The text files of a sweep: the rtl_power layout, the four-trace layout and the signal log.

use crate::channels::{Channel, ChannelType};
use crate::grid::{Grid, UNCOVERED_DB};
use sdrkit_core::Choice;
use sdrkit_util_geo::time::Civil;
use std::io::{self, Write};

/// Writes a sweep in the rtl_power layout: one line per piece of at most `per_line` bins, `date, time, low Hz, high Hz,
/// step Hz, samples, dB, dB, ...`, every piece with the same UTC date and time of `unix_s`. `samples` is the number of
/// samples behind each value.
pub fn write_rtl_power<W: Write>(
    w: &mut W,
    unix_s: f64,
    grid: &Grid,
    sweep_db: &[f32],
    samples: u64,
    per_line: usize,
) -> io::Result<()> {
    let c = Civil::from_unix_f64(unix_s.floor())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let per = per_line.max(1);
    let n = grid.bins.min(sweep_db.len());
    let mut a = 0;
    while a < n {
        let b = (a + per).min(n);
        let lo = grid.low_hz + a as f64 * grid.bin_hz;
        let hi = grid.low_hz + b as f64 * grid.bin_hz;
        write!(
            w,
            "{:04}-{:02}-{:02}, {:02}:{:02}:{:02}, {lo:.0}, {hi:.0}, {:.2}, {samples}",
            c.year, c.month, c.day, c.hour, c.minute, c.second, grid.bin_hz
        )?;
        for &v in &sweep_db[a..b] {
            write!(w, ", {v:.2}")?;
        }
        writeln!(w)?;
        a = b;
    }
    Ok(())
}

/// Writes the four-trace layout: `frequency_hz,live_dbfs,peak_dbfs,average_dbfs`, then one row per bin. A trace that is
/// `None`, and a bin that no hop covered, give an empty cell.
pub fn write_traces<W: Write>(
    w: &mut W,
    grid: &Grid,
    live: Option<&[f32]>,
    peak: Option<&[f32]>,
    average: Option<&[f32]>,
) -> io::Result<()> {
    writeln!(w, "frequency_hz,live_dbfs,peak_dbfs,average_dbfs")?;
    let cell = |w: &mut W, t: Option<&[f32]>, k: usize| -> io::Result<()> {
        match t.and_then(|t| t.get(k)) {
            Some(&v) if v > UNCOVERED_DB => write!(w, "{v:.2}"),
            _ => Ok(()),
        }
    };
    for k in 0..grid.bins {
        write!(w, "{:.0},", grid.centre_hz(k))?;
        cell(w, live, k)?;
        write!(w, ",")?;
        cell(w, peak, k)?;
        write!(w, ",")?;
        cell(w, average, k)?;
        writeln!(w)?;
    }
    Ok(())
}

/// Writes the signal log: a header, then one row per channel. The flags are the words `image`, `spur` and `dc`.
pub fn write_signal_log<'a, W: Write>(
    w: &mut W,
    channels: impl IntoIterator<Item = &'a Channel>,
) -> io::Result<()> {
    writeln!(
        w,
        "id,first_seen_s,last_seen_s,centre_hz,width_hz,best_snr_db,peak_dbfs,count,sweeps,present,type,flags"
    )?;
    for c in channels {
        write!(
            w,
            "{},{:.3},{:.3},{:.0},{:.0},{:.1},{:.1},{},{},{:.2},{},",
            c.id,
            c.first_seen_s,
            c.last_seen_s,
            c.centre_hz,
            c.width_hz,
            c.snr_db,
            c.peak_db,
            c.count,
            c.sweeps_seen,
            c.share_present,
            ChannelType::CHOICES[usize::from(c.kind.index())].label
        )?;
        let flags = [
            (c.flags.image, "image"),
            (c.flags.spur, "spur"),
            (c.flags.dc, "dc"),
        ];
        let mut first = true;
        for (_, word) in flags.iter().filter(|f| f.0) {
            if !first {
                write!(w, " ")?;
            }
            write!(w, "{word}")?;
            first = false;
        }
        writeln!(w)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtl_power_text() {
        let g = Grid::new(100e6, 101e6, 100e3).unwrap();
        let v: Vec<f32> = (0..10).map(|i| -50.0 - i as f32).collect();
        let mut buf = Vec::new();
        write_rtl_power(&mut buf, 1_794_096_000.0, &g, &v, 4096, 4).unwrap();
        write_rtl_power(&mut buf, 1_794_096_000.0 + 3661.0, &g, &v, 4096, 4).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.lines().count(), 6);
        assert!(text.starts_with(
            "2026-11-08, 00:00:00, 100000000, 100400000, 100000.00, 4096, -50.00, -51.00, -52.00, -53.00\n"
        ));
        assert!(text
            .lines()
            .nth(3)
            .unwrap()
            .starts_with("2026-11-08, 01:01:01, 100000000"));
    }

    #[test]
    fn four_trace_text() {
        let g = Grid::new(100e6, 100.3e6, 100e3).unwrap();
        let live = [-10.5, -20.0, UNCOVERED_DB];
        let peak = [-5.0, -6.0, -7.0];
        let mut buf = Vec::new();
        write_traces(&mut buf, &g, Some(&live), Some(&peak), None).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(
            text,
            "frequency_hz,live_dbfs,peak_dbfs,average_dbfs\n100050000,-10.50,-5.00,\n100150000,-20.00,-6.00,\n100250000,,-7.00,\n"
        );
        let g = Grid::new(100e6, 200e6, 5e3).unwrap();
        let v = vec![-100.25f32; g.bins];
        let mut buf = Vec::new();
        write_traces(&mut buf, &g, Some(&v), Some(&v), Some(&v)).unwrap();
        assert_eq!(String::from_utf8_lossy(&buf).lines().count(), g.bins + 1);
    }

    #[test]
    fn signal_log() {
        let mut c = Channel {
            id: 7,
            centre_hz: 145.5e6,
            width_hz: 12e3,
            snr_db: 25.0,
            peak_db: -50.0,
            kind: crate::channels::ChannelType::Nfm,
            sweeps_seen: 4,
            share_present: 1.0,
            count: 1,
            first_seen_s: 1.0,
            last_seen_s: 4.0,
            active: true,
            confirmed: true,
            flags: Default::default(),
            priority: false,
        };
        let mut buf = Vec::new();
        write_signal_log(&mut buf, [&c]).unwrap();
        c.flags.image = true;
        c.flags.dc = true;
        write_signal_log(&mut buf, [&c]).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(
            text.lines().nth(1).unwrap(),
            "7,1.000,4.000,145500000,12000,25.0,-50.0,1,4,1.00,NFM,"
        );
        assert!(text.lines().nth(3).unwrap().ends_with(",NFM,image dc"));
    }
}

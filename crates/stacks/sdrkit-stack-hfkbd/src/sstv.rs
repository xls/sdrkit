//! SSTV: Robot 36 and 72, Martin M1 and M2, Scottie S1, S2 and DX, PD50, PD90, PD120, PD180 and Wraase SC2-180. The receiver
//! reads the tone from an unwrapped-phase ring, starts a picture at the VIS header (or at five sync pulses of a known line
//! period), follows the slant of the line syncs and reads each pixel from a ring whose band follows the pixel time of the mode.
//! The transmitter fits a picture given as pixels to the transmit mode and sends it once, from the same tone plan as the
//! test signal: the VIS header, the start pulse of the Scottie modes and the lines, one tone at a time with a continuous
//! phase. The generator sends a picture with or without its header, again and again.
//!
//! Standard: J. L. Barber N7CXI, "Proposal for SSTV mode specifications" (Dayton SSTV forum, 2000).

use crate::decoder::{self, flush_logs, put_text, Emit, HfGenerator, Modulator, GEN_SECTION};
use crate::picture::{self, HfPicture, PhaseRing};
use crate::typed::{
    SstvFields, SstvPixels, SstvSendArgs, SstvTxEvent, SstvTxFields, SSTV_PICTURE, SSTV_SEND,
    SSTV_STOP, SSTV_TX,
};
use sdrkit_algo_dsp::denoise::{
    line_wiener, LeeNoise, PictureLee, LEE_MIN_VALUES, LINE_WIENER_HALF,
};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_codec_image::geom::resize_box;
use sdrkit_codec_image::history::PictureHistory;
use sdrkit_codec_image::{PixelFormat as ImageFormat, Raster};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::keys::Family as SourceFamily;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    Choice as _, ChoiceDecl, Clock, Complex32, ConfigError, GenConfig, Generator, LockState,
    Record, Results, Schema, Setting, SettingError, SettingId, SettingSet as _, SignalInfo,
    StackConfig, StageError, Status, Value, Values,
};
use sdrkit_graph::components::{HostStreams, TxSink};
use sdrkit_graph::{
    values_of, Applied, Catalog, CommandTarget, Component, ComponentInfo, DecoderComponent, Format,
    GraphBuilder, GraphError, Io, Kept, Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
    TagKind,
};
use std::sync::Arc;

/// How a mode lays out a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Family {
    /// Robot 36: Y, then R-Y on even and B-Y on odd lines.
    Robot36,
    /// Robot 72: Y, R-Y, B-Y.
    Robot72,
    /// Martin: green, blue, red.
    Martin,
    /// Scottie: green, blue, sync, red.
    Scottie,
    /// PD: Y of the odd row, R-Y, B-Y, Y of the even row (two rows a line).
    Pd,
    /// Wraase SC2: red, green, blue.
    Wraase,
}

/// An SSTV mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SstvMode {
    /// Its name.
    pub name: &'static str,
    /// VIS code.
    pub vis: u8,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
    /// Line period, ms.
    pub line_ms: f64,
    /// Sync, ms.
    pub sync_ms: f64,
    /// Scan (one colour channel), ms.
    pub scan_ms: f64,
    /// Layout.
    pub family: Family,
}

/// The twelve required modes.
pub const MODES: [SstvMode; 12] = [
    SstvMode {
        name: "Robot 36",
        vis: 8,
        w: 320,
        h: 240,
        line_ms: 150.0,
        sync_ms: 9.0,
        scan_ms: 88.0,
        family: Family::Robot36,
    },
    SstvMode {
        name: "Robot 72",
        vis: 12,
        w: 320,
        h: 240,
        line_ms: 300.0,
        sync_ms: 9.0,
        scan_ms: 138.0,
        family: Family::Robot72,
    },
    SstvMode {
        name: "Martin M1",
        vis: 44,
        w: 320,
        h: 256,
        line_ms: 446.446,
        sync_ms: 4.862,
        scan_ms: 146.432,
        family: Family::Martin,
    },
    SstvMode {
        name: "Martin M2",
        vis: 40,
        w: 320,
        h: 256,
        line_ms: 226.798,
        sync_ms: 4.862,
        scan_ms: 73.216,
        family: Family::Martin,
    },
    SstvMode {
        name: "Scottie S1",
        vis: 60,
        w: 320,
        h: 256,
        line_ms: 428.22,
        sync_ms: 9.0,
        scan_ms: 138.24,
        family: Family::Scottie,
    },
    SstvMode {
        name: "Scottie S2",
        vis: 56,
        w: 320,
        h: 256,
        line_ms: 277.692,
        sync_ms: 9.0,
        scan_ms: 88.064,
        family: Family::Scottie,
    },
    SstvMode {
        name: "Scottie DX",
        vis: 76,
        w: 320,
        h: 256,
        line_ms: 1050.3,
        sync_ms: 9.0,
        scan_ms: 345.6,
        family: Family::Scottie,
    },
    SstvMode {
        name: "PD50",
        vis: 93,
        w: 320,
        h: 256,
        line_ms: 388.16,
        sync_ms: 20.0,
        scan_ms: 91.52,
        family: Family::Pd,
    },
    SstvMode {
        name: "PD90",
        vis: 99,
        w: 320,
        h: 256,
        line_ms: 703.04,
        sync_ms: 20.0,
        scan_ms: 170.24,
        family: Family::Pd,
    },
    SstvMode {
        name: "PD120",
        vis: 95,
        w: 640,
        h: 496,
        line_ms: 508.48,
        sync_ms: 20.0,
        scan_ms: 121.6,
        family: Family::Pd,
    },
    SstvMode {
        name: "PD180",
        vis: 96,
        w: 640,
        h: 496,
        line_ms: 754.24,
        sync_ms: 20.0,
        scan_ms: 183.04,
        family: Family::Pd,
    },
    SstvMode {
        name: "Wraase SC2-180",
        vis: 55,
        w: 320,
        h: 256,
        line_ms: 711.0225,
        sync_ms: 5.5225,
        scan_ms: 235.0,
        family: Family::Wraase,
    },
];

/// A mode by name or VIS code.
pub fn mode(name: &str) -> Option<SstvMode> {
    MODES
        .iter()
        .copied()
        .find(|m| m.name.eq_ignore_ascii_case(name))
}

/// What a stretch of a line is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    /// The 1200 Hz sync.
    Sync,
    /// A steady tone.
    Tone(f64),
    /// A scan of a channel: 0 R, 1 G, 2 B, 3 Y (of the line's row), 4 R-Y, 5 B-Y, 6 Y of the second row (PD).
    Scan(u8),
}

impl SstvMode {
    /// Transmitted lines.
    pub fn lines(&self) -> u32 {
        if self.family == Family::Pd {
            self.h / 2
        } else {
            self.h
        }
    }
    /// The segments of line `l` as (segment, ms).
    pub fn segments(&self, l: u32) -> Vec<(Seg, f64)> {
        let (s, sc) = (self.sync_ms, self.scan_ms);
        match self.family {
            Family::Robot36 => {
                let even = l.is_multiple_of(2);
                vec![
                    (Seg::Sync, s),
                    (Seg::Tone(1500.0), 3.0),
                    (Seg::Scan(3), sc),
                    (Seg::Tone(if even { 1500.0 } else { 2300.0 }), 4.5),
                    (Seg::Tone(1900.0), 1.5),
                    (Seg::Scan(if even { 4 } else { 5 }), 44.0),
                ]
            }
            Family::Robot72 => vec![
                (Seg::Sync, s),
                (Seg::Tone(1500.0), 3.0),
                (Seg::Scan(3), sc),
                (Seg::Tone(1500.0), 4.5),
                (Seg::Tone(1900.0), 1.5),
                (Seg::Scan(4), 69.0),
                (Seg::Tone(2300.0), 4.5),
                (Seg::Tone(1900.0), 1.5),
                (Seg::Scan(5), 69.0),
            ],
            Family::Martin => vec![
                (Seg::Sync, s),
                (Seg::Tone(1500.0), 0.572),
                (Seg::Scan(1), sc),
                (Seg::Tone(1500.0), 0.572),
                (Seg::Scan(2), sc),
                (Seg::Tone(1500.0), 0.572),
                (Seg::Scan(0), sc),
                (Seg::Tone(1500.0), 0.572),
            ],
            Family::Scottie => vec![
                (Seg::Tone(1500.0), 1.5),
                (Seg::Scan(1), sc),
                (Seg::Tone(1500.0), 1.5),
                (Seg::Scan(2), sc),
                (Seg::Sync, s),
                (Seg::Tone(1500.0), 1.5),
                (Seg::Scan(0), sc),
            ],
            Family::Pd => vec![
                (Seg::Sync, s),
                (Seg::Tone(1500.0), 2.08),
                (Seg::Scan(3), sc),
                (Seg::Scan(4), sc),
                (Seg::Scan(5), sc),
                (Seg::Scan(6), sc),
            ],
            Family::Wraase => vec![
                (Seg::Sync, s),
                (Seg::Tone(1500.0), 0.5),
                (Seg::Scan(0), sc),
                (Seg::Scan(1), sc),
                (Seg::Scan(2), sc),
            ],
        }
    }
    /// Where the sync starts in a line, ms.
    pub fn sync_at(&self) -> f64 {
        if self.family == Family::Scottie {
            2.0 * self.scan_ms + 3.0
        } else {
            0.0
        }
    }
}

/// The divisor of R - Y in the colour-difference coding of the specification.
const RY_DIV: f64 = 1.4;
/// The divisor of B - Y in the colour-difference coding of the specification.
const BY_DIV: f64 = 1.78;

/// Y, R-Y and B-Y (0 to 255, the colour differences centred on 128) to RGB, rounded and saturated: the inverse of
/// [`rgb_to_ycc`].
pub fn ycc_to_rgb(y: f64, ry: f64, by: f64) -> [u8; 3] {
    let r = y + RY_DIV * (ry - 128.0);
    let b = y + BY_DIV * (by - 128.0);
    let g = (y - 0.299 * r - 0.114 * b) / 0.587;
    [r, g, b].map(|v| v.round().clamp(0.0, 255.0) as u8)
}

/// RGB to Y, R-Y and B-Y as the specification codes them: Y = 0.299 R + 0.587 G + 0.114 B (the luma weights of ITU-R
/// BT.601), R-Y = 128 + (R - Y) / 1.4 and B-Y = 128 + (B - Y) / 1.78, the colour differences clamped to 0 to 255.
pub fn rgb_to_ycc(p: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = p.map(f64::from);
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    [
        y,
        (128.0 + (r - y) / RY_DIV).clamp(0.0, 255.0),
        (128.0 + (b - y) / BY_DIV).clamp(0.0, 255.0),
    ]
}

/// A test picture: colour bars, a grey ramp and blocks (RGB, row by row).
pub fn test_picture(w: u32, h: u32) -> Vec<u8> {
    let bars = [
        [255, 255, 255],
        [255, 255, 0],
        [0, 255, 255],
        [0, 255, 0],
        [255, 0, 255],
        [255, 0, 0],
        [0, 0, 255],
        [0, 0, 0],
    ];
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let p: [u8; 3] = if y < h / 2 {
                bars[(x * 8 / w) as usize]
            } else if y < 3 * h / 4 {
                let g = (x * 255 / (w - 1)) as u8;
                [g, g, g]
            } else if ((x / 20) + (y / 20)) % 2 == 0 {
                [200, 60, 40]
            } else {
                [40, 80, 200]
            };
            v.extend_from_slice(&p);
        }
    }
    v
}

/// A textured test picture: vertical, then horizontal stripes 4 pixels wide on the left half and 8 on the right, then dots of 2
/// x 2 pixels every 8 on the left and a 4-pixel checkerboard on the right (RGB, row by row).
pub fn texture_picture(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let left = x < w / 2;
            let width = if left { 4 } else { 8 };
            let p: [u8; 3] = if y < h / 3 {
                if (x / width) % 2 == 0 {
                    [230, 230, 230]
                } else if left {
                    [20, 20, 20]
                } else {
                    [200, 40, 40]
                }
            } else if y < 2 * h / 3 {
                if (y / width) % 2 == 0 {
                    [40, 200, 200]
                } else {
                    [30, 30, 120]
                }
            } else if left {
                if x % 8 < 2 && y % 8 < 2 {
                    [240, 240, 240]
                } else {
                    [20, 30, 90]
                }
            } else if ((x / 4) + (y / 4)) % 2 == 0 {
                [220, 200, 40]
            } else {
                [40, 120, 40]
            };
            v.extend_from_slice(&p);
        }
    }
    v
}

/// The sync tone, Hz.
pub const SYNC_HZ: f64 = 1200.0;

/// The tone of level `v` (0 black to 255 white): 1500 to 2300 Hz.
pub fn level_hz(v: f64) -> f64 {
    1500.0 + 800.0 * v / 255.0
}

/// The tones of the VIS header of `code`: 1900 Hz for 300 ms, 1200 Hz for 10 ms, 1900 Hz for 300 ms, then ten bits of 30 ms:
/// the start bit (1200 Hz), seven data bits least significant first (1 at 1100 Hz, 0 at 1300 Hz), the parity bit (1 when the
/// count of ones is odd, so that the eight bits have even parity) and the stop bit (1200 Hz). 910 ms in all.
pub fn vis_tones(code: u8) -> Vec<(f64, f64)> {
    let bit = |one: bool| (if one { 1100.0 } else { 1300.0 }, 30.0);
    let mut t = vec![
        (1900.0, 300.0),
        (SYNC_HZ, 10.0),
        (1900.0, 300.0),
        (SYNC_HZ, 30.0),
    ];
    t.extend((0..7).map(|b| bit((code >> b) & 1 == 1)));
    t.push(bit(code.count_ones() % 2 == 1));
    t.push((SYNC_HZ, 30.0));
    t
}

/// The time on air of a picture in `m`, ms: the VIS header when `vis`, the 9 ms start pulse of the Scottie modes when
/// `start_pulse`, and the lines.
pub fn air_time_ms(m: &SstvMode, vis: bool, start_pulse: bool) -> f64 {
    let header = if vis { 910.0 } else { 0.0 };
    let pulse = if start_pulse && m.family == Family::Scottie {
        m.sync_ms
    } else {
        0.0
    };
    let lines: f64 = (0..m.lines())
        .map(|l| m.segments(l).iter().map(|s| s.1).sum::<f64>())
        .sum();
    header + pulse + lines
}

/// A picture to send in one mode: the tones of its header and of each line. The picture is RGB, row by row, of the size of
/// the mode. Martin, Scottie and Wraase send R, G and B as they are; Robot and PD send Y, R-Y and B-Y ([`rgb_to_ycc`]).
/// The chroma of Robot 36 is the mean of the 2 x 2 group of rows (n with its lowest bit cleared, and the next) and columns
/// (2j, 2j + 1), R-Y on even lines and B-Y on odd ones; Robot 72 averages columns 2j and 2j + 1 of its row; PD averages each
/// column over its two rows. A scan of L ms with N pixels sends pixel i from i L / N to (i + 1) L / N ms after its start.
#[derive(Clone, Debug)]
pub struct SstvSignal {
    m: SstvMode,
    rgb: Vec<u8>,
    vis: bool,
    start_pulse: bool,
}

impl SstvSignal {
    /// The picture `rgb` (the size of `m`, RGB row by row; missing bytes are black) in `m`, with the VIS header and, in the
    /// Scottie modes, the start pulse before the first line.
    pub fn new(m: SstvMode, rgb: Vec<u8>) -> Self {
        let mut rgb = rgb;
        rgb.resize((m.w * m.h * 3) as usize, 0);
        SstvSignal {
            m,
            rgb,
            vis: true,
            start_pulse: true,
        }
    }
    /// With or without the VIS header.
    #[must_use]
    pub fn with_vis(mut self, vis: bool) -> Self {
        self.vis = vis;
        self
    }
    /// With or without the start pulse of the Scottie modes: 9 ms of 1200 Hz between the VIS header and the first line.
    #[must_use]
    pub fn with_start_pulse(mut self, on: bool) -> Self {
        self.start_pulse = on;
        self
    }
    /// The mode.
    pub fn mode(&self) -> SstvMode {
        self.m
    }
    /// The time on air, ms ([`air_time_ms`]).
    pub fn air_time_ms(&self) -> f64 {
        air_time_ms(&self.m, self.vis, self.start_pulse)
    }
    /// The tones before the first line as (Hz, ms): the VIS header and the start pulse.
    pub fn header(&self) -> Vec<(f64, f64)> {
        let mut t = if self.vis {
            vis_tones(self.m.vis)
        } else {
            Vec::new()
        };
        if self.start_pulse && self.m.family == Family::Scottie {
            t.push((SYNC_HZ, self.m.sync_ms));
        }
        t
    }
    /// The tones of line `l` as (Hz, ms), into `out`.
    pub fn line(&self, l: u32, out: &mut Vec<(f64, f64)>) {
        out.clear();
        for (seg, ms) in self.m.segments(l) {
            match seg {
                Seg::Sync => out.push((SYNC_HZ, ms)),
                Seg::Tone(f) => out.push((f, ms)),
                Seg::Scan(ch) => {
                    let n = self.pixels(ch);
                    let d = ms / f64::from(n);
                    out.extend((0..n).map(|j| (level_hz(self.value(l, ch, j, n)), d)));
                }
            }
        }
    }
    /// The pixels of a scan of channel `ch`: half the width for the chroma of the Robot modes.
    fn pixels(&self, ch: u8) -> u32 {
        let robot = matches!(self.m.family, Family::Robot36 | Family::Robot72);
        if robot && matches!(ch, 4 | 5) {
            self.m.w / 2
        } else {
            self.m.w
        }
    }
    fn px(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y.min(self.m.h - 1) * self.m.w + x.min(self.m.w - 1)) * 3) as usize;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
    /// The level of pixel `j` of the scan of channel `ch` (as [`Seg::Scan`]) in line `l`, the scan having `n` pixels.
    fn value(&self, l: u32, ch: u8, j: u32, n: u32) -> f64 {
        let pd = self.m.family == Family::Pd;
        match ch {
            0..=2 => f64::from(self.px(j, l)[ch as usize]),
            3 => rgb_to_ycc(self.px(j, if pd { 2 * l } else { l }))[0],
            6 => rgb_to_ycc(self.px(j, 2 * l + 1))[0],
            _ => {
                let k = if ch == 4 { 1 } else { 2 };
                let rows = match self.m.family {
                    Family::Robot36 => [l & !1, (l & !1) + 1],
                    Family::Pd => [2 * l, 2 * l + 1],
                    _ => [l, l],
                };
                let cols = if n < self.m.w {
                    [2 * j, 2 * j + 1]
                } else {
                    [j, j]
                };
                let mut sum = 0.0;
                for y in rows {
                    for x in cols {
                        sum += rgb_to_ycc(self.px(x, y))[k];
                    }
                }
                sum / 4.0
            }
        }
    }
}

/// Writes an [`SstvSignal`] as complex samples at a stream rate, the dial at 0 Hz (an audio tone of f Hz at +f Hz): one tone
/// at a time at a constant level, with a continuous phase. Tone k starts at the sample nearest to the sum of the lengths of
/// the tones before it, so the total length is exact to a sample and no error builds up. A tone of 0 Hz is silence.
pub struct SstvWriter {
    sig: SstvSignal,
    rate: f64,
    amp: f32,
    nco: Nco,
    lead_ms: f64,
    tail_ms: f64,
    repeat: bool,
    tones: Vec<(f64, f64)>,
    k: usize,
    /// The next group of tones: 0 the header, 1 to the lines the lines, one more the pause.
    group: u32,
    t_ms: f64,
    end: u64,
    freq: f64,
    n: u64,
}

impl SstvWriter {
    /// `sig` once at `rate` and level `amp` (of full scale).
    pub fn new(sig: SstvSignal, rate: f64, amp: f32) -> Self {
        SstvWriter {
            sig,
            rate,
            amp,
            nco: Nco::new(rate, 0.0),
            lead_ms: 0.0,
            tail_ms: 0.0,
            repeat: false,
            tones: Vec::new(),
            k: 0,
            group: 0,
            t_ms: 0.0,
            end: 0,
            freq: 0.0,
            n: 0,
        }
    }
    /// The picture again and again, each time after `lead_ms` of silence and followed by `tail_ms` of silence.
    #[must_use]
    pub fn repeated(mut self, lead_ms: f64, tail_ms: f64) -> Self {
        self.lead_ms = lead_ms;
        self.tail_ms = tail_ms;
        self.repeat = true;
        self
    }
    /// The signal.
    pub fn signal(&self) -> &SstvSignal {
        &self.sig
    }
    /// The line being written, from 1; 0 during the header.
    pub fn line(&self) -> u32 {
        self.group.saturating_sub(1).min(self.sig.m.lines())
    }
    /// The samples written.
    pub fn written(&self) -> u64 {
        self.n
    }
    /// Loads the next group of tones. False when a picture sent once is over.
    fn next_group(&mut self) -> bool {
        let lines = self.sig.m.lines();
        if self.group > lines {
            if !self.repeat {
                return false;
            }
            if self.group == lines + 1 {
                self.tones.clear();
                self.tones.push((0.0, self.tail_ms));
                self.group += 1;
                self.k = 0;
                return true;
            }
            self.group = 0;
        }
        if self.group == 0 {
            self.tones.clear();
            if self.lead_ms > 0.0 {
                self.tones.push((0.0, self.lead_ms));
            }
            let header = self.sig.header();
            self.tones.extend(header);
        } else {
            self.sig.line(self.group - 1, &mut self.tones);
        }
        self.group += 1;
        self.k = 0;
        true
    }
    /// Starts the next tone. False at the end.
    fn next_tone(&mut self) -> bool {
        while self.k >= self.tones.len() {
            if !self.next_group() {
                return false;
            }
        }
        let (f, ms) = self.tones[self.k];
        self.k += 1;
        self.t_ms += ms;
        self.end = (self.t_ms * self.rate / 1000.0).round() as u64;
        if f != self.freq {
            self.freq = f;
            if f > 0.0 {
                self.nco.set_freq(f);
            }
        }
        true
    }
    /// Whether a picture sent once is over.
    pub fn finished(&self) -> bool {
        !self.repeat
            && self.n >= self.end
            && self.k >= self.tones.len()
            && self.group > self.sig.m.lines()
    }
    /// Fills `out` from the start and returns the samples written: fewer than `out` holds only at the end of a picture sent
    /// once.
    pub fn fill(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.n >= self.end {
                if !self.next_tone() {
                    break;
                }
                continue;
            }
            let m = ((self.end - self.n) as usize).min(out.len() - i);
            let o = &mut out[i..i + m];
            if self.freq > 0.0 {
                self.nco.fill(o);
                for z in o.iter_mut() {
                    *z *= self.amp;
                }
            } else {
                o.fill(Complex32::default());
            }
            i += m;
            self.n += m as u64;
        }
        i
    }
}

/// The test signal's modulator: the picture after 500 ms of silence, then 1000 ms of silence, again and again.
pub struct SstvModulator {
    w: SstvWriter,
}

impl SstvModulator {
    /// Sends `rgb` (the mode's size, RGB row by row) in `m` at full scale, with the VIS header and the Scottie start pulse when
    /// `vis`.
    pub fn new(m: SstvMode, rate: f64, rgb: &[u8], vis: bool) -> Self {
        let sig = SstvSignal::new(m, rgb.to_vec())
            .with_vis(vis)
            .with_start_pulse(vis);
        SstvModulator {
            w: SstvWriter::new(sig, rate, 1.0).repeated(500.0, 1000.0),
        }
    }
}

impl Modulator for SstvModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        self.w.fill(out);
    }
}

/// The picture mode of the test signal: an entry of [`MODES`].
pub const GEN_MODE: CatalogueSetting = CatalogueSetting {
    key: "gen_mode",
    label: "Picture mode",
    help: "The SSTV mode of the test picture.",
    choices: || {
        MODES
            .iter()
            .map(|m| ChoiceDecl::new(m.name, m.name))
            .collect()
    },
    default: "Robot 36",
    advanced: false,
};
/// The test signal sends the VIS header.
pub const GEN_VIS: Setting<field::Bool> = Setting::new("gen_vis", "Send the header", "", true)
    .section(GEN_SECTION)
    .help(
        "Off: the picture starts without its VIS header, so the stack finds it by the line period.",
    )
    .advanced();

sdrkit_core::choice_enum! {
    /// The picture of the test signal.
    pub enum GenPicture {
        /// Colour bars, a grey ramp and blocks: flat areas.
        Card = ("card", "Test card"),
        /// Stripes 4 and 8 pixels wide, dots and a checkerboard.
        Texture = ("texture", "Texture"),
    }
}

/// The picture of the test signal.
pub const GEN_PICTURE: Setting<field::Choice<GenPicture>> =
    Setting::choice("gen_picture", "Test picture", GenPicture::Card)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the SSTV stack.
    pub enum SstvGenSetting {
        /// The picture mode.
        Mode = GEN_MODE,
        /// The header.
        Vis = GEN_VIS,
        /// The picture.
        Picture = GEN_PICTURE,
    }
}

/// The noise-adaptive picture filter: the line Wiener filter on the tones of each scan and the 3 x 3 Lee filter of each colour.
pub const PICTURE_FILTER: Setting<field::Bool> =
    Setting::new("picture_filter", "Picture filter", "", true).help(
        "Smooths the noise of a weak picture along each scan line and over 3 x 3 pixels, with the noise measured on the syncs and \
         in the picture. A clean picture is left as it is.",
    );

sdrkit_core::settings! {
    /// The settings of the SSTV receiver.
    pub enum SstvSetting {
        /// The picture filter.
        PictureFilter = PICTURE_FILTER,
    }
}

const MS: f64 = 8.0;
/// The seconds of input kept.
const HISTORY_S: f64 = 16.0;

/// The rings of a picture, each with the band of the picture's mode ([`video_half_hz`]).
struct Video {
    /// The phase of the picture band, centred on the middle of the video tones: the pixels are read here.
    picture: PhaseRing,
    /// The phase of a band of the same width that holds the sync tone [`SYNC_EDGE_HZ`] inside its lower edge, as the SSTV
    /// band holds it. The noise of a tone grows towards the edge of its band: the sync tone there has the noise of the pixels with
    /// the most of it.
    sync: PhaseRing,
}

struct Rx {
    m: SstvMode,
    vis: i32,
    /// Sample index of line 0's start, and the line period in samples (fitted).
    t0: f64,
    period: f64,
    nominal: f64,
    found: Vec<(f64, f64)>,
    next_line: u32,
    missed: u32,
    rgb: Vec<u8>,
    chroma: Vec<f64>,
    rows: u32,
    id: u64,
    start: f64,
    /// For each pixel length of the mode (samples): the pixel noise (Hz^2) of a mean over that length, measured on the syncs.
    line_noise: Vec<(f64, Option<f64>)>,
    /// The picture shown: the filtered rows below [`Rx::filtered`], the raw ones from there.
    shown: Vec<u8>,
    filtered: u32,
    /// The raw rows whose Laplacian is in [`Rx::lee`].
    noise_rows: u32,
    /// The picture noise of each colour.
    lee: [LeeNoise; 3],
    /// A window of up to five rows of one colour and its filtered form.
    window: Vec<u8>,
    window_out: Vec<u8>,
    plane: PictureLee,
}

/// The SSTV receiver.
pub struct SstvDecoder {
    /// The phase of the whole SSTV band: the header and the syncs are found here.
    ring: PhaseRing,
    /// The last seconds of the input, from which the rings of a picture are filled.
    audio: Vec<Complex32>,
    /// The rings of the picture being received.
    video: Option<Video>,
    dial: f64,
    t: u64,
    next_check: u64,
    offset: f64,
    pulses: Vec<f64>,
    in_pulse: Option<u64>,
    rx: Option<Rx>,
    hist: PictureHistory<HfPicture>,
    last: (i32, f64),
    serial: u64,
    state: u8,
    logs: Vec<String>,
    status: Status,
    last_done: u64,
    pending_vis: Option<(SstvMode, u8, f64)>,
    /// The picture filter is on.
    filter_on: bool,
}

impl SstvDecoder {
    /// A receiver.
    pub fn new() -> Self {
        SstvDecoder {
            ring: PhaseRing::new(8000.0, 1900.0, BAND_HALF_HZ, HISTORY_S),
            audio: vec![Complex32::default(); (HISTORY_S * 8000.0) as usize],
            video: None,
            dial: 0.0,
            t: 0,
            next_check: 0,
            offset: 0.0,
            pulses: Vec::new(),
            in_pulse: None,
            rx: None,
            hist: PictureHistory::new(8),
            last: (-1, 0.0),
            serial: 0,
            state: 0,
            logs: Vec::new(),
            status: Status::default(),
            last_done: 0,
            pending_vis: None,
            filter_on: true,
        }
    }

    fn f(&self, a: f64, b: f64) -> f64 {
        self.ring.mean_freq(a, b)
    }

    /// Looks for a VIS header whose start bit begins at sample `s`.
    fn vis_at(&self, s: f64) -> Option<(u8, f64)> {
        let leader = self.f(s - 290.0 * MS, s - 10.0 * MS);
        let e = leader - 1900.0;
        if e.abs() > 130.0 {
            return None;
        }
        let l1 = self.f(s - 290.0 * MS, s - 150.0 * MS);
        let l2 = self.f(s - 150.0 * MS, s - 10.0 * MS);
        if (l1 - l2).abs() > 25.0 {
            return None;
        }
        if (self.f(s + 3.0 * MS, s + 27.0 * MS) - 1200.0 - e).abs() > 60.0 {
            return None;
        }
        let mut code = 0u8;
        let mut ones = 0;
        for i in 0..8 {
            let a = s + (30.0 * (i + 1) as f64 + 4.0) * MS;
            let f = self.f(a, a + 22.0 * MS) - e;
            if !(1030.0..1370.0).contains(&f) || (f - 1200.0).abs() < 40.0 {
                return None;
            }
            let one = f < 1200.0;
            if one {
                ones += 1;
                if i < 7 {
                    code |= 1 << i;
                }
            }
        }
        if ones % 2 != 0 {
            return None;
        }
        let stop = self.f(s + 274.0 * MS, s + 296.0 * MS) - e;
        if (stop - 1200.0).abs() > 60.0 {
            return None;
        }
        Some((code, e))
    }

    /// The end of the 1200 Hz plateau that starts near `from` (the stop bit running into the first sync).
    fn plateau_end(&self, from: f64, to: f64) -> Option<f64> {
        let thr = 1350.0 + self.offset;
        let mut i = from;
        while i + 2.0 * MS < to {
            if self.f(i, i + 2.0 * MS) > thr {
                // Refine to the sample.
                let mut j = i - 2.0 * MS;
                while j < i + 2.0 * MS {
                    if self.f(j, j + 1.0 * MS) > thr {
                        return Some(j + 0.5 * MS);
                    }
                    j += 1.0;
                }
                return Some(i);
            }
            i += 1.0 * MS;
        }
        None
    }

    fn start(&mut self, m: SstvMode, vis: i32, t0: f64) {
        let p = m.line_ms * MS;
        self.serial += 1;
        let id = self.hist.next_id();
        self.logs
            .push(format!("{}: picture started (VIS {vis})", m.name));
        self.rx = Some(Rx {
            m,
            vis,
            t0,
            period: p,
            nominal: p,
            found: Vec::new(),
            next_line: 0,
            missed: 0,
            rgb: vec![0; (m.w * m.h * 3) as usize],
            chroma: vec![128.0; (m.w * 2) as usize],
            rows: 0,
            id,
            start: t0 / 8000.0,
            line_noise: pixel_lengths(&m).into_iter().map(|l| (l, None)).collect(),
            shown: vec![0; (m.w * m.h * 3) as usize],
            filtered: 0,
            noise_rows: 0,
            lee: [LeeNoise::new(), LeeNoise::new(), LeeNoise::new()],
            window: Vec::new(),
            window_out: Vec::new(),
            plane: PictureLee::new(),
        });
        self.state = 1;
        self.pulses.clear();
        // The rings of the picture, filled from half a second before its first line.
        let half = video_half_hz(&m);
        let mut video = Video {
            picture: PhaseRing::new(8000.0, 1900.0 + self.offset, half, HISTORY_S),
            sync: PhaseRing::new(
                8000.0,
                1200.0 + self.offset + half - SYNC_EDGE_HZ,
                half,
                HISTORY_S,
            ),
        };
        let n = self.audio.len() as u64;
        let end = self.ring.count;
        let from = ((t0 - 4000.0).max(0.0) as u64)
            .max(end.saturating_sub(n - 1))
            .min(end);
        video.picture.count = from;
        video.sync.count = from;
        for i in from..end {
            let x = [self.audio[(i % n) as usize]];
            video.picture.push(&x);
            video.sync.push(&x);
        }
        self.video = Some(video);
    }

    fn idle(&mut self) {
        let now = self.next_check as f64 - self.ring.delay - 4.0;
        // A header found: wait until the stop bit has run into the first sync.
        if let Some((m, code, s)) = self.pending_vis {
            if now < s + 420.0 * MS {
                return;
            }
            self.pending_vis = None;
            // The first line: the far end of the plateau where the stop bit runs into the sync.
            let t0 = match self.plateau_end(s + 280.0 * MS, s + 420.0 * MS) {
                Some(end) => match m.family {
                    Family::Scottie => end,
                    _ => end - m.sync_ms * MS,
                },
                None => s + 300.0 * MS,
            };
            self.start(m, code as i32, t0);
            return;
        }
        // The VIS header: its stop bit ends 300 ms after the start bit begins.
        let s0 = now - 300.0 * MS;
        if s0 > 320.0 * MS && self.ring.holds((s0 - 320.0 * MS) as u64) {
            if let Some((code, e)) = self.vis_at(s0) {
                // The start bit's edge, to the sample.
                let mut best = (f64::MAX, s0);
                let mut s = s0 - 6.0 * MS;
                while s < s0 + 6.0 * MS {
                    let d = self.f(s - 2.0 * MS, s) - self.f(s, s + 2.0 * MS);
                    if -d < best.0 {
                        best = (-d, s);
                    }
                    s += 1.0;
                }
                if let Some(m) = MODES.iter().copied().find(|m| m.vis == code) {
                    self.offset = e;
                    self.pending_vis = Some((m, code, best.1));
                    self.logs.push(format!("VIS {code}: {}", m.name));
                    return;
                }
            }
        }
        // The pulse log: five sync pulses at the line period of a known mode start it without a header.
        let a = now - 4.0 * MS;
        if a > 0.0 && self.ring.holds(a as u64) {
            let f = self.f(a, now);
            let low = (f - 1200.0 - self.offset).abs() < 110.0;
            match (low, self.in_pulse) {
                (true, None) => self.in_pulse = Some(a as u64),
                (false, Some(s)) => {
                    self.in_pulse = None;
                    let len = a - s as f64;
                    if (2.0 * MS..30.0 * MS).contains(&len) {
                        self.pulses.push(s as f64);
                        if self.pulses.len() > 12 {
                            self.pulses.remove(0);
                        }
                        self.check_pulses();
                    }
                }
                _ => {}
            }
        }
    }

    fn check_pulses(&mut self) {
        let n = self.pulses.len();
        if n < 5 {
            return;
        }
        let p = &self.pulses[n - 5..];
        let d = (p[4] - p[0]) / 4.0;
        if (0..4).any(|i| ((p[i + 1] - p[i]) - d).abs() > 0.01 * d + 24.0) {
            return;
        }
        for m in MODES {
            let per = m.line_ms * MS;
            if (d - per).abs() < 0.006 * per {
                let last = p[4];
                // The tuning error from the pulses: their mean is about 12 Hz high.
                let sync = self.f(last + 1.0 * MS, last + (m.sync_ms - 1.0) * MS);
                self.offset = 0.9 * self.offset + 0.1 * (sync - 1212.0);
                let t0 = last - m.sync_at() * MS + per;
                self.start(m, -1, t0);
                if let Some(rx) = &mut self.rx {
                    rx.found.push((1.0, t0 + m.sync_at() * MS));
                    rx.found.clear();
                }
                return;
            }
        }
    }

    /// Searches line `k`'s sync near where it is predicted and returns its start. The window of the sync's length whose mean
    /// tone is lowest places the pulse; the start is then the rising edge into the 1500 Hz porch, less the sync's length. That
    /// edge lies between two known tones, so neither the picture before the sync nor the unequal heights of its two edges bias
    /// it, to a fraction of a sample.
    fn find_sync(&self, rx: &Rx, k: u32) -> Option<f64> {
        let pred = rx.t0 + k as f64 * rx.period + rx.m.sync_at() * MS * rx.period / rx.nominal;
        let w = rx.m.sync_ms * MS * 0.8;
        let mut best = (f64::MAX, pred);
        let mut s = pred - 6.0 * MS;
        while s < pred + 6.0 * MS {
            let f = self.f(s, s + w);
            if f < best.0 {
                best = (f, s);
            }
            s += 2.0;
        }
        if (best.0 - 1200.0 - self.offset).abs() > 120.0 {
            return None;
        }
        let len = rx.m.sync_ms * MS * rx.period / rx.nominal;
        let g = |s: f64| self.f(s, s + len);
        let (from, to) = ((best.1 - 0.4 * len).round(), (best.1 + 0.4 * len).round());
        let mut low = (f64::MAX, best.1);
        let mut s = from;
        while s <= to {
            let v = g(s);
            if v < low.0 {
                low = (v, s);
            }
            s += 1.0;
        }
        // The rising edge: the place where a window in the sync reads 1200 Hz and one in the porch 1500 Hz (least squares).
        let scale = rx.period / rx.nominal;
        let before = (0.5 * len).min(1.5 * MS * scale);
        let after = porch_ms(&rx.m).min(1.5) * MS * scale;
        let (sync, porch) = (1200.0 + self.offset, 1500.0 + self.offset);
        let miss = |s: f64| {
            (self.f(s - before, s) - sync).powi(2) + (self.f(s, s + after) - porch).powi(2)
        };
        let centre = (low.1 + len).round();
        let reach = (0.3 * len).round();
        let mut top = (f64::MAX, centre);
        let mut s = centre - reach;
        while s <= centre + reach {
            let v = miss(s);
            if v < top.0 {
                top = (v, s);
            }
            s += 1.0;
        }
        let (a, m, b) = (miss(top.1 - 1.0), top.0, miss(top.1 + 1.0));
        let den = a + b - 2.0 * m;
        let d = if den > 0.0 {
            (0.5 * (a - b) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some(top.1 + d - len)
    }

    fn receive(&mut self, out: &mut Outbox) {
        let Some(mut rx) = self.rx.take() else { return };
        // A narrow picture band has a longer filter than the SSTV band.
        let delay = self
            .video
            .as_ref()
            .map_or(self.ring.delay, |v| v.picture.delay.max(self.ring.delay));
        let now = self.next_check as f64 - delay - 4.0;
        while rx.next_line < rx.m.lines() {
            let k = rx.next_line;
            let end = rx.t0 + (k + 1) as f64 * rx.period + 8.0 * MS;
            if end > now {
                break;
            }
            match self.find_sync(&rx, k) {
                Some(s) => {
                    rx.missed = 0;
                    self.sync_noise(&mut rx, s);
                    rx.found
                        .push((k as f64, s - rx.m.sync_at() * MS * rx.period / rx.nominal));
                    if rx.found.len() > 400 {
                        rx.found.remove(0);
                    }
                    // The slant is the slope of a straight line through the line starts.
                    if rx.found.len() >= 4 {
                        fit(&mut rx);
                    }
                }
                None => {
                    rx.missed += 1;
                }
            }
            if rx.missed >= 3 {
                self.logs
                    .push(format!("{}: syncs stopped at line {k}", rx.m.name));
                self.finish(rx, out);
                return;
            }
            self.read_line(&mut rx, k);
            if self.filter_on {
                let fin = final_rows(&rx.m, k);
                filter_rows(&mut rx, fin, false);
            }
            rx.next_line += 1;
            self.serial += 1;
            if k % 8 == 7 {
                out.picture(now / 8000.0, picture_of(&rx, false).to_picture());
            }
        }
        if rx.next_line >= rx.m.lines() {
            self.finish(rx, out);
            return;
        }
        self.rx = Some(rx);
    }

    /// Measures the pixel noise on the sync that starts at sample `s`, for each pixel length of the mode: the steady middle of
    /// the pulse (20 to 80 %) cut into windows of that length; the variance of their mean tones in the sync ring. The first
    /// value sets it, then a weight of 0.1. A length that the pulse holds fewer than three times is not measured.
    fn sync_noise(&self, rx: &mut Rx, s: f64) {
        let Some(video) = &self.video else { return };
        let len = rx.m.sync_ms * MS * rx.period / rx.nominal;
        let (a, b) = (s + 0.2 * len, s + 0.8 * len);
        for (pixel, noise) in &mut rx.line_noise {
            let n = ((b - a) / *pixel).floor() as usize;
            if n < SYNC_NOISE_WINDOWS {
                continue;
            }
            let (mut sum, mut sq) = (0.0, 0.0);
            for i in 0..n {
                let f = video
                    .sync
                    .mean_freq(a + i as f64 * *pixel, a + (i + 1) as f64 * *pixel);
                sum += f;
                sq += f * f;
            }
            let mean = sum / n as f64;
            let v = (sq / n as f64 - mean * mean).max(0.0);
            *noise = Some(noise.map_or(v, |old| old + LINE_NOISE_WEIGHT * (v - old)));
        }
    }

    fn read_line(&self, rx: &mut Rx, l: u32) {
        let m = rx.m;
        let scale = rx.period / rx.nominal;
        let start = rx.t0 + l as f64 * rx.period;
        let mut t = start;
        let px = |f: f64| ((f - 1500.0 - self.offset) / 800.0 * 255.0).clamp(0.0, 255.0);
        let w = m.w as usize;
        let mut y = vec![0.0; w];
        let mut y2 = vec![0.0; w];
        let mut rgb = vec![[0.0f64; 3]; w];
        let mut ry = vec![128.0; w];
        let mut by = vec![128.0; w];
        let mut tones = vec![0f32; w];
        let mut smooth = vec![0f32; w];
        for (seg, ms) in m.segments(l) {
            let d = ms * MS * scale;
            if let Seg::Scan(ch) = seg {
                let pd = d / w as f64;
                // The mean tone over each pixel's own slot, in the picture band.
                for (x, tone) in tones.iter_mut().enumerate() {
                    let a = t + x as f64 * pd;
                    *tone = match &self.video {
                        Some(v) => v.picture.mean_freq(a, a + pd),
                        None => self.f(a, a + pd),
                    } as f32;
                }
                // The line Wiener filter with the noise measured for the pixel length nearest to this scan's.
                let noise = rx
                    .line_noise
                    .iter()
                    .min_by(|a, b| (a.0 - pd).abs().total_cmp(&(b.0 - pd).abs()))
                    .and_then(|n| n.1);
                if let (true, Some(nv)) = (self.filter_on, noise) {
                    if line_wiener(&tones, nv as f32, LINE_WIENER_HALF, &mut smooth).is_ok() {
                        tones.copy_from_slice(&smooth);
                    }
                }
                for x in 0..w {
                    let v = px(f64::from(tones[x]));
                    match ch {
                        0..=2 => rgb[x][ch as usize] = v,
                        3 => y[x] = v,
                        4 => ry[x] = v,
                        5 => by[x] = v,
                        _ => y2[x] = v,
                    }
                }
            }
            t += d;
        }
        let mut put = |row: u32, x: usize, p: [u8; 3]| {
            if row < m.h {
                let i = ((row * m.w) as usize + x) * 3;
                rx.rgb[i..i + 3].copy_from_slice(&p);
            }
        };
        match m.family {
            Family::Martin | Family::Scottie | Family::Wraase => {
                for x in 0..w {
                    put(l, x, [rgb[x][0] as u8, rgb[x][1] as u8, rgb[x][2] as u8]);
                }
                rx.rows = l + 1;
            }
            Family::Robot72 => {
                for x in 0..w {
                    put(l, x, ycc_to_rgb(y[x], ry[x], by[x]));
                }
                rx.rows = l + 1;
            }
            Family::Pd => {
                for x in 0..w {
                    put(2 * l, x, ycc_to_rgb(y[x], ry[x], by[x]));
                    put(2 * l + 1, x, ycc_to_rgb(y2[x], ry[x], by[x]));
                }
                rx.rows = 2 * l + 2;
            }
            Family::Robot36 => {
                // R-Y on even lines, B-Y on odd: an odd line completes the pair.
                let even = l.is_multiple_of(2);
                for x in 0..w {
                    if even {
                        rx.chroma[x] = ry[x];
                        rx.chroma[w + x] = y[x];
                        // Shown at once with neutral blue until the odd line comes.
                        put(l, x, ycc_to_rgb(y[x], ry[x], 128.0));
                    } else {
                        let (r, ye) = (rx.chroma[x], rx.chroma[w + x]);
                        put(l - 1, x, ycc_to_rgb(ye, r, by[x]));
                        put(l, x, ycc_to_rgb(y[x], r, by[x]));
                    }
                }
                rx.rows = l + 1;
            }
        }
    }

    fn finish(&mut self, mut rx: Rx, out: &mut Outbox) {
        if self.filter_on {
            let rows = rx.rows;
            filter_rows(&mut rx, rows, true);
        }
        let complete = rx.next_line >= rx.m.lines();
        let p = picture_of(&rx, true);
        self.logs.push(format!(
            "{}: picture {} ({} rows, slant {:.0} ppm)",
            rx.m.name,
            if complete { "complete" } else { "cut off" },
            rx.rows,
            (rx.period / rx.nominal - 1.0) * 1e6
        ));
        let time = self.t as f64 / 8000.0;
        out.event(picture::picture_event(&SSTV_PICTURE, &p, time, self.dial));
        out.picture(time, p.to_picture());
        self.last = (rx.vis, (rx.period / rx.nominal - 1.0) * 1e6);
        self.hist.finish(p.id, p);
        self.state = 2;
        self.last_done = self.t;
        self.video = None;
        self.serial += 1;
        self.pulses.clear();
        self.in_pulse = None;
    }
}

fn fit(rx: &mut Rx) {
    let n = rx.found.len() as f64;
    let (sx, sy) = rx
        .found
        .iter()
        .fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let (mx, my) = (sx / n, sy / n);
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for p in &rx.found {
        sxx += (p.0 - mx) * (p.0 - mx);
        sxy += (p.0 - mx) * (p.1 - my);
    }
    if sxx <= 0.0 {
        return;
    }
    let b = sxy / sxx;
    // Outliers (a sync hidden in noise) are dropped and the fit made again.
    let a = my - b * mx;
    let before = rx.found.len();
    rx.found.retain(|p| (p.1 - (a + b * p.0)).abs() < 3.0 * MS);
    if rx.found.len() < before && rx.found.len() >= 4 {
        return fit(rx);
    }
    if (b / rx.nominal - 1.0).abs() < 0.01 {
        rx.period = b;
        rx.t0 = a;
    }
}

/// Windows of the shortest pixel a sync must hold for its noise to be measured.
const SYNC_NOISE_WINDOWS: usize = 3;
/// The weight of a new sync in the averaged pixel noise.
const LINE_NOISE_WEIGHT: f64 = 0.1;

/// The 1500 Hz porch after the sync of a mode, ms.
fn porch_ms(m: &SstvMode) -> f64 {
    let seg = m.segments(0);
    seg.iter()
        .position(|(s, _)| *s == Seg::Sync)
        .and_then(|i| seg.get(i + 1))
        .map_or(0.5, |&(_, ms)| ms)
}

/// The pixel lengths of a mode's scans, in samples at 8 kHz, each once (the Robot modes scan colour faster than luminance).
fn pixel_lengths(m: &SstvMode) -> Vec<f64> {
    let mut lengths: Vec<f64> = Vec::new();
    for (seg, ms) in (0..2).flat_map(|l| m.segments(l)) {
        let len = ms * MS / m.w as f64;
        if matches!(seg, Seg::Scan(_)) && !lengths.iter().any(|l| (l - len).abs() < 1e-9) {
            lengths.push(len);
        }
    }
    lengths
}

/// The video tones lie this far either side of the middle of the picture band, Hz.
const VIDEO_DEVIATION_HZ: f64 = 400.0;
/// The period of the finest stripes that the picture band keeps, in luminance pixels: stripes 4 pixels wide.
const STRIPE_PERIOD_PIXELS: f64 = 8.0;
/// Half the width of the SSTV band, Hz: the header and the syncs are found in it.
const BAND_HALF_HZ: f64 = 950.0;
/// No picture band is wider than this either side, Hz: the audio of an SSB transmitter ends near 2700 Hz, 800 Hz above the
/// middle of the video tones, so sidebands farther out are not sent.
const PICTURE_HALF_MAX_HZ: f64 = 800.0;
/// The sync tone lies this far inside the lower edge of the SSTV band, Hz.
const SYNC_EDGE_HZ: f64 = 250.0;

/// Half the width of the picture band of a mode, Hz: the deviation of the video tones and the first sidebands of the finest
/// stripes kept, at most [`PICTURE_HALF_MAX_HZ`]. A narrower band loses those stripes; a wider one passes more noise to every pixel. The
/// longest pixel of the mode sets the band: the Robot modes scan colour in half the time of the luminance, with half its
/// detail.
fn video_half_hz(m: &SstvMode) -> f64 {
    let pixel_s = pixel_lengths(m).into_iter().fold(0.0, f64::max) / (MS * 1000.0);
    (VIDEO_DEVIATION_HZ + 1.0 / (STRIPE_PERIOD_PIXELS * pixel_s)).min(PICTURE_HALF_MAX_HZ)
}

/// The rows that no later line rewrites once line `l` of mode `m` is read: Robot 36 rewrites the row of an even line with the
/// odd line's colour, PD writes two rows a line.
fn final_rows(m: &SstvMode, l: u32) -> u32 {
    match m.family {
        Family::Robot36 if l.is_multiple_of(2) => l,
        Family::Pd => 2 * l + 2,
        _ => l + 1,
    }
    .min(m.h)
}

/// The picture filter: adds the Laplacians of the raw rows that are final to the noise of each colour, then filters each row
/// whose rows two above and two below are final (all rows up to `fin` when `done`) with click removal and the 3 x 3 Lee filter
/// of each colour into the picture shown. Rows wait, shown raw, until each colour's noise is known.
fn filter_rows(rx: &mut Rx, fin: u32, done: bool) {
    let w = rx.m.w as usize;
    let row = |rgb: &[u8], r: u32, c: usize, out: &mut Vec<u8>| {
        let base = r as usize * w * 3;
        out.extend((0..w).map(|x| rgb[base + 3 * x + c]));
    };
    let (mut a, mut b, mut c) = (
        Vec::with_capacity(w),
        Vec::with_capacity(w),
        Vec::with_capacity(w),
    );
    while rx.noise_rows + 1 < fin {
        let r = rx.noise_rows;
        if r >= 1 {
            for k in 0..3 {
                a.clear();
                b.clear();
                c.clear();
                row(&rx.rgb, r - 1, k, &mut a);
                row(&rx.rgb, r, k, &mut b);
                row(&rx.rgb, r + 1, k, &mut c);
                let _ = rx.lee[k].add_row(&a, &b, &c);
            }
        }
        rx.noise_rows += 1;
    }
    let sigma: Vec<f32> = rx
        .lee
        .iter()
        .filter_map(|n| n.sigma(LEE_MIN_VALUES))
        .collect();
    if sigma.len() < 3 || fin == 0 {
        return;
    }
    let last = fin - 1;
    while rx.filtered < fin && (done || rx.filtered + 2 <= last) {
        let r = rx.filtered;
        let (lo, hi) = (r.saturating_sub(2), (r + 2).min(last));
        for (k, &sg) in sigma.iter().enumerate() {
            rx.window.clear();
            for q in lo..=hi {
                row(&rx.rgb, q, k, &mut rx.window);
            }
            rx.window_out.resize(rx.window.len(), 0);
            if rx
                .plane
                .filter_plane(&rx.window, w, sg, &mut rx.window_out)
                .is_err()
            {
                return;
            }
            let from = (r - lo) as usize * w;
            let base = r as usize * w * 3;
            for x in 0..w {
                rx.shown[base + 3 * x + k] = rx.window_out[from + x];
            }
        }
        rx.filtered += 1;
    }
}

fn picture_of(rx: &Rx, done: bool) -> HfPicture {
    let (w, h) = (rx.m.w, rx.m.h);
    let mut p = HfPicture::new(rx.id, rx.m.name, w, h, rx.start);
    for y in 0..h {
        let src = if y < rx.filtered { &rx.shown } else { &rx.rgb };
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            p.put(x, y, src[i], src[i + 1], src[i + 2]);
        }
    }
    p.rows = rx.rows;
    p.complete = done && rx.next_line >= rx.m.lines();
    p.colour = true;
    p.progress = [rx.rows as u64, h as u64];
    p
}

impl Default for SstvDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl sdrkit_graph::Decoder for SstvDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.sstv"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        SstvSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match SstvSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SstvSetting::PictureFilter => self.filter_on = PICTURE_FILTER.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(SstvSetting::PictureFilter, Value::Bool(self.filter_on))])
    }
    fn results(&self) -> Published {
        crate::typed::results("sstv")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.rx = None;
        self.video = None;
        self.pulses.clear();
        self.state = 0;
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.drop_picture();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let n = self.audio.len() as u64;
        for (i, &s) in x.iter().enumerate() {
            self.audio[((self.ring.count + i as u64) % n) as usize] = s;
        }
        self.ring.push(x);
        if let Some(v) = &mut self.video {
            v.picture.push(x);
            v.sync.push(x);
        }
        self.t += x.len() as u64;
        while self.next_check + 8 <= self.ring.count {
            self.next_check += 8;
            if self.rx.is_some() {
                // Lines are read when their time has passed. Checked once a millisecond.
                if self.next_check.is_multiple_of(80) {
                    self.receive(out);
                }
            } else {
                self.idle();
            }
        }
        if self.state == 2 && self.t > self.last_done + 16000 {
            self.state = 0;
        }
    }
    fn report(&mut self, res: &mut Published) {
        let (mode, vis, h, rows, slant) = match &self.rx {
            Some(rx) => (
                rx.m.name,
                rx.vis,
                rx.m.h,
                rx.rows,
                (rx.period / rx.nominal - 1.0) * 1e6,
            ),
            None => match self.hist.latest() {
                Some(p) => (p.mode, self.last.0, p.height(), p.rows, self.last.1),
                None => ("", -1, 0, 0, 0.0),
            },
        };
        let st = &mut self.status;
        st.lock_state = match self.state {
            1 => LockState::Locked,
            2 => LockState::Tracking,
            _ => LockState::Searching,
        };
        st.offset_hz = Some(self.offset);
        if self.state == 1 {
            put_text(&mut st.summary, format_args!("SSTV {mode} {rows}/{h}"));
        } else if !mode.is_empty() {
            put_text(
                &mut st.summary,
                format_args!("SSTV {mode} {rows}/{h} (done)"),
            );
        } else {
            put_text(&mut st.summary, format_args!("SSTV: waiting for a picture"));
        }
        res.results_mut().set_status(st);
        let has = self.rx.is_some() || self.hist.latest().is_some();
        res.put(&SstvFields {
            mode: has.then_some(mode),
            vis: has.then_some(vis as i64),
            slant_ppm: has.then_some(slant),
            offset_hz: Hertz(self.offset),
        });
        let current = self.rx.as_ref().map(|rx| picture_of(rx, false));
        res.put(&picture::picture_fields(current.as_ref(), &self.hist));
        flush_logs(&mut self.logs, res);
    }
    fn clear(&mut self) {
        self.hist.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.drop_picture();
        if !kept.field("pictures") {
            self.hist.clear();
        }
    }
}

impl SstvDecoder {
    /// Drops the picture being received. The finished ones stay.
    fn drop_picture(&mut self) {
        self.rx = None;
        self.video = None;
        self.pulses.clear();
        self.in_pulse = None;
        self.state = 0;
    }
}

/// The generator. It reads the settings `gen_mode` and `gen_vis`, and sends the test picture.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let m = MODES[usize::from(v.entry(&GEN_MODE)).min(MODES.len() - 1)];
    let vis = v.get(&GEN_VIS);
    let pic = match v.get(&GEN_PICTURE) {
        GenPicture::Card => test_picture(m.w, m.h),
        GenPicture::Texture => texture_picture(m.w, m.h),
    };
    let md = SstvModulator::new(m, cfg.rate_hz, &pic, vis);
    Ok(HfGenerator::boxed(cfg.rate_hz, md, SignalInfo::new(2500.0)))
}

/// The transmit mode: an entry of [`MODES`].
pub const TX_MODE: CatalogueSetting = CatalogueSetting {
    key: "tx_mode",
    label: "Transmit mode",
    help: "The SSTV mode of the pictures sent.",
    choices: || {
        MODES
            .iter()
            .map(|m| ChoiceDecl::new(m.name, m.name))
            .collect()
    },
    default: "Robot 36",
    advanced: false,
};

sdrkit_core::choice_enum! {
    /// How a picture of another shape than the mode is fitted to it.
    pub enum Fit {
        /// The whole picture, its shape kept, with black bars at the sides or at the top and bottom.
        Bars = ("bars", "Keep the shape, black bars"),
        /// The middle of the picture, its shape kept, cut at the sides or at the top and bottom.
        Crop = ("crop", "Keep the shape, crop"),
        /// The whole picture stretched to the mode.
        Stretch = ("stretch", "Stretch"),
    }
}

/// How a picture is fitted to the transmit mode.
pub const FIT: Setting<field::Choice<Fit>> = Setting::choice("fit", "Fit", Fit::Bars).help(
    "How a picture of another shape than the mode is fitted: whole with black bars, cut to the shape of the mode, or \
     stretched.",
);

/// Whether a picture is sent again and again.
pub const REPEAT: Setting<field::Bool> = Setting::new("repeat", "Repeat", "", false).help(
    "Sends the picture again after each pause until stop. Off at the start: a picture is sent once.",
);
/// The pause between two pictures of a repeat.
pub const PAUSE_S: Setting<field::Duration> = Setting::new("pause_s", "Pause between pictures", "s", 1.9)
    .range(0.0, 600.0, 0.1)
    .help("The time between the end of a picture and the start of the next one when repeat is on. The transmitter is off in the pause.");

sdrkit_core::settings! {
    /// The settings of [`SstvSender`].
    pub enum SstvTxSetting {
        /// The transmit mode.
        Mode = TX_MODE,
        /// The fit.
        Fit = FIT,
        /// USB, LSB or NFM, shared with the front end.
        Demod = crate::front::DEMOD,
        /// Repeat.
        Repeat = REPEAT,
        /// The pause between pictures.
        PauseS = PAUSE_S,
    }
}

/// The settings of the transmitter that the receiver does not have.
pub const TX_OWN_SETTINGS: &[&'static dyn sdrkit_core::settings::DeclareSetting] =
    &[&TX_MODE, &FIT, &REPEAT, &PAUSE_S];

sdrkit_core::commands! {
    /// The commands of [`SstvSender`].
    pub enum SstvTxCommand {
        /// Send a picture.
        Send = SSTV_SEND.command,
        /// Stop.
        Stop = SSTV_STOP,
    }
}

/// The level of the transmitted tone, of full scale.
pub const TX_LEVEL: f32 = 0.5;
/// The lowest stream rate the transmitter writes at, Hz.
pub const TX_MIN_RATE: f64 = 8000.0;
/// The peak deviation of the FM form, Hz: the SSTV audio tone moves the carrier by this much (a 12.5 kHz channel).
pub const FM_DEVIATION_HZ: f64 = 2500.0;
/// The largest width or height of a picture given to `send`.
pub const MAX_SIDE: u32 = 8192;

/// Why a picture is not sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit setting is off.
    Off,
    /// A picture is being sent.
    Busy,
    /// The stream rate is below [`TX_MIN_RATE`].
    Rate,
    /// The width or the height is outside 1 to [`MAX_SIDE`].
    Size,
    /// The pixel bytes do not match the width, the height and the pixel format.
    Length,
    /// The demodulator is AM.
    Sideband,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TxError::Off => "the transmitter is off",
            TxError::Busy => "a picture is being sent",
            TxError::Rate => "the transmit rate must be at least 8 kHz",
            TxError::Size => "the width and the height must be 1 to 8192 pixels",
            TxError::Length => {
                "the pixel bytes do not match the width, the height and the pixel format"
            }
            TxError::Sideband => "the picture is sent on USB, LSB or NFM",
        })
    }
}

impl std::error::Error for TxError {}

/// A rectangle of pixels: left, top, width, height.
type Rect = (u32, u32, u32, u32);

/// The picture `pixels` (`width` x `height`, rows top first, no padding, in `format`) fitted to the size of `m` by `fit`, as
/// RGB row by row. Bars: the picture is scaled by the smaller of the two ratios of the sizes and centred, the rest black.
/// Crop: the middle part of the picture with the shape of the mode, scaled by the larger ratio. Stretch: the whole picture
/// to the whole mode. Where the part is larger than its place it is reduced by the area mean of the pixels it covers; where
/// it is smaller, place pixel (x, y) takes pixel (floor(x W / w), floor(y H / h)) of the part, W x H being the part and w x h
/// the place.
///
/// # Errors
///
/// [`TxError::Size`] or [`TxError::Length`].
pub fn fit_picture(
    pixels: &[u8],
    width: u32,
    height: u32,
    format: SstvPixels,
    m: &SstvMode,
    fit: Fit,
) -> Result<Vec<u8>, TxError> {
    let (w, h) = (width, height);
    if !(1..=MAX_SIDE).contains(&w) || !(1..=MAX_SIDE).contains(&h) {
        return Err(TxError::Size);
    }
    let bpp = match format {
        SstvPixels::Rgb8 => 3,
        SstvPixels::Bgra => 4,
        SstvPixels::Grey8 => 1,
    };
    if pixels.len() != w as usize * h as usize * bpp {
        return Err(TxError::Length);
    }
    let (mw, mh) = (m.w, m.h);
    let (fw, fh) = (f64::from(w), f64::from(h));
    let (src, dst): (Rect, Rect) = match fit {
        Fit::Stretch => ((0, 0, w, h), (0, 0, mw, mh)),
        Fit::Bars => {
            let s = (f64::from(mw) / fw).min(f64::from(mh) / fh);
            let dw = ((fw * s).round() as u32).clamp(1, mw);
            let dh = ((fh * s).round() as u32).clamp(1, mh);
            ((0, 0, w, h), ((mw - dw) / 2, (mh - dh) / 2, dw, dh))
        }
        Fit::Crop => {
            let s = (f64::from(mw) / fw).max(f64::from(mh) / fh);
            let sw = ((f64::from(mw) / s).round() as u32).clamp(1, w);
            let sh = ((f64::from(mh) / s).round() as u32).clamp(1, h);
            (((w - sw) / 2, (h - sh) / 2, sw, sh), (0, 0, mw, mh))
        }
    };
    // The part, as a raster of the shared image code (grey or BGRA).
    let (sx, sy, sw, sh) = src;
    let mut part = Vec::with_capacity(sw as usize * sh as usize * bpp.max(4));
    for y in sy..sy + sh {
        let row = &pixels[(y as usize * w as usize + sx as usize) * bpp..][..sw as usize * bpp];
        match format {
            SstvPixels::Rgb8 => {
                for p in row.as_chunks::<3>().0 {
                    part.extend_from_slice(&[p[2], p[1], p[0], 255]);
                }
            }
            SstvPixels::Bgra | SstvPixels::Grey8 => part.extend_from_slice(row),
        }
    }
    let img_format = match format {
        SstvPixels::Grey8 => ImageFormat::Grey8,
        _ => ImageFormat::Bgra,
    };
    let part = Raster::from_vec(img_format, sw, sh, part).map_err(|_| TxError::Length)?;
    let (dx, dy, dw, dh) = dst;
    let small = if sw > dw || sh > dh {
        resize_box(&part, dw.min(sw), dh.min(sh))
    } else {
        part
    };
    let (pw, ph) = (small.width(), small.height());
    let pb = small.bytes_per_pixel();
    let data = small.data();
    let mut rgb = vec![0u8; (mw * mh * 3) as usize];
    for y in 0..dh {
        let py = (u64::from(y) * u64::from(ph) / u64::from(dh)) as usize;
        for x in 0..dw {
            let px = (u64::from(x) * u64::from(pw) / u64::from(dw)) as usize;
            let s = &data[(py * pw as usize + px) * pb..][..pb];
            let c = if pb == 1 {
                [s[0]; 3]
            } else {
                [s[2], s[1], s[0]]
            };
            let i = (((dy + y) * mw + dx + x) * 3) as usize;
            rgb[i..i + 3].copy_from_slice(&c);
        }
    }
    Ok(rgb)
}

/// A picture on air: the tones, and the FM modulator of the NFM form.
struct TxBurst {
    w: SstvWriter,
    fm: Option<sdrkit_algo_sim::modulate::Fm>,
    /// Samples written.
    sent: u64,
}

impl TxBurst {
    /// Writes up to `out.len()` samples: the tones on the upper sideband, mirrored on the lower one, or the carrier at the
    /// dial frequency-modulated by the real audio of the tones. Returns how many.
    fn fill(&mut self, out: &mut [Complex32], lower: bool, audio: &mut Vec<f32>) -> usize {
        let n = self.w.fill(out);
        let out = &mut out[..n];
        if let Some(fm) = &mut self.fm {
            audio.clear();
            audio.extend(out.iter().map(|z| z.re / TX_LEVEL));
            if fm.process(audio, out).is_ok() {
                for z in out.iter_mut() {
                    *z *= TX_LEVEL;
                }
            }
        } else if lower {
            for z in out.iter_mut() {
                *z = z.conj();
            }
        }
        n
    }
}

/// The transmitter (`hfkbd.sstvtx`): input `in` is the session's samples (their rate, pace and dial), output `out` the
/// complex baseband for `tx.sink`, centred on the dial, at [`TX_LEVEL`]: the picture's tones on the upper sideband (USB), their
/// mirror image (LSB), or the carrier at the dial frequency-modulated by the real audio of the tones with a peak deviation of
/// [`FM_DEVIATION_HZ`] (NFM); AM refuses `send`. Output `events` the `sstv.tx` events. `send` fits the picture to the
/// transmit mode ([`fit_picture`]) and sends it: the VIS header, the start pulse of the Scottie modes and the lines
/// ([`SstvSignal`]); with repeat on, the same picture goes again as a new burst each `pause_s` after the end of the last one,
/// until stop. Each pass writes as many samples as the pass brings in. The last sample carries the end mark; `stop`, the
/// transmitter going off and a new format end the picture and the repeat at once, with the end mark on one silent sample on
/// the next pass. On a live radio whose host times bursts
/// (`HostStreams::transmit_lead`), the first sample states its time on air (`TagKind::BurstStart`): the time of the pass
/// that writes it plus the lead of the host and two passes.
pub struct SstvSender {
    mode: usize,
    fit: Fit,
    demod: crate::front::Demod,
    repeat: bool,
    pause_s: f64,
    /// The picture of a repeat and the sample at which it goes again.
    again: Option<(SstvSignal, u64)>,
    /// The picture sent last, kept for a repeat.
    last: Option<SstvSignal>,
    samples: u64,
    audio: Vec<f32>,
    rate: f64,
    dial: f64,
    transmit: bool,
    burst: Option<TxBurst>,
    end_owed: bool,
    events: Vec<sdrkit_core::Event>,
    results: Published,
    /// The samples come from a radio as they are received.
    live: bool,
    /// The clock of the host.
    wall: Clock,
    /// The host, which says how early a burst with a stated start is written.
    host: Option<Arc<dyn HostStreams>>,
}

impl Default for SstvSender {
    fn default() -> Self {
        let mut s = SstvSender {
            mode: usize::from(TX_MODE.default_index()),
            fit: Fit::Bars,
            demod: crate::front::DEMOD.default,
            repeat: REPEAT.default,
            pause_s: PAUSE_S.default,
            again: None,
            last: None,
            samples: 0,
            audio: Vec::new(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            burst: None,
            end_owed: false,
            events: Vec::new(),
            results: Published::new(SstvTxFields::SHAPE.fields),
            live: false,
            wall: Clock::fixed(0.0),
            host: None,
        };
        s.publish();
        s
    }
}

impl SstvSender {
    fn event(&mut self, state: &str, reason: &str, mode: &str) {
        self.events.push(SSTV_TX.event(
            0.0,
            self.dial,
            &SstvTxEvent {
                state,
                reason,
                mode,
                family: SourceFamily::Message,
            },
        ));
    }

    /// The time on air of a burst whose first sample is written in a pass of `pass` seconds of samples: the time of the
    /// pass, the lead of the host and two passes, as the next pass can come one pass late. `None` if the start is not
    /// stated: the radio is not live or the host gives no lead.
    fn start_time(&self, pass: f64) -> Option<f64> {
        let lead = self.host.as_ref()?.transmit_lead().as_secs_f64();
        (self.live && lead > 0.0).then(|| self.wall.now() + lead + 2.0 * pass)
    }

    /// Ends the picture on air and a repeat; its end mark goes out on the next pass.
    fn cut(&mut self, reason: &str) {
        self.again = None;
        self.last = None;
        if let Some(b) = self.burst.take() {
            self.end_owed = true;
            let name = b.w.signal().mode().name;
            self.event("finished", reason, name);
        }
    }

    /// Starts sending `a` in the transmit mode.
    fn send(&mut self, a: &SstvSendArgs) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.burst.is_some() {
            return Err(TxError::Busy);
        }
        if self.rate < TX_MIN_RATE {
            return Err(TxError::Rate);
        }
        let side = |v: i64| u32::try_from(v).map_err(|_| TxError::Size);
        let m = MODES[self.mode];
        if self.demod == crate::front::Demod::Am {
            return Err(TxError::Sideband);
        }
        let rgb = fit_picture(
            &a.pixels,
            side(a.width)?,
            side(a.height)?,
            a.pixel_format,
            &m,
            self.fit,
        )?;
        self.again = None;
        self.start(SstvSignal::new(m, rgb));
        Ok(())
    }

    /// Puts `sig` on air: the tones, or their FM form on NFM.
    fn start(&mut self, sig: SstvSignal) {
        let fm = (self.demod == crate::front::Demod::Nfm)
            .then(|| sdrkit_algo_sim::modulate::Fm::new(self.rate, FM_DEVIATION_HZ).ok())
            .flatten();
        self.last = Some(sig.clone());
        self.burst = Some(TxBurst {
            w: SstvWriter::new(sig, self.rate, TX_LEVEL),
            fm,
            sent: 0,
        });
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let m = MODES[self.mode];
        let picture_s = Seconds(air_time_ms(&m, true, true) / 1000.0);
        let next_s = self
            .again
            .as_ref()
            .map(|a| Seconds(a.1.saturating_sub(self.samples) as f64 / self.rate.max(1.0)));
        let f = match &self.burst {
            None => SstvTxFields {
                tx_sending: false,
                tx_mode: None,
                tx_line: None,
                tx_lines: None,
                tx_picture_s: picture_s,
                tx_next_s: next_s,
            },
            Some(b) => {
                let sent = b.w.signal().mode();
                SstvTxFields {
                    tx_sending: true,
                    tx_mode: Some(sent.name),
                    tx_line: Some(i64::from(b.w.line())),
                    tx_lines: Some(i64::from(sent.lines())),
                    tx_picture_s: picture_s,
                    tx_next_s: None,
                }
            }
        };
        self.results.put(&f);
    }

    /// Writes the end mark on the last sample of output 0, or on one silent sample when the pass wrote none.
    fn end_mark(io: &mut Io) {
        let out = io.iq_out(0);
        if out.is_empty() {
            out.push(Complex32::default());
        }
        let last = out.len() - 1;
        io.output(0).tag_here(last, TagKind::BurstEnd);
    }
}

impl Component for SstvSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfkbd.sstvtx",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SstvTxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.cut("the format changed");
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.live = ctx.live;
        self.wall = ctx.clock.clone();
        self.host.clone_from(&ctx.streams);
        if !ctx.transmit {
            self.cut("the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.end_owed) {
            Self::end_mark(io);
        }
        let room = io.input_len(0);
        if self.burst.is_none() && self.transmit {
            if let Some((sig, at)) = self.again.take() {
                if self.samples >= at {
                    let name = sig.mode().name;
                    self.start(sig);
                    self.event("started", "repeat", name);
                } else {
                    self.again = Some((sig, at));
                }
            }
        }
        let stated = self
            .burst
            .as_ref()
            .filter(|b| b.sent == 0)
            .and_then(|_| self.start_time(room as f64 / self.rate.max(1.0)));
        let lower = self.demod == crate::front::Demod::Lsb;
        if let Some(b) = &mut self.burst {
            let out = io.iq_out(0);
            let first = out.len();
            out.resize(first + room, Complex32::default());
            let n = b.fill(&mut out[first..], lower, &mut self.audio);
            out.truncate(first + n);
            if b.sent == 0 && n > 0 {
                if let Some(unix_s) = stated {
                    io.output(0).tag_here(first, TagKind::BurstStart { unix_s });
                }
            }
            b.sent += n as u64;
            if b.w.finished() {
                Self::end_mark(io);
                if let Some(b) = self.burst.take() {
                    self.event("finished", "", b.w.signal().mode().name);
                }
                if self.repeat {
                    let at = self.samples + n as u64 + (self.pause_s * self.rate).round() as u64;
                    self.again = self.last.take().map(|sig| (sig, at));
                }
            }
        }
        self.samples += room as u64;
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SstvTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SstvTxSetting::Mode => self.mode = TX_MODE.read(id, v)?.min(MODES.len() - 1),
            SstvTxSetting::Fit => self.fit = FIT.read(id, v)?,
            SstvTxSetting::Demod => self.demod = crate::front::DEMOD.read(id, v)?,
            SstvTxSetting::Repeat => {
                self.repeat = REPEAT.read(id, v)?;
                if !self.repeat {
                    self.again = None;
                }
            }
            SstvTxSetting::PauseS => self.pause_s = PAUSE_S.read(id, v)?,
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (SstvTxSetting::Mode, Value::Choice(self.mode as u16)),
            (SstvTxSetting::Fit, Value::Choice(self.fit.index())),
            (SstvTxSetting::Demod, Value::Choice(self.demod.index())),
            (SstvTxSetting::Repeat, Value::Bool(self.repeat)),
            (SstvTxSetting::PauseS, Value::Duration(self.pause_s)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        SstvTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SstvTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SstvTxCommand::Send => {
                let a: SstvSendArgs = SSTV_SEND.args(args)?;
                let name = MODES[self.mode].name;
                match self.send(&a) {
                    Ok(()) => self.event("started", "", name),
                    Err(e) => {
                        self.event("refused", &e.to_string(), name);
                        return Err(CommandError::Failed {
                            source: StageError::new(e),
                        });
                    }
                }
            }
            SstvTxCommand::Stop => self.cut("stopped"),
        }
        self.publish();
        Ok(None)
    }
}

/// The receiver and transmitter of the stack: the family front end, the receiver on its 8 kHz sideband, and the
/// transmitter on the session's samples into the transmit sink. `listen` goes to the front end, `demod` to the front end and
/// the transmitter, the receiver's settings to the receiver, `tx_mode`, `fit`, `repeat` and `pause_s` to the transmitter; `send` and `stop` go to the transmitter, `clear` to
/// every component.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("front", crate::front::HfFront::new())
        .add("dec", DecoderComponent::new(SstvDecoder::new()))
        .add("sender", SstvSender::default())
        .add("tx", TxSink::default())
        .input("front")
        .input("sender.in")
        .connect("front.caudio", "dec.in")
        .connect("sender.out", "tx.in")
        .output("dec.events")
        .output("dec.pictures")
        .output("sender.events")
        .output("front.listen")
        .status_from("dec")
        .clock(0.1);
    let mut g = StackGraph::of(crate::typed::shape("sstv"), b.build()?)
        .min_rate(
            crate::front::MIN_RATE,
            "HF keyboard stacks need a sample rate of at least 12 ksps",
        )
        .route("listen", &["front.listen"])?
        .route("demod", &["front.demod", "sender.demod"])?;
    for s in SstvSetting::schema().settings() {
        let k = &s.field.key;
        g = g.route(k, &[&format!("dec.{k}")])?;
    }
    for s in TX_OWN_SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("sender.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("send", CommandTarget::of("sender", "send"))
        .command("stop", CommandTarget::of("sender", "stop"))
        .with_settings(&cfg.settings)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_layouts_add_up() {
        for m in MODES {
            let total: f64 = m.segments(0).iter().map(|s| s.1).sum();
            assert!(
                (total - m.line_ms).abs() < 0.01,
                "{}: {total} vs {}",
                m.name,
                m.line_ms
            );
        }
    }

    /// The tones of the ten bits after the leaders and the break.
    fn bits(code: u8) -> Vec<f64> {
        let t = vis_tones(code);
        assert_eq!(t.iter().map(|x| x.1).sum::<f64>(), 910.0);
        t[3..].iter().map(|x| x.0).collect()
    }

    #[test]
    fn vis_headers_of_the_worked_examples() {
        assert_eq!(
            bits(8),
            [1200.0, 1300.0, 1300.0, 1300.0, 1100.0, 1300.0, 1300.0, 1300.0, 1100.0, 1200.0]
        );
        assert_eq!(
            bits(44),
            [1200.0, 1300.0, 1300.0, 1100.0, 1100.0, 1300.0, 1100.0, 1300.0, 1100.0, 1200.0]
        );
    }

    #[test]
    fn vis_codes_of_the_mode_description() {
        let table = [
            ("Robot 36", 8),
            ("Robot 72", 12),
            ("Martin M1", 44),
            ("Martin M2", 40),
            ("Scottie S1", 60),
            ("Scottie S2", 56),
            ("Scottie DX", 76),
            ("PD50", 93),
            ("PD90", 99),
            ("PD120", 95),
            ("PD180", 96),
            ("Wraase SC2-180", 55),
        ];
        for (name, code) in table {
            assert_eq!(mode(name).map(|m| m.vis), Some(code), "{name}");
        }
    }

    /// Worked example 3: a red pixel in Robot 36 is Y 76.2 (1739.2 Hz), R-Y 255 (2300 Hz), B-Y 85.2 (1767.2 Hz); in Martin
    /// M1 it is G and B at 1500 Hz and R at 2300 Hz, each for 0.4576 ms.
    #[test]
    fn a_red_pixel_as_the_specification_codes_it() {
        let [y, ry, by] = rgb_to_ycc([255, 0, 0]);
        assert!((y - 76.2).abs() < 0.05 && ry == 255.0 && (by - 85.2).abs() < 0.05);
        assert!((level_hz(y) - 1739.2).abs() < 0.1);
        assert!((level_hz(by) - 1767.2).abs() < 0.1);
        let m = mode("Martin M1").unwrap();
        let mut red = vec![0u8; (m.w * m.h * 3) as usize];
        red.as_chunks_mut::<3>()
            .0
            .iter_mut()
            .for_each(|p| p[0] = 255);
        let mut t = Vec::new();
        SstvSignal::new(m, red).line(0, &mut t);
        let scans: Vec<&(f64, f64)> = t.iter().filter(|x| (x.1 - 0.4576).abs() < 1e-9).collect();
        assert_eq!(scans.len(), 960);
        assert!(scans[..640].iter().all(|x| x.0 == 1500.0));
        assert!(scans[640..].iter().all(|x| x.0 == 2300.0));
    }

    /// The chroma of Robot 36 is 160 tones of 0.275 ms, each the mean of a 2 x 2 group; Robot 72 sends R-Y and B-Y of 160
    /// tones of 0.43125 ms, each the mean of two columns.
    #[test]
    fn robot_chroma_is_half_width() {
        for (name, rows, count) in [("Robot 36", 2, 160), ("Robot 72", 1, 320)] {
            let m = mode(name).unwrap();
            // Columns alternate black and white; on Robot 36 the second row of each pair is black.
            let mut pic = vec![0u8; (m.w * m.h * 3) as usize];
            for y in (0..m.h).step_by(rows) {
                for x in (1..m.w).step_by(2) {
                    let i = ((y * m.w + x) * 3) as usize;
                    pic[i..i + 3].fill(255);
                }
            }
            let mut t = Vec::new();
            SstvSignal::new(m, pic).line(0, &mut t);
            // Sync and porch, then 320 tones of Y.
            let y = &t[2..322];
            assert!(
                y.iter().all(|x| (x.1 - m.scan_ms / 320.0).abs() < 1e-12),
                "{name}"
            );
            let rest = &t[322..];
            let chroma: Vec<&(f64, f64)> = rest.iter().filter(|x| x.1 < 1.0).collect();
            let per = if rows == 2 {
                44.0 / 160.0
            } else {
                69.0 / 160.0
            };
            assert_eq!(chroma.len(), count, "{name}");
            assert!(chroma.iter().all(|x| (x.1 - per).abs() < 1e-12), "{name}");
            // Grey has no colour: R-Y and B-Y are 128.
            assert!(
                chroma.iter().all(|x| (x.0 - level_hz(128.0)).abs() < 1e-9),
                "{name}"
            );
        }
    }

    #[test]
    fn colour_round_trips() {
        for p in [
            [255u8, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [128, 128, 128],
            [200, 60, 40],
        ] {
            let [y, ry, by] = rgb_to_ycc(p);
            let q = ycc_to_rgb(y, ry, by);
            for k in 0..3 {
                assert!((p[k] as i32 - q[k] as i32).abs() <= 3, "{p:?} {q:?}");
            }
        }
    }
}

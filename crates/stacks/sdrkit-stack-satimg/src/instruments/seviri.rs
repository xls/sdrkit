//! The SEVIRI packets of the raw downlink of Meteosat Second Generation: one step of the scan mirror is 16 packets, eleven of
//! three lines of a standard channel and five of the high-resolution visible channel (HRV). The packets carry a time, which
//! places their lines in the 15-minute repeat cycle. The pictures are reduced: every second standard line and pixel, every
//! third HRV line and pixel, north at the top and west at the left.

use sdrkit_codec_image::strip::Strip;
use std::sync::Arc;

/// The virtual channel of the SEVIRI packets.
pub const VC: u8 = 0;
/// The APID of the SEVIRI packets.
pub const APID: u16 = 2046;
/// Bytes of user data a packet needs (the test signal sends one more).
pub const MIN_USER: usize = 14_385;
/// Bytes of user data of a test packet.
pub const USER_BYTES: usize = 14_386;
/// The first byte of the pixels in the user data.
pub const PIXELS_AT: usize = 8;
/// Pixels of a standard line.
pub const LINE: usize = 3834;
/// Pixels of an HRV line.
pub const HRV_LINE: usize = 5751;
/// Parts of a step.
pub const PARTS: u8 = 16;
/// The standard channels: parts 0 to 10.
pub const CHANNELS: u8 = 11;
/// Seconds of a repeat cycle.
pub const CYCLE_S: f64 = 900.0;
/// Standard lines of a cycle.
pub const LINES: u32 = 4482;
/// Standard lines a second: 1494 in 300 s.
pub const LINES_PER_S: f64 = 1494.0 / 300.0;
/// HRV lines kept: below this.
pub const HRV_LINES: u32 = 13_500;
/// Width of both reduced pictures.
pub const WIDTH: usize = 1917;
/// Rows of the reduced SEVIRI picture.
pub const ROWS: u32 = 2241;
/// Rows of the reduced HRV picture.
pub const HRV_ROWS: u32 = 4500;
/// A part 0 line this much below the last one starts a new disc.
pub const NEW_DISC_LINES: u32 = 1000;
/// The instrument name of the standard picture.
pub const SEVIRI: &str = "SEVIRI";
/// The instrument name of the HRV picture.
pub const HRV: &str = "SEVIRI HRV";

/// The two clock numbers of a satellite number: the day offset and the extra seconds. `None` for a number whose packets are
/// not placed.
pub fn clock(satellite: u16) -> Option<(u32, u32)> {
    match satellite {
        322 => Some((19_559, 6442)),
        323 => Some((18_249, 34_738)),
        324 => Some((18_980, 42_207)),
        _ => None,
    }
}

/// The parts whose three lines are stored in the opposite order.
pub fn reversed(part: u8) -> bool {
    matches!(part, 3 | 4 | 8 | 9 | 10)
}

/// The place in the repeat cycle, s, of the 48-bit time `time` (units of 1/65 536 s) of satellite `satellite`.
pub fn cycle_pos(satellite: u16, time: u64) -> Option<f64> {
    let (day, extra) = clock(satellite)?;
    let t = f64::from(day) * 86_400.0 + time as f64 / 65_536.0 + f64::from(extra);
    Some(t.rem_euclid(CYCLE_S))
}

/// The first line of a packet of part `part` at cycle place `pos`: a standard line for parts 0 to 10, an HRV line for the
/// others.
pub fn first_line(part: u8, pos: f64) -> u32 {
    if part < CHANNELS {
        (pos * LINES_PER_S).floor() as u32
    } else {
        (pos * 3.0 * LINES_PER_S).floor() as u32 + 2 * u32::from(part - CHANNELS)
    }
}

/// The row of standard line `r` in the reduced picture: only even lines are kept, from the bottom.
pub fn row(r: u32) -> Option<u32> {
    (r.is_multiple_of(2) && r < LINES).then(|| ROWS - 1 - r / 2)
}

/// The row of HRV line `r` in the reduced picture: only lines that are a multiple of 3 are kept, from the bottom.
pub fn hrv_row(r: u32) -> Option<u32> {
    (r.is_multiple_of(3) && r < HRV_LINES).then(|| HRV_ROWS - 1 - r / 3)
}

/// The 10-bit pixel `i` of a packed run of pixels, most significant bit first.
pub fn pixel(px: &[u8], i: usize) -> u16 {
    let bit = 10 * i;
    let at = bit / 8;
    let v = u16::from(*px.get(at).unwrap_or(&0)) << 8 | u16::from(*px.get(at + 1).unwrap_or(&0));
    (v >> (6 - bit % 8)) & 0x3FF
}

/// The packed bytes of 10-bit pixels, most significant bit first.
pub fn pack(px: &[u16], out: &mut [u8]) {
    for (i, &v) in px.iter().enumerate() {
        for k in 0..10 {
            let b = 10 * i + k;
            if b / 8 < out.len() {
                out[b / 8] |= (((v >> (9 - k)) & 1) as u8) << (7 - b % 8);
            }
        }
    }
}

/// The user data of the test packet of part `part` for standard line `line` of satellite `satellite`: the time that puts the
/// line at a quarter of its period, and the pixels of `value(c, x, y)` with y taken modulo `lines`.
pub fn test_user(
    satellite: u16,
    part: u8,
    line: u32,
    lines: u32,
    value: impl Fn(u16, u32, u32) -> u16,
) -> Vec<u8> {
    let mut u = vec![0u8; USER_BYTES];
    let (day, extra) = clock(satellite).unwrap_or((0, 0));
    let base = (f64::from(day) * 86_400.0 + f64::from(extra)).rem_euclid(CYCLE_S);
    let pos = (f64::from(line) + 0.25) / LINES_PER_S;
    let time = ((pos - base).rem_euclid(CYCLE_S) * 65_536.0).round() as u64;
    u[..6].copy_from_slice(&time.to_be_bytes()[2..]);
    let mut px = Vec::with_capacity(3 * LINE);
    if part < CHANNELS {
        for k in 0..3 {
            let j = if reversed(part) { 2 - k } else { k };
            let y = (line + j) % lines;
            px.extend((0..LINE as u32).map(|x| value(u16::from(part), x, y)));
        }
    } else {
        let n = if part == PARTS - 1 { 1 } else { 2 };
        for k in 0..n {
            let y = (3 * line + 2 * u32::from(part - CHANNELS) + k) % lines;
            px.extend((0..HRV_LINE as u32).map(|x| value(u16::from(CHANNELS), x, y)));
        }
    }
    pack(&px, &mut u[PIXELS_AT..]);
    u
}

/// Counters of [`SeviriDecoder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeviriStats {
    /// SEVIRI packets taken.
    pub packets: u64,
    /// Packets too short for their part, or of a satellite without clock numbers.
    pub refused: u64,
    /// Lines of part 0 stored.
    pub lines: u64,
    /// Rows of single channels written to the SEVIRI picture.
    pub rows: u64,
    /// Rows written to the HRV picture.
    pub hrv_rows: u64,
    /// Discs finished.
    pub discs: u64,
}

/// SEVIRI packets to the strips of the two reduced pictures. A packet of part 0 whose line is more than 1000 lines below
/// the last one starts a new disc: the strips that follow belong to new pictures.
#[derive(Clone, Debug, Default)]
pub struct SeviriDecoder {
    last_line: Option<u32>,
    picture: u32,
    /// The satellite number of the last packet.
    pub satellite: Option<u16>,
    /// A disc finished since the flag was last cleared.
    pub disc_done: bool,
    /// Counters.
    pub stats: SeviriStats,
}

impl SeviriDecoder {
    /// Empties the disc in progress and forgets the last line.
    pub fn new_picture(&mut self) {
        self.last_line = None;
        self.picture += 1;
        self.stats.lines = 0;
        self.stats.rows = 0;
        self.stats.hrv_rows = 0;
    }

    /// Takes the user data of a SEVIRI packet with sequence count `seq` from satellite `satellite` and appends its strips to
    /// `out`.
    pub fn push(&mut self, satellite: u16, seq: u16, user: &[u8], out: &mut Vec<Strip>) {
        self.satellite = Some(satellite);
        let part = (seq & 15) as u8;
        let need = PIXELS_AT
            + if part < CHANNELS {
                (3 * LINE * 10).div_ceil(8)
            } else if part < PARTS - 1 {
                (2 * HRV_LINE * 10).div_ceil(8)
            } else {
                (HRV_LINE * 10).div_ceil(8)
            };
        let pos = user
            .get(..6)
            .map(|t| t.iter().fold(0u64, |v, &b| (v << 8) | u64::from(b)))
            .and_then(|t| cycle_pos(satellite, t));
        let Some(pos) = pos.filter(|_| user.len() >= need.max(MIN_USER)) else {
            self.stats.refused += 1;
            return;
        };
        self.stats.packets += 1;
        let l = first_line(part, pos);
        let px = &user[PIXELS_AT..];
        if part < CHANNELS {
            if part == 0 {
                if self.last_line.is_some_and(|last| l + NEW_DISC_LINES < last) {
                    self.stats.discs += 1;
                    self.disc_done = true;
                    self.new_picture();
                }
                self.last_line = Some(l);
            }
            for j in 0..3u32 {
                let Some(r) = row(l + j) else { continue };
                let k = if reversed(part) { 2 - j } else { j } as usize;
                let samples: Vec<u8> = (0..WIDTH)
                    .map(|x| (pixel(px, k * LINE + 2 * (WIDTH - 1 - x)) >> 2) as u8)
                    .collect();
                out.push(self.strip(SEVIRI, u16::from(part) + 1, r, samples));
                self.stats.rows += 1;
                if part == 0 {
                    self.stats.lines += 1;
                }
            }
        } else {
            let n = if part == PARTS - 1 { 1 } else { 2 };
            for k in 0..n {
                let Some(r) = hrv_row(l + k) else { continue };
                let k = k as usize;
                let samples: Vec<u8> = (0..WIDTH)
                    .map(|x| (pixel(px, k * HRV_LINE + 3 * (WIDTH - 1 - x)) >> 2) as u8)
                    .collect();
                out.push(self.strip(HRV, 1, r, samples));
                self.stats.hrv_rows += 1;
            }
        }
    }

    fn strip(&self, instrument: &'static str, channel: u16, row: u32, samples: Vec<u8>) -> Strip {
        let mut s = Strip::new(channel, row, 0, 1, WIDTH as u32, Arc::from(samples));
        s.picture = self.picture;
        s.instrument = instrument;
        s
    }
}

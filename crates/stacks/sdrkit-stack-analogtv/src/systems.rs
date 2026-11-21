//! The television systems: the RF layout, the line and field timing and the colour parameters of each system.
//!
//! Standard: ITU-R BT.470-6 Tables 1, 1-1, 1-2, 2 and 3.

/// A raster: lines, timing and levels. Times are microseconds after 0H.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raster {
    /// Lines a frame.
    pub lines: u32,
    /// Line frequency, Hz.
    pub line_hz: f64,
    /// Front porch.
    pub front_porch_us: f64,
    /// Blanking end (start of the active line).
    pub blank_end_us: f64,
    /// Active time.
    pub active_us: f64,
    /// Line sync width.
    pub sync_us: f64,
    /// Sync edge 10-90 % time.
    pub sync_edge_us: f64,
    /// Blanking edge 10-90 % time.
    pub blank_edge_us: f64,
    /// Equalising pulse width (0: none).
    pub eq_us: f64,
    /// Equalising pulses a sequence.
    pub eq_count: u32,
    /// Broad pulse width.
    pub broad_us: f64,
    /// Broad pulses a field.
    pub broad_count: u32,
    /// Sync level against white = 1 (negative).
    pub sync_level: f64,
    /// Black setup above blanking.
    pub setup: f64,
    /// Half-line index of the first broad pulse of field 1 and field 2.
    pub first_broad: [u32; 2],
    /// First visible line of field 1 and field 2 (1-based).
    pub first_visible: [u32; 2],
    /// Visible rows a field.
    pub rows: u32,
    /// Picture width and height.
    pub width: u32,
    /// Picture height.
    pub height: u32,
}

impl Raster {
    /// Line period, microseconds.
    pub fn line_us(&self) -> f64 {
        1e6 / self.line_hz
    }
    /// Fields a second.
    pub fn field_hz(&self) -> f64 {
        2.0 * self.line_hz / self.lines as f64
    }
    /// The raster group the sync search uses (625 and 525 share one).
    pub fn group(&self) -> RasterGroup {
        match self.lines {
            405 => RasterGroup::L405,
            819 => RasterGroup::L819,
            _ => RasterGroup::L625,
        }
    }
    /// What half-line `h` (0 .. 2 lines) carries.
    pub fn half_line_pulse(&self, h: u32) -> Pulse {
        let n2 = 2 * self.lines;
        let h = h % n2;
        for &fb in &self.first_broad {
            let d = (h + n2 - fb) % n2;
            if d < self.broad_count {
                return Pulse::Broad;
            }
            if self.eq_count > 0 {
                if d >= self.broad_count && d < self.broad_count + self.eq_count {
                    return Pulse::Equalising;
                }
                let before = (fb + n2 - h) % n2;
                if before >= 1 && before <= self.eq_count {
                    return Pulse::Equalising;
                }
            }
        }
        if h.is_multiple_of(2) {
            Pulse::Line
        } else {
            Pulse::None
        }
    }
    /// The 405 or 819 line rasters (no equalising pulses, wider sync).
    pub fn is_old(&self) -> bool {
        self.lines == 405 || self.lines == 819
    }
    /// The standard line frequency's line count text ("625/50").
    pub fn label(&self) -> &'static str {
        match self.lines {
            525 => "525/59.94",
            405 => "405/50",
            819 => "819/50",
            _ => "625/50",
        }
    }
    /// The visible row of line `line` (1-based), and whether it is in field 2.
    pub fn row_of(&self, line: u32) -> Option<(u32, bool)> {
        for f in 0..2 {
            let first = self.first_visible[f];
            if line >= first && line < first + self.rows {
                return Some((2 * (line - first) + f as u32, f == 1));
            }
        }
        None
    }
}

/// What a half-line carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pulse {
    /// A line sync.
    Line,
    /// An equalising pulse.
    Equalising,
    /// A broad (field-synchronising) pulse.
    Broad,
    /// Nothing (odd half-lines of picture lines).
    None,
}

/// The raster groups of the line search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RasterGroup {
    /// 625 and 525 lines.
    L625,
    /// 405 lines.
    L405,
    /// 819 lines.
    L819,
}

impl RasterGroup {
    /// Nominal line period, sync width and accepted ranges (µs): (line, sync, sync min, sync max, line min, line max).
    pub fn limits(self) -> (f64, f64, f64, f64, f64, f64) {
        match self {
            RasterGroup::L625 => (64.0, 4.7, 3.9, 5.6, 62.5, 65.5),
            RasterGroup::L405 => (98.765, 9.0, 7.4, 10.8, 96.0, 101.5),
            RasterGroup::L819 => (48.840, 2.5, 1.9, 3.2, 47.3, 50.3),
        }
    }
    /// All groups.
    pub const ALL: [RasterGroup; 3] = [RasterGroup::L625, RasterGroup::L405, RasterGroup::L819];
}

/// 625 lines, 50 fields.
pub const R625: Raster = Raster {
    lines: 625,
    line_hz: 15_625.0,
    front_porch_us: 1.5,
    blank_end_us: 10.5,
    active_us: 52.0,
    sync_us: 4.7,
    sync_edge_us: 0.2,
    blank_edge_us: 0.3,
    eq_us: 2.35,
    eq_count: 5,
    broad_us: 27.3,
    broad_count: 5,
    sync_level: -3.0 / 7.0,
    setup: 0.0,
    first_broad: [0, 625],
    first_visible: [23, 336],
    rows: 288,
    width: 768,
    height: 576,
};

/// 525 lines, 59.94 fields.
pub const R525: Raster = Raster {
    lines: 525,
    line_hz: 4.5e6 / 286.0,
    front_porch_us: 1.5,
    blank_end_us: 9.4,
    active_us: 52.656,
    sync_us: 4.7,
    sync_edge_us: 0.14,
    blank_edge_us: 0.14,
    eq_us: 2.3,
    eq_count: 6,
    broad_us: 27.1,
    broad_count: 6,
    sync_level: -0.4,
    setup: 0.075,
    first_broad: [6, 531],
    first_visible: [22, 285],
    rows: 240,
    width: 640,
    height: 480,
};

/// 405 lines (system A).
pub const R405: Raster = Raster {
    lines: 405,
    line_hz: 10_125.0,
    front_porch_us: 1.75,
    blank_end_us: 16.25,
    active_us: 1e6 / 10_125.0 - 18.0,
    sync_us: 9.0,
    sync_edge_us: 0.25,
    blank_edge_us: 0.3,
    eq_us: 0.0,
    eq_count: 0,
    broad_us: 40.0,
    broad_count: 8,
    sync_level: -3.0 / 7.0,
    setup: 0.0,
    first_broad: [0, 405],
    first_visible: [15, 218],
    rows: 188,
    width: 504,
    height: 376,
};

/// 819 lines (system E).
pub const R819: Raster = Raster {
    lines: 819,
    line_hz: 20_475.0,
    front_porch_us: 0.6,
    blank_end_us: 8.9,
    active_us: 1e6 / 20_475.0 - 9.5,
    sync_us: 2.5,
    sync_edge_us: 0.1,
    blank_edge_us: 0.12,
    eq_us: 0.0,
    eq_count: 0,
    broad_us: 20.0,
    broad_count: 8,
    sync_level: -1.0 / 3.0,
    setup: 0.0,
    first_broad: [0, 819],
    first_visible: [26, 436],
    rows: 384,
    width: 1024,
    height: 768,
};

/// The television systems.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemId {
    /// B (7 MHz VHF).
    B,
    /// G (8 MHz UHF).
    G,
    /// H.
    H,
    /// I.
    I,
    /// D/K.
    DK,
    /// K'.
    Kp,
    /// M.
    M,
    /// N.
    N,
    /// A (405 lines).
    A,
    /// C.
    C,
    /// E (819 lines).
    E,
    /// L.
    L,
    /// L' (band I, turned round).
    Lp,
}

sdrkit_core::choice_enum! {
    /// A colour system.
    pub enum Colour {
        /// PAL at 4.43 MHz.
        Pal = ("pal", "PAL"),
        /// PAL-M (525 lines, 3.576 MHz).
        PalM = ("pal_m", "PAL-M"),
        /// PAL-N (625 lines, 3.582 MHz).
        PalN = ("pal_n", "PAL-N"),
        /// NTSC (525 lines, 3.58 MHz).
        Ntsc = ("ntsc", "NTSC"),
        /// NTSC on 625 lines at 3.582 MHz.
        NtscN = ("ntsc_n", "NTSC-N"),
        /// NTSC at 4.43 MHz on 625 lines.
        Ntsc443 = ("ntsc443", "NTSC 4.43"),
        /// SECAM.
        Secam = ("secam", "SECAM"),
        /// No colour.
        Mono = ("mono", "mono"),
    }
}

/// Colour subcarrier parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColourParams {
    /// Subcarrier, Hz.
    pub fsc: f64,
    /// Burst start, µs after 0H.
    pub burst_start_us: f64,
    /// Burst cycles.
    pub burst_cycles: f64,
    /// Burst peak amplitude (white = 1).
    pub burst_amp: f64,
    /// PAL V switch.
    pub v_switch: bool,
}

/// SECAM rest frequencies and deviations.
pub mod secam {
    /// f0R = 282 fH.
    pub const F0R: f64 = 4_406_250.0;
    /// f0B = 272 fH.
    pub const F0B: f64 = 4_250_000.0;
    /// Deviation of D'R per unit.
    pub const DEV_R: f64 = 280e3;
    /// Deviation of D'B per unit.
    pub const DEV_B: f64 = 230e3;
    /// The bell centre.
    pub const F_BELL: f64 = 4_286_000.0;
    /// D'R = -1.902 (R - Y).
    pub const KR: f64 = -1.902;
    /// D'B = 1.505 (B - Y).
    pub const KB: f64 = 1.505;
    /// Lower and upper frequency limits.
    pub const F_MIN: f64 = 3_900_000.0;
    /// Upper limit.
    pub const F_MAX: f64 = 4_756_000.0;
    /// Low-frequency pre-emphasis corner (85 kHz).
    pub const F1: f64 = 85e3;
    /// Anti-bell amplitude at f0 against the luminance amplitude (2 M0 = 23 %).
    pub const M0: f64 = 0.115;
    /// The bell's Q factor.
    pub const BELL_Q: f64 = 16.0;
}

impl Colour {
    /// The subcarrier parameters (`None` for SECAM and mono).
    pub fn params(self) -> Option<ColourParams> {
        let fh625 = 15_625.0;
        let fh525 = 4.5e6 / 286.0;
        Some(match self {
            Colour::Pal => ColourParams {
                fsc: (1135.0 / 4.0 + 1.0 / 625.0) * fh625,
                burst_start_us: 5.6,
                burst_cycles: 10.0,
                burst_amp: 3.0 / 14.0,
                v_switch: true,
            },
            Colour::PalM => ColourParams {
                fsc: 909.0 / 4.0 * fh525,
                burst_start_us: 5.3,
                burst_cycles: 9.0,
                burst_amp: 0.2,
                v_switch: true,
            },
            Colour::PalN => ColourParams {
                fsc: (917.0 / 4.0 + 1.0 / 625.0) * fh625,
                burst_start_us: 5.6,
                burst_cycles: 9.0,
                burst_amp: 3.0 / 14.0,
                v_switch: true,
            },
            Colour::Ntsc => ColourParams {
                fsc: 455.0 / 2.0 * fh525,
                burst_start_us: 5.3,
                burst_cycles: 9.0,
                burst_amp: 0.2,
                v_switch: false,
            },
            Colour::NtscN => ColourParams {
                fsc: (917.0 / 4.0 + 1.0 / 625.0) * fh625,
                burst_start_us: 5.6,
                burst_cycles: 9.0,
                burst_amp: 3.0 / 14.0,
                v_switch: false,
            },
            Colour::Ntsc443 => ColourParams {
                fsc: (1135.0 / 4.0 + 1.0 / 625.0) * fh625,
                burst_start_us: 5.6,
                burst_cycles: 10.0,
                burst_amp: 3.0 / 14.0,
                v_switch: false,
            },
            Colour::Secam | Colour::Mono => return None,
        })
    }
    /// The name results report.
    pub fn name(self) -> &'static str {
        match self {
            Colour::Pal => "PAL",
            Colour::PalM => "PAL-M",
            Colour::PalN => "PAL-N",
            Colour::Ntsc => "NTSC",
            Colour::NtscN => "NTSC-N",
            Colour::Ntsc443 => "NTSC 4.43",
            Colour::Secam => "SECAM",
            Colour::Mono => "mono",
        }
    }
    /// Whether it uses a 3.58 MHz subcarrier on 625 lines (decides system N).
    pub fn is_n(self) -> bool {
        matches!(self, Colour::PalN | Colour::NtscN)
    }
}

/// One system's RF layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System {
    /// Which.
    pub id: SystemId,
    /// The letter as names show it ("B/G", "D/K", "K'", "L'").
    pub letter: &'static str,
    /// The raster.
    pub raster: Raster,
    /// Channel width, MHz.
    pub channel_mhz: f64,
    /// Vision carrier above the lower channel edge, MHz.
    pub vision_above_mhz: f64,
    /// Sound spacing from vision, MHz (negative below the carrier).
    pub spacing_mhz: f64,
    /// Video bandwidth, MHz.
    pub video_mhz: f64,
    /// Vestigial sideband width, MHz.
    pub vestige_mhz: f64,
    /// Positive vision modulation.
    pub positive: bool,
    /// AM sound.
    pub am_sound: bool,
    /// FM sound deviation, Hz.
    pub deviation_hz: f64,
    /// Sound de-emphasis, µs.
    pub deemph_us: f64,
    /// Peak white carrier fraction (negative modulation).
    pub peak_white: f64,
    /// Sound carrier against vision, dB.
    pub sound_db: f64,
    /// The colour by default.
    pub default_colour: Colour,
}

impl System {
    /// The system.
    pub fn get(id: SystemId) -> System {
        let neg = |id, letter, ch, sp, video, vest, white, sdb, colour| System {
            id,
            letter,
            raster: R625,
            channel_mhz: ch,
            vision_above_mhz: 1.25,
            spacing_mhz: sp,
            video_mhz: video,
            vestige_mhz: vest,
            positive: false,
            am_sound: false,
            deviation_hz: 50e3,
            deemph_us: 50.0,
            peak_white: white,
            sound_db: sdb,
            default_colour: colour,
        };
        let pos = |id, letter, raster, ch, above, sp, video, vest, colour| System {
            id,
            letter,
            raster,
            channel_mhz: ch,
            vision_above_mhz: above,
            spacing_mhz: sp,
            video_mhz: video,
            vestige_mhz: vest,
            positive: true,
            am_sound: true,
            deviation_hz: 0.0,
            deemph_us: 0.0,
            peak_white: 1.0,
            sound_db: -10.0,
            default_colour: colour,
        };
        match id {
            SystemId::B => neg(id, "B/G", 7.0, 5.5, 5.0, 0.75, 0.125, -13.0, Colour::Pal),
            SystemId::G => neg(id, "B/G", 8.0, 5.5, 5.0, 0.75, 0.125, -13.0, Colour::Pal),
            SystemId::H => neg(id, "H", 8.0, 5.5, 5.0, 1.25, 0.125, -13.0, Colour::Pal),
            SystemId::I => neg(id, "I", 8.0, 5.9996, 5.5, 1.25, 0.2, -10.0, Colour::Pal),
            SystemId::DK => neg(id, "D/K", 8.0, 6.5, 6.0, 0.75, 0.125, -13.0, Colour::Pal),
            SystemId::Kp => neg(id, "K'", 8.0, 6.5, 6.0, 1.25, 0.125, -13.0, Colour::Secam),
            SystemId::M => System {
                raster: R525,
                deviation_hz: 25e3,
                deemph_us: 75.0,
                ..neg(id, "M", 6.0, 4.5, 4.2, 0.75, 0.125, -10.0, Colour::Ntsc)
            },
            SystemId::N => System {
                deviation_hz: 25e3,
                deemph_us: 75.0,
                ..neg(id, "N", 6.0, 4.5, 4.2, 0.75, 0.125, -10.0, Colour::PalN)
            },
            SystemId::A => pos(id, "A", R405, 5.0, 3.75, -3.5, 3.0, 0.75, Colour::Mono),
            SystemId::C => pos(id, "C", R625, 7.0, 1.25, 5.5, 5.0, 0.75, Colour::Mono),
            SystemId::E => pos(id, "E", R819, 14.0, 2.25, 11.15, 10.0, 2.0, Colour::Mono),
            SystemId::L => pos(id, "L", R625, 8.0, 1.25, 6.5, 6.0, 1.25, Colour::Secam),
            SystemId::Lp => pos(id, "L'", R625, 8.0, 6.75, -6.5, 6.0, 1.25, Colour::Secam),
        }
    }
    /// The vision carrier's position from the channel centre, MHz.
    pub fn vision_from_centre_mhz(&self) -> f64 {
        -(self.channel_mhz / 2.0 - self.vision_above_mhz)
    }
    /// Turned round (main sideband and sound below the carrier).
    pub fn turned_round(&self) -> bool {
        self.spacing_mhz < 0.0
    }
    /// Blanking level against the peak carrier (positive modulation).
    pub fn positive_blanking(&self) -> f64 {
        if self.id == SystemId::E {
            0.25
        } else {
            0.30
        }
    }
    /// The NICAM carrier above vision, Hz (0 where the system has none).
    pub fn nicam_hz(&self) -> f64 {
        match self.id {
            SystemId::I => 6.552e6,
            SystemId::B | SystemId::G | SystemId::H | SystemId::L | SystemId::DK | SystemId::Kp => {
                5.85e6
            }
            _ => 0.0,
        }
    }
    /// Whether colour `c` exists on this system (the generator refuses the rest).
    pub fn allows(&self, c: Colour) -> bool {
        if c == Colour::Mono {
            return true;
        }
        if self.raster.is_old() {
            return false;
        }
        if self.positive && !matches!(c, Colour::Secam | Colour::Pal) {
            return false;
        }
        let r525 = self.raster.lines == 525;
        match c {
            Colour::Ntsc | Colour::PalM => r525,
            Colour::Secam => true,
            Colour::Pal | Colour::PalN | Colour::NtscN | Colour::Ntsc443 => !r525,
            Colour::Mono => true,
        }
    }
    /// The BT.470 style name of this system with colour `c` ("PAL B/G 625/50", "A 405/50 mono").
    pub fn name(&self, c: Colour) -> String {
        let r = self.raster.label();
        match c {
            Colour::Mono => format!("{} {r} mono", self.letter),
            Colour::PalM => format!("PAL-M {r}"),
            Colour::PalN => format!("PAL-N {r}"),
            Colour::NtscN => format!("NTSC-N {r}"),
            Colour::Ntsc443 => format!("NTSC 4.43 {} {r}", self.letter),
            Colour::Pal => format!("PAL {} {r}", self.letter),
            Colour::Ntsc => format!("NTSC {} {r}", self.letter),
            Colour::Secam => format!("SECAM {} {r}", self.letter),
        }
    }
    /// The system for a setting value ("b", "g", "i", "dk", "m", "n", "a", "c", "e", "h", "kp", "l", "lp").
    pub fn from_key(key: &str) -> Option<SystemId> {
        Some(match key {
            "b" => SystemId::B,
            "g" | "bg" => SystemId::G,
            "h" => SystemId::H,
            "i" => SystemId::I,
            "dk" => SystemId::DK,
            "kp" => SystemId::Kp,
            "m" => SystemId::M,
            "n" => SystemId::N,
            "a" => SystemId::A,
            "c" => SystemId::C,
            "e" => SystemId::E,
            "l" => SystemId::L,
            "lp" => SystemId::Lp,
            _ => return None,
        })
    }
}

/// Every system.
pub const ALL_SYSTEMS: [SystemId; 13] = [
    SystemId::B,
    SystemId::G,
    SystemId::H,
    SystemId::I,
    SystemId::DK,
    SystemId::Kp,
    SystemId::M,
    SystemId::N,
    SystemId::A,
    SystemId::C,
    SystemId::E,
    SystemId::L,
    SystemId::Lp,
];

/// The system identified from what was found.
pub fn identify(lines: u32, positive: bool, spacing_mhz: f64, turned: bool) -> SystemId {
    match lines {
        525 => return SystemId::M,
        405 => return SystemId::A,
        819 => return SystemId::E,
        _ => {}
    }
    let near = |x: f64| (spacing_mhz.abs() - x).abs() < 0.2;
    if positive {
        if turned {
            SystemId::Lp
        } else if near(5.5) {
            SystemId::C
        } else {
            SystemId::L
        }
    } else if near(6.0) {
        SystemId::I
    } else if near(6.5) {
        SystemId::DK
    } else if near(4.5) {
        SystemId::N
    } else {
        SystemId::G
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_lines_carry_the_standard_pulses() {
        let count = |r: &Raster, p| {
            (0..2 * r.lines)
                .filter(|&h| r.half_line_pulse(h) == p)
                .count()
        };
        assert_eq!(count(&R625, Pulse::Broad), 10);
        assert_eq!(count(&R625, Pulse::Equalising), 20);
        assert_eq!(count(&R525, Pulse::Broad), 12);
        assert_eq!(count(&R525, Pulse::Equalising), 24);
        assert_eq!(count(&R405, Pulse::Broad), 16);
        assert_eq!(R405.half_line_pulse(0), Pulse::Broad);
        assert_eq!(R405.half_line_pulse(405), Pulse::Broad);
        assert_eq!(R405.half_line_pulse(403), Pulse::None);
        assert_eq!(R405.half_line_pulse(16), Pulse::Line);
        assert_eq!(R625.half_line_pulse(620), Pulse::Equalising);
        assert_eq!(R625.half_line_pulse(618), Pulse::Line);
        assert_eq!(R525.half_line_pulse(0), Pulse::Equalising);
        assert_eq!(R525.half_line_pulse(6), Pulse::Broad);
    }

    #[test]
    fn layouts_and_names() {
        let a = System::get(SystemId::A);
        assert_eq!(
            (a.raster.lines, a.raster.line_hz, a.positive, a.am_sound),
            (405, 10_125.0, true, true)
        );
        assert_eq!((a.spacing_mhz, a.video_mhz), (-3.5, 3.0));
        assert!((a.vision_from_centre_mhz() - 1.25).abs() < 1e-9);
        let e = System::get(SystemId::E);
        assert_eq!(
            (e.raster.lines, e.raster.line_hz, e.spacing_mhz, e.video_mhz),
            (819, 20_475.0, 11.15, 10.0)
        );
        assert_eq!(e.positive_blanking(), 0.25);
        let lp = System::get(SystemId::Lp);
        assert!(lp.turned_round());
        assert!((lp.vision_from_centre_mhz() - 2.75).abs() < 1e-9);
        assert_eq!(
            System::get(SystemId::L).name(Colour::Secam),
            "SECAM L 625/50"
        );
        assert_eq!(System::get(SystemId::H).vestige_mhz, 1.25);
        assert_eq!(System::get(SystemId::Kp).vestige_mhz, 1.25);
        assert_eq!(
            System::get(SystemId::N).name(Colour::NtscN),
            "NTSC-N 625/50"
        );
        assert_eq!(
            System::get(SystemId::I).name(Colour::Ntsc443),
            "NTSC 4.43 I 625/50"
        );
        assert_eq!(
            System::get(SystemId::M).name(Colour::Secam),
            "SECAM M 525/59.94"
        );
        assert_eq!(System::get(SystemId::B).name(Colour::Pal), "PAL B/G 625/50");
        assert_eq!(System::get(SystemId::A).name(Colour::Mono), "A 405/50 mono");
        assert!((Colour::NtscN.params().unwrap().fsc - 3_582_056.25).abs() < 1e-6);
        assert!(!Colour::NtscN.params().unwrap().v_switch);
        assert!((Colour::Pal.params().unwrap().fsc - 4_433_618.75).abs() < 1e-6);
        assert!(!System::get(SystemId::A).allows(Colour::Pal));
        assert!(!System::get(SystemId::L).allows(Colour::Ntsc));
        assert!(System::get(SystemId::M).allows(Colour::PalM));
    }

    #[test]
    fn identification_follows_section_6_1() {
        assert_eq!(identify(625, false, 5.5, false), SystemId::G);
        assert_eq!(identify(625, false, 6.0, false), SystemId::I);
        assert_eq!(identify(625, false, 6.5, false), SystemId::DK);
        assert_eq!(identify(625, false, 4.5, false), SystemId::N);
        assert_eq!(identify(625, true, 6.5, true), SystemId::Lp);
        assert_eq!(identify(625, true, 5.5, false), SystemId::C);
        assert_eq!(identify(625, true, 6.5, false), SystemId::L);
        assert_eq!(identify(525, false, 4.5, false), SystemId::M);
    }
}

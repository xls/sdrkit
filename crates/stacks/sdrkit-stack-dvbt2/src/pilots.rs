//! Where the pilots, the reserved tones and the data cells sit in each symbol of a T2-frame, and the value of every pilot.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 8.3.2, 9.2, 9.3 and 9.4, annexes G and H.

use crate::error::Rule;
use crate::params::{frame_closing, Fft, Guard, PilotPattern};
use crate::tables;
use sdrkit_util_bits::seq::DVBT_PILOTS;

/// The physical shape of a T2-frame: everything that fixes the carriers of each symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameShape {
    /// FFT size.
    pub fft: Fft,
    /// Extended carrier mode (8K, 16K and 32K only).
    pub extended: bool,
    /// Scattered pilot pattern.
    pub pp: PilotPattern,
    /// Guard interval.
    pub guard: Guard,
    /// MISO (P2 in its MISO format and the MISO pilot rules).
    pub miso: bool,
    /// Tone reservation in the data symbols (PAPR codes 0010 and 0011). The P2 symbols always reserve their tones.
    pub tr: bool,
    /// L_data, the data symbols after the P2 symbols (NUM_DATA_SYMBOLS).
    pub l_data: usize,
}

impl FrameShape {
    /// N_P2.
    pub fn n_p2(&self) -> usize {
        self.fft.n_p2()
    }

    /// L_F, the symbols after P1.
    pub fn l_f(&self) -> usize {
        self.n_p2() + self.l_data
    }

    /// Whether the last symbol is a frame closing symbol.
    pub fn has_fc(&self) -> bool {
        frame_closing(self.guard, self.pp, self.miso)
    }

    /// K_ext when the extended carrier mode is on, else 0.
    pub fn kx(&self) -> usize {
        if self.extended {
            self.fft.k_ext()
        } else {
            0
        }
    }

    /// K_total.
    pub fn k_total(&self) -> usize {
        self.fft.k_total(self.extended)
    }

    /// The samples of one symbol after P1 (FFT size and guard), or `None` when the guard is not allowed for the size.
    pub fn symbol_len(&self) -> Option<usize> {
        Some(self.fft.size() + self.guard.samples(self.fft)?)
    }

    /// The kind of symbol `l` (0 is the first P2 symbol).
    pub fn kind(&self, l: usize) -> SymbolKind {
        if l < self.n_p2() {
            SymbolKind::P2
        } else if self.has_fc() && l + 1 == self.l_f() {
            SymbolKind::FrameClosing
        } else {
            SymbolKind::Normal
        }
    }

    /// Returns the rule that the shape breaks, if any: the extended mode exists for the size, the guard is allowed for the size, and the frame is at least
    /// N_P2 + 3 symbols (32K, an even number) or N_P2 + 7 symbols (clause 8.3.1).
    pub fn check(&self) -> Result<(), Rule> {
        if self.extended && !self.fft.has_extended() {
            return Err(Rule::ExtendedCarriers);
        }
        if self.guard.samples(self.fft).is_none() {
            return Err(Rule::GuardForFft);
        }
        let min = if self.fft == Fft::K32 { 3 } else { 7 };
        if self.l_data < min || self.l_f() > 4095 {
            return Err(Rule::SymbolCount);
        }
        if self.fft == Fft::K32 && !self.l_f().is_multiple_of(2) {
            return Err(Rule::OddSymbols);
        }
        Ok(())
    }
}

/// The three kinds of symbol after P1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    /// A P2 symbol (L1 signalling).
    P2,
    /// A normal data symbol.
    Normal,
    /// The frame closing symbol.
    FrameClosing,
}

/// The kind of a pilot, which sets its MISO rule (clause 9.2.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilotKind {
    /// Scattered pilot.
    Scattered,
    /// Continual pilot on a carrier that never carries a scattered pilot.
    Continual,
    /// Continual pilot on a scattered-pilot-bearing carrier.
    ContinualOnGrid,
    /// Edge pilot.
    Edge,
    /// P2 pilot that MISO group 2 inverts on odd multiples of three.
    P2,
    /// P2 pilot that both MISO groups send the same (the MISO additions).
    P2Same,
    /// Frame closing pilot.
    FrameClosing,
}

/// One pilot cell: carrier, amplitude and kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotCell {
    /// Carrier index k (0 is K_min).
    pub k: u32,
    /// Amplitude (A_SP, A_CP or A_P2).
    pub amp: f32,
    /// Kind.
    pub kind: PilotKind,
}

/// The carriers of one symbol: its data cells in carrier order and its pilots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolMap {
    /// The carriers of the data cells, in increasing order (clause 9.4).
    pub data: Vec<u32>,
    /// The pilots, in increasing carrier order.
    pub pilots: Vec<PilotCell>,
    /// The reserved tones.
    pub reserved: Vec<u32>,
}

/// The continual pilot carriers of an FFT size and pattern in a carrier mode, sorted (clauses 9.2.4.1 and 9.2.4.2, annex G).
/// In extended carrier mode the normal-mode carriers move up by K_ext and the carriers of table G.2 are added.
pub fn continual_pilots(fft: Fft, pp: PilotPattern, extended: bool) -> Vec<u32> {
    let (groups, kmod) = fft.cp_groups();
    let kmax = fft.k_normal() - 1;
    let kx = if extended { fft.k_ext() } else { 0 };
    let mut v: Vec<u32> = Vec::new();
    for g in tables::CP_GROUPS.iter().take(groups) {
        for &k32 in g[pp.code() as usize] {
            let k = match kmod {
                Some(m) => k32 as usize % m,
                None => k32 as usize,
            };
            if k <= kmax {
                v.push((k + kx) as u32);
            }
        }
    }
    if extended && fft.has_extended() {
        let row = fft.row() - Fft::K8.row();
        v.extend(
            tables::CP_EXTENDED[row][pp.code() as usize]
                .iter()
                .map(|&k| k as u32),
        );
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// The symbol-level PRBS w_i of clause 9.2.2.1 (x^11 + x^2 + 1 from all ones), `n` values.
pub fn pilot_prbs(n: usize) -> Vec<u8> {
    let mut r = DVBT_PILOTS;
    let mut w = vec![0u8; n];
    r.fill_bits(&mut w);
    w
}

/// The frame-level PN bit pn_l of clause 9.2.2.2 (table 56), for symbol `l` (0 to 2 623).
pub fn frame_pn(l: usize) -> u8 {
    let byte = tables::FRAME_PN.get(l / 8).copied().unwrap_or(0);
    (byte >> (7 - l % 8)) & 1
}

/// The pilot grid of one frame shape: a map for the P2 symbols, one for each scattered pilot phase of the normal symbols (and
/// each tone reservation shift), one for the frame closing symbol, and the pilot sequence.
#[derive(Clone, Debug)]
pub struct PilotMaps {
    shape: FrameShape,
    p2: SymbolMap,
    normal: Vec<SymbolMap>,
    fc: Option<SymbolMap>,
    w: Vec<u8>,
    fc_active: usize,
}

impl PilotMaps {
    /// The maps of a frame shape. Errors when [`FrameShape::check`] refuses the shape.
    pub fn new(shape: FrameShape) -> Result<PilotMaps, Rule> {
        shape.check()?;
        let fft = shape.fft;
        let kt = shape.k_total();
        let kx = shape.kx();
        let kmax = kt - 1;
        let fi = fft.row();
        // P2 (clause 9.2.6, 9.3).
        let tr_p2: Vec<u32> = tables::TR_P2[fi]
            .iter()
            .map(|&k| (k as usize + kx) as u32)
            .collect();
        let a_p2 = if fft == Fft::K32 && !shape.miso {
            37f32.sqrt() / 5.0
        } else {
            31f32.sqrt() / 5.0
        };
        let mut class = vec![Class::Data; kt];
        for &k in &tr_p2 {
            class[k as usize] = Class::Reserved;
        }
        let step = if fft == Fft::K32 && !shape.miso { 6 } else { 3 };
        for (k, c) in class.iter_mut().enumerate() {
            let band = shape.extended && (k < kx || k + kx > kmax);
            if k % step == 0 || band {
                *c = Class::Pilot(a_p2, PilotKind::P2);
            }
        }
        if shape.miso {
            for k in [kx + 1, kx + 2, kmax - kx - 2, kmax - kx - 1] {
                class[k] = Class::Pilot(a_p2, PilotKind::P2Same);
            }
            for &ki in &tr_p2 {
                let ki = ki as usize;
                let adj = match ki % 3 {
                    1 => Some(ki + 1),
                    2 => Some(ki - 1),
                    _ => None,
                };
                if let Some(a) = adj.filter(|&a| a <= kmax && !tr_p2.contains(&(a as u32))) {
                    if !matches!(class[a], Class::Pilot(..)) {
                        class[a] = Class::Pilot(a_p2, PilotKind::P2Same);
                    }
                }
            }
        }
        let p2 = SymbolMap::from_classes(&class);
        // Normal symbols (clauses 9.2.3 to 9.2.5, 9.3): one map per phase l mod D_Y.
        let (dx, dy) = (shape.pp.dx(), shape.pp.dy());
        let a_sp = shape.pp.a_sp();
        let cps = continual_pilots(fft, shape.pp, shape.extended);
        let mut normal = Vec::with_capacity(dy);
        for phase in 0..dy {
            class.fill(Class::Data);
            for &k in &cps {
                let on_grid = (k as i64 - kx as i64).rem_euclid(dx as i64) == 0;
                let kind = if on_grid {
                    PilotKind::ContinualOnGrid
                } else {
                    PilotKind::Continual
                };
                class[k as usize] = Class::Pilot(fft.a_cp(), kind);
            }
            for (k, c) in class.iter_mut().enumerate() {
                if (k as i64 - kx as i64).rem_euclid((dx * dy) as i64) == (dx * phase) as i64 {
                    *c = Class::Pilot(a_sp, PilotKind::Scattered);
                }
            }
            class[0] = Class::Pilot(a_sp, PilotKind::Edge);
            class[kmax] = Class::Pilot(a_sp, PilotKind::Edge);
            if shape.tr {
                for &i in tables::TR_DATA[fi] {
                    let k = i as usize + dx * phase + kx;
                    if k < kt {
                        class[k] = Class::Reserved;
                    }
                }
            }
            normal.push(SymbolMap::from_classes(&class));
        }
        // Frame closing symbol (clause 9.2.7, 8.3.9).
        let mut fc_active = 0;
        let fc = shape.has_fc().then(|| {
            class.fill(Class::Data);
            for (k, c) in class.iter_mut().enumerate() {
                if k % dx == 0 {
                    *c = Class::Pilot(a_sp, PilotKind::FrameClosing);
                }
            }
            let extra = matches!(
                (fft, shape.pp),
                (Fft::K1, PilotPattern::Pp4 | PilotPattern::Pp5) | (Fft::K2, PilotPattern::Pp7)
            );
            if extra {
                class[kmax - 1] = Class::Pilot(a_sp, PilotKind::FrameClosing);
            }
            class[0] = Class::Pilot(a_sp, PilotKind::Edge);
            class[kmax] = Class::Pilot(a_sp, PilotKind::Edge);
            if shape.tr {
                for &k in &tr_p2 {
                    class[k as usize] = Class::Reserved;
                }
            }
            let m = SymbolMap::from_classes(&class);
            fc_active = fc_active_cells(shape, m.data.len());
            m
        });
        let w = pilot_prbs(fft.k_total(true) + 1);
        Ok(PilotMaps {
            shape,
            p2,
            normal,
            fc,
            w,
            fc_active,
        })
    }

    /// The frame shape.
    pub fn shape(&self) -> &FrameShape {
        &self.shape
    }

    /// The map of symbol `l` (0 is the first P2 symbol).
    pub fn symbol(&self, l: usize) -> &SymbolMap {
        match self.shape.kind(l) {
            SymbolKind::P2 => &self.p2,
            SymbolKind::FrameClosing => self.fc.as_ref().unwrap_or(&self.p2),
            SymbolKind::Normal => &self.normal[l % self.normal.len()],
        }
    }

    /// The map of the P2 symbols.
    pub fn p2(&self) -> &SymbolMap {
        &self.p2
    }

    /// The map of normal symbols with scattered pilot phase `phase` (l mod D_Y).
    pub fn normal(&self, phase: usize) -> &SymbolMap {
        &self.normal[phase % self.normal.len()]
    }

    /// The data cells of symbol `l` that carry modulation: C_P2, C_data, or C_FC for the frame closing symbol.
    pub fn active_cells(&self, l: usize) -> usize {
        match self.shape.kind(l) {
            SymbolKind::FrameClosing => self.fc_active,
            _ => self.symbol(l).data.len(),
        }
    }

    /// The pilot sequence bit r_{l,k} (clause 9.2.2): w at k + K_ext in normal carrier mode (at k in extended mode) XOR pn_l.
    pub fn r(&self, l: usize, k: u32) -> u8 {
        let off = if self.shape.extended {
            0
        } else {
            self.shape.fft.k_ext()
        };
        self.w[k as usize + off] ^ frame_pn(l)
    }

    /// The value of a pilot in symbol `l` as sent by MISO group 1 (or SISO), or by MISO group 2 when `group2` (clauses 9.2.3.3,
    /// 9.2.4.4, 9.2.5, 9.2.6.3, 9.2.7.3 and 9.2.8).
    pub fn pilot_value(&self, l: usize, p: &PilotCell, group2: bool) -> f32 {
        let base = p.amp * (1.0 - 2.0 * self.r(l, p.k) as f32);
        if !group2 {
            return base;
        }
        let dx = self.shape.pp.dx() as i64;
        let kx = self.shape.kx() as i64;
        let rel = p.k as i64 - kx;
        let invert = match p.kind {
            PilotKind::Scattered | PilotKind::FrameClosing | PilotKind::ContinualOnGrid => {
                rel.rem_euclid(dx) == 0 && rel.div_euclid(dx).rem_euclid(2) == 1
            }
            PilotKind::Continual | PilotKind::P2Same => false,
            PilotKind::Edge => l % 2 == 1,
            PilotKind::P2 => p.k.is_multiple_of(3) && (p.k / 3) % 2 == 1,
        };
        if invert {
            -base
        } else {
            base
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    Data,
    Pilot(f32, PilotKind),
    Reserved,
}

impl SymbolMap {
    fn from_classes(class: &[Class]) -> SymbolMap {
        let mut m = SymbolMap::default();
        for (k, c) in class.iter().enumerate() {
            match *c {
                Class::Data => m.data.push(k as u32),
                Class::Pilot(amp, kind) => m.pilots.push(PilotCell {
                    k: k as u32,
                    amp,
                    kind,
                }),
                Class::Reserved => m.reserved.push(k as u32),
            }
        }
        m
    }
}

/// C_FC for a shape: table 50 lists it for each size and pattern. With tone reservation its N_TR value is subtracted (clause
/// 8.3.2). `n_fc` is the number of data cells of the frame closing symbol. A shape the table does not list keeps every cell.
fn fc_active_cells(shape: FrameShape, n_fc: usize) -> usize {
    let p = shape.pp.code() as usize;
    let f = shape.fft.row();
    let c = if shape.extended {
        tables::C_FC_EXTENDED[f - Fft::K8.row()][p]
    } else {
        tables::C_FC[f][p]
    };
    if c == 0 {
        return n_fc;
    }
    let tr = if shape.tr { tables::N_TR[f] } else { 0 };
    (c - tr).min(n_fc)
}

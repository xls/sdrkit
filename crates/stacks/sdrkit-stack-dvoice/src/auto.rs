//! Digital voice (auto): one receiver that looks for the frame syncs of every 4-level FSK stack at once on a 4800 and a 2400
//! symbol/s lane, and passes each transmission to the stack whose sync it carries. The detected stack is published.

use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::{AutoFields, SeenRow, SEEN};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::{Declare, Published, TableDef};

sdrkit_core::choice_enum! {
    /// A stack that the detector finds.
    pub enum Detected {
        /// DMR.
        Dmr = ("dmr", "DMR"),
        /// NXDN.
        Nxdn = ("nxdn", "NXDN"),
        /// dPMR.
        Dpmr = ("dpmr", "dPMR"),
        /// P25 Phase 1.
        P25 = ("p25", "P25"),
        /// System Fusion.
        Ysf = ("ysf", "Yaesu System Fusion"),
        /// M17.
        M17 = ("m17", "M17"),
    }
}

struct Sub {
    stack: Detected,
    proto: Box<dyn Protocol>,
    /// Lane it runs on: 0 for 4800 symbols/s, 1 for 2400 (NXDN runs on both).
    lanes: [bool; 2],
    blocks_seen: u64,
    last_ok_s: f64,
    taken_s: f64,
    taken_len: u32,
}

/// The automatic detector.
pub struct Auto {
    subs: Vec<Sub>,
    active: [Option<usize>; 2],
    detected: Option<usize>,
}

impl Default for Auto {
    fn default() -> Self {
        let mk = |stack, p: Box<dyn Protocol>, lanes: [bool; 2]| Sub {
            stack,
            proto: p,
            lanes,
            blocks_seen: 0,
            last_ok_s: -10.0,
            taken_s: -10.0,
            taken_len: 0,
        };
        Auto {
            subs: vec![
                mk(
                    Detected::Dmr,
                    Box::<crate::dmr::Dmr>::default(),
                    [true, false],
                ),
                mk(
                    Detected::Nxdn,
                    Box::<crate::nxdn::Nxdn>::default(),
                    [true, true],
                ),
                mk(
                    Detected::Dpmr,
                    Box::<crate::dpmr::Dpmr>::default(),
                    [false, true],
                ),
                mk(
                    Detected::P25,
                    Box::<crate::p25::P25>::default(),
                    [true, false],
                ),
                mk(
                    Detected::Ysf,
                    Box::<crate::ysf::Ysf>::default(),
                    [true, false],
                ),
                mk(
                    Detected::M17,
                    Box::<crate::m17::M17>::default(),
                    [true, false],
                ),
            ],
            active: [None, None],
            detected: None,
        }
    }
}

impl Auto {
    fn track(&mut self, i: usize, before: u64, ctx: &Ctx) {
        if ctx.blocks_ok > before {
            let s = &mut self.subs[i];
            s.blocks_seen += ctx.blocks_ok - before;
            s.last_ok_s = ctx.now;
            self.detected = Some(i);
        }
    }
}

impl Protocol for Auto {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "dvoice-auto",
            name: "Digital voice (auto)",
            baud: 4800.0,
            two_level: false,
            channel_hz: 12_500.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: true,
            extra_baud: Some(2400.0),
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        let mut v = Vec::new();
        for (i, s) in self.subs.iter().enumerate() {
            // Polarity is taken as sent: one stack's inverted sync is often another stack's sync.
            for mut p in s.proto.patterns() {
                p.id += (i as u8) * 16;
                v.push(p.no_auto_invert());
            }
        }
        v
    }

    fn on_sync(
        &mut self,
        id: u8,
        inverted: bool,
        pre: &[Sym],
        at: u64,
        tracking: bool,
        ctx: &mut Ctx,
    ) -> SyncAnswer {
        let i = (id / 16) as usize;
        let lane = ctx.lane as usize;
        if i >= self.subs.len() || !self.subs[i].lanes[lane] {
            return SyncAnswer::Ignore;
        }
        // While one stack decodes on this lane, a sync of another stack is taken only when the first has gone quiet. A stack that
        // holds the lane on a sync and has decoded nothing since releases the lane at once (its sync can be a false one). Just
        // after a take (a frame is being read), only a sync at least as long can take the lane over.
        let len = pre.len() as u32;
        if let Some(a) = self.active[lane] {
            let sa = &self.subs[a];
            let proven = sa.last_ok_s >= sa.taken_s;
            if a != i && proven && ctx.now - sa.last_ok_s < 0.5 {
                return SyncAnswer::Ignore;
            }
            if a != i && !proven && ctx.now - sa.taken_s < 0.25 && len < sa.taken_len {
                return SyncAnswer::Ignore;
            }
        }
        let before = ctx.blocks_ok;
        let ans = self.subs[i].proto.on_sync(
            id % 16,
            inverted,
            pre,
            at,
            tracking && self.active[lane] == Some(i),
            ctx,
        );
        if ans == SyncAnswer::Take {
            if self.active[lane] != Some(i) {
                self.subs[i].taken_s = ctx.now;
                self.subs[i].taken_len = len;
            }
            self.active[lane] = Some(i);
        }
        self.track(i, before, ctx);
        ans
    }

    fn on_symbol(&mut self, s: Sym, at: u64, ctx: &mut Ctx) -> SymAnswer {
        let lane = ctx.lane as usize;
        let Some(i) = self.active[lane] else {
            return SymAnswer::Stop;
        };
        let before = ctx.blocks_ok;
        let a = self.subs[i].proto.on_symbol(s, at, ctx);
        self.track(i, before, ctx);
        if a == SymAnswer::Stop {
            self.active[lane] = None;
        }
        a
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        for s in self.subs.iter_mut() {
            s.proto.on_tick(ctx);
        }
    }

    fn on_reset(&mut self, ctx: &mut Ctx) {
        for s in self.subs.iter_mut() {
            s.proto.on_reset(ctx);
        }
        self.active = [None, None];
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        // The detected stack's own fields are published as well.
        let mut v = <AutoFields as Fields>::SHAPE.fields.to_vec();
        for s in &self.subs {
            for f in s.proto.fields() {
                if !v.iter().any(|x| x.key() == f.key()) {
                    v.push(f);
                }
            }
        }
        v
    }

    fn tables(&self) -> Vec<&'static TableDef> {
        let mut v = vec![SEEN.def()];
        for s in &self.subs {
            for t in s.proto.tables() {
                if !v.iter().any(|x| x.key == t.key) {
                    v.push(t);
                }
            }
        }
        v
    }

    fn publish(&self, r: &mut Published) {
        r.put(&AutoFields {
            detected: self.detected.map(|i| self.subs[i].stack),
        });
        for s in self.subs.iter().filter(|s| s.blocks_seen > 0) {
            let row = SeenRow {
                stack: s.stack,
                blocks: s.blocks_seen as i64,
                last_s: s.last_ok_s,
            };
            SEEN.put(r, s.proto.params().mode, &row);
        }
        if let Some(i) = self.detected {
            self.subs[i].proto.publish(r);
        }
    }

    fn codec(&self) -> Codec {
        self.detected
            .map_or(Codec::Ambe2, |i| self.subs[i].proto.codec())
    }
}

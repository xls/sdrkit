//! The ACARS block stages: the checker (block check, repair, parity) and the multi-block assembler.
//! Standard: ARINC 618 clauses 2.3.5 (block check) and 2.3.6 (multi-block messages).

use super::JOIN_BLOCKS;
use super::{Block, FrontState, Message, RawBlock};
use sdrkit_core::{ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_aero::acars::assemble::{Assembler, Joined, Quality};
use sdrkit_proto_aero::acars::{self, check_and_repair, strip_parity, Check};

/// Checks and repairs blocks and clears their parity bits.
#[derive(Default)]
pub struct Checker;

impl Component for Checker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.acars_check",
            vec![Port::typed::<RawBlock>("in")],
            vec![Port::typed::<Block>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Block>(), 0.0)])
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.items::<RawBlock>(0);
        if input.is_empty() {
            return;
        }
        let out = io.items_out::<Block>(0);
        for t in input {
            out.push(Timed {
                time: t.time,
                value: check(&t.value),
            });
        }
    }
}

/// Checks one raw block: the BCS test with repair (ARINC 618 clause 2.3.5), then the parity bits cleared. A block is good
/// when the BCS holds after the repair and the fields parse.
pub fn check(r: &RawBlock) -> Block {
    let mut body = r.bytes.clone();
    let (good, bits_fixed) = match check_and_repair(&mut body, r.bcs) {
        Check::Good { bits_fixed } => (true, bits_fixed),
        Check::Bad => (false, 0),
    };
    strip_parity(&mut body);
    let good = good && acars::Block::parse(&body).is_ok();
    Block {
        freq_hz: r.freq_hz,
        level_db: r.level_db,
        end_s: r.end_s,
        good,
        bits_fixed,
        body,
    }
}

/// The multi-block assembler. Bad blocks are dropped here (the tables count them from the checker).
pub struct JoinStage {
    joiner: Assembler<(f64, f32)>,
    enabled: bool,
    out: Vec<Joined<(f64, f32)>>,
}

impl Default for JoinStage {
    fn default() -> Self {
        JoinStage {
            joiner: Assembler::new(),
            enabled: JOIN_BLOCKS.default,
            out: Vec::new(),
        }
    }
}

sdrkit_core::settings! {
    /// The settings of [`JoinStage`].
    pub enum JoinSetting {
        /// Join the blocks of a long message.
        JoinBlocks = JOIN_BLOCKS,
    }
}

impl Component for JoinStage {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.acars_join",
            vec![
                Port::typed::<Block>("in"),
                Port::typed::<FrontState>("state"),
            ],
            vec![Port::typed::<Message>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(JoinSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.joiner.clear();
        Ok(vec![Format::new(Kind::typed::<Message>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for t in io.items::<Block>(0) {
            let b = &t.value;
            if !b.good {
                continue;
            }
            let Ok(block) = acars::Block::parse(&b.body) else {
                continue;
            };
            let q = Quality {
                bits_fixed: b.bits_fixed,
                bcs_ok: true,
            };
            self.joiner.push(
                b.end_s,
                &block,
                q,
                0,
                (b.freq_hz, b.level_db),
                &mut self.out,
            );
        }
        for st in io.items::<FrontState>(1) {
            self.joiner.expire(st.value.time_s, &mut self.out);
        }
        if !self.out.is_empty() {
            let o = io.items_out::<Message>(0);
            for j in self.out.drain(..) {
                let (freq_hz, level_db) = j.body.extra;
                o.push(Timed {
                    time: j.last_t,
                    value: Message {
                        freq_hz,
                        level_db,
                        end_s: j.last_t,
                        msg: acars::Message::from_joined(&j),
                    },
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match JoinSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            JoinSetting::JoinBlocks => {
                self.enabled = JOIN_BLOCKS.read(id, v)?;
                self.joiner.set_enabled(self.enabled, &mut self.out);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(JoinSetting::JoinBlocks, Value::Bool(self.enabled))])
    }
}

//! The LoRa channels of a stack: a planner turns the stack's settings and the input (rate, centre, tuned frequency) into the
//! plan of channels and detectors, and runs the shared `lora.bank` with it. The plan also leaves as an item, so the decoders and
//! the texting clients know which setting each detector serves. Three planners: `mesh.channels`, `lorawan.channels` and
//! `loraraw.channels`.

use crate::mesh::plan::{self, MeshConfig, Plan};
use crate::proto::{lorawan, meshtastic};
use crate::settings::{
    CodingRate, LdroChoice, LoraBandwidth, MeshPlanSetting, RawPlanSetting, RawSync,
    WidePlanSetting, AUTO_BW, AUTO_SF, BW, CHANNELS, CR, CRC, CUSTOM, CUSTOM_BW, CUSTOM_CR,
    CUSTOM_FREQ, CUSTOM_PREAMBLE, CUSTOM_SF, CUSTOM_SYNC, DEFAULTS_ONLY, DIAL_FREQ, DOWNLINKS, FSK,
    FSK_FREQ, IMPLICIT, JUMP_CHIPS, JUMP_SEARCH, LDRO, LENGTH, LW_CHANNELS, MAX_CHANNELS,
    MT_CHANNEL_NAME, MT_CHANNEL_NUM, MT_FREQ_OFFSET, MT_OVERRIDE_FREQ, MT_PRESET, MT_SLOT_END,
    MT_SLOT_START, OFFSET, PREAMBLE, PRESET_SEARCH, PRIVATE_SYNC, PROTOCOLS, SF, SYNC, SYNC_CUSTOM,
    WIDE_CHANNELS, WIDE_DIAL_FREQ,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::{
    Choice, ConfigError, HostRequest, Record, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, TagKind,
    Timed,
};
use sdrkit_proto_lora_phy::bank::{BankChannel, BankPlan};
use sdrkit_proto_lora_phy::coding::Header;
use sdrkit_proto_lora_phy::components::{BankState, LoraBank, LoraBankCommand};
use sdrkit_proto_lora_phy::params::{fmt_khz, Ldro, LoraParams};
use sdrkit_proto_lora_phy::rx::{DetectorConfig, LoraFrame, LoraSymbols};
use serde::{Deserialize, Serialize};

/// The input a plan is made for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The frequency at the centre of the samples, Hz (0 when not known).
    pub centre_hz: f64,
    /// Where the user tuned, Hz; `None` when not known.
    pub tuned_hz: Option<f64>,
}

impl Input {
    /// The frequency to listen at: where the user tuned, else the centre of the capture.
    pub fn dial(&self) -> f64 {
        self.tuned_hz.unwrap_or(self.centre_hz)
    }
}

/// The tuned frequency of a stream format or a retune tag, where 0 stands for not known.
pub fn known_tuning(tuned_hz: f64) -> Option<f64> {
    (tuned_hz > 0.0).then_some(tuned_hz)
}

/// What a stack searches: its settings and the plan they give for an input.
pub trait Planner: Send + 'static {
    /// The plan as a graph item.
    type Item: Item + Clone + PartialEq;
    /// The planner's settings.
    type Setting: SettingSet;
    /// The component's registered name.
    const NAME: &'static str;
    /// Applies setting `id`. Returns true if the dial moved, so that the caller can request the host to tune there.
    ///
    /// # Errors
    ///
    /// [`SettingError::NoSuchId`] for an id outside [`Planner::Setting`], and the refusals of the planner.
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError>;
    /// The values the planner runs with.
    fn values(&self) -> Values;
    /// The bank plan and the plan item for `input`.
    fn plan(&self, input: &Input) -> (BankPlan, Self::Item);
    /// The dial of the plan, when the plan has its own.
    fn dial(&self) -> Option<f64> {
        None
    }
}

/// A planner and the shared bank it runs (component documentation in the module).
pub struct LoraChannels<P: Planner> {
    /// The planner.
    pub planner: P,
    bank: LoraBank,
    input: Input,
    dirty: bool,
    item: Option<P::Item>,
    pending: Option<P::Item>,
    retune: bool,
}

impl<P: Planner> LoraChannels<P> {
    /// The channels of `planner`.
    pub fn new(planner: P) -> Self {
        LoraChannels {
            planner,
            bank: LoraBank::new(BankPlan::default()),
            input: Input::default(),
            dirty: true,
            item: None,
            pending: None,
            retune: false,
        }
    }

    /// Why the bank refused the last plan, if it did.
    pub fn error(&self) -> Option<sdrkit_proto_lora_phy::LoraError> {
        self.bank.error()
    }

    /// Makes the plan for the current input and returns it to the bank. A plan item that changed waits in `pending`.
    fn replan(&mut self) {
        self.dirty = false;
        if self.input.rate <= 0.0 {
            return;
        }
        let (bank, item) = self.planner.plan(&self.input);
        self.bank.set_plan(bank);
        if self.item.as_ref() != Some(&item) {
            self.item = Some(item.clone());
            self.pending = Some(item);
        }
    }
}

impl<P: Planner> Component for LoraChannels<P> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            P::NAME,
            vec![Port::iq("in")],
            vec![
                Port::typed::<LoraFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<LoraSymbols>("symbols").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<BankState>("state").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<P::Item>("plan").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(P::Setting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(f) = inputs.first() {
            self.input = Input {
                rate: f.rate,
                centre_hz: f.centre_hz,
                tuned_hz: known_tuning(f.tuned_hz),
            };
        }
        self.item = None;
        self.replan();
        let mut out = self.bank.configure(inputs)?;
        out.push(Format::new(Kind::typed::<P::Item>(), 0.0));
        out.push(Format::new(Kind::typed::<HostRequest>(), 0.0));
        Ok(out)
    }

    fn process(&mut self, io: &mut Io) {
        let retuned = io.input(0).tags().iter().rev().find_map(|t| match t.kind {
            TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } => Some((centre_hz, tuned_hz)),
            _ => None,
        });
        if let Some((c, t)) = retuned {
            self.input.centre_hz = c;
            self.input.tuned_hz = known_tuning(t);
            self.dirty = true;
        }
        let time = io.input(0).first() as f64 / self.input.rate.max(1.0);
        if self.dirty {
            self.replan();
        }
        if let Some(item) = self.pending.take() {
            io.items_out::<P::Item>(3).push(Timed { time, value: item });
        }
        if std::mem::take(&mut self.retune) {
            if let Some(dial) = self.planner.dial() {
                if self.input.tuned_hz.is_some_and(|t| (dial - t).abs() > 1.0) {
                    io.items_out::<HostRequest>(4).push(Timed {
                        time,
                        value: HostRequest::Retune {
                            tuned_hz: dial,
                            reason: "the channels have their own dial".into(),
                        },
                    });
                }
            }
        }
        self.bank.process(io);
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        // Only a change the user makes after the start requests the host to tune: the session starts where the host tuned.
        if self.planner.apply(id, value)? && self.input.rate > 0.0 {
            self.retune = true;
        }
        self.dirty = true;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        self.planner.values()
    }

    fn commands(&self) -> &Commands {
        LoraBankCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        self.bank.invoke(id, args)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The mesh receiver.

impl Item for Plan {
    const KIND: &'static str = "mesh.plan";
}

/// The planner of the mesh receiver.
pub struct MeshPlanner {
    /// The plan's settings.
    pub cfg: MeshConfig,
    /// The choice of [`MT_PRESET`].
    preset_choice: u16,
    custom_bw: LoraBandwidth,
    custom_cr: CodingRate,
    /// The texts of [`CHANNELS`] and [`LW_CHANNELS`].
    channels_text: String,
    lw_channels_text: String,
}

impl MeshPlanner {
    /// The planner of a stack (`mesh`, `meshtastic`, ...).
    pub fn new(stack: &str) -> Self {
        MeshPlanner {
            cfg: MeshConfig::for_submode(stack),
            preset_choice: MT_PRESET.default_index(),
            custom_bw: LoraBandwidth::Bw250,
            custom_cr: CodingRate::Cr5,
            channels_text: String::new(),
            lw_channels_text: String::new(),
        }
    }
}

/// The refusal of catalogue entry `index`, which the catalogue does not hold.
fn not_a_choice(id: SettingId, index: usize) -> SettingError {
    SettingError::NotAChoice {
        id,
        index: u16::try_from(index).unwrap_or(u16::MAX),
    }
}

fn narrow<T: TryFrom<i64>>(id: SettingId, v: i64) -> Result<T, SettingError> {
    T::try_from(v).map_err(|_| SettingError::NotPermitted { id })
}

/// The LoRaWAN channels of the text `t` of setting `id`.
fn lw_list(id: SettingId, t: &str) -> Result<Vec<lorawan::PlanChannel>, SettingError> {
    lorawan::parse_channels(t).ok_or(SettingError::Malformed { id, at: None })
}

impl Planner for MeshPlanner {
    type Item = Plan;
    type Setting = MeshPlanSetting;
    const NAME: &'static str = "mesh.channels";

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        let before = self.cfg.own_dial();
        let c = &mut self.cfg;
        match MeshPlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MeshPlanSetting::DialFreq => c.dial_hz = DIAL_FREQ.read(id, v)?,
            MeshPlanSetting::Protocols => {
                // A stack of one protocol searches its own: only `mesh` reads the mask.
                let mask: u32 = narrow(id, PROTOCOLS.read(id, v)?)?;
                if c.submode == "mesh" {
                    c.mask = mask;
                }
            }
            MeshPlanSetting::PresetSearch => c.preset_search = PRESET_SEARCH.read(id, v)?,
            MeshPlanSetting::MtSlotStart => c.mt_band.start_hz = MT_SLOT_START.read(id, v)?,
            MeshPlanSetting::MtSlotEnd => c.mt_band.end_hz = MT_SLOT_END.read(id, v)?,
            MeshPlanSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                c.channels =
                    plan::parse_channels(t).ok_or(SettingError::Malformed { id, at: None })?;
                self.channels_text = t.to_string();
            }
            MeshPlanSetting::LwChannels => {
                let t = LW_CHANNELS.read(id, v)?;
                c.lw_channels = lw_list(id, t)?;
                self.lw_channels_text = t.to_string();
            }
            MeshPlanSetting::MtPreset => {
                let i = MT_PRESET.read(id, v)?;
                c.mt_preset = meshtastic::PRESETS
                    .get(i)
                    .ok_or_else(|| not_a_choice(id, i))?
                    .name
                    .into();
                self.preset_choice = u16::try_from(i).map_err(|_| not_a_choice(id, i))?;
            }
            MeshPlanSetting::MtChannelName => {
                c.mt_channel_name = MT_CHANNEL_NAME.read(id, v)?.trim().to_string();
            }
            MeshPlanSetting::MtChannelNum => {
                let num: u32 = narrow(id, MT_CHANNEL_NUM.read(id, v)?)?;
                // With a slot band, a number above its slots is refused.
                if let Err(e) = c.check_channel_num(num) {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 0.0,
                        max: f64::from(e.slots),
                        got: f64::from(num),
                    });
                }
                c.mt_channel_num = num;
            }
            MeshPlanSetting::MtOverrideFreq => c.mt_override_hz = MT_OVERRIDE_FREQ.read(id, v)?,
            MeshPlanSetting::MtFreqOffset => c.mt_offset_hz = MT_FREQ_OFFSET.read(id, v)?,
            MeshPlanSetting::Custom => c.custom = CUSTOM.read(id, v)?,
            MeshPlanSetting::CustomFreq => c.custom_setting.freq_hz = CUSTOM_FREQ.read(id, v)?,
            MeshPlanSetting::CustomBw => {
                self.custom_bw = CUSTOM_BW.read(id, v)?;
                c.custom_setting.bw = self.custom_bw.hz();
            }
            MeshPlanSetting::CustomSf => c.custom_setting.sf = narrow(id, CUSTOM_SF.read(id, v)?)?,
            MeshPlanSetting::CustomCr => {
                self.custom_cr = CUSTOM_CR.read(id, v)?;
                c.custom_setting.cr = self.custom_cr.denominator();
            }
            MeshPlanSetting::CustomSync => {
                c.custom_setting.sync = narrow(id, CUSTOM_SYNC.read(id, v)?)?;
            }
            MeshPlanSetting::CustomPreamble => {
                c.custom_setting.preamble = narrow(id, CUSTOM_PREAMBLE.read(id, v)?)?;
            }
            MeshPlanSetting::JumpSearch => c.jump_search = JUMP_SEARCH.read(id, v)?,
            MeshPlanSetting::JumpChips => c.jump_chips = JUMP_CHIPS.read(id, v)?,
        }
        Ok(self.cfg.own_dial() != before)
    }

    fn values(&self) -> Values {
        let c = &self.cfg;
        let cs = &c.custom_setting;
        values_of([
            (MeshPlanSetting::DialFreq, Value::Hz(c.dial_hz)),
            (MeshPlanSetting::Protocols, Value::I64(i64::from(c.mask))),
            (MeshPlanSetting::PresetSearch, Value::Bool(c.preset_search)),
            (MeshPlanSetting::MtSlotStart, Value::Hz(c.mt_band.start_hz)),
            (MeshPlanSetting::MtSlotEnd, Value::Hz(c.mt_band.end_hz)),
            (
                MeshPlanSetting::Channels,
                Value::Text(self.channels_text.clone()),
            ),
            (
                MeshPlanSetting::LwChannels,
                Value::Text(self.lw_channels_text.clone()),
            ),
            (MeshPlanSetting::MtPreset, Value::Choice(self.preset_choice)),
            (
                MeshPlanSetting::MtChannelName,
                Value::Text(c.mt_channel_name.clone()),
            ),
            (
                MeshPlanSetting::MtChannelNum,
                Value::I64(i64::from(c.mt_channel_num)),
            ),
            (MeshPlanSetting::MtOverrideFreq, Value::Hz(c.mt_override_hz)),
            (MeshPlanSetting::MtFreqOffset, Value::Hz(c.mt_offset_hz)),
            (MeshPlanSetting::Custom, Value::Bool(c.custom)),
            (MeshPlanSetting::CustomFreq, Value::Hz(cs.freq_hz)),
            (
                MeshPlanSetting::CustomBw,
                Value::Choice(self.custom_bw.index()),
            ),
            (MeshPlanSetting::CustomSf, Value::I64(i64::from(cs.sf))),
            (
                MeshPlanSetting::CustomCr,
                Value::Choice(self.custom_cr.index()),
            ),
            (MeshPlanSetting::CustomSync, Value::I64(i64::from(cs.sync))),
            (
                MeshPlanSetting::CustomPreamble,
                Value::I64(i64::from(cs.preamble)),
            ),
            (MeshPlanSetting::JumpSearch, Value::Bool(c.jump_search)),
            (MeshPlanSetting::JumpChips, Value::F64(c.jump_chips)),
        ])
    }

    fn plan(&self, input: &Input) -> (BankPlan, Plan) {
        let dial = self.cfg.dial(input.tuned_hz);
        let centre = if input.centre_hz > 0.0 {
            input.centre_hz
        } else {
            dial + 300e3
        };
        let mut p = plan::build(&self.cfg, input.tuned_hz, centre, input.rate);
        p.name = format!("{} MHz", plan::fmt_mhz(dial));
        (p.bank.clone(), p)
    }

    fn dial(&self) -> Option<f64> {
        self.cfg.own_dial()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRa (raw).

/// The settings of `loraraw`.
#[derive(Clone, Debug, PartialEq)]
pub struct RawConfig {
    /// Search SF7 to SF12.
    pub auto_sf: bool,
    /// The spreading factor.
    pub sf: u8,
    /// Search 125, 250 and 500 kHz.
    pub auto_bw: bool,
    /// The bandwidth, Hz.
    pub bw: f64,
    /// Implicit header.
    pub implicit: bool,
    /// The coding rate denominator.
    pub cr: u8,
    /// CRC.
    pub crc: bool,
    /// The length of an implicit frame.
    pub length: u8,
    /// The sync words.
    pub syncs: Vec<u8>,
    /// The sync choice.
    pub sync_choice: RawSync,
    /// The custom sync word (used when the choice is "custom").
    pub sync_custom: u8,
    /// The preamble length.
    pub preamble: u16,
    /// LDRO.
    pub ldro: LdroChoice,
    /// Offset from the dial, Hz.
    pub offset: f64,
    /// The lost-sample search.
    pub jump_search: bool,
}

impl Default for RawConfig {
    fn default() -> Self {
        RawConfig {
            auto_sf: true,
            sf: 7,
            auto_bw: true,
            bw: 125e3,
            implicit: false,
            cr: 5,
            crc: true,
            length: 16,
            syncs: vec![0x12],
            sync_choice: RawSync::Private,
            sync_custom: 0x12,
            preamble: 8,
            ldro: LdroChoice::Auto,
            offset: 0.0,
            jump_search: true,
        }
    }
}

impl RawConfig {
    /// The setting a transmission uses (an automatic SF or bandwidth sends at SF7 or 125 kHz).
    pub fn tx_params(&self) -> LoraParams {
        let mut p = LoraParams::new(
            if self.auto_sf { 7 } else { self.sf },
            if self.auto_bw { 125e3 } else { self.bw },
            self.cr,
            self.preamble,
            self.syncs.first().copied().unwrap_or(0x12),
        );
        p.crc = self.crc;
        p.implicit = self.implicit;
        p.implicit_len = self.length;
        p.ldro = self.ldro.ldro();
        p
    }
}

/// The plan of `loraraw` for the dial `dial`: the bank and the line that says what is searched.
pub fn raw_plan(c: &RawConfig, dial: f64) -> (BankPlan, String) {
    let bws: Vec<f64> = if c.auto_bw {
        vec![125e3, 250e3, 500e3]
    } else {
        vec![c.bw]
    };
    let sfs: Vec<u8> = if c.auto_sf {
        (7..=12).collect()
    } else {
        vec![c.sf]
    };
    let mut chans = Vec::new();
    let mut tag = 0u32;
    for &bw in &bws {
        let mut dets = Vec::new();
        for &sf in &sfs {
            let mut d = DetectorConfig::new(sf, bw, c.syncs.first().copied().unwrap_or(0x12));
            d.syncs = c.syncs.clone();
            d.preamble = c.preamble;
            d.ldro = c.ldro.ldro();
            d.jump_search = c.jump_search;
            if c.implicit {
                d.implicit = Some(Header {
                    len: c.length,
                    cr: c.cr,
                    crc: c.crc,
                });
            }
            d.tag = tag;
            tag += 1;
            dets.push(d);
        }
        chans.push(BankChannel {
            freq_hz: dial + c.offset,
            bw,
            detectors: dets,
        });
    }
    let line = format!(
        "{}, {}, {} header",
        if c.auto_sf {
            "SF7 - SF12".to_string()
        } else {
            format!("SF{}", c.sf)
        },
        bws.iter()
            .map(|b| format!("{} kHz", fmt_khz(*b)))
            .collect::<Vec<_>>()
            .join(", "),
        if c.implicit { "implicit" } else { "explicit" }
    );
    (
        BankPlan {
            channels: chans,
            fft_from: 3,
        },
        line,
    )
}

/// The plan of `loraraw` as an item (`loraraw.plan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RawPlan {
    /// The bank.
    pub bank: BankPlan,
    /// What is searched, in words.
    pub line: String,
    /// The dial, Hz.
    pub dial_hz: f64,
    /// The setting a transmission uses.
    pub tx: LoraParams,
    /// The channel frequency a transmission uses, Hz.
    pub tx_hz: f64,
}

impl Item for RawPlan {
    const KIND: &'static str = "loraraw.plan";
}

impl LdroChoice {
    /// The setting of the physical layer.
    pub fn ldro(self) -> Ldro {
        match self {
            LdroChoice::Auto => Ldro::Auto,
            LdroChoice::Off => Ldro::Off,
            LdroChoice::On => Ldro::On,
        }
    }
}

/// The planner of `loraraw`.
pub struct RawPlanner {
    /// The settings.
    pub cfg: RawConfig,
    bw: LoraBandwidth,
}

impl Default for RawPlanner {
    fn default() -> Self {
        RawPlanner {
            cfg: RawConfig::default(),
            bw: LoraBandwidth::Bw125,
        }
    }
}

impl Planner for RawPlanner {
    type Item = RawPlan;
    type Setting = RawPlanSetting;
    const NAME: &'static str = "loraraw.channels";

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        let c = &mut self.cfg;
        match RawPlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RawPlanSetting::AutoSf => c.auto_sf = AUTO_SF.read(id, v)?,
            RawPlanSetting::Sf => c.sf = narrow(id, SF.read(id, v)?)?,
            RawPlanSetting::AutoBw => c.auto_bw = AUTO_BW.read(id, v)?,
            RawPlanSetting::Bw => {
                self.bw = BW.read(id, v)?;
                c.bw = self.bw.hz();
            }
            RawPlanSetting::Implicit => c.implicit = IMPLICIT.read(id, v)?,
            RawPlanSetting::Cr => c.cr = narrow(id, CR.read(id, v)?)?,
            RawPlanSetting::Crc => c.crc = CRC.read(id, v)?,
            RawPlanSetting::Length => c.length = narrow(id, LENGTH.read(id, v)?)?,
            RawPlanSetting::Sync => c.sync_choice = SYNC.read(id, v)?,
            RawPlanSetting::SyncCustom => c.sync_custom = narrow(id, SYNC_CUSTOM.read(id, v)?)?,
            RawPlanSetting::Preamble => c.preamble = narrow(id, PREAMBLE.read(id, v)?)?,
            RawPlanSetting::Ldro => c.ldro = LDRO.read(id, v)?,
            RawPlanSetting::Offset => c.offset = OFFSET.read(id, v)?,
            RawPlanSetting::JumpSearch => c.jump_search = JUMP_SEARCH.read(id, v)?,
        }
        // The custom word counts only when "custom" is chosen, whatever order the two settings come in.
        c.syncs = match c.sync_choice {
            RawSync::Lorawan => vec![0x34],
            RawSync::Meshtastic => vec![0x2B],
            RawSync::Any => vec![0x12, 0x34, 0x2B],
            RawSync::Custom => vec![c.sync_custom],
            RawSync::Private => vec![0x12],
        };
        Ok(false)
    }

    fn values(&self) -> Values {
        let c = &self.cfg;
        values_of([
            (RawPlanSetting::AutoSf, Value::Bool(c.auto_sf)),
            (RawPlanSetting::Sf, Value::I64(i64::from(c.sf))),
            (RawPlanSetting::AutoBw, Value::Bool(c.auto_bw)),
            (RawPlanSetting::Bw, Value::Choice(self.bw.index())),
            (RawPlanSetting::Implicit, Value::Bool(c.implicit)),
            (RawPlanSetting::Cr, Value::I64(i64::from(c.cr))),
            (RawPlanSetting::Crc, Value::Bool(c.crc)),
            (RawPlanSetting::Length, Value::I64(i64::from(c.length))),
            (RawPlanSetting::Sync, Value::Choice(c.sync_choice.index())),
            (
                RawPlanSetting::SyncCustom,
                Value::I64(i64::from(c.sync_custom)),
            ),
            (RawPlanSetting::Preamble, Value::I64(i64::from(c.preamble))),
            (RawPlanSetting::Ldro, Value::Choice(c.ldro.index())),
            (RawPlanSetting::Offset, Value::Hz(c.offset)),
            (RawPlanSetting::JumpSearch, Value::Bool(c.jump_search)),
        ])
    }

    fn plan(&self, input: &Input) -> (BankPlan, RawPlan) {
        let dial = input.dial();
        let (bank, mut line) = raw_plan(&self.cfg, dial);
        for c in &bank.channels {
            if !sdrkit_proto_lora_phy::channel::in_band(
                input.rate,
                c.freq_hz - input.centre_hz,
                c.bw,
            ) {
                line = line.replace(
                    &format!("{} kHz", fmt_khz(c.bw)),
                    &format!("{} kHz (outside the captured band)", fmt_khz(c.bw)),
                );
            }
        }
        let item = RawPlan {
            bank: bank.clone(),
            line,
            dial_hz: dial,
            tx: self.cfg.tx_params(),
            tx_hz: dial + self.cfg.offset,
        };
        (bank, item)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRaWAN (all channels).

/// The settings of `lorawanwide`.
#[derive(Clone, Debug, PartialEq)]
pub struct WideConfig {
    /// The middle of the channels, Hz (0: the tuned frequency).
    pub dial_hz: f64,
    /// The channels of the plan. Without one, the plan is one channel at the dial.
    pub channels: Vec<lorawan::PlanChannel>,
    /// The FSK channel of the plan, Hz (0: none).
    pub fsk_hz: f64,
    /// Downlinks too.
    pub downlinks: bool,
    /// Default channels only.
    pub defaults_only: bool,
    /// Also the private sync word 0x12.
    pub private_sync: bool,
    /// Channels at most.
    pub max_channels: usize,
    /// 250 and 500 kHz channels.
    pub wide_channels: bool,
    /// The FSK channel.
    pub fsk: bool,
}

impl Default for WideConfig {
    fn default() -> Self {
        WideConfig {
            dial_hz: 0.0,
            channels: Vec::new(),
            fsk_hz: 0.0,
            downlinks: true,
            defaults_only: false,
            private_sync: false,
            max_channels: 24,
            wide_channels: true,
            fsk: true,
        }
    }
}

impl WideConfig {
    /// The dial for a session tuned to `tuned_hz`: the one set, else the tuned frequency, else 0.
    pub fn dial(&self, tuned_hz: Option<f64>) -> f64 {
        if self.dial_hz > 0.0 {
            self.dial_hz
        } else {
            tuned_hz.unwrap_or(0.0)
        }
    }
    /// Every channel of the plan at the dial `dial`, the channels for downlinks only when downlinks are decoded.
    pub fn plan_channels(&self, dial: f64) -> Vec<lorawan::PlanChannel> {
        let all = if self.channels.is_empty() {
            vec![lorawan::default_channel(dial.round())]
        } else {
            self.channels.clone()
        };
        all.into_iter()
            .filter(|c| c.uplink || self.downlinks)
            .collect()
    }
}

/// The plan of `lorawanwide` as an item (`lorawan.plan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WidePlan {
    /// The bank.
    pub bank: BankPlan,
    /// Every channel of the plan: centre and bandwidth, Hz.
    pub channels: Vec<(f64, f64)>,
    /// What is decoded, in words.
    pub note: String,
    /// The plan in words ("16 channels, 867.1 to 869.525 MHz").
    pub plan_name: String,
    /// The input states its centre.
    pub centre_known: bool,
    /// The FSK channel when it is decoded, Hz.
    pub fsk_hz: Option<f64>,
}

impl Item for WidePlan {
    const KIND: &'static str = "lorawan.plan";
}

/// The plan of `lorawanwide` at the dial `dial` for a capture at `rate` centred on `centre`.
pub fn wide_plan(
    c: &WideConfig,
    dial: f64,
    centre: f64,
    rate: f64,
    centre_known: bool,
) -> WidePlan {
    let all = c.plan_channels(dial);
    let list: Vec<_> = all
        .iter()
        .filter(|ch| !c.defaults_only || ch.default)
        .filter(|ch| c.wide_channels || ch.bw <= 125e3)
        .cloned()
        .collect();
    let n = list.len();
    let mut fit: Vec<_> = list
        .iter()
        .filter(|ch| sdrkit_proto_lora_phy::channel::in_band(rate, ch.hz - centre, ch.bw))
        .cloned()
        .collect();
    let f = fit.len();
    fit.sort_by(|a, b| (a.hz - centre).abs().total_cmp(&(b.hz - centre).abs()));
    fit.truncate(c.max_channels);
    fit.sort_by(|a, b| a.hz.total_cmp(&b.hz).then(a.bw.total_cmp(&b.bw)));
    let mut syncs = vec![0x34];
    if c.private_sync {
        syncs.push(0x12);
    }
    let mut tag = 0u32;
    let mut chans = Vec::new();
    for ch in &fit {
        let mut dets = Vec::new();
        for sf in ch.sf_min..=ch.sf_max {
            for inv in [false, true] {
                let use_it = if inv {
                    c.downlinks && ch.downlink
                } else {
                    ch.uplink
                };
                if !use_it {
                    continue;
                }
                let mut d = DetectorConfig::new(sf, ch.bw, 0x34);
                d.syncs = syncs.clone();
                d.invert = inv;
                d.preamble = 8;
                d.tag = tag;
                tag += 1;
                dets.push(d);
            }
        }
        chans.push(BankChannel {
            freq_hz: ch.hz,
            bw: ch.bw,
            detectors: dets,
        });
    }
    let k = chans.len();
    let fsk = Some(c.fsk_hz).filter(|&f| c.fsk && f > 0.0);
    let fsk_fits =
        fsk.is_some_and(|fh| rate >= 500e3 && (fh - centre).abs() + 100e3 <= 0.45 * rate);
    let mut note = format!(
        "{k} of {n} channels are decoded ({f} fit the {:.1} Msps capture around {} MHz{})",
        rate / 1e6,
        plan::fmt_mhz(centre),
        if f > k {
            ", the rest is over the channel limit"
        } else {
            ""
        }
    );
    if fsk.is_some() {
        note += if fsk_fits {
            ", and the FSK channel"
        } else {
            "; the FSK channel lies outside the capture"
        };
    }
    WidePlan {
        bank: BankPlan {
            channels: chans,
            fft_from: 3,
        },
        channels: all.iter().map(|c| (c.hz, c.bw)).collect(),
        note,
        plan_name: plan_words(&all),
        centre_known,
        fsk_hz: fsk.filter(|_| fsk_fits),
    }
}

/// The plan in words: the count of channels and their span.
fn plan_words(all: &[lorawan::PlanChannel]) -> String {
    let lo = all.iter().map(|c| c.hz).fold(f64::INFINITY, f64::min);
    let hi = all.iter().map(|c| c.hz).fold(f64::NEG_INFINITY, f64::max);
    match all.len() {
        0 => "no channel".into(),
        1 => format!("1 channel, {} MHz", plan::fmt_mhz(lo)),
        n => format!(
            "{n} channels, {} to {} MHz",
            plan::fmt_mhz(lo),
            plan::fmt_mhz(hi)
        ),
    }
}

/// The planner of `lorawanwide`.
#[derive(Default)]
pub struct WidePlanner {
    /// The settings.
    pub cfg: WideConfig,
    /// The text of [`LW_CHANNELS`].
    channels_text: String,
}

impl Planner for WidePlanner {
    type Item = WidePlan;
    type Setting = WidePlanSetting;
    const NAME: &'static str = "lorawan.channels";

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        let c = &mut self.cfg;
        match WidePlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            WidePlanSetting::DialFreq => c.dial_hz = WIDE_DIAL_FREQ.read(id, v)?,
            WidePlanSetting::LwChannels => {
                let t = LW_CHANNELS.read(id, v)?;
                c.channels = lw_list(id, t)?;
                self.channels_text = t.to_string();
            }
            WidePlanSetting::FskFreq => c.fsk_hz = FSK_FREQ.read(id, v)?,
            WidePlanSetting::Downlinks => c.downlinks = DOWNLINKS.read(id, v)?,
            WidePlanSetting::DefaultsOnly => c.defaults_only = DEFAULTS_ONLY.read(id, v)?,
            WidePlanSetting::PrivateSync => c.private_sync = PRIVATE_SYNC.read(id, v)?,
            WidePlanSetting::MaxChannels => c.max_channels = narrow(id, MAX_CHANNELS.read(id, v)?)?,
            WidePlanSetting::WideChannels => c.wide_channels = WIDE_CHANNELS.read(id, v)?,
            WidePlanSetting::Fsk => c.fsk = FSK.read(id, v)?,
        }
        Ok(false)
    }

    fn values(&self) -> Values {
        let c = &self.cfg;
        values_of([
            (WidePlanSetting::DialFreq, Value::Hz(c.dial_hz)),
            (
                WidePlanSetting::LwChannels,
                Value::Text(self.channels_text.clone()),
            ),
            (WidePlanSetting::Downlinks, Value::Bool(c.downlinks)),
            (WidePlanSetting::DefaultsOnly, Value::Bool(c.defaults_only)),
            (WidePlanSetting::PrivateSync, Value::Bool(c.private_sync)),
            (
                WidePlanSetting::MaxChannels,
                Value::I64(i64::try_from(c.max_channels).unwrap_or(i64::MAX)),
            ),
            (WidePlanSetting::WideChannels, Value::Bool(c.wide_channels)),
            (WidePlanSetting::Fsk, Value::Bool(c.fsk)),
            (WidePlanSetting::FskFreq, Value::Hz(c.fsk_hz)),
        ])
    }

    fn plan(&self, input: &Input) -> (BankPlan, WidePlan) {
        let known = input.centre_hz > 0.0;
        let dial = self.cfg.dial(input.tuned_hz);
        let centre = if known { input.centre_hz } else { dial };
        let p = wide_plan(&self.cfg, dial, centre, input.rate, known);
        (p.bank.clone(), p)
    }
}

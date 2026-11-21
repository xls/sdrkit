//! Project Horus balloon telemetry (`horus`): Horus Binary v1 and v2 (4FSK at 100 baud with Golay coding, interleaving and
//! scrambling) and the UKHAS sentences of Horus RTTY at 50, 100 and 300 baud, all read at once on every channel. The formats
//! are those of the horusdemodlib wiki and the UKHAS sentence convention; the payload ID list and the custom field formats
//! are data that the host gives as settings. The graph is a channel bank (the dial channel and the channels that the span
//! search opens, with their demodulators) and the payload table with the packet log. The layer 2 constants and the receiver
//! design are not confirmed by a public document.

pub mod bank;
pub mod demod;
pub mod gen;
pub mod layer2;
pub mod packet;
pub mod payloads;
pub mod span;

use crate::family::REPORT_S;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, Hz,
    LockProfile, Receiver, Setting, SettingSet, SignalInfo, StackConfig, StackDescriptor, Tuning,
    Value,
};
use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, GraphError, StackGraph};

/// The stack id.
pub const ID: &str = "horus";

sdrkit_core::choice_enum! {
    /// What the channels of the stack are doing.
    #[derive(Default)]
    pub enum State {
        /// No tones found yet.
        #[default]
        Waiting = ("waiting", "Waiting"),
        /// Tones found, no packet within the last 30 s.
        Listening = ("listening", "Listening"),
        /// A packet within the last 30 s.
        Decoding = ("decoding", "Decoding"),
    }
}

sdrkit_core::choice_enum! {
    /// The kind of a packet.
    pub enum Kind {
        /// Horus Binary v1.
        V1 = ("v1", "Horus Binary v1"),
        /// Horus Binary v2.
        V2 = ("v2", "Horus Binary v2"),
        /// Horus RTTY.
        Rtty = ("rtty", "RTTY"),
    }
}

sdrkit_core::choice_enum! {
    /// What a channel decodes.
    #[derive(Default)]
    pub enum Mode {
        /// No packet within the last 30 s.
        #[default]
        Listening = ("listening", "Listening"),
        /// Horus Binary within the last 30 s.
        Binary = ("binary", "Horus Binary"),
        /// RTTY within the last 30 s.
        Rtty = ("rtty", "RTTY"),
    }
}

/// The lowest input rate: a sound card with analytic samples.
pub const MIN_RATE: f64 = 16e3;

/// Look for other balloons in the span.
pub const SPAN_SEARCH: Setting<field::Bool> = Setting::new(
    "span_search",
    "Look for other balloons in the span",
    "",
    true,
)
.help("Opens a channel on each narrow signal of the span, at 40 ksps and more.");
/// The channels at most.
pub const MAX_CHANNELS: Setting<field::I64> =
    Setting::new("max_channels", "Channels at most", "", 6)
        .range(1.0, 8.0, 1.0)
        .help("The dial channel and the channels of the span search; 1 is the dial only.");
/// The payload ID list.
pub const PAYLOAD_IDS: Setting<field::Text> = Setting::new("payload_ids", "Payload IDs", "", "256, 4FSKTEST-V2").help(
    "One \"id, callsign\" a line (id 0 to 65535); # starts a comment; comma, tab, semicolon and spaces separate. An ID that the \
     list does not name is shown as \"ID n\".",
);
/// The custom field formats of v2.
pub const CUSTOM_FORMATS: Setting<field::Text> = Setting::new(
    "custom_formats",
    "Custom fields of v2",
    "",
    "4FSKTEST-V2|<hhBHxx|ascent_rate:divide_by_100,ext_temperature:divide_by_10,ext_humidity:none,ext_pressure:divide_by_10|||Default custom fields for RS41ng",
)
.help(
    "One format a line: callsign | struct | name:post-processing, ... | sentence names, ... | other payload IDs, ... | comment. \
     Post-processing: none, battery_5v_byte, divide_by_10, divide_by_100. A list with a line that does not read is refused.",
);
/// What the test signal sends.
pub const GEN_CONTENT: Setting<field::Choice<gen::Content>> = Setting::choice("gen_content", "Content", gen::Content::V2)
    .section("Test signal")
    .help("Binary v2 or v1, RTTY at 100 baud 7N2, 300 baud 8N2 or 50 baud 7N2, or three balloons (v2, v1 12 kHz above, RTTY 18 kHz below).");
/// The tone spacing of the test signal.
pub const GEN_SPACING: Setting<field::F64> =
    Setting::new("gen_spacing_hz", "Tone spacing", "Hz", 0.0)
        .range(0.0, 2000.0, 1.0)
        .section("Test signal")
        .help("0 for 270 Hz (4FSK) and 500 Hz (RTTY).");

sdrkit_core::settings! {
    /// The settings of the Horus stack.
    pub enum HorusSetting {
        /// Look for other balloons in the span.
        SpanSearch = SPAN_SEARCH,
        /// The channels at most.
        MaxChannels = MAX_CHANNELS,
        /// The payload ID list.
        PayloadIds = PAYLOAD_IDS,
        /// The custom field formats of v2.
        CustomFormats = CUSTOM_FORMATS,
        /// What the test signal sends.
        GenContent = GEN_CONTENT,
        /// The tone spacing of the test signal.
        GenSpacing = GEN_SPACING,
    }
}

/// The receiver: the channel bank and the payload table.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let shape = crate::typed::shape(ID);
    let graph = GraphBuilder::new()
        .add("channels", bank::ChannelBank::default())
        .add("payloads", payloads::PayloadTable::default())
        .input("channels")
        .input("payloads.in")
        .connect("channels.packets", "payloads.packets")
        .connect("channels.report", "payloads.report")
        .output("payloads.events")
        .clock(REPORT_S)
        .status_from("payloads")
        .declare(&shape.results_decl())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut rx = StackGraph::of(shape, graph)
        .min_rate(MIN_RATE, "Horus needs a sample rate of at least 16 ksps");
    for (key, component) in [
        (SPAN_SEARCH.key(), "channels"),
        (MAX_CHANNELS.key(), "channels"),
        (PAYLOAD_IDS.key(), "payloads"),
        (CUSTOM_FORMATS.key(), "payloads"),
    ] {
        rx = rx.route(key, &[&format!("{component}.{key}")])?;
    }
    rx.command(
        sdrkit_util_objects::commands::CLEAR.key,
        CommandTarget::every(sdrkit_util_objects::commands::CLEAR.key),
    )
    .with_settings(&cfg.settings)
    .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// The test signal of the stack at the rate of `cfg`: the content of `gen_content` with the spacing of `gen_spacing_hz`,
/// each balloon at unit power while it transmits, the dial balloon at 0 Hz. Noise, carrier offset, clock error and the
/// mirrored spectrum come from the standard impairments.
pub struct HorusGenerator {
    rate: f64,
    signal: gen::Signal,
}

impl Generator for HorusGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.signal.fill(out);
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the specification is that of one balloon in 3 kHz.
        Some(
            SignalInfo::new(3_000.0)
                .with_symbol_rate(demod::BAUD)
                .with_bit_rate(2.0 * demod::BAUD),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator of the test signal.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let spacing = cfg.settings.get(&GEN_SPACING);
    let o = gen::GenOptions {
        rate_hz: cfg.rate_hz,
        content: cfg.settings.get(&GEN_CONTENT),
        spacing_hz: (spacing > 0.0).then_some(spacing),
        amplitude: 1.0,
        ..gen::GenOptions::default()
    };
    if o.content == gen::Content::Three && cfg.rate_hz < 64e3 {
        return Err(ConfigError::RateTooLow {
            need: 64e3,
            got: cfg.rate_hz,
        });
    }
    Ok(Box::new(HorusGenerator {
        rate: cfg.rate_hz,
        signal: gen::Signal::new(&o),
    }))
}

/// The verify cases: the v2 test signal on the dial, and three balloons in the span.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut one = VerifyCase::new("test signal, binary v2", o, 0.0);
    one.seconds = o.seconds.max(10.0);
    one.expect_audio = Some(false);
    one.check = Some(|r| {
        let n = crate::family::field(r, crate::typed::HorusLogFields::PACKETS.key);
        vec![CheckLine::new(
            crate::typed::HorusLogFields::PACKETS.key,
            n >= 2.0,
            format!("{n}, 2 expected"),
        )]
    });
    let mut three = VerifyCase::new("test signal, three balloons", o, 0.0)
        .with(&GEN_CONTENT, gen::Content::Three);
    three.seconds = o.seconds.max(20.0);
    three.expect_audio = Some(false);
    three.check = Some(|r| {
        let n = crate::family::field(r, crate::typed::HorusFields::CHANNELS.key);
        let p = r
            .decl()
            .table_id(crate::typed::HORUS_PAYLOADS.key)
            .and_then(|t| r.table(t))
            .map_or(0, |t| t.len());
        vec![
            CheckLine::new(
                crate::typed::HorusFields::CHANNELS.key,
                n >= 3.0,
                format!("{n}, 3 expected"),
            ),
            CheckLine::new(
                crate::typed::HORUS_PAYLOADS.key,
                p >= 3,
                format!("{p}, 3 expected"),
            ),
        ]
    });
    vec![one, three]
}

/// Horus at 960 ksps: the dial channel and the span search over 20 s of three balloons.
fn horus_960k(s: f64) -> BenchResult {
    let content = [(
        HorusSetting::GenContent.id(),
        Value::Choice(gen::Content::Three as u16),
    )];
    let x = crate::bench::gen(&DESCRIPTOR, 960e3, 20.0, &content);
    crate::bench::run(&DESCRIPTOR, 960e3, DESCRIPTOR.tuning.default_hz, s, &x, &[])
}

/// The benchmarks of the stack.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "science.horus_960k",
        run: horus_960k,
    }]
}

/// Horus balloons.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Horus balloons",
    group: "Science, time and utility",
    blurb: "Amateur high-altitude balloons: Horus Binary v1 and v2 and RTTY telemetry, several balloons at once across the span, \
            with their tracks, climb rates and custom sensor fields. The coding constants of Horus Binary are not confirmed \
            by a public document.",
    // The stack holds no frequency: the host sets it, for example from data/science/horus_frequencies.csv.
    tuning: Tuning {
        min_hz: 400e6,
        max_hz: 450e6,
        default_hz: 0.0,
        rate_hz: 960e3,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.9e6,
        tune_offset_hz: 30e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: HorusSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: Some(benchmarks),
};

/// Adds the components of the stack to `r`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("horus.channels", || Box::new(bank::ChannelBank::default()));
    r.add("horus.payloads", || {
        Box::new(payloads::PayloadTable::default())
    });
}

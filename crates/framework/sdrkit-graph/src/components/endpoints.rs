//! The ends of a graph that reach outside it: the host's streams (`host.out`, `host.in`), the transmit sink (`tx.sink`), a stack's test
//! signal as a source (`gen.stack`) and a network sink (`net.sink`). Each reaches the outside through what the
//! [`RunContext`] returns.

use crate::component::{values_of, Applied, Component, ComponentInfo, Io, Port};
use crate::context::RunContext;
use crate::stream::{Format, Kind, TagKind};
use sdrkit_core::interface::Fields;
use sdrkit_core::net::{Conn, Datagram, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Generator, Published, Results, Setting, SettingError,
    SettingId, SettingSet, StageError, Value, Values,
};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

/// The samples that an endpoint passes to the host in one call.
#[derive(Clone, Copy, Debug)]
pub enum HostData<'a> {
    /// Interleaved audio frames.
    Audio {
        /// The samples, `channels` per frame.
        samples: &'a [f32],
        /// The frames per second.
        rate_hz: f64,
        /// The samples per frame.
        channels: u16,
    },
    /// Complex samples.
    Iq {
        /// The samples.
        samples: &'a [Complex32],
        /// The samples per second.
        rate_hz: f64,
        /// The frequency at 0 Hz of the samples.
        centre_hz: f64,
        /// The last sample ends a burst ([`TagKind::BurstEnd`]).
        end: bool,
        /// The first sample is the first of a burst and must leave the antenna at this UTC time, seconds since
        /// 1970-01-01 ([`TagKind::BurstStart`]). The samples are written before that time, early by at least
        /// [`HostStreams::transmit_lead`]; written later, the burst goes out at once. `None`: the samples go out
        /// as they come.
        start: Option<f64>,
    },
}

/// The stream of the host that an endpoint writes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPort {
    /// The audio that the host drains.
    Out,
    /// The samples that the host supplies to a transmitter.
    Transmit,
    /// The audio that the host gives to the graph, such as a microphone.
    Microphone,
}

/// The rate of the audio that [`HostStreams::read`] gives and `host.in` puts out, Hz.
pub const HOST_IN_RATE: f64 = 48_000.0;

/// The host's side of the endpoints of a graph. The host supplies it in [`RunContext::streams`]. A call must not wait.
pub trait HostStreams: Send + Sync {
    /// Takes `data` for the host stream `port`.
    fn write(&self, port: HostPort, data: HostData<'_>);
    /// Copies the audio that the host gave for `port` into `out`, oldest first: mono samples at [`HOST_IN_RATE`].
    /// Returns the count of samples copied, 0 when none wait. The default has no audio.
    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        let _ = (port, out);
        0
    }
    /// As [`HostStreams::read`], in whole interleaved frames of `channels` channels. A host that got another channel
    /// count converts: one channel goes to every channel, and more channels become their mean for one. Returns the
    /// count of samples copied. The default takes the mono audio of [`HostStreams::read`] and puts it in every
    /// channel.
    fn read_channels(&self, port: HostPort, channels: u16, out: &mut [f32]) -> usize {
        let c = usize::from(channels.max(1));
        let room = out.len() / c;
        let frames = self.read(port, &mut out[..room]);
        for f in (0..frames).rev() {
            let s = out[f];
            out[f * c..(f + 1) * c].fill(s);
        }
        frames * c
    }
    /// Tells the host the rate and the centre of the samples that will come for [`HostPort::Transmit`], before the
    /// first of them: the host can then prepare its transmitter and know [`HostStreams::transmit_lead`]. The
    /// default does nothing.
    fn transmit_format(&self, rate_hz: f64, centre_hz: f64) {
        let _ = (rate_hz, centre_hz);
    }
    /// How long before its stated start ([`HostData::Iq`] `start`) the first sample of a burst must be written: the
    /// latency of the transmitter and the time that the host needs to hand the burst over. Zero (the default) while
    /// the host does not know it or does not time bursts: a component then states no start.
    fn transmit_lead(&self) -> Duration {
        Duration::ZERO
    }
    /// A component kept the audio that the host gave for `port` in this pass: it uses it, such as a sender whose
    /// transmission carries it. Called once per pass while that holds, also when no audio came. The host can then
    /// tell when its audio is wanted. The default does nothing.
    fn kept(&self, port: HostPort) {
        let _ = port;
    }
}

/// Makes the test signal of a stack: the stack's id and the sample rate (0 for the stack's own rate). `None` if no such stack
/// has a test signal.
pub type MakeGenerator = Arc<dyn Fn(&str, f64) -> Option<Box<dyn Generator>> + Send + Sync>;

/// Why an endpoint cannot run.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EndpointError {
    /// The run context supplies no test signals.
    #[error("the host gives no test signals")]
    NoGenerators,
    /// No stack of this id has a test signal.
    #[error("no stack \"{stack}\" with a test signal")]
    NoSuchGenerator {
        /// The requested stack id.
        stack: String,
    },
    /// The run context supplies no network.
    #[error("the host gives no network")]
    NoTransport,
    /// The address is not `host:port`.
    #[error("\"{address}\" is not host:port")]
    BadAddress {
        /// The text given.
        address: String,
    },
}

fn stage(e: EndpointError) -> ConfigError {
    ConfigError::Stage(StageError::new(e))
}

/// The host's audio (`host.out`): the audio goes to the host's audio stream of the graph, which the host drains, as the audio
/// at the graph's outputs does. Without a host the audio is dropped.
#[derive(Default)]
pub struct HostOut {
    streams: Option<Arc<dyn HostStreams>>,
    format: Option<Format>,
}

impl Component for HostOut {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("host.out", vec![Port::audio("in")])
    }
    fn values(&self) -> Values {
        crate::no_values()
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.streams.clone_from(&ctx.streams);
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.format = Some(inputs[0].clone());
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let (Some(s), Some(f)) = (&self.streams, &self.format) else {
            return;
        };
        write_input(&**s, HostPort::Out, f, io);
    }
}

/// Hands input 0 of `io`, of format `f`, to `port` of the host. Complex samples are cut after each
/// [`TagKind::BurstEnd`] and before each [`TagKind::BurstStart`]: the part that ends at an end is marked `end`, and
/// the part that begins at a start carries its time.
fn write_input(s: &dyn HostStreams, port: HostPort, f: &Format, io: &Io) {
    if f.kind == Kind::Iq {
        let samples = io.iq(0);
        let input = io.input(0);
        let place = |at: u64| usize::try_from(at.checked_sub(input.first())?).ok();
        // The cuts in the order of the samples: the count of samples before the cut, and the start that follows it.
        let mut cuts: Vec<(usize, Option<f64>)> = input
            .tags()
            .iter()
            .filter_map(|t| match t.kind {
                TagKind::BurstEnd => Some((place(t.at)? + 1, None)),
                TagKind::BurstStart { unix_s } => Some((place(t.at)?, Some(unix_s))),
                _ => None,
            })
            .collect();
        cuts.sort_by_key(|c| (c.0, c.1.is_some()));
        let (mut from, mut start) = (0, None);
        let write = |from: usize, to: usize, end: bool, start: Option<f64>| {
            s.write(
                port,
                HostData::Iq {
                    samples: &samples[from..to],
                    rate_hz: f.rate,
                    centre_hz: f.centre_hz,
                    end,
                    start,
                },
            );
        };
        for (to, stated) in cuts {
            let to = to.min(samples.len());
            if to < from {
                continue;
            }
            match stated {
                None => {
                    write(from, to, true, start.take());
                    from = to;
                }
                Some(utc) => {
                    if to > from {
                        write(from, to, false, start.take());
                        from = to;
                    }
                    start = Some(utc);
                }
            }
        }
        if from < samples.len() {
            write(from, samples.len(), false, start);
        }
    } else {
        let a = io.audio(0);
        if !a.samples.is_empty() {
            s.write(
                port,
                HostData::Audio {
                    samples: &a.samples,
                    rate_hz: f.rate,
                    channels: a.channels.max(1),
                },
            );
        }
    }
}

/// The samples that `host.in` takes from the host in one call.
const HOST_IN_CHUNK: usize = 4800;

/// The host's audio input (`host.in`): the audio that the host gives to the session, such as a microphone, as audio
/// at [`HOST_IN_RATE`] on output `out`: mono, or stereo for [`HostIn::stereo`]. The host converts audio of the other
/// channel count ([`HostStreams::read_channels`]). Input `in` is the samples of the session: each pass of them takes
/// all the audio that waits, at most one second. Without a host, and while the host gives nothing, the output is
/// empty.
pub struct HostIn {
    streams: Option<Arc<dyn HostStreams>>,
    channels: u16,
    chunk: Vec<f32>,
}

impl Default for HostIn {
    fn default() -> Self {
        HostIn {
            streams: None,
            channels: 1,
            chunk: vec![0.0; HOST_IN_CHUNK],
        }
    }
}

impl HostIn {
    /// A host audio input that puts out stereo, for a component whose input takes two channels.
    pub fn stereo() -> Self {
        HostIn {
            channels: 2,
            ..HostIn::default()
        }
    }

    /// The channels of the audio that the input puts out: 1 or 2.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Component for HostIn {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("host.in", vec![Port::iq("in")], vec![Port::audio("out")])
    }
    fn values(&self) -> Values {
        crate::no_values()
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.streams.clone_from(&ctx.streams);
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::audio(HOST_IN_RATE, self.channels)])
    }
    /// Open, so the code that owns the graph reads [`HostIn::channels`].
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn process(&mut self, io: &mut Io) {
        let Some(s) = &self.streams else {
            return;
        };
        let out = &mut io.audio_out(0).samples;
        let most = HOST_IN_RATE as usize * usize::from(self.channels);
        let mut taken = 0;
        while taken < most {
            let n = s.read_channels(HostPort::Microphone, self.channels, &mut self.chunk);
            out.extend_from_slice(&self.chunk[..n]);
            taken += n;
            if n < self.chunk.len() {
                break;
            }
        }
    }
}

/// The transmit sink (`tx.sink`): complex samples for a transmitter. The host drains them. Without a host they are dropped.
/// A [`TagKind::BurstEnd`] on the input ends a burst: the transmitter stays on air until it comes. A
/// [`TagKind::BurstStart`] states when the burst that begins there leaves the antenna. The host learns the format of
/// the samples when the sink is configured ([`HostStreams::transmit_format`]).
#[derive(Default)]
pub struct TxSink {
    streams: Option<Arc<dyn HostStreams>>,
    format: Option<Format>,
    /// The centre of the graph's input at the last configure or retune.
    input_centre_hz: Option<f64>,
}

impl TxSink {
    /// Tells the host the format of the samples, once the sink has both the host and the format.
    fn announce(&self) {
        if let (Some(s), Some(f)) = (&self.streams, &self.format) {
            s.transmit_format(f.rate, f.centre_hz);
        }
    }
}

impl Component for TxSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("tx.sink", vec![Port::iq("in")])
    }
    fn values(&self) -> Values {
        crate::no_values()
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.streams.clone_from(&ctx.streams);
        self.announce();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.format = Some(inputs[0].clone());
        self.input_centre_hz = None;
        self.announce();
        Ok(vec![])
    }
    /// A retune that keeps the components moves the transmit centre by the step of the input centre, and tells the host.
    fn retuned(&mut self, centre_hz: f64, _tuned_hz: f64) {
        if let (Some(before), Some(f)) = (self.input_centre_hz, self.format.as_mut()) {
            let step = centre_hz - before;
            if step != 0.0 {
                f.centre_hz += step;
                self.announce();
            }
        }
        self.input_centre_hz = Some(centre_hz);
    }
    fn process(&mut self, io: &mut Io) {
        let (Some(s), Some(f)) = (&self.streams, &self.format) else {
            return;
        };
        write_input(&**s, HostPort::Transmit, f, io);
    }
}

const STACK: Setting<field::Text> =
    Setting::new("stack", "Stack", "", "").help("The id of the stack whose test signal plays.");
const RATE: Setting<field::Hz> = Setting::new("rate_hz", "Rate", "Hz", 0.0)
    .range(0.0, 1e9, 0.0)
    .help("0 plays at the stack's own rate.");
const CENTRE: Setting<field::Hz> =
    Setting::new("centre_hz", "Centre", "Hz", 0.0).range(0.0, 1e12, 0.0);
const CHUNK: Setting<field::Duration> = Setting::new("chunk_s", "Chunk", "s", 0.02)
    .range(0.001, 1.0, 0.0)
    .help("The signal made in each pass of the graph.");

sdrkit_core::settings! {
    /// The settings of [`StackGenerator`].
    pub enum StackGeneratorSetting {
        /// The stack.
        Stack = STACK,
        /// The sample rate.
        Rate = RATE,
        /// The frequency at the centre of the samples.
        Centre = CENTRE,
        /// The length of signal per pass.
        Chunk = CHUNK,
    }
}

/// The result set of [`StackGenerator`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct StackGeneratorFields {
    /// The finite test signal of the stack has ended. No value before the component is configured.
    #[field(label = "Ended")]
    pub ended: Option<bool>,
}

/// A stack's test signal as a source (`gen.stack`): each pass of the graph makes `chunk_s` seconds of it. The graph's runner
/// paces the passes. The run context makes the signal ([`RunContext::generators`]).
pub struct StackGenerator {
    stack: String,
    rate_hz: f64,
    centre_hz: f64,
    chunk_s: f64,
    make: Option<MakeGenerator>,
    gen: Option<Box<dyn Generator>>,
    chunk: usize,
    results: Published,
}

impl Default for StackGenerator {
    fn default() -> Self {
        StackGenerator {
            stack: String::new(),
            rate_hz: RATE.default,
            centre_hz: CENTRE.default,
            chunk_s: CHUNK.default,
            make: None,
            gen: None,
            chunk: 0,
            results: Published::new(StackGeneratorFields::SHAPE.fields),
        }
    }
}

impl Component for StackGenerator {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("gen.stack", vec![Port::iq("out")])
            .with_settings(StackGeneratorSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (
                StackGeneratorSetting::Stack,
                Value::Text(self.stack.clone()),
            ),
            (StackGeneratorSetting::Rate, Value::Hz(self.rate_hz)),
            (StackGeneratorSetting::Centre, Value::Hz(self.centre_hz)),
            (StackGeneratorSetting::Chunk, Value::Duration(self.chunk_s)),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.make.clone_from(&ctx.generators);
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let make = self
            .make
            .as_ref()
            .ok_or(stage(EndpointError::NoGenerators))?;
        let gen = make(&self.stack, self.rate_hz).ok_or_else(|| {
            stage(EndpointError::NoSuchGenerator {
                stack: self.stack.clone(),
            })
        })?;
        let rate = gen.rate_hz();
        if rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.chunk = ((rate * self.chunk_s).round() as usize).max(1);
        self.gen = Some(gen);
        self.results
            .put(&StackGeneratorFields { ended: Some(false) });
        Ok(vec![Format::iq(rate, self.centre_hz, self.centre_hz)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(g) = &mut self.gen else { return };
        let out = io.iq_out(0);
        let start = out.len();
        out.resize(start + self.chunk, Complex32::default());
        let n = g.generate(&mut out[start..]);
        out.truncate(start + n);
        if n < self.chunk {
            self.results
                .put(&StackGeneratorFields { ended: Some(true) });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match StackGeneratorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            StackGeneratorSetting::Stack => STACK.read(id, v)?.clone_into(&mut self.stack),
            StackGeneratorSetting::Rate => self.rate_hz = RATE.read(id, v)?,
            StackGeneratorSetting::Centre => self.centre_hz = CENTRE.read(id, v)?,
            StackGeneratorSetting::Chunk => self.chunk_s = CHUNK.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

sdrkit_core::choice_enum! {
    /// How a network sink sends.
    pub enum NetProtocol {
        /// UDP datagrams.
        Udp = ("udp", "UDP"),
        /// A TCP connection to a server.
        Tcp = ("tcp", "TCP"),
    }
}

sdrkit_core::choice_enum! {
    /// The form of each sample that a network sink sends.
    pub enum NetSample {
        /// 32-bit floats, little-endian.
        F32 = ("f32", "32-bit float"),
        /// 16-bit signed integers, little-endian, full scale at 1.0.
        S16 = ("s16", "16-bit integer"),
    }
}

const PROTOCOL: Setting<field::Choice<NetProtocol>> =
    Setting::choice("protocol", "Protocol", NetProtocol::Udp);
const ADDRESS: Setting<field::Text> = Setting::new("address", "Address", "", "127.0.0.1:7355")
    .help("host:port of the receiver of the samples.");
const SAMPLE: Setting<field::Choice<NetSample>> =
    Setting::choice("sample", "Sample", NetSample::F32);

sdrkit_core::settings! {
    /// The settings of [`NetSink`].
    pub enum NetSinkSetting {
        /// UDP or TCP.
        Protocol = PROTOCOL,
        /// Where to send.
        Address = ADDRESS,
        /// The form of each sample.
        Sample = SAMPLE,
    }
}

/// The result set of the network sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct NetSinkFields {
    /// The bytes sent. No value before the first send that succeeded.
    #[field(label = "Sent", unit = "B")]
    pub sent_bytes: Option<i64>,
    /// The sends that failed. The samples of a failed send are lost. No value before the first failure.
    #[field(label = "Failed sends")]
    pub failed: Option<i64>,
}

/// The largest UDP payload that the sink sends in one datagram.
const DATAGRAM: usize = 1472;
/// The longest wait for a TCP connection.
const CONNECT: Duration = Duration::from_secs(2);

enum Socket {
    Udp(Box<dyn Datagram>),
    Tcp(Box<dyn Conn>),
}

/// A network sink (`net.sink`): complex samples or audio, as interleaved little-endian numbers, to a UDP receiver or a TCP
/// server. The run context supplies the network ([`RunContext::transport`]). The sink opens its socket when it is configured.
/// After a failed send it opens it again at the next configure.
pub struct NetSink {
    protocol: NetProtocol,
    address: String,
    sample: NetSample,
    transport: Option<Arc<dyn Transport>>,
    socket: Option<Socket>,
    bytes: Vec<u8>,
    sent: i64,
    sent_once: bool,
    failed: i64,
    results: Published,
}

impl Default for NetSink {
    fn default() -> Self {
        NetSink {
            protocol: PROTOCOL.default,
            address: ADDRESS.default.to_owned(),
            sample: SAMPLE.default,
            transport: None,
            socket: None,
            bytes: Vec::new(),
            sent: 0,
            sent_once: false,
            failed: 0,
            results: Published::new(NetSinkFields::SHAPE.fields),
        }
    }
}

impl NetSink {
    fn open(&self) -> Result<Socket, ConfigError> {
        let t = self
            .transport
            .as_ref()
            .ok_or(stage(EndpointError::NoTransport))?;
        let bad = || {
            stage(EndpointError::BadAddress {
                address: self.address.clone(),
            })
        };
        let (host, port) = self.address.rsplit_once(':').ok_or_else(bad)?;
        let port: u16 = port.parse().map_err(|_| bad())?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let net = |e: sdrkit_core::net::NetError| ConfigError::Stage(StageError::new(e));
        Ok(match self.protocol {
            NetProtocol::Udp => Socket::Udp(t.datagram(host, port).map_err(net)?),
            NetProtocol::Tcp => Socket::Tcp(t.connect(host, port, false, CONNECT).map_err(net)?),
        })
    }

    fn encode(&mut self, values: impl Iterator<Item = f32>) {
        self.bytes.clear();
        match self.sample {
            NetSample::F32 => values.for_each(|v| self.bytes.extend_from_slice(&v.to_le_bytes())),
            NetSample::S16 => values.for_each(|v| {
                let s = (v * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
                self.bytes.extend_from_slice(&s.to_le_bytes());
            }),
        }
    }

    fn send(&mut self) {
        let Some(sock) = &mut self.socket else { return };
        let ok = match sock {
            Socket::Udp(d) => self.bytes.chunks(DATAGRAM).all(|c| d.send(c).is_ok()),
            Socket::Tcp(c) => c.write_all(&self.bytes).is_ok(),
        };
        if ok {
            self.sent += self.bytes.len() as i64;
            self.sent_once = true;
        } else {
            self.failed += 1;
            self.socket = None;
        }
        self.results.put(&NetSinkFields {
            sent_bytes: self.sent_once.then_some(self.sent),
            failed: (self.failed > 0).then_some(self.failed),
        });
    }
}

impl Component for NetSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "net.sink",
            vec![Port::one_of("in", &[Kind::Iq, Kind::Audio])],
        )
        .with_settings(NetSinkSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (
                NetSinkSetting::Protocol,
                Value::Choice(self.protocol.index()),
            ),
            (NetSinkSetting::Address, Value::Text(self.address.clone())),
            (NetSinkSetting::Sample, Value::Choice(self.sample.index())),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transport.clone_from(&ctx.transport);
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.socket = Some(self.open()?);
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if self.socket.is_none() {
            return;
        }
        if io.input_format(0).kind == Kind::Iq {
            let x = io.iq(0);
            if x.is_empty() {
                return;
            }
            self.encode(x.iter().flat_map(|c| [c.re, c.im]));
        } else {
            let a = io.audio(0);
            if a.samples.is_empty() {
                return;
            }
            self.encode(a.samples.iter().copied());
        }
        self.send();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NetSinkSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NetSinkSetting::Protocol => self.protocol = PROTOCOL.read(id, v)?,
            NetSinkSetting::Address => ADDRESS.read(id, v)?.clone_into(&mut self.address),
            NetSinkSetting::Sample => self.sample = SAMPLE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

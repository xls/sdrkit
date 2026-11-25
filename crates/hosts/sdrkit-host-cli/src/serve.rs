//! The commands for USB radios and for serving: `devices` lists the attached USB radios, `serve` shares a source with
//! network clients over rtl_tcp and SoapyRemote, and `usb record` writes the USB exchanges of a radio as a script.

use crate::error::CliError;
use crate::host::{self, Live};
use crate::live::{self, Source, Tune};
use crate::out::Out;
use crate::reflect;
use crate::text::{self, Bare};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Choice, Owner, Value};
use sdrkit_host_engine::{
    Engine, ListenArgs, ListenReply, OwnerRef, RadioId, ShareControl, ShareFields, ShareFormat,
    ShareId, ShareProtocol, DEVICES, LISTEN,
};
use std::time::{Duration, Instant};

/// How often a command that follows looks for changes.
const FOLLOW: Duration = Duration::from_millis(200);
/// The usual port of an rtl_tcp server.
pub const RTL_TCP_PORT: u16 = 1234;
/// The usual port of a SoapyRemote server.
pub const SOAPY_PORT: u16 = 55132;

/// `devices`: the table of the attached USB radios, and with `follow` each arrival and removal until `stop` returns
/// true. Listing opens no device.
///
/// # Errors
///
/// What the engine returns.
pub fn devices(
    engine: &Engine,
    follow: bool,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    // The first tick of the engine lists the bus.
    engine.settle(Duration::from_secs(2));
    let mut since = reflect::print_table(engine, OwnerRef::Engine, "engine", DEVICES.key, 0, out)?;
    if !follow {
        return Ok(());
    }
    // The events of the start are not arrivals.
    let mut waiting = Vec::new();
    engine.take_events(OwnerRef::Engine, &mut waiting)?;
    while !stop() {
        std::thread::sleep(FOLLOW);
        let mut events = Vec::new();
        engine.take_events(OwnerRef::Engine, &mut events)?;
        let n = events
            .iter()
            .filter(|e| e.kind().starts_with("device."))
            .count();
        if n > 0 {
            reflect::write_events(engine, "engine", &events, out)?;
            since =
                reflect::print_table(engine, OwnerRef::Engine, "engine", DEVICES.key, since, out)?;
        }
    }
    Ok(())
}

/// What `serve` does.
#[derive(Clone, Debug, Default)]
pub struct Serve {
    /// The source, as `run` takes it.
    pub source: String,
    /// `[host:]port` of an rtl_tcp listener.
    pub rtl_tcp: Option<String>,
    /// `[host:]port` of a SoapyRemote listener.
    pub soapy: Option<String>,
    /// The control: follow, remote or slice.
    pub control: Option<String>,
    /// The sample format that the share carries: native, cu8, cs8, cs16 or cf32.
    pub format: Option<String>,
    /// A stack to run on the source beside the share.
    pub stack: Option<String>,
    /// The frequency, the rate, the gain and the settings of the radio (and of the stack).
    pub tune: Tune,
    /// Stop after this many seconds.
    pub seconds: Option<f64>,
}

/// The host and the port of `[host:]port`. A text without a host takes every interface.
fn host_port(text: &str) -> Result<(String, u16), CliError> {
    let bad = || CliError::BadNumber {
        text: text.to_owned(),
    };
    let (host, port) = match text.rsplit_once(':') {
        Some((h, p)) => (h.trim_matches(['[', ']']), p),
        None => (sdrkit_host_engine::DEFAULT_HOST, text),
    };
    if host.is_empty() {
        return Err(bad());
    }
    Ok((host.to_owned(), port.parse().map_err(|_| bad())?))
}

/// The choice of `E` with the key `key`.
fn choice<E: Choice>(key: &str, what: &str) -> Result<E, CliError> {
    E::from_key(key).ok_or_else(|| CliError::BadValue {
        key: what.to_owned(),
        text: key.to_owned(),
        kind: sdrkit_core::ValueKind::Choice,
        forms: format!(
            ": {}",
            E::CHOICES
                .iter()
                .map(|c| c.key.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}

/// Opens the radio of `serve` without a stack: the frequency, the rate, the gain and the `--set` values go to the radio.
fn open_radio(engine: &Engine, source: &Source, tune: &Tune) -> Result<RadioId, CliError> {
    let kind = engine
        .find(&format!("driver.{}", source.driver))
        .map_err(|_| CliError::UnknownDriver {
            key: source.driver.clone(),
        })?;
    let mut values = host::defaults(engine, kind)?;
    let mut put = |key: &str, v: Value| -> Result<(), CliError> {
        let id = values
            .schema()
            .id(key)
            .map_err(|_| CliError::UnknownSetting {
                owner: format!("the driver `{}`", source.driver),
                key: key.to_owned(),
            })?;
        values
            .set(id, &v)
            .map_err(|e| sdrkit_host_engine::EngineError::setting(values.schema(), e))?;
        Ok(())
    };
    if let Some(f) = &tune.freq {
        put(
            sdrkit_driver_core::CENTRE.field.key,
            Value::Hz(text::parse_hz(f, Bare::Frequency)?),
        )?;
    }
    if let (Some(r), None) = (&tune.rate, &source.mount) {
        put(
            sdrkit_driver_core::RATE.field.key,
            Value::Hz(text::parse_hz(r, Bare::Hz)?),
        )?;
    }
    if let Some(g) = tune.gain {
        put("gain", Value::F64(g))?;
        put("agc", Value::Bool(false))?;
    }
    let radio = engine
        .open_radio(&source.driver, &source.address, &values)
        .map_err(|e| CliError::of_open(engine, &source.driver, e))?;
    for s in &tune.set {
        let (k, v) = text::key_value(s)?;
        let key = k.strip_prefix("radio.").unwrap_or(k);
        if let Err(e) = engine.set_text(OwnerRef::Radio(radio), key, v) {
            let _ = engine.close_radio(radio);
            return Err(e.into());
        }
    }
    Ok(radio)
}

/// Starts one listener of `share` and prints where it listens.
fn listen(
    engine: &Engine,
    share: OwnerRef,
    protocol: ShareProtocol,
    at: &str,
    out: &mut Out,
) -> Result<(), CliError> {
    let (host, port) = host_port(at)?;
    let reply = engine.owner(share, |o| {
        LISTEN.call(
            o,
            &ListenArgs {
                protocol,
                port: i64::from(port),
                host: Some(&host),
            },
        )
    })?;
    let reply = reply.map_err(|error| sdrkit_host_engine::EngineError::Command {
        text: error.to_string(),
        error,
    })?;
    let port = reply
        .as_ref()
        .and_then(|r| LISTEN.read_reply::<ListenReply>(r))
        .map_or(i64::from(port), |r| r.port);
    out.line(&format!("{} on {host}:{port}", protocol.label()))
}

/// One line with the state of the share.
fn status(engine: &Engine, share: OwnerRef) -> Result<String, CliError> {
    Ok(engine.owner(share, |o: &mut dyn Owner| {
        let Some(f) = ShareFields::read(o.results().fields()) else {
            return String::new();
        };
        let mut s = format!(
            "{} at {} around {}, {} clients, {} samples",
            f.format,
            text::hz(f.rate_hz.0),
            text::hz(f.centre_hz.0),
            f.clients,
            f.samples
        );
        if f.requests_refused > 0 {
            s.push_str(&format!(
                ", {} requests not carried out",
                f.requests_refused
            ));
        }
        if f.transmitting {
            s.push_str(", a client transmits");
        }
        if let Some(p) = f.problem {
            s.push_str(&format!(", problem: {p}"));
        }
        s
    })?)
}

/// `serve`: opens the source, runs the stack `--stack` on it if one is given, shares the radio and listens, then prints
/// the clients that come and go and a status line until `stop` returns true or the time is over. Without `--rtl-tcp` and
/// `--soapy` it listens for rtl_tcp on its usual port.
///
/// # Errors
///
/// [`CliError::UnknownDriver`], [`CliError::UnknownStack`], [`CliError::BadValue`] for a control or a format that does
/// not exist, the errors of the engine.
pub fn serve(
    engine: &Engine,
    s: &Serve,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let control: Option<ShareControl> = s
        .control
        .as_deref()
        .map(|c| choice(c, "--control"))
        .transpose()?;
    let format: Option<ShareFormat> = s
        .format
        .as_deref()
        .map(|c| choice(c, "--format"))
        .transpose()?;
    let stack = s.stack.as_deref().map(live::stack).transpose()?;
    let source = Source::parse(&s.source, stack.map_or("", |d| d.id));
    let live = match stack {
        Some(desc) => Some(live::start(engine, &source, desc, &s.tune, None, out)?),
        None => None,
    };
    let radio = match &live {
        Some(l) => l.radio,
        None => Some(open_radio(engine, &source, &s.tune)?),
    };
    let r = radio
        .ok_or(sdrkit_host_engine::EngineError::NoRadio)
        .map_err(CliError::from)
        .and_then(|radio| shared(engine, radio, s, control, format, out, stop));
    let e = end(engine, live.as_ref(), radio);
    r.and(e)
}

/// Closes what `serve` opened.
fn end(engine: &Engine, live: Option<&Live>, radio: Option<RadioId>) -> Result<(), CliError> {
    match (live, radio) {
        (Some(l), _) => live::end(engine, l),
        (None, Some(r)) => Ok(engine.close_radio(r)?),
        (None, None) => Ok(()),
    }
}

/// Shares `radio` as `s` says and follows the share until the end.
fn shared(
    engine: &Engine,
    radio: RadioId,
    s: &Serve,
    control: Option<ShareControl>,
    format: Option<ShareFormat>,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let id: ShareId = engine.open_share(radio)?;
    let share = OwnerRef::Share(id);
    let name = engine.key_of(share).unwrap_or_default();
    let mut run = || -> Result<(), CliError> {
        if let Some(c) = control {
            engine.set_text(share, "control", c.key())?;
        }
        if let Some(f) = format {
            engine.set_text(share, "format", f.key())?;
        }
        out.line(&format!("{name} serves {}", s.source))?;
        if s.rtl_tcp.is_none() && s.soapy.is_none() {
            listen(
                engine,
                share,
                ShareProtocol::RtlTcp,
                &RTL_TCP_PORT.to_string(),
                out,
            )?;
        }
        if let Some(at) = &s.rtl_tcp {
            listen(engine, share, ShareProtocol::RtlTcp, at, out)?;
        }
        if let Some(at) = &s.soapy {
            listen(engine, share, ShareProtocol::SoapyRemote, at, out)?;
        }
        let t0 = Instant::now();
        let mut shown = Instant::now();
        while !stop() && s.seconds.is_none_or(|n| t0.elapsed().as_secs_f64() < n) {
            std::thread::sleep(FOLLOW);
            reflect::print_events(engine, share, &name, out)?;
            if shown.elapsed() >= Duration::from_secs(1) {
                shown = Instant::now();
                out.status(&status(engine, share)?)?;
            }
        }
        out.done()?;
        reflect::print_events(engine, share, &name, out)?;
        out.line(&status(engine, share)?)
    };
    let r = run();
    let e = engine.close_share(id).map_err(CliError::from);
    r.and(e)
}

/// What `usb record` does.
#[derive(Clone, Debug)]
pub struct UsbRecord {
    /// The source: `usb:rtl:<selector>` or `usb:hackrf:<selector>`.
    pub source: String,
    /// The file that gets the script.
    pub out: std::path::PathBuf,
    /// The time that the radio streams.
    pub seconds: f64,
    /// The frequency, the rate, the gain and the settings of the radio.
    pub tune: Tune,
}

/// `usb record`: opens the USB radio at the source, lets it stream for the given time, closes it and reports what it
/// delivered. The caller made the engine on a bus that records, and writes the script afterwards.
///
/// # Errors
///
/// [`CliError::UnknownDriver`], the errors of the engine.
pub fn usb_record(
    engine: &Engine,
    r: &UsbRecord,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let source = Source::parse(&r.source, "");
    let radio = open_radio(engine, &source, &r.tune)?;
    let t0 = Instant::now();
    while !stop() && t0.elapsed().as_secs_f64() < r.seconds {
        std::thread::sleep(FOLLOW);
    }
    engine.settle(Duration::from_secs(2));
    let samples = engine.owner(OwnerRef::Radio(radio), |o| {
        let res = o.results();
        res.decl()
            .field_id("samples")
            .and_then(|id| res.fields().value(id).cloned())
    })?;
    let closed = engine.close_radio(radio);
    if let Some(Value::I64(n)) = samples {
        out.line(&format!("{n} samples from {}", r.source))?;
    }
    Ok(closed?)
}

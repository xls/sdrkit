//! The commands for USB radios and for serving: `devices` lists the attached USB radios, `serve` shares a source with
//! network clients over rtl_tcp and SoapyRemote, and `usb record` writes the USB exchanges of a radio as a script.

use crate::error::CliError;
use crate::host::{self, Live};
use crate::live::{self, Source, Tune};
use crate::out::Out;
use crate::reflect;
use crate::serve_screen::{self, Client, Note, Screen};
use crate::text::{self, Bare};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Choice, Owner, Value};
use sdrkit_host_engine::{
    Engine, ListenArgs, ListenReply, OwnerRef, RadioId, ShareControl, ShareFields, ShareFormat,
    ShareId, ShareProtocol, DEVICES, LISTEN,
};
use sdrkit_serve_core::{Answer, Request};
use std::io::IsTerminal;
use std::time::{Duration, Instant, SystemTime};

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
    /// Log lines and a status line, not the live screen.
    pub plain: bool,
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
) -> Result<(String, i64), CliError> {
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
    out.line(&format!("{} on {host}:{port}", protocol.label()))?;
    Ok((host, port))
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
        // What each client asked for and did not get, and what it loses.
        let notes = &o.results().status().summary;
        if !notes.is_empty() {
            s.push_str(&format!("; {notes}"));
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
    // Without a stack nobody here steers the radio: the first client does. A stack steers it, and the clients follow.
    let control: ShareControl = match s.control.as_deref() {
        Some(c) => choice(c, "--control")?,
        None if s.stack.is_some() => ShareControl::Follow,
        None => ShareControl::Remote,
    };
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
    control: ShareControl,
    format: Option<ShareFormat>,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let id: ShareId = engine.open_share(radio)?;
    let share = OwnerRef::Share(id);
    let name = engine.key_of(share).unwrap_or_default();
    let mut run = || -> Result<(), CliError> {
        engine.set_text(share, "control", control.key())?;
        if let Some(f) = format {
            engine.set_text(share, "format", f.key())?;
        }
        out.line(&format!(
            "{name} serves {} ({})",
            s.source,
            match control {
                ShareControl::Follow => "the clients follow the tuning of this program",
                ShareControl::Remote => "the first client steers the radio",
                ShareControl::Slice => "each client gets its own part of the capture",
            }
        ))?;
        let mut at = Vec::new();
        if s.rtl_tcp.is_none() && s.soapy.is_none() {
            let (h, p) = listen(
                engine,
                share,
                ShareProtocol::RtlTcp,
                &RTL_TCP_PORT.to_string(),
                out,
            )?;
            at.push((ShareProtocol::RtlTcp, h, p));
        }
        if let Some(a) = &s.rtl_tcp {
            let (h, p) = listen(engine, share, ShareProtocol::RtlTcp, a, out)?;
            at.push((ShareProtocol::RtlTcp, h, p));
        }
        if let Some(a) = &s.soapy {
            let (h, p) = listen(engine, share, ShareProtocol::SoapyRemote, a, out)?;
            at.push((ShareProtocol::SoapyRemote, h, p));
        }
        if !s.plain && std::io::stdout().is_terminal() {
            let addresses = connect_to(&at);
            return live_screen(engine, radio, id, s, &addresses, out, stop);
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

/// The addresses that clients connect to. For a listener on every interface: the address of the interface that
/// leads to other networks, and this computer's own.
fn connect_to(at: &[(ShareProtocol, String, i64)]) -> Vec<String> {
    // A datagram socket that is connected sends nothing but knows its own address on the way out.
    let lan = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|u| u.connect("192.0.2.1:9").map(|()| u))
        .and_then(|u| u.local_addr())
        .ok()
        .map(|a| a.ip())
        .filter(|ip| !ip.is_unspecified());
    let mut out = Vec::new();
    for (p, host, port) in at {
        let every = host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_unspecified());
        if every {
            if let Some(ip) = lan {
                out.push(format!("{ip}:{port} ({})", p.label()));
            }
            out.push(format!("127.0.0.1:{port} ({})", p.label()));
        } else {
            out.push(format!("{host}:{port} ({})", p.label()));
        }
    }
    out
}

/// What a client asked for, in words: "frequency 106.5 MHz".
fn asked(r: &Request) -> String {
    match *r {
        Request::Frequency { hz } => format!("frequency {}", text::hz(hz)),
        Request::Rate { hz } => format!("sample rate {} Msps", sdrkit_driver_core::fmt_m(hz)),
        Request::Bandwidth { hz } => format!("bandwidth {}", text::hz(hz)),
        Request::Gain { db } => format!("gain {db} dB"),
        Request::Agc { on } => format!("automatic gain {}", if on { "on" } else { "off" }),
        Request::BiasTee { on } => format!("bias tee {}", if on { "on" } else { "off" }),
        Request::Correction { ppm } => format!("frequency correction {ppm} ppm"),
        Request::Other { name, args } => format!("{name} {}", args[0]),
    }
}

/// A request and its answer in words: "frequency 106.5 MHz", "not carried out: bandwidth 5 MHz (follow mode)".
fn request_text(n: &sdrkit_host_engine::RequestNote) -> String {
    let mode = n.control.key();
    match n.answer {
        Answer::Accepted => asked(&n.request),
        Answer::Adjusted(got) if got == n.request => asked(&n.request),
        Answer::Adjusted(got) => format!(
            "not carried out: {} ({mode} mode), it gets {}",
            asked(&n.request),
            asked(&got)
        ),
        Answer::Refused => format!("not carried out: {} ({mode} mode)", asked(&n.request)),
    }
}

/// The settings of the radio that the screen shows: the gain section, then the bandwidth, the antenna and the bias
/// tee, as label and value with its unit. A stage at -1 follows the overall gain.
fn radio_settings(engine: &Engine, radio: RadioId) -> Vec<(String, String)> {
    use sdrkit_driver_core::form::{GAIN_SECTION, STAGE_FOLLOWS};
    engine
        .owner(OwnerRef::Radio(radio), |o: &mut dyn Owner| {
            let Some(v) = o.values() else {
                return Vec::new();
            };
            let mut out = Vec::new();
            let shown = ["bandwidth", "dc_removal", "antenna", "bias_tee", "ppm"];
            for gain in [true, false] {
                for (i, d) in v.schema().settings().iter().enumerate() {
                    let key = d.field.key.as_ref();
                    let pick = if gain {
                        d.section == GAIN_SECTION
                    } else {
                        shown.contains(&key) && d.section != GAIN_SECTION
                    };
                    let Some(val) = v.value(sdrkit_core::SettingId(i as u16)).filter(|_| pick)
                    else {
                        continue;
                    };
                    // The bandwidth of a radio is the width of its analogue filter.
                    let label = if key == "bandwidth" {
                        "filter".to_owned()
                    } else {
                        d.field.label.to_lowercase()
                    };
                    let text = match val {
                        Value::I64(n) if *n == STAGE_FOLLOWS && d.section == GAIN_SECTION => {
                            "follows the gain".to_owned()
                        }
                        Value::Bool(b) => (if *b { "on" } else { "off" }).to_owned(),
                        Value::Hz(x) | Value::F64(x) if key == "bandwidth" && *x <= 0.0 => {
                            "automatic".to_owned()
                        }
                        Value::Text(t) if t.is_empty() => "default".to_owned(),
                        other => text::cell(&d.field, other, 0.0),
                    };
                    out.push((label, text));
                }
            }
            out
        })
        .unwrap_or_default()
}

/// The counters of a client at the frame before, for the rates of the next frame.
#[derive(Clone, Copy)]
struct Seen {
    bytes: u64,
    samples: u64,
    dropped: u64,
    part: Option<f64>,
}

/// The screen from the share `id` of `radio` now; `seen` keeps the counters of each client between frames, `dt` is the
/// time since the frame before, s.
#[allow(clippy::too_many_arguments)]
fn screen(
    engine: &Engine,
    radio: RadioId,
    id: ShareId,
    addresses: &[String],
    uptime_s: f64,
    dt: f64,
    seen: &mut std::collections::HashMap<u64, Seen>,
    misses: &mut u64,
) -> Result<Screen, CliError> {
    let v = engine.share_view(id)?;
    let r = OwnerRef::Radio(radio);
    let num = |key: &str| -> f64 {
        engine
            .owner(r, |o: &mut dyn Owner| {
                let res = o.results();
                res.decl()
                    .field_id(key)
                    .and_then(|f| res.fields().value(f).cloned())
            })
            .ok()
            .flatten()
            .and_then(|x| match x {
                Value::Hz(x) | Value::F64(x) => Some(x),
                Value::I64(n) => Some(n as f64),
                _ => None,
            })
            .unwrap_or(0.0)
    };
    let capture = v.snapshot.capture;
    let control = match v.control {
        ShareControl::Follow => "the clients follow the tuning of this program".to_owned(),
        ShareControl::Slice => "each client gets its own part of the capture".to_owned(),
        ShareControl::Remote if v.host_reads => {
            "a stack of this program steers the radio, the clients follow it".to_owned()
        }
        ShareControl::Remote => match v.steering {
            Some(c) => format!("client {c} steers the radio"),
            None => "the first client steers the radio; none is connected".to_owned(),
        },
    };
    let now = SystemTime::now();
    let mut bytes = 0.0;
    let mut clients = Vec::new();
    let mut kept = std::collections::HashMap::new();
    for c in v.snapshot.clients.iter().filter(|c| !c.closed && c.engaged) {
        let before = seen.get(&c.id).copied();
        let d = |now: u64, then: Option<u64>| then.map_or(0.0, |t| now.saturating_sub(t) as f64);
        let sent = d(c.bytes, before.map(|b| b.bytes));
        bytes += sent;
        let gets_hz = c.slice.map_or(capture.rate_hz, |s| s.rate_hz);
        // Dropped samples count at the rate of the radio, sent ones at the rate of the client.
        let scale = if capture.rate_hz > 0.0 {
            gets_hz / capture.rate_hz
        } else {
            1.0
        };
        let out_n = d(c.samples, before.map(|b| b.samples));
        let lost_n = d(c.dropped_samples, before.map(|b| b.dropped)) * scale;
        let part = if before.is_some() && out_n + lost_n > 0.0 {
            Some(out_n / (out_n + lost_n))
        } else {
            before.and_then(|b| b.part)
        };
        kept.insert(
            c.id,
            Seen {
                bytes: c.bytes,
                samples: c.samples,
                dropped: c.dropped_samples,
                part,
            },
        );
        let protocol = match (c.protocol, c.path) {
            ("rtl_tcp", _) => "rtl_tcp".to_owned(),
            (_, "") => "SoapyRemote".to_owned(),
            (_, p) => format!("SoapyRemote {}", p.to_uppercase()),
        };
        clients.push(Client {
            id: c.id,
            address: c.peer.map(|a| a.to_string()).unwrap_or_default(),
            protocol,
            format: c
                .wire
                .map_or("-".to_owned(), |w| format!("{w:?}").to_lowercase()),
            asked_hz: c.asked_rate_hz,
            gets_hz,
            capture_hz: capture.rate_hz,
            mbit_s: if dt > 0.0 { sent * 8.0 / dt / 1e6 } else { 0.0 },
            part,
            connected_s: now.duration_since(c.since).map_or(0.0, |x| x.as_secs_f64()),
        });
    }
    *seen = kept;
    let notes = v
        .requests
        .iter()
        .take(serve_screen::NOTES)
        .map(|n| Note {
            age_s: now.duration_since(n.at).map_or(0.0, |x| x.as_secs_f64()),
            who: format!("client {}", n.client),
            text: request_text(n),
        })
        .collect();
    // The sender of this computer falls behind when a block of the radio finds no free buffer, or the ring between the
    // radio and the share fills.
    let missed = v.snapshot.pool_misses.saturating_sub(*misses);
    *misses = v.snapshot.pool_misses;
    let ring = num(sdrkit_driver_core::BUFFER_PCT.key);
    let behind = match (missed, ring >= 50.0) {
        (0, false) => None,
        (0, true) => Some(format!("the ring of the radio is {ring:.0} % full")),
        (m, false) => Some(format!("{m} blocks found no buffer")),
        (m, true) => Some(format!(
            "{m} blocks found no buffer, the ring of the radio is {ring:.0} % full"
        )),
    };
    Ok(Screen {
        behind,
        radio: serve_screen::Radio {
            name: v.radio_name.clone(),
            driver: v.driver.clone(),
            control,
            centre_hz: num(sdrkit_driver_core::CENTRE_GIVEN.key),
            rate_hz: num(sdrkit_driver_core::RATE_GIVEN.key),
            settings: radio_settings(engine, radio),
            level_dbfs: v.level_dbfs,
            peak_dbfs: v.peak_dbfs,
            lost: num(sdrkit_driver_core::RADIO_LOST.key) as i64,
        },
        bytes_per_s: if dt > 0.0 { bytes / dt } else { 0.0 },
        uptime_s,
        clients,
        notes,
        addresses: addresses.to_vec(),
    })
}

/// The terminal in raw mode on the other screen, until it drops: the terminal is restored also when a panic ends the
/// program.
struct Terminal;

impl Terminal {
    fn start() -> Result<Terminal, CliError> {
        use ratatui::crossterm::{cursor, execute, terminal};
        terminal::enable_raw_mode()?;
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            Terminal::restore();
            hook(info);
        }));
        execute!(
            std::io::stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide
        )?;
        Ok(Terminal)
    }

    fn restore() {
        use ratatui::crossterm::{cursor, execute, terminal};
        let _ = execute!(
            std::io::stdout(),
            cursor::Show,
            terminal::LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        Terminal::restore();
    }
}

/// How often the live screen is drawn.
const FRAME: Duration = Duration::from_millis(500);

/// The live screen of `serve`: drawn in place twice a second until q, Ctrl-C, `stop` or the end of the time. The last
/// screen is printed as text after the terminal is restored.
#[allow(clippy::too_many_arguments)]
fn live_screen(
    engine: &Engine,
    radio: RadioId,
    id: ShareId,
    s: &Serve,
    addresses: &[String],
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    use ratatui::crossterm::event::{self, Event as Key, KeyCode, KeyModifiers};
    use ratatui::crossterm::{cursor, queue, terminal};
    use std::io::Write;
    let t0 = Instant::now();
    let mut seen = std::collections::HashMap::new();
    let mut misses = 0u64;
    let mut last = Instant::now();
    let mut shown = Vec::new();
    let term = Terminal::start()?;
    let mut stdout = std::io::stdout();
    while !stop() && s.seconds.is_none_or(|n| t0.elapsed().as_secs_f64() < n) {
        let dt = last.elapsed().as_secs_f64();
        last = Instant::now();
        let sc = screen(
            engine,
            radio,
            id,
            addresses,
            t0.elapsed().as_secs_f64(),
            dt,
            &mut seen,
            &mut misses,
        )?;
        let (w, h) = terminal::size().unwrap_or((80, 24));
        shown = serve_screen::lines(&sc, usize::from(w).max(20));
        queue!(stdout, cursor::MoveTo(0, 0))?;
        for (i, l) in shown.iter().take(usize::from(h)).enumerate() {
            queue!(
                stdout,
                cursor::MoveTo(0, i as u16),
                terminal::Clear(terminal::ClearType::CurrentLine)
            )?;
            write!(stdout, "{l}")?;
        }
        queue!(stdout, terminal::Clear(terminal::ClearType::FromCursorDown))?;
        stdout.flush()?;
        let until = Instant::now() + FRAME;
        let mut quit = false;
        while Instant::now() < until && !stop() {
            if event::poll(Duration::from_millis(50))? {
                if let Key::Key(k) = event::read()? {
                    let ctrl_c =
                        k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL);
                    if k.code == KeyCode::Char('q') || ctrl_c {
                        quit = true;
                        break;
                    }
                    let mode = match k.code {
                        KeyCode::Char('f') => Some(ShareControl::Follow),
                        KeyCode::Char('r') => Some(ShareControl::Remote),
                        KeyCode::Char('s') => Some(ShareControl::Slice),
                        _ => None,
                    };
                    if let Some(m) = mode {
                        engine.set_text(OwnerRef::Share(id), "control", m.key())?;
                        break;
                    }
                }
            }
        }
        if quit {
            break;
        }
    }
    drop(term);
    out.line(&shown.join("\n"))
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

//! The text form of a [`Script`], as the crate documentation describes it.

use crate::script::{BulkItem, BulkKind, Entry, Mode, Payload, Script, Step};
use crate::{Setup, Speed, Unavailable, UsbDeviceInfo, UsbError, UsbId};
use std::fmt::Write as _;
use std::time::Duration;

/// The first line of a script file.
const HEADER: &str = "sdrkit-usb 1";

/// A line of a script file that cannot be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {text}")]
pub struct ParseError {
    /// The line number, from 1; 0 for the file as a whole.
    pub line: usize,
    /// What is wrong.
    pub text: String,
}

impl Script {
    /// Reads a script from its text form.
    ///
    /// # Errors
    ///
    /// The first line that is not in the form.
    pub fn parse(text: &str) -> Result<Script, ParseError> {
        let mut script = Script::new();
        let mut header = false;
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fail = |text: String| ParseError { line: n + 1, text };
            if !header {
                if line != HEADER {
                    return Err(fail(format!("the first line must be `{HEADER}`")));
                }
                header = true;
                continue;
            }
            let tokens = tokens(line).map_err(fail)?;
            parse_line(&mut script, &tokens).map_err(fail)?;
        }
        if !header {
            return Err(ParseError {
                line: 0,
                text: format!("no `{HEADER}` line"),
            });
        }
        Ok(script)
    }

    /// The text form of the script.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str(HEADER);
        s.push('\n');
        s.push_str(match self.mode {
            Mode::Strict => "mode strict\n",
            Mode::Lenient => "mode lenient\n",
        });
        for d in &self.devices {
            s.push_str("device ");
            s.push_str(&device_fields(d));
            s.push('\n');
        }
        for e in &self.entries {
            match e {
                Entry::Step(step) => s.push_str(&step_text(step)),
                Entry::Bulk(item) => s.push_str(&bulk_text(item)),
            }
            s.push('\n');
        }
        s
    }
}

/// The line of a step.
pub(crate) fn step_text(step: &Step) -> String {
    let mut s = call_text(step);
    match step {
        Step::Open { reply, .. }
        | Step::Claim { reply, .. }
        | Step::ControlOut { reply, .. }
        | Step::ClearHalt { reply, .. }
        | Step::Reset { reply, .. } => {
            if let Err(e) = reply {
                let _ = write!(s, " -> {}", err_text(e));
            }
        }
        Step::ControlIn { reply, .. } => match reply {
            Ok(d) => {
                let _ = write!(s, " -> {}", hex(d));
            }
            Err(e) => {
                let _ = write!(s, " -> {}", err_text(e));
            }
        },
        Step::BulkOut { reply, data, .. } => match reply {
            Ok(n) if *n == data.len() => {}
            Ok(n) => {
                let _ = write!(s, " -> {n}");
            }
            Err(e) => {
                let _ = write!(s, " -> {}", err_text(e));
            }
        },
        Step::Unplug { .. } | Step::Plug { .. } => {}
    }
    s
}

/// The line of a step without its reply: how a call is named in a report.
pub(crate) fn call_text(step: &Step) -> String {
    match step {
        Step::Open { device, .. } => format!("open {}", device.0),
        Step::Claim {
            device, interface, ..
        } => format!("claim {} {interface}", device.0),
        Step::ControlIn {
            device,
            setup,
            length,
            ..
        } => format!("in {} {} {length}", device.0, setup_text(setup)),
        Step::ControlOut {
            device,
            setup,
            data,
            ..
        } => format!("out {} {} {}", device.0, setup_text(setup), hex(data)),
        Step::BulkOut {
            device,
            endpoint,
            data,
            ..
        } => format!("bout {} {endpoint:02x} {}", device.0, payload_text(data)),
        Step::ClearHalt {
            device, endpoint, ..
        } => format!("halt {} {endpoint:02x}", device.0),
        Step::Reset { device, .. } => format!("reset {}", device.0),
        Step::Unplug { device } => format!("unplug {}", device.0),
        Step::Plug { info } => format!("plug {}", device_fields(info)),
    }
}

fn bulk_text(item: &BulkItem) -> String {
    let mut s = format!("bulk {} {:02x}", item.device.0, item.endpoint);
    if !item.after.is_zero() {
        let _ = write!(s, " after={}us", item.after.as_micros());
    }
    if item.repeat != 1 {
        let _ = write!(s, " rep={}", item.repeat);
    }
    s.push(' ');
    match &item.kind {
        BulkKind::Data(p) => s.push_str(&payload_text(p)),
        BulkKind::Error(e) => s.push_str(&err_text(e)),
        BulkKind::Unplug => s.push_str("!unplug"),
    }
    s
}

fn setup_text(s: &Setup) -> String {
    format!(
        "{:02x} {:02x} {:04x} {:04x}",
        s.request_type, s.request, s.value, s.index
    )
}

fn device_fields(d: &UsbDeviceInfo) -> String {
    let ports: Vec<String> = d.ports.iter().map(u8::to_string).collect();
    let ports = if ports.is_empty() {
        "-".to_owned()
    } else {
        ports.join(".")
    };
    let mut s = format!(
        "{} {:04x}:{:04x} bus={} ports={ports}",
        d.id.0,
        d.vendor_id,
        d.product_id,
        quote(&d.bus)
    );
    for (k, v) in [
        ("manufacturer", &d.manufacturer),
        ("product", &d.product),
        ("serial", &d.serial),
    ] {
        if let Some(v) = v {
            let _ = write!(s, " {k}={}", quote(v));
        }
    }
    if let Some(b) = d.blocked {
        let _ = write!(s, " blocked={}", b.key());
    }
    if d.release != 0 {
        let _ = write!(s, " release={:04x}", d.release);
    }
    if d.speed != Speed::Unknown {
        let _ = write!(s, " speed={}", d.speed.key());
    }
    s
}

fn hex(d: &[u8]) -> String {
    if d.is_empty() {
        return "-".to_owned();
    }
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn payload_text(p: &Payload) -> String {
    match p {
        Payload::Full(d) => hex(d),
        Payload::Cut { len, crc, head } => format!("cut:{len}:{crc:08x}:{}", hex(head)),
    }
}

fn err_text(e: &UsbError) -> String {
    match e {
        UsbError::Other(t) | UsbError::Script(t) if !t.is_empty() => {
            format!("!{}={}", e.key(), quote(t))
        }
        _ => format!("!{}", e.key()),
    }
}

fn quote(t: &str) -> String {
    let mut s = String::with_capacity(t.len() + 2);
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Splits a line at white space outside double quotes. Quotes and escapes stay in the tokens.
fn tokens(line: &str) -> Result<Vec<&str>, String> {
    let mut out = Vec::new();
    let mut start = None;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                out.push(&line[s..i]);
            }
        } else {
            if start.is_none() {
                start = Some(i);
            }
            if c == '"' {
                quoted = true;
            }
        }
    }
    if quoted {
        return Err("a quote is not closed".into());
    }
    if let Some(s) = start {
        out.push(&line[s..]);
    }
    Ok(out)
}

fn unquote(t: &str) -> Result<String, String> {
    let inner = t
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .ok_or_else(|| format!("`{t}` is not a quoted text"))?;
    let mut s = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => s.push('\n'),
                Some(c) => s.push(c),
                None => return Err("a text ends in `\\`".into()),
            }
        } else {
            s.push(c);
        }
    }
    Ok(s)
}

fn parse_hex(t: &str) -> Result<Vec<u8>, String> {
    if t == "-" {
        return Ok(Vec::new());
    }
    if !t.len().is_multiple_of(2) || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("`{t}` is not hexadecimal bytes"));
    }
    Ok((0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).unwrap_or(0))
        .collect())
}

fn parse_payload(t: &str) -> Result<Payload, String> {
    let Some(rest) = t.strip_prefix("cut:") else {
        return parse_hex(t).map(Payload::Full);
    };
    let mut parts = rest.splitn(3, ':');
    let (Some(len), Some(crc), Some(head)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(format!("`{t}` is not `cut:<len>:<crc>:<head>`"));
    };
    let len = len
        .parse()
        .map_err(|_| format!("`{len}` is not a length"))?;
    let crc = u32::from_str_radix(crc, 16).map_err(|_| format!("`{crc}` is not a CRC"))?;
    let head = parse_hex(head)?;
    if head.len() > len {
        return Err("the head is longer than the payload".into());
    }
    Ok(Payload::Cut { len, crc, head })
}

fn parse_err(t: &str) -> Result<UsbError, String> {
    let body = t
        .strip_prefix('!')
        .ok_or_else(|| format!("`{t}` is not an error"))?;
    let (key, text) = match body.split_once('=') {
        Some((k, v)) => (k, unquote(v)?),
        None => (body, String::new()),
    };
    UsbError::from_key(key, &text).ok_or_else(|| format!("`{key}` is not an error key"))
}

fn hex8(t: &str) -> Result<u8, String> {
    u8::from_str_radix(t, 16).map_err(|_| format!("`{t}` is not a hexadecimal byte"))
}

fn hex16(t: &str) -> Result<u16, String> {
    u16::from_str_radix(t, 16).map_err(|_| format!("`{t}` is not a hexadecimal word"))
}

fn dec<T: std::str::FromStr>(t: &str) -> Result<T, String> {
    t.parse().map_err(|_| format!("`{t}` is not a number"))
}

fn device_id(t: &str) -> Result<UsbId, String> {
    dec(t).map(UsbId)
}

fn parse_device(f: &[&str]) -> Result<UsbDeviceInfo, String> {
    let [id, vp, rest @ ..] = f else {
        return Err("a device needs an id and `<vid>:<pid>`".into());
    };
    let (v, p) = vp
        .split_once(':')
        .ok_or_else(|| format!("`{vp}` is not `<vid>:<pid>`"))?;
    let mut d = UsbDeviceInfo::new(device_id(id)?, hex16(v)?, hex16(p)?);
    for kv in rest {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| format!("`{kv}` is not `<key>=<value>`"))?;
        match k {
            "bus" => d.bus = unquote(v)?,
            "ports" if v == "-" => d.ports.clear(),
            "ports" => {
                d.ports = v.split('.').map(dec::<u8>).collect::<Result<_, _>>()?;
            }
            "manufacturer" => d.manufacturer = Some(unquote(v)?),
            "product" => d.product = Some(unquote(v)?),
            "serial" => d.serial = Some(unquote(v)?),
            "blocked" => {
                d.blocked =
                    Some(Unavailable::from_key(v).ok_or_else(|| format!("`{v}` is not a reason"))?);
            }
            "release" => d.release = hex16(v)?,
            "speed" => {
                d.speed = Speed::from_key(v).ok_or_else(|| format!("`{v}` is not a speed"))?;
            }
            _ => return Err(format!("`{k}` is not a field of a device")),
        }
    }
    Ok(d)
}

/// Splits `call -> reply` into the call tokens and the reply token.
fn split_reply<'a, 'b>(f: &'b [&'a str]) -> Result<(&'b [&'a str], Option<&'a str>), String> {
    match f.iter().position(|t| *t == "->") {
        None => Ok((f, None)),
        Some(i) if i + 2 == f.len() => Ok((&f[..i], Some(f[i + 1]))),
        Some(_) => Err("`->` must be followed by one reply".into()),
    }
}

fn unit_reply(r: Option<&str>) -> Result<Result<(), UsbError>, String> {
    match r {
        None => Ok(Ok(())),
        Some(t) => parse_err(t).map(Err),
    }
}

fn parse_setup(f: &[&str]) -> Result<Setup, String> {
    Ok(Setup {
        request_type: hex8(f[0])?,
        request: hex8(f[1])?,
        value: hex16(f[2])?,
        index: hex16(f[3])?,
    })
}

fn parse_line(script: &mut Script, t: &[&str]) -> Result<(), String> {
    let (&kind, f) = t.split_first().ok_or("empty line")?;
    let (call, reply) = split_reply(f)?;
    let need = |n: usize| {
        if call.len() == n {
            Ok(())
        } else {
            Err(format!("`{kind}` takes {n} fields"))
        }
    };
    let step = match kind {
        "mode" => {
            need(1)?;
            script.mode = match call[0] {
                "strict" => Mode::Strict,
                "lenient" => Mode::Lenient,
                m => return Err(format!("`{m}` is not a mode")),
            };
            return Ok(());
        }
        "device" => {
            script.devices.push(parse_device(call)?);
            return Ok(());
        }
        "bulk" => {
            script.entries.push(Entry::Bulk(parse_bulk(call)?));
            return Ok(());
        }
        "open" => {
            need(1)?;
            Step::Open {
                device: device_id(call[0])?,
                reply: unit_reply(reply)?,
            }
        }
        "claim" => {
            need(2)?;
            Step::Claim {
                device: device_id(call[0])?,
                interface: dec(call[1])?,
                reply: unit_reply(reply)?,
            }
        }
        "in" => {
            need(6)?;
            let r = reply.ok_or("`in` needs a reply after `->`")?;
            Step::ControlIn {
                device: device_id(call[0])?,
                setup: parse_setup(&call[1..5])?,
                length: dec(call[5])?,
                reply: if r.starts_with('!') {
                    Err(parse_err(r)?)
                } else {
                    Ok(parse_hex(r)?)
                },
            }
        }
        "out" => {
            need(6)?;
            Step::ControlOut {
                device: device_id(call[0])?,
                setup: parse_setup(&call[1..5])?,
                data: parse_hex(call[5])?,
                reply: unit_reply(reply)?,
            }
        }
        "bout" => {
            need(3)?;
            let data = parse_payload(call[2])?;
            let reply = match reply {
                None => Ok(data.len()),
                Some(r) if r.starts_with('!') => Err(parse_err(r)?),
                Some(r) => Ok(dec(r)?),
            };
            Step::BulkOut {
                device: device_id(call[0])?,
                endpoint: hex8(call[1])?,
                data,
                reply,
            }
        }
        "halt" => {
            need(2)?;
            Step::ClearHalt {
                device: device_id(call[0])?,
                endpoint: hex8(call[1])?,
                reply: unit_reply(reply)?,
            }
        }
        "reset" => {
            need(1)?;
            Step::Reset {
                device: device_id(call[0])?,
                reply: unit_reply(reply)?,
            }
        }
        "unplug" => {
            need(1)?;
            Step::Unplug {
                device: device_id(call[0])?,
            }
        }
        "plug" => Step::Plug {
            info: parse_device(call)?,
        },
        _ => return Err(format!("`{kind}` is not a kind of line")),
    };
    script.entries.push(Entry::Step(step));
    Ok(())
}

fn parse_bulk(f: &[&str]) -> Result<BulkItem, String> {
    let [dev, ep, opts @ .., last] = f else {
        return Err("`bulk` takes a device, an endpoint and the data".into());
    };
    let mut item = BulkItem {
        device: device_id(dev)?,
        endpoint: hex8(ep)?,
        after: Duration::ZERO,
        repeat: 1,
        kind: BulkKind::Unplug,
    };
    for o in opts {
        if let Some(v) = o.strip_prefix("after=") {
            item.after = if let Some(us) = v.strip_suffix("us") {
                Duration::from_micros(dec(us)?)
            } else if let Some(ms) = v.strip_suffix("ms") {
                Duration::from_millis(dec(ms)?)
            } else {
                return Err(format!("`{v}` is not a time in `us` or `ms`"));
            };
        } else if let Some(v) = o.strip_prefix("rep=") {
            item.repeat = dec(v)?;
            if item.repeat == 0 {
                return Err("`rep` must be at least 1".into());
            }
        } else {
            return Err(format!("`{o}` is not an option of `bulk`"));
        }
    }
    item.kind = match *last {
        "!unplug" => BulkKind::Unplug,
        t if t.starts_with('!') => BulkKind::Error(parse_err(t)?),
        t => BulkKind::Data(parse_payload(t)?),
    };
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Script {
        let mut d = UsbDeviceInfo::new(UsbId(1), 0x0bda, 0x2838);
        d.bus = "1".into();
        d.ports = vec![2, 1];
        d.manufacturer = Some("Maker \"A\"".into());
        d.serial = Some("00\\01".into());
        let mut plugged = d.clone();
        plugged.id = UsbId(2);
        plugged.blocked = Some(Unavailable::NeedsWinUsb);
        plugged.ports.clear();
        plugged.release = 0x0102;
        plugged.speed = Speed::SuperPlus;
        let mut s = Script::new();
        s.mode = Mode::Lenient;
        s.devices.push(d);
        let dev = UsbId(1);
        s.step(Step::Open {
            device: dev,
            reply: Ok(()),
        })
        .step(Step::Claim {
            device: dev,
            interface: 0,
            reply: Err(UsbError::KernelDriver),
        })
        .step(Step::ControlIn {
            device: dev,
            setup: Setup::vendor(0, 0x20, 0x10).input(),
            length: 2,
            reply: Ok(vec![0x0a, 0x0b]),
        })
        .step(Step::ControlIn {
            device: dev,
            setup: Setup::vendor(0, 0x20, 0x10).input(),
            length: 2,
            reply: Err(UsbError::Other("odd \"thing\"".into())),
        })
        .step(Step::ControlOut {
            device: dev,
            setup: Setup::vendor(1, 0x21, 0x11),
            data: vec![],
            reply: Err(UsbError::Stall),
        })
        .step(Step::BulkOut {
            device: dev,
            endpoint: 0x02,
            data: Payload::of(&[1, 2, 3, 4, 5], 2),
            reply: Ok(3),
        })
        .step(Step::ClearHalt {
            device: dev,
            endpoint: 0x81,
            reply: Ok(()),
        })
        .step(Step::Reset {
            device: dev,
            reply: Err(UsbError::Other(String::new())),
        })
        .bulk(BulkItem {
            device: dev,
            endpoint: 0x81,
            after: Duration::from_micros(1500),
            repeat: 3,
            kind: BulkKind::Data(Payload::Full(vec![0xff, 0])),
        })
        .bulk(BulkItem {
            device: dev,
            endpoint: 0x81,
            after: Duration::ZERO,
            repeat: 1,
            kind: BulkKind::Error(UsbError::Stall),
        })
        .bulk(BulkItem {
            device: dev,
            endpoint: 0x81,
            after: Duration::ZERO,
            repeat: 1,
            kind: BulkKind::Unplug,
        })
        .step(Step::Unplug { device: dev })
        .step(Step::Plug { info: plugged });
        s
    }

    #[test]
    fn text_round_trips() {
        let s = sample();
        let text = s.to_text();
        let back = Script::parse(&text).unwrap();
        assert_eq!(back, s, "{text}");
        assert_eq!(back.to_text(), text);
    }

    #[test]
    fn bad_lines_name_their_line() {
        let e = Script::parse("sdrkit-usb 1\n# note\nopen x\n").unwrap_err();
        assert_eq!(e.line, 3);
        assert!(Script::parse("open 1\n").is_err());
        let e = Script::parse("sdrkit-usb 1\nin 1 c0 00 0000 0000 1\n").unwrap_err();
        assert!(e.text.contains("reply"), "{e}");
        let e = Script::parse("sdrkit-usb 1\ndevice 1 0bda:2838 bus=\"x\n").unwrap_err();
        assert!(e.text.contains("quote"), "{e}");
    }
}

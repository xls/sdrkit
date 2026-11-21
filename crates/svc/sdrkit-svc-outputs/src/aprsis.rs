//! Receive-only APRS-IS gating: APRS packets heard on the air go to an APRS-IS server under the user's call sign. It
//! never transmits and never sends anything from the network to the radio.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::runner::{Item, Sink};
use crate::OutputError;
use sdrkit_core::net::{Conn, Transport};

/// APRS-IS settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AprsIsSettings {
    /// Server host.
    pub host: String,
    /// Server port (14580 for a filtered feed).
    pub port: u16,
    /// The user's call sign with SSID (`VK2ABC-10`).
    pub callsign: String,
    /// The passcode; `None` computes it from the call sign.
    pub passcode: Option<u16>,
    /// Seconds within which a repeat of the same packet is not gated again.
    pub dupe_s: f64,
    /// Connection and read timeout, seconds.
    pub timeout_s: f64,
}

impl Default for AprsIsSettings {
    fn default() -> Self {
        AprsIsSettings {
            host: "rotate.aprs2.net".into(),
            port: 14580,
            callsign: String::new(),
            passcode: None,
            dupe_s: 30.0,
            timeout_s: 10.0,
        }
    }
}

/// The APRS-IS passcode of a call sign (the SSID is ignored).
pub fn passcode(callsign: &str) -> u16 {
    let base = callsign
        .split('-')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    let b = base.as_bytes();
    let mut hash: u16 = 0x73E2;
    let mut i = 0;
    while i < b.len() {
        hash ^= (b[i] as u16) << 8;
        if i + 1 < b.len() {
            hash ^= b[i + 1] as u16;
        }
        i += 2;
    }
    hash & 0x7FFF
}

const NO_GATE: [&str; 4] = ["TCPIP", "TCPXX", "NOGATE", "RFONLY"];

fn path_refuses(header: &str) -> bool {
    header
        .split(['>', ','])
        .any(|p| NO_GATE.contains(&p.trim_end_matches('*')))
}

/// The line to gate for a TNC2 packet heard on the air, or `None` when it must not be gated.
pub fn gate_line(tnc2: &str, my_call: &str) -> Option<String> {
    let (header, info) = tnc2.trim_end_matches(['\r', '\n']).split_once(':')?;
    if !header.contains('>')
        || path_refuses(header)
        || info.starts_with('?')
        || header.contains(",q")
    {
        return None;
    }
    if let Some(inner) = info.strip_prefix('}') {
        let (ih, _) = inner.split_once(':')?;
        if path_refuses(ih) {
            return None;
        }
    }
    Some(format!("{header},qAR,{my_call}:{info}"))
}

/// The APRS-IS gate.
pub struct AprsIsSink {
    s: AprsIsSettings,
    transport: Arc<dyn Transport>,
    conn: Option<BufReader<Box<dyn Conn>>>,
    recent: HashMap<String, Instant>,
}

impl AprsIsSink {
    /// A gate. The call sign must be the user's own (an empty call or `N0CALL` is refused).
    pub fn new(mut s: AprsIsSettings, transport: Arc<dyn Transport>) -> Result<Self, OutputError> {
        s.callsign = s.callsign.trim().to_ascii_uppercase();
        let base = s.callsign.split('-').next().unwrap_or("");
        if base.len() < 3
            || base == "N0CALL"
            || base == "NOCALL"
            || !s
                .callsign
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(OutputError::Setting(crate::BadSetting::Callsign));
        }
        Ok(AprsIsSink {
            s,
            transport,
            conn: None,
            recent: HashMap::new(),
        })
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs_f64(self.s.timeout_s.clamp(0.1, 120.0))
    }

    fn ensure(&mut self) -> Result<(), OutputError> {
        if self.conn.is_some() {
            return Ok(());
        }
        let mut c = self
            .transport
            .connect(&self.s.host, self.s.port, false, self.timeout())?;
        c.set_read_timeout(Some(self.timeout()))?;
        let mut r = BufReader::new(c);
        let mut banner = String::new();
        r.read_line(&mut banner)?;
        let pass = self
            .s
            .passcode
            .unwrap_or_else(|| passcode(&self.s.callsign));
        let login = format!(
            "user {} pass {} vers sdrkit {}\r\n",
            self.s.callsign,
            pass,
            env!("CARGO_PKG_VERSION")
        );
        r.get_mut().write_all(login.as_bytes())?;
        r.get_mut().flush()?;
        let mut resp = String::new();
        r.read_line(&mut resp)?;
        if !resp.starts_with("# logresp") || resp.contains("unverified") {
            return Err(OutputError::Protocol(crate::Protocol::LoginRefused));
        }
        self.conn = Some(r);
        Ok(())
    }
}

impl Sink<Item> for AprsIsSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        let Ok(tnc2) = std::str::from_utf8(item.body()) else {
            return Ok(());
        };
        let Some(line) = gate_line(tnc2.trim(), &self.s.callsign) else {
            return Ok(());
        };
        let key = line
            .split_once(",qAR,")
            .map(|(h, _)| h.to_string())
            .unwrap_or_default()
            + line.split_once(':').map_or("", |x| x.1);
        let now = Instant::now();
        let dupe = Duration::from_secs_f64(self.s.dupe_s.max(0.0));
        if self
            .recent
            .get(&key)
            .is_some_and(|t| now.duration_since(*t) < dupe)
        {
            return Ok(());
        }
        self.ensure()?;
        let c = self
            .conn
            .as_mut()
            .ok_or(OutputError::Protocol(crate::Protocol::NotConnected))?
            .get_mut();
        c.write_all(line.as_bytes())?;
        c.write_all(b"\r\n")?;
        c.flush()?;
        self.recent.insert(key, now);
        if self.recent.len() > 4096 {
            self.recent.retain(|_, t| now.duration_since(*t) < dupe);
        }
        Ok(())
    }

    fn idle(&mut self) -> Result<(), OutputError> {
        // Read what the server sent (keep-alive comments), so its buffer never fills; a closed connection is reset.
        let t = self.timeout();
        if let Some(r) = self.conn.as_mut() {
            r.get_mut()
                .set_read_timeout(Some(Duration::from_millis(1)))?;
            let mut buf = [0u8; 2048];
            let res = r.read(&mut buf);
            r.get_mut().set_read_timeout(Some(t))?;
            match res {
                Ok(0) => return Err(OutputError::Protocol(crate::Protocol::NoLogin)),
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.conn = None;
    }

    fn connected(&self) -> bool {
        self.conn.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passcodes() {
        // The hash is case- and SSID-blind.
        assert_eq!(passcode("N0CALL"), passcode("n0call-9"));
        assert!(passcode("VK2ABC") <= 0x7FFF);
        assert_eq!(passcode(""), 0x73E2 & 0x7FFF);
    }

    #[test]
    fn gating_rules() {
        assert_eq!(
            gate_line("VK2ABC-9>APRS,WIDE1-1*:!hello", "VK2XYZ-10").unwrap(),
            "VK2ABC-9>APRS,WIDE1-1*,qAR,VK2XYZ-10:!hello"
        );
        assert!(gate_line("VK2ABC>APRS,TCPIP*:x", "C").is_none());
        assert!(gate_line("VK2ABC>APRS,NOGATE:x", "C").is_none());
        assert!(gate_line("VK2ABC>APRS,RFONLY:x", "C").is_none());
        assert!(gate_line("VK2ABC>APRS:?APRS?", "C").is_none());
        assert!(gate_line("VK2ABC>APRS:}VK2Q>APRS,TCPIP,VK2ABC*:inner", "C").is_none());
        assert!(gate_line("VK2ABC>APRS:}VK2Q>APRS,WIDE1-1:inner", "C").is_some());
        assert!(gate_line("VK2ABC>APRS,qAR,X:x", "C").is_none());
        assert!(gate_line("garbage", "C").is_none());
    }

    #[test]
    fn a_call_sign_of_ones_own_is_needed() {
        let t: Arc<dyn Transport> = Arc::new(sdrkit_core::net::StdNet);
        assert!(AprsIsSink::new(AprsIsSettings::default(), t.clone()).is_err());
        assert!(AprsIsSink::new(
            AprsIsSettings {
                callsign: "N0CALL".into(),
                ..AprsIsSettings::default()
            },
            t.clone()
        )
        .is_err());
        assert!(AprsIsSink::new(
            AprsIsSettings {
                callsign: "vk2abc-10".into(),
                ..AprsIsSettings::default()
            },
            t
        )
        .is_ok());
    }
}

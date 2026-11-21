//! MQTT 3.1.1 output: CONNECT with a last will, PUBLISH at QoS 0 or 1, keep-alive, and home-automation discovery.
//! Standard: OASIS MQTT Version 3.1.1.

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sdrkit_core::publish::keys::Family;
use sdrkit_core::Clock;
use sdrkit_svc_events::choice_key;
use serde::{Deserialize, Serialize};

use crate::net::Url;
use crate::runner::{Item, Sink};
use crate::OutputError;
use sdrkit_core::net::{Conn, Transport};

/// MQTT settings.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MqttSettings {
    /// `mqtt://host:port` or `mqtts://host:port` (TLS, through the host's transport; never downgraded).
    pub url: String,
    /// Client id (empty: `sdrkit-` and a number from the clock).
    pub client_id: String,
    /// User name (empty: none).
    pub username: String,
    /// Password (empty: none). Never logged or shown in errors.
    pub password: String,
    /// Keep-alive, seconds.
    pub keepalive_s: u16,
    /// 0 or 1.
    pub qos: u8,
    /// Topic template for every observation.
    pub topic: String,
    /// Retain every observation (rarely useful).
    pub retain: bool,
    /// The base topic for status and the sensors' state.
    pub base: String,
    /// Publish discovery configs for home-automation systems.
    pub discovery: bool,
    /// The discovery prefix.
    pub discovery_prefix: String,
    /// The node id used in discovery topics and unique ids.
    pub node_id: String,
    /// Seconds to wait for CONNACK and PUBACK.
    pub timeout_s: f64,
}

impl std::fmt::Debug for MqttSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MqttSettings")
            .field("url", &self.url)
            .field("client_id", &self.client_id)
            .field("username", &self.username)
            .field(
                "password",
                &if self.password.is_empty() { "" } else { "***" },
            )
            .field("qos", &self.qos)
            .field("topic", &self.topic)
            .finish_non_exhaustive()
    }
}

impl Default for MqttSettings {
    fn default() -> Self {
        MqttSettings {
            url: "mqtt://127.0.0.1:1883".into(),
            client_id: String::new(),
            username: String::new(),
            password: String::new(),
            keepalive_s: 60,
            qos: 0,
            topic: "sdrkit/{family}/{type}/{identity}".into(),
            retain: false,
            base: "sdrkit".into(),
            discovery: false,
            discovery_prefix: "homeassistant".into(),
            node_id: "sdrkit".into(),
            timeout_s: 10.0,
        }
    }
}

/// The sensors published for discovery: (object id, name, family).
pub const SENSORS: [(&str, &str, Family); 5] = [
    ("last_aircraft", "Last aircraft", Family::Aircraft),
    ("last_vessel", "Last vessel", Family::Vessel),
    ("last_aprs", "Last APRS station", Family::Aprs),
    ("last_pager", "Last pager message", Family::Pager),
    ("last_sonde", "Last radiosonde", Family::Sonde),
];

/// The discovery record of one sensor.
#[derive(Serialize)]
struct Discovery<'a> {
    name: &'a str,
    unique_id: String,
    object_id: String,
    state_topic: String,
    value_template: &'a str,
    json_attributes_topic: String,
    availability_topic: &'a str,
    payload_available: &'a str,
    payload_not_available: &'a str,
    icon: &'a str,
    device: Device<'a>,
}

/// The device of the discovery records.
#[derive(Serialize)]
struct Device<'a> {
    identifiers: [&'a str; 1],
    name: &'a str,
    manufacturer: &'a str,
    model: &'a str,
}

fn put_str(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len().min(65_535) as u16).to_be_bytes());
    out.extend_from_slice(&s[..s.len().min(65_535)]);
}

fn put_len(out: &mut Vec<u8>, mut n: usize) {
    loop {
        let mut b = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            b |= 0x80;
        }
        out.push(b);
        if n == 0 {
            break;
        }
    }
}

fn packet(first: u8, body: &[u8]) -> Vec<u8> {
    let mut p = vec![first];
    put_len(&mut p, body.len());
    p.extend_from_slice(body);
    p
}

/// A CONNECT packet.
#[allow(clippy::too_many_arguments)]
pub fn connect_packet(
    client_id: &str,
    user: &str,
    pass: &str,
    keepalive: u16,
    will: Option<(&str, &[u8], bool)>,
) -> Vec<u8> {
    let mut b = Vec::new();
    put_str(&mut b, b"MQTT");
    b.push(4);
    let mut flags = 0x02; // clean session
    if !user.is_empty() {
        flags |= 0x80;
        if !pass.is_empty() {
            flags |= 0x40;
        }
    }
    if let Some((_, _, retain)) = will {
        flags |= 0x04 | if retain { 0x20 } else { 0 };
    }
    b.push(flags);
    b.extend_from_slice(&keepalive.to_be_bytes());
    put_str(&mut b, client_id.as_bytes());
    if let Some((t, m, _)) = will {
        put_str(&mut b, t.as_bytes());
        put_str(&mut b, m);
    }
    if !user.is_empty() {
        put_str(&mut b, user.as_bytes());
        if !pass.is_empty() {
            put_str(&mut b, pass.as_bytes());
        }
    }
    packet(0x10, &b)
}

/// A PUBLISH packet.
pub fn publish_packet(topic: &str, payload: &[u8], qos: u8, retain: bool, id: u16) -> Vec<u8> {
    let mut b = Vec::with_capacity(topic.len() + payload.len() + 4);
    put_str(&mut b, topic.as_bytes());
    if qos > 0 {
        b.extend_from_slice(&id.to_be_bytes());
    }
    b.extend_from_slice(payload);
    packet(0x30 | (qos.min(1) << 1) | u8::from(retain), &b)
}

/// Reads one packet: (first byte, body).
pub fn read_packet(r: &mut dyn Read) -> Result<(u8, Vec<u8>), OutputError> {
    let mut first = [0u8; 1];
    r.read_exact(&mut first)?;
    let mut len = 0usize;
    for i in 0..4 {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        len |= ((b[0] & 0x7F) as usize) << (7 * i);
        if b[0] & 0x80 == 0 {
            let mut body = vec![0u8; len];
            r.read_exact(&mut body)?;
            return Ok((first[0], body));
        }
    }
    Err(OutputError::Protocol(crate::Protocol::LengthTooLong))
}

/// The MQTT sink.
pub struct MqttSink {
    s: MqttSettings,
    url: Url,
    transport: Arc<dyn Transport>,
    conn: Option<Box<dyn Conn>>,
    next_id: u16,
    last_send: Instant,
}

impl MqttSink {
    /// A sink; nothing connects until the first observation (or keep-alive).
    pub fn new(
        mut s: MqttSettings,
        transport: Arc<dyn Transport>,
        clock: &Clock,
    ) -> Result<Self, OutputError> {
        let url = Url::parse(&s.url)?;
        if !matches!(
            url.scheme.as_str(),
            "mqtt" | "mqtts" | "tcp" | "ssl" | "tls"
        ) {
            return Err(OutputError::Setting(crate::BadSetting::Scheme));
        }
        if s.qos > 1 {
            return Err(OutputError::Setting(crate::BadSetting::Qos));
        }
        if s.client_id.is_empty() {
            s.client_id = format!("sdrkit-{}", (clock.now() * 1000.0) as u64 % 1_000_000);
        }
        Ok(MqttSink {
            s,
            url,
            transport,
            conn: None,
            next_id: 1,
            last_send: Instant::now(),
        })
    }

    fn status_topic(&self) -> String {
        format!("{}/status", self.s.base)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs_f64(self.s.timeout_s.clamp(0.1, 120.0))
    }

    fn ensure(&mut self) -> Result<(), OutputError> {
        if self.conn.is_some() {
            return Ok(());
        }
        let mut c =
            self.transport
                .connect(&self.url.host, self.url.port, self.url.tls, self.timeout())?;
        let status = self.status_topic();
        c.write_all(&connect_packet(
            &self.s.client_id,
            &self.s.username,
            &self.s.password,
            self.s.keepalive_s,
            Some((&status, b"offline", true)),
        ))?;
        c.set_read_timeout(Some(self.timeout()))?;
        let (h, body) = read_packet(&mut c)?;
        if h >> 4 != 2 || body.len() < 2 {
            return Err(OutputError::Protocol(crate::Protocol::NoConnack));
        }
        if body[1] != 0 {
            use crate::Protocol as P;
            let why = match body[1] {
                1 => P::BadVersion,
                2 => P::BadClientId,
                3 => P::Unavailable,
                4 => P::BadLogin,
                5 => P::NotAuthorised,
                _ => P::Refused,
            };
            return Err(OutputError::Protocol(why));
        }
        self.conn = Some(c);
        self.send(&status, b"online", true)?;
        if self.s.discovery {
            for (obj, name, family) in SENSORS {
                let node = self.s.node_id.clone();
                let state = format!("{}/last/{}", self.s.base, choice_key(family));
                let cfg = Discovery {
                    name,
                    unique_id: format!("{node}_{obj}"),
                    object_id: format!("{node}_{obj}"),
                    state_topic: state.clone(),
                    value_template: "{{ value_json.identity | default(value_json.kind) }}",
                    json_attributes_topic: state,
                    availability_topic: &status,
                    payload_available: "online",
                    payload_not_available: "offline",
                    icon: "mdi:radio-tower",
                    device: Device {
                        identifiers: [&node],
                        name: "sdrkit",
                        manufacturer: "sdrkit",
                        model: "sdrkit",
                    },
                };
                let cfg = serde_json::to_vec(&cfg)?;
                let t = format!("{}/sensor/{node}/{obj}/config", self.s.discovery_prefix);
                self.send(&t, &cfg, true)?;
            }
        }
        Ok(())
    }

    fn send(&mut self, topic: &str, payload: &[u8], retain: bool) -> Result<(), OutputError> {
        let qos = self.s.qos;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).unwrap_or(1);
        let c = self
            .conn
            .as_mut()
            .ok_or(OutputError::Protocol(crate::Protocol::NotConnected))?;
        c.write_all(&publish_packet(topic, payload, qos, retain, id))?;
        c.flush()?;
        self.last_send = Instant::now();
        if qos == 1 {
            loop {
                let (h, body) = read_packet(c)?;
                match h >> 4 {
                    4 if body.len() >= 2 && u16::from_be_bytes([body[0], body[1]]) == id => break,
                    4 | 13 => continue, // an older PUBACK, PINGRESP
                    _ => return Err(OutputError::Protocol(crate::Protocol::UnexpectedPacket)),
                }
            }
        }
        Ok(())
    }
}

impl Sink<Item> for MqttSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        let Item::Packet {
            topic,
            family,
            body,
        } = item
        else {
            return Ok(());
        };
        self.ensure()?;
        let payload = body.strip_suffix(b"\n").unwrap_or(body);
        let retain = self.s.retain;
        self.send(topic, payload, retain)?;
        if self.s.discovery && SENSORS.iter().any(|(_, _, f)| f == family) {
            let state = format!("{}/last/{}", self.s.base, choice_key(*family));
            self.send(&state, payload, true)?;
        }
        Ok(())
    }

    fn idle(&mut self) -> Result<(), OutputError> {
        let keep = Duration::from_secs(u64::from(self.s.keepalive_s.max(2)) / 2);
        if let Some(c) = self.conn.as_mut() {
            if self.last_send.elapsed() >= keep {
                c.write_all(&[0xC0, 0x00])?;
                c.flush()?;
                let (h, _) = read_packet(c)?;
                if h >> 4 != 13 {
                    return Err(OutputError::Protocol(crate::Protocol::NoPing));
                }
                self.last_send = Instant::now();
            }
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.conn = None;
    }

    fn close(&mut self) {
        if let Some(mut c) = self.conn.take() {
            // Say goodbye cleanly; the broker then does not publish the will, so publish "offline" first.
            let status = self.status_topic();
            let _ = c.write_all(&publish_packet(&status, b"offline", 0, true, 0));
            let _ = c.write_all(&[0xE0, 0x00]);
            let _ = c.flush();
        }
    }

    fn connected(&self) -> bool {
        self.conn.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets() {
        let c = connect_packet("id", "u", "p", 60, Some(("s", b"offline", true)));
        assert_eq!(c[0], 0x10);
        assert_eq!(&c[2..8], b"\x00\x04MQTT");
        assert_eq!(c[8], 4);
        assert_eq!(c[9], 0x80 | 0x40 | 0x20 | 0x04 | 0x02);
        assert_eq!(c[1] as usize, c.len() - 2);
        let p = publish_packet("a/b", b"x", 1, true, 7);
        assert_eq!(p, [0x33, 8, 0, 3, b'a', b'/', b'b', 0, 7, b'x']);
        let big = publish_packet("t", &[0u8; 200], 0, false, 0);
        assert_eq!(&big[1..3], &[0xCB, 0x01]); // 203 as a variable length
        let (h, body) = read_packet(&mut &big[..]).unwrap();
        assert_eq!((h, body.len()), (0x30, 203));
    }

    #[test]
    fn settings_hide_the_password() {
        let s = MqttSettings {
            password: "hunter2".into(),
            ..MqttSettings::default()
        };
        assert!(!format!("{s:?}").contains("hunter2"));
        assert!(MqttSink::new(
            MqttSettings {
                url: "http://x".into(),
                ..MqttSettings::default()
            },
            Arc::new(sdrkit_core::net::StdNet),
            &Clock::fixed(0.0)
        )
        .is_err());
    }
}

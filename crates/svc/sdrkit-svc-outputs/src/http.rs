//! HTTP POST: a small HTTP/1.1 client for webhooks and pushes, one connection per request, and the webhook sink.

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::net::Url;
use crate::runner::{Item, Sink};
use crate::OutputError;
use sdrkit_core::net::Transport;

/// The most bytes of a reply that are read.
const REPLY_CAP: usize = 64 * 1024;

/// Posts `body` to `url` with extra headers; returns the status code and the start of the reply's body. A status outside 200 to 299
/// is an error. Headers and bodies are never put in error messages, so tokens cannot leak into logs.
pub fn post(
    transport: &dyn Transport,
    url: &str,
    content_type: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    timeout: Duration,
) -> Result<(u16, String), OutputError> {
    let u = Url::parse(url)?;
    if u.scheme != "http" && u.scheme != "https" {
        return Err(OutputError::Setting(crate::BadSetting::Scheme));
    }
    let mut c = transport.connect(&u.host, u.port, u.tls, timeout)?;
    let host = if u.host.contains(':') {
        format!("[{}]", u.host)
    } else {
        u.host.clone()
    };
    let mut req = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nUser-Agent: sdrkit\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        u.path,
        host,
        u.port,
        content_type,
        body.len()
    );
    for (k, v) in headers {
        if k.contains(['\r', '\n', ':']) || v.contains(['\r', '\n']) {
            return Err(OutputError::Setting(crate::BadSetting::Header));
        }
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    c.write_all(req.as_bytes())?;
    c.write_all(body)?;
    c.flush()?;
    c.set_read_timeout(Some(timeout))?;
    let mut reply = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match c.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                reply.extend_from_slice(&buf[..n]);
                if reply.len() >= REPLY_CAP {
                    break;
                }
                // Stop once the head and the announced body are in, for servers that keep the connection open.
                if let Some(end) = find(&reply, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&reply[..end]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok());
                    if len.is_some_and(|l| reply.len() >= end + 4 + l) {
                        break;
                    }
                }
            }
            Err(e) if reply.is_empty() => return Err(e.into()),
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&reply);
    let status: u16 = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or(OutputError::Protocol(crate::Protocol::NoStatus))?;
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.chars().take(512).collect())
        .unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(OutputError::Status(status));
    }
    Ok((status, body))
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Webhook settings.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookSettings {
    /// `http://` or `https://` (TLS through the host's transport).
    pub url: String,
    /// Bearer token (empty: none). Never logged.
    pub token: String,
    /// Seconds to wait.
    pub timeout_s: f64,
}

impl Default for WebhookSettings {
    fn default() -> Self {
        WebhookSettings {
            url: String::new(),
            token: String::new(),
            timeout_s: 10.0,
        }
    }
}

impl std::fmt::Debug for WebhookSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookSettings")
            .field("url", &self.url)
            .field("token", &if self.token.is_empty() { "" } else { "***" })
            .finish()
    }
}

/// Posts each item: the observation as JSON.
pub struct WebhookSink {
    s: WebhookSettings,
    transport: Arc<dyn Transport>,
}

impl WebhookSink {
    /// A webhook sink.
    pub fn new(s: WebhookSettings, transport: Arc<dyn Transport>) -> Result<Self, OutputError> {
        Url::parse(&s.url)?;
        Ok(WebhookSink { s, transport })
    }
}

/// A bearer header for a token, or nothing.
pub fn bearer(token: &str) -> Option<String> {
    (!token.is_empty()).then(|| format!("Bearer {token}"))
}

impl Sink<Item> for WebhookSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        let body = item.body().strip_suffix(b"\n").unwrap_or(item.body());
        if body.is_empty() {
            return Ok(());
        }
        let auth = bearer(&self.s.token);
        let headers: Vec<(&str, &str)> = auth
            .as_deref()
            .map(|a| ("Authorization", a))
            .into_iter()
            .collect();
        post(
            &*self.transport,
            &self.s.url,
            "application/json",
            &headers,
            body,
            Duration::from_secs_f64(self.s.timeout_s.clamp(0.1, 120.0)),
        )?;
        Ok(())
    }
}

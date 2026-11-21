//! Addresses and the back-off between connection attempts. The sinks open their sockets through the host's
//! [`Transport`](sdrkit_core::net::Transport).

use std::time::Duration;

use crate::OutputError;

/// The host and the port of `host:port` (`[::1]:port` for IPv6).
///
/// # Errors
///
/// [`crate::BadSetting::Address`] if the text is not `host:port`.
pub fn host_port(text: &str) -> Result<(String, u16), OutputError> {
    sdrkit_core::net::host_port(text, None)
        .map(|(h, p)| (h.to_owned(), p))
        .map_err(|_| OutputError::Setting(crate::BadSetting::Address))
}

/// A parsed address: `scheme://[user:pass@]host[:port][/path]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    /// The scheme, lower case.
    pub scheme: String,
    /// Whether the scheme requires TLS (`https`, `mqtts`, `ssl`, `tls`).
    pub tls: bool,
    /// Host name or address (an IPv6 address without its brackets).
    pub host: String,
    /// Port (the scheme's default when not given).
    pub port: u16,
    /// Path with query, starting with `/`.
    pub path: String,
}

impl Url {
    /// Parses an address. Default ports: http 80, https 443, mqtt 1883, mqtts 8883.
    pub fn parse(s: &str) -> Result<Url, OutputError> {
        let bad = || OutputError::Setting(crate::BadSetting::Address);
        let (scheme, rest) = s.trim().split_once("://").ok_or_else(bad)?;
        let scheme = scheme.to_ascii_lowercase();
        let (auth, path) = match rest.find('/') {
            Some(i) => (&rest[..i], rest[i..].to_string()),
            None => (rest, "/".to_string()),
        };
        let hostport = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
        let (host, port) = if let Some(r) = hostport.strip_prefix('[') {
            let (h, tail) = r.split_once(']').ok_or_else(bad)?;
            (h.to_string(), tail.strip_prefix(':'))
        } else {
            match hostport.rsplit_once(':') {
                Some((h, p)) => (h.to_string(), Some(p)),
                None => (hostport.to_string(), None),
            }
        };
        if host.is_empty() {
            return Err(bad());
        }
        let tls = matches!(scheme.as_str(), "https" | "mqtts" | "ssl" | "tls");
        let default = match scheme.as_str() {
            "http" => 80,
            "https" => 443,
            "mqtt" | "tcp" => 1883,
            "mqtts" | "ssl" | "tls" => 8883,
            _ => 0,
        };
        let port = match port {
            Some(p) => p.parse().map_err(|_| bad())?,
            None if default > 0 => default,
            None => return Err(bad()),
        };
        Ok(Url {
            scheme,
            tls,
            host,
            port,
            path,
        })
    }
}

/// Exponential back-off between connection attempts: 1 s, 2 s, 4 s … up to a cap, reset after a success.
#[derive(Clone, Debug)]
pub struct Backoff {
    next: Duration,
    first: Duration,
    cap: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Backoff::new(Duration::from_secs(1), Duration::from_secs(60))
    }
}

impl Backoff {
    /// A back-off from `first` doubling up to `cap`.
    pub fn new(first: Duration, cap: Duration) -> Self {
        Backoff {
            next: first,
            first,
            cap,
        }
    }
    /// The wait before the next attempt, and doubles it.
    pub fn next_wait(&mut self) -> Duration {
        let w = self.next;
        self.next = (self.next * 2).min(self.cap);
        w
    }
    /// After a success.
    pub fn reset(&mut self) {
        self.next = self.first;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let u = Url::parse("https://hooks.example.org/a/b?c=1").unwrap();
        assert_eq!(
            (u.tls, u.host.as_str(), u.port, u.path.as_str()),
            (true, "hooks.example.org", 443, "/a/b?c=1")
        );
        let m = Url::parse("mqtt://user:pw@[::1]:1884").unwrap();
        assert_eq!(
            (m.host.as_str(), m.port, m.path.as_str(), m.tls),
            ("::1", 1884, "/", false)
        );
        assert_eq!(Url::parse("mqtts://broker").unwrap().port, 8883);
        assert!(Url::parse("broker:1883").is_err());
        assert!(Url::parse("foo://host").is_err());
        assert!(Url::parse("http://:80").is_err());
    }

    #[test]
    fn backoff_doubles_to_a_cap() {
        let mut b = Backoff::new(Duration::from_secs(1), Duration::from_secs(5));
        let w: Vec<u64> = (0..5).map(|_| b.next_wait().as_secs()).collect();
        assert_eq!(w, [1, 2, 4, 5, 5]);
        b.reset();
        assert_eq!(b.next_wait().as_secs(), 1);
    }
}

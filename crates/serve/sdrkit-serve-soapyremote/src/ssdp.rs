//! The discovery answer: a thread on the SSDP port answers an M-SEARCH for the SoapyRemote service (or for all services)
//! with the server's id, port and age, as `discovery::parse_ssdp` reads it.

use sdrkit_core::net::{timed_out, DatagramOptions, Interface, NetError, Transport};
use sdrkit_driver_soapyremote::discovery::{SEARCH_TARGET, SSDP_V4};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// The age the answer states, s.
const MAX_AGE: u32 = 120;

/// The answer to a search: the server `uuid` at `host:port`.
pub fn ssdp_answer(uuid: &str, host: &str, port: u16) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age={MAX_AGE}\r\nEXT:\r\nLOCATION: http://{host}:{port}/\r\nSERVER: {} \
         UPnP/1.1 sdrkit/{}\r\nST: {SEARCH_TARGET}\r\nUSN: uuid:{uuid}::{SEARCH_TARGET}\r\n\r\n",
        std::env::consts::OS,
        env!("CARGO_PKG_VERSION")
    )
}

/// True if `text` is an M-SEARCH for the SoapyRemote service or for all services.
fn is_search(text: &str) -> bool {
    let mut lines = text.split("\r\n").flat_map(|l| l.split('\n'));
    if !lines
        .next()
        .is_some_and(|l| l.trim().to_uppercase().starts_with("M-SEARCH"))
    {
        return false;
    }
    lines.any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("st") && matches!(v.trim(), SEARCH_TARGET | "ssdp:all")
        })
    })
}

/// A running discovery answer. Dropping it stops the thread.
pub struct Ssdp {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Ssdp {
    /// Answers searches sent to the SSDP group and port for the server `uuid` at `host:port`. The socket joins the group
    /// on the default interface.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the port cannot be opened, the group cannot be joined or the thread cannot start.
    pub fn start(
        transport: &Arc<dyn Transport>,
        uuid: &str,
        host: &str,
        port: u16,
    ) -> Result<Ssdp, NetError> {
        Ssdp::start_on(transport, uuid, host, port, SSDP_V4, None)
    }

    /// As [`Ssdp::start`], for searches sent to `group` (a multicast address and a port), joined on `interface` or, for
    /// `None`, on the default interface. Other programs can listen on the same port.
    ///
    /// # Errors
    ///
    /// As [`Ssdp::start`].
    pub fn start_on(
        transport: &Arc<dyn Transport>,
        uuid: &str,
        host: &str,
        port: u16,
        group: SocketAddr,
        interface: Option<Interface>,
    ) -> Result<Ssdp, NetError> {
        let mut sock = transport.bind(&DatagramOptions {
            multicast_interface: interface,
            ..DatagramOptions::member_of(group)
        })?;
        sock.set_read_timeout(Some(Duration::from_millis(20)))?;
        let answer = ssdp_answer(uuid, host, port);
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::Builder::new()
            .name("sdrkit-serve-ssdp".into())
            .spawn(move || {
                let mut buf = [0u8; 2048];
                while !s.load(Ordering::Acquire) {
                    match sock.recv_from(&mut buf) {
                        Ok((n, from)) => {
                            if is_search(&String::from_utf8_lossy(&buf[..n])) {
                                let _ = sock.send_to(answer.as_bytes(), from);
                            }
                        }
                        Err(e) if timed_out(&e) => {}
                        Err(_) => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
            })
            .map_err(NetError::from)?;
        Ok(Ssdp {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Ssdp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_driver_soapyremote::discovery::{m_search, parse_ssdp, SsdpKind};

    #[test]
    fn the_answer_reads_back_and_only_searches_are_answered() {
        let m = parse_ssdp(&ssdp_answer("1234-abcd", "192.168.10.2", 55133)).unwrap();
        assert_eq!(
            (m.kind, m.uuid.as_str(), m.port, m.max_age),
            (
                SsdpKind::Response,
                "1234-abcd",
                55133,
                Duration::from_secs(120)
            )
        );
        assert!(is_search(&m_search()));
        assert!(!is_search(&m_search().replace(SEARCH_TARGET, "urn:other")));
        assert!(!is_search("NOTIFY * HTTP/1.1\r\nST: ssdp:all\r\n\r\n"));
    }
}

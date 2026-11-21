//! TLS by rustls as a [`Transport`] that wraps the plain connections of another transport: the certificate test (the
//! system's trust, given roots, or one pinned certificate) and the TLS handshake.
//!
//! Standard: RFC 8446 (TLS 1.3) and RFC 5246 (TLS 1.2), through rustls.

use core::net::SocketAddr;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};
use sdrkit_core::net::{Conn, Datagram, DatagramOptions, Interface, Listener, NetError, Transport};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Which server certificates a connection accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trust {
    /// The system's trust: the Security framework on Apple platforms, the system's root bundle elsewhere.
    System,
    /// Only certificates that chain to these roots (DER).
    Roots(Vec<Vec<u8>>),
    /// Only the certificate whose SHA-256 digest (of its DER form) is this value. This suits a server with a self-signed
    /// certificate.
    Pinned([u8; 32]),
}

/// Reads a SHA-256 certificate digest written as 64 hexadecimal digits. Colons and spaces between the digits are allowed.
/// `None` if the text is not such a digest.
///
/// ```
/// let d = sdrkit_driver_core::ws::tls::parse_pin(&"AB:".repeat(32)).unwrap();
/// assert_eq!(d, [0xab; 32]);
/// assert!(sdrkit_driver_core::ws::tls::parse_pin("12").is_none());
/// ```
pub fn parse_pin(text: &str) -> Option<[u8; 32]> {
    let digits: Vec<u8> = text.bytes().filter(|b| !matches!(b, b':' | b' ')).collect();
    if digits.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (o, pair) in out.iter_mut().zip(digits.as_chunks::<2>().0) {
        *o = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

/// The certificate test for a connection: `roots` if given, else the system's trust.
///
/// # Errors
///
/// [`NetError::Tls`] if a root is not a certificate.
pub fn verifier(
    provider: Arc<CryptoProvider>,
    roots: Option<&[Vec<u8>]>,
) -> Result<Arc<dyn ServerCertVerifier>, NetError> {
    if let Some(r) = roots {
        let mut store = RootCertStore::empty();
        for der in r {
            store
                .add(CertificateDer::from(der.clone()))
                .map_err(|_| NetError::Tls)?;
        }
        return webpki(store, provider);
    }
    system(provider)
}

fn webpki(
    store: RootCertStore,
    provider: Arc<CryptoProvider>,
) -> Result<Arc<dyn ServerCertVerifier>, NetError> {
    if store.is_empty() {
        return Ok(Arc::new(NoTrust(provider)));
    }
    WebPkiServerVerifier::builder_with_provider(Arc::new(store), provider)
        .build()
        .map(|v| v as Arc<dyn ServerCertVerifier>)
        .map_err(|_| NetError::Tls)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn system(provider: Arc<CryptoProvider>) -> Result<Arc<dyn ServerCertVerifier>, NetError> {
    Ok(Arc::new(apple::Verifier(provider)))
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn system(provider: Arc<CryptoProvider>) -> Result<Arc<dyn ServerCertVerifier>, NetError> {
    use rustls::pki_types::pem::PemObject;
    let mut store = RootCertStore::empty();
    for path in [
        "/etc/ssl/certs/ca-certificates.crt",
        "/etc/pki/tls/certs/ca-bundle.crt",
        "/etc/ssl/ca-bundle.pem",
        "/etc/ssl/cert.pem",
        "/system/etc/security/cacerts.pem",
    ] {
        if let Ok(it) = CertificateDer::pem_file_iter(path) {
            for c in it.flatten() {
                let _ = store.add(c);
            }
        }
        if !store.is_empty() {
            break;
        }
    }
    webpki(store, provider)
}

fn schemes(p: &CryptoProvider) -> Vec<SignatureScheme> {
    p.signature_verification_algorithms.supported_schemes()
}

/// The signature tests every verifier here shares: the provider's algorithms.
macro_rules! signatures {
    () => {
        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }
        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            schemes(&self.0)
        }
    };
}

/// Trusts nothing (no roots were found): every certificate fails as not trusted.
#[derive(Debug)]
struct NoTrust(Arc<CryptoProvider>);

impl ServerCertVerifier for NoTrust {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::UnknownIssuer,
        ))
    }
    signatures!();
}

/// Trusts one certificate, by the SHA-256 digest of its DER form. It still holds the handshake signatures against it.
#[derive(Debug)]
struct Pinned(Arc<CryptoProvider>, [u8; 32]);

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if sdrkit_algo_crypto::sha256(end_entity.as_ref()) == self.1 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::UnknownIssuer,
            ))
        }
    }
    signatures!();
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use super::*;
    use security_framework::certificate::SecCertificate;
    use security_framework::policy::SecPolicy;
    use security_framework::secure_transport::SslProtocolSide;
    use security_framework::trust::SecTrust;

    /// Asks the system's trust evaluation (the same as the platform's own web views), so profiles and enterprise roots count.
    #[derive(Debug)]
    pub struct Verifier(pub Arc<CryptoProvider>);

    impl ServerCertVerifier for Verifier {
        fn verify_server_cert(
            &self,
            end_entity: &CertificateDer<'_>,
            intermediates: &[CertificateDer<'_>],
            server_name: &ServerName<'_>,
            _ocsp: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            let bad = || rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding);
            let certs = std::iter::once(end_entity)
                .chain(intermediates)
                .map(|c| SecCertificate::from_der(c.as_ref()).map_err(|_| bad()))
                .collect::<Result<Vec<_>, _>>()?;
            let host = match server_name {
                ServerName::DnsName(d) => d.as_ref().to_string(),
                ServerName::IpAddress(ip) => core::net::IpAddr::from(*ip).to_string(),
                _ => return Err(bad()),
            };
            let policy = SecPolicy::create_ssl(SslProtocolSide::SERVER, Some(&host));
            let trust = SecTrust::create_with_certificates(&certs, &[policy]).map_err(|_| bad())?;
            trust.evaluate_with_error().map_err(|_| {
                rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer)
            })?;
            Ok(ServerCertVerified::assertion())
        }
        signatures!();
    }
}

/// The rustls client set-up for `trust`, with the ring crypto provider and the safe default protocol versions.
///
/// # Errors
///
/// [`NetError::Tls`] if rustls refuses the set-up.
pub fn client_config(trust: &Trust) -> Result<Arc<rustls::ClientConfig>, NetError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier: Arc<dyn ServerCertVerifier> = match trust {
        Trust::System => verifier(provider.clone(), None)?,
        Trust::Roots(r) => verifier(provider.clone(), Some(r))?,
        Trust::Pinned(d) => Arc::new(Pinned(provider.clone(), *d)),
    };
    let cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| NetError::Tls)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(Arc::new(cfg))
}

/// A [`Transport`] that adds TLS to the connections of another transport: a connection with `tls` set runs the TLS handshake
/// over a plain connection of the inner transport. Datagrams and listeners are those of the inner transport.
pub struct RustlsTransport {
    inner: Arc<dyn Transport>,
    config: Arc<rustls::ClientConfig>,
}

impl RustlsTransport {
    /// TLS with the certificate test `trust` over `inner`.
    ///
    /// # Errors
    ///
    /// [`NetError::Tls`] if rustls refuses the set-up.
    pub fn new(inner: Arc<dyn Transport>, trust: &Trust) -> Result<Self, NetError> {
        Ok(RustlsTransport {
            inner,
            config: client_config(trust)?,
        })
    }

    fn handshake(
        &self,
        mut conn: Box<dyn Conn>,
        host: &str,
        wait: Duration,
    ) -> Result<Box<dyn Conn>, NetError> {
        let name = ServerName::try_from(host.to_string()).map_err(|_| NetError::BadAddress)?;
        let session =
            rustls::ClientConnection::new(self.config.clone(), name).map_err(|_| NetError::Tls)?;
        let ctl = conn.try_clone()?;
        conn.set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;
        let mut s = rustls::StreamOwned::new(session, conn);
        while s.conn.is_handshaking() {
            if let Err(e) = s.conn.complete_io(&mut s.sock) {
                let inner = e
                    .get_ref()
                    .and_then(|i| i.downcast_ref::<rustls::Error>())
                    .cloned();
                return Err(match inner {
                    Some(rustls::Error::InvalidCertificate(_)) => NetError::Untrusted,
                    Some(_) => NetError::Tls,
                    None => NetError::TimedOut,
                });
            }
        }
        Ok(Box::new(TlsConn {
            session: Arc::new(Mutex::new(s)),
            ctl,
        }))
    }
}

impl Transport for RustlsTransport {
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn Conn>, NetError> {
        let conn = self.inner.connect(host, port, false, timeout)?;
        if !tls {
            return Ok(conn);
        }
        self.handshake(conn, host, timeout)
    }
    fn datagram(&self, host: &str, port: u16) -> Result<Box<dyn Datagram>, NetError> {
        self.inner.datagram(host, port)
    }
    fn bind(&self, options: &DatagramOptions) -> Result<Box<dyn Datagram>, NetError> {
        self.inner.bind(options)
    }
    fn listen(&self, host: &str, port: u16) -> Result<Box<dyn Listener>, NetError> {
        self.inner.listen(host, port)
    }
    fn resolve(&self, host: &str, port: u16) -> Result<SocketAddr, NetError> {
        self.inner.resolve(host, port)
    }
    fn interfaces(&self) -> Vec<Interface> {
        self.inner.interfaces()
    }
}

type Session = rustls::StreamOwned<rustls::ClientConnection, Box<dyn Conn>>;

/// A TLS connection. Its handles share one session: reads and writes take turns on its lock, and a read holds the lock for at
/// most the read timeout, so keep that timeout short (100 ms).
struct TlsConn {
    session: Arc<Mutex<Session>>,
    ctl: Box<dyn Conn>,
}

impl TlsConn {
    fn lock(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Read for TlsConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.lock().read(buf)
    }
}

impl Write for TlsConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.lock().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.lock().flush()
    }
}

impl Conn for TlsConn {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        self.ctl.set_read_timeout(timeout)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        self.ctl.set_write_timeout(timeout)
    }
    fn set_recv_buffer(&mut self, bytes: usize) -> Result<(), NetError> {
        self.ctl.set_recv_buffer(bytes)
    }
    fn try_clone(&self) -> Result<Box<dyn Conn>, NetError> {
        Ok(Box::new(TlsConn {
            session: self.session.clone(),
            ctl: self.ctl.try_clone()?,
        }))
    }
    fn shutdown(&self) {
        self.ctl.shutdown();
    }
    fn peer_addr(&self) -> Option<core::net::SocketAddr> {
        self.ctl.peer_addr()
    }
}

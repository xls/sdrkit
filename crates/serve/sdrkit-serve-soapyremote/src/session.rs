//! One RPC connection: it frames the packets, answers each call from the device description and the owner's answers, and
//! sets up, activates and closes the client's streams over UDP or TCP (the stream argument `remote:prot`). A call it does
//! not know is answered with an exception, which a client takes as "not supported".

use crate::stream::{tx_loop, RxLink, StreamOutlet, TxEnds};
use crate::{native, rx_formats, Device, Side};
use sdrkit_core::net::{Conn, DatagramOptions, Transport};
use sdrkit_driver_soapyremote::format::Format;
use sdrkit_driver_soapyremote::rpc::{
    self, call, ArgInfo, ArgType, Kwargs, Packer, Unexpected, Unpacker, HEADER, MAX_PACKET,
    OVERHEAD, RX, TRAILER,
};
use sdrkit_driver_soapyremote::wire::{Wire, PROT_KEY};
use sdrkit_serve_core::{Answer, Context, Parser, Request, TxRequest};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The key of the bias tee setting.
const BIAS_KEY: &str = "biastee";
/// The MTU when the client states none.
const DEFAULT_MTU: usize = 1500;
/// How long the set-up of a TCP stream waits for the client's two connections.
const ACCEPT_WAIT: Duration = Duration::from_secs(5);

type Reply = Result<Packer, String>;

/// A stream that the client set up.
enum Stream {
    Rx {
        link: Arc<Mutex<RxLink>>,
        /// The status path of a TCP stream, held open until the close.
        _status: Option<Wire>,
        format: Format,
        /// Set when the present outlet must stop.
        off: Arc<AtomicBool>,
    },
    Tx {
        ends: Option<TxEnds>,
        writer: Option<sdrkit_serve_core::TxWriter>,
        bpe: usize,
        window: u32,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    },
}

/// The values a direction last got, as the client reads them back.
#[derive(Clone, Debug, Default)]
struct Granted {
    freq: Option<f64>,
    rate: Option<f64>,
    gain: Option<f64>,
    agc: bool,
    ppm: f64,
    bandwidth: Option<f64>,
    antenna: Option<String>,
    elements: Vec<(String, f64)>,
}

/// The state of one RPC connection.
pub(crate) struct Rpc {
    transport: Arc<dyn Transport>,
    stream_host: Option<String>,
    device: Arc<Device>,
    uuid: String,
    peer: Option<SocketAddr>,
    buf: Vec<u8>,
    streams: Vec<Option<Stream>>,
    rx: Granted,
    tx: Granted,
    bias: bool,
}

fn void() -> Packer {
    let mut p = Packer::default();
    p.void();
    p
}

fn one(f: impl FnOnce(&mut Packer)) -> Reply {
    let mut p = Packer::default();
    f(&mut p);
    Ok(p)
}

fn bad(_: Unexpected) -> String {
    "the arguments of the call do not parse".into()
}

impl Rpc {
    /// A connection of the client at `peer`; TCP streams listen on `stream_host` (`None`: every address of the peer's
    /// family).
    pub(crate) fn new(
        transport: Arc<dyn Transport>,
        stream_host: Option<String>,
        device: Arc<Device>,
        uuid: &str,
        peer: Option<SocketAddr>,
    ) -> Rpc {
        Rpc {
            transport,
            stream_host,
            device,
            uuid: uuid.to_owned(),
            peer,
            buf: Vec::new(),
            streams: Vec::new(),
            rx: Granted::default(),
            tx: Granted::default(),
            bias: false,
        }
    }

    fn side(&self, dir: i8) -> Option<&Side> {
        if dir == RX {
            Some(&self.device.rx)
        } else {
            self.device.tx.as_ref()
        }
    }

    fn granted(&mut self, dir: i8) -> &mut Granted {
        if dir == RX {
            &mut self.rx
        } else {
            &mut self.tx
        }
    }

    /// Asks the owner for a receive setting; for transmit the value is kept for the transmit request. It returns the
    /// value granted.
    fn ask(&mut self, cx: &mut Context<'_>, dir: i8, req: Request) -> Result<Request, String> {
        if dir != RX {
            return Ok(req);
        }
        match cx.request(req) {
            Answer::Accepted => Ok(req),
            Answer::Adjusted(r) => Ok(r),
            Answer::Refused => Err(format!("the server refused {}", what(&req))),
        }
    }

    /// Answers the call in `payload`.
    fn answer(&mut self, payload: &[u8], version: u32, cx: &mut Context<'_>) -> Reply {
        let mut u = Unpacker::new(payload, version);
        let n = u.call().map_err(bad)?;
        let dev = self.device.clone();
        match n {
            call::FIND => one(|p| {
                p.kwargs_list(&[self.kwargs()]);
            }),
            call::MAKE => {
                let kw = u.kwargs().unwrap_or_default();
                let differs = |key: &str, have: &str| {
                    kw.get(key)
                        .is_some_and(|v| !v.is_empty() && !v.eq_ignore_ascii_case(have))
                };
                let serial = dev.info.get("serial").map_or("", String::as_str);
                if differs("driver", &dev.driver) || differs("serial", serial) {
                    return Err("no match".into());
                }
                Ok(void())
            }
            call::UNMAKE | call::START_LOG | call::STOP_LOG => Ok(void()),
            call::HANGUP => {
                cx.hang_up();
                Ok(void())
            }
            call::SERVER_ID => one(|p| {
                p.string(&self.uuid);
            }),
            call::DRIVER_KEY => one(|p| {
                p.string(&dev.driver);
            }),
            call::HARDWARE_KEY => one(|p| {
                p.string(&dev.hardware);
            }),
            call::HARDWARE_INFO => one(|p| {
                p.kwargs(&dev.info);
            }),
            call::NUM_CHANNELS => {
                let dir = u.char().map_err(bad)?;
                one(|p| {
                    p.i32(i32::from(self.side(dir).is_some()));
                })
            }
            call::SETTING_INFO => one(|p| {
                let list: Vec<ArgInfo> = dev
                    .bias_tee
                    .then(|| ArgInfo {
                        key: BIAS_KEY.into(),
                        value: "false".into(),
                        name: "Bias tee".into(),
                        description: "Power on the antenna input".into(),
                        kind: Some(ArgType::Bool),
                        ..ArgInfo::default()
                    })
                    .into_iter()
                    .collect();
                p.arg_info_list(&list, version);
            }),
            call::READ_SETTING => {
                let key = u.string().map_err(bad)?;
                if !(dev.bias_tee && key.eq_ignore_ascii_case(BIAS_KEY)) {
                    return Err(format!("no setting {key}"));
                }
                one(|p| {
                    p.string(if self.bias { "true" } else { "false" });
                })
            }
            call::WRITE_SETTING => {
                let (key, value) = (u.string().map_err(bad)?, u.string().map_err(bad)?);
                if !(dev.bias_tee && key.eq_ignore_ascii_case(BIAS_KEY)) {
                    return Err(format!("no setting {key}"));
                }
                let on = matches!(
                    value.trim().to_lowercase().as_str(),
                    "true" | "1" | "yes" | "on"
                );
                self.ask(cx, RX, Request::BiasTee { on })?;
                self.bias = on;
                Ok(void())
            }
            call::LIST_SENSORS => one(|p| {
                p.string_list::<&str>(&[]);
            }),
            call::SETUP_STREAM => self.setup(&mut u, cx),
            call::ACTIVATE_STREAM => self.activate(&mut u, cx),
            call::DEACTIVATE_STREAM => {
                let id = u.i32().map_err(bad)?;
                self.deactivate(id, cx);
                one(|p| {
                    p.i32(0);
                })
            }
            call::CLOSE_STREAM => {
                let id = u.i32().map_err(bad)?;
                self.deactivate(id, cx);
                if let Some(s) = usize::try_from(id)
                    .ok()
                    .and_then(|i| self.streams.get_mut(i))
                {
                    *s = None;
                }
                Ok(void())
            }
            _ => self.channel_call(n, &mut u, cx, version),
        }
    }

    /// The calls that name a direction and a channel.
    fn channel_call(
        &mut self,
        n: u32,
        u: &mut Unpacker<'_>,
        cx: &mut Context<'_>,
        version: u32,
    ) -> Reply {
        let dir = u.char().map_err(bad)?;
        let ch = u.i32().map_err(bad)?;
        let Some(side) = self.side(dir).cloned().filter(|_| ch == 0) else {
            return Err(format!("no channel {ch} in direction {dir}"));
        };
        let source = cx.client().source_format();
        let capture = cx.client().capture();
        match n {
            call::FULL_DUPLEX => one(|p| {
                p.bool(true);
            }),
            call::HAS_GAIN_MODE => one(|p| {
                p.bool(side.agc);
            }),
            call::HAS_FREQ_CORRECTION => one(|p| {
                p.bool(side.correction);
            }),
            call::CHANNEL_INFO => one(|p| {
                p.kwargs(&Kwargs::new());
            }),
            call::STREAM_FORMATS => one(|p| {
                let names: Vec<&str> = if dir == RX {
                    rx_formats(source).iter().map(|f| f.name()).collect()
                } else {
                    vec![Format::Cs16.name(), Format::Cf32.name()]
                };
                p.string_list(&names);
            }),
            call::NATIVE_FORMAT => one(|p| {
                let (f, scale) = if dir == RX {
                    native(source)
                } else {
                    (Format::Cs16, 32768.0)
                };
                p.string(f.name()).f64(scale);
            }),
            call::STREAM_ARG_INFO => one(|p| {
                p.arg_info_list(&[], version);
            }),
            call::FREQUENCY_RANGE => one(|p| {
                p.range_list(&side.freqs, version);
            }),
            call::LIST_FREQUENCIES => one(|p| {
                p.string_list(&["RF"]);
            }),
            call::SAMPLE_RATE_RANGE => one(|p| {
                p.range_list(&side.rates, version);
            }),
            call::LIST_SAMPLE_RATES => one(|p| {
                let list: Vec<f64> = side.rates.iter().flat_map(|r| [r.min, r.max]).collect();
                p.f64_list(&list);
            }),
            call::BANDWIDTH_RANGE => one(|p| {
                p.range_list(&side.bandwidths, version);
            }),
            call::LIST_BANDWIDTHS => one(|p| {
                let list: Vec<f64> = side
                    .bandwidths
                    .iter()
                    .flat_map(|r| [r.min, r.max])
                    .collect();
                p.f64_list(&list);
            }),
            call::GAIN_RANGE => one(|p| {
                p.range(side.gain, version);
            }),
            call::LIST_GAINS => one(|p| {
                let names: Vec<&str> = side.elements.iter().map(|e| e.0.as_str()).collect();
                p.string_list(&names);
            }),
            call::GAIN_RANGE_ELEMENT => {
                let name = u.string().map_err(bad)?;
                let r = side
                    .elements
                    .iter()
                    .find(|e| e.0 == name)
                    .ok_or_else(|| format!("no gain element {name}"))?
                    .1;
                one(|p| {
                    p.range(r, version);
                })
            }
            call::LIST_ANTENNAS => one(|p| {
                p.string_list(&side.antennas);
            }),
            call::GET_ANTENNA => {
                let a = self
                    .granted(dir)
                    .antenna
                    .clone()
                    .or_else(|| side.antennas.first().cloned())
                    .unwrap_or_default();
                one(|p| {
                    p.string(&a);
                })
            }
            call::SET_ANTENNA => {
                let name = u.string().map_err(bad)?;
                let i = side
                    .antennas
                    .iter()
                    .position(|a| *a == name)
                    .ok_or_else(|| format!("no antenna {name}"))?;
                self.ask(
                    cx,
                    dir,
                    Request::Other {
                        name: "antenna",
                        args: [i as i64, 0],
                    },
                )?;
                self.granted(dir).antenna = Some(name);
                Ok(void())
            }
            call::SET_FREQUENCY => {
                let hz = u.f64().map_err(bad)?;
                let got = self.ask(cx, dir, Request::Frequency { hz })?;
                self.granted(dir).freq = Some(value_of(&got).unwrap_or(hz));
                Ok(void())
            }
            call::GET_FREQUENCY => {
                let v = if dir == RX {
                    cx.client().slice().map(|s| s.centre_hz).or(self.rx.freq)
                } else {
                    self.tx.freq
                };
                one(|p| {
                    p.f64(v.unwrap_or(capture.centre_hz));
                })
            }
            call::SET_SAMPLE_RATE => {
                let hz = u.f64().map_err(bad)?;
                let got = self.ask(cx, dir, Request::Rate { hz })?;
                self.granted(dir).rate = Some(value_of(&got).unwrap_or(hz));
                Ok(void())
            }
            call::GET_SAMPLE_RATE => {
                let v = if dir == RX {
                    cx.client().slice().map(|s| s.rate_hz).or(self.rx.rate)
                } else {
                    self.tx.rate
                };
                one(|p| {
                    p.f64(v.unwrap_or(capture.rate_hz));
                })
            }
            call::SET_BANDWIDTH => {
                let hz = u.f64().map_err(bad)?;
                if side.bandwidths.is_empty() {
                    return Ok(void());
                }
                let got = self.ask(cx, dir, Request::Bandwidth { hz })?;
                self.granted(dir).bandwidth = Some(value_of(&got).unwrap_or(hz));
                Ok(void())
            }
            call::GET_BANDWIDTH => {
                let v = self.granted(dir).bandwidth.unwrap_or(capture.rate_hz);
                one(|p| {
                    p.f64(v);
                })
            }
            call::SET_GAIN => {
                let db = u.f64().map_err(bad)?;
                if !has_gain(&side) {
                    return Ok(void());
                }
                let got = self.ask(cx, dir, Request::Gain { db })?;
                let g = self.granted(dir);
                g.gain = Some(value_of(&got).unwrap_or(db));
                // The overall gain spreads over the elements again.
                g.elements.clear();
                Ok(void())
            }
            call::GET_GAIN => {
                let v = overall(&side, self.granted(dir));
                one(|p| {
                    p.f64(v);
                })
            }
            call::SET_GAIN_ELEMENT => {
                let (name, db) = (u.string().map_err(bad)?, u.f64().map_err(bad)?);
                let i = side
                    .elements
                    .iter()
                    .position(|e| e.0 == name)
                    .ok_or_else(|| format!("no gain element {name}"))?;
                let req = if side.elements.len() == 1 {
                    Request::Gain { db }
                } else {
                    Request::Other {
                        name: "gain_element",
                        args: [i as i64, (db * 1000.0).round() as i64],
                    }
                };
                let got = self.ask(cx, dir, req)?;
                let g = self.granted(dir);
                if side.elements.len() == 1 {
                    g.gain = Some(value_of(&got).unwrap_or(db));
                } else {
                    g.elements.retain(|e| e.0 != name);
                    g.elements.push((name, db));
                }
                Ok(void())
            }
            call::GET_GAIN_ELEMENT => {
                let name = u.string().map_err(bad)?;
                let v = element(&side, self.granted(dir), &name)
                    .ok_or_else(|| format!("no gain element {name}"))?;
                one(|p| {
                    p.f64(v);
                })
            }
            call::SET_GAIN_MODE => {
                let on = u.bool().map_err(bad)?;
                if !side.agc {
                    return Ok(void());
                }
                let got = self.ask(cx, dir, Request::Agc { on })?;
                self.granted(dir).agc = match got {
                    Request::Agc { on } => on,
                    _ => on,
                };
                Ok(void())
            }
            call::GET_GAIN_MODE => {
                let v = self.granted(dir).agc;
                one(|p| {
                    p.bool(v);
                })
            }
            call::SET_FREQ_CORRECTION => {
                let ppm = u.f64().map_err(bad)?;
                if !side.correction {
                    return Ok(void());
                }
                self.ask(cx, dir, Request::Correction { ppm })?;
                self.granted(dir).ppm = ppm;
                Ok(void())
            }
            call::GET_FREQ_CORRECTION => {
                let v = self.granted(dir).ppm;
                one(|p| {
                    p.f64(v);
                })
            }
            call::LIST_CHANNEL_SENSORS => one(|p| {
                p.string_list::<&str>(&[]);
            }),
            _ => Err(format!("call {n} is not supported")),
        }
    }

    fn kwargs(&self) -> Kwargs {
        let mut kw = self.device.info.clone();
        kw.insert("driver".into(), self.device.driver.clone());
        kw.insert("label".into(), self.device.hardware.clone());
        kw
    }

    /// Call 300: the paths of a stream, its id and the server's port. Everything that can refuse the stream is checked
    /// before a TCP stream sends its early reply.
    fn setup(&mut self, u: &mut Unpacker<'_>, cx: &mut Context<'_>) -> Reply {
        let dir = u.char().map_err(bad)?;
        let name = u.string().map_err(bad)?;
        let channels = u.size_list().map_err(bad)?;
        let kw = u.kwargs().map_err(bad)?;
        let (dport, sport) = (u.string().map_err(bad)?, u.string().map_err(bad)?);
        if self.side(dir).is_none() || channels.iter().any(|c| *c != 0) {
            return Err("no such channel".into());
        }
        let prot = kw.get(PROT_KEY).map_or("udp", |p| p.trim());
        let tcp = if prot.eq_ignore_ascii_case("tcp") {
            true
        } else if prot.eq_ignore_ascii_case("udp") {
            false
        } else {
            return Err(format!(
                "stream protocol {prot} is not supported: udp or tcp"
            ));
        };
        let ports = if tcp {
            None
        } else {
            let port = |s: &str, what: &str| {
                s.trim()
                    .parse::<u16>()
                    .map_err(|_| format!("bad {what} port"))
            };
            Some((port(&dport, "data")?, port(&sport, "status")?))
        };
        let format = Format::from_name(&name).ok_or_else(|| format!("no format {name}"))?;
        let source = cx.client().source_format();
        let ok = if dir == RX {
            rx_formats(source).contains(&format)
        } else {
            matches!(format, Format::Cs16 | Format::Cf32)
        };
        if !ok {
            return Err(format!("format {name} is not served"));
        }
        let mtu = kw
            .get("remote:mtu")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_MTU);
        let window_bytes: usize = kw
            .get("remote:window")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(1 << 20);
        let peer = self
            .peer
            .map(|p| p.ip())
            .ok_or("the client has no address")?;
        let writer = if dir == RX {
            None
        } else {
            let g = self.tx.clone();
            let req = TxRequest {
                format: format.sample_type(),
                rate_hz: g.rate.unwrap_or_else(|| {
                    self.device
                        .tx
                        .as_ref()
                        .and_then(|t| t.rates.first())
                        .map_or(0.0, |r| r.min)
                }),
                centre_hz: g.freq.unwrap_or(0.0),
            };
            Some(
                cx.transmit(req)
                    .map_err(|_| "the server refused to transmit".to_string())?,
            )
        };
        let (data, status, port) = match ports {
            None => self.tcp_paths(peer, cx)?,
            Some((dport, sport)) => {
                let any = match peer {
                    IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                };
                let open = || {
                    self.transport
                        .bind(&DatagramOptions::at(SocketAddr::new(any, 0)))
                        .map_err(|e| e.to_string())
                };
                let data = open()?;
                let port = data.local_addr().port();
                let data = Wire::udp(data, SocketAddr::new(peer, dport));
                // A receive stream sends nothing on the status path of UDP.
                let status = match writer {
                    Some(_) => Some(Wire::udp(open()?, SocketAddr::new(peer, sport))),
                    None => None,
                };
                (data, status, port)
            }
        };
        let stream = match writer {
            None => Stream::Rx {
                link: Arc::new(Mutex::new(RxLink::new(data, format.bytes(), mtu))),
                _status: status,
                format,
                off: Arc::new(AtomicBool::new(false)),
            },
            Some(writer) => Stream::Tx {
                ends: Some(TxEnds {
                    data,
                    status: status.ok_or("the stream has no status path")?,
                }),
                writer: Some(writer),
                bpe: format.bytes(),
                window: (window_bytes / mtu.max(1)).max(8) as u32,
                stop: Arc::new(AtomicBool::new(false)),
                thread: None,
            },
        };
        let id = self.streams.len() as i32;
        self.streams.push(Some(stream));
        one(|p| {
            p.i32(id).string(&port.to_string());
        })
    }

    /// The two connections of a TCP stream: it listens, sends the port as the early reply, and takes the data connection
    /// first and the status connection second from the client's address. It returns them and the port.
    fn tcp_paths(
        &self,
        peer: IpAddr,
        cx: &mut Context<'_>,
    ) -> Result<(Wire, Option<Wire>, u16), String> {
        let host = self.stream_host.clone().unwrap_or_else(|| {
            match peer {
                IpAddr::V4(_) => "0.0.0.0",
                IpAddr::V6(_) => "::",
            }
            .to_owned()
        });
        let mut listener = self
            .transport
            .listen(&host, 0)
            .map_err(|e| format!("cannot listen for the stream: {e}"))?;
        let port = listener.local_addr().port();
        let mut early = Packer::default();
        early.string(&port.to_string());
        if !cx.reply(&rpc::packet(&early.buf, rpc::VERSION)) {
            return Err("the client left".into());
        }
        let end = Instant::now() + ACCEPT_WAIT;
        let mut got: Vec<Box<dyn Conn>> = Vec::with_capacity(2);
        while got.len() < 2 {
            match listener.accept() {
                Ok(Some(c)) if c.peer_addr().is_some_and(|a| a.ip() == peer) => got.push(c),
                // Another host: not this client's stream.
                Ok(Some(_)) => {}
                Ok(None) if Instant::now() < end && !cx.client().stopping() => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(None) => return Err("the client did not connect its stream".into()),
                Err(e) => return Err(e.to_string()),
            }
        }
        let status = got.pop().map(Wire::tcp);
        let data = got.pop().map(Wire::tcp).ok_or("no data connection")?;
        Ok((data, status, port))
    }

    /// Call 302: a receive stream gets its outlet, a transmit stream its receiving thread.
    fn activate(&mut self, u: &mut Unpacker<'_>, cx: &mut Context<'_>) -> Reply {
        let id = u.i32().map_err(bad)?;
        let s = usize::try_from(id)
            .ok()
            .and_then(|i| self.streams.get_mut(i))
            .and_then(Option::as_mut)
            .ok_or_else(|| format!("no stream {id}"))?;
        match s {
            Stream::Rx {
                link, format, off, ..
            } => {
                off.store(true, Ordering::Release);
                *off = Arc::new(AtomicBool::new(false));
                let o = StreamOutlet::new(link.clone(), format.sample_type(), off.clone());
                cx.client().set_outlet(Some(Box::new(o)));
            }
            Stream::Tx {
                ends,
                writer,
                bpe,
                window,
                stop,
                thread,
            } => {
                if thread.is_none() {
                    let (Some(e), Some(w)) = (ends.take(), writer.take()) else {
                        return Err("the stream was closed".into());
                    };
                    let (bpe, window, stop2) = (*bpe, *window, stop.clone());
                    *thread = Some(
                        std::thread::Builder::new()
                            .name("sdrkit-serve-soapy-tx".into())
                            .spawn(move || tx_loop(e, w, bpe, window, &stop2))
                            .map_err(|e| e.to_string())?,
                    );
                }
            }
        }
        one(|p| {
            p.i32(0);
        })
    }

    fn deactivate(&mut self, id: i32, cx: &mut Context<'_>) {
        let Some(s) = usize::try_from(id)
            .ok()
            .and_then(|i| self.streams.get_mut(i))
            .and_then(Option::as_mut)
        else {
            return;
        };
        match s {
            Stream::Rx { off, .. } => {
                off.store(true, Ordering::Release);
                cx.client().set_outlet(None);
            }
            Stream::Tx { stop, thread, .. } => {
                stop.store(true, Ordering::Release);
                if let Some(t) = thread.take() {
                    let _ = t.join();
                }
            }
        }
    }

    fn close_all(&mut self, cx: &mut Context<'_>) {
        for id in 0..self.streams.len() {
            self.deactivate(id as i32, cx);
        }
        self.streams.clear();
    }
}

/// True if `side` has a gain that can be set.
fn has_gain(side: &Side) -> bool {
    side.gain.max > side.gain.min || side.elements.iter().any(|e| e.1.max > e.1.min)
}

/// The gain of element `name`: the value the client set on it, else its part of the overall gain. The overall gain
/// above the bottom of its range fills the elements in order, each up to its span. `None` for no such element.
fn element(side: &Side, g: &Granted, name: &str) -> Option<f64> {
    let mut left = g.gain.unwrap_or(side.gain.min) - side.gain.min;
    for (n, r) in &side.elements {
        let part = left.clamp(0.0, (r.max - r.min).max(0.0));
        left -= part;
        if n == name {
            let set = g.elements.iter().find(|e| e.0 == *name).map(|e| e.1);
            return Some(set.unwrap_or(r.min + part));
        }
    }
    None
}

/// The overall gain: the value granted, or with elements that the client set one by one, the bottom of the overall
/// range plus what each element is above the bottom of its own.
fn overall(side: &Side, g: &Granted) -> f64 {
    if g.elements.is_empty() || side.elements.len() < 2 {
        return g.gain.unwrap_or(side.gain.min);
    }
    side.gain.min
        + side
            .elements
            .iter()
            .filter_map(|(n, r)| element(side, g, n).map(|v| v - r.min))
            .sum::<f64>()
}

/// A request as the text of a refusal.
fn what(r: &Request) -> String {
    match *r {
        Request::Frequency { hz } => format!("the frequency {hz} Hz"),
        Request::Rate { hz } => format!("the sample rate {hz} Hz"),
        Request::Bandwidth { hz } => format!("the bandwidth {hz} Hz"),
        Request::Gain { db } => format!("the gain {db} dB"),
        Request::Agc { on } => format!("automatic gain {}", if on { "on" } else { "off" }),
        Request::BiasTee { on } => format!("the bias tee {}", if on { "on" } else { "off" }),
        Request::Correction { ppm } => format!("the frequency correction {ppm} ppm"),
        Request::Other { name, .. } => format!("the setting {name}"),
    }
}

/// The value that a granted request carries.
fn value_of(r: &Request) -> Option<f64> {
    match *r {
        Request::Frequency { hz } | Request::Rate { hz } | Request::Bandwidth { hz } => Some(hz),
        Request::Gain { db } => Some(db),
        _ => None,
    }
}

impl Parser for Rpc {
    fn feed(&mut self, bytes: &[u8], cx: &mut Context<'_>) {
        self.buf.extend_from_slice(bytes);
        while self.buf.len() >= 12 {
            let len =
                u32::from_be_bytes([self.buf[8], self.buf[9], self.buf[10], self.buf[11]]) as usize;
            if self.buf[..4] != HEADER || !(OVERHEAD..=MAX_PACKET).contains(&len) {
                cx.hang_up();
                return;
            }
            if self.buf.len() < len {
                return;
            }
            let packet: Vec<u8> = self.buf.drain(..len).collect();
            if packet[len - 4..] != TRAILER {
                cx.hang_up();
                return;
            }
            let version = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
            let reply = match self.answer(&packet[12..len - 4], version, cx) {
                Ok(p) => p,
                Err(text) => {
                    let mut p = Packer::default();
                    p.exception(&text);
                    p
                }
            };
            if !cx.reply(&rpc::packet(&reply.buf, rpc::VERSION)) {
                return;
            }
        }
    }

    fn closed(&mut self, cx: &mut Context<'_>) {
        self.close_all(cx);
    }
}

impl Drop for Rpc {
    fn drop(&mut self) {
        for s in self.streams.iter_mut().flatten() {
            if let Stream::Tx { stop, thread, .. } = s {
                stop.store(true, Ordering::Release);
                if let Some(t) = thread.take() {
                    let _ = t.join();
                }
            }
        }
    }
}

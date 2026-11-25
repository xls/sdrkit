//! One RPC connection: it frames the packets, answers each call from the device description and the owner's answers, and
//! sets up, activates and closes the client's streams. A call it does not know is answered with an exception, which a
//! client takes as "not supported".

use crate::stream::{tx_loop, TxEnds, UdpOutlet};
use crate::{native, rx_formats, Device, Side};
use sdrkit_core::net::{DatagramOptions, Transport};
use sdrkit_driver_soapyremote::format::Format;
use sdrkit_driver_soapyremote::rpc::{
    self, call, ArgInfo, ArgType, Kwargs, Packer, Unexpected, Unpacker, HEADER, MAX_PACKET,
    OVERHEAD, RX, TRAILER,
};
use sdrkit_serve_core::{Answer, Context, Parser, Request, TxRequest};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// The key of the bias tee setting.
const BIAS_KEY: &str = "biastee";
/// The MTU when the client states none.
const DEFAULT_MTU: usize = 1500;

type Reply = Result<Packer, String>;

/// A stream that the client set up.
enum Stream {
    Rx {
        data: Box<dyn sdrkit_core::net::Datagram>,
        to: SocketAddr,
        format: Format,
        mtu: usize,
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
    pub(crate) fn new(
        transport: Arc<dyn Transport>,
        device: Arc<Device>,
        uuid: &str,
        peer: Option<SocketAddr>,
    ) -> Rpc {
        Rpc {
            transport,
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
            Answer::Refused => Err("the server refused the setting".into()),
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
            call::FULL_DUPLEX | call::HAS_GAIN_MODE | call::HAS_FREQ_CORRECTION => one(|p| {
                p.bool(true);
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
                let got = self.ask(cx, dir, Request::Gain { db })?;
                self.granted(dir).gain = Some(value_of(&got).unwrap_or(db));
                Ok(void())
            }
            call::GET_GAIN => {
                let v = self.granted(dir).gain.unwrap_or(side.gain.min);
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
                self.ask(cx, dir, req)?;
                let g = self.granted(dir);
                g.elements.retain(|e| e.0 != name);
                g.elements.push((name, db));
                Ok(void())
            }
            call::GET_GAIN_ELEMENT => {
                let name = u.string().map_err(bad)?;
                let range = side
                    .elements
                    .iter()
                    .find(|e| e.0 == name)
                    .ok_or_else(|| format!("no gain element {name}"))?
                    .1;
                let g = self.granted(dir);
                let v = g
                    .elements
                    .iter()
                    .find(|e| e.0 == name)
                    .map(|e| e.1)
                    .or(g.gain)
                    .unwrap_or(range.min);
                one(|p| {
                    p.f64(v);
                })
            }
            call::SET_GAIN_MODE => {
                let on = u.bool().map_err(bad)?;
                self.ask(cx, dir, Request::Agc { on })?;
                self.granted(dir).agc = on;
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

    /// Call 300: the sockets of a stream, its id and the server's port.
    fn setup(&mut self, u: &mut Unpacker<'_>, cx: &mut Context<'_>) -> Reply {
        let dir = u.char().map_err(bad)?;
        let name = u.string().map_err(bad)?;
        let channels = u.size_list().map_err(bad)?;
        let kw = u.kwargs().map_err(bad)?;
        let dport: u16 = u
            .string()
            .map_err(bad)?
            .trim()
            .parse()
            .map_err(|_| "bad data port")?;
        let sport: u16 = u
            .string()
            .map_err(bad)?
            .trim()
            .parse()
            .map_err(|_| "bad status port")?;
        if self.side(dir).is_none() || channels.iter().any(|c| *c != 0) {
            return Err("no such channel".into());
        }
        let prot = kw.get("remote:prot").map_or("udp", String::as_str);
        if !prot.eq_ignore_ascii_case("udp") {
            return Err(format!("this server streams over udp, not {prot}"));
        }
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
        let data_to = SocketAddr::new(peer, dport);
        let stream = if dir == RX {
            Stream::Rx {
                data,
                to: data_to,
                format,
                mtu,
            }
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
            let writer = cx
                .transmit(req)
                .map_err(|_| "the server refused to transmit".to_string())?;
            let window = (window_bytes / mtu.max(1)).max(8) as u32;
            Stream::Tx {
                ends: Some(TxEnds {
                    data,
                    status: open()?,
                    data_to,
                    status_to: SocketAddr::new(peer, sport),
                }),
                writer: Some(writer),
                bpe: format.bytes(),
                window,
                stop: Arc::new(AtomicBool::new(false)),
                thread: None,
            }
        };
        let id = self.streams.len() as i32;
        self.streams.push(Some(stream));
        one(|p| {
            p.i32(id).string(&port.to_string());
        })
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
                data,
                to,
                format,
                mtu,
            } => {
                let d = data.try_clone().map_err(|e| e.to_string())?;
                let o = UdpOutlet::new(d, *to, format.sample_type(), format.bytes(), *mtu);
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
            Stream::Rx { .. } => cx.client().set_outlet(None),
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

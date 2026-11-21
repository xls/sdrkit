//! A fake SDR++ server on a fake network. It serves one client at a time and models an Airspy source.

use crate::menu::step;
use crate::proto::{cmd, pkt};
use sdrkit_core::net::{Conn, FakeTransport, Transport};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub(crate) const S: &str = "Airspy Source";
pub(crate) const SR_ID: &str = "##_airspy_sr_sel_Airspy Source";
pub(crate) const LIN_ID: &str = "##_airspy_lin_gain_Airspy Source";
const RATES: [f64; 2] = [10e6, 2.5e6];

/// The fake's state and options.
#[derive(Debug)]
pub(crate) struct FakeState {
    pub rate_entry: usize,
    pub gain_mode: usize,
    pub lin_gain: i32,
    pub sens_gain: i32,
    pub lna: i32,
    pub mix: i32,
    pub vga: i32,
    pub lna_agc: bool,
    pub bias: bool,
    pub running: bool,
    pub sample_type: u8,
    pub compression: bool,
    pub freq: f64,
    pub freq_bytes: Vec<u8>,
    pub log: Vec<String>,
    /// Broken answers sent in race mode.
    pub broken: u32,
    pub accepts: u32,
    pub busy_answers: u32,
    pub client: bool,
    // Options.
    pub race: bool,
    pub throttle: f64,
    pub cmd_delay: Duration,
    pub i_follows_gain: bool,
    pub no_source: bool,
    pub unknown: Vec<u32>,
    pub corrupt_frames: u32,
    pub drop_now: bool,
    pub push_rate: Option<f64>,
    /// Bytes of every packet the server received, in order.
    pub received: Vec<Vec<u8>>,
    /// Bytes of every non-sample packet the server sent, in order.
    pub sent: Vec<Vec<u8>>,
}

impl Default for FakeState {
    fn default() -> Self {
        FakeState {
            rate_entry: 0,
            gain_mode: 1,
            lin_gain: 18,
            sens_gain: 10,
            lna: 0,
            mix: 0,
            vga: 0,
            lna_agc: false,
            bias: false,
            running: false,
            sample_type: 1,
            compression: false,
            freq: 0.0,
            freq_bytes: Vec::new(),
            log: Vec::new(),
            broken: 0,
            accepts: 0,
            busy_answers: 0,
            client: false,
            race: false,
            throttle: 1.0,
            cmd_delay: Duration::ZERO,
            i_follows_gain: false,
            no_source: false,
            unknown: Vec::new(),
            corrupt_frames: 0,
            drop_now: false,
            push_rate: None,
            received: Vec::new(),
            sent: Vec::new(),
        }
    }
}

impl FakeState {
    pub fn rate(&self) -> f64 {
        RATES[self.rate_entry]
    }
    pub fn has(&self, line: &str) -> bool {
        self.log.iter().any(|l| l == line)
    }
    pub fn pos(&self, line: &str) -> Option<usize> {
        self.log.iter().position(|l| l == line)
    }
}

/// A draw-list writer.
#[derive(Default)]
pub(crate) struct W(pub Vec<u8>);
impl W {
    pub fn step(mut self, code: u8, sync: u8) -> Self {
        self.0.extend([0, code, sync]);
        self
    }
    pub fn s(mut self, t: &str) -> Self {
        self.0.push(4);
        self.0.extend((t.len() as u16).to_le_bytes());
        self.0.extend(t.as_bytes());
        self
    }
    pub fn i(mut self, v: i32) -> Self {
        self.0.push(2);
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn f(mut self, v: f32) -> Self {
        self.0.push(3);
        self.0.extend(v.to_le_bytes());
        self
    }
    pub fn b(mut self, v: bool) -> Self {
        self.0.extend([1, v as u8]);
        self
    }
    fn when(self, c: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if c {
            f(self)
        } else {
            self
        }
    }
    fn slider(self, label: &str, id: &str, v: i32, hi: i32, disabled: bool) -> Self {
        self.step(step::LEFT_LABEL, 0)
            .s(label)
            .step(step::FILL_WIDTH, 0)
            .when(disabled, |w| w.step(step::BEGIN_DISABLED, 0))
            .step(step::SLIDER_INT, 0)
            .s(id)
            .i(v)
            .i(0)
            .i(hi)
            .i(1)
            .i(0)
            .when(disabled, |w| w.step(step::END_DISABLED, 0))
    }
}

/// The menu of the fake: the real menu, with the changes the fake state makes.
pub(crate) fn menu(st: &FakeState) -> Vec<u8> {
    let run = st.running;
    let w = W::default()
        .when(run, |w| w.step(step::BEGIN_DISABLED, 0))
        .step(step::FILL_WIDTH, 0)
        .when(!st.no_source, |w| {
            w.step(step::COMBO, 1)
                .s("##sdrpp_server_src_sel")
                .i(0)
                .s("Airspy\0")
                .i(-1)
                .step(step::FILL_WIDTH, 0)
        })
        .step(step::COMBO, 1)
        .s(&format!("##_airspy_dev_sel_{S}"))
        .i(0)
        .s("258C62DC2D43588F\0")
        .i(-1)
        .step(step::COMBO, 0)
        .s(SR_ID)
        .i(st.rate_entry as i32)
        .s("10.0MHz\x002.5MHz\0")
        .i(-1)
        .step(step::SAME_LINE, 0)
        .step(step::FILL_WIDTH, 0)
        .step(step::BUTTON, 1)
        .s(&format!("Refresh##_airspy_refr_{S}"))
        .f(0.0)
        .f(0.0)
        .when(run, |w| w.step(step::END_DISABLED, 0))
        .step(step::BEGIN_GROUP, 0)
        .step(step::COLUMNS, 0)
        .i(3)
        .s(&format!("AirspyGainModeColumns##_{S}"))
        .b(false)
        .step(step::RADIO_BUTTON, 1)
        .s(&format!("Sensitive##_airspy_gm_{S}"))
        .b(st.gain_mode == 0)
        .step(step::NEXT_COLUMN, 0)
        .step(step::RADIO_BUTTON, 1)
        .s(&format!("Linear##_airspy_gm_{S}"))
        .b(st.gain_mode == 1)
        .step(step::NEXT_COLUMN, 0)
        .step(step::RADIO_BUTTON, 1)
        .s(&format!("Free##_airspy_gm_{S}"))
        .b(st.gain_mode == 2)
        .step(step::COLUMNS, 0)
        .i(1)
        .s(&format!("EndAirspyGainModeColumns##_{S}"))
        .b(false)
        .step(step::END_GROUP, 0);
    let w = match st.gain_mode {
        0 => w.slider(
            "Gain",
            &format!("##_airspy_sens_gain_{S}"),
            st.sens_gain,
            21,
            false,
        ),
        1 => w.slider("Gain", LIN_ID, st.lin_gain, 21, false),
        _ => w
            .slider(
                "LNA Gain",
                &format!("##_airspy_lna_gain_{S}"),
                st.lna,
                15,
                st.lna_agc,
            )
            .slider(
                "Mixer Gain",
                &format!("##_airspy_mix_gain_{S}"),
                st.mix,
                15,
                false,
            )
            .slider(
                "VGA Gain",
                &format!("##_airspy_vga_gain_{S}"),
                st.vga,
                15,
                false,
            )
            .step(step::CHECKBOX, 0)
            .s(&format!("LNA AGC##_airspy_{S}"))
            .b(st.lna_agc),
    };
    w.step(step::CHECKBOX, 0)
        .s(&format!("Bias T##_airspy_{S}"))
        .b(st.bias)
        .0
}

pub(crate) fn packet(kind: u32, body: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(8 + body.len());
    p.extend(kind.to_le_bytes());
    p.extend(((8 + body.len()) as u32).to_le_bytes());
    p.extend(body);
    p
}

fn command(c: u32, arg: &[u8]) -> Vec<u8> {
    let mut b = c.to_le_bytes().to_vec();
    b.extend(arg);
    packet(pkt::COMMAND, &b)
}

fn ack(c: u32, data: &[u8]) -> Vec<u8> {
    let mut b = c.to_le_bytes().to_vec();
    b.extend(data);
    packet(pkt::COMMAND_ACK, &b)
}

fn rate_packet(r: f64) -> Vec<u8> {
    command(cmd::SET_SAMPLERATE, &r.to_le_bytes())
}

/// The running fake.
pub(crate) struct Fake {
    pub net: FakeTransport,
    pub st: Arc<Mutex<FakeState>>,
    stop: Arc<AtomicBool>,
}

pub(crate) const HOST: &str = "127.0.0.1";
pub(crate) const PORT: u16 = 5259;

impl Fake {
    pub fn start() -> Fake {
        Self::with(|_| {})
    }

    pub fn with(f: impl FnOnce(&mut FakeState)) -> Fake {
        let mut st = FakeState::default();
        f(&mut st);
        let st = Arc::new(Mutex::new(st));
        let stop = Arc::new(AtomicBool::new(false));
        let net = FakeTransport::new();
        let (s2, stop2) = (st.clone(), stop.clone());
        net.serve(HOST, PORT, move |s| accept(s, &s2, &stop2));
        Fake { net, st, stop }
    }

    pub fn addr(&self) -> String {
        format!("{HOST}:{PORT}")
    }

    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    pub fn state(&self) -> MutexGuard<'_, FakeState> {
        self.st.lock().unwrap()
    }

    /// Waits until `f` holds, up to `secs`.
    pub fn wait(&self, secs: f64, f: impl Fn(&FakeState) -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs_f64(secs);
        while Instant::now() < end {
            if f(&self.state()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        f(&self.state())
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.net.stop_serving(HOST, PORT);
    }
}

/// One accepted connection: the server, or a DISCONNECT when a client is already served.
fn accept(mut s: Box<dyn Conn>, st: &Arc<Mutex<FakeState>>, stop: &Arc<AtomicBool>) {
    let busy = {
        let mut g = st.lock().unwrap();
        g.accepts += 1;
        let b = g.client;
        if b {
            g.busy_answers += 1;
        } else {
            g.client = true;
        }
        b
    };
    if busy {
        let _ = s.write_all(&command(cmd::DISCONNECT, &[]));
        std::thread::sleep(Duration::from_millis(50));
        s.shutdown();
    } else {
        serve(s, st, stop);
        st.lock().unwrap().client = false;
    }
}

/// Reads the client's packets and forwards them with their arrival time.
fn reader(mut s: Box<dyn Conn>, tx: mpsc::Sender<(Instant, Vec<u8>)>) {
    loop {
        let mut h = [0u8; 8];
        if s.read_exact(&mut h).is_err() {
            return;
        }
        let size = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
        if !(8..=1 << 20).contains(&size) {
            return;
        }
        let mut p = h.to_vec();
        p.resize(size, 0);
        if s.read_exact(&mut p[8..]).is_err() {
            return;
        }
        if tx.send((Instant::now(), p)).is_err() {
            return;
        }
    }
}

struct Client<'a> {
    s: Box<dyn Conn>,
    st: &'a Mutex<FakeState>,
    rate_sent: bool,
}

impl Client<'_> {
    fn send(&mut self, p: &[u8], record: bool) -> bool {
        if record {
            self.st.lock().unwrap().sent.push(p.to_vec());
        }
        self.s.write_all(p).is_ok()
    }

    /// Handles one client packet. Returns false when the connection should end.
    fn handle(&mut self, p: &[u8]) -> bool {
        let mut g = self.st.lock().unwrap();
        g.received.push(p.to_vec());
        let kind = u32::from_le_bytes(p[0..4].try_into().unwrap());
        let body = &p[8..];
        if kind != pkt::COMMAND || body.len() < 4 {
            drop(g);
            return self.send(&packet(pkt::ERROR, &[1]), true);
        }
        let c = u32::from_le_bytes(body[0..4].try_into().unwrap());
        let arg = &body[4..];
        if g.unknown.contains(&c) {
            drop(g);
            return self.send(&packet(pkt::ERROR, &[2]), true);
        }
        match c {
            cmd::GET_UI => {
                g.log.push("get_ui".into());
                if g.race && !self.rate_sent {
                    // The broken answer of the connect race: the rate's header in front of the menu.
                    g.broken += 1;
                    let m = menu(&g);
                    drop(g);
                    let mut b = vec![1, 0, 0, 0, 0x14, 0, 0, 0, 0, 0, 0, 0];
                    b.extend(m);
                    return self.send(&b, true);
                }
                let m = menu(&g);
                drop(g);
                self.send(&ack(cmd::GET_UI, &m), true)
            }
            cmd::UI_ACTION => {
                let Some((sync, id, v)) = parse_action(arg) else {
                    drop(g);
                    return self.send(&packet(pkt::ERROR, &[3]), true);
                };
                g.log.push(format!("action {id}={v}"));
                let n = v.parse::<f64>().unwrap_or(0.0);
                let i = n.round() as i32;
                let mut rate_change = false;
                match id.as_str() {
                    SR_ID => {
                        if (0..2).contains(&i) {
                            g.rate_entry = i as usize;
                            rate_change = true;
                        }
                    }
                    LIN_ID => g.lin_gain = i,
                    _ if id == format!("##_airspy_sens_gain_{S}") => g.sens_gain = i,
                    _ if id == format!("##_airspy_lna_gain_{S}") => g.lna = i,
                    _ if id == format!("##_airspy_mix_gain_{S}") => g.mix = i,
                    _ if id == format!("##_airspy_vga_gain_{S}") => g.vga = i,
                    _ if id == format!("LNA AGC##_airspy_{S}") => g.lna_agc = i != 0,
                    _ if id == format!("Bias T##_airspy_{S}") => g.bias = i != 0,
                    _ if id == format!("Sensitive##_airspy_gm_{S}") => g.gain_mode = 0,
                    _ if id == format!("Linear##_airspy_gm_{S}") => g.gain_mode = 1,
                    _ if id == format!("Free##_airspy_gm_{S}") => g.gain_mode = 2,
                    _ => {}
                }
                let r = g.rate();
                let m = menu(&g);
                drop(g);
                if rate_change && !self.send(&rate_packet(r), true) {
                    return false;
                }
                !sync || self.send(&ack(cmd::UI_ACTION, &m), true)
            }
            cmd::START => {
                g.running = true;
                g.log.push("start".into());
                true
            }
            cmd::STOP => {
                g.running = false;
                g.log.push("stop".into());
                true
            }
            cmd::SET_FREQUENCY if arg.len() == 8 => {
                let f = f64::from_le_bytes(arg.try_into().unwrap());
                g.freq = f;
                g.freq_bytes = arg.to_vec();
                g.log.push(format!("freq {}", f.round() as i64));
                drop(g);
                self.send(&ack(cmd::SET_FREQUENCY, &[]), true)
            }
            cmd::SET_SAMPLE_TYPE if arg.len() == 1 => {
                g.sample_type = arg[0];
                g.log.push(format!("type {}", arg[0]));
                true
            }
            cmd::SET_COMPRESSION if arg.len() == 1 => {
                g.compression = arg[0] != 0;
                g.log.push(format!("compression {}", arg[0]));
                true
            }
            _ => {
                drop(g);
                self.send(&packet(pkt::ERROR, &[2]), true)
            }
        }
    }

    fn samples(&mut self, n: usize) -> bool {
        let g = self.st.lock().unwrap();
        let i = if g.i_follows_gain {
            g.lin_gain as f32 / 32.0
        } else {
            0.5
        };
        let q = -0.25f32;
        let ty = g.sample_type;
        let compress = g.compression;
        let corrupt = g.corrupt_frames > 0;
        drop(g);
        let mut body = Vec::with_capacity(8 + n * 8);
        body.extend(0u16.to_le_bytes());
        body.extend((ty as u16).to_le_bytes());
        body.extend(1.0f32.to_le_bytes());
        for _ in 0..n {
            match ty {
                0 => body.extend([
                    (i * 128.0).round() as i8 as u8,
                    (q * 128.0).round() as i8 as u8,
                ]),
                2 => {
                    body.extend(i.to_le_bytes());
                    body.extend(q.to_le_bytes());
                }
                _ => {
                    body.extend(((i * 32768.0).round() as i16).to_le_bytes());
                    body.extend(((q * 32768.0).round() as i16).to_le_bytes());
                }
            }
        }
        let p = if corrupt {
            self.st.lock().unwrap().corrupt_frames -= 1;
            packet(
                pkt::BASEBAND_COMPRESSED,
                &[
                    0x28, 0xb5, 0x2f, 0xfd, 0x08, 0x10, 0x85, 0x00, 0x00, 0xff, 0xff, 0xff, 0x13,
                ],
            )
        } else if compress {
            let z = ruzstd::encoding::compress_to_vec(
                &body[..],
                ruzstd::encoding::CompressionLevel::Fastest,
            );
            packet(pkt::BASEBAND_COMPRESSED, &z)
        } else {
            packet(pkt::BASEBAND, &body)
        };
        self.send(&p, false)
    }
}

fn parse_action(a: &[u8]) -> Option<(bool, String, String)> {
    let sync = *a.first()? == 1;
    if *a.get(1)? != 4 {
        return None;
    }
    let len = u16::from_le_bytes(a.get(2..4)?.try_into().ok()?) as usize;
    let id = String::from_utf8_lossy(a.get(4..4 + len)?).into_owned();
    let v = &a[4 + len..];
    let val = match *v.first()? {
        1 => format!("{}", *v.get(1)?),
        2 => format!("{}", i32::from_le_bytes(v.get(1..5)?.try_into().ok()?)),
        3 => format!("{}", f32::from_le_bytes(v.get(1..5)?.try_into().ok()?)),
        _ => return None,
    };
    Some((sync, id, val))
}

fn serve(mut s: Box<dyn Conn>, st: &Mutex<FakeState>, stop: &AtomicBool) {
    let accepted = Instant::now();
    let race = {
        let mut g = st.lock().unwrap();
        g.running = false;
        g.sample_type = 1;
        g.compression = false;
        g.race
    };
    let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
    let (tx, rx) = mpsc::channel();
    let rs = s.try_clone().unwrap();
    let rt = std::thread::spawn(move || reader(rs, tx));
    let mut c = Client {
        s,
        st,
        rate_sent: false,
    };
    if !race {
        let r = st.lock().unwrap().rate();
        c.rate_sent = true;
        if !c.send(&rate_packet(r), true) {
            return;
        }
    }
    let mut queue: VecDeque<(Instant, Vec<u8>)> = VecDeque::new();
    let mut last = Instant::now();
    let mut acc = 0.0f64;
    let mut was_running = false;
    let mut gone = false;
    'run: while !stop.load(Ordering::Acquire) {
        if !c.rate_sent && accepted.elapsed() >= Duration::from_millis(300) {
            let r = st.lock().unwrap().rate();
            c.rate_sent = true;
            if !c.send(&rate_packet(r), true) {
                break;
            }
        }
        loop {
            match rx.try_recv() {
                Ok(p) => queue.push_back(p),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    gone = true;
                    break;
                }
            }
        }
        let delay = st.lock().unwrap().cmd_delay;
        while queue.front().is_some_and(|(t, _)| t.elapsed() >= delay) {
            let (_, p) = queue.pop_front().unwrap();
            if !c.handle(&p) {
                break 'run;
            }
        }
        if gone && queue.is_empty() {
            break;
        }
        let (running, rate, throttle, drop_now, push) = {
            let mut g = st.lock().unwrap();
            (
                g.running,
                g.rate(),
                g.throttle,
                std::mem::take(&mut g.drop_now),
                g.push_rate.take(),
            )
        };
        if drop_now {
            c.s.shutdown();
            break;
        }
        if let Some(r) = push {
            if !c.send(&rate_packet(r), true) {
                break;
            }
        }
        let now = Instant::now();
        if running {
            if !was_running {
                acc = 0.0;
                last = now;
            }
            acc += now.duration_since(last).as_secs_f64() * rate * throttle;
            let n = (acc.floor() as usize).min(1 << 20);
            if n > 0 {
                acc -= n as f64;
                if !c.samples(n) {
                    break;
                }
            }
        }
        was_running = running;
        last = now;
        std::thread::sleep(Duration::from_millis(2));
    }
    c.s.shutdown();
    let _ = rt.join();
}

/// A server that answers like a web server, on its own fake network.
pub(crate) fn not_sdrpp() -> FakeTransport {
    let net = FakeTransport::new();
    net.serve(HOST, PORT, |mut s| {
        let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
        std::thread::sleep(Duration::from_millis(300));
    });
    net
}

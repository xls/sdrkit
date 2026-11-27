//! A fake rtl_tcp server on a fake network, for tests: it sends the greeting, streams 8-bit samples, records the commands of
//! each connection and can drop or mute connections.

use super::{cmd, RtlTcpDriver, DEFAULT_PORT};
use sdrkit_core::net::{Conn, FakeTransport, Transport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How the fake server behaves.
#[derive(Clone)]
pub struct FakeOpts {
    /// The 12 bytes sent first.
    pub greeting: [u8; 12],
    /// Stream at this rate whatever rate is requested. None obeys command 0x02.
    pub own_rate: Option<f64>,
    /// Close the first connection after this long.
    pub drop_first_after: Option<Duration>,
    /// Connections after the first get no greeting and no samples.
    pub mute_after_first: bool,
    /// The samples have the magnitude [`level_of`] of the frequency of the tuner, and a frequency command takes effect
    /// this long after it arrives: the samples of that time are those of the frequency before. The server looks for
    /// commands before each piece of samples that it makes, so no sample of the time after a command misses it.
    pub tune_takes: Option<Duration>,
}

impl Default for FakeOpts {
    fn default() -> Self {
        FakeOpts {
            greeting: *b"RTL0\0\0\0\x05\0\0\0\x1d",
            own_rate: None,
            drop_first_after: None,
            mute_after_first: false,
            tune_takes: None,
        }
    }
}

/// The magnitude of the samples of a server with [`FakeOpts::tune_takes`] while its tuner is at `hz`: 0.1 at 100 MHz and
/// 0.05 more for each MHz above, to the nearest value of an 8-bit sample.
pub fn level_of(hz: f64) -> f32 {
    (f32::from(level_step(hz)) + 0.5) / 128.0
}

/// The distance of the I part from the middle of its range, in steps, for [`level_of`].
fn level_step(hz: f64) -> u8 {
    let level = 0.1 + (hz - 100e6) / 1e6 * 0.05;
    (level * 128.0 - 0.5).round().clamp(0.0, 127.0) as u8
}

/// The tuner of a server with [`FakeOpts::tune_takes`]: the frequency in effect, and the one that takes effect after
/// a count of bytes more.
#[derive(Default)]
struct Tuner {
    hz: f64,
    next: Option<(usize, f64)>,
}

impl Tuner {
    /// A frequency command for `hz` arrived: it takes effect after `bytes` more bytes.
    fn command(&mut self, hz: f64, bytes: usize) {
        if let Some((_, hz)) = self.next.take() {
            self.hz = hz;
        }
        self.next = Some((bytes & !1, hz));
    }

    /// `n` bytes are made: returns the frequency of the first `k` of them, `k`, and the frequency of the others.
    fn make(&mut self, n: usize) -> (f64, usize, f64) {
        let old = self.hz;
        match self.next {
            Some((left, hz)) if left <= n => {
                (self.hz, self.next) = (hz, None);
                (old, left, hz)
            }
            Some((left, hz)) => {
                self.next = Some((left - n, hz));
                (old, n, old)
            }
            None => (old, n, old),
        }
    }
}

#[derive(Default)]
struct Seen {
    cmds: Mutex<Vec<(u8, u32)>>,
}

/// A fake rtl_tcp server. It stops when dropped.
pub struct FakeRtl {
    net: FakeTransport,
    conns: Arc<Mutex<Vec<Arc<Seen>>>>,
    stop: Arc<AtomicBool>,
}

impl FakeRtl {
    /// A server on a fake network of its own, at `127.0.0.1` on the rtl_tcp port.
    pub fn start(opts: FakeOpts) -> FakeRtl {
        Self::start_on(FakeTransport::new(), opts)
    }

    /// A server on the fake network `net`, at `127.0.0.1` on the rtl_tcp port.
    pub fn start_on(net: FakeTransport, opts: FakeOpts) -> FakeRtl {
        let conns: Arc<Mutex<Vec<Arc<Seen>>>> = Default::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2) = (conns.clone(), stop.clone());
        net.serve("127.0.0.1", DEFAULT_PORT, move |s| {
            let seen = Arc::new(Seen::default());
            let first = {
                let mut c = c2.lock().unwrap();
                c.push(seen.clone());
                c.len() == 1
            };
            serve(s, &seen, &opts, first, &s2);
        });
        FakeRtl { net, conns, stop }
    }

    /// The address to open: host and port.
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{DEFAULT_PORT}")
    }

    /// A driver on the network of the server.
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    /// The fake network the server is on.
    pub fn net(&self) -> &FakeTransport {
        &self.net
    }

    /// A driver on the network of the server.
    pub fn driver(&self) -> RtlTcpDriver {
        RtlTcpDriver::new(Arc::new(self.net.clone()))
    }

    /// The number of connections made so far.
    pub fn connections(&self) -> usize {
        self.conns.lock().unwrap().len()
    }

    /// The commands (byte and value) that connection `i` sent, in order.
    pub fn cmds(&self, i: usize) -> Vec<(u8, u32)> {
        self.conns
            .lock()
            .unwrap()
            .get(i)
            .map(|c| c.cmds.lock().unwrap().clone())
            .unwrap_or_default()
    }

    /// Stops the server. Open connections end.
    pub fn shutdown(&mut self) {
        self.net.stop_serving("127.0.0.1", DEFAULT_PORT);
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for FakeRtl {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve(mut s: Box<dyn Conn>, seen: &Arc<Seen>, o: &FakeOpts, first: bool, stop: &AtomicBool) {
    if !first && o.mute_after_first {
        let mut b = [0u8; 64];
        let _ = s.set_read_timeout(Some(Duration::from_millis(50)));
        while !stop.load(Ordering::Relaxed) {
            if let Ok(0) = s.read(&mut b) {
                return;
            }
        }
        return;
    }
    if s.write_all(&o.greeting).is_err() {
        return;
    }
    let rate = Arc::new(Mutex::new(o.own_rate.unwrap_or(0.0)));
    let mut r = s.try_clone().unwrap();
    let (rate2, seen2, obey) = (rate.clone(), seen.clone(), o.own_rate.is_none());
    let mut tuner = Tuner::default();
    // With a tuner, this thread reads the commands itself between the pieces of samples.
    let mut commands = Vec::new();
    if o.tune_takes.is_some() {
        let _ = r.set_read_timeout(Some(Duration::from_millis(5)));
    } else {
        std::thread::spawn(move || {
            let mut c = [0u8; 5];
            while r.read_exact(&mut c).is_ok() {
                let v = u32::from_be_bytes([c[1], c[2], c[3], c[4]]);
                seen2.cmds.lock().unwrap().push((c[0], v));
                if obey && c[0] == cmd::RATE {
                    *rate2.lock().unwrap() = f64::from(v);
                }
            }
        });
        return stream(s, o, first, stop, &rate, None);
    }
    let takes = o.tune_takes.unwrap_or_default();
    let mut read = move |tuner: &mut Tuner, rate: &Mutex<f64>| -> bool {
        let mut b = [0u8; 64];
        match r.read(&mut b) {
            Ok(0) => return false,
            Ok(k) => commands.extend_from_slice(&b[..k]),
            Err(_) => {}
        }
        while commands.len() >= 5 {
            let c: Vec<u8> = commands.drain(..5).collect();
            let v = u32::from_be_bytes([c[1], c[2], c[3], c[4]]);
            seen.cmds.lock().unwrap().push((c[0], v));
            if c[0] == cmd::FREQ {
                let bytes = (takes.as_secs_f64() * *rate.lock().unwrap() * 2.0) as usize;
                tuner.command(f64::from(v), bytes);
            }
            if obey && c[0] == cmd::RATE {
                *rate.lock().unwrap() = f64::from(v);
            }
        }
        true
    };
    let rate3 = rate.clone();
    let mut tuned = move |n: usize| -> Option<(f64, usize, f64)> {
        read(&mut tuner, &rate3).then(|| tuner.make(n))
    };
    stream(s, o, first, stop, &rate, Some(&mut tuned));
}

/// What [`stream`] asks before it makes a piece of `n` bytes: the frequency of the first bytes, their count and the
/// frequency of the others. `None` ends the connection. The call waits up to 5 ms for a command.
type Tuned<'a> = &'a mut dyn FnMut(usize) -> Option<(f64, usize, f64)>;

/// Sends the samples of one connection at the rate `rate` until the connection or the server ends. Without `tuned`
/// the bytes count up.
fn stream(
    mut s: Box<dyn Conn>,
    o: &FakeOpts,
    first: bool,
    stop: &AtomicBool,
    rate: &Mutex<f64>,
    mut tuned: Option<Tuned<'_>>,
) {
    let start = Instant::now();
    let mut epoch = (start, 0.0f64);
    let mut sent: u64 = 0;
    let mut k: u8 = 0;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if stop.load(Ordering::Relaxed)
            || (first && o.drop_first_after.is_some_and(|d| start.elapsed() >= d))
        {
            s.shutdown();
            return;
        }
        let r = *rate.lock().unwrap();
        if r != epoch.1 {
            epoch = (Instant::now(), r);
            sent = 0;
        }
        let due = (epoch.0.elapsed().as_secs_f64() * r * 2.0) as u64;
        if due > sent {
            let n = (due - sent).min(1 << 20) as usize;
            if let Some(tuned) = tuned.as_mut() {
                let Some((old, from, new)) = tuned(n) else {
                    return;
                };
                for (i, b) in buf[..n].iter_mut().enumerate() {
                    let step = level_step(if i < from { old } else { new });
                    // I goes up and down by the level around the middle, so the samples have no DC part; Q stays there.
                    let at = sent as usize + i;
                    let up = (at / 2).is_multiple_of(2);
                    *b = match (at.is_multiple_of(2), up) {
                        (true, true) => 128 + step,
                        (true, false) => 127 - step,
                        (false, _) => 128,
                    };
                }
            } else {
                for b in &mut buf[..n] {
                    *b = k;
                    k = k.wrapping_add(1);
                }
            }
            if s.write_all(&buf[..n]).is_err() {
                return;
            }
            sent += n as u64;
        } else if let Some(tuned) = tuned.as_mut() {
            if tuned(0).is_none() {
                return;
            }
            continue;
        }
        if tuned.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

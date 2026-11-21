//! One receiver of an HPSDR radio, as the shared runner drives it. The opener joins the stream of the radio, and opens it when
//! this is the first receiver. The read loop does no network I/O: it writes the samples of the packets that the stream thread
//! passes to it into its ring as 32-bit words.

use crate::hub::{lock, Hub, Item, JoinError, Popped, RxParams, RxSlot, Stream};
use crate::p1;
use crate::p2;
use crate::plan;
use crate::profile::Profile;
use crate::stream1::Stream1;
use crate::stream2::Stream2;
use sdrkit_core::net::{NetError, Transport};
use sdrkit_core::{Hz, Results, Values};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::{Capabilities, Doing, DriverError, RateLimits, CENTRE};
use sdrkit_graph::stream::SampleType;
use std::mem::MaybeUninit;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often the read loop of a receiver wakes to look for a stop.
pub const WAKE: Duration = Duration::from_millis(100);

/// What a receiver's source shares with its radio across reopens.
#[derive(Default)]
pub struct RxShared {
    /// The stream it takes samples from now.
    pub current: Mutex<Option<Arc<dyn Stream>>>,
    /// When its stream was lost.
    pub lost_at: Mutex<Option<Instant>>,
}

/// Where a receiver is: profile, address, number.
#[derive(Clone)]
pub struct Place {
    /// The profile.
    pub p: Profile,
    /// The host.
    pub host: String,
    /// The port.
    pub port: u16,
    /// `host:port`.
    pub label: String,
    /// The receiver number, from 1.
    pub n: usize,
}

/// The capabilities of a receiver on the board `board`, if known.
pub fn capabilities(pl: &Place, board: Option<u8>) -> Capabilities {
    let p = &pl.p;
    let rates = p.rates(board);
    Capabilities {
        name: format!("{} RX{} {}", p.name, pl.n, pl.label),
        rates: RateLimits {
            min_hz: rates.first().copied().map_or(0.0, Hz::from),
            max_hz: rates.last().copied().map_or(0.0, Hz::from),
            ranges: rates.iter().map(|&r| (Hz::from(r), Hz::from(r))).collect(),
            steady_hz: None,
        },
        min_freq_hz: p.min_freq_hz,
        max_freq_hz: p.max_freq_hz,
        gain_db: p.gain_range(board),
        bandwidth_hz: None,
        has_agc: false,
        transmit: Some(crate::tx::limits(p)),
        has_bias_tee: false,
        realtime: true,
        common_rates_hz: Vec::new(),
        common_bandwidths_hz: Vec::new(),
        gain_steps_db: Vec::new(),
    }
}

/// Joins receiver `pl.n` to the stream of its radio, and opens the stream when there is none or only a lost one.
///
/// # Errors
///
/// [`DriverError::Streams`] for a receiver the radio does not have, else the error of the join or the open.
pub fn join(
    pl: &Place,
    hub: &Hub,
    net: &dyn Transport,
    params: &RxParams,
    notices: &Notices,
    abort: &Arc<Abort>,
) -> Result<(Arc<dyn Stream>, Arc<RxSlot>), DriverError> {
    if let Some(k) = hub.known(&pl.label).receivers {
        if pl.n > usize::from(k) {
            return Err(DriverError::Streams {
                given: pl.n,
                has: usize::from(k),
            });
        }
    }
    let cell = hub.cell(&pl.p, &pl.label);
    let mut g = lock(&cell);
    if let Some(s) = g.as_ref().and_then(|w| w.upgrade()) {
        match s.join(pl.n, params, notices) {
            Ok((slot, _)) => return Ok((s, slot)),
            Err(JoinError::Failed(e)) => return Err(e),
            Err(JoinError::Unusable) => {}
        }
    }
    let (s, slot): (Arc<dyn Stream>, Arc<RxSlot>) = if pl.p.protocol == 1 {
        let (s, slot, _) = Stream1::open(
            pl.p, hub, net, &pl.host, pl.port, pl.n, params, notices, abort,
        )?;
        (s, slot)
    } else {
        let (s, slot, _) = Stream2::open(
            pl.p, hub, net, &pl.host, pl.port, pl.n, params, notices, abort,
        )?;
        (s, slot)
    };
    *g = Some(Arc::downgrade(&s));
    Ok((s, slot))
}

/// One open receiver.
pub struct RxRadio {
    pl: Place,
    hub: Arc<Hub>,
    stream: Option<Arc<dyn Stream>>,
    slot: Arc<RxSlot>,
    shared: Arc<RxShared>,
    centre: Hz,
}

impl RxRadio {
    /// Opens receiver `pl.n` with `values`: the opener of the runner.
    ///
    /// # Errors
    ///
    /// What [`join`] returns, and [`DriverError::Refused`] for a value the radio cannot take.
    pub fn open(
        pl: &Place,
        hub: &Arc<Hub>,
        net: &Arc<dyn Transport>,
        shared: &Arc<RxShared>,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<RxRadio, DriverError> {
        let params = RxParams::from_values(&pl.p, hub.known(&pl.label).board, values)?;
        let (stream, slot) = join(pl, hub, net.as_ref(), &params, notices, abort)?;
        let rate = stream.rate(pl.n);
        if let Some(t) = lock(&shared.lost_at).take() {
            let s = t.elapsed().as_secs_f64();
            notices.log(format!(
                "{} RX{}: back after {s:.1} s, about {} samples lost",
                pl.p.name,
                pl.n,
                (s * rate as f64).round() as u64
            ));
        }
        notices.log(format!(
            "{} RX{}: receiving from {} at {} ksps",
            pl.p.name,
            pl.n,
            pl.label,
            rate / 1000
        ));
        *lock(&shared.current) = Some(stream.clone());
        Ok(RxRadio {
            pl: pl.clone(),
            hub: hub.clone(),
            stream: Some(stream),
            slot,
            shared: shared.clone(),
            centre: values.get(&CENTRE),
        })
    }

    fn board(&self) -> Option<u8> {
        self.stream
            .as_ref()
            .and_then(|s| s.facts().board)
            .or_else(|| self.hub.known(&self.pl.label).board)
    }
}

impl Radio for RxRadio {
    type Stream = RxStream;

    fn readings(&mut self, results: &mut Results) {
        if let Some(s) = &self.stream {
            s.write_readings(self.pl.n, results);
        }
    }

    fn configure(&mut self, values: &Values, _live: bool) -> Result<String, DriverError> {
        let params = RxParams::from_values(&self.pl.p, self.board(), values)?;
        let s = self.stream.as_ref().ok_or(DriverError::NotNow {
            state: Doing::Closed,
        })?;
        let note = s.update(self.pl.n, &params)?;
        self.centre = values.get(&CENTRE);
        Ok(note)
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<RxStream, DriverError> {
        if !self.stream.as_ref().is_some_and(|s| !s.lost()) {
            return Err(NetError::TimedOut.into());
        }
        Ok(RxStream {
            slot: self.slot.clone(),
            protocol: self.pl.p.protocol,
            abort: abort.clone(),
            shared: self.shared.clone(),
        })
    }

    fn close(&mut self) {
        if let Some(s) = self.stream.take() {
            s.leave(self.pl.n);
            let mut cur = lock(&self.shared.current);
            if cur.as_ref().is_some_and(|c| Arc::ptr_eq(c, &s)) {
                *cur = None;
            }
        }
    }

    fn rate_hz(&self) -> Hz {
        self.stream
            .as_ref()
            .map(|s| Hz::from(s.rate(self.pl.n)))
            .unwrap_or(0.0)
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn capabilities(&self) -> Capabilities {
        capabilities(&self.pl, self.board())
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cs32
    }

    fn status(&self) -> String {
        self.stream
            .as_ref()
            .map(|s| s.status(self.pl.n))
            .unwrap_or_default()
    }
}

/// The read loop of a receiver: waits on its stream and writes the samples of each packet into the ring.
pub struct RxStream {
    slot: Arc<RxSlot>,
    protocol: u8,
    abort: Arc<Abort>,
    shared: Arc<RxShared>,
}

/// Writes one sample as two little-endian 32-bit words into the 8 bytes of `d`.
#[inline]
fn put(d: &mut [MaybeUninit<u8>; 8], [i, q]: [i32; 2]) {
    for (o, b) in d
        .iter_mut()
        .zip(i.to_le_bytes().into_iter().chain(q.to_le_bytes()))
    {
        o.write(b);
    }
}

impl RxStream {
    fn write(&self, sink: &mut Sink, it: &Item) {
        let bytes = it.pkt.bytes();
        let swap = self.slot.swap.load(Ordering::Relaxed);
        let factor = self.slot.factor.load(Ordering::Relaxed);
        let n = if self.protocol == 1 {
            let lay = usize::from(it.layout);
            let k = self.slot.n - 1;
            let per = p1::per_packet(lay);
            sink.write_with(per, |dst, from| {
                for (j, d) in dst.as_chunks_mut::<8>().0.iter_mut().enumerate() {
                    let [i, q] = p1::rx_sample(bytes, lay, k, from + j, swap);
                    if factor == plan::UNITY {
                        put(d, [i, q]);
                    } else {
                        put(
                            d,
                            [plan::scale_word(i, factor), plan::scale_word(q, factor)],
                        );
                    }
                }
            });
            per
        } else {
            let Some(count) = p2::ddc_count(bytes) else {
                return;
            };
            sink.write_with(count, |dst, from| {
                for (j, d) in dst.as_chunks_mut::<8>().0.iter_mut().enumerate() {
                    put(d, p2::ddc_sample(bytes, from + j, swap));
                }
            });
            count
        };
        self.slot
            .counters
            .offered
            .fetch_add(n as u64, Ordering::Relaxed);
    }
}

impl RadioStream for RxStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        self.slot.activate(true);
        let r = loop {
            if ctx.stopping() || self.abort.fired() || sink.closed() {
                break Ok(());
            }
            match self.slot.pop(WAKE) {
                Popped::Item(it) => self.write(sink, &it),
                Popped::Idle => {}
                Popped::Lost(why) => {
                    *lock(&self.shared.lost_at) = Some(Instant::now());
                    break Err(why);
                }
                Popped::Closed => break Ok(()),
            }
        };
        self.slot.activate(false);
        r
    }
}

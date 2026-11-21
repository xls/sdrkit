//! An open SDR++ server as the shared runner drives it.

use crate::conn::{Conn, Fail, Shortfall};
use crate::menu::{self, gain_target, Control, Kind, Menu, ServerSetting, SERVER_SOURCE_ID};
use crate::proto::{self, cmd, ActionValue, SampleType};
use crate::{lock, Mem, Options, SdrppError, Timing, PREFIX};
use crate::{ARRIVING, BUSY, MBIT_S, SERVER_ERROR, SERVER_RATE, SOURCE};
use sdrkit_core::net::Transport;
use sdrkit_core::Results;
use sdrkit_core::{Hz, SettingDecl, Value, Values};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::{Capabilities, DriverError, RateLimits, AGC, CENTRE, GAIN, RATE};
use sdrkit_graph::stream::SampleType as RingType;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// The receive buffer sizes requested, largest first: 10 Msps of int16 is 40 MB/s.
const RCVBUF: [usize; 3] = [16 << 20, 8 << 20, 4 << 20];
/// How long the rate may still take after the menu's answer.
const LATE_RATE: Duration = Duration::from_millis(500);
/// The gain range offered before the server's menu has been seen.
pub(crate) const DEFAULT_GAIN: (f64, f64) = (0.0, 21.0);

/// Where a server is and how the driver waits on it.
pub(crate) struct At {
    pub transport: Arc<dyn Transport>,
    pub host: String,
    pub port: u16,
    pub label: String,
    pub timing: Timing,
}

/// Connect, greeting and menu. Returns the connection with the rate and menu read (both kept in `mem`).
pub(crate) fn connect(
    at: &At,
    mem: &Mem,
    shortfall: &Arc<Shortfall>,
    abort: &Arc<Abort>,
    notices: &Notices,
) -> Result<Arc<Conn>, DriverError> {
    let (label, t) = (at.label.as_str(), &at.timing);
    let mut sock = net::connect(&at.transport, &at.host, at.port, false, t.connect, abort)?;
    net::recv_buffer(sock.as_mut(), &RCVBUF);
    let conn = Conn::new(sock, label, mem.clone(), notices.clone(), shortfall.clone())?;
    // The greeting: nothing is sent until the rate has come, so the first command does not race it.
    match conn.wait(t.greeting, t.greeting, |st| st.rate.is_some()) {
        Ok(()) | Err(Fail::NoAnswer) => {}
        Err(f) => return Err(conn.error(f)),
    }
    let target = conn
        .send_answered(cmd::GET_UI, &[])
        .map_err(|_| conn.error(Fail::Closed))?;
    if let Err(f) = conn.wait(t.answer_open, Duration::from_secs(30), |st| {
        st.acks[cmd::GET_UI as usize] >= target
    }) {
        let f = if conn.state().busy { Fail::Busy } else { f };
        conn.close(false);
        return Err(conn.error(f));
    }
    if conn.state().rate.is_none() {
        let _ = conn.wait(LATE_RATE, LATE_RATE, |st| st.rate.is_some());
    }
    Ok(conn)
}

/// One open connection to an SDR++ server.
pub(crate) struct SdrppRadio {
    conn: Arc<Conn>,
    label: String,
    mem: Mem,
    timing: Timing,
    started: bool,
    opening: bool,
    sent_type: Option<SampleType>,
    sent_freq: Option<f64>,
    sent_slider: HashMap<String, f64>,
    /// The controls of the menu at open that are settings of their own.
    server: Vec<ServerSetting>,
    /// The value last sent of each of them. It starts with the values at open.
    sent_server: Vec<Value>,
    centre: Hz,
    asked: Hz,
    closed: bool,
}

impl SdrppRadio {
    /// Opens: connect, make sure that the server has a source, log, send the compression setting.
    pub(crate) fn open(
        at: &At,
        mem: Mem,
        shortfall: Arc<Shortfall>,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<SdrppRadio, DriverError> {
        let (label, timing) = (at.label.as_str(), at.timing);
        let conn = connect(at, &mem, &shortfall, abort, notices)?;
        let (source, rate) = {
            let st = conn.state();
            (
                st.menu
                    .as_ref()
                    .and_then(|m| m.source().map(str::to_string)),
                st.rate,
            )
        };
        let Some(source) = source else {
            conn.close(false);
            return Err(DriverError::driver(SdrppError::NoSource));
        };
        notices.log(format!(
            "{PREFIX}connected to {label} (source {source}, {} Msps)",
            rate.map(sdrkit_driver_core::fmt_m)
                .unwrap_or_else(|| "unknown".into())
        ));
        {
            let mut m = lock(&mem);
            m.busy = false;
            m.last_error = None;
            m.connected = true;
        }
        let opts = Options::from_values(values);
        conn.send(cmd::SET_COMPRESSION, &[opts.compression as u8])?;
        let server = conn
            .state()
            .menu
            .as_ref()
            .map(menu::server_settings)
            .unwrap_or_default();
        let sent_server = server.iter().map(|s| s.decl.default.clone()).collect();
        Ok(SdrppRadio {
            conn,
            label: label.into(),
            mem,
            timing,
            started: false,
            opening: true,
            sent_type: None,
            sent_freq: None,
            sent_slider: HashMap::new(),
            server,
            sent_server,
            centre: values.get(&CENTRE),
            asked: values.get(&RATE),
            closed: false,
        })
    }

    fn idle(&self) -> Duration {
        if self.opening {
            self.timing.answer_open
        } else {
            self.timing.answer
        }
    }

    fn menu(&self) -> Menu {
        self.conn.state().menu.clone().unwrap_or_default()
    }

    fn stopped_only(&self, id: &str) -> bool {
        id == SERVER_SOURCE_ID || lock(&self.mem).stopped_only.contains(id)
    }

    /// Sends a UI_ACTION and waits for its answer, or (`flush`) marks a flush and sends it without waiting.
    fn action(&mut self, id: &str, v: ActionValue, flush: bool) -> Result<(), DriverError> {
        let arg = proto::ui_action_arg(id, v, true);
        if flush {
            self.conn.send_flushed(cmd::UI_ACTION, &arg)
        } else {
            let t = self.conn.send_answered(cmd::UI_ACTION, &arg)?;
            self.conn
                .wait_answer(cmd::UI_ACTION, t, self.idle())
                .map_err(|_| self.conn.dropped())
        }
    }

    /// Holds the stream stopped for this configure, once.
    fn hold(&mut self, held: &mut bool) -> Result<(), DriverError> {
        if self.started && !*held {
            self.conn.hold(true);
            self.conn.send(cmd::STOP, &[])?;
            *held = true;
        }
        Ok(())
    }

    fn configure_inner(
        &mut self,
        values: &Values,
        streaming: bool,
        held: &mut bool,
    ) -> Result<Vec<String>, DriverError> {
        let opts = Options::from_values(values);
        let mut warnings = Vec::new();
        // 1. The sample rate.
        let m = self.menu();
        if let Some(list) = m.rate_list() {
            let rates: Vec<f64> = list.entries.iter().map(|e| menu::rate_text(e)).collect();
            let pick = pick_rate(&rates, values.get(&RATE).max(0.0));
            if pick as i32 != list.selected {
                let id = list.id.clone();
                if streaming && self.stopped_only(&id) {
                    self.hold(held)?;
                }
                self.action(&id, ActionValue::Int(pick as i32), false)?;
            }
        }
        // 2. The gain.
        if !opts.server_gain && !values.get(&AGC) {
            {
                let g = values.get(&GAIN);
                let m = self.menu();
                match m.gain_plan() {
                    None => warnings.push(
                        "the server's radio has no gain control this driver can find: set the gain on the SDR++ server".into(),
                    ),
                    Some(p) if !p.drivable() => warnings.push(
                        "the gain is set by the server's AGC: turn the AGC off under the radio settings to set it here".into(),
                    ),
                    Some(p) => {
                        let flush = streaming && !*held;
                        let moves: Vec<(String, ActionValue, f64)> = p
                            .sliders
                            .iter()
                            .filter(|c| !c.disabled)
                            .filter_map(|c| {
                                let target = gain_target(c, g, p.lo, p.hi);
                                let cur = if streaming {
                                    self.sent_slider.get(&c.id).copied().unwrap_or(c.value)
                                } else {
                                    c.value
                                };
                                ((target - cur).abs() >= c.tolerance())
                                    .then(|| (c.id.clone(), slider_value(c, target), target))
                            })
                            .collect();
                        for (id, v, target) in moves {
                            self.action(&id, v, flush)?;
                            self.sent_slider.insert(id, target);
                        }
                    }
                }
            }
        }
        // 3. The controls of the server that are settings of their own.
        self.server_controls(values, streaming, held)?;
        // 4. The sample type.
        if self.sent_type != Some(opts.sample_type) {
            self.conn
                .send(cmd::SET_SAMPLE_TYPE, &[opts.sample_type as u8])?;
            self.sent_type = Some(opts.sample_type);
            self.conn.state().sample_type = opts.sample_type;
        }
        // 5. The frequency, corrected for the PPM.
        let f = corrected_frequency(values.get(&CENTRE), opts.ppm);
        if self.sent_freq != Some(f) {
            let arg = f.to_le_bytes();
            if streaming && !*held {
                self.conn.send_flushed(cmd::SET_FREQUENCY, &arg)?;
            } else {
                let t = self.conn.send_answered(cmd::SET_FREQUENCY, &arg)?;
                self.conn
                    .wait_answer(cmd::SET_FREQUENCY, t, self.idle())
                    .map_err(|_| self.conn.dropped())?;
            }
            self.sent_freq = Some(f);
        }
        self.centre = values.get(&CENTRE);
        Ok(warnings)
    }
}

impl SdrppRadio {
    /// Sends the controls of the server whose setting changed. A control that the menu does not offer, or greys out, is
    /// [`DriverError::Refused`] of its setting.
    fn server_controls(
        &mut self,
        values: &Values,
        streaming: bool,
        held: &mut bool,
    ) -> Result<(), DriverError> {
        for i in 0..self.server.len() {
            let id = crate::DESCRIPTOR.run_time_id(i);
            let Some(v) = values.value(id).filter(|v| **v != self.sent_server[i]) else {
                continue;
            };
            let Some((control, action)) = self.server[i].target.action(v) else {
                continue;
            };
            let control = control.to_owned();
            if self.menu().get(&control).is_none_or(|c| c.disabled) {
                return Err(DriverError::Refused { id });
            }
            if streaming && self.stopped_only(&control) {
                self.hold(held)?;
            }
            self.action(&control, action, false)?;
            self.sent_server[i] = v.clone();
        }
        Ok(())
    }
}

/// The value element a slider takes.
fn slider_value(c: &Control, v: f64) -> ActionValue {
    if c.kind == Kind::SliderFloat {
        ActionValue::Float(v as f32)
    } else {
        ActionValue::Int(v.round() as i32)
    }
}

/// The frequency requested from the server: max(centre, 0) / (1 + ppm x 1e-6), ppm outside -1000 to 1000 counting as 0.
pub fn corrected_frequency(centre: Hz, ppm: f64) -> f64 {
    let ppm = if (-1000.0..=1000.0).contains(&ppm) {
        ppm
    } else {
        0.0
    };
    centre.max(0.0) / (1.0 + ppm * 1e-6)
}

/// The entry for requested rate `w`: the slowest at or above w x 0.9999, else the fastest.
pub fn pick_rate(rates: &[f64], w: f64) -> usize {
    let mut best: Option<usize> = None;
    for (i, &r) in rates.iter().enumerate() {
        if r >= w * 0.9999 && best.is_none_or(|b| r < rates[b]) {
            best = Some(i);
        }
    }
    best.unwrap_or_else(|| {
        let mut f = 0;
        for (i, &r) in rates.iter().enumerate() {
            if r > rates[f] {
                f = i;
            }
        }
        f
    })
}

/// The capabilities from what is kept of a server.
pub(crate) fn capabilities(label: &str, mem: &Mem) -> Capabilities {
    let m = lock(mem);
    let source = m.menu.as_ref().and_then(|x| x.source().map(str::to_string));
    let mut rates: Vec<f64> = m
        .menu
        .as_ref()
        .map(|x| x.rates())
        .unwrap_or_default()
        .into_iter()
        .filter(|r| *r > 0.0)
        .collect();
    rates.sort_by(f64::total_cmp);
    rates.dedup();
    if rates.is_empty() {
        if let Some(r) = m.rate.filter(|r| *r > 0.0) {
            rates.push(r);
        }
    }
    let limits = match (rates.first(), rates.last()) {
        (Some(&lo), Some(&hi)) => RateLimits {
            min_hz: lo,
            max_hz: hi,
            ranges: rates.iter().map(|&r| (r, r)).collect(),
            steady_hz: None,
        },
        _ => RateLimits::default(),
    };
    let gain = m
        .menu
        .as_ref()
        .and_then(|x| x.gain_plan().map(|p| (p.lo, p.hi, p.step())))
        .unwrap_or((DEFAULT_GAIN.0, DEFAULT_GAIN.1, 1.0));
    Capabilities {
        name: match source {
            Some(s) => format!("SDR++ server {label}: {s} (network)"),
            None => format!("SDR++ server {label} (network)"),
        },
        rates: limits,
        min_freq_hz: 0.0,
        max_freq_hz: 0.0,
        gain_db: Some(gain),
        bandwidth_hz: None,
        has_agc: false,
        transmit: None,
        has_bias_tee: false,
        realtime: true,
        common_rates_hz: Vec::new(),
        common_bandwidths_hz: Vec::new(),
        gain_steps_db: Vec::new(),
    }
}

impl Radio for SdrppRadio {
    type Stream = SdrppStream;

    fn settings(&self) -> Vec<SettingDecl> {
        self.server.iter().map(|s| s.decl.clone()).collect()
    }

    fn readings(&mut self, results: &mut Results) {
        let m = lock(&self.mem);
        let mut f = results.fields_mut();
        f.opt(&SOURCE, m.menu.as_ref().and_then(Menu::source));
        f.opt(&SERVER_RATE, m.rate);
        f.opt(&MBIT_S, m.throughput.map(|t| t.mbit_s));
        f.opt(&ARRIVING, m.throughput.map(|t| t.samples_per_s));
        f.opt(&SERVER_ERROR, m.last_error.as_deref());
        f.set(&BUSY, m.busy);
    }

    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError> {
        let streaming = live && self.started;
        self.conn.state().configuring = true;
        let mut held = false;
        let res = self.configure_inner(values, streaming, &mut held);
        let res = res.and_then(|w| {
            if held {
                self.conn.hold(false);
                self.conn.send(cmd::START, &[])?;
            }
            Ok(w)
        });
        self.conn.state().configuring = false;
        let mut warnings = res?;
        let asked = values.get(&RATE);
        self.asked = asked;
        let rate = self.rate_hz();
        if asked > 0.0 && rate < 0.97 * asked {
            warnings.push(format!(
                "this radio only offers {:.2} Msps, {:.2} Msps were requested for this channel width",
                rate / 1e6,
                asked / 1e6
            ));
        }
        Ok(warnings
            .into_iter()
            .map(|w| format!("{PREFIX}{w}"))
            .collect::<Vec<_>>()
            .join("; "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<SdrppStream, DriverError> {
        self.conn.register(abort);
        if !self.started {
            let before = self.menu();
            self.conn.send(cmd::START, &[])?;
            self.started = true;
            self.conn.expect(true);
            // Learn the controls the server only takes while stopped.
            let t = self.conn.send_answered(cmd::GET_UI, &[])?;
            self.conn
                .wait_answer(cmd::GET_UI, t, self.timing.answer)
                .map_err(|_| self.conn.dropped())?;
            let after = self.menu();
            let mut learnt: Vec<String> = after
                .controls
                .iter()
                .filter(|c| c.disabled && c.kind != Kind::Text)
                .filter(|c| before.get(&c.id).is_some_and(|b| !b.disabled))
                .map(|c| c.id.clone())
                .collect();
            learnt.push(SERVER_SOURCE_ID.into());
            lock(&self.mem).stopped_only.extend(learnt);
            self.conn.expect(true);
        }
        self.opening = false;
        Ok(SdrppStream {
            conn: self.conn.clone(),
            stall: self.timing.stall,
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.conn.close(self.started);
        self.started = false;
        lock(&self.mem).connected = false;
    }

    fn rate_hz(&self) -> Hz {
        if let Some(r) = self.conn.state().rate {
            return r;
        }
        let m = lock(&self.mem);
        m.rate
            .or_else(|| {
                let l = m.menu.as_ref()?.rate_list()?;
                l.selected_entry().map(menu::rate_text)
            })
            .unwrap_or(self.asked)
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn capabilities(&self) -> Capabilities {
        capabilities(&self.label, &self.mem)
    }

    fn sample_type(&self) -> RingType {
        RingType::Cf32
    }
}

/// The read loop of one connection.
pub(crate) struct SdrppStream {
    conn: Arc<Conn>,
    stall: Duration,
}

impl RadioStream for SdrppStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        self.conn.run(sink, || ctx.stopping(), self.stall)
    }
}

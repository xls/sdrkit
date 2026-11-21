//! `dtmb.fec`: the frame decoder of the accepted profile. Per frame it weights the frame by its decision error (a frame hit by an
//! impulse or a level step is trusted less), measures the MER, and passes the data symbols through the bit chain; the packets of
//! good code words go out as a transport stream. The component is the stage `fec` of the stack.
//!
//! Standard: GB 20600-2006 clause 4.5.

use super::chain::{Counters, FrameDecoder};
use super::profile::{point_set, Carriers, Mapping, Profile, DATA, SI};
use super::si::DataFrame;
use super::{width_values, DtmbSetting, Width, BODY, WIDTH};
use sdrkit_algo_modem::Constellation;
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};

/// Seconds of signal within which a good packet keeps the transport stream locked.
const TS_LOCK_S: f64 = 0.7;

/// What `dtmb.fec` reports after each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FecState {
    /// The profile decoded.
    pub profile: Option<Profile>,
    /// The carrier form.
    pub carriers: Option<Carriers>,
    /// The decoder's counts.
    pub counters: Counters,
    /// MER of the data symbols, dB (smoothed 0.3 per frame).
    pub mer_db: Option<f64>,
    /// A good packet came out within the last 0.7 s.
    pub ts_lock: bool,
    /// The net bit rate of the profile, bit/s.
    pub payload_bps: f64,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl Item for FecState {
    const KIND: &'static str = "dtmb.fec_state";
}

/// `dtmb.fec`. Input `in`: [`DataFrame`] from `dtmb.si`. Outputs: `ts` (transport packets), `state` ([`FecState`]).
pub struct Fec {
    width: Width,
    dec: Option<FrameDecoder>,
    constellation: Option<Constellation>,
    var: Vec<f32>,
    packets: Vec<(f64, [u8; TS])>,
    ts: TsOut,
    last_good: Option<f64>,
    state: FecState,
}

impl Default for Fec {
    fn default() -> Self {
        Fec::new(Width::W8)
    }
}

impl Fec {
    /// For a channel width.
    pub fn new(width: Width) -> Fec {
        Fec {
            width,
            dec: None,
            constellation: None,
            var: vec![0.0; DATA],
            packets: Vec::with_capacity(64),
            ts: TsOut::default(),
            last_good: None,
            state: FecState::default(),
        }
    }

    /// The decision error of a frame: the mean squared distance of every third data symbol (every twelfth for 32QAM) to the
    /// nearest point.
    fn decision_error(&self, data: &[Complex32], mapping: Mapping) -> f32 {
        let Some(c) = &self.constellation else {
            return 0.0;
        };
        let step = if mapping == Mapping::Qam32 { 12 } else { 3 };
        let (mut e, mut n) = (0f32, 0usize);
        for &z in data.iter().step_by(step) {
            e += (z - c.nearest(z)).norm_sqr();
            n += 1;
        }
        e / n.max(1) as f32
    }

    /// One frame; the packets go to `out`.
    fn frame(&mut self, f: &DataFrame, time: f64, out: &mut Vec<Timed<TsPacket>>) {
        let Some(profile) = f.profile else {
            self.dec = None;
            self.constellation = None;
            self.state.profile = None;
            self.state.carriers = None;
            self.state.mer_db = None;
            return;
        };
        if f.fresh || self.dec.is_none() {
            match FrameDecoder::new(profile) {
                Ok(d) => self.dec = Some(d),
                Err(e) => {
                    self.state.log.push(format!("DTMB: no decoder: {e}"));
                    return;
                }
            }
            let cm = match profile.mapping {
                Mapping::Qam4Nr => Mapping::Qam4,
                m => m,
            };
            self.constellation = Constellation::new(point_set(cm)).ok();
            self.state.mer_db = None;
        }
        let header = f.header.unwrap_or(super::pn::Header::Pn945);
        let frame_s = (header.len() + BODY) as f64 / self.width.symbol_rate();
        self.state.profile = Some(profile);
        self.state.carriers = f.carriers;
        self.state.payload_bps = profile.net_rate(header, self.width.symbol_rate());
        self.packets.clear();
        if let Some(d) = self.dec.as_mut() {
            for _ in 0..f.lost {
                d.erase(time, &mut self.packets);
            }
        }
        match &f.body {
            Some(b) => {
                let data = &b.cells[SI..BODY];
                self.var.copy_from_slice(&b.var[SI..BODY]);
                let err = self.decision_error(data, profile.mapping);
                let mean_var = self.var.iter().sum::<f32>() / DATA as f32;
                let ratio = err / mean_var.max(1e-9);
                if ratio > 1.5 {
                    self.var.iter_mut().for_each(|v| *v *= ratio);
                }
                let mer = 10.0 * (1.0 / err.max(1e-9) as f64).log10();
                self.state.mer_db = Some(match self.state.mer_db {
                    Some(m) => m + 0.3 * (mer - m),
                    None => mer,
                });
                if let Some(d) = self.dec.as_mut() {
                    d.push(time, data, &self.var, &mut self.packets);
                }
            }
            None => {
                if let Some(d) = self.dec.as_mut() {
                    d.erase(time, &mut self.packets);
                }
            }
        }
        let dt = frame_s / profile.packets_per_frame() as f64;
        self.deliver(time, dt, out);
    }

    /// Sends the packets held to `out`, the packets of one frame `dt` apart, and takes the decoder's counts.
    fn deliver(&mut self, time: f64, dt: f64, out: &mut Vec<Timed<TsPacket>>) {
        if let Some(d) = &self.dec {
            self.state.counters = d.counters;
        }
        let (mut frame, mut k) = (f64::NAN, 0u32);
        for (t, p) in &self.packets {
            k = if *t == frame { k + 1 } else { 0 };
            frame = *t;
            self.ts.push(out, p, t + f64::from(k) * dt, false);
        }
        if !self.packets.is_empty() {
            self.last_good = Some(time);
        }
        self.state.ts_lock = self.last_good.is_some_and(|t| time - t <= TS_LOCK_S);
    }
}

impl Component for Fec {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dtmb.fec",
            vec![Port::typed::<DataFrame>("in")],
            vec![
                stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FecState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DtmbSetting::schema())
    }
    fn values(&self) -> Values {
        width_values(self.width)
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec = None;
        self.constellation = None;
        self.last_good = None;
        self.state = FecState::default();
        Ok(vec![
            Format::new(stream::kind(), 0.0),
            Format::new(Kind::typed::<FecState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.items::<DataFrame>(0).len();
        for i in 0..n {
            let (time, f) = {
                let it = &io.items::<DataFrame>(0)[i];
                (it.time, it.value.clone())
            };
            let mut out = std::mem::take(io.items_out::<TsPacket>(0));
            self.frame(&f, time, &mut out);
            *io.items_out::<TsPacket>(0) = out;
            let value = self.state.clone();
            self.state.log.clear();
            io.items_out::<FecState>(1).push(Timed { time, value });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DtmbSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DtmbSetting::Width => self.width = WIDTH.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
}

//! The test signal generators: the mesh scenes (texting, amateur, LoRaWAN), the LoRa raw scene, the LoRaWAN wide scene and the
//! APRS scene, built with the real packet formats. A scene takes its channels from the same settings as the receiver, and
//! places them at the dial the settings give (0 without one). The signal is clean: noise and the receiver's impairments come
//! from the framework's `gen_*` settings.

use crate::components::channels::{MeshPlanner, Planner, WideConfig, WidePlanner};
use crate::mesh::plan::MeshConfig;
use crate::mesh::store::HOLD_S;
use crate::mesh::types::Protocol;
use crate::proto::meshcore::{self as mc, Identity, McPacket};
use crate::proto::meshtastic::{self as mt, Data, Header, Position, Route, Routing, User};
use crate::proto::{hamlora, lorawan};
use crate::settings::{GenScene, MeshPlanSetting, WidePlanSetting};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SettingSet, SignalInfo, Values};
use sdrkit_proto_lora_phy::coding;
use sdrkit_proto_lora_phy::fsk;
use sdrkit_proto_lora_phy::modulate::TxFrame;
use sdrkit_proto_lora_phy::params::LoraParams;
use sdrkit_util_bits::protobuf::Writer;

/// Builds one cycle of a scene: (cycle number, start time, the bursts of the cycle).
pub type BuildCycle = Box<dyn FnMut(u32, f64, &mut Vec<Burst>) + Send>;

/// A frame of the raw scene: SF, bandwidth, CR, CRC, carrier offset, dB, payload.
pub type RawFrameSpec = (u8, f64, u8, bool, f64, f64, Vec<u8>);
/// A LoRa frame of a scene: time, channel, setting, payload, carrier offset.
pub type SceneLora = (f64, f64, LoraParams, Vec<u8>, f64);
/// An FSK frame of a scene: time, channel, payload.
pub type SceneFsk = (f64, f64, Vec<u8>);
/// A frame of a mesh scene: time, channel, setting, payload, level relative to the strongest node (dB), and the payload byte
/// whose lowest bit is flipped after the checksum is made (a damaged copy).
pub type MeshFrame = (f64, f64, LoraParams, Vec<u8>, f64, Option<usize>);

/// The time from the damaged copy of a text to its good copy, s: longer than a damaged text waits for a good copy, so the
/// damaged text shows first.
pub const DAMAGED_COPY_GAP_S: f64 = HOLD_S + 3.0;

/// A scene of transmitters, made in pieces of any size. `build` makes the next cycle when the last one ends. The signal is clean:
/// noise and the receiver's impairments come from the framework's `gen_*` settings.
pub struct SceneGen {
    rate: f64,
    scene: Option<Scene>,
    cycle_s: f64,
    cycle_no: u32,
    pos: u64,
    build: BuildCycle,
    pending: Vec<Burst>,
    info: SignalInfo,
    /// Stop after this many samples (0: never).
    pub limit: u64,
}

impl SceneGen {
    /// A generator at `rate` (a rate that is not positive produces silence) whose cycles of `cycle_s` are made by `build(cycle,
    /// start_s, bursts)`. `info` describes the signal for the framework's noise settings.
    pub fn new(rate: f64, cycle_s: f64, info: SignalInfo, build: BuildCycle) -> Self {
        let mut g = SceneGen {
            rate,
            scene: Scene::new(rate).ok(),
            cycle_s,
            cycle_no: 0,
            pos: 0,
            build,
            pending: Vec::new(),
            info,
            limit: 0,
        };
        g.add_cycle(0, 0.0);
        g
    }
    fn add_cycle(&mut self, cycle: u32, t0: f64) {
        self.pending.clear();
        (self.build)(cycle, t0, &mut self.pending);
        let Some(scene) = &mut self.scene else {
            return;
        };
        for b in self.pending.drain(..) {
            // A burst outside the band or before the start is left out.
            let _ = scene.add_burst(b.start_s, b.offset_hz, b.level_db, b.samples);
        }
    }
    /// Renders `seconds` of the scene into a buffer.
    pub fn render(&mut self, seconds: f64) -> Vec<Complex32> {
        let mut v = vec![Complex32::default(); (seconds * self.rate) as usize];
        self.generate(&mut v);
        v
    }
}

impl Generator for SceneGen {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut n = out.len();
        if self.limit > 0 {
            n = n.min(self.limit.saturating_sub(self.pos) as usize);
        }
        let out = &mut out[..n];
        let end_t = (self.pos + n as u64) as f64 / self.rate;
        while self.cycle_s > 0.0 && end_t > (self.cycle_no + 1) as f64 * self.cycle_s - 0.5 {
            self.cycle_no += 1;
            let t0 = self.cycle_no as f64 * self.cycle_s;
            self.add_cycle(self.cycle_no, t0);
        }
        match &mut self.scene {
            Some(scene) => {
                scene.generate(out);
            }
            None => out.fill(Complex32::default()),
        }
        self.pos += n as u64;
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}

/// A LoRa frame as a burst of a scene: `amp` is the amplitude, 1 at full level.
pub fn lora_burst(
    p: &LoraParams,
    payload: &[u8],
    start_s: f64,
    carrier_hz: f64,
    amp: f32,
    ppm: f64,
    rate: f64,
) -> Option<Burst> {
    let f = TxFrame::new(p, payload).ok()?;
    Some(
        f.at(start_s)
            .carrier(carrier_hz)
            .amplitude(amp)
            .ppm(ppm)
            .burst(rate),
    )
}

/// A LoRa frame as [`lora_burst`] gives it, with the lowest bit of payload byte `at` flipped after the checksum is made, so
/// that the checksum fails. The flipped byte and the checksum must lie in different interleaver blocks; `None` if they do
/// not, or `at` is outside the payload.
#[allow(clippy::too_many_arguments)]
pub fn damaged_burst(
    p: &LoraParams,
    payload: &[u8],
    at: usize,
    start_s: f64,
    carrier_hz: f64,
    amp: f32,
    ppm: f64,
    rate: f64,
) -> Option<Burst> {
    let mut flipped = payload.to_vec();
    *flipped.get_mut(at)? ^= 1;
    let mut symbols = coding::encode(payload, p).ok()?;
    let bad = coding::encode(&flipped, p).ok()?;
    // The symbols differ in the block of the flipped byte and in the blocks of the checksum: the frame takes the first
    // block from the flipped payload and keeps the checksum of the good one.
    let first = symbols.iter().zip(&bad).position(|(a, b)| a != b)?;
    let head = usize::from(coding::first_block(p.sf).c);
    let later = usize::from(coding::later_block(p.sf, p.cr, p.layout().ldro).c);
    let start = if first < head {
        0
    } else {
        head + (first - head) / later * later
    };
    let end = if first < head { head } else { start + later };
    if !symbols.iter().zip(&bad).skip(end).any(|(a, b)| a != b) {
        return None;
    }
    symbols[start..end].copy_from_slice(&bad[start..end]);
    Some(
        TxFrame::from_symbols(p, symbols)
            .at(start_s)
            .carrier(carrier_hz)
            .amplitude(amp)
            .ppm(ppm)
            .burst(rate),
    )
}

/// The amplitude of the strongest frame (unit power while it sends).
pub const AMP: f32 = 1.0;

fn db(x: f64) -> f32 {
    10f64.powf(x / 20.0) as f32
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Payload builders.

/// A Meshtastic channel packet on LongFast with the default key.
#[allow(clippy::too_many_arguments)]
pub fn mt_packet(
    from: u32,
    to: u32,
    id: u32,
    hop_start: u8,
    hop_limit: u8,
    want_ack: bool,
    relay: u8,
    data: &Data,
) -> Vec<u8> {
    let h = Header {
        to,
        from,
        id,
        hop_limit,
        want_ack,
        via_mqtt: false,
        hop_start,
        channel: mt::channel_hash("LongFast", &mt::DEFAULT_KEY),
        next_hop: 0,
        relay,
    };
    mt::build_channel_packet(&h, &mt::DEFAULT_KEY, &data.encode())
}

/// Device telemetry.
pub fn telemetry_device(
    time: u32,
    battery: u32,
    volts: f32,
    ch_util: f32,
    air_tx: f32,
    uptime: u32,
) -> Vec<u8> {
    let mut m_buf = Vec::new();
    let mut m = Writer::new(&mut m_buf);
    m.uint(1, battery as u64)
        .float(2, volts)
        .float(3, ch_util)
        .float(4, air_tx)
        .uint(5, uptime as u64);
    let mut w_buf = Vec::new();
    let mut w = Writer::new(&mut w_buf);
    w.fixed32(1, time).bytes(2, &m_buf);
    w_buf
}

/// Environment telemetry.
pub fn telemetry_env(time: u32, t: f32, h: f32, p: f32) -> Vec<u8> {
    let mut m_buf = Vec::new();
    let mut m = Writer::new(&mut m_buf);
    m.float(1, t).float(2, h).float(3, p);
    let mut w_buf = Vec::new();
    let mut w = Writer::new(&mut w_buf);
    w.fixed32(1, time).bytes(3, &m_buf);
    w_buf
}

fn data(port: u32, payload: Vec<u8>) -> Data {
    Data {
        port,
        payload,
        bitfield: Some(0),
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
/// A LoRaWAN data frame: MHDR, DevAddr, FCtrl, FCnt, FOpts, FPort and payload, MIC.
pub fn lw_data(
    mtype: u8,
    devaddr: u32,
    fctrl: u8,
    fcnt: u16,
    fopts: &[u8],
    port: Option<u8>,
    payload: &[u8],
    mic: u32,
) -> Vec<u8> {
    let mut b = vec![mtype << 5];
    b.extend_from_slice(&devaddr.to_le_bytes());
    b.push((fctrl & 0xF0) | (fopts.len() as u8 & 0x0F));
    b.extend_from_slice(&fcnt.to_le_bytes());
    b.extend_from_slice(fopts);
    if let Some(p) = port {
        b.push(p);
        b.extend_from_slice(payload);
    }
    b.extend_from_slice(&mic.to_be_bytes());
    b
}

/// A join request.
pub fn lw_join(join_eui: u64, dev_eui: u64, nonce: u16, mic: u32) -> Vec<u8> {
    let mut b = vec![0u8];
    b.extend_from_slice(&join_eui.to_le_bytes());
    b.extend_from_slice(&dev_eui.to_le_bytes());
    b.extend_from_slice(&nonce.to_le_bytes());
    b.extend_from_slice(&mic.to_be_bytes());
    b
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Mesh scenes.

/// What a mesh scene sends.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshScene {
    /// The scene: [`GenScene::Texting`], [`GenScene::Amateur`] or [`GenScene::Lorawan`].
    pub kind: GenScene,
    /// Protocol mask (0: all of the scene).
    pub mask: u32,
    /// The receiver's centre, Hz.
    pub centre_hz: f64,
    /// The Meshtastic LongFast slot, Hz.
    pub mt_hz: f64,
    /// The MeshCore channel, Hz, and its setting.
    pub mc: (f64, LoraParams),
    /// The LoRa APRS channel, Hz, and its setting.
    pub aprs: (f64, LoraParams),
    /// The MeshCom channel, Hz, and its setting.
    pub meshcom: (f64, LoraParams),
    /// The LoRaWAN uplink channels of the devices, Hz, and their highest SF.
    pub lw: (Vec<f64>, u8),
    /// The level of the weakest node below the strongest, dB.
    pub spread_db: f64,
    /// Vary each packet's SNR by ±1.5 dB.
    pub vary: bool,
    /// The first frame's time, s.
    pub first_s: f64,
    /// One more Meshtastic text, first as a damaged copy and [`DAMAGED_COPY_GAP_S`] later as a good copy.
    pub damaged_copy: bool,
}

impl MeshScene {
    /// The scene `kind` on the channels of `cfg`, for a receiver at the stack's own dial (0 without one) with the radio 300 kHz
    /// above, sampled at `rate`. Each protocol sends on its first channel inside the capture.
    pub fn of(kind: GenScene, cfg: &MeshConfig, rate: f64) -> Self {
        let dial = cfg.own_dial().unwrap_or(0.0);
        let centre_hz = dial + 300e3;
        let lf = mt::preset("LongFast").unwrap_or(&mt::PRESETS[0]);
        let mt_hz = mt::slot_hz(lf, &cfg.slot_band(lf, dial), "", 0, 0.0, 0.0).unwrap_or(dial);
        let first = |p: Protocol| {
            let list = cfg.channels_of(p, dial);
            list.iter()
                .find(|c| {
                    sdrkit_proto_lora_phy::channel::in_band(
                        rate,
                        c.freq_hz - centre_hz,
                        c.params.bw,
                    )
                })
                .or(list.first())
                .map(|c| (c.freq_hz, c.params.clone()))
                .unwrap_or((dial, LoraParams::default()))
        };
        let ups: Vec<lorawan::PlanChannel> = cfg
            .lorawan_channels(dial)
            .into_iter()
            .filter(|c| c.uplink && c.bw == 125e3)
            .collect();
        let mut lw: Vec<&lorawan::PlanChannel> =
            ups.iter().filter(|c| c.hz >= dial - 1.0).take(3).collect();
        if lw.is_empty() {
            lw = ups.iter().take(3).collect();
        }
        let sf_max = lw.iter().map(|c| c.sf_max).max().unwrap_or(12);
        MeshScene {
            kind,
            mask: 0,
            centre_hz,
            mt_hz,
            mc: first(Protocol::MeshCore),
            aprs: first(Protocol::LoraAprs),
            meshcom: first(Protocol::MeshCom),
            lw: (lw.iter().map(|c| c.hz).collect(), sf_max),
            spread_db: 20.0,
            vary: true,
            first_s: 0.2,
            damaged_copy: false,
        }
    }
    /// The texting scene of the `mesh` stack at its defaults: every channel at dial 0, the radio 300 kHz above, at 2 Msps.
    pub fn texting() -> Self {
        MeshScene::of(GenScene::Texting, &MeshConfig::for_submode("mesh"), 2e6)
    }
}

/// The values of `values` (in the schema of a stack) applied to `planner` (in its own schema) by key. A value the planner
/// refuses keeps its default.
fn apply_by_key<P: Planner>(planner: &mut P, schema: &sdrkit_core::Schema, values: &Values) {
    for decl in schema.settings() {
        let key = decl.field.key.as_ref();
        let (Ok(pid), Ok(sid)) = (schema.id(key), values.schema().id(key)) else {
            continue;
        };
        if let Some(v) = values.value(sid) {
            let _ = planner.apply(pid, v);
        }
    }
}

/// The channel settings of the mesh stack `sub` in the stack's values `values`, as its receiver takes them.
pub fn mesh_config(sub: &str, values: &Values) -> MeshConfig {
    let mut p = MeshPlanner::new(sub);
    apply_by_key(&mut p, &MeshPlanSetting::schema(), values);
    p.cfg
}

/// The channel settings of `lorawanwide` in the stack's values `values`, as its receiver takes them.
pub fn wide_config(values: &Values) -> WideConfig {
    let mut p = WidePlanner::default();
    apply_by_key(&mut p, &WidePlanSetting::schema(), values);
    p.cfg
}

/// The nodes of the Meshtastic part of a scene.
pub const MT_NODES: [(u32, &str, &str, u32, u32); 6] = [
    (0xa1b2c301, "Harbour Base", "HB", 43, 0),
    (0xa1b2c302, "Hill Relay", "HR", 4, 2),
    (0xa1b2c303, "Bike Tracker", "BT", 43, 0),
    (0xa1b2c304, "Garden Sensor", "GS", 9, 0),
    (0xa1b2c305, "Boat Radio", "BR", 43, 0),
    (0xa1b2c306, "Tower Station", "TS", 4, 2),
];

/// The MeshCore nodes: seed byte, name, role.
pub const MC_NODES: [(u8, &str, u8); 3] =
    [(11, "Dune Repeater", 2), (12, "Sara", 1), (13, "Omar", 1)];

/// The text of the damaged copy of the texting scene.
pub const DAMAGED_COPY_TEXT: &str = "this text came damaged first";

/// The frames of one cycle of a mesh scene.
pub fn mesh_cycle(s: &MeshScene, cycle: u32, t0: f64, rate: f64) -> Vec<MeshFrame> {
    let mut v = Vec::new();
    let mut rng = Rng::new(0xC0FFEE ^ cycle as u64);
    let fits = |f: f64, bw: f64| sdrkit_proto_lora_phy::channel::in_band(rate, f - s.centre_hz, bw);
    let all = s.mask == 0;
    let (lf_hz, mc_hz, mc_params) = (s.mt_hz, s.mc.0, s.mc.1.clone());
    let ts = 1_760_000_000 + cycle * 90;
    if s.kind == GenScene::Texting {
        let lf = LoraParams::new(11, 250e3, 5, 16, 0x2B);
        if (all || s.mask & 1 != 0) && fits(lf_hz, 250e3) {
            let mut t = t0 + s.first_s;
            let base_id = 0x1000 + cycle * 0x100;
            let mut id = base_id;
            let push = |v: &mut Vec<_>, t: &mut f64, node: usize, bytes: Vec<u8>| {
                let rel = -(node as f64) * s.spread_db / 5.0;
                let dur = lf.airtime(bytes.len());
                v.push((*t, lf_hz, lf.clone(), bytes, rel, None));
                *t += dur + 0.15;
            };
            for (k, &(num, long, short, hw, role)) in MT_NODES.iter().enumerate() {
                let u = User {
                    id: mt::node_name(num),
                    long_name: long.into(),
                    short_name: short.into(),
                    hw_model: hw,
                    role,
                    ..Default::default()
                };
                id += 1;
                push(
                    &mut v,
                    &mut t,
                    k,
                    mt_packet(
                        num,
                        mt::BROADCAST,
                        id,
                        3,
                        3,
                        false,
                        num as u8,
                        &data(4, u.encode()),
                    ),
                );
                let lat = 52.3702 + 0.01 * k as f64;
                let lon = 4.8952 + 0.01 * k as f64;
                id += 1;
                push(
                    &mut v,
                    &mut t,
                    k,
                    mt_packet(
                        num,
                        mt::BROADCAST,
                        id,
                        3,
                        3,
                        false,
                        num as u8,
                        &data(3, Position::encode(lat, lon, 10 + k as i32, ts, 32, true)),
                    ),
                );
                id += 1;
                push(
                    &mut v,
                    &mut t,
                    k,
                    mt_packet(
                        num,
                        mt::BROADCAST,
                        id,
                        3,
                        3,
                        false,
                        num as u8,
                        &data(
                            67,
                            telemetry_device(ts, 60 + 5 * k as u32, 3.9, 12.5, 1.25, 86400),
                        ),
                    ),
                );
                id += 1;
                push(
                    &mut v,
                    &mut t,
                    k,
                    mt_packet(
                        num,
                        mt::BROADCAST,
                        id,
                        3,
                        3,
                        false,
                        num as u8,
                        &data(67, telemetry_env(ts, 21.5 + k as f32, 48.0, 1005.5)),
                    ),
                );
            }
            let n = |k: usize| MT_NODES[k].0;
            let chat = [
                (0usize, "hello from sdrkit", 0u32, 3u8, None::<usize>),
                (1, "hello back, all fine here", 1, 3, None),
                (2, "relayed reply", 0, 2, Some(1)),
                (3, "good morning mesh", 0, 3, None),
            ];
            let first_text = id + 1;
            for (k, text, reply, hop_limit, relay) in chat {
                id += 1;
                let mut d = data(1, text.as_bytes().to_vec());
                if reply > 0 {
                    d.reply_id = first_text;
                }
                let relay_byte = relay.map_or(n(k) as u8, |r| n(r) as u8);
                push(
                    &mut v,
                    &mut t,
                    k,
                    mt_packet(n(k), mt::BROADCAST, id, 3, hop_limit, false, relay_byte, &d),
                );
            }
            // A direct text with want-ack and its routing ack.
            id += 1;
            let dm = id;
            push(
                &mut v,
                &mut t,
                0,
                mt_packet(
                    n(0),
                    n(4),
                    dm,
                    3,
                    3,
                    true,
                    n(0) as u8,
                    &data(1, b"ping".to_vec()),
                ),
            );
            id += 1;
            let mut ack = data(5, Routing::encode_error(0));
            ack.request_id = dm;
            push(
                &mut v,
                &mut t,
                4,
                mt_packet(n(4), n(0), id, 3, 3, false, n(4) as u8, &ack),
            );
            // A traceroute and its reply.
            id += 1;
            let tr = id;
            let mut req = data(70, Route::default().encode());
            req.want_response = true;
            push(
                &mut v,
                &mut t,
                0,
                mt_packet(n(0), n(5), tr, 3, 3, false, n(0) as u8, &req),
            );
            id += 1;
            let mut rep = data(
                70,
                Route {
                    route: vec![n(1)],
                    snr_towards: vec![-8, 10],
                    ..Default::default()
                }
                .encode(),
            );
            rep.request_id = tr;
            push(
                &mut v,
                &mut t,
                5,
                mt_packet(n(5), n(0), id, 3, 2, false, n(1) as u8, &rep),
            );
            if s.damaged_copy {
                // One channel text, first with a bit of its text flipped, then the good copy after the wait for one.
                id += 1;
                let bytes = mt_packet(
                    n(3),
                    mt::BROADCAST,
                    id,
                    3,
                    3,
                    false,
                    n(3) as u8,
                    &data(1, DAMAGED_COPY_TEXT.as_bytes().to_vec()),
                );
                // The first byte of the text: after the header of 16 bytes and the tags of the port and the payload.
                let at = 20;
                let rel = -3.0 * s.spread_db / 5.0;
                v.push((t, lf_hz, lf.clone(), bytes.clone(), rel, Some(at)));
                v.push((t + DAMAGED_COPY_GAP_S, lf_hz, lf.clone(), bytes, rel, None));
            }
        }
        if (all || s.mask & 2 != 0) && fits(mc_hz, mc_params.bw) {
            // After the Meshtastic frames: a MeshCore channel can lie inside the LongFast channel.
            let mut t = v
                .iter()
                .map(|f| f.0 + f.2.airtime(f.3.len()) + 0.2)
                .fold(t0 + s.first_s + 0.1, f64::max);
            let ids: Vec<Identity> = MC_NODES
                .iter()
                .map(|n| Identity::from_seed(&[n.0; 32]))
                .collect();
            let push = |v: &mut Vec<_>, t: &mut f64, k: usize, bytes: Vec<u8>| {
                let rel = -(k as f64) * s.spread_db / 4.0;
                let dur = mc_params.airtime(bytes.len());
                v.push((*t, mc_hz, mc_params.clone(), bytes, rel, None));
                *t += dur + 0.2;
            };
            for (k, &(_, name, role)) in MC_NODES.iter().enumerate() {
                let a = mc::build_advert(
                    &ids[k],
                    ts,
                    role,
                    Some((52.36 + 0.01 * k as f64, 4.88 + 0.01 * k as f64)),
                    name,
                );
                let p = McPacket {
                    route: 1,
                    ptype: 4,
                    version: 0,
                    hash_size: 1,
                    path: Vec::new(),
                    payload: a,
                };
                push(&mut v, &mut t, k, p.encode());
            }
            let texts = [
                (1usize, "hello mesh from Sara", false),
                (2, "hi Sara, Omar here", false),
                (1, "the weather is fine", true),
                (2, "see you at the harbour", false),
                (0, "repeater online", false),
                (1, "meeting at noon", false),
                (2, "noted, thanks", false),
            ];
            for (i, (k, text, relayed)) in texts.iter().enumerate() {
                let path = if *relayed {
                    vec![vec![ids[0].hash()]]
                } else {
                    Vec::new()
                };
                if let Some(b) = mc::build_group_text(
                    &mc::PUBLIC_SECRET,
                    ts + 10 + i as u32,
                    MC_NODES[*k].1,
                    text,
                    path,
                ) {
                    push(&mut v, &mut t, *k, b);
                }
            }
        }
    }
    if s.kind == GenScene::Amateur {
        let (aprs_hz, la) = (s.aprs.0, s.aprs.1.clone());
        let (mcom_hz, mp) = (s.meshcom.0, s.meshcom.1.clone());
        let mut aprs_end = t0 + s.first_s;
        if (all || s.mask & 4 != 0) && fits(aprs_hz, la.bw) {
            let mut t = t0 + s.first_s;
            let pk = [
                "N0CALL-7>APLRT1,WIDE1-1:!5222.21N/00453.71E>sdrkit tracker 12.6V",
                "N0GATE-10>APLRIG:!5221.00N/00452.00EL LoRa iGate",
                "N0WX-3>APLRT1,N0CALL-10*,WIDE1*:@092345z5220.00N/00450.00E_220/004g005t077r000p000P000h50b09900",
                "N0CALL-7>APLRT1::N0CALL-9 :hello from sdrkit{12",
                "N0CALL-7>APLRT1:;HARBOUR  *092345z5222.50N/00454.00E/marina object",
                "N0GATE-10>APLRIG:>Abu Dhabi, 35°C",
                "N0TLM-1>APLRT1:T#005,199,000,255,073,123,01101001",
            ];
            for (i, p) in pk.iter().enumerate() {
                let b = hamlora::build_lora_aprs(p);
                let dur = la.airtime(b.len());
                v.push((t, aprs_hz, la.clone(), b, -(i as f64) * 2.0, None));
                t += dur + 0.2;
            }
            aprs_end = t;
        }
        if (all || s.mask & 8 != 0) && fits(mcom_hz, mp.bw) {
            // On the LoRa APRS channel, after its frames.
            let mut t = if (mcom_hz - aprs_hz).abs() <= 1.0 {
                aprs_end
            } else {
                t0 + s.first_s + 0.05
            };
            let pk: [(u8, u32, u8, &str, &str, &str); 5] = [
                (
                    b'!',
                    0x11223344,
                    4,
                    "N0MC-2",
                    "*",
                    "!5222.21N/00453.71E#MeshCom node /B=064 /A=000123",
                ),
                (
                    b'!',
                    0x11223345,
                    3,
                    "N0MC-3,N0MC-2",
                    "*",
                    "!5223.00N/00455.00E#relayed node /B=080",
                ),
                (b':', 0x11223346, 5, "N0MC-2", "*", ":hello from sdrkit"),
                (b':', 0x11223347, 5, "N0MC-3", "N0MC-2", ":QSL, 59 here"),
                (b':', 0x11223348, 5, "N0MC-2", "9", ":group hello"),
            ];
            for (kind, id, hops, src, dest, payload) in pk {
                let b = hamlora::build_meshcom(
                    kind,
                    id + cycle * 16,
                    hops,
                    0x10,
                    src,
                    dest,
                    &payload[1..],
                );
                let dur = mp.airtime(b.len());
                v.push((t, mcom_hz, mp.clone(), b, -3.0, None));
                t += dur + 0.2;
            }
        }
    }
    if s.kind == GenScene::Lorawan && !s.lw.0.is_empty() {
        let chans = &s.lw.0;
        let sfs: Vec<u8> = (7..=s.lw.1.max(7)).collect();
        let mut t = t0 + s.first_s;
        let devs = [0x260B1234u32, 0x48000123, 0x2C001234, 0x00000077];
        let fcnt = (cycle * 2) as u16;
        let mut frames: Vec<Vec<u8>> = Vec::new();
        for (k, &d) in devs.iter().enumerate() {
            frames.push(lw_data(
                2,
                d,
                0x80,
                100 + fcnt + k as u16,
                &[],
                Some(1),
                &[k as u8; 8],
                0xA1B2C3D4,
            ));
        }
        frames.push(lw_join(
            0x70B3D57ED0000001,
            0xA840410000000001,
            0x1234 + cycle as u16,
            0x01020304,
        ));
        frames.push(lw_join(
            0x70B3D57ED0000001,
            0x24E1240000000002,
            0x2345 + cycle as u16,
            0x05060708,
        ));
        for (i, b) in frames.into_iter().enumerate() {
            let sf = sfs[i % sfs.len()];
            let f = chans[i % chans.len()];
            let p = LoraParams::new(sf, 125e3, 5, 8, 0x34);
            if !fits(f, 125e3) {
                continue;
            }
            let dur = p.airtime(b.len());
            v.push((t, f, p, b, -(i as f64) * 2.0, None));
            t += dur + 0.15;
        }
    }
    let _ = rng.next_u32();
    v
}

/// The mesh generator: the scene of 11.1, cycling every 90 s. `seed` sets each node's crystal offset and the level of each packet.
pub fn mesh_generator(s: MeshScene, rate: f64, seed: u64) -> SceneGen {
    let sc = s.clone();
    let mut rng = Rng::new(seed);
    // Each node's crystal offset, by frequency and relative level.
    let mut offsets: std::collections::BTreeMap<(i64, i64), f64> = Default::default();
    let info = SignalInfo::new(250e3);
    SceneGen::new(
        rate,
        90.0,
        info,
        Box::new(move |cycle, t0, bursts| {
            for (t, f, p, bytes, rel, damage) in mesh_cycle(&sc, cycle, t0, rate) {
                let key = ((f / 1e3) as i64, (rel * 10.0) as i64);
                let xo = *offsets
                    .entry(key)
                    .or_insert_with(|| (rng.uniform() * 2.0 - 1.0) * 1500.0);
                let vary = if sc.vary {
                    (rng.uniform() * 2.0 - 1.0) * 1.5
                } else {
                    0.0
                };
                let amp = AMP * db(rel + vary);
                let carrier = f - sc.centre_hz + xo;
                bursts.extend(match damage {
                    Some(at) => damaged_burst(&p, &bytes, at, t, carrier, amp, 0.0, rate),
                    None => lora_burst(&p, &bytes, t, carrier, amp, 0.0, rate),
                });
            }
        }),
    )
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRa raw.

/// The nine frames of the raw scene: (SF, BW, CR, CRC, carrier offset, dB, payload, LDRO).
pub fn raw_frames() -> Vec<RawFrameSpec> {
    vec![
        (
            7,
            125e3,
            5,
            true,
            1500.0,
            0.0,
            b"sdrkit LoRa raw SF7 125k".to_vec(),
        ),
        (
            8,
            125e3,
            6,
            true,
            -2200.0,
            0.0,
            b"sdrkit raw SF8 4/6".to_vec(),
        ),
        (
            9,
            250e3,
            5,
            true,
            3100.0,
            3.0,
            b"sdrkit raw SF9 250k".to_vec(),
        ),
        (
            10,
            500e3,
            8,
            false,
            -800.0,
            6.0,
            b"SF10 500k no crc".to_vec(),
        ),
        (7, 500e3, 7, true, 400.0, 6.0, b"SF7 500k 4/7".to_vec()),
        (
            11,
            125e3,
            5,
            true,
            1200.0,
            0.0,
            br#"{"t":21.5,"h":48}"#.to_vec(),
        ),
        (12, 125e3, 5, true, -900.0, 0.0, b"sdrkit SF12".to_vec()),
        (9, 125e3, 5, true, 2500.0, 0.0, b"temp=21.5;hum=48".to_vec()),
        (
            8,
            250e3,
            5,
            true,
            -3000.0,
            3.0,
            vec![0x00, 0x01, 0xFE, 0xFF, 0x80, 0x7F, 0x55, 0xAA],
        ),
    ]
}

/// The raw scene for a receiver whose channel is at `chan_off` Hz in the input: looping, 0.15 s gaps.
pub fn raw_generator(rate: f64, chan_off: f64, sync: u8, implicit: bool) -> SceneGen {
    let frames = raw_frames();
    let cycle: f64 = if implicit {
        let p = implicit_params(sync);
        3.0 * (p.airtime(12) + 0.15) + 0.2
    } else {
        frames
            .iter()
            .map(|f| raw_params(f, sync).airtime(f.6.len()) + 0.15)
            .sum::<f64>()
            + 0.2
    };
    SceneGen::new(
        rate,
        cycle,
        SignalInfo::new(125e3),
        Box::new(move |_c, t0, bursts| {
            let mut t = t0 + 0.1;
            if implicit {
                let p = implicit_params(sync);
                for k in 0..3 {
                    let payload = format!("implicit {k:03}").into_bytes();
                    bursts.extend(lora_burst(
                        &p,
                        &payload,
                        t,
                        chan_off + 1800.0,
                        AMP,
                        0.0,
                        rate,
                    ));
                    t += p.airtime(12) + 0.15;
                }
                return;
            }
            for f in &frames {
                let p = raw_params(f, sync);
                let amp = AMP * db(f.5);
                bursts.extend(lora_burst(&p, &f.6, t, chan_off + f.4, amp, 0.0, rate));
                t += p.airtime(f.6.len()) + 0.15;
            }
        }),
    )
}

fn raw_params(f: &RawFrameSpec, sync: u8) -> LoraParams {
    let mut p = LoraParams::new(f.0, f.1, f.2, 8, sync);
    p.crc = f.3;
    p
}

/// The implicit scene's setting: SF8, 250 kHz, 4/6, CRC, 12 bytes.
pub fn implicit_params(sync: u8) -> LoraParams {
    let mut p = LoraParams::new(8, 250e3, 6, 8, sync);
    p.implicit = true;
    p.implicit_len = 12;
    p.crc = true;
    p
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRaWAN wide.

/// The frames of the wide scene on the channels of `c`, at its dial (0 without one): (time, channel, setting, payload, carrier
/// offset).
pub fn wide_frames(c: &WideConfig, cycle: u32, t0: f64) -> (Vec<SceneLora>, Vec<SceneFsk>) {
    // The uplink channels within 1.6 MHz of the scene's centre (the dial + 100 kHz), so that a 4 Msps capture holds them all.
    let dial = c.dial(None);
    let mid = dial + 100e3;
    let list: Vec<lorawan::PlanChannel> = c
        .plan_channels(dial)
        .into_iter()
        .filter(|c| c.uplink)
        .collect();
    let chans: Vec<f64> = list
        .iter()
        .filter(|c| c.bw == 125e3 && c.uplink && (c.hz - mid).abs() <= 1.6e6)
        .map(|c| c.hz)
        .collect();
    let ch = |i: usize| chans[i % chans.len().max(1)];
    let plan = c;
    let c = cycle as u16;
    // The plan's highest SF on its 125 kHz uplink channels, and whether gateways answer on them (in plans whose downlinks have
    // their own channels, outside the capture, the scene sends no downlink).
    let sf_cap = list
        .iter()
        .filter(|c| c.bw == 125e3 && c.uplink)
        .map(|c| c.sf_max)
        .max()
        .unwrap_or(12);
    let downs_here = list.iter().any(|c| c.bw == 125e3 && c.uplink && c.downlink);
    let up = |sf: u8| LoraParams::new(sf.min(sf_cap), 125e3, 5, 8, 0x34);
    let mut down = |sf: u8| {
        let mut p = LoraParams::new(sf.min(sf_cap), 125e3, 5, 8, 0x34);
        p.invert_iq = true;
        p.crc = false;
        p
    };
    let mut v = Vec::new();
    let mut t = t0 + 0.05;
    let add = |v: &mut Vec<_>, t: &mut f64, f: f64, p: LoraParams, b: Vec<u8>, cfo: f64| {
        let d = p.airtime(b.len());
        v.push((*t, f, p, b, cfo));
        *t += d + 0.02;
    };
    add(
        &mut v,
        &mut t,
        ch(5),
        up(7),
        lw_data(
            2,
            0x260B1234,
            0x80,
            100 + c,
            &[],
            Some(1),
            &[1, 2, 3, 4, 5, 6, 7, 8],
            0xA1B2C3D4,
        ),
        1300.0,
    );
    add(
        &mut v,
        &mut t,
        ch(0),
        up(11),
        lw_data(
            2,
            0x48000123,
            0x80,
            3041 + c,
            &[],
            Some(2),
            &[9; 20],
            0x0BADF00D,
        ),
        -2100.0,
    );
    add(
        &mut v,
        &mut t,
        ch(6),
        up(9),
        lw_data(
            4,
            0x260B1234,
            0xC0,
            7 + c,
            &[0x06, 0x4F, 0x0A, 0x02],
            Some(10),
            &[1, 2, 3],
            0x55AA55AA,
        ),
        600.0,
    );
    if downs_here {
        add(
            &mut v,
            &mut t,
            ch(6),
            down(9),
            lw_data(
                3,
                0x260B1234,
                0x20,
                3 + c,
                &[0x03, 0x51, 0x07, 0x00, 0x01],
                None,
                &[],
                0xDEADBEEF,
            ),
            0.0,
        );
    }
    add(
        &mut v,
        &mut t,
        ch(1),
        up(12),
        lw_join(
            0x70B3D57ED0000001,
            0x70B3D57ED0123456,
            0x4A2B + c,
            0x12345678,
        ),
        800.0,
    );
    if downs_here {
        add(
            &mut v,
            &mut t,
            ch(1),
            down(12),
            {
                let mut b = vec![0x20];
                b.extend_from_slice(&[0x5A; 16]);
                b
            },
            -400.0,
        );
    }
    add(
        &mut v,
        &mut t,
        ch(2),
        up(8),
        {
            let mut b = vec![0xE0];
            b.extend_from_slice(b"vendor frame");
            b
        },
        1100.0,
    );
    add(
        &mut v,
        &mut t,
        ch(3),
        up(7),
        lw_data(
            2,
            0x26011A2B,
            0x80,
            55 + c,
            &[],
            Some(3),
            &[7; 4],
            0x11223344,
        ),
        -500.0,
    );
    let mut out = v;
    // The 250 kHz (DR6) uplink, in a plan that has one.
    {
        let f = list.iter().find(|c| c.bw == 250e3).map(|c| c.hz);
        if let Some(f) = f {
            let p = LoraParams::new(7, 250e3, 5, 8, 0x34);
            let tt = out
                .last()
                .map(|x| x.0 + x.2.airtime(x.3.len()) + 0.05)
                .unwrap_or(t0);
            out.push((
                tt,
                f,
                p,
                lw_data(
                    2,
                    0x260B5678,
                    0x80,
                    12 + c,
                    &[],
                    Some(5),
                    &[3; 10],
                    0x99887766,
                ),
                700.0,
            ));
        }
    }
    let mut fsk_frames = Vec::new();
    if let Some(f) = Some(plan.fsk_hz).filter(|&f| f > 0.0) {
        fsk_frames.push((
            t0 + 0.5,
            f,
            lw_data(
                2,
                0x260BFFFF,
                0x80,
                1 + c,
                &[],
                Some(4),
                &[0x42; 12],
                0xCAFEBABE,
            ),
        ));
    }
    let _ = &mut down;
    (out, fsk_frames)
}

/// The wide scene's generator on the channels of `plan` for a receiver centred on `centre_hz`.
pub fn wide_generator(plan: &WideConfig, rate: f64, centre_hz: f64) -> SceneGen {
    let plan = plan.clone();
    SceneGen::new(
        rate,
        8.0,
        SignalInfo::new(125e3),
        Box::new(move |cycle, t0, bursts| {
            let (lora, fsks) = wide_frames(&plan, cycle, t0);
            for (t, f, p, b, cfo) in lora {
                if !sdrkit_proto_lora_phy::channel::in_band(rate, f - centre_hz, p.bw) {
                    continue;
                }
                bursts.extend(lora_burst(&p, &b, t, f - centre_hz + cfo, AMP, 0.0, rate));
            }
            for (t, f, b) in fsks {
                if (f - centre_hz).abs() + 100e3 > 0.45 * rate {
                    continue;
                }
                let mut bytes = Vec::new();
                if fsk::encode(&b, 0x55, 5, &mut bytes).is_err() {
                    continue;
                }
                if let Ok(iq) = fsk::modulate(&bytes, rate, true) {
                    bursts.push(Burst {
                        start_s: t,
                        offset_hz: f - centre_hz,
                        level_db: 20.0 * (AMP as f64).log10(),
                        samples: iq,
                    });
                }
            }
        }),
    )
}

// ---------------------------------------------------------------------------------------------------------------------------------
// APRS.

/// A UI frame without its FCS from text addresses. With `repeated`, the first repeater has its has-been-repeated bit set.
pub fn ui_frame(src: &str, dest: &str, path: &[&str], repeated: bool, info: &[u8]) -> Vec<u8> {
    let addr = |t: &str| sdrkit_proto_link::ax25::Address::parse(t).ok();
    let mut out = Vec::new();
    let (Some(s), Some(d)) = (addr(src), addr(dest)) else {
        return out;
    };
    let mut reps: Vec<sdrkit_proto_link::ax25::Address> =
        path.iter().filter_map(|p| addr(p)).collect();
    if repeated {
        if let Some(r) = reps.first_mut() {
            r.flag = true;
        }
    }
    let _ = sdrkit_proto_link::ax25::Frame::ui(d, s, &reps, info).and_then(|f| f.encode(&mut out));
    out
}

/// The frames of the APRS cycle: (speed, frame bytes without the FCS).
pub fn aprs_frames(option: u8) -> Vec<(u32, Vec<u8>)> {
    let ui = |src: &str, path: &[&str], info: &[u8], dest: &str| {
        ui_frame(src, dest, path, path.len() > 1, info)
    };
    let mut v = Vec::new();
    if option != 2 {
        let s = |x: &str| x.as_bytes().to_vec();
        v.push((
            1200,
            ui(
                "N0CALL-9",
                &["WIDE1-1", "WIDE2-1"],
                &s("!5222.21N/00453.71E>088/036/A=000328sdrkit AFSK test"),
                "APRS",
            ),
        ));
        v.push((
            1200,
            ui("N0CALL-8", &[], &s("=/5L!!<*e7>7P[compressed"), "APRS"),
        ));
        let mut mice = b"`zQc".to_vec();
        mice.extend_from_slice(&[0x1F, b'X', b't', b'>', b'/']);
        mice.extend_from_slice(b"mic-e test");
        v.push((1200, ui("N0CALL-7", &[], &mice, "UR2R21")));
        v.push((
            1200,
            ui(
                "N0CALL-1",
                &[],
                &s(";HARBOUR  *092345z5222.50N/00454.00E/marina object"),
                "APRS",
            ),
        ));
        v.push((
            1200,
            ui(
                "N0CALL-1",
                &["WIDE1-1"],
                &s(")AID #2!5222.00N/00452.00EA"),
                "APRS",
            ),
        ));
        v.push((
            1200,
            ui("N0CALL-1", &[], &s(":N0CALL-9 :Test message{42"), "APRS"),
        ));
        v.push((1200, ui("N0CALL-9", &[], &s(":N0CALL-1 :ack42"), "APRS")));
        v.push((
            1200,
            ui(
                "N0WX-3",
                &[],
                &s("@092345z5220.00N/00450.00E_220/004g005t077r000p000P000h50b09900"),
                "APRS",
            ),
        ));
        v.push((1200, ui("N0CALL-9", &[], &s(">on the road"), "APRS")));
    }
    if option != 1 {
        let s = |x: &str| x.as_bytes().to_vec();
        v.push((
            9600,
            ui(
                "N0DIGI-1",
                &[],
                &s("!5223.00N/00455.00E#digipeater"),
                "APRS",
            ),
        ));
        v.push((
            9600,
            ui("N0CALL-2", &[], &s(":N0CALL-3 :fast hello{7"), "APRS"),
        ));
        v.push((9600, ui("N0CALL-2", &[], &s(">high speed status"), "APRS")));
        v.push((
            9600,
            ui(
                "N0FIX-1",
                &[],
                &s("!5221.50N/00451.50E-fixed station with a long comment for the test"),
                "APRS",
            ),
        ));
    }
    v
}

/// The APRS generator: the cycle of 10.10 on one FM channel at `chan_off` Hz.
pub fn aprs_generator(rate: f64, chan_off: f64, option: u8, deviation: f64) -> SceneGen {
    let frames: Vec<(u32, Vec<Complex32>)> = aprs_frames(option)
        .into_iter()
        .filter_map(|(baud, f)| {
            Some((
                baud,
                crate::components::modulate::packet_iq(
                    &f,
                    &crate::mesh::types::AfskTx {
                        baud,
                        ..Default::default()
                    },
                    deviation,
                    rate,
                )?,
            ))
        })
        .collect();
    let cycle = frames
        .iter()
        .map(|(_, iq)| iq.len() as f64 / rate + 0.3)
        .sum::<f64>()
        + 0.2;
    SceneGen::new(
        rate,
        cycle,
        SignalInfo::new(16e3),
        Box::new(move |_c, t0, bursts| {
            let mut t = t0 + 0.1;
            for (_, iq) in &frames {
                bursts.push(Burst {
                    start_s: t,
                    offset_hz: chan_off,
                    level_db: 0.0,
                    samples: iq.clone(),
                });
                t += iq.len() as f64 / rate + 0.3;
            }
        }),
    )
}

/// Builds the generator of stack `stack` from its configuration. The settings of `cfg` follow the schema of that stack.
pub fn from_config(stack: &str, cfg: &GenConfig) -> Box<dyn Generator> {
    use crate::settings::{
        RawSync, Speeds, GEN_DAMAGED_COPY, GEN_IMPLICIT, GEN_SCENE, GEN_SPEEDS, OFFSET, SYNC,
    };
    let s = &cfg.settings;
    let rate = cfg.rate_hz;
    match stack {
        "loraraw" => {
            let sync = match s.get(&SYNC) {
                RawSync::Lorawan => 0x34,
                RawSync::Meshtastic => 0x2B,
                _ => 0x12,
            };
            Box::new(raw_generator(
                rate,
                -300e3 + s.get(&OFFSET),
                sync,
                s.get(&GEN_IMPLICIT),
            ))
        }
        "lorawanwide" => {
            let plan = wide_config(s);
            let centre = plan.dial(None) + 100e3;
            Box::new(wide_generator(&plan, rate, centre))
        }
        "aprs" => {
            let opt = match s.get(&GEN_SPEEDS) {
                Speeds::Both => 0,
                Speeds::Only1200 => 1,
                Speeds::Only9600 => 2,
            };
            Box::new(aprs_generator(rate, -50e3, opt, 3000.0))
        }
        _ => {
            let sub = cfg.submode.clone().unwrap_or_else(|| stack.to_string());
            let kind = match s.get(&GEN_SCENE) {
                GenScene::Auto => match sub.as_str() {
                    "loraaprs" | "meshcom" => GenScene::Amateur,
                    "lorawan" => GenScene::Lorawan,
                    _ => GenScene::Texting,
                },
                k => k,
            };
            let mut sc = MeshScene::of(kind, &mesh_config(&sub, s), rate);
            // Only the stacks that read Meshtastic texts declare the damaged copy.
            sc.damaged_copy =
                s.schema().id(GEN_DAMAGED_COPY.key()).is_ok() && s.get(&GEN_DAMAGED_COPY);
            Box::new(mesh_generator(sc, rate, cfg.seed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_cycle_has_enough_frames_and_none_overlap() {
        for damaged_copy in [false, true] {
            let s = MeshScene {
                damaged_copy,
                ..MeshScene::texting()
            };
            check_cycle(&mesh_cycle(&s, 0, 0.0, 2e6));
        }
    }

    #[test]
    fn the_damaged_copy_comes_before_its_good_copy() {
        let s = MeshScene {
            damaged_copy: true,
            ..MeshScene::texting()
        };
        let v = mesh_cycle(&s, 0, 0.0, 2e6);
        let bad: Vec<_> = v.iter().filter(|f| f.5.is_some()).collect();
        assert_eq!(bad.len(), 1);
        let good = v.iter().find(|f| f.5.is_none() && f.3 == bad[0].3).unwrap();
        assert_eq!(good.0 - bad[0].0, DAMAGED_COPY_GAP_S);
        assert_eq!(DAMAGED_COPY_GAP_S, 6.0);
        assert_eq!(
            v.len(),
            mesh_cycle(&MeshScene::texting(), 0, 0.0, 2e6).len() + 2
        );
        let b = bad[0];
        assert!(damaged_burst(&b.2, &b.3, b.5.unwrap(), 0.0, 0.0, 1.0, 0.0, 2e6).is_some());
        assert!(damaged_burst(&b.2, &b.3, b.3.len(), 0.0, 0.0, 1.0, 0.0, 2e6).is_none());
    }

    fn check_cycle(v: &[MeshFrame]) {
        let mt = v.iter().filter(|f| f.2.sync == 0x2B).count();
        let mcn = v.iter().filter(|f| f.2.sync == 0x12).count();
        assert!(mt >= 31, "{mt}");
        assert!(mcn >= 10, "{mcn}");
        let mut on: Vec<(f64, f64)> = v
            .iter()
            .map(|x| (x.0, x.0 + x.2.airtime(x.3.len())))
            .collect();
        on.sort_by(|a, b| a.0.total_cmp(&b.0));
        for w in on.windows(2) {
            assert!(w[1].0 >= w[0].1, "overlap at {}", w[1].0);
        }
        assert!(on.last().unwrap().1 < 85.0);
    }
}

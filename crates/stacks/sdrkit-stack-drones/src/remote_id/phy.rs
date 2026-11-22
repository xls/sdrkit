//! The receive paths of Remote ID: the Bluetooth LE channel and its advertising receiver, the Wi-Fi channel, and the DSSS and
//! OFDM receivers of `sdrkit-proto-wlan-phy`. Each path reads the Remote ID of its good packets and sends it to the drone table.

use super::messages::{from_ble_adv_data, from_wifi_frame, Transport};
use super::{plan, RidItem, RidPacket};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::meter::IntervalLevel;
use sdrkit_core::publish::field;
use sdrkit_core::{
    ConfigError, Setting, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RateRule, Timed,
};
use sdrkit_proto_wlan_phy::ble::AdvReceiver;
use sdrkit_proto_wlan_phy::dsss::DsssReceiver;
use sdrkit_proto_wlan_phy::ofdm::OfdmReceiver;
use sdrkit_proto_wlan_phy::Complex32;

/// The rate of the Bluetooth LE channel: 4 samples per bit.
pub const BLE_RATE: f64 = 4e6;
/// The rate of the Wi-Fi channel.
pub const WIFI_RATE: f64 = 20e6;
/// The width kept by the Bluetooth LE channel filter, Hz.
pub const BLE_WIDTH_HZ: f64 = 1.3e6;
/// The width kept by the Wi-Fi channel filter, Hz.
pub const WIFI_WIDTH_HZ: f64 = 17e6;

/// Which receiver a counter comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phy {
    /// Bluetooth LE 1M advertising.
    Ble,
    /// 802.11b DSSS.
    Dsss,
    /// 802.11a/g OFDM.
    Ofdm,
}

/// The counters of one receiver since it started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhyStats {
    /// The receiver.
    pub phy: Phy,
    /// Packets or frames whose CRC or FCS passed.
    pub good: u64,
    /// Packets or frames whose CRC or FCS failed.
    pub bad: u64,
    /// The mean power of the channel input over the last report period, dB relative to full scale.
    pub level_dbfs: Option<f64>,
}

/// Run the receiver.
pub const ENABLED: Setting<field::Bool> = Setting::new("enabled", "Run", "", true);

sdrkit_core::settings! {
    /// The settings of [`BleRid`] and [`WifiRid`].
    pub enum RunSetting {
        /// Run the receiver.
        Enabled = ENABLED,
    }
}

/// The error of the physical layer crate as a configuration error.
fn phy_error(e: sdrkit_proto_wlan_phy::Error) -> ConfigError {
    ConfigError::Stage(StageError::new(e))
}

/// The Bluetooth LE path (`drones.ble`): the advertising channel of the plan brought to 0 Hz at 4 Msps, then the LE 1M
/// advertising receiver. It sends a [`RidItem`] for each good packet and the counters at each report.
pub struct BleRid {
    enabled: bool,
    chan: Option<Channelizer>,
    rx: Option<AdvReceiver>,
    buf: Vec<Complex32>,
    level: IntervalLevel,
    items: Vec<Timed<RidItem>>,
}

impl Default for BleRid {
    fn default() -> Self {
        BleRid {
            enabled: true,
            chan: None,
            rx: None,
            buf: Vec::new(),
            level: IntervalLevel::default(),
            items: Vec::new(),
        }
    }
}

impl Component for BleRid {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.ble",
            vec![Port::iq("in")],
            vec![Port::typed::<RidItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(RunSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.chan = None;
        self.rx = None;
        self.level = IntervalLevel::default();
        let p = plan(f.centre_hz);
        if let (true, Some((ch, off))) = (self.enabled, p.ble) {
            let mut cfg =
                ChannelConfig::new(f.rate, off + f.signal_offset_hz(), BLE_WIDTH_HZ, BLE_RATE);
            cfg.exact_rate = true;
            self.chan = Some(Channelizer::new(cfg)?);
            let mut rx = AdvReceiver::new(4).map_err(phy_error)?;
            rx.set_channel_hint((ch != 0).then_some(ch))
                .map_err(phy_error)?;
            self.rx = Some(rx);
        }
        Ok(vec![Format::new(Kind::typed::<RidItem>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if let (Some(chan), Some(rx)) = (self.chan.as_mut(), self.rx.as_mut()) {
            self.buf.clear();
            chan.process(x, &mut self.buf);
            self.level.push(&self.buf);
            let items = &mut self.items;
            rx.process(&self.buf, |p| {
                let level = Some(p.level_dbfs);
                let time = p.sample as f64 / BLE_RATE;
                let Ok(pdu) = p.parse() else {
                    return;
                };
                let carried = pdu.data().and_then(from_ble_adv_data);
                let value = match (carried, pdu.adv_a()) {
                    (Some(c), Some(a)) => RidItem::Packet(RidPacket {
                        transport: Transport::Ble,
                        channel: p.channel,
                        address: a.text(),
                        counter: c.counter,
                        messages: c.messages,
                        payload: c.payload,
                        level_dbfs: level,
                        cfo_hz: p.offset_hz,
                        snr_db: p.snr_db,
                    }),
                    _ => RidItem::Plain(Transport::Ble),
                };
                items.push(Timed { time, value });
            });
        }
        let tick = io.tick();
        let o = io.items_out::<RidItem>(0);
        o.append(&mut self.items);
        if tick {
            if let Some(rx) = &self.rx {
                let s = rx.stats();
                o.push(Timed {
                    time: 0.0,
                    value: RidItem::Stats(PhyStats {
                        phy: Phy::Ble,
                        good: s.packets,
                        bad: s.crc_failures,
                        level_dbfs: self.level.take_dbfs(),
                    }),
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RunSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RunSetting::Enabled => self.enabled = ENABLED.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(RunSetting::Enabled, Value::Bool(self.enabled))])
    }
}

/// The Wi-Fi channel (`drones.wifi_channel`): the Wi-Fi channel of the plan brought to 0 Hz at exactly 20 Msps. A rate other
/// than 20 Msps goes through the anti-alias filters of the shared channeliser.
#[derive(Default)]
pub struct WifiChannel {
    chan: Option<Channelizer>,
}

impl Component for WifiChannel {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.wifi_channel",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(WIFI_RATE))],
        )
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let off = plan(f.centre_hz).wifi.1 + f.signal_offset_hz();
        self.chan = Some(wifi_channelizer(f.rate, off)?);
        let mut o = Format::iq(WIFI_RATE, f.centre_hz + off, f.centre_hz + off);
        o.bandwidth_hz = 20e6;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if let Some(c) = self.chan.as_mut() {
            c.process(x, io.iq_out(0));
        }
    }
}

/// The channeliser of the Wi-Fi channel: the signal at `offset_hz` brought to 0 Hz at exactly 20 Msps from input at `rate`.
/// Errors for a rate the shared channeliser refuses.
pub fn wifi_channelizer(
    rate: f64,
    offset_hz: f64,
) -> Result<Channelizer, sdrkit_algo_dsp::DspError> {
    let mut cfg = ChannelConfig::new(rate, offset_hz, WIFI_WIDTH_HZ, WIFI_RATE);
    cfg.exact_rate = true;
    Channelizer::new(cfg)
}

/// The Wi-Fi receiver of one PHY.
enum WifiRx {
    Dsss(Box<DsssReceiver>),
    Ofdm(Box<OfdmReceiver>),
}

/// A Wi-Fi path on the 20 Msps channel (`drones.wifi_dsss` or `drones.wifi_ofdm`): the receiver of `sdrkit-proto-wlan-phy`. It sends
/// a [`RidItem`] for each frame whose FCS passed and the counters at each report.
pub struct WifiRid {
    phy: Phy,
    enabled: bool,
    rx: Option<WifiRx>,
    level: IntervalLevel,
    items: Vec<Timed<RidItem>>,
}

impl WifiRid {
    /// The DSSS path.
    pub fn dsss() -> WifiRid {
        WifiRid::new(Phy::Dsss)
    }
    /// The OFDM path.
    pub fn ofdm() -> WifiRid {
        WifiRid::new(Phy::Ofdm)
    }
    fn new(phy: Phy) -> WifiRid {
        WifiRid {
            phy,
            enabled: true,
            rx: None,
            level: IntervalLevel::default(),
            items: Vec::new(),
        }
    }
}

fn wifi_item(mpdu: &[u8], level_dbfs: Option<f64>, cfo_hz: f64, snr_db: f64) -> RidItem {
    match from_wifi_frame(mpdu) {
        Some((address, c)) => RidItem::Packet(RidPacket {
            transport: Transport::WiFi,
            channel: 0,
            address,
            counter: c.counter,
            messages: c.messages,
            payload: c.payload,
            level_dbfs,
            cfo_hz,
            snr_db,
        }),
        None => RidItem::Plain(Transport::WiFi),
    }
}

impl Component for WifiRid {
    fn info(&self) -> ComponentInfo {
        let name = match self.phy {
            Phy::Ofdm => "drones.wifi_ofdm",
            _ => "drones.wifi_dsss",
        };
        ComponentInfo::processor(
            name,
            vec![Port::iq("in").rate(RateRule::Exact(WIFI_RATE))],
            vec![Port::typed::<RidItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(RunSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if (f.rate - WIFI_RATE).abs() > 1.0 {
            return Err(ConfigError::WrongRate {
                need: WIFI_RATE,
                got: f.rate,
            });
        }
        self.level = IntervalLevel::default();
        self.rx = match (self.enabled, self.phy) {
            (false, _) => None,
            (true, Phy::Ofdm) => Some(WifiRx::Ofdm(Box::new(OfdmReceiver::new()))),
            (true, _) => Some(WifiRx::Dsss(Box::new(
                DsssReceiver::new(20).map_err(phy_error)?,
            ))),
        };
        Ok(vec![Format::new(Kind::typed::<RidItem>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if self.rx.is_some() {
            self.level.push(x);
        }
        let items = &mut self.items;
        match self.rx.as_mut() {
            Some(WifiRx::Dsss(rx)) => {
                rx.process(x, |fr| {
                    let level = Some(fr.level_dbfs);
                    items.push(Timed {
                        time: fr.sample as f64 / WIFI_RATE,
                        value: wifi_item(fr.mpdu, level, fr.offset_hz, fr.snr_db),
                    });
                });
            }
            Some(WifiRx::Ofdm(rx)) => {
                rx.process(x, |fr| {
                    let level = Some(fr.level_dbfs);
                    items.push(Timed {
                        time: fr.sample as f64 / WIFI_RATE,
                        value: wifi_item(fr.mpdu, level, fr.offset_hz, fr.snr_db),
                    });
                });
            }
            None => {}
        }
        let tick = io.tick();
        let o = io.items_out::<RidItem>(0);
        o.append(&mut self.items);
        if tick {
            let counts = match &self.rx {
                Some(WifiRx::Dsss(rx)) => {
                    let s = rx.stats();
                    Some((s.frames, s.bad_fcs))
                }
                Some(WifiRx::Ofdm(rx)) => {
                    let s = rx.stats();
                    Some((s.frames, s.bad_fcs))
                }
                None => None,
            };
            if let Some((good, bad)) = counts {
                o.push(Timed {
                    time: 0.0,
                    value: RidItem::Stats(PhyStats {
                        phy: self.phy,
                        good,
                        bad,
                        level_dbfs: self.level.take_dbfs(),
                    }),
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RunSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RunSetting::Enabled => self.enabled = ENABLED.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(RunSetting::Enabled, Value::Bool(self.enabled))])
    }
}

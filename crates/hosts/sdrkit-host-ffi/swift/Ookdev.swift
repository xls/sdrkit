// sdrkit: the typed interface of the shared interface "ookdev", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum OokdevInterface {
    public static let major = 1
    public static let minor = 0
}

/// The count of `ookdev.pulses`.
public struct OokdevPulsesFields {
    /// Pulse trains made.
    public var trains: Int64

    init(_ c: sdrkit_ookdev_pulses_fields) {
        trains = c.trains
    }
}

/// The counts of `ookdev.burst_pulses`.
public struct OokdevBurstPulsesFields {
    /// Pulse trains made.
    public var trains: Int64
    /// Bursts that made no train.
    public var dropped: Int64

    init(_ c: sdrkit_ookdev_burst_pulses_fields) {
        trains = c.trains
        dropped = c.dropped
    }
}

/// The counts of `ookdev.decode`, and of a decoder of a stack that offers trains to a protocol list.
public struct OokdevDecodeFields {
    /// Readings the protocols made.
    public var readings: Int64
    /// Trains no protocol decoded.
    public var unknownTrains: Int64

    init(_ c: sdrkit_ookdev_decode_fields) {
        readings = c.readings
        unknownTrains = c.unknown_trains
    }
}

/// The fields of an `ookdev.reading` event. `source` is the model and the id, `text` the measurements as [`Reading::line`] returns them. A measurement that the device does not give has no value.
public struct OokdevReadingEvent {
    public enum Family: Int32 {
        case aircraft = 0
        case vessel = 1
        case aprs = 2
        case pager = 3
        case sonde = 4
        case broadcast = 5
        case voice = 6
        case message = 7
        case sensor = 8
        case satellite = 9
        case receiver = 10
        case alert = 11
        case other = 12
    }
    public enum Check: Int32 {
        case crc = 0
        case checksum = 1
        case parity = 2
        case `repeat` = 3
        case manchester = 4
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The model and the id.
    public var source: String
    /// The measurements as one line.
    public var text: String
    /// The channel, if the device sends one.
    public var channel: String?
    /// The family of the event.
    public var family: Family?
    /// The model of the device.
    public var model: String
    /// The device kind.
    public var deviceKind: String
    /// The device id as the protocol shows it.
    public var id: String
    /// The battery state: yes for good, no for low.
    public var batteryOk: Bool?
    /// The frame test that accepted the frame.
    public var check: Check?
    /// The frequency of the signal from the centre of the stream.
    public var offsetHz: Double
    /// The signal level.
    public var levelDbfs: Double
    /// The signal-to-noise ratio.
    public var snrDb: Double
    /// Air temperature.
    public var temperatureC: Double?
    /// The temperature of the first probe.
    public var temperature1C: Double?
    /// The temperature of the second probe.
    public var temperature2C: Double?
    /// Black globe temperature.
    public var globeTemperatureC: Double?
    /// Relative humidity.
    public var humidityPct: Double?
    /// Absolute humidity: grams of water in a cubic metre of air.
    public var absoluteHumidityGM3: Double?
    /// Soil moisture.
    public var moisturePct: Double?
    /// Pressure (air or tyre). A decoder that receives inches of mercury multiplies them by [`KPA_PER_INHG`](crate::reading::measure::KPA_PER_INHG).
    public var pressureKpa: Double?
    /// Mean wind speed.
    public var windSpeedMS: Double?
    /// Wind gust speed.
    public var windGustMS: Double?
    /// Wind direction, degrees clockwise from north.
    public var windDirDeg: Double?
    /// Rain since the device started counting.
    public var rainMm: Double?
    /// UV index.
    public var uvIndex: Double?
    /// Light.
    public var lightLux: Double?
    /// Battery charge.
    public var batteryLevelPct: Double?
    /// A fill or dimmer level.
    public var levelPct: Double?
    /// Power of the first channel.
    public var power0W: Double?
    /// Power of the second channel.
    public var power1W: Double?
    /// Power of the third channel.
    public var power2W: Double?
    /// Power. A decoder that receives British thermal units (IT) per hour multiplies them by [`W_PER_BTU_IT_H`](crate::reading::measure::W_PER_BTU_IT_H).
    public var powerW: Double?
    /// Current.
    public var currentA: Double?
    /// Voltage.
    public var voltageV: Double?
    /// Energy that a meter counted.
    public var energyKwh: Double?
    /// Gas volume that a meter counted.
    public var gasVolumeM3: Double?
    /// Water volume that a meter counted.
    public var waterVolumeM3: Double?
    /// Luminance as a share of the full scale of the sensor.
    public var luminancePct: Double?
    /// Carbon dioxide.
    public var co2Ppm: Double?
    /// Formaldehyde.
    public var hchoPpb: Double?
    /// Particles up to 2.5 µm.
    public var pm25UgM3: Double?
    /// Particles up to 10 µm.
    public var pm10UgM3: Double?
    /// A meter reading or a sensor value without a unit.
    public var value: Double?
    /// The time between two reports.
    public var interval: Double?
    /// The time between two wake-ups.
    public var wakeUpInterval: Double?
    /// Consumption counter of a meter.
    public var consumption: Int64?
    /// The consumption counter of the last interval.
    public var lastConsumption: Int64?
    /// The number of intervals counted.
    public var intervalCount: Int64?
    /// A message counter.
    public var counter: Int64?
    /// A sequence number.
    public var sequence: Int64?
    /// The repeat number of a transmission.
    public var `repeat`: Int64?
    /// The time offset of the transmission.
    public var transmitOffset: Int64?
    /// A level step without a unit.
    public var level: Int64?
    /// Volatile organic compounds, as a level step.
    public var vocLevel: Int64?
    /// An alarm code.
    public var alarm: Int64?
    /// An alarm type code.
    public var alarmType: Int64?
    /// An alarm level.
    public var alarmLevel: Int64?
    /// A status code.
    public var status: Int64?
    /// Status bits.
    public var flags: Int64?
    /// An event code.
    public var event: Int64?
    /// A heartbeat code.
    public var heartbeat: Int64?
    /// The type of the sensor.
    public var sensorType: Int64?
    /// The type of the packet.
    public var packetType: Int64?
    /// The ERT (encoder receiver transmitter) type of a meter.
    public var ertType: Int64?
    /// The protocol version of the device.
    public var version: Int64?
    /// The application version of the device.
    public var appVersion: Int64?
    /// Battery bits that a device sends without a known meaning.
    public var batteryBits: Int64?
    /// The node that sent a frame of a network.
    public var sourceNode: Int64?
    /// The length of a frame.
    public var lengthBytes: Int64?
    /// A battery was put in.
    public var newBattery: Bool?
    /// The device is learning a code.
    public var learning: Bool?
    /// The sender requests an acknowledgement.
    public var ackRequested: Bool?
    /// The device moves.
    public var moving: Bool?
    /// The case was opened.
    public var tamper: Bool?
    /// The device was tampered with physically.
    public var physicalTamper: Bool?
    /// The encoder was tampered with.
    public var encoderTamper: Bool?
    /// A door or window is open.
    public var doorOpen: Bool?
    /// A switch is on.
    public var switchOn: Bool?
    /// A state as the device names it ("open", "alarm").
    public var state: String?
    /// The state of a three-state code.
    public var triState: String?
    /// An operating mode.
    public var mode: String?
    /// The type of the device or the message.
    public var type: String?
    /// A button.
    public var button: String?
    /// A code that a remote sends.
    public var code: String?
    /// A command.
    public var command: String?
    /// The command class of a home network command.
    public var commandClass: String?
    /// The manufacturer.
    public var manufacturer: String?
    /// The id of a home network.
    public var homeId: String?
    /// The receiver of a frame of a network.
    public var destination: String?
    /// The kind of a frame.
    public var frame: String?
    /// The C field of a meter bus frame.
    public var cField: String?

    init(_ c: sdrkit_ookdev_reading_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        family = Family(rawValue: c.family)
        model = sdrkitText(c.model)
        deviceKind = sdrkitText(c.device_kind)
        id = sdrkitText(c.id)
        batteryOk = c.has_battery_ok ? c.battery_ok : nil
        check = Check(rawValue: c.check)
        offsetHz = c.offset_hz
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        temperatureC = c.has_temperature_c ? c.temperature_c : nil
        temperature1C = c.has_temperature_1_c ? c.temperature_1_c : nil
        temperature2C = c.has_temperature_2_c ? c.temperature_2_c : nil
        globeTemperatureC = c.has_globe_temperature_c ? c.globe_temperature_c : nil
        humidityPct = c.has_humidity_pct ? c.humidity_pct : nil
        absoluteHumidityGM3 = c.has_absolute_humidity_g_m3 ? c.absolute_humidity_g_m3 : nil
        moisturePct = c.has_moisture_pct ? c.moisture_pct : nil
        pressureKpa = c.has_pressure_kpa ? c.pressure_kpa : nil
        windSpeedMS = c.has_wind_speed_m_s ? c.wind_speed_m_s : nil
        windGustMS = c.has_wind_gust_m_s ? c.wind_gust_m_s : nil
        windDirDeg = c.has_wind_dir_deg ? c.wind_dir_deg : nil
        rainMm = c.has_rain_mm ? c.rain_mm : nil
        uvIndex = c.has_uv_index ? c.uv_index : nil
        lightLux = c.has_light_lux ? c.light_lux : nil
        batteryLevelPct = c.has_battery_level_pct ? c.battery_level_pct : nil
        levelPct = c.has_level_pct ? c.level_pct : nil
        power0W = c.has_power_0_w ? c.power_0_w : nil
        power1W = c.has_power_1_w ? c.power_1_w : nil
        power2W = c.has_power_2_w ? c.power_2_w : nil
        powerW = c.has_power_w ? c.power_w : nil
        currentA = c.has_current_a ? c.current_a : nil
        voltageV = c.has_voltage_v ? c.voltage_v : nil
        energyKwh = c.has_energy_kwh ? c.energy_kwh : nil
        gasVolumeM3 = c.has_gas_volume_m3 ? c.gas_volume_m3 : nil
        waterVolumeM3 = c.has_water_volume_m3 ? c.water_volume_m3 : nil
        luminancePct = c.has_luminance_pct ? c.luminance_pct : nil
        co2Ppm = c.has_co2_ppm ? c.co2_ppm : nil
        hchoPpb = c.has_hcho_ppb ? c.hcho_ppb : nil
        pm25UgM3 = c.has_pm2_5_ug_m3 ? c.pm2_5_ug_m3 : nil
        pm10UgM3 = c.has_pm10_ug_m3 ? c.pm10_ug_m3 : nil
        value = c.has_value ? c.value : nil
        interval = c.has_interval ? c.interval : nil
        wakeUpInterval = c.has_wake_up_interval ? c.wake_up_interval : nil
        consumption = c.has_consumption ? c.consumption : nil
        lastConsumption = c.has_last_consumption ? c.last_consumption : nil
        intervalCount = c.has_interval_count ? c.interval_count : nil
        counter = c.has_counter ? c.counter : nil
        sequence = c.has_sequence ? c.sequence : nil
        `repeat` = c.has_repeat ? c.repeat : nil
        transmitOffset = c.has_transmit_offset ? c.transmit_offset : nil
        level = c.has_level ? c.level : nil
        vocLevel = c.has_voc_level ? c.voc_level : nil
        alarm = c.has_alarm ? c.alarm : nil
        alarmType = c.has_alarm_type ? c.alarm_type : nil
        alarmLevel = c.has_alarm_level ? c.alarm_level : nil
        status = c.has_status ? c.status : nil
        flags = c.has_flags ? c.flags : nil
        event = c.has_event ? c.event : nil
        heartbeat = c.has_heartbeat ? c.heartbeat : nil
        sensorType = c.has_sensor_type ? c.sensor_type : nil
        packetType = c.has_packet_type ? c.packet_type : nil
        ertType = c.has_ert_type ? c.ert_type : nil
        version = c.has_version ? c.version : nil
        appVersion = c.has_app_version ? c.app_version : nil
        batteryBits = c.has_battery_bits ? c.battery_bits : nil
        sourceNode = c.has_source_node ? c.source_node : nil
        lengthBytes = c.has_length_bytes ? c.length_bytes : nil
        newBattery = c.has_new_battery ? c.new_battery : nil
        learning = c.has_learning ? c.learning : nil
        ackRequested = c.has_ack_requested ? c.ack_requested : nil
        moving = c.has_moving ? c.moving : nil
        tamper = c.has_tamper ? c.tamper : nil
        physicalTamper = c.has_physical_tamper ? c.physical_tamper : nil
        encoderTamper = c.has_encoder_tamper ? c.encoder_tamper : nil
        doorOpen = c.has_door_open ? c.door_open : nil
        switchOn = c.has_switch_on ? c.switch_on : nil
        state = c.has_state ? sdrkitText(c.state) : nil
        triState = c.has_tri_state ? sdrkitText(c.tri_state) : nil
        mode = c.has_mode ? sdrkitText(c.mode) : nil
        type = c.has_type ? sdrkitText(c.type) : nil
        button = c.has_button ? sdrkitText(c.button) : nil
        code = c.has_code ? sdrkitText(c.code) : nil
        command = c.has_command ? sdrkitText(c.command) : nil
        commandClass = c.has_command_class ? sdrkitText(c.command_class) : nil
        manufacturer = c.has_manufacturer ? sdrkitText(c.manufacturer) : nil
        homeId = c.has_home_id ? sdrkitText(c.home_id) : nil
        destination = c.has_destination ? sdrkitText(c.destination) : nil
        frame = c.has_frame ? sdrkitText(c.frame) : nil
        cField = c.has_c_field ? sdrkitText(c.c_field) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<OokdevReadingEvent, SDRKitCode> {
        var c = sdrkit_ookdev_reading_event()
        let code = sdrkit_ookdev_reading_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(OokdevReadingEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum OokdevTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum OokdevCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum OokdevSettings {
}

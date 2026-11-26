// sdrkit: the typed interface of the settings that every radio has, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum RadioInterface {
    public static let major = 1
    public static let minor = 0
}

/// The link and the counts of a radio.
public struct RadioFields {
    public enum State: Int32 {
        case idle = 0
        case connecting = 1
        case up = 2
        case away = 3
        case stopped = 4
    }
    /// Where the link to the radio is.
    public var state: State?
    /// The sample rate that the radio provides.
    public var rateHz: Double?
    /// The centre frequency of the samples.
    public var centreHz: Double?
    /// The samples that the radio delivered.
    public var samples: Int64
    /// The samples dropped because the engine did not keep up.
    public var dropped: Int64
    /// The samples that wait in the buffer, in percent of its size.
    public var bufferPct: Double?
    /// The samples that the radio or its link did not deliver.
    public var radioLost: Int64?
    /// The samples missed while the link was down.
    public var linkDown: Int64?
    /// The delivered rate in percent of the set rate.
    public var deliveredPct: Double?
    /// The last error of the radio.
    public var problem: String?

    init(_ c: sdrkit_radio_fields) {
        state = State(rawValue: c.state)
        rateHz = c.has_rate_hz ? c.rate_hz : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        samples = c.samples
        dropped = c.dropped
        bufferPct = c.has_buffer_pct ? c.buffer_pct : nil
        radioLost = c.has_radio_lost ? c.radio_lost : nil
        linkDown = c.has_link_down ? c.link_down : nil
        deliveredPct = c.has_delivered_pct ? c.delivered_pct : nil
        problem = c.has_problem ? sdrkitText(c.problem) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RadioFields, SDRKitCode> {
        var c = sdrkit_radio_fields()
        let code = sdrkit_radio_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RadioFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Radio connected.
public struct RadioConnectedEvent {
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
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Source.
    public var source: String?
    /// Text.
    public var text: String?
    /// Family.
    public var family: Family?

    init(_ c: sdrkit_radio_connected_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RadioConnectedEvent, SDRKitCode> {
        var c = sdrkit_radio_connected_event()
        let code = sdrkit_radio_connected_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RadioConnectedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Radio lost.
public struct RadioLostEvent {
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
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Source.
    public var source: String?
    /// Text.
    public var text: String?
    /// Family.
    public var family: Family?

    init(_ c: sdrkit_radio_lost_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RadioLostEvent, SDRKitCode> {
        var c = sdrkit_radio_lost_event()
        let code = sdrkit_radio_lost_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RadioLostEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Samples dropped.
public struct RadioOverflowEvent {
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
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Source.
    public var source: String?
    /// Text.
    public var text: String?
    /// Family.
    public var family: Family?

    init(_ c: sdrkit_radio_overflow_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RadioOverflowEvent, SDRKitCode> {
        var c = sdrkit_radio_overflow_event()
        let code = sdrkit_radio_overflow_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RadioOverflowEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Restart needed.
public struct RadioRestartEvent {
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
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Source.
    public var source: String?
    /// Text.
    public var text: String?
    /// Family.
    public var family: Family?

    init(_ c: sdrkit_radio_restart_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RadioRestartEvent, SDRKitCode> {
        var c = sdrkit_radio_restart_event()
        let code = sdrkit_radio_restart_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RadioRestartEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum RadioTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum RadioCommands {
    /// Reconnect. 
    public static func radioReconnect(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_radio_reconnect(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum RadioSettings {
    /// Centre frequency.
    public static func radioCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre_hz", value))
    }
    /// Gain.
    public static func radioGain(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gain", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioGain(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gain", value))
    }
    /// Automatic gain.
    public static func radioAgc(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "agc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioAgc(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "agc", value))
    }
    /// Sample rate.
    public static func radioRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "rate", value))
    }
    /// Bandwidth.
    public static func radioBandwidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioBandwidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "bandwidth", value))
    }
    /// Exact rate.
    public static func radioRateExact(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "rate_exact", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioRateExact(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "rate_exact", value))
    }
    /// Bias tee.
    public static func radioBiasTee(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "bias_tee", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadioBiasTee(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "bias_tee", value))
    }
}

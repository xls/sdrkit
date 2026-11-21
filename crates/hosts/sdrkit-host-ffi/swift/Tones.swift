// sdrkit: the typed interface of the shared interface "tones", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum TonesInterface {
    public static let major = 1
    public static let minor = 0
}

/// The fields of a `tones.subaudio` event: a change of a CTCSS tone or of a DCS code.
public struct TonesSubaudioEvent {
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
    public enum Kind: Int32 {
        case ctcss = 0
        case dcs = 1
    }
    public enum Event: Int32 {
        case locked = 0
        case released = 1
        case turnOff = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The kind of thing that the event is about.
    public var family: Family?
    /// One line for people.
    public var text: String
    /// The signalling.
    public var kind: Kind?
    /// The change.
    public var event: Event?
    /// The CTCSS tone.
    public var toneHz: Double?
    /// The CTCSS tone is an EIA tone.
    public var eia: Bool?
    /// The Motorola code of the CTCSS tone.
    public var motorola: String?
    /// The measured CTCSS tone.
    public var measuredHz: Double?
    /// The DCS code with its polarity ("D023N").
    public var code: String?
    /// The DCS code is a standard code.
    public var standard: Bool?
    /// A standard code that the same word also reads as.
    public var alias: String?
    /// The level of the tone, dB relative to a full-scale sine.
    public var levelDb: Double

    init(_ c: sdrkit_tones_subaudio_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        text = sdrkitText(c.text)
        kind = Kind(rawValue: c.kind)
        event = Event(rawValue: c.event)
        toneHz = c.has_tone_hz ? c.tone_hz : nil
        eia = c.has_eia ? c.eia : nil
        motorola = c.has_motorola ? sdrkitText(c.motorola) : nil
        measuredHz = c.has_measured_hz ? c.measured_hz : nil
        code = c.has_code ? sdrkitText(c.code) : nil
        standard = c.has_standard ? c.standard : nil
        alias = c.has_alias ? sdrkitText(c.alias) : nil
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TonesSubaudioEvent, SDRKitCode> {
        var c = sdrkit_tones_subaudio_event()
        let code = sdrkit_tones_subaudio_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TonesSubaudioEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a `tones.dtmf` event: a DTMF key that went down or up.
public struct TonesDtmfEvent {
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
    /// The kind of thing that the event is about.
    public var family: Family?
    /// One line for people: the key.
    public var text: String
    /// The DTMF key.
    public var key: String
    /// The key went down (or up).
    public var down: Bool
    /// The level of the row tone.
    public var rowDb: Double
    /// The level of the column tone.
    public var columnDb: Double

    init(_ c: sdrkit_tones_dtmf_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        text = sdrkitText(c.text)
        key = sdrkitText(c.key)
        down = c.down
        rowDb = c.row_db
        columnDb = c.column_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TonesDtmfEvent, SDRKitCode> {
        var c = sdrkit_tones_dtmf_event()
        let code = sdrkit_tones_dtmf_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TonesDtmfEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a `tones.selcall` event: a selcall call.
public struct TonesSelcallEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The digits of the call.
    public var source: String
    /// One line for people: the digits.
    public var text: String
    /// The digits of a call as heard.
    public var digits: String
    /// The digits as sent, with the repeat tones.
    public var sent: String
    /// The tone set the call was read as.
    public var set: String
    /// The other tone sets the call fits, separated by commas.
    public var alsoFits: String?
    /// The length of the call.
    public var lengthS: Double
    /// The mean length of a tone.
    public var toneS: Double

    init(_ c: sdrkit_tones_selcall_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        digits = sdrkitText(c.digits)
        sent = sdrkitText(c.sent)
        set = sdrkitText(c.set)
        alsoFits = c.has_also_fits ? sdrkitText(c.also_fits) : nil
        lengthS = c.length_s
        toneS = c.tone_s
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TonesSelcallEvent, SDRKitCode> {
        var c = sdrkit_tones_selcall_event()
        let code = sdrkit_tones_selcall_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TonesSelcallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a `tones.selcal` event: a SELCAL call.
public struct TonesSelcalEvent {
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
    /// The code.
    public var source: String
    /// One line for people: the code.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The SELCAL code ("AB-CD").
    public var code: String
    /// The code uses only the 16 classic tones.
    public var classic: Bool
    /// The level of the tones, dB relative to a full-scale sine.
    public var levelDb: Double
    /// The shift of the tones from their frequencies (a sideband receiver off tune).
    public var shiftHz: Double

    init(_ c: sdrkit_tones_selcal_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        code = sdrkitText(c.code)
        classic = c.classic
        levelDb = c.level_db
        shiftHz = c.shift_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TonesSelcalEvent, SDRKitCode> {
        var c = sdrkit_tones_selcal_event()
        let code = sdrkit_tones_selcal_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TonesSelcalEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a `tones.two_tone` event: a two-tone page.
public struct TonesTwoToneEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// One line for people: the two tones.
    public var text: String
    /// The frequency of tone A.
    public var aHz: Double
    /// The frequency of tone B.
    public var bHz: Double
    /// The length of tone A.
    public var aS: Double
    /// The length of tone B.
    public var bS: Double
    /// Tone B ended (it did not run past the end of the input).
    public var ended: Bool

    init(_ c: sdrkit_tones_two_tone_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        aHz = c.a_hz
        bHz = c.b_hz
        aS = c.a_s
        bS = c.b_s
        ended = c.ended
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TonesTwoToneEvent, SDRKitCode> {
        var c = sdrkit_tones_two_tone_event()
        let code = sdrkit_tones_two_tone_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TonesTwoToneEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum TonesTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum TonesCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum TonesSettings {
}

// sdrkit: the typed interface of the stack family "road", version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum RoadInterface {
    public static let major = 1
    public static let minor = 0
}

/// The result fields of V2X.
public struct V2xFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Safety messages (BSM and CAM).
    public var safetyMessages: Int64
    /// DENM frames, counted.
    public var denm: Int64
    /// Vehicles in the table.
    public var vehicles: Int64
    /// Short training fields detected.
    public var preambles: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double?
    /// The text of a decoded message.
    public var text: String
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_v2x_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        safetyMessages = c.safety_messages
        denm = c.denm
        vehicles = c.vehicles
        preambles = c.preambles
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        text = sdrkitText(c.text)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<V2xFields, SDRKitCode> {
        var c = sdrkit_v2x_fields()
        let code = sdrkit_v2x_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(V2xFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct V2xDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_v2x_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<V2xDecoderFields, SDRKitCode> {
        var c = sdrkit_v2x_decoder_fields()
        let code = sdrkit_v2x_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(V2xDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A vehicle heard. The source is the temporary id.
public struct V2xVehicleRow {
    public enum Kind: Int32 {
        case bsm = 0
        case cam = 1
        case denm = 2
        case wsmp = 3
        case geonetworking = 4
    }
    /// The row key.
    public var key: String
    /// The kind of a row.
    public var kind: Kind?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The text of a decoded message.
    public var text: String
    /// Messages from a vehicle.
    public var frames: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Wall-clock time when first heard.
    public var firstHeard: Double?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_v2x_vehicle_row) {
        key = sdrkitText(c.key)
        kind = Kind(rawValue: c.kind)
        source = sdrkitText(c.source)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        text = sdrkitText(c.text)
        frames = c.frames
        levelDbfs = c.level_dbfs
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A frame of the frame log.
public struct V2xFrameRow {
    public enum Kind: Int32 {
        case bsm = 0
        case cam = 1
        case denm = 2
        case wsmp = 3
        case geonetworking = 4
    }
    /// The row key.
    public var key: String
    /// The signal time of a frame.
    public var timeS: Double
    /// The kind of a row.
    public var kind: Kind?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The text of a decoded message.
    public var text: String
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double

    init(_ c: sdrkit_v2x_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        levelDbfs = c.level_dbfs
    }
}

/// A vehicle heard for the first time.
public struct V2xVehicleEvent {
    public enum Kind: Int32 {
        case bsm = 0
        case cam = 1
        case denm = 2
        case wsmp = 3
        case geonetworking = 4
    }
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
    /// The kind of a row.
    public var kind: Kind?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_v2x_vehicle_event) {
        time = c.time
        freqHz = c.freq_hz
        kind = Kind(rawValue: c.kind)
        source = sdrkitText(c.source)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<V2xVehicleEvent, SDRKitCode> {
        var c = sdrkit_v2x_vehicle_event()
        let code = sdrkit_v2x_vehicle_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(V2xVehicleEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum RoadTables {
    /// The changed rows of the table "vehicles" of the stack "v2x", newest first, at most `capacity` rows.
    public static func v2xVehicles(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[V2xVehicleRow], SDRKitCode> {
        var rows = [sdrkit_v2x_vehicle_row](repeating: sdrkit_v2x_vehicle_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_v2x_vehicles_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(V2xVehicleRow.init))
    }
    /// The changed rows of the table "frames" of the stack "v2x", newest first, at most `capacity` rows.
    public static func v2xFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[V2xFrameRow], SDRKitCode> {
        var rows = [sdrkit_v2x_frame_row](repeating: sdrkit_v2x_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_v2x_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(V2xFrameRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum RoadCommands {
    /// Clear. Forgets everything decoded so far.
    public static func v2xClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_v2x_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func v2xRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_v2x_vehicle_args()
        args.size = UInt32(MemoryLayout<sdrkit_v2x_vehicle_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_v2x_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum RoadSettings {
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum RoadGeneratorSettings {
    /// The options of the setting "gen_rate" of the stack "v2x".
    public enum V2xRate: Int32 {
        case n3MbitS = 0
        case n6MbitS = 1
    }
    /// Test signal data rate.
    public static func v2xRate(_ handle: SDRKIT_HANDLE) -> Result<V2xRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(V2xRate(rawValue: v) ?? V2xRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setV2xRate(_ handle: SDRKIT_HANDLE, _ value: V2xRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
}

// sdrkit: the typed interface of the settings that every record has, version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum RecordInterface {
    public static let major = 1
    public static let minor = 1
}

/// A row of the table "running" (Running recordings).
public struct RecordRunningRow {
    public enum Kind: Int32 {
        case audio = 0
        case iq = 1
        case ts = 2
    }
    public enum State: Int32 {
        case recording = 0
        case waiting = 1
        case ending = 2
        case failed = 3
    }
    public enum Problem: Int32 {
        case none = 0
        case noSpace = 1
        case notFound = 2
        case denied = 3
        case alreadyExists = 4
        case invalidPath = 5
        case io = 6
    }
    /// The row key.
    public var key: String
    /// Kind.
    public var kind: Kind?
    /// Session.
    public var session: String
    /// State.
    public var state: State?
    /// File.
    public var file: String?
    /// Files kept.
    public var files: Int64
    /// Written.
    public var frames: Int64
    /// Left out.
    public var dropped: Int64
    /// Size.
    public var bytes: Int64
    /// Problem.
    public var problem: Problem?
    /// Started.
    public var started: Double

    init(_ c: sdrkit_record_running_row) {
        key = sdrkitText(c.key)
        kind = Kind(rawValue: c.kind)
        session = sdrkitText(c.session)
        state = State(rawValue: c.state)
        file = c.has_file ? sdrkitText(c.file) : nil
        files = c.files
        frames = c.frames
        dropped = c.dropped
        bytes = c.bytes
        problem = Problem(rawValue: c.problem)
        started = c.started
    }
}

/// A row of the table "recordings" (Recordings).
public struct RecordRecordingRow {
    public enum Kind: Int32 {
        case audio = 0
        case iq = 1
        case ts = 2
    }
    public enum Container: Int32 {
        case wav = 0
        case flac = 1
        case sigmf = 2
        case raw = 3
        case ts = 4
    }
    /// The row key.
    public var key: String
    /// Kind.
    public var kind: Kind?
    /// Container.
    public var container: Container?
    /// Size.
    public var bytes: Int64
    /// Length.
    public var seconds: Double?
    /// Rate.
    public var rateHz: Double?
    /// Centre.
    public var centreHz: Double?
    /// Channels.
    public var channels: Int64
    /// Modified.
    public var modified: Double?

    init(_ c: sdrkit_record_recording_row) {
        key = sdrkitText(c.key)
        kind = Kind(rawValue: c.kind)
        container = Container(rawValue: c.container)
        bytes = c.bytes
        seconds = c.has_seconds ? c.seconds : nil
        rateHz = c.has_rate_hz ? c.rate_hz : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        channels = c.channels
        modified = c.has_modified ? c.modified : nil
    }
}

/// The reply of `audio.start`.
public struct RecordAudioStartReply {
    /// Recording.
    public var recording: String

    init(_ c: sdrkit_record_audio_start_reply) {
        recording = sdrkitText(c.recording)
    }
}

/// The reply of `iq.start`.
public struct RecordIqStartReply {
    /// Recording.
    public var recording: String

    init(_ c: sdrkit_record_iq_start_reply) {
        recording = sdrkitText(c.recording)
    }
}

/// The reply of `ts.start`.
public struct RecordTsStartReply {
    /// Recording.
    public var recording: String

    init(_ c: sdrkit_record_ts_start_reply) {
        recording = sdrkitText(c.recording)
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum RecordTables {
    /// The changed rows of the table "running" of the stack "record", newest first, at most `capacity` rows.
    public static func recordRunning(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[RecordRunningRow], SDRKitCode> {
        var rows = [sdrkit_record_running_row](repeating: sdrkit_record_running_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_record_running_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RecordRunningRow.init))
    }
    /// The changed rows of the table "recordings" of the stack "record", newest first, at most `capacity` rows.
    public static func recordRecordings(_ handle: SDRKIT_HANDLE, capacity: Int = 100000) -> Result<[RecordRecordingRow], SDRKitCode> {
        var rows = [sdrkit_record_recording_row](repeating: sdrkit_record_recording_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_record_recordings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RecordRecordingRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum RecordCommands {
    /// Record audio. Records the audio of the session with the audio settings.
    public static func recordAudioStart(_ handle: SDRKIT_HANDLE, session: String, label: String? = nil, seconds: Double? = nil) -> Result<RecordAudioStartReply, SDRKitCode> {
        var reply = sdrkit_record_audio_start_reply()
        var args = sdrkit_record_audio_start_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_audio_start_args>.size)
        let code: Int32 = sdrkitWithString(session) { session_ in sdrkitWithString(label ?? "") { label_ in { () -> Int32 in
        args.session = session_
        args.has_label = label != nil
        args.label = label_
        args.has_seconds = seconds != nil
        if let given = seconds { args.seconds = given }
        return sdrkit_record_audio_start(handle, &args, &reply)
        }() } }
        return code == SDRKIT_OK ? .success(RecordAudioStartReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Stop audio recording. 
    public static func recordAudioStop(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_record_audio_stop_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_audio_stop_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_record_audio_stop(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Record IQ. Records the samples of the session with the IQ settings.
    public static func recordIqStart(_ handle: SDRKIT_HANDLE, session: String, seconds: Double? = nil) -> Result<RecordIqStartReply, SDRKitCode> {
        var reply = sdrkit_record_iq_start_reply()
        var args = sdrkit_record_iq_start_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_iq_start_args>.size)
        let code: Int32 = sdrkitWithString(session) { session_ in { () -> Int32 in
        args.session = session_
        args.has_seconds = seconds != nil
        if let given = seconds { args.seconds = given }
        return sdrkit_record_iq_start(handle, &args, &reply)
        }() }
        return code == SDRKIT_OK ? .success(RecordIqStartReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Stop IQ recording. 
    public static func recordIqStop(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_record_iq_stop_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_iq_stop_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_record_iq_stop(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Record stream. Records the transport stream of the session: the multiplex, or one file per service.
    public static func recordTsStart(_ handle: SDRKIT_HANDLE, session: String, services: String? = nil) -> Result<RecordTsStartReply, SDRKitCode> {
        var reply = sdrkit_record_ts_start_reply()
        var args = sdrkit_record_ts_start_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_ts_start_args>.size)
        let code: Int32 = sdrkitWithString(session) { session_ in sdrkitWithString(services ?? "") { services_ in { () -> Int32 in
        args.session = session_
        args.has_services = services != nil
        args.services = services_
        return sdrkit_record_ts_start(handle, &args, &reply)
        }() } }
        return code == SDRKIT_OK ? .success(RecordTsStartReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Stop stream recording. Stops a stream recording or the time-shift buffer.
    public static func recordTsStop(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_record_ts_stop_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_ts_stop_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_record_ts_stop(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Delete recording. Deletes a recording file and the metadata beside it.
    public static func recordDelete(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_record_delete_args()
        args.size = UInt32(MemoryLayout<sdrkit_record_delete_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_record_delete(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum RecordSettings {
    /// Folder.
    public static func recordFolder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "folder", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordFolder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "folder", $0) })
    }
    /// Name prefix.
    public static func recordPrefix(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "prefix", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordPrefix(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "prefix", $0) })
    }
    /// Local time offset.
    public static func recordUtcOffsetMin(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "utc_offset_min", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordUtcOffsetMin(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "utc_offset_min", value))
    }
    /// Reserve.
    public static func recordReserveBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "reserve_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordReserveBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "reserve_bytes", value))
    }
    /// Folder budget.
    public static func recordBudgetBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "budget_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordBudgetBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "budget_bytes", value))
    }
    /// Write queue.
    public static func recordQueueBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "queue_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordQueueBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "queue_bytes", value))
    }
    /// The options of the setting "audio_container" of the stack "record".
    public enum RecordAudioContainer: Int32 {
        case wav = 0
        case flac = 1
    }
    /// Audio file.
    public static func recordAudioContainer(_ handle: SDRKIT_HANDLE) -> Result<RecordAudioContainer, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "audio_container", &v)
        return code == SDRKIT_OK ? .success(RecordAudioContainer(rawValue: v) ?? RecordAudioContainer(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordAudioContainer(_ handle: SDRKIT_HANDLE, _ value: RecordAudioContainer) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "audio_container", value.rawValue))
    }
    /// Mono.
    public static func recordMono(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mono", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordMono(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mono", value))
    }
    /// The options of the setting "split" of the stack "record".
    public enum RecordSplit: Int32 {
        case whole = 0
        case transmission = 1
    }
    /// Files.
    public static func recordSplit(_ handle: SDRKIT_HANDLE) -> Result<RecordSplit, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "split", &v)
        return code == SDRKIT_OK ? .success(RecordSplit(rawValue: v) ?? RecordSplit(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordSplit(_ handle: SDRKIT_HANDLE, _ value: RecordSplit) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "split", value.rawValue))
    }
    /// The options of the setting "gate" of the stack "record".
    public enum RecordGate: Int32 {
        case squelch = 0
        case level = 1
        case always = 2
    }
    /// Transmission gate.
    public static func recordGate(_ handle: SDRKIT_HANDLE) -> Result<RecordGate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gate", &v)
        return code == SDRKIT_OK ? .success(RecordGate(rawValue: v) ?? RecordGate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordGate(_ handle: SDRKIT_HANDLE, _ value: RecordGate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gate", value.rawValue))
    }
    /// Open level.
    public static func recordOpenDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "open_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordOpenDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "open_db", value))
    }
    /// Close level.
    public static func recordCloseDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "close_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordCloseDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "close_db", value))
    }
    /// Pre-roll.
    public static func recordPreroll(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "preroll", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordPreroll(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "preroll", value))
    }
    /// Hang.
    public static func recordHang(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordHang(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang", value))
    }
    /// Shortest kept.
    public static func recordMinLength(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_length", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordMinLength(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_length", value))
    }
    /// Longest file.
    public static func recordMaxFile(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_file", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordMaxFile(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_file", value))
    }
    /// Log of kept files.
    public static func recordLog(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "log", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordLog(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "log", value))
    }
    /// Title tag.
    public static func recordTitle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "title", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordTitle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "title", $0) })
    }
    /// Comment tag.
    public static func recordComment(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "comment", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordComment(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "comment", $0) })
    }
    /// Channel IQ.
    public static func recordChannelIq(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "channel_iq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordChannelIq(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "channel_iq", value))
    }
    /// The options of the setting "iq_container" of the stack "record".
    public enum RecordIqContainer: Int32 {
        case sigmf = 0
        case raw = 1
    }
    /// IQ file.
    public static func recordIqContainer(_ handle: SDRKIT_HANDLE) -> Result<RecordIqContainer, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "iq_container", &v)
        return code == SDRKIT_OK ? .success(RecordIqContainer(rawValue: v) ?? RecordIqContainer(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordIqContainer(_ handle: SDRKIT_HANDLE, _ value: RecordIqContainer) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "iq_container", value.rawValue))
    }
    /// The options of the setting "iq_sample" of the stack "record".
    public enum RecordIqSample: Int32 {
        case cs8 = 0
        case cs16 = 1
        case cf32 = 2
    }
    /// IQ samples.
    public static func recordIqSample(_ handle: SDRKIT_HANDLE) -> Result<RecordIqSample, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "iq_sample", &v)
        return code == SDRKIT_OK ? .success(RecordIqSample(rawValue: v) ?? RecordIqSample(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordIqSample(_ handle: SDRKIT_HANDLE, _ value: RecordIqSample) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "iq_sample", value.rawValue))
    }
    /// Leave out null packets.
    public static func recordTsDropNull(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "ts_drop_null", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordTsDropNull(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "ts_drop_null", value))
    }
    /// Largest stream file.
    public static func recordTsMaxBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "ts_max_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordTsMaxBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "ts_max_bytes", value))
    }
    /// Stream sidecar.
    public static func recordTsSidecar(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "ts_sidecar", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordTsSidecar(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "ts_sidecar", value))
    }
    /// Rewind buffer.
    public static func recordRewind(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "rewind", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordRewind(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "rewind", value))
    }
    /// Rewind memory.
    public static func recordRewindBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "rewind_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordRewindBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "rewind_bytes", value))
    }
    /// Centre history factor.
    public static func recordRewindLongFactor(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "rewind_long_factor", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordRewindLongFactor(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "rewind_long_factor", value))
    }
    /// Centre history memory.
    public static func recordRewindLongBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "rewind_long_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordRewindLongBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "rewind_long_bytes", value))
    }
    /// Time-shift size.
    public static func recordTimeshiftBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "timeshift_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordTimeshiftBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "timeshift_bytes", value))
    }
    /// Time-shift segment.
    public static func recordTimeshiftSegmentBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "timeshift_segment_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRecordTimeshiftSegmentBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "timeshift_segment_bytes", value))
    }
}

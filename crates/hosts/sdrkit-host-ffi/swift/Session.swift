// sdrkit: the typed interface of the settings that every session has, version 1.4. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum SessionInterface {
    public static let major = 1
    public static let minor = 4
}

/// The state and the counts of a session.
public struct SessionFields {
    public enum State: Int32 {
        case waiting = 0
        case running = 1
        case failed = 2
        case stopped = 3
    }
    /// Where the session is.
    public var state: State?
    /// The samples that the receiver was fed.
    public var samples: Int64
    /// The samples that the session lost: those that its queue or a stage could not take, and those that its driver lost in the gaps it marked. A gap made on purpose (a retune that flushes the radio, a step of a hop) is told to the stack and is not counted. The column `lost` of the table of connections shows the places.
    public var dropped: Int64
    /// The events of a type that the catalog of the engine does not list. They are dropped.
    public var eventsUnlisted: Int64
    /// Why the receiver refused its input, or the last setting that it refused.
    public var problem: String?
    /// Why the engine refused the last request of the receiver.
    public var requestProblem: String?
    /// The audio samples lost because the host did not take them in time.
    public var audioLost: Int64
    /// The transmit samples lost because the host did not take them in time.
    public var transmitLost: Int64
    /// The changes of frequency that the receiver has taken.
    public var retunes: Int64
    /// The bursts that the radio of the session sent.
    public var txSent: Int64?
    /// The bursts that were refused or did not reach the radio in full.
    public var txFailed: Int64?
    /// When the last burst ended.
    public var txLast: Double?
    /// What stops the session from transmitting now.
    public var txBlocked: String?
    /// The signal to noise ratio that the receiver measures.
    public var snrDb: Double?
    /// The level that the receiver measures.
    public var levelDbfs: Double?
    /// The offset of the signal from the tuned frequency that the receiver measures.
    public var offsetHz: Double?
    /// The share of one core that the stack takes now, all its threads together.
    public var share: Double?
    /// The cores that the stack needs at its rate, measured over the last second.
    public var coresNeeded: Double?
    /// The cores of this device that the recorded cost of the stack comes to.
    public var coresDeclared: Double?
    /// The cores that were free for the session when it started.
    public var coresFree: Double?
    /// The threads that run the stack.
    public var threads: Int64?
    /// The times that a stage found the queue after it full and waited.
    public var stageBehind: Int64?
    /// The places where samples are missing that the stack was told of, lost or left out on purpose.
    public var gaps: Int64?
    /// Why the session may lose samples: its stack needs more cores than are free.
    public var loadProblem: String?
    /// The calibration units per second that one fast core of this device runs.
    public var coreUnits: Double?
    /// The media packets and pictures lost because the host did not take them in time. Decoded video frames are kept for half a second.
    public var mediaLost: Int64
    /// The samples of the audio that the host gave to the session and that the graph did not take in time: the oldest go first.
    public var audioInLost: Int64
    /// The rate of the audio that the graph of the session takes from the host without conversion, such as a microphone. It has a value while the graph has a host audio input and the session can transmit.
    public var audioInRateHz: Double?
    /// True while a transmission that sends the host audio runs or push-to-talk is pressed: the host opens its microphone and gives the audio only then.
    public var audioInWanted: Bool

    init(_ c: sdrkit_session_fields) {
        state = State(rawValue: c.state)
        samples = c.samples
        dropped = c.dropped
        eventsUnlisted = c.events_unlisted
        problem = c.has_problem ? sdrkitText(c.problem) : nil
        requestProblem = c.has_request_problem ? sdrkitText(c.request_problem) : nil
        audioLost = c.audio_lost
        transmitLost = c.transmit_lost
        retunes = c.retunes
        txSent = c.has_tx_sent ? c.tx_sent : nil
        txFailed = c.has_tx_failed ? c.tx_failed : nil
        txLast = c.has_tx_last ? c.tx_last : nil
        txBlocked = c.has_tx_blocked ? sdrkitText(c.tx_blocked) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        share = c.has_share ? c.share : nil
        coresNeeded = c.has_cores_needed ? c.cores_needed : nil
        coresDeclared = c.has_cores_declared ? c.cores_declared : nil
        coresFree = c.has_cores_free ? c.cores_free : nil
        threads = c.has_threads ? c.threads : nil
        stageBehind = c.has_stage_behind ? c.stage_behind : nil
        gaps = c.has_gaps ? c.gaps : nil
        loadProblem = c.has_load_problem ? sdrkitText(c.load_problem) : nil
        coreUnits = c.has_core_units ? c.core_units : nil
        mediaLost = c.media_lost
        audioInLost = c.audio_in_lost
        audioInRateHz = c.has_audio_in_rate_hz ? c.audio_in_rate_hz : nil
        audioInWanted = c.audio_in_wanted
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SessionFields, SDRKitCode> {
        var c = sdrkit_session_fields()
        let code = sdrkit_session_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SessionFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the table "session.components" (Components).
public struct SessionComponentLoadRow {
    /// The row key.
    public var key: String
    /// Section.
    public var section: String
    /// Stage.
    public var stage: String
    /// Thread.
    public var thread: Int64
    /// Share.
    public var share: Double
    /// Share since start.
    public var shareTotal: Double
    /// Calls.
    public var calls: Int64
    /// Items in.
    public var itemsIn: Int64
    /// Items out.
    public var itemsOut: Int64
    /// Mean call.
    public var meanUs: Double
    /// Worst call.
    public var worstUs: Double
    /// Units.
    public var units: Double

    init(_ c: sdrkit_session_component_load_row) {
        key = sdrkitText(c.key)
        section = sdrkitText(c.section)
        stage = sdrkitText(c.stage)
        thread = c.thread
        share = c.share
        shareTotal = c.share_total
        calls = c.calls
        itemsIn = c.items_in
        itemsOut = c.items_out
        meanUs = c.mean_us
        worstUs = c.worst_us
        units = c.units
    }
}

/// A row of the table "session.stages" (Stages).
public struct SessionStageLoadRow {
    /// The row key.
    public var key: String
    /// Thread.
    public var thread: Int64
    /// Own thread.
    public var cut: Bool
    /// Share.
    public var share: Double
    /// Cost.
    public var cost: Double
    /// Declared cost.
    public var declared: Double?
    /// Queue fill.
    public var queueFill: Double
    /// Queue units.
    public var queueUnits: Int64
    /// Queue most.
    public var queueUnitsMax: Int64
    /// Behind.
    public var behind: Int64
    /// Units.
    public var units: Double
    /// Declared units.
    public var declaredUnits: Double?

    init(_ c: sdrkit_session_stage_load_row) {
        key = sdrkitText(c.key)
        thread = c.thread
        cut = c.cut
        share = c.share
        cost = c.cost
        declared = c.has_declared ? c.declared : nil
        queueFill = c.queue_fill
        queueUnits = c.queue_units
        queueUnitsMax = c.queue_units_max
        behind = c.behind
        units = c.units
        declaredUnits = c.has_declared_units ? c.declared_units : nil
    }
}

/// A row of the table "session.connections" (Connections).
public struct SessionConnectionLoadRow {
    /// The row key.
    public var key: String
    /// From.
    public var from: String
    /// Output.
    public var output: String
    /// To.
    public var to: String
    /// Input.
    public var input: String
    /// Format.
    public var format: String
    /// Stage.
    public var stage: String
    /// Items.
    public var itemsPerS: Double
    /// Bytes.
    public var bytesPerS: Double
    /// Held.
    public var heldBytes: Int64
    /// Held mean.
    public var heldMeanBytes: Double
    /// Held most.
    public var heldMaxBytes: Int64
    /// Capacity.
    public var capacityBytes: Int64?
    /// Fill.
    public var fill: Double?
    /// Wait.
    public var waitMeanUs: Double?
    /// Wait worst.
    public var waitWorstUs: Double?
    /// Found full.
    public var full: Int64
    /// Found empty.
    public var empty: Int64
    /// Stage boundary.
    public var boundary: Bool
    /// Cut.
    public var cut: Bool
    /// Lost.
    public var lost: Int64

    init(_ c: sdrkit_session_connection_load_row) {
        key = sdrkitText(c.key)
        from = sdrkitText(c.from)
        output = sdrkitText(c.output)
        to = sdrkitText(c.to)
        input = sdrkitText(c.input)
        format = sdrkitText(c.format)
        stage = sdrkitText(c.stage)
        itemsPerS = c.items_per_s
        bytesPerS = c.bytes_per_s
        heldBytes = c.held_bytes
        heldMeanBytes = c.held_mean_bytes
        heldMaxBytes = c.held_max_bytes
        capacityBytes = c.has_capacity_bytes ? c.capacity_bytes : nil
        fill = c.has_fill ? c.fill : nil
        waitMeanUs = c.has_wait_mean_us ? c.wait_mean_us : nil
        waitWorstUs = c.has_wait_worst_us ? c.wait_worst_us : nil
        full = c.full
        empty = c.empty
        boundary = c.boundary
        cut = c.cut
        lost = c.lost
    }
}

/// Burst.
public struct SessionTransmitEvent {
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
    /// Sent.
    public var sent: Bool
    /// Samples.
    public var samples: Int64
    /// The attenuation of the transmitter during the burst: the negative of `tx_power_db`.
    public var attenuation: Double?
    /// Attempts.
    public var attempts: Int64
    /// Refused because.
    public var refused: String?
    /// Family.
    public var family: Family?
    /// The radio setting `tx_power` during the burst: the output power relative to the full output of the radio.
    public var txPowerDb: Double?

    init(_ c: sdrkit_session_transmit_event) {
        time = c.time
        freqHz = c.freq_hz
        sent = c.sent
        samples = c.samples
        attenuation = c.has_attenuation ? c.attenuation : nil
        attempts = c.attempts
        refused = c.has_refused ? sdrkitText(c.refused) : nil
        family = Family(rawValue: c.family)
        txPowerDb = c.has_tx_power_db ? c.tx_power_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SessionTransmitEvent, SDRKitCode> {
        var c = sdrkit_session_transmit_event()
        let code = sdrkit_session_transmit_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SessionTransmitEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum SessionTables {
    /// The changed rows of the table "session.components" of the stack "session", newest first, at most `capacity` rows.
    public static func sessionComponents(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[SessionComponentLoadRow], SDRKitCode> {
        var rows = [sdrkit_session_component_load_row](repeating: sdrkit_session_component_load_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_session_components_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SessionComponentLoadRow.init))
    }
    /// The changed rows of the table "session.stages" of the stack "session", newest first, at most `capacity` rows.
    public static func sessionStages(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SessionStageLoadRow], SDRKitCode> {
        var rows = [sdrkit_session_stage_load_row](repeating: sdrkit_session_stage_load_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_session_stages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SessionStageLoadRow.init))
    }
    /// The changed rows of the table "session.connections" of the stack "session", newest first, at most `capacity` rows.
    public static func sessionConnections(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[SessionConnectionLoadRow], SDRKitCode> {
        var rows = [sdrkit_session_connection_load_row](repeating: sdrkit_session_connection_load_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_session_connections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SessionConnectionLoadRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum SessionCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum SessionSettings {
    /// Frequency.
    public static func sessionTunedHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tuned_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionTunedHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tuned_hz", value))
    }
    /// Spectrum.
    public static func sessionSpectrum(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spectrum", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionSpectrum(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spectrum", value))
    }
    /// The options of the setting "spectrum_size" of the stack "session".
    public enum SessionSpectrumSize: Int32 {
        case n256 = 0
        case n512 = 1
        case n1024 = 2
        case n2048 = 3
        case n4096 = 4
        case n8192 = 5
        case n16384 = 6
        case n32768 = 7
        case n65536 = 8
        case n131072 = 9
        case n262144 = 10
    }
    /// Spectrum size.
    public static func sessionSpectrumSize(_ handle: SDRKIT_HANDLE) -> Result<SessionSpectrumSize, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "spectrum_size", &v)
        return code == SDRKIT_OK ? .success(SessionSpectrumSize(rawValue: v) ?? SessionSpectrumSize(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionSpectrumSize(_ handle: SDRKIT_HANDLE, _ value: SessionSpectrumSize) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "spectrum_size", value.rawValue))
    }
    /// The options of the setting "spectrum_window" of the stack "session".
    public enum SessionSpectrumWindow: Int32 {
        case blackmanHarris = 0
        case hann = 1
        case rectangular = 2
    }
    /// Spectrum window.
    public static func sessionSpectrumWindow(_ handle: SDRKIT_HANDLE) -> Result<SessionSpectrumWindow, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "spectrum_window", &v)
        return code == SDRKIT_OK ? .success(SessionSpectrumWindow(rawValue: v) ?? SessionSpectrumWindow(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionSpectrumWindow(_ handle: SDRKIT_HANDLE, _ value: SessionSpectrumWindow) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "spectrum_window", value.rawValue))
    }
    /// Spectrum frames.
    public static func sessionSpectrumFps(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "spectrum_fps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionSpectrumFps(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "spectrum_fps", value))
    }
    /// Volume.
    public static func sessionVolume(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "volume", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionVolume(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "volume", value))
    }
    /// Pan.
    public static func sessionPan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionPan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pan", value))
    }
    /// Mute.
    public static func sessionMute(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mute", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionMute(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mute", value))
    }
    /// Solo.
    public static func sessionSolo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "solo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSessionSolo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "solo", value))
    }
}

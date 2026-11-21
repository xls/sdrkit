// sdrkit: the typed interface of the settings that every scan has, version 2.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum ScanInterface {
    public static let major = 2
    public static let minor = 1
}

/// The state and the progress of the scan service.
public struct ScanFields {
    public enum State: Int32 {
        case idle = 0
        case settle = 1
        case listen = 2
        case hold = 3
        case hang = 4
        case paused = 5
        case test = 6
        case done = 7
        case locked = 8
        case look = 9
    }
    public enum Kind: Int32 {
        case none = 0
        case activity = 1
        case channels = 2
        case span = 3
        case survey = 4
    }
    public enum End: Int32 {
        case none = 0
        case passesDone = 1
        case allLockedOut = 2
        case nothingToScan = 3
        case stopped = 4
        case complete = 5
        case timeOver = 6
    }
    /// Where the scan is.
    public var state: State?
    /// The kind of the scan that runs or ran last.
    public var kind: Kind?
    /// The place of the channel in the list of the scan.
    public var index: Int64?
    /// The channels of the scan.
    public var count: Int64?
    /// The frequency of the channel.
    public var freqHz: Double?
    /// The name of the channel.
    public var name: String?
    /// The stack of the channel.
    public var stack: String?
    /// The scan listens to the priority channel.
    public var atPriority: Bool?
    /// The pass over the list.
    public var pass: Int64?
    /// The channels that the scan has visited.
    public var steps: Int64?
    /// The channels with a signal so far.
    public var found: Int64?
    /// Why the scan ended.
    public var end: End?
    /// The session that the scan tunes.
    public var session: String?
    /// The rows of the scan table.
    public var total: Int64?
    /// The time until the total time of a span or survey scan is over.
    public var timeLeftS: Double?
    /// The time that one pass of the scan takes, from its plan.
    public var passS: Double?
    /// The passes that fit into the total time, from the plan of the scan.
    public var passes: Double?
    /// The candidates of a survey that wait for a visit.
    public var candidates: Int64?
    /// The value of the watched result of the stack on the block that is held.
    public var watched: Double?

    init(_ c: sdrkit_scan_fields) {
        state = State(rawValue: c.state)
        kind = Kind(rawValue: c.kind)
        index = c.has_index ? c.index : nil
        count = c.has_count ? c.count : nil
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        name = c.has_name ? sdrkitText(c.name) : nil
        stack = c.has_stack ? sdrkitText(c.stack) : nil
        atPriority = c.has_at_priority ? c.at_priority : nil
        pass = c.has_pass ? c.pass : nil
        steps = c.has_steps ? c.steps : nil
        found = c.has_found ? c.found : nil
        end = End(rawValue: c.end)
        session = c.has_session ? sdrkitText(c.session) : nil
        total = c.has_total ? c.total : nil
        timeLeftS = c.has_time_left_s ? c.time_left_s : nil
        passS = c.has_pass_s ? c.pass_s : nil
        passes = c.has_passes ? c.passes : nil
        candidates = c.has_candidates ? c.candidates : nil
        watched = c.has_watched ? c.watched : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ScanFields, SDRKitCode> {
        var c = sdrkit_scan_fields()
        let code = sdrkit_scan_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ScanFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the table "hits" (Scan hits).
public struct ScanHitRow {
    public enum Empty: Int32 {
        case noSignal = 0
        case noLock = 1
    }
    /// The row key.
    public var key: String
    /// Frequency.
    public var freqHz: Double
    /// Name.
    public var name: String
    /// Stack.
    public var stack: String
    /// Locked.
    public var locked: Bool
    /// SNR.
    public var snrDb: Double?
    /// Level.
    public var levelDbfs: Double?
    /// Start.
    public var start: Double
    /// End.
    public var endTime: Double
    /// Priority.
    public var priority: Bool?
    /// Tone.
    public var tone: String?
    /// Memory.
    public var memory: String?
    /// Channel.
    public var channel: String?
    /// Empty because.
    public var empty: Empty?
    /// Pass.
    public var pass: Int64
    /// Text.
    public var text: String

    init(_ c: sdrkit_scan_hit_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
        name = sdrkitText(c.name)
        stack = sdrkitText(c.stack)
        locked = c.locked
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        start = c.start
        endTime = c.end_time
        priority = c.has_priority ? c.priority : nil
        tone = c.has_tone ? sdrkitText(c.tone) : nil
        memory = c.has_memory ? sdrkitText(c.memory) : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        empty = c.has_empty ? Empty(rawValue: c.empty) : nil
        pass = c.pass
        text = sdrkitText(c.text)
    }
}

/// A row of the table "candidates" (Survey candidates).
public struct ScanCandidateRow {
    public enum State: Int32 {
        case waiting = 0
        case visiting = 1
        case found = 2
        case rejected = 3
    }
    /// The row key.
    public var key: String
    /// Frequency.
    public var freqHz: Double
    /// Score.
    public var score: Double
    /// Speed.
    public var wpm: Double
    /// SNR.
    public var snrDb: Double
    /// State.
    public var state: State?
    /// Pass.
    public var pass: Int64
    /// First seen.
    public var firstSeen: Double
    /// Skipped until.
    public var skipUntil: Double?

    init(_ c: sdrkit_scan_candidate_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
        score = c.score
        wpm = c.wpm
        snrDb = c.snr_db
        state = State(rawValue: c.state)
        pass = c.pass
        firstSeen = c.first_seen
        skipUntil = c.has_skip_until ? c.skip_until : nil
    }
}

/// A row of the table "occupied" (Occupied channels of a survey, 2 to 30 kHz wide; a lone carrier is not listed).
public struct ScanOccupiedRow {
    /// The row key.
    public var key: String
    /// Frequency.
    public var freqHz: Double
    /// Width.
    public var widthHz: Double
    /// SNR.
    public var snrDb: Double
    /// Pass.
    public var pass: Int64
    /// Seen.
    public var seen: Int64

    init(_ c: sdrkit_scan_occupied_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
        widthHz = c.width_hz
        snrDb = c.snr_db
        pass = c.pass
        seen = c.seen
    }
}

/// The reply of `start`.
public struct ScanStartReply {
    public enum Kind: Int32 {
        case none = 0
        case activity = 1
        case channels = 2
        case span = 3
        case survey = 4
    }
    /// Channels.
    public var channels: Int64
    /// Kind.
    public var kind: Kind?
    /// Dwell.
    public var dwellS: Double
    /// Time of a pass.
    public var passS: Double
    /// Total time.
    public var totalS: Double
    /// Passes.
    public var passes: Double
    /// Time of a visit.
    public var visitS: Double
    /// Time of a pass of span blocks.
    public var spanPassS: Double

    init(_ c: sdrkit_scan_start_reply) {
        channels = c.channels
        kind = Kind(rawValue: c.kind)
        dwellS = c.dwell_s
        passS = c.pass_s
        totalS = c.total_s
        passes = c.passes
        visitS = c.visit_s
        spanPassS = c.span_pass_s
    }
}

/// The reply of `span`.
public struct ScanSpanReply {
    public enum Kind: Int32 {
        case none = 0
        case activity = 1
        case channels = 2
        case span = 3
        case survey = 4
    }
    /// Channels.
    public var channels: Int64
    /// Kind.
    public var kind: Kind?
    /// Dwell.
    public var dwellS: Double
    /// Time of a pass.
    public var passS: Double
    /// Total time.
    public var totalS: Double
    /// Passes.
    public var passes: Double
    /// Time of a visit.
    public var visitS: Double
    /// Time of a pass of span blocks.
    public var spanPassS: Double

    init(_ c: sdrkit_scan_span_reply) {
        channels = c.channels
        kind = Kind(rawValue: c.kind)
        dwellS = c.dwell_s
        passS = c.pass_s
        totalS = c.total_s
        passes = c.passes
        visitS = c.visit_s
        spanPassS = c.span_pass_s
    }
}

/// The reply of `survey`.
public struct ScanSurveyReply {
    public enum Kind: Int32 {
        case none = 0
        case activity = 1
        case channels = 2
        case span = 3
        case survey = 4
    }
    /// Channels.
    public var channels: Int64
    /// Kind.
    public var kind: Kind?
    /// Dwell.
    public var dwellS: Double
    /// Time of a pass.
    public var passS: Double
    /// Total time.
    public var totalS: Double
    /// Passes.
    public var passes: Double
    /// Time of a visit.
    public var visitS: Double
    /// Time of a pass of span blocks.
    public var spanPassS: Double

    init(_ c: sdrkit_scan_survey_reply) {
        channels = c.channels
        kind = Kind(rawValue: c.kind)
        dwellS = c.dwell_s
        passS = c.pass_s
        totalS = c.total_s
        passes = c.passes
        visitS = c.visit_s
        spanPassS = c.span_pass_s
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum ScanTables {
    /// The changed rows of the table "hits" of the stack "scan", newest first, at most `capacity` rows.
    public static func scanHits(_ handle: SDRKIT_HANDLE, capacity: Int = 10000) -> Result<[ScanHitRow], SDRKitCode> {
        var rows = [sdrkit_scan_hit_row](repeating: sdrkit_scan_hit_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_scan_hits_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScanHitRow.init))
    }
    /// The changed rows of the table "candidates" of the stack "scan", newest first, at most `capacity` rows.
    public static func scanCandidates(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[ScanCandidateRow], SDRKitCode> {
        var rows = [sdrkit_scan_candidate_row](repeating: sdrkit_scan_candidate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_scan_candidates_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScanCandidateRow.init))
    }
    /// The changed rows of the table "occupied" of the stack "scan", newest first, at most `capacity` rows.
    public static func scanOccupied(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[ScanOccupiedRow], SDRKitCode> {
        var rows = [sdrkit_scan_occupied_row](repeating: sdrkit_scan_occupied_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_scan_occupied_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScanOccupiedRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum ScanCommands {
    /// Scan the band of the stack. Starts a channel scan of the stack, judged by its lock profile: the channels that the host gave for the stack, else one channel for each bandwidth of the range of the stack. If that range has more than 2000 channels, the scan takes the part of it that the session receives now, around its frequency.
    public static func scanStart(_ handle: SDRKIT_HANDLE, session: String, stack: String, dry: Bool? = nil) -> Result<ScanStartReply, SDRKitCode> {
        var reply = sdrkit_scan_start_reply()
        var args = sdrkit_scan_start_args()
        args.size = UInt32(MemoryLayout<sdrkit_scan_start_args>.size)
        let code: Int32 = sdrkitWithString(session) { session_ in sdrkitWithString(stack) { stack_ in { () -> Int32 in
        args.session = session_
        args.stack = stack_
        args.has_dry = dry != nil
        if let given = dry { args.dry = given }
        return sdrkit_scan_start(handle, &args, &reply)
        }() } }
        return code == SDRKIT_OK ? .success(ScanStartReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Stop scan. 
    public static func scanStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_scan_stop(handle)
        return sdrkitResult(code)
    }
    /// Scan span blocks. Starts a scan of the range from `start_hz` to `stop_hz` in blocks of `span_hz`, each held for `dwell_s` and judged by the result `watch` of the stack: a value over `above` is a find, which holds the scan (the setting after_find). The passes repeat until `total_s` is over.
    public static func scanSpan(_ handle: SDRKIT_HANDLE, session: String, startHz: Double, stopHz: Double, spanHz: Double, watch: String, above: Double? = nil, stack: String? = nil, dwellS: Double? = nil, totalS: Double? = nil, dry: Bool? = nil) -> Result<ScanSpanReply, SDRKitCode> {
        var reply = sdrkit_scan_span_reply()
        var args = sdrkit_scan_span_args()
        args.size = UInt32(MemoryLayout<sdrkit_scan_span_args>.size)
        let code: Int32 = sdrkitWithString(session) { session_ in sdrkitWithString(watch) { watch_ in sdrkitWithString(stack ?? "") { stack_ in { () -> Int32 in
        args.session = session_
        args.start_hz = startHz
        args.stop_hz = stopHz
        args.span_hz = spanHz
        args.watch = watch_
        args.has_above = above != nil
        if let given = above { args.above = given }
        args.has_stack = stack != nil
        args.stack = stack_
        args.has_dwell_s = dwellS != nil
        if let given = dwellS { args.dwell_s = given }
        args.has_total_s = totalS != nil
        if let given = totalS { args.total_s = given }
        args.has_dry = dry != nil
        if let given = dry { args.dry = given }
        return sdrkit_scan_span(handle, &args, &reply)
        }() } } }
        return code == SDRKIT_OK ? .success(ScanSpanReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Survey a wide range. Starts a survey of the range from `start_hz` to `stop_hz`. Stage one steps the radio in blocks of `rate_hz` and looks for keyed carriers for the time of the setting look each, without the stack; stage two visits each candidate as a span block and lets the result `watch` of the stack decide. Stage one also lists the occupied channels of each block in the table occupied: power 3 dB or more over the local noise across 2 to 30 kHz, of any modulation, with centre, width and level; a lone carrier or spur, a ripple of the passband, a run at the end of a block and the edge of a wider signal are refused, and the rows are not visited. When one pass of span blocks fits into `total_s` (without a total time: when it is not longer than one pass of stage one), the survey runs span blocks; the reply says which. A survey with a `dwell_s` shorter than the look is refused. Without `rate_hz` the survey takes the width that the session receives, which is known as soon as the session has started: before its first samples it is the rate that the session asked of its radio, or the rate of its host feed. A survey can start at once after its session.
    public static func scanSurvey(_ handle: SDRKIT_HANDLE, session: String, startHz: Double, stopHz: Double, spanHz: Double, watch: String, above: Double? = nil, stack: String? = nil, dwellS: Double? = nil, totalS: Double? = nil, rateHz: Double? = nil, dry: Bool? = nil) -> Result<ScanSurveyReply, SDRKitCode> {
        var reply = sdrkit_scan_survey_reply()
        var args = sdrkit_scan_survey_args()
        args.size = UInt32(MemoryLayout<sdrkit_scan_survey_args>.size)
        let code: Int32 = sdrkitWithString(session) { session_ in sdrkitWithString(watch) { watch_ in sdrkitWithString(stack ?? "") { stack_ in { () -> Int32 in
        args.session = session_
        args.start_hz = startHz
        args.stop_hz = stopHz
        args.span_hz = spanHz
        args.watch = watch_
        args.has_above = above != nil
        if let given = above { args.above = given }
        args.has_stack = stack != nil
        args.stack = stack_
        args.has_dwell_s = dwellS != nil
        if let given = dwellS { args.dwell_s = given }
        args.has_total_s = totalS != nil
        if let given = totalS { args.total_s = given }
        args.has_rate_hz = rateHz != nil
        if let given = rateHz { args.rate_hz = given }
        args.has_dry = dry != nil
        if let given = dry { args.dry = given }
        return sdrkit_scan_survey(handle, &args, &reply)
        }() } } }
        return code == SDRKIT_OK ? .success(ScanSurveyReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Attach text to a hit. Attaches a text, such as the text that the stack decoded, to a find of the running or last span or survey scan.
    public static func scanHitText(_ handle: SDRKIT_HANDLE, text: String, hit: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_scan_hit_text_args()
        args.size = UInt32(MemoryLayout<sdrkit_scan_hit_text_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in sdrkitWithString(hit ?? "") { hit_ in { () -> Int32 in
        args.text = text_
        args.has_hit = hit != nil
        args.hit = hit_
        return sdrkit_scan_hit_text(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum ScanSettings {
    /// Settle time.
    public static func scanSettle(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "settle", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanSettle(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "settle", value))
    }
    /// Dwell.
    public static func scanDwell(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanDwell(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell", value))
    }
    /// Resume delay.
    public static func scanHang(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanHang(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang", value))
    }
    /// Longest hold.
    public static func scanMaxHold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanMaxHold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_hold", value))
    }
    /// The options of the setting "stop_on" of the stack "scan".
    public enum ScanStopOn: Int32 {
        case squelch = 0
        case snr = 1
        case level = 2
        case lock = 3
        case squelchOrLock = 4
    }
    /// Stop on.
    public static func scanStopOn(_ handle: SDRKIT_HANDLE) -> Result<ScanStopOn, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "stop_on", &v)
        return code == SDRKIT_OK ? .success(ScanStopOn(rawValue: v) ?? ScanStopOn(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setScanStopOn(_ handle: SDRKIT_HANDLE, _ value: ScanStopOn) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "stop_on", value.rawValue))
    }
    /// Threshold.
    public static func scanThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Priority interval.
    public static func scanPriorityInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "priority_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanPriorityInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "priority_interval", value))
    }
    /// Priority dwell.
    public static func scanPriorityDwell(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "priority_dwell", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanPriorityDwell(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "priority_dwell", value))
    }
    /// Passes.
    public static func scanPasses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "passes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanPasses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "passes", value))
    }
    /// Most hits.
    public static func scanHitCap(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "hit_cap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanHitCap(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "hit_cap", value))
    }
    /// Keep empty channels.
    public static func scanKeepEmpty(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "keep_empty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanKeepEmpty(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "keep_empty", value))
    }
    /// The options of the setting "after_find" of the stack "scan".
    public enum ScanAfterFind: Int32 {
        case stay = 0
        case resume = 1
    }
    /// After a find.
    public static func scanAfterFind(_ handle: SDRKIT_HANDLE) -> Result<ScanAfterFind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "after_find", &v)
        return code == SDRKIT_OK ? .success(ScanAfterFind(rawValue: v) ?? ScanAfterFind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setScanAfterFind(_ handle: SDRKIT_HANDLE, _ value: ScanAfterFind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "after_find", value.rawValue))
    }
    /// Quiet time.
    public static func scanQuiet(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "quiet", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanQuiet(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "quiet", value))
    }
    /// Rejection hold.
    public static func scanRejectHold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "reject_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanRejectHold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "reject_hold", value))
    }
    /// Look time.
    public static func scanLook(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "look", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setScanLook(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "look", value))
    }
}

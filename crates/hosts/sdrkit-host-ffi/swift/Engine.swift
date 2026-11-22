// sdrkit: the typed interface of the settings that every engine has, version 1.7. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum EngineInterface {
    public static let major = 1
    public static let minor = 7
}

/// The calibration of the device and the cores of the engine.
public struct EngineFields {
    /// The calibration units per second that one fast core of this device runs, with the kernels of this build. Absent until the calibration has finished.
    public var coreUnits: Double?
    /// The same of one slow core. Equal to the fast core where the platform has one class of cores.
    public var coreUnitsSlow: Double?
    /// The fast cores of this device.
    public var coresFast: Int64?
    /// The slow cores of this device. 0 where the platform has one class of cores.
    public var coresSlow: Int64?
    /// The cores that the sessions may use: the setting `cores_max`, or the fast cores when it is 0.
    public var coresUsable: Int64

    init(_ c: sdrkit_engine_fields) {
        coreUnits = c.has_core_units ? c.core_units : nil
        coreUnitsSlow = c.has_core_units_slow ? c.core_units_slow : nil
        coresFast = c.has_cores_fast ? c.cores_fast : nil
        coresSlow = c.has_cores_slow ? c.cores_slow : nil
        coresUsable = c.cores_usable
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EngineFields, SDRKitCode> {
        var c = sdrkit_engine_fields()
        let code = sdrkit_engine_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EngineFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the table "stacks" (Stacks).
public struct EngineStackRow {
    public enum AutoGain: Int32 {
        case radio = 0
        case high = 1
        case hold = 2
    }
    public enum Kind: Int32 {
        case broadcast = 0
        case television = 1
        case voice = 2
        case textChat = 3
        case weakSignal = 4
        case mesh = 5
        case tracking = 6
        case datalink = 7
        case pictures = 8
        case satelliteImagery = 9
        case measurement = 10
        case science = 11
        case navigation = 12
        case networks = 13
        case paging = 14
        case devices = 15
        case utility = 16
        case satelliteLinks = 17
        case general = 18
    }
    public enum Audio: Int32 {
        case programme = 0
        case voice = 1
        case monitor = 2
    }
    /// The row key.
    public var key: String
    /// Name.
    public var name: String
    /// Group.
    public var group: String
    /// About.
    public var blurb: String
    /// Lowest frequency.
    public var minHz: Double
    /// Highest frequency.
    public var maxHz: Double
    /// Frequency.
    public var defaultHz: Double
    /// Sample rate.
    public var rateHz: Double
    /// Lowest sample rate.
    public var minRateHz: Double
    /// Bandwidth.
    public var bandwidthHz: Double
    /// Automatic gain.
    public var autoGain: AutoGain?
    /// Test signal.
    public var generator: Bool
    /// Settle time.
    public var lockSettleS: Double
    /// Lock timeout.
    public var lockTimeoutS: Double
    /// Signal SNR.
    public var lockSnrDb: Double
    /// Name time.
    public var lockCollectS: Double
    /// Transmit.
    public var transmit: Bool
    /// The channels of host audio that the graph of the stack takes for a transmission: 0 for none, 1 for mono, 2 for stereo.
    public var hostAudioChannels: Int64
    /// The kind of content of the stack. A front end chooses its view of the stack from it. No value for a stack without an interface description.
    public var kind: Kind?
    /// The kind of sound that the stack gives. No value for a stack without sound.
    public var audio: Audio?
    /// The stack gives still pictures.
    public var pictures: Bool
    /// The stack gives moving pictures.
    public var video: Bool
    /// The stack gives running text for people: decoded characters or whole messages.
    public var text: Bool

    init(_ c: sdrkit_engine_stack_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        group = sdrkitText(c.group)
        blurb = sdrkitText(c.blurb)
        minHz = c.min_hz
        maxHz = c.max_hz
        defaultHz = c.default_hz
        rateHz = c.rate_hz
        minRateHz = c.min_rate_hz
        bandwidthHz = c.bandwidth_hz
        autoGain = AutoGain(rawValue: c.auto_gain)
        generator = c.generator
        lockSettleS = c.lock_settle_s
        lockTimeoutS = c.lock_timeout_s
        lockSnrDb = c.lock_snr_db
        lockCollectS = c.lock_collect_s
        transmit = c.transmit
        hostAudioChannels = c.host_audio_channels
        kind = c.has_kind ? Kind(rawValue: c.kind) : nil
        audio = c.has_audio ? Audio(rawValue: c.audio) : nil
        pictures = c.pictures
        video = c.video
        text = c.text
    }
}

/// A row of the table "submodes" (Variants).
public struct EngineSubmodeRow {
    /// The row key.
    public var key: String
    /// Stack.
    public var stack: String
    /// Name.
    public var name: String
    /// Frequency.
    public var defaultHz: Double

    init(_ c: sdrkit_engine_submode_row) {
        key = sdrkitText(c.key)
        stack = sdrkitText(c.stack)
        name = sdrkitText(c.name)
        defaultHz = c.default_hz
    }
}

/// A row of the table "stack_signals" (Signal types).
public struct EngineStackSignalRow {
    public enum Signal: Int32 {
        case carrier = 0
        case amVoice = 1
        case amBroadcast = 2
        case narrowFmVoice = 3
        case keyedVoice = 4
        case wideFm = 5
        case fmBroadcast = 6
        case ssbVoiceUpper = 7
        case ssbVoiceLower = 8
        case cwMorse = 9
        case narrowDigital = 10
        case digital = 11
        case wideDigital = 12
        case adsBModeS = 13
        case modeACReplies = 14
        case modeSInterrogations = 15
        case uat = 16
        case dvbT = 17
        case dvbT2 = 18
        case dab = 19
        case isdbT = 20
        case isdbTsb = 21
        case dtmb = 22
        case atsc10 = 23
        case dmr = 24
        case p25Phase1 = 25
        case p25Phase2 = 26
        case nxdn96 = 27
        case nxdn48 = 28
        case dpmr = 29
        case yaesuSystemFusion = 30
        case m17 = 31
        case dStar = 32
        case tetra = 33
        case pocsag = 34
        case ais = 35
        case lora = 36
        case packetAfsk1200 = 37
        case rtty = 38
        case sitor = 39
        case psk31 = 40
        case mfsk = 41
        case dominoex = 42
        case thor = 43
        case olivia = 44
        case contestia = 45
        case mt63 = 46
        case fsq = 47
        case ifkp = 48
        case hellschreiber = 49
        case sstv = 50
        case ardop = 51
        case ros = 52
        case ft8 = 53
        case ft4 = 54
        case ft2 = 55
        case wspr = 56
        case jt65 = 57
        case jt9 = 58
        case jt4 = 59
        case q65 = 60
        case fst4 = 61
        case fst4w = 62
        case msk144 = 63
        case js8 = 64
        case djiDroneid = 65
        case flarm = 66
        case atsc30 = 67
        case analogTv = 68
        case mac = 69
        case fmVideo = 70
        case acars = 71
        case selcal = 72
        case hfdl = 73
        case vdlMode2 = 74
        case vdlMode4 = 75
        case gnss = 76
        case sbas = 77
        case lbj = 78
        case balise = 79
        case gsm = 80
        case nbIot = 81
        case lte = 82
        case nr = 83
        case dect = 84
        case ant = 85
        case ieee802154 = 86
        case nfc = 87
        case nrf24 = 88
        case sigfox = 89
        case wirelessMBus = 90
        case wiSun = 91
        case zWave = 92
        case dvbS = 93
        case dvbS2 = 94
        case dvbC = 95
        case dvbC2 = 96
        case isdbS = 97
        case tetrapol = 98
        case freedv = 99
        case mpt1327 = 100
        case ltr = 101
        case edacs = 102
        case multitonePaging = 103
        case wifi = 104
        case wifiHalow = 105
        case bluetoothLe = 106
        case ndb = 107
        case vor = 108
        case ils = 109
        case dme = 110
        case loranC = 111
        case dgps = 112
        case lrit = 113
        case hrit = 114
        case hrpt = 115
        case lrpt = 116
        case sVissr = 117
        case dsc = 118
        case weatherFax = 119
        case vdes = 120
        case atis = 121
        case drm = 122
        case hamdrm = 123
        case hdRadio = 124
        case cdr = 125
        case pactor1 = 126
        case pactor2 = 127
        case pactor3 = 128
        case pactor4 = 129
        case hfOceanRadar = 130
        case ale2g = 131
        case radiosonde = 132
        case same = 133
        case timeSignal = 134
        case dcp = 135
        case argos = 136
        case cospasSarsat406 = 137
        case inmarsatC = 138
        case inmarsatAero = 139
        case orbcomm = 140
        case iridium = 141
        case globalstar = 142
    }
    /// The row key.
    public var key: String
    /// Stack.
    public var stack: String
    /// Signal type.
    public var signal: Signal?

    init(_ c: sdrkit_engine_stack_signal_row) {
        key = sdrkitText(c.key)
        stack = sdrkitText(c.stack)
        signal = Signal(rawValue: c.signal)
    }
}

/// A row of the table "drivers" (Drivers).
public struct EngineDriverRow {
    /// The row key.
    public var key: String
    /// Name.
    public var name: String
    /// Address.
    public var address: String
    /// Lowest sample rate.
    public var minRateHz: Double?
    /// Highest sample rate.
    public var maxRateHz: Double?
    /// Transmit.
    public var transmit: Bool
    /// Highest transmit rate.
    public var txMaxRateHz: Double?

    init(_ c: sdrkit_engine_driver_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        address = sdrkitText(c.address)
        minRateHz = c.has_min_rate_hz ? c.min_rate_hz : nil
        maxRateHz = c.has_max_rate_hz ? c.max_rate_hz : nil
        transmit = c.transmit
        txMaxRateHz = c.has_tx_max_rate_hz ? c.tx_max_rate_hz : nil
    }
}

/// A row of the table "component_kinds" (Component kinds).
public struct EngineComponentKindRow {
    /// The row key.
    public var key: String
    /// Inputs.
    public var inputs: Int64
    /// Outputs.
    public var outputs: Int64
    /// Settings.
    public var settings: Int64

    init(_ c: sdrkit_engine_component_kind_row) {
        key = sdrkitText(c.key)
        inputs = c.inputs
        outputs = c.outputs
        settings = c.settings
    }
}

/// A row of the table "services" (Services).
public struct EngineServiceRow {
    /// The row key.
    public var key: String
    /// Name.
    public var name: String

    init(_ c: sdrkit_engine_service_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
    }
}

/// A row of the table "radios" (Radios).
public struct EngineRadioRow {
    /// The row key.
    public var key: String
    /// Driver.
    public var driver: String
    /// Address.
    public var address: String
    /// Transmit.
    public var transmit: Bool
    /// Lowest transmit frequency.
    public var txMinHz: Double?
    /// Highest transmit frequency.
    public var txMaxHz: Double?
    /// Highest transmit rate.
    public var txMaxRateHz: Double?

    init(_ c: sdrkit_engine_radio_row) {
        key = sdrkitText(c.key)
        driver = sdrkitText(c.driver)
        address = sdrkitText(c.address)
        transmit = c.transmit
        txMinHz = c.has_tx_min_hz ? c.tx_min_hz : nil
        txMaxHz = c.has_tx_max_hz ? c.tx_max_hz : nil
        txMaxRateHz = c.has_tx_max_rate_hz ? c.tx_max_rate_hz : nil
    }
}

/// A row of the table "sessions" (Sessions).
public struct EngineSessionRow {
    public enum State: Int32 {
        case waiting = 0
        case running = 1
        case failed = 2
        case stopped = 3
    }
    /// The row key.
    public var key: String
    /// Stack.
    public var stack: String?
    /// Radio.
    public var radio: String
    /// State.
    public var state: State?

    init(_ c: sdrkit_engine_session_row) {
        key = sdrkitText(c.key)
        stack = c.has_stack ? sdrkitText(c.stack) : nil
        radio = sdrkitText(c.radio)
        state = State(rawValue: c.state)
    }
}

/// A row of the table "requests" (Requests).
public struct EngineRequestRow {
    public enum Kind: Int32 {
        case retune = 0
        case hop = 1
        case follow = 2
        case release = 3
        case tune = 4
        case storeMemory = 5
        case memoryHit = 6
        case record = 7
        case rewind = 8
        case script = 9
        case output = 10
    }
    /// The row key.
    public var key: String
    /// From.
    public var from: String
    /// Request.
    public var kind: Kind?
    /// Frequency.
    public var freqHz: Double?
    /// Done.
    public var done: Bool
    /// Refused because.
    public var refused: String?
    /// Session.
    public var session: String?

    init(_ c: sdrkit_engine_request_row) {
        key = sdrkitText(c.key)
        from = sdrkitText(c.from)
        kind = Kind(rawValue: c.kind)
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        done = c.done
        refused = c.has_refused ? sdrkitText(c.refused) : nil
        session = c.has_session ? sdrkitText(c.session) : nil
    }
}

/// A row of the table "benchmarks" (Benchmarks).
public struct EngineBenchmarkRow {
    /// The row key.
    public var key: String
    /// Stack.
    public var stack: String
    /// Items per second.
    public var itemsPerS: Double
    /// Processor share.
    public var cpuShare: Double
    /// Threads.
    public var threads: Int64
    /// Fullest thread.
    public var threadCores: Double
    /// Status.
    public var status: String

    init(_ c: sdrkit_engine_benchmark_row) {
        key = sdrkitText(c.key)
        stack = sdrkitText(c.stack)
        itemsPerS = c.items_per_s
        cpuShare = c.cpu_share
        threads = c.threads
        threadCores = c.thread_cores
        status = sdrkitText(c.status)
    }
}

/// A row of the table "benchmark.components" (Benchmark components).
public struct EngineBenchmarkComponentRow {
    /// The row key.
    public var key: String
    /// Benchmark.
    public var benchmark: String
    /// Component.
    public var component: String
    /// Section.
    public var section: String
    /// Stage.
    public var stage: String
    /// Thread.
    public var thread: Int64
    /// Cost.
    public var cost: Double
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

    init(_ c: sdrkit_engine_benchmark_component_row) {
        key = sdrkitText(c.key)
        benchmark = sdrkitText(c.benchmark)
        component = sdrkitText(c.component)
        section = sdrkitText(c.section)
        stage = sdrkitText(c.stage)
        thread = c.thread
        cost = c.cost
        calls = c.calls
        itemsIn = c.items_in
        itemsOut = c.items_out
        meanUs = c.mean_us
        worstUs = c.worst_us
        units = c.units
    }
}

/// A row of the table "benchmark.stages" (Benchmark stages).
public struct EngineBenchmarkStageRow {
    /// The row key.
    public var key: String
    /// Benchmark.
    public var benchmark: String
    /// Stage.
    public var stage: String
    /// Thread.
    public var thread: Int64
    /// Cost.
    public var cost: Double
    /// Units.
    public var units: Double
    /// Declared units.
    public var declaredUnits: Double?

    init(_ c: sdrkit_engine_benchmark_stage_row) {
        key = sdrkitText(c.key)
        benchmark = sdrkitText(c.benchmark)
        stage = sdrkitText(c.stage)
        thread = c.thread
        cost = c.cost
        units = c.units
        declaredUnits = c.has_declared_units ? c.declared_units : nil
    }
}

/// A row of the table "benchmark.connections" (Benchmark connections).
public struct EngineBenchmarkConnectionRow {
    /// The row key.
    public var key: String
    /// Benchmark.
    public var benchmark: String
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
    /// Held mean.
    public var heldMeanBytes: Double
    /// Held most.
    public var heldMaxBytes: Int64
    /// Capacity.
    public var capacityBytes: Int64?
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

    init(_ c: sdrkit_engine_benchmark_connection_row) {
        key = sdrkitText(c.key)
        benchmark = sdrkitText(c.benchmark)
        from = sdrkitText(c.from)
        output = sdrkitText(c.output)
        to = sdrkitText(c.to)
        input = sdrkitText(c.input)
        format = sdrkitText(c.format)
        stage = sdrkitText(c.stage)
        itemsPerS = c.items_per_s
        bytesPerS = c.bytes_per_s
        heldMeanBytes = c.held_mean_bytes
        heldMaxBytes = c.held_max_bytes
        capacityBytes = c.has_capacity_bytes ? c.capacity_bytes : nil
        waitMeanUs = c.has_wait_mean_us ? c.wait_mean_us : nil
        waitWorstUs = c.has_wait_worst_us ? c.wait_worst_us : nil
        full = c.full
        empty = c.empty
        boundary = c.boundary
    }
}

/// A row of the table "stack_transmits" (Transmit commands).
public struct EngineStackTransmitRow {
    public enum Content: Int32 {
        case text = 0
        case voice = 1
        case beacon = 2
        case file = 3
        case frame = 4
        case carrier = 5
    }
    /// The row key.
    public var key: String
    /// Stack.
    public var stack: String
    /// Command.
    public var command: String
    /// Content.
    public var content: Content?
    /// Stopped by.
    public var stoppedBy: String?

    init(_ c: sdrkit_engine_stack_transmit_row) {
        key = sdrkitText(c.key)
        stack = sdrkitText(c.stack)
        command = sdrkitText(c.command)
        content = Content(rawValue: c.content)
        stoppedBy = c.has_stopped_by ? sdrkitText(c.stopped_by) : nil
    }
}

/// The reply of `state.save`.
public struct EngineStateSaveReply {
    /// State.
    public var state: [UInt8]

    init(_ c: sdrkit_engine_state_save_reply) {
        state = [UInt8](UnsafeBufferPointer(start: c.state.ptr, count: c.state.len))
    }
}

/// Request.
public struct EngineRequestEvent {
    public enum Kind: Int32 {
        case retune = 0
        case hop = 1
        case follow = 2
        case release = 3
        case tune = 4
        case storeMemory = 5
        case memoryHit = 6
        case record = 7
        case rewind = 8
        case script = 9
        case output = 10
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
    /// From.
    public var from: String
    /// Request.
    public var kind: Kind?
    /// Done.
    public var done: Bool
    /// Refused because.
    public var refused: String?
    /// Session.
    public var session: String?
    /// Family.
    public var family: Family?

    init(_ c: sdrkit_engine_request_event) {
        time = c.time
        freqHz = c.freq_hz
        from = sdrkitText(c.from)
        kind = Kind(rawValue: c.kind)
        done = c.done
        refused = c.has_refused ? sdrkitText(c.refused) : nil
        session = c.has_session ? sdrkitText(c.session) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EngineRequestEvent, SDRKitCode> {
        var c = sdrkit_engine_request_event()
        let code = sdrkit_engine_request_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EngineRequestEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum EngineTables {
    /// The changed rows of the table "stacks" of the stack "engine", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func engineStacks(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineStackRow], SDRKitCode> {
        var rows = [sdrkit_engine_stack_row](repeating: sdrkit_engine_stack_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_stacks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineStackRow.init))
    }
    /// The changed rows of the table "submodes" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineSubmodes(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineSubmodeRow], SDRKitCode> {
        var rows = [sdrkit_engine_submode_row](repeating: sdrkit_engine_submode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_submodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineSubmodeRow.init))
    }
    /// The changed rows of the table "stack_signals" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineStackSignals(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineStackSignalRow], SDRKitCode> {
        var rows = [sdrkit_engine_stack_signal_row](repeating: sdrkit_engine_stack_signal_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_stack_signals_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineStackSignalRow.init))
    }
    /// The changed rows of the table "drivers" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineDrivers(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineDriverRow], SDRKitCode> {
        var rows = [sdrkit_engine_driver_row](repeating: sdrkit_engine_driver_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_drivers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineDriverRow.init))
    }
    /// The changed rows of the table "component_kinds" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineComponentKinds(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineComponentKindRow], SDRKitCode> {
        var rows = [sdrkit_engine_component_kind_row](repeating: sdrkit_engine_component_kind_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_component_kinds_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineComponentKindRow.init))
    }
    /// The changed rows of the table "services" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineServices(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineServiceRow], SDRKitCode> {
        var rows = [sdrkit_engine_service_row](repeating: sdrkit_engine_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineServiceRow.init))
    }
    /// The changed rows of the table "radios" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineRadios(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineRadioRow], SDRKitCode> {
        var rows = [sdrkit_engine_radio_row](repeating: sdrkit_engine_radio_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_radios_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineRadioRow.init))
    }
    /// The changed rows of the table "sessions" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineSessions(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineSessionRow], SDRKitCode> {
        var rows = [sdrkit_engine_session_row](repeating: sdrkit_engine_session_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_sessions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineSessionRow.init))
    }
    /// The changed rows of the table "requests" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineRequests(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[EngineRequestRow], SDRKitCode> {
        var rows = [sdrkit_engine_request_row](repeating: sdrkit_engine_request_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_requests_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineRequestRow.init))
    }
    /// The changed rows of the table "benchmarks" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineBenchmarks(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineBenchmarkRow], SDRKitCode> {
        var rows = [sdrkit_engine_benchmark_row](repeating: sdrkit_engine_benchmark_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_benchmarks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineBenchmarkRow.init))
    }
    /// The changed rows of the table "benchmark.components" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineBenchmarkComponents(_ handle: SDRKIT_HANDLE, capacity: Int = 16384) -> Result<[EngineBenchmarkComponentRow], SDRKitCode> {
        var rows = [sdrkit_engine_benchmark_component_row](repeating: sdrkit_engine_benchmark_component_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_benchmark_components_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineBenchmarkComponentRow.init))
    }
    /// The changed rows of the table "benchmark.stages" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineBenchmarkStages(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineBenchmarkStageRow], SDRKitCode> {
        var rows = [sdrkit_engine_benchmark_stage_row](repeating: sdrkit_engine_benchmark_stage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_benchmark_stages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineBenchmarkStageRow.init))
    }
    /// The changed rows of the table "benchmark.connections" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineBenchmarkConnections(_ handle: SDRKIT_HANDLE, capacity: Int = 16384) -> Result<[EngineBenchmarkConnectionRow], SDRKitCode> {
        var rows = [sdrkit_engine_benchmark_connection_row](repeating: sdrkit_engine_benchmark_connection_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_benchmark_connections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineBenchmarkConnectionRow.init))
    }
    /// The changed rows of the table "stack_transmits" of the stack "engine", newest first, at most `capacity` rows.
    public static func engineStackTransmits(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[EngineStackTransmitRow], SDRKitCode> {
        var rows = [sdrkit_engine_stack_transmit_row](repeating: sdrkit_engine_stack_transmit_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_engine_stack_transmits_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EngineStackTransmitRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum EngineCommands {
    /// Save state. Returns the saved state of every radio, session and service that has one.
    public static func engineStateSave(_ handle: SDRKIT_HANDLE) -> Result<EngineStateSaveReply, SDRKitCode> {
        var reply = sdrkit_engine_state_save_reply()
        let code = sdrkit_engine_state_save(handle, &reply)
        return code == SDRKIT_OK ? .success(EngineStateSaveReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Restore state. Restores the saved state of each owner named in the bytes of `state.save`. Every owner must be there.
    public static func engineStateRestore(_ handle: SDRKIT_HANDLE, state: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_engine_state_restore_args()
        args.size = UInt32(MemoryLayout<sdrkit_engine_state_restore_args>.size)
        let code: Int32 = state.withUnsafeBufferPointer { state_ in { () -> Int32 in
        args.state = sdrkit_bytes(ptr: state_.baseAddress, len: state_.count)
        return sdrkit_engine_state_restore(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Run benchmarks. Runs the benchmarks of every stack, or those that `only` selects, on a thread of its own, each for about `seconds`. The table `benchmarks` shows the results.
    public static func engineBenchmarkRun(_ handle: SDRKIT_HANDLE, seconds: Double? = nil, only: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_engine_benchmark_run_args()
        args.size = UInt32(MemoryLayout<sdrkit_engine_benchmark_run_args>.size)
        let code: Int32 = sdrkitWithString(only ?? "") { only_ in { () -> Int32 in
        args.has_seconds = seconds != nil
        if let given = seconds { args.seconds = given }
        args.has_only = only != nil
        args.only = only_
        return sdrkit_engine_benchmark_run(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum EngineSettings {
    /// Callsign.
    public static func engineCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// Locator.
    public static func engineLocator(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "locator", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineLocator(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "locator", $0) })
    }
    /// Position.
    public static func enginePosition(_ handle: SDRKIT_HANDLE) -> Result<SDRKitPosition, SDRKitCode> {
        var v: sdrkit_position = sdrkit_position()
        let code = sdrkit_typed_setting_get_position(handle, "position", &v)
        return code == SDRKIT_OK ? .success(SDRKitPosition(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEnginePosition(_ handle: SDRKIT_HANDLE, _ value: SDRKitPosition) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_position(handle, "position", value.c))
    }
    /// Height.
    public static func engineHeightM(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "height_m", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineHeightM(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "height_m", value))
    }
    /// Time zone.
    public static func engineTimeZone(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "time_zone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineTimeZone(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "time_zone", value))
    }
    /// Requests.
    public static func engineRequestsAllowed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "requests_allowed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineRequestsAllowed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "requests_allowed", value))
    }
    /// Transmit.
    public static func engineTransmitAllowed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "transmit_allowed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineTransmitAllowed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "transmit_allowed", value))
    }
    /// The options of the setting "backend_fft" of the stack "engine".
    public enum EngineBackendFft: Int32 {
        case auto = 0
        case accelerate = 1
        case rustfft = 2
    }
    /// FFT backend.
    public static func engineBackendFft(_ handle: SDRKIT_HANDLE) -> Result<EngineBackendFft, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "backend_fft", &v)
        return code == SDRKIT_OK ? .success(EngineBackendFft(rawValue: v) ?? EngineBackendFft(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineBackendFft(_ handle: SDRKIT_HANDLE, _ value: EngineBackendFft) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "backend_fft", value.rawValue))
    }
    /// The options of the setting "backend_fir" of the stack "engine".
    public enum EngineBackendFir: Int32 {
        case auto = 0
        case neon = 1
        case plain = 2
    }
    /// FIR backend.
    public static func engineBackendFir(_ handle: SDRKIT_HANDLE) -> Result<EngineBackendFir, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "backend_fir", &v)
        return code == SDRKIT_OK ? .success(EngineBackendFir(rawValue: v) ?? EngineBackendFir(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineBackendFir(_ handle: SDRKIT_HANDLE, _ value: EngineBackendFir) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "backend_fir", value.rawValue))
    }
    /// The options of the setting "backend_resample" of the stack "engine".
    public enum EngineBackendResample: Int32 {
        case auto = 0
        case neon = 1
        case plain = 2
    }
    /// Resampler backend.
    public static func engineBackendResample(_ handle: SDRKIT_HANDLE) -> Result<EngineBackendResample, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "backend_resample", &v)
        return code == SDRKIT_OK ? .success(EngineBackendResample(rawValue: v) ?? EngineBackendResample(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineBackendResample(_ handle: SDRKIT_HANDLE, _ value: EngineBackendResample) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "backend_resample", value.rawValue))
    }
    /// The options of the setting "backend_viterbi" of the stack "engine".
    public enum EngineBackendViterbi: Int32 {
        case auto = 0
        case neon = 1
        case plain = 2
    }
    /// Viterbi backend.
    public static func engineBackendViterbi(_ handle: SDRKIT_HANDLE) -> Result<EngineBackendViterbi, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "backend_viterbi", &v)
        return code == SDRKIT_OK ? .success(EngineBackendViterbi(rawValue: v) ?? EngineBackendViterbi(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineBackendViterbi(_ handle: SDRKIT_HANDLE, _ value: EngineBackendViterbi) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "backend_viterbi", value.rawValue))
    }
    /// The options of the setting "backend_rs" of the stack "engine".
    public enum EngineBackendRs: Int32 {
        case auto = 0
        case neon = 1
        case plain = 2
    }
    /// Reed-Solomon backend.
    public static func engineBackendRs(_ handle: SDRKIT_HANDLE) -> Result<EngineBackendRs, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "backend_rs", &v)
        return code == SDRKIT_OK ? .success(EngineBackendRs(rawValue: v) ?? EngineBackendRs(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineBackendRs(_ handle: SDRKIT_HANDLE, _ value: EngineBackendRs) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "backend_rs", value.rawValue))
    }
    /// The options of the setting "backend_ldpc" of the stack "engine".
    public enum EngineBackendLdpc: Int32 {
        case auto = 0
        case neon = 1
        case plain = 2
    }
    /// LDPC backend.
    public static func engineBackendLdpc(_ handle: SDRKIT_HANDLE) -> Result<EngineBackendLdpc, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "backend_ldpc", &v)
        return code == SDRKIT_OK ? .success(EngineBackendLdpc(rawValue: v) ?? EngineBackendLdpc(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineBackendLdpc(_ handle: SDRKIT_HANDLE, _ value: EngineBackendLdpc) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "backend_ldpc", value.rawValue))
    }
    /// Cores.
    public static func engineCoresMax(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "cores_max", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEngineCoresMax(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "cores_max", value))
    }
}

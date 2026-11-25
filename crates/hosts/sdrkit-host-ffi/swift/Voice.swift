// sdrkit: the typed interface of the stack family "voice", version 1.5. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum VoiceInterface {
    public static let major = 1
    public static let minor = 5
}

/// The result fields that every single-channel stack shares.
public struct VoiceDetectorFields {
    public enum State: Int32 {
        case searching = 0
        case signal = 1
        case confirmed = 2
    }
    /// How far the detector is.
    public var state: State?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// The quality measure of the stack.
    public var qualityDb: Double?
    /// The signal time.
    public var signalTimeS: Double
    /// The rate of the input.
    public var inputRateHz: Double
    /// The count of good results.
    public var good: Int64
    /// The count of bad results.
    public var bad: Int64

    init(_ c: sdrkit_voice_detector_fields) {
        state = State(rawValue: c.state)
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        qualityDb = c.has_quality_db ? c.quality_db : nil
        signalTimeS = c.signal_time_s
        inputRateHz = c.input_rate_hz
        good = c.good
        bad = c.bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<VoiceDetectorFields, SDRKitCode> {
        var c = sdrkit_voice_detector_fields()
        let code = sdrkit_voice_detector_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VoiceDetectorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of HF packet.
public struct HfpacketFields {
    /// The tuning correction.
    public var toneOffsetHz: Double
    /// Different sources heard.
    public var stations: Int64
    /// The newest frame in the monitor form.
    public var lastFrame: String?

    init(_ c: sdrkit_hfpacket_fields) {
        toneOffsetHz = c.tone_offset_hz
        stations = c.stations
        lastFrame = c.has_last_frame ? sdrkitText(c.last_frame) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfpacketFields, SDRKitCode> {
        var c = sdrkit_hfpacket_fields()
        let code = sdrkit_hfpacket_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfpacketFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The count of the input guard of a single-channel stack.
public struct VoiceGuardFields {
    /// Input samples replaced because they were not finite or far beyond full scale.
    public var badSamples: Int64

    init(_ c: sdrkit_voice_guard_fields) {
        badSamples = c.bad_samples
    }
}

/// An event of a single-channel stack. The row key is the number of the event.
public struct VoiceDetectorEventRow {
    /// The row key.
    public var key: String
    /// The wall-clock time of an event.
    public var timeUtc: Double?
    /// The signal time.
    public var signalTimeS: Double
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_voice_detector_event_row) {
        key = sdrkitText(c.key)
        timeUtc = c.has_time_utc ? c.time_utc : nil
        signalTimeS = c.signal_time_s
        text = sdrkitText(c.text)
    }
}

/// A frame with a good FCS. The row key is the number of the frame.
public struct HfpacketFrameRow {
    /// The row key.
    public var key: String
    /// When a call started.
    public var timeUtc: Double?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The source of a frame.
    public var destination: String
    /// The digipeater path of a frame.
    public var path: String
    /// The frame type.
    public var type: String
    /// The protocol identifier.
    public var pid: Int64?
    /// The information field as text.
    public var info: String
    /// The tuning correction.
    public var toneOffsetHz: Double

    init(_ c: sdrkit_hfpacket_frame_row) {
        key = sdrkitText(c.key)
        timeUtc = c.has_time_utc ? c.time_utc : nil
        source = sdrkitText(c.source)
        destination = sdrkitText(c.destination)
        path = sdrkitText(c.path)
        type = sdrkitText(c.type)
        pid = c.has_pid ? c.pid : nil
        info = sdrkitText(c.info)
        toneOffsetHz = c.tone_offset_hz
    }
}

/// The result fields of the 121.5 MHz ELT detector.
public struct Elt121Fields {
    /// The periods of the sweeps confirm the signal.
    public var confirmed: Bool
    /// Sweeps a second.
    public var sweepRateHz: Double?
    /// The first frequency of a sweep.
    public var sweepTopHz: Double?
    /// The lowest frequency of a sweep.
    public var sweepBottomHz: Double?
    /// The modulation of the tone.
    public var depthPct: Double?

    init(_ c: sdrkit_elt121_fields) {
        confirmed = c.confirmed
        sweepRateHz = c.has_sweep_rate_hz ? c.sweep_rate_hz : nil
        sweepTopHz = c.has_sweep_top_hz ? c.sweep_top_hz : nil
        sweepBottomHz = c.has_sweep_bottom_hz ? c.sweep_bottom_hz : nil
        depthPct = c.has_depth_pct ? c.depth_pct : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Elt121Fields, SDRKitCode> {
        var c = sdrkit_elt121_fields()
        let code = sdrkit_elt121_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Elt121Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the voice inversion detector.
public struct VoiceInversionFields {
    /// Inverted speech is there.
    public var inverted: Bool
    /// The inversion carrier found.
    public var carrierHz: Double?
    /// The fit of the spectrum as heard.
    public var fitHeard: Double?
    /// The best fit of the flipped spectrum.
    public var fitInverted: Double?

    init(_ c: sdrkit_voice_inversion_fields) {
        inverted = c.inverted
        carrierHz = c.has_carrier_hz ? c.carrier_hz : nil
        fitHeard = c.has_fit_heard ? c.fit_heard : nil
        fitInverted = c.has_fit_inverted ? c.fit_inverted : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<VoiceInversionFields, SDRKitCode> {
        var c = sdrkit_voice_inversion_fields()
        let code = sdrkit_voice_inversion_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VoiceInversionFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the multi-carrier receiver.
public struct VoiceChannelsFields {
    public enum Submode: Int32 {
        case cb = 0
        case frs = 1
        case pmr446 = 2
        case hamRepeaters = 3
        case hfvoice = 4
    }
    public enum ScanState: Int32 {
        case idle = 0
        case priority = 1
        case priorityProbe = 2
        case scanning = 3
        case holding = 4
    }
    /// The service.
    public var submode: Submode?
    /// The name of the plan in use. Absent without a plan.
    public var plan: String?
    /// The channels of the plan in use.
    public var planChannels: Int64
    /// The raster step in use.
    public var rasterStepHz: Double
    /// Carriers followed now.
    public var carriers: Int64
    /// Channels with a carrier now.
    public var activeChannels: Int64
    /// Channels with an open squelch.
    public var openChannels: Int64
    /// The channel that plays.
    public var playing: String?
    /// The frequency that plays.
    public var playingHz: Double?
    /// The scan is on.
    public var scan: Bool
    /// What the player does.
    public var scanState: ScanState?
    /// The scan steps the radio.
    public var singleTuner: Bool
    /// Where the stepping scan requires the radio.
    public var tuneHz: Double?
    /// The capture holds more than one channel.
    public var multiChannel: Bool
    /// Calls since the start.
    public var calls: Int64
    /// The input rate.
    public var inputRateHz: Double
    /// The centre of the capture.
    public var centreHz: Double
    /// The signal time.
    public var signalTimeS: Double
    /// Reports made.
    public var reports: Int64

    init(_ c: sdrkit_voice_channels_fields) {
        submode = c.has_submode ? Submode(rawValue: c.submode) : nil
        plan = c.has_plan ? sdrkitText(c.plan) : nil
        planChannels = c.plan_channels
        rasterStepHz = c.raster_step_hz
        carriers = c.carriers
        activeChannels = c.active_channels
        openChannels = c.open_channels
        playing = c.has_playing ? sdrkitText(c.playing) : nil
        playingHz = c.has_playing_hz ? c.playing_hz : nil
        scan = c.scan
        scanState = ScanState(rawValue: c.scan_state)
        singleTuner = c.single_tuner
        tuneHz = c.has_tune_hz ? c.tune_hz : nil
        multiChannel = c.multi_channel
        calls = c.calls
        inputRateHz = c.input_rate_hz
        centreHz = c.centre_hz
        signalTimeS = c.signal_time_s
        reports = c.reports
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<VoiceChannelsFields, SDRKitCode> {
        var c = sdrkit_voice_channels_fields()
        let code = sdrkit_voice_channels_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VoiceChannelsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A call in the call log.
public struct VoiceCallRow {
    /// The row key.
    public var key: String
    /// The number of a call.
    public var serial: Int64
    /// When a call started.
    public var timeUtc: Double?
    /// The channel key.
    public var keyField: Int64
    /// The number of the plan.
    public var number: Int64
    /// The short label.
    public var label: String
    /// Where to listen.
    public var hz: Double
    /// The signal time a call started.
    public var startS: Double
    /// How long a call lasted.
    public var durationS: Double
    /// The highest level.
    public var peakDbfs: Double
    /// The mean level of a call.
    public var meanDbfs: Double
    /// The demodulator of a call.
    public var demod: String
    /// The tone or code heard.
    public var tone: String
    /// The squelch is open.
    public var `open`: Bool

    init(_ c: sdrkit_voice_call_row) {
        key = sdrkitText(c.key)
        serial = c.serial
        timeUtc = c.has_time_utc ? c.time_utc : nil
        keyField = c.key_
        number = c.number
        label = sdrkitText(c.label)
        hz = c.hz
        startS = c.start_s
        durationS = c.duration_s
        peakDbfs = c.peak_dbfs
        meanDbfs = c.mean_dbfs
        demod = sdrkitText(c.demod)
        tone = sdrkitText(c.tone)
        `open` = c.open
    }
}

/// A channel of the plan or a carrier heard.
public struct VoiceChannelRow {
    public enum Demod: Int32 {
        case fm = 0
        case am = 1
        case usb = 2
        case lsb = 3
    }
    /// The row key.
    public var key: String
    /// The channel key.
    public var keyField: Int64
    /// The number of the plan.
    public var number: Int64
    /// The short label.
    public var label: String
    /// The use of the channel.
    public var name: String
    /// Where to listen.
    public var hz: Double
    /// The repeater input.
    public var inputHz: Double?
    /// A carrier is there.
    public var active: Bool
    /// The squelch is open.
    public var `open`: Bool
    /// The channel plays.
    public var playing: Bool
    /// The channel is in the scan list.
    public var inScan: Bool
    /// The schedule says the channel broadcasts now.
    public var onAir: Bool
    /// The channel level.
    public var levelDbfs: Double?
    /// The demodulator in use.
    public var demod: Demod?
    /// The tone or code heard.
    public var tone: String
    /// The other reading of a DCS word.
    public var toneAlias: String
    /// The last tone or code heard on the channel.
    public var lastTone: String
    /// Calls on the channel.
    public var calls: Int64
    /// The time the channel was open in all.
    public var totalS: Double
    /// The time since the channel was last open.
    public var lastHeardS: Double?
    /// When the channel was last open.
    public var lastHeardUtc: Double?
    /// The highest level.
    public var peakDbfs: Double?

    init(_ c: sdrkit_voice_channel_row) {
        key = sdrkitText(c.key)
        keyField = c.key_
        number = c.number
        label = sdrkitText(c.label)
        name = sdrkitText(c.name)
        hz = c.hz
        inputHz = c.has_input_hz ? c.input_hz : nil
        active = c.active
        `open` = c.open
        playing = c.playing
        inScan = c.in_scan
        onAir = c.on_air
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        demod = Demod(rawValue: c.demod)
        tone = sdrkitText(c.tone)
        toneAlias = sdrkitText(c.tone_alias)
        lastTone = sdrkitText(c.last_tone)
        calls = c.calls
        totalS = c.total_s
        lastHeardS = c.has_last_heard_s ? c.last_heard_s : nil
        lastHeardUtc = c.has_last_heard_utc ? c.last_heard_utc : nil
        peakDbfs = c.has_peak_dbfs ? c.peak_dbfs : nil
    }
}

/// The result fields of the MDC-1200 burst decoder.
public struct Mdc1200Fields {
    /// Bursts so far.
    public var bursts: Int64
    /// Blocks whose check value was wrong.
    public var checkFailures: Int64

    init(_ c: sdrkit_mdc1200_fields) {
        bursts = c.bursts
        checkFailures = c.check_failures
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Mdc1200Fields, SDRKitCode> {
        var c = sdrkit_mdc1200_fields()
        let code = sdrkit_mdc1200_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Mdc1200Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst in the burst log.
public struct Mdc1200BurstRow {
    /// The row key.
    public var key: String
    /// The number of a call.
    public var serial: Int64
    /// When a call started.
    public var timeUtc: Double?
    /// Where to listen.
    public var hz: Double
    /// The short label.
    public var label: String
    /// The unit id, 4 hex digits.
    public var unitId: String
    /// The operation code, 2 hex digits.
    public var op: String
    /// The argument, 2 hex digits.
    public var arg: String
    /// A double packet.
    public var double: Bool
    /// The four bytes of the second block of a double packet, 8 hex digits.
    public var extra: String
    /// The meaning of the packet.
    public var text: String

    init(_ c: sdrkit_mdc1200_burst_row) {
        key = sdrkitText(c.key)
        serial = c.serial
        timeUtc = c.has_time_utc ? c.time_utc : nil
        hz = c.hz
        label = sdrkitText(c.label)
        unitId = sdrkitText(c.unit_id)
        op = sdrkitText(c.op)
        arg = sdrkitText(c.arg)
        double = c.double_
        extra = sdrkitText(c.extra)
        text = sdrkitText(c.text)
    }
}

/// The result fields of the wireless microphone finder.
public struct WmicFields {
    /// Candidates now.
    public var candidates: Int64
    /// The name of the band plan.
    public var plan: String?
    /// The input rate.
    public var inputRateHz: Double
    /// The centre of the capture.
    public var centreHz: Double
    /// The signal time.
    public var signalTimeS: Double
    /// Reports made.
    public var reports: Int64

    init(_ c: sdrkit_wmic_fields) {
        candidates = c.candidates
        plan = c.has_plan ? sdrkitText(c.plan) : nil
        inputRateHz = c.input_rate_hz
        centreHz = c.centre_hz
        signalTimeS = c.signal_time_s
        reports = c.reports
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WmicFields, SDRKitCode> {
        var c = sdrkit_wmic_fields()
        let code = sdrkit_wmic_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WmicFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A carrier that can be a wireless microphone.
public struct WmicCandidateRow {
    /// The row key.
    public var key: String
    /// The carrier id of the finder.
    public var id: Int64
    /// Where to listen.
    public var hz: Double
    /// The occupied width.
    public var widthHz: Double
    /// The channel level.
    public var levelDbfs: Double
    /// The signal-to-noise ratio.
    public var snrDb: Double
    /// The peak deviation.
    public var deviationHz: Double?
    /// It looks like analogue FM.
    public var fm: Bool
    /// Above the threshold now.
    public var active: Bool
    /// This candidate plays.
    public var listening: Bool
    /// The time since it was first reported.
    public var ageS: Double
    /// The band of the plan that holds it.
    public var band: String

    init(_ c: sdrkit_wmic_candidate_row) {
        key = sdrkitText(c.key)
        id = c.id
        hz = c.hz
        widthHz = c.width_hz
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        fm = c.fm
        active = c.active
        listening = c.listening
        ageS = c.age_s
        band = sdrkitText(c.band)
    }
}

/// The result fields of the FleetSync burst decoder.
public struct FleetsyncFields {
    /// Bursts so far.
    public var bursts: Int64
    /// Frames found by their sync word whose check failed.
    public var checkFailures: Int64
    /// Fleet and unit pairs heard.
    public var units: Int64

    init(_ c: sdrkit_fleetsync_fields) {
        bursts = c.bursts
        checkFailures = c.check_failures
        units = c.units
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FleetsyncFields, SDRKitCode> {
        var c = sdrkit_fleetsync_fields()
        let code = sdrkit_fleetsync_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FleetsyncFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A FleetSync burst: a row of the burst log and the fields of its event.
public struct FleetsyncBurstRow {
    /// The row key.
    public var key: String
    /// The number of the burst.
    public var serial: Int64
    /// When it was decoded.
    public var timeUtc: Double?
    /// The frequency of the carrier.
    public var hz: Double
    /// The frequency in MHz, 4 decimals.
    public var label: String
    /// The fleet number.
    public var fleet: Int64
    /// The unit number.
    public var unit: Int64
    /// The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
    public var inRange: Bool
    /// "FleetSync" or "FleetSync II".
    public var kind: String
    /// The signalling rate.
    public var bitRate: Int64
    /// Word 1, 8 hex digits.
    public var word1: String
    /// Word 2, 8 hex digits.
    public var word2: String
    /// Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits.
    public var raw: String
    /// "good", or for FleetSync II the bits that the code repaired.
    public var check: String
    /// The burst in words.
    public var text: String

    init(_ c: sdrkit_fleetsync_burst_row) {
        key = sdrkitText(c.key)
        serial = c.serial
        timeUtc = c.has_time_utc ? c.time_utc : nil
        hz = c.hz
        label = sdrkitText(c.label)
        fleet = c.fleet
        unit = c.unit
        inRange = c.in_range
        kind = sdrkitText(c.kind)
        bitRate = c.bit_rate
        word1 = sdrkitText(c.word1)
        word2 = sdrkitText(c.word2)
        raw = sdrkitText(c.raw)
        check = sdrkitText(c.check)
        text = sdrkitText(c.text)
    }
}

/// A fleet and unit heard.
public struct FleetsyncUnitRow {
    /// The row key.
    public var key: String
    /// The fleet number.
    public var fleet: Int64
    /// The unit number.
    public var unit: Int64
    /// The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
    public var inRange: Bool
    /// Bursts heard.
    public var heard: Int64?
    /// When it was last heard.
    public var lastHeard: Double?
    /// The carrier of its last burst.
    public var hz: Double
    /// The type of its last burst.
    public var kind: String

    init(_ c: sdrkit_fleetsync_unit_row) {
        key = sdrkitText(c.key)
        fleet = c.fleet
        unit = c.unit
        inRange = c.in_range
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        hz = c.hz
        kind = sdrkitText(c.kind)
    }
}

/// A frame with a good FCS.
public struct HfpacketFrameEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The source of a frame.
    public var destination: String
    /// The digipeater path of a frame.
    public var path: String
    /// The frame type.
    public var type: String
    /// The information field as text.
    public var info: String
    /// The tuning correction.
    public var toneOffsetHz: Double

    init(_ c: sdrkit_hfpacket_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        source = sdrkitText(c.source)
        destination = sdrkitText(c.destination)
        path = sdrkitText(c.path)
        type = sdrkitText(c.type)
        info = sdrkitText(c.info)
        toneOffsetHz = c.tone_offset_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfpacketFrameEvent, SDRKitCode> {
        var c = sdrkit_hfpacket_frame_event()
        let code = sdrkit_hfpacket_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfpacketFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A station heard for the first time.
public struct HfpacketStationEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The station.
    public var source: String

    init(_ c: sdrkit_hfpacket_station_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        source = sdrkitText(c.source)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfpacketStationEvent, SDRKitCode> {
        var c = sdrkit_hfpacket_station_event()
        let code = sdrkit_hfpacket_station_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfpacketStationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An ELT tone found.
public struct Elt121FoundEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// Sweeps a second.
    public var sweepRateHz: Double
    /// The first frequency of a sweep.
    public var sweepTopHz: Double
    /// The lowest frequency of a sweep.
    public var sweepBottomHz: Double

    init(_ c: sdrkit_elt121_found_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        sweepRateHz = c.sweep_rate_hz
        sweepTopHz = c.sweep_top_hz
        sweepBottomHz = c.sweep_bottom_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Elt121FoundEvent, SDRKitCode> {
        var c = sdrkit_elt121_found_event()
        let code = sdrkit_elt121_found_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Elt121FoundEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An ELT tone ended.
public struct Elt121EndedEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_elt121_ended_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Elt121EndedEvent, SDRKitCode> {
        var c = sdrkit_elt121_ended_event()
        let code = sdrkit_elt121_ended_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Elt121EndedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Inverted speech found.
public struct VoiceInversionFoundEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The inversion carrier found.
    public var carrierHz: Double

    init(_ c: sdrkit_voice_inversion_found_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        carrierHz = c.carrier_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VoiceInversionFoundEvent, SDRKitCode> {
        var c = sdrkit_voice_inversion_found_event()
        let code = sdrkit_voice_inversion_found_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VoiceInversionFoundEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Speech that is not inverted.
public struct VoiceInversionClearEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_voice_inversion_clear_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VoiceInversionClearEvent, SDRKitCode> {
        var c = sdrkit_voice_inversion_clear_event()
        let code = sdrkit_voice_inversion_clear_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VoiceInversionClearEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A call that ended.
public struct VoiceChannelsCallEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of a call.
    public var serial: Int64
    /// The channel key.
    public var key: Int64
    /// The number of the plan.
    public var number: Int64
    /// The short label.
    public var label: String
    /// The signal time a call started.
    public var startS: Double
    /// How long a call lasted.
    public var durationS: Double
    /// The highest level.
    public var peakDbfs: Double
    /// The mean level of a call.
    public var meanDbfs: Double
    /// The demodulator of a call.
    public var demod: String
    /// The tone or code heard.
    public var tone: String

    init(_ c: sdrkit_voice_channels_call_event) {
        time = c.time
        freqHz = c.freq_hz
        serial = c.serial
        key = c.key
        number = c.number
        label = sdrkitText(c.label)
        startS = c.start_s
        durationS = c.duration_s
        peakDbfs = c.peak_dbfs
        meanDbfs = c.mean_dbfs
        demod = sdrkitText(c.demod)
        tone = sdrkitText(c.tone)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VoiceChannelsCallEvent, SDRKitCode> {
        var c = sdrkit_voice_channels_call_event()
        let code = sdrkit_voice_channels_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VoiceChannelsCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A call that ended.
public struct Mdc1200CallEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of a call.
    public var serial: Int64
    /// The channel key.
    public var key: Int64
    /// The number of the plan.
    public var number: Int64
    /// The short label.
    public var label: String
    /// The signal time a call started.
    public var startS: Double
    /// How long a call lasted.
    public var durationS: Double
    /// The highest level.
    public var peakDbfs: Double
    /// The mean level of a call.
    public var meanDbfs: Double
    /// The demodulator of a call.
    public var demod: String
    /// The tone or code heard.
    public var tone: String

    init(_ c: sdrkit_mdc1200_call_event) {
        time = c.time
        freqHz = c.freq_hz
        serial = c.serial
        key = c.key
        number = c.number
        label = sdrkitText(c.label)
        startS = c.start_s
        durationS = c.duration_s
        peakDbfs = c.peak_dbfs
        meanDbfs = c.mean_dbfs
        demod = sdrkitText(c.demod)
        tone = sdrkitText(c.tone)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Mdc1200CallEvent, SDRKitCode> {
        var c = sdrkit_mdc1200_call_event()
        let code = sdrkit_mdc1200_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Mdc1200CallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst that was decoded.
public struct Mdc1200BurstEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of a call.
    public var serial: Int64
    /// When a call started.
    public var timeUtc: Double?
    /// Where to listen.
    public var hz: Double
    /// The short label.
    public var label: String
    /// The unit id, 4 hex digits.
    public var unitId: String
    /// The operation code, 2 hex digits.
    public var op: String
    /// The argument, 2 hex digits.
    public var arg: String
    /// A double packet.
    public var double: Bool
    /// The four bytes of the second block of a double packet, 8 hex digits.
    public var extra: String
    /// The meaning of the packet.
    public var text: String

    init(_ c: sdrkit_mdc1200_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        serial = c.serial
        timeUtc = c.has_time_utc ? c.time_utc : nil
        hz = c.hz
        label = sdrkitText(c.label)
        unitId = sdrkitText(c.unit_id)
        op = sdrkitText(c.op)
        arg = sdrkitText(c.arg)
        double = c.double_
        extra = sdrkitText(c.extra)
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Mdc1200BurstEvent, SDRKitCode> {
        var c = sdrkit_mdc1200_burst_event()
        let code = sdrkit_mdc1200_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Mdc1200BurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A new candidate carrier.
public struct WmicCandidateEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The carrier id of the finder.
    public var id: Int64
    /// Where to listen.
    public var hz: Double
    /// The occupied width.
    public var widthHz: Double
    /// The channel level.
    public var levelDbfs: Double
    /// The signal-to-noise ratio.
    public var snrDb: Double
    /// The peak deviation.
    public var deviationHz: Double?
    /// It looks like analogue FM.
    public var fm: Bool
    /// The band of the plan that holds it.
    public var band: String

    init(_ c: sdrkit_wmic_candidate_event) {
        time = c.time
        freqHz = c.freq_hz
        id = c.id
        hz = c.hz
        widthHz = c.width_hz
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        fm = c.fm
        band = sdrkitText(c.band)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WmicCandidateEvent, SDRKitCode> {
        var c = sdrkit_wmic_candidate_event()
        let code = sdrkit_wmic_candidate_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WmicCandidateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A call that ended.
public struct FleetsyncCallEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of a call.
    public var serial: Int64
    /// The channel key.
    public var key: Int64
    /// The number of the plan.
    public var number: Int64
    /// The short label.
    public var label: String
    /// The signal time a call started.
    public var startS: Double
    /// How long a call lasted.
    public var durationS: Double
    /// The highest level.
    public var peakDbfs: Double
    /// The mean level of a call.
    public var meanDbfs: Double
    /// The demodulator of a call.
    public var demod: String
    /// The tone or code heard.
    public var tone: String

    init(_ c: sdrkit_fleetsync_call_event) {
        time = c.time
        freqHz = c.freq_hz
        serial = c.serial
        key = c.key
        number = c.number
        label = sdrkitText(c.label)
        startS = c.start_s
        durationS = c.duration_s
        peakDbfs = c.peak_dbfs
        meanDbfs = c.mean_dbfs
        demod = sdrkitText(c.demod)
        tone = sdrkitText(c.tone)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FleetsyncCallEvent, SDRKitCode> {
        var c = sdrkit_fleetsync_call_event()
        let code = sdrkit_fleetsync_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FleetsyncCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A FleetSync burst: a row of the burst log and the fields of its event.
public struct FleetsyncBurstEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the burst.
    public var serial: Int64
    /// When it was decoded.
    public var timeUtc: Double?
    /// The frequency of the carrier.
    public var hz: Double
    /// The frequency in MHz, 4 decimals.
    public var label: String
    /// The fleet number.
    public var fleet: Int64
    /// The unit number.
    public var unit: Int64
    /// The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
    public var inRange: Bool
    /// "FleetSync" or "FleetSync II".
    public var kind: String
    /// The signalling rate.
    public var bitRate: Int64
    /// Word 1, 8 hex digits.
    public var word1: String
    /// Word 2, 8 hex digits.
    public var word2: String
    /// Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits.
    public var raw: String
    /// "good", or for FleetSync II the bits that the code repaired.
    public var check: String
    /// The burst in words.
    public var text: String

    init(_ c: sdrkit_fleetsync_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        serial = c.serial
        timeUtc = c.has_time_utc ? c.time_utc : nil
        hz = c.hz
        label = sdrkitText(c.label)
        fleet = c.fleet
        unit = c.unit
        inRange = c.in_range
        kind = sdrkitText(c.kind)
        bitRate = c.bit_rate
        word1 = sdrkitText(c.word1)
        word2 = sdrkitText(c.word2)
        raw = sdrkitText(c.raw)
        check = sdrkitText(c.check)
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FleetsyncBurstEvent, SDRKitCode> {
        var c = sdrkit_fleetsync_burst_event()
        let code = sdrkit_fleetsync_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FleetsyncBurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum VoiceComponents {
    /// The result fields of the component "guard" of the stack "hfpacket" in the snapshot of `handle`.
    public static func hfpacketGuardFields(_ handle: SDRKIT_HANDLE) -> Result<VoiceGuardFields, SDRKitCode> {
        var c = sdrkit_voice_guard_fields()
        let code = sdrkit_hfpacket_guard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VoiceGuardFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "guard" of the stack "elt121" in the snapshot of `handle`.
    public static func elt121GuardFields(_ handle: SDRKIT_HANDLE) -> Result<VoiceGuardFields, SDRKitCode> {
        var c = sdrkit_voice_guard_fields()
        let code = sdrkit_elt121_guard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VoiceGuardFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "guard" of the stack "voice_inversion" in the snapshot of `handle`.
    public static func voiceInversionGuardFields(_ handle: SDRKIT_HANDLE) -> Result<VoiceGuardFields, SDRKitCode> {
        var c = sdrkit_voice_guard_fields()
        let code = sdrkit_voice_inversion_guard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VoiceGuardFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum VoiceTables {
    /// The changed rows of the table "events" of the stack "hfpacket", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `text`. Time: `time_utc`.
    public static func hfpacketEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[VoiceDetectorEventRow], SDRKitCode> {
        var rows = [sdrkit_voice_detector_event_row](repeating: sdrkit_voice_detector_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfpacket_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceDetectorEventRow.init))
    }
    /// The changed rows of the table "frames" of the stack "hfpacket", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_utc`.
    public static func hfpacketFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[HfpacketFrameRow], SDRKitCode> {
        var rows = [sdrkit_hfpacket_frame_row](repeating: sdrkit_hfpacket_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfpacket_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfpacketFrameRow.init))
    }
    /// The changed rows of the table "events" of the stack "elt121", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `text`. Time: `time_utc`.
    public static func elt121Events(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[VoiceDetectorEventRow], SDRKitCode> {
        var rows = [sdrkit_voice_detector_event_row](repeating: sdrkit_voice_detector_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_elt121_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceDetectorEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "voice_inversion", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `text`. Time: `time_utc`.
    public static func voiceInversionEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[VoiceDetectorEventRow], SDRKitCode> {
        var rows = [sdrkit_voice_detector_event_row](repeating: sdrkit_voice_detector_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_voice_inversion_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceDetectorEventRow.init))
    }
    /// The changed rows of the table "calls" of the stack "voice_channels", newest first, at most `capacity` rows.
    /// The rows are the calls of the stack.
    /// The column `label` names a row.
    /// Time: `time_utc`.
    public static func voiceChannelsCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[VoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_voice_call_row](repeating: sdrkit_voice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_voice_channels_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceCallRow.init))
    }
    /// The changed rows of the table "channels" of the stack "voice_channels", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard_utc`.
    public static func voiceChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[VoiceChannelRow], SDRKitCode> {
        var rows = [sdrkit_voice_channel_row](repeating: sdrkit_voice_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_voice_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceChannelRow.init))
    }
    /// The changed rows of the table "calls" of the stack "mdc1200", newest first, at most `capacity` rows.
    /// The rows are the calls of the stack.
    /// The column `label` names a row.
    /// Time: `time_utc`.
    public static func mdc1200Calls(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[VoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_voice_call_row](repeating: sdrkit_voice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mdc1200_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceCallRow.init))
    }
    /// The changed rows of the table "channels" of the stack "mdc1200", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard_utc`.
    public static func mdc1200Channels(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[VoiceChannelRow], SDRKitCode> {
        var rows = [sdrkit_voice_channel_row](repeating: sdrkit_voice_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mdc1200_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceChannelRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "mdc1200", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `label` names a row.
    /// Sender: `unit_id`. Text: `text`. Time: `time_utc`.
    public static func mdc1200Bursts(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[Mdc1200BurstRow], SDRKitCode> {
        var rows = [sdrkit_mdc1200_burst_row](repeating: sdrkit_mdc1200_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mdc1200_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Mdc1200BurstRow.init))
    }
    /// The changed rows of the table "candidates" of the stack "wmic", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func wmicCandidates(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WmicCandidateRow], SDRKitCode> {
        var rows = [sdrkit_wmic_candidate_row](repeating: sdrkit_wmic_candidate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wmic_candidates_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WmicCandidateRow.init))
    }
    /// The changed rows of the table "calls" of the stack "fleetsync", newest first, at most `capacity` rows.
    /// The rows are the calls of the stack.
    /// The column `label` names a row.
    /// Time: `time_utc`.
    public static func fleetsyncCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[VoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_voice_call_row](repeating: sdrkit_voice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fleetsync_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceCallRow.init))
    }
    /// The changed rows of the table "channels" of the stack "fleetsync", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard_utc`.
    public static func fleetsyncChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[VoiceChannelRow], SDRKitCode> {
        var rows = [sdrkit_voice_channel_row](repeating: sdrkit_voice_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fleetsync_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VoiceChannelRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "fleetsync", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `label` names a row.
    /// Sender: `unit`. Text: `text`. Time: `time_utc`.
    public static func fleetsyncBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[FleetsyncBurstRow], SDRKitCode> {
        var rows = [sdrkit_fleetsync_burst_row](repeating: sdrkit_fleetsync_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fleetsync_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FleetsyncBurstRow.init))
    }
    /// The changed rows of the table "units" of the stack "fleetsync", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// Time: `last_heard`.
    public static func fleetsyncUnits(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[FleetsyncUnitRow], SDRKitCode> {
        var rows = [sdrkit_fleetsync_unit_row](repeating: sdrkit_fleetsync_unit_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fleetsync_units_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FleetsyncUnitRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum VoiceCommands {
    /// Clear. Forgets everything decoded so far.
    public static func hfpacketClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfpacket_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func elt121Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_elt121_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func voiceInversionClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_voice_inversion_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func voiceChannelsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_voice_channels_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func mdc1200Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mdc1200_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func wmicClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wmic_clear(handle)
        return sdrkitResult(code)
    }
    /// Listen. Plays a candidate. The candidate that plays stops.
    public static func wmicListen(_ handle: SDRKIT_HANDLE, id: Int64) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wmic_listen_args()
        args.size = UInt32(MemoryLayout<sdrkit_wmic_listen_args>.size)
        let code: Int32 = { () -> Int32 in
        args.id = id
        return sdrkit_wmic_listen(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func fleetsyncClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fleetsync_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum VoiceSettings {
    /// The options of the setting "sideband" of the stack "hfpacket".
    public enum HfpacketSideband: Int32 {
        case usb = 0
        case lsb = 1
    }
    /// Sideband.
    public static func hfpacketSideband(_ handle: SDRKIT_HANDLE) -> Result<HfpacketSideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sideband", &v)
        return code == SDRKIT_OK ? .success(HfpacketSideband(rawValue: v) ?? HfpacketSideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfpacketSideband(_ handle: SDRKIT_HANDLE, _ value: HfpacketSideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sideband", value.rawValue))
    }
    /// Carrier (Hz, 0 = automatic).
    public static func voiceInversionCarrierHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "carrier_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceInversionCarrierHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "carrier_hz", value))
    }
    /// Channel plan.
    public static func voiceChannelsPlan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsPlan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "plan", $0) })
    }
    /// Raster step.
    public static func voiceChannelsRasterStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsRasterStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value))
    }
    /// Raster origin.
    public static func voiceChannelsRasterOriginHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsRasterOriginHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value))
    }
    /// Channel width.
    public static func voiceChannelsChannelWidthHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_width_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsChannelWidthHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_width_hz", value))
    }
    /// Channel.
    public static func voiceChannelsChannel(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsChannel(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channel", $0) })
    }
    /// The options of the setting "demod" of the stack "voice_channels".
    public enum VoiceChannelsDemod: Int32 {
        case auto = 0
        case fm = 1
        case am = 2
        case usb = 3
        case lsb = 4
    }
    /// Demodulator.
    public static func voiceChannelsDemod(_ handle: SDRKIT_HANDLE) -> Result<VoiceChannelsDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(VoiceChannelsDemod(rawValue: v) ?? VoiceChannelsDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsDemod(_ handle: SDRKIT_HANDLE, _ value: VoiceChannelsDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Squelch.
    public static func voiceChannelsSquelch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsSquelch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch", value))
    }
    /// De-emphasis.
    public static func voiceChannelsDeemphasis(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deemphasis", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsDeemphasis(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deemphasis", value))
    }
    /// Scan.
    public static func voiceChannelsScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Channels.
    public static func voiceChannelsScanChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "scan_channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsScanChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "scan_channels", $0) })
    }
    /// Dwell.
    public static func voiceChannelsScanDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsScanDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_dwell_s", value))
    }
    /// Hang.
    public static func voiceChannelsScanHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsScanHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_hang_s", value))
    }
    /// Carriers at once.
    public static func voiceChannelsCarriers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsCarriers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "carriers", value))
    }
    /// Only heard.
    public static func voiceChannelsShowHeard(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "show_heard", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsShowHeard(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "show_heard", value))
    }
    /// Time of the first sample.
    public static func voiceChannelsTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
    /// Raster step.
    public static func mdc1200RasterStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200RasterStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value))
    }
    /// Raster origin.
    public static func mdc1200RasterOriginHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200RasterOriginHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value))
    }
    /// Channel width.
    public static func mdc1200ChannelWidthHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_width_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200ChannelWidthHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_width_hz", value))
    }
    /// Squelch.
    public static func mdc1200Squelch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200Squelch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch", value))
    }
    /// De-emphasis.
    public static func mdc1200Deemphasis(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deemphasis", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200Deemphasis(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deemphasis", value))
    }
    /// Carriers at once.
    public static func mdc1200Carriers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200Carriers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "carriers", value))
    }
    /// Time of the first sample.
    public static func mdc1200TimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200TimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
    /// Band plan.
    public static func wmicPlan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicPlan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "plan", $0) })
    }
    /// Raster step.
    public static func wmicRasterStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicRasterStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value))
    }
    /// Raster origin.
    public static func wmicRasterOriginHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicRasterOriginHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value))
    }
    /// Squelch.
    public static func wmicSquelch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicSquelch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch", value))
    }
    /// Carriers at once.
    public static func wmicCarriers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicCarriers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "carriers", value))
    }
    /// Listen.
    public static func wmicListen(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicListen(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "listen", value))
    }
    /// Raster step.
    public static func fleetsyncRasterStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncRasterStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value))
    }
    /// Raster origin.
    public static func fleetsyncRasterOriginHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncRasterOriginHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value))
    }
    /// Channel width.
    public static func fleetsyncChannelWidthHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_width_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncChannelWidthHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_width_hz", value))
    }
    /// Squelch.
    public static func fleetsyncSquelch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncSquelch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch", value))
    }
    /// De-emphasis.
    public static func fleetsyncDeemphasis(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deemphasis", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncDeemphasis(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deemphasis", value))
    }
    /// Carriers at once.
    public static func fleetsyncCarriers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncCarriers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "carriers", value))
    }
    /// The options of the setting "bit_rate" of the stack "fleetsync".
    public enum FleetsyncBitRate: Int32 {
        case auto = 0
        case n1200BitS = 1
        case n2400BitS = 2
    }
    /// Signalling rate.
    public static func fleetsyncBitRate(_ handle: SDRKIT_HANDLE) -> Result<FleetsyncBitRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bit_rate", &v)
        return code == SDRKIT_OK ? .success(FleetsyncBitRate(rawValue: v) ?? FleetsyncBitRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncBitRate(_ handle: SDRKIT_HANDLE, _ value: FleetsyncBitRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bit_rate", value.rawValue))
    }
    /// Time of the first sample.
    public static func fleetsyncTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum VoiceGeneratorSettings {
    /// Test signal: frames.
    public static func hfpacketFrames(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfpacketFrames(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_frames", value))
    }
    /// Test signal: lead.
    public static func hfpacketLeadS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lead_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfpacketLeadS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lead_s", value))
    }
    /// Test signal: tail.
    public static func hfpacketTailS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tail_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfpacketTailS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tail_s", value))
    }
    /// Test signal: mark.
    public static func hfpacketMarkHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_mark_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfpacketMarkHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_mark_hz", value))
    }
    /// Test signal: space.
    public static func hfpacketSpaceHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_space_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfpacketSpaceHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_space_hz", value))
    }
    /// Test signal: sweeps a second.
    public static func elt121SweepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_sweep_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElt121SweepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_sweep_hz", value))
    }
    /// Test signal: duty.
    public static func elt121DutyPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_duty_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElt121DutyPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_duty_pct", value))
    }
    /// Test signal: from.
    public static func elt121TopHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_top_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElt121TopHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_top_hz", value))
    }
    /// Test signal: to.
    public static func elt121BottomHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bottom_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElt121BottomHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bottom_hz", value))
    }
    /// Test signal: depth.
    public static func elt121Depth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElt121Depth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// Test signal: length.
    public static func elt121Seconds(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_seconds", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElt121Seconds(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_seconds", value))
    }
    /// Test signal: inversion carrier.
    public static func voiceInversionInvertHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_invert_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceInversionInvertHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_invert_hz", value))
    }
    /// Test signal: length.
    public static func voiceInversionSeconds(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_seconds", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceInversionSeconds(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_seconds", value))
    }
    /// Test signal: deviation.
    public static func voiceInversionDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceInversionDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// Test signal C/N.
    public static func voiceChannelsCnrDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsCnrDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value))
    }
    /// The options of the setting "gen_scene" of the stack "voice_channels".
    public enum VoiceChannelsScene: Int32 {
        case auto = 0
        case mixed = 1
        case amFmSsb = 2
        case fmTones = 3
        case sideband = 4
    }
    /// Test signal scene.
    public static func voiceChannelsScene(_ handle: SDRKIT_HANDLE) -> Result<VoiceChannelsScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(VoiceChannelsScene(rawValue: v) ?? VoiceChannelsScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setVoiceChannelsScene(_ handle: SDRKIT_HANDLE, _ value: VoiceChannelsScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal C/N.
    public static func mdc1200CnrDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMdc1200CnrDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value))
    }
    /// Test signal C/N.
    public static func wmicCnrDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmicCnrDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value))
    }
    /// Test signal C/N.
    public static func fleetsyncCnrDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncCnrDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value))
    }
    /// Test signal fleet.
    public static func fleetsyncFleet(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fleet", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncFleet(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fleet", value))
    }
    /// Test signal unit.
    public static func fleetsyncUnit(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_unit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncUnit(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_unit", value))
    }
    /// The options of the setting "gen_bit_rate" of the stack "fleetsync".
    public enum FleetsyncBitRate: Int32 {
        case n1200BitS = 0
        case n2400BitS = 1
    }
    /// Test signal rate.
    public static func fleetsyncBitRate(_ handle: SDRKIT_HANDLE) -> Result<FleetsyncBitRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bit_rate", &v)
        return code == SDRKIT_OK ? .success(FleetsyncBitRate(rawValue: v) ?? FleetsyncBitRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncBitRate(_ handle: SDRKIT_HANDLE, _ value: FleetsyncBitRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bit_rate", value.rawValue))
    }
    /// The options of the setting "gen_frame" of the stack "fleetsync".
    public enum FleetsyncFrame: Int32 {
        case both = 0
        case plain = 1
        case fs2 = 2
    }
    /// Test signal frames.
    public static func fleetsyncFrame(_ handle: SDRKIT_HANDLE) -> Result<FleetsyncFrame, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_frame", &v)
        return code == SDRKIT_OK ? .success(FleetsyncFrame(rawValue: v) ?? FleetsyncFrame(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetsyncFrame(_ handle: SDRKIT_HANDLE, _ value: FleetsyncFrame) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_frame", value.rawValue))
    }
}

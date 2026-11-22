// sdrkit: the typed interface of the stack family "wsjt", version 3.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum WsjtInterface {
    public static let major = 3
    public static let minor = 0
}

/// The result fields of a slot stack: the slot clock, the clock estimate from the decodes, and the counts since Clear.
public struct WsjtFields {
    public enum TimeSource: Int32 {
        case systemClock = 0
        case recordingStart = 1
        case searching = 2
        case foundFromSignals = 3
    }
    public enum Js8State: Int32 {
        case listening = 0
        case slots = 1
        case frames = 2
    }
    /// The time source of the slot clock. It has no value before the clock reports.
    public var timeSource: TimeSource?
    /// The UTC of the newest audio.
    public var utc: Double
    /// The median DT of the recent decodes. It has no value without a recent decode.
    public var clockErrorS: Double?
    /// The spread of the recent DTs. It has no value without a recent decode.
    public var clockSpreadS: Double?
    /// The decodes that the clock estimate uses.
    public var clockDecodes: Int64
    /// The clock is more than 1 s off.
    public var clockWarning: Bool
    /// Decodes since Clear.
    public var decodesTotal: Int64
    /// JS8 frames since Clear.
    public var js8Frames: Int64
    /// JS8 candidates that did not decode since Clear.
    public var js8Failed: Int64
    /// What JS8 has heard.
    public var js8State: Js8State?
    /// The sequence number of the newest line of the JS8 waterfall. It has no value before the first line.
    public var js8WaterfallSeq: Int64?

    init(_ c: sdrkit_wsjt_fields) {
        timeSource = c.has_time_source ? TimeSource(rawValue: c.time_source) : nil
        utc = c.utc
        clockErrorS = c.has_clock_error_s ? c.clock_error_s : nil
        clockSpreadS = c.has_clock_spread_s ? c.clock_spread_s : nil
        clockDecodes = c.clock_decodes
        clockWarning = c.clock_warning
        decodesTotal = c.decodes_total
        js8Frames = c.js8_frames
        js8Failed = c.js8_failed
        js8State = Js8State(rawValue: c.js8_state)
        js8WaterfallSeq = c.has_js8_waterfall_seq ? c.js8_waterfall_seq : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WsjtFields, SDRKitCode> {
        var c = sdrkit_wsjt_fields()
        let code = sdrkit_wsjt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the decoder of one mode (`dec_<mode>`).
public struct WsjtSlotDecoderFields {
    /// The slots decoded.
    public var slots: Int64
    /// The decodes.
    public var decodes: Int64
    /// The time that the last slot took to decode.
    public var decodeMs: Double

    init(_ c: sdrkit_wsjt_slot_decoder_fields) {
        slots = c.slots
        decodes = c.decodes
        decodeMs = c.decode_ms
    }
}

/// The result fields of whole-span decoding and the multi-band monitor (`wide`).
public struct WsjtWideFields {
    /// The dials decoded.
    public var channels: Int64
    /// The share of real time that decoding takes.
    public var loadPct: Double
    /// Decoding does not keep up.
    public var tooSlow: Bool

    init(_ c: sdrkit_wsjt_wide_fields) {
        channels = c.channels
        loadPct = c.load_pct
        tooSlow = c.too_slow
    }
}

/// A row of the decode table. The row key is the place of the decode in the log.
public struct WsjtDecodeRow {
    public enum How: Int32 {
        case alone = 0
        case combined = 1
        case aPriori = 2
    }
    /// The row key.
    public var key: String
    /// The UTC of the slot.
    public var utc: Double
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The time offset from the slot start.
    public var dtS: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The amateur band of the dial, if the dial is in one.
    public var band: String?
    /// The dial frequency, if the slot has one.
    public var dialHz: Double?
    /// The mode with its sub-mode and period ("JT65B").
    public var mode: String
    /// The decoded message.
    public var message: String
    /// The decode was found on the other sideband.
    public var mirrored: Bool
    /// How the decode was made.
    public var how: How?
    /// The decode is from the first part of a slot. The decode of the whole slot replaces it.
    public var early: Bool

    init(_ c: sdrkit_wsjt_decode_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        snrDb = c.snr_db
        dtS = c.dt_s
        audioHz = c.audio_hz
        band = c.has_band ? sdrkitText(c.band) : nil
        dialHz = c.has_dial_hz ? c.dial_hz : nil
        mode = sdrkitText(c.mode)
        message = sdrkitText(c.message)
        mirrored = c.mirrored
        how = How(rawValue: c.how)
        early = c.early
    }
}

/// A row of the WSPR and FST4W spots. The row key is the place of the spot in the log.
public struct WsprSpotRow {
    /// The row key.
    public var key: String
    /// The UTC of the slot.
    public var utc: Double
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The time offset from the slot start.
    public var dtS: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The callsign of the beacon.
    public var callsign: String
    /// The locator of the beacon.
    public var locator: String
    /// The power that the beacon states.
    public var powerDbm: Int64
    /// The decoded message.
    public var message: String

    init(_ c: sdrkit_wspr_spot_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        snrDb = c.snr_db
        dtS = c.dt_s
        audioHz = c.audio_hz
        callsign = sdrkitText(c.callsign)
        locator = sdrkitText(c.locator)
        powerDbm = c.power_dbm
        message = sdrkitText(c.message)
    }
}

/// A row of the modes table: the counters and the state of one mode heard, or of one JS8 speed. The row key is the mode id, or `js8n`, `js8f` and `js8t`.
public struct WsjtModeRow {
    public enum State: Int32 {
        case decoding = 0
        case noTable = 1
        case noDecoder = 2
    }
    /// The row key.
    public var key: String
    /// The mode with its sub-mode and period.
    public var mode: String
    /// The T/R period.
    public var period: Double
    /// The slots decoded.
    public var slots: Int64
    /// Decodes since Clear.
    public var decodes: Int64
    /// The decodes of the last slot.
    public var lastCount: Int64
    /// The UTC of the last slot decoded. It has no value before the first slot.
    public var utc: Double?
    /// The time that the last slot took to decode.
    public var decodeMs: Double
    /// Whether the mode decodes.
    public var state: State?

    init(_ c: sdrkit_wsjt_mode_row) {
        key = sdrkitText(c.key)
        mode = sdrkitText(c.mode)
        period = c.period
        slots = c.slots
        decodes = c.decodes
        lastCount = c.last_count
        utc = c.has_utc ? c.utc : nil
        decodeMs = c.decode_ms
        state = State(rawValue: c.state)
    }
}

/// A row of the station table: a station heard in any mode. The row key is the callsign.
public struct WsjtStationRow {
    public enum Mode: Int32 {
        case ft8 = 0
        case ft4 = 1
        case ft2 = 2
        case wspr = 3
        case jt65 = 4
        case jt9 = 5
        case jt4 = 6
        case q65 = 7
        case fst4 = 8
        case fst4w = 9
        case msk144 = 10
        case js8 = 11
    }
    /// The row key.
    public var key: String
    /// The callsign.
    public var callsign: String
    /// The locator, if one was heard.
    public var locator: String?
    /// The mode of the newest decode.
    public var mode: Mode?
    /// The SNR of the newest decode.
    public var snrDb: Double
    /// The radio frequency of the newest decode.
    public var freqHz: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last decode.
    public var lastHeard: Double?

    init(_ c: sdrkit_wsjt_station_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        mode = Mode(rawValue: c.mode)
        snrDb = c.snr_db
        freqHz = c.freq_hz
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the JS8 frames. The row key is the place of the frame in the log.
public struct Js8FrameRow {
    public enum Submode: Int32 {
        case normal = 0
        case fast = 1
        case turbo = 2
    }
    /// The row key.
    public var key: String
    /// The UTC of the slot.
    public var utc: Double
    /// The speed of the frame.
    public var submode: Submode?
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The time offset from the slot start.
    public var dtS: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The text of the frame.
    public var frame: String

    init(_ c: sdrkit_js8_frame_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        submode = c.has_submode ? Submode(rawValue: c.submode) : nil
        snrDb = c.snr_db
        dtS = c.dt_s
        audioHz = c.audio_hz
        frame = sdrkitText(c.frame)
    }
}

/// A row of the JS8 messages. The row key is the place of the message in the log.
public struct Js8MessageRow {
    /// The row key.
    public var key: String
    /// The UTC of the first frame.
    public var utc: Double
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The message.
    public var message: String

    init(_ c: sdrkit_js8_message_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        snrDb = c.snr_db
        audioHz = c.audio_hz
        message = sdrkitText(c.message)
    }
}

/// A row of the JS8 stations. The row key is the callsign.
public struct Js8StationRow {
    /// The row key.
    public var key: String
    /// The callsign.
    public var callsign: String
    /// The locator, if one was heard.
    public var locator: String?
    /// The SNR of the newest frame.
    public var snrDb: Double
    /// The audio frequency of the newest frame.
    public var audioHz: Double
    /// The UTC of the newest frame.
    public var utc: Double
    /// The newest text of the station.
    public var lastText: String
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_js8_station_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        snrDb = c.snr_db
        audioHz = c.audio_hz
        utc = c.utc
        lastText = sdrkitText(c.last_text)
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the multi-band monitor: one band and mode of the selection. The row key is the band index in two digits and the mode index.
public struct WsjtMultibandRow {
    public enum Mode: Int32 {
        case ft8 = 0
        case ft4 = 1
        case ft2 = 2
        case wspr = 3
        case jt65 = 4
        case jt9 = 5
        case jt4 = 6
        case q65 = 7
        case fst4 = 8
        case fst4w = 9
        case msk144 = 10
        case js8 = 11
    }
    /// The row key.
    public var key: String
    /// The band.
    public var band: String
    /// The mode.
    public var mode: Mode?
    /// The dial of the mode on the band, if it has one.
    public var dialHz: Double?
    /// A decoder runs on the dial.
    public var running: Bool
    /// The decodes of the dial.
    public var decodes: Int64
    /// The capture covers the dial.
    public var covered: Bool
    /// The span that the capture needs around its centre to cover the dial. It has no value for a covered dial.
    public var spanNeededHz: Double?

    init(_ c: sdrkit_wsjt_multiband_row) {
        key = sdrkitText(c.key)
        band = sdrkitText(c.band)
        mode = Mode(rawValue: c.mode)
        dialHz = c.has_dial_hz ? c.dial_hz : nil
        running = c.running
        decodes = c.decodes
        covered = c.covered
        spanNeededHz = c.has_span_needed_hz ? c.span_needed_hz : nil
    }
}

/// The result fields of band hopping (`hop`).
public struct WsjtHopFields {
    /// The band of the present hop slot. It has no value when no band is planned or hopping is off.
    public var band: String?
    /// The UTC of the next band change. It has no value while hopping is off.
    public var nextUtc: Double?

    init(_ c: sdrkit_wsjt_hop_fields) {
        band = c.has_band ? sdrkitText(c.band) : nil
        nextUtc = c.has_next_utc ? c.next_utc : nil
    }
}

/// The result fields that each of `fsk441`, `jt6m`, `iscat` and `pi4` shows.
public struct WsjtScatterFields {
    /// The decodes since the last clear.
    public var decodesTotal: Int64
    /// What no public document confirms.
    public var note: String
    /// The input level. These stacks do not measure it: it has no value.
    public var levelDbfs: Double?

    init(_ c: sdrkit_wsjt_scatter_fields) {
        decodesTotal = c.decodes_total
        note = sdrkitText(c.note)
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WsjtScatterFields, SDRKitCode> {
        var c = sdrkit_wsjt_scatter_fields()
        let code = sdrkit_wsjt_scatter_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtScatterFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields that `fsk441` adds.
public struct Fsk441Fields {
    /// The level of the burst finder: the last frame over the noise.
    public var burstLevelDb: Double
    /// The bursts found.
    public var bursts: Int64

    init(_ c: sdrkit_fsk441_fields) {
        burstLevelDb = c.burst_level_db
        bursts = c.bursts
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Fsk441Fields, SDRKitCode> {
        var c = sdrkit_fsk441_fields()
        let code = sdrkit_fsk441_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Fsk441Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct WsjtDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_wsjt_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WsjtDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_decoder_fields()
        let code = sdrkit_wsjt_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the decode table of `fsk441`. The row key is the number of the decode.
public struct Fsk441DecodeRow {
    /// The row key.
    public var key: String
    /// The time of the start of the decode.
    public var utc: Double
    /// The signal time of the start of the decode, from the first sample.
    public var timeS: Double
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var message: String
    /// The contrast of the decode. It is no calibrated SNR.
    public var contrastDb: Double
    /// The length of the burst.
    public var durationMs: Double

    init(_ c: sdrkit_fsk441_decode_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        timeS = c.time_s
        snrDb = c.snr_db
        audioHz = c.audio_hz
        mode = sdrkitText(c.mode)
        message = sdrkitText(c.message)
        contrastDb = c.contrast_db
        durationMs = c.duration_ms
    }
}

/// The result fields that `jt6m` adds. No field has a value before the first ping.
public struct Jt6mFields {
    /// The folded message of the newest window that gave one.
    public var folded: String?
    /// The period of the folded message, in characters.
    public var foldPeriod: Int64?
    /// The offset of the sync tone of the newest ping.
    public var tuningHz: Double?

    init(_ c: sdrkit_jt6m_fields) {
        folded = c.has_folded ? sdrkitText(c.folded) : nil
        foldPeriod = c.has_fold_period ? c.fold_period : nil
        tuningHz = c.has_tuning_hz ? c.tuning_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Jt6mFields, SDRKitCode> {
        var c = sdrkit_jt6m_fields()
        let code = sdrkit_jt6m_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Jt6mFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ping table of `jt6m`. The row key is the number of the ping.
public struct Jt6mDecodeRow {
    /// The row key.
    public var key: String
    /// The time of the start of the decode.
    public var utc: Double
    /// The signal time of the start of the decode, from the first sample.
    public var timeS: Double
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var message: String
    /// The length of the ping.
    public var durationS: Double
    /// The clear frames of the ping.
    public var clearFrames: Int64

    init(_ c: sdrkit_jt6m_decode_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        timeS = c.time_s
        snrDb = c.snr_db
        audioHz = c.audio_hz
        mode = sdrkitText(c.mode)
        message = sdrkitText(c.message)
        durationS = c.duration_s
        clearFrames = c.clear_frames
    }
}

/// The result fields that `iscat` adds.
public struct IscatFields {
    /// The variant of the newest decode. No value before the first decode.
    public var variant: String?

    init(_ c: sdrkit_iscat_fields) {
        variant = c.has_variant ? sdrkitText(c.variant) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IscatFields, SDRKitCode> {
        var c = sdrkit_iscat_fields()
        let code = sdrkit_iscat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IscatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the decode table of `iscat`. The row key is the number of the decode.
public struct IscatDecodeRow {
    /// The row key.
    public var key: String
    /// The time of the start of the decode.
    public var utc: Double
    /// The signal time of the start of the decode, from the first sample.
    public var timeS: Double
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var message: String
    /// The variant of the decode.
    public var variant: String
    /// The sync ratio r.
    public var syncRatio: Double
    /// The whole frames used.
    public var frames: Int64
    /// The characters shown as `?`.
    public var unclear: Int64

    init(_ c: sdrkit_iscat_decode_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        timeS = c.time_s
        snrDb = c.snr_db
        audioHz = c.audio_hz
        mode = sdrkitText(c.mode)
        message = sdrkitText(c.message)
        variant = sdrkitText(c.variant)
        syncRatio = c.sync_ratio
        frames = c.frames
        unclear = c.unclear
    }
}

/// A row of the decode table of `pi4`. The row key is the number of the decode.
public struct Pi4DecodeRow {
    /// The row key.
    public var key: String
    /// The time of the start of the decode.
    public var utc: Double
    /// The signal time of the start of the decode, from the first sample.
    public var timeS: Double
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var message: String
    /// The start of the decode against the minute.
    public var dtS: Double
    /// The carrier frequency: tone 0 plus half a spacing.
    public var carrierHz: Double
    /// The sync value c.
    public var sync: Double
    /// The tone spacing factor of the decode.
    public var k: Int64

    init(_ c: sdrkit_pi4_decode_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        timeS = c.time_s
        snrDb = c.snr_db
        audioHz = c.audio_hz
        mode = sdrkitText(c.mode)
        message = sdrkitText(c.message)
        dtS = c.dt_s
        carrierHz = c.carrier_hz
        sync = c.sync
        k = c.k
    }
}

/// The fields of a decode of any mode but JS8.
public struct WsjtDecodeEvent {
    public enum Mode: Int32 {
        case ft8 = 0
        case ft4 = 1
        case ft2 = 2
        case wspr = 3
        case jt65 = 4
        case jt9 = 5
        case jt4 = 6
        case q65 = 7
        case fst4 = 8
        case fst4w = 9
        case msk144 = 10
        case js8 = 11
    }
    public enum How: Int32 {
        case alone = 0
        case combined = 1
        case aPriori = 2
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
    /// The mode.
    public var mode: Mode?
    /// The decoded message.
    public var text: String
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The time offset from the slot start.
    public var dtS: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The calling station, if the message names one.
    public var callsign: String?
    /// The locator, if the message holds one.
    public var locator: String?
    /// The message is a CQ.
    public var cq: Bool
    /// The decode was found on the other sideband.
    public var mirrored: Bool
    /// How the decode was made.
    public var how: How?
    /// The power that a WSPR or FST4W station states.
    public var powerDbm: Int64?
    /// The calling station, if the message names one.
    public var source: String?
    /// The centre of the locator, if the message holds one.
    public var position: SDRKitPosition?
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_wsjt_decode_event) {
        time = c.time
        freqHz = c.freq_hz
        mode = Mode(rawValue: c.mode)
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        dtS = c.dt_s
        audioHz = c.audio_hz
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        cq = c.cq
        mirrored = c.mirrored
        how = How(rawValue: c.how)
        powerDbm = c.has_power_dbm ? c.power_dbm : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WsjtDecodeEvent, SDRKitCode> {
        var c = sdrkit_wsjt_decode_event()
        let code = sdrkit_wsjt_decode_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WsjtDecodeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a station heard, with the standard spot fields.
public struct WsjtSpotEvent {
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
    /// The callsign.
    public var callsign: String
    /// The locator, if one was heard.
    public var locator: String?
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The radio frequency.
    public var freqHzField: Double
    /// The emission as spot networks name it.
    public var mode: String
    /// The callsign.
    public var source: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_wsjt_spot_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        snrDb = c.snr_db
        freqHzField = c.freq_hz_
        mode = sdrkitText(c.mode)
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WsjtSpotEvent, SDRKitCode> {
        var c = sdrkit_wsjt_spot_event()
        let code = sdrkit_wsjt_spot_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WsjtSpotEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a JS8 frame.
public struct Js8FrameEvent {
    public enum Submode: Int32 {
        case normal = 0
        case fast = 1
        case turbo = 2
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
    /// The text of the frame.
    public var text: String
    /// The twelve characters of the frame.
    public var chars: String
    /// The SNR in 2500 Hz.
    public var snrDb: Double
    /// The speed of the frame.
    public var submode: Submode?
    /// The sending station, if the frame names one.
    public var source: String?
    /// The station or group addressed, if the frame names one.
    public var target: String?
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_js8_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        chars = sdrkitText(c.chars)
        snrDb = c.snr_db
        submode = c.has_submode ? Submode(rawValue: c.submode) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Js8FrameEvent, SDRKitCode> {
        var c = sdrkit_js8_frame_event()
        let code = sdrkit_js8_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Js8FrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a JS8 message, sent when its last frame came or when it stopped.
public struct Js8MessageEvent {
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
    /// The message.
    public var text: String
    /// The last frame of the message came.
    public var complete: Bool
    /// The sending station, if it is known.
    public var source: String?
    /// The station or group addressed, if it is known.
    public var target: String?
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_js8_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        complete = c.complete
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Js8MessageEvent, SDRKitCode> {
        var c = sdrkit_js8_message_event()
        let code = sdrkit_js8_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Js8MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a message to send.
public struct WsjtTxEvent {
    public enum Mode: Int32 {
        case ft8 = 0
        case ft4 = 1
        case ft2 = 2
        case wspr = 3
        case jt65 = 4
        case jt9 = 5
        case jt4 = 6
        case q65 = 7
        case fst4 = 8
        case fst4w = 9
        case msk144 = 10
        case js8 = 11
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
    /// The mode.
    public var mode: Mode?
    /// The message.
    public var text: String
    /// The audio frequency.
    public var audioHz: Double
    /// The UTC of the start of the transmit slot.
    public var slot: Double
    /// The T/R period.
    public var period: Double
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_wsjt_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        mode = Mode(rawValue: c.mode)
        text = sdrkitText(c.text)
        audioHz = c.audio_hz
        slot = c.slot
        period = c.period
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WsjtTxEvent, SDRKitCode> {
        var c = sdrkit_wsjt_tx_event()
        let code = sdrkit_wsjt_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WsjtTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a decode of `fsk441`.
public struct Fsk441DecodeEvent {
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
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var text: String
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The first callsign of the text.
    public var callsign: String?
    /// The first callsign of the text.
    public var source: String?
    /// The family of the source: a message.
    public var family: Family?
    /// The contrast of the decode. It is no calibrated SNR.
    public var contrastDb: Double
    /// The length of the burst.
    public var durationMs: Double

    init(_ c: sdrkit_fsk441_decode_event) {
        time = c.time
        freqHz = c.freq_hz
        mode = sdrkitText(c.mode)
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        audioHz = c.audio_hz
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        contrastDb = c.contrast_db
        durationMs = c.duration_ms
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Fsk441DecodeEvent, SDRKitCode> {
        var c = sdrkit_fsk441_decode_event()
        let code = sdrkit_fsk441_decode_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Fsk441DecodeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a ping of `jt6m`.
public struct Jt6mDecodeEvent {
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
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var text: String
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The first callsign of the text.
    public var callsign: String?
    /// The first callsign of the text.
    public var source: String?
    /// The family of the source: a message.
    public var family: Family?
    /// The length of the ping.
    public var durationS: Double
    /// The clear frames of the ping.
    public var clearFrames: Int64

    init(_ c: sdrkit_jt6m_decode_event) {
        time = c.time
        freqHz = c.freq_hz
        mode = sdrkitText(c.mode)
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        audioHz = c.audio_hz
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        durationS = c.duration_s
        clearFrames = c.clear_frames
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Jt6mDecodeEvent, SDRKitCode> {
        var c = sdrkit_jt6m_decode_event()
        let code = sdrkit_jt6m_decode_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Jt6mDecodeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a decode of `iscat`.
public struct IscatDecodeEvent {
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
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var text: String
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The first callsign of the text.
    public var callsign: String?
    /// The first callsign of the text.
    public var source: String?
    /// The family of the source: a message.
    public var family: Family?
    /// The variant of the decode.
    public var variant: String
    /// The sync ratio r.
    public var syncRatio: Double
    /// The whole frames used.
    public var frames: Int64
    /// The characters shown as `?`.
    public var unclear: Int64

    init(_ c: sdrkit_iscat_decode_event) {
        time = c.time
        freqHz = c.freq_hz
        mode = sdrkitText(c.mode)
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        audioHz = c.audio_hz
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        variant = sdrkitText(c.variant)
        syncRatio = c.sync_ratio
        frames = c.frames
        unclear = c.unclear
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IscatDecodeEvent, SDRKitCode> {
        var c = sdrkit_iscat_decode_event()
        let code = sdrkit_iscat_decode_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IscatDecodeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a decode of `pi4`.
public struct Pi4DecodeEvent {
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
    /// The mode or its variant.
    public var mode: String
    /// The decoded text.
    public var text: String
    /// The SNR.
    public var snrDb: Double
    /// The audio frequency.
    public var audioHz: Double
    /// The first callsign of the text.
    public var callsign: String?
    /// The first callsign of the text.
    public var source: String?
    /// The family of the source: a message.
    public var family: Family?
    /// The start of the decode against the minute.
    public var dtS: Double
    /// The carrier frequency: tone 0 plus half a spacing.
    public var carrierHz: Double
    /// The sync value c.
    public var sync: Double
    /// The tone spacing factor of the decode.
    public var k: Int64

    init(_ c: sdrkit_pi4_decode_event) {
        time = c.time
        freqHz = c.freq_hz
        mode = sdrkitText(c.mode)
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        audioHz = c.audio_hz
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        dtS = c.dt_s
        carrierHz = c.carrier_hz
        sync = c.sync
        k = c.k
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Pi4DecodeEvent, SDRKitCode> {
        var c = sdrkit_pi4_decode_event()
        let code = sdrkit_pi4_decode_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Pi4DecodeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "wsjt" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct WsjtDropped {
    /// The items that the component "dec_ft8" dropped.
    public var decFt8: Int64?
    /// The items that the component "dec_ft4" dropped.
    public var decFt4: Int64?
    /// The items that the component "dec_ft2" dropped.
    public var decFt2: Int64?
    /// The items that the component "dec_wspr" dropped.
    public var decWspr: Int64?
    /// The items that the component "dec_jt65" dropped.
    public var decJt65: Int64?
    /// The items that the component "dec_jt9" dropped.
    public var decJt9: Int64?
    /// The items that the component "dec_jt4" dropped.
    public var decJt4: Int64?
    /// The items that the component "dec_q65" dropped.
    public var decQ65: Int64?
    /// The items that the component "dec_fst4" dropped.
    public var decFst4: Int64?
    /// The items that the component "dec_fst4w" dropped.
    public var decFst4w: Int64?
    /// The items that the component "dec_msk144" dropped.
    public var decMsk144: Int64?
    /// The items that the component "dec_js8" dropped.
    public var decJs8: Int64?

    init(_ c: sdrkit_wsjt_dropped) {
        decFt8 = c.has_dec_ft8 ? c.dec_ft8 : nil
        decFt4 = c.has_dec_ft4 ? c.dec_ft4 : nil
        decFt2 = c.has_dec_ft2 ? c.dec_ft2 : nil
        decWspr = c.has_dec_wspr ? c.dec_wspr : nil
        decJt65 = c.has_dec_jt65 ? c.dec_jt65 : nil
        decJt9 = c.has_dec_jt9 ? c.dec_jt9 : nil
        decJt4 = c.has_dec_jt4 ? c.dec_jt4 : nil
        decQ65 = c.has_dec_q65 ? c.dec_q65 : nil
        decFst4 = c.has_dec_fst4 ? c.dec_fst4 : nil
        decFst4w = c.has_dec_fst4w ? c.dec_fst4w : nil
        decMsk144 = c.has_dec_msk144 ? c.dec_msk144 : nil
        decJs8 = c.has_dec_js8 ? c.dec_js8 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WsjtDropped, SDRKitCode> {
        var c = sdrkit_wsjt_dropped()
        let code = sdrkit_wsjt_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ft8" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Ft8Dropped {
    /// The items that the component "dec_ft8" dropped.
    public var decFt8: Int64?

    init(_ c: sdrkit_ft8_dropped) {
        decFt8 = c.has_dec_ft8 ? c.dec_ft8 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Ft8Dropped, SDRKitCode> {
        var c = sdrkit_ft8_dropped()
        let code = sdrkit_ft8_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Ft8Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ft4" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Ft4Dropped {
    /// The items that the component "dec_ft4" dropped.
    public var decFt4: Int64?

    init(_ c: sdrkit_ft4_dropped) {
        decFt4 = c.has_dec_ft4 ? c.dec_ft4 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Ft4Dropped, SDRKitCode> {
        var c = sdrkit_ft4_dropped()
        let code = sdrkit_ft4_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Ft4Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ft2" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Ft2Dropped {
    /// The items that the component "dec_ft2" dropped.
    public var decFt2: Int64?

    init(_ c: sdrkit_ft2_dropped) {
        decFt2 = c.has_dec_ft2 ? c.dec_ft2 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Ft2Dropped, SDRKitCode> {
        var c = sdrkit_ft2_dropped()
        let code = sdrkit_ft2_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Ft2Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "wspr" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct WsprDropped {
    /// The items that the component "dec_wspr" dropped.
    public var decWspr: Int64?

    init(_ c: sdrkit_wspr_dropped) {
        decWspr = c.has_dec_wspr ? c.dec_wspr : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WsprDropped, SDRKitCode> {
        var c = sdrkit_wspr_dropped()
        let code = sdrkit_wspr_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsprDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "jt65" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Jt65Dropped {
    /// The items that the component "dec_jt65" dropped.
    public var decJt65: Int64?

    init(_ c: sdrkit_jt65_dropped) {
        decJt65 = c.has_dec_jt65 ? c.dec_jt65 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Jt65Dropped, SDRKitCode> {
        var c = sdrkit_jt65_dropped()
        let code = sdrkit_jt65_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Jt65Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "jt9" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Jt9Dropped {
    /// The items that the component "dec_jt9" dropped.
    public var decJt9: Int64?

    init(_ c: sdrkit_jt9_dropped) {
        decJt9 = c.has_dec_jt9 ? c.dec_jt9 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Jt9Dropped, SDRKitCode> {
        var c = sdrkit_jt9_dropped()
        let code = sdrkit_jt9_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Jt9Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "jt4" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Jt4Dropped {
    /// The items that the component "dec_jt4" dropped.
    public var decJt4: Int64?

    init(_ c: sdrkit_jt4_dropped) {
        decJt4 = c.has_dec_jt4 ? c.dec_jt4 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Jt4Dropped, SDRKitCode> {
        var c = sdrkit_jt4_dropped()
        let code = sdrkit_jt4_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Jt4Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "q65" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Q65Dropped {
    /// The items that the component "dec_q65" dropped.
    public var decQ65: Int64?

    init(_ c: sdrkit_q65_dropped) {
        decQ65 = c.has_dec_q65 ? c.dec_q65 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Q65Dropped, SDRKitCode> {
        var c = sdrkit_q65_dropped()
        let code = sdrkit_q65_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Q65Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "fst4" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Fst4Dropped {
    /// The items that the component "dec_fst4" dropped.
    public var decFst4: Int64?

    init(_ c: sdrkit_fst4_dropped) {
        decFst4 = c.has_dec_fst4 ? c.dec_fst4 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Fst4Dropped, SDRKitCode> {
        var c = sdrkit_fst4_dropped()
        let code = sdrkit_fst4_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Fst4Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "fst4w" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Fst4wDropped {
    /// The items that the component "dec_fst4w" dropped.
    public var decFst4w: Int64?

    init(_ c: sdrkit_fst4w_dropped) {
        decFst4w = c.has_dec_fst4w ? c.dec_fst4w : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Fst4wDropped, SDRKitCode> {
        var c = sdrkit_fst4w_dropped()
        let code = sdrkit_fst4w_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Fst4wDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "msk144" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Msk144Dropped {
    /// The items that the component "dec_msk144" dropped.
    public var decMsk144: Int64?

    init(_ c: sdrkit_msk144_dropped) {
        decMsk144 = c.has_dec_msk144 ? c.dec_msk144 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Msk144Dropped, SDRKitCode> {
        var c = sdrkit_msk144_dropped()
        let code = sdrkit_msk144_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Msk144Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "js8" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Js8Dropped {
    /// The items that the component "dec_js8" dropped.
    public var decJs8: Int64?

    init(_ c: sdrkit_js8_dropped) {
        decJs8 = c.has_dec_js8 ? c.dec_js8 : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Js8Dropped, SDRKitCode> {
        var c = sdrkit_js8_dropped()
        let code = sdrkit_js8_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Js8Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum WsjtComponents {
    /// The result fields of the component "dec_ft8" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecFt8SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_ft8_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_ft4" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecFt4SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_ft4_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_ft2" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecFt2SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_ft2_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_wspr" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecWsprSlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_wspr_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_jt65" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecJt65SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_jt65_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_jt9" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecJt9SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_jt9_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_jt4" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecJt4SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_jt4_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_q65" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecQ65SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_q65_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_fst4" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecFst4SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_fst4_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_fst4w" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecFst4wSlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_fst4w_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_msk144" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecMsk144SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_msk144_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_js8" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtDecJs8SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wsjt_dec_js8_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "wide" of the stack "wsjt" in the snapshot of `handle`.
    public static func wsjtWideFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtWideFields, SDRKitCode> {
        var c = sdrkit_wsjt_wide_fields()
        let code = sdrkit_wsjt_wide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtWideFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_ft8" of the stack "ft8" in the snapshot of `handle`.
    public static func ft8DecFt8SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_ft8_dec_ft8_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "wide" of the stack "ft8" in the snapshot of `handle`.
    public static func ft8WideFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtWideFields, SDRKitCode> {
        var c = sdrkit_wsjt_wide_fields()
        let code = sdrkit_ft8_wide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtWideFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "hop" of the stack "ft8" in the snapshot of `handle`.
    public static func ft8HopFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtHopFields, SDRKitCode> {
        var c = sdrkit_wsjt_hop_fields()
        let code = sdrkit_ft8_hop_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtHopFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_ft4" of the stack "ft4" in the snapshot of `handle`.
    public static func ft4DecFt4SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_ft4_dec_ft4_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "wide" of the stack "ft4" in the snapshot of `handle`.
    public static func ft4WideFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtWideFields, SDRKitCode> {
        var c = sdrkit_wsjt_wide_fields()
        let code = sdrkit_ft4_wide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtWideFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "hop" of the stack "ft4" in the snapshot of `handle`.
    public static func ft4HopFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtHopFields, SDRKitCode> {
        var c = sdrkit_wsjt_hop_fields()
        let code = sdrkit_ft4_hop_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtHopFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_ft2" of the stack "ft2" in the snapshot of `handle`.
    public static func ft2DecFt2SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_ft2_dec_ft2_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "wide" of the stack "ft2" in the snapshot of `handle`.
    public static func ft2WideFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtWideFields, SDRKitCode> {
        var c = sdrkit_wsjt_wide_fields()
        let code = sdrkit_ft2_wide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtWideFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "hop" of the stack "ft2" in the snapshot of `handle`.
    public static func ft2HopFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtHopFields, SDRKitCode> {
        var c = sdrkit_wsjt_hop_fields()
        let code = sdrkit_ft2_hop_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtHopFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_wspr" of the stack "wspr" in the snapshot of `handle`.
    public static func wsprDecWsprSlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_wspr_dec_wspr_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "wide" of the stack "wspr" in the snapshot of `handle`.
    public static func wsprWideFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtWideFields, SDRKitCode> {
        var c = sdrkit_wsjt_wide_fields()
        let code = sdrkit_wspr_wide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtWideFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "hop" of the stack "wspr" in the snapshot of `handle`.
    public static func wsprHopFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtHopFields, SDRKitCode> {
        var c = sdrkit_wsjt_hop_fields()
        let code = sdrkit_wspr_hop_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtHopFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_jt65" of the stack "jt65" in the snapshot of `handle`.
    public static func jt65DecJt65SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_jt65_dec_jt65_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_jt9" of the stack "jt9" in the snapshot of `handle`.
    public static func jt9DecJt9SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_jt9_dec_jt9_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_jt4" of the stack "jt4" in the snapshot of `handle`.
    public static func jt4DecJt4SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_jt4_dec_jt4_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_q65" of the stack "q65" in the snapshot of `handle`.
    public static func q65DecQ65SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_q65_dec_q65_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_fst4" of the stack "fst4" in the snapshot of `handle`.
    public static func fst4DecFst4SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_fst4_dec_fst4_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_fst4w" of the stack "fst4w" in the snapshot of `handle`.
    public static func fst4wDecFst4wSlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_fst4w_dec_fst4w_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_msk144" of the stack "msk144" in the snapshot of `handle`.
    public static func msk144DecMsk144SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_msk144_dec_msk144_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dec_js8" of the stack "js8" in the snapshot of `handle`.
    public static func js8DecJs8SlotDecoderFields(_ handle: SDRKIT_HANDLE) -> Result<WsjtSlotDecoderFields, SDRKitCode> {
        var c = sdrkit_wsjt_slot_decoder_fields()
        let code = sdrkit_js8_dec_js8_slot_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WsjtSlotDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum WsjtTables {
    /// The changed rows of the table "decodes" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtDecodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtWsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtModes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtStations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtJs8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtJs8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtJs8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "wide.multiband" of the stack "wsjt", newest first, at most `capacity` rows.
    public static func wsjtWideMultiband(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WsjtMultibandRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_multiband_row](repeating: sdrkit_wsjt_multiband_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wsjt_wide_multiband_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtMultibandRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "wide.multiband" of the stack "ft8", newest first, at most `capacity` rows.
    public static func ft8WideMultiband(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WsjtMultibandRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_multiband_row](repeating: sdrkit_wsjt_multiband_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft8_wide_multiband_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtMultibandRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "wide.multiband" of the stack "ft4", newest first, at most `capacity` rows.
    public static func ft4WideMultiband(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WsjtMultibandRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_multiband_row](repeating: sdrkit_wsjt_multiband_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft4_wide_multiband_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtMultibandRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "wide.multiband" of the stack "ft2", newest first, at most `capacity` rows.
    public static func ft2WideMultiband(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WsjtMultibandRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_multiband_row](repeating: sdrkit_wsjt_multiband_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ft2_wide_multiband_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtMultibandRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprDecodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprModes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprStations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprJs8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprJs8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprJs8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "wide.multiband" of the stack "wspr", newest first, at most `capacity` rows.
    public static func wsprWideMultiband(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WsjtMultibandRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_multiband_row](repeating: sdrkit_wsjt_multiband_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wspr_wide_multiband_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtMultibandRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "jt65", newest first, at most `capacity` rows.
    public static func jt65Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt65_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "jt9", newest first, at most `capacity` rows.
    public static func jt9Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt9_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "jt4", newest first, at most `capacity` rows.
    public static func jt4Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt4_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "q65", newest first, at most `capacity` rows.
    public static func q65Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_q65_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "fst4", newest first, at most `capacity` rows.
    public static func fst4Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wDecodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wWsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wModes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wStations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wJs8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wJs8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "fst4w", newest first, at most `capacity` rows.
    public static func fst4wJs8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fst4w_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144Js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144Js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "msk144", newest first, at most `capacity` rows.
    public static func msk144Js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msk144_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtDecodeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_decode_row](repeating: sdrkit_wsjt_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtDecodeRow.init))
    }
    /// The changed rows of the table "wspr_spots" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8WsprSpots(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsprSpotRow], SDRKitCode> {
        var rows = [sdrkit_wspr_spot_row](repeating: sdrkit_wspr_spot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_wspr_spots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsprSpotRow.init))
    }
    /// The changed rows of the table "modes" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8Modes(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[WsjtModeRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_mode_row](repeating: sdrkit_wsjt_mode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_modes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtModeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8WsjtStations(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[WsjtStationRow], SDRKitCode> {
        var rows = [sdrkit_wsjt_station_row](repeating: sdrkit_wsjt_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_wsjt_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WsjtStationRow.init))
    }
    /// The changed rows of the table "js8_frames" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8FrameRow], SDRKitCode> {
        var rows = [sdrkit_js8_frame_row](repeating: sdrkit_js8_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8FrameRow.init))
    }
    /// The changed rows of the table "js8_messages" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Js8MessageRow], SDRKitCode> {
        var rows = [sdrkit_js8_message_row](repeating: sdrkit_js8_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8MessageRow.init))
    }
    /// The changed rows of the table "js8_stations" of the stack "js8", newest first, at most `capacity` rows.
    public static func js8Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[Js8StationRow], SDRKitCode> {
        var rows = [sdrkit_js8_station_row](repeating: sdrkit_js8_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_js8_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Js8StationRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "fsk441", newest first, at most `capacity` rows.
    public static func fsk441Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[Fsk441DecodeRow], SDRKitCode> {
        var rows = [sdrkit_fsk441_decode_row](repeating: sdrkit_fsk441_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsk441_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Fsk441DecodeRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "jt6m", newest first, at most `capacity` rows.
    public static func jt6mDecodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[Jt6mDecodeRow], SDRKitCode> {
        var rows = [sdrkit_jt6m_decode_row](repeating: sdrkit_jt6m_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jt6m_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Jt6mDecodeRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "iscat", newest first, at most `capacity` rows.
    public static func iscatDecodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[IscatDecodeRow], SDRKitCode> {
        var rows = [sdrkit_iscat_decode_row](repeating: sdrkit_iscat_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_iscat_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IscatDecodeRow.init))
    }
    /// The changed rows of the table "decodes" of the stack "pi4", newest first, at most `capacity` rows.
    public static func pi4Decodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[Pi4DecodeRow], SDRKitCode> {
        var rows = [sdrkit_pi4_decode_row](repeating: sdrkit_pi4_decode_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pi4_decodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Pi4DecodeRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum WsjtCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func wsjtClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wsjt_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func wsjtRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_wsjt_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func wsjtCq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wsjt_cq(handle)
        return sdrkitResult(code)
    }
    /// Start the WSPR beacon. Sends the call, locator and power of the station in the beacon slots. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func wsjtBeacon(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wsjt_beacon(handle)
        return sdrkitResult(code)
    }
    /// Send. Queues a JS8 message. My call and locator come from the station. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func wsjtSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_wsjt_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func wsjtHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wsjt_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ft8Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft8_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ft8Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ft8_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func ft8Cq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft8_cq(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func ft8Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft8_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ft4Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft4_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ft4Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ft4_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func ft4Cq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft4_cq(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func ft4Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft4_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ft2Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft2_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ft2Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ft2_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func ft2Cq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft2_cq(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func ft2Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ft2_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func wsprClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wspr_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func wsprRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_wspr_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Start the WSPR beacon. Sends the call, locator and power of the station in the beacon slots. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func wsprBeacon(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wspr_beacon(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func wsprHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wspr_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func jt65Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt65_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func jt65Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_jt65_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func jt65Cq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt65_cq(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func jt65Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt65_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func jt9Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt9_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func jt9Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_jt9_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func jt9Cq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt9_cq(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func jt9Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt9_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func jt4Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt4_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func jt4Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_jt4_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call CQ. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func jt4Cq(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt4_cq(handle)
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func jt4Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt4_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func q65Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_q65_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func q65Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_q65_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fst4Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fst4_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func fst4Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_fst4_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fst4wClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fst4w_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func fst4wRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_fst4w_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func msk144Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_msk144_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func msk144Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_msk144_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func js8Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_js8_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func js8Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_js8_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send. Queues a JS8 message. My call and locator come from the station. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func js8Send(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wsjt_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_wsjt_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_js8_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Stops transmitting at once.
    public static func js8Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_js8_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fsk441Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk441_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func jt6mClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jt6m_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func iscatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_iscat_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func pi4Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pi4_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum WsjtSettings {
    /// Decode FT8 (15 s slots).
    public static func wsjtEnableFt8(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_ft8", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableFt8(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_ft8", value))
    }
    /// Decode FT4 (7.5 s slots).
    public static func wsjtEnableFt4(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_ft4", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableFt4(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_ft4", value))
    }
    /// Decode FT2 (3.75 s slots).
    public static func wsjtEnableFt2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_ft2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableFt2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_ft2", value))
    }
    /// Decode WSPR (120 s slots).
    public static func wsjtEnableWspr(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_wspr", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableWspr(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_wspr", value))
    }
    /// Decode JT65 (60 s slots).
    public static func wsjtEnableJt65(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_jt65", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableJt65(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_jt65", value))
    }
    /// Decode JT9 (60 s slots).
    public static func wsjtEnableJt9(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_jt9", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableJt9(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_jt9", value))
    }
    /// Decode JT4 (60 s slots).
    public static func wsjtEnableJt4(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_jt4", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableJt4(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_jt4", value))
    }
    /// Decode Q65 (60 s slots).
    public static func wsjtEnableQ65(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_q65", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableQ65(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_q65", value))
    }
    /// Decode FST4 (60 s slots).
    public static func wsjtEnableFst4(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_fst4", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableFst4(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_fst4", value))
    }
    /// Decode FST4W (120 s slots).
    public static func wsjtEnableFst4w(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_fst4w", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableFst4w(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_fst4w", value))
    }
    /// Decode MSK144 (15 s slots).
    public static func wsjtEnableMsk144(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "enable_msk144", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEnableMsk144(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "enable_msk144", value))
    }
    /// The options of the setting "jt65_submode" of the stack "wsjt".
    public enum WsjtJt65Submode: Int32 {
        case a = 0
        case b = 1
        case c = 2
    }
    /// JT65 sub-mode.
    public static func wsjtJt65Submode(_ handle: SDRKIT_HANDLE) -> Result<WsjtJt65Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "jt65_submode", &v)
        return code == SDRKIT_OK ? .success(WsjtJt65Submode(rawValue: v) ?? WsjtJt65Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJt65Submode(_ handle: SDRKIT_HANDLE, _ value: WsjtJt65Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "jt65_submode", value.rawValue))
    }
    /// The options of the setting "jt4_submode" of the stack "wsjt".
    public enum WsjtJt4Submode: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
        case e = 4
        case f = 5
        case g = 6
    }
    /// JT4 sub-mode.
    public static func wsjtJt4Submode(_ handle: SDRKIT_HANDLE) -> Result<WsjtJt4Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "jt4_submode", &v)
        return code == SDRKIT_OK ? .success(WsjtJt4Submode(rawValue: v) ?? WsjtJt4Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJt4Submode(_ handle: SDRKIT_HANDLE, _ value: WsjtJt4Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "jt4_submode", value.rawValue))
    }
    /// The options of the setting "q65_submode" of the stack "wsjt".
    public enum WsjtQ65Submode: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
        case e = 4
    }
    /// Q65 sub-mode.
    public static func wsjtQ65Submode(_ handle: SDRKIT_HANDLE) -> Result<WsjtQ65Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "q65_submode", &v)
        return code == SDRKIT_OK ? .success(WsjtQ65Submode(rawValue: v) ?? WsjtQ65Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQ65Submode(_ handle: SDRKIT_HANDLE, _ value: WsjtQ65Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "q65_submode", value.rawValue))
    }
    /// Q65 T/R period.
    public static func wsjtQ65Period(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "q65_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQ65Period(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "q65_period", value))
    }
    /// FST4 T/R period.
    public static func wsjtFst4Period(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fst4_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtFst4Period(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fst4_period", value))
    }
    /// FST4W T/R period.
    public static func wsjtFst4wPeriod(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fst4w_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtFst4wPeriod(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fst4w_period", value))
    }
    /// MSK144 T/R period.
    public static func wsjtMsk144Period(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "msk144_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtMsk144Period(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "msk144_period", value))
    }
    /// Code tables.
    public static func wsjtCodeDir(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "code_dir", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtCodeDir(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "code_dir", $0) })
    }
    /// JS8 Normal (15 s).
    public static func wsjtJs8Normal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "js8_normal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJs8Normal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "js8_normal", value))
    }
    /// JS8 Fast (10 s).
    public static func wsjtJs8Fast(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "js8_fast", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJs8Fast(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "js8_fast", value))
    }
    /// JS8 Turbo (6 s).
    public static func wsjtJs8Turbo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "js8_turbo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJs8Turbo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "js8_turbo", value))
    }
    /// Upside down too.
    public static func wsjtMirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtMirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mirrored", value))
    }
    /// Deep decoding.
    public static func wsjtDeep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtDeep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// Early decode.
    public static func wsjtEarlyDecode(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "early_decode", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtEarlyDecode(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "early_decode", value))
    }
    /// The options of the setting "demod" of the stack "wsjt".
    public enum WsjtDemod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func wsjtDemod(_ handle: SDRKIT_HANDLE) -> Result<WsjtDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(WsjtDemod(rawValue: v) ?? WsjtDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtDemod(_ handle: SDRKIT_HANDLE, _ value: WsjtDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func wsjtListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "wsjt".
    public enum WsjtTimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func wsjtTimeSource(_ handle: SDRKIT_HANDLE) -> Result<WsjtTimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(WsjtTimeSource(rawValue: v) ?? WsjtTimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtTimeSource(_ handle: SDRKIT_HANDLE, _ value: WsjtTimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func wsjtTimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtTimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func wsjtBandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtBandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Dials.
    public static func wsjtDials(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "dials", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtDials(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "dials", $0) })
    }
    /// Decode the whole span.
    public static func wsjtWide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtWide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide", value))
    }
    /// Span.
    public static func wsjtWideSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wide_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtWideSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wide_span", value))
    }
    /// Decode all selected at once.
    public static func wsjtMultiband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "multiband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtMultiband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "multiband", value))
    }
    /// Bands and modes.
    public static func wsjtMultibandSelect(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "multiband_select", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtMultibandSelect(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "multiband_select", value))
    }
    /// Spot events.
    public static func wsjtSpots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtSpots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// The options of the setting "qso_mode" of the stack "wsjt".
    public enum WsjtQsoMode: Int32 {
        case ft8 = 0
        case ft4 = 1
        case ft2 = 2
        case jt65 = 3
        case jt9 = 4
        case jt4 = 5
    }
    /// Mode.
    public static func wsjtQsoMode(_ handle: SDRKIT_HANDLE) -> Result<WsjtQsoMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "qso_mode", &v)
        return code == SDRKIT_OK ? .success(WsjtQsoMode(rawValue: v) ?? WsjtQsoMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoMode(_ handle: SDRKIT_HANDLE, _ value: WsjtQsoMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "qso_mode", value.rawValue))
    }
    /// Transmit in even slots.
    public static func wsjtQsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func wsjtQsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func wsjtQsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func wsjtQsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func wsjtQsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func wsjtQsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtQsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// Power.
    public static func wsjtBeaconDbm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beacon_dbm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtBeaconDbm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beacon_dbm", value))
    }
    /// One slot in N.
    public static func wsjtBeaconEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beacon_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtBeaconEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beacon_every", value))
    }
    /// Offset.
    public static func wsjtBeaconOffset(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beacon_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtBeaconOffset(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beacon_offset", value))
    }
    /// Audio frequency.
    public static func wsjtBeaconHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "beacon_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtBeaconHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "beacon_hz", value))
    }
    /// The options of the setting "js8_speed" of the stack "wsjt".
    public enum WsjtJs8Speed: Int32 {
        case normal = 0
        case fast = 1
        case turbo = 2
    }
    /// Speed.
    public static func wsjtJs8Speed(_ handle: SDRKIT_HANDLE) -> Result<WsjtJs8Speed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "js8_speed", &v)
        return code == SDRKIT_OK ? .success(WsjtJs8Speed(rawValue: v) ?? WsjtJs8Speed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJs8Speed(_ handle: SDRKIT_HANDLE, _ value: WsjtJs8Speed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "js8_speed", value.rawValue))
    }
    /// Audio frequency.
    public static func wsjtJs8Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "js8_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtJs8Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "js8_hz", value))
    }
    /// Upside down too.
    public static func ft8Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mirrored", value))
    }
    /// Deep decoding.
    public static func ft8Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// Early decode.
    public static func ft8EarlyDecode(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "early_decode", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8EarlyDecode(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "early_decode", value))
    }
    /// The options of the setting "demod" of the stack "ft8".
    public enum Ft8Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func ft8Demod(_ handle: SDRKIT_HANDLE) -> Result<Ft8Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Ft8Demod(rawValue: v) ?? Ft8Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Demod(_ handle: SDRKIT_HANDLE, _ value: Ft8Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func ft8Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "ft8".
    public enum Ft8TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func ft8TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Ft8TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Ft8TimeSource(rawValue: v) ?? Ft8TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8TimeSource(_ handle: SDRKIT_HANDLE, _ value: Ft8TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func ft8TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func ft8BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Dials.
    public static func ft8Dials(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "dials", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Dials(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "dials", $0) })
    }
    /// Decode the whole span.
    public static func ft8Wide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Wide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide", value))
    }
    /// Span.
    public static func ft8WideSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wide_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8WideSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wide_span", value))
    }
    /// Decode all selected at once.
    public static func ft8Multiband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "multiband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Multiband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "multiband", value))
    }
    /// Bands and modes.
    public static func ft8MultibandSelect(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "multiband_select", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8MultibandSelect(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "multiband_select", value))
    }
    /// Band hopping.
    public static func ft8HopEnabled(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.enabled", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8HopEnabled(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.enabled", value))
    }
    /// Coordinated hopping.
    public static func ft8HopCoordinated(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.coordinated", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8HopCoordinated(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value))
    }
    /// Bands of each hour.
    public static func ft8HopHours(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.hours", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8HopHours(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.hours", $0) })
    }
    /// Band list.
    public static func ft8HopBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8HopBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.bands", $0) })
    }
    /// Coordinated cycle.
    public static func ft8HopCycle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.cycle", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8HopCycle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.cycle", $0) })
    }
    /// Spot events.
    public static func ft8Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Transmit in even slots.
    public static func ft8QsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8QsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func ft8QsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8QsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func ft8QsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8QsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func ft8QsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8QsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func ft8QsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8QsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func ft8QsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8QsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// Upside down too.
    public static func ft4Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mirrored", value))
    }
    /// Deep decoding.
    public static func ft4Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// Early decode.
    public static func ft4EarlyDecode(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "early_decode", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4EarlyDecode(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "early_decode", value))
    }
    /// The options of the setting "demod" of the stack "ft4".
    public enum Ft4Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func ft4Demod(_ handle: SDRKIT_HANDLE) -> Result<Ft4Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Ft4Demod(rawValue: v) ?? Ft4Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Demod(_ handle: SDRKIT_HANDLE, _ value: Ft4Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func ft4Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "ft4".
    public enum Ft4TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func ft4TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Ft4TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Ft4TimeSource(rawValue: v) ?? Ft4TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4TimeSource(_ handle: SDRKIT_HANDLE, _ value: Ft4TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func ft4TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func ft4BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Dials.
    public static func ft4Dials(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "dials", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Dials(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "dials", $0) })
    }
    /// Decode the whole span.
    public static func ft4Wide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Wide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide", value))
    }
    /// Span.
    public static func ft4WideSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wide_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4WideSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wide_span", value))
    }
    /// Decode all selected at once.
    public static func ft4Multiband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "multiband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Multiband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "multiband", value))
    }
    /// Bands and modes.
    public static func ft4MultibandSelect(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "multiband_select", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4MultibandSelect(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "multiband_select", value))
    }
    /// Band hopping.
    public static func ft4HopEnabled(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.enabled", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4HopEnabled(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.enabled", value))
    }
    /// Coordinated hopping.
    public static func ft4HopCoordinated(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.coordinated", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4HopCoordinated(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value))
    }
    /// Bands of each hour.
    public static func ft4HopHours(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.hours", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4HopHours(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.hours", $0) })
    }
    /// Band list.
    public static func ft4HopBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4HopBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.bands", $0) })
    }
    /// Coordinated cycle.
    public static func ft4HopCycle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.cycle", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4HopCycle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.cycle", $0) })
    }
    /// Spot events.
    public static func ft4Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Transmit in even slots.
    public static func ft4QsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4QsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func ft4QsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4QsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func ft4QsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4QsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func ft4QsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4QsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func ft4QsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4QsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func ft4QsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4QsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// Upside down too.
    public static func ft2Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mirrored", value))
    }
    /// Deep decoding.
    public static func ft2Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "ft2".
    public enum Ft2Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func ft2Demod(_ handle: SDRKIT_HANDLE) -> Result<Ft2Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Ft2Demod(rawValue: v) ?? Ft2Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Demod(_ handle: SDRKIT_HANDLE, _ value: Ft2Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func ft2Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "ft2".
    public enum Ft2TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func ft2TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Ft2TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Ft2TimeSource(rawValue: v) ?? Ft2TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2TimeSource(_ handle: SDRKIT_HANDLE, _ value: Ft2TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func ft2TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func ft2BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Dials.
    public static func ft2Dials(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "dials", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Dials(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "dials", $0) })
    }
    /// Decode the whole span.
    public static func ft2Wide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Wide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide", value))
    }
    /// Span.
    public static func ft2WideSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wide_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2WideSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wide_span", value))
    }
    /// Decode all selected at once.
    public static func ft2Multiband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "multiband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Multiband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "multiband", value))
    }
    /// Bands and modes.
    public static func ft2MultibandSelect(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "multiband_select", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2MultibandSelect(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "multiband_select", value))
    }
    /// Band hopping.
    public static func ft2HopEnabled(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.enabled", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2HopEnabled(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.enabled", value))
    }
    /// Coordinated hopping.
    public static func ft2HopCoordinated(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.coordinated", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2HopCoordinated(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value))
    }
    /// Bands of each hour.
    public static func ft2HopHours(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.hours", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2HopHours(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.hours", $0) })
    }
    /// Band list.
    public static func ft2HopBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2HopBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.bands", $0) })
    }
    /// Coordinated cycle.
    public static func ft2HopCycle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.cycle", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2HopCycle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.cycle", $0) })
    }
    /// Spot events.
    public static func ft2Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Transmit in even slots.
    public static func ft2QsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2QsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func ft2QsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2QsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func ft2QsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2QsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func ft2QsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2QsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func ft2QsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2QsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func ft2QsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2QsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// Upside down too.
    public static func wsprMirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprMirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mirrored", value))
    }
    /// Deep decoding.
    public static func wsprDeep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprDeep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "wspr".
    public enum WsprDemod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func wsprDemod(_ handle: SDRKIT_HANDLE) -> Result<WsprDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(WsprDemod(rawValue: v) ?? WsprDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprDemod(_ handle: SDRKIT_HANDLE, _ value: WsprDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func wsprListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "wspr".
    public enum WsprTimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func wsprTimeSource(_ handle: SDRKIT_HANDLE) -> Result<WsprTimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(WsprTimeSource(rawValue: v) ?? WsprTimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprTimeSource(_ handle: SDRKIT_HANDLE, _ value: WsprTimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func wsprTimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprTimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func wsprBandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprBandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Dials.
    public static func wsprDials(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "dials", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprDials(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "dials", $0) })
    }
    /// Decode the whole span.
    public static func wsprWide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprWide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide", value))
    }
    /// Span.
    public static func wsprWideSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wide_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprWideSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wide_span", value))
    }
    /// Decode all selected at once.
    public static func wsprMultiband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "multiband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprMultiband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "multiband", value))
    }
    /// Bands and modes.
    public static func wsprMultibandSelect(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "multiband_select", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprMultibandSelect(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "multiband_select", value))
    }
    /// Band hopping.
    public static func wsprHopEnabled(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.enabled", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprHopEnabled(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.enabled", value))
    }
    /// Coordinated hopping.
    public static func wsprHopCoordinated(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop.coordinated", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprHopCoordinated(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value))
    }
    /// Bands of each hour.
    public static func wsprHopHours(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.hours", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprHopHours(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.hours", $0) })
    }
    /// Band list.
    public static func wsprHopBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprHopBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.bands", $0) })
    }
    /// Coordinated cycle.
    public static func wsprHopCycle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hop.cycle", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprHopCycle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hop.cycle", $0) })
    }
    /// Spot events.
    public static func wsprSpots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprSpots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Power.
    public static func wsprBeaconDbm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beacon_dbm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprBeaconDbm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beacon_dbm", value))
    }
    /// One slot in N.
    public static func wsprBeaconEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beacon_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprBeaconEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beacon_every", value))
    }
    /// Offset.
    public static func wsprBeaconOffset(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beacon_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprBeaconOffset(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beacon_offset", value))
    }
    /// Audio frequency.
    public static func wsprBeaconHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "beacon_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprBeaconHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "beacon_hz", value))
    }
    /// The options of the setting "jt65_submode" of the stack "jt65".
    public enum Jt65Submode: Int32 {
        case a = 0
        case b = 1
        case c = 2
    }
    /// JT65 sub-mode.
    public static func jt65Submode(_ handle: SDRKIT_HANDLE) -> Result<Jt65Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "jt65_submode", &v)
        return code == SDRKIT_OK ? .success(Jt65Submode(rawValue: v) ?? Jt65Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Submode(_ handle: SDRKIT_HANDLE, _ value: Jt65Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "jt65_submode", value.rawValue))
    }
    /// Deep decoding.
    public static func jt65Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "jt65".
    public enum Jt65Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func jt65Demod(_ handle: SDRKIT_HANDLE) -> Result<Jt65Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Jt65Demod(rawValue: v) ?? Jt65Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Demod(_ handle: SDRKIT_HANDLE, _ value: Jt65Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func jt65Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "jt65".
    public enum Jt65TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func jt65TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Jt65TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Jt65TimeSource(rawValue: v) ?? Jt65TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65TimeSource(_ handle: SDRKIT_HANDLE, _ value: Jt65TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func jt65TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func jt65BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func jt65Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Transmit in even slots.
    public static func jt65QsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65QsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func jt65QsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65QsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func jt65QsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65QsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func jt65QsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65QsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func jt65QsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65QsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func jt65QsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65QsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// Deep decoding.
    public static func jt9Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "jt9".
    public enum Jt9Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func jt9Demod(_ handle: SDRKIT_HANDLE) -> Result<Jt9Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Jt9Demod(rawValue: v) ?? Jt9Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Demod(_ handle: SDRKIT_HANDLE, _ value: Jt9Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func jt9Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "jt9".
    public enum Jt9TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func jt9TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Jt9TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Jt9TimeSource(rawValue: v) ?? Jt9TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9TimeSource(_ handle: SDRKIT_HANDLE, _ value: Jt9TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func jt9TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func jt9BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func jt9Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Transmit in even slots.
    public static func jt9QsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9QsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func jt9QsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9QsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func jt9QsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9QsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func jt9QsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9QsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func jt9QsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9QsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func jt9QsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9QsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// The options of the setting "jt4_submode" of the stack "jt4".
    public enum Jt4Submode: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
        case e = 4
        case f = 5
        case g = 6
    }
    /// JT4 sub-mode.
    public static func jt4Submode(_ handle: SDRKIT_HANDLE) -> Result<Jt4Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "jt4_submode", &v)
        return code == SDRKIT_OK ? .success(Jt4Submode(rawValue: v) ?? Jt4Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Submode(_ handle: SDRKIT_HANDLE, _ value: Jt4Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "jt4_submode", value.rawValue))
    }
    /// Deep decoding.
    public static func jt4Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "jt4".
    public enum Jt4Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func jt4Demod(_ handle: SDRKIT_HANDLE) -> Result<Jt4Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Jt4Demod(rawValue: v) ?? Jt4Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Demod(_ handle: SDRKIT_HANDLE, _ value: Jt4Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func jt4Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "jt4".
    public enum Jt4TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func jt4TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Jt4TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Jt4TimeSource(rawValue: v) ?? Jt4TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4TimeSource(_ handle: SDRKIT_HANDLE, _ value: Jt4TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func jt4TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func jt4BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func jt4Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// Transmit in even slots.
    public static func jt4QsoEven(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_even", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4QsoEven(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_even", value))
    }
    /// Auto-sequence.
    public static func jt4QsoAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4QsoAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_auto", value))
    }
    /// Hold TX frequency.
    public static func jt4QsoHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "qso_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4QsoHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "qso_hold", value))
    }
    /// TX audio frequency.
    public static func jt4QsoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4QsoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_hz", value))
    }
    /// Retry limit.
    public static func jt4QsoRetries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "qso_retries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4QsoRetries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "qso_retries", value))
    }
    /// Watchdog.
    public static func jt4QsoWatchdog(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "qso_watchdog", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4QsoWatchdog(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value))
    }
    /// The options of the setting "q65_submode" of the stack "q65".
    public enum Q65Submode: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
        case e = 4
    }
    /// Q65 sub-mode.
    public static func q65Submode(_ handle: SDRKIT_HANDLE) -> Result<Q65Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "q65_submode", &v)
        return code == SDRKIT_OK ? .success(Q65Submode(rawValue: v) ?? Q65Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Submode(_ handle: SDRKIT_HANDLE, _ value: Q65Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "q65_submode", value.rawValue))
    }
    /// Q65 T/R period.
    public static func q65Period(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "q65_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Period(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "q65_period", value))
    }
    /// Deep decoding.
    public static func q65Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "q65".
    public enum Q65Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func q65Demod(_ handle: SDRKIT_HANDLE) -> Result<Q65Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Q65Demod(rawValue: v) ?? Q65Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Demod(_ handle: SDRKIT_HANDLE, _ value: Q65Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func q65Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "q65".
    public enum Q65TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func q65TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Q65TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Q65TimeSource(rawValue: v) ?? Q65TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65TimeSource(_ handle: SDRKIT_HANDLE, _ value: Q65TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func q65TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func q65BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func q65Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// FST4 T/R period.
    public static func fst4Period(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fst4_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Period(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fst4_period", value))
    }
    /// Code tables.
    public static func fst4CodeDir(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "code_dir", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4CodeDir(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "code_dir", $0) })
    }
    /// Deep decoding.
    public static func fst4Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "fst4".
    public enum Fst4Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func fst4Demod(_ handle: SDRKIT_HANDLE) -> Result<Fst4Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Fst4Demod(rawValue: v) ?? Fst4Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Demod(_ handle: SDRKIT_HANDLE, _ value: Fst4Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func fst4Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "fst4".
    public enum Fst4TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func fst4TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Fst4TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Fst4TimeSource(rawValue: v) ?? Fst4TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4TimeSource(_ handle: SDRKIT_HANDLE, _ value: Fst4TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func fst4TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func fst4BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func fst4Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// FST4W T/R period.
    public static func fst4wPeriod(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fst4w_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wPeriod(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fst4w_period", value))
    }
    /// Code tables.
    public static func fst4wCodeDir(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "code_dir", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wCodeDir(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "code_dir", $0) })
    }
    /// Deep decoding.
    public static func fst4wDeep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wDeep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "fst4w".
    public enum Fst4wDemod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func fst4wDemod(_ handle: SDRKIT_HANDLE) -> Result<Fst4wDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Fst4wDemod(rawValue: v) ?? Fst4wDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wDemod(_ handle: SDRKIT_HANDLE, _ value: Fst4wDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func fst4wListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "fst4w".
    public enum Fst4wTimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func fst4wTimeSource(_ handle: SDRKIT_HANDLE) -> Result<Fst4wTimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Fst4wTimeSource(rawValue: v) ?? Fst4wTimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wTimeSource(_ handle: SDRKIT_HANDLE, _ value: Fst4wTimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func fst4wTimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wTimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func fst4wBandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wBandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func fst4wSpots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wSpots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// MSK144 T/R period.
    public static func msk144Period(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "msk144_period", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Period(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "msk144_period", value))
    }
    /// Code tables.
    public static func msk144CodeDir(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "code_dir", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144CodeDir(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "code_dir", $0) })
    }
    /// Deep decoding.
    public static func msk144Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "msk144".
    public enum Msk144Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func msk144Demod(_ handle: SDRKIT_HANDLE) -> Result<Msk144Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Msk144Demod(rawValue: v) ?? Msk144Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Demod(_ handle: SDRKIT_HANDLE, _ value: Msk144Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func msk144Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "msk144".
    public enum Msk144TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func msk144TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Msk144TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Msk144TimeSource(rawValue: v) ?? Msk144TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144TimeSource(_ handle: SDRKIT_HANDLE, _ value: Msk144TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func msk144TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func msk144BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func msk144Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// JS8 Normal (15 s).
    public static func js8Normal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "js8_normal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Normal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "js8_normal", value))
    }
    /// JS8 Fast (10 s).
    public static func js8Fast(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "js8_fast", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Fast(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "js8_fast", value))
    }
    /// JS8 Turbo (6 s).
    public static func js8Turbo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "js8_turbo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Turbo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "js8_turbo", value))
    }
    /// Deep decoding.
    public static func js8Deep(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deep", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Deep(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deep", value))
    }
    /// The options of the setting "demod" of the stack "js8".
    public enum Js8Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func js8Demod(_ handle: SDRKIT_HANDLE) -> Result<Js8Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Js8Demod(rawValue: v) ?? Js8Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Demod(_ handle: SDRKIT_HANDLE, _ value: Js8Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func js8Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "time_source" of the stack "js8".
    public enum Js8TimeSource: Int32 {
        case system = 0
        case start = 1
        case search = 2
    }
    /// Slot clock.
    public static func js8TimeSource(_ handle: SDRKIT_HANDLE) -> Result<Js8TimeSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "time_source", &v)
        return code == SDRKIT_OK ? .success(Js8TimeSource(rawValue: v) ?? Js8TimeSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8TimeSource(_ handle: SDRKIT_HANDLE, _ value: Js8TimeSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "time_source", value.rawValue))
    }
    /// Recording start (UTC).
    public static func js8TimeStart(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "time_start", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8TimeStart(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "time_start", $0) })
    }
    /// Band edges.
    public static func js8BandEdges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_edges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8BandEdges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_edges", $0) })
    }
    /// Spot events.
    public static func js8Spots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "spots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Spots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "spots", value))
    }
    /// The options of the setting "js8_speed" of the stack "js8".
    public enum Js8Speed: Int32 {
        case normal = 0
        case fast = 1
        case turbo = 2
    }
    /// Speed.
    public static func js8Speed(_ handle: SDRKIT_HANDLE) -> Result<Js8Speed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "js8_speed", &v)
        return code == SDRKIT_OK ? .success(Js8Speed(rawValue: v) ?? Js8Speed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Speed(_ handle: SDRKIT_HANDLE, _ value: Js8Speed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "js8_speed", value.rawValue))
    }
    /// Audio frequency.
    public static func js8Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "js8_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "js8_hz", value))
    }
    /// Frequency tolerance.
    public static func fsk441TolHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tol_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk441TolHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tol_hz", value))
    }
    /// Burst threshold.
    public static func fsk441BurstDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "burst_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk441BurstDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "burst_db", value))
    }
    /// The options of the setting "demod" of the stack "fsk441".
    public enum Fsk441Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func fsk441Demod(_ handle: SDRKIT_HANDLE) -> Result<Fsk441Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Fsk441Demod(rawValue: v) ?? Fsk441Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk441Demod(_ handle: SDRKIT_HANDLE, _ value: Fsk441Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func fsk441Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk441Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Frequency tolerance.
    public static func jt6mTolHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tol_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt6mTolHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tol_hz", value))
    }
    /// The options of the setting "demod" of the stack "jt6m".
    public enum Jt6mDemod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func jt6mDemod(_ handle: SDRKIT_HANDLE) -> Result<Jt6mDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Jt6mDemod(rawValue: v) ?? Jt6mDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setJt6mDemod(_ handle: SDRKIT_HANDLE, _ value: Jt6mDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func jt6mListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt6mListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "submode" of the stack "iscat".
    public enum IscatSubmode: Int32 {
        case auto = 0
        case a = 1
        case b = 2
    }
    /// Variant.
    public static func iscatSubmode(_ handle: SDRKIT_HANDLE) -> Result<IscatSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(IscatSubmode(rawValue: v) ?? IscatSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIscatSubmode(_ handle: SDRKIT_HANDLE, _ value: IscatSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Frequency tolerance.
    public static func iscatTolHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tol_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIscatTolHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tol_hz", value))
    }
    /// The options of the setting "demod" of the stack "iscat".
    public enum IscatDemod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func iscatDemod(_ handle: SDRKIT_HANDLE) -> Result<IscatDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(IscatDemod(rawValue: v) ?? IscatDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIscatDemod(_ handle: SDRKIT_HANDLE, _ value: IscatDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func iscatListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIscatListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "k" of the stack "pi4".
    public enum Pi4K: Int32 {
        case pi4 = 0
        case pi480 = 1
        case pi496 = 2
        case pi4120 = 3
    }
    /// Tone spacing factor.
    public static func pi4K(_ handle: SDRKIT_HANDLE) -> Result<Pi4K, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "k", &v)
        return code == SDRKIT_OK ? .success(Pi4K(rawValue: v) ?? Pi4K(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4K(_ handle: SDRKIT_HANDLE, _ value: Pi4K) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "k", value.rawValue))
    }
    /// Lowest tone-0 frequency.
    public static func pi4FLoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "f_lo_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4FLoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "f_lo_hz", value))
    }
    /// Highest tone-0 frequency.
    public static func pi4FHiHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "f_hi_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4FHiHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "f_hi_hz", value))
    }
    /// The options of the setting "demod" of the stack "pi4".
    public enum Pi4Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func pi4Demod(_ handle: SDRKIT_HANDLE) -> Result<Pi4Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Pi4Demod(rawValue: v) ?? Pi4Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4Demod(_ handle: SDRKIT_HANDLE, _ value: Pi4Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func pi4Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum WsjtGeneratorSettings {
    /// Test signal upside down.
    public static func wsjtMirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtMirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func wsjtLive(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtLive(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func wsjtSingle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtSingle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func wsjtHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsjtHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func ft8Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func ft8Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func ft8Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func ft8Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt8Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func ft4Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func ft4Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func ft4Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func ft4Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt4Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func ft2Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func ft2Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func ft2Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func ft2Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFt2Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func wsprMirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprMirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func wsprLive(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprLive(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func wsprSingle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprSingle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func wsprHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWsprHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func jt65Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func jt65Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func jt65Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func jt65Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt65Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func jt9Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func jt9Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func jt9Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func jt9Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt9Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func jt4Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func jt4Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func jt4Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func jt4Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt4Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func q65Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func q65Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func q65Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func q65Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQ65Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func fst4Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func fst4Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func fst4Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func fst4Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func fst4wMirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wMirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func fst4wLive(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wLive(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func fst4wSingle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wSingle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func fst4wHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFst4wHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func msk144Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func msk144Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func msk144Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func msk144Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsk144Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test signal upside down.
    public static func js8Mirrored(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_mirrored", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Mirrored(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value))
    }
    /// Test signal in step with the clock.
    public static func js8Live(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_live", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Live(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_live", value))
    }
    /// Single station message.
    public static func js8Single(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_single", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Single(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_single", $0) })
    }
    /// Single station frequency.
    public static func js8Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJs8Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_hz", value))
    }
    /// Test text.
    public static func fsk441Text(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk441Text(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
    /// Test text.
    public static func jt6mText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setJt6mText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
    /// Continuous test signal.
    public static func jt6mContinuous(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_continuous", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJt6mContinuous(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_continuous", value))
    }
    /// Test text.
    public static func iscatText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setIscatText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
    /// The options of the setting "gen_variant" of the stack "iscat".
    public enum IscatVariant: Int32 {
        case a = 0
        case b = 1
    }
    /// Test signal variant.
    public static func iscatVariant(_ handle: SDRKIT_HANDLE) -> Result<IscatVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &v)
        return code == SDRKIT_OK ? .success(IscatVariant(rawValue: v) ?? IscatVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIscatVariant(_ handle: SDRKIT_HANDLE, _ value: IscatVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_variant", value.rawValue))
    }
    /// Test text.
    public static func pi4Text(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4Text(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
    /// The options of the setting "gen_k" of the stack "pi4".
    public enum Pi4K: Int32 {
        case pi4 = 0
        case pi480 = 1
        case pi496 = 2
        case pi4120 = 3
    }
    /// Test signal tone spacing factor.
    public static func pi4K(_ handle: SDRKIT_HANDLE) -> Result<Pi4K, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_k", &v)
        return code == SDRKIT_OK ? .success(Pi4K(rawValue: v) ?? Pi4K(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPi4K(_ handle: SDRKIT_HANDLE, _ value: Pi4K) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_k", value.rawValue))
    }
}

// sdrkit: the typed interface of the stack family "marine", version 1.3. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum MarineInterface {
    public static let major = 1
    public static let minor = 3
}

/// The result fields of the AIS stack.
public struct AisFields {
    public enum State: Int32 {
        case searching = 0
        case signal = 1
        case decoding = 2
    }
    /// The reception state of a display.
    public var state: State?
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Bursts detected.
    public var bursts: Int64
    /// Messages found on another frequency candidate or part of a burst.
    public var recovered: Int64
    /// Messages found after cancelling a stronger burst.
    public var cancelled: Int64
    /// Recovered and cancelled messages together.
    public var collisionRecoveries: Int64
    /// Stations in the table.
    public var stations: Int64
    /// Stations with a position.
    public var withPosition: Int64
    /// Stations that sent message 27.
    public var longRangeStations: Int64
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double?
    /// Reports made.
    public var reports: Int64
    /// Signal time since the start.
    public var signalTime: Double
    /// The view filter hides some stations.
    public var filtered: Bool
    /// Names in the name table.
    public var names: Int64
    /// The start of the next pass of the satellite over the station.
    public var nextPass: Double?
    /// The highest elevation of the next pass.
    public var nextPassElevationDeg: Double?

    init(_ c: sdrkit_ais_fields) {
        state = State(rawValue: c.state)
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        bursts = c.bursts
        recovered = c.recovered
        cancelled = c.cancelled
        collisionRecoveries = c.collision_recoveries
        stations = c.stations
        withPosition = c.with_position
        longRangeStations = c.long_range_stations
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        reports = c.reports
        signalTime = c.signal_time
        filtered = c.filtered
        names = c.names
        nextPass = c.has_next_pass ? c.next_pass : nil
        nextPassElevationDeg = c.has_next_pass_elevation_deg ? c.next_pass_elevation_deg : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AisFields, SDRKitCode> {
        var c = sdrkit_ais_fields()
        let code = sdrkit_ais_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AisFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the AIS station table. The row key is the MMSI.
public struct AisStationRow {
    /// The row key.
    public var key: String
    /// The station name, else its MMSI.
    public var name: String
    /// The MMSI, nine digits.
    public var mmsi: String
    /// The call sign of a station heard.
    public var callsign: String?
    /// The ship type in words, else the class.
    public var type: String
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The position came from message 27.
    public var longRangePosition: Bool
    /// The channel or talk group of a call.
    public var channel: String
    /// The navigational status in words.
    public var status: String?
    /// The destination.
    public var destination: String?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// Times heard.
    public var heard: Int64?

    init(_ c: sdrkit_ais_station_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        mmsi = sdrkitText(c.mmsi)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        type = sdrkitText(c.type)
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        longRangePosition = c.long_range_position
        channel = sdrkitText(c.channel)
        status = c.has_status ? sdrkitText(c.status) : nil
        destination = c.has_destination ? sdrkitText(c.destination) : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A row of the track points of the stations whose track is shown. The row key is the MMSI, `/` and the time of the point.
public struct AisTrackPointRow {
    /// The row key.
    public var key: String
    /// The station a track point belongs to.
    public var station: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// When (wall-clock time, when known).
    public var time: Double?

    init(_ c: sdrkit_ais_track_point_row) {
        key = sdrkitText(c.key)
        station = sdrkitText(c.station)
        position = SDRKitPosition(c.position)
        time = c.has_time ? c.time : nil
    }
}

/// A row of the count of messages by type. The row key is the message type in two digits.
public struct AisCounterRow {
    /// The row key.
    public var key: String
    /// The message type name.
    public var type: String
    /// Messages on AIS 1.
    public var countA: Int64
    /// Messages on AIS 2.
    public var countB: Int64
    /// Messages on channel 75.
    public var countC: Int64
    /// Messages on channel 76.
    public var countD: Int64
    /// Messages on every channel.
    public var total: Int64

    init(_ c: sdrkit_ais_counter_row) {
        key = sdrkitText(c.key)
        type = sdrkitText(c.type)
        countA = c.count_a
        countB = c.count_b
        countC = c.count_c
        countD = c.count_d
        total = c.total
    }
}

/// A row of the channel table of the AIS stack. The row key is the letter of the channel.
public struct AisChannelRow {
    /// The row key.
    public var key: String
    /// The channel's name.
    public var name: String
    /// The frequency a station was heard on.
    public var freqHz: Double
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Good messages in the last minute.
    public var messagesPerMin: Double
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Noise floor, dBFS.
    public var noiseDbfs: Double?
    /// The Doppler of the channel's bursts now (satellite mode).
    public var dopplerHz: Double?
    /// The rate of the Doppler (satellite mode).
    public var dopplerRateHzS: Double?

    init(_ c: sdrkit_ais_channel_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        freqHz = c.freq_hz
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        messagesPerMin = c.messages_per_min
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        dopplerHz = c.has_doppler_hz ? c.doppler_hz : nil
        dopplerRateHzS = c.has_doppler_rate_hz_s ? c.doppler_rate_hz_s : nil
    }
}

/// A row of the NMEA sentence table. The row key is the place of the sentence in the log.
public struct AisSentenceRow {
    /// The row key.
    public var key: String
    /// An `!AIVDM` sentence.
    public var sentence: String

    init(_ c: sdrkit_ais_sentence_row) {
        key = sdrkitText(c.key)
        sentence = sdrkitText(c.sentence)
    }
}

/// A row of the reception table. The row key is the place of the reception in the log.
public struct AisReceptionRow {
    public enum Layer: Int32 {
        case direct = 0
        case recovered = 1
        case cancelled = 2
    }
    /// The row key.
    public var key: String
    /// When (wall-clock time, when known).
    public var time: Double?
    /// The channel or talk group of a call.
    public var channel: String
    /// The message type.
    public var msgType: Int64
    /// The MMSI, nine digits.
    public var mmsi: String
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// How a message was found.
    public var layer: Layer?

    init(_ c: sdrkit_ais_reception_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        channel = sdrkitText(c.channel)
        msgType = c.msg_type
        mmsi = sdrkitText(c.mmsi)
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        layer = Layer(rawValue: c.layer)
    }
}

/// A row of the alert table. The row key is the place of the alert in the log.
public struct AisAlertRow {
    public enum Kind: Int32 {
        case cpa = 0
        case enter = 1
        case exit = 2
    }
    /// The row key.
    public var key: String
    /// When (wall-clock time, when known).
    public var time: Double?
    /// What an alert is about.
    public var kind: Kind?
    /// The fence of an enter or exit.
    public var fence: String?
    /// The MMSI, nine digits.
    public var mmsi: String
    /// The station name, else its MMSI.
    public var name: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The distance from the station (CPA) or the fence.
    public var distanceM: Double
    /// The distance at the closest point of approach.
    public var cpaM: Double?
    /// The time to the closest point of approach.
    public var tcpa: Double?

    init(_ c: sdrkit_ais_alert_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        kind = Kind(rawValue: c.kind)
        fence = c.has_fence ? sdrkitText(c.fence) : nil
        mmsi = sdrkitText(c.mmsi)
        name = sdrkitText(c.name)
        position = SDRKitPosition(c.position)
        distanceM = c.distance_m
        cpaM = c.has_cpa_m ? c.cpa_m : nil
        tcpa = c.has_tcpa ? c.tcpa : nil
    }
}

/// A row of the hop plan of a receiver that is narrower than the four channels. The row key is the number of the step.
public struct AisHopRow {
    /// The row key.
    public var key: String
    /// The centre of a hopping step.
    public var centreHz: Double
    /// The dwell of a hopping step (none for one step that covers all).
    public var dwell: Double?

    init(_ c: sdrkit_ais_hop_row) {
        key = sdrkitText(c.key)
        centreHz = c.centre_hz
        dwell = c.has_dwell ? c.dwell : nil
    }
}

/// The row of the selected station, with every value that the station sent. The row key is the MMSI.
public struct AisSelectedRow {
    /// The row key.
    public var key: String
    /// The station name, else its MMSI.
    public var name: String
    /// The MMSI, nine digits.
    public var mmsi: String
    /// The station class.
    public var `class`: String
    /// The call sign of a station heard.
    public var callsign: String?
    /// The IMO number.
    public var imo: Int64?
    /// The ship type in words, else the class.
    public var type: String?
    /// The destination.
    public var destination: String?
    /// The estimated arrival, "MM-DD HH:MM UTC".
    public var eta: String?
    /// The draught.
    public var draughtM: Double?
    /// The length.
    public var lengthM: Double?
    /// The beam.
    public var beamM: Double?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The true heading.
    public var headingDeg: Double?
    /// The rate of turn.
    public var rateOfTurnDegMin: Double?
    /// The altitude of a SAR aircraft.
    public var altitudeM: Double?
    /// The navigational status in words.
    public var status: String?
    /// Times heard.
    public var heard: Int64?
    /// The type of the last message.
    public var lastType: Int64
    /// The channel or talk group of a call.
    public var channel: String
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// It sent message 27.
    public var longRange: Bool
    /// The position latency flag of message 27.
    public var latency: Bool?
    /// The last sentences, one per line.
    public var sentences: String

    init(_ c: sdrkit_ais_selected_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        mmsi = sdrkitText(c.mmsi)
        `class` = sdrkitText(c.class_)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        imo = c.has_imo ? c.imo : nil
        type = c.has_type ? sdrkitText(c.type) : nil
        destination = c.has_destination ? sdrkitText(c.destination) : nil
        eta = c.has_eta ? sdrkitText(c.eta) : nil
        draughtM = c.has_draught_m ? c.draught_m : nil
        lengthM = c.has_length_m ? c.length_m : nil
        beamM = c.has_beam_m ? c.beam_m : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        headingDeg = c.has_heading_deg ? c.heading_deg : nil
        rateOfTurnDegMin = c.has_rate_of_turn_deg_min ? c.rate_of_turn_deg_min : nil
        altitudeM = c.has_altitude_m ? c.altitude_m : nil
        status = c.has_status ? sdrkitText(c.status) : nil
        heard = c.has_heard ? c.heard : nil
        lastType = c.last_type
        channel = sdrkitText(c.channel)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        longRange = c.long_range
        latency = c.has_latency ? c.latency : nil
        sentences = sdrkitText(c.sentences)
    }
}

/// The result fields of the marine HF stack.
public struct MarineHfFields {
    public enum State: Int32 {
        case searching = 0
        case signal = 1
        case decoding = 2
    }
    public enum ActiveService: Int32 {
        case none = 0
        case navtex = 1
        case dsc = 2
        case dscVhf = 3
        case fax = 4
    }
    public enum FaxState: Int32 {
        case waiting = 0
        case phasing = 1
        case receiving = 2
        case stopped = 3
    }
    /// The reception state of a display.
    public var state: State?
    /// The service heard last.
    public var activeService: ActiveService?
    /// An FSK pair stands above the noise.
    public var fskFound: Bool
    /// The lower FSK tone, from the dial.
    public var fskLowHz: Double?
    /// The higher FSK tone, from the dial.
    public var fskHighHz: Double?
    /// The measured shift.
    public var shiftHz: Double?
    /// Symbol rate found, Hz.
    public var symbolRateHz: Double?
    /// The level of the tones over the noise, per bin.
    public var toneLevelDb: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double?
    /// NAVTEX messages complete with no unreadable character.
    public var navtexGood: Int64
    /// Other NAVTEX messages.
    public var navtexBad: Int64
    /// DSC calls with a good check.
    public var dscGood: Int64
    /// DSC calls with a failed check.
    public var dscBad: Int64
    /// The DSC list holds a distress call.
    public var distress: Bool
    /// The fax decoder's state.
    public var faxState: FaxState?
    /// The index of cooperation.
    public var ioc: Int64?
    /// Lines per minute.
    public var lpm: Int64?
    /// Picture lines received.
    public var lines: Int64?
    /// Picture width, pixels.
    public var width: Int64?
    /// The slant in use.
    public var slantPpm: Double?
    /// The black level.
    public var blackLevelHz: Double?
    /// The last start or stop tone measured.
    public var lastToneHz: Double?
    /// Phasing lines seen.
    public var phasingLines: Int64?
    /// The fax SNR: tone over audio power, or the black-white swing over the noise of the black level.
    public var faxSnrDb: Double?
    /// Pictures started.
    public var picturesStarted: Int64?
    /// Pictures finished.
    public var picturesDone: Int64?
    /// Why the last picture ended.
    public var endReason: String?
    /// The channel the scanner holds.
    public var scanChannelHz: Double?
    /// Reports made.
    public var reports: Int64
    /// Signal time since the start.
    public var signalTime: Double

    init(_ c: sdrkit_marine_hf_fields) {
        state = State(rawValue: c.state)
        activeService = ActiveService(rawValue: c.active_service)
        fskFound = c.fsk_found
        fskLowHz = c.has_fsk_low_hz ? c.fsk_low_hz : nil
        fskHighHz = c.has_fsk_high_hz ? c.fsk_high_hz : nil
        shiftHz = c.has_shift_hz ? c.shift_hz : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        toneLevelDb = c.has_tone_level_db ? c.tone_level_db : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        navtexGood = c.navtex_good
        navtexBad = c.navtex_bad
        dscGood = c.dsc_good
        dscBad = c.dsc_bad
        distress = c.distress
        faxState = c.has_fax_state ? FaxState(rawValue: c.fax_state) : nil
        ioc = c.has_ioc ? c.ioc : nil
        lpm = c.has_lpm ? c.lpm : nil
        lines = c.has_lines ? c.lines : nil
        width = c.has_width ? c.width : nil
        slantPpm = c.has_slant_ppm ? c.slant_ppm : nil
        blackLevelHz = c.has_black_level_hz ? c.black_level_hz : nil
        lastToneHz = c.has_last_tone_hz ? c.last_tone_hz : nil
        phasingLines = c.has_phasing_lines ? c.phasing_lines : nil
        faxSnrDb = c.has_fax_snr_db ? c.fax_snr_db : nil
        picturesStarted = c.has_pictures_started ? c.pictures_started : nil
        picturesDone = c.has_pictures_done ? c.pictures_done : nil
        endReason = c.has_end_reason ? sdrkitText(c.end_reason) : nil
        scanChannelHz = c.has_scan_channel_hz ? c.scan_channel_hz : nil
        reports = c.reports
        signalTime = c.signal_time
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MarineHfFields, SDRKitCode> {
        var c = sdrkit_marine_hf_fields()
        let code = sdrkit_marine_hf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MarineHfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the NAVTEX message table. The row key is the place of the message in the log.
public struct MarineNavtexRow {
    /// The row key.
    public var key: String
    /// When a message or call was heard (wall time).
    public var time: Double?
    /// B1 B2 B3 B4.
    public var header: String
    /// The name of the subject.
    public var subjectName: String
    /// Characters of the text.
    public var chars: Int64
    /// Unreadable characters.
    public var errors: Int64
    /// NNNN was seen.
    public var complete: Bool
    /// Times the message came again.
    public var repeats: Int64
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_marine_navtex_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        header = sdrkitText(c.header)
        subjectName = sdrkitText(c.subject_name)
        chars = c.chars
        errors = c.errors
        complete = c.complete
        repeats = c.repeats
        text = sdrkitText(c.text)
    }
}

/// A row of the DSC call table. The row key is the place of the call in the log.
public struct MarineDscRow {
    /// The row key.
    public var key: String
    /// When a message or call was heard (wall time).
    public var time: Double?
    /// The frequency a station was heard on.
    public var freqHz: Double?
    /// The kind of call in words.
    public var kind: String
    /// The sender's MMSI.
    public var fromMmsi: String?
    /// The sender's name.
    public var fromName: String?
    /// The sender's country.
    public var fromCountry: String?
    /// The called station.
    public var toMmsi: String?
    /// The category in words.
    public var categoryName: String?
    /// The nature of distress in words.
    public var natureName: String?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The telecommands in words.
    public var telecommands: String?
    /// The receive frequency or channel the call proposes.
    public var freqRx: String?
    /// The transmit frequency or channel the call proposes.
    public var freqTx: String?
    /// The error-check character matched.
    public var eccOk: Bool
    /// Symbols taken from the second copy or rebuilt.
    public var repaired: Int64
    /// Symbols unreadable in both copies.
    public var erasures: Int64
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_marine_dsc_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        kind = sdrkitText(c.kind)
        fromMmsi = c.has_from_mmsi ? sdrkitText(c.from_mmsi) : nil
        fromName = c.has_from_name ? sdrkitText(c.from_name) : nil
        fromCountry = c.has_from_country ? sdrkitText(c.from_country) : nil
        toMmsi = c.has_to_mmsi ? sdrkitText(c.to_mmsi) : nil
        categoryName = c.has_category_name ? sdrkitText(c.category_name) : nil
        natureName = c.has_nature_name ? sdrkitText(c.nature_name) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        telecommands = c.has_telecommands ? sdrkitText(c.telecommands) : nil
        freqRx = c.has_freq_rx ? sdrkitText(c.freq_rx) : nil
        freqTx = c.has_freq_tx ? sdrkitText(c.freq_tx) : nil
        eccOk = c.ecc_ok
        repaired = c.repaired
        erasures = c.erasures
        text = sdrkitText(c.text)
    }
}

/// The result fields of the VDES stack.
public struct VdesFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Bursts found on the VDE-TER channels.
    public var bursts: Int64
    /// Reports made.
    public var reports: Int64

    init(_ c: sdrkit_vdes_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        bursts = c.bursts
        reports = c.reports
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<VdesFields, SDRKitCode> {
        var c = sdrkit_vdes_fields()
        let code = sdrkit_vdes_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VdesFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ASM frame table. The row key is the place of the frame in the log.
public struct VdesAsmRow {
    /// The row key.
    public var key: String
    /// When (wall-clock time, when known).
    public var time: Double?
    /// The channel or talk group of a call.
    public var channel: String
    /// The payload length.
    public var bits: Int64
    /// The AIS message type of the payload.
    public var msgType: Int64?
    /// The MMSI, nine digits.
    public var mmsi: String?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The payload in hexadecimal.
    public var hex: String

    init(_ c: sdrkit_vdes_asm_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        channel = sdrkitText(c.channel)
        bits = c.bits
        msgType = c.has_msg_type ? c.msg_type : nil
        mmsi = c.has_mmsi ? sdrkitText(c.mmsi) : nil
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        position = c.has_position ? SDRKitPosition(c.position) : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        hex = sdrkitText(c.hex)
    }
}

/// A row of the VDE-TER burst table. The row key is the place of the burst in the log.
public struct VdesBurstRow {
    public enum Modulation: Int32 {
        case pi4QPSK = 0
        case n8PSK = 1
        case n16QAM = 2
        case unknown = 3
    }
    /// The row key.
    public var key: String
    /// When (wall-clock time, when known).
    public var time: Double?
    /// The centre of a burst's occupied sub-channels.
    public var centreHz: Double
    /// The bandwidth of a burst.
    public var bandwidthHz: Double
    /// The modulation of a burst.
    public var modulation: Modulation?
    /// The peak power of a burst.
    public var rssiDbfs: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// How long a burst lasted.
    public var duration: Double
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// The spread of the symbol amplitudes.
    public var cv: Double
    /// The mean of cos(4 x phase step).
    public var psi: Double
    /// The symbols used for the classification.
    public var symbols: Int64
    /// The occupied sub-channels.
    public var subchannels: String

    init(_ c: sdrkit_vdes_burst_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        centreHz = c.centre_hz
        bandwidthHz = c.bandwidth_hz
        modulation = Modulation(rawValue: c.modulation)
        rssiDbfs = c.rssi_dbfs
        snrDb = c.snr_db
        duration = c.duration
        offsetHz = c.offset_hz
        cv = c.cv
        psi = c.psi
        symbols = c.symbols
        subchannels = sdrkitText(c.subchannels)
    }
}

/// A row of the channel table of the VDES stack. The row key is the letter of an ASM channel, or `TER` and the number of a VDE-TER band.
public struct VdesChannelRow {
    /// The row key.
    public var key: String
    /// The name of a channel.
    public var name: String
    /// The frequency a station was heard on.
    public var freqHz: Double
    /// The capture holds the channel.
    public var inView: Bool
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64?
    /// Frames that failed their check.
    public var framesBad: Int64?
    /// Bursts found on the VDE-TER channels.
    public var bursts: Int64?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Noise floor, dBFS.
    public var noiseDbfs: Double?

    init(_ c: sdrkit_vdes_channel_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        freqHz = c.freq_hz
        inView = c.in_view
        framesOk = c.has_frames_ok ? c.frames_ok : nil
        framesBad = c.has_frames_bad ? c.frames_bad : nil
        bursts = c.has_bursts ? c.bursts : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
    }
}

/// A row of the count of bursts by modulation. The row key is the name of the modulation.
public struct VdesModulationRow {
    public enum Modulation: Int32 {
        case pi4QPSK = 0
        case n8PSK = 1
        case n16QAM = 2
        case unknown = 3
    }
    /// The row key.
    public var key: String
    /// The modulation of a burst.
    public var modulation: Modulation?
    /// A number of bursts.
    public var count: Int64

    init(_ c: sdrkit_vdes_modulation_row) {
        key = sdrkitText(c.key)
        modulation = Modulation(rawValue: c.modulation)
        count = c.count
    }
}

/// A row of the count of bursts by bandwidth. The row key is the bandwidth in Hz.
public struct VdesBandwidthRow {
    /// The row key.
    public var key: String
    /// The bandwidth of a burst.
    public var bandwidthHz: Double
    /// A number of bursts.
    public var count: Int64

    init(_ c: sdrkit_vdes_bandwidth_row) {
        key = sdrkitText(c.key)
        bandwidthHz = c.bandwidth_hz
        count = c.count
    }
}

/// The result fields of the marine VHF stack.
public struct MarinevhfVhfFields {
    public enum ScanState: Int32 {
        case idle = 0
        case priority = 1
        case priorityProbe = 2
        case scanning = 3
        case holding = 4
    }
    /// Carriers the finder tracks.
    public var carriers: Int64
    /// Carriers active now.
    public var activeChannels: Int64
    /// The channel that plays.
    public var playing: String?
    /// The frequency that plays.
    public var playingHz: Double?
    /// What the player does.
    public var scanState: ScanState?
    /// The frequency the single-tuner scan requested from the radio.
    public var tuneHz: Double?
    /// Calls since the start.
    public var calls: Int64
    /// The capture holds more than one channel.
    public var multiChannel: Bool
    /// Reports made.
    public var reports: Int64

    init(_ c: sdrkit_marinevhf_vhf_fields) {
        carriers = c.carriers
        activeChannels = c.active_channels
        playing = c.has_playing ? sdrkitText(c.playing) : nil
        playingHz = c.has_playing_hz ? c.playing_hz : nil
        scanState = ScanState(rawValue: c.scan_state)
        tuneHz = c.has_tune_hz ? c.tune_hz : nil
        calls = c.calls
        multiChannel = c.multi_channel
        reports = c.reports
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MarinevhfVhfFields, SDRKitCode> {
        var c = sdrkit_marinevhf_vhf_fields()
        let code = sdrkit_marinevhf_vhf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MarinevhfVhfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the call table of the marine VHF stack. The row key is the place of the call in the log.
public struct MarinevhfVhfCallRow {
    /// The row key.
    public var key: String
    /// The number of a call.
    public var serial: Int64
    /// The channel or talk group of a call.
    public var channel: String
    /// The frequency a station was heard on.
    public var freqHz: Double
    /// When a call started (wall-clock time, when known).
    public var time: Double?
    /// The signal time a call started.
    public var start: Double
    /// How long a call lasted.
    public var duration: Double
    /// The highest level of a call or channel.
    public var peakDbfs: Double
    /// The mean level of a call.
    public var meanDbfs: Double
    /// The call or channel is open.
    public var `open`: Bool

    init(_ c: sdrkit_marinevhf_vhf_call_row) {
        key = sdrkitText(c.key)
        serial = c.serial
        channel = sdrkitText(c.channel)
        freqHz = c.freq_hz
        time = c.has_time ? c.time : nil
        start = c.start
        duration = c.duration
        peakDbfs = c.peak_dbfs
        meanDbfs = c.mean_dbfs
        `open` = c.open
    }
}

/// A row of the channel table of the marine VHF stack. The row key is the channel frequency in units of the raster.
public struct MarinevhfVhfChannelRow {
    /// The row key.
    public var key: String
    /// The channel or talk group of a call.
    public var channel: String
    /// The frequency a station was heard on.
    public var freqHz: Double
    /// The channel carries a signal now.
    public var active: Bool
    /// The call or channel is open.
    public var `open`: Bool
    /// The channel plays.
    public var nowPlaying: Bool
    /// The channel is in the scan list.
    public var inScan: Bool
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Calls on the channel.
    public var calls: Int64
    /// The time the channel was open in all.
    public var total: Double
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// The highest level of a call or channel.
    public var peakDbfs: Double?

    init(_ c: sdrkit_marinevhf_vhf_channel_row) {
        key = sdrkitText(c.key)
        channel = sdrkitText(c.channel)
        freqHz = c.freq_hz
        active = c.active
        `open` = c.open
        nowPlaying = c.now_playing
        inScan = c.in_scan
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        calls = c.calls
        total = c.total
        lastHeard = c.has_last_heard ? c.last_heard : nil
        peakDbfs = c.has_peak_dbfs ? c.peak_dbfs : nil
    }
}

/// The result fields of the ATIS stack.
public struct AtisFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Checked DSC calls of another format.
    public var otherCalls: Int64
    /// Stations in the table.
    public var stations: Int64
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Reports made.
    public var reports: Int64

    init(_ c: sdrkit_atis_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        otherCalls = c.other_calls
        stations = c.stations
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        reports = c.reports
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AtisFields, SDRKitCode> {
        var c = sdrkit_atis_fields()
        let code = sdrkit_atis_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AtisFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ATIS message table. The row key is the place of the message in the log.
public struct AtisMessageRow {
    /// The row key.
    public var key: String
    /// When (wall-clock time, when known).
    public var time: Double?
    /// The ten digits.
    public var digits: String
    /// The call sign of a station heard.
    public var callsign: String
    /// The nine digits after the 9.
    public var mmsi: String
    /// The country of the MID.
    public var country: String
    /// The name from the name table.
    public var name: String
    /// The check symbol agrees.
    public var checkOk: Bool
    /// The identity starts with 9.
    public var standardZ: Bool
    /// The tones were swapped.
    public var inverted: Bool
    /// Symbols taken from the second copy or rebuilt.
    public var repaired: Int64
    /// Symbols unreadable in both copies.
    public var erasures: Int64
    /// The level over the floor.
    public var levelDb: Double
    /// The offset of the branch that found the message.
    public var branchHz: Double

    init(_ c: sdrkit_atis_message_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        digits = sdrkitText(c.digits)
        callsign = sdrkitText(c.callsign)
        mmsi = sdrkitText(c.mmsi)
        country = sdrkitText(c.country)
        name = sdrkitText(c.name)
        checkOk = c.check_ok
        standardZ = c.standard_z
        inverted = c.inverted
        repaired = c.repaired
        erasures = c.erasures
        levelDb = c.level_db
        branchHz = c.branch_hz
    }
}

/// A row of the ATIS station table. The row key is the digits of the identity.
public struct AtisStationRow {
    /// The row key.
    public var key: String
    /// The ten digits.
    public var digits: String
    /// The call sign of a station heard.
    public var callsign: String
    /// The nine digits after the 9.
    public var mmsi: String
    /// The country of the MID.
    public var country: String
    /// The name from the name table.
    public var name: String
    /// The best level over the floor.
    public var bestLevelDb: Double
    /// Times heard.
    public var heard: Int64?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_atis_station_row) {
        key = sdrkitText(c.key)
        digits = sdrkitText(c.digits)
        callsign = sdrkitText(c.callsign)
        mmsi = sdrkitText(c.mmsi)
        country = sdrkitText(c.country)
        name = sdrkitText(c.name)
        bestLevelDb = c.best_level_db
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The fields of the event of a station heard.
public struct AisVesselEvent {
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
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// The text of a decoded message.
    public var text: String?
    /// The station name, else its MMSI.
    public var name: String
    /// The call sign of a station heard.
    public var callsign: String
    /// The ship and cargo type code.
    public var shipType: Int64?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The navigational status code.
    public var navStatus: Int64?
    /// The station class.
    public var `class`: String
    /// The destination.
    public var destination: String

    init(_ c: sdrkit_ais_vessel_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        text = c.has_text ? sdrkitText(c.text) : nil
        name = sdrkitText(c.name)
        callsign = sdrkitText(c.callsign)
        shipType = c.has_ship_type ? c.ship_type : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        navStatus = c.has_nav_status ? c.nav_status : nil
        `class` = sdrkitText(c.class_)
        destination = sdrkitText(c.destination)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AisVesselEvent, SDRKitCode> {
        var c = sdrkit_ais_vessel_event()
        let code = sdrkit_ais_vessel_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AisVesselEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of an NMEA sentence.
public struct AisNmeaEvent {
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// An `!AIVDM` sentence.
    public var sentence: String

    init(_ c: sdrkit_ais_nmea_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
        sentence = sdrkitText(c.sentence)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AisNmeaEvent, SDRKitCode> {
        var c = sdrkit_ais_nmea_event()
        let code = sdrkit_ais_nmea_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AisNmeaEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a closest point of approach alert.
public struct AisCpaEvent {
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
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// The station name, else its MMSI.
    public var name: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The distance from the station (CPA) or the fence.
    public var distanceM: Double
    /// The distance at the closest point of approach.
    public var cpaM: Double?
    /// The time to the closest point of approach.
    public var tcpa: Double?

    init(_ c: sdrkit_ais_cpa_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        name = sdrkitText(c.name)
        position = SDRKitPosition(c.position)
        distanceM = c.distance_m
        cpaM = c.has_cpa_m ? c.cpa_m : nil
        tcpa = c.has_tcpa ? c.tcpa : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AisCpaEvent, SDRKitCode> {
        var c = sdrkit_ais_cpa_event()
        let code = sdrkit_ais_cpa_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AisCpaEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a geofence alert.
public struct AisGeofenceEvent {
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
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    public enum Transition: Int32 {
        case enter = 0
        case exit = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// The station name, else its MMSI.
    public var name: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The distance from the station (CPA) or the fence.
    public var distanceM: Double
    /// The fence of an enter or exit.
    public var fence: String
    /// A fence crossed: enter or exit.
    public var transition: Transition?

    init(_ c: sdrkit_ais_geofence_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        name = sdrkitText(c.name)
        position = SDRKitPosition(c.position)
        distanceM = c.distance_m
        fence = sdrkitText(c.fence)
        transition = Transition(rawValue: c.transition)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AisGeofenceEvent, SDRKitCode> {
        var c = sdrkit_ais_geofence_event()
        let code = sdrkit_ais_geofence_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AisGeofenceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a NAVTEX message.
public struct MarineNavtexMessageEvent {
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
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// The text of a decoded message.
    public var text: String
    /// B1 B2 B3 B4.
    public var header: String
    /// B2, the subject indicator.
    public var subject: String
    /// The name of the subject.
    public var subjectName: String
    /// B3 B4, the serial number.
    public var number: Int64
    /// NNNN was seen.
    public var complete: Bool
    /// Unreadable characters.
    public var errors: Int64
    /// Characters of the text.
    public var chars: Int64

    init(_ c: sdrkit_marine_navtex_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        text = sdrkitText(c.text)
        header = sdrkitText(c.header)
        subject = sdrkitText(c.subject)
        subjectName = sdrkitText(c.subject_name)
        number = c.number
        complete = c.complete
        errors = c.errors
        chars = c.chars
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MarineNavtexMessageEvent, SDRKitCode> {
        var c = sdrkit_marine_navtex_message_event()
        let code = sdrkit_marine_navtex_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MarineNavtexMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a DSC call.
public struct MarineDscCallEvent {
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
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// The text of a decoded message.
    public var text: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The format specifier.
    public var format: Int64
    /// The kind of call in words.
    public var kind: String
    /// The category code.
    public var category: Int64?
    /// The category in words.
    public var categoryName: String?
    /// The sender's name.
    public var fromName: String?
    /// The sender's country.
    public var fromCountry: String?
    /// What the called station is.
    public var toType: String?
    /// The station in distress.
    public var distressMmsi: String?
    /// The nature of distress code.
    public var nature: Int64?
    /// The nature of distress in words.
    public var natureName: String?
    /// The position in the call, as sent.
    public var positionText: String?
    /// The time in the call (HH:MM UTC).
    public var utc: String?
    /// The first telecommand.
    public var telecommand1: Int64?
    /// The second telecommand.
    public var telecommand2: Int64?
    /// The telecommands in words.
    public var telecommands: String?
    /// The receive frequency or channel the call proposes.
    public var freqRx: String
    /// The transmit frequency or channel the call proposes.
    public var freqTx: String
    /// The end of sequence character.
    public var eos: Int64?
    /// The error-check character matched.
    public var eccOk: Bool
    /// Symbols taken from the second copy or rebuilt.
    public var repaired: Int64
    /// Symbols unreadable in both copies.
    public var erasures: Int64
    /// Heard on VHF channel 70.
    public var vhf: Bool

    init(_ c: sdrkit_marine_dsc_call_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        text = sdrkitText(c.text)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        format = c.format
        kind = sdrkitText(c.kind)
        category = c.has_category ? c.category : nil
        categoryName = c.has_category_name ? sdrkitText(c.category_name) : nil
        fromName = c.has_from_name ? sdrkitText(c.from_name) : nil
        fromCountry = c.has_from_country ? sdrkitText(c.from_country) : nil
        toType = c.has_to_type ? sdrkitText(c.to_type) : nil
        distressMmsi = c.has_distress_mmsi ? sdrkitText(c.distress_mmsi) : nil
        nature = c.has_nature ? c.nature : nil
        natureName = c.has_nature_name ? sdrkitText(c.nature_name) : nil
        positionText = c.has_position_text ? sdrkitText(c.position_text) : nil
        utc = c.has_utc ? sdrkitText(c.utc) : nil
        telecommand1 = c.has_telecommand1 ? c.telecommand1 : nil
        telecommand2 = c.has_telecommand2 ? c.telecommand2 : nil
        telecommands = c.has_telecommands ? sdrkitText(c.telecommands) : nil
        freqRx = sdrkitText(c.freq_rx)
        freqTx = sdrkitText(c.freq_tx)
        eos = c.has_eos ? c.eos : nil
        eccOk = c.ecc_ok
        repaired = c.repaired
        erasures = c.erasures
        vhf = c.vhf
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MarineDscCallEvent, SDRKitCode> {
        var c = sdrkit_marine_dsc_call_event()
        let code = sdrkit_marine_dsc_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MarineDscCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of an ASM frame.
public struct VdesAsmEvent {
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The channel or talk group of a call.
    public var channel: String
    /// The payload length.
    public var bits: Int64
    /// The payload in hexadecimal.
    public var hex: String
    /// The AIS message type of the payload.
    public var msgType: Int64?

    init(_ c: sdrkit_vdes_asm_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        channel = sdrkitText(c.channel)
        bits = c.bits
        hex = sdrkitText(c.hex)
        msgType = c.has_msg_type ? c.msg_type : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VdesAsmEvent, SDRKitCode> {
        var c = sdrkit_vdes_asm_event()
        let code = sdrkit_vdes_asm_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VdesAsmEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of an NMEA sentence of an ASM frame.
public struct VdesNmeaEvent {
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// An `!AIVDM` sentence.
    public var sentence: String

    init(_ c: sdrkit_vdes_nmea_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        sentence = sdrkitText(c.sentence)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VdesNmeaEvent, SDRKitCode> {
        var c = sdrkit_vdes_nmea_event()
        let code = sdrkit_vdes_nmea_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VdesNmeaEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a VDE-TER burst.
public struct VdesBurstEvent {
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
    public enum Modulation: Int32 {
        case pi4QPSK = 0
        case n8PSK = 1
        case n16QAM = 2
        case unknown = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The modulation of a burst.
    public var modulation: Modulation?
    /// The bandwidth of a burst.
    public var bandwidthHz: Double
    /// How long a burst lasted.
    public var duration: Double

    init(_ c: sdrkit_vdes_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        modulation = Modulation(rawValue: c.modulation)
        bandwidthHz = c.bandwidth_hz
        duration = c.duration
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VdesBurstEvent, SDRKitCode> {
        var c = sdrkit_vdes_burst_event()
        let code = sdrkit_vdes_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VdesBurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a call on a marine VHF channel.
public struct MarinevhfCallEvent {
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// The channel or talk group of a call.
    public var channel: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The signal time a call started.
    public var start: Double
    /// How long a call lasted.
    public var duration: Double
    /// The highest level of a call or channel.
    public var peakDbfs: Double

    init(_ c: sdrkit_marinevhf_call_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = sdrkitText(c.channel)
        family = Family(rawValue: c.family)
        start = c.start
        duration = c.duration
        peakDbfs = c.peak_dbfs
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MarinevhfCallEvent, SDRKitCode> {
        var c = sdrkit_marinevhf_call_event()
        let code = sdrkit_marinevhf_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MarinevhfCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of an ATIS message.
public struct AtisMessageEvent {
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// The channel or talk group of a call.
    public var channel: String?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The call sign of a station heard.
    public var callsign: String
    /// The nine digits after the 9.
    public var mmsi: String
    /// The country of the MID.
    public var country: String
    /// The check symbol agrees.
    public var checkOk: Bool

    init(_ c: sdrkit_atis_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        family = Family(rawValue: c.family)
        callsign = sdrkitText(c.callsign)
        mmsi = sdrkitText(c.mmsi)
        country = sdrkitText(c.country)
        checkOk = c.check_ok
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AtisMessageEvent, SDRKitCode> {
        var c = sdrkit_atis_message_event()
        let code = sdrkit_atis_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AtisMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum MarineTables {
    /// The changed rows of the table "stations" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Course: `course_deg`. Speed: `speed_kt`.
    public static func aisStations(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[AisStationRow], SDRKitCode> {
        var rows = [sdrkit_ais_station_row](repeating: sdrkit_ais_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisStationRow.init))
    }
    /// The changed rows of the table "track_points" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `station` names a row.
    /// Place of a row: the column `position`.
    public static func aisTrackPoints(_ handle: SDRKIT_HANDLE, capacity: Int = 2500) -> Result<[AisTrackPointRow], SDRKitCode> {
        var rows = [sdrkit_ais_track_point_row](repeating: sdrkit_ais_track_point_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_track_points_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisTrackPointRow.init))
    }
    /// The changed rows of the table "counters" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func aisCounters(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[AisCounterRow], SDRKitCode> {
        var rows = [sdrkit_ais_counter_row](repeating: sdrkit_ais_counter_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_counters_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisCounterRow.init))
    }
    /// The changed rows of the table "channels" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Default order: the column `freq_hz`, lowest first.
    public static func aisChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[AisChannelRow], SDRKitCode> {
        var rows = [sdrkit_ais_channel_row](repeating: sdrkit_ais_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisChannelRow.init))
    }
    /// The changed rows of the table "sentences" of the stack "ais", newest first, at most `capacity` rows.
    public static func aisSentences(_ handle: SDRKIT_HANDLE, capacity: Int = 50) -> Result<[AisSentenceRow], SDRKitCode> {
        var rows = [sdrkit_ais_sentence_row](repeating: sdrkit_ais_sentence_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_sentences_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisSentenceRow.init))
    }
    /// The changed rows of the table "receptions" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `mmsi` names a row.
    public static func aisReceptions(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AisReceptionRow], SDRKitCode> {
        var rows = [sdrkit_ais_reception_row](repeating: sdrkit_ais_reception_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_receptions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisReceptionRow.init))
    }
    /// The changed rows of the table "alerts" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    public static func aisAlerts(_ handle: SDRKIT_HANDLE, capacity: Int = 50) -> Result<[AisAlertRow], SDRKitCode> {
        var rows = [sdrkit_ais_alert_row](repeating: sdrkit_ais_alert_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_alerts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisAlertRow.init))
    }
    /// The changed rows of the table "hop_plan" of the stack "ais", newest first, at most `capacity` rows.
    /// Default order: the column `centre_hz`, lowest first.
    public static func aisHopPlan(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[AisHopRow], SDRKitCode> {
        var rows = [sdrkit_ais_hop_row](repeating: sdrkit_ais_hop_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_hop_plan_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisHopRow.init))
    }
    /// The changed rows of the table "selected" of the stack "ais", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Height: `altitude_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func aisSelected(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[AisSelectedRow], SDRKitCode> {
        var rows = [sdrkit_ais_selected_row](repeating: sdrkit_ais_selected_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ais_selected_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AisSelectedRow.init))
    }
    /// The changed rows of the table "navtex" of the stack "marine", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `header` names a row.
    /// Text: `text`. Time: `time`.
    public static func marineNavtex(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MarineNavtexRow], SDRKitCode> {
        var rows = [sdrkit_marine_navtex_row](repeating: sdrkit_marine_navtex_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_marine_navtex_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MarineNavtexRow.init))
    }
    /// The changed rows of the table "dsc" of the stack "marine", newest first, at most `capacity` rows.
    /// The rows are the calls of the stack.
    /// The column `from_mmsi` names a row.
    /// Place of a row: the column `position`.
    /// Sender: `from_mmsi`. Addressee: `to_mmsi`. Text: `text`. Time: `time`.
    public static func marineDsc(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MarineDscRow], SDRKitCode> {
        var rows = [sdrkit_marine_dsc_row](repeating: sdrkit_marine_dsc_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_marine_dsc_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MarineDscRow.init))
    }
    /// The changed rows of the table "asm" of the stack "vdes", newest first, at most `capacity` rows.
    /// The column `mmsi` names a row.
    /// Place of a row: the column `position`.
    /// Course: `course_deg`. Speed: `speed_kt`.
    public static func vdesAsm(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[VdesAsmRow], SDRKitCode> {
        var rows = [sdrkit_vdes_asm_row](repeating: sdrkit_vdes_asm_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdes_asm_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VdesAsmRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "vdes", newest first, at most `capacity` rows.
    public static func vdesBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[VdesBurstRow], SDRKitCode> {
        var rows = [sdrkit_vdes_burst_row](repeating: sdrkit_vdes_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdes_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VdesBurstRow.init))
    }
    /// The changed rows of the table "channels" of the stack "vdes", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Default order: the column `freq_hz`, lowest first.
    public static func vdesChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[VdesChannelRow], SDRKitCode> {
        var rows = [sdrkit_vdes_channel_row](repeating: sdrkit_vdes_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdes_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VdesChannelRow.init))
    }
    /// The changed rows of the table "by_modulation" of the stack "vdes", newest first, at most `capacity` rows.
    /// The column `modulation` names a row.
    public static func vdesByModulation(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[VdesModulationRow], SDRKitCode> {
        var rows = [sdrkit_vdes_modulation_row](repeating: sdrkit_vdes_modulation_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdes_by_modulation_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VdesModulationRow.init))
    }
    /// The changed rows of the table "by_bandwidth" of the stack "vdes", newest first, at most `capacity` rows.
    /// The column `bandwidth_hz` names a row.
    /// Default order: the column `bandwidth_hz`, lowest first.
    public static func vdesByBandwidth(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[VdesBandwidthRow], SDRKitCode> {
        var rows = [sdrkit_vdes_bandwidth_row](repeating: sdrkit_vdes_bandwidth_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdes_by_bandwidth_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(VdesBandwidthRow.init))
    }
    /// The changed rows of the table "calls" of the stack "marinevhf", newest first, at most `capacity` rows.
    /// The rows are the calls of the stack.
    /// The column `channel` names a row.
    /// Time: `time`.
    public static func marinevhfCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[MarinevhfVhfCallRow], SDRKitCode> {
        var rows = [sdrkit_marinevhf_vhf_call_row](repeating: sdrkit_marinevhf_vhf_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_marinevhf_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MarinevhfVhfCallRow.init))
    }
    /// The changed rows of the table "channels" of the stack "marinevhf", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    /// Default order: the column `freq_hz`, lowest first.
    /// Time: `last_heard`.
    public static func marinevhfChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MarinevhfVhfChannelRow], SDRKitCode> {
        var rows = [sdrkit_marinevhf_vhf_channel_row](repeating: sdrkit_marinevhf_vhf_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_marinevhf_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MarinevhfVhfChannelRow.init))
    }
    /// The changed rows of the table "messages" of the stack "atis", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `name` names a row.
    /// Sender: `callsign`. Time: `time`.
    public static func atisMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[AtisMessageRow], SDRKitCode> {
        var rows = [sdrkit_atis_message_row](repeating: sdrkit_atis_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atis_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AtisMessageRow.init))
    }
    /// The changed rows of the table "stations" of the stack "atis", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func atisStations(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AtisStationRow], SDRKitCode> {
        var rows = [sdrkit_atis_station_row](repeating: sdrkit_atis_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atis_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AtisStationRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum MarineCommands {
    /// Select. Shows one station in full.
    public static func aisSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ais_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_ais_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ais_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func aisClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ais_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func aisRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ais_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_ais_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ais_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func marineClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_marine_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear the picture. Starts the fax picture again.
    public static func marineClearPicture(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_marine_clear_picture(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func vdesClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vdes_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func marinevhfClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_marinevhf_clear(handle)
        return sdrkitResult(code)
    }
    /// Close all. Ends every open call.
    public static func marinevhfCloseAll(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_marinevhf_close_all(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func atisClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_atis_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func atisRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_atis_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_atis_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_atis_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum MarineSettings {
    /// The options of the setting "channels" of the stack "ais".
    public enum AisChannels: Int32 {
        case all = 0
        case ais12 = 1
        case longrange = 2
    }
    /// Channels.
    public static func aisChannels(_ handle: SDRKIT_HANDLE) -> Result<AisChannels, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(AisChannels(rawValue: v) ?? AisChannels(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAisChannels(_ handle: SDRKIT_HANDLE, _ value: AisChannels) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channels", value.rawValue))
    }
    /// Collision handling.
    public static func aisCollisionCancel(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "collision_cancel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisCollisionCancel(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "collision_cancel", value))
    }
    /// Satellite mode.
    public static func aisSatellite(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "satellite", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisSatellite(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "satellite", value))
    }
    /// Hopping.
    public static func aisHop(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisHop(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop", value))
    }
    /// Time base.
    public static func aisTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
    /// Satellite elements.
    public static func aisSatelliteTle(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "satellite_tle", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAisSatelliteTle(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "satellite_tle", $0) })
    }
    /// Minimum elevation.
    public static func aisMinElevation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_elevation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisMinElevation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_elevation", value))
    }
    /// Arm the passes.
    public static func aisArmPasses(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "arm_passes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisArmPasses(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "arm_passes", value))
    }
    /// CPA alerts.
    public static func aisCpa(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "cpa", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisCpa(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "cpa", value))
    }
    /// CPA limit.
    public static func aisCpaNm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "cpa_nm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisCpaNm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "cpa_nm", value))
    }
    /// TCPA limit.
    public static func aisTcpaMin(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tcpa_min", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisTcpaMin(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tcpa_min", value))
    }
    /// CPA cool-down.
    public static func aisCpaCooldownMin(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "cpa_cooldown_min", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisCpaCooldownMin(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "cpa_cooldown_min", value))
    }
    /// Own ship moving.
    public static func aisOwnMoving(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "own_moving", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisOwnMoving(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "own_moving", value))
    }
    /// Own speed.
    public static func aisOwnSogKn(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "own_sog_kn", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisOwnSogKn(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "own_sog_kn", value))
    }
    /// Own course.
    public static func aisOwnCogDeg(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "own_cog_deg", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisOwnCogDeg(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "own_cog_deg", value))
    }
    /// Geofences.
    public static func aisGeofences(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "geofences", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAisGeofences(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "geofences", $0) })
    }
    /// Filter.
    public static func aisFilter(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAisFilter(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "filter", $0) })
    }
    /// Send AIS as NMEA.
    public static func aisNmeaOut(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "nmea_out", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisNmeaOut(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "nmea_out", value))
    }
    /// Name table.
    public static func aisNames(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "names", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAisNames(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "names", $0) })
    }
    /// The options of the setting "service" of the stack "marine".
    public enum MarineService: Int32 {
        case auto = 0
        case navtex = 1
        case dsc = 2
        case wefax = 3
    }
    /// Service.
    public static func marineService(_ handle: SDRKIT_HANDLE) -> Result<MarineService, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "service", &v)
        return code == SDRKIT_OK ? .success(MarineService(rawValue: v) ?? MarineService(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineService(_ handle: SDRKIT_HANDLE, _ value: MarineService) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "service", value.rawValue))
    }
    /// The options of the setting "fax_lpm" of the stack "marine".
    public enum MarineFaxLpm: Int32 {
        case detect = 0
        case n60 = 1
        case n90 = 2
        case n120 = 3
        case n240 = 4
    }
    /// Lines per minute.
    public static func marineFaxLpm(_ handle: SDRKIT_HANDLE) -> Result<MarineFaxLpm, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fax_lpm", &v)
        return code == SDRKIT_OK ? .success(MarineFaxLpm(rawValue: v) ?? MarineFaxLpm(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineFaxLpm(_ handle: SDRKIT_HANDLE, _ value: MarineFaxLpm) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fax_lpm", value.rawValue))
    }
    /// The options of the setting "fax_ioc" of the stack "marine".
    public enum MarineFaxIoc: Int32 {
        case tone = 0
        case n576 = 1
        case n288 = 2
    }
    /// Index of cooperation.
    public static func marineFaxIoc(_ handle: SDRKIT_HANDLE) -> Result<MarineFaxIoc, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fax_ioc", &v)
        return code == SDRKIT_OK ? .success(MarineFaxIoc(rawValue: v) ?? MarineFaxIoc(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineFaxIoc(_ handle: SDRKIT_HANDLE, _ value: MarineFaxIoc) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fax_ioc", value.rawValue))
    }
    /// Automatic slant.
    public static func marineFaxAutoSlant(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "fax_auto_slant", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineFaxAutoSlant(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "fax_auto_slant", value))
    }
    /// Slant.
    public static func marineFaxSlantPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fax_slant_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineFaxSlantPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fax_slant_ppm", value))
    }
    /// Most lines.
    public static func marineFaxMaxLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "fax_max_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineFaxMaxLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "fax_max_lines", value))
    }
    /// The options of the setting "fax_rotate" of the stack "marine".
    public enum MarineFaxRotate: Int32 {
        case none = 0
        case n1Turn = 1
        case n2Turns = 2
        case n3Turns = 3
        case n1TurnBack = 4
    }
    /// Rotation.
    public static func marineFaxRotate(_ handle: SDRKIT_HANDLE) -> Result<MarineFaxRotate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fax_rotate", &v)
        return code == SDRKIT_OK ? .success(MarineFaxRotate(rawValue: v) ?? MarineFaxRotate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineFaxRotate(_ handle: SDRKIT_HANDLE, _ value: MarineFaxRotate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fax_rotate", value.rawValue))
    }
    /// Row filter.
    public static func marineRowFilter(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "row_filter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineRowFilter(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "row_filter", value))
    }
    /// Listen.
    public static func marineListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// DSC scan.
    public static func marineDscScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "dsc_scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineDscScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "dsc_scan", value))
    }
    /// DSC channel sets.
    public static func marineDscScanSets(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "dsc_scan_sets", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineDscScanSets(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "dsc_scan_sets", $0) })
    }
    /// DSC dwell.
    public static func marineDscScanDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dsc_scan_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineDscScanDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dsc_scan_dwell_s", value))
    }
    /// Parallel DSC.
    public static func marineDscParallel(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "dsc_parallel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineDscParallel(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "dsc_parallel", value))
    }
    /// Name table.
    public static func marineNames(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "names", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineNames(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "names", $0) })
    }
    /// Time base.
    public static func marineTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
    /// Send ASM as NMEA.
    public static func vdesNmeaOut(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "nmea_out", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdesNmeaOut(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "nmea_out", value))
    }
    /// Time base.
    public static func vdesTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdesTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
    /// Channel plan.
    public static func marinevhfPlan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfPlan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "plan", $0) })
    }
    /// Channel.
    public static func marinevhfChannelHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfChannelHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_hz", value))
    }
    /// Squelch.
    public static func marinevhfSquelch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfSquelch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch", value))
    }
    /// De-emphasis.
    public static func marinevhfDeemphasis(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "deemphasis", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfDeemphasis(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "deemphasis", value))
    }
    /// Priority channel.
    public static func marinevhfPriorityHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "priority_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfPriorityHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "priority_hz", value))
    }
    /// Scan.
    public static func marinevhfScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Scan list.
    public static func marinevhfScanHz(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "scan_hz", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfScanHz(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "scan_hz", $0) })
    }
    /// Dwell.
    public static func marinevhfScanDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfScanDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_dwell_s", value))
    }
    /// Hang.
    public static func marinevhfScanHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfScanHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_hang_s", value))
    }
    /// Carriers.
    public static func marinevhfCarriers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfCarriers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "carriers", value))
    }
    /// Only channels heard.
    public static func marinevhfShowHeard(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "show_heard", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfShowHeard(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "show_heard", value))
    }
    /// Time base.
    public static func marinevhfTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinevhfTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
    /// Listen.
    public static func atisListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtisListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Name table.
    public static func atisNames(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "names", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAtisNames(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "names", $0) })
    }
    /// Channel plan.
    public static func atisPlan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAtisPlan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "plan", $0) })
    }
    /// Time base.
    public static func atisTimeBase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "time_base", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtisTimeBase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "time_base", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum MarineGeneratorSettings {
    /// Vessels.
    public static func aisVessels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_vessels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisVessels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_vessels", value))
    }
    /// Speed of time.
    public static func aisSpeed(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_speed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAisSpeed(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_speed", value))
    }
    /// The options of the setting "gen_service" of the stack "marine".
    public enum MarineService: Int32 {
        case navtex = 0
        case dsc = 1
        case wefax = 2
        case dscVhf = 3
    }
    /// Test signal.
    public static func marineService(_ handle: SDRKIT_HANDLE) -> Result<MarineService, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_service", &v)
        return code == SDRKIT_OK ? .success(MarineService(rawValue: v) ?? MarineService(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineService(_ handle: SDRKIT_HANDLE, _ value: MarineService) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_service", value.rawValue))
    }
    /// NAVTEX phasing.
    public static func marineIdleS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_idle_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineIdleS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_idle_s", value))
    }
    /// Fax IOC.
    public static func marineIoc(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ioc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineIoc(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ioc", value))
    }
    /// Fax lines.
    public static func marineLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Fax lines per minute.
    public static func marineLpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarineLpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lpm", value))
    }
    /// Fax phasing.
    public static func marinePhasingS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_phasing_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMarinePhasingS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_phasing_s", value))
    }
}

// sdrkit: the typed interface of the stack family "avdata", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum AvdataInterface {
    public static let major = 1
    public static let minor = 1
}

/// The result fields of ACARS.
public struct AcarsFields {
    /// Good blocks.
    public var framesOk: Int64
    /// Bad blocks.
    public var framesBad: Int64
    /// Good blocks that needed a repair.
    public var blocksRepaired: Int64
    /// Frame starts (SYN SYN SOH) seen.
    public var framesStarted: Int64
    /// Messages with a position.
    public var positions: Int64
    /// Channels being decoded now.
    public var channelsActive: Int64
    /// Aircraft in the table.
    public var aircraft: Int64
    /// Messages in the list.
    public var messages: Int64
    /// Good blocks, percent of all blocks. No value before a block.
    public var goodPct: Double?
    /// What is watched: the channel list, or the 25 kHz grid.
    public var watching: String

    init(_ c: sdrkit_acars_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        blocksRepaired = c.blocks_repaired
        framesStarted = c.frames_started
        positions = c.positions
        channelsActive = c.channels_active
        aircraft = c.aircraft
        messages = c.messages
        goodPct = c.has_good_pct ? c.good_pct : nil
        watching = sdrkitText(c.watching)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AcarsFields, SDRKitCode> {
        var c = sdrkit_acars_fields()
        let code = sdrkit_acars_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AcarsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the messages. The row key is the place in the log.
public struct AcarsMessageRow {
    /// The row key.
    public var key: String
    /// The wall-clock time. No value for a recording.
    public var time: Double?
    /// The signal time.
    public var signalS: Double
    /// The channel.
    public var frequency: Double
    /// The level of the carrier.
    public var levelDb: Double
    /// The mode character.
    public var mode: String?
    /// The registration.
    public var registration: String?
    /// The row of the aircraft in the aircraft table.
    public var aircraft: String?
    /// The flight id of a downlink.
    public var flight: String?
    /// The label.
    public var label: String?
    /// The meaning of the label.
    public var labelMeaning: String?
    /// The block id of the first block.
    public var blockId: String?
    /// The message sequence number of a downlink.
    public var msn: String?
    /// Aircraft to ground.
    public var downlink: Bool
    /// The technical acknowledgement: `NAK`, or the block id acknowledged.
    public var ack: String?
    /// The printable text.
    public var text: String?
    /// The reading of the fixed format.
    public var reading: String?
    /// The kind of the reading.
    public var readingKind: String?
    /// The category of the reading.
    public var readingCategory: String?
    /// The CPDLC envelope: AT1, CR1, CC1 or DR1.
    public var cpdlcImi: String?
    /// The CPDLC ground facility.
    public var cpdlcGround: String?
    /// The aircraft named in the CPDLC envelope.
    public var cpdlcAircraft: String?
    /// The CPDLC envelope check passed.
    public var cpdlcCrcOk: Bool?
    /// The CPDLC message body decoded completely.
    public var cpdlcParsed: Bool?
    /// The decoded CPDLC message.
    public var cpdlcText: String?
    /// Blocks joined.
    public var blocks: Int64
    /// Bits the repair changed.
    public var bitsFixed: Int64
    /// The position the message gives.
    public var position: SDRKitPosition?
    /// The altitude the message gives.
    public var altFt: Double?

    init(_ c: sdrkit_acars_message_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        frequency = c.frequency
        levelDb = c.level_db
        mode = c.has_mode ? sdrkitText(c.mode) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        aircraft = c.has_aircraft ? sdrkitText(c.aircraft) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        labelMeaning = c.has_label_meaning ? sdrkitText(c.label_meaning) : nil
        blockId = c.has_block_id ? sdrkitText(c.block_id) : nil
        msn = c.has_msn ? sdrkitText(c.msn) : nil
        downlink = c.downlink
        ack = c.has_ack ? sdrkitText(c.ack) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        reading = c.has_reading ? sdrkitText(c.reading) : nil
        readingKind = c.has_reading_kind ? sdrkitText(c.reading_kind) : nil
        readingCategory = c.has_reading_category ? sdrkitText(c.reading_category) : nil
        cpdlcImi = c.has_cpdlc_imi ? sdrkitText(c.cpdlc_imi) : nil
        cpdlcGround = c.has_cpdlc_ground ? sdrkitText(c.cpdlc_ground) : nil
        cpdlcAircraft = c.has_cpdlc_aircraft ? sdrkitText(c.cpdlc_aircraft) : nil
        cpdlcCrcOk = c.has_cpdlc_crc_ok ? c.cpdlc_crc_ok : nil
        cpdlcParsed = c.has_cpdlc_parsed ? c.cpdlc_parsed : nil
        cpdlcText = c.has_cpdlc_text ? sdrkitText(c.cpdlc_text) : nil
        blocks = c.blocks
        bitsFixed = c.bits_fixed
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
    }
}

/// A row of the aircraft. The row key is the registration.
public struct AcarsAircraftRow {
    /// The row key.
    public var key: String
    /// The 24-bit address, six hex digits.
    public var address: String?
    /// The registration.
    public var registration: String?
    /// The State the address is allocated to.
    public var country: String?
    /// The flight id.
    public var flight: String?
    /// The label of the last message.
    public var label: String?
    /// Messages of the aircraft.
    public var messages: Int64
    /// The last position.
    public var position: SDRKitPosition?
    /// The altitude of the last position.
    public var altFt: Double?
    /// The track.
    public var trackDeg: Double?
    /// The ground speed.
    public var gsKt: Double?
    /// The vertical rate.
    public var vsFpm: Double?
    /// When the aircraft was last heard.
    public var lastHeard: Double?
    /// The channel of the last message.
    public var frequency: Double
    /// The level of the last downlink.
    public var levelDb: Double

    init(_ c: sdrkit_acars_aircraft_row) {
        key = sdrkitText(c.key)
        address = c.has_address ? sdrkitText(c.address) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        messages = c.messages
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        trackDeg = c.has_track_deg ? c.track_deg : nil
        gsKt = c.has_gs_kt ? c.gs_kt : nil
        vsFpm = c.has_vs_fpm ? c.vs_fpm : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        frequency = c.frequency
        levelDb = c.level_db
    }
}

/// A row of the channels. The row key is the frequency in hertz.
public struct AcarsChannelRow {
    /// The row key.
    public var key: String
    /// The channel.
    public var frequency: Double
    /// The level now. No value without a carrier.
    public var levelDb: Double?
    /// The signal to noise ratio in 7 kHz.
    public var snrDb: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// A decoder is on the channel.
    public var active: Bool
    /// Good blocks.
    public var good: Int64
    /// Bad blocks.
    public var bad: Int64
    /// The wall-clock time of the last good block, when known.
    public var lastHeard: Double?

    init(_ c: sdrkit_acars_channel_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        levelDb = c.has_level_db ? c.level_db : nil
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        active = c.active
        good = c.good
        bad = c.bad
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of VDL Mode 2.
public struct Vdl2Fields {
    /// Good frames.
    public var framesOk: Int64
    /// Bursts that failed.
    public var burstsBad: Int64
    /// Octets the Reed-Solomon code corrected.
    public var octetsCorrected: Int64
    /// Headers that needed their FEC.
    public var headersFixed: Int64
    /// Frames with a bad FCS.
    public var fcsBad: Int64
    /// Frames with a position.
    public var positions: Int64
    /// How far the radio is off the channel grid. No value before the first report of the channels.
    public var tuningError: Double?
    /// Aircraft in the table.
    public var aircraft: Int64
    /// Ground stations in the table.
    public var groundStations: Int64
    /// Frames in the list.
    public var messages: Int64

    init(_ c: sdrkit_vdl2_fields) {
        framesOk = c.frames_ok
        burstsBad = c.bursts_bad
        octetsCorrected = c.octets_corrected
        headersFixed = c.headers_fixed
        fcsBad = c.fcs_bad
        positions = c.positions
        tuningError = c.has_tuning_error ? c.tuning_error : nil
        aircraft = c.aircraft
        groundStations = c.ground_stations
        messages = c.messages
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Vdl2Fields, SDRKitCode> {
        var c = sdrkit_vdl2_fields()
        let code = sdrkit_vdl2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Vdl2Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the frames. The row key is the place in the log.
public struct Vdl2MessageRow {
    /// The row key.
    public var key: String
    /// The wall-clock time. No value for a recording.
    public var time: Double?
    /// The signal time.
    public var signalS: Double
    /// The channel.
    public var frequency: Double
    /// The level of the burst.
    public var levelDb: Double
    /// The signal to noise ratio of the burst.
    public var snrDb: Double
    /// The source address, six hex digits.
    public var from: String
    /// The type of the source address.
    public var fromType: Int64
    /// The destination address, six hex digits.
    public var to: String
    /// The type of the destination address.
    public var toType: Int64
    /// I, S or U.
    public var frameKind: String?
    /// The name of the frame.
    public var frame: String?
    /// The C/R bit of the source.
    public var response: Bool
    /// The deepest layer read.
    public var `protocol`: String?
    /// One line.
    public var summary: String?
    /// The registration of an ACARS message.
    public var registration: String?
    /// The flight id of an ACARS message.
    public var flight: String?
    /// The label of an ACARS message.
    public var label: String?
    /// The text of an ACARS message.
    public var text: String?
    /// The ACARS check sequence passed.
    public var acarsOk: Bool
    /// Blocks joined into the ACARS message.
    public var blocks: Int64?
    /// The kind of the reading.
    public var readingKind: String?
    /// The category of the reading.
    public var readingCategory: String?
    /// The CPDLC envelope: AT1, CR1, CC1 or DR1.
    public var cpdlcImi: String?
    /// The CPDLC ground facility.
    public var cpdlcGround: String?
    /// The aircraft named in the CPDLC envelope.
    public var cpdlcAircraft: String?
    /// The CPDLC envelope check passed.
    public var cpdlcCrcOk: Bool?
    /// The CPDLC message body decoded completely.
    public var cpdlcParsed: Bool?
    /// The decoded CPDLC message.
    public var cpdlcText: String?
    /// The position the frame gives.
    public var position: SDRKitPosition?
    /// The altitude the frame gives.
    public var altFt: Double?
    /// Every layer, one line each, joined by " | ".
    public var detail: String?

    init(_ c: sdrkit_vdl2_message_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        frequency = c.frequency
        levelDb = c.level_db
        snrDb = c.snr_db
        from = sdrkitText(c.from)
        fromType = c.from_type
        to = sdrkitText(c.to)
        toType = c.to_type
        frameKind = c.has_frame_kind ? sdrkitText(c.frame_kind) : nil
        frame = c.has_frame ? sdrkitText(c.frame) : nil
        response = c.response
        `protocol` = c.has_protocol ? sdrkitText(c.protocol) : nil
        summary = c.has_summary ? sdrkitText(c.summary) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        acarsOk = c.acars_ok
        blocks = c.has_blocks ? c.blocks : nil
        readingKind = c.has_reading_kind ? sdrkitText(c.reading_kind) : nil
        readingCategory = c.has_reading_category ? sdrkitText(c.reading_category) : nil
        cpdlcImi = c.has_cpdlc_imi ? sdrkitText(c.cpdlc_imi) : nil
        cpdlcGround = c.has_cpdlc_ground ? sdrkitText(c.cpdlc_ground) : nil
        cpdlcAircraft = c.has_cpdlc_aircraft ? sdrkitText(c.cpdlc_aircraft) : nil
        cpdlcCrcOk = c.has_cpdlc_crc_ok ? c.cpdlc_crc_ok : nil
        cpdlcParsed = c.has_cpdlc_parsed ? c.cpdlc_parsed : nil
        cpdlcText = c.has_cpdlc_text ? sdrkitText(c.cpdlc_text) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        detail = c.has_detail ? sdrkitText(c.detail) : nil
    }
}

/// A row of the aircraft. The row key is the address.
public struct Vdl2AircraftRow {
    /// The row key.
    public var key: String
    /// The 24-bit address, six hex digits.
    public var address: String?
    /// The registration.
    public var registration: String?
    /// The State the address is allocated to.
    public var country: String?
    /// The flight id.
    public var flight: String?
    /// The label of the last message.
    public var label: String?
    /// Messages of the aircraft.
    public var messages: Int64
    /// The last position.
    public var position: SDRKitPosition?
    /// The altitude of the last position.
    public var altFt: Double?
    /// The track.
    public var trackDeg: Double?
    /// The ground speed.
    public var gsKt: Double?
    /// The vertical rate.
    public var vsFpm: Double?
    /// When the aircraft was last heard.
    public var lastHeard: Double?
    /// Frames to or from the aircraft.
    public var count: Int64
    /// The channel of the last frame.
    public var frequency: Double
    /// The ground station the aircraft last talked to.
    public var groundStation: String?
    /// The position now, by dead reckoning from the last fix.
    public var estimate: SDRKitPosition?
    /// The points of the trail.
    public var trailPoints: Int64

    init(_ c: sdrkit_vdl2_aircraft_row) {
        key = sdrkitText(c.key)
        address = c.has_address ? sdrkitText(c.address) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        messages = c.messages
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        trackDeg = c.has_track_deg ? c.track_deg : nil
        gsKt = c.has_gs_kt ? c.gs_kt : nil
        vsFpm = c.has_vs_fpm ? c.vs_fpm : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        count = c.count
        frequency = c.frequency
        groundStation = c.has_ground_station ? sdrkitText(c.ground_station) : nil
        estimate = c.has_estimate ? SDRKitPosition(c.estimate) : nil
        trailPoints = c.trail_points
    }
}

/// A row of the ground stations. The row key is the address.
public struct Vdl2StationRow {
    /// The row key.
    public var key: String
    /// The address, six hex digits.
    public var address: String
    /// The kind of the station.
    public var kind: String
    /// Frames the station sent.
    public var count: Int64
    /// The channel.
    public var frequency: Double
    /// The level of its last frame.
    public var levelDb: Double
    /// When the station was last heard.
    public var lastHeard: Double?
    /// The airports the station covers.
    public var airports: String?
    /// The position from its GSIF.
    public var position: SDRKitPosition?

    init(_ c: sdrkit_vdl2_station_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        kind = sdrkitText(c.kind)
        count = c.count
        frequency = c.frequency
        levelDb = c.level_db
        lastHeard = c.has_last_heard ? c.last_heard : nil
        airports = c.has_airports ? sdrkitText(c.airports) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
    }
}

/// A row of the channels heard now. The row key is the frequency in hertz.
public struct Vdl2ChannelRow {
    /// The row key.
    public var key: String
    /// The channel.
    public var frequency: Double
    /// The channel is inside the band of the input.
    public var inBand: Bool
    /// A good burst in the last 2 s.
    public var active: Bool
    /// The level of the channel.
    public var levelDb: Double
    /// The signal to noise ratio of the channel.
    public var snrDb: Double
    /// Bursts found.
    public var found: Int64
    /// Good bursts.
    public var good: Int64
    /// Bad bursts.
    public var bad: Int64
    /// Frames.
    public var frames: Int64
    /// The offset of the last burst.
    public var offsetHz: Double

    init(_ c: sdrkit_vdl2_channel_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        inBand = c.in_band
        active = c.active
        levelDb = c.level_db
        snrDb = c.snr_db
        found = c.found
        good = c.good
        bad = c.bad
        frames = c.frames
        offsetHz = c.offset_hz
    }
}

/// The result fields of VDL Mode 4.
public struct Vdl4Fields {
    /// Frames with a good FCS.
    public var frames: Int64
    /// Bursts that failed on the channels now heard.
    public var bad: Int64
    /// Aircraft in the table.
    public var aircraft: Int64
    /// Ground stations in the table.
    public var groundStations: Int64

    init(_ c: sdrkit_vdl4_fields) {
        frames = c.frames
        bad = c.bad
        aircraft = c.aircraft
        groundStations = c.ground_stations
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Vdl4Fields, SDRKitCode> {
        var c = sdrkit_vdl4_fields()
        let code = sdrkit_vdl4_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Vdl4Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the messages. The row key is the place in the log.
public struct Vdl4MessageRow {
    /// The row key.
    public var key: String
    /// The wall-clock time. No value for a recording.
    public var time: Double?
    /// The signal time.
    public var signalS: Double
    /// The channel.
    public var frequency: Double
    /// The level of the burst.
    public var levelDb: Double
    /// The signal to noise ratio of the burst.
    public var snrDb: Double
    /// The address of the sender, six hex digits.
    public var address: String
    /// The kind of the address.
    public var addressKind: String
    /// The name of the message.
    public var message: String?
    /// One line.
    public var summary: String?
    /// The text of the DLS.
    public var dlsText: String?
    /// The position the message gives.
    public var position: SDRKitPosition?
    /// The altitude the message gives.
    public var altFt: Double?

    init(_ c: sdrkit_vdl4_message_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        frequency = c.frequency
        levelDb = c.level_db
        snrDb = c.snr_db
        address = sdrkitText(c.address)
        addressKind = sdrkitText(c.address_kind)
        message = c.has_message ? sdrkitText(c.message) : nil
        summary = c.has_summary ? sdrkitText(c.summary) : nil
        dlsText = c.has_dls_text ? sdrkitText(c.dls_text) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
    }
}

/// A row of the aircraft. The row key is the address.
public struct Vdl4AircraftRow {
    /// The row key.
    public var key: String
    /// The 24-bit address, six hex digits.
    public var address: String?
    /// The registration.
    public var registration: String?
    /// The State the address is allocated to.
    public var country: String?
    /// The flight id.
    public var flight: String?
    /// The label of the last message.
    public var label: String?
    /// Messages of the aircraft.
    public var messages: Int64
    /// The last position.
    public var position: SDRKitPosition?
    /// The altitude of the last position.
    public var altFt: Double?
    /// The track.
    public var trackDeg: Double?
    /// The ground speed.
    public var gsKt: Double?
    /// The vertical rate.
    public var vsFpm: Double?
    /// When the aircraft was last heard.
    public var lastHeard: Double?
    /// Messages heard from it on the link.
    public var count: Int64
    /// The channel of the last burst.
    public var frequency: Double
    /// The points of the trail.
    public var trailPoints: Int64

    init(_ c: sdrkit_vdl4_aircraft_row) {
        key = sdrkitText(c.key)
        address = c.has_address ? sdrkitText(c.address) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        messages = c.messages
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        trackDeg = c.has_track_deg ? c.track_deg : nil
        gsKt = c.has_gs_kt ? c.gs_kt : nil
        vsFpm = c.has_vs_fpm ? c.vs_fpm : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        count = c.count
        frequency = c.frequency
        trailPoints = c.trail_points
    }
}

/// A row of the ground stations. The row key is the address.
public struct Vdl4StationRow {
    /// The row key.
    public var key: String
    /// The address, six hex digits.
    public var address: String
    /// Messages of the station.
    public var count: Int64
    /// The channel.
    public var frequency: Double
    /// When the station was last heard.
    public var lastHeard: Double?
    /// The position the station gave.
    public var position: SDRKitPosition?

    init(_ c: sdrkit_vdl4_station_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        count = c.count
        frequency = c.frequency
        lastHeard = c.has_last_heard ? c.last_heard : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
    }
}

/// A row of the channels heard now. The row key is the frequency in hertz.
public struct Vdl4ChannelRow {
    /// The row key.
    public var key: String
    /// The channel.
    public var frequency: Double
    /// The channel is inside the band of the input.
    public var inBand: Bool
    /// Bursts found.
    public var found: Int64
    /// Good bursts.
    public var good: Int64
    /// Bad bursts.
    public var bad: Int64
    /// The level of the channel.
    public var levelDb: Double
    /// The signal to noise ratio of the channel.
    public var snrDb: Double

    init(_ c: sdrkit_vdl4_channel_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        inBand = c.in_band
        found = c.found
        good = c.good
        bad = c.bad
        levelDb = c.level_db
        snrDb = c.snr_db
    }
}

/// A row of the calls. The row key is the place in the log.
public struct SelcalCallRow {
    /// The row key.
    public var key: String
    /// The wall-clock time. No value for a recording.
    public var time: Double?
    /// The signal time.
    public var signalS: Double
    /// The code, "AM-CP".
    public var code: String
    /// The code as its row key in the codes.
    public var codeRow: String
    /// The four tones, "a b / c d".
    public var tones: String
    /// The dial frequency. No value when it is not known.
    public var dial: Double?
    /// The level of the tones, dB relative to a full-scale sine.
    public var levelDb: Double
    /// The aircraft the code belongs to.
    public var aircraft: String?

    init(_ c: sdrkit_selcal_call_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        code = sdrkitText(c.code)
        codeRow = sdrkitText(c.code_row)
        tones = sdrkitText(c.tones)
        dial = c.has_dial ? c.dial : nil
        levelDb = c.level_db
        aircraft = c.has_aircraft ? sdrkitText(c.aircraft) : nil
    }
}

/// A row of the codes heard. The row key is the code.
public struct SelcalCodeRow {
    /// The row key.
    public var key: String
    /// The code.
    public var code: String
    /// The aircraft the code belongs to.
    public var aircraft: String?
    /// How often the code was heard.
    public var heard: Int64?
    /// When the code was last heard.
    public var lastHeard: Double?
    /// The last dial frequency. No value when it is not known.
    public var dial: Double?

    init(_ c: sdrkit_selcal_code_row) {
        key = sdrkitText(c.key)
        code = sdrkitText(c.code)
        aircraft = c.has_aircraft ? sdrkitText(c.aircraft) : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        dial = c.has_dial ? c.dial : nil
    }
}

/// The result fields of Airband.
public struct AirbandFields {
    /// The rate of the channel receivers.
    public var channelRate: Double
    /// How far the radio is off. No value while it is not known.
    public var tuningError: Double?
    /// The squelch over the noise.
    public var squelchDb: Double
    /// How long a channel stays open after its carrier goes.
    public var hang: Double
    /// The scan is on.
    public var scan: Bool
    /// The channel the scan holds. No value while it holds none.
    public var scanChannel: Double?
    /// Transmissions heard.
    public var transmissions: Int64

    init(_ c: sdrkit_airband_fields) {
        channelRate = c.channel_rate
        tuningError = c.has_tuning_error ? c.tuning_error : nil
        squelchDb = c.squelch_db
        hang = c.hang
        scan = c.scan
        scanChannel = c.has_scan_channel ? c.scan_channel : nil
        transmissions = c.transmissions
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AirbandFields, SDRKitCode> {
        var c = sdrkit_airband_fields()
        let code = sdrkit_airband_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AirbandFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the Airband transmitter. The measurements have no value while the stack does not transmit.
public struct AirbandTxFields {
    /// Whether the stack transmits now.
    public var transmitting: Bool
    /// The transmit frequency: the tuned frequency.
    public var txFreqHz: Double?
    /// The time on air of the transmission.
    public var txTimeS: Double?
    /// The peak of the microphone audio over the last report.
    public var txMicDbfs: Double?
    /// The peak modulation depth over the last report.
    public var txDepth: Double?

    init(_ c: sdrkit_airband_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txTimeS = c.has_tx_time_s ? c.tx_time_s : nil
        txMicDbfs = c.has_tx_mic_dbfs ? c.tx_mic_dbfs : nil
        txDepth = c.has_tx_depth ? c.tx_depth : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AirbandTxFields, SDRKitCode> {
        var c = sdrkit_airband_tx_fields()
        let code = sdrkit_airband_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AirbandTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the channels. The row key is the name of the channel.
public struct AirbandChannelRow {
    /// The row key.
    public var key: String
    /// The name of the channel.
    public var name: String
    /// The label of the channel.
    public var label: String?
    /// The frequency of the channel.
    public var frequency: Double
    /// The channel is inside the band of the input.
    public var inBand: Bool
    /// The carrier over the noise.
    public var cnDb: Double
    /// The level of the channel.
    public var levelDb: Double
    /// The squelch of the channel, when it has its own.
    public var squelchDb: Double?
    /// The squelch is open.
    public var `open`: Bool
    /// The channel plays.
    public var playing: Bool
    /// Two stations on the channel at once.
    public var heterodyne: Bool
    /// The carrier offset.
    public var offsetHz: Double
    /// Transmissions on the channel.
    public var count: Int64

    init(_ c: sdrkit_airband_channel_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        label = c.has_label ? sdrkitText(c.label) : nil
        frequency = c.frequency
        inBand = c.in_band
        cnDb = c.cn_db
        levelDb = c.level_db
        squelchDb = c.has_squelch_db ? c.squelch_db : nil
        `open` = c.open
        playing = c.playing
        heterodyne = c.heterodyne
        offsetHz = c.offset_hz
        count = c.count
    }
}

/// A row of the band watch: a carrier on the air that is not in the list. The row key is its name.
public struct AirbandSeenRow {
    /// The row key.
    public var key: String
    /// The name of the channel.
    public var name: String
    /// The frequency.
    public var frequency: Double
    /// The channel is on the 8.33 kHz grid.
    public var is833: Bool
    /// The level over the floor.
    public var excessDb: Double
    /// How often the carrier was heard.
    public var reports: Int64
    /// The carrier is on now.
    public var active: Bool
    /// The frequency is approximate.
    public var approximate: Bool
    /// The signal time it was last seen.
    public var lastS: Double

    init(_ c: sdrkit_airband_seen_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        frequency = c.frequency
        is833 = c.is833
        excessDb = c.excess_db
        reports = c.reports
        active = c.active
        approximate = c.approximate
        lastS = c.last_s
    }
}

/// A row of the transmissions. The row key is the place in the log.
public struct AirbandActivityRow {
    /// The row key.
    public var key: String
    /// The wall-clock time. No value for a recording.
    public var time: Double?
    /// The signal time of the start.
    public var startS: Double
    /// The name of the channel.
    public var name: String
    /// The label of the channel.
    public var label: String?
    /// The frequency of the channel.
    public var frequency: Double
    /// The length of the transmission.
    public var length: Double
    /// The carrier over the noise.
    public var cnDb: Double
    /// The level.
    public var levelDb: Double
    /// Two stations at once.
    public var heterodyne: Bool

    init(_ c: sdrkit_airband_activity_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        startS = c.start_s
        name = sdrkitText(c.name)
        label = c.has_label ? sdrkitText(c.label) : nil
        frequency = c.frequency
        length = c.length
        cnDb = c.cn_db
        levelDb = c.level_db
        heterodyne = c.heterodyne
    }
}

/// The result fields of HFDL.
public struct HfdlFields {
    /// Good PDUs: squitters and MPDUs, one a burst.
    public var pdus: Int64
    /// Bursts that failed.
    public var burstsBad: Int64
    /// Squitters.
    public var squitters: Int64
    /// LPDUs.
    public var lpdus: Int64
    /// ACARS messages.
    public var acars: Int64
    /// Messages with a position.
    public var positions: Int64
    /// Good bursts at 300 bit/s.
    public var bursts300: Int64
    /// Good bursts at 600 bit/s.
    public var bursts600: Int64
    /// Good bursts at 1200 bit/s.
    public var bursts1200: Int64
    /// Good bursts at 1800 bit/s.
    public var bursts1800: Int64
    /// Bursts lost: the preamble was refused.
    public var lostPreamble: Int64
    /// Bursts lost: cut short.
    public var lostShort: Int64
    /// Squitters lost to a bad FCS.
    public var lostSquitterFcs: Int64
    /// Bursts lost to a bad header FCS.
    public var lostHeaderFcs: Int64
    /// LPDUs lost to a bad FCS.
    public var lostLpduFcs: Int64
    /// LPDUs of an unknown type.
    public var lostLpduType: Int64
    /// What the band hopper does.
    public var hopper: String
    /// Aircraft in the table.
    public var aircraft: Int64

    init(_ c: sdrkit_hfdl_fields) {
        pdus = c.pdus
        burstsBad = c.bursts_bad
        squitters = c.squitters
        lpdus = c.lpdus
        acars = c.acars
        positions = c.positions
        bursts300 = c.bursts_300
        bursts600 = c.bursts_600
        bursts1200 = c.bursts_1200
        bursts1800 = c.bursts_1800
        lostPreamble = c.lost_preamble
        lostShort = c.lost_short
        lostSquitterFcs = c.lost_squitter_fcs
        lostHeaderFcs = c.lost_header_fcs
        lostLpduFcs = c.lost_lpdu_fcs
        lostLpduType = c.lost_lpdu_type
        hopper = sdrkitText(c.hopper)
        aircraft = c.aircraft
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfdlFields, SDRKitCode> {
        var c = sdrkit_hfdl_fields()
        let code = sdrkit_hfdl_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfdlFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the messages. The row key is the place in the log.
public struct HfdlMessageRow {
    /// The row key.
    public var key: String
    /// The wall-clock time. No value for a recording.
    public var time: Double?
    /// The signal time.
    public var signalS: Double
    /// The dial frequency of the channel.
    public var dial: Double
    /// The level of the burst.
    public var levelDb: Double
    /// The signal to noise ratio of the burst.
    public var snrDb: Double
    /// The offset of the burst.
    public var offsetHz: Double
    /// The bit rate of the burst.
    public var bps: Int64
    /// The ground station id.
    public var gsId: Int64
    /// The ground station as its row key.
    public var groundStation: String
    /// The aircraft id the ground station gave.
    public var acId: Int64?
    /// The kind of the PDU.
    public var kind: String?
    /// The kind of the HFNPDU.
    public var hfnpdu: String?
    /// Ground to aircraft.
    public var uplink: Bool
    /// One line.
    public var summary: String?
    /// The registration of an ACARS message.
    public var registration: String?
    /// The flight id.
    public var flight: String?
    /// The label of an ACARS message.
    public var label: String?
    /// The block id of an ACARS message.
    public var blockId: String?
    /// The message sequence number of an ACARS message.
    public var msn: String?
    /// The text of an ACARS message.
    public var text: String?
    /// The ACARS check sequence passed.
    public var acarsOk: Bool
    /// The kind of the reading.
    public var readingKind: String?
    /// The category of the reading.
    public var readingCategory: String?
    /// The CPDLC envelope: AT1, CR1, CC1 or DR1.
    public var cpdlcImi: String?
    /// The CPDLC ground facility.
    public var cpdlcGround: String?
    /// The aircraft named in the CPDLC envelope.
    public var cpdlcAircraft: String?
    /// The CPDLC envelope check passed.
    public var cpdlcCrcOk: Bool?
    /// The CPDLC message body decoded completely.
    public var cpdlcParsed: Bool?
    /// The decoded CPDLC message.
    public var cpdlcText: String?
    /// The position the message gives.
    public var position: SDRKitPosition?
    /// Every decoded layer, joined by " | ".
    public var detail: String?

    init(_ c: sdrkit_hfdl_message_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        dial = c.dial
        levelDb = c.level_db
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        bps = c.bps
        gsId = c.gs_id
        groundStation = sdrkitText(c.ground_station)
        acId = c.has_ac_id ? c.ac_id : nil
        kind = c.has_kind ? sdrkitText(c.kind) : nil
        hfnpdu = c.has_hfnpdu ? sdrkitText(c.hfnpdu) : nil
        uplink = c.uplink
        summary = c.has_summary ? sdrkitText(c.summary) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        blockId = c.has_block_id ? sdrkitText(c.block_id) : nil
        msn = c.has_msn ? sdrkitText(c.msn) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        acarsOk = c.acars_ok
        readingKind = c.has_reading_kind ? sdrkitText(c.reading_kind) : nil
        readingCategory = c.has_reading_category ? sdrkitText(c.reading_category) : nil
        cpdlcImi = c.has_cpdlc_imi ? sdrkitText(c.cpdlc_imi) : nil
        cpdlcGround = c.has_cpdlc_ground ? sdrkitText(c.cpdlc_ground) : nil
        cpdlcAircraft = c.has_cpdlc_aircraft ? sdrkitText(c.cpdlc_aircraft) : nil
        cpdlcCrcOk = c.has_cpdlc_crc_ok ? c.cpdlc_crc_ok : nil
        cpdlcParsed = c.has_cpdlc_parsed ? c.cpdlc_parsed : nil
        cpdlcText = c.has_cpdlc_text ? sdrkitText(c.cpdlc_text) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        detail = c.has_detail ? sdrkitText(c.detail) : nil
    }
}

/// A row of the aircraft. The row key is the address, or the ground station and the aircraft id while the address is not known.
public struct HfdlAircraftRow {
    /// The row key.
    public var key: String
    /// The row key.
    public var keyField: String
    /// The 24-bit address, six hex digits.
    public var address: String?
    /// The registration.
    public var registration: String?
    /// The flight id.
    public var flight: String?
    /// The ground station id.
    public var gsId: Int64
    /// The ground station as its row key.
    public var groundStation: String
    /// Messages of the aircraft.
    public var count: Int64
    /// The dial frequency of the last message.
    public var dial: Double
    /// When the aircraft was last heard.
    public var lastHeard: Double?
    /// The aircraft is logged on.
    public var loggedOn: Bool
    /// The last position.
    public var position: SDRKitPosition?
    /// The altitude of the last position.
    public var altFt: Double?

    init(_ c: sdrkit_hfdl_aircraft_row) {
        key = sdrkitText(c.key)
        keyField = sdrkitText(c.key_)
        address = c.has_address ? sdrkitText(c.address) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        flight = c.has_flight ? sdrkitText(c.flight) : nil
        gsId = c.gs_id
        groundStation = sdrkitText(c.ground_station)
        count = c.count
        dial = c.dial
        lastHeard = c.has_last_heard ? c.last_heard : nil
        loggedOn = c.logged_on
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
    }
}

/// A row of the ground stations. The row key is the station id.
public struct HfdlStationRow {
    /// The row key.
    public var key: String
    /// The station id.
    public var gsId: Int64
    /// The name of the station.
    public var name: String
    /// Heard directly: a squitter or an uplink.
    public var heardDirect: Bool
    /// The station is synchronised to UTC.
    public var utcSync: Bool
    /// Squitters.
    public var squitters: Int64
    /// Messages that name the station.
    public var count: Int64
    /// The dial frequency it was heard on.
    public var dial: Double
    /// When the station was last heard.
    public var lastHeard: Double?
    /// The frame index of the last squitter.
    public var frameIndex: Int64?
    /// The frequencies in use by the last squitter, kHz, separated by spaces.
    public var activeKhz: String?

    init(_ c: sdrkit_hfdl_station_row) {
        key = sdrkitText(c.key)
        gsId = c.gs_id
        name = sdrkitText(c.name)
        heardDirect = c.heard_direct
        utcSync = c.utc_sync
        squitters = c.squitters
        count = c.count
        dial = c.dial
        lastHeard = c.has_last_heard ? c.last_heard : nil
        frameIndex = c.has_frame_index ? c.frame_index : nil
        activeKhz = c.has_active_khz ? sdrkitText(c.active_khz) : nil
    }
}

/// A row of the channels. The row key is the dial frequency in hertz.
public struct HfdlChannelRow {
    /// The row key.
    public var key: String
    /// The dial frequency.
    public var dial: Double
    /// The name of the ground station of the channel.
    public var name: String?
    /// The ground station as its row key.
    public var groundStation: String?
    /// The channel is inside the band of the input.
    public var inBand: Bool
    /// Preambles found.
    public var found: Int64
    /// Good bursts.
    public var good: Int64
    /// Bad bursts.
    public var bad: Int64
    /// The signal to noise ratio of the channel.
    public var snrDb: Double
    /// The offset of the last burst.
    public var offsetHz: Double
    /// The bit rate of the last burst.
    public var bps: Int64

    init(_ c: sdrkit_hfdl_channel_row) {
        key = sdrkitText(c.key)
        dial = c.dial
        name = c.has_name ? sdrkitText(c.name) : nil
        groundStation = c.has_ground_station ? sdrkitText(c.ground_station) : nil
        inBand = c.in_band
        found = c.found
        good = c.good
        bad = c.bad
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        bps = c.bps
    }
}

/// The result fields of VDL Mode 3.
public struct Vdl3Fields {
    /// The state: 0 listening, 1 bursts seen, 2 decoding.
    public var rxState: Int64
    /// Training sequences found.
    public var trainings: Int64
    /// Good bursts.
    public var framesOk: Int64
    /// Bursts that failed.
    public var framesBad: Int64
    /// M uplink bursts.
    public var mUplink: Int64
    /// M uplink bursts of the 3T configuration.
    public var mUplink3t: Int64
    /// Handoff check bursts.
    public var handoff: Int64
    /// Poll response bursts.
    public var pollResponse: Int64
    /// M downlink bursts.
    public var mDownlink: Int64
    /// Net entry request bursts.
    public var netEntry: Int64
    /// V/D data bursts.
    public var vdData: Int64
    /// V/D voice bursts.
    public var vdVoice: Int64
    /// The level of the last burst. No value before a burst.
    public var levelDbfs: Double?
    /// The signal to noise ratio of the last burst. No value before a burst.
    public var snrDb: Double?
    /// The carrier offset of the last burst. No value before a burst.
    public var offsetHz: Double?
    /// The summary of the newest burst.
    public var text: String?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_vdl3_fields) {
        rxState = c.rx_state
        trainings = c.trainings
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        mUplink = c.m_uplink
        mUplink3t = c.m_uplink_3t
        handoff = c.handoff
        pollResponse = c.poll_response
        mDownlink = c.m_downlink
        netEntry = c.net_entry
        vdData = c.vd_data
        vdVoice = c.vd_voice
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Vdl3Fields, SDRKitCode> {
        var c = sdrkit_vdl3_fields()
        let code = sdrkit_vdl3_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Vdl3Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct AvdataDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_avdata_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AvdataDecoderFields, SDRKitCode> {
        var c = sdrkit_avdata_decoder_fields()
        let code = sdrkit_avdata_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvdataDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the bursts. The row key is the place in the log.
public struct Vdl3BurstRow {
    public enum Kind: Int32 {
        case mUplink = 0
        case mUplink3t = 1
        case handoff = 2
        case pollResponse = 3
        case mDownlink = 4
        case netEntry = 5
        case vdData = 6
        case vdVoice = 7
    }
    public enum Training: Int32 {
        case s1 = 0
        case s1Star = 1
        case s2 = 2
        case s2Star = 3
    }
    /// The row key.
    public var key: String
    /// The signal time of the end of the training.
    public var timeS: Double
    /// The burst kind.
    public var kind: Kind?
    /// The training sequence.
    public var training: Training?
    /// Symbols after the training.
    public var symbols: Int64
    /// Every Golay word decoded.
    public var golayOk: Bool
    /// Bits corrected by Golay.
    public var golayCorrected: Int64
    /// The information words.
    public var info: String
    /// The Reed-Solomon word was good.
    public var rsOk: Bool
    /// Octets corrected by Reed-Solomon.
    public var rsCorrected: Int64
    /// The data octets, hex.
    public var dataHex: String
    /// The printable text of the data.
    public var dataText: String
    /// The level of the burst.
    public var levelDbfs: Double
    /// The signal to noise ratio of the burst.
    public var snrDb: Double
    /// The carrier offset of the burst.
    public var cfoHz: Double
    /// The error vector magnitude of the burst.
    public var evmPct: Double
    /// The summary of the burst.
    public var summary: String

    init(_ c: sdrkit_vdl3_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        training = Training(rawValue: c.training)
        symbols = c.symbols
        golayOk = c.golay_ok
        golayCorrected = c.golay_corrected
        info = sdrkitText(c.info)
        rsOk = c.rs_ok
        rsCorrected = c.rs_corrected
        dataHex = sdrkitText(c.data_hex)
        dataText = sdrkitText(c.data_text)
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        cfoHz = c.cfo_hz
        evmPct = c.evm_pct
        summary = sdrkitText(c.summary)
    }
}

/// The result fields of GBAS VDB.
public struct GbasVdbFields {
    /// Slots of 8 that held a burst in the last frame.
    public var slotsBusy: Int64
    /// The line of the symbol rate was seen.
    public var symbolRateSeen: Bool
    /// Bursts counted.
    public var bursts: Int64
    /// The length of the last burst. No value before a burst.
    public var lengthMs: Double?
    /// The signal to noise ratio of the last burst. No value before a burst.
    public var snrDb: Double?
    /// The summary line.
    public var text: String
    /// What the stack decodes.
    public var content: String
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_gbas_vdb_fields) {
        slotsBusy = c.slots_busy
        symbolRateSeen = c.symbol_rate_seen
        bursts = c.bursts
        lengthMs = c.has_length_ms ? c.length_ms : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        text = sdrkitText(c.text)
        content = sdrkitText(c.content)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GbasVdbFields, SDRKitCode> {
        var c = sdrkit_gbas_vdb_fields()
        let code = sdrkit_gbas_vdb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GbasVdbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the burst log: every 16th burst. The row key is the place in the log.
public struct GbasVdbBurstRow {
    /// The row key.
    public var key: String
    /// The signal time of the burst.
    public var timeS: Double
    /// The length of the burst.
    public var lengthMs: Double
    /// Slots busy then.
    public var slotsBusy: Int64
    /// The signal to noise ratio of the burst.
    public var snrDb: Double

    init(_ c: sdrkit_gbas_vdb_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        lengthMs = c.length_ms
        slotsBusy = c.slots_busy
        snrDb = c.snr_db
    }
}

/// The result fields of an OFDM structure stack.
public struct AvdataOfdmShapeFields {
    /// The OFDM of the numerology is there.
    public var detected: Bool
    /// The symbol duration with its prefix. No value while the numerology of the settings is not valid.
    public var symbolUs: Double?
    /// The prefix duration. No value while the numerology of the settings is not valid.
    public var prefixUs: Double?
    /// The sample of the symbol period where the prefix correlation peaks, while detected.
    public var timing: Int64?
    /// The prefix correlation, while detected.
    public var correlation: Double?
    /// The carrier offset, while detected.
    public var offsetHz: Double?
    /// The frame structure in words. No value while it is not known.
    public var frame: String?
    /// What the stack decodes.
    public var content: String
    /// The summary line.
    public var text: String
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_avdata_ofdm_shape_fields) {
        detected = c.detected
        symbolUs = c.has_symbol_us ? c.symbol_us : nil
        prefixUs = c.has_prefix_us ? c.prefix_us : nil
        timing = c.has_timing ? c.timing : nil
        correlation = c.has_correlation ? c.correlation : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        frame = c.has_frame ? sdrkitText(c.frame) : nil
        content = sdrkitText(c.content)
        text = sdrkitText(c.text)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AvdataOfdmShapeFields, SDRKitCode> {
        var c = sdrkit_avdata_ofdm_shape_fields()
        let code = sdrkit_avdata_ofdm_shape_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvdataOfdmShapeFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the detection starts. The row key is the place in the log.
public struct AvdataOfdmShapeDetectionRow {
    /// The row key.
    public var key: String
    /// The signal time of the start.
    public var timeS: Double
    /// The symbol duration with its prefix.
    public var symbolUs: Double
    /// The prefix duration.
    public var prefixUs: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// The prefix correlation.
    public var correlation: Double
    /// The frame structure in words. No value while it was not known.
    public var frame: String?

    init(_ c: sdrkit_avdata_ofdm_shape_detection_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        symbolUs = c.symbol_us
        prefixUs = c.prefix_us
        offsetHz = c.offset_hz
        correlation = c.correlation
        frame = c.has_frame ? sdrkitText(c.frame) : nil
    }
}

/// An ACARS message. The source is the registration of the aircraft, as the link names it. The link does not name the ground station, so the target has no value.
public struct AcarsMessageEvent {
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
    /// The registration.
    public var source: String
    /// The receiver. No value: the link does not name it.
    public var target: String?
    /// The text.
    public var text: String
    /// The signal to noise ratio. No value: the stack does not measure it per message.
    public var snrDb: Double?
    /// What the event is about: an aircraft.
    public var family: Family?
    /// The flight id of a downlink.
    public var flight: String
    /// The label.
    public var label: String
    /// Aircraft to ground.
    public var downlink: Bool
    /// The message sequence number.
    public var msn: String
    /// The position the message gives.
    public var position: SDRKitPosition?
    /// The altitude the message gives.
    public var altFt: Double?

    init(_ c: sdrkit_acars_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = c.has_target ? sdrkitText(c.target) : nil
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        flight = sdrkitText(c.flight)
        label = sdrkitText(c.label)
        downlink = c.downlink
        msn = sdrkitText(c.msn)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AcarsMessageEvent, SDRKitCode> {
        var c = sdrkit_acars_message_event()
        let code = sdrkit_acars_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AcarsMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A VDL2 frame. The source and the target are the addresses of the sender and the receiver, six hex digits.
public struct Vdl2MessageEvent {
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
    /// The address of the sender.
    public var source: String
    /// The address of the receiver.
    public var target: String
    /// The summary of the frame.
    public var text: String
    /// The signal to noise ratio of the burst.
    public var snrDb: Double?
    /// What the event is about: an aircraft.
    public var family: Family?
    /// An aircraft sent the frame.
    public var fromAir: Bool
    /// The deepest layer read.
    public var `protocol`: String
    /// The name of the frame.
    public var frame: String

    init(_ c: sdrkit_vdl2_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        fromAir = c.from_air
        `protocol` = sdrkitText(c.protocol)
        frame = sdrkitText(c.frame)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Vdl2MessageEvent, SDRKitCode> {
        var c = sdrkit_vdl2_message_event()
        let code = sdrkit_vdl2_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Vdl2MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A VDL4 message. The source is the address of the sender, six hex digits.
public struct Vdl4MessageEvent {
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
    /// The address of the sender.
    public var source: String
    /// The summary of the message.
    public var text: String
    /// The signal to noise ratio of the burst.
    public var snrDb: Double?
    /// What the event is about: an aircraft.
    public var family: Family?
    /// The name of the message.
    public var message: String
    /// The position the message gives.
    public var position: SDRKitPosition?

    init(_ c: sdrkit_vdl4_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        message = sdrkitText(c.message)
        position = c.has_position ? SDRKitPosition(c.position) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Vdl4MessageEvent, SDRKitCode> {
        var c = sdrkit_vdl4_message_event()
        let code = sdrkit_vdl4_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Vdl4MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A transmission, when it ends. Analogue voice carries no identity: the source and the target have no value. The channel is the name of the channel, the text its label.
public struct AirbandActivityEvent {
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
    /// The sender. No value: voice carries no identity.
    public var source: String?
    /// The receiver. No value: voice carries no identity.
    public var target: String?
    /// The name of the channel.
    public var channel: String
    /// The label of the channel.
    public var text: String?
    /// The carrier over the noise.
    public var snrDb: Double
    /// What the event is about: voice.
    public var family: Family?
    /// The length of the transmission.
    public var length: Double
    /// The carrier over the noise.
    public var cnDb: Double

    init(_ c: sdrkit_airband_activity_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = sdrkitText(c.channel)
        text = c.has_text ? sdrkitText(c.text) : nil
        snrDb = c.snr_db
        family = Family(rawValue: c.family)
        length = c.length
        cnDb = c.cn_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AirbandActivityEvent, SDRKitCode> {
        var c = sdrkit_airband_activity_event()
        let code = sdrkit_airband_activity_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AirbandActivityEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An HFDL message. The source is the sender (the address or registration of the aircraft, or the name of the ground station), the target the receiver.
public struct HfdlMessageEvent {
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
    /// The sender.
    public var source: String
    /// The receiver. No value for a squitter.
    public var target: String?
    /// The summary of the message.
    public var text: String
    /// The signal to noise ratio of the burst.
    public var snrDb: Double?
    /// What the event is about: an aircraft.
    public var family: Family?
    /// The position the message gives.
    public var position: SDRKitPosition?
    /// The kind of the PDU.
    public var kind: String
    /// Ground to aircraft.
    public var uplink: Bool
    /// The ground station id.
    public var gsId: Int64

    init(_ c: sdrkit_hfdl_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = c.has_target ? sdrkitText(c.target) : nil
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        kind = sdrkitText(c.kind)
        uplink = c.uplink
        gsId = c.gs_id
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdlMessageEvent, SDRKitCode> {
        var c = sdrkit_hfdl_message_event()
        let code = sdrkit_hfdl_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdlMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum AvdataTables {
    /// The changed rows of the table "messages" of the stack "acars", newest first, at most `capacity` rows.
    public static func acarsMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AcarsMessageRow], SDRKitCode> {
        var rows = [sdrkit_acars_message_row](repeating: sdrkit_acars_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_acars_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AcarsMessageRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "acars", newest first, at most `capacity` rows.
    public static func acarsAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AcarsAircraftRow], SDRKitCode> {
        var rows = [sdrkit_acars_aircraft_row](repeating: sdrkit_acars_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_acars_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AcarsAircraftRow.init))
    }
    /// The changed rows of the table "channels" of the stack "acars", newest first, at most `capacity` rows.
    public static func acarsChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[AcarsChannelRow], SDRKitCode> {
        var rows = [sdrkit_acars_channel_row](repeating: sdrkit_acars_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_acars_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AcarsChannelRow.init))
    }
    /// The changed rows of the table "messages" of the stack "vdl2", newest first, at most `capacity` rows.
    public static func vdl2Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[Vdl2MessageRow], SDRKitCode> {
        var rows = [sdrkit_vdl2_message_row](repeating: sdrkit_vdl2_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl2_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl2MessageRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "vdl2", newest first, at most `capacity` rows.
    public static func vdl2Aircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Vdl2AircraftRow], SDRKitCode> {
        var rows = [sdrkit_vdl2_aircraft_row](repeating: sdrkit_vdl2_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl2_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl2AircraftRow.init))
    }
    /// The changed rows of the table "ground_stations" of the stack "vdl2", newest first, at most `capacity` rows.
    public static func vdl2GroundStations(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Vdl2StationRow], SDRKitCode> {
        var rows = [sdrkit_vdl2_station_row](repeating: sdrkit_vdl2_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl2_ground_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl2StationRow.init))
    }
    /// The changed rows of the table "channels" of the stack "vdl2", newest first, at most `capacity` rows.
    public static func vdl2Channels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Vdl2ChannelRow], SDRKitCode> {
        var rows = [sdrkit_vdl2_channel_row](repeating: sdrkit_vdl2_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl2_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl2ChannelRow.init))
    }
    /// The changed rows of the table "messages" of the stack "vdl4", newest first, at most `capacity` rows.
    public static func vdl4Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[Vdl4MessageRow], SDRKitCode> {
        var rows = [sdrkit_vdl4_message_row](repeating: sdrkit_vdl4_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl4_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl4MessageRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "vdl4", newest first, at most `capacity` rows.
    public static func vdl4Aircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Vdl4AircraftRow], SDRKitCode> {
        var rows = [sdrkit_vdl4_aircraft_row](repeating: sdrkit_vdl4_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl4_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl4AircraftRow.init))
    }
    /// The changed rows of the table "ground_stations" of the stack "vdl4", newest first, at most `capacity` rows.
    public static func vdl4GroundStations(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Vdl4StationRow], SDRKitCode> {
        var rows = [sdrkit_vdl4_station_row](repeating: sdrkit_vdl4_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl4_ground_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl4StationRow.init))
    }
    /// The changed rows of the table "channels" of the stack "vdl4", newest first, at most `capacity` rows.
    public static func vdl4Channels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Vdl4ChannelRow], SDRKitCode> {
        var rows = [sdrkit_vdl4_channel_row](repeating: sdrkit_vdl4_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl4_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl4ChannelRow.init))
    }
    /// The changed rows of the table "calls" of the stack "selcal", newest first, at most `capacity` rows.
    public static func selcalCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[SelcalCallRow], SDRKitCode> {
        var rows = [sdrkit_selcal_call_row](repeating: sdrkit_selcal_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_selcal_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SelcalCallRow.init))
    }
    /// The changed rows of the table "codes" of the stack "selcal", newest first, at most `capacity` rows.
    public static func selcalCodes(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[SelcalCodeRow], SDRKitCode> {
        var rows = [sdrkit_selcal_code_row](repeating: sdrkit_selcal_code_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_selcal_codes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SelcalCodeRow.init))
    }
    /// The changed rows of the table "channels" of the stack "airband", newest first, at most `capacity` rows.
    public static func airbandChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[AirbandChannelRow], SDRKitCode> {
        var rows = [sdrkit_airband_channel_row](repeating: sdrkit_airband_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_airband_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AirbandChannelRow.init))
    }
    /// The changed rows of the table "seen" of the stack "airband", newest first, at most `capacity` rows.
    public static func airbandSeen(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AirbandSeenRow], SDRKitCode> {
        var rows = [sdrkit_airband_seen_row](repeating: sdrkit_airband_seen_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_airband_seen_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AirbandSeenRow.init))
    }
    /// The changed rows of the table "activity" of the stack "airband", newest first, at most `capacity` rows.
    public static func airbandActivity(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[AirbandActivityRow], SDRKitCode> {
        var rows = [sdrkit_airband_activity_row](repeating: sdrkit_airband_activity_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_airband_activity_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AirbandActivityRow.init))
    }
    /// The changed rows of the table "messages" of the stack "hfdl", newest first, at most `capacity` rows.
    public static func hfdlMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[HfdlMessageRow], SDRKitCode> {
        var rows = [sdrkit_hfdl_message_row](repeating: sdrkit_hfdl_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfdl_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdlMessageRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "hfdl", newest first, at most `capacity` rows.
    public static func hfdlAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[HfdlAircraftRow], SDRKitCode> {
        var rows = [sdrkit_hfdl_aircraft_row](repeating: sdrkit_hfdl_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfdl_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdlAircraftRow.init))
    }
    /// The changed rows of the table "ground_stations" of the stack "hfdl", newest first, at most `capacity` rows.
    public static func hfdlGroundStations(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[HfdlStationRow], SDRKitCode> {
        var rows = [sdrkit_hfdl_station_row](repeating: sdrkit_hfdl_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfdl_ground_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdlStationRow.init))
    }
    /// The changed rows of the table "channels" of the stack "hfdl", newest first, at most `capacity` rows.
    public static func hfdlChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[HfdlChannelRow], SDRKitCode> {
        var rows = [sdrkit_hfdl_channel_row](repeating: sdrkit_hfdl_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfdl_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdlChannelRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "vdl3", newest first, at most `capacity` rows.
    public static func vdl3Bursts(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[Vdl3BurstRow], SDRKitCode> {
        var rows = [sdrkit_vdl3_burst_row](repeating: sdrkit_vdl3_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vdl3_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Vdl3BurstRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "gbas_vdb", newest first, at most `capacity` rows.
    public static func gbasVdbBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[GbasVdbBurstRow], SDRKitCode> {
        var rows = [sdrkit_gbas_vdb_burst_row](repeating: sdrkit_gbas_vdb_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gbas_vdb_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GbasVdbBurstRow.init))
    }
    /// The changed rows of the table "detections" of the stack "ldacs", newest first, at most `capacity` rows.
    public static func ldacsDetections(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AvdataOfdmShapeDetectionRow], SDRKitCode> {
        var rows = [sdrkit_avdata_ofdm_shape_detection_row](repeating: sdrkit_avdata_ofdm_shape_detection_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ldacs_detections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AvdataOfdmShapeDetectionRow.init))
    }
    /// The changed rows of the table "detections" of the stack "aeromacs", newest first, at most `capacity` rows.
    public static func aeromacsDetections(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AvdataOfdmShapeDetectionRow], SDRKitCode> {
        var rows = [sdrkit_avdata_ofdm_shape_detection_row](repeating: sdrkit_avdata_ofdm_shape_detection_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aeromacs_detections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AvdataOfdmShapeDetectionRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum AvdataCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func acarsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_acars_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func vdl2Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vdl2_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func vdl2Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_vdl2_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_vdl2_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_vdl2_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func vdl4Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vdl4_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func vdl4Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_vdl4_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_vdl4_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_vdl4_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func selcalClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_selcal_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func selcalRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_selcal_code_args()
        args.size = UInt32(MemoryLayout<sdrkit_selcal_code_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_selcal_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func airbandClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_airband_clear(handle)
        return sdrkitResult(code)
    }
    /// Push to talk. With on true, sends the speech of the microphone input as AM with full carrier on the tuned frequency, in the audio band of the channel spacing, until ptt with on false, halt or the time-out. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
    public static func airbandPtt(_ handle: SDRKIT_HANDLE, on: Bool) -> Result<Void, SDRKitCode> {
        var args = sdrkit_airband_ptt_args()
        args.size = UInt32(MemoryLayout<sdrkit_airband_ptt_args>.size)
        let code: Int32 = { () -> Int32 in
        args.on = on
        return sdrkit_airband_ptt(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Halt. Ends the transmission at once, with the fall of the carrier.
    public static func airbandHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_airband_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func hfdlClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfdl_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func hfdlRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfdl_aircraft_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfdl_aircraft_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_hfdl_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func vdl3Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vdl3_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func gbasVdbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gbas_vdb_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ldacsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ldacs_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func aeromacsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_aeromacs_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum AvdataSettings {
    /// Channels.
    public static func acarsChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAcarsChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Threshold.
    public static func acarsThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAcarsThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Join blocks.
    public static func acarsJoinBlocks(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "join_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAcarsJoinBlocks(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "join_blocks", value))
    }
    /// Channels.
    public static func vdl2Channels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl2Channels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Track tuning.
    public static func vdl2TrackTuning(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "track_tuning", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl2TrackTuning(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "track_tuning", value))
    }
    /// Tuning error guess.
    public static func vdl2TuneErrorHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tune_error_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl2TuneErrorHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tune_error_hz", value))
    }
    /// Channels.
    public static func vdl4Channels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl4Channels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// The options of the setting "demod" of the stack "selcal".
    public enum SelcalDemod: Int32 {
        case auto = 0
        case fm = 1
        case usb = 2
        case lsb = 3
        case am = 4
    }
    /// Demodulator.
    public static func selcalDemod(_ handle: SDRKIT_HANDLE) -> Result<SelcalDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(SelcalDemod(rawValue: v) ?? SelcalDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSelcalDemod(_ handle: SDRKIT_HANDLE, _ value: SelcalDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func selcalListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSelcalListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Audio input.
    public static func selcalAudioInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSelcalAudioInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_input", value))
    }
    /// Aircraft table.
    public static func selcalAircraftTable(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aircraft_table", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSelcalAircraftTable(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aircraft_table", $0) })
    }
    /// Channels.
    public static func airbandChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Squelch.
    public static func airbandSquelchDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandSquelchDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_db", value))
    }
    /// Hang.
    public static func airbandHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Scan.
    public static func airbandScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Hold.
    public static func airbandScanHold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandScanHold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan_hold", value))
    }
    /// Noise remover.
    public static func airbandNoiseRemover(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "noise_remover", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandNoiseRemover(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "noise_remover", value))
    }
    /// Strength.
    public static func airbandNoiseRemoverStrength(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "noise_remover_strength", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandNoiseRemoverStrength(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "noise_remover_strength", value))
    }
    /// Only active.
    public static func airbandOnlyActive(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "only_active", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandOnlyActive(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "only_active", value))
    }
    /// Listen.
    public static func airbandListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "tx_spacing" of the stack "airband".
    public enum AirbandTxSpacing: Int32 {
        case n833 = 0
        case n25Khz = 1
    }
    /// Channel spacing.
    public static func airbandTxSpacing(_ handle: SDRKIT_HANDLE) -> Result<AirbandTxSpacing, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_spacing", &v)
        return code == SDRKIT_OK ? .success(AirbandTxSpacing(rawValue: v) ?? AirbandTxSpacing(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandTxSpacing(_ handle: SDRKIT_HANDLE, _ value: AirbandTxSpacing) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_spacing", value.rawValue))
    }
    /// Modulation depth.
    public static func airbandTxAmDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_am_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandTxAmDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_am_depth", value))
    }
    /// Microphone gain.
    public static func airbandTxMicGain(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_mic_gain", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandTxMicGain(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_mic_gain", value))
    }
    /// Time-out.
    public static func airbandTxTimeout(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_timeout", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAirbandTxTimeout(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_timeout", value))
    }
    /// Channels.
    public static func hfdlChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfdlChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Whole span.
    public static func hfdlWholeSpan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "whole_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfdlWholeSpan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "whole_span", value))
    }
    /// Shared channeliser.
    public static func hfdlSharedChanneliser(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "shared_channeliser", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfdlSharedChanneliser(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "shared_channeliser", value))
    }
    /// Channel filter.
    public static func hfdlChannelFilterHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_filter_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfdlChannelFilterHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_filter_hz", value))
    }
    /// Follow the best band.
    public static func hfdlFollowBestBand(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_best_band", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfdlFollowBestBand(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_best_band", value))
    }
    /// Training threshold.
    public static func vdl3SyncThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "sync_threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl3SyncThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "sync_threshold", value))
    }
    /// Largest phase error.
    public static func vdl3EvmMax(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "evm_max", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl3EvmMax(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "evm_max", value))
    }
    /// The options of the setting "rs_bit_order" of the stack "vdl3".
    public enum Vdl3RsBitOrder: Int32 {
        case lsbFirst = 0
        case msbFirst = 1
    }
    /// Reed-Solomon octet bit order.
    public static func vdl3RsBitOrder(_ handle: SDRKIT_HANDLE) -> Result<Vdl3RsBitOrder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rs_bit_order", &v)
        return code == SDRKIT_OK ? .success(Vdl3RsBitOrder(rawValue: v) ?? Vdl3RsBitOrder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl3RsBitOrder(_ handle: SDRKIT_HANDLE, _ value: Vdl3RsBitOrder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rs_bit_order", value.rawValue))
    }
    /// FFT length.
    public static func ldacsFftLen(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "fft_len", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLdacsFftLen(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "fft_len", value))
    }
    /// Cyclic prefix.
    public static func ldacsPrefix(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "prefix", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLdacsPrefix(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "prefix", value))
    }
    /// Frame period.
    public static func ldacsFrameMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "frame_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLdacsFrameMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "frame_ms", value))
    }
    /// FFT length.
    public static func aeromacsFftLen(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "fft_len", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeromacsFftLen(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "fft_len", value))
    }
    /// Cyclic prefix.
    public static func aeromacsPrefix(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "prefix", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeromacsPrefix(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "prefix", value))
    }
    /// Frame period.
    public static func aeromacsFrameMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "frame_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeromacsFrameMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "frame_ms", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum AvdataGeneratorSettings {
    /// Test aircraft.
    public static func acarsAircraft(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_aircraft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAcarsAircraft(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value))
    }
    /// Test traffic.
    public static func acarsRateFactor(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rate_factor", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAcarsRateFactor(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rate_factor", value))
    }
    /// Test positions.
    public static func acarsPositions(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_positions", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAcarsPositions(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_positions", value))
    }
    /// Test aircraft.
    public static func vdl2Aircraft(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_aircraft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl2Aircraft(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value))
    }
    /// Test signal adjacent station.
    public static func vdl3Adjacent(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_adjacent", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVdl3Adjacent(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_adjacent", value))
    }
    /// Test signal slots.
    public static func gbasVdbSlots(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_slots", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGbasVdbSlots(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_slots", $0) })
    }
    /// Test signal frames.
    public static func gbasVdbFrames(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGbasVdbFrames(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_frames", value))
    }
    /// Test signal duty.
    public static func ldacsDuty(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_duty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLdacsDuty(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_duty", value))
    }
    /// Test signal duty.
    public static func aeromacsDuty(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_duty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeromacsDuty(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_duty", value))
    }
}

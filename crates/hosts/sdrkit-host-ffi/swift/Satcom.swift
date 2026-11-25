// sdrkit: the typed interface of the stack family "satcom", version 2.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum SatcomInterface {
    public static let major = 2
    public static let minor = 0
}

/// The result fields of Inmarsat-C.
public struct InmarsatcFields {
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The offset of the carrier of the first channel from the tuned frequency.
    public var offsetHz: Double
    /// The drift.
    public var driftHzS: Double
    /// The frames found.
    public var framesFound: Int64
    /// The packets good.
    public var packetsOk: Int64
    /// The packets bad.
    public var packetsBad: Int64
    /// The messages seen.
    public var messageCount: Int64
    /// The es/N0.
    public var esn0Db: Double?
    /// The eb/N0.
    public var ebn0Db: Double?
    /// The unique word errors.
    public var uwErrors: Int64?
    /// The unique word errors (average).
    public var uwErrorsAvg: Double?
    /// The symbol error rate.
    public var symbolErrorRate: Double?
    /// The polarity slips.
    public var polaritySlips: Int64
    /// The ocean region code.
    public var oceanRegionCode: Int64?
    /// The ocean region.
    public var region: String?
    /// The station id.
    public var stationId: Int64?
    /// The station.
    public var stationName: String?
    /// The channel type code.
    public var channelTypeCode: Int64?
    /// The channel type.
    public var channelType: String?
    /// The network version.
    public var networkVersion: Int64?
    /// The signalling channel.
    public var signallingChannel: Int64?
    /// The random interval.
    public var randomInterval: Int64?
    /// The status byte.
    public var statusByte: Int64?
    /// The status.
    public var statusText: String?
    /// The services mask.
    public var servicesMask: Int64?
    /// The services.
    public var servicesText: String?
    /// The signalling uplink.
    public var signallingUplinkHz: Double?
    /// The frame number.
    public var frameNumber: Int64?
    /// The frame time of day.
    public var frameTime: Double?

    init(_ c: sdrkit_inmarsatc_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        offsetHz = c.offset_hz
        driftHzS = c.drift_hz_s
        framesFound = c.frames_found
        packetsOk = c.packets_ok
        packetsBad = c.packets_bad
        messageCount = c.message_count
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        ebn0Db = c.has_ebn0_db ? c.ebn0_db : nil
        uwErrors = c.has_uw_errors ? c.uw_errors : nil
        uwErrorsAvg = c.has_uw_errors_avg ? c.uw_errors_avg : nil
        symbolErrorRate = c.has_symbol_error_rate ? c.symbol_error_rate : nil
        polaritySlips = c.polarity_slips
        oceanRegionCode = c.has_ocean_region_code ? c.ocean_region_code : nil
        region = c.has_region ? sdrkitText(c.region) : nil
        stationId = c.has_station_id ? c.station_id : nil
        stationName = c.has_station_name ? sdrkitText(c.station_name) : nil
        channelTypeCode = c.has_channel_type_code ? c.channel_type_code : nil
        channelType = c.has_channel_type ? sdrkitText(c.channel_type) : nil
        networkVersion = c.has_network_version ? c.network_version : nil
        signallingChannel = c.has_signalling_channel ? c.signalling_channel : nil
        randomInterval = c.has_random_interval ? c.random_interval : nil
        statusByte = c.has_status_byte ? c.status_byte : nil
        statusText = c.has_status_text ? sdrkitText(c.status_text) : nil
        servicesMask = c.has_services_mask ? c.services_mask : nil
        servicesText = c.has_services_text ? sdrkitText(c.services_text) : nil
        signallingUplinkHz = c.has_signalling_uplink_hz ? c.signalling_uplink_hz : nil
        frameNumber = c.has_frame_number ? c.frame_number : nil
        frameTime = c.has_frame_time ? c.frame_time : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<InmarsatcFields, SDRKitCode> {
        var c = sdrkit_inmarsatc_fields()
        let code = sdrkit_inmarsatc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(InmarsatcFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct SatcomDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_satcom_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatcomDecoderFields, SDRKitCode> {
        var c = sdrkit_satcom_decoder_fields()
        let code = sdrkit_satcom_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatcomDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An EGC message. The row key is the message number and the service.
public struct InmarsatcMessageRow {
    /// The row key.
    public var key: String
    /// The first heard.
    public var firstHeard: Double?
    /// When it was last heard.
    public var lastHeard: Double?
    /// The channel a message came on (0 the main one).
    public var channel: Int64
    /// The message number.
    public var messageNumber: Int64
    /// The EGC service code.
    public var serviceCode: Int64
    /// The service.
    public var service: String
    /// The kind.
    public var kind: String
    /// The priority.
    public var priority: String
    /// The presentation.
    public var presentation: String
    /// The packets of the message heard.
    public var packets: Int64
    /// Every packet of the message was heard.
    public var complete: Bool
    /// The heard complete.
    public var timesHeard: Int64
    /// The area of a message.
    public var area: String
    /// The subject of a message.
    public var subject: String
    /// The warning.
    public var warningNumber: String
    /// The text of the message.
    public var text: String
    /// The text of the message is cut short.
    public var truncated: Bool

    init(_ c: sdrkit_inmarsatc_message_row) {
        key = sdrkitText(c.key)
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        channel = c.channel
        messageNumber = c.message_number
        serviceCode = c.service_code
        service = sdrkitText(c.service)
        kind = sdrkitText(c.kind)
        priority = sdrkitText(c.priority)
        presentation = sdrkitText(c.presentation)
        packets = c.packets
        complete = c.complete
        timesHeard = c.times_heard
        area = sdrkitText(c.area)
        subject = sdrkitText(c.subject)
        warningNumber = sdrkitText(c.warning_number)
        text = sdrkitText(c.text)
        truncated = c.truncated
    }
}

/// A channel followed now. The row key is the channel index.
public struct InmarsatcChannelRow {
    public enum State: Int32 {
        case searching = 0
        case carrier = 1
        case frames = 2
    }
    /// The row key.
    public var key: String
    /// The channel index.
    public var index: Int64
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double
    /// A carrier was found.
    public var carrier: Bool
    /// The state.
    public var state: State?
    /// The eb/N0.
    public var ebn0Db: Double
    /// The unique word errors (average).
    public var uwErrorsAvg: Double
    /// The frame number.
    public var frameNumber: Int64
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The messages.
    public var messages: Int64
    /// The ocean region.
    public var region: String
    /// The station.
    public var stationName: String
    /// The channel type.
    public var channelType: String

    init(_ c: sdrkit_inmarsatc_channel_row) {
        key = sdrkitText(c.key)
        index = c.index
        offsetHz = c.offset_hz
        carrier = c.carrier
        state = State(rawValue: c.state)
        ebn0Db = c.ebn0_db
        uwErrorsAvg = c.uw_errors_avg
        frameNumber = c.frame_number
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        messages = c.messages
        region = sdrkitText(c.region)
        stationName = sdrkitText(c.station_name)
        channelType = sdrkitText(c.channel_type)
    }
}

/// The packets of one descriptor type. The row key is the descriptor in hexadecimal ("2A").
public struct InmarsatcPacketCountRow {
    /// The row key.
    public var key: String
    /// The descriptor.
    public var descriptor: Int64
    /// The packet type.
    public var descriptorName: String
    /// The packets good.
    public var packetsOk: Int64
    /// The packets bad.
    public var packetsBad: Int64

    init(_ c: sdrkit_inmarsatc_packet_count_row) {
        key = sdrkitText(c.key)
        descriptor = c.descriptor
        descriptorName = sdrkitText(c.descriptor_name)
        packetsOk = c.packets_ok
        packetsBad = c.packets_bad
    }
}

/// The result fields of the Aero display.
public struct AeroFields {
    /// The signal units with a good check.
    public var framesOk: Int64
    /// The signal units with a bad check.
    public var framesBad: Int64
    /// The frames read.
    public var frames: Int64
    /// The messages in the log.
    public var messages: Int64
    /// The positions heard.
    public var positions: Int64
    /// The aircraft in the table.
    public var aircraft: Int64
    /// The channels that decode frames.
    public var channelsDecoding: Int64
    /// The best Eb/N0 of the channels that decode. Absent while none decodes.
    public var ebn0Db: Double?

    init(_ c: sdrkit_aero_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        frames = c.frames
        messages = c.messages
        positions = c.positions
        aircraft = c.aircraft
        channelsDecoding = c.channels_decoding
        ebn0Db = c.has_ebn0_db ? c.ebn0_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AeroFields, SDRKitCode> {
        var c = sdrkit_aero_fields()
        let code = sdrkit_aero_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AeroFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the channel table of the Aero stacks. The row key is the place of the channel.
public struct AeroChannelRow {
    public enum State: Int32 {
        case seen = 0
        case carrier = 1
        case frames = 2
        case data = 3
    }
    /// The row key.
    public var key: String
    /// The id of the channel.
    public var id: Int64
    /// The offset from the centre of the input.
    public var offsetHz: Double
    /// The frequency of the channel.
    public var frequency: Double
    /// The bit rate.
    public var rate: Int64
    /// How far the channel decodes.
    public var state: State?
    /// The energy per bit over the noise density.
    public var ebn0Db: Double
    /// The level of the carrier.
    public var levelDb: Double
    /// The frames read.
    public var frames: Int64
    /// The frames whose unique word was missed.
    public var uwMisses: Int64
    /// The signal units with a good check.
    public var susGood: Int64
    /// The signal units with a bad check.
    public var susBad: Int64
    /// The bit error rate of the channel.
    public var ber: Double

    init(_ c: sdrkit_aero_channel_row) {
        key = sdrkitText(c.key)
        id = c.id
        offsetHz = c.offset_hz
        frequency = c.frequency
        rate = c.rate
        state = State(rawValue: c.state)
        ebn0Db = c.ebn0_db
        levelDb = c.level_db
        frames = c.frames
        uwMisses = c.uw_misses
        susGood = c.sus_good
        susBad = c.sus_bad
        ber = c.ber
    }
}

/// The count of one SU type. The row key is the type in two hexadecimal digits (FE for SUs that failed their check).
public struct AeroSuTypeRow {
    /// The row key.
    public var key: String
    /// The SU type.
    public var suType: Int64
    /// The name of the type.
    public var suName: String
    /// The SUs of the type.
    public var count: Int64

    init(_ c: sdrkit_aero_su_type_row) {
        key = sdrkitText(c.key)
        suType = c.su_type
        suName = sdrkitText(c.su_name)
        count = c.count
    }
}

/// A log-on or log-off. The row key is the number of the entry in the log.
public struct AeroLogonRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var at: Double
    /// The wall-clock time, when known.
    public var time: Double?
    /// The id of the channel.
    public var channel: Int64
    /// The AES id, six hexadecimal digits.
    public var aes: String
    /// The GES id, two hexadecimal digits.
    public var ges: String
    /// A log-on (else a log-off).
    public var logon: Bool

    init(_ c: sdrkit_aero_logon_row) {
        key = sdrkitText(c.key)
        at = c.at
        time = c.has_time ? c.time : nil
        channel = c.channel
        aes = sdrkitText(c.aes)
        ges = sdrkitText(c.ges)
        logon = c.logon
    }
}

/// An ACARS message. The row key is the place of the message in the log.
public struct AeroMessageRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var at: Double
    /// The wall-clock time, when known.
    public var time: Double?
    /// The id of the channel.
    public var channel: Int64
    /// The bit rate of the channel.
    public var rate: Int64
    /// The frequency of the channel.
    public var frequency: Double
    /// The AES id, six hexadecimal digits.
    public var aes: String
    /// The GES id, two hexadecimal digits.
    public var ges: String
    /// The registration.
    public var registration: String
    /// The flight id.
    public var flight: String
    /// The ACARS label.
    public var label: String
    /// What the label means.
    public var labelName: String
    /// The message sequence number.
    public var messageNumber: String
    /// The text.
    public var text: String
    /// The block check is good.
    public var checkOk: Bool
    /// Every block of the message was heard.
    public var complete: Bool
    /// The ADS-C report in words. Empty for a message that is not ADS-C.
    public var adsc: String
    /// The position in the message.
    public var position: SDRKitPosition?
    /// The altitude in the message.
    public var altFt: Double?
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

    init(_ c: sdrkit_aero_message_row) {
        key = sdrkitText(c.key)
        at = c.at
        time = c.has_time ? c.time : nil
        channel = c.channel
        rate = c.rate
        frequency = c.frequency
        aes = sdrkitText(c.aes)
        ges = sdrkitText(c.ges)
        registration = sdrkitText(c.registration)
        flight = sdrkitText(c.flight)
        label = sdrkitText(c.label)
        labelName = sdrkitText(c.label_name)
        messageNumber = sdrkitText(c.message_number)
        text = sdrkitText(c.text)
        checkOk = c.check_ok
        complete = c.complete
        adsc = sdrkitText(c.adsc)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        reading = c.has_reading ? sdrkitText(c.reading) : nil
        readingKind = c.has_reading_kind ? sdrkitText(c.reading_kind) : nil
        readingCategory = c.has_reading_category ? sdrkitText(c.reading_category) : nil
        cpdlcImi = c.has_cpdlc_imi ? sdrkitText(c.cpdlc_imi) : nil
        cpdlcGround = c.has_cpdlc_ground ? sdrkitText(c.cpdlc_ground) : nil
        cpdlcAircraft = c.has_cpdlc_aircraft ? sdrkitText(c.cpdlc_aircraft) : nil
        cpdlcCrcOk = c.has_cpdlc_crc_ok ? c.cpdlc_crc_ok : nil
        cpdlcParsed = c.has_cpdlc_parsed ? c.cpdlc_parsed : nil
        cpdlcText = c.has_cpdlc_text ? sdrkitText(c.cpdlc_text) : nil
    }
}

/// An aircraft heard: the standard aircraft columns, then the frequency last heard on. The row key is the address, else the registration.
public struct AeroAircraftRow {
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
    /// Messages heard.
    public var messages: Int64
    /// The last position.
    public var position: SDRKitPosition?
    /// The altitude of the last position.
    public var altFt: Double?
    /// The track, degrees true.
    public var trackDeg: Double?
    /// The ground speed.
    public var gsKt: Double?
    /// The vertical rate.
    public var vsFpm: Double?
    /// Wall-clock time when last heard.
    public var lastHeard: Double?
    /// The frequency last heard on.
    public var frequency: Double

    init(_ c: sdrkit_aero_aircraft_row) {
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
    }
}

/// A system fact (a satellite, the control of the P and R channels). The row key is the key of the fact.
public struct AeroFactRow {
    /// The row key.
    public var key: String
    /// The kind of the fact.
    public var kind: String
    /// The fact in words.
    public var text: String
    /// Times heard.
    public var heard: Int64?
    /// Wall-clock time when last heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_aero_fact_row) {
        key = sdrkitText(c.key)
        kind = sdrkitText(c.kind)
        text = sdrkitText(c.text)
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the merged Inmarsat display.
public struct InmarsatFields {
    /// The ACARS messages and the Inmarsat-C bulletin frames.
    public var framesOk: Int64
    /// The satellite that the identification names. Absent while it names none.
    public var satellite: String?
    /// How sure the identification is, 0 to 1.
    public var satelliteConfidence: Double?
    /// Two satellites fit about as well.
    public var satelliteAmbiguous: Bool
    /// The beam, in words.
    public var beam: String
    /// A note on the identification.
    public var identificationNote: String
    /// The ACARS messages.
    public var messages: Int64
    /// The ACARS messages with a position.
    public var positions: Int64
    /// The Inmarsat-C frames with a bulletin board.
    public var egcFrames: Int64
    /// The carriers found.
    public var carriers: Int64
    /// The carriers that decode.
    public var decoding: Int64

    init(_ c: sdrkit_inmarsat_fields) {
        framesOk = c.frames_ok
        satellite = c.has_satellite ? sdrkitText(c.satellite) : nil
        satelliteConfidence = c.has_satellite_confidence ? c.satellite_confidence : nil
        satelliteAmbiguous = c.satellite_ambiguous
        beam = sdrkitText(c.beam)
        identificationNote = sdrkitText(c.identification_note)
        messages = c.messages
        positions = c.positions
        egcFrames = c.egc_frames
        carriers = c.carriers
        decoding = c.decoding
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<InmarsatFields, SDRKitCode> {
        var c = sdrkit_inmarsat_fields()
        let code = sdrkit_inmarsat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(InmarsatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A carrier found. The row key is the carrier id in five digits.
public struct InmarsatCarrierRow {
    public enum Class: Int32 {
        case narrow = 0
        case medium = 1
        case wide = 2
    }
    public enum Decodes: Int32 {
        case unknown = 0
        case aeroP = 1
        case inmarsatC = 2
    }
    /// The row key.
    public var key: String
    /// The id of the carrier.
    public var id: Int64
    /// The offset from the centre of the input.
    public var offsetHz: Double
    /// The frequency of the carrier.
    public var frequency: Double
    /// The width of the carrier.
    public var width: Double
    /// The level over the floor.
    public var snrDb: Double
    /// The class by the width.
    public var `class`: Class?
    /// What decodes the carrier.
    public var decodes: Decodes?
    /// The bit rate.
    public var rate: Int64
    /// How far the carrier decodes: 0 seen, 1 carrier, 2 frames, 3 data.
    public var state: Int64
    /// The energy per bit over the noise density.
    public var ebn0Db: Double
    /// The frames read.
    public var frames: Int64
    /// What the carrier is on the identified satellite.
    public var satellite: String
    /// The window of the carrier is parked.
    public var parked: Bool
    /// The carrier has the focus.
    public var focused: Bool
    /// The window that holds the carrier.
    public var window: Int64
    /// The carrier has the width of a voice channel.
    public var voiceCandidate: Bool

    init(_ c: sdrkit_inmarsat_carrier_row) {
        key = sdrkitText(c.key)
        id = c.id
        offsetHz = c.offset_hz
        frequency = c.frequency
        width = c.width
        snrDb = c.snr_db
        `class` = Class(rawValue: c.class_)
        decodes = Decodes(rawValue: c.decodes)
        rate = c.rate
        state = c.state
        ebn0Db = c.ebn0_db
        frames = c.frames
        satellite = sdrkitText(c.satellite)
        parked = c.parked
        focused = c.focused
        window = c.window
        voiceCandidate = c.voice_candidate
    }
}

/// A satellite that the identification weighed. The row key is the name.
public struct InmarsatCandidateRow {
    /// The row key.
    public var key: String
    /// The name of the satellite.
    public var name: String
    /// How sure the identification is of this satellite, 0 to 1.
    public var satelliteConfidence: Double
    /// The score of the decoded content.
    public var content: Double?
    /// The score of the frequencies.
    public var frequencyScore: Double?
    /// The score of the geometry.
    public var geometry: Double?
    /// The elevation from the station.
    public var elevationDeg: Double?
    /// The azimuth from the station.
    public var azimuthDeg: Double?

    init(_ c: sdrkit_inmarsat_candidate_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        satelliteConfidence = c.satellite_confidence
        content = c.has_content ? c.content : nil
        frequencyScore = c.has_frequency_score ? c.frequency_score : nil
        geometry = c.has_geometry ? c.geometry : nil
        elevationDeg = c.has_elevation_deg ? c.elevation_deg : nil
        azimuthDeg = c.has_azimuth_deg ? c.azimuth_deg : nil
    }
}

/// A reason of a candidate. The row key is `<candidate>/<n>`.
public struct InmarsatCandidateReasonRow {
    /// The row key.
    public var key: String
    /// The candidate.
    public var candidate: String
    /// The reason in words.
    public var why: String

    init(_ c: sdrkit_inmarsat_candidate_reason_row) {
        key = sdrkitText(c.key)
        candidate = sdrkitText(c.candidate)
        why = sdrkitText(c.why)
    }
}

/// The result fields of Iridium.
public struct IridiumFields {
    /// Bursts the detector found.
    public var bursts: Int64
    /// Bursts demodulated.
    public var demodulated: Int64
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Input items dropped because a helper thread did not keep up.
    public var dropped: Int64
    /// Bursts found twice and counted once.
    public var duplicates: Int64
    /// Bursts with the downlink unique word.
    public var downlink: Int64
    /// Bursts with the uplink unique word.
    public var uplink: Int64
    /// Bursts found per second (the last report interval).
    public var burstRate: Double
    /// Unique words found per second (the last report interval).
    public var uwRate: Double
    /// The mean confidence of the bursts in the last interval.
    public var confidencePct: Double?
    /// The mean Es/N0 of the bursts in the last interval.
    public var esn0Db: Double?
    /// The detector's noise floor, dB relative to full scale in 1 Hz.
    public var noiseDbHz: Double?

    init(_ c: sdrkit_iridium_fields) {
        bursts = c.bursts
        demodulated = c.demodulated
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        dropped = c.dropped
        duplicates = c.duplicates
        downlink = c.downlink
        uplink = c.uplink
        burstRate = c.burst_rate
        uwRate = c.uw_rate
        confidencePct = c.has_confidence_pct ? c.confidence_pct : nil
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        noiseDbHz = c.has_noise_db_hz ? c.noise_db_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IridiumFields, SDRKitCode> {
        var c = sdrkit_iridium_fields()
        let code = sdrkit_iridium_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IridiumFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A RAW line. The row key is the sequence number of the line.
public struct IridiumRawRow {
    public enum Direction: Int32 {
        case downlink = 0
        case uplink = 1
    }
    /// The row key.
    public var key: String
    /// At (signal time).
    public var at: Double
    /// Frequency.
    public var frequency: Double
    /// Direction.
    public var direction: Direction?
    /// Confidence.
    public var confidencePct: Double
    /// RAW line.
    public var line: String

    init(_ c: sdrkit_iridium_raw_row) {
        key = sdrkitText(c.key)
        at = c.at
        frequency = c.frequency
        direction = Direction(rawValue: c.direction)
        confidencePct = c.confidence_pct
        line = sdrkitText(c.line)
    }
}

/// The result fields of Globalstar.
public struct GlobalstarFields {
    /// Channels active now.
    public var activeChannels: Int64
    /// Stretches of activity completed since the start (or the last Clear).
    public var activities: Int64
    /// The mean Doppler of the active channels.
    public var doppler: Double?

    init(_ c: sdrkit_globalstar_fields) {
        activeChannels = c.active_channels
        activities = c.activities
        doppler = c.has_doppler ? c.doppler : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GlobalstarFields, SDRKitCode> {
        var c = sdrkit_globalstar_fields()
        let code = sdrkit_globalstar_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GlobalstarFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the 13 channels. The row key is the channel number ("07").
public struct GlobalstarChannelRow {
    /// The row key.
    public var key: String
    /// A channel number, 1 to 13.
    public var channel: Int64
    /// The centre of the channel.
    public var centre: Double
    /// The channel lies inside the input.
    public var inView: Bool
    /// The channel is active now.
    public var active: Bool
    /// The power of the channel.
    public var powerDbfs: Double
    /// The noise floor of the channel.
    public var floorDbfs: Double
    /// The signal to noise ratio of the channel.
    public var snrDb: Double
    /// The share of time the channel was on (60 s average).
    public var occupancyPct: Double
    /// The Doppler shift of the channel.
    public var doppler: Double
    /// The signal time when the channel was last active.
    public var lastActive: Double?
    /// The stretches of activity on the channel.
    public var events: Int64

    init(_ c: sdrkit_globalstar_channel_row) {
        key = sdrkitText(c.key)
        channel = c.channel
        centre = c.centre
        inView = c.in_view
        active = c.active
        powerDbfs = c.power_dbfs
        floorDbfs = c.floor_dbfs
        snrDb = c.snr_db
        occupancyPct = c.occupancy_pct
        doppler = c.doppler
        lastActive = c.has_last_active ? c.last_active : nil
        events = c.events
    }
}

/// A stretch of activity on a channel. The row key is the sequence number of the stretch.
public struct GlobalstarActivityRow {
    /// The row key.
    public var key: String
    /// The signal time of the start.
    public var start: Double
    /// How long a stretch of activity lasted.
    public var duration: Double
    /// A channel number, 1 to 13.
    public var channel: Int64
    /// The peak SNR.
    public var peakSnrDb: Double
    /// The Doppler shift at the start.
    public var dopplerStart: Double
    /// The Doppler shift at the end.
    public var dopplerEnd: Double
    /// The wall-clock time of the start, when the samples are live.
    public var wallStart: Double?

    init(_ c: sdrkit_globalstar_activity_row) {
        key = sdrkitText(c.key)
        start = c.start
        duration = c.duration
        channel = c.channel
        peakSnrDb = c.peak_snr_db
        dopplerStart = c.doppler_start
        dopplerEnd = c.doppler_end
        wallStart = c.has_wall_start ? c.wall_start : nil
    }
}

/// The result fields of GOES DCS.
public struct GoesDcsFields {
    /// The reports read whole.
    public var framesOk: Int64
    /// The reports cut off or lost.
    public var framesBad: Int64
    /// The carriers followed now.
    public var carriers: Int64

    init(_ c: sdrkit_goes_dcs_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        carriers = c.carriers
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GoesDcsFields, SDRKitCode> {
        var c = sdrkit_goes_dcs_fields()
        let code = sdrkit_goes_dcs_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GoesDcsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A platform report. The row key is the sequence number of the report.
public struct GoesDcsReportRow {
    public enum Format: Int32 {
        case ascii = 0
        case pseudoBinary = 1
        case other = 2
    }
    /// The row key.
    public var key: String
    /// The signal time of a report.
    public var at: Double
    /// The wall-clock time of a report (live radios only).
    public var heardAt: Double?
    /// The address word (8 hex digits).
    public var address: String
    /// The bit rate.
    public var bitRateHz: Double
    /// Manchester coded.
    public var manchester: Bool
    /// The channel number.
    public var channel: Int64
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The flag byte, or -1.
    public var flags: Int64
    /// The data format.
    public var format: Format?
    /// The report ended with an EOT.
    public var eot: Bool
    /// Characters whose parity failed.
    public var parityErrors: Int64
    /// The data of the report.
    public var text: String

    init(_ c: sdrkit_goes_dcs_report_row) {
        key = sdrkitText(c.key)
        at = c.at
        heardAt = c.has_heard_at ? c.heard_at : nil
        address = sdrkitText(c.address)
        bitRateHz = c.bit_rate_hz
        manchester = c.manchester
        channel = c.channel
        snrDb = c.snr_db
        flags = c.flags
        format = Format(rawValue: c.format)
        eot = c.eot
        parityErrors = c.parity_errors
        text = sdrkitText(c.text)
    }
}

/// A platform heard. The row key is the address.
public struct GoesDcsPlatformRow {
    public enum Format: Int32 {
        case ascii = 0
        case pseudoBinary = 1
        case other = 2
    }
    /// The row key.
    public var key: String
    /// The address word (8 hex digits).
    public var address: String
    /// The channel number.
    public var channel: Int64
    /// The bit rate.
    public var bitRateHz: Double
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The data format.
    public var format: Format?
    /// The data of the report.
    public var text: String
    /// When it was last heard.
    public var lastHeard: Double?
    /// How many times it was heard.
    public var heard: Int64?

    init(_ c: sdrkit_goes_dcs_platform_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        channel = c.channel
        bitRateHz = c.bit_rate_hz
        snrDb = c.snr_db
        format = Format(rawValue: c.format)
        text = sdrkitText(c.text)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A carrier followed now. The row key is its place in the list ("000").
public struct GoesDcsCarrierRow {
    /// The row key.
    public var key: String
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double
    /// The channel number.
    public var channel: Int64
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The bit rate.
    public var bitRateHz: Double?

    init(_ c: sdrkit_goes_dcs_carrier_row) {
        key = sdrkitText(c.key)
        offsetHz = c.offset_hz
        channel = c.channel
        snrDb = c.snr_db
        bitRateHz = c.has_bit_rate_hz ? c.bit_rate_hz : nil
    }
}

/// The result fields of Argos.
public struct ArgosFields {
    public enum Link: Int32 {
        case auto = 0
        case uplink = 1
        case downlink = 2
    }
    public enum Coding: Int32 {
        case nrz = 0
        case invertedNrz = 1
        case nrzi = 2
    }
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The link the display shows: uplink or downlink.
    public var link: Link?
    /// The receiver holds lock.
    public var locked: Bool
    /// The downlink's carrier line over the noise.
    public var carrierLineDb: Double
    /// Downlink bits received.
    public var downlinkBits: Int64
    /// The downlink coding in use.
    public var coding: Coding?

    init(_ c: sdrkit_argos_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        link = Link(rawValue: c.link)
        locked = c.locked
        carrierLineDb = c.carrier_line_db
        downlinkBits = c.downlink_bits
        coding = c.has_coding ? Coding(rawValue: c.coding) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ArgosFields, SDRKitCode> {
        var c = sdrkit_argos_fields()
        let code = sdrkit_argos_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ArgosFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An uplink burst. The row key is the sequence number of the burst.
public struct ArgosBurstRow {
    public enum Kind: Int32 {
        case a2 = 0
        case other = 1
    }
    /// The row key.
    public var key: String
    /// The signal time of the row's moment.
    public var at: Double
    /// The wall-clock time of the row's moment, when the samples were live.
    public var heardAt: Double?
    /// The modulation of a burst.
    public var kind: Kind?
    /// The carrier from the dial.
    public var offsetHz: Double
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The unmodulated carrier.
    public var carrierMs: Double
    /// The whole burst.
    public var totalMs: Double
    /// The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
    public var quality: Double
    /// The platform id (7 hex digits).
    public var platform: String?
    /// The number of user data bits.
    public var dataBits: Int64?
    /// The user data, the first bit the high bit of the first byte.
    public var data: [UInt8]?

    init(_ c: sdrkit_argos_burst_row) {
        key = sdrkitText(c.key)
        at = c.at
        heardAt = c.has_heard_at ? c.heard_at : nil
        kind = Kind(rawValue: c.kind)
        offsetHz = c.offset_hz
        snrDb = c.snr_db
        carrierMs = c.carrier_ms
        totalMs = c.total_ms
        quality = c.quality
        platform = c.has_platform ? sdrkitText(c.platform) : nil
        dataBits = c.has_data_bits ? c.data_bits : nil
        data = c.has_data ? [UInt8](UnsafeBufferPointer(start: c.data.ptr, count: c.data.len)) : nil
    }
}

/// A platform heard. The row key is the platform id.
public struct ArgosPlatformRow {
    /// The row key.
    public var key: String
    /// The platform id (7 hex digits).
    public var platform: String
    /// The user data, the first bit the high bit of the first byte.
    public var data: [UInt8]
    /// The drift.
    public var driftHzS: Double
    /// When it was last heard.
    public var lastHeard: Double?
    /// How many times it was heard.
    public var heard: Int64?

    init(_ c: sdrkit_argos_platform_row) {
        key = sdrkitText(c.key)
        platform = sdrkitText(c.platform)
        data = [UInt8](UnsafeBufferPointer(start: c.data.ptr, count: c.data.len))
        driftHzS = c.drift_hz_s
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A downlink frame with a good FCS. The row key is the sequence number of the frame.
public struct ArgosFrameRow {
    public enum Coding: Int32 {
        case nrz = 0
        case invertedNrz = 1
        case nrzi = 2
    }
    /// The row key.
    public var key: String
    /// The signal time of the row's moment.
    public var at: Double
    /// The downlink coding in use.
    public var coding: Coding?
    /// The length of a frame without its FCS.
    public var bytes: Int64
    /// A frame's bytes.
    public var frame: [UInt8]

    init(_ c: sdrkit_argos_frame_row) {
        key = sdrkitText(c.key)
        at = c.at
        coding = Coding(rawValue: c.coding)
        bytes = c.bytes
        frame = [UInt8](UnsafeBufferPointer(start: c.frame.ptr, count: c.frame.len))
    }
}

/// The result fields of Meteosat HRDCP.
public struct HrdcpFields {
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The carriers followed now.
    public var carriers: Int64
    /// The channel plan in use.
    public var plan: String
    /// What is not confirmed by a public document.
    public var note: String

    init(_ c: sdrkit_hrdcp_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        carriers = c.carriers
        plan = sdrkitText(c.plan)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HrdcpFields, SDRKitCode> {
        var c = sdrkit_hrdcp_fields()
        let code = sdrkit_hrdcp_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HrdcpFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of the log. The row key is the sequence number of the message.
public struct HrdcpReportRow {
    /// The row key.
    public var key: String
    /// The signal time of a message.
    public var at: Double
    /// The wall-clock time of a message (live radios only).
    public var heardAt: Double?
    /// The address word (8 hex digits).
    public var address: String
    /// The bits corrected.
    public var bitsCorrected: Int64
    /// The sequence counter.
    public var sequence: Int64
    /// Self-timed or alert.
    public var type: String
    /// The 10 health bits.
    public var health: String
    /// The compression of the data.
    public var compression: String
    /// The CRC passed.
    public var crcOk: Bool
    /// Coded frames of the message.
    public var frames: Int64
    /// Frames whose words did not decode.
    public var framesBad: Int64
    /// Reed-Solomon bytes corrected.
    public var rsCorrected: Int64
    /// Platform data bytes.
    public var bytes: Int64
    /// ASCII or binary.
    public var format: String
    /// The bit rate.
    public var bitRateHz: Double
    /// Manchester coded.
    public var manchester: Bool
    /// The frequency of the carrier.
    public var freqHz: Double
    /// The channel number of the plan.
    public var channel: Int64?
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The data of the message.
    public var text: String

    init(_ c: sdrkit_hrdcp_report_row) {
        key = sdrkitText(c.key)
        at = c.at
        heardAt = c.has_heard_at ? c.heard_at : nil
        address = sdrkitText(c.address)
        bitsCorrected = c.bits_corrected
        sequence = c.sequence
        type = sdrkitText(c.type)
        health = sdrkitText(c.health)
        compression = sdrkitText(c.compression)
        crcOk = c.crc_ok
        frames = c.frames
        framesBad = c.frames_bad
        rsCorrected = c.rs_corrected
        bytes = c.bytes
        format = sdrkitText(c.format)
        bitRateHz = c.bit_rate_hz
        manchester = c.manchester
        freqHz = c.freq_hz
        channel = c.has_channel ? c.channel : nil
        snrDb = c.snr_db
        text = sdrkitText(c.text)
    }
}

/// A platform heard. The row key is the address.
public struct HrdcpPlatformRow {
    /// The row key.
    public var key: String
    /// The address word (8 hex digits).
    public var address: String
    /// The channel number of the plan.
    public var channel: Int64?
    /// The signal to noise ratio.
    public var snrDb: Double
    /// ASCII or binary.
    public var format: String
    /// The data of the last message.
    public var text: String
    /// When it was last heard.
    public var lastHeard: Double?
    /// How many times it was heard.
    public var heard: Int64?

    init(_ c: sdrkit_hrdcp_platform_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        channel = c.has_channel ? c.channel : nil
        snrDb = c.snr_db
        format = sdrkitText(c.format)
        text = sdrkitText(c.text)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A carrier followed now. The row key is its place in the list ("000").
public struct HrdcpCarrierRow {
    /// The row key.
    public var key: String
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double
    /// The channel number of the plan.
    public var channel: Int64?
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The carrier state: 0 looking for the marker, 2 decoding.
    public var state: Int64
    /// The bit rate.
    public var bitRateHz: Double?
    /// Platform data bytes.
    public var bytes: Int64

    init(_ c: sdrkit_hrdcp_carrier_row) {
        key = sdrkitText(c.key)
        offsetHz = c.offset_hz
        channel = c.has_channel ? c.channel : nil
        snrDb = c.snr_db
        state = c.state
        bitRateHz = c.has_bit_rate_hz ? c.bit_rate_hz : nil
        bytes = c.bytes
    }
}

/// The result fields of COSPAS-SARSAT.
public struct SarsatFields {
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Bursts heard.
    public var burstsHeard: Int64
    /// First-generation finders running.
    public var finders: Int64
    /// The width the finders cover.
    public var spanHz: Double

    init(_ c: sdrkit_sarsat_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        burstsHeard = c.bursts_heard
        finders = c.finders
        spanHz = c.span_hz
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SarsatFields, SDRKitCode> {
        var c = sdrkit_sarsat_fields()
        let code = sdrkit_sarsat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SarsatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst of the log. The row key is the sequence number of the burst.
public struct SarsatBurstRow {
    /// The row key.
    public var key: String
    /// The signal time of a burst.
    public var at: Double
    /// The wall-clock time of the burst, when the samples were live.
    public var heardAt: Double?
    /// The beacon generation, 1 or 2.
    public var generation: Int64
    /// The beacon's hex identity (15 digits, or 23 for the second generation).
    public var hexId: String
    /// The beacon type (EPIRB, ELT, PLB ...).
    public var beaconType: String
    /// The country of the beacon's MID.
    public var country: String
    /// The protocol in words.
    public var `protocol`: String
    /// The identification in words.
    public var identification: String
    /// The position that the beacon sends.
    public var position: SDRKitPosition?
    /// The 121.5 MHz homing flag.
    public var homing: Bool
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The level of the input.
    public var levelDbfs: Double?
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double

    init(_ c: sdrkit_sarsat_burst_row) {
        key = sdrkitText(c.key)
        at = c.at
        heardAt = c.has_heard_at ? c.heard_at : nil
        generation = c.generation
        hexId = sdrkitText(c.hex_id)
        beaconType = sdrkitText(c.beacon_type)
        country = sdrkitText(c.country)
        `protocol` = sdrkitText(c.protocol)
        identification = sdrkitText(c.identification)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        homing = c.homing
        snrDb = c.snr_db
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.offset_hz
    }
}

/// A beacon heard, with its last burst. The row key is the hex identity.
public struct SarsatBeaconRow {
    /// The row key.
    public var key: String
    /// The beacon's hex identity (15 digits, or 23 for the second generation).
    public var hexId: String
    /// The beacon generation, 1 or 2.
    public var generation: Int64
    /// The beacon type (EPIRB, ELT, PLB ...).
    public var beaconType: String
    /// The country of the beacon's MID.
    public var country: String
    /// The identification in words.
    public var identification: String
    /// The position that the beacon sends.
    public var position: SDRKitPosition?
    /// Bursts whose soft bits were summed to decode the message.
    public var combined: Int64?
    /// A later burst decoded to the same identification before the message was reported.
    public var confirmed: Bool?
    /// The distance from the decoded word to the next one that passes, over the mean reliability of a bit.
    public var repairMargin: Double?
    /// The reliability of the repaired bits over the mean reliability of a bit.
    public var repairCost: Double?
    /// When it was last heard.
    public var lastHeard: Double?
    /// How many times it was heard.
    public var heard: Int64?

    init(_ c: sdrkit_sarsat_beacon_row) {
        key = sdrkitText(c.key)
        hexId = sdrkitText(c.hex_id)
        generation = c.generation
        beaconType = sdrkitText(c.beacon_type)
        country = sdrkitText(c.country)
        identification = sdrkitText(c.identification)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        combined = c.has_combined ? c.combined : nil
        confirmed = c.has_confirmed ? c.confirmed : nil
        repairMargin = c.has_repair_margin ? c.repair_margin : nil
        repairCost = c.has_repair_cost ? c.repair_cost : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// The result fields of the LoRa satellite stack.
public struct LoraSatFields {
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The pass planner's name of the satellite overhead.
    public var satellite: String
    /// The name of the preset that runs.
    public var preset: String?
    /// The number of detectors without Doppler tracking.
    public var demodulators: Int64
    /// The largest Doppler shift expected.
    public var dopplerMaxHz: Double
    /// The largest Doppler rate expected.
    public var dopplerRateMaxHzS: Double

    init(_ c: sdrkit_lora_sat_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        satellite = sdrkitText(c.satellite)
        preset = c.has_preset ? sdrkitText(c.preset) : nil
        demodulators = c.demodulators
        dopplerMaxHz = c.doppler_max_hz
        dopplerRateMaxHzS = c.doppler_rate_max_hz_s
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LoraSatFields, SDRKitCode> {
        var c = sdrkit_lora_sat_fields()
        let code = sdrkit_lora_sat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraSatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of the log. The row key is the sequence number of the frame.
public struct LoraSatFrameRow {
    public enum Crc: Int32 {
        case none = 0
        case ok = 1
        case bad = 2
    }
    /// The row key.
    public var key: String
    /// The wall-clock time a frame was heard (live radios only).
    public var heardAt: Double?
    /// The spreading factor of a frame.
    public var sf: Int64
    /// The bandwidth of a frame.
    public var bwHz: Double
    /// The coding rate of a frame: the denominator n of 4/n.
    public var cr: Int64
    /// The CRC of a frame.
    public var crc: Crc?
    /// The sync word of a frame.
    public var sync: Int64
    /// The payload length of a frame.
    public var bytes: Int64
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The carrier offset from the dial at the first data symbol.
    public var dopplerHz: Double
    /// The carrier rate during a frame.
    public var dopplerRateHzS: Double?
    /// The frequency of the frame.
    public var freqHz: Double
    /// The pass planner's name of the satellite overhead.
    public var satellite: String?
    /// The payload of a frame.
    public var payload: [UInt8]
    /// The payload as printable text.
    public var text: String

    init(_ c: sdrkit_lora_sat_frame_row) {
        key = sdrkitText(c.key)
        heardAt = c.has_heard_at ? c.heard_at : nil
        sf = c.sf
        bwHz = c.bw_hz
        cr = c.cr
        crc = Crc(rawValue: c.crc)
        sync = c.sync
        bytes = c.bytes
        snrDb = c.snr_db
        dopplerHz = c.doppler_hz
        dopplerRateHzS = c.has_doppler_rate_hz_s ? c.doppler_rate_hz_s : nil
        freqHz = c.freq_hz
        satellite = c.has_satellite ? sdrkitText(c.satellite) : nil
        payload = [UInt8](UnsafeBufferPointer(start: c.payload.ptr, count: c.payload.len))
        text = sdrkitText(c.text)
    }
}

/// A demodulator running now. The row key is its place in the list ("000").
public struct LoraSatDemodulatorRow {
    /// The row key.
    public var key: String
    /// The spreading factor of a frame.
    public var sf: Int64
    /// The bandwidth of a frame.
    public var bwHz: Double
    /// Detector counters: preambles found.
    public var preambles: Int64
    /// Detector counters: preambles without a sync word or SFD.
    public var syncFailed: Int64
    /// Detector counters: headers that failed.
    public var headerFailed: Int64
    /// Detector counters: frames decoded, good or not.
    public var frames: Int64
    /// Detector counters: frames whose CRC failed.
    public var crcFailed: Int64

    init(_ c: sdrkit_lora_sat_demodulator_row) {
        key = sdrkitText(c.key)
        sf = c.sf
        bwHz = c.bw_hz
        preambles = c.preambles
        syncFailed = c.sync_failed
        headerFailed = c.header_failed
        frames = c.frames
        crcFailed = c.crc_failed
    }
}

/// A sync word of a demodulator. The row key is the demodulator and the sync word ("000/18").
public struct LoraSatDemodulatorSyncRow {
    /// The row key.
    public var key: String
    /// The detector a sync word belongs to.
    public var demodulator: String
    /// The sync word of a frame.
    public var sync: Int64

    init(_ c: sdrkit_lora_sat_demodulator_sync_row) {
        key = sdrkitText(c.key)
        demodulator = sdrkitText(c.demodulator)
        sync = c.sync
    }
}

/// The frames of one spreading factor. The row key is the SF ("07").
public struct LoraSatSfRow {
    /// The row key.
    public var key: String
    /// The spreading factor of a frame.
    public var sf: Int64
    /// Detector counters: frames decoded, good or not.
    public var frames: Int64

    init(_ c: sdrkit_lora_sat_sf_row) {
        key = sdrkitText(c.key)
        sf = c.sf
        frames = c.frames
    }
}

/// The result fields of Orbcomm.
public struct OrbcommFields {
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Messages joined from their parts.
    public var messagesJoined: Int64
    /// Satellites heard.
    public var satellites: Int64

    init(_ c: sdrkit_orbcomm_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        messagesJoined = c.messages_joined
        satellites = c.satellites
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<OrbcommFields, SDRKitCode> {
        var c = sdrkit_orbcomm_fields()
        let code = sdrkit_orbcomm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(OrbcommFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A satellite heard. The row key is the satellite number.
public struct OrbcommSatelliteRow {
    /// The row key.
    public var key: String
    /// The satellite.
    public var satellite: Int64
    /// The frequency.
    public var frequency: Double?
    /// The doppler.
    public var doppler: Double?
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The frames.
    public var frames: Int64
    /// How many times it was heard.
    public var heard: Int64?
    /// The bad packets.
    public var bad: Int64
    /// The position.
    public var position: SDRKitPosition?
    /// The height.
    public var heightM: Double?
    /// The inclination.
    public var inclinationDeg: Double?
    /// The period.
    public var period: Double?
    /// When it was last heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_orbcomm_satellite_row) {
        key = sdrkitText(c.key)
        satellite = c.satellite
        frequency = c.has_frequency ? c.frequency : nil
        doppler = c.has_doppler ? c.doppler : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        frames = c.frames
        heard = c.has_heard ? c.heard : nil
        bad = c.bad
        position = c.has_position ? SDRKitPosition(c.position) : nil
        heightM = c.has_height_m ? c.height_m : nil
        inclinationDeg = c.has_inclination_deg ? c.inclination_deg : nil
        period = c.has_period ? c.period : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A channel that a satellite announces. The row key is `<satellite>/<u or d><n>`.
public struct OrbcommSatelliteChannelRow {
    /// The row key.
    public var key: String
    /// The satellite.
    public var satellite: Int64
    /// The channel is an uplink (else a downlink).
    public var uplink: Bool
    /// The frequency.
    public var frequency: Double

    init(_ c: sdrkit_orbcomm_satellite_channel_row) {
        key = sdrkitText(c.key)
        satellite = c.satellite
        uplink = c.uplink
        frequency = c.frequency
    }
}

/// A decoder. The row key is the decoder number.
public struct OrbcommDecoderRow {
    public enum State: Int32 {
        case free = 0
        case acquiring = 1
        case tracking = 2
        case frameLock = 3
    }
    /// The row key.
    public var key: String
    /// The decoder.
    public var decoder: Int64
    /// The state.
    public var state: State?
    /// The frequency.
    public var frequency: Double?
    /// The offset.
    public var offset: Double?
    /// The doppler.
    public var doppler: Double?
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The level.
    public var levelDb: Double?
    /// The satellite.
    public var satellite: Int64?
    /// The clock error.
    public var clockPpm: Double?
    /// The frames.
    public var frames: Int64?
    /// Frames that passed their check.
    public var framesOk: Int64?
    /// Frames that failed their check.
    public var framesBad: Int64?
    /// The sync losses.
    public var syncLosses: Int64?

    init(_ c: sdrkit_orbcomm_decoder_row) {
        key = sdrkitText(c.key)
        decoder = c.decoder
        state = State(rawValue: c.state)
        frequency = c.has_frequency ? c.frequency : nil
        offset = c.has_offset ? c.offset : nil
        doppler = c.has_doppler ? c.doppler : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDb = c.has_level_db ? c.level_db : nil
        satellite = c.has_satellite ? c.satellite : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        frames = c.has_frames ? c.frames : nil
        framesOk = c.has_frames_ok ? c.frames_ok : nil
        framesBad = c.has_frames_bad ? c.frames_bad : nil
        syncLosses = c.has_sync_losses ? c.sync_losses : nil
    }
}

/// A packet of the log. The row key is the sequence number of the packet.
public struct OrbcommPacketRow {
    public enum TypeOption: Int32 {
        case sync = 0
        case message = 1
        case uplink = 2
        case downlink = 3
        case network = 4
        case fill = 5
        case ephemeris = 6
        case elements = 7
        case other = 8
        case messageJoined = 9
    }
    /// The row key.
    public var key: String
    /// The at (signal time).
    public var at: Double
    /// The time.
    public var wall: Double?
    /// The satellite.
    public var satellite: Int64?
    /// The decoder.
    public var decoder: Int64
    /// The type.
    public var type: TypeOption?
    /// The bytes.
    public var bytes: [UInt8]?
    /// What the packet says.
    public var text: String

    init(_ c: sdrkit_orbcomm_packet_row) {
        key = sdrkitText(c.key)
        at = c.at
        wall = c.has_wall ? c.wall : nil
        satellite = c.has_satellite ? c.satellite : nil
        decoder = c.decoder
        type = TypeOption(rawValue: c.type)
        bytes = c.has_bytes ? [UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len)) : nil
        text = sdrkitText(c.text)
    }
}

/// The packets of one type. The row key is the key of the type.
public struct OrbcommPacketTypeRow {
    public enum TypeOption: Int32 {
        case sync = 0
        case message = 1
        case uplink = 2
        case downlink = 3
        case network = 4
        case fill = 5
        case ephemeris = 6
        case elements = 7
        case other = 8
        case messageJoined = 9
    }
    /// The row key.
    public var key: String
    /// The type.
    public var type: TypeOption?
    /// The count.
    public var count: Int64

    init(_ c: sdrkit_orbcomm_packet_type_row) {
        key = sdrkitText(c.key)
        type = TypeOption(rawValue: c.type)
        count = c.count
    }
}

/// The result fields of the PSK demodulator.
public struct PskDemodFields {
    public enum Order: Int32 {
        case auto = 0
        case bpsk = 1
        case qpsk = 2
        case oqpsk = 3
        case n8psk = 4
    }
    public enum Shape: Int32 {
        case rect = 0
        case rrc = 1
    }
    public enum Note: Int32 {
        case tooShort = 0
        case noRateLine = 1
        case noTrial = 2
        case noConstellation = 3
    }
    /// The symbol rate.
    public var symbolRateHz: Double?
    /// The modulation order the demodulator runs.
    public var order: Order?
    /// The carrier loop's lock value (1 on the points).
    public var lock: Double?
    /// The carrier, from the dial.
    public var carrierHz: Double?
    /// The drift.
    public var driftHzS: Double?
    /// The EVM.
    public var evmPct: Double?
    /// Es/N0 from the error vector.
    public var esn0Db: Double?
    /// Eb/N0 from the error vector.
    public var ebn0Db: Double?
    /// Symbols demodulated since the lock.
    public var symbols: Int64
    /// Bits since the start.
    public var bits: Int64
    /// Frames that passed their check.
    public var framesOk: Int64
    /// The newest bits, oldest first, as the characters 0 and 1.
    public var bitStream: String
    /// The index of the first bit of `bit_stream`.
    public var bitsFirst: Int64
    /// The phase rotation the sync words resolved.
    public var rotationDeg: Double
    /// Whether the sync words resolved a spectral inversion.
    public var conjugated: Bool
    /// Whether the sync words resolved an OQPSK rail shift.
    public var railShift: Bool
    /// Whether the phase ambiguity is resolved.
    public var resolved: Bool
    /// The centre of the signal found, from the dial.
    public var signalHz: Double?
    /// Its width at 3 dB.
    public var width3dbHz: Double?
    /// Its width at 10 dB.
    public var width10dbHz: Double?
    /// Its 99 % occupied bandwidth.
    public var occupiedHz: Double?
    /// Its in-band SNR.
    public var signalSnrDb: Double?
    /// The symbol rate the analyser measured, before snapping.
    public var baudRawHz: Double?
    /// Whether the rate was snapped to a standard one.
    public var snapped: Bool?
    /// The confidence of the rate, 0 to 1.
    public var rateConfidence: Double?
    /// The confidence of the order, 0 to 1.
    public var orderConfidence: Double?
    /// The pulse shape.
    public var shape: Shape?
    /// The roll-off of root raised cosine pulses.
    public var rollOff: Double?
    /// The envelope line over the median.
    public var envelopeDb: Double?
    /// The squared signal's line over the median.
    public var lineX2Db: Double?
    /// The fourth power's line over the median.
    public var lineX4Db: Double?
    /// The eighth power's line over the median.
    public var lineX8Db: Double?
    /// What the analyser said when it found no PSK signal.
    public var note: Note?

    init(_ c: sdrkit_psk_demod_fields) {
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        order = c.has_order ? Order(rawValue: c.order) : nil
        lock = c.has_lock ? c.lock : nil
        carrierHz = c.has_carrier_hz ? c.carrier_hz : nil
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        evmPct = c.has_evm_pct ? c.evm_pct : nil
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        ebn0Db = c.has_ebn0_db ? c.ebn0_db : nil
        symbols = c.symbols
        bits = c.bits
        framesOk = c.frames_ok
        bitStream = sdrkitText(c.bit_stream)
        bitsFirst = c.bits_first
        rotationDeg = c.rotation_deg
        conjugated = c.conjugated
        railShift = c.rail_shift
        resolved = c.resolved
        signalHz = c.has_signal_hz ? c.signal_hz : nil
        width3dbHz = c.has_width_3db_hz ? c.width_3db_hz : nil
        width10dbHz = c.has_width_10db_hz ? c.width_10db_hz : nil
        occupiedHz = c.has_occupied_hz ? c.occupied_hz : nil
        signalSnrDb = c.has_signal_snr_db ? c.signal_snr_db : nil
        baudRawHz = c.has_baud_raw_hz ? c.baud_raw_hz : nil
        snapped = c.has_snapped ? c.snapped : nil
        rateConfidence = c.has_rate_confidence ? c.rate_confidence : nil
        orderConfidence = c.has_order_confidence ? c.order_confidence : nil
        shape = c.has_shape ? Shape(rawValue: c.shape) : nil
        rollOff = c.has_roll_off ? c.roll_off : nil
        envelopeDb = c.has_envelope_db ? c.envelope_db : nil
        lineX2Db = c.has_line_x2_db ? c.line_x2_db : nil
        lineX4Db = c.has_line_x4_db ? c.line_x4_db : nil
        lineX8Db = c.has_line_x8_db ? c.line_x8_db : nil
        note = c.has_note ? Note(rawValue: c.note) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PskDemodFields, SDRKitCode> {
        var c = sdrkit_psk_demod_fields()
        let code = sdrkit_psk_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PskDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A rate candidate of the analyser. The row key is its place ("0").
public struct PskDemodRateRow {
    /// The row key.
    public var key: String
    /// A rate candidate.
    public var rateHz: Double
    /// A line's strength over the median.
    public var strengthDb: Double

    init(_ c: sdrkit_psk_demod_rate_row) {
        key = sdrkitText(c.key)
        rateHz = c.rate_hz
        strengthDb = c.strength_db
    }
}

/// A trial demodulation of the analyser. The row key is its place ("00").
public struct PskDemodTrialRow {
    public enum Order: Int32 {
        case auto = 0
        case bpsk = 1
        case qpsk = 2
        case oqpsk = 3
        case n8psk = 4
    }
    /// The row key.
    public var key: String
    /// The modulation order the demodulator runs.
    public var order: Order?
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The error vector of a trial, unit power.
    public var evm: Double

    init(_ c: sdrkit_psk_demod_trial_row) {
        key = sdrkitText(c.key)
        order = Order(rawValue: c.order)
        symbolRateHz = c.symbol_rate_hz
        evm = c.evm
    }
}

/// The bit index after a recent sync word, oldest first. The row key is its place ("00").
public struct PskDemodSyncRow {
    /// The row key.
    public var key: String
    /// The bit index after a sync word.
    public var bit: Int64

    init(_ c: sdrkit_psk_demod_sync_row) {
        key = sdrkitText(c.key)
        bit = c.bit
    }
}

/// A line of the log of analyses. The row key is the sequence number of the line.
public struct PskDemodLogRow {
    /// The row key.
    public var key: String
    /// The signal time of a line of the log.
    public var at: Double
    /// The line.
    public var text: String

    init(_ c: sdrkit_psk_demod_log_row) {
        key = sdrkitText(c.key)
        at = c.at
        text = sdrkitText(c.text)
    }
}

/// The result fields of QO-100.
public struct Qo100Fields {
    public enum BeaconLock: Int32 {
        case outside = 0
        case searching = 1
        case locked = 2
        case held = 3
    }
    /// The LNB's LO error (actual minus nominal).
    public var lnbErrorHz: Double
    /// The LNB error in parts per million of the nominal LO.
    public var lnbErrorPpm: Double
    /// The drift.
    public var driftHzS: Double
    /// The phase of the beacon lock.
    public var beaconLock: BeaconLock?
    /// Whether the LNB error is applied to the frequencies shown.
    public var correctionApplied: Bool
    /// Fine estimates that found a CW beacon.
    public var measurements: Int64
    /// The locked for.
    public var lockedFor: Double
    /// The USB dial for listening (RF).
    public var dialHz: Double
    /// The conversation followed by the dial.
    public var listening: Int64?
    /// Whether the wideband transponder is outside the radio's band.
    public var widebandOutside: Bool
    /// The noise floor of the narrowband spectrum.
    public var narrowFloorDbfs: Double?
    /// The level of the strongest signal in the 2.7 kHz above the dial less the level of the PSK beacon, both as the peak of a spectrum bin. While the stack sends, that signal is the user's own downlink: it must stay below 0 dB. With nothing on the dial it is the level of the noise peaks there. No value while the beacon is not found.
    public var ownToBeaconDb: Double?

    init(_ c: sdrkit_qo100_fields) {
        lnbErrorHz = c.lnb_error_hz
        lnbErrorPpm = c.lnb_error_ppm
        driftHzS = c.drift_hz_s
        beaconLock = BeaconLock(rawValue: c.beacon_lock)
        correctionApplied = c.correction_applied
        measurements = c.measurements
        lockedFor = c.locked_for
        dialHz = c.dial_hz
        listening = c.has_listening ? c.listening : nil
        widebandOutside = c.wideband_outside
        narrowFloorDbfs = c.has_narrow_floor_dbfs ? c.narrow_floor_dbfs : nil
        ownToBeaconDb = c.has_own_to_beacon_db ? c.own_to_beacon_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Qo100Fields, SDRKitCode> {
        var c = sdrkit_qo100_fields()
        let code = sdrkit_qo100_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Qo100Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the voice sender of QO-100. The measurements have no value while the stack does not send.
public struct Qo100TxFields {
    /// Whether the stack transmits now.
    public var transmitting: Bool
    /// The uplink frequency of the dial: the dial less the translation of the transponder.
    public var txFreqHz: Double
    /// The time on air of the transmission.
    public var txTimeS: Double?
    /// The peak of the microphone audio over the last report.
    public var txMicDbfs: Double?
    /// The peak of the modulation over the last report, against the peak level.
    public var txPeakDb: Double?
    /// The gain of the level control of the speech processor.
    public var txLevelDb: Double?
    /// The most gain reduction of the limiter over the last report: 0 when it did not act.
    public var txLimitDb: Double?

    init(_ c: sdrkit_qo100_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.tx_freq_hz
        txTimeS = c.has_tx_time_s ? c.tx_time_s : nil
        txMicDbfs = c.has_tx_mic_dbfs ? c.tx_mic_dbfs : nil
        txPeakDb = c.has_tx_peak_db ? c.tx_peak_db : nil
        txLevelDb = c.has_tx_level_db ? c.tx_level_db : nil
        txLimitDb = c.has_tx_limit_db ? c.tx_limit_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Qo100TxFields, SDRKitCode> {
        var c = sdrkit_qo100_tx_fields()
        let code = sdrkit_qo100_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Qo100TxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A beacon of the narrowband transponder. The row key is its place ("000").
public struct Qo100BeaconRow {
    /// The row key.
    public var key: String
    /// The name of a beacon.
    public var name: String
    /// A nominal RF.
    public var nominalHz: Double
    /// Whether a beacon was found.
    public var found: Bool
    /// The RF where a beacon shows with no correction.
    public var apparentHz: Double
    /// The signal to noise ratio.
    public var snrDb: Double

    init(_ c: sdrkit_qo100_beacon_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        nominalHz = c.nominal_hz
        found = c.found
        apparentHz = c.apparent_hz
        snrDb = c.snr_db
    }
}

/// A signal of the spectrum now. The row key is its place ("000").
public struct Qo100SignalRow {
    public enum Class: Int32 {
        case unknown = 0
        case beacon = 1
        case carrier = 2
        case cw = 3
        case psk = 4
        case ssb = 5
        case sstv = 6
        case wideDigital = 7
    }
    /// The row key.
    public var key: String
    /// The RF of a signal.
    public var rfHz: Double
    /// The width of a signal.
    public var widthHz: Double
    /// The level of a signal.
    public var levelDbfs: Double
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The tracker's class of a signal.
    public var `class`: Class?
    /// The beacon a signal is.
    public var beacon: String?
    /// The id of a track.
    public var track: Int64?

    init(_ c: sdrkit_qo100_signal_row) {
        key = sdrkitText(c.key)
        rfHz = c.rf_hz
        widthHz = c.width_hz
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        `class` = Class(rawValue: c.class_)
        beacon = c.has_beacon ? sdrkitText(c.beacon) : nil
        track = c.has_track ? c.track : nil
    }
}

/// A signal followed over time. The row key is the track number.
public struct Qo100TrackRow {
    public enum Class: Int32 {
        case unknown = 0
        case beacon = 1
        case carrier = 2
        case cw = 3
        case psk = 4
        case ssb = 5
        case sstv = 6
        case wideDigital = 7
    }
    /// The row key.
    public var key: String
    /// When it was last heard.
    public var lastHeard: Double?
    /// The RF of a signal.
    public var rfHz: Double
    /// The width of a signal.
    public var widthHz: Double
    /// The level of a signal.
    public var levelDbfs: Double
    /// The signal to noise ratio.
    public var snrDb: Double
    /// Whether a track or a conversation is active now.
    public var active: Bool
    /// The share of the last 30 s a track was on.
    public var dutyPct: Double
    /// The tracker's class of a signal.
    public var `class`: Class?
    /// The conversation of a track.
    public var conversation: Int64?

    init(_ c: sdrkit_qo100_track_row) {
        key = sdrkitText(c.key)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        rfHz = c.rf_hz
        widthHz = c.width_hz
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        active = c.active
        dutyPct = c.duty_pct
        `class` = Class(rawValue: c.class_)
        conversation = c.has_conversation ? c.conversation : nil
    }
}

/// A conversation: SSB tracks grouped by frequency and time. The row key is the conversation number.
public struct Qo100ConversationRow {
    /// The row key.
    public var key: String
    /// When it was last heard.
    public var lastHeard: Double?
    /// Whether a track or a conversation is active now.
    public var active: Bool
    /// The USB dial for listening (RF).
    public var dialHz: Double
    /// The lower edge of a conversation.
    public var lowerEdgeHz: Double
    /// The level of a signal.
    public var levelDbfs: Double
    /// The peak level of a conversation.
    public var peakDbfs: Double
    /// The stations estimated in a conversation.
    public var stations: Int64
    /// The talk time of a conversation.
    public var talk: Double
    /// How long a conversation has lasted.
    public var length: Double

    init(_ c: sdrkit_qo100_conversation_row) {
        key = sdrkitText(c.key)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        active = c.active
        dialHz = c.dial_hz
        lowerEdgeHz = c.lower_edge_hz
        levelDbfs = c.level_dbfs
        peakDbfs = c.peak_dbfs
        stations = c.stations
        talk = c.talk
        length = c.length
    }
}

/// A callsign read in a conversation. The row key is the conversation and the callsign ("3/G4ABC").
public struct Qo100CallsignRow {
    /// The row key.
    public var key: String
    /// The conversation of a track.
    public var conversation: Int64
    /// The callsign.
    public var callsign: String

    init(_ c: sdrkit_qo100_callsign_row) {
        key = sdrkitText(c.key)
        conversation = c.conversation
        callsign = sdrkitText(c.callsign)
    }
}

/// A DATV carrier of the wideband transponder. The row key is its place ("000").
public struct Qo100DatvCarrierRow {
    /// The row key.
    public var key: String
    /// The RF of a signal.
    public var rfHz: Double
    /// The centre of a DATV carrier in the input.
    public var ifHz: Double
    /// The width of a signal.
    public var widthHz: Double
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The signal to noise ratio.
    public var snrDb: Double
    /// Whether a DATV carrier has a flat top.
    public var flatTopped: Bool
    /// Whether a DATV carrier is the beacon.
    public var isBeacon: Bool

    init(_ c: sdrkit_qo100_datv_carrier_row) {
        key = sdrkitText(c.key)
        rfHz = c.rf_hz
        ifHz = c.if_hz
        widthHz = c.width_hz
        symbolRateHz = c.symbol_rate_hz
        snrDb = c.snr_db
        flatTopped = c.flat_topped
        isBeacon = c.is_beacon
    }
}

/// The result fields of the CubeSat telemetry stack.
public struct CubesatFields {
    /// Frames that passed their check.
    public var framesOk: Int64
    /// Frames the Reed-Solomon code repaired.
    public var rsRepaired: Int64
    /// Frames sent to the KISS output.
    public var kissSent: Int64
    /// Which parts of the framings received are not confirmed by a public document.
    public var note: String

    init(_ c: sdrkit_cubesat_fields) {
        framesOk = c.frames_ok
        rsRepaired = c.rs_repaired
        kissSent = c.kiss_sent
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CubesatFields, SDRKitCode> {
        var c = sdrkit_cubesat_fields()
        let code = sdrkit_cubesat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CubesatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the AX.25 sender of the CubeSat stack.
public struct CubesatTxFields {
    /// Whether a transmission is on air.
    public var txSending: Bool
    /// The frames of the transmission on air.
    public var txSent: Int64
    /// The frames waiting for the next transmission.
    public var txWaiting: Int64

    init(_ c: sdrkit_cubesat_tx_fields) {
        txSending = c.tx_sending
        txSent = c.tx_sent
        txWaiting = c.tx_waiting
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CubesatTxFields, SDRKitCode> {
        var c = sdrkit_cubesat_tx_fields()
        let code = sdrkit_cubesat_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CubesatTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of the log. The row key is the sequence number of the frame.
public struct CubesatFrameRow {
    public enum Framing: Int32 {
        case ax25Afsk = 0
        case ax25G3ruh = 1
        case ax25Nrzi = 2
        case ax100Mode5 = 3
        case ax100Mode6 = 4
        case ccsdsRs = 5
        case geoscan = 6
        case lucky7 = 7
        case spino = 8
        case erminaz = 9
        case usp = 10
        case ngham = 11
        case u482c = 12
        case mobitex = 13
        case mobitexNx = 14
        case beesat1 = 15
        case beesat9 = 16
        case ccsdsConcat = 17
    }
    /// The row key.
    public var key: String
    /// The wall-clock time a frame was heard (live radios only).
    public var heardAt: Double?
    /// The framing of a frame.
    public var framing: Framing?
    /// The bit rate of a frame.
    public var baud: Int64
    /// Reed-Solomon symbols corrected (none for a framing without Reed-Solomon).
    public var rsCorrected: Int64?
    /// The satellite of a frame.
    public var satellite: String?
    /// Its NORAD number.
    public var norad: Int64
    /// The name comes from the dial frequency.
    public var guess: Bool
    /// The frequency of the frame.
    public var freqHz: Double
    /// AX.25 source.
    public var from: String?
    /// AX.25 destination.
    public var to: String?
    /// AX.25 repeaters, separated by commas.
    public var path: String?
    /// AX.25 frame type.
    public var frameType: String?
    /// AX.25 PID.
    public var pid: Int64?
    /// The CSP header in words.
    public var csp: String?
    /// The frame's data.
    public var data: [UInt8]
    /// The payload.
    public var payload: [UInt8]
    /// The text beacon.
    public var text: String?
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// What passed the checks of a frame.
    public var validated: String?
    /// The Lucky-7 kind of a frame.
    public var kind: String?
    /// The sync word was found complemented.
    public var inverted: Bool
    /// The spacecraft id of a CCSDS header.
    public var spacecraft: Int64?
    /// The virtual channel of a CCSDS header.
    public var vc: Int64?
    /// The NGHam flags.
    public var flags: Int64?

    init(_ c: sdrkit_cubesat_frame_row) {
        key = sdrkitText(c.key)
        heardAt = c.has_heard_at ? c.heard_at : nil
        framing = Framing(rawValue: c.framing)
        baud = c.baud
        rsCorrected = c.has_rs_corrected ? c.rs_corrected : nil
        satellite = c.has_satellite ? sdrkitText(c.satellite) : nil
        norad = c.norad
        guess = c.guess
        freqHz = c.freq_hz
        from = c.has_from ? sdrkitText(c.from) : nil
        to = c.has_to ? sdrkitText(c.to) : nil
        path = c.has_path ? sdrkitText(c.path) : nil
        frameType = c.has_frame_type ? sdrkitText(c.frame_type) : nil
        pid = c.has_pid ? c.pid : nil
        csp = c.has_csp ? sdrkitText(c.csp) : nil
        data = [UInt8](UnsafeBufferPointer(start: c.data.ptr, count: c.data.len))
        payload = [UInt8](UnsafeBufferPointer(start: c.payload.ptr, count: c.payload.len))
        text = c.has_text ? sdrkitText(c.text) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        validated = c.has_validated ? sdrkitText(c.validated) : nil
        kind = c.has_kind ? sdrkitText(c.kind) : nil
        inverted = c.inverted
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        vc = c.has_vc ? c.vc : nil
        flags = c.has_flags ? c.flags : nil
    }
}

/// The frames of one framing. The row key is the key of the framing.
public struct CubesatFramingRow {
    public enum Framing: Int32 {
        case ax25Afsk = 0
        case ax25G3ruh = 1
        case ax25Nrzi = 2
        case ax100Mode5 = 3
        case ax100Mode6 = 4
        case ccsdsRs = 5
        case geoscan = 6
        case lucky7 = 7
        case spino = 8
        case erminaz = 9
        case usp = 10
        case ngham = 11
        case u482c = 12
        case mobitex = 13
        case mobitexNx = 14
        case beesat1 = 15
        case beesat9 = 16
        case ccsdsConcat = 17
    }
    /// The row key.
    public var key: String
    /// The framing of a frame.
    public var framing: Framing?
    /// Frames by framing.
    public var frames: Int64
    /// Sync word matches of a framing.
    public var syncHits: Int64?
    /// Candidates of a framing that failed their checks.
    public var decodeFailed: Int64?

    init(_ c: sdrkit_cubesat_framing_row) {
        key = sdrkitText(c.key)
        framing = Framing(rawValue: c.framing)
        frames = c.frames
        syncHits = c.has_sync_hits ? c.sync_hits : nil
        decodeFailed = c.has_decode_failed ? c.decode_failed : nil
    }
}

/// A carrier found now. The row key is its place ("000").
public struct CubesatCarrierRow {
    /// The row key.
    public var key: String
    /// A carrier's offset from the dial.
    public var offsetHz: Double
    /// The signal to noise ratio.
    public var snrDb: Double

    init(_ c: sdrkit_cubesat_carrier_row) {
        key = sdrkitText(c.key)
        offsetHz = c.offset_hz
        snrDb = c.snr_db
    }
}

/// The result fields of Fleet Broadcast.
public struct FleetBroadcastFields {
    /// The state: 0 nothing, 1 carrier, 2 bits, 3 framed.
    public var state: Int64
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// The frequency a station was heard on.
    public var freqHz: Double?
    /// The line of the carrier search over the floor.
    public var lineDb: Double
    /// Sample-clock or symbol-clock error, parts per million.
    public var clockPpm: Double
    /// The eye: 0 clean, towards 1 noisy.
    public var eye: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Symbols so far.
    public var symbols: Int64
    /// The frames are aligned.
    public var framed: Bool
    /// Several columns repeat.
    public var ambiguous: Bool
    /// The repeat of the sync channel, frames.
    public var syncPeriod: Int64?
    /// The agreement of the sync channel with itself one period later.
    public var syncAgreement: Double?
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// The class letters, channel 0 first.
    public var classes: String
    /// What is not confirmed by a public document.
    public var note: String

    init(_ c: sdrkit_fleet_broadcast_fields) {
        state = c.state
        offsetHz = c.offset_hz
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        lineDb = c.line_db
        clockPpm = c.clock_ppm
        eye = c.eye
        snrDb = c.snr_db
        symbols = c.symbols
        framed = c.framed
        ambiguous = c.ambiguous
        syncPeriod = c.has_sync_period ? c.sync_period : nil
        syncAgreement = c.has_sync_agreement ? c.sync_agreement : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        levelDbfs = c.level_dbfs
        classes = sdrkitText(c.classes)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FleetBroadcastFields, SDRKitCode> {
        var c = sdrkit_fleet_broadcast_fields()
        let code = sdrkit_fleet_broadcast_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FleetBroadcastFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// One of the 16 channels. The row key is its place, `000` first.
public struct FleetBroadcastChannelRow {
    /// The row key.
    public var key: String
    /// The channel (or column, when not framed).
    public var channel: Int64
    /// The class of a channel.
    public var `class`: String
    /// The share of bit changes.
    public var activityPct: Double
    /// The share of ones.
    public var onesPct: Double
    /// The repeat of the sync channel, frames.
    public var syncPeriod: Int64?
    /// The last 64 bits as hex, oldest first.
    public var lastBits: String

    init(_ c: sdrkit_fleet_broadcast_channel_row) {
        key = sdrkitText(c.key)
        channel = c.channel
        `class` = sdrkitText(c.class_)
        activityPct = c.activity_pct
        onesPct = c.ones_pct
        syncPeriod = c.has_sync_period ? c.sync_period : nil
        lastBits = sdrkitText(c.last_bits)
    }
}

/// A frame of the frame log. The row key is its place, `000` first.
public struct FleetBroadcastFrameRow {
    /// The row key.
    public var key: String
    /// A frame as 4 hex digits, channel 0 the most significant bit.
    public var word: String
    /// A frame as 16 bits, channel 0 first.
    public var bits: String
    /// The signal time of the start of the frame.
    public var timeS: Double

    init(_ c: sdrkit_fleet_broadcast_frame_row) {
        key = sdrkitText(c.key)
        word = sdrkitText(c.word)
        bits = sdrkitText(c.bits)
        timeS = c.time_s
    }
}

/// The result fields of BGAN.
public struct BganFields {
    /// Carriers found.
    public var carriers: Int64
    /// Carriers in frame sync.
    public var inSync: Int64
    /// Carriers whose signalling decoded.
    public var decoded: Int64
    /// GMR-2 class carriers.
    public var gmr2: Int64
    /// Networks heard (MCC-MNC).
    public var networks: String
    /// Satellites heard.
    public var satellitesHeard: String
    /// FEC blocks decoded.
    public var blocksOk: Int64
    /// FEC blocks that failed.
    public var blocksBad: Int64
    /// Connection PDUs counted, never read.
    public var connectionPdus: Int64
    /// PDUs addressed to a terminal counted, never read.
    public var addressedPdus: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// What is not confirmed by a public document.
    public var note: String

    init(_ c: sdrkit_bgan_fields) {
        carriers = c.carriers
        inSync = c.in_sync
        decoded = c.decoded
        gmr2 = c.gmr2
        networks = sdrkitText(c.networks)
        satellitesHeard = sdrkitText(c.satellites_heard)
        blocksOk = c.blocks_ok
        blocksBad = c.blocks_bad
        connectionPdus = c.connection_pdus
        addressedPdus = c.addressed_pdus
        levelDbfs = c.level_dbfs
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BganFields, SDRKitCode> {
        var c = sdrkit_bgan_fields()
        let code = sdrkit_bgan_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BganFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A carrier. The row key is its place, `000` first.
public struct BganCarrierRow {
    /// The row key.
    public var key: String
    /// The frequency a station was heard on.
    public var freqHz: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// Width of a carrier.
    public var widthHz: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// What the carrier is.
    public var kind: String
    /// The demodulator state.
    public var state: String
    /// Bearer.
    public var bearer: String?
    /// Subtype of block 0.
    public var subtype: String?
    /// Symbol rate found, Hz.
    public var symbolRateHz: Double?
    /// Frequency left after the channel filter.
    public var cfoHz: Double?
    /// C/N0 from the pilots.
    public var cn0Dbhz: Double?
    /// Frames in sync.
    public var frames: Int64
    /// FEC blocks decoded.
    public var blocksOk: Int64
    /// FEC blocks that failed.
    public var blocksBad: Int64
    /// Share of blocks with PDUs.
    public var loadPct: Double
    /// Frame sync without decodable tables.
    public var structureOnly: Bool
    /// Spot beam id.
    public var spotBeam: Int64?
    /// rnc-id.
    public var rnc: Int64?
    /// bct-id.
    public var bctId: Int64?
    /// f-bearer.
    public var fBearer: Int64?
    /// Beam type.
    public var beamType: String?
    /// MCC-MNC.
    public var network: String?
    /// The carrying satellite.
    public var satellite: String?
    /// Seconds since last heard.
    public var lastSeen: Double

    init(_ c: sdrkit_bgan_carrier_row) {
        key = sdrkitText(c.key)
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        offsetHz = c.offset_hz
        widthHz = c.width_hz
        snrDb = c.snr_db
        kind = sdrkitText(c.kind)
        state = sdrkitText(c.state)
        bearer = c.has_bearer ? sdrkitText(c.bearer) : nil
        subtype = c.has_subtype ? sdrkitText(c.subtype) : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        cfoHz = c.has_cfo_hz ? c.cfo_hz : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
        frames = c.frames
        blocksOk = c.blocks_ok
        blocksBad = c.blocks_bad
        loadPct = c.load_pct
        structureOnly = c.structure_only
        spotBeam = c.has_spot_beam ? c.spot_beam : nil
        rnc = c.has_rnc ? c.rnc : nil
        bctId = c.has_bct_id ? c.bct_id : nil
        fBearer = c.has_f_bearer ? c.f_bearer : nil
        beamType = c.has_beam_type ? sdrkitText(c.beam_type) : nil
        network = c.has_network ? sdrkitText(c.network) : nil
        satellite = c.has_satellite ? sdrkitText(c.satellite) : nil
        lastSeen = c.last_seen
    }
}

/// A spot beam. The row key is its place, `000` first.
public struct BganBeamRow {
    /// The row key.
    public var key: String
    /// Spot beam id.
    public var spotBeam: Int64
    /// Beam type.
    public var beamType: String?
    /// The forward frequency of a primary bearer.
    public var primaryHz: Double?
    /// The FBearerType name of a primary bearer.
    public var bearerType: String?

    init(_ c: sdrkit_bgan_beam_row) {
        key = sdrkitText(c.key)
        spotBeam = c.spot_beam
        beamType = c.has_beam_type ? sdrkitText(c.beam_type) : nil
        primaryHz = c.has_primary_hz ? c.primary_hz : nil
        bearerType = c.has_bearer_type ? sdrkitText(c.bearer_type) : nil
    }
}

/// A line of the signalling log. The row key is its place, `000` first.
public struct BganLogRow {
    /// The row key.
    public var key: String
    /// Seconds into the capture.
    public var at: Double
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_bgan_log_row) {
        key = sdrkitText(c.key)
        at = c.at
        text = sdrkitText(c.text)
    }
}

/// The result fields of the satellite carrier survey.
public struct SatSurveyFields {
    /// The text of a decoded message.
    public var text: String
    /// The method of the submode.
    public var method: String
    /// The threshold in force.
    public var thresholdInForceDb: Double
    /// The bin of the transform.
    public var binHz: Double?
    /// Element sets read.
    public var catalogue: Int64
    /// Carrier lines shown.
    public var lines: Int64?
    /// Channels active now.
    public var channelsActive: Int64?
    /// Channels that have been active.
    public var channelsSeen: Int64?
    /// The frame rate searched.
    public var frameRateHz: Double?
    /// The frame rate that fitted best.
    public var measuredRateHz: Double?
    /// The level of the frame fit.
    public var frameLevelDb: Double?
    /// The frame structure is detected.
    public var detected: Bool?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_sat_survey_fields) {
        text = sdrkitText(c.text)
        method = sdrkitText(c.method)
        thresholdInForceDb = c.threshold_in_force_db
        binHz = c.has_bin_hz ? c.bin_hz : nil
        catalogue = c.catalogue
        lines = c.has_lines ? c.lines : nil
        channelsActive = c.has_channels_active ? c.channels_active : nil
        channelsSeen = c.has_channels_seen ? c.channels_seen : nil
        frameRateHz = c.has_frame_rate_hz ? c.frame_rate_hz : nil
        measuredRateHz = c.has_measured_rate_hz ? c.measured_rate_hz : nil
        frameLevelDb = c.has_frame_level_db ? c.frame_level_db : nil
        detected = c.has_detected ? c.detected : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatSurveyFields, SDRKitCode> {
        var c = sdrkit_sat_survey_fields()
        let code = sdrkit_sat_survey_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatSurveyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A carrier line. The row key is the number of the line.
public struct SatSurveyCarrierRow {
    /// The row key.
    public var key: String
    /// The number of a carrier line.
    public var line: Int64
    /// Offset from the centre of the samples.
    public var offsetHz: Double
    /// The frequency, MHz.
    public var mhz: Double
    /// The level over the background or floor.
    public var snrDb: Double
    /// The length of a line.
    public var durationS: Double
    /// The drift of a line.
    public var driftHzS: Double?
    /// The line or channel is active.
    public var active: Bool
    /// The channel name of a line.
    public var channel: String
    /// The satellite that the Doppler curve names.
    public var satellite: String
    /// Its catalogue number.
    public var norad: Int64?
    /// The rms of the Doppler match.
    public var rmsHz: Double?
    /// Another satellite fits about as well.
    public var ambiguous: Bool

    init(_ c: sdrkit_sat_survey_carrier_row) {
        key = sdrkitText(c.key)
        line = c.line
        offsetHz = c.offset_hz
        mhz = c.mhz
        snrDb = c.snr_db
        durationS = c.duration_s
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        active = c.active
        channel = sdrkitText(c.channel)
        satellite = sdrkitText(c.satellite)
        norad = c.has_norad ? c.norad : nil
        rmsHz = c.has_rms_hz ? c.rms_hz : nil
        ambiguous = c.ambiguous
    }
}

/// A channel that has been active. The row key is its grid frequency in whole Hz.
public struct SatSurveyChannelRow {
    /// The row key.
    public var key: String
    /// Offset from the centre of the samples.
    public var offsetHz: Double
    /// The frequency of a channel.
    public var frequencyHz: Double
    /// The width of a channel.
    public var widthHz: Double
    /// The level over the background or floor.
    public var snrDb: Double
    /// The share of time a channel is on.
    public var duty: Double
    /// Times a channel turned active.
    public var bursts: Int64
    /// The signal time a channel was last on.
    public var lastSeenS: Double
    /// The line or channel is active.
    public var active: Bool
    /// The name of a channel.
    public var name: String

    init(_ c: sdrkit_sat_survey_channel_row) {
        key = sdrkitText(c.key)
        offsetHz = c.offset_hz
        frequencyHz = c.frequency_hz
        widthHz = c.width_hz
        snrDb = c.snr_db
        duty = c.duty
        bursts = c.bursts
        lastSeenS = c.last_seen_s
        active = c.active
        name = sdrkitText(c.name)
    }
}

/// A complete EGC message. The source is the message number and the service, the text the message.
public struct InmarsatcEgcEvent {
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
    /// The message number and the service.
    public var source: String
    /// The text of the message.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?
    /// How urgent the message is, from its priority.
    public var urgency: Urgency?
    /// The message number.
    public var messageNumber: Int64
    /// The EGC service code.
    public var serviceCode: Int64
    /// The EGC priority code (0 routine, 1 safety, 2 urgency, 3 distress).
    public var priorityCode: Int64
    /// The subject of a message.
    public var subject: String
    /// The area of a message.
    public var area: String

    init(_ c: sdrkit_inmarsatc_egc_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        messageNumber = c.message_number
        serviceCode = c.service_code
        priorityCode = c.priority_code
        subject = sdrkitText(c.subject)
        area = sdrkitText(c.area)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<InmarsatcEgcEvent, SDRKitCode> {
        var c = sdrkit_inmarsatc_egc_event()
        let code = sdrkit_inmarsatc_egc_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(InmarsatcEgcEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An ACARS message.
public struct AeroMessageEvent {
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
    /// The registration. Absent when the message names none.
    public var source: String?
    /// The text.
    public var text: String
    /// What the event is about: an aircraft.
    public var family: Family?
    /// The position in the message.
    public var position: SDRKitPosition?
    /// The ACARS label.
    public var label: String
    /// The AES id, six hexadecimal digits.
    public var aes: String
    /// The flight id.
    public var flight: String
    /// The block check is good.
    public var checkOk: Bool

    init(_ c: sdrkit_aero_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        label = sdrkitText(c.label)
        aes = sdrkitText(c.aes)
        flight = sdrkitText(c.flight)
        checkOk = c.check_ok
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AeroMessageEvent, SDRKitCode> {
        var c = sdrkit_aero_message_event()
        let code = sdrkit_aero_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AeroMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A log-on or log-off.
public struct AeroLogonEvent {
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
    /// The AES id, six hexadecimal digits.
    public var source: String
    /// `log-on` or `log-off`.
    public var text: String
    /// What the event is about: an aircraft.
    public var family: Family?
    /// The GES id, two hexadecimal digits.
    public var ges: String
    /// A log-on (else a log-off).
    public var logon: Bool

    init(_ c: sdrkit_aero_logon_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        ges = sdrkitText(c.ges)
        logon = c.logon
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AeroLogonEvent, SDRKitCode> {
        var c = sdrkit_aero_logon_event()
        let code = sdrkit_aero_logon_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AeroLogonEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A RAW line of one burst.
public struct IridiumRawEvent {
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
    /// The text of a decoded message.
    public var text: String
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// Confidence.
    public var confidencePct: Double

    init(_ c: sdrkit_iridium_raw_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        confidencePct = c.confidence_pct
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IridiumRawEvent, SDRKitCode> {
        var c = sdrkit_iridium_raw_event()
        let code = sdrkit_iridium_raw_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IridiumRawEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A stretch of activity on a channel. The source is the channel ("channel 7").
public struct GlobalstarActivityEvent {
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
    /// The channel ("channel 7").
    public var source: String
    /// The length and the Doppler shift of the stretch.
    public var text: String
    /// The peak signal to noise ratio of the stretch.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// A channel number, 1 to 13.
    public var channel: Int64
    /// How long a stretch of activity lasted.
    public var duration: Double

    init(_ c: sdrkit_globalstar_activity_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        channel = c.channel
        duration = c.duration
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GlobalstarActivityEvent, SDRKitCode> {
        var c = sdrkit_globalstar_activity_event()
        let code = sdrkit_globalstar_activity_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GlobalstarActivityEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A platform report. The source is the address, the text the data.
public struct GoesDcsReportEvent {
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
    public enum Format: Int32 {
        case ascii = 0
        case pseudoBinary = 1
        case other = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The address of the platform.
    public var source: String
    /// The data of the report.
    public var text: String
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The address word (8 hex digits).
    public var address: String
    /// The channel number.
    public var channel: Int64
    /// The bit rate.
    public var bitRateHz: Double
    /// Manchester coded.
    public var manchester: Bool
    /// The flag byte, or -1.
    public var flags: Int64
    /// The data format.
    public var format: Format?
    /// The report ended with an EOT.
    public var eot: Bool
    /// Characters whose parity failed.
    public var parityErrors: Int64
    /// The address bits corrected.
    public var bitsCorrected: Int64

    init(_ c: sdrkit_goes_dcs_report_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        address = sdrkitText(c.address)
        channel = c.channel
        bitRateHz = c.bit_rate_hz
        manchester = c.manchester
        flags = c.flags
        format = Format(rawValue: c.format)
        eot = c.eot
        parityErrors = c.parity_errors
        bitsCorrected = c.bits_corrected
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GoesDcsReportEvent, SDRKitCode> {
        var c = sdrkit_goes_dcs_report_event()
        let code = sdrkit_goes_dcs_report_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GoesDcsReportEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A platform message. The source is the platform id, the text the data in hexadecimal.
public struct ArgosPttEvent {
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
    /// The platform id.
    public var source: String
    /// The data in hexadecimal.
    public var text: String
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The platform id (7 hex digits).
    public var platform: String
    /// The number of user data bits.
    public var dataBits: Int64
    /// The carrier from the dial.
    public var offsetHz: Double
    /// The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
    public var quality: Double
    /// The user data, the first bit the high bit of the first byte.
    public var data: [UInt8]

    init(_ c: sdrkit_argos_ptt_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        platform = sdrkitText(c.platform)
        dataBits = c.data_bits
        offsetHz = c.offset_hz
        quality = c.quality
        data = [UInt8](UnsafeBufferPointer(start: c.data.ptr, count: c.data.len))
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArgosPttEvent, SDRKitCode> {
        var c = sdrkit_argos_ptt_event()
        let code = sdrkit_argos_ptt_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArgosPttEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A downlink frame with a good FCS.
public struct ArgosDownlinkFrameEvent {
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
    public enum Coding: Int32 {
        case nrz = 0
        case invertedNrz = 1
        case nrzi = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The downlink coding in use.
    public var coding: Coding?
    /// The length of a frame without its FCS.
    public var bytes: Int64
    /// A frame's bytes.
    public var frame: [UInt8]

    init(_ c: sdrkit_argos_downlink_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        coding = Coding(rawValue: c.coding)
        bytes = c.bytes
        frame = [UInt8](UnsafeBufferPointer(start: c.frame.ptr, count: c.frame.len))
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArgosDownlinkFrameEvent, SDRKitCode> {
        var c = sdrkit_argos_downlink_frame_event()
        let code = sdrkit_argos_downlink_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArgosDownlinkFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message received whole with a good CRC. The source is the address, the text the data.
public struct HrdcpReportEvent {
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
    /// The address of the platform.
    public var source: String
    /// The data of the message.
    public var text: String
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The address word (8 hex digits).
    public var address: String
    /// The channel number of the plan.
    public var channel: Int64?
    /// The sequence counter.
    public var sequence: Int64
    /// Self-timed or alert.
    public var type: String
    /// The 10 health bits.
    public var health: String
    /// The compression of the data.
    public var compression: String
    /// Platform data bytes.
    public var bytes: Int64
    /// Reed-Solomon bytes corrected.
    public var rsCorrected: Int64
    /// The CRC passed.
    public var crcOk: Bool

    init(_ c: sdrkit_hrdcp_report_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        address = sdrkitText(c.address)
        channel = c.has_channel ? c.channel : nil
        sequence = c.sequence
        type = sdrkitText(c.type)
        health = sdrkitText(c.health)
        compression = sdrkitText(c.compression)
        bytes = c.bytes
        rsCorrected = c.rs_corrected
        crcOk = c.crc_ok
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HrdcpReportEvent, SDRKitCode> {
        var c = sdrkit_hrdcp_report_event()
        let code = sdrkit_hrdcp_report_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HrdcpReportEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A beacon decoded. The source is the hex identity. A distress beacon writes the urgency emergency, a test beacon none.
public struct SarsatBeaconEvent {
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
    /// The hex identity.
    public var source: String
    /// What the beacon says.
    public var text: String
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// How urgent the beacon is: emergency for a distress beacon.
    public var urgency: Urgency?
    /// The position that the beacon sends.
    public var position: SDRKitPosition?
    /// The beacon generation, 1 or 2.
    public var generation: Int64
    /// The beacon's hex identity (15 digits, or 23 for the second generation).
    public var hexId: String
    /// The beacon type (EPIRB, ELT, PLB ...).
    public var beaconType: String
    /// The country of the beacon's MID.
    public var country: String
    /// The country code (MID).
    public var mid: Int64
    /// The protocol in words.
    public var `protocol`: String
    /// The identification in words.
    public var identification: String
    /// The 121.5 MHz homing flag.
    public var homing: Bool
    /// The self-test or test-protocol flag.
    public var selfTest: Bool
    /// The bits corrected.
    public var bitsCorrected: Int64
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double
    /// The message bits as hexadecimal digits (as received after the repair).
    public var messageHex: String

    init(_ c: sdrkit_sarsat_beacon_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        generation = c.generation
        hexId = sdrkitText(c.hex_id)
        beaconType = sdrkitText(c.beacon_type)
        country = sdrkitText(c.country)
        mid = c.mid
        `protocol` = sdrkitText(c.protocol)
        identification = sdrkitText(c.identification)
        homing = c.homing
        selfTest = c.self_test
        bitsCorrected = c.bits_corrected
        offsetHz = c.offset_hz
        messageHex = sdrkitText(c.message_hex)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SarsatBeaconEvent, SDRKitCode> {
        var c = sdrkit_sarsat_beacon_event()
        let code = sdrkit_sarsat_beacon_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SarsatBeaconEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame with a good CRC. The source is the satellite overhead, the text the payload as printable text.
public struct LoraSatFrameEvent {
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
    /// The satellite overhead.
    public var source: String?
    /// The payload as printable text.
    public var text: String
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The spreading factor of a frame.
    public var sf: Int64
    /// The payload of a frame.
    public var payload: [UInt8]
    /// The carrier offset from the dial at the first data symbol.
    public var dopplerHz: Double
    /// The carrier rate during a frame.
    public var dopplerRateHzS: Double?

    init(_ c: sdrkit_lora_sat_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        sf = c.sf
        payload = [UInt8](UnsafeBufferPointer(start: c.payload.ptr, count: c.payload.len))
        dopplerHz = c.doppler_hz
        dopplerRateHzS = c.has_doppler_rate_hz_s ? c.doppler_rate_hz_s : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<LoraSatFrameEvent, SDRKitCode> {
        var c = sdrkit_lora_sat_frame_event()
        let code = sdrkit_lora_sat_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(LoraSatFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message joined from its parts. The source is the satellite ("satellite 12").
public struct OrbcommMessageEvent {
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
    /// The satellite ("satellite 12").
    public var source: String
    /// The text of the message.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The message number of a joined message.
    public var messageId: Int64
    /// Whether every part of a joined message came.
    public var complete: Bool

    init(_ c: sdrkit_orbcomm_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        messageId = c.message_id
        complete = c.complete
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<OrbcommMessageEvent, SDRKitCode> {
        var c = sdrkit_orbcomm_message_event()
        let code = sdrkit_orbcomm_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(OrbcommMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A signal was locked.
public struct PskDemodLockEvent {
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
    public enum Order: Int32 {
        case auto = 0
        case bpsk = 1
        case qpsk = 2
        case oqpsk = 3
        case n8psk = 4
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The order and the symbol rate.
    public var text: String
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The modulation order the demodulator runs.
    public var order: Order?

    init(_ c: sdrkit_psk_demod_lock_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        symbolRateHz = c.symbol_rate_hz
        order = c.has_order ? Order(rawValue: c.order) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PskDemodLockEvent, SDRKitCode> {
        var c = sdrkit_psk_demod_lock_event()
        let code = sdrkit_psk_demod_lock_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PskDemodLockEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Bits packed into bytes for the host. The text is the bytes in hexadecimal.
public struct PskDemodBytesEvent {
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
    /// The bytes in hexadecimal.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The bytes of a bytes event.
    public var bytes: [UInt8]

    init(_ c: sdrkit_psk_demod_bytes_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        bytes = [UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len))
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PskDemodBytesEvent, SDRKitCode> {
        var c = sdrkit_psk_demod_bytes_event()
        let code = sdrkit_psk_demod_bytes_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PskDemodBytesEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The beacons were locked.
public struct Qo100LockEvent {
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
    /// The LNB error at the lock.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The LNB's LO error (actual minus nominal).
    public var lnbErrorHz: Double
    /// The LNB error in parts per million of the nominal LO.
    public var lnbErrorPpm: Double

    init(_ c: sdrkit_qo100_lock_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        lnbErrorHz = c.lnb_error_hz
        lnbErrorPpm = c.lnb_error_ppm
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Qo100LockEvent, SDRKitCode> {
        var c = sdrkit_qo100_lock_event()
        let code = sdrkit_qo100_lock_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Qo100LockEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A conversation started. The channel is the conversation number.
public struct Qo100ConversationEvent {
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
    /// "conversation started".
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The channel.
    public var channel: String
    /// The conversation of a track.
    public var conversation: Int64
    /// The USB dial for listening (RF).
    public var dialHz: Double

    init(_ c: sdrkit_qo100_conversation_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        channel = sdrkitText(c.channel)
        conversation = c.conversation
        dialHz = c.dial_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Qo100ConversationEvent, SDRKitCode> {
        var c = sdrkit_qo100_conversation_event()
        let code = sdrkit_qo100_conversation_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Qo100ConversationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame. The source is the satellite, the text the text beacon or nothing.
public struct CubesatFrameEvent {
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
    public enum Framing: Int32 {
        case ax25Afsk = 0
        case ax25G3ruh = 1
        case ax25Nrzi = 2
        case ax100Mode5 = 3
        case ax100Mode6 = 4
        case ccsdsRs = 5
        case geoscan = 6
        case lucky7 = 7
        case spino = 8
        case erminaz = 9
        case usp = 10
        case ngham = 11
        case u482c = 12
        case mobitex = 13
        case mobitexNx = 14
        case beesat1 = 15
        case beesat9 = 16
        case ccsdsConcat = 17
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The satellite.
    public var source: String?
    /// The text beacon.
    public var text: String?
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The framing of a frame.
    public var framing: Framing?
    /// Its NORAD number.
    public var norad: Int64
    /// The frame's data.
    public var data: [UInt8]

    init(_ c: sdrkit_cubesat_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        family = Family(rawValue: c.family)
        framing = Framing(rawValue: c.framing)
        norad = c.norad
        data = [UInt8](UnsafeBufferPointer(start: c.data.ptr, count: c.data.len))
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CubesatFrameEvent, SDRKitCode> {
        var c = sdrkit_cubesat_frame_event()
        let code = sdrkit_cubesat_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CubesatFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame in KISS form for a packet program. The source is the satellite.
public struct CubesatKissEvent {
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
    /// The satellite.
    public var source: String?
    /// The kind of thing that the event is about.
    public var family: Family?
    /// The KISS frame.
    public var kissFrame: [UInt8]

    init(_ c: sdrkit_cubesat_kiss_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
        kissFrame = [UInt8](UnsafeBufferPointer(start: c.kiss_frame.ptr, count: c.kiss_frame.len))
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CubesatKissEvent, SDRKitCode> {
        var c = sdrkit_cubesat_kiss_event()
        let code = sdrkit_cubesat_kiss_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CubesatKissEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of an AX.25 transmission.
public struct CubesatTxEvent {
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
    /// The state: started, queued, finished or refused.
    public var state: String
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The destination and the digipeaters of the frames, separated by commas.
    public var target: String
    /// The text or the hexadecimal bytes of the frame for started, queued and refused; empty for finished.
    public var text: String
    /// The family of the source: a satellite.
    public var family: Family?

    init(_ c: sdrkit_cubesat_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CubesatTxEvent, SDRKitCode> {
        var c = sdrkit_cubesat_tx_event()
        let code = sdrkit_cubesat_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CubesatTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A change of the state of the receiver.
public struct FleetBroadcastStateEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The state: 0 nothing, 1 carrier, 2 bits, 3 framed.
    public var state: Int64

    init(_ c: sdrkit_fleet_broadcast_state_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        state = c.state
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FleetBroadcastStateEvent, SDRKitCode> {
        var c = sdrkit_fleet_broadcast_state_event()
        let code = sdrkit_fleet_broadcast_state_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FleetBroadcastStateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A line of decoded signalling.
public struct BganSignallingEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_bgan_signalling_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BganSignallingEvent, SDRKitCode> {
        var c = sdrkit_bgan_signalling_event()
        let code = sdrkit_bgan_signalling_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BganSignallingEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A finished carrier line.
public struct SatSurveyCarrierEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The number of a carrier line.
    public var line: Int64
    /// Offset from the centre of the samples.
    public var offsetHz: Double
    /// The frequency, MHz.
    public var mhz: Double
    /// The level over the background or floor.
    public var snrDb: Double
    /// The length of a line.
    public var durationS: Double
    /// The drift of a line.
    public var driftHzS: Double?
    /// The channel name of a line.
    public var channel: String
    /// The satellite that the Doppler curve names.
    public var satellite: String
    /// Its catalogue number.
    public var norad: Int64?
    /// The rms of the Doppler match.
    public var rmsHz: Double?
    /// Another satellite fits about as well.
    public var ambiguous: Bool

    init(_ c: sdrkit_sat_survey_carrier_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        line = c.line
        offsetHz = c.offset_hz
        mhz = c.mhz
        snrDb = c.snr_db
        durationS = c.duration_s
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        channel = sdrkitText(c.channel)
        satellite = sdrkitText(c.satellite)
        norad = c.has_norad ? c.norad : nil
        rmsHz = c.has_rms_hz ? c.rms_hz : nil
        ambiguous = c.ambiguous
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SatSurveyCarrierEvent, SDRKitCode> {
        var c = sdrkit_sat_survey_carrier_event()
        let code = sdrkit_sat_survey_carrier_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SatSurveyCarrierEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "inmarsatc" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct InmarsatcDropped {
    /// The items that the component "demod" dropped.
    public var demod: Int64?

    init(_ c: sdrkit_inmarsatc_dropped) {
        demod = c.has_demod ? c.demod : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<InmarsatcDropped, SDRKitCode> {
        var c = sdrkit_inmarsatc_dropped()
        let code = sdrkit_inmarsatc_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(InmarsatcDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "aero" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct AeroDropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?
    /// The items that the component "su" dropped.
    public var su: Int64?

    init(_ c: sdrkit_aero_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
        su = c.has_su ? c.su : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AeroDropped, SDRKitCode> {
        var c = sdrkit_aero_dropped()
        let code = sdrkit_aero_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(AeroDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "inmarsat" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct InmarsatDropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?
    /// The items that the component "aero_display" dropped.
    public var aeroDisplay: Int64?
    /// The items that the component "egc" dropped.
    public var egc: Int64?

    init(_ c: sdrkit_inmarsat_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
        aeroDisplay = c.has_aero_display ? c.aero_display : nil
        egc = c.has_egc ? c.egc : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<InmarsatDropped, SDRKitCode> {
        var c = sdrkit_inmarsat_dropped()
        let code = sdrkit_inmarsat_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(InmarsatDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "globalstar" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct GlobalstarDropped {
    /// The items that the component "channels" dropped.
    public var channels: Int64?

    init(_ c: sdrkit_globalstar_dropped) {
        channels = c.has_channels ? c.channels : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GlobalstarDropped, SDRKitCode> {
        var c = sdrkit_globalstar_dropped()
        let code = sdrkit_globalstar_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(GlobalstarDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "goes_dcs" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct GoesDcsDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_goes_dcs_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GoesDcsDropped, SDRKitCode> {
        var c = sdrkit_goes_dcs_dropped()
        let code = sdrkit_goes_dcs_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(GoesDcsDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "argos" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct ArgosDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_argos_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ArgosDropped, SDRKitCode> {
        var c = sdrkit_argos_dropped()
        let code = sdrkit_argos_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(ArgosDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "hrdcp" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct HrdcpDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_hrdcp_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HrdcpDropped, SDRKitCode> {
        var c = sdrkit_hrdcp_dropped()
        let code = sdrkit_hrdcp_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(HrdcpDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "sarsat" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct SarsatDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_sarsat_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SarsatDropped, SDRKitCode> {
        var c = sdrkit_sarsat_dropped()
        let code = sdrkit_sarsat_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(SarsatDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "lora_sat" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct LoraSatDropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?

    init(_ c: sdrkit_lora_sat_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LoraSatDropped, SDRKitCode> {
        var c = sdrkit_lora_sat_dropped()
        let code = sdrkit_lora_sat_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraSatDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "orbcomm" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct OrbcommDropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?

    init(_ c: sdrkit_orbcomm_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<OrbcommDropped, SDRKitCode> {
        var c = sdrkit_orbcomm_dropped()
        let code = sdrkit_orbcomm_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(OrbcommDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "psk_demod" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct PskDemodDropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?
    /// The items that the component "bits" dropped.
    public var bits: Int64?

    init(_ c: sdrkit_psk_demod_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
        bits = c.has_bits ? c.bits : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PskDemodDropped, SDRKitCode> {
        var c = sdrkit_psk_demod_dropped()
        let code = sdrkit_psk_demod_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(PskDemodDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "qo100" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Qo100Dropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?

    init(_ c: sdrkit_qo100_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Qo100Dropped, SDRKitCode> {
        var c = sdrkit_qo100_dropped()
        let code = sdrkit_qo100_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Qo100Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "cubesat" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct CubesatDropped {
    /// The items that the component "receiver" dropped.
    public var receiver: Int64?
    /// The items that the component "naming" dropped.
    public var naming: Int64?

    init(_ c: sdrkit_cubesat_dropped) {
        receiver = c.has_receiver ? c.receiver : nil
        naming = c.has_naming ? c.naming : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CubesatDropped, SDRKitCode> {
        var c = sdrkit_cubesat_dropped()
        let code = sdrkit_cubesat_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(CubesatDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum SatcomComponents {
    /// The result fields of the component "aero_display" of the stack "inmarsat" in the snapshot of `handle`.
    public static func inmarsatAeroDisplayAeroFields(_ handle: SDRKIT_HANDLE) -> Result<AeroFields, SDRKitCode> {
        var c = sdrkit_aero_fields()
        let code = sdrkit_inmarsat_aero_display_aero_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AeroFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "egc" of the stack "inmarsat" in the snapshot of `handle`.
    public static func inmarsatEgcInmarsatcFields(_ handle: SDRKIT_HANDLE) -> Result<InmarsatcFields, SDRKitCode> {
        var c = sdrkit_inmarsatc_fields()
        let code = sdrkit_inmarsat_egc_inmarsatc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(InmarsatcFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum SatcomTables {
    /// The changed rows of the table "messages" of the stack "inmarsatc", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `text`. Time: `first_heard`.
    public static func inmarsatcMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[InmarsatcMessageRow], SDRKitCode> {
        var rows = [sdrkit_inmarsatc_message_row](repeating: sdrkit_inmarsatc_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsatc_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatcMessageRow.init))
    }
    /// The changed rows of the table "channels" of the stack "inmarsatc", newest first, at most `capacity` rows.
    public static func inmarsatcChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[InmarsatcChannelRow], SDRKitCode> {
        var rows = [sdrkit_inmarsatc_channel_row](repeating: sdrkit_inmarsatc_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsatc_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatcChannelRow.init))
    }
    /// The changed rows of the table "packet_counts" of the stack "inmarsatc", newest first, at most `capacity` rows.
    public static func inmarsatcPacketCounts(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[InmarsatcPacketCountRow], SDRKitCode> {
        var rows = [sdrkit_inmarsatc_packet_count_row](repeating: sdrkit_inmarsatc_packet_count_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsatc_packet_counts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatcPacketCountRow.init))
    }
    /// The changed rows of the table "channels" of the stack "aero", newest first, at most `capacity` rows.
    public static func aeroChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 12) -> Result<[AeroChannelRow], SDRKitCode> {
        var rows = [sdrkit_aero_channel_row](repeating: sdrkit_aero_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aero_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroChannelRow.init))
    }
    /// The changed rows of the table "su_types" of the stack "aero", newest first, at most `capacity` rows.
    public static func aeroSuTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[AeroSuTypeRow], SDRKitCode> {
        var rows = [sdrkit_aero_su_type_row](repeating: sdrkit_aero_su_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aero_su_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroSuTypeRow.init))
    }
    /// The changed rows of the table "logons" of the stack "aero", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time`.
    public static func aeroLogons(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AeroLogonRow], SDRKitCode> {
        var rows = [sdrkit_aero_logon_row](repeating: sdrkit_aero_logon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aero_logons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroLogonRow.init))
    }
    /// The changed rows of the table "messages" of the stack "aero", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `flight` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_ft`.
    /// Sender: `aes`. Addressee: `ges`. Text: `text`. Time: `time`.
    public static func aeroMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AeroMessageRow], SDRKitCode> {
        var rows = [sdrkit_aero_message_row](repeating: sdrkit_aero_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aero_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroMessageRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "aero", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `flight` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.
    /// Time: `last_heard`.
    public static func aeroAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AeroAircraftRow], SDRKitCode> {
        var rows = [sdrkit_aero_aircraft_row](repeating: sdrkit_aero_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aero_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroAircraftRow.init))
    }
    /// The changed rows of the table "facts" of the stack "aero", newest first, at most `capacity` rows.
    public static func aeroFacts(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[AeroFactRow], SDRKitCode> {
        var rows = [sdrkit_aero_fact_row](repeating: sdrkit_aero_fact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aero_facts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroFactRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "inmarsat", newest first, at most `capacity` rows.
    /// The column `satellite` names a row.
    /// Default order: the column `snr_db`, highest first.
    public static func inmarsatCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[InmarsatCarrierRow], SDRKitCode> {
        var rows = [sdrkit_inmarsat_carrier_row](repeating: sdrkit_inmarsat_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatCarrierRow.init))
    }
    /// The changed rows of the table "candidates" of the stack "inmarsat", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func inmarsatCandidates(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[InmarsatCandidateRow], SDRKitCode> {
        var rows = [sdrkit_inmarsat_candidate_row](repeating: sdrkit_inmarsat_candidate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_candidates_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatCandidateRow.init))
    }
    /// The changed rows of the table "candidate_reasons" of the stack "inmarsat", newest first, at most `capacity` rows.
    public static func inmarsatCandidateReasons(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[InmarsatCandidateReasonRow], SDRKitCode> {
        var rows = [sdrkit_inmarsat_candidate_reason_row](repeating: sdrkit_inmarsat_candidate_reason_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_candidate_reasons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatCandidateReasonRow.init))
    }
    /// The changed rows of the table "aero_display.channels" of the stack "inmarsat", newest first, at most `capacity` rows.
    public static func inmarsatAeroDisplayChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 12) -> Result<[AeroChannelRow], SDRKitCode> {
        var rows = [sdrkit_aero_channel_row](repeating: sdrkit_aero_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_aero_display_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroChannelRow.init))
    }
    /// The changed rows of the table "aero_display.su_types" of the stack "inmarsat", newest first, at most `capacity` rows.
    public static func inmarsatAeroDisplaySuTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[AeroSuTypeRow], SDRKitCode> {
        var rows = [sdrkit_aero_su_type_row](repeating: sdrkit_aero_su_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_aero_display_su_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroSuTypeRow.init))
    }
    /// The changed rows of the table "aero_display.logons" of the stack "inmarsat", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time`.
    public static func inmarsatAeroDisplayLogons(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AeroLogonRow], SDRKitCode> {
        var rows = [sdrkit_aero_logon_row](repeating: sdrkit_aero_logon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_aero_display_logons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroLogonRow.init))
    }
    /// The changed rows of the table "aero_display.messages" of the stack "inmarsat", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `flight` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_ft`.
    /// Sender: `aes`. Addressee: `ges`. Text: `text`. Time: `time`.
    public static func inmarsatAeroDisplayMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AeroMessageRow], SDRKitCode> {
        var rows = [sdrkit_aero_message_row](repeating: sdrkit_aero_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_aero_display_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroMessageRow.init))
    }
    /// The changed rows of the table "aero_display.aircraft" of the stack "inmarsat", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `flight` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.
    /// Time: `last_heard`.
    public static func inmarsatAeroDisplayAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AeroAircraftRow], SDRKitCode> {
        var rows = [sdrkit_aero_aircraft_row](repeating: sdrkit_aero_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_aero_display_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroAircraftRow.init))
    }
    /// The changed rows of the table "aero_display.facts" of the stack "inmarsat", newest first, at most `capacity` rows.
    public static func inmarsatAeroDisplayFacts(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[AeroFactRow], SDRKitCode> {
        var rows = [sdrkit_aero_fact_row](repeating: sdrkit_aero_fact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_aero_display_facts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AeroFactRow.init))
    }
    /// The changed rows of the table "egc.messages" of the stack "inmarsat", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `text`. Time: `first_heard`.
    public static func inmarsatEgcMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[InmarsatcMessageRow], SDRKitCode> {
        var rows = [sdrkit_inmarsatc_message_row](repeating: sdrkit_inmarsatc_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_egc_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatcMessageRow.init))
    }
    /// The changed rows of the table "egc.channels" of the stack "inmarsat", newest first, at most `capacity` rows.
    public static func inmarsatEgcChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[InmarsatcChannelRow], SDRKitCode> {
        var rows = [sdrkit_inmarsatc_channel_row](repeating: sdrkit_inmarsatc_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_egc_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatcChannelRow.init))
    }
    /// The changed rows of the table "egc.packet_counts" of the stack "inmarsat", newest first, at most `capacity` rows.
    public static func inmarsatEgcPacketCounts(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[InmarsatcPacketCountRow], SDRKitCode> {
        var rows = [sdrkit_inmarsatc_packet_count_row](repeating: sdrkit_inmarsatc_packet_count_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_inmarsat_egc_packet_counts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InmarsatcPacketCountRow.init))
    }
    /// The changed rows of the table "raw" of the stack "iridium", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `line`. Time: `at`.
    public static func iridiumRaw(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[IridiumRawRow], SDRKitCode> {
        var rows = [sdrkit_iridium_raw_row](repeating: sdrkit_iridium_raw_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_iridium_raw_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IridiumRawRow.init))
    }
    /// The changed rows of the table "channels" of the stack "globalstar", newest first, at most `capacity` rows.
    public static func globalstarChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 13) -> Result<[GlobalstarChannelRow], SDRKitCode> {
        var rows = [sdrkit_globalstar_channel_row](repeating: sdrkit_globalstar_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_globalstar_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GlobalstarChannelRow.init))
    }
    /// The changed rows of the table "activity" of the stack "globalstar", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `wall_start`.
    public static func globalstarActivity(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[GlobalstarActivityRow], SDRKitCode> {
        var rows = [sdrkit_globalstar_activity_row](repeating: sdrkit_globalstar_activity_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_globalstar_activity_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GlobalstarActivityRow.init))
    }
    /// The changed rows of the table "reports" of the stack "goes_dcs", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `address` names a row.
    /// Time: `heard_at`.
    public static func goesDcsReports(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[GoesDcsReportRow], SDRKitCode> {
        var rows = [sdrkit_goes_dcs_report_row](repeating: sdrkit_goes_dcs_report_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_dcs_reports_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesDcsReportRow.init))
    }
    /// The changed rows of the table "platforms" of the stack "goes_dcs", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func goesDcsPlatforms(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[GoesDcsPlatformRow], SDRKitCode> {
        var rows = [sdrkit_goes_dcs_platform_row](repeating: sdrkit_goes_dcs_platform_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_dcs_platforms_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesDcsPlatformRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "goes_dcs", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func goesDcsCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[GoesDcsCarrierRow], SDRKitCode> {
        var rows = [sdrkit_goes_dcs_carrier_row](repeating: sdrkit_goes_dcs_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_dcs_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesDcsCarrierRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "argos", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `platform` names a row.
    /// Time: `heard_at`.
    public static func argosBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[ArgosBurstRow], SDRKitCode> {
        var rows = [sdrkit_argos_burst_row](repeating: sdrkit_argos_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_argos_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ArgosBurstRow.init))
    }
    /// The changed rows of the table "platforms" of the stack "argos", newest first, at most `capacity` rows.
    /// The column `platform` names a row.
    public static func argosPlatforms(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[ArgosPlatformRow], SDRKitCode> {
        var rows = [sdrkit_argos_platform_row](repeating: sdrkit_argos_platform_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_argos_platforms_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ArgosPlatformRow.init))
    }
    /// The changed rows of the table "frames" of the stack "argos", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `at`.
    public static func argosFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[ArgosFrameRow], SDRKitCode> {
        var rows = [sdrkit_argos_frame_row](repeating: sdrkit_argos_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_argos_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ArgosFrameRow.init))
    }
    /// The changed rows of the table "reports" of the stack "hrdcp", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `address` names a row.
    /// Time: `heard_at`.
    public static func hrdcpReports(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[HrdcpReportRow], SDRKitCode> {
        var rows = [sdrkit_hrdcp_report_row](repeating: sdrkit_hrdcp_report_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hrdcp_reports_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HrdcpReportRow.init))
    }
    /// The changed rows of the table "platforms" of the stack "hrdcp", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func hrdcpPlatforms(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[HrdcpPlatformRow], SDRKitCode> {
        var rows = [sdrkit_hrdcp_platform_row](repeating: sdrkit_hrdcp_platform_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hrdcp_platforms_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HrdcpPlatformRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "hrdcp", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func hrdcpCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[HrdcpCarrierRow], SDRKitCode> {
        var rows = [sdrkit_hrdcp_carrier_row](repeating: sdrkit_hrdcp_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hrdcp_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HrdcpCarrierRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "sarsat", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `hex_id` names a row.
    /// Place of a row: the column `position`.
    /// Time: `heard_at`.
    public static func sarsatBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[SarsatBurstRow], SDRKitCode> {
        var rows = [sdrkit_sarsat_burst_row](repeating: sdrkit_sarsat_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sarsat_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SarsatBurstRow.init))
    }
    /// The changed rows of the table "beacons" of the stack "sarsat", newest first, at most `capacity` rows.
    /// The column `hex_id` names a row.
    /// Place of a row: the column `position`.
    public static func sarsatBeacons(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[SarsatBeaconRow], SDRKitCode> {
        var rows = [sdrkit_sarsat_beacon_row](repeating: sdrkit_sarsat_beacon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sarsat_beacons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SarsatBeaconRow.init))
    }
    /// The changed rows of the table "frames" of the stack "lora_sat", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `satellite` names a row.
    /// Time: `heard_at`.
    public static func loraSatFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[LoraSatFrameRow], SDRKitCode> {
        var rows = [sdrkit_lora_sat_frame_row](repeating: sdrkit_lora_sat_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lora_sat_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LoraSatFrameRow.init))
    }
    /// The changed rows of the table "demodulators" of the stack "lora_sat", newest first, at most `capacity` rows.
    public static func loraSatDemodulators(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[LoraSatDemodulatorRow], SDRKitCode> {
        var rows = [sdrkit_lora_sat_demodulator_row](repeating: sdrkit_lora_sat_demodulator_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lora_sat_demodulators_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LoraSatDemodulatorRow.init))
    }
    /// The changed rows of the table "demodulator_syncs" of the stack "lora_sat", newest first, at most `capacity` rows.
    public static func loraSatDemodulatorSyncs(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[LoraSatDemodulatorSyncRow], SDRKitCode> {
        var rows = [sdrkit_lora_sat_demodulator_sync_row](repeating: sdrkit_lora_sat_demodulator_sync_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lora_sat_demodulator_syncs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LoraSatDemodulatorSyncRow.init))
    }
    /// The changed rows of the table "frames_per_sf" of the stack "lora_sat", newest first, at most `capacity` rows.
    public static func loraSatFramesPerSf(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[LoraSatSfRow], SDRKitCode> {
        var rows = [sdrkit_lora_sat_sf_row](repeating: sdrkit_lora_sat_sf_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lora_sat_frames_per_sf_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LoraSatSfRow.init))
    }
    /// The changed rows of the table "satellites" of the stack "orbcomm", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    /// Place of a row: the column `position`.
    /// Height: `height_m`.
    public static func orbcommSatellites(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[OrbcommSatelliteRow], SDRKitCode> {
        var rows = [sdrkit_orbcomm_satellite_row](repeating: sdrkit_orbcomm_satellite_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_orbcomm_satellites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OrbcommSatelliteRow.init))
    }
    /// The changed rows of the table "satellite_channels" of the stack "orbcomm", newest first, at most `capacity` rows.
    public static func orbcommSatelliteChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[OrbcommSatelliteChannelRow], SDRKitCode> {
        var rows = [sdrkit_orbcomm_satellite_channel_row](repeating: sdrkit_orbcomm_satellite_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_orbcomm_satellite_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OrbcommSatelliteChannelRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "orbcomm", newest first, at most `capacity` rows.
    public static func orbcommDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 2) -> Result<[OrbcommDecoderRow], SDRKitCode> {
        var rows = [sdrkit_orbcomm_decoder_row](repeating: sdrkit_orbcomm_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_orbcomm_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OrbcommDecoderRow.init))
    }
    /// The changed rows of the table "packets" of the stack "orbcomm", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `wall`.
    public static func orbcommPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[OrbcommPacketRow], SDRKitCode> {
        var rows = [sdrkit_orbcomm_packet_row](repeating: sdrkit_orbcomm_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_orbcomm_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OrbcommPacketRow.init))
    }
    /// The changed rows of the table "packet_types" of the stack "orbcomm", newest first, at most `capacity` rows.
    public static func orbcommPacketTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 9) -> Result<[OrbcommPacketTypeRow], SDRKitCode> {
        var rows = [sdrkit_orbcomm_packet_type_row](repeating: sdrkit_orbcomm_packet_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_orbcomm_packet_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OrbcommPacketTypeRow.init))
    }
    /// The changed rows of the table "rates" of the stack "psk_demod", newest first, at most `capacity` rows.
    public static func pskDemodRates(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[PskDemodRateRow], SDRKitCode> {
        var rows = [sdrkit_psk_demod_rate_row](repeating: sdrkit_psk_demod_rate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_psk_demod_rates_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PskDemodRateRow.init))
    }
    /// The changed rows of the table "trials" of the stack "psk_demod", newest first, at most `capacity` rows.
    public static func pskDemodTrials(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[PskDemodTrialRow], SDRKitCode> {
        var rows = [sdrkit_psk_demod_trial_row](repeating: sdrkit_psk_demod_trial_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_psk_demod_trials_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PskDemodTrialRow.init))
    }
    /// The changed rows of the table "syncs" of the stack "psk_demod", newest first, at most `capacity` rows.
    public static func pskDemodSyncs(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[PskDemodSyncRow], SDRKitCode> {
        var rows = [sdrkit_psk_demod_sync_row](repeating: sdrkit_psk_demod_sync_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_psk_demod_syncs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PskDemodSyncRow.init))
    }
    /// The changed rows of the table "log" of the stack "psk_demod", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `at`.
    public static func pskDemodLog(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[PskDemodLogRow], SDRKitCode> {
        var rows = [sdrkit_psk_demod_log_row](repeating: sdrkit_psk_demod_log_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_psk_demod_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PskDemodLogRow.init))
    }
    /// The changed rows of the table "beacons" of the stack "qo100", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func qo100Beacons(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[Qo100BeaconRow], SDRKitCode> {
        var rows = [sdrkit_qo100_beacon_row](repeating: sdrkit_qo100_beacon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qo100_beacons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Qo100BeaconRow.init))
    }
    /// The changed rows of the table "signals" of the stack "qo100", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func qo100Signals(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Qo100SignalRow], SDRKitCode> {
        var rows = [sdrkit_qo100_signal_row](repeating: sdrkit_qo100_signal_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qo100_signals_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Qo100SignalRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "qo100", newest first, at most `capacity` rows.
    public static func qo100Tracks(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Qo100TrackRow], SDRKitCode> {
        var rows = [sdrkit_qo100_track_row](repeating: sdrkit_qo100_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qo100_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Qo100TrackRow.init))
    }
    /// The changed rows of the table "conversations" of the stack "qo100", newest first, at most `capacity` rows.
    public static func qo100Conversations(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[Qo100ConversationRow], SDRKitCode> {
        var rows = [sdrkit_qo100_conversation_row](repeating: sdrkit_qo100_conversation_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qo100_conversations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Qo100ConversationRow.init))
    }
    /// The changed rows of the table "conversation_callsigns" of the stack "qo100", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func qo100ConversationCallsigns(_ handle: SDRKIT_HANDLE, capacity: Int = 480) -> Result<[Qo100CallsignRow], SDRKitCode> {
        var rows = [sdrkit_qo100_callsign_row](repeating: sdrkit_qo100_callsign_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qo100_conversation_callsigns_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Qo100CallsignRow.init))
    }
    /// The changed rows of the table "datv" of the stack "qo100", newest first, at most `capacity` rows.
    public static func qo100Datv(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Qo100DatvCarrierRow], SDRKitCode> {
        var rows = [sdrkit_qo100_datv_carrier_row](repeating: sdrkit_qo100_datv_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qo100_datv_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Qo100DatvCarrierRow.init))
    }
    /// The changed rows of the table "frames" of the stack "cubesat", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `satellite` names a row.
    /// Time: `heard_at`.
    public static func cubesatFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[CubesatFrameRow], SDRKitCode> {
        var rows = [sdrkit_cubesat_frame_row](repeating: sdrkit_cubesat_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cubesat_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CubesatFrameRow.init))
    }
    /// The changed rows of the table "frames_per_framing" of the stack "cubesat", newest first, at most `capacity` rows.
    public static func cubesatFramesPerFraming(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[CubesatFramingRow], SDRKitCode> {
        var rows = [sdrkit_cubesat_framing_row](repeating: sdrkit_cubesat_framing_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cubesat_frames_per_framing_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CubesatFramingRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "cubesat", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func cubesatCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[CubesatCarrierRow], SDRKitCode> {
        var rows = [sdrkit_cubesat_carrier_row](repeating: sdrkit_cubesat_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cubesat_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CubesatCarrierRow.init))
    }
    /// The changed rows of the table "channels" of the stack "fleet_broadcast", newest first, at most `capacity` rows.
    public static func fleetBroadcastChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[FleetBroadcastChannelRow], SDRKitCode> {
        var rows = [sdrkit_fleet_broadcast_channel_row](repeating: sdrkit_fleet_broadcast_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fleet_broadcast_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FleetBroadcastChannelRow.init))
    }
    /// The changed rows of the table "frame_log" of the stack "fleet_broadcast", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func fleetBroadcastFrameLog(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[FleetBroadcastFrameRow], SDRKitCode> {
        var rows = [sdrkit_fleet_broadcast_frame_row](repeating: sdrkit_fleet_broadcast_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fleet_broadcast_frame_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FleetBroadcastFrameRow.init))
    }
    /// The changed rows of the table "carrier_table" of the stack "bgan", newest first, at most `capacity` rows.
    /// The column `satellite` names a row.
    /// Default order: the column `snr_db`, highest first.
    public static func bganCarrierTable(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[BganCarrierRow], SDRKitCode> {
        var rows = [sdrkit_bgan_carrier_row](repeating: sdrkit_bgan_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_bgan_carrier_table_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BganCarrierRow.init))
    }
    /// The changed rows of the table "beam_table" of the stack "bgan", newest first, at most `capacity` rows.
    public static func bganBeamTable(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BganBeamRow], SDRKitCode> {
        var rows = [sdrkit_bgan_beam_row](repeating: sdrkit_bgan_beam_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_bgan_beam_table_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BganBeamRow.init))
    }
    /// The changed rows of the table "log" of the stack "bgan", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `at`.
    public static func bganLog(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[BganLogRow], SDRKitCode> {
        var rows = [sdrkit_bgan_log_row](repeating: sdrkit_bgan_log_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_bgan_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BganLogRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "sat_survey", newest first, at most `capacity` rows.
    /// The column `satellite` names a row.
    /// Default order: the column `snr_db`, highest first.
    public static func satSurveyCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatSurveyCarrierRow], SDRKitCode> {
        var rows = [sdrkit_sat_survey_carrier_row](repeating: sdrkit_sat_survey_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_survey_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatSurveyCarrierRow.init))
    }
    /// The changed rows of the table "channels" of the stack "sat_survey", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func satSurveyChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[SatSurveyChannelRow], SDRKitCode> {
        var rows = [sdrkit_sat_survey_channel_row](repeating: sdrkit_sat_survey_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_survey_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatSurveyChannelRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum SatcomCommands {
    /// Clear. Forgets everything decoded so far.
    public static func inmarsatcClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_inmarsatc_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func aeroClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_aero_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func aeroRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_aero_aircraft_args()
        args.size = UInt32(MemoryLayout<sdrkit_aero_aircraft_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_aero_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func inmarsatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_inmarsat_clear(handle)
        return sdrkitResult(code)
    }
    /// Focus on carrier. Never parks the window of this carrier. An empty row ends the focus.
    public static func inmarsatCarrierFocus(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_inmarsat_carrier_args()
        args.size = UInt32(MemoryLayout<sdrkit_inmarsat_carrier_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_inmarsat_carrier_focus(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func iridiumClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_iridium_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func globalstarClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_globalstar_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func goesDcsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_goes_dcs_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func goesDcsRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_goes_dcs_platform_args()
        args.size = UInt32(MemoryLayout<sdrkit_goes_dcs_platform_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_goes_dcs_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func argosClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_argos_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func argosRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_argos_platform_args()
        args.size = UInt32(MemoryLayout<sdrkit_argos_platform_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_argos_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func hrdcpClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hrdcp_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func sarsatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sarsat_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func sarsatRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_sarsat_beacon_args()
        args.size = UInt32(MemoryLayout<sdrkit_sarsat_beacon_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_sarsat_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Apply preset. Asks the host to tune the dial to the preset.
    public static func sarsatPresetApply(_ handle: SDRKIT_HANDLE, preset: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_sarsat_preset_args()
        args.size = UInt32(MemoryLayout<sdrkit_sarsat_preset_args>.size)
        let code: Int32 = { () -> Int32 in
        args.has_preset = preset != nil
        if let given = preset { args.preset = given }
        return sdrkit_sarsat_preset_apply(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func loraSatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lora_sat_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func orbcommClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_orbcomm_clear(handle)
        return sdrkitResult(code)
    }
    /// Tune to channel. Asks the host to tune the dial to the channel.
    public static func orbcommPresetApply(_ handle: SDRKIT_HANDLE, channel: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_orbcomm_channel_args()
        args.size = UInt32(MemoryLayout<sdrkit_orbcomm_channel_args>.size)
        let code: Int32 = { () -> Int32 in
        args.has_channel = channel != nil
        if let given = channel { args.channel = given }
        return sdrkit_orbcomm_preset_apply(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func orbcommRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_orbcomm_satellite_args()
        args.size = UInt32(MemoryLayout<sdrkit_orbcomm_satellite_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_orbcomm_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func pskDemodClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_psk_demod_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func qo100Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qo100_clear(handle)
        return sdrkitResult(code)
    }
    /// Speech-to-text. Gives the text that the host's speech-to-text heard. Callsigns are read from it.
    public static func qo100SpeechText(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_qo100_speech_args()
        args.size = UInt32(MemoryLayout<sdrkit_qo100_speech_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_qo100_speech_text(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Listen to conversation. Makes the dial follow a conversation. An empty row returns to the dial.
    public static func qo100ConversationListen(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_qo100_listen_args()
        args.size = UInt32(MemoryLayout<sdrkit_qo100_listen_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_qo100_conversation_listen(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func qo100Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_qo100_conversation_args()
        args.size = UInt32(MemoryLayout<sdrkit_qo100_conversation_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_qo100_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Push to talk. With on true, sends the speech of the microphone input in upper sideband on the uplink of the dial until ptt with on false or the time-out. Transmit must be on. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
    public static func qo100Ptt(_ handle: SDRKIT_HANDLE, on: Bool) -> Result<Void, SDRKitCode> {
        var args = sdrkit_qo100_ptt_args()
        args.size = UInt32(MemoryLayout<sdrkit_qo100_ptt_args>.size)
        let code: Int32 = { () -> Int32 in
        args.on = on
        return sdrkit_qo100_ptt(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cubesatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cubesat_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a frame. Builds a UI frame from the station callsign to the destination and queues it. A transmission starts when none is on air; frames sent during a transmission go in the next one. Transmit must be on and the station callsign set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func cubesatSend(_ handle: SDRKIT_HANDLE, destination: String? = nil, path: String? = nil, text: String? = nil, hex: String? = nil, pid: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cubesat_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_cubesat_send_args>.size)
        let code: Int32 = sdrkitWithString(destination ?? "") { destination_ in sdrkitWithString(path ?? "") { path_ in sdrkitWithString(text ?? "") { text_ in sdrkitWithString(hex ?? "") { hex_ in { () -> Int32 in
        args.has_destination = destination != nil
        args.destination = destination_
        args.has_path = path != nil
        args.path = path_
        args.has_text = text != nil
        args.text = text_
        args.has_hex = hex != nil
        args.hex = hex_
        args.has_pid = pid != nil
        if let given = pid { args.pid = given }
        return sdrkit_cubesat_send(handle, &args)
        }() } } } }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the transmission after the current bit with a 5 ms fall.
    public static func cubesatHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cubesat_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fleetBroadcastClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fleet_broadcast_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func bganClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_bgan_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func satSurveyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sat_survey_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum SatcomSettings {
    /// Channel search.
    public static func inmarsatcChannelSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "channel_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcChannelSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "channel_search", value))
    }
    /// Search span.
    public static func inmarsatcSearchSpanHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_span_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcSearchSpanHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_span_hz", value))
    }
    /// The options of the setting "satellite" of the stack "inmarsatc".
    public enum InmarsatcSatellite: Int32 {
        case none = 0
        case alphasat = 1
        case i4f3 = 2
        case i3f5 = 3
        case i4f1 = 4
    }
    /// Satellite.
    public static func inmarsatcSatellite(_ handle: SDRKIT_HANDLE) -> Result<InmarsatcSatellite, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "satellite", &v)
        return code == SDRKIT_OK ? .success(InmarsatcSatellite(rawValue: v) ?? InmarsatcSatellite(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcSatellite(_ handle: SDRKIT_HANDLE, _ value: InmarsatcSatellite) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "satellite", value.rawValue))
    }
    /// The options of the setting "min_priority" of the stack "inmarsatc".
    public enum InmarsatcMinPriority: Int32 {
        case all = 0
        case safetyAndHigher = 1
        case urgencyAndHigher = 2
        case distress = 3
    }
    /// Priority.
    public static func inmarsatcMinPriority(_ handle: SDRKIT_HANDLE) -> Result<InmarsatcMinPriority, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "min_priority", &v)
        return code == SDRKIT_OK ? .success(InmarsatcMinPriority(rawValue: v) ?? InmarsatcMinPriority(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcMinPriority(_ handle: SDRKIT_HANDLE, _ value: InmarsatcMinPriority) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "min_priority", value.rawValue))
    }
    /// The options of the setting "service_filter" of the stack "inmarsatc".
    public enum InmarsatcServiceFilter: Int32 {
        case all = 0
        case safetynet = 1
        case fleetnet = 2
    }
    /// Service.
    public static func inmarsatcServiceFilter(_ handle: SDRKIT_HANDLE) -> Result<InmarsatcServiceFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "service_filter", &v)
        return code == SDRKIT_OK ? .success(InmarsatcServiceFilter(rawValue: v) ?? InmarsatcServiceFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcServiceFilter(_ handle: SDRKIT_HANDLE, _ value: InmarsatcServiceFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "service_filter", value.rawValue))
    }
    /// Search limit.
    public static func aeroSearchLimitHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_limit_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroSearchLimitHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_limit_hz", value))
    }
    /// The options of the setting "satellite" of the stack "aero".
    public enum AeroSatellite: Int32 {
        case none = 0
        case alphasat = 1
        case i4f3 = 2
        case i3f5 = 3
        case i6f1 = 4
        case i4f1 = 5
    }
    /// Satellite.
    public static func aeroSatellite(_ handle: SDRKIT_HANDLE) -> Result<AeroSatellite, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "satellite", &v)
        return code == SDRKIT_OK ? .success(AeroSatellite(rawValue: v) ?? AeroSatellite(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroSatellite(_ handle: SDRKIT_HANDLE, _ value: AeroSatellite) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "satellite", value.rawValue))
    }
    /// Label filter.
    public static func aeroFilterLabel(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "filter_label", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroFilterLabel(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "filter_label", $0) })
    }
    /// Registration filter.
    public static func aeroFilterRegistration(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "filter_registration", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroFilterRegistration(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "filter_registration", $0) })
    }
    /// Show failed messages.
    public static func aeroShowFailed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "show_failed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroShowFailed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "show_failed", value))
    }
    /// Inmarsat-C.
    public static func inmarsatC(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "inmarsat_c", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatC(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "inmarsat_c", value))
    }
    /// CPU budget.
    public static func inmarsatBudget(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "budget", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatBudget(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "budget", value))
    }
    /// The options of the setting "satellite" of the stack "inmarsat".
    public enum InmarsatSatellite: Int32 {
        case auto = 0
        case alphasat25E = 1
        case inmarsat4F398W = 2
        case inmarsat3F554WAORW = 3
        case inmarsat6F1835E = 4
        case inmarsat4F11435E = 5
        case inmarsat155WAORE = 6
        case inmarsat645EIOR = 7
        case inmarsat178EPOR = 8
    }
    /// Satellite.
    public static func inmarsatSatellite(_ handle: SDRKIT_HANDLE) -> Result<InmarsatSatellite, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "satellite", &v)
        return code == SDRKIT_OK ? .success(InmarsatSatellite(rawValue: v) ?? InmarsatSatellite(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatSatellite(_ handle: SDRKIT_HANDLE, _ value: InmarsatSatellite) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "satellite", value.rawValue))
    }
    /// Burst threshold.
    public static func iridiumThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIridiumThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// RAW output.
    public static func iridiumRawOutput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw_output", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIridiumRawOutput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw_output", value))
    }
    /// Doppler plot span.
    public static func globalstarWindowS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "window_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlobalstarWindowS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "window_s", value))
    }
    /// Pass overlay.
    public static func globalstarPassOverlay(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "pass_overlay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlobalstarPassOverlay(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "pass_overlay", value))
    }
    /// Carrier threshold.
    public static func goesDcsThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesDcsThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// The options of the setting "link" of the stack "argos".
    public enum ArgosLink: Int32 {
        case auto = 0
        case uplink = 1
        case downlink = 2
    }
    /// Link.
    public static func argosLink(_ handle: SDRKIT_HANDLE) -> Result<ArgosLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "link", &v)
        return code == SDRKIT_OK ? .success(ArgosLink(rawValue: v) ?? ArgosLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosLink(_ handle: SDRKIT_HANDLE, _ value: ArgosLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "link", value.rawValue))
    }
    /// Carrier threshold.
    public static func hrdcpThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHrdcpThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Channel plan.
    public static func hrdcpChannelPlan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channel_plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHrdcpChannelPlan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channel_plan", $0) })
    }
    /// Wide search.
    public static func sarsatWideSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatWideSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide_search", value))
    }
    /// Repeat combining.
    public static func sarsatRepeatCombining(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "repeat_combining", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatRepeatCombining(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "repeat_combining", value))
    }
    /// Shortest repeat interval.
    public static func sarsatRepeatMinS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "repeat_min_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatRepeatMinS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "repeat_min_s", value))
    }
    /// Longest repeat interval.
    public static func sarsatRepeatMaxS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "repeat_max_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatRepeatMaxS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "repeat_max_s", value))
    }
    /// Follow the dial.
    public static func loraSatFollowDial(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_dial", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatFollowDial(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_dial", value))
    }
    /// Search SF.
    public static func loraSatAutoSf(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatAutoSf(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_sf", value))
    }
    /// The options of the setting "sf" of the stack "lora_sat".
    public enum LoraSatSf: Int32 {
        case n7 = 0
        case n8 = 1
        case n9 = 2
        case n10 = 3
        case n11 = 4
        case n12 = 5
    }
    /// SF.
    public static func loraSatSf(_ handle: SDRKIT_HANDLE) -> Result<LoraSatSf, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sf", &v)
        return code == SDRKIT_OK ? .success(LoraSatSf(rawValue: v) ?? LoraSatSf(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatSf(_ handle: SDRKIT_HANDLE, _ value: LoraSatSf) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sf", value.rawValue))
    }
    /// Search bandwidth.
    public static func loraSatAutoBw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_bw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatAutoBw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_bw", value))
    }
    /// The options of the setting "bw_khz" of the stack "lora_sat".
    public enum LoraSatBwKhz: Int32 {
        case n417 = 0
        case n625 = 1
        case n125Khz = 2
        case n250Khz = 3
        case n500Khz = 4
    }
    /// Bandwidth, kHz.
    public static func loraSatBwKhz(_ handle: SDRKIT_HANDLE) -> Result<LoraSatBwKhz, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bw_khz", &v)
        return code == SDRKIT_OK ? .success(LoraSatBwKhz(rawValue: v) ?? LoraSatBwKhz(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatBwKhz(_ handle: SDRKIT_HANDLE, _ value: LoraSatBwKhz) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bw_khz", value.rawValue))
    }
    /// The options of the setting "sync" of the stack "lora_sat".
    public enum LoraSatSync: Int32 {
        case both = 0
        case n0x12 = 1
        case n0x34 = 2
        case other = 3
    }
    /// Sync word.
    public static func loraSatSync(_ handle: SDRKIT_HANDLE) -> Result<LoraSatSync, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sync", &v)
        return code == SDRKIT_OK ? .success(LoraSatSync(rawValue: v) ?? LoraSatSync(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatSync(_ handle: SDRKIT_HANDLE, _ value: LoraSatSync) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sync", value.rawValue))
    }
    /// Other sync word.
    public static func loraSatSyncOther(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sync_other", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatSyncOther(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sync_other", value))
    }
    /// Preamble.
    public static func loraSatPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "preamble", value))
    }
    /// Doppler allowance.
    public static func loraSatDopplerMaxHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "doppler_max_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatDopplerMaxHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "doppler_max_hz", value))
    }
    /// Track the carrier.
    public static func loraSatTrack(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "track", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatTrack(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "track", value))
    }
    /// Channel offset.
    public static func loraSatOffsetHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "offset_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatOffsetHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "offset_hz", value))
    }
    /// Satellite overhead.
    public static func loraSatPassSatellite(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "pass_satellite", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatPassSatellite(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "pass_satellite", $0) })
    }
    /// The options of the setting "decoders" of the stack "orbcomm".
    public enum OrbcommDecoders: Int32 {
        case n1 = 0
        case n2 = 1
    }
    /// Decoders.
    public static func orbcommDecoders(_ handle: SDRKIT_HANDLE) -> Result<OrbcommDecoders, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "decoders", &v)
        return code == SDRKIT_OK ? .success(OrbcommDecoders(rawValue: v) ?? OrbcommDecoders(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommDecoders(_ handle: SDRKIT_HANDLE, _ value: OrbcommDecoders) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "decoders", value.rawValue))
    }
    /// Follow satellite.
    public static func orbcommFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "order" of the stack "psk_demod".
    public enum PskDemodOrder: Int32 {
        case auto = 0
        case bpsk = 1
        case qpsk = 2
        case oqpsk = 3
        case n8psk = 4
    }
    /// Order.
    public static func pskDemodOrder(_ handle: SDRKIT_HANDLE) -> Result<PskDemodOrder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "order", &v)
        return code == SDRKIT_OK ? .success(PskDemodOrder(rawValue: v) ?? PskDemodOrder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodOrder(_ handle: SDRKIT_HANDLE, _ value: PskDemodOrder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "order", value.rawValue))
    }
    /// Symbol rate.
    public static func pskDemodBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "baud", value))
    }
    /// The options of the setting "shape" of the stack "psk_demod".
    public enum PskDemodShape: Int32 {
        case auto = 0
        case rect = 1
        case n02 = 2
        case n025 = 3
        case n035 = 4
        case n05 = 5
        case n08 = 6
        case n10 = 7
    }
    /// Pulse shape.
    public static func pskDemodShape(_ handle: SDRKIT_HANDLE) -> Result<PskDemodShape, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "shape", &v)
        return code == SDRKIT_OK ? .success(PskDemodShape(rawValue: v) ?? PskDemodShape(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodShape(_ handle: SDRKIT_HANDLE, _ value: PskDemodShape) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "shape", value.rawValue))
    }
    /// Search span.
    public static func pskDemodSpan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "span", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodSpan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "span", $0) })
    }
    /// Snap to standard rates.
    public static func pskDemodSnapStandard(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "snap_standard", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodSnapStandard(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "snap_standard", value))
    }
    /// Follow Doppler.
    public static func pskDemodFollowDoppler(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_doppler", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodFollowDoppler(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_doppler", value))
    }
    /// Clicked frequency.
    public static func pskDemodClickHz(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "click_hz", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodClickHz(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "click_hz", $0) })
    }
    /// The options of the setting "differential" of the stack "psk_demod".
    public enum PskDemodDifferential: Int32 {
        case none = 0
        case dpsk = 1
        case nrzm = 2
        case nrzs = 3
    }
    /// Differential.
    public static func pskDemodDifferential(_ handle: SDRKIT_HANDLE) -> Result<PskDemodDifferential, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "differential", &v)
        return code == SDRKIT_OK ? .success(PskDemodDifferential(rawValue: v) ?? PskDemodDifferential(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodDifferential(_ handle: SDRKIT_HANDLE, _ value: PskDemodDifferential) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "differential", value.rawValue))
    }
    /// The options of the setting "descrambler" of the stack "psk_demod".
    public enum PskDemodDescrambler: Int32 {
        case none = 0
        case g3ruh = 1
        case ccsds = 2
    }
    /// Descrambler.
    public static func pskDemodDescrambler(_ handle: SDRKIT_HANDLE) -> Result<PskDemodDescrambler, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "descrambler", &v)
        return code == SDRKIT_OK ? .success(PskDemodDescrambler(rawValue: v) ?? PskDemodDescrambler(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodDescrambler(_ handle: SDRKIT_HANDLE, _ value: PskDemodDescrambler) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "descrambler", value.rawValue))
    }
    /// Sync word.
    public static func pskDemodSync(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "sync", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodSync(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "sync", $0) })
    }
    /// Invert the bits.
    public static func pskDemodInvert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodInvert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "invert", value))
    }
    /// Save bits.
    public static func pskDemodSaveBits(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "save_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodSaveBits(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "save_bits", value))
    }
    /// The options of the setting "lo" of the stack "qo100".
    public enum Qo100Lo: Int32 {
        case n9750Mhz = 0
        case n9739Mhz = 1
        case n10000Mhz = 2
        case n10600Mhz = 3
        case custom = 4
    }
    /// LNB oscillator.
    public static func qo100Lo(_ handle: SDRKIT_HANDLE) -> Result<Qo100Lo, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "lo", &v)
        return code == SDRKIT_OK ? .success(Qo100Lo(rawValue: v) ?? Qo100Lo(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100Lo(_ handle: SDRKIT_HANDLE, _ value: Qo100Lo) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "lo", value.rawValue))
    }
    /// Custom LO.
    public static func qo100LoCustomMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "lo_custom_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100LoCustomMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "lo_custom_mhz", value))
    }
    /// Automatic correction.
    public static func qo100AutoCorrection(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_correction", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100AutoCorrection(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_correction", value))
    }
    /// LNB calibration.
    public static func qo100LnbPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "lnb_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100LnbPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "lnb_ppm", value))
    }
    /// Dial (RF).
    public static func qo100DialHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100DialHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_hz", value))
    }
    /// The options of the setting "auto_listen" of the stack "qo100".
    public enum Qo100AutoListen: Int32 {
        case off = 0
        case loudest = 1
        case newest = 2
    }
    /// Auto-listen.
    public static func qo100AutoListen(_ handle: SDRKIT_HANDLE) -> Result<Qo100AutoListen, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "auto_listen", &v)
        return code == SDRKIT_OK ? .success(Qo100AutoListen(rawValue: v) ?? Qo100AutoListen(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100AutoListen(_ handle: SDRKIT_HANDLE, _ value: Qo100AutoListen) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "auto_listen", value.rawValue))
    }
    /// Conversation gap.
    public static func qo100ConversationGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "conversation_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100ConversationGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "conversation_gap_s", value))
    }
    /// Peak level.
    public static func qo100TxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100TxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Time-out.
    public static func qo100TxTimeout(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_timeout", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100TxTimeout(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_timeout", value))
    }
    /// The options of the setting "search" of the stack "cubesat".
    public enum CubesatSearch: Int32 {
        case table = 0
        case smart = 1
    }
    /// Search.
    public static func cubesatSearch(_ handle: SDRKIT_HANDLE) -> Result<CubesatSearch, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "search", &v)
        return code == SDRKIT_OK ? .success(CubesatSearch(rawValue: v) ?? CubesatSearch(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatSearch(_ handle: SDRKIT_HANDLE, _ value: CubesatSearch) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "search", value.rawValue))
    }
    /// CCSDS code length.
    public static func cubesatCcsdsLength(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "ccsds_length", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatCcsdsLength(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "ccsds_length", value))
    }
    /// Carriers followed.
    public static func cubesatSmartMaxChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "smart.max_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatSmartMaxChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "smart.max_channels", value))
    }
    /// Carrier SNR.
    public static func cubesatSmartMinSnrDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "smart.min_snr_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatSmartMinSnrDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "smart.min_snr_db", value))
    }
    /// Extra bit rates.
    public static func cubesatExtraBitrates(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "extra_bitrates", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatExtraBitrates(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "extra_bitrates", $0) })
    }
    /// The options of the setting "extra_ax25" of the stack "cubesat".
    public enum CubesatExtraAx25: Int32 {
        case off = 0
        case g3ruh = 1
        case nrzi = 2
    }
    /// AX.25 on the extra slicers.
    public static func cubesatExtraAx25(_ handle: SDRKIT_HANDLE) -> Result<CubesatExtraAx25, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "extra_ax25", &v)
        return code == SDRKIT_OK ? .success(CubesatExtraAx25(rawValue: v) ?? CubesatExtraAx25(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatExtraAx25(_ handle: SDRKIT_HANDLE, _ value: CubesatExtraAx25) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "extra_ax25", value.rawValue))
    }
    /// U482C audio carrier.
    public static func cubesatAfskCarrierHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "afsk_carrier_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatAfskCarrierHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "afsk_carrier_hz", value))
    }
    /// The options of the setting "afsk_bitrate" of the stack "cubesat".
    public enum CubesatAfskBitrate: Int32 {
        case n1200BitS = 0
        case n2400BitS = 1
        case n4800BitS = 2
    }
    /// U482C audio bit rate.
    public static func cubesatAfskBitrate(_ handle: SDRKIT_HANDLE) -> Result<CubesatAfskBitrate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "afsk_bitrate", &v)
        return code == SDRKIT_OK ? .success(CubesatAfskBitrate(rawValue: v) ?? CubesatAfskBitrate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatAfskBitrate(_ handle: SDRKIT_HANDLE, _ value: CubesatAfskBitrate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "afsk_bitrate", value.rawValue))
    }
    /// The options of the setting "geoscan_length" of the stack "cubesat".
    public enum CubesatGeoscanLength: Int32 {
        case n66Bytes = 0
        case n74Bytes = 1
    }
    /// Geoscan frame bytes.
    public static func cubesatGeoscanLength(_ handle: SDRKIT_HANDLE) -> Result<CubesatGeoscanLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "geoscan_length", &v)
        return code == SDRKIT_OK ? .success(CubesatGeoscanLength(rawValue: v) ?? CubesatGeoscanLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatGeoscanLength(_ handle: SDRKIT_HANDLE, _ value: CubesatGeoscanLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "geoscan_length", value.rawValue))
    }
    /// The options of the setting "ccsds_basis" of the stack "cubesat".
    public enum CubesatCcsdsBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// CCSDS RS basis.
    public static func cubesatCcsdsBasis(_ handle: SDRKIT_HANDLE) -> Result<CubesatCcsdsBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "ccsds_basis", &v)
        return code == SDRKIT_OK ? .success(CubesatCcsdsBasis(rawValue: v) ?? CubesatCcsdsBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatCcsdsBasis(_ handle: SDRKIT_HANDLE, _ value: CubesatCcsdsBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "ccsds_basis", value.rawValue))
    }
    /// CCSDS concatenated frame size.
    public static func cubesatConcatFrameBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "concat.frame_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatConcatFrameBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "concat.frame_bytes", value))
    }
    /// Interleave.
    public static func cubesatConcatInterleave(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "concat.interleave", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatConcatInterleave(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "concat.interleave", value))
    }
    /// The options of the setting "concat.basis" of the stack "cubesat".
    public enum CubesatConcatBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// Basis.
    public static func cubesatConcatBasis(_ handle: SDRKIT_HANDLE) -> Result<CubesatConcatBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "concat.basis", &v)
        return code == SDRKIT_OK ? .success(CubesatConcatBasis(rawValue: v) ?? CubesatConcatBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatConcatBasis(_ handle: SDRKIT_HANDLE, _ value: CubesatConcatBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "concat.basis", value.rawValue))
    }
    /// The options of the setting "concat.code" of the stack "cubesat".
    public enum CubesatConcatCode: Int32 {
        case ccsds = 0
        case nasaDsn = 1
        case ccsdsUninverted = 2
        case nasaDsnUninverted = 3
    }
    /// Code.
    public static func cubesatConcatCode(_ handle: SDRKIT_HANDLE) -> Result<CubesatConcatCode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "concat.code", &v)
        return code == SDRKIT_OK ? .success(CubesatConcatCode(rawValue: v) ?? CubesatConcatCode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatConcatCode(_ handle: SDRKIT_HANDLE, _ value: CubesatConcatCode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "concat.code", value.rawValue))
    }
    /// NRZ-M.
    public static func cubesatConcatNrzm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "concat.nrzm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatConcatNrzm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "concat.nrzm", value))
    }
    /// Randomiser.
    public static func cubesatConcatRandomiser(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "concat.randomiser", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatConcatRandomiser(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "concat.randomiser", value))
    }
    /// The options of the setting "mobitex_variant" of the stack "cubesat".
    public enum CubesatMobitexVariant: Int32 {
        case mobitexNx = 0
        case beesat1 = 1
        case beesat9 = 2
    }
    /// Mobitex variant.
    public static func cubesatMobitexVariant(_ handle: SDRKIT_HANDLE) -> Result<CubesatMobitexVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mobitex_variant", &v)
        return code == SDRKIT_OK ? .success(CubesatMobitexVariant(rawValue: v) ?? CubesatMobitexVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatMobitexVariant(_ handle: SDRKIT_HANDLE, _ value: CubesatMobitexVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mobitex_variant", value.rawValue))
    }
    /// Satellite table.
    public static func cubesatSatellites(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "satellites", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatSatellites(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "satellites", $0) })
    }
    /// Satellite followed.
    public static func cubesatTracked(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "tracked", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatTracked(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "tracked", $0) })
    }
    /// KISS output.
    public static func cubesatKiss(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "kiss", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatKiss(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "kiss", value))
    }
    /// The options of the setting "framing_filter" of the stack "cubesat".
    public enum CubesatFramingFilter: Int32 {
        case all = 0
        case ax25Afsk = 1
        case ax25G3ruh = 2
        case ax25Nrzi = 3
        case ax100Mode5 = 4
        case ax100Mode6 = 5
        case ccsdsRs = 6
        case geoscan = 7
        case lucky7 = 8
        case spino = 9
        case erminaz = 10
        case usp = 11
        case ngham = 12
        case u482c = 13
        case mobitex = 14
        case ccsdsConcat = 15
    }
    /// Framing.
    public static func cubesatFramingFilter(_ handle: SDRKIT_HANDLE) -> Result<CubesatFramingFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "framing_filter", &v)
        return code == SDRKIT_OK ? .success(CubesatFramingFilter(rawValue: v) ?? CubesatFramingFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatFramingFilter(_ handle: SDRKIT_HANDLE, _ value: CubesatFramingFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "framing_filter", value.rawValue))
    }
    /// The options of the setting "tx_rate" of the stack "cubesat".
    public enum CubesatTxRate: Int32 {
        case n1200BitSAfsk = 0
        case n9600BitSG3ruh = 1
    }
    /// Speed.
    public static func cubesatTxRate(_ handle: SDRKIT_HANDLE) -> Result<CubesatTxRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_rate", &v)
        return code == SDRKIT_OK ? .success(CubesatTxRate(rawValue: v) ?? CubesatTxRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatTxRate(_ handle: SDRKIT_HANDLE, _ value: CubesatTxRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_rate", value.rawValue))
    }
    /// Flags before.
    public static func cubesatTxDelay(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_delay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatTxDelay(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_delay", value))
    }
    /// Level.
    public static func cubesatTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Invert bits.
    public static func fleetBroadcastInvert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetBroadcastInvert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "invert", value))
    }
    /// Decode every FEC block.
    public static func bganDecodeAll(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_all", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganDecodeAll(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_all", value))
    }
    /// The options of the setting "bearer" of the stack "bgan".
    public enum BganBearer: Int32 {
        case auto = 0
        case f80t025q1b = 1
        case f80t1q1b = 2
        case f80t1q4b = 3
        case f80t1x4b = 4
        case fr80t25x4165b = 5
        case fr80t25x165b = 6
        case fr80t25x326b = 7
        case fr80t25x647b = 8
        case f80t45x8b = 9
        case fr80t5x4169b = 10
        case fr80t5x169b = 11
        case fr80t5x3211b = 12
        case fr80t5x6413b = 13
    }
    /// Bearer.
    public static func bganBearer(_ handle: SDRKIT_HANDLE) -> Result<BganBearer, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bearer", &v)
        return code == SDRKIT_OK ? .success(BganBearer(rawValue: v) ?? BganBearer(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBganBearer(_ handle: SDRKIT_HANDLE, _ value: BganBearer) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bearer", value.rawValue))
    }
    /// The options of the setting "uw_order" of the stack "bgan".
    public enum BganUwOrder: Int32 {
        case msbFirst = 0
        case lsbFirst = 1
    }
    /// Unique word bit order.
    public static func bganUwOrder(_ handle: SDRKIT_HANDLE) -> Result<BganUwOrder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "uw_order", &v)
        return code == SDRKIT_OK ? .success(BganUwOrder(rawValue: v) ?? BganUwOrder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBganUwOrder(_ handle: SDRKIT_HANDLE, _ value: BganUwOrder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "uw_order", value.rawValue))
    }
    /// The options of the setting "iq_labels" of the stack "bgan".
    public enum BganIqLabels: Int32 {
        case inOrder = 0
        case swapped = 1
    }
    /// I and Q columns.
    public static func bganIqLabels(_ handle: SDRKIT_HANDLE) -> Result<BganIqLabels, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "iq_labels", &v)
        return code == SDRKIT_OK ? .success(BganIqLabels(rawValue: v) ?? BganIqLabels(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBganIqLabels(_ handle: SDRKIT_HANDLE, _ value: BganIqLabels) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "iq_labels", value.rawValue))
    }
    /// Satellite names.
    public static func bganSatellites(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "satellites", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setBganSatellites(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "satellites", $0) })
    }
    /// Scan the band.
    public static func bganScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Scan from.
    public static func bganScanFromHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_from_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganScanFromHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_from_hz", value))
    }
    /// Scan to.
    public static func bganScanToHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_to_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganScanToHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_to_hz", value))
    }
    /// Threshold.
    public static func satSurveyThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveyThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Channel table.
    public static func satSurveyChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveyChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Satellite elements.
    public static func satSurveySatellites(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "satellites", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveySatellites(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "satellites", $0) })
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func satSurveyStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveyStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum SatcomGeneratorSettings {
    /// Inverted polarity.
    public static func inmarsatcInvert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcInvert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert", value))
    }
    /// First frame number.
    public static func inmarsatcFirstFrame(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_first_frame", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcFirstFrame(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_first_frame", value))
    }
    /// Extra channels.
    public static func inmarsatcExtra(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_extra", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcExtra(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_extra", value))
    }
    /// Start into the first frame.
    public static func inmarsatcStartSymbols(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_start_symbols", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatcStartSymbols(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_start_symbols", value))
    }
    /// Channels (1 = 600, 2 = 1200, 4 = 10500).
    public static func aeroChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_channels", value))
    }
    /// No built-in offsets and drift.
    public static func aeroPlain(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_plain", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAeroPlain(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_plain", value))
    }
    /// Channels (1 = 600, 2 = 1200, 4 = 10500).
    public static func inmarsatChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_channels", value))
    }
    /// No built-in offsets and drift.
    public static func inmarsatPlain(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_plain", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInmarsatPlain(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_plain", value))
    }
    /// Doppler.
    public static func iridiumBurstDoppler(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_burst_doppler", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIridiumBurstDoppler(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_burst_doppler", value))
    }
    /// Bursts per frame.
    public static func iridiumBursts(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_bursts", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIridiumBursts(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_bursts", value))
    }
    /// Radio centre.
    public static func iridiumCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIridiumCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// The options of the setting "gen_scenario" of the stack "globalstar".
    public enum GlobalstarScenario: Int32 {
        case pass = 0
        case steady = 1
        case silence = 2
    }
    /// Scenario.
    public static func globalstarScenario(_ handle: SDRKIT_HANDLE) -> Result<GlobalstarScenario, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &v)
        return code == SDRKIT_OK ? .success(GlobalstarScenario(rawValue: v) ?? GlobalstarScenario(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGlobalstarScenario(_ handle: SDRKIT_HANDLE, _ value: GlobalstarScenario) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scenario", value.rawValue))
    }
    /// Channel.
    public static func globalstarChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlobalstarChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_channel", value))
    }
    /// Time scale.
    public static func globalstarTimeScale(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_time_scale", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlobalstarTimeScale(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_time_scale", value))
    }
    /// Radio centre.
    public static func globalstarCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlobalstarCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// The options of the setting "gen_scenario" of the stack "goes_dcs".
    public enum GoesDcsScenario: Int32 {
        case threePlatforms = 0
        case onlyThe300BitSPlatform = 1
        case oneOfEachRate300And1200AsNrz = 2
        case threePlatformsAndASteadyCarrier = 3
    }
    /// Scenario.
    public static func goesDcsScenario(_ handle: SDRKIT_HANDLE) -> Result<GoesDcsScenario, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &v)
        return code == SDRKIT_OK ? .success(GoesDcsScenario(rawValue: v) ?? GoesDcsScenario(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesDcsScenario(_ handle: SDRKIT_HANDLE, _ value: GoesDcsScenario) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scenario", value.rawValue))
    }
    /// Downlink.
    public static func argosDownlink(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_downlink", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosDownlink(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_downlink", value))
    }
    /// Seconds between bursts.
    public static func argosGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value))
    }
    /// Burst-to-burst step.
    public static func argosStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_step_hz", value))
    }
    /// Another modulation each cycle.
    public static func argosOther(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_other", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosOther(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_other", value))
    }
    /// Largest Doppler of the pass.
    public static func argosPassDopplerHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pass_doppler_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosPassDopplerHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pass_doppler_hz", value))
    }
    /// Doppler time constant.
    public static func argosPassTauS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pass_tau_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosPassTauS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pass_tau_s", value))
    }
    /// Closest approach.
    public static func argosPassTcaS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pass_tca_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosPassTcaS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pass_tca_s", value))
    }
    /// BPSK.
    public static func argosBpsk(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_bpsk", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosBpsk(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_bpsk", value))
    }
    /// NRZI.
    public static func argosNrzi(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nrzi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArgosNrzi(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nrzi", value))
    }
    /// The options of the setting "gen_scene" of the stack "hrdcp".
    public enum HrdcpScene: Int32 {
        case twoPlatforms = 0
        case platformAAlone = 1
    }
    /// Scene.
    public static func hrdcpScene(_ handle: SDRKIT_HANDLE) -> Result<HrdcpScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(HrdcpScene(rawValue: v) ?? HrdcpScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHrdcpScene(_ handle: SDRKIT_HANDLE, _ value: HrdcpScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Second generation.
    public static func sarsatSecondGeneration(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_second_generation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatSecondGeneration(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_second_generation", value))
    }
    /// Test beacon.
    public static func sarsatBeacon(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_beacon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatBeacon(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_beacon", value))
    }
    /// Reversed phase polarity.
    public static func sarsatReverse(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_reverse", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatReverse(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_reverse", value))
    }
    /// Bit rate error.
    public static func sarsatBitratePct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bitrate_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatBitratePct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bitrate_pct", value))
    }
    /// Seconds between bursts.
    public static func sarsatGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSarsatGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value))
    }
    /// The options of the setting "gen_band" of the stack "lora_sat".
    public enum LoraSatBand: Int32 {
        case n4368 = 0
        case n8681 = 1
        case n137055 = 2
    }
    /// Pass.
    public static func loraSatBand(_ handle: SDRKIT_HANDLE) -> Result<LoraSatBand, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_band", &v)
        return code == SDRKIT_OK ? .success(LoraSatBand(rawValue: v) ?? LoraSatBand(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatBand(_ handle: SDRKIT_HANDLE, _ value: LoraSatBand) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_band", value.rawValue))
    }
    /// The options of the setting "gen_sf" of the stack "lora_sat".
    public enum LoraSatSf: Int32 {
        case thePassS = 0
        case n7 = 1
        case n8 = 2
        case n9 = 3
        case n10 = 4
        case n11 = 5
        case n12 = 6
    }
    /// SF.
    public static func loraSatSf(_ handle: SDRKIT_HANDLE) -> Result<LoraSatSf, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_sf", &v)
        return code == SDRKIT_OK ? .success(LoraSatSf(rawValue: v) ?? LoraSatSf(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatSf(_ handle: SDRKIT_HANDLE, _ value: LoraSatSf) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_sf", value.rawValue))
    }
    /// The options of the setting "gen_bw_khz" of the stack "lora_sat".
    public enum LoraSatBwKhz: Int32 {
        case thePassS = 0
        case n417 = 1
        case n625 = 2
        case n125Khz = 3
        case n250Khz = 4
        case n500Khz = 5
    }
    /// Bandwidth, kHz.
    public static func loraSatBwKhz(_ handle: SDRKIT_HANDLE) -> Result<LoraSatBwKhz, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bw_khz", &v)
        return code == SDRKIT_OK ? .success(LoraSatBwKhz(rawValue: v) ?? LoraSatBwKhz(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatBwKhz(_ handle: SDRKIT_HANDLE, _ value: LoraSatBwKhz) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bw_khz", value.rawValue))
    }
    /// Doppler of the pass.
    public static func loraSatPassDoppler(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_pass_doppler", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatPassDoppler(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_pass_doppler", value))
    }
    /// Seconds into the pass.
    public static func loraSatStartS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_start_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatStartS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_start_s", value))
    }
    /// Sync word.
    public static func loraSatSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraSatSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_sync", value))
    }
    /// Downlinks.
    public static func orbcommDownlinks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_downlinks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommDownlinks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_downlinks", value))
    }
    /// Satellite Doppler.
    public static func orbcommSatelliteDopplerHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_satellite_doppler_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommSatelliteDopplerHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_satellite_doppler_hz", value))
    }
    /// Doppler ramp (+-).
    public static func orbcommRampHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ramp_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommRampHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ramp_hz", value))
    }
    /// Ramp leg.
    public static func orbcommRampS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ramp_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommRampS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ramp_s", value))
    }
    /// Broken checks.
    public static func orbcommBroken(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_broken", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommBroken(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_broken", value))
    }
    /// Radio centre.
    public static func orbcommCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOrbcommCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// The options of the setting "gen_order" of the stack "psk_demod".
    public enum PskDemodOrder: Int32 {
        case bpsk = 0
        case qpsk = 1
        case oqpsk = 2
        case n8psk = 3
    }
    /// Order.
    public static func pskDemodOrder(_ handle: SDRKIT_HANDLE) -> Result<PskDemodOrder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_order", &v)
        return code == SDRKIT_OK ? .success(PskDemodOrder(rawValue: v) ?? PskDemodOrder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodOrder(_ handle: SDRKIT_HANDLE, _ value: PskDemodOrder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_order", value.rawValue))
    }
    /// Symbol rate.
    public static func pskDemodBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud", value))
    }
    /// Roll-off (0: rectangular).
    public static func pskDemodRolloff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rolloff", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodRolloff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value))
    }
    /// The options of the setting "gen_coding" of the stack "psk_demod".
    public enum PskDemodCoding: Int32 {
        case none = 0
        case dpsk = 1
        case nrzm = 2
        case nrzs = 3
        case g3ruh = 4
        case ccsds = 5
    }
    /// Coding.
    public static func pskDemodCoding(_ handle: SDRKIT_HANDLE) -> Result<PskDemodCoding, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_coding", &v)
        return code == SDRKIT_OK ? .success(PskDemodCoding(rawValue: v) ?? PskDemodCoding(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodCoding(_ handle: SDRKIT_HANDLE, _ value: PskDemodCoding) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_coding", value.rawValue))
    }
    /// Sync word.
    public static func pskDemodSync(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_sync", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodSync(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_sync", $0) })
    }
    /// Conjugated symbols.
    public static func pskDemodConjugate(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_conjugate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskDemodConjugate(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_conjugate", value))
    }
    /// No signals.
    public static func qo100Empty(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_empty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100Empty(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_empty", value))
    }
    /// Radio centre (RF, 0: automatic).
    public static func qo100CentreMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100CentreMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_mhz", value))
    }
    /// The options of the setting "gen_lo" of the stack "qo100".
    public enum Qo100Lo: Int32 {
        case n9750Mhz = 0
        case n9739Mhz = 1
        case n10000Mhz = 2
        case n10600Mhz = 3
        case custom = 4
    }
    /// Nominal LO.
    public static func qo100Lo(_ handle: SDRKIT_HANDLE) -> Result<Qo100Lo, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_lo", &v)
        return code == SDRKIT_OK ? .success(Qo100Lo(rawValue: v) ?? Qo100Lo(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQo100Lo(_ handle: SDRKIT_HANDLE, _ value: Qo100Lo) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_lo", value.rawValue))
    }
    /// The options of the setting "gen_framing" of the stack "cubesat".
    public enum CubesatFraming: Int32 {
        case everyFramingInTurn = 0
        case ax25Afsk1200 = 1
        case ax25G3ruh9600 = 2
        case ax100Mode59600 = 3
        case ax100Mode54800 = 4
        case ax100Mode69600 = 5
        case ccsdsRs9600 = 6
        case geoscan9600 = 7
        case lucky74800 = 8
        case spino2400 = 9
        case erminaz9600 = 10
        case usp9600 = 11
        case ngham4800 = 12
        case u482cAfsk4800 = 13
        case mobitexNx4800 = 14
        case ccsdsConcatenated9600 = 15
    }
    /// Framing.
    public static func cubesatFraming(_ handle: SDRKIT_HANDLE) -> Result<CubesatFraming, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_framing", &v)
        return code == SDRKIT_OK ? .success(CubesatFraming(rawValue: v) ?? CubesatFraming(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCubesatFraming(_ handle: SDRKIT_HANDLE, _ value: CubesatFraming) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_framing", value.rawValue))
    }
    /// The options of the setting "gen_scenario" of the stack "fleet_broadcast".
    public enum FleetBroadcastScenario: Int32 {
        case syncReversalsIdleAndRandomChannels = 0
        case syncIdleChannels = 1
        case noiseOnly = 2
        case aPlainCarrier = 3
        case randomBitsEverywhere = 4
    }
    /// Scenario.
    public static func fleetBroadcastScenario(_ handle: SDRKIT_HANDLE) -> Result<FleetBroadcastScenario, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &v)
        return code == SDRKIT_OK ? .success(FleetBroadcastScenario(rawValue: v) ?? FleetBroadcastScenario(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetBroadcastScenario(_ handle: SDRKIT_HANDLE, _ value: FleetBroadcastScenario) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scenario", value.rawValue))
    }
    /// Carrier from the dial.
    public static func fleetBroadcastCarrierHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetBroadcastCarrierHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_hz", value))
    }
    /// Fading depth.
    public static func fleetBroadcastFadeDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fade_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetBroadcastFadeDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fade_db", value))
    }
    /// Invert bits.
    public static func fleetBroadcastInvert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFleetBroadcastInvert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert", value))
    }
    /// The options of the setting "gen_bearer" of the stack "bgan".
    public enum BganBearer: Int32 {
        case aGroupOfCarriers = 0
        case f80t025q1b = 1
        case f80t1q1b = 2
        case f80t1q4b = 3
        case f80t1x4b = 4
        case fr80t25x4165b = 5
        case fr80t25x165b = 6
        case fr80t25x326b = 7
        case fr80t25x647b = 8
        case f80t45x8b = 9
        case fr80t5x4169b = 10
        case fr80t5x169b = 11
        case fr80t5x3211b = 12
        case fr80t5x6413b = 13
    }
    /// Bearers.
    public static func bganBearer(_ handle: SDRKIT_HANDLE) -> Result<BganBearer, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bearer", &v)
        return code == SDRKIT_OK ? .success(BganBearer(rawValue: v) ?? BganBearer(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBganBearer(_ handle: SDRKIT_HANDLE, _ value: BganBearer) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bearer", value.rawValue))
    }
    /// The options of the setting "gen_subtype" of the stack "bgan".
    public enum BganSubtype: Int32 {
        case l8 = 0
        case l7 = 1
        case l6 = 2
        case l5 = 3
        case l4 = 4
        case l3 = 5
        case l2 = 6
        case l1 = 7
        case r = 8
        case h1 = 9
        case h2 = 10
        case h3 = 11
        case h4 = 12
        case h5 = 13
        case h6 = 14
    }
    /// Subtype.
    public static func bganSubtype(_ handle: SDRKIT_HANDLE) -> Result<BganSubtype, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_subtype", &v)
        return code == SDRKIT_OK ? .success(BganSubtype(rawValue: v) ?? BganSubtype(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBganSubtype(_ handle: SDRKIT_HANDLE, _ value: BganSubtype) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_subtype", value.rawValue))
    }
    /// Carrier C/N0.
    public static func bganCarrierCn0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_cn0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganCarrierCn0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_cn0", value))
    }
    /// Carrier from the dial.
    public static func bganCarrierOffsetHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_offset_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganCarrierOffsetHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_offset_hz", value))
    }
    /// Symbol clock error.
    public static func bganCarrierClockPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_clock_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganCarrierClockPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_clock_ppm", value))
    }
    /// Carrier drift.
    public static func bganCarrierDriftHzS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_drift_hz_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganCarrierDriftHzS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_drift_hz_s", value))
    }
    /// Fading rate.
    public static func bganFadeHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fade_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganFadeHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fade_hz", value))
    }
    /// Fading depth.
    public static func bganFadeDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fade_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganFadeDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fade_depth", value))
    }
    /// Spot beam.
    public static func bganSpot(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_spot", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBganSpot(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_spot", value))
    }
    /// Test signal.
    public static func satSurveySignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveySignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_signal", value))
    }
    /// Test signal S/N.
    public static func satSurveyLevelDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_level_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveyLevelDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_level_db", value))
    }
    /// Test signal carrier offset.
    public static func satSurveyCarrierHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatSurveyCarrierHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_hz", value))
    }
}

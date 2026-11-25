// sdrkit: the typed interface of the stack family "drones", version 1.3. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum DronesInterface {
    public static let major = 1
    public static let minor = 3
}

/// The result fields of Drone Remote ID.
public struct RemoteIdFields {
    public enum Band: Int32 {
        case ble38WiFi6 = 0
        case wiFi6 = 1
        case wiFi1 = 2
        case wiFi11 = 3
        case ble37WiFi1 = 4
        case ble38 = 5
        case ble39 = 6
    }
    /// The BLE advertising channel in the band (0 for a file without a centre).
    public var bleChannel: Int64?
    /// The offset of the BLE advertising channel from the centre.
    public var bleOffsetHz: Double?
    /// The Wi-Fi channel in the band (0 for a file without a centre).
    public var wifiChannel: Int64?
    /// The offset of the Wi-Fi channel from the centre.
    public var wifiOffsetHz: Double
    /// The band of the present centre, when it is one of the bands that the stack offers.
    public var band: Band?
    /// Drones in the table.
    public var drones: Int64
    /// Drones in the table whose trust verdict is suspicious or spoofed.
    public var flagged: Int64
    /// Messages decoded.
    public var messages: Int64
    /// Messages refused: members of a Message Pack that are packs or of a reserved type, authentication pages that fail their checks.
    public var messagesRefused: Int64
    /// Messages with a value outside its range (a Location timestamp above 36000, a latitude or longitude out of range), which the tables show as absent.
    public var fieldsOutOfRange: Int64
    /// Good BLE packets.
    public var bleGood: Int64
    /// Good BLE packets with Remote ID.
    public var bleRemoteId: Int64
    /// Good Wi-Fi frames.
    public var wifiGood: Int64
    /// Good Wi-Fi frames with Remote ID.
    public var wifiRemoteId: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Basic ID messages (a field: all drones; a column: one drone).
    public var basicIdMessages: Int64
    /// Location messages.
    public var locationMessages: Int64
    /// Authentication messages.
    public var authenticationMessages: Int64
    /// Self-ID messages.
    public var selfIdMessages: Int64
    /// System messages.
    public var systemMessages: Int64
    /// Operator ID messages.
    public var operatorIdMessages: Int64
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?

    init(_ c: sdrkit_remote_id_fields) {
        bleChannel = c.has_ble_channel ? c.ble_channel : nil
        bleOffsetHz = c.has_ble_offset_hz ? c.ble_offset_hz : nil
        wifiChannel = c.has_wifi_channel ? c.wifi_channel : nil
        wifiOffsetHz = c.wifi_offset_hz
        band = c.has_band ? Band(rawValue: c.band) : nil
        drones = c.drones
        flagged = c.flagged
        messages = c.messages
        messagesRefused = c.messages_refused
        fieldsOutOfRange = c.fields_out_of_range
        bleGood = c.ble_good
        bleRemoteId = c.ble_remote_id
        wifiGood = c.wifi_good
        wifiRemoteId = c.wifi_remote_id
        framesBad = c.frames_bad
        basicIdMessages = c.basic_id_messages
        locationMessages = c.location_messages
        authenticationMessages = c.authentication_messages
        selfIdMessages = c.self_id_messages
        systemMessages = c.system_messages
        operatorIdMessages = c.operator_id_messages
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RemoteIdFields, SDRKitCode> {
        var c = sdrkit_remote_id_fields()
        let code = sdrkit_remote_id_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RemoteIdFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A drone heard on Remote ID. The row key is the broadcast address. The source is the shown ID: the serial number, else the session ID, else the CAA registration, else the UTM UUID, else the address.
public struct RemoteIdDroneRow {
    public enum Verdict: Int32 {
        case authenticated = 0
        case plausible = 1
        case suspicious = 2
        case spoofed = 3
    }
    public enum Via: Int32 {
        case ble = 0
        case wifi = 1
        case both = 2
    }
    /// The row key.
    public var key: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The broadcast address.
    public var address: String
    /// The trust verdict of the broadcasts heard since the stack started: authenticated, plausible, suspicious or spoofed. No value before the first packet since the start (a drone of a restored state).
    public var verdict: Verdict?
    /// The active reasons of the trust verdict: their stable names (`teleport`, `counter_conflict`, ...) joined by ", ". Empty when there is none.
    public var reasons: String
    /// Serial number.
    public var serial: String?
    /// Session ID.
    public var sessionId: String?
    /// CAA registration.
    public var caaRegistration: String?
    /// UTM assigned UUID.
    public var utmUuid: String?
    /// UA type code (ASTM F3411-22a, Basic ID message).
    public var uaType: Int64?
    /// Operational status code (ASTM F3411-22a, Location message).
    public var status: Int64?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// Geodetic altitude.
    public var altGeoM: Double?
    /// Pressure altitude.
    public var altBaroM: Double?
    /// Height above take-off or ground, as the column `height_type` says.
    public var heightM: Double?
    /// What the height is above: 0 take-off, 1 ground.
    public var heightType: Int64?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// Vertical speed, up.
    public var vspeedMS: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// Horizontal accuracy code.
    public var horizAcc: Int64?
    /// Vertical accuracy code.
    public var vertAcc: Int64?
    /// Speed accuracy code.
    public var speedAcc: Int64?
    /// Tenths of a second since the hour.
    public var timestampDs: Int64?
    /// The operator's position.
    public var operatorPosition: SDRKitPosition?
    /// The operator's altitude.
    public var operatorAltM: Double?
    /// Operator location type code.
    public var locationType: Int64?
    /// Classification type code.
    public var classification: Int64?
    /// EU category code.
    public var category: Int64?
    /// EU class code (1 to 7 are C0 to C6).
    public var `class`: Int64?
    /// System time stamp, seconds since 2019-01-01.
    public var systemTimestampS: Int64?
    /// Operator ID.
    public var operatorId: String?
    /// Self-ID text.
    public var selfId: String?
    /// Authentication type code.
    public var authentication: Int64?
    /// Authentication pages.
    public var authenticationPages: Int64?
    /// Authentication data length in bytes, from page 0.
    public var authenticationLength: Int64?
    /// Every page of the present authentication was heard. No signature is verified.
    public var authenticationComplete: Bool?
    /// Basic ID messages (a field: all drones; a column: one drone).
    public var basicIdMessages: Int64
    /// Location messages.
    public var locationMessages: Int64
    /// Authentication messages.
    public var authenticationMessages: Int64
    /// Self-ID messages.
    public var selfIdMessages: Int64
    /// System messages.
    public var systemMessages: Int64
    /// Operator ID messages.
    public var operatorIdMessages: Int64
    /// Trail points.
    public var trackPoints: Int64
    /// How the drone was heard.
    public var via: Via?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// Times heard.
    public var heard: Int64?

    init(_ c: sdrkit_remote_id_drone_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        address = sdrkitText(c.address)
        verdict = c.has_verdict ? Verdict(rawValue: c.verdict) : nil
        reasons = sdrkitText(c.reasons)
        serial = c.has_serial ? sdrkitText(c.serial) : nil
        sessionId = c.has_session_id ? sdrkitText(c.session_id) : nil
        caaRegistration = c.has_caa_registration ? sdrkitText(c.caa_registration) : nil
        utmUuid = c.has_utm_uuid ? sdrkitText(c.utm_uuid) : nil
        uaType = c.has_ua_type ? c.ua_type : nil
        status = c.has_status ? c.status : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altGeoM = c.has_alt_geo_m ? c.alt_geo_m : nil
        altBaroM = c.has_alt_baro_m ? c.alt_baro_m : nil
        heightM = c.has_height_m ? c.height_m : nil
        heightType = c.has_height_type ? c.height_type : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        vspeedMS = c.has_vspeed_m_s ? c.vspeed_m_s : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        horizAcc = c.has_horiz_acc ? c.horiz_acc : nil
        vertAcc = c.has_vert_acc ? c.vert_acc : nil
        speedAcc = c.has_speed_acc ? c.speed_acc : nil
        timestampDs = c.has_timestamp_ds ? c.timestamp_ds : nil
        operatorPosition = c.has_operator_position ? SDRKitPosition(c.operator_position) : nil
        operatorAltM = c.has_operator_alt_m ? c.operator_alt_m : nil
        locationType = c.has_location_type ? c.location_type : nil
        classification = c.has_classification ? c.classification : nil
        category = c.has_category ? c.category : nil
        `class` = c.has_class_ ? c.class_ : nil
        systemTimestampS = c.has_system_timestamp_s ? c.system_timestamp_s : nil
        operatorId = c.has_operator_id ? sdrkitText(c.operator_id) : nil
        selfId = c.has_self_id ? sdrkitText(c.self_id) : nil
        authentication = c.has_authentication ? c.authentication : nil
        authenticationPages = c.has_authentication_pages ? c.authentication_pages : nil
        authenticationLength = c.has_authentication_length ? c.authentication_length : nil
        authenticationComplete = c.has_authentication_complete ? c.authentication_complete : nil
        basicIdMessages = c.basic_id_messages
        locationMessages = c.location_messages
        authenticationMessages = c.authentication_messages
        selfIdMessages = c.self_id_messages
        systemMessages = c.system_messages
        operatorIdMessages = c.operator_id_messages
        trackPoints = c.track_points
        via = Via(rawValue: c.via)
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A line of the message log. The row key is the sequence number of the line.
public struct RemoteIdMessageRow {
    public enum Via: Int32 {
        case ble = 0
        case wifi = 1
    }
    public enum TypeOption: Int32 {
        case basicId = 0
        case location = 1
        case authentication = 2
        case selfId = 3
        case system = 4
        case operatorId = 5
    }
    /// The row key.
    public var key: String
    /// Signal time of a log line.
    public var timeS: Double
    /// Wall time of a log line.
    public var time: Double?
    /// The carrier of a log line.
    public var via: Via?
    /// The radio channel of a log line (BLE 37 to 39, Wi-Fi 1 to 13; none when unknown).
    public var radioChannel: Int64?
    /// The broadcast address.
    public var address: String
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// The message type.
    public var type: TypeOption?
    /// The summary line of a message.
    public var message: String

    init(_ c: sdrkit_remote_id_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        time = c.has_time ? c.time : nil
        via = Via(rawValue: c.via)
        radioChannel = c.has_radio_channel ? c.radio_channel : nil
        address = sdrkitText(c.address)
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        type = TypeOption(rawValue: c.type)
        message = sdrkitText(c.message)
    }
}

/// A track point of a drone. The row key is the drone key and the signal time of the point.
public struct DronesDroneTrackRow {
    /// The row key.
    public var key: String
    /// The drone a track point belongs to.
    public var drone: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The signal time of a track point.
    public var timeS: Double

    init(_ c: sdrkit_drones_drone_track_row) {
        key = sdrkitText(c.key)
        drone = sdrkitText(c.drone)
        position = SDRKitPosition(c.position)
        timeS = c.time_s
    }
}

/// The result fields of DJI drones.
public struct DjiDroneidFields {
    /// Bursts measured.
    public var burstsMeasured: Int64
    /// OFDM-like bursts.
    public var burstsOfdmLike: Int64
    /// DroneID packets decoded.
    public var droneidDecoded: Int64
    /// DroneID bursts with the symbols whose CRC failed.
    public var droneidCrcFailed: Int64
    /// DroneID attempts.
    public var droneidTried: Int64
    /// The capture centre.
    public var centreHz: Double?
    /// The carrier offset of the last DroneID burst from the 0.5 MHz grid.
    public var burstCfoHz: Double?
    /// Drones in the table.
    public var drones: Int64
    /// Aircraft in the aircraft table: the drones with a readable packet, the aircraft that send the encrypted format, and the sender of empty frames.
    public var aircraft: Int64
    /// DroneID frames of the encrypted format: the aircraft is present, its position and serial number are not readable.
    public var droneidEncrypted: Int64
    /// Empty DroneID frames: the aircraft is present and sends no data in this state.
    public var droneidEmpty: Int64
    /// DroneID frames whose CRC-24 passed and whose layout is not known.
    public var droneidOther: Int64
    /// Frames of zeros that were refused: the received bits did not agree with an empty frame.
    public var droneidRefused: Int64
    /// Pairs of the two DroneID symbols that the continuous search found.
    public var droneidPairs: Int64
    /// Bursts of the continuous search whose two symbols passed the check at the full rate.
    public var droneidBursts: Int64
    /// The DroneID frequencies inside the capture that the continuous search covers.
    public var droneidSearched: Int64
    /// The track with the most likely DJI shape: a row key of the `signals` table.
    public var signal: String?
    /// Noise floor, dBFS.
    public var noiseDbfs: Double?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double?

    init(_ c: sdrkit_dji_droneid_fields) {
        burstsMeasured = c.bursts_measured
        burstsOfdmLike = c.bursts_ofdm_like
        droneidDecoded = c.droneid_decoded
        droneidCrcFailed = c.droneid_crc_failed
        droneidTried = c.droneid_tried
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        burstCfoHz = c.has_burst_cfo_hz ? c.burst_cfo_hz : nil
        drones = c.drones
        aircraft = c.aircraft
        droneidEncrypted = c.droneid_encrypted
        droneidEmpty = c.droneid_empty
        droneidOther = c.droneid_other
        droneidRefused = c.droneid_refused
        droneidPairs = c.droneid_pairs
        droneidBursts = c.droneid_bursts
        droneidSearched = c.droneid_searched
        signal = c.has_signal ? sdrkitText(c.signal) : nil
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DjiDroneidFields, SDRKitCode> {
        var c = sdrkit_dji_droneid_fields()
        let code = sdrkit_dji_droneid_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DjiDroneidFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A drone link recognised by its shape.
public struct DjiDroneidSignalRow {
    public enum Class: Int32 {
        case otherWideband = 0
        case unknownOfdm = 1
        case ocusyncLike = 2
        case lightbridgeLike = 3
        case droneid = 4
    }
    public enum Band: Int32 {
        case n2g4 = 0
        case n5g8 = 1
        case n5g2 = 2
    }
    public enum Reason: Int32 {
        case decoded = 0
        case zcOnly = 1
        case everyMs = 2
        case periodic = 3
        case otherPeriod = 4
        case irregular = 5
        case tooFew = 6
        case notOfdm = 7
    }
    /// The row key.
    public var key: String
    /// The track number.
    public var track: Int64
    /// The class of a track.
    public var `class`: Class?
    /// The confidence of the class.
    public var confidencePct: Double
    /// The radio frequency.
    public var freqHz: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// The width.
    public var widthHz: Double
    /// The last burst was clipped.
    public var clipped: Bool
    /// The burst length.
    public var burst: Double
    /// The burst period.
    public var period: Double?
    /// The duty cycle.
    public var dutyPct: Double
    /// The share of gaps that are whole periods.
    public var regularityPct: Double
    /// How OFDM-like the bursts are.
    public var ofdmPct: Double
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Bursts seen.
    public var bursts: Int64
    /// The DroneID symbols were seen.
    public var zcSeen: Bool
    /// DroneID packets decoded on the track.
    public var decoded: Int64
    /// DroneID CRC failures on the track.
    public var failed: Int64
    /// The band.
    public var band: Band?
    /// Why the track has its class.
    public var reason: Reason?
    /// The track is within 3 MHz of a frequency reported for DroneID.
    public var nearDroneid: Bool
    /// The signal time of the last burst.
    public var lastS: Double

    init(_ c: sdrkit_dji_droneid_signal_row) {
        key = sdrkitText(c.key)
        track = c.track
        `class` = Class(rawValue: c.class_)
        confidencePct = c.confidence_pct
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        offsetHz = c.offset_hz
        widthHz = c.width_hz
        clipped = c.clipped
        burst = c.burst
        period = c.has_period ? c.period : nil
        dutyPct = c.duty_pct
        regularityPct = c.regularity_pct
        ofdmPct = c.ofdm_pct
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        bursts = c.bursts
        zcSeen = c.zc_seen
        decoded = c.decoded
        failed = c.failed
        band = c.has_band ? Band(rawValue: c.band) : nil
        reason = Reason(rawValue: c.reason)
        nearDroneid = c.near_droneid
        lastS = c.last_s
    }
}

/// A DJI drone heard on DroneID.
public struct DjiDroneidDroneRow {
    /// The row key.
    public var key: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet.
    public var altM: Double
    /// The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude.
    public var heightM: Double
    /// The altitude field as received (a signed 16-bit number), whose unit is not confirmed.
    public var altRaw: Int64
    /// The height field as received (a signed 16-bit number), whose unit is not confirmed.
    public var heightRaw: Int64
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double
    /// The home position.
    public var home: SDRKitPosition?
    /// The pilot's position.
    public var pilot: SDRKitPosition?
    /// The packet sequence number.
    public var seq: Int64
    /// The state information bits.
    public var stateInfo: Int64
    /// The packet version.
    public var version: Int64
    /// The inner CRC passed.
    public var innerCrcOk: Bool
    /// Velocity north.
    public var vNorth: Int64
    /// Velocity east.
    public var vEast: Int64
    /// Velocity up.
    public var vUp: Int64
    /// The raw yaw.
    public var yaw: Int64
    /// The GPS time of the packet.
    public var gpsTime: Double
    /// The device type code.
    public var deviceType: Int64
    /// The UUID.
    public var uuid: String?
    /// The radio frequency.
    public var freqHz: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The track number.
    public var track: Int64
    /// Trail points.
    public var trailPoints: Int64
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// Times heard.
    public var heard: Int64?

    init(_ c: sdrkit_dji_droneid_drone_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.alt_m
        heightM = c.height_m
        altRaw = c.alt_raw
        heightRaw = c.height_raw
        speedKt = c.speed_kt
        courseDeg = c.course_deg
        home = c.has_home ? SDRKitPosition(c.home) : nil
        pilot = c.has_pilot ? SDRKitPosition(c.pilot) : nil
        seq = c.seq
        stateInfo = c.state_info
        version = c.version
        innerCrcOk = c.inner_crc_ok
        vNorth = c.v_north
        vEast = c.v_east
        vUp = c.v_up
        yaw = c.yaw
        gpsTime = c.gps_time
        deviceType = c.device_type
        uuid = c.has_uuid ? sdrkitText(c.uuid) : nil
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        snrDb = c.snr_db
        track = c.track
        trailPoints = c.trail_points
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A burst in the burst log.
public struct DjiDroneidBurstRow {
    public enum Droneid: Int32 {
        case notTried = 0
        case noDroneid = 1
        case crcFailed = 2
        case decoded = 3
    }
    /// The row key.
    public var key: String
    /// The signal time of a burst.
    public var timeS: Double
    /// The wall time of a burst.
    public var time: Double?
    /// The track number.
    public var track: Int64
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// The width.
    public var widthHz: Double
    /// The burst length.
    public var burst: Double
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// How OFDM-like the bursts are.
    public var ofdmPct: Double
    /// The DroneID outcome of a burst.
    public var droneid: Droneid?

    init(_ c: sdrkit_dji_droneid_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        time = c.has_time ? c.time : nil
        track = c.track
        offsetHz = c.offset_hz
        widthHz = c.width_hz
        burst = c.burst
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        ofdmPct = c.ofdm_pct
        droneid = Droneid(rawValue: c.droneid)
    }
}

/// A DJI aircraft heard on DroneID, in any format.
public struct DjiDroneidAircraftRow {
    public enum Format: Int32 {
        case readable = 0
        case encrypted = 1
        case encryptedCryp = 2
        case empty = 3
    }
    /// The row key.
    public var key: String
    /// The name of the row: the serial number of a drone with a readable packet, `session-` and the session key of an aircraft that sends the encrypted format, `empty frames` for the sender of empty frames.
    public var aircraft: String
    /// The 4 bytes of the encrypted format that are constant for one aircraft, as 8 hex digits. They tell two aircraft apart within a session. They are not a serial number and do not identify the aircraft.
    public var sessionKey: String?
    /// The drone of a readable packet: a row key of the `drones` table.
    public var drone: String?
    /// The format of the last frame.
    public var format: Format?
    /// The type byte of the last frame of the encrypted format (0x10, 0x11 and 0x13 were seen).
    public var frameType: Int64?
    /// The frames received.
    public var frames: Int64
    /// The signal time of the first frame.
    public var firstS: Double
    /// The signal time of the last frame.
    public var lastS: Double
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The radio frequency.
    public var freqHz: Double?
    /// Wall-clock time when first heard.
    public var firstHeard: Double?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_dji_droneid_aircraft_row) {
        key = sdrkitText(c.key)
        aircraft = sdrkitText(c.aircraft)
        sessionKey = c.has_session_key ? sdrkitText(c.session_key) : nil
        drone = c.has_drone ? sdrkitText(c.drone) : nil
        format = Format(rawValue: c.format)
        frameType = c.has_frame_type ? c.frame_type : nil
        frames = c.frames
        firstS = c.first_s
        lastS = c.last_s
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of FLARM.
public struct FlarmFields {
    public enum KeyTime: Int32 {
        case clock = 0
        case startPlusSignal = 1
    }
    /// Old-protocol packets decoded.
    public var v6Decoded: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Syncs taken.
    public var syncs: Int64
    /// Good frames on 868.2 MHz.
    public var frames8682: Int64
    /// Good frames on 868.4 MHz.
    public var frames8684: Int64
    /// Newer-protocol packets.
    public var newerPackets: Int64
    /// Newer-protocol devices.
    public var newerDevices: Int64
    /// Packets of other types.
    public var otherType: Int64
    /// Privacy-flagged aircraft.
    public var privacyAircraft: Int64
    /// Privacy-flagged packets.
    public var privacyPackets: Int64
    /// Aircraft in the table.
    public var aircraft: Int64
    /// Aircraft with a position.
    public var withPosition: Int64
    /// The farthest range seen.
    public var farthestKm: Double
    /// Packets a second, over the last second.
    public var packetsPerS: Double
    /// Where the key time comes from.
    public var keyTime: KeyTime?
    /// The station position is missing: without it the packets give no positions.
    public var needsStation: Bool
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Noise floor, dBFS.
    public var noiseDbfs: Double?

    init(_ c: sdrkit_flarm_fields) {
        v6Decoded = c.v6_decoded
        framesBad = c.frames_bad
        syncs = c.syncs
        frames8682 = c.frames_868_2
        frames8684 = c.frames_868_4
        newerPackets = c.newer_packets
        newerDevices = c.newer_devices
        otherType = c.other_type
        privacyAircraft = c.privacy_aircraft
        privacyPackets = c.privacy_packets
        aircraft = c.aircraft
        withPosition = c.with_position
        farthestKm = c.farthest_km
        packetsPerS = c.packets_per_s
        keyTime = KeyTime(rawValue: c.key_time)
        needsStation = c.needs_station
        snrDb = c.has_snr_db ? c.snr_db : nil
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FlarmFields, SDRKitCode> {
        var c = sdrkit_flarm_fields()
        let code = sdrkit_flarm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FlarmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An aircraft heard on FLARM.
public struct FlarmAircraftRow {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// The row key.
    public var key: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// Address type code (0 random, 1 ICAO, 2 FLARM, 3 anonymous).
    public var addressType: Int64
    /// Aircraft type code (0 to 15).
    public var type: Int64
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// Altitude.
    public var altM: Double
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double
    /// Climb.
    public var climbMS: Double
    /// Distance from the station.
    public var distanceKm: Double?
    /// Bearing from the station.
    public var bearingDeg: Double?
    /// The radio channel.
    public var channelHz: Double
    /// Level over the noise.
    public var levelDb: Double
    /// Airborne.
    public var airborne: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// Times heard.
    public var heard: Int64?

    init(_ c: sdrkit_flarm_aircraft_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        addressType = c.address_type
        type = c.type
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.alt_m
        speedKt = c.speed_kt
        courseDeg = c.course_deg
        climbMS = c.climb_m_s
        distanceKm = c.has_distance_km ? c.distance_km : nil
        bearingDeg = c.has_bearing_deg ? c.bearing_deg : nil
        channelHz = c.channel_hz
        levelDb = c.level_db
        airborne = c.airborne
        urgency = Urgency(rawValue: c.urgency)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A track point of an aircraft. The row key is the aircraft key and the signal time of the point.
public struct DronesAircraftTrackRow {
    /// The row key.
    public var key: String
    /// The aircraft a track point belongs to.
    public var aircraft: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The signal time of a track point.
    public var timeS: Double

    init(_ c: sdrkit_drones_aircraft_track_row) {
        key = sdrkitText(c.key)
        aircraft = sdrkitText(c.aircraft)
        position = SDRKitPosition(c.position)
        timeS = c.time_s
    }
}

/// The result fields of ADS-L.
public struct AdslFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Packets with good parity on the band.
    public var packets8682: Int64
    /// Packets with good parity on the band.
    public var packets8684: Int64
    /// Packets with good parity on the band.
    public var packetsLdr: Int64
    /// Packets with good parity on the band.
    public var packetsHdr: Int64
    /// The band is inside the capture and received.
    public var on8682: Bool
    /// The band is inside the capture and received.
    public var on8684: Bool
    /// The band is inside the capture and received.
    public var onLdr: Bool
    /// The band is inside the capture and received.
    public var onHdr: Bool
    /// Traffic packets.
    public var traffic: Int64
    /// Status packets.
    public var status: Int64
    /// Traffic Uplink packets.
    public var uplink: Int64
    /// FIS-B Uplink packets.
    public var fisB: Int64
    /// Remote Identification packets.
    public var remoteId: Int64
    /// Packets of other payload types.
    public var other: Int64
    /// Packets with a reserved key index: their content is not read.
    public var encrypted: Int64
    /// Packets with the signature flag. The signature is not checked: no register of public keys exists.
    public var signed: Int64
    /// Packets that needed a repair.
    public var corrected: Int64
    /// Syncs taken.
    public var syncs: Int64
    /// Noise floor, dBFS.
    public var noiseDbfs: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Good packets a second, over the last second.
    public var packetsPerS: Double
    /// Aircraft in the table.
    public var aircraft: Int64
    /// Aircraft with a position.
    public var withPosition: Int64
    /// The farthest range seen.
    public var maxRangeKm: Double
    /// FLARM packets decoded from the same capture.
    public var flarmPackets: Int64

    init(_ c: sdrkit_adsl_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        packets8682 = c.packets_868_2
        packets8684 = c.packets_868_4
        packetsLdr = c.packets_ldr
        packetsHdr = c.packets_hdr
        on8682 = c.on_868_2
        on8684 = c.on_868_4
        onLdr = c.on_ldr
        onHdr = c.on_hdr
        traffic = c.traffic
        status = c.status
        uplink = c.uplink
        fisB = c.fis_b
        remoteId = c.remote_id
        other = c.other
        encrypted = c.encrypted
        signed = c.signed_
        corrected = c.corrected
        syncs = c.syncs
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        packetsPerS = c.packets_per_s
        aircraft = c.aircraft
        withPosition = c.with_position
        maxRangeKm = c.max_range_km
        flarmPackets = c.flarm_packets
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AdslFields, SDRKitCode> {
        var c = sdrkit_adsl_fields()
        let code = sdrkit_adsl_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AdslFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An aircraft heard on ADS-L.
public struct AdslAircraftRow {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// The row key.
    public var key: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// Where the address of an aircraft comes from.
    public var addressSource: String
    /// The aircraft category.
    public var category: String
    /// The flight state.
    public var flightState: String
    /// The emergency status.
    public var emergency: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The drone's altitude.
    public var altM: Double?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// Climb.
    public var climbMS: Double?
    /// Level over the noise.
    public var levelDb: Double
    /// Distance from the station.
    public var distanceKm: Double?
    /// Bearing from the station.
    public var bearingDeg: Double?
    /// The radio channel.
    public var channelHz: Double
    /// The channel of the last packet.
    public var channel: String
    /// Last heard through a ground station or a relay.
    public var relayed: Bool
    /// The navigation integrity category.
    public var nic: Int64
    /// The horizontal position accuracy category.
    public var hfom: Int64
    /// The vertical position accuracy category.
    public var vfom: Int64
    /// The velocity accuracy category.
    public var velocityAccuracy: Int64
    /// The capabilities of a Status packet.
    public var capabilities: String
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    public var `private`: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// Times heard.
    public var heard: Int64?

    init(_ c: sdrkit_adsl_aircraft_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        addressSource = sdrkitText(c.address_source)
        category = sdrkitText(c.category)
        flightState = sdrkitText(c.flight_state)
        emergency = sdrkitText(c.emergency)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.has_alt_m ? c.alt_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        levelDb = c.level_db
        distanceKm = c.has_distance_km ? c.distance_km : nil
        bearingDeg = c.has_bearing_deg ? c.bearing_deg : nil
        channelHz = c.channel_hz
        channel = sdrkitText(c.channel)
        relayed = c.relayed
        nic = c.nic
        hfom = c.hfom
        vfom = c.vfom
        velocityAccuracy = c.velocity_accuracy
        capabilities = sdrkitText(c.capabilities)
        `private` = c.private_
        urgency = Urgency(rawValue: c.urgency)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// The result fields of the OGN tracker.
public struct OgnFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Sync words found.
    public var syncs: Int64
    /// Packets that needed a repair.
    public var corrected: Int64
    /// Packets with an encrypted position.
    public var encrypted: Int64
    /// Trackers in the table.
    public var trackers: Int64
    /// The channels of the plan inside the capture.
    public var channelsOn: Int64
    /// The channels of the plan and which are received.
    public var channels: String
    /// Noise floor, dBFS.
    public var noiseDbfs: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_ogn_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        syncs = c.syncs
        corrected = c.corrected
        encrypted = c.encrypted
        trackers = c.trackers
        channelsOn = c.channels_on
        channels = sdrkitText(c.channels)
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<OgnFields, SDRKitCode> {
        var c = sdrkit_ogn_fields()
        let code = sdrkit_ogn_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(OgnFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A packet in the packet log.
public struct OgnPacketRow {
    public enum Kind: Int32 {
        case position = 0
        case status = 1
        case encrypted = 2
    }
    /// The row key.
    public var key: String
    /// The signal time at the end of a packet.
    public var timeS: Double
    /// What a packet is.
    public var kind: Kind?
    /// The address and the name of its type.
    public var id: String
    /// A packet in one line.
    public var decoded: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The radio channel.
    public var channelHz: Double
    /// The repair changed a bit of the packet.
    public var repaired: Bool
    /// The 26 bytes of a packet after the repair.
    public var hex: String

    init(_ c: sdrkit_ogn_packet_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        id = sdrkitText(c.id)
        decoded = sdrkitText(c.decoded)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        channelHz = c.channel_hz
        repaired = c.repaired
        hex = sdrkitText(c.hex)
    }
}

/// An aircraft heard on OGN.
public struct OgnAircraftRow {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    /// The row key.
    public var key: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The name of the address type.
    public var addressType: String
    /// The aircraft type.
    public var aircraftType: String?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The drone's altitude.
    public var altM: Double?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// Climb.
    public var climbMS: Double?
    /// The turn rate.
    public var turnDegS: Double?
    /// The count of relays.
    public var relay: Int64
    /// The sender does not want to be shown by other trackers.
    public var stealth: Bool?
    /// The fix quality, 0 to 3.
    public var fixQuality: Int64?
    /// The second of the minute of the last position.
    public var second: Int64?
    /// Level over the noise.
    public var levelDb: Double
    /// The radio channel.
    public var channelHz: Double
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    public var `private`: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// Times heard.
    public var heard: Int64?
    /// Wall-clock time when first heard.
    public var firstHeard: Double?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_ogn_aircraft_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        addressType = sdrkitText(c.address_type)
        aircraftType = c.has_aircraft_type ? sdrkitText(c.aircraft_type) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.has_alt_m ? c.alt_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        turnDegS = c.has_turn_deg_s ? c.turn_deg_s : nil
        relay = c.relay
        stealth = c.has_stealth ? c.stealth : nil
        fixQuality = c.has_fix_quality ? c.fix_quality : nil
        second = c.has_second ? c.second : nil
        levelDb = c.level_db
        channelHz = c.channel_hz
        `private` = c.private_
        urgency = Urgency(rawValue: c.urgency)
        heard = c.has_heard ? c.heard : nil
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the link stacks.
public struct DronesLinkFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Bursts analysed.
    public var bursts: Int64
    /// Bursts without a frame.
    public var burstsWithoutFrame: Int64
    /// 0 nothing, 1 bursts seen, 2 frames decoded lately.
    public var state: Int64
    /// Links in the table.
    public var linksHeard: Int64
    /// The span of the capture that is searched.
    public var covered: String
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_drones_link_fields) {
        framesOk = c.frames_ok
        bursts = c.bursts
        burstsWithoutFrame = c.bursts_without_frame
        state = c.state
        linksHeard = c.links_heard
        covered = sdrkitText(c.covered)
        snrDb = c.has_snr_db ? c.snr_db : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DronesLinkFields, SDRKitCode> {
        var c = sdrkit_drones_link_fields()
        let code = sdrkit_drones_link_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DronesLinkFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct DronesDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_drones_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DronesDecoderFields, SDRKitCode> {
        var c = sdrkit_drones_decoder_fields()
        let code = sdrkit_drones_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DronesDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The PPM fields of `rc_links` (the submode `ppm72`). The measurements have no value in the other submodes and before a good frame.
public struct RcLinksPpmFields {
    public enum PulseState: Int32 {
        case carrierLow = 0
        case carrierHigh = 1
    }
    /// The channels of the last good frame.
    public var channels: Int64?
    /// The last good frame: the sync and the channels.
    public var frameMs: Double?
    /// The frames per second.
    public var frameRateHz: Double?
    /// The carrier state of the pulses.
    public var pulseState: PulseState?
    /// The channel times of the last good frame in ms, separated by commas.
    public var channelMs: String?
    /// The channels in percent of travel, separated by commas: (ms - centre) x 200.
    public var channelPct: String?
    /// The good frames: as many channels as the frame before.
    public var ppmFramesOk: Int64
    /// The bad frames: a channel out of its window, or 1 or more than 12 channels.
    public var ppmFramesBad: Int64

    init(_ c: sdrkit_rc_links_ppm_fields) {
        channels = c.has_channels ? c.channels : nil
        frameMs = c.has_frame_ms ? c.frame_ms : nil
        frameRateHz = c.has_frame_rate_hz ? c.frame_rate_hz : nil
        pulseState = c.has_pulse_state ? PulseState(rawValue: c.pulse_state) : nil
        channelMs = c.has_channel_ms ? sdrkitText(c.channel_ms) : nil
        channelPct = c.has_channel_pct ? sdrkitText(c.channel_pct) : nil
        ppmFramesOk = c.ppm_frames_ok
        ppmFramesBad = c.ppm_frames_bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RcLinksPpmFields, SDRKitCode> {
        var c = sdrkit_rc_links_ppm_fields()
        let code = sdrkit_rc_links_ppm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RcLinksPpmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A link heard.
public struct DronesHeardLinkRow {
    /// The row key.
    public var key: String
    /// The link name.
    public var link: String
    /// The transmitter id.
    public var id: String
    /// Frames of a link.
    public var frames: Int64
    /// Wall-clock time when first heard.
    public var firstHeard: Double?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?
    /// The type of the last frame of a link.
    public var lastType: String
    /// The fields of the last frame of a link.
    public var lastDetail: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The altitude of the last position.
    public var altM: Double?

    init(_ c: sdrkit_drones_heard_link_row) {
        key = sdrkitText(c.key)
        link = sdrkitText(c.link)
        id = sdrkitText(c.id)
        frames = c.frames
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        lastType = sdrkitText(c.last_type)
        lastDetail = sdrkitText(c.last_detail)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.has_alt_m ? c.alt_m : nil
    }
}

/// A frame in the frame log.
public struct DronesLinkFrameRow {
    /// The row key.
    public var key: String
    /// The wall time of a frame.
    public var utc: Double?
    /// The signal time of a frame.
    public var timeS: Double
    /// The link name.
    public var link: String
    /// The transmitter id.
    public var id: String
    /// What a frame is.
    public var type: String
    /// The fields of a frame.
    public var detail: String
    /// The first bytes of a frame as hex.
    public var hex: String
    /// The frequency of a frame.
    public var freqHz: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The payload is encrypted.
    public var encrypted: Bool

    init(_ c: sdrkit_drones_link_frame_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        link = sdrkitText(c.link)
        id = sdrkitText(c.id)
        type = sdrkitText(c.type)
        detail = sdrkitText(c.detail)
        hex = sdrkitText(c.hex)
        freqHz = c.freq_hz
        snrDb = c.snr_db
        encrypted = c.encrypted
    }
}

/// A row of the PPM frame table.
public struct RcLinksPpmFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the sync that ended the frame.
    public var timeS: Double
    /// The channels.
    public var channels: Int64
    /// The channel times in ms, separated by commas.
    public var valuesMs: String
    /// The frame: the sync and the channels.
    public var frameMs: Double
    /// The peak over the floor of the envelope, 20 log10.
    public var levelDb: Double

    init(_ c: sdrkit_rc_links_ppm_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        channels = c.channels
        valuesMs = sdrkitText(c.values_ms)
        frameMs = c.frame_ms
        levelDb = c.level_db
    }
}

/// A drone with a position, at most every 5 s for each drone. The urgency is emergency when the drone states the emergency status.
public struct RemoteIdDroneEvent {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The broadcast address.
    public var address: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// Geodetic altitude.
    public var altGeoM: Double?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The operator's position.
    public var operatorPosition: SDRKitPosition?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_remote_id_drone_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        address = sdrkitText(c.address)
        position = SDRKitPosition(c.position)
        altGeoM = c.has_alt_geo_m ? c.alt_geo_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        operatorPosition = c.has_operator_position ? SDRKitPosition(c.operator_position) : nil
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RemoteIdDroneEvent, SDRKitCode> {
        var c = sdrkit_remote_id_drone_event()
        let code = sdrkit_remote_id_drone_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RemoteIdDroneEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A change of the trust verdict of a Remote ID drone. The urgency is notice when the verdict is suspicious or spoofed, else none.
public struct RemoteIdVerdictEvent {
    public enum Verdict: Int32 {
        case authenticated = 0
        case plausible = 1
        case suspicious = 2
        case spoofed = 3
    }
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The broadcast address.
    public var address: String
    /// The new trust verdict.
    public var verdict: Verdict?
    /// The active reasons of the verdict: their stable names joined by ", ". Empty when there is none.
    public var reasons: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_remote_id_verdict_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        address = sdrkitText(c.address)
        verdict = Verdict(rawValue: c.verdict)
        reasons = sdrkitText(c.reasons)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RemoteIdVerdictEvent, SDRKitCode> {
        var c = sdrkit_remote_id_verdict_event()
        let code = sdrkit_remote_id_verdict_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RemoteIdVerdictEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A DroneID packet.
public struct DjiDroneidEvent {
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
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet.
    public var altM: Double
    /// The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude.
    public var heightM: Double
    /// The altitude field as received (a signed 16-bit number), whose unit is not confirmed.
    public var altRaw: Int64
    /// The height field as received (a signed 16-bit number), whose unit is not confirmed.
    public var heightRaw: Int64
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double
    /// The home position.
    public var home: SDRKitPosition?
    /// The pilot's position.
    public var pilot: SDRKitPosition?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_dji_droneid_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.alt_m
        heightM = c.height_m
        altRaw = c.alt_raw
        heightRaw = c.height_raw
        speedKt = c.speed_kt
        courseDeg = c.course_deg
        home = c.has_home ? SDRKitPosition(c.home) : nil
        pilot = c.has_pilot ? SDRKitPosition(c.pilot) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DjiDroneidEvent, SDRKitCode> {
        var c = sdrkit_dji_droneid_event()
        let code = sdrkit_dji_droneid_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DjiDroneidEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A DJI aircraft that was not heard before.
public struct DjiDroneidDjiAircraftEvent {
    public enum Format: Int32 {
        case readable = 0
        case encrypted = 1
        case encryptedCryp = 2
        case empty = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The 4 bytes of the encrypted format that are constant for one aircraft, as 8 hex digits. They tell two aircraft apart within a session. They are not a serial number and do not identify the aircraft.
    public var sessionKey: String?
    /// The format of the frame.
    public var format: Format?
    /// The type byte of a frame of the encrypted format.
    public var frameType: Int64?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_dji_droneid_dji_aircraft_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        sessionKey = c.has_session_key ? sdrkitText(c.session_key) : nil
        format = Format(rawValue: c.format)
        frameType = c.has_frame_type ? c.frame_type : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        snrDb = c.snr_db
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DjiDroneidDjiAircraftEvent, SDRKitCode> {
        var c = sdrkit_dji_droneid_dji_aircraft_event()
        let code = sdrkit_dji_droneid_dji_aircraft_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DjiDroneidDjiAircraftEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An aircraft heard on FLARM.
public struct FlarmAircraftEvent {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// Aircraft type code (0 to 15).
    public var type: Int64
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// Altitude.
    public var altM: Double
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double
    /// Climb.
    public var climbMS: Double
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_flarm_aircraft_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        type = c.type
        position = SDRKitPosition(c.position)
        altM = c.alt_m
        speedKt = c.speed_kt
        courseDeg = c.course_deg
        climbMS = c.climb_m_s
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FlarmAircraftEvent, SDRKitCode> {
        var c = sdrkit_flarm_aircraft_event()
        let code = sdrkit_flarm_aircraft_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FlarmAircraftEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An aircraft heard on ADS-L.
public struct AdslAircraftEvent {
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
    /// Where the address of an aircraft comes from.
    public var addressSource: String
    /// The aircraft category.
    public var category: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    public var `private`: Bool
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_adsl_aircraft_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        addressSource = sdrkitText(c.address_source)
        category = sdrkitText(c.category)
        position = SDRKitPosition(c.position)
        `private` = c.private_
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AdslAircraftEvent, SDRKitCode> {
        var c = sdrkit_adsl_aircraft_event()
        let code = sdrkit_adsl_aircraft_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AdslAircraftEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An ADS-L aircraft in an emergency.
public struct AdslEmergencyEvent {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The text of a decoded message.
    public var text: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    public var `private`: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_adsl_emergency_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        `private` = c.private_
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AdslEmergencyEvent, SDRKitCode> {
        var c = sdrkit_adsl_emergency_event()
        let code = sdrkit_adsl_emergency_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AdslEmergencyEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An OGN tracker heard.
public struct OgnTrackerEvent {
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
    /// The name of the address type.
    public var addressType: String
    /// The aircraft type.
    public var aircraftType: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    public var `private`: Bool
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_ogn_tracker_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        addressType = sdrkitText(c.address_type)
        aircraftType = sdrkitText(c.aircraft_type)
        position = SDRKitPosition(c.position)
        `private` = c.private_
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<OgnTrackerEvent, SDRKitCode> {
        var c = sdrkit_ogn_tracker_event()
        let code = sdrkit_ogn_tracker_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(OgnTrackerEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An OGN tracker in an emergency.
public struct OgnEmergencyEvent {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The text of a decoded message.
    public var text: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    public var `private`: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_ogn_emergency_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        `private` = c.private_
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<OgnEmergencyEvent, SDRKitCode> {
        var c = sdrkit_ogn_emergency_event()
        let code = sdrkit_ogn_emergency_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(OgnEmergencyEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A new radio-control link.
public struct RcLinksLinkEvent {
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
    /// The link name.
    public var link: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_rc_links_link_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        link = sdrkitText(c.link)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RcLinksLinkEvent, SDRKitCode> {
        var c = sdrkit_rc_links_link_event()
        let code = sdrkit_rc_links_link_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RcLinksLinkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A good PPM frame.
public struct RcLinksRcPpmFrameEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the sync that ended the frame.
    public var timeS: Double
    /// The channels.
    public var channels: Int64
    /// The channel times in ms, separated by commas.
    public var valuesMs: String
    /// The frame: the sync and the channels.
    public var frameMs: Double
    /// The peak over the floor of the envelope, 20 log10.
    public var levelDb: Double

    init(_ c: sdrkit_rc_links_rc_ppm_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        channels = c.channels
        valuesMs = sdrkitText(c.values_ms)
        frameMs = c.frame_ms
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RcLinksRcPpmFrameEvent, SDRKitCode> {
        var c = sdrkit_rc_links_rc_ppm_frame_event()
        let code = sdrkit_rc_links_rc_ppm_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RcLinksRcPpmFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A new MAVLink vehicle.
public struct MavlinkSikVehicleEvent {
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
    /// The link name.
    public var link: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_mavlink_sik_vehicle_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        link = sdrkitText(c.link)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MavlinkSikVehicleEvent, SDRKitCode> {
        var c = sdrkit_mavlink_sik_vehicle_event()
        let code = sdrkit_mavlink_sik_vehicle_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MavlinkSikVehicleEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum DronesTables {
    /// The changed rows of the table "drones" of the stack "remote_id", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_geo_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func remoteIdDrones(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[RemoteIdDroneRow], SDRKitCode> {
        var rows = [sdrkit_remote_id_drone_row](repeating: sdrkit_remote_id_drone_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_remote_id_drones_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RemoteIdDroneRow.init))
    }
    /// The changed rows of the table "messages" of the stack "remote_id", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func remoteIdMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[RemoteIdMessageRow], SDRKitCode> {
        var rows = [sdrkit_remote_id_message_row](repeating: sdrkit_remote_id_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_remote_id_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RemoteIdMessageRow.init))
    }
    /// The changed rows of the table "drone_tracks" of the stack "remote_id", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func remoteIdDroneTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 24000) -> Result<[DronesDroneTrackRow], SDRKitCode> {
        var rows = [sdrkit_drones_drone_track_row](repeating: sdrkit_drones_drone_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_remote_id_drone_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesDroneTrackRow.init))
    }
    /// The changed rows of the table "signals" of the stack "dji_droneid", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func djiDroneidSignals(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[DjiDroneidSignalRow], SDRKitCode> {
        var rows = [sdrkit_dji_droneid_signal_row](repeating: sdrkit_dji_droneid_signal_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dji_droneid_signals_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DjiDroneidSignalRow.init))
    }
    /// The changed rows of the table "drones" of the stack "dji_droneid", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func djiDroneidDrones(_ handle: SDRKIT_HANDLE, capacity: Int = 50) -> Result<[DjiDroneidDroneRow], SDRKitCode> {
        var rows = [sdrkit_dji_droneid_drone_row](repeating: sdrkit_dji_droneid_drone_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dji_droneid_drones_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DjiDroneidDroneRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "dji_droneid", newest first, at most `capacity` rows.
    public static func djiDroneidBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DjiDroneidBurstRow], SDRKitCode> {
        var rows = [sdrkit_dji_droneid_burst_row](repeating: sdrkit_dji_droneid_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dji_droneid_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DjiDroneidBurstRow.init))
    }
    /// The changed rows of the table "drone_tracks" of the stack "dji_droneid", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func djiDroneidDroneTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 6000) -> Result<[DronesDroneTrackRow], SDRKitCode> {
        var rows = [sdrkit_drones_drone_track_row](repeating: sdrkit_drones_drone_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dji_droneid_drone_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesDroneTrackRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "dji_droneid", newest first, at most `capacity` rows.
    /// The column `aircraft` names a row.
    public static func djiDroneidAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 50) -> Result<[DjiDroneidAircraftRow], SDRKitCode> {
        var rows = [sdrkit_dji_droneid_aircraft_row](repeating: sdrkit_dji_droneid_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dji_droneid_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DjiDroneidAircraftRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "flarm", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func flarmAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[FlarmAircraftRow], SDRKitCode> {
        var rows = [sdrkit_flarm_aircraft_row](repeating: sdrkit_flarm_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_flarm_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FlarmAircraftRow.init))
    }
    /// The changed rows of the table "aircraft_tracks" of the stack "flarm", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func flarmAircraftTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 25600) -> Result<[DronesAircraftTrackRow], SDRKitCode> {
        var rows = [sdrkit_drones_aircraft_track_row](repeating: sdrkit_drones_aircraft_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_flarm_aircraft_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesAircraftTrackRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "adsl", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func adslAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[AdslAircraftRow], SDRKitCode> {
        var rows = [sdrkit_adsl_aircraft_row](repeating: sdrkit_adsl_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsl_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdslAircraftRow.init))
    }
    /// The changed rows of the table "aircraft_tracks" of the stack "adsl", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func adslAircraftTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 25600) -> Result<[DronesAircraftTrackRow], SDRKitCode> {
        var rows = [sdrkit_drones_aircraft_track_row](repeating: sdrkit_drones_aircraft_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsl_aircraft_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesAircraftTrackRow.init))
    }
    /// The changed rows of the table "packets" of the stack "ogn", newest first, at most `capacity` rows.
    /// The column `hex` names a row.
    /// Place of a row: the column `position`.
    public static func ognPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[OgnPacketRow], SDRKitCode> {
        var rows = [sdrkit_ogn_packet_row](repeating: sdrkit_ogn_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ogn_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OgnPacketRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "ogn", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func ognAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[OgnAircraftRow], SDRKitCode> {
        var rows = [sdrkit_ogn_aircraft_row](repeating: sdrkit_ogn_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ogn_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OgnAircraftRow.init))
    }
    /// The changed rows of the table "aircraft_tracks" of the stack "ogn", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func ognAircraftTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 51200) -> Result<[DronesAircraftTrackRow], SDRKitCode> {
        var rows = [sdrkit_drones_aircraft_track_row](repeating: sdrkit_drones_aircraft_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ogn_aircraft_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesAircraftTrackRow.init))
    }
    /// The changed rows of the table "links" of the stack "rc_links", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    public static func rcLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DronesHeardLinkRow], SDRKitCode> {
        var rows = [sdrkit_drones_heard_link_row](repeating: sdrkit_drones_heard_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rc_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesHeardLinkRow.init))
    }
    /// The changed rows of the table "frames" of the stack "rc_links", newest first, at most `capacity` rows.
    /// The column `hex` names a row.
    public static func rcLinksFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DronesLinkFrameRow], SDRKitCode> {
        var rows = [sdrkit_drones_link_frame_row](repeating: sdrkit_drones_link_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rc_links_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesLinkFrameRow.init))
    }
    /// The changed rows of the table "ppm_frames" of the stack "rc_links", newest first, at most `capacity` rows.
    public static func rcLinksPpmFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[RcLinksPpmFrameRow], SDRKitCode> {
        var rows = [sdrkit_rc_links_ppm_frame_row](repeating: sdrkit_rc_links_ppm_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rc_links_ppm_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RcLinksPpmFrameRow.init))
    }
    /// The changed rows of the table "links" of the stack "mavlink_sik", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    public static func mavlinkSikLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DronesHeardLinkRow], SDRKitCode> {
        var rows = [sdrkit_drones_heard_link_row](repeating: sdrkit_drones_heard_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mavlink_sik_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesHeardLinkRow.init))
    }
    /// The changed rows of the table "frames" of the stack "mavlink_sik", newest first, at most `capacity` rows.
    /// The column `hex` names a row.
    public static func mavlinkSikFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DronesLinkFrameRow], SDRKitCode> {
        var rows = [sdrkit_drones_link_frame_row](repeating: sdrkit_drones_link_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mavlink_sik_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DronesLinkFrameRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum DronesCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func remoteIdClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_remote_id_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func remoteIdRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_drone_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_drone_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_remote_id_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func djiDroneidClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dji_droneid_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func djiDroneidRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_drone_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_drone_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dji_droneid_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func flarmClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_flarm_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func flarmRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_aircraft_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_aircraft_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_flarm_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func adslClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_adsl_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func adslRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_aircraft_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_aircraft_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_adsl_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ognClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ogn_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ognRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_aircraft_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_aircraft_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ogn_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func rcLinksClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rc_links_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func rcLinksRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_link_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_link_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_rc_links_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func mavlinkSikClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mavlink_sik_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func mavlinkSikRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_drones_link_args()
        args.size = UInt32(MemoryLayout<sdrkit_drones_link_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_mavlink_sik_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum DronesSettings {
    /// The options of the setting "band" of the stack "remote_id".
    public enum RemoteIdBand: Int32 {
        case ble38WiFi6 = 0
        case wiFi6 = 1
        case wiFi1 = 2
        case wiFi11 = 3
        case ble37WiFi1 = 4
        case ble38 = 5
        case ble39 = 6
    }
    /// Band.
    public static func remoteIdBand(_ handle: SDRKIT_HANDLE) -> Result<RemoteIdBand, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band", &v)
        return code == SDRKIT_OK ? .success(RemoteIdBand(rawValue: v) ?? RemoteIdBand(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdBand(_ handle: SDRKIT_HANDLE, _ value: RemoteIdBand) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band", value.rawValue))
    }
    /// Bluetooth LE.
    public static func remoteIdBle(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "ble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdBle(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "ble", value))
    }
    /// Wi-Fi OFDM beacons.
    public static func remoteIdWifiOfdm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wifi_ofdm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdWifiOfdm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wifi_ofdm", value))
    }
    /// Shared-radio check.
    public static func remoteIdSharedRadio(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "shared_radio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdSharedRadio(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "shared_radio", value))
    }
    /// Decode DroneID.
    public static func djiDroneidDecodeDroneid(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_droneid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidDecodeDroneid(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_droneid", value))
    }
    /// Stamp with this computer's clock.
    public static func djiDroneidWallClock(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wall_clock", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidWallClock(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wall_clock", value))
    }
    /// The options of the setting "droneid_tuning" of the stack "dji_droneid".
    public enum DjiDroneidTuning: Int32 {
        case fixed = 0
        case step = 1
    }
    /// DroneID tuning.
    public static func djiDroneidTuning(_ handle: SDRKIT_HANDLE) -> Result<DjiDroneidTuning, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "droneid_tuning", &v)
        return code == SDRKIT_OK ? .success(DjiDroneidTuning(rawValue: v) ?? DjiDroneidTuning(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidTuning(_ handle: SDRKIT_HANDLE, _ value: DjiDroneidTuning) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "droneid_tuning", value.rawValue))
    }
    /// DroneID dwell.
    public static func djiDroneidDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "droneid_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "droneid_dwell_s", value))
    }
    /// Key time from this computer's clock.
    public static func flarmWallClock(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wall_clock", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFlarmWallClock(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wall_clock", value))
    }
    /// Start time (unix seconds).
    public static func flarmStartTime(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_time", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFlarmStartTime(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_time", value))
    }
    /// Forget aircraft after.
    public static func flarmExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFlarmExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// M-band 1.
    public static func adslM1Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "m1_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslM1Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "m1_hz", value))
    }
    /// M-band 2.
    public static func adslM2Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "m2_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslM2Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "m2_hz", value))
    }
    /// O-band.
    public static func adslOHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "o_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslOHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "o_hz", value))
    }
    /// Forget aircraft after.
    public static func adslExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// Decode FLARM from the same capture.
    public static func adslWithFlarm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "with_flarm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslWithFlarm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "with_flarm", value))
    }
    /// M-band chip detector by tone energy.
    public static func adslToneChips(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tone_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslToneChips(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tone_chips", value))
    }
    /// Key time from this computer's clock.
    public static func adslWallClock(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wall_clock", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslWallClock(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wall_clock", value))
    }
    /// Start time (unix seconds).
    public static func adslStartTime(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_time", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslStartTime(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_time", value))
    }
    /// Channels.
    public static func ognChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channels", value))
    }
    /// Channel spacing.
    public static func ognChannelStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnChannelStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_step_hz", value))
    }
    /// Forget trackers after.
    public static func ognExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// The options of the setting "submode" of the stack "rc_links".
    public enum RcLinksSubmode: Int32 {
        case all = 0
        case frsky = 1
        case flysky = 2
        case sfhss = 3
        case dsm = 4
        case ppm72 = 5
    }
    /// Link.
    public static func rcLinksSubmode(_ handle: SDRKIT_HANDLE) -> Result<RcLinksSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(RcLinksSubmode(rawValue: v) ?? RcLinksSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksSubmode(_ handle: SDRKIT_HANDLE, _ value: RcLinksSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Burst threshold.
    public static func rcLinksThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// The options of the setting "flysky_bit_order" of the stack "rc_links".
    public enum RcLinksFlyskyBitOrder: Int32 {
        case both = 0
        case msb = 1
        case lsb = 2
    }
    /// FlySky bit order.
    public static func rcLinksFlyskyBitOrder(_ handle: SDRKIT_HANDLE) -> Result<RcLinksFlyskyBitOrder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "flysky_bit_order", &v)
        return code == SDRKIT_OK ? .success(RcLinksFlyskyBitOrder(rawValue: v) ?? RcLinksFlyskyBitOrder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksFlyskyBitOrder(_ handle: SDRKIT_HANDLE, _ value: RcLinksFlyskyBitOrder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "flysky_bit_order", value.rawValue))
    }
    /// The options of the setting "dsm_chip_polarity" of the stack "rc_links".
    public enum RcLinksDsmChipPolarity: Int32 {
        case high = 0
        case low = 1
    }
    /// DSM chip polarity.
    public static func rcLinksDsmChipPolarity(_ handle: SDRKIT_HANDLE) -> Result<RcLinksDsmChipPolarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "dsm_chip_polarity", &v)
        return code == SDRKIT_OK ? .success(RcLinksDsmChipPolarity(rawValue: v) ?? RcLinksDsmChipPolarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksDsmChipPolarity(_ handle: SDRKIT_HANDLE, _ value: RcLinksDsmChipPolarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "dsm_chip_polarity", value.rawValue))
    }
    /// The options of the setting "sfhss_crc" of the stack "rc_links".
    public enum RcLinksSfhssCrc: Int32 {
        case either = 0
        case ccitt = 1
        case cc2500 = 2
    }
    /// S-FHSS CRC.
    public static func rcLinksSfhssCrc(_ handle: SDRKIT_HANDLE) -> Result<RcLinksSfhssCrc, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sfhss_crc", &v)
        return code == SDRKIT_OK ? .success(RcLinksSfhssCrc(rawValue: v) ?? RcLinksSfhssCrc(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksSfhssCrc(_ handle: SDRKIT_HANDLE, _ value: RcLinksSfhssCrc) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sfhss_crc", value.rawValue))
    }
    /// Shortest sync gap (PPM).
    public static func rcLinksPpmSyncMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ppm_sync_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmSyncMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ppm_sync_ms", value))
    }
    /// Shortest channel (PPM).
    public static func rcLinksPpmChannelMinMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ppm_channel_min_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmChannelMinMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ppm_channel_min_ms", value))
    }
    /// Longest channel (PPM).
    public static func rcLinksPpmChannelMaxMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ppm_channel_max_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmChannelMaxMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ppm_channel_max_ms", value))
    }
    /// Centre of travel (PPM).
    public static func rcLinksPpmCentreMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ppm_centre_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmCentreMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ppm_centre_ms", value))
    }
    /// Row interval (PPM).
    public static func rcLinksPpmRowS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ppm_row_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmRowS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ppm_row_s", value))
    }
    /// The options of the setting "air_rate" of the stack "mavlink_sik".
    public enum MavlinkSikAirRate: Int32 {
        case auto = 0
        case n2 = 1
        case n4 = 2
        case n8 = 3
        case n16 = 4
        case n19 = 5
        case n24 = 6
        case n32 = 7
        case n48 = 8
        case n64 = 9
        case n96 = 10
        case n128 = 11
        case n192 = 12
        case n250 = 13
    }
    /// Air speed.
    public static func mavlinkSikAirRate(_ handle: SDRKIT_HANDLE) -> Result<MavlinkSikAirRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "air_rate", &v)
        return code == SDRKIT_OK ? .success(MavlinkSikAirRate(rawValue: v) ?? MavlinkSikAirRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMavlinkSikAirRate(_ handle: SDRKIT_HANDLE, _ value: MavlinkSikAirRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "air_rate", value.rawValue))
    }
    /// Burst threshold.
    public static func mavlinkSikThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMavlinkSikThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum DronesGeneratorSettings {
    /// The options of the setting "gen_carriers" of the stack "remote_id".
    public enum RemoteIdCarriers: Int32 {
        case both = 0
        case ble = 1
        case wifi = 2
    }
    /// Test signal carriers.
    public static func remoteIdCarriers(_ handle: SDRKIT_HANDLE) -> Result<RemoteIdCarriers, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_carriers", &v)
        return code == SDRKIT_OK ? .success(RemoteIdCarriers(rawValue: v) ?? RemoteIdCarriers(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdCarriers(_ handle: SDRKIT_HANDLE, _ value: RemoteIdCarriers) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_carriers", value.rawValue))
    }
    /// Test signal radio centre.
    public static func remoteIdCentreMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdCentreMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_mhz", value))
    }
    /// The options of the setting "gen_wifi_phy" of the stack "remote_id".
    public enum RemoteIdWifiPhy: Int32 {
        case dsss = 0
        case ofdm = 1
    }
    /// Test signal Wi-Fi PHY.
    public static func remoteIdWifiPhy(_ handle: SDRKIT_HANDLE) -> Result<RemoteIdWifiPhy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_wifi_phy", &v)
        return code == SDRKIT_OK ? .success(RemoteIdWifiPhy(rawValue: v) ?? RemoteIdWifiPhy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdWifiPhy(_ handle: SDRKIT_HANDLE, _ value: RemoteIdWifiPhy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_wifi_phy", value.rawValue))
    }
    /// Test signal spoofer.
    public static func remoteIdSpoofer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_spoofer", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRemoteIdSpoofer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_spoofer", value))
    }
    /// The options of the setting "gen_scenario" of the stack "dji_droneid".
    public enum DjiDroneidScenario: Int32 {
        case everything = 0
        case droneidOnly = 1
        case ocusyncLikeOnly = 2
        case lightbridgeLikeOnly = 3
        case wiFiLikeOnly = 4
        case droneidWithCollisions = 5
    }
    /// Test signal.
    public static func djiDroneidScenario(_ handle: SDRKIT_HANDLE) -> Result<DjiDroneidScenario, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &v)
        return code == SDRKIT_OK ? .success(DjiDroneidScenario(rawValue: v) ?? DjiDroneidScenario(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidScenario(_ handle: SDRKIT_HANDLE, _ value: DjiDroneidScenario) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scenario", value.rawValue))
    }
    /// DroneID period.
    public static func djiDroneidPeriodS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_period_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidPeriodS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_period_s", value))
    }
    /// 8-symbol DroneID.
    public static func djiDroneidEightSymbols(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_eight_symbols", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidEightSymbols(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_eight_symbols", value))
    }
    /// 20 MHz OcuSync-like.
    public static func djiDroneidWide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidWide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_wide", value))
    }
    /// The options of the setting "gen_frame" of the stack "dji_droneid".
    public enum DjiDroneidFrame: Int32 {
        case readable = 0
        case encrypted = 1
        case empty = 2
    }
    /// DroneID frame.
    public static func djiDroneidFrame(_ handle: SDRKIT_HANDLE) -> Result<DjiDroneidFrame, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_frame", &v)
        return code == SDRKIT_OK ? .success(DjiDroneidFrame(rawValue: v) ?? DjiDroneidFrame(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidFrame(_ handle: SDRKIT_HANDLE, _ value: DjiDroneidFrame) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_frame", value.rawValue))
    }
    /// On a DroneID frequency.
    public static func djiDroneidFrequency(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_droneid_frequency", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidFrequency(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_droneid_frequency", value))
    }
    /// Two aircraft.
    public static func djiDroneidTwoAircraft(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_two_aircraft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidTwoAircraft(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_two_aircraft", value))
    }
    /// Test signal radio centre.
    public static func djiDroneidCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDjiDroneidCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test signal centre.
    public static func flarmCentre(_ handle: SDRKIT_HANDLE) -> Result<SDRKitPosition, SDRKitCode> {
        var v: sdrkit_position = sdrkit_position()
        let code = sdrkit_typed_setting_get_position(handle, "gen_centre", &v)
        return code == SDRKIT_OK ? .success(SDRKitPosition(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFlarmCentre(_ handle: SDRKIT_HANDLE, _ value: SDRKitPosition) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_position(handle, "gen_centre", value.c))
    }
    /// Test signal centre.
    public static func adslCentre(_ handle: SDRKIT_HANDLE) -> Result<SDRKitPosition, SDRKitCode> {
        var v: sdrkit_position = sdrkit_position()
        let code = sdrkit_typed_setting_get_position(handle, "gen_centre", &v)
        return code == SDRKIT_OK ? .success(SDRKitPosition(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslCentre(_ handle: SDRKIT_HANDLE, _ value: SDRKitPosition) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_position(handle, "gen_centre", value.c))
    }
    /// Test signal aircraft.
    public static func adslAircraft(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_aircraft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslAircraft(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value))
    }
    /// Test signal key index.
    public static func adslKeyIndex(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_key_index", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslKeyIndex(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_key_index", value))
    }
    /// Test signal with Reed-Solomon.
    public static func adslFec(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_fec", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslFec(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_fec", value))
    }
    /// Test signal M-band preamble.
    public static func adslPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_preamble", value))
    }
    /// Wrong bit in every n-th packet.
    public static func adslBadBit(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_bad_bit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdslBadBit(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_bad_bit", value))
    }
    /// Test signal centre.
    public static func ognCentre(_ handle: SDRKIT_HANDLE) -> Result<SDRKitPosition, SDRKitCode> {
        var v: sdrkit_position = sdrkit_position()
        let code = sdrkit_typed_setting_get_position(handle, "gen_centre", &v)
        return code == SDRKIT_OK ? .success(SDRKitPosition(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnCentre(_ handle: SDRKIT_HANDLE, _ value: SDRKitPosition) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_position(handle, "gen_centre", value.c))
    }
    /// Test signal trackers.
    public static func ognTrackers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_trackers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnTrackers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_trackers", value))
    }
    /// Wrong bits in every third packet.
    public static func ognBadBits(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_bad_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnBadBits(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_bad_bits", value))
    }
    /// Test signal with an emergency.
    public static func ognEmergency(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_emergency", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOgnEmergency(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_emergency", value))
    }
    /// PPM channels.
    public static func rcLinksChannelsMs(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_channels_ms", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksChannelsMs(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_channels_ms", $0) })
    }
    /// PPM frame.
    public static func rcLinksFrameMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_frame_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksFrameMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_frame_ms", value))
    }
    /// PPM pulse.
    public static func rcLinksPulseUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pulse_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPulseUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pulse_us", value))
    }
    /// PPM timing jitter.
    public static func rcLinksJitterUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_jitter_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksJitterUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_jitter_us", value))
    }
    /// PPM transmitter clock error.
    public static func rcLinksPpmClockPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ppm_clock_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmClockPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ppm_clock_ppm", value))
    }
    /// PPM keying depth.
    public static func rcLinksPpmDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ppm_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ppm_depth", value))
    }
    /// PPM carrier only in the pulse.
    public static func rcLinksPpmInvert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_ppm_invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRcLinksPpmInvert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_ppm_invert", value))
    }
}

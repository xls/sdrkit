// sdrkit: the typed interface of the stack family "gnss", version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum GnssInterface {
    public static let major = 1
    public static let minor = 0
}

/// The result fields of the receiver.
public struct GnssFields {
    public enum SearchSystem: Int32 {
        case gps = 0
        case qzss = 1
        case sbas = 2
        case galileo = 3
        case beidou = 4
        case glonass = 5
    }
    public enum FrequencySource: Int32 {
        case measured = 0
        case searchCentre = 1
        case unknown = 2
    }
    /// The centre frequency of the input.
    public var centreHz: Double
    /// The sample rate of the input.
    public var inputRate: Double
    /// The system that the search works on now.
    public var searchSystem: SearchSystem?
    /// The PRN that the search works on now.
    public var searchPrn: Int64?
    /// The part of the search round done.
    public var searchProgressPct: Double?
    /// The number of the search round, from 1.
    public var searchRound: Int64?
    /// The centre of the search window (the radio's frequency error at L1).
    public var searchCentre: Double?
    /// The half width of the search window.
    public var searchHalfWidth: Double?
    /// The integration of the search.
    public var searchIntegrationMs: Double?
    /// The radio's frequency error at L1, measured by the fix.
    public var frequencyError: Double?
    /// The same error in parts per million of 1575.42 MHz.
    public var frequencyErrorPpm: Double?
    /// Where the frequency error comes from.
    public var frequencySource: FrequencySource?
    /// Signal time of the first lock.
    public var firstLock: Double?
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// SBAS messages with a good CRC.
    public var sbasMessages: Int64
    /// GLONASS time minus GPS time.
    public var glonassTauGpsS: Double?
    /// GLONASS time minus UTC(SU).
    public var glonassTauCS: Double?
    /// BeiDou time minus GPS time, less 14 s.
    public var bdtGpsS: Double?
    /// GPS satellites with an almanac entry.
    public var almanacSatellites: Int64
    /// Whether the ionosphere model comes from the satellites.
    public var ionoBroadcast: Bool
    /// GPS time minus UTC: broadcast, else from the station's leap-second table.
    public var leapSeconds: Int64?
    /// Whether the satellites broadcast the leap seconds.
    public var leapSecondsBroadcast: Bool
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Input samples clipped.
    public var clippedPct: Double
    /// Input DC, in phase (full scale 1).
    public var dcI: Double
    /// Input DC, quadrature (full scale 1).
    public var dcQ: Double
    /// Signal time fed.
    public var signal: Double
    /// What the receiver waits for while it has no fix, for people, one item a line.
    public var progress: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The height of the object that the event is about, metres above mean sea level.
    public var heightM: Double?
    /// Horizontal error, 1 sigma.
    public var errorHM: Double?
    /// Vertical error, 1 sigma.
    public var errorVM: Double?
    /// Horizontal dilution of precision.
    public var hdop: Double?
    /// Vertical dilution of precision.
    public var vdop: Double?
    /// Position dilution of precision.
    public var pdop: Double?
    /// Satellites in the fix.
    public var satellitesUsed: Int64?
    /// Satellites tracked.
    public var satellitesTracked: Int64
    /// UTC of the fix.
    public var utc: Double?
    /// GPS week of the fix.
    public var gpsWeek: Int64?
    /// GPS seconds of the week of the fix.
    public var gpsTowS: Double?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The receiver's clock bias.
    public var clockBiasM: Double?
    /// Signal time of the first fix.
    public var firstFix: Double?

    init(_ c: sdrkit_gnss_fields) {
        centreHz = c.centre_hz
        inputRate = c.input_rate
        searchSystem = c.has_search_system ? SearchSystem(rawValue: c.search_system) : nil
        searchPrn = c.has_search_prn ? c.search_prn : nil
        searchProgressPct = c.has_search_progress_pct ? c.search_progress_pct : nil
        searchRound = c.has_search_round ? c.search_round : nil
        searchCentre = c.has_search_centre ? c.search_centre : nil
        searchHalfWidth = c.has_search_half_width ? c.search_half_width : nil
        searchIntegrationMs = c.has_search_integration_ms ? c.search_integration_ms : nil
        frequencyError = c.has_frequency_error ? c.frequency_error : nil
        frequencyErrorPpm = c.has_frequency_error_ppm ? c.frequency_error_ppm : nil
        frequencySource = FrequencySource(rawValue: c.frequency_source)
        firstLock = c.has_first_lock ? c.first_lock : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        sbasMessages = c.sbas_messages
        glonassTauGpsS = c.has_glonass_tau_gps_s ? c.glonass_tau_gps_s : nil
        glonassTauCS = c.has_glonass_tau_c_s ? c.glonass_tau_c_s : nil
        bdtGpsS = c.has_bdt_gps_s ? c.bdt_gps_s : nil
        almanacSatellites = c.almanac_satellites
        ionoBroadcast = c.iono_broadcast
        leapSeconds = c.has_leap_seconds ? c.leap_seconds : nil
        leapSecondsBroadcast = c.leap_seconds_broadcast
        levelDbfs = c.level_dbfs
        clippedPct = c.clipped_pct
        dcI = c.dc_i
        dcQ = c.dc_q
        signal = c.signal
        progress = sdrkitText(c.progress)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        heightM = c.has_height_m ? c.height_m : nil
        errorHM = c.has_error_h_m ? c.error_h_m : nil
        errorVM = c.has_error_v_m ? c.error_v_m : nil
        hdop = c.has_hdop ? c.hdop : nil
        vdop = c.has_vdop ? c.vdop : nil
        pdop = c.has_pdop ? c.pdop : nil
        satellitesUsed = c.has_satellites_used ? c.satellites_used : nil
        satellitesTracked = c.satellites_tracked
        utc = c.has_utc ? c.utc : nil
        gpsWeek = c.has_gps_week ? c.gps_week : nil
        gpsTowS = c.has_gps_tow_s ? c.gps_tow_s : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        clockBiasM = c.has_clock_bias_m ? c.clock_bias_m : nil
        firstFix = c.has_first_fix ? c.first_fix : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GnssFields, SDRKitCode> {
        var c = sdrkit_gnss_fields()
        let code = sdrkit_gnss_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GnssFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the interference monitor.
public struct GnssInterferenceFields {
    public enum Interference: Int32 {
        case clear = 0
        case raisedNoise = 1
        case interference = 2
    }
    /// The monitor's verdict.
    public var interference: Interference?
    /// A tone that sweeps through the band.
    public var swept: Bool
    /// Pulses.
    public var pulsed: Bool
    /// A narrow line that the receiver does not know.
    public var narrowband: Bool
    /// A raised noise floor.
    public var wideband: Bool
    /// Why the monitor says so, for people.
    public var interferenceReason: String
    /// The noise floor, dBFS in 1 Hz.
    public var noiseFloorDbfs: Double
    /// The band power.
    public var bandPowerDbfs: Double
    /// Spectral flatness, 0 to 1.
    public var flatness: Double
    /// Whether spoofing is suspected.
    public var spoofing: Bool
    /// Why spoofing is suspected, for people.
    public var spoofingReason: String?

    init(_ c: sdrkit_gnss_interference_fields) {
        interference = Interference(rawValue: c.interference)
        swept = c.swept
        pulsed = c.pulsed
        narrowband = c.narrowband
        wideband = c.wideband
        interferenceReason = sdrkitText(c.interference_reason)
        noiseFloorDbfs = c.noise_floor_dbfs
        bandPowerDbfs = c.band_power_dbfs
        flatness = c.flatness
        spoofing = c.spoofing
        spoofingReason = c.has_spoofing_reason ? sdrkitText(c.spoofing_reason) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GnssInterferenceFields, SDRKitCode> {
        var c = sdrkit_gnss_interference_fields()
        let code = sdrkit_gnss_interference_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GnssInterferenceFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.
public struct GnssGuardFields {
    /// Input samples replaced because they were not finite or far beyond full scale.
    public var badSamples: Int64

    init(_ c: sdrkit_gnss_guard_fields) {
        badSamples = c.bad_samples
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GnssGuardFields, SDRKitCode> {
        var c = sdrkit_gnss_guard_fields()
        let code = sdrkit_gnss_guard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GnssGuardFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A satellite system. The row key is the key of the system.
public struct GnssSystemRow {
    public enum System: Int32 {
        case gps = 0
        case qzss = 1
        case sbas = 2
        case galileo = 3
        case beidou = 4
        case glonass = 5
    }
    /// The row key.
    public var key: String
    /// A satellite system.
    public var system: System?
    /// This system is selected for reception.
    public var wanted: Bool
    /// The receiver has a band for this system at the present tune.
    public var active: Bool
    /// The receiver has searched or tracks this system.
    public var searched: Bool

    init(_ c: sdrkit_gnss_system_row) {
        key = sdrkitText(c.key)
        system = System(rawValue: c.system)
        wanted = c.wanted
        active = c.active
        searched = c.searched
    }
}

/// A satellite system in the fix. The row key is the key of the system.
public struct GnssFixSystemRow {
    public enum System: Int32 {
        case gps = 0
        case qzss = 1
        case sbas = 2
        case galileo = 3
        case beidou = 4
        case glonass = 5
    }
    /// The row key.
    public var key: String
    /// A satellite system.
    public var system: System?
    /// Satellites of a system in the fix.
    public var satellites: Int64
    /// The receiver clock of a system in the fix.
    public var clockM: Double

    init(_ c: sdrkit_gnss_fix_system_row) {
        key = sdrkitText(c.key)
        system = System(rawValue: c.system)
        satellites = c.satellites
        clockM = c.clock_m
    }
}

/// A satellite: what the channels, the navigation data, the sky and the orbits say of it.
public struct GnssSatelliteRow {
    public enum System: Int32 {
        case gps = 0
        case qzss = 1
        case sbas = 2
        case galileo = 3
        case beidou = 4
        case glonass = 5
    }
    public enum State: Int32 {
        case pullIn = 0
        case locked = 1
        case bitSync = 2
        case frameSync = 3
        case ephemeris = 4
    }
    public enum Ephemeris: Int32 {
        case held = 0
        case collecting = 1
        case waiting = 2
        case notUsed = 3
    }
    public enum Orbit: Int32 {
        case ephemeris = 0
        case almanac = 1
    }
    /// The row key.
    public var key: String
    /// A satellite system.
    public var system: System?
    /// A PRN or slot number.
    public var prn: Int64?
    /// The frequency channel of a GLONASS satellite.
    public var gloChannel: Int64?
    /// A channel tracks the satellite.
    public var tracked: Bool
    /// The state of the satellite's channel.
    public var state: State?
    /// Carrier to noise density.
    public var cn0Dbhz: Double?
    /// Doppler at the signal's carrier, with the radio's error.
    public var doppler: Double?
    /// Code phase.
    public var codeChips: Double?
    /// Azimuth, degrees from true north.
    public var azimuthDeg: Double?
    /// Elevation.
    public var elevationDeg: Double?
    /// Time since lock was gained, seconds.
    public var lockedFor: Double?
    /// Frames that passed their check.
    public var framesGood: Int64?
    /// Frames received.
    public var frames: Int64?
    /// The residual of the satellite in the fix.
    public var residualM: Double?
    /// The satellite says it is healthy.
    public var healthy: Bool?
    /// The satellite is in the fix.
    public var used: Bool
    /// What the receiver holds of the ephemeris.
    public var ephemeris: Ephemeris?
    /// Ephemeris parts collected.
    public var parts: Int64?
    /// Ephemeris parts needed.
    public var partsOf: Int64?
    /// About how long until the ephemeris is complete.
    public var secondsLeft: Double?
    /// The issue of data (IODE, IODnav, AODE or the GLONASS tb index).
    public var iod: Int64?
    /// The age of the data from its epoch (negative: ahead).
    public var dataAge: Double?
    /// The week of the data.
    public var week: Int64?
    /// The time of week of the last unit.
    public var towS: Double?
    /// The satellite clock af0.
    public var af0S: Double?
    /// The point below the satellite.
    public var subPoint: SDRKitPosition?
    /// The height of the satellite.
    public var heightKm: Double?
    /// Where the satellite's position comes from.
    public var orbit: Orbit?

    init(_ c: sdrkit_gnss_satellite_row) {
        key = sdrkitText(c.key)
        system = c.has_system ? System(rawValue: c.system) : nil
        prn = c.has_prn ? c.prn : nil
        gloChannel = c.has_glo_channel ? c.glo_channel : nil
        tracked = c.tracked
        state = c.has_state ? State(rawValue: c.state) : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
        doppler = c.has_doppler ? c.doppler : nil
        codeChips = c.has_code_chips ? c.code_chips : nil
        azimuthDeg = c.has_azimuth_deg ? c.azimuth_deg : nil
        elevationDeg = c.has_elevation_deg ? c.elevation_deg : nil
        lockedFor = c.has_locked_for ? c.locked_for : nil
        framesGood = c.has_frames_good ? c.frames_good : nil
        frames = c.has_frames ? c.frames : nil
        residualM = c.has_residual_m ? c.residual_m : nil
        healthy = c.has_healthy ? c.healthy : nil
        used = c.used
        ephemeris = c.has_ephemeris ? Ephemeris(rawValue: c.ephemeris) : nil
        parts = c.has_parts ? c.parts : nil
        partsOf = c.has_parts_of ? c.parts_of : nil
        secondsLeft = c.has_seconds_left ? c.seconds_left : nil
        iod = c.has_iod ? c.iod : nil
        dataAge = c.has_data_age ? c.data_age : nil
        week = c.has_week ? c.week : nil
        towS = c.has_tow_s ? c.tow_s : nil
        af0S = c.has_af0_s ? c.af0_s : nil
        subPoint = c.has_sub_point ? SDRKitPosition(c.sub_point) : nil
        heightKm = c.has_height_km ? c.height_km : nil
        orbit = c.has_orbit ? Orbit(rawValue: c.orbit) : nil
    }
}

/// A line of the interference spectrum.
public struct GnssInterferenceLineRow {
    /// The row key.
    public var key: String
    /// The offset of a line from the centre.
    public var offset: Double
    /// A level above the floor.
    public var levelDb: Double
    /// A line that the receiver has identified (a satellite or the radio's own).
    public var known: Bool

    init(_ c: sdrkit_gnss_interference_line_row) {
        key = sdrkitText(c.key)
        offset = c.offset
        levelDb = c.level_db
        known = c.known
    }
}

/// An interference event.
public struct GnssInterferenceEventRow {
    public enum Level: Int32 {
        case info = 0
        case warning = 1
        case alarm = 2
    }
    /// The row key.
    public var key: String
    /// Signal time of an event.
    public var at: Double
    /// The level of an event.
    public var level: Level?
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_gnss_interference_event_row) {
        key = sdrkitText(c.key)
        at = c.at
        level = Level(rawValue: c.level)
        text = sdrkitText(c.text)
    }
}

/// The result fields of the SBAS content decoder.
public struct SbasFields {
    /// A PRN mask and fast corrections are held.
    public var dataValid: Bool
    /// The SBAS messages with a good CRC.
    public var sbasMessages: Int64
    /// The messages missed.
    public var messagesMissed: Int64
    /// Help for people while nothing is heard.
    public var help: String?
    /// The PRN whose corrections the tables show.
    public var selected: Int64?
    /// GEO navigation of the selected PRN: the issue of data.
    public var geoIodn: Int64?
    /// GEO navigation: the time of applicability.
    public var geoT0S: Double?
    /// GEO navigation: the accuracy index.
    public var geoUra: Int64?
    /// GEO navigation: the distance from the centre of the earth.
    public var geoRadiusM: Double?
    /// GEO navigation: the longitude.
    public var geoLongitudeDeg: Double?
    /// Network time of the selected PRN: the week.
    public var sbasWeek: Int64?
    /// Network time: the time of week.
    public var sbasTowS: Int64?
    /// Network time: the leap seconds.
    public var sbasLeapS: Int64?
    /// Network time: the leap seconds after the next change.
    public var sbasLeapNextS: Int64?
    /// Network time: the UTC offset A0.
    public var utcA0S: Double?
    /// Network time: the UTC drift A1.
    public var utcA1: Double?
    /// Network time: the identifier of the UTC standard.
    public var utcId: Int64?

    init(_ c: sdrkit_sbas_fields) {
        dataValid = c.data_valid
        sbasMessages = c.sbas_messages
        messagesMissed = c.messages_missed
        help = c.has_help ? sdrkitText(c.help) : nil
        selected = c.has_selected ? c.selected : nil
        geoIodn = c.has_geo_iodn ? c.geo_iodn : nil
        geoT0S = c.has_geo_t0_s ? c.geo_t0_s : nil
        geoUra = c.has_geo_ura ? c.geo_ura : nil
        geoRadiusM = c.has_geo_radius_m ? c.geo_radius_m : nil
        geoLongitudeDeg = c.has_geo_longitude_deg ? c.geo_longitude_deg : nil
        sbasWeek = c.has_sbas_week ? c.sbas_week : nil
        sbasTowS = c.has_sbas_tow_s ? c.sbas_tow_s : nil
        sbasLeapS = c.has_sbas_leap_s ? c.sbas_leap_s : nil
        sbasLeapNextS = c.has_sbas_leap_next_s ? c.sbas_leap_next_s : nil
        utcA0S = c.has_utc_a0_s ? c.utc_a0_s : nil
        utcA1 = c.has_utc_a1 ? c.utc_a1 : nil
        utcId = c.has_utc_id ? c.utc_id : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SbasFields, SDRKitCode> {
        var c = sdrkit_sbas_fields()
        let code = sdrkit_sbas_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SbasFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the SBAS satellite table. The row key is the PRN.
public struct SbasSatelliteRow {
    public enum Service: Int32 {
        case egnos = 0
        case waas = 1
        case msas = 2
        case gagan = 3
        case sdcm = 4
        case bdsbas = 5
        case kass = 6
    }
    public enum State: Int32 {
        case pullIn = 0
        case locked = 1
        case bitSync = 2
        case frameSync = 3
        case ephemeris = 4
    }
    /// The row key.
    public var key: String
    /// The PRN.
    public var prn: Int64
    /// The service of the PRN.
    public var service: Service?
    /// The state of the channel of the satellite.
    public var state: State?
    /// The carrier to noise density.
    public var cn0Dbhz: Double
    /// The Doppler at the carrier, with the error of the radio.
    public var doppler: Double
    /// The messages received.
    public var messages: Int64
    /// The messages missed.
    public var messagesMissed: Int64

    init(_ c: sdrkit_sbas_satellite_row) {
        key = sdrkitText(c.key)
        prn = c.prn
        service = c.has_service ? Service(rawValue: c.service) : nil
        state = c.has_state ? State(rawValue: c.state) : nil
        cn0Dbhz = c.cn0_dbhz
        doppler = c.doppler
        messages = c.messages
        messagesMissed = c.messages_missed
    }
}

/// A message type that an SBAS satellite sent. The row key is the PRN and the type, as `123/9`.
public struct SbasSatelliteTypeRow {
    /// The row key.
    public var key: String
    /// The SBAS satellite.
    public var prn: Int64
    /// The message type.
    public var messageType: Int64

    init(_ c: sdrkit_sbas_satellite_type_row) {
        key = sdrkitText(c.key)
        prn = c.prn
        messageType = c.message_type
    }
}

/// The count of one message type. The row key is the type.
public struct SbasTypeRow {
    /// The row key.
    public var key: String
    /// The message type.
    public var messageType: Int64
    /// The name of the type.
    public var name: String?
    /// The messages of the type.
    public var count: Int64

    init(_ c: sdrkit_sbas_type_row) {
        key = sdrkitText(c.key)
        messageType = c.message_type
        name = c.has_name ? sdrkitText(c.name) : nil
        count = c.count
    }
}

/// A row of the message log. The row key is the number of the message since the start.
public struct SbasMessageRow {
    /// The row key.
    public var key: String
    /// The signal time of the message.
    public var at: Double
    /// The SBAS satellite.
    public var prn: Int64
    /// The message type.
    public var messageType: Int64
    /// The content in words.
    public var text: String

    init(_ c: sdrkit_sbas_message_row) {
        key = sdrkitText(c.key)
        at = c.at
        prn = c.prn
        messageType = c.message_type
        text = sdrkitText(c.text)
    }
}

/// A fast correction of the selected PRN. The row key is the slot.
public struct SbasFastCorrectionRow {
    public enum System: Int32 {
        case gps = 0
        case qzss = 1
        case sbas = 2
        case galileo = 3
        case beidou = 4
        case glonass = 5
    }
    /// The row key.
    public var key: String
    /// The slot of the PRN mask.
    public var slot: Int64
    /// The system of the corrected satellite.
    public var system: System?
    /// The PRN of the corrected satellite.
    public var prn: Int64?
    /// The fast correction.
    public var correctionM: Double?
    /// The UDRE indicator.
    public var udrei: Int64
    /// The UDRE as one sigma. No value for "not monitored" and "do not use".
    public var udreSigmaM: Double?
    /// The issue of data of the fast corrections.
    public var iodf: Int64
    /// The issue of data of the PRN mask.
    public var iodp: Int64
    /// The correction belongs to the PRN mask held.
    public var iodpCurrent: Bool
    /// The degradation factor.
    public var degradation: Double?

    init(_ c: sdrkit_sbas_fast_correction_row) {
        key = sdrkitText(c.key)
        slot = c.slot
        system = c.has_system ? System(rawValue: c.system) : nil
        prn = c.has_prn ? c.prn : nil
        correctionM = c.has_correction_m ? c.correction_m : nil
        udrei = c.udrei
        udreSigmaM = c.has_udre_sigma_m ? c.udre_sigma_m : nil
        iodf = c.iodf
        iodp = c.iodp
        iodpCurrent = c.iodp_current
        degradation = c.has_degradation ? c.degradation : nil
    }
}

/// An ionosphere band of the selected PRN. The row key is the band.
public struct SbasIonosphereRow {
    /// The row key.
    public var key: String
    /// The band.
    public var band: Int64
    /// The issue of data of the ionosphere.
    public var iodi: Int64
    /// The grid points.
    public var points: Int64
    /// The blocks seen.
    public var blocksSeen: Int64
    /// The blocks needed.
    public var blocksNeeded: Int64
    /// The delays held.
    public var values: Int64
    /// The points not monitored.
    public var notMonitored: Int64
    /// The smallest vertical delay.
    public var minM: Double
    /// The mean vertical delay.
    public var meanM: Double
    /// The largest vertical delay.
    public var maxM: Double
    /// The worst GIVE indicator.
    public var worstGivei: Int64

    init(_ c: sdrkit_sbas_ionosphere_row) {
        key = sdrkitText(c.key)
        band = c.band
        iodi = c.iodi
        points = c.points
        blocksSeen = c.blocks_seen
        blocksNeeded = c.blocks_needed
        values = c.values
        notMonitored = c.not_monitored
        minM = c.min_m
        meanM = c.mean_m
        maxM = c.max_m
        worstGivei = c.worst_givei
    }
}

/// The result fields of NavIC.
public struct NavicFields {
    /// 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes.
    public var state: Int64
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double?
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The PRN being searched.
    public var searchingPrn: Int64
    /// What is not confirmed by a public document.
    public var note: String

    init(_ c: sdrkit_navic_fields) {
        state = c.state
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        searchingPrn = c.searching_prn
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NavicFields, SDRKitCode> {
        var c = sdrkit_navic_fields()
        let code = sdrkit_navic_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NavicFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct NavicDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_navic_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NavicDecoderFields, SDRKitCode> {
        var c = sdrkit_navic_decoder_fields()
        let code = sdrkit_navic_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NavicDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A NavIC satellite.
public struct NavicSatelliteRow {
    /// The row key.
    public var key: String
    /// The PRN.
    public var prn: Int64
    /// 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes.
    public var state: Int64
    /// C/N0.
    public var cn0Dbhz: Double
    /// Doppler.
    public var dopplerHz: Double
    /// Subframes good.
    public var subframesOk: Int64
    /// Subframes bad.
    public var subframesBad: Int64
    /// The last subframe ID.
    public var subframeId: Int64?
    /// The time of week count.
    public var towc: Int64?
    /// The week number.
    public var wn: Int64?
    /// The alert flag.
    public var alert: Bool?

    init(_ c: sdrkit_navic_satellite_row) {
        key = sdrkitText(c.key)
        prn = c.prn
        state = c.state
        cn0Dbhz = c.cn0_dbhz
        dopplerHz = c.doppler_hz
        subframesOk = c.subframes_ok
        subframesBad = c.subframes_bad
        subframeId = c.has_subframe_id ? c.subframe_id : nil
        towc = c.has_towc ? c.towc : nil
        wn = c.has_wn ? c.wn : nil
        alert = c.has_alert ? c.alert : nil
    }
}

/// A fix.
public struct GnssFixEvent {
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
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition
    /// The height of the object that the event is about, metres above mean sea level.
    public var heightM: Double
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// Satellites in the fix.
    public var satellitesUsed: Int64

    init(_ c: sdrkit_gnss_fix_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        position = SDRKitPosition(c.position)
        heightM = c.height_m
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        satellitesUsed = c.satellites_used
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GnssFixEvent, SDRKitCode> {
        var c = sdrkit_gnss_fix_event()
        let code = sdrkit_gnss_fix_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GnssFixEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An NMEA 0183 sentence.
public struct GnssTextNmeaEvent {
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
    /// The family of the event.
    public var family: Family?
    /// The sentence.
    public var text: String

    init(_ c: sdrkit_gnss_text_nmea_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GnssTextNmeaEvent, SDRKitCode> {
        var c = sdrkit_gnss_text_nmea_event()
        let code = sdrkit_gnss_text_nmea_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GnssTextNmeaEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A NavIC subframe.
public struct NavicSubframeEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The PRN.
    public var prn: Int64
    /// The last subframe ID.
    public var subframeId: Int64
    /// The time of week count.
    public var towc: Int64
    /// The week number.
    public var wn: Int64?
    /// The message ID.
    public var messageId: Int64?
    /// The PRN ID.
    public var prnId: Int64?
    /// The alert flag.
    public var alert: Bool
    /// The autonav flag.
    public var autonav: Bool

    init(_ c: sdrkit_navic_subframe_event) {
        time = c.time
        freqHz = c.freq_hz
        prn = c.prn
        subframeId = c.subframe_id
        towc = c.towc
        wn = c.has_wn ? c.wn : nil
        messageId = c.has_message_id ? c.message_id : nil
        prnId = c.has_prn_id ? c.prn_id : nil
        alert = c.alert
        autonav = c.autonav
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<NavicSubframeEvent, SDRKitCode> {
        var c = sdrkit_navic_subframe_event()
        let code = sdrkit_navic_subframe_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(NavicSubframeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum GnssComponents {
    /// The result fields of the component "rx" of the stack "sbas" in the snapshot of `handle`.
    public static func sbasRxGnssFields(_ handle: SDRKIT_HANDLE) -> Result<GnssFields, SDRKitCode> {
        var c = sdrkit_gnss_fields()
        let code = sdrkit_sbas_rx_gnss_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GnssFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum GnssTables {
    /// The changed rows of the table "systems" of the stack "gnss", newest first, at most `capacity` rows.
    public static func gnssSystems(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[GnssSystemRow], SDRKitCode> {
        var rows = [sdrkit_gnss_system_row](repeating: sdrkit_gnss_system_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gnss_systems_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssSystemRow.init))
    }
    /// The changed rows of the table "fix_systems" of the stack "gnss", newest first, at most `capacity` rows.
    public static func gnssFixSystems(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[GnssFixSystemRow], SDRKitCode> {
        var rows = [sdrkit_gnss_fix_system_row](repeating: sdrkit_gnss_fix_system_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gnss_fix_systems_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssFixSystemRow.init))
    }
    /// The changed rows of the table "satellites" of the stack "gnss", newest first, at most `capacity` rows.
    public static func gnssSatellites(_ handle: SDRKIT_HANDLE, capacity: Int = 160) -> Result<[GnssSatelliteRow], SDRKitCode> {
        var rows = [sdrkit_gnss_satellite_row](repeating: sdrkit_gnss_satellite_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gnss_satellites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssSatelliteRow.init))
    }
    /// The changed rows of the table "interference_lines" of the stack "gnss", newest first, at most `capacity` rows.
    public static func gnssInterferenceLines(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[GnssInterferenceLineRow], SDRKitCode> {
        var rows = [sdrkit_gnss_interference_line_row](repeating: sdrkit_gnss_interference_line_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gnss_interference_lines_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssInterferenceLineRow.init))
    }
    /// The changed rows of the table "interference_events" of the stack "gnss", newest first, at most `capacity` rows.
    public static func gnssInterferenceEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[GnssInterferenceEventRow], SDRKitCode> {
        var rows = [sdrkit_gnss_interference_event_row](repeating: sdrkit_gnss_interference_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gnss_interference_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssInterferenceEventRow.init))
    }
    /// The changed rows of the table "sbas_satellites" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasSatellites(_ handle: SDRKIT_HANDLE, capacity: Int = 39) -> Result<[SbasSatelliteRow], SDRKitCode> {
        var rows = [sdrkit_sbas_satellite_row](repeating: sdrkit_sbas_satellite_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_satellites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SbasSatelliteRow.init))
    }
    /// The changed rows of the table "sbas_satellite_types" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasSatelliteTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 2496) -> Result<[SbasSatelliteTypeRow], SDRKitCode> {
        var rows = [sdrkit_sbas_satellite_type_row](repeating: sdrkit_sbas_satellite_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_satellite_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SbasSatelliteTypeRow.init))
    }
    /// The changed rows of the table "types" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SbasTypeRow], SDRKitCode> {
        var rows = [sdrkit_sbas_type_row](repeating: sdrkit_sbas_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SbasTypeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[SbasMessageRow], SDRKitCode> {
        var rows = [sdrkit_sbas_message_row](repeating: sdrkit_sbas_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SbasMessageRow.init))
    }
    /// The changed rows of the table "fast_corrections" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasFastCorrections(_ handle: SDRKIT_HANDLE, capacity: Int = 51) -> Result<[SbasFastCorrectionRow], SDRKitCode> {
        var rows = [sdrkit_sbas_fast_correction_row](repeating: sdrkit_sbas_fast_correction_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_fast_corrections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SbasFastCorrectionRow.init))
    }
    /// The changed rows of the table "ionosphere" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasIonosphere(_ handle: SDRKIT_HANDLE, capacity: Int = 11) -> Result<[SbasIonosphereRow], SDRKitCode> {
        var rows = [sdrkit_sbas_ionosphere_row](repeating: sdrkit_sbas_ionosphere_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_ionosphere_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SbasIonosphereRow.init))
    }
    /// The changed rows of the table "interference_lines" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasInterferenceLines(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[GnssInterferenceLineRow], SDRKitCode> {
        var rows = [sdrkit_gnss_interference_line_row](repeating: sdrkit_gnss_interference_line_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_interference_lines_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssInterferenceLineRow.init))
    }
    /// The changed rows of the table "interference_events" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasInterferenceEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[GnssInterferenceEventRow], SDRKitCode> {
        var rows = [sdrkit_gnss_interference_event_row](repeating: sdrkit_gnss_interference_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_interference_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssInterferenceEventRow.init))
    }
    /// The changed rows of the table "rx.systems" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasRxSystems(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[GnssSystemRow], SDRKitCode> {
        var rows = [sdrkit_gnss_system_row](repeating: sdrkit_gnss_system_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_rx_systems_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssSystemRow.init))
    }
    /// The changed rows of the table "rx.fix_systems" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasRxFixSystems(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[GnssFixSystemRow], SDRKitCode> {
        var rows = [sdrkit_gnss_fix_system_row](repeating: sdrkit_gnss_fix_system_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_rx_fix_systems_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssFixSystemRow.init))
    }
    /// The changed rows of the table "rx.satellites" of the stack "sbas", newest first, at most `capacity` rows.
    public static func sbasRxSatellites(_ handle: SDRKIT_HANDLE, capacity: Int = 160) -> Result<[GnssSatelliteRow], SDRKitCode> {
        var rows = [sdrkit_gnss_satellite_row](repeating: sdrkit_gnss_satellite_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sbas_rx_satellites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GnssSatelliteRow.init))
    }
    /// The changed rows of the table "satellites" of the stack "navic", newest first, at most `capacity` rows.
    public static func navicSatellites(_ handle: SDRKIT_HANDLE, capacity: Int = 14) -> Result<[NavicSatelliteRow], SDRKitCode> {
        var rows = [sdrkit_navic_satellite_row](repeating: sdrkit_navic_satellite_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_navic_satellites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NavicSatelliteRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum GnssCommands {
    /// Reset. Drops every channel and search. The navigation data stays.
    public static func gnssReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gnss_reset(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops every channel and search. The navigation data stays.
    public static func sbasReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sbas_reset(handle)
        return sdrkitResult(code)
    }
    /// Select. Shows the corrections of this SBAS satellite.
    public static func sbasSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_sbas_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_sbas_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_sbas_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets every message decoded so far.
    public static func sbasClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sbas_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func navicClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_navic_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum GnssSettings {
    /// The options of the setting "tune" of the stack "gnss".
    public enum GnssTune: Int32 {
        case l1 = 0
        case b1i = 1
        case glo = 2
        case all = 3
        case custom = 4
    }
    /// Tune.
    public static func gnssTune(_ handle: SDRKIT_HANDLE) -> Result<GnssTune, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tune", &v)
        return code == SDRKIT_OK ? .success(GnssTune(rawValue: v) ?? GnssTune(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssTune(_ handle: SDRKIT_HANDLE, _ value: GnssTune) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tune", value.rawValue))
    }
    /// GPS.
    public static func gnssSysGps(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sys_gps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSysGps(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sys_gps", value))
    }
    /// QZSS.
    public static func gnssSysQzss(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sys_qzss", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSysQzss(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sys_qzss", value))
    }
    /// SBAS.
    public static func gnssSysSbas(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sys_sbas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSysSbas(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sys_sbas", value))
    }
    /// Galileo.
    public static func gnssSysGalileo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sys_galileo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSysGalileo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sys_galileo", value))
    }
    /// BeiDou.
    public static func gnssSysBeidou(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sys_beidou", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSysBeidou(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sys_beidou", value))
    }
    /// GLONASS.
    public static func gnssSysGlonass(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sys_glonass", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSysGlonass(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sys_glonass", value))
    }
    /// Elevation mask.
    public static func gnssElevationMask(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "elevation_mask", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssElevationMask(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "elevation_mask", value))
    }
    /// Search half width.
    public static func gnssSearchHalfWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_half_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSearchHalfWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_half_width", value))
    }
    /// Radio frequency error.
    public static func gnssFrequencyHint(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "frequency_hint", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssFrequencyHint(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "frequency_hint", value))
    }
    /// Use the remembered error.
    public static func gnssFrequencyHintValid(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "frequency_hint_valid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssFrequencyHintValid(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "frequency_hint_valid", value))
    }
    /// Acquisition effort.
    public static func gnssAcquisitionEffort(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "acquisition_effort", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssAcquisitionEffort(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "acquisition_effort", value))
    }
    /// Known week.
    public static func gnssKnownWeek(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "known_week", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssKnownWeek(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "known_week", value))
    }
    /// Compare the computer clock.
    public static func gnssClockCompare(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "clock_compare", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssClockCompare(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "clock_compare", value))
    }
    /// Galileo E1-B code table.
    public static func gnssGalileoCodes(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "galileo_codes", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssGalileoCodes(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "galileo_codes", $0) })
    }
    /// Search half width.
    public static func sbasSearchHalfWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_half_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbasSearchHalfWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_half_width", value))
    }
    /// Radio frequency error.
    public static func sbasFrequencyHint(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "frequency_hint", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbasFrequencyHint(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "frequency_hint", value))
    }
    /// Use the remembered error.
    public static func sbasFrequencyHintValid(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "frequency_hint_valid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbasFrequencyHintValid(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "frequency_hint_valid", value))
    }
    /// Acquisition effort.
    public static func sbasAcquisitionEffort(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "acquisition_effort", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbasAcquisitionEffort(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "acquisition_effort", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum GnssGeneratorSettings {
    /// GPS.
    public static func gnssGps(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_gps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssGps(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_gps", value))
    }
    /// QZSS.
    public static func gnssQzss(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_qzss", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssQzss(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_qzss", value))
    }
    /// SBAS.
    public static func gnssSbas(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_sbas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSbas(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_sbas", value))
    }
    /// Galileo.
    public static func gnssGalileo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_galileo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssGalileo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_galileo", value))
    }
    /// BeiDou.
    public static func gnssBeidou(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_beidou", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssBeidou(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_beidou", value))
    }
    /// GLONASS.
    public static func gnssGlonass(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_glonass", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssGlonass(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_glonass", value))
    }
    /// Satellites (0: all).
    public static func gnssSats(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_sats", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssSats(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_sats", value))
    }
    /// Cold start.
    public static func gnssCold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_cold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGnssCold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_cold", value))
    }
    /// Satellites.
    public static func navicSats(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_sats", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNavicSats(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_sats", value))
    }
    /// Satellite Doppler.
    public static func navicSatDopplerHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_sat_doppler_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNavicSatDopplerHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_sat_doppler_hz", value))
    }
}

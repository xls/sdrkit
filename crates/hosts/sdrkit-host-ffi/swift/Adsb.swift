// sdrkit: the typed interface of the stack family "adsb", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum AdsbInterface {
    public static let major = 1
    public static let minor = 1
}

/// The result fields of the tracker.
public struct AdsbTrackerFields {
    /// Aircraft in the table.
    public var aircraftCount: Int64
    /// Aircraft with a position.
    public var withPosition: Int64
    /// Good messages per second over the last second.
    public var messageRate: Double
    /// The mean level of the messages.
    public var levelDbfs: Double?
    /// The noise floor.
    public var noiseDbfs: Double
    /// The mean signal to noise ratio of the messages.
    public var snrDb: Double?
    /// The farthest position counted.
    public var maxRangeM: Double?
    /// Reports made.
    public var report: Int64
    /// The signal time of the report, seconds from the start of the input.
    public var timeS: Double
    /// Messages that passed their check.
    public var framesOk: Int64
    /// Messages that failed their check.
    public var framesBad: Int64
    /// Messages repaired.
    public var repaired: Int64
    /// Messages at least 3 dB below full scale.
    public var loud: Int64
    /// Preamble candidates.
    public var candidates: Int64
    /// The demodulator effort in use.
    public var effort: Int64
    /// A station position that fits the aircraft heard, when the one given is far from all of them.
    public var positionHint: SDRKitPosition?
    /// Positions counted in the range statistics.
    public var coveragePositions: Int64
    /// The highest altitude counted.
    public var coverageMaxAltFt: Int64?

    init(_ c: sdrkit_adsb_tracker_fields) {
        aircraftCount = c.aircraft_count
        withPosition = c.with_position
        messageRate = c.message_rate
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        noiseDbfs = c.noise_dbfs
        snrDb = c.has_snr_db ? c.snr_db : nil
        maxRangeM = c.has_max_range_m ? c.max_range_m : nil
        report = c.report
        timeS = c.time_s
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        repaired = c.repaired
        loud = c.loud
        candidates = c.candidates
        effort = c.effort
        positionHint = c.has_position_hint ? SDRKitPosition(c.position_hint) : nil
        coveragePositions = c.coverage_positions
        coverageMaxAltFt = c.has_coverage_max_alt_ft ? c.coverage_max_alt_ft : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AdsbTrackerFields, SDRKitCode> {
        var c = sdrkit_adsb_tracker_fields()
        let code = sdrkit_adsb_tracker_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AdsbTrackerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the feeds out.
public struct AdsbFeedFields {
    /// Messages sent to the feeds.
    public var feedMessages: Int64

    init(_ c: sdrkit_adsb_feed_fields) {
        feedMessages = c.feed_messages
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AdsbFeedFields, SDRKitCode> {
        var c = sdrkit_adsb_feed_fields()
        let code = sdrkit_adsb_feed_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AdsbFeedFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the Mode A/C decoder.
public struct ModeacFields {
    /// Replies decoded.
    public var modeacReplies: Int64
    /// Codes shown.
    public var modeacCodes: Int64

    init(_ c: sdrkit_modeac_fields) {
        modeacReplies = c.modeac_replies
        modeacCodes = c.modeac_codes
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ModeacFields, SDRKitCode> {
        var c = sdrkit_modeac_fields()
        let code = sdrkit_modeac_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ModeacFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ADS-B aircraft table: the tracker's state for one aircraft. The row key is the address.
public struct AdsbAircraftRow {
    public enum Fix: Int32 {
        case global = 0
        case local = 1
        case localFromReceiver = 2
        case cprPair = 3
        case cprEven = 4
        case cprOdd = 5
    }
    public enum SpeedKind: Int32 {
        case ground = 0
        case ias = 1
        case tas = 2
    }
    /// The row key.
    public var key: String
    /// The 24-bit address, six hex digits (`~` in front for a non-ICAO address).
    public var icao: String
    /// The flight identification.
    public var callsign: String?
    /// The registration derived from the address.
    public var registration: String?
    /// The State the address is allocated to.
    public var country: String?
    /// The emitter category, set letter and code ("A3").
    public var category: String?
    /// The identity code, four octal digits.
    public var squawk: String?
    /// The emergency state (0 none).
    public var emergency: Int64
    /// The emergency state in words.
    public var emergencyName: String?
    /// On the ground.
    public var onGround: Bool?
    /// The alert bit of the flight status.
    public var alert: Bool
    /// The special position identification (ident) bit.
    public var spi: Bool
    /// The last position.
    public var position: SDRKitPosition?
    /// How the position was found, or which CPR frames wait for a partner.
    public var fix: Fix?
    /// When the position was heard, wall time.
    public var positionTime: Double?
    /// The barometric altitude.
    public var altFt: Int64?
    /// The GNSS height.
    public var gnssAltFt: Int64?
    /// The speed.
    public var speedKt: Double?
    /// What the speed is.
    public var speedKind: SpeedKind?
    /// The track or heading.
    public var headingDeg: Double?
    /// Whether the heading is the track over the ground.
    public var headingIsTrack: Bool?
    /// The vertical rate.
    public var verticalRateFpm: Int64?
    /// GNSS height minus barometric altitude.
    public var gnssMinusBaroFt: Int64?
    /// The selected altitude.
    public var selectedAltFt: Int64?
    /// The selected heading.
    public var selectedHeadingDeg: Double?
    /// The barometric pressure setting.
    public var baroSettingHpa: Double?
    /// The ADS-B version number.
    public var adsbVersion: Int64?
    /// The navigation accuracy category for position.
    public var nacp: Int64?
    /// The source integrity level.
    public var sil: Int64?
    /// The navigation integrity category.
    public var nic: Int64?
    /// The indicated airspeed.
    public var iasKt: Double?
    /// The Mach number.
    public var mach: Double?
    /// The roll angle.
    public var rollDeg: Double?
    /// Where the derived wind blows from.
    public var windDerivedFromDeg: Double?
    /// The derived wind speed.
    public var windDerivedKt: Double?
    /// Where the reported wind blows from.
    public var windReportedFromDeg: Double?
    /// The reported wind speed.
    public var windReportedKt: Double?
    /// The static air temperature from TAS and Mach.
    public var satDerivedC: Double?
    /// The static air temperature reported.
    public var satReportedC: Double?
    /// The static pressure reported.
    public var pressureHpa: Double?
    /// The humidity reported.
    public var humidityPct: Double?
    /// The hazards reported, with their levels.
    public var hazards: String?
    /// The ACAS resolution advisory in words.
    public var advisory: String?
    /// The address of the threat of the advisory.
    public var advisoryThreat: String?
    /// When the advisory was heard, wall time.
    public var advisoryTime: Double?
    /// The Mode S subnetwork version.
    public var subnetworkVersion: Int64?
    /// ACAS generates resolution advisories.
    public var acasRaCapable: Bool?
    /// The GICB registers the transponder holds.
    public var gicb: String?
    /// Comm-B messages were heard.
    public var datalink: Bool
    /// Heard through TIS-B.
    public var tisb: Bool
    /// The mean pulse level.
    public var levelDbfs: Double?
    /// The distance from the station.
    public var distanceM: Double?
    /// The bearing from the station.
    public var bearingDeg: Double?
    /// Where the last update came from: the row key of the sources table ("0" this receiver, else the number of the feed).
    public var source: String
    /// The trail, packed.
    public var trail: [UInt8]
    /// Times heard.
    public var heard: Int64?
    /// Wall-clock time when last heard, in whole seconds.
    public var lastHeard: Double?

    init(_ c: sdrkit_adsb_aircraft_row) {
        key = sdrkitText(c.key)
        icao = sdrkitText(c.icao)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        category = c.has_category ? sdrkitText(c.category) : nil
        squawk = c.has_squawk ? sdrkitText(c.squawk) : nil
        emergency = c.emergency
        emergencyName = c.has_emergency_name ? sdrkitText(c.emergency_name) : nil
        onGround = c.has_on_ground ? c.on_ground : nil
        alert = c.alert
        spi = c.spi
        position = c.has_position ? SDRKitPosition(c.position) : nil
        fix = c.has_fix ? Fix(rawValue: c.fix) : nil
        positionTime = c.has_position_time ? c.position_time : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        gnssAltFt = c.has_gnss_alt_ft ? c.gnss_alt_ft : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        speedKind = c.has_speed_kind ? SpeedKind(rawValue: c.speed_kind) : nil
        headingDeg = c.has_heading_deg ? c.heading_deg : nil
        headingIsTrack = c.has_heading_is_track ? c.heading_is_track : nil
        verticalRateFpm = c.has_vertical_rate_fpm ? c.vertical_rate_fpm : nil
        gnssMinusBaroFt = c.has_gnss_minus_baro_ft ? c.gnss_minus_baro_ft : nil
        selectedAltFt = c.has_selected_alt_ft ? c.selected_alt_ft : nil
        selectedHeadingDeg = c.has_selected_heading_deg ? c.selected_heading_deg : nil
        baroSettingHpa = c.has_baro_setting_hpa ? c.baro_setting_hpa : nil
        adsbVersion = c.has_adsb_version ? c.adsb_version : nil
        nacp = c.has_nacp ? c.nacp : nil
        sil = c.has_sil ? c.sil : nil
        nic = c.has_nic ? c.nic : nil
        iasKt = c.has_ias_kt ? c.ias_kt : nil
        mach = c.has_mach ? c.mach : nil
        rollDeg = c.has_roll_deg ? c.roll_deg : nil
        windDerivedFromDeg = c.has_wind_derived_from_deg ? c.wind_derived_from_deg : nil
        windDerivedKt = c.has_wind_derived_kt ? c.wind_derived_kt : nil
        windReportedFromDeg = c.has_wind_reported_from_deg ? c.wind_reported_from_deg : nil
        windReportedKt = c.has_wind_reported_kt ? c.wind_reported_kt : nil
        satDerivedC = c.has_sat_derived_c ? c.sat_derived_c : nil
        satReportedC = c.has_sat_reported_c ? c.sat_reported_c : nil
        pressureHpa = c.has_pressure_hpa ? c.pressure_hpa : nil
        humidityPct = c.has_humidity_pct ? c.humidity_pct : nil
        hazards = c.has_hazards ? sdrkitText(c.hazards) : nil
        advisory = c.has_advisory ? sdrkitText(c.advisory) : nil
        advisoryThreat = c.has_advisory_threat ? sdrkitText(c.advisory_threat) : nil
        advisoryTime = c.has_advisory_time ? c.advisory_time : nil
        subnetworkVersion = c.has_subnetwork_version ? c.subnetwork_version : nil
        acasRaCapable = c.has_acas_ra_capable ? c.acas_ra_capable : nil
        gicb = c.has_gicb ? sdrkitText(c.gicb) : nil
        datalink = c.datalink
        tisb = c.tisb
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        distanceM = c.has_distance_m ? c.distance_m : nil
        bearingDeg = c.has_bearing_deg ? c.bearing_deg : nil
        source = sdrkitText(c.source)
        trail = [UInt8](UnsafeBufferPointer(start: c.trail.ptr, count: c.trail.len))
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the message monitor: one good message of this receiver.
public struct AdsbMessageRow {
    /// The row key.
    public var key: String
    /// Signal time of the preamble.
    public var timeS: Double
    /// The frame in hex.
    public var hex: String
    /// The address.
    public var icao: String
    /// The downlink format.
    public var df: Int64
    /// Bits the repair flipped.
    public var fixedBits: Int64
    /// Level.
    public var levelDbfs: Double
    /// The description in words.
    public var message: String

    init(_ c: sdrkit_adsb_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        hex = sdrkitText(c.hex)
        icao = sdrkitText(c.icao)
        df = c.df
        fixedBits = c.fixed_bits
        levelDbfs = c.level_dbfs
        message = sdrkitText(c.message)
    }
}

/// A receiver that the table takes messages from. The row key is the number of the source: "0" for this receiver, then the number of each feed in.
public struct AdsbSourceRow {
    public enum Format: Int32 {
        case beast = 0
        case sbs = 1
        case asterix = 2
    }
    /// The row key.
    public var key: String
    /// The name.
    public var name: String
    /// The format, known after the first bytes.
    public var format: Format?
    /// Connected now.
    public var connected: Bool
    /// Messages taken.
    public var messages: Int64
    /// Aircraft whose last update came from it.
    public var aircraft: Int64
    /// The last error.
    public var error: String?

    init(_ c: sdrkit_adsb_source_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        format = c.has_format ? Format(rawValue: c.format) : nil
        connected = c.connected
        messages = c.messages
        aircraft = c.aircraft
        error = c.has_error ? sdrkitText(c.error) : nil
    }
}

/// A count of messages of one format. The row key is the format number in two digits.
public struct AdsbCountRow {
    /// The row key.
    public var key: String
    /// A downlink or uplink format number.
    public var formatNumber: Int64
    /// How many.
    public var count: Int64

    init(_ c: sdrkit_adsb_count_row) {
        key = sdrkitText(c.key)
        formatNumber = c.format_number
        count = c.count
    }
}

/// A GPS interference cell of half a degree with at least three aircraft.
public struct AdsbGpsCellRow {
    /// The row key.
    public var key: String
    /// The south-west corner of the cell.
    public var corner: SDRKitPosition
    /// Aircraft in the cell.
    public var aircraft: Int64
    /// The level, 0 to 3.
    public var level: Int64

    init(_ c: sdrkit_adsb_gps_cell_row) {
        key = sdrkitText(c.key)
        corner = SDRKitPosition(c.corner)
        aircraft = c.aircraft
        level = c.level
    }
}

/// A feed out: a server or an outbound target. The row key is the name.
public struct AdsbFeedRow {
    /// The row key.
    public var key: String
    /// The server ("beast", "sbs", "asterix") or the outbound target ("format@host:port").
    public var name: String
    /// The TCP port of a server or a target.
    public var port: Int64?
    /// Clients connected to a server.
    public var clients: Int64?
    /// Messages dropped because a client or target was slow.
    public var dropped: Int64?
    /// A target is connected.
    public var connected: Bool?
    /// Why a server did not start.
    public var error: String?

    init(_ c: sdrkit_adsb_feed_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        port = c.has_port ? c.port : nil
        clients = c.has_clients ? c.clients : nil
        dropped = c.has_dropped ? c.dropped : nil
        connected = c.has_connected ? c.connected : nil
        error = c.has_error ? sdrkitText(c.error) : nil
    }
}

/// A Mode A/C code heard at least the set number of times. The row key is the code.
public struct ModeacRow {
    /// The row key.
    public var key: String
    /// The 13 code bits in hex.
    public var code: String
    /// The code read as a squawk.
    public var squawk: String
    /// The code read as a Mode C altitude.
    public var altFt: Int64?
    /// Replies heard.
    public var replies: Int64?
    /// An SPI pulse was heard.
    public var spi: Bool
    /// Level of the last reply.
    public var levelDbfs: Double
    /// Wall-clock time when last heard, in whole seconds.
    public var lastHeard: Double?

    init(_ c: sdrkit_modeac_row) {
        key = sdrkitText(c.key)
        code = sdrkitText(c.code)
        squawk = sdrkitText(c.squawk)
        altFt = c.has_alt_ft ? c.alt_ft : nil
        replies = c.has_replies ? c.replies : nil
        spi = c.spi
        levelDbfs = c.level_dbfs
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the UAT tracker.
public struct UatFields {
    /// Aircraft in the table.
    public var aircraftCount: Int64
    /// Aircraft with a position.
    public var withPosition: Int64
    /// Good messages per second over the last second.
    public var messageRate: Double
    /// FIS-B products held.
    public var fisbProducts: Int64
    /// TIS-B messages among the last 64.
    public var tisbRecent: Int64
    /// FIS-B files waiting for segments.
    public var fisbPending: Int64
    /// Frames that passed their check.
    public var framesOk: Int64
    /// The signal to noise ratio.
    public var snrDb: Double?
    /// The noise floor.
    public var noiseDbfs: Double
    /// Reports made.
    public var report: Int64
    /// The signal time of the report, seconds from the start of the input.
    public var timeS: Double

    init(_ c: sdrkit_uat_fields) {
        aircraftCount = c.aircraft_count
        withPosition = c.with_position
        messageRate = c.message_rate
        fisbProducts = c.fisb_products
        tisbRecent = c.tisb_recent
        fisbPending = c.fisb_pending
        framesOk = c.frames_ok
        snrDb = c.has_snr_db ? c.snr_db : nil
        noiseDbfs = c.noise_dbfs
        report = c.report
        timeS = c.time_s
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<UatFields, SDRKitCode> {
        var c = sdrkit_uat_fields()
        let code = sdrkit_uat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(UatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the UAT aircraft table. The row key is the address.
public struct UatAircraftRow {
    /// The row key.
    public var key: String
    /// The 24-bit address, six hex digits.
    public var icao: String
    /// The address qualifier.
    public var addressQualifier: Int64
    /// The flight identification.
    public var callsign: String?
    /// The registration derived from the address.
    public var registration: String?
    /// The State the address is allocated to.
    public var country: String?
    /// The identity code, four octal digits.
    public var squawk: String?
    /// The last position.
    public var position: SDRKitPosition?
    /// The barometric altitude.
    public var altFt: Int64?
    /// The geometric altitude.
    public var geoAltFt: Int64?
    /// The speed.
    public var speedKt: Double?
    /// The track or heading.
    public var headingDeg: Double?
    /// The vertical rate.
    public var verticalRateFpm: Int64?
    /// On the ground.
    public var onGround: Bool
    /// Heard through TIS-B.
    public var tisb: Bool
    /// The navigation integrity category.
    public var nic: Int64
    /// The UAT emitter category.
    public var category: Int64?
    /// The emergency state (0 none).
    public var emergency: Int64
    /// The level of the last message.
    public var levelDbfs: Double
    /// The distance from the station.
    public var distanceM: Double?
    /// The bearing from the station.
    public var bearingDeg: Double?
    /// Times heard.
    public var heard: Int64?
    /// Wall-clock time when last heard, in whole seconds.
    public var lastHeard: Double?

    init(_ c: sdrkit_uat_aircraft_row) {
        key = sdrkitText(c.key)
        icao = sdrkitText(c.icao)
        addressQualifier = c.address_qualifier
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        squawk = c.has_squawk ? sdrkitText(c.squawk) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altFt = c.has_alt_ft ? c.alt_ft : nil
        geoAltFt = c.has_geo_alt_ft ? c.geo_alt_ft : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        headingDeg = c.has_heading_deg ? c.heading_deg : nil
        verticalRateFpm = c.has_vertical_rate_fpm ? c.vertical_rate_fpm : nil
        onGround = c.on_ground
        tisb = c.tisb
        nic = c.nic
        category = c.has_category ? c.category : nil
        emergency = c.emergency
        levelDbfs = c.level_dbfs
        distanceM = c.has_distance_m ? c.distance_m : nil
        bearingDeg = c.has_bearing_deg ? c.bearing_deg : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A FIS-B product held. The row key is the number of the product in six digits.
public struct UatFisbProductRow {
    /// The row key.
    public var key: String
    /// Signal time the product was received.
    public var timeS: Double
    /// The product id.
    public var productId: Int64
    /// The product name.
    public var name: String
    /// The product's own time ("MM-DD HH:MM:SS", the parts sent).
    public var stamp: String
    /// The text.
    public var text: String
    /// TWGO records in the product.
    public var records: Int64
    /// Grid blocks in the product.
    public var blocks: Int64

    init(_ c: sdrkit_uat_fisb_product_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        productId = c.product_id
        name = sdrkitText(c.name)
        stamp = sdrkitText(c.stamp)
        text = sdrkitText(c.text)
        records = c.records
        blocks = c.blocks
    }
}

/// A text or graphic record (TWGO) of a product held: a NOTAM, AIRMET, SIGMET, G-AIRMET, CWA, SUA or TFR.
public struct UatFisbRecordRow {
    public enum Kind: Int32 {
        case text = 0
        case graphic = 1
    }
    public enum Geometry: Int32 {
        case polygon = 0
        case polyline = 1
        case circle = 2
        case point = 3
        case other = 4
    }
    /// The row key.
    public var key: String
    /// The product the record belongs to: the row key of the products table.
    public var product: String
    /// Signal time the product was received.
    public var timeS: Double
    /// The product id.
    public var productId: Int64
    /// The product name.
    public var name: String
    /// The report number of the record.
    public var report: Int64
    /// What the record is.
    public var kind: Kind?
    /// The record is active (text records).
    public var active: Bool?
    /// The label of a graphic record.
    public var label: String?
    /// The overlay record identifier.
    public var overlay: Int64?
    /// The start of the validity ("MM-DD HH:MM", the parts sent).
    public var start: String?
    /// The end of the validity.
    public var end: String?
    /// The geometry of a graphic record.
    public var geometry: Geometry?
    /// The first vertex, the centre or the point.
    public var position: SDRKitPosition?
    /// The number of vertices of a path. The vertices are rows of the vertices table.
    public var vertices: Int64?
    /// The radius of a circle.
    public var radiusM: Double?
    /// The lowest altitude.
    public var altBottomFt: Int64?
    /// The highest altitude.
    public var altTopFt: Int64?
    /// The text of a text record.
    public var text: String?

    init(_ c: sdrkit_uat_fisb_record_row) {
        key = sdrkitText(c.key)
        product = sdrkitText(c.product)
        timeS = c.time_s
        productId = c.product_id
        name = sdrkitText(c.name)
        report = c.report
        kind = Kind(rawValue: c.kind)
        active = c.has_active ? c.active : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        overlay = c.has_overlay ? c.overlay : nil
        start = c.has_start ? sdrkitText(c.start) : nil
        end = c.has_end ? sdrkitText(c.end) : nil
        geometry = c.has_geometry ? Geometry(rawValue: c.geometry) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        vertices = c.has_vertices ? c.vertices : nil
        radiusM = c.has_radius_m ? c.radius_m : nil
        altBottomFt = c.has_alt_bottom_ft ? c.alt_bottom_ft : nil
        altTopFt = c.has_alt_top_ft ? c.alt_top_ft : nil
        text = c.has_text ? sdrkitText(c.text) : nil
    }
}

/// A block of a gridded product held (NEXRAD, icing, cloud tops, turbulence, lightning).
public struct UatFisbBlockRow {
    /// The row key.
    public var key: String
    /// The product the block belongs to: the row key of the products table.
    public var product: String
    /// Signal time the product was received.
    public var timeS: Double
    /// The product id.
    public var productId: Int64
    /// The product name.
    public var name: String
    /// The block number.
    public var block: Int64
    /// The block is in the southern hemisphere.
    public var south: Bool
    /// The scale factor (0 high, 1 medium, 2 low resolution).
    public var scale: Int64
    /// The north-west corner of the block.
    public var corner: SDRKitPosition
    /// The height of the block.
    public var latSizeDeg: Double
    /// The width of the block.
    public var lonSizeDeg: Double
    /// The highest bin value.
    public var maxBin: Int64
    /// The 128 bins, one hex digit each, rows from the north. An empty block has none.
    public var bins: String?

    init(_ c: sdrkit_uat_fisb_block_row) {
        key = sdrkitText(c.key)
        product = sdrkitText(c.product)
        timeS = c.time_s
        productId = c.product_id
        name = sdrkitText(c.name)
        block = c.block
        south = c.south
        scale = c.scale
        corner = SDRKitPosition(c.corner)
        latSizeDeg = c.lat_size_deg
        lonSizeDeg = c.lon_size_deg
        maxBin = c.max_bin
        bins = c.has_bins ? sdrkitText(c.bins) : nil
    }
}

/// A vertex of the path of a graphic record held.
public struct UatFisbVertexRow {
    /// The row key.
    public var key: String
    /// The record the vertex belongs to: the row key of the records table.
    public var record: String
    /// The place of the vertex in its path, from 0.
    public var index: Int64
    /// The position.
    public var position: SDRKitPosition
    /// The altitude.
    public var altFt: Int64

    init(_ c: sdrkit_uat_fisb_vertex_row) {
        key = sdrkitText(c.key)
        record = sdrkitText(c.record)
        index = c.index
        position = SDRKitPosition(c.position)
        altFt = c.alt_ft
    }
}

/// A line of the UAT message log.
public struct UatLogRow {
    public enum Kind: Int32 {
        case adsb = 0
        case tisb = 1
        case fisb = 2
        case uat = 3
    }
    /// The row key.
    public var key: String
    /// Signal time.
    public var timeS: Double
    /// What the message is.
    public var kind: Kind?
    /// The address.
    public var address: String?
    /// Level.
    public var levelDbfs: Double
    /// What it said.
    public var summary: String

    init(_ c: sdrkit_uat_log_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        address = c.has_address ? sdrkitText(c.address) : nil
        levelDbfs = c.level_dbfs
        summary = sdrkitText(c.summary)
    }
}

/// The result fields of the interrogation decoder.
public struct Uplink1030UplinkFields {
    /// Interrogations of all kinds.
    public var interrogations: Int64
    /// Mode A interrogations.
    public var modeA: Int64
    /// Mode C interrogations.
    public var modeC: Int64
    /// Mode S interrogations.
    public var modeS: Int64
    /// Mode A/C/S all-calls.
    public var allCallA: Int64
    /// Mode C/S all-calls.
    public var allCallC: Int64
    /// Mode A/C-only all-calls.
    public var acOnlyAllCall: Int64

    init(_ c: sdrkit_uplink1030_uplink_fields) {
        interrogations = c.interrogations
        modeA = c.mode_a
        modeC = c.mode_c
        modeS = c.mode_s
        allCallA = c.all_call_a
        allCallC = c.all_call_c
        acOnlyAllCall = c.ac_only_all_call
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Uplink1030UplinkFields, SDRKitCode> {
        var c = sdrkit_uplink1030_uplink_fields()
        let code = sdrkit_uplink1030_uplink_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Uplink1030UplinkFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An aircraft that the interrogations address. The row key is the address.
public struct Uplink1030AddressedRow {
    /// The row key.
    public var key: String
    /// The interrogated address.
    public var icao: String
    /// Interrogations.
    public var interrogations: Int64?
    /// Uplink formats with their counts ("UF4x3 UF5x1").
    public var formats: String
    /// The fields of the last interrogation.
    public var last: String
    /// Wall-clock time when last heard, in whole seconds.
    public var lastHeard: Double?

    init(_ c: sdrkit_uplink1030_addressed_row) {
        key = sdrkitText(c.key)
        icao = sdrkitText(c.icao)
        interrogations = c.has_interrogations ? c.interrogations : nil
        formats = sdrkitText(c.formats)
        last = sdrkitText(c.last)
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// An interrogator heard in all-calls. The row key is its code.
public struct Uplink1030InterrogatorRow {
    /// The row key.
    public var key: String
    /// The interrogator code of an all-call.
    public var code: String
    /// All-calls.
    public var allCalls: Int64?
    /// Wall-clock time when last heard, in whole seconds.
    public var lastHeard: Double?

    init(_ c: sdrkit_uplink1030_interrogator_row) {
        key = sdrkitText(c.key)
        code = sdrkitText(c.code)
        allCalls = c.has_all_calls ? c.all_calls : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The fields of the event of an aircraft seen or updated. The address is the source. The urgency is an emergency for squawk 7500, 7600 or 7700 and for an emergency state.
public struct AdsbAircraftEvent {
    public enum Event: Int32 {
        case new = 0
        case update = 1
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
    /// The first event of an aircraft, or a later one.
    public var event: Event?
    /// The address.
    public var source: String
    /// The flight identification.
    public var callsign: String?
    /// The registration derived from the address.
    public var registration: String?
    /// The emitter category, set letter and code ("A3").
    public var category: String?
    /// The identity code, four octal digits.
    public var squawk: String?
    /// The emergency state (0 none).
    public var emergency: Int64
    /// On the ground.
    public var onGround: Bool?
    /// The barometric altitude.
    public var altitudeFt: Int64?
    /// The speed over the ground.
    public var speedKt: Double?
    /// The course over the ground.
    public var courseDeg: Double?
    /// The vertical rate.
    public var verticalRateFpm: Int64?
    /// The position.
    public var position: SDRKitPosition?
    /// The distance from the station.
    public var distanceKm: Double?
    /// Messages heard.
    public var messages: Int64
    /// How urgent the event is.
    public var urgency: Urgency?
    /// Why the event is urgent.
    public var text: String?
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_adsb_aircraft_event) {
        time = c.time
        freqHz = c.freq_hz
        event = Event(rawValue: c.event)
        source = sdrkitText(c.source)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        registration = c.has_registration ? sdrkitText(c.registration) : nil
        category = c.has_category ? sdrkitText(c.category) : nil
        squawk = c.has_squawk ? sdrkitText(c.squawk) : nil
        emergency = c.emergency
        onGround = c.has_on_ground ? c.on_ground : nil
        altitudeFt = c.has_altitude_ft ? c.altitude_ft : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        verticalRateFpm = c.has_vertical_rate_fpm ? c.vertical_rate_fpm : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        distanceKm = c.has_distance_km ? c.distance_km : nil
        messages = c.messages
        urgency = Urgency(rawValue: c.urgency)
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AdsbAircraftEvent, SDRKitCode> {
        var c = sdrkit_adsb_aircraft_event()
        let code = sdrkit_adsb_aircraft_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AdsbAircraftEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a decoded UAT message. The address is the source. The urgency is an emergency for squawk 7500, 7600 or 7700 and for an emergency status.
public struct UatMessageEvent {
    public enum Kind: Int32 {
        case adsb = 0
        case tisb = 1
        case fisb = 2
        case uat = 3
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
    /// What the message said.
    public var text: String
    /// What the message is.
    public var kind: Kind?
    /// The address.
    public var source: String?
    /// The level.
    public var levelDbfs: Double
    /// The position decoded from the message.
    public var position: SDRKitPosition?
    /// How urgent the event is.
    public var urgency: Urgency?
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_uat_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        kind = Kind(rawValue: c.kind)
        source = c.has_source ? sdrkitText(c.source) : nil
        levelDbfs = c.level_dbfs
        position = c.has_position ? SDRKitPosition(c.position) : nil
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<UatMessageEvent, SDRKitCode> {
        var c = sdrkit_uat_message_event()
        let code = sdrkit_uat_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(UatMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a Mode A/C reply.
public struct ModeacReplyEvent {
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
    /// The 13 code bits in hex.
    public var code: String
    /// The code read as a squawk.
    public var squawk: String
    /// The code read as a Mode C altitude.
    public var altitudeFt: Int64?
    /// An SPI pulse was heard.
    public var spi: Bool
    /// The level.
    public var levelDbfs: Double
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_modeac_reply_event) {
        time = c.time
        freqHz = c.freq_hz
        code = sdrkitText(c.code)
        squawk = sdrkitText(c.squawk)
        altitudeFt = c.has_altitude_ft ? c.altitude_ft : nil
        spi = c.spi
        levelDbfs = c.level_dbfs
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ModeacReplyEvent, SDRKitCode> {
        var c = sdrkit_modeac_reply_event()
        let code = sdrkit_modeac_reply_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ModeacReplyEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of an interrogation. The interrogated address is the target once it is confirmed.
public struct Uplink1030UplinkInterrogationEvent {
    public enum Kind: Int32 {
        case modeA = 0
        case modeC = 1
        case allCallA = 2
        case allCallC = 3
        case acOnlyAllCall = 4
        case modeS = 5
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
    /// What the interrogation is.
    public var kind: Kind?
    /// The level.
    public var levelDbfs: Double
    /// The P2 (side-lobe suppression) pulse was seen.
    public var p2: Bool
    /// The uplink format of a Mode S interrogation.
    public var uf: Int64?
    /// The Mode S message in hex.
    public var hex: String?
    /// The interrogated address, once it is confirmed (see `confirmed`).
    public var target: String?
    /// Mode S to one aircraft: the address from the parity is in the table of addressed aircraft, so it is the target. False when it is not yet: a damaged message also gives an address, so the target has no value and the address is only in the message. No value for Mode A/C, all-calls and broadcasts.
    public var confirmed: Bool?
    /// The fields of a Mode S interrogation.
    public var text: String?
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_uplink1030_uplink_interrogation_event) {
        time = c.time
        freqHz = c.freq_hz
        kind = Kind(rawValue: c.kind)
        levelDbfs = c.level_dbfs
        p2 = c.p2
        uf = c.has_uf ? c.uf : nil
        hex = c.has_hex ? sdrkitText(c.hex) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        confirmed = c.has_confirmed ? c.confirmed : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Uplink1030UplinkInterrogationEvent, SDRKitCode> {
        var c = sdrkit_uplink1030_uplink_interrogation_event()
        let code = sdrkit_uplink1030_uplink_interrogation_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Uplink1030UplinkInterrogationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum AdsbTables {
    /// The changed rows of the table "aircraft" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 160) -> Result<[AdsbAircraftRow], SDRKitCode> {
        var rows = [sdrkit_adsb_aircraft_row](repeating: sdrkit_adsb_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbAircraftRow.init))
    }
    /// The changed rows of the table "messages" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[AdsbMessageRow], SDRKitCode> {
        var rows = [sdrkit_adsb_message_row](repeating: sdrkit_adsb_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbMessageRow.init))
    }
    /// The changed rows of the table "sources" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbSources(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[AdsbSourceRow], SDRKitCode> {
        var rows = [sdrkit_adsb_source_row](repeating: sdrkit_adsb_source_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_sources_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbSourceRow.init))
    }
    /// The changed rows of the table "by_df" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbByDf(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[AdsbCountRow], SDRKitCode> {
        var rows = [sdrkit_adsb_count_row](repeating: sdrkit_adsb_count_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_by_df_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbCountRow.init))
    }
    /// The changed rows of the table "gps_cells" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbGpsCells(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[AdsbGpsCellRow], SDRKitCode> {
        var rows = [sdrkit_adsb_gps_cell_row](repeating: sdrkit_adsb_gps_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_gps_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbGpsCellRow.init))
    }
    /// The changed rows of the table "feeds" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbFeeds(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[AdsbFeedRow], SDRKitCode> {
        var rows = [sdrkit_adsb_feed_row](repeating: sdrkit_adsb_feed_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_feeds_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbFeedRow.init))
    }
    /// The changed rows of the table "modeac" of the stack "adsb", newest first, at most `capacity` rows.
    public static func adsbModeac(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[ModeacRow], SDRKitCode> {
        var rows = [sdrkit_modeac_row](repeating: sdrkit_modeac_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_adsb_modeac_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ModeacRow.init))
    }
    /// The changed rows of the table "aircraft" of the stack "uat", newest first, at most `capacity` rows.
    public static func uatAircraft(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[UatAircraftRow], SDRKitCode> {
        var rows = [sdrkit_uat_aircraft_row](repeating: sdrkit_uat_aircraft_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uat_aircraft_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UatAircraftRow.init))
    }
    /// The changed rows of the table "fisb_products" of the stack "uat", newest first, at most `capacity` rows.
    public static func uatFisbProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[UatFisbProductRow], SDRKitCode> {
        var rows = [sdrkit_uat_fisb_product_row](repeating: sdrkit_uat_fisb_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uat_fisb_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UatFisbProductRow.init))
    }
    /// The changed rows of the table "fisb_records" of the stack "uat", newest first, at most `capacity` rows.
    public static func uatFisbRecords(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[UatFisbRecordRow], SDRKitCode> {
        var rows = [sdrkit_uat_fisb_record_row](repeating: sdrkit_uat_fisb_record_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uat_fisb_records_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UatFisbRecordRow.init))
    }
    /// The changed rows of the table "fisb_blocks" of the stack "uat", newest first, at most `capacity` rows.
    public static func uatFisbBlocks(_ handle: SDRKIT_HANDLE, capacity: Int = 2048) -> Result<[UatFisbBlockRow], SDRKitCode> {
        var rows = [sdrkit_uat_fisb_block_row](repeating: sdrkit_uat_fisb_block_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uat_fisb_blocks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UatFisbBlockRow.init))
    }
    /// The changed rows of the table "fisb_vertices" of the stack "uat", newest first, at most `capacity` rows.
    public static func uatFisbVertices(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[UatFisbVertexRow], SDRKitCode> {
        var rows = [sdrkit_uat_fisb_vertex_row](repeating: sdrkit_uat_fisb_vertex_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uat_fisb_vertices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UatFisbVertexRow.init))
    }
    /// The changed rows of the table "log" of the stack "uat", newest first, at most `capacity` rows.
    public static func uatLog(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[UatLogRow], SDRKitCode> {
        var rows = [sdrkit_uat_log_row](repeating: sdrkit_uat_log_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uat_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UatLogRow.init))
    }
    /// The changed rows of the table "modeac" of the stack "modeac", newest first, at most `capacity` rows.
    public static func modeac(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[ModeacRow], SDRKitCode> {
        var rows = [sdrkit_modeac_row](repeating: sdrkit_modeac_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_modeac_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ModeacRow.init))
    }
    /// The changed rows of the table "addressed" of the stack "uplink1030", newest first, at most `capacity` rows.
    public static func uplink1030Addressed(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Uplink1030AddressedRow], SDRKitCode> {
        var rows = [sdrkit_uplink1030_addressed_row](repeating: sdrkit_uplink1030_addressed_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uplink1030_addressed_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Uplink1030AddressedRow.init))
    }
    /// The changed rows of the table "interrogators" of the stack "uplink1030", newest first, at most `capacity` rows.
    public static func uplink1030Interrogators(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Uplink1030InterrogatorRow], SDRKitCode> {
        var rows = [sdrkit_uplink1030_interrogator_row](repeating: sdrkit_uplink1030_interrogator_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uplink1030_interrogators_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Uplink1030InterrogatorRow.init))
    }
    /// The changed rows of the table "by_uf" of the stack "uplink1030", newest first, at most `capacity` rows.
    public static func uplink1030ByUf(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[AdsbCountRow], SDRKitCode> {
        var rows = [sdrkit_adsb_count_row](repeating: sdrkit_adsb_count_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uplink1030_by_uf_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AdsbCountRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum AdsbCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func adsbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_adsb_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset statistics. Starts the range statistics again.
    public static func adsbResetStats(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_adsb_reset_stats(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func uatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_uat_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func modeacClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_modeac_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func uplink1030Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_uplink1030_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum AdsbSettings {
    /// The options of the setting "correction" of the stack "adsb".
    public enum AdsbCorrection: Int32 {
        case off = 0
        case n1bit = 1
        case n3bits = 2
    }
    /// Error correction.
    public static func adsbCorrection(_ handle: SDRKIT_HANDLE) -> Result<AdsbCorrection, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "correction", &v)
        return code == SDRKIT_OK ? .success(AdsbCorrection(rawValue: v) ?? AdsbCorrection(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbCorrection(_ handle: SDRKIT_HANDLE, _ value: AdsbCorrection) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "correction", value.rawValue))
    }
    /// Repair unknown addresses.
    public static func adsbRepairUnknown(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "repair_unknown", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbRepairUnknown(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "repair_unknown", value))
    }
    /// Expiry time.
    public static func adsbExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// The options of the setting "effort" of the stack "adsb".
    public enum AdsbEffort: Int32 {
        case auto = 0
        case allPasses = 1
        case strongCandidatesOnly = 2
        case plainDetector = 3
    }
    /// Effort.
    public static func adsbEffort(_ handle: SDRKIT_HANDLE) -> Result<AdsbEffort, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "effort", &v)
        return code == SDRKIT_OK ? .success(AdsbEffort(rawValue: v) ?? AdsbEffort(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbEffort(_ handle: SDRKIT_HANDLE, _ value: AdsbEffort) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "effort", value.rawValue))
    }
    /// Mode A/C replies.
    public static func adsbModeac(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "modeac", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbModeac(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "modeac", value))
    }
    /// Feeds from other receivers.
    public static func adsbFeedsIn(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "feeds_in", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbFeedsIn(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "feeds_in", $0) })
    }
    /// Allow hosts outside the local network.
    public static func adsbAllowRemote(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_remote", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbAllowRemote(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_remote", value))
    }
    /// Beast server.
    public static func adsbBeastServer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "beast_server", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbBeastServer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "beast_server", value))
    }
    /// Beast port.
    public static func adsbBeastPort(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "beast_port", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbBeastPort(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "beast_port", value))
    }
    /// SBS server.
    public static func adsbSbsServer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sbs_server", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbSbsServer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sbs_server", value))
    }
    /// SBS port.
    public static func adsbSbsPort(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sbs_port", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbSbsPort(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sbs_port", value))
    }
    /// ASTERIX server.
    public static func adsbAsterixServer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "asterix_server", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbAsterixServer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "asterix_server", value))
    }
    /// ASTERIX port.
    public static func adsbAsterixPort(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "asterix_port", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbAsterixPort(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "asterix_port", value))
    }
    /// The options of the setting "asterix_cat" of the stack "adsb".
    public enum AdsbAsterixCat: Int32 {
        case all = 0
        case n021Only = 1
        case n048Only = 2
        case n062Only = 3
    }
    /// ASTERIX categories.
    public static func adsbAsterixCat(_ handle: SDRKIT_HANDLE) -> Result<AdsbAsterixCat, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "asterix_cat", &v)
        return code == SDRKIT_OK ? .success(AdsbAsterixCat(rawValue: v) ?? AdsbAsterixCat(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbAsterixCat(_ handle: SDRKIT_HANDLE, _ value: AdsbAsterixCat) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "asterix_cat", value.rawValue))
    }
    /// ASTERIX SAC.
    public static func adsbSac(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sac", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbSac(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sac", value))
    }
    /// ASTERIX SIC.
    public static func adsbSic(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sic", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbSic(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sic", value))
    }
    /// Servers accept the local network.
    public static func adsbAllowLan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_lan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbAllowLan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_lan", value))
    }
    /// Outbound targets.
    public static func adsbTargets(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "targets", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbTargets(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "targets", $0) })
    }
    /// Drop time.
    public static func adsbDropS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "drop_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbDropS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "drop_s", value))
    }
    /// GPS interference layer.
    public static func adsbGpsLayer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gps_layer", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbGpsLayer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gps_layer", value))
    }
    /// Expiry time.
    public static func uatExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUatExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// Expiry time.
    public static func modeacExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setModeacExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// Replies before a code is shown.
    public static func modeacMinReplies(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "min_replies", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setModeacMinReplies(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "min_replies", value))
    }
    /// Expiry time.
    public static func uplink1030ExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUplink1030ExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum AdsbGeneratorSettings {
    /// Aircraft.
    public static func adsbAircraft(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_aircraft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbAircraft(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value))
    }
    /// Radar replies.
    public static func adsbReplies(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_replies", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbReplies(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_replies", value))
    }
    /// Emergency demonstration.
    public static func adsbEmergency(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_emergency", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbEmergency(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_emergency", value))
    }
    /// Message rate multiplier.
    public static func adsbRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rate", value))
    }
    /// Pulse filter width.
    public static func adsbFilterMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_filter_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbFilterMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_filter_mhz", value))
    }
    /// Airspace centre latitude.
    public static func adsbCentreLat(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_lat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbCentreLat(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_lat", value))
    }
    /// Airspace centre longitude.
    public static func adsbCentreLon(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_lon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAdsbCentreLon(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_lon", value))
    }
    /// Aircraft.
    public static func uatAircraft(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_aircraft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUatAircraft(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value))
    }
    /// Uplink frames.
    public static func uatUplink(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_uplink", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUatUplink(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_uplink", value))
    }
    /// Example frame only.
    public static func uatExample(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_example", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUatExample(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_example", value))
    }
    /// Example bursts.
    public static func uatBursts(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_bursts", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUatBursts(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_bursts", value))
    }
    /// Transponders.
    public static func modeacTargets(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_targets", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setModeacTargets(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_targets", value))
    }
}

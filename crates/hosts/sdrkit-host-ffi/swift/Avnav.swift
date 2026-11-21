// sdrkit: the typed interface of the stack family "avnav", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum AvnavInterface {
    public static let major = 1
    public static let minor = 1
}

/// The tuned frequency.
public struct AvnavTunedFields {
    /// The tuned frequency.
    public var tunedHz: Double

    init(_ c: sdrkit_avnav_tuned_fields) {
        tunedHz = c.tuned_hz
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AvnavTunedFields, SDRKitCode> {
        var c = sdrkit_avnav_tuned_fields()
        let code = sdrkit_avnav_tuned_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavTunedFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The identification of the station.
public struct AvnavIdentFields {
    /// The confirmed identification. No value before one is confirmed.
    public var ident: String?
    /// The last identification heard. No value before a word is heard.
    public var lastIdent: String?
    /// The Morse speed of the last identification. No value before a word is heard.
    public var identWpm: Double?

    init(_ c: sdrkit_avnav_ident_fields) {
        ident = c.has_ident ? sdrkitText(c.ident) : nil
        lastIdent = c.has_last_ident ? sdrkitText(c.last_ident) : nil
        identWpm = c.has_ident_wpm ? c.ident_wpm : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AvnavIdentFields, SDRKitCode> {
        var c = sdrkit_avnav_ident_fields()
        let code = sdrkit_avnav_ident_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavIdentFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The carrier of the station.
public struct AvnavCarrierFields {
    /// A carrier stands clear of the noise.
    public var carrier: Bool
    /// The frequency of the carrier. No value before the first measurement.
    public var carrierHz: Double?
    /// The carrier to noise density. No value before the first measurement.
    public var cn0Dbhz: Double?
    /// The offset of the carrier from the tuned frequency. No value before the first measurement.
    public var offsetHz: Double?
    /// The level of the carrier. No value before the first measurement.
    public var levelDbfs: Double?

    init(_ c: sdrkit_avnav_carrier_fields) {
        carrier = c.carrier
        carrierHz = c.has_carrier_hz ? c.carrier_hz : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AvnavCarrierFields, SDRKitCode> {
        var c = sdrkit_avnav_carrier_fields()
        let code = sdrkit_avnav_carrier_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavCarrierFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The identification that the average of the repeats reads. No value while the average reads nothing.
public struct NdbFields {
    /// The period of the identification.
    public var identPeriodS: Double?
    /// The robust z-score of the autocorrelation at that period.
    public var identPeriodZ: Double?
    /// The swing of the average of the repeats over its noise.
    public var identFoldQuality: Double?
    /// The word read from the average.
    public var identFoldText: String?

    init(_ c: sdrkit_ndb_fields) {
        identPeriodS = c.has_ident_period_s ? c.ident_period_s : nil
        identPeriodZ = c.has_ident_period_z ? c.ident_period_z : nil
        identFoldQuality = c.has_ident_fold_quality ? c.ident_fold_quality : nil
        identFoldText = c.has_ident_fold_text ? sdrkitText(c.ident_fold_text) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NdbFields, SDRKitCode> {
        var c = sdrkit_ndb_fields()
        let code = sdrkit_ndb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NdbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The identification tone that the tone keyer listens to.
public struct AvnavKeyerFields {
    /// The tone: the setting, or the tone found. No value before the first input.
    public var idToneHz: Double?
    /// The modulation depth of the tone. No value before the first input.
    public var idDepthPct: Double?

    init(_ c: sdrkit_avnav_keyer_fields) {
        idToneHz = c.has_id_tone_hz ? c.id_tone_hz : nil
        idDepthPct = c.has_id_depth_pct ? c.id_depth_pct : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AvnavKeyerFields, SDRKitCode> {
        var c = sdrkit_avnav_keyer_fields()
        let code = sdrkit_avnav_keyer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavKeyerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the identifications heard. The row key is the word.
public struct AvnavIdentRow {
    /// The row key.
    public var key: String
    /// The word.
    public var ident: String
    /// How often it was heard.
    public var heard: Int64

    init(_ c: sdrkit_avnav_ident_row) {
        key = sdrkitText(c.key)
        ident = sdrkitText(c.ident)
        heard = c.heard
    }
}

/// A row of the band scan. The row key is the frequency in whole hertz.
public struct NdbScanRow {
    /// The row key.
    public var key: String
    /// The frequency of the carrier.
    public var freqHz: Double
    /// The level of the carrier.
    public var levelDbfs: Double
    /// The carrier to noise density.
    public var cn0Dbhz: Double

    init(_ c: sdrkit_ndb_scan_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
        levelDbfs = c.level_dbfs
        cn0Dbhz = c.cn0_dbhz
    }
}

/// The result fields of VOR.
public struct VorFields {
    public enum ToFrom: Int32 {
        case to = 0
        case from = 1
        case off = 2
    }
    /// The radial. No value without a valid bearing.
    public var radialDeg: Double?
    /// The bearing to the station: the radial plus 180°. No value without a valid bearing.
    public var bearingToDeg: Double?
    /// The selected course.
    public var obsDeg: Double
    /// TO, FROM, or OFF (the flag: no valid bearing).
    public var toFrom: ToFrom?
    /// The course deviation, positive to fly right. No value without a valid bearing.
    public var cdiDeg: Double?
    /// The course deviation in dots. No value without a valid bearing.
    public var cdiDots: Double?
    /// The depth of the 30 Hz AM. No value before the first measurement.
    public var am30Pct: Double?
    /// The depth of the subcarrier. No value before the first measurement.
    public var subcarrierPct: Double?
    /// The peak deviation of the subcarrier. No value before the first measurement.
    public var deviationHz: Double?
    /// How steady the bearing is, 0 (random) to 1. No value before the first measurement.
    public var steadiness: Double?
    /// Measurements made.
    public var fixes: Int64
    /// The tuned frequency.
    public var tunedHz: Double
    /// The DME channel paired with the frequency ("17X"). No value for a frequency without one.
    public var dmeChannel: String?

    init(_ c: sdrkit_vor_fields) {
        radialDeg = c.has_radial_deg ? c.radial_deg : nil
        bearingToDeg = c.has_bearing_to_deg ? c.bearing_to_deg : nil
        obsDeg = c.obs_deg
        toFrom = ToFrom(rawValue: c.to_from)
        cdiDeg = c.has_cdi_deg ? c.cdi_deg : nil
        cdiDots = c.has_cdi_dots ? c.cdi_dots : nil
        am30Pct = c.has_am30_pct ? c.am30_pct : nil
        subcarrierPct = c.has_subcarrier_pct ? c.subcarrier_pct : nil
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        steadiness = c.has_steadiness ? c.steadiness : nil
        fixes = c.fixes
        tunedHz = c.tuned_hz
        dmeChannel = c.has_dme_channel ? sdrkitText(c.dme_channel) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<VorFields, SDRKitCode> {
        var c = sdrkit_vor_fields()
        let code = sdrkit_vor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of ILS.
public struct IlsFields {
    public enum Part: Int32 {
        case localizer = 0
        case glideslope = 1
        case marker = 2
    }
    public enum Command: Int32 {
        case onCourse = 0
        case onPath = 1
        case flyLeft = 2
        case flyRight = 3
        case flyUp = 4
        case flyDown = 5
        case flag = 6
    }
    /// Localizer or glide path.
    public var part: Part?
    /// The difference in depth of modulation (90 Hz less 150 Hz). No value while the flag is out.
    public var ddm: Double?
    /// The sum in depth of modulation. No value before the first measurement.
    public var sdmPct: Double?
    /// The depth of the 90 Hz tone. No value before the first measurement.
    public var m90Pct: Double?
    /// The depth of the 150 Hz tone. No value before the first measurement.
    public var m150Pct: Double?
    /// The needle, positive to fly right or up, limited to 150 µA either way. No value while the flag is out.
    public var needleUa: Double?
    /// The needle in dots (75 µA a dot), limited to 2.5 either way. No value while the flag is out.
    public var needleDots: Double?
    /// What the needle says.
    public var command: Command?
    /// The flag is out: no carrier, an SDM far from its nominal value, or the marker part, which has no needle.
    public var flag: Bool
    /// The paired localizer or glide path frequency. No value for a frequency without one.
    public var pairedHz: Double?
    /// The tuned frequency.
    public var tunedHz: Double
    /// The DME channel paired with the localizer ("17X"). No value for a frequency without one.
    public var dmeChannel: String?

    init(_ c: sdrkit_ils_fields) {
        part = Part(rawValue: c.part)
        ddm = c.has_ddm ? c.ddm : nil
        sdmPct = c.has_sdm_pct ? c.sdm_pct : nil
        m90Pct = c.has_m90_pct ? c.m90_pct : nil
        m150Pct = c.has_m150_pct ? c.m150_pct : nil
        needleUa = c.has_needle_ua ? c.needle_ua : nil
        needleDots = c.has_needle_dots ? c.needle_dots : nil
        command = Command(rawValue: c.command)
        flag = c.flag
        pairedHz = c.has_paired_hz ? c.paired_hz : nil
        tunedHz = c.tuned_hz
        dmeChannel = c.has_dme_channel ? sdrkitText(c.dme_channel) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IlsFields, SDRKitCode> {
        var c = sdrkit_ils_fields()
        let code = sdrkit_ils_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IlsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The marker beacon (the part `marker`): the passage under way, or the last one, and the live indication.
public struct IlsMarkerFields {
    public enum Marker: Int32 {
        case none = 0
        case outer = 1
        case middle = 2
        case inner = 3
    }
    public enum Pattern: Int32 {
        case notYet = 0
        case dashes = 1
        case dotDash = 2
        case dots = 3
        case irregular = 4
    }
    /// The marker of the passage, named by its tone. None before the first passage of three elements and outside the marker part.
    public var marker: Marker?
    /// The tone of the passage, measured. No value without a passage.
    public var markerToneHz: Double?
    /// The modulation depth of the tone. No value without a passage.
    public var markerDepthPct: Double?
    /// The keying as text ("2.0 dashes per second (0.30 s long)"). No value without a passage.
    public var keying: String?
    /// Elements a second, from the first onset to the last. No value without a passage.
    public var keyingRate: Double?
    /// Dot-dash pairs a minute. No value without mixed keying.
    public var pairsPerMin: Double?
    /// The mean dot length. No value without dots.
    public var dotS: Double?
    /// The mean dash length. No value without dashes.
    public var dashS: Double?
    /// The keying fits the standard (ICAO Annex 10 Volume I, 3.1.7.5). No value without a passage.
    public var keyingFits: Bool?
    /// The length of the passage. No value without a passage.
    public var passageS: Double?
    /// Passages finished, those of fewer than three elements not counted.
    public var passages: Int64
    /// The tone with the greatest held depth keys the last period.
    public var keyedNow: Bool
    /// The held depth of that tone. No value outside the marker part.
    public var heldDepthPct: Double?
    /// The pattern of the last on-lengths of that tone.
    public var pattern: Pattern?

    init(_ c: sdrkit_ils_marker_fields) {
        marker = Marker(rawValue: c.marker)
        markerToneHz = c.has_marker_tone_hz ? c.marker_tone_hz : nil
        markerDepthPct = c.has_marker_depth_pct ? c.marker_depth_pct : nil
        keying = c.has_keying ? sdrkitText(c.keying) : nil
        keyingRate = c.has_keying_rate ? c.keying_rate : nil
        pairsPerMin = c.has_pairs_per_min ? c.pairs_per_min : nil
        dotS = c.has_dot_s ? c.dot_s : nil
        dashS = c.has_dash_s ? c.dash_s : nil
        keyingFits = c.has_keying_fits ? c.keying_fits : nil
        passageS = c.has_passage_s ? c.passage_s : nil
        passages = c.passages
        keyedNow = c.keyed_now
        heldDepthPct = c.has_held_depth_pct ? c.held_depth_pct : nil
        pattern = Pattern(rawValue: c.pattern)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IlsMarkerFields, SDRKitCode> {
        var c = sdrkit_ils_marker_fields()
        let code = sdrkit_ils_marker_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IlsMarkerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the marker passages. The row key is the number of the passage.
public struct IlsMarkerPassageRow {
    public enum Kind: Int32 {
        case marker = 0
        case toneOnly = 1
    }
    public enum Marker: Int32 {
        case none = 0
        case outer = 1
        case middle = 2
        case inner = 3
    }
    /// The row key.
    public var key: String
    /// A marker, or a marker tone whose keying does not fit the standard.
    public var kind: Kind?
    /// The marker of the tone.
    public var marker: Marker?
    /// The tone, the keying, the depth and the length ("400 Hz, 2.0 dashes per second (0.30 s long), depth 95 %, 7.8 s").
    public var text: String
    /// The carrier level at the end of the passage. No value before the first carrier measurement.
    public var levelDbfs: Double?

    init(_ c: sdrkit_ils_marker_passage_row) {
        key = sdrkitText(c.key)
        kind = Kind(rawValue: c.kind)
        marker = Marker(rawValue: c.marker)
        text = sdrkitText(c.text)
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }
}

/// The result fields of DME, over the last second of pairs.
public struct DmeFields {
    public enum Heard: Int32 {
        case beacon = 0
        case interrogations = 1
        case none = 2
    }
    public enum ChannelMode: Int32 {
        case x = 0
        case y = 1
    }
    /// What is heard.
    public var heard: Heard?
    /// X or Y, from the pulse spacing. No value with too few pairs.
    public var channelMode: ChannelMode?
    /// The channel from the tuned frequency and the mode ("17X"). No value with too few pairs.
    public var channel: String?
    /// The paired VOR or localizer frequency. No value without a channel that has one.
    public var pairedVhfHz: Double?
    /// Pairs a second.
    public var pairsPerS: Double
    /// The mean pair spacing. No value with too few pairs.
    public var spacingUs: Double?
    /// The mean pulse width at half amplitude. No value with too few pairs.
    public var widthUs: Double?
    /// Pairs seen in all.
    public var pairs: Int64
    /// The mean level of the pulses. No value with too few pairs.
    public var levelDbfs: Double?

    init(_ c: sdrkit_dme_fields) {
        heard = Heard(rawValue: c.heard)
        channelMode = c.has_channel_mode ? ChannelMode(rawValue: c.channel_mode) : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        pairedVhfHz = c.has_paired_vhf_hz ? c.paired_vhf_hz : nil
        pairsPerS = c.pairs_per_s
        spacingUs = c.has_spacing_us ? c.spacing_us : nil
        widthUs = c.has_width_us ? c.width_us : nil
        pairs = c.pairs
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DmeFields, SDRKitCode> {
        var c = sdrkit_dme_fields()
        let code = sdrkit_dme_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DmeFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of Loran. A field of the chain has no value before the first measurement.
public struct LoranFields {
    /// The GRI in use, in tens of microseconds.
    public var gri: Int64?
    /// The chain of that GRI. No value for a GRI that the `chains` setting does not name.
    public var chain: String?
    /// The master is found: its correlation SNR is at least 10 dB.
    public var master: Bool
    /// The correlation SNR of the master.
    public var masterSnrDb: Double?
    /// Measurements made.
    public var fixes: Int64

    init(_ c: sdrkit_loran_fields) {
        gri = c.has_gri ? c.gri : nil
        chain = c.has_chain ? sdrkitText(c.chain) : nil
        master = c.master
        masterSnrDb = c.has_master_snr_db ? c.master_snr_db : nil
        fixes = c.fixes
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LoranFields, SDRKitCode> {
        var c = sdrkit_loran_fields()
        let code = sdrkit_loran_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoranFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the stations of the chain. The row key is the letter of the station.
public struct LoranStationRow {
    /// The row key.
    public var key: String
    /// `M` for the master, then the letters of the secondaries.
    public var station: String
    /// The time difference from the master.
    public var tdUs: Double
    /// The envelope-to-cycle difference relative to the master.
    public var ecdUs: Double
    /// The signal to noise ratio of the correlation.
    public var snrDb: Double
    /// The level relative to the master.
    public var levelDb: Double

    init(_ c: sdrkit_loran_station_row) {
        key = sdrkitText(c.key)
        station = sdrkitText(c.station)
        tdUs = c.td_us
        ecdUs = c.ecd_us
        snrDb = c.snr_db
        levelDb = c.level_db
    }
}

/// The result fields of DGPS. A field of the station has no value before the first frame.
public struct DgpsFields {
    public enum Health: Int32 {
        case scale1 = 0
        case scale075 = 1
        case scale05 = 2
        case scale03 = 3
        case scale02 = 4
        case scale01 = 5
        case notMonitored = 6
        case notWorking = 7
    }
    /// The base station id.
    public var stationId: Int64?
    /// The health of the station.
    public var health: Health?
    /// The last modified Z-count: the time into the hour.
    public var zcount: Double?
    /// The bit rate decoded. No value before the first report of the link.
    public var symbolRateHz: Double?
    /// The framer of that bit rate holds the frame sync.
    public var locked: Bool
    /// Words with good parity.
    public var goodWords: Int64
    /// Words with bad parity.
    public var badWords: Int64
    /// Frames decoded.
    public var framesOk: Int64
    /// The position of the base station. No value before a message of type 3.
    public var baseStation: SDRKitPosition?
    /// The height of the base station above the ellipsoid. No value before a message of type 3.
    public var baseHeightM: Double?
    /// The last text message. No value before a message of type 16.
    public var text: String?
    /// The satellites flagged unhealthy, by PRN. No value before a message of type 5.
    public var unhealthy: String?
    /// The corrections held.
    public var corrections: Int64

    init(_ c: sdrkit_dgps_fields) {
        stationId = c.has_station_id ? c.station_id : nil
        health = c.has_health ? Health(rawValue: c.health) : nil
        zcount = c.has_zcount ? c.zcount : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        locked = c.locked
        goodWords = c.good_words
        badWords = c.bad_words
        framesOk = c.frames_ok
        baseStation = c.has_base_station ? SDRKitPosition(c.base_station) : nil
        baseHeightM = c.has_base_height_m ? c.base_height_m : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        unhealthy = c.has_unhealthy ? sdrkitText(c.unhealthy) : nil
        corrections = c.corrections
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DgpsFields, SDRKitCode> {
        var c = sdrkit_dgps_fields()
        let code = sdrkit_dgps_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DgpsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The levels of the channel, from its power meter.
public struct DgpsMeterFields {
    /// The mean level. No value before the first tenth of a second.
    public var levelDbfs: Double?
    /// The peak level. No value before the first tenth of a second.
    public var peakDbfs: Double?

    init(_ c: sdrkit_dgps_meter_fields) {
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        peakDbfs = c.has_peak_dbfs ? c.peak_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DgpsMeterFields, SDRKitCode> {
        var c = sdrkit_dgps_meter_fields()
        let code = sdrkit_dgps_meter_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DgpsMeterFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The results of one MSK demodulator.
public struct DgpsMskFields {
    /// The error of the symbol clock.
    public var clockPpm: Double?
    /// The carrier offset that the demodulator removes.
    public var offsetHz: Double?

    init(_ c: sdrkit_dgps_msk_fields) {
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }
}

/// A row of the frames by message type. The row key is the type in two digits.
public struct DgpsMessageRow {
    /// The row key.
    public var key: String
    /// The message type.
    public var msgType: Int64
    /// The frames of the type.
    public var count: Int64

    init(_ c: sdrkit_dgps_message_row) {
        key = sdrkitText(c.key)
        msgType = c.msg_type
        count = c.count
    }
}

/// A row of the pseudorange corrections. The row key is the PRN in two digits.
public struct DgpsCorrectionRow {
    /// The row key.
    public var key: String
    /// The satellite.
    public var prn: Int64
    /// The pseudorange correction.
    public var prcM: Double
    /// The range-rate correction.
    public var rrcMS: Double
    /// The issue of data.
    public var iod: Int64
    /// The user differential range error.
    public var udre: Int64

    init(_ c: sdrkit_dgps_correction_row) {
        key = sdrkitText(c.key)
        prn = c.prn
        prcM = c.prc_m
        rrcMS = c.rrc_m_s
        iod = c.iod
        udre = c.udre
    }
}

/// A row of the beacon almanac. The row key is the station id.
public struct DgpsAlmanacRow {
    /// The row key.
    public var key: String
    /// The station id.
    public var stationId: Int64
    /// The frequency of the beacon.
    public var freqHz: Double
    /// The bit rate of the beacon.
    public var bitRateHz: Double
    /// The position of the beacon.
    public var position: SDRKitPosition
    /// The range of the beacon.
    public var rangeKm: Double
    /// The health of the beacon, 0 (healthy) to 3.
    public var health: Int64

    init(_ c: sdrkit_dgps_almanac_row) {
        key = sdrkitText(c.key)
        stationId = c.station_id
        freqHz = c.freq_hz
        bitRateHz = c.bit_rate_hz
        position = SDRKitPosition(c.position)
        rangeKm = c.range_km
        health = c.health
    }
}

/// The fields of a decoded identification word: a spot of the station that sends it. The source, the text and the callsign hold the word.
public struct AvnavMorseWordEvent {
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
    /// The word.
    public var source: String
    /// The word.
    public var text: String
    /// The family of the source: other.
    public var family: Family?
    /// The word.
    public var callsign: String
    /// No value: a beacon sends no locator.
    public var locator: String?
    /// No value: the decoder does not measure it.
    public var snrDb: Double?
    /// No value: the event carries the frequency.
    public var freqHzField: Double?
    /// The emission: "CW".
    public var mode: String
    /// The Morse speed.
    public var wpm: Double

    init(_ c: sdrkit_avnav_morse_word_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        callsign = sdrkitText(c.callsign)
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        freqHzField = c.has_freq_hz_ ? c.freq_hz_ : nil
        mode = sdrkitText(c.mode)
        wpm = c.wpm
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AvnavMorseWordEvent, SDRKitCode> {
        var c = sdrkit_avnav_morse_word_event()
        let code = sdrkit_avnav_morse_word_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AvnavMorseWordEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a finished marker passage.
public struct IlsMarkerEvent {
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
    public enum Kind: Int32 {
        case marker = 0
        case toneOnly = 1
    }
    public enum Marker: Int32 {
        case none = 0
        case outer = 1
        case middle = 2
        case inner = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The marker ("Outer marker").
    public var source: String
    /// The tone, the keying, the depth and the length.
    public var text: String
    /// The family of the source: other.
    public var family: Family?
    /// A marker, or a marker tone whose keying does not fit the standard.
    public var kind: Kind?
    /// The marker of the tone.
    public var marker: Marker?
    /// The tone, measured.
    public var toneHz: Double
    /// The modulation depth of the tone.
    public var depthPct: Double
    /// The length of the passage.
    public var durationS: Double

    init(_ c: sdrkit_ils_marker_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        kind = Kind(rawValue: c.kind)
        marker = Marker(rawValue: c.marker)
        toneHz = c.tone_hz
        depthPct = c.depth_pct
        durationS = c.duration_s
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IlsMarkerEvent, SDRKitCode> {
        var c = sdrkit_ils_marker_event()
        let code = sdrkit_ils_marker_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IlsMarkerEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an RTCM frame.
public struct DgpsRtcmFrameEvent {
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
    public enum Health: Int32 {
        case scale1 = 0
        case scale075 = 1
        case scale05 = 2
        case scale03 = 3
        case scale02 = 4
        case scale01 = 5
        case notMonitored = 6
        case notWorking = 7
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The station id.
    public var source: String
    /// A short text of the message.
    public var text: String
    /// The family of the source: other.
    public var family: Family?
    /// The message type.
    public var msgType: Int64
    /// The station id.
    public var stationId: Int64
    /// The modified Z-count, seconds into the hour.
    public var zcountS: Double
    /// The sequence number.
    public var sequence: Int64
    /// The health of the station.
    public var health: Health?

    init(_ c: sdrkit_dgps_rtcm_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        msgType = c.msg_type
        stationId = c.station_id
        zcountS = c.zcount_s
        sequence = c.sequence
        health = Health(rawValue: c.health)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DgpsRtcmFrameEvent, SDRKitCode> {
        var c = sdrkit_dgps_rtcm_frame_event()
        let code = sdrkit_dgps_rtcm_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DgpsRtcmFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum AvnavComponents {
    /// The result fields of the component "keyer" of the stack "ndb" in the snapshot of `handle`.
    public static func ndbKeyerFields(_ handle: SDRKIT_HANDLE) -> Result<AvnavKeyerFields, SDRKitCode> {
        var c = sdrkit_avnav_keyer_fields()
        let code = sdrkit_ndb_keyer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavKeyerFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "keyer" of the stack "vor" in the snapshot of `handle`.
    public static func vorKeyerFields(_ handle: SDRKIT_HANDLE) -> Result<AvnavKeyerFields, SDRKitCode> {
        var c = sdrkit_avnav_keyer_fields()
        let code = sdrkit_vor_keyer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavKeyerFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "keyer" of the stack "ils" in the snapshot of `handle`.
    public static func ilsKeyerFields(_ handle: SDRKIT_HANDLE) -> Result<AvnavKeyerFields, SDRKitCode> {
        var c = sdrkit_avnav_keyer_fields()
        let code = sdrkit_ils_keyer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AvnavKeyerFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "level" of the stack "dgps" in the snapshot of `handle`.
    public static func dgpsLevelDgpsMeterFields(_ handle: SDRKIT_HANDLE) -> Result<DgpsMeterFields, SDRKitCode> {
        var c = sdrkit_dgps_meter_fields()
        let code = sdrkit_dgps_level_dgps_meter_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DgpsMeterFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "msk50" of the stack "dgps" in the snapshot of `handle`.
    public static func dgpsMsk50DgpsMskFields(_ handle: SDRKIT_HANDLE) -> Result<DgpsMskFields, SDRKitCode> {
        var c = sdrkit_dgps_msk_fields()
        let code = sdrkit_dgps_msk50_dgps_msk_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DgpsMskFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "msk100" of the stack "dgps" in the snapshot of `handle`.
    public static func dgpsMsk100DgpsMskFields(_ handle: SDRKIT_HANDLE) -> Result<DgpsMskFields, SDRKitCode> {
        var c = sdrkit_dgps_msk_fields()
        let code = sdrkit_dgps_msk100_dgps_msk_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DgpsMskFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "msk200" of the stack "dgps" in the snapshot of `handle`.
    public static func dgpsMsk200DgpsMskFields(_ handle: SDRKIT_HANDLE) -> Result<DgpsMskFields, SDRKitCode> {
        var c = sdrkit_dgps_msk_fields()
        let code = sdrkit_dgps_msk200_dgps_msk_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DgpsMskFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum AvnavTables {
    /// The changed rows of the table "idents" of the stack "ndb", newest first, at most `capacity` rows.
    public static func ndbIdents(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[AvnavIdentRow], SDRKitCode> {
        var rows = [sdrkit_avnav_ident_row](repeating: sdrkit_avnav_ident_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ndb_idents_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AvnavIdentRow.init))
    }
    /// The changed rows of the table "scan" of the stack "ndb", newest first, at most `capacity` rows.
    public static func ndbScan(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[NdbScanRow], SDRKitCode> {
        var rows = [sdrkit_ndb_scan_row](repeating: sdrkit_ndb_scan_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ndb_scan_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NdbScanRow.init))
    }
    /// The changed rows of the table "idents" of the stack "vor", newest first, at most `capacity` rows.
    public static func vorIdents(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[AvnavIdentRow], SDRKitCode> {
        var rows = [sdrkit_avnav_ident_row](repeating: sdrkit_avnav_ident_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vor_idents_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AvnavIdentRow.init))
    }
    /// The changed rows of the table "idents" of the stack "ils", newest first, at most `capacity` rows.
    public static func ilsIdents(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[AvnavIdentRow], SDRKitCode> {
        var rows = [sdrkit_avnav_ident_row](repeating: sdrkit_avnav_ident_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ils_idents_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AvnavIdentRow.init))
    }
    /// The changed rows of the table "passages" of the stack "ils", newest first, at most `capacity` rows.
    public static func ilsPassages(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[IlsMarkerPassageRow], SDRKitCode> {
        var rows = [sdrkit_ils_marker_passage_row](repeating: sdrkit_ils_marker_passage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ils_passages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IlsMarkerPassageRow.init))
    }
    /// The changed rows of the table "idents" of the stack "dme", newest first, at most `capacity` rows.
    public static func dmeIdents(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[AvnavIdentRow], SDRKitCode> {
        var rows = [sdrkit_avnav_ident_row](repeating: sdrkit_avnav_ident_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dme_idents_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AvnavIdentRow.init))
    }
    /// The changed rows of the table "stations" of the stack "loran", newest first, at most `capacity` rows.
    public static func loranStations(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[LoranStationRow], SDRKitCode> {
        var rows = [sdrkit_loran_station_row](repeating: sdrkit_loran_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loran_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LoranStationRow.init))
    }
    /// The changed rows of the table "messages" of the stack "dgps", newest first, at most `capacity` rows.
    public static func dgpsMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DgpsMessageRow], SDRKitCode> {
        var rows = [sdrkit_dgps_message_row](repeating: sdrkit_dgps_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dgps_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DgpsMessageRow.init))
    }
    /// The changed rows of the table "corrections" of the stack "dgps", newest first, at most `capacity` rows.
    public static func dgpsCorrections(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[DgpsCorrectionRow], SDRKitCode> {
        var rows = [sdrkit_dgps_correction_row](repeating: sdrkit_dgps_correction_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dgps_corrections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DgpsCorrectionRow.init))
    }
    /// The changed rows of the table "almanac" of the stack "dgps", newest first, at most `capacity` rows.
    public static func dgpsAlmanac(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DgpsAlmanacRow], SDRKitCode> {
        var rows = [sdrkit_dgps_almanac_row](repeating: sdrkit_dgps_almanac_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dgps_almanac_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DgpsAlmanacRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum AvnavCommands {
    /// Clear. Forgets everything decoded so far.
    public static func ndbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ndb_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func vorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vor_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ilsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ils_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dmeClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dme_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func loranClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_loran_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dgpsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dgps_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum AvnavSettings {
    /// The options of the setting "ident_keying" of the stack "ndb".
    public enum NdbIdentKeying: Int32 {
        case auto = 0
        case a = 1
        case b = 2
    }
    /// Identification.
    public static func ndbIdentKeying(_ handle: SDRKIT_HANDLE) -> Result<NdbIdentKeying, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "ident_keying", &v)
        return code == SDRKIT_OK ? .success(NdbIdentKeying(rawValue: v) ?? NdbIdentKeying(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbIdentKeying(_ handle: SDRKIT_HANDLE, _ value: NdbIdentKeying) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "ident_keying", value.rawValue))
    }
    /// Identification tone.
    public static func ndbTone(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbTone(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tone", value))
    }
    /// The options of the setting "audio" of the stack "ndb".
    public enum NdbAudio: Int32 {
        case am = 0
        case cw = 1
    }
    /// Audio.
    public static func ndbAudio(_ handle: SDRKIT_HANDLE) -> Result<NdbAudio, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "audio", &v)
        return code == SDRKIT_OK ? .success(NdbAudio(rawValue: v) ?? NdbAudio(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbAudio(_ handle: SDRKIT_HANDLE, _ value: NdbAudio) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "audio", value.rawValue))
    }
    /// Beat pitch.
    public static func ndbPitch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pitch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbPitch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pitch", value))
    }
    /// Hearings to confirm.
    public static func ndbConfirm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "confirm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbConfirm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "confirm", value))
    }
    /// Carrier threshold.
    public static func ndbCarrierThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "carrier_threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbCarrierThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "carrier_threshold", value))
    }
    /// Synchronous tone detection.
    public static func ndbSyncTones(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sync_tones", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbSyncTones(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sync_tones", value))
    }
    /// Average repeated idents.
    public static func ndbIdentAverage(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "ident_average", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbIdentAverage(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "ident_average", value))
    }
    /// Scan threshold.
    public static func ndbScanThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbScanThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_threshold", value))
    }
    /// Scan every.
    public static func ndbScanInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbScanInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_interval", value))
    }
    /// Course (OBS).
    public static func vorObs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "obs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorObs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "obs", value))
    }
    /// Averaging.
    public static func vorAverage(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "average", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorAverage(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "average", value))
    }
    /// Hearings to confirm.
    public static func vorConfirm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "confirm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorConfirm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "confirm", value))
    }
    /// Carrier threshold.
    public static func vorCarrierThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "carrier_threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorCarrierThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "carrier_threshold", value))
    }
    /// The options of the setting "part" of the stack "ils".
    public enum IlsPart: Int32 {
        case auto = 0
        case localizer = 1
        case glideslope = 2
        case marker = 3
    }
    /// Part.
    public static func ilsPart(_ handle: SDRKIT_HANDLE) -> Result<IlsPart, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "part", &v)
        return code == SDRKIT_OK ? .success(IlsPart(rawValue: v) ?? IlsPart(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsPart(_ handle: SDRKIT_HANDLE, _ value: IlsPart) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "part", value.rawValue))
    }
    /// Averaging.
    public static func ilsAverage(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "average", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsAverage(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "average", value))
    }
    /// Hearings to confirm.
    public static func ilsConfirm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "confirm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsConfirm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "confirm", value))
    }
    /// Carrier threshold.
    public static func ilsCarrierThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "carrier_threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsCarrierThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "carrier_threshold", value))
    }
    /// The options of the setting "mode" of the stack "dme".
    public enum DmeMode: Int32 {
        case auto = 0
        case x = 1
        case y = 2
    }
    /// Channel mode.
    public static func dmeMode(_ handle: SDRKIT_HANDLE) -> Result<DmeMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mode", &v)
        return code == SDRKIT_OK ? .success(DmeMode(rawValue: v) ?? DmeMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeMode(_ handle: SDRKIT_HANDLE, _ value: DmeMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mode", value.rawValue))
    }
    /// Pulse threshold.
    public static func dmeThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// Hearings to confirm.
    public static func dmeConfirm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "confirm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeConfirm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "confirm", value))
    }
    /// GRI.
    public static func loranGri(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gri", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranGri(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gri", value))
    }
    /// Averaging.
    public static func loranAverage(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "average", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranAverage(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "average", value))
    }
    /// Secondary threshold.
    public static func loranThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// Secondaries.
    public static func loranMaxSecondaries(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_secondaries", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranMaxSecondaries(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_secondaries", value))
    }
    /// Chains.
    public static func loranChains(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "chains", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranChains(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "chains", $0) })
    }
    /// The options of the setting "bitrate" of the stack "dgps".
    public enum DgpsBitrate: Int32 {
        case automatic = 0
        case n50Bps = 1
        case n100Bps = 2
        case n200Bps = 3
    }
    /// Bit rate.
    public static func dgpsBitrate(_ handle: SDRKIT_HANDLE) -> Result<DgpsBitrate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bitrate", &v)
        return code == SDRKIT_OK ? .success(DgpsBitrate(rawValue: v) ?? DgpsBitrate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDgpsBitrate(_ handle: SDRKIT_HANDLE, _ value: DgpsBitrate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bitrate", value.rawValue))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum AvnavGeneratorSettings {
    /// Test signal: identification.
    public static func ndbIdent(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ident", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbIdent(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ident", $0) })
    }
    /// The options of the setting "gen_keying" of the stack "ndb".
    public enum NdbKeying: Int32 {
        case a2a = 0
        case a1a = 1
    }
    /// Test signal: keying.
    public static func ndbKeying(_ handle: SDRKIT_HANDLE) -> Result<NdbKeying, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_keying", &v)
        return code == SDRKIT_OK ? .success(NdbKeying(rawValue: v) ?? NdbKeying(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbKeying(_ handle: SDRKIT_HANDLE, _ value: NdbKeying) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_keying", value.rawValue))
    }
    /// Test signal: tone.
    public static func ndbTone(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbTone(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone", value))
    }
    /// Test signal: modulation depth.
    public static func ndbDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// Test signal: Morse speed.
    public static func ndbWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm", value))
    }
    /// Test signal: pause.
    public static func ndbPause(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pause", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbPause(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pause", value))
    }
    /// Test signal: other beacons.
    public static func ndbOthers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_others", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNdbOthers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_others", value))
    }
    /// The options of the setting "gen_kind" of the stack "vor".
    public enum VorKind: Int32 {
        case conventional = 0
        case doppler = 1
    }
    /// Test signal: station.
    public static func vorKind(_ handle: SDRKIT_HANDLE) -> Result<VorKind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &v)
        return code == SDRKIT_OK ? .success(VorKind(rawValue: v) ?? VorKind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setVorKind(_ handle: SDRKIT_HANDLE, _ value: VorKind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_kind", value.rawValue))
    }
    /// Test signal: radial.
    public static func vorRadial(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_radial", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorRadial(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_radial", value))
    }
    /// Test signal: identification.
    public static func vorIdent(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ident", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVorIdent(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ident", $0) })
    }
    /// Test signal: Morse speed.
    public static func vorWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm", value))
    }
    /// Test signal: pause between idents.
    public static func vorPause(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pause", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorPause(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pause", value))
    }
    /// Test signal: radial change.
    public static func vorRotate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rotate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVorRotate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rotate", value))
    }
    /// Test signal: DDM.
    public static func ilsDdm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ddm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsDdm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ddm", value))
    }
    /// Test signal: identification.
    public static func ilsIdent(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ident", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsIdent(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ident", $0) })
    }
    /// Test signal: Morse speed.
    public static func ilsWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm", value))
    }
    /// Test signal: pause between idents.
    public static func ilsPause(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pause", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsPause(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pause", value))
    }
    /// The options of the setting "gen_marker" of the stack "ils".
    public enum IlsMarker: Int32 {
        case outer = 0
        case middle = 1
        case inner = 2
    }
    /// Test signal: marker.
    public static func ilsMarker(_ handle: SDRKIT_HANDLE) -> Result<IlsMarker, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_marker", &v)
        return code == SDRKIT_OK ? .success(IlsMarker(rawValue: v) ?? IlsMarker(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsMarker(_ handle: SDRKIT_HANDLE, _ value: IlsMarker) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_marker", value.rawValue))
    }
    /// The options of the setting "gen_marker_keying" of the stack "ils".
    public enum IlsMarkerKeying: Int32 {
        case standard = 0
        case steady = 1
    }
    /// Test signal: marker keying.
    public static func ilsMarkerKeying(_ handle: SDRKIT_HANDLE) -> Result<IlsMarkerKeying, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_marker_keying", &v)
        return code == SDRKIT_OK ? .success(IlsMarkerKeying(rawValue: v) ?? IlsMarkerKeying(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsMarkerKeying(_ handle: SDRKIT_HANDLE, _ value: IlsMarkerKeying) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_marker_keying", value.rawValue))
    }
    /// Test signal: marker tone.
    public static func ilsMarkerTone(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_marker_tone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsMarkerTone(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_marker_tone", value))
    }
    /// Test signal: marker depth.
    public static func ilsMarkerDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_marker_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsMarkerDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_marker_depth", value))
    }
    /// Test signal: marker keyed from.
    public static func ilsMarkerStart(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_marker_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsMarkerStart(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_marker_start", value))
    }
    /// Test signal: marker keyed until.
    public static func ilsMarkerStop(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_marker_stop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIlsMarkerStop(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_marker_stop", value))
    }
    /// The options of the setting "gen_mode" of the stack "dme".
    public enum DmeMode: Int32 {
        case x = 0
        case y = 1
    }
    /// Test signal: mode.
    public static func dmeMode(_ handle: SDRKIT_HANDLE) -> Result<DmeMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(DmeMode(rawValue: v) ?? DmeMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeMode(_ handle: SDRKIT_HANDLE, _ value: DmeMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// The options of the setting "gen_kind" of the stack "dme".
    public enum DmeKind: Int32 {
        case beacon = 0
        case aircraft = 1
    }
    /// Test signal: transmitter.
    public static func dmeKind(_ handle: SDRKIT_HANDLE) -> Result<DmeKind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &v)
        return code == SDRKIT_OK ? .success(DmeKind(rawValue: v) ?? DmeKind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeKind(_ handle: SDRKIT_HANDLE, _ value: DmeKind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_kind", value.rawValue))
    }
    /// Test signal: pairs a second.
    public static func dmePpps(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ppps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmePpps(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ppps", value))
    }
    /// Test signal: identification.
    public static func dmeIdent(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ident", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeIdent(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ident", $0) })
    }
    /// Test signal: Morse speed.
    public static func dmeWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmeWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm", value))
    }
    /// Test signal: pause.
    public static func dmePause(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pause", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmePause(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pause", value))
    }
    /// Test signal: GRI.
    public static func loranGri(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_gri", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranGri(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_gri", value))
    }
    /// Test signal: TDs (µs).
    public static func loranTds(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tds", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranTds(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tds", $0) })
    }
    /// Test signal: ECD of the first secondary.
    public static func loranEcd(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ecd", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoranEcd(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ecd", value))
    }
    /// The options of the setting "gen_bitrate" of the stack "dgps".
    public enum DgpsBitrate: Int32 {
        case n50 = 0
        case n100 = 1
        case n200 = 2
    }
    /// Test signal: bit rate.
    public static func dgpsBitrate(_ handle: SDRKIT_HANDLE) -> Result<DgpsBitrate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bitrate", &v)
        return code == SDRKIT_OK ? .success(DgpsBitrate(rawValue: v) ?? DgpsBitrate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDgpsBitrate(_ handle: SDRKIT_HANDLE, _ value: DgpsBitrate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bitrate", value.rawValue))
    }
    /// Test signal: station id.
    public static func dgpsStation(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_station", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDgpsStation(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_station", value))
    }
    /// Test signal: latitude.
    public static func dgpsLat(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDgpsLat(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lat", value))
    }
    /// Test signal: longitude.
    public static func dgpsLon(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDgpsLon(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lon", value))
    }
    /// Test signal: text.
    public static func dgpsText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDgpsText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
}

// sdrkit: the typed interface of the stack family "science", version 1.2. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum ScienceInterface {
    public static let major = 1
    public static let minor = 2
}

/// The result fields of `sonde`.
public struct SondeFields {
    /// The number of sondes in the table.
    public var sondes: Int64
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Carrier searches so far.
    public var searches: Int64
    /// Channels in use.
    public var channelsUsed: Int64
    /// Channels allowed.
    public var channelsAllowed: Int64
    /// The band searched (the input rate).
    public var band: Double
    /// The capture is narrower than 2 MHz: only ±0.45 of the rate is searched.
    public var narrow: Bool
    /// The row key of the sonde the plots show.
    public var selected: String?

    init(_ c: sdrkit_sonde_fields) {
        sondes = c.sondes
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        searches = c.searches
        channelsUsed = c.channels_used
        channelsAllowed = c.channels_allowed
        band = c.band
        narrow = c.narrow
        selected = c.has_selected ? sdrkitText(c.selected) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SondeFields, SDRKitCode> {
        var c = sdrkit_sonde_fields()
        let code = sdrkit_sonde_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SondeFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the table `sondes`.
public struct SondeRow {
    public enum TypeOption: Int32 {
        case rs41 = 0
        case rs92 = 1
        case dfm = 2
        case m10 = 3
        case m20 = 4
        case lms6 = 5
    }
    /// The row key.
    public var key: String
    /// The family.
    public var type: TypeOption?
    /// The subtype.
    public var subtype: String?
    /// The serial.
    public var serial: String
    /// The receiving channel.
    public var channel: Int64?
    /// The absolute frequency.
    public var frequency: Double?
    /// The offset from the centre.
    public var offset: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The frame counter.
    public var frame: Int64?
    /// UTC from the sonde.
    public var time: Double?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The height of the object that the event is about, metres above mean sea level.
    public var heightM: Double?
    /// The highest altitude.
    public var maxAltM: Double?
    /// The vertical speed.
    public var climbMS: Double?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The satellites used.
    public var sats: Int64?
    /// The temperature.
    public var tempC: Double?
    /// The relative humidity.
    public var humidityPct: Double?
    /// The pressure.
    public var pressureHpa: Double?
    /// The battery.
    public var batteryV: Double?
    /// The burst or kill timer.
    public var kill: Double?
    /// The calibration parts held.
    public var calHeld: Int64?
    /// The calibration parts needed.
    public var calNeeded: Int64?
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Good frames as a share of all frames.
    public var okPct: Double
    /// The predicted landing point.
    public var landing: SDRKitPosition?
    /// The time to the landing.
    public var landingIn: Double?
    /// The distance from the station to the landing point.
    public var landingDistanceM: Double?
    /// The bearing from the station to the landing point.
    public var landingBearingDeg: Double?
    /// Times heard.
    public var heard: Int64?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_sonde_row) {
        key = sdrkitText(c.key)
        type = TypeOption(rawValue: c.type)
        subtype = c.has_subtype ? sdrkitText(c.subtype) : nil
        serial = sdrkitText(c.serial)
        channel = c.has_channel ? c.channel : nil
        frequency = c.has_frequency ? c.frequency : nil
        offset = c.offset
        snrDb = c.snr_db
        frame = c.has_frame ? c.frame : nil
        time = c.has_time ? c.time : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        heightM = c.has_height_m ? c.height_m : nil
        maxAltM = c.has_max_alt_m ? c.max_alt_m : nil
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        sats = c.has_sats ? c.sats : nil
        tempC = c.has_temp_c ? c.temp_c : nil
        humidityPct = c.has_humidity_pct ? c.humidity_pct : nil
        pressureHpa = c.has_pressure_hpa ? c.pressure_hpa : nil
        batteryV = c.has_battery_v ? c.battery_v : nil
        kill = c.has_kill ? c.kill : nil
        calHeld = c.has_cal_held ? c.cal_held : nil
        calNeeded = c.has_cal_needed ? c.cal_needed : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        okPct = c.ok_pct
        landing = c.has_landing ? SDRKitPosition(c.landing) : nil
        landingIn = c.has_landing_in ? c.landing_in : nil
        landingDistanceM = c.has_landing_distance_m ? c.landing_distance_m : nil
        landingBearingDeg = c.has_landing_bearing_deg ? c.landing_bearing_deg : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A carrier of the last search. The row key is the place in the list, two digits.
public struct SondeCarrierRow {
    /// The row key.
    public var key: String
    /// The offset from the centre.
    public var offset: Double
    /// The absolute frequency.
    public var frequency: Double?
    /// The carrier level.
    public var levelDb: Double
    /// The receiving channel.
    public var channel: Int64?

    init(_ c: sdrkit_sonde_carrier_row) {
        key = sdrkitText(c.key)
        offset = c.offset
        frequency = c.has_frequency ? c.frequency : nil
        levelDb = c.level_db
        channel = c.has_channel ? c.channel : nil
    }
}

/// The result fields of `time`.
public struct TimeFields {
    public enum Station: Int32 {
        case dcf77 = 0
        case msf = 1
        case wwvb = 2
        case jjy40 = 3
        case jjy60 = 4
        case bpc = 5
        case wwv = 6
        case chu = 7
        case bpm = 8
        case bpmc = 9
        case hla = 10
        case rwm = 11
    }
    public enum Path: Int32 {
        case groundWave = 0
        case skyWave = 1
    }
    public enum Weekday: Int32 {
        case monday = 0
        case tuesday = 1
        case wednesday = 2
        case thursday = 3
        case friday = 4
        case saturday = 5
        case sunday = 6
    }
    /// The station decoded.
    public var station: Station?
    /// WWVH (1200 Hz ticks) rather than WWV (1000 Hz ticks).
    public var wwvh: Bool?
    /// The carrier level.
    public var carrierDbfs: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The keying depth (LF) or the tick level over the background (HF).
    public var depthPct: Double
    /// Where the second's edge falls, after a whole second of signal time.
    public var tickMs: Double
    /// The jitter of the edges.
    public var jitterMs: Double
    /// Minutes decoded.
    public var minutesOk: Int64
    /// Minutes refused.
    public var minutesBad: Int64
    /// Two minutes in a row agree.
    public var confirmed: Bool
    /// The distance from the station.
    public var distanceM: Double?
    /// The path delay.
    public var delayMs: Double?
    /// How the path delay was found.
    public var path: Path?
    /// The symbols of this minute so far.
    public var symbols: String
    /// The received UTC now.
    public var utc: Double?
    /// The last minute as the station sends it.
    public var received: String?
    /// DUT1 (UT1 − UTC).
    public var dut1S: Double?
    /// Summer time is in force.
    public var summer: Bool?
    /// The day of the week sent, by name.
    public var weekday: Weekday?
    /// The computer clock minus the received time, now.
    public var clockOffsetS: Double?
    /// The same, averaged over the last minute.
    public var clockOffsetAvgS: Double?

    init(_ c: sdrkit_time_fields) {
        station = Station(rawValue: c.station)
        wwvh = c.has_wwvh ? c.wwvh : nil
        carrierDbfs = c.carrier_dbfs
        snrDb = c.snr_db
        depthPct = c.depth_pct
        tickMs = c.tick_ms
        jitterMs = c.jitter_ms
        minutesOk = c.minutes_ok
        minutesBad = c.minutes_bad
        confirmed = c.confirmed
        distanceM = c.has_distance_m ? c.distance_m : nil
        delayMs = c.has_delay_ms ? c.delay_ms : nil
        path = c.has_path ? Path(rawValue: c.path) : nil
        symbols = sdrkitText(c.symbols)
        utc = c.has_utc ? c.utc : nil
        received = c.has_received ? sdrkitText(c.received) : nil
        dut1S = c.has_dut1_s ? c.dut1_s : nil
        summer = c.has_summer ? c.summer : nil
        weekday = c.has_weekday ? Weekday(rawValue: c.weekday) : nil
        clockOffsetS = c.has_clock_offset_s ? c.clock_offset_s : nil
        clockOffsetAvgS = c.has_clock_offset_avg_s ? c.clock_offset_avg_s : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TimeFields, SDRKitCode> {
        var c = sdrkit_time_fields()
        let code = sdrkit_time_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TimeFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A minute of `time`. The row key is the sequence number of the minute.
public struct TimeMinuteRow {
    public enum Refusal: Int32 {
        case symbol = 0
        case notRead = 1
        case marker = 2
        case fixedBit = 3
        case fixedBits = 4
        case keyed59 = 5
        case keyed0 = 6
        case summerBits = 7
        case parity = 8
        case valueRange = 9
        case dateRange = 10
        case dayOfYear = 11
        case yearRange = 12
        case weekdayRange = 13
        case weekdayMismatch = 14
        case noMinuteMarker = 15
        case noMinutePulse = 16
        case dut1Sign = 17
        case dut1Doubled = 18
        case leapYearBit = 19
        case frameIndex = 20
        case fromBursts = 21
        case noP0 = 22
        case copiesDiffer = 23
        case notAFrame = 24
        case noSElement = 25
        case noMinuteFrame = 26
    }
    /// The row key.
    public var key: String
    /// The signal time of a minute.
    public var timeS: Double
    /// The host time of a minute.
    public var hostTime: Double?
    /// The decoded flag of a minute.
    public var ok: Bool
    /// The text of a minute.
    public var text: String?
    /// Why a minute was refused.
    public var refusal: Refusal?
    /// The second where the decoder stopped.
    public var second: Int64?
    /// The symbols of this minute so far.
    public var symbols: String

    init(_ c: sdrkit_time_minute_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        hostTime = c.has_host_time ? c.host_time : nil
        ok = c.ok
        text = c.has_text ? sdrkitText(c.text) : nil
        refusal = c.has_refusal ? Refusal(rawValue: c.refusal) : nil
        second = c.has_second ? c.second : nil
        symbols = sdrkitText(c.symbols)
    }
}

/// The result fields of `eczas`.
public struct EczasFields {
    public enum TxState: Int32 {
        case normal = 0
        case offDay = 1
        case offWeek = 2
        case offLonger = 3
    }
    /// The received UTC now.
    public var utc: Double?
    /// The hours that the packet adds to UTC for the local time in Poland.
    public var tzHours: Int64?
    /// A leap second is announced.
    public var leapAnnounce: Bool?
    /// The announced leap second is taken away.
    public var leapSubtract: Bool?
    /// A change of summer time is near.
    public var dstChangeSoon: Bool?
    /// The transmitter state.
    public var txState: TxState?
    /// The UTC of the last time packet.
    public var lastPacket: Double?
    /// Two packets in a row agree.
    public var confirmed: Bool
    /// Carrier offset from where it was expected, Hz.
    public var offsetHz: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// The quality of the last sync.
    public var syncQuality: Double
    /// Syncs found.
    public var syncs: Int64
    /// Frames cut.
    public var frames: Int64
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// Packets of the other slots.
    public var otherSlots: Int64
    /// Syncs not followed by the sync word.
    public var falseSyncs: Int64
    /// The distance from the transmitter.
    public var distanceM: Double?
    /// The path delay.
    public var delayMs: Double?
    /// The computer clock minus the received time, now.
    public var clockOffsetS: Double?
    /// The same, averaged over the last 10 packets.
    public var clockOffsetAvgS: Double?

    init(_ c: sdrkit_eczas_fields) {
        utc = c.has_utc ? c.utc : nil
        tzHours = c.has_tz_hours ? c.tz_hours : nil
        leapAnnounce = c.has_leap_announce ? c.leap_announce : nil
        leapSubtract = c.has_leap_subtract ? c.leap_subtract : nil
        dstChangeSoon = c.has_dst_change_soon ? c.dst_change_soon : nil
        txState = c.has_tx_state ? TxState(rawValue: c.tx_state) : nil
        lastPacket = c.has_last_packet ? c.last_packet : nil
        confirmed = c.confirmed
        offsetHz = c.offset_hz
        snrDb = c.snr_db
        levelDbfs = c.level_dbfs
        syncQuality = c.sync_quality
        syncs = c.syncs
        frames = c.frames
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        otherSlots = c.other_slots
        falseSyncs = c.false_syncs
        distanceM = c.has_distance_m ? c.distance_m : nil
        delayMs = c.has_delay_ms ? c.delay_ms : nil
        clockOffsetS = c.has_clock_offset_s ? c.clock_offset_s : nil
        clockOffsetAvgS = c.has_clock_offset_avg_s ? c.clock_offset_avg_s : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EczasFields, SDRKitCode> {
        var c = sdrkit_eczas_fields()
        let code = sdrkit_eczas_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EczasFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of `eczas`. The row key is the sequence number of the frame.
public struct EczasFrameRow {
    public enum Kind: Int32 {
        case time = 0
        case bad = 1
        case other = 2
        case disagrees = 3
    }
    /// The row key.
    public var key: String
    /// The signal time of a frame.
    public var signalTimeS: Double
    /// The host time of a frame.
    public var hostTime: Double?
    /// What a frame is.
    public var kind: Kind?
    /// The frame was accepted.
    public var ok: Bool
    /// The text of a frame.
    public var text: String
    /// The bytes of a frame.
    public var bytes: [UInt8]

    init(_ c: sdrkit_eczas_frame_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        hostTime = c.has_host_time ? c.host_time : nil
        kind = Kind(rawValue: c.kind)
        ok = c.ok
        text = sdrkitText(c.text)
        bytes = [UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len))
    }
}

/// The result fields of `same`.
public struct SameFields {
    public enum Channel: Int32 {
        case wx1 = 0
        case wx2 = 1
        case wx3 = 2
        case wx4 = 3
        case wx5 = 4
        case wx6 = 5
        case wx7 = 6
    }
    public enum Country: Int32 {
        case us = 0
        case canada = 1
    }
    /// The channel.
    public var channel: Channel?
    /// The channel frequency.
    public var frequency: Double
    /// Messages decoded.
    public var messagesOk: Int64
    /// Messages damaged.
    public var messagesBad: Int64
    /// An attention tone is on the air now.
    public var toneNow: Bool
    /// The country of the location names.
    public var country: Country?
    /// Counties in the loaded table.
    public var counties: Int64
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// The carrier error.
    public var carrier: Double

    init(_ c: sdrkit_same_fields) {
        channel = Channel(rawValue: c.channel)
        frequency = c.frequency
        messagesOk = c.messages_ok
        messagesBad = c.messages_bad
        toneNow = c.tone_now
        country = Country(rawValue: c.country)
        counties = c.counties
        snrDb = c.snr_db
        levelDbfs = c.level_dbfs
        carrier = c.carrier
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SameFields, SDRKitCode> {
        var c = sdrkit_same_fields()
        let code = sdrkit_same_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SameFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A SAME message. The row key is the count of rows added before it, as six digits.
public struct SameAlertRow {
    public enum Kind: Int32 {
        case alert = 0
        case end = 1
        case damaged = 2
    }
    public enum EventKind: Int32 {
        case warning = 0
        case watch = 1
        case emergency = 2
        case statement = 3
        case test = 4
        case message = 5
    }
    public enum Error: Int32 {
        case noStart = 0
        case originator = 1
        case originatorDash = 2
        case eventCode = 3
        case eventDash = 4
        case locationCode = 5
        case endsInLocations = 6
        case tooManyLocations = 7
        case locationSeparator = 8
        case purgeTime = 9
        case purgeMinutes = 10
        case purgeZero = 11
        case purgeDash = 12
        case issueTime = 13
        case issueDay = 14
        case issueRange = 15
        case issueDash = 16
        case callsignDash = 17
        case callsignLength = 18
        case callsign = 19
        case trailing = 20
    }
    /// The row key.
    public var key: String
    /// The signal time of a row.
    public var signalTimeS: Double
    /// The host time of a row.
    public var hostTime: Double?
    /// What the row is.
    public var kind: Kind?
    /// The event code.
    public var event: String?
    /// The event name.
    public var eventName: String?
    /// The kind of event (warning, watch ...).
    public var eventKind: EventKind?
    /// The originator code.
    public var originator: String?
    /// The originator's name.
    public var originatorName: String?
    /// The number of locations.
    public var locations: Int64
    /// How long the message is valid.
    public var validFor: Double?
    /// When the message was issued.
    public var issued: Double?
    /// The sender's call sign.
    public var callsign: String?
    /// The repeats heard.
    public var repeats: Int64
    /// The repeats identical to the result.
    public var agreeing: Int64
    /// Bits the vote changed.
    public var bitsChanged: Int64
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The carrier error.
    public var carrier: Double
    /// The attention tone that followed.
    public var tone: Double?
    /// The raw text.
    public var raw: String
    /// Why the message is damaged.
    public var error: Error?

    init(_ c: sdrkit_same_alert_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        hostTime = c.has_host_time ? c.host_time : nil
        kind = Kind(rawValue: c.kind)
        event = c.has_event ? sdrkitText(c.event) : nil
        eventName = c.has_event_name ? sdrkitText(c.event_name) : nil
        eventKind = c.has_event_kind ? EventKind(rawValue: c.event_kind) : nil
        originator = c.has_originator ? sdrkitText(c.originator) : nil
        originatorName = c.has_originator_name ? sdrkitText(c.originator_name) : nil
        locations = c.locations
        validFor = c.has_valid_for ? c.valid_for : nil
        issued = c.has_issued ? c.issued : nil
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        repeats = c.repeats
        agreeing = c.agreeing
        bitsChanged = c.bits_changed
        snrDb = c.snr_db
        carrier = c.carrier
        tone = c.has_tone ? c.tone : nil
        raw = sdrkitText(c.raw)
        error = c.has_error ? Error(rawValue: c.error) : nil
    }
}

/// A location of a SAME message. The row key is `<alert>/<n>`.
public struct SameAlertLocationRow {
    /// The row key.
    public var key: String
    /// The alert a location belongs to.
    public var alert: String
    /// A location code (PSSCCC).
    public var code: String
    /// The location as people read it.
    public var place: String
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?

    init(_ c: sdrkit_same_alert_location_row) {
        key = sdrkitText(c.key)
        alert = sdrkitText(c.alert)
        code = sdrkitText(c.code)
        place = sdrkitText(c.place)
        position = c.has_position ? SDRKitPosition(c.position) : nil
    }
}

/// An attention tone. The row key is the sequence number of the tone.
public struct SameToneRow {
    /// The row key.
    public var key: String
    /// The signal time of a row.
    public var signalTimeS: Double
    /// The length of a tone.
    public var duration: Double
    /// The level of a tone.
    public var levelDb: Double

    init(_ c: sdrkit_same_tone_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        duration = c.duration
        levelDb = c.level_db
    }
}

/// The result fields of `mscatter`.
public struct MscatterFields {
    /// Pings counted.
    public var pings: Int64
    /// Pings in the last 60 minutes.
    public var rate: Int64
    /// Broadband bursts refused.
    public var rejected: Int64
    /// The beacon's offset from the centre.
    public var beacon: Double?
    /// The beacon level over the noise in one bin.
    public var beaconSnrDb: Double
    /// Noise floor of the channel, dBFS.
    public var noiseDbfs: Double
    /// The detection level.
    public var thresholdDb: Double
    /// The level of the ping on the air now.
    public var pingNowDb: Double?

    init(_ c: sdrkit_mscatter_fields) {
        pings = c.pings
        rate = c.rate
        rejected = c.rejected
        beacon = c.has_beacon ? c.beacon : nil
        beaconSnrDb = c.beacon_snr_db
        noiseDbfs = c.noise_dbfs
        thresholdDb = c.threshold_db
        pingNowDb = c.has_ping_now_db ? c.ping_now_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MscatterFields, SDRKitCode> {
        var c = sdrkit_mscatter_fields()
        let code = sdrkit_mscatter_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MscatterFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A meteor ping. The row key is the sequence number of the ping.
public struct MscatterPingRow {
    public enum Kind: Int32 {
        case underdense = 0
        case overdense = 1
    }
    /// The row key.
    public var key: String
    /// The number of a ping.
    public var id: Int64
    /// UTC of a ping.
    public var utc: Double
    /// The length of a ping.
    public var duration: Double
    /// The peak level over the background.
    public var peakSnrDb: Double
    /// The Doppler shift at the peak.
    public var doppler: Double
    /// The drift of the Doppler shift.
    public var driftHzS: Double?
    /// Underdense or overdense.
    public var kind: Kind?

    init(_ c: sdrkit_mscatter_ping_row) {
        key = sdrkitText(c.key)
        id = c.id
        utc = c.utc
        duration = c.duration
        peakSnrDb = c.peak_snr_db
        doppler = c.doppler
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        kind = Kind(rawValue: c.kind)
    }
}

/// A meteor shower. The row key is its name.
public struct MscatterShowerRow {
    /// The row key.
    public var key: String
    /// The name of a shower.
    public var name: String
    /// Days to the peak of a shower (below 0 after it).
    public var peakInDays: Int64
    /// The shower is active.
    public var active: Bool
    /// The zenithal hourly rate of a shower.
    public var zhr: Int64

    init(_ c: sdrkit_mscatter_shower_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        peakInDays = c.peak_in_days
        active = c.active
        zhr = c.zhr
    }
}

/// The result fields of SSDV.
public struct SsdvFields {
    /// Packets received.
    public var packetsOk: Int64
    /// Packets refused.
    public var packetsBad: Int64
    /// Packets the RS code corrected.
    public var corrected: Int64
    /// Pictures complete.
    public var picturesDone: Int64
    /// The speed of the last good packet.
    public var lastBaud: Int64?
    /// The last good packet came as 8N2 bytes.
    public var lastN82: Bool?

    init(_ c: sdrkit_ssdv_fields) {
        packetsOk = c.packets_ok
        packetsBad = c.packets_bad
        corrected = c.corrected
        picturesDone = c.pictures_done
        lastBaud = c.has_last_baud ? c.last_baud : nil
        lastN82 = c.has_last_n82 ? c.last_n82 : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SsdvFields, SDRKitCode> {
        var c = sdrkit_ssdv_fields()
        let code = sdrkit_ssdv_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SsdvFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An SSDV packet. The row key is the sequence number of the packet.
public struct SsdvPacketRow {
    /// The row key.
    public var key: String
    /// Signal time of a packet.
    public var timeS: Double
    /// Host time of a packet.
    public var hostTime: Double?
    /// The call sign.
    public var call: String
    /// The image id.
    public var image: Int64
    /// The packet id.
    public var packet: Int64
    /// Bytes corrected in a packet.
    public var rs: Int64
    /// The speed of a packet.
    public var baud: Int64
    /// 8N2 bytes.
    public var n82: Bool
    /// Inverted bits.
    public var inverted: Bool
    /// A packet without FEC.
    public var noFec: Bool
    /// The last packet of a picture.
    public var eoi: Bool

    init(_ c: sdrkit_ssdv_packet_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        hostTime = c.has_host_time ? c.host_time : nil
        call = sdrkitText(c.call)
        image = c.image
        packet = c.packet
        rs = c.rs
        baud = c.baud
        n82 = c.n82
        inverted = c.inverted
        noFec = c.no_fec
        eoi = c.eoi
    }
}

/// An SSDV picture. The row key is the call sign and the image id.
public struct SsdvPictureRow {
    /// The row key.
    public var key: String
    /// The call sign.
    public var call: String
    /// The image id.
    public var image: Int64
    /// The width of a picture.
    public var width: Int64
    /// The height of a picture.
    public var height: Int64
    /// The quality level of a picture.
    public var quality: Int64
    /// Packets held of a picture.
    public var held: Int64
    /// Packets missing of a picture.
    public var missing: Int64
    /// The picture is complete.
    public var complete: Bool
    /// The size of the JPEG.
    public var bytes: Int64

    init(_ c: sdrkit_ssdv_picture_row) {
        key = sdrkitText(c.key)
        call = sdrkitText(c.call)
        image = c.image
        width = c.width
        height = c.height
        quality = c.quality
        held = c.held
        missing = c.missing
        complete = c.complete
        bytes = c.bytes
    }
}

/// The result fields of `astro`.
public struct AstroFields {
    public enum Note: Int32 {
        case switchingNeedsRadio = 0
        case yFactor = 1
        case noLine = 2
    }
    /// Finished integrations.
    public var finished: Int64
    /// The progress of the current integration.
    public var progressPct: Double
    /// The width of a bin.
    public var bin: Double
    /// The width of a bin in velocity.
    public var binKmS: Double
    /// The span of the spectrum.
    public var span: Double
    /// The span in velocity.
    public var spanKmS: Double
    /// Bins flagged as RFI in the last spectrum.
    public var binsFlagged: Int64
    /// The frequency of the peak.
    public var peak: Double?
    /// The velocity of the peak.
    public var peakKmS: Double?
    /// The excess at the peak.
    public var excessPct: Double?
    /// The peak over the residual rms.
    public var lineSnr: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// The right ascension of the target now.
    public var targetRaDeg: Double
    /// The declination of the target now.
    public var targetDecDeg: Double
    /// The azimuth of the target, with a station position.
    public var targetAzDeg: Double?
    /// The elevation of the target, with a station position.
    public var targetElDeg: Double?
    /// The observer's velocity towards the target from the Earth's rotation.
    public var lsrRotationKms: Double?
    /// The observer's velocity towards the target from the Earth's orbit.
    public var lsrOrbitKms: Double?
    /// The observer's velocity towards the target from the Sun's motion.
    public var lsrSunKms: Double?
    /// The velocity of the peak towards the target, corrected to the local standard of rest.
    public var velLsrKms: Double?
    /// The receiver temperature from the Y factor.
    public var trxK: Double?
    /// The gain per kelvin from the Y factor.
    public var gainPerK: Double?
    /// A note on the calibration, the switching or the fit.
    public var note: Note?

    init(_ c: sdrkit_astro_fields) {
        finished = c.finished
        progressPct = c.progress_pct
        bin = c.bin
        binKmS = c.bin_km_s
        span = c.span
        spanKmS = c.span_km_s
        binsFlagged = c.bins_flagged
        peak = c.has_peak ? c.peak : nil
        peakKmS = c.has_peak_km_s ? c.peak_km_s : nil
        excessPct = c.has_excess_pct ? c.excess_pct : nil
        lineSnr = c.has_line_snr ? c.line_snr : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        targetRaDeg = c.target_ra_deg
        targetDecDeg = c.target_dec_deg
        targetAzDeg = c.has_target_az_deg ? c.target_az_deg : nil
        targetElDeg = c.has_target_el_deg ? c.target_el_deg : nil
        lsrRotationKms = c.has_lsr_rotation_kms ? c.lsr_rotation_kms : nil
        lsrOrbitKms = c.has_lsr_orbit_kms ? c.lsr_orbit_kms : nil
        lsrSunKms = c.has_lsr_sun_kms ? c.lsr_sun_kms : nil
        velLsrKms = c.has_vel_lsr_kms ? c.vel_lsr_kms : nil
        trxK = c.has_trx_k ? c.trx_k : nil
        gainPerK = c.has_gain_per_k ? c.gain_per_k : nil
        note = c.has_note ? Note(rawValue: c.note) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AstroFields, SDRKitCode> {
        var c = sdrkit_astro_fields()
        let code = sdrkit_astro_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AstroFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A bin of the calibrated spectrum of the last finished integration. The row key is the bin, five digits.
public struct AstroCalibratedRow {
    /// The row key.
    public var key: String
    /// The frequency of a bin.
    public var freq: Double
    /// The topocentric velocity of a bin.
    public var velTopoKms: Double
    /// The LSR velocity of a bin (none without a station position).
    public var velLsrKms: Double?
    /// The antenna temperature of a bin: the processed ratio times the system temperature.
    public var taK: Double
    /// The bin was flagged as RFI.
    public var flagged: Bool

    init(_ c: sdrkit_astro_calibrated_row) {
        key = sdrkitText(c.key)
        freq = c.freq
        velTopoKms = c.vel_topo_kms
        velLsrKms = c.has_vel_lsr_kms ? c.vel_lsr_kms : nil
        taK = c.ta_k
        flagged = c.flagged
    }
}

/// A finished integration of the session. The row key is the sequence number of the integration.
public struct AstroSessionRow {
    /// The row key.
    public var key: String
    /// The signal time at the end of an integration.
    public var signalTimeS: Double
    /// The peak velocity of an integration.
    public var peakKms: Double
    /// The excess at the peak.
    public var excessPct: Double
    /// The peak over the residual rms.
    public var lineSnr: Double

    init(_ c: sdrkit_astro_session_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        peakKms = c.peak_kms
        excessPct = c.excess_pct
        lineSnr = c.line_snr
    }
}

/// A Gaussian of the last line fit. The row key is the place in the fit.
public struct AstroFitRow {
    /// The row key.
    public var key: String
    /// The centre of a fitted Gaussian.
    public var centreKms: Double
    /// The amplitude of a fitted Gaussian.
    public var amplitudeK: Double
    /// The full width at half maximum of a fitted Gaussian.
    public var fwhmKms: Double

    init(_ c: sdrkit_astro_fit_row) {
        key = sdrkitText(c.key)
        centreKms = c.centre_kms
        amplitudeK = c.amplitude_k
        fwhmKms = c.fwhm_kms
    }
}

/// A row of the table `lv_rows`.
public struct AstroMapRow {
    /// The row key.
    public var key: String
    /// The galactic longitude of a row of the map.
    public var longitude: Double
    /// The terminal velocity at a longitude.
    public var terminalKms: Double?

    init(_ c: sdrkit_astro_map_row) {
        key = sdrkitText(c.key)
        longitude = c.longitude
        terminalKms = c.has_terminal_kms ? c.terminal_kms : nil
    }
}

/// The result fields of `iono`.
public struct IonoFields {
    /// The input rate.
    public var rate: Double
    /// The radio frequency of the centre.
    public var centre: Double
    /// The signal time (UTC with a radio, else from the first sample).
    public var signalTimeS: Double
    /// Sweeps start on whole seconds.
    public var timingLock: Bool
    /// The span the radio sees.
    public var span: Double
    /// Sounders heard.
    public var soundersHeard: Int64
    /// Sweeps being received.
    public var activeSweeps: Int64
    /// The FFT size of the detector.
    public var detectorFft: Int64?
    /// The duration of a detector FFT.
    public var detectorMs: Double?
    /// The bin width of the detector.
    public var detectorBin: Double?
    /// FFTs done.
    public var ffts: Int64?
    /// Chirp peaks not yet tied to a sweep.
    public var loosePeaks: Int64?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// The row key of the sounder whose ionogram and history are shown.
    public var selected: String?

    init(_ c: sdrkit_iono_fields) {
        rate = c.rate
        centre = c.centre
        signalTimeS = c.signal_time_s
        timingLock = c.timing_lock
        span = c.span
        soundersHeard = c.sounders_heard
        activeSweeps = c.active_sweeps
        detectorFft = c.has_detector_fft ? c.detector_fft : nil
        detectorMs = c.has_detector_ms ? c.detector_ms : nil
        detectorBin = c.has_detector_bin ? c.detector_bin : nil
        ffts = c.has_ffts ? c.ffts : nil
        loosePeaks = c.has_loose_peaks ? c.loose_peaks : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        selected = c.has_selected ? sdrkitText(c.selected) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IonoFields, SDRKitCode> {
        var c = sdrkit_iono_fields()
        let code = sdrkit_iono_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IonoFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A sounder heard. The row key is the sounder's id.
public struct IonoSounderRow {
    public enum Geometry: Int32 {
        case auto = 0
        case vertical = 1
        case oblique = 2
    }
    /// The row key.
    public var key: String
    /// The id of a sounder.
    public var id: Int64
    /// A sweep rate.
    public var rateKhzS: Double
    /// The repetition period.
    public var period: Double?
    /// The start offset in the period.
    public var offsetS: Double?
    /// The start of the last sweep.
    public var lastStartS: Double
    /// When the sounder was last heard.
    public var lastSeenS: Double
    /// The start of the next sweep.
    public var nextS: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The lowest radio frequency heard.
    public var heardLow: Double
    /// The highest radio frequency heard.
    public var heardHigh: Double
    /// Sweeps received.
    public var sweeps: Int64
    /// Expected sweeps not heard.
    public var missed: Int64
    /// A sweep is being received.
    public var active: Bool
    /// The name of the known sounder it matches.
    public var known: String?
    /// How well it matches: 0 no match, 1 the rate (and the distance), 2 the rate and the schedule.
    public var matchLevel: Int64
    /// The known sounders of the same rate.
    public var candidates: Int64
    /// The geometry of the latest scaling.
    public var geometry: Geometry?
    /// The critical frequency of the E layer.
    public var foeMhz: Double?
    /// The critical frequency of the F1 layer.
    public var fof1Mhz: Double?
    /// The critical frequency of the F2 layer.
    public var fof2Mhz: Double?
    /// The virtual height of the E layer.
    public var hpeKm: Double?
    /// The virtual height of the F layer.
    public var hpfKm: Double?
    /// The propagation factor M(3000)F2.
    public var m3000: Double?
    /// The MUF over 3000 km.
    public var muf3000Mhz: Double?
    /// The MUF of the path from an oblique sounder.
    public var pathMufMhz: Double?
    /// The distance of an oblique sounder.
    public var distanceKm: Double?

    init(_ c: sdrkit_iono_sounder_row) {
        key = sdrkitText(c.key)
        id = c.id
        rateKhzS = c.rate_khz_s
        period = c.has_period ? c.period : nil
        offsetS = c.has_offset_s ? c.offset_s : nil
        lastStartS = c.last_start_s
        lastSeenS = c.last_seen_s
        nextS = c.has_next_s ? c.next_s : nil
        snrDb = c.snr_db
        heardLow = c.heard_low
        heardHigh = c.heard_high
        sweeps = c.sweeps
        missed = c.missed
        active = c.active
        known = c.has_known ? sdrkitText(c.known) : nil
        matchLevel = c.match_level
        candidates = c.candidates
        geometry = c.has_geometry ? Geometry(rawValue: c.geometry) : nil
        foeMhz = c.has_foe_mhz ? c.foe_mhz : nil
        fof1Mhz = c.has_fof1_mhz ? c.fof1_mhz : nil
        fof2Mhz = c.has_fof2_mhz ? c.fof2_mhz : nil
        hpeKm = c.has_hpe_km ? c.hpe_km : nil
        hpfKm = c.has_hpf_km ? c.hpf_km : nil
        m3000 = c.has_m3000 ? c.m3000 : nil
        muf3000Mhz = c.has_muf3000_mhz ? c.muf3000_mhz : nil
        pathMufMhz = c.has_path_muf_mhz ? c.path_muf_mhz : nil
        distanceKm = c.has_distance_km ? c.distance_km : nil
    }
}

/// A sweep heard. The row key is the sequence number of the sweep.
public struct IonoTimelineRow {
    /// The row key.
    public var key: String
    /// The start of a sweep.
    public var startS: Double
    /// The id of a sounder.
    public var id: Int64
    /// A sweep rate.
    public var rateKhzS: Double
    /// The lowest frequency of the sweep that crossed the span.
    public var partLow: Double
    /// The highest frequency of the sweep that crossed the span.
    public var partHigh: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The sweep was expected.
    public var predicted: Bool

    init(_ c: sdrkit_iono_timeline_row) {
        key = sdrkitText(c.key)
        startS = c.start_s
        id = c.id
        rateKhzS = c.rate_khz_s
        partLow = c.part_low
        partHigh = c.part_high
        snrDb = c.snr_db
        predicted = c.predicted
    }
}

/// A band and its state, from the selected sounder's usable MUF. The row key is the name of the band.
public struct IonoBandRow {
    public enum State: Int32 {
        case `open` = 0
        case closed = 1
        case absorbed = 2
        case unknown = 3
    }
    /// The row key.
    public var key: String
    /// A band.
    public var band: String
    /// The state of a band.
    public var state: State?

    init(_ c: sdrkit_iono_band_row) {
        key = sdrkitText(c.key)
        band = sdrkitText(c.band)
        state = State(rawValue: c.state)
    }
}

/// The result fields of `sferics`.
public struct SfericsFields {
    /// Strikes counted.
    public var strikes: Int64
    /// Flashes counted.
    public var flashes: Int64
    /// Impulses of a mains train left out.
    public var mains: Int64
    /// Impulses too long to be lightning.
    public var tooLong: Int64
    /// Strikes in the last minute.
    public var perMin1: Double
    /// Strikes per minute over 10 minutes.
    public var perMin10: Double
    /// The floor.
    public var floorDb: Double?
    /// The start level in use.
    public var thresholdDb: Double
    /// The peak of the newest strike.
    public var lastPeakDb: Double?
    /// The time since the newest strike.
    public var lastAgoS: Double?
    /// A mains train is being left out.
    public var mainsActive: Bool
    /// The storm phase.
    public var stormPhase: String
    /// The change of the median peak.
    public var stormDbChange: Double
    /// The activity word of the 10-minute rate.
    public var activity: String
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_sferics_fields) {
        strikes = c.strikes
        flashes = c.flashes
        mains = c.mains
        tooLong = c.too_long
        perMin1 = c.per_min_1
        perMin10 = c.per_min_10
        floorDb = c.has_floor_db ? c.floor_db : nil
        thresholdDb = c.threshold_db
        lastPeakDb = c.has_last_peak_db ? c.last_peak_db : nil
        lastAgoS = c.has_last_ago_s ? c.last_ago_s : nil
        mainsActive = c.mains_active
        stormPhase = sdrkitText(c.storm_phase)
        stormDbChange = c.storm_db_change
        activity = sdrkitText(c.activity)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SfericsFields, SDRKitCode> {
        var c = sdrkit_sferics_fields()
        let code = sdrkit_sferics_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SfericsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct ScienceDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_science_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ScienceDecoderFields, SDRKitCode> {
        var c = sdrkit_science_decoder_fields()
        let code = sdrkit_science_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ScienceDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A strike. The row key is its start, s of signal.
public struct SfericsStrikeRow {
    /// The row key.
    public var key: String
    /// The start of a strike.
    public var timeS: Double
    /// The start of a strike in UTC.
    public var utc: Double
    /// The peak of a strike over the floor.
    public var peakDb: Double
    /// The width of a strike.
    public var widthUs: Double
    /// The flash of a strike.
    public var flash: Int64

    init(_ c: sdrkit_sferics_strike_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        utc = c.utc
        peakDb = c.peak_db
        widthUs = c.width_us
        flash = c.flash
    }
}

/// The result fields of `supersid`.
public struct SupersidFields {
    /// Transmitters listed.
    public var listedN: Int64
    /// Transmitters in span.
    public var inSpanN: Int64
    /// Transmitters present.
    public var presentN: Int64
    /// Intervals measured.
    public var intervals: Int64
    /// The width of a bin of the spectrum.
    public var binHz: Double?
    /// Events in total.
    public var eventsTotal: Int64
    /// The transmitter of the last event.
    public var lastEventTx: String?
    /// The rise of the last event.
    public var lastEventRiseDb: Double?
    /// The age of the last event.
    public var lastEventAgeS: Double?
    /// The data file written last.
    public var logFile: String?
    /// Why the data file is not written.
    public var logError: String?
    /// The text of a decoded message.
    public var text: String
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_supersid_fields) {
        listedN = c.listed_n
        inSpanN = c.in_span_n
        presentN = c.present_n
        intervals = c.intervals
        binHz = c.has_bin_hz ? c.bin_hz : nil
        eventsTotal = c.events_total
        lastEventTx = c.has_last_event_tx ? sdrkitText(c.last_event_tx) : nil
        lastEventRiseDb = c.has_last_event_rise_db ? c.last_event_rise_db : nil
        lastEventAgeS = c.has_last_event_age_s ? c.last_event_age_s : nil
        logFile = c.has_log_file ? sdrkitText(c.log_file) : nil
        logError = c.has_log_error ? sdrkitText(c.log_error) : nil
        text = sdrkitText(c.text)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SupersidFields, SDRKitCode> {
        var c = sdrkit_supersid_fields()
        let code = sdrkit_supersid_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SupersidFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A monitored transmitter, in list order. The row key is its place in the list, two digits.
public struct SupersidTransmitterRow {
    /// The row key.
    public var key: String
    /// A transmitter's label.
    public var label: String
    /// The frequency a station was heard on.
    public var freqHz: Double
    /// In span.
    public var inSpan: Bool
    /// Present.
    public var present: Bool
    /// The level over the local noise.
    public var levelDb: Double?
    /// The absolute level.
    public var absoluteDbfs: Double?
    /// The median of the last hour.
    public var baselineDb: Double?
    /// The mean of the last 2 minutes less the baseline.
    public var riseDb: Double?
    /// An event is on.
    public var eventOn: Bool
    /// The peak step of the event that is on.
    public var eventPeakDb: Double?
    /// The sunlit share of the path.
    public var sunlit: Double?
    /// The key of the series of minute means.
    public var series: String

    init(_ c: sdrkit_supersid_transmitter_row) {
        key = sdrkitText(c.key)
        label = sdrkitText(c.label)
        freqHz = c.freq_hz
        inSpan = c.in_span
        present = c.present
        levelDb = c.has_level_db ? c.level_db : nil
        absoluteDbfs = c.has_absolute_dbfs ? c.absolute_dbfs : nil
        baselineDb = c.has_baseline_db ? c.baseline_db : nil
        riseDb = c.has_rise_db ? c.rise_db : nil
        eventOn = c.event_on
        eventPeakDb = c.has_event_peak_db ? c.event_peak_db : nil
        sunlit = c.has_sunlit ? c.sunlit : nil
        series = sdrkitText(c.series)
    }
}

/// A row of the table `sid_events`.
public struct SupersidEventRow {
    /// The row key.
    public var key: String
    /// A transmitter's label.
    public var label: String
    /// The frequency a station was heard on.
    public var freqHz: Double
    /// The UTC of an event's start.
    public var startUtc: Double
    /// The signal time of an event's start.
    public var signalTimeS: Double
    /// The mean of the last 2 minutes less the baseline.
    public var riseDb: Double
    /// The peak step of an event.
    public var peakDb: Double
    /// The UTC of an event's end.
    public var endUtc: Double?

    init(_ c: sdrkit_supersid_event_row) {
        key = sdrkitText(c.key)
        label = sdrkitText(c.label)
        freqHz = c.freq_hz
        startUtc = c.start_utc
        signalTimeS = c.signal_time_s
        riseDb = c.rise_db
        peakDb = c.peak_db
        endUtc = c.has_end_utc ? c.end_utc : nil
    }
}

/// A terminator mark of the UTC day. The row key is its place in time order, three digits.
public struct SupersidMarkRow {
    /// The row key.
    public var key: String
    /// A transmitter's label.
    public var label: String
    /// The point of the path of a mark.
    public var point: String
    /// The crossing of a mark.
    public var mark: String
    /// The UTC of a mark.
    public var utc: Double

    init(_ c: sdrkit_supersid_mark_row) {
        key = sdrkitText(c.key)
        label = sdrkitText(c.label)
        point = sdrkitText(c.point)
        mark = sdrkitText(c.mark)
        utc = c.utc
    }
}

/// The result fields of `schumann`.
public struct SchumannFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// The fundamental, Hz.
    public var fundamentalHz: Double?
    /// The height of the fundamental over the baseline.
    public var fundamentalDb: Double?
    /// The Q of the fundamental.
    public var fundamentalQ: Double?
    /// Resonances found.
    public var peaksFound: Int64
    /// Seconds of averaging.
    public var averagedS: Double
    /// The level of the last group of the transient band.
    public var levelDb: Double?
    /// Events in total.
    public var eventsTotal: Int64
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_schumann_fields) {
        text = sdrkitText(c.text)
        fundamentalHz = c.has_fundamental_hz ? c.fundamental_hz : nil
        fundamentalDb = c.has_fundamental_db ? c.fundamental_db : nil
        fundamentalQ = c.has_fundamental_q ? c.fundamental_q : nil
        peaksFound = c.peaks_found
        averagedS = c.averaged_s
        levelDb = c.has_level_db ? c.level_db : nil
        eventsTotal = c.events_total
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SchumannFields, SDRKitCode> {
        var c = sdrkit_schumann_fields()
        let code = sdrkit_schumann_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SchumannFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A resonance. The row key is its number, 1 to 5.
public struct SchumannPeakRow {
    /// The row key.
    public var key: String
    /// The resonance of a row of the peak table: 1 to 5.
    public var resonance: Int64
    /// The frequency of a resonance, 0 when none was found.
    public var frequencyHz: Double
    /// The height of a resonance over the baseline.
    public var heightDb: Double
    /// The Q of a resonance.
    public var q: Double

    init(_ c: sdrkit_schumann_peak_row) {
        key = sdrkitText(c.key)
        resonance = c.resonance
        frequencyHz = c.frequency_hz
        heightDb = c.height_db
        q = c.q
    }
}

/// An event of a monitor (`schumann`, `whistler`, `jove`, `callisto`, `eme_echo`, `beacon_events`). The row key is the sequence number of the event.
public struct ScienceEventRow {
    /// The row key.
    public var key: String
    /// The UTC of an event.
    public var utc: Double
    /// The signal time of an event.
    public var signalTimeS: Double
    /// The type of an event.
    public var type: String
    /// The characteristic frequency of an event.
    public var frequencyHz: Double
    /// The strength of an event over the background.
    public var peakDb: Double
    /// The duration of an event.
    public var durationS: Double
    /// Stack-specific value 1.
    public var aux1: Double
    /// Stack-specific value 2.
    public var aux2: Double
    /// The note of an event.
    public var eventNote: String

    init(_ c: sdrkit_science_event_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        signalTimeS = c.signal_time_s
        type = sdrkitText(c.type)
        frequencyHz = c.frequency_hz
        peakDb = c.peak_db
        durationS = c.duration_s
        aux1 = c.aux1
        aux2 = c.aux2
        eventNote = sdrkitText(c.event_note)
    }
}

/// A trend minute of `schumann`. The row key is the minute of the capture.
public struct SchumannTrendRow {
    /// The row key.
    public var key: String
    /// The UTC of the end of the minute.
    public var utc: Double
    /// The fundamental, Hz.
    public var fundamentalHz: Double
    /// The height of a resonance over the baseline.
    public var heightDb: Double

    init(_ c: sdrkit_schumann_trend_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        fundamentalHz = c.fundamental_hz
        heightDb = c.height_db
    }
}

/// The result fields of `whistler`.
public struct WhistlerFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// Whistlers found.
    public var whistlers: Int64
    /// Tweeks found.
    public var tweeks: Int64
    /// Events in total.
    public var eventsTotal: Int64
    /// Sferics counted.
    public var sferics: Int64
    /// The dispersion of the last whistler.
    public var dispersion: Double?
    /// The broadband value of the newest row.
    public var broadbandDb: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_whistler_fields) {
        text = sdrkitText(c.text)
        whistlers = c.whistlers
        tweeks = c.tweeks
        eventsTotal = c.events_total
        sferics = c.sferics
        dispersion = c.has_dispersion ? c.dispersion : nil
        broadbandDb = c.has_broadband_db ? c.broadband_db : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WhistlerFields, SDRKitCode> {
        var c = sdrkit_whistler_fields()
        let code = sdrkit_whistler_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WhistlerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A trend minute of `whistler`. The row key is the minute of the capture.
public struct WhistlerTrendRow {
    /// The row key.
    public var key: String
    /// The UTC of the end of the minute.
    public var utc: Double
    /// Events of a trend minute.
    public var eventsPerMin: Double
    /// Sferics of a trend minute.
    public var sfericsPerMin: Double

    init(_ c: sdrkit_whistler_trend_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        eventsPerMin = c.events_per_min
        sfericsPerMin = c.sferics_per_min
    }
}

/// The result fields of `lightning_tdoa`.
public struct LightningTdoaFields {
    /// The role in use.
    public var role: String
    /// Sferics timed.
    public var pulses: Int64
    /// The peak of the newest sferic.
    public var lastPeakDb: Double?
    /// The floor of the sferic timer.
    public var floorDb: Double?
    /// The best usable time source.
    public var timeSource: String
    /// Its 1-sigma now.
    public var clockSigmaUs: Double?
    /// The rate error of its sample clock.
    public var clockPpm: Double?
    /// The 1-sigma of the drift.
    public var clockPpmSigma: Double?
    /// A detection now would be fine.
    public var fine: Bool
    /// The 1-sigma of the GPS clock.
    public var sigmaGpsUs: Double?
    /// The 1-sigma of the GNSS clock.
    public var sigmaGnssUs: Double?
    /// The 1-sigma of the common-view clock.
    public var sigmaCvUs: Double?
    /// The 1-sigma of the PTP clock.
    public var sigmaPtpUs: Double?
    /// The 1-sigma of the NTP clock.
    public var sigmaNtpUs: Double?
    /// The 1-sigma of the unsynchronised computer clock.
    public var sigmaSystemUs: Double?
    /// The common-view clock stamps the detections.
    public var commonView: Bool
    /// The timing transmitter is locked.
    public var timingLocked: Bool?
    /// Its SNR.
    public var timingSnrDb: Double?
    /// The 1-sigma of its boundary.
    public var timingSigmaUs: Double?
    /// Its carrier error.
    public var timingCarrierHz: Double?
    /// The state of the network.
    public var network: String
    /// Datagrams sent.
    public var sent: Int64
    /// Datagrams that could not be sent.
    public var sendErrors: Int64
    /// Datagrams accepted by the server.
    public var packetsOk: Int64?
    /// Refused by the MAC.
    public var refusedMac: Int64?
    /// Refused as replays.
    public var replays: Int64?
    /// Refused as stale.
    public var stale: Int64?
    /// Refused as malformed.
    public var malformed: Int64?
    /// Refused by the rate limit.
    public var rateLimited: Int64?
    /// Groups of detections tried.
    public var groups: Int64?
    /// Fixes made.
    public var fixes: Int64?
    /// Fixes refused by the chi-square test.
    public var rejected: Int64?
    /// Groups without a fix.
    public var unmatched: Int64?
    /// Groups with too few fine stations.
    public var coarseOnly: Int64?
    /// Impulses too long for lightning.
    public var tooLong: Int64
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_lightning_tdoa_fields) {
        role = sdrkitText(c.role)
        pulses = c.pulses
        lastPeakDb = c.has_last_peak_db ? c.last_peak_db : nil
        floorDb = c.has_floor_db ? c.floor_db : nil
        timeSource = sdrkitText(c.time_source)
        clockSigmaUs = c.has_clock_sigma_us ? c.clock_sigma_us : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        clockPpmSigma = c.has_clock_ppm_sigma ? c.clock_ppm_sigma : nil
        fine = c.fine
        sigmaGpsUs = c.has_sigma_gps_us ? c.sigma_gps_us : nil
        sigmaGnssUs = c.has_sigma_gnss_us ? c.sigma_gnss_us : nil
        sigmaCvUs = c.has_sigma_cv_us ? c.sigma_cv_us : nil
        sigmaPtpUs = c.has_sigma_ptp_us ? c.sigma_ptp_us : nil
        sigmaNtpUs = c.has_sigma_ntp_us ? c.sigma_ntp_us : nil
        sigmaSystemUs = c.has_sigma_system_us ? c.sigma_system_us : nil
        commonView = c.common_view
        timingLocked = c.has_timing_locked ? c.timing_locked : nil
        timingSnrDb = c.has_timing_snr_db ? c.timing_snr_db : nil
        timingSigmaUs = c.has_timing_sigma_us ? c.timing_sigma_us : nil
        timingCarrierHz = c.has_timing_carrier_hz ? c.timing_carrier_hz : nil
        network = sdrkitText(c.network)
        sent = c.sent
        sendErrors = c.send_errors
        packetsOk = c.has_packets_ok ? c.packets_ok : nil
        refusedMac = c.has_refused_mac ? c.refused_mac : nil
        replays = c.has_replays ? c.replays : nil
        stale = c.has_stale ? c.stale : nil
        malformed = c.has_malformed ? c.malformed : nil
        rateLimited = c.has_rate_limited ? c.rate_limited : nil
        groups = c.has_groups ? c.groups : nil
        fixes = c.has_fixes ? c.fixes : nil
        rejected = c.has_rejected ? c.rejected : nil
        unmatched = c.has_unmatched ? c.unmatched : nil
        coarseOnly = c.has_coarse_only ? c.coarse_only : nil
        tooLong = c.too_long
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LightningTdoaFields, SDRKitCode> {
        var c = sdrkit_lightning_tdoa_fields()
        let code = sdrkit_lightning_tdoa_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LightningTdoaFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A located strike (server). The row key is the sequence number of the strike, eight digits.
public struct LightningTdoaStrikeRow {
    /// The row key.
    public var key: String
    /// The time of a strike, UTC ns.
    public var utcNs: Int64
    /// Latitude.
    public var lat: Double
    /// Longitude.
    public var lon: Double
    /// The semi-major axis of the 95 % ellipse.
    public var majorKm: Double
    /// The semi-minor axis.
    public var minorKm: Double
    /// The azimuth of the major axis.
    public var azDeg: Double
    /// Stations used.
    public var stations: Int64
    /// Coarse stations that heard it.
    public var alsoHeard: Int64
    /// The rms of the residuals.
    public var rmsUs: Double
    /// Chi-square.
    public var chi2: Double
    /// Degrees of freedom.
    public var dof: Int64
    /// No degree of freedom checks it.
    public var unchecked: Bool
    /// A second solution fits as well.
    public var ambiguous: Bool
    /// The time is UTC.
    public var utc: Bool

    init(_ c: sdrkit_lightning_tdoa_strike_row) {
        key = sdrkitText(c.key)
        utcNs = c.utc_ns
        lat = c.lat
        lon = c.lon
        majorKm = c.major_km
        minorKm = c.minor_km
        azDeg = c.az_deg
        stations = c.stations
        alsoHeard = c.also_heard
        rmsUs = c.rms_us
        chi2 = c.chi2
        dof = c.dof
        unchecked = c.unchecked
        ambiguous = c.ambiguous
        utc = c.utc
    }
}

/// A station of the network (server). The row key is the station id, eight hex digits.
public struct LightningTdoaStationRow {
    /// The row key.
    public var key: String
    /// The station id.
    public var id: String
    /// The station name.
    public var name: String
    /// Latitude.
    public var lat: Double
    /// Longitude.
    public var lon: Double
    /// The best usable time source.
    public var timeSource: String
    /// The 1-sigma of the station's clock.
    public var sigmaUs: Double
    /// The time since the station's last datagram.
    public var ageS: Double
    /// Detections received.
    public var detections: Int64
    /// Detections used in fixes.
    public var used: Int64
    /// A detection now would be fine.
    public var fine: Bool
    /// The station's clock is UTC.
    public var tiedToUtc: Bool
    /// The common-view correction.
    public var cvCorrectionUs: Double?

    init(_ c: sdrkit_lightning_tdoa_station_row) {
        key = sdrkitText(c.key)
        id = sdrkitText(c.id)
        name = sdrkitText(c.name)
        lat = c.lat
        lon = c.lon
        timeSource = sdrkitText(c.time_source)
        sigmaUs = c.sigma_us
        ageS = c.age_s
        detections = c.detections
        used = c.used
        fine = c.fine
        tiedToUtc = c.tied_to_utc
        cvCorrectionUs = c.has_cv_correction_us ? c.cv_correction_us : nil
    }
}

/// The result fields of the burst monitors `jove` and `callisto`.
public struct ScienceBurstFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// Bursts found.
    public var bursts: Int64
    /// Events in total.
    public var eventsTotal: Int64
    /// The level of the last row.
    public var levelDb: Double?
    /// The width of a bin.
    public var binHz: Double?
    /// The time of a row.
    public var rowS: Double?
    /// A burst is open.
    public var burstOpen: Bool
    /// Files written.
    public var files: Int64
    /// The last file written, empty before the first.
    public var lastFile: String
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_science_burst_fields) {
        text = sdrkitText(c.text)
        bursts = c.bursts
        eventsTotal = c.events_total
        levelDb = c.has_level_db ? c.level_db : nil
        binHz = c.has_bin_hz ? c.bin_hz : nil
        rowS = c.has_row_s ? c.row_s : nil
        burstOpen = c.burst_open
        files = c.files
        lastFile = sdrkitText(c.last_file)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ScienceBurstFields, SDRKitCode> {
        var c = sdrkit_science_burst_fields()
        let code = sdrkit_science_burst_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ScienceBurstFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A trend minute of the burst monitors (`jove`, `callisto`). The row key is the minute of the capture.
public struct ScienceBurstTrendRow {
    /// The row key.
    public var key: String
    /// The UTC of the end of the minute.
    public var utc: Double
    /// The level of a trend minute.
    public var levelChangeDb: Double
    /// Bursts of a trend minute.
    public var burstsPerMin: Double

    init(_ c: sdrkit_science_burst_trend_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        levelChangeDb = c.level_change_db
        burstsPerMin = c.bursts_per_min
    }
}

/// The result fields of `riometer`.
public struct RiometerFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// The power of the last minute, dB of full scale.
    public var powerDb: Double?
    /// The local sidereal time.
    public var lstH: Double?
    /// The curve at that time.
    public var curveDb: Double?
    /// The absorption.
    public var absorptionDb: Double?
    /// Bins of the curve with a value.
    public var curveBins: Int64
    /// The curve is ready.
    public var curveReady: Bool
    /// The interfered share.
    public var interferedShare: Double
    /// An event is open.
    public var eventOn: Bool
    /// Minutes measured.
    public var minutes: Int64
    /// The curve as text, as of the last minute measured or the last change of the curve.
    public var qdc: String?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_riometer_fields) {
        text = sdrkitText(c.text)
        powerDb = c.has_power_db ? c.power_db : nil
        lstH = c.has_lst_h ? c.lst_h : nil
        curveDb = c.has_curve_db ? c.curve_db : nil
        absorptionDb = c.has_absorption_db ? c.absorption_db : nil
        curveBins = c.curve_bins
        curveReady = c.curve_ready
        interferedShare = c.interfered_share
        eventOn = c.event_on
        minutes = c.minutes
        qdc = c.has_qdc ? sdrkitText(c.qdc) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RiometerFields, SDRKitCode> {
        var c = sdrkit_riometer_fields()
        let code = sdrkit_riometer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RiometerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The reply of `export_qdc`.
public struct RiometerQdcReply {
    /// The curve as text.
    public var qdc: String

    init(_ c: sdrkit_riometer_qdc_reply) {
        qdc = sdrkitText(c.qdc)
    }
}

/// The result fields of `eme_echo`.
public struct EmeEchoFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// Own pulses seen.
    public var pulses: Int64
    /// Echoes found.
    public var echoes: Int64
    /// The own carrier.
    public var carrierHz: Double?
    /// Transmitting now.
    public var transmitting: Bool
    /// The noise per bin.
    public var noiseDb: Double?
    /// The delay of the last echo.
    public var lastDelayS: Double?
    /// The Doppler shift of the last echo.
    public var lastDopplerHz: Double?
    /// The level of the last echo.
    public var lastLevelDb: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_eme_echo_fields) {
        text = sdrkitText(c.text)
        pulses = c.pulses
        echoes = c.echoes
        carrierHz = c.has_carrier_hz ? c.carrier_hz : nil
        transmitting = c.transmitting
        noiseDb = c.has_noise_db ? c.noise_db : nil
        lastDelayS = c.has_last_delay_s ? c.last_delay_s : nil
        lastDopplerHz = c.has_last_doppler_hz ? c.last_doppler_hz : nil
        lastLevelDb = c.has_last_level_db ? c.last_level_db : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EmeEchoFields, SDRKitCode> {
        var c = sdrkit_eme_echo_fields()
        let code = sdrkit_eme_echo_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EmeEchoFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A trend minute of `eme_echo`. The row key is the minute of the capture.
public struct EmeEchoTrendRow {
    /// The row key.
    public var key: String
    /// The UTC of the end of the minute.
    public var utc: Double
    /// The largest echo level of a trend minute.
    public var echoLevelDb: Double
    /// The mean delay of a trend minute.
    public var echoDelayS: Double

    init(_ c: sdrkit_eme_echo_trend_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        echoLevelDb = c.echo_level_db
        echoDelayS = c.echo_delay_s
    }
}

/// The result fields of `beacon_events`.
public struct BeaconEventsFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// Openings found.
    public var openings: Int64
    /// The noise per bin.
    public var noiseDb: Double?
    /// An opening is open.
    public var `open`: Bool
    /// The width of the last row.
    public var widthHz: Double?
    /// The best z of the last row.
    public var z: Double?
    /// The S/N of the last row.
    public var snrDb: Double?
    /// The shift of the last row.
    public var shiftHz: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_beacon_events_fields) {
        text = sdrkitText(c.text)
        openings = c.openings
        noiseDb = c.has_noise_db ? c.noise_db : nil
        `open` = c.open
        widthHz = c.has_width_hz ? c.width_hz : nil
        z = c.has_z ? c.z : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        shiftHz = c.has_shift_hz ? c.shift_hz : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BeaconEventsFields, SDRKitCode> {
        var c = sdrkit_beacon_events_fields()
        let code = sdrkit_beacon_events_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BeaconEventsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A trend minute of `beacon_events`. The row key is the minute of the capture.
public struct BeaconEventsTrendRow {
    /// The row key.
    public var key: String
    /// The UTC of the end of the minute.
    public var utc: Double
    /// The peak S/N of the hit rows of a trend minute.
    public var peakSnrDb: Double
    /// The mean width of the hit rows of a trend minute.
    public var meanWidthHz: Double

    init(_ c: sdrkit_beacon_events_trend_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        peakSnrDb = c.peak_snr_db
        meanWidthHz = c.mean_width_hz
    }
}

/// The result fields of `pulsar`.
public struct PulsarFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// Whether a detection was made.
    public var ok: Bool
    /// The best S/N.
    public var snr: Double?
    /// The best period.
    public var periodFoundS: Double?
    /// The best DM.
    public var dmFound: Double?
    /// The pulse phase.
    public var peakPhase: Double?
    /// The duty cycle.
    public var duty: Double?
    /// The time observed.
    public var observedS: Double
    /// A search runs.
    public var searching: Bool
    /// Searches made.
    public var searches: Int64
    /// Detections made.
    public var detections: Int64
    /// Rows held.
    public var rowsHeld: Int64
    /// The time of a row.
    public var rowMs: Double?
    /// Channels of the filter bank.
    public var bankChannels: Int64?
    /// The area needed.
    public var areaM2: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_pulsar_fields) {
        text = sdrkitText(c.text)
        ok = c.ok
        snr = c.has_snr ? c.snr : nil
        periodFoundS = c.has_period_found_s ? c.period_found_s : nil
        dmFound = c.has_dm_found ? c.dm_found : nil
        peakPhase = c.has_peak_phase ? c.peak_phase : nil
        duty = c.has_duty ? c.duty : nil
        observedS = c.observed_s
        searching = c.searching
        searches = c.searches
        detections = c.detections
        rowsHeld = c.rows_held
        rowMs = c.has_row_ms ? c.row_ms : nil
        bankChannels = c.has_bank_channels ? c.bank_channels : nil
        areaM2 = c.has_area_m2 ? c.area_m2 : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PulsarFields, SDRKitCode> {
        var c = sdrkit_pulsar_fields()
        let code = sdrkit_pulsar_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PulsarFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A search of `pulsar`. The row key is the sequence number of the search.
public struct PulsarSearchRow {
    /// The row key.
    public var key: String
    /// The time observed.
    public var observedS: Double
    /// Whether a detection was made.
    public var ok: Bool
    /// The best S/N.
    public var snr: Double?
    /// The best period.
    public var periodFoundS: Double
    /// The best DM.
    public var dmFound: Double
    /// The pulse phase.
    public var peakPhase: Double
    /// The duty cycle.
    public var duty: Double

    init(_ c: sdrkit_pulsar_search_row) {
        key = sdrkitText(c.key)
        observedS = c.observed_s
        ok = c.ok
        snr = c.has_snr ? c.snr : nil
        periodFoundS = c.period_found_s
        dmFound = c.dm_found
        peakPhase = c.peak_phase
        duty = c.duty
    }
}

/// The result fields of `radar_detect`.
public struct RadarDetectFields {
    /// The text of a decoded message: the summary of the stack.
    public var text: String
    /// The rotation period.
    public var rotationS: Double?
    /// Rotations per minute.
    public var rpm: Double?
    /// The repetition frequency of the fullest bin.
    public var prfHz: Double?
    /// The repetition text.
    public var prf: String?
    /// The median pulse width.
    public var pulseWidthUs: Double?
    /// The beamwidth.
    public var beamwidthDeg: Double?
    /// Crossings found.
    public var crossings: Int64
    /// Pulses found.
    public var pulses: Int64
    /// Rows written.
    public var rowsTotal: Int64
    /// The noise power.
    public var noiseDbfs: Double?
    /// Weather: sweeps in this volume.
    public var sweeps: Int64?
    /// Weather: volume starts seen.
    public var volumes: Int64?
    /// Weather: the length of the last whole volume.
    public var lastVolumeS: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_radar_detect_fields) {
        text = sdrkitText(c.text)
        rotationS = c.has_rotation_s ? c.rotation_s : nil
        rpm = c.has_rpm ? c.rpm : nil
        prfHz = c.has_prf_hz ? c.prf_hz : nil
        prf = c.has_prf ? sdrkitText(c.prf) : nil
        pulseWidthUs = c.has_pulse_width_us ? c.pulse_width_us : nil
        beamwidthDeg = c.has_beamwidth_deg ? c.beamwidth_deg : nil
        crossings = c.crossings
        pulses = c.pulses
        rowsTotal = c.rows_total
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        sweeps = c.has_sweeps ? c.sweeps : nil
        volumes = c.has_volumes ? c.volumes : nil
        lastVolumeS = c.has_last_volume_s ? c.last_volume_s : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RadarDetectFields, SDRKitCode> {
        var c = sdrkit_radar_detect_fields()
        let code = sdrkit_radar_detect_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RadarDetectFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of `radar_detect`: a rotation (marine) or a sweep (weather). The row key is the sequence number of the row.
public struct RadarDetectRow {
    /// The row key.
    public var key: String
    /// The signal time of a row.
    public var signalTimeS: Double
    /// The kind of a row: `rotation` or `sweep`.
    public var type: String
    /// The rotation period.
    public var rotationS: Double?
    /// Rotations per minute.
    public var rpm: Double?
    /// The repetition text.
    public var prf: String
    /// The median pulse width.
    public var pulseWidthUs: Double?
    /// The beamwidth.
    public var beamwidthDeg: Double?
    /// The peak of the crossing of a row.
    public var peakDb: Double
    /// The volume of a weather row.
    public var volume: Int64?
    /// The sweep of a weather row in its volume.
    public var sweep: Int64?

    init(_ c: sdrkit_radar_detect_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        type = sdrkitText(c.type)
        rotationS = c.has_rotation_s ? c.rotation_s : nil
        rpm = c.has_rpm ? c.rpm : nil
        prf = sdrkitText(c.prf)
        pulseWidthUs = c.has_pulse_width_us ? c.pulse_width_us : nil
        beamwidthDeg = c.has_beamwidth_deg ? c.beamwidth_deg : nil
        peakDb = c.peak_db
        volume = c.has_volume ? c.volume : nil
        sweep = c.has_sweep ? c.sweep : nil
    }
}

/// The present state of the channels of `horus`. The channel bank writes it; a change of frequency clears it.
public struct HorusFields {
    public enum State: Int32 {
        case waiting = 0
        case listening = 1
        case decoding = 2
    }
    /// Waiting for tones, listening to tones, or decoding (a packet within the last 30 s).
    public var state: State?
    /// Channels running, the dial channel included.
    public var channels: Int64
    /// Unique words found.
    public var syncs: Int64
    /// Frames that failed their CRC.
    public var failed: Int64
    /// The best SNR in 3 kHz of a packet of the last 60 s.
    public var snrDb: Double?

    init(_ c: sdrkit_horus_fields) {
        state = State(rawValue: c.state)
        channels = c.channels
        syncs = c.syncs
        failed = c.failed
        snrDb = c.has_snr_db ? c.snr_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HorusFields, SDRKitCode> {
        var c = sdrkit_horus_fields()
        let code = sdrkit_horus_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HorusFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// What `horus` received: the packet log and the payloads. It stays over a change of frequency.
public struct HorusLogFields {
    /// Packets decoded, duplicates left out.
    public var packets: Int64
    /// Payloads heard within the last 120 s.
    public var payloadsActive: Int64
    /// Lines of the setting `payload_ids` that could not be read.
    public var unreadableIds: Int64

    init(_ c: sdrkit_horus_log_fields) {
        packets = c.packets
        payloadsActive = c.payloads_active
        unreadableIds = c.unreadable_ids
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HorusLogFields, SDRKitCode> {
        var c = sdrkit_horus_log_fields()
        let code = sdrkit_horus_log_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HorusLogFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the table `payloads`. The row key is the payload: its callsign, or "ID n" for an ID that the list does not name.
public struct HorusPayloadRow {
    public enum TypeOption: Int32 {
        case v1 = 0
        case v2 = 1
        case rtty = 2
    }
    /// The row key.
    public var key: String
    /// The callsign, or "ID n".
    public var payload: String
    /// The kind of the last packet.
    public var type: TypeOption?
    /// The payload ID of a binary packet.
    public var payloadId: Int64?
    /// The sequence number of the last packet.
    public var seq: Int64
    /// The time of the last packet: its time of day on the receive date, UTC.
    public var time: Double
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The height of the object that the event is about, metres above mean sea level.
    public var heightM: Double
    /// The highest altitude so far.
    public var maxAltM: Double
    /// The climb rate over the track of the last 30 to 180 s.
    public var climbMS: Double?
    /// The speed of a binary packet.
    public var speedKmh: Double?
    /// The satellites of a binary packet.
    public var sats: Int64?
    /// The temperature of a binary packet.
    public var tempC: Double?
    /// The battery of a binary packet.
    public var batteryV: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// Packets of the payload.
    public var packets: Int64
    /// The middle of the tones of the last packet, from the dial.
    public var offset: Double
    /// The frequency of the middle of the tones.
    public var frequency: Double?
    /// The symbol rate of the last packet.
    public var baud: Double
    /// The tone spacing (4FSK) or shift (RTTY) of the last packet.
    public var spacing: Double
    /// The bits that the Golay code corrected in the last binary packet.
    public var corrections: Int64?
    /// The data bits of a character of the last RTTY packet.
    public var rttyBits: Int64?
    /// Points of the track.
    public var trackPoints: Int64
    /// The last packet as a sentence.
    public var sentence: String
    /// Wall-clock time when first heard, in whole seconds.
    public var firstHeard: Double?
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_horus_payload_row) {
        key = sdrkitText(c.key)
        payload = sdrkitText(c.payload)
        type = TypeOption(rawValue: c.type)
        payloadId = c.has_payload_id ? c.payload_id : nil
        seq = c.seq
        time = c.time
        position = c.has_position ? SDRKitPosition(c.position) : nil
        heightM = c.height_m
        maxAltM = c.max_alt_m
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        speedKmh = c.has_speed_kmh ? c.speed_kmh : nil
        sats = c.has_sats ? c.sats : nil
        tempC = c.has_temp_c ? c.temp_c : nil
        batteryV = c.has_battery_v ? c.battery_v : nil
        snrDb = c.snr_db
        packets = c.packets
        offset = c.offset
        frequency = c.has_frequency ? c.frequency : nil
        baud = c.baud
        spacing = c.spacing
        corrections = c.has_corrections ? c.corrections : nil
        rttyBits = c.has_rtty_bits ? c.rtty_bits : nil
        trackPoints = c.track_points
        sentence = sdrkitText(c.sentence)
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A custom field of the last v2 packet of a payload, or a field after the altitude of its last RTTY sentence. The row key is the payload, "/" and the place of the field from 1.
public struct HorusCustomFieldRow {
    /// The row key.
    public var key: String
    /// The payload.
    public var payload: String
    /// The name of the field.
    public var name: String
    /// The value, written by its post-processing.
    public var value: String

    init(_ c: sdrkit_horus_custom_field_row) {
        key = sdrkitText(c.key)
        payload = sdrkitText(c.payload)
        name = sdrkitText(c.name)
        value = sdrkitText(c.value)
    }
}

/// A row of the packet log. The row key is the number of the packet, six digits.
public struct HorusPacketRow {
    public enum TypeOption: Int32 {
        case v1 = 0
        case v2 = 1
        case rtty = 2
    }
    /// The row key.
    public var key: String
    /// When the packet was received.
    public var received: Double
    /// The payload.
    public var payload: String
    /// The kind of the packet.
    public var type: TypeOption?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The middle of the tones, from the dial.
    public var offset: Double
    /// The channel that received it (0 is the dial).
    public var channel: Int64
    /// The packet as a sentence.
    public var sentence: String

    init(_ c: sdrkit_horus_packet_row) {
        key = sdrkitText(c.key)
        received = c.received
        payload = sdrkitText(c.payload)
        type = TypeOption(rawValue: c.type)
        snrDb = c.snr_db
        offset = c.offset
        channel = c.channel
        sentence = sdrkitText(c.sentence)
    }
}

/// A row of the table `channels`. The row key is the channel number (0 is the dial channel).
public struct HorusChannelRow {
    public enum Decoding: Int32 {
        case listening = 0
        case binary = 1
        case rtty = 2
    }
    /// The row key.
    public var key: String
    /// The centre of the channel, from the dial.
    public var offset: Double
    /// What the channel decodes.
    public var decoding: Decoding?
    /// The lowest 4FSK tone, from the channel centre.
    public var tone1: Double?
    /// The second 4FSK tone.
    public var tone2: Double?
    /// The third 4FSK tone.
    public var tone3: Double?
    /// The highest 4FSK tone.
    public var tone4: Double?
    /// The lower RTTY tone, from the channel centre.
    public var rttyLow: Double?
    /// The higher RTTY tone.
    public var rttyHigh: Double?
    /// The 4FSK tone spacing.
    public var spacing: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Packets decoded.
    public var packets: Int64
    /// Unique words found.
    public var syncs: Int64
    /// Frames that failed their CRC.
    public var failed: Int64
    /// The span search opened the channel.
    public var searched: Bool

    init(_ c: sdrkit_horus_channel_row) {
        key = sdrkitText(c.key)
        offset = c.offset
        decoding = Decoding(rawValue: c.decoding)
        tone1 = c.has_tone_1 ? c.tone_1 : nil
        tone2 = c.has_tone_2 ? c.tone_2 : nil
        tone3 = c.has_tone_3 ? c.tone_3 : nil
        tone4 = c.has_tone_4 ? c.tone_4 : nil
        rttyLow = c.has_rtty_low ? c.rtty_low : nil
        rttyHigh = c.has_rtty_high ? c.rtty_high : nil
        spacing = c.has_spacing ? c.spacing : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        packets = c.packets
        syncs = c.syncs
        failed = c.failed
        searched = c.searched
    }
}

/// A sonde frame, every 5 s for each sonde. `source` is the serial, `text` the sonde for people.
public struct SondeFrameEvent {
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
    public enum TypeOption: Int32 {
        case rs41 = 0
        case rs92 = 1
        case dfm = 2
        case m10 = 3
        case m20 = 4
        case lms6 = 5
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The height of the object that the event is about, metres above mean sea level.
    public var heightM: Double?
    /// The speed over the ground of the object that the event is about, knots.
    public var speedKt: Double?
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    public var courseDeg: Double?
    /// The family.
    public var type: TypeOption?
    /// The serial.
    public var serial: String
    /// The temperature.
    public var tempC: Double?
    /// The relative humidity.
    public var humidityPct: Double?
    /// The vertical speed.
    public var climbMS: Double?
    /// The sonde's burst or kill timer runs.
    public var burstKiller: Bool?

    init(_ c: sdrkit_sonde_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        heightM = c.has_height_m ? c.height_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        type = TypeOption(rawValue: c.type)
        serial = sdrkitText(c.serial)
        tempC = c.has_temp_c ? c.temp_c : nil
        humidityPct = c.has_humidity_pct ? c.humidity_pct : nil
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        burstKiller = c.has_burst_killer ? c.burst_killer : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SondeFrameEvent, SDRKitCode> {
        var c = sdrkit_sonde_frame_event()
        let code = sdrkit_sonde_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SondeFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A minute decoded: `text` holds the time as the station sends it, `utc` the start of the minute.
public struct TimeMinuteEvent {
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
    public enum Station: Int32 {
        case dcf77 = 0
        case msf = 1
        case wwvb = 2
        case jjy40 = 3
        case jjy60 = 4
        case bpc = 5
        case wwv = 6
        case chu = 7
        case bpm = 8
        case bpmc = 9
        case hla = 10
        case rwm = 11
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The received UTC now.
    public var utc: Double
    /// The station decoded.
    public var station: Station?

    init(_ c: sdrkit_time_minute_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        utc = c.utc
        station = Station(rawValue: c.station)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TimeMinuteEvent, SDRKitCode> {
        var c = sdrkit_time_minute_event()
        let code = sdrkit_time_minute_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TimeMinuteEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A time packet received: `text` holds the time for people, `utc` the time.
public struct EczasTimeEvent {
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
    public enum TxState: Int32 {
        case normal = 0
        case offDay = 1
        case offWeek = 2
        case offLonger = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a frame.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The received UTC now.
    public var utc: Double
    /// The hours that the packet adds to UTC for the local time in Poland.
    public var tzHours: Int64
    /// The transmitter state.
    public var txState: TxState?

    init(_ c: sdrkit_eczas_time_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        utc = c.utc
        tzHours = c.tz_hours
        txState = TxState(rawValue: c.tx_state)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EczasTimeEvent, SDRKitCode> {
        var c = sdrkit_eczas_time_event()
        let code = sdrkit_eczas_time_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EczasTimeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A SAME alert. `source` is the sender's call sign, `text` the event and the places, `position` the first place on the map.
public struct SameAlertEvent {
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
    public enum EventKind: Int32 {
        case warning = 0
        case watch = 1
        case emergency = 2
        case statement = 3
        case test = 4
        case message = 5
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The event code.
    public var event: String
    /// The event name.
    public var eventName: String?
    /// The kind of event (warning, watch ...).
    public var eventKind: EventKind?
    /// The originator code.
    public var originator: String
    /// How long the message is valid.
    public var validFor: Double
    /// The sender's call sign.
    public var callsign: String
    /// The number of locations.
    public var locations: Int64
    /// The raw text.
    public var raw: String

    init(_ c: sdrkit_same_alert_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        urgency = Urgency(rawValue: c.urgency)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        event = sdrkitText(c.event)
        eventName = c.has_event_name ? sdrkitText(c.event_name) : nil
        eventKind = c.has_event_kind ? EventKind(rawValue: c.event_kind) : nil
        originator = sdrkitText(c.originator)
        validFor = c.valid_for
        callsign = sdrkitText(c.callsign)
        locations = c.locations
        raw = sdrkitText(c.raw)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SameAlertEvent, SDRKitCode> {
        var c = sdrkit_same_alert_event()
        let code = sdrkit_same_alert_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SameAlertEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A SAME end of message.
public struct SameEndEvent {
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
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The raw text.
    public var raw: String

    init(_ c: sdrkit_same_end_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        raw = sdrkitText(c.raw)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SameEndEvent, SDRKitCode> {
        var c = sdrkit_same_end_event()
        let code = sdrkit_same_end_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SameEndEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A meteor ping. `text` holds the ping for people.
public struct MscatterMsPingEvent {
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
        case underdense = 0
        case overdense = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message: the ping for people.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// UTC of a ping.
    public var utc: Double
    /// The length of a ping.
    public var duration: Double
    /// The peak level over the background.
    public var peakSnrDb: Double
    /// The Doppler shift at the peak.
    public var doppler: Double
    /// Underdense or overdense.
    public var kind: Kind?

    init(_ c: sdrkit_mscatter_ms_ping_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        utc = c.utc
        duration = c.duration
        peakSnrDb = c.peak_snr_db
        doppler = c.doppler
        kind = Kind(rawValue: c.kind)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MscatterMsPingEvent, SDRKitCode> {
        var c = sdrkit_mscatter_ms_ping_event()
        let code = sdrkit_mscatter_ms_ping_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MscatterMsPingEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The storm phase changed to approaching, overhead or receding.
public struct SfericsStormEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The storm phase.
    public var stormPhase: String
    /// The change of the median peak.
    public var stormDbChange: Double
    /// Strikes per minute in the last 5 minutes.
    public var perMinute: Double

    init(_ c: sdrkit_sferics_storm_event) {
        time = c.time
        freqHz = c.freq_hz
        stormPhase = sdrkitText(c.storm_phase)
        stormDbChange = c.storm_db_change
        perMinute = c.per_minute
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SfericsStormEvent, SDRKitCode> {
        var c = sdrkit_sferics_storm_event()
        let code = sdrkit_sferics_storm_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SfericsStormEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An event `sid.event`.
public struct SupersidSidEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// A transmitter's label.
    public var label: String
    /// The UTC of an event's start.
    public var startUtc: Double
    /// The mean of the last 2 minutes less the baseline.
    public var riseDb: Double
    /// The peak step of an event.
    public var peakDb: Double
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_supersid_sid_event) {
        time = c.time
        freqHz = c.freq_hz
        label = sdrkitText(c.label)
        startUtc = c.start_utc
        riseDb = c.rise_db
        peakDb = c.peak_db
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SupersidSidEvent, SDRKitCode> {
        var c = sdrkit_supersid_sid_event()
        let code = sdrkit_supersid_sid_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SupersidSidEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An event of a monitor. `text` holds its note.
public struct ScienceEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The UTC of an event.
    public var utc: Double
    /// The type of an event.
    public var type: String
    /// The characteristic frequency of an event.
    public var frequencyHz: Double
    /// The strength of an event over the background.
    public var peakDb: Double
    /// The duration of an event.
    public var durationS: Double
    /// Stack-specific value 1.
    public var aux1: Double
    /// Stack-specific value 2.
    public var aux2: Double

    init(_ c: sdrkit_science_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        utc = c.utc
        type = sdrkitText(c.type)
        frequencyHz = c.frequency_hz
        peakDb = c.peak_db
        durationS = c.duration_s
        aux1 = c.aux1
        aux2 = c.aux2
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ScienceEvent, SDRKitCode> {
        var c = sdrkit_science_event()
        let code = sdrkit_science_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ScienceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A strike located (server).
public struct LightningTdoaLightningStrikeEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The time of a strike, UTC ns.
    public var utcNs: Int64
    /// Latitude.
    public var lat: Double
    /// Longitude.
    public var lon: Double
    /// The semi-major axis of the 95 % ellipse.
    public var majorKm: Double
    /// The semi-minor axis.
    public var minorKm: Double
    /// The azimuth of the major axis.
    public var azDeg: Double
    /// Stations used.
    public var stations: Int64
    /// Coarse stations that heard it.
    public var alsoHeard: Int64
    /// The rms of the residuals.
    public var rmsUs: Double
    /// Chi-square.
    public var chi2: Double
    /// Degrees of freedom.
    public var dof: Int64
    /// No degree of freedom checks it.
    public var unchecked: Bool
    /// A second solution fits as well.
    public var ambiguous: Bool
    /// The time is UTC.
    public var utc: Bool

    init(_ c: sdrkit_lightning_tdoa_lightning_strike_event) {
        time = c.time
        freqHz = c.freq_hz
        utcNs = c.utc_ns
        lat = c.lat
        lon = c.lon
        majorKm = c.major_km
        minorKm = c.minor_km
        azDeg = c.az_deg
        stations = c.stations
        alsoHeard = c.also_heard
        rmsUs = c.rms_us
        chi2 = c.chi2
        dof = c.dof
        unchecked = c.unchecked
        ambiguous = c.ambiguous
        utc = c.utc
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<LightningTdoaLightningStrikeEvent, SDRKitCode> {
        var c = sdrkit_lightning_tdoa_lightning_strike_event()
        let code = sdrkit_lightning_tdoa_lightning_strike_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(LightningTdoaLightningStrikeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The start (`phase` `start`, `depth_db` the absorption then) or the end (`end`, the largest absorption, the duration) of an absorption event.
public struct RiometerAbsorptionEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// Start or end of an event.
    public var phase: String
    /// The depth of an event.
    public var depthDb: Double
    /// The time from the start of an event.
    public var durationS: Double

    init(_ c: sdrkit_riometer_absorption_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        phase = sdrkitText(c.phase)
        depthDb = c.depth_db
        durationS = c.duration_s
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RiometerAbsorptionEvent, SDRKitCode> {
        var c = sdrkit_riometer_absorption_event()
        let code = sdrkit_riometer_absorption_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RiometerAbsorptionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A search that detected the pulsar.
public struct PulsarDetectionEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message: the summary of the detection.
    public var text: String
    /// Whether a detection was made.
    public var ok: Bool
    /// The best S/N.
    public var snr: Double
    /// The best period.
    public var periodFoundS: Double
    /// The best DM.
    public var dmFound: Double
    /// The pulse phase.
    public var peakPhase: Double
    /// The duty cycle.
    public var duty: Double
    /// The time observed.
    public var observedS: Double

    init(_ c: sdrkit_pulsar_detection_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        ok = c.ok
        snr = c.snr
        periodFoundS = c.period_found_s
        dmFound = c.dm_found
        peakPhase = c.peak_phase
        duty = c.duty
        observedS = c.observed_s
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PulsarDetectionEvent, SDRKitCode> {
        var c = sdrkit_pulsar_detection_event()
        let code = sdrkit_pulsar_detection_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PulsarDetectionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// One row of `radar_detect`. `text` holds a summary.
public struct RadarDetectRadarRowEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message: the summary of the row.
    public var text: String
    /// The kind of a row: `rotation` or `sweep`.
    public var type: String
    /// The rotation period.
    public var rotationS: Double?
    /// Rotations per minute.
    public var rpm: Double?
    /// The repetition text.
    public var prf: String
    /// The median pulse width.
    public var pulseWidthUs: Double?
    /// The beamwidth.
    public var beamwidthDeg: Double?
    /// The peak of the crossing of a row.
    public var peakDb: Double
    /// The volume of a weather row.
    public var volume: Int64?
    /// The sweep of a weather row in its volume.
    public var sweep: Int64?

    init(_ c: sdrkit_radar_detect_radar_row_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        type = sdrkitText(c.type)
        rotationS = c.has_rotation_s ? c.rotation_s : nil
        rpm = c.has_rpm ? c.rpm : nil
        prf = sdrkitText(c.prf)
        pulseWidthUs = c.has_pulse_width_us ? c.pulse_width_us : nil
        beamwidthDeg = c.has_beamwidth_deg ? c.beamwidth_deg : nil
        peakDb = c.peak_db
        volume = c.has_volume ? c.volume : nil
        sweep = c.has_sweep ? c.sweep : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RadarDetectRadarRowEvent, SDRKitCode> {
        var c = sdrkit_radar_detect_radar_row_event()
        let code = sdrkit_radar_detect_radar_row_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RadarDetectRadarRowEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A Horus packet. `source` is the payload, `text` the summary "Horus CALL #seq lat lon alt m".
public struct HorusPacketEvent {
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
    public enum TypeOption: Int32 {
        case v1 = 0
        case v2 = 1
        case rtty = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?
    /// The position of the object that the event is about.
    public var position: SDRKitPosition?
    /// The height of the object that the event is about, metres above mean sea level.
    public var heightM: Double?
    /// The kind of the packet.
    public var type: TypeOption?
    /// The sequence number.
    public var seq: Int64
    /// The climb rate.
    public var climbMS: Double?
    /// The speed of a binary packet.
    public var speedKmh: Double?
    /// The satellites of a binary packet.
    public var sats: Int64?
    /// The temperature of a binary packet.
    public var tempC: Double?
    /// The battery of a binary packet.
    public var batteryV: Double?
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double
    /// The middle of the tones, from the dial.
    public var offset: Double
    /// The channel that received it (0 is the dial).
    public var channel: Int64
    /// The packet as a sentence.
    public var sentence: String

    init(_ c: sdrkit_horus_packet_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        heightM = c.has_height_m ? c.height_m : nil
        type = TypeOption(rawValue: c.type)
        seq = c.seq
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        speedKmh = c.has_speed_kmh ? c.speed_kmh : nil
        sats = c.has_sats ? c.sats : nil
        tempC = c.has_temp_c ? c.temp_c : nil
        batteryV = c.has_battery_v ? c.battery_v : nil
        snrDb = c.snr_db
        offset = c.offset
        channel = c.channel
        sentence = sdrkitText(c.sentence)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HorusPacketEvent, SDRKitCode> {
        var c = sdrkit_horus_packet_event()
        let code = sdrkit_horus_packet_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HorusPacketEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The span search opened a channel on a signal. `text` is "a signal +x.x kHz from the dial".
public struct HorusChannelOpenedEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The channel number.
    public var channel: Int64
    /// The centre of the channel, from the dial.
    public var offset: Double

    init(_ c: sdrkit_horus_channel_opened_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        channel = c.channel
        offset = c.offset
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HorusChannelOpenedEvent, SDRKitCode> {
        var c = sdrkit_horus_channel_opened_event()
        let code = sdrkit_horus_channel_opened_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HorusChannelOpenedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A channel that the span search opened closed: it decoded nothing for 120 s and its signal was gone for 30 s.
public struct HorusChannelClosedEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of a decoded message.
    public var text: String
    /// The channel number.
    public var channel: Int64
    /// The centre of the channel, from the dial.
    public var offset: Double

    init(_ c: sdrkit_horus_channel_closed_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        channel = c.channel
        offset = c.offset
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HorusChannelClosedEvent, SDRKitCode> {
        var c = sdrkit_horus_channel_closed_event()
        let code = sdrkit_horus_channel_closed_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HorusChannelClosedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum ScienceTables {
    /// The changed rows of the table "sondes" of the stack "sonde", newest first, at most `capacity` rows.
    /// The column `serial` names a row.
    /// Place of a row: the column `position`.
    /// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func sondeSondes(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[SondeRow], SDRKitCode> {
        var rows = [sdrkit_sonde_row](repeating: sdrkit_sonde_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sonde_sondes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SondeRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "sonde", newest first, at most `capacity` rows.
    /// Default order: the column `level_db`, highest first.
    public static func sondeCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[SondeCarrierRow], SDRKitCode> {
        var rows = [sdrkit_sonde_carrier_row](repeating: sdrkit_sonde_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sonde_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SondeCarrierRow.init))
    }
    /// The changed rows of the table "minutes" of the stack "time", newest first, at most `capacity` rows.
    public static func timeMinutes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[TimeMinuteRow], SDRKitCode> {
        var rows = [sdrkit_time_minute_row](repeating: sdrkit_time_minute_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_time_minutes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(TimeMinuteRow.init))
    }
    /// The changed rows of the table "frames" of the stack "eczas", newest first, at most `capacity` rows.
    public static func eczasFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[EczasFrameRow], SDRKitCode> {
        var rows = [sdrkit_eczas_frame_row](repeating: sdrkit_eczas_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_eczas_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EczasFrameRow.init))
    }
    /// The changed rows of the table "alerts" of the stack "same", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func sameAlerts(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[SameAlertRow], SDRKitCode> {
        var rows = [sdrkit_same_alert_row](repeating: sdrkit_same_alert_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_same_alerts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SameAlertRow.init))
    }
    /// The changed rows of the table "alert_locations" of the stack "same", newest first, at most `capacity` rows.
    /// The column `place` names a row.
    /// Place of a row: the column `position`.
    public static func sameAlertLocations(_ handle: SDRKIT_HANDLE, capacity: Int = 3100) -> Result<[SameAlertLocationRow], SDRKitCode> {
        var rows = [sdrkit_same_alert_location_row](repeating: sdrkit_same_alert_location_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_same_alert_locations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SameAlertLocationRow.init))
    }
    /// The changed rows of the table "tones" of the stack "same", newest first, at most `capacity` rows.
    public static func sameTones(_ handle: SDRKIT_HANDLE, capacity: Int = 40) -> Result<[SameToneRow], SDRKitCode> {
        var rows = [sdrkit_same_tone_row](repeating: sdrkit_same_tone_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_same_tones_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SameToneRow.init))
    }
    /// The changed rows of the table "pings" of the stack "mscatter", newest first, at most `capacity` rows.
    public static func mscatterPings(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[MscatterPingRow], SDRKitCode> {
        var rows = [sdrkit_mscatter_ping_row](repeating: sdrkit_mscatter_ping_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mscatter_pings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MscatterPingRow.init))
    }
    /// The changed rows of the table "showers" of the stack "mscatter", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func mscatterShowers(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MscatterShowerRow], SDRKitCode> {
        var rows = [sdrkit_mscatter_shower_row](repeating: sdrkit_mscatter_shower_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mscatter_showers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MscatterShowerRow.init))
    }
    /// The changed rows of the table "packets" of the stack "ssdv", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    public static func ssdvPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[SsdvPacketRow], SDRKitCode> {
        var rows = [sdrkit_ssdv_packet_row](repeating: sdrkit_ssdv_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ssdv_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SsdvPacketRow.init))
    }
    /// The changed rows of the table "pictures" of the stack "ssdv", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    public static func ssdvPictures(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[SsdvPictureRow], SDRKitCode> {
        var rows = [sdrkit_ssdv_picture_row](repeating: sdrkit_ssdv_picture_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ssdv_pictures_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SsdvPictureRow.init))
    }
    /// The changed rows of the table "calibrated" of the stack "astro", newest first, at most `capacity` rows.
    /// Default order: the column `freq`, lowest first.
    public static func astroCalibrated(_ handle: SDRKIT_HANDLE, capacity: Int = 16384) -> Result<[AstroCalibratedRow], SDRKitCode> {
        var rows = [sdrkit_astro_calibrated_row](repeating: sdrkit_astro_calibrated_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_astro_calibrated_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AstroCalibratedRow.init))
    }
    /// The changed rows of the table "session" of the stack "astro", newest first, at most `capacity` rows.
    public static func astroSession(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AstroSessionRow], SDRKitCode> {
        var rows = [sdrkit_astro_session_row](repeating: sdrkit_astro_session_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_astro_session_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AstroSessionRow.init))
    }
    /// The changed rows of the table "fit" of the stack "astro", newest first, at most `capacity` rows.
    public static func astroFit(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[AstroFitRow], SDRKitCode> {
        var rows = [sdrkit_astro_fit_row](repeating: sdrkit_astro_fit_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_astro_fit_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AstroFitRow.init))
    }
    /// The changed rows of the table "lv_rows" of the stack "astro", newest first, at most `capacity` rows.
    /// Default order: the column `longitude`, lowest first.
    public static func astroLvRows(_ handle: SDRKIT_HANDLE, capacity: Int = 721) -> Result<[AstroMapRow], SDRKitCode> {
        var rows = [sdrkit_astro_map_row](repeating: sdrkit_astro_map_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_astro_lv_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AstroMapRow.init))
    }
    /// The changed rows of the table "sounders" of the stack "iono", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func ionoSounders(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[IonoSounderRow], SDRKitCode> {
        var rows = [sdrkit_iono_sounder_row](repeating: sdrkit_iono_sounder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_iono_sounders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IonoSounderRow.init))
    }
    /// The changed rows of the table "timeline" of the stack "iono", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func ionoTimeline(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[IonoTimelineRow], SDRKitCode> {
        var rows = [sdrkit_iono_timeline_row](repeating: sdrkit_iono_timeline_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_iono_timeline_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IonoTimelineRow.init))
    }
    /// The changed rows of the table "bands" of the stack "iono", newest first, at most `capacity` rows.
    /// The column `band` names a row.
    public static func ionoBands(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[IonoBandRow], SDRKitCode> {
        var rows = [sdrkit_iono_band_row](repeating: sdrkit_iono_band_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_iono_bands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IonoBandRow.init))
    }
    /// The changed rows of the table "strikes" of the stack "sferics", newest first, at most `capacity` rows.
    public static func sfericsStrikes(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[SfericsStrikeRow], SDRKitCode> {
        var rows = [sdrkit_sferics_strike_row](repeating: sdrkit_sferics_strike_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sferics_strikes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SfericsStrikeRow.init))
    }
    /// The changed rows of the table "transmitters" of the stack "supersid", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func supersidTransmitters(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[SupersidTransmitterRow], SDRKitCode> {
        var rows = [sdrkit_supersid_transmitter_row](repeating: sdrkit_supersid_transmitter_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_supersid_transmitters_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SupersidTransmitterRow.init))
    }
    /// The changed rows of the table "sid_events" of the stack "supersid", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func supersidSidEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[SupersidEventRow], SDRKitCode> {
        var rows = [sdrkit_supersid_event_row](repeating: sdrkit_supersid_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_supersid_sid_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SupersidEventRow.init))
    }
    /// The changed rows of the table "marks" of the stack "supersid", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func supersidMarks(_ handle: SDRKIT_HANDLE, capacity: Int = 384) -> Result<[SupersidMarkRow], SDRKitCode> {
        var rows = [sdrkit_supersid_mark_row](repeating: sdrkit_supersid_mark_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_supersid_marks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SupersidMarkRow.init))
    }
    /// The changed rows of the table "peaks" of the stack "schumann", newest first, at most `capacity` rows.
    /// The column `resonance` names a row.
    /// Default order: the column `resonance`, lowest first.
    public static func schumannPeaks(_ handle: SDRKIT_HANDLE, capacity: Int = 5) -> Result<[SchumannPeakRow], SDRKitCode> {
        var rows = [sdrkit_schumann_peak_row](repeating: sdrkit_schumann_peak_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_schumann_peaks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SchumannPeakRow.init))
    }
    /// The changed rows of the table "events" of the stack "schumann", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func schumannEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ScienceEventRow], SDRKitCode> {
        var rows = [sdrkit_science_event_row](repeating: sdrkit_science_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_schumann_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceEventRow.init))
    }
    /// The changed rows of the table "trend" of the stack "schumann", newest first, at most `capacity` rows.
    public static func schumannTrend(_ handle: SDRKIT_HANDLE, capacity: Int = 1440) -> Result<[SchumannTrendRow], SDRKitCode> {
        var rows = [sdrkit_schumann_trend_row](repeating: sdrkit_schumann_trend_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_schumann_trend_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SchumannTrendRow.init))
    }
    /// The changed rows of the table "events" of the stack "whistler", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func whistlerEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ScienceEventRow], SDRKitCode> {
        var rows = [sdrkit_science_event_row](repeating: sdrkit_science_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_whistler_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceEventRow.init))
    }
    /// The changed rows of the table "trend" of the stack "whistler", newest first, at most `capacity` rows.
    public static func whistlerTrend(_ handle: SDRKIT_HANDLE, capacity: Int = 1440) -> Result<[WhistlerTrendRow], SDRKitCode> {
        var rows = [sdrkit_whistler_trend_row](repeating: sdrkit_whistler_trend_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_whistler_trend_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WhistlerTrendRow.init))
    }
    /// The changed rows of the table "strikes" of the stack "lightning_tdoa", newest first, at most `capacity` rows.
    /// Place of a row: the columns `lat` and `lon`.
    public static func lightningTdoaStrikes(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[LightningTdoaStrikeRow], SDRKitCode> {
        var rows = [sdrkit_lightning_tdoa_strike_row](repeating: sdrkit_lightning_tdoa_strike_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lightning_tdoa_strikes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LightningTdoaStrikeRow.init))
    }
    /// The changed rows of the table "stations" of the stack "lightning_tdoa", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Place of a row: the columns `lat` and `lon`.
    public static func lightningTdoaStations(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[LightningTdoaStationRow], SDRKitCode> {
        var rows = [sdrkit_lightning_tdoa_station_row](repeating: sdrkit_lightning_tdoa_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lightning_tdoa_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LightningTdoaStationRow.init))
    }
    /// The changed rows of the table "events" of the stack "jove", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func joveEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ScienceEventRow], SDRKitCode> {
        var rows = [sdrkit_science_event_row](repeating: sdrkit_science_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jove_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceEventRow.init))
    }
    /// The changed rows of the table "trend" of the stack "jove", newest first, at most `capacity` rows.
    public static func joveTrend(_ handle: SDRKIT_HANDLE, capacity: Int = 1440) -> Result<[ScienceBurstTrendRow], SDRKitCode> {
        var rows = [sdrkit_science_burst_trend_row](repeating: sdrkit_science_burst_trend_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_jove_trend_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceBurstTrendRow.init))
    }
    /// The changed rows of the table "events" of the stack "callisto", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func callistoEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ScienceEventRow], SDRKitCode> {
        var rows = [sdrkit_science_event_row](repeating: sdrkit_science_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_callisto_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceEventRow.init))
    }
    /// The changed rows of the table "trend" of the stack "callisto", newest first, at most `capacity` rows.
    public static func callistoTrend(_ handle: SDRKIT_HANDLE, capacity: Int = 1440) -> Result<[ScienceBurstTrendRow], SDRKitCode> {
        var rows = [sdrkit_science_burst_trend_row](repeating: sdrkit_science_burst_trend_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_callisto_trend_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceBurstTrendRow.init))
    }
    /// The changed rows of the table "events" of the stack "eme_echo", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func emeEchoEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ScienceEventRow], SDRKitCode> {
        var rows = [sdrkit_science_event_row](repeating: sdrkit_science_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_eme_echo_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceEventRow.init))
    }
    /// The changed rows of the table "trend" of the stack "eme_echo", newest first, at most `capacity` rows.
    public static func emeEchoTrend(_ handle: SDRKIT_HANDLE, capacity: Int = 1440) -> Result<[EmeEchoTrendRow], SDRKitCode> {
        var rows = [sdrkit_eme_echo_trend_row](repeating: sdrkit_eme_echo_trend_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_eme_echo_trend_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EmeEchoTrendRow.init))
    }
    /// The changed rows of the table "events" of the stack "beacon_events", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func beaconEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ScienceEventRow], SDRKitCode> {
        var rows = [sdrkit_science_event_row](repeating: sdrkit_science_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_beacon_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScienceEventRow.init))
    }
    /// The changed rows of the table "trend" of the stack "beacon_events", newest first, at most `capacity` rows.
    public static func beaconEventsTrend(_ handle: SDRKIT_HANDLE, capacity: Int = 1440) -> Result<[BeaconEventsTrendRow], SDRKitCode> {
        var rows = [sdrkit_beacon_events_trend_row](repeating: sdrkit_beacon_events_trend_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_beacon_events_trend_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BeaconEventsTrendRow.init))
    }
    /// The changed rows of the table "searches" of the stack "pulsar", newest first, at most `capacity` rows.
    public static func pulsarSearches(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[PulsarSearchRow], SDRKitCode> {
        var rows = [sdrkit_pulsar_search_row](repeating: sdrkit_pulsar_search_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pulsar_searches_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PulsarSearchRow.init))
    }
    /// The changed rows of the table "radar_rows" of the stack "radar_detect", newest first, at most `capacity` rows.
    public static func radarDetectRadarRows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[RadarDetectRow], SDRKitCode> {
        var rows = [sdrkit_radar_detect_row](repeating: sdrkit_radar_detect_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_radar_detect_radar_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RadarDetectRow.init))
    }
    /// The changed rows of the table "payloads" of the stack "horus", newest first, at most `capacity` rows.
    /// The column `payload` names a row.
    /// Place of a row: the column `position`.
    /// Height: `height_m`. Speed: `speed_kmh`.
    public static func horusPayloads(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[HorusPayloadRow], SDRKitCode> {
        var rows = [sdrkit_horus_payload_row](repeating: sdrkit_horus_payload_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_horus_payloads_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HorusPayloadRow.init))
    }
    /// The changed rows of the table "custom_fields" of the stack "horus", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func horusCustomFields(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[HorusCustomFieldRow], SDRKitCode> {
        var rows = [sdrkit_horus_custom_field_row](repeating: sdrkit_horus_custom_field_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_horus_custom_fields_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HorusCustomFieldRow.init))
    }
    /// The changed rows of the table "packets" of the stack "horus", newest first, at most `capacity` rows.
    /// The column `payload` names a row.
    public static func horusPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[HorusPacketRow], SDRKitCode> {
        var rows = [sdrkit_horus_packet_row](repeating: sdrkit_horus_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_horus_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HorusPacketRow.init))
    }
    /// The changed rows of the table "channels" of the stack "horus", newest first, at most `capacity` rows.
    public static func horusChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[HorusChannelRow], SDRKitCode> {
        var rows = [sdrkit_horus_channel_row](repeating: sdrkit_horus_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_horus_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HorusChannelRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum ScienceCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func sondeClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sonde_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func sondeRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_sonde_row_args()
        args.size = UInt32(MemoryLayout<sdrkit_sonde_row_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_sonde_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select. Shows the plots of one sonde.
    public static func sondeSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_sonde_row_args()
        args.size = UInt32(MemoryLayout<sdrkit_sonde_row_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_sonde_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func timeClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_time_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func eczasClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_eczas_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func sameClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_same_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear the county table. Forgets the loaded county table.
    public static func sameClearCounties(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_same_clear_counties(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func mscatterClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mscatter_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ssdvClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ssdv_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func astroClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_astro_clear(handle)
        return sdrkitResult(code)
    }
    /// Capture. Keeps the next finished integration as the off-source spectrum, or its power as the hot or the cold load.
    public static func astroCapture(_ handle: SDRKIT_HANDLE, what: Int32) -> Result<Void, SDRKitCode> {
        var args = sdrkit_astro_capture_args()
        args.size = UInt32(MemoryLayout<sdrkit_astro_capture_args>.size)
        let code: Int32 = { () -> Int32 in
        args.what = what
        return sdrkit_astro_capture(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Flag RFI. 
    public static func astroFlagRfi(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_astro_flag_rfi(handle)
        return sdrkitResult(code)
    }
    /// Fit lines. 
    public static func astroFitLines(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_astro_fit_lines(handle)
        return sdrkitResult(code)
    }
    /// Add to the l-v map. 
    public static func astroAddMap(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_astro_add_map(handle)
        return sdrkitResult(code)
    }
    /// Clear the map. 
    public static func astroClearMap(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_astro_clear_map(handle)
        return sdrkitResult(code)
    }
    /// Restart the integration. 
    public static func astroRestart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_astro_restart(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ionoClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_iono_clear(handle)
        return sdrkitResult(code)
    }
    /// Show a sounder. Shows the ionogram and the history of one sounder.
    public static func ionoSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_iono_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_iono_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_iono_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Show the latest sounder. 
    public static func ionoSelectLatest(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_iono_select_latest(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func sfericsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sferics_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and counters, and learns the floor and the mains train again.
    public static func sfericsReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sferics_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func supersidClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_supersid_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Clears, and starts the levels of the hour and the judging again (15 minutes); keeps the minute means.
    public static func supersidReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_supersid_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func schumannClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_schumann_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and starts the monitor again; the average and the comparison level are learned again.
    public static func schumannReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_schumann_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func whistlerClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_whistler_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and starts the detector again; the floor is learned again.
    public static func whistlerReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_whistler_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func lightningTdoaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lightning_tdoa_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and counters and learns the floor of the sferic timer again; the clock discipline is kept.
    public static func lightningTdoaReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lightning_tdoa_reset(handle)
        return sdrkitResult(code)
    }
    /// Feed a datagram. The server takes one datagram as if it came from the network.
    public static func lightningTdoaDatagram(_ handle: SDRKIT_HANDLE, bytes: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lightning_tdoa_datagram_args()
        args.size = UInt32(MemoryLayout<sdrkit_lightning_tdoa_datagram_args>.size)
        let code: Int32 = bytes.withUnsafeBufferPointer { bytes_ in { () -> Int32 in
        args.bytes = sdrkit_bytes(ptr: bytes_.baseAddress, len: bytes_.count)
        return sdrkit_lightning_tdoa_datagram(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Time mark. UTC at an input sample, from the radio's GPS time or a GNSS receiver, as the host reads it.
    public static func lightningTdoaTimeMark(_ handle: SDRKIT_HANDLE, source: String, sample: Int64, utcNs: Int64, sigmaNs: Double? = nil, biasNs: Double? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lightning_tdoa_time_mark_args()
        args.size = UInt32(MemoryLayout<sdrkit_lightning_tdoa_time_mark_args>.size)
        let code: Int32 = sdrkitWithString(source) { source_ in { () -> Int32 in
        args.source = source_
        args.sample = sample
        args.utc_ns = utcNs
        args.has_sigma_ns = sigmaNs != nil
        if let given = sigmaNs { args.sigma_ns = given }
        args.has_bias_ns = biasNs != nil
        if let given = biasNs { args.bias_ns = given }
        return sdrkit_lightning_tdoa_time_mark(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func joveClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jove_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and starts the detector again; the background is learned again.
    public static func joveReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_jove_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func callistoClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_callisto_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and starts the detector again; the background is learned again.
    public static func callistoReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_callisto_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func riometerClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_riometer_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Starts the power estimate again and empties the series; the quiet-day curve stays.
    public static func riometerReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_riometer_reset(handle)
        return sdrkitResult(code)
    }
    /// Export the quiet-day curve. Replies with the curve as text (qdc1 and 288 values).
    public static func riometerExportQdc(_ handle: SDRKIT_HANDLE) -> Result<RiometerQdcReply, SDRKitCode> {
        var reply = sdrkit_riometer_qdc_reply()
        let code = sdrkit_riometer_export_qdc(handle, &reply)
        return code == SDRKIT_OK ? .success(RiometerQdcReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Import a quiet-day curve. Takes a curve as text; a bad text is refused and changes nothing.
    public static func riometerImportQdc(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_riometer_qdc_args()
        args.size = UInt32(MemoryLayout<sdrkit_riometer_qdc_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_riometer_import_qdc(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func emeEchoClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_eme_echo_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and starts the detector again; the noise is learned again.
    public static func emeEchoReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_eme_echo_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func beaconEventsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_beacon_events_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Empties the tables and starts the detector again.
    public static func beaconEventsReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_beacon_events_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func pulsarClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pulsar_clear(handle)
        return sdrkitResult(code)
    }
    /// Search now. Runs a search on the rows held now.
    public static func pulsarSearchNow(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pulsar_search_now(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func radarDetectClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_radar_detect_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func horusClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_horus_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum ScienceSettings {
    /// Sondes followed at once.
    public static func sondeChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channels", value))
    }
    /// Landing smoothing.
    public static func sondeLandingWindowS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "landing_window_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeLandingWindowS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "landing_window_s", value))
    }
    /// Use the ascent winds.
    public static func sondeLandingLayers(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "landing_layers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeLandingLayers(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "landing_layers", value))
    }
    /// Layer.
    public static func sondeLandingLayerM(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "landing_layer_m", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeLandingLayerM(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "landing_layer_m", value))
    }
    /// Ascent wind weight.
    public static func sondeLandingAscentWeight(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "landing_ascent_weight", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeLandingAscentWeight(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "landing_ascent_weight", value))
    }
    /// Ground height.
    public static func sondeGroundM(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ground_m", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeGroundM(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ground_m", value))
    }
    /// The options of the setting "station" of the stack "time".
    public enum TimeStation: Int32 {
        case auto = 0
        case dcf77 = 1
        case msf = 2
        case wwvb = 3
        case jjy40 = 4
        case jjy60 = 5
        case bpc = 6
        case wwv = 7
        case chu = 8
        case bpm = 9
        case bpmc = 10
        case hla = 11
        case rwm = 12
    }
    /// Station.
    public static func timeStation(_ handle: SDRKIT_HANDLE) -> Result<TimeStation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "station", &v)
        return code == SDRKIT_OK ? .success(TimeStation(rawValue: v) ?? TimeStation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeStation(_ handle: SDRKIT_HANDLE, _ value: TimeStation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "station", value.rawValue))
    }
    /// The options of the setting "sixty_khz" of the stack "time".
    public enum TimeSixtyKhz: Int32 {
        case msf = 0
        case wwvb = 1
        case jjy60 = 2
    }
    /// 60 kHz is.
    public static func timeSixtyKhz(_ handle: SDRKIT_HANDLE) -> Result<TimeSixtyKhz, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sixty_khz", &v)
        return code == SDRKIT_OK ? .success(TimeSixtyKhz(rawValue: v) ?? TimeSixtyKhz(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeSixtyKhz(_ handle: SDRKIT_HANDLE, _ value: TimeSixtyKhz) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sixty_khz", value.rawValue))
    }
    /// Demodulated audio recording.
    public static func timeAudioInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeAudioInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_input", value))
    }
    /// Carrier tone of an LF recording.
    public static func timeAudioToneHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "audio_tone_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeAudioToneHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "audio_tone_hz", value))
    }
    /// Recording clock slow by.
    public static func timeClockSlowPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock_slow_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeClockSlowPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock_slow_pct", value))
    }
    /// Listen.
    public static func timeListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "clock_mode" of the stack "time".
    public enum TimeClockMode: Int32 {
        case none = 0
        case live = 1
        case fixed = 2
    }
    /// Clock to compare.
    public static func timeClockMode(_ handle: SDRKIT_HANDLE) -> Result<TimeClockMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "clock_mode", &v)
        return code == SDRKIT_OK ? .success(TimeClockMode(rawValue: v) ?? TimeClockMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeClockMode(_ handle: SDRKIT_HANDLE, _ value: TimeClockMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "clock_mode", value.rawValue))
    }
    /// Sample 0 at (UTC, Unix s).
    public static func timeClockOrigin(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock_origin", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeClockOrigin(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock_origin", value))
    }
    /// Year hint.
    public static func timeYearHint(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "year_hint", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeYearHint(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "year_hint", value))
    }
    /// Audio recording.
    public static func eczasAudioInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasAudioInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_input", value))
    }
    /// Carrier tone in the audio.
    public static func eczasAudioToneHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "audio_tone_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasAudioToneHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "audio_tone_hz", value))
    }
    /// Listen.
    public static func eczasListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "clock_mode" of the stack "eczas".
    public enum EczasClockMode: Int32 {
        case none = 0
        case live = 1
        case fixed = 2
    }
    /// Clock to compare.
    public static func eczasClockMode(_ handle: SDRKIT_HANDLE) -> Result<EczasClockMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "clock_mode", &v)
        return code == SDRKIT_OK ? .success(EczasClockMode(rawValue: v) ?? EczasClockMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasClockMode(_ handle: SDRKIT_HANDLE, _ value: EczasClockMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "clock_mode", value.rawValue))
    }
    /// Sample 0 at (UTC, Unix s).
    public static func eczasClockOrigin(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock_origin", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasClockOrigin(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock_origin", value))
    }
    /// The options of the setting "channel" of the stack "same".
    public enum SameChannel: Int32 {
        case wx1 = 0
        case wx2 = 1
        case wx3 = 2
        case wx4 = 3
        case wx5 = 4
        case wx6 = 5
        case wx7 = 6
    }
    /// Weather channel.
    public static func sameChannel(_ handle: SDRKIT_HANDLE) -> Result<SameChannel, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(SameChannel(rawValue: v) ?? SameChannel(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSameChannel(_ handle: SDRKIT_HANDLE, _ value: SameChannel) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel", value.rawValue))
    }
    /// The options of the setting "country" of the stack "same".
    public enum SameCountry: Int32 {
        case us = 0
        case canada = 1
    }
    /// Location names.
    public static func sameCountry(_ handle: SDRKIT_HANDLE) -> Result<SameCountry, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "country", &v)
        return code == SDRKIT_OK ? .success(SameCountry(rawValue: v) ?? SameCountry(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSameCountry(_ handle: SDRKIT_HANDLE, _ value: SameCountry) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "country", value.rawValue))
    }
    /// County table.
    public static func sameCountyText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "county_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSameCountyText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "county_text", $0) })
    }
    /// Year for issue times.
    public static func sameYear(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "year", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSameYear(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "year", value))
    }
    /// Listen.
    public static func sameListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSameListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Detection level.
    public static func mscatterThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMscatterThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func mscatterStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMscatterStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// The options of the setting "baud" of the stack "ssdv".
    public enum SsdvBaud: Int32 {
        case auto = 0
        case n3008n2 = 1
        case n6008n2 = 2
        case n12008n2 = 3
        case n1200 = 4
        case n2400 = 5
        case n4800 = 6
        case n9600 = 7
        case n19200 = 8
        case n115200 = 9
    }
    /// Speed.
    public static func ssdvBaud(_ handle: SDRKIT_HANDLE) -> Result<SsdvBaud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(SsdvBaud(rawValue: v) ?? SsdvBaud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSsdvBaud(_ handle: SDRKIT_HANDLE, _ value: SsdvBaud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "baud", value.rawValue))
    }
    /// The options of the setting "fft" of the stack "astro".
    public enum AstroFft: Int32 {
        case n1024 = 0
        case n2048 = 1
        case n4096 = 2
        case n8192 = 3
        case n16384 = 4
    }
    /// FFT size.
    public static func astroFft(_ handle: SDRKIT_HANDLE) -> Result<AstroFft, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fft", &v)
        return code == SDRKIT_OK ? .success(AstroFft(rawValue: v) ?? AstroFft(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroFft(_ handle: SDRKIT_HANDLE, _ value: AstroFft) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fft", value.rawValue))
    }
    /// Integration.
    public static func astroIntegrationS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "integration_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroIntegrationS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "integration_s", value))
    }
    /// The options of the setting "line" of the stack "astro".
    public enum AstroLine: Int32 {
        case hi = 0
        case oh1612 = 1
        case oh1665 = 2
        case oh1667 = 3
        case oh1720 = 4
        case custom = 5
    }
    /// Rest line.
    public static func astroLine(_ handle: SDRKIT_HANDLE) -> Result<AstroLine, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "line", &v)
        return code == SDRKIT_OK ? .success(AstroLine(rawValue: v) ?? AstroLine(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroLine(_ handle: SDRKIT_HANDLE, _ value: AstroLine) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "line", value.rawValue))
    }
    /// Custom rest frequency.
    public static func astroRestHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "rest_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroRestHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "rest_hz", value))
    }
    /// The options of the setting "band_shape" of the stack "astro".
    public enum AstroBandShape: Int32 {
        case none = 0
        case offSource = 1
        case switching = 2
    }
    /// Band shape from.
    public static func astroBandShape(_ handle: SDRKIT_HANDLE) -> Result<AstroBandShape, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band_shape", &v)
        return code == SDRKIT_OK ? .success(AstroBandShape(rawValue: v) ?? AstroBandShape(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroBandShape(_ handle: SDRKIT_HANDLE, _ value: AstroBandShape) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band_shape", value.rawValue))
    }
    /// Fold.
    public static func astroFold(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "fold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroFold(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "fold", value))
    }
    /// DC bridge.
    public static func astroDcBins(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "dc_bins", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroDcBins(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "dc_bins", value))
    }
    /// Baseline degree.
    public static func astroBaselineDegree(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "baseline_degree", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroBaselineDegree(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "baseline_degree", value))
    }
    /// Line window.
    public static func astroWindow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "window", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroWindow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "window", value))
    }
    /// Window from.
    public static func astroWindowLoKms(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "window_lo_kms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroWindowLoKms(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "window_lo_kms", value))
    }
    /// Window to.
    public static func astroWindowHiKms(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "window_hi_kms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroWindowHiKms(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "window_hi_kms", value))
    }
    /// Edges left out.
    public static func astroEdge(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "edge", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroEdge(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "edge", value))
    }
    /// Smoothing.
    public static func astroSmoothBins(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "smooth_bins", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroSmoothBins(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "smooth_bins", value))
    }
    /// System temperature.
    public static func astroTsysK(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tsys_k", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroTsysK(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tsys_k", value))
    }
    /// The options of the setting "lsr" of the stack "astro".
    public enum AstroLsr: Int32 {
        case kinematic = 0
        case dynamical = 1
    }
    /// LSR.
    public static func astroLsr(_ handle: SDRKIT_HANDLE) -> Result<AstroLsr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "lsr", &v)
        return code == SDRKIT_OK ? .success(AstroLsr(rawValue: v) ?? AstroLsr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroLsr(_ handle: SDRKIT_HANDLE, _ value: AstroLsr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "lsr", value.rawValue))
    }
    /// The options of the setting "target" of the stack "astro".
    public enum AstroTarget: Int32 {
        case galactic = 0
        case sun = 1
        case moon = 2
        case casa = 3
        case cyga = 4
        case sgra = 5
        case radec = 6
    }
    /// Target.
    public static func astroTarget(_ handle: SDRKIT_HANDLE) -> Result<AstroTarget, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "target", &v)
        return code == SDRKIT_OK ? .success(AstroTarget(rawValue: v) ?? AstroTarget(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroTarget(_ handle: SDRKIT_HANDLE, _ value: AstroTarget) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "target", value.rawValue))
    }
    /// Target l.
    public static func astroTargetL(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "target_l", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroTargetL(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "target_l", value))
    }
    /// Target b.
    public static func astroTargetB(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "target_b", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroTargetB(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "target_b", value))
    }
    /// Target RA (J2000).
    public static func astroTargetRa(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "target_ra", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroTargetRa(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "target_ra", value))
    }
    /// Target Dec (J2000).
    public static func astroTargetDec(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "target_dec", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroTargetDec(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "target_dec", value))
    }
    /// Sweeps start on whole seconds.
    public static func ionoLocked(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "locked", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIonoLocked(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "locked", value))
    }
    /// The options of the setting "geometry" of the stack "iono".
    public enum IonoGeometry: Int32 {
        case auto = 0
        case vertical = 1
        case oblique = 2
    }
    /// Geometry.
    public static func ionoGeometry(_ handle: SDRKIT_HANDLE) -> Result<IonoGeometry, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "geometry", &v)
        return code == SDRKIT_OK ? .success(IonoGeometry(rawValue: v) ?? IonoGeometry(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIonoGeometry(_ handle: SDRKIT_HANDLE, _ value: IonoGeometry) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "geometry", value.rawValue))
    }
    /// Colour scale top.
    public static func ionoDbTop(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "db_top", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIonoDbTop(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "db_top", value))
    }
    /// Threshold over the floor.
    public static func sfericsThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Leave out mains trains.
    public static func sfericsMainsFilter(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mains_filter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsMainsFilter(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mains_filter", value))
    }
    /// Merge time.
    public static func sfericsMergeMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "merge_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsMergeMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "merge_ms", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func sfericsStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Monitored transmitters.
    public static func supersidTransmitters(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "transmitters", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidTransmitters(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "transmitters", $0) })
    }
    /// Step threshold.
    public static func supersidStepDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "step_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidStepDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "step_db", value))
    }
    /// Sample interval.
    public static func supersidAverageS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "average_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidAverageS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "average_s", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func supersidStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// The options of the setting "log_format" of the stack "supersid".
    public enum SupersidLogFormat: Int32 {
        case off = 0
        case sidFormat = 1
        case supersidFormat = 2
    }
    /// Data file.
    public static func supersidLogFormat(_ handle: SDRKIT_HANDLE) -> Result<SupersidLogFormat, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "log_format", &v)
        return code == SDRKIT_OK ? .success(SupersidLogFormat(rawValue: v) ?? SupersidLogFormat(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidLogFormat(_ handle: SDRKIT_HANDLE, _ value: SupersidLogFormat) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "log_format", value.rawValue))
    }
    /// Site name.
    public static func supersidSite(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "site", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidSite(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "site", $0) })
    }
    /// Monitor id.
    public static func supersidMonitorId(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "monitor_id", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidMonitorId(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "monitor_id", $0) })
    }
    /// Data file folder.
    public static func supersidLogFolder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "log_folder", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidLogFolder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "log_folder", $0) })
    }
    /// Transient threshold.
    public static func schumannThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSchumannThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func schumannStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSchumannStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Whistler path threshold.
    public static func whistlerThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWhistlerThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func whistlerStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWhistlerStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// The options of the setting "role" of the stack "lightning_tdoa".
    public enum LightningTdoaRole: Int32 {
        case local = 0
        case client = 1
        case server = 2
    }
    /// Role.
    public static func lightningTdoaRole(_ handle: SDRKIT_HANDLE) -> Result<LightningTdoaRole, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "role", &v)
        return code == SDRKIT_OK ? .success(LightningTdoaRole(rawValue: v) ?? LightningTdoaRole(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaRole(_ handle: SDRKIT_HANDLE, _ value: LightningTdoaRole) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "role", value.rawValue))
    }
    /// Server address (client).
    public static func lightningTdoaServer(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "server", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaServer(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "server", $0) })
    }
    /// UDP port.
    public static func lightningTdoaPort(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "port", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaPort(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "port", value))
    }
    /// Network key (secret).
    public static func lightningTdoaNetworkKey(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "network_key", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaNetworkKey(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "network_key", $0) })
    }
    /// Station name.
    public static func lightningTdoaStationName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "station_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaStationName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "station_name", $0) })
    }
    /// The server's radio counts as a station.
    public static func lightningTdoaServerIsStation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "server_is_station", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaServerIsStation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "server_is_station", value))
    }
    /// The options of the setting "system_clock" of the stack "lightning_tdoa".
    public enum LightningTdoaSystemClock: Int32 {
        case system = 0
        case ntp = 1
        case ptp = 2
    }
    /// Computer clock.
    public static func lightningTdoaSystemClock(_ handle: SDRKIT_HANDLE) -> Result<LightningTdoaSystemClock, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "system_clock", &v)
        return code == SDRKIT_OK ? .success(LightningTdoaSystemClock(rawValue: v) ?? LightningTdoaSystemClock(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaSystemClock(_ handle: SDRKIT_HANDLE, _ value: LightningTdoaSystemClock) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "system_clock", value.rawValue))
    }
    /// Computer clock error.
    public static func lightningTdoaSystemSigmaUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "system_sigma_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaSystemSigmaUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "system_sigma_us", value))
    }
    /// Use the radio's GPS time.
    public static func lightningTdoaUseRadioTime(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "use_radio_time", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaUseRadioTime(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "use_radio_time", value))
    }
    /// Timing transmitter id.
    public static func lightningTdoaTimingId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "timing_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "timing_id", value))
    }
    /// Timing transmitter.
    public static func lightningTdoaTimingLabel(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "timing_label", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingLabel(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "timing_label", $0) })
    }
    /// Timing transmitter frequency.
    public static func lightningTdoaTimingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "timing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "timing_hz", value))
    }
    /// Timing transmitter symbol rate.
    public static func lightningTdoaTimingBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "timing_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "timing_baud", value))
    }
    /// Timing transmitter latitude.
    public static func lightningTdoaTimingLat(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "timing_lat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingLat(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "timing_lat", value))
    }
    /// Timing transmitter longitude.
    public static func lightningTdoaTimingLon(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "timing_lon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingLon(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "timing_lon", value))
    }
    /// Sferic threshold.
    public static func lightningTdoaThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Ground-wave velocity / c.
    public static func lightningTdoaVelocityFactor(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "velocity_factor", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaVelocityFactor(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "velocity_factor", value))
    }
    /// Ionosphere hops for long paths.
    public static func lightningTdoaHops(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hops", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaHops(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hops", value))
    }
    /// Reflection height.
    public static func lightningTdoaHopHeightKm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hop_height_km", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaHopHeightKm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hop_height_km", value))
    }
    /// Fine stations for a fix.
    public static func lightningTdoaMinStations(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "min_stations", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaMinStations(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "min_stations", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func lightningTdoaStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Burst threshold per bin.
    public static func joveThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJoveThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func joveStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJoveStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Burst threshold per bin.
    public static func callistoThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func callistoStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Write FITS files.
    public static func callistoFits(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "fits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoFits(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "fits", value))
    }
    /// FITS folder.
    public static func callistoFitsFolder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "fits_folder", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoFitsFolder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "fits_folder", $0) })
    }
    /// Instrument code.
    public static func callistoStationCode(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "station_code", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoStationCode(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "station_code", $0) })
    }
    /// Focus code.
    public static func callistoFocusCode(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "focus_code", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoFocusCode(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "focus_code", value))
    }
    /// File length.
    public static func callistoFileS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "file_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoFileS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "file_s", value))
    }
    /// The options of the setting "fits_bitpix" of the stack "callisto".
    public enum CallistoFitsBitpix: Int32 {
        case n16Bits = 0
        case n8Bits = 1
    }
    /// Pixel type.
    public static func callistoFitsBitpix(_ handle: SDRKIT_HANDLE) -> Result<CallistoFitsBitpix, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fits_bitpix", &v)
        return code == SDRKIT_OK ? .success(CallistoFitsBitpix(rawValue: v) ?? CallistoFitsBitpix(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoFitsBitpix(_ handle: SDRKIT_HANDLE, _ value: CallistoFitsBitpix) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fits_bitpix", value.rawValue))
    }
    /// dB per digit.
    public static func callistoDbPerDigit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "db_per_digit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoDbPerDigit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "db_per_digit", value))
    }
    /// Level of digit 0.
    public static func callistoZeroDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "zero_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoZeroDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "zero_dbfs", value))
    }
    /// Absorption event threshold.
    public static func riometerThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRiometerThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Quiet-day curve (saved state).
    public static func riometerQdc(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "qdc", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRiometerQdc(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "qdc", $0) })
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func riometerStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRiometerStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Echo threshold.
    public static func emeEchoThresholdSigma(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_sigma", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEmeEchoThresholdSigma(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_sigma", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func emeEchoStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEmeEchoStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Opening threshold.
    public static func beaconEventsThresholdZ(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_z", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconEventsThresholdZ(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_z", value))
    }
    /// UTC of sample 0 (files; 0 = host clock).
    public static func beaconEventsStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconEventsStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc", value))
    }
    /// Pulsar period.
    public static func pulsarPeriodS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "period_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarPeriodS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "period_s", value))
    }
    /// Dispersion measure.
    public static func pulsarDm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarDm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dm", value))
    }
    /// Filter-bank channels.
    public static func pulsarChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channels", value))
    }
    /// Power sample time.
    public static func pulsarSampleMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "sample_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarSampleMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "sample_ms", value))
    }
    /// Profile bins.
    public static func pulsarBins(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "bins", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarBins(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "bins", value))
    }
    /// Trial DM span (±).
    public static func pulsarDmSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dm_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarDmSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dm_span", value))
    }
    /// Trial DMs.
    public static func pulsarDmSteps(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "dm_steps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarDmSteps(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "dm_steps", value))
    }
    /// Trial period span (± relative).
    public static func pulsarPeriodSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "period_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarPeriodSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "period_span", value))
    }
    /// Trial periods.
    public static func pulsarPeriodSteps(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "period_steps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarPeriodSteps(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "period_steps", value))
    }
    /// Flux of the pulsar.
    public static func pulsarFluxJy(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "flux_jy", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarFluxJy(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "flux_jy", value))
    }
    /// System temperature.
    public static func pulsarTsysK(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tsys_k", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarTsysK(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tsys_k", value))
    }
    /// Observing time.
    public static func pulsarHours(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hours", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarHours(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hours", value))
    }
    /// Look for other balloons in the span.
    public static func horusSpanSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "span_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHorusSpanSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "span_search", value))
    }
    /// Channels at most.
    public static func horusMaxChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHorusMaxChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_channels", value))
    }
    /// Payload IDs.
    public static func horusPayloadIds(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "payload_ids", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHorusPayloadIds(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "payload_ids", $0) })
    }
    /// Custom fields of v2.
    public static func horusCustomFormats(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "custom_formats", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHorusCustomFormats(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "custom_formats", $0) })
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum ScienceGeneratorSettings {
    /// Test signal: sondes.
    public static func sondeCount(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_count", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeCount(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_count", value))
    }
    /// Test signal: types.
    public static func sondeTypes(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_types", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeTypes(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_types", $0) })
    }
    /// Test signal: east drift.
    public static func sondeDriftMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_drift_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeDriftMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_drift_ms", value))
    }
    /// Test signal: second RS41 offset.
    public static func sondeRs41bKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rs41b_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeRs41bKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rs41b_khz", value))
    }
    /// Test signal: 1680 MHz (LMS6).
    public static func sondeLband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_lband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSondeLband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_lband", value))
    }
    /// Test signal: start (UTC, 0 = host clock).
    public static func timeStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTimeStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_start_utc", value))
    }
    /// Test signal: start (UTC, 0 = host clock).
    public static func eczasStartUtc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_start_utc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasStartUtc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_start_utc", value))
    }
    /// Test signal: carrier only.
    public static func eczasCarrierOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_carrier_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEczasCarrierOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_carrier_only", value))
    }
    /// The options of the setting "gen_scene" of the stack "same".
    public enum SameScene: Int32 {
        case two = 0
        case tornado = 1
        case noise = 2
    }
    /// Test signal: scene.
    public static func sameScene(_ handle: SDRKIT_HANDLE) -> Result<SameScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(SameScene(rawValue: v) ?? SameScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSameScene(_ handle: SDRKIT_HANDLE, _ value: SameScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal: loop.
    public static func sameLoop(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_loop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSameLoop(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_loop", value))
    }
    /// Test signal: beacon offset.
    public static func mscatterBeaconHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_beacon_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMscatterBeaconHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_beacon_hz", value))
    }
    /// Test signal: mean gap between pings.
    public static func mscatterMeanGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_mean_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMscatterMeanGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_mean_gap_s", value))
    }
    /// The options of the setting "gen_mode" of the stack "ssdv".
    public enum SsdvMode: Int32 {
        case auto = 0
        case n3008n2 = 1
        case n6008n2 = 2
        case n12008n2 = 3
        case n1200 = 4
        case n2400 = 5
        case n4800 = 6
        case n9600 = 7
        case n19200 = 8
        case n115200 = 9
    }
    /// Test signal: speed.
    public static func ssdvMode(_ handle: SDRKIT_HANDLE) -> Result<SsdvMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(SsdvMode(rawValue: v) ?? SsdvMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSsdvMode(_ handle: SDRKIT_HANDLE, _ value: SsdvMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// Test signal: off source (no line).
    public static func astroOffSource(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_off_source", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroOffSource(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_off_source", value))
    }
    /// Test signal: line velocity.
    public static func astroLineKms(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_line_kms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAstroLineKms(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_line_kms", value))
    }
    /// Test signal: the radio's centre.
    public static func ionoCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIonoCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test signal: no sounders.
    public static func ionoNoiseOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_noise_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIonoNoiseOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_noise_only", value))
    }
    /// Test signal: strokes a second.
    public static func sfericsStrikeRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_strike_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsStrikeRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_strike_rate", value))
    }
    /// The options of the setting "gen_scene" of the stack "sferics".
    public enum SfericsScene: Int32 {
        case storm = 0
        case mains = 1
        case stormMains = 2
        case quiet = 3
    }
    /// Test signal: scene.
    public static func sfericsScene(_ handle: SDRKIT_HANDLE) -> Result<SfericsScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(SfericsScene(rawValue: v) ?? SfericsScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsScene(_ handle: SDRKIT_HANDLE, _ value: SfericsScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal: mains frequency.
    public static func sfericsMainsHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_mains_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSfericsMainsHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_mains_hz", value))
    }
    /// Test signal flare.
    public static func supersidFlare(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_flare", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSupersidFlare(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_flare", value))
    }
    /// Test signal fundamental.
    public static func schumannFundamentalHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fundamental_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSchumannFundamentalHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fundamental_hz", value))
    }
    /// Test signal transients.
    public static func schumannTransients(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_transients", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSchumannTransients(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_transients", value))
    }
    /// Test signal dispersion.
    public static func whistlerDispersion(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_dispersion", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWhistlerDispersion(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_dispersion", value))
    }
    /// Test signal tweek cut-off.
    public static func whistlerCutoffHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cutoff_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWhistlerCutoffHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cutoff_hz", value))
    }
    /// Test signal whistler SNR.
    public static func whistlerToneSnrDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_snr_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWhistlerToneSnrDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_snr_db", value))
    }
    /// Test signal: strikes a second.
    public static func lightningTdoaStrikeRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_strike_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaStrikeRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_strike_rate", value))
    }
    /// Test signal: timing transmitter.
    public static func lightningTdoaTimingTx(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_timing_tx", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingTx(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_timing_tx", value))
    }
    /// Test signal: timing transmitter offset.
    public static func lightningTdoaTimingOffsetHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_timing_offset_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaTimingOffsetHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_timing_offset_hz", value))
    }
    /// Test signal: station latitude.
    public static func lightningTdoaStationLat(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_station_lat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaStationLat(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_station_lat", value))
    }
    /// Test signal: station longitude.
    public static func lightningTdoaStationLon(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_station_lon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLightningTdoaStationLon(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_station_lon", value))
    }
    /// Test signal burst level.
    public static func joveLevelDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_level_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setJoveLevelDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_level_db", value))
    }
    /// Test signal burst level.
    public static func callistoLevelDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_level_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoLevelDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_level_db", value))
    }
    /// Test signal burst drift.
    public static func callistoBurstDriftHzS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_burst_drift_hz_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCallistoBurstDriftHzS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_burst_drift_hz_s", value))
    }
    /// Test signal burst share.
    public static func riometerBurstShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_burst_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRiometerBurstShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_burst_share", value))
    }
    /// Test signal echo delay.
    public static func emeEchoMoonDelayS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_moon_delay_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEmeEchoMoonDelayS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_moon_delay_s", value))
    }
    /// Test signal echo Doppler.
    public static func emeEchoMoonDopplerHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_moon_doppler_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEmeEchoMoonDopplerHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_moon_doppler_hz", value))
    }
    /// Test signal echo spread.
    public static func emeEchoMoonSpreadHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_moon_spread_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEmeEchoMoonSpreadHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_moon_spread_hz", value))
    }
    /// Test signal echo level.
    public static func emeEchoMoonLevelDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_moon_level_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEmeEchoMoonLevelDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_moon_level_db", value))
    }
    /// Test signal level.
    public static func beaconEventsLevelDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_level_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconEventsLevelDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_level_db", value))
    }
    /// Test signal spread.
    public static func beaconEventsSpreadHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spread_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconEventsSpreadHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spread_hz", value))
    }
    /// Test signal flutter.
    public static func beaconEventsFlutterDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_flutter_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconEventsFlutterDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_flutter_db", value))
    }
    /// Test signal S/N.
    public static func pulsarSnr(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_snr", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarSnr(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_snr", value))
    }
    /// Test signal period.
    public static func pulsarPeriodS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_period_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarPeriodS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_period_s", value))
    }
    /// Test signal DM.
    public static func pulsarDm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_dm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPulsarDm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_dm", value))
    }
    /// Test signal rotation.
    public static func radarDetectRpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadarDetectRpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rpm", value))
    }
    /// Test signal PRF.
    public static func radarDetectPrfHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_prf_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRadarDetectPrfHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_prf_hz", value))
    }
    /// The options of the setting "gen_dual_prf" of the stack "radar_detect".
    public enum RadarDetectDualPrf: Int32 {
        case auto = 0
        case single = 1
        case dual = 2
    }
    /// Test signal staggered PRF.
    public static func radarDetectDualPrf(_ handle: SDRKIT_HANDLE) -> Result<RadarDetectDualPrf, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_dual_prf", &v)
        return code == SDRKIT_OK ? .success(RadarDetectDualPrf(rawValue: v) ?? RadarDetectDualPrf(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRadarDetectDualPrf(_ handle: SDRKIT_HANDLE, _ value: RadarDetectDualPrf) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_dual_prf", value.rawValue))
    }
    /// The options of the setting "gen_content" of the stack "horus".
    public enum HorusContent: Int32 {
        case v2 = 0
        case v1 = 1
        case rtty100 = 2
        case rtty300 = 3
        case rtty50 = 4
        case three = 5
    }
    /// Content.
    public static func horusContent(_ handle: SDRKIT_HANDLE) -> Result<HorusContent, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_content", &v)
        return code == SDRKIT_OK ? .success(HorusContent(rawValue: v) ?? HorusContent(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHorusContent(_ handle: SDRKIT_HANDLE, _ value: HorusContent) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_content", value.rawValue))
    }
    /// Tone spacing.
    public static func horusSpacingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spacing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHorusSpacingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spacing_hz", value))
    }
}

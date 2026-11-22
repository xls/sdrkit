// sdrkit: the typed interface of the stack family "fm", version 1.3. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum FmInterface {
    public static let major = 1
    public static let minor = 3
}

/// The result fields of the FM receiver, in the order a screen lists them. The display writes the band plan ([`BandPart`]) and the measurements and RDS data ([`RdsPart`]); the broadcast data publisher writes the RadioText and whether its item is running.
public struct FmFields {
    /// The station name (PS): all 8 characters with trailing spaces, empty before the first name.
    public var station: String
    /// The name being collected (`_` for a segment not yet received).
    public var stationPartial: String
    /// The RadioText.
    public var text: String
    /// Every RadioText segment has arrived.
    public var textComplete: Bool
    /// The programme type's name in the table in force.
    public var programme: String?
    /// The programme type code.
    public var pty: Int64?
    /// The programme identification, four hexadecimal digits.
    public var pi: String?
    /// RBDS call letters.
    public var callLetters: String?
    /// RBDS meanings apply.
    public var rbds: Bool
    /// Traffic programme.
    public var tp: Bool
    /// Traffic announcement.
    public var ta: Bool
    /// Music (true) or speech.
    public var music: Bool
    /// Programme type name (PTYN).
    public var ptyn: String
    /// Extended country code.
    public var ecc: Int64?
    /// Language code.
    public var language: Int64?
    /// Programme item number, `DD HH:MM`.
    public var pin: String?
    /// The station's clock time.
    public var clock: Double?
    /// The station's local time offset.
    public var clockOffset: Double?
    /// Enhanced RadioText.
    public var ert: String
    /// The RadioText+ item is running.
    public var itemRunning: Bool
    /// RDS block sync.
    public var rdsSync: Bool
    /// RDS usable blocks.
    public var rdsOkPct: Double
    /// Accepted RDS groups.
    public var groups: Int64
    /// Good RDS blocks.
    public var blocksGood: Int64
    /// Corrected RDS blocks (voted ones too).
    public var blocksCorrected: Int64
    /// Bad RDS blocks.
    public var blocksBad: Int64
    /// RDS blocks recovered by voting.
    public var blocksVoted: Int64
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The level of the channel.
    public var levelDbfs: Double
    /// The offset of the carrier.
    public var offsetHz: Double
    /// A carrier is present.
    public var carrier: Bool
    /// Stereo is reported.
    public var stereo: Bool
    /// The stereo blend while stereo is reported.
    public var stereoPct: Double
    /// The pilot injection, percent of 75 kHz.
    public var pilotPct: Double
    /// The pilot PLL is locked.
    public var pilotLocked: Bool
    /// The peak deviation.
    public var deviationHz: Double
    /// The multipath ripple.
    public var multipathPct: Double
    /// The equaliser is in use for the sound.
    public var equaliser: Bool
    /// The squelch is closed.
    public var squelched: Bool
    /// The channel filter width in use.
    public var channelWidthHz: Double
    /// The upper frequency of the `mpx` series.
    public var mpxTopHz: Double
    /// Signal decoded since the last reset.
    public var signalTime: Double
    /// The lowest frequency of the band.
    public var bandStartHz: Double
    /// The highest frequency of the band.
    public var bandEndHz: Double
    /// The channel step.
    public var bandStepHz: Double
    /// The first channel of the raster.
    public var bandFirstHz: Double
    /// TMC location table number.
    public var tmcLtn: Int64?
    /// TMC service identifier.
    public var tmcSid: Int64?
    /// TMC alternative-frequency indicator.
    public var tmcAfi: Bool
    /// TMC enhanced mode.
    public var tmcEnhancedMode: Bool
    /// TMC geographical scope bits (I, N, R, U).
    public var tmcScope: Int64

    init(_ c: sdrkit_fm_fields) {
        station = sdrkitText(c.station)
        stationPartial = sdrkitText(c.station_partial)
        text = sdrkitText(c.text)
        textComplete = c.text_complete
        programme = c.has_programme ? sdrkitText(c.programme) : nil
        pty = c.has_pty ? c.pty : nil
        pi = c.has_pi ? sdrkitText(c.pi) : nil
        callLetters = c.has_call_letters ? sdrkitText(c.call_letters) : nil
        rbds = c.rbds
        tp = c.tp
        ta = c.ta
        music = c.music
        ptyn = sdrkitText(c.ptyn)
        ecc = c.has_ecc ? c.ecc : nil
        language = c.has_language ? c.language : nil
        pin = c.has_pin ? sdrkitText(c.pin) : nil
        clock = c.has_clock ? c.clock : nil
        clockOffset = c.has_clock_offset ? c.clock_offset : nil
        ert = sdrkitText(c.ert)
        itemRunning = c.item_running
        rdsSync = c.rds_sync
        rdsOkPct = c.rds_ok_pct
        groups = c.groups
        blocksGood = c.blocks_good
        blocksCorrected = c.blocks_corrected
        blocksBad = c.blocks_bad
        blocksVoted = c.blocks_voted
        snrDb = c.snr_db
        levelDbfs = c.level_dbfs
        offsetHz = c.offset_hz
        carrier = c.carrier
        stereo = c.stereo
        stereoPct = c.stereo_pct
        pilotPct = c.pilot_pct
        pilotLocked = c.pilot_locked
        deviationHz = c.deviation_hz
        multipathPct = c.multipath_pct
        equaliser = c.equaliser
        squelched = c.squelched
        channelWidthHz = c.channel_width_hz
        mpxTopHz = c.mpx_top_hz
        signalTime = c.signal_time
        bandStartHz = c.band_start_hz
        bandEndHz = c.band_end_hz
        bandStepHz = c.band_step_hz
        bandFirstHz = c.band_first_hz
        tmcLtn = c.has_tmc_ltn ? c.tmc_ltn : nil
        tmcSid = c.has_tmc_sid ? c.tmc_sid : nil
        tmcAfi = c.tmc_afi
        tmcEnhancedMode = c.tmc_enhanced_mode
        tmcScope = c.tmc_scope
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FmFields, SDRKitCode> {
        var c = sdrkit_fm_fields()
        let code = sdrkit_fm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the FM sender. The measurements have no value while the stack does not transmit.
public struct FmTxFields {
    /// Whether the stack transmits now.
    public var transmitting: Bool
    /// The transmit frequency: the tuned frequency.
    public var txFreqHz: Double?
    /// The largest deviation of the multiplex over the last report.
    public var txPeakHz: Double?
    /// The power of the multiplex over the last 60 s against that of a sine of ±19 kHz deviation (ITU-R BS.412): 0 dB is the limit.
    public var txMpxPowerDb: Double?
    /// The RDS groups sent in this transmission.
    public var txGroups: Int64

    init(_ c: sdrkit_fm_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txPeakHz = c.has_tx_peak_hz ? c.tx_peak_hz : nil
        txMpxPowerDb = c.has_tx_mpx_power_db ? c.tx_mpx_power_db : nil
        txGroups = c.tx_groups
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FmTxFields, SDRKitCode> {
        var c = sdrkit_fm_tx_fields()
        let code = sdrkit_fm_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FmTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the alternative frequencies. The row key is the place in the list (`00`).
public struct FmAfRow {
    /// The row key.
    public var key: String
    /// A frequency.
    public var freqHz: Double

    init(_ c: sdrkit_fm_af_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
    }
}

/// A row of the tags of the RadioText, which the broadcast data publisher writes. The row key is the class name of the content type (`ITEM.TITLE`), or its code.
public struct FmLabelTagRow {
    /// The row key.
    public var key: String
    /// The content type code (0 to 127).
    public var code: Int64
    /// What people call the content type ("Title").
    public var type: String?
    /// The tagged text.
    public var text: String

    init(_ c: sdrkit_fm_label_tag_row) {
        key = sdrkitText(c.key)
        code = c.code
        type = c.has_type ? sdrkitText(c.type) : nil
        text = sdrkitText(c.text)
    }
}

/// A row of the open data applications. The row key is the AID in hexadecimal.
public struct FmOdaRow {
    /// The row key.
    public var key: String
    /// An open data application identifier.
    public var aid: Int64
    /// The group type that carries an application ("11A").
    public var group: String

    init(_ c: sdrkit_fm_oda_row) {
        key = sdrkitText(c.key)
        aid = c.aid
        group = sdrkitText(c.group)
    }
}

/// A row of the other networks. The row key is the PI in hexadecimal.
public struct FmEonRow {
    /// The row key.
    public var key: String
    /// The programme identification, four hexadecimal digits.
    public var pi: String
    /// A name.
    public var name: String
    /// Traffic programme.
    public var tp: Bool
    /// Traffic announcement.
    public var ta: Bool
    /// The programme type code.
    public var pty: Int64?
    /// Programme item number, `DD HH:MM`.
    public var pin: String?

    init(_ c: sdrkit_fm_eon_row) {
        key = sdrkitText(c.key)
        pi = sdrkitText(c.pi)
        name = sdrkitText(c.name)
        tp = c.tp
        ta = c.ta
        pty = c.has_pty ? c.pty : nil
        pin = c.has_pin ? sdrkitText(c.pin) : nil
    }
}

/// A row of the alternative frequencies of the other networks. The row key is `<PI>/<place>` (`4A21/00`).
public struct FmEonAfRow {
    /// The row key.
    public var key: String
    /// The PI of the other network a row belongs to.
    public var network: String
    /// A frequency.
    public var freqHz: Double

    init(_ c: sdrkit_fm_eon_af_row) {
        key = sdrkitText(c.key)
        network = sdrkitText(c.network)
        freqHz = c.freq_hz
    }
}

/// A row of the traffic messages. The row key is the place, oldest first (`00`).
public struct FmTmcRow {
    /// The row key.
    public var key: String
    /// A TMC event code.
    public var event: Int64
    /// A TMC location code.
    public var location: Int64
    /// The TMC direction bit.
    public var negativeDirection: Bool
    /// The TMC extent.
    public var extent: Int64
    /// The TMC diversion bit.
    public var diversion: Bool
    /// The TMC duration code.
    public var duration: Int64
    /// How often a message came.
    public var repeats: Int64
    /// When a message came first.
    public var first: Double
    /// When a message came last.
    public var last: Double
    /// When a message expires.
    public var expires: Double

    init(_ c: sdrkit_fm_tmc_row) {
        key = sdrkitText(c.key)
        event = c.event
        location = c.location
        negativeDirection = c.negative_direction
        extent = c.extent
        diversion = c.diversion
        duration = c.duration
        repeats = c.repeats
        first = c.first
        last = c.last
        expires = c.expires
    }
}

/// The result fields of the STL link, beside the reception fields of the FM components.
public struct StlFields {
    public enum Link: Int32 {
        case nothing = 0
        case composite = 1
        case mono = 2
        case digitalUnknown = 3
        case unknown = 4
    }
    /// What the link is. A change holds after two reports of 0.5 s in a row.
    public var link: Link?
    /// The peak deviation over the last 0.5 s, noise included. No value without a carrier.
    public var peakDeviationHz: Double?
    /// The peak deviation against the deviation of full modulation. No value without a carrier.
    public var modulationPct: Double?
    /// The pilot level, as deviation. No value while the pilot is not locked.
    public var pilotHz: Double?
    /// The channel width follows the link (the automatic width).
    public var widthAuto: Bool
    /// The spectrum of the capture reaches far enough past the channel to measure the floor. Without it the digital check is off.
    public var floorMeasured: Bool
    /// The width of the occupied region. No value while none stands out.
    public var occupiedWidthHz: Double?
    /// The mean of the occupied region over the floor.
    public var overFloorDb: Double?
    /// The flatness of the occupied region: its 10th percentile against its mean (0 dB is flat).
    public var flatnessDb: Double?
    /// The SNR of the SCA subcarrier at 67 kHz. No value while it is absent.
    public var sca67SnrDb: Double?
    /// The SNR of the SCA subcarrier at 92 kHz. No value while it is absent.
    public var sca92SnrDb: Double?
    /// The peak of the sound over the last 0.5 s. No value for silence.
    public var soundPeakDbfs: Double?
    /// The RMS of the sound over the last 0.5 s. No value for silence.
    public var soundRmsDbfs: Double?

    init(_ c: sdrkit_stl_fields) {
        link = Link(rawValue: c.link)
        peakDeviationHz = c.has_peak_deviation_hz ? c.peak_deviation_hz : nil
        modulationPct = c.has_modulation_pct ? c.modulation_pct : nil
        pilotHz = c.has_pilot_hz ? c.pilot_hz : nil
        widthAuto = c.width_auto
        floorMeasured = c.floor_measured
        occupiedWidthHz = c.has_occupied_width_hz ? c.occupied_width_hz : nil
        overFloorDb = c.has_over_floor_db ? c.over_floor_db : nil
        flatnessDb = c.has_flatness_db ? c.flatness_db : nil
        sca67SnrDb = c.has_sca67_snr_db ? c.sca67_snr_db : nil
        sca92SnrDb = c.has_sca92_snr_db ? c.sca92_snr_db : nil
        soundPeakDbfs = c.has_sound_peak_dbfs ? c.sound_peak_dbfs : nil
        soundRmsDbfs = c.has_sound_rms_dbfs ? c.sound_rms_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<StlFields, SDRKitCode> {
        var c = sdrkit_stl_fields()
        let code = sdrkit_stl_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(StlFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The measurements of the FM receiver and the RDS data: a part of [`FmFields`], and a result set of its own in a stack that runs the FM receiver without the band plan.
public struct StlRdsPart {
    /// The station name (PS): all 8 characters with trailing spaces, empty before the first name.
    public var station: String
    /// The name being collected (`_` for a segment not yet received).
    public var stationPartial: String
    /// Every RadioText segment has arrived.
    public var textComplete: Bool
    /// The programme type's name in the table in force.
    public var programme: String?
    /// The programme type code.
    public var pty: Int64?
    /// The programme identification, four hexadecimal digits.
    public var pi: String?
    /// RBDS call letters.
    public var callLetters: String?
    /// RBDS meanings apply.
    public var rbds: Bool
    /// Traffic programme.
    public var tp: Bool
    /// Traffic announcement.
    public var ta: Bool
    /// Music (true) or speech.
    public var music: Bool
    /// Programme type name (PTYN).
    public var ptyn: String
    /// Extended country code.
    public var ecc: Int64?
    /// Language code.
    public var language: Int64?
    /// Programme item number, `DD HH:MM`.
    public var pin: String?
    /// The station's clock time.
    public var clock: Double?
    /// The station's local time offset.
    public var clockOffset: Double?
    /// Enhanced RadioText.
    public var ert: String
    /// RDS block sync.
    public var rdsSync: Bool
    /// RDS usable blocks.
    public var rdsOkPct: Double
    /// Accepted RDS groups.
    public var groups: Int64
    /// Good RDS blocks.
    public var blocksGood: Int64
    /// Corrected RDS blocks (voted ones too).
    public var blocksCorrected: Int64
    /// Bad RDS blocks.
    public var blocksBad: Int64
    /// RDS blocks recovered by voting.
    public var blocksVoted: Int64
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The level of the channel.
    public var levelDbfs: Double
    /// The offset of the carrier.
    public var offsetHz: Double
    /// A carrier is present.
    public var carrier: Bool
    /// Stereo is reported.
    public var stereo: Bool
    /// The stereo blend while stereo is reported.
    public var stereoPct: Double
    /// The pilot injection, percent of 75 kHz.
    public var pilotPct: Double
    /// The pilot PLL is locked.
    public var pilotLocked: Bool
    /// The peak deviation.
    public var deviationHz: Double
    /// The multipath ripple.
    public var multipathPct: Double
    /// The equaliser is in use for the sound.
    public var equaliser: Bool
    /// The squelch is closed.
    public var squelched: Bool
    /// The channel filter width in use.
    public var channelWidthHz: Double
    /// The upper frequency of the `mpx` series.
    public var mpxTopHz: Double
    /// Signal decoded since the last reset.
    public var signalTime: Double
    /// TMC location table number.
    public var tmcLtn: Int64?
    /// TMC service identifier.
    public var tmcSid: Int64?
    /// TMC alternative-frequency indicator.
    public var tmcAfi: Bool
    /// TMC enhanced mode.
    public var tmcEnhancedMode: Bool
    /// TMC geographical scope bits (I, N, R, U).
    public var tmcScope: Int64

    init(_ c: sdrkit_stl_rds_part) {
        station = sdrkitText(c.station)
        stationPartial = sdrkitText(c.station_partial)
        textComplete = c.text_complete
        programme = c.has_programme ? sdrkitText(c.programme) : nil
        pty = c.has_pty ? c.pty : nil
        pi = c.has_pi ? sdrkitText(c.pi) : nil
        callLetters = c.has_call_letters ? sdrkitText(c.call_letters) : nil
        rbds = c.rbds
        tp = c.tp
        ta = c.ta
        music = c.music
        ptyn = sdrkitText(c.ptyn)
        ecc = c.has_ecc ? c.ecc : nil
        language = c.has_language ? c.language : nil
        pin = c.has_pin ? sdrkitText(c.pin) : nil
        clock = c.has_clock ? c.clock : nil
        clockOffset = c.has_clock_offset ? c.clock_offset : nil
        ert = sdrkitText(c.ert)
        rdsSync = c.rds_sync
        rdsOkPct = c.rds_ok_pct
        groups = c.groups
        blocksGood = c.blocks_good
        blocksCorrected = c.blocks_corrected
        blocksBad = c.blocks_bad
        blocksVoted = c.blocks_voted
        snrDb = c.snr_db
        levelDbfs = c.level_dbfs
        offsetHz = c.offset_hz
        carrier = c.carrier
        stereo = c.stereo
        stereoPct = c.stereo_pct
        pilotPct = c.pilot_pct
        pilotLocked = c.pilot_locked
        deviationHz = c.deviation_hz
        multipathPct = c.multipath_pct
        equaliser = c.equaliser
        squelched = c.squelched
        channelWidthHz = c.channel_width_hz
        mpxTopHz = c.mpx_top_hz
        signalTime = c.signal_time
        tmcLtn = c.has_tmc_ltn ? c.tmc_ltn : nil
        tmcSid = c.has_tmc_sid ? c.tmc_sid : nil
        tmcAfi = c.tmc_afi
        tmcEnhancedMode = c.tmc_enhanced_mode
        tmcScope = c.tmc_scope
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<StlRdsPart, SDRKitCode> {
        var c = sdrkit_stl_rds_part()
        let code = sdrkit_stl_rds_part_read(handle, &c)
        return code == SDRKIT_OK ? .success(StlRdsPart(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The radio text and whether its item is running.
public struct StlLabelFields {
    /// The radio text.
    public var text: String?
    /// The tagged item (a song) is playing.
    public var itemRunning: Bool?

    init(_ c: sdrkit_stl_label_fields) {
        text = c.has_text ? sdrkitText(c.text) : nil
        itemRunning = c.has_item_running ? c.item_running : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<StlLabelFields, SDRKitCode> {
        var c = sdrkit_stl_label_fields()
        let code = sdrkit_stl_label_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(StlLabelFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A held change of the link. The row key is the count of changes (`0001`).
public struct StlLinkRow {
    public enum Link: Int32 {
        case nothing = 0
        case composite = 1
        case mono = 2
        case digitalUnknown = 3
        case unknown = 4
    }
    /// The row key.
    public var key: String
    /// The signal time of the change.
    public var timeS: Double
    /// What the link became.
    public var link: Link?
    /// The details: pilot, peak deviation and offset of an FM link, or level and width of a signal that is not FM.
    public var text: String
    /// The occupied region over the floor. No value without a region.
    public var overFloorDb: Double?

    init(_ c: sdrkit_stl_link_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        link = Link(rawValue: c.link)
        text = sdrkitText(c.text)
        overFloorDb = c.has_over_floor_db ? c.over_floor_db : nil
    }
}

/// The fields of the two events of the FM receiver.
public struct FmStationEvent {
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
    /// The PI, four hexadecimal digits.
    public var source: String
    /// The station name, or the RadioText.
    public var text: String
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_fm_station_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FmStationEvent, SDRKitCode> {
        var c = sdrkit_fm_station_event()
        let code = sdrkit_fm_station_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FmStationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the two events of the FM receiver.
public struct FmTextEvent {
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
    /// The PI, four hexadecimal digits.
    public var source: String
    /// The station name, or the RadioText.
    public var text: String
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_fm_text_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FmTextEvent, SDRKitCode> {
        var c = sdrkit_fm_text_event()
        let code = sdrkit_fm_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FmTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of the link.
public struct StlLinkEvent {
    public enum Link: Int32 {
        case nothing = 0
        case composite = 1
        case mono = 2
        case digitalUnknown = 3
        case unknown = 4
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the link became.
    public var link: Link?
    /// The details, as in the table of changes.
    public var text: String
    /// The occupied region over the floor. No value without a region.
    public var overFloorDb: Double?

    init(_ c: sdrkit_stl_link_event) {
        time = c.time
        freqHz = c.freq_hz
        link = Link(rawValue: c.link)
        text = sdrkitText(c.text)
        overFloorDb = c.has_over_floor_db ? c.over_floor_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<StlLinkEvent, SDRKitCode> {
        var c = sdrkit_stl_link_event()
        let code = sdrkit_stl_link_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(StlLinkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum FmTables {
    /// The changed rows of the table "af" of the stack "fm", newest first, at most `capacity` rows.
    public static func fmAf(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[FmAfRow], SDRKitCode> {
        var rows = [sdrkit_fm_af_row](repeating: sdrkit_fm_af_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_af_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmAfRow.init))
    }
    /// The changed rows of the table "label_tags" of the stack "fm", newest first, at most `capacity` rows.
    public static func fmLabelTags(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[FmLabelTagRow], SDRKitCode> {
        var rows = [sdrkit_fm_label_tag_row](repeating: sdrkit_fm_label_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_label_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmLabelTagRow.init))
    }
    /// The changed rows of the table "oda" of the stack "fm", newest first, at most `capacity` rows.
    public static func fmOda(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[FmOdaRow], SDRKitCode> {
        var rows = [sdrkit_fm_oda_row](repeating: sdrkit_fm_oda_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_oda_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmOdaRow.init))
    }
    /// The changed rows of the table "eon" of the stack "fm", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func fmEon(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[FmEonRow], SDRKitCode> {
        var rows = [sdrkit_fm_eon_row](repeating: sdrkit_fm_eon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_eon_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmEonRow.init))
    }
    /// The changed rows of the table "eon_af" of the stack "fm", newest first, at most `capacity` rows.
    public static func fmEonAf(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[FmEonAfRow], SDRKitCode> {
        var rows = [sdrkit_fm_eon_af_row](repeating: sdrkit_fm_eon_af_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_eon_af_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmEonAfRow.init))
    }
    /// The changed rows of the table "tmc" of the stack "fm", newest first, at most `capacity` rows.
    public static func fmTmc(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[FmTmcRow], SDRKitCode> {
        var rows = [sdrkit_fm_tmc_row](repeating: sdrkit_fm_tmc_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_tmc_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmTmcRow.init))
    }
    /// The changed rows of the table "links" of the stack "stl", newest first, at most `capacity` rows.
    public static func stlLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[StlLinkRow], SDRKitCode> {
        var rows = [sdrkit_stl_link_row](repeating: sdrkit_stl_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(StlLinkRow.init))
    }
    /// The changed rows of the table "af" of the stack "stl", newest first, at most `capacity` rows.
    public static func stlAf(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[FmAfRow], SDRKitCode> {
        var rows = [sdrkit_fm_af_row](repeating: sdrkit_fm_af_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_af_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmAfRow.init))
    }
    /// The changed rows of the table "label_tags" of the stack "stl", newest first, at most `capacity` rows.
    public static func stlLabelTags(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[FmLabelTagRow], SDRKitCode> {
        var rows = [sdrkit_fm_label_tag_row](repeating: sdrkit_fm_label_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_label_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmLabelTagRow.init))
    }
    /// The changed rows of the table "oda" of the stack "stl", newest first, at most `capacity` rows.
    public static func stlOda(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[FmOdaRow], SDRKitCode> {
        var rows = [sdrkit_fm_oda_row](repeating: sdrkit_fm_oda_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_oda_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmOdaRow.init))
    }
    /// The changed rows of the table "eon" of the stack "stl", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func stlEon(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[FmEonRow], SDRKitCode> {
        var rows = [sdrkit_fm_eon_row](repeating: sdrkit_fm_eon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_eon_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmEonRow.init))
    }
    /// The changed rows of the table "eon_af" of the stack "stl", newest first, at most `capacity` rows.
    public static func stlEonAf(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[FmEonAfRow], SDRKitCode> {
        var rows = [sdrkit_fm_eon_af_row](repeating: sdrkit_fm_eon_af_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_eon_af_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmEonAfRow.init))
    }
    /// The changed rows of the table "tmc" of the stack "stl", newest first, at most `capacity` rows.
    public static func stlTmc(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[FmTmcRow], SDRKitCode> {
        var rows = [sdrkit_fm_tmc_row](repeating: sdrkit_fm_tmc_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_stl_tmc_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmTmcRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum FmCommands {
    /// Clear. Forgets the station's name, text and the other RDS data.
    public static func fmClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fm_clear(handle)
        return sdrkitResult(code)
    }
    /// Start sending. Sends the FM broadcast signal without end: the audio input in stereo, with the pilot and RDS from the transmit settings, until stop. The setting tx_pi must be set. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func fmStart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fm_start(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission at once, with the fall of the carrier.
    public static func fmStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fm_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the station's name, text and the other RDS data.
    public static func stlClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_stl_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum FmSettings {
    /// The options of the setting "stereo" of the stack "fm".
    public enum FmStereo: Int32 {
        case auto = 0
        case mono = 1
        case stereo = 2
    }
    /// Stereo.
    public static func fmStereo(_ handle: SDRKIT_HANDLE) -> Result<FmStereo, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "stereo", &v)
        return code == SDRKIT_OK ? .success(FmStereo(rawValue: v) ?? FmStereo(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmStereo(_ handle: SDRKIT_HANDLE, _ value: FmStereo) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "stereo", value.rawValue))
    }
    /// Force mono.
    public static func fmForceMono(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "force_mono", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmForceMono(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "force_mono", value))
    }
    /// The options of the setting "deemphasis" of the stack "fm".
    public enum FmDeemphasis: Int32 {
        case off = 0
        case n50Us = 1
        case n75Us = 2
    }
    /// De-emphasis.
    public static func fmDeemphasis(_ handle: SDRKIT_HANDLE) -> Result<FmDeemphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deemphasis", &v)
        return code == SDRKIT_OK ? .success(FmDeemphasis(rawValue: v) ?? FmDeemphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmDeemphasis(_ handle: SDRKIT_HANDLE, _ value: FmDeemphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deemphasis", value.rawValue))
    }
    /// Squelch.
    public static func fmSquelch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmSquelch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch", value))
    }
    /// The options of the setting "audio_bandwidth" of the stack "fm".
    public enum FmAudioBandwidth: Int32 {
        case full = 0
        case n12Khz = 1
        case n8Khz = 2
        case n5Khz = 3
    }
    /// Audio bandwidth.
    public static func fmAudioBandwidth(_ handle: SDRKIT_HANDLE) -> Result<FmAudioBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "audio_bandwidth", &v)
        return code == SDRKIT_OK ? .success(FmAudioBandwidth(rawValue: v) ?? FmAudioBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmAudioBandwidth(_ handle: SDRKIT_HANDLE, _ value: FmAudioBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "audio_bandwidth", value.rawValue))
    }
    /// Bass.
    public static func fmBass(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "bass", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmBass(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "bass", value))
    }
    /// Treble.
    public static func fmTreble(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "treble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTreble(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "treble", value))
    }
    /// Warm.
    public static func fmWarm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "warm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmWarm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "warm", value))
    }
    /// Channel width.
    public static func fmChannelWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmChannelWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_width", value))
    }
    /// The options of the setting "channel_filter" of the stack "fm".
    public enum FmChannelFilter: Int32 {
        case standard = 0
        case wide = 1
        case narrow = 2
        case custom = 3
    }
    /// Channel filter.
    public static func fmChannelFilter(_ handle: SDRKIT_HANDLE) -> Result<FmChannelFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel_filter", &v)
        return code == SDRKIT_OK ? .success(FmChannelFilter(rawValue: v) ?? FmChannelFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmChannelFilter(_ handle: SDRKIT_HANDLE, _ value: FmChannelFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel_filter", value.rawValue))
    }
    /// Band start.
    public static func fmBandLowHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "band_low_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmBandLowHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "band_low_hz", value))
    }
    /// Band end.
    public static func fmBandHighHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "band_high_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmBandHighHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "band_high_hz", value))
    }
    /// First channel.
    public static func fmFirstHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "first_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmFirstHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "first_hz", value))
    }
    /// Channel step.
    public static func fmRasterHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "raster_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRasterHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "raster_hz", value))
    }
    /// Scan from.
    public static func fmScanLow(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_low", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmScanLow(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_low", value))
    }
    /// Scan to.
    public static func fmScanHigh(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_high", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmScanHigh(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_high", value))
    }
    /// Scan for full RDS.
    public static func fmScanFullRds(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan_full_rds", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmScanFullRds(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan_full_rds", value))
    }
    /// Station finder.
    public static func fmStationFinder(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "station_finder", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmStationFinder(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "station_finder", value))
    }
    /// The options of the setting "rds_variant" of the stack "fm".
    public enum FmRdsVariant: Int32 {
        case auto = 0
        case rds = 1
        case rbds = 2
    }
    /// RDS meanings.
    public static func fmRdsVariant(_ handle: SDRKIT_HANDLE) -> Result<FmRdsVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rds_variant", &v)
        return code == SDRKIT_OK ? .success(FmRdsVariant(rawValue: v) ?? FmRdsVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRdsVariant(_ handle: SDRKIT_HANDLE, _ value: FmRdsVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rds_variant", value.rawValue))
    }
    /// The options of the setting "rds_correction" of the stack "fm".
    public enum FmRdsCorrection: Int32 {
        case off = 0
        case shortBursts = 1
        case full = 2
    }
    /// RDS correction.
    public static func fmRdsCorrection(_ handle: SDRKIT_HANDLE) -> Result<FmRdsCorrection, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rds_correction", &v)
        return code == SDRKIT_OK ? .success(FmRdsCorrection(rawValue: v) ?? FmRdsCorrection(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRdsCorrection(_ handle: SDRKIT_HANDLE, _ value: FmRdsCorrection) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rds_correction", value.rawValue))
    }
    /// RDS voting.
    public static func fmRdsVoting(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "rds_voting", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRdsVoting(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "rds_voting", value))
    }
    /// The options of the setting "rds_charset" of the stack "fm".
    public enum FmRdsCharset: Int32 {
        case ebuLatin = 0
        case autoGb2312 = 1
        case gb2312 = 2
    }
    /// RDS characters.
    public static func fmRdsCharset(_ handle: SDRKIT_HANDLE) -> Result<FmRdsCharset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rds_charset", &v)
        return code == SDRKIT_OK ? .success(FmRdsCharset(rawValue: v) ?? FmRdsCharset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRdsCharset(_ handle: SDRKIT_HANDLE, _ value: FmRdsCharset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rds_charset", value.rawValue))
    }
    /// PI code.
    public static func fmTxPi(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "tx_pi", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxPi(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "tx_pi", $0) })
    }
    /// Station name (PS).
    public static func fmTxPs(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "tx_ps", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxPs(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "tx_ps", $0) })
    }
    /// Radio text.
    public static func fmTxRt(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "tx_rt", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxRt(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "tx_rt", $0) })
    }
    /// Programme type.
    public static func fmTxPty(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_pty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxPty(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_pty", value))
    }
    /// Traffic programme.
    public static func fmTxTp(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_tp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxTp(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_tp", value))
    }
    /// Traffic announcement now.
    public static func fmTxTa(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_ta", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxTa(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_ta", value))
    }
    /// Music.
    public static func fmTxMusic(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_music", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxMusic(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_music", value))
    }
    /// Alternative frequencies.
    public static func fmTxAf(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "tx_af", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxAf(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "tx_af", $0) })
    }
    /// Send the clock time.
    public static func fmTxCt(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_ct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxCt(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_ct", value))
    }
    /// Local time offset.
    public static func fmTxCtOffset(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_ct_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxCtOffset(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_ct_offset", value))
    }
    /// Stereo.
    public static func fmTxStereo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_stereo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxStereo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_stereo", value))
    }
    /// The options of the setting "tx_preemphasis" of the stack "fm".
    public enum FmTxPreemphasis: Int32 {
        case none = 0
        case n50Us = 1
        case n75Us = 2
    }
    /// Pre-emphasis.
    public static func fmTxPreemphasis(_ handle: SDRKIT_HANDLE) -> Result<FmTxPreemphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_preemphasis", &v)
        return code == SDRKIT_OK ? .success(FmTxPreemphasis(rawValue: v) ?? FmTxPreemphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxPreemphasis(_ handle: SDRKIT_HANDLE, _ value: FmTxPreemphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_preemphasis", value.rawValue))
    }
    /// Peak deviation.
    public static func fmTxDeviation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_deviation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxDeviation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_deviation", value))
    }
    /// Pilot level.
    public static func fmTxPilot(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_pilot", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxPilot(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_pilot", value))
    }
    /// RDS level.
    public static func fmTxRdsLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_rds_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxRdsLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_rds_level", value))
    }
    /// Audio limiter.
    public static func fmTxLimiter(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_limiter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTxLimiter(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_limiter", value))
    }
    /// The options of the setting "stereo" of the stack "stl".
    public enum StlStereo: Int32 {
        case auto = 0
        case mono = 1
        case stereo = 2
    }
    /// Stereo.
    public static func stlStereo(_ handle: SDRKIT_HANDLE) -> Result<StlStereo, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "stereo", &v)
        return code == SDRKIT_OK ? .success(StlStereo(rawValue: v) ?? StlStereo(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlStereo(_ handle: SDRKIT_HANDLE, _ value: StlStereo) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "stereo", value.rawValue))
    }
    /// The options of the setting "deemphasis" of the stack "stl".
    public enum StlDeemphasis: Int32 {
        case off = 0
        case n50Us = 1
        case n75Us = 2
    }
    /// De-emphasis.
    public static func stlDeemphasis(_ handle: SDRKIT_HANDLE) -> Result<StlDeemphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deemphasis", &v)
        return code == SDRKIT_OK ? .success(StlDeemphasis(rawValue: v) ?? StlDeemphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlDeemphasis(_ handle: SDRKIT_HANDLE, _ value: StlDeemphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deemphasis", value.rawValue))
    }
    /// Full deviation.
    public static func stlDeviation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "deviation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlDeviation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "deviation", value))
    }
    /// Automatic width.
    public static func stlAutoWidth(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlAutoWidth(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_width", value))
    }
    /// Channel width.
    public static func stlWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "width", value))
    }
    /// The options of the setting "rds_variant" of the stack "stl".
    public enum StlRdsVariant: Int32 {
        case auto = 0
        case rds = 1
        case rbds = 2
    }
    /// RDS meanings.
    public static func stlRdsVariant(_ handle: SDRKIT_HANDLE) -> Result<StlRdsVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rds_variant", &v)
        return code == SDRKIT_OK ? .success(StlRdsVariant(rawValue: v) ?? StlRdsVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRdsVariant(_ handle: SDRKIT_HANDLE, _ value: StlRdsVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rds_variant", value.rawValue))
    }
    /// The options of the setting "rds_correction" of the stack "stl".
    public enum StlRdsCorrection: Int32 {
        case off = 0
        case shortBursts = 1
        case full = 2
    }
    /// RDS correction.
    public static func stlRdsCorrection(_ handle: SDRKIT_HANDLE) -> Result<StlRdsCorrection, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rds_correction", &v)
        return code == SDRKIT_OK ? .success(StlRdsCorrection(rawValue: v) ?? StlRdsCorrection(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRdsCorrection(_ handle: SDRKIT_HANDLE, _ value: StlRdsCorrection) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rds_correction", value.rawValue))
    }
    /// RDS voting.
    public static func stlRdsVoting(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "rds_voting", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRdsVoting(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "rds_voting", value))
    }
    /// The options of the setting "rds_charset" of the stack "stl".
    public enum StlRdsCharset: Int32 {
        case ebuLatin = 0
        case autoGb2312 = 1
        case gb2312 = 2
    }
    /// RDS characters.
    public static func stlRdsCharset(_ handle: SDRKIT_HANDLE) -> Result<StlRdsCharset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rds_charset", &v)
        return code == SDRKIT_OK ? .success(StlRdsCharset(rawValue: v) ?? StlRdsCharset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRdsCharset(_ handle: SDRKIT_HANDLE, _ value: StlRdsCharset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rds_charset", value.rawValue))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum FmGeneratorSettings {
    /// Stereo.
    public static func fmStereo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_stereo", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmStereo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_stereo", value))
    }
    /// Left tone.
    public static func fmLeftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_left_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmLeftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_left_hz", value))
    }
    /// Left level.
    public static func fmLeftAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_left_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmLeftAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_left_amp", value))
    }
    /// Right tone.
    public static func fmRightHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_right_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRightHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_right_hz", value))
    }
    /// Right level.
    public static func fmRightAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_right_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRightAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_right_amp", value))
    }
    /// Pilot.
    public static func fmPilot(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pilot", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmPilot(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pilot", value))
    }
    /// RDS.
    public static func fmRds(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_rds", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRds(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_rds", value))
    }
    /// RDS injection.
    public static func fmRdsKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rds_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRdsKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rds_khz", value))
    }
    /// Station name.
    public static func fmPs(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ps", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmPs(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ps", $0) })
    }
    /// Radio text.
    public static func fmRt(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_rt", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmRt(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_rt", $0) })
    }
    /// PI code.
    public static func fmPi(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_pi", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmPi(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_pi", $0) })
    }
    /// Programme type.
    public static func fmPty(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_pty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmPty(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_pty", value))
    }
    /// Traffic programme.
    public static func fmTp(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_tp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTp(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_tp", value))
    }
    /// Traffic announcement.
    public static func fmTa(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_ta", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmTa(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_ta", value))
    }
    /// Music.
    public static func fmMusic(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_music", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMusic(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_music", value))
    }
    /// The options of the setting "gen_preemphasis" of the stack "fm".
    public enum FmPreemphasis: Int32 {
        case none = 0
        case n50Us = 1
        case n75Us = 2
    }
    /// Pre-emphasis.
    public static func fmPreemphasis(_ handle: SDRKIT_HANDLE) -> Result<FmPreemphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_preemphasis", &v)
        return code == SDRKIT_OK ? .success(FmPreemphasis(rawValue: v) ?? FmPreemphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmPreemphasis(_ handle: SDRKIT_HANDLE, _ value: FmPreemphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_preemphasis", value.rawValue))
    }
    /// Extra RDS groups.
    public static func fmExtraGroups(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_extra_groups", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFmExtraGroups(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_extra_groups", $0) })
    }
    /// The options of the setting "gen_link" of the stack "stl".
    public enum StlLink: Int32 {
        case composite = 0
        case mono = 1
        case digital = 2
    }
    /// Link.
    public static func stlLink(_ handle: SDRKIT_HANDLE) -> Result<StlLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_link", &v)
        return code == SDRKIT_OK ? .success(StlLink(rawValue: v) ?? StlLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlLink(_ handle: SDRKIT_HANDLE, _ value: StlLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_link", value.rawValue))
    }
    /// Full deviation.
    public static func stlDeviation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlDeviation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation", value))
    }
    /// SCA subcarriers.
    public static func stlSca(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_sca", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlSca(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_sca", value))
    }
    /// Digital width.
    public static func stlWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_width", value))
    }
    /// Left tone.
    public static func stlLeftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_left_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlLeftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_left_hz", value))
    }
    /// Left level.
    public static func stlLeftAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_left_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlLeftAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_left_amp", value))
    }
    /// Right tone.
    public static func stlRightHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_right_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRightHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_right_hz", value))
    }
    /// Right level.
    public static func stlRightAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_right_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRightAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_right_amp", value))
    }
    /// RDS.
    public static func stlRds(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_rds", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRds(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_rds", value))
    }
    /// Station name.
    public static func stlPs(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ps", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setStlPs(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ps", $0) })
    }
    /// Radio text.
    public static func stlRt(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_rt", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setStlRt(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_rt", $0) })
    }
    /// PI code.
    public static func stlPi(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_pi", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setStlPi(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_pi", $0) })
    }
    /// Programme type.
    public static func stlPty(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_pty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setStlPty(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_pty", value))
    }
    /// The options of the setting "gen_preemphasis" of the stack "stl".
    public enum StlPreemphasis: Int32 {
        case none = 0
        case n50Us = 1
        case n75Us = 2
    }
    /// Pre-emphasis.
    public static func stlPreemphasis(_ handle: SDRKIT_HANDLE) -> Result<StlPreemphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_preemphasis", &v)
        return code == SDRKIT_OK ? .success(StlPreemphasis(rawValue: v) ?? StlPreemphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setStlPreemphasis(_ handle: SDRKIT_HANDLE, _ value: StlPreemphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_preemphasis", value.rawValue))
    }
}

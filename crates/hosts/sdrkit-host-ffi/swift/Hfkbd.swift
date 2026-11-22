// sdrkit: the typed interface of the stack family "hfkbd", version 2.8. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum HfkbdInterface {
    public static let major = 2
    public static let minor = 8
}

/// The levels that the front end measures. A level has no value before the first input.
public struct HfkbdFrontFields {
    /// The power of the input.
    public var levelDbfs: Double?
    /// The power of the 200 to 3800 Hz sideband before the AGC.
    public var audioDbfs: Double?

    init(_ c: sdrkit_hfkbd_front_fields) {
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        audioDbfs = c.has_audio_dbfs ? c.audio_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_hfkbd_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of RTTY.
public struct RttyFields {
    /// The decoded text.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?
    /// The mark tone. It has no value while searching.
    public var markHz: Double?
    /// The space tone. It has no value while searching.
    public var spaceHz: Double?
    /// The speed in use.
    public var symbolRateHz: Double
    /// The shift in use.
    public var shiftHz: Double
    /// The mark is the upper tone.
    public var reverse: Bool
    /// Characters decoded.
    public var characters: Int64
    /// Characters that the framer dropped.
    public var framingErrors: Int64
    /// How clean the decisions are, 0 to 100.
    public var qualityPct: Double
    /// Auto is on.
    public var auto: Bool
    /// Auto has applied a guess that still decodes.
    public var autoLocked: Bool
    /// The state of Auto: 0 not asked, 1 waiting for audio or running, 2 applied, 3 nothing found.
    public var autoState: Int64

    init(_ c: sdrkit_rtty_fields) {
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        markHz = c.has_mark_hz ? c.mark_hz : nil
        spaceHz = c.has_space_hz ? c.space_hz : nil
        symbolRateHz = c.symbol_rate_hz
        shiftHz = c.shift_hz
        reverse = c.reverse
        characters = c.characters
        framingErrors = c.framing_errors
        qualityPct = c.quality_pct
        auto = c.auto_
        autoLocked = c.auto_locked
        autoState = c.auto_state
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RttyFields, SDRKitCode> {
        var c = sdrkit_rtty_fields()
        let code = sdrkit_rtty_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RttyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct HfkbdDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_hfkbd_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfkbdDecoderFields, SDRKitCode> {
        var c = sdrkit_hfkbd_decoder_fields()
        let code = sdrkit_hfkbd_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of a keyboard sender.
public struct HfkbdKeyboardTxFields {
    /// Whether a burst is on air: text or the tune carrier.
    public var txSending: Bool
    /// The characters of the burst on air coded into the signal so far.
    public var txSent: Int64
    /// The characters waiting to be sent.
    public var txWaiting: Int64

    init(_ c: sdrkit_hfkbd_keyboard_tx_fields) {
        txSending = c.tx_sending
        txSent = c.tx_sent
        txWaiting = c.tx_waiting
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfkbdKeyboardTxFields, SDRKitCode> {
        var c = sdrkit_hfkbd_keyboard_tx_fields()
        let code = sdrkit_hfkbd_keyboard_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdKeyboardTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the stations heard. The row key is the callsign.
public struct HfkbdStationRow {
    /// The row key.
    public var key: String
    /// The callsign.
    public var callsign: String
    /// Where it was last heard.
    public var freqHz: Double
    /// Its SNR when last heard.
    public var snrDb: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last time heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_hfkbd_station_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        freqHz = c.freq_hz
        snrDb = c.snr_db
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A guess of the last analysis of Auto. The row key is the rank, from 1.
public struct RttyGuessRow {
    /// The row key.
    public var key: String
    /// The lower tone.
    public var lowerHz: Double
    /// The shift.
    public var shiftHz: Double
    /// The speed.
    public var symbolRateHz: Double
    /// The mark is the upper tone.
    public var reverse: Bool
    /// The text score less 2.5 for each framing error.
    public var score: Double
    /// The characters of the trial.
    public var characters: Int64
    /// The framing errors of the trial.
    public var framingErrors: Int64

    init(_ c: sdrkit_rtty_guess_row) {
        key = sdrkitText(c.key)
        lowerHz = c.lower_hz
        shiftHz = c.shift_hz
        symbolRateHz = c.symbol_rate_hz
        reverse = c.reverse
        score = c.score
        characters = c.characters
        framingErrors = c.framing_errors
    }
}

/// The result fields of PSK.
public struct PskFields {
    public enum Modulation: Int32 {
        case bPSK31 = 0
        case bPSK63 = 1
        case qPSK31 = 2
        case qPSK63 = 3
        case bPSK125 = 4
        case bPSK250 = 5
        case qPSK125 = 6
    }
    /// The modulation decoded.
    public var modulation: Modulation?
    /// The noise floor of the window.
    public var floorDb: Double
    /// The id of the signal in focus, if one is.
    public var focus: Int64?
    /// Characters decoded.
    public var characters: Int64
    /// Characters that the code table does not have.
    public var errors: Int64
    /// Auto is on.
    public var auto: Bool
    /// Auto has applied a guess that still decodes.
    public var autoLocked: Bool
    /// The modulation that Auto found. It has no value before Auto finds one.
    public var autoModulation: String?

    init(_ c: sdrkit_psk_fields) {
        modulation = Modulation(rawValue: c.modulation)
        floorDb = c.floor_db
        focus = c.has_focus ? c.focus : nil
        characters = c.characters
        errors = c.errors
        auto = c.auto_
        autoLocked = c.auto_locked
        autoModulation = c.has_auto_modulation ? sdrkitText(c.auto_modulation) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PskFields, SDRKitCode> {
        var c = sdrkit_psk_fields()
        let code = sdrkit_psk_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PskFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A PSK signal heard, or one gone whose text stays. The row key is the signal id. The rows of the gone signals stay over a change of frequency: they are the log of the decoded texts.
public struct PskSignalRow {
    /// The row key.
    public var key: String
    /// The signal id.
    public var id: Int64
    /// The audio frequency.
    public var offsetHz: Double
    /// The correction since it was found.
    public var afcHz: Double
    /// The SNR.
    public var snrDb: Double
    /// How clean the decisions are, 0 to 100.
    public var qualityPct: Double
    /// Text is coming.
    public var dcd: Bool
    /// The user picked it.
    public var manual: Bool
    /// The signal time when it was first heard.
    public var firstS: Double
    /// The signal time when it was last heard.
    public var lastS: Double
    /// Characters decoded.
    public var characters: Int64
    /// The decoded text.
    public var text: String
    /// It is no longer heard.
    public var gone: Bool

    init(_ c: sdrkit_psk_signal_row) {
        key = sdrkitText(c.key)
        id = c.id
        offsetHz = c.offset_hz
        afcHz = c.afc_hz
        snrDb = c.snr_db
        qualityPct = c.quality_pct
        dcd = c.dcd
        manual = c.manual
        firstS = c.first_s
        lastS = c.last_s
        characters = c.characters
        text = sdrkitText(c.text)
        gone = c.gone
    }
}

/// The result fields of the CW skimmer. The fields of the sender have no value without a signal in focus.
public struct CwFields {
    public enum Playing: Int32 {
        case nothing = 0
        case signal = 1
        case dial = 2
    }
    /// The width of the band watched, centred on the dial.
    public var spanHz: Double
    /// The noise floor of the window.
    public var floorDb: Double
    /// The id of the signal in focus, if one is.
    public var focus: Int64?
    /// Characters decoded.
    public var characters: Int64
    /// Characters that the code table does not have.
    public var errors: Int64
    /// The dot of the signal in focus.
    public var dotMs: Double?
    /// Its dash as dots.
    public var dashRatio: Double?
    /// Its character gap.
    public var charGap: Double?
    /// Its word gap.
    public var wordGap: Double?
    /// How steady its keying is.
    public var steadinessPct: Double?
    /// Its speed.
    public var wpm: Double?
    /// The signals in the table that are heard now.
    public var signals: Int64
    /// The signal time now, for the age of a signal's last-heard time.
    public var nowS: Double
    /// The id of the signal listened to (the row key in the signals table): the focus, else the strongest signal in the table. Absent while there is none, also while the audio plays the dial.
    public var listened: Int64?
    /// What the audio plays: nothing, the signal listened to, or the dial while there is none.
    public var playing: Playing?
    /// The radio frequency that the audio plays: that of the signal listened to, or the dial. Absent while it plays nothing.
    public var listenedHz: Double?

    init(_ c: sdrkit_cw_fields) {
        spanHz = c.span_hz
        floorDb = c.floor_db
        focus = c.has_focus ? c.focus : nil
        characters = c.characters
        errors = c.errors
        dotMs = c.has_dot_ms ? c.dot_ms : nil
        dashRatio = c.has_dash_ratio ? c.dash_ratio : nil
        charGap = c.has_char_gap ? c.char_gap : nil
        wordGap = c.has_word_gap ? c.word_gap : nil
        steadinessPct = c.has_steadiness_pct ? c.steadiness_pct : nil
        wpm = c.has_wpm ? c.wpm : nil
        signals = c.signals
        nowS = c.now_s
        listened = c.has_listened ? c.listened : nil
        playing = Playing(rawValue: c.playing)
        listenedHz = c.has_listened_hz ? c.listened_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CwFields, SDRKitCode> {
        var c = sdrkit_cw_fields()
        let code = sdrkit_cw_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CwFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A CW signal heard, or one gone whose text stays. The row key is the signal id.
public struct CwSignalRow {
    /// The row key.
    public var key: String
    /// The signal id.
    public var id: Int64
    /// The audio frequency.
    public var offsetHz: Double
    /// The speed.
    public var wpm: Double
    /// The SNR.
    public var snrDb: Double
    /// The key is down.
    public var keyDown: Bool
    /// The speed is known.
    public var locked: Bool
    /// The sender spaces as Farnsworth.
    public var farnsworth: Bool
    /// The user picked it.
    public var manual: Bool
    /// The signal time when it was first heard.
    public var firstS: Double
    /// The signal time when it was last heard.
    public var lastS: Double
    /// Characters decoded.
    public var characters: Int64
    /// The decoded text.
    public var text: String
    /// The confidence of each character, 0 to 9.
    public var confidence: String
    /// The other readings of unclear characters.
    public var alternatives: String
    /// It is no longer heard.
    public var gone: Bool
    /// Characters that the code table does not have.
    public var errors: Int64
    /// The radio frequency of the carrier: the dial plus the offset.
    public var freqHz: Double
    /// The level of the carrier while the key is down, relative to full scale.
    public var levelDbfs: Double

    init(_ c: sdrkit_cw_signal_row) {
        key = sdrkitText(c.key)
        id = c.id
        offsetHz = c.offset_hz
        wpm = c.wpm
        snrDb = c.snr_db
        keyDown = c.key_down
        locked = c.locked
        farnsworth = c.farnsworth
        manual = c.manual
        firstS = c.first_s
        lastS = c.last_s
        characters = c.characters
        text = sdrkitText(c.text)
        confidence = sdrkitText(c.confidence)
        alternatives = sdrkitText(c.alternatives)
        gone = c.gone
        errors = c.errors
        freqHz = c.freq_hz
        levelDbfs = c.level_dbfs
    }
}

/// The result fields of the keyboard stacks of many tones: MFSK, Olivia, Contestia, DominoEX and THOR.
public struct HfkbdKeyboardFields {
    /// The variant in use.
    public var variant: String
    /// The audio centre now.
    public var centreHz: Double
    /// The tuning error found.
    public var offsetHz: Double
    /// The decoder's own SNR.
    public var snrDb: Double
    /// The decoded text.
    public var text: String
    /// Characters decoded.
    public var characters: Int64
    /// The newest callsign of the text, if it has one.
    public var callsign: String?
    /// How clean the decisions are, 0 to 100.
    public var qualityPct: Double

    init(_ c: sdrkit_hfkbd_keyboard_fields) {
        variant = sdrkitText(c.variant)
        centreHz = c.centre_hz
        offsetHz = c.offset_hz
        snrDb = c.snr_db
        text = sdrkitText(c.text)
        characters = c.characters
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        qualityPct = c.quality_pct
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfkbdKeyboardFields, SDRKitCode> {
        var c = sdrkit_hfkbd_keyboard_fields()
        let code = sdrkit_hfkbd_keyboard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdKeyboardFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The picture being received, or else the newest one. Each field of a picture has no value before the first.
public struct HfkbdPictureFields {
    /// The number of the picture.
    public var pictureId: Int64?
    /// Its width.
    public var pictureWidth: Int64?
    /// Its height.
    public var pictureHeight: Int64?
    /// The rows received.
    public var pictureRows: Int64?
    /// How much of it is in.
    public var pictureProgressPct: Double?
    /// It is complete.
    public var pictureComplete: Bool?
    /// It is in colour.
    public var pictureColour: Bool?
    /// Its mode.
    public var pictureMode: String?
    /// The pictures finished.
    public var pictures: Int64

    init(_ c: sdrkit_hfkbd_picture_fields) {
        pictureId = c.has_picture_id ? c.picture_id : nil
        pictureWidth = c.has_picture_width ? c.picture_width : nil
        pictureHeight = c.has_picture_height ? c.picture_height : nil
        pictureRows = c.has_picture_rows ? c.picture_rows : nil
        pictureProgressPct = c.has_picture_progress_pct ? c.picture_progress_pct : nil
        pictureComplete = c.has_picture_complete ? c.picture_complete : nil
        pictureColour = c.has_picture_colour ? c.picture_colour : nil
        pictureMode = c.has_picture_mode ? sdrkitText(c.picture_mode) : nil
        pictures = c.pictures
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfkbdPictureFields, SDRKitCode> {
        var c = sdrkit_hfkbd_picture_fields()
        let code = sdrkit_hfkbd_picture_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdPictureFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of Hellschreiber.
public struct HellFields {
    /// The variant in use.
    public var variant: String
    /// The audio centre now.
    public var centreHz: Double
    /// The pixel columns received.
    public var columns: Int64
    /// The SNR.
    public var snrDb: Double

    init(_ c: sdrkit_hell_fields) {
        variant = sdrkitText(c.variant)
        centreHz = c.centre_hz
        columns = c.columns
        snrDb = c.snr_db
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HellFields, SDRKitCode> {
        var c = sdrkit_hell_fields()
        let code = sdrkit_hell_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HellFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of MT63.
public struct Mt63Fields {
    public enum Interleave: Int32 {
        case auto = 0
        case short = 1
        case long = 2
    }
    /// The width of the signal in use.
    public var bandwidthHz: Double
    /// The interleave in use.
    public var interleave: Interleave?
    /// The centre of the carriers above the dial.
    public var centreHz: Double
    /// How clean the decisions are, 0 to 100.
    public var qualityPct: Double
    /// The noise floor.
    public var floorDb: Double
    /// The times the receiver locked.
    public var locks: Int64
    /// The correction since the lock.
    public var afcHz: Double
    /// The SNR.
    public var snrDb: Double
    /// Characters decoded.
    public var characters: Int64
    /// Characters that the code did not resolve.
    public var errors: Int64
    /// The decoded text.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?

    init(_ c: sdrkit_mt63_fields) {
        bandwidthHz = c.bandwidth_hz
        interleave = Interleave(rawValue: c.interleave)
        centreHz = c.centre_hz
        qualityPct = c.quality_pct
        floorDb = c.floor_db
        locks = c.locks
        afcHz = c.afc_hz
        snrDb = c.snr_db
        characters = c.characters
        errors = c.errors
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Mt63Fields, SDRKitCode> {
        var c = sdrkit_mt63_fields()
        let code = sdrkit_mt63_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Mt63Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of FSQ and IFKP. The tones have no value before a signal is heard.
public struct FsqFields {
    /// The speed in use.
    public var speed: String
    /// The width of the tones.
    public var bandHz: Double
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The lowest tone heard.
    public var lowestHz: Double?
    /// The highest tone heard.
    public var highestHz: Double?
    /// How clean the decisions are, 0 to 100.
    public var qualityPct: Double
    /// The SNR.
    public var snrDb: Double
    /// Characters decoded.
    public var characters: Int64
    /// Symbols that the receiver refused.
    public var droppedSymbols: Int64
    /// The decoded text.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?

    init(_ c: sdrkit_fsq_fields) {
        speed = sdrkitText(c.speed)
        bandHz = c.band_hz
        symbolRateHz = c.symbol_rate_hz
        lowestHz = c.has_lowest_hz ? c.lowest_hz : nil
        highestHz = c.has_highest_hz ? c.highest_hz : nil
        qualityPct = c.quality_pct
        snrDb = c.snr_db
        characters = c.characters
        droppedSymbols = c.dropped_symbols
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FsqFields, SDRKitCode> {
        var c = sdrkit_fsq_fields()
        let code = sdrkit_fsq_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FsqFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the FSQ messages. The row key is the place of the message in the log.
public struct FsqMessageRow {
    public enum Trigger: Int32 {
        case text = 0
        case report = 1
        case ack = 2
        case file = 3
        case relay = 4
        case `repeat` = 5
        case repeatLater = 6
        case store = 7
        case heard = 8
        case location = 9
        case motd = 10
        case version = 11
        case picture = 12
        case alert = 13
        case faster = 14
        case slower = 15
        case relayed = 16
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The sender, when the line starts with "CALL:".
    public var from: String?
    /// The two hex digits equal the CRC-8 of the callsign.
    public var crcOk: Bool
    /// The addressee.
    public var to: String?
    /// The trigger.
    public var trigger: Trigger?
    /// The text after the trigger.
    public var text: String
    /// Addressed to my call, allcall or cqcqcq.
    public var toMe: Bool

    init(_ c: sdrkit_fsq_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        from = c.has_from ? sdrkitText(c.from) : nil
        crcOk = c.crc_ok
        to = c.has_to ? sdrkitText(c.to) : nil
        trigger = c.has_trigger ? Trigger(rawValue: c.trigger) : nil
        text = sdrkitText(c.text)
        toMe = c.to_me
    }
}

/// The result fields of SSTV.
public struct SstvFields {
    /// The mode of the picture being received or the newest one. It has no value before the first picture.
    public var mode: String?
    /// Its VIS code, -1 when it was found from the line period.
    public var vis: Int64?
    /// Its slant: the line period over the nominal one, minus 1.
    public var slantPpm: Double?
    /// The tuning error.
    public var offsetHz: Double

    init(_ c: sdrkit_sstv_fields) {
        mode = c.has_mode ? sdrkitText(c.mode) : nil
        vis = c.has_vis ? c.vis : nil
        slantPpm = c.has_slant_ppm ? c.slant_ppm : nil
        offsetHz = c.offset_hz
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SstvFields, SDRKitCode> {
        var c = sdrkit_sstv_fields()
        let code = sdrkit_sstv_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SstvFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the SSTV transmitter.
public struct SstvTxFields {
    /// Whether a picture is being sent.
    public var txSending: Bool
    /// The mode of the picture being sent. No value while nothing is sent.
    public var txMode: String?
    /// The line being sent, from 1; 0 during the VIS header. No value while nothing is sent.
    public var txLine: Int64?
    /// The lines of the picture being sent. No value while nothing is sent.
    public var txLines: Int64?
    /// The time on air of one picture in the transmit mode: the VIS header and the lines.
    public var txPictureS: Double
    /// The time until the picture is sent again, while repeat waits in the pause. No value otherwise.
    public var txNextS: Double?

    init(_ c: sdrkit_sstv_tx_fields) {
        txSending = c.tx_sending
        txMode = c.has_tx_mode ? sdrkitText(c.tx_mode) : nil
        txLine = c.has_tx_line ? c.tx_line : nil
        txLines = c.has_tx_lines ? c.tx_lines : nil
        txPictureS = c.tx_picture_s
        txNextS = c.has_tx_next_s ? c.tx_next_s : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SstvTxFields, SDRKitCode> {
        var c = sdrkit_sstv_tx_fields()
        let code = sdrkit_sstv_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SstvTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of SITOR.
public struct SitorFields {
    /// The decoded text.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?
    /// The centre of the tones.
    public var offsetHz: Double
    /// The shift. It has no value before a signal is found.
    public var shiftHz: Double?
    /// The speed. It has no value before a signal is found.
    public var symbolRateHz: Double?
    /// The tones are the other way round.
    public var inverted: Bool
    /// Words of a good code.
    public var goodWords: Int64
    /// Words of a bad code.
    public var badWords: Int64
    /// Characters lost in both copies.
    public var mutilated: Int64
    /// Characters decoded.
    public var characters: Int64
    /// The ARQ monitor follows a link.
    public var arqLocked: Bool
    /// ARQ blocks read.
    public var arqBlocks: Int64
    /// ARQ blocks that were new.
    public var arqNewBlocks: Int64
    /// ARQ blocks sent again.
    public var arqRepeats: Int64
    /// ARQ slots that did not read.
    public var arqBad: Int64
    /// ARQ control signals read.
    public var arqControlSignals: Int64
    /// Where the control signals lie. It has no value before one is found.
    public var arqCsOffsetBits: Int64?
    /// ARQ characters decoded.
    public var arqChars: Int64
    /// The ARQ text.
    public var arqText: String
    /// The selective call of the link, if one was read.
    public var selcall: String?
    /// The changes of direction of the link.
    public var arqChanges: Int64

    init(_ c: sdrkit_sitor_fields) {
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        offsetHz = c.offset_hz
        shiftHz = c.has_shift_hz ? c.shift_hz : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        inverted = c.inverted
        goodWords = c.good_words
        badWords = c.bad_words
        mutilated = c.mutilated
        characters = c.characters
        arqLocked = c.arq_locked
        arqBlocks = c.arq_blocks
        arqNewBlocks = c.arq_new_blocks
        arqRepeats = c.arq_repeats
        arqBad = c.arq_bad
        arqControlSignals = c.arq_control_signals
        arqCsOffsetBits = c.has_arq_cs_offset_bits ? c.arq_cs_offset_bits : nil
        arqChars = c.arq_chars
        arqText = sdrkitText(c.arq_text)
        selcall = c.has_selcall ? sdrkitText(c.selcall) : nil
        arqChanges = c.arq_changes
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SitorFields, SDRKitCode> {
        var c = sdrkit_sitor_fields()
        let code = sdrkit_sitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SitorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of ARDOP.
public struct ArdopFields {
    /// Leaders heard.
    public var leaders: Int64
    /// Headers read.
    public var goodHeaders: Int64
    /// Leaders whose header did not read.
    public var unreadableLeaders: Int64
    /// Data frames good.
    public var dataOk: Int64
    /// Data frames bad.
    public var dataBad: Int64
    /// Reed-Solomon corrections.
    public var rsCorrections: Int64
    /// Control frames.
    public var control: Int64
    /// Other frames.
    public var other: Int64
    /// Stations heard.
    public var stationsHeard: Int64
    /// Frames heard.
    public var framesOk: Int64
    /// The newest text, if a frame held one.
    public var text: String?

    init(_ c: sdrkit_ardop_fields) {
        leaders = c.leaders
        goodHeaders = c.good_headers
        unreadableLeaders = c.unreadable_leaders
        dataOk = c.data_ok
        dataBad = c.data_bad
        rsCorrections = c.rs_corrections
        control = c.control
        other = c.other
        stationsHeard = c.stations_heard
        framesOk = c.frames_ok
        text = c.has_text ? sdrkitText(c.text) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ArdopFields, SDRKitCode> {
        var c = sdrkit_ardop_fields()
        let code = sdrkit_ardop_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ArdopFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the ARDOP sender and of its link.
public struct ArdopTxFields {
    public enum LinkState: Int32 {
        case idle = 0
        case connecting = 1
        case sending = 2
        case receiving = 3
        case disconnecting = 4
        case closed = 5
        case failed = 6
    }
    /// Whether a burst is on air.
    public var txSending: Bool
    /// The type of the frame on air. No value while nothing is sent.
    public var txFrame: String?
    /// The bursts waiting to be sent.
    public var txWaiting: Int64
    /// The state of the link.
    public var linkState: LinkState?
    /// The other station of the link. No value while no link was made.
    public var linkPeer: String?
    /// The session id of the link. No value while no link was made.
    public var linkSession: Int64?
    /// The data bytes of the stack that the other station acknowledged.
    public var linkSent: Int64
    /// The data bytes that the other station sent and the stack received.
    public var linkReceived: Int64
    /// The data bytes waiting to be sent on the link.
    public var linkWaiting: Int64
    /// The repeats in a row of the frame that waits for its answer.
    public var linkRetries: Int64
    /// Why the link state is what it is; empty when it needs no reason.
    public var linkReason: String
    /// The text received on the link, the newest 4000 bytes. No value before the first byte.
    public var linkText: String?
    /// The station of the newest connect request to the station callsign that the stack did not answer.
    public var linkRequest: String?

    init(_ c: sdrkit_ardop_tx_fields) {
        txSending = c.tx_sending
        txFrame = c.has_tx_frame ? sdrkitText(c.tx_frame) : nil
        txWaiting = c.tx_waiting
        linkState = LinkState(rawValue: c.link_state)
        linkPeer = c.has_link_peer ? sdrkitText(c.link_peer) : nil
        linkSession = c.has_link_session ? c.link_session : nil
        linkSent = c.link_sent
        linkReceived = c.link_received
        linkWaiting = c.link_waiting
        linkRetries = c.link_retries
        linkReason = sdrkitText(c.link_reason)
        linkText = c.has_link_text ? sdrkitText(c.link_text) : nil
        linkRequest = c.has_link_request ? sdrkitText(c.link_request) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ArdopTxFields, SDRKitCode> {
        var c = sdrkit_ardop_tx_fields()
        let code = sdrkit_ardop_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ArdopTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ARDOP frames. The row key is the number of the frame, counted from the start.
public struct ArdopFrameRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The frame type.
    public var type: Int64
    /// The name of the type.
    public var name: String
    /// What the frame is.
    public var kind: String
    /// Its modulation.
    public var modulation: String
    /// Its speed.
    public var baud: Int64
    /// The session id.
    public var session: Int64
    /// The sender, if the frame names one.
    public var from: String?
    /// The addressee, if the frame names one.
    public var to: String?
    /// The locator, if the frame holds one.
    public var locator: String?
    /// The text, if the frame holds one.
    public var text: String?
    /// The quality that the frame reports.
    public var quality: Int64?
    /// The SNR, if it was measured.
    public var snrDb: Double?
    /// The length of the leader, if it was measured.
    public var leaderMs: Int64?
    /// The header read.
    public var headerOk: Bool
    /// The body was read.
    public var bodyRead: Bool
    /// The body is good.
    public var bodyOk: Bool
    /// Reed-Solomon corrections.
    public var rsCorrections: Int64
    /// The offset of the tones.
    public var toneOffsetHz: Double
    /// The SNR of the tones.
    public var toneSnrDb: Double

    init(_ c: sdrkit_ardop_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        type = c.type
        name = sdrkitText(c.name)
        kind = sdrkitText(c.kind)
        modulation = sdrkitText(c.modulation)
        baud = c.baud
        session = c.session
        from = c.has_from ? sdrkitText(c.from) : nil
        to = c.has_to ? sdrkitText(c.to) : nil
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        quality = c.has_quality ? c.quality : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        leaderMs = c.has_leader_ms ? c.leader_ms : nil
        headerOk = c.header_ok
        bodyRead = c.body_read
        bodyOk = c.body_ok
        rsCorrections = c.rs_corrections
        toneOffsetHz = c.tone_offset_hz
        toneSnrDb = c.tone_snr_db
    }
}

/// A row of the ARDOP stations. The row key is the callsign.
public struct ArdopStationRow {
    /// The row key.
    public var key: String
    /// The callsign.
    public var callsign: String
    /// The locator, if one was heard.
    public var locator: String?
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last time heard.
    public var lastHeard: Double?
    /// The type of its last frame.
    public var lastType: String
    /// The station it called, if it called one.
    public var called: String?

    init(_ c: sdrkit_ardop_station_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        locator = c.has_locator ? sdrkitText(c.locator) : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        lastType = sdrkitText(c.last_type)
        called = c.has_called ? sdrkitText(c.called) : nil
    }
}

/// The result fields of ROS. The fields of the detection have no value without one.
public struct RosFields {
    /// The sub-mode found, if the rate gives one.
    public var subMode: String?
    /// What was found.
    public var name: String?
    /// Why.
    public var why: String?
    /// The symbol rate.
    public var symbolRateHz: Double?
    /// The spacing of the tone grid.
    public var spacingHz: Double?
    /// How well the tones fit the grid.
    public var fitPct: Double?
    /// The lowest tone.
    public var lowestHz: Double?
    /// The highest tone.
    public var highestHz: Double?
    /// How pure the tones are.
    public var purityPct: Double?
    /// The tone positions used.
    public var distinct: Int64?
    /// The score.
    public var scorePct: Double?
    /// The signal is identified.
    public var detected: Bool
    /// Few symbols were seen.
    public var weak: Bool
    /// The tuning error.
    public var offsetHz: Double
    /// Identifications.
    public var detections: Int64
    /// Signals that did not fit.
    public var misfits: Int64

    init(_ c: sdrkit_ros_fields) {
        subMode = c.has_sub_mode ? sdrkitText(c.sub_mode) : nil
        name = c.has_name ? sdrkitText(c.name) : nil
        why = c.has_why ? sdrkitText(c.why) : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        spacingHz = c.has_spacing_hz ? c.spacing_hz : nil
        fitPct = c.has_fit_pct ? c.fit_pct : nil
        lowestHz = c.has_lowest_hz ? c.lowest_hz : nil
        highestHz = c.has_highest_hz ? c.highest_hz : nil
        purityPct = c.has_purity_pct ? c.purity_pct : nil
        distinct = c.has_distinct ? c.distinct : nil
        scorePct = c.has_score_pct ? c.score_pct : nil
        detected = c.detected
        weak = c.weak
        offsetHz = c.offset_hz
        detections = c.detections
        misfits = c.misfits
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RosFields, SDRKitCode> {
        var c = sdrkit_ros_fields()
        let code = sdrkit_ros_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RosFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ROS identifications. The row key is the place of the identification in the log.
public struct RosDetectionRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The sub-mode, if the rate gives one.
    public var subMode: String?
    /// What was found.
    public var name: String
    /// The symbol rate.
    public var symbolRateHz: Double
    /// Why.
    public var why: String

    init(_ c: sdrkit_ros_detection_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        subMode = c.has_sub_mode ? sdrkitText(c.sub_mode) : nil
        name = sdrkitText(c.name)
        symbolRateHz = c.symbol_rate_hz
        why = sdrkitText(c.why)
    }
}

/// The result fields of Throb. The fields of the reading have no value before the first.
public struct ThrobFields {
    /// The decoded text.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?
    /// The variant found.
    public var variant: String?
    /// The audio centre found.
    public var centreHz: Double?
    /// The score of the reading.
    public var score: Double?
    /// The SNR of the reading.
    public var snrDb: Double?
    /// Characters decoded.
    public var characters: Int64
    /// A note on the stack.
    public var note: String

    init(_ c: sdrkit_throb_fields) {
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        variant = c.has_variant ? sdrkitText(c.variant) : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        score = c.has_score ? c.score : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        characters = c.characters
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ThrobFields, SDRKitCode> {
        var c = sdrkit_throb_fields()
        let code = sdrkit_throb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ThrobFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of QRSS. The fields of the reading have no value before the first.
public struct QrssFields {
    /// The text of the newest reading.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?
    /// The tone, if one is found.
    public var centreHz: Double?
    /// The length of an element, while a tone is found.
    public var elementS: Double?
    /// The SNR of the reading, while a tone is found.
    public var snrDb: Double?
    /// A tone is found.
    public var locked: Bool
    /// The picture rows made.
    public var pictureRows: Int64
    /// A note on the stack.
    public var note: String

    init(_ c: sdrkit_qrss_fields) {
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        elementS = c.has_element_s ? c.element_s : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        locked = c.locked
        pictureRows = c.picture_rows
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<QrssFields, SDRKitCode> {
        var c = sdrkit_qrss_fields()
        let code = sdrkit_qrss_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(QrssFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of CP16. The fields of the comb and the clock have no value before they are found.
public struct Cp16Fields {
    /// The decoded text.
    public var text: String
    /// The newest callsign of the text, if it has one.
    public var callsign: String?
    /// The state of the receiver.
    public var state: Int64
    /// The lowest carrier.
    public var baseHz: Double?
    /// The spacing of the carriers.
    public var spacingHz: Double?
    /// The time of one row.
    public var rowMs: Double?
    /// The seconds of one character.
    public var charS: Double?
    /// The signal is on the lower sideband.
    public var mirrored: Bool?
    /// The score of the comb.
    public var combDb: Double?
    /// The score of the timing, 0 to 1.
    public var timing: Double
    /// The SNR.
    public var snrDb: Double?
    /// The picture rows made.
    public var pictureRows: Int64
    /// Characters decoded.
    public var characters: Int64
    /// Characters read with a clear margin.
    public var sure: Int64
    /// Characters read without one.
    public var unclear: Int64
    /// The font that the characters are read against. It has no value before a font is loaded.
    public var fontName: String?
    /// A note on the stack.
    public var note: String

    init(_ c: sdrkit_cp16_fields) {
        text = sdrkitText(c.text)
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        state = c.state
        baseHz = c.has_base_hz ? c.base_hz : nil
        spacingHz = c.has_spacing_hz ? c.spacing_hz : nil
        rowMs = c.has_row_ms ? c.row_ms : nil
        charS = c.has_char_s ? c.char_s : nil
        mirrored = c.has_mirrored ? c.mirrored : nil
        combDb = c.has_comb_db ? c.comb_db : nil
        timing = c.timing
        snrDb = c.has_snr_db ? c.snr_db : nil
        pictureRows = c.picture_rows
        characters = c.characters
        sure = c.sure
        unclear = c.unclear
        fontName = c.has_font_name ? sdrkitText(c.font_name) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Cp16Fields, SDRKitCode> {
        var c = sdrkit_cp16_fields()
        let code = sdrkit_cp16_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Cp16Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of WINMOR.
public struct WinmorFields {
    /// A note on the stack.
    public var note: String
    /// Pilots heard.
    public var pilots: Int64
    /// The tuning error of the newest pilot, if one was heard.
    public var errorHz: Double?

    init(_ c: sdrkit_winmor_fields) {
        note = sdrkitText(c.note)
        pilots = c.pilots
        errorHz = c.has_error_hz ? c.error_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WinmorFields, SDRKitCode> {
        var c = sdrkit_winmor_fields()
        let code = sdrkit_winmor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WinmorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the WINMOR pilots. The row key is the place of the pilot in the log.
public struct WinmorPilotRow {
    /// The row key.
    public var key: String
    /// The UTC.
    public var time: Double
    /// The tuning error.
    public var errorHz: Double
    /// The pilot symbols heard.
    public var pilotSymbols: Int64
    /// The frame-type tones.
    public var tones: String

    init(_ c: sdrkit_winmor_pilot_row) {
        key = sdrkitText(c.key)
        time = c.time
        errorHz = c.error_hz
        pilotSymbols = c.pilot_symbols
        tones = sdrkitText(c.tones)
    }
}

/// The fields of a callsign heard for the first time.
public struct HfkbdSpotEvent {
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
    /// The callsign.
    public var source: String
    /// The SNR.
    public var snrDb: Double
    /// Where it was heard.
    public var freqHzField: Double
    /// The stack that heard it ("RTTY").
    public var mode: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_hfkbd_spot_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        source = sdrkitText(c.source)
        snrDb = c.snr_db
        freqHzField = c.freq_hz_
        mode = sdrkitText(c.mode)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfkbdSpotEvent, SDRKitCode> {
        var c = sdrkit_hfkbd_spot_event()
        let code = sdrkit_hfkbd_spot_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfkbdSpotEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a finished line of decoded text.
public struct HfkbdTextEvent {
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
    /// The callsign after "DE", if the line has one.
    public var source: String?
    /// The line.
    public var text: String
    /// The SNR.
    public var snrDb: Double
    /// The stack that heard it.
    public var mode: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_hfkbd_text_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        mode = sdrkitText(c.mode)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfkbdTextEvent, SDRKitCode> {
        var c = sdrkit_hfkbd_text_event()
        let code = sdrkit_hfkbd_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfkbdTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a keyboard transmission.
public struct HfkbdTxEvent {
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
    /// Why it changed: empty, stopped, the transmitter is off, the format changed, tune carrier, or why a text was refused.
    public var reason: String
    /// The text: as given to `send` for started, queued and refused; as coded for finished.
    public var text: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_hfkbd_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfkbdTxEvent, SDRKitCode> {
        var c = sdrkit_hfkbd_tx_event()
        let code = sdrkit_hfkbd_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfkbdTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a picture received or cut off.
public struct MfskPictureEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the picture.
    public var pictureId: Int64
    /// Its width.
    public var pictureWidth: Int64
    /// Its height.
    public var pictureHeight: Int64
    /// The rows received.
    public var pictureRows: Int64
    /// It is complete.
    public var pictureComplete: Bool
    /// Its mode.
    public var pictureMode: String

    init(_ c: sdrkit_mfsk_picture_event) {
        time = c.time
        freqHz = c.freq_hz
        pictureId = c.picture_id
        pictureWidth = c.picture_width
        pictureHeight = c.picture_height
        pictureRows = c.picture_rows
        pictureComplete = c.picture_complete
        pictureMode = sdrkitText(c.picture_mode)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MfskPictureEvent, SDRKitCode> {
        var c = sdrkit_mfsk_picture_event()
        let code = sdrkit_mfsk_picture_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MfskPictureEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an FSQ message.
public struct FsqMessageEvent {
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
    public enum Trigger: Int32 {
        case text = 0
        case report = 1
        case ack = 2
        case file = 3
        case relay = 4
        case `repeat` = 5
        case repeatLater = 6
        case store = 7
        case heard = 8
        case location = 9
        case motd = 10
        case version = 11
        case picture = 12
        case alert = 13
        case faster = 14
        case slower = 15
        case relayed = 16
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sender, when the line starts with "CALL:".
    public var source: String?
    /// The addressee.
    public var target: String?
    /// The text after the trigger.
    public var text: String
    /// The family of the source: a message.
    public var family: Family?
    /// The two hex digits equal the CRC-8 of the callsign.
    public var crcOk: Bool
    /// The trigger.
    public var trigger: Trigger?
    /// Addressed to my call, allcall or cqcqcq.
    public var toMe: Bool

    init(_ c: sdrkit_fsq_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        crcOk = c.crc_ok
        trigger = c.has_trigger ? Trigger(rawValue: c.trigger) : nil
        toMe = c.to_me
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FsqMessageEvent, SDRKitCode> {
        var c = sdrkit_fsq_message_event()
        let code = sdrkit_fsq_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FsqMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a picture received or cut off.
public struct SstvPictureEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the picture.
    public var pictureId: Int64
    /// Its width.
    public var pictureWidth: Int64
    /// Its height.
    public var pictureHeight: Int64
    /// The rows received.
    public var pictureRows: Int64
    /// It is complete.
    public var pictureComplete: Bool
    /// Its mode.
    public var pictureMode: String

    init(_ c: sdrkit_sstv_picture_event) {
        time = c.time
        freqHz = c.freq_hz
        pictureId = c.picture_id
        pictureWidth = c.picture_width
        pictureHeight = c.picture_height
        pictureRows = c.picture_rows
        pictureComplete = c.picture_complete
        pictureMode = sdrkitText(c.picture_mode)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SstvPictureEvent, SDRKitCode> {
        var c = sdrkit_sstv_picture_event()
        let code = sdrkit_sstv_picture_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SstvPictureEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of the SSTV transmission.
public struct SstvTxEvent {
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
    /// The state: started, finished or refused.
    public var state: String
    /// Why it changed: empty, stopped, the transmitter is off, or why a picture was refused.
    public var reason: String
    /// The mode of the picture.
    public var mode: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_sstv_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        mode = sdrkitText(c.mode)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SstvTxEvent, SDRKitCode> {
        var c = sdrkit_sstv_tx_event()
        let code = sdrkit_sstv_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SstvTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a SITOR-A link event.
public struct SitorArqEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What happened.
    public var text: String

    init(_ c: sdrkit_sitor_arq_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SitorArqEvent, SDRKitCode> {
        var c = sdrkit_sitor_arq_event()
        let code = sdrkit_sitor_arq_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SitorArqEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an ARDOP frame heard.
public struct ArdopFrameEvent {
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
    /// The sender, if the frame names one.
    public var source: String?
    /// The addressee, if the frame names one.
    public var target: String?
    /// The family of the source: a message.
    public var family: Family?
    /// The name of the type.
    public var name: String
    /// The frame type.
    public var type: Int64

    init(_ c: sdrkit_ardop_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        family = Family(rawValue: c.family)
        name = sdrkitText(c.name)
        type = c.type
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArdopFrameEvent, SDRKitCode> {
        var c = sdrkit_ardop_frame_event()
        let code = sdrkit_ardop_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArdopFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of the ARDOP link.
public struct ArdopLinkEvent {
    public enum State: Int32 {
        case idle = 0
        case connecting = 1
        case sending = 2
        case receiving = 3
        case disconnecting = 4
        case closed = 5
        case failed = 6
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
    /// The link state after the change.
    public var state: State?
    /// The other station.
    public var source: String
    /// What happened: connected, data received, the other station disconnected, a request not answered, or why the link failed.
    public var reason: String
    /// The data received, as text; empty when none came.
    public var text: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_ardop_link_event) {
        time = c.time
        freqHz = c.freq_hz
        state = State(rawValue: c.state)
        source = sdrkitText(c.source)
        reason = sdrkitText(c.reason)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArdopLinkEvent, SDRKitCode> {
        var c = sdrkit_ardop_link_event()
        let code = sdrkit_ardop_link_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArdopLinkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a ROS identification.
public struct RosDetectionEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sub-mode, if the rate gives one.
    public var subMode: String?
    /// What was found.
    public var name: String
    /// The symbol rate.
    public var symbolRateHz: Double

    init(_ c: sdrkit_ros_detection_event) {
        time = c.time
        freqHz = c.freq_hz
        subMode = c.has_sub_mode ? sdrkitText(c.sub_mode) : nil
        name = sdrkitText(c.name)
        symbolRateHz = c.symbol_rate_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RosDetectionEvent, SDRKitCode> {
        var c = sdrkit_ros_detection_event()
        let code = sdrkit_ros_detection_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RosDetectionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the characters of one Throb reading.
public struct ThrobTextEvent {
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
    /// The characters.
    public var text: String
    /// The variant.
    public var variant: String
    /// The audio centre.
    public var centreHz: Double
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_throb_text_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        variant = sdrkitText(c.variant)
        centreHz = c.centre_hz
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ThrobTextEvent, SDRKitCode> {
        var c = sdrkit_throb_text_event()
        let code = sdrkit_throb_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ThrobTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a QRSS reading.
public struct QrssTextEvent {
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
    /// The text.
    public var text: String
    /// The length of an element.
    public var elementS: Double
    /// The tone.
    public var centreHz: Double
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_qrss_text_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        elementS = c.element_s
        centreHz = c.centre_hz
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<QrssTextEvent, SDRKitCode> {
        var c = sdrkit_qrss_text_event()
        let code = sdrkit_qrss_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(QrssTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a CP16 character read.
public struct Cp16CharEvent {
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
    /// The character.
    public var text: String
    /// Its score against the font.
    public var score: Double
    /// Its margin over the next character.
    public var margin: Double
    /// The margin is clear.
    public var sure: Bool
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_cp16_char_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        score = c.score
        margin = c.margin
        sure = c.sure
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Cp16CharEvent, SDRKitCode> {
        var c = sdrkit_cp16_char_event()
        let code = sdrkit_cp16_char_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Cp16CharEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of the CP16 transmission.
public struct Cp16TxEvent {
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
    /// The state of the transmission.
    public var state: String
    /// Why it changed.
    public var reason: String
    /// The text sent.
    public var text: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_cp16_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Cp16TxEvent, SDRKitCode> {
        var c = sdrkit_cp16_tx_event()
        let code = sdrkit_cp16_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Cp16TxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a WINMOR pilot.
public struct WinmorPilotEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What was heard.
    public var text: String
    /// The tuning error.
    public var errorHz: Double
    /// The pilot symbols heard.
    public var pilotSymbols: Int64
    /// The frame-type tones.
    public var tones: String

    init(_ c: sdrkit_winmor_pilot_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        errorHz = c.error_hz
        pilotSymbols = c.pilot_symbols
        tones = sdrkitText(c.tones)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WinmorPilotEvent, SDRKitCode> {
        var c = sdrkit_winmor_pilot_event()
        let code = sdrkit_winmor_pilot_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WinmorPilotEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum HfkbdComponents {
    /// The result fields of the component "front" of the stack "rtty" in the snapshot of `handle`.
    public static func rttyFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_rtty_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "psk" in the snapshot of `handle`.
    public static func pskFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_psk_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "cw" in the snapshot of `handle`.
    public static func cwFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_cw_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "olivia" in the snapshot of `handle`.
    public static func oliviaFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_olivia_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "contestia" in the snapshot of `handle`.
    public static func contestiaFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_contestia_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "mfsk" in the snapshot of `handle`.
    public static func mfskFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_mfsk_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "dominoex" in the snapshot of `handle`.
    public static func dominoexFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_dominoex_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "thor" in the snapshot of `handle`.
    public static func thorFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_thor_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "hell" in the snapshot of `handle`.
    public static func hellFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_hell_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "mt63" in the snapshot of `handle`.
    public static func mt63FrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_mt63_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "fsq" in the snapshot of `handle`.
    public static func fsqFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_fsq_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "ifkp" in the snapshot of `handle`.
    public static func ifkpFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_ifkp_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "sstv" in the snapshot of `handle`.
    public static func sstvFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_sstv_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "sitor" in the snapshot of `handle`.
    public static func sitorFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_sitor_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "ardop" in the snapshot of `handle`.
    public static func ardopFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_ardop_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "ros" in the snapshot of `handle`.
    public static func rosFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_ros_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "throb" in the snapshot of `handle`.
    public static func throbFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_throb_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "qrss" in the snapshot of `handle`.
    public static func qrssFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_qrss_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "cp16" in the snapshot of `handle`.
    public static func cp16FrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_cp16_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "front" of the stack "winmor" in the snapshot of `handle`.
    public static func winmorFrontFields(_ handle: SDRKIT_HANDLE) -> Result<HfkbdFrontFields, SDRKitCode> {
        var c = sdrkit_hfkbd_front_fields()
        let code = sdrkit_winmor_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfkbdFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum HfkbdTables {
    /// The changed rows of the table "stations" of the stack "rtty", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func rttyStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rtty_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "auto_guesses" of the stack "rtty", newest first, at most `capacity` rows.
    public static func rttyAutoGuesses(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[RttyGuessRow], SDRKitCode> {
        var rows = [sdrkit_rtty_guess_row](repeating: sdrkit_rtty_guess_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rtty_auto_guesses_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RttyGuessRow.init))
    }
    /// The changed rows of the table "stations" of the stack "psk", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func pskStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_psk_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "signals" of the stack "psk", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func pskSignals(_ handle: SDRKIT_HANDLE, capacity: Int = 72) -> Result<[PskSignalRow], SDRKitCode> {
        var rows = [sdrkit_psk_signal_row](repeating: sdrkit_psk_signal_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_psk_signals_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PskSignalRow.init))
    }
    /// The changed rows of the table "stations" of the stack "cw", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func cwStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cw_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "signals" of the stack "cw", newest first, at most `capacity` rows.
    /// Default order: the column `snr_db`, highest first.
    public static func cwSignals(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[CwSignalRow], SDRKitCode> {
        var rows = [sdrkit_cw_signal_row](repeating: sdrkit_cw_signal_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cw_signals_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CwSignalRow.init))
    }
    /// The changed rows of the table "stations" of the stack "olivia", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func oliviaStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_olivia_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "contestia", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func contestiaStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_contestia_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "mfsk", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func mfskStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mfsk_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "dominoex", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func dominoexStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dominoex_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "thor", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func thorStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_thor_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "mt63", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func mt63Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mt63_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "fsq", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func fsqStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsq_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "messages" of the stack "fsq", newest first, at most `capacity` rows.
    /// The column `from` names a row.
    public static func fsqMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[FsqMessageRow], SDRKitCode> {
        var rows = [sdrkit_fsq_message_row](repeating: sdrkit_fsq_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsq_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FsqMessageRow.init))
    }
    /// The changed rows of the table "stations" of the stack "ifkp", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func ifkpStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ifkp_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "sitor", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func sitorStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sitor_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "frames" of the stack "ardop", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func ardopFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[ArdopFrameRow], SDRKitCode> {
        var rows = [sdrkit_ardop_frame_row](repeating: sdrkit_ardop_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ardop_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ArdopFrameRow.init))
    }
    /// The changed rows of the table "stations" of the stack "ardop", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func ardopStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[ArdopStationRow], SDRKitCode> {
        var rows = [sdrkit_ardop_station_row](repeating: sdrkit_ardop_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ardop_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ArdopStationRow.init))
    }
    /// The changed rows of the table "detections" of the stack "ros", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func rosDetections(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[RosDetectionRow], SDRKitCode> {
        var rows = [sdrkit_ros_detection_row](repeating: sdrkit_ros_detection_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ros_detections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RosDetectionRow.init))
    }
    /// The changed rows of the table "stations" of the stack "throb", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func throbStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_throb_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "qrss", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func qrssStations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qrss_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "stations" of the stack "cp16", newest first, at most `capacity` rows.
    /// The column `callsign` names a row.
    public static func cp16Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[HfkbdStationRow], SDRKitCode> {
        var rows = [sdrkit_hfkbd_station_row](repeating: sdrkit_hfkbd_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cp16_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfkbdStationRow.init))
    }
    /// The changed rows of the table "pilots" of the stack "winmor", newest first, at most `capacity` rows.
    public static func winmorPilots(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[WinmorPilotRow], SDRKitCode> {
        var rows = [sdrkit_winmor_pilot_row](repeating: sdrkit_winmor_pilot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_winmor_pilots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WinmorPilotRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum HfkbdCommands {
    /// Remove. Forgets one object, a favourite too.
    public static func rttyRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_rtty_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Find the settings. Runs one analysis on up to 20 s of the kept audio, once 6 s are kept, and applies its best guess even with Auto off.
    public static func rttyAutoLock(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rtty_auto_lock(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func rttyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rtty_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func rttySend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_rtty_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func rttyStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rtty_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func rttyTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rtty_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func pskRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_psk_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func pskClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_psk_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func pskSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_psk_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func pskStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_psk_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func pskTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_psk_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func cwRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_cw_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cwClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cw_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func oliviaRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_olivia_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func oliviaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_olivia_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func oliviaSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_olivia_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func oliviaStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_olivia_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func oliviaTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_olivia_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func contestiaRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_contestia_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func contestiaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_contestia_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func contestiaSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_contestia_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func contestiaStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_contestia_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func contestiaTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_contestia_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func mfskRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_mfsk_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func mfskClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mfsk_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func mfskSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_mfsk_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func mfskStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mfsk_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func mfskTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mfsk_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dominoexRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dominoex_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dominoexClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dominoex_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func dominoexSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_dominoex_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func dominoexStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dominoex_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func dominoexTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dominoex_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func thorRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_thor_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func thorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_thor_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func thorSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_thor_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func thorStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_thor_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func thorTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_thor_tune(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func hellClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hell_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func hellSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_hell_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func hellStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hell_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func hellTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hell_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func mt63Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_mt63_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func mt63Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mt63_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func mt63Send(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_mt63_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func mt63Stop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mt63_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func mt63Tune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mt63_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func fsqRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_fsq_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fsqClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsq_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a line. Queues one line from the station callsign with its check, to the addressee with the trigger when one is given. Transmit must be on and the station callsign set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func fsqSend(_ handle: SDRKIT_HANDLE, text: String, to: String? = nil, trigger: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_fsq_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_fsq_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in sdrkitWithString(to ?? "") { to_ in { () -> Int32 in
        args.text = text_
        args.has_to = to != nil
        args.to = to_
        args.has_trigger = trigger != nil
        if let given = trigger { args.trigger = given }
        return sdrkit_fsq_send(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func fsqStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsq_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func fsqTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsq_tune(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ifkpRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ifkp_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ifkpClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ifkp_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ifkpSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_ifkp_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Drops the text waiting and ends the signal with a 10 ms fade.
    public static func ifkpStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ifkp_stop(handle)
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ifkpTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ifkp_tune(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func sstvClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sstv_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a picture. Scales the picture to the transmit mode by the fit setting and sends it: the VIS header, then the lines; again after each pause while repeat is on. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func sstvSend(_ handle: SDRKIT_HANDLE, pixels: [UInt8], width: Int64, height: Int64, pixelFormat: Int32) -> Result<Void, SDRKitCode> {
        var args = sdrkit_sstv_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_sstv_send_args>.size)
        let code: Int32 = pixels.withUnsafeBufferPointer { pixels_ in { () -> Int32 in
        args.pixels = sdrkit_bytes(ptr: pixels_.baseAddress, len: pixels_.count)
        args.width = width
        args.height = height
        args.pixel_format = pixelFormat
        return sdrkit_sstv_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the picture at once.
    public static func sstvStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sstv_stop(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func sitorRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_sitor_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func sitorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sitor_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ardopRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ardop_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ardopClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ardop_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. While a link is up, the text joins the data of the link. Otherwise it is sent once as FEC data frames of the FEC frame type, to no station. Transmit must be on and the station callsign set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ardopSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_keyboard_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_keyboard_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_ardop_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send the ID. Sends one ID frame with the station callsign and locator. Transmit must be on and the station callsign set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ardopSendId(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ardop_send_id(handle)
        return sdrkitResult(code)
    }
    /// Ping. Sends one Ping frame from the station callsign to the called station. Transmit must be on and the station callsign set. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ardopPing(_ handle: SDRKIT_HANDLE, target: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ardop_call_args()
        args.size = UInt32(MemoryLayout<sdrkit_ardop_call_args>.size)
        let code: Int32 = sdrkitWithString(target) { target_ in { () -> Int32 in
        args.target = target_
        return sdrkit_ardop_ping(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Connect. Calls the station with ConReq500M, every 9 s until it answers, at most 11 times. On the answer the link is up: the text of `send` goes as 4FSK.500.100 data frames, each sent again until the other station acknowledges it. Transmit must be on and the station callsign set. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ardopConnect(_ handle: SDRKIT_HANDLE, target: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ardop_call_args()
        args.size = UInt32(MemoryLayout<sdrkit_ardop_call_args>.size)
        let code: Int32 = sdrkitWithString(target) { target_ in { () -> Int32 in
        args.target = target_
        return sdrkit_ardop_connect(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Disconnect. Ends the link once the data waiting is sent: DISC, answered by END. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func ardopDisconnect(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ardop_disconnect(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the burst on air at once, drops the bursts waiting and closes the link without DISC.
    public static func ardopStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ardop_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func rosClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ros_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func throbRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfkbd_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfkbd_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_throb_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func throbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_throb_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func qrssClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qrss_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cp16Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cp16_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a text. Sends the text with the stack's spacing, lowest carrier, seconds a character and sideband. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func cp16Send(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cp16_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_cp16_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_cp16_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission after the current row.
    public static func cp16Stop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cp16_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func winmorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_winmor_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum HfkbdSettings {
    /// Automatic.
    public static func rttyAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto", value))
    }
    /// The options of the setting "baud" of the stack "rtty".
    public enum RttyBaud: Int32 {
        case n4545 = 0
        case n50Bd = 1
        case n75Bd = 2
        case n100Bd = 3
    }
    /// Speed.
    public static func rttyBaud(_ handle: SDRKIT_HANDLE) -> Result<RttyBaud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(RttyBaud(rawValue: v) ?? RttyBaud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyBaud(_ handle: SDRKIT_HANDLE, _ value: RttyBaud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "baud", value.rawValue))
    }
    /// The options of the setting "shift" of the stack "rtty".
    public enum RttyShift: Int32 {
        case n170Hz = 0
        case n200Hz = 1
        case n425Hz = 2
        case n450Hz = 3
        case n850Hz = 4
    }
    /// Shift.
    public static func rttyShift(_ handle: SDRKIT_HANDLE) -> Result<RttyShift, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "shift", &v)
        return code == SDRKIT_OK ? .success(RttyShift(rawValue: v) ?? RttyShift(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyShift(_ handle: SDRKIT_HANDLE, _ value: RttyShift) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "shift", value.rawValue))
    }
    /// Reverse.
    public static func rttyReverse(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "reverse", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyReverse(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "reverse", value))
    }
    /// Unshift on space.
    public static func rttyUnshiftOnSpace(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "unshift_on_space", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyUnshiftOnSpace(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "unshift_on_space", value))
    }
    /// The options of the setting "demod" of the stack "rtty".
    public enum RttyDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func rttyDemod(_ handle: SDRKIT_HANDLE) -> Result<RttyDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(RttyDemod(rawValue: v) ?? RttyDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyDemod(_ handle: SDRKIT_HANDLE, _ value: RttyDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func rttyListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Audio centre.
    public static func rttyTxCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyTxCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value))
    }
    /// Level.
    public static func rttyTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func rttyTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// Automatic.
    public static func pskAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto", value))
    }
    /// The options of the setting "modulation" of the stack "psk".
    public enum PskModulation: Int32 {
        case bPSK31 = 0
        case bPSK63 = 1
        case qPSK31 = 2
        case qPSK63 = 3
        case bPSK125 = 4
        case bPSK250 = 5
        case qPSK125 = 6
    }
    /// Modulation.
    public static func pskModulation(_ handle: SDRKIT_HANDLE) -> Result<PskModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modulation", &v)
        return code == SDRKIT_OK ? .success(PskModulation(rawValue: v) ?? PskModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPskModulation(_ handle: SDRKIT_HANDLE, _ value: PskModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modulation", value.rawValue))
    }
    /// Threshold.
    public static func pskThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// Focus.
    public static func pskFocusHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "focus_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskFocusHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "focus_hz", value))
    }
    /// Listen.
    public static func pskListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Audio frequency.
    public static func pskTxCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskTxCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value))
    }
    /// Level.
    public static func pskTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func pskTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// Threshold.
    public static func cwThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// The options of the setting "decoder" of the stack "cw".
    public enum CwDecoder: Int32 {
        case adaptive = 0
        case classic = 1
    }
    /// Timing decoder.
    public static func cwDecoder(_ handle: SDRKIT_HANDLE) -> Result<CwDecoder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "decoder", &v)
        return code == SDRKIT_OK ? .success(CwDecoder(rawValue: v) ?? CwDecoder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCwDecoder(_ handle: SDRKIT_HANDLE, _ value: CwDecoder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "decoder", value.rawValue))
    }
    /// Speed.
    public static func cwWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wpm", value))
    }
    /// The options of the setting "filter" of the stack "cw".
    public enum CwFilter: Int32 {
        case n50hz = 0
        case n100hz = 1
        case n250hz = 2
        case n500hz = 3
    }
    /// Filter width.
    public static func cwFilter(_ handle: SDRKIT_HANDLE) -> Result<CwFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(CwFilter(rawValue: v) ?? CwFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCwFilter(_ handle: SDRKIT_HANDLE, _ value: CwFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// The options of the setting "listen" of the stack "cw".
    public enum CwListen: Int32 {
        case off = 0
        case tone = 1
        case signal = 2
    }
    /// Listen.
    public static func cwListen(_ handle: SDRKIT_HANDLE) -> Result<CwListen, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(CwListen(rawValue: v) ?? CwListen(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCwListen(_ handle: SDRKIT_HANDLE, _ value: CwListen) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "listen", value.rawValue))
    }
    /// Pitch.
    public static func cwPitch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pitch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwPitch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pitch", value))
    }
    /// Focus.
    public static func cwFocusHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "focus_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwFocusHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "focus_hz", value))
    }
    /// Forget after.
    public static func cwForgetS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "forget_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwForgetS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "forget_s", value))
    }
    /// The options of the setting "span" of the stack "cw".
    public enum CwSpan: Int32 {
        case n14k = 0
        case n48k = 1
        case n96k = 2
        case n192k = 3
    }
    /// Span.
    public static func cwSpan(_ handle: SDRKIT_HANDLE) -> Result<CwSpan, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "span", &v)
        return code == SDRKIT_OK ? .success(CwSpan(rawValue: v) ?? CwSpan(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCwSpan(_ handle: SDRKIT_HANDLE, _ value: CwSpan) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "span", value.rawValue))
    }
    /// The options of the setting "tones" of the stack "olivia".
    public enum OliviaTones: Int32 {
        case n2 = 0
        case n4 = 1
        case n8 = 2
        case n16 = 3
        case n32 = 4
        case n64 = 5
        case n128 = 6
        case n256 = 7
    }
    /// Tones.
    public static func oliviaTones(_ handle: SDRKIT_HANDLE) -> Result<OliviaTones, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tones", &v)
        return code == SDRKIT_OK ? .success(OliviaTones(rawValue: v) ?? OliviaTones(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaTones(_ handle: SDRKIT_HANDLE, _ value: OliviaTones) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tones", value.rawValue))
    }
    /// The options of the setting "bandwidth" of the stack "olivia".
    public enum OliviaBandwidth: Int32 {
        case n125Hz = 0
        case n250Hz = 1
        case n500Hz = 2
        case n1000Hz = 3
        case n2000Hz = 4
    }
    /// Bandwidth.
    public static func oliviaBandwidth(_ handle: SDRKIT_HANDLE) -> Result<OliviaBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(OliviaBandwidth(rawValue: v) ?? OliviaBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaBandwidth(_ handle: SDRKIT_HANDLE, _ value: OliviaBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// Audio centre.
    public static func oliviaCentre(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaCentre(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre", value))
    }
    /// Find signal.
    public static func oliviaFindSignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "find_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaFindSignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "find_signal", value))
    }
    /// The options of the setting "demod" of the stack "olivia".
    public enum OliviaDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func oliviaDemod(_ handle: SDRKIT_HANDLE) -> Result<OliviaDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(OliviaDemod(rawValue: v) ?? OliviaDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaDemod(_ handle: SDRKIT_HANDLE, _ value: OliviaDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func oliviaListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Level.
    public static func oliviaTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func oliviaTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "tones" of the stack "contestia".
    public enum ContestiaTones: Int32 {
        case n2 = 0
        case n4 = 1
        case n8 = 2
        case n16 = 3
        case n32 = 4
        case n64 = 5
        case n128 = 6
        case n256 = 7
    }
    /// Tones.
    public static func contestiaTones(_ handle: SDRKIT_HANDLE) -> Result<ContestiaTones, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tones", &v)
        return code == SDRKIT_OK ? .success(ContestiaTones(rawValue: v) ?? ContestiaTones(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaTones(_ handle: SDRKIT_HANDLE, _ value: ContestiaTones) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tones", value.rawValue))
    }
    /// The options of the setting "bandwidth" of the stack "contestia".
    public enum ContestiaBandwidth: Int32 {
        case n125Hz = 0
        case n250Hz = 1
        case n500Hz = 2
        case n1000Hz = 3
        case n2000Hz = 4
    }
    /// Bandwidth.
    public static func contestiaBandwidth(_ handle: SDRKIT_HANDLE) -> Result<ContestiaBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(ContestiaBandwidth(rawValue: v) ?? ContestiaBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaBandwidth(_ handle: SDRKIT_HANDLE, _ value: ContestiaBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// Audio centre.
    public static func contestiaCentre(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaCentre(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre", value))
    }
    /// Find signal.
    public static func contestiaFindSignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "find_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaFindSignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "find_signal", value))
    }
    /// The options of the setting "demod" of the stack "contestia".
    public enum ContestiaDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func contestiaDemod(_ handle: SDRKIT_HANDLE) -> Result<ContestiaDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(ContestiaDemod(rawValue: v) ?? ContestiaDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaDemod(_ handle: SDRKIT_HANDLE, _ value: ContestiaDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func contestiaListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Level.
    public static func contestiaTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func contestiaTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "variant" of the stack "mfsk".
    public enum MfskVariant: Int32 {
        case mFSK4 = 0
        case mFSK8 = 1
        case mFSK11 = 2
        case mFSK16 = 3
        case mFSK22 = 4
        case mFSK31 = 5
        case mFSK32 = 6
        case mFSK64 = 7
        case mFSK128 = 8
        case mFSK64L = 9
        case mFSK128L = 10
    }
    /// Variant.
    public static func mfskVariant(_ handle: SDRKIT_HANDLE) -> Result<MfskVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "variant", &v)
        return code == SDRKIT_OK ? .success(MfskVariant(rawValue: v) ?? MfskVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskVariant(_ handle: SDRKIT_HANDLE, _ value: MfskVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "variant", value.rawValue))
    }
    /// Audio centre.
    public static func mfskCentre(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskCentre(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre", value))
    }
    /// Find signal.
    public static func mfskFindSignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "find_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskFindSignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "find_signal", value))
    }
    /// The options of the setting "demod" of the stack "mfsk".
    public enum MfskDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func mfskDemod(_ handle: SDRKIT_HANDLE) -> Result<MfskDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(MfskDemod(rawValue: v) ?? MfskDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskDemod(_ handle: SDRKIT_HANDLE, _ value: MfskDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func mfskListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Level.
    public static func mfskTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func mfskTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "variant" of the stack "dominoex".
    public enum DominoexVariant: Int32 {
        case dominoEX4 = 0
        case dominoEX5 = 1
        case dominoEX8 = 2
        case dominoEX11 = 3
        case dominoEX16 = 4
        case dominoEX22 = 5
    }
    /// Variant.
    public static func dominoexVariant(_ handle: SDRKIT_HANDLE) -> Result<DominoexVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "variant", &v)
        return code == SDRKIT_OK ? .success(DominoexVariant(rawValue: v) ?? DominoexVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexVariant(_ handle: SDRKIT_HANDLE, _ value: DominoexVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "variant", value.rawValue))
    }
    /// Audio centre.
    public static func dominoexCentre(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexCentre(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre", value))
    }
    /// Find signal.
    public static func dominoexFindSignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "find_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexFindSignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "find_signal", value))
    }
    /// The options of the setting "demod" of the stack "dominoex".
    public enum DominoexDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func dominoexDemod(_ handle: SDRKIT_HANDLE) -> Result<DominoexDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(DominoexDemod(rawValue: v) ?? DominoexDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexDemod(_ handle: SDRKIT_HANDLE, _ value: DominoexDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func dominoexListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Level.
    public static func dominoexTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func dominoexTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "variant" of the stack "thor".
    public enum ThorVariant: Int32 {
        case tHOR4 = 0
        case tHOR5 = 1
        case tHOR8 = 2
        case tHOR11 = 3
        case tHOR16 = 4
        case tHOR22 = 5
        case tHOR25x4 = 6
        case tHOR50x1 = 7
        case tHOR50x2 = 8
        case tHOR100 = 9
    }
    /// Variant.
    public static func thorVariant(_ handle: SDRKIT_HANDLE) -> Result<ThorVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "variant", &v)
        return code == SDRKIT_OK ? .success(ThorVariant(rawValue: v) ?? ThorVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setThorVariant(_ handle: SDRKIT_HANDLE, _ value: ThorVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "variant", value.rawValue))
    }
    /// Audio centre.
    public static func thorCentre(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThorCentre(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre", value))
    }
    /// Find signal.
    public static func thorFindSignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "find_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThorFindSignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "find_signal", value))
    }
    /// The options of the setting "demod" of the stack "thor".
    public enum ThorDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func thorDemod(_ handle: SDRKIT_HANDLE) -> Result<ThorDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(ThorDemod(rawValue: v) ?? ThorDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setThorDemod(_ handle: SDRKIT_HANDLE, _ value: ThorDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func thorListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThorListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Level.
    public static func thorTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThorTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func thorTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThorTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "variant" of the stack "hell".
    public enum HellVariant: Int32 {
        case feldHell = 0
        case slowHell = 1
        case hell80 = 2
        case hellX5 = 3
        case hellX9 = 4
        case fSKHell245 = 5
        case fSKHell105 = 6
    }
    /// Variant.
    public static func hellVariant(_ handle: SDRKIT_HANDLE) -> Result<HellVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "variant", &v)
        return code == SDRKIT_OK ? .success(HellVariant(rawValue: v) ?? HellVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHellVariant(_ handle: SDRKIT_HANDLE, _ value: HellVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "variant", value.rawValue))
    }
    /// Audio centre.
    public static func hellCentre(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHellCentre(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre", value))
    }
    /// Find signal.
    public static func hellFindSignal(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "find_signal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHellFindSignal(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "find_signal", value))
    }
    /// The options of the setting "demod" of the stack "hell".
    public enum HellDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func hellDemod(_ handle: SDRKIT_HANDLE) -> Result<HellDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(HellDemod(rawValue: v) ?? HellDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHellDemod(_ handle: SDRKIT_HANDLE, _ value: HellDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func hellListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHellListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Level.
    public static func hellTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHellTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func hellTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHellTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// Font file.
    public static func hellFont(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "font", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHellFont(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "font", $0) })
    }
    /// The options of the setting "bandwidth" of the stack "mt63".
    public enum Mt63Bandwidth: Int32 {
        case auto = 0
        case n500Hz = 1
        case n1000Hz = 2
        case n2000Hz = 3
    }
    /// Bandwidth.
    public static func mt63Bandwidth(_ handle: SDRKIT_HANDLE) -> Result<Mt63Bandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(Mt63Bandwidth(rawValue: v) ?? Mt63Bandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63Bandwidth(_ handle: SDRKIT_HANDLE, _ value: Mt63Bandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// The options of the setting "interleave" of the stack "mt63".
    public enum Mt63Interleave: Int32 {
        case auto = 0
        case short = 1
        case long = 2
    }
    /// Interleave.
    public static func mt63Interleave(_ handle: SDRKIT_HANDLE) -> Result<Mt63Interleave, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "interleave", &v)
        return code == SDRKIT_OK ? .success(Mt63Interleave(rawValue: v) ?? Mt63Interleave(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63Interleave(_ handle: SDRKIT_HANDLE, _ value: Mt63Interleave) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "interleave", value.rawValue))
    }
    /// Listen.
    public static func mt63Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Lowest carrier.
    public static func mt63TxLowHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_low_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63TxLowHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_low_hz", value))
    }
    /// Level.
    public static func mt63TxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63TxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func mt63TxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63TxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "speed" of the stack "fsq".
    public enum FsqSpeed: Int32 {
        case auto = 0
        case n15 = 1
        case n2 = 2
        case n3 = 3
        case n45 = 4
        case n6 = 5
    }
    /// Speed.
    public static func fsqSpeed(_ handle: SDRKIT_HANDLE) -> Result<FsqSpeed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "speed", &v)
        return code == SDRKIT_OK ? .success(FsqSpeed(rawValue: v) ?? FsqSpeed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqSpeed(_ handle: SDRKIT_HANDLE, _ value: FsqSpeed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "speed", value.rawValue))
    }
    /// Listen.
    public static func fsqListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Audio centre.
    public static func fsqTxCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqTxCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value))
    }
    /// Level.
    public static func fsqTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func fsqTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// The options of the setting "speed" of the stack "ifkp".
    public enum IfkpSpeed: Int32 {
        case auto = 0
        case n05 = 1
        case n10 = 2
        case n20 = 3
    }
    /// Speed.
    public static func ifkpSpeed(_ handle: SDRKIT_HANDLE) -> Result<IfkpSpeed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "speed", &v)
        return code == SDRKIT_OK ? .success(IfkpSpeed(rawValue: v) ?? IfkpSpeed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpSpeed(_ handle: SDRKIT_HANDLE, _ value: IfkpSpeed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "speed", value.rawValue))
    }
    /// Listen.
    public static func ifkpListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Audio centre.
    public static func ifkpTxCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpTxCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value))
    }
    /// Level.
    public static func ifkpTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Tune time.
    public static func ifkpTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
    /// Picture filter.
    public static func sstvPictureFilter(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "picture_filter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvPictureFilter(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "picture_filter", value))
    }
    /// The options of the setting "demod" of the stack "sstv".
    public enum SstvDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func sstvDemod(_ handle: SDRKIT_HANDLE) -> Result<SstvDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(SstvDemod(rawValue: v) ?? SstvDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvDemod(_ handle: SDRKIT_HANDLE, _ value: SstvDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func sstvListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "tx_mode" of the stack "sstv".
    public enum SstvTxMode: Int32 {
        case robot36 = 0
        case robot72 = 1
        case martinM1 = 2
        case martinM2 = 3
        case scottieS1 = 4
        case scottieS2 = 5
        case scottieDX = 6
        case pD50 = 7
        case pD90 = 8
        case pD120 = 9
        case pD180 = 10
        case wraaseSC2180 = 11
    }
    /// Transmit mode.
    public static func sstvTxMode(_ handle: SDRKIT_HANDLE) -> Result<SstvTxMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_mode", &v)
        return code == SDRKIT_OK ? .success(SstvTxMode(rawValue: v) ?? SstvTxMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvTxMode(_ handle: SDRKIT_HANDLE, _ value: SstvTxMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_mode", value.rawValue))
    }
    /// The options of the setting "fit" of the stack "sstv".
    public enum SstvFit: Int32 {
        case bars = 0
        case crop = 1
        case stretch = 2
    }
    /// Fit.
    public static func sstvFit(_ handle: SDRKIT_HANDLE) -> Result<SstvFit, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fit", &v)
        return code == SDRKIT_OK ? .success(SstvFit(rawValue: v) ?? SstvFit(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvFit(_ handle: SDRKIT_HANDLE, _ value: SstvFit) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fit", value.rawValue))
    }
    /// Repeat.
    public static func sstvRepeat(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "repeat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvRepeat(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "repeat", value))
    }
    /// Pause between pictures.
    public static func sstvPauseS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pause_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvPauseS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pause_s", value))
    }
    /// Listen.
    public static func sitorListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSitorListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "demod" of the stack "ardop".
    public enum ArdopDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func ardopDemod(_ handle: SDRKIT_HANDLE) -> Result<ArdopDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(ArdopDemod(rawValue: v) ?? ArdopDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArdopDemod(_ handle: SDRKIT_HANDLE, _ value: ArdopDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func ardopListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArdopListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Leader.
    public static func ardopLeaderMs(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "leader_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArdopLeaderMs(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "leader_ms", value))
    }
    /// The options of the setting "fec_type" of the stack "ardop".
    public enum ArdopFecType: Int32 {
        case n4fsk20050s = 0
        case n4fsk500100s = 1
        case n4fsk500100 = 2
        case n4psk200100 = 3
        case n4psk200100s = 4
        case n8psk200100 = 5
        case n16qam200100 = 6
        case n4psk500100 = 7
        case n8psk500100 = 8
        case n16qam500100 = 9
        case n4psk1000100 = 10
        case n8psk1000100 = 11
        case n16qam1000100 = 12
        case n4psk2000100 = 13
        case n8psk2000100 = 14
        case n16qam2000100 = 15
    }
    /// FEC frame type.
    public static func ardopFecType(_ handle: SDRKIT_HANDLE) -> Result<ArdopFecType, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "fec_type", &v)
        return code == SDRKIT_OK ? .success(ArdopFecType(rawValue: v) ?? ArdopFecType(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArdopFecType(_ handle: SDRKIT_HANDLE, _ value: ArdopFecType) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "fec_type", value.rawValue))
    }
    /// Answer calls.
    public static func ardopAutoAnswer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_answer", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArdopAutoAnswer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_answer", value))
    }
    /// The options of the setting "demod" of the stack "ros".
    public enum RosDemod: Int32 {
        case usb = 0
        case lsb = 1
    }
    /// Demodulator.
    public static func rosDemod(_ handle: SDRKIT_HANDLE) -> Result<RosDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(RosDemod(rawValue: v) ?? RosDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRosDemod(_ handle: SDRKIT_HANDLE, _ value: RosDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func rosListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRosListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "variant" of the stack "throb".
    public enum ThrobVariant: Int32 {
        case auto = 0
        case throb1 = 1
        case throb2 = 2
        case throb4 = 3
        case throbx1 = 4
        case throbx2 = 5
        case throbx4 = 6
    }
    /// Variant.
    public static func throbVariant(_ handle: SDRKIT_HANDLE) -> Result<ThrobVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "variant", &v)
        return code == SDRKIT_OK ? .success(ThrobVariant(rawValue: v) ?? ThrobVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setThrobVariant(_ handle: SDRKIT_HANDLE, _ value: ThrobVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "variant", value.rawValue))
    }
    /// The options of the setting "demod" of the stack "throb".
    public enum ThrobDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func throbDemod(_ handle: SDRKIT_HANDLE) -> Result<ThrobDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(ThrobDemod(rawValue: v) ?? ThrobDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setThrobDemod(_ handle: SDRKIT_HANDLE, _ value: ThrobDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func throbListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThrobListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "submode" of the stack "qrss".
    public enum QrssSubmode: Int32 {
        case qrss = 0
        case dfcw = 1
    }
    /// Keying.
    public static func qrssSubmode(_ handle: SDRKIT_HANDLE) -> Result<QrssSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(QrssSubmode(rawValue: v) ?? QrssSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssSubmode(_ handle: SDRKIT_HANDLE, _ value: QrssSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Automatic centre and element length.
    public static func qrssAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto", value))
    }
    /// Tone.
    public static func qrssCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "centre_hz", value))
    }
    /// Element length.
    public static func qrssElementS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "element_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssElementS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "element_s", value))
    }
    /// Window width.
    public static func qrssSpanHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "span_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssSpanHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "span_hz", value))
    }
    /// Dash below the dot frequency.
    public static func qrssDashBelow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "dash_below", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssDashBelow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "dash_below", value))
    }
    /// The options of the setting "demod" of the stack "qrss".
    public enum QrssDemod: Int32 {
        case usb = 0
        case lsb = 1
        case nfm = 2
        case am = 3
    }
    /// Demodulator.
    public static func qrssDemod(_ handle: SDRKIT_HANDLE) -> Result<QrssDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(QrssDemod(rawValue: v) ?? QrssDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssDemod(_ handle: SDRKIT_HANDLE, _ value: QrssDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Listen.
    public static func qrssListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Fixed parameters.
    public static func cp16Fixed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "fixed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Fixed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "fixed", value))
    }
    /// Lowest carrier.
    public static func cp16BaseHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "base_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16BaseHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "base_hz", value))
    }
    /// Spacing.
    public static func cp16SpacingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "spacing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16SpacingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "spacing_hz", value))
    }
    /// Seconds a character.
    public static func cp16CharS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "char_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16CharS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "char_s", value))
    }
    /// The options of the setting "sideband" of the stack "cp16".
    public enum Cp16Sideband: Int32 {
        case auto = 0
        case upper = 1
        case lower = 2
    }
    /// Sideband.
    public static func cp16Sideband(_ handle: SDRKIT_HANDLE) -> Result<Cp16Sideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sideband", &v)
        return code == SDRKIT_OK ? .success(Cp16Sideband(rawValue: v) ?? Cp16Sideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Sideband(_ handle: SDRKIT_HANDLE, _ value: Cp16Sideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sideband", value.rawValue))
    }
    /// Read characters.
    public static func cp16Read(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "read", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Read(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "read", value))
    }
    /// Font file.
    public static func cp16Font(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "font", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Font(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "font", $0) })
    }
    /// Listen.
    public static func cp16Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Listen.
    public static func winmorListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWinmorListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum HfkbdGeneratorSettings {
    /// Test text.
    public static func rttyMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Lower tone.
    public static func rttyLowerHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lower_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRttyLowerHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lower_hz", value))
    }
    /// Test text.
    public static func pskMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPskMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Audio frequency.
    public static func pskFreqHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_freq_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPskFreqHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_freq_hz", value))
    }
    /// Test text.
    public static func cwMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCwMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Carrier.
    public static func cwCarrierHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwCarrierHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_hz", value))
    }
    /// Speed.
    public static func cwWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm", value))
    }
    /// Farnsworth speed.
    public static func cwFarnsworthWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_farnsworth_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwFarnsworthWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_farnsworth_wpm", value))
    }
    /// Jitter.
    public static func cwJitter(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_jitter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwJitter(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_jitter", value))
    }
    /// Carriers.
    public static func cwSignals(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_signals", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwSignals(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_signals", value))
    }
    /// Top speed.
    public static func cwWpmMax(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm_max", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwWpmMax(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm_max", value))
    }
    /// Level spread.
    public static func cwLevelSpreadDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_level_spread_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwLevelSpreadDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_level_spread_db", value))
    }
    /// Noise only.
    public static func cwNoiseOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_noise_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwNoiseOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_noise_only", value))
    }
    /// Test text.
    public static func oliviaMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setOliviaMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Test text.
    public static func contestiaMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setContestiaMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Test text.
    public static func mfskMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Picture width.
    public static func mfskPictureWidth(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_picture_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskPictureWidth(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_picture_width", value))
    }
    /// Picture height.
    public static func mfskPictureHeight(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_picture_height", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMfskPictureHeight(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_picture_height", value))
    }
    /// Test text.
    public static func dominoexMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDominoexMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Test text.
    public static func thorMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setThorMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Test text.
    public static func hellMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHellMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Test text.
    public static func mt63Message(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63Message(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Audio centre.
    public static func mt63CentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMt63CentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test text.
    public static func fsqMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Audio centre.
    public static func fsqCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsqCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test text.
    public static func ifkpMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Audio centre.
    public static func ifkpCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIfkpCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// The options of the setting "gen_mode" of the stack "sstv".
    public enum SstvMode: Int32 {
        case robot36 = 0
        case robot72 = 1
        case martinM1 = 2
        case martinM2 = 3
        case scottieS1 = 4
        case scottieS2 = 5
        case scottieDX = 6
        case pD50 = 7
        case pD90 = 8
        case pD120 = 9
        case pD180 = 10
        case wraaseSC2180 = 11
    }
    /// Picture mode.
    public static func sstvMode(_ handle: SDRKIT_HANDLE) -> Result<SstvMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(SstvMode(rawValue: v) ?? SstvMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvMode(_ handle: SDRKIT_HANDLE, _ value: SstvMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// Send the header.
    public static func sstvVis(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_vis", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvVis(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_vis", value))
    }
    /// The options of the setting "gen_picture" of the stack "sstv".
    public enum SstvPicture: Int32 {
        case card = 0
        case texture = 1
    }
    /// Test picture.
    public static func sstvPicture(_ handle: SDRKIT_HANDLE) -> Result<SstvPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &v)
        return code == SDRKIT_OK ? .success(SstvPicture(rawValue: v) ?? SstvPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSstvPicture(_ handle: SDRKIT_HANDLE, _ value: SstvPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_picture", value.rawValue))
    }
    /// Test text.
    public static func sitorMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSitorMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Lower tone.
    public static func sitorLowerHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lower_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSitorLowerHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lower_hz", value))
    }
    /// The options of the setting "gen_data" of the stack "ardop".
    public enum ArdopData: Int32 {
        case header = 0
        case n4psk200100 = 1
        case n4psk200100s = 2
        case n8psk200100 = 3
        case n16qam200100 = 4
        case n4psk500100 = 5
        case n8psk500100 = 6
        case n16qam500100 = 7
        case n4psk1000100 = 8
        case n8psk1000100 = 9
        case n16qam1000100 = 10
        case n4psk2000100 = 11
        case n8psk2000100 = 12
        case n16qam2000100 = 13
    }
    /// Last frame.
    public static func ardopData(_ handle: SDRKIT_HANDLE) -> Result<ArdopData, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_data", &v)
        return code == SDRKIT_OK ? .success(ArdopData(rawValue: v) ?? ArdopData(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArdopData(_ handle: SDRKIT_HANDLE, _ value: ArdopData) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_data", value.rawValue))
    }
    /// The options of the setting "gen_signal" of the stack "ros".
    public enum RosSignal: Int32 {
        case grid = 0
        case noise = 1
        case fsk = 2
        case carrier = 3
        case sweep = 4
    }
    /// Test signal.
    public static func rosSignal(_ handle: SDRKIT_HANDLE) -> Result<RosSignal, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &v)
        return code == SDRKIT_OK ? .success(RosSignal(rawValue: v) ?? RosSignal(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRosSignal(_ handle: SDRKIT_HANDLE, _ value: RosSignal) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_signal", value.rawValue))
    }
    /// Symbol rate.
    public static func rosBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRosBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud", value))
    }
    /// Test text.
    public static func throbMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setThrobMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// The options of the setting "gen_variant" of the stack "throb".
    public enum ThrobVariant: Int32 {
        case throb1 = 0
        case throb2 = 1
        case throb4 = 2
        case throbx1 = 3
        case throbx2 = 4
        case throbx4 = 5
    }
    /// Test signal variant.
    public static func throbVariant(_ handle: SDRKIT_HANDLE) -> Result<ThrobVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &v)
        return code == SDRKIT_OK ? .success(ThrobVariant(rawValue: v) ?? ThrobVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setThrobVariant(_ handle: SDRKIT_HANDLE, _ value: ThrobVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_variant", value.rawValue))
    }
    /// Audio centre.
    public static func throbCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThrobCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test text.
    public static func qrssMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_message", $0) })
    }
    /// Test signal element length.
    public static func qrssElementS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_element_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssElementS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_element_s", value))
    }
    /// Test signal DFCW shift.
    public static func qrssShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQrssShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value))
    }
    /// Test signal seconds a character.
    public static func cp16CharS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_char_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16CharS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_char_s", value))
    }
    /// Test signal spacing.
    public static func cp16SpacingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spacing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16SpacingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spacing_hz", value))
    }
    /// Test signal on the lower sideband.
    public static func cp16Lsb(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_lsb", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Lsb(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_lsb", value))
    }
    /// Test signal lowest carrier.
    public static func cp16BaseHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_base_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16BaseHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_base_hz", value))
    }
    /// The options of the setting "gen_text" of the stack "cp16".
    public enum Cp16Text: Int32 {
        case letters = 0
        case chinese = 1
    }
    /// Test signal text.
    public static func cp16Text(_ handle: SDRKIT_HANDLE) -> Result<Cp16Text, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(Cp16Text(rawValue: v) ?? Cp16Text(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCp16Text(_ handle: SDRKIT_HANDLE, _ value: Cp16Text) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_text", value.rawValue))
    }
    /// Frame-type tones.
    public static func winmorTones(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tones", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWinmorTones(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tones", $0) })
    }
    /// Tuning error.
    public static func winmorErrorHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_error_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWinmorErrorHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_error_hz", value))
    }
}

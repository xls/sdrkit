// sdrkit: the typed interface of the stack family "hfdata", version 1.7. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum HfdataInterface {
    public static let major = 1
    public static let minor = 7
}

/// The result fields of the CROWD36 stack.
public struct Crowd36Fields {
    /// What no public document confirms.
    public var note: String
    /// The text read, the last characters.
    public var text: String
    /// The search state: 0 searching, 1 tones found, 2 decoding.
    public var state: Int64
    /// The offset of the tones from their nominal place. Absent while the tones are not found.
    public var offsetHz: Double?
    /// The SNR in 3 kHz of the last fit. Absent while the tones are not found.
    public var snrDb: Double?
    /// The sync tones of the last sync, after the correction. Absent before a sync.
    public var syncTones: String?
    /// Characters read.
    public var characters: Int64
    /// Text lines read.
    public var good: Int64
    /// Tones read outside the character tables.
    public var bad: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double
    /// Text lines refused by the check of their form.
    public var refused: Int64

    init(_ c: sdrkit_crowd36_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        state = c.state
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        syncTones = c.has_sync_tones ? sdrkitText(c.sync_tones) : nil
        characters = c.characters
        good = c.good
        bad = c.bad
        levelDbfs = c.level_dbfs
        refused = c.refused
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Crowd36Fields, SDRKitCode> {
        var c = sdrkit_crowd36_fields()
        let code = sdrkit_crowd36_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Crowd36Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct HfdataDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_hfdata_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfdataDecoderFields, SDRKitCode> {
        var c = sdrkit_hfdata_decoder_fields()
        let code = sdrkit_hfdata_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfdataDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of a number-sender stack: one thing read. The row key is the count of rows before it, as six digits.
public struct HfdataSenderRow {
    /// The row key.
    public var key: String
    /// The time it was read.
    public var time: Double
    /// What it is: start, message start, groups, block sync, break, end, sync, text.
    public var kind: String
    /// The signal and speed it was read as.
    public var id: String
    /// What was read.
    public var text: String
    /// The SNR in 3 kHz of the last fit of the tones. Absent without a fit.
    public var snr: Double?

    init(_ c: sdrkit_hfdata_sender_row) {
        key = sdrkitText(c.key)
        time = c.time
        kind = sdrkitText(c.kind)
        id = sdrkitText(c.id)
        text = sdrkitText(c.text)
        snr = c.has_snr ? c.snr : nil
    }
}

/// The result fields of the XPA stack.
public struct XpaFields {
    /// What no public document confirms.
    public var note: String
    /// The groups read, the last characters.
    public var text: String
    /// The search state: 0 searching, 1 tones found, 2 decoding.
    public var state: Int64
    /// The offset of the tones from their nominal place. Absent while the tones are not found.
    public var offsetHz: Double?
    /// The SNR in 3 kHz of the last fit. Absent while the tones are not found.
    public var snrDb: Double?
    /// The speed of the reading that won. Absent before a reading wins.
    public var speed: Double?
    /// The sideband of the reading that won: "upper" or "lower (tones reversed)".
    public var sideband: String?
    /// The numbering of the reading that won: "as found" or "moved by +N tones".
    public var numbering: String?
    /// Groups read.
    public var groups: Int64
    /// Lines of groups read.
    public var good: Int64
    /// Tones read without a meaning.
    public var bad: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_xpa_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        state = c.state
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        speed = c.has_speed ? c.speed : nil
        sideband = c.has_sideband ? sdrkitText(c.sideband) : nil
        numbering = c.has_numbering ? sdrkitText(c.numbering) : nil
        groups = c.groups
        good = c.good
        bad = c.bad
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<XpaFields, SDRKitCode> {
        var c = sdrkit_xpa_fields()
        let code = sdrkit_xpa_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(XpaFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the FSK200 stack. A field that the submode in use does not count has no value.
public struct Fsk200Fields {
    /// What no public document confirms.
    public var note: String
    /// The text and blocks read, the last characters.
    public var text: String
    /// The search state: 0 searching, 1 tones found, 2 decoding.
    public var state: Int64
    /// The centre of the tone pair above the dial. Absent while the tones are not found.
    public var offsetHz: Double?
    /// The SNR in 3 kHz of the last fit. Absent while the tones are not found.
    public var snrDb: Double?
    /// The tones from the dial and their sense. Absent while the tones are not found.
    public var tones: String?
    /// The traffic kind of f06: F06 figure groups, ASCII or F06a file.
    public var traffic: String?
    /// Characters read (f01).
    public var characters: Int64?
    /// Characters with a stop unit of space (f01).
    public var framingErrors: Int64?
    /// Blocks whose CRC holds, and dividers (f06).
    public var blocksGood: Int64?
    /// Blocks corrected by one bit (f06).
    public var blocksCorrected: Int64?
    /// Blocks with a CRC error (f06).
    public var crcErrors: Int64?
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_fsk200_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        state = c.state
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        tones = c.has_tones ? sdrkitText(c.tones) : nil
        traffic = c.has_traffic ? sdrkitText(c.traffic) : nil
        characters = c.has_characters ? c.characters : nil
        framingErrors = c.has_framing_errors ? c.framing_errors : nil
        blocksGood = c.has_blocks_good ? c.blocks_good : nil
        blocksCorrected = c.has_blocks_corrected ? c.blocks_corrected : nil
        crcErrors = c.has_crc_errors ? c.crc_errors : nil
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Fsk200Fields, SDRKitCode> {
        var c = sdrkit_fsk200_fields()
        let code = sdrkit_fsk200_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Fsk200Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the GW FSK stack.
public struct GwfskFields {
    /// What no public document confirms.
    public var note: String
    /// The state: 0 nothing heard, 1 bursts heard, 2 bursts read.
    public var state: Int64
    /// The tones of the last burst whose tones were found.
    public var tones: String?
    /// The code of the last shore station heard.
    public var lastStation: String?
    /// Bursts analysed.
    public var bursts: Int64
    /// Bursts with tones that were not understood.
    public var notUnderstood: Int64
    /// Packets read.
    public var packets: Int64
    /// Channel markers read.
    public var markers: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_gwfsk_fields) {
        note = sdrkitText(c.note)
        state = c.state
        tones = c.has_tones ? sdrkitText(c.tones) : nil
        lastStation = c.has_last_station ? sdrkitText(c.last_station) : nil
        bursts = c.bursts
        notUnderstood = c.not_understood
        packets = c.packets
        markers = c.markers
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GwfskFields, SDRKitCode> {
        var c = sdrkit_gwfsk_fields()
        let code = sdrkit_gwfsk_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GwfskFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the telephone FSK modem stack.
public struct FskModemsFields {
    /// What no public document confirms.
    public var note: String
    /// The state: 0 nothing heard, 1 a burst is open, 2 messages read.
    public var state: Int64
    /// The standard of the last message.
    public var standard: String?
    /// The tones, the sideband and the dial error of the last message.
    public var tones: String?
    /// The bytes, framing errors and length of the last message.
    public var lastMessage: String?
    /// Messages read.
    public var messages: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_fsk_modems_fields) {
        note = sdrkitText(c.note)
        state = c.state
        standard = c.has_standard ? sdrkitText(c.standard) : nil
        tones = c.has_tones ? sdrkitText(c.tones) : nil
        lastMessage = c.has_last_message ? sdrkitText(c.last_message) : nil
        messages = c.messages
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FskModemsFields, SDRKitCode> {
        var c = sdrkit_fsk_modems_fields()
        let code = sdrkit_fsk_modems_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FskModemsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the UVB-76 buzzer monitor. The measures of the train are absent while no train is locked.
public struct Uvb76Fields {
    /// What the stack does not do and what no public document confirms.
    public var note: String
    /// The state: 0 no carrier, 1 carrier, 2 buzz train locked.
    public var state: Int64
    /// A carrier is in the channel.
    public var carrier: Bool
    /// Buzzes heard.
    public var buzzes: Int64
    /// The buzz rate, 60 over the median period.
    public var ratePerMin: Double?
    /// The median buzz length.
    public var buzzS: Double?
    /// The median period.
    public var periodS: Double?
    /// The median distance of the periods from their median.
    public var spreadMs: Double?
    /// Train breaks logged.
    public var breaks: Int64
    /// Long transmissions logged.
    public var longTransmissions: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_uvb76_fields) {
        note = sdrkitText(c.note)
        state = c.state
        carrier = c.carrier
        buzzes = c.buzzes
        ratePerMin = c.has_rate_per_min ? c.rate_per_min : nil
        buzzS = c.has_buzz_s ? c.buzz_s : nil
        periodS = c.has_period_s ? c.period_s : nil
        spreadMs = c.has_spread_ms ? c.spread_ms : nil
        breaks = c.breaks
        longTransmissions = c.long_transmissions
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Uvb76Fields, SDRKitCode> {
        var c = sdrkit_uvb76_fields()
        let code = sdrkit_uvb76_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Uvb76Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the VLF MSK monitor. The measures are absent while no transmitter is on.
public struct VlfMskFields {
    /// What the stack does not do and what no public document confirms.
    public var note: String
    /// The state: 0 off air or too weak, 1 carrier only, 2 MSK.
    public var state: Int64
    /// The class: "MSK 200 bit/s", "MSK about N bit/s", "carrier only" or "off air or too weak".
    public var signalClass: String
    /// The carrier from the tuned frequency.
    public var carrierHz: Double?
    /// The bit rate.
    public var bitRate: Double?
    /// The spacing of the two lines of the squared signal.
    public var lineSpacingHz: Double?
    /// The level over the noise.
    public var levelDb: Double?
    /// The listed station near the carrier: "likely NAME".
    public var hint: String?
    /// Transmitters that came on.
    public var transmitters: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_vlf_msk_fields) {
        note = sdrkitText(c.note)
        state = c.state
        signalClass = sdrkitText(c.signal_class)
        carrierHz = c.has_carrier_hz ? c.carrier_hz : nil
        bitRate = c.has_bit_rate ? c.bit_rate : nil
        lineSpacingHz = c.has_line_spacing_hz ? c.line_spacing_hz : nil
        levelDb = c.has_level_db ? c.level_db : nil
        hint = c.has_hint ? sdrkitText(c.hint) : nil
        transmitters = c.transmitters
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<VlfMskFields, SDRKitCode> {
        var c = sdrkit_vlf_msk_fields()
        let code = sdrkit_vlf_msk_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(VlfMskFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of a teleprinter stack.
public struct HfdataTeleprinterFields {
    /// What no public document confirms.
    public var note: String
    /// The text received, the last characters.
    public var text: String
    /// The search state: 0 searching, 1 signal found, 2 decoding.
    public var state: Int64
    /// The speed of the signal. Absent while the front has no lock.
    public var baud: Double?
    /// The centre of the signal above the dial. Absent while the front has no lock.
    public var centreHz: Double?
    /// The shift between the tones. Absent while the front has no lock or the signal has no shift.
    public var shiftHz: Double?
    /// What was found beside the speed (cycle, depth, tone length). Empty while not decoding.
    public var detail: String
    /// Words, characters or frames read.
    public var good: Int64
    /// Words, characters or frames that failed their check.
    public var bad: Int64
    /// The level of the channel, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_hfdata_teleprinter_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        state = c.state
        baud = c.has_baud ? c.baud : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        shiftHz = c.has_shift_hz ? c.shift_hz : nil
        detail = sdrkitText(c.detail)
        good = c.good
        bad = c.bad
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfdataTeleprinterFields, SDRKitCode> {
        var c = sdrkit_hfdata_teleprinter_fields()
        let code = sdrkit_hfdata_teleprinter_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfdataTeleprinterFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A telegram of EFR. The row key is the number of the telegram.
public struct EfrTelegramRow {
    /// The row key.
    public var key: String
    /// The time a telegram ended.
    public var time: Double
    /// The checksum: "good" or "bad".
    public var checksum: String
    /// The message number (user byte 1).
    public var message: Int64
    /// Address 1 (user byte 2).
    public var address1: Int64
    /// Address 2 (user byte 3).
    public var address2: Int64
    /// The number of payload bytes.
    public var payloadBytes: Int64
    /// The payload as hex.
    public var payload: String

    init(_ c: sdrkit_efr_telegram_row) {
        key = sdrkitText(c.key)
        time = c.time
        checksum = sdrkitText(c.checksum)
        message = c.message
        address1 = c.address1
        address2 = c.address2
        payloadBytes = c.payload_bytes
        payload = sdrkitText(c.payload)
    }
}

/// The result fields of 3G-ALE.
public struct Ale3gFields {
    /// The parts that no public document confirms.
    public var note: String
    /// The text received, the last lines.
    public var text: String
    /// Bursts read.
    public var burstsTotal: Int64
    /// PDUs with a good check.
    public var pdusGood: Int64
    /// The signal-to-noise ratio of the last burst. Absent without a measure.
    public var snrDb: Double?
    /// The frequency error of the last burst. Absent without a measure.
    public var offsetHz: Double?

    init(_ c: sdrkit_ale3g_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        burstsTotal = c.bursts_total
        pdusGood = c.pdus_good
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Ale3gFields, SDRKitCode> {
        var c = sdrkit_ale3g_fields()
        let code = sdrkit_ale3g_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Ale3gFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A PDU of 3G-ALE. The row key is the number of the PDU.
public struct Ale3gPduRow {
    /// The row key.
    public var key: String
    /// The UTC of the PDU.
    public var utc: Double
    /// The burst (BW0 to BW4).
    public var burst: String
    /// The PDU type.
    public var type: String
    /// The check of the PDU passed.
    public var crcGood: Bool
    /// The PDU in words.
    public var words: String
    /// The signal-to-noise ratio of the burst.
    public var snrDb: Double
    /// The frequency error of the burst.
    public var offsetHz: Double
    /// The data bytes of a packet or datagram.
    public var data: String

    init(_ c: sdrkit_ale3g_pdu_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        burst = sdrkitText(c.burst)
        type = sdrkitText(c.type)
        crcGood = c.crc_good
        words = sdrkitText(c.words)
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        data = sdrkitText(c.data)
    }
}

/// The result fields of STANAG 4481.
public struct S4481Fields {
    /// What no public document confirms.
    public var note: String
    /// The text received, the last characters.
    public var text: String
    /// What is received: one channel, sub-carriers, or nothing.
    public var reception: String

    init(_ c: sdrkit_s4481_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        reception = sdrkitText(c.reception)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<S4481Fields, SDRKitCode> {
        var c = sdrkit_s4481_fields()
        let code = sdrkit_s4481_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(S4481Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A channel of STANAG 4481: the single channel or a sub-carrier. The row key is the number of the channel.
public struct S4481ChannelRow {
    /// The row key.
    public var key: String
    /// The number of the channel (0: the single channel).
    public var channel: Int64
    /// The mark tone.
    public var markHz: Double
    /// The space tone.
    public var spaceHz: Double
    /// The channel carries a signal.
    public var on: Bool
    /// The speed of the channel. Absent until it is found.
    public var speedBd: Double?
    /// "start-stop" or "sync".
    public var framing: String
    /// Characters read.
    public var characters: Int64
    /// Characters with a framing error.
    public var framingErrors: Int64
    /// Bits received.
    public var bits: Int64
    /// The signal-to-noise ratio of the channel.
    public var snrDb: Double
    /// The last text of the channel.
    public var lastText: String

    init(_ c: sdrkit_s4481_channel_row) {
        key = sdrkitText(c.key)
        channel = c.channel
        markHz = c.mark_hz
        spaceHz = c.space_hz
        on = c.on
        speedBd = c.has_speed_bd ? c.speed_bd : nil
        framing = sdrkitText(c.framing)
        characters = c.characters
        framingErrors = c.framing_errors
        bits = c.bits
        snrDb = c.snr_db
        lastText = sdrkitText(c.last_text)
    }
}

/// The result fields of Link 11.
public struct Link11Fields {
    /// The parts that no public document confirms.
    public var note: String
    /// The text received, the last lines.
    public var text: String
    /// Messages since the last Clear.
    public var messagesTotal: Int64
    /// The Doppler offset of the last message. Absent without a measure.
    public var offsetHz: Double?

    init(_ c: sdrkit_link11_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        messagesTotal = c.messages_total
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Link11Fields, SDRKitCode> {
        var c = sdrkit_link11_fields()
        let code = sdrkit_link11_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Link11Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of Link 11. The row key is the number of the message.
public struct Link11MessageRow {
    /// The row key.
    public var key: String
    /// The start of the message, seconds.
    public var startS: Double
    /// The frame rate: "fast" or "slow".
    public var rate: String
    /// Preamble frames.
    public var preambleFrames: Int64
    /// Data frames.
    public var dataFrames: Int64
    /// The duration, seconds.
    public var durationS: Double
    /// The Doppler offset.
    public var dopplerHz: Double
    /// The constellation offset, degrees.
    public var offsetDeg: Double
    /// The signal-to-noise ratio of the message.
    public var snrDb: Double

    init(_ c: sdrkit_link11_message_row) {
        key = sdrkitText(c.key)
        startS = c.start_s
        rate = sdrkitText(c.rate)
        preambleFrames = c.preamble_frames
        dataFrames = c.data_frames
        durationS = c.duration_s
        dopplerHz = c.doppler_hz
        offsetDeg = c.offset_deg
        snrDb = c.snr_db
    }
}

/// The result fields of the CIS stack. A field that the submode in use does not measure has no value.
public struct CisHfFields {
    /// What no public document confirms.
    public var note: String
    /// The text received, the last characters.
    public var text: String
    /// The search state: 0 searching, 1 signal found, 2 decoding (or identified).
    public var state: Int64
    /// What was found, in one line.
    public var found: String
    /// The name of the signal identified, or empty.
    public var identified: String
    /// Characters, words or frames read.
    public var good: Int64
    /// Characters, words or frames that failed their check.
    public var bad: Int64
    /// The frequency error. Absent without a measure.
    public var offsetHz: Double?
    /// The signal-to-noise ratio. Absent without a measure.
    public var snrDb: Double?
    /// The centre of the signal above the dial (2-FSK and CIS-3000).
    public var centreHz: Double?
    /// The shift between the tones (2-FSK).
    public var shiftHz: Double?
    /// The eye opening of the 2-FSK demodulators.
    public var eye: Double?
    /// Frames read (CIS-14).
    public var frames: Int64?
    /// The state bits of the last frames (CIS-14), raw.
    public var stateBits: String?
    /// The pilot (CIS-12).
    public var pilotHz: Double?
    /// Carriers seen (CIS-12, OFDM).
    public var carriers: Int64?
    /// The symbol rate.
    public var symbolRate: Double?
    /// The modulation found.
    public var modulation: String?
    /// The moment of the 2nd power.
    public var m2: Double?
    /// The moment of the 4th power.
    public var m4: Double?
    /// The moment of the 8th power.
    public var m8: Double?
    /// Tones seen (CIS-36).
    public var tones: Int64?
    /// The spacing of the tones or carriers.
    public var spacingHz: Double?
    /// The lowest tone or band edge.
    public var lowHz: Double?
    /// The highest tone or band edge.
    public var highHz: Double?
    /// The span of the tones (CIS-36).
    public var spanHz: Double?
    /// Double gaps between tones (CIS-36).
    public var doubleGaps: Int64?
    /// The useful part of an OFDM symbol.
    public var usefulMs: Double?
    /// The guard of an OFDM symbol.
    public var guardMs: Double?
    /// The centre carrier of the OFDM signal is off.
    public var centreOff: Bool?
    /// A pilot at 3300 Hz is there.
    public var pilot3300: Bool?
    /// The spectral line of the 2nd power.
    public var line2Db: Double?
    /// The spectral line of the 4th power.
    public var line4Db: Double?
    /// The spectral line of the 8th power.
    public var line8Db: Double?
    /// The spectral line at the symbol rate.
    public var rateLineDb: Double?

    init(_ c: sdrkit_cis_hf_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        state = c.state
        found = sdrkitText(c.found)
        identified = sdrkitText(c.identified)
        good = c.good
        bad = c.bad
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        shiftHz = c.has_shift_hz ? c.shift_hz : nil
        eye = c.has_eye ? c.eye : nil
        frames = c.has_frames ? c.frames : nil
        stateBits = c.has_state_bits ? sdrkitText(c.state_bits) : nil
        pilotHz = c.has_pilot_hz ? c.pilot_hz : nil
        carriers = c.has_carriers ? c.carriers : nil
        symbolRate = c.has_symbol_rate ? c.symbol_rate : nil
        modulation = c.has_modulation ? sdrkitText(c.modulation) : nil
        m2 = c.has_m2 ? c.m2 : nil
        m4 = c.has_m4 ? c.m4 : nil
        m8 = c.has_m8 ? c.m8 : nil
        tones = c.has_tones ? c.tones : nil
        spacingHz = c.has_spacing_hz ? c.spacing_hz : nil
        lowHz = c.has_low_hz ? c.low_hz : nil
        highHz = c.has_high_hz ? c.high_hz : nil
        spanHz = c.has_span_hz ? c.span_hz : nil
        doubleGaps = c.has_double_gaps ? c.double_gaps : nil
        usefulMs = c.has_useful_ms ? c.useful_ms : nil
        guardMs = c.has_guard_ms ? c.guard_ms : nil
        centreOff = c.has_centre_off ? c.centre_off : nil
        pilot3300 = c.has_pilot_3300 ? c.pilot_3300 : nil
        line2Db = c.has_line2_db ? c.line2_db : nil
        line4Db = c.has_line4_db ? c.line4_db : nil
        line8Db = c.has_line8_db ? c.line8_db : nil
        rateLineDb = c.has_rate_line_db ? c.rate_line_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CisHfFields, SDRKitCode> {
        var c = sdrkit_cis_hf_fields()
        let code = sdrkit_cis_hf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CisHfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A `bee` message of CIS-36-50. The row key is the number of the message.
public struct CisHfMessageRow {
    /// The row key.
    public var key: String
    /// The time the message ended.
    public var time: Double
    /// The speed of the message.
    public var speed: Double
    /// The start word.
    public var startWord: String
    /// The session key.
    public var sessionKey: String
    /// Characters read.
    public var characters: Int64
    /// Characters that failed the 3:4 check.
    public var failing: Int64
    /// What ended the message.
    public var endedBy: String

    init(_ c: sdrkit_cis_hf_message_row) {
        key = sdrkitText(c.key)
        time = c.time
        speed = c.speed
        startWord = sdrkitText(c.start_word)
        sessionKey = sdrkitText(c.session_key)
        characters = c.characters
        failing = c.failing
        endedBy = sdrkitText(c.ended_by)
    }
}

/// A carrier of CIS-12. The row key is the number of the carrier.
public struct CisHfCarrierRow {
    /// The row key.
    public var key: String
    /// The carrier number (1 to 12).
    public var number: Int64
    /// The carrier frequency.
    public var freqHz: Double
    /// The carrier is on.
    public var on: Bool

    init(_ c: sdrkit_cis_hf_carrier_row) {
        key = sdrkitText(c.key)
        number = c.number
        freqHz = c.freq_hz
        on = c.on
    }
}

/// The result fields of the HF signal identifier: the measures of the last analysis.
public struct HfidFields {
    public enum Best: Int32 {
        case serial2400 = 0
        case stanag4285 = 1
        case mil110a = 2
        case mil110b = 3
        case stanag4481 = 4
        case ale3g = 5
        case link11 = 6
        case ale2g = 7
        case cis36 = 8
        case multiTone = 9
        case pactor1 = 10
        case pactor2 = 11
        case buzzer = 12
        case carrier = 13
        case unclassified = 14
    }
    /// A signal was found by the last analysis.
    public var signal: Bool
    /// The best candidate.
    public var best: Best?
    /// The confidence of the best candidate, 0 to 95 %.
    public var bestConfidencePct: Double?
    /// The analyses run.
    public var analyses: Int64
    /// The lower edge of the occupied band.
    public var bandLow: Double?
    /// The upper edge of the occupied band.
    public var bandHigh: Double?
    /// The signal-to-noise ratio in the band. Absent without a signal.
    public var snrDb: Double?
    /// The noise floor.
    public var noiseDbfs: Double?
    /// The steady carrier line.
    public var carrierLine: Double?
    /// The carrier line over its surroundings.
    public var carrierLineDb: Double?
    /// The 605 Hz tone over its surroundings.
    public var tone605Db: Double?
    /// A tone at 2915 Hz.
    public var tone2915: Bool
    /// The tone comb spacing.
    public var combSpacing: Double?
    /// The regularity of the comb, 0 to 100 %.
    public var combRegularityPct: Double?
    /// The tones of the comb.
    public var combTones: Int64?
    /// The two-tone shift.
    public var shift: Double?
    /// The symbol rate at the clock line.
    public var symbolRateHz: Double?
    /// The clock line over its surroundings.
    public var clockLineDb: Double?
    /// The slow envelope modulation.
    public var envelopeModulation: Double?
    /// The envelope modulation line over its surroundings.
    public var envelopeModulationDb: Double?
    /// The bursts in the analysed audio (0: continuous).
    public var bursts: Int64?
    /// The part of the time with a burst on.
    public var dutyPct: Double?
    /// The constellation order at the clock: 2, 4, 8, or 99 for higher or unclear.
    public var order: Int64?
    /// The repetition period in symbols.
    public var repeatSymbols: Int64?
    /// The repetition period.
    public var repeatPeriod: Double?
    /// The correlation at the repetition period, 0 to 100 %.
    public var repeatCorrelationPct: Double?
    /// The measured parameters as lines of text.
    public var measured: String
    /// The level of the audio, dBFS.
    public var levelDbfs: Double
    /// The audio rate of the analysis.
    public var audioRate: Double

    init(_ c: sdrkit_hfid_fields) {
        signal = c.signal
        best = c.has_best ? Best(rawValue: c.best) : nil
        bestConfidencePct = c.has_best_confidence_pct ? c.best_confidence_pct : nil
        analyses = c.analyses
        bandLow = c.has_band_low ? c.band_low : nil
        bandHigh = c.has_band_high ? c.band_high : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        carrierLine = c.has_carrier_line ? c.carrier_line : nil
        carrierLineDb = c.has_carrier_line_db ? c.carrier_line_db : nil
        tone605Db = c.has_tone_605_db ? c.tone_605_db : nil
        tone2915 = c.tone_2915
        combSpacing = c.has_comb_spacing ? c.comb_spacing : nil
        combRegularityPct = c.has_comb_regularity_pct ? c.comb_regularity_pct : nil
        combTones = c.has_comb_tones ? c.comb_tones : nil
        shift = c.has_shift ? c.shift : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        clockLineDb = c.has_clock_line_db ? c.clock_line_db : nil
        envelopeModulation = c.has_envelope_modulation ? c.envelope_modulation : nil
        envelopeModulationDb = c.has_envelope_modulation_db ? c.envelope_modulation_db : nil
        bursts = c.has_bursts ? c.bursts : nil
        dutyPct = c.has_duty_pct ? c.duty_pct : nil
        order = c.has_order ? c.order : nil
        repeatSymbols = c.has_repeat_symbols ? c.repeat_symbols : nil
        repeatPeriod = c.has_repeat_period ? c.repeat_period : nil
        repeatCorrelationPct = c.has_repeat_correlation_pct ? c.repeat_correlation_pct : nil
        measured = sdrkitText(c.measured)
        levelDbfs = c.level_dbfs
        audioRate = c.audio_rate
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfidFields, SDRKitCode> {
        var c = sdrkit_hfid_fields()
        let code = sdrkit_hfid_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfidFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A candidate of the last analysis of the identifier. The row key is the place, "0" first.
public struct HfidCandidateRow {
    public enum Waveform: Int32 {
        case serial2400 = 0
        case stanag4285 = 1
        case mil110a = 2
        case mil110b = 3
        case stanag4481 = 4
        case ale3g = 5
        case link11 = 6
        case ale2g = 7
        case cis36 = 8
        case multiTone = 9
        case pactor1 = 10
        case pactor2 = 11
        case buzzer = 12
        case carrier = 13
        case unclassified = 14
    }
    /// The row key.
    public var key: String
    /// The waveform.
    public var waveform: Waveform?
    /// The confidence of the estimate, 0 to 95 %.
    public var confidencePct: Double
    /// What the score is based on.
    public var basis: String
    /// What the waveform is.
    public var note: String
    /// A public source.
    public var link: String

    init(_ c: sdrkit_hfid_candidate_row) {
        key = sdrkitText(c.key)
        waveform = Waveform(rawValue: c.waveform)
        confidencePct = c.confidence_pct
        basis = sdrkitText(c.basis)
        note = sdrkitText(c.note)
        link = sdrkitText(c.link)
    }
}

/// The result fields of ALE.
public struct AleFields {
    public enum Clock: Int32 {
        case system = 0
        case recording = 1
        case stream = 2
    }
    /// In word sync (the lock state "Sync").
    public var inSync: Bool
    /// A word was decoded in the last 60 s.
    public var wordsRecent: Bool
    /// Words decoded.
    public var wordsOk: Int64
    /// Words missed.
    public var wordsMissed: Int64
    /// Calls in the event log.
    public var calls: Int64
    /// Stations heard.
    public var stations: Int64
    /// The frequency correction.
    public var offsetHz: Double
    /// The signal-to-noise ratio of the last call. Absent before a call.
    public var snrDb: Double?
    /// The bit error rate of the last call. Absent before a call.
    public var ber: Double?
    /// The level of the audio, dBFS.
    public var levelDbfs: Double
    /// Where the times come from.
    public var clock: Clock?

    init(_ c: sdrkit_ale_fields) {
        inSync = c.in_sync
        wordsRecent = c.words_recent
        wordsOk = c.words_ok
        wordsMissed = c.words_missed
        calls = c.calls
        stations = c.stations
        offsetHz = c.offset_hz
        snrDb = c.has_snr_db ? c.snr_db : nil
        ber = c.has_ber ? c.ber : nil
        levelDbfs = c.level_dbfs
        clock = Clock(rawValue: c.clock)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AleFields, SDRKitCode> {
        var c = sdrkit_ale_fields()
        let code = sdrkit_ale_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AleFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the ALE transmitter. The values of a transmission have no value while nothing is sent.
public struct AleTxFields {
    public enum TxKind: Int32 {
        case call = 0
        case sounding = 1
        case response = 2
        case ack = 3
        case amd = 4
    }
    public enum TxLink: Int32 {
        case idle = 0
        case calling = 1
        case responding = 2
        case linked = 3
        case timedOut = 4
        case notAnswered = 5
    }
    /// Whether a call or a sounding is being sent.
    public var txSending: Bool
    /// What is being sent.
    public var txKind: TxKind?
    /// The called address of a call.
    public var txTo: String?
    /// The word being sent, from 1.
    public var txWord: Int64?
    /// The words of the transmission.
    public var txWords: Int64?
    /// What the transmitter does not do.
    public var txNote: String
    /// The state of the three-way handshake.
    public var txLink: TxLink?
    /// The other station of the handshake. Absent while idle.
    public var txLinkStation: String?
    /// Why the link state is what it is; empty when it needs no reason.
    public var txLinkReason: String

    init(_ c: sdrkit_ale_tx_fields) {
        txSending = c.tx_sending
        txKind = c.has_tx_kind ? TxKind(rawValue: c.tx_kind) : nil
        txTo = c.has_tx_to ? sdrkitText(c.tx_to) : nil
        txWord = c.has_tx_word ? c.tx_word : nil
        txWords = c.has_tx_words ? c.tx_words : nil
        txNote = sdrkitText(c.tx_note)
        txLink = TxLink(rawValue: c.tx_link)
        txLinkStation = c.has_tx_link_station ? sdrkitText(c.tx_link_station) : nil
        txLinkReason = sdrkitText(c.tx_link_reason)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AleTxFields, SDRKitCode> {
        var c = sdrkit_ale_tx_fields()
        let code = sdrkit_ale_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AleTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A call in the ALE event log. The row key is the sequence number of the call.
public struct AleEventRow {
    public enum TypeOption: Int32 {
        case sounding = 0
        case individual = 1
        case net = 2
        case group = 3
        case allCall = 4
        case anyCall = 5
        case response = 6
        case ack = 7
        case netResponse = 8
    }
    /// The row key.
    public var key: String
    /// The signal time of the first word.
    public var start: Double
    /// The UTC of the first word, when the clock is known.
    public var time: Double?
    /// The dial it was heard on, when known.
    public var dial: Double?
    /// The caller, when known.
    public var from: String?
    /// The TO addresses then the THRU members.
    public var to: String
    /// The type of the call.
    public var type: TypeOption?
    /// TIS (else TWS).
    public var tis: Bool
    /// The AMD text.
    public var amd: String
    /// The commands.
    public var commands: String
    /// The bit error rate of the call.
    public var ber: Double
    /// The mean word SNR in 3 kHz.
    public var snrDb: Double
    /// The frequency correction at the end of the call.
    public var offsetHz: Double
    /// Words in the transmission.
    public var words: Int64

    init(_ c: sdrkit_ale_event_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        dial = c.has_dial ? c.dial : nil
        from = c.has_from ? sdrkitText(c.from) : nil
        to = sdrkitText(c.to)
        type = TypeOption(rawValue: c.type)
        tis = c.tis
        amd = sdrkitText(c.amd)
        commands = sdrkitText(c.commands)
        ber = c.ber
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        words = c.words
    }
}

/// A caller of ALE. The row key is the address.
public struct AleStationRow {
    public enum LastType: Int32 {
        case sounding = 0
        case individual = 1
        case net = 2
        case group = 3
        case allCall = 4
        case anyCall = 5
        case response = 6
        case ack = 7
        case netResponse = 8
    }
    /// The row key.
    public var key: String
    /// The address.
    public var address: String
    /// The dial of the last call, when known.
    public var dial: Double?
    /// The SNR of the last call.
    public var snrDb: Double
    /// The bit error rate of the last call.
    public var ber: Double
    /// The type of the last call.
    public var lastType: LastType?
    /// Times heard.
    public var heard: Int64?
    /// When last heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_ale_station_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        dial = c.has_dial ? c.dial : nil
        snrDb = c.snr_db
        ber = c.ber
        lastType = LastType(rawValue: c.last_type)
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A decoded ALE word. The row key is the sequence number of the word.
public struct AleWordRow {
    public enum TypeOption: Int32 {
        case data = 0
        case thru = 1
        case to = 2
        case tws = 3
        case from = 4
        case tis = 5
        case cmd = 6
        case rep = 7
    }
    /// The row key.
    public var key: String
    /// The signal time of its first symbol.
    public var start: Double
    /// The UTC, when the clock is known.
    public var time: Double?
    /// The word type.
    public var type: TypeOption?
    /// The three characters of the word (non-printing ones as '.').
    public var text: String
    /// Vote errors.
    public var votes: Int64
    /// Golay corrections.
    public var golay: Int64

    init(_ c: sdrkit_ale_word_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        type = TypeOption(rawValue: c.type)
        text = sdrkitText(c.text)
        votes = c.votes
        golay = c.golay
    }
}

/// A net address learned from the answers. The row key is the address.
public struct AleNetRow {
    /// The row key.
    public var key: String
    /// The address.
    public var address: String

    init(_ c: sdrkit_ale_net_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
    }
}

/// The link quality of a station on a channel. The row key is the address and the dial.
public struct AleLqaRow {
    /// The row key.
    public var key: String
    /// The address.
    public var address: String
    /// The dial, when known.
    public var dial: Double?
    /// The mean vote errors a word of its last call, 0 to 30.
    public var lqaBer: Int64
    /// The SINAD of its last call, 0 to 30 dB.
    public var sinadDb: Int64
    /// The multipath figure of its last call (not measured: 0).
    public var multipath: Int64
    /// The BER that the station reported of this station's signal, in its last call to the own address with an LQA report.
    public var reportedBer: Int64?
    /// The SINAD that the station reported of this station's signal, dB.
    public var reportedSinadDb: Int64?
    /// The multipath that the station reported.
    public var reportedMultipath: Int64?
    /// Calls heard from it on this channel.
    public var heard: Int64?
    /// When last heard on this channel.
    public var lastHeard: Double?

    init(_ c: sdrkit_ale_lqa_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        dial = c.has_dial ? c.dial : nil
        lqaBer = c.lqa_ber
        sinadDb = c.sinad_db
        multipath = c.multipath
        reportedBer = c.has_reported_ber ? c.reported_ber : nil
        reportedSinadDb = c.has_reported_sinad_db ? c.reported_sinad_db : nil
        reportedMultipath = c.has_reported_multipath ? c.reported_multipath : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// An AMD text heard. The row key is the sequence number of the text.
public struct AleMessageRow {
    /// The row key.
    public var key: String
    /// The signal time of the call's first word.
    public var start: Double
    /// The UTC of the call's first word, when the clock is known.
    public var time: Double?
    /// The dial it was heard on, when known.
    public var dial: Double?
    /// The caller, when known.
    public var from: String?
    /// The called addresses.
    public var to: String
    /// The text.
    public var text: String

    init(_ c: sdrkit_ale_message_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        dial = c.has_dial ? c.dial : nil
        from = c.has_from ? sdrkitText(c.from) : nil
        to = sdrkitText(c.to)
        text = sdrkitText(c.text)
    }
}

/// The result fields of the HF ocean radar stack.
public struct HfradarFields {
    public enum Clock: Int32 {
        case system = 0
        case recording = 1
        case stream = 2
    }
    /// The radar band the radio is in, if it is in one.
    public var band: String?
    /// Radars shown.
    public var radarCount: Int64
    /// Sweeps counted.
    public var sweepsCounted: Int64
    /// Tracks rejected.
    public var tracksRejected: Int64
    /// Broadband bursts.
    public var broadbandBursts: Int64
    /// The best SNR of the radars heard. Absent without a radar.
    public var snrDb: Double?
    /// The stops of the scan, while it steps.
    public var scanStops: Int64?
    /// The time of one waterfall row.
    public var waterfallRow: Double
    /// The signal time of the newest waterfall row.
    public var waterfallNewest: Double
    /// Where the times come from.
    public var clock: Clock?

    init(_ c: sdrkit_hfradar_fields) {
        band = c.has_band ? sdrkitText(c.band) : nil
        radarCount = c.radar_count
        sweepsCounted = c.sweeps_counted
        tracksRejected = c.tracks_rejected
        broadbandBursts = c.broadband_bursts
        snrDb = c.has_snr_db ? c.snr_db : nil
        scanStops = c.has_scan_stops ? c.scan_stops : nil
        waterfallRow = c.waterfall_row
        waterfallNewest = c.waterfall_newest
        clock = Clock(rawValue: c.clock)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfradarFields, SDRKitCode> {
        var c = sdrkit_hfradar_fields()
        let code = sdrkit_hfradar_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfradarFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A radar. The row key is the radar number.
public struct HfradarRadarRow {
    public enum Direction: Int32 {
        case rising = 0
        case falling = 1
    }
    /// The row key.
    public var key: String
    /// The radar number.
    public var number: Int64
    /// The centre of the sweep.
    public var centre: Double
    /// The centre against the tuned frequency.
    public var offsetHz: Double
    /// The radar band, if the radar is in one.
    public var band: String?
    /// The width of the sweep.
    public var width: Double
    /// The sweep rate.
    public var rateHzS: Double
    /// The direction of the sweep.
    public var direction: Direction?
    /// The repetition period of the sweeps, when measured.
    public var `repeat`: Double?
    /// The signal-to-noise ratio.
    public var snrDb: Double
    /// The level, dBFS.
    public var levelDbfs: Double
    /// Sweeps counted.
    public var sweeps: Int64
    /// The signal time when first heard.
    public var first: Double
    /// The signal time when last heard.
    public var last: Double
    /// The wall time when first heard, when the clock is known.
    public var firstHeard: Double?
    /// The wall time when last heard, in whole seconds, when the clock is known.
    public var lastHeard: Double?
    /// The keyed ID, when read.
    public var id: String?
    /// The speed of the keyed ID.
    public var idWpm: Double?
    /// Times the ID was seen.
    public var idSeen: Int64
    /// What the radar may be.
    public var hint: String

    init(_ c: sdrkit_hfradar_radar_row) {
        key = sdrkitText(c.key)
        number = c.number
        centre = c.centre
        offsetHz = c.offset_hz
        band = c.has_band ? sdrkitText(c.band) : nil
        width = c.width
        rateHzS = c.rate_hz_s
        direction = Direction(rawValue: c.direction)
        `repeat` = c.has_repeat ? c.repeat : nil
        snrDb = c.snr_db
        levelDbfs = c.level_dbfs
        sweeps = c.sweeps
        first = c.first
        last = c.last
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        id = c.has_id ? sdrkitText(c.id) : nil
        idWpm = c.has_id_wpm ? c.id_wpm : nil
        idSeen = c.id_seen
        hint = sdrkitText(c.hint)
    }
}

/// A radar heard. The row key is the sequence number.
public struct HfradarHeardRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var start: Double
    /// The wall time, when the clock is known.
    public var time: Double?
    /// The radar number.
    public var number: Int64
    /// The centre of the sweep.
    public var frequency: Double
    /// The radar band, if the radar is in one.
    public var band: String?
    /// The signal-to-noise ratio.
    public var snrDb: Double

    init(_ c: sdrkit_hfradar_heard_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        number = c.number
        frequency = c.frequency
        band = c.has_band ? sdrkitText(c.band) : nil
        snrDb = c.snr_db
    }
}

/// An oceanographic radar band. The row key is the band name.
public struct HfradarBandRow {
    /// The row key.
    public var key: String
    /// The band name ("12 - 14 MHz").
    public var name: String
    /// The lower edge of the working range.
    public var low: Double
    /// The upper edge of the working range.
    public var high: Double
    /// The middle of the band: where `band.tune` tunes the radio.
    public var middle: Double
    /// The range class of the radars of the band.
    public var rangeClass: String

    init(_ c: sdrkit_hfradar_band_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        low = c.low
        high = c.high
        middle = c.middle
        rangeClass = sdrkitText(c.range_class)
    }
}

/// The result fields of the PACTOR-II, -III and -IV stacks: the measures of the last analysis.
public struct HfdataPactorhfFields {
    public enum Clock: Int32 {
        case system = 0
        case recording = 1
        case stream = 2
    }
    /// A signal was found by the last analysis.
    public var signal: Bool
    /// The variant found: 2, 3 or 4 (0 for nothing).
    public var variant: Int64
    /// The signal on the channel is not the variant that the stack receives.
    public var otherVariant: Bool
    /// The symbol rate. Absent without a signal.
    public var symbolRateHz: Double?
    /// The clock error. Absent without a signal.
    public var clockPpm: Double?
    /// The middle of the outer carriers.
    public var centre: Double?
    /// The centre against its nominal place.
    public var offsetHz: Double?
    /// The carrier spacing (0 for one carrier).
    public var spacing: Double?
    /// The occupied bandwidth.
    public var bandwidth: Double?
    /// The clock line of the symbol clock.
    public var clockLineDb: Double?
    /// The number of carriers.
    public var carrierCount: Int64
    /// The PACTOR-III speed level (0 when unknown or another variant).
    public var speedLevel: Int64
    /// The PACTOR-IV constellation estimate: 2, 4, 8, 16 (QAM), 0 unknown.
    public var constellation: Int64
    /// The number of symbols demodulated on each carrier.
    public var symbolCount: Int64
    /// The start of the demodulated stretch, into the analysed audio.
    public var stretchStart: Double?
    /// The length of the demodulated stretch.
    public var stretchLength: Double?
    /// A repeating frame structure was found.
    public var frameFound: Bool
    /// The frame period in symbols.
    public var frameSymbols: Int64?
    /// The frame period.
    public var framePeriod: Double?
    /// The number of frame copies in the buffer.
    public var frameCopies: Int64?
    /// The mean agreement of neighbouring copies.
    public var frameAgreementPct: Double?
    /// The first symbol of the constant part.
    public var constantStart: Int64?
    /// The length of the constant part, in symbols.
    public var constantSymbols: Int64?
    /// The constant part as hexadecimal, 2 bits per symbol.
    public var constantHex: String
    /// The analysis note (PACTOR-IV).
    public var note: String
    /// The newest phase steps of the first carrier, one digit per symbol.
    public var symbols1: String
    /// The newest phase steps of the second carrier.
    public var symbols2: String
    /// The newest phase steps of the third carrier.
    public var symbols3: String
    /// The analyses run.
    public var analyses: Int64
    /// The analyses that found a signal.
    public var analysesWithSignal: Int64
    /// The audio level of the last second.
    public var levelDbfs: Double?
    /// Where the times come from.
    public var clock: Clock?

    init(_ c: sdrkit_hfdata_pactorhf_fields) {
        signal = c.signal
        variant = c.variant
        otherVariant = c.other_variant
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        centre = c.has_centre ? c.centre : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        spacing = c.has_spacing ? c.spacing : nil
        bandwidth = c.has_bandwidth ? c.bandwidth : nil
        clockLineDb = c.has_clock_line_db ? c.clock_line_db : nil
        carrierCount = c.carrier_count
        speedLevel = c.speed_level
        constellation = c.constellation
        symbolCount = c.symbol_count
        stretchStart = c.has_stretch_start ? c.stretch_start : nil
        stretchLength = c.has_stretch_length ? c.stretch_length : nil
        frameFound = c.frame_found
        frameSymbols = c.has_frame_symbols ? c.frame_symbols : nil
        framePeriod = c.has_frame_period ? c.frame_period : nil
        frameCopies = c.has_frame_copies ? c.frame_copies : nil
        frameAgreementPct = c.has_frame_agreement_pct ? c.frame_agreement_pct : nil
        constantStart = c.has_constant_start ? c.constant_start : nil
        constantSymbols = c.has_constant_symbols ? c.constant_symbols : nil
        constantHex = sdrkitText(c.constant_hex)
        note = sdrkitText(c.note)
        symbols1 = sdrkitText(c.symbols_1)
        symbols2 = sdrkitText(c.symbols_2)
        symbols3 = sdrkitText(c.symbols_3)
        analyses = c.analyses
        analysesWithSignal = c.analyses_with_signal
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        clock = Clock(rawValue: c.clock)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfdataPactorhfFields, SDRKitCode> {
        var c = sdrkit_hfdata_pactorhf_fields()
        let code = sdrkit_hfdata_pactorhf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfdataPactorhfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A carrier of the last analysis. The row key is the carrier's number, "00" first.
public struct HfdataPactorhfCarrierRow {
    public enum Modulation: Int32 {
        case dBpsk = 0
        case dQpsk = 1
        case d8psk = 2
        case d16psk = 3
    }
    /// The row key.
    public var key: String
    /// The carrier frequency.
    public var frequency: Double
    /// The clock line of the carrier.
    public var lineDb: Double
    /// Phase steps per symbol.
    public var steps: Int64
    /// The modulation, when the phase steps are 2, 4, 8 or 16.
    public var modulation: Modulation?
    /// The concentration of the phase steps, 0 to 100 %.
    public var concentrationPct: Double

    init(_ c: sdrkit_hfdata_pactorhf_carrier_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        lineDb = c.line_db
        steps = c.steps
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        concentrationPct = c.concentration_pct
    }
}

/// An event of the PACTOR-II, -III and -IV stacks. The row key is the sequence number.
public struct HfdataPactorhfEventRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var start: Double
    /// The UTC, when the clock is known.
    public var time: Double?
    /// What happened.
    public var text: String

    init(_ c: sdrkit_hfdata_pactorhf_event_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        text = sdrkitText(c.text)
    }
}

/// The result fields of PACTOR-I.
public struct Pactor1Fields {
    public enum Clock: Int32 {
        case system = 0
        case recording = 1
        case stream = 2
    }
    /// A tone pair is locked or a frame was read in the last 30 s (the lock state "Signal").
    public var signal: Bool
    /// A frame was read in the last 30 s (the lock state "Packets").
    public var packetsRecent: Bool
    /// Good packets.
    public var packetsOk: Int64
    /// Good packets at 200 baud.
    public var packetsFast: Int64
    /// Good packets at 100 baud.
    public var packetsSlow: Int64
    /// Packets whose CRC failed.
    public var packetsBad: Int64
    /// Control signals.
    public var controlSignals: Int64
    /// Repeated packets.
    public var repeats: Int64
    /// Packets read by memory ARQ.
    public var memoryArqSaves: Int64
    /// Changeovers (CS3).
    public var changeovers: Int64
    /// Connect packets.
    public var connects: Int64
    /// The link is in unproto (FEC) mode, else ARQ.
    public var fecMode: Bool
    /// The Huffman table holds all 95 printable characters.
    public var huffmanComplete: Bool
    /// Entries in the Huffman table.
    public var huffmanEntries: Int64
    /// The locked lower tone. Absent without a lock.
    public var pairLow: Double?
    /// The locked upper tone. Absent without a lock.
    public var pairHigh: Double?
    /// The centre of the pair against 1500 Hz. Absent without a lock.
    public var offsetHz: Double?
    /// The SNR of the tone pair. Absent without a lock.
    public var snrDb: Double?
    /// The audio level of the last second.
    public var levelDbfs: Double?
    /// The text of both sides.
    public var text: String
    /// Where the times come from.
    public var clock: Clock?

    init(_ c: sdrkit_pactor1_fields) {
        signal = c.signal
        packetsRecent = c.packets_recent
        packetsOk = c.packets_ok
        packetsFast = c.packets_fast
        packetsSlow = c.packets_slow
        packetsBad = c.packets_bad
        controlSignals = c.control_signals
        repeats = c.repeats
        memoryArqSaves = c.memory_arq_saves
        changeovers = c.changeovers
        connects = c.connects
        fecMode = c.fec_mode
        huffmanComplete = c.huffman_complete
        huffmanEntries = c.huffman_entries
        pairLow = c.has_pair_low ? c.pair_low : nil
        pairHigh = c.has_pair_high ? c.pair_high : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        text = sdrkitText(c.text)
        clock = Clock(rawValue: c.clock)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Pactor1Fields, SDRKitCode> {
        var c = sdrkit_pactor1_fields()
        let code = sdrkit_pactor1_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Pactor1Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of PACTOR-I. The row key is the frame's number in the log.
public struct Pactor1FrameRow {
    public enum Kind: Int32 {
        case data = 0
        case connect = 1
        case qrt = 2
        case cs1 = 3
        case cs2 = 4
        case cs3 = 5
        case cs4 = 6
        case badPacket = 7
    }
    /// The row key.
    public var key: String
    /// The signal time of the frame's first bit.
    public var start: Double
    /// The UTC of the frame, when the clock is known.
    public var time: Double?
    /// The kind of frame.
    public var kind: Kind?
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The header byte of a packet, 0x55 or 0xAA.
    public var header: Int64?
    /// The packet number 0 to 3.
    public var number: Int64?
    /// Huffman text.
    public var huffman: Bool
    /// The break-in flag.
    public var breakIn: Bool
    /// The QRT flag.
    public var qrt: Bool
    /// The CRC passed.
    public var crcOk: Bool
    /// Read by memory ARQ.
    public var memoryArq: Bool
    /// A repeat of the packet before.
    public var `repeat`: Bool
    /// A repeat with no control signal since the packet before.
    public var fecRepeat: Bool
    /// The tones were swapped.
    public var inverted: Bool
    /// The sending side, 1 or 2.
    public var side: Int64
    /// The text of the frame.
    public var text: String
    /// The data bytes in hexadecimal.
    public var dataHex: String
    /// The called station of a connect packet: the row key of its row of the stations table.
    public var call: String?
    /// The SNR of the frame.
    public var snrDb: Double
    /// The lower tone of the frame.
    public var pairLow: Double

    init(_ c: sdrkit_pactor1_frame_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        kind = Kind(rawValue: c.kind)
        symbolRateHz = c.symbol_rate_hz
        header = c.has_header ? c.header : nil
        number = c.has_number ? c.number : nil
        huffman = c.huffman
        breakIn = c.break_in
        qrt = c.qrt
        crcOk = c.crc_ok
        memoryArq = c.memory_arq
        `repeat` = c.repeat
        fecRepeat = c.fec_repeat
        inverted = c.inverted
        side = c.side
        text = sdrkitText(c.text)
        dataHex = sdrkitText(c.data_hex)
        call = c.has_call ? sdrkitText(c.call) : nil
        snrDb = c.snr_db
        pairLow = c.pair_low
    }
}

/// A station of PACTOR-I. The row key is the callsign.
public struct Pactor1StationRow {
    public enum SeenAs: Int32 {
        case connect = 0
        case text = 1
    }
    /// The row key.
    public var key: String
    /// The callsign.
    public var call: String
    /// How the station was seen.
    public var seenAs: SeenAs?
    /// Times heard.
    public var heard: Int64?
    /// When last heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_pactor1_station_row) {
        key = sdrkitText(c.key)
        call = sdrkitText(c.call)
        seenAs = SeenAs(rawValue: c.seen_as)
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the HF beacon monitor.
public struct HfbeaconsFields {
    public enum Clock: Int32 {
        case system = 0
        case recording = 1
        case stream = 2
    }
    /// The low edge of the span shown (centre - 0.4 x rate).
    public var spanLow: Double
    /// The high edge of the span shown (centre + 0.4 x rate).
    public var spanHigh: Double
    /// Beacons of the list in the span.
    public var inSpan: Int64
    /// Beacons heard now.
    public var heardNow: Int64
    /// Channels watched.
    public var channels: Int64
    /// Frames processed.
    public var frames: Int64
    /// Keying runs started.
    public var runsStarted: Int64
    /// Where the times come from.
    public var clock: Clock?
    /// The clock is good enough for the time slots of the beacons.
    public var clockValid: Bool
    /// Beacons in the list.
    public var listCount: Int64
    /// The station position is known.
    public var stationKnown: Bool
    /// The band to try now, when the model names one.
    public var bestBand: String?
    /// The stations heard on that band.
    public var bestBandStations: Int64?
    /// The level of the input, dBFS.
    public var levelDbfs: Double

    init(_ c: sdrkit_hfbeacons_fields) {
        spanLow = c.span_low
        spanHigh = c.span_high
        inSpan = c.in_span
        heardNow = c.heard_now
        channels = c.channels
        frames = c.frames
        runsStarted = c.runs_started
        clock = Clock(rawValue: c.clock)
        clockValid = c.clock_valid
        listCount = c.list_count
        stationKnown = c.station_known
        bestBand = c.has_best_band ? sdrkitText(c.best_band) : nil
        bestBandStations = c.has_best_band_stations ? c.best_band_stations : nil
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfbeaconsFields, SDRKitCode> {
        var c = sdrkit_hfbeacons_fields()
        let code = sdrkit_hfbeacons_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfbeaconsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the own beacon. The values of a sending have no value while nothing is sent.
public struct HfbeaconsHfbTxFields {
    /// Whether the beacon runs: it sends now or waits for its next sending.
    public var txBeaconOn: Bool
    /// Whether a sending is on air.
    public var txSending: Bool
    /// The share of the sending on air that is sent.
    public var txProgressPct: Double?
    /// The time to the next sending, while the beacon waits.
    public var txNextS: Double?
    /// The sendings since `start`.
    public var txSendings: Int64
    /// The time on air of one sending with the present settings.
    public var txSendingS: Double
    /// The share of the interval that one sending is on air.
    public var txDutyPct: Double
    /// What the beacon is: a proposal of the specification, not taken from a standard.
    public var txNote: String

    init(_ c: sdrkit_hfbeacons_hfb_tx_fields) {
        txBeaconOn = c.tx_beacon_on
        txSending = c.tx_sending
        txProgressPct = c.has_tx_progress_pct ? c.tx_progress_pct : nil
        txNextS = c.has_tx_next_s ? c.tx_next_s : nil
        txSendings = c.tx_sendings
        txSendingS = c.tx_sending_s
        txDutyPct = c.tx_duty_pct
        txNote = sdrkitText(c.tx_note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfbeaconsHfbTxFields, SDRKitCode> {
        var c = sdrkit_hfbeacons_hfb_tx_fields()
        let code = sdrkit_hfbeacons_hfb_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfbeaconsHfbTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A beacon of the list. The row key is the place in the list, "0000" first.
public struct HfbeaconsBeaconRow {
    public enum Kind: Int32 {
        case ncdxf = 0
        case letter = 1
        case dashes = 2
        case carrier = 3
    }
    /// The row key.
    public var key: String
    /// The beacon's name.
    public var name: String
    /// The frequency of the beacon.
    public var frequency: Double
    /// The keying pattern.
    public var kind: Kind?
    /// The entry of the list is uncertain.
    public var uncertain: Bool
    /// The beacon is inside the capture.
    public var inCapture: Bool
    /// The beacon is heard now.
    public var heard: Bool
    /// The confidence of the identity.
    public var confidence: Int64
    /// The signal time when last heard.
    public var last: Double?
    /// The UTC when last heard, in whole seconds.
    public var lastHeard: Double?
    /// The SNR when last heard.
    public var snrDb: Double?
    /// The best SNR of the last hour.
    public var best1hDb: Double?
    /// The best SNR of the last 24 hours.
    public var best24hDb: Double?
    /// Power steps heard.
    public var steps: Int64
    /// The SNR of each power step.
    public var stepSnrs: String
    /// The distance to the station.
    public var distanceKm: Double?
    /// The bearing from the station.
    public var bearingDeg: Double?
    /// The newest 48 history points in dB, "-" for listened and not heard.
    public var trend: String
    /// History points in 24 hours.
    public var listens: Int64
    /// History points heard.
    public var heardCount: Int64
    /// A note on the beacon.
    public var note: String
    /// The band of the beacon.
    public var band: String
    /// The position of the beacon.
    public var position: SDRKitPosition?

    init(_ c: sdrkit_hfbeacons_beacon_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        frequency = c.frequency
        kind = Kind(rawValue: c.kind)
        uncertain = c.uncertain
        inCapture = c.in_capture
        heard = c.heard
        confidence = c.confidence
        last = c.has_last ? c.last : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        best1hDb = c.has_best_1h_db ? c.best_1h_db : nil
        best24hDb = c.has_best_24h_db ? c.best_24h_db : nil
        steps = c.steps
        stepSnrs = sdrkitText(c.step_snrs)
        distanceKm = c.has_distance_km ? c.distance_km : nil
        bearingDeg = c.has_bearing_deg ? c.bearing_deg : nil
        trend = sdrkitText(c.trend)
        listens = c.listens
        heardCount = c.heard_count
        note = sdrkitText(c.note)
        band = sdrkitText(c.band)
        position = c.has_position ? SDRKitPosition(c.position) : nil
    }
}

/// A band of the beacon list. The row key is the place, "00" first.
public struct HfbeaconsBandRow {
    /// The row key.
    public var key: String
    /// The band.
    public var band: String
    /// Beacons of the band in the list.
    public var inList: Int64
    /// Beacons of the band in the capture.
    public var inCapture: Int64
    /// Beacons of the band heard now.
    public var heardNow: Int64
    /// The farthest beacon heard.
    public var farthestKm: Double?

    init(_ c: sdrkit_hfbeacons_band_row) {
        key = sdrkitText(c.key)
        band = sdrkitText(c.band)
        inList = c.in_list
        inCapture = c.in_capture
        heardNow = c.heard_now
        farthestKm = c.has_farthest_km ? c.farthest_km : nil
    }
}

/// The propagation of a band. The row key is the place, "00" first.
public struct HfbeaconsPropagationRow {
    public enum State: Int32 {
        case `open` = 0
        case marginal = 1
        case closed = 2
    }
    /// The row key.
    public var key: String
    /// The band.
    public var band: String
    /// The state of the band.
    public var state: State?
    /// Stations heard.
    public var stations: Int64
    /// The best SNR.
    public var bestSnrDb: Double?
    /// The farthest station heard.
    public var farthestKm: Double?
    /// The score of the band.
    public var score: Double

    init(_ c: sdrkit_hfbeacons_propagation_row) {
        key = sdrkitText(c.key)
        band = sdrkitText(c.band)
        state = State(rawValue: c.state)
        stations = c.stations
        bestSnrDb = c.has_best_snr_db ? c.best_snr_db : nil
        farthestKm = c.has_farthest_km ? c.farthest_km : nil
        score = c.score
    }
}

/// The result fields of the wave buoy stack.
public struct WavebuoyFields {
    public enum Format: Int32 {
        case carrier = 0
        case n2fsk = 1
        case n4mfsk = 2
    }
    /// The link format.
    public var format: Format?
    /// The format is forced by the `format` setting.
    public var formatForced: Bool
    /// The carrier finder sees a signal.
    public var signalFound: Bool
    /// The data is valid: 2FSK with the HXV vectors locked, or 4FSK with the symbols locked.
    public var dataValid: Bool
    /// The centre of the signal about the tuned frequency.
    public var offsetHz: Double?
    /// The absolute frequency of the signal centre.
    public var frequency: Double?
    /// The listed buoy channel within 6 kHz of the signal.
    public var channel: String?
    /// The peak of the spectrum over the floor.
    public var peakDb: Double?
    /// The width holding 90 % of the signal power.
    public var width: Double?
    /// The SNR in 100 Hz.
    public var snrDb: Double?
    /// The symbol rate.
    public var symbolRateHz: Double?
    /// The clock error.
    public var clockPpm: Double?
    /// The measured distance between the tones.
    public var toneStep: Double?
    /// The RMS distance of the symbol means to the nearest tone, in tone steps.
    public var symbolErrorSteps: Double?
    /// Symbols demodulated since the last restart.
    public var symbolCount: Int64
    /// The symbols are locked.
    public var locked: Bool
    /// The newest 320 bits (higher tone = 1; 4FSK: the tone number as two bits).
    public var bits: String
    /// The HXV vectors are locked.
    public var hxvLocked: Bool
    /// The tones are the other way round.
    public var hxvInverted: Bool
    /// Vectors accepted.
    public var vectorCount: Int64
    /// Vectors accepted with no bit repaired.
    public var cleanCount: Int64
    /// Vectors accepted with 1 or 2 bits repaired.
    public var repairedCount: Int64
    /// Vectors lost.
    public var lostCount: Int64
    /// Blocks with a system word read.
    public var blockCount: Int64
    /// The system word number of the last block.
    public var lastBlock: Int64?
    /// The last vector is a WR-SG vector (two vertical samples, west AAA).
    public var wrsg: Bool
    /// The heave of the last vector.
    public var heaveCm: Int64?
    /// The north displacement of the last vector.
    public var northCm: Int64?
    /// The west displacement of the last vector.
    public var westCm: Int64?
    /// System file words received, of 16.
    public var systemWordCount: Int64
    /// Spectrum points received, of 64.
    public var spectrumPointCount: Int64
    /// The buoy type number Tn (system word 0).
    public var tn: Int64?
    /// The GPS module works (system word 0).
    public var gpsOk: Bool?
    /// The GPS has a fix (system word 0).
    public var gpsFix: Bool?
    /// The RMS wave height (system word 1).
    public var hrmsM: Double?
    /// The significant wave height, 4 Hrms (system word 1).
    public var hsM: Double?
    /// The zero-crossing frequency (system word 2).
    public var fz: Double?
    /// The zero-crossing period, 1 / fz (system word 2).
    public var tzS: Double?
    /// The largest spectral density (system word 3).
    public var psdMaxM2Hz: Double?
    /// The buoy temperature Tr (system word 4).
    public var trC: Double?
    /// The water temperature Tw (system word 5).
    public var twC: Double?
    /// The operational life left (system word 6).
    public var lifeLeftWeeks: Int64?
    /// The battery status, 0 to 7 (system word 6).
    public var batteryStatus: Int64?
    /// The vertical accelerometer offset (system word 7).
    public var av0MS2: Double?
    /// The x accelerometer offset (system word 8).
    public var ax0MS2: Double?
    /// The y accelerometer offset (system word 9).
    public var ay0MS2: Double?
    /// The buoy position (system words 10 to 13).
    public var position: SDRKitPosition?
    /// The orientation (system word 14).
    public var orientationDeg: Double?
    /// The inclination (system word 15).
    public var inclinationDeg: Double?
    /// The repetition period of the bit stream.
    public var periodBits: Int64?
    /// How far the period stands out of random bits (z score).
    public var periodZ: Double?
    /// The constant part of a period, in hex.
    public var sync: String?
    /// The first bit of the constant part in the period.
    public var syncStartBit: Int64?
    /// The CRC or checksum that fits the frames (an attempt, never a verified Datawell CRC).
    public var crcAttempt: String?
    /// Frames the attempt passed in the last analysis.
    public var crcPasses: Int64
    /// Frames the CRC attempt accepts.
    public var framesOk: Int64
    /// Frames the CRC attempt refuses.
    public var framesBad: Int64

    init(_ c: sdrkit_wavebuoy_fields) {
        format = Format(rawValue: c.format)
        formatForced = c.format_forced
        signalFound = c.signal_found
        dataValid = c.data_valid
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        frequency = c.has_frequency ? c.frequency : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        peakDb = c.has_peak_db ? c.peak_db : nil
        width = c.has_width ? c.width : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        toneStep = c.has_tone_step ? c.tone_step : nil
        symbolErrorSteps = c.has_symbol_error_steps ? c.symbol_error_steps : nil
        symbolCount = c.symbol_count
        locked = c.locked
        bits = sdrkitText(c.bits)
        hxvLocked = c.hxv_locked
        hxvInverted = c.hxv_inverted
        vectorCount = c.vector_count
        cleanCount = c.clean_count
        repairedCount = c.repaired_count
        lostCount = c.lost_count
        blockCount = c.block_count
        lastBlock = c.has_last_block ? c.last_block : nil
        wrsg = c.wrsg
        heaveCm = c.has_heave_cm ? c.heave_cm : nil
        northCm = c.has_north_cm ? c.north_cm : nil
        westCm = c.has_west_cm ? c.west_cm : nil
        systemWordCount = c.system_word_count
        spectrumPointCount = c.spectrum_point_count
        tn = c.has_tn ? c.tn : nil
        gpsOk = c.has_gps_ok ? c.gps_ok : nil
        gpsFix = c.has_gps_fix ? c.gps_fix : nil
        hrmsM = c.has_hrms_m ? c.hrms_m : nil
        hsM = c.has_hs_m ? c.hs_m : nil
        fz = c.has_fz ? c.fz : nil
        tzS = c.has_tz_s ? c.tz_s : nil
        psdMaxM2Hz = c.has_psd_max_m2_hz ? c.psd_max_m2_hz : nil
        trC = c.has_tr_c ? c.tr_c : nil
        twC = c.has_tw_c ? c.tw_c : nil
        lifeLeftWeeks = c.has_life_left_weeks ? c.life_left_weeks : nil
        batteryStatus = c.has_battery_status ? c.battery_status : nil
        av0MS2 = c.has_av0_m_s2 ? c.av0_m_s2 : nil
        ax0MS2 = c.has_ax0_m_s2 ? c.ax0_m_s2 : nil
        ay0MS2 = c.has_ay0_m_s2 ? c.ay0_m_s2 : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        orientationDeg = c.has_orientation_deg ? c.orientation_deg : nil
        inclinationDeg = c.has_inclination_deg ? c.inclination_deg : nil
        periodBits = c.has_period_bits ? c.period_bits : nil
        periodZ = c.has_period_z ? c.period_z : nil
        sync = c.has_sync ? sdrkitText(c.sync) : nil
        syncStartBit = c.has_sync_start_bit ? c.sync_start_bit : nil
        crcAttempt = c.has_crc_attempt ? sdrkitText(c.crc_attempt) : nil
        crcPasses = c.crc_passes
        framesOk = c.frames_ok
        framesBad = c.frames_bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WavebuoyFields, SDRKitCode> {
        var c = sdrkit_wavebuoy_fields()
        let code = sdrkit_wavebuoy_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WavebuoyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An entry of the directional spectrum received. The row key is the entry number, "00" first.
public struct WavebuoySpectrumRow {
    /// The row key.
    public var key: String
    /// The spectrum entry number, 0 to 63.
    public var entry: Int64
    /// The frequency of the entry.
    public var frequency: Double
    /// The spectral density, once PSDmax is known.
    public var psdM2Hz: Double?
    /// The mean wave direction.
    public var directionDeg: Double
    /// The directional spread.
    public var spreadDeg: Double
    /// The second-order Fourier coefficient m2 (no unit).
    public var m2: Double
    /// The second-order Fourier coefficient n2 (no unit).
    public var n2: Double
    /// The check factor K (no unit).
    public var k: Double
    /// The density relative to PSDmax (no unit).
    public var relativePsd: Double

    init(_ c: sdrkit_wavebuoy_spectrum_row) {
        key = sdrkitText(c.key)
        entry = c.entry
        frequency = c.frequency
        psdM2Hz = c.has_psd_m2_hz ? c.psd_m2_hz : nil
        directionDeg = c.direction_deg
        spreadDeg = c.spread_deg
        m2 = c.m2
        n2 = c.n2
        k = c.k
        relativePsd = c.relative_psd
    }
}

/// A system file word received. The row key is the word number, "00" first.
public struct WavebuoySystemWordRow {
    /// The row key.
    public var key: String
    /// The system file word number, 0 to 15.
    public var word: Int64
    /// The 12-bit value of the word.
    public var value: Int64

    init(_ c: sdrkit_wavebuoy_system_word_row) {
        key = sdrkitText(c.key)
        word = c.word
        value = c.value
    }
}

/// A frame cut at the sync. The row key is the frame's number in the log.
public struct WavebuoyFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var start: Double
    /// The bits of the frame in hex.
    public var hex: String
    /// The CRC attempt accepts the frame.
    public var accepted: Bool

    init(_ c: sdrkit_wavebuoy_frame_row) {
        key = sdrkitText(c.key)
        start = c.start
        hex = sdrkitText(c.hex)
        accepted = c.accepted
    }
}

/// The result fields of a serial-tone modem.
public struct HfdataSerialFields {
    /// The parts of the stack that no public document confirms.
    public var note: String
    /// The text received, the last characters.
    public var text: String
    /// The rate and interleaver being received.
    public var waveform: String
    /// Messages ended by their end-of-message pattern.
    public var messagesTotal: Int64
    /// Interleaver blocks decoded.
    public var blocksTotal: Int64
    /// D_PDUs with a good header CRC.
    public var dpdusGood: Int64
    /// The signal-to-noise ratio. Absent without a measure.
    public var snrDb: Double?
    /// The frequency error. Absent without a measure.
    public var offsetHz: Double?

    init(_ c: sdrkit_hfdata_serial_fields) {
        note = sdrkitText(c.note)
        text = sdrkitText(c.text)
        waveform = sdrkitText(c.waveform)
        messagesTotal = c.messages_total
        blocksTotal = c.blocks_total
        dpdusGood = c.dpdus_good
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfdataSerialFields, SDRKitCode> {
        var c = sdrkit_hfdata_serial_fields()
        let code = sdrkit_hfdata_serial_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfdataSerialFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A STANAG 5066 D_PDU found in the bits of a modem. The row key is the number of the D_PDU.
public struct HfdataDpduRow {
    /// The row key.
    public var key: String
    /// The UTC of the D_PDU.
    public var utc: Double
    /// The D_PDU type.
    public var type: String
    /// The EOT byte.
    public var eot: Int64
    /// The destination address (hex).
    public var destination: String
    /// The source address (hex).
    public var source: String
    /// Header bytes.
    public var headerBytes: Int64
    /// Payload bytes.
    public var payloadBytes: Int64
    /// The data CRC: good, bad or none.
    public var dataCrc: String
    /// The first 32 payload bytes (hex).
    public var payload: String

    init(_ c: sdrkit_hfdata_dpdu_row) {
        key = sdrkitText(c.key)
        utc = c.utc
        type = sdrkitText(c.type)
        eot = c.eot
        destination = sdrkitText(c.destination)
        source = sdrkitText(c.source)
        headerBytes = c.header_bytes
        payloadBytes = c.payload_bytes
        dataCrc = sdrkitText(c.data_crc)
        payload = sdrkitText(c.payload)
    }
}

/// The result fields of the Robust Packet stack: the comb of the last analysis and the last burst.
public struct RprFields {
    public enum State: Int32 {
        case nothing = 0
        case comb = 1
        case bursts = 2
    }
    public enum Modulation: Int32 {
        case dBpsk = 0
        case dQpsk = 1
    }
    public enum Clock: Int32 {
        case system = 0
        case recording = 1
        case stream = 2
    }
    /// What the last analysis found: nothing, the comb, or bursts.
    public var state: State?
    /// The centre of the comb in the audio. Absent without a comb.
    public var combCentre: Double?
    /// The comb centre against the audio centre setting.
    public var offsetHz: Double?
    /// The mean power inside the comb over the mean power beside it.
    public var combDb: Double?
    /// The carriers seen: those of the last burst with a clear step pattern, else the comb cells that stand out.
    public var carrierCount: Int64
    /// The carrier spacing measured on the last burst.
    public var spacing: Double?
    /// The symbol rate of the last burst, at the peak of its clock line.
    public var symbolRateHz: Double?
    /// The clock error of the last burst of more than 150 symbols.
    public var clockPpm: Double?
    /// The speed level of the last burst: 1 (D-BPSK) or 2 (D-QPSK); 0 before a burst.
    public var speedLevel: Int64
    /// The modulation of the last burst.
    public var modulation: Modulation?
    /// The net rate of the speed level before the AX.25 overhead: 200 or 600 bit/s.
    public var netRate: Int64?
    /// The SNR of the last burst, from the phase error of its steps.
    public var snrDb: Double?
    /// The best SNR of a burst since the last Clear.
    public var bestSnrDb: Double?
    /// The length of the last burst.
    public var burstLength: Double?
    /// The newest step indices of carrier 0, one digit per symbol.
    public var symbols1: String
    /// The newest step indices of carrier 1.
    public var symbols2: String
    /// The newest step indices of carrier 2.
    public var symbols3: String
    /// What the stack does not do.
    public var note: String
    /// The analyses run.
    public var analyses: Int64
    /// The analyses that found the comb.
    public var detections: Int64
    /// The bursts demodulated since the last Clear.
    public var bursts: Int64
    /// The audio level of the last second.
    public var levelDbfs: Double?
    /// Where the times come from.
    public var clock: Clock?

    init(_ c: sdrkit_rpr_fields) {
        state = State(rawValue: c.state)
        combCentre = c.has_comb_centre ? c.comb_centre : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        combDb = c.has_comb_db ? c.comb_db : nil
        carrierCount = c.carrier_count
        spacing = c.has_spacing ? c.spacing : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        speedLevel = c.speed_level
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        netRate = c.has_net_rate ? c.net_rate : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        bestSnrDb = c.has_best_snr_db ? c.best_snr_db : nil
        burstLength = c.has_burst_length ? c.burst_length : nil
        symbols1 = sdrkitText(c.symbols_1)
        symbols2 = sdrkitText(c.symbols_2)
        symbols3 = sdrkitText(c.symbols_3)
        note = sdrkitText(c.note)
        analyses = c.analyses
        detections = c.detections
        bursts = c.bursts
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        clock = Clock(rawValue: c.clock)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RprFields, SDRKitCode> {
        var c = sdrkit_rpr_fields()
        let code = sdrkit_rpr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RprFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A carrier of the last burst. The row key is the carrier number, "0" first.
public struct RprCarrierRow {
    /// The row key.
    public var key: String
    /// The carrier frequency in the audio: its place in the comb and its measured turn per symbol.
    public var frequency: Double
    /// The concentration of its phase steps for the order of the burst, 0 to 100 %.
    public var concentrationPct: Double

    init(_ c: sdrkit_rpr_carrier_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        concentrationPct = c.concentration_pct
    }
}

/// A burst demodulated. The row key is the sequence number.
public struct RprBurstRow {
    public enum Modulation: Int32 {
        case dBpsk = 0
        case dQpsk = 1
    }
    /// The row key.
    public var key: String
    /// The signal time of its start.
    public var start: Double
    /// The UTC of its start, when the clock is known.
    public var time: Double?
    /// Its length.
    public var length: Double
    /// The speed level: 1 or 2.
    public var speedLevel: Int64
    /// The modulation.
    public var modulation: Modulation?
    /// The net rate of the speed level: 200 or 600 bit/s.
    public var netRate: Int64
    /// The carriers with a clear step pattern.
    public var carriers: Int64
    /// The symbols demodulated on each carrier.
    public var symbols: Int64
    /// The SNR from the phase error of its steps.
    public var snrDb: Double
    /// The comb centre against the audio centre setting.
    public var offset: Double
    /// The step indices of the first 24 symbol times as hexadecimal: carrier 0 to 7 in turn, 1 bit (D-BPSK) or 2 bits (D-QPSK) each, most significant first. The bits are still coded and scrambled.
    public var firstSteps: String

    init(_ c: sdrkit_rpr_burst_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        length = c.length
        speedLevel = c.speed_level
        modulation = Modulation(rawValue: c.modulation)
        netRate = c.net_rate
        carriers = c.carriers
        symbols = c.symbols
        snrDb = c.snr_db
        offset = c.offset
        firstSteps = sdrkitText(c.first_steps)
    }
}

/// An event of the Robust Packet stack. The row key is the sequence number.
public struct RprEventRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var start: Double
    /// The UTC, when the clock is known.
    public var time: Double?
    /// What happened.
    public var text: String

    init(_ c: sdrkit_rpr_event_row) {
        key = sdrkitText(c.key)
        start = c.start
        time = c.has_time ? c.time : nil
        text = sdrkitText(c.text)
    }
}

/// New text: a line, 64 characters, or what came before a pause.
public struct HfdataTextEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text.
    public var text: String

    init(_ c: sdrkit_hfdata_text_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdataTextEvent, SDRKitCode> {
        var c = sdrkit_hfdata_text_event()
        let code = sdrkit_hfdata_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdataTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tones of a sender were found.
public struct HfdataTonesFoundEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The offset of the tones from their nominal place.
    public var offset: Double
    /// The SNR in 3 kHz.
    public var snr: Double

    init(_ c: sdrkit_hfdata_tones_found_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        offset = c.offset
        snr = c.snr
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdataTonesFoundEvent, SDRKitCode> {
        var c = sdrkit_hfdata_tones_found_event()
        let code = sdrkit_hfdata_tones_found_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdataTonesFoundEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tones of a sender are gone.
public struct HfdataTonesLostEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The offset of the tones when they were lost.
    public var offset: Double
    /// Characters or groups read in the lock.
    public var read: Int64

    init(_ c: sdrkit_hfdata_tones_lost_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        offset = c.offset
        read = c.read
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdataTonesLostEvent, SDRKitCode> {
        var c = sdrkit_hfdata_tones_lost_event()
        let code = sdrkit_hfdata_tones_lost_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdataTonesLostEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A block of f06 was read.
public struct Fsk200F06BlockEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the row.
    public var text: String
    /// The block number. Absent for a divider.
    public var number: Int64?
    /// Divider, Header, Block or File.
    public var kind: String
    /// The CRC result.
    public var crc: String

    init(_ c: sdrkit_fsk200_f06_block_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        number = c.has_number ? c.number : nil
        kind = sdrkitText(c.kind)
        crc = sdrkitText(c.crc)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Fsk200F06BlockEvent, SDRKitCode> {
        var c = sdrkit_fsk200_f06_block_event()
        let code = sdrkit_fsk200_f06_block_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Fsk200F06BlockEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst of GW FSK was read.
public struct GwfskGwPacketEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the row.
    public var text: String
    /// Channel marker, Packet, Position or ACK.
    public var kind: String
    /// The type of a packet.
    public var packetType: Int64?
    /// The subtype of a packet.
    public var subtype: Int64?

    init(_ c: sdrkit_gwfsk_gw_packet_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        kind = sdrkitText(c.kind)
        packetType = c.has_packet_type ? c.packet_type : nil
        subtype = c.has_subtype ? c.subtype : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GwfskGwPacketEvent, SDRKitCode> {
        var c = sdrkit_gwfsk_gw_packet_event()
        let code = sdrkit_gwfsk_gw_packet_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GwfskGwPacketEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst of a telephone FSK modem was read.
public struct FskModemsModemMessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the row.
    public var text: String
    /// The standard.
    public var standard: String
    /// The sideband: upper or lower.
    public var sideband: String
    /// The mark and space tones.
    public var tones: String
    /// The bytes, as hex.
    public var bytes: String
    /// Framing errors.
    public var framingErrors: Int64
    /// The length of the burst.
    public var duration: Double

    init(_ c: sdrkit_fsk_modems_modem_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        standard = sdrkitText(c.standard)
        sideband = sdrkitText(c.sideband)
        tones = sdrkitText(c.tones)
        bytes = sdrkitText(c.bytes)
        framingErrors = c.framing_errors
        duration = c.duration
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FskModemsModemMessageEvent, SDRKitCode> {
        var c = sdrkit_fsk_modems_modem_message_event()
        let code = sdrkit_fsk_modems_modem_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FskModemsModemMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A buzz train, a break or a long transmission.
public struct Uvb76Event {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the row.
    public var text: String
    /// Buzz train, Train break or Long transmission.
    public var kind: String
    /// The value: the rate, the time since the last buzz or the length.
    public var value: String

    init(_ c: sdrkit_uvb76_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        kind = sdrkitText(c.kind)
        value = sdrkitText(c.value)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Uvb76Event, SDRKitCode> {
        var c = sdrkit_uvb76_event()
        let code = sdrkit_uvb76_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Uvb76Event(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A transmitter came on, went off or changed.
public struct VlfMskVlfTransmitterEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the row.
    public var text: String
    /// on, off or changed.
    public var kind: String
    /// The bit rate.
    public var rate: Double?
    /// The carrier from the tuned frequency.
    public var carrier: Double?
    /// The level over the noise.
    public var level: Double?
    /// The listed station near the carrier.
    public var hint: String?

    init(_ c: sdrkit_vlf_msk_vlf_transmitter_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        kind = sdrkitText(c.kind)
        rate = c.has_rate ? c.rate : nil
        carrier = c.has_carrier ? c.carrier : nil
        level = c.has_level ? c.level : nil
        hint = c.has_hint ? sdrkitText(c.hint) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<VlfMskVlfTransmitterEvent, SDRKitCode> {
        var c = sdrkit_vlf_msk_vlf_transmitter_event()
        let code = sdrkit_vlf_msk_vlf_transmitter_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(VlfMskVlfTransmitterEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A lock of the words, characters or frames.
public struct HfdataFskLockEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The speed of the signal.
    public var baud: Double
    /// The shift between the tones.
    public var shiftHz: Double
    /// The centre of the signal above the dial.
    public var centreHz: Double
    /// What was found beside the speed.
    public var detail: String

    init(_ c: sdrkit_hfdata_fsk_lock_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        baud = c.baud
        shiftHz = c.shift_hz
        centreHz = c.centre_hz
        detail = sdrkitText(c.detail)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdataFskLockEvent, SDRKitCode> {
        var c = sdrkit_hfdata_fsk_lock_event()
        let code = sdrkit_hfdata_fsk_lock_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdataFskLockEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The sync of the words, characters or frames was lost.
public struct HfdataFskLostEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// Why sync was lost.
    public var reason: String

    init(_ c: sdrkit_hfdata_fsk_lost_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdataFskLostEvent, SDRKitCode> {
        var c = sdrkit_hfdata_fsk_lost_event()
        let code = sdrkit_hfdata_fsk_lost_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdataFskLostEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A telegram of EFR.
public struct EfrTelegramEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The checksum: "good" or "bad".
    public var checksum: String
    /// The message number (user byte 1).
    public var message: Int64
    /// Address 1 (user byte 2).
    public var address1: Int64
    /// Address 2 (user byte 3).
    public var address2: Int64
    /// The number of payload bytes.
    public var payloadBytes: Int64
    /// The payload as hex.
    public var payload: String

    init(_ c: sdrkit_efr_telegram_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        checksum = sdrkitText(c.checksum)
        message = c.message
        address1 = c.address1
        address2 = c.address2
        payloadBytes = c.payload_bytes
        payload = sdrkitText(c.payload)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EfrTelegramEvent, SDRKitCode> {
        var c = sdrkit_efr_telegram_event()
        let code = sdrkit_efr_telegram_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EfrTelegramEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of G-TOR.
public struct GtorFrameEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The speed of the frame.
    public var baud: Double
    /// The command of a frame: data, changeover, disconnect, connect.
    public var command: String
    /// The block number, 0 to 3.
    public var block: Int64
    /// "plain" or "Golay".
    public var form: String
    /// The bits were inverted.
    public var inverted: Bool
    /// The compression: none, Huffman, swapped Huffman.
    public var compression: String

    init(_ c: sdrkit_gtor_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        baud = c.baud
        command = sdrkitText(c.command)
        block = c.block
        form = sdrkitText(c.form)
        inverted = c.inverted
        compression = sdrkitText(c.compression)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GtorFrameEvent, SDRKitCode> {
        var c = sdrkit_gtor_frame_event()
        let code = sdrkit_gtor_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GtorFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tone pairs of Piccolo were found.
public struct PiccoloPairsEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_piccolo_pairs_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PiccoloPairsEvent, SDRKitCode> {
        var c = sdrkit_piccolo_pairs_event()
        let code = sdrkit_piccolo_pairs_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PiccoloPairsEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tone pairs of Coquelet were found.
public struct CoqueletPairsEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_coquelet_pairs_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CoqueletPairsEvent, SDRKitCode> {
        var c = sdrkit_coquelet_pairs_event()
        let code = sdrkit_coquelet_pairs_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CoqueletPairsEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A PDU of 3G-ALE.
public struct Ale3gPduEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The burst (BW0 to BW4).
    public var burst: String
    /// The PDU type.
    public var type: String

    init(_ c: sdrkit_ale3g_pdu_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        burst = sdrkitText(c.burst)
        type = sdrkitText(c.type)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Ale3gPduEvent, SDRKitCode> {
        var c = sdrkit_ale3g_pdu_event()
        let code = sdrkit_ale3g_pdu_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Ale3gPduEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A complete datagram of 3G-ALE.
public struct Ale3gDatagramEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_ale3g_datagram_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Ale3gDatagramEvent, SDRKitCode> {
        var c = sdrkit_ale3g_datagram_event()
        let code = sdrkit_ale3g_datagram_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Ale3gDatagramEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An event of STANAG 4481.
public struct S4481Event {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_s4481_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<S4481Event, SDRKitCode> {
        var c = sdrkit_s4481_event()
        let code = sdrkit_s4481_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(S4481Event(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An event of Link 11.
public struct Link11Event {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_link11_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Link11Event, SDRKitCode> {
        var c = sdrkit_link11_event()
        let code = sdrkit_link11_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Link11Event(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of Link 11.
public struct Link11MessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the message.
    public var text: String
    /// Data frames.
    public var dataFrames: Int64
    /// The quarter turns of every data frame (15 digits a frame).
    public var steps: String

    init(_ c: sdrkit_link11_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        dataFrames = c.data_frames
        steps = sdrkitText(c.steps)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Link11MessageEvent, SDRKitCode> {
        var c = sdrkit_link11_message_event()
        let code = sdrkit_link11_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Link11MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A `bee` message of CIS-36-50.
public struct CisHfCisMessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The start word.
    public var startWord: String
    /// The session key.
    public var sessionKey: String
    /// Characters read.
    public var characters: Int64
    /// Characters that failed the 3:4 check.
    public var failing: Int64

    init(_ c: sdrkit_cis_hf_cis_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        startWord = sdrkitText(c.start_word)
        sessionKey = sdrkitText(c.session_key)
        characters = c.characters
        failing = c.failing
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CisHfCisMessageEvent, SDRKitCode> {
        var c = sdrkit_cis_hf_cis_message_event()
        let code = sdrkit_cis_hf_cis_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CisHfCisMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A signal was identified.
public struct CisHfCisIdentEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The name of the signal identified.
    public var identified: String

    init(_ c: sdrkit_cis_hf_cis_ident_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        identified = sdrkitText(c.identified)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CisHfCisIdentEvent, SDRKitCode> {
        var c = sdrkit_cis_hf_cis_ident_event()
        let code = sdrkit_cis_hf_cis_ident_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CisHfCisIdentEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An ALE call: `source` is the caller, `target` the called addresses and `text` the call for people.
public struct AleCallEvent {
    public enum TypeOption: Int32 {
        case sounding = 0
        case individual = 1
        case net = 2
        case group = 3
        case allCall = 4
        case anyCall = 5
        case response = 6
        case ack = 7
        case netResponse = 8
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
    /// The type of the call.
    public var type: TypeOption?
    /// The caller.
    public var source: String
    /// The called addresses.
    public var target: String
    /// The call for people.
    public var text: String
    /// TIS (else TWS).
    public var tis: Bool
    /// The AMD text.
    public var amd: String
    /// The commands.
    public var commands: String
    /// The bit error rate of the call.
    public var ber: Double
    /// The mean word SNR in 3 kHz.
    public var snrDb: Double
    /// The frequency correction at the end of the call.
    public var offsetHz: Double
    /// Words in the transmission.
    public var words: Int64
    /// What the event is about: a message.
    public var family: Family?

    init(_ c: sdrkit_ale_call_event) {
        time = c.time
        freqHz = c.freq_hz
        type = TypeOption(rawValue: c.type)
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        tis = c.tis
        amd = sdrkitText(c.amd)
        commands = sdrkitText(c.commands)
        ber = c.ber
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        words = c.words
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AleCallEvent, SDRKitCode> {
        var c = sdrkit_ale_call_event()
        let code = sdrkit_ale_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AleCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of the ALE transmission.
public struct AleTxEvent {
    public enum Kind: Int32 {
        case call = 0
        case sounding = 1
        case response = 2
        case ack = 3
        case amd = 4
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
    /// The state: started, finished or refused.
    public var state: String
    /// Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused.
    public var reason: String
    /// What is sent.
    public var kind: Kind?
    /// The called address of a call; empty for a sounding.
    public var to: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_ale_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        kind = Kind(rawValue: c.kind)
        to = sdrkitText(c.to)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AleTxEvent, SDRKitCode> {
        var c = sdrkit_ale_tx_event()
        let code = sdrkit_ale_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AleTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a step of the three-way handshake.
public struct AleLinkEvent {
    public enum State: Int32 {
        case idle = 0
        case calling = 1
        case responding = 2
        case linked = 3
        case timedOut = 4
        case notAnswered = 5
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
    /// The link state after the step.
    public var state: State?
    /// The other station.
    public var source: String
    /// The step: the call sent, the call heard, the response heard, the acknowledgement sent or heard, or why there was no answer.
    public var reason: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_ale_link_event) {
        time = c.time
        freqHz = c.freq_hz
        state = State(rawValue: c.state)
        source = sdrkitText(c.source)
        reason = sdrkitText(c.reason)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AleLinkEvent, SDRKitCode> {
        var c = sdrkit_ale_link_event()
        let code = sdrkit_ale_link_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AleLinkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A radar heard first, or again after a minute's pause. `source` is the keyed ID when it was read.
public struct HfradarRadarHeardEvent {
    public enum Direction: Int32 {
        case rising = 0
        case falling = 1
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
    /// The radar number.
    public var radar: Int64
    /// The radar band, if the radar is in one.
    public var band: String?
    /// The width of the sweep.
    public var width: Double
    /// The direction of the sweep.
    public var direction: Direction?
    /// The keyed ID, when read.
    public var source: String?
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_hfradar_radar_heard_event) {
        time = c.time
        freqHz = c.freq_hz
        radar = c.radar
        band = c.has_band ? sdrkitText(c.band) : nil
        width = c.width
        direction = Direction(rawValue: c.direction)
        source = c.has_source ? sdrkitText(c.source) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfradarRadarHeardEvent, SDRKitCode> {
        var c = sdrkit_hfradar_radar_heard_event()
        let code = sdrkit_hfradar_radar_heard_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfradarRadarHeardEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A new variant or carrier count: `text` describes the signal.
public struct HfdataPactorhfAnalysisEvent {
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
    /// The variant found.
    public var variant: Int64
    /// The signal in words.
    public var text: String
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_hfdata_pactorhf_analysis_event) {
        time = c.time
        freqHz = c.freq_hz
        variant = c.variant
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfdataPactorhfAnalysisEvent, SDRKitCode> {
        var c = sdrkit_hfdata_pactorhf_analysis_event()
        let code = sdrkit_hfdata_pactorhf_analysis_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfdataPactorhfAnalysisEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of PACTOR-I.
public struct Pactor1PactorFrameEvent {
    public enum Kind: Int32 {
        case data = 0
        case connect = 1
        case qrt = 2
        case cs1 = 3
        case cs2 = 4
        case cs3 = 5
        case cs4 = 6
        case badPacket = 7
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
    /// The kind of frame.
    public var kind: Kind?
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The header byte of a packet.
    public var header: Int64?
    /// The packet number 0 to 3.
    public var number: Int64?
    /// The sender; not read.
    public var source: String?
    /// The called station of a connect packet.
    public var target: String?
    /// The text of the frame.
    public var text: String
    /// The sending side, 1 or 2.
    public var side: Int64
    /// The tones were swapped.
    public var inverted: Bool
    /// A repeat of the packet before.
    public var `repeat`: Bool
    /// The SNR of the frame.
    public var snrDb: Double
    /// What the event is about: a message.
    public var family: Family?

    init(_ c: sdrkit_pactor1_pactor_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        kind = Kind(rawValue: c.kind)
        symbolRateHz = c.symbol_rate_hz
        header = c.has_header ? c.header : nil
        number = c.has_number ? c.number : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        text = sdrkitText(c.text)
        side = c.side
        inverted = c.inverted
        `repeat` = c.repeat
        snrDb = c.snr_db
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Pactor1PactorFrameEvent, SDRKitCode> {
        var c = sdrkit_pactor1_pactor_frame_event()
        let code = sdrkit_pactor1_pactor_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Pactor1PactorFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A beacon heard with its identity confirmed: `source` is the beacon's name, `text` the note.
public struct HfbeaconsHfbDetectionEvent {
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
    /// The beacon's name.
    public var source: String
    /// The SNR.
    public var snrDb: Double
    /// The confidence of the identity.
    public var confidence: Int64
    /// The note.
    public var text: String
    /// The position of the beacon, when known.
    public var position: SDRKitPosition?
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_hfbeacons_hfb_detection_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        snrDb = c.snr_db
        confidence = c.confidence
        text = sdrkitText(c.text)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfbeaconsHfbDetectionEvent, SDRKitCode> {
        var c = sdrkit_hfbeacons_hfb_detection_event()
        let code = sdrkit_hfbeacons_hfb_detection_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfbeaconsHfbDetectionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of the own beacon.
public struct HfbeaconsHfbTxEvent {
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
    /// The state: started (the first sending), sending (a later one), sent (a sending ended), finished (the beacon ended) or refused.
    public var state: String
    /// Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused.
    public var reason: String
    /// The text keyed.
    public var text: String
    /// The family of the source: a message.
    public var family: Family?

    init(_ c: sdrkit_hfbeacons_hfb_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfbeaconsHfbTxEvent, SDRKitCode> {
        var c = sdrkit_hfbeacons_hfb_tx_event()
        let code = sdrkit_hfbeacons_hfb_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfbeaconsHfbTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An accepted HXV vector.
public struct WavebuoyVectorEvent {
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
    /// The heave of the vector.
    public var heaveCm: Int64
    /// The north displacement of the vector.
    public var northCm: Int64
    /// The west displacement of the vector.
    public var westCm: Int64
    /// The cyclic data word of the vector.
    public var cyclic: Int64
    /// What the event is about: a sensor.
    public var family: Family?

    init(_ c: sdrkit_wavebuoy_vector_event) {
        time = c.time
        freqHz = c.freq_hz
        heaveCm = c.heave_cm
        northCm = c.north_cm
        westCm = c.west_cm
        cyclic = c.cyclic
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WavebuoyVectorEvent, SDRKitCode> {
        var c = sdrkit_wavebuoy_vector_event()
        let code = sdrkit_wavebuoy_vector_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WavebuoyVectorEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A status event of MIL-STD-188-110: sync, end of message, loss.
public struct Ms110Event {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_ms110_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Ms110Event, SDRKitCode> {
        var c = sdrkit_ms110_event()
        let code = sdrkit_ms110_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Ms110Event(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of MIL-STD-188-110 with its text.
public struct Ms110MessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the message.
    public var text: String
    /// The bytes of the message.
    public var bytes: Int64

    init(_ c: sdrkit_ms110_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        bytes = c.bytes
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Ms110MessageEvent, SDRKitCode> {
        var c = sdrkit_ms110_message_event()
        let code = sdrkit_ms110_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Ms110MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A status event of STANAG 4285: sync, end of message, loss.
public struct S4285Event {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_s4285_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<S4285Event, SDRKitCode> {
        var c = sdrkit_s4285_event()
        let code = sdrkit_s4285_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(S4285Event(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of STANAG 4285 with its text.
public struct S4285MessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the message.
    public var text: String
    /// The bytes of the message.
    public var bytes: Int64

    init(_ c: sdrkit_s4285_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        bytes = c.bytes
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<S4285MessageEvent, SDRKitCode> {
        var c = sdrkit_s4285_message_event()
        let code = sdrkit_s4285_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(S4285MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A status event of STANAG 4539: sync, end of message, loss.
public struct S4539Event {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String

    init(_ c: sdrkit_s4539_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<S4539Event, SDRKitCode> {
        var c = sdrkit_s4539_event()
        let code = sdrkit_s4539_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(S4539Event(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of STANAG 4539 with its text.
public struct S4539MessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the message.
    public var text: String
    /// The bytes of the message.
    public var bytes: Int64

    init(_ c: sdrkit_s4539_message_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        bytes = c.bytes
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<S4539MessageEvent, SDRKitCode> {
        var c = sdrkit_s4539_message_event()
        let code = sdrkit_s4539_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(S4539MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst demodulated, with its whole stream of phase steps.
public struct RprBurstEvent {
    public enum Modulation: Int32 {
        case dBpsk = 0
        case dQpsk = 1
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
    /// The speed level: 1 or 2.
    public var speedLevel: Int64
    /// The modulation.
    public var modulation: Modulation?
    /// The carriers with a clear step pattern.
    public var carriers: Int64
    /// The symbols demodulated on each carrier.
    public var symbols: Int64
    /// The length of the burst.
    public var length: Double
    /// The SNR from the phase error of its steps.
    public var snrDb: Double
    /// The comb centre against the audio centre setting.
    public var offsetHz: Double
    /// The step indices, one byte each (0 or 1 for D-BPSK, 0 to 3 for D-QPSK; the step is index x 360 / order degrees after the constant turn of the carrier): for each symbol time after the first, carrier 0 to 7 in turn.
    public var steps: [UInt8]
    /// The burst in words.
    public var text: String
    /// What the event is about.
    public var family: Family?

    init(_ c: sdrkit_rpr_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        speedLevel = c.speed_level
        modulation = Modulation(rawValue: c.modulation)
        carriers = c.carriers
        symbols = c.symbols
        length = c.length
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        steps = [UInt8](UnsafeBufferPointer(start: c.steps.ptr, count: c.steps.len))
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RprBurstEvent, SDRKitCode> {
        var c = sdrkit_rpr_burst_event()
        let code = sdrkit_rpr_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RprBurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum HfdataTables {
    /// The changed rows of the table "rows" of the stack "crowd36", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func crowd36Rows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_crowd36_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "rows" of the stack "xpa", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func xpaRows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_xpa_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "rows" of the stack "fsk200", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func fsk200Rows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsk200_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "rows" of the stack "gwfsk", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func gwfskRows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gwfsk_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "rows" of the stack "fsk_modems", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func fskModemsRows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsk_modems_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "rows" of the stack "uvb76", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func uvb76Rows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_uvb76_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "rows" of the stack "vlf_msk", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func vlfMskRows(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataSenderRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_sender_row](repeating: sdrkit_hfdata_sender_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_vlf_msk_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataSenderRow.init))
    }
    /// The changed rows of the table "telegrams" of the stack "efr", newest first, at most `capacity` rows.
    public static func efrTelegrams(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[EfrTelegramRow], SDRKitCode> {
        var rows = [sdrkit_efr_telegram_row](repeating: sdrkit_efr_telegram_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_efr_telegrams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EfrTelegramRow.init))
    }
    /// The changed rows of the table "pdus" of the stack "ale3g", newest first, at most `capacity` rows.
    public static func ale3gPdus(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[Ale3gPduRow], SDRKitCode> {
        var rows = [sdrkit_ale3g_pdu_row](repeating: sdrkit_ale3g_pdu_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale3g_pdus_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Ale3gPduRow.init))
    }
    /// The changed rows of the table "channels" of the stack "s4481", newest first, at most `capacity` rows.
    /// The column `channel` names a row.
    /// Default order: the column `channel`, lowest first.
    public static func s4481Channels(_ handle: SDRKIT_HANDLE, capacity: Int = 17) -> Result<[S4481ChannelRow], SDRKitCode> {
        var rows = [sdrkit_s4481_channel_row](repeating: sdrkit_s4481_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_s4481_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(S4481ChannelRow.init))
    }
    /// The changed rows of the table "messages" of the stack "link11", newest first, at most `capacity` rows.
    public static func link11Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[Link11MessageRow], SDRKitCode> {
        var rows = [sdrkit_link11_message_row](repeating: sdrkit_link11_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_link11_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Link11MessageRow.init))
    }
    /// The changed rows of the table "messages" of the stack "cis_hf", newest first, at most `capacity` rows.
    public static func cisHfMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[CisHfMessageRow], SDRKitCode> {
        var rows = [sdrkit_cis_hf_message_row](repeating: sdrkit_cis_hf_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cis_hf_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CisHfMessageRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "cis_hf", newest first, at most `capacity` rows.
    /// The column `number` names a row.
    /// Default order: the column `number`, lowest first.
    public static func cisHfCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 12) -> Result<[CisHfCarrierRow], SDRKitCode> {
        var rows = [sdrkit_cis_hf_carrier_row](repeating: sdrkit_cis_hf_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cis_hf_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CisHfCarrierRow.init))
    }
    /// The changed rows of the table "candidates" of the stack "hfid", newest first, at most `capacity` rows.
    /// The column `waveform` names a row.
    /// Default order: the column `confidence_pct`, highest first.
    public static func hfidCandidates(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[HfidCandidateRow], SDRKitCode> {
        var rows = [sdrkit_hfid_candidate_row](repeating: sdrkit_hfid_candidate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfid_candidates_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfidCandidateRow.init))
    }
    /// The changed rows of the table "events" of the stack "ale", newest first, at most `capacity` rows.
    /// The column `from` names a row.
    public static func aleEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AleEventRow], SDRKitCode> {
        var rows = [sdrkit_ale_event_row](repeating: sdrkit_ale_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AleEventRow.init))
    }
    /// The changed rows of the table "stations" of the stack "ale", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func aleStations(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AleStationRow], SDRKitCode> {
        var rows = [sdrkit_ale_station_row](repeating: sdrkit_ale_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AleStationRow.init))
    }
    /// The changed rows of the table "words" of the stack "ale", newest first, at most `capacity` rows.
    public static func aleWords(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[AleWordRow], SDRKitCode> {
        var rows = [sdrkit_ale_word_row](repeating: sdrkit_ale_word_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale_words_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AleWordRow.init))
    }
    /// The changed rows of the table "learned_nets" of the stack "ale", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func aleLearnedNets(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[AleNetRow], SDRKitCode> {
        var rows = [sdrkit_ale_net_row](repeating: sdrkit_ale_net_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale_learned_nets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AleNetRow.init))
    }
    /// The changed rows of the table "lqa" of the stack "ale", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func aleLqa(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AleLqaRow], SDRKitCode> {
        var rows = [sdrkit_ale_lqa_row](repeating: sdrkit_ale_lqa_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale_lqa_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AleLqaRow.init))
    }
    /// The changed rows of the table "messages" of the stack "ale", newest first, at most `capacity` rows.
    /// The column `from` names a row.
    public static func aleMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AleMessageRow], SDRKitCode> {
        var rows = [sdrkit_ale_message_row](repeating: sdrkit_ale_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ale_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AleMessageRow.init))
    }
    /// The changed rows of the table "radars" of the stack "hfradar", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func hfradarRadars(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[HfradarRadarRow], SDRKitCode> {
        var rows = [sdrkit_hfradar_radar_row](repeating: sdrkit_hfradar_radar_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfradar_radars_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfradarRadarRow.init))
    }
    /// The changed rows of the table "heard" of the stack "hfradar", newest first, at most `capacity` rows.
    public static func hfradarHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[HfradarHeardRow], SDRKitCode> {
        var rows = [sdrkit_hfradar_heard_row](repeating: sdrkit_hfradar_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfradar_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfradarHeardRow.init))
    }
    /// The changed rows of the table "bands" of the stack "hfradar", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Default order: the column `low`, lowest first.
    public static func hfradarBands(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[HfradarBandRow], SDRKitCode> {
        var rows = [sdrkit_hfradar_band_row](repeating: sdrkit_hfradar_band_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfradar_bands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfradarBandRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "pactor2", newest first, at most `capacity` rows.
    /// Default order: the column `frequency`, lowest first.
    public static func pactor2Carriers(_ handle: SDRKIT_HANDLE, capacity: Int = 18) -> Result<[HfdataPactorhfCarrierRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_pactorhf_carrier_row](repeating: sdrkit_hfdata_pactorhf_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor2_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataPactorhfCarrierRow.init))
    }
    /// The changed rows of the table "events" of the stack "pactor2", newest first, at most `capacity` rows.
    public static func pactor2Events(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[HfdataPactorhfEventRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_pactorhf_event_row](repeating: sdrkit_hfdata_pactorhf_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor2_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataPactorhfEventRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "pactor3", newest first, at most `capacity` rows.
    /// Default order: the column `frequency`, lowest first.
    public static func pactor3Carriers(_ handle: SDRKIT_HANDLE, capacity: Int = 18) -> Result<[HfdataPactorhfCarrierRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_pactorhf_carrier_row](repeating: sdrkit_hfdata_pactorhf_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor3_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataPactorhfCarrierRow.init))
    }
    /// The changed rows of the table "events" of the stack "pactor3", newest first, at most `capacity` rows.
    public static func pactor3Events(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[HfdataPactorhfEventRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_pactorhf_event_row](repeating: sdrkit_hfdata_pactorhf_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor3_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataPactorhfEventRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "pactor4", newest first, at most `capacity` rows.
    /// Default order: the column `frequency`, lowest first.
    public static func pactor4Carriers(_ handle: SDRKIT_HANDLE, capacity: Int = 18) -> Result<[HfdataPactorhfCarrierRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_pactorhf_carrier_row](repeating: sdrkit_hfdata_pactorhf_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor4_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataPactorhfCarrierRow.init))
    }
    /// The changed rows of the table "events" of the stack "pactor4", newest first, at most `capacity` rows.
    public static func pactor4Events(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[HfdataPactorhfEventRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_pactorhf_event_row](repeating: sdrkit_hfdata_pactorhf_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor4_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataPactorhfEventRow.init))
    }
    /// The changed rows of the table "frames" of the stack "pactor1", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    public static func pactor1Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[Pactor1FrameRow], SDRKitCode> {
        var rows = [sdrkit_pactor1_frame_row](repeating: sdrkit_pactor1_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor1_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Pactor1FrameRow.init))
    }
    /// The changed rows of the table "stations" of the stack "pactor1", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    public static func pactor1Stations(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[Pactor1StationRow], SDRKitCode> {
        var rows = [sdrkit_pactor1_station_row](repeating: sdrkit_pactor1_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pactor1_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Pactor1StationRow.init))
    }
    /// The changed rows of the table "beacons" of the stack "hfbeacons", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    public static func hfbeaconsBeacons(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[HfbeaconsBeaconRow], SDRKitCode> {
        var rows = [sdrkit_hfbeacons_beacon_row](repeating: sdrkit_hfbeacons_beacon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfbeacons_beacons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfbeaconsBeaconRow.init))
    }
    /// The changed rows of the table "bands" of the stack "hfbeacons", newest first, at most `capacity` rows.
    /// The column `band` names a row.
    public static func hfbeaconsBands(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[HfbeaconsBandRow], SDRKitCode> {
        var rows = [sdrkit_hfbeacons_band_row](repeating: sdrkit_hfbeacons_band_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfbeacons_bands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfbeaconsBandRow.init))
    }
    /// The changed rows of the table "propagation" of the stack "hfbeacons", newest first, at most `capacity` rows.
    /// The column `band` names a row.
    public static func hfbeaconsPropagation(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[HfbeaconsPropagationRow], SDRKitCode> {
        var rows = [sdrkit_hfbeacons_propagation_row](repeating: sdrkit_hfbeacons_propagation_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hfbeacons_propagation_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfbeaconsPropagationRow.init))
    }
    /// The changed rows of the table "spectrum" of the stack "wavebuoy", newest first, at most `capacity` rows.
    /// Default order: the column `entry`, lowest first.
    public static func wavebuoySpectrum(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WavebuoySpectrumRow], SDRKitCode> {
        var rows = [sdrkit_wavebuoy_spectrum_row](repeating: sdrkit_wavebuoy_spectrum_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wavebuoy_spectrum_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WavebuoySpectrumRow.init))
    }
    /// The changed rows of the table "system_words" of the stack "wavebuoy", newest first, at most `capacity` rows.
    /// The column `word` names a row.
    /// Default order: the column `word`, lowest first.
    public static func wavebuoySystemWords(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[WavebuoySystemWordRow], SDRKitCode> {
        var rows = [sdrkit_wavebuoy_system_word_row](repeating: sdrkit_wavebuoy_system_word_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wavebuoy_system_words_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WavebuoySystemWordRow.init))
    }
    /// The changed rows of the table "frames" of the stack "wavebuoy", newest first, at most `capacity` rows.
    public static func wavebuoyFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 40) -> Result<[WavebuoyFrameRow], SDRKitCode> {
        var rows = [sdrkit_wavebuoy_frame_row](repeating: sdrkit_wavebuoy_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wavebuoy_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WavebuoyFrameRow.init))
    }
    /// The changed rows of the table "dpdus" of the stack "ms110", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    public static func ms110Dpdus(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataDpduRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_dpdu_row](repeating: sdrkit_hfdata_dpdu_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ms110_dpdus_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataDpduRow.init))
    }
    /// The changed rows of the table "dpdus" of the stack "s4285", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    public static func s4285Dpdus(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataDpduRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_dpdu_row](repeating: sdrkit_hfdata_dpdu_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_s4285_dpdus_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataDpduRow.init))
    }
    /// The changed rows of the table "dpdus" of the stack "s4539", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    public static func s4539Dpdus(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HfdataDpduRow], SDRKitCode> {
        var rows = [sdrkit_hfdata_dpdu_row](repeating: sdrkit_hfdata_dpdu_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_s4539_dpdus_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfdataDpduRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "rpr", newest first, at most `capacity` rows.
    /// Default order: the column `frequency`, lowest first.
    public static func rprCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[RprCarrierRow], SDRKitCode> {
        var rows = [sdrkit_rpr_carrier_row](repeating: sdrkit_rpr_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rpr_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RprCarrierRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "rpr", newest first, at most `capacity` rows.
    public static func rprBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[RprBurstRow], SDRKitCode> {
        var rows = [sdrkit_rpr_burst_row](repeating: sdrkit_rpr_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rpr_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RprBurstRow.init))
    }
    /// The changed rows of the table "events" of the stack "rpr", newest first, at most `capacity` rows.
    public static func rprEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[RprEventRow], SDRKitCode> {
        var rows = [sdrkit_rpr_event_row](repeating: sdrkit_rpr_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rpr_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RprEventRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum HfdataCommands {
    /// Clear. Forgets everything decoded so far.
    public static func crowd36Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_crowd36_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func crowd36Reset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_crowd36_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func xpaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_xpa_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func xpaReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_xpa_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fsk200Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk200_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func fsk200Reset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk200_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func gwfskClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gwfsk_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func gwfskReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gwfsk_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fskModemsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk_modems_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func fskModemsReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk_modems_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func uvb76Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_uvb76_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func uvb76Reset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_uvb76_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func vlfMskClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vlf_msk_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func vlfMskReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_vlf_msk_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func efrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_efr_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func efrReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_efr_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func gtorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gtor_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func gtorReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gtor_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func arqeClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_arqe_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func arqeReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_arqe_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func autospecClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_autospec_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func autospecReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_autospec_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func piccoloClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_piccolo_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func piccoloReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_piccolo_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func coqueletClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_coquelet_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func coqueletReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_coquelet_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ale3gClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ale3g_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func s4481Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_s4481_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func link11Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_link11_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cisHfClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cis_hf_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Searches the signal again; keeps the text and the tables.
    public static func cisHfReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cis_hf_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func aleClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ale_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func aleRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ale_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_ale_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ale_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Call. Sends one individual call on this channel: the called address in TO words the calling cycle repeats times, then the own address in TIS words. Transmit must be on and the own address set. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func aleCall(_ handle: SDRKIT_HANDLE, to: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ale_call_args()
        args.size = UInt32(MemoryLayout<sdrkit_ale_call_args>.size)
        let code: Int32 = sdrkitWithString(to) { to_ in { () -> Int32 in
        args.to = to_
        return sdrkit_ale_call(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Sound. Sends one sounding on this channel: the own address in TWS or TIS words, the calling cycle repeats times. Transmit must be on and the own address set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func aleSound(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ale_sound(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the call or the sounding at once.
    public static func aleStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ale_stop(handle)
        return sdrkitResult(code)
    }
    /// Send AMD text. Sends an AMD text. To the linked station: the called address twice, the text, the own address in TWS words.          To another station: a call with the text before the own address, which starts the three-way handshake.          Transmit must be on and the own address set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func aleAmd(_ handle: SDRKIT_HANDLE, to: String, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ale_amd_args()
        args.size = UInt32(MemoryLayout<sdrkit_ale_amd_args>.size)
        let code: Int32 = sdrkitWithString(to) { to_ in sdrkitWithString(text) { text_ in { () -> Int32 in
        args.to = to_
        args.text = text_
        return sdrkit_ale_amd(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func hfradarClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfradar_clear(handle)
        return sdrkitResult(code)
    }
    /// Tune to a band. Asks the host to tune to the middle of an oceanographic radar band. The radio takes in one part of the band at a time (1.8 MHz at 2 Msps, so a whole band at once from about 12 Msps for 12 - 14 MHz): a wide radio sees all radars of a band together, a narrow one needs the scan. Refused while the scan is on.
    public static func hfradarBandTune(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_hfradar_band_args()
        args.size = UInt32(MemoryLayout<sdrkit_hfradar_band_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_hfradar_band_tune(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func pactor2Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pactor2_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func pactor3Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pactor3_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func pactor4Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pactor4_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func pactor1Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pactor1_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func pactor1Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_pactor1_station_args()
        args.size = UInt32(MemoryLayout<sdrkit_pactor1_station_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_pactor1_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func hfbeaconsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfbeacons_clear(handle)
        return sdrkitResult(code)
    }
    /// Restore the built-in list. Goes back to the built-in beacon list.
    public static func hfbeaconsListRestore(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfbeacons_list_restore(handle)
        return sdrkitResult(code)
    }
    /// Start the beacon. Sends the message in Morse at offset_s after the next interval boundary of UTC (at once when that is now), then again each interval, until stop or count sendings. Each sending is one transmission; between sendings the transmitter is free. Transmit must be on and the call sign set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func hfbeaconsStart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfbeacons_start(handle)
        return sdrkitResult(code)
    }
    /// Stop the beacon. Ends the sending on air within 50 ms, with its fall, and ends the schedule.
    public static func hfbeaconsStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfbeacons_stop(handle)
        return sdrkitResult(code)
    }
    /// Send the beacon once. Sends the message in Morse once, at once, outside the schedule. Transmit must be on and the call sign set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func hfbeaconsSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hfbeacons_send(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the bits, frames, vectors and system words.
    public static func wavebuoyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wavebuoy_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ms110Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ms110_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func s4285Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_s4285_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func s4539Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_s4539_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func rprClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rpr_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum HfdataSettings {
    /// Sync high tone.
    public static func crowd36SyncHighTone(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sync_high_tone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36SyncHighTone(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sync_high_tone", value))
    }
    /// The options of the setting "sideband" of the stack "crowd36".
    public enum Crowd36Sideband: Int32 {
        case upper = 0
        case lower = 1
    }
    /// Sideband.
    public static func crowd36Sideband(_ handle: SDRKIT_HANDLE) -> Result<Crowd36Sideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sideband", &v)
        return code == SDRKIT_OK ? .success(Crowd36Sideband(rawValue: v) ?? Crowd36Sideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36Sideband(_ handle: SDRKIT_HANDLE, _ value: Crowd36Sideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sideband", value.rawValue))
    }
    /// The options of the setting "speed" of the stack "crowd36".
    public enum Crowd36Speed: Int32 {
        case n40Bd = 0
        case n20Bd = 1
        case n10Bd = 2
    }
    /// Speed.
    public static func crowd36Speed(_ handle: SDRKIT_HANDLE) -> Result<Crowd36Speed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "speed", &v)
        return code == SDRKIT_OK ? .success(Crowd36Speed(rawValue: v) ?? Crowd36Speed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36Speed(_ handle: SDRKIT_HANDLE, _ value: Crowd36Speed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "speed", value.rawValue))
    }
    /// Search width.
    public static func crowd36SearchHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36SearchHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_hz", value))
    }
    /// The options of the setting "submode" of the stack "xpa".
    public enum XpaSubmode: Int32 {
        case xpa = 0
        case xpa2 = 1
    }
    /// Signal.
    public static func xpaSubmode(_ handle: SDRKIT_HANDLE) -> Result<XpaSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(XpaSubmode(rawValue: v) ?? XpaSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaSubmode(_ handle: SDRKIT_HANDLE, _ value: XpaSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// The options of the setting "speed" of the stack "xpa".
    public enum XpaSpeed: Int32 {
        case auto = 0
        case n10Bd = 1
        case n20Bd = 2
    }
    /// Speed.
    public static func xpaSpeed(_ handle: SDRKIT_HANDLE) -> Result<XpaSpeed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "speed", &v)
        return code == SDRKIT_OK ? .success(XpaSpeed(rawValue: v) ?? XpaSpeed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaSpeed(_ handle: SDRKIT_HANDLE, _ value: XpaSpeed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "speed", value.rawValue))
    }
    /// The options of the setting "sideband" of the stack "xpa".
    public enum XpaSideband: Int32 {
        case auto = 0
        case upper = 1
        case lower = 2
    }
    /// Sideband.
    public static func xpaSideband(_ handle: SDRKIT_HANDLE) -> Result<XpaSideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sideband", &v)
        return code == SDRKIT_OK ? .success(XpaSideband(rawValue: v) ?? XpaSideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaSideband(_ handle: SDRKIT_HANDLE, _ value: XpaSideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sideband", value.rawValue))
    }
    /// The options of the setting "numbering" of the stack "xpa".
    public enum XpaNumbering: Int32 {
        case auto = 0
        case minus3 = 1
        case minus2 = 2
        case minus1 = 3
        case none = 4
        case plus1 = 5
        case plus2 = 6
        case plus3 = 7
    }
    /// Tone numbering correction.
    public static func xpaNumbering(_ handle: SDRKIT_HANDLE) -> Result<XpaNumbering, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "numbering", &v)
        return code == SDRKIT_OK ? .success(XpaNumbering(rawValue: v) ?? XpaNumbering(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaNumbering(_ handle: SDRKIT_HANDLE, _ value: XpaNumbering) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "numbering", value.rawValue))
    }
    /// Search width.
    public static func xpaSearchHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaSearchHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_hz", value))
    }
    /// The options of the setting "submode" of the stack "fsk200".
    public enum Fsk200Submode: Int32 {
        case f01 = 0
        case f06 = 1
    }
    /// Signal.
    public static func fsk200Submode(_ handle: SDRKIT_HANDLE) -> Result<Fsk200Submode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(Fsk200Submode(rawValue: v) ?? Fsk200Submode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Submode(_ handle: SDRKIT_HANDLE, _ value: Fsk200Submode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// The options of the setting "polarity" of the stack "fsk200".
    public enum Fsk200Polarity: Int32 {
        case auto = 0
        case normal = 1
        case reversed = 2
    }
    /// Tone sense.
    public static func fsk200Polarity(_ handle: SDRKIT_HANDLE) -> Result<Fsk200Polarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "polarity", &v)
        return code == SDRKIT_OK ? .success(Fsk200Polarity(rawValue: v) ?? Fsk200Polarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Polarity(_ handle: SDRKIT_HANDLE, _ value: Fsk200Polarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "polarity", value.rawValue))
    }
    /// The options of the setting "text_case" of the stack "fsk200".
    public enum Fsk200TextCase: Int32 {
        case figures = 0
        case ita2 = 1
    }
    /// Case.
    public static func fsk200TextCase(_ handle: SDRKIT_HANDLE) -> Result<Fsk200TextCase, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "text_case", &v)
        return code == SDRKIT_OK ? .success(Fsk200TextCase(rawValue: v) ?? Fsk200TextCase(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200TextCase(_ handle: SDRKIT_HANDLE, _ value: Fsk200TextCase) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "text_case", value.rawValue))
    }
    /// Sync word errors accepted.
    public static func fsk200SyncErrors(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sync_errors", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200SyncErrors(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sync_errors", value))
    }
    /// Correct one bit by the CRC.
    public static func fsk200CorrectOneBit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "correct_one_bit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200CorrectOneBit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "correct_one_bit", value))
    }
    /// The options of the setting "polarity" of the stack "gwfsk".
    public enum GwfskPolarity: Int32 {
        case auto = 0
        case normal = 1
        case reversed = 2
    }
    /// Bit sense.
    public static func gwfskPolarity(_ handle: SDRKIT_HANDLE) -> Result<GwfskPolarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "polarity", &v)
        return code == SDRKIT_OK ? .success(GwfskPolarity(rawValue: v) ?? GwfskPolarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskPolarity(_ handle: SDRKIT_HANDLE, _ value: GwfskPolarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "polarity", value.rawValue))
    }
    /// Show bursts not understood.
    public static func gwfskShowUnknown(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "show_unknown", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskShowUnknown(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "show_unknown", value))
    }
    /// Shore station names.
    public static func gwfskStations(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "stations", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskStations(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "stations", $0) })
    }
    /// The options of the setting "submode" of the stack "fsk_modems".
    public enum FskModemsSubmode: Int32 {
        case auto = 0
        case bell103Orig = 1
        case bell103Ans = 2
        case v21Ch1 = 3
        case v21Ch2 = 4
        case v23Mode2 = 5
        case bell202 = 6
    }
    /// Standard.
    public static func fskModemsSubmode(_ handle: SDRKIT_HANDLE) -> Result<FskModemsSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(FskModemsSubmode(rawValue: v) ?? FskModemsSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsSubmode(_ handle: SDRKIT_HANDLE, _ value: FskModemsSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Dial error searched.
    public static func fskModemsDialSearchHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_search_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsDialSearchHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_search_hz", value))
    }
    /// The options of the setting "sideband" of the stack "fsk_modems".
    public enum FskModemsSideband: Int32 {
        case auto = 0
        case upper = 1
        case lower = 2
    }
    /// Sideband.
    public static func fskModemsSideband(_ handle: SDRKIT_HANDLE) -> Result<FskModemsSideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sideband", &v)
        return code == SDRKIT_OK ? .success(FskModemsSideband(rawValue: v) ?? FskModemsSideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsSideband(_ handle: SDRKIT_HANDLE, _ value: FskModemsSideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sideband", value.rawValue))
    }
    /// Shortest buzz.
    public static func uvb76MinBuzzS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_buzz_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76MinBuzzS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_buzz_s", value))
    }
    /// Longest buzz.
    public static func uvb76LongTxS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "long_tx_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76LongTxS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "long_tx_s", value))
    }
    /// Carrier line share.
    public static func uvb76CarrierShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "carrier_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76CarrierShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "carrier_share", value))
    }
    /// Report time.
    public static func vlfMskReportS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "report_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskReportS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "report_s", value))
    }
    /// Standard rates.
    public static func vlfMskRates(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "rates", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskRates(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "rates", $0) })
    }
    /// Rate tolerance.
    public static func vlfMskRateTolerance(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "rate_tolerance", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskRateTolerance(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "rate_tolerance", value))
    }
    /// Station hint window.
    public static func vlfMskHintHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hint_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskHintHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hint_hz", value))
    }
    /// Station list.
    public static func vlfMskStations(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "stations", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskStations(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "stations", $0) })
    }
    /// Shift.
    public static func efrShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEfrShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "shift_hz", value))
    }
    /// The options of the setting "polarity" of the stack "efr".
    public enum EfrPolarity: Int32 {
        case auto = 0
        case normal = 1
        case inverted = 2
    }
    /// Polarity.
    public static func efrPolarity(_ handle: SDRKIT_HANDLE) -> Result<EfrPolarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "polarity", &v)
        return code == SDRKIT_OK ? .success(EfrPolarity(rawValue: v) ?? EfrPolarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEfrPolarity(_ handle: SDRKIT_HANDLE, _ value: EfrPolarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "polarity", value.rawValue))
    }
    /// Speed.
    public static func efrBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEfrBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "baud", value))
    }
    /// Longest frame.
    public static func efrMaxLen(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_len", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEfrMaxLen(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_len", value))
    }
    /// The options of the setting "baud" of the stack "gtor".
    public enum GtorBaud: Int32 {
        case auto = 0
        case n100Bd = 1
        case n200Bd = 2
        case n300Bd = 3
    }
    /// Speed.
    public static func gtorBaud(_ handle: SDRKIT_HANDLE) -> Result<GtorBaud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(GtorBaud(rawValue: v) ?? GtorBaud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGtorBaud(_ handle: SDRKIT_HANDLE, _ value: GtorBaud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "baud", value.rawValue))
    }
    /// Shift.
    public static func gtorShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGtorShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "shift_hz", value))
    }
    /// The options of the setting "submode" of the stack "arqe".
    public enum ArqeSubmode: Int32 {
        case arqe = 0
        case arqn = 1
    }
    /// Signal.
    public static func arqeSubmode(_ handle: SDRKIT_HANDLE) -> Result<ArqeSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(ArqeSubmode(rawValue: v) ?? ArqeSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeSubmode(_ handle: SDRKIT_HANDLE, _ value: ArqeSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Speed.
    public static func arqeBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "baud", value))
    }
    /// Shift.
    public static func arqeShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "shift_hz", value))
    }
    /// The options of the setting "polarity" of the stack "arqe".
    public enum ArqePolarity: Int32 {
        case auto = 0
        case normal = 1
        case inverted = 2
    }
    /// Polarity.
    public static func arqePolarity(_ handle: SDRKIT_HANDLE) -> Result<ArqePolarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "polarity", &v)
        return code == SDRKIT_OK ? .success(ArqePolarity(rawValue: v) ?? ArqePolarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqePolarity(_ handle: SDRKIT_HANDLE, _ value: ArqePolarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "polarity", value.rawValue))
    }
    /// The options of the setting "cycle" of the stack "arqe".
    public enum ArqeCycle: Int32 {
        case auto = 0
        case none = 1
        case every4thWord = 2
        case every8thWord = 3
    }
    /// Inversion cycle.
    public static func arqeCycle(_ handle: SDRKIT_HANDLE) -> Result<ArqeCycle, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "cycle", &v)
        return code == SDRKIT_OK ? .success(ArqeCycle(rawValue: v) ?? ArqeCycle(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeCycle(_ handle: SDRKIT_HANDLE, _ value: ArqeCycle) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "cycle", value.rawValue))
    }
    /// The options of the setting "alphabet" of the stack "arqe".
    public enum ArqeAlphabet: Int32 {
        case us = 0
        case itu = 1
    }
    /// Figures case.
    public static func arqeAlphabet(_ handle: SDRKIT_HANDLE) -> Result<ArqeAlphabet, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "alphabet", &v)
        return code == SDRKIT_OK ? .success(ArqeAlphabet(rawValue: v) ?? ArqeAlphabet(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeAlphabet(_ handle: SDRKIT_HANDLE, _ value: ArqeAlphabet) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "alphabet", value.rawValue))
    }
    /// The options of the setting "word_code" of the stack "arqe".
    public enum ArqeWordCode: Int32 {
        case parity = 0
        case f342 = 1
    }
    /// Word code.
    public static func arqeWordCode(_ handle: SDRKIT_HANDLE) -> Result<ArqeWordCode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "word_code", &v)
        return code == SDRKIT_OK ? .success(ArqeWordCode(rawValue: v) ?? ArqeWordCode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeWordCode(_ handle: SDRKIT_HANDLE, _ value: ArqeWordCode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "word_code", value.rawValue))
    }
    /// The options of the setting "channels" of the stack "arqe".
    public enum ArqeChannels: Int32 {
        case auto = 0
        case oneChannel = 1
        case twoChannels = 2
        case fourChannels = 3
    }
    /// Channels.
    public static func arqeChannels(_ handle: SDRKIT_HANDLE) -> Result<ArqeChannels, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(ArqeChannels(rawValue: v) ?? ArqeChannels(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeChannels(_ handle: SDRKIT_HANDLE, _ value: ArqeChannels) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channels", value.rawValue))
    }
    /// The options of the setting "channel" of the stack "arqe".
    public enum ArqeChannel: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
    }
    /// Channel shown.
    public static func arqeChannel(_ handle: SDRKIT_HANDLE) -> Result<ArqeChannel, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(ArqeChannel(rawValue: v) ?? ArqeChannel(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeChannel(_ handle: SDRKIT_HANDLE, _ value: ArqeChannel) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel", value.rawValue))
    }
    /// The options of the setting "submode" of the stack "autospec".
    public enum AutospecSubmode: Int32 {
        case autospec = 0
        case spread = 1
    }
    /// Signal.
    public static func autospecSubmode(_ handle: SDRKIT_HANDLE) -> Result<AutospecSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(AutospecSubmode(rawValue: v) ?? AutospecSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecSubmode(_ handle: SDRKIT_HANDLE, _ value: AutospecSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Speed.
    public static func autospecBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "baud", value))
    }
    /// Shift.
    public static func autospecShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "shift_hz", value))
    }
    /// The options of the setting "polarity" of the stack "autospec".
    public enum AutospecPolarity: Int32 {
        case auto = 0
        case normal = 1
        case inverted = 2
    }
    /// Polarity.
    public static func autospecPolarity(_ handle: SDRKIT_HANDLE) -> Result<AutospecPolarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "polarity", &v)
        return code == SDRKIT_OK ? .success(AutospecPolarity(rawValue: v) ?? AutospecPolarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecPolarity(_ handle: SDRKIT_HANDLE, _ value: AutospecPolarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "polarity", value.rawValue))
    }
    /// The options of the setting "depth" of the stack "autospec".
    public enum AutospecDepth: Int32 {
        case auto = 0
        case n11 = 1
        case n21 = 2
        case n51 = 3
    }
    /// Spread depth.
    public static func autospecDepth(_ handle: SDRKIT_HANDLE) -> Result<AutospecDepth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "depth", &v)
        return code == SDRKIT_OK ? .success(AutospecDepth(rawValue: v) ?? AutospecDepth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecDepth(_ handle: SDRKIT_HANDLE, _ value: AutospecDepth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "depth", value.rawValue))
    }
    /// The options of the setting "alphabet" of the stack "autospec".
    public enum AutospecAlphabet: Int32 {
        case us = 0
        case itu = 1
    }
    /// Figures case.
    public static func autospecAlphabet(_ handle: SDRKIT_HANDLE) -> Result<AutospecAlphabet, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "alphabet", &v)
        return code == SDRKIT_OK ? .success(AutospecAlphabet(rawValue: v) ?? AutospecAlphabet(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecAlphabet(_ handle: SDRKIT_HANDLE, _ value: AutospecAlphabet) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "alphabet", value.rawValue))
    }
    /// Idle character.
    public static func autospecIdleCode(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "idle_code", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecIdleCode(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "idle_code", value))
    }
    /// The options of the setting "alphabet" of the stack "piccolo".
    public enum PiccoloAlphabet: Int32 {
        case us = 0
        case itu = 1
    }
    /// Figures case.
    public static func piccoloAlphabet(_ handle: SDRKIT_HANDLE) -> Result<PiccoloAlphabet, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "alphabet", &v)
        return code == SDRKIT_OK ? .success(PiccoloAlphabet(rawValue: v) ?? PiccoloAlphabet(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPiccoloAlphabet(_ handle: SDRKIT_HANDLE, _ value: PiccoloAlphabet) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "alphabet", value.rawValue))
    }
    /// Tone spacing.
    public static func piccoloSpacingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "spacing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPiccoloSpacingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "spacing_hz", value))
    }
    /// Tone length.
    public static func piccoloToneMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tone_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPiccoloToneMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tone_ms", value))
    }
    /// The options of the setting "tone_ms" of the stack "coquelet".
    public enum CoqueletToneMs: Int32 {
        case auto = 0
        case n375 = 1
        case n50Ms = 2
        case n75Ms = 3
    }
    /// Tone length.
    public static func coqueletToneMs(_ handle: SDRKIT_HANDLE) -> Result<CoqueletToneMs, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tone_ms", &v)
        return code == SDRKIT_OK ? .success(CoqueletToneMs(rawValue: v) ?? CoqueletToneMs(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCoqueletToneMs(_ handle: SDRKIT_HANDLE, _ value: CoqueletToneMs) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tone_ms", value.rawValue))
    }
    /// The options of the setting "alphabet" of the stack "coquelet".
    public enum CoqueletAlphabet: Int32 {
        case us = 0
        case itu = 1
    }
    /// Figures case.
    public static func coqueletAlphabet(_ handle: SDRKIT_HANDLE) -> Result<CoqueletAlphabet, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "alphabet", &v)
        return code == SDRKIT_OK ? .success(CoqueletAlphabet(rawValue: v) ?? CoqueletAlphabet(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCoqueletAlphabet(_ handle: SDRKIT_HANDLE, _ value: CoqueletAlphabet) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "alphabet", value.rawValue))
    }
    /// Tone spacing.
    public static func coqueletSpacingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "spacing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCoqueletSpacingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "spacing_hz", value))
    }
    /// BW2 PN taps.
    public static func ale3gBw2Taps(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "bw2_taps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAle3gBw2Taps(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "bw2_taps", value))
    }
    /// The options of the setting "channel_mode" of the stack "s4481".
    public enum S4481ChannelMode: Int32 {
        case auto = 0
        case single = 1
        case multi = 2
    }
    /// Channels.
    public static func s4481ChannelMode(_ handle: SDRKIT_HANDLE) -> Result<S4481ChannelMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel_mode", &v)
        return code == SDRKIT_OK ? .success(S4481ChannelMode(rawValue: v) ?? S4481ChannelMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4481ChannelMode(_ handle: SDRKIT_HANDLE, _ value: S4481ChannelMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel_mode", value.rawValue))
    }
    /// The options of the setting "baud" of the stack "s4481".
    public enum S4481Baud: Int32 {
        case auto = 0
        case n4545 = 1
        case n50Bd = 2
        case n75Bd = 3
        case n100Bd = 4
    }
    /// Speed.
    public static func s4481Baud(_ handle: SDRKIT_HANDLE) -> Result<S4481Baud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(S4481Baud(rawValue: v) ?? S4481Baud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4481Baud(_ handle: SDRKIT_HANDLE, _ value: S4481Baud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "baud", value.rawValue))
    }
    /// The options of the setting "alphabet" of the stack "s4481".
    public enum S4481Alphabet: Int32 {
        case us = 0
        case itu = 1
    }
    /// Figures case.
    public static func s4481Alphabet(_ handle: SDRKIT_HANDLE) -> Result<S4481Alphabet, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "alphabet", &v)
        return code == SDRKIT_OK ? .success(S4481Alphabet(rawValue: v) ?? S4481Alphabet(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4481Alphabet(_ handle: SDRKIT_HANDLE, _ value: S4481Alphabet) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "alphabet", value.rawValue))
    }
    /// The options of the setting "frame_rate" of the stack "link11".
    public enum Link11FrameRate: Int32 {
        case auto = 0
        case fast = 1
        case slow = 2
    }
    /// Frame rate.
    public static func link11FrameRate(_ handle: SDRKIT_HANDLE) -> Result<Link11FrameRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_rate", &v)
        return code == SDRKIT_OK ? .success(Link11FrameRate(rawValue: v) ?? Link11FrameRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLink11FrameRate(_ handle: SDRKIT_HANDLE, _ value: Link11FrameRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_rate", value.rawValue))
    }
    /// The options of the setting "submode" of the stack "cis_hf".
    public enum CisHfSubmode: Int32 {
        case bee = 0
        case cis14 = 1
        case cis12 = 2
        case cis36 = 3
        case ofdmHdr = 4
        case cis128 = 5
        case cis3000 = 6
    }
    /// Signal.
    public static func cisHfSubmode(_ handle: SDRKIT_HANDLE) -> Result<CisHfSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(CisHfSubmode(rawValue: v) ?? CisHfSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfSubmode(_ handle: SDRKIT_HANDLE, _ value: CisHfSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Speed.
    public static func cisHfBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "baud", value))
    }
    /// Shift.
    public static func cisHfShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "shift_hz", value))
    }
    /// CIS-3000 roll-off.
    public static func cisHfRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// Net addresses.
    public static func aleNets(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "nets", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAleNets(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "nets", $0) })
    }
    /// Listen.
    public static func aleListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "demod" of the stack "ale".
    public enum AleDemod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func aleDemod(_ handle: SDRKIT_HANDLE) -> Result<AleDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(AleDemod(rawValue: v) ?? AleDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAleDemod(_ handle: SDRKIT_HANDLE, _ value: AleDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Own address.
    public static func aleSelfAddress(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "self_address", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAleSelfAddress(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "self_address", $0) })
    }
    /// Calling cycle repeats.
    public static func aleCallRepeats(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "call_repeats", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleCallRepeats(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "call_repeats", value))
    }
    /// The options of the setting "sounding" of the stack "ale".
    public enum AleSounding: Int32 {
        case tws = 0
        case tis = 1
    }
    /// Sounding word.
    public static func aleSounding(_ handle: SDRKIT_HANDLE) -> Result<AleSounding, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sounding", &v)
        return code == SDRKIT_OK ? .success(AleSounding(rawValue: v) ?? AleSounding(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAleSounding(_ handle: SDRKIT_HANDLE, _ value: AleSounding) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sounding", value.rawValue))
    }
    /// Scanned channels.
    public static func aleScanChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "scan_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleScanChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "scan_channels", value))
    }
    /// Dwell on a channel.
    public static func aleScanDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "scan_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleScanDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "scan_dwell_s", value))
    }
    /// Answer calls.
    public static func aleAutoAnswer(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_answer", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleAutoAnswer(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_answer", value))
    }
    /// Exchange link quality.
    public static func aleLqa(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "lqa", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleLqa(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "lqa", value))
    }
    /// Wait for an answer.
    public static func aleAnswerWaitS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "answer_wait_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleAnswerWaitS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "answer_wait_s", value))
    }
    /// Scan the radar bands.
    public static func hfradarScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfradarScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Dwell.
    public static func hfradarDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfradarDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell_s", value))
    }
    /// Analysis interval.
    public static func pactor2AnalysisIntervalS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor2AnalysisIntervalS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value))
    }
    /// Analysis interval.
    public static func pactor3AnalysisIntervalS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor3AnalysisIntervalS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value))
    }
    /// Analysis interval.
    public static func pactor4AnalysisIntervalS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor4AnalysisIntervalS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value))
    }
    /// Listen.
    public static func pactor1Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor1Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// The options of the setting "demod" of the stack "pactor1".
    public enum Pactor1Demod: Int32 {
        case auto = 0
        case usb = 1
        case lsb = 2
        case nfm = 3
        case am = 4
    }
    /// Demodulator.
    public static func pactor1Demod(_ handle: SDRKIT_HANDLE) -> Result<Pactor1Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(Pactor1Demod(rawValue: v) ?? Pactor1Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor1Demod(_ handle: SDRKIT_HANDLE, _ value: Pactor1Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Huffman table.
    public static func pactor1HuffmanTable(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "huffman_table", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor1HuffmanTable(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "huffman_table", $0) })
    }
    /// Frequency error.
    public static func hfbeaconsPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ppm", value))
    }
    /// Step through the bands.
    public static func hfbeaconsHop(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsHop(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop", value))
    }
    /// Dwell.
    public static func hfbeaconsDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell_s", value))
    }
    /// NCDXF/IARU.
    public static func hfbeaconsWatchNcdxf(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "watch_ncdxf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsWatchNcdxf(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "watch_ncdxf", value))
    }
    /// Long Dashes.
    public static func hfbeaconsWatchDashes(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "watch_dashes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsWatchDashes(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "watch_dashes", value))
    }
    /// Single letters.
    public static func hfbeaconsWatchLetters(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "watch_letters", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsWatchLetters(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "watch_letters", value))
    }
    /// Your carriers.
    public static func hfbeaconsWatchCarriers(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "watch_carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsWatchCarriers(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "watch_carriers", value))
    }
    /// Beacon list.
    public static func hfbeaconsListJson(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "list_json", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsListJson(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "list_json", $0) })
    }
    /// Clock offset.
    public static func hfbeaconsClockOffsetS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock_offset_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsClockOffsetS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock_offset_s", value))
    }
    /// Recording start (UTC seconds; 0 unknown).
    public static func hfbeaconsStartUtcS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_utc_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsStartUtcS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_utc_s", value))
    }
    /// Listen.
    public static func hfbeaconsListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Listen to the beacon on.
    public static func hfbeaconsListenKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "listen_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsListenKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "listen_khz", value))
    }
    /// Call sign.
    public static func hfbeaconsCall(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "call", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsCall(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "call", $0) })
    }
    /// Message.
    public static func hfbeaconsMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "message", $0) })
    }
    /// Speed.
    public static func hfbeaconsWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wpm", value))
    }
    /// Tone.
    public static func hfbeaconsPitchHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pitch_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsPitchHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pitch_hz", value))
    }
    /// Long dash.
    public static func hfbeaconsLongDashS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "long_dash_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsLongDashS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "long_dash_s", value))
    }
    /// Interval.
    public static func hfbeaconsIntervalMin(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "interval_min", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsIntervalMin(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "interval_min", value))
    }
    /// Power steps.
    public static func hfbeaconsPowerSteps(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "power_steps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsPowerSteps(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "power_steps", value))
    }
    /// Start in the interval.
    public static func hfbeaconsOffsetS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "offset_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsOffsetS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "offset_s", value))
    }
    /// Transmissions.
    public static func hfbeaconsCount(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "count", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsCount(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "count", value))
    }
    /// The options of the setting "format" of the stack "wavebuoy".
    public enum WavebuoyFormat: Int32 {
        case auto = 0
        case n2fsk = 1
        case n4mfsk = 2
    }
    /// Format.
    public static func wavebuoyFormat(_ handle: SDRKIT_HANDLE) -> Result<WavebuoyFormat, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "format", &v)
        return code == SDRKIT_OK ? .success(WavebuoyFormat(rawValue: v) ?? WavebuoyFormat(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWavebuoyFormat(_ handle: SDRKIT_HANDLE, _ value: WavebuoyFormat) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "format", value.rawValue))
    }
    /// The options of the setting "profile" of the stack "s4285".
    public enum S4285Profile: Int32 {
        case auto = 0
        case n2400Short = 1
        case n1200Short = 2
        case n600Short = 3
        case n300Short = 4
        case n150Short = 5
        case n75Short = 6
        case n3600Uncoded = 7
        case n2400Uncoded = 8
        case n1200Uncoded = 9
        case n2400Long = 10
        case n1200Long = 11
        case n600Long = 12
        case n300Long = 13
        case n150Long = 14
        case n75Long = 15
    }
    /// Rate and interleaver.
    public static func s4285Profile(_ handle: SDRKIT_HANDLE) -> Result<S4285Profile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "profile", &v)
        return code == SDRKIT_OK ? .success(S4285Profile(rawValue: v) ?? S4285Profile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4285Profile(_ handle: SDRKIT_HANDLE, _ value: S4285Profile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "profile", value.rawValue))
    }
    /// The options of the setting "scrambler_start" of the stack "s4285".
    public enum S4285ScramblerStart: Int32 {
        case allOnes = 0
        case one = 1
    }
    /// Scrambler start.
    public static func s4285ScramblerStart(_ handle: SDRKIT_HANDLE) -> Result<S4285ScramblerStart, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "scrambler_start", &v)
        return code == SDRKIT_OK ? .success(S4285ScramblerStart(rawValue: v) ?? S4285ScramblerStart(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4285ScramblerStart(_ handle: SDRKIT_HANDLE, _ value: S4285ScramblerStart) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "scrambler_start", value.rawValue))
    }
    /// Multipath equaliser.
    public static func s4539Equaliser(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "equaliser", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setS4539Equaliser(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "equaliser", value))
    }
    /// Analysis interval.
    public static func rprAnalysisIntervalS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRprAnalysisIntervalS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value))
    }
    /// Audio centre.
    public static func rprAudioCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "audio_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRprAudioCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "audio_centre_hz", value))
    }
    /// The options of the setting "sideband" of the stack "rpr".
    public enum RprSideband: Int32 {
        case upper = 0
        case lower = 1
    }
    /// Sideband.
    public static func rprSideband(_ handle: SDRKIT_HANDLE) -> Result<RprSideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sideband", &v)
        return code == SDRKIT_OK ? .success(RprSideband(rawValue: v) ?? RprSideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRprSideband(_ handle: SDRKIT_HANDLE, _ value: RprSideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sideband", value.rawValue))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum HfdataGeneratorSettings {
    /// Groups.
    public static func crowd36Groups(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_groups", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36Groups(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_groups", value))
    }
    /// Sync high tone.
    public static func crowd36SyncHigh(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_sync_high", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36SyncHigh(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_sync_high", value))
    }
    /// Sync low tone.
    public static func crowd36SyncLow(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_sync_low", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCrowd36SyncLow(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_sync_low", value))
    }
    /// Groups.
    public static func xpaGroups(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_groups", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaGroups(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_groups", value))
    }
    /// The options of the setting "gen_baud" of the stack "xpa".
    public enum XpaBaud: Int32 {
        case n10Bd = 0
        case n20Bd = 1
    }
    /// XPA speed.
    public static func xpaBaud(_ handle: SDRKIT_HANDLE) -> Result<XpaBaud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(XpaBaud(rawValue: v) ?? XpaBaud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaBaud(_ handle: SDRKIT_HANDLE, _ value: XpaBaud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_baud", value.rawValue))
    }
    /// Lower sideband.
    public static func xpaLower(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_lower", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setXpaLower(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_lower", value))
    }
    /// Groups.
    public static func fsk200Groups(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_groups", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Groups(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_groups", value))
    }
    /// Blocks.
    public static func fsk200Blocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Blocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_blocks", value))
    }
    /// The options of the setting "gen_kind" of the stack "fsk200".
    public enum Fsk200Kind: Int32 {
        case f06 = 0
        case ascii = 1
        case f06a = 2
    }
    /// Traffic.
    public static func fsk200Kind(_ handle: SDRKIT_HANDLE) -> Result<Fsk200Kind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &v)
        return code == SDRKIT_OK ? .success(Fsk200Kind(rawValue: v) ?? Fsk200Kind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Kind(_ handle: SDRKIT_HANDLE, _ value: Fsk200Kind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_kind", value.rawValue))
    }
    /// Text.
    public static func fsk200Text(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Text(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
    /// File name.
    public static func fsk200Name(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Name(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_name", $0) })
    }
    /// Bits flipped.
    public static func fsk200BitErrors(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_bit_errors", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200BitErrors(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_bit_errors", $0) })
    }
    /// Centre.
    public static func fsk200CentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200CentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Reversed sense.
    public static func fsk200Reversed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_reversed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFsk200Reversed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_reversed", value))
    }
    /// Position report.
    public static func gwfskPosition(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_position", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskPosition(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_position", value))
    }
    /// Gap.
    public static func gwfskGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value))
    }
    /// Centre.
    public static func gwfskCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Reversed sense.
    public static func gwfskReversed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_reversed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskReversed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_reversed", value))
    }
    /// Bursts.
    public static func gwfskContent(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_content", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGwfskContent(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_content", $0) })
    }
    /// The options of the setting "gen_standard" of the stack "fsk_modems".
    public enum FskModemsStandard: Int32 {
        case bell103Orig = 0
        case bell103Ans = 1
        case v21Ch1 = 2
        case v21Ch2 = 3
        case v23Mode2 = 4
        case bell202 = 5
    }
    /// Standard.
    public static func fskModemsStandard(_ handle: SDRKIT_HANDLE) -> Result<FskModemsStandard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_standard", &v)
        return code == SDRKIT_OK ? .success(FskModemsStandard(rawValue: v) ?? FskModemsStandard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsStandard(_ handle: SDRKIT_HANDLE, _ value: FskModemsStandard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_standard", value.rawValue))
    }
    /// Bytes.
    public static func fskModemsCount(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_count", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsCount(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_count", value))
    }
    /// Stop bits.
    public static func fskModemsStopBits(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_stop_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsStopBits(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_stop_bits", value))
    }
    /// Idle bits between bytes.
    public static func fskModemsGapBits(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_gap_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsGapBits(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_gap_bits", value))
    }
    /// Idle mark.
    public static func fskModemsIdleS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_idle_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsIdleS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_idle_s", value))
    }
    /// Silence before.
    public static func fskModemsLeadS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lead_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsLeadS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lead_s", value))
    }
    /// Silence after.
    public static func fskModemsTailS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tail_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsTailS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tail_s", value))
    }
    /// Lower sideband.
    public static func fskModemsLower(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_lower", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsLower(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_lower", value))
    }
    /// Speed factor.
    public static func fskModemsBaudScale(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud_scale", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskModemsBaudScale(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud_scale", value))
    }
    /// Buzz period.
    public static func uvb76PeriodS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_period_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76PeriodS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_period_s", value))
    }
    /// Buzz length.
    public static func uvb76BuzzS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_buzz_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76BuzzS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_buzz_s", value))
    }
    /// Modulation depth.
    public static func uvb76Depth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76Depth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// Pause at.
    public static func uvb76BreakAtS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_break_at_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76BreakAtS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_break_at_s", value))
    }
    /// Pause.
    public static func uvb76BreakS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_break_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76BreakS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_break_s", value))
    }
    /// Voice at.
    public static func uvb76VoiceAtS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_voice_at_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76VoiceAtS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_voice_at_s", value))
    }
    /// Voice length.
    public static func uvb76VoiceS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_voice_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUvb76VoiceS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_voice_s", value))
    }
    /// Bit rate.
    public static func vlfMskRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rate", value))
    }
    /// Carrier offset.
    public static func vlfMskFcHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fc_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskFcHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fc_hz", value))
    }
    /// On from.
    public static func vlfMskFromS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_from_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskFromS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_from_s", value))
    }
    /// Off at.
    public static func vlfMskToS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_to_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setVlfMskToS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_to_s", value))
    }
    /// Centre.
    public static func efrCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEfrCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// The options of the setting "gen_baud" of the stack "gtor".
    public enum GtorBaud: Int32 {
        case auto = 0
        case n100Bd = 1
        case n200Bd = 2
        case n300Bd = 3
    }
    /// Speed.
    public static func gtorBaud(_ handle: SDRKIT_HANDLE) -> Result<GtorBaud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(GtorBaud(rawValue: v) ?? GtorBaud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGtorBaud(_ handle: SDRKIT_HANDLE, _ value: GtorBaud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_baud", value.rawValue))
    }
    /// Golay form.
    public static func gtorGolay(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_golay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGtorGolay(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_golay", value))
    }
    /// Shift.
    public static func gtorShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGtorShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value))
    }
    /// Centre.
    public static func gtorCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGtorCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Speed.
    public static func arqeBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud", value))
    }
    /// Shift.
    public static func arqeShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value))
    }
    /// The options of the setting "gen_cycle" of the stack "arqe".
    public enum ArqeCycle: Int32 {
        case auto = 0
        case none = 1
        case every4thWord = 2
        case every8thWord = 3
    }
    /// Inversion cycle.
    public static func arqeCycle(_ handle: SDRKIT_HANDLE) -> Result<ArqeCycle, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_cycle", &v)
        return code == SDRKIT_OK ? .success(ArqeCycle(rawValue: v) ?? ArqeCycle(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeCycle(_ handle: SDRKIT_HANDLE, _ value: ArqeCycle) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_cycle", value.rawValue))
    }
    /// The options of the setting "gen_channels" of the stack "arqe".
    public enum ArqeChannels: Int32 {
        case auto = 0
        case oneChannel = 1
        case twoChannels = 2
        case fourChannels = 3
    }
    /// Channels.
    public static func arqeChannels(_ handle: SDRKIT_HANDLE) -> Result<ArqeChannels, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_channels", &v)
        return code == SDRKIT_OK ? .success(ArqeChannels(rawValue: v) ?? ArqeChannels(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeChannels(_ handle: SDRKIT_HANDLE, _ value: ArqeChannels) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_channels", value.rawValue))
    }
    /// The options of the setting "gen_word_code" of the stack "arqe".
    public enum ArqeWordCode: Int32 {
        case parity = 0
        case f342 = 1
    }
    /// Word code.
    public static func arqeWordCode(_ handle: SDRKIT_HANDLE) -> Result<ArqeWordCode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_word_code", &v)
        return code == SDRKIT_OK ? .success(ArqeWordCode(rawValue: v) ?? ArqeWordCode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeWordCode(_ handle: SDRKIT_HANDLE, _ value: ArqeWordCode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_word_code", value.rawValue))
    }
    /// Centre.
    public static func arqeCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArqeCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Speed.
    public static func autospecBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud", value))
    }
    /// Shift.
    public static func autospecShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value))
    }
    /// The options of the setting "gen_depth" of the stack "autospec".
    public enum AutospecDepth: Int32 {
        case auto = 0
        case n11 = 1
        case n21 = 2
        case n51 = 3
    }
    /// Spread depth.
    public static func autospecDepth(_ handle: SDRKIT_HANDLE) -> Result<AutospecDepth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(AutospecDepth(rawValue: v) ?? AutospecDepth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecDepth(_ handle: SDRKIT_HANDLE, _ value: AutospecDepth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_depth", value.rawValue))
    }
    /// Centre.
    public static func autospecCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAutospecCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Tone A.
    public static func piccoloBaseHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_base_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPiccoloBaseHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_base_hz", value))
    }
    /// The options of the setting "gen_tone_ms" of the stack "coquelet".
    public enum CoqueletToneMs: Int32 {
        case auto = 0
        case n375 = 1
        case n50Ms = 2
        case n75Ms = 3
    }
    /// Tone length.
    public static func coqueletToneMs(_ handle: SDRKIT_HANDLE) -> Result<CoqueletToneMs, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_tone_ms", &v)
        return code == SDRKIT_OK ? .success(CoqueletToneMs(rawValue: v) ?? CoqueletToneMs(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCoqueletToneMs(_ handle: SDRKIT_HANDLE, _ value: CoqueletToneMs) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_tone_ms", value.rawValue))
    }
    /// Tone 1.
    public static func coqueletBaseHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_base_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCoqueletBaseHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_base_hz", value))
    }
    /// The options of the setting "gen_set" of the stack "ale3g".
    public enum Ale3gSet: Int32 {
        case exchange = 0
        case bw0 = 1
    }
    /// Bursts.
    public static func ale3gSet(_ handle: SDRKIT_HANDLE) -> Result<Ale3gSet, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_set", &v)
        return code == SDRKIT_OK ? .success(Ale3gSet(rawValue: v) ?? Ale3gSet(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAle3gSet(_ handle: SDRKIT_HANDLE, _ value: Ale3gSet) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_set", value.rawValue))
    }
    /// Multi-channel.
    public static func s4481Multi(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_multi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setS4481Multi(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_multi", value))
    }
    /// The options of the setting "gen_baud" of the stack "s4481".
    public enum S4481Baud: Int32 {
        case auto = 0
        case n4545 = 1
        case n50Bd = 2
        case n75Bd = 3
        case n100Bd = 4
    }
    /// Speed.
    public static func s4481Baud(_ handle: SDRKIT_HANDLE) -> Result<S4481Baud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(S4481Baud(rawValue: v) ?? S4481Baud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4481Baud(_ handle: SDRKIT_HANDLE, _ value: S4481Baud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_baud", value.rawValue))
    }
    /// Single-channel mark.
    public static func s4481SingleMarkHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_single_mark_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setS4481SingleMarkHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_single_mark_hz", value))
    }
    /// The options of the setting "gen_rate" of the stack "link11".
    public enum Link11Rate: Int32 {
        case auto = 0
        case fast = 1
        case slow = 2
    }
    /// Frame rate.
    public static func link11Rate(_ handle: SDRKIT_HANDLE) -> Result<Link11Rate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(Link11Rate(rawValue: v) ?? Link11Rate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLink11Rate(_ handle: SDRKIT_HANDLE, _ value: Link11Rate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
    /// Phase step offset.
    public static func link11OffsetDeg(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_offset_deg", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLink11OffsetDeg(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_offset_deg", value))
    }
    /// Preamble frames.
    public static func link11PreambleFrames(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_preamble_frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLink11PreambleFrames(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_preamble_frames", value))
    }
    /// The options of the setting "gen_variant" of the stack "cis_hf".
    public enum CisHfVariant: Int32 {
        case n3650 = 0
        case n50100 = 1
    }
    /// BEE variant.
    public static func cisHfVariant(_ handle: SDRKIT_HANDLE) -> Result<CisHfVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &v)
        return code == SDRKIT_OK ? .success(CisHfVariant(rawValue: v) ?? CisHfVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfVariant(_ handle: SDRKIT_HANDLE, _ value: CisHfVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_variant", value.rawValue))
    }
    /// BEE 1 on the lower tone.
    public static func cisHfLowerOne(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_lower_one", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfLowerOne(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_lower_one", value))
    }
    /// Centre.
    public static func cisHfCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// CIS-14 shift.
    public static func cisHfShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value))
    }
    /// The options of the setting "gen_order" of the stack "cis_hf".
    public enum CisHfOrder: Int32 {
        case dbpsk = 0
        case dqpsk = 1
    }
    /// CIS-12 order.
    public static func cisHfOrder(_ handle: SDRKIT_HANDLE) -> Result<CisHfOrder, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_order", &v)
        return code == SDRKIT_OK ? .success(CisHfOrder(rawValue: v) ?? CisHfOrder(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfOrder(_ handle: SDRKIT_HANDLE, _ value: CisHfOrder) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_order", value.rawValue))
    }
    /// The options of the setting "gen_rate" of the stack "cis_hf".
    public enum CisHfRate: Int32 {
        case n10Bd = 0
        case n20Bd = 1
        case n40Bd = 2
    }
    /// CIS-36 tone rate.
    public static func cisHfRate(_ handle: SDRKIT_HANDLE) -> Result<CisHfRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(CisHfRate(rawValue: v) ?? CisHfRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfRate(_ handle: SDRKIT_HANDLE, _ value: CisHfRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
    /// The options of the setting "gen_ofdm" of the stack "cis_hf".
    public enum CisHfOfdm: Int32 {
        case cis4540 = 0
        case cis4533 = 1
        case cis60 = 2
        case cis93 = 3
    }
    /// OFDM signal.
    public static func cisHfOfdm(_ handle: SDRKIT_HANDLE) -> Result<CisHfOfdm, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_ofdm", &v)
        return code == SDRKIT_OK ? .success(CisHfOfdm(rawValue: v) ?? CisHfOfdm(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCisHfOfdm(_ handle: SDRKIT_HANDLE, _ value: CisHfOfdm) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_ofdm", value.rawValue))
    }
    /// The options of the setting "gen_calls" of the stack "hfid".
    public enum HfidCalls: Int32 {
        case testCalls = 0
        case sounding = 1
    }
    /// Calls.
    public static func hfidCalls(_ handle: SDRKIT_HANDLE) -> Result<HfidCalls, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_calls", &v)
        return code == SDRKIT_OK ? .success(HfidCalls(rawValue: v) ?? HfidCalls(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfidCalls(_ handle: SDRKIT_HANDLE, _ value: HfidCalls) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_calls", value.rawValue))
    }
    /// Channel bit errors.
    public static func hfidBer(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ber", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfidBer(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ber", value))
    }
    /// The options of the setting "gen_calls" of the stack "ale".
    public enum AleCalls: Int32 {
        case testCalls = 0
        case sounding = 1
    }
    /// Calls.
    public static func aleCalls(_ handle: SDRKIT_HANDLE) -> Result<AleCalls, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_calls", &v)
        return code == SDRKIT_OK ? .success(AleCalls(rawValue: v) ?? AleCalls(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAleCalls(_ handle: SDRKIT_HANDLE, _ value: AleCalls) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_calls", value.rawValue))
    }
    /// Channel bit errors.
    public static func aleBer(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ber", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAleBer(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ber", value))
    }
    /// The options of the setting "gen_signal" of the stack "hfradar".
    public enum HfradarSignal: Int32 {
        case radars = 0
        case nothing = 1
        case radarsWithId = 2
        case junk = 3
    }
    /// Signal.
    public static func hfradarSignal(_ handle: SDRKIT_HANDLE) -> Result<HfradarSignal, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &v)
        return code == SDRKIT_OK ? .success(HfradarSignal(rawValue: v) ?? HfradarSignal(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfradarSignal(_ handle: SDRKIT_HANDLE, _ value: HfradarSignal) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_signal", value.rawValue))
    }
    /// Speed level.
    public static func pactor3Level(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor3Level(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_level", value))
    }
    /// The options of the setting "gen_constellation" of the stack "pactor4".
    public enum Pactor4Constellation: Int32 {
        case n2Points = 0
        case n4Points = 1
        case n8Points = 2
        case n16Points = 3
    }
    /// Constellation.
    public static func pactor4Constellation(_ handle: SDRKIT_HANDLE) -> Result<Pactor4Constellation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &v)
        return code == SDRKIT_OK ? .success(Pactor4Constellation(rawValue: v) ?? Pactor4Constellation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor4Constellation(_ handle: SDRKIT_HANDLE, _ value: Pactor4Constellation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_constellation", value.rawValue))
    }
    /// The options of the setting "gen_script" of the stack "pactor1".
    public enum Pactor1Script: Int32 {
        case arq = 0
        case fec = 1
    }
    /// Script.
    public static func pactor1Script(_ handle: SDRKIT_HANDLE) -> Result<Pactor1Script, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_script", &v)
        return code == SDRKIT_OK ? .success(Pactor1Script(rawValue: v) ?? Pactor1Script(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPactor1Script(_ handle: SDRKIT_HANDLE, _ value: Pactor1Script) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_script", value.rawValue))
    }
    /// The options of the setting "gen_signal" of the stack "hfbeacons".
    public enum HfbeaconsSignal: Int32 {
        case beacons = 0
        case nothing = 1
    }
    /// Signal.
    public static func hfbeaconsSignal(_ handle: SDRKIT_HANDLE) -> Result<HfbeaconsSignal, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &v)
        return code == SDRKIT_OK ? .success(HfbeaconsSignal(rawValue: v) ?? HfbeaconsSignal(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsSignal(_ handle: SDRKIT_HANDLE, _ value: HfbeaconsSignal) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_signal", value.rawValue))
    }
    /// Centre (0 = 3.65 MHz).
    public static func hfbeaconsCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfbeaconsCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// The options of the setting "gen_signal" of the stack "wavebuoy".
    public enum WavebuoySignal: Int32 {
        case n2fsk = 0
        case n4fsk = 1
        case nothing = 2
        case carrier = 3
    }
    /// Signal.
    public static func wavebuoySignal(_ handle: SDRKIT_HANDLE) -> Result<WavebuoySignal, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &v)
        return code == SDRKIT_OK ? .success(WavebuoySignal(rawValue: v) ?? WavebuoySignal(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWavebuoySignal(_ handle: SDRKIT_HANDLE, _ value: WavebuoySignal) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_signal", value.rawValue))
    }
    /// Inverted tones.
    public static func wavebuoyInvertedTones(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_inverted_tones", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWavebuoyInvertedTones(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_inverted_tones", value))
    }
    /// WR-SG.
    public static func wavebuoyWrsg(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_wrsg", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWavebuoyWrsg(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_wrsg", value))
    }
    /// The options of the setting "gen_bps" of the stack "ms110".
    public enum Ms110Bps: Int32 {
        case n75BitS = 0
        case n150BitS = 1
        case n300BitS = 2
        case n600BitS = 3
        case n1200BitS = 4
        case n2400BitS = 5
        case n4800BitS = 6
    }
    /// Data rate.
    public static func ms110Bps(_ handle: SDRKIT_HANDLE) -> Result<Ms110Bps, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bps", &v)
        return code == SDRKIT_OK ? .success(Ms110Bps(rawValue: v) ?? Ms110Bps(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMs110Bps(_ handle: SDRKIT_HANDLE, _ value: Ms110Bps) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bps", value.rawValue))
    }
    /// The options of the setting "gen_interleaver" of the stack "ms110".
    public enum Ms110Interleaver: Int32 {
        case short = 0
        case long = 1
    }
    /// Interleaver.
    public static func ms110Interleaver(_ handle: SDRKIT_HANDLE) -> Result<Ms110Interleaver, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_interleaver", &v)
        return code == SDRKIT_OK ? .success(Ms110Interleaver(rawValue: v) ?? Ms110Interleaver(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMs110Interleaver(_ handle: SDRKIT_HANDLE, _ value: Ms110Interleaver) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_interleaver", value.rawValue))
    }
    /// The options of the setting "gen_profile" of the stack "s4285".
    public enum S4285Profile: Int32 {
        case auto = 0
        case n2400Short = 1
        case n1200Short = 2
        case n600Short = 3
        case n300Short = 4
        case n150Short = 5
        case n75Short = 6
        case n3600Uncoded = 7
        case n2400Uncoded = 8
        case n1200Uncoded = 9
        case n2400Long = 10
        case n1200Long = 11
        case n600Long = 12
        case n300Long = 13
        case n150Long = 14
        case n75Long = 15
    }
    /// Profile.
    public static func s4285Profile(_ handle: SDRKIT_HANDLE) -> Result<S4285Profile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_profile", &v)
        return code == SDRKIT_OK ? .success(S4285Profile(rawValue: v) ?? S4285Profile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4285Profile(_ handle: SDRKIT_HANDLE, _ value: S4285Profile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_profile", value.rawValue))
    }
    /// The options of the setting "gen_bps" of the stack "s4539".
    public enum S4539Bps: Int32 {
        case n3200BitS = 0
        case n4800BitS = 1
        case n6400BitS = 2
        case n8000BitS = 3
        case n9600BitS = 4
        case n12800BitS = 5
    }
    /// Data rate.
    public static func s4539Bps(_ handle: SDRKIT_HANDLE) -> Result<S4539Bps, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bps", &v)
        return code == SDRKIT_OK ? .success(S4539Bps(rawValue: v) ?? S4539Bps(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4539Bps(_ handle: SDRKIT_HANDLE, _ value: S4539Bps) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bps", value.rawValue))
    }
    /// The options of the setting "gen_frames" of the stack "s4539".
    public enum S4539Frames: Int32 {
        case ultraShort = 0
        case veryShort = 1
        case short = 2
        case medium = 3
        case long = 4
        case veryLong = 5
    }
    /// Interleaver.
    public static func s4539Frames(_ handle: SDRKIT_HANDLE) -> Result<S4539Frames, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_frames", &v)
        return code == SDRKIT_OK ? .success(S4539Frames(rawValue: v) ?? S4539Frames(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setS4539Frames(_ handle: SDRKIT_HANDLE, _ value: S4539Frames) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_frames", value.rawValue))
    }
    /// AGC blocks.
    public static func s4539AgcBlocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_agc_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setS4539AgcBlocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_agc_blocks", value))
    }
    /// End of message.
    public static func s4539Eom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_eom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setS4539Eom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_eom", value))
    }
    /// The options of the setting "gen_speed" of the stack "rpr".
    public enum RprSpeed: Int32 {
        case level1 = 0
        case level2 = 1
    }
    /// Speed level.
    public static func rprSpeed(_ handle: SDRKIT_HANDLE) -> Result<RprSpeed, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_speed", &v)
        return code == SDRKIT_OK ? .success(RprSpeed(rawValue: v) ?? RprSpeed(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRprSpeed(_ handle: SDRKIT_HANDLE, _ value: RprSpeed) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_speed", value.rawValue))
    }
    /// Gap between bursts.
    public static func rprGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRprGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value))
    }
}

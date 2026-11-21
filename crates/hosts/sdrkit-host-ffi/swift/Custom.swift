// sdrkit: the typed interface of the stack family "custom", version 1.2. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum CustomInterface {
    public static let major = 1
    public static let minor = 2
}

/// The result fields of `slotted_voice`. A value of the signal has no value while no receiver is locked.
public struct SlottedVoiceFields {
    /// A receiver is locked.
    public var locked: Bool
    /// The carrier offset.
    public var offsetHz: Double?
    /// The SNR.
    public var snrDb: Double?
    /// The frame version received, 3 or 4.
    public var frameVersion: Int64?
    /// The voice bit rate.
    public var voiceKbps: Double?
    /// The symbol rate.
    public var symbolRateHz: Double?
    /// Words or slots that decoded, of the last receiver that was locked.
    public var framesOk: Int64?
    /// Words or slots that did not decode, of the last receiver that was locked.
    public var framesBad: Int64?
    /// The station identity of the sweep, while a receiver reads it.
    public var sweepId: String?
    /// The sweep level relative to the voice.
    public var sweepDb: Double?
    /// The callsign of the newest beacon.
    public var callsign: String?
    /// The note on the source of the stack.
    public var note: String

    init(_ c: sdrkit_slotted_voice_fields) {
        locked = c.locked
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        frameVersion = c.has_frame_version ? c.frame_version : nil
        voiceKbps = c.has_voice_kbps ? c.voice_kbps : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        framesOk = c.has_frames_ok ? c.frames_ok : nil
        framesBad = c.has_frames_bad ? c.frames_bad : nil
        sweepId = c.has_sweep_id ? sdrkitText(c.sweep_id) : nil
        sweepDb = c.has_sweep_db ? c.sweep_db : nil
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceFields, SDRKitCode> {
        var c = sdrkit_slotted_voice_fields()
        let code = sdrkit_slotted_voice_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SlottedVoiceFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct CustomDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_custom_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CustomDecoderFields, SDRKitCode> {
        var c = sdrkit_custom_decoder_fields()
        let code = sdrkit_custom_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CustomDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the stations heard in beacons. The row key is the callsign.
public struct SlottedVoiceStationRow {
    /// The row key.
    public var key: String
    /// The callsign.
    public var callsign: String
    /// The grid locator.
    public var locator: String
    /// The position.
    public var position: SDRKitPosition
    /// The beacon power value.
    public var powerDbm: Double
    /// The link quality value.
    public var link: Int64
    /// The beacon text, or the text of the last full beacon of the station.
    public var text: String
    /// When the station was last heard.
    public var lastHeard: Double

    init(_ c: sdrkit_slotted_voice_station_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        locator = sdrkitText(c.locator)
        position = SDRKitPosition(c.position)
        powerDbm = c.power_dbm
        link = c.link
        text = sdrkitText(c.text)
        lastHeard = c.last_heard
    }
}

/// The result fields of `qam_video`. A value of the last frame has no value before the first frame.
public struct QamVideoFields {
    /// A frame header passed in the last 2 s.
    public var locked: Bool
    /// The profile of the last frame.
    public var profile: String?
    /// The Es/N0 of the last frame.
    public var esn0Db: Double?
    /// The carrier offset of the last frame.
    public var offsetHz: Double?
    /// Frames with all four blocks good.
    public var framesOk: Int64
    /// Frames with a failed block.
    public var framesBad: Int64
    /// Blocks that failed.
    public var blocksBad: Int64
    /// Groups restored by the erasure code.
    public var groupsRestored: Int64
    /// Groups lost.
    public var groupsLost: Int64
    /// Packets received.
    public var packets: Int64
    /// The callsign of the station identification.
    public var callsign: String?
    /// The symbol rate of the listen profile.
    public var symbolRateHz: Double
    /// The note on the source of the stack.
    public var note: String

    init(_ c: sdrkit_qam_video_fields) {
        locked = c.locked
        profile = c.has_profile ? sdrkitText(c.profile) : nil
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        blocksBad = c.blocks_bad
        groupsRestored = c.groups_restored
        groupsLost = c.groups_lost
        packets = c.packets
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        symbolRateHz = c.symbol_rate_hz
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<QamVideoFields, SDRKitCode> {
        var c = sdrkit_qam_video_fields()
        let code = sdrkit_qam_video_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(QamVideoFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the streams heard. The row key is the packet type and the stream id.
public struct QamVideoStreamRow {
    /// The row key.
    public var key: String
    /// The stream id.
    public var stream: String
    /// The packet type: video, audio, text, picture, data or other.
    public var type: String
    /// Packets of the stream.
    public var packets: Int64

    init(_ c: sdrkit_qam_video_stream_row) {
        key = sdrkitText(c.key)
        stream = sdrkitText(c.stream)
        type = sdrkitText(c.type)
        packets = c.packets
    }
}

/// The result fields of `picture_burst`. A value of the last burst has no value before the first decoded burst.
public struct PictureBurstFields {
    /// The header of the last burst decoded.
    public var headerOk: Bool?
    /// The profile of the last burst.
    public var profile: Int64?
    /// Layers of the last burst decoded.
    public var layers: Int64?
    /// Layers of the last burst sent.
    public var layersSent: Int64?
    /// The SNR of the last burst.
    public var snrDb: Double?
    /// The carrier offset of the last burst.
    public var offsetHz: Double?
    /// The clock error of the last burst, if measured.
    public var clockPpm: Double?
    /// The callsign of the last burst.
    public var callsign: String?
    /// Bursts seen.
    public var bursts: Int64
    /// Bursts decoded.
    public var burstsDecoded: Int64
    /// A burst is being received, or one was decoded.
    public var locked: Bool
    /// The note on the source of the stack.
    public var note: String

    init(_ c: sdrkit_picture_burst_fields) {
        headerOk = c.has_header_ok ? c.header_ok : nil
        profile = c.has_profile ? c.profile : nil
        layers = c.has_layers ? c.layers : nil
        layersSent = c.has_layers_sent ? c.layers_sent : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        bursts = c.bursts
        burstsDecoded = c.bursts_decoded
        locked = c.locked
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PictureBurstFields, SDRKitCode> {
        var c = sdrkit_picture_burst_fields()
        let code = sdrkit_picture_burst_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PictureBurstFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the pictures received. The row key is the number of the decoded burst.
public struct PictureBurstPictureRow {
    /// The row key.
    public var key: String
    /// The callsign.
    public var callsign: String
    /// The picture size, "width x height".
    public var sizeField: String
    /// Layers decoded.
    public var layers: Int64
    /// When the burst was decoded.
    public var time: Double

    init(_ c: sdrkit_picture_burst_picture_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        sizeField = sdrkitText(c.size_)
        layers = c.layers
        time = c.time
    }
}

/// The result fields of `arq_packet`. A value of the last burst has no value before the first burst.
public struct ArqPacketFields {
    /// Blocks whose CRC passed.
    public var blocksGood: Int64
    /// Blocks whose CRC failed.
    public var blocksBad: Int64
    /// The profile of the last burst.
    public var profile: Int64?
    /// The Es/N0 of the last burst.
    public var snrDb: Double?
    /// The carrier offset of the last burst.
    public var offsetHz: Double?
    /// The callsign of the last burst.
    public var callsign: String?
    /// Bursts whose header was read.
    public var bursts: Int64
    /// Messages listed.
    public var messages: Int64
    /// Messages heard again within 120 s.
    public var repeats: Int64
    /// Acknowledgement bursts heard.
    public var acksHeard: Int64
    /// A burst header was read in the last 5 s.
    public var locked: Bool
    /// The note on the source of the stack.
    public var note: String
    /// The sequence number of the last burst.
    public var sequence: Int64?

    init(_ c: sdrkit_arq_packet_fields) {
        blocksGood = c.blocks_good
        blocksBad = c.blocks_bad
        profile = c.has_profile ? c.profile : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        bursts = c.bursts
        messages = c.messages
        repeats = c.repeats
        acksHeard = c.acks_heard
        locked = c.locked
        note = sdrkitText(c.note)
        sequence = c.has_sequence ? c.sequence : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ArqPacketFields, SDRKitCode> {
        var c = sdrkit_arq_packet_fields()
        let code = sdrkit_arq_packet_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ArqPacketFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the messages received. The row key is a running number.
public struct ArqPacketMessageRow {
    /// The row key.
    public var key: String
    /// The sending station.
    public var callsign: String
    /// The sequence number of the last burst of the message.
    public var sequence: Int64
    /// The message length.
    public var length: Int64
    /// The message as text.
    public var text: String
    /// When the message was completed.
    public var time: Double

    init(_ c: sdrkit_arq_packet_message_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        sequence = c.sequence
        length = c.length
        text = sdrkitText(c.text)
        time = c.time
    }
}

/// The result fields of `file_transfer`. A value of the last frame has no value before the first frame, and a value of the transfer none while no transfer is announced.
public struct FileTransferFields {
    /// The session id of the active transfer.
    public var session: Int64?
    /// Blocks whose CRC passed.
    public var blocksGood: Int64
    /// Blocks whose CRC failed.
    public var blocksBad: Int64
    /// Frames whose header was read.
    public var frames: Int64
    /// Bytes of the active transfer received.
    public var progress: Double?
    /// The last file passed its hash.
    public var hashOk: Bool?
    /// The profile of the last frame.
    public var profile: Int64?
    /// The width of the last frame.
    public var width: String?
    /// The Es/N0 of the last frame.
    public var snrDb: Double?
    /// The carrier offset of the last frame.
    public var offsetHz: Double?
    /// The callsign of the last frame.
    public var callsign: String?
    /// Files received.
    public var files: Int64
    /// Bytes held from an earlier session when the last transfer was announced.
    public var heldBytes: Int64
    /// Acknowledgement frames heard.
    public var acksHeard: Int64
    /// A frame header was read in the last 5 s.
    public var locked: Bool
    /// The note on the source of the stack.
    public var note: String

    init(_ c: sdrkit_file_transfer_fields) {
        session = c.has_session ? c.session : nil
        blocksGood = c.blocks_good
        blocksBad = c.blocks_bad
        frames = c.frames
        progress = c.has_progress ? c.progress : nil
        hashOk = c.has_hash_ok ? c.hash_ok : nil
        profile = c.has_profile ? c.profile : nil
        width = c.has_width ? sdrkitText(c.width) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        files = c.files
        heldBytes = c.held_bytes
        acksHeard = c.acks_heard
        locked = c.locked
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FileTransferFields, SDRKitCode> {
        var c = sdrkit_file_transfer_fields()
        let code = sdrkit_file_transfer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FileTransferFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the files heard. The row key is the session id and the start of the hash.
public struct FileTransferFileRow {
    /// The row key.
    public var key: String
    /// The file name.
    public var name: String
    /// The size.
    public var sizeField: Int64
    /// The hash, hex.
    public var hash: String
    /// The state: receiving, verified, or the hash failed.
    public var state: String
    /// The sending station.
    public var callsign: String
    /// When the state was reached.
    public var time: Double

    init(_ c: sdrkit_file_transfer_file_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        sizeField = c.size_
        hash = sdrkitText(c.hash)
        state = sdrkitText(c.state)
        callsign = sdrkitText(c.callsign)
        time = c.time
    }
}

/// The result fields of `hf_chat`. A value of the last burst has no value before the first burst.
public struct HfChatFields {
    /// The profile of the last burst.
    public var profile: Int64?
    /// The SNR of the last burst.
    public var snrDb: Double?
    /// The carrier offset of the last burst.
    public var offsetHz: Double?
    /// The clock error of the last burst against the slot grid.
    public var clockErrorS: Double?
    /// The sending station of the last burst; empty if the burst names none.
    public var callsign: String?
    /// Bursts decoded.
    public var bursts: Int64
    /// Blocks whose code word and CRC held.
    public var blocksGood: Int64
    /// Blocks that did not decode.
    public var blocksBad: Int64
    /// Messages completed.
    public var messages: Int64
    /// Beacons heard.
    public var beacons: Int64
    /// Acknowledgements heard.
    public var acksHeard: Int64
    /// Voice blocks heard.
    public var voiceBlocks: Int64
    /// A burst was decoded in the last 30 s.
    public var locked: Bool
    /// The note on the source of the stack.
    public var note: String
    /// The message id of the last burst; 0 if it carries no message part.
    public var messageId: Int64?

    init(_ c: sdrkit_hf_chat_fields) {
        profile = c.has_profile ? c.profile : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockErrorS = c.has_clock_error_s ? c.clock_error_s : nil
        callsign = c.has_callsign ? sdrkitText(c.callsign) : nil
        bursts = c.bursts
        blocksGood = c.blocks_good
        blocksBad = c.blocks_bad
        messages = c.messages
        beacons = c.beacons
        acksHeard = c.acks_heard
        voiceBlocks = c.voice_blocks
        locked = c.locked
        note = sdrkitText(c.note)
        messageId = c.has_message_id ? c.message_id : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HfChatFields, SDRKitCode> {
        var c = sdrkit_hf_chat_fields()
        let code = sdrkit_hf_chat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HfChatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the messages received. The row key is a running number.
public struct HfChatMessageRow {
    /// The row key.
    public var key: String
    /// The sending station; empty for a data message.
    public var callsign: String
    /// The recipient; empty for a data message.
    public var to: String
    /// The text.
    public var text: String
    /// When the message was completed, UTC.
    public var time: Double

    init(_ c: sdrkit_hf_chat_message_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        to = sdrkitText(c.to)
        text = sdrkitText(c.text)
        time = c.time
    }
}

/// A row of the beacons received. The row key is a running number.
public struct HfChatBeaconRow {
    /// The row key.
    public var key: String
    /// The station.
    public var callsign: String
    /// The locator.
    public var locator: String
    /// The position.
    public var position: SDRKitPosition
    /// The power.
    public var powerDbm: Double
    /// The text of a full beacon; empty for a short one.
    public var text: String
    /// When the beacon was completed, UTC.
    public var time: Double

    init(_ c: sdrkit_hf_chat_beacon_row) {
        key = sdrkitText(c.key)
        callsign = sdrkitText(c.callsign)
        locator = sdrkitText(c.locator)
        position = SDRKitPosition(c.position)
        powerDbm = c.power_dbm
        text = sdrkitText(c.text)
        time = c.time
    }
}

/// The fields of a station beacon.
public struct SlottedVoiceStationEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The callsign.
    public var callsign: String
    /// The grid locator.
    public var locator: String
    /// The position.
    public var position: SDRKitPosition
    /// The beacon power value.
    public var powerDbm: Double
    /// The beacon text.
    public var text: String

    init(_ c: sdrkit_slotted_voice_station_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        locator = sdrkitText(c.locator)
        position = SDRKitPosition(c.position)
        powerDbm = c.power_dbm
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SlottedVoiceStationEvent, SDRKitCode> {
        var c = sdrkit_slotted_voice_station_event()
        let code = sdrkit_slotted_voice_station_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SlottedVoiceStationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a transmission that started, finished or was refused.
public struct SlottedVoiceTxEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the transmission did: `started`, `finished` or `refused`.
    public var state: String
    /// Why the transmission was refused or ended early; empty for none.
    public var reason: String

    init(_ c: sdrkit_slotted_voice_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SlottedVoiceTxEvent, SDRKitCode> {
        var c = sdrkit_slotted_voice_tx_event()
        let code = sdrkit_slotted_voice_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SlottedVoiceTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a transmission that started, finished or was refused.
public struct QamVideoTxEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the transmission did: `started`, `finished` or `refused`.
    public var state: String
    /// Why the transmission was refused or ended early; empty for none.
    public var reason: String

    init(_ c: sdrkit_qam_video_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<QamVideoTxEvent, SDRKitCode> {
        var c = sdrkit_qam_video_tx_event()
        let code = sdrkit_qam_video_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(QamVideoTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a decoded burst.
public struct PictureBurstEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The callsign.
    public var callsign: String
    /// The picture size, "width x height".
    public var sizeField: String
    /// Layers decoded.
    public var layers: Int64

    init(_ c: sdrkit_picture_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        sizeField = sdrkitText(c.size_)
        layers = c.layers
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PictureBurstEvent, SDRKitCode> {
        var c = sdrkit_picture_burst_event()
        let code = sdrkit_picture_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PictureBurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a transmission that started, finished or was refused.
public struct PictureBurstPictureTxEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the transmission did: `started`, `finished` or `refused`.
    public var state: String
    /// Why the transmission was refused or ended early; empty for none.
    public var reason: String

    init(_ c: sdrkit_picture_burst_picture_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PictureBurstPictureTxEvent, SDRKitCode> {
        var c = sdrkit_picture_burst_picture_tx_event()
        let code = sdrkit_picture_burst_picture_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PictureBurstPictureTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a completed message.
public struct ArqPacketMessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sending station.
    public var callsign: String
    /// The sequence number of the last burst of the message.
    public var sequence: Int64
    /// The message length.
    public var length: Int64
    /// The message as text.
    public var text: String

    init(_ c: sdrkit_arq_packet_message_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        sequence = c.sequence
        length = c.length
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArqPacketMessageEvent, SDRKitCode> {
        var c = sdrkit_arq_packet_message_event()
        let code = sdrkit_arq_packet_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArqPacketMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the acknowledgement that the receiver would send for a data burst.
public struct ArqPacketAckOutEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sending station of the burst.
    public var callsign: String
    /// The sequence number of the burst.
    public var sequence: Int64
    /// Blocks of the burst.
    public var blocks: Int64
    /// Blocks acknowledged.
    public var blocksAcked: Int64
    /// The Es/N0 of the burst.
    public var snrDb: Double

    init(_ c: sdrkit_arq_packet_ack_out_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        sequence = c.sequence
        blocks = c.blocks
        blocksAcked = c.blocks_acked
        snrDb = c.snr_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArqPacketAckOutEvent, SDRKitCode> {
        var c = sdrkit_arq_packet_ack_out_event()
        let code = sdrkit_arq_packet_ack_out_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArqPacketAckOutEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an acknowledgement burst heard.
public struct ArqPacketAckInEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The acknowledging station.
    public var callsign: String
    /// The sequence number that it acknowledges.
    public var sequence: Int64
    /// Blocks acknowledged.
    public var blocksAcked: Int64
    /// The Es/N0 that it reports.
    public var snrDb: Double

    init(_ c: sdrkit_arq_packet_ack_in_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        sequence = c.sequence
        blocksAcked = c.blocks_acked
        snrDb = c.snr_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArqPacketAckInEvent, SDRKitCode> {
        var c = sdrkit_arq_packet_ack_in_event()
        let code = sdrkit_arq_packet_ack_in_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArqPacketAckInEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a transmission that started, finished or was refused.
public struct ArqPacketTxEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the transmission did: `started`, `finished` or `refused`.
    public var state: String
    /// Why the transmission was refused or ended early; empty for none.
    public var reason: String

    init(_ c: sdrkit_arq_packet_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ArqPacketTxEvent, SDRKitCode> {
        var c = sdrkit_arq_packet_tx_event()
        let code = sdrkit_arq_packet_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ArqPacketTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a file that passed every check and was saved.
public struct FileTransferFileReceivedEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The file name.
    public var name: String
    /// The size.
    public var sizeField: Int64
    /// The hash, hex.
    public var hash: String
    /// Where the file was saved; empty if it was not saved.
    public var path: String
    /// The sending station.
    public var callsign: String

    init(_ c: sdrkit_file_transfer_file_received_event) {
        time = c.time
        freqHz = c.freq_hz
        name = sdrkitText(c.name)
        sizeField = c.size_
        hash = sdrkitText(c.hash)
        path = sdrkitText(c.path)
        callsign = sdrkitText(c.callsign)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FileTransferFileReceivedEvent, SDRKitCode> {
        var c = sdrkit_file_transfer_file_received_event()
        let code = sdrkit_file_transfer_file_received_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FileTransferFileReceivedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an announced transfer.
public struct FileTransferFileOpenEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The session id.
    public var session: Int64
    /// The file name.
    public var name: String
    /// The size.
    public var sizeField: Int64
    /// Bytes held from an earlier session.
    public var heldBytes: Int64
    /// The sending station.
    public var callsign: String

    init(_ c: sdrkit_file_transfer_file_open_event) {
        time = c.time
        freqHz = c.freq_hz
        session = c.session
        name = sdrkitText(c.name)
        sizeField = c.size_
        heldBytes = c.held_bytes
        callsign = sdrkitText(c.callsign)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FileTransferFileOpenEvent, SDRKitCode> {
        var c = sdrkit_file_transfer_file_open_event()
        let code = sdrkit_file_transfer_file_open_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FileTransferFileOpenEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a transmission that started, finished or was refused.
public struct FileTransferFileTxEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the transmission did: `started`, `finished` or `refused`.
    public var state: String
    /// Why the transmission was refused or ended early; empty for none.
    public var reason: String

    init(_ c: sdrkit_file_transfer_file_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FileTransferFileTxEvent, SDRKitCode> {
        var c = sdrkit_file_transfer_file_tx_event()
        let code = sdrkit_file_transfer_file_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FileTransferFileTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a completed chat or data message.
public struct HfChatMessageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The sending station; empty for a data message.
    public var callsign: String
    /// The recipient; empty for a data message.
    public var to: String
    /// The text.
    public var text: String

    init(_ c: sdrkit_hf_chat_message_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        to = sdrkitText(c.to)
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfChatMessageEvent, SDRKitCode> {
        var c = sdrkit_hf_chat_message_event()
        let code = sdrkit_hf_chat_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfChatMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a completed beacon.
public struct HfChatBeaconEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The station.
    public var callsign: String
    /// The locator.
    public var locator: String
    /// The position.
    public var position: SDRKitPosition
    /// The power.
    public var powerDbm: Double
    /// The text of a full beacon; empty for a short one.
    public var text: String

    init(_ c: sdrkit_hf_chat_beacon_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        locator = sdrkitText(c.locator)
        position = SDRKitPosition(c.position)
        powerDbm = c.power_dbm
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfChatBeaconEvent, SDRKitCode> {
        var c = sdrkit_hf_chat_beacon_event()
        let code = sdrkit_hf_chat_beacon_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfChatBeaconEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an acknowledgement heard.
public struct HfChatAckEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The acknowledging station.
    public var callsign: String
    /// The message id that it acknowledges.
    public var messageId: Int64
    /// Blocks acknowledged.
    public var blocksAcked: Int64
    /// The SNR that it reports.
    public var snrDb: Double
    /// The profile that it asks for.
    public var profile: Int64

    init(_ c: sdrkit_hf_chat_ack_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        messageId = c.message_id
        blocksAcked = c.blocks_acked
        snrDb = c.snr_db
        profile = c.profile
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfChatAckEvent, SDRKitCode> {
        var c = sdrkit_hf_chat_ack_event()
        let code = sdrkit_hf_chat_ack_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfChatAckEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a voice block heard.
public struct HfChatVoiceEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The calling station.
    public var callsign: String
    /// The call number.
    public var call: Int64
    /// The profile of the burst.
    public var profile: Int64

    init(_ c: sdrkit_hf_chat_voice_event) {
        time = c.time
        freqHz = c.freq_hz
        callsign = sdrkitText(c.callsign)
        call = c.call
        profile = c.profile
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfChatVoiceEvent, SDRKitCode> {
        var c = sdrkit_hf_chat_voice_event()
        let code = sdrkit_hf_chat_voice_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfChatVoiceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a transmission that started, finished or was refused.
public struct HfChatTxEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// What the transmission did: `started`, `finished` or `refused`.
    public var state: String
    /// Why the transmission was refused or ended early; empty for none.
    public var reason: String

    init(_ c: sdrkit_hf_chat_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HfChatTxEvent, SDRKitCode> {
        var c = sdrkit_hf_chat_tx_event()
        let code = sdrkit_hf_chat_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HfChatTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum CustomTables {
    /// The changed rows of the table "stations" of the stack "slotted_voice", newest first, at most `capacity` rows.
    public static func slottedVoiceStations(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[SlottedVoiceStationRow], SDRKitCode> {
        var rows = [sdrkit_slotted_voice_station_row](repeating: sdrkit_slotted_voice_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_slotted_voice_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SlottedVoiceStationRow.init))
    }
    /// The changed rows of the table "streams" of the stack "qam_video", newest first, at most `capacity` rows.
    public static func qamVideoStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[QamVideoStreamRow], SDRKitCode> {
        var rows = [sdrkit_qam_video_stream_row](repeating: sdrkit_qam_video_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qam_video_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(QamVideoStreamRow.init))
    }
    /// The changed rows of the table "pictures" of the stack "picture_burst", newest first, at most `capacity` rows.
    public static func pictureBurstPictures(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[PictureBurstPictureRow], SDRKitCode> {
        var rows = [sdrkit_picture_burst_picture_row](repeating: sdrkit_picture_burst_picture_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_picture_burst_pictures_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PictureBurstPictureRow.init))
    }
    /// The changed rows of the table "messages" of the stack "arq_packet", newest first, at most `capacity` rows.
    public static func arqPacketMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[ArqPacketMessageRow], SDRKitCode> {
        var rows = [sdrkit_arq_packet_message_row](repeating: sdrkit_arq_packet_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_arq_packet_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ArqPacketMessageRow.init))
    }
    /// The changed rows of the table "files" of the stack "file_transfer", newest first, at most `capacity` rows.
    public static func fileTransferFiles(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[FileTransferFileRow], SDRKitCode> {
        var rows = [sdrkit_file_transfer_file_row](repeating: sdrkit_file_transfer_file_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_file_transfer_files_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FileTransferFileRow.init))
    }
    /// The changed rows of the table "messages" of the stack "hf_chat", newest first, at most `capacity` rows.
    public static func hfChatMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[HfChatMessageRow], SDRKitCode> {
        var rows = [sdrkit_hf_chat_message_row](repeating: sdrkit_hf_chat_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hf_chat_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfChatMessageRow.init))
    }
    /// The changed rows of the table "beacons" of the stack "hf_chat", newest first, at most `capacity` rows.
    public static func hfChatBeacons(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[HfChatBeaconRow], SDRKitCode> {
        var rows = [sdrkit_hf_chat_beacon_row](repeating: sdrkit_hf_chat_beacon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hf_chat_beacons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HfChatBeaconRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum CustomCommands {
    /// Clear. Forgets everything decoded so far.
    public static func slottedVoiceClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_slotted_voice_clear(handle)
        return sdrkitResult(code)
    }
    /// Send. Speaks the speech file (or the test vowel) once with the station's settings. Transmit must be on. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func slottedVoiceSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_slotted_voice_send(handle)
        return sdrkitResult(code)
    }
    /// Stop. Ends the transmission at the next frame.
    public static func slottedVoiceStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_slotted_voice_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func qamVideoClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qam_video_clear(handle)
        return sdrkitResult(code)
    }
    /// Send. Sends the file (or the synthetic test stream) once with the station's settings. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func qamVideoSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qam_video_send(handle)
        return sdrkitResult(code)
    }
    /// Stop. Ends the transmission at once.
    public static func qamVideoStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qam_video_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func pictureBurstClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_picture_burst_clear(handle)
        return sdrkitResult(code)
    }
    /// Send. Sends the picture (or the test picture) once in a burst of 10 s. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func pictureBurstSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_picture_burst_send(handle)
        return sdrkitResult(code)
    }
    /// Stop. Ends the transmission at once.
    public static func pictureBurstStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_picture_burst_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func arqPacketClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_arq_packet_clear(handle)
        return sdrkitResult(code)
    }
    /// Send. Sends the message (the text, the file or a test text) once. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func arqPacketSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_arq_packet_send(handle)
        return sdrkitResult(code)
    }
    /// Stop. Ends the transmission after the burst on the air.
    public static func arqPacketStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_arq_packet_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fileTransferClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_file_transfer_clear(handle)
        return sdrkitResult(code)
    }
    /// Send. Sends the file (or a test text) one way or in a two-way session. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func fileTransferSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_file_transfer_send(handle)
        return sdrkitResult(code)
    }
    /// Stop. Ends the transmission after the frame or burst on the air.
    public static func fileTransferStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_file_transfer_stop(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func hfChatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hf_chat_clear(handle)
        return sdrkitResult(code)
    }
    /// Send. Sends the message in the next caller slots. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func hfChatSend(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hf_chat_send(handle)
        return sdrkitResult(code)
    }
    /// Stop. Ends the transmission after the burst on the air.
    public static func hfChatStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hf_chat_stop(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum CustomSettings {
    /// The options of the setting "listen_rate" of the stack "slotted_voice".
    public enum SlottedVoiceListenRate: Int32 {
        case both = 0
        case n8000SymbolsASecond = 1
        case n16000SymbolsASecond = 2
    }
    /// Symbol rates.
    public static func slottedVoiceListenRate(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceListenRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "listen_rate", &v)
        return code == SDRKIT_OK ? .success(SlottedVoiceListenRate(rawValue: v) ?? SlottedVoiceListenRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceListenRate(_ handle: SDRKIT_HANDLE, _ value: SlottedVoiceListenRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "listen_rate", value.rawValue))
    }
    /// Station callsign.
    public static func slottedVoiceCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// Grid locator.
    public static func slottedVoiceGrid(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "grid", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceGrid(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "grid", $0) })
    }
    /// Latitude.
    public static func slottedVoiceLat(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "lat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceLat(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "lat", value))
    }
    /// Longitude.
    public static func slottedVoiceLon(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "lon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceLon(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "lon", value))
    }
    /// Beacon power value.
    public static func slottedVoicePowerDbm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "power_dbm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoicePowerDbm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "power_dbm", value))
    }
    /// Link quality value.
    public static func slottedVoiceLink(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "link", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceLink(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "link", value))
    }
    /// Beacon.
    public static func slottedVoiceBeacon(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "beacon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceBeacon(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "beacon", value))
    }
    /// Beacon text.
    public static func slottedVoiceBeaconText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "beacon_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceBeaconText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "beacon_text", $0) })
    }
    /// The options of the setting "frame_version" of the stack "slotted_voice".
    public enum SlottedVoiceFrameVersion: Int32 {
        case version3 = 0
        case version4 = 1
    }
    /// Frame version.
    public static func slottedVoiceFrameVersion(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceFrameVersion, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_version", &v)
        return code == SDRKIT_OK ? .success(SlottedVoiceFrameVersion(rawValue: v) ?? SlottedVoiceFrameVersion(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceFrameVersion(_ handle: SDRKIT_HANDLE, _ value: SlottedVoiceFrameVersion) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_version", value.rawValue))
    }
    /// The options of the setting "bit_rate" of the stack "slotted_voice".
    public enum SlottedVoiceBitRate: Int32 {
        case n12KbitS = 0
        case n16KbitS = 1
        case n24KbitS = 2
        case n32KbitS = 3
    }
    /// Bit rate.
    public static func slottedVoiceBitRate(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceBitRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bit_rate", &v)
        return code == SDRKIT_OK ? .success(SlottedVoiceBitRate(rawValue: v) ?? SlottedVoiceBitRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceBitRate(_ handle: SDRKIT_HANDLE, _ value: SlottedVoiceBitRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bit_rate", value.rawValue))
    }
    /// Voice slots.
    public static func slottedVoiceSlots(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "voice_slots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceSlots(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "voice_slots", value))
    }
    /// The options of the setting "voice_profile" of the stack "slotted_voice".
    public enum SlottedVoiceProfile: Int32 {
        case v80 = 0
        case v60 = 1
        case v48 = 2
        case v32r = 3
    }
    /// Voice profile.
    public static func slottedVoiceProfile(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "voice_profile", &v)
        return code == SDRKIT_OK ? .success(SlottedVoiceProfile(rawValue: v) ?? SlottedVoiceProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceProfile(_ handle: SDRKIT_HANDLE, _ value: SlottedVoiceProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "voice_profile", value.rawValue))
    }
    /// Robust data slots.
    public static func slottedVoiceRobustData(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "robust_data", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceRobustData(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "robust_data", value))
    }
    /// Speech file.
    public static func slottedVoiceSpeechFile(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "speech_file", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceSpeechFile(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "speech_file", $0) })
    }
    /// The options of the setting "listen_profile" of the stack "qam_video".
    public enum QamVideoListenProfile: Int32 {
        case a = 0
        case b = 1
        case cd = 2
    }
    /// Listen profile.
    public static func qamVideoListenProfile(_ handle: SDRKIT_HANDLE) -> Result<QamVideoListenProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "listen_profile", &v)
        return code == SDRKIT_OK ? .success(QamVideoListenProfile(rawValue: v) ?? QamVideoListenProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoListenProfile(_ handle: SDRKIT_HANDLE, _ value: QamVideoListenProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "listen_profile", value.rawValue))
    }
    /// Station callsign.
    public static func qamVideoCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// Station tag.
    public static func qamVideoStationTag(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "station_tag", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoStationTag(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "station_tag", $0) })
    }
    /// The options of the setting "profile" of the stack "qam_video".
    public enum QamVideoProfile: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
    }
    /// Profile.
    public static func qamVideoProfile(_ handle: SDRKIT_HANDLE) -> Result<QamVideoProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "profile", &v)
        return code == SDRKIT_OK ? .success(QamVideoProfile(rawValue: v) ?? QamVideoProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoProfile(_ handle: SDRKIT_HANDLE, _ value: QamVideoProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "profile", value.rawValue))
    }
    /// FEC profile.
    public static func qamVideoFec(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "fec", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoFec(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "fec", value))
    }
    /// File.
    public static func qamVideoFile(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "file", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoFile(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "file", $0) })
    }
    /// Synthetic stream.
    public static func qamVideoMediaS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "media_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQamVideoMediaS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "media_s", value))
    }
    /// Station callsign.
    public static func pictureBurstCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPictureBurstCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// The options of the setting "profile" of the stack "picture_burst".
    public enum PictureBurstProfile: Int32 {
        case n0Balanced = 0
        case n1Robust = 1
        case n2Quality = 2
        case n3Direct = 3
    }
    /// Profile.
    public static func pictureBurstProfile(_ handle: SDRKIT_HANDLE) -> Result<PictureBurstProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "profile", &v)
        return code == SDRKIT_OK ? .success(PictureBurstProfile(rawValue: v) ?? PictureBurstProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPictureBurstProfile(_ handle: SDRKIT_HANDLE, _ value: PictureBurstProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "profile", value.rawValue))
    }
    /// Picture.
    public static func pictureBurstPicture(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "picture", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPictureBurstPicture(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "picture", $0) })
    }
    /// Station callsign.
    public static func arqPacketCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setArqPacketCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// The options of the setting "profile" of the stack "arq_packet".
    public enum ArqPacketProfile: Int32 {
        case n0Bpsk14 = 0
        case n1Qpsk14 = 1
        case n2Qpsk12 = 2
        case n3Qpsk34 = 3
        case n416Qam12 = 4
        case n516Qam34 = 5
        case n664Qam23 = 6
        case n7256Qam34 = 7
        case n81024Qam56 = 8
    }
    /// Profile.
    public static func arqPacketProfile(_ handle: SDRKIT_HANDLE) -> Result<ArqPacketProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "profile", &v)
        return code == SDRKIT_OK ? .success(ArqPacketProfile(rawValue: v) ?? ArqPacketProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqPacketProfile(_ handle: SDRKIT_HANDLE, _ value: ArqPacketProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "profile", value.rawValue))
    }
    /// Passes.
    public static func arqPacketPasses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "passes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setArqPacketPasses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "passes", value))
    }
    /// The options of the setting "link" of the stack "arq_packet".
    public enum ArqPacketLink: Int32 {
        case oneWay = 0
        case twoWay = 1
    }
    /// Link.
    public static func arqPacketLink(_ handle: SDRKIT_HANDLE) -> Result<ArqPacketLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "link", &v)
        return code == SDRKIT_OK ? .success(ArqPacketLink(rawValue: v) ?? ArqPacketLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setArqPacketLink(_ handle: SDRKIT_HANDLE, _ value: ArqPacketLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "link", value.rawValue))
    }
    /// Message.
    public static func arqPacketMessage(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "message", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setArqPacketMessage(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "message", $0) })
    }
    /// File.
    public static func arqPacketFile(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "file", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setArqPacketFile(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "file", $0) })
    }
    /// Station callsign.
    public static func fileTransferCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// The options of the setting "widths" of the stack "file_transfer".
    public enum FileTransferWidths: Int32 {
        case all = 0
        case n1m = 1
        case n500k = 2
        case n200k = 3
    }
    /// Widths searched.
    public static func fileTransferWidths(_ handle: SDRKIT_HANDLE) -> Result<FileTransferWidths, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "widths", &v)
        return code == SDRKIT_OK ? .success(FileTransferWidths(rawValue: v) ?? FileTransferWidths(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferWidths(_ handle: SDRKIT_HANDLE, _ value: FileTransferWidths) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "widths", value.rawValue))
    }
    /// Folder.
    public static func fileTransferFolder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "folder", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferFolder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "folder", $0) })
    }
    /// The options of the setting "width" of the stack "file_transfer".
    public enum FileTransferWidth: Int32 {
        case n1m = 0
        case n500k = 1
        case n200k = 2
    }
    /// Width.
    public static func fileTransferWidth(_ handle: SDRKIT_HANDLE) -> Result<FileTransferWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "width", &v)
        return code == SDRKIT_OK ? .success(FileTransferWidth(rawValue: v) ?? FileTransferWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferWidth(_ handle: SDRKIT_HANDLE, _ value: FileTransferWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "width", value.rawValue))
    }
    /// The options of the setting "profile" of the stack "file_transfer".
    public enum FileTransferProfile: Int32 {
        case n0Bpsk14 = 0
        case n1Qpsk14 = 1
        case n2Qpsk12 = 2
        case n3Qpsk34 = 3
        case n416Qam34 = 4
        case n564Qam34 = 5
        case n6256Qam34 = 6
        case n7256Qam56 = 7
        case n81024Qam56 = 8
        case n91024Qam910 = 9
    }
    /// Profile.
    public static func fileTransferProfile(_ handle: SDRKIT_HANDLE) -> Result<FileTransferProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "profile", &v)
        return code == SDRKIT_OK ? .success(FileTransferProfile(rawValue: v) ?? FileTransferProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferProfile(_ handle: SDRKIT_HANDLE, _ value: FileTransferProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "profile", value.rawValue))
    }
    /// Passes.
    public static func fileTransferPasses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "passes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferPasses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "passes", value))
    }
    /// The options of the setting "link" of the stack "file_transfer".
    public enum FileTransferLink: Int32 {
        case oneWay = 0
        case twoWay = 1
    }
    /// Link.
    public static func fileTransferLink(_ handle: SDRKIT_HANDLE) -> Result<FileTransferLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "link", &v)
        return code == SDRKIT_OK ? .success(FileTransferLink(rawValue: v) ?? FileTransferLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferLink(_ handle: SDRKIT_HANDLE, _ value: FileTransferLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "link", value.rawValue))
    }
    /// File.
    public static func fileTransferFile(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "file", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferFile(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "file", $0) })
    }
    /// Recipient.
    public static func fileTransferRecipient(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "recipient", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferRecipient(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "recipient", $0) })
    }
    /// Session id.
    public static func fileTransferSession(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "session", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferSession(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "session", value))
    }
    /// Fixed profile.
    public static func fileTransferFixedProfile(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "fixed_profile", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFileTransferFixedProfile(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "fixed_profile", value))
    }
    /// The options of the setting "listen" of the stack "hf_chat".
    public enum HfChatListen: Int32 {
        case all = 0
        case profile0 = 1
        case profile1 = 2
        case profile2 = 3
        case profile3 = 4
        case profile4 = 5
        case profile5 = 6
        case profile6 = 7
        case profile7 = 8
    }
    /// Profiles searched.
    public static func hfChatListen(_ handle: SDRKIT_HANDLE) -> Result<HfChatListen, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(HfChatListen(rawValue: v) ?? HfChatListen(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatListen(_ handle: SDRKIT_HANDLE, _ value: HfChatListen) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "listen", value.rawValue))
    }
    /// Station callsign.
    public static func hfChatCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// The options of the setting "profile" of the stack "hf_chat".
    public enum HfChatProfile: Int32 {
        case n0AbyssChirpSf10Rate14X3 = 0
        case n1DeepChirpSf10Rate14 = 1
        case n2WeakChirpSf9Rate13 = 2
        case n3ChatChirpSf8Rate12 = 3
        case n4MidChirpSf6Rate12 = 4
        case n5FastDataOfdmBpskRate14 = 5
        case n6Voice700OfdmQpsk = 6
        case n7Voice1200OfdmQpsk = 7
    }
    /// Profile.
    public static func hfChatProfile(_ handle: SDRKIT_HANDLE) -> Result<HfChatProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "profile", &v)
        return code == SDRKIT_OK ? .success(HfChatProfile(rawValue: v) ?? HfChatProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatProfile(_ handle: SDRKIT_HANDLE, _ value: HfChatProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "profile", value.rawValue))
    }
    /// The options of the setting "kind" of the stack "hf_chat".
    public enum HfChatKind: Int32 {
        case chat = 0
        case shortBeacon = 1
        case fullBeacon = 2
        case data = 3
    }
    /// Message.
    public static func hfChatKind(_ handle: SDRKIT_HANDLE) -> Result<HfChatKind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "kind", &v)
        return code == SDRKIT_OK ? .success(HfChatKind(rawValue: v) ?? HfChatKind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatKind(_ handle: SDRKIT_HANDLE, _ value: HfChatKind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "kind", value.rawValue))
    }
    /// Recipient.
    public static func hfChatTo(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "to", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatTo(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "to", $0) })
    }
    /// Text.
    public static func hfChatText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "text", $0) })
    }
    /// Locator.
    public static func hfChatLocator(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "locator", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatLocator(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "locator", $0) })
    }
    /// Latitude.
    public static func hfChatLat(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "lat", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatLat(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "lat", value))
    }
    /// Longitude.
    public static func hfChatLon(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "lon", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatLon(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "lon", value))
    }
    /// Power step.
    public static func hfChatPowerStep(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "power_step", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatPowerStep(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "power_step", value))
    }
    /// File.
    public static func hfChatFile(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "file", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatFile(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "file", $0) })
    }
    /// Passes.
    public static func hfChatPasses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "passes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatPasses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "passes", value))
    }
    /// The options of the setting "link" of the stack "hf_chat".
    public enum HfChatLink: Int32 {
        case oneWay = 0
        case twoWay = 1
    }
    /// Link.
    public static func hfChatLink(_ handle: SDRKIT_HANDLE) -> Result<HfChatLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "link", &v)
        return code == SDRKIT_OK ? .success(HfChatLink(rawValue: v) ?? HfChatLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHfChatLink(_ handle: SDRKIT_HANDLE, _ value: HfChatLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "link", value.rawValue))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum CustomGeneratorSettings {
    /// The options of the setting "gen_bit_rate" of the stack "slotted_voice".
    public enum SlottedVoiceBitRate: Int32 {
        case n12KbitS = 0
        case n16KbitS = 1
        case n24KbitS = 2
        case n32KbitS = 3
    }
    /// Test signal bit rate.
    public static func slottedVoiceBitRate(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceBitRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bit_rate", &v)
        return code == SDRKIT_OK ? .success(SlottedVoiceBitRate(rawValue: v) ?? SlottedVoiceBitRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceBitRate(_ handle: SDRKIT_HANDLE, _ value: SlottedVoiceBitRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bit_rate", value.rawValue))
    }
    /// The options of the setting "gen_frame_version" of the stack "slotted_voice".
    public enum SlottedVoiceFrameVersion: Int32 {
        case version3 = 0
        case version4 = 1
    }
    /// Test signal frame version.
    public static func slottedVoiceFrameVersion(_ handle: SDRKIT_HANDLE) -> Result<SlottedVoiceFrameVersion, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_frame_version", &v)
        return code == SDRKIT_OK ? .success(SlottedVoiceFrameVersion(rawValue: v) ?? SlottedVoiceFrameVersion(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSlottedVoiceFrameVersion(_ handle: SDRKIT_HANDLE, _ value: SlottedVoiceFrameVersion) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_frame_version", value.rawValue))
    }
}

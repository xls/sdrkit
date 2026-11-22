// sdrkit: the typed interface of the stack family "dvbt2", version 1.4. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum Dvbt2Interface {
    public static let major = 1
    public static let minor = 4
}

/// The result fields of `dvbt2`, which the receiver `rx` writes. A value of the frame shape has no value before the P2 symbols are found, and a value of the L1 signalling has no value before the L1-pre is decoded.
public struct Dvbt2Fields {
    public enum Detection: Int32 {
        case searching = 0
        case p1 = 1
        case l1 = 2
        case flowing = 3
    }
    public enum Fft: Int32 {
        case n1k = 0
        case n2k = 1
        case n4k = 2
        case n8k = 3
        case n16k = 4
        case n32k = 5
    }
    public enum Guard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
        case n1128 = 4
        case n19128 = 5
        case n19256 = 6
    }
    public enum PilotPattern: Int32 {
        case pp1 = 0
        case pp2 = 1
        case pp3 = 2
        case pp4 = 3
        case pp5 = 4
        case pp6 = 5
        case pp7 = 6
        case pp8 = 7
    }
    public enum Profile: Int32 {
        case base = 0
        case lite = 1
    }
    public enum L1Modulation: Int32 {
        case bpsk = 0
        case qpsk = 1
        case n16qam = 2
        case n64qam = 3
        case n256qam = 4
    }
    public enum Problem: Int32 {
        case tfs = 0
        case frameShape = 1
        case l1PostSize = 2
        case noSuchPlp = 3
        case noDataPlp = 4
        case unknownCoding = 5
    }
    /// Detection: searching, P1 found, L1 decoded, or packets flowing.
    public var detection: Detection?
    /// The FFT size.
    public var fft: Fft?
    /// The guard interval.
    public var `guard`: Guard?
    /// The scattered pilot pattern.
    public var pilotPattern: PilotPattern?
    /// Extended carrier mode.
    public var extendedCarriers: Bool?
    /// MISO.
    public var miso: Bool?
    /// The profile.
    public var profile: Profile?
    /// The L1-post constellation.
    public var l1Modulation: L1Modulation?
    /// CELL_ID.
    public var cellId: Int64?
    /// NETWORK_ID.
    public var networkId: Int64?
    /// T2_SYSTEM_ID.
    public var t2SystemId: Int64?
    /// NUM_T2_FRAMES.
    public var framesPerSuperframe: Int64?
    /// NUM_DATA_SYMBOLS.
    public var dataSymbols: Int64?
    /// The T2-frame length.
    public var frameLength: Double?
    /// T2-frames decoded.
    public var t2Frames: Int64
    /// L1 signalling failures.
    public var l1Errors: Int64
    /// The data PLP decoded.
    public var plp: Int64?
    /// Its common PLP.
    public var commonPlp: Int64?
    /// Mean LDPC iterations per FEC block.
    public var ldpcIterations: Double
    /// The net bit rate of the decoded data PLP.
    public var netRateBitS: Double?
    /// A configuration that the receiver cannot follow.
    public var problem: Problem?
    /// The carrier offset from the centre. No value while the receiver searches.
    public var offsetHz: Double?
    /// The SNR.
    public var snrDb: Double?
    /// FEC blocks good.
    public var framesOk: Int64
    /// FEC blocks that failed.
    public var framesBad: Int64
    /// Bits that the BCH decoder corrected.
    public var bitsCorrected: Int64

    init(_ c: sdrkit_dvbt2_fields) {
        detection = Detection(rawValue: c.detection)
        fft = c.has_fft ? Fft(rawValue: c.fft) : nil
        `guard` = c.has_guard ? Guard(rawValue: c.guard) : nil
        pilotPattern = c.has_pilot_pattern ? PilotPattern(rawValue: c.pilot_pattern) : nil
        extendedCarriers = c.has_extended_carriers ? c.extended_carriers : nil
        miso = c.has_miso ? c.miso : nil
        profile = c.has_profile ? Profile(rawValue: c.profile) : nil
        l1Modulation = c.has_l1_modulation ? L1Modulation(rawValue: c.l1_modulation) : nil
        cellId = c.has_cell_id ? c.cell_id : nil
        networkId = c.has_network_id ? c.network_id : nil
        t2SystemId = c.has_t2_system_id ? c.t2_system_id : nil
        framesPerSuperframe = c.has_frames_per_superframe ? c.frames_per_superframe : nil
        dataSymbols = c.has_data_symbols ? c.data_symbols : nil
        frameLength = c.has_frame_length ? c.frame_length : nil
        t2Frames = c.t2_frames
        l1Errors = c.l1_errors
        plp = c.has_plp ? c.plp : nil
        commonPlp = c.has_common_plp ? c.common_plp : nil
        ldpcIterations = c.ldpc_iterations
        netRateBitS = c.has_net_rate_bit_s ? c.net_rate_bit_s : nil
        problem = c.has_problem ? Problem(rawValue: c.problem) : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        bitsCorrected = c.bits_corrected
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Fields, SDRKitCode> {
        var c = sdrkit_dvbt2_fields()
        let code = sdrkit_dvbt2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbt2Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the sender `transmit` of `dvbt2`.
public struct Dvbt2TxFields {
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency: the centre of the channel sent. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// The bit rate of the transport stream that the settings carry: the rate that the host feeds at most. 0 while the settings cannot be sent.
    public var txBitrateBps: Double
    /// The packets of the host sent in this transmission, or in the last one.
    public var txPackets: Int64
    /// The null packets put in slots that found no packet of the host.
    public var txNulls: Int64
    /// The packets that wait to be sent.
    public var txQueue: Int64
    /// The time that the waiting packets take at the useful bit rate.
    public var txQueueS: Double
    /// The PCRs rewritten to the time of their slot.
    public var txPcrRestamped: Int64
    /// The length of a T2-frame. No value while the settings cannot be sent.
    public var txFrameS: Double?
    /// The FEC blocks of a T2-frame. No value while the settings cannot be sent.
    public var txFecBlocks: Int64?
    /// Why the settings cannot be sent. No value while they can.
    public var txProblem: String?

    init(_ c: sdrkit_dvbt2_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txBitrateBps = c.tx_bitrate_bps
        txPackets = c.tx_packets
        txNulls = c.tx_nulls
        txQueue = c.tx_queue
        txQueueS = c.tx_queue_s
        txPcrRestamped = c.tx_pcr_restamped
        txFrameS = c.has_tx_frame_s ? c.tx_frame_s : nil
        txFecBlocks = c.has_tx_fec_blocks ? c.tx_fec_blocks : nil
        txProblem = c.has_tx_problem ? sdrkitText(c.tx_problem) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxFields, SDRKitCode> {
        var c = sdrkit_dvbt2_tx_fields()
        let code = sdrkit_dvbt2_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbt2TxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the PLPs of the multiplex. The row key is PLP_ID in decimal.
public struct Dvbt2PlpRow {
    public enum PlpType: Int32 {
        case common = 0
        case data1 = 1
        case data2 = 2
        case reserved = 3
    }
    public enum Payload: Int32 {
        case gfps = 0
        case gcs = 1
        case gse = 2
        case ts = 3
        case reserved = 4
    }
    public enum Modulation: Int32 {
        case bpsk = 0
        case qpsk = 1
        case n16qam = 2
        case n64qam = 3
        case n256qam = 4
    }
    public enum CodeRate: Int32 {
        case n12 = 0
        case n35 = 1
        case n23 = 2
        case n34 = 3
        case n45 = 4
        case n56 = 5
        case n13 = 6
        case n25 = 7
        case n14 = 8
    }
    public enum TimeInterleaving: Int32 {
        case off = 0
        case blocks = 1
        case frames = 2
    }
    /// The row key.
    public var key: String
    /// PLP_ID.
    public var plpId: Int64
    /// PLP_TYPE.
    public var plpType: PlpType?
    /// PLP_PAYLOAD_TYPE.
    public var payload: Payload?
    /// The constellation. No value for a reserved code.
    public var modulation: Modulation?
    /// The code rate. No value for a code that the profile does not use.
    public var codeRate: CodeRate?
    /// The FEC frame length. No value for a reserved code.
    public var fecFrameBits: Int64?
    /// Rotated constellation.
    public var rotated: Bool
    /// PLP_NUM_BLOCKS of this frame.
    public var fecBlocks: Int64
    /// PLP_GROUP_ID.
    public var groupId: Int64
    /// How the PLP is time interleaved.
    public var timeInterleaving: TimeInterleaving?
    /// TIME_IL_LENGTH: TI-blocks per frame, or T2-frames per TI-block.
    public var timeIlLength: Int64

    init(_ c: sdrkit_dvbt2_plp_row) {
        key = sdrkitText(c.key)
        plpId = c.plp_id
        plpType = PlpType(rawValue: c.plp_type)
        payload = Payload(rawValue: c.payload)
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        codeRate = c.has_code_rate ? CodeRate(rawValue: c.code_rate) : nil
        fecFrameBits = c.has_fec_frame_bits ? c.fec_frame_bits : nil
        rotated = c.rotated
        fecBlocks = c.fec_blocks
        groupId = c.group_id
        timeInterleaving = TimeInterleaving(rawValue: c.time_interleaving)
        timeIlLength = c.time_il_length
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum Dvbt2Components {
    /// The result fields of the component "ts" of the stack "dvbt2" in the snapshot of `handle`.
    public static func dvbt2TsBbFrameFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastBbFrameFields, SDRKitCode> {
        var c = sdrkit_ipcast_bb_frame_fields()
        let code = sdrkit_dvbt2_ts_bb_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastBbFrameFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvbt2" in the snapshot of `handle`.
    public static func dvbt2TsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvbt2_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvbt2" in the snapshot of `handle`.
    public static func dvbt2PsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvbt2_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "dvbt2" in the snapshot of `handle`.
    public static func dvbt2TeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_dvbt2_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "dvbt2" in the snapshot of `handle`.
    public static func dvbt2DvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_dvbt2_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum Dvbt2Tables {
    /// The changed rows of the table "plps" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2Plps(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbt2PlpRow], SDRKitCode> {
        var rows = [sdrkit_dvbt2_plp_row](repeating: sdrkit_dvbt2_plp_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_plps_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbt2PlpRow.init))
    }
    /// The changed rows of the table "services" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2Services(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2Tracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2Guide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2GuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2TeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2TeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2TsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvbt2", newest first, at most `capacity` rows.
    public static func dvbt2PsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt2_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum Dvbt2Commands {
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func dvbt2ServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt2_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt2_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbt2_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a teletext page. Shows a teletext page.
    public static func dvbt2TeletextSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt2_teletext_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt2_teletext_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbt2_teletext_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func dvbt2TrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt2_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt2_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbt2_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Start sending. Sends a DVB-T2 signal with the tx_* settings from the start of a T2-frame, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func dvbt2Start(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbt2_start(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission after the T2-frame on air with a fall of 5 ms. The packets that wait are dropped.
    public static func dvbt2Stop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbt2_stop(handle)
        return sdrkitResult(code)
    }
    /// Add packets. Adds transport packets to the queue of the stream. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.
    public static func dvbt2Append(_ handle: SDRKIT_HANDLE, packets: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt2_tx_append_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt2_tx_append_args>.size)
        let code: Int32 = packets.withUnsafeBufferPointer { packets_ in { () -> Int32 in
        args.packets = sdrkit_bytes(ptr: packets_.baseAddress, len: packets_.count)
        return sdrkit_dvbt2_append(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum Dvbt2Settings {
    /// The options of the setting "bandwidth" of the stack "dvbt2".
    public enum Dvbt2Bandwidth: Int32 {
        case n17 = 0
        case n5Mhz = 1
        case n6Mhz = 2
        case n7Mhz = 3
        case n8Mhz = 4
        case n10Mhz = 5
    }
    /// Channel bandwidth.
    public static func dvbt2Bandwidth(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Bandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Bandwidth(rawValue: v) ?? Dvbt2Bandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Bandwidth(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Bandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// PLP.
    public static func dvbt2Plp(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "plp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Plp(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "plp", value))
    }
    /// Decode video.
    public static func dvbt2DecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2DecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// The options of the setting "tx_bandwidth" of the stack "dvbt2".
    public enum Dvbt2TxBandwidth: Int32 {
        case n17 = 0
        case n5Mhz = 1
        case n6Mhz = 2
        case n7Mhz = 3
        case n8Mhz = 4
        case n10Mhz = 5
    }
    /// Channel width.
    public static func dvbt2TxBandwidth(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_bandwidth", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxBandwidth(rawValue: v) ?? Dvbt2TxBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxBandwidth(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_bandwidth", value.rawValue))
    }
    /// The options of the setting "tx_fft" of the stack "dvbt2".
    public enum Dvbt2TxFft: Int32 {
        case n1k = 0
        case n2k = 1
        case n4k = 2
        case n8k = 3
        case n16k = 4
        case n32k = 5
    }
    /// FFT size.
    public static func dvbt2TxFft(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxFft, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_fft", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxFft(rawValue: v) ?? Dvbt2TxFft(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxFft(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxFft) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_fft", value.rawValue))
    }
    /// The options of the setting "tx_guard" of the stack "dvbt2".
    public enum Dvbt2TxGuard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
        case n1128 = 4
        case n19128 = 5
        case n19256 = 6
    }
    /// Guard interval.
    public static func dvbt2TxGuard(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxGuard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_guard", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxGuard(rawValue: v) ?? Dvbt2TxGuard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxGuard(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxGuard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_guard", value.rawValue))
    }
    /// The options of the setting "tx_pilots" of the stack "dvbt2".
    public enum Dvbt2TxPilots: Int32 {
        case pp1 = 0
        case pp2 = 1
        case pp3 = 2
        case pp4 = 3
        case pp5 = 4
        case pp6 = 5
        case pp7 = 6
        case pp8 = 7
    }
    /// Pilot pattern.
    public static func dvbt2TxPilots(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_pilots", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxPilots(rawValue: v) ?? Dvbt2TxPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxPilots(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_pilots", value.rawValue))
    }
    /// Extended carriers.
    public static func dvbt2TxExtended(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_extended", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxExtended(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_extended", value))
    }
    /// The options of the setting "tx_modulation" of the stack "dvbt2".
    public enum Dvbt2TxModulation: Int32 {
        case qpsk = 0
        case n16qam = 1
        case n64qam = 2
        case n256qam = 3
    }
    /// Constellation.
    public static func dvbt2TxModulation(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_modulation", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxModulation(rawValue: v) ?? Dvbt2TxModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxModulation(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_modulation", value.rawValue))
    }
    /// The options of the setting "tx_rate" of the stack "dvbt2".
    public enum Dvbt2TxRate: Int32 {
        case n12 = 0
        case n35 = 1
        case n23 = 2
        case n34 = 3
        case n45 = 4
        case n56 = 5
        case n13 = 6
        case n25 = 7
    }
    /// Code rate.
    public static func dvbt2TxRate(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_rate", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxRate(rawValue: v) ?? Dvbt2TxRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxRate(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_rate", value.rawValue))
    }
    /// Rotated constellation.
    public static func dvbt2TxRotation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_rotation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxRotation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_rotation", value))
    }
    /// The options of the setting "tx_fec_frame" of the stack "dvbt2".
    public enum Dvbt2TxFecFrame: Int32 {
        case n16200Bits = 0
        case n64800Bits = 1
    }
    /// FEC frame.
    public static func dvbt2TxFecFrame(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxFecFrame, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_fec_frame", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxFecFrame(rawValue: v) ?? Dvbt2TxFecFrame(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxFecFrame(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxFecFrame) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_fec_frame", value.rawValue))
    }
    /// Time interleaver.
    public static func dvbt2TxTiBlocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_ti_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxTiBlocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_ti_blocks", value))
    }
    /// Longest frame.
    public static func dvbt2TxFrameMs(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_frame_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxFrameMs(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_frame_ms", value))
    }
    /// The options of the setting "tx_l1_mod" of the stack "dvbt2".
    public enum Dvbt2TxL1Mod: Int32 {
        case bpsk = 0
        case qpsk = 1
        case n16qam = 2
        case n64qam = 3
    }
    /// L1-post constellation.
    public static func dvbt2TxL1Mod(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxL1Mod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_l1_mod", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxL1Mod(rawValue: v) ?? Dvbt2TxL1Mod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxL1Mod(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxL1Mod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_l1_mod", value.rawValue))
    }
    /// Cell id.
    public static func dvbt2TxCellId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_cell_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxCellId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_cell_id", value))
    }
    /// Network id.
    public static func dvbt2TxNetworkId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_network_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxNetworkId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_network_id", value))
    }
    /// T2 system id.
    public static func dvbt2TxSystemId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_system_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxSystemId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_system_id", value))
    }
    /// The options of the setting "tx_profile" of the stack "dvbt2".
    public enum Dvbt2TxProfile: Int32 {
        case base = 0
        case lite = 1
    }
    /// Profile.
    public static func dvbt2TxProfile(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxProfile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_profile", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxProfile(rawValue: v) ?? Dvbt2TxProfile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxProfile(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxProfile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_profile", value.rawValue))
    }
    /// The options of the setting "tx_miso" of the stack "dvbt2".
    public enum Dvbt2TxMiso: Int32 {
        case siso = 0
        case group1 = 1
        case group2 = 2
    }
    /// MISO.
    public static func dvbt2TxMiso(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2TxMiso, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_miso", &v)
        return code == SDRKIT_OK ? .success(Dvbt2TxMiso(rawValue: v) ?? Dvbt2TxMiso(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2TxMiso(_ handle: SDRKIT_HANDLE, _ value: Dvbt2TxMiso) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_miso", value.rawValue))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum Dvbt2GeneratorSettings {
    /// The options of the setting "gen_fft" of the stack "dvbt2".
    public enum Dvbt2Fft: Int32 {
        case n1k = 0
        case n2k = 1
        case n4k = 2
        case n8k = 3
        case n16k = 4
        case n32k = 5
    }
    /// FFT size.
    public static func dvbt2Fft(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Fft, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_fft", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Fft(rawValue: v) ?? Dvbt2Fft(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Fft(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Fft) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_fft", value.rawValue))
    }
    /// The options of the setting "gen_guard" of the stack "dvbt2".
    public enum Dvbt2Guard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
        case n1128 = 4
        case n19128 = 5
        case n19256 = 6
    }
    /// Guard interval.
    public static func dvbt2Guard(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Guard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Guard(rawValue: v) ?? Dvbt2Guard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Guard(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Guard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_guard", value.rawValue))
    }
    /// The options of the setting "gen_pilots" of the stack "dvbt2".
    public enum Dvbt2Pilots: Int32 {
        case pp1 = 0
        case pp2 = 1
        case pp3 = 2
        case pp4 = 3
        case pp5 = 4
        case pp6 = 5
        case pp7 = 6
        case pp8 = 7
    }
    /// Pilot pattern.
    public static func dvbt2Pilots(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Pilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_pilots", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Pilots(rawValue: v) ?? Dvbt2Pilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Pilots(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Pilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_pilots", value.rawValue))
    }
    /// Extended carriers.
    public static func dvbt2Extended(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_extended", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Extended(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_extended", value))
    }
    /// The options of the setting "gen_modulation" of the stack "dvbt2".
    public enum Dvbt2Modulation: Int32 {
        case qpsk = 0
        case n16qam = 1
        case n64qam = 2
        case n256qam = 3
    }
    /// Constellation.
    public static func dvbt2Modulation(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Modulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Modulation(rawValue: v) ?? Dvbt2Modulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Modulation(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Modulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_modulation", value.rawValue))
    }
    /// The options of the setting "gen_rate" of the stack "dvbt2".
    public enum Dvbt2Rate: Int32 {
        case n12 = 0
        case n35 = 1
        case n23 = 2
        case n34 = 3
        case n45 = 4
        case n56 = 5
        case n13 = 6
        case n25 = 7
    }
    /// Code rate.
    public static func dvbt2Rate(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Rate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Rate(rawValue: v) ?? Dvbt2Rate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Rate(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Rate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
    /// The options of the setting "gen_fec_frame" of the stack "dvbt2".
    public enum Dvbt2FecFrame: Int32 {
        case n16200Bits = 0
        case n64800Bits = 1
    }
    /// FEC frame.
    public static func dvbt2FecFrame(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2FecFrame, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_fec_frame", &v)
        return code == SDRKIT_OK ? .success(Dvbt2FecFrame(rawValue: v) ?? Dvbt2FecFrame(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2FecFrame(_ handle: SDRKIT_HANDLE, _ value: Dvbt2FecFrame) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_fec_frame", value.rawValue))
    }
    /// Rotated constellation.
    public static func dvbt2Rotation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_rotation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Rotation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_rotation", value))
    }
    /// The options of the setting "gen_profile" of the stack "dvbt2".
    public enum Dvbt2Profile: Int32 {
        case base = 0
        case lite = 1
    }
    /// Profile.
    public static func dvbt2Profile(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Profile, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_profile", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Profile(rawValue: v) ?? Dvbt2Profile(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Profile(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Profile) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_profile", value.rawValue))
    }
    /// MISO.
    public static func dvbt2Miso(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_miso", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Miso(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_miso", value))
    }
    /// FEC blocks per frame.
    public static func dvbt2Blocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Blocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_blocks", value))
    }
    /// The options of the setting "gen_source" of the stack "dvbt2".
    public enum Dvbt2Source: Int32 {
        case mux = 0
        case pattern = 1
    }
    /// Content.
    public static func dvbt2Source(_ handle: SDRKIT_HANDLE) -> Result<Dvbt2Source, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_source", &v)
        return code == SDRKIT_OK ? .success(Dvbt2Source(rawValue: v) ?? Dvbt2Source(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbt2Source(_ handle: SDRKIT_HANDLE, _ value: Dvbt2Source) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_source", value.rawValue))
    }
}

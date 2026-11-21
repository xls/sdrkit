// sdrkit: the typed interface of the stack family "satimg", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum SatimgInterface {
    public static let major = 1
    public static let minor = 1
}

/// The fields of the link layers that every coded stack publishes: the front end, the demodulator, the code, the frames and the packets. A field whose report has not come has no value.
public struct SatimgLinkFields {
    /// The Es/N0 of the demodulator, dB.
    public var snrDb: Double?
    /// The level of the input, dBFS.
    public var levelDbfs: Double?
    /// The carrier offset: the energy location, the search and the loop.
    public var offsetHz: Double?
    /// The error of the symbol clock, ppm.
    public var clockPpm: Double?
    /// The nominal symbol rate.
    public var symbolRateHz: Double?
    /// Frames decoded.
    public var framesOk: Int64?
    /// Frames that failed their check.
    public var framesBad: Int64?
    /// The bit error rate before the code.
    public var ber: Double?
    /// The seconds of signal decoded.
    public var signalTime: Double
    /// The input rate.
    public var inputRateHz: Double?
    /// The carrier lock metric, 0 to 1.
    public var lockMetric: Double?
    /// The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late.
    public var phaseAmbiguity: Int64?
    /// The code synchroniser holds its lock.
    public var codeLocked: Bool?
    /// NRZ-M was found before the code.
    public var nrzm: Bool?
    /// Wrong bits of the last coded marker compared (of 52).
    public var markerErrors: Int64?
    /// Reed-Solomon symbols corrected.
    public var bytesCorrected: Int64?
    /// Frame markers found.
    public var markers: Int64?
    /// The spacecraft of the last frame.
    public var spacecraft: Int64?
    /// The virtual channel of the last frame.
    public var vcid: Int64?
    /// The virtual channel frame count of the last frame.
    public var frameCount: Int64?
    /// Space packets given out.
    public var packets: Int64?
    /// Space packets lost.
    public var packetsLost: Int64?

    init(_ c: sdrkit_satimg_link_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        framesOk = c.has_frames_ok ? c.frames_ok : nil
        framesBad = c.has_frames_bad ? c.frames_bad : nil
        ber = c.has_ber ? c.ber : nil
        signalTime = c.signal_time
        inputRateHz = c.has_input_rate_hz ? c.input_rate_hz : nil
        lockMetric = c.has_lock_metric ? c.lock_metric : nil
        phaseAmbiguity = c.has_phase_ambiguity ? c.phase_ambiguity : nil
        codeLocked = c.has_code_locked ? c.code_locked : nil
        nrzm = c.has_nrzm ? c.nrzm : nil
        markerErrors = c.has_marker_errors ? c.marker_errors : nil
        bytesCorrected = c.has_bytes_corrected ? c.bytes_corrected : nil
        markers = c.has_markers ? c.markers : nil
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        vcid = c.has_vcid ? c.vcid : nil
        frameCount = c.has_frame_count ? c.frame_count : nil
        packets = c.has_packets ? c.packets : nil
        packetsLost = c.has_packets_lost ? c.packets_lost : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatimgLinkFields, SDRKitCode> {
        var c = sdrkit_satimg_link_fields()
        let code = sdrkit_satimg_link_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatimgLinkFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of Meteor-M LRPT beside the link layers.
public struct MeteorLrptFields {
    public enum Modulation: Int32 {
        case oqpsk72 = 0
        case qpsk72 = 1
        case oqpsk80 = 2
    }
    public enum ModulationSetting: Int32 {
        case auto = 0
        case oqpsk72 = 1
        case qpsk72 = 2
        case oqpsk80 = 3
    }
    /// The running modulation.
    public var modulation: Modulation?
    /// The modulation setting: automatic or fixed.
    public var modulationSetting: ModulationSetting?
    /// The deinterleaver of the 80k link holds the markers.
    public var interleaverLocked: Bool
    /// The on-board time of the last image packet, seconds of the day (Moscow time).
    public var onboardTime: Double?
    /// The quality factor of the last image packet.
    public var quality: Int64?
    /// Rows of 8 lines in the picture.
    public var pictureRows: Int64
    /// Lines of 1568 pixels in the picture.
    public var pictureLines: Int64
    /// The row time learnt from the on-board times.
    public var rowTime: Double?
    /// The signal time at which frames first decoded.
    public var firstDecoding: Double?
    /// The channels the automatic colour view shows ("1,2,3").
    public var colourChannels: String?

    init(_ c: sdrkit_meteor_lrpt_fields) {
        modulation = Modulation(rawValue: c.modulation)
        modulationSetting = ModulationSetting(rawValue: c.modulation_setting)
        interleaverLocked = c.interleaver_locked
        onboardTime = c.has_onboard_time ? c.onboard_time : nil
        quality = c.has_quality ? c.quality : nil
        pictureRows = c.picture_rows
        pictureLines = c.picture_lines
        rowTime = c.has_row_time ? c.row_time : nil
        firstDecoding = c.has_first_decoding ? c.first_decoding : nil
        colourChannels = c.has_colour_channels ? sdrkitText(c.colour_channels) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeteorLrptFields, SDRKitCode> {
        var c = sdrkit_meteor_lrpt_fields()
        let code = sdrkit_meteor_lrpt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeteorLrptFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The packets of one channel of Meteor-M LRPT. The row key is the channel.
public struct MeteorLrptChannelRow {
    /// The row key.
    public var key: String
    /// The name of an image channel or the telemetry.
    public var channel: String
    /// The packets of a channel.
    public var packets: Int64

    init(_ c: sdrkit_meteor_lrpt_channel_row) {
        key = sdrkitText(c.key)
        channel = sdrkitText(c.channel)
        packets = c.packets
    }
}

/// A finished image. The row key names the image. The picture itself leaves on the picture stream.
public struct SatimgImageRow {
    /// The row key.
    public var key: String
    /// The product, or the instrument, that an image shows.
    public var product: String
    /// The channel that an image shows, when the product names one.
    public var channel: String?
    /// The signal time at which the image started.
    public var time: Double?
    /// The image width in pixels.
    public var width: Int64
    /// The image height in pixels (rows received for a picture that grows).
    public var height: Int64?

    init(_ c: sdrkit_satimg_image_row) {
        key = sdrkitText(c.key)
        product = sdrkitText(c.product)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        time = c.has_time ? c.time : nil
        width = c.width
        height = c.has_height ? c.height : nil
    }
}

/// The fields of an xRIT stack (GOES HRIT, GK-2A LRIT and HRIT, Elektro-L) beside the link layers.
public struct SatimgXritFields {
    public enum RsBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// The name of the running link ("HRIT", "LRIT", "HRIT 665.4 ksym/s", "GGAK 5 ksym/s").
    public var link: String
    /// The carrier is locked (also while frames decode).
    public var carrierLocked: Bool?
    /// The symbol rate the timing loop measures.
    public var symbolRateMeasuredHz: Double?
    /// The second code output arrives inverted (as CCSDS sends it).
    public var g2Inverted: Bool?
    /// The basis of the Reed-Solomon symbols.
    public var rsBasis: RsBasis?
    /// Bytes corrected over the last second, percent.
    public var byteErrorsPct: Double?
    /// Transfer frames (fill frames not counted).
    public var transferFrames: Int64?
    /// Fill frames.
    public var fillFrames: Int64?
    /// Gaps of the frame counts.
    public var frameGaps: Int64?
    /// Packets with a wrong CRC.
    public var crcErrors: Int64?
    /// Files.
    public var files: Int64?
    /// Files with errors.
    public var fileErrors: Int64?
    /// Files whose length differs from their transport header.
    public var lengthErrors: Int64?
    /// Products kept.
    public var products: Int64?
    /// GK-2A: keys loaded.
    public var keys: Int64?
    /// GK-2A: segments waiting for a key.
    public var segmentsLocked: Int64?
    /// What the user can do (empty when all is well).
    public var hint: String
    /// The signal time at which frames first decoded.
    public var firstDecoding: Double?
    /// CADUs imported from a file.
    public var cadusImported: Int64?
    /// GGAK frames.
    public var ggakFrames: Int64?
    /// GGAK frames with a good CRC.
    public var ggakGood: Int64?
    /// GGAK frames with a bad CRC.
    public var ggakBad: Int64?
    /// The marker of the last GGAK frame.
    public var ggakMarker: Int64?
    /// |B| of the last plausible GGAK field.
    public var ggakField: Double?

    init(_ c: sdrkit_satimg_xrit_fields) {
        link = sdrkitText(c.link)
        carrierLocked = c.has_carrier_locked ? c.carrier_locked : nil
        symbolRateMeasuredHz = c.has_symbol_rate_measured_hz ? c.symbol_rate_measured_hz : nil
        g2Inverted = c.has_g2_inverted ? c.g2_inverted : nil
        rsBasis = c.has_rs_basis ? RsBasis(rawValue: c.rs_basis) : nil
        byteErrorsPct = c.has_byte_errors_pct ? c.byte_errors_pct : nil
        transferFrames = c.has_transfer_frames ? c.transfer_frames : nil
        fillFrames = c.has_fill_frames ? c.fill_frames : nil
        frameGaps = c.has_frame_gaps ? c.frame_gaps : nil
        crcErrors = c.has_crc_errors ? c.crc_errors : nil
        files = c.has_files ? c.files : nil
        fileErrors = c.has_file_errors ? c.file_errors : nil
        lengthErrors = c.has_length_errors ? c.length_errors : nil
        products = c.has_products ? c.products : nil
        keys = c.has_keys ? c.keys : nil
        segmentsLocked = c.has_segments_locked ? c.segments_locked : nil
        hint = sdrkitText(c.hint)
        firstDecoding = c.has_first_decoding ? c.first_decoding : nil
        cadusImported = c.has_cadus_imported ? c.cadus_imported : nil
        ggakFrames = c.has_ggak_frames ? c.ggak_frames : nil
        ggakGood = c.has_ggak_good ? c.ggak_good : nil
        ggakBad = c.has_ggak_bad ? c.ggak_bad : nil
        ggakMarker = c.has_ggak_marker ? c.ggak_marker : nil
        ggakField = c.has_ggak_field ? c.ggak_field : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatimgXritFields, SDRKitCode> {
        var c = sdrkit_satimg_xrit_fields()
        let code = sdrkit_satimg_xrit_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatimgXritFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The frames of one virtual channel. The row key is the virtual channel.
public struct SatimgVirtualChannelRow {
    /// The row key.
    public var key: String
    /// The virtual channel of a frame header.
    public var vcid: Int64
    /// The good frames of a virtual channel.
    public var frames: Int64

    init(_ c: sdrkit_satimg_virtual_channel_row) {
        key = sdrkitText(c.key)
        vcid = c.vcid
        frames = c.frames
    }
}

/// A product of an xRIT stack. The row key is the product id.
public struct SatimgProductRow {
    public enum Kind: Int32 {
        case adminMessage = 0
        case dcs = 1
        case emwin = 2
        case mesoscale = 3
        case fullDisk = 4
        case conus = 5
        case image = 6
        case text = 7
        case other = 8
    }
    /// The row key.
    public var key: String
    /// The name of a product in the `products` table.
    public var product: String
    /// The kind of a product in the `products` table.
    public var kind: Kind?
    /// How much of a product arrived: rows or segments for images, a size for others, "..." while it arrives, "errors".
    public var done: String

    init(_ c: sdrkit_satimg_product_row) {
        key = sdrkitText(c.key)
        product = sdrkitText(c.product)
        kind = Kind(rawValue: c.kind)
        done = sdrkitText(c.done)
    }
}

/// The fields of FY-2 S-VISSR beside the link layers.
public struct Fy2SvissrFields {
    /// Lines decoded.
    public var lines: Int64
    /// Sync windows found.
    public var syncs: Int64
    /// Frames refused.
    public var rejected: Int64
    /// Frames with under 2 % preamble errors.
    public var usable: Int64
    /// The preamble bit error ratio of the last frame.
    public var preambleBer: Double
    /// The estimate of the data bit error ratio (the set bits of the spare area).
    public var dataBer: Double
    /// The last line counter.
    public var lineCounter: Int64?
    /// Pictures started.
    public var picturesStarted: Int64
    /// Frames imported.
    public var imported: Int64
    /// What to look at when nothing decodes.
    public var hint: String

    init(_ c: sdrkit_fy2_svissr_fields) {
        lines = c.lines
        syncs = c.syncs
        rejected = c.rejected
        usable = c.usable
        preambleBer = c.preamble_ber
        dataBer = c.data_ber
        lineCounter = c.has_line_counter ? c.line_counter : nil
        picturesStarted = c.pictures_started
        imported = c.imported
        hint = sdrkitText(c.hint)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Fy2SvissrFields, SDRKitCode> {
        var c = sdrkit_fy2_svissr_fields()
        let code = sdrkit_fy2_svissr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Fy2SvissrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of FY-3 AHRPT and MPT beside the link layers.
public struct SatimgFy3Fields {
    public enum Product: Int32 {
        case ahrpt = 0
        case mpt = 1
    }
    /// The product: AHRPT or MPT.
    public var product: Product?
    /// The preset setting (its choice key, "auto" for automatic).
    public var preset: String
    /// The satellite heard (from the spacecraft id).
    public var satellite: String?
    /// The symbol rate measured from the signal.
    public var measuredSymbolRateHz: Double?
    /// Both rails have their phase.
    public var railsLocked: Bool?
    /// The lock quality of the I rail, 0 to 1.
    public var railIQuality: Double?
    /// The lock quality of the Q rail, 0 to 1.
    public var railQQuality: Double?
    /// The differential hypothesis locked (0 to 7).
    public var differentialHypothesis: Int64?
    /// The rows of the tallest picture.
    public var pictureRows: Int64
    /// What to look at when nothing decodes.
    public var hint: String

    init(_ c: sdrkit_satimg_fy3_fields) {
        product = Product(rawValue: c.product)
        preset = sdrkitText(c.preset)
        satellite = c.has_satellite ? sdrkitText(c.satellite) : nil
        measuredSymbolRateHz = c.has_measured_symbol_rate_hz ? c.measured_symbol_rate_hz : nil
        railsLocked = c.has_rails_locked ? c.rails_locked : nil
        railIQuality = c.has_rail_i_quality ? c.rail_i_quality : nil
        railQQuality = c.has_rail_q_quality ? c.rail_q_quality : nil
        differentialHypothesis = c.has_differential_hypothesis ? c.differential_hypothesis : nil
        pictureRows = c.picture_rows
        hint = sdrkitText(c.hint)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatimgFy3Fields, SDRKitCode> {
        var c = sdrkit_satimg_fy3_fields()
        let code = sdrkit_satimg_fy3_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatimgFy3Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An instrument of FY-3. The row key names the instrument.
public struct SatimgFy3InstrumentRow {
    /// The row key.
    public var key: String
    /// An instrument.
    public var instrument: String
    /// The frames of an instrument.
    public var frames: Int64
    /// The lines or scans of an instrument.
    public var rows: Int64

    init(_ c: sdrkit_satimg_fy3_instrument_row) {
        key = sdrkitText(c.key)
        instrument = sdrkitText(c.instrument)
        frames = c.frames
        rows = c.rows
    }
}

/// The fields of NOAA APT.
public struct NoaaAptFields {
    /// The level of the input, dBFS.
    public var levelDbfs: Double?
    /// The carrier offset: the energy location, the search and the loop.
    public var offsetHz: Double
    /// The error of the symbol clock, ppm.
    public var clockPpm: Double
    /// The seconds of signal decoded.
    public var signalTime: Double
    /// The input rate.
    public var inputRateHz: Double?
    /// The line lock holds.
    public var locked: Bool
    /// Lines decoded.
    public var lines: Int64
    /// Lines taken at the expected place because their sync was not found.
    public var linesMissed: Int64
    /// The mean subcarrier amplitude (full deviation is 1).
    public var subcarrierLevel: Double
    /// The correlation of the last line sync with its pattern.
    public var syncQuality: Double
    /// Words a second.
    public var wordRateHz: Double
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_noaa_apt_fields) {
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.offset_hz
        clockPpm = c.clock_ppm
        signalTime = c.signal_time
        inputRateHz = c.has_input_rate_hz ? c.input_rate_hz : nil
        locked = c.locked
        lines = c.lines
        linesMissed = c.lines_missed
        subcarrierLevel = c.subcarrier_level
        syncQuality = c.sync_quality
        wordRateHz = c.word_rate_hz
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NoaaAptFields, SDRKitCode> {
        var c = sdrkit_noaa_apt_fields()
        let code = sdrkit_noaa_apt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NoaaAptFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of NOAA DSB.
public struct NoaaDsbFields {
    /// The Es/N0 of the demodulator, dB.
    public var snrDb: Double?
    /// The level of the input, dBFS.
    public var levelDbfs: Double?
    /// The carrier offset: the energy location, the search and the loop.
    public var offsetHz: Double?
    /// The error of the symbol clock, ppm.
    public var clockPpm: Double?
    /// The nominal symbol rate.
    public var symbolRateHz: Double?
    /// Frames decoded.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The seconds of signal decoded.
    public var signalTime: Double
    /// The input rate.
    public var inputRateHz: Double?
    /// The carrier lock metric, 0 to 1.
    public var lockMetric: Double?
    /// The spacecraft of the last frame.
    public var spacecraft: Int64?
    /// Sync words found.
    public var syncHits: Int64
    /// TIP minor frames.
    public var tipFrames: Int64
    /// HIRS lines.
    public var lines: Int64
    /// The minor frame counter of the last frame, 0 to 319.
    public var minorFrame: Int64?
    /// Which chip starts a bit: 0 or 1.
    public var chipPairing: Int64?
    /// The data arrive inverted.
    public var inverted: Bool
    /// HIRS elements without odd parity.
    public var hirsParityErrors: Int64
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_noaa_dsb_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        signalTime = c.signal_time
        inputRateHz = c.has_input_rate_hz ? c.input_rate_hz : nil
        lockMetric = c.has_lock_metric ? c.lock_metric : nil
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        syncHits = c.sync_hits
        tipFrames = c.tip_frames
        lines = c.lines
        minorFrame = c.has_minor_frame ? c.minor_frame : nil
        chipPairing = c.has_chip_pairing ? c.chip_pairing : nil
        inverted = c.inverted
        hirsParityErrors = c.hirs_parity_errors
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NoaaDsbFields, SDRKitCode> {
        var c = sdrkit_noaa_dsb_fields()
        let code = sdrkit_noaa_dsb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NoaaDsbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of GOES GVAR.
public struct GoesGvarFields {
    /// The Es/N0 of the demodulator, dB.
    public var snrDb: Double?
    /// The level of the input, dBFS.
    public var levelDbfs: Double?
    /// The carrier offset: the energy location, the search and the loop.
    public var offsetHz: Double?
    /// The error of the symbol clock, ppm.
    public var clockPpm: Double?
    /// The nominal symbol rate.
    public var symbolRateHz: Double?
    /// Frames decoded.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The seconds of signal decoded.
    public var signalTime: Double
    /// The input rate.
    public var inputRateHz: Double?
    /// The carrier lock metric, 0 to 1.
    public var lockMetric: Double?
    /// The spacecraft of the last frame.
    public var spacecraft: Int64?
    /// Synchronisation codes found.
    public var syncHits: Int64
    /// Idle blocks.
    public var idleBlocks: Int64
    /// Blocks whose data field failed its CRC.
    public var dataCrcErrors: Int64
    /// The id of the last block.
    public var blockId: Int64?
    /// The block count of the last block.
    public var blockCount: Int64?
    /// The scan count of the last imager block.
    public var scan: Int64
    /// Imager frames whose start was seen.
    public var frameStarts: Int64
    /// Rows of the visible picture.
    public var visibleRows: Int64
    /// Rows of the infrared picture.
    public var infraredRows: Int64
    /// Detector records left out.
    public var recordsRefused: Int64
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_goes_gvar_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        signalTime = c.signal_time
        inputRateHz = c.has_input_rate_hz ? c.input_rate_hz : nil
        lockMetric = c.has_lock_metric ? c.lock_metric : nil
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        syncHits = c.sync_hits
        idleBlocks = c.idle_blocks
        dataCrcErrors = c.data_crc_errors
        blockId = c.has_block_id ? c.block_id : nil
        blockCount = c.has_block_count ? c.block_count : nil
        scan = c.scan
        frameStarts = c.frame_starts
        visibleRows = c.visible_rows
        infraredRows = c.infrared_rows
        recordsRefused = c.records_refused
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GoesGvarFields, SDRKitCode> {
        var c = sdrkit_goes_gvar_fields()
        let code = sdrkit_goes_gvar_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GoesGvarFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A block of GOES GVAR. The row key is the place of the block.
public struct GoesGvarBlockRow {
    /// The row key.
    public var key: String
    /// The block count of the last block.
    public var blockCount: Int64
    /// The id of the last block.
    public var blockId: Int64
    /// The word size of a block.
    public var wordSize: Int64
    /// The word count of a block.
    public var wordCount: Int64
    /// A header copy of a block had a good CRC.
    public var headerOk: Bool
    /// The data field of a block had a good CRC.
    public var dataOk: Bool

    init(_ c: sdrkit_goes_gvar_block_row) {
        key = sdrkitText(c.key)
        blockCount = c.block_count
        blockId = c.block_id
        wordSize = c.word_size
        wordCount = c.word_count
        headerOk = c.header_ok
        dataOk = c.data_ok
    }
}

/// The fields of AWS beside the link layers.
public struct AwsFields {
    /// Scans of the radiometer.
    public var lines: Int64
    /// Science packets.
    public var sciencePackets: Int64
    /// Packets whose error control field was wrong.
    public var packetCrcErrors: Int64
    /// Scan records that were not 7599 bytes long.
    public var oddRecords: Int64
    /// Idle frames.
    public var idleFrames: Int64
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_aws_fields) {
        lines = c.lines
        sciencePackets = c.science_packets
        packetCrcErrors = c.packet_crc_errors
        oddRecords = c.odd_records
        idleFrames = c.idle_frames
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AwsFields, SDRKitCode> {
        var c = sdrkit_aws_fields()
        let code = sdrkit_aws_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AwsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The frames of one virtual channel of AWS. The row key is the virtual channel.
public struct AwsVirtualChannelRow {
    /// The row key.
    public var key: String
    /// A virtual channel.
    public var vcid: Int64
    /// The frames of a virtual channel.
    public var frames: Int64

    init(_ c: sdrkit_aws_virtual_channel_row) {
        key = sdrkitText(c.key)
        vcid = c.vcid
        frames = c.frames
    }
}

/// The packets of one application process of AWS. The row key is the APID.
public struct AwsApidRow {
    /// The row key.
    public var key: String
    /// An application process.
    public var apid: Int64
    /// The packets of an application process.
    public var packets: Int64

    init(_ c: sdrkit_aws_apid_row) {
        key = sdrkitText(c.key)
        apid = c.apid
        packets = c.packets
    }
}

/// The fields of NOAA GAC.
public struct NoaaGacFields {
    /// The Es/N0 of the demodulator, dB.
    public var snrDb: Double?
    /// The level of the input, dBFS.
    public var levelDbfs: Double?
    /// The carrier offset: the energy location, the search and the loop.
    public var offsetHz: Double?
    /// The error of the symbol clock, ppm.
    public var clockPpm: Double?
    /// The nominal symbol rate.
    public var symbolRateHz: Double?
    /// Frames decoded.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The seconds of signal decoded.
    public var signalTime: Double
    /// The input rate.
    public var inputRateHz: Double?
    /// The carrier lock metric, 0 to 1.
    public var lockMetric: Double?
    /// A confirmed frame lock is held.
    public var frameLock: Bool
    /// The spacecraft of the last frame.
    public var spacecraft: Int64?
    /// The frame code of the identification word: 0 in a GAC frame.
    public var idCode: Int64?
    /// Sync words found.
    public var syncHits: Int64
    /// TIP minor frames.
    public var tipFrames: Int64
    /// AVHRR lines.
    public var lines: Int64
    /// HIRS lines.
    public var hirsLines: Int64
    /// The data arrive inverted.
    public var inverted: Bool
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_noaa_gac_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        signalTime = c.signal_time
        inputRateHz = c.has_input_rate_hz ? c.input_rate_hz : nil
        lockMetric = c.has_lock_metric ? c.lock_metric : nil
        frameLock = c.frame_lock
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        idCode = c.has_id_code ? c.id_code : nil
        syncHits = c.sync_hits
        tipFrames = c.tip_frames
        lines = c.lines
        hirsLines = c.hirs_lines
        inverted = c.inverted
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NoaaGacFields, SDRKitCode> {
        var c = sdrkit_noaa_gac_fields()
        let code = sdrkit_noaa_gac_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NoaaGacFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the raw Meteosat downlink beside the link layers.
public struct MsgRawFields {
    /// The first ten bits of the last frame, read as one number.
    public var satelliteNumber: Int64?
    /// Lines of part 0 stored.
    public var lines: Int64
    /// Rows of single channels written to the SEVIRI picture.
    public var channelRows: Int64
    /// Rows of the HRV picture.
    public var hrvRows: Int64
    /// SEVIRI packets taken.
    public var seviriPackets: Int64
    /// SEVIRI packets not placed: too short, or of a satellite number without clock numbers.
    public var packetsRefused: Int64
    /// Discs finished.
    public var discs: Int64
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_msg_raw_fields) {
        satelliteNumber = c.has_satellite_number ? c.satellite_number : nil
        lines = c.lines
        channelRows = c.channel_rows
        hrvRows = c.hrv_rows
        seviriPackets = c.seviri_packets
        packetsRefused = c.packets_refused
        discs = c.discs
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MsgRawFields, SDRKitCode> {
        var c = sdrkit_msg_raw_fields()
        let code = sdrkit_msg_raw_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MsgRawFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of every stack that lists frames: the link, the frame lock and the note.
public struct SatimgFrameTableFields {
    /// The Es/N0 of the demodulator, dB.
    public var snrDb: Double?
    /// The level of the input, dBFS.
    public var levelDbfs: Double?
    /// The carrier offset: the energy location, the search and the loop.
    public var offsetHz: Double?
    /// The error of the symbol clock, ppm.
    public var clockPpm: Double?
    /// The nominal symbol rate.
    public var symbolRateHz: Double?
    /// Frames decoded.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The seconds of signal decoded.
    public var signalTime: Double
    /// The input rate.
    public var inputRateHz: Double?
    /// The carrier lock metric, 0 to 1.
    public var lockMetric: Double?
    /// A confirmed frame lock is held.
    public var frameLock: Bool
    /// Sync words found.
    public var syncHits: Int64
    /// Reed-Solomon symbols corrected.
    public var bytesCorrected: Int64
    /// The spacecraft of a frame header.
    public var spacecraft: Int64?
    /// The virtual channel of a frame header.
    public var vcid: Int64?
    /// The QPSK stream in use, 0 to 3.
    public var stream: Int64?
    /// The data arrive inverted.
    public var inverted: Bool
    /// A note ("picture not decoded: ...").
    public var note: String

    init(_ c: sdrkit_satimg_frame_table_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        signalTime = c.signal_time
        inputRateHz = c.has_input_rate_hz ? c.input_rate_hz : nil
        lockMetric = c.has_lock_metric ? c.lock_metric : nil
        frameLock = c.frame_lock
        syncHits = c.sync_hits
        bytesCorrected = c.bytes_corrected
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        vcid = c.has_vcid ? c.vcid : nil
        stream = c.has_stream ? c.stream : nil
        inverted = c.inverted
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatimgFrameTableFields, SDRKitCode> {
        var c = sdrkit_satimg_frame_table_fields()
        let code = sdrkit_satimg_frame_table_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatimgFrameTableFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of a stack that lists frames. The row key is the place of the frame in the stream.
public struct SatimgFrameRow {
    /// The row key.
    public var key: String
    /// Signal time of a frame.
    public var time: Double
    /// The spacecraft of a frame header.
    public var spacecraft: Int64?
    /// The virtual channel of a frame header.
    public var vcid: Int64?
    /// Bytes of a frame with its sync word.
    public var bytes: Int64
    /// The check of a frame: good, check failed, or none.
    public var check: String
    /// The first 24 bytes of a frame after decoding.
    public var hex: String

    init(_ c: sdrkit_satimg_frame_row) {
        key = sdrkitText(c.key)
        time = c.time
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        vcid = c.has_vcid ? c.vcid : nil
        bytes = c.bytes
        check = sdrkitText(c.check)
        hex = sdrkitText(c.hex)
    }
}

/// The fields of the S-band CCSDS stack beside those of the frame table.
public struct SbandCcsdsFields {
    /// The name of the link.
    public var link: String
    /// The carrier is locked (also while frames decode).
    public var carrierLocked: Bool?
    /// The code synchroniser holds its lock.
    public var codeLocked: Bool?
    /// The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late.
    public var phaseAmbiguity: Int64?
    /// What the user can do (empty when all is well).
    public var hint: String

    init(_ c: sdrkit_sband_ccsds_fields) {
        link = sdrkitText(c.link)
        carrierLocked = c.has_carrier_locked ? c.carrier_locked : nil
        codeLocked = c.has_code_locked ? c.code_locked : nil
        phaseAmbiguity = c.has_phase_ambiguity ? c.phase_ambiguity : nil
        hint = sdrkitText(c.hint)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SbandCcsdsFields, SDRKitCode> {
        var c = sdrkit_sband_ccsds_fields()
        let code = sdrkit_sband_ccsds_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SbandCcsdsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of RDAS beside those of the frame table.
public struct RdasFields {
    /// The carrier is locked (also while frames decode).
    public var carrierLocked: Bool
    /// MSU-GS frames found.
    public var packets: Int64
    /// Infrared lines.
    public var linesIr: Int64
    /// Reduced visible lines.
    public var linesVisible: Int64
    /// Input samples let go by the gate.
    public var gateSkipped: Int64?
    /// The share of the input let go by the gate, percent.
    public var gateSkippedPct: Double?
    /// The height of the last gate line over the mean.
    public var gateLine: Double?
    /// What the user can do (empty when all is well).
    public var hint: String

    init(_ c: sdrkit_rdas_fields) {
        carrierLocked = c.carrier_locked
        packets = c.packets
        linesIr = c.lines_ir
        linesVisible = c.lines_visible
        gateSkipped = c.has_gate_skipped ? c.gate_skipped : nil
        gateSkippedPct = c.has_gate_skipped_pct ? c.gate_skipped_pct : nil
        gateLine = c.has_gate_line ? c.gate_line : nil
        hint = sdrkitText(c.hint)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RdasFields, SDRKitCode> {
        var c = sdrkit_rdas_fields()
        let code = sdrkit_rdas_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RdasFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the transfer frames in the data fields of a DVB-S2 carrier.
public struct SatimgCaduFields {
    /// CADUs found.
    public var cadus: Int64
    /// The spacecraft id of the frames. No value before a frame is read.
    public var spacecraft: Int64?
    /// The virtual channel of the last frame.
    public var vcid: Int64?
    /// The frames of each virtual channel, for people.
    public var framesPerVc: String
    /// Space packets.
    public var spacePackets: Int64
    /// Space packets lost.
    public var packetsLost: Int64
    /// Breaks in the stream of data fields.
    public var fieldBreaks: Int64

    init(_ c: sdrkit_satimg_cadu_fields) {
        cadus = c.cadus
        spacecraft = c.has_spacecraft ? c.spacecraft : nil
        vcid = c.has_vcid ? c.vcid : nil
        framesPerVc = sdrkitText(c.frames_per_vc)
        spacePackets = c.space_packets
        packetsLost = c.packets_lost
        fieldBreaks = c.field_breaks
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatimgCaduFields, SDRKitCode> {
        var c = sdrkit_satimg_cadu_fields()
        let code = sdrkit_satimg_cadu_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatimgCaduFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the file layer of `gk2a_uhrit`.
public struct Gk2aUhritFields {
    /// Frames with a bad CRC-16.
    public var framesCrcBad: Int64
    /// Packets with a bad CRC-32.
    public var packetsCrcBad: Int64
    /// Files.
    public var files: Int64
    /// The last file name.
    public var lastFile: String
    /// Products kept by the file layer. No value before the file layer reports.
    public var products: Int64?
    /// What the user can do.
    public var hint: String
    /// A note on the state of the stack.
    public var note: String

    init(_ c: sdrkit_gk2a_uhrit_fields) {
        framesCrcBad = c.frames_crc_bad
        packetsCrcBad = c.packets_crc_bad
        files = c.files
        lastFile = sdrkitText(c.last_file)
        products = c.has_products ? c.products : nil
        hint = sdrkitText(c.hint)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Gk2aUhritFields, SDRKitCode> {
        var c = sdrkit_gk2a_uhrit_fields()
        let code = sdrkit_gk2a_uhrit_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Gk2aUhritFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the packets of each APID of `gk2a_uhrit`. The row key is the APID.
public struct Gk2aUhritApidRow {
    /// The row key.
    public var key: String
    /// The APID.
    public var apid: Int64
    /// The packets of the APID.
    public var packets: Int64

    init(_ c: sdrkit_gk2a_uhrit_apid_row) {
        key = sdrkitText(c.key)
        apid = c.apid
        packets = c.packets
    }
}

/// A row of the frames of each virtual channel of `gk2a_uhrit`. The row key is the virtual channel.
public struct SatimgUhritFrameRow {
    /// The row key.
    public var key: String
    /// The virtual channel.
    public var vcid: Int64
    /// The frames of the virtual channel.
    public var frames: Int64

    init(_ c: sdrkit_satimg_uhrit_frame_row) {
        key = sdrkitText(c.key)
        vcid = c.vcid
        frames = c.frames
    }
}

/// The result fields of `fy4_lrit` and `fy4a_hrit` beside those of the DVB-S2 receiver. A count of a layer that has not reported has no value.
public struct SatimgFy4Fields {
    /// The name of the running link.
    public var link: String
    /// The carrier of the link: the standard, the MODCOD and the symbol rate.
    public var signal: String
    /// Transport packets.
    public var tsPackets: Int64
    /// Transport errors.
    public var tsErrors: Int64
    /// CADUs found in the transport stream or imported.
    public var cadus: Int64
    /// CADUs imported from a file.
    public var cadusImported: Int64
    /// Transfer frames (fill frames not counted).
    public var transferFrames: Int64?
    /// Fill frames.
    public var fillFrames: Int64?
    /// Gaps of the frame counts.
    public var frameGaps: Int64?
    /// Space packets.
    public var packets: Int64?
    /// Files.
    public var files: Int64?
    /// Files with errors.
    public var fileErrors: Int64?
    /// Encrypted files.
    public var encryptedFiles: Int64?
    /// Image segments.
    public var imageSegments: Int64?
    /// Pictures complete.
    public var picturesComplete: Int64?
    /// Products kept.
    public var products: Int64?
    /// What the user can do (empty when all is well).
    public var hint: String
    /// The seconds of signal decoded.
    public var signalTime: Double

    init(_ c: sdrkit_satimg_fy4_fields) {
        link = sdrkitText(c.link)
        signal = sdrkitText(c.signal)
        tsPackets = c.ts_packets
        tsErrors = c.ts_errors
        cadus = c.cadus
        cadusImported = c.cadus_imported
        transferFrames = c.has_transfer_frames ? c.transfer_frames : nil
        fillFrames = c.has_fill_frames ? c.fill_frames : nil
        frameGaps = c.has_frame_gaps ? c.frame_gaps : nil
        packets = c.has_packets ? c.packets : nil
        files = c.has_files ? c.files : nil
        fileErrors = c.has_file_errors ? c.file_errors : nil
        encryptedFiles = c.has_encrypted_files ? c.encrypted_files : nil
        imageSegments = c.has_image_segments ? c.image_segments : nil
        picturesComplete = c.has_pictures_complete ? c.pictures_complete : nil
        products = c.has_products ? c.products : nil
        hint = sdrkitText(c.hint)
        signalTime = c.signal_time
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatimgFy4Fields, SDRKitCode> {
        var c = sdrkit_satimg_fy4_fields()
        let code = sdrkit_satimg_fy4_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatimgFy4Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `goes_grb` beside those of the DVB-S2 receiver and the CADUs.
public struct GoesGrbFields {
    /// Packets with a bad CRC.
    public var packetsCrcBad: Int64
    /// Payloads.
    public var payloads: Int64
    /// Picture blocks.
    public var pictureBlocks: Int64
    /// JPEG 2000 failures.
    public var j2kFailures: Int64
    /// Files.
    public var files: Int64
    /// Rows of the picture shown.
    public var pictureLines: Int64
    /// The product shown.
    public var shownProduct: String
    /// What the user can do.
    public var hint: String
    /// A note on the state of the stack.
    public var note: String

    init(_ c: sdrkit_goes_grb_fields) {
        packetsCrcBad = c.packets_crc_bad
        payloads = c.payloads
        pictureBlocks = c.picture_blocks
        j2kFailures = c.j2k_failures
        files = c.files
        pictureLines = c.picture_lines
        shownProduct = sdrkitText(c.shown_product)
        hint = sdrkitText(c.hint)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GoesGrbFields, SDRKitCode> {
        var c = sdrkit_goes_grb_fields()
        let code = sdrkit_goes_grb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GoesGrbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The GLM lightning results of `goes_grb`.
public struct GoesGrbGlmFields {
    /// Complete GLM payloads (APIDs 300 to 303).
    public var glmPayloads: Int64
    /// GLM payloads dropped because a packet of them was lost or damaged.
    public var glmDropped: Int64
    /// Packets missing from the sequence counts of the GLM APIDs.
    public var glmPacketsLost: Int64
    /// Compressed GLM payloads, saved as files and not decoded.
    public var glmNotDecoded: Int64
    /// Flash and event units whose length does not match their count.
    public var glmBadLength: Int64
    /// Group units whose length does not match their count.
    public var glmGroupBadLength: Int64
    /// Flashes decoded.
    public var flashCount: Int64
    /// Groups decoded.
    public var groupCount: Int64
    /// Events counted.
    public var eventCount: Int64
    /// Flash and group records with an impossible value.
    public var glmRejected: Int64
    /// The time of the first event of the newest flash (UTC).
    public var newestFlash: Double?
    /// Flashes in the 60 s of flash time up to the newest flash.
    public var flashRate: Double

    init(_ c: sdrkit_goes_grb_glm_fields) {
        glmPayloads = c.glm_payloads
        glmDropped = c.glm_dropped
        glmPacketsLost = c.glm_packets_lost
        glmNotDecoded = c.glm_not_decoded
        glmBadLength = c.glm_bad_length
        glmGroupBadLength = c.glm_group_bad_length
        flashCount = c.flash_count
        groupCount = c.group_count
        eventCount = c.event_count
        glmRejected = c.glm_rejected
        newestFlash = c.has_newest_flash ? c.newest_flash : nil
        flashRate = c.flash_rate
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GoesGrbGlmFields, SDRKitCode> {
        var c = sdrkit_goes_grb_glm_fields()
        let code = sdrkit_goes_grb_glm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GoesGrbGlmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A picture product of `goes_grb`. The row key is the APID.
public struct GoesGrbProductRow {
    /// The row key.
    public var key: String
    /// The APID.
    public var apid: Int64
    /// The name of the product of the APID.
    public var name: String
    /// The signal time of the last update.
    public var updated: Double

    init(_ c: sdrkit_goes_grb_product_row) {
        key = sdrkitText(c.key)
        apid = c.apid
        name = sdrkitText(c.name)
        updated = c.updated
    }
}

/// A row of the packets of each APID of `goes_grb`. The row key is the APID.
public struct GoesGrbApidRow {
    /// The row key.
    public var key: String
    /// The APID.
    public var apid: Int64
    /// The name of the product of the APID.
    public var name: String
    /// The packets of the APID.
    public var packets: Int64

    init(_ c: sdrkit_goes_grb_apid_row) {
        key = sdrkitText(c.key)
        apid = c.apid
        name = sdrkitText(c.name)
        packets = c.packets
    }
}

/// A lightning flash of `goes_grb`. The row key is the product time (microseconds since J2000) and the flash id.
public struct GoesGrbGlmFlashRow {
    public enum Quality: Int32 {
        case good = 0
        case timeOrder = 1
        case eventCount = 2
        case duration = 3
    }
    /// The row key.
    public var key: String
    /// The product-unique flash id.
    public var flash: Int64
    /// The time of the first event (UTC).
    public var time: Double
    /// From the first to the last event.
    public var duration: Double
    /// The energy-weighted centroid, latitude first.
    public var position: SDRKitPosition
    /// The area covered.
    public var areaKm2: Double?
    /// The radiant energy.
    public var energyFj: Double?
    /// The data quality.
    public var quality: Quality?
    /// Groups received with this flash as parent.
    public var groups: Int64
    /// Events received in those groups.
    public var events: Int64

    init(_ c: sdrkit_goes_grb_glm_flash_row) {
        key = sdrkitText(c.key)
        flash = c.flash
        time = c.time
        duration = c.duration
        position = SDRKitPosition(c.position)
        areaKm2 = c.has_area_km2 ? c.area_km2 : nil
        energyFj = c.has_energy_fj ? c.energy_fj : nil
        quality = c.has_quality ? Quality(rawValue: c.quality) : nil
        groups = c.groups
        events = c.events
    }
}

/// A lightning group of `goes_grb`. The row key is the product time (microseconds since J2000) and the group id.
public struct GoesGrbGlmGroupRow {
    public enum Quality: Int32 {
        case good = 0
        case timeOrder = 1
        case eventCount = 2
        case duration = 3
    }
    /// The row key.
    public var key: String
    /// The product-unique group id.
    public var group: Int64
    /// The mean time of the events (UTC).
    public var time: Double
    /// The energy-weighted centroid, latitude first.
    public var position: SDRKitPosition
    /// The area covered.
    public var areaKm2: Double?
    /// The radiant energy.
    public var energyFj: Double?
    /// The parent flash id.
    public var flash: Int64
    /// The data quality.
    public var quality: Quality?
    /// Events received with this group as parent.
    public var events: Int64

    init(_ c: sdrkit_goes_grb_glm_group_row) {
        key = sdrkitText(c.key)
        group = c.group
        time = c.time
        position = SDRKitPosition(c.position)
        areaKm2 = c.has_area_km2 ? c.area_km2 : nil
        energyFj = c.has_energy_fj ? c.energy_fj : nil
        flash = c.flash
        quality = c.has_quality ? Quality(rawValue: c.quality) : nil
        events = c.events
    }
}

/// The result fields of `sat_filecast` beside those of the DVB-S2 receiver.
public struct SatFilecastFields {
    /// The PID read.
    public var pid: Int64
    /// Transport packets on the PID.
    public var pidPackets: Int64
    /// IP datagrams.
    public var datagrams: Int64
    /// Datagrams that are not UDP.
    public var notUdp: Int64
    /// File-broadcast packets.
    public var broadcastPackets: Int64
    /// File-broadcast packets refused.
    public var badPackets: Int64
    /// Complete files.
    public var filesComplete: Int64
    /// Incomplete files.
    public var filesIncomplete: Int64
    /// Files whose bzip2 data failed.
    public var filesFailed: Int64
    /// Files dropped (the queue was full).
    public var filesDropped: Int64
    /// Bytes of files.
    public var bytes: Int64
    /// Files open.
    public var openFiles: Int64
    /// The last error.
    public var lastError: String
    /// Pictures finished.
    public var picturesFinished: Int64
    /// What the user can do.
    public var hint: String
    /// A note on the state of the stack.
    public var note: String

    init(_ c: sdrkit_sat_filecast_fields) {
        pid = c.pid
        pidPackets = c.pid_packets
        datagrams = c.datagrams
        notUdp = c.not_udp
        broadcastPackets = c.broadcast_packets
        badPackets = c.bad_packets
        filesComplete = c.files_complete
        filesIncomplete = c.files_incomplete
        filesFailed = c.files_failed
        filesDropped = c.files_dropped
        bytes = c.bytes
        openFiles = c.open_files
        lastError = sdrkitText(c.last_error)
        picturesFinished = c.pictures_finished
        hint = sdrkitText(c.hint)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatFilecastFields, SDRKitCode> {
        var c = sdrkit_sat_filecast_fields()
        let code = sdrkit_sat_filecast_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatFilecastFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A file of `sat_filecast`. The row key is the number of the file.
public struct SatFilecastRecentFileRow {
    /// The row key.
    public var key: String
    /// The name of the file.
    public var name: String
    /// The size of the file.
    public var sizeField: Int64
    /// The parts of the file.
    public var parts: Int64
    /// The parts received.
    public var received: Int64
    /// Every part arrived.
    public var complete: Bool

    init(_ c: sdrkit_sat_filecast_recent_file_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        sizeField = c.size_
        parts = c.parts
        received = c.received
        complete = c.complete
    }
}

/// A picture of `sat_filecast`. The row key is the channel.
public struct SatFilecastPictureRow {
    /// The row key.
    public var key: String
    /// The channel.
    public var channel: String
    /// The time stamp of the picture.
    public var stamp: String
    /// The width of the picture.
    public var width: Int64
    /// The height of the picture.
    public var height: Int64
    /// The segments received.
    public var segments: Int64
    /// The segments in all.
    public var total: Int64

    init(_ c: sdrkit_sat_filecast_picture_row) {
        key = sdrkitText(c.key)
        channel = sdrkitText(c.channel)
        stamp = sdrkitText(c.stamp)
        width = c.width
        height = c.height
        segments = c.segments
        total = c.total
    }
}

/// The fields of `hrpt` beside the link layers.
public struct HrptFields {
    public enum Variant: Int32 {
        case metop = 0
        case fy3 = 1
        case noaa = 2
    }
    public enum VariantSetting: Int32 {
        case auto = 0
        case metop = 1
        case fy3 = 2
        case noaa = 3
    }
    /// The running variant.
    public var variant: Variant?
    /// The variant setting: automatic or fixed.
    public var variantSetting: VariantSetting?
    /// The code synchroniser undid a mirrored transform (the second output inverted). No value for NOAA.
    public var phaseAmbiguityMirrored: Bool?
    /// NOAA: the polarity plus 2 times the chip phase of the framer. No value for the other satellites.
    public var noaaRotation: Int64?
    /// AVHRR packets of APID 103.
    public var packetsApid103: Int64
    /// AVHRR packets of APID 104.
    public var packetsApid104: Int64
    /// Other packets.
    public var otherPackets: Int64
    /// NOAA minor frames.
    public var noaaFrames: Int64
    /// AVHRR lines decoded.
    public var avhrrLines: Int64
    /// Lines of 2048 pixels in the picture.
    public var pictureLines: Int64
    /// The on-board time of the last line, seconds of the day.
    public var onboardTime: Double?
    /// HIRS lines given out (NOAA).
    public var hirsLines: Int64
    /// HIRS elements put into a line.
    public var hirsValid: Int64
    /// HIRS elements without odd parity.
    public var hirsParityErrors: Int64

    init(_ c: sdrkit_hrpt_fields) {
        variant = Variant(rawValue: c.variant)
        variantSetting = VariantSetting(rawValue: c.variant_setting)
        phaseAmbiguityMirrored = c.has_phase_ambiguity_mirrored ? c.phase_ambiguity_mirrored : nil
        noaaRotation = c.has_noaa_rotation ? c.noaa_rotation : nil
        packetsApid103 = c.packets_apid_103
        packetsApid104 = c.packets_apid_104
        otherPackets = c.other_packets
        noaaFrames = c.noaa_frames
        avhrrLines = c.avhrr_lines
        pictureLines = c.picture_lines
        onboardTime = c.has_onboard_time ? c.onboard_time : nil
        hirsLines = c.hirs_lines
        hirsValid = c.hirs_valid
        hirsParityErrors = c.hirs_parity_errors
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HrptFields, SDRKitCode> {
        var c = sdrkit_hrpt_fields()
        let code = sdrkit_hrpt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HrptFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of EODB beside those of the frame table.
public struct EodbFields {
    /// The carrier is locked (also while frames decode).
    public var carrierLocked: Bool
    /// Packets.
    public var packets: Int64
    /// Space packets lost.
    public var packetsLost: Int64
    /// Space packets that went into a picture.
    public var instrumentPackets: Int64
    /// Picture lines.
    public var lines: Int64
    /// The satellite of the last good frame.
    public var satellite: String
    /// The code hypothesis that holds the lock.
    public var hypothesis: String
    /// The input rate is below the least rate of the link.
    public var rateTooLow: Bool
    /// CADUs of frame files taken.
    public var imported: Int64
    /// Input samples let go by the gate.
    public var gateSkipped: Int64?
    /// The share of the input let go by the gate, percent.
    public var gateSkippedPct: Double?
    /// The height of the last gate line over the mean.
    public var gateLine: Double?
    /// What the user can do (empty when all is well).
    public var hint: String

    init(_ c: sdrkit_eodb_fields) {
        carrierLocked = c.carrier_locked
        packets = c.packets
        packetsLost = c.packets_lost
        instrumentPackets = c.instrument_packets
        lines = c.lines
        satellite = sdrkitText(c.satellite)
        hypothesis = sdrkitText(c.hypothesis)
        rateTooLow = c.rate_too_low
        imported = c.imported
        gateSkipped = c.has_gate_skipped ? c.gate_skipped : nil
        gateSkippedPct = c.has_gate_skipped_pct ? c.gate_skipped_pct : nil
        gateLine = c.has_gate_line ? c.gate_line : nil
        hint = sdrkitText(c.hint)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EodbFields, SDRKitCode> {
        var c = sdrkit_eodb_fields()
        let code = sdrkit_eodb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EodbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A product that changed.
public struct SatimgProductEvent {
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
        case adminMessage = 0
        case dcs = 1
        case emwin = 2
        case mesoscale = 3
        case fullDisk = 4
        case conus = 5
        case image = 6
        case text = 7
        case other = 8
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Where the product comes from: its name.
    public var source: String
    /// The text: the name of a product, the text of a text product.
    public var text: String
    /// The family of the source: a satellite.
    public var family: Family?
    /// The product id.
    public var id: Int64
    /// The kind of a product.
    public var kind: Kind?
    /// The name of a product.
    public var name: String
    /// The virtual channel of a product.
    public var vcid: Int64
    /// The APID of a product.
    public var apid: Int64
    /// Rows received.
    public var rows: Int64
    /// The picture width.
    public var width: Int64
    /// The picture height.
    public var height: Int64
    /// Segments received.
    public var segments: Int64
    /// Segments in all.
    public var segmentsTotal: Int64
    /// Every segment arrived.
    public var complete: Bool
    /// A file of the product had errors.
    public var errors: Bool
    /// Grows with every change of the product.
    public var version: Int64
    /// A note ("picture not decoded: ...").
    public var note: String?

    init(_ c: sdrkit_satimg_product_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        id = c.id
        kind = Kind(rawValue: c.kind)
        name = sdrkitText(c.name)
        vcid = c.vcid
        apid = c.apid
        rows = c.rows
        width = c.width
        height = c.height
        segments = c.segments
        segmentsTotal = c.segments_total
        complete = c.complete
        errors = c.errors
        version = c.version
        note = c.has_note ? sdrkitText(c.note) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SatimgProductEvent, SDRKitCode> {
        var c = sdrkit_satimg_product_event()
        let code = sdrkit_satimg_product_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SatimgProductEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A text product.
public struct SatimgTextEvent {
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
    /// Where the product comes from: its name.
    public var source: String
    /// The text: the name of a product, the text of a text product.
    public var text: String
    /// The family of the source: a satellite.
    public var family: Family?
    /// The name of a product.
    public var name: String
    /// The file name inside a ZIP archive.
    public var innerName: String?
    /// The product id.
    public var id: Int64

    init(_ c: sdrkit_satimg_text_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        name = sdrkitText(c.name)
        innerName = c.has_inner_name ? sdrkitText(c.inner_name) : nil
        id = c.id
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SatimgTextEvent, SDRKitCode> {
        var c = sdrkit_satimg_text_event()
        let code = sdrkit_satimg_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SatimgTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A GGAK telemetry frame of Elektro-L.
public struct SatimgGgakFrameEvent {
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
    /// The family of the source: a satellite.
    public var family: Family?
    /// The marker of a GGAK frame.
    public var marker: Int64
    /// The sub-frame of a GGAK frame.
    public var subframe: Int64
    /// The time stamp of a GGAK frame.
    public var timestamp: Int64
    /// The CRC of a GGAK frame is right.
    public var crcOk: Bool
    /// The polarity of a GGAK frame was inverted.
    public var inverted: Bool
    /// Bx of the first triple of a GGAK frame.
    public var bx: Int64
    /// By of the first triple of a GGAK frame.
    public var by: Int64
    /// Bz of the first triple of a GGAK frame.
    public var bz: Int64

    init(_ c: sdrkit_satimg_ggak_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
        marker = c.marker
        subframe = c.subframe
        timestamp = c.timestamp
        crcOk = c.crc_ok
        inverted = c.inverted
        bx = c.bx
        by = c.by
        bz = c.bz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SatimgGgakFrameEvent, SDRKitCode> {
        var c = sdrkit_satimg_ggak_frame_event()
        let code = sdrkit_satimg_ggak_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SatimgGgakFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum SatimgComponents {
    /// The result fields of the component "s2_report" of the stack "gk2a_uhrit" in the snapshot of `handle`.
    public static func gk2aUhritS2ReportDvbs2Fields(_ handle: SDRKIT_HANDLE) -> Result<Dvbs2Fields, SDRKitCode> {
        var c = sdrkit_dvbs2_fields()
        let code = sdrkit_gk2a_uhrit_s2_report_dvbs2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbs2Fields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_report" of the stack "fy4_lrit" in the snapshot of `handle`.
    public static func fy4LritS2ReportDvbs2Fields(_ handle: SDRKIT_HANDLE) -> Result<Dvbs2Fields, SDRKitCode> {
        var c = sdrkit_dvbs2_fields()
        let code = sdrkit_fy4_lrit_s2_report_dvbs2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbs2Fields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_bbframe" of the stack "fy4_lrit" in the snapshot of `handle`.
    public static func fy4LritS2BbframeBbFrameFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastBbFrameFields, SDRKitCode> {
        var c = sdrkit_ipcast_bb_frame_fields()
        let code = sdrkit_fy4_lrit_s2_bbframe_bb_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastBbFrameFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_report" of the stack "fy4a_hrit" in the snapshot of `handle`.
    public static func fy4aHritS2ReportDvbs2Fields(_ handle: SDRKIT_HANDLE) -> Result<Dvbs2Fields, SDRKitCode> {
        var c = sdrkit_dvbs2_fields()
        let code = sdrkit_fy4a_hrit_s2_report_dvbs2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbs2Fields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_bbframe" of the stack "fy4a_hrit" in the snapshot of `handle`.
    public static func fy4aHritS2BbframeBbFrameFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastBbFrameFields, SDRKitCode> {
        var c = sdrkit_ipcast_bb_frame_fields()
        let code = sdrkit_fy4a_hrit_s2_bbframe_bb_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastBbFrameFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_report" of the stack "goes_grb" in the snapshot of `handle`.
    public static func goesGrbS2ReportDvbs2Fields(_ handle: SDRKIT_HANDLE) -> Result<Dvbs2Fields, SDRKitCode> {
        var c = sdrkit_dvbs2_fields()
        let code = sdrkit_goes_grb_s2_report_dvbs2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbs2Fields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_report" of the stack "sat_filecast" in the snapshot of `handle`.
    public static func satFilecastS2ReportDvbs2Fields(_ handle: SDRKIT_HANDLE) -> Result<Dvbs2Fields, SDRKitCode> {
        var c = sdrkit_dvbs2_fields()
        let code = sdrkit_sat_filecast_s2_report_dvbs2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbs2Fields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "s2_bbframe" of the stack "sat_filecast" in the snapshot of `handle`.
    public static func satFilecastS2BbframeBbFrameFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastBbFrameFields, SDRKitCode> {
        var c = sdrkit_ipcast_bb_frame_fields()
        let code = sdrkit_sat_filecast_s2_bbframe_bb_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastBbFrameFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum SatimgTables {
    /// The changed rows of the table "channels" of the stack "meteor_lrpt", newest first, at most `capacity` rows.
    public static func meteorLrptChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[MeteorLrptChannelRow], SDRKitCode> {
        var rows = [sdrkit_meteor_lrpt_channel_row](repeating: sdrkit_meteor_lrpt_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meteor_lrpt_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeteorLrptChannelRow.init))
    }
    /// The changed rows of the table "images" of the stack "meteor_lrpt", newest first, at most `capacity` rows.
    public static func meteorLrptImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meteor_lrpt_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "goes_hrit", newest first, at most `capacity` rows.
    public static func goesHritVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_hrit_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "products" of the stack "goes_hrit", newest first, at most `capacity` rows.
    public static func goesHritProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_hrit_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "goes_hrit", newest first, at most `capacity` rows.
    public static func goesHritImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_hrit_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "gk2a_lrit", newest first, at most `capacity` rows.
    public static func gk2aLritVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_lrit_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "products" of the stack "gk2a_lrit", newest first, at most `capacity` rows.
    public static func gk2aLritProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_lrit_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "gk2a_lrit", newest first, at most `capacity` rows.
    public static func gk2aLritImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_lrit_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "gk2a_hrit", newest first, at most `capacity` rows.
    public static func gk2aHritVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_hrit_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "products" of the stack "gk2a_hrit", newest first, at most `capacity` rows.
    public static func gk2aHritProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_hrit_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "gk2a_hrit", newest first, at most `capacity` rows.
    public static func gk2aHritImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_hrit_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "images" of the stack "fy2_svissr", newest first, at most `capacity` rows.
    public static func fy2SvissrImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy2_svissr_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "instruments" of the stack "fy3_ahrpt", newest first, at most `capacity` rows.
    public static func fy3AhrptInstruments(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[SatimgFy3InstrumentRow], SDRKitCode> {
        var rows = [sdrkit_satimg_fy3_instrument_row](repeating: sdrkit_satimg_fy3_instrument_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy3_ahrpt_instruments_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFy3InstrumentRow.init))
    }
    /// The changed rows of the table "images" of the stack "fy3_ahrpt", newest first, at most `capacity` rows.
    public static func fy3AhrptImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy3_ahrpt_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "instruments" of the stack "fy3_mpt", newest first, at most `capacity` rows.
    public static func fy3MptInstruments(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[SatimgFy3InstrumentRow], SDRKitCode> {
        var rows = [sdrkit_satimg_fy3_instrument_row](repeating: sdrkit_satimg_fy3_instrument_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy3_mpt_instruments_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFy3InstrumentRow.init))
    }
    /// The changed rows of the table "images" of the stack "fy3_mpt", newest first, at most `capacity` rows.
    public static func fy3MptImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy3_mpt_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "images" of the stack "noaa_apt", newest first, at most `capacity` rows.
    public static func noaaAptImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noaa_apt_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "images" of the stack "noaa_dsb", newest first, at most `capacity` rows.
    public static func noaaDsbImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noaa_dsb_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "blocks" of the stack "goes_gvar", newest first, at most `capacity` rows.
    public static func goesGvarBlocks(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[GoesGvarBlockRow], SDRKitCode> {
        var rows = [sdrkit_goes_gvar_block_row](repeating: sdrkit_goes_gvar_block_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_gvar_blocks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesGvarBlockRow.init))
    }
    /// The changed rows of the table "images" of the stack "goes_gvar", newest first, at most `capacity` rows.
    public static func goesGvarImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_gvar_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "aws", newest first, at most `capacity` rows.
    public static func awsVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[AwsVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_aws_virtual_channel_row](repeating: sdrkit_aws_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aws_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AwsVirtualChannelRow.init))
    }
    /// The changed rows of the table "apids" of the stack "aws", newest first, at most `capacity` rows.
    public static func awsApids(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[AwsApidRow], SDRKitCode> {
        var rows = [sdrkit_aws_apid_row](repeating: sdrkit_aws_apid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aws_apids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AwsApidRow.init))
    }
    /// The changed rows of the table "images" of the stack "aws", newest first, at most `capacity` rows.
    public static func awsImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aws_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "images" of the stack "noaa_gac", newest first, at most `capacity` rows.
    public static func noaaGacImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noaa_gac_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "msg_raw", newest first, at most `capacity` rows.
    public static func msgRawVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msg_raw_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "images" of the stack "msg_raw", newest first, at most `capacity` rows.
    public static func msgRawImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_msg_raw_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "frames" of the stack "goes_cda", newest first, at most `capacity` rows.
    public static func goesCdaFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_frame_row](repeating: sdrkit_satimg_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_cda_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFrameRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "goes_cda", newest first, at most `capacity` rows.
    public static func goesCdaVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_cda_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "frames" of the stack "goes_mdl", newest first, at most `capacity` rows.
    public static func goesMdlFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_frame_row](repeating: sdrkit_satimg_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_mdl_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFrameRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "goes_mdl", newest first, at most `capacity` rows.
    public static func goesMdlVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_mdl_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "frames" of the stack "gk2a_cdas", newest first, at most `capacity` rows.
    public static func gk2aCdasFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_frame_row](repeating: sdrkit_satimg_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_cdas_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFrameRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "gk2a_cdas", newest first, at most `capacity` rows.
    public static func gk2aCdasVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_cdas_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "frames" of the stack "sband_ccsds", newest first, at most `capacity` rows.
    public static func sbandCcsdsFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_frame_row](repeating: sdrkit_satimg_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sband_ccsds_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFrameRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "sband_ccsds", newest first, at most `capacity` rows.
    public static func sbandCcsdsVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sband_ccsds_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "frames" of the stack "rdas", newest first, at most `capacity` rows.
    public static func rdasFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_frame_row](repeating: sdrkit_satimg_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rdas_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFrameRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "rdas", newest first, at most `capacity` rows.
    public static func rdasVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rdas_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Dvbs2CarrierRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_carrier_row](repeating: sdrkit_dvbs2_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2CarrierRow.init))
    }
    /// The changed rows of the table "streams" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbs2StreamRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_row](repeating: sdrkit_dvbs2_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamRow.init))
    }
    /// The changed rows of the table "stream_modcods" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritStreamModcods(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbs2StreamModcodRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_modcod_row](repeating: sdrkit_dvbs2_stream_modcod_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_stream_modcods_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamModcodRow.init))
    }
    /// The changed rows of the table "packets_by_apid" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritPacketsByApid(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Gk2aUhritApidRow], SDRKitCode> {
        var rows = [sdrkit_gk2a_uhrit_apid_row](repeating: sdrkit_gk2a_uhrit_apid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_packets_by_apid_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Gk2aUhritApidRow.init))
    }
    /// The changed rows of the table "frames" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgUhritFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_uhrit_frame_row](repeating: sdrkit_satimg_uhrit_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgUhritFrameRow.init))
    }
    /// The changed rows of the table "products" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "gk2a_uhrit", newest first, at most `capacity` rows.
    public static func gk2aUhritImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gk2a_uhrit_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "fy4_lrit", newest first, at most `capacity` rows.
    public static func fy4LritCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Dvbs2CarrierRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_carrier_row](repeating: sdrkit_dvbs2_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4_lrit_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2CarrierRow.init))
    }
    /// The changed rows of the table "streams" of the stack "fy4_lrit", newest first, at most `capacity` rows.
    public static func fy4LritStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbs2StreamRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_row](repeating: sdrkit_dvbs2_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4_lrit_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamRow.init))
    }
    /// The changed rows of the table "stream_modcods" of the stack "fy4_lrit", newest first, at most `capacity` rows.
    public static func fy4LritStreamModcods(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbs2StreamModcodRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_modcod_row](repeating: sdrkit_dvbs2_stream_modcod_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4_lrit_stream_modcods_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamModcodRow.init))
    }
    /// The changed rows of the table "products" of the stack "fy4_lrit", newest first, at most `capacity` rows.
    public static func fy4LritProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4_lrit_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "fy4_lrit", newest first, at most `capacity` rows.
    public static func fy4LritImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4_lrit_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "fy4a_hrit", newest first, at most `capacity` rows.
    public static func fy4aHritCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Dvbs2CarrierRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_carrier_row](repeating: sdrkit_dvbs2_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4a_hrit_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2CarrierRow.init))
    }
    /// The changed rows of the table "streams" of the stack "fy4a_hrit", newest first, at most `capacity` rows.
    public static func fy4aHritStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbs2StreamRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_row](repeating: sdrkit_dvbs2_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4a_hrit_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamRow.init))
    }
    /// The changed rows of the table "stream_modcods" of the stack "fy4a_hrit", newest first, at most `capacity` rows.
    public static func fy4aHritStreamModcods(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbs2StreamModcodRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_modcod_row](repeating: sdrkit_dvbs2_stream_modcod_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4a_hrit_stream_modcods_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamModcodRow.init))
    }
    /// The changed rows of the table "products" of the stack "fy4a_hrit", newest first, at most `capacity` rows.
    public static func fy4aHritProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4a_hrit_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "fy4a_hrit", newest first, at most `capacity` rows.
    public static func fy4aHritImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fy4a_hrit_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Dvbs2CarrierRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_carrier_row](repeating: sdrkit_dvbs2_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2CarrierRow.init))
    }
    /// The changed rows of the table "streams" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbs2StreamRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_row](repeating: sdrkit_dvbs2_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamRow.init))
    }
    /// The changed rows of the table "stream_modcods" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbStreamModcods(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbs2StreamModcodRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_modcod_row](repeating: sdrkit_dvbs2_stream_modcod_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_stream_modcods_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamModcodRow.init))
    }
    /// The changed rows of the table "grb_products" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[GoesGrbProductRow], SDRKitCode> {
        var rows = [sdrkit_goes_grb_product_row](repeating: sdrkit_goes_grb_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesGrbProductRow.init))
    }
    /// The changed rows of the table "packets_by_apid" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbPacketsByApid(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[GoesGrbApidRow], SDRKitCode> {
        var rows = [sdrkit_goes_grb_apid_row](repeating: sdrkit_goes_grb_apid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_packets_by_apid_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesGrbApidRow.init))
    }
    /// The changed rows of the table "frames" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgUhritFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_uhrit_frame_row](repeating: sdrkit_satimg_uhrit_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgUhritFrameRow.init))
    }
    /// The changed rows of the table "flashes" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbFlashes(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[GoesGrbGlmFlashRow], SDRKitCode> {
        var rows = [sdrkit_goes_grb_glm_flash_row](repeating: sdrkit_goes_grb_glm_flash_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_flashes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesGrbGlmFlashRow.init))
    }
    /// The changed rows of the table "groups" of the stack "goes_grb", newest first, at most `capacity` rows.
    public static func goesGrbGroups(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[GoesGrbGlmGroupRow], SDRKitCode> {
        var rows = [sdrkit_goes_grb_glm_group_row](repeating: sdrkit_goes_grb_glm_group_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_goes_grb_groups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GoesGrbGlmGroupRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "sat_filecast", newest first, at most `capacity` rows.
    public static func satFilecastCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Dvbs2CarrierRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_carrier_row](repeating: sdrkit_dvbs2_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_filecast_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2CarrierRow.init))
    }
    /// The changed rows of the table "streams" of the stack "sat_filecast", newest first, at most `capacity` rows.
    public static func satFilecastStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbs2StreamRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_row](repeating: sdrkit_dvbs2_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_filecast_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamRow.init))
    }
    /// The changed rows of the table "stream_modcods" of the stack "sat_filecast", newest first, at most `capacity` rows.
    public static func satFilecastStreamModcods(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbs2StreamModcodRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_modcod_row](repeating: sdrkit_dvbs2_stream_modcod_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_filecast_stream_modcods_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamModcodRow.init))
    }
    /// The changed rows of the table "recent_files" of the stack "sat_filecast", newest first, at most `capacity` rows.
    public static func satFilecastRecentFiles(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[SatFilecastRecentFileRow], SDRKitCode> {
        var rows = [sdrkit_sat_filecast_recent_file_row](repeating: sdrkit_sat_filecast_recent_file_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_filecast_recent_files_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatFilecastRecentFileRow.init))
    }
    /// The changed rows of the table "pictures" of the stack "sat_filecast", newest first, at most `capacity` rows.
    public static func satFilecastPictures(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatFilecastPictureRow], SDRKitCode> {
        var rows = [sdrkit_sat_filecast_picture_row](repeating: sdrkit_sat_filecast_picture_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sat_filecast_pictures_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatFilecastPictureRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "elektro_l", newest first, at most `capacity` rows.
    public static func elektroLVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_elektro_l_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "products" of the stack "elektro_l", newest first, at most `capacity` rows.
    public static func elektroLProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgProductRow], SDRKitCode> {
        var rows = [sdrkit_satimg_product_row](repeating: sdrkit_satimg_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_elektro_l_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgProductRow.init))
    }
    /// The changed rows of the table "images" of the stack "elektro_l", newest first, at most `capacity` rows.
    public static func elektroLImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_elektro_l_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "hrpt", newest first, at most `capacity` rows.
    public static func hrptVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hrpt_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
    /// The changed rows of the table "images" of the stack "hrpt", newest first, at most `capacity` rows.
    public static func hrptImages(_ handle: SDRKIT_HANDLE, capacity: Int = 80) -> Result<[SatimgImageRow], SDRKitCode> {
        var rows = [sdrkit_satimg_image_row](repeating: sdrkit_satimg_image_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hrpt_images_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgImageRow.init))
    }
    /// The changed rows of the table "frames" of the stack "eodb", newest first, at most `capacity` rows.
    public static func eodbFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgFrameRow], SDRKitCode> {
        var rows = [sdrkit_satimg_frame_row](repeating: sdrkit_satimg_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_eodb_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgFrameRow.init))
    }
    /// The changed rows of the table "virtual_channels" of the stack "eodb", newest first, at most `capacity` rows.
    public static func eodbVirtualChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[SatimgVirtualChannelRow], SDRKitCode> {
        var rows = [sdrkit_satimg_virtual_channel_row](repeating: sdrkit_satimg_virtual_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_eodb_virtual_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatimgVirtualChannelRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum SatimgCommands {
    /// New picture. Starts a new picture.
    public static func meteorLrptClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meteor_lrpt_clear(handle)
        return sdrkitResult(code)
    }
    /// Open CADU. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
    public static func goesHritOpenCadu(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_goes_hrit_open_cadu(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the newest frame.
    public static func goesHritClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_goes_hrit_clear(handle)
        return sdrkitResult(code)
    }
    /// Open CADU. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
    public static func gk2aLritOpenCadu(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_gk2a_lrit_open_cadu(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the newest frame.
    public static func gk2aLritClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gk2a_lrit_clear(handle)
        return sdrkitResult(code)
    }
    /// Import keys. Reads a KMA key file.
    public static func gk2aLritImportKeys(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_gk2a_lrit_import_keys(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Add keys. Adds keys typed as text.
    public static func gk2aLritAddKeys(_ handle: SDRKIT_HANDLE, keys: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_keys_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_keys_args>.size)
        let code: Int32 = sdrkitWithString(keys) { keys_ in { () -> Int32 in
        args.keys = keys_
        return sdrkit_gk2a_lrit_add_keys(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Open CADU. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
    public static func gk2aHritOpenCadu(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_gk2a_hrit_open_cadu(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the newest frame.
    public static func gk2aHritClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gk2a_hrit_clear(handle)
        return sdrkitResult(code)
    }
    /// Import keys. Reads a KMA key file.
    public static func gk2aHritImportKeys(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_gk2a_hrit_import_keys(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Add keys. Adds keys typed as text.
    public static func gk2aHritAddKeys(_ handle: SDRKIT_HANDLE, keys: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_keys_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_keys_args>.size)
        let code: Int32 = sdrkitWithString(keys) { keys_ in { () -> Int32 in
        args.keys = keys_
        return sdrkit_gk2a_hrit_add_keys(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Open .svissr. Decodes a file of clear 44 356-byte frames at the next input.
    public static func fy2SvissrOpenSvissr(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_fy2_svissr_open_svissr(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// New picture. Forgets every picture and starts new ones.
    public static func fy2SvissrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fy2_svissr_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func fy3AhrptClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fy3_ahrpt_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func fy3MptClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fy3_mpt_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func noaaAptClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noaa_apt_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func noaaDsbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noaa_dsb_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func goesGvarClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_goes_gvar_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func awsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_aws_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func noaaGacClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noaa_gac_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func msgRawClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_msg_raw_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Empties the frame table and the counters; the synchronisation stays.
    public static func goesCdaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_goes_cda_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Empties the frame table and the counters; the synchronisation stays.
    public static func goesMdlClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_goes_mdl_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Empties the frame table and the counters; the synchronisation stays.
    public static func gk2aCdasClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gk2a_cdas_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Empties the frame table and the counters; the synchronisation stays.
    public static func sbandCcsdsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sband_ccsds_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Empties the frame table and the counters; the synchronisation stays.
    public static func rdasClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rdas_clear(handle)
        return sdrkitResult(code)
    }
    /// Import keys. Reads a KMA key file.
    public static func gk2aUhritImportKeys(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_gk2a_uhrit_import_keys(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Add keys. Adds keys typed as text.
    public static func gk2aUhritAddKeys(_ handle: SDRKIT_HANDLE, keys: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_keys_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_keys_args>.size)
        let code: Int32 = sdrkitWithString(keys) { keys_ in { () -> Int32 in
        args.keys = keys_
        return sdrkit_gk2a_uhrit_add_keys(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets every product.
    public static func gk2aUhritClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gk2a_uhrit_clear(handle)
        return sdrkitResult(code)
    }
    /// Open CADU. Decodes a file of 1024-byte CADUs at the next input.
    public static func fy4LritOpenCadu(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_fy4_lrit_open_cadu(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets every product.
    public static func fy4LritClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fy4_lrit_clear(handle)
        return sdrkitResult(code)
    }
    /// Open CADU. Decodes a file of 1024-byte CADUs at the next input.
    public static func fy4aHritOpenCadu(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_fy4a_hrit_open_cadu(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets every product.
    public static func fy4aHritClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fy4a_hrit_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the pictures, the payloads and the counters.
    public static func goesGrbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_goes_grb_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the open files and the counters; keeps the pictures.
    public static func satFilecastClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sat_filecast_clear(handle)
        return sdrkitResult(code)
    }
    /// Open CADU. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
    public static func elektroLOpenCadu(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_elektro_l_open_cadu(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the newest frame.
    public static func elektroLClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_elektro_l_clear(handle)
        return sdrkitResult(code)
    }
    /// New picture. Starts a new picture.
    public static func hrptClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hrpt_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Empties the frame table and the counters; the synchronisation stays.
    public static func eodbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_eodb_clear(handle)
        return sdrkitResult(code)
    }
    /// Open frame file. Decodes a file of CADUs of the link's length, each starting with the marker 1ACFFC1D.
    public static func eodbOpenFrames(_ handle: SDRKIT_HANDLE, path: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_satimg_file_args()
        args.size = UInt32(MemoryLayout<sdrkit_satimg_file_args>.size)
        let code: Int32 = sdrkitWithString(path) { path_ in { () -> Int32 in
        args.path = path_
        return sdrkit_eodb_open_frames(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Make product. Makes a colour product of the channel pictures.
    public static func eodbProduct(_ handle: SDRKIT_HANDLE, product: String, width: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_eodb_product_args()
        args.size = UInt32(MemoryLayout<sdrkit_eodb_product_args>.size)
        let code: Int32 = sdrkitWithString(product) { product_ in { () -> Int32 in
        args.product = product_
        args.has_width = width != nil
        if let given = width { args.width = given }
        return sdrkit_eodb_product(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum SatimgSettings {
    /// The options of the setting "frequency_preset" of the stack "meteor_lrpt".
    public enum MeteorLrptFrequencyPreset: Int32 {
        case n137900 = 0
        case n137100 = 1
    }
    /// Frequency.
    public static func meteorLrptFrequencyPreset(_ handle: SDRKIT_HANDLE) -> Result<MeteorLrptFrequencyPreset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frequency_preset", &v)
        return code == SDRKIT_OK ? .success(MeteorLrptFrequencyPreset(rawValue: v) ?? MeteorLrptFrequencyPreset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptFrequencyPreset(_ handle: SDRKIT_HANDLE, _ value: MeteorLrptFrequencyPreset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frequency_preset", value.rawValue))
    }
    /// The options of the setting "modulation" of the stack "meteor_lrpt".
    public enum MeteorLrptModulation: Int32 {
        case auto = 0
        case oqpsk72 = 1
        case qpsk72 = 2
        case oqpsk80 = 3
    }
    /// Modulation.
    public static func meteorLrptModulation(_ handle: SDRKIT_HANDLE) -> Result<MeteorLrptModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modulation", &v)
        return code == SDRKIT_OK ? .success(MeteorLrptModulation(rawValue: v) ?? MeteorLrptModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptModulation(_ handle: SDRKIT_HANDLE, _ value: MeteorLrptModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modulation", value.rawValue))
    }
    /// The options of the setting "view" of the stack "meteor_lrpt".
    public enum MeteorLrptView: Int32 {
        case auto = 0
        case rgb123 = 1
        case rgb125 = 2
        case ch1 = 3
        case ch2 = 4
        case ch3 = 5
        case ch4 = 6
        case ch5 = 7
        case ch6 = 8
    }
    /// View.
    public static func meteorLrptView(_ handle: SDRKIT_HANDLE) -> Result<MeteorLrptView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(MeteorLrptView(rawValue: v) ?? MeteorLrptView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptView(_ handle: SDRKIT_HANDLE, _ value: MeteorLrptView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// The options of the setting "enhance" of the stack "meteor_lrpt".
    public enum MeteorLrptEnhance: Int32 {
        case none = 0
        case stretch = 1
        case brighten = 2
        case thermal = 3
        case satellite = 4
    }
    /// Enhancement.
    public static func meteorLrptEnhance(_ handle: SDRKIT_HANDLE) -> Result<MeteorLrptEnhance, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "enhance", &v)
        return code == SDRKIT_OK ? .success(MeteorLrptEnhance(rawValue: v) ?? MeteorLrptEnhance(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptEnhance(_ handle: SDRKIT_HANDLE, _ value: MeteorLrptEnhance) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "enhance", value.rawValue))
    }
    /// Follow.
    public static func meteorLrptFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "filter" of the stack "goes_hrit".
    public enum GoesHritFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func goesHritFilter(_ handle: SDRKIT_HANDLE) -> Result<GoesHritFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(GoesHritFilter(rawValue: v) ?? GoesHritFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritFilter(_ handle: SDRKIT_HANDLE, _ value: GoesHritFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func goesHritFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// The options of the setting "filter" of the stack "gk2a_lrit".
    public enum Gk2aLritFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func gk2aLritFilter(_ handle: SDRKIT_HANDLE) -> Result<Gk2aLritFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(Gk2aLritFilter(rawValue: v) ?? Gk2aLritFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritFilter(_ handle: SDRKIT_HANDLE, _ value: Gk2aLritFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func gk2aLritFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// The options of the setting "filter" of the stack "gk2a_hrit".
    public enum Gk2aHritFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func gk2aHritFilter(_ handle: SDRKIT_HANDLE) -> Result<Gk2aHritFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(Gk2aHritFilter(rawValue: v) ?? Gk2aHritFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritFilter(_ handle: SDRKIT_HANDLE, _ value: Gk2aHritFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func gk2aHritFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// Symbol rate.
    public static func fy2SvissrSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy2SvissrSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// Newest.
    public static func fy2SvissrFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy2SvissrFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "preset" of the stack "fy3_ahrpt".
    public enum Fy3AhrptPreset: Int32 {
        case auto = 0
        case fy3c = 1
        case fy3ab = 2
        case fy3g = 3
        case fy3d = 4
        case fy3f = 5
        case fy3eh = 6
    }
    /// Satellite.
    public static func fy3AhrptPreset(_ handle: SDRKIT_HANDLE) -> Result<Fy3AhrptPreset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "preset", &v)
        return code == SDRKIT_OK ? .success(Fy3AhrptPreset(rawValue: v) ?? Fy3AhrptPreset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptPreset(_ handle: SDRKIT_HANDLE, _ value: Fy3AhrptPreset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "preset", value.rawValue))
    }
    /// Symbol rate.
    public static func fy3AhrptSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// Roll-off.
    public static func fy3AhrptRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// The options of the setting "instrument" of the stack "fy3_ahrpt".
    public enum Fy3AhrptInstrument: Int32 {
        case vIRR = 0
        case mERSI = 1
        case mERSIII = 2
        case mERSI3 = 3
        case mERSILL = 4
        case mERSIRM = 5
        case mWRI = 6
        case mWRI2 = 7
    }
    /// Instrument.
    public static func fy3AhrptInstrument(_ handle: SDRKIT_HANDLE) -> Result<Fy3AhrptInstrument, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "instrument", &v)
        return code == SDRKIT_OK ? .success(Fy3AhrptInstrument(rawValue: v) ?? Fy3AhrptInstrument(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptInstrument(_ handle: SDRKIT_HANDLE, _ value: Fy3AhrptInstrument) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "instrument", value.rawValue))
    }
    /// The options of the setting "view" of the stack "fy3_ahrpt".
    public enum Fy3AhrptView: Int32 {
        case `true` = 0
        case `false` = 1
        case ch1 = 2
        case ch2 = 3
        case ch3 = 4
        case ch4 = 5
        case ch5 = 6
        case ch6 = 7
        case ch7 = 8
        case ch8 = 9
        case ch9 = 10
        case ch10 = 11
        case ch11 = 12
        case ch12 = 13
        case ch13 = 14
        case ch14 = 15
        case ch15 = 16
        case ch16 = 17
        case ch17 = 18
        case ch18 = 19
        case ch19 = 20
        case ch20 = 21
        case ch21 = 22
        case ch22 = 23
        case ch23 = 24
        case ch24 = 25
        case ch25 = 26
        case ch26 = 27
    }
    /// View.
    public static func fy3AhrptView(_ handle: SDRKIT_HANDLE) -> Result<Fy3AhrptView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(Fy3AhrptView(rawValue: v) ?? Fy3AhrptView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptView(_ handle: SDRKIT_HANDLE, _ value: Fy3AhrptView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func fy3AhrptFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "preset" of the stack "fy3_mpt".
    public enum Fy3MptPreset: Int32 {
        case auto = 0
        case mptAb = 1
        case mptC = 2
    }
    /// Satellite.
    public static func fy3MptPreset(_ handle: SDRKIT_HANDLE) -> Result<Fy3MptPreset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "preset", &v)
        return code == SDRKIT_OK ? .success(Fy3MptPreset(rawValue: v) ?? Fy3MptPreset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptPreset(_ handle: SDRKIT_HANDLE, _ value: Fy3MptPreset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "preset", value.rawValue))
    }
    /// Symbol rate.
    public static func fy3MptSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// Roll-off.
    public static func fy3MptRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// The options of the setting "instrument" of the stack "fy3_mpt".
    public enum Fy3MptInstrument: Int32 {
        case mERSI = 0
        case mERSIII = 1
        case mERSI3 = 2
        case mERSILL = 3
        case mERSIRM = 4
        case vIRR = 5
        case mWRI = 6
        case mWRI2 = 7
    }
    /// Instrument.
    public static func fy3MptInstrument(_ handle: SDRKIT_HANDLE) -> Result<Fy3MptInstrument, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "instrument", &v)
        return code == SDRKIT_OK ? .success(Fy3MptInstrument(rawValue: v) ?? Fy3MptInstrument(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptInstrument(_ handle: SDRKIT_HANDLE, _ value: Fy3MptInstrument) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "instrument", value.rawValue))
    }
    /// The options of the setting "view" of the stack "fy3_mpt".
    public enum Fy3MptView: Int32 {
        case `true` = 0
        case ch1 = 1
        case ch2 = 2
        case ch3 = 3
        case ch4 = 4
        case ch5 = 5
        case ch6 = 6
        case ch7 = 7
        case ch8 = 8
        case ch9 = 9
        case ch10 = 10
        case ch11 = 11
        case ch12 = 12
        case ch13 = 13
        case ch14 = 14
        case ch15 = 15
        case ch16 = 16
        case ch17 = 17
        case ch18 = 18
        case ch19 = 19
        case ch20 = 20
        case ch21 = 21
        case ch22 = 22
        case ch23 = 23
        case ch24 = 24
        case ch25 = 25
        case `false` = 26
        case ch26 = 27
    }
    /// View.
    public static func fy3MptView(_ handle: SDRKIT_HANDLE) -> Result<Fy3MptView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(Fy3MptView(rawValue: v) ?? Fy3MptView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptView(_ handle: SDRKIT_HANDLE, _ value: Fy3MptView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func fy3MptFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "picture" of the stack "noaa_apt".
    public enum NoaaAptPicture: Int32 {
        case aPT = 0
        case aPTChannelA = 1
        case aPTChannelB = 2
    }
    /// Picture.
    public static func noaaAptPicture(_ handle: SDRKIT_HANDLE) -> Result<NoaaAptPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "picture", &v)
        return code == SDRKIT_OK ? .success(NoaaAptPicture(rawValue: v) ?? NoaaAptPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaAptPicture(_ handle: SDRKIT_HANDLE, _ value: NoaaAptPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "picture", value.rawValue))
    }
    /// Follow.
    public static func noaaAptFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaAptFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Line filter.
    public static func noaaAptLineFilter(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "line_filter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaAptLineFilter(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "line_filter", value))
    }
    /// The options of the setting "view" of the stack "noaa_dsb".
    public enum NoaaDsbView: Int32 {
        case rgb214 = 0
        case ch1 = 1
        case ch2 = 2
        case ch3 = 3
        case ch4 = 4
        case ch5 = 5
        case ch6 = 6
        case ch7 = 7
        case ch8 = 8
        case ch9 = 9
        case ch10 = 10
        case ch11 = 11
        case ch12 = 12
        case ch13 = 13
        case ch14 = 14
        case ch15 = 15
        case ch16 = 16
        case ch17 = 17
        case ch18 = 18
        case ch19 = 19
        case ch20 = 20
    }
    /// View.
    public static func noaaDsbView(_ handle: SDRKIT_HANDLE) -> Result<NoaaDsbView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(NoaaDsbView(rawValue: v) ?? NoaaDsbView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaDsbView(_ handle: SDRKIT_HANDLE, _ value: NoaaDsbView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func noaaDsbFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaDsbFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "picture" of the stack "goes_gvar".
    public enum GoesGvarPicture: Int32 {
        case imagerVisible = 0
        case imagerInfrared = 1
    }
    /// Picture.
    public static func goesGvarPicture(_ handle: SDRKIT_HANDLE) -> Result<GoesGvarPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "picture", &v)
        return code == SDRKIT_OK ? .success(GoesGvarPicture(rawValue: v) ?? GoesGvarPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGvarPicture(_ handle: SDRKIT_HANDLE, _ value: GoesGvarPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "picture", value.rawValue))
    }
    /// The options of the setting "view" of the stack "goes_gvar".
    public enum GoesGvarView: Int32 {
        case visible = 0
        case ir2 = 1
        case ir3 = 2
        case ir4 = 3
        case ir6 = 4
    }
    /// View.
    public static func goesGvarView(_ handle: SDRKIT_HANDLE) -> Result<GoesGvarView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(GoesGvarView(rawValue: v) ?? GoesGvarView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGvarView(_ handle: SDRKIT_HANDLE, _ value: GoesGvarView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func goesGvarFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGvarFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "view" of the stack "aws".
    public enum AwsView: Int32 {
        case ch1 = 0
        case ch2 = 1
        case ch3 = 2
        case ch4 = 3
        case ch5 = 4
        case ch6 = 5
        case ch7 = 6
        case ch8 = 7
        case ch9 = 8
        case ch10 = 9
        case ch11 = 10
        case ch12 = 11
        case ch13 = 12
        case ch14 = 13
        case ch15 = 14
        case ch16 = 15
        case ch17 = 16
        case ch18 = 17
        case ch19 = 18
    }
    /// View.
    public static func awsView(_ handle: SDRKIT_HANDLE) -> Result<AwsView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(AwsView(rawValue: v) ?? AwsView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAwsView(_ handle: SDRKIT_HANDLE, _ value: AwsView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func awsFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAwsFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "picture" of the stack "noaa_gac".
    public enum NoaaGacPicture: Int32 {
        case aVHRRGAC = 0
        case hIRS = 1
    }
    /// Picture.
    public static func noaaGacPicture(_ handle: SDRKIT_HANDLE) -> Result<NoaaGacPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "picture", &v)
        return code == SDRKIT_OK ? .success(NoaaGacPicture(rawValue: v) ?? NoaaGacPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaGacPicture(_ handle: SDRKIT_HANDLE, _ value: NoaaGacPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "picture", value.rawValue))
    }
    /// The options of the setting "view" of the stack "noaa_gac".
    public enum NoaaGacView: Int32 {
        case rgb214 = 0
        case ch1 = 1
        case ch2 = 2
        case ch3 = 3
        case ch4 = 4
        case ch5 = 5
        case ch6 = 6
        case ch7 = 7
        case ch8 = 8
        case ch9 = 9
        case ch10 = 10
        case ch11 = 11
        case ch12 = 12
        case ch13 = 13
        case ch14 = 14
        case ch15 = 15
        case ch16 = 16
        case ch17 = 17
        case ch18 = 18
        case ch19 = 19
        case ch20 = 20
    }
    /// View.
    public static func noaaGacView(_ handle: SDRKIT_HANDLE) -> Result<NoaaGacView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(NoaaGacView(rawValue: v) ?? NoaaGacView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaGacView(_ handle: SDRKIT_HANDLE, _ value: NoaaGacView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func noaaGacFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaGacFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// The options of the setting "picture" of the stack "msg_raw".
    public enum MsgRawPicture: Int32 {
        case sEVIRI = 0
        case sEVIRIHRV = 1
    }
    /// Picture.
    public static func msgRawPicture(_ handle: SDRKIT_HANDLE) -> Result<MsgRawPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "picture", &v)
        return code == SDRKIT_OK ? .success(MsgRawPicture(rawValue: v) ?? MsgRawPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMsgRawPicture(_ handle: SDRKIT_HANDLE, _ value: MsgRawPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "picture", value.rawValue))
    }
    /// The options of the setting "view" of the stack "msg_raw".
    public enum MsgRawView: Int32 {
        case rgb214 = 0
        case ch1 = 1
        case ch2 = 2
        case ch3 = 3
        case ch4 = 4
        case ch5 = 5
        case ch6 = 6
        case ch7 = 7
        case ch8 = 8
        case ch9 = 9
        case ch10 = 10
        case ch11 = 11
        case hrv = 12
    }
    /// View.
    public static func msgRawView(_ handle: SDRKIT_HANDLE) -> Result<MsgRawView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(MsgRawView(rawValue: v) ?? MsgRawView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMsgRawView(_ handle: SDRKIT_HANDLE, _ value: MsgRawView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// Follow.
    public static func msgRawFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsgRawFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Link name.
    public static func sbandCcsdsLinkName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "link_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsLinkName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "link_name", $0) })
    }
    /// The options of the setting "modulation" of the stack "sband_ccsds".
    public enum SbandCcsdsModulation: Int32 {
        case bpsk = 0
        case qpsk = 1
        case oqpsk = 2
        case pm = 3
        case fsk = 4
    }
    /// Modulation.
    public static func sbandCcsdsModulation(_ handle: SDRKIT_HANDLE) -> Result<SbandCcsdsModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modulation", &v)
        return code == SDRKIT_OK ? .success(SbandCcsdsModulation(rawValue: v) ?? SbandCcsdsModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsModulation(_ handle: SDRKIT_HANDLE, _ value: SbandCcsdsModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modulation", value.rawValue))
    }
    /// Symbol rate.
    public static func sbandCcsdsSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// Matched filter roll-off.
    public static func sbandCcsdsRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// K = 7 code.
    public static func sbandCcsdsConv(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "conv", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsConv(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "conv", value))
    }
    /// Frame length.
    public static func sbandCcsdsFrameBits(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "frame_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsFrameBits(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "frame_bits", value))
    }
    /// Sync word.
    public static func sbandCcsdsSyncWord(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "sync_word", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsSyncWord(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "sync_word", $0) })
    }
    /// NRZ-M.
    public static func sbandCcsdsNrzm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "nrzm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsNrzm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "nrzm", value))
    }
    /// The options of the setting "randomiser" of the stack "sband_ccsds".
    public enum SbandCcsdsRandomiser: Int32 {
        case none = 0
        case frame = 1
        case inside = 2
    }
    /// CCSDS randomiser.
    public static func sbandCcsdsRandomiser(_ handle: SDRKIT_HANDLE) -> Result<SbandCcsdsRandomiser, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "randomiser", &v)
        return code == SDRKIT_OK ? .success(SbandCcsdsRandomiser(rawValue: v) ?? SbandCcsdsRandomiser(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsRandomiser(_ handle: SDRKIT_HANDLE, _ value: SbandCcsdsRandomiser) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "randomiser", value.rawValue))
    }
    /// The options of the setting "rs" of the stack "sband_ccsds".
    public enum SbandCcsdsRs: Int32 {
        case none = 0
        case code255223 = 1
        case code255239 = 2
    }
    /// Reed-Solomon.
    public static func sbandCcsdsRs(_ handle: SDRKIT_HANDLE) -> Result<SbandCcsdsRs, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rs", &v)
        return code == SDRKIT_OK ? .success(SbandCcsdsRs(rawValue: v) ?? SbandCcsdsRs(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsRs(_ handle: SDRKIT_HANDLE, _ value: SbandCcsdsRs) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rs", value.rawValue))
    }
    /// Interleave.
    public static func sbandCcsdsRsInterleave(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "rs_interleave", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsRsInterleave(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "rs_interleave", value))
    }
    /// The options of the setting "rs_basis" of the stack "sband_ccsds".
    public enum SbandCcsdsRsBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// Basis.
    public static func sbandCcsdsRsBasis(_ handle: SDRKIT_HANDLE) -> Result<SbandCcsdsRsBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "rs_basis", &v)
        return code == SDRKIT_OK ? .success(SbandCcsdsRsBasis(rawValue: v) ?? SbandCcsdsRsBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSbandCcsdsRsBasis(_ handle: SDRKIT_HANDLE, _ value: SbandCcsdsRsBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "rs_basis", value.rawValue))
    }
    /// Symbol rate.
    public static func rdasSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRdasSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// Matched filter roll-off.
    public static func rdasRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRdasRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// Symbol rate.
    public static func gk2aUhritSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "roll_off" of the stack "gk2a_uhrit".
    public enum Gk2aUhritRollOff: Int32 {
        case auto = 0
        case n035 = 1
        case n025 = 2
        case n020 = 3
        case n015 = 4
        case n010 = 5
        case n005 = 6
    }
    /// Roll-off.
    public static func gk2aUhritRollOff(_ handle: SDRKIT_HANDLE) -> Result<Gk2aUhritRollOff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(Gk2aUhritRollOff(rawValue: v) ?? Gk2aUhritRollOff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritRollOff(_ handle: SDRKIT_HANDLE, _ value: Gk2aUhritRollOff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "roll_off", value.rawValue))
    }
    /// Carrier.
    public static func gk2aUhritCarrierOffset(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "carrier_offset", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritCarrierOffset(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "carrier_offset", $0) })
    }
    /// Secondary header outside the length.
    public static func gk2aUhritUncountedSecondary(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "uncounted_secondary", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritUncountedSecondary(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "uncounted_secondary", value))
    }
    /// The options of the setting "filter" of the stack "gk2a_uhrit".
    public enum Gk2aUhritFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func gk2aUhritFilter(_ handle: SDRKIT_HANDLE) -> Result<Gk2aUhritFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(Gk2aUhritFilter(rawValue: v) ?? Gk2aUhritFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritFilter(_ handle: SDRKIT_HANDLE, _ value: Gk2aUhritFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func gk2aUhritFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// The options of the setting "modcod_rule" of the stack "gk2a_uhrit".
    public enum Gk2aUhritModcodRule: Int32 {
        case any = 0
        case fixed = 1
        case set = 2
    }
    /// MODCODs taken.
    public static func gk2aUhritModcodRule(_ handle: SDRKIT_HANDLE) -> Result<Gk2aUhritModcodRule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &v)
        return code == SDRKIT_OK ? .success(Gk2aUhritModcodRule(rawValue: v) ?? Gk2aUhritModcodRule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritModcodRule(_ handle: SDRKIT_HANDLE, _ value: Gk2aUhritModcodRule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modcod_rule", value.rawValue))
    }
    /// MODCOD.
    public static func gk2aUhritModcod(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "modcod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritModcod(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "modcod", value))
    }
    /// MODCOD set.
    public static func gk2aUhritModcods(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "modcods", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritModcods(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "modcods", $0) })
    }
    /// The options of the setting "frame_length" of the stack "gk2a_uhrit".
    public enum Gk2aUhritFrameLength: Int32 {
        case any = 0
        case normal = 1
        case short = 2
    }
    /// Frame length.
    public static func gk2aUhritFrameLength(_ handle: SDRKIT_HANDLE) -> Result<Gk2aUhritFrameLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_length", &v)
        return code == SDRKIT_OK ? .success(Gk2aUhritFrameLength(rawValue: v) ?? Gk2aUhritFrameLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritFrameLength(_ handle: SDRKIT_HANDLE, _ value: Gk2aUhritFrameLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_length", value.rawValue))
    }
    /// The options of the setting "pilots" of the stack "gk2a_uhrit".
    public enum Gk2aUhritPilots: Int32 {
        case any = 0
        case on = 1
        case off = 2
    }
    /// Pilots.
    public static func gk2aUhritPilots(_ handle: SDRKIT_HANDLE) -> Result<Gk2aUhritPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pilots", &v)
        return code == SDRKIT_OK ? .success(Gk2aUhritPilots(rawValue: v) ?? Gk2aUhritPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritPilots(_ handle: SDRKIT_HANDLE, _ value: Gk2aUhritPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pilots", value.rawValue))
    }
    /// PL scrambling code.
    public static func gk2aUhritPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aUhritPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value))
    }
    /// The options of the setting "satellite" of the stack "fy4_lrit".
    public enum Fy4LritSatellite: Int32 {
        case fy4a = 0
        case fy4b = 1
    }
    /// Satellite.
    public static func fy4LritSatellite(_ handle: SDRKIT_HANDLE) -> Result<Fy4LritSatellite, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "satellite", &v)
        return code == SDRKIT_OK ? .success(Fy4LritSatellite(rawValue: v) ?? Fy4LritSatellite(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritSatellite(_ handle: SDRKIT_HANDLE, _ value: Fy4LritSatellite) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "satellite", value.rawValue))
    }
    /// Symbol rate.
    public static func fy4LritSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "roll_off" of the stack "fy4_lrit".
    public enum Fy4LritRollOff: Int32 {
        case auto = 0
        case n035 = 1
        case n025 = 2
        case n020 = 3
        case n015 = 4
        case n010 = 5
        case n005 = 6
    }
    /// Roll-off.
    public static func fy4LritRollOff(_ handle: SDRKIT_HANDLE) -> Result<Fy4LritRollOff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(Fy4LritRollOff(rawValue: v) ?? Fy4LritRollOff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritRollOff(_ handle: SDRKIT_HANDLE, _ value: Fy4LritRollOff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "roll_off", value.rawValue))
    }
    /// Carrier.
    public static func fy4LritCarrierOffset(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "carrier_offset", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritCarrierOffset(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "carrier_offset", $0) })
    }
    /// Input stream.
    public static func fy4LritIsi(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "isi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritIsi(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "isi", value))
    }
    /// The options of the setting "modcod_rule" of the stack "fy4_lrit".
    public enum Fy4LritModcodRule: Int32 {
        case any = 0
        case fixed = 1
        case set = 2
    }
    /// MODCODs taken.
    public static func fy4LritModcodRule(_ handle: SDRKIT_HANDLE) -> Result<Fy4LritModcodRule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &v)
        return code == SDRKIT_OK ? .success(Fy4LritModcodRule(rawValue: v) ?? Fy4LritModcodRule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritModcodRule(_ handle: SDRKIT_HANDLE, _ value: Fy4LritModcodRule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modcod_rule", value.rawValue))
    }
    /// MODCOD.
    public static func fy4LritModcod(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "modcod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritModcod(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "modcod", value))
    }
    /// MODCOD set.
    public static func fy4LritModcods(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "modcods", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritModcods(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "modcods", $0) })
    }
    /// The options of the setting "frame_length" of the stack "fy4_lrit".
    public enum Fy4LritFrameLength: Int32 {
        case any = 0
        case normal = 1
        case short = 2
    }
    /// Frame length.
    public static func fy4LritFrameLength(_ handle: SDRKIT_HANDLE) -> Result<Fy4LritFrameLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_length", &v)
        return code == SDRKIT_OK ? .success(Fy4LritFrameLength(rawValue: v) ?? Fy4LritFrameLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritFrameLength(_ handle: SDRKIT_HANDLE, _ value: Fy4LritFrameLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_length", value.rawValue))
    }
    /// The options of the setting "pilots" of the stack "fy4_lrit".
    public enum Fy4LritPilots: Int32 {
        case any = 0
        case on = 1
        case off = 2
    }
    /// Pilots.
    public static func fy4LritPilots(_ handle: SDRKIT_HANDLE) -> Result<Fy4LritPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pilots", &v)
        return code == SDRKIT_OK ? .success(Fy4LritPilots(rawValue: v) ?? Fy4LritPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritPilots(_ handle: SDRKIT_HANDLE, _ value: Fy4LritPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pilots", value.rawValue))
    }
    /// PL scrambling code.
    public static func fy4LritPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value))
    }
    /// The options of the setting "filter" of the stack "fy4_lrit".
    public enum Fy4LritFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func fy4LritFilter(_ handle: SDRKIT_HANDLE) -> Result<Fy4LritFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(Fy4LritFilter(rawValue: v) ?? Fy4LritFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritFilter(_ handle: SDRKIT_HANDLE, _ value: Fy4LritFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func fy4LritFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4LritFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// Symbol rate.
    public static func fy4aHritSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "roll_off" of the stack "fy4a_hrit".
    public enum Fy4aHritRollOff: Int32 {
        case auto = 0
        case n035 = 1
        case n025 = 2
        case n020 = 3
        case n015 = 4
        case n010 = 5
        case n005 = 6
    }
    /// Roll-off.
    public static func fy4aHritRollOff(_ handle: SDRKIT_HANDLE) -> Result<Fy4aHritRollOff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(Fy4aHritRollOff(rawValue: v) ?? Fy4aHritRollOff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritRollOff(_ handle: SDRKIT_HANDLE, _ value: Fy4aHritRollOff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "roll_off", value.rawValue))
    }
    /// Carrier.
    public static func fy4aHritCarrierOffset(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "carrier_offset", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritCarrierOffset(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "carrier_offset", $0) })
    }
    /// Input stream.
    public static func fy4aHritIsi(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "isi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritIsi(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "isi", value))
    }
    /// The options of the setting "modcod_rule" of the stack "fy4a_hrit".
    public enum Fy4aHritModcodRule: Int32 {
        case any = 0
        case fixed = 1
        case set = 2
    }
    /// MODCODs taken.
    public static func fy4aHritModcodRule(_ handle: SDRKIT_HANDLE) -> Result<Fy4aHritModcodRule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &v)
        return code == SDRKIT_OK ? .success(Fy4aHritModcodRule(rawValue: v) ?? Fy4aHritModcodRule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritModcodRule(_ handle: SDRKIT_HANDLE, _ value: Fy4aHritModcodRule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modcod_rule", value.rawValue))
    }
    /// MODCOD.
    public static func fy4aHritModcod(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "modcod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritModcod(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "modcod", value))
    }
    /// MODCOD set.
    public static func fy4aHritModcods(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "modcods", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritModcods(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "modcods", $0) })
    }
    /// The options of the setting "frame_length" of the stack "fy4a_hrit".
    public enum Fy4aHritFrameLength: Int32 {
        case any = 0
        case normal = 1
        case short = 2
    }
    /// Frame length.
    public static func fy4aHritFrameLength(_ handle: SDRKIT_HANDLE) -> Result<Fy4aHritFrameLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_length", &v)
        return code == SDRKIT_OK ? .success(Fy4aHritFrameLength(rawValue: v) ?? Fy4aHritFrameLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritFrameLength(_ handle: SDRKIT_HANDLE, _ value: Fy4aHritFrameLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_length", value.rawValue))
    }
    /// The options of the setting "pilots" of the stack "fy4a_hrit".
    public enum Fy4aHritPilots: Int32 {
        case any = 0
        case on = 1
        case off = 2
    }
    /// Pilots.
    public static func fy4aHritPilots(_ handle: SDRKIT_HANDLE) -> Result<Fy4aHritPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pilots", &v)
        return code == SDRKIT_OK ? .success(Fy4aHritPilots(rawValue: v) ?? Fy4aHritPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritPilots(_ handle: SDRKIT_HANDLE, _ value: Fy4aHritPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pilots", value.rawValue))
    }
    /// PL scrambling code.
    public static func fy4aHritPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value))
    }
    /// The options of the setting "filter" of the stack "fy4a_hrit".
    public enum Fy4aHritFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func fy4aHritFilter(_ handle: SDRKIT_HANDLE) -> Result<Fy4aHritFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(Fy4aHritFilter(rawValue: v) ?? Fy4aHritFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritFilter(_ handle: SDRKIT_HANDLE, _ value: Fy4aHritFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func fy4aHritFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy4aHritFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// Symbol rate.
    public static func goesGrbSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "roll_off" of the stack "goes_grb".
    public enum GoesGrbRollOff: Int32 {
        case auto = 0
        case n035 = 1
        case n025 = 2
        case n020 = 3
        case n015 = 4
        case n010 = 5
        case n005 = 6
    }
    /// Roll-off.
    public static func goesGrbRollOff(_ handle: SDRKIT_HANDLE) -> Result<GoesGrbRollOff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(GoesGrbRollOff(rawValue: v) ?? GoesGrbRollOff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbRollOff(_ handle: SDRKIT_HANDLE, _ value: GoesGrbRollOff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "roll_off", value.rawValue))
    }
    /// Carrier.
    public static func goesGrbCarrierOffset(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "carrier_offset", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbCarrierOffset(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "carrier_offset", $0) })
    }
    /// Product shown.
    public static func goesGrbProduct(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "product", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbProduct(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "product", value))
    }
    /// Virtual channels.
    public static func goesGrbChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// The options of the setting "modcod_rule" of the stack "goes_grb".
    public enum GoesGrbModcodRule: Int32 {
        case any = 0
        case fixed = 1
        case set = 2
    }
    /// MODCODs taken.
    public static func goesGrbModcodRule(_ handle: SDRKIT_HANDLE) -> Result<GoesGrbModcodRule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &v)
        return code == SDRKIT_OK ? .success(GoesGrbModcodRule(rawValue: v) ?? GoesGrbModcodRule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbModcodRule(_ handle: SDRKIT_HANDLE, _ value: GoesGrbModcodRule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modcod_rule", value.rawValue))
    }
    /// MODCOD.
    public static func goesGrbModcod(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "modcod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbModcod(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "modcod", value))
    }
    /// MODCOD set.
    public static func goesGrbModcods(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "modcods", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbModcods(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "modcods", $0) })
    }
    /// The options of the setting "frame_length" of the stack "goes_grb".
    public enum GoesGrbFrameLength: Int32 {
        case any = 0
        case normal = 1
        case short = 2
    }
    /// Frame length.
    public static func goesGrbFrameLength(_ handle: SDRKIT_HANDLE) -> Result<GoesGrbFrameLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_length", &v)
        return code == SDRKIT_OK ? .success(GoesGrbFrameLength(rawValue: v) ?? GoesGrbFrameLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbFrameLength(_ handle: SDRKIT_HANDLE, _ value: GoesGrbFrameLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_length", value.rawValue))
    }
    /// The options of the setting "pilots" of the stack "goes_grb".
    public enum GoesGrbPilots: Int32 {
        case any = 0
        case on = 1
        case off = 2
    }
    /// Pilots.
    public static func goesGrbPilots(_ handle: SDRKIT_HANDLE) -> Result<GoesGrbPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pilots", &v)
        return code == SDRKIT_OK ? .success(GoesGrbPilots(rawValue: v) ?? GoesGrbPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbPilots(_ handle: SDRKIT_HANDLE, _ value: GoesGrbPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pilots", value.rawValue))
    }
    /// PL scrambling code.
    public static func goesGrbPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGrbPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value))
    }
    /// Symbol rate.
    public static func satFilecastSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "roll_off" of the stack "sat_filecast".
    public enum SatFilecastRollOff: Int32 {
        case auto = 0
        case n035 = 1
        case n025 = 2
        case n020 = 3
        case n015 = 4
        case n010 = 5
        case n005 = 6
    }
    /// Roll-off.
    public static func satFilecastRollOff(_ handle: SDRKIT_HANDLE) -> Result<SatFilecastRollOff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(SatFilecastRollOff(rawValue: v) ?? SatFilecastRollOff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastRollOff(_ handle: SDRKIT_HANDLE, _ value: SatFilecastRollOff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "roll_off", value.rawValue))
    }
    /// Carrier.
    public static func satFilecastCarrierOffset(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "carrier_offset", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastCarrierOffset(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "carrier_offset", $0) })
    }
    /// PID of the MPE stream.
    public static func satFilecastPid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastPid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pid", value))
    }
    /// Part size.
    public static func satFilecastPartBytes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "part_bytes", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastPartBytes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "part_bytes", value))
    }
    /// Give up on a file after.
    public static func satFilecastMaxSilence(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_silence", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastMaxSilence(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_silence", value))
    }
    /// Name tag of GEONETCAST.
    public static func satFilecastNameTag(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "name_tag", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastNameTag(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "name_tag", value))
    }
    /// Input stream.
    public static func satFilecastIsi(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "isi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastIsi(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "isi", value))
    }
    /// The options of the setting "modcod_rule" of the stack "sat_filecast".
    public enum SatFilecastModcodRule: Int32 {
        case any = 0
        case fixed = 1
        case set = 2
    }
    /// MODCODs taken.
    public static func satFilecastModcodRule(_ handle: SDRKIT_HANDLE) -> Result<SatFilecastModcodRule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &v)
        return code == SDRKIT_OK ? .success(SatFilecastModcodRule(rawValue: v) ?? SatFilecastModcodRule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastModcodRule(_ handle: SDRKIT_HANDLE, _ value: SatFilecastModcodRule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modcod_rule", value.rawValue))
    }
    /// MODCOD.
    public static func satFilecastModcod(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "modcod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastModcod(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "modcod", value))
    }
    /// MODCOD set.
    public static func satFilecastModcods(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "modcods", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastModcods(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "modcods", $0) })
    }
    /// The options of the setting "frame_length" of the stack "sat_filecast".
    public enum SatFilecastFrameLength: Int32 {
        case any = 0
        case normal = 1
        case short = 2
    }
    /// Frame length.
    public static func satFilecastFrameLength(_ handle: SDRKIT_HANDLE) -> Result<SatFilecastFrameLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_length", &v)
        return code == SDRKIT_OK ? .success(SatFilecastFrameLength(rawValue: v) ?? SatFilecastFrameLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastFrameLength(_ handle: SDRKIT_HANDLE, _ value: SatFilecastFrameLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_length", value.rawValue))
    }
    /// The options of the setting "pilots" of the stack "sat_filecast".
    public enum SatFilecastPilots: Int32 {
        case any = 0
        case on = 1
        case off = 2
    }
    /// Pilots.
    public static func satFilecastPilots(_ handle: SDRKIT_HANDLE) -> Result<SatFilecastPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pilots", &v)
        return code == SDRKIT_OK ? .success(SatFilecastPilots(rawValue: v) ?? SatFilecastPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastPilots(_ handle: SDRKIT_HANDLE, _ value: SatFilecastPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pilots", value.rawValue))
    }
    /// PL scrambling code.
    public static func satFilecastPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value))
    }
    /// The options of the setting "craft" of the stack "elektro_l".
    public enum ElektroLCraft: Int32 {
        case n3 = 0
        case n4 = 1
        case n5 = 2
    }
    /// Satellite.
    public static func elektroLCraft(_ handle: SDRKIT_HANDLE) -> Result<ElektroLCraft, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "craft", &v)
        return code == SDRKIT_OK ? .success(ElektroLCraft(rawValue: v) ?? ElektroLCraft(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLCraft(_ handle: SDRKIT_HANDLE, _ value: ElektroLCraft) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "craft", value.rawValue))
    }
    /// The options of the setting "link" of the stack "elektro_l".
    public enum ElektroLLink: Int32 {
        case hrit665 = 0
        case hrit1000 = 1
        case lrit128 = 2
        case lrit64 = 3
        case ggak = 4
    }
    /// Link.
    public static func elektroLLink(_ handle: SDRKIT_HANDLE) -> Result<ElektroLLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "link", &v)
        return code == SDRKIT_OK ? .success(ElektroLLink(rawValue: v) ?? ElektroLLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLLink(_ handle: SDRKIT_HANDLE, _ value: ElektroLLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "link", value.rawValue))
    }
    /// The options of the setting "filter" of the stack "elektro_l".
    public enum ElektroLFilter: Int32 {
        case all = 0
        case images = 1
        case text = 2
    }
    /// Show.
    public static func elektroLFilter(_ handle: SDRKIT_HANDLE) -> Result<ElektroLFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "filter", &v)
        return code == SDRKIT_OK ? .success(ElektroLFilter(rawValue: v) ?? ElektroLFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLFilter(_ handle: SDRKIT_HANDLE, _ value: ElektroLFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "filter", value.rawValue))
    }
    /// Newest.
    public static func elektroLFollowNewest(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_newest", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLFollowNewest(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_newest", value))
    }
    /// The options of the setting "frequency_preset" of the stack "hrpt".
    public enum HrptFrequencyPreset: Int32 {
        case n1701300 = 0
        case n1701400 = 1
        case n1698000 = 2
        case n1702500 = 3
        case n1707000 = 4
    }
    /// Frequency.
    public static func hrptFrequencyPreset(_ handle: SDRKIT_HANDLE) -> Result<HrptFrequencyPreset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frequency_preset", &v)
        return code == SDRKIT_OK ? .success(HrptFrequencyPreset(rawValue: v) ?? HrptFrequencyPreset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptFrequencyPreset(_ handle: SDRKIT_HANDLE, _ value: HrptFrequencyPreset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frequency_preset", value.rawValue))
    }
    /// The options of the setting "variant" of the stack "hrpt".
    public enum HrptVariant: Int32 {
        case auto = 0
        case metop = 1
        case fy3 = 2
        case noaa = 3
    }
    /// Satellite.
    public static func hrptVariant(_ handle: SDRKIT_HANDLE) -> Result<HrptVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "variant", &v)
        return code == SDRKIT_OK ? .success(HrptVariant(rawValue: v) ?? HrptVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptVariant(_ handle: SDRKIT_HANDLE, _ value: HrptVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "variant", value.rawValue))
    }
    /// The options of the setting "view" of the stack "hrpt".
    public enum HrptView: Int32 {
        case rgb214 = 0
        case ch1 = 1
        case ch2 = 2
        case ch3 = 3
        case ch4 = 4
        case ch5 = 5
    }
    /// View.
    public static func hrptView(_ handle: SDRKIT_HANDLE) -> Result<HrptView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "view", &v)
        return code == SDRKIT_OK ? .success(HrptView(rawValue: v) ?? HrptView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptView(_ handle: SDRKIT_HANDLE, _ value: HrptView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "view", value.rawValue))
    }
    /// The options of the setting "hirs_view" of the stack "hrpt".
    public enum HrptHirsView: Int32 {
        case rgb214 = 0
        case ch1 = 1
        case ch2 = 2
        case ch3 = 3
        case ch4 = 4
        case ch5 = 5
        case ch6 = 6
        case ch7 = 7
        case ch8 = 8
        case ch9 = 9
        case ch10 = 10
        case ch11 = 11
        case ch12 = 12
        case ch13 = 13
        case ch14 = 14
        case ch15 = 15
        case ch16 = 16
        case ch17 = 17
        case ch18 = 18
        case ch19 = 19
        case ch20 = 20
    }
    /// HIRS view.
    public static func hrptHirsView(_ handle: SDRKIT_HANDLE) -> Result<HrptHirsView, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "hirs_view", &v)
        return code == SDRKIT_OK ? .success(HrptHirsView(rawValue: v) ?? HrptHirsView(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptHirsView(_ handle: SDRKIT_HANDLE, _ value: HrptHirsView) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "hirs_view", value.rawValue))
    }
    /// Follow.
    public static func hrptFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Symbol rate.
    public static func eodbSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEodbSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// Matched filter roll-off.
    public static func eodbRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEodbRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// Satellite list.
    public static func eodbSatellites(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "satellites", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEodbSatellites(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "satellites", $0) })
    }
    /// AVHRR calibration.
    public static func eodbAvhrrCalibration(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "avhrr_calibration", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEodbAvhrrCalibration(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "avhrr_calibration", $0) })
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum SatimgGeneratorSettings {
    /// The options of the setting "gen_modulation" of the stack "meteor_lrpt".
    public enum MeteorLrptModulation: Int32 {
        case oqpsk72 = 0
        case qpsk72 = 1
        case oqpsk80 = 2
    }
    /// Test modulation.
    public static func meteorLrptModulation(_ handle: SDRKIT_HANDLE) -> Result<MeteorLrptModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &v)
        return code == SDRKIT_OK ? .success(MeteorLrptModulation(rawValue: v) ?? MeteorLrptModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptModulation(_ handle: SDRKIT_HANDLE, _ value: MeteorLrptModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_modulation", value.rawValue))
    }
    /// Test night channels.
    public static func meteorLrptNight(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_night", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptNight(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_night", value))
    }
    /// Test picture rows.
    public static func meteorLrptRows(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_rows", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeteorLrptRows(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_rows", value))
    }
    /// Test G2 inverted.
    public static func goesHritInvertG2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritInvertG2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value))
    }
    /// Test NRZ-M.
    public static func goesHritNrzm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nrzm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritNrzm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value))
    }
    /// The options of the setting "gen_basis" of the stack "goes_hrit".
    public enum GoesHritBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// Test Reed-Solomon basis.
    public static func goesHritBasis(_ handle: SDRKIT_HANDLE) -> Result<GoesHritBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &v)
        return code == SDRKIT_OK ? .success(GoesHritBasis(rawValue: v) ?? GoesHritBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritBasis(_ handle: SDRKIT_HANDLE, _ value: GoesHritBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_basis", value.rawValue))
    }
    /// Test fill frames.
    public static func goesHritFill(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fill", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritFill(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fill", value))
    }
    /// Test files once.
    public static func goesHritOnce(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_once", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritOnce(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_once", value))
    }
    /// Test frame count start.
    public static func goesHritCounter(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_counter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesHritCounter(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_counter", value))
    }
    /// Test G2 inverted.
    public static func gk2aLritInvertG2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritInvertG2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value))
    }
    /// Test NRZ-M.
    public static func gk2aLritNrzm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nrzm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritNrzm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value))
    }
    /// The options of the setting "gen_basis" of the stack "gk2a_lrit".
    public enum Gk2aLritBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// Test Reed-Solomon basis.
    public static func gk2aLritBasis(_ handle: SDRKIT_HANDLE) -> Result<Gk2aLritBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &v)
        return code == SDRKIT_OK ? .success(Gk2aLritBasis(rawValue: v) ?? Gk2aLritBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritBasis(_ handle: SDRKIT_HANDLE, _ value: Gk2aLritBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_basis", value.rawValue))
    }
    /// Test fill frames.
    public static func gk2aLritFill(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fill", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritFill(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fill", value))
    }
    /// Test files once.
    public static func gk2aLritOnce(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_once", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritOnce(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_once", value))
    }
    /// Test frame count start.
    public static func gk2aLritCounter(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_counter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritCounter(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_counter", value))
    }
    /// Test encrypted VI006.
    public static func gk2aLritEncrypt(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_encrypt", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aLritEncrypt(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_encrypt", value))
    }
    /// Test G2 inverted.
    public static func gk2aHritInvertG2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritInvertG2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value))
    }
    /// Test NRZ-M.
    public static func gk2aHritNrzm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nrzm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritNrzm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value))
    }
    /// The options of the setting "gen_basis" of the stack "gk2a_hrit".
    public enum Gk2aHritBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// Test Reed-Solomon basis.
    public static func gk2aHritBasis(_ handle: SDRKIT_HANDLE) -> Result<Gk2aHritBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &v)
        return code == SDRKIT_OK ? .success(Gk2aHritBasis(rawValue: v) ?? Gk2aHritBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritBasis(_ handle: SDRKIT_HANDLE, _ value: Gk2aHritBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_basis", value.rawValue))
    }
    /// Test fill frames.
    public static func gk2aHritFill(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fill", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritFill(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fill", value))
    }
    /// Test files once.
    public static func gk2aHritOnce(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_once", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritOnce(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_once", value))
    }
    /// Test frame count start.
    public static func gk2aHritCounter(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_counter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritCounter(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_counter", value))
    }
    /// Test encrypted VI006.
    public static func gk2aHritEncrypt(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_encrypt", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGk2aHritEncrypt(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_encrypt", value))
    }
    /// Test symbol rate.
    public static func fy2SvissrSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy2SvissrSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Test first line.
    public static func fy2SvissrFirstLine(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_first_line", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy2SvissrFirstLine(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_first_line", value))
    }
    /// Test lines.
    public static func fy2SvissrLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy2SvissrLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test spacecraft id.
    public static func fy3AhrptSatellite(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_satellite", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptSatellite(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_satellite", value))
    }
    /// Test symbol rate.
    public static func fy3AhrptSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Test Q negated.
    public static func fy3AhrptQneg(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_qneg", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptQneg(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_qneg", value))
    }
    /// Test I and Q exchanged.
    public static func fy3AhrptSwap(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_swap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptSwap(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_swap", value))
    }
    /// Test fill frame every.
    public static func fy3AhrptFillEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fill_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3AhrptFillEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fill_every", value))
    }
    /// Test spacecraft id.
    public static func fy3MptSatellite(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_satellite", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptSatellite(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_satellite", value))
    }
    /// Test symbol rate.
    public static func fy3MptSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Test Q negated.
    public static func fy3MptQneg(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_qneg", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptQneg(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_qneg", value))
    }
    /// Test I and Q exchanged.
    public static func fy3MptSwap(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_swap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptSwap(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_swap", value))
    }
    /// Test fill frame every.
    public static func fy3MptFillEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fill_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFy3MptFillEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fill_every", value))
    }
    /// Test picture lines.
    public static func noaaAptLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaAptLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test spacecraft address.
    public static func noaaDsbScid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_scid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaDsbScid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_scid", value))
    }
    /// Test phase index.
    public static func noaaDsbPmIndex(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pm_index", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaDsbPmIndex(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pm_index", value))
    }
    /// Test picture lines.
    public static func noaaDsbLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaDsbLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test scans.
    public static func goesGvarLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesGvarLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test picture scans.
    public static func awsLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAwsLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test spacecraft address.
    public static func noaaGacScid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_scid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaGacScid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_scid", value))
    }
    /// Test picture lines.
    public static func noaaGacLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoaaGacLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test satellite number.
    public static func msgRawScid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_scid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsgRawScid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_scid", value))
    }
    /// Test picture lines.
    public static func msgRawLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsgRawLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test first line.
    public static func msgRawStart(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMsgRawStart(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_start", value))
    }
    /// Test phase index (GOES-N).
    public static func goesCdaPmIndex(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pm_index", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGoesCdaPmIndex(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pm_index", value))
    }
    /// Symbol rate.
    public static func satFilecastSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatFilecastSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Test G2 inverted.
    public static func elektroLInvertG2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLInvertG2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value))
    }
    /// Test NRZ-M.
    public static func elektroLNrzm(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nrzm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLNrzm(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value))
    }
    /// The options of the setting "gen_basis" of the stack "elektro_l".
    public enum ElektroLBasis: Int32 {
        case dual = 0
        case conventional = 1
    }
    /// Test Reed-Solomon basis.
    public static func elektroLBasis(_ handle: SDRKIT_HANDLE) -> Result<ElektroLBasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &v)
        return code == SDRKIT_OK ? .success(ElektroLBasis(rawValue: v) ?? ElektroLBasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLBasis(_ handle: SDRKIT_HANDLE, _ value: ElektroLBasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_basis", value.rawValue))
    }
    /// Test fill frames.
    public static func elektroLFill(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_fill", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLFill(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_fill", value))
    }
    /// Test files once.
    public static func elektroLOnce(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_once", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLOnce(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_once", value))
    }
    /// Test frame count start.
    public static func elektroLCounter(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_counter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLCounter(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_counter", value))
    }
    /// The options of the setting "gen_link" of the stack "elektro_l".
    public enum ElektroLLink: Int32 {
        case hrit665 = 0
        case hrit1000 = 1
        case lrit128 = 2
        case lrit64 = 3
        case ggak = 4
    }
    /// Test link.
    public static func elektroLLink(_ handle: SDRKIT_HANDLE) -> Result<ElektroLLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_link", &v)
        return code == SDRKIT_OK ? .success(ElektroLLink(rawValue: v) ?? ElektroLLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setElektroLLink(_ handle: SDRKIT_HANDLE, _ value: ElektroLLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_link", value.rawValue))
    }
    /// The options of the setting "gen_variant" of the stack "hrpt".
    public enum HrptVariant: Int32 {
        case metop = 0
        case fy3 = 1
        case noaa = 2
    }
    /// Test satellite.
    public static func hrptVariant(_ handle: SDRKIT_HANDLE) -> Result<HrptVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &v)
        return code == SDRKIT_OK ? .success(HrptVariant(rawValue: v) ?? HrptVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptVariant(_ handle: SDRKIT_HANDLE, _ value: HrptVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_variant", value.rawValue))
    }
    /// Test channel 3B (APID 104).
    public static func hrpt3b(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_3b", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHrpt3b(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_3b", value))
    }
    /// Test picture lines.
    public static func hrptLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_lines", value))
    }
    /// Test inverted NOAA polarity.
    public static func hrptPolarity(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_polarity", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptPolarity(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_polarity", value))
    }
    /// Test phase index (Meteor).
    public static func hrptPmIndex(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pm_index", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHrptPmIndex(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pm_index", value))
    }
    /// Test signal ambiguity.
    public static func eodbAmbiguity(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ambiguity", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEodbAmbiguity(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ambiguity", value))
    }
}

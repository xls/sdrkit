// sdrkit: the typed interface of the stack family "cell", version 1.8. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum CellInterface {
    public static let major = 1
    public static let minor = 8
}

/// The result fields of `railway_cir`.
public struct RailwayCirFields {
    public enum State: Int32 {
        case waiting = 0
        case listening = 1
        case decoding = 2
    }
    /// The summary of the last message.
    public var text: String
    /// Messages decoded.
    public var messages: Int64
    /// Sync words found.
    public var syncs: Int64
    /// Readings that failed: CRC failures, rejections and ambiguous frames.
    public var failedChecks: Int64
    /// The carrier offset of the channel.
    public var carrierOffsetHz: Double?
    /// The level of the channel.
    public var levelDb: Double?
    /// What the receiver is doing.
    public var state: State?

    init(_ c: sdrkit_railway_cir_fields) {
        text = sdrkitText(c.text)
        messages = c.messages
        syncs = c.syncs
        failedChecks = c.failed_checks
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
        levelDb = c.has_level_db ? c.level_db : nil
        state = State(rawValue: c.state)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RailwayCirFields, SDRKitCode> {
        var c = sdrkit_railway_cir_fields()
        let code = sdrkit_railway_cir_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RailwayCirFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct CellDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_cell_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CellDecoderFields, SDRKitCode> {
        var c = sdrkit_cell_decoder_fields()
        let code = sdrkit_cell_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CellDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the message list of `railway_cir`, newest first. The row key is the sequence number of the message.
public struct RailwayCirMessageRow {
    /// The row key.
    public var key: String
    /// The wall time of the reception.
    public var received: Double
    /// The protocol: `KLW` or `TDCS`.
    public var `protocol`: String
    /// The train (TDCS).
    public var train: String?
    /// The locomotive.
    public var locomotive: String
    /// The action (KLW).
    public var action: String?
    /// The tail device id (KLW).
    public var tailId: String?
    /// The brake pipe pressure (KLW).
    public var pressureKpa: Double?
    /// The speed (TDCS).
    public var speedKmh: Double?
    /// The kilometre post (TDCS).
    public var kmPostKm: Double?
    /// The frame marks itself as a test frame (TDCS).
    public var testFrame: Bool
    /// The car count (TDCS).
    public var cars: Int64?
    /// The bits corrected.
    public var corrections: Int64
    /// The raw frame as hex.
    public var raw: String

    init(_ c: sdrkit_railway_cir_message_row) {
        key = sdrkitText(c.key)
        received = c.received
        `protocol` = sdrkitText(c.protocol)
        train = c.has_train ? sdrkitText(c.train) : nil
        locomotive = sdrkitText(c.locomotive)
        action = c.has_action ? sdrkitText(c.action) : nil
        tailId = c.has_tail_id ? sdrkitText(c.tail_id) : nil
        pressureKpa = c.has_pressure_kpa ? c.pressure_kpa : nil
        speedKmh = c.has_speed_kmh ? c.speed_kmh : nil
        kmPostKm = c.has_km_post_km ? c.km_post_km : nil
        testFrame = c.test_frame
        cars = c.has_cars ? c.cars : nil
        corrections = c.corrections
        raw = sdrkitText(c.raw)
    }
}

/// The counts and the level of the capture search of every cell stack.
public struct CellSearchFields {
    /// Captures searched.
    public var captures: Int64
    /// Retunes seen (scan steps).
    public var retunes: Int64
    /// Messages decoded.
    public var decoded: Int64
    /// Messages that failed their check.
    public var failed: Int64
    /// The centre frequency of the newest capture.
    public var captureCentre: Double?
    /// The input level.
    public var levelDbfs: Double?

    init(_ c: sdrkit_cell_search_fields) {
        captures = c.captures
        retunes = c.retunes
        decoded = c.decoded
        failed = c.failed
        captureCentre = c.has_capture_centre ? c.capture_centre : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CellSearchFields, SDRKitCode> {
        var c = sdrkit_cell_search_fields()
        let code = sdrkit_cell_search_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CellSearchFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `cell`.
public struct CellFields {
    /// Cells in the table.
    public var cellCount: Int64
    /// Cells with SIB1.
    public var identified: Int64
    /// Operators of the cells.
    public var operators: Int64
    /// The notice.
    public var banner: String

    init(_ c: sdrkit_cell_fields) {
        cellCount = c.cell_count
        identified = c.identified
        operators = c.operators
        banner = sdrkitText(c.banner)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CellFields, SDRKitCode> {
        var c = sdrkit_cell_fields()
        let code = sdrkit_cell_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CellFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// Where the band search of `cell` and `nr` is, and why nothing shows.
public struct CellBandSearchFields {
    /// The band searched now ("band 28", "n78"). None when no search runs.
    public var searchBand: String?
    /// The radio centre of the search step, counted from 1.
    public var searchStep: Int64?
    /// The radio centres of the search.
    public var searchSteps: Int64?
    /// The passes over every step done since the search started.
    public var searchPasses: Int64
    /// Why no cell shows: what the newest capture held, or what the radio gives. None while cells show.
    public var quietReason: String?

    init(_ c: sdrkit_cell_band_search_fields) {
        searchBand = c.has_search_band ? sdrkitText(c.search_band) : nil
        searchStep = c.has_search_step ? c.search_step : nil
        searchSteps = c.has_search_steps ? c.search_steps : nil
        searchPasses = c.search_passes
        quietReason = c.has_quiet_reason ? sdrkitText(c.quiet_reason) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CellBandSearchFields, SDRKitCode> {
        var c = sdrkit_cell_band_search_fields()
        let code = sdrkit_cell_band_search_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CellBandSearchFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the cells of `cell`. The row key is the cell key.
public struct CellRow {
    public enum Duplex: Int32 {
        case fDD = 0
        case tDD = 1
    }
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// The primary network.
    public var plmn: String?
    /// The operator of the primary network.
    public var `operator`: String?
    /// The country of the primary network.
    public var country: String?
    /// The networks of the cell.
    public var plmnCount: Int64?
    /// The band.
    public var band: Int64?
    /// Whether SIB1 confirmed the band.
    public var bandConfirmed: Bool
    /// The EARFCN.
    public var earfcn: Int64?
    /// The nominal frequency.
    public var frequency: Double
    /// The physical cell identity.
    public var pci: Int64
    /// The duplex.
    public var duplex: Duplex?
    /// The eNB identity.
    public var enb: Int64?
    /// The sector.
    public var sector: Int64?
    /// The cell identity.
    public var cellId: Int64?
    /// The tracking area code.
    public var tac: Int64?
    /// The bandwidth.
    public var width: Double?
    /// The CRS ports.
    public var ports: Int64?
    /// The RSRP.
    public var rsrpDbfs: Double
    /// The RSRQ.
    public var rsrqDb: Double
    /// The SINR.
    public var sinrDb: Double
    /// The carrier offset.
    public var carrierOffset: Double
    /// Whether the cell is barred.
    public var barred: Bool?
    /// Whether SIB1 schedules ETWS.
    public var etws: Bool
    /// Whether SIB1 schedules CMAS.
    public var cmas: Bool
    /// When the cell was last heard.
    public var lastHeard: Double?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_cell_row) {
        key = sdrkitText(c.key)
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        plmnCount = c.has_plmn_count ? c.plmn_count : nil
        band = c.has_band ? c.band : nil
        bandConfirmed = c.band_confirmed
        earfcn = c.has_earfcn ? c.earfcn : nil
        frequency = c.frequency
        pci = c.pci
        duplex = Duplex(rawValue: c.duplex)
        enb = c.has_enb ? c.enb : nil
        sector = c.has_sector ? c.sector : nil
        cellId = c.has_cell_id ? c.cell_id : nil
        tac = c.has_tac ? c.tac : nil
        width = c.has_width ? c.width : nil
        ports = c.has_ports ? c.ports : nil
        rsrpDbfs = c.rsrp_dbfs
        rsrqDb = c.rsrq_db
        sinrDb = c.sinr_db
        carrierOffset = c.carrier_offset
        barred = c.has_barred ? c.barred : nil
        etws = c.etws
        cmas = c.cmas
        lastHeard = c.has_last_heard ? c.last_heard : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// A row of the details of `cell`. The row key is the cell key.
public struct CellDetailRow {
    public enum CyclicPrefix: Int32 {
        case normal = 0
        case extended = 1
    }
    public enum PhichDuration: Int32 {
        case normal = 0
        case extended = 1
    }
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The cyclic prefix.
    public var cyclicPrefix: CyclicPrefix?
    /// The PHICH duration.
    public var phichDuration: PhichDuration?
    /// The PHICH Ng index.
    public var phichNg: Int64?
    /// The SFN of the first whole frame.
    public var sfn: Int64?
    /// The clock error.
    public var clockPpm: Double
    /// SI messages decoded.
    public var siMessages: Int64
    /// SI messages that failed their CRC.
    public var siFailed: Int64
    /// q-RxLevMin of SIB1.
    public var qRxLevMinDbm: Double?
    /// p-Max of SIB1.
    public var pMaxDbm: Double?
    /// The SI window.
    public var siWindow: Double?
    /// The value tag of SIB1.
    public var valueTag: Int64?
    /// The CRS power of SIB2.
    public var refSignalPowerDbm: Double?
    /// p-b of SIB2.
    public var pB: Int64?
    /// The uplink EARFCN.
    public var ulEarfcn: Int64?
    /// The uplink RBs.
    public var ulRbs: Int64?
    /// The random access preambles.
    public var raPreambles: Int64?
    /// The modification period coefficient.
    public var modPeriodCoeff: Int64?
    /// The paging cycle.
    public var pagingCycle: Int64?
    /// t300.
    public var t300: Double?
    /// t310.
    public var t310: Double?
    /// n310.
    public var n310: Int64?
    /// The time alignment timer.
    public var timeAlignment: Double?
    /// Whether access class barring is on.
    public var acBarring: Bool?
    /// q-Hyst of SIB3.
    public var qHystDb: Double?
    /// The reselection priority of SIB3.
    public var priority: Int64?
    /// s-IntraSearch of SIB3.
    public var sIntraSearchDb: Double?
    /// s-NonIntraSearch of SIB3.
    public var sNonIntraSearchDb: Double?
    /// threshServingLow of SIB3.
    public var threshServingLowDb: Double?
    /// t-Reselection of SIB3.
    public var tReselection: Double?

    init(_ c: sdrkit_cell_detail_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        cyclicPrefix = CyclicPrefix(rawValue: c.cyclic_prefix)
        phichDuration = c.has_phich_duration ? PhichDuration(rawValue: c.phich_duration) : nil
        phichNg = c.has_phich_ng ? c.phich_ng : nil
        sfn = c.has_sfn ? c.sfn : nil
        clockPpm = c.clock_ppm
        siMessages = c.si_messages
        siFailed = c.si_failed
        qRxLevMinDbm = c.has_q_rx_lev_min_dbm ? c.q_rx_lev_min_dbm : nil
        pMaxDbm = c.has_p_max_dbm ? c.p_max_dbm : nil
        siWindow = c.has_si_window ? c.si_window : nil
        valueTag = c.has_value_tag ? c.value_tag : nil
        refSignalPowerDbm = c.has_ref_signal_power_dbm ? c.ref_signal_power_dbm : nil
        pB = c.has_p_b ? c.p_b : nil
        ulEarfcn = c.has_ul_earfcn ? c.ul_earfcn : nil
        ulRbs = c.has_ul_rbs ? c.ul_rbs : nil
        raPreambles = c.has_ra_preambles ? c.ra_preambles : nil
        modPeriodCoeff = c.has_mod_period_coeff ? c.mod_period_coeff : nil
        pagingCycle = c.has_paging_cycle ? c.paging_cycle : nil
        t300 = c.has_t300 ? c.t300 : nil
        t310 = c.has_t310 ? c.t310 : nil
        n310 = c.has_n310 ? c.n310 : nil
        timeAlignment = c.has_time_alignment ? c.time_alignment : nil
        acBarring = c.has_ac_barring ? c.ac_barring : nil
        qHystDb = c.has_q_hyst_db ? c.q_hyst_db : nil
        priority = c.has_priority ? c.priority : nil
        sIntraSearchDb = c.has_s_intra_search_db ? c.s_intra_search_db : nil
        sNonIntraSearchDb = c.has_s_non_intra_search_db ? c.s_non_intra_search_db : nil
        threshServingLowDb = c.has_thresh_serving_low_db ? c.thresh_serving_low_db : nil
        tReselection = c.has_t_reselection ? c.t_reselection : nil
    }
}

/// A network of a cell of `cell`. The row key is the cell key and the place of the network.
public struct CellPlmnRow {
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The network.
    public var plmn: String
    /// The operator of the network.
    public var `operator`: String?

    init(_ c: sdrkit_cell_plmn_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        plmn = sdrkitText(c.plmn)
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
    }
}

/// Another band of a cell of `cell`. The row key is the cell key and the band.
public struct CellBandRow {
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The band.
    public var band: Int64
    /// Whether the band is of the multi-band list.
    public var mfbi: Bool

    init(_ c: sdrkit_cell_band_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        band = c.band
        mfbi = c.mfbi
    }
}

/// A system information block of a cell of `cell`. The row key is the cell key and the SIB number.
public struct CellSibRow {
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The SIB number.
    public var sib: Int64
    /// The period of the SIB.
    public var period: Int64?
    /// Whether the SIB was read.
    public var seen: Bool

    init(_ c: sdrkit_cell_sib_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        sib = c.sib
        period = c.has_period ? c.period : nil
        seen = c.seen
    }
}

/// A neighbour of a cell of `cell`.
public struct CellNeighbourRow {
    public enum List: Int32 {
        case intra = 0
        case inter = 1
        case cdma2000 = 2
    }
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The list that names the neighbour.
    public var list: List?
    /// The EARFCN of the neighbour.
    public var earfcn: Int64?
    /// The physical cell identity of the neighbour.
    public var pci: Int64
    /// q-OffsetCell.
    public var qOffsetDb: Double?
    /// The ARFCN of a CDMA2000 neighbour; `pci` is then its PN offset.
    public var cdma2000Arfcn: Int64?

    init(_ c: sdrkit_cell_neighbour_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        list = List(rawValue: c.list)
        earfcn = c.has_earfcn ? c.earfcn : nil
        pci = c.pci
        qOffsetDb = c.has_q_offset_db ? c.q_offset_db : nil
        cdma2000Arfcn = c.has_cdma2000_arfcn ? c.cdma2000_arfcn : nil
    }
}

/// Another carrier that a cell of `cell` names in SIB5. The row key is the cell key and the EARFCN.
public struct CellInterCarrierRow {
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The EARFCN.
    public var earfcn: Int64
    /// The RBs.
    public var rbs: Int64
    /// The reselection priority.
    public var priority: Int64?
    /// threshX-High.
    public var threshHighDb: Double
    /// threshX-Low.
    public var threshLowDb: Double
    /// q-RxLevMin.
    public var qRxLevMinDbm: Double

    init(_ c: sdrkit_cell_inter_carrier_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        earfcn = c.earfcn
        rbs = c.rbs
        priority = c.has_priority ? c.priority : nil
        threshHighDb = c.thresh_high_db
        threshLowDb = c.thresh_low_db
        qRxLevMinDbm = c.q_rx_lev_min_dbm
    }
}

/// A site of `cell`: the cells of one eNB. The row key is the network and the eNB.
public struct CellSiteRow {
    /// The row key.
    public var key: String
    /// The network.
    public var plmn: String?
    /// The operator of the network.
    public var `operator`: String?
    /// The eNB identity.
    public var enb: Int64
    /// The tracking area code.
    public var tac: Int64
    /// The cells of the site.
    public var cellCount: Int64

    init(_ c: sdrkit_cell_site_row) {
        key = sdrkitText(c.key)
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        enb = c.enb
        tac = c.tac
        cellCount = c.cell_count
    }
}

/// An operator of `cell`. The row key is the network.
public struct CellOperatorRow {
    /// The row key.
    public var key: String
    /// The network.
    public var plmn: String
    /// The operator of the network.
    public var `operator`: String?
    /// The country of the network.
    public var country: String?
    /// The cells of the operator.
    public var cellCount: Int64

    init(_ c: sdrkit_cell_operator_row) {
        key = sdrkitText(c.key)
        plmn = sdrkitText(c.plmn)
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        cellCount = c.cell_count
    }
}

/// A carrier of the newest capture of `cell`. The row key is its place, strongest first.
public struct CellCarrierRow {
    /// The row key.
    public var key: String
    /// The frequency.
    public var frequency: Double
    /// The width.
    public var width: Double
    /// The level.
    public var levelDb: Double

    init(_ c: sdrkit_cell_carrier_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        width = c.width
        levelDb = c.level_db
    }
}

/// A carrier of another radio system that a cell of `cell` names in SIB6, SIB7, SIB8 or SIB24. The row key is the cell key, the system and the channel number.
public struct CellOtherCarrierRow {
    public enum System: Int32 {
        case umtsFdd = 0
        case umtsTdd = 1
        case gsm = 2
        case cdma2000Hrpd = 3
        case cdma20001x = 4
        case nr = 5
    }
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The system.
    public var system: System?
    /// The channel number: UARFCN, GSM ARFCN, CDMA2000 ARFCN or NR-ARFCN.
    public var channel: Int64
    /// The downlink frequency, where the channel number gives it alone (GSM and NR).
    public var frequency: Double?
    /// The band: dcs1800 or pcs1900 (GSM), bc0 .. (CDMA2000), n78 .. (NR).
    public var band: String?
    /// The SSB spacing (NR).
    public var ssbScs: Double?
    /// The reselection priority.
    public var priority: Int64?
    /// q-RxLevMin.
    public var qRxLevMinDbm: Double?
    /// The cells named on the carrier (CDMA2000).
    public var cells: Int64

    init(_ c: sdrkit_cell_other_carrier_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        system = System(rawValue: c.system)
        channel = c.channel
        frequency = c.has_frequency ? c.frequency : nil
        band = c.has_band ? sdrkitText(c.band) : nil
        ssbScs = c.has_ssb_scs ? c.ssb_scs : nil
        priority = c.has_priority ? c.priority : nil
        qRxLevMinDbm = c.has_q_rx_lev_min_dbm ? c.q_rx_lev_min_dbm : nil
        cells = c.cells
    }
}

/// A cell of a coverage survey: one row per cell found, in the same columns for every generation. The row key is the key of the cell in the cells table of the stack. The survey table has no role: the cells table is the nodes table of the stack, and a host finds the survey by its key `survey`.
public struct CellSurveyRow {
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// Operator, generation, band and physical identity.
    public var name: String
    /// The generation.
    public var generation: Generation?
    /// The operator name, else the network.
    public var `operator`: String?
    /// The network.
    public var plmn: String?
    /// The band.
    public var band: String?
    /// The channel number: ARFCN, UARFCN, EARFCN or NR-ARFCN.
    public var channel: Int64?
    /// The frequency.
    public var frequency: Double
    /// The physical identity: PCI, primary scrambling code or BSIC.
    public var pci: Int64
    /// The cell identity of the broadcast.
    public var cellIdentity: Int64?
    /// The tracking or location area code.
    public var areaCode: Int64?
    /// RSRP, RSCP or the received level.
    public var levelDbfs: Double?

    init(_ c: sdrkit_cell_survey_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        generation = Generation(rawValue: c.generation)
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        band = c.has_band ? sdrkitText(c.band) : nil
        channel = c.has_channel ? c.channel : nil
        frequency = c.frequency
        pci = c.pci
        cellIdentity = c.has_cell_identity ? c.cell_identity : nil
        areaCode = c.has_area_code ? c.area_code : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }
}

/// The result fields of `nr`.
public struct NrFields {
    /// Cells in the table.
    public var cellCount: Int64
    /// Cells with a MIB.
    public var withMib: Int64
    /// Cells with a SIB1.
    public var withSib1: Int64
    /// The lowest GSCN the newest capture searched.
    public var gscnLo: Int64?
    /// The highest GSCN the newest capture searched.
    public var gscnHi: Int64?
    /// The candidates the newest capture searched.
    public var candidates: Int64?
    /// The notice.
    public var banner: String

    init(_ c: sdrkit_nr_fields) {
        cellCount = c.cell_count
        withMib = c.with_mib
        withSib1 = c.with_sib1
        gscnLo = c.has_gscn_lo ? c.gscn_lo : nil
        gscnHi = c.has_gscn_hi ? c.gscn_hi : nil
        candidates = c.has_candidates ? c.candidates : nil
        banner = sdrkitText(c.banner)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NrFields, SDRKitCode> {
        var c = sdrkit_nr_fields()
        let code = sdrkit_nr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the cells of `nr`.
public struct NrCellRow {
    public enum Decoded: Int32 {
        case sync = 0
        case mib = 1
        case sib1 = 2
    }
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// How far the cell was decoded.
    public var decoded: Decoded?
    /// The first network of SIB1.
    public var plmn: String?
    /// The operator of that network.
    public var `operator`: String?
    /// The tracking area code of SIB1.
    public var tac: Int64?
    /// The NR cell identity of SIB1 (36 bits).
    public var nci: Int64?
    /// The band.
    public var band: Int64?
    /// The GSCN.
    public var gscn: Int64?
    /// The frequency of the SSB.
    public var frequency: Double
    /// The physical cell identity.
    public var pci: Int64
    /// The subcarrier spacing of the SSB.
    public var ssbScs: Double
    /// The beams seen.
    public var beams: Int64
    /// The SFN of the MIB.
    public var sfn: Int64?
    /// Whether the MIB schedules SIB1.
    public var sib1: Bool
    /// The RSRP.
    public var rsrpDbfs: Double
    /// The RSRQ.
    public var rsrqDb: Double
    /// The SINR.
    public var sinrDb: Double
    /// When the cell was last heard.
    public var lastHeard: Double?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_nr_cell_row) {
        key = sdrkitText(c.key)
        decoded = Decoded(rawValue: c.decoded)
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        tac = c.has_tac ? c.tac : nil
        nci = c.has_nci ? c.nci : nil
        band = c.has_band ? c.band : nil
        gscn = c.has_gscn ? c.gscn : nil
        frequency = c.frequency
        pci = c.pci
        ssbScs = c.ssb_scs
        beams = c.beams
        sfn = c.has_sfn ? c.sfn : nil
        sib1 = c.sib1
        rsrpDbfs = c.rsrp_dbfs
        rsrqDb = c.rsrq_db
        sinrDb = c.sinr_db
        lastHeard = c.has_last_heard ? c.last_heard : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// The details of a cell of `nr`. The row key is the cell key.
public struct NrDetailRow {
    public enum Sib1: Int32 {
        case noMib = 0
        case notTried = 1
        case notScheduled = 2
        case noFrequency = 3
        case tooNarrow = 4
        case noControl = 5
        case noData = 6
        case unreadable = 7
        case decoded = 8
    }
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// How far the reception of SIB1 went.
    public var sib1: Sib1?
    /// The parts of SIB1 read: 1 the networks, 2 the scheduling, 3 the downlink carrier, 4 the SSB facts.
    public var sib1Parts: Int64?
    /// The width of CORESET#0.
    public var coreset0Width: Double?
    /// The sample rate that holds CORESET#0 with the radio centre of the newest try.
    public var rateNeeded: Double?
    /// subCarrierSpacingCommon of the MIB.
    public var scsCommon: Double?
    /// k_SSB of the MIB.
    public var kSsb: Int64?
    /// cellBarred of the MIB.
    public var barred: Bool?
    /// intraFreqReselection of the MIB is allowed.
    public var intraFreqReselection: Bool?
    /// cellReservedForOtherUse of SIB1.
    public var reservedOtherUse: Bool?
    /// q-RxLevMin of SIB1.
    public var qRxLevMinDbm: Double?
    /// The bands of SIB1, apart by spaces.
    public var bands: String?
    /// The SSB period of SIB1.
    public var ssbPeriod: Double?
    /// The spacing of the carrier of SIB1.
    public var carrierScs: Double?
    /// The width of the carrier of SIB1.
    public var carrierRbs: Int64?
    /// offsetToPointA of SIB1.
    public var offsetToPointA: Int64?
    /// The first block of the initial downlink BWP.
    public var bwpStart: Int64?
    /// The width of the initial downlink BWP.
    public var bwpRbs: Int64?
    /// The SI window, slots.
    public var siWindow: Int64?
    /// Whether SIB1 configures a TDD pattern.
    public var tdd: Bool?
    /// SIB1 decodes.
    public var sib1Count: Int64
    /// The slots of Type0 PDCCH occasions that the newest try searched.
    public var sib1Occasions: Int64?
    /// The control channels with the SI-RNTI that the newest try decoded.
    public var sib1Pdcch: Int64?

    init(_ c: sdrkit_nr_detail_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        sib1 = Sib1(rawValue: c.sib1)
        sib1Parts = c.has_sib1_parts ? c.sib1_parts : nil
        coreset0Width = c.has_coreset0_width ? c.coreset0_width : nil
        rateNeeded = c.has_rate_needed ? c.rate_needed : nil
        scsCommon = c.has_scs_common ? c.scs_common : nil
        kSsb = c.has_k_ssb ? c.k_ssb : nil
        barred = c.has_barred ? c.barred : nil
        intraFreqReselection = c.has_intra_freq_reselection ? c.intra_freq_reselection : nil
        reservedOtherUse = c.has_reserved_other_use ? c.reserved_other_use : nil
        qRxLevMinDbm = c.has_q_rx_lev_min_dbm ? c.q_rx_lev_min_dbm : nil
        bands = c.has_bands ? sdrkitText(c.bands) : nil
        ssbPeriod = c.has_ssb_period ? c.ssb_period : nil
        carrierScs = c.has_carrier_scs ? c.carrier_scs : nil
        carrierRbs = c.has_carrier_rbs ? c.carrier_rbs : nil
        offsetToPointA = c.has_offset_to_point_a ? c.offset_to_point_a : nil
        bwpStart = c.has_bwp_start ? c.bwp_start : nil
        bwpRbs = c.has_bwp_rbs ? c.bwp_rbs : nil
        siWindow = c.has_si_window ? c.si_window : nil
        tdd = c.has_tdd ? c.tdd : nil
        sib1Count = c.sib1_count
        sib1Occasions = c.has_sib1_occasions ? c.sib1_occasions : nil
        sib1Pdcch = c.has_sib1_pdcch ? c.sib1_pdcch : nil
    }
}

/// A network of a cell of `nr`, from SIB1. The row key is the cell key and the place of the network.
public struct NrNetworkRow {
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The network.
    public var plmn: String
    /// The operator of the network.
    public var `operator`: String?
    /// The tracking area code.
    public var tac: Int64?
    /// The NR cell identity (36 bits).
    public var nci: Int64
    /// The gNB identity, when SIB1 gives its length.
    public var gnb: Int64?
    /// The cell identity within the gNB.
    public var cellId: Int64?
    /// cellReservedForOperatorUse.
    public var reservedOperator: Bool

    init(_ c: sdrkit_nr_network_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        plmn = sdrkitText(c.plmn)
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        tac = c.has_tac ? c.tac : nil
        nci = c.nci
        gnb = c.has_gnb ? c.gnb : nil
        cellId = c.has_cell_id ? c.cell_id : nil
        reservedOperator = c.reserved_operator
    }
}

/// A SIB that the SIB1 of a cell of `nr` schedules. The row key is the cell key and the SIB number.
public struct NrSibRow {
    /// The row key.
    public var key: String
    /// The row key of the cell.
    public var cell: String
    /// The SIB number.
    public var sib: Int64
    /// The period of its SI message.
    public var period: Double
    /// Whether the message is broadcast (else sent on request).
    public var broadcasting: Bool
    /// The value tag.
    public var valueTag: Int64?

    init(_ c: sdrkit_nr_sib_row) {
        key = sdrkitText(c.key)
        cell = sdrkitText(c.cell)
        sib = c.sib
        period = c.period
        broadcasting = c.broadcasting
        valueTag = c.has_value_tag ? c.value_tag : nil
    }
}

/// The result fields of `nbiot`.
public struct NbiotFields {
    /// Cells in the table.
    public var cellCount: Int64
    /// Cells with SIB1-NB.
    public var identified: Int64

    init(_ c: sdrkit_nbiot_fields) {
        cellCount = c.cell_count
        identified = c.identified
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NbiotFields, SDRKitCode> {
        var c = sdrkit_nbiot_fields()
        let code = sdrkit_nbiot_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NbiotFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the cells of `nbiot`.
public struct NbiotCellRow {
    public enum OperationMode: Int32 {
        case inbandSamePci = 0
        case inbandDifferentPci = 1
        case guardband = 2
        case standalone = 3
    }
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// The primary network.
    public var plmn: String?
    /// The operator of the primary network.
    public var `operator`: String?
    /// The band.
    public var band: Int64?
    /// The EARFCN.
    public var earfcn: Int64?
    /// The nominal frequency (the offset from 0 Hz when the radio centre is not known).
    public var frequency: Double
    /// The physical cell identity.
    public var pci: Int64
    /// The operation mode.
    public var operationMode: OperationMode?
    /// The eNB identity.
    public var enb: Int64?
    /// The sector.
    public var sector: Int64?
    /// The tracking area code.
    public var tac: Int64?
    /// Whether the cell is barred.
    public var barred: Bool?
    /// The level.
    public var levelDbfs: Double
    /// The SNR.
    public var snrDb: Double
    /// When the cell was last heard.
    public var lastHeard: Double?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_nbiot_cell_row) {
        key = sdrkitText(c.key)
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        band = c.has_band ? c.band : nil
        earfcn = c.has_earfcn ? c.earfcn : nil
        frequency = c.frequency
        pci = c.pci
        operationMode = c.has_operation_mode ? OperationMode(rawValue: c.operation_mode) : nil
        enb = c.has_enb ? c.enb : nil
        sector = c.has_sector ? c.sector : nil
        tac = c.has_tac ? c.tac : nil
        barred = c.has_barred ? c.barred : nil
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        lastHeard = c.has_last_heard ? c.last_heard : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// The result fields of `gsmr`.
public struct GsmrFields {
    /// Cells in the table.
    public var cellCount: Int64
    /// Cells with a location area.
    public var identified: Int64
    /// Cells in the railway band.
    public var railwayCells: Int64

    init(_ c: sdrkit_gsmr_fields) {
        cellCount = c.cell_count
        identified = c.identified
        railwayCells = c.railway_cells
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GsmrFields, SDRKitCode> {
        var c = sdrkit_gsmr_fields()
        let code = sdrkit_gsmr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GsmrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the cells of `gsmr`.
public struct GsmrCellRow {
    public enum Band: Int32 {
        case gsmR = 0
        case gsm900 = 1
        case gsm850 = 2
        case dcs1800 = 3
    }
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// The network.
    public var plmn: String?
    /// The operator of the network.
    public var `operator`: String?
    /// The ARFCN.
    public var arfcn: Int64?
    /// The frequency.
    public var frequency: Double
    /// The base station identity code.
    public var bsic: Int64
    /// The network colour code.
    public var ncc: Int64
    /// The base station colour code.
    public var bcc: Int64
    /// The location area code.
    public var lac: Int64?
    /// The cell identity.
    public var cellId: Int64?
    /// The band of the ARFCN.
    public var band: Band?
    /// The level.
    public var levelDbfs: Double
    /// The SNR.
    public var snrDb: Double
    /// When the cell was last heard.
    public var lastHeard: Double?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_gsmr_cell_row) {
        key = sdrkitText(c.key)
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        arfcn = c.has_arfcn ? c.arfcn : nil
        frequency = c.frequency
        bsic = c.bsic
        ncc = c.ncc
        bcc = c.bcc
        lac = c.has_lac ? c.lac : nil
        cellId = c.has_cell_id ? c.cell_id : nil
        band = c.has_band ? Band(rawValue: c.band) : nil
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        lastHeard = c.has_last_heard ? c.last_heard : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// A carrier of the newest capture of `gsmr`. The row key is its place, strongest first.
public struct GsmrCarrierRow {
    /// The row key.
    public var key: String
    /// The frequency.
    public var frequency: Double
    /// The level over the floor of the spectrum.
    public var levelDb: Double
    /// Whether an SCH decoded on the carrier.
    public var synchronised: Bool

    init(_ c: sdrkit_gsmr_carrier_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        levelDb = c.level_db
        synchronised = c.synchronised
    }
}

/// The result fields of `lbj`.
public struct LbjFields {
    /// Bursts with a sync word.
    public var bursts: Int64
    /// Messages shown.
    public var messages: Int64
    /// Messages refused by the checks.
    public var failedChecks: Int64
    /// Empty frames.
    public var blankFrames: Int64
    /// The level of the channel.
    public var levelDbfs: Double?
    /// The SNR of the channel.
    public var snrDb: Double?
    /// The carrier offset of the newest message.
    public var offsetHz: Double?

    init(_ c: sdrkit_lbj_fields) {
        bursts = c.bursts
        messages = c.messages
        failedChecks = c.failed_checks
        blankFrames = c.blank_frames
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LbjFields, SDRKitCode> {
        var c = sdrkit_lbj_fields()
        let code = sdrkit_lbj_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LbjFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of `lbj`. The row key is the place of the message in the order received.
public struct LbjMessageRow {
    public enum Direction: Int32 {
        case up = 0
        case down = 1
    }
    /// The row key.
    public var key: String
    /// When the message came (wall clock), when known.
    public var received: Double?
    /// The train (prefix and digits), or the network time of a time message.
    public var train: String?
    /// The speed.
    public var speedKmh: Int64?
    /// The kilometre post.
    public var kmPostKm: Double?
    /// Up or down.
    public var direction: Direction?
    /// The locomotive class and number.
    public var locomotive: String?
    /// The route name.
    public var route: String?
    /// The position.
    public var position: SDRKitPosition?
    /// The level of the burst.
    public var levelDbfs: Double

    init(_ c: sdrkit_lbj_message_row) {
        key = sdrkitText(c.key)
        received = c.has_received ? c.received : nil
        train = c.has_train ? sdrkitText(c.train) : nil
        speedKmh = c.has_speed_kmh ? c.speed_kmh : nil
        kmPostKm = c.has_km_post_km ? c.km_post_km : nil
        direction = c.has_direction ? Direction(rawValue: c.direction) : nil
        locomotive = c.has_locomotive ? sdrkitText(c.locomotive) : nil
        route = c.has_route ? sdrkitText(c.route) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        levelDbfs = c.level_dbfs
    }
}

/// The result fields of `ebicab`.
public struct EbicabFields {
    /// Bursts.
    public var bursts: Int64
    /// Telegrams decoded.
    public var telegrams: Int64
    /// Long telegrams.
    public var long: Int64
    /// Short telegrams.
    public var short: Int64
    /// Telegrams that came inverted.
    public var inverted: Int64
    /// Bursts without a telegram.
    public var burstsWithoutTelegram: Int64
    /// Parity hits.
    public var parityHits: Int64
    /// The bit rate of the newest burst.
    public var bitRateHz: Double?
    /// The carrier offset of the newest burst.
    public var offsetHz: Double?
    /// The SNR of the newest burst.
    public var snrDb: Double?

    init(_ c: sdrkit_ebicab_fields) {
        bursts = c.bursts
        telegrams = c.telegrams
        long = c.long_
        short = c.short_
        inverted = c.inverted
        burstsWithoutTelegram = c.bursts_without_telegram
        parityHits = c.parity_hits
        bitRateHz = c.has_bit_rate_hz ? c.bit_rate_hz : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EbicabFields, SDRKitCode> {
        var c = sdrkit_ebicab_fields()
        let code = sdrkit_ebicab_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EbicabFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A telegram of `ebicab`. The row key is the place of the telegram in the order received.
public struct EbicabTelegramRow {
    /// The row key.
    public var key: String
    /// When the telegram came (wall clock), when known.
    public var received: Double?
    /// The length.
    public var lengthBits: Int64
    /// NID_C.
    public var nidC: Int64
    /// NID_BG.
    public var nidBg: Int64
    /// N_PIG.
    public var nPig: Int64
    /// N_TOTAL.
    public var nTotal: Int64
    /// M_VERSION.
    public var mVersion: Int64
    /// The copies received.
    public var copies: Int64
    /// The user bits.
    public var userBits: [UInt8]

    init(_ c: sdrkit_ebicab_telegram_row) {
        key = sdrkitText(c.key)
        received = c.has_received ? c.received : nil
        lengthBits = c.length_bits
        nidC = c.nid_c
        nidBg = c.nid_bg
        nPig = c.n_pig
        nTotal = c.n_total
        mVersion = c.m_version
        copies = c.copies
        userBits = [UInt8](UnsafeBufferPointer(start: c.user_bits.ptr, count: c.user_bits.len))
    }
}

/// A balise of `ebicab` heard.
public struct EbicabBaliseRow {
    /// The row key.
    public var key: String
    /// NID_C.
    public var nidC: Int64
    /// NID_BG.
    public var nidBg: Int64
    /// The place of the balise in its group.
    public var baliseInGroup: Int64
    /// The balises of the group.
    public var groupSize: Int64
    /// When the balise was last heard.
    public var lastHeard: Double?
    /// Times heard.
    public var heard: Int64?

    init(_ c: sdrkit_ebicab_balise_row) {
        key = sdrkitText(c.key)
        nidC = c.nid_c
        nidBg = c.nid_bg
        baliseInGroup = c.balise_in_group
        groupSize = c.group_size
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
    }
}

/// A burst of `ebicab` without a telegram. The row key is the place of the burst in the order received.
public struct EbicabOtherBurstRow {
    public enum Hint: Int32 {
        case ebicab700 = 0
        case ebicab900 = 1
        case shortEurobalise = 2
        case longEurobalise = 3
    }
    /// The row key.
    public var key: String
    /// When the burst came (wall clock), when known.
    public var received: Double?
    /// The bits of the burst.
    public var bits: Int64
    /// The length of the burst.
    public var duration: Double
    /// The SNR.
    public var snrDb: Double
    /// The first bits.
    public var firstBits: [UInt8]
    /// What the length of the burst suggests.
    public var hint: Hint?

    init(_ c: sdrkit_ebicab_other_burst_row) {
        key = sdrkitText(c.key)
        received = c.has_received ? c.received : nil
        bits = c.bits
        duration = c.duration
        snrDb = c.snr_db
        firstBits = [UInt8](UnsafeBufferPointer(start: c.first_bits.ptr, count: c.first_bits.len))
        hint = c.has_hint ? Hint(rawValue: c.hint) : nil
    }
}

/// The result fields of `umts`.
public struct UmtsFields {
    /// Cells in the table.
    public var cellCount: Int64

    init(_ c: sdrkit_umts_fields) {
        cellCount = c.cell_count
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<UmtsFields, SDRKitCode> {
        var c = sdrkit_umts_fields()
        let code = sdrkit_umts_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(UmtsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the cells of `umts`. The row key is the primary scrambling code and the radio centre.
public struct UmtsCellRow {
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// The primary scrambling code.
    public var code: Int64
    /// The code group.
    public var group: Int64
    /// Ec/Io of the CPICH.
    public var ecIoDb: Double
    /// The CPICH level (RSCP).
    public var levelDbfs: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// The frame offset.
    public var frameOffset: Int64
    /// Whether the cell sends with STTD.
    public var sttd: Bool?
    /// Captures in which the cell was seen.
    public var capturesSeen: Int64
    /// BCH blocks decoded.
    public var bchBlocks: Int64
    /// The network.
    public var plmn: String?
    /// The operator of the network.
    public var `operator`: String?
    /// The cell identity.
    public var cellId: Int64?
    /// The RNC and the cell of the identity.
    public var cellIdParts: String?
    /// The UARFCN.
    public var uarfcn: Int64?
    /// When the cell was last heard.
    public var lastHeard: Double?
    /// The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known.
    public var frequency: Double?
    /// The location area code (SIB 1).
    public var lac: Int64?
    /// The routing area code (SIB 1).
    public var rac: Int64?
    /// The cell is barred (SIB 3).
    public var barred: Bool?
    /// The cell is reserved for operator use (SIB 3).
    public var reservedOperator: Bool?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_umts_cell_row) {
        key = sdrkitText(c.key)
        code = c.code
        group = c.group
        ecIoDb = c.ec_io_db
        levelDbfs = c.level_dbfs
        offsetHz = c.offset_hz
        frameOffset = c.frame_offset
        sttd = c.has_sttd ? c.sttd : nil
        capturesSeen = c.captures_seen
        bchBlocks = c.bch_blocks
        plmn = c.has_plmn ? sdrkitText(c.plmn) : nil
        `operator` = c.has_operator_ ? sdrkitText(c.operator_) : nil
        cellId = c.has_cell_id ? c.cell_id : nil
        cellIdParts = c.has_cell_id_parts ? sdrkitText(c.cell_id_parts) : nil
        uarfcn = c.has_uarfcn ? c.uarfcn : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        frequency = c.has_frequency ? c.frequency : nil
        lac = c.has_lac ? c.lac : nil
        rac = c.has_rac ? c.rac : nil
        barred = c.has_barred ? c.barred : nil
        reservedOperator = c.has_reserved_operator ? c.reserved_operator : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// The result fields of `cdma2000`.
public struct Cdma2000Fields {
    /// Base stations in the table.
    public var stationCount: Int64
    /// Base stations with a Sync Channel Message.
    public var identified: Int64

    init(_ c: sdrkit_cdma2000_fields) {
        stationCount = c.station_count
        identified = c.identified
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Cdma2000Fields, SDRKitCode> {
        var c = sdrkit_cdma2000_fields()
        let code = sdrkit_cdma2000_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Cdma2000Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the base stations of `cdma2000`.
public struct Cdma2000CellRow {
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// The PN offset.
    public var pn: String
    /// The system identity.
    public var sid: Int64?
    /// The network identity.
    public var nid: Int64?
    /// Ec/Io of the pilot.
    public var ecIoDb: Double
    /// The pilot level.
    public var levelDbfs: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// The clock error.
    public var clockPpm: Double
    /// The paging channel.
    public var paging: String?
    /// The protocol revisions.
    public var `protocol`: String?
    /// The system time.
    public var systemTime: String?
    /// The local time.
    public var localTime: String?
    /// The long code state.
    public var lcState: String?
    /// Messages good of all.
    public var messages: String
    /// When the base station was last heard.
    public var lastHeard: Double?
    /// The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known.
    public var frequency: Double?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_cdma2000_cell_row) {
        key = sdrkitText(c.key)
        pn = sdrkitText(c.pn)
        sid = c.has_sid ? c.sid : nil
        nid = c.has_nid ? c.nid : nil
        ecIoDb = c.ec_io_db
        levelDbfs = c.level_dbfs
        offsetHz = c.offset_hz
        clockPpm = c.clock_ppm
        paging = c.has_paging ? sdrkitText(c.paging) : nil
        `protocol` = c.has_protocol ? sdrkitText(c.protocol) : nil
        systemTime = c.has_system_time ? sdrkitText(c.system_time) : nil
        localTime = c.has_local_time ? sdrkitText(c.local_time) : nil
        lcState = c.has_lc_state ? sdrkitText(c.lc_state) : nil
        messages = sdrkitText(c.messages)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        frequency = c.has_frequency ? c.frequency : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// The result fields of `tdscdma`.
public struct TdscdmaFields {
    /// Cells in the table.
    public var cellCount: Int64

    init(_ c: sdrkit_tdscdma_fields) {
        cellCount = c.cell_count
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TdscdmaFields, SDRKitCode> {
        var c = sdrkit_tdscdma_fields()
        let code = sdrkit_tdscdma_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TdscdmaFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the cells of `tdscdma`. The row key is the code and the radio centre.
public struct TdscdmaCellRow {
    public enum Generation: Int32 {
        case n2g = 0
        case n3g = 1
        case n4g = 2
        case n5g = 3
    }
    /// The row key.
    public var key: String
    /// The SYNC-DL code.
    public var syncDl: Int64
    /// The cell parameter, "p / p+1" when only the pair is known.
    public var parameter: String?
    /// The DwPTS level.
    public var levelDbfs: Double
    /// The DwPTS SNR.
    public var snrDb: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// The quadruples counted.
    public var quadruples: String
    /// The SFN parity of the first whole frame of the newest capture.
    public var frameParity: String?
    /// The midamble clarity.
    public var clarity: Double
    /// Captures in which the cell was seen.
    public var capturesSeen: Int64
    /// When the cell was last heard.
    public var lastHeard: Double?
    /// The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known.
    public var frequency: Double?
    /// The generation of the radio system.
    public var generation: Generation?
    /// Operator, generation, band and physical identity.
    public var name: String

    init(_ c: sdrkit_tdscdma_cell_row) {
        key = sdrkitText(c.key)
        syncDl = c.sync_dl
        parameter = c.has_parameter ? sdrkitText(c.parameter) : nil
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        quadruples = sdrkitText(c.quadruples)
        frameParity = c.has_frame_parity ? sdrkitText(c.frame_parity) : nil
        clarity = c.clarity
        capturesSeen = c.captures_seen
        lastHeard = c.has_last_heard ? c.last_heard : nil
        frequency = c.has_frequency ? c.frequency : nil
        generation = Generation(rawValue: c.generation)
        name = sdrkitText(c.name)
    }
}

/// The result fields of `eot`.
public struct EotFields {
    /// Frames shown.
    public var framesOk: Int64
    /// Frames the code refused.
    public var framesBad: Int64
    /// Frames with bits repaired.
    public var framesRepaired: Int64
    /// Frame syncs.
    public var syncs: Int64
    /// The unit of the newest frame.
    public var lastUnit: String?
    /// The input level.
    public var levelDbfs: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_eot_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        framesRepaired = c.frames_repaired
        syncs = c.syncs
        lastUnit = c.has_last_unit ? sdrkitText(c.last_unit) : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EotFields, SDRKitCode> {
        var c = sdrkit_eot_fields()
        let code = sdrkit_eot_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EotFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A frame of `eot`, newest first.
public struct EotFrameRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The link: end of train or head of train.
    public var link: String
    /// The unit.
    public var unit: String
    /// The frame for people.
    public var text: String

    init(_ c: sdrkit_eot_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        link = sdrkitText(c.link)
        unit = sdrkitText(c.unit)
        text = sdrkitText(c.text)
    }
}

/// The result fields of `cv2x`.
public struct Cv2xFields {
    /// Whether the sidelink structure is there.
    public var detected: Bool
    /// The SC-FDMA symbol length of the channel width.
    public var symbolUs: Double
    /// The cyclic prefix length of the channel width.
    public var prefixUs: Double
    /// The slot timing of the detection.
    public var slotTiming: Int64?
    /// The prefix correlation of the detection.
    public var correlation: Double?
    /// The carrier offset of the detection.
    public var offsetHz: Double?
    /// Busy subframes a second.
    public var busyPerS: Double?
    /// The share of busy subframes.
    public var busyShare: Double?
    /// Whether the last symbol of the subframe is a guard.
    public var guardSeen: Bool?
    /// The frame shape over the 1 ms subframe.
    public var frame: String?
    /// What the stack shows of the content.
    public var content: String
    /// The summary.
    public var text: String
    /// What no public document read for the stack confirms.
    public var note: String

    init(_ c: sdrkit_cv2x_fields) {
        detected = c.detected
        symbolUs = c.symbol_us
        prefixUs = c.prefix_us
        slotTiming = c.has_slot_timing ? c.slot_timing : nil
        correlation = c.has_correlation ? c.correlation : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        busyPerS = c.has_busy_per_s ? c.busy_per_s : nil
        busyShare = c.has_busy_share ? c.busy_share : nil
        guardSeen = c.has_guard_seen ? c.guard_seen : nil
        frame = c.has_frame ? sdrkitText(c.frame) : nil
        content = sdrkitText(c.content)
        text = sdrkitText(c.text)
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Cv2xFields, SDRKitCode> {
        var c = sdrkit_cv2x_fields()
        let code = sdrkit_cv2x_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Cv2xFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A detection start of `cv2x`, newest first.
public struct Cv2xDetectionRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// The prefix correlation.
    public var correlation: Double
    /// The slot timing.
    public var slotTiming: Int64

    init(_ c: sdrkit_cv2x_detection_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        offsetHz = c.offset_hz
        correlation = c.correlation
        slotTiming = c.slot_timing
    }
}

/// A message of the end-of-train device.
public struct RailwayCirKlwEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The summary: `KLW <action> <locomotive>`.
    public var text: String
    /// The locomotive.
    public var locomotive: String
    /// The reserved command byte.
    public var reserved: Int64
    /// The action code.
    public var actionCode: Int64
    /// The action.
    public var action: String
    /// The parameter as 4 characters.
    public var parameter: String?
    /// The brake pipe pressure of a pressure action.
    public var pressureKpa: Double?
    /// The tail device id.
    public var tailId: String?
    /// Extra bytes as hex.
    public var extra: String?
    /// The calibration clock, `YYYY-MM-DDThh:mm:ss`.
    public var clock: String?
    /// The bits corrected.
    public var corrections: Int64
    /// The raw frame as hex.
    public var raw: String

    init(_ c: sdrkit_railway_cir_klw_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        locomotive = sdrkitText(c.locomotive)
        reserved = c.reserved
        actionCode = c.action_code
        action = sdrkitText(c.action)
        parameter = c.has_parameter ? sdrkitText(c.parameter) : nil
        pressureKpa = c.has_pressure_kpa ? c.pressure_kpa : nil
        tailId = c.has_tail_id ? sdrkitText(c.tail_id) : nil
        extra = c.has_extra ? sdrkitText(c.extra) : nil
        clock = c.has_clock ? sdrkitText(c.clock) : nil
        corrections = c.corrections
        raw = sdrkitText(c.raw)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RailwayCirKlwEvent, SDRKitCode> {
        var c = sdrkit_railway_cir_klw_event()
        let code = sdrkit_railway_cir_klw_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RailwayCirKlwEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A dispatch message.
public struct RailwayCirTdcsEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The summary: `TDCS <train> <locomotive>`.
    public var text: String
    /// The train.
    public var train: String
    /// The locomotive.
    public var locomotive: String
    /// The locomotive type code.
    public var locoCode: Int64
    /// The locomotive serial.
    public var locoSerial: Int64
    /// The kilometre post.
    public var kmPostKm: Double?
    /// The mileage increases.
    public var increasing: Bool
    /// The frame marks itself as a test frame.
    public var testFrame: Bool
    /// The speed.
    public var speedKmh: Double
    /// The weight.
    public var weightT: Double
    /// The length field divided by 10 (its unit is not published).
    public var length: Double
    /// The car count.
    public var cars: Int64
    /// The station.
    public var station: Int64
    /// The track.
    public var track: Int64
    /// The actual track.
    public var actualTrack: Int64
    /// The driver id.
    public var driver: Int64
    /// The signal type.
    public var signalType: Int64
    /// The semaphore type.
    public var semaphoreType: Int64
    /// The locomotive role.
    public var role: Int64
    /// The bits corrected.
    public var corrections: Int64
    /// The raw frame as hex.
    public var raw: String

    init(_ c: sdrkit_railway_cir_tdcs_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        train = sdrkitText(c.train)
        locomotive = sdrkitText(c.locomotive)
        locoCode = c.loco_code
        locoSerial = c.loco_serial
        kmPostKm = c.has_km_post_km ? c.km_post_km : nil
        increasing = c.increasing
        testFrame = c.test_frame
        speedKmh = c.speed_kmh
        weightT = c.weight_t
        length = c.length
        cars = c.cars
        station = c.station
        track = c.track
        actualTrack = c.actual_track
        driver = c.driver
        signalType = c.signal_type
        semaphoreType = c.semaphore_type
        role = c.role
        corrections = c.corrections
        raw = sdrkitText(c.raw)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RailwayCirTdcsEvent, SDRKitCode> {
        var c = sdrkit_railway_cir_tdcs_event()
        let code = sdrkit_railway_cir_tdcs_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RailwayCirTdcsEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellLteCellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_lte_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellLteCellEvent, SDRKitCode> {
        var c = sdrkit_cell_lte_cell_event()
        let code = sdrkit_cell_lte_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellLteCellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellNrCellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_nr_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellNrCellEvent, SDRKitCode> {
        var c = sdrkit_cell_nr_cell_event()
        let code = sdrkit_cell_nr_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellNrCellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellNbiotCellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_nbiot_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellNbiotCellEvent, SDRKitCode> {
        var c = sdrkit_cell_nbiot_cell_event()
        let code = sdrkit_cell_nbiot_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellNbiotCellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellGsmCellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_gsm_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellGsmCellEvent, SDRKitCode> {
        var c = sdrkit_cell_gsm_cell_event()
        let code = sdrkit_cell_gsm_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellGsmCellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A message of `lbj` shown or completed by its pair.
public struct LbjMessageEvent {
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
    public enum Direction: Int32 {
        case up = 0
        case down = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The train, or the network time of a time message.
    public var source: String?
    /// The message for people.
    public var text: String
    /// The family of the source.
    public var family: Family?
    /// The position.
    public var position: SDRKitPosition?
    /// The speed in knots.
    public var speedKt: Double?
    /// The speed.
    public var speedKmh: Int64?
    /// The kilometre post.
    public var kmPostKm: Double?
    /// Up or down.
    public var direction: Direction?
    /// The locomotive class and number.
    public var locomotive: String?
    /// The route name.
    public var route: String?

    init(_ c: sdrkit_lbj_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        speedKmh = c.has_speed_kmh ? c.speed_kmh : nil
        kmPostKm = c.has_km_post_km ? c.km_post_km : nil
        direction = c.has_direction ? Direction(rawValue: c.direction) : nil
        locomotive = c.has_locomotive ? sdrkitText(c.locomotive) : nil
        route = c.has_route ? sdrkitText(c.route) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<LbjMessageEvent, SDRKitCode> {
        var c = sdrkit_lbj_message_event()
        let code = sdrkit_lbj_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(LbjMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A new telegram of `ebicab`.
public struct EbicabBaliseTelegramEvent {
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
    /// The balise.
    public var source: String
    /// The telegram for people.
    public var text: String
    /// The family of the source.
    public var family: Family?
    /// The length.
    public var lengthBits: Int64
    /// NID_C.
    public var nidC: Int64
    /// NID_BG.
    public var nidBg: Int64
    /// N_PIG.
    public var nPig: Int64

    init(_ c: sdrkit_ebicab_balise_telegram_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        lengthBits = c.length_bits
        nidC = c.nid_c
        nidBg = c.nid_bg
        nPig = c.n_pig
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EbicabBaliseTelegramEvent, SDRKitCode> {
        var c = sdrkit_ebicab_balise_telegram_event()
        let code = sdrkit_ebicab_balise_telegram_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EbicabBaliseTelegramEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellUmtsCellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_umts_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellUmtsCellEvent, SDRKitCode> {
        var c = sdrkit_cell_umts_cell_event()
        let code = sdrkit_cell_umts_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellUmtsCellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellCdma2000CellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_cdma2000_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellCdma2000CellEvent, SDRKitCode> {
        var c = sdrkit_cell_cdma2000_cell_event()
        let code = sdrkit_cell_cdma2000_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellCdma2000CellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A cell found or identified by a search stack.
public struct CellTdscdmaCellEvent {
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
    /// The row key of the cell in the `cells` table.
    public var source: String
    /// What was found.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_cell_tdscdma_cell_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<CellTdscdmaCellEvent, SDRKitCode> {
        var c = sdrkit_cell_tdscdma_cell_event()
        let code = sdrkit_cell_tdscdma_cell_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(CellTdscdmaCellEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// One frame with every field. The fields of the end-of-train message are absent in a head-of-train frame.
public struct EotFrameEvent {
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
    /// The link.
    public var link: String
    /// The unit.
    public var source: String
    /// The message type.
    public var messageType: Int64
    /// The chaining bits.
    public var chaining: Int64?
    /// The battery condition.
    public var battery: Int64?
    /// The brake pipe pressure.
    public var pressurePsi: Int64?
    /// The battery charge.
    public var chargePct: Int64?
    /// The spare bit.
    public var spare: Bool?
    /// The valve is open.
    public var valveOpen: Bool?
    /// The confirmation bit.
    public var confirmation: Bool?
    /// The turbine runs.
    public var turbine: Bool?
    /// The train moves.
    public var moving: Bool?
    /// The marker battery is good.
    public var markerBattery: Bool?
    /// The marker light is on.
    public var markerLight: Bool?
    /// The bits repaired.
    public var bitsCorrected: Int64
    /// The frame for people.
    public var text: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_eot_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        link = sdrkitText(c.link)
        source = sdrkitText(c.source)
        messageType = c.message_type
        chaining = c.has_chaining ? c.chaining : nil
        battery = c.has_battery ? c.battery : nil
        pressurePsi = c.has_pressure_psi ? c.pressure_psi : nil
        chargePct = c.has_charge_pct ? c.charge_pct : nil
        spare = c.has_spare ? c.spare : nil
        valveOpen = c.has_valve_open ? c.valve_open : nil
        confirmation = c.has_confirmation ? c.confirmation : nil
        turbine = c.has_turbine ? c.turbine : nil
        moving = c.has_moving ? c.moving : nil
        markerBattery = c.has_marker_battery ? c.marker_battery : nil
        markerLight = c.has_marker_light ? c.marker_light : nil
        bitsCorrected = c.bits_corrected
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EotFrameEvent, SDRKitCode> {
        var c = sdrkit_eot_frame_event()
        let code = sdrkit_eot_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EotFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "lbj" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct LbjDropped {
    /// The items that the component "modem" dropped.
    public var modem: Int64?

    init(_ c: sdrkit_lbj_dropped) {
        modem = c.has_modem ? c.modem : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LbjDropped, SDRKitCode> {
        var c = sdrkit_lbj_dropped()
        let code = sdrkit_lbj_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(LbjDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ebicab" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct EbicabDropped {
    /// The items that the component "uplink" dropped.
    public var uplink: Int64?

    init(_ c: sdrkit_ebicab_dropped) {
        uplink = c.has_uplink ? c.uplink : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EbicabDropped, SDRKitCode> {
        var c = sdrkit_ebicab_dropped()
        let code = sdrkit_ebicab_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(EbicabDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "eot" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct EotDropped {
    /// The items that the component "modem" dropped.
    public var modem: Int64?

    init(_ c: sdrkit_eot_dropped) {
        modem = c.has_modem ? c.modem : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EotDropped, SDRKitCode> {
        var c = sdrkit_eot_dropped()
        let code = sdrkit_eot_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(EotDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum CellTables {
    /// The changed rows of the table "messages" of the stack "railway_cir", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `received`.
    public static func railwayCirMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 2000) -> Result<[RailwayCirMessageRow], SDRKitCode> {
        var rows = [sdrkit_railway_cir_message_row](repeating: sdrkit_railway_cir_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_railway_cir_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RailwayCirMessageRow.init))
    }
    /// The changed rows of the table "cells" of the stack "cell", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func cellCells(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[CellRow], SDRKitCode> {
        var rows = [sdrkit_cell_row](repeating: sdrkit_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellRow.init))
    }
    /// The changed rows of the table "details" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[CellDetailRow], SDRKitCode> {
        var rows = [sdrkit_cell_detail_row](repeating: sdrkit_cell_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellDetailRow.init))
    }
    /// The changed rows of the table "cell_plmns" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellPlmns(_ handle: SDRKIT_HANDLE, capacity: Int = 1600) -> Result<[CellPlmnRow], SDRKitCode> {
        var rows = [sdrkit_cell_plmn_row](repeating: sdrkit_cell_plmn_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_plmns_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellPlmnRow.init))
    }
    /// The changed rows of the table "cell_bands" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellBands(_ handle: SDRKIT_HANDLE, capacity: Int = 1600) -> Result<[CellBandRow], SDRKitCode> {
        var rows = [sdrkit_cell_band_row](repeating: sdrkit_cell_band_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_bands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellBandRow.init))
    }
    /// The changed rows of the table "cell_sibs" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellSibs(_ handle: SDRKIT_HANDLE, capacity: Int = 1600) -> Result<[CellSibRow], SDRKitCode> {
        var rows = [sdrkit_cell_sib_row](repeating: sdrkit_cell_sib_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_sibs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellSibRow.init))
    }
    /// The changed rows of the table "neighbours" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellNeighbours(_ handle: SDRKIT_HANDLE, capacity: Int = 1600) -> Result<[CellNeighbourRow], SDRKitCode> {
        var rows = [sdrkit_cell_neighbour_row](repeating: sdrkit_cell_neighbour_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_neighbours_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellNeighbourRow.init))
    }
    /// The changed rows of the table "inter_carriers" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellInterCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 1600) -> Result<[CellInterCarrierRow], SDRKitCode> {
        var rows = [sdrkit_cell_inter_carrier_row](repeating: sdrkit_cell_inter_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_inter_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellInterCarrierRow.init))
    }
    /// The changed rows of the table "sites" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellSites(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[CellSiteRow], SDRKitCode> {
        var rows = [sdrkit_cell_site_row](repeating: sdrkit_cell_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellSiteRow.init))
    }
    /// The changed rows of the table "operators" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellOperators(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[CellOperatorRow], SDRKitCode> {
        var rows = [sdrkit_cell_operator_row](repeating: sdrkit_cell_operator_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_operators_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellOperatorRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "cell", newest first, at most `capacity` rows.
    /// Default order: the column `level_db`, highest first.
    public static func cellCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[CellCarrierRow], SDRKitCode> {
        var rows = [sdrkit_cell_carrier_row](repeating: sdrkit_cell_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellCarrierRow.init))
    }
    /// The changed rows of the table "other_carriers" of the stack "cell", newest first, at most `capacity` rows.
    public static func cellOtherCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 1600) -> Result<[CellOtherCarrierRow], SDRKitCode> {
        var rows = [sdrkit_cell_other_carrier_row](repeating: sdrkit_cell_other_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_other_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellOtherCarrierRow.init))
    }
    /// The changed rows of the table "survey" of the stack "cell", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func cellSurvey(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[CellSurveyRow], SDRKitCode> {
        var rows = [sdrkit_cell_survey_row](repeating: sdrkit_cell_survey_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cell_survey_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellSurveyRow.init))
    }
    /// The changed rows of the table "cells" of the stack "nr", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func nrCells(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[NrCellRow], SDRKitCode> {
        var rows = [sdrkit_nr_cell_row](repeating: sdrkit_nr_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nr_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NrCellRow.init))
    }
    /// The changed rows of the table "nr_details" of the stack "nr", newest first, at most `capacity` rows.
    public static func nrDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[NrDetailRow], SDRKitCode> {
        var rows = [sdrkit_nr_detail_row](repeating: sdrkit_nr_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nr_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NrDetailRow.init))
    }
    /// The changed rows of the table "nr_networks" of the stack "nr", newest first, at most `capacity` rows.
    public static func nrNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 1200) -> Result<[NrNetworkRow], SDRKitCode> {
        var rows = [sdrkit_nr_network_row](repeating: sdrkit_nr_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nr_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NrNetworkRow.init))
    }
    /// The changed rows of the table "nr_sibs" of the stack "nr", newest first, at most `capacity` rows.
    public static func nrSibs(_ handle: SDRKIT_HANDLE, capacity: Int = 2400) -> Result<[NrSibRow], SDRKitCode> {
        var rows = [sdrkit_nr_sib_row](repeating: sdrkit_nr_sib_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nr_sibs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NrSibRow.init))
    }
    /// The changed rows of the table "survey" of the stack "nr", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func nrSurvey(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[CellSurveyRow], SDRKitCode> {
        var rows = [sdrkit_cell_survey_row](repeating: sdrkit_cell_survey_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nr_survey_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellSurveyRow.init))
    }
    /// The changed rows of the table "cells" of the stack "nbiot", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func nbiotCells(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[NbiotCellRow], SDRKitCode> {
        var rows = [sdrkit_nbiot_cell_row](repeating: sdrkit_nbiot_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nbiot_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NbiotCellRow.init))
    }
    /// The changed rows of the table "cells" of the stack "gsmr", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func gsmrCells(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[GsmrCellRow], SDRKitCode> {
        var rows = [sdrkit_gsmr_cell_row](repeating: sdrkit_gsmr_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gsmr_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GsmrCellRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "gsmr", newest first, at most `capacity` rows.
    /// Default order: the column `level_db`, highest first.
    public static func gsmrCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[GsmrCarrierRow], SDRKitCode> {
        var rows = [sdrkit_gsmr_carrier_row](repeating: sdrkit_gsmr_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gsmr_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GsmrCarrierRow.init))
    }
    /// The changed rows of the table "survey" of the stack "gsmr", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func gsmrSurvey(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[CellSurveyRow], SDRKitCode> {
        var rows = [sdrkit_cell_survey_row](repeating: sdrkit_cell_survey_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gsmr_survey_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellSurveyRow.init))
    }
    /// The changed rows of the table "messages" of the stack "lbj", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Place of a row: the column `position`.
    /// Speed: `speed_kmh`.
    /// Time: `received`.
    public static func lbjMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[LbjMessageRow], SDRKitCode> {
        var rows = [sdrkit_lbj_message_row](repeating: sdrkit_lbj_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lbj_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LbjMessageRow.init))
    }
    /// The changed rows of the table "telegrams" of the stack "ebicab", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `received`.
    public static func ebicabTelegrams(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[EbicabTelegramRow], SDRKitCode> {
        var rows = [sdrkit_ebicab_telegram_row](repeating: sdrkit_ebicab_telegram_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ebicab_telegrams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EbicabTelegramRow.init))
    }
    /// The changed rows of the table "balises" of the stack "ebicab", newest first, at most `capacity` rows.
    public static func ebicabBalises(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[EbicabBaliseRow], SDRKitCode> {
        var rows = [sdrkit_ebicab_balise_row](repeating: sdrkit_ebicab_balise_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ebicab_balises_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EbicabBaliseRow.init))
    }
    /// The changed rows of the table "other_bursts" of the stack "ebicab", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `received`.
    public static func ebicabOtherBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[EbicabOtherBurstRow], SDRKitCode> {
        var rows = [sdrkit_ebicab_other_burst_row](repeating: sdrkit_ebicab_other_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ebicab_other_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EbicabOtherBurstRow.init))
    }
    /// The changed rows of the table "cells" of the stack "umts", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func umtsCells(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[UmtsCellRow], SDRKitCode> {
        var rows = [sdrkit_umts_cell_row](repeating: sdrkit_umts_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_umts_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(UmtsCellRow.init))
    }
    /// The changed rows of the table "survey" of the stack "umts", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func umtsSurvey(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[CellSurveyRow], SDRKitCode> {
        var rows = [sdrkit_cell_survey_row](repeating: sdrkit_cell_survey_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_umts_survey_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CellSurveyRow.init))
    }
    /// The changed rows of the table "cells" of the stack "cdma2000", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func cdma2000Cells(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Cdma2000CellRow], SDRKitCode> {
        var rows = [sdrkit_cdma2000_cell_row](repeating: sdrkit_cdma2000_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cdma2000_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Cdma2000CellRow.init))
    }
    /// The changed rows of the table "cells" of the stack "tdscdma", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Time: `last_heard`.
    public static func tdscdmaCells(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[TdscdmaCellRow], SDRKitCode> {
        var rows = [sdrkit_tdscdma_cell_row](repeating: sdrkit_tdscdma_cell_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tdscdma_cells_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(TdscdmaCellRow.init))
    }
    /// The changed rows of the table "frames" of the stack "eot", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func eotFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[EotFrameRow], SDRKitCode> {
        var rows = [sdrkit_eot_frame_row](repeating: sdrkit_eot_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_eot_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EotFrameRow.init))
    }
    /// The changed rows of the table "detections" of the stack "cv2x", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func cv2xDetections(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[Cv2xDetectionRow], SDRKitCode> {
        var rows = [sdrkit_cv2x_detection_row](repeating: sdrkit_cv2x_detection_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cv2x_detections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Cv2xDetectionRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum CellCommands {
    /// Clear. Forgets everything decoded so far.
    public static func railwayCirClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_railway_cir_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cellClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cell_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func cellRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_cell_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Search the bands. Steps the radio over the raster of the bands of the setting search_bands (empty: the bands of the band order, else the band of the tuned frequency) and lists every cell found. With band_search on auto the search stays on the strongest cell after one pass.
    public static func cellSearch(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cell_search(handle)
        return sdrkitResult(code)
    }
    /// Stay on a cell. Ends the band search and tunes to the carrier of a cell of the cells table, or of the strongest cell found.
    public static func cellStay(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_stay_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_stay_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_cell_stay(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func nrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nr_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func nrRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_nr_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Search the bands. Steps the radio over the raster of the bands of the setting search_bands (empty: the bands of the band order, else the band of the tuned frequency) and lists every cell found. With band_search on auto the search stays on the strongest cell after one pass.
    public static func nrSearch(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nr_search(handle)
        return sdrkitResult(code)
    }
    /// Stay on a cell. Ends the band search and tunes to the carrier of a cell of the cells table, or of the strongest cell found.
    public static func nrStay(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_stay_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_stay_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_nr_stay(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func nbiotClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nbiot_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func nbiotRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_nbiot_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func gsmrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gsmr_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func gsmrRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_gsmr_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func lbjClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lbj_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ebicabClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ebicab_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ebicabRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ebicab_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_ebicab_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ebicab_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func umtsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_umts_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func umtsRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_umts_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cdma2000Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cdma2000_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func cdma2000Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_cdma2000_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func tdscdmaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tdscdma_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func tdscdmaRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_cell_search_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_cell_search_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_tdscdma_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func eotClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_eot_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cv2xClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cv2x_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum CellSettings {
    /// Messages kept.
    public static func railwayCirHistory(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "history", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRailwayCirHistory(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "history", value))
    }
    /// Locomotive classes.
    public static func railwayCirLocomotives(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "locomotives", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRailwayCirLocomotives(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "locomotives", $0) })
    }
    /// Capture length.
    public static func cellCaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellCaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// Read system information.
    public static func cellSystemInformation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "system_information", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellSystemInformation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "system_information", value))
    }
    /// Cells per carrier.
    public static func cellMaxCells(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_cells", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellMaxCells(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_cells", value))
    }
    /// PSS threshold.
    public static func cellMinPssDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_pss_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellMinPssDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_pss_db", value))
    }
    /// Carrier offset searched.
    public static func cellMaxCfoKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_cfo_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellMaxCfoKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_cfo_khz", value))
    }
    /// Hide bands.
    public static func cellHideBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hide_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCellHideBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hide_bands", $0) })
    }
    /// Only bands.
    public static func cellOnlyBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "only_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCellOnlyBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "only_bands", $0) })
    }
    /// Search.
    public static func cellSearch(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "search", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCellSearch(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "search", $0) })
    }
    /// Minimum RSRP.
    public static func cellMinRsrp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_rsrp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellMinRsrp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_rsrp", value))
    }
    /// Band confirmed only.
    public static func cellBandKnown(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "band_known", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellBandKnown(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "band_known", value))
    }
    /// Identified only.
    public static func cellIdentified(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "identified", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellIdentified(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "identified", value))
    }
    /// Leave out barred cells.
    public static func cellNotBarred(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "not_barred", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellNotBarred(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "not_barred", value))
    }
    /// One row per cell.
    public static func cellOnePerCell(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "one_per_cell", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellOnePerCell(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "one_per_cell", value))
    }
    /// Band order.
    public static func cellBandOrder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_order", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCellBandOrder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_order", $0) })
    }
    /// The options of the setting "band_search" of the stack "cell".
    public enum CellBandSearch: Int32 {
        case auto = 0
        case survey = 1
        case off = 2
    }
    /// Band search.
    public static func cellBandSearch(_ handle: SDRKIT_HANDLE) -> Result<CellBandSearch, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band_search", &v)
        return code == SDRKIT_OK ? .success(CellBandSearch(rawValue: v) ?? CellBandSearch(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCellBandSearch(_ handle: SDRKIT_HANDLE, _ value: CellBandSearch) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band_search", value.rawValue))
    }
    /// Bands searched.
    public static func cellSearchBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "search_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCellSearchBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "search_bands", $0) })
    }
    /// Searched first.
    public static func cellSearchFirst(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "search_first", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCellSearchFirst(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "search_first", $0) })
    }
    /// Time per step.
    public static func cellSearchDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellSearchDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_dwell_s", value))
    }
    /// Capture length.
    public static func nrCaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrCaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// Cells per candidate.
    public static func nrMaxCells(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_cells", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrMaxCells(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_cells", value))
    }
    /// PSS threshold.
    public static func nrMinPssDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_pss_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrMinPssDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_pss_db", value))
    }
    /// Hide bands.
    public static func nrHideBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hide_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNrHideBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hide_bands", $0) })
    }
    /// Only bands.
    public static func nrOnlyBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "only_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNrOnlyBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "only_bands", $0) })
    }
    /// Minimum RSRP.
    public static func nrMinRsrp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_rsrp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrMinRsrp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_rsrp", value))
    }
    /// Only cells with a MIB.
    public static func nrMibOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mib_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrMibOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mib_only", value))
    }
    /// Leave out barred cells.
    public static func nrNotBarred(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "not_barred", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrNotBarred(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "not_barred", value))
    }
    /// Band order.
    public static func nrBandOrder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_order", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNrBandOrder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_order", $0) })
    }
    /// Symbol phase of up-conversion.
    public static func nrSymbolPhase(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "symbol_phase", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrSymbolPhase(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "symbol_phase", value))
    }
    /// The options of the setting "band_search" of the stack "nr".
    public enum NrBandSearch: Int32 {
        case auto = 0
        case survey = 1
        case off = 2
    }
    /// Band search.
    public static func nrBandSearch(_ handle: SDRKIT_HANDLE) -> Result<NrBandSearch, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band_search", &v)
        return code == SDRKIT_OK ? .success(NrBandSearch(rawValue: v) ?? NrBandSearch(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNrBandSearch(_ handle: SDRKIT_HANDLE, _ value: NrBandSearch) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band_search", value.rawValue))
    }
    /// Bands searched.
    public static func nrSearchBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "search_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNrSearchBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "search_bands", $0) })
    }
    /// Searched first.
    public static func nrSearchFirst(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "search_first", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNrSearchFirst(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "search_first", $0) })
    }
    /// Time per step.
    public static func nrSearchDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrSearchDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_dwell_s", value))
    }
    /// Capture length.
    public static func nbiotCaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotCaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// NPSS threshold.
    public static func nbiotMinNpss(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_npss", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotMinNpss(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_npss", value))
    }
    /// Read SIB1-NB.
    public static func nbiotReadSib1(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "read_sib1", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotReadSib1(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "read_sib1", value))
    }
    /// Radio frequency correction.
    public static func nbiotFreqCorrectionPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "freq_correction_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotFreqCorrectionPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "freq_correction_ppm", value))
    }
    /// Band order.
    public static func nbiotBandOrder(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "band_order", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotBandOrder(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "band_order", $0) })
    }
    /// Capture length.
    public static func gsmrCaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGsmrCaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// Carriers per capture.
    public static func gsmrMaxCarriers(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGsmrMaxCarriers(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_carriers", value))
    }
    /// The options of the setting "gsm_band" of the stack "gsmr".
    public enum GsmrGsmBand: Int32 {
        case n900Mhz = 0
        case all = 1
    }
    /// Band raster.
    public static func gsmrGsmBand(_ handle: SDRKIT_HANDLE) -> Result<GsmrGsmBand, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gsm_band", &v)
        return code == SDRKIT_OK ? .success(GsmrGsmBand(rawValue: v) ?? GsmrGsmBand(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGsmrGsmBand(_ handle: SDRKIT_HANDLE, _ value: GsmrGsmBand) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gsm_band", value.rawValue))
    }
    /// Locomotive classes.
    public static func lbjLocomotives(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "locomotives", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLbjLocomotives(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "locomotives", $0) })
    }
    /// Capture length.
    public static func umtsCaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUmtsCaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// Carrier search range.
    public static func umtsMaxOffsetHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_offset_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUmtsMaxOffsetHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_offset_hz", value))
    }
    /// Cells per capture.
    public static func umtsMaxCells(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_cells", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUmtsMaxCells(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_cells", value))
    }
    /// Read the broadcast channel.
    public static func umtsReadBch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "read_bch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUmtsReadBch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "read_bch", value))
    }
    /// Pulse roll-off.
    public static func umtsRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setUmtsRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// Capture length.
    public static func cdma2000CaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCdma2000CaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// The options of the setting "band_class" of the stack "cdma2000".
    public enum Cdma2000BandClass: Int32 {
        case none = 0
        case n0 = 1
        case n1 = 2
        case n4 = 3
    }
    /// Band class.
    public static func cdma2000BandClass(_ handle: SDRKIT_HANDLE) -> Result<Cdma2000BandClass, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band_class", &v)
        return code == SDRKIT_OK ? .success(Cdma2000BandClass(rawValue: v) ?? Cdma2000BandClass(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdma2000BandClass(_ handle: SDRKIT_HANDLE, _ value: Cdma2000BandClass) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band_class", value.rawValue))
    }
    /// Carrier search range.
    public static func cdma2000MaxOffsetHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_offset_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCdma2000MaxOffsetHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_offset_hz", value))
    }
    /// Pilots per capture.
    public static func cdma2000MaxCells(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_cells", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCdma2000MaxCells(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_cells", value))
    }
    /// Pilot threshold.
    public static func cdma2000Threshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCdma2000Threshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// Capture length.
    public static func tdscdmaCaptureS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "capture_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTdscdmaCaptureS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "capture_s", value))
    }
    /// SYNC-DL threshold.
    public static func tdscdmaThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTdscdmaThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// Cells per capture.
    public static func tdscdmaMaxCells(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_cells", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTdscdmaMaxCells(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_cells", value))
    }
    /// Pulse roll-off.
    public static func tdscdmaRollOff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTdscdmaRollOff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "roll_off", value))
    }
    /// The options of the setting "direction" of the stack "eot".
    public enum EotDirection: Int32 {
        case eot = 0
        case hot = 1
    }
    /// Link.
    public static func eotDirection(_ handle: SDRKIT_HANDLE) -> Result<EotDirection, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "direction", &v)
        return code == SDRKIT_OK ? .success(EotDirection(rawValue: v) ?? EotDirection(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEotDirection(_ handle: SDRKIT_HANDLE, _ value: EotDirection) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "direction", value.rawValue))
    }
    /// The options of the setting "channel_mhz" of the stack "cv2x".
    public enum Cv2xChannelMhz: Int32 {
        case n10Mhz = 0
        case n20Mhz = 1
    }
    /// Channel width.
    public static func cv2xChannelMhz(_ handle: SDRKIT_HANDLE) -> Result<Cv2xChannelMhz, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel_mhz", &v)
        return code == SDRKIT_OK ? .success(Cv2xChannelMhz(rawValue: v) ?? Cv2xChannelMhz(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCv2xChannelMhz(_ handle: SDRKIT_HANDLE, _ value: Cv2xChannelMhz) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel_mhz", value.rawValue))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum CellGeneratorSettings {
    /// Invert the air bits.
    public static func railwayCirInvertBits(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRailwayCirInvertBits(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert_bits", value))
    }
    /// Tone deviation.
    public static func railwayCirDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRailwayCirDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// The options of the setting "gen_scene" of the stack "cell".
    public enum CellScene: Int32 {
        case n10Mhz2Ports = 0
        case n5Mhz4Ports = 1
        case n14Mhz = 2
        case n20Mhz = 3
    }
    /// Test signal scene.
    public static func cellScene(_ handle: SDRKIT_HANDLE) -> Result<CellScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(CellScene(rawValue: v) ?? CellScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCellScene(_ handle: SDRKIT_HANDLE, _ value: CellScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal first SFN.
    public static func cellSfn0(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_sfn0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellSfn0(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_sfn0", value))
    }
    /// Test signal load.
    public static func cellLoad(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_load", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCellLoad(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_load", value))
    }
    /// The options of the setting "gen_scene" of the stack "nr".
    public enum NrScene: Int32 {
        case twoN78Cells = 0
        case n28Cell = 1
        case barredN78Cell = 2
        case n5Cell30KhzBlocks = 3
    }
    /// Test signal scene.
    public static func nrScene(_ handle: SDRKIT_HANDLE) -> Result<NrScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(NrScene(rawValue: v) ?? NrScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNrScene(_ handle: SDRKIT_HANDLE, _ value: NrScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// The options of the setting "gen_scene" of the stack "nbiot".
    public enum NbiotScene: Int32 {
        case standalone = 0
        case guardBand = 1
        case inBandSamePci = 2
        case inBandOtherPci = 3
    }
    /// Test signal scene.
    public static func nbiotScene(_ handle: SDRKIT_HANDLE) -> Result<NbiotScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(NbiotScene(rawValue: v) ?? NbiotScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotScene(_ handle: SDRKIT_HANDLE, _ value: NbiotScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal load.
    public static func nbiotLoad(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_load", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotLoad(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_load", value))
    }
    /// Test signal NPBCH rotation.
    public static func nbiotNpbchRotation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_npbch_rotation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNbiotNpbchRotation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_npbch_rotation", value))
    }
    /// The options of the setting "gen_scene" of the stack "gsmr".
    public enum GsmrScene: Int32 {
        case twoCells = 0
        case bandEdge = 1
        case threeCells = 2
    }
    /// Test signal scene.
    public static func gsmrScene(_ handle: SDRKIT_HANDLE) -> Result<GsmrScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(GsmrScene(rawValue: v) ?? GsmrScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGsmrScene(_ handle: SDRKIT_HANDLE, _ value: GsmrScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal radio centre.
    public static func gsmrCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGsmrCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test signal deviation.
    public static func lbjDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLbjDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// The options of the setting "gen_scene" of the stack "ebicab".
    public enum EbicabScene: Int32 {
        case longTelegrams = 0
        case shortTelegram = 1
        case allThreeInTurn = 2
        case randomBits = 3
    }
    /// Test signal scene.
    public static func ebicabScene(_ handle: SDRKIT_HANDLE) -> Result<EbicabScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(EbicabScene(rawValue: v) ?? EbicabScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEbicabScene(_ handle: SDRKIT_HANDLE, _ value: EbicabScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Test signal burst.
    public static func ebicabBurstMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_burst_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEbicabBurstMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_burst_ms", value))
    }
    /// Test signal gap.
    public static func ebicabGapMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEbicabGapMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_ms", value))
    }
    /// Test signal random bits per burst.
    public static func ebicabRandomBits(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_random_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEbicabRandomBits(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_random_bits", value))
    }
    /// Test signal bit rate error.
    public static func ebicabBitRatePct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bit_rate_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEbicabBitRatePct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bit_rate_pct", value))
    }
    /// Test signal code multipliers.
    public static func tdscdmaMultipliers(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_multipliers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTdscdmaMultipliers(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_multipliers", value))
    }
    /// Test signal unit.
    public static func eotUnit(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_unit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEotUnit(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_unit", value))
    }
    /// Test signal brake pipe.
    public static func eotPsi(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_psi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEotPsi(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_psi", value))
    }
    /// Test signal message type.
    public static func eotType(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_type", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEotType(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_type", value))
    }
    /// Test signal moving.
    public static func eotMotion(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_motion", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEotMotion(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_motion", value))
    }
    /// Test signal busy share.
    public static func cv2xBusy(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_busy", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCv2xBusy(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_busy", value))
    }
    /// Test signal guard symbol.
    public static func cv2xGuard(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCv2xGuard(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_guard", value))
    }
    /// Test signal resource blocks.
    public static func cv2xResourceBlocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_resource_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCv2xResourceBlocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_resource_blocks", value))
    }
}

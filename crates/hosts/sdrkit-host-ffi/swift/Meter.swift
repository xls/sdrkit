// sdrkit: the typed interface of the stack family "meter", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum MeterInterface {
    public static let major = 1
    public static let minor = 1
}

/// The result fields of `fm_monitor`, measured once a second from the FM-demodulated multiplex (MPX).
public struct FmMonitorFields {
    public enum Pilot: Int32 {
        case absent = 0
        case inRange = 1
        case outside = 2
    }
    /// The headline: stereo or mono, the peak, the pilot and RDS levels.
    public var text: String
    /// The peak deviation of the last second: the largest third-largest excursion of its 5 ms blocks.
    public var peakDeviationKhz: Double?
    /// The RMS deviation.
    public var rmsDeviationKhz: Double?
    /// The MPX power of the last second (0 dBr is a sine of 19 kHz deviation).
    public var mpxPowerDbr: Double?
    /// The MPX power over the last 60 s in steps of 1 s, as ITU-R BS.412 integrates it; no value before 60 s.
    public var mpxPower60sDbr: Double?
    /// The carrier offset.
    public var carrierOffsetKhz: Double?
    /// The audio (L+R), peak equivalent.
    public var audioKhz: Double?
    /// The 19 kHz pilot, peak equivalent.
    public var pilotKhz: Double?
    /// The pilot as a share of the deviation limit.
    public var pilotPct: Double?
    /// The pilot against its usual range.
    public var pilot: Pilot?
    /// The stereo (L-R) band, peak equivalent.
    public var stereoBandKhz: Double?
    /// Stereo: a pilot and an L-R band over 0.5 kHz.
    public var stereo: Bool
    /// The RDS injection, peak equivalent.
    public var rdsKhz: Double?
    /// RDS is present.
    public var rds: Bool
    /// The 67 kHz subcarrier over the floor; no value when absent.
    public var sca67Db: Double?
    /// The 92 kHz subcarrier over the floor; no value when absent or below a working rate of 250 ksps.
    public var sca92Db: Double?
    /// The seconds whose peak was over the threshold.
    public var overDeviationS: Int64
    /// The input level.
    public var levelDb: Double?
    /// What the monitor measures.
    public var note: String

    init(_ c: sdrkit_fm_monitor_fields) {
        text = sdrkitText(c.text)
        peakDeviationKhz = c.has_peak_deviation_khz ? c.peak_deviation_khz : nil
        rmsDeviationKhz = c.has_rms_deviation_khz ? c.rms_deviation_khz : nil
        mpxPowerDbr = c.has_mpx_power_dbr ? c.mpx_power_dbr : nil
        mpxPower60sDbr = c.has_mpx_power_60s_dbr ? c.mpx_power_60s_dbr : nil
        carrierOffsetKhz = c.has_carrier_offset_khz ? c.carrier_offset_khz : nil
        audioKhz = c.has_audio_khz ? c.audio_khz : nil
        pilotKhz = c.has_pilot_khz ? c.pilot_khz : nil
        pilotPct = c.has_pilot_pct ? c.pilot_pct : nil
        pilot = Pilot(rawValue: c.pilot)
        stereoBandKhz = c.has_stereo_band_khz ? c.stereo_band_khz : nil
        stereo = c.stereo
        rdsKhz = c.has_rds_khz ? c.rds_khz : nil
        rds = c.rds
        sca67Db = c.has_sca67_db ? c.sca67_db : nil
        sca92Db = c.has_sca92_db ? c.sca92_db : nil
        overDeviationS = c.over_deviation_s
        levelDb = c.has_level_db ? c.level_db : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FmMonitorFields, SDRKitCode> {
        var c = sdrkit_fm_monitor_fields()
        let code = sdrkit_fm_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FmMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct MeterDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_meter_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeterDecoderFields, SDRKitCode> {
        var c = sdrkit_meter_decoder_fields()
        let code = sdrkit_meter_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeterDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the log of `fm_monitor`. The row key is the sequence number of the row.
public struct FmMonitorLogRow {
    /// The row key.
    public var key: String
    /// The signal time of the row.
    public var signalTimeS: Double
    /// The kind: `Pilot`, `RDS`, `SCA` or `Over-deviation`.
    public var kind: String
    /// What changed, or the peak of an over-deviation.
    public var identity: String
    /// The text of the row.
    public var text: String

    init(_ c: sdrkit_fm_monitor_log_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        kind = sdrkitText(c.kind)
        identity = sdrkitText(c.identity)
        text = sdrkitText(c.text)
    }
}

/// The result fields of `am_monitor`, measured once a second. The measurements have no value while the carrier test fails.
public struct AmMonitorFields {
    /// The headline: the offset, the modulation, the peaks and the audio bandwidth, or "no carrier".
    public var text: String
    /// The carrier offset: a tuning or station error.
    public var carrierOffsetHz: Double?
    /// The carrier level, relative to full scale.
    public var carrierDb: Double?
    /// The modulation depth: the depth of a sine of the same RMS.
    public var depthPct: Double?
    /// The positive peak against the mean carrier.
    public var peakPosPct: Double?
    /// The negative peak against the mean carrier (at most 100 %).
    public var peakNegPct: Double?
    /// The asymmetry: (positive - negative) / (positive + negative).
    public var asymmetryPct: Double?
    /// The mains hum below the carrier; no value when no hum line is seen.
    public var humDb: Double?
    /// The frequency of the mains hum line.
    public var humHz: Double?
    /// The audio bandwidth: 99 % of the audio power lies below it.
    public var bandwidthKhz: Double?
    /// The share of the last second at zero carrier.
    public var overModPct: Double?
    /// The seconds that were over-modulated.
    public var overModS: Int64
    /// The carrier is present (the carrier test and the level rule).
    public var carrier: Bool
    /// What the monitor measures.
    public var note: String

    init(_ c: sdrkit_am_monitor_fields) {
        text = sdrkitText(c.text)
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
        carrierDb = c.has_carrier_db ? c.carrier_db : nil
        depthPct = c.has_depth_pct ? c.depth_pct : nil
        peakPosPct = c.has_peak_pos_pct ? c.peak_pos_pct : nil
        peakNegPct = c.has_peak_neg_pct ? c.peak_neg_pct : nil
        asymmetryPct = c.has_asymmetry_pct ? c.asymmetry_pct : nil
        humDb = c.has_hum_db ? c.hum_db : nil
        humHz = c.has_hum_hz ? c.hum_hz : nil
        bandwidthKhz = c.has_bandwidth_khz ? c.bandwidth_khz : nil
        overModPct = c.has_over_mod_pct ? c.over_mod_pct : nil
        overModS = c.over_mod_s
        carrier = c.carrier
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AmMonitorFields, SDRKitCode> {
        var c = sdrkit_am_monitor_fields()
        let code = sdrkit_am_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AmMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the log of `am_monitor`. The row key is the sequence number of the row.
public struct AmMonitorLogRow {
    /// The row key.
    public var key: String
    /// The signal time of the row.
    public var signalTimeS: Double
    /// The kind: `Carrier` or `Over-modulation`.
    public var kind: String
    /// What changed: `present`, `lost`, `returned`, or the share of the second at zero carrier.
    public var identity: String
    /// The text of the row.
    public var text: String

    init(_ c: sdrkit_am_monitor_log_row) {
        key = sdrkitText(c.key)
        signalTimeS = c.signal_time_s
        kind = sdrkitText(c.kind)
        identity = sdrkitText(c.identity)
        text = sdrkitText(c.text)
    }
}

/// The status of a spectrum watcher: the headline, the state, the counts of readings and the input level.
public struct MeterWatchFields {
    public enum State: Int32 {
        case nothing = 0
        case energy = 1
        case measuring = 2
    }
    /// The headline.
    public var text: String
    /// The state.
    public var state: State?
    /// The good readings: what the stack counts as a measurement.
    public var good: Int64
    /// The bad readings: what the stack counts as a fault of the watched band.
    public var bad: Int64
    /// The input level of the last report, relative to full scale.
    public var levelDbfs: Double?
    /// What the stack measures.
    public var note: String

    init(_ c: sdrkit_meter_watch_fields) {
        text = sdrkitText(c.text)
        state = State(rawValue: c.state)
        good = c.good
        bad = c.bad
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeterWatchFields, SDRKitCode> {
        var c = sdrkit_meter_watch_fields()
        let code = sdrkit_meter_watch_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeterWatchFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `occupancy`.
public struct OccupancyFields {
    /// The channels of the grid.
    public var channels: Int64
    /// The width of a channel: whole bins.
    public var channelKhz: Double
    /// The span that the channels cover.
    public var spanMhz: Double
    /// The noise floor of a channel, relative, per bin; no value before the first decision.
    public var floorDb: Double?
    /// The channels busy now.
    public var busyNow: Int64
    /// The busy share of the channel-time since the end of the warm-up; no value before it.
    public var bandOccupancyPct: Double?
    /// The time measured after the warm-up.
    public var elapsedS: Double

    init(_ c: sdrkit_occupancy_fields) {
        channels = c.channels
        channelKhz = c.channel_khz
        spanMhz = c.span_mhz
        floorDb = c.has_floor_db ? c.floor_db : nil
        busyNow = c.busy_now
        bandOccupancyPct = c.has_band_occupancy_pct ? c.band_occupancy_pct : nil
        elapsedS = c.elapsed_s
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<OccupancyFields, SDRKitCode> {
        var c = sdrkit_occupancy_fields()
        let code = sdrkit_occupancy_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(OccupancyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the list of busy channels: a channel that has been busy, the busiest first.
public struct OccupancyChannelRow {
    /// The row key.
    public var key: String
    /// The centre of the channel (an offset from the centre when the tuned frequency is not known).
    public var frequency: Double
    /// The busy time over the time measured.
    public var dutyPct: Double
    /// The transmissions that ended.
    public var transmissions: Int64
    /// The longest transmission.
    public var longestS: Double
    /// The channel is busy now.
    public var busy: Bool

    init(_ c: sdrkit_occupancy_channel_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        dutyPct = c.duty_pct
        transmissions = c.transmissions
        longestS = c.longest_s
        busy = c.busy
    }
}

/// A row of the activity log: one transmission of at least the shortest listed length.
public struct OccupancyActivityRow {
    /// The row key.
    public var key: String
    /// The end of the transmission, signal time.
    public var timeS: Double
    /// The centre of the channel.
    public var frequency: Double
    /// The length of the transmission.
    public var lengthS: Double
    /// The highest channel power over the floor.
    public var peakOverFloorDb: Double
    /// The highest channel power, relative, per bin.
    public var levelDb: Double

    init(_ c: sdrkit_occupancy_activity_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        frequency = c.frequency
        lengthS = c.length_s
        peakOverFloorDb = c.peak_over_floor_db
        levelDb = c.level_db
    }
}

/// The result fields of `noise_floor`.
public struct NoiseFloorFields {
    /// The floor of the band: the upper median of the sub-bands now, dB re full scale per bin.
    public var floorDb: Double?
    /// The floor now against the oldest kept value (the baseline window ago, or the start).
    public var floorChangeDb: Double?
    /// The sub-bands raised now.
    public var raised: Int64
    /// The seconds measured.
    public var elapsedS: Double

    init(_ c: sdrkit_noise_floor_fields) {
        floorDb = c.has_floor_db ? c.floor_db : nil
        floorChangeDb = c.has_floor_change_db ? c.floor_change_db : nil
        raised = c.raised
        elapsedS = c.elapsed_s
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NoiseFloorFields, SDRKitCode> {
        var c = sdrkit_noise_floor_fields()
        let code = sdrkit_noise_floor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NoiseFloorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the list of sub-bands.
public struct NoiseFloorSubBandRow {
    /// The row key.
    public var key: String
    /// The centre of the sub-band.
    public var frequency: Double
    /// The floor of the last second, dB re full scale per bin.
    public var nowDb: Double?
    /// The baseline: the 20th percentile of the baseline window.
    public var baselineDb: Double?
    /// The 10th percentile of the kept history.
    public var p10Db: Double?
    /// The median of the kept history.
    public var p50Db: Double?
    /// The 90th percentile of the kept history.
    public var p90Db: Double?
    /// The minutes of history kept.
    public var historyMin: Double
    /// The sub-band is raised.
    public var raised: Bool

    init(_ c: sdrkit_noise_floor_sub_band_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        nowDb = c.has_now_db ? c.now_db : nil
        baselineDb = c.has_baseline_db ? c.baseline_db : nil
        p10Db = c.has_p10_db ? c.p10_db : nil
        p50Db = c.has_p50_db ? c.p50_db : nil
        p90Db = c.has_p90_db ? c.p90_db : nil
        historyMin = c.history_min
        raised = c.raised
    }
}

/// A row of the noise floor log.
public struct NoiseFloorEventRow {
    public enum Kind: Int32 {
        case rise = 0
        case back = 1
        case wholeBand = 2
    }
    /// The row key.
    public var key: String
    /// The signal time of the row.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The centre of the sub-band; no value for the whole band.
    public var frequency: Double?
    /// The sub-band, from 1; no value for the whole band.
    public var subBand: Int64?
    /// The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band.
    public var riseDb: Double
    /// The baseline of the sub-band.
    public var baselineDb: Double?
    /// How long the rise lasted (rows of the fall back).
    public var durationS: Double?
    /// The floor of the sub-band, or of the band, that second.
    public var levelDb: Double?

    init(_ c: sdrkit_noise_floor_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        frequency = c.has_frequency ? c.frequency : nil
        subBand = c.has_sub_band ? c.sub_band : nil
        riseDb = c.rise_db
        baselineDb = c.has_baseline_db ? c.baseline_db : nil
        durationS = c.has_duration_s ? c.duration_s : nil
        levelDb = c.has_level_db ? c.level_db : nil
    }
}

/// The result fields of `burst_finder`.
public struct BurstFinderFields {
    /// The bursts found.
    public var bursts: Int64
    /// The time listened.
    public var elapsedS: Double
    /// Bursts per second over the time listened.
    public var burstsPerS: Double
    /// The sources found repeating now.
    public var repeating: Int64
    /// The width of a bin: the frequency resolution.
    public var binKhz: Double
    /// The frame step: the time resolution.
    public var frameUs: Double

    init(_ c: sdrkit_burst_finder_fields) {
        bursts = c.bursts
        elapsedS = c.elapsed_s
        burstsPerS = c.bursts_per_s
        repeating = c.repeating
        binKhz = c.bin_khz
        frameUs = c.frame_us
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BurstFinderFields, SDRKitCode> {
        var c = sdrkit_burst_finder_fields()
        let code = sdrkit_burst_finder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BurstFinderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the burst log.
public struct BurstFinderBurstRow {
    public enum Class: Int32 {
        case narrow = 0
        case wide = 1
    }
    /// The row key.
    public var key: String
    /// The start of the burst, signal time.
    public var timeS: Double
    /// The median centre of the burst.
    public var frequency: Double
    /// The length of the burst.
    public var lengthMs: Double
    /// The bandwidth class.
    public var `class`: Class?
    /// The median width, at least one bin.
    public var widthKhz: Double
    /// The mean peak over the median of the frames.
    public var levelDb: Double

    init(_ c: sdrkit_burst_finder_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        frequency = c.frequency
        lengthMs = c.length_ms
        `class` = Class(rawValue: c.class_)
        widthKhz = c.width_khz
        levelDb = c.level_db
    }
}

/// A row of the list of repeating sources.
public struct BurstFinderRepeatingRow {
    /// The row key.
    public var key: String
    /// The frequency of the source: its first burst.
    public var frequency: Double
    /// The median interval between its bursts.
    public var periodS: Double
    /// Its bursts in the history.
    public var bursts: Int64
    /// The RMS deviation of the agreeing intervals from the period, over the period.
    public var spreadPct: Double

    init(_ c: sdrkit_burst_finder_repeating_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        periodS = c.period_s
        bursts = c.bursts
        spreadPct = c.spread_pct
    }
}

/// The result fields of `hopper`. The readings of the hopper have no value until an evaluation finds one.
public struct HopperFields {
    /// The hop rate: one over the median start-to-start time.
    public var hopRate: Double?
    /// The median start-to-start time.
    public var hopPeriodMs: Double?
    /// The median dwell.
    public var dwellMs: Double?
    /// The median silence between dwells.
    public var guardMs: Double?
    /// The channels found.
    public var channels: Int64?
    /// The median distance of neighbouring channels.
    public var spacingKhz: Double?
    /// At least 90 % of the channels lie on a grid of the spacing.
    public var grid: Bool?
    /// The highest minus the lowest channel plus one spacing.
    public var spanKhz: Double?
    /// The lowest channel.
    public var lowest: Double?
    /// The highest channel.
    public var highest: Double?
    /// The mean dwell level over the frame median.
    public var levelDb: Double?
    /// The dwells since the start.
    public var dwells: Int64
    /// The dwells kept for the evaluation.
    public var dwellsWindow: Int64
    /// The width of a bin.
    public var binKhz: Double
    /// The frame step.
    public var frameUs: Double

    init(_ c: sdrkit_hopper_fields) {
        hopRate = c.has_hop_rate ? c.hop_rate : nil
        hopPeriodMs = c.has_hop_period_ms ? c.hop_period_ms : nil
        dwellMs = c.has_dwell_ms ? c.dwell_ms : nil
        guardMs = c.has_guard_ms ? c.guard_ms : nil
        channels = c.has_channels ? c.channels : nil
        spacingKhz = c.has_spacing_khz ? c.spacing_khz : nil
        grid = c.has_grid ? c.grid : nil
        spanKhz = c.has_span_khz ? c.span_khz : nil
        lowest = c.has_lowest ? c.lowest : nil
        highest = c.has_highest ? c.highest : nil
        levelDb = c.has_level_db ? c.level_db : nil
        dwells = c.dwells
        dwellsWindow = c.dwells_window
        binKhz = c.bin_khz
        frameUs = c.frame_us
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HopperFields, SDRKitCode> {
        var c = sdrkit_hopper_fields()
        let code = sdrkit_hopper_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HopperFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the hopper log: an evaluation that found a hopper or a change of it.
public struct HopperRow {
    /// The row key.
    public var key: String
    /// The signal time of the evaluation.
    public var timeS: Double
    /// The hop rate.
    public var hopRate: Double
    /// The channels.
    public var channels: Int64
    /// The channel spacing.
    public var spacingKhz: Double
    /// The span.
    public var spanKhz: Double
    /// The lowest channel.
    public var lowest: Double
    /// The highest channel.
    public var highest: Double
    /// The median dwell.
    public var dwellMs: Double
    /// The median guard.
    public var guardMs: Double
    /// The mean dwell level over the frame median.
    public var levelDb: Double

    init(_ c: sdrkit_hopper_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        hopRate = c.hop_rate
        channels = c.channels
        spacingKhz = c.spacing_khz
        spanKhz = c.span_khz
        lowest = c.lowest
        highest = c.highest
        dwellMs = c.dwell_ms
        guardMs = c.guard_ms
        levelDb = c.level_db
    }
}

/// The result fields of `birdie`.
public struct BirdieFields {
    /// The persistent carriers listed now.
    public var carriers: Int64
    /// The snapshots taken.
    public var snapshots: Int64
    /// The time of a snapshot.
    public var snapshotS: Double
    /// The width of a bin.
    public var binHz: Double

    init(_ c: sdrkit_birdie_fields) {
        carriers = c.carriers
        snapshots = c.snapshots
        snapshotS = c.snapshot_s
        binHz = c.bin_hz
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BirdieFields, SDRKitCode> {
        var c = sdrkit_birdie_fields()
        let code = sdrkit_birdie_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BirdieFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the list of persistent carriers, the strongest first.
public struct BirdieCarrierRow {
    public enum Class: Int32 {
        case bare = 0
        case modulated = 1
    }
    /// The row key.
    public var key: String
    /// The smoothed frequency.
    public var frequency: Double
    /// The mean level over the noise per bin.
    public var levelDb: Double
    /// The standard deviation of the level.
    public var levelSdDb: Double
    /// The class.
    public var `class`: Class?
    /// The least-squares drift of the frequency.
    public var driftHzMin: Double
    /// The RMS of the frequency about the drift line.
    public var scatterHz: Double
    /// The snapshots of the presence window in which it was seen.
    public var present: Int64
    /// The snapshots of the presence window.
    public var of: Int64

    init(_ c: sdrkit_birdie_carrier_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        levelDb = c.level_db
        levelSdDb = c.level_sd_db
        `class` = Class(rawValue: c.class_)
        driftHzMin = c.drift_hz_min
        scatterHz = c.scatter_hz
        present = c.present
        of = c.of
    }
}

/// A row of the carrier log.
public struct BirdieEventRow {
    public enum Kind: Int32 {
        case new = 0
        case returned = 1
        case gone = 2
    }
    public enum Class: Int32 {
        case bare = 0
        case modulated = 1
    }
    /// The row key.
    public var key: String
    /// The signal time of the row.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The frequency.
    public var frequency: Double
    /// The mean level over the noise.
    public var levelDb: Double
    /// The class.
    public var `class`: Class?
    /// The drift.
    public var driftHzMin: Double
    /// The scatter.
    public var scatterHz: Double

    init(_ c: sdrkit_birdie_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        frequency = c.frequency
        levelDb = c.level_db
        `class` = Class(rawValue: c.class_)
        driftHzMin = c.drift_hz_min
        scatterHz = c.scatter_hz
    }
}

/// The result fields of `obw`. The readings have no value while no signal is found.
public struct ObwFields {
    /// The band that holds the power share, half the rest below and half above it.
    public var obwKhz: Double?
    /// The lower edge of the occupied band.
    public var obwLow: Double?
    /// The upper edge of the occupied band.
    public var obwHigh: Double?
    /// The width at 3 dB under the peak of the smoothed spectrum.
    public var bw3Khz: Double?
    /// The width at 6 dB under the peak.
    public var bw6Khz: Double?
    /// The width at 20 dB under the peak.
    public var bw20Khz: Double?
    /// The width at 26 dB under the peak.
    public var bw26Khz: Double?
    /// The middle of the occupied band from the tuned frequency.
    public var centreOffsetHz: Double?
    /// The power between the edges over the floor, dB re full scale.
    public var powerDb: Double?
    /// The signal power over the floor in the same bins.
    public var snrDb: Double?
    /// The power of the band one adjacent offset below, under the signal; no value outside the spectrum.
    public var adjacentLowDb: Double?
    /// The same above.
    public var adjacentHighDb: Double?
    /// The power of the band two adjacent offsets below, under the signal.
    public var alternateLowDb: Double?
    /// The same above.
    public var alternateHighDb: Double?
    /// A signal was found in the last evaluation.
    public var signal: Bool

    init(_ c: sdrkit_obw_fields) {
        obwKhz = c.has_obw_khz ? c.obw_khz : nil
        obwLow = c.has_obw_low ? c.obw_low : nil
        obwHigh = c.has_obw_high ? c.obw_high : nil
        bw3Khz = c.has_bw3_khz ? c.bw3_khz : nil
        bw6Khz = c.has_bw6_khz ? c.bw6_khz : nil
        bw20Khz = c.has_bw20_khz ? c.bw20_khz : nil
        bw26Khz = c.has_bw26_khz ? c.bw26_khz : nil
        centreOffsetHz = c.has_centre_offset_hz ? c.centre_offset_hz : nil
        powerDb = c.has_power_db ? c.power_db : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        adjacentLowDb = c.has_adjacent_low_db ? c.adjacent_low_db : nil
        adjacentHighDb = c.has_adjacent_high_db ? c.adjacent_high_db : nil
        alternateLowDb = c.has_alternate_low_db ? c.alternate_low_db : nil
        alternateHighDb = c.has_alternate_high_db ? c.alternate_high_db : nil
        signal = c.signal
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ObwFields, SDRKitCode> {
        var c = sdrkit_obw_fields()
        let code = sdrkit_obw_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ObwFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the bandwidth log.
public struct ObwBandwidthRow {
    /// The row key.
    public var key: String
    /// The signal time of the evaluation.
    public var timeS: Double
    /// The band that holds the power share, half the rest below and half above it.
    public var obwKhz: Double
    /// The lower edge of the occupied band.
    public var obwLow: Double
    /// The upper edge of the occupied band.
    public var obwHigh: Double
    /// The width at 3 dB under the peak of the smoothed spectrum.
    public var bw3Khz: Double
    /// The width at 6 dB under the peak.
    public var bw6Khz: Double
    /// The width at 20 dB under the peak.
    public var bw20Khz: Double
    /// The width at 26 dB under the peak.
    public var bw26Khz: Double
    /// The middle of the occupied band from the tuned frequency.
    public var centreOffsetHz: Double
    /// The power between the edges over the floor, dB re full scale.
    public var powerDb: Double
    /// The signal power over the floor in the same bins.
    public var snrDb: Double
    /// The power of the band one adjacent offset below, under the signal; no value outside the spectrum.
    public var adjacentLowDb: Double?
    /// The same above.
    public var adjacentHighDb: Double?
    /// The power of the band two adjacent offsets below, under the signal.
    public var alternateLowDb: Double?
    /// The same above.
    public var alternateHighDb: Double?

    init(_ c: sdrkit_obw_bandwidth_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        obwKhz = c.obw_khz
        obwLow = c.obw_low
        obwHigh = c.obw_high
        bw3Khz = c.bw3_khz
        bw6Khz = c.bw6_khz
        bw20Khz = c.bw20_khz
        bw26Khz = c.bw26_khz
        centreOffsetHz = c.centre_offset_hz
        powerDb = c.power_db
        snrDb = c.snr_db
        adjacentLowDb = c.has_adjacent_low_db ? c.adjacent_low_db : nil
        adjacentHighDb = c.has_adjacent_high_db ? c.adjacent_high_db : nil
        alternateLowDb = c.has_alternate_low_db ? c.alternate_low_db : nil
        alternateHighDb = c.has_alternate_high_db ? c.alternate_high_db : nil
    }
}

/// The result fields of `ptt_log`.
public struct PttLogFields {
    /// The channel is keyed now.
    public var keyed: Bool
    /// The length of the present transmission; no value while idle.
    public var keyedS: Double?
    /// The floor of the channel, relative; no value before the first reading.
    public var floorDb: Double?
    /// The transmissions that ended.
    public var transmissions: Int64
    /// Those shorter than the kerchunk length.
    public var kerchunks: Int64
    /// The keyed share of the time since the end of the warm-up, the present transmission included.
    public var dutyPct: Double?
    /// The keyed share of the last 10 minutes (or of the time measured, when shorter).
    public var duty10minPct: Double?
    /// The mean length of the transmissions that ended.
    public var meanS: Double?
    /// The median length.
    public var medianS: Double?
    /// The longest.
    public var longestS: Double?
    /// Transmissions per hour of the time measured.
    public var perHour: Double?
    /// The time measured since the end of the warm-up.
    public var elapsedS: Double

    init(_ c: sdrkit_ptt_log_fields) {
        keyed = c.keyed
        keyedS = c.has_keyed_s ? c.keyed_s : nil
        floorDb = c.has_floor_db ? c.floor_db : nil
        transmissions = c.transmissions
        kerchunks = c.kerchunks
        dutyPct = c.has_duty_pct ? c.duty_pct : nil
        duty10minPct = c.has_duty_10min_pct ? c.duty_10min_pct : nil
        meanS = c.has_mean_s ? c.mean_s : nil
        medianS = c.has_median_s ? c.median_s : nil
        longestS = c.has_longest_s ? c.longest_s : nil
        perHour = c.has_per_hour ? c.per_hour : nil
        elapsedS = c.elapsed_s
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PttLogFields, SDRKitCode> {
        var c = sdrkit_ptt_log_fields()
        let code = sdrkit_ptt_log_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PttLogFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the transmission log.
public struct PttLogTransmissionRow {
    public enum Kind: Int32 {
        case transmission = 0
        case kerchunk = 1
        case stuckKey = 2
    }
    /// The row key.
    public var key: String
    /// The number of the transmission, from 1.
    public var number: Int64
    /// The kind.
    public var kind: Kind?
    /// The start, signal time.
    public var startS: Double
    /// The length (so far, for a stuck key).
    public var lengthS: Double
    /// The mean level over the floor of its keyed blocks.
    public var overFloorDb: Double
    /// The time from the end of the transmission before; no value for the first.
    public var gapS: Double?
    /// The mean level of its keyed blocks, relative to full scale.
    public var levelDb: Double

    init(_ c: sdrkit_ptt_log_transmission_row) {
        key = sdrkitText(c.key)
        number = c.number
        kind = Kind(rawValue: c.kind)
        startS = c.start_s
        lengthS = c.length_s
        overFloorDb = c.over_floor_db
        gapS = c.has_gap_s ? c.gap_s : nil
        levelDb = c.level_db
    }
}

/// The result fields of `noise_locator`.
public struct NoiseLocatorFields {
    public enum Class: Int32 {
        case twiceMains = 0
        case mains = 1
        case steady = 2
        case none = 3
    }
    /// The impulses per second of the impulse window.
    public var impulseRate: Double?
    /// The median impulse level over the noise of the analysis band.
    public var impulseLevelDb: Double?
    /// The 75th minus the 25th percentile of the impulse levels.
    public var impulseSpreadDb: Double?
    /// The repetition rate of the present pattern; no value without one.
    public var repetitionHz: Double?
    /// The class of the present pattern.
    public var `class`: Class?
    /// The mains frequency of the class, when it names one.
    public var mainsFoundHz: Double?
    /// The share of the impulses on the rate.
    public var strengthPct: Double?
    /// The impulses since the start.
    public var impulses: Int64

    init(_ c: sdrkit_noise_locator_fields) {
        impulseRate = c.has_impulse_rate ? c.impulse_rate : nil
        impulseLevelDb = c.has_impulse_level_db ? c.impulse_level_db : nil
        impulseSpreadDb = c.has_impulse_spread_db ? c.impulse_spread_db : nil
        repetitionHz = c.has_repetition_hz ? c.repetition_hz : nil
        `class` = Class(rawValue: c.class_)
        mainsFoundHz = c.has_mains_found_hz ? c.mains_found_hz : nil
        strengthPct = c.has_strength_pct ? c.strength_pct : nil
        impulses = c.impulses
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NoiseLocatorFields, SDRKitCode> {
        var c = sdrkit_noise_locator_fields()
        let code = sdrkit_noise_locator_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NoiseLocatorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the pattern log.
public struct NoiseLocatorPatternRow {
    public enum Kind: Int32 {
        case found = 0
        case gone = 1
    }
    public enum Class: Int32 {
        case twiceMains = 0
        case mains = 1
        case steady = 2
        case none = 3
    }
    /// The row key.
    public var key: String
    /// The signal time of the report.
    public var timeS: Double
    /// Found or gone.
    public var kind: Kind?
    /// The repetition rate.
    public var repetitionHz: Double
    /// The class of the rate.
    public var `class`: Class?
    /// The impulses per second of the window.
    public var impulseRate: Double
    /// The median impulse level over the noise of the analysis band.
    public var levelDb: Double
    /// The share of the impulses on the rate.
    public var strengthPct: Double

    init(_ c: sdrkit_noise_locator_pattern_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        repetitionHz = c.repetition_hz
        `class` = Class(rawValue: c.class_)
        impulseRate = c.impulse_rate
        levelDb = c.level_db
        strengthPct = c.strength_pct
    }
}

/// The result fields of `freq_accuracy`. The readings of the carrier have no value while the carrier is absent.
public struct FreqAccuracyFields {
    /// The mean offset of the last 10 readings from the tuned frequency.
    public var offsetHz: Double?
    /// The sample standard deviation of the last 10 readings.
    public var scatterHz: Double?
    /// The offset over the tuned frequency; no value without the tuned frequency.
    public var errorPpm: Double?
    /// The carrier to noise density of the last window.
    public var cn0Dbhz: Double?
    /// The least-squares slope of the last 120 readings; from 8 readings.
    public var driftHzS: Double?
    /// The drift over the tuned frequency.
    public var driftPpmH: Double?
    /// The readings kept.
    public var readings: Int64
    /// The length of one reading window.
    public var windowS: Double?
    /// The carrier was present in the last window.
    public var carrier: Bool?

    init(_ c: sdrkit_freq_accuracy_fields) {
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        scatterHz = c.has_scatter_hz ? c.scatter_hz : nil
        errorPpm = c.has_error_ppm ? c.error_ppm : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        driftPpmH = c.has_drift_ppm_h ? c.drift_ppm_h : nil
        readings = c.readings
        windowS = c.has_window_s ? c.window_s : nil
        carrier = c.has_carrier ? c.carrier : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FreqAccuracyFields, SDRKitCode> {
        var c = sdrkit_freq_accuracy_fields()
        let code = sdrkit_freq_accuracy_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FreqAccuracyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the Allan deviation table: one averaging time.
public struct FreqAccuracyAllanRow {
    /// The row key.
    public var key: String
    /// The averaging time.
    public var tauS: Double
    /// The fractional Allan deviation; no value without the tuned frequency.
    public var adev: Double?
    /// The Allan deviation of the offset.
    public var adevHz: Double
    /// The groups it is from.
    public var groups: Int64

    init(_ c: sdrkit_freq_accuracy_allan_row) {
        key = sdrkitText(c.key)
        tauS = c.tau_s
        adev = c.has_adev ? c.adev : nil
        adevHz = c.adev_hz
        groups = c.groups
    }
}

/// A row of the `freq_accuracy` log.
public struct FreqAccuracyEventRow {
    public enum Kind: Int32 {
        case frequency = 0
        case step = 1
        case lost = 2
        case returned = 3
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The offset of the reading (for a loss, the last reading).
    public var offsetHz: Double
    /// The size of a step.
    public var stepHz: Double?
    /// The carrier to noise density of the reading.
    public var cn0Dbhz: Double?

    init(_ c: sdrkit_freq_accuracy_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        offsetHz = c.offset_hz
        stepHz = c.has_step_hz ? c.step_hz : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
    }
}

/// The result fields of `sinad`, from the last window. The readings have no value while no test tone is found.
public struct SinadFields {
    /// SINAD in the audio band, flat weighting.
    public var sinadDb: Double?
    /// The SINAD is under the sensitivity mark (12 dB by default).
    public var below12db: Bool?
    /// The test tone.
    public var toneHz: Double?
    /// The peak deviation of the tone.
    public var deviationKhz: Double?
    /// THD of harmonics 2 to 5.
    public var thdPct: Double?
    /// THD and noise.
    public var thdnPct: Double?
    /// The carrier offset: the mean of the discriminator audio.
    public var carrierOffsetHz: Double?
    /// The windows measured.
    public var windows: Int64

    init(_ c: sdrkit_sinad_fields) {
        sinadDb = c.has_sinad_db ? c.sinad_db : nil
        below12db = c.has_below_12db ? c.below_12db : nil
        toneHz = c.has_tone_hz ? c.tone_hz : nil
        deviationKhz = c.has_deviation_khz ? c.deviation_khz : nil
        thdPct = c.has_thd_pct ? c.thd_pct : nil
        thdnPct = c.has_thdn_pct ? c.thdn_pct : nil
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
        windows = c.windows
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SinadFields, SDRKitCode> {
        var c = sdrkit_sinad_fields()
        let code = sdrkit_sinad_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SinadFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `sinad` log.
public struct SinadEventRow {
    public enum Crossing: Int32 {
        case none = 0
        case fell = 1
        case rose = 2
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// SINAD.
    public var sinadDb: Double
    /// The change against the mark.
    public var crossing: Crossing?
    /// The test tone.
    public var toneHz: Double
    /// The peak deviation.
    public var deviationKhz: Double
    /// THD.
    public var thdPct: Double

    init(_ c: sdrkit_sinad_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        sinadDb = c.sinad_db
        crossing = Crossing(rawValue: c.crossing)
        toneHz = c.tone_hz
        deviationKhz = c.deviation_khz
        thdPct = c.thd_pct
    }
}

/// The result fields of `nfm_meter`, from the last window. The voice readings have no value without a carrier, the CTCSS readings none without a tone.
public struct NfmMeterFields {
    /// The 99.9th percentile of the voice band deviation of the window.
    public var voicePeakKhz: Double?
    /// The largest voice peak so far.
    public var voicePeakMaxKhz: Double?
    /// Windows over the narrow channel limit.
    public var overNarrow: Int64
    /// Windows over the wide channel limit.
    public var overWide: Int64
    /// Windows measured.
    public var windows: Int64
    /// The CTCSS tone measured.
    public var ctcssHz: Double?
    /// The nearest standard tone.
    public var ctcssStandardHz: Double?
    /// Measured minus standard.
    public var ctcssErrorHz: Double?
    /// The peak deviation of the CTCSS tone.
    public var ctcssDeviationHz: Double?
    /// The CTCSS deviation as a share of the narrow limit.
    public var ctcssShareNarrowPct: Double?
    /// The CTCSS deviation as a share of the wide limit.
    public var ctcssShareWidePct: Double?
    /// The carrier offset: the mean of the discriminator audio.
    public var carrierOffsetHz: Double?

    init(_ c: sdrkit_nfm_meter_fields) {
        voicePeakKhz = c.has_voice_peak_khz ? c.voice_peak_khz : nil
        voicePeakMaxKhz = c.has_voice_peak_max_khz ? c.voice_peak_max_khz : nil
        overNarrow = c.over_narrow
        overWide = c.over_wide
        windows = c.windows
        ctcssHz = c.has_ctcss_hz ? c.ctcss_hz : nil
        ctcssStandardHz = c.has_ctcss_standard_hz ? c.ctcss_standard_hz : nil
        ctcssErrorHz = c.has_ctcss_error_hz ? c.ctcss_error_hz : nil
        ctcssDeviationHz = c.has_ctcss_deviation_hz ? c.ctcss_deviation_hz : nil
        ctcssShareNarrowPct = c.has_ctcss_share_narrow_pct ? c.ctcss_share_narrow_pct : nil
        ctcssShareWidePct = c.has_ctcss_share_wide_pct ? c.ctcss_share_wide_pct : nil
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NfmMeterFields, SDRKitCode> {
        var c = sdrkit_nfm_meter_fields()
        let code = sdrkit_nfm_meter_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NfmMeterFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `nfm_meter` log.
public struct NfmMeterEventRow {
    public enum Kind: Int32 {
        case tone = 0
        case toneGone = 1
        case overNarrow = 2
        case overWide = 3
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The nearest standard tone.
    public var ctcssStandardHz: Double?
    /// The CTCSS tone measured.
    public var ctcssHz: Double?
    /// The CTCSS deviation.
    public var ctcssDeviationHz: Double?
    /// The voice peak of the window.
    public var voicePeakKhz: Double?

    init(_ c: sdrkit_nfm_meter_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        ctcssStandardHz = c.has_ctcss_standard_hz ? c.ctcss_standard_hz : nil
        ctcssHz = c.has_ctcss_hz ? c.ctcss_hz : nil
        ctcssDeviationHz = c.has_ctcss_deviation_hz ? c.ctcss_deviation_hz : nil
        voicePeakKhz = c.has_voice_peak_khz ? c.voice_peak_khz : nil
    }
}

/// The result fields of `tx_stability`: the transmission now, and the last analysed transmission.
public struct TxStabilityFields {
    /// A transmission is on.
    public var transmitting: Bool
    /// The length of the transmission.
    public var lengthS: Double?
    /// The frequency readings in it.
    public var readings: Int64?
    /// The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency.
    public var startHz: Double?
    /// The mean frequency of the last 0.4 s.
    public var finalHz: Double?
    /// Start minus final.
    public var keyupStepHz: Double?
    /// The time of the last reading outside the settling band, plus 0.05 s.
    public var settleS: Double?
    /// The larger of 10 Hz and a tenth of the key-up step.
    public var settleBandHz: Double?
    /// The least-squares slope of the readings of the second half.
    public var driftHzS: Double?
    /// The mean of all readings.
    public var meanHz: Double?
    /// The mean over the tuned frequency; no value without it.
    public var meanPpm: Double?
    /// The RMS residual about the drift line.
    public var scatterHz: Double?
    /// The transmissions seen, the short ones included.
    public var transmissions: Int64

    init(_ c: sdrkit_tx_stability_fields) {
        transmitting = c.transmitting
        lengthS = c.has_length_s ? c.length_s : nil
        readings = c.has_readings ? c.readings : nil
        startHz = c.has_start_hz ? c.start_hz : nil
        finalHz = c.has_final_hz ? c.final_hz : nil
        keyupStepHz = c.has_keyup_step_hz ? c.keyup_step_hz : nil
        settleS = c.has_settle_s ? c.settle_s : nil
        settleBandHz = c.has_settle_band_hz ? c.settle_band_hz : nil
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        meanHz = c.has_mean_hz ? c.mean_hz : nil
        meanPpm = c.has_mean_ppm ? c.mean_ppm : nil
        scatterHz = c.has_scatter_hz ? c.scatter_hz : nil
        transmissions = c.transmissions
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TxStabilityFields, SDRKitCode> {
        var c = sdrkit_tx_stability_fields()
        let code = sdrkit_tx_stability_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TxStabilityFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the transmission table: one analysed transmission.
public struct TxStabilityTransmissionRow {
    /// The row key.
    public var key: String
    /// The key-up, signal time.
    public var timeS: Double
    /// The length of the transmission.
    public var lengthS: Double
    /// The frequency readings in it.
    public var readings: Int64
    /// The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency.
    public var startHz: Double
    /// The mean frequency of the last 0.4 s.
    public var finalHz: Double
    /// Start minus final.
    public var keyupStepHz: Double
    /// The time of the last reading outside the settling band, plus 0.05 s.
    public var settleS: Double
    /// The larger of 10 Hz and a tenth of the key-up step.
    public var settleBandHz: Double
    /// The least-squares slope of the readings of the second half.
    public var driftHzS: Double?
    /// The mean of all readings.
    public var meanHz: Double
    /// The mean over the tuned frequency; no value without it.
    public var meanPpm: Double?
    /// The RMS residual about the drift line.
    public var scatterHz: Double?

    init(_ c: sdrkit_tx_stability_transmission_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        lengthS = c.length_s
        readings = c.readings
        startHz = c.start_hz
        finalHz = c.final_hz
        keyupStepHz = c.keyup_step_hz
        settleS = c.settle_s
        settleBandHz = c.settle_band_hz
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        meanHz = c.mean_hz
        meanPpm = c.has_mean_ppm ? c.mean_ppm : nil
        scatterHz = c.has_scatter_hz ? c.scatter_hz : nil
    }
}

/// The result fields of `ssb_test`, from the last window. A level under the floor of the measurement is the floor with its flag set.
public struct SsbTestFields {
    public enum Kind: Int32 {
        case none = 0
        case oneTone = 1
        case twoTones = 2
    }
    public enum Sideband: Int32 {
        case upper = 0
        case lower = 1
    }
    /// The test seen.
    public var kind: Kind?
    /// The sideband of the tones.
    public var sideband: Sideband?
    /// The audio frequency of the (lower) tone.
    public var tone1Hz: Double?
    /// The audio frequency of the higher tone of two.
    public var tone2Hz: Double?
    /// The stronger tone over the weaker.
    public var toneLevelDiffDb: Double?
    /// The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search: the leak is at least this far under the tone.
    public var carrierDb: Double?
    /// No carrier is seen and the value is the floor of the carrier search, a bound.
    public var carrierUnder: Bool?
    /// A carrier was seen; else it is assumed at the tuned frequency for the tones and products.
    public var carrierFound: Bool?
    /// The opposite sideband of the (lower) tone.
    public var oppositeDb: Double?
    /// The opposite sideband is under the floor.
    public var oppositeUnder: Bool?
    /// Harmonic 2 of the audio tone (one tone).
    public var h2Db: Double?
    /// Harmonic 3 of the audio tone (one tone).
    public var h3Db: Double?
    /// Harmonic 4 of the audio tone (one tone).
    public var h4Db: Double?
    /// Harmonic 5 of the audio tone (one tone).
    public var h5Db: Double?
    /// Harmonic 2 is under the floor.
    public var h2Under: Bool?
    /// Harmonic 3 is under the floor.
    public var h3Under: Bool?
    /// Harmonic 4 is under the floor.
    public var h4Under: Bool?
    /// Harmonic 5 is under the floor.
    public var h5Under: Bool?
    /// The stronger product of order 3 under the weaker tone (two tones).
    public var imd3Db: Double?
    /// The stronger product of order 5 under the weaker tone (two tones).
    public var imd5Db: Double?
    /// The stronger product of order 7 under the weaker tone (two tones).
    public var imd7Db: Double?
    /// The products of order 3 are under the floor.
    public var imd3Under: Bool?
    /// The products of order 5 are under the floor.
    public var imd5Under: Bool?
    /// The products of order 7 are under the floor.
    public var imd7Under: Bool?
    /// The product of order 3 under the peak envelope power.
    public var imd3PepDb: Double?
    /// The product of order 5 under the peak envelope power.
    public var imd5PepDb: Double?
    /// The product of order 7 under the peak envelope power.
    public var imd7PepDb: Double?
    /// The floor of this measurement under the tone: the noise of 7 bins.
    public var floorDb: Double?

    init(_ c: sdrkit_ssb_test_fields) {
        kind = Kind(rawValue: c.kind)
        sideband = c.has_sideband ? Sideband(rawValue: c.sideband) : nil
        tone1Hz = c.has_tone1_hz ? c.tone1_hz : nil
        tone2Hz = c.has_tone2_hz ? c.tone2_hz : nil
        toneLevelDiffDb = c.has_tone_level_diff_db ? c.tone_level_diff_db : nil
        carrierDb = c.has_carrier_db ? c.carrier_db : nil
        carrierUnder = c.has_carrier_under ? c.carrier_under : nil
        carrierFound = c.has_carrier_found ? c.carrier_found : nil
        oppositeDb = c.has_opposite_db ? c.opposite_db : nil
        oppositeUnder = c.has_opposite_under ? c.opposite_under : nil
        h2Db = c.has_h2_db ? c.h2_db : nil
        h3Db = c.has_h3_db ? c.h3_db : nil
        h4Db = c.has_h4_db ? c.h4_db : nil
        h5Db = c.has_h5_db ? c.h5_db : nil
        h2Under = c.has_h2_under ? c.h2_under : nil
        h3Under = c.has_h3_under ? c.h3_under : nil
        h4Under = c.has_h4_under ? c.h4_under : nil
        h5Under = c.has_h5_under ? c.h5_under : nil
        imd3Db = c.has_imd3_db ? c.imd3_db : nil
        imd5Db = c.has_imd5_db ? c.imd5_db : nil
        imd7Db = c.has_imd7_db ? c.imd7_db : nil
        imd3Under = c.has_imd3_under ? c.imd3_under : nil
        imd5Under = c.has_imd5_under ? c.imd5_under : nil
        imd7Under = c.has_imd7_under ? c.imd7_under : nil
        imd3PepDb = c.has_imd3_pep_db ? c.imd3_pep_db : nil
        imd5PepDb = c.has_imd5_pep_db ? c.imd5_pep_db : nil
        imd7PepDb = c.has_imd7_pep_db ? c.imd7_pep_db : nil
        floorDb = c.has_floor_db ? c.floor_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SsbTestFields, SDRKitCode> {
        var c = sdrkit_ssb_test_fields()
        let code = sdrkit_ssb_test_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SsbTestFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `ssb_test` log.
public struct SsbTestEventRow {
    public enum Kind: Int32 {
        case none = 0
        case oneTone = 1
        case twoTones = 2
    }
    public enum Sideband: Int32 {
        case upper = 0
        case lower = 1
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The test seen.
    public var kind: Kind?
    /// The sideband of the tones.
    public var sideband: Sideband?
    /// The audio frequency of the (lower) tone.
    public var tone1Hz: Double?
    /// The audio frequency of the higher tone of two.
    public var tone2Hz: Double?
    /// The stronger tone over the weaker.
    public var toneLevelDiffDb: Double?
    /// The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search: the leak is at least this far under the tone.
    public var carrierDb: Double?
    /// No carrier is seen and the value is the floor of the carrier search, a bound.
    public var carrierUnder: Bool?
    /// A carrier was seen; else it is assumed at the tuned frequency for the tones and products.
    public var carrierFound: Bool?
    /// The opposite sideband of the (lower) tone.
    public var oppositeDb: Double?
    /// The opposite sideband is under the floor.
    public var oppositeUnder: Bool?
    /// Harmonic 2 of the audio tone (one tone).
    public var h2Db: Double?
    /// Harmonic 3 of the audio tone (one tone).
    public var h3Db: Double?
    /// Harmonic 4 of the audio tone (one tone).
    public var h4Db: Double?
    /// Harmonic 5 of the audio tone (one tone).
    public var h5Db: Double?
    /// Harmonic 2 is under the floor.
    public var h2Under: Bool?
    /// Harmonic 3 is under the floor.
    public var h3Under: Bool?
    /// Harmonic 4 is under the floor.
    public var h4Under: Bool?
    /// Harmonic 5 is under the floor.
    public var h5Under: Bool?
    /// The stronger product of order 3 under the weaker tone (two tones).
    public var imd3Db: Double?
    /// The stronger product of order 5 under the weaker tone (two tones).
    public var imd5Db: Double?
    /// The stronger product of order 7 under the weaker tone (two tones).
    public var imd7Db: Double?
    /// The products of order 3 are under the floor.
    public var imd3Under: Bool?
    /// The products of order 5 are under the floor.
    public var imd5Under: Bool?
    /// The products of order 7 are under the floor.
    public var imd7Under: Bool?
    /// The product of order 3 under the peak envelope power.
    public var imd3PepDb: Double?
    /// The product of order 5 under the peak envelope power.
    public var imd5PepDb: Double?
    /// The product of order 7 under the peak envelope power.
    public var imd7PepDb: Double?
    /// The floor of this measurement under the tone: the noise of 7 bins.
    public var floorDb: Double?

    init(_ c: sdrkit_ssb_test_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        sideband = c.has_sideband ? Sideband(rawValue: c.sideband) : nil
        tone1Hz = c.has_tone1_hz ? c.tone1_hz : nil
        tone2Hz = c.has_tone2_hz ? c.tone2_hz : nil
        toneLevelDiffDb = c.has_tone_level_diff_db ? c.tone_level_diff_db : nil
        carrierDb = c.has_carrier_db ? c.carrier_db : nil
        carrierUnder = c.has_carrier_under ? c.carrier_under : nil
        carrierFound = c.has_carrier_found ? c.carrier_found : nil
        oppositeDb = c.has_opposite_db ? c.opposite_db : nil
        oppositeUnder = c.has_opposite_under ? c.opposite_under : nil
        h2Db = c.has_h2_db ? c.h2_db : nil
        h3Db = c.has_h3_db ? c.h3_db : nil
        h4Db = c.has_h4_db ? c.h4_db : nil
        h5Db = c.has_h5_db ? c.h5_db : nil
        h2Under = c.has_h2_under ? c.h2_under : nil
        h3Under = c.has_h3_under ? c.h3_under : nil
        h4Under = c.has_h4_under ? c.h4_under : nil
        h5Under = c.has_h5_under ? c.h5_under : nil
        imd3Db = c.has_imd3_db ? c.imd3_db : nil
        imd5Db = c.has_imd5_db ? c.imd5_db : nil
        imd7Db = c.has_imd7_db ? c.imd7_db : nil
        imd3Under = c.has_imd3_under ? c.imd3_under : nil
        imd5Under = c.has_imd5_under ? c.imd5_under : nil
        imd7Under = c.has_imd7_under ? c.imd7_under : nil
        imd3PepDb = c.has_imd3_pep_db ? c.imd3_pep_db : nil
        imd5PepDb = c.has_imd5_pep_db ? c.imd5_pep_db : nil
        imd7PepDb = c.has_imd7_pep_db ? c.imd7_pep_db : nil
        floorDb = c.has_floor_db ? c.floor_db : nil
    }
}

/// The result fields of `afsk_tuner`: the last analysed burst and the totals.
public struct AfskTunerFields {
    /// The mark tone of the last burst.
    public var markHz: Double?
    /// The space tone of the last burst.
    public var spaceHz: Double?
    /// The mark tone minus the set mark tone.
    public var markErrorHz: Double?
    /// The space tone minus the set space tone.
    public var spaceErrorHz: Double?
    /// The twist of the last burst.
    public var twistDb: Double?
    /// The peak deviation of the last burst.
    public var peakDeviationKhz: Double?
    /// The length of the last burst.
    public var lengthS: Double?
    /// The gap before the last burst.
    public var gapS: Double?
    /// The bursts analysed.
    public var bursts: Int64
    /// The least peak deviation over the bursts.
    public var deviationMinKhz: Double?
    /// The largest peak deviation over the bursts.
    public var deviationMaxKhz: Double?

    init(_ c: sdrkit_afsk_tuner_fields) {
        markHz = c.has_mark_hz ? c.mark_hz : nil
        spaceHz = c.has_space_hz ? c.space_hz : nil
        markErrorHz = c.has_mark_error_hz ? c.mark_error_hz : nil
        spaceErrorHz = c.has_space_error_hz ? c.space_error_hz : nil
        twistDb = c.has_twist_db ? c.twist_db : nil
        peakDeviationKhz = c.has_peak_deviation_khz ? c.peak_deviation_khz : nil
        lengthS = c.has_length_s ? c.length_s : nil
        gapS = c.has_gap_s ? c.gap_s : nil
        bursts = c.bursts
        deviationMinKhz = c.has_deviation_min_khz ? c.deviation_min_khz : nil
        deviationMaxKhz = c.has_deviation_max_khz ? c.deviation_max_khz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AfskTunerFields, SDRKitCode> {
        var c = sdrkit_afsk_tuner_fields()
        let code = sdrkit_afsk_tuner_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AfskTunerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the burst table.
public struct AfskTunerBurstRow {
    /// The row key.
    public var key: String
    /// The start of the burst, signal time.
    public var timeS: Double
    /// The length of the burst.
    public var lengthS: Double
    /// The mark tone.
    public var markHz: Double
    /// The space tone.
    public var spaceHz: Double
    /// The space-band power over the mark-band power.
    public var twistDb: Double
    /// The 99.5th percentile of the band-limited audio.
    public var peakDeviationKhz: Double
    /// The time since the end of the burst before; no value for the first.
    public var gapS: Double?

    init(_ c: sdrkit_afsk_tuner_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        lengthS = c.length_s
        markHz = c.mark_hz
        spaceHz = c.space_hz
        twistDb = c.twist_db
        peakDeviationKhz = c.peak_deviation_khz
        gapS = c.has_gap_s ? c.gap_s : nil
    }
}

/// The result fields of `phase_noise`, from the average. They have no value before 4 blocks are averaged.
public struct PhaseNoiseFields {
    /// Single-sideband phase noise at 10 Hz from the carrier.
    public var l10hz: Double?
    /// Single-sideband phase noise at 100 Hz from the carrier.
    public var l100hz: Double?
    /// Single-sideband phase noise at 1 kHz from the carrier.
    public var l1khz: Double?
    /// Single-sideband phase noise at 10 kHz from the carrier.
    public var l10khz: Double?
    /// Single-sideband phase noise at 100 kHz from the carrier.
    public var l100khz: Double?
    /// The RMS phase jitter of the integration band.
    public var jitterMrad: Double?
    /// The jitter as time at the tuned frequency; no value without it.
    public var jitterPs: Double?
    /// The carrier offset from the tuned frequency.
    public var carrierOffsetHz: Double?
    /// The carrier over the noise in 200 kHz.
    public var cnrDb: Double?
    /// The blocks in the average.
    public var blocks: Int64
    /// The length of a block.
    public var blockS: Double?
    /// The width of a bin.
    public var binHz: Double?
    /// The equivalent noise bandwidth of the window.
    public var enbwBins: Double?

    init(_ c: sdrkit_phase_noise_fields) {
        l10hz = c.has_l_10hz ? c.l_10hz : nil
        l100hz = c.has_l_100hz ? c.l_100hz : nil
        l1khz = c.has_l_1khz ? c.l_1khz : nil
        l10khz = c.has_l_10khz ? c.l_10khz : nil
        l100khz = c.has_l_100khz ? c.l_100khz : nil
        jitterMrad = c.has_jitter_mrad ? c.jitter_mrad : nil
        jitterPs = c.has_jitter_ps ? c.jitter_ps : nil
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
        cnrDb = c.has_cnr_db ? c.cnr_db : nil
        blocks = c.blocks
        blockS = c.has_block_s ? c.block_s : nil
        binHz = c.has_bin_hz ? c.bin_hz : nil
        enbwBins = c.has_enbw_bins ? c.enbw_bins : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PhaseNoiseFields, SDRKitCode> {
        var c = sdrkit_phase_noise_fields()
        let code = sdrkit_phase_noise_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PhaseNoiseFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the spur table.
public struct PhaseNoiseSpurRow {
    /// The row key.
    public var key: String
    /// The offset of the spur from the carrier, upper side.
    public var offsetHz: Double
    /// The level of the spur.
    public var levelDbc: Double

    init(_ c: sdrkit_phase_noise_spur_row) {
        key = sdrkitText(c.key)
        offsetHz = c.offset_hz
        levelDbc = c.level_dbc
    }
}

/// A row of the `phase_noise` log.
public struct PhaseNoiseEventRow {
    public enum Kind: Int32 {
        case reading = 0
        case lost = 1
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// Single-sideband phase noise at 10 Hz from the carrier.
    public var l10hz: Double?
    /// Single-sideband phase noise at 100 Hz from the carrier.
    public var l100hz: Double?
    /// Single-sideband phase noise at 1 kHz from the carrier.
    public var l1khz: Double?
    /// Single-sideband phase noise at 10 kHz from the carrier.
    public var l10khz: Double?
    /// Single-sideband phase noise at 100 kHz from the carrier.
    public var l100khz: Double?
    /// The jitter.
    public var jitterMrad: Double?
    /// The spurs listed.
    public var spurs: Int64

    init(_ c: sdrkit_phase_noise_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        l10hz = c.has_l_10hz ? c.l_10hz : nil
        l100hz = c.has_l_100hz ? c.l_100hz : nil
        l1khz = c.has_l_1khz ? c.l_1khz : nil
        l10khz = c.has_l_10khz ? c.l_10khz : nil
        l100khz = c.has_l_100khz ? c.l_100khz : nil
        jitterMrad = c.has_jitter_mrad ? c.jitter_mrad : nil
        spurs = c.spurs
    }
}

/// The result fields of `cw_keying`: the evaluation of the last marks and spaces. The evaluation has no value before 10 marks and when no mark came in the last 8 s.
public struct CwKeyingFields {
    /// The speed in words a minute of 50 units.
    public var wpm: Double?
    /// The mean dit.
    public var ditMs: Double?
    /// The mean dah; no value with fewer than 2 dahs.
    public var dahMs: Double?
    /// The mean dah over the mean dit.
    public var dahRatio: Double?
    /// The mean gap inside a character, in dits.
    public var elementGap: Double?
    /// The mean gap between characters, in dits.
    public var charGap: Double?
    /// The mean gap between words, in dits.
    public var wordGap: Double?
    /// The character gaps counted.
    public var charGaps: Int64?
    /// The word gaps counted.
    public var wordGaps: Int64?
    /// The dit length over the element gap length.
    public var weight: Double?
    /// The population standard deviation of the dits over their mean.
    public var ditScatterPct: Double?
    /// The mean 10 to 90 % rise of the marks; 1.5 ms is the least the envelope filter shows.
    public var riseMs: Double?
    /// The mean 90 to 10 % fall of the marks.
    public var fallMs: Double?
    /// The mean frequency of the start of a mark minus that of its second half.
    public var chirpHz: Double?
    /// The marks the chirp is from.
    public var chirpMarks: Int64?
    /// The marks in the window.
    public var marks: Int64
    /// The spaces in the window.
    public var spaces: Int64
    /// The carrier offset from the tuned frequency.
    public var carrierOffsetHz: Double?

    init(_ c: sdrkit_cw_keying_fields) {
        wpm = c.has_wpm ? c.wpm : nil
        ditMs = c.has_dit_ms ? c.dit_ms : nil
        dahMs = c.has_dah_ms ? c.dah_ms : nil
        dahRatio = c.has_dah_ratio ? c.dah_ratio : nil
        elementGap = c.has_element_gap ? c.element_gap : nil
        charGap = c.has_char_gap ? c.char_gap : nil
        wordGap = c.has_word_gap ? c.word_gap : nil
        charGaps = c.has_char_gaps ? c.char_gaps : nil
        wordGaps = c.has_word_gaps ? c.word_gaps : nil
        weight = c.has_weight ? c.weight : nil
        ditScatterPct = c.has_dit_scatter_pct ? c.dit_scatter_pct : nil
        riseMs = c.has_rise_ms ? c.rise_ms : nil
        fallMs = c.has_fall_ms ? c.fall_ms : nil
        chirpHz = c.has_chirp_hz ? c.chirp_hz : nil
        chirpMarks = c.has_chirp_marks ? c.chirp_marks : nil
        marks = c.marks
        spaces = c.spaces
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CwKeyingFields, SDRKitCode> {
        var c = sdrkit_cw_keying_fields()
        let code = sdrkit_cw_keying_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CwKeyingFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `cw_keying` log.
public struct CwKeyingEventRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The speed.
    public var wpm: Double
    /// The mean dit.
    public var ditMs: Double
    /// The dah ratio.
    public var dahRatio: Double?
    /// The element gap.
    public var elementGap: Double?
    /// The character gap.
    public var charGap: Double?
    /// The word gap.
    public var wordGap: Double?
    /// The weight.
    public var weight: Double?
    /// The dit scatter.
    public var ditScatterPct: Double
    /// The rise.
    public var riseMs: Double?
    /// The fall.
    public var fallMs: Double?
    /// The chirp.
    public var chirpHz: Double?

    init(_ c: sdrkit_cw_keying_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        wpm = c.wpm
        ditMs = c.dit_ms
        dahRatio = c.has_dah_ratio ? c.dah_ratio : nil
        elementGap = c.has_element_gap ? c.element_gap : nil
        charGap = c.has_char_gap ? c.char_gap : nil
        wordGap = c.has_word_gap ? c.word_gap : nil
        weight = c.has_weight ? c.weight : nil
        ditScatterPct = c.dit_scatter_pct
        riseMs = c.has_rise_ms ? c.rise_ms : nil
        fallMs = c.has_fall_ms ? c.fall_ms : nil
        chirpHz = c.has_chirp_hz ? c.chirp_hz : nil
    }
}

/// The result fields of `fsk_analyser`, from the last block. They have no value while no FSK signal is found.
public struct FskAnalyserFields {
    public enum Levels: Int32 {
        case twoLevels = 0
        case fourLevels = 1
    }
    /// The number of levels.
    public var levels: Levels?
    /// Two levels with a modulation index of 0.5.
    public var mskLike: Bool?
    /// The symbol rate.
    public var symbolRateBd: Double?
    /// Two levels: the distance of the tones; four: the mean distance of neighbours.
    public var shiftHz: Double?
    /// The distance of the outer levels.
    public var outerHz: Double?
    /// The outer distance over the symbol rate.
    public var modIndex: Double?
    /// The centre of the levels from the tuned frequency.
    public var centreHz: Double?
    /// Four levels: the mean outer spacing over the inner spacing.
    public var spacingRatio: Double?
    /// Four levels: the spacing ratio is within 0.35 of 1.
    public var spacingEqual: Bool?
    /// The level spacing over the spread inside a level.
    public var eyeSnrDb: Double?

    init(_ c: sdrkit_fsk_analyser_fields) {
        levels = c.has_levels ? Levels(rawValue: c.levels) : nil
        mskLike = c.has_msk_like ? c.msk_like : nil
        symbolRateBd = c.has_symbol_rate_bd ? c.symbol_rate_bd : nil
        shiftHz = c.has_shift_hz ? c.shift_hz : nil
        outerHz = c.has_outer_hz ? c.outer_hz : nil
        modIndex = c.has_mod_index ? c.mod_index : nil
        centreHz = c.has_centre_hz ? c.centre_hz : nil
        spacingRatio = c.has_spacing_ratio ? c.spacing_ratio : nil
        spacingEqual = c.has_spacing_equal ? c.spacing_equal : nil
        eyeSnrDb = c.has_eye_snr_db ? c.eye_snr_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FskAnalyserFields, SDRKitCode> {
        var c = sdrkit_fsk_analyser_fields()
        let code = sdrkit_fsk_analyser_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FskAnalyserFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the level table.
public struct FskAnalyserLevelRow {
    /// The row key.
    public var key: String
    /// The place of the level from the lowest, from 0.
    public var index: Int64
    /// The frequency of the level from the tuned frequency.
    public var frequencyHz: Double

    init(_ c: sdrkit_fsk_analyser_level_row) {
        key = sdrkitText(c.key)
        index = c.index
        frequencyHz = c.frequency_hz
    }
}

/// A row of the `fsk_analyser` log.
public struct FskAnalyserEventRow {
    public enum Levels: Int32 {
        case twoLevels = 0
        case fourLevels = 1
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The number of levels.
    public var levels: Levels?
    /// The symbol rate.
    public var symbolRateBd: Double
    /// The shift.
    public var shiftHz: Double
    /// The modulation index.
    public var modIndex: Double
    /// The centre.
    public var centreHz: Double
    /// The eye SNR.
    public var eyeSnrDb: Double

    init(_ c: sdrkit_fsk_analyser_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        levels = Levels(rawValue: c.levels)
        symbolRateBd = c.symbol_rate_bd
        shiftHz = c.shift_hz
        modIndex = c.mod_index
        centreHz = c.centre_hz
        eyeSnrDb = c.eye_snr_db
    }
}

/// The result fields of `mer`, from the last block. The readings have no value without a lock.
public struct MerFields {
    public enum Modulation: Int32 {
        case bpsk = 0
        case qpsk = 1
        case psk8 = 2
    }
    /// A PSK carrier was found and measured in the last block.
    public var locked: Bool?
    /// The order.
    public var modulation: Modulation?
    /// The modulation error ratio after matched filtering, timing and phase recovery.
    public var merDb: Double?
    /// The RMS error vector, of the RMS amplitude.
    public var evmPct: Double?
    /// The symbol rate.
    public var symbolRateBd: Double?
    /// The carrier offset from the tuned frequency.
    public var carrierOffsetHz: Double?
    /// The roll-off of the best matched filter.
    public var rolloff: Double?
    /// The symbols measured in the last block.
    public var symbols: Int64?

    init(_ c: sdrkit_mer_fields) {
        locked = c.has_locked ? c.locked : nil
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        evmPct = c.has_evm_pct ? c.evm_pct : nil
        symbolRateBd = c.has_symbol_rate_bd ? c.symbol_rate_bd : nil
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
        rolloff = c.has_rolloff ? c.rolloff : nil
        symbols = c.has_symbols ? c.symbols : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MerFields, SDRKitCode> {
        var c = sdrkit_mer_fields()
        let code = sdrkit_mer_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `mer` log.
public struct MerEventRow {
    public enum Modulation: Int32 {
        case bpsk = 0
        case qpsk = 1
        case psk8 = 2
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The order.
    public var modulation: Modulation?
    /// MER.
    public var merDb: Double
    /// EVM.
    public var evmPct: Double
    /// The symbol rate.
    public var symbolRateBd: Double
    /// The carrier offset.
    public var carrierOffsetHz: Double
    /// The mean power of the block, relative to full scale.
    public var levelDb: Double

    init(_ c: sdrkit_mer_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        modulation = Modulation(rawValue: c.modulation)
        merDb = c.mer_db
        evmPct = c.evm_pct
        symbolRateBd = c.symbol_rate_bd
        carrierOffsetHz = c.carrier_offset_hz
        levelDb = c.level_db
    }
}

/// The result fields of `papr`, from the last evaluation over the kept samples.
public struct PaprFields {
    public enum Reading: Int32 {
        case constantEnvelope = 0
        case gaussianLike = 1
        case lessPeaky = 2
        case peakier = 3
    }
    /// The mean power, relative to full scale.
    public var meanDbfs: Double?
    /// The largest instantaneous power.
    public var peakDbfs: Double?
    /// The peak over the mean power.
    public var crestDb: Double?
    /// The level over the mean exceeded by 10 % of the samples.
    public var ccdf10Db: Double?
    /// The level exceeded by 1 %.
    public var ccdf1Db: Double?
    /// The level exceeded by 0.1 %.
    public var ccdf01Db: Double?
    /// The level exceeded by 0.01 %; only with a million samples or more.
    public var ccdf001Db: Double?
    /// The samples in the statistics.
    public var samples: Int64?
    /// The shape against Gaussian noise.
    public var reading: Reading?

    init(_ c: sdrkit_papr_fields) {
        meanDbfs = c.has_mean_dbfs ? c.mean_dbfs : nil
        peakDbfs = c.has_peak_dbfs ? c.peak_dbfs : nil
        crestDb = c.has_crest_db ? c.crest_db : nil
        ccdf10Db = c.has_ccdf_10_db ? c.ccdf_10_db : nil
        ccdf1Db = c.has_ccdf_1_db ? c.ccdf_1_db : nil
        ccdf01Db = c.has_ccdf_01_db ? c.ccdf_01_db : nil
        ccdf001Db = c.has_ccdf_001_db ? c.ccdf_001_db : nil
        samples = c.has_samples ? c.samples : nil
        reading = c.has_reading ? Reading(rawValue: c.reading) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PaprFields, SDRKitCode> {
        var c = sdrkit_papr_fields()
        let code = sdrkit_papr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PaprFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `papr` log.
public struct PaprEventRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The level at 10 %.
    public var ccdf10Db: Double
    /// The level at 1 %.
    public var ccdf1Db: Double
    /// The level at 0.1 %.
    public var ccdf01Db: Double
    /// The level at 0.01 %.
    public var ccdf001Db: Double?
    /// The crest factor.
    public var crestDb: Double
    /// The mean power.
    public var meanDbfs: Double
    /// The samples in the statistics.
    public var samples: Int64

    init(_ c: sdrkit_papr_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        ccdf10Db = c.ccdf_10_db
        ccdf1Db = c.ccdf_1_db
        ccdf01Db = c.ccdf_01_db
        ccdf001Db = c.has_ccdf_001_db ? c.ccdf_001_db : nil
        crestDb = c.crest_db
        meanDbfs = c.mean_dbfs
        samples = c.samples
    }
}

/// The result fields of `input_health`, from the last block of raw samples. Levels are against a full scale of 1.0 per component.
public struct InputHealthFields {
    public enum Verdict: Int32 {
        case low = 0
        case good = 1
        case high = 2
        case overload = 3
    }
    /// The RMS level, DC included.
    public var rmsDbfs: Double?
    /// The largest of `|I|` and `|Q|`.
    public var peakDbfs: Double?
    /// The peak over the RMS level.
    public var crestDb: Double?
    /// The samples with `|I|` or `|Q|` at the clip level or over it.
    public var clippingPpm: Double?
    /// The mean of I.
    public var dcIPct: Double?
    /// The mean of Q.
    public var dcQPct: Double?
    /// The length of the mean vector.
    public var dcPct: Double?
    /// The power of the mean vector under the RMS level.
    public var dcUnderRmsDb: Double?
    /// `10 log10(V_Q / V_I)`.
    public var iqGainDb: Double?
    /// `asin` of the correlation of I and Q.
    public var iqPhaseDeg: Double?
    /// The image rejection that the mismatch implies, at most 120 dB.
    public var imageRejectionDb: Double?
    /// The level verdict.
    public var verdict: Verdict?

    init(_ c: sdrkit_input_health_fields) {
        rmsDbfs = c.has_rms_dbfs ? c.rms_dbfs : nil
        peakDbfs = c.has_peak_dbfs ? c.peak_dbfs : nil
        crestDb = c.has_crest_db ? c.crest_db : nil
        clippingPpm = c.has_clipping_ppm ? c.clipping_ppm : nil
        dcIPct = c.has_dc_i_pct ? c.dc_i_pct : nil
        dcQPct = c.has_dc_q_pct ? c.dc_q_pct : nil
        dcPct = c.has_dc_pct ? c.dc_pct : nil
        dcUnderRmsDb = c.has_dc_under_rms_db ? c.dc_under_rms_db : nil
        iqGainDb = c.has_iq_gain_db ? c.iq_gain_db : nil
        iqPhaseDeg = c.has_iq_phase_deg ? c.iq_phase_deg : nil
        imageRejectionDb = c.has_image_rejection_db ? c.image_rejection_db : nil
        verdict = c.has_verdict ? Verdict(rawValue: c.verdict) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<InputHealthFields, SDRKitCode> {
        var c = sdrkit_input_health_fields()
        let code = sdrkit_input_health_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(InputHealthFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `input_health` log.
public struct InputHealthEventRow {
    public enum Kind: Int32 {
        case verdict = 0
        case clipping = 1
    }
    public enum Verdict: Int32 {
        case low = 0
        case good = 1
        case high = 2
        case overload = 3
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// What the row reports.
    public var kind: Kind?
    /// The verdict.
    public var verdict: Verdict?
    /// The RMS level.
    public var rmsDbfs: Double
    /// The peak.
    public var peakDbfs: Double
    /// The samples near full scale.
    public var clippingPpm: Double

    init(_ c: sdrkit_input_health_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        verdict = Verdict(rawValue: c.verdict)
        rmsDbfs = c.rms_dbfs
        peakDbfs = c.peak_dbfs
        clippingPpm = c.clipping_ppm
    }
}

/// The result fields of `noise_figure`. Levels are relative and uncalibrated.
public struct NoiseFigureFields {
    /// The level of the present plateau of the power.
    public var powerDb: Double?
    /// The Y-factor: the mean of the kept steps.
    public var yDb: Double?
    /// The Y-factor as a ratio.
    public var yLin: Double?
    /// The sample standard deviation of the kept steps.
    public var ySpreadDb: Double?
    /// The steps seen.
    public var steps: Int64

    init(_ c: sdrkit_noise_figure_fields) {
        powerDb = c.has_power_db ? c.power_db : nil
        yDb = c.has_y_db ? c.y_db : nil
        yLin = c.has_y_lin ? c.y_lin : nil
        ySpreadDb = c.has_y_spread_db ? c.y_spread_db : nil
        steps = c.steps
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NoiseFigureFields, SDRKitCode> {
        var c = sdrkit_noise_figure_fields()
        let code = sdrkit_noise_figure_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NoiseFigureFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the noise figure table: one source ENR of the setting.
public struct NoiseFigureRow {
    /// The row key.
    public var key: String
    /// The excess noise ratio of the source.
    public var enrDb: Double
    /// `ENR - 10 log10(Y - 1)`; no value when Y is 1 or less.
    public var nfDb: Double?
    /// The noise figure is negative: the ENR is lower than the step implies.
    public var negative: Bool

    init(_ c: sdrkit_noise_figure_row) {
        key = sdrkitText(c.key)
        enrDb = c.enr_db
        nfDb = c.has_nf_db ? c.nf_db : nil
        negative = c.negative
    }
}

/// A row of the `noise_figure` log: one step.
public struct NoiseFigureEventRow {
    public enum State: Int32 {
        case sourceOn = 0
        case sourceOff = 1
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The state of the source after the step.
    public var state: State?
    /// The plateau before.
    public var fromDb: Double
    /// The plateau after.
    public var toDb: Double
    /// The size of the step.
    public var yDb: Double
    /// The size as a ratio.
    public var yLin: Double

    init(_ c: sdrkit_noise_figure_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        state = State(rawValue: c.state)
        fromDb = c.from_db
        toDb = c.to_db
        yDb = c.y_db
        yLin = c.y_lin
    }
}

/// The result fields of `kurtosis`, from the last evaluation.
public struct KurtosisFields {
    /// The share of the used bins whose SK is within the limits.
    public var cleanPct: Double?
    /// The expected standard deviation of SK for Gaussian noise.
    public var sigma: Double
    /// The flagged regions.
    public var regions: Int64?
    /// The time of one evaluation.
    public var blockMs: Double

    init(_ c: sdrkit_kurtosis_fields) {
        cleanPct = c.has_clean_pct ? c.clean_pct : nil
        sigma = c.sigma
        regions = c.has_regions ? c.regions : nil
        blockMs = c.block_ms
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<KurtosisFields, SDRKitCode> {
        var c = sdrkit_kurtosis_fields()
        let code = sdrkit_kurtosis_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(KurtosisFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A flagged region of `kurtosis`.
public struct KurtosisRegionRow {
    public enum Kind: Int32 {
        case pulsed = 0
        case steady = 1
    }
    /// The row key.
    public var key: String
    /// The low edge.
    public var low: Double
    /// The high edge.
    public var high: Double
    /// The kind.
    public var kind: Kind?
    /// The mean SK of its bins.
    public var sk: Double
    /// The share of frames at full power, `1 / (SK + 1)`, for a pulsed region with SK over 1.5.
    public var fullPowerPct: Double?
    /// The mean power over the noise per bin.
    public var overNoiseDb: Double

    init(_ c: sdrkit_kurtosis_region_row) {
        key = sdrkitText(c.key)
        low = c.low
        high = c.high
        kind = Kind(rawValue: c.kind)
        sk = c.sk
        fullPowerPct = c.has_full_power_pct ? c.full_power_pct : nil
        overNoiseDb = c.over_noise_db
    }
}

/// A row of the `kurtosis` log.
public struct KurtosisEventRow {
    public enum Kind: Int32 {
        case pulsed = 0
        case steady = 1
        case gone = 2
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// What happened.
    public var kind: Kind?
    /// The centre of the region.
    public var centre: Double
    /// The low edge.
    public var low: Double
    /// The high edge.
    public var high: Double
    /// The mean SK.
    public var sk: Double
    /// The mean power over the noise.
    public var overNoiseDb: Double

    init(_ c: sdrkit_kurtosis_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        centre = c.centre
        low = c.low
        high = c.high
        sk = c.sk
        overNoiseDb = c.over_noise_db
    }
}

/// The result fields of `glitch`.
public struct GlitchFields {
    /// The steady tone whose phase is watched; no value while none is found.
    public var toneHz: Double?
    /// Sample drop events.
    public var sampleDrops: Int64
    /// Sample repeat events.
    public var sampleRepeats: Int64
    /// Zero runs.
    public var zeroRuns: Int64
    /// Repeated blocks.
    public var repeatedBlocks: Int64
    /// Level steps.
    public var levelSteps: Int64
    /// The time observed since the start or the last retune.
    public var elapsedS: Double

    init(_ c: sdrkit_glitch_fields) {
        toneHz = c.has_tone_hz ? c.tone_hz : nil
        sampleDrops = c.sample_drops
        sampleRepeats = c.sample_repeats
        zeroRuns = c.zero_runs
        repeatedBlocks = c.repeated_blocks
        levelSteps = c.level_steps
        elapsedS = c.elapsed_s
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GlitchFields, SDRKitCode> {
        var c = sdrkit_glitch_fields()
        let code = sdrkit_glitch_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GlitchFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `glitch` log: one stream fault.
public struct GlitchEventRow {
    public enum Kind: Int32 {
        case droppedSamples = 0
        case repeatedSamples = 1
        case zeroRun = 2
        case repeatedBlock = 3
        case levelStep = 4
    }
    /// The row key.
    public var key: String
    /// The signal time of the fault.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The samples missing or repeated: the smallest count that fits the phase step.
    public var samples: Int64?
    /// The phase step of the tone.
    public var phaseStepRad: Double?
    /// The size of a level step.
    public var stepDb: Double?
    /// The level before a step.
    public var fromDb: Double?
    /// The level after a step.
    public var toDb: Double?

    init(_ c: sdrkit_glitch_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        samples = c.has_samples ? c.samples : nil
        phaseStepRad = c.has_phase_step_rad ? c.phase_step_rad : nil
        stepDb = c.has_step_db ? c.step_db : nil
        fromDb = c.has_from_db ? c.from_db : nil
        toDb = c.has_to_db ? c.to_db : nil
    }
}

/// The result fields of `intermod`, from the last evaluation.
public struct IntermodFields {
    /// The strong signals used as parents.
    public var inputs: Int64?
    /// The peaks at product frequencies.
    public var products: Int64?
    /// Those that follow the law.
    public var productsFollow: Int64?
    /// Those called independent.
    public var productsIndependent: Int64?
    /// Those unclear or that cannot be told.
    public var productsUnknown: Int64?
    /// The snapshots taken.
    public var snapshots: Int64

    init(_ c: sdrkit_intermod_fields) {
        inputs = c.has_inputs ? c.inputs : nil
        products = c.has_products ? c.products : nil
        productsFollow = c.has_products_follow ? c.products_follow : nil
        productsIndependent = c.has_products_independent ? c.products_independent : nil
        productsUnknown = c.has_products_unknown ? c.products_unknown : nil
        snapshots = c.snapshots
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IntermodFields, SDRKitCode> {
        var c = sdrkit_intermod_fields()
        let code = sdrkit_intermod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IntermodFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A strong signal of `intermod`.
public struct IntermodInputRow {
    /// The row key.
    public var key: String
    /// The frequency (an offset from the centre when the tuned frequency is not known).
    public var frequency: Double
    /// The mean level over the noise.
    public var levelDb: Double

    init(_ c: sdrkit_intermod_input_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        levelDb = c.level_db
    }
}

/// A peak at a product frequency.
public struct IntermodProductRow {
    public enum Kind: Int32 {
        case n2aB = 0
        case n2bA = 1
        case bA = 2
    }
    public enum Verdict: Int32 {
        case follows = 0
        case independent = 1
        case unclear = 2
        case cannotTell = 3
    }
    /// The row key.
    public var key: String
    /// The frequency of the peak.
    public var frequency: Double
    /// The kind of product.
    public var kind: Kind?
    /// The lower parent.
    public var parentA: Double
    /// The upper parent.
    public var parentB: Double
    /// The mean level under the weaker parent.
    public var belowDb: Double
    /// The verdict of the level fit.
    public var verdict: Verdict?
    /// The slope of its level against the law.
    public var slope: Double?
    /// The correlation of its level with the law.
    public var correlation: Double?
    /// The snapshots of the fit.
    public var points: Int64

    init(_ c: sdrkit_intermod_product_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        kind = Kind(rawValue: c.kind)
        parentA = c.parent_a
        parentB = c.parent_b
        belowDb = c.below_db
        verdict = Verdict(rawValue: c.verdict)
        slope = c.has_slope ? c.slope : nil
        correlation = c.has_correlation ? c.correlation : nil
        points = c.points
    }
}

/// A row of the `intermod` log.
public struct IntermodEventRow {
    public enum Kind: Int32 {
        case n2aB = 0
        case n2bA = 1
        case bA = 2
    }
    public enum Verdict: Int32 {
        case follows = 0
        case independent = 1
        case unclear = 2
        case cannotTell = 3
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The frequency of the peak.
    public var frequency: Double
    /// The kind of product.
    public var kind: Kind?
    /// The lower parent.
    public var parentA: Double
    /// The upper parent.
    public var parentB: Double
    /// The mean level under the weaker parent.
    public var belowDb: Double
    /// The verdict.
    public var verdict: Verdict?
    /// The slope.
    public var slope: Double?
    /// The correlation.
    public var correlation: Double?
    /// The snapshots of the fit.
    public var points: Int64

    init(_ c: sdrkit_intermod_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        frequency = c.frequency
        kind = Kind(rawValue: c.kind)
        parentA = c.parent_a
        parentB = c.parent_b
        belowDb = c.below_db
        verdict = Verdict(rawValue: c.verdict)
        slope = c.has_slope ? c.slope : nil
        correlation = c.has_correlation ? c.correlation : nil
        points = c.points
    }
}

/// The result fields of `spur`, from the last evaluation. The findings hold for this radio at this gain, rate and tuning.
public struct SpurFields {
    /// The largest bin within 2 bins of 0 Hz over the noise.
    public var dcDb: Double?
    /// The DC spike is over its threshold.
    public var dcSpike: Bool?
    /// The strong signals.
    public var signals: Int64?
    /// Images not called independent.
    public var images: Int64?
    /// Harmonics not called independent.
    public var harmonics: Int64?
    /// Fixed spurs.
    public var fixedSpurs: Int64?
    /// The snapshots taken.
    public var snapshots: Int64

    init(_ c: sdrkit_spur_fields) {
        dcDb = c.has_dc_db ? c.dc_db : nil
        dcSpike = c.has_dc_spike ? c.dc_spike : nil
        signals = c.has_signals ? c.signals : nil
        images = c.has_images ? c.images : nil
        harmonics = c.has_harmonics ? c.harmonics : nil
        fixedSpurs = c.has_fixed_spurs ? c.fixed_spurs : nil
        snapshots = c.snapshots
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SpurFields, SDRKitCode> {
        var c = sdrkit_spur_fields()
        let code = sdrkit_spur_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SpurFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A finding of `spur`.
public struct SpurFindingRow {
    public enum Kind: Int32 {
        case image = 0
        case secondHarmonic = 1
        case thirdHarmonic = 2
        case fixedSpur = 3
    }
    public enum Verdict: Int32 {
        case follows = 0
        case independent = 1
        case unclear = 2
        case cannotTell = 3
        case fixed = 4
    }
    /// The row key.
    public var key: String
    /// The kind.
    public var kind: Kind?
    /// The frequency of the peak.
    public var frequency: Double
    /// The signal that it belongs to; none for a fixed spur.
    public var fromSignal: Double?
    /// The mean level under the signal; for a fixed spur the mean level over the noise.
    public var belowDb: Double
    /// The verdict.
    public var verdict: Verdict?
    /// The slope of its level against the signal's.
    public var slope: Double?
    /// The correlation.
    public var correlation: Double?
    /// The image rejection (images).
    public var imageRejectionDb: Double?
    /// The gain mismatch alone that gives this image.
    public var gainMismatchDb: Double?
    /// The phase mismatch alone that gives this image.
    public var phaseMismatchDeg: Double?

    init(_ c: sdrkit_spur_finding_row) {
        key = sdrkitText(c.key)
        kind = Kind(rawValue: c.kind)
        frequency = c.frequency
        fromSignal = c.has_from_signal ? c.from_signal : nil
        belowDb = c.below_db
        verdict = Verdict(rawValue: c.verdict)
        slope = c.has_slope ? c.slope : nil
        correlation = c.has_correlation ? c.correlation : nil
        imageRejectionDb = c.has_image_rejection_db ? c.image_rejection_db : nil
        gainMismatchDb = c.has_gain_mismatch_db ? c.gain_mismatch_db : nil
        phaseMismatchDeg = c.has_phase_mismatch_deg ? c.phase_mismatch_deg : nil
    }
}

/// A row of the `spur` log.
public struct SpurEventRow {
    public enum Kind: Int32 {
        case image = 0
        case secondHarmonic = 1
        case thirdHarmonic = 2
        case fixedSpur = 3
        case dc = 4
    }
    public enum Verdict: Int32 {
        case follows = 0
        case independent = 1
        case unclear = 2
        case cannotTell = 3
        case fixed = 4
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The frequency of the peak.
    public var frequency: Double
    /// The signal that it belongs to.
    public var fromSignal: Double?
    /// The mean level under the signal, or over the noise.
    public var belowDb: Double?
    /// The verdict.
    public var verdict: Verdict?
    /// The slope.
    public var slope: Double?
    /// The correlation.
    public var correlation: Double?
    /// The image rejection.
    public var imageRejectionDb: Double?
    /// The gain mismatch alone.
    public var gainMismatchDb: Double?
    /// The phase mismatch alone.
    public var phaseMismatchDeg: Double?
    /// The DC spike over the noise (kind dc).
    public var dcDb: Double?

    init(_ c: sdrkit_spur_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        frequency = c.frequency
        fromSignal = c.has_from_signal ? c.from_signal : nil
        belowDb = c.has_below_db ? c.below_db : nil
        verdict = c.has_verdict ? Verdict(rawValue: c.verdict) : nil
        slope = c.has_slope ? c.slope : nil
        correlation = c.has_correlation ? c.correlation : nil
        imageRejectionDb = c.has_image_rejection_db ? c.image_rejection_db : nil
        gainMismatchDb = c.has_gain_mismatch_db ? c.gain_mismatch_db : nil
        phaseMismatchDeg = c.has_phase_mismatch_deg ? c.phase_mismatch_deg : nil
        dcDb = c.has_dc_db ? c.dc_db : nil
    }
}

/// The result fields of `fading`: the statistics of the envelope of one carrier over the window, and the carrier now.
public struct FadingFields {
    /// The Rician K-factor; 40 dB for no fading, no value for K = 0 (Rayleigh).
    public var kDb: Double?
    /// The K-factor as a ratio.
    public var k: Double?
    /// No steady component: K = 0.
    public var rayleigh: Bool?
    /// The level exceeded by 99 % of the time, against the mean power.
    public var p1Db: Double?
    /// The level at 10 %.
    public var p10Db: Double?
    /// The median level.
    public var p50Db: Double?
    /// The level at 90 %.
    public var p90Db: Double?
    /// The level at 99 %.
    public var p99Db: Double?
    /// The median over the 1 % level.
    public var fadeDepthDb: Double?
    /// The scintillation index S4.
    public var s4: Double?
    /// Downward crossings of the fade threshold per second.
    public var fadeRateHz: Double?
    /// The average time under the fade threshold.
    public var fadeLengthMs: Double?
    /// The RMS Doppler spread.
    public var dopplerHz: Double?
    /// The carrier to noise density of the last block.
    public var cn0Dbhz: Double?
    /// The carrier from the tuned frequency.
    public var carrierOffsetHz: Double?
    /// The envelope kept.
    public var windowS: Double?

    init(_ c: sdrkit_fading_fields) {
        kDb = c.has_k_db ? c.k_db : nil
        k = c.has_k ? c.k : nil
        rayleigh = c.has_rayleigh ? c.rayleigh : nil
        p1Db = c.has_p1_db ? c.p1_db : nil
        p10Db = c.has_p10_db ? c.p10_db : nil
        p50Db = c.has_p50_db ? c.p50_db : nil
        p90Db = c.has_p90_db ? c.p90_db : nil
        p99Db = c.has_p99_db ? c.p99_db : nil
        fadeDepthDb = c.has_fade_depth_db ? c.fade_depth_db : nil
        s4 = c.has_s4 ? c.s4 : nil
        fadeRateHz = c.has_fade_rate_hz ? c.fade_rate_hz : nil
        fadeLengthMs = c.has_fade_length_ms ? c.fade_length_ms : nil
        dopplerHz = c.has_doppler_hz ? c.doppler_hz : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
        windowS = c.has_window_s ? c.window_s : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FadingFields, SDRKitCode> {
        var c = sdrkit_fading_fields()
        let code = sdrkit_fading_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FadingFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `fading` log.
public struct FadingEventRow {
    public enum Kind: Int32 {
        case found = 0
        case lost = 1
        case fade = 2
        case reading = 3
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// What happened.
    public var kind: Kind?
    /// The deepest level of a fade against the mean power.
    public var depthDb: Double?
    /// The length of a fade.
    public var lengthMs: Double?
    /// The K-factor.
    public var kDb: Double?
    /// The level at 10 %.
    public var p10Db: Double?
    /// The fade rate.
    public var fadeRateHz: Double?
    /// The average fade length.
    public var fadeLengthMs: Double?
    /// The Doppler spread.
    public var dopplerHz: Double?
    /// The carrier to noise density.
    public var cn0Dbhz: Double?

    init(_ c: sdrkit_fading_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        depthDb = c.has_depth_db ? c.depth_db : nil
        lengthMs = c.has_length_ms ? c.length_ms : nil
        kDb = c.has_k_db ? c.k_db : nil
        p10Db = c.has_p10_db ? c.p10_db : nil
        fadeRateHz = c.has_fade_rate_hz ? c.fade_rate_hz : nil
        fadeLengthMs = c.has_fade_length_ms ? c.fade_length_ms : nil
        dopplerHz = c.has_doppler_hz ? c.doppler_hz : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
    }
}

/// The result fields of `beacon_monitor`.
public struct BeaconMonitorFields {
    public enum Class: Int32 {
        case none = 0
        case steady = 1
        case fading = 2
        case auroraLike = 3
    }
    /// The class of the path.
    public var `class`: Class?
    /// The line over the noise in a bin, last spectrum.
    public var levelDb: Double?
    /// The Doppler spread of the line (a Gaussian full width at half height), last spectrum.
    public var spreadHz: Double?
    /// The largest minus the smallest level of the last 12 s, at the last decision.
    public var swingDb: Double?
    /// The line from the tuned frequency, last spectrum.
    public var lineOffsetHz: Double?
    /// The class changes.
    public var changes: Int64

    init(_ c: sdrkit_beacon_monitor_fields) {
        `class` = Class(rawValue: c.class_)
        levelDb = c.has_level_db ? c.level_db : nil
        spreadHz = c.has_spread_hz ? c.spread_hz : nil
        swingDb = c.has_swing_db ? c.swing_db : nil
        lineOffsetHz = c.has_line_offset_hz ? c.line_offset_hz : nil
        changes = c.changes
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BeaconMonitorFields, SDRKitCode> {
        var c = sdrkit_beacon_monitor_fields()
        let code = sdrkit_beacon_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BeaconMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A change of class of `beacon_monitor`.
public struct BeaconMonitorEventRow {
    public enum FromClass: Int32 {
        case none = 0
        case steady = 1
        case fading = 2
        case auroraLike = 3
    }
    public enum ToClass: Int32 {
        case none = 0
        case steady = 1
        case fading = 2
        case auroraLike = 3
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The class before.
    public var fromClass: FromClass?
    /// The class after.
    public var toClass: ToClass?
    /// The last level.
    public var levelDb: Double?
    /// The median spread of the decision.
    public var spreadHz: Double?
    /// The swing of the decision.
    public var swingDb: Double?

    init(_ c: sdrkit_beacon_monitor_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        fromClass = FromClass(rawValue: c.from_class)
        toClass = ToClass(rawValue: c.to_class)
        levelDb = c.has_level_db ? c.level_db : nil
        spreadHz = c.has_spread_hz ? c.spread_hz : nil
        swingDb = c.has_swing_db ? c.swing_db : nil
    }
}

/// The pilot of an FM station was present or absent at the first measurement, came or went.
public struct FmMonitorPilotEvent {
    public enum Change: Int32 {
        case present = 0
        case absent = 1
        case lost = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// What changed.
    public var change: Change?
    /// The pilot of the second, peak equivalent.
    public var pilotKhz: Double

    init(_ c: sdrkit_fm_monitor_pilot_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        change = Change(rawValue: c.change)
        pilotKhz = c.pilot_khz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FmMonitorPilotEvent, SDRKitCode> {
        var c = sdrkit_fm_monitor_pilot_event()
        let code = sdrkit_fm_monitor_pilot_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FmMonitorPilotEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// RDS came or went.
public struct FmMonitorRdsEvent {
    public enum Change: Int32 {
        case present = 0
        case lost = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// What changed.
    public var change: Change?
    /// The RDS injection of the second, peak equivalent.
    public var rdsKhz: Double

    init(_ c: sdrkit_fm_monitor_rds_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        change = Change(rawValue: c.change)
        rdsKhz = c.rds_khz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FmMonitorRdsEvent, SDRKitCode> {
        var c = sdrkit_fm_monitor_rds_event()
        let code = sdrkit_fm_monitor_rds_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FmMonitorRdsEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A subcarrier at 67 or 92 kHz came or went.
public struct FmMonitorScaEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The subcarrier, 67 or 92.
    public var subcarrierKhz: Double
    /// The subcarrier came (true) or went.
    public var on: Bool
    /// Its level over the floor in that second.
    public var levelDb: Double?

    init(_ c: sdrkit_fm_monitor_sca_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        subcarrierKhz = c.subcarrier_khz
        on = c.on
        levelDb = c.has_level_db ? c.level_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FmMonitorScaEvent, SDRKitCode> {
        var c = sdrkit_fm_monitor_sca_event()
        let code = sdrkit_fm_monitor_sca_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FmMonitorScaEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A second whose peak deviation was over the threshold.
public struct FmMonitorOverDeviationEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The peak deviation of the second.
    public var peakDeviationKhz: Double
    /// The MPX power of the second.
    public var mpxPowerDbr: Double

    init(_ c: sdrkit_fm_monitor_over_deviation_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        peakDeviationKhz = c.peak_deviation_khz
        mpxPowerDbr = c.mpx_power_dbr
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FmMonitorOverDeviationEvent, SDRKitCode> {
        var c = sdrkit_fm_monitor_over_deviation_event()
        let code = sdrkit_fm_monitor_over_deviation_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FmMonitorOverDeviationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The carrier of an AM station was seen, was lost or came back.
public struct AmMonitorCarrierEvent {
    public enum Change: Int32 {
        case present = 0
        case lost = 1
        case returned = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// What changed.
    public var change: Change?
    /// The carrier offset of the second.
    public var carrierOffsetHz: Double
    /// The carrier level of the second.
    public var carrierDb: Double

    init(_ c: sdrkit_am_monitor_carrier_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        change = Change(rawValue: c.change)
        carrierOffsetHz = c.carrier_offset_hz
        carrierDb = c.carrier_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AmMonitorCarrierEvent, SDRKitCode> {
        var c = sdrkit_am_monitor_carrier_event()
        let code = sdrkit_am_monitor_carrier_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AmMonitorCarrierEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A second of an AM station was over-modulated.
public struct AmMonitorOverModulationEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The share of the second at zero carrier.
    public var overModPct: Double
    /// The positive peak of the second.
    public var peakPosPct: Double

    init(_ c: sdrkit_am_monitor_over_modulation_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        overModPct = c.over_mod_pct
        peakPosPct = c.peak_pos_pct
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AmMonitorOverModulationEvent, SDRKitCode> {
        var c = sdrkit_am_monitor_over_modulation_event()
        let code = sdrkit_am_monitor_over_modulation_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AmMonitorOverModulationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A transmission of a channel ended.
public struct OccupancyActivityEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The end of the transmission, signal time.
    public var timeS: Double
    /// The centre of the channel.
    public var frequency: Double
    /// The length of the transmission.
    public var lengthS: Double
    /// The highest channel power over the floor.
    public var peakOverFloorDb: Double
    /// The highest channel power, relative, per bin.
    public var levelDb: Double

    init(_ c: sdrkit_occupancy_activity_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        frequency = c.frequency
        lengthS = c.length_s
        peakOverFloorDb = c.peak_over_floor_db
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<OccupancyActivityEvent, SDRKitCode> {
        var c = sdrkit_occupancy_activity_event()
        let code = sdrkit_occupancy_activity_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(OccupancyActivityEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the noise floor log was written.
public struct NoiseFloorEvent {
    public enum Kind: Int32 {
        case rise = 0
        case back = 1
        case wholeBand = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the row.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The centre of the sub-band; no value for the whole band.
    public var frequency: Double?
    /// The sub-band, from 1; no value for the whole band.
    public var subBand: Int64?
    /// The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band.
    public var riseDb: Double
    /// The baseline of the sub-band.
    public var baselineDb: Double?
    /// How long the rise lasted (rows of the fall back).
    public var durationS: Double?
    /// The floor of the sub-band, or of the band, that second.
    public var levelDb: Double?

    init(_ c: sdrkit_noise_floor_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        frequency = c.has_frequency ? c.frequency : nil
        subBand = c.has_sub_band ? c.sub_band : nil
        riseDb = c.rise_db
        baselineDb = c.has_baseline_db ? c.baseline_db : nil
        durationS = c.has_duration_s ? c.duration_s : nil
        levelDb = c.has_level_db ? c.level_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<NoiseFloorEvent, SDRKitCode> {
        var c = sdrkit_noise_floor_event()
        let code = sdrkit_noise_floor_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(NoiseFloorEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst was found.
public struct BurstFinderBurstFoundEvent {
    public enum Class: Int32 {
        case narrow = 0
        case wide = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The start of the burst, signal time.
    public var timeS: Double
    /// The median centre of the burst.
    public var frequency: Double
    /// The length of the burst.
    public var lengthMs: Double
    /// The bandwidth class.
    public var `class`: Class?
    /// The median width, at least one bin.
    public var widthKhz: Double
    /// The mean peak over the median of the frames.
    public var levelDb: Double

    init(_ c: sdrkit_burst_finder_burst_found_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        frequency = c.frequency
        lengthMs = c.length_ms
        `class` = Class(rawValue: c.class_)
        widthKhz = c.width_khz
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BurstFinderBurstFoundEvent, SDRKitCode> {
        var c = sdrkit_burst_finder_burst_found_event()
        let code = sdrkit_burst_finder_burst_found_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BurstFinderBurstFoundEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A source was found repeating.
public struct BurstFinderBurstRepeatingEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The frequency of the source: its first burst.
    public var frequency: Double
    /// The median interval between its bursts.
    public var periodS: Double
    /// Its bursts in the history.
    public var bursts: Int64
    /// The RMS deviation of the agreeing intervals from the period, over the period.
    public var spreadPct: Double

    init(_ c: sdrkit_burst_finder_burst_repeating_event) {
        time = c.time
        freqHz = c.freq_hz
        frequency = c.frequency
        periodS = c.period_s
        bursts = c.bursts
        spreadPct = c.spread_pct
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BurstFinderBurstRepeatingEvent, SDRKitCode> {
        var c = sdrkit_burst_finder_burst_repeating_event()
        let code = sdrkit_burst_finder_burst_repeating_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BurstFinderBurstRepeatingEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A hopper was found, or its rate or its channel count changed.
public struct HopperChangedEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the evaluation.
    public var timeS: Double
    /// The hop rate.
    public var hopRate: Double
    /// The channels.
    public var channels: Int64
    /// The channel spacing.
    public var spacingKhz: Double
    /// The span.
    public var spanKhz: Double
    /// The lowest channel.
    public var lowest: Double
    /// The highest channel.
    public var highest: Double
    /// The median dwell.
    public var dwellMs: Double
    /// The median guard.
    public var guardMs: Double
    /// The mean dwell level over the frame median.
    public var levelDb: Double

    init(_ c: sdrkit_hopper_changed_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        hopRate = c.hop_rate
        channels = c.channels
        spacingKhz = c.spacing_khz
        spanKhz = c.span_khz
        lowest = c.lowest
        highest = c.highest
        dwellMs = c.dwell_ms
        guardMs = c.guard_ms
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HopperChangedEvent, SDRKitCode> {
        var c = sdrkit_hopper_changed_event()
        let code = sdrkit_hopper_changed_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HopperChangedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A persistent carrier was listed, came back or went.
public struct BirdieEvent {
    public enum Kind: Int32 {
        case new = 0
        case returned = 1
        case gone = 2
    }
    public enum Class: Int32 {
        case bare = 0
        case modulated = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the row.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The frequency.
    public var frequency: Double
    /// The mean level over the noise.
    public var levelDb: Double
    /// The class.
    public var `class`: Class?
    /// The drift.
    public var driftHzMin: Double
    /// The scatter.
    public var scatterHz: Double

    init(_ c: sdrkit_birdie_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        frequency = c.frequency
        levelDb = c.level_db
        `class` = Class(rawValue: c.class_)
        driftHzMin = c.drift_hz_min
        scatterHz = c.scatter_hz
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BirdieEvent, SDRKitCode> {
        var c = sdrkit_birdie_event()
        let code = sdrkit_birdie_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BirdieEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An evaluation was logged.
public struct ObwReadingEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the evaluation.
    public var timeS: Double
    /// The band that holds the power share, half the rest below and half above it.
    public var obwKhz: Double
    /// The lower edge of the occupied band.
    public var obwLow: Double
    /// The upper edge of the occupied band.
    public var obwHigh: Double
    /// The width at 3 dB under the peak of the smoothed spectrum.
    public var bw3Khz: Double
    /// The width at 6 dB under the peak.
    public var bw6Khz: Double
    /// The width at 20 dB under the peak.
    public var bw20Khz: Double
    /// The width at 26 dB under the peak.
    public var bw26Khz: Double
    /// The middle of the occupied band from the tuned frequency.
    public var centreOffsetHz: Double
    /// The power between the edges over the floor, dB re full scale.
    public var powerDb: Double
    /// The signal power over the floor in the same bins.
    public var snrDb: Double
    /// The power of the band one adjacent offset below, under the signal; no value outside the spectrum.
    public var adjacentLowDb: Double?
    /// The same above.
    public var adjacentHighDb: Double?
    /// The power of the band two adjacent offsets below, under the signal.
    public var alternateLowDb: Double?
    /// The same above.
    public var alternateHighDb: Double?

    init(_ c: sdrkit_obw_reading_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        obwKhz = c.obw_khz
        obwLow = c.obw_low
        obwHigh = c.obw_high
        bw3Khz = c.bw3_khz
        bw6Khz = c.bw6_khz
        bw20Khz = c.bw20_khz
        bw26Khz = c.bw26_khz
        centreOffsetHz = c.centre_offset_hz
        powerDb = c.power_db
        snrDb = c.snr_db
        adjacentLowDb = c.has_adjacent_low_db ? c.adjacent_low_db : nil
        adjacentHighDb = c.has_adjacent_high_db ? c.adjacent_high_db : nil
        alternateLowDb = c.has_alternate_low_db ? c.alternate_low_db : nil
        alternateHighDb = c.has_alternate_high_db ? c.alternate_high_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ObwReadingEvent, SDRKitCode> {
        var c = sdrkit_obw_reading_event()
        let code = sdrkit_obw_reading_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ObwReadingEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A transmission ended, or a stuck key was found.
public struct PttLogPttTransmissionEvent {
    public enum Kind: Int32 {
        case transmission = 0
        case kerchunk = 1
        case stuckKey = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the transmission, from 1.
    public var number: Int64
    /// The kind.
    public var kind: Kind?
    /// The start, signal time.
    public var startS: Double
    /// The length (so far, for a stuck key).
    public var lengthS: Double
    /// The mean level over the floor of its keyed blocks.
    public var overFloorDb: Double
    /// The time from the end of the transmission before; no value for the first.
    public var gapS: Double?
    /// The mean level of its keyed blocks, relative to full scale.
    public var levelDb: Double

    init(_ c: sdrkit_ptt_log_ptt_transmission_event) {
        time = c.time
        freqHz = c.freq_hz
        number = c.number
        kind = Kind(rawValue: c.kind)
        startS = c.start_s
        lengthS = c.length_s
        overFloorDb = c.over_floor_db
        gapS = c.has_gap_s ? c.gap_s : nil
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PttLogPttTransmissionEvent, SDRKitCode> {
        var c = sdrkit_ptt_log_ptt_transmission_event()
        let code = sdrkit_ptt_log_ptt_transmission_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PttLogPttTransmissionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A pattern was found or went.
public struct NoiseLocatorPatternEvent {
    public enum Kind: Int32 {
        case found = 0
        case gone = 1
    }
    public enum Class: Int32 {
        case twiceMains = 0
        case mains = 1
        case steady = 2
        case none = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the report.
    public var timeS: Double
    /// Found or gone.
    public var kind: Kind?
    /// The repetition rate.
    public var repetitionHz: Double
    /// The class of the rate.
    public var `class`: Class?
    /// The impulses per second of the window.
    public var impulseRate: Double
    /// The median impulse level over the noise of the analysis band.
    public var levelDb: Double
    /// The share of the impulses on the rate.
    public var strengthPct: Double

    init(_ c: sdrkit_noise_locator_pattern_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        repetitionHz = c.repetition_hz
        `class` = Class(rawValue: c.class_)
        impulseRate = c.impulse_rate
        levelDb = c.level_db
        strengthPct = c.strength_pct
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<NoiseLocatorPatternEvent, SDRKitCode> {
        var c = sdrkit_noise_locator_pattern_event()
        let code = sdrkit_noise_locator_pattern_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(NoiseLocatorPatternEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the `freq_accuracy` log was written.
public struct FreqAccuracyEvent {
    public enum Kind: Int32 {
        case frequency = 0
        case step = 1
        case lost = 2
        case returned = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The offset of the reading (for a loss, the last reading).
    public var offsetHz: Double
    /// The size of a step.
    public var stepHz: Double?
    /// The carrier to noise density of the reading.
    public var cn0Dbhz: Double?

    init(_ c: sdrkit_freq_accuracy_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        offsetHz = c.offset_hz
        stepHz = c.has_step_hz ? c.step_hz : nil
        cn0Dbhz = c.has_cn0_dbhz ? c.cn0_dbhz : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FreqAccuracyEvent, SDRKitCode> {
        var c = sdrkit_freq_accuracy_event()
        let code = sdrkit_freq_accuracy_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FreqAccuracyEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The SINAD crossed the sensitivity mark.
public struct SinadCrossingEvent {
    public enum Crossing: Int32 {
        case none = 0
        case fell = 1
        case rose = 2
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time.
    public var timeS: Double
    /// SINAD.
    public var sinadDb: Double
    /// The change against the mark.
    public var crossing: Crossing?
    /// The test tone.
    public var toneHz: Double
    /// The peak deviation.
    public var deviationKhz: Double
    /// THD.
    public var thdPct: Double

    init(_ c: sdrkit_sinad_crossing_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        sinadDb = c.sinad_db
        crossing = Crossing(rawValue: c.crossing)
        toneHz = c.tone_hz
        deviationKhz = c.deviation_khz
        thdPct = c.thd_pct
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SinadCrossingEvent, SDRKitCode> {
        var c = sdrkit_sinad_crossing_event()
        let code = sdrkit_sinad_crossing_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SinadCrossingEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A transmission was analysed.
public struct TxStabilityTransmissionEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The key-up, signal time.
    public var timeS: Double
    /// The length of the transmission.
    public var lengthS: Double
    /// The frequency readings in it.
    public var readings: Int64
    /// The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency.
    public var startHz: Double
    /// The mean frequency of the last 0.4 s.
    public var finalHz: Double
    /// Start minus final.
    public var keyupStepHz: Double
    /// The time of the last reading outside the settling band, plus 0.05 s.
    public var settleS: Double
    /// The larger of 10 Hz and a tenth of the key-up step.
    public var settleBandHz: Double
    /// The least-squares slope of the readings of the second half.
    public var driftHzS: Double?
    /// The mean of all readings.
    public var meanHz: Double
    /// The mean over the tuned frequency; no value without it.
    public var meanPpm: Double?
    /// The RMS residual about the drift line.
    public var scatterHz: Double?

    init(_ c: sdrkit_tx_stability_transmission_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        lengthS = c.length_s
        readings = c.readings
        startHz = c.start_hz
        finalHz = c.final_hz
        keyupStepHz = c.keyup_step_hz
        settleS = c.settle_s
        settleBandHz = c.settle_band_hz
        driftHzS = c.has_drift_hz_s ? c.drift_hz_s : nil
        meanHz = c.mean_hz
        meanPpm = c.has_mean_ppm ? c.mean_ppm : nil
        scatterHz = c.has_scatter_hz ? c.scatter_hz : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TxStabilityTransmissionEvent, SDRKitCode> {
        var c = sdrkit_tx_stability_transmission_event()
        let code = sdrkit_tx_stability_transmission_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TxStabilityTransmissionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A burst was analysed.
public struct AfskTunerBurstEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The start of the burst, signal time.
    public var timeS: Double
    /// The length of the burst.
    public var lengthS: Double
    /// The mark tone.
    public var markHz: Double
    /// The space tone.
    public var spaceHz: Double
    /// The space-band power over the mark-band power.
    public var twistDb: Double
    /// The 99.5th percentile of the band-limited audio.
    public var peakDeviationKhz: Double
    /// The time since the end of the burst before; no value for the first.
    public var gapS: Double?

    init(_ c: sdrkit_afsk_tuner_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        lengthS = c.length_s
        markHz = c.mark_hz
        spaceHz = c.space_hz
        twistDb = c.twist_db
        peakDeviationKhz = c.peak_deviation_khz
        gapS = c.has_gap_s ? c.gap_s : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AfskTunerBurstEvent, SDRKitCode> {
        var c = sdrkit_afsk_tuner_burst_event()
        let code = sdrkit_afsk_tuner_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AfskTunerBurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A stream fault of `glitch`.
public struct GlitchEvent {
    public enum Kind: Int32 {
        case droppedSamples = 0
        case repeatedSamples = 1
        case zeroRun = 2
        case repeatedBlock = 3
        case levelStep = 4
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the fault.
    public var timeS: Double
    /// The kind.
    public var kind: Kind?
    /// The samples missing or repeated.
    public var samples: Int64?
    /// The phase step of the tone.
    public var phaseStepRad: Double?
    /// The size of a level step.
    public var stepDb: Double?
    /// The level before a step.
    public var fromDb: Double?
    /// The level after a step.
    public var toDb: Double?

    init(_ c: sdrkit_glitch_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        samples = c.has_samples ? c.samples : nil
        phaseStepRad = c.has_phase_step_rad ? c.phase_step_rad : nil
        stepDb = c.has_step_db ? c.step_db : nil
        fromDb = c.has_from_db ? c.from_db : nil
        toDb = c.has_to_db ? c.to_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GlitchEvent, SDRKitCode> {
        var c = sdrkit_glitch_event()
        let code = sdrkit_glitch_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GlitchEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A verdict of a product that held for two evaluations.
public struct IntermodProductEvent {
    public enum Kind: Int32 {
        case n2aB = 0
        case n2bA = 1
        case bA = 2
    }
    public enum Verdict: Int32 {
        case follows = 0
        case independent = 1
        case unclear = 2
        case cannotTell = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time.
    public var timeS: Double
    /// The frequency of the peak.
    public var frequency: Double
    /// The kind of product.
    public var kind: Kind?
    /// The lower parent.
    public var parentA: Double
    /// The upper parent.
    public var parentB: Double
    /// The mean level under the weaker parent.
    public var belowDb: Double
    /// The verdict.
    public var verdict: Verdict?
    /// The slope.
    public var slope: Double?
    /// The correlation.
    public var correlation: Double?
    /// The snapshots of the fit.
    public var points: Int64

    init(_ c: sdrkit_intermod_product_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        frequency = c.frequency
        kind = Kind(rawValue: c.kind)
        parentA = c.parent_a
        parentB = c.parent_b
        belowDb = c.below_db
        verdict = Verdict(rawValue: c.verdict)
        slope = c.has_slope ? c.slope : nil
        correlation = c.has_correlation ? c.correlation : nil
        points = c.points
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IntermodProductEvent, SDRKitCode> {
        var c = sdrkit_intermod_product_event()
        let code = sdrkit_intermod_product_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IntermodProductEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A change of class of the path.
public struct BeaconMonitorChangeEvent {
    public enum FromClass: Int32 {
        case none = 0
        case steady = 1
        case fading = 2
        case auroraLike = 3
    }
    public enum ToClass: Int32 {
        case none = 0
        case steady = 1
        case fading = 2
        case auroraLike = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time.
    public var timeS: Double
    /// The class before.
    public var fromClass: FromClass?
    /// The class after.
    public var toClass: ToClass?
    /// The last level.
    public var levelDb: Double?
    /// The median spread of the decision.
    public var spreadHz: Double?
    /// The swing of the decision.
    public var swingDb: Double?

    init(_ c: sdrkit_beacon_monitor_change_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        fromClass = FromClass(rawValue: c.from_class)
        toClass = ToClass(rawValue: c.to_class)
        levelDb = c.has_level_db ? c.level_db : nil
        spreadHz = c.has_spread_hz ? c.spread_hz : nil
        swingDb = c.has_swing_db ? c.swing_db : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BeaconMonitorChangeEvent, SDRKitCode> {
        var c = sdrkit_beacon_monitor_change_event()
        let code = sdrkit_beacon_monitor_change_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BeaconMonitorChangeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum MeterTables {
    /// The changed rows of the table "log" of the stack "fm_monitor", newest first, at most `capacity` rows.
    /// The column `identity` names a row.
    public static func fmMonitorLog(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[FmMonitorLogRow], SDRKitCode> {
        var rows = [sdrkit_fm_monitor_log_row](repeating: sdrkit_fm_monitor_log_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fm_monitor_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FmMonitorLogRow.init))
    }
    /// The changed rows of the table "log" of the stack "am_monitor", newest first, at most `capacity` rows.
    /// The column `identity` names a row.
    public static func amMonitorLog(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AmMonitorLogRow], SDRKitCode> {
        var rows = [sdrkit_am_monitor_log_row](repeating: sdrkit_am_monitor_log_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_am_monitor_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AmMonitorLogRow.init))
    }
    /// The changed rows of the table "channels" of the stack "occupancy", newest first, at most `capacity` rows.
    public static func occupancyChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[OccupancyChannelRow], SDRKitCode> {
        var rows = [sdrkit_occupancy_channel_row](repeating: sdrkit_occupancy_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_occupancy_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OccupancyChannelRow.init))
    }
    /// The changed rows of the table "activity" of the stack "occupancy", newest first, at most `capacity` rows.
    public static func occupancyActivity(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[OccupancyActivityRow], SDRKitCode> {
        var rows = [sdrkit_occupancy_activity_row](repeating: sdrkit_occupancy_activity_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_occupancy_activity_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OccupancyActivityRow.init))
    }
    /// The changed rows of the table "sub_bands" of the stack "noise_floor", newest first, at most `capacity` rows.
    public static func noiseFloorSubBands(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[NoiseFloorSubBandRow], SDRKitCode> {
        var rows = [sdrkit_noise_floor_sub_band_row](repeating: sdrkit_noise_floor_sub_band_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noise_floor_sub_bands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NoiseFloorSubBandRow.init))
    }
    /// The changed rows of the table "noise_events" of the stack "noise_floor", newest first, at most `capacity` rows.
    public static func noiseFloorNoiseEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[NoiseFloorEventRow], SDRKitCode> {
        var rows = [sdrkit_noise_floor_event_row](repeating: sdrkit_noise_floor_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noise_floor_noise_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NoiseFloorEventRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "burst_finder", newest first, at most `capacity` rows.
    public static func burstFinderBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[BurstFinderBurstRow], SDRKitCode> {
        var rows = [sdrkit_burst_finder_burst_row](repeating: sdrkit_burst_finder_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_burst_finder_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BurstFinderBurstRow.init))
    }
    /// The changed rows of the table "repeating" of the stack "burst_finder", newest first, at most `capacity` rows.
    public static func burstFinderRepeating(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[BurstFinderRepeatingRow], SDRKitCode> {
        var rows = [sdrkit_burst_finder_repeating_row](repeating: sdrkit_burst_finder_repeating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_burst_finder_repeating_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BurstFinderRepeatingRow.init))
    }
    /// The changed rows of the table "hoppers" of the stack "hopper", newest first, at most `capacity` rows.
    public static func hopperHoppers(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[HopperRow], SDRKitCode> {
        var rows = [sdrkit_hopper_row](repeating: sdrkit_hopper_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hopper_hoppers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HopperRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "birdie", newest first, at most `capacity` rows.
    /// Default order: the column `level_db`, highest first.
    public static func birdieCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[BirdieCarrierRow], SDRKitCode> {
        var rows = [sdrkit_birdie_carrier_row](repeating: sdrkit_birdie_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_birdie_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BirdieCarrierRow.init))
    }
    /// The changed rows of the table "carrier_events" of the stack "birdie", newest first, at most `capacity` rows.
    public static func birdieCarrierEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[BirdieEventRow], SDRKitCode> {
        var rows = [sdrkit_birdie_event_row](repeating: sdrkit_birdie_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_birdie_carrier_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BirdieEventRow.init))
    }
    /// The changed rows of the table "bandwidths" of the stack "obw", newest first, at most `capacity` rows.
    public static func obwBandwidths(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[ObwBandwidthRow], SDRKitCode> {
        var rows = [sdrkit_obw_bandwidth_row](repeating: sdrkit_obw_bandwidth_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_obw_bandwidths_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ObwBandwidthRow.init))
    }
    /// The changed rows of the table "transmissions" of the stack "ptt_log", newest first, at most `capacity` rows.
    public static func pttLogTransmissions(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[PttLogTransmissionRow], SDRKitCode> {
        var rows = [sdrkit_ptt_log_transmission_row](repeating: sdrkit_ptt_log_transmission_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ptt_log_transmissions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PttLogTransmissionRow.init))
    }
    /// The changed rows of the table "patterns" of the stack "noise_locator", newest first, at most `capacity` rows.
    public static func noiseLocatorPatterns(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[NoiseLocatorPatternRow], SDRKitCode> {
        var rows = [sdrkit_noise_locator_pattern_row](repeating: sdrkit_noise_locator_pattern_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noise_locator_patterns_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NoiseLocatorPatternRow.init))
    }
    /// The changed rows of the table "allan" of the stack "freq_accuracy", newest first, at most `capacity` rows.
    public static func freqAccuracyAllan(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[FreqAccuracyAllanRow], SDRKitCode> {
        var rows = [sdrkit_freq_accuracy_allan_row](repeating: sdrkit_freq_accuracy_allan_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freq_accuracy_allan_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FreqAccuracyAllanRow.init))
    }
    /// The changed rows of the table "events" of the stack "freq_accuracy", newest first, at most `capacity` rows.
    public static func freqAccuracyEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[FreqAccuracyEventRow], SDRKitCode> {
        var rows = [sdrkit_freq_accuracy_event_row](repeating: sdrkit_freq_accuracy_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freq_accuracy_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FreqAccuracyEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "sinad", newest first, at most `capacity` rows.
    public static func sinadEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[SinadEventRow], SDRKitCode> {
        var rows = [sdrkit_sinad_event_row](repeating: sdrkit_sinad_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sinad_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SinadEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "nfm_meter", newest first, at most `capacity` rows.
    public static func nfmMeterEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[NfmMeterEventRow], SDRKitCode> {
        var rows = [sdrkit_nfm_meter_event_row](repeating: sdrkit_nfm_meter_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nfm_meter_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NfmMeterEventRow.init))
    }
    /// The changed rows of the table "transmissions" of the stack "tx_stability", newest first, at most `capacity` rows.
    public static func txStabilityTransmissions(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[TxStabilityTransmissionRow], SDRKitCode> {
        var rows = [sdrkit_tx_stability_transmission_row](repeating: sdrkit_tx_stability_transmission_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tx_stability_transmissions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(TxStabilityTransmissionRow.init))
    }
    /// The changed rows of the table "events" of the stack "ssb_test", newest first, at most `capacity` rows.
    public static func ssbTestEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[SsbTestEventRow], SDRKitCode> {
        var rows = [sdrkit_ssb_test_event_row](repeating: sdrkit_ssb_test_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ssb_test_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SsbTestEventRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "afsk_tuner", newest first, at most `capacity` rows.
    public static func afskTunerBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[AfskTunerBurstRow], SDRKitCode> {
        var rows = [sdrkit_afsk_tuner_burst_row](repeating: sdrkit_afsk_tuner_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_afsk_tuner_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AfskTunerBurstRow.init))
    }
    /// The changed rows of the table "spurs" of the stack "phase_noise", newest first, at most `capacity` rows.
    /// Default order: the column `level_dbc`, highest first.
    public static func phaseNoiseSpurs(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[PhaseNoiseSpurRow], SDRKitCode> {
        var rows = [sdrkit_phase_noise_spur_row](repeating: sdrkit_phase_noise_spur_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_phase_noise_spurs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PhaseNoiseSpurRow.init))
    }
    /// The changed rows of the table "events" of the stack "phase_noise", newest first, at most `capacity` rows.
    public static func phaseNoiseEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[PhaseNoiseEventRow], SDRKitCode> {
        var rows = [sdrkit_phase_noise_event_row](repeating: sdrkit_phase_noise_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_phase_noise_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PhaseNoiseEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "cw_keying", newest first, at most `capacity` rows.
    public static func cwKeyingEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[CwKeyingEventRow], SDRKitCode> {
        var rows = [sdrkit_cw_keying_event_row](repeating: sdrkit_cw_keying_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cw_keying_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CwKeyingEventRow.init))
    }
    /// The changed rows of the table "level_freqs" of the stack "fsk_analyser", newest first, at most `capacity` rows.
    public static func fskAnalyserLevelFreqs(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[FskAnalyserLevelRow], SDRKitCode> {
        var rows = [sdrkit_fsk_analyser_level_row](repeating: sdrkit_fsk_analyser_level_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsk_analyser_level_freqs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FskAnalyserLevelRow.init))
    }
    /// The changed rows of the table "events" of the stack "fsk_analyser", newest first, at most `capacity` rows.
    public static func fskAnalyserEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[FskAnalyserEventRow], SDRKitCode> {
        var rows = [sdrkit_fsk_analyser_event_row](repeating: sdrkit_fsk_analyser_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fsk_analyser_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FskAnalyserEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "mer", newest first, at most `capacity` rows.
    public static func merEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[MerEventRow], SDRKitCode> {
        var rows = [sdrkit_mer_event_row](repeating: sdrkit_mer_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mer_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MerEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "papr", newest first, at most `capacity` rows.
    public static func paprEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[PaprEventRow], SDRKitCode> {
        var rows = [sdrkit_papr_event_row](repeating: sdrkit_papr_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_papr_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PaprEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "input_health", newest first, at most `capacity` rows.
    public static func inputHealthEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[InputHealthEventRow], SDRKitCode> {
        var rows = [sdrkit_input_health_event_row](repeating: sdrkit_input_health_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_input_health_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(InputHealthEventRow.init))
    }
    /// The changed rows of the table "noise_figure" of the stack "noise_figure", newest first, at most `capacity` rows.
    public static func noiseFigure(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[NoiseFigureRow], SDRKitCode> {
        var rows = [sdrkit_noise_figure_row](repeating: sdrkit_noise_figure_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noise_figure_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NoiseFigureRow.init))
    }
    /// The changed rows of the table "events" of the stack "noise_figure", newest first, at most `capacity` rows.
    public static func noiseFigureEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[NoiseFigureEventRow], SDRKitCode> {
        var rows = [sdrkit_noise_figure_event_row](repeating: sdrkit_noise_figure_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_noise_figure_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NoiseFigureEventRow.init))
    }
    /// The changed rows of the table "regions" of the stack "kurtosis", newest first, at most `capacity` rows.
    public static func kurtosisRegions(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[KurtosisRegionRow], SDRKitCode> {
        var rows = [sdrkit_kurtosis_region_row](repeating: sdrkit_kurtosis_region_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_kurtosis_regions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(KurtosisRegionRow.init))
    }
    /// The changed rows of the table "events" of the stack "kurtosis", newest first, at most `capacity` rows.
    public static func kurtosisEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[KurtosisEventRow], SDRKitCode> {
        var rows = [sdrkit_kurtosis_event_row](repeating: sdrkit_kurtosis_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_kurtosis_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(KurtosisEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "glitch", newest first, at most `capacity` rows.
    public static func glitchEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[GlitchEventRow], SDRKitCode> {
        var rows = [sdrkit_glitch_event_row](repeating: sdrkit_glitch_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_glitch_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GlitchEventRow.init))
    }
    /// The changed rows of the table "inputs" of the stack "intermod", newest first, at most `capacity` rows.
    public static func intermodInputs(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[IntermodInputRow], SDRKitCode> {
        var rows = [sdrkit_intermod_input_row](repeating: sdrkit_intermod_input_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_intermod_inputs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IntermodInputRow.init))
    }
    /// The changed rows of the table "products" of the stack "intermod", newest first, at most `capacity` rows.
    public static func intermodProducts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[IntermodProductRow], SDRKitCode> {
        var rows = [sdrkit_intermod_product_row](repeating: sdrkit_intermod_product_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_intermod_products_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IntermodProductRow.init))
    }
    /// The changed rows of the table "events" of the stack "intermod", newest first, at most `capacity` rows.
    public static func intermodEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[IntermodEventRow], SDRKitCode> {
        var rows = [sdrkit_intermod_event_row](repeating: sdrkit_intermod_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_intermod_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IntermodEventRow.init))
    }
    /// The changed rows of the table "findings" of the stack "spur", newest first, at most `capacity` rows.
    public static func spurFindings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[SpurFindingRow], SDRKitCode> {
        var rows = [sdrkit_spur_finding_row](repeating: sdrkit_spur_finding_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_spur_findings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SpurFindingRow.init))
    }
    /// The changed rows of the table "events" of the stack "spur", newest first, at most `capacity` rows.
    public static func spurEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[SpurEventRow], SDRKitCode> {
        var rows = [sdrkit_spur_event_row](repeating: sdrkit_spur_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_spur_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SpurEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "fading", newest first, at most `capacity` rows.
    public static func fadingEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[FadingEventRow], SDRKitCode> {
        var rows = [sdrkit_fading_event_row](repeating: sdrkit_fading_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fading_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FadingEventRow.init))
    }
    /// The changed rows of the table "events" of the stack "beacon_monitor", newest first, at most `capacity` rows.
    public static func beaconMonitorEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[BeaconMonitorEventRow], SDRKitCode> {
        var rows = [sdrkit_beacon_monitor_event_row](repeating: sdrkit_beacon_monitor_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_beacon_monitor_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BeaconMonitorEventRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum MeterCommands {
    /// Clear. Forgets everything decoded so far.
    public static func fmMonitorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fm_monitor_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func amMonitorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_am_monitor_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func occupancyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_occupancy_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Learns the floor again and drops the channel states; the tables stay.
    public static func occupancyReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_occupancy_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func noiseFloorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noise_floor_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the histories of the sub-bands and their rise states; the tables stay.
    public static func noiseFloorReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noise_floor_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func burstFinderClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_burst_finder_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the bursts in progress; the tables stay.
    public static func burstFinderReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_burst_finder_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func hopperClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hopper_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the dwells and the readings; the table stays.
    public static func hopperReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hopper_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func birdieClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_birdie_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the tracks and the list; the log stays.
    public static func birdieReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_birdie_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func obwClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_obw_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Starts the average again; the table stays.
    public static func obwReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_obw_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func pttLogClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ptt_log_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Learns the floor again and drops a transmission in progress; the table stays.
    public static func pttLogReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ptt_log_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func noiseLocatorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noise_locator_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Learns the noise again and drops the impulses and the pattern; the table stays.
    public static func noiseLocatorReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noise_locator_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func freqAccuracyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_freq_accuracy_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func freqAccuracyReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_freq_accuracy_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func sinadClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sinad_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func sinadReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sinad_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func nfmMeterClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nfm_meter_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func nfmMeterReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nfm_meter_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func txStabilityClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tx_stability_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func txStabilityReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tx_stability_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ssbTestClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ssb_test_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func ssbTestReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ssb_test_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func afskTunerClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_afsk_tuner_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func afskTunerReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_afsk_tuner_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func phaseNoiseClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_phase_noise_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func phaseNoiseReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_phase_noise_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func cwKeyingClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cw_keying_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func cwKeyingReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cw_keying_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fskAnalyserClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk_analyser_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func fskAnalyserReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fsk_analyser_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func merClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mer_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func merReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mer_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func paprClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_papr_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func paprReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_papr_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func inputHealthClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_input_health_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func inputHealthReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_input_health_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func noiseFigureClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noise_figure_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func noiseFigureReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_noise_figure_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func kurtosisClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_kurtosis_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func kurtosisReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_kurtosis_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func glitchClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_glitch_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func glitchReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_glitch_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func intermodClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_intermod_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func intermodReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_intermod_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func spurClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_spur_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func spurReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_spur_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func fadingClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fading_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func fadingReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fading_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func beaconMonitorClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_beacon_monitor_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, averages and floors, as after a retune; the tables stay.
    public static func beaconMonitorReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_beacon_monitor_reset(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum MeterSettings {
    /// Over-deviation threshold.
    public static func fmMonitorOverDeviationKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "over_deviation_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorOverDeviationKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "over_deviation_khz", value))
    }
    /// The options of the setting "deviation_limit" of the stack "fm_monitor".
    public enum FmMonitorDeviationLimit: Int32 {
        case n75Khz = 0
        case n50Khz = 1
    }
    /// Peak deviation limit.
    public static func fmMonitorDeviationLimit(_ handle: SDRKIT_HANDLE) -> Result<FmMonitorDeviationLimit, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deviation_limit", &v)
        return code == SDRKIT_OK ? .success(FmMonitorDeviationLimit(rawValue: v) ?? FmMonitorDeviationLimit(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorDeviationLimit(_ handle: SDRKIT_HANDLE, _ value: FmMonitorDeviationLimit) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deviation_limit", value.rawValue))
    }
    /// Pilot range margin.
    public static func fmMonitorPilotMarginKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pilot_margin_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorPilotMarginKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pilot_margin_khz", value))
    }
    /// Over-modulation: share of a second at zero carrier.
    public static func amMonitorOverModSharePct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "over_mod_share_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorOverModSharePct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "over_mod_share_pct", value))
    }
    /// Carrier lost below the best level by.
    public static func amMonitorCarrierLossDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "carrier_loss_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorCarrierLossDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "carrier_loss_db", value))
    }
    /// Channel width.
    public static func occupancyGridKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "grid_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOccupancyGridKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "grid_khz", value))
    }
    /// Busy above (over the floor).
    public static func occupancyOnDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "on_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOccupancyOnDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "on_db", value))
    }
    /// Idle below (over the floor).
    public static func occupancyOffDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "off_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOccupancyOffDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "off_db", value))
    }
    /// Floor quantile of the channels.
    public static func occupancyFloorShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "floor_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOccupancyFloorShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "floor_share", value))
    }
    /// Shortest transmission listed.
    public static func occupancyMinLogS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_log_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setOccupancyMinLogS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_log_s", value))
    }
    /// Sub-bands.
    public static func noiseFloorSubBands(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sub_bands", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorSubBands(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sub_bands", value))
    }
    /// Rise over the baseline.
    public static func noiseFloorRiseDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "rise_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorRiseDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "rise_db", value))
    }
    /// Rise must hold for.
    public static func noiseFloorHoldS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorHoldS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_s", value))
    }
    /// Baseline window.
    public static func noiseFloorBaselineMin(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "baseline_min", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorBaselineMin(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "baseline_min", value))
    }
    /// Sub-bands that make a whole-band change.
    public static func noiseFloorWholeBandShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "whole_band_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorWholeBandShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "whole_band_share", value))
    }
    /// Hot bin over the frame median.
    public static func burstFinderHotDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hot_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBurstFinderHotDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hot_db", value))
    }
    /// Longest burst.
    public static func burstFinderMaxBurstS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_burst_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBurstFinderMaxBurstS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_burst_s", value))
    }
    /// Interval agreement.
    public static func burstFinderRepeatTolerancePct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "repeat_tolerance_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBurstFinderRepeatTolerancePct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "repeat_tolerance_pct", value))
    }
    /// Frame length.
    public static func burstFinderFftSize(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "fft_size", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBurstFinderFftSize(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "fft_size", value))
    }
    /// Strongest bin over the frame median.
    public static func hopperPresentDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "present_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperPresentDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "present_db", value))
    }
    /// Dwells before a report.
    public static func hopperMinDwells(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "min_dwells", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperMinDwells(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "min_dwells", value))
    }
    /// Fewest channels.
    public static func hopperMinChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "min_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperMinChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "min_channels", value))
    }
    /// Peak over the noise.
    public static func birdieThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Present in at least.
    public static func birdiePresencePct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "presence_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdiePresencePct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "presence_pct", value))
    }
    /// Presence window.
    public static func birdieWindowS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "window_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieWindowS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "window_s", value))
    }
    /// Bare carrier when the narrowness is at least.
    public static func birdieBareShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "bare_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieBareShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "bare_share", value))
    }
    /// Occupied power share.
    public static func obwPowerPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "power_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwPowerPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "power_pct", value))
    }
    /// The x of the x dB bandwidth in the headline.
    public static func obwXDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "x_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwXDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "x_db", value))
    }
    /// Averaging time.
    public static func obwAverageS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "average_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwAverageS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "average_s", value))
    }
    /// Adjacent band centre, in bandwidths.
    public static func obwAdjacentOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "adjacent_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwAdjacentOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "adjacent_offset", value))
    }
    /// Channel width.
    public static func pttLogChannelKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogChannelKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_khz", value))
    }
    /// Keyed above (over the floor).
    public static func pttLogOnDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "on_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogOnDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "on_db", value))
    }
    /// Released below (over the floor).
    public static func pttLogOffDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "off_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogOffDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "off_db", value))
    }
    /// Release after.
    public static func pttLogHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Kerchunk shorter than.
    public static func pttLogKerchunkS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "kerchunk_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogKerchunkS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "kerchunk_s", value))
    }
    /// Stuck key after.
    public static func pttLogStuckS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "stuck_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogStuckS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "stuck_s", value))
    }
    /// Mains frequencies.
    public static func noiseLocatorMainsHz(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mains_hz", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorMainsHz(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mains_hz", $0) })
    }
    /// Impulse threshold over the noise.
    public static func noiseLocatorSigmas(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "sigmas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorSigmas(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "sigmas", value))
    }
    /// Impulse window.
    public static func noiseLocatorWindowS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "window_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorWindowS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "window_s", value))
    }
    /// Analysis band.
    public static func noiseLocatorAnalysisKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "analysis_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorAnalysisKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "analysis_khz", value))
    }
    /// Search half width.
    public static func freqAccuracySearchHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracySearchHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_hz", value))
    }
    /// Carrier over the noise per bin.
    public static func freqAccuracyPresentDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "present_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyPresentDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "present_db", value))
    }
    /// Step threshold in scatter widths.
    public static func freqAccuracyStepSigmas(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "step_sigmas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyStepSigmas(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "step_sigmas", value))
    }
    /// Smallest step.
    public static func freqAccuracyStepMinHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "step_min_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyStepMinHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "step_min_hz", value))
    }
    /// Frequency row every.
    public static func freqAccuracyRowEveryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "row_every_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyRowEveryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "row_every_s", value))
    }
    /// Audio band, low edge.
    public static func sinadBandLoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "band_lo_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadBandLoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "band_lo_hz", value))
    }
    /// Audio band, high edge.
    public static func sinadBandHiHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "band_hi_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadBandHiHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "band_hi_hz", value))
    }
    /// Sensitivity mark.
    public static func sinadMarkDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "sinad_mark_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadMarkDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "sinad_mark_db", value))
    }
    /// Tone over the band median.
    public static func sinadToneMinDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tone_min_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadToneMinDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tone_min_db", value))
    }
    /// Narrow channel deviation limit.
    public static func nfmMeterLimitNarrowHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "limit_narrow_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterLimitNarrowHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "limit_narrow_hz", value))
    }
    /// Wide channel deviation limit.
    public static func nfmMeterLimitWideHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "limit_wide_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterLimitWideHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "limit_wide_hz", value))
    }
    /// Voice peak percentile.
    public static func nfmMeterPeakPercentile(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "peak_percentile", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterPeakPercentile(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "peak_percentile", value))
    }
    /// CTCSS over its neighbourhood.
    public static func nfmMeterCtcssMinDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "ctcss_min_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterCtcssMinDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "ctcss_min_db", value))
    }
    /// Key-up over the floor.
    public static func txStabilityOnDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "on_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityOnDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "on_db", value))
    }
    /// Key-down under.
    public static func txStabilityOffDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "off_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityOffDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "off_db", value))
    }
    /// Hang before key-down.
    public static func txStabilityHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Shortest transmission analysed.
    public static func txStabilityMinTxS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_tx_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityMinTxS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_tx_s", value))
    }
    /// Least reading length.
    public static func txStabilityReadingS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "reading_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityReadingS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "reading_s", value))
    }
    /// Tone search, low edge.
    public static func ssbTestToneMinHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tone_min_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestToneMinHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tone_min_hz", value))
    }
    /// Tone search, high edge.
    public static func ssbTestToneMaxHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tone_max_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestToneMaxHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tone_max_hz", value))
    }
    /// Second tone within.
    public static func ssbTestTwoToneDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "two_tone_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestTwoToneDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "two_tone_db", value))
    }
    /// PEP over one tone.
    public static func ssbTestPepOffsetDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "pep_offset_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestPepOffsetDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "pep_offset_db", value))
    }
    /// Mark tone.
    public static func afskTunerMarkHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mark_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerMarkHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mark_hz", value))
    }
    /// Space tone.
    public static func afskTunerSpaceHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "space_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerSpaceHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "space_hz", value))
    }
    /// Burst on over the floor.
    public static func afskTunerOnRatio(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "on_ratio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerOnRatio(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "on_ratio", value))
    }
    /// Burst off over the floor.
    public static func afskTunerOffRatio(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "off_ratio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerOffRatio(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "off_ratio", value))
    }
    /// Shortest burst analysed.
    public static func afskTunerMinBurstS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_burst_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerMinBurstS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_burst_s", value))
    }
    /// Blocks averaged.
    public static func phaseNoiseAverageBlocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "average_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseAverageBlocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "average_blocks", value))
    }
    /// Spur over its neighbourhood.
    public static func phaseNoiseSpurRatioDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "spur_ratio_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseSpurRatioDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "spur_ratio_db", value))
    }
    /// Jitter band, low edge.
    public static func phaseNoiseJitterLoHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jitter_lo_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseJitterLoHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jitter_lo_hz", value))
    }
    /// Jitter band, high edge.
    public static func phaseNoiseJitterHiHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jitter_hi_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseJitterHiHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jitter_hi_hz", value))
    }
    /// Carrier search half width.
    public static func cwKeyingSearchHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "search_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingSearchHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "search_hz", value))
    }
    /// Key-down above (of the peak).
    public static func cwKeyingOnShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "on_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingOnShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "on_share", value))
    }
    /// Key-up under (of the peak).
    public static func cwKeyingOffShare(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "off_share", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingOffShare(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "off_share", value))
    }
    /// Elements kept.
    public static func cwKeyingElements(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "elements", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingElements(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "elements", value))
    }
    /// Shortest mark for the chirp.
    public static func cwKeyingMinChirpMarkS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_chirp_mark_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingMinChirpMarkS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_chirp_mark_s", value))
    }
    /// Block length.
    public static func fskAnalyserBlockS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "block_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserBlockS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "block_s", value))
    }
    /// Lowest symbol rate.
    public static func fskAnalyserMinBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserMinBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_baud", value))
    }
    /// Lowest eye SNR shown.
    public static func fskAnalyserMinEyeDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_eye_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserMinEyeDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_eye_db", value))
    }
    /// Block length.
    public static func merBlockS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "block_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerBlockS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "block_s", value))
    }
    /// Lowest symbol rate.
    public static func merMinBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerMinBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_baud", value))
    }
    /// Lowest MER shown.
    public static func merMinMerDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_mer_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerMinMerDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_mer_db", value))
    }
    /// Roll-offs tried.
    public static func merRolloffs(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "rolloffs", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMerRolloffs(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "rolloffs", $0) })
    }
    /// Samples in the statistics.
    public static func paprKeepSamples(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "keep_samples", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprKeepSamples(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "keep_samples", value))
    }
    /// Evaluate every (blocks of 16384).
    public static func paprEveryBlocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "every_blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprEveryBlocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "every_blocks", value))
    }
    /// Reading every.
    public static func inputHealthBlockS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "block_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthBlockS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "block_s", value))
    }
    /// Clip at (of full scale).
    public static func inputHealthClipLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clip_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthClipLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clip_level", value))
    }
    /// Overload above.
    public static func inputHealthOverloadPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "overload_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthOverloadPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "overload_ppm", value))
    }
    /// High above.
    public static func inputHealthHighDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "high_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthHighDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "high_dbfs", value))
    }
    /// Low under.
    public static func inputHealthLowDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "low_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthLowDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "low_dbfs", value))
    }
    /// Source ENR values.
    public static func noiseFigureEnrDb(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "enr_db", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureEnrDb(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "enr_db", $0) })
    }
    /// A window joins a plateau within.
    public static func noiseFigureAgreeDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "agree_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureAgreeDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "agree_db", value))
    }
    /// Smallest step.
    public static func noiseFigureStepDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "step_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureStepDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "step_db", value))
    }
    /// Steps averaged.
    public static func noiseFigureStepsKept(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "steps_kept", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureStepsKept(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "steps_kept", value))
    }
    /// Frame length.
    public static func kurtosisFft(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "fft", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setKurtosisFft(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "fft", value))
    }
    /// Frames per estimate.
    public static func kurtosisFrames(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setKurtosisFrames(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "frames", value))
    }
    /// Estimates averaged.
    public static func kurtosisBlocks(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "blocks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setKurtosisBlocks(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "blocks", value))
    }
    /// Flag threshold.
    public static func kurtosisSigmas(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "sigmas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setKurtosisSigmas(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "sigmas", value))
    }
    /// Shortest zero run.
    public static func glitchZeroRun(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "zero_run", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchZeroRun(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "zero_run", value))
    }
    /// Repeated block length.
    public static func glitchBlock(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "block", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchBlock(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "block", value))
    }
    /// Level step.
    public static func glitchStepDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "step_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchStepDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "step_db", value))
    }
    /// Tone over the noise per sample.
    public static func glitchToneDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tone_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchToneDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tone_db", value))
    }
    /// Snapshot length.
    public static func intermodSnapshotS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "snapshot_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodSnapshotS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "snapshot_s", value))
    }
    /// Peak over the noise.
    public static func intermodPeakDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "peak_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodPeakDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "peak_db", value))
    }
    /// Strong signal over the noise (mean).
    public static func intermodInputDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "input_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodInputDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "input_db", value))
    }
    /// Strong signals used.
    public static func intermodMaxInputs(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_inputs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodMaxInputs(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_inputs", value))
    }
    /// Also f2 - f1.
    public static func intermodSecondOrder(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "second_order", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodSecondOrder(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "second_order", value))
    }
    /// Peak over the noise.
    public static func spurPeakDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "peak_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurPeakDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "peak_db", value))
    }
    /// DC spike over the noise.
    public static func spurDcDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dc_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurDcDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dc_db", value))
    }
    /// Fixed spur grid as a fraction of the rate.
    public static func spurGrid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "spur_grid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurGrid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "spur_grid", value))
    }
    /// Strong signals used.
    public static func spurMaxSignals(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_signals", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurMaxSignals(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_signals", value))
    }
    /// Statistics window.
    public static func fadingWindowS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "window_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingWindowS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "window_s", value))
    }
    /// First reading after.
    public static func fadingFirstS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "first_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingFirstS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "first_s", value))
    }
    /// Fade threshold under the mean.
    public static func fadingFadeDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fade_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingFadeDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fade_db", value))
    }
    /// Fade logged deeper than.
    public static func fadingLogDepthDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "log_depth_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingLogDepthDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "log_depth_db", value))
    }
    /// Fade logged longer than.
    public static func fadingLogLengthS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "log_length_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingLogLengthS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "log_length_s", value))
    }
    /// Line over the noise.
    public static func beaconMonitorPresentDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "present_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorPresentDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "present_db", value))
    }
    /// Aurora-like above.
    public static func beaconMonitorAuroraHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "aurora_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorAuroraHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "aurora_hz", value))
    }
    /// Fading swing above.
    public static func beaconMonitorFadingDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fading_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorFadingDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fading_db", value))
    }
    /// Spectra per decision.
    public static func beaconMonitorSpectra(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "spectra", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorSpectra(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "spectra", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum MeterGeneratorSettings {
    /// Audio tone deviation.
    public static func fmMonitorAudioKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_audio_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorAudioKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_audio_khz", value))
    }
    /// Second tone deviation.
    public static func fmMonitorTone2Khz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone2_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorTone2Khz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone2_khz", value))
    }
    /// Pilot deviation.
    public static func fmMonitorPilotKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pilot_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorPilotKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pilot_khz", value))
    }
    /// L-R tone deviation.
    public static func fmMonitorStereoKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_stereo_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorStereoKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_stereo_khz", value))
    }
    /// RDS deviation.
    public static func fmMonitorRdsKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rds_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorRdsKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rds_khz", value))
    }
    /// 67 kHz subcarrier deviation.
    public static func fmMonitorSca67Khz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_sca67_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorSca67Khz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_sca67_khz", value))
    }
    /// 92 kHz subcarrier deviation.
    public static func fmMonitorSca92Khz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_sca92_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorSca92Khz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_sca92_khz", value))
    }
    /// Multiplex scale.
    public static func fmMonitorScale(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_scale", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorScale(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_scale", value))
    }
    /// Pilot from.
    public static func fmMonitorPilotFromS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pilot_from_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorPilotFromS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pilot_from_s", value))
    }
    /// Pilot until.
    public static func fmMonitorPilotUntilS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pilot_until_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorPilotUntilS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pilot_until_s", value))
    }
    /// RDS from.
    public static func fmMonitorRdsFromS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rds_from_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorRdsFromS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rds_from_s", value))
    }
    /// RDS until.
    public static func fmMonitorRdsUntilS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rds_until_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorRdsUntilS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rds_until_s", value))
    }
    /// Subcarriers from.
    public static func fmMonitorScaFromS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_sca_from_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFmMonitorScaFromS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_sca_from_s", value))
    }
    /// First tone depth.
    public static func amMonitorDepth1(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth1", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorDepth1(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth1", value))
    }
    /// Second tone depth.
    public static func amMonitorDepth2(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorDepth2(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth2", value))
    }
    /// First tone.
    public static func amMonitorTone1Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone1_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorTone1Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone1_hz", value))
    }
    /// Second tone.
    public static func amMonitorTone2Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone2_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorTone2Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone2_hz", value))
    }
    /// Asymmetry.
    public static func amMonitorAsymmetry(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_asymmetry", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorAsymmetry(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_asymmetry", value))
    }
    /// Carrier cut from.
    public static func amMonitorCutFromS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cut_from_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorCutFromS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cut_from_s", value))
    }
    /// Carrier cut until.
    public static func amMonitorCutUntilS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_cut_until_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorCutUntilS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_cut_until_s", value))
    }
    /// Clip at zero carrier.
    public static func amMonitorClip(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_clip", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAmMonitorClip(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_clip", value))
    }
    /// Carriers.
    public static func occupancyTones(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tones", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setOccupancyTones(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tones", $0) })
    }
    /// Added noise centre.
    public static func noiseFloorExtraHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorExtraHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_hz", value))
    }
    /// Added noise width.
    public static func noiseFloorExtraBwHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_bw_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorExtraBwHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_bw_hz", value))
    }
    /// Added noise power.
    public static func noiseFloorExtraPower(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_power", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorExtraPower(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_power", value))
    }
    /// Added noise from.
    public static func noiseFloorExtraFromS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_from_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorExtraFromS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_from_s", value))
    }
    /// Added noise until.
    public static func noiseFloorExtraToS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_to_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorExtraToS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_to_s", value))
    }
    /// Carrier offset.
    public static func noiseFloorToneHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorToneHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value))
    }
    /// Carrier amplitude.
    public static func noiseFloorToneAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFloorToneAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_amp", value))
    }
    /// Bursts.
    public static func burstFinderBursts(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_bursts", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setBurstFinderBursts(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_bursts", $0) })
    }
    /// Channels.
    public static func hopperChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_channels", value))
    }
    /// Channel spacing.
    public static func hopperSpacingHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spacing_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperSpacingHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spacing_hz", value))
    }
    /// Dwell.
    public static func hopperDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_dwell_s", value))
    }
    /// Guard.
    public static func hopperGuardS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_guard_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperGuardS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_guard_s", value))
    }
    /// Hop clock jitter.
    public static func hopperJitter(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_jitter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperJitter(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_jitter", value))
    }
    /// FSK deviation.
    public static func hopperFskHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fsk_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperFskHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fsk_hz", value))
    }
    /// FSK bit rate.
    public static func hopperFskBps(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fsk_bps", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHopperFskBps(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fsk_bps", value))
    }
    /// Carriers.
    public static func birdieTones(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tones", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieTones(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tones", $0) })
    }
    /// Modulated carrier offset.
    public static func birdieFmHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fm_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieFmHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fm_hz", value))
    }
    /// Modulated carrier amplitude.
    public static func birdieFmAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fm_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieFmAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fm_amp", value))
    }
    /// Modulated carrier deviation.
    public static func birdieFmDevHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fm_dev_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBirdieFmDevHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fm_dev_hz", value))
    }
    /// Symbol rate.
    public static func obwSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Roll-off.
    public static func obwRolloff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rolloff", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwRolloff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value))
    }
    /// PSK points.
    public static func obwPsk(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_psk", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwPsk(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_psk", value))
    }
    /// Neighbour.
    public static func obwNeighbour(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_neighbour", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwNeighbour(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_neighbour", value))
    }
    /// Neighbour level.
    public static func obwNeighbourDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_neighbour_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwNeighbourDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_neighbour_db", value))
    }
    /// Neighbour offset.
    public static func obwNeighbourOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_neighbour_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setObwNeighbourOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_neighbour_offset", value))
    }
    /// Transmissions.
    public static func pttLogKeyed(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_keyed", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogKeyed(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_keyed", $0) })
    }
    /// Fades.
    public static func pttLogFades(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_fades", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogFades(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_fades", $0) })
    }
    /// Carrier offset.
    public static func pttLogToneHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPttLogToneHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value))
    }
    /// Impulse rate.
    public static func noiseLocatorRateHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rate_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorRateHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rate_hz", value))
    }
    /// Timing jitter.
    public static func noiseLocatorJitterUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_jitter_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorJitterUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_jitter_us", value))
    }
    /// Impulse width.
    public static func noiseLocatorWidthUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_width_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorWidthUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_width_us", value))
    }
    /// Impulse peak.
    public static func noiseLocatorPeakDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_peak_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorPeakDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_peak_db", value))
    }
    /// Peak spread.
    public static func noiseLocatorSpreadDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spread_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorSpreadDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spread_db", value))
    }
    /// Missing share.
    public static func noiseLocatorDrop(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_drop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorDrop(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_drop", value))
    }
    /// Random impulse rate.
    public static func noiseLocatorRandomRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_random_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorRandomRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_random_rate", value))
    }
    /// Random impulse peak.
    public static func noiseLocatorRandomDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_random_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorRandomDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_random_db", value))
    }
    /// Rate drift.
    public static func noiseLocatorRateDriftHzS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rate_drift_hz_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorRateDriftHzS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rate_drift_hz_s", value))
    }
    /// Reference noise per quadrature.
    public static func noiseLocatorNoiseSigma(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_noise_sigma", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseLocatorNoiseSigma(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_noise_sigma", value))
    }
    /// Steps.
    public static func freqAccuracySteps(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_steps", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracySteps(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_steps", $0) })
    }
    /// Gaps.
    public static func freqAccuracyGaps(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_gaps", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyGaps(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_gaps", $0) })
    }
    /// Frequency noise.
    public static func freqAccuracyFmNoiseHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fm_noise_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyFmNoiseHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fm_noise_hz", value))
    }
    /// Frequency noise hold.
    public static func freqAccuracyFmHoldS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fm_hold_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreqAccuracyFmHoldS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fm_hold_s", value))
    }
    /// Test tone.
    public static func sinadToneHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadToneHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value))
    }
    /// Deviation.
    public static func sinadDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// Second harmonic.
    public static func sinadH2(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_h2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadH2(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_h2", value))
    }
    /// Third harmonic.
    public static func sinadH3(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_h3", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadH3(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_h3", value))
    }
    /// Later deviations.
    public static func sinadDeviationAfter(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_deviation_after", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSinadDeviationAfter(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_deviation_after", $0) })
    }
    /// Voice scale.
    public static func nfmMeterVoiceHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_voice_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterVoiceHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_voice_hz", value))
    }
    /// CTCSS tone.
    public static func nfmMeterCtcssHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ctcss_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterCtcssHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ctcss_hz", value))
    }
    /// CTCSS deviation.
    public static func nfmMeterCtcssDevHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ctcss_dev_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterCtcssDevHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ctcss_dev_hz", value))
    }
    /// CTCSS off at.
    public static func nfmMeterCtcssOffS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ctcss_off_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNfmMeterCtcssOffS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ctcss_off_s", value))
    }
    /// Transmissions.
    public static func txStabilityKeyed(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_keyed", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityKeyed(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_keyed", $0) })
    }
    /// Final frequency.
    public static func txStabilityFinalHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_final_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityFinalHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_final_hz", value))
    }
    /// Start offset.
    public static func txStabilityStartOffsetHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_start_offset_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityStartOffsetHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_start_offset_hz", value))
    }
    /// Settling time constant.
    public static func txStabilityTauS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tau_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityTauS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tau_s", value))
    }
    /// Drift in a transmission.
    public static func txStabilityTxDriftHzS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tx_drift_hz_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityTxDriftHzS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tx_drift_hz_s", value))
    }
    /// 1 kHz tone deviation.
    public static func txStabilityModHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_mod_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityModHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_mod_hz", value))
    }
    /// Key ramp.
    public static func txStabilityRampS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ramp_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTxStabilityRampS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ramp_s", value))
    }
    /// Tones.
    public static func ssbTestTones(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tones", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestTones(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tones", $0) })
    }
    /// The options of the setting "gen_sideband" of the stack "ssb_test".
    public enum SsbTestSideband: Int32 {
        case upper = 0
        case lower = 1
    }
    /// Sideband.
    public static func ssbTestSideband(_ handle: SDRKIT_HANDLE) -> Result<SsbTestSideband, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_sideband", &v)
        return code == SDRKIT_OK ? .success(SsbTestSideband(rawValue: v) ?? SsbTestSideband(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestSideband(_ handle: SDRKIT_HANDLE, _ value: SsbTestSideband) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_sideband", value.rawValue))
    }
    /// Carrier leak.
    public static func ssbTestCarrierDbc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_carrier_dbc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSsbTestCarrierDbc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_carrier_dbc", value))
    }
    /// Deviation.
    public static func afskTunerDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// Twist, space over mark.
    public static func afskTunerTwistDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_twist_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerTwistDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_twist_db", value))
    }
    /// Bit rate.
    public static func afskTunerBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud", value))
    }
    /// Burst period.
    public static func afskTunerPeriodS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_period_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerPeriodS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_period_s", value))
    }
    /// Burst length.
    public static func afskTunerBurstS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_burst_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerBurstS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_burst_s", value))
    }
    /// First burst at.
    public static func afskTunerFirstS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_first_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAfskTunerFirstS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_first_s", value))
    }
    /// Linewidth.
    public static func phaseNoiseLinewidthHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_linewidth_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseLinewidthHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_linewidth_hz", value))
    }
    /// White phase floor.
    public static func phaseNoiseFloorDbc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_floor_dbc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseFloorDbc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_floor_dbc", value))
    }
    /// Spur offset.
    public static func phaseNoiseSpurHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spur_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseSpurHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spur_hz", value))
    }
    /// Spur level.
    public static func phaseNoiseSpurDbc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spur_dbc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPhaseNoiseSpurDbc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spur_dbc", value))
    }
    /// Text.
    public static func cwKeyingText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_text", $0) })
    }
    /// Speed.
    public static func cwKeyingWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wpm", value))
    }
    /// Dah ratio.
    public static func cwKeyingDahRatio(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_dah_ratio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingDahRatio(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_dah_ratio", value))
    }
    /// Weight.
    public static func cwKeyingWeight(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_weight", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingWeight(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_weight", value))
    }
    /// Character gap.
    public static func cwKeyingCharGap(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_char_gap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingCharGap(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_char_gap", value))
    }
    /// Word gap.
    public static func cwKeyingWordGap(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_word_gap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingWordGap(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_word_gap", value))
    }
    /// Mark jitter.
    public static func cwKeyingJitter(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_jitter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingJitter(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_jitter", value))
    }
    /// Edge length.
    public static func cwKeyingRampS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ramp_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingRampS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ramp_s", value))
    }
    /// Chirp.
    public static func cwKeyingChirpHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_chirp_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingChirpHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_chirp_hz", value))
    }
    /// Chirp time constant.
    public static func cwKeyingChirpTauS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_chirp_tau_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingChirpTauS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_chirp_tau_s", value))
    }
    /// Level swing at 0.2 Hz.
    public static func cwKeyingFadeDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fade_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCwKeyingFadeDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fade_db", value))
    }
    /// Levels.
    public static func fskAnalyserLevels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_levels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserLevels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_levels", value))
    }
    /// Symbol rate.
    public static func fskAnalyserBaud(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_baud", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserBaud(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_baud", value))
    }
    /// Deviation.
    public static func fskAnalyserDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// Gaussian BT.
    public static func fskAnalyserBt(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bt", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFskAnalyserBt(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bt", value))
    }
    /// Phases.
    public static func merPsk(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_psk", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerPsk(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_psk", value))
    }
    /// Symbol rate.
    public static func merSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Roll-off.
    public static func merRolloff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rolloff", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerRolloff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value))
    }
    /// Points turned by pi / M.
    public static func merRotated(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_rotated", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMerRotated(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_rotated", value))
    }
    /// The options of the setting "gen_kind" of the stack "papr".
    public enum PaprKind: Int32 {
        case noise = 0
        case fm = 1
        case qpsk = 2
        case clippedNoise = 3
        case noiseWithImpulses = 4
    }
    /// Signal.
    public static func paprKind(_ handle: SDRKIT_HANDLE) -> Result<PaprKind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &v)
        return code == SDRKIT_OK ? .success(PaprKind(rawValue: v) ?? PaprKind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprKind(_ handle: SDRKIT_HANDLE, _ value: PaprKind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_kind", value.rawValue))
    }
    /// Power.
    public static func paprPower(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_power", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprPower(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_power", value))
    }
    /// Magnitude limit.
    public static func paprClip(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_clip", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprClip(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_clip", value))
    }
    /// Impulse every.
    public static func paprImpulseEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_impulse_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprImpulseEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_impulse_every", value))
    }
    /// Impulse size.
    public static func paprImpulseAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_impulse_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprImpulseAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_impulse_amp", value))
    }
    /// Symbol rate.
    public static func paprSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// Roll-off.
    public static func paprRolloff(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_rolloff", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPaprRolloff(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value))
    }
    /// Noise power.
    public static func inputHealthPower(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_power", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthPower(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_power", value))
    }
    /// Tones.
    public static func inputHealthTones(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tones", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthTones(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tones", $0) })
    }
    /// Clip at.
    public static func inputHealthClip(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_clip", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthClip(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_clip", value))
    }
    /// Gain after the clip.
    public static func inputHealthScale(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_scale", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setInputHealthScale(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_scale", value))
    }
    /// Y-factor.
    public static func noiseFigureYDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_y_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureYDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_y_db", value))
    }
    /// Source toggles every.
    public static func noiseFigurePeriodS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_period_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigurePeriodS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_period_s", value))
    }
    /// Gain drift.
    public static func noiseFigureDriftDbS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_drift_db_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureDriftDbS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_drift_db_s", value))
    }
    /// Receiver noise.
    public static func noiseFigureP0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_p0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNoiseFigureP0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_p0", value))
    }
    /// Tones.
    public static func kurtosisTones(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_tones", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setKurtosisTones(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_tones", $0) })
    }
    /// Tone.
    public static func glitchToneHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchToneHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value))
    }
    /// Tone amplitude.
    public static func glitchToneAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchToneAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone_amp", value))
    }
    /// Drops and repeats.
    public static func glitchDrops(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_drops", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchDrops(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_drops", $0) })
    }
    /// Zero runs.
    public static func glitchZeroRuns(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_zero_runs", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchZeroRuns(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_zero_runs", $0) })
    }
    /// Repeated blocks.
    public static func glitchRepeatBlocks(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_repeat_blocks", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchRepeatBlocks(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_repeat_blocks", $0) })
    }
    /// Gain steps.
    public static func glitchGainSteps(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_gain_steps", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGlitchGainSteps(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_gain_steps", $0) })
    }
    /// First tone.
    public static func intermodF1Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_f1_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodF1Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_f1_hz", value))
    }
    /// Second tone.
    public static func intermodF2Hz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_f2_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodF2Hz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_f2_hz", value))
    }
    /// Tone amplitude.
    public static func intermodAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_amp", value))
    }
    /// Level swing.
    public static func intermodSwingDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_swing_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodSwingDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_swing_db", value))
    }
    /// Swing rates.
    public static func intermodSwingHz(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_swing_hz", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodSwingHz(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_swing_hz", $0) })
    }
    /// Cubic term of the receiver.
    public static func intermodK(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_k", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodK(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_k", value))
    }
    /// Independent tone.
    public static func intermodExtraHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodExtraHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_hz", value))
    }
    /// Independent tone amplitude.
    public static func intermodExtraAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_extra_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIntermodExtraAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_extra_amp", value))
    }
    /// Signals.
    public static func spurSignals(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_signals", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurSignals(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_signals", $0) })
    }
    /// Level swing.
    public static func spurSwingDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_swing_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurSwingDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_swing_db", value))
    }
    /// Image rejection.
    public static func spurImageDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_image_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurImageDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_image_db", value))
    }
    /// Image phase.
    public static func spurImagePhase(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_image_phase", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurImagePhase(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_image_phase", value))
    }
    /// DC offset.
    public static func spurDc(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_dc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurDc(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_dc", value))
    }
    /// Second-order term.
    public static func spurH2(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_h2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurH2(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_h2", value))
    }
    /// Third-order term.
    public static func spurH3(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_h3", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurH3(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_h3", value))
    }
    /// Fixed tone.
    public static func spurHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spur_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spur_hz", value))
    }
    /// Fixed tone amplitude.
    public static func spurAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spur_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spur_amp", value))
    }
    /// Real signal.
    public static func spurRealHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_real_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurRealHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_real_hz", value))
    }
    /// Real signal amplitude.
    public static func spurRealAmp(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_real_amp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurRealAmp(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_real_amp", value))
    }
    /// Shift of the signals.
    public static func spurShiftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSpurShiftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value))
    }
    /// Largest Doppler.
    public static func fadingFdHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fd_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingFdHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fd_hz", value))
    }
    /// Rice factor.
    public static func fadingK(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_k", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingK(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_k", value))
    }
    /// No carrier.
    public static func fadingOff(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_off", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFadingOff(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_off", $0) })
    }
    /// Fading rate.
    public static func beaconMonitorFadeHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fade_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorFadeHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fade_hz", value))
    }
    /// Fading depth.
    public static func beaconMonitorFadeDepthDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_fade_depth_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorFadeDepthDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_fade_depth_db", value))
    }
    /// Aurora spread.
    public static func beaconMonitorSpreadHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_spread_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorSpreadHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_spread_hz", value))
    }
    /// No beacon.
    public static func beaconMonitorOff(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_off", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setBeaconMonitorOff(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_off", $0) })
    }
}

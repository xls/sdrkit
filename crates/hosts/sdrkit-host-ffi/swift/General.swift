// sdrkit: the typed interface of the stack family "general", version 1.3. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum GeneralInterface {
    public static let major = 1
    public static let minor = 3
}

/// The result fields of the AM processor. The measurements have no value while the processor does not run.
public struct GeneralAmFields {
    public enum AmDetector: Int32 {
        case envelope = 0
        case synchronous = 1
        case upper = 2
        case lower = 3
    }
    /// Whether the AM processor runs.
    public var amActive: Bool
    /// Whether it found a carrier.
    public var amCarrierFound: Bool
    /// Whether its loop is locked.
    public var amLocked: Bool
    /// The carrier relative to the channel. No value without a carrier.
    public var amCarrierOffsetHz: Double?
    /// The carrier level.
    public var amCarrierDbfs: Double?
    /// The programme SNR.
    public var amProgrammeSnrDb: Double?
    /// The detector in use.
    public var amDetector: AmDetector?
    /// Whether one sideband was chosen because of interference.
    public var amOneSide: Bool
    /// The interference measure above the carrier.
    public var amInterferenceUpper: Double?
    /// The interference measure below the carrier.
    public var amInterferenceLower: Double?
    /// The audio bandwidth in use.
    public var amBandwidthHz: Double?
    /// The noise reduction floor. No value with the noise reduction off.
    public var amNrFloorDb: Double?
    /// The share of samples the AM blanker removed.
    public var amBlankedPct: Double?
    /// The threshold of the AM blanker.
    public var amBlankerDb: Double?
    /// The hum notched (50 or 60 Hz). No value for none.
    public var amHumHz: Double?
    /// Whether the loop holds through a fade.
    public var amFading: Bool
    /// Whether the synthetic treble is on.
    public var amEnhanced: Bool
    /// The gain of the leveller.
    public var amLevellerDb: Double?
    /// The state of the AM processor in words.
    public var amStatus: String?

    init(_ c: sdrkit_general_am_fields) {
        amActive = c.am_active
        amCarrierFound = c.am_carrier_found
        amLocked = c.am_locked
        amCarrierOffsetHz = c.has_am_carrier_offset_hz ? c.am_carrier_offset_hz : nil
        amCarrierDbfs = c.has_am_carrier_dbfs ? c.am_carrier_dbfs : nil
        amProgrammeSnrDb = c.has_am_programme_snr_db ? c.am_programme_snr_db : nil
        amDetector = c.has_am_detector ? AmDetector(rawValue: c.am_detector) : nil
        amOneSide = c.am_one_side
        amInterferenceUpper = c.has_am_interference_upper ? c.am_interference_upper : nil
        amInterferenceLower = c.has_am_interference_lower ? c.am_interference_lower : nil
        amBandwidthHz = c.has_am_bandwidth_hz ? c.am_bandwidth_hz : nil
        amNrFloorDb = c.has_am_nr_floor_db ? c.am_nr_floor_db : nil
        amBlankedPct = c.has_am_blanked_pct ? c.am_blanked_pct : nil
        amBlankerDb = c.has_am_blanker_db ? c.am_blanker_db : nil
        amHumHz = c.has_am_hum_hz ? c.am_hum_hz : nil
        amFading = c.am_fading
        amEnhanced = c.am_enhanced
        amLevellerDb = c.has_am_leveller_db ? c.am_leveller_db : nil
        amStatus = c.has_am_status ? sdrkitText(c.am_status) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GeneralAmFields, SDRKitCode> {
        var c = sdrkit_general_am_fields()
        let code = sdrkit_general_am_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GeneralAmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the report.
public struct GeneralFields {
    public enum Demod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    /// The demodulator.
    public var demod: Demod?
    /// The filter width.
    public var widthHz: Double
    /// The input rate.
    public var inputRateHz: Double
    /// The demodulation rate (48 kHz, WFM 240 kHz).
    public var demodRateHz: Double
    /// The channel relative to the dial.
    public var channelOffsetHz: Double
    /// The dial frequency.
    public var dialHz: Double
    /// The listening frequency: the dial plus the channel offset.
    public var listeningHz: Double
    /// The low edge of the passband relative to the channel.
    public var filterLoHz: Double
    /// The high edge of the passband relative to the channel.
    public var filterHiHz: Double
    /// The low edge of the passband.
    public var passbandLoHz: Double
    /// The high edge of the passband.
    public var passbandHiHz: Double
    /// The level in the channel. No value before the first squelch block.
    public var levelDbfs: Double?
    /// The noise in the channel. No value while the noise is not measured.
    public var noiseDbfs: Double?
    /// The SNR. No value while the noise is not measured.
    public var snrDb: Double?
    /// The squelch threshold. No value with the squelch off or the noise not measured.
    public var squelchThresholdDbfs: Double?
    /// Whether the squelch is open.
    public var squelchOpen: Bool
    /// The frequency offset of an FM signal. No value with the squelch closed or another demodulator.
    public var offsetHz: Double?
    /// The power of the audio played over the last report. No value without audio.
    public var audioDbfs: Double?
    /// 10 ms blocks with the squelch open. No value before the first squelch block.
    public var blocksOpen: Int64?
    /// 10 ms blocks with the squelch closed. No value before the first squelch block.
    public var blocksClosed: Int64?
    /// The share of samples the blanker removed.
    public var blankedPct: Double
    /// The speech probability of the voice noise remover for the last 10 ms (0 to 1).
    public var speechProbability: Double
    /// The position of the needle of the S-meter (0 to 1). No value without a level.
    public var needle: Double?
    /// The CTCSS tone found.
    public var ctcssHz: Double?
    /// The measured frequency of the CTCSS tone.
    public var ctcssMeasuredHz: Double?
    /// The level of the CTCSS tone against 5 kHz deviation.
    public var ctcssDb: Double?
    /// The DCS code found ("D023N").
    public var dcs: String?
    /// Another standard code that the same waveform reads as.
    public var dcsAlias: String?
    /// The CTCSS tone or DCS code heard, as a label for lists of things heard ("CTCSS 88.5 Hz", "DCS D023N").
    public var tone: String?
    /// Whether the tone gate lets the audio through.
    public var toneGateOpen: Bool

    init(_ c: sdrkit_general_fields) {
        demod = Demod(rawValue: c.demod)
        widthHz = c.width_hz
        inputRateHz = c.input_rate_hz
        demodRateHz = c.demod_rate_hz
        channelOffsetHz = c.channel_offset_hz
        dialHz = c.dial_hz
        listeningHz = c.listening_hz
        filterLoHz = c.filter_lo_hz
        filterHiHz = c.filter_hi_hz
        passbandLoHz = c.passband_lo_hz
        passbandHiHz = c.passband_hi_hz
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        squelchThresholdDbfs = c.has_squelch_threshold_dbfs ? c.squelch_threshold_dbfs : nil
        squelchOpen = c.squelch_open
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        audioDbfs = c.has_audio_dbfs ? c.audio_dbfs : nil
        blocksOpen = c.has_blocks_open ? c.blocks_open : nil
        blocksClosed = c.has_blocks_closed ? c.blocks_closed : nil
        blankedPct = c.blanked_pct
        speechProbability = c.speech_probability
        needle = c.has_needle ? c.needle : nil
        ctcssHz = c.has_ctcss_hz ? c.ctcss_hz : nil
        ctcssMeasuredHz = c.has_ctcss_measured_hz ? c.ctcss_measured_hz : nil
        ctcssDb = c.has_ctcss_db ? c.ctcss_db : nil
        dcs = c.has_dcs ? sdrkitText(c.dcs) : nil
        dcsAlias = c.has_dcs_alias ? sdrkitText(c.dcs_alias) : nil
        tone = c.has_tone ? sdrkitText(c.tone) : nil
        toneGateOpen = c.tone_gate_open
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GeneralFields, SDRKitCode> {
        var c = sdrkit_general_fields()
        let code = sdrkit_general_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GeneralFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the transmitter. The measurements have no value while the stack does not transmit.
public struct GeneralTransmitFields {
    public enum TxKind: Int32 {
        case voice = 0
        case text = 1
        case carrier = 2
    }
    /// Whether the stack transmits now. The receive audio is muted meanwhile.
    public var transmitting: Bool
    /// The transmit frequency: the listening frequency plus the transmit offset.
    public var txFreqHz: Double?
    /// What the transmission sends.
    public var txKind: TxKind?
    /// The time on air of the transmission.
    public var txTimeS: Double?
    /// The peak of the microphone audio over the last report. No value without voice.
    public var txMicDbfs: Double?
    /// The peak of the modulation over the last report, against full modulation (deviation, depth or full scale).
    public var txPeakDb: Double?
    /// The gain of the level control of the speech processor.
    public var txLevelDb: Double?
    /// The most gain reduction of the limiter and the compressor over the last report: 0 when they did not act.
    public var txLimitDb: Double?

    init(_ c: sdrkit_general_transmit_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txKind = c.has_tx_kind ? TxKind(rawValue: c.tx_kind) : nil
        txTimeS = c.has_tx_time_s ? c.tx_time_s : nil
        txMicDbfs = c.has_tx_mic_dbfs ? c.tx_mic_dbfs : nil
        txPeakDb = c.has_tx_peak_db ? c.tx_peak_db : nil
        txLevelDb = c.has_tx_level_db ? c.tx_level_db : nil
        txLimitDb = c.has_tx_limit_db ? c.tx_limit_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GeneralTransmitFields, SDRKitCode> {
        var c = sdrkit_general_transmit_fields()
        let code = sdrkit_general_transmit_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GeneralTransmitFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the width presets. The row key is the place of the preset ("00", "01" ...).
public struct GeneralWidthPresetRow {
    /// The row key.
    public var key: String
    /// The width.
    public var widthHz: Double

    init(_ c: sdrkit_general_width_preset_row) {
        key = sdrkitText(c.key)
        widthHz = c.width_hz
    }
}

/// A row of the built-in presets. The row key is the key of the preset.
public struct GeneralPresetRow {
    public enum Demod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    public enum Squelch: Int32 {
        case off = 0
        case auto = 1
        case level = 2
    }
    /// The row key.
    public var key: String
    /// The name of the preset.
    public var name: String
    /// The frequency of the preset.
    public var freqHz: Double
    /// The demodulator of the preset.
    public var demod: Demod?
    /// The filter width of the preset.
    public var width: Double
    /// The squelch of the preset.
    public var squelch: Squelch?

    init(_ c: sdrkit_general_preset_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        freqHz = c.freq_hz
        demod = Demod(rawValue: c.demod)
        width = c.width
        squelch = Squelch(rawValue: c.squelch)
    }
}

/// A row of the extra channels. The row key is the number of the channel ("1" to "3").
public struct GeneralChannelRow {
    public enum Demod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    /// The row key.
    public var key: String
    /// The name of the channel.
    public var channel: String
    /// The listening frequency of the channel.
    public var freqHz: Double?
    /// The demodulator of the channel.
    public var demod: Demod?
    /// Whether the squelch of the channel is open.
    public var squelchOpen: Bool?
    /// The summary line of the channel.
    public var summary: String

    init(_ c: sdrkit_general_channel_row) {
        key = sdrkitText(c.key)
        channel = sdrkitText(c.channel)
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        demod = c.has_demod ? Demod(rawValue: c.demod) : nil
        squelchOpen = c.has_squelch_open ? c.squelch_open : nil
        summary = sdrkitText(c.summary)
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum GeneralTables {
    /// The changed rows of the table "width_presets" of the stack "general", newest first, at most `capacity` rows.
    public static func generalWidthPresets(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[GeneralWidthPresetRow], SDRKitCode> {
        var rows = [sdrkit_general_width_preset_row](repeating: sdrkit_general_width_preset_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_general_width_presets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GeneralWidthPresetRow.init))
    }
    /// The changed rows of the table "presets" of the stack "general", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func generalPresets(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[GeneralPresetRow], SDRKitCode> {
        var rows = [sdrkit_general_preset_row](repeating: sdrkit_general_preset_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_general_presets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GeneralPresetRow.init))
    }
    /// The changed rows of the table "channels" of the stack "general", newest first, at most `capacity` rows.
    public static func generalChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[GeneralChannelRow], SDRKitCode> {
        var rows = [sdrkit_general_channel_row](repeating: sdrkit_general_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_general_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GeneralChannelRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum GeneralCommands {
    /// Apply preset. Sets the demodulator, the filter width and the squelch of a preset, and tunes to its frequency.
    public static func generalPresetApply(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_general_preset_apply_args()
        args.size = UInt32(MemoryLayout<sdrkit_general_preset_apply_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_general_preset_apply(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Push to talk. With on true, sends the speech of the microphone input in the mode of the demodulator (NFM, AM, USB or LSB) until ptt with on false, halt or the time-out. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
    public static func generalPtt(_ handle: SDRKIT_HANDLE, on: Bool) -> Result<Void, SDRKitCode> {
        var args = sdrkit_general_ptt_args()
        args.size = UInt32(MemoryLayout<sdrkit_general_ptt_args>.size)
        let code: Int32 = { () -> Int32 in
        args.on = on
        return sdrkit_general_ptt(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Send Morse. Keys the text in Morse code at the CW speed, with the demodulator on CW, the carrier at the listening frequency plus the transmit offset. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func generalSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_general_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_general_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_general_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Tune. Sends an unmodulated carrier at the listening frequency plus the transmit offset for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func generalTune(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_general_tune(handle)
        return sdrkitResult(code)
    }
    /// Halt. Ends speech, Morse code or the tune carrier at once.
    public static func generalHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_general_halt(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum GeneralSettings {
    /// The options of the setting "demod" of the stack "general".
    public enum GeneralDemod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    /// Demodulator.
    public static func generalDemod(_ handle: SDRKIT_HANDLE) -> Result<GeneralDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(GeneralDemod(rawValue: v) ?? GeneralDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralDemod(_ handle: SDRKIT_HANDLE, _ value: GeneralDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Filter.
    public static func generalWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "width", value))
    }
    /// Channel offset.
    public static func generalChannelOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralChannelOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_offset", value))
    }
    /// Step.
    public static func generalStep(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "step", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralStep(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "step", value))
    }
    /// Custom baseband filter.
    public static func generalCustomFilter(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "custom_filter", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralCustomFilter(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "custom_filter", $0) })
    }
    /// The options of the setting "squelch" of the stack "general".
    public enum GeneralSquelch: Int32 {
        case off = 0
        case auto = 1
        case level = 2
    }
    /// Squelch.
    public static func generalSquelch(_ handle: SDRKIT_HANDLE) -> Result<GeneralSquelch, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(GeneralSquelch(rawValue: v) ?? GeneralSquelch(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralSquelch(_ handle: SDRKIT_HANDLE, _ value: GeneralSquelch) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "squelch", value.rawValue))
    }
    /// Squelch level.
    public static func generalSquelchLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralSquelchLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_level", value))
    }
    /// AGC.
    public static func generalAgc(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "agc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAgc(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "agc", value))
    }
    /// Gain.
    public static func generalGain(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gain", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralGain(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gain", value))
    }
    /// The options of the setting "deemph_nfm" of the stack "general".
    public enum GeneralDeemphNfm: Int32 {
        case off = 0
        case n50Us = 1
        case n75Us = 2
        case n750Us = 3
    }
    /// De-emphasis (NFM).
    public static func generalDeemphNfm(_ handle: SDRKIT_HANDLE) -> Result<GeneralDeemphNfm, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deemph_nfm", &v)
        return code == SDRKIT_OK ? .success(GeneralDeemphNfm(rawValue: v) ?? GeneralDeemphNfm(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralDeemphNfm(_ handle: SDRKIT_HANDLE, _ value: GeneralDeemphNfm) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deemph_nfm", value.rawValue))
    }
    /// The options of the setting "deemph_wfm" of the stack "general".
    public enum GeneralDeemphWfm: Int32 {
        case off = 0
        case n50Us = 1
        case n75Us = 2
        case n750Us = 3
    }
    /// De-emphasis (WFM).
    public static func generalDeemphWfm(_ handle: SDRKIT_HANDLE) -> Result<GeneralDeemphWfm, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deemph_wfm", &v)
        return code == SDRKIT_OK ? .success(GeneralDeemphWfm(rawValue: v) ?? GeneralDeemphWfm(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralDeemphWfm(_ handle: SDRKIT_HANDLE, _ value: GeneralDeemphWfm) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deemph_wfm", value.rawValue))
    }
    /// The options of the setting "nfm_lowpass" of the stack "general".
    public enum GeneralNfmLowpass: Int32 {
        case n3khz = 0
        case n4khz = 1
    }
    /// Audio low-pass (NFM).
    public static func generalNfmLowpass(_ handle: SDRKIT_HANDLE) -> Result<GeneralNfmLowpass, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "nfm_lowpass", &v)
        return code == SDRKIT_OK ? .success(GeneralNfmLowpass(rawValue: v) ?? GeneralNfmLowpass(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralNfmLowpass(_ handle: SDRKIT_HANDLE, _ value: GeneralNfmLowpass) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "nfm_lowpass", value.rawValue))
    }
    /// Pitch.
    public static func generalCwPitch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "cw_pitch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralCwPitch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "cw_pitch", value))
    }
    /// Auto quality.
    public static func generalAmAuto(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "am_auto", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAmAuto(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "am_auto", value))
    }
    /// The options of the setting "am_detector" of the stack "general".
    public enum GeneralAmDetector: Int32 {
        case auto = 0
        case envelope = 1
        case synchronous = 2
        case upper = 3
        case lower = 4
    }
    /// Detector.
    public static func generalAmDetector(_ handle: SDRKIT_HANDLE) -> Result<GeneralAmDetector, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "am_detector", &v)
        return code == SDRKIT_OK ? .success(GeneralAmDetector(rawValue: v) ?? GeneralAmDetector(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAmDetector(_ handle: SDRKIT_HANDLE, _ value: GeneralAmDetector) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "am_detector", value.rawValue))
    }
    /// The options of the setting "am_nr" of the stack "general".
    public enum GeneralAmNr: Int32 {
        case auto = 0
        case off = 1
        case gentle = 2
        case strong = 3
    }
    /// Noise.
    public static func generalAmNr(_ handle: SDRKIT_HANDLE) -> Result<GeneralAmNr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "am_nr", &v)
        return code == SDRKIT_OK ? .success(GeneralAmNr(rawValue: v) ?? GeneralAmNr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAmNr(_ handle: SDRKIT_HANDLE, _ value: GeneralAmNr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "am_nr", value.rawValue))
    }
    /// Audio bandwidth.
    public static func generalAmBandwidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "am_bandwidth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAmBandwidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "am_bandwidth", value))
    }
    /// Enhance (synthetic treble).
    public static func generalAmEnhance(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "am_enhance", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAmEnhance(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "am_enhance", value))
    }
    /// Blanker.
    public static func generalNb(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "nb", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralNb(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "nb", value))
    }
    /// Blanker threshold.
    public static func generalNbThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "nb_threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralNbThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "nb_threshold", value))
    }
    /// High-pass.
    public static func generalAfHighpass(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "af_highpass", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAfHighpass(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "af_highpass", value))
    }
    /// Low-pass.
    public static func generalAfLowpass(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "af_lowpass", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAfLowpass(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "af_lowpass", value))
    }
    /// Notch.
    public static func generalAfNotch(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "af_notch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAfNotch(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "af_notch", value))
    }
    /// Noise remover (neural).
    public static func generalVoiceNr(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "voice_nr", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralVoiceNr(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "voice_nr", value))
    }
    /// Strength.
    public static func generalVoiceNrStrength(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "voice_nr_strength", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralVoiceNrStrength(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "voice_nr_strength", value))
    }
    /// Presets.
    public static func generalPresets(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "presets", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralPresets(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "presets", $0) })
    }
    /// Tone gate.
    public static func generalToneGate(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "tone_gate", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralToneGate(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "tone_gate", $0) })
    }
    /// Heard channel.
    public static func generalListenChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "listen_channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralListenChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "listen_channel", value))
    }
    /// Extra channel 1.
    public static func generalExtra1(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "extra1", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra1(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "extra1", value))
    }
    /// Extra 1 offset.
    public static func generalExtra1Offset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "extra1_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra1Offset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "extra1_offset", value))
    }
    /// The options of the setting "extra1_demod" of the stack "general".
    public enum GeneralExtra1Demod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    /// Extra 1 demodulator.
    public static func generalExtra1Demod(_ handle: SDRKIT_HANDLE) -> Result<GeneralExtra1Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "extra1_demod", &v)
        return code == SDRKIT_OK ? .success(GeneralExtra1Demod(rawValue: v) ?? GeneralExtra1Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra1Demod(_ handle: SDRKIT_HANDLE, _ value: GeneralExtra1Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "extra1_demod", value.rawValue))
    }
    /// Extra 1 filter.
    public static func generalExtra1Width(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "extra1_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra1Width(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "extra1_width", value))
    }
    /// Extra channel 2.
    public static func generalExtra2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "extra2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "extra2", value))
    }
    /// Extra 2 offset.
    public static func generalExtra2Offset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "extra2_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra2Offset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "extra2_offset", value))
    }
    /// The options of the setting "extra2_demod" of the stack "general".
    public enum GeneralExtra2Demod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    /// Extra 2 demodulator.
    public static func generalExtra2Demod(_ handle: SDRKIT_HANDLE) -> Result<GeneralExtra2Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "extra2_demod", &v)
        return code == SDRKIT_OK ? .success(GeneralExtra2Demod(rawValue: v) ?? GeneralExtra2Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra2Demod(_ handle: SDRKIT_HANDLE, _ value: GeneralExtra2Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "extra2_demod", value.rawValue))
    }
    /// Extra 2 filter.
    public static func generalExtra2Width(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "extra2_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra2Width(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "extra2_width", value))
    }
    /// Extra channel 3.
    public static func generalExtra3(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "extra3", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra3(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "extra3", value))
    }
    /// Extra 3 offset.
    public static func generalExtra3Offset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "extra3_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra3Offset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "extra3_offset", value))
    }
    /// The options of the setting "extra3_demod" of the stack "general".
    public enum GeneralExtra3Demod: Int32 {
        case nfm = 0
        case wfm = 1
        case am = 2
        case usb = 3
        case lsb = 4
        case cw = 5
    }
    /// Extra 3 demodulator.
    public static func generalExtra3Demod(_ handle: SDRKIT_HANDLE) -> Result<GeneralExtra3Demod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "extra3_demod", &v)
        return code == SDRKIT_OK ? .success(GeneralExtra3Demod(rawValue: v) ?? GeneralExtra3Demod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra3Demod(_ handle: SDRKIT_HANDLE, _ value: GeneralExtra3Demod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "extra3_demod", value.rawValue))
    }
    /// Extra 3 filter.
    public static func generalExtra3Width(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "extra3_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralExtra3Width(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "extra3_width", value))
    }
    /// Transmit offset.
    public static func generalTxSplit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_split", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxSplit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_split", value))
    }
    /// Deviation (NFM).
    public static func generalTxDeviation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_deviation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxDeviation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_deviation", value))
    }
    /// The options of the setting "tx_preemphasis" of the stack "general".
    public enum GeneralTxPreemphasis: Int32 {
        case off = 0
        case n50Us = 1
        case n75Us = 2
        case n750Us = 3
    }
    /// Pre-emphasis (NFM).
    public static func generalTxPreemphasis(_ handle: SDRKIT_HANDLE) -> Result<GeneralTxPreemphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_preemphasis", &v)
        return code == SDRKIT_OK ? .success(GeneralTxPreemphasis(rawValue: v) ?? GeneralTxPreemphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxPreemphasis(_ handle: SDRKIT_HANDLE, _ value: GeneralTxPreemphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_preemphasis", value.rawValue))
    }
    /// CTCSS tone.
    public static func generalTxCtcss(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_ctcss", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxCtcss(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_ctcss", value))
    }
    /// Modulation depth (AM).
    public static func generalTxAmDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_am_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxAmDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_am_depth", value))
    }
    /// The options of the setting "tx_ssb_method" of the stack "general".
    public enum GeneralTxSsbMethod: Int32 {
        case filter = 0
        case phasing = 1
    }
    /// Sideband method (SSB).
    public static func generalTxSsbMethod(_ handle: SDRKIT_HANDLE) -> Result<GeneralTxSsbMethod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_ssb_method", &v)
        return code == SDRKIT_OK ? .success(GeneralTxSsbMethod(rawValue: v) ?? GeneralTxSsbMethod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxSsbMethod(_ handle: SDRKIT_HANDLE, _ value: GeneralTxSsbMethod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_ssb_method", value.rawValue))
    }
    /// The options of the setting "tx_ssb_band" of the stack "general".
    public enum GeneralTxSsbBand: Int32 {
        case fixed = 0
        case filter = 1
    }
    /// Audio band (SSB).
    public static func generalTxSsbBand(_ handle: SDRKIT_HANDLE) -> Result<GeneralTxSsbBand, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_ssb_band", &v)
        return code == SDRKIT_OK ? .success(GeneralTxSsbBand(rawValue: v) ?? GeneralTxSsbBand(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxSsbBand(_ handle: SDRKIT_HANDLE, _ value: GeneralTxSsbBand) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_ssb_band", value.rawValue))
    }
    /// Compression (SSB).
    public static func generalTxCompression(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_compression", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxCompression(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_compression", value))
    }
    /// Time-out.
    public static func generalTxTimeout(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_timeout", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxTimeout(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_timeout", value))
    }
    /// Speed (CW).
    public static func generalTxCwWpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_cw_wpm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxCwWpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_cw_wpm", value))
    }
    /// Overall speed (CW).
    public static func generalTxCwFarnsworth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_cw_farnsworth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxCwFarnsworth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_cw_farnsworth", value))
    }
    /// Tune time.
    public static func generalTxTuneLimit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTxTuneLimit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum GeneralGeneratorSettings {
    /// Noise only.
    public static func generalNoiseOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_noise_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralNoiseOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_noise_only", value))
    }
    /// Tune offset.
    public static func generalTuneOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tune_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTuneOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tune_offset", value))
    }
    /// Tone.
    public static func generalTone(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralTone(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone", value))
    }
    /// Modulation.
    public static func generalMod(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_mod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralMod(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_mod", value))
    }
    /// CTCSS tone.
    public static func generalCtcss(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ctcss", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralCtcss(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ctcss", value))
    }
    /// CTCSS deviation.
    public static func generalCtcssDev(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ctcss_dev", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralCtcssDev(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ctcss_dev", value))
    }
    /// DCS code.
    public static func generalDcs(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_dcs", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralDcs(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_dcs", $0) })
    }
    /// The options of the setting "gen_audio" of the stack "general".
    public enum GeneralAudio: Int32 {
        case tone = 0
        case voice = 1
    }
    /// Sound.
    public static func generalAudio(_ handle: SDRKIT_HANDLE) -> Result<GeneralAudio, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_audio", &v)
        return code == SDRKIT_OK ? .success(GeneralAudio(rawValue: v) ?? GeneralAudio(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGeneralAudio(_ handle: SDRKIT_HANDLE, _ value: GeneralAudio) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_audio", value.rawValue))
    }
}

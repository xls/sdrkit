<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# General

AM (with synchronous detection), narrow and wide FM, USB, LSB and CW, with squelch, noise reduction and presets; transmits speech, Morse code and a tune carrier.

Stack id: `general`. Group: Listening.

The general receiver (AM with synchronous detection, NFM, WFM mono, USB, LSB and CW) with its transmit branch (push-to-talk speech, Morse code, a tune carrier), the listening chain that every audio stack shares, the tone identification and the band tables.

Signal types: AM voice, AM broadcast, Narrow FM voice, Keyed voice, Wide FM, SSB voice (upper), SSB voice (lower), CW (Morse).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 1.6 MHz |
| Offset from the signal | 250 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `general` | General | 0 Hz |
| `am` | AM broadcast | 0 Hz |
| `shortwave` | Shortwave | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:general` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `demod` | Demodulator | choice | `nfm` | `nfm` (NFM), `wfm` (WFM), `am` (AM), `usb` (USB), `lsb` (LSB), `cw` (CW) | NFM, WFM (mono), AM, USB, LSB or CW. |
| `width` | Filter | frequency | 0 Hz | 0 Hz to 220 kHz | The width of the channel filter. Each demodulator keeps its own. A width outside the range of the demodulator is brought into that range. |
| `channel_offset` | Channel offset | frequency | 0 Hz | -100 MHz to 100 MHz | Where the listening channel sits relative to the dial, inside the capture. |
| `step` | Step | frequency | 12.5 kHz | 1 Hz to 100 MHz | The tuning step. Each demodulator keeps its own. |
| `custom_filter` | Custom baseband filter | text | empty |  | Low-pass prototype taps, separated by commas, in place of the built-in channel filter. Empty for the built-in one. |
| `squelch` | Squelch | choice | `auto` | `off` (Off), `auto` (Auto), `level` (Level) | Auto opens 6 dB above the measured noise. Level opens at a level in dBFS. |
| `squelch_level` | Squelch level | number (dBFS) | -60 | -140 to 20 |  |
| `agc` | AGC | yes or no | true |  | AM follows the carrier. SSB and CW follow the envelope. |
| `gain` | Gain | number (dB) | 20 | -20 to 80 | Used when the AGC is off. |
| `deemph_nfm` | De-emphasis (NFM) | choice | `off` | `off` (Off), `50` (50 µs), `75` (75 µs), `750` (750 µs) |  |
| `deemph_wfm` | De-emphasis (WFM) | choice | `50` | `off` (Off), `50` (50 µs), `75` (75 µs), `750` (750 µs) |  |
| `nfm_lowpass` | Audio low-pass (NFM) | choice | `3khz` | `3khz` (3 kHz), `4khz` (4 kHz) | 3 kHz has about 3.7 dB less noise and keeps speech. 4 kHz passes more treble and more noise. |
| `cw_pitch` | Pitch | frequency | 700 Hz | 200 Hz to 1.5 kHz |  |
| `am_auto` | Auto quality | yes or no | true |  | Locks to the carrier, picks the cleaner sideband, removes whistles, hum and clicks, and levels the sound. |
| `am_detector` | Detector | choice | `auto` | `auto` (Auto), `envelope` (Envelope), `synchronous` (Synchronous), `upper` (Upper only), `lower` (Lower only) |  |
| `am_nr` | Noise | choice | `auto` | `auto` (Auto), `off` (Off), `gentle` (Gentle), `strong` (Strong) |  |
| `am_bandwidth` | Audio bandwidth | frequency | 0 Hz | 0 Hz to 10 kHz | A width is 2500 to 10 000 Hz. |
| `am_enhance` | Enhance (synthetic treble) | yes or no | false |  |  |
| `nb` | Blanker | yes or no | false |  | Removes clicks and ignition noise. Not offered for WFM. |
| `nb_threshold` | Blanker threshold | number (dB) | 12 | 6 to 30 |  |
| `af_highpass` | High-pass | frequency | 0 Hz | 0 Hz to 23.9 kHz |  |
| `af_lowpass` | Low-pass | frequency | 0 Hz | 0 Hz to 23.9 kHz |  |
| `af_notch` | Notch | frequency | 0 Hz | 0 Hz to 23.9 kHz |  |
| `voice_nr` | Noise remover (neural) | yes or no | false |  | For NFM, WFM and AM speech. Each demodulator keeps its own. |
| `voice_nr_strength` | Strength | number (%) | 80 | 0 to 100 |  |
| `presets` | Presets | text | empty |  | Entries separated by ";", each a key, "=", then the frequency in Hz, the demodulator, the filter width in Hz and the name separated by ",", for example "cw=7030000,cw,500,CW". At most 64 entries. The host fills it from a preset file. Empty: no presets. |
| `tone_gate` | Tone gate | text | `open` |  | open, any, tone:<Hz> or code:<023N>. |
| `listen_channel` | Heard channel | whole number | 0 | 0 to 3 | 0 for the main channel, 1 to 3 for an extra channel. |
| `extra1` | Extra channel 1 | yes or no | false |  |  |
| `extra1_offset` | Extra 1 offset | frequency | 100 kHz | -100 MHz to 100 MHz |  |
| `extra1_demod` | Extra 1 demodulator | choice | `nfm` | `nfm` (NFM), `wfm` (WFM), `am` (AM), `usb` (USB), `lsb` (LSB), `cw` (CW) |  |
| `extra1_width` | Extra 1 filter | frequency | 0 Hz | 0 Hz to 220 kHz |  |
| `extra2` | Extra channel 2 | yes or no | false |  |  |
| `extra2_offset` | Extra 2 offset | frequency | -100 kHz | -100 MHz to 100 MHz |  |
| `extra2_demod` | Extra 2 demodulator | choice | `nfm` | `nfm` (NFM), `wfm` (WFM), `am` (AM), `usb` (USB), `lsb` (LSB), `cw` (CW) |  |
| `extra2_width` | Extra 2 filter | frequency | 0 Hz | 0 Hz to 220 kHz |  |
| `extra3` | Extra channel 3 | yes or no | false |  |  |
| `extra3_offset` | Extra 3 offset | frequency | 200 kHz | -100 MHz to 100 MHz |  |
| `extra3_demod` | Extra 3 demodulator | choice | `nfm` | `nfm` (NFM), `wfm` (WFM), `am` (AM), `usb` (USB), `lsb` (LSB), `cw` (CW) |  |
| `extra3_width` | Extra 3 filter | frequency | 0 Hz | 0 Hz to 220 kHz |  |
| `tx_split` | Transmit offset | frequency | 0 Hz | -10 GHz to 10 GHz | The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex. |
| `tx_deviation` | Deviation (NFM) | frequency | 5 kHz | 1 kHz to 10 kHz | The peak deviation of NFM speech with its tone: 5000 Hz in 25 kHz channels, 2500 Hz in 12.5 kHz channels. |
| `tx_preemphasis` | Pre-emphasis (NFM) | choice | `off` | `off` (Off), `50` (50 µs), `75` (75 µs), `750` (750 µs) | Lifts the treble of NFM speech. A receiver with the same de-emphasis hears it flat. |
| `tx_ctcss` | CTCSS tone | frequency | 0 Hz | 0 Hz to 300 Hz | A sub-audio tone sent under NFM speech at 12 % of the deviation. |
| `tx_am_depth` | Modulation depth (AM) | number (%) | 90 | 10 to 100 | The largest modulation depth of AM. The carrier and both sidebands stay within full scale. |
| `tx_ssb_method` | Sideband method (SSB) | choice | `filter` | `filter` (Filter), `phasing` (Phasing) | Filter: a band-pass filter passes one sideband. Phasing: a Hilbert transformer cancels the other one. |
| `tx_ssb_band` | Audio band (SSB) | choice | `fixed` | `fixed` (300 to 2700 Hz), `filter` (As the filter) | 300 to 2700 Hz, or from 300 Hz over the width of the receive filter. |
| `tx_compression` | Compression (SSB) | number (dB) | 0 | 0 to 20 | Raises the average level of SSB speech by up to this much. The peaks stay at full scale. |
| `tx_timeout` | Time-out | duration (s) | 180 s | 1 to 3600 | Push-to-talk ends after this time on air. |
| `tx_cw_wpm` | Speed (CW) | number (WPM) | 20 | 5 to 60 | The speed of the characters that send keys. |
| `tx_cw_farnsworth` | Overall speed (CW) | number (WPM) | 0 | 0 to 60 | Farnsworth spacing: the characters at the speed, the gaps stretched to this lower overall speed. |
| `tx_tune_limit` | Tune time | duration (s) | 10 s | 1 to 120 | The tune carrier ends after this time. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_noise_only` | Noise only | yes or no | false |  | No station: only the noise of the test signal settings. |
| `gen_tune_offset` | Tune offset | frequency | 250 kHz | -10 MHz to 10 MHz |  |
| `gen_tone` | Tone | frequency | 1 kHz | 1 Hz to 20 kHz |  |
| `gen_mod` | Modulation | number | 0.5 | 0 to 1 |  |
| `gen_ctcss` | CTCSS tone | frequency | 0 Hz | 0 Hz to 300 Hz |  |
| `gen_ctcss_dev` | CTCSS deviation | frequency | 600 Hz | 0 Hz to 2 kHz |  |
| `gen_dcs` | DCS code | text | empty |  | Three octal digits, with N or I for the polarity (023N). Empty for none. |
| `gen_audio` | Sound | choice | `tone` | `tone` (Tone), `voice` (Voice) | Tone: the steady tone of the setting Tone. Voice: noise of the voice band in syllables of 4 Hz with pauses as long, at the modulation of the setting Modulation, which fills the channel as speech does. Voice applies to FM and AM; SSB and CW keep the tone. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `am_active` | AM processor | yes or no |
| `am_carrier_found` | AM carrier found | yes or no |
| `am_locked` | AM locked | yes or no |
| `am_carrier_offset_hz` | AM carrier offset | frequency |
| `am_carrier_dbfs` | AM carrier | number (dBFS) |
| `am_programme_snr_db` | AM programme SNR | number (dB) |
| `am_detector` | AM detector | choice |
| `am_one_side` | AM one sideband | yes or no |
| `am_interference_upper` | Interference above | number |
| `am_interference_lower` | Interference below | number |
| `am_bandwidth_hz` | AM audio bandwidth | frequency |
| `am_nr_floor_db` | AM noise floor | number (dB) |
| `am_blanked_pct` | AM blanked | number (%) |
| `am_blanker_db` | AM blanker threshold | number (dB) |
| `am_hum_hz` | Hum notched | frequency |
| `am_fading` | Fading | yes or no |
| `am_enhanced` | Synthetic treble | yes or no |
| `am_leveller_db` | Leveller gain | number (dB) |
| `am_status` | AM status | text |
| `demod` | Demodulator | choice |
| `width_hz` | Filter width | frequency |
| `input_rate_hz` | Input rate | frequency |
| `demod_rate_hz` | Demodulation rate | frequency |
| `channel_offset_hz` | Channel offset | frequency |
| `dial_hz` | Dial | frequency |
| `listening_hz` | Listening frequency | frequency |
| `filter_lo_hz` | Filter low edge | frequency |
| `filter_hi_hz` | Filter high edge | frequency |
| `passband_lo_hz` | Passband low | frequency |
| `passband_hi_hz` | Passband high | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `squelch_threshold_dbfs` | Squelch threshold | number (dBFS) |
| `squelch_open` | Squelch open | yes or no |
| `offset_hz` | Offset | frequency |
| `audio_dbfs` | Audio | number (dBFS) |
| `blocks_open` | Blocks open | whole number |
| `blocks_closed` | Blocks closed | whole number |
| `blanked_pct` | Blanked | number (%) |
| `speech_probability` | Speech probability | number |
| `needle` | S-meter needle | number |
| `ctcss_hz` | CTCSS tone | frequency |
| `ctcss_measured_hz` | CTCSS measured | frequency |
| `ctcss_db` | CTCSS level | number (dB) |
| `dcs` | DCS code | text |
| `dcs_alias` | DCS alias | text |
| `tone` | Tone | text |
| `tone_gate_open` | Tone gate open | yes or no |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_kind` | Sending | choice |
| `tx_time_s` | Time on air | duration (s) |
| `tx_mic_dbfs` | Microphone peak | number (dBFS) |
| `tx_peak_db` | Modulation peak | number (dB) |
| `tx_level_db` | Level gain | number (dB) |
| `tx_limit_db` | Limiter | number (dB) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `width_presets` | Width presets | `width_hz` (frequency) |
| `presets` | Presets | `name` (text), `freq_hz` (frequency), `demod` (choice), `width` (frequency), `squelch` (choice) |
| `channels` | Extra channels | `channel` (text), `freq_hz` (frequency), `demod` (choice), `squelch_open` (yes or no), `summary` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `whistles` | Whistles notched (number, frequency) |  | Hz |

## Events

| Kind | Label | Fields |
|---|---|---|
| `tones.subaudio` | CTCSS or DCS | `family` (choice), `text` (text), `kind` (choice), `event` (choice), `tone_hz` (frequency), `eia` (yes or no), `motorola` (text), `measured_hz` (frequency), `code` (text), `standard` (yes or no), `alias` (text), `level_db` (number (dB)) |
| `tones.dtmf` | DTMF key | `family` (choice), `text` (text), `key` (text), `down` (yes or no), `row_db` (number (dB)), `column_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `preset.apply` | Apply preset | `row` (text, required) |  | Sets the demodulator, the filter width and the squelch of a preset, and tunes to its frequency. |
| `ptt` | Push to talk | `on` (yes or no, required) |  | With on true, sends the speech of the microphone input in the mode of the demodulator (NFM, AM, USB or LSB) until ptt with on false, halt or the time-out. |
| `send` | Send Morse | `text` (text, required) |  | Keys the text in Morse code at the CW speed, with the demodulator on CW, the carrier at the listening frequency plus the transmit offset. |
| `tune` | Tune |  |  | Sends an unmodulated carrier at the listening frequency plus the transmit offset for the tune time. |
| `halt` | Halt |  |  | Ends speech, Morse code or the tune carrier at once. |

## Typed interface

The stack is in the family `general`, version 1.3. The C header `sdrkit/general.h`, the registry record `registry/general.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: General receiver (`general`). Media: sound (monitor).

### Result fields

Struct `AmFields`. The result fields of the AM processor. The measurements have no value while the processor does not run.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `am_active` | yes or no | always | 1.0 | Whether the AM processor runs. |
| `am_carrier_found` | yes or no | always | 1.0 | Whether it found a carrier. |
| `am_locked` | yes or no | always | 1.0 | Whether its loop is locked. |
| `am_carrier_offset_hz` | frequency | optional | 1.0 | The carrier relative to the channel. No value without a carrier. |
| `am_carrier_dbfs` | number (dBFS) | optional | 1.0 | The carrier level. |
| `am_programme_snr_db` | number (dB) | optional | 1.0 | The programme SNR. |
| `am_detector` | choice | optional | 1.0 | The detector in use. |
| `am_one_side` | yes or no | always | 1.0 | Whether one sideband was chosen because of interference. |
| `am_interference_upper` | number | optional | 1.0 | The interference measure above the carrier. |
| `am_interference_lower` | number | optional | 1.0 | The interference measure below the carrier. |
| `am_bandwidth_hz` | frequency | optional | 1.0 | The audio bandwidth in use. |
| `am_nr_floor_db` | number (dB) | optional | 1.0 | The noise reduction floor. No value with the noise reduction off. |
| `am_blanked_pct` | number (%) | optional | 1.0 | The share of samples the AM blanker removed. |
| `am_blanker_db` | number (dB) | optional | 1.0 | The threshold of the AM blanker. |
| `am_hum_hz` | frequency | optional | 1.0 | The hum notched (50 or 60 Hz). No value for none. |
| `am_fading` | yes or no | always | 1.0 | Whether the loop holds through a fade. |
| `am_enhanced` | yes or no | always | 1.0 | Whether the synthetic treble is on. |
| `am_leveller_db` | number (dB) | optional | 1.0 | The gain of the leveller. |
| `am_status` | text | optional | 1.0 | The state of the AM processor in words. |

### Result fields

Struct `GeneralFields`. The result fields of the report.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `demod` | choice | always | 1.0 | The demodulator. |
| `width_hz` | frequency | always | 1.0 | The filter width. |
| `input_rate_hz` | frequency | always | 1.0 | The input rate. |
| `demod_rate_hz` | frequency | always | 1.0 | The demodulation rate (48 kHz, WFM 240 kHz). |
| `channel_offset_hz` | frequency | always | 1.0 | The channel relative to the dial. |
| `dial_hz` | frequency | always | 1.0 | The dial frequency. |
| `listening_hz` | frequency | always | 1.0 | The listening frequency: the dial plus the channel offset. |
| `filter_lo_hz` | frequency | always | 1.0 | The low edge of the passband relative to the channel. |
| `filter_hi_hz` | frequency | always | 1.0 | The high edge of the passband relative to the channel. |
| `passband_lo_hz` | frequency | always | 1.0 | The low edge of the passband. |
| `passband_hi_hz` | frequency | always | 1.0 | The high edge of the passband. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level in the channel. No value before the first squelch block. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | The noise in the channel. No value while the noise is not measured. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR. No value while the noise is not measured. |
| `squelch_threshold_dbfs` | number (dBFS) | optional | 1.0 | The squelch threshold. No value with the squelch off or the noise not measured. |
| `squelch_open` | yes or no | always | 1.0 | Whether the squelch is open. |
| `offset_hz` | frequency | optional | 1.0 | The frequency offset of an FM signal. No value with the squelch closed or another demodulator. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the audio played over the last report. No value without audio. |
| `blocks_open` | whole number | optional | 1.0 | 10 ms blocks with the squelch open. No value before the first squelch block. |
| `blocks_closed` | whole number | optional | 1.0 | 10 ms blocks with the squelch closed. No value before the first squelch block. |
| `blanked_pct` | number (%) | always | 1.0 | The share of samples the blanker removed. |
| `speech_probability` | number | always | 1.0 | The speech probability of the voice noise remover for the last 10 ms (0 to 1). |
| `needle` | number | optional | 1.0 | The position of the needle of the S-meter (0 to 1). No value without a level. |
| `ctcss_hz` | frequency | optional | 1.0 | The CTCSS tone found. |
| `ctcss_measured_hz` | frequency | optional | 1.0 | The measured frequency of the CTCSS tone. |
| `ctcss_db` | number (dB) | optional | 1.0 | The level of the CTCSS tone against 5 kHz deviation. |
| `dcs` | text | optional | 1.0 | The DCS code found ("D023N"). |
| `dcs_alias` | text | optional | 1.0 | Another standard code that the same waveform reads as. |
| `tone` | text | optional | 1.0 | The CTCSS tone or DCS code heard, as a label for lists of things heard ("CTCSS 88.5 Hz", "DCS D023N"). |
| `tone_gate_open` | yes or no | always | 1.0 | Whether the tone gate lets the audio through. |

### Result fields

Struct `TransmitFields`. The result fields of the transmitter. The measurements have no value while the stack does not transmit.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.1 | Whether the stack transmits now. The receive audio is muted meanwhile. |
| `tx_freq_hz` | frequency | optional | 1.1 | The transmit frequency: the listening frequency plus the transmit offset. |
| `tx_kind` | choice | optional | 1.1 | What the transmission sends. |
| `tx_time_s` | duration (s) | optional | 1.1 | The time on air of the transmission. |
| `tx_mic_dbfs` | number (dBFS) | optional | 1.1 | The peak of the microphone audio over the last report. No value without voice. |
| `tx_peak_db` | number (dB) | optional | 1.1 | The peak of the modulation over the last report, against full modulation (deviation, depth or full scale). |
| `tx_level_db` | number (dB) | optional | 1.1 | The gain of the level control of the speech processor. |
| `tx_limit_db` | number (dB) | optional | 1.1 | The most gain reduction of the limiter and the compressor over the last report: 0 when they did not act. |

### Table `width_presets`

Struct `WidthPresetRow`. A row of the width presets. The row key is the place of the preset ("00", "01" ...).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `width_hz` | frequency | always | 1.0 | The width. |

### Table `presets`

Struct `PresetRow`. A row of the built-in presets. The row key is the key of the preset.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the preset. |
| `freq_hz` | frequency | always | 1.0 | The frequency of the preset. |
| `demod` | choice | always | 1.0 | The demodulator of the preset. |
| `width` | frequency | always | 1.0 | The filter width of the preset. |
| `squelch` | choice | always | 1.0 | The squelch of the preset. |

The column `name` names a row.

### Table `channels`

Struct `ChannelRow`. A row of the extra channels. The row key is the number of the channel ("1" to "3").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | text | always | 1.0 | The name of the channel. |
| `freq_hz` | frequency | optional | 1.0 | The listening frequency of the channel. |
| `demod` | choice | optional | 1.0 | The demodulator of the channel. |
| `squelch_open` | yes or no | optional | 1.0 | Whether the squelch of the channel is open. |
| `summary` | text | always | 1.0 | The summary line of the channel. |

### Event `tones.subaudio`

Struct `SubAudioFields`. The fields of a `tones.subaudio` event: a change of a CTCSS tone or of a DCS code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `text` | text | always | 1.0 | One line for people. |
| `kind` | choice | always | 1.0 | The signalling. |
| `event` | choice | always | 1.0 | The change. |
| `tone_hz` | frequency | optional | 1.0 | The CTCSS tone. |
| `eia` | yes or no | optional | 1.0 | The CTCSS tone is an EIA tone. |
| `motorola` | text | optional | 1.0 | The Motorola code of the CTCSS tone. |
| `measured_hz` | frequency | optional | 1.0 | The measured CTCSS tone. |
| `code` | text | optional | 1.0 | The DCS code with its polarity ("D023N"). |
| `standard` | yes or no | optional | 1.0 | The DCS code is a standard code. |
| `alias` | text | optional | 1.0 | A standard code that the same word also reads as. |
| `level_db` | number (dB) | always | 1.0 | The level of the tone, dB relative to a full-scale sine. |

### Event `tones.dtmf`

Struct `DtmfFields`. The fields of a `tones.dtmf` event: a DTMF key that went down or up.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `text` | text | always | 1.0 | One line for people: the key. |
| `key` | text | always | 1.0 | The DTMF key. |
| `down` | yes or no | always | 1.0 | The key went down (or up). |
| `row_db` | number (dB) | always | 1.0 | The level of the row tone. |
| `column_db` | number (dB) | always | 1.0 | The level of the column tone. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `preset.apply` | `sdrkit_general_preset_apply` | `sdrkit_general_preset_apply_args` | none |
| `ptt` | `sdrkit_general_ptt` | `sdrkit_general_ptt_args` | none |
| `send` | `sdrkit_general_send` | `sdrkit_general_send_args` | none |
| `tune` | `sdrkit_general_tune` | none | none |
| `halt` | `sdrkit_general_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `demod` | `sdrkit_general_get_demod` | `sdrkit_general_set_demod` |
| `width` | `sdrkit_general_get_width` | `sdrkit_general_set_width` |
| `channel_offset` | `sdrkit_general_get_channel_offset` | `sdrkit_general_set_channel_offset` |
| `step` | `sdrkit_general_get_step` | `sdrkit_general_set_step` |
| `custom_filter` | `sdrkit_general_get_custom_filter` | `sdrkit_general_set_custom_filter` |
| `squelch` | `sdrkit_general_get_squelch` | `sdrkit_general_set_squelch` |
| `squelch_level` | `sdrkit_general_get_squelch_level` | `sdrkit_general_set_squelch_level` |
| `agc` | `sdrkit_general_get_agc` | `sdrkit_general_set_agc` |
| `gain` | `sdrkit_general_get_gain` | `sdrkit_general_set_gain` |
| `deemph_nfm` | `sdrkit_general_get_deemph_nfm` | `sdrkit_general_set_deemph_nfm` |
| `deemph_wfm` | `sdrkit_general_get_deemph_wfm` | `sdrkit_general_set_deemph_wfm` |
| `nfm_lowpass` | `sdrkit_general_get_nfm_lowpass` | `sdrkit_general_set_nfm_lowpass` |
| `cw_pitch` | `sdrkit_general_get_cw_pitch` | `sdrkit_general_set_cw_pitch` |
| `am_auto` | `sdrkit_general_get_am_auto` | `sdrkit_general_set_am_auto` |
| `am_detector` | `sdrkit_general_get_am_detector` | `sdrkit_general_set_am_detector` |
| `am_nr` | `sdrkit_general_get_am_nr` | `sdrkit_general_set_am_nr` |
| `am_bandwidth` | `sdrkit_general_get_am_bandwidth` | `sdrkit_general_set_am_bandwidth` |
| `am_enhance` | `sdrkit_general_get_am_enhance` | `sdrkit_general_set_am_enhance` |
| `nb` | `sdrkit_general_get_nb` | `sdrkit_general_set_nb` |
| `nb_threshold` | `sdrkit_general_get_nb_threshold` | `sdrkit_general_set_nb_threshold` |
| `af_highpass` | `sdrkit_general_get_af_highpass` | `sdrkit_general_set_af_highpass` |
| `af_lowpass` | `sdrkit_general_get_af_lowpass` | `sdrkit_general_set_af_lowpass` |
| `af_notch` | `sdrkit_general_get_af_notch` | `sdrkit_general_set_af_notch` |
| `voice_nr` | `sdrkit_general_get_voice_nr` | `sdrkit_general_set_voice_nr` |
| `voice_nr_strength` | `sdrkit_general_get_voice_nr_strength` | `sdrkit_general_set_voice_nr_strength` |
| `presets` | `sdrkit_general_get_presets` | `sdrkit_general_set_presets` |
| `tone_gate` | `sdrkit_general_get_tone_gate` | `sdrkit_general_set_tone_gate` |
| `listen_channel` | `sdrkit_general_get_listen_channel` | `sdrkit_general_set_listen_channel` |
| `extra1` | `sdrkit_general_get_extra1` | `sdrkit_general_set_extra1` |
| `extra1_offset` | `sdrkit_general_get_extra1_offset` | `sdrkit_general_set_extra1_offset` |
| `extra1_demod` | `sdrkit_general_get_extra1_demod` | `sdrkit_general_set_extra1_demod` |
| `extra1_width` | `sdrkit_general_get_extra1_width` | `sdrkit_general_set_extra1_width` |
| `extra2` | `sdrkit_general_get_extra2` | `sdrkit_general_set_extra2` |
| `extra2_offset` | `sdrkit_general_get_extra2_offset` | `sdrkit_general_set_extra2_offset` |
| `extra2_demod` | `sdrkit_general_get_extra2_demod` | `sdrkit_general_set_extra2_demod` |
| `extra2_width` | `sdrkit_general_get_extra2_width` | `sdrkit_general_set_extra2_width` |
| `extra3` | `sdrkit_general_get_extra3` | `sdrkit_general_set_extra3` |
| `extra3_offset` | `sdrkit_general_get_extra3_offset` | `sdrkit_general_set_extra3_offset` |
| `extra3_demod` | `sdrkit_general_get_extra3_demod` | `sdrkit_general_set_extra3_demod` |
| `extra3_width` | `sdrkit_general_get_extra3_width` | `sdrkit_general_set_extra3_width` |
| `tx_split` | `sdrkit_general_get_tx_split` | `sdrkit_general_set_tx_split` |
| `tx_deviation` | `sdrkit_general_get_tx_deviation` | `sdrkit_general_set_tx_deviation` |
| `tx_preemphasis` | `sdrkit_general_get_tx_preemphasis` | `sdrkit_general_set_tx_preemphasis` |
| `tx_ctcss` | `sdrkit_general_get_tx_ctcss` | `sdrkit_general_set_tx_ctcss` |
| `tx_am_depth` | `sdrkit_general_get_tx_am_depth` | `sdrkit_general_set_tx_am_depth` |
| `tx_ssb_method` | `sdrkit_general_get_tx_ssb_method` | `sdrkit_general_set_tx_ssb_method` |
| `tx_ssb_band` | `sdrkit_general_get_tx_ssb_band` | `sdrkit_general_set_tx_ssb_band` |
| `tx_compression` | `sdrkit_general_get_tx_compression` | `sdrkit_general_set_tx_compression` |
| `tx_timeout` | `sdrkit_general_get_tx_timeout` | `sdrkit_general_set_tx_timeout` |
| `tx_cw_wpm` | `sdrkit_general_get_tx_cw_wpm` | `sdrkit_general_set_tx_cw_wpm` |
| `tx_cw_farnsworth` | `sdrkit_general_get_tx_cw_farnsworth` | `sdrkit_general_set_tx_cw_farnsworth` |
| `tx_tune_limit` | `sdrkit_general_get_tx_tune_limit` | `sdrkit_general_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_noise_only` | `sdrkit_general_gen_get_noise_only` | `sdrkit_general_gen_set_noise_only` |
| `gen_tune_offset` | `sdrkit_general_gen_get_tune_offset` | `sdrkit_general_gen_set_tune_offset` |
| `gen_tone` | `sdrkit_general_gen_get_tone` | `sdrkit_general_gen_set_tone` |
| `gen_mod` | `sdrkit_general_gen_get_mod` | `sdrkit_general_gen_set_mod` |
| `gen_ctcss` | `sdrkit_general_gen_get_ctcss` | `sdrkit_general_gen_set_ctcss` |
| `gen_ctcss_dev` | `sdrkit_general_gen_get_ctcss_dev` | `sdrkit_general_gen_set_ctcss_dev` |
| `gen_dcs` | `sdrkit_general_gen_get_dcs` | `sdrkit_general_gen_set_dcs` |
| `gen_audio` | `sdrkit_general_gen_get_audio` | `sdrkit_general_gen_set_audio` |

## Try it

```sh
sdrkit run general --source synth:general
sdrkit run general --source synth:general --submode general
sdrkit run general --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/general/general_am_250k.sigmf-meta`
- `tests/vectors/general/general_cw_250k.sigmf-meta`
- `tests/vectors/general/general_nfm_ctcss_250k.sigmf-meta`
- `tests/vectors/general/general_usb_250k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack transmits in the mode of the demodulator, at the listening frequency plus the setting `tx_split`. The transmit
stream has the rate of the session; its samples carry the signal at 0 Hz, and its centre is the transmit frequency. With
CW and the tune carrier the carrier is at that frequency; the receive side hears a carrier at the channel centre at its
pitch. WFM is not offered for transmission.

- `ptt` with `on` true sends speech in NFM, AM, USB or LSB until `ptt` with `on` false, `halt` or the time-out
  (`tx_timeout`). The speech processor has a high-pass at 250 Hz, the audio band of the mode (NFM 3 kHz, AM half the
  filter width, SSB 300 to 2700 Hz or the filter), a slow level control to an RMS of 0.3 of full modulation (within 20 dB),
  a look-ahead limiter without gain steps, and for SSB an optional compressor (`tx_compression`). NFM adds the optional
  pre-emphasis and CTCSS tone (12 % of the deviation); AM keeps the carrier and both sidebands within full scale at the
  depth limit; SSB uses a band-pass filter or the phasing method and limits the complex envelope.
- `send` keys a text in Morse code in CW at `tx_cw_wpm`, with Farnsworth spacing at `tx_cw_farnsworth`.
- `tune` sends a carrier for `tx_tune_limit` seconds.
- `halt` ends any of them.

Every transmission rises and falls with a 5 ms raised cosine and ends with an end mark. The peak magnitude is 0.9 of full
scale. While the stack transmits, and for 0.1 s after the end mark, the receive audio is muted. A change of the format
(a retune, a new rate), a change of the demodulator or of the channel offset, and the transmitter switching off end a
transmission. The other transmit settings apply to the next transmission.

The speech comes in at the microphone input of the component `general.transmit` (`mic`, mono audio; 48 kHz is used as it
is, another rate is converted). The engine has no audio source for it yet, so in a session this input is open and
push-to-talk sends silence. A Rust host gives the stack a source with `GeneralReceiver::with_mic`.

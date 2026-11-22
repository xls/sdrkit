<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FT8

The 15-second weak-signal mode of HF and 6 m contacts.

Stack id: `ft8`. Group: Weak-signal digital.

Signal types: FT8.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 3.5 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ft8` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `mirrored` | Upside down too | yes or no | true |  | Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR). |
| `deep` | Deep decoding | yes or no | true |  | A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band. |
| `early_decode` | Early decode | yes or no | true |  | FT8 and FT4: decode the slot once more when 85 % of the transmission is in, while the decoder is idle. Early decodes are shown at once and replaced by the decodes of the whole slot; they are not counted. |
| `demod` | Demodulator | choice | `usb` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) |  |
| `listen` | Listen | yes or no | false |  | Copies the 8 kHz audio to the speaker. Decoding is the same either way. |
| `time_source` | Slot clock | choice | `system` | `system` (System clock), `start` (Recording start), `search` (Slot search) |  |
| `time_start` | Recording start (UTC) | text | empty |  | An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given. |
| `band_edges` | Band edges | text | empty |  | The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band. |
| `dials` | Dials | text | empty |  | The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by ";", for example "ft8=14074000". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial. |
| `wide` | Decode the whole span | yes or no | false |  |  |
| `wide_span` | Span | frequency | 250 kHz | 1 kHz to 100 MHz |  |
| `multiband` | Decode all selected at once | yes or no | false |  |  |
| `multiband_select` | Bands and modes | whole number | 0 | 0 to 17592186044415 | The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR. |
| `hop.enabled` | Band hopping | yes or no | false |  | At the start of each 2-minute slot, asks the host to tune to the dial of the band of the plan. Off while the whole span is decoded. |
| `hop.coordinated` | Coordinated hopping | yes or no | true |  | Every station takes the same band of the cycle in the same slot. |
| `hop.hours` | Bands of each hour | text | `0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0` |  | 24 hexadecimal masks separated by commas, UTC hour 0 first. Bit b of a mask is band b of the band list. |
| `spots` | Spot events | yes or no | false |  | Makes a spot event of every decode with a calling station, for the host to forward. |
| `qso_even` | Transmit in even slots | yes or no | true |  |  |
| `qso_auto` | Auto-sequence | yes or no | true |  |  |
| `qso_hold` | Hold TX frequency | yes or no | false |  |  |
| `qso_hz` | TX audio frequency | frequency | 1.5 kHz | 200 Hz to 3.5 kHz |  |
| `qso_retries` | Retry limit | whole number | 4 | 1 to 20 |  |
| `qso_watchdog` | Watchdog | duration (s) | 360 s | 30 to 3600 |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `hop.bands` | Band list | text | empty |  | The band names in bit order, separated by commas (for example from data/wsjt/band_hopping.csv). Empty: no band. |
| `hop.cycle` | Coordinated cycle | text | empty |  | The bands of the coordinated cycle in their order, by the names of the band list. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mirrored` | Test signal upside down | yes or no | false |  |  |
| `gen_live` | Test signal in step with the clock | yes or no | false |  |  |
| `gen_single` | Single station message | text | empty |  |  |
| `gen_hz` | Single station frequency | frequency | 1.5 kHz | 200 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `time_source` | Slot clock | choice |
| `utc` | UTC | time |
| `clock_error_s` | Clock error | number (s) |
| `clock_spread_s` | Clock spread | number (s) |
| `clock_decodes` | Clock decodes | whole number |
| `clock_warning` | Clock warning | yes or no |
| `decodes_total` | Decodes | whole number |
| `js8_frames` | JS8 frames | whole number |
| `js8_failed` | JS8 candidates failed | whole number |
| `js8_state` | JS8 | choice |
| `js8_waterfall_seq` | Waterfall line | whole number |
| `dec_ft8.slots` | Slots decoded | whole number |
| `dec_ft8.decodes` | Decodes | whole number |
| `dec_ft8.decode_ms` | Decoding time | number (ms) |
| `dec_ft8.dropped` | Dropped | whole number |
| `wide.channels` | Dials decoded | whole number |
| `wide.load_pct` | Decoding load | number (%) |
| `wide.too_slow` | Too slow | yes or no |
| `hop.band` | Hop band | text |
| `hop.next_utc` | Next band change | time |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `decodes` | Decodes | `utc` (time), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `band` (text), `dial_hz` (frequency), `mode` (text), `message` (text), `mirrored` (yes or no), `how` (choice), `early` (yes or no) |
| `wspr_spots` | WSPR spots | `utc` (time), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `callsign` (text), `locator` (text), `power_dbm` (whole number (dBm)), `message` (text) |
| `modes` | Modes | `mode` (text), `period` (duration (s)), `slots` (whole number), `decodes` (whole number), `last_count` (whole number), `utc` (time), `decode_ms` (number (ms)), `state` (choice) |
| `stations` | Stations | `callsign` (text), `locator` (text), `mode` (choice), `snr_db` (number (dB)), `freq_hz` (frequency), `heard` (whole number), `last_heard` (time) |
| `js8_frames` | JS8 frames | `utc` (time), `submode` (choice), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `frame` (text) |
| `js8_messages` | JS8 messages | `utc` (time), `snr_db` (number (dB)), `audio_hz` (frequency), `message` (text) |
| `js8_stations` | JS8 stations | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `audio_hz` (frequency), `utc` (time), `last_text` (text), `last_heard` (time) |
| `wide.multiband` | Multi-band monitor | `band` (text), `mode` (choice), `dial_hz` (frequency), `running` (yes or no), `decodes` (whole number), `covered` (yes or no), `span_needed_hz` (frequency) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `js8_waterfall` | JS8 waterfall | Hz | dB |

## Map points

The stack puts points on a map: Stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `wsjt.decode` | Decode | `mode` (choice), `text` (text), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `callsign` (text), `locator` (text), `cq` (yes or no), `mirrored` (yes or no), `how` (choice), `power_dbm` (whole number (dBm)), `source` (text), `position` (position), `family` (choice) |
| `wsjt.spot` | Station heard | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `source` (text), `family` (choice) |
| `js8.frame` | JS8 frame | `text` (text), `chars` (text), `snr_db` (number (dB)), `submode` (choice), `source` (text), `target` (text), `family` (choice) |
| `js8.message` | JS8 message | `text` (text), `complete` (yes or no), `source` (text), `target` (text), `family` (choice) |
| `wsjt.tx` | Message to send | `mode` (choice), `text` (text), `audio_hz` (frequency), `slot` (time), `period` (duration (s)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `cq` | Call CQ |  |  | Starts calling CQ in the next transmit slot. |
| `halt` | Halt |  |  | Stops transmitting at once. |

## Typed interface

The stack is in the family `wsjt`, version 4.0. The C header `sdrkit/wsjt.h`, the registry record `registry/wsjt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `WsjtFields`. The result fields of a slot stack: the slot clock, the clock estimate from the decodes, and the counts since Clear.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_source` | choice | optional | 1.0 | The time source of the slot clock. It has no value before the clock reports. |
| `utc` | time | always | 1.0 | The UTC of the newest audio. |
| `clock_error_s` | number (s) | optional | 1.0 | The median DT of the recent decodes. It has no value without a recent decode. |
| `clock_spread_s` | number (s) | optional | 1.0 | The spread of the recent DTs. It has no value without a recent decode. |
| `clock_decodes` | whole number | always | 1.0 | The decodes that the clock estimate uses. |
| `clock_warning` | yes or no | always | 1.0 | The clock is more than 1 s off. |
| `decodes_total` | whole number | always | 1.0 | Decodes since Clear. |
| `js8_frames` | whole number | always | 1.0 | JS8 frames since Clear. |
| `js8_failed` | whole number | always | 1.0 | JS8 candidates that did not decode since Clear. |
| `js8_state` | choice | always | 1.0 | What JS8 has heard. |
| `js8_waterfall_seq` | whole number | optional | 1.0 | The sequence number of the newest line of the JS8 waterfall. It has no value before the first line. |

### Table `decodes`

Struct `DecodeRow`. A row of the decode table. The row key is the place of the decode in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the slot. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `dt_s` | number (s) | always | 1.0 | The time offset from the slot start. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `band` | text | optional | 1.0 | The amateur band of the dial, if the dial is in one. |
| `dial_hz` | frequency | optional | 1.0 | The dial frequency, if the slot has one. |
| `mode` | text | always | 1.0 | The mode with its sub-mode and period ("JT65B"). |
| `message` | text | always | 1.0 | The decoded message. |
| `mirrored` | yes or no | always | 1.0 | The decode was found on the other sideband. |
| `how` | choice | always | 1.0 | How the decode was made. |
| `early` | yes or no | always | 1.0 | The decode is from the first part of a slot. The decode of the whole slot replaces it. |

### Table `wspr_spots`

Struct `WsprSpotRow`. A row of the WSPR and FST4W spots. The row key is the place of the spot in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the slot. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `dt_s` | number (s) | always | 1.0 | The time offset from the slot start. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `callsign` | text | always | 1.0 | The callsign of the beacon. |
| `locator` | text | always | 1.0 | The locator of the beacon. |
| `power_dbm` | whole number (dBm) | always | 1.0 | The power that the beacon states. |
| `message` | text | always | 1.0 | The decoded message. |

### Table `modes`

Struct `ModeRow`. A row of the modes table: the counters and the state of one mode heard, or of one JS8 speed. The row key is the mode id, or `js8n`, `js8f` and `js8t`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | text | always | 1.0 | The mode with its sub-mode and period. |
| `period` | duration (s) | always | 1.0 | The T/R period. |
| `slots` | whole number | always | 1.0 | The slots decoded. |
| `decodes` | whole number | always | 1.0 | Decodes since Clear. |
| `last_count` | whole number | always | 1.0 | The decodes of the last slot. |
| `utc` | time | optional | 1.0 | The UTC of the last slot decoded. It has no value before the first slot. |
| `decode_ms` | number (ms) | always | 1.0 | The time that the last slot took to decode. |
| `state` | choice | always | 1.0 | Whether the mode decodes. |

### Table `stations`

Struct `StationRow`. A row of the station table: a station heard in any mode. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `mode` | choice | always | 1.0 | The mode of the newest decode. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the newest decode. |
| `freq_hz` | frequency | always | 1.0 | The radio frequency of the newest decode. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last decode. |

### Table `js8_frames`

Struct `Js8FrameRow`. A row of the JS8 frames. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the slot. |
| `submode` | choice | optional | 1.0 | The speed of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `dt_s` | number (s) | always | 1.0 | The time offset from the slot start. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `frame` | text | always | 1.0 | The text of the frame. |

### Table `js8_messages`

Struct `Js8MessageRow`. A row of the JS8 messages. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the first frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `message` | text | always | 1.0 | The message. |

### Table `js8_stations`

Struct `Js8StationRow`. A row of the JS8 stations. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the newest frame. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency of the newest frame. |
| `utc` | time | always | 1.0 | The UTC of the newest frame. |
| `last_text` | text | always | 1.0 | The newest text of the station. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

### Table `wide.multiband`

Struct `MultibandRow`. A row of the multi-band monitor: one band and mode of the selection. The row key is the band index in two digits and the mode index.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | text | always | 1.0 | The band. |
| `mode` | choice | always | 1.0 | The mode. |
| `dial_hz` | frequency | optional | 1.0 | The dial of the mode on the band, if it has one. |
| `running` | yes or no | always | 1.0 | A decoder runs on the dial. |
| `decodes` | whole number | always | 1.0 | The decodes of the dial. |
| `covered` | yes or no | always | 1.0 | The capture covers the dial. |
| `span_needed_hz` | frequency | optional | 1.0 | The span that the capture needs around its centre to cover the dial. It has no value for a covered dial. |

### Event `wsjt.decode`

Struct `DecodeEvent`. The fields of a decode of any mode but JS8.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | choice | always | 1.0 | The mode. |
| `text` | text | always | 1.0 | The decoded message. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `dt_s` | number (s) | always | 1.0 | The time offset from the slot start. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `callsign` | text | optional | 1.0 | The calling station, if the message names one. |
| `locator` | text | optional | 1.0 | The locator, if the message holds one. |
| `cq` | yes or no | always | 1.0 | The message is a CQ. |
| `mirrored` | yes or no | always | 1.0 | The decode was found on the other sideband. |
| `how` | choice | always | 1.0 | How the decode was made. |
| `power_dbm` | whole number (dBm) | optional | 1.0 | The power that a WSPR or FST4W station states. |
| `source` | text | optional | 1.0 | The calling station, if the message names one. |
| `position` | position | optional | 1.0 | The centre of the locator, if the message holds one. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `wsjt.spot`

Struct `SpotEvent`. The fields of a station heard, with the standard spot fields.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `freq_hz` | frequency | always | 1.0 | The radio frequency. |
| `mode` | text | always | 1.0 | The emission as spot networks name it. |
| `source` | text | always | 1.0 | The callsign. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `js8.frame`

Struct `Js8FrameEvent`. The fields of a JS8 frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the frame. |
| `chars` | text | always | 1.0 | The twelve characters of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `submode` | choice | optional | 1.0 | The speed of the frame. |
| `source` | text | optional | 1.0 | The sending station, if the frame names one. |
| `target` | text | optional | 1.0 | The station or group addressed, if the frame names one. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `js8.message`

Struct `Js8MessageEvent`. The fields of a JS8 message, sent when its last frame came or when it stopped.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The message. |
| `complete` | yes or no | always | 1.0 | The last frame of the message came. |
| `source` | text | optional | 1.0 | The sending station, if it is known. |
| `target` | text | optional | 1.0 | The station or group addressed, if it is known. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `wsjt.tx`

Struct `TxEvent`. The fields of a message to send.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | choice | always | 1.0 | The mode. |
| `text` | text | always | 1.0 | The message. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `slot` | time | always | 1.0 | The UTC of the start of the transmit slot. |
| `period` | duration (s) | always | 1.0 | The T/R period. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ft8_clear` | none | none |
| `remove` | `sdrkit_ft8_remove` | `sdrkit_wsjt_station_args` | none |
| `cq` | `sdrkit_ft8_cq` | none | none |
| `halt` | `sdrkit_ft8_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `mirrored` | `sdrkit_ft8_get_mirrored` | `sdrkit_ft8_set_mirrored` |
| `deep` | `sdrkit_ft8_get_deep` | `sdrkit_ft8_set_deep` |
| `early_decode` | `sdrkit_ft8_get_early_decode` | `sdrkit_ft8_set_early_decode` |
| `demod` | `sdrkit_ft8_get_demod` | `sdrkit_ft8_set_demod` |
| `listen` | `sdrkit_ft8_get_listen` | `sdrkit_ft8_set_listen` |
| `time_source` | `sdrkit_ft8_get_time_source` | `sdrkit_ft8_set_time_source` |
| `time_start` | `sdrkit_ft8_get_time_start` | `sdrkit_ft8_set_time_start` |
| `band_edges` | `sdrkit_ft8_get_band_edges` | `sdrkit_ft8_set_band_edges` |
| `dials` | `sdrkit_ft8_get_dials` | `sdrkit_ft8_set_dials` |
| `wide` | `sdrkit_ft8_get_wide` | `sdrkit_ft8_set_wide` |
| `wide_span` | `sdrkit_ft8_get_wide_span` | `sdrkit_ft8_set_wide_span` |
| `multiband` | `sdrkit_ft8_get_multiband` | `sdrkit_ft8_set_multiband` |
| `multiband_select` | `sdrkit_ft8_get_multiband_select` | `sdrkit_ft8_set_multiband_select` |
| `hop.enabled` | `sdrkit_ft8_get_hop_enabled` | `sdrkit_ft8_set_hop_enabled` |
| `hop.coordinated` | `sdrkit_ft8_get_hop_coordinated` | `sdrkit_ft8_set_hop_coordinated` |
| `hop.hours` | `sdrkit_ft8_get_hop_hours` | `sdrkit_ft8_set_hop_hours` |
| `hop.bands` | `sdrkit_ft8_get_hop_bands` | `sdrkit_ft8_set_hop_bands` |
| `hop.cycle` | `sdrkit_ft8_get_hop_cycle` | `sdrkit_ft8_set_hop_cycle` |
| `spots` | `sdrkit_ft8_get_spots` | `sdrkit_ft8_set_spots` |
| `qso_even` | `sdrkit_ft8_get_qso_even` | `sdrkit_ft8_set_qso_even` |
| `qso_auto` | `sdrkit_ft8_get_qso_auto` | `sdrkit_ft8_set_qso_auto` |
| `qso_hold` | `sdrkit_ft8_get_qso_hold` | `sdrkit_ft8_set_qso_hold` |
| `qso_hz` | `sdrkit_ft8_get_qso_hz` | `sdrkit_ft8_set_qso_hz` |
| `qso_retries` | `sdrkit_ft8_get_qso_retries` | `sdrkit_ft8_set_qso_retries` |
| `qso_watchdog` | `sdrkit_ft8_get_qso_watchdog` | `sdrkit_ft8_set_qso_watchdog` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mirrored` | `sdrkit_ft8_gen_get_mirrored` | `sdrkit_ft8_gen_set_mirrored` |
| `gen_live` | `sdrkit_ft8_gen_get_live` | `sdrkit_ft8_gen_set_live` |
| `gen_single` | `sdrkit_ft8_gen_get_single` | `sdrkit_ft8_gen_set_single` |
| `gen_hz` | `sdrkit_ft8_gen_get_hz` | `sdrkit_ft8_gen_set_hz` |

## Try it

```sh
sdrkit run ft8 --source synth:ft8
sdrkit run ft8 --source <driver>:<address> --freq 0
```
<!-- generated: end -->

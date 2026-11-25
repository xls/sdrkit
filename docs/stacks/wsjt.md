<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Weak-signal (all enabled)

Every enabled weak-signal decoder on the same audio: FT8, FT4, FT2, WSPR, JT65, JT9, JT4, Q65, FST4, FST4W, MSK144 and JS8.

Stack id: `wsjt`. Group: Weak-signal digital.

Signal types: FT8, FT4, FT2, WSPR, JT65, JT9, JT4, Q65, FST4, FST4W, MSK144, JS8.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wsjt` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `enable_ft8` | Decode FT8 (15 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_ft4` | Decode FT4 (7.5 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_ft2` | Decode FT2 (3.75 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_wspr` | Decode WSPR (120 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_jt65` | Decode JT65 (60 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_jt9` | Decode JT9 (60 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_jt4` | Decode JT4 (60 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_q65` | Decode Q65 (60 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_fst4` | Decode FST4 (60 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_fst4w` | Decode FST4W (120 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `enable_msk144` | Decode MSK144 (15 s slots) | yes or no | true |  | Takes effect at the next slot. |
| `jt65_submode` | JT65 sub-mode | choice | `A` | `A` (A (2.7 Hz)), `B` (B (5.4 Hz)), `C` (C (10.8 Hz)) |  |
| `jt4_submode` | JT4 sub-mode | choice | `A` | `A` (A (4.4 Hz)), `B` (B (8.75 Hz)), `C` (C (17.5 Hz)), `D` (D (35 Hz)), `E` (E (70 Hz)), `F` (F (140 Hz)), `G` (G (280 Hz)) |  |
| `q65_submode` | Q65 sub-mode | choice | `A` | `A`, `B`, `C`, `D`, `E` |  |
| `q65_period` | Q65 T/R period | duration (s) | 60 s | 15 to 300 | One of 15, 30, 60, 120, 300 s. |
| `fst4_period` | FST4 T/R period | duration (s) | 60 s | 15 to 1800 | One of 15, 30, 60, 120, 300, 900, 1800 s. |
| `fst4w_period` | FST4W T/R period | duration (s) | 120 s | 15 to 1800 | One of 15, 30, 60, 120, 300, 900, 1800 s. |
| `msk144_period` | MSK144 T/R period | duration (s) | 15 s | 15 to 30 | One of 15, 30 s. |
| `js8_normal` | JS8 Normal (15 s) | yes or no | true |  |  |
| `js8_fast` | JS8 Fast (10 s) | yes or no | true |  |  |
| `js8_turbo` | JS8 Turbo (6 s) | yes or no | true |  |  |
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
| `spots` | Spot events | yes or no | false |  | Makes a spot event of every decode with a calling station, for the host to forward. |
| `qso_mode` | Mode | choice | `ft8` | `ft8` (FT8), `ft4` (FT4), `ft2` (FT2), `jt65` (JT65), `jt9` (JT9), `jt4` (JT4) |  |
| `qso_even` | Transmit in even slots | yes or no | true |  |  |
| `qso_auto` | Auto-sequence | yes or no | true |  |  |
| `qso_hold` | Hold TX frequency | yes or no | false |  |  |
| `qso_hz` | TX audio frequency | frequency | 1.5 kHz | 200 Hz to 3.5 kHz |  |
| `qso_retries` | Retry limit | whole number | 4 | 1 to 20 |  |
| `qso_watchdog` | Watchdog | duration (s) | 360 s | 30 to 3600 |  |
| `beacon_dbm` | Power | whole number (dBm) | 10 | 0 to 60 | WSPR sends 0 to 60 dBm, ending in 0, 3 or 7. |
| `beacon_every` | One slot in N | whole number | 5 | 1 to 100 |  |
| `beacon_offset` | Offset | whole number | 0 | 0 to 99 |  |
| `beacon_hz` | Audio frequency | frequency | 1.5 kHz | 1.4 kHz to 1.6 kHz |  |
| `js8_speed` | Speed | choice | `normal` | `normal` (Normal), `fast` (Fast), `turbo` (Turbo) |  |
| `js8_hz` | Audio frequency | frequency | 1.5 kHz | 200 Hz to 3 kHz |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mirrored` | Test signal upside down | yes or no | false |  |  |
| `gen_live` | Test signal in step with the clock | yes or no | false |  |  |
| `gen_single` | Single station message | text | empty |  |  |
| `gen_hz` | Single station frequency | frequency | 1.5 kHz | 200 Hz to 3.5 kHz |  |
| `gen_short` | Test signal short messages | yes or no | false |  | Sends a single station message `<CALL1 CALL2> RPT`, whose report is -03, +00, +03, +06, +10, +13, +16, R-03 to R+16, RRR or 73, as the 40-bit short frame. |

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
| `dec_ft4.slots` | Slots decoded | whole number |
| `dec_ft4.decodes` | Decodes | whole number |
| `dec_ft4.decode_ms` | Decoding time | number (ms) |
| `dec_ft4.dropped` | Dropped | whole number |
| `dec_ft2.slots` | Slots decoded | whole number |
| `dec_ft2.decodes` | Decodes | whole number |
| `dec_ft2.decode_ms` | Decoding time | number (ms) |
| `dec_ft2.dropped` | Dropped | whole number |
| `dec_wspr.slots` | Slots decoded | whole number |
| `dec_wspr.decodes` | Decodes | whole number |
| `dec_wspr.decode_ms` | Decoding time | number (ms) |
| `dec_wspr.dropped` | Dropped | whole number |
| `dec_jt65.slots` | Slots decoded | whole number |
| `dec_jt65.decodes` | Decodes | whole number |
| `dec_jt65.decode_ms` | Decoding time | number (ms) |
| `dec_jt65.dropped` | Dropped | whole number |
| `dec_jt9.slots` | Slots decoded | whole number |
| `dec_jt9.decodes` | Decodes | whole number |
| `dec_jt9.decode_ms` | Decoding time | number (ms) |
| `dec_jt9.dropped` | Dropped | whole number |
| `dec_jt4.slots` | Slots decoded | whole number |
| `dec_jt4.decodes` | Decodes | whole number |
| `dec_jt4.decode_ms` | Decoding time | number (ms) |
| `dec_jt4.dropped` | Dropped | whole number |
| `dec_q65.slots` | Slots decoded | whole number |
| `dec_q65.decodes` | Decodes | whole number |
| `dec_q65.decode_ms` | Decoding time | number (ms) |
| `dec_q65.dropped` | Dropped | whole number |
| `dec_fst4.slots` | Slots decoded | whole number |
| `dec_fst4.decodes` | Decodes | whole number |
| `dec_fst4.decode_ms` | Decoding time | number (ms) |
| `dec_fst4.dropped` | Dropped | whole number |
| `dec_fst4w.slots` | Slots decoded | whole number |
| `dec_fst4w.decodes` | Decodes | whole number |
| `dec_fst4w.decode_ms` | Decoding time | number (ms) |
| `dec_fst4w.dropped` | Dropped | whole number |
| `dec_msk144.slots` | Slots decoded | whole number |
| `dec_msk144.decodes` | Decodes | whole number |
| `dec_msk144.decode_ms` | Decoding time | number (ms) |
| `dec_msk144.dropped` | Dropped | whole number |
| `dec_js8.slots` | Slots decoded | whole number |
| `dec_js8.decodes` | Decodes | whole number |
| `dec_js8.decode_ms` | Decoding time | number (ms) |
| `dec_js8.dropped` | Dropped | whole number |
| `wide.channels` | Dials decoded | whole number |
| `wide.load_pct` | Decoding load | number (%) |
| `wide.too_slow` | Too slow | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `decodes` | Decodes | `utc` (time), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `band` (text), `dial_hz` (frequency), `mode` (text), `message` (text), `mirrored` (yes or no), `how` (choice), `early` (yes or no) |
| `wspr_spots` | WSPR spots | `utc` (time), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `callsign` (text), `locator` (text), `power_dbm` (whole number (dBm)), `message` (text), `position` (position) |
| `modes` | Modes | `mode` (text), `period` (duration (s)), `slots` (whole number), `decodes` (whole number), `last_count` (whole number), `utc` (time), `decode_ms` (number (ms)), `state` (choice) |
| `stations` | Stations | `callsign` (text), `locator` (text), `mode` (choice), `snr_db` (number (dB)), `freq_hz` (frequency), `heard` (whole number), `last_heard` (time), `position` (position) |
| `js8_frames` | JS8 frames | `utc` (time), `submode` (choice), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `frame` (text) |
| `js8_messages` | JS8 messages | `utc` (time), `snr_db` (number (dB)), `audio_hz` (frequency), `message` (text) |
| `js8_stations` | JS8 stations | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `audio_hz` (frequency), `utc` (time), `last_text` (text), `last_heard` (time), `position` (position) |
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
| `js8.frame` | JS8 frame | `text` (text), `chars` (text), `snr_db` (number (dB)), `submode` (choice), `source` (text), `target` (text), `cq` (yes or no), `family` (choice) |
| `js8.message` | JS8 message | `text` (text), `complete` (yes or no), `source` (text), `target` (text), `cq` (yes or no), `family` (choice) |
| `wsjt.tx` | Message to send | `mode` (choice), `text` (text), `audio_hz` (frequency), `slot` (time), `period` (duration (s)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `cq` | Call CQ |  |  | Starts calling CQ in the next transmit slot. |
| `beacon` | Start the WSPR beacon |  |  | Sends the call, locator and power of the station in the beacon slots. |
| `send` | Send | `text` (text, required) |  | Queues a JS8 message. My call and locator come from the station. |
| `halt` | Halt |  |  | Stops transmitting at once. |

## Typed interface

The stack is in the family `wsjt`, version 5.2. The C header `sdrkit/wsjt.h`, the registry record `registry/wsjt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Weak-signal messages (`weak_signal`). Media: sound (monitor), text.

### Result fields

Struct `WsjtFields`. The result fields of a slot stack: the slot clock and the clock estimate from the decodes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_source` | choice | optional | 1.0 | The time source of the slot clock. It has no value before the clock reports. |
| `utc` | time | always | 1.0 | The UTC of the newest audio. |
| `clock_error_s` | number (s) | optional | 1.0 | The median DT of the recent decodes. It has no value without a recent decode. |
| `clock_spread_s` | number (s) | optional | 1.0 | The spread of the recent DTs. It has no value without a recent decode. |
| `clock_decodes` | whole number | always | 1.0 | The decodes that the clock estimate uses. |
| `clock_warning` | yes or no | always | 1.0 | The clock is more than 1 s off. |

### Result fields

Struct `WsjtDecodeFields`. The result field of a slot stack with a mode other than JS8: the count of the decodes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `decodes_total` | whole number | always | 1.0 | Decodes since Clear. |

### Result fields

Struct `Js8Fields`. The result fields of a slot stack with JS8.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
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

Default order: the column `utc`, highest first.

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
| `position` | position | optional | 1.0 | The centre of the locator, if the spot holds one. |

The column `callsign` names a row. Default order: the column `utc`, highest first. Place of a row: the column `position`.

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
| `position` | position | optional | 1.0 | The centre of the locator, if one was heard. |

The rows are the stations or nodes of the stack. The column `callsign` names a row. Default order: the column `last_heard`, highest first. Place of a row: the column `position`. Time: `last_heard`.

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

Default order: the column `utc`, highest first.

### Table `js8_messages`

Struct `Js8MessageRow`. A row of the JS8 messages. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the first frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `message` | text | always | 1.0 | The message. |

Default order: the column `utc`, highest first.

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
| `position` | position | optional | 1.0 | The centre of the locator, if one was heard. |

The column `callsign` names a row. Default order: the column `last_heard`, highest first. Place of a row: the column `position`.

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
| `cq` | yes or no | always | 1.0 | The frame is a CQ: a heartbeat to all with a CQ. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `js8.message`

Struct `Js8MessageEvent`. The fields of a JS8 message, sent when its last frame came or when it stopped.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The message. |
| `complete` | yes or no | always | 1.0 | The last frame of the message came. |
| `source` | text | optional | 1.0 | The sending station, if it is known. |
| `target` | text | optional | 1.0 | The station or group addressed, if it is known. |
| `cq` | yes or no | always | 1.0 | The message is a CQ. |
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
| `clear` | `sdrkit_wsjt_clear` | none | none |
| `remove` | `sdrkit_wsjt_remove` | `sdrkit_wsjt_station_args` | none |
| `cq` | `sdrkit_wsjt_cq` | none | none |
| `beacon` | `sdrkit_wsjt_beacon` | none | none |
| `send` | `sdrkit_wsjt_send` | `sdrkit_wsjt_send_args` | none |
| `halt` | `sdrkit_wsjt_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `enable_ft8` | `sdrkit_wsjt_get_enable_ft8` | `sdrkit_wsjt_set_enable_ft8` |
| `enable_ft4` | `sdrkit_wsjt_get_enable_ft4` | `sdrkit_wsjt_set_enable_ft4` |
| `enable_ft2` | `sdrkit_wsjt_get_enable_ft2` | `sdrkit_wsjt_set_enable_ft2` |
| `enable_wspr` | `sdrkit_wsjt_get_enable_wspr` | `sdrkit_wsjt_set_enable_wspr` |
| `enable_jt65` | `sdrkit_wsjt_get_enable_jt65` | `sdrkit_wsjt_set_enable_jt65` |
| `enable_jt9` | `sdrkit_wsjt_get_enable_jt9` | `sdrkit_wsjt_set_enable_jt9` |
| `enable_jt4` | `sdrkit_wsjt_get_enable_jt4` | `sdrkit_wsjt_set_enable_jt4` |
| `enable_q65` | `sdrkit_wsjt_get_enable_q65` | `sdrkit_wsjt_set_enable_q65` |
| `enable_fst4` | `sdrkit_wsjt_get_enable_fst4` | `sdrkit_wsjt_set_enable_fst4` |
| `enable_fst4w` | `sdrkit_wsjt_get_enable_fst4w` | `sdrkit_wsjt_set_enable_fst4w` |
| `enable_msk144` | `sdrkit_wsjt_get_enable_msk144` | `sdrkit_wsjt_set_enable_msk144` |
| `jt65_submode` | `sdrkit_wsjt_get_jt65_submode` | `sdrkit_wsjt_set_jt65_submode` |
| `jt4_submode` | `sdrkit_wsjt_get_jt4_submode` | `sdrkit_wsjt_set_jt4_submode` |
| `q65_submode` | `sdrkit_wsjt_get_q65_submode` | `sdrkit_wsjt_set_q65_submode` |
| `q65_period` | `sdrkit_wsjt_get_q65_period` | `sdrkit_wsjt_set_q65_period` |
| `fst4_period` | `sdrkit_wsjt_get_fst4_period` | `sdrkit_wsjt_set_fst4_period` |
| `fst4w_period` | `sdrkit_wsjt_get_fst4w_period` | `sdrkit_wsjt_set_fst4w_period` |
| `msk144_period` | `sdrkit_wsjt_get_msk144_period` | `sdrkit_wsjt_set_msk144_period` |
| `js8_normal` | `sdrkit_wsjt_get_js8_normal` | `sdrkit_wsjt_set_js8_normal` |
| `js8_fast` | `sdrkit_wsjt_get_js8_fast` | `sdrkit_wsjt_set_js8_fast` |
| `js8_turbo` | `sdrkit_wsjt_get_js8_turbo` | `sdrkit_wsjt_set_js8_turbo` |
| `mirrored` | `sdrkit_wsjt_get_mirrored` | `sdrkit_wsjt_set_mirrored` |
| `deep` | `sdrkit_wsjt_get_deep` | `sdrkit_wsjt_set_deep` |
| `early_decode` | `sdrkit_wsjt_get_early_decode` | `sdrkit_wsjt_set_early_decode` |
| `demod` | `sdrkit_wsjt_get_demod` | `sdrkit_wsjt_set_demod` |
| `listen` | `sdrkit_wsjt_get_listen` | `sdrkit_wsjt_set_listen` |
| `time_source` | `sdrkit_wsjt_get_time_source` | `sdrkit_wsjt_set_time_source` |
| `time_start` | `sdrkit_wsjt_get_time_start` | `sdrkit_wsjt_set_time_start` |
| `band_edges` | `sdrkit_wsjt_get_band_edges` | `sdrkit_wsjt_set_band_edges` |
| `dials` | `sdrkit_wsjt_get_dials` | `sdrkit_wsjt_set_dials` |
| `wide` | `sdrkit_wsjt_get_wide` | `sdrkit_wsjt_set_wide` |
| `wide_span` | `sdrkit_wsjt_get_wide_span` | `sdrkit_wsjt_set_wide_span` |
| `multiband` | `sdrkit_wsjt_get_multiband` | `sdrkit_wsjt_set_multiband` |
| `multiband_select` | `sdrkit_wsjt_get_multiband_select` | `sdrkit_wsjt_set_multiband_select` |
| `spots` | `sdrkit_wsjt_get_spots` | `sdrkit_wsjt_set_spots` |
| `qso_mode` | `sdrkit_wsjt_get_qso_mode` | `sdrkit_wsjt_set_qso_mode` |
| `qso_even` | `sdrkit_wsjt_get_qso_even` | `sdrkit_wsjt_set_qso_even` |
| `qso_auto` | `sdrkit_wsjt_get_qso_auto` | `sdrkit_wsjt_set_qso_auto` |
| `qso_hold` | `sdrkit_wsjt_get_qso_hold` | `sdrkit_wsjt_set_qso_hold` |
| `qso_hz` | `sdrkit_wsjt_get_qso_hz` | `sdrkit_wsjt_set_qso_hz` |
| `qso_retries` | `sdrkit_wsjt_get_qso_retries` | `sdrkit_wsjt_set_qso_retries` |
| `qso_watchdog` | `sdrkit_wsjt_get_qso_watchdog` | `sdrkit_wsjt_set_qso_watchdog` |
| `beacon_dbm` | `sdrkit_wsjt_get_beacon_dbm` | `sdrkit_wsjt_set_beacon_dbm` |
| `beacon_every` | `sdrkit_wsjt_get_beacon_every` | `sdrkit_wsjt_set_beacon_every` |
| `beacon_offset` | `sdrkit_wsjt_get_beacon_offset` | `sdrkit_wsjt_set_beacon_offset` |
| `beacon_hz` | `sdrkit_wsjt_get_beacon_hz` | `sdrkit_wsjt_set_beacon_hz` |
| `js8_speed` | `sdrkit_wsjt_get_js8_speed` | `sdrkit_wsjt_set_js8_speed` |
| `js8_hz` | `sdrkit_wsjt_get_js8_hz` | `sdrkit_wsjt_set_js8_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mirrored` | `sdrkit_wsjt_gen_get_mirrored` | `sdrkit_wsjt_gen_set_mirrored` |
| `gen_live` | `sdrkit_wsjt_gen_get_live` | `sdrkit_wsjt_gen_set_live` |
| `gen_single` | `sdrkit_wsjt_gen_get_single` | `sdrkit_wsjt_gen_set_single` |
| `gen_hz` | `sdrkit_wsjt_gen_get_hz` | `sdrkit_wsjt_gen_set_hz` |
| `gen_short` | `sdrkit_wsjt_gen_get_short` | `sdrkit_wsjt_gen_set_short` |

## Try it

```sh
sdrkit run wsjt --source synth:wsjt
sdrkit run wsjt --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

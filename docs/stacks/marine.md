<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Marine (HF)

NAVTEX warnings, DSC calls on MF/HF and VHF channel 70, and HF weather fax charts.

Stack id: `marine`. Group: Marine.

Signal types: SITOR, DSC, Weather fax.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 518 kHz |
| Tuning range | 100 kHz to 174 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 10 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `navtex` | NAVTEX | 518 kHz |
| `dsc` | DSC | 2.1875 MHz |
| `wefax` | Weather fax | 7.8781 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:marine` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `service` | Service | choice | `auto` | `auto` (Automatic), `navtex` (NAVTEX), `dsc` (DSC), `wefax` (Weather fax) | Which decoders run. Automatic chooses from the dial. |
| `fax_lpm` | Lines per minute | choice | `detect` | `detect` (Detect), `60`, `90`, `120`, `240` |  |
| `fax_ioc` | Index of cooperation | choice | `tone` | `tone` (From the start tone), `576`, `288` |  |
| `fax_auto_slant` | Automatic slant | yes or no | true |  |  |
| `fax_slant_ppm` | Slant | number (ppm) | 0 | -300 to 300 |  |
| `fax_max_lines` | Most lines | whole number | 8000 | 1 to 20000 |  |
| `fax_rotate` | Rotation | choice | `0` | `0` (None), `1` (1 turn), `2` (2 turns), `3` (3 turns), `-1` (1 turn back) |  |
| `row_filter` | Row filter | yes or no | true |  | Smooths the noise along each row of a weak fax picture, with the noise measured on the row. |
| `listen` | Listen | yes or no | false |  |  |
| `dsc_scan` | DSC scan | yes or no | false |  |  |
| `dsc_scan_sets` | DSC channel sets | text | `distress` |  | distress, worldwide, series_a, series_b, vhf |
| `dsc_scan_dwell_s` | DSC dwell | duration (s) | 4 s | 1 to 30 |  |
| `dsc_parallel` | Parallel DSC | yes or no | false |  |  |
| `names` | Name table | text | empty |  | Ship and coast station names by MMSI to import, one record per line. |
| `time_base` | Time base | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_service` | Test signal | choice | `navtex` | `navtex` (NAVTEX), `dsc` (DSC), `wefax` (Weather fax), `dsc_vhf` (DSC on VHF) | What the test signal sends when no submode names it. |
| `gen_idle_s` | NAVTEX phasing | duration (s) | 6 s | 0 to 60 |  |
| `gen_ioc` | Fax IOC | whole number | 576 | 100 to 1000 |  |
| `gen_lines` | Fax lines | whole number | 800 | 1 to 20000 |  |
| `gen_lpm` | Fax lines per minute | number | 120 | 30 to 480 |  |
| `gen_phasing_s` | Fax phasing | duration (s) | 10 s | 0 to 60 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `active_service` | Service | choice |
| `fsk_found` | FSK pair | yes or no |
| `fsk_low_hz` | Low tone | frequency |
| `fsk_high_hz` | High tone | frequency |
| `shift_hz` | Shift | frequency |
| `symbol_rate_hz` | Symbol rate | frequency |
| `tone_level_db` | Tones over the noise | number (dB) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `navtex_good` | NAVTEX good | whole number |
| `navtex_bad` | NAVTEX bad | whole number |
| `dsc_good` | DSC good | whole number |
| `dsc_bad` | DSC bad | whole number |
| `distress` | Distress | yes or no |
| `fax_state` | Fax | choice |
| `ioc` | IOC | whole number |
| `lpm` | Lines per minute | whole number |
| `lines` | Lines | whole number |
| `width` | Width | whole number |
| `slant_ppm` | Slant | number (ppm) |
| `black_level_hz` | Black level | frequency |
| `last_tone_hz` | Last tone | frequency |
| `phasing_lines` | Phasing lines | whole number |
| `fax_snr_db` | Fax SNR | number (dB) |
| `pictures_started` | Pictures started | whole number |
| `pictures_done` | Pictures done | whole number |
| `end_reason` | Picture end | text |
| `scan_channel_hz` | Scan channel | frequency |
| `reports` | Reports | whole number |
| `signal_time` | Signal time | duration (s) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `navtex` | NAVTEX | `time` (time), `header` (text), `subject_name` (text), `chars` (whole number), `errors` (whole number), `complete` (yes or no), `repeats` (whole number), `text` (text) |
| `dsc` | DSC | `time` (time), `freq_hz` (frequency), `kind` (text), `from_mmsi` (text), `from_name` (text), `from_country` (text), `to_mmsi` (text), `category_name` (text), `nature_name` (text), `position` (position), `telecommands` (text), `freq_rx` (text), `freq_tx` (text), `ecc_ok` (yes or no), `repaired` (whole number), `erasures` (whole number), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `audio_spectrum` | Audio spectrum | Hz | dB |

## Map points

The stack puts points on a map: DSC positions.

## Events

| Kind | Label | Fields |
|---|---|---|
| `navtex.message` | NAVTEX message | `source` (text), `family` (choice), `urgency` (choice), `text` (text), `header` (text), `subject` (text), `subject_name` (text), `number` (whole number), `complete` (yes or no), `errors` (whole number), `chars` (whole number) |
| `dsc.call` | DSC call | `source` (text), `target` (text), `family` (choice), `urgency` (choice), `text` (text), `position` (position), `format` (whole number), `kind` (text), `category` (whole number), `category_name` (text), `from_name` (text), `from_country` (text), `to_type` (text), `distress_mmsi` (text), `nature` (whole number), `nature_name` (text), `position_text` (text), `utc` (text), `telecommand1` (whole number), `telecommand2` (whole number), `telecommands` (text), `freq_rx` (text), `freq_tx` (text), `eos` (whole number), `ecc_ok` (yes or no), `repaired` (whole number), `erasures` (whole number), `vhf` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `clear_picture` | Clear the picture |  |  | Starts the fax picture again. |

## Typed interface

The stack is in the family `marine`, version 1.3. The C header `sdrkit/marine.h`, the registry record `registry/marine.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: sound (monitor), text.

### Result fields

Struct `HfFields`. The result fields of the marine HF stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The reception state of a display. |
| `active_service` | choice | always | 1.0 | The service heard last. |
| `fsk_found` | yes or no | always | 1.0 | An FSK pair stands above the noise. |
| `fsk_low_hz` | frequency | optional | 1.0 | The lower FSK tone, from the dial. |
| `fsk_high_hz` | frequency | optional | 1.0 | The higher FSK tone, from the dial. |
| `shift_hz` | frequency | optional | 1.0 | The measured shift. |
| `symbol_rate_hz` | frequency | optional | 1.0 | Symbol rate found, Hz. |
| `tone_level_db` | number (dB) | optional | 1.0 | The level of the tones over the noise, per bin. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | optional | 1.0 | Carrier offset from where it was expected, Hz. |
| `navtex_good` | whole number | always | 1.0 | NAVTEX messages complete with no unreadable character. |
| `navtex_bad` | whole number | always | 1.0 | Other NAVTEX messages. |
| `dsc_good` | whole number | always | 1.0 | DSC calls with a good check. |
| `dsc_bad` | whole number | always | 1.0 | DSC calls with a failed check. |
| `distress` | yes or no | always | 1.0 | The DSC list holds a distress call. |
| `fax_state` | choice | optional | 1.0 | The fax decoder's state. |
| `ioc` | whole number | optional | 1.0 | The index of cooperation. |
| `lpm` | whole number | optional | 1.0 | Lines per minute. |
| `lines` | whole number | optional | 1.0 | Picture lines received. |
| `width` | whole number | optional | 1.0 | Picture width, pixels. |
| `slant_ppm` | number (ppm) | optional | 1.0 | The slant in use. |
| `black_level_hz` | frequency | optional | 1.0 | The black level. |
| `last_tone_hz` | frequency | optional | 1.0 | The last start or stop tone measured. |
| `phasing_lines` | whole number | optional | 1.0 | Phasing lines seen. |
| `fax_snr_db` | number (dB) | optional | 1.0 | The fax SNR: tone over audio power, or the black-white swing over the noise of the black level. |
| `pictures_started` | whole number | optional | 1.0 | Pictures started. |
| `pictures_done` | whole number | optional | 1.0 | Pictures finished. |
| `end_reason` | text | optional | 1.0 | Why the last picture ended. |
| `scan_channel_hz` | frequency | optional | 1.0 | The channel the scanner holds. |
| `reports` | whole number | always | 1.0 | Reports made. |
| `signal_time` | duration (s) | always | 1.0 | Signal time since the start. |

### Table `navtex`

Struct `NavtexRow`. A row of the NAVTEX message table. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When a message or call was heard (wall time). |
| `header` | text | always | 1.0 | B1 B2 B3 B4. |
| `subject_name` | text | always | 1.0 | The name of the subject. |
| `chars` | whole number | always | 1.0 | Characters of the text. |
| `errors` | whole number | always | 1.0 | Unreadable characters. |
| `complete` | yes or no | always | 1.0 | NNNN was seen. |
| `repeats` | whole number | always | 1.0 | Times the message came again. |
| `text` | text | always | 1.0 | The text of a decoded message. |

The rows are the messages of the stack. The column `header` names a row. Text: `text`. Time: `time`.

### Table `dsc`

Struct `DscRow`. A row of the DSC call table. The row key is the place of the call in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When a message or call was heard (wall time). |
| `freq_hz` | frequency | optional | 1.0 | The frequency a station was heard on. |
| `kind` | text | always | 1.0 | The kind of call in words. |
| `from_mmsi` | text | optional | 1.0 | The sender's MMSI. |
| `from_name` | text | optional | 1.0 | The sender's name. |
| `from_country` | text | optional | 1.0 | The sender's country. |
| `to_mmsi` | text | optional | 1.0 | The called station. |
| `category_name` | text | optional | 1.0 | The category in words. |
| `nature_name` | text | optional | 1.0 | The nature of distress in words. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `telecommands` | text | optional | 1.0 | The telecommands in words. |
| `freq_rx` | text | optional | 1.0 | The receive frequency or channel the call proposes. |
| `freq_tx` | text | optional | 1.0 | The transmit frequency or channel the call proposes. |
| `ecc_ok` | yes or no | always | 1.0 | The error-check character matched. |
| `repaired` | whole number | always | 1.0 | Symbols taken from the second copy or rebuilt. |
| `erasures` | whole number | always | 1.0 | Symbols unreadable in both copies. |
| `text` | text | always | 1.0 | The text of a decoded message. |

The rows are the calls of the stack. The column `from_mmsi` names a row. Place of a row: the column `position`. Sender: `from_mmsi`. Addressee: `to_mmsi`. Text: `text`. Time: `time`.

### Event `navtex.message`

Struct `NavtexMessageEvent`. The fields of the event of a NAVTEX message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `header` | text | always | 1.0 | B1 B2 B3 B4. |
| `subject` | text | always | 1.0 | B2, the subject indicator. |
| `subject_name` | text | always | 1.0 | The name of the subject. |
| `number` | whole number | always | 1.0 | B3 B4, the serial number. |
| `complete` | yes or no | always | 1.0 | NNNN was seen. |
| `errors` | whole number | always | 1.0 | Unreadable characters. |
| `chars` | whole number | always | 1.0 | Characters of the text. |

### Event `dsc.call`

Struct `DscCallEvent`. The fields of the event of a DSC call.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `target` | text | optional | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `format` | whole number | always | 1.0 | The format specifier. |
| `kind` | text | always | 1.0 | The kind of call in words. |
| `category` | whole number | optional | 1.0 | The category code. |
| `category_name` | text | optional | 1.0 | The category in words. |
| `from_name` | text | optional | 1.0 | The sender's name. |
| `from_country` | text | optional | 1.0 | The sender's country. |
| `to_type` | text | optional | 1.0 | What the called station is. |
| `distress_mmsi` | text | optional | 1.0 | The station in distress. |
| `nature` | whole number | optional | 1.0 | The nature of distress code. |
| `nature_name` | text | optional | 1.0 | The nature of distress in words. |
| `position_text` | text | optional | 1.0 | The position in the call, as sent. |
| `utc` | text | optional | 1.0 | The time in the call (HH:MM UTC). |
| `telecommand1` | whole number | optional | 1.0 | The first telecommand. |
| `telecommand2` | whole number | optional | 1.0 | The second telecommand. |
| `telecommands` | text | optional | 1.0 | The telecommands in words. |
| `freq_rx` | text | always | 1.0 | The receive frequency or channel the call proposes. |
| `freq_tx` | text | always | 1.0 | The transmit frequency or channel the call proposes. |
| `eos` | whole number | optional | 1.0 | The end of sequence character. |
| `ecc_ok` | yes or no | always | 1.0 | The error-check character matched. |
| `repaired` | whole number | always | 1.0 | Symbols taken from the second copy or rebuilt. |
| `erasures` | whole number | always | 1.0 | Symbols unreadable in both copies. |
| `vhf` | yes or no | always | 1.0 | Heard on VHF channel 70. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_marine_clear` | none | none |
| `clear_picture` | `sdrkit_marine_clear_picture` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `service` | `sdrkit_marine_get_service` | `sdrkit_marine_set_service` |
| `fax_lpm` | `sdrkit_marine_get_fax_lpm` | `sdrkit_marine_set_fax_lpm` |
| `fax_ioc` | `sdrkit_marine_get_fax_ioc` | `sdrkit_marine_set_fax_ioc` |
| `fax_auto_slant` | `sdrkit_marine_get_fax_auto_slant` | `sdrkit_marine_set_fax_auto_slant` |
| `fax_slant_ppm` | `sdrkit_marine_get_fax_slant_ppm` | `sdrkit_marine_set_fax_slant_ppm` |
| `fax_max_lines` | `sdrkit_marine_get_fax_max_lines` | `sdrkit_marine_set_fax_max_lines` |
| `fax_rotate` | `sdrkit_marine_get_fax_rotate` | `sdrkit_marine_set_fax_rotate` |
| `row_filter` | `sdrkit_marine_get_row_filter` | `sdrkit_marine_set_row_filter` |
| `listen` | `sdrkit_marine_get_listen` | `sdrkit_marine_set_listen` |
| `dsc_scan` | `sdrkit_marine_get_dsc_scan` | `sdrkit_marine_set_dsc_scan` |
| `dsc_scan_sets` | `sdrkit_marine_get_dsc_scan_sets` | `sdrkit_marine_set_dsc_scan_sets` |
| `dsc_scan_dwell_s` | `sdrkit_marine_get_dsc_scan_dwell_s` | `sdrkit_marine_set_dsc_scan_dwell_s` |
| `dsc_parallel` | `sdrkit_marine_get_dsc_parallel` | `sdrkit_marine_set_dsc_parallel` |
| `names` | `sdrkit_marine_get_names` | `sdrkit_marine_set_names` |
| `time_base` | `sdrkit_marine_get_time_base` | `sdrkit_marine_set_time_base` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_service` | `sdrkit_marine_gen_get_service` | `sdrkit_marine_gen_set_service` |
| `gen_idle_s` | `sdrkit_marine_gen_get_idle_s` | `sdrkit_marine_gen_set_idle_s` |
| `gen_ioc` | `sdrkit_marine_gen_get_ioc` | `sdrkit_marine_gen_set_ioc` |
| `gen_lines` | `sdrkit_marine_gen_get_lines` | `sdrkit_marine_gen_set_lines` |
| `gen_lpm` | `sdrkit_marine_gen_get_lpm` | `sdrkit_marine_gen_set_lpm` |
| `gen_phasing_s` | `sdrkit_marine_gen_get_phasing_s` | `sdrkit_marine_gen_set_phasing_s` |

## Try it

```sh
sdrkit run marine --source synth:marine
sdrkit run marine --source synth:marine --submode navtex
sdrkit run marine --source <driver>:<address> --freq 0.518
```

## Test recordings

- `tests/vectors/marine/dsc_mfhf_12k.sigmf-meta`
- `tests/vectors/marine/dsc_vhf_ch70_48k.sigmf-meta`
- `tests/vectors/marine/navtex_518k_12k.sigmf-meta`
- `tests/vectors/marine/wefax_ioc576_120lpm_12k.sigmf-meta`
<!-- generated: end -->

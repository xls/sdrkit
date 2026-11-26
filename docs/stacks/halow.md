<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wi-Fi HaLow (802.11ah)

Sub-GHz Wi-Fi HaLow networks from their S1G beacons: names, security, channels, widths and MCS.

Stack id: `halow`. Group: Data and IoT.

Signal types: Wi-Fi HaLow.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 755 MHz to 928 MHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 4 MHz |
| Radio filter | 4 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:halow` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `start_hz` | Channel starting frequency | frequency | 0 Hz | 0 Hz to 1 GHz | Channel n is centred at this frequency plus n x 500 kHz. |
| `channels_1` | 1 MHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 3 5"). |
| `channels_2` | 2 MHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 3 5"). |
| `channels_4` | 4 MHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 3 5"). |
| `channels_8` | 8 MHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 3 5"). |
| `channels_16` | 16 MHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 3 5"). |
| `list_width` | Channel list width | choice | `1` | `1` (1 MHz), `2` (2 MHz), `4` (4 MHz), `8` (8 MHz), `16` (16 MHz) |  |
| `channel` | Channel | whole number | 1 | 1 to 255 | A channel of the plan at the list width: the host tunes to it. |
| `scan` | Scan | yes or no | false |  | Asks the host to step through the plan's channels at the list width. |
| `dwell_s` | Dwell | duration (s) | 6 s | 1 to 60 |  |
| `filter_text` | Filter | text | empty |  | Shows only the networks whose SSID, BSSID, vendor or security holds this text. |
| `min_level_dbfs` | Level above | number (dBFS) | -120 | -120 to 0 |  |
| `receive` | Receive | choice | `auto14` | `auto14` (Auto 1 - 4 MHz), `auto_all` (Auto all widths), `1` (1 MHz), `2` (2 MHz), `4` (4 MHz), `8` (8 MHz), `16` (16 MHz) | The widths read; the rate asked follows, and the radio is reopened when it changes. |
| `sub_channel_scan` | Sub-channel scan | yes or no | true |  | Look for 1 and 2 MHz primary channels away from the centre of a wider capture. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `networks` | Networks | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `undecoded` | Only counted | whole number |
| `mimo_frames` | Several streams | whole number |
| `stbc_frames` | STBC | whole number |
| `ldpc_frames` | LDPC | whole number |
| `ampdu_frames` | A-MPDU | whole number |
| `ndp_frames` | NDP | whole number |
| `preambles` | Preambles | whole number |
| `sig_ok` | SIG fields read | whole number |
| `sig_bad` | SIG fields not readable | whole number |
| `data_bad` | Data fields cut short | whole number |
| `beacons_seen` | Network frames | whole number |
| `frames_per_second` | Frames per second | whole number |
| `tuned_hz` | Tuned | frequency |
| `channel` | Channel | whole number |
| `max_width_hz` | Widest channel | frequency |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `scanning` | Scanning | yes or no |
| `receive_note` | Note | text |
| `data_note` | Data | text |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `networks` | Networks | `ssid` (text), `bssid` (text), `vendor` (text), `channel` (whole number), `freq_hz` (frequency), `width_hz` (frequency), `op_width_hz` (frequency), `mcs` (whole number), `nss` (whole number), `snr_db` (number (dB)), `security` (text), `interval` (duration (s)), `level_dbfs` (number (dBFS)), `beacons` (whole number), `country` (text), `last_heard` (time) |
| `frames` | Frames | `signal_s` (number (s)), `width_hz` (frequency), `mcs` (whole number), `nss` (whole number), `preamble` (text), `snr_db` (number (dB)), `type` (text), `bssid` (text), `length` (whole number), `details` (text) |
| `widths` | Widths | `width_hz` (frequency), `sig_ok` (whole number), `min_rate_hz` (frequency), `rate_hz` (frequency), `state` (choice) |
| `by_mcs` | By MCS | `mcs` (whole number), `ppdus` (whole number) |
| `by_preamble` | By preamble | `preamble` (choice), `ppdus` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `wlan`, version 4.3. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `networks`.

### Result fields

Struct `HalowFields`. The result fields of `halow`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `networks` | whole number | always | 1.0 | Networks in the table. |
| `frames_ok` | whole number | always | 1.0 | Frames with a good FCS. |
| `frames_bad` | whole number | always | 1.0 | Frames with a bad FCS. |
| `undecoded` | whole number | always | 1.0 | PPDUs read from their SIG field only. |
| `mimo_frames` | whole number | always | 1.0 | PPDUs with several spatial streams. |
| `stbc_frames` | whole number | always | 1.0 | PPDUs with STBC. |
| `ldpc_frames` | whole number | always | 1.0 | PPDUs with LDPC coding. |
| `ampdu_frames` | whole number | always | 1.0 | A-MPDUs. |
| `ndp_frames` | whole number | always | 1.0 | Null data packets. |
| `preambles` | whole number | always | 1.0 | Preambles detected. |
| `sig_ok` | whole number | always | 1.0 | SIG fields read. |
| `sig_bad` | whole number | always | 1.0 | SIG fields not readable. |
| `data_bad` | whole number | always | 1.0 | Data fields cut short. |
| `beacons_seen` | whole number | always | 1.0 | Network frames (beacons and probe responses). |
| `frames_per_second` | whole number | always | 1.0 | Frames in the last whole second. |
| `tuned_hz` | frequency | optional | 1.0 | The tuned frequency. No value when it is not known, as for a recording. |
| `channel` | whole number | optional | 1.0 | The nearest 1 MHz channel of the plan, if one is within 1 MHz of the tuned frequency. |
| `max_width_hz` | frequency | always | 1.0 | The widest channel that the rate holds. |
| `snr_db` | number (dB) | optional | 1.0 | The running mean of the SNR. No value before the first good frame. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last good frame. No value before it. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input. No value before the first input. |
| `scanning` | yes or no | always | 1.0 | The host steps through the channels. |
| `receive_note` | text | always | 1.0 | The receive and privacy note. |
| `data_note` | text | always | 1.0 | The note on what the data decoder reads. |

### Table `networks`

Struct `HalowNetworkRow`. A row of the networks of `halow`. The row key is the BSSID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ssid` | text | always | 1.0 | The SSID, "(hidden)", the compressed SSID or "-" when not known. |
| `bssid` | text | always | 1.0 | The BSSID. |
| `vendor` | text | always | 1.0 | The vendor. |
| `channel` | whole number | always | 1.0 | The channel number. |
| `freq_hz` | frequency | optional | 1.0 | The frequency of the channel, if known. |
| `width_hz` | frequency | always | 1.0 | The width of the last PPDU. |
| `op_width_hz` | frequency | optional | 1.0 | The operating width, if a beacon gave it. |
| `mcs` | whole number | always | 1.0 | The MCS of the last PPDU. |
| `nss` | whole number | always | 1.0 | The spatial streams of the last PPDU. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the last PPDU. |
| `security` | text | always | 1.0 | The security. |
| `interval` | duration (s) | always | 1.0 | The beacon interval. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last PPDU. |
| `beacons` | whole number | always | 1.0 | Beacons heard. |
| `country` | text | always | 1.0 | The country. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The rows are the stations or nodes of the stack. The column `ssid` names a row. Default order: the column `snr_db`, highest first. Time: `last_heard`.

### Table `frames`

Struct `HalowFrameRow`. A row of the frame log of `halow`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `width_hz` | frequency | always | 1.0 | The width of the PPDU. |
| `mcs` | whole number | always | 1.0 | The MCS. |
| `nss` | whole number | always | 1.0 | The spatial streams. |
| `preamble` | text | optional | 1.0 | The name of the preamble, if the SIG field gave it. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the PPDU. |
| `type` | text | always | 1.0 | The kind of the frame. |
| `bssid` | text | always | 1.0 | The BSSID. |
| `length` | whole number | always | 1.0 | The length. |
| `details` | text | always | 1.0 | The details of the frame. |

The column `bssid` names a row.

### Table `widths`

Struct `HalowWidthRow`. A row of the width view of `halow`. The row key is the width in MHz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `width_hz` | frequency | always | 1.0 | The width. |
| `sig_ok` | whole number | always | 1.0 | SIG fields read at the width. |
| `min_rate_hz` | frequency | always | 1.0 | The lowest radio rate for the width. |
| `rate_hz` | frequency | always | 1.0 | The radio rate recommended for the width. |
| `state` | choice | always | 1.0 | Whether the stack reads the width. |

### Table `by_mcs`

Struct `HalowMcsRow`. A row of the PPDUs of each MCS of `halow`. The row key is the MCS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mcs` | whole number | always | 1.0 | The MCS. |
| `ppdus` | whole number | always | 1.0 | PPDUs counted. |

### Table `by_preamble`

Struct `HalowPreambleRow`. A row of the PPDUs of each preamble of `halow`. The row key is the key of the preamble.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `preamble` | choice | always | 1.0 | The preamble. |
| `ppdus` | whole number | always | 1.0 | PPDUs counted. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_halow_clear` | none | none |
| `remove` | `sdrkit_halow_remove` | `sdrkit_wlan_network_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `start_hz` | `sdrkit_halow_get_start_hz` | `sdrkit_halow_set_start_hz` |
| `channels_1` | `sdrkit_halow_get_channels_1` | `sdrkit_halow_set_channels_1` |
| `channels_2` | `sdrkit_halow_get_channels_2` | `sdrkit_halow_set_channels_2` |
| `channels_4` | `sdrkit_halow_get_channels_4` | `sdrkit_halow_set_channels_4` |
| `channels_8` | `sdrkit_halow_get_channels_8` | `sdrkit_halow_set_channels_8` |
| `channels_16` | `sdrkit_halow_get_channels_16` | `sdrkit_halow_set_channels_16` |
| `list_width` | `sdrkit_halow_get_list_width` | `sdrkit_halow_set_list_width` |
| `channel` | `sdrkit_halow_get_channel` | `sdrkit_halow_set_channel` |
| `scan` | `sdrkit_halow_get_scan` | `sdrkit_halow_set_scan` |
| `dwell_s` | `sdrkit_halow_get_dwell_s` | `sdrkit_halow_set_dwell_s` |
| `filter_text` | `sdrkit_halow_get_filter_text` | `sdrkit_halow_set_filter_text` |
| `min_level_dbfs` | `sdrkit_halow_get_min_level_dbfs` | `sdrkit_halow_set_min_level_dbfs` |
| `receive` | `sdrkit_halow_get_receive` | `sdrkit_halow_set_receive` |
| `sub_channel_scan` | `sdrkit_halow_get_sub_channel_scan` | `sdrkit_halow_set_sub_channel_scan` |

## Try it

```sh
sdrkit run halow --source synth:halow
sdrkit run halow --source <driver>:<address> --freq 0
```
<!-- generated: end -->

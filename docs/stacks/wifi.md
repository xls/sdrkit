<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wi-Fi networks

The Wi-Fi access points around on 2.4, 5 and 6 GHz from their beacons: names, security, channels and capabilities.

Stack id: `wifi`. Group: Data and IoT.

Signal types: Wi-Fi.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.437 GHz |
| Tuning range | 2.4 GHz to 7.125 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 20 MHz |
| Signal bandwidth | 20 MHz |
| Radio filter | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wifi` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `band` | Band | choice | `2.4` | `2.4` (2.4 GHz), `5` (5 GHz), `6` (6 GHz) | Choosing a band asks the host to tune to the first channel of its hop list. |
| `channel` | Channel | whole number | 6 | 1 to 233 | A channel of the band: the host tunes to it. |
| `hop` | Hop | yes or no | false |  | Asks the host to step through the hop list; dwell longer than 102.4 ms (the beacon interval). |
| `dwell_ms` | Dwell | number (ms) | 200 | 100 to 2000 |  |
| `psc_only` | 6 GHz: preferred scanning channels only | yes or no | true |  |  |
| `channels_24` | 2.4 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band. |
| `channels_5` | 5 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band. |
| `channels_6` | 6 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band. |
| `filter_text` | Filter | text | empty |  | Shows only the networks whose SSID, BSSID, vendor or security holds this text. |
| `min_level_dbfs` | Level above | number (dBFS) | -120 | -120 to 0 |  |
| `open_only` | Open only | yes or no | false |  |  |
| `band_filter` | Band shown | choice | `all` | `all` (All), `2.4` (2.4 GHz), `5` (5 GHz), `6` (6 GHz) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Scene | choice | `2.4` | `2.4` (2.4 GHz), `5` (5 GHz), `6` (6 GHz) |  |
| `gen_channel` | Primary channel | whole number | 0 | 0 to 233 | 0: the scene's own channel. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `networks` | Networks | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `dsss_sfds` | DSSS preambles | whole number |
| `dsss_headers_ok` | DSSS headers good | whole number |
| `dsss_headers_bad` | DSSS headers bad | whole number |
| `dsss_cck_headers` | CCK headers | whole number |
| `ofdm_preambles` | OFDM preambles | whole number |
| `ofdm_signal_ok` | SIGNAL good | whole number |
| `ofdm_signal_bad` | SIGNAL bad | whole number |
| `mgmt_frames` | Management frames | whole number |
| `control_frames` | Control frames | whole number |
| `data_frames` | Data frames | whole number |
| `beacons` | Beacons | whole number |
| `probe_responses` | Probe responses | whole number |
| `fils_frames` | FILS frames | whole number |
| `duplicate_frames` | Duplicates | whole number |
| `band` | Band | choice |
| `channel` | Channel | whole number |
| `sub_channels` | Channels at once | whole number |
| `phy` | PHY | choice |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `hopping` | Hopping | yes or no |
| `passive` | Passive | text |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `networks` | Networks | `ssid` (text), `bssid` (text), `vendor` (text), `band` (choice), `channel` (whole number), `heard_on` (whole number), `width_hz` (frequency), `standard` (text), `security` (text), `level_dbfs` (number (dBFS)), `first_heard` (time), `last_heard` (time), `beacons` (whole number), `probe_responses` (whole number), `interval` (duration (s)), `ciphers` (text), `akm` (text), `pmf` (text), `capability` (text), `details` (text) |
| `beacons` | Beacons | `signal_s` (number (s)), `channel` (whole number), `bssid` (text), `level_dbfs` (number (dBFS)), `bytes` (whole number), `details` (text) |
| `ofdm_rates` | OFDM frames by rate | `rate_mbps` (number (Mbit/s)), `frames` (whole number) |
| `bands` | Bands | `band` (choice), `networks` (whole number), `beacons` (whole number), `ax_be` (whole number), `wpa3_owe` (whole number), `open` (whole number), `strongest_dbfs` (number (dBFS)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `wlan`, version 3.0. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `WifiFields`. The result fields of `wifi`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `networks` | whole number | always | 1.0 | Networks in the table. |
| `frames_ok` | whole number | always | 1.0 | Frames with a good FCS. |
| `frames_bad` | whole number | always | 1.0 | Frames with a bad FCS. |
| `dsss_sfds` | whole number | always | 1.0 | DSSS SFDs found. |
| `dsss_headers_ok` | whole number | always | 1.0 | DSSS PLCP headers with a good CRC. |
| `dsss_headers_bad` | whole number | always | 1.0 | DSSS PLCP headers with a bad CRC. |
| `dsss_cck_headers` | whole number | always | 1.0 | DSSS headers of a CCK rate, not decoded. |
| `ofdm_preambles` | whole number | always | 1.0 | OFDM short preambles found. |
| `ofdm_signal_ok` | whole number | always | 1.0 | OFDM SIGNAL fields with a good parity. |
| `ofdm_signal_bad` | whole number | always | 1.0 | OFDM SIGNAL fields with a bad parity or rate. |
| `mgmt_frames` | whole number | always | 1.0 | Management frames. |
| `control_frames` | whole number | always | 1.0 | Control frames. |
| `data_frames` | whole number | always | 1.0 | Data frames. |
| `beacons` | whole number | always | 1.0 | Beacons. |
| `probe_responses` | whole number | always | 1.0 | Probe responses. |
| `fils_frames` | whole number | always | 1.0 | FILS discovery frames. |
| `duplicate_frames` | whole number | always | 1.0 | Frames heard again on a second channel at once. |
| `band` | choice | optional | 1.0 | The band of the tuned frequency, if it is in one. |
| `channel` | whole number | optional | 1.0 | The channel of the tuned frequency, if it is one. |
| `sub_channels` | whole number | always | 1.0 | The 20 MHz channels that the stack reads at once. |
| `phy` | choice | optional | 1.0 | The PHY of the last good frame. No value before it. |
| `snr_db` | number (dB) | optional | 1.0 | The running mean of the SNR. No value before the first good frame. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last good frame. No value before it. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input. No value before the first input. |
| `hopping` | yes or no | always | 1.0 | The host steps through the hop list. |
| `passive` | text | always | 1.0 | The passive note. |

### Table `networks`

Struct `WifiNetworkRow`. A row of the networks of `wifi`. The row key is the BSSID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ssid` | text | always | 1.0 | The SSID, or "(hidden)". |
| `bssid` | text | always | 1.0 | The BSSID. |
| `vendor` | text | always | 1.0 | The vendor of the BSSID, or "(local)" for a locally administered one. |
| `band` | choice | optional | 1.0 | The band. |
| `channel` | whole number | always | 1.0 | The primary channel. |
| `heard_on` | whole number | always | 1.0 | The channel the frames were heard on. |
| `width_hz` | frequency | always | 1.0 | The channel width. |
| `standard` | text | always | 1.0 | The newest standard ("ax"). |
| `security` | text | always | 1.0 | The security. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `beacons` | whole number | always | 1.0 | Beacons heard. |
| `probe_responses` | whole number | always | 1.0 | Probe responses heard. |
| `interval` | duration (s) | always | 1.0 | The beacon interval. |
| `ciphers` | text | always | 1.0 | The pairwise ciphers. |
| `akm` | text | always | 1.0 | The key management. |
| `pmf` | text | always | 1.0 | Protected management frames: "required", "capable" or "". |
| `capability` | text | always | 1.0 | The capability bits as text. |
| `details` | text | always | 1.0 | The other elements as text. |

The column `ssid` names a row. Default order: the column `level_dbfs`, highest first.

### Table `beacons`

Struct `WifiBeaconRow`. A row of the frame log of `wifi`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `channel` | whole number | always | 1.0 | The channel the frame was heard on. |
| `bssid` | text | always | 1.0 | The BSSID. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `bytes` | whole number | always | 1.0 | The length, FCS included. |
| `details` | text | always | 1.0 | The summary. |

The column `bssid` names a row.

### Table `ofdm_rates`

Struct `WifiRateRow`. A row of the good OFDM frames at each rate of `wifi`. The row key is the rate in Mbit/s.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rate_mbps` | number (Mbit/s) | always | 1.0 | The rate. |
| `frames` | whole number | always | 1.0 | Good frames at the rate. |

### Table `bands`

Struct `WifiBandRow`. A row of the summary of each band of `wifi`. The row key is the key of the band.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | choice | always | 1.0 | The band. |
| `networks` | whole number | always | 1.0 | Networks in the band. |
| `beacons` | whole number | always | 1.0 | Beacons heard in the band. |
| `ax_be` | whole number | always | 1.0 | Networks of 802.11ax or 802.11be. |
| `wpa3_owe` | whole number | always | 1.0 | Networks with WPA3 or OWE. |
| `open` | whole number | always | 1.0 | Open networks. |
| `strongest_dbfs` | number (dBFS) | always | 1.0 | The level of the strongest network. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_wifi_clear` | none | none |
| `remove` | `sdrkit_wifi_remove` | `sdrkit_wlan_network_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `band` | `sdrkit_wifi_get_band` | `sdrkit_wifi_set_band` |
| `channel` | `sdrkit_wifi_get_channel` | `sdrkit_wifi_set_channel` |
| `hop` | `sdrkit_wifi_get_hop` | `sdrkit_wifi_set_hop` |
| `dwell_ms` | `sdrkit_wifi_get_dwell_ms` | `sdrkit_wifi_set_dwell_ms` |
| `psc_only` | `sdrkit_wifi_get_psc_only` | `sdrkit_wifi_set_psc_only` |
| `channels_24` | `sdrkit_wifi_get_channels_24` | `sdrkit_wifi_set_channels_24` |
| `channels_5` | `sdrkit_wifi_get_channels_5` | `sdrkit_wifi_set_channels_5` |
| `channels_6` | `sdrkit_wifi_get_channels_6` | `sdrkit_wifi_set_channels_6` |
| `filter_text` | `sdrkit_wifi_get_filter_text` | `sdrkit_wifi_set_filter_text` |
| `min_level_dbfs` | `sdrkit_wifi_get_min_level_dbfs` | `sdrkit_wifi_set_min_level_dbfs` |
| `open_only` | `sdrkit_wifi_get_open_only` | `sdrkit_wifi_set_open_only` |
| `band_filter` | `sdrkit_wifi_get_band_filter` | `sdrkit_wifi_set_band_filter` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_wifi_gen_get_scene` | `sdrkit_wifi_gen_set_scene` |
| `gen_channel` | `sdrkit_wifi_gen_get_channel` | `sdrkit_wifi_gen_set_channel` |

## Try it

```sh
sdrkit run wifi --source synth:wifi
sdrkit run wifi --source iiod:192.168.2.1 --freq 2437
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wi-Fi networks

The Wi-Fi access points around on 2.4, 5 and 6 GHz from their beacons: names, security, channels and capabilities. Works from 4 Msps with the 802.11b (DSSS) frames of 2.4 GHz only; OFDM and the 5 and 6 GHz bands need 20 Msps.

Stack id: `wifi`. Group: Data and IoT.

Signal types: Wi-Fi.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.437 GHz |
| Tuning range | 2.4 GHz to 7.125 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 20 MHz |
| Radio filter | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wifi` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `band` | Band | choice | `2.4` | `2.4` (2.4 GHz), `5` (5 GHz), `6` (6 GHz) | Choosing a band asks the host to tune to the first channel of its hop list. |
| `channel` | Channel | whole number | 6 | 1 to 233 | A channel of the band: the host tunes to it. A number the band lacks selects the first channel of its hop list. |
| `hop` | Hop | yes or no | true |  | Asks the host to step through the hop list of the band it is tuned to, from the tuned channel. |
| `dwell_ms` | Dwell | number (ms) | 130 | 110 to 2000 |  |
| `psc_only` | 6 GHz: preferred scanning channels only | yes or no | true |  |  |
| `channels_24` | 2.4 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"), in the order of the hop. Empty: every channel of the band; on 2.4 GHz channels 1 to 11, which every region permits; on 5 GHz channels 36 to 48 and 149 to 165 first. |
| `channels_5` | 5 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"), in the order of the hop. Empty: every channel of the band; on 2.4 GHz channels 1 to 11, which every region permits; on 5 GHz channels 36 to 48 and 149 to 165 first. |
| `channels_6` | 6 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"), in the order of the hop. Empty: every channel of the band; on 2.4 GHz channels 1 to 11, which every region permits; on 5 GHz channels 36 to 48 and 149 to 165 first. |
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
| `stations` | Stations | whole number |
| `ppdu_ht` | HT PPDUs | whole number |
| `ppdu_vht` | VHT PPDUs | whole number |
| `ppdu_he_su` | HE SU PPDUs | whole number |
| `ppdu_he_er_su` | HE ER SU PPDUs | whole number |
| `ppdu_he_mu` | HE MU PPDUs | whole number |
| `ppdu_he_tb` | HE TB PPDUs | whole number |
| `ppdu_he_bad_sig` | HE with a bad HE-SIG-A PPDUs | whole number |
| `ppdu_eht` | EHT PPDUs | whole number |
| `ppdu_decoded` | PPDUs decoded | whole number |
| `ppdu_not_decoded_streams` | PPDUs not decoded: two or more spatial streams | whole number |
| `ppdu_not_decoded_width` | PPDUs not decoded: wider than 20 MHz | whole number |
| `ppdu_not_decoded_ldpc` | PPDUs not decoded: LDPC coding | whole number |
| `ppdu_not_decoded_stbc` | PPDUs not decoded: STBC | whole number |
| `ppdu_not_decoded_multi_user` | PPDUs not decoded: multi-user | whole number |
| `ppdu_not_decoded_trigger_based` | PPDUs not decoded: trigger-based | whole number |
| `ppdu_not_decoded_he_data` | PPDUs not decoded: HE data | whole number |
| `ppdu_not_decoded_eht_data` | PPDUs not decoded: EHT data | whole number |
| `ppdu_not_decoded_signal_field` | PPDUs not decoded: a bad signal field | whole number |
| `ppdu_not_decoded_mcs` | PPDUs not decoded: an MCS that is not decoded | whole number |
| `ppdu_not_decoded_length` | PPDUs not decoded: a length that does not fit | whole number |
| `ppdu_not_decoded_data` | PPDUs not decoded: data that did not decode | whole number |
| `ppdu_not_decoded_other` | PPDUs not decoded: another reason | whole number |
| `ppdu_not_decoded_busy` | PPDUs not decoded: the decoder was busy | whole number |
| `ampdu_bad_delimiters` | A-MPDU bad delimiters | whole number |
| `probe_requests` | Probe requests | whole number |
| `authentications` | Authentications | whole number |
| `associations` | Associations | whole number |
| `departures` | Departures | whole number |
| `actions` | Actions | whole number |
| `rts` | RTS | whole number |
| `cts` | CTS | whole number |
| `acks` | ACKs | whole number |
| `block_acks` | Block Acks | whole number |
| `quiet_reason` | What is left out | text |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `networks` | Networks | `ssid` (text), `bssid` (text), `vendor` (text), `band` (choice), `channel` (whole number), `heard_on` (whole number), `centre_hz` (frequency), `width_hz` (frequency), `generation` (choice), `security` (choice), `level_dbfs` (number (dBFS)), `first_heard` (time), `last_heard` (time), `beacons` (whole number), `probe_responses` (whole number), `interval` (duration (s)), `ciphers` (text), `akm` (text), `pmf` (text), `capability` (text), `details` (text), `stations` (whole number) |
| `stations` | Stations | `address` (text), `vendor` (text), `randomised` (yes or no), `bssid` (text), `link` (choice), `last_heard` (time), `level_dbfs` (number (dBFS)), `sent` (whole number), `received` (whole number), `probes` (whole number), `data` (whole number), `null_data` (whole number), `protected` (whole number), `ssids` (text), `generation` (choice), `max_rate_mbps` (number (Mbit/s)), `channel` (whole number) |
| `beacons` | Beacons | `signal_s` (number (s)), `channel` (whole number), `bssid` (text), `level_dbfs` (number (dBFS)), `bytes` (whole number), `details` (text), `format` (text), `mcs` (whole number), `aggregated` (yes or no) |
| `ofdm_rates` | OFDM frames by rate | `rate_mbps` (number (Mbit/s)), `frames` (whole number) |
| `bands` | Bands | `band` (choice), `networks` (whole number), `beacons` (whole number), `ax_be` (whole number), `wpa3_owe` (whole number), `open` (whole number), `strongest_dbfs` (number (dBFS)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `wlan`, version 4.8. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `networks`, `stations`.

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
| `stations` | whole number | always | 1.0 | Client stations in the table. |
| `ppdu_ht` | whole number | always | 1.0 | HT PPDUs whose signal field was read since the last clear. |
| `ppdu_vht` | whole number | always | 1.0 | VHT PPDUs whose signal field was read since the last clear. |
| `ppdu_he_su` | whole number | always | 1.0 | HE SU PPDUs whose signal field was read since the last clear. |
| `ppdu_he_er_su` | whole number | always | 1.0 | HE ER SU PPDUs whose signal field was read since the last clear. |
| `ppdu_he_mu` | whole number | always | 1.0 | HE MU PPDUs whose signal field was read since the last clear. |
| `ppdu_he_tb` | whole number | always | 1.0 | HE TB PPDUs whose signal field was read since the last clear. |
| `ppdu_he_bad_sig` | whole number | always | 1.0 | HE with a bad HE-SIG-A PPDUs whose signal field was read since the last clear. |
| `ppdu_eht` | whole number | always | 1.0 | EHT PPDUs whose signal field was read since the last clear. |
| `ppdu_decoded` | whole number | always | 1.0 | HT, VHT, HE and EHT PPDUs whose data was decoded. |
| `ppdu_not_decoded_streams` | whole number | always | 1.0 | PPDUs not decoded because of two or more spatial streams. |
| `ppdu_not_decoded_width` | whole number | always | 1.0 | PPDUs not decoded because of wider than 20 MHz. |
| `ppdu_not_decoded_ldpc` | whole number | always | 1.0 | PPDUs not decoded because of LDPC coding. |
| `ppdu_not_decoded_stbc` | whole number | always | 1.0 | PPDUs not decoded because of STBC. |
| `ppdu_not_decoded_multi_user` | whole number | always | 1.0 | PPDUs not decoded because of multi-user. |
| `ppdu_not_decoded_trigger_based` | whole number | always | 1.0 | PPDUs not decoded because of trigger-based. |
| `ppdu_not_decoded_he_data` | whole number | always | 1.0 | PPDUs not decoded because of HE data. |
| `ppdu_not_decoded_eht_data` | whole number | always | 1.0 | PPDUs not decoded because of EHT data. |
| `ppdu_not_decoded_signal_field` | whole number | always | 1.0 | PPDUs not decoded because of a bad signal field. |
| `ppdu_not_decoded_mcs` | whole number | always | 1.0 | PPDUs not decoded because of an MCS that is not decoded. |
| `ppdu_not_decoded_length` | whole number | always | 1.0 | PPDUs not decoded because of a length that does not fit. |
| `ppdu_not_decoded_data` | whole number | always | 1.0 | PPDUs not decoded because of data that did not decode. |
| `ppdu_not_decoded_other` | whole number | always | 1.0 | PPDUs not decoded because of another reason. |
| `ppdu_not_decoded_busy` | whole number | always | 1.0 | PPDUs not decoded because of the decoder was busy. |
| `ampdu_bad_delimiters` | whole number | always | 1.0 | A-MPDU delimiters with a bad CRC or signature since the last clear. |
| `probe_requests` | whole number | always | 1.0 | Probe requests since the last clear. |
| `authentications` | whole number | always | 1.0 | Authentication frames since the last clear. |
| `associations` | whole number | always | 1.0 | Association and reassociation frames since the last clear. |
| `departures` | whole number | always | 1.0 | Deauthentication and disassociation frames since the last clear. |
| `actions` | whole number | always | 1.0 | Action frames since the last clear. |
| `rts` | whole number | always | 1.0 | RTS frames since the last clear. |
| `cts` | whole number | always | 1.0 | CTS frames since the last clear. |
| `acks` | whole number | always | 1.0 | ACK frames since the last clear. |
| `block_acks` | whole number | always | 1.0 | Block Ack frames and requests since the last clear. |
| `quiet_reason` | text | optional | 4.7 | What is not decoded, and why: the frames that the sample rate leaves out, or the work that the decoder sheds while live input comes faster than it works. None while everything is decoded. |

### Table `networks`

Struct `WifiNetworkRow`. A row of the networks of `wifi`. The row key is the BSSID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ssid` | text | always | 1.0 | The SSID, or "(hidden)". |
| `bssid` | text | always | 1.0 | The BSSID. |
| `vendor` | text | always | 1.0 | The vendor of the BSSID, or "(local)" for a locally administered one. |
| `band` | choice | optional | 1.0 | The band. |
| `channel` | whole number | optional | 1.0 | The primary channel. No value when the frames give none and the receiver does not know its own channel. |
| `heard_on` | whole number | optional | 1.0 | The channel the frames were heard on. No value when the receiver does not know it, as for a recording. |
| `centre_hz` | frequency | optional | 1.0 | The centre frequency of the whole channel (all its 20 MHz parts). No value when the channel is not known. |
| `width_hz` | frequency | always | 1.0 | The channel width. |
| `generation` | choice | optional | 1.0 | The newest amendment the network declares. No value when the band and the elements do not tell. |
| `security` | choice | always | 1.0 | The security. |
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
| `stations` | whole number | always | 1.0 | Client stations heard with this BSSID. |

The rows are the stations or nodes of the stack. The column `ssid` names a row. Default order: the column `level_dbfs`, highest first. Time: `last_heard`.

### Table `stations`

Struct `WifiStationRow`. A row of the client stations of `wifi`. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address. |
| `vendor` | text | always | 1.0 | The vendor of the address. |
| `randomised` | yes or no | always | 1.0 | The address is locally administered (randomised). |
| `bssid` | text | optional | 1.0 | The BSSID of its access point, if a frame tied it to one. |
| `link` | choice | always | 1.0 | How far it got with its access point. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame, for a live source. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the last frame it sent. |
| `sent` | whole number | always | 1.0 | Frames it sent. |
| `received` | whole number | always | 1.0 | Frames sent to it. |
| `probes` | whole number | always | 1.0 | Probe requests it sent. |
| `data` | whole number | always | 1.0 | Data frames to or from it. |
| `null_data` | whole number | always | 1.0 | Null data frames it sent. |
| `protected` | whole number | always | 1.0 | Protected frames to or from it. |
| `ssids` | text | always | 1.0 | The SSIDs it probed for, joined with ", ". |
| `generation` | choice | optional | 1.0 | The newest amendment its frames declare. No value when none does. |
| `max_rate_mbps` | number (Mbit/s) | optional | 1.0 | The highest data rate its frames declare. |
| `channel` | whole number | optional | 1.0 | The channel of its last frame (no value when not known). |

The column `address` names a row. Default order: the column `last_heard`, highest first.

### Table `beacons`

Struct `WifiBeaconRow`. A row of the frame log of `wifi`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `channel` | whole number | optional | 1.0 | The channel the frame was heard on. No value when the receiver does not know it. |
| `bssid` | text | always | 1.0 | The BSSID. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `bytes` | whole number | always | 1.0 | The length, FCS included. |
| `details` | text | always | 1.0 | The summary. |
| `format` | text | optional | 1.0 | The PPDU format of the frame ("HT", "VHT"); no value for DSSS and non-HT OFDM. |
| `mcs` | whole number | optional | 1.0 | The MCS of an HT or VHT frame. |
| `aggregated` | yes or no | always | 1.0 | The frame came in an A-MPDU. |

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

## Sample rates and what is skipped

- From 20 Msps the stack receives everything it decodes: DSSS, non-HT OFDM, and HT and VHT PPDUs with one spatial stream
  at 20 MHz. From 43.5 Msps a 5 or 6 GHz capture is split into its 20 MHz channels.
- From 4 Msps to 20 Msps only the 802.11b (DSSS) frames of 2.4 GHz are received. Nothing is received in the 5 and 6 GHz
  bands at these rates. The result `quiet_reason` and the status line say so.
- On a busy channel the HT and VHT data fields can cost more processor time than the signal lasts. When an OFDM channel
  falls more than 50 ms behind its input, the stack stops decoding the data fields of HT and VHT PPDUs until it has
  caught up. Those PPDUs are counted from their signal fields in `ppdu_not_decoded_busy`; their MPDUs (and so their
  stations and frame counts) are missing. Non-HT frames, beacons among them, are always decoded.

## Live input that comes faster than the decoder works

The decoder measures the time each piece of live input takes against the signal time it holds. When the work is more
than 0.15 s behind, it sheds work in a fixed order, and `quiet_reason` says what is left out:

1. The chains on the channels next to the tuned one.
2. The HT and VHT data fields of the tuned channel (counted in `ppdu_not_decoded_busy`).

The DSSS and non-HT frames of the tuned channel, beacons and probe responses among them, are always decoded. After 5 s
in which the work takes less than 45 % of the signal time, one step is taken back; when that step is shed again at
once, the next try comes after twice the time, up to 80 s. A recording or a test signal sheds nothing.

## Hop

With `hop` on, the stack asks the host to step through the hop list of the band the radio is tuned to, from the tuned
channel. The hop is asked for when the first samples tell the band, so a radio tuned to 5 GHz hops on 5 GHz. With
`hop` off from the start the radio stays where the host tuned it. On 5 GHz without a list the channels 36 to 48 and
149 to 165 come first, then the radar detection channels, then 169 to 177. The results come out at least every 0.25 s
of signal whatever the dwell, and the status line names the channel being heard.

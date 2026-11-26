<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ESP-NOW

ESP-NOW frames of Espressif devices on a 2.4 GHz Wi-Fi channel: senders, peers, payloads and encrypted frames.

Stack id: `espnow`. Group: Data and IoT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.412 GHz |
| Tuning range | 2.4 GHz to 2.5 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 20 MHz |
| Signal bandwidth | 20 MHz |
| Radio filter | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:espnow` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | whole number | 1 | 1 to 14 | A 2.4 GHz Wi-Fi channel: the host tunes to it. |
| `hop` | Hop | yes or no | false |  | Asks the host to step through the channels of the hop list. |
| `dwell_ms` | Dwell | number (ms) | 300 | 100 to 5000 |  |
| `channels_24` | 2.4 GHz channels | text | empty |  | Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band; on 2.4 GHz channels 1 to 11, which every region permits. |
| `keys` | Keys | text | empty |  | 16-byte keys, 32 hex digits each, apart by spaces or commas. Each is tried as the CCMP key of a protected frame. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Scene | choice | `mixed` | `mixed` (Mixed scene), `single` (One frame) |  |
| `gen_phy` | PHY | choice | `dsss1` | `dsss1` (DSSS 1 Mbit/s), `dsss2_long` (DSSS 2 Mbit/s, long preamble), `dsss2_short` (DSSS 2 Mbit/s, short preamble), `ofdm6` (OFDM 6 Mbit/s), `ofdm12` (OFDM 12 Mbit/s) |  |
| `gen_sender` | Sender | text | `24:6F:28:11:22:33` |  | A MAC address, AA:BB:CC:DD:EE:FF. |
| `gen_peer` | Peer | text | `broadcast` |  | A MAC address, AA:BB:CC:DD:EE:FF, or broadcast. |
| `gen_payload` | Payload | text | `temp=21.5` |  | Up to 250 bytes in version 1, 1470 in version 2. |
| `gen_version` | Version | whole number | 1 | 1 to 2 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `devices` | Devices | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `action_frames` | Action frames | whole number |
| `espnow_frames` | ESP-NOW frames | whole number |
| `encrypted` | Encrypted | whole number |
| `decrypted` | Decrypted | whole number |
| `broadcast` | Broadcast | whole number |
| `heard` | Heard | yes or no |
| `channel` | Channel | whole number |
| `channel_hz` | Channel frequency | frequency |
| `channel_offset_hz` | Channel offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `input_rate_hz` | Input rate | frequency |
| `dsss_sfds` | DSSS preambles | whole number |
| `dsss_headers_ok` | DSSS headers good | whole number |
| `dsss_headers_bad` | DSSS headers bad | whole number |
| `dsss_cck_headers` | CCK headers | whole number |
| `ofdm_preambles` | OFDM preambles | whole number |
| `ofdm_signal_ok` | SIGNAL good | whole number |
| `ofdm_signal_bad` | SIGNAL bad | whole number |
| `hopping` | Hopping | yes or no |
| `note` | Note | text |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Devices | `address` (text), `vendor` (text), `espressif` (yes or no), `packets` (whole number), `encrypted` (whole number), `broadcasts` (whole number), `rate` (number (1/s)), `level_dbfs` (number (dBFS)), `channel` (whole number), `phy_rate_kbps` (whole number (kbit/s)), `last_to` (text), `last_payload` (text), `version` (whole number), `first_heard` (time), `last_heard` (time), `first_s` (number (s)), `last_s` (number (s)) |
| `frames` | Frames | `time` (time), `signal_s` (number (s)), `channel` (whole number), `phy_rate_kbps` (whole number (kbit/s)), `short_preamble` (yes or no), `from` (text), `to` (text), `seq` (whole number), `version` (whole number), `encrypted` (yes or no), `decrypted` (yes or no), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `offset_hz` (frequency), `payload` (text), `details` (text) |
| `by_rate` | Frames by PHY rate | `rate` (text), `frames` (whole number) |
| `by_channel` | Frames by channel | `channel` (whole number), `frames` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `espnow.frame` | ESP-NOW frame | `source` (text), `to` (text), `text` (text), `level_dbfs` (number (dBFS)), `family` (choice) |
| `espnow.device` | New ESP-NOW device | `source` (text), `vendor` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `wlan`, version 4.6. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `devices`.

### Result fields

Struct `EspnowFields`. The result fields of `espnow`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `devices` | whole number | always | 1.0 | Devices in the table. |
| `frames_ok` | whole number | always | 1.0 | Wi-Fi frames with a good FCS. |
| `frames_bad` | whole number | always | 1.0 | Wi-Fi frames with a bad FCS. |
| `action_frames` | whole number | always | 1.0 | Management action frames. |
| `espnow_frames` | whole number | always | 1.0 | ESP-NOW frames. |
| `encrypted` | whole number | always | 1.0 | ESP-NOW frames that were protected. |
| `decrypted` | whole number | always | 1.0 | Protected frames that a key opened. |
| `broadcast` | whole number | always | 1.0 | ESP-NOW frames to the broadcast address. |
| `heard` | yes or no | always | 1.0 | An ESP-NOW frame came in the last 5 s. |
| `channel` | whole number | optional | 1.0 | The channel nearest the radio centre. No value when the centre is not known. |
| `channel_hz` | frequency | optional | 1.0 | The centre frequency of the channel. No value when the centre is not known. |
| `channel_offset_hz` | frequency | optional | 1.0 | The offset of the channel's centre from the radio centre. No value when the centre is not known. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the last good frame. No value before it. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last good frame. No value before it. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last good frame. No value before it. |
| `input_rate_hz` | frequency | always | 1.0 | The input rate. |
| `dsss_sfds` | whole number | always | 1.0 | DSSS SFDs found. |
| `dsss_headers_ok` | whole number | always | 1.0 | DSSS PLCP headers with a good CRC. |
| `dsss_headers_bad` | whole number | always | 1.0 | DSSS PLCP headers with a bad CRC. |
| `dsss_cck_headers` | whole number | always | 1.0 | DSSS headers of a CCK rate, not decoded. |
| `ofdm_preambles` | whole number | always | 1.0 | OFDM short preambles found. |
| `ofdm_signal_ok` | whole number | always | 1.0 | OFDM SIGNAL fields with a good parity. |
| `ofdm_signal_bad` | whole number | always | 1.0 | OFDM SIGNAL fields with a bad parity or rate. |
| `hopping` | yes or no | always | 1.0 | The host steps through the hop list. |
| `note` | text | always | 1.0 | What is decoded, what is only detected, and what no public document confirms. |

### Table `devices`

Struct `EspnowDeviceRow`. A row of the devices of `espnow`. The row key is the sender address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The sender address. |
| `vendor` | text | always | 1.0 | The maker of the address, or "(local)" for a locally administered one. |
| `espressif` | yes or no | always | 1.0 | The address is one of Espressif. |
| `packets` | whole number | always | 1.0 | ESP-NOW frames heard. |
| `encrypted` | whole number | always | 1.0 | Protected frames heard. |
| `broadcasts` | whole number | always | 1.0 | Frames to the broadcast address. |
| `rate` | number (1/s) | always | 1.0 | Packets per second over the present window of 1 to 10 s; 0 after 10 s unheard. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `channel` | whole number | optional | 1.0 | The channel of the last frame. No value when the receiver does not know it. |
| `phy_rate_kbps` | whole number (kbit/s) | always | 1.0 | The PHY rate of the last frame. |
| `last_to` | text | always | 1.0 | "broadcast" or the destination of the last frame. |
| `last_payload` | text | always | 1.0 | The payload of the last frame as hex; empty when it was encrypted. |
| `version` | whole number | optional | 1.0 | The ESP-NOW version of the last readable frame. No value before one. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `first_s` | number (s) | always | 1.0 | The signal time of the first frame. |
| `last_s` | number (s) | always | 1.0 | The signal time of the last frame. |

The rows are the stations or nodes of the stack. The column `address` names a row. Default order: the column `level_dbfs`, highest first. Time: `last_heard`.

### Table `frames`

Struct `EspnowFrameRow`. A row of the frame log of `espnow`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The UTC of the frame. No value for a recording. |
| `signal_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `channel` | whole number | optional | 1.0 | The channel. No value when the receiver does not know it. |
| `phy_rate_kbps` | whole number (kbit/s) | always | 1.0 | The PHY rate. |
| `short_preamble` | yes or no | always | 1.0 | A DSSS frame with the short preamble. |
| `from` | text | always | 1.0 | The sender address. |
| `to` | text | always | 1.0 | "broadcast" or the destination address. |
| `seq` | whole number | always | 1.0 | The sequence number. |
| `version` | whole number | optional | 1.0 | The ESP-NOW version. No value for a frame that stayed encrypted. |
| `encrypted` | yes or no | always | 1.0 | The frame was protected. |
| `decrypted` | yes or no | always | 1.0 | A key opened the frame. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the frame. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the frame. |
| `payload` | text | always | 1.0 | The payload as hex; empty when it stayed encrypted. |
| `details` | text | always | 1.0 | The summary. |

The column `from` names a row.

### Table `by_rate`

Struct `EspnowRateRow`. A row of the ESP-NOW frames at each PHY rate of `espnow`. The row key is the key of the rate.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rate` | text | always | 1.0 | The PHY rate group. |
| `frames` | whole number | always | 1.0 | ESP-NOW frames at the rate. |

### Table `by_channel`

Struct `EspnowChannelRow`. A row of the ESP-NOW frames on each channel of `espnow`. The row key is the channel number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | whole number | always | 1.0 | The channel. |
| `frames` | whole number | always | 1.0 | ESP-NOW frames on the channel. |

### Event `espnow.frame`

Struct `EspnowFrameEvent`. The fields of an ESP-NOW frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender address. |
| `to` | text | always | 1.0 | "broadcast" or the destination address. |
| `text` | text | always | 1.0 | The summary. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `family` | choice | always | 1.0 | The family of the source. |

### Event `espnow.device`

Struct `EspnowDeviceEvent`. The fields of a device heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender address. |
| `vendor` | text | always | 1.0 | The maker of the address. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_espnow_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_espnow_get_channel` | `sdrkit_espnow_set_channel` |
| `hop` | `sdrkit_espnow_get_hop` | `sdrkit_espnow_set_hop` |
| `dwell_ms` | `sdrkit_espnow_get_dwell_ms` | `sdrkit_espnow_set_dwell_ms` |
| `channels_24` | `sdrkit_espnow_get_channels_24` | `sdrkit_espnow_set_channels_24` |
| `keys` | `sdrkit_espnow_get_keys` | `sdrkit_espnow_set_keys` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_espnow_gen_get_scene` | `sdrkit_espnow_gen_set_scene` |
| `gen_phy` | `sdrkit_espnow_gen_get_phy` | `sdrkit_espnow_gen_set_phy` |
| `gen_sender` | `sdrkit_espnow_gen_get_sender` | `sdrkit_espnow_gen_set_sender` |
| `gen_peer` | `sdrkit_espnow_gen_get_peer` | `sdrkit_espnow_gen_set_peer` |
| `gen_payload` | `sdrkit_espnow_gen_get_payload` | `sdrkit_espnow_gen_set_payload` |
| `gen_version` | `sdrkit_espnow_gen_get_version` | `sdrkit_espnow_gen_set_version` |

## Try it

```sh
sdrkit run espnow --source synth:espnow
sdrkit run espnow --source iiod:192.168.2.1 --freq 2412
```
<!-- generated: end -->

## Description

ESP-NOW is the connectionless protocol of Espressif devices. A frame is an IEEE 802.11 management action frame (frame control
D0, or E0 for action no ack) of category 127 (vendor specific) with the Espressif organisation identifier 18 FE 34, four
random bytes, and one or more vendor-specific elements (id 221, type 4). Each element carries a version byte (bits 3 to 0 the
version, bit 4 "more data") and a body of 0 to 250 bytes; version 2 joins the bodies of all elements of a frame, up to 1470
bytes.

The stack runs the DSSS and non-HT OFDM receivers of the Wi-Fi stack at 20 Msps on the 2.4 GHz channel nearest the radio
centre (within 12 MHz), and keeps every frame with a good FCS that is an ESP-NOW frame. Decoded: clear frames at DSSS 1 and
2 Mbit/s (long and short preamble) and at OFDM 6 Mbit/s and above, versions 1 and 2. A protected frame (CCMP, IEEE Std
802.11-2020 clause 12.5.3) is tried with each key of `keys`, with the management nonce flag and without; the first MIC that
holds opens it. A protected frame that no key opens is only detected: it is listed as encrypted, with its addresses, sequence
number and packet number, when its sender has an Espressif address or its address 3 is the broadcast address. Other action
frames are counted only. CCK frames at 5.5 and 11 Mbit/s are counted by the receiver and not decoded.

A key is always a setting of the user: the stack holds none, searches for none, and does not keep `keys` in its saved
state of the tables.

Not confirmed by a public document: the maker's public guide names a primary master key and local master keys per peer, and
does not state how the key of a frame follows from them; the stack takes a key entered here as the CCMP key itself. The CCMP
header, nonce and additional data as applied to these frames, subtype 14 and channel 1 as the usual channel are from the
specification. The CCMP code holds the test vector of IEEE Std 802.11-2012 Annex M.6.4. Not tried on a real signal.

Standard: IEEE Std 802.11-2020 clauses 9, 12.5.3, 16 and 17; the maker's public ESP-NOW guide.

## Tuning: notes

Below 20 Msps the stack says "ESP-NOW needs a sample rate of at least 20 Msps". The channels that each region permits are
data: `data/wlan/wifi_domains.csv` lists the 2.4 GHz channels per regulatory domain, and a host sets `channels_24` of the
stack from it. `channel` and `hop` ask the host to tune; the stack never tunes the radio itself.

## Result tables: notes

The devices are keyed by the sender address, at most 300; the one heard longest ago goes first. The rate of a device is the
frames of a window of 1 to 10 s over the window's length; the window starts again every 10 s, and the rate is 0 after 10 s
unheard. The frame log keeps the newest 500 frames. The summary of a frame reads `v1, 9 bytes to broadcast: "temp=21.5"` or
`encrypted (CCMP, PN 1) to 24:6F:28:11:22:33`; the payload is quoted when it has at least 3 bytes and at least 80 % are
printable. Frames with a bad FCS are counted in `frames_bad` and never listed.

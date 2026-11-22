<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Bluetooth Classic

Bluetooth BR/EDR discovery over a wide capture: devices and piconets by the LAP of every access code; inquiry responses give the address, class of device and clock.

Stack id: `btclassic`. Group: Data and IoT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.441 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 20 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:btclassic` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `sync_errors` | Sync word errors allowed | whole number (bits) | 2 | 0 to 4 | How many of the 64 sync-word bits may differ from the sync word of the LAP they carry. |
| `threshold_db` | Burst threshold | number (dB) | 10 | 6 to 20 | How far above the noise floor of a 1 MHz channel a burst must rise. |
| `wide_channels` | Wideband from | whole number (channels) | 5 | 3 to 10 | Adjacent active channels of similar level, this many or more, are a wideband signal (Wi-Fi) and are skipped. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_wideband` | Test signal wideband bursts | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `access_codes` | Access codes | whole number |
| `bursts_without_code` | Bursts without an access code | whole number |
| `inquiry_ids` | Inquiry ID packets | whole number |
| `inquiry_responses` | Inquiry responses | whole number |
| `bursts_analysed` | Bursts analysed | whole number |
| `bursts_wideband` | Wideband bursts | whole number |
| `channels_covered` | Channels covered | whole number |
| `channels_relative` | Channel numbers relative | yes or no |
| `cfo_hz` | Carrier offset | number (Hz) |
| `snr_db` | SNR | number (dB) |
| `input_level_dbfs` | Input level | number (dBFS) |
| `state` | State | whole number |
| `devices` | Devices | whole number |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Devices | `lap` (text), `kind` (text), `address` (text), `class` (text), `major_class` (text), `class_raw` (whole number), `clock` (whole number (1.25 ms)), `level_dbfs` (number (dBFS)), `level_history` (text (dBFS)), `first_heard` (time), `last_heard` (time), `packets` (whole number), `last_channel_mhz` (number (MHz)), `channels_heard` (text) |
| `packets` | Packets | `utc` (time), `time_s` (number (s)), `channel_mhz` (number (MHz)), `lap` (text), `level_dbfs` (number (dBFS)), `sync_errors` (whole number), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `btclassic.device_new` | New Bluetooth device | `lap` (text), `kind` (text), `family` (choice) |
| `btclassic.inquiry_response` | Bluetooth inquiry response | `address` (text), `class` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear_devices` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `wlan`, version 2.1. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `BtclassicFields`. The result fields of `btclassic`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `access_codes` | whole number | always | 1.0 | Good access codes. |
| `bursts_without_code` | whole number | always | 1.0 | Bursts analysed without an access code. |
| `inquiry_ids` | whole number | always | 1.0 | GIAC and LIAC ID packets. |
| `inquiry_responses` | whole number | always | 1.0 | FHS packets read. |
| `bursts_analysed` | whole number | always | 1.0 | Bursts analysed. |
| `bursts_wideband` | whole number | always | 1.0 | Wideband bursts skipped. |
| `channels_covered` | whole number | always | 1.0 | The channels inside the capture. |
| `channels_relative` | yes or no | always | 1.0 | The channel numbers count from a capture centre taken as channel 39 (the radio centre is not known). |
| `cfo_hz` | number (Hz) | optional | 1.0 | The carrier offset of the last good packet. No value before it. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last good packet. No value before it. |
| `input_level_dbfs` | number (dBFS) | optional | 1.0 | The input level. No value before the first input. |
| `state` | whole number | always | 1.0 | 1 when an access code was found in the last 5 s of signal. |
| `devices` | whole number | always | 1.0 | Devices in the table. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `devices`

Struct `BtclassicDeviceRow`. A row of the devices of `btclassic`. The row key is the LAP.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `lap` | text | always | 1.0 | The LAP as hex text. |
| `kind` | text | always | 1.0 | What the device is. |
| `address` | text | optional | 1.0 | The address from an FHS packet. |
| `class` | text | optional | 1.0 | The class of device as text. |
| `major_class` | text | optional | 1.0 | The major class name. |
| `class_raw` | whole number | optional | 1.0 | The class of device value. |
| `clock` | whole number (1.25 ms) | optional | 1.0 | CLK27..CLK2 from the FHS packet. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the last packet. |
| `level_history` | text (dBFS) | always | 1.0 | The last levels. |
| `first_heard` | time | optional | 1.0 | The UTC of the first packet. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet. |
| `packets` | whole number | always | 1.0 | Packets heard. |
| `last_channel_mhz` | number (MHz) | always | 1.0 | The channel of the last packet. |
| `channels_heard` | text | always | 1.0 | The channels heard, as hex: bit k is channel k. |

The column `address` names a row.

### Table `packets`

Struct `BtclassicPacketRow`. A row of the packet log of `btclassic`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the packet, for a live source. |
| `time_s` | number (s) | always | 1.0 | The signal time of the packet. |
| `channel_mhz` | number (MHz) | always | 1.0 | The channel of the packet. |
| `lap` | text | always | 1.0 | The LAP as hex text. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the packet. |
| `sync_errors` | whole number | always | 1.0 | Sync-word bits that differed. |
| `text` | text | always | 1.0 | What the packet is. |

### Event `btclassic.device_new`

Struct `BtclassicDeviceEvent`. The fields of a LAP heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `lap` | text | always | 1.0 | The LAP as hex text. |
| `kind` | text | always | 1.0 | What the device is. |
| `family` | choice | always | 1.0 | The family of the source. |

### Event `btclassic.inquiry_response`

Struct `BtclassicInquiryEvent`. The fields of an inquiry response.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address. |
| `class` | text | always | 1.0 | The class of device as text. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear_devices` | `sdrkit_btclassic_clear_devices` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `sync_errors` | `sdrkit_btclassic_get_sync_errors` | `sdrkit_btclassic_set_sync_errors` |
| `threshold_db` | `sdrkit_btclassic_get_threshold_db` | `sdrkit_btclassic_set_threshold_db` |
| `wide_channels` | `sdrkit_btclassic_get_wide_channels` | `sdrkit_btclassic_set_wide_channels` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_wideband` | `sdrkit_btclassic_gen_get_wideband` | `sdrkit_btclassic_gen_set_wideband` |

## Try it

```sh
sdrkit run btclassic --source synth:btclassic
sdrkit run btclassic --source iiod:192.168.2.1 --freq 2441
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

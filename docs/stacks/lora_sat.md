<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LoRa satellites

LoRa frames from and to satellites, with the Doppler shift and Doppler rate of each pass. Receive only.

Stack id: `lora_sat`. Group: Satellite.

Signal types: LoRa.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 436.8 MHz |
| Tuning range | 137 MHz to 1 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 250 kHz |
| Offset from the signal | 300 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:lora_sat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `follow_dial` | Follow the dial | yes or no | true |  | When the dial is within 15 kHz of a preset, run its SF, bandwidth and sync. |
| `auto_sf` | Search SF | yes or no | true |  | SF7 to SF12 (a preset's SF overrides while following the dial). |
| `sf` | SF | choice | `11` | `7`, `8`, `9`, `10`, `11`, `12` |  |
| `auto_bw` | Search bandwidth | yes or no | false |  | 125, 250 and 500 kHz. |
| `bw_khz` | Bandwidth, kHz | choice | `125` | `41.7` (41.7 kHz), `62.5` (62.5 kHz), `125` (125 kHz), `250` (250 kHz), `500` (500 kHz) |  |
| `sync` | Sync word | choice | `both` | `both` (Try 0x12 and 0x34), `0x12`, `0x34`, `other` (Other) |  |
| `sync_other` | Other sync word | whole number | 18 | 0 to 255 |  |
| `preamble` | Preamble | whole number (symbols) | 8 | 6 to 65535 |  |
| `doppler_max_hz` | Doppler allowance | frequency | 0 Hz | 0 Hz to 60 kHz | 0: from the dial. |
| `track` | Track the carrier | yes or no | true |  | Follow the carrier through each frame. |
| `offset_hz` | Channel offset | frequency | 0 Hz | -500 kHz to 500 kHz |  |
| `pass_satellite` | Satellite overhead | text | empty |  | The pass planner's (the host's) name of the satellite overhead. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_band` | Pass | choice | `436.8` | `436.8` (436.8 MHz, SF11), `868.1` (868.1 MHz, SF10), `137.055` (137.055 MHz, SF8, 41.7 kHz) |  |
| `gen_sf` | SF | choice | `0` | `0` (The pass's), `7`, `8`, `9`, `10`, `11`, `12` |  |
| `gen_bw_khz` | Bandwidth, kHz | choice | `0` | `0` (The pass's), `41.7` (41.7 kHz), `62.5` (62.5 kHz), `125` (125 kHz), `250` (250 kHz), `500` (500 kHz) |  |
| `gen_pass_doppler` | Doppler of the pass | yes or no | true |  |  |
| `gen_start_s` | Seconds into the pass | duration (s) | 0 s | 0 to 600 |  |
| `gen_sync` | Sync word | whole number | 18 | 0 to 255 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `satellite` | Satellite | text |
| `preset` | Preset | text |
| `demodulators` | Demodulators | whole number |
| `doppler_max_hz` | Doppler allowance | frequency |
| `doppler_rate_max_hz_s` | Doppler rate allowance | number (Hz/s) |
| `dropped` | Dropped | whole number |
| `receiver.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `heard_at` (time), `sf` (whole number), `bw_hz` (frequency), `cr` (whole number), `crc` (choice), `sync` (whole number), `bytes` (whole number), `snr_db` (number (dB)), `doppler_hz` (frequency), `doppler_rate_hz_s` (number (Hz/s)), `freq_hz` (frequency), `satellite` (text), `payload` (bytes), `text` (text) |
| `demodulators` | Demodulators | `sf` (whole number), `bw_hz` (frequency), `preambles` (whole number), `sync_failed` (whole number), `header_failed` (whole number), `frames` (whole number), `crc_failed` (whole number) |
| `demodulator_syncs` | Sync words of the demodulators | `demodulator` (text), `sync` (whole number) |
| `frames_per_sf` | Frames by SF | `sf` (whole number), `frames` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `doppler` | Doppler curve | s | Hz |

## Events

| Kind | Label | Fields |
|---|---|---|
| `lora_sat.frame` | LoRa frame | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `sf` (whole number), `payload` (bytes), `doppler_hz` (frequency), `doppler_rate_hz_s` (number (Hz/s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.2. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `LoraSatFields`. The result fields of the LoRa satellite stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `satellite` | text | always | 1.0 | The pass planner's name of the satellite overhead. |
| `preset` | text | optional | 1.0 | The name of the preset that runs. |
| `demodulators` | whole number | always | 1.0 | The number of detectors without Doppler tracking. |
| `doppler_max_hz` | frequency | always | 1.0 | The largest Doppler shift expected. |
| `doppler_rate_max_hz_s` | number (Hz/s) | always | 1.0 | The largest Doppler rate expected. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `frames`

Struct `LoraSatFrameRow`. A frame of the log. The row key is the sequence number of the frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `heard_at` | time | optional | 1.0 | The wall-clock time a frame was heard (live radios only). |
| `sf` | whole number | always | 1.0 | The spreading factor of a frame. |
| `bw_hz` | frequency | always | 1.0 | The bandwidth of a frame. |
| `cr` | whole number | always | 1.0 | The coding rate of a frame: the denominator n of 4/n. |
| `crc` | choice | always | 1.0 | The CRC of a frame. |
| `sync` | whole number | always | 1.0 | The sync word of a frame. |
| `bytes` | whole number | always | 1.0 | The payload length of a frame. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `doppler_hz` | frequency | always | 1.0 | The carrier offset from the dial at the first data symbol. |
| `doppler_rate_hz_s` | number (Hz/s) | optional | 1.0 | The carrier rate during a frame. |
| `freq_hz` | frequency | always | 1.0 | The frequency of the frame. |
| `satellite` | text | optional | 1.0 | The pass planner's name of the satellite overhead. |
| `payload` | bytes | always | 1.0 | The payload of a frame. |
| `text` | text | always | 1.0 | The payload as printable text. |

### Table `demodulators`

Struct `LoraSatDemodulatorRow`. A demodulator running now. The row key is its place in the list ("000").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sf` | whole number | always | 1.0 | The spreading factor of a frame. |
| `bw_hz` | frequency | always | 1.0 | The bandwidth of a frame. |
| `preambles` | whole number | always | 1.0 | Detector counters: preambles found. |
| `sync_failed` | whole number | always | 1.0 | Detector counters: preambles without a sync word or SFD. |
| `header_failed` | whole number | always | 1.0 | Detector counters: headers that failed. |
| `frames` | whole number | always | 1.0 | Detector counters: frames decoded, good or not. |
| `crc_failed` | whole number | always | 1.0 | Detector counters: frames whose CRC failed. |

### Table `demodulator_syncs`

Struct `LoraSatDemodulatorSyncRow`. A sync word of a demodulator. The row key is the demodulator and the sync word ("000/18").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `demodulator` | text | always | 1.0 | The detector a sync word belongs to. |
| `sync` | whole number | always | 1.0 | The sync word of a frame. |

### Table `frames_per_sf`

Struct `LoraSatSfRow`. The frames of one spreading factor. The row key is the SF ("07").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sf` | whole number | always | 1.0 | The spreading factor of a frame. |
| `frames` | whole number | always | 1.0 | Detector counters: frames decoded, good or not. |

### Event `lora_sat.frame`

Struct `LoraSatFrameEvent`. A frame with a good CRC. The source is the satellite overhead, the text the payload as printable text.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The satellite overhead. |
| `text` | text | always | 1.0 | The payload as printable text. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `sf` | whole number | always | 1.0 | The spreading factor of a frame. |
| `payload` | bytes | always | 1.0 | The payload of a frame. |
| `doppler_hz` | frequency | always | 1.0 | The carrier offset from the dial at the first data symbol. |
| `doppler_rate_hz_s` | number (Hz/s) | optional | 1.0 | The carrier rate during a frame. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_lora_sat_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `follow_dial` | `sdrkit_lora_sat_get_follow_dial` | `sdrkit_lora_sat_set_follow_dial` |
| `auto_sf` | `sdrkit_lora_sat_get_auto_sf` | `sdrkit_lora_sat_set_auto_sf` |
| `sf` | `sdrkit_lora_sat_get_sf` | `sdrkit_lora_sat_set_sf` |
| `auto_bw` | `sdrkit_lora_sat_get_auto_bw` | `sdrkit_lora_sat_set_auto_bw` |
| `bw_khz` | `sdrkit_lora_sat_get_bw_khz` | `sdrkit_lora_sat_set_bw_khz` |
| `sync` | `sdrkit_lora_sat_get_sync` | `sdrkit_lora_sat_set_sync` |
| `sync_other` | `sdrkit_lora_sat_get_sync_other` | `sdrkit_lora_sat_set_sync_other` |
| `preamble` | `sdrkit_lora_sat_get_preamble` | `sdrkit_lora_sat_set_preamble` |
| `doppler_max_hz` | `sdrkit_lora_sat_get_doppler_max_hz` | `sdrkit_lora_sat_set_doppler_max_hz` |
| `track` | `sdrkit_lora_sat_get_track` | `sdrkit_lora_sat_set_track` |
| `offset_hz` | `sdrkit_lora_sat_get_offset_hz` | `sdrkit_lora_sat_set_offset_hz` |
| `pass_satellite` | `sdrkit_lora_sat_get_pass_satellite` | `sdrkit_lora_sat_set_pass_satellite` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_band` | `sdrkit_lora_sat_gen_get_band` | `sdrkit_lora_sat_gen_set_band` |
| `gen_sf` | `sdrkit_lora_sat_gen_get_sf` | `sdrkit_lora_sat_gen_set_sf` |
| `gen_bw_khz` | `sdrkit_lora_sat_gen_get_bw_khz` | `sdrkit_lora_sat_gen_set_bw_khz` |
| `gen_pass_doppler` | `sdrkit_lora_sat_gen_get_pass_doppler` | `sdrkit_lora_sat_gen_set_pass_doppler` |
| `gen_start_s` | `sdrkit_lora_sat_gen_get_start_s` | `sdrkit_lora_sat_gen_set_start_s` |
| `gen_sync` | `sdrkit_lora_sat_gen_get_sync` | `sdrkit_lora_sat_gen_set_sync` |

## Try it

```sh
sdrkit run lora_sat --source synth:lora_sat
sdrkit run lora_sat --source iiod:192.168.2.1 --freq 436.8
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

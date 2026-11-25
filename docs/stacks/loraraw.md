<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LoRa (raw)

Plain point-to-point LoRa frames as bytes at every spreading factor and bandwidth; receive and transmit.

Stack id: `loraraw`. Group: IoT and mesh.

Signal types: LoRa.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 137 MHz to 2.5 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 250 kHz |
| Offset from the signal | 300 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:loraraw` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `auto_sf` | Search SF7 to SF12 | yes or no | true |  |  |
| `sf` | Spreading factor | whole number | 7 | 5 to 12 |  |
| `auto_bw` | Search 125, 250 and 500 kHz | yes or no | true |  |  |
| `bw` | Bandwidth | choice | `125000` | `7812.5` (7.8 kHz), `10417` (10.4 kHz), `15625` (15.6 kHz), `20833` (20.8 kHz), `31250` (31.25 kHz), `41667` (41.7 kHz), `62500` (62.5 kHz), `125000` (125 kHz), `250000` (250 kHz), `500000` (500 kHz), `812500` (812.5 kHz (2.4 GHz)), `1625000` (1625 kHz (2.4 GHz)) |  |
| `implicit` | Implicit header | yes or no | false |  |  |
| `cr` | Coding rate | whole number | 5 | 5 to 8 | 5 is 4/5, 8 is 4/8. |
| `crc` | CRC | yes or no | true |  |  |
| `length` | Length | whole number (B) | 16 | 1 to 255 |  |
| `sync` | Sync word | choice | `0x12` | `0x12` (0x12 private), `0x34` (0x34 LoRaWAN), `0x2B` (0x2B Meshtastic), `any` (Any of these), `custom` (Custom) |  |
| `sync_custom` | Custom sync word | whole number | 18 | 0 to 255 | Used when the sync word is custom. |
| `preamble` | Preamble | whole number | 8 | 6 to 65 |  |
| `ldro` | LDRO | choice | `auto` | `auto` (Automatic), `off` (Off), `on` (On) |  |
| `offset` | Offset from the dial | frequency | 0 Hz | -1 MHz to 1 MHz |  |
| `tx_repeat_s` | Repeat every | duration (s) | 0 s | 0 to 3600 | 0 sends each frame once. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `jump_search` | Lost-sample search | yes or no | true |  |  |
| `clock0` | Computer clock at signal 0 | number (s) | 0 | 0 to 1000000000000 | Seconds since 1970 at the first sample. 0 when the times follow the signal. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_implicit` | Implicit scene | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `preambles` | Preambles | whole number |
| `header_failed` | Headers failed | whole number |
| `searching` | Searching | text |
| `dial` | Dial | frequency |
| `tx_sent` | Sent | whole number |
| `sf5` | SF5 frames | whole number |
| `sf6` | SF6 frames | whole number |
| `sf7` | SF7 frames | whole number |
| `sf8` | SF8 frames | whole number |
| `sf9` | SF9 frames | whole number |
| `sf10` | SF10 frames | whole number |
| `sf11` | SF11 frames | whole number |
| `sf12` | SF12 frames | whole number |
| `bw125` | 125 kHz frames | whole number |
| `bw250` | 250 kHz frames | whole number |
| `bw500` | 500 kHz frames | whole number |
| `snr_db` | SNR | number (dB) |
| `bad_samples` | Bad samples | whole number |
| `modulate.tx_bursts` | Bursts written | whole number |
| `modulate.tx_samples` | Samples written | whole number |
| `modulate.tx_dropped` | Frames dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `sf` (whole number), `bw` (frequency), `cr` (whole number), `len` (whole number (B)), `crc` (choice), `snr_db` (number (dB)), `level_dbfs` (number (dBFS)), `cfo` (frequency), `payload` (bytes), `text` (text) |
| `decoders` | Decoders | `sf` (whole number), `bw` (frequency), `syncs` (text), `in_band` (yes or no), `preambles` (whole number), `frames` (whole number), `crc_errors` (whole number), `header_failed` (whole number), `sync_failed` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `lora.packet` | LoRa packet | `net` (choice), `modulation` (choice), `snr_db` (number (dB)), `sf` (whole number), `payload` (bytes), `crc_ok` (yes or no), `source` (text), `target` (text), `devaddr` (text), `iq_inverted` (yes or no), `crc` (yes or no), `bw` (frequency), `cr` (whole number), `implicit` (yes or no), `sync` (whole number), `level_dbfs` (number (dBFS)), `family` (choice) |
| `tx.request` | Frame to send | `id` (whole number), `what` (choice), `payload` (bytes), `freq_hz` (frequency), `not_before_s` (number (s)), `air_time` (duration (s)), `message_key` (whole number), `setting` (text), `afsk_baud` (whole number (bit/s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `reset` | Reset |  |  | Forgets the signal and makes the bank again. |
| `send` | Send a frame | `payload` (bytes, required) |  | Sends the bytes with the LoRa setting of the stack. Transmit must be on. |

## Typed interface

The stack is in the family `lora`, version 5.0. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

Key readings: `frames_ok`, `frames_bad`, `snr_db`.

### Result fields

Struct `RawFields`. The result fields of the raw LoRa stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the bank reports. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame, while frames come. |
| `frames_ok` | whole number | always | 1.0 | Frames with a good CRC. |
| `frames_bad` | whole number | always | 1.0 | Frames whose CRC failed. |
| `preambles` | whole number | always | 1.0 | Frame preambles seen. |
| `header_failed` | whole number | always | 1.0 | Headers that failed their checksum. |
| `searching` | text | always | 1.0 | What is searched. |
| `dial` | frequency | always | 1.0 | The dial. |
| `tx_sent` | whole number | always | 1.0 | Frames sent. |
| `sf5` | whole number | always | 1.0 | Frames at SF5. |
| `sf6` | whole number | always | 1.0 | Frames at SF6. |
| `sf7` | whole number | always | 1.0 | Frames at SF7. |
| `sf8` | whole number | always | 1.0 | Frames at SF8. |
| `sf9` | whole number | always | 1.0 | Frames at SF9. |
| `sf10` | whole number | always | 1.0 | Frames at SF10. |
| `sf11` | whole number | always | 1.0 | Frames at SF11. |
| `sf12` | whole number | always | 1.0 | Frames at SF12. |
| `bw125` | whole number | always | 1.0 | Frames at 125 kHz. |
| `bw250` | whole number | always | 1.0 | Frames at 250 kHz. |
| `bw500` | whole number | always | 1.0 | Frames at 500 kHz. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last frame, while frames come. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `frames`

Struct `RawFrameRow`. A row of the raw frame log. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `sf` | whole number | always | 1.0 | The spreading factor. |
| `bw` | frequency | always | 1.0 | The bandwidth. |
| `cr` | whole number | always | 1.0 | The coding rate denominator. |
| `len` | whole number (B) | always | 1.0 | The payload length. |
| `crc` | choice | always | 1.0 | What the CRC says. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `cfo` | frequency | always | 1.0 | The carrier offset. |
| `payload` | bytes | always | 1.0 | The payload. |
| `text` | text | always | 1.0 | The payload as text for people. |

The rows are a log: they arrive over time. Time: `time_s`.

### Table `decoders`

Struct `RawDecoderRow`. A row of the raw decoders. The row key is the tag of the detector.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sf` | whole number | always | 1.0 | The spreading factor. |
| `bw` | frequency | always | 1.0 | The bandwidth. |
| `syncs` | text | always | 1.0 | The sync words searched, in words. |
| `in_band` | yes or no | always | 1.0 | The channel lies inside the captured band. |
| `preambles` | whole number | always | 1.0 | Preambles. |
| `frames` | whole number | always | 1.0 | Frames decoded. |
| `crc_errors` | whole number | always | 1.0 | CRC errors. |
| `header_failed` | whole number | always | 1.0 | Headers failed. |
| `sync_failed` | whole number | always | 1.0 | Sync words failed. |

### Event `lora.packet`

Struct `LoraPacketEvent`. The fields of one LoRa or FSK frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol of the frame. |
| `modulation` | choice | always | 1.0 | How the frame was modulated. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `sf` | whole number | optional | 1.0 | The spreading factor, none for FSK. |
| `payload` | bytes | always | 1.0 | The payload. |
| `crc_ok` | yes or no | always | 1.0 | The CRC is right, or the frame has none. |
| `source` | text | optional | 1.0 | The sender, if the protocol names one. |
| `target` | text | optional | 1.0 | The addressee, if the protocol names one. |
| `devaddr` | text | optional | 1.0 | LoRaWAN: the device address, eight hex digits. |
| `iq_inverted` | yes or no | optional | 1.0 | LoRaWAN: the IQ is inverted (a downlink). |
| `crc` | yes or no | optional | 1.0 | The frame has a CRC. |
| `bw` | frequency | optional | 1.0 | The bandwidth. |
| `cr` | whole number | optional | 1.0 | The coding rate denominator. |
| `implicit` | yes or no | optional | 1.0 | The header is implicit. |
| `sync` | whole number | optional | 1.0 | The sync word. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the frame. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `tx.request`

Struct `TxRequestEvent`. The fields of a frame that the stack requests the host to send.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | A number for the reply. |
| `what` | choice | always | 1.0 | What the frame is. |
| `payload` | bytes | always | 1.0 | The payload. |
| `freq_hz` | frequency | always | 1.0 | The frequency to send on. |
| `not_before_s` | number (s) | always | 1.0 | The earliest send time, signal seconds. |
| `air_time` | duration (s) | optional | 1.0 | The time on air of a LoRa frame. |
| `message_key` | whole number | optional | 1.0 | The own message that the frame carries. |
| `setting` | text | optional | 1.0 | The LoRa setting in words. |
| `afsk_baud` | whole number (bit/s) | optional | 1.0 | The packet speed of an AX.25 frame. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_loraraw_clear` | none | none |
| `reset` | `sdrkit_loraraw_reset` | none | none |
| `send` | `sdrkit_loraraw_send` | `sdrkit_loraraw_raw_send_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `auto_sf` | `sdrkit_loraraw_get_auto_sf` | `sdrkit_loraraw_set_auto_sf` |
| `sf` | `sdrkit_loraraw_get_sf` | `sdrkit_loraraw_set_sf` |
| `auto_bw` | `sdrkit_loraraw_get_auto_bw` | `sdrkit_loraraw_set_auto_bw` |
| `bw` | `sdrkit_loraraw_get_bw` | `sdrkit_loraraw_set_bw` |
| `implicit` | `sdrkit_loraraw_get_implicit` | `sdrkit_loraraw_set_implicit` |
| `cr` | `sdrkit_loraraw_get_cr` | `sdrkit_loraraw_set_cr` |
| `crc` | `sdrkit_loraraw_get_crc` | `sdrkit_loraraw_set_crc` |
| `length` | `sdrkit_loraraw_get_length` | `sdrkit_loraraw_set_length` |
| `sync` | `sdrkit_loraraw_get_sync` | `sdrkit_loraraw_set_sync` |
| `sync_custom` | `sdrkit_loraraw_get_sync_custom` | `sdrkit_loraraw_set_sync_custom` |
| `preamble` | `sdrkit_loraraw_get_preamble` | `sdrkit_loraraw_set_preamble` |
| `ldro` | `sdrkit_loraraw_get_ldro` | `sdrkit_loraraw_set_ldro` |
| `offset` | `sdrkit_loraraw_get_offset` | `sdrkit_loraraw_set_offset` |
| `jump_search` | `sdrkit_loraraw_get_jump_search` | `sdrkit_loraraw_set_jump_search` |
| `tx_repeat_s` | `sdrkit_loraraw_get_tx_repeat_s` | `sdrkit_loraraw_set_tx_repeat_s` |
| `clock0` | `sdrkit_loraraw_get_clock0` | `sdrkit_loraraw_set_clock0` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_implicit` | `sdrkit_loraraw_gen_get_implicit` | `sdrkit_loraraw_gen_set_implicit` |

## Try it

```sh
sdrkit run loraraw --source synth:loraraw
sdrkit run loraraw --source <driver>:<address> --freq 0
```
<!-- generated: end -->

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Railway CIR data

The data channel of the Chinese railway train radio: end-of-train device frames (KLW) or dispatch frames (TDCS) with the train, the locomotive, the brake pipe pressure, the kilometre post and the speed.

Stack id: `railway_cir`. Group: Cellular and railway.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 450 MHz |
| Tuning range | 400 MHz to 900 MHz |
| Sample rate | 960 kHz |
| Smallest sample rate | 192 kHz |
| Signal bandwidth | 35 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 150 kHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `klw` | End-of-train device (KLW) | 450 MHz |
| `tdcs` | Dispatch (TDCS) | 450 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:railway_cir` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `history` | Messages kept | whole number | 400 | 50 to 2000 | The oldest message is dropped beyond this count. |
| `locomotives` | Locomotive classes | text | empty |  | The class names of the locomotive type codes, one code and its name a line (the data file data/cell/locomotives.txt). Without it a locomotive shows its type code. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_invert_bits` | Invert the air bits | yes or no | false |  |  |
| `gen_deviation_hz` | Tone deviation | number (Hz) | 2500 | 300 to 6000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `messages` | Messages | whole number |
| `syncs` | Sync words | whole number |
| `failed_checks` | Failed checks | whole number |
| `carrier_offset_hz` | Carrier offset | number (Hz) |
| `level_db` | Level | number (dB) |
| `state` | State | choice |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `received` (time), `protocol` (text), `train` (text), `locomotive` (text), `action` (text), `tail_id` (text), `pressure_kpa` (number (kPa)), `speed_kmh` (number (km/h)), `km_post_km` (number (km)), `test_frame` (yes or no), `cars` (whole number), `corrections` (whole number), `raw` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `railway_cir.klw` | KLW message | `text` (text), `locomotive` (text), `reserved` (whole number), `action_code` (whole number), `action` (text), `parameter` (text), `pressure_kpa` (number (kPa)), `tail_id` (text), `extra` (text), `clock` (text), `corrections` (whole number), `raw` (text) |
| `railway_cir.tdcs` | TDCS message | `text` (text), `train` (text), `locomotive` (text), `loco_code` (whole number), `loco_serial` (whole number), `km_post_km` (number (km)), `increasing` (yes or no), `test_frame` (yes or no), `speed_kmh` (number (km/h)), `weight_t` (number (t)), `length` (number), `cars` (whole number), `station` (whole number), `track` (whole number), `actual_track` (whole number), `driver` (whole number), `signal_type` (whole number), `semaphore_type` (whole number), `role` (whole number), `corrections` (whole number), `raw` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `cell`, version 1.4. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `RailwayCirFields`. The result fields of `railway_cir`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The summary of the last message. |
| `messages` | whole number | always | 1.0 | Messages decoded. |
| `syncs` | whole number | always | 1.0 | Sync words found. |
| `failed_checks` | whole number | always | 1.0 | Readings that failed: CRC failures, rejections and ambiguous frames. |
| `carrier_offset_hz` | number (Hz) | optional | 1.0 | The carrier offset of the channel. |
| `level_db` | number (dB) | optional | 1.0 | The level of the channel. |
| `state` | choice | always | 1.0 | What the receiver is doing. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `RailwayCirMessageRow`. A row of the message list of `railway_cir`, newest first. The row key is the sequence number of the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `received` | time | always | 1.0 | The wall time of the reception. |
| `protocol` | text | always | 1.0 | The protocol: `KLW` or `TDCS`. |
| `train` | text | optional | 1.0 | The train (TDCS). |
| `locomotive` | text | always | 1.0 | The locomotive. |
| `action` | text | optional | 1.0 | The action (KLW). |
| `tail_id` | text | optional | 1.0 | The tail device id (KLW). |
| `pressure_kpa` | number (kPa) | optional | 1.0 | The brake pipe pressure (KLW). |
| `speed_kmh` | number (km/h) | optional | 1.0 | The speed (TDCS). |
| `km_post_km` | number (km) | optional | 1.0 | The kilometre post (TDCS). |
| `test_frame` | yes or no | always | 1.0 | The frame marks itself as a test frame (TDCS). |
| `cars` | whole number | optional | 1.0 | The car count (TDCS). |
| `corrections` | whole number | always | 1.0 | The bits corrected. |
| `raw` | text | always | 1.0 | The raw frame as hex. |

The rows are a log: they arrive over time. Time: `received`.

### Event `railway_cir.klw`

Struct `RailwayCirKlwEvent`. A message of the end-of-train device.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The summary: `KLW <action> <locomotive>`. |
| `locomotive` | text | always | 1.0 | The locomotive. |
| `reserved` | whole number | always | 1.0 | The reserved command byte. |
| `action_code` | whole number | always | 1.0 | The action code. |
| `action` | text | always | 1.0 | The action. |
| `parameter` | text | optional | 1.0 | The parameter as 4 characters. |
| `pressure_kpa` | number (kPa) | optional | 1.0 | The brake pipe pressure of a pressure action. |
| `tail_id` | text | optional | 1.0 | The tail device id. |
| `extra` | text | optional | 1.0 | Extra bytes as hex. |
| `clock` | text | optional | 1.0 | The calibration clock, `YYYY-MM-DDThh:mm:ss`. |
| `corrections` | whole number | always | 1.0 | The bits corrected. |
| `raw` | text | always | 1.0 | The raw frame as hex. |

### Event `railway_cir.tdcs`

Struct `RailwayCirTdcsEvent`. A dispatch message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The summary: `TDCS <train> <locomotive>`. |
| `train` | text | always | 1.0 | The train. |
| `locomotive` | text | always | 1.0 | The locomotive. |
| `loco_code` | whole number | always | 1.0 | The locomotive type code. |
| `loco_serial` | whole number | always | 1.0 | The locomotive serial. |
| `km_post_km` | number (km) | optional | 1.0 | The kilometre post. |
| `increasing` | yes or no | always | 1.0 | The mileage increases. |
| `test_frame` | yes or no | always | 1.0 | The frame marks itself as a test frame. |
| `speed_kmh` | number (km/h) | always | 1.0 | The speed. |
| `weight_t` | number (t) | always | 1.0 | The weight. |
| `length` | number | always | 1.0 | The length field divided by 10 (its unit is not published). |
| `cars` | whole number | always | 1.0 | The car count. |
| `station` | whole number | always | 1.0 | The station. |
| `track` | whole number | always | 1.0 | The track. |
| `actual_track` | whole number | always | 1.0 | The actual track. |
| `driver` | whole number | always | 1.0 | The driver id. |
| `signal_type` | whole number | always | 1.0 | The signal type. |
| `semaphore_type` | whole number | always | 1.0 | The semaphore type. |
| `role` | whole number | always | 1.0 | The locomotive role. |
| `corrections` | whole number | always | 1.0 | The bits corrected. |
| `raw` | text | always | 1.0 | The raw frame as hex. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_railway_cir_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `history` | `sdrkit_railway_cir_get_history` | `sdrkit_railway_cir_set_history` |
| `locomotives` | `sdrkit_railway_cir_get_locomotives` | `sdrkit_railway_cir_set_locomotives` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_invert_bits` | `sdrkit_railway_cir_gen_get_invert_bits` | `sdrkit_railway_cir_gen_set_invert_bits` |
| `gen_deviation_hz` | `sdrkit_railway_cir_gen_get_deviation_hz` | `sdrkit_railway_cir_gen_set_deviation_hz` |

## Try it

```sh
sdrkit run railway_cir --source synth:railway_cir
sdrkit run railway_cir --source synth:railway_cir --submode klw
sdrkit run railway_cir --source iiod:192.168.2.1 --freq 450
```
<!-- generated: end -->

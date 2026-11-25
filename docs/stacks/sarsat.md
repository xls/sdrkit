<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# COSPAS-SARSAT 406 MHz

Distress beacons on 406 MHz, first and second generation: identity, protocol, country and position. Receive only.

Stack id: `sarsat`. Group: Satellite.

Signal types: COSPAS-SARSAT 406 MHz.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 406.028 MHz |
| Tuning range | 406 MHz to 1.546 GHz |
| Sample rate | 256 kHz |
| Smallest sample rate | 128 kHz |
| Signal bandwidth | 100 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 16 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sarsat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `wide_search` | Wide search | yes or no | false |  | A bank of burst finders 90 kHz apart across up to 1.1 MHz of the input. It is on by itself while the dial is between 1543.5 and 1546.0 MHz. |
| `repeat_combining` | Repeat combining | yes or no | true |  | Sums the soft bits of a first-generation burst that does not decode with those of the same beacon's earlier bursts on the same carrier. |
| `repeat_min_s` | Shortest repeat interval | duration (s) | 47.5 s | 1 to 300 | C/S T.001: a beacon repeats every 50 s +-5 %. |
| `repeat_max_s` | Longest repeat interval | duration (s) | 52.5 s | 1 to 300 | C/S T.001: a beacon repeats every 50 s +-5 %. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_second_generation` | Second generation | yes or no | false |  |  |
| `gen_beacon` | Test beacon | whole number | 0 | 0 to 5 | 0 sends every test beacon in turn. |
| `gen_reverse` | Reversed phase polarity | yes or no | false |  |  |
| `gen_bitrate_pct` | Bit rate error | number (%) | 0 | -2 to 2 | The first-generation bit rate away from 400 bit/s (T.001 allows 1 %). |
| `gen_gap_s` | Seconds between bursts | duration (s) | 3 s | 0.2 to 60 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `bursts_heard` | Bursts heard | whole number |
| `finders` | Finders | whole number |
| `span_hz` | Search span | frequency |
| `dropped` | Dropped | whole number |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bursts` | Bursts | `at` (duration (s)), `heard_at` (time), `generation` (whole number), `hex_id` (text), `beacon_type` (text), `country` (text), `protocol` (text), `identification` (text), `position` (position), `homing` (yes or no), `snr_db` (number (dB)), `level_dbfs` (number (dBFS)), `offset_hz` (frequency) |
| `beacons` | Beacons | `hex_id` (text), `generation` (whole number), `beacon_type` (text), `country` (text), `identification` (text), `position` (position), `combined` (whole number), `confirmed` (yes or no), `repair_margin` (number), `repair_cost` (number), `last_heard` (time), `heard` (whole number) |

## Map points

The stack puts points on a map: Beacons.

## Events

| Kind | Label | Fields |
|---|---|---|
| `sarsat.beacon` | Beacon decoded | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `urgency` (choice), `position` (position), `generation` (whole number), `hex_id` (text), `beacon_type` (text), `country` (text), `mid` (whole number), `protocol` (text), `identification` (text), `homing` (yes or no), `self_test` (yes or no), `bits_corrected` (whole number), `offset_hz` (frequency), `message_hex` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `preset.apply` | Apply preset | `preset` (choice, default `406.028`) |  | Asks the host to tune the dial to the preset. |

## Typed interface

The stack is in the family `satcom`, version 2.0. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `SarsatFields`. The result fields of COSPAS-SARSAT.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `bursts_heard` | whole number | always | 1.0 | Bursts heard. |
| `finders` | whole number | always | 1.0 | First-generation finders running. |
| `span_hz` | frequency | always | 1.0 | The width the finders cover. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bursts`

Struct `SarsatBurstRow`. A burst of the log. The row key is the sequence number of the burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of a burst. |
| `heard_at` | time | optional | 1.0 | The wall-clock time of the burst, when the samples were live. |
| `generation` | whole number | always | 1.0 | The beacon generation, 1 or 2. |
| `hex_id` | text | always | 1.0 | The beacon's hex identity (15 digits, or 23 for the second generation). |
| `beacon_type` | text | always | 1.0 | The beacon type (EPIRB, ELT, PLB ...). |
| `country` | text | always | 1.0 | The country of the beacon's MID. |
| `protocol` | text | always | 1.0 | The protocol in words. |
| `identification` | text | always | 1.0 | The identification in words. |
| `position` | position | optional | 1.0 | The position that the beacon sends. |
| `homing` | yes or no | always | 1.0 | The 121.5 MHz homing flag. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input. |
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier from the tuned frequency. |

The rows are a log: they arrive over time. The column `hex_id` names a row. Place of a row: the column `position`. Time: `heard_at`.

### Table `beacons`

Struct `SarsatBeaconRow`. A beacon heard, with its last burst. The row key is the hex identity.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `hex_id` | text | always | 1.0 | The beacon's hex identity (15 digits, or 23 for the second generation). |
| `generation` | whole number | always | 1.0 | The beacon generation, 1 or 2. |
| `beacon_type` | text | always | 1.0 | The beacon type (EPIRB, ELT, PLB ...). |
| `country` | text | always | 1.0 | The country of the beacon's MID. |
| `identification` | text | always | 1.0 | The identification in words. |
| `position` | position | optional | 1.0 | The position that the beacon sends. |
| `combined` | whole number | optional | 1.0 | Bursts whose soft bits were summed to decode the message. |
| `confirmed` | yes or no | optional | 1.0 | A later burst decoded to the same identification before the message was reported. |
| `repair_margin` | number | optional | 1.0 | The distance from the decoded word to the next one that passes, over the mean reliability of a bit. |
| `repair_cost` | number | optional | 1.0 | The reliability of the repaired bits over the mean reliability of a bit. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |
| `heard` | whole number | optional | 1.0 | How many times it was heard. |

The column `hex_id` names a row. Place of a row: the column `position`.

### Event `sarsat.beacon`

Struct `SarsatBeaconEvent`. A beacon decoded. The source is the hex identity. A distress beacon writes the urgency emergency, a test beacon none.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The hex identity. |
| `text` | text | always | 1.0 | What the beacon says. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `urgency` | choice | always | 1.0 | How urgent the beacon is: emergency for a distress beacon. |
| `position` | position | optional | 1.0 | The position that the beacon sends. |
| `generation` | whole number | always | 1.0 | The beacon generation, 1 or 2. |
| `hex_id` | text | always | 1.0 | The beacon's hex identity (15 digits, or 23 for the second generation). |
| `beacon_type` | text | always | 1.0 | The beacon type (EPIRB, ELT, PLB ...). |
| `country` | text | always | 1.0 | The country of the beacon's MID. |
| `mid` | whole number | always | 1.0 | The country code (MID). |
| `protocol` | text | always | 1.0 | The protocol in words. |
| `identification` | text | always | 1.0 | The identification in words. |
| `homing` | yes or no | always | 1.0 | The 121.5 MHz homing flag. |
| `self_test` | yes or no | always | 1.0 | The self-test or test-protocol flag. |
| `bits_corrected` | whole number | always | 1.0 | The bits corrected. |
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier from the tuned frequency. |
| `message_hex` | text | always | 1.0 | The message bits as hexadecimal digits (as received after the repair). |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sarsat_clear` | none | none |
| `remove` | `sdrkit_sarsat_remove` | `sdrkit_sarsat_beacon_args` | none |
| `preset.apply` | `sdrkit_sarsat_preset_apply` | `sdrkit_sarsat_preset_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `wide_search` | `sdrkit_sarsat_get_wide_search` | `sdrkit_sarsat_set_wide_search` |
| `repeat_combining` | `sdrkit_sarsat_get_repeat_combining` | `sdrkit_sarsat_set_repeat_combining` |
| `repeat_min_s` | `sdrkit_sarsat_get_repeat_min_s` | `sdrkit_sarsat_set_repeat_min_s` |
| `repeat_max_s` | `sdrkit_sarsat_get_repeat_max_s` | `sdrkit_sarsat_set_repeat_max_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_second_generation` | `sdrkit_sarsat_gen_get_second_generation` | `sdrkit_sarsat_gen_set_second_generation` |
| `gen_beacon` | `sdrkit_sarsat_gen_get_beacon` | `sdrkit_sarsat_gen_set_beacon` |
| `gen_reverse` | `sdrkit_sarsat_gen_get_reverse` | `sdrkit_sarsat_gen_set_reverse` |
| `gen_bitrate_pct` | `sdrkit_sarsat_gen_get_bitrate_pct` | `sdrkit_sarsat_gen_set_bitrate_pct` |
| `gen_gap_s` | `sdrkit_sarsat_gen_get_gap_s` | `sdrkit_sarsat_gen_set_gap_s` |

## Try it

```sh
sdrkit run sarsat --source synth:sarsat
sdrkit run sarsat --source iiod:192.168.2.1 --freq 406.028
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

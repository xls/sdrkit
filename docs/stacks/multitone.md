<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Multitone Paging

A detector of Multitone paging transmissions: carrier, deviation, tones and FSK measurements.

Stack id: `multitone`. Group: Paging.

Signal types: Multitone paging.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 446.475 MHz |
| Tuning range | 25 MHz to 480 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

The stack has no settings.

## Result fields

| Key | Label | Kind |
|---|---|---|
| `carrier` | Carrier | yes or no |
| `transmissions` | Transmissions | whole number |
| `bursts` | Bursts | whole number |
| `valid_words` | Valid BCH words | whole number |
| `other_words` | Other words | whole number |
| `level_dbfs` | Level | number (dBFS) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `transmissions` | Transmissions | `time_s` (number (s)), `duration` (duration (s)), `snr_db` (number (dB)), `deviation_hz` (frequency), `kind` (choice), `period_ms` (number (ms)), `burst` (whole number) |
| `tx_tones` | Tones of the transmissions | `transmission` (text), `hz` (frequency), `seconds` (duration (s)), `start` (duration (s)) |
| `tx_lines` | Lines of the transmissions | `transmission` (text), `hz` (frequency), `db` (number (dB)) |
| `tones_heard` | Tones heard | `hz` (frequency), `count` (whole number), `seconds` (duration (s)), `last_s` (number (s)) |
| `bursts` | Bursts | `burst` (whole number), `kind` (choice), `baud` (number), `bits` (whole number), `bch_valid` (whole number), `bch_total` (whole number), `verdict` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `multitone.transmission` | Multitone transmission | `kind` (choice), `duration` (duration (s)), `snr_db` (number (dB)), `deviation_hz` (frequency), `text` (text), `family` (choice) |
| `multitone.burst` | Multitone FSK burst | `kind` (choice), `baud` (number), `bits` (whole number), `bch_valid` (whole number), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `trunk`, version 1.1. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `MultitoneFields`. The result fields of the Multitone detector.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carrier` | yes or no | always | 1.0 | Carrier. |
| `transmissions` | whole number | always | 1.0 | The transmissions measured. |
| `bursts` | whole number | always | 1.0 | Bursts. |
| `valid_words` | whole number | always | 1.0 | Valid BCH words. |
| `other_words` | whole number | always | 1.0 | Other words. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |

### Table `transmissions`

Struct `MultitoneTransmissionRow`. A transmission. The row key is its number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Time. |
| `duration` | duration (s) | always | 1.0 | Duration. |
| `snr_db` | number (dB) | always | 1.0 | SNR. |
| `deviation_hz` | frequency | always | 1.0 | Deviation. |
| `kind` | choice | always | 1.0 | Kind. |
| `period_ms` | number (ms) | optional | 1.0 | Period. |
| `burst` | whole number | optional | 1.0 | Burst. |

### Table `tx_tones`

Struct `MultitoneToneRow`. A tone of a transmission. The row key is the transmission, `/` and the place of the tone.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmission` | text | always | 1.0 | The transmission a row belongs to: a row key of [`TX_TABLE`]. |
| `hz` | frequency | always | 1.0 | A frequency. |
| `seconds` | duration (s) | always | 1.0 | Length. |
| `start` | duration (s) | always | 1.0 | Start in the transmission. |

### Table `tx_lines`

Struct `MultitoneLineRow`. A spectral line of a transmission. The row key is the transmission, `/` and the place of the line.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmission` | text | always | 1.0 | The transmission a row belongs to: a row key of [`TX_TABLE`]. |
| `hz` | frequency | always | 1.0 | A frequency. |
| `db` | number (dB) | always | 1.0 | Above the median. |

### Table `tones_heard`

Struct `MultitoneHeardRow`. A tone heard and how often. The row key is the tone.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `hz` | frequency | always | 1.0 | A frequency. |
| `count` | whole number | always | 1.0 | Count. |
| `seconds` | duration (s) | always | 1.0 | Length. |
| `last_s` | number (s) | always | 1.0 | Last. |

### Table `bursts`

Struct `MultitoneBurstRow`. An FSK burst. The row key is its number, eight digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `burst` | whole number | always | 1.0 | Burst. |
| `kind` | choice | always | 1.0 | Kind. |
| `baud` | number | always | 1.0 | Baud. |
| `bits` | whole number | always | 1.0 | Bits. |
| `bch_valid` | whole number | always | 1.0 | Valid BCH words. |
| `bch_total` | whole number | always | 1.0 | Words. |
| `verdict` | text | always | 1.0 | Verdict. |

### Event `multitone.transmission`

Struct `MultitoneTransmissionEvent`. A finished transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | Kind. |
| `duration` | duration (s) | always | 1.0 | Duration. |
| `snr_db` | number (dB) | always | 1.0 | SNR. |
| `deviation_hz` | frequency | always | 1.0 | Deviation. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `multitone.burst`

Struct `MultitoneBurstEvent`. A finished FSK burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | Kind. |
| `baud` | number | always | 1.0 | Baud. |
| `bits` | whole number | always | 1.0 | Bits. |
| `bch_valid` | whole number | always | 1.0 | Valid BCH words. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_multitone_clear` | none | none |

## Try it

```sh
sdrkit run multitone --source synth:multitone
sdrkit run multitone --source iiod:192.168.2.1 --freq 446.475
```
<!-- generated: end -->

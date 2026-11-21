<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Iridium

Iridium bursts over the whole band, demodulated and written as RAW text lines. Receive only.

Stack id: `iridium`. Group: Satellite.

Signal types: Iridium.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.622 GHz |
| Tuning range | 1.61 GHz to 1.63 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 10.5 MHz |
| Radio filter | 9 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:iridium` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Burst threshold | number (dB) | 13 | 8 to 24 | Burst level over the noise floor needed to demodulate a burst. |
| `raw_output` | RAW output | yes or no | false |  | Publish each burst as a RAW line (events iridium.raw and the table raw). The host writes them where it wants. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_burst_doppler` | Doppler | yes or no | true |  | Each burst gets a Doppler shift of up to +-35 kHz. |
| `gen_bursts` | Bursts per frame | whole number | 4 | 1 to 20 |  |
| `gen_centre_hz` | Radio centre | frequency | 1.622 GHz | 1.61 GHz to 1.63 GHz | Where the radio is tuned, so the generator puts the channels in the right place. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bursts` | Bursts found | whole number |
| `demodulated` | Bursts demodulated | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `dropped` | Dropped | whole number |
| `duplicates` | Duplicates | whole number |
| `downlink` | Downlink bursts | whole number |
| `uplink` | Uplink bursts | whole number |
| `burst_rate` | Bursts a second | frequency |
| `uw_rate` | Unique words a second | frequency |
| `confidence_pct` | Confidence | number (%) |
| `esn0_db` | Es/N0 | number (dB) |
| `noise_db_hz` | Noise density | number (dBFS/Hz) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `raw` | RAW lines | `at` (duration (s)), `frequency` (frequency), `direction` (choice), `confidence_pct` (number (%)), `line` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `scatter_detected` | Bursts found | s | Hz |
| `scatter_uw` | Bursts with a unique word | s | Hz |

## Events

| Kind | Label | Fields |
|---|---|---|
| `iridium.raw` | RAW line | `text` (text), `snr_db` (number (dB)), `family` (choice), `confidence_pct` (number (%)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.0. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `IridiumFields`. The result fields of Iridium.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts` | whole number | always | 1.0 | Bursts the detector found. |
| `demodulated` | whole number | always | 1.0 | Bursts demodulated. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `dropped` | whole number | always | 1.0 | Input items dropped because a helper thread did not keep up. |
| `duplicates` | whole number | always | 1.0 | Bursts found twice and counted once. |
| `downlink` | whole number | always | 1.0 | Bursts with the downlink unique word. |
| `uplink` | whole number | always | 1.0 | Bursts with the uplink unique word. |
| `burst_rate` | frequency | always | 1.0 | Bursts found per second (the last report interval). |
| `uw_rate` | frequency | always | 1.0 | Unique words found per second (the last report interval). |
| `confidence_pct` | number (%) | optional | 1.0 | The mean confidence of the bursts in the last interval. |
| `esn0_db` | number (dB) | optional | 1.0 | The mean Es/N0 of the bursts in the last interval. |
| `noise_db_hz` | number (dBFS/Hz) | optional | 1.0 | The detector's noise floor, dB relative to full scale in 1 Hz. |

### Table `raw`

Struct `IridiumRawRow`. A RAW line. The row key is the sequence number of the line.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | At (signal time). |
| `frequency` | frequency | always | 1.0 | Frequency. |
| `direction` | choice | always | 1.0 | Direction. |
| `confidence_pct` | number (%) | always | 1.0 | Confidence. |
| `line` | text | always | 1.0 | RAW line. |

### Event `iridium.raw`

Struct `IridiumRawEvent`. A RAW line of one burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `confidence_pct` | number (%) | always | 1.0 | Confidence. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_iridium_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_iridium_get_threshold_db` | `sdrkit_iridium_set_threshold_db` |
| `raw_output` | `sdrkit_iridium_get_raw_output` | `sdrkit_iridium_set_raw_output` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_burst_doppler` | `sdrkit_iridium_gen_get_burst_doppler` | `sdrkit_iridium_gen_set_burst_doppler` |
| `gen_bursts` | `sdrkit_iridium_gen_get_bursts` | `sdrkit_iridium_gen_set_bursts` |
| `gen_centre_hz` | `sdrkit_iridium_gen_get_centre_hz` | `sdrkit_iridium_gen_set_centre_hz` |

## Try it

```sh
sdrkit run iridium --source synth:iridium
sdrkit run iridium --source iiod:192.168.2.1 --freq 1622
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Meteor scatter

Counts and measures meteor echoes of a CW beacon such as the GRAVES radar on 143.050 MHz.

Stack id: `mscatter`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 30 MHz to 300 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 8 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mscatter` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Detection level | number (dB) | 13 | 6 to 30 |  |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_beacon_hz` | Test signal: beacon offset | frequency | 420 Hz | -4 kHz to 4 kHz |  |
| `gen_mean_gap_s` | Test signal: mean gap between pings | duration (s) | 3 s | 0 to 600 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `pings` | Pings | whole number |
| `rate` | Pings in the last hour | whole number |
| `rejected` | Bursts refused | whole number |
| `beacon` | Beacon offset | frequency |
| `beacon_snr_db` | Beacon SNR | number (dB) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `threshold_db` | Detection level | number (dB) |
| `ping_now_db` | Ping now | number (dB) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `pings` | Pings | `id` (whole number), `utc` (time), `duration` (duration (s)), `peak_snr_db` (number (dB)), `doppler` (frequency), `drift_hz_s` (number (Hz/s)), `kind` (choice) |
| `showers` | Showers | `name` (text), `peak_in_days` (whole number (d)), `active` (yes or no), `zhr` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `hourly` | Pings per hour | h |  |
| `daily` | Pings per day | d |  |
| `beacon_spectrum` | Around the beacon | bin | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ms.ping` | Meteor ping | `text` (text), `family` (choice), `utc` (time), `duration` (duration (s)), `peak_snr_db` (number (dB)), `doppler` (frequency), `kind` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `science`, version 1.3. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

### Result fields

Struct `MscatterFields`. The result fields of `mscatter`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pings` | whole number | always | 1.0 | Pings counted. |
| `rate` | whole number | always | 1.0 | Pings in the last 60 minutes. |
| `rejected` | whole number | always | 1.0 | Broadband bursts refused. |
| `beacon` | frequency | optional | 1.0 | The beacon's offset from the centre. |
| `beacon_snr_db` | number (dB) | always | 1.0 | The beacon level over the noise in one bin. |
| `noise_dbfs` | number (dBFS) | always | 1.0 | Noise floor of the channel, dBFS. |
| `threshold_db` | number (dB) | always | 1.0 | The detection level. |
| `ping_now_db` | number (dB) | optional | 1.0 | The level of the ping on the air now. |

### Table `pings`

Struct `MscatterPingRow`. A meteor ping. The row key is the sequence number of the ping.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The number of a ping. |
| `utc` | time | always | 1.0 | UTC of a ping. |
| `duration` | duration (s) | always | 1.0 | The length of a ping. |
| `peak_snr_db` | number (dB) | always | 1.0 | The peak level over the background. |
| `doppler` | frequency | always | 1.0 | The Doppler shift at the peak. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The drift of the Doppler shift. |
| `kind` | choice | always | 1.0 | Underdense or overdense. |

### Table `showers`

Struct `MscatterShowerRow`. A meteor shower. The row key is its name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of a shower. |
| `peak_in_days` | whole number (d) | always | 1.0 | Days to the peak of a shower (below 0 after it). |
| `active` | yes or no | always | 1.0 | The shower is active. |
| `zhr` | whole number | always | 1.0 | The zenithal hourly rate of a shower. |

The column `name` names a row.

### Event `ms.ping`

Struct `MscatterPingEvent`. A meteor ping. `text` holds the ping for people.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the ping for people. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `utc` | time | always | 1.0 | UTC of a ping. |
| `duration` | duration (s) | always | 1.0 | The length of a ping. |
| `peak_snr_db` | number (dB) | always | 1.0 | The peak level over the background. |
| `doppler` | frequency | always | 1.0 | The Doppler shift at the peak. |
| `kind` | choice | always | 1.0 | Underdense or overdense. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_mscatter_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_mscatter_get_threshold_db` | `sdrkit_mscatter_set_threshold_db` |
| `start_utc` | `sdrkit_mscatter_get_start_utc` | `sdrkit_mscatter_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_beacon_hz` | `sdrkit_mscatter_gen_get_beacon_hz` | `sdrkit_mscatter_gen_set_beacon_hz` |
| `gen_mean_gap_s` | `sdrkit_mscatter_gen_get_mean_gap_s` | `sdrkit_mscatter_gen_set_mean_gap_s` |

## Try it

```sh
sdrkit run mscatter --source synth:mscatter
sdrkit run mscatter --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/science/mscatter_beacon_24k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

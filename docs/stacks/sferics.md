<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Sferics (lightning)

Counts the radio impulses of lightning as strikes and flashes: strike rates, a 60-minute histogram and a storm trend, with mains impulse trains left out. Any band below 30 MHz; VLF and LF hear them best.

Stack id: `sferics`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 3 MHz |
| Tuning range | 3 kHz to 30 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 20 kHz |
| Signal bandwidth | 1 MHz |
| Radio filter | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sferics` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Threshold over the floor | number (dB) | 10 | 6 to 30 | An impulse starts when the 25 µs power average rises this far over the floor. |
| `mains_filter` | Leave out mains trains | yes or no | true |  | Impulses on a 10 ms or 1/120 s grid are counted as mains, not as strikes. |
| `merge_ms` | Merge time | number (ms) | 0.5 | 0.1 to 5 | An impulse that starts this soon after the end of the one before joins it. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_strike_rate` | Test signal: strokes a second | number (1/s) | 1.5 | 0 to 50 | Flashes of 1 to 3 strokes come at random gaps of 1.7 / rate seconds on average. |
| `gen_scene` | Test signal: scene | choice | `storm` | `storm` (Storm), `mains` (Mains train only), `storm_mains` (Storm and mains train), `quiet` (Quiet) |  |
| `gen_mains_hz` | Test signal: mains frequency | frequency | 50 Hz | 45 Hz to 65 Hz | The train has two impulses a period: 100 a second at 50 Hz, 120 at 60 Hz. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `strikes` | Strikes | whole number |
| `flashes` | Flashes | whole number |
| `mains` | Mains impulses | whole number |
| `too_long` | Too long | whole number |
| `per_min_1` | Strikes, last minute | number (1/min) |
| `per_min_10` | Strikes per minute, 10 minutes | number (1/min) |
| `floor_db` | Floor | number (dBFS) |
| `threshold_db` | Threshold | number (dB) |
| `last_peak_db` | Last peak | number (dB) |
| `last_ago_s` | Last strike ago | number (s) |
| `mains_active` | Mains train | yes or no |
| `storm_phase` | Storm | text |
| `storm_db_change` | Storm change | number (dB) |
| `activity` | Activity | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `strikes` | Strikes | `time_s` (number (s)), `utc` (time), `peak_db` (number (dB)), `width_us` (number (us)), `flash` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `per_minute` | Strikes per minute | min | strikes |

## Events

| Kind | Label | Fields |
|---|---|---|
| `sferics.storm` | Storm phase | `storm_phase` (text), `storm_db_change` (number (dB)), `per_minute` (number (1/min)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Empties the tables and counters, and learns the floor and the mains train again. |

## Typed interface

The stack is in the family `science`, version 2.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

Key readings: `per_min_1`, `strikes`, `activity`, `storm_phase`, `last_peak_db`.

### Result fields

Struct `SfericsFields`. The result fields of `sferics`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `strikes` | whole number | always | 1.0 | Strikes counted. |
| `flashes` | whole number | always | 1.0 | Flashes counted. |
| `mains` | whole number | always | 1.0 | Impulses of a mains train left out. |
| `too_long` | whole number | always | 1.0 | Impulses too long to be lightning. |
| `per_min_1` | number (1/min) | always | 1.0 | Strikes in the last minute. |
| `per_min_10` | number (1/min) | always | 1.0 | Strikes per minute over 10 minutes. |
| `floor_db` | number (dBFS) | optional | 1.0 | The floor. |
| `threshold_db` | number (dB) | always | 1.0 | The start level in use. |
| `last_peak_db` | number (dB) | optional | 1.0 | The peak of the newest strike. |
| `last_ago_s` | number (s) | optional | 1.0 | The time since the newest strike. |
| `mains_active` | yes or no | always | 1.0 | A mains train is being left out. |
| `storm_phase` | text | always | 1.0 | The storm phase. |
| `storm_db_change` | number (dB) | always | 1.0 | The change of the median peak. |
| `activity` | text | always | 1.0 | The activity word of the 10-minute rate. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `strikes`

Struct `SfericsStrikeRow`. A strike. The row key is its start, s of signal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The start of a strike. |
| `utc` | time | always | 1.0 | The start of a strike in UTC. |
| `peak_db` | number (dB) | always | 1.0 | The peak of a strike over the floor. |
| `width_us` | number (us) | always | 1.0 | The width of a strike. |
| `flash` | whole number | always | 1.0 | The flash of a strike. |

The rows are a log: they arrive over time. Time: `utc`.

### Event `sferics.storm`

Struct `SfericsStormEvent`. The storm phase changed to approaching, overhead or receding.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `storm_phase` | text | always | 1.0 | The storm phase. |
| `storm_db_change` | number (dB) | always | 1.0 | The change of the median peak. |
| `per_minute` | number (1/min) | always | 1.0 | Strikes per minute in the last 5 minutes. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sferics_clear` | none | none |
| `reset` | `sdrkit_sferics_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_sferics_get_threshold_db` | `sdrkit_sferics_set_threshold_db` |
| `mains_filter` | `sdrkit_sferics_get_mains_filter` | `sdrkit_sferics_set_mains_filter` |
| `merge_ms` | `sdrkit_sferics_get_merge_ms` | `sdrkit_sferics_set_merge_ms` |
| `start_utc` | `sdrkit_sferics_get_start_utc` | `sdrkit_sferics_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_strike_rate` | `sdrkit_sferics_gen_get_strike_rate` | `sdrkit_sferics_gen_set_strike_rate` |
| `gen_scene` | `sdrkit_sferics_gen_get_scene` | `sdrkit_sferics_gen_set_scene` |
| `gen_mains_hz` | `sdrkit_sferics_gen_get_mains_hz` | `sdrkit_sferics_gen_set_mains_hz` |

## Try it

```sh
sdrkit run sferics --source synth:sferics
sdrkit run sferics --source <driver>:<address> --freq 3
```
<!-- generated: end -->

## Description

A lightning discharge radiates a broadband radio impulse, a sferic. It is heard on every band below about 30 MHz and is strongest at VLF and LF. The receiver averages the power over 25 µs and finds impulses that rise over its own noise floor, the median of the 1 ms means of the last 4 s. Impulses less than the merge time apart join; impulses longer than 5 ms are counted as too long, not as lightning. Each impulse is counted 0.5 s after its start, so that a train of mains impulses (lamp dimmers, chargers, power lines, on a 10 ms or 1/120 s grid) is recognised first and left out. A strike more than 0.5 s after the one before starts a new flash. The stack gives the strike rates, the strikes of each of the last 60 minutes and a storm trend from the rates and peaks of two halves of 10 minutes. It says when and how strong, not where. Not confirmed by a public document: every level, time and rule of the detector (the 25 µs power average, the floor as the median of 1 ms means over 4 s, the end 3 dB under the threshold after 100 µs, 5 ms as too long for lightning, the 0.5 s commit delay and flash gap), the recognition of mains trains (10 ms and 1/120 s grids, 24 of 32 impulses) and the storm trend (its halves of 5 minutes, its rates and its 2.5 dB and 6 dB limits). The strength is relative to this receiver's floor and antenna; it is not calibrated, and irregular impulsive noise counts as lightning.

## Tuning: notes

The stack holds no dial: any frequency below 30 MHz hears sferics, and the usual dials are in `data/vlf/vlf_dials.csv`, for the host to offer. A sound card fed as IQ at 48 ksps works too (timing to about 20 µs). The front end's blind IQ correction is off for this stack, because it would take impulses for an imbalance.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Result fields: notes

`per_min_10` is the strikes of the last 10 minutes per minute (over the time so far when that is shorter, at least one minute); `activity` is `none` below 0.2 a minute, `low` below 3, `moderate` below 15, else `high`. `storm_phase` is `none` (fewer than 5 strikes in 10 minutes), `distant`, `steady`, `approaching`, `overhead` or `receding`; it is a trend, not a position: stronger may mean nearer or a stronger cell. The lamp searches until the floor is valid (0.5 s), tracks, and locks while a strike lies in the last 10 minutes.

## Commands: notes

`clear` keeps the noise floor and the mains train; `reset` learns both again.

## Try it: notes

```sh
sdrkit run sferics --source synth:sferics --seconds 30
sdrkit run sferics --source synth:sferics gen_scene=storm_mains
sdrkit run sferics --source iiod:192.168.2.1 --freq 0.5
```

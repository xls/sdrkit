<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Radio Jove

A spectrogram of the whole capture near 20 MHz and the broadband bursts of Jupiter and the Sun in it: time, duration, strength and width, with the background level minute by minute.

Stack id: `jove`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 20.1 MHz |
| Tuning range | 10 MHz to 40 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:jove` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Burst threshold per bin | number (dB) | 1 | 0 to 20 | The least excess of an active bin over its background: 0.5 to 20 dB, 0 for the default of 1 dB. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_level_db` | Test signal burst level | number (dB) | 6 | -10 to 30 | The power density of a burst over the noise density. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `bursts` | Bursts | whole number |
| `events_total` | Events | whole number |
| `level_db` | Level | number (dB) |
| `bin_hz` | Bin | frequency |
| `row_s` | Row time | duration (s) |
| `burst_open` | Burst open | yes or no |
| `files` | Files written | whole number |
| `last_file` | Last file | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `utc` (time), `signal_time_s` (number (s)), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number), `event_note` (text) |
| `trend` | Trend per minute | `utc` (time), `level_change_db` (number (dB)), `bursts_per_min` (number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `events_per_hour` | Events per UTC hour | h |  |

## Result pictures

| Key | Label | X unit | Y unit | Cell unit |
|---|---|---|---|---|
| `spectrogram` | Spectrogram | Hz | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `science.event` | Science event | `text` (text), `utc` (time), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Empties the tables and starts the detector again; the background is learned again. |

## Typed interface

The stack is in the family `science`, version 1.2. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

### Result fields

Struct `BurstFields`. The result fields of the burst monitors `jove` and `callisto`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `bursts` | whole number | always | 1.0 | Bursts found. |
| `events_total` | whole number | always | 1.0 | Events in total. |
| `level_db` | number (dB) | optional | 1.0 | The level of the last row. |
| `bin_hz` | frequency | optional | 1.0 | The width of a bin. |
| `row_s` | duration (s) | optional | 1.0 | The time of a row. |
| `burst_open` | yes or no | always | 1.0 | A burst is open. |
| `files` | whole number | always | 1.0 | Files written. |
| `last_file` | text | always | 1.0 | The last file written, empty before the first. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `ScienceEventRow`. An event of a monitor (`schumann`, `whistler`, `jove`, `callisto`, `eme_echo`, `beacon_events`). The row key is the sequence number of the event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of an event. |
| `signal_time_s` | number (s) | always | 1.0 | The signal time of an event. |
| `type` | text | always | 1.0 | The type of an event. |
| `frequency_hz` | frequency | always | 1.0 | The characteristic frequency of an event. |
| `peak_db` | number (dB) | always | 1.0 | The strength of an event over the background. |
| `duration_s` | duration (s) | always | 1.0 | The duration of an event. |
| `aux1` | number | always | 1.0 | Stack-specific value 1. |
| `aux2` | number | always | 1.0 | Stack-specific value 2. |
| `event_note` | text | always | 1.0 | The note of an event. |

The column `type` names a row.

### Table `trend`

Struct `BurstTrendRow`. A trend minute of the burst monitors (`jove`, `callisto`). The row key is the minute of the capture.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the end of the minute. |
| `level_change_db` | number (dB) | always | 1.0 | The level of a trend minute. |
| `bursts_per_min` | number | always | 1.0 | Bursts of a trend minute. |

### Event `science.event`

Struct `ScienceEvent`. An event of a monitor. `text` holds its note.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `utc` | time | always | 1.0 | The UTC of an event. |
| `type` | text | always | 1.0 | The type of an event. |
| `frequency_hz` | frequency | always | 1.0 | The characteristic frequency of an event. |
| `peak_db` | number (dB) | always | 1.0 | The strength of an event over the background. |
| `duration_s` | duration (s) | always | 1.0 | The duration of an event. |
| `aux1` | number | always | 1.0 | Stack-specific value 1. |
| `aux2` | number | always | 1.0 | Stack-specific value 2. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_jove_clear` | none | none |
| `reset` | `sdrkit_jove_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_jove_get_threshold_db` | `sdrkit_jove_set_threshold_db` |
| `start_utc` | `sdrkit_jove_get_start_utc` | `sdrkit_jove_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_level_db` | `sdrkit_jove_gen_get_level_db` | `sdrkit_jove_gen_set_level_db` |

## Try it

```sh
sdrkit run jove --source synth:jove
sdrkit run jove --source <driver>:<address> --freq 20.1
```
<!-- generated: end -->

## Description

The receiver makes a spectrogram of the whole capture. The transform size starts at 8 and doubles while it is below 256 and rate / (2 N) is at least 0.7 of 31.25 kHz (64 points at 2 Msps, 8 at 250 ksps). Frames of N samples without overlap get a Hann window; the power of each bin is divided by the sum of the squared window, so white noise of variance s² gives s² in every bin. A row is the mean of the frames of about 0.1 s, in increasing frequency from -rate / 2.

Each bin has a background (dB) and a variance that start at the first row (0.04 dB²). The excess of a bin is its dB value minus its background. A bin is active when 20 or more quiet rows were learned and its excess is over max(threshold, 5 standard deviations). A row is active when half the bins or more are active: a broadband burst. Narrowband interference never reaches half the band. Only rows that are not active, while no burst is open, update the background, with the weight max(1 / (rows + 1), row time / 20 s). A burst opens on an active row and closes after two inactive rows. Its strength is the largest mean excess of the active bins of a row, its width the largest active bins × bin, its frequency the excess-weighted centroid of its first row. A burst under 1 s is `short`, else `long`. L and S bursts are not told apart: rows of 0.1 s cannot resolve S bursts.

Radio JOVE receivers listen at 20.1 MHz (NASA Radio JOVE receiver manual).

Not confirmed by a public document: every rule of the detector (the 31.25 kHz bins and 0.1 s rows, the background learned from 20 quiet rows with a 20 s time constant, a bin active over max(threshold, 5 standard deviations), a burst over half the band, closed after 2 quiet rows, short under 1 s). L and S bursts are not classified.

Each burst is a row of the `events` table and a `science.event` event of type `burst`: `frequency_hz` the centroid of its first row (Hz from the centre), `peak_db` its strength, `duration_s`, `aux1` its drift (Hz/s, the least-squares slope of the centroid over 4 rows or more, else 0), `aux2` its width (Hz), the note `short` or `long`. The `trend` table has one row a minute: the mean level minus the mean level of the first minute, and the bursts that started in the minute.

## Tuning: notes

The usual dial and range are in `data/science/science_dials.csv`. An HF dipole or a Radio JOVE antenna near 20 MHz is needed.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is complex white noise with bursts: combs of tones about 5 kHz apart with random phases, 0.8 × rate wide at the centre, at 0.2 and 0.55 of a cycle of clamp(8e6 / rate, 4, 60) s, 0.15 of it long. There is no burst in the first 3 s, while the detector learns its background.

## Result fields: notes

`files` and `last_file` stay 0 and empty: `jove` writes no file.

## Result pictures: notes

The spectrogram has the excess of every bin of every row, from -5 to 30 dB, and keeps 150 rows.

## Try it: notes

```sh
sdrkit run jove --source synth:jove --seconds 8
sdrkit run jove --source <driver>:<address> --freq 20.1
```

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Beacon openings

Follows a beacon carrier within ±1 kHz and logs every opening: start, duration, peak S/N, width, shift and flutter, with a class by the character of the signal (steady, fluttering or Doppler-spread).

Stack id: `beacon_events`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 50 MHz |
| Tuning range | 28 MHz to 1.3 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 200 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:beacon_events` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_z` | Opening threshold | number (z) | 12 | 0 to 30 | The best scan z of a hit row: 4 to 30, 0 for 12. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_level_db` | Test signal level | number (dB) | 30 | -10 to 60 | The carrier over the noise in a 2 Hz bin. |
| `gen_spread_hz` | Test signal spread | number (Hz) | 120 | 0 to 400 | The Doppler spread of the carrier while open. |
| `gen_flutter_db` | Test signal flutter | number (dB) | 3 | 0 to 20 | The standard deviation of the level of the carrier while open. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `openings` | Openings | whole number |
| `noise_db` | Noise per bin | number (dBFS) |
| `open` | Open | yes or no |
| `width_hz` | Width | frequency |
| `z` | Scan z | number |
| `snr_db` | S/N | number (dB) |
| `shift_hz` | Shift | frequency |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `utc` (time), `signal_time_s` (number (s)), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number), `event_note` (text) |
| `trend` | Trend per minute | `utc` (time), `peak_snr_db` (number (dB)), `mean_width_hz` (number (Hz)) |

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
| `reset` | Reset |  |  | Empties the tables and starts the detector again. |

## Typed interface

The stack is in the family `science`, version 2.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

Key readings: `open`, `snr_db`, `shift_hz`, `openings`.

### Result fields

Struct `BeaconEventsFields`. The result fields of `beacon_events`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `openings` | whole number | always | 1.0 | Openings found. |
| `noise_db` | number (dBFS) | optional | 1.0 | The noise per bin. |
| `open` | yes or no | always | 1.0 | An opening is open. |
| `width_hz` | frequency | optional | 1.0 | The width of the last row. |
| `z` | number | optional | 1.0 | The best z of the last row. |
| `snr_db` | number (dB) | optional | 1.0 | The S/N of the last row. |
| `shift_hz` | frequency | optional | 1.0 | The shift of the last row. |
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

The rows are a log: they arrive over time. The column `type` names a row. Time: `utc`.

### Table `trend`

Struct `BeaconEventsTrendRow`. A trend minute of `beacon_events`. The row key is the minute of the capture.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the end of the minute. |
| `peak_snr_db` | number (dB) | always | 1.0 | The peak S/N of the hit rows of a trend minute. |
| `mean_width_hz` | number (Hz) | always | 1.0 | The mean width of the hit rows of a trend minute. |

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
| `clear` | `sdrkit_beacon_events_clear` | none | none |
| `reset` | `sdrkit_beacon_events_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_z` | `sdrkit_beacon_events_get_threshold_z` | `sdrkit_beacon_events_set_threshold_z` |
| `start_utc` | `sdrkit_beacon_events_get_start_utc` | `sdrkit_beacon_events_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_level_db` | `sdrkit_beacon_events_gen_get_level_db` | `sdrkit_beacon_events_gen_set_level_db` |
| `gen_spread_hz` | `sdrkit_beacon_events_gen_get_spread_hz` | `sdrkit_beacon_events_gen_set_spread_hz` |
| `gen_flutter_db` | `sdrkit_beacon_events_gen_get_flutter_db` | `sdrkit_beacon_events_gen_set_flutter_db` |

## Try it

```sh
sdrkit run beacon_events --source synth:beacon_events
sdrkit run beacon_events --source <driver>:<address> --freq 50
```
<!-- generated: end -->

## Description

The receiver averages the input in groups of max(1, round(rate / 2000)) samples to about 2 ksps. Transforms of 1024 points (Hann) without overlap give a row every 0.512 s of bins of about 1.95 Hz, centred on the dial: a span of ±1 kHz around the beacon. The window correlates neighbouring bins, so noise alone reaches a z of 12 in about 1 % of the rows: such a single hit keeps an open opening open, but four hits in a row are needed to open one. The noise of a row is its median bin over 0.693.

Each row is scanned with windows of 5, 13, 31, 77 and 155 bins that slide by a quarter of their width: z = (sum - w × noise) / (noise × sqrt(w)). The best z of all windows decides a hit. The narrowest window whose own best z is 0.85 of that or more gives the centre, the excess and the S/N (10 log10(excess / noise), -10 dB without excess). The width is the larger of that window and 2.355 times the rms spread of the bins over 4 times the noise within ±100 bins of the centre (the full width at half height of a Gaussian: a Doppler-spread signal is a few blobs). The shift is the centre from the dial.

A row is a hit when its best z is the threshold or more. An opening opens after 4 hits in a row and starts at the first of them; it closes 10 s after its last hit. Any miss resets the count of hits; while no opening is open, a miss also forgets the rows collected. The width and the shift of an opening are the medians of its hit rows, the flutter is the standard deviation of their S/N without the first and the last row, the peak their largest S/N. The class describes the signal only, not the path: width 40 Hz or more "Doppler-spread (aurora-like)", else flutter 3 dB or more "fluttering carrier (TEP / scatter-like)", else "steady carrier (sporadic-E-like)".

Not confirmed by a public document: every rule of the detector (the windows of 5 to 155 bins, the threshold z of 12, 4 hits to open and 10 s to close, the width at 0.85 of the best z and from the bins over 4 noise levels) and the classes (40 Hz and 3 dB), which describe the signal, not the path.

Each opening is a row of the `events` table and a `science.event` event of type `opening`: `frequency_hz` the shift, `peak_db` the peak S/N, `duration_s` from the first hit to the end of the last, `aux1` the width (Hz), `aux2` the flutter (dB), the note the class. The `trend` table has one row for each minute with hits: the peak S/N and the mean width of the hit rows.

## Tuning: notes

The stack holds no beacon list: tune the dial to the beacon. The usual dial and range are in `data/science/science_dials.csv`.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is complex white noise and a carrier at +60 Hz, 30 dB over the noise in a 2 Hz bin, open at 5 s and 40 s of every 70 s for 18 s. While open, its frequency walks around the offset (an Ornstein-Uhlenbeck process of standard deviation spread / 2.5 and correlation time 0.05 s) and its level walks in dB (standard deviation the flutter, correlation time 3 s).

## Result pictures: notes

The display has a row every 0.512 s of ±1 kHz in cells of 4 bins, dB over the noise from -5 to 40, and keeps 120 rows.

## Try it: notes

```sh
sdrkit run beacon_events --source synth:beacon_events --seconds 75
sdrkit run beacon_events --source <driver>:<address> --freq 50
```

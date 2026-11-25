<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Whistlers and tweeks

In VLF audio from a sound card: whistlers that fall along Eckersley's law, found by a search over the dispersion, tweeks just after a sferic, and the count of sferics.

Stack id: `whistler`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 0 Hz to 100 kHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 16 kHz |
| Signal bandwidth | 10 kHz |
| Radio filter | 48 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:whistler` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Whistler path threshold | number (dB) | 6 | 0 to 15 | The least mean level of a whistler path over the floor: 1 to 15 dB, 0 for the default of 6 dB. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_dispersion` | Test signal dispersion | number (s√Hz) | 60 | 5 to 300 | D of the whistlers of the test signal. 60 is a test value: documents give about 25 to 40 s√Hz for night-time whistlers at low latitude, more by day and at higher latitude. |
| `gen_cutoff_hz` | Test signal tweek cut-off | frequency | 1.8 kHz | 1 kHz to 3 kHz | Documents give 1.6 to 1.9 kHz for the first mode (about 1.8 kHz for a reflection height of 83 km). |
| `gen_tone_snr_db` | Test signal whistler SNR | number (dB) | 15 | -20 to 40 | The power of a whistler over the power of the noise in 1 kHz. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `whistlers` | Whistlers | whole number |
| `tweeks` | Tweeks | whole number |
| `events_total` | Events | whole number |
| `sferics` | Sferics | whole number |
| `dispersion` | Last dispersion | number (s√Hz) |
| `broadband_db` | Broadband level | number (dB) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `utc` (time), `signal_time_s` (number (s)), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number), `event_note` (text) |
| `trend` | Trend per minute | `utc` (time), `events_per_min` (number), `sferics_per_min` (number) |

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
| `reset` | Reset |  |  | Empties the tables and starts the detector again; the floor is learned again. |

## Typed interface

The stack is in the family `science`, version 2.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

Key readings: `whistlers`, `tweeks`, `sferics`, `dispersion`.

### Result fields

Struct `WhistlerFields`. The result fields of `whistler`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `whistlers` | whole number | always | 1.0 | Whistlers found. |
| `tweeks` | whole number | always | 1.0 | Tweeks found. |
| `events_total` | whole number | always | 1.0 | Events in total. |
| `sferics` | whole number | always | 1.0 | Sferics counted. |
| `dispersion` | number (s√Hz) | optional | 1.0 | The dispersion of the last whistler. |
| `broadband_db` | number (dB) | optional | 1.0 | The broadband value of the newest row. |
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

Struct `WhistlerTrendRow`. A trend minute of `whistler`. The row key is the minute of the capture.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the end of the minute. |
| `events_per_min` | number | always | 1.0 | Events of a trend minute. |
| `sferics_per_min` | number | always | 1.0 | Sferics of a trend minute. |

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
| `clear` | `sdrkit_whistler_clear` | none | none |
| `reset` | `sdrkit_whistler_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_whistler_get_threshold_db` | `sdrkit_whistler_set_threshold_db` |
| `start_utc` | `sdrkit_whistler_get_start_utc` | `sdrkit_whistler_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_dispersion` | `sdrkit_whistler_gen_get_dispersion` | `sdrkit_whistler_gen_set_dispersion` |
| `gen_cutoff_hz` | `sdrkit_whistler_gen_get_cutoff_hz` | `sdrkit_whistler_gen_set_cutoff_hz` |
| `gen_tone_snr_db` | `sdrkit_whistler_gen_get_tone_snr_db` | `sdrkit_whistler_gen_set_tone_snr_db` |

## Try it

```sh
sdrkit run whistler --source synth:whistler
sdrkit run whistler --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Description

The receiver takes the real part of its input and averages groups of samples down to about 24 ksps. It makes a spectrogram of 512-point Hann transforms every 128 samples (5.3 ms, bins of 46.9 Hz) and divides each bin by its own floor: the mean of the first 200 rows, then a slow follower with a time constant of 20 s. A row value is that ratio in dB, from -10 to 40. Nothing is detected in the first 400 rows (2.1 s).

A sferic is a row whose mean value over 1 to 10 kHz is 5 dB or more and the largest of 8 rows either side. A tweek follows a sferic: the mean of the rows 16 to 48 ms later has a narrow ridge of 6 dB or more at 1400 to 2300 Hz (first mode) or 3100 to 4600 Hz (second mode), 4 dB over the values 600 Hz to each side, with no broadband energy at 6 to 9 kHz. Its frequency is where the ridge settles in the last rows. A whistler follows Eckersley's law t(f) = t0 + D / sqrt(f), with D in s sqrt(Hz) (T. L. Eckersley, 1935; R. A. Helliwell, "Whistlers and Related Ionospheric Phenomena", 1965). Every 0.5 s the receiver scores every path from 1.5 to 9 kHz through the last 4 s of rows, for D from 10 to 300 s sqrt(Hz) in steps of 7 %: the mean of the row values from 0 to 15 dB, where at least 75 % of the points are above 4 dB. The best path at the threshold or more, and 1.5 s or more from the last whistler, is a whistler at its time at 4 kHz. The path search is the shared ridge search of `sdrkit-algo-dsp` (`dispersion`), which takes the law as a parameter. Below 20 ksps the bands end at 0.45 of the working rate.

Published studies give 1.6 to 1.9 kHz for the cut-off of the first waveguide mode (about 1.8 kHz for a reflection height of 83 km) and about 25 to 40 s sqrt(Hz) for night-time whistlers at low latitude, more by day and at higher latitude. The test signal's dispersion of 60 s sqrt(Hz) is a test value, not a typical one.

Not confirmed by a public document: every rule of the detector (the floor, the tweek windows 1400 to 2300 Hz and 3100 to 4600 Hz with their 6 dB and 4 dB margins, the path from 1.5 to 9 kHz scored above 4 dB on 75 % of its points, the search of D from 10 to 300 s√Hz in steps of 7 %, the 1.5 s between whistlers). Eckersley's law holds only well below the nose frequency; tweek modes above the second are not reported.

Each event is a row of the `events` table and a `science.event` event. For a whistler, `frequency_hz` is 4000, `peak_db` the path score, `duration_s` D (1 / sqrt(1500) - 1 / sqrt(9000)), `aux1` D and `aux2` the share of the path above 4 dB. For a tweek, `duration_s` is 0.04 s, `peak_db` the ridge and `aux1` the mode (1 or 2). The `trend` table has one row a minute (events and sferics of that minute) with `utc` = start + (minute + 1) × 60 s. The `utc` of an event is `start_utc` plus the whole seconds of its time.

## Tuning: notes

The stack holds no dial: it takes sound-card input, and the usual dials are in `data/vlf/vlf_dials.csv`. The blind IQ correction of the front end is off for it.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is white noise of standard deviation 1 with sferics (2 ms of decaying noise) every 0.5 to 0.9 s away from the events, and every 20 s: a whistler from a sferic at 3 s, tweeks at 6 and 11 s, a whistler at 14 s and a tweek at 18 s. A whistler falls from 9 kHz to 1.5 kHz; a tweek has its first two modes at amplitudes 1 and 0.6, settling on the cut-off and twice it. The whole signal is then scaled by 1/16 and stated at a power of 1, so that the 8-bit model of the `synth` source (12 dB of headroom) keeps the sferic peaks.

## Result pictures: notes

The spectrogram has a row every 19 rows of the detector (0.1 s), the largest value of each pair of bins from 0.5 to 10 kHz, and keeps 120 rows.

## Try it: notes

```sh
sdrkit run whistler --source synth:whistler --seconds 8
sdrkit run whistler --source synth:whistler gen_dispersion=35
sdrkit run whistler --source <driver>:<address> --freq 0
```

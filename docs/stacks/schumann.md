<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Schumann resonances

From an ELF sensor on a sound card: the five lowest Schumann resonances (frequency, height, Q) over a 5-minute average, a spectrogram of 3 to 45 Hz, the fundamental minute by minute, and ELF transients.

Stack id: `schumann`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 0 Hz to 100 kHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 400 Hz |
| Signal bandwidth | 100 Hz |
| Radio filter | 10 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:schumann` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Transient threshold | number (dB) | 8 | 0 to 20 | A 0.5 s group this much over the median of the quiet groups is part of a transient: 1 to 20 dB, 0 for the default of 8 dB. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_fundamental_hz` | Test signal fundamental | frequency | 7.83 Hz | 5 Hz to 12 Hz | The first resonance of the test signal; the others follow in proportion. |
| `gen_transients` | Test signal transients | number | 0 | 0 to 10 | A 12 Hz transient every 60 s from 70 s, Gaussian envelope of 0.25 s: its amplitude is 6 × this level × the rms of the background. 0 for none. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `fundamental_hz` | Fundamental | frequency |
| `fundamental_db` | Fundamental height | number (dB) |
| `fundamental_q` | Fundamental Q | number |
| `peaks_found` | Resonances found | whole number |
| `averaged_s` | Averaged | duration (s) |
| `level_db` | Level | number (dB) |
| `events_total` | Events | whole number |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `peaks` | Resonances | `resonance` (whole number), `frequency_hz` (frequency), `height_db` (number (dB)), `q` (number) |
| `events` | Events | `utc` (time), `signal_time_s` (number (s)), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number), `event_note` (text) |
| `trend` | Trend per minute | `utc` (time), `fundamental_hz` (number (Hz)), `height_db` (number (dB)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `average_spectrum` | Average spectrum | Hz | dB |
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
| `reset` | Reset |  |  | Empties the tables and starts the monitor again; the average and the comparison level are learned again. |

## Typed interface

The stack is in the family `science`, version 1.3. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

### Result fields

Struct `SchumannFields`. The result fields of `schumann`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `fundamental_hz` | frequency | optional | 1.0 | The fundamental, Hz. |
| `fundamental_db` | number (dB) | optional | 1.0 | The height of the fundamental over the baseline. |
| `fundamental_q` | number | optional | 1.0 | The Q of the fundamental. |
| `peaks_found` | whole number | always | 1.0 | Resonances found. |
| `averaged_s` | duration (s) | always | 1.0 | Seconds of averaging. |
| `level_db` | number (dB) | optional | 1.0 | The level of the last group of the transient band. |
| `events_total` | whole number | always | 1.0 | Events in total. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `peaks`

Struct `SchumannPeakRow`. A resonance. The row key is its number, 1 to 5.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `resonance` | whole number | always | 1.0 | The resonance of a row of the peak table: 1 to 5. |
| `frequency_hz` | frequency | always | 1.0 | The frequency of a resonance, 0 when none was found. |
| `height_db` | number (dB) | always | 1.0 | The height of a resonance over the baseline. |
| `q` | number | always | 1.0 | The Q of a resonance. |

The column `resonance` names a row. Default order: the column `resonance`, lowest first.

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

Struct `SchumannTrendRow`. A trend minute of `schumann`. The row key is the minute of the capture.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the end of the minute. |
| `fundamental_hz` | number (Hz) | always | 1.0 | The fundamental, Hz. |
| `height_db` | number (dB) | always | 1.0 | The height of a resonance over the baseline. |

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
| `clear` | `sdrkit_schumann_clear` | none | none |
| `reset` | `sdrkit_schumann_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_schumann_get_threshold_db` | `sdrkit_schumann_set_threshold_db` |
| `start_utc` | `sdrkit_schumann_get_start_utc` | `sdrkit_schumann_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_fundamental_hz` | `sdrkit_schumann_gen_get_fundamental_hz` | `sdrkit_schumann_gen_set_fundamental_hz` |
| `gen_transients` | `sdrkit_schumann_gen_get_transients` | `sdrkit_schumann_gen_set_transients` |

## Try it

```sh
sdrkit run schumann --source synth:schumann
sdrkit run schumann --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Description

The receiver takes the real part of its input and averages groups of samples down to about 200 sps (240 samples at 48 ksps). It makes 2048-point Hann transforms every 1024 samples (5.12 s, bins of 0.098 Hz at 200 sps), each bin scaled so that white noise of variance s² reads s², and averages them with a memory of 5 minutes. The baseline of each bin from 3 to 60 Hz is the median of the average over ±5 Hz. After 60 s of averaging it looks within ±1.6 Hz of each published resonance, 7.83, 14.1, 20.3, 26.3 and 32.5 Hz (as collected in the Wikipedia article "Schumann resonances", with diurnal maxima near 9, 14 and 20 UT), for the largest ratio of the average to the baseline. A resonance needs 2 dB or more; its frequency comes from a parabola through the logarithms of three ratios, its width from the run of bins above half its excess, and its Q is the frequency over the width. The ±1.6 Hz windows keep 50 Hz and 60 Hz mains out of the peaks.

ELF transients: the 200 sps stream passes a one-pole high-pass at 5 Hz and two one-pole low-passes at 40 Hz. Its energy over groups of 0.5 s is compared with the median of the last 600 quiet groups (5 minutes), once 60 are held. A group at the threshold or more over that level starts or continues a transient; the transient ends at the first quiet group. Its time is the start of its first group, its duration the groups × 0.5 s, `peak_db` the largest excess, `frequency_hz` the zero-crossing frequency of the band during the transient, and `aux1` the comparison level (dB). Transients are not claimed to be Q-bursts.

Not confirmed by a public document: the windows of ±1.6 Hz around the published frequencies, the 2 dB least peak, the 5-minute average, the baseline (the median over ±5 Hz), and the transient rules (the 5 to 40 Hz band, the energy of 0.5 s groups against the median of the last 600 quiet groups). Transients are not claimed to be Q-bursts.

Each transient is a row of the `events` table and a `science.event` event; its `utc` is `start_utc` plus the whole seconds of its time. The `trend` table has one row a minute, the fundamental (0 when none is found) and its height, with `utc` = start + (minute + 1) × 60 s. The first spectrum needs 10.24 s of signal and the first resonances 60 s of averaging (about 66 s of signal).

## Tuning: notes

The stack holds no dial: it takes sound-card input from an ELF sensor (a radio cannot hear 8 Hz), and the usual dials are in `data/vlf/vlf_dials.csv`. The front end keeps DC, and its blind IQ correction is off for it.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal, at about 200 sps: white noise of standard deviation 1, and each resonance as 21 steady tones over 30 % of its frequency with a Gaussian weight and Rayleigh amplitudes, peaking at 10, 7, 5, 4 and 3 times the noise density. Each sample is held over a group of input samples, and 50 Hz mains of amplitude 0.5 is added at the input rate. The whole signal is then scaled by 1/4 and stated at a power of 1, so that the 8-bit model of the `synth` source (12 dB of headroom) keeps transients of level 2.

## Result fields: notes

`level_db` is the energy of the last 0.5 s group of the 5 to 40 Hz band, in dB of the input's square.

## Result tables: notes

The `peaks` table has a row for each resonance 1 to 5 once 60 s are averaged; `frequency_hz` is 0 where none was found.

## Result pictures: notes

The spectrogram has a row per spectrum (5.12 s), the largest ratio of the spectrum to the baseline in each group of 4 bins from 3 to 45 Hz, and keeps 120 rows.

## Try it: notes

```sh
sdrkit run schumann --source synth:schumann --seconds 120
sdrkit run schumann --source synth:schumann --seconds 200 gen_transients=2
sdrkit run schumann --source <driver>:<address> --freq 0
```

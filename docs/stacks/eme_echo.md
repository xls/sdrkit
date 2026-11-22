<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Moon echo test

Finds your own keyed carrier and its echo from the Moon 2.3 to 2.85 s later within ±500 Hz: delay, Doppler shift, level and spread. The stack listens while you transmit; it transmits nothing.

Stack id: `eme_echo`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 144 MHz |
| Tuning range | 50 MHz to 10.4 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 200 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:eme_echo` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_sigma` | Echo threshold | number (sigma) | 6 | 0 to 20 | The best echo score over the control mean in standard deviations of the control: 2 to 20, 0 for 6. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_moon_delay_s` | Test signal echo delay | number (s) | 2.55 | 2 to 3 | The delay of the echo after the carrier. |
| `gen_moon_doppler_hz` | Test signal echo Doppler | number (Hz) | 120 | -450 to 450 | The shift of the echo from the carrier. |
| `gen_moon_spread_hz` | Test signal echo spread | number (Hz) | 4 | 0 to 50 | The Doppler spread of the echo. |
| `gen_moon_level_db` | Test signal echo level | number (dB) | 10 | -60 to 40 | The echo over the noise in 12 Hz. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `pulses` | Own pulses | whole number |
| `echoes` | Echoes | whole number |
| `carrier_hz` | Own carrier offset | frequency |
| `transmitting` | Transmitting | yes or no |
| `noise_db` | Noise per bin | number (dBFS) |
| `last_delay_s` | Last echo delay | duration (s) |
| `last_doppler_hz` | Last echo Doppler | frequency |
| `last_level_db` | Last echo level | number (dB) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `utc` (time), `signal_time_s` (number (s)), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number), `event_note` (text) |
| `trend` | Trend per minute | `utc` (time), `echo_level_db` (number (dB)), `echo_delay_s` (number (s)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `events_per_hour` | Events per UTC hour | h |  |

## Result pictures

| Key | Label | X unit | Y unit | Cell unit |
|---|---|---|---|---|
| `spectrogram` | Spectrogram around the own carrier | Hz | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `science.event` | Science event | `text` (text), `utc` (time), `type` (text), `frequency_hz` (frequency), `peak_db` (number (dB)), `duration_s` (duration (s)), `aux1` (number), `aux2` (number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Empties the tables and starts the detector again; the noise is learned again. |

## Typed interface

The stack is in the family `science`, version 1.3. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

### Result fields

Struct `EmeEchoFields`. The result fields of `eme_echo`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `pulses` | whole number | always | 1.0 | Own pulses seen. |
| `echoes` | whole number | always | 1.0 | Echoes found. |
| `carrier_hz` | frequency | optional | 1.0 | The own carrier. |
| `transmitting` | yes or no | always | 1.0 | Transmitting now. |
| `noise_db` | number (dBFS) | optional | 1.0 | The noise per bin. |
| `last_delay_s` | duration (s) | optional | 1.0 | The delay of the last echo. |
| `last_doppler_hz` | frequency | optional | 1.0 | The Doppler shift of the last echo. |
| `last_level_db` | number (dB) | optional | 1.0 | The level of the last echo. |
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

Struct `EmeEchoTrendRow`. A trend minute of `eme_echo`. The row key is the minute of the capture.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the end of the minute. |
| `echo_level_db` | number (dB) | always | 1.0 | The largest echo level of a trend minute. |
| `echo_delay_s` | number (s) | always | 1.0 | The mean delay of a trend minute. |

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
| `clear` | `sdrkit_eme_echo_clear` | none | none |
| `reset` | `sdrkit_eme_echo_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_sigma` | `sdrkit_eme_echo_get_threshold_sigma` | `sdrkit_eme_echo_set_threshold_sigma` |
| `start_utc` | `sdrkit_eme_echo_get_start_utc` | `sdrkit_eme_echo_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_moon_delay_s` | `sdrkit_eme_echo_gen_get_moon_delay_s` | `sdrkit_eme_echo_gen_set_moon_delay_s` |
| `gen_moon_doppler_hz` | `sdrkit_eme_echo_gen_get_moon_doppler_hz` | `sdrkit_eme_echo_gen_set_moon_doppler_hz` |
| `gen_moon_spread_hz` | `sdrkit_eme_echo_gen_get_moon_spread_hz` | `sdrkit_eme_echo_gen_set_moon_spread_hz` |
| `gen_moon_level_db` | `sdrkit_eme_echo_gen_get_moon_level_db` | `sdrkit_eme_echo_gen_set_moon_level_db` |

## Try it

```sh
sdrkit run eme_echo --source synth:eme_echo
sdrkit run eme_echo --source iiod:192.168.2.1 --freq 144
```
<!-- generated: end -->

## Description

The receiver averages the input in groups of max(1, round(rate / 2000)) samples to about 2 ksps (4 at 8 ksps, 125 at 250 ksps). Transforms of 512 points (Hann) every 128 samples give rows every 64 ms of bins of about 3.9 Hz. Per row it keeps the strongest bin and the noise, the median bin over 0.693. The noise of the stack is the mean of the first 60 rows, then follows the rows with a time constant of 30 s while you do not transmit. The rows of the last 24 s are kept.

Your transmission is on when the strongest bin is 25 dB over the noise in a 3.9 Hz bin, and off after 3 rows below. A pulse of 0.5 s or more is kept with its carrier bin (the median of the strongest bins of its rows) and its power (their mean).

Each pulse is searched once, when 3.9 s and twice its length have passed. A window of `len` = max(2, rows - 2) rows and three bins has a score: its mean power. Control scores, at delays from 3.15 s plus the window to 0.6 s more and every third bin within ±500 Hz of your carrier, give a mean and a standard deviation; at least 20 are needed. The best score at delays from 2.3 to 2.85 s and every bin within ±500 Hz is an echo when it is the threshold or more standard deviations over the control mean. Windows that overlap a transmission are skipped, so pulses should be about 10 s apart. The delay is refined by a parabola through the scores one row either side, by at most half a row. The spread runs from the best bin to each side while the score keeps over half of the best's excess.

The delay window covers the Earth-Moon distance with margin: the round trip at the mean distance of 384 400 km is about 2.56 s. Public guides give about 300 to 350 Hz of Doppler shift at 144 MHz (one gives 440 Hz); the ±500 Hz window covers them all (J. Taylor, "Lunar echoes"; B. Atkins, "EME echo Doppler spreading and lunar libration").

Not confirmed by a public document: every rule of the detector (the 3.9 Hz bins, the noise of the first 60 rows then a 30 s time constant, the own carrier at 25 dB over the noise for 0.5 s or more, the search 3.9 s plus twice the pulse length after its start, the control delays from 3.15 s plus the window, at least 20 control scores, the threshold of 6 standard deviations, the spread at half the best excess). The delay window of 2.3 to 2.85 s covers the Earth-Moon distance with margin; ±500 Hz covers the Doppler shift that public guides give at 144 MHz (about 300 to 350 Hz). The level of an echo is its three-bin score over the control mean, not a level in 12 Hz. Pulses about 10 s apart: a window over another transmission is skipped.

Each echo is a row of the `events` table and a `science.event` event of type `echo`: the time of the pulse, `frequency_hz` the Doppler shift from your carrier, `peak_db` the best score over the control mean, `duration_s` the window, `aux1` the delay (s), `aux2` the spread (Hz); the note gives the echo below your carrier in dB and the z score. The `trend` table has one row a minute: the largest echo level and the mean delay of the pulses of that minute.

## Tuning: notes

The usual dial and range are in `data/science/science_dials.csv`.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is complex white noise; a carrier at +30 Hz, 50 dB over the noise in 4 Hz, keyed at 5 s and 12 s of every 14 s for 1.5 s (the stack learns the noise in the first 60 rows, 3.84 s); its echo 2.55 s later, +120 Hz, spread by a random frequency walk (an Ornstein-Uhlenbeck process of standard deviation spread / 2.5 and correlation time 0.3 s), 10 dB over the noise in 12 Hz.

## Result pictures: notes

The display has a row every 4 rows (0.256 s) of ±500 Hz around your carrier, bins in pairs, dB over the noise from -5 to 40, and keeps 120 rows.

## Try it: notes

```sh
sdrkit run eme_echo --source synth:eme_echo --seconds 34
sdrkit run eme_echo --source iiod:192.168.2.1 --freq 144
```

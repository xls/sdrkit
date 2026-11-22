<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CODAR / HF ocean radar

Coastal HF ocean radars (CODAR SeaSonde, WERA): their FMCW sweeps, repetition, band and keyed ID (receive only).

Stack id: `hfradar`. Group: HF utility.

CODAR / HF ocean radar, receive only. ```text iq -> [Hann FFT ~1 ms, floor, burst guard, peaks, tracks] --+--> hfradar.sweep --+--> [radars, events, waterfall, Morse ID] -> results +--> excess 5 ms ----+ ```

Signal types: HF ocean radar.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 13.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 240 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hfradar` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `scan` | Scan the radar bands | yes or no | false |  | Step through the radar bands in parts that the sample rate takes in, and listen to each for the dwell time. A radar heard on one part is not heard on the others meanwhile. Off: one position. |
| `dwell_s` | Dwell | duration (s) | 20 s | 5 to 300 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_signal` | Signal | choice | `radars` | `radars` (Radars), `nothing` (Nothing), `radars_with_id` (Radars with an ID), `junk` (Junk) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `band` | Band now | text |
| `radar_count` | Radars | whole number |
| `sweeps_counted` | Sweeps counted | whole number |
| `tracks_rejected` | Tracks rejected | whole number |
| `broadband_bursts` | Broadband bursts | whole number |
| `snr_db` | SNR | number (dB) |
| `scan_stops` | Scan stops | whole number |
| `waterfall_row` | Waterfall row | duration (s) |
| `waterfall_newest` | Newest waterfall row | duration (s) |
| `clock` | Clock | choice |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `radars` | Radars | `number` (whole number), `centre` (frequency), `offset_hz` (frequency), `band` (text), `width` (frequency), `rate_hz_s` (number (Hz/s)), `direction` (choice), `repeat` (duration (s)), `snr_db` (number (dB)), `level_dbfs` (number (dBFS)), `sweeps` (whole number), `first` (duration (s)), `last` (duration (s)), `first_heard` (time), `last_heard` (time), `id` (text), `id_wpm` (number (wpm)), `id_seen` (whole number), `hint` (text) |
| `heard` | Heard | `start` (duration (s)), `time` (time), `number` (whole number), `frequency` (frequency), `band` (text), `snr_db` (number (dB)) |
| `bands` | Radar bands | `name` (text), `low` (frequency), `high` (frequency), `middle` (frequency), `range_class` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `sweep_marks` | Sweep marks | s | Hz |
| `snr_history` | SNR history | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfradar.radar_heard` | Radar heard | `radar` (whole number), `band` (text), `width` (frequency), `direction` (choice), `source` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `band.tune` | Tune to a band | `row` (text, required) |  | Asks the host to tune to the middle of an oceanographic radar band. The radio takes in one part of the band at a time (1.8 MHz at 2 Msps, so a whole band at once from about 12 Msps for 12 - 14 MHz): a wide radio sees all radars of a band together, a narrow one needs the scan. Refused while the scan is on. |

## Typed interface

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: pictures.

### Result fields

Struct `HfradarFields`. The result fields of the HF ocean radar stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | text | optional | 1.0 | The radar band the radio is in, if it is in one. |
| `radar_count` | whole number | always | 1.0 | Radars shown. |
| `sweeps_counted` | whole number | always | 1.0 | Sweeps counted. |
| `tracks_rejected` | whole number | always | 1.0 | Tracks rejected. |
| `broadband_bursts` | whole number | always | 1.0 | Broadband bursts. |
| `snr_db` | number (dB) | optional | 1.0 | The best SNR of the radars heard. Absent without a radar. |
| `scan_stops` | whole number | optional | 1.0 | The stops of the scan, while it steps. |
| `waterfall_row` | duration (s) | always | 1.0 | The time of one waterfall row. |
| `waterfall_newest` | duration (s) | always | 1.0 | The signal time of the newest waterfall row. |
| `clock` | choice | always | 1.0 | Where the times come from. |

### Table `radars`

Struct `HfradarRadarRow`. A radar. The row key is the radar number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `number` | whole number | always | 1.0 | The radar number. |
| `centre` | frequency | always | 1.0 | The centre of the sweep. |
| `offset_hz` | frequency | always | 1.0 | The centre against the tuned frequency. |
| `band` | text | optional | 1.0 | The radar band, if the radar is in one. |
| `width` | frequency | always | 1.0 | The width of the sweep. |
| `rate_hz_s` | number (Hz/s) | always | 1.0 | The sweep rate. |
| `direction` | choice | always | 1.0 | The direction of the sweep. |
| `repeat` | duration (s) | optional | 1.0 | The repetition period of the sweeps, when measured. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level, dBFS. |
| `sweeps` | whole number | always | 1.0 | Sweeps counted. |
| `first` | duration (s) | always | 1.0 | The signal time when first heard. |
| `last` | duration (s) | always | 1.0 | The signal time when last heard. |
| `first_heard` | time | optional | 1.0 | The wall time when first heard, when the clock is known. |
| `last_heard` | time | optional | 1.0 | The wall time when last heard, in whole seconds, when the clock is known. |
| `id` | text | optional | 1.0 | The keyed ID, when read. |
| `id_wpm` | number (wpm) | optional | 1.0 | The speed of the keyed ID. |
| `id_seen` | whole number | always | 1.0 | Times the ID was seen. |
| `hint` | text | always | 1.0 | What the radar may be. |

The column `id` names a row.

### Table `heard`

Struct `HfradarHeardRow`. A radar heard. The row key is the sequence number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time. |
| `time` | time | optional | 1.0 | The wall time, when the clock is known. |
| `number` | whole number | always | 1.0 | The radar number. |
| `frequency` | frequency | always | 1.0 | The centre of the sweep. |
| `band` | text | optional | 1.0 | The radar band, if the radar is in one. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio. |

### Table `bands`

Struct `HfradarBandRow`. An oceanographic radar band. The row key is the band name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.4 | The band name ("12 - 14 MHz"). |
| `low` | frequency | always | 1.4 | The lower edge of the working range. |
| `high` | frequency | always | 1.4 | The upper edge of the working range. |
| `middle` | frequency | always | 1.4 | The middle of the band: where `band.tune` tunes the radio. |
| `range_class` | text | always | 1.4 | The range class of the radars of the band. |

The column `name` names a row. Default order: the column `low`, lowest first.

### Event `hfradar.radar_heard`

Struct `HfradarRadarHeardEvent`. A radar heard first, or again after a minute's pause. `source` is the keyed ID when it was read.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `radar` | whole number | always | 1.0 | The radar number. |
| `band` | text | optional | 1.0 | The radar band, if the radar is in one. |
| `width` | frequency | always | 1.0 | The width of the sweep. |
| `direction` | choice | always | 1.0 | The direction of the sweep. |
| `source` | text | optional | 1.0 | The keyed ID, when read. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hfradar_clear` | none | none |
| `band.tune` | `sdrkit_hfradar_band_tune` | `sdrkit_hfradar_band_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `scan` | `sdrkit_hfradar_get_scan` | `sdrkit_hfradar_set_scan` |
| `dwell_s` | `sdrkit_hfradar_get_dwell_s` | `sdrkit_hfradar_set_dwell_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_signal` | `sdrkit_hfradar_gen_get_signal` | `sdrkit_hfradar_gen_set_signal` |

## Try it

```sh
sdrkit run hfradar --source synth:hfradar
sdrkit run hfradar --source <driver>:<address> --freq 13.5
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## What the stack measures and what it guesses

The stack receives only. It finds the linear FM sweeps of coastal HF ocean radars and measures them. It decodes no data.

Measured from the signal, for each radar:

- Centre frequency (Hz, absolute when the radio frequency is known) and offset from the radio centre.
- Sweep width (Hz) and sweep rate (Hz/s), from a straight-line fit of each sweep.
- Direction (rising or falling).
- Repetition period (s), from the median of the intervals between sweep starts. With fewer than 3 usable intervals the
  value is absent, not zero.
- SNR (dB) and peak level (dBFS), the sweeps counted, and the first and last time heard.

Guessed, never measured:

- The hint. It begins with "possible": "possible SeaSonde-like radar" for a repetition of 0.35 to 1.2 s and a width of
  15 to 150 kHz, else "possible HF ocean radar". The range resolution in the hint is c / 2B from the measured width.
  The hint is not an identification.
- The keyed ID. It is always marked unverified.

The `bands` table lists the oceanographic radar bands. The command `band.tune` asks the host to tune to the middle of
one band. It is refused while the scan is on. With a radio that is narrower than a band, use the `scan` setting.

## Limits

- Bins within 3 kHz of a steady signal take no peaks: a bin is steady when a second floor, held at 1.5 times itself so
  that a passing sweep barely raises it, stands more than 3 dB over its median. This keeps the speech of broadcast
  carriers from making false sweeps (in the tests a broadcast band gives no sweep). A sweep of 4 or more points
  coasts through such bins for up to 0.5 s. A sweep that crosses several strong carriers can still break into
  fragments: then its width reads short, or its parts are listed as separate radars.
- A radar weaker than about 8 dB in one bin of the 1 ms spectrum is not found (measured: 10 of 10 seeds at 8 dB,
  4 of 10 at 7 dB, none at 6 dB).
- Only FMCW sweeps are found. Pulsed radars are not found.

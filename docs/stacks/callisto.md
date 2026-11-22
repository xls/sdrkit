<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Solar burst spectrogram

A spectrogram in 62.5 kHz bins and 0.25 s rows, the e-CALLISTO resolution; solar radio bursts with start, duration, strength, bandwidth and drift rate, and e-CALLISTO FITS files.

Stack id: `callisto`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 200 MHz |
| Tuning range | 45 MHz to 870 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:callisto` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Burst threshold per bin | number (dB) | 1 | 0 to 20 | The least excess of an active bin over its background: 0.5 to 20 dB, 0 for the default of 1 dB. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |
| `fits` | Write FITS files | yes or no | false |  | Writes e-CALLISTO FITS files of the spectrogram into the file folder of the host's storage. |
| `fits_folder` | FITS folder | text | `callisto` |  | A folder of the host's storage. |
| `station_code` | Instrument code | text | `LOCAL` |  | Letters and digits: the start of the file name, INSTRUME and ORIGIN. |
| `focus_code` | Focus code | whole number | 59 | 0 to 63 | The two digits at the end of the file name (the manual lists 00 to 63, default 59). |
| `file_s` | File length | number (s) | 900 | 10 to 3600 | Files cover whole multiples of this length in UTC (e-CALLISTO: 900 s). |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `fits_bitpix` | Pixel type | choice | `16` | `16` (16 bits (the manual's example)), `8` (8 bits) | BITPIX of the image: 16 as the manual's example header (values 0 to 255), or 8. |
| `db_per_digit` | dB per digit | number (dB) | 0.4 | 0.01 to 10 | The power step of one digit. Not given by a public document that was read. |
| `zero_dbfs` | Level of digit 0 | number (dBFS) | -100 | -200 to 0 | The power of digit 0, dB of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_level_db` | Test signal burst level | number (dB) | 6 | -10 to 30 | The power density of a burst over the noise density. |
| `gen_burst_drift_hz_s` | Test signal burst drift | number (Hz/s) | 0 | -10000000 to 10000000 | The drift of the bursts; 0 for 0.2 × rate over a quarter of the cycle. |

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

The stack is in the family `science`, version 1.1. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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
| `clear` | `sdrkit_callisto_clear` | none | none |
| `reset` | `sdrkit_callisto_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_callisto_get_threshold_db` | `sdrkit_callisto_set_threshold_db` |
| `start_utc` | `sdrkit_callisto_get_start_utc` | `sdrkit_callisto_set_start_utc` |
| `fits` | `sdrkit_callisto_get_fits` | `sdrkit_callisto_set_fits` |
| `fits_folder` | `sdrkit_callisto_get_fits_folder` | `sdrkit_callisto_set_fits_folder` |
| `station_code` | `sdrkit_callisto_get_station_code` | `sdrkit_callisto_set_station_code` |
| `focus_code` | `sdrkit_callisto_get_focus_code` | `sdrkit_callisto_set_focus_code` |
| `file_s` | `sdrkit_callisto_get_file_s` | `sdrkit_callisto_set_file_s` |
| `fits_bitpix` | `sdrkit_callisto_get_fits_bitpix` | `sdrkit_callisto_set_fits_bitpix` |
| `db_per_digit` | `sdrkit_callisto_get_db_per_digit` | `sdrkit_callisto_set_db_per_digit` |
| `zero_dbfs` | `sdrkit_callisto_get_zero_dbfs` | `sdrkit_callisto_set_zero_dbfs` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_level_db` | `sdrkit_callisto_gen_get_level_db` | `sdrkit_callisto_gen_set_level_db` |
| `gen_burst_drift_hz_s` | `sdrkit_callisto_gen_get_burst_drift_hz_s` | `sdrkit_callisto_gen_set_burst_drift_hz_s` |

## Try it

```sh
sdrkit run callisto --source synth:callisto
sdrkit run callisto --source iiod:192.168.2.1 --freq 200
```
<!-- generated: end -->

## Description

The receiver makes a spectrogram of the whole capture. The transform size starts at 8 and doubles while it is below 256 and rate / (2 N) is at least 0.7 of 62.5 kHz (32 points at 2 Msps, 8 at 250 ksps). Frames of N samples without overlap get a Hann window; the power of each bin is divided by the sum of the squared window. A row is the mean of the frames of about 0.25 s, in increasing frequency from -rate / 2.

The bursts follow the rules of `jove` with one change: a row is active when 2 bins or more are active, because solar bursts drift across the band. A burst of 4 active rows or more has a drift: the least-squares slope of its excess-weighted centroid against time. It is a `drifting burst` when the drift times its duration is 2 bins or more, else a `burst`. Burst types I to V are not named: no public document read gives thresholds for them.

The e-CALLISTO network measures 45 to 870 MHz with 62.5 kHz resolution and about 300 kHz radiometric bandwidth, 200 channels a sweep and 4 sweeps a second, in files of 900 s (e-CALLISTO network pages; ETH CALLISTO Operating Manual). The stack is not a CALLISTO spectrometer: one capture covers a few MHz and there is no frequency sweep.

With `fits` on, the stack writes FITS files through the host's storage into `fits_folder`. A file covers a whole multiple of `file_s` in UTC (900 s as the network) and is written when the next one starts; its name is `<station>_<yyyymmdd>_<hhmmss>_<ff>.fit` (ETH CALLISTO Operating Manual; the archive stores the files gzipped as `.fit.gz`, the stack does not gzip). The primary header has the keywords of the manual's chapter 11 in its order: `SIMPLE`, `BITPIX`, `NAXIS`, `NAXIS1` (time samples), `NAXIS2` (channels), `EXTEND`, the four `COMMENT` lines, `DATE` (`yyyy-mm-dd`), `CONTENT`, `ORIGIN`, `TELESCOP` (`Radio Spectrometer`), `INSTRUME`, `OBJECT` (`Sun`), `DATE-OBS` and `DATE-END` (`yyyy/mm/dd`), `TIME-OBS` and `TIME-END` (`hh:mm:ss.sss`), `BZERO = 0.`, `BSCALE = 1.`, `BUNIT = 'digits'`, `DATAMIN`, `DATAMAX`, `CRVAL1` (seconds of the day), `CRPIX1`, `CTYPE1 = 'Time [UT]'`, `CDELT1` (the row time), `CRVAL2` (the highest channel, MHz), `CRPIX2`, `CTYPE2 = 'Frequency [MHz]'`, `CDELT2` (minus the bin, MHz), `HISTORY`, `OBS_LAT`, `OBS_LAC`, `OBS_LON`, `OBS_LOC`, `OBS_ALT` (from the station position), `FRQFILE`, `PWM_VAL`. The image holds digits from 0 to 255 in 16-bit pixels, as the manual's example header (8-bit pixels are a setting); its rows are the channels from the highest frequency down. One binary table extension follows with the columns `TIME` (seconds of the day, one per time sample) and `FREQUENCY` (MHz, one per channel), as the readers of e-CALLISTO files take them; the manual describes two tables. The stack's reader takes either layout and `BITPIX` 8 or 16. The files are written with the shared FITS writer `sdrkit-codec-fits` (FITS Standard 4.0).

Not confirmed by a public document: every rule of the detector (the background learned from 20 quiet rows with a 20 s time constant, a bin active over max(threshold, 5 standard deviations), a burst over 2 bins, closed after 2 quiet rows, drifting when 4 rows or more move 2 bins or more). Burst types I to V are not named. The capture has no frequency sweep, so a file holds the bins of the capture, not 200 channels. In the file: the digits (dB per digit and the level of digit 0 are settings), the 16-bit pixels (the manual's example; 8 bits is a setting), one binary table with TIME in seconds of the day (the readers in use; the manual describes two tables), FRQFILE 'NONE', PWM_VAL 0, DATE the date of the first row, files of whole multiples of the file length in UTC; the archive gzips the files, the stack does not.

Each burst is a row of the `events` table and a `science.event` event: `frequency_hz` the centroid of its first row (Hz from the centre), `peak_db` its strength, `duration_s`, `aux1` its drift (Hz/s, 0 with fewer than 4 rows), `aux2` its bandwidth (Hz). The `trend` table has one row a minute: the mean level minus that of the first minute, and the bursts that started in the minute.

## Tuning: notes

The usual dial and range are in `data/science/science_dials.csv`. A wide-band log-periodic antenna or similar is needed.

## Settings: notes

The test signal is complex white noise with drifting bursts: combs of tones about 5 kHz apart, 0.2 × rate wide, starting at -0.25 × rate and drifting 0.2 × rate over a quarter of the cycle, at 0.2 and 0.6 of a cycle of clamp(8e6 / rate, 8, 60) s, 0.25 of it long. There is no burst in the first 7.5 s, while the detector learns its background.

## Result pictures: notes

The spectrogram has the excess of every bin of every row, from -5 to 30 dB, and keeps 150 rows.

## Try it: notes

```sh
sdrkit run callisto --source synth:callisto --seconds 16
sdrkit run callisto --source synth:callisto fits=true file_s=10 --seconds 30
sdrkit run callisto --source iiod:192.168.2.1 --freq 200
```

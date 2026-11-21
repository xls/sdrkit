<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SuperSID (VLF)

The level of each monitored VLF transmitter every 5 s, sudden ionospheric disturbances (as from a solar flare), terminator marks and the SuperSID data file.

Stack id: `supersid`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 22 kHz |
| Tuning range | 3 kHz to 100 kHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 19.2 kHz |
| Radio filter | 200 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:supersid` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `transmitters` | Monitored transmitters | text | empty |  | One record per line or between semicolons: label,frequency_hz or label,frequency_hz,lat,lon. At most 32. The host fills it from its transmitter file. |
| `step_db` | Step threshold | number (dB) | 1.5 | 0.3 to 12 | The change of the 2-minute mean against 10 minutes earlier that starts an event. |
| `average_s` | Sample interval | number (s) | 5 | 1 to 60 | The time over which the spectra are summed for one level; also the log interval of the file. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |
| `log_format` | Data file | choice | `off` | `off` (Off), `sid_format` (sid_format (one file per transmitter)), `supersid_format` (supersid_format (one file, all transmitters)) | Writes the SuperSID text data file, one per UTC day, into the data file folder. |
| `site` | Site name | text | empty |  | The Site line of the data file and the start of its name. |
| `monitor_id` | Monitor id | text | empty |  | The MonitorID line of the data file. |
| `log_folder` | Data file folder | text | `supersid` |  | A folder of the host's storage. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_flare` | Test signal flare | yes or no | false |  | The first carrier 3 dB stronger from 17 minutes on. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `listed_n` | Transmitters listed | whole number |
| `in_span_n` | Transmitters in span | whole number |
| `present_n` | Transmitters present | whole number |
| `intervals` | Intervals measured | whole number |
| `bin_hz` | Bin width | frequency |
| `events_total` | Events | whole number |
| `last_event_tx` | Last event transmitter | text |
| `last_event_rise_db` | Last event rise | number (dB) |
| `last_event_age_s` | Last event age | duration (s) |
| `log_file` | Data file | text |
| `log_error` | Data file error | text |
| `text` | Text | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `transmitters` | Transmitters | `label` (text), `freq_hz` (frequency), `in_span` (yes or no), `present` (yes or no), `level_db` (number (dB)), `absolute_dbfs` (number (dBFS)), `baseline_db` (number (dB)), `rise_db` (number (dB)), `event_on` (yes or no), `event_peak_db` (number (dB)), `sunlit` (number), `series` (text) |
| `sid_events` | Sudden changes | `label` (text), `freq_hz` (frequency), `start_utc` (time), `signal_time_s` (number (s)), `rise_db` (number (dB)), `peak_db` (number (dB)), `end_utc` (time) |
| `marks` | Terminator marks | `label` (text), `point` (text), `mark` (text), `utc` (time) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `minutes_01` | Minute means, transmitter 1 | s | dB |
| `minutes_02` | Minute means, transmitter 2 | s | dB |
| `minutes_03` | Minute means, transmitter 3 | s | dB |
| `minutes_04` | Minute means, transmitter 4 | s | dB |
| `minutes_05` | Minute means, transmitter 5 | s | dB |
| `minutes_06` | Minute means, transmitter 6 | s | dB |
| `minutes_07` | Minute means, transmitter 7 | s | dB |
| `minutes_08` | Minute means, transmitter 8 | s | dB |
| `minutes_09` | Minute means, transmitter 9 | s | dB |
| `minutes_10` | Minute means, transmitter 10 | s | dB |
| `minutes_11` | Minute means, transmitter 11 | s | dB |
| `minutes_12` | Minute means, transmitter 12 | s | dB |
| `minutes_13` | Minute means, transmitter 13 | s | dB |
| `minutes_14` | Minute means, transmitter 14 | s | dB |
| `minutes_15` | Minute means, transmitter 15 | s | dB |
| `minutes_16` | Minute means, transmitter 16 | s | dB |
| `minutes_17` | Minute means, transmitter 17 | s | dB |
| `minutes_18` | Minute means, transmitter 18 | s | dB |
| `minutes_19` | Minute means, transmitter 19 | s | dB |
| `minutes_20` | Minute means, transmitter 20 | s | dB |
| `minutes_21` | Minute means, transmitter 21 | s | dB |
| `minutes_22` | Minute means, transmitter 22 | s | dB |
| `minutes_23` | Minute means, transmitter 23 | s | dB |
| `minutes_24` | Minute means, transmitter 24 | s | dB |
| `minutes_25` | Minute means, transmitter 25 | s | dB |
| `minutes_26` | Minute means, transmitter 26 | s | dB |
| `minutes_27` | Minute means, transmitter 27 | s | dB |
| `minutes_28` | Minute means, transmitter 28 | s | dB |
| `minutes_29` | Minute means, transmitter 29 | s | dB |
| `minutes_30` | Minute means, transmitter 30 | s | dB |
| `minutes_31` | Minute means, transmitter 31 | s | dB |
| `minutes_32` | Minute means, transmitter 32 | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `sid.event` | Sudden ionospheric disturbance | `label` (text), `start_utc` (time), `rise_db` (number (dB)), `peak_db` (number (dB)), `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Clears, and starts the levels of the hour and the judging again (15 minutes); keeps the minute means. |

## Typed interface

The stack is in the family `science`, version 1.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SupersidFields`. The result fields of `supersid`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `listed_n` | whole number | always | 1.0 | Transmitters listed. |
| `in_span_n` | whole number | always | 1.0 | Transmitters in span. |
| `present_n` | whole number | always | 1.0 | Transmitters present. |
| `intervals` | whole number | always | 1.0 | Intervals measured. |
| `bin_hz` | frequency | optional | 1.0 | The width of a bin of the spectrum. |
| `events_total` | whole number | always | 1.0 | Events in total. |
| `last_event_tx` | text | optional | 1.0 | The transmitter of the last event. |
| `last_event_rise_db` | number (dB) | optional | 1.0 | The rise of the last event. |
| `last_event_age_s` | duration (s) | optional | 1.0 | The age of the last event. |
| `log_file` | text | optional | 1.0 | The data file written last. |
| `log_error` | text | optional | 1.0 | Why the data file is not written. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `transmitters`

Struct `SupersidTransmitterRow`. A monitored transmitter, in list order. The row key is its place in the list, two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `label` | text | always | 1.0 | A transmitter's label. |
| `freq_hz` | frequency | always | 1.0 | The frequency a station was heard on. |
| `in_span` | yes or no | always | 1.0 | In span. |
| `present` | yes or no | always | 1.0 | Present. |
| `level_db` | number (dB) | optional | 1.0 | The level over the local noise. |
| `absolute_dbfs` | number (dBFS) | optional | 1.0 | The absolute level. |
| `baseline_db` | number (dB) | optional | 1.0 | The median of the last hour. |
| `rise_db` | number (dB) | optional | 1.0 | The mean of the last 2 minutes less the baseline. |
| `event_on` | yes or no | always | 1.0 | An event is on. |
| `event_peak_db` | number (dB) | optional | 1.0 | The peak step of the event that is on. |
| `sunlit` | number | optional | 1.0 | The sunlit share of the path. |
| `series` | text | always | 1.0 | The key of the series of minute means. |

### Table `sid_events`

Struct `SupersidEventRow`. A row of the table `sid_events`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `label` | text | always | 1.0 | A transmitter's label. |
| `freq_hz` | frequency | always | 1.0 | The frequency a station was heard on. |
| `start_utc` | time | always | 1.0 | The UTC of an event's start. |
| `signal_time_s` | number (s) | always | 1.0 | The signal time of an event's start. |
| `rise_db` | number (dB) | always | 1.0 | The mean of the last 2 minutes less the baseline. |
| `peak_db` | number (dB) | always | 1.0 | The peak step of an event. |
| `end_utc` | time | optional | 1.0 | The UTC of an event's end. |

### Table `marks`

Struct `SupersidMarkRow`. A terminator mark of the UTC day. The row key is its place in time order, three digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `label` | text | always | 1.0 | A transmitter's label. |
| `point` | text | always | 1.0 | The point of the path of a mark. |
| `mark` | text | always | 1.0 | The crossing of a mark. |
| `utc` | time | always | 1.0 | The UTC of a mark. |

### Event `sid.event`

Struct `SupersidSidEvent`. An event `sid.event`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `label` | text | always | 1.0 | A transmitter's label. |
| `start_utc` | time | always | 1.0 | The UTC of an event's start. |
| `rise_db` | number (dB) | always | 1.0 | The mean of the last 2 minutes less the baseline. |
| `peak_db` | number (dB) | always | 1.0 | The peak step of an event. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_supersid_clear` | none | none |
| `reset` | `sdrkit_supersid_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `transmitters` | `sdrkit_supersid_get_transmitters` | `sdrkit_supersid_set_transmitters` |
| `step_db` | `sdrkit_supersid_get_step_db` | `sdrkit_supersid_set_step_db` |
| `average_s` | `sdrkit_supersid_get_average_s` | `sdrkit_supersid_set_average_s` |
| `start_utc` | `sdrkit_supersid_get_start_utc` | `sdrkit_supersid_set_start_utc` |
| `log_format` | `sdrkit_supersid_get_log_format` | `sdrkit_supersid_set_log_format` |
| `site` | `sdrkit_supersid_get_site` | `sdrkit_supersid_set_site` |
| `monitor_id` | `sdrkit_supersid_get_monitor_id` | `sdrkit_supersid_set_monitor_id` |
| `log_folder` | `sdrkit_supersid_get_log_folder` | `sdrkit_supersid_set_log_folder` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_flare` | `sdrkit_supersid_gen_get_flare` | `sdrkit_supersid_gen_set_flare` |

## Try it

```sh
sdrkit run supersid --source synth:supersid
sdrkit run supersid --source <driver>:<address> --freq 0.022
```
<!-- generated: end -->

## Description

The level of each monitored VLF transmitter (about 15 to 50 kHz) in every sample interval, 5 s by default, and its sudden changes. A solar flare raises the ionisation of the D region and changes the received level within minutes: a sudden ionospheric disturbance (SID). At 250 ksps or more the front end takes a 24 ksps channel around the dial; at a lower rate the input is used as it is. The receiver sums the power spectra of consecutive Hann windows (the next power of two at or above a third of the rate: 8192 points, 2.93 Hz bins at 24 ksps) over each interval. The level of a transmitter is the mean power of the bins within ±60 Hz of its carrier over the mean of the quieter of the two noise bands 150 to 400 Hz away, in dB; a transmitter is present from 6 dB. A transmitter is in span when its offset from the dial is less than 0.85 of half the rate; above 300 ksps without the front end nothing is in span. After 15 minutes of levels, an event starts when the mean of the last 2 minutes differs from the mean of the 2 minutes that ended 10 minutes earlier by the step threshold (1.5 dB) or more, the transmitter is present and the hourly median is 6 dB or more; it ends when the 2-minute mean is within half the step of the hourly median. Rises and falls both count. The day-night terminator moves the level more slowly and gives no event. When the station's position and a transmitter's position are known, the receiver lists the times of sunrise and sunset of the UTC day at the ground (Sun at −0.833°) and at the D region (85 km, Sun at −9.3°) for the transmitter, the great-circle midpoint and the receiver, and the sunlit share of the path (21 points). The stack holds no transmitter list: the host fills the setting `transmitters` from its data file (`data/vlf/supersid_transmitters.csv`; `supersid::list::setting_text_from_csv` turns its rows into the setting text). A real flare should be checked against satellite X-ray data; the stack does not do this. Not confirmed by a public document: the 5 s interval, the level rule, the 6 dB presence limit, the event rule (2-minute means, 10 minutes back, 1.5 dB, the end at half the step, nothing judged before 15 minutes) and the 85 km height of the D region. In the data file, the unit of the values, the file names, the form of UTC_Offset, the empty Longitude and Latitude of a station without a position and the rows of 0 before the first reading are sdrkit's choices.

## Tuning: notes

The usual dials are in `data/vlf/vlf_dials.csv`, for the host to offer. The 24 ksps channel of the front end is flat to ±9.6 kHz; a transmitter between 9.6 and 10.2 kHz from the dial is still in span, and its level over noise holds, but its absolute level reads low.

## Settings: notes

Advanced settings. The owner works without any of them set.

The text of `transmitters`, for example from the data file:

```text
NAA,24000,44.6444,-67.2844
DHO38,23400,53.0872,7.6086
GQD,22100
```

## The data file

With `log_format` set, the receiver writes the SuperSID text data file through the host's storage into `log_folder`, one file per UTC day: `<site>_<label>_<YYYY-MM-DD>.csv` per transmitter in `sid_format` (a label that is listed twice gets `_<frequency>` after it), `<site>_<YYYY-MM-DD>.csv` (or `supersid_<YYYY-MM-DD>.csv` without a site) in `supersid_format`.

The header lines are `# Key = Value` in this order: `Site`, `Contact`, `Longitude`, `Latitude`, two bare `#` lines, `UTC_Offset` (`+HH:MM`), `TimeZone`, `UTC_StartTime` (`YYYY-MM-DD HH:MM:SS`, 00:00:00 of the file's day), `LogInterval` (s), `LogType` (`raw`), `MonitorID`, then `StationID` and `Frequency` (Hz) in `sid_format`, or `Stations` and `Frequencies` (lists separated by `, `) in `supersid_format`. `Supersid_Version` is not written. Without a station position, `Longitude` and `Latitude` are empty.

No document defines the unit of the values. The receiver writes the power in the transmitter's signal band (±60 Hz) relative to full scale, linear (a full-scale tone is 1), and says so in the `Contact` line; 0 means no reading (out of span). A `sid_format` line is `YYYY-MM-DD HH:MM:SS, <value>` with the value in `%.15f`, the time the start of the interval. A `supersid_format` line has no time: the values of all transmitters in the header's order, each `%.15f`, joined by `, `; row n is 0:00:00 UTC + n × LogInterval. A file that starts during the day holds rows of 0 from 0:00:00 up to the first interval measured. A file that is already there is continued: a `sid_format` file gets its new lines at the end; a `supersid_format` file with the same stations and frequencies is padded with rows of 0 up to the present row; one with other stations is left alone and the receiver writes a new file with the suffix `_2`, `_3` and so on.

## Result tables: notes

The series `minutes_01` to `minutes_32` hold the minute means of the last 24 hours of the transmitters in list order (signal time in s, level in dB); the column `series` of the table names a transmitter's series.

## Events: notes

`rise_db` is the 2-minute mean less the hourly median at the start; `peak_db` is the change against 10 minutes earlier.

## Try it: notes

```sh
sdrkit run supersid --source synth:supersid --seconds 30 transmitters="A,24000;B,24800;C,21400;D,23400"
sdrkit run supersid --source synth:supersid --seconds 1500 gen_flare=true transmitters="A,24000;B,24800;C,21400;D,23400"
```

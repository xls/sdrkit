<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Riometer

Cosmic radio noise near 30 MHz with interference rejected, against a quiet-day curve per sidereal time: the absorption of the ionosphere in dB and its events. Needs a fixed radio gain.

Stack id: `riometer`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 30 MHz |
| Tuning range | 20 MHz to 60 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:riometer` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Absorption event threshold | number (dB) | 0.3 | 0.05 to 6 | An event lasts while the absorption is at this or more. |
| `qdc` | Quiet-day curve (saved state) | text | empty |  | The curve as text: qdc1 and 288 values. Empty keeps the curve the stack holds; a bad text is refused. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_burst_share` | Test signal burst share | number | 0.03 | 0 to 0.5 | The share of 20 ms chunks of the test signal that are three times stronger. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `power_db` | Power | number (dBFS) |
| `lst_h` | Local sidereal time | number (h) |
| `curve_db` | Quiet-day curve here | number (dBFS) |
| `absorption_db` | Absorption | number (dB) |
| `curve_bins` | Curve bins filled (of 288) | whole number |
| `curve_ready` | Curve ready | yes or no |
| `interfered_share` | Interfered share | number |
| `event_on` | Absorption event | yes or no |
| `minutes` | Minutes measured | whole number |
| `qdc` | Quiet-day curve | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `absorption` | Absorption per minute | min | dB |
| `power` | Power per minute | min | dBFS |

## Events

| Kind | Label | Fields |
|---|---|---|
| `riometer.absorption` | Absorption event | `text` (text), `phase` (text), `depth_db` (number (dB)), `duration_s` (duration (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Starts the power estimate again and empties the series; the quiet-day curve stays. |
| `export_qdc` | Export the quiet-day curve |  | `qdc` (text) | Replies with the curve as text (qdc1 and 288 values). |
| `import_qdc` | Import a quiet-day curve | `text` (text, required) |  | Takes a curve as text; a bad text is refused and changes nothing. |

## Typed interface

The stack is in the family `science`, version 1.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `RiometerFields`. The result fields of `riometer`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `power_db` | number (dBFS) | optional | 1.0 | The power of the last minute, dB of full scale. |
| `lst_h` | number (h) | optional | 1.0 | The local sidereal time. |
| `curve_db` | number (dBFS) | optional | 1.0 | The curve at that time. |
| `absorption_db` | number (dB) | optional | 1.0 | The absorption. |
| `curve_bins` | whole number | always | 1.0 | Bins of the curve with a value. |
| `curve_ready` | yes or no | always | 1.0 | The curve is ready. |
| `interfered_share` | number | always | 1.0 | The interfered share. |
| `event_on` | yes or no | always | 1.0 | An event is open. |
| `minutes` | whole number | always | 1.0 | Minutes measured. |
| `qdc` | text | optional | 1.0 | The curve as text, as of the last minute measured or the last change of the curve. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Event `riometer.absorption`

Struct `RiometerAbsorptionEvent`. The start (`phase` `start`, `depth_db` the absorption then) or the end (`end`, the largest absorption, the duration) of an absorption event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `phase` | text | always | 1.0 | Start or end of an event. |
| `depth_db` | number (dB) | always | 1.0 | The depth of an event. |
| `duration_s` | duration (s) | always | 1.0 | The time from the start of an event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_riometer_clear` | none | none |
| `reset` | `sdrkit_riometer_reset` | none | none |
| `export_qdc` | `sdrkit_riometer_export_qdc` | none | `sdrkit_riometer_qdc_reply` |
| `import_qdc` | `sdrkit_riometer_import_qdc` | `sdrkit_riometer_qdc_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_riometer_get_threshold_db` | `sdrkit_riometer_set_threshold_db` |
| `qdc` | `sdrkit_riometer_get_qdc` | `sdrkit_riometer_set_qdc` |
| `start_utc` | `sdrkit_riometer_get_start_utc` | `sdrkit_riometer_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_burst_share` | `sdrkit_riometer_gen_get_burst_share` | `sdrkit_riometer_gen_set_burst_share` |

## Try it

```sh
sdrkit run riometer --source synth:riometer
sdrkit run riometer --source <driver>:<address> --freq 30
```
<!-- generated: end -->

## Description

Cosmic radio noise near 30 MHz is steady for a given sidereal time. Extra ionisation of the D region (polar cap absorption, flares, aurora) absorbs part of it. The absorption is the quiet-day level over the measured level, in dB (G. C. Little and H. Leinbach, "The Riometer - A Device for the Continuous Measurement of Ionospheric Absorption", Proc. IRE 47 (2), 1959, pp. 315-320).

The power estimate rejects interference in three steps. Chunks of max(1024, rate × 20 ms) samples: the power spectra of the whole 1024-point segments of a chunk (Hann) are summed, and the median bin, turned into a mean by the median-to-mean ratio of that many spectra, is the chunk value: the mean noise power per sample, which a narrow carrier hardly moves. White noise of power 1 per sample reads 1 at any rate. Seconds follow the sample count: the value of a second is the lower quartile of its chunks, and a second is interfered when its median is over 1.35 times that. A minute's power is the element 20 of its 60 sorted second values (the lower third); a minute is interfered when more than 20 of its seconds were. The interfered share is 0.98 times the last plus 0.02 for an interfered minute.

The quiet-day curve has 288 bins of 5 sidereal minutes. The local sidereal time comes from the UTC of the minute and the longitude of the station position (GMST of `sdrkit-util-geo`, which at J2000.0 is the 18.697374558 h of the USNO formula). Without a position the stack measures power only and says that the position is missing. A minute that is not interfered updates its bin: an empty bin takes the power; a power up to 1.15 times the bin raises it by 0.2 of the difference (the upper envelope of quiet minutes; a jump of more than 0.6 dB is interference, not sky); a lower power pulls the bin down by 0.001 of the difference (gain drift). The curve is ready with 36 bins (one to two days). The absorption is max(-2, 10 log10(curve / power)) dB once ready, else 0. An event starts when the absorption reaches the threshold and ends at the next quiet minute below it. The curve stays through `clear` and `reset`; it is saved and restored as text: `qdc1` and the 288 linear values in `%.4g` form, 0 for an empty bin (the `qdc` setting, the `export_qdc` and `import_qdc` commands, and the `qdc` result field). A bad text is refused and changes nothing.

Not confirmed by a public document: the power estimate (chunks of 20 ms, the median of 1024 bins, the lower quartile of each second, a second interfered when its median is over 1.35 times that, the element 20 of 60 for a minute, a minute interfered when more than 20 of its seconds were), the quiet-day curve (288 bins of 5 sidereal minutes, raised by 0.2 of a quiet minute up to 1.15 times the bin, lowered by 0.001, ready with 36 bins) and the 0.3 dB event threshold. The receiver gain must stay fixed; the stack cannot see an automatic gain change. A bin that is first filled by an unflagged minute of interference stays high for days, because the pull-down is slow.

## Tuning: notes

The usual dial and range are in `data/science/science_dials.csv`. A fixed antenna pointed at the sky is needed.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is complex white noise of amplitude 0.1 per part in chunks of 20 ms; a share of the chunks (3 %) is three times stronger (+9.5 dB), and the level is 0.5 dB lower in the second half of every 10 minutes.

## Result series: notes

The series keep 24 hours of minutes.

## Events: notes

At the start `phase` is `start` and `depth_db` the absorption of that minute; at the end `phase` is `end`, `depth_db` the largest absorption and `duration_s` the time from the start.

## Try it: notes

```sh
sdrkit run riometer --source synth:riometer --seconds 130
sdrkit run riometer --source <driver>:<address> --freq 30
```

A minute of power needs 60 s of signal; the curve needs one to two days.

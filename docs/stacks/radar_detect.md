<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Pulse radar (detect)

Rotating pulse radars in S, C and X band: the rotation period, the pulse repetition frequency (single or dual), the pulse width and the beamwidth from the beam passing; for weather radars, the sweeps and volume restarts.

Stack id: `radar_detect`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 3.05 GHz |
| Tuning range | 2.7 GHz to 9.6 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `marine` | Marine radar | 3.05 GHz |
| `weather` | Weather radar | 5.625 GHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:radar_detect` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_rpm` | Test signal rotation | number (rpm) | 0 | 0 to 120 | Rotations per minute of the beam of the test signal. |
| `gen_prf_hz` | Test signal PRF | frequency | 0 Hz | 0 Hz to 10 kHz | Pulses per second of the test signal; with a staggered train, the higher of the two. |
| `gen_dual_prf` | Test signal staggered PRF | choice | `auto` | `auto` (The submode's), `single` (Single), `dual` (Dual) | Dual: the intervals alternate 1 / PRF and 1 / (0.75 PRF), as 1200 and 900 Hz. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `rotation_s` | Rotation | number (s) |
| `rpm` | Rotations per minute | number (rpm) |
| `prf_hz` | PRF | frequency |
| `prf` | PRF text | text |
| `pulse_width_us` | Pulse width | number (µs) |
| `beamwidth_deg` | Beamwidth | number (°) |
| `crossings` | Crossings | whole number |
| `pulses` | Pulses | whole number |
| `rows_total` | Rows | whole number |
| `noise_dbfs` | Noise | number (dBFS) |
| `sweeps` | Sweeps in the volume | whole number |
| `volumes` | Volumes | whole number |
| `last_volume_s` | Last volume | number (s) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `radar_rows` | Rows | `signal_time_s` (number (s)), `type` (text), `rotation_s` (number (s)), `rpm` (number (rpm)), `prf` (text), `pulse_width_us` (number (µs)), `beamwidth_deg` (number (°)), `peak_db` (number (dB)), `volume` (whole number), `sweep` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `radar.row` | Radar row | `text` (text), `type` (text), `rotation_s` (number (s)), `rpm` (number (rpm)), `prf` (text), `pulse_width_us` (number (µs)), `beamwidth_deg` (number (°)), `peak_db` (number (dB)), `volume` (whole number), `sweep` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `science`, version 1.1. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

### Result fields

Struct `RadarDetectFields`. The result fields of `radar_detect`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `rotation_s` | number (s) | optional | 1.0 | The rotation period. |
| `rpm` | number (rpm) | optional | 1.0 | Rotations per minute. |
| `prf_hz` | frequency | optional | 1.0 | The repetition frequency of the fullest bin. |
| `prf` | text | optional | 1.0 | The repetition text. |
| `pulse_width_us` | number (µs) | optional | 1.0 | The median pulse width. |
| `beamwidth_deg` | number (°) | optional | 1.0 | The beamwidth. |
| `crossings` | whole number | always | 1.0 | Crossings found. |
| `pulses` | whole number | always | 1.0 | Pulses found. |
| `rows_total` | whole number | always | 1.0 | Rows written. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | The noise power. |
| `sweeps` | whole number | optional | 1.0 | Weather: sweeps in this volume. |
| `volumes` | whole number | optional | 1.0 | Weather: volume starts seen. |
| `last_volume_s` | number (s) | optional | 1.0 | Weather: the length of the last whole volume. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `radar_rows`

Struct `RadarDetectRow`. A row of `radar_detect`: a rotation (marine) or a sweep (weather). The row key is the sequence number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time of a row. |
| `type` | text | always | 1.0 | The kind of a row: `rotation` or `sweep`. |
| `rotation_s` | number (s) | optional | 1.0 | The rotation period. |
| `rpm` | number (rpm) | optional | 1.0 | Rotations per minute. |
| `prf` | text | always | 1.0 | The repetition text. |
| `pulse_width_us` | number (µs) | optional | 1.0 | The median pulse width. |
| `beamwidth_deg` | number (°) | optional | 1.0 | The beamwidth. |
| `peak_db` | number (dB) | always | 1.0 | The peak of the crossing of a row. |
| `volume` | whole number | optional | 1.0 | The volume of a weather row. |
| `sweep` | whole number | optional | 1.0 | The sweep of a weather row in its volume. |

### Event `radar.row`

Struct `RadarDetectRowEvent`. One row of `radar_detect`. `text` holds a summary.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the row. |
| `type` | text | always | 1.0 | The kind of a row: `rotation` or `sweep`. |
| `rotation_s` | number (s) | optional | 1.0 | The rotation period. |
| `rpm` | number (rpm) | optional | 1.0 | Rotations per minute. |
| `prf` | text | always | 1.0 | The repetition text. |
| `pulse_width_us` | number (µs) | optional | 1.0 | The median pulse width. |
| `beamwidth_deg` | number (°) | optional | 1.0 | The beamwidth. |
| `peak_db` | number (dB) | always | 1.0 | The peak of the crossing of a row. |
| `volume` | whole number | optional | 1.0 | The volume of a weather row. |
| `sweep` | whole number | optional | 1.0 | The sweep of a weather row in its volume. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_radar_detect_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_rpm` | `sdrkit_radar_detect_gen_get_rpm` | `sdrkit_radar_detect_gen_set_rpm` |
| `gen_prf_hz` | `sdrkit_radar_detect_gen_get_prf_hz` | `sdrkit_radar_detect_gen_set_prf_hz` |
| `gen_dual_prf` | `sdrkit_radar_detect_gen_get_dual_prf` | `sdrkit_radar_detect_gen_set_dual_prf` |

## Try it

```sh
sdrkit run radar_detect --source synth:radar_detect
sdrkit run radar_detect --source synth:radar_detect --submode marine
sdrkit run radar_detect --source iiod:192.168.2.1 --freq 3050
```
<!-- generated: end -->

## Description

The receiver works on every sample at the radio rate. The noise is the mean of |x|² over the first 2000 samples, then an exponential mean with a time constant of 0.2 s over the samples below 4 times the noise and outside pulses. A pulse starts at the first sample above 20 times the noise (13 dB) and ends at the first sample below 5 times the noise, or after 200 µs. The start-to-start intervals from 50 µs to 20 ms are kept (the last 400), and the pulse lengths (the last 200). A histogram of the intervals in 5 µs bins gives the repetition frequency (PRF): 1 / the mean of the intervals in the fullest bin and its two neighbours. When the fullest bin more than 3 bins (15 µs) away holds more than a third of the first, the train is staggered and the text is `dual f_hi / f_lo Hz, ratio r`. The pulse width is the median length.

The peak pulse level over the noise goes into buckets of 2 ms. A beam crossing opens at a bucket within 6 dB of the largest level of the last 3500 buckets (7 s), when that level is 13 dB or more, and closes after more than 6 buckets below. Its time is the power-weighted mean time of its buckets, its width runs from its first to its last bucket (plus 2 ms), and its peak is its largest level. With 3 crossings or more, the rotation period is the median m of the intervals between the last 80 crossings when m is 0.5 to 60 s and at least half the intervals lie within 20 % of m. The beamwidth is the width of the last crossing × 360° / m: the width at 6 dB under the peak, so it is wider than the 3 dB beamwidth. The pulses, the interval histogram and the crossings are the shared pulse-train analysis of `sdrkit-algo-dsp` (`pulsetrain`), which takes every number as a parameter.

`marine` writes a row when the rotation is first found, when it changes by more than 5 %, or 30 s after the last row. `weather` writes a row for every crossing (a sweep). A crossing more than 2.5 dB stronger than the one before starts a new volume, because the scan restarts at the lowest, strongest elevation. `volumes` counts these restarts; the first volume seen does not count. A weaker sweep is seen only when no stronger sweep lies in the last 7 s.

One antenna gives the rotation and the beam crossing, not the bearing: a bearing needs two coherent channels (direction finding). FMCW radars send no pulses and are not seen.

Not confirmed by a public document: every rule of the detector (the first noise over 2000 samples and its 0.2 s mean below 4 times the noise; pulses from 13 dB over the noise to 5 times it or 200 µs; intervals of 50 µs to 20 ms in 5 µs bins with a second frequency above a third of the fullest bin and more than 3 bins away; 2 ms buckets, crossings within 6 dB of the largest level of 7 s and at least 13 dB, closed after 6 buckets; the rotation from 3 crossings with half the intervals within 20 % of the median, 0.5 to 60 s; marine rows at a 5 % change or every 30 s; a new weather volume at 2.5 dB). ITU-R M.1313 (maritime radionavigation radars) and ITU-R M.1849 (ground-based meteorological radars) give typical values of these radars and the band limits; neither was read.

## Tuning: notes

An antenna for S band (2.7 to 3.1 GHz), C band (5.6 GHz) or X band (9.3 to 9.5 GHz). The default frequency is only a start for the descriptor; the usual dials of each submode are in `data/science/science_dials.csv`, for the host to offer.

## Submodes: notes

Choose one with `--submode <id>`. Without one, the stack is `marine`.

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is complex noise of power 1 and pulses at the repetition interval, each with a random phase and the gain of a rotating beam, -12 (Δaz / beamwidth)² dB, clamped to the side-lobe level. The beam points at the receiver a quarter of a rotation after the start and once a rotation after. Marine: 24 rpm, a 2.0° beam, 0.8 µs pulses, 30 dB peak, side lobes at -28 dB, 1000 Hz. Weather: 10 rpm (6 s), a 1.0° beam, 2.0 µs pulses, a volume of four sweeps at 38, 34, 30 and 26 dB, side lobes at -40 dB, intervals alternating 1/1200 and 1/900 s. A sample holds the part of a pulse that falls in its interval. The whole signal is scaled so that the strongest peak has an amplitude of 3.2 and stated at a power of 1, so the 8-bit model of the `synth` source (full scale 4.0) does not clip it: the noise has a standard deviation of 0.1 (marine) or 0.04 (weather).

## Result fields: notes

`sweeps`, `volumes` and `last_volume_s` are given by `weather` only.

## Result tables: notes

A row is a rotation (`marine`) or a sweep (`weather`) at the time of its crossing; `peak_db` is the peak of the crossing over the noise. `volume` (1 for the first volume seen) and `sweep` (from 1 in its volume) are given by `weather` only.

## Commands: notes

`clear` empties the rows, the intervals, the crossings and the counters; the noise level is kept.

## Try it: notes

```sh
sdrkit run radar_detect --source synth:radar_detect/marine --submode marine --seconds 16
sdrkit run radar_detect --source synth:radar_detect/weather --submode weather --seconds 26
sdrkit run radar_detect --source synth:radar_detect/marine --submode marine gen_dual_prf=dual gen_rpm=48
sdrkit run radar_detect --source <driver>:<address> --submode marine --freq 3050
```

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Satellite carrier survey

Carriers, channels and frame structures of satellite signals without a public specification: measured, never decoded; carriers named by their Doppler curve when element sets and the station position are given. Receive only.

Stack id: `sat_survey`. Group: Satellite.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 437.5 MHz |
| Tuning range | 30 MHz to 12.7 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 900 kHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `zombie` | Satellites still transmitting | 145.97 MHz |
| `doppler_id` | Carriers named by Doppler | 437.5 MHz |
| `uhf_satcom` | UHF military satellite channels | 255 MHz |
| `gmr1` | GMR-1 channels | 1.535 GHz |
| `starlink` | Starlink frames | 11.325 GHz |
| `oneweb` | OneWeb sequence | 11.075 GHz |
| `tdrs` | TDRS forward link | 2.1064 GHz |
| `tiangong` | Tiangong carriers | 2.224075 GHz |
| `tsikada` | Tsikada navigation carriers | 150 MHz |
| `gonets` | Gonets messaging bursts | 388.5 MHz |
| `tiantong` | Tiantong S-band channels | 2.185 GHz |
| `iss` | Space station downlinks | 145.8 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sat_survey` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Threshold | number (dB) | 0 | 0 to 30 | The detection threshold: 3 to 30 dB, 0 for the submode's own. |
| `channels` | Channel table | text | empty |  | Channels as text, one a line (or separated by ;): frequency_hz,name[,norad]. Lines of the data file (submode,frequency_hz,name,norad) are taken for this submode only. A measured frequency takes the nearest name within max(3.5 kHz, 30 ppm). |
| `satellites` | Satellite elements | text | empty |  | Two-line element sets (each with or without a name line, checksums checked). With the station position they name the finished carrier lines of the submodes with a Doppler match. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_signal` | Test signal | yes or no | true |  | The test signal holds the signal of the submode's method; off, noise only. |
| `gen_level_db` | Test signal S/N | number (dB) | 0 | -30 to 60 | Carrier and channel methods: S/N in 1 kHz (0 gives 18 and 30 dB). Frame methods: S/N over the whole rate (0 gives 0 dB). |
| `gen_carrier_hz` | Test signal carrier offset | frequency | 20 kHz | -1 MHz to 1 MHz | Where the carrier of the carrier method sits, from the centre, before its slow Doppler curve. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `method` | Method | text |
| `threshold_in_force_db` | Threshold in force | number (dB) |
| `bin_hz` | Bin | number (Hz) |
| `catalogue` | Element sets | whole number |
| `lines` | Carrier lines | whole number |
| `channels_active` | Channels active | whole number |
| `channels_seen` | Channels seen | whole number |
| `frame_rate_hz` | Frame rate searched | number (Hz) |
| `measured_rate_hz` | Frame rate measured | number (Hz) |
| `frame_level_db` | Frame level | number (dB) |
| `detected` | Detected | yes or no |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carrier lines | `line` (whole number), `offset_hz` (frequency), `mhz` (number (MHz)), `snr_db` (number (dB)), `duration_s` (duration (s)), `drift_hz_s` (number (Hz/s)), `active` (yes or no), `channel` (text), `satellite` (text), `norad` (whole number), `rms_hz` (number (Hz)), `ambiguous` (yes or no) |
| `channels` | Channels | `offset_hz` (frequency), `frequency_hz` (frequency), `width_hz` (number (Hz)), `snr_db` (number (dB)), `duty` (number), `bursts` (whole number), `last_seen_s` (number (s)), `active` (yes or no), `name` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Spectrum over the background | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `survey.carrier` | Carrier line | `text` (text), `line` (whole number), `offset_hz` (frequency), `mhz` (number (MHz)), `snr_db` (number (dB)), `duration_s` (duration (s)), `drift_hz_s` (number (Hz/s)), `channel` (text), `satellite` (text), `norad` (whole number), `rms_hz` (number (Hz)), `ambiguous` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.0. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SatSurveyFields`. The result fields of the satellite carrier survey.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `method` | text | always | 1.0 | The method of the submode. |
| `threshold_in_force_db` | number (dB) | always | 1.0 | The threshold in force. |
| `bin_hz` | number (Hz) | optional | 1.0 | The bin of the transform. |
| `catalogue` | whole number | always | 1.0 | Element sets read. |
| `lines` | whole number | optional | 1.0 | Carrier lines shown. |
| `channels_active` | whole number | optional | 1.0 | Channels active now. |
| `channels_seen` | whole number | optional | 1.0 | Channels that have been active. |
| `frame_rate_hz` | number (Hz) | optional | 1.0 | The frame rate searched. |
| `measured_rate_hz` | number (Hz) | optional | 1.0 | The frame rate that fitted best. |
| `frame_level_db` | number (dB) | optional | 1.0 | The level of the frame fit. |
| `detected` | yes or no | optional | 1.0 | The frame structure is detected. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `carriers`

Struct `SatSurveyCarrierRow`. A carrier line. The row key is the number of the line.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `line` | whole number | always | 1.0 | The number of a carrier line. |
| `offset_hz` | frequency | always | 1.0 | Offset from the centre of the samples. |
| `mhz` | number (MHz) | always | 1.0 | The frequency, MHz. |
| `snr_db` | number (dB) | always | 1.0 | The level over the background or floor. |
| `duration_s` | duration (s) | always | 1.0 | The length of a line. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The drift of a line. |
| `active` | yes or no | always | 1.0 | The line or channel is active. |
| `channel` | text | always | 1.0 | The channel name of a line. |
| `satellite` | text | always | 1.0 | The satellite that the Doppler curve names. |
| `norad` | whole number | optional | 1.0 | Its catalogue number. |
| `rms_hz` | number (Hz) | optional | 1.0 | The rms of the Doppler match. |
| `ambiguous` | yes or no | always | 1.0 | Another satellite fits about as well. |

### Table `channels`

Struct `SatSurveyChannelRow`. A channel that has been active. The row key is its grid frequency in whole Hz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | always | 1.0 | Offset from the centre of the samples. |
| `frequency_hz` | frequency | always | 1.0 | The frequency of a channel. |
| `width_hz` | number (Hz) | always | 1.0 | The width of a channel. |
| `snr_db` | number (dB) | always | 1.0 | The level over the background or floor. |
| `duty` | number | always | 1.0 | The share of time a channel is on. |
| `bursts` | whole number | always | 1.0 | Times a channel turned active. |
| `last_seen_s` | number (s) | always | 1.0 | The signal time a channel was last on. |
| `active` | yes or no | always | 1.0 | The line or channel is active. |
| `name` | text | always | 1.0 | The name of a channel. |

### Event `survey.carrier`

Struct `SatSurveyCarrierEvent`. A finished carrier line.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `line` | whole number | always | 1.0 | The number of a carrier line. |
| `offset_hz` | frequency | always | 1.0 | Offset from the centre of the samples. |
| `mhz` | number (MHz) | always | 1.0 | The frequency, MHz. |
| `snr_db` | number (dB) | always | 1.0 | The level over the background or floor. |
| `duration_s` | duration (s) | always | 1.0 | The length of a line. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The drift of a line. |
| `channel` | text | always | 1.0 | The channel name of a line. |
| `satellite` | text | always | 1.0 | The satellite that the Doppler curve names. |
| `norad` | whole number | optional | 1.0 | Its catalogue number. |
| `rms_hz` | number (Hz) | optional | 1.0 | The rms of the Doppler match. |
| `ambiguous` | yes or no | always | 1.0 | Another satellite fits about as well. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sat_survey_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_sat_survey_get_threshold_db` | `sdrkit_sat_survey_set_threshold_db` |
| `channels` | `sdrkit_sat_survey_get_channels` | `sdrkit_sat_survey_set_channels` |
| `satellites` | `sdrkit_sat_survey_get_satellites` | `sdrkit_sat_survey_set_satellites` |
| `start_utc` | `sdrkit_sat_survey_get_start_utc` | `sdrkit_sat_survey_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_signal` | `sdrkit_sat_survey_gen_get_signal` | `sdrkit_sat_survey_gen_set_signal` |
| `gen_level_db` | `sdrkit_sat_survey_gen_get_level_db` | `sdrkit_sat_survey_gen_set_level_db` |
| `gen_carrier_hz` | `sdrkit_sat_survey_gen_get_carrier_hz` | `sdrkit_sat_survey_gen_set_carrier_hz` |

## Try it

```sh
sdrkit run sat_survey --source synth:sat_survey
sdrkit run sat_survey --source synth:sat_survey --submode zombie
sdrkit run sat_survey --source iiod:192.168.2.1 --freq 437.5
```
<!-- generated: end -->

## Description

The survey has twelve submodes and four methods. A submode is a method with the parameters of its signal (bin, threshold, frame rate, channel grid). The frequencies and the channel names are data: the host gives the channel table of a submode in the `channels` setting, and the usual dials are in `data/science/science_dials.csv`. The submode never follows the frequency. Input samples that are not finite count as 0, and the results are published every 0.25 s of signal.

**Carrier method** (`zombie`, `doppler_id`, `tdrs`, `tiangong`, `tsikada`, `iss`). The receiver makes Hann transforms of N = 2^l points, l from 8 up to 20 while the rate over N is above the submode's bin, every N / 2 samples, and averages them into frames of about 0.25 s (at 1 Msps with 8 Hz bins: N = 131 072, bins of 7.6 Hz, 4 spectra a frame). The background is the median of each of clamp(N / 64, 4, 64) equal parts of the band. A peak is a local maximum over max(threshold, the level that noise reaches about once in 100 frames + 1.6 dB), N / 40 bins away from the edges, placed by a parabola through three values in dB; the strongest 200 peaks of a frame are kept. The shared multi-line tracker of `sdrkit-algo-dsp` (`linetrack`) follows the carriers: the strongest track takes the nearest free peak to its prediction (a line through its last 12 points when it has 4) within max(3 bins, 0.25 × the fastest drift × the time since its last point + 2 bins). The fastest drift is 0.032 Hz/s per MHz of the dial + 20 bins per second. Every free peak starts a track, at most 64 at a time. A track ends when it is not seen for 5 s and is kept when it lasted 5 s with 8 points; a full track (4000 points) drops every second point. The table shows the active tracks of 2.5 s or more and the finished ones. A line takes the name of the nearest channel of the table within max(3.5 kHz, 30 ppm of the frequency).

**Doppler match** (`zombie`, `doppler_id`, `tiangong`, `tsikada`, `iss`). A finished line of 30 s or more is compared with every satellite of the `satellites` setting, seen from the station position of the host, by the shared match of `sdrkit-util-geo` (`orbit::dopplermatch`). At most 48 points are taken evenly over the line. A satellite is skipped when it is below −2° at one of them, when its orbit is deep-space, or when the best constant offset (the transmit frequency is not known) is over 25 kHz. The satellites are ranked by the rms after the offset. There is no answer when the best rms is over 60 Hz or when the line's own spread is under 3 × max(1 Hz, the best rms). The answer is confident when the second rms is over 3 × max(3 Hz, the best) and ambiguous (satellites flying together) when it is under 1.5 × that. SGP4 is the propagator of Spacetrack Report 3 and Vallado et al. (2006). The UTC of a point is `start_utc` (or the host clock at the start) plus its signal time.

**Channel method** (`uhf_satcom`, `gmr1`, `gonets`, `tiantong`). Hann transforms of N = 2^l points, l from 8 up to 18 while the rate over N is above the submode's bin, without overlap; a decision every max(2, floor(0.02 × rate / N)) spectra, on their mean. The floor is the 40th percentile of the bins. The on level is max(10^(threshold / 10), 1.5 × the noise limit) × the floor. The channels lie on a grid of absolute frequencies (multiples of the grid), and each bin belongs to the nearest one. A channel is hit when its largest bin is over the on level and active after two hits in a row; a burst is counted when it turns active. Its duty moves by the decision time over 20 s up on a hit and down on a miss. A row gives the offset of the largest bin, the frequency, the width (bins over the on level × the bin), the S/N (largest bin over the floor), the duty, the bursts, the last time seen, whether it is active and its name; the active channels come first, then by S/N. Document: ETSI TS 101 376-5-5 clause 4.2 gives the GMR-1 carrier spacing of 31.25 kHz (centre frequencies 1525 MHz + 31.25 kHz × N).

**Frame-gap method** (`starlink`). The power envelope is the mean of |x|² over max(1, floor(rate / 200 kHz)) samples. Segments of 65 536 envelope values, mean removed and Hann windowed, add their power spectra. For clock errors from −60 to +60 ppm in steps of 4, the frame rate 750 Hz × (1 + ppm × 10⁻⁶) and its harmonics 1 to 8 are looked for: the peak is the largest of 3 bins, the noise the median of the bins 10 to 40 away on both sides; harmonics below bin 45 or within 40 bins of half the segment are skipped. The best ratio of the summed peaks to the summed noise is the level, and its rate the measured rate. The frame structure is detected after 4 segments when the level is over the threshold − 3 dB.

**Repeated-sequence method** (`oneweb`). The period is L = round(rate / 1000 Hz). Each sample is multiplied by the conjugates of the samples L + 1, L and L − 1 earlier (a clock error of 50 ppm moves a period of 20 000 samples by one sample). The period is cut into 64 bins; at the end of each period each bin adds |sum of its products| / sum of its powers, for each lag. Every 400 periods, for each lag, z = (the largest bin − the median) / the mean absolute deviation from the median; the best lag gives the level 10 log10(z) and the measured rate. The sequence is detected when the level is over the threshold + 3 dB; the fold then starts again. Komodromos and Humphreys (OneWeb parameter estimation) report a synchronisation sequence repeated every millisecond.

Nothing is decoded. The `iss` submode names the active channel; the band hop between 145.8 and 437.8 MHz and the start of a matching stack (SSTV, APRS, FM) are host behaviour.

Not confirmed by a public document: every detection rule of the survey. Carrier: transforms with bins under the submode's bin, frames of 0.25 s, a background of up to 64 medians, peaks over max(threshold, the level noise reaches once in 100 frames + 1.6 dB), tracks gated by max(3 bins, 0.25 × (0.032 Hz/s per MHz + 20 bins/s) × gap + 2 bins), ended after 5 s unseen, kept from 5 s and 8 points, shown from 2.5 s; the Doppler match of lines of 30 s or more with 48 points, -2° elevation, offsets to 25 kHz, believable to 60 Hz rms, confident over 3 ×, ambiguous under 1.5 × the best. Channel: decisions every 20 ms, the floor at the 40th percentile, 2 hits to turn active. Frame gap: the envelope at about 200 ksps, segments of 65536 values, 8 harmonics over clock errors of ±60 ppm, detected 3 dB under the threshold after 4 segments. Repeated sequence: 64 bins, folds of 400 periods, detected 3 dB over the threshold. The per-submode thresholds and bins. The Starlink frame rate of 750 Hz and the Starlink test signal (32 subcarriers, 1056-sample symbols at 240 Msps, 302 symbols a frame): no document that states them could be read. The channel tables are from public satellite lists and press reports. With Gaussian noise of 60 Hz on a line, about half the passes stay under the 60 Hz limit and are named. Geostationary and other deep-space objects are not named, and GMR-1 control channels are not decoded.

Each finished carrier line is a `survey.carrier` event, with the satellite when the Doppler curve names one.

## Tuning: notes

The submodes ask for these rates: 1 Msps (`zombie`, `doppler_id`, `tiangong`, `tsikada`, `iss`), 2 Msps (`tdrs`), 4 Msps (`gonets`), 10 Msps (`uhf_satcom`, `gmr1`, `tiantong`) and 20 Msps (`starlink`, `oneweb`); give them with `--rate`. Each runs from 250 ksps, with a coarser bin or a weaker envelope. The frame-gap method needs its asked rate: at 1 Msps the envelope averages only 5 samples and the level of the test signal stays near 5.5 dB, at the edge of detection, so the engine's checks leave `starlink` out and the crate tests run it at 20 Msps. The front end must not remove DC: a carrier may sit at the centre. The Ku-band submodes need an LNB and a dish.

## Submodes: notes

| Submode | Method | Asked rate | Bin (Hz) | Threshold (dB) | Frame (Hz) | Grid (Hz) | Doppler match |
|---|---|---|---|---|---|---|---|
| `zombie` | carrier | 1 M | 8 | 12 | - | - | yes |
| `doppler_id` | carrier | 1 M | 8 | 12 | - | - | yes |
| `uhf_satcom` | channel | 10 M | 1000 | 8 | - | 5000 | no |
| `gmr1` | channel | 10 M | 2000 | 8 | - | 31250 | no |
| `starlink` | frame gap | 20 M | - | 8 | 750 | - | no |
| `oneweb` | repeated sequence | 20 M | - | 8 | 1000 | - | no |
| `tdrs` | carrier | 2 M | 30 | 10 | - | - | no |
| `tiangong` | carrier | 1 M | 10 | 10 | - | - | yes |
| `tsikada` | carrier | 1 M | 8 | 10 | - | - | yes |
| `gonets` | channel | 4 M | 500 | 8 | - | 25000 | no |
| `tiantong` | channel | 10 M | 2000 | 8 | - | 25000 | no |
| `iss` | carrier | 1 M | 10 | 10 | - | - | yes |

The channel tables are in `data/science/sat_survey_channels.csv` (rows `submode,frequency_hz,name,norad`).

## Settings: notes

Advanced settings. The owner works without any of them set.

The test signal is complex noise of standard deviation 0.05 per part with, by method: a carrier at `gen_carrier_hz` + 3000 tanh((300 − t) / 120) Hz; three channels at +3, −7 and +12 grid steps, on for 1.5, 2.0 and 1.0 s every 4, 7 and 3 s, each frequency modulated by ±800 Hz at 600 Hz (a channel beyond 0.45 of the rate is left out); random QPSK on 32 subcarriers in symbols of 1056 samples at 240 Msps rescaled to the rate, 302 symbols a frame and then a gap, 750 frames a second; or random QPSK whose first 40 µs of every millisecond is the same sequence. The signal sits at the centre of the samples, and its stated power is the mean power of the output. The Starlink numbers of the test signal are not confirmed by a document that could be read; the 40 µs length of the OneWeb sequence is a test value.

## Result fields: notes

`threshold_in_force_db` is the peak threshold of the carrier method (the noise limit when it is higher), the on level of the channel method, or the threshold of the frame methods.

## Result tables: notes

The `carriers` table keeps 64 rows and the `channels` table 128. The S/N of a line is its level over the background in one bin; the drift is fitted over its last 16 points.

## Result series: notes

The spectrum is the last carrier frame in dB over its background, 256 values (the largest of each part), from the lowest frequency.

## Try it: notes

```sh
sdrkit run sat_survey --submode tsikada --source synth:sat_survey/tsikada --seconds 12
sdrkit run sat_survey --submode uhf_satcom --source synth:sat_survey/uhf_satcom --rate 10M --seconds 4
sdrkit run sat_survey --submode starlink --source synth:sat_survey/starlink --rate 20M --seconds 3
sdrkit run sat_survey --submode oneweb --source synth:sat_survey/oneweb --rate 20M --seconds 2
sdrkit run sat_survey --submode doppler_id --source <driver>:<address> --freq 437.5 satellites=<elements>
```

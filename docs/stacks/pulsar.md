<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Pulsar folding

A filter bank, incoherent dedispersion over trial DMs and folding over trial periods round your ephemeris: the S/N, the period and DM found, the pulse profile and the S/N curves. Needs a dish or long Yagis and a low-noise amplifier.

Stack id: `pulsar`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 408 MHz |
| Tuning range | 100 MHz to 6 GHz |
| Sample rate | 2.4 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 2.4 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:pulsar` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `period_s` | Pulsar period | number (s) | 0.5 | 0.0015 to 10 | The period of the pulsar from your ephemeris. 0.5 s is a test value; the data file data/science/pulsars.csv lists real ones. |
| `dm` | Dispersion measure | number (pc cm⁻³) | 20 | 0 to 1000 | The dispersion measure of the pulsar. 20 is a test value. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Filter-bank channels | whole number | 64 | 8 to 1024 | A power of two from 8 to 1024. |
| `sample_ms` | Power sample time | number (ms) | 0.5 | 0.1 to 100 | The time of a row asked; a row is a whole number of spectra, so the time used differs a little. |
| `bins` | Profile bins | whole number | 64 | 16 to 256 |  |
| `dm_span` | Trial DM span (±) | number (pc cm⁻³) | 4 | 0 to 100 |  |
| `dm_steps` | Trial DMs | whole number | 9 | 1 to 101 |  |
| `period_span` | Trial period span (± relative) | number | 0.0002 | 0 to 0.001 | 2e-4 covers the Earth's orbital Doppler (up to 1e-4 of the period) for a period known to 1e-5. |
| `period_steps` | Trial periods | whole number | 21 | 1 to 201 |  |
| `flux_jy` | Flux of the pulsar | number (Jy) | 1 | 0.01 to 10000 | For the area estimate only. |
| `tsys_k` | System temperature | number (K) | 150 | 10 to 100000 | For the area estimate only. |
| `hours` | Observing time | number (h) | 1 | 0.05 to 10000 | For the area estimate only. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_snr` | Test signal S/N | number | 0.1 | 0 to 10 | The power of the tones at the top of a pulse over the power of the noise in the whole band. |
| `gen_period_s` | Test signal period | number (s) | 0.5 | 0.0015 to 10 | The period of the pulses of the test signal. 0.5 s is a test value, not a known pulsar. |
| `gen_dm` | Test signal DM | number (pc cm⁻³) | 20 | 0 to 1000 | The dispersion measure of the pulses of the test signal, for a radio centred at 408 MHz. 20 is a test value. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `ok` | Detected | yes or no |
| `snr` | S/N | number |
| `period_found_s` | Period found | number (s) |
| `dm_found` | DM found | number (pc cm⁻³) |
| `peak_phase` | Peak phase | number |
| `duty` | Duty cycle | number |
| `observed_s` | Time observed | duration (s) |
| `searching` | Searching | yes or no |
| `searches` | Searches | whole number |
| `detections` | Detections | whole number |
| `rows_held` | Rows held | whole number |
| `row_ms` | Row time | number (ms) |
| `bank_channels` | Channels | whole number |
| `area_m2` | Effective area needed | number (m²) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `searches` | Searches | `observed_s` (duration (s)), `ok` (yes or no), `snr` (number), `period_found_s` (number (s)), `dm_found` (number (pc cm⁻³)), `peak_phase` (number), `duty` (number), `time_s` (number (s)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `profile` | Profile | phase | σ |
| `dm_curve` | S/N over DM | pc cm⁻³ |  |
| `period_curve` | S/N over period | s |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `pulsar.detection` | Pulsar detection | `text` (text), `ok` (yes or no), `snr` (number), `period_found_s` (number (s)), `dm_found` (number (pc cm⁻³)), `peak_phase` (number), `duty` (number), `observed_s` (duration (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `search_now` | Search now |  |  | Runs a search on the rows held now. |

## Typed interface

The stack is in the family `science`, version 2.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

Key readings: `period_found_s`, `dm_found`, `snr`, `ok`, `detections`.

### Result fields

Struct `PulsarFields`. The result fields of `pulsar`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the stack. |
| `ok` | yes or no | always | 1.0 | Whether a detection was made. |
| `snr` | number | optional | 1.0 | The best S/N. |
| `period_found_s` | number (s) | optional | 1.0 | The best period. |
| `dm_found` | number (pc cm⁻³) | optional | 1.0 | The best DM. |
| `peak_phase` | number | optional | 1.0 | The pulse phase. |
| `duty` | number | optional | 1.0 | The duty cycle. |
| `observed_s` | duration (s) | always | 1.0 | The time observed. |
| `searching` | yes or no | always | 1.0 | A search runs. |
| `searches` | whole number | always | 1.0 | Searches made. |
| `detections` | whole number | always | 1.0 | Detections made. |
| `rows_held` | whole number | always | 1.0 | Rows held. |
| `row_ms` | number (ms) | optional | 1.0 | The time of a row. |
| `bank_channels` | whole number | optional | 1.0 | Channels of the filter bank. |
| `area_m2` | number (m²) | optional | 1.0 | The area needed. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `searches`

Struct `PulsarSearchRow`. A search of `pulsar`. The row key is the sequence number of the search.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `observed_s` | duration (s) | always | 1.0 | The time observed. |
| `ok` | yes or no | always | 1.0 | Whether a detection was made. |
| `snr` | number | optional | 1.0 | The best S/N. |
| `period_found_s` | number (s) | always | 1.0 | The best period. |
| `dm_found` | number (pc cm⁻³) | always | 1.0 | The best DM. |
| `peak_phase` | number | always | 1.0 | The pulse phase. |
| `duty` | number | always | 1.0 | The duty cycle. |
| `time_s` | number (s) | always | 1.0 | The signal time at the end of the rows searched. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `pulsar.detection`

Struct `PulsarDetectionEvent`. A search that detected the pulsar.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message: the summary of the detection. |
| `ok` | yes or no | always | 1.0 | Whether a detection was made. |
| `snr` | number | always | 1.0 | The best S/N. |
| `period_found_s` | number (s) | always | 1.0 | The best period. |
| `dm_found` | number (pc cm⁻³) | always | 1.0 | The best DM. |
| `peak_phase` | number | always | 1.0 | The pulse phase. |
| `duty` | number | always | 1.0 | The duty cycle. |
| `observed_s` | duration (s) | always | 1.0 | The time observed. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_pulsar_clear` | none | none |
| `search_now` | `sdrkit_pulsar_search_now` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `period_s` | `sdrkit_pulsar_get_period_s` | `sdrkit_pulsar_set_period_s` |
| `dm` | `sdrkit_pulsar_get_dm` | `sdrkit_pulsar_set_dm` |
| `channels` | `sdrkit_pulsar_get_channels` | `sdrkit_pulsar_set_channels` |
| `sample_ms` | `sdrkit_pulsar_get_sample_ms` | `sdrkit_pulsar_set_sample_ms` |
| `bins` | `sdrkit_pulsar_get_bins` | `sdrkit_pulsar_set_bins` |
| `dm_span` | `sdrkit_pulsar_get_dm_span` | `sdrkit_pulsar_set_dm_span` |
| `dm_steps` | `sdrkit_pulsar_get_dm_steps` | `sdrkit_pulsar_set_dm_steps` |
| `period_span` | `sdrkit_pulsar_get_period_span` | `sdrkit_pulsar_set_period_span` |
| `period_steps` | `sdrkit_pulsar_get_period_steps` | `sdrkit_pulsar_set_period_steps` |
| `flux_jy` | `sdrkit_pulsar_get_flux_jy` | `sdrkit_pulsar_set_flux_jy` |
| `tsys_k` | `sdrkit_pulsar_get_tsys_k` | `sdrkit_pulsar_set_tsys_k` |
| `hours` | `sdrkit_pulsar_get_hours` | `sdrkit_pulsar_set_hours` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_snr` | `sdrkit_pulsar_gen_get_snr` | `sdrkit_pulsar_gen_set_snr` |
| `gen_period_s` | `sdrkit_pulsar_gen_get_period_s` | `sdrkit_pulsar_gen_set_period_s` |
| `gen_dm` | `sdrkit_pulsar_gen_get_dm` | `sdrkit_pulsar_gen_set_dm` |

## Try it

```sh
sdrkit run pulsar --source synth:pulsar
sdrkit run pulsar --source iiod:192.168.2.1 --freq 408
```
<!-- generated: end -->

## Description

The receiver needs about 1 m² of effective area and a low-noise amplifier at the feed: a dish of 1.5 to 2 m or several long Yagis. A whip or a discone shows no pulsar. The observing frequency is the centre of the radio and the bandwidth its sample rate. The period and the DM of the pulsar are settings: the file `data/science/pulsars.csv` lists bright pulsars with the periods and DMs of the ATNF Pulsar Catalogue, and the period of your own ephemeris is better, since a period changes with time.

The filter bank makes Hann-windowed transforms of C samples without overlap (C the `channels` setting, a power of two from 8 to 1024). Channel 0 is the lowest frequency; channel c is centred at centre − rate / 2 + c × rate / C. A row is the mean of R = max(1, round(sample time × rate / C)) spectra, so the row time is R × C / rate (0.507 ms at 2.4 Msps, 64 channels and 0.5 ms). The receiver keeps up to 16 M values of rows (64 MB) and drops the oldest beyond that. A new period or DM, a new channel count or sample time, and a retune drop the rows.

A search needs at least max(200, 8 periods) rows. It normalises each channel (v / mean − 1 in groups of 1 s of rows, then divided by the rms of the channel), shifts each channel by its dispersion delay to the top channel for each trial DM, sums the channels, and folds the sum at each trial period into the profile bins (the mean of each bin). The dispersion delay is Δt = 4.148808 × 10³ s × DM × (1 / f_lo² − 1 / f_hi²), DM in pc cm⁻³ and f in MHz (D. R. Lorimer, M. Kramer, "Handbook of Pulsar Astronomy"). The trial DMs are DM − span to DM + span in equal steps (negative ones skipped, and a DM whose largest shift leaves fewer than 100 rows); the trial periods are P × (1 ± span) in equal steps. The S/N of a profile: the lowest 75 % of the bins give the off-pulse mean m and the deviation sd = √(variance / 0.5); for pulse widths of 1, 2, 3, 4, 6, 8, 12 and 16 bins (up to a third of the bins) and every start bin, S/N = Σ(profile − m) / (sd √w). The best trial gives the period, the DM, the phase of the middle of the pulse, the duty cycle and the profile in units of sd, mean removed, with the peak in the middle bin. A best S/N of 7 or more is a detection. The search is the shared dedispersion and folding of `sdrkit-algo-dsp` (`fold`), which takes the dispersion law as a parameter.

The first search runs at 20 s observed, then each at the time observed + max(15 s, 0.3 × the time observed); `search_now` runs one at once, and the end of a recording runs one on the rows not yet searched. With live samples a search runs on a helper thread on a copy of the rows and `searching` is on until it ends; with a recording or the test signal it runs in line, so the results do not depend on timing.

The effective area needed for S/N 5 at a duty cycle of 3 % with two polarisations and the radio rate as bandwidth comes from the radiometer equation G = S/N × T_sys / (S √(n_pol B t) √((1 − d) / d)), A = 2 k G / 10⁻²⁶ m², from the flux, system temperature and observing time settings. It is shown, not used.

Not confirmed by a public document: the filter bank (Hann transforms without overlap, a row the mean of whole spectra), the normalisation in groups of 1 s, the 100 rows kept after the largest shift, the pulse widths of 1 to 16 bins, the off-pulse share of 75 % and its variance share of 0.5, the detection at S/N 7, the search schedule (first at 20 s, then after max(15 s, 0.3 x the time observed)), the default trial spans and steps, and the radiometer equation of the area estimate. The dispersion constant 4.148808e3 s MHz² is from Lorimer and Kramer, Handbook of Pulsar Astronomy. The period span covers the Earth's orbital Doppler only: no acceleration search, no coherent dedispersion.

Over 2.4 MHz at 408 MHz one DM unit moves the lowest channel by only 0.29 ms, so the S/N changes little over the default span of ± 4: the DM found is a rough value. The 400 MHz fluxes of the data file are not checked against a document.

Each search is a row of the `searches` table. A detection is also a `pulsar.detection` event at the signal time of the search. The `profile`, `dm_curve` and `period_curve` series hold the best search; `clear` empties the table and the counters and keeps the rows and the series.

## Tuning: notes

The default dial of 408 MHz and the range are data: the usual dials are in `data/science/science_dials.csv`. The blind IQ correction of the front end is off for it.

## Settings: notes

The test signal is complex noise of power 1 and, in each channel of the filter bank, a tone at the centre of the channel with a random phase and the amplitude √(S/N / C), keyed by raised-cosine pulses of duty 3 % that arrive at n × P + the dispersion delay of the channel. It is dispersed for a radio centred at 408 MHz, since a test signal does not know where the radio is tuned. The whole signal is scaled by 0.2 and stated at a power of 1, so that the 8-bit model of the `synth` source keeps the noise peaks and the noise spans many steps of the converter.

## Try it: notes

```sh
sdrkit run pulsar --source synth:pulsar --seconds 30
sdrkit run pulsar --source synth:pulsar gen_snr=0.05 --seconds 40
sdrkit run pulsar --source <driver>:<address> --freq 408 period_s=0.714520 dm=26.7641
```

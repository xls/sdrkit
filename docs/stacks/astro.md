<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Radio astronomy

The hydrogen line (and OH lines): long integrations, velocities, calibration, line fits and maps of the Milky Way.

Stack id: `astro`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.420406 GHz |
| Tuning range | 10 MHz to 6 GHz |
| Sample rate | 2.5 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 2.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:astro` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `fft` | FFT size | choice | `4096` | `1024`, `2048`, `4096`, `8192`, `16384` |  |
| `integration_s` | Integration | duration (s) | 10 s | 0.01 to 3600 |  |
| `line` | Rest line | choice | `hi` | `hi` (H I 1420.405751 MHz), `oh1612` (OH 1612.231 MHz), `oh1665` (OH 1665.402 MHz), `oh1667` (OH 1667.359 MHz), `oh1720` (OH 1720.530 MHz), `custom` (Custom) |  |
| `rest_hz` | Custom rest frequency | frequency | 1.420406 GHz | 10 MHz to 6 GHz |  |
| `band_shape` | Band shape from | choice | `none` | `none` (The spectrum itself), `off_source` (Off source), `switching` (Frequency switching) |  |
| `fold` | Fold | yes or no | true |  |  |
| `dc_bins` | DC bridge | whole number (bins) | 2 | -1 to 16 |  |
| `baseline_degree` | Baseline degree | whole number | 3 | -1 to 9 |  |
| `window` | Line window | yes or no | true |  |  |
| `window_lo_kms` | Window from | number (km/s) | -150 | -500 to 500 |  |
| `window_hi_kms` | Window to | number (km/s) | 150 | -500 to 500 |  |
| `edge` | Edges left out | number | 0.08 | 0 to 0.3 |  |
| `smooth_bins` | Smoothing | whole number (bins) | 1 | 1 to 64 |  |
| `tsys_k` | System temperature | number (K) | 100 | 1 to 2000 |  |
| `lsr` | LSR | choice | `kinematic` | `kinematic` (Kinematic (IAU)), `dynamical` (Dynamical (Schönrich)) |  |
| `target` | Target | choice | `galactic` | `galactic` (Galactic l/b), `sun` (Sun), `moon` (Moon), `casa` (Cas A), `cyga` (Cyg A), `sgra` (Sgr A*), `radec` (Custom RA/Dec) |  |
| `target_l` | Target l | number (°) | 0 | 0 to 360 |  |
| `target_b` | Target b | number (°) | 0 | -90 to 90 |  |
| `target_ra` | Target RA (J2000) | number (°) | 0 | 0 to 360 |  |
| `target_dec` | Target Dec (J2000) | number (°) | 0 | -90 to 90 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_off_source` | Test signal: off source (no line) | yes or no | false |  |  |
| `gen_line_kms` | Test signal: line velocity | number (km/s) | -42 | -250 to 250 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `finished` | Integrations | whole number |
| `progress_pct` | Progress | number (%) |
| `bin` | Bin width | frequency |
| `bin_km_s` | Bin width | number (km/s) |
| `span` | Span | frequency |
| `span_km_s` | Span | number (km/s) |
| `bins_flagged` | Bins flagged | whole number |
| `peak` | Peak | frequency |
| `peak_km_s` | Peak velocity | number (km/s) |
| `excess_pct` | Excess | number (%) |
| `line_snr` | Peak over rms | number |
| `snr_db` | SNR | number (dB) |
| `target_ra_deg` | Target RA | number (°) |
| `target_dec_deg` | Target Dec | number (°) |
| `target_az_deg` | Target azimuth | number (°) |
| `target_el_deg` | Target elevation | number (°) |
| `lsr_rotation_kms` | Earth rotation | number (km/s) |
| `lsr_orbit_kms` | Earth orbit | number (km/s) |
| `lsr_sun_kms` | Sun | number (km/s) |
| `vel_lsr_kms` | Peak velocity (LSR) | number (km/s) |
| `trx_k` | Receiver temperature | number (K) |
| `gain_per_k` | Gain per kelvin | number |
| `note` | Note | choice |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calibrated` | Calibrated spectrum | `freq` (frequency), `vel_topo_kms` (number (km/s)), `vel_lsr_kms` (number (km/s)), `ta_k` (number (K)), `flagged` (yes or no) |
| `session` | Session | `signal_time_s` (number (s)), `peak_kms` (number (km/s)), `excess_pct` (number (%)), `line_snr` (number) |
| `fit` | Line fit | `centre_kms` (number (km/s)), `amplitude_k` (number (K)), `fwhm_kms` (number (km/s)) |
| `lv_rows` | Map rows | `longitude` (number (°)), `terminal_kms` (number (km/s)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Spectrum | km/s |  |
| `total_power` | Total power | s | dBFS |
| `drift` | Peak excess by time | s |  |

## Result pictures

| Key | Label | X unit | Y unit | Cell unit |
|---|---|---|---|---|
| `lv_map` | Longitude-velocity map | km/s | ° | K |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `capture` | Capture | `what` (choice, required) |  | Keeps the next finished integration as the off-source spectrum, or its power as the hot or the cold load. |
| `flag_rfi` | Flag RFI |  |  |  |
| `fit_lines` | Fit lines |  |  |  |
| `add_map` | Add to the l-v map |  |  |  |
| `clear_map` | Clear the map |  |  |  |
| `restart` | Restart the integration |  |  |  |

## Typed interface

The stack is in the family `science`, version 1.2. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: none.

### Result fields

Struct `AstroFields`. The result fields of `astro`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `finished` | whole number | always | 1.0 | Finished integrations. |
| `progress_pct` | number (%) | always | 1.0 | The progress of the current integration. |
| `bin` | frequency | always | 1.0 | The width of a bin. |
| `bin_km_s` | number (km/s) | always | 1.0 | The width of a bin in velocity. |
| `span` | frequency | always | 1.0 | The span of the spectrum. |
| `span_km_s` | number (km/s) | always | 1.0 | The span in velocity. |
| `bins_flagged` | whole number | always | 1.0 | Bins flagged as RFI in the last spectrum. |
| `peak` | frequency | optional | 1.0 | The frequency of the peak. |
| `peak_km_s` | number (km/s) | optional | 1.0 | The velocity of the peak. |
| `excess_pct` | number (%) | optional | 1.0 | The excess at the peak. |
| `line_snr` | number | optional | 1.0 | The peak over the residual rms. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `target_ra_deg` | number (°) | always | 1.0 | The right ascension of the target now. |
| `target_dec_deg` | number (°) | always | 1.0 | The declination of the target now. |
| `target_az_deg` | number (°) | optional | 1.0 | The azimuth of the target, with a station position. |
| `target_el_deg` | number (°) | optional | 1.0 | The elevation of the target, with a station position. |
| `lsr_rotation_kms` | number (km/s) | optional | 1.0 | The observer's velocity towards the target from the Earth's rotation. |
| `lsr_orbit_kms` | number (km/s) | optional | 1.0 | The observer's velocity towards the target from the Earth's orbit. |
| `lsr_sun_kms` | number (km/s) | optional | 1.0 | The observer's velocity towards the target from the Sun's motion. |
| `vel_lsr_kms` | number (km/s) | optional | 1.0 | The velocity of the peak towards the target, corrected to the local standard of rest. |
| `trx_k` | number (K) | optional | 1.0 | The receiver temperature from the Y factor. |
| `gain_per_k` | number | optional | 1.0 | The gain per kelvin from the Y factor. |
| `note` | choice | optional | 1.0 | A note on the calibration, the switching or the fit. |

### Table `calibrated`

Struct `AstroCalibratedRow`. A bin of the calibrated spectrum of the last finished integration. The row key is the bin, five digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq` | frequency | always | 1.0 | The frequency of a bin. |
| `vel_topo_kms` | number (km/s) | always | 1.0 | The topocentric velocity of a bin. |
| `vel_lsr_kms` | number (km/s) | optional | 1.0 | The LSR velocity of a bin (none without a station position). |
| `ta_k` | number (K) | always | 1.0 | The antenna temperature of a bin: the processed ratio times the system temperature. |
| `flagged` | yes or no | always | 1.0 | The bin was flagged as RFI. |

Default order: the column `freq`, lowest first.

### Table `session`

Struct `AstroSessionRow`. A finished integration of the session. The row key is the sequence number of the integration.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time at the end of an integration. |
| `peak_kms` | number (km/s) | always | 1.0 | The peak velocity of an integration. |
| `excess_pct` | number (%) | always | 1.0 | The excess at the peak. |
| `line_snr` | number | always | 1.0 | The peak over the residual rms. |

### Table `fit`

Struct `AstroFitRow`. A Gaussian of the last line fit. The row key is the place in the fit.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `centre_kms` | number (km/s) | always | 1.0 | The centre of a fitted Gaussian. |
| `amplitude_k` | number (K) | always | 1.0 | The amplitude of a fitted Gaussian. |
| `fwhm_kms` | number (km/s) | always | 1.0 | The full width at half maximum of a fitted Gaussian. |

### Table `lv_rows`

Struct `AstroMapRow`. A row of the table `lv_rows`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `longitude` | number (°) | always | 1.0 | The galactic longitude of a row of the map. |
| `terminal_kms` | number (km/s) | optional | 1.0 | The terminal velocity at a longitude. |

Default order: the column `longitude`, lowest first.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_astro_clear` | none | none |
| `capture` | `sdrkit_astro_capture` | `sdrkit_astro_capture_args` | none |
| `flag_rfi` | `sdrkit_astro_flag_rfi` | none | none |
| `fit_lines` | `sdrkit_astro_fit_lines` | none | none |
| `add_map` | `sdrkit_astro_add_map` | none | none |
| `clear_map` | `sdrkit_astro_clear_map` | none | none |
| `restart` | `sdrkit_astro_restart` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `fft` | `sdrkit_astro_get_fft` | `sdrkit_astro_set_fft` |
| `integration_s` | `sdrkit_astro_get_integration_s` | `sdrkit_astro_set_integration_s` |
| `line` | `sdrkit_astro_get_line` | `sdrkit_astro_set_line` |
| `rest_hz` | `sdrkit_astro_get_rest_hz` | `sdrkit_astro_set_rest_hz` |
| `band_shape` | `sdrkit_astro_get_band_shape` | `sdrkit_astro_set_band_shape` |
| `fold` | `sdrkit_astro_get_fold` | `sdrkit_astro_set_fold` |
| `dc_bins` | `sdrkit_astro_get_dc_bins` | `sdrkit_astro_set_dc_bins` |
| `baseline_degree` | `sdrkit_astro_get_baseline_degree` | `sdrkit_astro_set_baseline_degree` |
| `window` | `sdrkit_astro_get_window` | `sdrkit_astro_set_window` |
| `window_lo_kms` | `sdrkit_astro_get_window_lo_kms` | `sdrkit_astro_set_window_lo_kms` |
| `window_hi_kms` | `sdrkit_astro_get_window_hi_kms` | `sdrkit_astro_set_window_hi_kms` |
| `edge` | `sdrkit_astro_get_edge` | `sdrkit_astro_set_edge` |
| `smooth_bins` | `sdrkit_astro_get_smooth_bins` | `sdrkit_astro_set_smooth_bins` |
| `tsys_k` | `sdrkit_astro_get_tsys_k` | `sdrkit_astro_set_tsys_k` |
| `lsr` | `sdrkit_astro_get_lsr` | `sdrkit_astro_set_lsr` |
| `target` | `sdrkit_astro_get_target` | `sdrkit_astro_set_target` |
| `target_l` | `sdrkit_astro_get_target_l` | `sdrkit_astro_set_target_l` |
| `target_b` | `sdrkit_astro_get_target_b` | `sdrkit_astro_set_target_b` |
| `target_ra` | `sdrkit_astro_get_target_ra` | `sdrkit_astro_set_target_ra` |
| `target_dec` | `sdrkit_astro_get_target_dec` | `sdrkit_astro_set_target_dec` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_off_source` | `sdrkit_astro_gen_get_off_source` | `sdrkit_astro_gen_set_off_source` |
| `gen_line_kms` | `sdrkit_astro_gen_get_line_kms` | `sdrkit_astro_gen_set_line_kms` |

## Try it

```sh
sdrkit run astro --source synth:astro
sdrkit run astro --source iiod:192.168.2.1 --freq 1420.405751
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

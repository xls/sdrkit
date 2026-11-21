<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Ionospheric sounders

Chirp sounders: finds them, builds their ionograms and scales them (foE, foF2, MUF, path MUF) to tell which bands are open.

Stack id: `iono`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 10 MHz |
| Tuning range | 100 kHz to 40 MHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 18 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:iono` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `locked` | Sweeps start on whole seconds | yes or no | true |  |  |
| `geometry` | Geometry | choice | `auto` | `auto` (Automatic), `vertical` (Vertical), `oblique` (Oblique) |  |
| `db_top` | Colour scale top | number (dB) | 20 | 5 to 40 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre_hz` | Test signal: the radio's centre | frequency | 10 MHz | 100 kHz to 40 MHz |  |
| `gen_noise_only` | Test signal: no sounders | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `rate` | Sample rate | frequency |
| `centre` | Centre | frequency |
| `signal_time_s` | Signal time | number (s) |
| `timing_lock` | Timing lock | yes or no |
| `span` | Span | frequency |
| `sounders_heard` | Sounders heard | whole number |
| `active_sweeps` | Active sweeps | whole number |
| `detector_fft` | Detector FFT | whole number |
| `detector_ms` | Detector FFT | number (ms) |
| `detector_bin` | Detector bin | frequency |
| `ffts` | FFTs | whole number |
| `loose_peaks` | Peaks not yet tied | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `selected` | Sounder shown | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sounders` | Sounders | `id` (whole number), `rate_khz_s` (number (kHz/s)), `period` (duration (s)), `offset_s` (number (s)), `last_start_s` (number (s)), `last_seen_s` (number (s)), `next_s` (number (s)), `snr_db` (number (dB)), `heard_low` (frequency), `heard_high` (frequency), `sweeps` (whole number), `missed` (whole number), `active` (yes or no), `known` (text), `match_level` (whole number), `candidates` (whole number), `geometry` (choice), `foe_mhz` (number (MHz)), `fof1_mhz` (number (MHz)), `fof2_mhz` (number (MHz)), `hpe_km` (number (km)), `hpf_km` (number (km)), `m3000` (number), `muf3000_mhz` (number (MHz)), `path_muf_mhz` (number (MHz)), `distance_km` (number (km)) |
| `timeline` | Timeline | `start_s` (number (s)), `id` (whole number), `rate_khz_s` (number (kHz/s)), `part_low` (frequency), `part_high` (frequency), `snr_db` (number (dB)), `predicted` (yes or no) |
| `bands` | Bands open now | `band` (text), `state` (choice) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `coverage` | Coverage (1 captured, 2 span now, 3 selected sounder) | MHz |  |
| `history_foe` | foE | s | MHz |
| `history_fof1` | foF1 | s | MHz |
| `history_fof2` | foF2 | s | MHz |
| `history_hpe` | h'E | s | km |
| `history_hpf` | h'F | s | km |
| `history_m3000` | M(3000)F2 | s |  |
| `history_muf3000` | MUF(3000)F2 | s | MHz |
| `history_path_muf` | Path MUF | s | MHz |
| `history_distance` | Distance | s | km |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `select` | Show a sounder | `row` (text, required) |  | Shows the ionogram and the history of one sounder. |
| `select_latest` | Show the latest sounder |  |  |  |

## Typed interface

The stack is in the family `science`, version 1.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `IonoFields`. The result fields of `iono`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rate` | frequency | always | 1.0 | The input rate. |
| `centre` | frequency | always | 1.0 | The radio frequency of the centre. |
| `signal_time_s` | number (s) | always | 1.0 | The signal time (UTC with a radio, else from the first sample). |
| `timing_lock` | yes or no | always | 1.0 | Sweeps start on whole seconds. |
| `span` | frequency | always | 1.0 | The span the radio sees. |
| `sounders_heard` | whole number | always | 1.0 | Sounders heard. |
| `active_sweeps` | whole number | always | 1.0 | Sweeps being received. |
| `detector_fft` | whole number | optional | 1.0 | The FFT size of the detector. |
| `detector_ms` | number (ms) | optional | 1.0 | The duration of a detector FFT. |
| `detector_bin` | frequency | optional | 1.0 | The bin width of the detector. |
| `ffts` | whole number | optional | 1.0 | FFTs done. |
| `loose_peaks` | whole number | optional | 1.0 | Chirp peaks not yet tied to a sweep. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `selected` | text | optional | 1.0 | The row key of the sounder whose ionogram and history are shown. |

### Table `sounders`

Struct `IonoSounderRow`. A sounder heard. The row key is the sounder's id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The id of a sounder. |
| `rate_khz_s` | number (kHz/s) | always | 1.0 | A sweep rate. |
| `period` | duration (s) | optional | 1.0 | The repetition period. |
| `offset_s` | number (s) | optional | 1.0 | The start offset in the period. |
| `last_start_s` | number (s) | always | 1.0 | The start of the last sweep. |
| `last_seen_s` | number (s) | always | 1.0 | When the sounder was last heard. |
| `next_s` | number (s) | optional | 1.0 | The start of the next sweep. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `heard_low` | frequency | always | 1.0 | The lowest radio frequency heard. |
| `heard_high` | frequency | always | 1.0 | The highest radio frequency heard. |
| `sweeps` | whole number | always | 1.0 | Sweeps received. |
| `missed` | whole number | always | 1.0 | Expected sweeps not heard. |
| `active` | yes or no | always | 1.0 | A sweep is being received. |
| `known` | text | optional | 1.0 | The name of the known sounder it matches. |
| `match_level` | whole number | always | 1.0 | How well it matches: 0 no match, 1 the rate (and the distance), 2 the rate and the schedule. |
| `candidates` | whole number | always | 1.0 | The known sounders of the same rate. |
| `geometry` | choice | optional | 1.0 | The geometry of the latest scaling. |
| `foe_mhz` | number (MHz) | optional | 1.0 | The critical frequency of the E layer. |
| `fof1_mhz` | number (MHz) | optional | 1.0 | The critical frequency of the F1 layer. |
| `fof2_mhz` | number (MHz) | optional | 1.0 | The critical frequency of the F2 layer. |
| `hpe_km` | number (km) | optional | 1.0 | The virtual height of the E layer. |
| `hpf_km` | number (km) | optional | 1.0 | The virtual height of the F layer. |
| `m3000` | number | optional | 1.0 | The propagation factor M(3000)F2. |
| `muf3000_mhz` | number (MHz) | optional | 1.0 | The MUF over 3000 km. |
| `path_muf_mhz` | number (MHz) | optional | 1.0 | The MUF of the path from an oblique sounder. |
| `distance_km` | number (km) | optional | 1.0 | The distance of an oblique sounder. |

### Table `timeline`

Struct `IonoTimelineRow`. A sweep heard. The row key is the sequence number of the sweep.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start_s` | number (s) | always | 1.0 | The start of a sweep. |
| `id` | whole number | always | 1.0 | The id of a sounder. |
| `rate_khz_s` | number (kHz/s) | always | 1.0 | A sweep rate. |
| `part_low` | frequency | always | 1.0 | The lowest frequency of the sweep that crossed the span. |
| `part_high` | frequency | always | 1.0 | The highest frequency of the sweep that crossed the span. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `predicted` | yes or no | always | 1.0 | The sweep was expected. |

### Table `bands`

Struct `IonoBandRow`. A band and its state, from the selected sounder's usable MUF. The row key is the name of the band.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | text | always | 1.0 | A band. |
| `state` | choice | always | 1.0 | The state of a band. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_iono_clear` | none | none |
| `select` | `sdrkit_iono_select` | `sdrkit_iono_select_args` | none |
| `select_latest` | `sdrkit_iono_select_latest` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `locked` | `sdrkit_iono_get_locked` | `sdrkit_iono_set_locked` |
| `geometry` | `sdrkit_iono_get_geometry` | `sdrkit_iono_set_geometry` |
| `db_top` | `sdrkit_iono_get_db_top` | `sdrkit_iono_set_db_top` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre_hz` | `sdrkit_iono_gen_get_centre_hz` | `sdrkit_iono_gen_set_centre_hz` |
| `gen_noise_only` | `sdrkit_iono_gen_get_noise_only` | `sdrkit_iono_gen_set_noise_only` |

## Try it

```sh
sdrkit run iono --source synth:iono
sdrkit run iono --source <driver>:<address> --freq 10
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

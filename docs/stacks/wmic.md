<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wireless mic finder

Wide FM carriers of wireless microphones and in-ear monitors: the width and the deviation of each one, whether it looks like analogue FM, and the audio of one. A plan file that the host loads names the bands.

Stack id: `wmic`. Group: Listening.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 470 MHz to 960 MHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wmic` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `plan` | Band plan | text | empty |  | The path of a plan file in the host's storage, in a format that plans.import takes (docs/schema/plans.json). Its bands name the band of each candidate. Empty for no plan: the band of a candidate is then empty. |
| `raster_step_hz` | Raster step | frequency | 0 Hz | 0 Hz to 200 kHz | The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes. |
| `raster_origin_hz` | Raster origin | frequency | 0 Hz | 0 Hz to 300 GHz | A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz. |
| `squelch` | Squelch | number (%) | 50 | 0 to 100 | 0 opens at the weakest signal. |
| `carriers` | Carriers at once | whole number | 10 | 1 to 24 |  |
| `listen` | Listen | whole number | 0 | 0 to 1000000000 | The id of the candidate that plays. 0 plays nothing. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_cnr_db` | Test signal C/N | number (dB) | 30 | 0 to 80 | The carrier-to-noise ratio of the test signal in 10 kHz. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `candidates` | Candidates | whole number |
| `plan` | Band plan | text |
| `input_rate_hz` | Input rate | frequency |
| `centre_hz` | Centre | frequency |
| `signal_time_s` | Signal time | duration (s) |
| `reports` | Reports | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `candidates` | Candidates | `id` (whole number), `hz` (frequency), `width_hz` (frequency), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `deviation_hz` (frequency), `fm` (yes or no), `active` (yes or no), `listening` (yes or no), `age_s` (duration (s)), `band` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wmic.candidate` | Candidate | `id` (whole number), `hz` (frequency), `width_hz` (frequency), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `deviation_hz` (frequency), `fm` (yes or no), `band` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `listen` | Listen | `id` (whole number, required) |  | Plays a candidate. The candidate that plays stops. |

## Typed interface

The stack is in the family `voice`, version 1.5. The C header `sdrkit/voice.h`, the registry record `registry/voice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: sound (monitor).

Key readings: `candidates`, `plan`.

### Result fields

Struct `WmicFields`. The result fields of the wireless microphone finder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `candidates` | whole number | always | 1.0 | Candidates now. |
| `plan` | text | optional | 1.0 | The name of the band plan. |
| `input_rate_hz` | frequency | always | 1.0 | The input rate. |
| `centre_hz` | frequency | always | 1.0 | The centre of the capture. |
| `signal_time_s` | duration (s) | always | 1.0 | The signal time. |
| `reports` | whole number | always | 1.0 | Reports made. |

### Table `candidates`

Struct `WmicCandidateRow`. A carrier that can be a wireless microphone.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The carrier id of the finder. |
| `hz` | frequency | always | 1.0 | Where to listen. |
| `width_hz` | frequency | always | 1.0 | The occupied width. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The channel level. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio. |
| `deviation_hz` | frequency | optional | 1.0 | The peak deviation. |
| `fm` | yes or no | always | 1.0 | It looks like analogue FM. |
| `active` | yes or no | always | 1.0 | Above the threshold now. |
| `listening` | yes or no | always | 1.0 | This candidate plays. |
| `age_s` | duration (s) | always | 1.0 | The time since it was first reported. |
| `band` | text | always | 1.0 | The band of the plan that holds it. |

Default order: the column `snr_db`, highest first.

### Event `wmic.candidate`

Struct `WmicCandidateEvent`. A new candidate carrier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The carrier id of the finder. |
| `hz` | frequency | always | 1.0 | Where to listen. |
| `width_hz` | frequency | always | 1.0 | The occupied width. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The channel level. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio. |
| `deviation_hz` | frequency | optional | 1.0 | The peak deviation. |
| `fm` | yes or no | always | 1.0 | It looks like analogue FM. |
| `band` | text | always | 1.0 | The band of the plan that holds it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_wmic_clear` | none | none |
| `listen` | `sdrkit_wmic_listen` | `sdrkit_wmic_listen_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `plan` | `sdrkit_wmic_get_plan` | `sdrkit_wmic_set_plan` |
| `raster_step_hz` | `sdrkit_wmic_get_raster_step_hz` | `sdrkit_wmic_set_raster_step_hz` |
| `raster_origin_hz` | `sdrkit_wmic_get_raster_origin_hz` | `sdrkit_wmic_set_raster_origin_hz` |
| `squelch` | `sdrkit_wmic_get_squelch` | `sdrkit_wmic_set_squelch` |
| `carriers` | `sdrkit_wmic_get_carriers` | `sdrkit_wmic_set_carriers` |
| `listen` | `sdrkit_wmic_get_listen` | `sdrkit_wmic_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_cnr_db` | `sdrkit_wmic_gen_get_cnr_db` | `sdrkit_wmic_gen_set_cnr_db` |

## Try it

```sh
sdrkit run wmic --source synth:wmic
sdrkit run wmic --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## How it works

A carrier finder searches a raster of 100 kHz through the multiples of the step; `raster_step_hz` and `raster_origin_hz` change it. `squelch` sets its threshold: 5 dB above the noise at 0, 13 dB at 100. A carrier is reported after 0.3 s above the threshold with at least 6 dB of signal-to-noise ratio, and stays listed for 8 s after it went quiet. A carrier between two raster frequencies is one candidate.

Each carrier has a candidate receiver: a channel of 160 kHz at 192 kHz and an FM discriminator. It measures in windows of 0.1 s, and only in windows where the carrier is at full strength.

| Column | How it is measured |
|---|---|
| `hz` | The frequency of the channel plus the mean of the discriminator. |
| `width_hz` | The part of the spectrum more than 6 dB above the noise. |
| `deviation_hz` | Half the range of the discriminator in a window after a 15 kHz low-pass, smoothed over windows. No value before the first window at full strength. |
| `fm` | The envelope varies by less than 0.35 of its mean and the deviation is above 3 kHz. The value stays for 2 s after the last such window, through the pauses of speech. |
| `band` | The name of the band of the plan file that holds `hz`. Empty without a plan file and outside every band. |

An unmodulated carrier, an AM carrier and a digital microphone are listed and are not FM. A silent analogue microphone is not FM until it is modulated. The stack does not decode digital systems, does not expand companded audio and does not show a pilot tone.

The picked candidate plays at 48 kHz without de-emphasis. Nothing plays without a pick. The command `listen` with the id of the candidate that plays stops it.

## Band plans

The stack holds no band list and no rule for a country. The host names a plan file in the `plan` setting. The file is in the host's storage and has the format of `docs/schema/plans.json` (the format that `plans.import` takes). The stack reads the name and the edges of each band of the first plan of the file. Candidates outside every band are listed too. Without a plan file the status says "no channel plan loaded".

The folder `crates/stacks/sdrkit-stack-voice/data` has example plan files. The library does not contain them.

| File | Plan | Bands |
|---|---|---|
| `microphones_eu.json` | Wireless microphone bands, Europe | 3 |
| `microphones_us.json` | Wireless microphone bands, USA | 4 |

## Try it: notes

```sh
sdrkit run wmic --source synth:wmic
sdrkit run wmic plan=plans/microphones_eu.json --storage <folder with plans/microphones_eu.json> --source iiod:192.168.2.1 --freq 590
```

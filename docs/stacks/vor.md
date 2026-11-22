<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# VOR

The radial from a VOR or Doppler VOR, the course deviation for a selected course, and the station's identification.

Stack id: `vor`. Group: Navigation.

Signal types: VOR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 113.1 MHz |
| Tuning range | 108 MHz to 118 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 64 kHz |
| Signal bandwidth | 25 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:vor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `obs` | Course (OBS) | number (°) | 0 | 0 to 360 | The selected course: the deviation and the TO/FROM flag are shown for it. |
| `average` | Averaging | duration (s) | 1 s | 0.1 to 30 | How long the bearing is averaged over. |
| `confirm` | Hearings to confirm | whole number | 2 | 1 to 10 | How many times an identification must be heard before it is shown. |
| `carrier_threshold` | Carrier threshold | number (dB-Hz) | 30 | 0 to 90 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_kind` | Test signal: station | choice | `conventional` | `conventional` (Conventional VOR), `doppler` (Doppler VOR) |  |
| `gen_radial` | Test signal: radial | number (°) | 45 | 0 to 360 |  |
| `gen_ident` | Test signal: identification | text | `SYD` |  |  |
| `gen_wpm` | Test signal: Morse speed | number (wpm) | 10 | 3 to 20 |  |
| `gen_pause` | Test signal: pause between idents | duration (s) | 2 s | 0.5 to 30 |  |
| `gen_rotate` | Test signal: radial change | number (°/s) | 0 | -30 to 30 | The radial moves this fast (an aircraft flying round the station). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `radial_deg` | Radial | number (°) |
| `bearing_to_deg` | Bearing to | number (°) |
| `obs_deg` | Course | number (°) |
| `to_from` | To or from | choice |
| `cdi_deg` | Course deviation | number (°) |
| `cdi_dots` | Course deviation | number (dots) |
| `am30_pct` | 30 Hz AM | number (%) |
| `subcarrier_pct` | Subcarrier | number (%) |
| `deviation_hz` | Subcarrier deviation | frequency |
| `steadiness` | Steadiness | number |
| `fixes` | Measurements | whole number |
| `tuned_hz` | Tuned | frequency |
| `dme_channel` | DME channel | text |
| `ident` | Identification | text |
| `last_ident` | Last heard | text |
| `ident_wpm` | Morse speed | number (wpm) |
| `carrier` | Carrier | yes or no |
| `carrier_hz` | Carrier | frequency |
| `cn0_dbhz` | C/N0 | number (dB-Hz) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `id_tone_hz` | Identification tone | frequency |
| `id_depth_pct` | Identification depth | number (%) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `idents` | Identifications heard | `ident` (text), `heard` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `morse.word` | Identification word | `source` (text), `text` (text), `family` (choice), `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `wpm` (number (wpm)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avnav`, version 1.4. The C header `sdrkit/avnav.h`, the registry record `registry/avnav.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Navigation aids (`navigation`). Media: sound (monitor).

### Result fields

Struct `VorFields`. The result fields of VOR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `radial_deg` | number (°) | optional | 1.0 | The radial. No value without a valid bearing. |
| `bearing_to_deg` | number (°) | optional | 1.0 | The bearing to the station: the radial plus 180°. No value without a valid bearing. |
| `obs_deg` | number (°) | always | 1.0 | The selected course. |
| `to_from` | choice | always | 1.0 | TO, FROM, or OFF (the flag: no valid bearing). |
| `cdi_deg` | number (°) | optional | 1.0 | The course deviation, positive to fly right. No value without a valid bearing. |
| `cdi_dots` | number (dots) | optional | 1.0 | The course deviation in dots. No value without a valid bearing. |
| `am30_pct` | number (%) | optional | 1.0 | The depth of the 30 Hz AM. No value before the first measurement. |
| `subcarrier_pct` | number (%) | optional | 1.0 | The depth of the subcarrier. No value before the first measurement. |
| `deviation_hz` | frequency | optional | 1.0 | The peak deviation of the subcarrier. No value before the first measurement. |
| `steadiness` | number | optional | 1.0 | How steady the bearing is, 0 (random) to 1. No value before the first measurement. |
| `fixes` | whole number | always | 1.0 | Measurements made. |
| `tuned_hz` | frequency | always | 1.0 | The tuned frequency. |
| `dme_channel` | text | optional | 1.0 | The DME channel paired with the frequency ("17X"). No value for a frequency without one. |

### Result fields

Struct `IdentFields`. The identification of the station.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ident` | text | optional | 1.0 | The confirmed identification. No value before one is confirmed. |
| `last_ident` | text | optional | 1.0 | The last identification heard. No value before a word is heard. |
| `ident_wpm` | number (wpm) | optional | 1.0 | The Morse speed of the last identification. No value before a word is heard. |

### Result fields

Struct `CarrierFields`. The carrier of the station.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carrier` | yes or no | always | 1.0 | A carrier stands clear of the noise. |
| `carrier_hz` | frequency | optional | 1.0 | The frequency of the carrier. No value before the first measurement. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | The carrier to noise density. No value before the first measurement. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the carrier from the tuned frequency. No value before the first measurement. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the carrier. No value before the first measurement. |

### Result fields

Struct `KeyerFields`. The identification tone that the tone keyer listens to.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id_tone_hz` | frequency | optional | 1.0 | The tone: the setting, or the tone found. No value before the first input. |
| `id_depth_pct` | number (%) | optional | 1.0 | The modulation depth of the tone. No value before the first input. |

### Table `idents`

Struct `IdentRow`. A row of the identifications heard. The row key is the word.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ident` | text | always | 1.0 | The word. |
| `heard` | whole number | always | 1.0 | How often it was heard. |

The column `ident` names a row.

### Event `morse.word`

Struct `IdentWordEvent`. The fields of a decoded identification word: a spot of the station that sends it. The source, the text and the callsign hold the word.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The word. |
| `text` | text | always | 1.0 | The word. |
| `family` | choice | always | 1.0 | The family of the source: other. |
| `callsign` | text | always | 1.0 | The word. |
| `locator` | text | optional | 1.0 | No value: a beacon sends no locator. |
| `snr_db` | number (dB) | optional | 1.0 | No value: the decoder does not measure it. |
| `freq_hz` | frequency | optional | 1.0 | No value: the event carries the frequency. |
| `mode` | text | always | 1.0 | The emission: "CW". |
| `wpm` | number (wpm) | always | 1.0 | The Morse speed. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_vor_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `obs` | `sdrkit_vor_get_obs` | `sdrkit_vor_set_obs` |
| `average` | `sdrkit_vor_get_average` | `sdrkit_vor_set_average` |
| `confirm` | `sdrkit_vor_get_confirm` | `sdrkit_vor_set_confirm` |
| `carrier_threshold` | `sdrkit_vor_get_carrier_threshold` | `sdrkit_vor_set_carrier_threshold` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_kind` | `sdrkit_vor_gen_get_kind` | `sdrkit_vor_gen_set_kind` |
| `gen_radial` | `sdrkit_vor_gen_get_radial` | `sdrkit_vor_gen_set_radial` |
| `gen_ident` | `sdrkit_vor_gen_get_ident` | `sdrkit_vor_gen_set_ident` |
| `gen_wpm` | `sdrkit_vor_gen_get_wpm` | `sdrkit_vor_gen_set_wpm` |
| `gen_pause` | `sdrkit_vor_gen_get_pause` | `sdrkit_vor_gen_set_pause` |
| `gen_rotate` | `sdrkit_vor_gen_get_rotate` | `sdrkit_vor_gen_set_rotate` |

## Try it

```sh
sdrkit run vor --source synth:vor
sdrkit run vor --source iiod:192.168.2.1 --freq 113.1
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

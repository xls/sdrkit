<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ILS

Localizer, glide path and marker beacons: deviation from the course and the path, the flag, the identification, paired frequencies, and the marker passed over.

Stack id: `ils`. Group: Navigation.

Signal types: ILS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 110.3 MHz |
| Tuning range | 74.5 MHz to 336 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 32 kHz |
| Signal bandwidth | 10 kHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `localizer` | Localizer | 110.3 MHz |
| `glideslope` | Glide path | 335 MHz |
| `marker` | Marker beacon | 75 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ils` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `part` | Part | choice | `auto` | `auto` (By frequency), `localizer` (Localizer), `glideslope` (Glide path), `marker` (Marker beacon) | Localizer, glide path or marker beacon. By frequency, 74.5 to 75.5 MHz is a marker beacon and above 300 MHz a glide path. |
| `average` | Averaging | duration (s) | 0.5 s | 0.1 to 10 |  |
| `confirm` | Hearings to confirm | whole number | 2 | 1 to 10 | How many times an identification must be heard before it is shown. |
| `carrier_threshold` | Carrier threshold | number (dB-Hz) | 30 | 0 to 90 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_ddm` | Test signal: DDM | number | 0.05 | -0.4 to 0.4 | Positive: 90 Hz predominates. |
| `gen_ident` | Test signal: identification | text | `ISYD` |  |  |
| `gen_wpm` | Test signal: Morse speed | number (wpm) | 10 | 3 to 20 |  |
| `gen_pause` | Test signal: pause between idents | duration (s) | 2 s | 0.5 to 30 |  |
| `gen_marker` | Test signal: marker | choice | `outer` | `outer` (Outer), `middle` (Middle), `inner` (Inner) | The marker beacon of the part `marker`. |
| `gen_marker_keying` | Test signal: marker keying | choice | `standard` | `standard` (As the standard), `steady` (Steady tone) |  |
| `gen_marker_tone` | Test signal: marker tone | frequency | 0 Hz | 0 Hz to 4 kHz | 0 sends the tone of the marker. |
| `gen_marker_depth` | Test signal: marker depth | number (%) | 95 | 0 to 100 |  |
| `gen_marker_start` | Test signal: marker keyed from | duration (s) | 0 s | 0 to 3600 |  |
| `gen_marker_stop` | Test signal: marker keyed until | duration (s) | 0 s | 0 to 3600 | 0 keys without end. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `part` | Part | choice |
| `ddm` | DDM | number |
| `sdm_pct` | SDM | number (%) |
| `m90_pct` | 90 Hz | number (%) |
| `m150_pct` | 150 Hz | number (%) |
| `needle_ua` | Needle | number (µA) |
| `needle_dots` | Needle | number (dots) |
| `command` | Command | choice |
| `flag` | Flag | yes or no |
| `paired_hz` | Paired | frequency |
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
| `marker` | Marker | choice |
| `marker_tone_hz` | Marker tone | frequency |
| `marker_depth_pct` | Marker depth | number (%) |
| `keying` | Keying | text |
| `keying_rate` | Keying rate | number (/s) |
| `pairs_per_min` | Dot-dash pairs | number (/min) |
| `dot_s` | Dot | number (s) |
| `dash_s` | Dash | number (s) |
| `keying_fits` | Keying fits | yes or no |
| `passage_s` | Passage | number (s) |
| `passages` | Passages | whole number |
| `keyed_now` | Keyed now | yes or no |
| `held_depth_pct` | Held depth | number (%) |
| `pattern` | Pattern heard | choice |
| `id_tone_hz` | Identification tone | frequency |
| `id_depth_pct` | Identification depth | number (%) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `idents` | Identifications heard | `ident` (text), `heard` (whole number) |
| `passages` | Marker passages | `kind` (choice), `marker` (choice), `text` (text), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `morse.word` | Identification word | `source` (text), `text` (text), `family` (choice), `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `wpm` (number (wpm)) |
| `ils.marker` | Marker passage | `source` (text), `text` (text), `family` (choice), `kind` (choice), `marker` (choice), `tone_hz` (frequency), `depth_pct` (number (%)), `duration_s` (number (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avnav`, version 1.1. The C header `sdrkit/avnav.h`, the registry record `registry/avnav.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `IlsFields`. The result fields of ILS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `part` | choice | always | 1.0 | Localizer or glide path. |
| `ddm` | number | optional | 1.0 | The difference in depth of modulation (90 Hz less 150 Hz). No value while the flag is out. |
| `sdm_pct` | number (%) | optional | 1.0 | The sum in depth of modulation. No value before the first measurement. |
| `m90_pct` | number (%) | optional | 1.0 | The depth of the 90 Hz tone. No value before the first measurement. |
| `m150_pct` | number (%) | optional | 1.0 | The depth of the 150 Hz tone. No value before the first measurement. |
| `needle_ua` | number (µA) | optional | 1.0 | The needle, positive to fly right or up, limited to 150 µA either way. No value while the flag is out. |
| `needle_dots` | number (dots) | optional | 1.0 | The needle in dots (75 µA a dot), limited to 2.5 either way. No value while the flag is out. |
| `command` | choice | always | 1.0 | What the needle says. |
| `flag` | yes or no | always | 1.0 | The flag is out: no carrier, an SDM far from its nominal value, or the marker part, which has no needle. |
| `paired_hz` | frequency | optional | 1.0 | The paired localizer or glide path frequency. No value for a frequency without one. |
| `tuned_hz` | frequency | always | 1.0 | The tuned frequency. |
| `dme_channel` | text | optional | 1.0 | The DME channel paired with the localizer ("17X"). No value for a frequency without one. |

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

Struct `MarkerFields`. The marker beacon (the part `marker`): the passage under way, or the last one, and the live indication.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `marker` | choice | always | 1.1 | The marker of the passage, named by its tone. None before the first passage of three elements and outside the marker part. |
| `marker_tone_hz` | frequency | optional | 1.1 | The tone of the passage, measured. No value without a passage. |
| `marker_depth_pct` | number (%) | optional | 1.1 | The modulation depth of the tone. No value without a passage. |
| `keying` | text | optional | 1.1 | The keying as text ("2.0 dashes per second (0.30 s long)"). No value without a passage. |
| `keying_rate` | number (/s) | optional | 1.1 | Elements a second, from the first onset to the last. No value without a passage. |
| `pairs_per_min` | number (/min) | optional | 1.1 | Dot-dash pairs a minute. No value without mixed keying. |
| `dot_s` | number (s) | optional | 1.1 | The mean dot length. No value without dots. |
| `dash_s` | number (s) | optional | 1.1 | The mean dash length. No value without dashes. |
| `keying_fits` | yes or no | optional | 1.1 | The keying fits the standard (ICAO Annex 10 Volume I, 3.1.7.5). No value without a passage. |
| `passage_s` | number (s) | optional | 1.1 | The length of the passage. No value without a passage. |
| `passages` | whole number | always | 1.1 | Passages finished, those of fewer than three elements not counted. |
| `keyed_now` | yes or no | always | 1.1 | The tone with the greatest held depth keys the last period. |
| `held_depth_pct` | number (%) | optional | 1.1 | The held depth of that tone. No value outside the marker part. |
| `pattern` | choice | always | 1.1 | The pattern of the last on-lengths of that tone. |

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

### Table `passages`

Struct `MarkerPassageRow`. A row of the marker passages. The row key is the number of the passage.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.1 | A marker, or a marker tone whose keying does not fit the standard. |
| `marker` | choice | always | 1.1 | The marker of the tone. |
| `text` | text | always | 1.1 | The tone, the keying, the depth and the length ("400 Hz, 2.0 dashes per second (0.30 s long), depth 95 %, 7.8 s"). |
| `level_dbfs` | number (dBFS) | optional | 1.1 | The carrier level at the end of the passage. No value before the first carrier measurement. |

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

### Event `ils.marker`

Struct `MarkerEvent`. The fields of a finished marker passage.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.1 | The marker ("Outer marker"). |
| `text` | text | always | 1.1 | The tone, the keying, the depth and the length. |
| `family` | choice | always | 1.1 | The family of the source: other. |
| `kind` | choice | always | 1.1 | A marker, or a marker tone whose keying does not fit the standard. |
| `marker` | choice | always | 1.1 | The marker of the tone. |
| `tone_hz` | frequency | always | 1.1 | The tone, measured. |
| `depth_pct` | number (%) | always | 1.1 | The modulation depth of the tone. |
| `duration_s` | number (s) | always | 1.1 | The length of the passage. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ils_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `part` | `sdrkit_ils_get_part` | `sdrkit_ils_set_part` |
| `average` | `sdrkit_ils_get_average` | `sdrkit_ils_set_average` |
| `confirm` | `sdrkit_ils_get_confirm` | `sdrkit_ils_set_confirm` |
| `carrier_threshold` | `sdrkit_ils_get_carrier_threshold` | `sdrkit_ils_set_carrier_threshold` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_ddm` | `sdrkit_ils_gen_get_ddm` | `sdrkit_ils_gen_set_ddm` |
| `gen_ident` | `sdrkit_ils_gen_get_ident` | `sdrkit_ils_gen_set_ident` |
| `gen_wpm` | `sdrkit_ils_gen_get_wpm` | `sdrkit_ils_gen_set_wpm` |
| `gen_pause` | `sdrkit_ils_gen_get_pause` | `sdrkit_ils_gen_set_pause` |
| `gen_marker` | `sdrkit_ils_gen_get_marker` | `sdrkit_ils_gen_set_marker` |
| `gen_marker_keying` | `sdrkit_ils_gen_get_marker_keying` | `sdrkit_ils_gen_set_marker_keying` |
| `gen_marker_tone` | `sdrkit_ils_gen_get_marker_tone` | `sdrkit_ils_gen_set_marker_tone` |
| `gen_marker_depth` | `sdrkit_ils_gen_get_marker_depth` | `sdrkit_ils_gen_set_marker_depth` |
| `gen_marker_start` | `sdrkit_ils_gen_get_marker_start` | `sdrkit_ils_gen_set_marker_start` |
| `gen_marker_stop` | `sdrkit_ils_gen_get_marker_stop` | `sdrkit_ils_gen_set_marker_stop` |

## Try it

```sh
sdrkit run ils --source synth:ils
sdrkit run ils --source synth:ils --submode localizer
sdrkit run ils --source iiod:192.168.2.1 --freq 110.3
```
<!-- generated: end -->

## Description

The part `marker` names the 75 MHz marker beacon that the aircraft passes over and says whether its keying fits ICAO Annex 10 Volume I, 3.1.7. The automatic part choice takes it for a tuned frequency of 74.5 to 75.5 MHz; the plan `data/plans/aviation.json` lists the band 74.8 to 75.2 MHz for the stack.

The receiver reads the envelope relative to the carrier, so a tone's amplitude is its modulation depth, and measures the 400, 1300 and 3000 Hz tones over periods of 10 ms. A period is keyed when the strongest tone has a depth of at least 30 % and at least 3 times the depth of each other tone. A passage is a run of keyed periods of one tone; a keyed period of another tone, or 1.5 s without a keyed period, ends it. A passage of fewer than 3 elements is dropped. The rate is measured from the first element onset to the last. The keying fits when it matches 3.1.7.5.1 within the tolerance of plus or minus 15 %:

| Marker | Tone | Keying of the standard | Fits when |
|---|---|---|---|
| outer | 400 Hz | 2 dashes a second | not mixed, 1.7 to 2.3 elements a second, shortest element at least 0.15 s |
| middle | 1300 Hz | alternate dashes at 2 a second and dots at 6 a second | dots and dashes, their counts within 1, 75 to 140 dot-dash pairs a minute, longest element under 0.6 s |
| inner | 3000 Hz | 6 dots a second | not mixed, 5.1 to 6.9 elements a second, shortest element at most 0.14 s |

Keying is mixed when the sorted element lengths step up by a ratio of 1.8 or more, with a fifth of the elements (one at the least) on each side of the step; the elements above the step are dashes. The shortest and the longest element leave out the outer tenth, so one element cut by the start of the input or by a fade does not decide. The window of the middle marker covers both readings of 3.1.7.5.1 b: a dash slot of 1/2 s and a dot slot of 1/6 s (90 pairs a minute), and one pair for each dash (120 a minute). A passage that fits is a row of the kind "Marker" and an `ils.marker` event; one that does not is a row of the kind "Tone only". While a passage runs, the fields show it once it has 3 elements; after it, they show the last passage. The live fields (keyed now, held depth, pattern heard) come from the same periods: a tone is on above a depth of 35 % and off below 20 %, and the last 8 on-lengths give the pattern (a dot below 0.18 s, a dash 0.25 to 0.6 s).

The test signal (`gen_marker`, `gen_marker_keying`, `gen_marker_tone`, `gen_marker_depth`, `gen_marker_start`, `gen_marker_stop`) keys the tone of a carrier that is never interrupted, at 95 % depth: dashes of 0.30 s in slots of 1/2 s, dots of 0.08 s in slots of 1/6 s, and for the middle marker a dash and a dot in turn (90 pairs a minute). Its signal to noise ratio is measured in 12 kHz.

Not confirmed by a public document: the dot and dash lengths of the test signal, the element limits of the detector (0.15 s, 0.14 s, 0.6 s, the ratio 1.8 and the trimmed tenth), and the pair window of the middle marker. ICAO Annex 10 gives only the rates.

The depth is read against the mean envelope, which holds the noise as well as the carrier: at 10 dB signal to noise in 12 kHz it reads about 82 % for 95 %. With the carrier 3.5 kHz below the tuned frequency (the carrier tolerance of 3.1.7.2.1 is plus or minus 0.005 %, 3.75 kHz) the 10 kHz channel cuts one sideband of the 3000 Hz tone and its depth reads about 45 %; the inner marker is still named.

Open: a 3000 Hz tone at the edge of its tolerance of plus or minus 2.5 % (75 Hz) loses depth in the 10 ms period.

## Settings: notes

Advanced settings. The owner works without any of them set.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NDB

Non-directional beacons: the carrier, the Morse identification, the audio, and every beacon in the capture.

Stack id: `ndb`. Group: Navigation.

Signal types: NDB.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 344 kHz |
| Tuning range | 150 kHz to 1.8 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 16 kHz |
| Signal bandwidth | 3 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ndb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `ident_keying` | Identification | choice | `auto` | `auto` (Automatic), `a` (Keyed tone (A2A)), `b` (Keyed carrier (A1A)) | A keyed tone, a keyed carrier, or the one that keys. |
| `tone` | Identification tone | frequency | 0 Hz | 0 Hz to 3 kHz | 0 finds it (400 or 1020 Hz). |
| `audio` | Audio | choice | `am` | `am` (AM), `cw` (Beat note) |  |
| `pitch` | Beat pitch | frequency | 800 Hz | 100 Hz to 3 kHz |  |
| `confirm` | Hearings to confirm | whole number | 2 | 1 to 10 | How many times an identification must be heard before it is shown. |
| `carrier_threshold` | Carrier threshold | number (dB-Hz) | 30 | 0 to 90 |  |
| `sync_tones` | Synchronous tone detection | yes or no | true |  | The tone is read from the in-phase part against the followed carrier, not from the envelope. |
| `ident_average` | Average repeated idents | yes or no | true |  | The identification is also read from the average of its repeats over up to three minutes. |
| `scan_threshold` | Scan threshold | number (dB-Hz) | 35 | 10 to 90 | How far above the noise a carrier must stand to be listed. |
| `scan_interval` | Scan every | duration (s) | 2 s | 0.5 to 60 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_ident` | Test signal: identification | text | `BK` |  |  |
| `gen_keying` | Test signal: keying | choice | `a2a` | `a2a` (Keyed tone), `a1a` (Keyed carrier) |  |
| `gen_tone` | Test signal: tone | frequency | 1.02 kHz | 100 Hz to 3 kHz |  |
| `gen_depth` | Test signal: modulation depth | number | 0.3 | 0.05 to 1 |  |
| `gen_wpm` | Test signal: Morse speed | number (wpm) | 10 | 3 to 20 |  |
| `gen_pause` | Test signal: pause | duration (s) | 2 s | 0.5 to 30 |  |
| `gen_others` | Test signal: other beacons | whole number | 2 | 0 to 8 | Beacons beside the tuned one, for the band scan. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `tuned_hz` | Tuned | frequency |
| `ident` | Identification | text |
| `last_ident` | Last heard | text |
| `ident_wpm` | Morse speed | number (wpm) |
| `carrier` | Carrier | yes or no |
| `carrier_hz` | Carrier | frequency |
| `cn0_dbhz` | C/N0 | number (dB-Hz) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `ident_period_s` | Identification period | number (s) |
| `ident_period_z` | Period z-score | number |
| `ident_fold_quality` | Average quality | number |
| `ident_fold_text` | Read from the average | text |
| `id_tone_hz` | Identification tone | frequency |
| `id_depth_pct` | Identification depth | number (%) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `idents` | Identifications heard | `ident` (text), `heard` (whole number) |
| `scan` | Beacons in the input | `freq_hz` (frequency), `level_dbfs` (number (dBFS)), `cn0_dbhz` (number (dB-Hz)) |

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

Struct `TunedFields`. The tuned frequency.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tuned_hz` | frequency | always | 1.0 | The tuned frequency. |

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

Struct `NdbFields`. The identification that the average of the repeats reads. No value while the average reads nothing.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ident_period_s` | number (s) | optional | 1.0 | The period of the identification. |
| `ident_period_z` | number | optional | 1.0 | The robust z-score of the autocorrelation at that period. |
| `ident_fold_quality` | number | optional | 1.0 | The swing of the average of the repeats over its noise. |
| `ident_fold_text` | text | optional | 1.0 | The word read from the average. |

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

### Table `scan`

Struct `NdbScanRow`. A row of the band scan. The row key is the frequency in whole hertz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq_hz` | frequency | always | 1.0 | The frequency of the carrier. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the carrier. |
| `cn0_dbhz` | number (dB-Hz) | always | 1.0 | The carrier to noise density. |

Default order: the column `cn0_dbhz`, highest first.

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
| `clear` | `sdrkit_ndb_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `ident_keying` | `sdrkit_ndb_get_ident_keying` | `sdrkit_ndb_set_ident_keying` |
| `tone` | `sdrkit_ndb_get_tone` | `sdrkit_ndb_set_tone` |
| `audio` | `sdrkit_ndb_get_audio` | `sdrkit_ndb_set_audio` |
| `pitch` | `sdrkit_ndb_get_pitch` | `sdrkit_ndb_set_pitch` |
| `confirm` | `sdrkit_ndb_get_confirm` | `sdrkit_ndb_set_confirm` |
| `carrier_threshold` | `sdrkit_ndb_get_carrier_threshold` | `sdrkit_ndb_set_carrier_threshold` |
| `sync_tones` | `sdrkit_ndb_get_sync_tones` | `sdrkit_ndb_set_sync_tones` |
| `ident_average` | `sdrkit_ndb_get_ident_average` | `sdrkit_ndb_set_ident_average` |
| `scan_threshold` | `sdrkit_ndb_get_scan_threshold` | `sdrkit_ndb_set_scan_threshold` |
| `scan_interval` | `sdrkit_ndb_get_scan_interval` | `sdrkit_ndb_set_scan_interval` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_ident` | `sdrkit_ndb_gen_get_ident` | `sdrkit_ndb_gen_set_ident` |
| `gen_keying` | `sdrkit_ndb_gen_get_keying` | `sdrkit_ndb_gen_set_keying` |
| `gen_tone` | `sdrkit_ndb_gen_get_tone` | `sdrkit_ndb_gen_set_tone` |
| `gen_depth` | `sdrkit_ndb_gen_get_depth` | `sdrkit_ndb_gen_set_depth` |
| `gen_wpm` | `sdrkit_ndb_gen_get_wpm` | `sdrkit_ndb_gen_set_wpm` |
| `gen_pause` | `sdrkit_ndb_gen_get_pause` | `sdrkit_ndb_gen_set_pause` |
| `gen_others` | `sdrkit_ndb_gen_get_others` | `sdrkit_ndb_gen_set_others` |

## Try it

```sh
sdrkit run ndb --source synth:ndb
sdrkit run ndb --source <driver>:<address> --freq 0.344
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

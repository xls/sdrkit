<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Piccolo Mk VI

6-tone MFSK teleprinter: each ITA2 character a pair of 50 ms tones 20 Hz apart from a 6 x 6 matrix; the tones and the pair phase found.

Stack id: `piccolo`. Group: HF utility.

Piccolo Mark VI: a 6-tone MFSK teleprinter. One tone at a time, 20 Hz apart, 50 ms each, continuous phase; each ITA2 character is a pair of tones from a 6 x 6 matrix. While idle the signal sends SYNC and its barred form (D E, E D); the pair D D is never sent, which tells which tone starts a pair.

Standard: ITU-R SM.1052 Annex 1 names "Piccolo mk 6"; no public document gives the tones or the matrix.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 6.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:piccolo` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `alphabet` | Figures case | choice | `us` | `us` (US), `itu` (ITU) | The figures of the ITA2 alphabet: US teleprinter or ITU. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `spacing_hz` | Tone spacing | frequency | 20 Hz | 5 Hz to 200 Hz | Not confirmed by a public document. |
| `tone_ms` | Tone length | number (ms) | 50 | 5 to 500 | Not confirmed by a public document. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_base_hz` | Tone A | frequency | 460 Hz | 300 Hz to 2.8 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `state` | State | whole number |
| `baud` | Speed | number (Bd) |
| `centre_hz` | Centre | number (Hz) |
| `shift_hz` | Shift | number (Hz) |
| `detail` | Detail | text |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fsk.lock` | Sync | `text` (text), `baud` (number (Bd)), `shift_hz` (number (Hz)), `centre_hz` (number (Hz)), `detail` (text) |
| `fsk.lost` | Sync lost | `text` (text), `reason` (text) |
| `text` | Text | `text` (text) |
| `piccolo.pairs` | Tone pairs | `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.2. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `TeleprinterFields`. The result fields of a teleprinter stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `text` | text | always | 1.0 | The text received, the last characters. |
| `state` | whole number | always | 1.0 | The search state: 0 searching, 1 signal found, 2 decoding. |
| `baud` | number (Bd) | optional | 1.0 | The speed of the signal. Absent while the front has no lock. |
| `centre_hz` | number (Hz) | optional | 1.0 | The centre of the signal above the dial. Absent while the front has no lock. |
| `shift_hz` | number (Hz) | optional | 1.0 | The shift between the tones. Absent while the front has no lock or the signal has no shift. |
| `detail` | text | always | 1.0 | What was found beside the speed (cycle, depth, tone length). Empty while not decoding. |
| `good` | whole number | always | 1.0 | Words, characters or frames read. |
| `bad` | whole number | always | 1.0 | Words, characters or frames that failed their check. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel, dBFS. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Event `fsk.lock`

Struct `FskLockEvent`. A lock of the words, characters or frames.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `baud` | number (Bd) | always | 1.0 | The speed of the signal. |
| `shift_hz` | number (Hz) | always | 1.0 | The shift between the tones. |
| `centre_hz` | number (Hz) | always | 1.0 | The centre of the signal above the dial. |
| `detail` | text | always | 1.0 | What was found beside the speed. |

### Event `fsk.lost`

Struct `FskLostEvent`. The sync of the words, characters or frames was lost.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `reason` | text | always | 1.0 | Why sync was lost. |

### Event `text`

Struct `TextEvent`. New text: a line, 64 characters, or what came before a pause.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text. |

### Event `piccolo.pairs`

Struct `PiccoloPairsEvent`. The tone pairs of Piccolo were found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_piccolo_clear` | none | none |
| `reset` | `sdrkit_piccolo_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `alphabet` | `sdrkit_piccolo_get_alphabet` | `sdrkit_piccolo_set_alphabet` |
| `spacing_hz` | `sdrkit_piccolo_get_spacing_hz` | `sdrkit_piccolo_set_spacing_hz` |
| `tone_ms` | `sdrkit_piccolo_get_tone_ms` | `sdrkit_piccolo_set_tone_ms` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_base_hz` | `sdrkit_piccolo_gen_get_base_hz` | `sdrkit_piccolo_gen_set_base_hz` |

## Try it

```sh
sdrkit run piccolo --source synth:piccolo
sdrkit run piccolo --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The receiver takes the 24 kHz channel of the family front end, sums each three samples to 8000 samples/s and runs a tone-slot front: the base tone from a 1.024 s spectrum (the largest sum over the comb of tones), the slot length and phase from the share of the strongest tone over 70 slots, then the strongest tone of every slot. While the signal idles on SYNC (tones D and E) the base moves so that those tones are D and E. After 64 slots the pair phase is the one where D D (never sent) is rare and SYNC common. SYNC, MI and D D are skipped, LTRS and FIGS switch the case, the unused pair counts as bad.

Standard: ITU-R SM.1052 Annex 1 names Piccolo Mk 6. The tones, their spacing and length, the matrix and the idle pattern are not confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv` and band presets are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Try it: notes

```sh
sdrkit run piccolo --source synth:piccolo --seconds 16
sdrkit run piccolo --source <driver>:<address> --freq 7
```

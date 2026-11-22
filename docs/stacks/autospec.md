<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# AUTOSPEC / SPREAD

FEC teleprinter on 2-FSK: 10-unit Bauer code words of ITA2 characters, one error corrected; SPREAD spreads each word over 11, 21 or 51 places (found).

Stack id: `autospec`. Group: HF utility.

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

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `autospec` | AUTOSPEC | 6.5 MHz |
| `spread` | SPREAD | 6.5 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:autospec` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Signal | choice | `autospec` | `autospec` (AUTOSPEC), `spread` (SPREAD) |  |
| `baud` | Speed | number (Bd) | 0 | 0 to 300 | 0 finds it from the candidates of the submode. |
| `shift_hz` | Shift | frequency | 0 Hz | 0 Hz to 1.2 kHz | 0 finds it from the signal. |
| `polarity` | Polarity | choice | `auto` | `auto` (Automatic), `normal` (Normal), `inverted` (Inverted) |  |
| `depth` | Spread depth | choice | `auto` | `auto` (Automatic), `11`, `21`, `51` | SPREAD only. |
| `alphabet` | Figures case | choice | `us` | `us` (US), `itu` (ITU) | The figures of the ITA2 alphabet: US teleprinter or ITU. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `idle_code` | Idle character | whole number | 21 | 0 to 31 | The ITA2 code sent while idle; a run of three or more is not shown. Not confirmed by a public document. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_baud` | Speed | number (Bd) | 68.5 | 10 to 300 |  |
| `gen_shift_hz` | Shift | frequency | 0 Hz | 0 Hz to 1.1 kHz | 0 sends 270 Hz for AUTOSPEC and 410 Hz for SPREAD. |
| `gen_depth` | Spread depth | choice | `11` | `auto` (Automatic), `11`, `21`, `51` | Automatic sends 11. |
| `gen_centre_hz` | Centre | frequency | 1.5 kHz | 400 Hz to 3.5 kHz |  |

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

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.7. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

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

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_autospec_clear` | none | none |
| `reset` | `sdrkit_autospec_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_autospec_get_submode` | `sdrkit_autospec_set_submode` |
| `baud` | `sdrkit_autospec_get_baud` | `sdrkit_autospec_set_baud` |
| `shift_hz` | `sdrkit_autospec_get_shift_hz` | `sdrkit_autospec_set_shift_hz` |
| `polarity` | `sdrkit_autospec_get_polarity` | `sdrkit_autospec_set_polarity` |
| `depth` | `sdrkit_autospec_get_depth` | `sdrkit_autospec_set_depth` |
| `alphabet` | `sdrkit_autospec_get_alphabet` | `sdrkit_autospec_set_alphabet` |
| `idle_code` | `sdrkit_autospec_get_idle_code` | `sdrkit_autospec_set_idle_code` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_baud` | `sdrkit_autospec_gen_get_baud` | `sdrkit_autospec_gen_set_baud` |
| `gen_shift_hz` | `sdrkit_autospec_gen_get_shift_hz` | `sdrkit_autospec_gen_set_shift_hz` |
| `gen_depth` | `sdrkit_autospec_gen_get_depth` | `sdrkit_autospec_gen_set_depth` |
| `gen_centre_hz` | `sdrkit_autospec_gen_get_centre_hz` | `sdrkit_autospec_gen_set_centre_hz` |

## Try it

```sh
sdrkit run autospec --source synth:autospec
sdrkit run autospec --source synth:autospec --submode autospec
sdrkit run autospec --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The receiver takes the 24 kHz channel of the family front end, sums each three samples to 8000 samples/s and runs a 2-FSK front: the tone pair from a 1.024 s Hann spectrum, the speed from the eye opening at each candidate speed (else from the line of the transition times), the clock phase, a refinement of both tones in 6 Hz steps, and an integrate-and-dump slicer whose clock follows the signal slowly. The front searches again after 12 s without a speed, or after 3000 symbols without sync. Each character is a Bauer code word: the five elements, then five check units that repeat them for an odd character and invert them for an even one (minimum distance 4). SPREAD sends unit k of character n at stream position 10 n + k S. The lock tries each depth, both polarities and the 10 offsets over 90 words: at least 80 % consistent, at least 10 different characters, the most common at most 60 %. A word decodes to the nearest code word; at distance 2 or more an underscore is shown. A run of three or more idle characters is not shown.

Standard: ITU-R SM.1052 Annex 1 names AUTOSPEC. The Bauer code rule, the speeds, the SPREAD layout and the idle character are not confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv` and band presets are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Try it: notes

```sh
sdrkit run autospec --source synth:autospec --seconds 35
sdrkit run autospec --source synth:autospec/spread --submode spread --seconds 45
sdrkit run autospec --source <driver>:<address> --freq 7
```

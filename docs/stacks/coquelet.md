<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Coquelet-8

8-tone MFSK teleprinter: each ITA2 character a pair of tones 26.67 Hz apart; the tones, the tone length (37.5, 50 or 75 ms) and the pair phase found.

Stack id: `coquelet`. Group: HF utility.

Coquelet-8: an 8-tone MFSK teleprinter. One tone at a time, 26.67 Hz apart, 37.5, 50 or 75 ms each (found); each ITA2 character is a pair of tones: the first (tone 1 to 8) carries elements 3, 4 and 5, the second (tone 5 to 8) elements 1 and 2. While idle the signal alternates tones 1 and 8, which is also the pair of M: a run of one or two such pairs is shown as M, a longer run is idle.

Standard: ITU-R SM.1052 Annex 1 names "Coquelet mk 1, 2"; no public document gives the tones or the code.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:coquelet` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tone_ms` | Tone length | choice | `auto` | `auto` (Automatic), `37.5` (37.5 ms), `50` (50 ms), `75` (75 ms) |  |
| `alphabet` | Figures case | choice | `us` | `us` (US), `itu` (ITU) | The figures of the ITA2 alphabet: US teleprinter or ITU. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `spacing_hz` | Tone spacing | frequency | 26.666667 Hz | 5 Hz to 200 Hz | Not confirmed by a public document. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tone_ms` | Tone length | choice | `50` | `auto` (Automatic), `37.5` (37.5 ms), `50` (50 ms), `75` (75 ms) | Automatic sends 50 ms. |
| `gen_base_hz` | Tone 1 | frequency | 773 Hz | 300 Hz to 2.7 kHz |  |

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
| `coquelet.pairs` | Tone pairs | `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

### Event `coquelet.pairs`

Struct `CoqueletPairsEvent`. The tone pairs of Coquelet were found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_coquelet_clear` | none | none |
| `reset` | `sdrkit_coquelet_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `tone_ms` | `sdrkit_coquelet_get_tone_ms` | `sdrkit_coquelet_set_tone_ms` |
| `alphabet` | `sdrkit_coquelet_get_alphabet` | `sdrkit_coquelet_set_alphabet` |
| `spacing_hz` | `sdrkit_coquelet_get_spacing_hz` | `sdrkit_coquelet_set_spacing_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tone_ms` | `sdrkit_coquelet_gen_get_tone_ms` | `sdrkit_coquelet_gen_set_tone_ms` |
| `gen_base_hz` | `sdrkit_coquelet_gen_get_base_hz` | `sdrkit_coquelet_gen_set_base_hz` |

## Try it

```sh
sdrkit run coquelet --source synth:coquelet
sdrkit run coquelet --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The receiver takes the 24 kHz channel of the family front end, sums each three samples to 8000 samples/s and runs a tone-slot front: the base tone from a 1.024 s spectrum (the largest sum over the comb of tones), the slot length and phase from the share of the strongest tone over 70 slots, then the strongest tone of every slot. The slot lengths 37.5, 50 and 75 ms are tried; a longer slot wins at 0.9 of the best share (half a slot also scores well). The first tone carries elements 3, 4 and 5, the second (tones 5 to 8) elements 1 and 2; the pair phase is the one where the second tone is most often 5 to 8. The idle pair (1, 8) is also M: a run of one or two is shown as M, a longer run is idle.

Standard: ITU-R SM.1052 Annex 1 names Coquelet mk 1 and 2. The tones, their spacing, the tone table and the idle rule are not confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv` and band presets are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Try it: notes

```sh
sdrkit run coquelet --source synth:coquelet --seconds 16
sdrkit run coquelet --source <driver>:<address> --freq 7
```

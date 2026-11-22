<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ARQ-E / ARQ-N

Duplex ARQ teleprinter on 2-FSK with 7-unit words: speed, shift, word alignment, polarity and the inversion cycle found; ITA2 text.

Stack id: `arqe`. Group: HF utility.

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
| `arqe` | ARQ-E | 6.5 MHz |
| `arqn` | ARQ-N | 6.5 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:arqe` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Signal | choice | `arqe` | `arqe` (ARQ-E), `arqn` (ARQ-N) |  |
| `baud` | Speed | number (Bd) | 0 | 0 to 300 | 0 finds it from the candidates of the submode. |
| `shift_hz` | Shift | frequency | 0 Hz | 0 Hz to 1.2 kHz | 0 finds it from the signal. |
| `polarity` | Polarity | choice | `auto` | `auto` (Automatic), `normal` (Normal), `inverted` (Inverted) |  |
| `cycle` | Inversion cycle | choice | `auto` | `auto` (Automatic), `none` (No inversion), `4` (Every 4th word), `8` (Every 8th word) | ARQ-E only; ARQ-N inverts no word. |
| `alphabet` | Figures case | choice | `us` | `us` (US), `itu` (ITU) | The figures of the ITA2 alphabet: US teleprinter or ITU. |
| `word_code` | Word code | choice | `parity` | `parity` (Identification and parity), `f342` (ITU-R F.342 constant ratio) | Which 7-unit code the signal uses is not confirmed by a public document: an identification unit, ITA2 and odd parity, or ITU-R F.342. |
| `channels` | Channels | choice | `auto` | `auto` (Automatic), `1` (One channel), `2` (Two channels), `4` (Four channels) | ITU-R F.342: 2 channels send their characters in turn; 4 channels send A and C, then B and D, element by element. |
| `channel` | Channel shown | choice | `a` | `a` (A), `b` (B), `c` (C), `d` (D) | The channel whose text is shown when the signal has 2 or 4 channels. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_baud` | Speed | number (Bd) | 96 | 10 to 300 |  |
| `gen_shift_hz` | Shift | frequency | 400 Hz | 60 Hz to 1.1 kHz |  |
| `gen_cycle` | Inversion cycle | choice | `8` | `auto` (Automatic), `none` (No inversion), `4` (Every 4th word), `8` (Every 8th word) | Automatic sends 8. ARQ-N inverts no word. |
| `gen_channels` | Channels | choice | `1` | `auto` (Automatic), `1` (One channel), `2` (Two channels), `4` (Four channels) | Automatic sends one. Channel A sends the test text, the others the test text after their name. |
| `gen_word_code` | Word code | choice | `parity` | `parity` (Identification and parity), `f342` (ITU-R F.342 constant ratio) |  |
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

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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
| `clear` | `sdrkit_arqe_clear` | none | none |
| `reset` | `sdrkit_arqe_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_arqe_get_submode` | `sdrkit_arqe_set_submode` |
| `baud` | `sdrkit_arqe_get_baud` | `sdrkit_arqe_set_baud` |
| `shift_hz` | `sdrkit_arqe_get_shift_hz` | `sdrkit_arqe_set_shift_hz` |
| `polarity` | `sdrkit_arqe_get_polarity` | `sdrkit_arqe_set_polarity` |
| `cycle` | `sdrkit_arqe_get_cycle` | `sdrkit_arqe_set_cycle` |
| `alphabet` | `sdrkit_arqe_get_alphabet` | `sdrkit_arqe_set_alphabet` |
| `word_code` | `sdrkit_arqe_get_word_code` | `sdrkit_arqe_set_word_code` |
| `channels` | `sdrkit_arqe_get_channels` | `sdrkit_arqe_set_channels` |
| `channel` | `sdrkit_arqe_get_channel` | `sdrkit_arqe_set_channel` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_baud` | `sdrkit_arqe_gen_get_baud` | `sdrkit_arqe_gen_set_baud` |
| `gen_shift_hz` | `sdrkit_arqe_gen_get_shift_hz` | `sdrkit_arqe_gen_set_shift_hz` |
| `gen_cycle` | `sdrkit_arqe_gen_get_cycle` | `sdrkit_arqe_gen_set_cycle` |
| `gen_channels` | `sdrkit_arqe_gen_get_channels` | `sdrkit_arqe_gen_set_channels` |
| `gen_word_code` | `sdrkit_arqe_gen_get_word_code` | `sdrkit_arqe_gen_set_word_code` |
| `gen_centre_hz` | `sdrkit_arqe_gen_get_centre_hz` | `sdrkit_arqe_gen_set_centre_hz` |

## Try it

```sh
sdrkit run arqe --source synth:arqe
sdrkit run arqe --source synth:arqe --submode arqe
sdrkit run arqe --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The receiver takes the 24 kHz channel of the family front end, sums each three samples to 8000 samples/s and runs a 2-FSK front: the tone pair from a 1.024 s Hann spectrum, the speed from the eye opening at each candidate speed (else from the line of the transition times), the clock phase, a refinement of both tones in 6 Hz steps, and an integrate-and-dump slicer whose clock follows the signal slowly. The front searches again after 12 s without a speed, or after 3000 symbols without sync. The word lock tries both polarities, the 7 word alignments and, for ARQ-E, no inversion and the cycles of 4 and 8 words at each phase over 120 words; at least 93 % must pass the word check, and at least 24 traffic words of 10 kinds must be among them (an idle pattern passes at any alignment). A word that fails counts as bad; the lock is lost when the error integrator (0.95 x + 1 on a bad word) passes 8. Two word codes can be read (`word_code`): an identification unit, the five ITA2 elements and an odd-parity unit, or the constant-ratio code of ITU-R F.342-2 Table I, whose candidate speeds are then those of F.342-2 Table IV and F.519 (48, 72, 85 5/7, 96, 100, 171 3/7, 192 and 200 Bd). Signals of 2 or 4 time-division channels of F.342-2 Annex I are read (the setting `channels`; 4 channels interleave A with C and B with D element by element) and the setting `channel` picks the channel shown; on a 4-channel signal that idles the automatic speed can find half the rate, so give the speed. Which one the signals named ARQ-E use is not established by a public document.

Standard: ITU-R F.342-2 (the 7-unit code, repetition cycles of 4 and 8, 48 to 192 Bd), ITU-R F.519 (single channel), ITU-R SM.1052 Annex 1 (the names). The candidate speeds 46.2, 50, 64, 144, 184.6 and 288 Bd, ARQ-N, the parity word and the US figures case are not confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv` and band presets are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run arqe --source synth:arqe --seconds 25
sdrkit run arqe --source synth:arqe/arqn --submode arqn --seconds 25
sdrkit run arqe --source <driver>:<address> --freq 7
```

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# WINMOR (detect only)

Detect only: the 1500 Hz alternating-phase pilot of the retired WINMOR modem, the tuning error and the four raw frame-type tones.

Stack id: `winmor`. Group: HF and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 7 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 3 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:winmor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tones` | Frame-type tones | text | `1 6 3 4` |  | Four indices 0 to 7. |
| `gen_error_hz` | Tuning error | frequency | 0 Hz | -200 Hz to 200 Hz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `note` | Note | text |
| `pilots` | Pilots | whole number |
| `error_hz` | Tuning error | number (Hz) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `pilots` | Pilots | `time` (time), `error_hz` (number (Hz)), `pilot_symbols` (whole number), `tones` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `winmor.pilot` | Pilot | `text` (text), `error_hz` (number (Hz)), `pilot_symbols` (whole number), `tones` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfkbd`, version 2.6. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `WinmorFields`. The result fields of WINMOR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | A note on the stack. |
| `pilots` | whole number | always | 1.0 | Pilots heard. |
| `error_hz` | number (Hz) | optional | 1.0 | The tuning error of the newest pilot, if one was heard. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `pilots`

Struct `WinmorPilotRow`. A row of the WINMOR pilots. The row key is the place of the pilot in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | always | 1.0 | The UTC. |
| `error_hz` | number (Hz) | always | 1.0 | The tuning error. |
| `pilot_symbols` | whole number | always | 1.0 | The pilot symbols heard. |
| `tones` | text | always | 1.0 | The frame-type tones. |

### Event `winmor.pilot`

Struct `WinmorPilotEvent`. The fields of a WINMOR pilot.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | What was heard. |
| `error_hz` | number (Hz) | always | 1.0 | The tuning error. |
| `pilot_symbols` | whole number | always | 1.0 | The pilot symbols heard. |
| `tones` | text | always | 1.0 | The frame-type tones. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_winmor_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen` | `sdrkit_winmor_get_listen` | `sdrkit_winmor_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tones` | `sdrkit_winmor_gen_get_tones` | `sdrkit_winmor_gen_set_tones` |
| `gen_error_hz` | `sdrkit_winmor_gen_get_error_hz` | `sdrkit_winmor_gen_set_error_hz` |

## Try it

```sh
sdrkit run winmor --source synth:winmor
sdrkit run winmor --source <driver>:<address> --freq 7
```
<!-- generated: end -->

## Description

Each WINMOR frame begins with a 1500 Hz carrier keyed in DBPSK at 93.75 Bd whose phase is reversed every symbol, then one symbol that keeps the phase (the frame sync), then four 4-FSK symbols at 46.875 Bd that carry the frame type. The receiver works at 12 kHz: it counts the samples where the correlation of the signal with itself one symbol earlier stays above 0.75 of its power, triggers after 12 symbols of that, and 34 symbols later measures the tuning error from the line of the squared signal (2600 to 3400 Hz), finds the symbol timing from the phase reversals of the 8 symbols before the trigger, walks to the frame sync, and names the strongest of the eight frame-type tones in each of the four 256-sample symbols after it (indices 0 to 7). A plain carrier keeps its phase and is not reported. Frames are not read: the symbol mapping, the Reed-Solomon code and the callsign packing are not in the specification.

Standard: R. Muething KN6KB, WINMOR protocol specification, revision 1.0.6, sections 5 and 6.1. It was not read for this receiver, so nothing here is confirmed by a public document; the tone 1358.375 Hz of the specification is taken as 1359.375 Hz.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfkbd/data/slow_dials.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run winmor --source synth:winmor
sdrkit run winmor --source <driver>:<address> --freq 7.1
```

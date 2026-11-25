<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Link 11 CLEW (structure)

The 16-tone Link 11 waveform: messages, frame rate, frame count, Doppler offset and the raw phase steps of the 15 data tones (no bits).

Stack id: `link11`. Group: HF utility.

Link 11 CLEW, structure only: the 16-tone parallel waveform with its 605 Hz Doppler tone and 15 data tones in frames of 13.33 or 22 ms; message boundaries, the frame count and the raw phase steps. No bits are read. ```text iq -> [hf_front 24 kHz] -> [link11: tone DFTs, preamble, onset, rate and alignment, phase steps] -> message table, text ```

Standard: MIL-STD-188-203-1A and STANAG 5511 (not public); public summaries.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:link11` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `frame_rate` | Frame rate | choice | `auto` | `auto` (Automatic), `fast` (Fast (13.33 ms)), `slow` (Slow (22 ms)) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_rate` | Frame rate | choice | `fast` | `auto` (Automatic), `fast` (Fast (13.33 ms)), `slow` (Slow (22 ms)) | Automatic sends fast frames. |
| `gen_offset_deg` | Phase step offset | number (°) | 45 | 0 to 89 |  |
| `gen_preamble_frames` | Preamble frames | whole number | 5 | 2 to 20 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `messages_total` | Messages | whole number |
| `offset_hz` | Offset | frequency |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `start_s` (number (s)), `rate` (text), `preamble_frames` (whole number), `data_frames` (whole number), `duration_s` (number (s)), `doppler_hz` (number (Hz)), `offset_deg` (number (°)), `snr_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `link11.event` | Event | `text` (text) |
| `link11.message` | Message | `text` (text), `data_frames` (whole number), `steps` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfdata`, version 2.0. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

### Result fields

Struct `Link11Fields`. The result fields of Link 11.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | The parts that no public document confirms. |
| `text` | text | always | 1.0 | The text received, the last lines. |
| `messages_total` | whole number | always | 1.0 | Messages since the last Clear. |
| `offset_hz` | frequency | optional | 1.0 | The Doppler offset of the last message. Absent without a measure. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `Link11MessageRow`. A message of Link 11. The row key is the number of the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start_s` | number (s) | always | 1.0 | The start of the message, seconds. |
| `rate` | text | always | 1.0 | The frame rate: "fast" or "slow". |
| `preamble_frames` | whole number | always | 1.0 | Preamble frames. |
| `data_frames` | whole number | always | 1.0 | Data frames. |
| `duration_s` | number (s) | always | 1.0 | The duration, seconds. |
| `doppler_hz` | number (Hz) | always | 1.0 | The Doppler offset. |
| `offset_deg` | number (°) | always | 1.0 | The constellation offset, degrees. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio of the message. |

The rows are a log: they arrive over time. Time: `start_s`.

### Event `link11.event`

Struct `Link11Event`. An event of Link 11.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |

### Event `link11.message`

Struct `Link11MessageEvent`. A message of Link 11.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the message. |
| `data_frames` | whole number | always | 1.0 | Data frames. |
| `steps` | text | always | 1.0 | The quarter turns of every data frame (15 digits a frame). |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_link11_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `frame_rate` | `sdrkit_link11_get_frame_rate` | `sdrkit_link11_set_frame_rate` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_rate` | `sdrkit_link11_gen_get_rate` | `sdrkit_link11_gen_set_rate` |
| `gen_offset_deg` | `sdrkit_link11_gen_get_offset_deg` | `sdrkit_link11_gen_set_offset_deg` |
| `gen_preamble_frames` | `sdrkit_link11_gen_get_preamble_frames` | `sdrkit_link11_gen_set_preamble_frames` |

## Try it

```sh
sdrkit run link11 --source synth:link11
sdrkit run link11 --source <driver>:<address> --freq 6.5
```

## Test recordings

- `tests/vectors/hfdata/wfid_link11_24k.sigmf-meta`
<!-- generated: end -->

## Description

The 16-tone Link 11 waveform: messages, frame rate, frame count, Doppler offset and the raw phase steps of the 15 data tones (no bits). The receiver takes DFTs at the tones over windows referred to absolute time, finds the preamble of the 605 Hz Doppler tone and the 2915 Hz sync tone, the onset of the data tones, the frame rate and alignment that keep each tone's phase inside a frame, and the phase step of each data tone frame by frame against the Doppler tone's; the constellation offset comes from the fourth power of the steps. MIL-STD-188-203-1A and STANAG 5511 are not public: the tone plan and the frame rates come from public summaries, and nothing here is confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_serial_dials.csv` and band presets
are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run link11 --source synth:link11 --seconds 15 --audio none
sdrkit run link11 --source iiod:192.168.2.1 --freq 7 frame_rate=auto
```

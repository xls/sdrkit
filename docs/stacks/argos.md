<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Argos

Argos PTT-A2 platform bursts (other modulations listed) and the A-DCS downlink's HDLC frames. Receive only.

Stack id: `argos`. Group: Satellite.

Signal types: Argos.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 401.65 MHz |
| Tuning range | 401.5 MHz to 466.1 MHz |
| Sample rate | 256 kHz |
| Smallest sample rate | 128 kHz |
| Signal bandwidth | 100 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 16 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:argos` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `link` | Link | choice | `auto` | `auto` (Automatic (from the dial)), `uplink` (Uplink), `downlink` (Downlink) | Automatic: the downlink above 430 MHz. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_downlink` | Downlink | yes or no | false |  |  |
| `gen_gap_s` | Seconds between bursts | duration (s) | 2 s | 0.5 to 60 |  |
| `gen_step_hz` | Burst-to-burst step | frequency | 0 Hz | -500 Hz to 500 Hz |  |
| `gen_other` | Another modulation each cycle | yes or no | false |  |  |
| `gen_pass_doppler_hz` | Largest Doppler of the pass | frequency | 9 kHz | 0 Hz to 20 kHz |  |
| `gen_pass_tau_s` | Doppler time constant | duration (s) | 60 s | 5 to 600 |  |
| `gen_pass_tca_s` | Closest approach | duration (s) | 40 s | 0 to 3600 |  |
| `gen_bpsk` | BPSK | yes or no | false |  |  |
| `gen_nrzi` | NRZI | yes or no | true |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `link` | Link | choice |
| `locked` | Locked | yes or no |
| `carrier_line_db` | Carrier line | number (dB) |
| `downlink_bits` | Downlink bits | whole number |
| `coding` | Coding | choice |
| `dropped` | Dropped | whole number |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bursts` | Bursts | `at` (duration (s)), `heard_at` (time), `kind` (choice), `offset_hz` (frequency), `snr_db` (number (dB)), `carrier_ms` (number (ms)), `total_ms` (number (ms)), `quality` (number), `platform` (text), `data_bits` (whole number), `data` (bytes) |
| `platforms` | Platforms | `platform` (text), `data` (bytes), `drift_hz_s` (number (Hz/s)), `last_heard` (time), `heard` (whole number) |
| `frames` | Downlink frames | `at` (duration (s)), `coding` (choice), `bytes` (whole number), `frame` (bytes) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `doppler` | Downlink Doppler | s | Hz |

## Events

| Kind | Label | Fields |
|---|---|---|
| `argos.ptt` | Platform message | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `platform` (text), `data_bits` (whole number), `offset_hz` (frequency), `quality` (number), `data` (bytes) |
| `argos.downlink_frame` | Downlink frame | `family` (choice), `coding` (choice), `bytes` (whole number), `frame` (bytes) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `satcom`, version 1.5. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

### Result fields

Struct `ArgosFields`. The result fields of Argos.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `link` | choice | always | 1.0 | The link the display shows: uplink or downlink. |
| `locked` | yes or no | always | 1.0 | The receiver holds lock. |
| `carrier_line_db` | number (dB) | always | 1.0 | The downlink's carrier line over the noise. |
| `downlink_bits` | whole number | always | 1.0 | Downlink bits received. |
| `coding` | choice | optional | 1.0 | The downlink coding in use. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bursts`

Struct `ArgosBurstRow`. An uplink burst. The row key is the sequence number of the burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of the row's moment. |
| `heard_at` | time | optional | 1.0 | The wall-clock time of the row's moment, when the samples were live. |
| `kind` | choice | always | 1.0 | The modulation of a burst. |
| `offset_hz` | frequency | always | 1.0 | The carrier from the dial. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `carrier_ms` | number (ms) | always | 1.0 | The unmodulated carrier. |
| `total_ms` | number (ms) | always | 1.0 | The whole burst. |
| `quality` | number | always | 1.0 | The quality of the coherent soft bits (1 without noise, 0.64 for noise alone). |
| `platform` | text | optional | 1.0 | The platform id (7 hex digits). |
| `data_bits` | whole number | optional | 1.0 | The number of user data bits. |
| `data` | bytes | optional | 1.0 | The user data, the first bit the high bit of the first byte. |

The column `platform` names a row.

### Table `platforms`

Struct `ArgosPlatformRow`. A platform heard. The row key is the platform id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `platform` | text | always | 1.0 | The platform id (7 hex digits). |
| `data` | bytes | always | 1.0 | The user data, the first bit the high bit of the first byte. |
| `drift_hz_s` | number (Hz/s) | always | 1.0 | The drift. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |
| `heard` | whole number | optional | 1.0 | How many times it was heard. |

The column `platform` names a row.

### Table `frames`

Struct `ArgosFrameRow`. A downlink frame with a good FCS. The row key is the sequence number of the frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of the row's moment. |
| `coding` | choice | always | 1.0 | The downlink coding in use. |
| `bytes` | whole number | always | 1.0 | The length of a frame without its FCS. |
| `frame` | bytes | always | 1.0 | A frame's bytes. |

### Event `argos.ptt`

Struct `ArgosPttEvent`. A platform message. The source is the platform id, the text the data in hexadecimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The platform id. |
| `text` | text | always | 1.0 | The data in hexadecimal. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `platform` | text | always | 1.0 | The platform id (7 hex digits). |
| `data_bits` | whole number | always | 1.0 | The number of user data bits. |
| `offset_hz` | frequency | always | 1.0 | The carrier from the dial. |
| `quality` | number | always | 1.0 | The quality of the coherent soft bits (1 without noise, 0.64 for noise alone). |
| `data` | bytes | always | 1.0 | The user data, the first bit the high bit of the first byte. |

### Event `argos.downlink_frame`

Struct `ArgosDownlinkFrameEvent`. A downlink frame with a good FCS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `coding` | choice | always | 1.0 | The downlink coding in use. |
| `bytes` | whole number | always | 1.0 | The length of a frame without its FCS. |
| `frame` | bytes | always | 1.0 | A frame's bytes. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_argos_clear` | none | none |
| `remove` | `sdrkit_argos_remove` | `sdrkit_argos_platform_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `link` | `sdrkit_argos_get_link` | `sdrkit_argos_set_link` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_downlink` | `sdrkit_argos_gen_get_downlink` | `sdrkit_argos_gen_set_downlink` |
| `gen_gap_s` | `sdrkit_argos_gen_get_gap_s` | `sdrkit_argos_gen_set_gap_s` |
| `gen_step_hz` | `sdrkit_argos_gen_get_step_hz` | `sdrkit_argos_gen_set_step_hz` |
| `gen_other` | `sdrkit_argos_gen_get_other` | `sdrkit_argos_gen_set_other` |
| `gen_pass_doppler_hz` | `sdrkit_argos_gen_get_pass_doppler_hz` | `sdrkit_argos_gen_set_pass_doppler_hz` |
| `gen_pass_tau_s` | `sdrkit_argos_gen_get_pass_tau_s` | `sdrkit_argos_gen_set_pass_tau_s` |
| `gen_pass_tca_s` | `sdrkit_argos_gen_get_pass_tca_s` | `sdrkit_argos_gen_set_pass_tca_s` |
| `gen_bpsk` | `sdrkit_argos_gen_get_bpsk` | `sdrkit_argos_gen_set_bpsk` |
| `gen_nrzi` | `sdrkit_argos_gen_get_nrzi` | `sdrkit_argos_gen_set_nrzi` |

## Try it

```sh
sdrkit run argos --source synth:argos
sdrkit run argos --source iiod:192.168.2.1 --freq 401.65
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

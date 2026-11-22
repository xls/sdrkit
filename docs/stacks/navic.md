<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NavIC

The L5 SPS signal of NavIC (IRNSS): satellites tracked, C/N0, Doppler, subframes with the time of week count and the week number. No position. Experimental: the time count scale and the interleaver direction are not confirmed by a public document. Needs an active antenna (bias-tee on).

Stack id: `navic`. Group: Satellite navigation.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.17645 GHz |
| Tuning range | 1.1 GHz to 2.6 GHz |
| Sample rate | 4.096 MHz |
| Smallest sample rate | 2.2 MHz |
| Signal bandwidth | 3.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:navic` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_sats` | Satellites | whole number | 4 | 1 to 7 |  |
| `gen_sat_doppler_hz` | Satellite Doppler | number (Hz) | 2500 | -9000 to 9000 | Satellite i has this Doppler times 1, -0.5, 0.5, 0.75, 1, 1.25, 1.5. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `searching_prn` | Searching PRN | whole number |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `satellites` | Satellites | `prn` (whole number), `state` (whole number), `cn0_dbhz` (number (dB-Hz)), `doppler_hz` (frequency), `subframes_ok` (whole number), `subframes_bad` (whole number), `subframe_id` (whole number), `towc` (whole number), `wn` (whole number), `alert` (yes or no) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `navic.subframe` | NavIC subframe | `prn` (whole number), `subframe_id` (whole number), `towc` (whole number), `wn` (whole number), `message_id` (whole number), `prn_id` (whole number), `alert` (yes or no), `autonav` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `gnss`, version 1.1. The C header `sdrkit/gnss.h`, the registry record `registry/gnss.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Navigation aids (`navigation`). Media: none.

### Result fields

Struct `NavicFields`. The result fields of NavIC.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | whole number | always | 1.0 | 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | optional | 1.0 | Carrier offset from where it was expected, Hz. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `searching_prn` | whole number | always | 1.0 | The PRN being searched. |
| `note` | text | always | 1.0 | What is not confirmed by a public document. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `satellites`

Struct `NavicSatelliteRow`. A NavIC satellite.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `prn` | whole number | always | 1.0 | The PRN. |
| `state` | whole number | always | 1.0 | 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes. |
| `cn0_dbhz` | number (dB-Hz) | always | 1.0 | C/N0. |
| `doppler_hz` | frequency | always | 1.0 | Doppler. |
| `subframes_ok` | whole number | always | 1.0 | Subframes good. |
| `subframes_bad` | whole number | always | 1.0 | Subframes bad. |
| `subframe_id` | whole number | optional | 1.0 | The last subframe ID. |
| `towc` | whole number | optional | 1.0 | The time of week count. |
| `wn` | whole number | optional | 1.0 | The week number. |
| `alert` | yes or no | optional | 1.0 | The alert flag. |

Default order: the column `cn0_dbhz`, highest first.

### Event `navic.subframe`

Struct `NavicSubframeEvent`. A NavIC subframe.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `prn` | whole number | always | 1.0 | The PRN. |
| `subframe_id` | whole number | always | 1.0 | The last subframe ID. |
| `towc` | whole number | always | 1.0 | The time of week count. |
| `wn` | whole number | optional | 1.0 | The week number. |
| `message_id` | whole number | optional | 1.0 | The message ID. |
| `prn_id` | whole number | optional | 1.0 | The PRN ID. |
| `alert` | yes or no | always | 1.0 | The alert flag. |
| `autonav` | yes or no | always | 1.0 | The autonav flag. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_navic_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_sats` | `sdrkit_navic_gen_get_sats` | `sdrkit_navic_gen_set_sats` |
| `gen_sat_doppler_hz` | `sdrkit_navic_gen_get_sat_doppler_hz` | `sdrkit_navic_gen_set_sat_doppler_hz` |

## Try it

```sh
sdrkit run navic --source synth:navic
sdrkit run navic --source iiod:192.168.2.1 --freq 1176.45
```
<!-- generated: end -->

## Description

The stack searches the ranging codes of PRN 1 to 14 with the code search of the `gnss` stack, confirms each hit, and tracks up to 8 satellites on its tracking channels. A channel finds the bit edges, reads symbols of 20 ms, finds the subframe by its sync word EB90 in either polarity, removes the 8 by 73 interleaver, decodes the K = 7 code and checks the CRC-24Q. The default frequency is the L5 carrier of the ICD. The field `note` says what is not confirmed by a public document.

Standard: IRNSS Signal-in-Space ICD for Standard Positioning Service, version 1.1, ISRO.

## Settings: notes

Advanced settings. The owner works without any of them set.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# TV sat beacon decoder

The beacon carrier of a TV satellite through an LNB: carrier measurements and PCM telemetry frames.

Stack id: `satbeacon`. Group: TV.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 11.70525 GHz |
| Tuning range | 1 GHz to 20 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:satbeacon` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bitrate` | Bit rate | number (bit/s) | 4800 | 300 to 9600 |  |
| `gen_subcarrier_khz` | Subcarrier | number (kHz) | 48 | 5 to 100 |  |
| `gen_nrz` | NRZ instead of bi-phase | yes or no | false |  |  |
| `gen_carrier_only` | Carrier only | yes or no | false |  |  |
| `gen_pilot` | Pilot carrier | yes or no | true |  | A second carrier 545.642 kHz above the beacon, 8 dB below it. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `signal_time` | Signal time | duration (s) |
| `work_rate` | Work rate | frequency |
| `offset_hz` | Offset | frequency |
| `rf` | Carrier frequency | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `cn0_dbhz` | C/N0 | number (dB-Hz) |
| `drift_hz_min` | Drift | number (Hz/min) |
| `noise_density_dbfs` | Noise density | number (dBFS/Hz) |
| `carrier_locked` | Carrier locked | yes or no |
| `pm_rms_deg` | Phase modulation (rms) | number (degrees) |
| `subcarrier` | Subcarrier | frequency |
| `tones` | Subcarrier humps | whole number |
| `tone_spacing` | Hump spacing | frequency |
| `subcarrier_snr_db` | Subcarrier SNR | number (dB) |
| `bit_rate_bps` | Bit rate | number (bit/s) |
| `coding` | Coding | choice |
| `clock_locked` | Bit clock locked | yes or no |
| `frame_bits` | Frame length | whole number (bits) |
| `frame_agreement` | Frame agreement | number |
| `fixed_bits` | Fixed bits | whole number |
| `sync_word` | Sync word | choice |
| `counter_byte` | Counter byte | whole number |
| `counter_modulo` | Counter modulo | whole number |
| `bits` | Bits | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `template` | Template | bytes |
| `fixed_mask` | Fixed mask | bytes |
| `pilot_spacing` | Second carrier spacing | frequency |
| `lines` | Lines | whole number |
| `first_frames` | First frames found at | duration (s) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `lines` | Lines | `offset` (frequency), `level_dbfs` (number (dBFS)), `over_noise_db` (number (dB)) |
| `frames` | Frames | `frame` (whole number), `counter` (whole number), `errors` (whole number), `good` (yes or no), `bytes` (bytes) |
| `decoded` | Decoded | `name` (text), `kind` (choice), `value` (text), `bytes` (text), `confidence` (choice), `reason` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `rf_spectrum` | RF spectrum | bin | dB |
| `pm_spectrum` | Phase modulation spectrum | bin | dB |
| `column_stability` | Column stability | byte |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `beacon.frame` | Telemetry frame | `frame` (whole number), `counter` (whole number), `errors` (whole number), `good` (yes or no), `bytes` (bytes) |
| `beacon.carrier` | Beacon carrier | `signal_time` (duration (s)), `offset_hz` (frequency), `level_dbfs` (number (dBFS)), `cn0_dbhz` (number (dB-Hz)), `drift_hz_min` (number (Hz/min)), `bit_rate_bps` (number (bit/s)), `frame_bits` (whole number (bits)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the frames learned. |

## Typed interface

The stack is in the family `dvbcs`, version 1.5. The C header `sdrkit/dvbcs.h`, the registry record `registry/dvbcs.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

### Result fields

Struct `SatbeaconFields`. The result fields of `satbeacon`: the carrier, the subcarrier, the bit clock and the frame learner. A measurement of the subcarrier has no value without a subcarrier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | What the decoder does now. |
| `signal_time` | duration (s) | always | 1.0 | The signal time decoded. |
| `work_rate` | frequency | always | 1.0 | The work rate of the carrier loop. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the carrier from the centre. |
| `rf` | frequency | optional | 1.0 | The RF frequency of the carrier. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `cn0_dbhz` | number (dB-Hz) | always | 1.0 | The carrier to noise density. |
| `drift_hz_min` | number (Hz/min) | always | 1.0 | The drift of the carrier. |
| `noise_density_dbfs` | number (dBFS/Hz) | always | 1.0 | The noise density. |
| `carrier_locked` | yes or no | always | 1.0 | The carrier loop is locked. |
| `pm_rms_deg` | number (degrees) | always | 1.0 | The rms of the phase modulation. |
| `subcarrier` | frequency | optional | 1.0 | The subcarrier frequency. |
| `tones` | whole number | optional | 1.0 | The humps of the subcarrier spectrum. |
| `tone_spacing` | frequency | optional | 1.0 | Their spacing. |
| `subcarrier_snr_db` | number (dB) | optional | 1.0 | The subcarrier over the noise of the phase modulation. |
| `bit_rate_bps` | number (bit/s) | optional | 1.0 | The bit rate. No value before the bit clock is found. |
| `coding` | choice | optional | 1.0 | The line code. |
| `clock_locked` | yes or no | always | 1.0 | The bit clock is locked. |
| `frame_bits` | whole number (bits) | optional | 1.0 | The frame length. No value before the frames are found. |
| `frame_agreement` | number | always | 1.0 | The agreement of the frames with the template. |
| `fixed_bits` | whole number | always | 1.0 | Bits that do not change from frame to frame. |
| `sync_word` | choice | optional | 1.0 | The sync word. |
| `counter_byte` | whole number | optional | 1.0 | The byte of the frame counter. |
| `counter_modulo` | whole number | optional | 1.0 | The modulo of the frame counter. |
| `bits` | whole number | always | 1.0 | Bits received. |
| `frames_ok` | whole number | always | 1.0 | Frames good. |
| `frames_bad` | whole number | always | 1.0 | Frames bad. |
| `template` | bytes | always | 1.0 | The template of a frame. |
| `fixed_mask` | bytes | always | 1.0 | The mask of the fixed bits, one bit for each bit of the frame. |
| `pilot_spacing` | frequency | optional | 1.0 | The spacing of a second carrier above the beacon. |
| `lines` | whole number | always | 1.0 | The lines of the spectrum. |
| `first_frames` | duration (s) | optional | 1.0 | The signal time when the first frames were found. |

### Table `lines`

Struct `SatbeaconLineRow`. A row of the narrow lines of the spectrum, strongest first. The row key is the place.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset` | frequency | always | 1.0 | The offset of the line from the centre. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Its level. |
| `over_noise_db` | number (dB) | always | 1.0 | Its height over the noise. |

Default order: the column `level_dbfs`, highest first.

### Table `frames`

Struct `SatbeaconFrameRow`. A row of the frames received. The row key is the frame number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frame` | whole number | always | 1.0 | The frame number. |
| `counter` | whole number | optional | 1.0 | The frame counter. |
| `errors` | whole number | always | 1.0 | The errors against the template. |
| `good` | yes or no | always | 1.0 | The frame is good. |
| `bytes` | bytes | always | 1.0 | The bytes of the frame. |

### Table `decoded`

Struct `SatbeaconDecodedRow`. A row of the decoded view. The row key is the name and the bytes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the field. |
| `kind` | choice | always | 1.0 | What the line is. |
| `value` | text | always | 1.0 | Its value, for people. |
| `bytes` | text | always | 1.0 | Its bytes in the frame, for people. |
| `confidence` | choice | always | 1.0 | How sure the reading is. |
| `reason` | text | always | 1.0 | Why, for people. |

The column `name` names a row.

### Event `beacon.frame`

Struct `SatbeaconFrameEvent`. The fields of a telemetry frame, when it is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frame` | whole number | always | 1.0 | The frame number. |
| `counter` | whole number | optional | 1.0 | The frame counter. |
| `errors` | whole number | always | 1.0 | The errors against the template. |
| `good` | yes or no | always | 1.0 | The frame is good. |
| `bytes` | bytes | always | 1.0 | The bytes of the frame. |

### Event `beacon.carrier`

Struct `SatbeaconCarrierEvent`. The fields of the carrier measurements of one second of signal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time` | duration (s) | always | 1.0 | The signal time. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the carrier from the centre. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `cn0_dbhz` | number (dB-Hz) | always | 1.0 | The carrier to noise density. |
| `drift_hz_min` | number (Hz/min) | always | 1.0 | The drift of the carrier. |
| `bit_rate_bps` | number (bit/s) | optional | 1.0 | The bit rate. No value before the bit clock is found. |
| `frame_bits` | whole number (bits) | optional | 1.0 | The frame length. No value before the frames are found. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_satbeacon_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bitrate` | `sdrkit_satbeacon_gen_get_bitrate` | `sdrkit_satbeacon_gen_set_bitrate` |
| `gen_subcarrier_khz` | `sdrkit_satbeacon_gen_get_subcarrier_khz` | `sdrkit_satbeacon_gen_set_subcarrier_khz` |
| `gen_nrz` | `sdrkit_satbeacon_gen_get_nrz` | `sdrkit_satbeacon_gen_set_nrz` |
| `gen_carrier_only` | `sdrkit_satbeacon_gen_get_carrier_only` | `sdrkit_satbeacon_gen_set_carrier_only` |
| `gen_pilot` | `sdrkit_satbeacon_gen_get_pilot` | `sdrkit_satbeacon_gen_set_pilot` |

## Try it

```sh
sdrkit run satbeacon --source synth:satbeacon
sdrkit run satbeacon --source <driver>:<address> --freq 11705.25
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wave-buoy HF link (Datawell)

Datawell wave buoys on HF: heave, north and west, the sea state and the wave spectrum from the 2FSK HXV link (receive only).

Stack id: `wavebuoy`. Group: HF utility.

The wave-buoy HF link (Datawell Waverider), receive only: the 2FSK HXV format decoded, the 4FSK HVA format measured. ```text iq -> [front ~16 kHz] --+--> [carrier finder] -> wavebuoy.carrier --+--> [FSK demodulator] -> wavebuoy.symbol --+--> [HXV decoder] -> wavebuoy.vector --+ +---------------------------------------------+                                        +--> [frame analyser] -> wavebuoy.frame -+--> [results] ```

## Tuning

| Property | Value |
|---|---|
| Default frequency | 30.475 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 250 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 250 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wavebuoy` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `format` | Format | choice | `auto` | `auto` (automatic (from the bandwidth)), `2fsk` (2FSK), `4mfsk` (4MFSK) | A buoy that sends another format than the stack expects shows as a wrong-format signal. Use this choice to force one. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_signal` | Signal | choice | `2fsk` | `2fsk` (2FSK), `4fsk` (4FSK), `nothing` (Nothing), `carrier` (Carrier) |  |
| `gen_inverted_tones` | Inverted tones | yes or no | false |  |  |
| `gen_wrsg` | WR-SG | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `format` | Format | choice |
| `format_forced` | Format forced | yes or no |
| `signal_found` | Signal found | yes or no |
| `data_valid` | Data valid | yes or no |
| `offset_hz` | Offset | frequency |
| `frequency` | Frequency | frequency |
| `channel` | Channel | text |
| `peak_db` | Peak over the floor | number (dB) |
| `width` | Width (90 % of the power) | frequency |
| `snr_db` | SNR | number (dB) |
| `symbol_rate_hz` | Symbol rate | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `tone_step` | Tone step | frequency |
| `symbol_error_steps` | Symbol error | number (steps) |
| `symbol_count` | Symbols | whole number |
| `locked` | Locked | yes or no |
| `bits` | Bits | text |
| `hxv_locked` | HXV locked | yes or no |
| `hxv_inverted` | Inverted | yes or no |
| `vector_count` | Vectors | whole number |
| `clean_count` | Clean vectors | whole number |
| `repaired_count` | Repaired vectors | whole number |
| `lost_count` | Lost vectors | whole number |
| `block_count` | Blocks | whole number |
| `last_block` | Last block | whole number |
| `wrsg` | WR-SG | yes or no |
| `heave_cm` | Heave | whole number (cm) |
| `north_cm` | North | whole number (cm) |
| `west_cm` | West | whole number (cm) |
| `system_word_count` | System words received | whole number |
| `spectrum_point_count` | Spectrum points received | whole number |
| `tn` | Tn | whole number |
| `gps_ok` | GPS module ok | yes or no |
| `gps_fix` | GPS fix | yes or no |
| `hrms_m` | Hrms | number (m) |
| `hs_m` | Hs | number (m) |
| `fz` | fz | frequency |
| `tz_s` | Tz | number (s) |
| `psd_max_m2_hz` | PSDmax | number (m^2/Hz) |
| `tr_c` | Tr | number (°C) |
| `tw_c` | Tw (water) | number (°C) |
| `life_left_weeks` | Life left | whole number (weeks) |
| `battery_status` | Battery status | whole number |
| `av0_m_s2` | av0 | number (m/s^2) |
| `ax0_m_s2` | ax0 | number (m/s^2) |
| `ay0_m_s2` | ay0 | number (m/s^2) |
| `position` | Position | position |
| `orientation_deg` | Orientation | number (°) |
| `inclination_deg` | Inclination | number (°) |
| `period_bits` | Period | whole number (bits) |
| `period_z` | Period z | number |
| `sync` | Sync | text |
| `sync_start_bit` | Sync start | whole number (bit) |
| `crc_attempt` | CRC attempt | text |
| `crc_passes` | CRC passes | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `spectrum` | Spectrum | `entry` (whole number), `frequency` (frequency), `psd_m2_hz` (number (m^2/Hz)), `direction_deg` (number (°)), `spread_deg` (number (°)), `m2` (number), `n2` (number), `k` (number), `relative_psd` (number) |
| `system_words` | System words | `word` (whole number), `value` (whole number) |
| `frames` | Frames | `start` (duration (s)), `hex` (text), `accepted` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `scatter` | Symbol means | symbol | Hz |
| `heave` | Heave | sample | cm |
| `north` | North | sample | cm |
| `west` | West | sample | cm |
| `spectrum` | Spectrum | column |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wavebuoy.vector` | HXV vector | `heave_cm` (whole number (cm)), `north_cm` (whole number (cm)), `west_cm` (whole number (cm)), `cyclic` (whole number), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the bits, frames, vectors and system words. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `WavebuoyFields`. The result fields of the wave buoy stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `format` | choice | always | 1.0 | The link format. |
| `format_forced` | yes or no | always | 1.0 | The format is forced by the `format` setting. |
| `signal_found` | yes or no | always | 1.0 | The carrier finder sees a signal. |
| `data_valid` | yes or no | always | 1.0 | The data is valid: 2FSK with the HXV vectors locked, or 4FSK with the symbols locked. |
| `offset_hz` | frequency | optional | 1.0 | The centre of the signal about the tuned frequency. |
| `frequency` | frequency | optional | 1.0 | The absolute frequency of the signal centre. |
| `channel` | text | optional | 1.0 | The listed buoy channel within 6 kHz of the signal. |
| `peak_db` | number (dB) | optional | 1.0 | The peak of the spectrum over the floor. |
| `width` | frequency | optional | 1.0 | The width holding 90 % of the signal power. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR in 100 Hz. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The clock error. |
| `tone_step` | frequency | optional | 1.0 | The measured distance between the tones. |
| `symbol_error_steps` | number (steps) | optional | 1.0 | The RMS distance of the symbol means to the nearest tone, in tone steps. |
| `symbol_count` | whole number | always | 1.0 | Symbols demodulated since the last restart. |
| `locked` | yes or no | always | 1.0 | The symbols are locked. |
| `bits` | text | always | 1.0 | The newest 320 bits (higher tone = 1; 4FSK: the tone number as two bits). |
| `hxv_locked` | yes or no | always | 1.0 | The HXV vectors are locked. |
| `hxv_inverted` | yes or no | always | 1.0 | The tones are the other way round. |
| `vector_count` | whole number | always | 1.0 | Vectors accepted. |
| `clean_count` | whole number | always | 1.0 | Vectors accepted with no bit repaired. |
| `repaired_count` | whole number | always | 1.0 | Vectors accepted with 1 or 2 bits repaired. |
| `lost_count` | whole number | always | 1.0 | Vectors lost. |
| `block_count` | whole number | always | 1.0 | Blocks with a system word read. |
| `last_block` | whole number | optional | 1.0 | The system word number of the last block. |
| `wrsg` | yes or no | always | 1.0 | The last vector is a WR-SG vector (two vertical samples, west AAA). |
| `heave_cm` | whole number (cm) | optional | 1.0 | The heave of the last vector. |
| `north_cm` | whole number (cm) | optional | 1.0 | The north displacement of the last vector. |
| `west_cm` | whole number (cm) | optional | 1.0 | The west displacement of the last vector. |
| `system_word_count` | whole number | always | 1.0 | System file words received, of 16. |
| `spectrum_point_count` | whole number | always | 1.0 | Spectrum points received, of 64. |
| `tn` | whole number | optional | 1.0 | The buoy type number Tn (system word 0). |
| `gps_ok` | yes or no | optional | 1.0 | The GPS module works (system word 0). |
| `gps_fix` | yes or no | optional | 1.0 | The GPS has a fix (system word 0). |
| `hrms_m` | number (m) | optional | 1.0 | The RMS wave height (system word 1). |
| `hs_m` | number (m) | optional | 1.0 | The significant wave height, 4 Hrms (system word 1). |
| `fz` | frequency | optional | 1.0 | The zero-crossing frequency (system word 2). |
| `tz_s` | number (s) | optional | 1.0 | The zero-crossing period, 1 / fz (system word 2). |
| `psd_max_m2_hz` | number (m^2/Hz) | optional | 1.0 | The largest spectral density (system word 3). |
| `tr_c` | number (°C) | optional | 1.0 | The buoy temperature Tr (system word 4). |
| `tw_c` | number (°C) | optional | 1.0 | The water temperature Tw (system word 5). |
| `life_left_weeks` | whole number (weeks) | optional | 1.0 | The operational life left (system word 6). |
| `battery_status` | whole number | optional | 1.0 | The battery status, 0 to 7 (system word 6). |
| `av0_m_s2` | number (m/s^2) | optional | 1.0 | The vertical accelerometer offset (system word 7). |
| `ax0_m_s2` | number (m/s^2) | optional | 1.0 | The x accelerometer offset (system word 8). |
| `ay0_m_s2` | number (m/s^2) | optional | 1.0 | The y accelerometer offset (system word 9). |
| `position` | position | optional | 1.0 | The buoy position (system words 10 to 13). |
| `orientation_deg` | number (°) | optional | 1.0 | The orientation (system word 14). |
| `inclination_deg` | number (°) | optional | 1.0 | The inclination (system word 15). |
| `period_bits` | whole number (bits) | optional | 1.0 | The repetition period of the bit stream. |
| `period_z` | number | optional | 1.0 | How far the period stands out of random bits (z score). |
| `sync` | text | optional | 1.0 | The constant part of a period, in hex. |
| `sync_start_bit` | whole number (bit) | optional | 1.0 | The first bit of the constant part in the period. |
| `crc_attempt` | text | optional | 1.0 | The CRC or checksum that fits the frames (an attempt, never a verified Datawell CRC). |
| `crc_passes` | whole number | always | 1.0 | Frames the attempt passed in the last analysis. |
| `frames_ok` | whole number | always | 1.0 | Frames the CRC attempt accepts. |
| `frames_bad` | whole number | always | 1.0 | Frames the CRC attempt refuses. |

### Table `spectrum`

Struct `WavebuoySpectrumRow`. An entry of the directional spectrum received. The row key is the entry number, "00" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `entry` | whole number | always | 1.0 | The spectrum entry number, 0 to 63. |
| `frequency` | frequency | always | 1.0 | The frequency of the entry. |
| `psd_m2_hz` | number (m^2/Hz) | optional | 1.0 | The spectral density, once PSDmax is known. |
| `direction_deg` | number (°) | always | 1.0 | The mean wave direction. |
| `spread_deg` | number (°) | always | 1.0 | The directional spread. |
| `m2` | number | always | 1.0 | The second-order Fourier coefficient m2 (no unit). |
| `n2` | number | always | 1.0 | The second-order Fourier coefficient n2 (no unit). |
| `k` | number | always | 1.0 | The check factor K (no unit). |
| `relative_psd` | number | always | 1.0 | The density relative to PSDmax (no unit). |

### Table `system_words`

Struct `WavebuoySystemWordRow`. A system file word received. The row key is the word number, "00" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `word` | whole number | always | 1.0 | The system file word number, 0 to 15. |
| `value` | whole number | always | 1.0 | The 12-bit value of the word. |

### Table `frames`

Struct `WavebuoyFrameRow`. A frame cut at the sync. The row key is the frame's number in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time of the frame. |
| `hex` | text | always | 1.0 | The bits of the frame in hex. |
| `accepted` | yes or no | always | 1.0 | The CRC attempt accepts the frame. |

### Event `wavebuoy.vector`

Struct `WavebuoyVectorEvent`. An accepted HXV vector.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `heave_cm` | whole number (cm) | always | 1.0 | The heave of the vector. |
| `north_cm` | whole number (cm) | always | 1.0 | The north displacement of the vector. |
| `west_cm` | whole number (cm) | always | 1.0 | The west displacement of the vector. |
| `cyclic` | whole number | always | 1.0 | The cyclic data word of the vector. |
| `family` | choice | always | 1.0 | What the event is about: a sensor. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_wavebuoy_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `format` | `sdrkit_wavebuoy_get_format` | `sdrkit_wavebuoy_set_format` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_signal` | `sdrkit_wavebuoy_gen_get_signal` | `sdrkit_wavebuoy_gen_set_signal` |
| `gen_inverted_tones` | `sdrkit_wavebuoy_gen_get_inverted_tones` | `sdrkit_wavebuoy_gen_set_inverted_tones` |
| `gen_wrsg` | `sdrkit_wavebuoy_gen_get_wrsg` | `sdrkit_wavebuoy_gen_set_wrsg` |

## Try it

```sh
sdrkit run wavebuoy --source synth:wavebuoy
sdrkit run wavebuoy --source <driver>:<address> --freq 30.475
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

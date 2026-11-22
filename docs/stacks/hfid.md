<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HF waveform ID

Identification only: measures an HF data signal's physical layer and ranks the waveforms it could be (estimates).

Stack id: `hfid`. Group: HF utility.

The HF waveform identifier, identification only. It measures the physical layer of an HF signal in the upper sideband and ranks candidate waveforms by how well the measurements fit their published parameters. Nothing is decoded.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 4.625 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 6 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hfid` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_calls` | Calls | choice | `test_calls` | `test_calls` (The test calls), `sounding` (The sounding only) |  |
| `gen_ber` | Channel bit errors | number | 0 | 0 to 0.2 | The share of channel bits turned over before they become tones. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `signal` | Signal | yes or no |
| `best` | Best estimate | choice |
| `best_confidence_pct` | Confidence (estimate) | number (%) |
| `analyses` | Analyses | whole number |
| `band_low` | Band from | frequency |
| `band_high` | Band to | frequency |
| `snr_db` | SNR | number (dB) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `carrier_line` | Carrier line | frequency |
| `carrier_line_db` | Carrier line level | number (dB) |
| `tone_605_db` | Tone at 605 Hz | number (dB) |
| `tone_2915` | Tone at 2915 Hz | yes or no |
| `comb_spacing` | Tone spacing | frequency |
| `comb_regularity_pct` | Comb regularity | number (%) |
| `comb_tones` | Tones | whole number |
| `shift` | Shift | frequency |
| `symbol_rate_hz` | Symbol rate | frequency |
| `clock_line_db` | Clock line | number (dB) |
| `envelope_modulation` | Envelope modulation | frequency |
| `envelope_modulation_db` | Envelope modulation level | number (dB) |
| `bursts` | Bursts | whole number |
| `duty_pct` | On | number (%) |
| `order` | Constellation order | whole number |
| `repeat_symbols` | Repetition (symbols) | whole number |
| `repeat_period` | Repetition | duration (s) |
| `repeat_correlation_pct` | Repetition correlation | number (%) |
| `measured` | Measured parameters | text |
| `level_dbfs` | Level | number (dBFS) |
| `audio_rate` | Audio rate | frequency |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `candidates` | Candidates | `waveform` (choice), `confidence_pct` (number (%)), `basis` (text), `note` (text), `link` (text) |

## Typed interface

The stack is in the family `hfdata`, version 1.5. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

### Result fields

Struct `HfidFields`. The result fields of the HF signal identifier: the measures of the last analysis.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal` | yes or no | always | 1.0 | A signal was found by the last analysis. |
| `best` | choice | optional | 1.0 | The best candidate. |
| `best_confidence_pct` | number (%) | optional | 1.0 | The confidence of the best candidate, 0 to 95 %. |
| `analyses` | whole number | always | 1.0 | The analyses run. |
| `band_low` | frequency | optional | 1.0 | The lower edge of the occupied band. |
| `band_high` | frequency | optional | 1.0 | The upper edge of the occupied band. |
| `snr_db` | number (dB) | optional | 1.0 | The signal-to-noise ratio in the band. Absent without a signal. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | The noise floor. |
| `carrier_line` | frequency | optional | 1.0 | The steady carrier line. |
| `carrier_line_db` | number (dB) | optional | 1.0 | The carrier line over its surroundings. |
| `tone_605_db` | number (dB) | optional | 1.0 | The 605 Hz tone over its surroundings. |
| `tone_2915` | yes or no | always | 1.0 | A tone at 2915 Hz. |
| `comb_spacing` | frequency | optional | 1.0 | The tone comb spacing. |
| `comb_regularity_pct` | number (%) | optional | 1.0 | The regularity of the comb, 0 to 100 %. |
| `comb_tones` | whole number | optional | 1.0 | The tones of the comb. |
| `shift` | frequency | optional | 1.0 | The two-tone shift. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate at the clock line. |
| `clock_line_db` | number (dB) | optional | 1.0 | The clock line over its surroundings. |
| `envelope_modulation` | frequency | optional | 1.0 | The slow envelope modulation. |
| `envelope_modulation_db` | number (dB) | optional | 1.0 | The envelope modulation line over its surroundings. |
| `bursts` | whole number | optional | 1.0 | The bursts in the analysed audio (0: continuous). |
| `duty_pct` | number (%) | optional | 1.0 | The part of the time with a burst on. |
| `order` | whole number | optional | 1.0 | The constellation order at the clock: 2, 4, 8, or 99 for higher or unclear. |
| `repeat_symbols` | whole number | optional | 1.0 | The repetition period in symbols. |
| `repeat_period` | duration (s) | optional | 1.0 | The repetition period. |
| `repeat_correlation_pct` | number (%) | optional | 1.0 | The correlation at the repetition period, 0 to 100 %. |
| `measured` | text | always | 1.0 | The measured parameters as lines of text. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the audio, dBFS. |
| `audio_rate` | frequency | always | 1.0 | The audio rate of the analysis. |

### Table `candidates`

Struct `HfidCandidateRow`. A candidate of the last analysis of the identifier. The row key is the place, "0" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `waveform` | choice | always | 1.0 | The waveform. |
| `confidence_pct` | number (%) | always | 1.0 | The confidence of the estimate, 0 to 95 %. |
| `basis` | text | always | 1.0 | What the score is based on. |
| `note` | text | always | 1.0 | What the waveform is. |
| `link` | text | always | 1.0 | A public source. |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_calls` | `sdrkit_hfid_gen_get_calls` | `sdrkit_hfid_gen_set_calls` |
| `gen_ber` | `sdrkit_hfid_gen_get_ber` | `sdrkit_hfid_gen_set_ber` |

## Try it

```sh
sdrkit run hfid --source synth:hfid
sdrkit run hfid --source <driver>:<address> --freq 4.625
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CIS HF data signals

CIS HF signals: BEE messages (2-FSK) as hex, CIS-14 frames raw, the CIS-12 phase steps; CIS-36, the CIS OFDM signals, CIS-128 and CIS-3000 identified only.

Stack id: `cis_hf`. Group: HF utility.

CIS HF data signals: seven signals in one stack, chosen by the submode. `bee` (CIS-36-50 and CIS-50-50, 2-FSK) reads the message starts, the session key and the 3:4 characters as hex; `cis14` (CIS-14) shows its 14-bit frames raw; `cis12` (CIS-12) reads the phase steps of its 12 DPSK carriers. `cis36` (CIS-36, 36-tone MFSK), `ofdm_hdr` (CIS-45, CIS-60, CIS-93), `cis128` (CIS-128) and `cis3000` (CIS-3000, 8-PSK at 3000 Bd) are identified only.

Standard: no public document of an issuing body describes these signals.

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
| `bee` | CIS-36-50 / CIS-50-50 (BEE) | 6.5 MHz |
| `cis14` | CIS-14 | 6.5 MHz |
| `cis12` | CIS-12 | 6.5 MHz |
| `cis36` | CIS-36 (identification) | 6.5 MHz |
| `ofdm_hdr` | CIS OFDM (identification) | 6.5 MHz |
| `cis128` | CIS-128 (identification) | 6.5 MHz |
| `cis3000` | CIS-3000 (identification) | 6.5 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cis_hf` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Signal | choice | `bee` | `bee` (CIS-36-50 / CIS-50-50 (BEE)), `cis14` (CIS-14), `cis12` (CIS-12), `cis36` (CIS-36 (identification)), `ofdm_hdr` (CIS OFDM (identification)), `cis128` (CIS-128 (identification)), `cis3000` (CIS-3000 (identification)) |  |
| `baud` | Speed | number (Bd) | 0 | 0 to 300 | bee and cis14: 0 tries every candidate speed; a speed replaces them. |
| `shift_hz` | Shift | frequency | 0 Hz | 0 Hz to 1.2 kHz | bee and cis14: 0 finds it from the signal. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `roll_off` | CIS-3000 roll-off | number | 0.15 | 0.05 to 1 | The pulse of CIS-3000 is not published; a root-raised cosine of this roll-off is assumed. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_variant` | BEE variant | choice | `36-50` | `36-50` (CIS-36-50), `50-100` (CIS-50-50) |  |
| `gen_lower_one` | BEE 1 on the lower tone | yes or no | true |  |  |
| `gen_centre_hz` | Centre | frequency | 0 Hz | 0 Hz to 5 kHz | 0 takes 1700 Hz for BEE and 1500 Hz for CIS-14. |
| `gen_shift_hz` | CIS-14 shift | frequency | 200 Hz | 60 Hz to 1.1 kHz |  |
| `gen_order` | CIS-12 order | choice | `4` | `2` (DBPSK), `4` (DQPSK) |  |
| `gen_rate` | CIS-36 tone rate | choice | `20` | `10` (10 Bd), `20` (20 Bd), `40` (40 Bd) |  |
| `gen_ofdm` | OFDM signal | choice | `cis45_40` | `cis45_40` (CIS-45 (40 Bd)), `cis45_33` (CIS-45 (33.3 Bd)), `cis60` (CIS-60), `cis93` (CIS-93) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `state` | State | whole number |
| `found` | Found | text |
| `identified` | Identified | text |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `centre_hz` | Centre | number (Hz) |
| `shift_hz` | Shift | number (Hz) |
| `eye` | Eye | number |
| `frames` | Frames | whole number |
| `state_bits` | State bits | text |
| `pilot_hz` | Pilot | number (Hz) |
| `carriers` | Carriers seen | whole number |
| `symbol_rate` | Symbol rate | number (Bd) |
| `modulation` | Modulation | text |
| `m2` | M2 | number |
| `m4` | M4 | number |
| `m8` | M8 | number |
| `tones` | Tones seen | whole number |
| `spacing_hz` | Spacing | number (Hz) |
| `low_hz` | Lowest | number (Hz) |
| `high_hz` | Highest | number (Hz) |
| `span_hz` | Span | number (Hz) |
| `double_gaps` | Double gaps | whole number |
| `useful_ms` | Useful part | number (ms) |
| `guard_ms` | Guard | number (ms) |
| `centre_off` | Centre carrier off | yes or no |
| `pilot_3300` | Pilot 3300 Hz | yes or no |
| `line2_db` | Line of the 2nd power | number (dB) |
| `line4_db` | Line of the 4th power | number (dB) |
| `line8_db` | Line of the 8th power | number (dB) |
| `rate_line_db` | Symbol-rate line | number (dB) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `time` (time), `speed` (number (Bd)), `start_word` (text), `session_key` (text), `characters` (whole number), `failing` (whole number), `ended_by` (text) |
| `carriers` | Carriers | `number` (whole number), `freq_hz` (number (Hz)), `on` (yes or no) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `text` | Text | `text` (text) |
| `cis.message` | Message | `text` (text), `start_word` (text), `session_key` (text), `characters` (whole number), `failing` (whole number) |
| `cis.ident` | Identified | `text` (text), `identified` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `CisHfFields`. The result fields of the CIS stack. A field that the submode in use does not measure has no value.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `text` | text | always | 1.0 | The text received, the last characters. |
| `state` | whole number | always | 1.0 | The search state: 0 searching, 1 signal found, 2 decoding (or identified). |
| `found` | text | always | 1.0 | What was found, in one line. |
| `identified` | text | always | 1.0 | The name of the signal identified, or empty. |
| `good` | whole number | always | 1.0 | Characters, words or frames read. |
| `bad` | whole number | always | 1.0 | Characters, words or frames that failed their check. |
| `offset_hz` | frequency | optional | 1.0 | The frequency error. Absent without a measure. |
| `snr_db` | number (dB) | optional | 1.0 | The signal-to-noise ratio. Absent without a measure. |
| `centre_hz` | number (Hz) | optional | 1.0 | The centre of the signal above the dial (2-FSK and CIS-3000). |
| `shift_hz` | number (Hz) | optional | 1.0 | The shift between the tones (2-FSK). |
| `eye` | number | optional | 1.0 | The eye opening of the 2-FSK demodulators. |
| `frames` | whole number | optional | 1.0 | Frames read (CIS-14). |
| `state_bits` | text | optional | 1.0 | The state bits of the last frames (CIS-14), raw. |
| `pilot_hz` | number (Hz) | optional | 1.0 | The pilot (CIS-12). |
| `carriers` | whole number | optional | 1.0 | Carriers seen (CIS-12, OFDM). |
| `symbol_rate` | number (Bd) | optional | 1.0 | The symbol rate. |
| `modulation` | text | optional | 1.0 | The modulation found. |
| `m2` | number | optional | 1.0 | The moment of the 2nd power. |
| `m4` | number | optional | 1.0 | The moment of the 4th power. |
| `m8` | number | optional | 1.0 | The moment of the 8th power. |
| `tones` | whole number | optional | 1.0 | Tones seen (CIS-36). |
| `spacing_hz` | number (Hz) | optional | 1.0 | The spacing of the tones or carriers. |
| `low_hz` | number (Hz) | optional | 1.0 | The lowest tone or band edge. |
| `high_hz` | number (Hz) | optional | 1.0 | The highest tone or band edge. |
| `span_hz` | number (Hz) | optional | 1.0 | The span of the tones (CIS-36). |
| `double_gaps` | whole number | optional | 1.0 | Double gaps between tones (CIS-36). |
| `useful_ms` | number (ms) | optional | 1.0 | The useful part of an OFDM symbol. |
| `guard_ms` | number (ms) | optional | 1.0 | The guard of an OFDM symbol. |
| `centre_off` | yes or no | optional | 1.0 | The centre carrier of the OFDM signal is off. |
| `pilot_3300` | yes or no | optional | 1.0 | A pilot at 3300 Hz is there. |
| `line2_db` | number (dB) | optional | 1.0 | The spectral line of the 2nd power. |
| `line4_db` | number (dB) | optional | 1.0 | The spectral line of the 4th power. |
| `line8_db` | number (dB) | optional | 1.0 | The spectral line of the 8th power. |
| `rate_line_db` | number (dB) | optional | 1.0 | The spectral line at the symbol rate. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `CisHfMessageRow`. A `bee` message of CIS-36-50. The row key is the number of the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | always | 1.0 | The time the message ended. |
| `speed` | number (Bd) | always | 1.0 | The speed of the message. |
| `start_word` | text | always | 1.0 | The start word. |
| `session_key` | text | always | 1.0 | The session key. |
| `characters` | whole number | always | 1.0 | Characters read. |
| `failing` | whole number | always | 1.0 | Characters that failed the 3:4 check. |
| `ended_by` | text | always | 1.0 | What ended the message. |

### Table `carriers`

Struct `CisHfCarrierRow`. A carrier of CIS-12. The row key is the number of the carrier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `number` | whole number | always | 1.0 | The carrier number (1 to 12). |
| `freq_hz` | number (Hz) | always | 1.0 | The carrier frequency. |
| `on` | yes or no | always | 1.0 | The carrier is on. |

### Event `text`

Struct `TextEvent`. New text: a line, 64 characters, or what came before a pause.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text. |

### Event `cis.message`

Struct `CisMessageEvent`. A `bee` message of CIS-36-50.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `start_word` | text | always | 1.0 | The start word. |
| `session_key` | text | always | 1.0 | The session key. |
| `characters` | whole number | always | 1.0 | Characters read. |
| `failing` | whole number | always | 1.0 | Characters that failed the 3:4 check. |

### Event `cis.ident`

Struct `CisIdentEvent`. A signal was identified.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `identified` | text | always | 1.0 | The name of the signal identified. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_cis_hf_clear` | none | none |
| `reset` | `sdrkit_cis_hf_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_cis_hf_get_submode` | `sdrkit_cis_hf_set_submode` |
| `baud` | `sdrkit_cis_hf_get_baud` | `sdrkit_cis_hf_set_baud` |
| `shift_hz` | `sdrkit_cis_hf_get_shift_hz` | `sdrkit_cis_hf_set_shift_hz` |
| `roll_off` | `sdrkit_cis_hf_get_roll_off` | `sdrkit_cis_hf_set_roll_off` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_variant` | `sdrkit_cis_hf_gen_get_variant` | `sdrkit_cis_hf_gen_set_variant` |
| `gen_lower_one` | `sdrkit_cis_hf_gen_get_lower_one` | `sdrkit_cis_hf_gen_set_lower_one` |
| `gen_centre_hz` | `sdrkit_cis_hf_gen_get_centre_hz` | `sdrkit_cis_hf_gen_set_centre_hz` |
| `gen_shift_hz` | `sdrkit_cis_hf_gen_get_shift_hz` | `sdrkit_cis_hf_gen_set_shift_hz` |
| `gen_order` | `sdrkit_cis_hf_gen_get_order` | `sdrkit_cis_hf_gen_set_order` |
| `gen_rate` | `sdrkit_cis_hf_gen_get_rate` | `sdrkit_cis_hf_gen_set_rate` |
| `gen_ofdm` | `sdrkit_cis_hf_gen_get_ofdm` | `sdrkit_cis_hf_gen_set_ofdm` |

## Try it

```sh
sdrkit run cis_hf --source synth:cis_hf
sdrkit run cis_hf --source synth:cis_hf --submode bee
sdrkit run cis_hf --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The channel is the 24 kHz channel of the family front end decimated by 2 (a 63-tap half-band) to 12 000 samples/s, so the receivers read 100 to 5900 Hz above the dial. The measures share an averaged spectrum of 4096 points (the noise floor is its median bin). `bee`: the tone pair with a lock that follows it, one demodulator a speed (36, 50, 75, 100, 150 Bd) with its own symbol clock, and per demodulator a framer: a start word of 44 bits with 20 to 22 ones, the 70-bit key twice (at most 2 bits differ, 8 of 10 key characters valid), then 7-bit characters with three 1 bits shown as hex, ended by three characters 77; a follow-on message needs no start. `cis14`: 96 Bd, frames of 14 bits aligned on the two state bits that stay constant, each printed as state bits, word a, word b and parity bits. `cis12`: the 3300 Hz pilot gives the frequency error, the carriers at 700 + 200 k Hz are found in the spectrum, the symbol clock comes from the energy of the 5 ms carrier integrations, and each phase step is a digit; the PSK order comes from the moments of the steps. `cis36`: the strongest tone of 25 ms frames, the histogram of the tones, their spacing and the tone rate. `ofdm_hdr` and `cis128`: the useful length from the autocorrelation, the symbol rate from the guard correlation, the carriers and their PSK order; the names come from the published useful length and rate. `cis3000`: the symbol-rate line of the envelope and the lines of the 2nd, 4th and 8th powers after a root-raised-cosine matched filter. Four submodes identify the signal only; nothing is decrypted.

Standard: No document of an issuing body describes these signals. The BEE start word bits, the key repetition, the end character, the CIS-14 parity rule and state codes, the CIS-12 pulse shape and frames, the CIS-36 tone groups, the CIS-128 spacing and guard and the CIS-3000 pulse are not confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv`, for the host to offer.

## Try it: notes

```sh
sdrkit run cis_hf --source synth:cis_hf --seconds 26
sdrkit run cis_hf --source synth:cis_hf/cis12 --submode cis12
sdrkit run cis_hf --source <driver>:<address> --freq 8
```

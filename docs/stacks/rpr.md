<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Robust Packet (physical layer)

Robust Packet physical layer: the eight-carrier comb, the bursts, D-BPSK or D-QPSK and the phase steps (the coding is not public; no frames).

Stack id: `rpr`. Group: HF utility.

Robust Packet: the physical layer only, receive only. The waveform is eight differential PSK carriers 60 Hz apart around 1500 Hz on upper-sideband audio, 50 Bd on every carrier, D-BPSK at speed level 1 and D-QPSK at speed level 2 (the maker's data sheet). The convolutional code, its puncturing, the scrambler, the interleaver, the CRC and the preamble content are not public, so nothing is decoded: the stack finds the comb, the bursts and the symbol clock, measures them, and gives the differential phase steps of every carrier.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 10.1473 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:rpr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `analysis_interval_s` | Analysis interval | duration (s) | 4 s | 0.5 to 60 |  |
| `audio_centre_hz` | Audio centre | frequency | 1.5 kHz | 1 kHz to 2.6 kHz | The audio frequency of the middle of the eight carriers when the dial is right. |
| `sideband` | Sideband | choice | `upper` | `upper` (Upper), `lower` (Lower) | The sideband the signal is sent on. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_speed` | Speed level | choice | `level1` | `level1` (Level 1 (D-BPSK)), `level2` (Level 2 (D-QPSK)) |  |
| `gen_gap_s` | Gap between bursts | duration (s) | 1.5 s | 0.1 to 30 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `comb_centre` | Comb centre | frequency |
| `offset_hz` | Offset | frequency |
| `comb_db` | Comb strength | number (dB) |
| `carrier_count` | Carriers seen | whole number |
| `spacing` | Spacing | frequency |
| `symbol_rate_hz` | Symbol rate | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `speed_level` | Speed level | whole number |
| `modulation` | Modulation | choice |
| `net_rate` | Net rate | whole number (bit/s) |
| `snr_db` | SNR | number (dB) |
| `best_snr_db` | Best SNR | number (dB) |
| `burst_length` | Last burst | duration (s) |
| `symbols_1` | Steps, carrier 0 | text |
| `symbols_2` | Steps, carrier 1 | text |
| `symbols_3` | Steps, carrier 2 | text |
| `note` | Note | text |
| `analyses` | Analyses | whole number |
| `detections` | Analyses with the comb | whole number |
| `bursts` | Bursts | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `clock` | Clock | choice |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carriers | `frequency` (frequency), `concentration_pct` (number (%)) |
| `bursts` | Bursts | `start` (duration (s)), `time` (time), `length` (duration (s)), `speed_level` (whole number), `modulation` (choice), `net_rate` (whole number (bit/s)), `carriers` (whole number), `symbols` (whole number), `snr_db` (number (dB)), `offset` (frequency), `first_steps` (text) |
| `events` | Events | `start` (duration (s)), `time` (time), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level` | Level | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `rpr.burst` | Robust Packet burst | `speed_level` (whole number), `modulation` (choice), `carriers` (whole number), `symbols` (whole number), `length` (duration (s)), `snr_db` (number (dB)), `offset_hz` (frequency), `steps` (bytes), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `hfdata`, version 2.0. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

Key readings: `state`, `speed_level`, `net_rate`, `snr_db`.

### Result fields

Struct `RprFields`. The result fields of the Robust Packet stack: the comb of the last analysis and the last burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | What the last analysis found: nothing, the comb, or bursts. |
| `comb_centre` | frequency | optional | 1.0 | The centre of the comb in the audio. Absent without a comb. |
| `offset_hz` | frequency | optional | 1.0 | The comb centre against the audio centre setting. |
| `comb_db` | number (dB) | optional | 1.0 | The mean power inside the comb over the mean power beside it. |
| `carrier_count` | whole number | always | 1.0 | The carriers seen: those of the last burst with a clear step pattern, else the comb cells that stand out. |
| `spacing` | frequency | optional | 1.0 | The carrier spacing measured on the last burst. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate of the last burst, at the peak of its clock line. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The clock error of the last burst of more than 150 symbols. |
| `speed_level` | whole number | always | 1.0 | The speed level of the last burst: 1 (D-BPSK) or 2 (D-QPSK); 0 before a burst. |
| `modulation` | choice | optional | 1.0 | The modulation of the last burst. |
| `net_rate` | whole number (bit/s) | optional | 1.0 | The net rate of the speed level before the AX.25 overhead: 200 or 600 bit/s. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last burst, from the phase error of its steps. |
| `best_snr_db` | number (dB) | optional | 1.0 | The best SNR of a burst since the last Clear. |
| `burst_length` | duration (s) | optional | 1.0 | The length of the last burst. |
| `symbols_1` | text | always | 1.0 | The newest step indices of carrier 0, one digit per symbol. |
| `symbols_2` | text | always | 1.0 | The newest step indices of carrier 1. |
| `symbols_3` | text | always | 1.0 | The newest step indices of carrier 2. |
| `note` | text | always | 1.0 | What the stack does not do. |
| `analyses` | whole number | always | 1.0 | The analyses run. |
| `detections` | whole number | always | 1.0 | The analyses that found the comb. |
| `bursts` | whole number | always | 1.0 | The bursts demodulated since the last Clear. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The audio level of the last second. |
| `clock` | choice | always | 1.0 | Where the times come from. |

### Table `carriers`

Struct `RprCarrierRow`. A carrier of the last burst. The row key is the carrier number, "0" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The carrier frequency in the audio: its place in the comb and its measured turn per symbol. |
| `concentration_pct` | number (%) | always | 1.0 | The concentration of its phase steps for the order of the burst, 0 to 100 %. |

Default order: the column `frequency`, lowest first.

### Table `bursts`

Struct `RprBurstRow`. A burst demodulated. The row key is the sequence number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time of its start. |
| `time` | time | optional | 1.0 | The UTC of its start, when the clock is known. |
| `length` | duration (s) | always | 1.0 | Its length. |
| `speed_level` | whole number | always | 1.0 | The speed level: 1 or 2. |
| `modulation` | choice | always | 1.0 | The modulation. |
| `net_rate` | whole number (bit/s) | always | 1.0 | The net rate of the speed level: 200 or 600 bit/s. |
| `carriers` | whole number | always | 1.0 | The carriers with a clear step pattern. |
| `symbols` | whole number | always | 1.0 | The symbols demodulated on each carrier. |
| `snr_db` | number (dB) | always | 1.0 | The SNR from the phase error of its steps. |
| `offset` | frequency | always | 1.0 | The comb centre against the audio centre setting. |
| `first_steps` | text | always | 1.0 | The step indices of the first 24 symbol times as hexadecimal: carrier 0 to 7 in turn, 1 bit (D-BPSK) or 2 bits (D-QPSK) each, most significant first. The bits are still coded and scrambled. |

The rows are a log: they arrive over time. Time: `time`.

### Table `events`

Struct `RprEventRow`. An event of the Robust Packet stack. The row key is the sequence number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time. |
| `time` | time | optional | 1.0 | The UTC, when the clock is known. |
| `text` | text | always | 1.0 | What happened. |

The rows are a log: they arrive over time. Time: `time`.

### Event `rpr.burst`

Struct `RprBurstEvent`. A burst demodulated, with its whole stream of phase steps.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `speed_level` | whole number | always | 1.0 | The speed level: 1 or 2. |
| `modulation` | choice | always | 1.0 | The modulation. |
| `carriers` | whole number | always | 1.0 | The carriers with a clear step pattern. |
| `symbols` | whole number | always | 1.0 | The symbols demodulated on each carrier. |
| `length` | duration (s) | always | 1.0 | The length of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The SNR from the phase error of its steps. |
| `offset_hz` | frequency | always | 1.0 | The comb centre against the audio centre setting. |
| `steps` | bytes | always | 1.0 | The step indices, one byte each (0 or 1 for D-BPSK, 0 to 3 for D-QPSK; the step is index x 360 / order degrees after the constant turn of the carrier): for each symbol time after the first, carrier 0 to 7 in turn. |
| `text` | text | always | 1.0 | The burst in words. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_rpr_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `analysis_interval_s` | `sdrkit_rpr_get_analysis_interval_s` | `sdrkit_rpr_set_analysis_interval_s` |
| `audio_centre_hz` | `sdrkit_rpr_get_audio_centre_hz` | `sdrkit_rpr_set_audio_centre_hz` |
| `sideband` | `sdrkit_rpr_get_sideband` | `sdrkit_rpr_set_sideband` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_speed` | `sdrkit_rpr_gen_get_speed` | `sdrkit_rpr_gen_set_speed` |
| `gen_gap_s` | `sdrkit_rpr_gen_get_gap_s` | `sdrkit_rpr_gen_set_gap_s` |

## Try it

```sh
sdrkit run rpr --source synth:rpr
sdrkit run rpr --source <driver>:<address> --freq 10.1473
```
<!-- generated: end -->

## Description

Robust Packet is an HF packet waveform: eight differential PSK carriers 60 Hz apart around 1500 Hz of upper-sideband audio
(1290 to 1710 Hz, 500 Hz wide), 50 Bd on every carrier. Speed level 1 is D-BPSK with a rate 1/2 code (200 bit/s before the
AX.25 overhead); speed level 2 is D-QPSK with a rate 3/4 code (600 bit/s). These facts are in the maker's data sheet.

The stack is a physical-layer receiver and measurer. It decodes no frames and no text: the code polynomials, the
puncturing, the scrambler, the interleaver, the CRC, the preamble content and the phase convention are not public. The
status says so.

Every 4 s the stack analyses the newest 30 s of 8 kHz audio (the sideband without AGC):

1. It finds the comb of eight carriers within 500 Hz of the audio centre, and measures its centre, its offset and its
   strength.
2. It finds the bursts in the summed power of the carriers. A burst is listed when it has closed, once.
3. For each burst it places the symbol centres on the 50 Hz line of the carrier power, takes the differential phase step
   of every carrier, and decides D-BPSK or D-QPSK from the second and fourth powers of the steps.
4. It removes the constant turn of each carrier and decides each step: index 0 to 1 (D-BPSK) or 0 to 3 (D-QPSK), the step
   being index x 360 / order degrees. The index is relative to the observed axes.

A burst is listed when at least 5 carriers show a clear step pattern, at least 6 carriers carry power within 10 dB of
the strongest, and the 50 Hz clock line is clear. FSK bursts (PACTOR-I) and continuous signals are not listed.

Measured: the comb centre and offset, the comb strength, the carriers seen, the carrier spacing (from the turn of each
carrier), the symbol rate (from the clock line), the clock error of bursts of more than 150 symbols, the modulation and the
net rate class, the SNR from the phase error of the steps, and the length of each burst.

The `rpr.burst` event holds the whole stream of step indices of a burst in `steps`, one byte per step: for each symbol time
after the first, carrier 0 to 7 in turn. The `first_steps` column of the bursts table shows the first 24 symbol times as
hexadecimal. These bits are still coded and scrambled.

The test signal sends bursts of random steps on the waveform of the specification: 3 ramp symbols, 8 preamble symbols per
carrier (a fixed pattern of the test signal; the real preamble is not public), 60 data symbols per carrier and 3 ramp
symbols, with a gap of silence between bursts. A symbol is a guard of 3.33 ms, over which the phase fades from the previous
symbol, and an active part of 1/60 s. These bursts are not Robust Packet frames. The symbol shape is an assumption: the data
sheet says only "pulse-shaped".

## Tuning: notes

The stack holds no list of frequencies. The usual dials (upper sideband) are in `data/plans/rpr.json`: 10.1473 MHz, and
3.610, 7.0473 and 14.1033 MHz without a named source. The carriers lie 1290 to 1710 Hz above the dial.

## Settings: notes

`audio_centre_hz` is where the middle of the carriers lies when the dial is right; the offset is measured from it.
`sideband` selects the upper or lower sideband. `gen_speed` and `gen_gap_s` set the test signal.

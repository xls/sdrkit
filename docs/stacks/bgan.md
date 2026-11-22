<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Inmarsat BGAN

Inmarsat BGAN forward carriers of the L band: every carrier found and classed, frames and FEC blocks decoded with the published tables, and the broadcast signalling read (bulletin boards, networks, satellites, spot beams, return schedules). Broadcast signalling only: no user data. Synthetic signals only. Receive only.

Stack id: `bgan`. Group: Satellite.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.545 GHz |
| Tuning range | 1.518 GHz to 1.559 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 600 kHz |
| Signal bandwidth | 1.6 MHz |
| Radio filter | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:bgan` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `decode_all` | Decode every FEC block | yes or no | true |  | Off: block 0 of each frame only. |
| `bearer` | Bearer | choice | `auto` | `auto` (Any bearer), `1` (F80T0.25Q-1B), `2` (F80T1Q-1B), `3` (F80T1Q-4B), `4` (F80T1X-4B), `5` (FR80T2.5X4/16-5B), `6` (FR80T2.5X16-5B), `7` (FR80T2.5X32-6B), `8` (FR80T2.5X64-7B), `9` (F80T4.5X-8B), `10` (FR80T5X4/16-9B), `11` (FR80T5X16-9B), `12` (FR80T5X32-11B), `13` (FR80T5X64-13B) |  |
| `satellites` | Satellite names | text | empty |  | One a line (or separated by ;): longitude in degrees east, name. A SatelliteLocation within 1.5 degrees takes the name; without one the longitude is shown. |
| `scan` | Scan the band | yes or no | false |  | Asks the host to visit the band from scan_from_hz to scan_to_hz in steps of the sample rate, 7 s on each. The carrier rows stay with their last state. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `uw_order` | Unique word bit order | choice | `msb_first` | `msb_first` (First symbol is the most significant bit), `lsb_first` (First symbol is the least significant bit) | No public document states which end of the printed hex is sent first. |
| `iq_labels` | I and Q columns | choice | `in_order` | `in_order` (In column order), `swapped` (Swapped within each pair) | How the columns of the 38 tables labelled with I and Q map to the bits of a symbol; no public document states it. |
| `scan_from_hz` | Scan from | frequency | 1.518 GHz | 1.518 GHz to 1.559 GHz |  |
| `scan_to_hz` | Scan to | frequency | 1.559 GHz | 1.518 GHz to 1.559 GHz |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bearer` | Bearers | choice | `0` | `0` (A group of carriers), `1` (F80T0.25Q-1B), `2` (F80T1Q-1B), `3` (F80T1Q-4B), `4` (F80T1X-4B), `5` (FR80T2.5X4/16-5B), `6` (FR80T2.5X16-5B), `7` (FR80T2.5X32-6B), `8` (FR80T2.5X64-7B), `9` (F80T4.5X-8B), `10` (FR80T5X4/16-9B), `11` (FR80T5X16-9B), `12` (FR80T5X32-11B), `13` (FR80T5X64-13B) |  |
| `gen_subtype` | Subtype | choice | `r` | `l8` (L8), `l7` (L7), `l6` (L6), `l5` (L5), `l4` (L4), `l3` (L3), `l2` (L2), `l1` (L1), `r` (R), `h1` (H1), `h2` (H2), `h3` (H3), `h4` (H4), `h5` (H5), `h6` (H6) |  |
| `gen_carrier_cn0` | Carrier C/N0 | number (dB-Hz) | 70 | 20 to 100 |  |
| `gen_carrier_offset_hz` | Carrier from the dial | number (Hz) | 0 | -900000 to 900000 |  |
| `gen_carrier_clock_ppm` | Symbol clock error | number (ppm) | 0 | -100 to 100 |  |
| `gen_carrier_drift_hz_s` | Carrier drift | number (Hz/s) | 0 | -1000 to 1000 |  |
| `gen_fade_hz` | Fading rate | number (Hz) | 0 | 0 to 100 | 0 for none. |
| `gen_fade_depth` | Fading depth | number | 0 | 0 to 1 | The share of the amplitude that the fading takes away at its deepest. |
| `gen_spot` | Spot beam | whole number | 42 | 0 to 255 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `carriers` | Carriers | whole number |
| `in_sync` | In frame sync | whole number |
| `decoded` | Signalling decoded | whole number |
| `gmr2` | GMR-2 class carriers | whole number |
| `networks` | Networks | text |
| `satellites_heard` | Satellites | text |
| `blocks_ok` | Blocks good | whole number |
| `blocks_bad` | Blocks bad | whole number |
| `connection_pdus` | Connection PDUs | whole number |
| `addressed_pdus` | Addressed PDUs | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carrier_table` | Carriers | `freq_hz` (frequency), `offset_hz` (frequency), `width_hz` (frequency), `snr_db` (number (dB)), `kind` (text), `state` (text), `bearer` (text), `subtype` (text), `symbol_rate_hz` (frequency), `cfo_hz` (frequency), `cn0_dbhz` (number (dB-Hz)), `frames` (whole number), `blocks_ok` (whole number), `blocks_bad` (whole number), `load_pct` (number (%)), `structure_only` (yes or no), `spot_beam` (whole number), `rnc` (whole number), `bct_id` (whole number), `f_bearer` (whole number), `beam_type` (text), `network` (text), `satellite` (text), `last_seen` (duration (s)) |
| `beam_table` | Beams | `spot_beam` (whole number), `beam_type` (text), `primary_hz` (frequency), `bearer_type` (text) |
| `log` | Log | `at` (duration (s)), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `bgan.signalling` | BGAN signalling | `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.2. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `BganFields`. The result fields of BGAN.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carriers` | whole number | always | 1.0 | Carriers found. |
| `in_sync` | whole number | always | 1.0 | Carriers in frame sync. |
| `decoded` | whole number | always | 1.0 | Carriers whose signalling decoded. |
| `gmr2` | whole number | always | 1.0 | GMR-2 class carriers. |
| `networks` | text | always | 1.0 | Networks heard (MCC-MNC). |
| `satellites_heard` | text | always | 1.0 | Satellites heard. |
| `blocks_ok` | whole number | always | 1.0 | FEC blocks decoded. |
| `blocks_bad` | whole number | always | 1.0 | FEC blocks that failed. |
| `connection_pdus` | whole number | always | 1.0 | Connection PDUs counted, never read. |
| `addressed_pdus` | whole number | always | 1.0 | PDUs addressed to a terminal counted, never read. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `note` | text | always | 1.0 | What is not confirmed by a public document. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `carrier_table`

Struct `BganCarrierRow`. A carrier. The row key is its place, `000` first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq_hz` | frequency | optional | 1.0 | The frequency a station was heard on. |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `width_hz` | frequency | always | 1.0 | Width of a carrier. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `kind` | text | always | 1.0 | What the carrier is. |
| `state` | text | always | 1.0 | The demodulator state. |
| `bearer` | text | optional | 1.0 | Bearer. |
| `subtype` | text | optional | 1.0 | Subtype of block 0. |
| `symbol_rate_hz` | frequency | optional | 1.0 | Symbol rate found, Hz. |
| `cfo_hz` | frequency | optional | 1.0 | Frequency left after the channel filter. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | C/N0 from the pilots. |
| `frames` | whole number | always | 1.0 | Frames in sync. |
| `blocks_ok` | whole number | always | 1.0 | FEC blocks decoded. |
| `blocks_bad` | whole number | always | 1.0 | FEC blocks that failed. |
| `load_pct` | number (%) | always | 1.0 | Share of blocks with PDUs. |
| `structure_only` | yes or no | always | 1.0 | Frame sync without decodable tables. |
| `spot_beam` | whole number | optional | 1.0 | Spot beam id. |
| `rnc` | whole number | optional | 1.0 | rnc-id. |
| `bct_id` | whole number | optional | 1.0 | bct-id. |
| `f_bearer` | whole number | optional | 1.0 | f-bearer. |
| `beam_type` | text | optional | 1.0 | Beam type. |
| `network` | text | optional | 1.0 | MCC-MNC. |
| `satellite` | text | optional | 1.0 | The carrying satellite. |
| `last_seen` | duration (s) | always | 1.0 | Seconds since last heard. |

### Table `beam_table`

Struct `BganBeamRow`. A spot beam. The row key is its place, `000` first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `spot_beam` | whole number | always | 1.0 | Spot beam id. |
| `beam_type` | text | optional | 1.0 | Beam type. |
| `primary_hz` | frequency | optional | 1.0 | The forward frequency of a primary bearer. |
| `bearer_type` | text | optional | 1.0 | The FBearerType name of a primary bearer. |

### Table `log`

Struct `BganLogRow`. A line of the signalling log. The row key is its place, `000` first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | Seconds into the capture. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Event `bgan.signalling`

Struct `BganSignallingEvent`. A line of decoded signalling.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_bgan_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `decode_all` | `sdrkit_bgan_get_decode_all` | `sdrkit_bgan_set_decode_all` |
| `bearer` | `sdrkit_bgan_get_bearer` | `sdrkit_bgan_set_bearer` |
| `uw_order` | `sdrkit_bgan_get_uw_order` | `sdrkit_bgan_set_uw_order` |
| `iq_labels` | `sdrkit_bgan_get_iq_labels` | `sdrkit_bgan_set_iq_labels` |
| `satellites` | `sdrkit_bgan_get_satellites` | `sdrkit_bgan_set_satellites` |
| `scan` | `sdrkit_bgan_get_scan` | `sdrkit_bgan_set_scan` |
| `scan_from_hz` | `sdrkit_bgan_get_scan_from_hz` | `sdrkit_bgan_set_scan_from_hz` |
| `scan_to_hz` | `sdrkit_bgan_get_scan_to_hz` | `sdrkit_bgan_set_scan_to_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bearer` | `sdrkit_bgan_gen_get_bearer` | `sdrkit_bgan_gen_set_bearer` |
| `gen_subtype` | `sdrkit_bgan_gen_get_subtype` | `sdrkit_bgan_gen_set_subtype` |
| `gen_carrier_cn0` | `sdrkit_bgan_gen_get_carrier_cn0` | `sdrkit_bgan_gen_set_carrier_cn0` |
| `gen_carrier_offset_hz` | `sdrkit_bgan_gen_get_carrier_offset_hz` | `sdrkit_bgan_gen_set_carrier_offset_hz` |
| `gen_carrier_clock_ppm` | `sdrkit_bgan_gen_get_carrier_clock_ppm` | `sdrkit_bgan_gen_set_carrier_clock_ppm` |
| `gen_carrier_drift_hz_s` | `sdrkit_bgan_gen_get_carrier_drift_hz_s` | `sdrkit_bgan_gen_set_carrier_drift_hz_s` |
| `gen_fade_hz` | `sdrkit_bgan_gen_get_fade_hz` | `sdrkit_bgan_gen_set_fade_hz` |
| `gen_fade_depth` | `sdrkit_bgan_gen_get_fade_depth` | `sdrkit_bgan_gen_set_fade_depth` |
| `gen_spot` | `sdrkit_bgan_gen_get_spot` | `sdrkit_bgan_gen_set_spot` |

## Try it

```sh
sdrkit run bgan --source synth:bgan
sdrkit run bgan --source iiod:192.168.2.1 --freq 1545
```
<!-- generated: end -->

## Description

Standards: ETSI TS 102 744-2-1 V1.1.1 (physical layer; the tables of its Annex C come with the standard as a separate archive) and ETSI TS 102 744-3-1 V1.1.1 (Bearer Control layer).

## What it does

The stack takes one wide capture of the forward band (1518 to 1559 MHz). The family carrier detector finds the carriers every 0.2 s. A carrier whose width fits a BGAN symbol rate gets its own demodulator:

| Width | Symbol rates tried | Bearers |
|---|---|---|
| 6 to 16 kHz | 8.4 kBd | F80T0.25Q-1B |
| 24 to 62 kHz | 33.6 kBd | F80T1Q-1B, F80T1Q-4B, F80T1X-4B |
| 62 to 130 kHz | 84 kBd | FR80T2.5X4/16-5B, FR80T2.5X16-5B, FR80T2.5X32-6B, FR80T2.5X64-7B |
| 130 to 225 kHz | 168 kBd, then 151.2 kBd | FR80T5X4/16-9B, FR80T5X16-9B, FR80T5X32-11B, FR80T5X64-13B, F80T4.5X-8B |

The demodulator filters the carrier to 4 samples per symbol with a root raised cosine filter. It finds the frame by the pilot comb (a pilot after every DS data symbols) and the 40-symbol unique word, which also gives the subtype of block 0 (L8 to H6) and the polarity of the word. The polarity separates the QPSK bearers from the others. The pilots and the unique word give the timing, the frequency, the channel and the noise of each frame. The FEC blocks are demapped (max-log), turbo decoded with the 16-state code of the standard and the interleaver and mapping tables of Annex C, descrambled, and read. Bearers with the same frame structure are told apart by the tables that decode. A bearer with dynamic modulation takes the modulation and code rate of each block from the ForwardBearerCodeRate of the previous frame.

A carrier of 130 to 225 kHz that does not lock is tested for the pair of lines that filtered OQPSK at 135.4 ksym/s leaves in its squared spectrum. Such a carrier is listed as "GMR-2 class (IsatPhone type)". This label comes from the shape of the spectrum; it does not identify the carrier.

The stack reads broadcast PDUs only: bulletin boards with their AVPs (satellite location, beam, network, primary bearers, time, code rates), AVP lists, return schedules, and the headers of spot beam maps, bearer tables, GPS ephemerides and system information indexes. PDUs addressed to a terminal and connection PDUs are counted. They are never read or kept. No user data, voice or subscriber identity is decoded. The return link (1626.5 to 1660.5 MHz and 1668 to 1675 MHz) cannot be received from the ground.

F80T1Q-1B has no published tables: the stack shows its frame structure only.

The field `note` says what is not confirmed by a public document. Two values are settings for that reason: the bit order of the unique-word hex (`uw_order`) and how the I and Q columns of 38 of the published tables map to the bits of a symbol (`iq_labels`).

## Tuning: notes

The default frequency is a frequency of the forward band, not a carrier the stack knows. The stack holds no channel or satellite list.

## Settings: notes

The file `data/links/inmarsat_satellites.csv` holds satellite names by longitude in the form of the setting `satellites`. It also gives the forward band in use (1525 to 1559 MHz).

## Result tables: notes

The carrier table has one row for each carrier found: `kind` is "not identified", "BGAN forward bearer", "GMR-2 class (IsatPhone type)" or "other"; `state` is "searching", "frame sync" or "signalling decoded"; `load_pct` is the share of FEC blocks that carried PDUs; `structure_only` is set when no published table decodes the bearer. The beam table lists the primary bearers announced, by spot beam. The log holds plain-language lines of broadcast signalling, newest first, up to 200; a line is added when what a carrier broadcasts changes.

## Test signal

The test signal is a group of carriers (the default) or one bearer. The group holds a global-beam F80T0.25Q-1B at L8, F80T1Q-4B, F80T1X-4B, FR80T2.5X4/16-5B, F80T4.5X-8B, FR80T5X16-9B and FR80T2.5X32-6B, and a filtered OQPSK carrier at 135.4 ksym/s. Each carrier has frames of the real structure with FEC blocks coded with the published tables. Block 0 carries a bulletin board (rnc 7, net-ver 2, bct-id 3, f-bearer 1, the spot beam) with satellite 3 at 24.9 E, the beam, network 901-11, a primary bearer at 1545.0 MHz and a time, and a return schedule; odd blocks carry a connection PDU. These are not real network values. The bearers with dynamic modulation switch their even blocks to 16-QAM. F80T1Q-1B carries random QPSK data in its frame structure. The generator sets the C/N0 of each carrier and adds its own noise.

## Try it: notes

```sh
sdrkit run bgan --source synth:bgan
sdrkit run bgan --source iiod:192.168.2.1 --freq 1545.5
```

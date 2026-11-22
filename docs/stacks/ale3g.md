<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# 3G-ALE

Third-generation ALE bursts: link setup (BW0), traffic management, HDL and LDL packets and acknowledgements (BW1 to BW4), with the datagrams rebuilt.

Stack id: `ale3g`. Group: HF utility.

3G-ALE (third-generation HF link automation): the BW0 link-setup bursts with their LE PDUs, and the traffic bursts BW1 (TM, HDL_ACK, HDL_EOM), BW2 (HDL packets), BW3 (LDL packets) and BW4 (LDL acknowledgements); the datagrams rebuilt. ```text iq -> [hf_front 24 kHz] -> [ale3g: matched filter, burst detectors, RAKE Walsh despreading, Viterbi decoders, PDUs] -> PDU log, datagrams ```

Standard: MIL-STD-188-141B Appendix C.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ale3g` prints this table with every option.

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bw2_taps` | BW2 PN taps | whole number | 49185 | 1 to 65535 | The register bits (as a 16-bit mask) whose parity is the new bit of the BW2 PN generator. The default is Figure C-12 as drawn (bits 15, 14, 5 and 0); no recording has confirmed it. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_set` | Bursts | choice | `exchange` | `exchange` (A full exchange), `bw0` (Link setup (BW0) only) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `bursts_total` | Bursts | whole number |
| `pdus_good` | PDUs good | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `pdus` | PDUs | `utc` (time), `burst` (text), `type` (text), `crc_good` (yes or no), `words` (text), `snr_db` (number (dB)), `offset_hz` (frequency), `data` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ale3g.pdu` | PDU | `text` (text), `burst` (text), `type` (text) |
| `ale3g.datagram` | Datagram | `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

### Result fields

Struct `Ale3gFields`. The result fields of 3G-ALE.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | The parts that no public document confirms. |
| `text` | text | always | 1.0 | The text received, the last lines. |
| `bursts_total` | whole number | always | 1.0 | Bursts read. |
| `pdus_good` | whole number | always | 1.0 | PDUs with a good check. |
| `snr_db` | number (dB) | optional | 1.0 | The signal-to-noise ratio of the last burst. Absent without a measure. |
| `offset_hz` | frequency | optional | 1.0 | The frequency error of the last burst. Absent without a measure. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `pdus`

Struct `Ale3gPduRow`. A PDU of 3G-ALE. The row key is the number of the PDU.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the PDU. |
| `burst` | text | always | 1.0 | The burst (BW0 to BW4). |
| `type` | text | always | 1.0 | The PDU type. |
| `crc_good` | yes or no | always | 1.0 | The check of the PDU passed. |
| `words` | text | always | 1.0 | The PDU in words. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio of the burst. |
| `offset_hz` | frequency | always | 1.0 | The frequency error of the burst. |
| `data` | text | always | 1.0 | The data bytes of a packet or datagram. |

### Event `ale3g.pdu`

Struct `Ale3gPduEvent`. A PDU of 3G-ALE.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `burst` | text | always | 1.0 | The burst (BW0 to BW4). |
| `type` | text | always | 1.0 | The PDU type. |

### Event `ale3g.datagram`

Struct `Ale3gDatagramEvent`. A complete datagram of 3G-ALE.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ale3g_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bw2_taps` | `sdrkit_ale3g_get_bw2_taps` | `sdrkit_ale3g_set_bw2_taps` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_set` | `sdrkit_ale3g_gen_get_set` | `sdrkit_ale3g_gen_set_set` |

## Try it

```sh
sdrkit run ale3g --source synth:ale3g
sdrkit run ale3g --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

Third-generation ALE bursts: link setup (BW0), traffic management, HDL and LDL packets and acknowledgements (BW1 to BW4), with the datagrams rebuilt. The receiver follows MIL-STD-188-141B Appendix C: detectors of the TLC/AGC sequence and of the BW0, BW1 and BW3 preambles (coherent over 48 symbols, five frequency hypotheses), timing, frequency and up to three paths from the known sequence, Walsh symbols despread and combined over the paths (RAKE), the interleavers of section C.5.1.2, the Viterbi decoders of the K = 7, K = 8 and K = 9 codes, and the PDUs with their CRCs (the LE PDU prefixes of Figure C-17). BW2 is recognised by its PN preamble after the TLC/AGC sequence; BW3 packets of 64 and 128 bytes are decoded from either half of the code or from both. Not confirmed by a public document: the BW2 PN generator (a setting) and the receiver's thresholds. BW3 packets of 256 and 512 bytes are not decoded: the printed interleaver parameters give no permutation.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_serial_dials.csv` and band presets
are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Settings: notes

The stack has no everyday settings: the bursts and their PDUs are found in the signal.

## Try it: notes

```sh
sdrkit run ale3g --source synth:ale3g --seconds 15 --audio none
sdrkit run ale3g --source iiod:192.168.2.1 --freq 7
```

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# nRF24L01

nRF24L01 Enhanced ShockBurst frames on one 2.4 GHz channel at 250 kbit/s, 1 and 2 Mbit/s (needs a user test on a real signal).

Stack id: `nrf24`. Group: 2.4 GHz, channel 0-125.

nRF24L01 Enhanced ShockBurst : bursts on one 2.4 GHz channel, read at 250 kbit/s, 1 and 2 Mbit/s, and frames found by trying start positions and address lengths against the CRC. Content a device encrypts is shown only as bytes.

Standard: Nordic nRF24L01+ Product Specification 1.0, clause 7.3 (Enhanced ShockBurst packet format).

Signal types: nRF24.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.402 GHz |
| Tuning range | 2.4 GHz to 2.526 GHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 5 MHz |
| Offset from the signal | 1.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:nrf24` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | whole number | 2 | 0 to 125 | Asks the host to tune 2400 + N MHz. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_rate` | Test signal rate | choice | `all` | `1000` (1 Mbit/s), `2000` (2 Mbit/s), `250` (250 kbit/s), `all` (All three in turn) |  |
| `gen_crc` | Test frames | choice | `both` | `16` (CRC-16), `8` (CRC-8), `both` (Both) |  |
| `gen_random` | Random-bit bursts (no frame) | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames` | Frames | whole number |
| `bad_bursts` | Bursts without frame | whole number |
| `bursts` | Bursts | whole number |
| `crc16_frames` | CRC-16 frames | whole number |
| `crc8_frames` | CRC-8 frames | whole number |
| `addresses` | Addresses | whole number |
| `channel` | Channel | whole number |
| `demod.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `bit_rate` (frequency), `address` (text), `address_len` (whole number), `length` (whole number), `pid` (whole number), `no_ack` (yes or no), `crc_bits` (whole number), `payload` (text), `text` (text), `snr_db` (number (dB)), `repeats` (whole number) |
| `addresses` | Addresses | `address` (text), `address_len` (whole number), `bit_rate` (frequency), `crc_bits` (whole number), `length` (whole number), `frequency` (frequency), `heard` (whole number), `last_heard` (time) |
| `bad_bursts` | Bursts without frame | `time_s` (number (s)), `duration_us` (number (us)), `snr_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `nrf24.address` | New nRF24 address | `source` (text), `text` (text), `family` (choice), `bit_rate` (frequency), `crc_bits` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `ism`, version 4.2. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

Key readings: `addresses`.

### Result fields

Struct `NrfFields`. The result fields of the tables of the nRF24L01 stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames` | whole number | always | 1.0 | Frames decoded. |
| `bad_bursts` | whole number | always | 1.0 | Bursts without a frame. |
| `bursts` | whole number | always | 1.0 | Bursts. |
| `crc16_frames` | whole number | always | 1.0 | Frames with a CRC-16. |
| `crc8_frames` | whole number | always | 1.0 | Frames with a CRC-8. |
| `addresses` | whole number | always | 1.0 | The addresses in the table. |
| `channel` | whole number | always | 1.0 | The channel number N (2400 + N MHz). |

### Table `frames`

Struct `NrfFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `bit_rate` | frequency | always | 1.0 | The bit rate. |
| `address` | text | always | 1.0 | The address as hex. |
| `address_len` | whole number | always | 1.0 | The address length in bytes. |
| `length` | whole number | always | 1.0 | The payload length in bytes. |
| `pid` | whole number | always | 1.0 | The PID. |
| `no_ack` | yes or no | always | 1.0 | The NO_ACK bit. |
| `crc_bits` | whole number | always | 1.0 | The CRC bits. |
| `payload` | text | always | 1.0 | The payload as hex. |
| `text` | text | always | 1.0 | The printable text of the payload. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `repeats` | whole number | always | 1.0 | The same frame again in a row within 2 s. |

The column `address` names a row.

### Table `addresses`

Struct `NrfAddressRow`. A row of the addresses. The row key is the address as hex.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address as hex. |
| `address_len` | whole number | always | 1.0 | The address length in bytes. |
| `bit_rate` | frequency | always | 1.0 | The bit rate. |
| `crc_bits` | whole number | always | 1.0 | The CRC bits. |
| `length` | whole number | always | 1.0 | The last payload length in bytes. |
| `frequency` | frequency | always | 1.0 | The carrier frequency. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The rows are the stations or nodes of the stack. The column `address` names a row. Default order: the column `last_heard`, highest first. Time: `last_heard`.

### Table `bad_bursts`

Struct `NrfBadBurstRow`. A row of the bursts without a frame. The row key is the number of the burst in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the burst. |
| `duration_us` | number (us) | always | 1.0 | The duration of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |

### Event `nrf24.address`

Struct `NrfAddressEvent`. The fields of a new address event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address as hex. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `bit_rate` | frequency | always | 1.0 | The bit rate. |
| `crc_bits` | whole number | always | 1.0 | The CRC bits. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_nrf24_clear` | none | none |
| `remove` | `sdrkit_nrf24_remove` | `sdrkit_nrf24_nrf_address_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_nrf24_get_channel` | `sdrkit_nrf24_set_channel` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_rate` | `sdrkit_nrf24_gen_get_rate` | `sdrkit_nrf24_gen_set_rate` |
| `gen_crc` | `sdrkit_nrf24_gen_get_crc` | `sdrkit_nrf24_gen_set_crc` |
| `gen_random` | `sdrkit_nrf24_gen_get_random` | `sdrkit_nrf24_gen_set_random` |

## Try it

```sh
sdrkit run nrf24 --source synth:nrf24
sdrkit run nrf24 --source iiod:192.168.2.1 --freq 2402
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

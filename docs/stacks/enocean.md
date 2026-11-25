<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# EnOcean

EnOcean switches, contacts and sensors (ERP1 at 868.3 MHz, ERP2 at 902.875 and 928.35 MHz): sender, telegram type and the readings of the common profiles; secure telegrams shown as encrypted.

Stack id: `enocean`. Group: Home automation.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.3 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 1 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 300 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:enocean` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold` | Detection threshold | number (dB) | 10 | 6 to 25 | How far above the noise floor a burst must rise. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_radio` | Test signal protocol | choice | `erp1` | `erp1` (ERP1 (amplitude keying)), `erp2` (ERP2 (frequency keying)) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `telegrams` | Telegrams | whole number |
| `subtelegrams` | Subtelegrams | whole number |
| `frames_bad` | Frames bad | whole number |
| `devices` | Devices | whole number |
| `bursts` | Bursts | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Devices | `name` (text), `radio` (text), `telegram_type` (text), `profile` (text), `reading` (text), `telegrams` (whole number), `secure` (whole number), `level_dbfs` (number (dBFS)), `first_heard` (time), `last_heard` (time) |
| `telegrams` | Telegrams | `utc` (time), `time_s` (number (s)), `sender` (text), `radio` (text), `telegram_type` (text), `reading` (text), `data` (text), `destination` (text), `repeater` (text), `subtelegrams` (whole number), `check` (text), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `enocean.telegram` | EnOcean telegram | `sender` (text), `telegram_type` (text), `reading` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `ism`, version 4.2. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

Key readings: `devices`.

### Result fields

Struct `EnoceanFields`. The result fields of the EnOcean stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `telegrams` | whole number | always | 4.2 | Telegrams; the subtelegrams of one telegram count once. |
| `subtelegrams` | whole number | always | 4.2 | Subtelegrams with a good check. |
| `frames_bad` | whole number | always | 4.2 | Frames that the line coding accepted and the check refused. |
| `devices` | whole number | always | 4.2 | Senders heard. |
| `bursts` | whole number | always | 4.2 | Bursts over the noise floor. |
| `level_dbfs` | number (dBFS) | optional | 4.2 | The power of the input. It has no value before the first input. |
| `note` | text | always | 4.2 | What no real signal has confirmed and what is not read. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `devices`

Struct `EnoceanDeviceRow`. A row of the devices. The row key is the sender id, hex.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 4.2 | The name of the device for people: "EnOcean", the profile when known, and the id. |
| `radio` | text | always | 4.2 | The radio protocol last heard. |
| `telegram_type` | text | always | 4.2 | The last telegram type. |
| `profile` | text | optional | 4.2 | The profile (EEP), from a teach-in or certain from the telegram type. |
| `reading` | text | always | 4.2 | The last reading. |
| `telegrams` | whole number | always | 4.2 | Telegrams heard. |
| `secure` | whole number | always | 4.2 | Secure telegrams heard. |
| `level_dbfs` | number (dBFS) | always | 4.2 | The level of the last telegram. |
| `first_heard` | time | optional | 4.2 | The UTC of the first telegram. |
| `last_heard` | time | optional | 4.2 | The UTC of the last telegram. |

The rows are the stations or nodes of the stack. The column `name` names a row. Default order: the column `last_heard`, highest first. Time: `first_heard`.

### Table `telegrams`

Struct `EnoceanTelegramRow`. A row of the telegram log. The row key is the number of the telegram in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 4.2 | The UTC of the first subtelegram, when the samples are live. |
| `time_s` | number (s) | always | 4.2 | The signal time of the first subtelegram. |
| `sender` | text | always | 4.2 | The sender id, hex. |
| `radio` | text | always | 4.2 | The radio protocol: ERP1 or ERP2. |
| `telegram_type` | text | always | 4.2 | The telegram type (RORG). |
| `reading` | text | always | 4.2 | What the telegram says. |
| `data` | text | always | 4.2 | The payload as hex; empty for a secure telegram. |
| `destination` | text | optional | 4.2 | The destination id of an addressed telegram, hex. |
| `repeater` | text | always | 4.2 | The repeater level of the first subtelegram. |
| `subtelegrams` | whole number | always | 4.2 | Subtelegrams heard of this telegram. |
| `check` | text | always | 4.2 | The check that passed. |
| `level_dbfs` | number (dBFS) | always | 4.2 | The level of the first subtelegram. |

The rows are a log: they arrive over time. The column `sender` names a row. Time: `utc`.

### Event `enocean.telegram`

Struct `EnoceanTelegramEvent`. The fields of a telegram event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sender` | text | always | 4.2 | The sender id, hex. |
| `telegram_type` | text | always | 4.2 | The telegram type (RORG). |
| `reading` | text | always | 4.2 | What the telegram says. |
| `family` | choice | always | 4.2 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_enocean_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold` | `sdrkit_enocean_get_threshold` | `sdrkit_enocean_set_threshold` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_radio` | `sdrkit_enocean_gen_get_radio` | `sdrkit_enocean_gen_set_radio` |

## Try it

```sh
sdrkit run enocean --source synth:enocean
sdrkit run enocean --source iiod:192.168.2.1 --freq 868.3
```
<!-- generated: end -->

## What is decoded

Every burst is read as ERP1 (868.3 MHz, amplitude keying, 125 kbit/s, 8/12 coding, a 1 sent at low power) and as ERP2
(frequency keying, 125 kbit/s, ±62.5 kHz, a 1 on the upper tone, at 902.875 or 928.35 MHz). The frequency comes from the
session; the plan file `data/plans/enocean.json` names the frequency of each region.

| Telegram type | What is shown |
|---|---|
| RPS (F6) | Rocker buttons and the energy bow as F6-02-01 reads them; the profile is not sent |
| 1BS (D5) | Contact open or closed (D5-00-01), teach-in |
| 4BS (A5) | Teach-in with its profile and manufacturer; then the data of A5-02-01 to A5-02-1B (temperature), A5-04-01 (humidity, temperature), A5-06-01 and A5-06-02 (light) and A5-07-01 (motion). Without a teach-in the four bytes are shown raw |
| Secure (30, 31, 33, 34, 35) | "encrypted" and the length; nothing is decrypted |
| Others | The type name and the length |

The subtelegrams of one telegram (up to three within the 100 ms RX maturity time) count as one telegram. ERP1 telegrams
pass their checksum or CRC-8; ERP2 telegrams pass their CRC-8.

## Not confirmed

The stack was written from the public EnOcean documents (EnOcean Radio Protocol 1 V1.2, EnOcean Radio Protocol 2 V1.3,
EnOcean Equipment Profiles 3.1 and the EEP 2.6.8 profile sheets) and checked only on its own test signal. Not read: the
4-bit check of the 6-byte ERP1 switch form, whose layout is also unconfirmed; start of frame variant 1 when its 1.5-bit
runs read short; ERP2 telegrams of 6 bytes or fewer, which have no check.

The stack only receives.

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SSDV pictures

Pictures from high-altitude balloons and small satellites sent as SSDV packets.

Stack id: `ssdv`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 434.65 MHz |
| Tuning range | 30 MHz to 1 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 50 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ssdv` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `baud` | Speed | choice | `auto` | `auto` (Automatic (all at once)), `300_8n2` (300 (8N2)), `600_8n2` (600 (8N2)), `1200_8n2` (1200 (8N2)), `1200`, `2400`, `4800`, `9600`, `19200`, `115200` (115200 (Wenet)) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mode` | Test signal: speed | choice | `9600` | `auto` (Automatic (all at once)), `300_8n2` (300 (8N2)), `600_8n2` (600 (8N2)), `1200_8n2` (1200 (8N2)), `1200`, `2400`, `4800`, `9600`, `19200`, `115200` (115200 (Wenet)) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `packets_ok` | Packets | whole number |
| `packets_bad` | Packets refused | whole number |
| `corrected` | Packets corrected | whole number |
| `pictures_done` | Pictures complete | whole number |
| `last_baud` | Speed | whole number (Bd) |
| `last_n82` | 8N2 | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `packets` | Packets | `time_s` (number (s)), `host_time` (time), `call` (text), `image` (whole number), `packet` (whole number), `rs` (whole number), `baud` (whole number (Bd)), `n82` (yes or no), `inverted` (yes or no), `no_fec` (yes or no), `eoi` (yes or no) |
| `pictures` | Pictures | `call` (text), `image` (whole number), `width` (whole number (px)), `height` (whole number (px)), `quality` (whole number), `held` (whole number), `missing` (whole number), `complete` (yes or no), `bytes` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `science`, version 2.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Pictures and fax (`pictures`). Media: pictures.

Key readings: `pictures_done`, `packets_ok`, `packets_bad`.

### Result fields

Struct `SsdvFields`. The result fields of SSDV.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `packets_ok` | whole number | always | 1.0 | Packets received. |
| `packets_bad` | whole number | always | 1.0 | Packets refused. |
| `corrected` | whole number | always | 1.0 | Packets the RS code corrected. |
| `pictures_done` | whole number | always | 1.0 | Pictures complete. |
| `last_baud` | whole number (Bd) | optional | 1.0 | The speed of the last good packet. |
| `last_n82` | yes or no | optional | 1.0 | The last good packet came as 8N2 bytes. |

### Table `packets`

Struct `SsdvPacketRow`. An SSDV packet. The row key is the sequence number of the packet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time of a packet. |
| `host_time` | time | optional | 1.0 | Host time of a packet. |
| `call` | text | always | 1.0 | The call sign. |
| `image` | whole number | always | 1.0 | The image id. |
| `packet` | whole number | always | 1.0 | The packet id. |
| `rs` | whole number | always | 1.0 | Bytes corrected in a packet. |
| `baud` | whole number (Bd) | always | 1.0 | The speed of a packet. |
| `n82` | yes or no | always | 1.0 | 8N2 bytes. |
| `inverted` | yes or no | always | 1.0 | Inverted bits. |
| `no_fec` | yes or no | always | 1.0 | A packet without FEC. |
| `eoi` | yes or no | always | 1.0 | The last packet of a picture. |

The rows are a log: they arrive over time. The column `call` names a row. Time: `host_time`.

### Table `pictures`

Struct `SsdvPictureRow`. An SSDV picture. The row key is the call sign and the image id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `call` | text | always | 1.0 | The call sign. |
| `image` | whole number | always | 1.0 | The image id. |
| `width` | whole number (px) | always | 1.0 | The width of a picture. |
| `height` | whole number (px) | always | 1.0 | The height of a picture. |
| `quality` | whole number | always | 1.0 | The quality level of a picture. |
| `held` | whole number | always | 1.0 | Packets held of a picture. |
| `missing` | whole number | always | 1.0 | Packets missing of a picture. |
| `complete` | yes or no | always | 1.0 | The picture is complete. |
| `bytes` | whole number | always | 1.0 | The size of the JPEG. |

The column `call` names a row.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ssdv_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `baud` | `sdrkit_ssdv_get_baud` | `sdrkit_ssdv_set_baud` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mode` | `sdrkit_ssdv_gen_get_mode` | `sdrkit_ssdv_gen_set_mode` |

## Try it

```sh
sdrkit run ssdv --source synth:ssdv
sdrkit run ssdv --source iiod:192.168.2.1 --freq 434.65
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

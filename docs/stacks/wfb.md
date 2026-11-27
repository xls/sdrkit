<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# wfb-ng / OpenHD

Drone video links of injected 802.11n frames: link id, radio port, FEC block and fragment, frames lost. The payload stays encrypted: structure only.

Stack id: `wfb`. Group: Data and IoT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 5.805 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 20 MHz |
| Smallest sample rate | 20 MHz |
| Signal bandwidth | 20 MHz |
| Radio filter | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wfb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `links` | Links shown | choice | `both` | `wfb-ng`, `openhd` (OpenHD), `both` (Both) | Which systems the tables show. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `ht_frames` | HT frames decoded | whole number |
| `ht_not_decodable` | HT frames not decodable here | whole number |
| `bad_fcs` | Frames with a bad FCS | whole number |
| `links_heard` | Links | whole number |
| `frames_lost` | Frames lost | whole number |
| `snr_db` | SNR | number (dB) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `links` | Links | `link` (text), `id` (text), `frames` (whole number), `frames_lost` (whole number), `first_heard` (time), `last_heard` (time), `snr_db` (number (dB)), `last_type` (text), `last_detail` (text) |
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `link` (text), `id` (text), `type` (text), `detail` (text), `hex` (text), `snr_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `wlan`, version 4.7. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `WfbFields`. The result fields of `wfb`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames of wfb-ng or OpenHD links. |
| `ht_frames` | whole number | always | 1.0 | HT frames with a good FCS. |
| `ht_not_decodable` | whole number | always | 1.0 | HT-mixed PPDUs that the decoder does not take. |
| `bad_fcs` | whole number | always | 1.0 | HT frames with a bad FCS. |
| `links_heard` | whole number | always | 1.0 | Links in the table. |
| `frames_lost` | whole number | always | 1.0 | Frames lost over every link. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last frame. No value before the first frame. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `links`

Struct `WfbLinkRow`. A row of the links of `wfb`. The row key is the system and the link id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | text | always | 1.0 | "wfb-ng" or "OpenHD". |
| `id` | text | always | 1.0 | The link id. |
| `frames` | whole number | always | 1.0 | Frames heard. |
| `frames_lost` | whole number | always | 1.0 | Frames lost. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the last frame. |
| `last_type` | text | always | 1.0 | The type of the last frame. |
| `last_detail` | text | always | 1.0 | The detail of the last frame. |

The column `id` names a row.

### Table `frames`

Struct `WfbFrameRow`. A row of the frame log of `wfb`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the frame, for a live source. |
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `link` | text | always | 1.0 | "wfb-ng" or "OpenHD". |
| `id` | text | always | 1.0 | The link id. |
| `type` | text | always | 1.0 | The type of the frame. |
| `detail` | text | always | 1.0 | The detail of the frame. |
| `hex` | text | always | 1.0 | The first payload bytes, hex. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the frame. |

The column `hex` names a row.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_wfb_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `links` | `sdrkit_wfb_get_links` | `sdrkit_wfb_set_links` |

## Try it

```sh
sdrkit run wfb --source synth:wfb
sdrkit run wfb --source iiod:192.168.2.1 --freq 5805
```
<!-- generated: end -->

## Description

wfb-ng and OpenHD send their video as 802.11n data frames that the transmitter injects without joining a network. The stack
brings the channel at the dial to 20 Msps and runs the non-HT OFDM receiver of the Wi-Fi stack: short and long training fields,
carrier offset, channel and L-SIG. A PPDU whose L-SIG says 6 Mbit/s goes to the HT-mixed decoder before the non-HT data
decoder: the first HT-SIG symbol must carry its energy on the Q axis; HT-SIG is decoded and its CRC checked; the channel of the
data symbols comes from the HT-LTF; each symbol's common phase and phase slope come from its four pilots; the soft bits are
weighted by the channel, deinterleaved, depunctured and decoded, the scrambler seed found and the FCS checked.

Decoded: one spatial stream, 20 MHz, the convolutional code, MCS 0 to 7, the long and the short guard interval. Counted as "not
decodable here": MCS 8 and up, 40 MHz, aggregation, LDPC, STBC, extension streams.

A frame of frame control 08 01 to the broadcast address is a wfb-ng frame when addresses 2 and 3 both hold "WB" and the channel
id (link id and radio port): payload type 1 is a data packet whose nonce gives the FEC block and the fragment, type 2 a session
key packet. It is an OpenHD frame when addresses 2 and 3 start with the same direction byte (1 air, 2 ground) and end with the
same radio port; the two addresses make its nonce. For each wfb-ng link and port the stack counts the frames lost from the
blocks and fragments it sees.

Not confirmed by a public document: the wfb-ng and OpenHD frame layouts. The values of the HT data pilots are taken fixed or
moving one place each symbol, whichever fits the received pilots better. Not tried on a real signal.

Standard: IEEE Std 802.11-2020 clauses 17 and 19.

## Tuning: notes

Below 20 Msps the stack says "wfb-ng / OpenHD needs 20 Msps". From 40 Msps the dial can lie anywhere in the capture. The Wi-Fi
channels per region are data, for the host to offer; `data/plans/drone_links.json` holds the bands of these links.

## Result tables: notes

A wfb-ng link is "link 0A1B2C port 3", an OpenHD link "air port 5" or "ground port 2". The detail of a frame gives the MCS, the
guard interval, the block and fragment or the nonce, and the length; the frame log shows the first 32 payload bytes.

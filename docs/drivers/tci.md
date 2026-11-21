<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# TCI (network)

A program that serves the TCI interface, such as ExpertSDR3 for SunSDR radios.

Driver key: `tci`.

## Address

- `tci:<host>`: port 50001.
- `tci:<host>:<port>`

The device has one receiver: one radio of an address is open at a time, and the sessions share it.

## Radio settings

Every radio has these. Give one as `--set key=value`. If the stack has a setting with the same key, write `radio.key=value`.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `centre_hz` | Centre frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `rate` | Sample rate | frequency | 0 Hz | 0 Hz to 1 GHz | The rate of the samples from the radio. On Automatic the radio takes the least rate at or above what the stack needs; a rate that is set stays, and a stack that needs more is refused. Write 0 to return to Automatic. |
| `bandwidth` | Bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz | The width of the analogue filter of the radio. Without a width, the driver chooses one that fits the sample rate. |
| `gain` | Gain | number (dB) | 30 | -100 to 200 |  |
| `agc` | Automatic gain | yes or no | true |  |  |
| `rate_exact` | Exact rate | yes or no | false |  | Sends the rate as it is, even outside the rates the driver picks. |
| `bias_tee` | Bias tee | yes or no | false |  |  |

## Driver settings

Only this driver has these.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `receiver` | Receiver | whole number | 0 | 0 to 7 | The receiver of the server whose IQ to take, counted from 0. A receiver the server does not have is refused. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cf32 | 48 kHz to 384 kHz | 1 |

## Readings

| Key | Label | Kind |
|---|---|---|
| `state` | Link | choice |
| `rate_hz` | Sample rate | frequency |
| `centre_hz` | Centre | frequency |
| `samples` | Samples | whole number |
| `dropped` | Dropped | whole number |
| `buffer_pct` | Buffer | number (%) |
| `radio_lost` | Lost by the radio | whole number |
| `link_down` | Lost while away | whole number |
| `delivered_pct` | Delivered | number (%) |
| `problem` | Problem | text |
| `server` | Server | text |
| `device` | Device | text |
| `receive_only` | Receive only | yes or no |
| `receivers` | Receivers | whole number |
| `channels` | Channels per receiver | whole number |
| `vfo_min_hz` | Lowest VFO | frequency |
| `vfo_max_hz` | Highest VFO | frequency |
| `if_min_hz` | Lowest IF offset | frequency |
| `if_max_hz` | Highest IF offset | frequency |
| `receiver` | Receiver | whole number |
| `smeter_dbm` | S-meter (server) | number (dBm) |
| `server_trx` | Server transmits | yes or no |
| `tx_packets` | Transmit packets | whole number |
| `tx_frames` | Transmit frames | whole number |
| `tx_underrun_frames` | Transmit underrun frames | whole number |
| `iq_rate_hz` | IQ rate | frequency |
| `format` | Sample type | text |
| `unusable` | Packets dropped | whole number |
| `server_running` | Server radio runs | yes or no |
| `panorama_hz` | Panorama (server) | frequency |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `low_throughput` | Link too slow | yes or no |

## Events

| Kind | Label | Fields |
|---|---|---|
| `radio.connected` | Radio connected | `source` (text), `text` (text), `family` (choice) |
| `radio.lost` | Radio lost | `source` (text), `text` (text), `family` (choice) |
| `radio.overflow` | Samples dropped | `source` (text), `text` (text), `family` (choice) |
| `radio.restart` | Restart needed | `source` (text), `text` (text), `family` (choice) |

## Commands of an open radio

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `reconnect` | Reconnect |  |  |  |

## Commands of the driver

They work before any radio is open. Run one with `sdrkit call kind:driver:tci <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 50001) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source tci:192.168.1.30
```
<!-- generated: end -->

## Readings

| Key | Label | Kind |
|---|---|---|
| `state` | Link | choice |
| `rate_hz` | Sample rate | frequency |
| `centre_hz` | Centre | frequency |
| `samples` | Samples | whole number |
| `dropped` | Dropped | whole number |
| `buffer_pct` | Buffer | number (%) |
| `radio_lost` | Lost by the radio | whole number |
| `link_down` | Lost while away | whole number |
| `delivered_pct` | Delivered | number (%) |
| `problem` | Problem | text |
| `server` | Server | text |
| `device` | Device | text |
| `receive_only` | Receive only | yes or no |
| `receivers` | Receivers | whole number |
| `channels` | Channels per receiver | whole number |
| `vfo_min_hz` | Lowest VFO | frequency |
| `vfo_max_hz` | Highest VFO | frequency |
| `if_min_hz` | Lowest IF offset | frequency |
| `if_max_hz` | Highest IF offset | frequency |
| `receiver` | Receiver | whole number |
| `smeter_dbm` | S-meter (server) | number (dBm) |
| `server_trx` | Server transmits | yes or no |
| `tx_packets` | Transmit packets | whole number |
| `tx_frames` | Transmit frames | whole number |
| `tx_underrun_frames` | Transmit underrun frames | whole number |
| `iq_rate_hz` | IQ rate | frequency |
| `format` | Sample type | text |
| `unusable` | Packets dropped | whole number |
| `server_running` | Server radio runs | yes or no |
| `panorama_hz` | Panorama (server) | frequency |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `low_throughput` | Link too slow | yes or no |

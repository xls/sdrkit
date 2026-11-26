<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# KiwiSDR

A KiwiSDR receiver on the network. The driver reads the IQ stream of one channel.

Driver key: `kiwisdr`.

## Address

- `kiwisdr:<host>`: port 8073.
- `kiwisdr:<host>:<port>`
- `kiwisdr:http://<host>:<port>/`

Each open radio of an address is a receiver of its own.

## Radio settings

Every radio has these. Give one as `--set key=value`. If the stack has a setting with the same key, write `radio.key=value`.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `centre_hz` | Centre frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `gain` | Gain | number (dB) | 30 | -100 to 200 |  |
| `agc` | Automatic gain | yes or no | true |  |  |
| `rate` | Sample rate | frequency | 0 Hz | 0 Hz to 1 GHz | The rate of the samples from the radio. On Automatic the radio takes the least rate at or above what the stack needs; a rate that is set stays, and a stack that needs more is refused. Write 0 to return to Automatic. |
| `bandwidth` | Bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz | The width of the analogue filter of the radio. Without a width, the driver chooses one that fits the sample rate. |
| `rate_exact` | Exact rate | yes or no | false |  | Sends the rate as it is, even outside the rates the driver picks. |
| `bias_tee` | Bias tee | yes or no | false |  |  |

## Driver settings

Only this driver has these.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `passband` | Passband | choice | `5000` | `3000` (±3.0 kHz), `5000` (±5.0 kHz), `5900` (±5.9 kHz), `max` (Widest the receiver allows) | The half-width of the IQ filter of the receiver around the tuned frequency. The receiver sends 12 kHz of spectrum. A passband wider than the rate of the receiver allows is refused. |
| `swap_iq` | Swap I and Q | yes or no | false |  | Mirrors the spectrum. Turn it on if signals that belong above the frequency appear below it. |
| `password` | Password | text | empty |  | Only if the owner set one. Never shown in names, logs or the status. |
| `tlimit_password` | Time-limit exemption password | text | empty |  | Some owners give regular listeners a password that lifts the time limit of a session. |
| `user_name` | Your name, shown to the owner | text | `sdrkit` |  | Shown in the listener list of the receiver, so the owner knows who listens. At most 32 characters. |
| `location` | Your location, shown to the owner | text | empty |  | Shown to the owner and to other listeners. At most 64 characters. If empty, nothing is sent. |
| `users` | Show other listeners | yes or no | true |  | Asks the receiver every 10 s who else is connected. |
| `waterfall` | Receiver's waterfall | yes or no | false |  | Also shows the wide waterfall of the receiver. It opens a second connection to the receiver. |
| `wf_zoom` | Waterfall zoom | whole number | 0 | 0 to 14 | 0 shows the whole range. Each step halves the span around the tuned frequency. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cs16 | 8 kHz to 25 kHz | 1 |

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
| `rssi` | S-meter | number (dBm) |
| `rssi_max` | S-meter, highest of the last second | number (dBm) |
| `s_units` | S-units | text |
| `overload` | ADC overload | yes or no |
| `gps` | GPS time | yes or no |
| `gps_time` | GPS time of the samples | time (s) |
| `frames` | Frames received | whole number |
| `missed` | Frames missed | whole number |
| `bad` | Bad frames | whole number |
| `not_iq` | Frames without IQ | whole number |
| `gaps` | Gaps filled | whole number |
| `throughput` | Throughput | number (Mbit/s) |
| `iq_rate` | IQ samples received | number (%) |
| `version` | Version | text |
| `channels` | Channels | whole number |
| `free_channels` | Channels without a password | whole number |
| `lowest_hz` | Covered from | frequency |
| `highest_hz` | Covered to | frequency |
| `session_limit` | Session limit | whole number (min) |
| `rx_name` | Receiver | text |
| `rx_location` | Location | text |
| `rx_antenna` | Antenna | text |
| `rx_grid` | Grid | text |
| `rx_position` | Position | position |
| `reason` | Not streaming | text |
| `waterfall_problem` | Waterfall not available | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `listeners` | Listeners | `channel` (whole number), `name` (text), `location` (text), `freq_hz` (frequency), `demod` (text), `connected` (duration (s)) |

## Result pictures

| Key | Label | X unit | Y unit | Cell unit |
|---|---|---|---|---|
| `waterfall` | Waterfall | Hz | line | dB |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:kiwisdr <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `probe` | Test the connection | `address` (text, required), `password` (text, default empty) |  | Connects to the receiver, logs in and closes at once. It streams nothing. A receiver that answers is listed. |
| `address` | Build an address | `host` (text, required), `port` (whole number, default 8073) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `receivers` | Receivers tested | `address` (text), `version` (text), `channels` (whole number) |

## Try it

```sh
sdrkit run fm --source kiwisdr:kiwi.example.org:8073
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
| `rssi` | S-meter | number (dBm) |
| `rssi_max` | S-meter, highest of the last second | number (dBm) |
| `s_units` | S-units | text |
| `overload` | ADC overload | yes or no |
| `gps` | GPS time | yes or no |
| `gps_time` | GPS time of the samples | time (s) |
| `frames` | Frames received | whole number |
| `missed` | Frames missed | whole number |
| `bad` | Bad frames | whole number |
| `not_iq` | Frames without IQ | whole number |
| `gaps` | Gaps filled | whole number |
| `throughput` | Throughput | number (Mbit/s) |
| `iq_rate` | IQ samples received | number (%) |
| `version` | Version | text |
| `channels` | Channels | whole number |
| `free_channels` | Channels without a password | whole number |
| `lowest_hz` | Covered from | frequency |
| `highest_hz` | Covered to | frequency |
| `session_limit` | Session limit | whole number (min) |
| `rx_name` | Receiver | text |
| `rx_location` | Location | text |
| `rx_antenna` | Antenna | text |
| `rx_grid` | Grid | text |
| `rx_position` | Position | position |
| `reason` | Not streaming | text |
| `waterfall_problem` | Waterfall not available | text |

<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Red Pitaya

A Red Pitaya radio, through the HPSDR protocol 1.

Driver key: `redpitaya`.

## Address

- `redpitaya:<host>:<port>`: receiver 1, the port is required.
- `redpitaya:<host>:<port>/rx<n>`: receiver n.

The device has one receiver: one radio of an address is open at a time, and the sessions share it.

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
| `ppm` | Frequency correction | number (ppm) | 0 | -200 to 200 | The clock error of the radio in ppm, positive when its clock runs fast (signals then show a little low). 0: no correction. |
| `swap_iq` | Swap I and Q | yes or no | false |  | Mirror the spectrum if signals show on the wrong side of the tuned frequency. The two words of each sample are taken the other way round. |
| `txtimeout` | Transmit time limit | number (s) | 180 | 10 to 600 | The longest time one transmission can last. Then sdrkit ends it. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cs32 | 48 kHz to 384 kHz | 1 |

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
| `board` | Board | whole number |
| `board_name` | Board name | text |
| `mac` | MAC | text |
| `version` | Version | text |
| `protocol` | Protocol | text |
| `radio_receivers` | Receivers it has | whole number |
| `busy` | Busy at discovery | yes or no |
| `receivers` | Receivers in the stream | whole number |
| `receiver` | This receiver | whole number |
| `frequency_sent` | Frequency sent | frequency |
| `overload` | ADC overload | yes or no |
| `pll` | PLL locked | yes or no |
| `temperature` | Temperature | number (°C) |
| `exciter` | Exciter power (raw) | whole number |
| `fwd` | Forward power (raw) | whole number |
| `rev` | Reverse power (raw) | whole number |
| `supply` | Supply voltage (raw) | whole number |
| `swr` | SWR | number |
| `seq_lost` | Lost in sequence gaps | whole number (samples) |
| `settle_lost` | Lost while settling | whole number (samples) |
| `queue_dropped` | Dropped (receiver behind) | whole number (samples) |
| `underruns` | Transmit underrun | whole number (samples) |
| `mbps` | Throughput | number (Mbit/s) |
| `pps` | Packets | number (1/s) |
| `achieved` | Achieved rate | number (sps) |
| `gaps` | Sequence gaps | whole number |
| `settle_drops` | Dropped while settling | whole number (packets) |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:redpitaya <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `discover` | Find radios | `wait_s` (number (s), default 1.5) |  | Asks every radio of this kind on the local networks to answer. The radios that answer are listed. |
| `probe` | Test an address | `address` (text, required), `wait_s` (number (s), default 1.5) |  | Asks the radio at the address to answer. A radio that answers is listed. |
| `address` | Build an address | `host` (text, required), `port` (whole number, required), `receiver` (whole number, default 1) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `radios` | Radios found | `address` (text), `busy` (yes or no), `board` (whole number), `receivers` (whole number), `mac` (text), `version` (text), `protocol` (whole number) |

## Transmit

The radio can transmit. The range of the transmit power and the power that the transmitter starts with come from the radio when it opens: the bottom of the range for a radio that can drive an amplifier, 10 dB below full power for a low-power board.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tx_frequency` | Transmit frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `tx_rate` | Transmit rate | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_bandwidth` | Transmit bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_power` | Transmit power | number (dB) | -10 | -200 to 0 | The output relative to the full output of the radio: 0 dB is full power, -10 dB is a tenth of it. |

Streams it accepts:

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cf32 | 48 kHz to 48 kHz | 1 |

## Try it

```sh
sdrkit run fm --source redpitaya:192.168.1.41:1024
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
| `board` | Board | whole number |
| `board_name` | Board name | text |
| `mac` | MAC | text |
| `version` | Version | text |
| `protocol` | Protocol | text |
| `radio_receivers` | Receivers it has | whole number |
| `busy` | Busy at discovery | yes or no |
| `receivers` | Receivers in the stream | whole number |
| `receiver` | This receiver | whole number |
| `frequency_sent` | Frequency sent | frequency |
| `overload` | ADC overload | yes or no |
| `pll` | PLL locked | yes or no |
| `temperature` | Temperature | number (°C) |
| `exciter` | Exciter power (raw) | whole number |
| `fwd` | Forward power (raw) | whole number |
| `rev` | Reverse power (raw) | whole number |
| `supply` | Supply voltage (raw) | whole number |
| `swr` | SWR | number |
| `seq_lost` | Lost in sequence gaps | whole number (samples) |
| `settle_lost` | Lost while settling | whole number (samples) |
| `queue_dropped` | Dropped (receiver behind) | whole number (samples) |
| `underruns` | Transmit underrun | whole number (samples) |
| `mbps` | Throughput | number (Mbit/s) |
| `pps` | Packets | number (/s) |
| `achieved` | Achieved rate | number (sps) |
| `gaps` | Sequence gaps | whole number |
| `settle_drops` | Dropped while settling | whole number (packets) |

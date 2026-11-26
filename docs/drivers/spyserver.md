<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SpyServer

A SpyServer, which shares one radio over TCP.

Driver key: `spyserver`.

## Address

- `spyserver:<host>:<port>`: the port is required.

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
| `format` | Sample format | choice | `8` | `8` (8-bit (low bandwidth)), `16` (16-bit) | The sample format the server sends. 8-bit needs the least network (2 bytes a sample). 16-bit keeps weak signals next to strong ones apart better, at twice the bandwidth. A server that forces a format sends that one. The format is fixed while the radio is open. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cu8 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |
| Iq | Cs16 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |
| Iq | Cf32 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |

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
| `device` | Device | text |
| `serial` | Serial number | whole number |
| `in_control` | In control | yes or no |
| `gain_index` | Gain index | whole number |
| `digital_gain_db` | Digital gain | number (dB) |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `low_throughput` | Link too slow | yes or no |
| `dropped_samples` | Samples dropped by the server | whole number |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:spyserver <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, required) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source spyserver:192.168.1.30:5555
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
| `device` | Device | text |
| `serial` | Serial number | whole number |
| `in_control` | In control | yes or no |
| `gain_index` | Gain index | whole number |
| `digital_gain_db` | Digital gain | number (dB) |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `low_throughput` | Link too slow | yes or no |

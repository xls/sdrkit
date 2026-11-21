<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SDR++ server

The server mode of SDR++, which streams the baseband of one radio.

Driver key: `sdrpp`.

## Address

- `sdrpp:<host>`: port 5259.
- `sdrpp:<host>:<port>`

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
| `sample_type` | Sample format | choice | `i16` | `i8` (8-bit), `i16` (16-bit), `f32` (32-bit float) | What the server sends. 16-bit keeps weak signals beside strong ones. 8-bit needs half the network (2 bytes per sample) and lets a slow link carry a higher rate. Float is exact but takes 8 bytes per sample (at 10 Msps: 160, 320 and 640 Mbit/s). |
| `compression` | Compression | yes or no | false |  | The server compresses each packet (zstd). Less network, more CPU on both ends. Most useful with 8-bit or 16-bit samples. |
| `server_gain` | Leave the gain to the server | yes or no | false |  | The driver never moves the gain sliders of the server. The gain stays as set on the server (or by its AGC). |
| `ppm` | Frequency correction | number (ppm) | 0 | -200 to 200 | The clock error of the radio in ppm, positive when its clock runs fast (signals show a little low). The driver asks the server for the corrected frequency. The sample rate is not corrected. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
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
| `source` | Source | text |
| `server_rate_hz` | Rate of the server | frequency |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `server_error` | Server error | text |
| `busy` | Another client | yes or no |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:sdrpp <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 5259) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source sdrpp:192.168.1.30
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
| `source` | Source | text |
| `server_rate_hz` | Rate of the server | frequency |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `server_error` | Server error | text |
| `busy` | Another client | yes or no |

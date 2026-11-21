<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SDRplay RSP (rsp_tcp server)

An rsp_tcp server, which shares an SDRplay RSP receiver on the network.

Driver key: `rsp_tcp`.

## Address

- `rsp_tcp:<host>`: port 1234.
- `rsp_tcp:<host>:<port>`

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
| `protocol` | Protocol | choice | `auto` | `auto` (Auto (from the greeting)), `extended` (Extended (RSP commands)), `compat` (rtl_tcp commands only) | Auto uses the RSP commands when the server greets in extended mode (rsp_tcp started with its extended-mode option), and plain rtl_tcp commands otherwise. Extended insists on the extended greeting. rtl_tcp commands only: for servers that misbehave with the RSP commands. |
| `ppm` | Frequency correction | whole number (ppm) | 0 | -200 to 200 | The clock error of the radio in ppm, positive when the clock runs fast. |
| `antenna` | Antenna | choice | `first` | `first` (First input), `a` (Antenna A), `b` (Antenna B), `c` (Antenna C), `hiz` (Hi-Z), `t1` (Tuner 1 (50 ohm)), `t2` (Tuner 2 (50 ohm)), `t1hiz` (Tuner 1 Hi-Z) | The input the antenna is on. Hi-Z inputs are for 1 kHz to 30 MHz. On the RSPduo, the tuner choice of the server can limit this. A model that lacks the input refuses it. |
| `gainsplit` | Gain split | choice | `sensitivity` | `sensitivity` (Sensitivity), `linearity` (Linearity) | How one gain number is shared between the LNA and the IF stage. Sensitivity keeps the LNA gain (best noise figure). Linearity gives up LNA gain first (better with strong neighbouring signals). The samples on the link are 8-bit, so set the gain so that the signal neither clips nor sinks into the noise. |
| `lnastate` | Gain stage: LNA state | whole number | -1 | -1 to 27 | Set the LNA state by hand. -1 follows the gain number. The LNA steps depend on the model and the band. A state the band does not have is refused. |
| `ifgr` | Gain stage: IF gain reduction | whole number (dB) | -1 | -1 to 59 | Set the IF gain reduction by hand (20 to 59 dB). -1 follows the gain number. |
| `agcsetpoint` | AGC set point | whole number (dBFS) | -30 | -60 to -20 | The level the AGC of the radio holds the signal at. The AGC runs when automatic gain is on. |
| `rfnotch` | FM/MW notch | yes or no | false |  | Cut strong broadcast (FM, MW) or DAB band III signals that overload the radio. Leave off to receive those bands. |
| `dabnotch` | DAB notch | yes or no | false |  | Cut strong broadcast (FM, MW) or DAB band III signals that overload the radio. Leave off to receive those bands. |
| `amnotch` | Hi-Z MW notch | yes or no | false |  | Cuts strong MW broadcast signals on the Hi-Z input of the RSPduo. Leave off to receive MW. |
| `refout` | Clock output | yes or no | false |  | The 24 MHz clock of the radio on its output socket, to lock another radio. |
| `server_rate` | Rate of the server | frequency | 0 Hz | 0 Hz to 40 MHz | The rate the server sends whatever it is asked. The driver finds it. 0 means none was found. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cu8 | 62.5 kHz to 10 MHz | 1 |
| Iq | Cs16 | 62.5 kHz to 10 MHz | 1 |

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
| `extended` | RSP commands | yes or no |
| `version` | Version | text |
| `model` | Model | text |
| `format` | Sample format | choice |
| `controls` | Controls | text |
| `tuner` | Tuner type | whole number |
| `gains` | Gain count | whole number |
| `throughput` | Link | number (Mbit/s) |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:rsp_tcp <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 1234) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source rsp_tcp:192.168.1.20
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
| `extended` | RSP commands | yes or no |
| `version` | Version | text |
| `model` | Model | text |
| `format` | Sample format | choice |
| `controls` | Controls | text |
| `tuner` | Tuner type | whole number |
| `gains` | Gain count | whole number |
| `throughput` | Link | number (Mbit/s) |

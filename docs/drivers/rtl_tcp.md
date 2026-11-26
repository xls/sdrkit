<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# rtl_tcp server

An rtl_tcp server, which shares an RTL-SDR receiver on the network.

Driver key: `rtl_tcp`.

## Address

- `rtl_tcp:<host>`: port 1234.
- `rtl_tcp:<host>:<port>`

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
| `ppm` | Frequency correction | number (ppm) | 0 | -200 to 200 | The clock error in ppm, positive if the clock runs fast (signals show up a little low). Dongles without a TCXO are often 20 to 100 ppm off. |
| `ds` | Direct sampling | choice | `auto` | `auto` (Auto), `off` (Off), `i` (I input), `q` (Q input) | I or Q samples HF directly below 24 MHz (a V3 has its HF input on Q). Auto leaves it to the server: a Blog V4 upconverts HF itself. |
| `server` | Server radio | choice | `auto` | `auto` (Auto), `rtl` (RTL-SDR (up to 2.56 Msps)), `other` (Other radio (up to 20 Msps)), `any` (Any rate (0.1 to 40 Msps)) | RTL-SDR: a real RTL-SDR, which drops samples above 2.56 Msps. Other radio: any rate from 0.25 to 20 Msps. Any rate: 0.1 to 40 Msps; 8-bit I/Q at 10 Msps is 160 Mbit/s, more than most Wi-Fi gives. Auto: an RTL-SDR unless the server greets as another radio or the stack needs more than 2.56 Msps. |
| `server_rate` | Rate of the server | frequency | 0 Hz | 0 Hz to 40 MHz | The rate the server sends whatever it is asked. The driver finds it. 0 means none was found. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cu8 | 225.001 kHz to 40 MHz | 1 |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:rtl_tcp <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 1234) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source rtl_tcp:192.168.1.20:1234
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

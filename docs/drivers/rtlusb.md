<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# RTL-SDR dongle (USB)

An RTL2832U dongle (RTL-SDR) on a USB port of this computer.

Driver key: `rtlusb`.

## Address

- `rtlusb:`: the first free dongle.
- `rtlusb:<serial>`: the dongle with this serial number.
- `rtlusb:#<n>`: the n-th dongle, from 0.
- `rtlusb:@<bus>-<port>.<port>`: the dongle at this USB port.

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
| `rtl_agc` | RTL2832U digital gain | yes or no | false |  | The digital gain stage of the RTL2832U after the ADC. Off: the samples keep the level that the tuner gives. |
| `tuner_agc` | Tuner AGC loops | yes or no | false |  | With automatic gain on, let the detectors of the tuner set its gain. Off: the driver holds a fixed tuner gain that it chooses from the level of the samples, which keeps strong stations nearby from overloading the dongle. |
| `ppm` | Frequency correction | number (ppm) | 0 | -200 to 200 | The clock error of the dongle in ppm, positive if its clock runs fast (signals then show a little low). Dongles without a TCXO are often 20 to 100 ppm off. |
| `offset_tuning` | Offset tuning | yes or no | false |  | Tune an E4000 tuner 85 % of half the sample rate away and shift back in the demodulator, so that the DC spike of the tuner is not at the centre. An R820T or R828D tuner has an IF of its own and does not use this. |
| `ds` | Direct sampling | choice | `off` | `off` (Off), `i` (I input), `q` (Q input) | Below 24 MHz, sample the antenna input on the I or the Q input of the ADC and leave the tuner off. An RTL-SDR Blog V3 has its HF input on Q; other dongles need a change to the board. A Blog V4 receives HF through its own upconverter and does not use this. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cu8 | 225.001 kHz to 3.2 MHz | 1 |

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
| `tuner` | Tuner | text |
| `pll_locked` | PLL locked | yes or no |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:rtlusb <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `discover` | Find dongles |  |  | Lists the RTL2832U dongles on USB. A dongle that no program has open is opened for a moment to read which tuner it has. |
| `address` | Build an address | `device` (text, default empty) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Dongles | `address` (text), `name` (text), `serial` (text), `tuner` (text), `path` (text), `state` (text) |

## Try it

```sh
sdrkit run fm --source usb:rtl:
```
<!-- generated: end -->

## The address with the scheme `usb:`

`usb:rtl:<selector>` names the same dongles: `usb:rtl:` is the first free dongle, `usb:rtl:<serial>`, `usb:rtl:#<n>` and `usb:rtl:@<bus>-<port path>` name one. The engine table `devices` lists the attached dongles with these addresses, and `sdrkit devices` prints it.

## What the dongle needs from the system

- Windows: the dongle must be bound to the WinUSB driver once. Until then its state is `needs_driver`.
- Linux: a udev rule lets a user who is not root open the dongle. The television (DVB-T) driver of the kernel takes the dongle when it is attached; the library detaches it while the dongle is open.
- macOS: no setup.
- iPhone and iPad: no USB access. Open the dongle through a server on the network (`rtl_tcp:<host>:<port>`).

[USB radios and serving a radio to the network](../serving.md) has the steps, the udev rule and the reasons that an open reports.

## Notes

- The samples are unsigned 8-bit I/Q. A share serves them to rtl_tcp clients with the bytes that the dongle gave.
- A retune drops the samples that the tuner made before it settled, and the radio marks the place as a gap.
- Direct sampling (`ds`) receives below 24 MHz on a dongle that has an HF input.

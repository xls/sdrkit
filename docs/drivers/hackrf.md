<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HackRF

A HackRF One, HackRF Pro, Jawbreaker or rad1o on a USB port of this computer. It transmits.

Driver key: `hackrf`.

## Address

- `hackrf:`: the first free HackRF.
- `hackrf:<serial>`: or its last digits.
- `hackrf:#<n>`: the n-th HackRF, from 0.
- `hackrf:@<bus>-<port path>`

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
| `lna_db` | Gain stage: LNA | whole number (dB) | -1 | -1 to 40 | The receive IF gain, 0 to 40 dB in 8 dB steps; a value between steps is rounded down. -1 follows the gain. Start at 16 dB and raise LNA and VGA by about the same amount. |
| `vga_db` | Gain stage: VGA | whole number (dB) | -1 | -1 to 62 | The receive baseband gain, 0 to 62 dB in 2 dB steps; an odd value is rounded down. -1 follows the gain. |
| `amp` | RF amplifier | yes or no | false |  | The front-end amplifier, about 11 dB, less at high frequencies. The antenna port takes at most -5 dBm: more can damage the device. Use an attenuator for strong signals. |
| `clock_out` | Clock output | yes or no | false |  | A 10 MHz, 3.3 V square wave on CLKOUT for a high-impedance load, to lock another device. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cs8 | 2 MHz to 20 MHz | 1 |

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
| `board` | Board | text |
| `board_id` | Board id | whole number |
| `platforms` | Firmware platforms | text |
| `firmware` | Firmware | text |
| `usb_api` | USB API | text |
| `part_id` | Part id | text |
| `serial` | Serial number | text |
| `clock_in` | Clock | text |
| `dc_spike` | DC spike at the centre | yes or no |
| `tune_offset_hz` | Advised tune offset | frequency |
| `rx_to_tx_ms` | Receive to transmit | number (ms) |
| `tx_to_rx_ms` | Transmit to receive | number (ms) |
| `shortfalls` | Device shortfalls | whole number |
| `measured_rate_hz` | Measured sample rate | frequency |
| `tx_output_dbm` | Output (estimate, not measured) | number (dBm) |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:hackrf <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `discover` | Find HackRF devices | `probe` (yes or no, default true) |  | Lists the HackRF devices on the USB bus, with a device in DFU mode marked as not usable. |
| `address` | Build an address | `device` (text, default empty) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | HackRF devices | `address` (text), `name` (text), `board` (text), `firmware` (text), `serial` (text), `path` (text), `state` (text) |

## Transmit

The radio can transmit. The range of the transmit power and the power that the transmitter starts with come from the radio when it opens: the bottom of the range for a radio that can drive an amplifier, 10 dB below full power for a low-power board.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tx_frequency` | Transmit frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `tx_rate` | Transmit rate | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_bandwidth` | Transmit bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_power` | Transmit power | number (dB) | -10 | -200 to 0 | The output relative to the full output of the radio: 0 dB is full power, -10 dB is a tenth of it. |
| `tx_amp` | Transmit amplifier | yes or no | false |  | The front-end amplifier while transmitting, about 11 dB. Off, the output is about 11 dB below the typical figures of the board. |

Streams it accepts:

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cf32 | 2 MHz to 20 MHz | 1 |

## Try it

```sh
sdrkit run fm --source usb:hackrf:
```
<!-- generated: end -->

## The address with the scheme `usb:`

`usb:hackrf:<selector>` names the same devices: `usb:hackrf:` is the first free HackRF, `usb:hackrf:<serial>` (or its last digits), `usb:hackrf:#<n>` and `usb:hackrf:@<bus>-<port path>` name one. The engine table `devices` lists the attached devices with these addresses, and `sdrkit devices` prints it.

## What the device needs from the system

- Windows: a HackRF with recent firmware tells Windows to use the WinUSB driver. An older one must be bound to WinUSB once.
- Linux: a udev rule lets a user who is not root open the device.
- macOS: no setup.
- iPhone and iPad: no USB access. Open the device through a server on the network (`soapyremote:<host>`).

[USB radios and serving a radio to the network](../serving.md) has the steps, the udev rule and the reasons that an open reports.

## Notes

- The HackRF is half duplex: it receives or it transmits. While it transmits, the receive stream has a gap. The radio marks the gap; it is not counted in "Lost by the radio".
- Transmit has one switch, the engine setting `transmit_allowed`. The driver adds none.
- A device in DFU mode (firmware update) is listed by the command `discover` of the driver and cannot be opened.
- The antenna port takes at most -5 dBm. More can damage the device.

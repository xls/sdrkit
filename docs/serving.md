# USB radios and serving a radio to the network

This page describes two parts of the engine: the radios that are attached to a USB port of this computer, and the share, which serves the samples of any open radio to clients on the network.

The pages of the two USB drivers list their settings and readings: [RTL-SDR dongle](drivers/rtlusb.md) and [HackRF](drivers/hackrf.md).

## USB radios

The library talks to a USB radio itself. It needs no driver library of the maker.

A USB radio has the address `usb:<family>:<selector>`:

| Address | The radio |
|---|---|
| `usb:rtl:` | The first RTL2832U dongle that is free. |
| `usb:hackrf:` | The first HackRF that is free. |
| `usb:rtl:<serial>` | The device with this serial number. A HackRF also takes the last digits of its serial number. |
| `usb:rtl:#<n>` | The n-th device of its family in the order of the bus, counted from 0. |
| `usb:rtl:@<bus>-<port path>` | The device at this USB port, for example `usb:rtl:@1-2.3`. |

The addresses `rtlusb:<selector>` and `hackrf:<selector>` name the same radios by the key of their driver.

### The table of devices

The engine lists the attached USB radios in its table `devices`. Listing opens no device.

| Column | What it holds |
|---|---|
| `address` | The address that opens the device. It finds the same device after it was unplugged and plugged in again. |
| `driver` | The driver kind that takes the device. |
| `name`, `serial`, `path` | The name, the serial number and the USB port of the device. |
| `state` | `free`, `open`, `no_permission`, `needs_driver` or `busy`. |
| `radio` | The radio that the device is open as, when it is open. |

The engine raises the event `device.arrived` when a device is attached and `device.left` when it is removed. A program that shows a list of radios reads the table and listens for these two events.

The state `busy` and the state `no_permission` on Linux are known only after an open of the device failed, because the engine does not open a device to list it.

`sdrkit devices` prints the table. `sdrkit devices --follow` also prints each arrival and removal.

### When a radio cannot be used

An open that cannot work on this system fails with the error "unavailable" (`SDRKIT_E_UNAVAILABLE` in C) and one of these reasons:

| Reason | What it means | What to do |
|---|---|---|
| `no_usb_here` | The platform gives a program no USB access. | Use the radio through a server on the network. See "iPhone and iPad" below. |
| `no_permission` | The user may not open the device. | Linux: add the udev rule below. Android: allow the access when the system asks. |
| `kernel_driver` | A driver of the system holds the device and did not release it. | Linux: see the note on the kernel driver below. |
| `needs_winusb` | Windows has not bound the device to its WinUSB driver. | Bind it once. See "Windows" below. |
| `not_found` | No attached device matches the address. | Check the cable and the address. `sdrkit devices` lists what is attached. |

The engine table `drivers` has the columns `available` and `unavailable_why`. On a platform without USB access, the two USB kinds are listed with `available` = no and the reason `no_usb_here`.

### Windows: bind the device to WinUSB

Windows lets a program talk to a USB device only through a driver. The library uses WinUSB, the generic USB driver that is part of Windows.

- A HackRF with recent firmware tells Windows to use WinUSB. You do nothing.
- An RTL2832U dongle does not. Windows installs a television driver for it or no driver. Bind the interface "Bulk-In, Interface (Interface 0)" of the dongle to WinUSB once, with a tool that installs the WinUSB driver for one USB device. After that the dongle shows the state `free`.

Until a device is bound, its row in the table `devices` has the state `needs_driver`, and an open fails with the reason `needs_winusb`.

### Linux: the udev rule and the kernel driver

A user who is not root may open a USB device only if a udev rule allows it. Put these lines in `/etc/udev/rules.d/60-sdrkit.rules`:

```
# RTL2832U dongles
SUBSYSTEM=="usb", ATTRS{idVendor}=="0bda", ATTRS{idProduct}=="2832", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="0bda", ATTRS{idProduct}=="2838", MODE="0660", TAG+="uaccess"
# HackRF One, Jawbreaker, rad1o
SUBSYSTEM=="usb", ATTRS{idVendor}=="1d50", ATTRS{idProduct}=="6089", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="1d50", ATTRS{idProduct}=="604b", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="1d50", ATTRS{idProduct}=="cc15", MODE="0660", TAG+="uaccess"
```

Then run `sudo udevadm control --reload-rules` and plug the device in again. A dongle with other USB ids needs a line with its ids; `lsusb` prints them.

The Linux kernel has a television (DVB-T) driver for the RTL2832U. It takes the dongle when the dongle is attached. The library detaches that driver from the dongle for the time that the dongle is open, so you do not have to block the kernel driver. If the detach is refused, the open fails with the reason `kernel_driver`.

### macOS

No setup. A device that another program has open shows the state `busy` after an open failed.

### iPhone and iPad

iOS gives an application no access to a USB radio. The two USB kinds are in the table `drivers` with `available` = no and the reason `no_usb_here`, and the table `devices` is empty. On these devices, use a radio that another computer serves: run `sdrkit serve` there and open `rtl_tcp:<host>:<port>` or `soapyremote:<host>` here.

### A record of the USB exchanges

`sdrkit usb record <source> --out <file>` opens a USB radio, lets it stream for `--seconds` (2 by default) and writes every USB exchange into a script file: the control transfers in order and the bulk data cut short. The tests of a driver replay such a script on a bus without hardware. Attach the script to a report of a dongle that does not work.

```sh
sdrkit usb record usb:rtl: --out dongle.usbscript --freq 104.9 --rate 2.4M
```

## Serving a radio: the share

A share serves the samples of one open radio to clients on the network. It speaks two protocols:

- rtl_tcp: unsigned 8-bit samples over one TCP connection. Receive only.
- SoapyRemote: calls over TCP and samples over UDP, or over TCP when the client asks for it, in 8-bit, 16-bit or float format. Receive, and transmit on a radio that transmits.

A radio has at most one share. A share can have several listeners, for example one for each protocol.

### What a share does to the radio

A share takes the samples as the radio gives them. It never sets the sample rate or the frequency of the radio by itself, and it never stops a stack of this program from doing so. You can run a stack on the same radio at the same time.

A client that is slow loses only its own samples. Each client has its own queue (`client_queue_ms`); a full queue drops samples for that client alone. The radio and the stacks of this program are not held up. A client that loses samples for longer than `drop_after_s` is disconnected.

Samples of 8 and 16 bits reach a client with the bytes that the radio gave.

### Settings

| Setting | Default | What it does |
|---|---|---|
| `control` | `follow` | Who steers the radio. See below. |
| `max_clients` | 8 | The most clients at one time. |
| `client_queue_ms` | 500 | The time of signal that waits for a slow client. |
| `drop_after_s` | 5 | A client that loses samples for this long is disconnected. |
| `allow` | private ranges | The address ranges that may connect, separated by commas. Empty allows every address. |
| `remote_gain` | on | With the control `remote`, the steering client may set the gain. |
| `format` | `native` | The sample format in which the samples wait for the clients: `native`, `cu8`, `cs8`, `cs16`, `cf32`. |

A change of `max_clients`, `client_queue_ms`, `drop_after_s`, `allow` or `format` disconnects the clients; the listeners stay on their ports. When the sample rate of the radio changes while clients are connected, the clients stay, and the share sizes its queues for the new rate when the last client has left.

### Who steers the radio

| `control` | What a client can change |
|---|---|
| `follow` | Nothing. A request to tune, to set the rate or the gain is not carried out and is counted. The client is told the value in force, so a client that sets the radio when it connects stays connected and shows what it gets. |
| `remote` | One client sets the frequency, the rate, the bandwidth and (with `remote_gain`) the gain of the radio, while no stack of this program reads the radio. It is the client that the command `grant` names, or else the client that connected first. When a stack of this program starts on the radio, the stack steers and the client gets the answer of `follow`. A rate that the user of this program set on the radio is not changed. |
| `slice` | Each client gets its own frequency and rate inside what the radio captures. The radio does not move. A request outside the capture is refused. |

### Who may connect, and who may transmit

A listener takes every interface of the computer unless you give it one address. The setting `allow` then limits who may connect. Its default is this computer and the private address ranges: `127.0.0.0/8`, `::1`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, `fc00::/7`, `fe80::/10`. A connection from another address is refused and counted.

The two protocols have no password and no encryption. Do not allow addresses of the internet.

No client transmits by default. A SoapyRemote client transmits only when both of these hold:

1. The command `grant` of the share gave this client the right (`transmit=true`).
2. The engine setting `transmit_allowed` is on. It is the one transmit switch of the engine.

The samples of the client then go out through the transmitter of the radio, one client at a time.

### Commands

| Command | What it does |
|---|---|
| engine `share.open radio=<radio key>` | Opens the share of a radio. The reply holds the key of the share, `share.h<n>`. |
| engine `share.close share=<share key>` | Closes the share. Closing the radio closes its share first. |
| `listen protocol=<rtl_tcp\|soapyremote> port=<port> [host=<address>]` | Starts a listener. Port 0 lets the system choose; the reply states the port. The usual ports are 1234 for rtl_tcp and 55132 for SoapyRemote. |
| `unlisten listener=<number>` | Stops a listener. Its clients stay. |
| `kick client=<number>` | Disconnects a client. |
| `grant client=<number> [steer=true] [transmit=true]` | Sets what a client may do beyond listening. |

### Results and events

The fields of a share show its format, rate and centre, the count of clients and listeners, the samples queued, the requests done and refused, and whether a client transmits. The table `listeners` has one row for each listener, and the table `clients` one row for each client with its address, the bytes and samples sent, the samples dropped for it, its slice, and whether it steers or may transmit.

The events are `share.client_joined`, `share.client_left` (with the reason: closed, too slow, kicked, share stopped), `share.refused` (address not allowed, too many clients) and `share.listen_failed`.

### Discovery

A SoapyRemote listener on an address that other computers reach answers the searches of SoapyRemote clients (SSDP on UDP port 1900), so the radio shows in their lists. A listener on `127.0.0.1` does not answer searches. If the port 1900 cannot be opened, the listener works without discovery, and a client names the address.

### From the command line

```sh
# An RTL dongle on FM for rtl_tcp clients, port 1234 on every interface:
sdrkit serve usb:rtl: --freq 104.9 --rate 2.4M

# Listen to FM here and serve the same dongle to rtl_tcp and SoapyRemote clients:
sdrkit serve usb:rtl: --stack fm --freq 104.9 --rtl-tcp 1234 --soapy 55132

# Let one client steer a HackRF, for this computer only:
sdrkit serve usb:hackrf: --control remote --soapy 127.0.0.1:55132

# The test signal of the FM stack as an rtl_tcp server, to try a client without a radio:
sdrkit serve synth:fm --rate 1.024M --rtl-tcp 127.0.0.1:1234
```

`sdrkit serve` prints where it listens, each client that comes and goes, and a status line. Ctrl-C stops it. On another computer, open the radio with `sdrkit run fm --source rtl_tcp:<host>:1234 --freq 104.9`.

### From a program

In C, `sdrkit_share_open(radio, &share)` opens the share and `sdrkit_share_close(share)` closes it. The header `sdrkit/share.h` has the typed calls for its settings, fields, tables, events and commands. In Swift, `engine.openShare(radio:)` returns a `Share`, and the file `Share.swift` has the same typed calls. In Rust, `Engine::open_share(radio)` returns the `ShareId` of the owner `OwnerRef::Share`.

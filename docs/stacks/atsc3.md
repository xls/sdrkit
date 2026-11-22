<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ATSC 3.0

ATSC 3.0 (NextGen TV), 6 MHz channel, ROUTE services

Stack id: `atsc3`. Group: TV.

Signal types: ATSC 3.0.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 539 MHz |
| Tuning range | 174 MHz to 806 MHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 6.5 MHz |
| Signal bandwidth | 6 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:atsc3` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `plp` | PLP | whole number | -1 | -1 to 63 | The id of the PLP to decode. -1 takes the first core PLP with low level signalling that the stack decodes, or without one the first core PLP that it decodes. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bootstrap_seen` | Bootstrap seen | yes or no |
| `bootstraps` | Bootstraps found | whole number |
| `frames_taken` | Frames | whole number |
| `frames_ok` | Frames good | whole number |
| `offset_hz` | Offset | frequency |
| `minor_version` | Minor version | whole number |
| `system_bandwidth` | Channel | choice |
| `preamble_structure` | Preamble structure | whole number |
| `min_time_next` | Minimum time to the next frame | duration (s) |
| `post_rate` | Post-bootstrap sample rate | frequency |
| `ea_wake_up` | Emergency alert wake-up | yes or no |
| `frame_length` | Frame length | duration (s) |
| `lls_tables` | Service list tables | whole number |
| `signed_signalling` | Signalling signed | yes or no |
| `leap_seconds` | Leap seconds | whole number (s) |
| `service` | Service chosen | whole number |
| `service_received` | Service received | whole number |
| `route_objects` | ROUTE objects | whole number |
| `media_samples` | Media samples | whole number |
| `step_bootstrap` | Bootstrap step | choice |
| `step_l1` | L1 step | choice |
| `step_service` | Service step | choice |
| `frames_failed` | Frames not decoded | whole number |
| `not_decoded` | Not decoded | choice |
| `fft_size` | FFT size | whole number |
| `guard_samples` | Guard interval | whole number (samples) |
| `l1_basic_mode` | L1-Basic mode | whole number |
| `l1_detail_mode` | L1-Detail mode | whole number |
| `preamble_symbols` | Preamble symbols | whole number |
| `subframes` | Subframes | whole number |
| `subframe_symbols` | Symbols of the first subframe | whole number |
| `pilot_dx` | Scattered pilot carrier spacing | whole number |
| `pilot_dy` | Scattered pilot symbol period | whole number |
| `plps` | PLPs | whole number |
| `plp_id` | PLP decoded | whole number |
| `plp_points` | PLP constellation points | whole number |
| `plp_code_rate` | PLP code rate | whole number (/15) |
| `plp_ldpc_bits` | PLP LDPC code word | whole number (bit) |
| `fec_blocks` | FEC blocks in the last frame | whole number |
| `fec_blocks_good` | FEC blocks decoded in the last frame | whole number |
| `baseband_packets` | Baseband packets | whole number |
| `baseband_packets_bad` | Baseband packets bad | whole number |
| `outer_corrected` | BCH corrections | whole number |
| `ldpc_iterations` | LDPC iterations | number |
| `mer_db` | MER (PLP cells) | number (dB) |
| `pilot_snr_db` | Pilot SNR | number (dB) |
| `step_plp` | PLP step | choice |
| `alp.alp_packets` | ALP packets | whole number |
| `alp.alp_dropped` | ALP packets dropped | whole number |
| `alp.compressed_ip` | Compressed IP packets | whole number |
| `ip.ip_packets` | IP packets | whole number |
| `ip.ip_bad` | Bad IP packets | whole number |
| `ip.reassembled` | Reassembled | whole number |
| `ip.flows` | Flows | whole number |
| `ip.encrypted_flows` | Encrypted flows | whole number |
| `route.objects` | Objects | whole number |
| `route.objects_dropped` | Objects dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `services` | Services | `major` (whole number), `minor` (whole number), `name` (text), `hidden` (yes or no), `id` (whole number), `category` (choice), `delivery` (choice), `action` (choice), `sls_objects` (whole number), `sls_documents` (whole number), `sls_signed_packages` (whole number) |
| `components` | Media components | `tsi` (whole number), `fourcc` (text) |
| `plps` | PLPs | `subframe` (whole number), `id` (whole number), `layer` (whole number), `points` (whole number), `code_rate` (whole number (/15)), `ldpc_bits` (whole number (bit)), `cells` (whole number), `time_interleaver` (whole number), `decoded` (yes or no), `bonding` (whole number), `bonded_bsid` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |

## Typed interface

The stack is in the family `tvterr`, version 1.5. The C header `sdrkit/tvterr.h`, the registry record `registry/tvterr.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `Atsc3Fields`. The result fields of `atsc3`: the bootstrap, the frames, the service list and the media. A value of the bootstrap has no value before a bootstrap is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bootstrap_seen` | yes or no | always | 1.0 | A bootstrap was seen recently. |
| `bootstraps` | whole number | always | 1.0 | Bootstraps found. |
| `frames_taken` | whole number | always | 1.0 | Frames taken. |
| `frames_ok` | whole number | always | 1.0 | Frames decoded. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last bootstrap. |
| `minor_version` | whole number | optional | 1.0 | The minor version of the bootstrap. |
| `system_bandwidth` | choice | optional | 1.0 | The system bandwidth that the bootstrap signals. |
| `preamble_structure` | whole number | optional | 1.0 | The preamble structure, 0 to 159. |
| `min_time_next` | duration (s) | optional | 1.0 | The minimum time to the next frame. |
| `post_rate` | frequency | optional | 1.0 | The sample rate after the bootstrap. |
| `ea_wake_up` | yes or no | optional | 1.0 | The emergency alert wake-up. |
| `frame_length` | duration (s) | optional | 1.0 | The time between the last two bootstraps. |
| `lls_tables` | whole number | always | 1.0 | Service list tables read. |
| `signed_signalling` | yes or no | always | 1.0 | Signed multi-tables or signed signalling packages arrived in the last 10 s. |
| `leap_seconds` | whole number (s) | optional | 1.0 | TAI minus UTC from the system time table. |
| `service` | whole number | optional | 1.0 | The service chosen with `service.select`. No value for the automatic choice. |
| `service_received` | whole number | optional | 1.0 | The service received: the chosen one, or the first visible linear TV service delivered by ROUTE. |
| `route_objects` | whole number | always | 1.0 | ROUTE objects taken. |
| `media_samples` | whole number | always | 1.0 | Media samples given out. |
| `step_bootstrap` | choice | always | 1.0 | The bootstrap step. |
| `step_l1` | choice | always | 1.0 | The L1 step. |
| `step_service` | choice | always | 1.0 | The service signalling step. |

### Result fields

Struct `Atsc3FrameFields`. The result fields of `atsc3` about the frames: what the L1 signalling says, the PLP that is decoded and its FEC blocks. A value of the signalling has no value before L1-Basic is read, a value of the PLP none before a FEC block is decoded.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_failed` | whole number | always | 1.2 | Frames that were not decoded. |
| `not_decoded` | choice | optional | 1.2 | Why the last frame was not decoded. No value after a decoded frame. |
| `fft_size` | whole number | optional | 1.2 | The FFT size of the Preamble and the first subframe, points. |
| `guard_samples` | whole number (samples) | optional | 1.2 | The guard interval of the Preamble and the first subframe. |
| `l1_basic_mode` | whole number | optional | 1.2 | The L1-Basic mode that the bootstrap signals, 1 to 5. |
| `l1_detail_mode` | whole number | optional | 1.2 | The L1-Detail mode, 1 to 7. |
| `preamble_symbols` | whole number | optional | 1.2 | The Preamble symbols of the frame. |
| `subframes` | whole number | optional | 1.2 | The subframes of the frame. The first one is decoded. |
| `subframe_symbols` | whole number | optional | 1.2 | The data and subframe boundary symbols of the first subframe. |
| `pilot_dx` | whole number | optional | 1.2 | The spacing of the carriers that bear scattered pilots in the first subframe. |
| `pilot_dy` | whole number | optional | 1.2 | The symbols of one scattered pilot sequence in the first subframe. |
| `plps` | whole number | optional | 1.2 | The PLPs of the frame: the different PLP ids in all its subframes. |
| `plp_id` | whole number | optional | 1.2 | The id of the PLP that is decoded. |
| `plp_points` | whole number | optional | 1.2 | The constellation points of the PLP that is decoded: 4, 16, 64 or 256. |
| `plp_code_rate` | whole number (/15) | optional | 1.2 | The code rate of the PLP that is decoded, in fifteenths. |
| `plp_ldpc_bits` | whole number (bit) | optional | 1.2 | The LDPC code word of the PLP that is decoded: 16200 or 64800 bits. |
| `fec_blocks` | whole number | always | 1.2 | FEC blocks of the PLP that ended in the last frame. |
| `fec_blocks_good` | whole number | always | 1.2 | Those of them that decoded. |
| `baseband_packets` | whole number | always | 1.2 | Baseband packets given out. |
| `baseband_packets_bad` | whole number | always | 1.2 | FEC blocks whose baseband packet was not good. |
| `outer_corrected` | whole number | always | 1.2 | Bits that the outer BCH code corrected after the LDPC decoder. |
| `ldpc_iterations` | number | optional | 1.2 | The mean LDPC iterations of the FEC blocks of the last frame. |
| `mer_db` | number (dB) | optional | 1.2 | The MER of the cells of the decoded PLP against the nearest constellation points. |
| `pilot_snr_db` | number (dB) | optional | 1.2 | The signal to noise ratio that the pilots of the first subframe give. |
| `step_plp` | choice | always | 1.2 | The PLP step: green when more than 99 % of the baseband packets are good, amber above 50 %, red below. |

### Table `services`

Struct `Atsc3ServiceRow`. A row of the services of the service list. The row key is the service id; `service.select` takes it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `major` | whole number | always | 1.0 | The major channel number. |
| `minor` | whole number | always | 1.0 | The minor channel number. |
| `name` | text | always | 1.0 | The short name. |
| `hidden` | yes or no | always | 1.0 | The service is hidden. |
| `id` | whole number | always | 1.0 | The service id. |
| `category` | choice | always | 1.0 | The category of the service. |
| `delivery` | choice | always | 1.0 | How the signalling is delivered. |
| `action` | choice | always | 1.0 | What the receiver does with the service. |
| `sls_objects` | whole number | always | 1.0 | Service layer signalling objects received on TSI 0 of the ROUTE session of the service. |
| `sls_documents` | whole number | always | 1.0 | Signalling documents (USBD, S-TSID, MPD and others) read from those objects. |
| `sls_signed_packages` | whole number | always | 1.0 | Those objects that were signed packages. The signature is not verified. |

### Table `components`

Struct `Atsc3ComponentRow`. A row of the media components seen, in the order they appeared. The row key is the ROUTE session.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tsi` | whole number | always | 1.0 | The ROUTE session of the component. |
| `fourcc` | text | always | 1.0 | The four-character code of the codec of the component. |

### Table `plps`

Struct `Atsc3PlpRow`. A row of the PLPs of the last frame: one row for each PLP of each subframe. The row key is the subframe number, a point and the PLP id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `subframe` | whole number | always | 1.4 | The number of the subframe in the frame, from 0. |
| `id` | whole number | always | 1.2 | The PLP id. |
| `layer` | whole number | always | 1.2 | The layer: 0 core, above 0 enhanced. |
| `points` | whole number | optional | 1.2 | The constellation points, also of a coding that the stack does not decode. No value for a reserved code. |
| `code_rate` | whole number (/15) | optional | 1.2 | The code rate in fifteenths. |
| `ldpc_bits` | whole number (bit) | optional | 1.2 | The LDPC code word, bits. |
| `cells` | whole number | always | 1.2 | The data cells of the PLP in the subframe. |
| `time_interleaver` | whole number | always | 1.2 | The time interleaver: 0 none, 1 convolutional, 2 hybrid. |
| `decoded` | yes or no | always | 1.2 | The PLP is the one that the stack decodes. |
| `bonding` | whole number | optional | 1.5 | The channel bonding format (table 9.25): 0 plain, 1 SNR averaging, 2 and 3 reserved. No value when the PLP is not bonded with another RF channel. |
| `bonded_bsid` | whole number | optional | 1.5 | The BSID of the RF channel that the PLP is bonded with, as L1-Detail lists it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `service.select` | `sdrkit_atsc3_service_select` | `sdrkit_tvterr_service_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `plp` | `sdrkit_atsc3_get_plp` | `sdrkit_atsc3_set_plp` |

## Try it

```sh
sdrkit run atsc3 --source synth:atsc3
sdrkit run atsc3 --source iiod:192.168.2.1 --freq 539
```
<!-- generated: end -->

## Description

An ATSC 3.0 frame is a bootstrap, a Preamble of one or more OFDM symbols with the L1 signalling, and one or more subframes of
OFDM symbols that carry the PLPs. The stack works at 6.144 Msps, the rate of the bootstrap. It works in these steps:

1. `atsc3.sync` finds each bootstrap and reads its fields: the start of the frame, the carrier offset and the preamble
   structure (FFT size, guard interval, pilot spacing and L1-Basic mode of the Preamble).
2. `atsc3.frame` takes the samples of the frame. The guard interval of the first Preamble symbol gives the carrier offset
   that the bootstrap left. Each Preamble symbol is transformed, its channel is estimated from the Preamble pilots, and its
   data cells are equalised and frequency de-interleaved.
3. L1-Basic (200 bits) is decoded from the first cells: demapping, the bit demultiplexer, repetition and puncturing undone,
   the 16200-bit LDPC code at rate 3/15, the BCH code, descrambling and the CRC. L1-Detail follows in the same way, from one
   or more coded blocks spread over the Preamble symbols.
4. The signalling gives the symbols of every subframe, each with its own FFT size, guard interval, pilot pattern and
   boundary symbols. The stack takes the subframes one after the other. For each: data symbols with scattered, continual and edge pilots, and
   subframe boundary symbols with denser pilots and null cells. The channel of each symbol is the pilots of that symbol and,
   between them, the mean of the pilots of the symbols before and after; between pilot carriers it is a straight line after
   the mean delay is taken out. The continual pilots measure the noise.
5. The data cells of the first subframe are numbered from the cells of the last Preamble symbol after the signalling,
   those of a later subframe from its first symbol. A
   non-dispersed PLP is one run of them. A dispersed PLP is cut into subslices, one every `L1D_plp_subslice_interval`
   cells; the last subslice can be shorter. The stack takes the PLP whose id the setting `plp` gives. At -1 it takes the
   first PLP with low level signalling that it decodes, or without one the first PLP that it decodes.
6. `atsc3.fec` undoes the time interleaver of the PLP, as L1-Detail gives it:
   - The convolutional time interleaver: the cells go through delay lines of 512 to 1448 rows that last from frame to frame.
     The start row of each frame must be where the frame before left the rows. If it is not, or a frame is lost, the lines
     start again. The FEC blocks come out when the lines are full: 0.06 s of cells at 512 rows, 0.45 s at 1448 rows.
   - The hybrid time interleaver within a subframe: each TI block is read back from the twisted block interleaver, and
     each FEC block from the cell interleaver when the PLP uses it.
   - The hybrid time interleaver across subframes: the parts of each TI block arrive over 2 to 16 subframes. The TI block
     is read back when its last part is in. A lost frame starts the wait again.
7. `atsc3.fec` cuts the cells into FEC blocks. A block that begins in one frame and ends in the next is joined.
   Each block is demapped, de-interleaved (block, group-wise and parity interleaver), LDPC decoded with up to 50 iterations,
   checked by the BCH code or the CRC, and descrambled: a baseband packet.
8. The baseband packets go to the link layer (`ipcast.alp`, `ipcast.ip`), the service list (`atsc3.lls`) and ROUTE
   (`ipcast.route`, `atsc3.media`).

The state is Locked while frames decode, Tracking while bootstraps are found and no frame decodes, and Searching when no
bootstrap came for 0.5 s (or 2.5 frame lengths). `not_decoded` names why the last frame was not decoded.

## What is decoded

| Part | Decoded | Not decoded |
|---|---|---|
| Sample rate after the bootstrap | 6.144 Msps (sample-rate coefficient 0) | every other rate: 6.912 Msps of most 6 MHz channels, 8.064 Msps, 9.216 Msps |
| L1-Basic | modes 1 to 5 | |
| L1-Detail | modes 1 to 7, several coded blocks, several Preamble symbols | several coded blocks with additional parity |
| First subframe | 8K, 16K and 32K FFT, every guard interval, pattern, boost and carrier reduction, boundary symbols, frequency interleaver | MIMO, MISO, tone reservation, a subframe that ends more than 1 s after the bootstrap |
| Other subframes | as the first, each from its own parameters; the chosen PLP is taken in every subframe that carries it | a subframe that ends more than 1 s after the bootstrap, or with a waveform that the stack does not decode, and the subframes after it |
| PLP | one PLP at a time, chosen by its id, non-dispersed or dispersed: QPSK, 16, 64 and 256 points, every code rate at 16200 and 64800 bits, BCH, CRC or no outer code | more than one PLP at a time, 1024 and 4096 points, LDM (a subframe with an enhanced layer is not decoded), channel bonding |
| Time interleaver | none; convolutional at 512, 724, 887 and 1024 rows, and at 1254 and 1448 rows with extended interleaving; hybrid within a subframe and across subframes (cell interleaver, twisted block interleaver, convolutional delay line) | |
| Above the baseband packets | ALP, IP, UDP, the service list, ROUTE objects, media samples | MMTP; the media is not played |

Tested on the stack's own test signal: L1-Basic modes 1 to 5, L1-Detail modes 1, 2, 5 and 7, one and several coded blocks
and Preamble symbols, symbol-aligned and time-aligned frames, the three FFT sizes, boundary symbols with null cells, carrier
reduction, the frequency interleaver on and off, a PLP of 4, 16, 64 or 256 points at both LDPC lengths with BCH, CRC or no
outer code, FEC blocks that cross from frame to frame, a PLP that begins after other cells, two PLPs in one subframe
(non-dispersed and dispersed), the convolutional time interleaver at its six depths, and the hybrid time interleaver within
a subframe with one and several TI blocks and across 1 to 16 subframes, with and without the cell interleaver. Built from the standard and
not exercised by a signal: the other guard intervals, pilot patterns, code rates and L1-Detail modes, and a subframe without
boundary symbols.

A subframe without boundary symbols was not exercised by a signal.

Open in the time interleavers:

- The hybrid time interleaver across subframes: the standard names the unit that moves between the branches of the delay
  line a row. The stack takes it as as many consecutive cells of the twisted block read-out as the TI block has FEC
  blocks, which is the reading that fits the sizes of the branches. This was not checked against a signal.
- The test signal starts the rows of the convolutional time interleaver with zeros and not with the cells that the
  standard gives. A receiver does not use these cells.

The state is Locked only after a FEC block came out: while a time de-interleaver fills, the state is Tracking.

Limits of the receiver: the channel estimate does not follow echoes longer than about a quarter of the pilot spacing or fast
fading; the carrier offset is taken once per frame; the sample clock is not corrected, so a clock error moves the symbols
within the guard interval over a frame.

## Test signal

`synth:atsc3` sends frames of 0.1016 s at 6.144 Msps, resampled to the output rate:

- the bootstrap with preamble structure 3: 8K FFT, guard 192 samples, Preamble pilots every 16 carriers, L1-Basic mode 4;
- one Preamble symbol with L1-Basic and L1-Detail (mode 2, 25 bytes, 774 cells) and the first cells of the PLP;
- one subframe of 72 symbols with 6913 carriers: scattered pilots SP16_2 at boost 1, a boundary symbol first and last (300
  null cells each), frequency interleaved;
- one PLP of 482596 cells in each frame: QPSK, code rate 8/15, 64800-bit LDPC with BCH, no time interleaver. The FEC blocks
  cross from frame to frame;
- with other options: more subframes, each with its own FFT size, guard interval and pilots; more PLPs, each in one run or in subslices, and for each PLP the convolutional or the hybrid time
  interleaver (within a subframe or across subframes);
- baseband packets of 4296 bytes with ALP packets of UDP datagrams to 239.255.10.1 port 5000. Each datagram holds a counter
  and 1192 bytes that follow from the seed and the counter.

The guard interval of 1024 samples that the specification names as the default is not used: the Preamble and the first subframe have one guard interval
(A/322 clause 7.2.2.1), and preamble structure 3 gives 192 samples.

## Measured

- The stack is Locked within 0.3 s of the test signal at 8 Msps.
- White noise, signal power 1 in 6.144 MHz: every frame decodes at 7 dB. L1-Basic mode 4 sets this limit. With L1-Basic mode 1
  and L1-Detail mode 1 the same PLP decodes at 3.5 dB and the signalling reads at -2 dB.
- 3 kHz carrier offset at 10 dB, at 8 and 20 Msps: no baseband packet is bad.
- About 0.25 of one desktop core at 8 Msps for the whole receiver. Once warm, the receive path allocates nothing.

## Standards

ATSC A/321 (bootstrap), A/322:2023-03 (physical layer), A/330 (link layer), A/331 (signalling and delivery). The CRC of the
L1 signalling is the one of A/322:2023-03 clause 6.1.2.2, x^32 + x^21 + x^16 + x^11 + 1. No signal from a transmitter was
decoded: the tables of A/322 are checked against the counts and the sequences that the standard prints.

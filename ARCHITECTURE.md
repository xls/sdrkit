# sdrkit architecture

This document describes how the library is built: its objects, its streams, the graph, the stacks, the drivers, the
engine and the C API. It is for a developer who changes the library. A developer who only calls the library reads
[README.md](README.md) and the header `crates/hosts/sdrkit-host-ffi/include/sdrkit.h`.

## Crates

A crate is `crates/<group>/sdrkit-<group>-<name>`. The framework crates and the helper have no group in their names.

```
crates/
  framework/  sdrkit-core         settings, results, events, commands, owners, the stack contract, storage, codecs,
                                  backends, requests, saved state
              sdrkit-graph        streams, components, graphs, adapters, taps, stack graphs
              sdrkit-components   the standard components (dsp, bits, fec, modem, sim, testing)
              sdrkit-macros       the derive macros for shapes of rows, result sets and events
  util/       bits, text, ids, geo, objects      bit and byte tools, text codes, identity tables, geodesy, heard tables
  algo/       dsp, fec, modem, crypto, sim, locate
                                                 filters and transforms, error correction, modems, ciphers, channels,
                                                 location from arrival times
  proto/      link, ccsds, mpegts, lora-phy, pager, tones, wlan-phy, aero, bcdata, ipcast, ookdev, 6lowpan, bt,
              channels, glyph                    protocols that more than one stack uses
  codec/      aac, mpa, codec2, image, apple, bzip2, fits, custom
                                                 audio, speech, picture and file decoders
  svc/        record, scan, memory, store, events, watch, automation, outputs, freqdb, sweep, identify, bank
                                                 the services
  data/       plans                              the reader of band and channel plan files
  stacks/     adsb, analogtv, avdata, avnav, broadcast, cell, custom, dab, drones, dvbcs, dvbt, dvbt2, dvoice, fm,
              general, gnss, hfdata, hfkbd, ism, lora, marine, meter, road, satcom, satimg, science, trunk, tvterr,
              voice, wlan, wsjt
                                                 one crate per family of stacks
  drivers/    core                               the driver contract, the runner, test signal, file, rtl_tcp, iiod
              usb                                the USB transport: device list, arrivals, control and bulk transfers
              rtlusb, hackrf                     the radios on a USB port
              hpsdr, kiwisdr, openwebrx, rsptcp, sdrconnect, sdrpp, soapyremote, spyserver, tci
              registry                           the list of drivers
  serve/      core                               the hub that feeds network clients from one radio, with slices
              rtltcp, soapyremote                the servers of the two protocols that the share speaks
  hosts/      engine                             radios, sessions, threads, services, storage
              catalog                            the list of stacks, components and codecs
              ffi                                the C API (libsdrkit.a, sdrkit.h, the family headers)
              cli, tui                           the command-line tool and the terminal application
  helpers/    sdrkit-presets                     reads the data folder: presets of a stack, plans of a country, lookups
  dev/        test                               helpers for tests: recordings, vectors, counting allocator
              interface                          writes the registry record, the C header and the Swift file of a family
```

Outside `crates/`, the folder `data/` holds the regional data files, and `registry/` holds one record of the interface
of each family.

`sdrkit-host-catalog` is the one place that names every stack crate, and `sdrkit-driver-registry` the one place that
names every driver crate. The engine receives the stack list and the component registry as arguments and depends on no
stack crate.

## Objects

Everything that an application can look at or change is an object with an owner key:

| Object | Key | What it is |
|---|---|---|
| Engine | `engine` | The root object. It holds the lists of everything else as tables. |
| Radio | `radio.r<n>` | An open radio. |
| Session | `session.s<n>` | One stack that runs on one input. |
| Stack | reached from its session | The receive chain of a session. |
| Component | reached from its session | One component of the graph of a session. |
| Service | `service.<key>` | A service of the engine. |
| Kind | `stack.<id>`, `driver.<key>`, `component.<type>` | A stack, a driver or a component kind as the lists hold it: its declarations and its kind commands. |

Every object implements `sdrkit_core::Owner`. Each part of the trait is a declaration that a caller reads at run time:

- Settings. For a described stack, the declarations of an owner come from the description of its family
  (`src/typed.rs`). `schema()` returns the `Schema`: one `SettingDecl` per setting with its id, key, label, value
  type, unit, range, default, options, section and help text, and the flags `restart` (a change takes effect after the
  owner restarts) and `fixed` (the setting cannot be changed). `values()` returns the current `Values`. `apply(id, value)`
  checks the value against the declaration and applies it, or returns a `SettingError` and changes nothing.
- Results. `results_decl()` returns the `ResultsDecl`: the fields, tables, series and rasters of the object.
  `results()` returns the current `Results`.
- Events. `events()` returns one `EventDecl` per event type with its fields.
- Commands. `commands()` returns the `Commands`: one `CommandDef` per command with its arguments and reply fields.
  `invoke(id, args)` runs a command and returns its reply record, or a `CommandError`.
- Problems. `problem(text)` records a fault of the object in its results.

A value is a `sdrkit_core::Value`: none, bool, integer, number, string, frequency, time, duration, position, enum or
bytes. Settings, result fields, table cells, event fields, command arguments and reply fields all carry this one type,
so one piece of code in an application shows and edits all of them.

### Results

`Results` holds:

- the status: a state (off, searching, tracking, locked), the SNR, the level, the frequency offset and a summary line;
- a record of fields;
- tables. A table has declared columns, and each row has a key (the address of an aircraft, the frequency of a
  station). A row is added, changed or removed by its key. A table keeps a declared number of rows; when it is full,
  the oldest row is removed;
- series (a curve with axes), rasters (a picture that grows by rows) and map points;
- a log of text lines, with a limit.

A stack writes its results on its decoding thread. The engine copies them into a snapshot about 30 times a second.
Every change carries the snapshot counter, so a reader that passes the counter of its last read receives only the fields
and rows that changed after it. A list of things heard (aircraft, stations, nodes, packets) is always a table. No object
has a call of its own to read a list.

### Events and commands

An event is a typed record: its type, the time in the signal, the frequency, and the declared fields. A stack appends
events to its output. The engine queues them per object and passes the events of every session to the services. A
caller takes the queued events of one object.

A command is the way to make an object do something that is not a setting: select a DAB service, clear a table, send a
message, start a scan, save the state. A command that works on a row of a table takes the key of that row as an
argument, and the declaration of the argument names the table.

### A result set states the present

A component fills its whole result set on each report. The set says what is true now. A measurement that does not
exist now is absent and does not keep its last value. A value that is still true, such as a decoded text, is kept in the
state of the component and written again on each report. The framework keeps no last value for a field that a report
leaves out.

### A change of frequency

When the host changes the frequency of a session, the engine tells each component of the stack to forget its results
(`Component::forget`). This is no command of the component. The shape of a stack lists with `.kept(..)` what stays, by
table, by result set or by field: a list that a scan builds, a database of stations, a log of received messages. A
component that holds a kept item and other results implements `forget` and clears the others. A component that does
not implement it runs its `clear` command if nothing of it is kept, and else keeps its results. A snapshot between the call and the moment that the stack takes the frequency
holds no result of the stack. The fields of the session, the events and the waiting audio stay. A change that a service
or the stack itself orders does not clear.

## Streams

Sample and media data does not fit a table. These are the only data paths with dedicated calls. All of them belong to a
session, except a feed, which belongs to the engine.

| Stream | Direction | Contents |
|---|---|---|
| Audio | to the host | Decoded sound as interleaved `f32` frames, with the rate and the channel count. |
| Media | to the host | Compressed audio and video packets with codec, pts, dts and keyframe flag. The session keeps the last ten seconds. |
| Pictures | to the host | Decoded pictures (SSTV, weather satellites, slideshows). The session keeps the last 16. |
| Spectrum | to the host | Power spectrum frames of the input of the session, when the session setting `spectrum` is on. |
| Taps | to the host | A copy of the stream at any output port of the graph of the session. |
| Transmit | to the host | Samples that a graph wrote to `tx.sink`, when the engine does not send them to a radio. |
| Feed | from the host | Complex samples that the host pushes, as the input of any number of sessions. |

The session thread never waits for a reader. Each stream is a bounded buffer, and each reader reads from its own
position. Audio that no one reads in time is dropped and counted in the result field `session.audio_lost`.

## Components and graphs

The receive chain of a stack is a graph of components. The graph crate is `sdrkit-graph`.

### Streams inside a graph

An edge of a graph carries a stream. A stream has a `Kind` and a `Format`.

| Kind | Item |
|---|---|
| `Iq` | Complex samples: radio samples, a channel. |
| `Real` | Real samples: an FM multiplex, a demodulated signal. |
| `Audio` | Audio frames of `Format::channels` samples each. |
| `Symbols` | Complex points of a constellation. |
| `HardBits` | Hard bits, one `u8` per bit, the first sent first. |
| `Llr` | Soft bits, one `i8` log-likelihood ratio per bit. Positive means 0, and 0 is an erasure. |
| `Bytes` | Byte packets: transport stream packets, frames after error correction, compressed media. |
| `Messages` | Events. |
| `Spectrum` | Frames of dB bins. |
| `Item(type)` | Values of one Rust type that implements `Item`, such as a frame or a report. |

`Format` is fixed when the graph is configured: kind, rate, channel count, sample type, bit order, centre frequency,
tuned frequency, bandwidth, frame length and, for compressed media, a `MediaFormat` with the four-character code of the
codec. Each call of a component sees one `Buffer` per port: the items of this call, the absolute index of the first one,
and the tags at item positions. The tags are `Retuned`, `GainChanged`, `Discontinuity`, `Timestamp`, `EndOfStream`,
`BurstEnd`, `ConfigChanged`, `Lock` and `FrameStart`. `ComponentInfo::tags` states how a component passes the tags of
its inputs on to its outputs.

### Components

```rust
pub trait Component: Send {
    fn info(&self) -> ComponentInfo;
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError>;
    fn process(&mut self, io: &mut Io);
    fn values(&self) -> Values;
    // with defaults: finish, apply, latency, results, commands, invoke, retuned, attach
}
```

`info()` returns the `ComponentInfo`: the kind name, the role (`Generator`, `Processor` or `Sink`), the input and output
ports, the settings schema and the tag policy. A `Port` has a name that is unique among the inputs or the outputs of the
component (`in`, `out`, `report`), a `FormatConstraint` (what the port accepts or produces) and, for an output, the rule
that derives its rate and channel count. An input made with `Port::optional` can stay unconnected; the port states what
the component then does.

`configure` receives the formats of the connected inputs and returns the formats of the outputs. `process` reads the
input buffers and writes the output buffers. A component never starts a thread. Slow work runs on the helper threads of
the `RunContext` that `attach` passes. The context also carries the station, the storage, the clock, the random source,
the network, the codec plug-ins, the test signals of the stacks, and whether the samples are live.

A `Decoder` is a plain type that is fed items and writes typed outputs and results. `DecoderComponent` makes it a
component: it supplies the ports, a retune at the exact sample, the `clear` command, the rate of result updates and an
optional helper thread for live input.

A later component can control an earlier one. The controller has an output `Port::command("control")`, the controlled
component has an input of the same name, and the edge between them is a feedback edge. A command sent in one pass takes
effect at the start of the next.

### The registry

`sdrkit_graph::Registry` maps a kind name (`<family>.<name>`, such as `dsp.channel`, `fm.stereo`, `lora.bank`) to a
constructor. `sdrkit_components::register` adds the standard components. Each stack crate has a `register` function for
its own components. `sdrkit_host_catalog::registry()` calls all of them. The engine lists the registry as the table
`component_kinds`, and each kind is an object `component.<type>` with its declarations. The registry also holds media
decoders by four-character code (`add_decoder`, `decoder_for`).

### Building a graph

A graph is built through `GraphPlan`, one checked step at a time: add a component by kind, set a setting, connect an
output to an input. A step that does not fit is refused at once: an unknown kind, a value that the schema of the
component refuses, or a connection that no conversion can join (`ConnectError`). `GraphPlan::build` makes the `Graph`.
A stack, the C API and a JSON `Description` all build through this type.

Where a produced format does not fit an input, the graph inserts adapters from the `Catalog`. An adapter has a class:

| Class | Example | When the graph inserts it |
|---|---|---|
| `Lossless` | A rate change inside the band, a frequency shift, a duplicate. | Always. |
| `DeclaredLossy` | A downmix, a hard decision. | Only for an input that states that it takes it. |
| `MeaningChanging` | IQ to real, which is a demodulator. | Never. The connection is refused. |

The graph searches for the cheapest chain of adapters. The name of an inserted adapter starts with `~`. `validate` and
`describe` report every port with its format and every adapter.

### Running a graph

`Graph` implements `sdrkit_core::Receiver`. It runs its components in topological order, once per pass, on one thread
unless the engine cut it at a stage boundary (see "How the threads of a session are chosen").
Feedback edges deliver in the next pass. Other calls on a graph:

- `tap(port, chunks)` copies the stream at an output port into bounded rings for another thread (`TapReader`). When the
  reader is behind, the copy is dropped and counted. Neither side waits.
- `Staged::component_loads` and `Staged::stage_loads` return the work counters of each component and each stage.

The input guard replaces samples that are not finite with 0 and limits samples far beyond full scale. Every stack graph
guards its input. `dsp.sample_guard` is the same guard as a component, for a stream inside a graph.

This is the graph of the FM stack at an input rate of 4 MHz. `sdrkit graph fm` prints its ports and adapters.

```
input (Iq 4 MHz)
  |
channel (Iq 500 kHz) <--- finder
  |
demod --- meas (Real 250 kHz) ---> spectrum --- noise report ---> stereo, squelch
  |                          \---> rds <--- carrier (from stereo)
  sound (Real 250 kHz)
  |
stereo (Audio 50 kHz, 2 channels)
  |
deemph
  |
~resampler1 (dsp.resampler, inserted by the graph)
  |
audio (Audio 48 kHz, 2 channels)
  |
squelch ---> the audio stream of the session

channel, demod, spectrum, stereo, squelch, rds --- report items ---> display ---> results and events
```

### Sources and sinks

A graph does not know where its input comes from. The input of a session is one of:

- a radio (`Input::Radio`). Any driver: a network radio, a recording through the `file` driver, or the test signal of a
  stack through the `synth` driver;
- a host feed (`Input::Feed`). The application pushes complex samples that it has from any origin. The library opens no
  sound device: samples from a sound card or a microphone enter through a feed;
- nothing (`Input::Inside`). A generator component of the graph makes the signal, such as `gen.stack` (the test signal
  of a stack). The session runs a pass every 20 ms.

A graph ends in sinks:

| Sink | Where the stream goes |
|---|---|
| `host.out` | The audio stream of the session. |
| `tx.sink` | The transmitter of the radio of the session. Without one, the transmit stream of the host. |
| `net.sink` | A UDP destination or a TCP server. The connection is made on its own thread. |
| `record.iq`, `record.audio`, `record.ts` | A file in the storage. |
| `dsp.null_sink` | Nowhere. It counts the items. |

A graph can have several sinks. A transmit burst reaches the radio whole, with an end mark. The result fields
`session.tx_sent` and `session.tx_failed` count the bursts.

### Shared derivations of a radio

One radio serves many sessions. The engine puts a feed between the radio and its sessions. The feed passes each block of
samples to every session by reference, and it runs a derivation that several sessions ask for once for all of them:

- the spectrum. Sessions with the setting `spectrum` on, and the sweep service, read the frames of one transform;
- channels. A channel bank reads the radio once and cuts every channel with one filterbank. Each channel is a session
  that reads a feed of its own. Its audio is mixed into the audio of the bank session, with a volume, a pan, a mute and
  a solo setting per channel.

## Stacks

A stack is the receive chain of one technology, declared by a `sdrkit_core::StackDescriptor`:

| Field | Contents |
|---|---|
| `id`, `name`, `group`, `blurb` | The id that settings files and recordings store, and the texts of the stack list. |
| `tuning` | The frequency range and the default frequency, the sample rate that the stack asks for and the least rate that it accepts, the bandwidth, and how far to tune the radio from the signal. |
| `submodes` | The variants of the stack: id, name and default frequency. |
| `settings` | The settings schema. At most one setting may state the sample rate: `Setting::sets_rate(f)`, where `f: fn(&Value) -> f64` returns the rate for a value of the setting, or 0 for the rate of `tuning`. DVB-T and DVB-T2 state the OFDM rate of the setting `bandwidth` this way; Auto gives 0. `StackDescriptor::rate_hz(values)` is the rate that the stack asks for with these values. The operator of the session takes it, and the engine sets the radio to it when the session starts and when the setting changes, if no other session reads the radio. A radio that does not have the rate runs the nearest one, and the stack resamples. |
| `make_receiver` | Makes the receive chain from a `StackConfig` (the values and the submode). |
| `make_generator` | Makes the test signal. |
| `signal_types` | The signal types that the stack receives. |
| `lock_profile`, `auto_gain` | How a scan judges a channel of this stack, and what automatic gain means for it. |
| `verify`, `benchmarks` | The test runs and the benchmarks of the stack. |

The stage boundaries of a stack are marks in its graph, and the cost of its stages is in the file `cost.txt` of its
family. The engine reads both through `Receiver::staged`.

`sdrkit_host_catalog::all()` returns every descriptor. The engine lists them as the tables `stacks`, `submodes` and
`stack_signals`.

`make_receiver` returns a `sdrkit_core::Receiver`, which is an `Owner` with these functions: `configure(input)` takes
the rate and the centre frequency of the input; `feed(iq, out)` decodes a block of any length and never blocks;
`finish(out)` decodes what is left when the input ends; `retuned(input)` drops the lock after a change of frequency;
`set_station` and `set_facilities` pass the station and the facilities of the host; `save_state` and `restore_state`
return and take a `StateBlob`. `Output` carries the audio, the events, the media packets and the requests to the host
of one block. One thread at a time feeds a receive chain.

A stack whose decoder is a graph returns a `sdrkit_graph::StackGraph`. It wraps a `Graph` and adds what such stacks
share: the least input rate, the results declaration of the stack, the routes from stack settings to component settings,
passes of a fixed size, the input guard, the station context, requests to the host and saved state. The stack itself
then consists of components, a function that connects them, and its description. `StackGraph::of(shape, graph)` builds
the receive chain from the shape of the stack, and the build fails if the graph does not match the shape.

A submode selects a variant of one stack. `ils` has `localizer` and `glideslope`, `marine` has `dsc`, `navtex` and
`wefax`, `freedv` has one per waveform, `general` has `am`, `shortwave` and `general`. The submode is an argument of the
start call. A stack with no submode argument runs its default variant.

## Drivers

A driver implements `sdrkit_driver_core::SourceDriver`: `descriptor()` returns its `DriverDescriptor`, and
`open(address, values)` returns a `Source`. The descriptor declares the key, the name, the address forms, the settings,
the commands, the readings (result fields of the radio), the stream formats that the radio supplies (`StreamFormat`:
kind, sample type, lowest and highest rate, channels), and the formats that it takes for transmit (none for a receive-only
radio).

A `Source` is an open radio. `start(sink)` starts the thread of the radio, which writes samples into a ring
(`RingWriter`) until `stop`. `apply(values)` changes settings while the radio runs and reports what was retuned.
`poll(results)` updates the readings.

### The runner

The network radios share one base, `sdrkit_driver_core::runner`. A driver supplies a type that implements `Radio` and
one that implements `RadioStream`. `StreamRunner<R>` implements `Source` for them.

```rust
pub trait Radio: Send + 'static {
    type Stream: RadioStream;
    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError>;
    fn stream(&mut self, abort: &Arc<Abort>) -> Result<Self::Stream, DriverError>;
    fn close(&mut self);
    fn rate_hz(&self) -> Hz;
    fn centre_hz(&self) -> Hz;
    fn capabilities(&self) -> Capabilities;
    fn sample_type(&self) -> SampleType;
    // and: stream_ended, live_rate_change, check, status, readings, settings, save_state, restore_state
}

pub trait RadioStream: Send + 'static {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError>;
}
```

The runner opens the radio with retries, owns the radio thread, reconnects with the last settings after a loss, keeps
the settings that arrive while the radio is away, decides between a live change and a restart, and counts lost samples.
`RunnerConfig` sets the number of open attempts, the gaps between attempts and between reconnects, and the restart
timeout. `RunnerHooks` is the part that a driver replaces when it needs to: `restart_needed(values)`, `extra_loss()`,
`status()` and `stopped()`, each with a default.

### Addresses

An application names a radio by an address, `<driver key>:<rest>`. The driver key selects the driver, and the driver
reads the rest (`iiod:192.168.2.1`, `rtl_tcp:<host>:<port>`, `file:<path>`). The descriptor lists every form with its
default port, and the engine table `drivers` shows them. `sdrkit_driver_registry::all()` returns the drivers.

### USB radios

`sdrkit-driver-usb` is the transport of the USB drivers: the traits `UsbBus`, `UsbDevice`, `BulkIn` and `BulkOut`.
`platform_bus()` returns the bus of the system through the `nusb` crate (usbfs on Linux and Android, IOKit on macOS,
WinUSB on Windows); iOS has `NoUsb`, on which every open fails. `FakeUsb` replays a script of USB exchanges, so the
tests of `rtlusb` and `hackrf` and the tests of the engine run without hardware. The address `usb:rtl:<selector>` or
`usb:hackrf:<selector>` names a device: empty for the first free one, a serial number, `#<n>` or `@<bus>-<port path>`.
The engine lists the attached devices in its table `devices` and raises `device.arrived` and `device.left`. A listing
opens no device. A driver that flushes the device at a retune marks the samples that it leaves out as a gap made on
purpose (`Sink::deliberate_gap`): the stacks are told of the gap, and neither the radio nor a session counts it as lost.

### The test-signal driver

The driver `synth` plays the `Generator` of a stack, paced like a recording. Its address is `synth:<stack>` or
`synth:<stack>/<submode>`. It has the settings of a radio (centre frequency, rate, gain), a speed (`pace`), a gain model
and 8-bit quantisation, the impairments (`gen_seed`, `gen_level_dbfs`, `gen_noise`, `gen_snr_db` and others) and the
generator settings of the stack. A session on `synth` runs the same code path as a session on a radio, so a stack runs
with no hardware.

## Codecs

A stack that receives coded audio writes it as a `Bytes` stream with a `MediaFormat`. When a decoder is registered for
the four-character code of the format, the graph decodes the packets, and the sound reaches the audio stream. The
packets are also in the media stream of the session.

A codec implements `sdrkit_core::codec::AudioCodec` (`decode(frame, erasure, pcm)`). A `CodecSet` holds factories by
four-character code and channel layout (`register`, `find`, `has`). `sdrkit_host_catalog::codecs()` builds the set: the
AudioToolbox decoders on Apple platforms (`sdrkit-codec-apple`), then the decoders of `sdrkit-codec-aac` and
`sdrkit-codec-mpa`, which take the place of the platform decoders for their codecs. The set is in `Facilities::codecs`.

Each decoder state runs on its own thread behind `sdrkit_graph::codec::GuardedCodec`, with a time limit per call. A
codec that panics or does not return is stopped and turned off in the set, and a notice names it.

## Acceleration backends

A kernel family is a group of functions with more than one implementation. There are six:

| Family | Trait and table | Engine setting |
|---|---|---|
| FFT | `sdrkit_algo_dsp::fft::{FftBackend, BACKENDS}` | `backend_fft` |
| FIR filters | `sdrkit_algo_dsp::fir::{FirBackend, BACKENDS}` | `backend_fir` |
| Resampling | `sdrkit_algo_dsp::resample::{ResampleBackend, BACKENDS}` | `backend_resample` |
| Viterbi decoding | `sdrkit_algo_fec::conv::{ViterbiBackend, BACKENDS}` | `backend_viterbi` |
| Reed-Solomon | `sdrkit_algo_fec::rs::{RsBackend, BACKENDS}` | `backend_rs` |
| LDPC | `sdrkit_algo_fec::ldpc::{LdpcBackend, BACKENDS}` | `backend_ldpc` |

Each family trait extends `sdrkit_core::backend::Backend` (`name()`, `available()`). `BACKENDS` is a
`sdrkit_core::backend::Registry` over a static table. The table lists the backends in order of preference. Its last
entry is the plain backend, which is portable Rust and always available. The first available entry is the default: on
an Apple computer with an ARM processor, the FFT uses Accelerate and the Viterbi and Reed-Solomon decoders use NEON.

Code that needs a kernel asks the registry of the family to build one and does not name a backend. The engine setting
of the family selects a backend by name, or `auto` for the default. A selection applies to objects that are built after
it. Objects that run keep theirs. The status line of the engine names the selected backend of each family.

`check_backend` of each family compares a backend with the plain one.

## Services

A service works on what the sessions produce. It is an object `service.<key>` with settings, results, events and
commands like any other. The engine makes each service with the facilities and the lookups that it declares (storage,
clock, the stack list, the band plans), and passes it the events of every session on the control thread.

| Key | Service |
|---|---|
| `service.record` | Recorder: IQ, audio and transport stream recordings, and a rewind buffer of the samples of a radio. |
| `service.scan` | Scanner: scans channels, such as the channels of a plan that the host gives it. |
| `service.sweep` | Sweep: steps the radio over a frequency range and reads the shared spectrum. |
| `service.identify` | Identify: names the kind of a signal. |
| `service.memory` | Memories: stored channels. |
| `service.freqdb` | Frequency database. |
| `service.store` | Message store: keeps decoded messages in `store/messages.jsonl`. |
| `service.sessions` | Conversations. |
| `service.watch` | Watch list. |
| `service.automation` | Automation rules. |
| `service.outputs` | Outputs to other programs: MQTT, webhooks, JSON lines, UDP and TCP, AIS NMEA, KISS, APRS-IS, SBS. |
| `service.bank` | Channel bank: many channels of one radio at once. |

## The engine

`sdrkit_host_engine::Engine` owns the radios, the sessions and the services. `Engine::new(Setup)` takes the stack list,
the component registry, the `Facilities` (storage, clock, random source, codecs) and the network transport.

### Sessions and threads

```
radio thread (driver) --ring--> pump thread (engine) --> feed --blocks--> session thread: graph --> stream buffers
                                                           \------------> session thread: graph --> stream buffers
control thread: services, requests, timers (a tick every 100 ms)
```

- Each open radio has the thread of its driver, which writes the ring, and a pump thread of the engine, which reads the
  ring into the feed of the radio.
- Each session has one session thread. It runs the receive chain on the blocks of its feed and writes the stream buffers.
  A session keeps the blocks that it has not used in a queue as long as the ring of its radio. When a session is behind,
  it loses blocks. Other sessions on the same feed are not affected. The section on threads below states where a loss
  is counted.
- A session of a heavy stack also has stage threads. The next section states how the engine chooses them.
- A share is an owner of the engine with a session of its own on a radio. That session never steers the radio: it
  takes the samples as the radio gives them and passes them to the hub of `sdrkit-serve-core`, which feeds each network
  client from a ring of its own and cuts a client that stays behind. The listeners speak rtl_tcp and SoapyRemote.
  With the control `remote`, one client steers the radio while no session of the host reads it.
- The control thread runs the services, the request handler and the timers.
- One pool of helper threads, which the engine owns and sizes, runs slow work for components and loops over the items
  of one component. Network sinks connect on threads of their own.

A call of the application works on copies and never waits for a session thread. A session has its own settings
(`tuned_hz`, `spectrum`, `spectrum_size`, `spectrum_window`, `spectrum_fps`, `volume`, `pan`, `mute`, `solo`) and result
fields (`session.state`, `session.samples`, `session.dropped`, `session.problem`). `Engine::start_stack` starts a stack
on an input; with a centre frequency it also tunes the radio, at the offset that the stack asks for.
`Engine::start_graph` starts a graph that the host built with a `GraphPlan`.

### How the threads of a session are chosen

A stack starts no thread and asks for none. It declares two things, and the engine decides.

**Stage boundaries.** A stack marks edges of its graph with `GraphBuilder::stage(from, to, Stage::new(name))`. The
stage `name` begins at the input of `to`. The engine may put a queue on a marked edge and run the two sides on
different threads. One unit of the queue is one pass of the graph: every buffer that crosses the edge, with its tags.
The components after the edge see the same calls as on one thread, so the output of the stack is the same for every
placement. The graph refuses a mark that a feedback edge crosses, and a mark after which a component reads the input
of the graph. `Stage::ahead_bytes` is the read-ahead bound of a stage: the most bytes of units that wait before it.
Within this bound a stage with its own thread works in advance, and it keeps its tables for the present settings as
any component does.

**Cost.** Each family has a file `cost.txt` with one line per stage and variant. The unit is the calibration unit per
second of signal. One calibration unit is one pass of the fixed workload of `sdrkit_graph::calibrate`: a FIR filter of
complex samples, a transform of 1024 points, a Viterbi decoder of the K=7 code of DVB and a table-driven de-interleave.
The filter, the transform and the decoder run on the default backends of their kernel families, as the stacks of the
same build do (NEON on 64-bit ARM, the transform of Accelerate on Apple computers). The sizes never change. The test
`cost` of the family writes the file from the work counters of the graph. In a release build it fails when a line is
stale by more than a factor of two; a debug build checks only that each stage has its line. Nobody writes a line by
hand.

When the engine starts, a thread of its own runs `calibrate::units_per_second` for about 15 ms in the class of the
fast cores, and, where the platform has slow cores, 15 ms more in the background class, which runs on them. The
result is kept for the process: the units per second of one fast core and of one slow core, and the number of each
(engine fields `engine.core_units`, `engine.core_units_slow`, `engine.cores_fast`, `engine.cores_slow`). Where the
platform has one class of cores, the slow number equals the fast one. The recorded cost of a stage divided by the
units of a fast core is the share of a core that the stage needs on this device. `Engine::stack_need` returns this for
a stack before a session starts it: the cores, the threads and whether the stack keeps up with its signal here. The C
call is `sdrkit_stack_need`, the Swift call `Engine.need(stack:submode:)`; both return `SDRKIT_E_NOT_NOW` (Swift: nil)
until the calibration has finished. A session that starts before then runs on one thread until the result is there.

At the start of a session the engine reads the stages and their cost and compares them with the cores that it may
use. The engine setting `cores_max` sets these; 0 takes the performance cores of the machine. The cost that the
running sessions declared is taken off.

- A stack that needs less than 0.7 of a core stays on the session thread.
- A heavier stack gets cuts so that the stages of one thread need at most 0.8 of a core together, as far as cores are
  free. Each cut has a stage thread, which runs only its stages.
- A stack that needs more than is free still starts. The result field `session.load_problem` then reads, for example,
  "needs about 2.4 cores, 2 free".

The measurement of the running session has the last word. Every 250 ms the session thread reads the work counters:

- A thread that works more than 0.85 of the time for half a second, or whose queue stays more than three quarters
  full, gets one more cut, at the boundary that divides its measured work most evenly.
- Two neighbouring threads that together work less than 0.4 of the time for five seconds become one.
- A placement stays for at least one second.

A change happens between two passes. The passes that wait in the queues are finished first, in order, so no unit is
lost. A call that needs every component on one thread (a setting, a command, a retune, a saved state) takes the
components back in the same way, and the next block places them again.

A queue between two stages is bounded. A stage that finds the queue after it full waits, and the wait reaches the
session thread. The blocks then stay in the queue of the session, and when that is full the session loses blocks. The
session tells its stack of each loss with `Receiver::discontinuity`, and the first component reads a `Discontinuity`
tag at the first sample after the gap. A gap that the driver marked in its ring arrives the same way.

A loss is counted for the session and not at the ring, so that one slow session does not take samples from the
others. The column `lost` of the table `session.connections` shows where it happened. The row `queue` holds the
samples that the queue of the session could not take and the samples missing in the gaps that the driver marked. A
loss while the queue before a stage was full, or its writer waited, counts at the first connection into the deepest
such stage: that stage held the session up. The rows add up to the result field `session.dropped`.

`Pool::map(items, f)` runs a loop over the items of one component on the pool of the engine and returns the results
in the order of the items. The calling thread works too, so the call runs in line when no helper is free, with the
same results. The engine sets the number of helpers from `cores_max`.

On Apple platforms a stage thread of a session has the class user-interactive, and a pool thread has the class
user-initiated. Other platforms keep the priority of the thread that started the engine.

The session shows what it measured. `sdrkit profile <stack>` prints the same tables.

| Result | Contents |
|---|---|
| Table `session.components` | One row per component in run order, each followed by its sections: stage, thread, share of one core over the last second and since the start, calls, items in and out, mean and worst time of a call, and the cost in calibration units. |
| Table `session.stages` | One row per stage: thread, whether it has its own thread, share, cost in cores and in units, recorded cost, fill of the queue before it, units that wait, and the times that the stage before it waited. |
| `session.share`, `session.cores_needed` | The share of one core that the stack takes now, and the cores that it needs at its rate. |
| `session.cores_declared`, `session.cores_free`, `session.core_units` | The recorded cost in cores of this device, the cores that were free at the start, and the units per second of one core. |
| Table `session.connections` | One row per connection: first the queue from the input to the stack (key `queue`), then each connection from an output port to an input port in the run order of the readers. Format, stage, items and bytes per second, bytes held now, on the mean and at most, the bound and the fill, the wait of an item, the times the writer found it full and the reader empty, whether it is a stage boundary and cut now, and the samples lost there. |
| `session.threads`, `session.stage_behind`, `session.gaps` | The threads of the stack, the waits at full queues, and the gaps that the stack was told of. |
| `session.dropped` | The samples that the session lost, gaps of the driver included: the sum of the column `lost` of `session.connections`. |

The counters are always on. The graph reads the clock twice for each call of a component. A component times its
inner steps with `let t = io.section("viterbi");` and drops `t` at the end of the step. Code deeper in a component
keeps the handle of `io.meter()` and calls `meter.section(..)`. A section shows as a row under its component.

### Requests

A receive chain never tunes a radio. It puts a `HostRequest` in its output: retune, hop, follow a frequency, stop following,
tune a session to a stored channel, save a memory, record. The request handler of the engine grants or refuses each
request with a typed error. A request is also an event and a row of the engine table `requests`. With the engine setting
`requests_allowed` off, the engine refuses every request.

A hop steps the radio of the session through a list of frequencies, a dwell on each. Only the session that steers the
radio hops it: the one session of the host that reads the radio (a share does not count), or the one that hops it
already. Each step tunes the session through its operator, as a retune does. The pump thread counts the dwell in
samples, so a step lasts its dwell of signal time at any load; at the end of a step it passes no samples until the radio
has moved. The samples written before the move are left out as one gap made on purpose, and the first block of the step
carries the new centre, so a stack is told the centre before the samples of the step. A second session on the radio
follows the centre and moves no radio while the hop runs. The same hop requested again goes on where it is; another one
takes its place. The hop ends with its session, at a retune or a tune of the session, and with `stop_on_lock` when the
stack reports lock at the end of a step.

Transmit has one switch, the engine setting `transmit_allowed`. It is off by default and never saved. A stack
declares with `Receiver::transmits` that it makes transmissions, and a radio declares a transmitter in its capabilities;
neither has a switch. The engine connects a transmitter only to a session whose stack transmits and whose radio has
one, and it calls `Receiver::set_transmit` with the state of the switch. While the switch is off the stack makes no
transmission, each own message fails, and the engine sends no burst to a radio.

A command that transmits declares what it sends (`Command::transmits`), and a command whose transmission lasts, such
as a beacon or a calling loop, names the command that stops it (`Command::stopped_by`). The engine table
`stack_transmits` lists both.

### Storage

The library reads and writes files only through `sdrkit_core::storage::Storage`: read, write, open a reader or a writer,
remove, rename, list, stat and free space, on relative paths (`StorePath`). There are three implementations: a folder
(`DirStorage`), memory (`MemStorage`), and the callbacks of the host (`sdrkit_storage` in the C API). With memory
storage nothing outlives the engine.

### Saved settings and state

The settings of every object are one versioned JSON file, `{ "version": 1, "owners": { "<owner key>": { "<setting
key>": <value> } } }`. The engine exports it, imports it, and writes a JSON Schema of the settings of an object.

What an object has gathered (heard tables, logs) is saved state. An object returns it as a `StateBlob` with its owner
and a version; a blob opens only for that owner and version. The engine commands `state.save` and `state.restore` write
and read the state of the whole engine as one sequence of bytes, which the application stores where it likes.

### Plans and regional data

The framework, the engine and the stacks hold no regional knowledge. The engine has no region setting, no plan table
and no plan command. The station that every stack gets is the position, the height, the callsign, the locator and
the time. A stack takes plain settings (frequencies, steps, channel lists, limits) and does not know where a value
came from.

Regional data is files under `data/`, described in `data/README.md`: band and channel plans, known stations, cities,
DAB transmitters, countries, frequency entries and the tables of single stacks. `sdrkit-data-plans` reads plan files
with a published schema (`docs/schema/plans.json`) and returns their rows: band groups, the plan view of each group,
and the rows of countries as they are written. It makes no choice of a plan; a host picks a plan by its id. Only the
tables of standards that hold everywhere are constants in it: the cellular bands and the DAB blocks.

The helper library `sdrkit-presets` reads the data folder for a host:

- `Data::open` reads the folder. `Data::preset` works out the settings and the start frequency of a stack for a
  country and a city, and `start` applies the preset through the public calls of the engine.
- `Data::plan_ids` lists the plans that a country uses. `Data::scan_channels` builds the channels of a scan from the
  plans that the host names, for `Engine::set_scan_channels`. `Data::frequency_entries` reads
  `data/freqdb/builtin.csv` for `Engine::set_frequency_entries`.
- `Data::search` finds plans, channels and frequency entries by name and by frequency. `Data::labels` places the
  labels of a span. `Data::name_of`, `Data::channel_named` and `Data::step` name a frequency, find a typed channel
  and step to the next channel of a plan.
- `Data::open_personal_plans`, `Data::import_plan`, `Data::export_plan` and `Data::remove_plan` keep the user's plans
  in a file that the host names.
- One module for each family with a data file returns the plain settings of a named row (`fm`, `dvbt`, `dab`, `lora`,
  `marine`, `wlan`, `ism`, `cell`, `avdata`, `wsjt`, `general`, `broadcast`, `analogtv`).

- `Data::stack_channels` lists the channels of a stack in a country, `Data::preset` gives its start frequency, and
  `preset_settings` fits the settings of a preset to the schema of the stack in an engine (a choice becomes its
  index), ready to set.
- `data_folder` finds the data folder of a host: the folder given, else the first with plans of `data` in the
  storage folder, `~/.sdrkit/data`, `data` (the repository) or `share/sdrkit/data` (an installed copy) in a folder
  above the program, and `data` in the current folder.

The command-line tool and the terminal application use the helper: their option `--region <country>` reads rows and
sets plain values, and every command finds the data folder with `data_folder`. The terminal application keeps the
chosen region in its own settings file, `tui.conf` in the storage folder (else `~/.sdrkit`); the engine has no region.
A host in C or Swift gets the same lookups from the presets part of the C API: `sdrkit_presets_channels`,
`sdrkit_presets_dial` and `sdrkit_presets_settings` give the channels, the start frequency and the settings of a stack
in a country, `sdrkit_presets_service_end` says why a stack has no channels where its mode is out of service ("3G
(UMTS) was switched off in Australia in 2024"), and the Swift class `Presets` wraps them.

## The C API

`sdrkit-host-ffi` is a layer over the engine. It has two interfaces to the same objects. The typed interface is the
first choice. The generic interface finds things by text key.

### The typed interface

Each stack family has one description of its interface in `src/typed.rs` of its crate: the result sets, the tables
with the type of their rows, the events, the commands and the settings of each of its stacks
([docs/stack-interface.md](docs/stack-interface.md)). The session, the radio and the engine have a description too, in
`typed.rs` of `sdrkit-host-engine`. Everything else is written from the description by `sdrkit-dev-interface`:

| Written | For |
|---|---|
| The Rust types that the stack fills | The stack |
| `include/sdrkit/<family>.h` | A host in C or C++ |
| `swift/<Family>.swift` | A host in Swift |
| The declarations of the generic interface | A host that shows any stack |
| `docs/stacks/<stack>.md` | A reader |
| `registry/<family>.reg` | The compatibility check |

The files for the session, the radio and the engine are `include/sdrkit/session.h`, `radio.h` and `engine.h`, and
`sdrkit.h` includes them. A function of a family header reads a result set (`sdrkit_fm_fields_read`), a table
(`sdrkit_fm_af_rows`) or an event (`sdrkit_fm_text_event_read`) as a struct, and gets and sets a setting
(`sdrkit_fm_set_force_mono`, `sdrkit_session_set_tuned_hz`, `sdrkit_engine_set_callsign`). A wrong name or type does not
compile. The comment in `sdrkit.h` under "Start here" lists the calls of a stack in order, and
`examples/c/fm_audio.c` is the shortest whole example. The folder `examples/` holds the programs in C, C++ and
Rust, and `tools/package.sh` lays out an installed copy of the library for them.

- Structs with a size. Each struct that crosses the interface begins with `uint32_t size`. The caller sets it with the
  `*_INIT` initialiser of `sdrkit/init.h` (`SDRKIT_AUDIO_INFO_INIT`). The library reads and writes no more than `size`
  bytes, so a struct grows only at its end. A struct that the library fills in a family header needs no initialiser,
  because its function passes the size.
- Absent values. A member that can be absent has a `has_` member before it. It holds a value only if `has_` is true.
- Views. A string or bytes that a call returns is a view into a buffer of the handle. It is valid until the next call
  on the same handle. The caller copies what it keeps.
- Versions. The base header has the version `SDRKIT_INTERFACE_VERSION_MAJOR` and `SDRKIT_INTERFACE_VERSION_MINOR`, and
  `sdrkit_interface_version` returns the version of the library. Each family has its own version.
  `sdrkit_families_count` and `sdrkit_family` list the families, and `sdrkit_family_version` returns the version of one.
  A program runs with a library of the same major version and the same or a higher minor version.
- The registry. `registry/<family>.reg` records every item of a family with its type, unit and the version in which it
  appeared. A test of `sdrkit-host-catalog` compares the description with the record. A new item is a compatible
  change with a higher minor version. A removal, a rename or a change of type or unit needs a higher major version.
- Families as features. Each family is a cargo feature of `sdrkit-host-ffi` and of the catalog. The feature
  `all-families` is on by default. A host that builds with `--no-default-features` and the families it needs gets a
  smaller library.
- Presets as a feature. The cargo feature `presets` of `sdrkit-host-ffi` adds the calls `sdrkit_presets_*` of the
  hand-written header `include/sdrkit_presets.h`: the lookups of `sdrkit-presets` over a data folder
  (`sdrkit_presets_open`, `sdrkit_presets_plan_ids`, `sdrkit_presets_scan_channels_set`,
  `sdrkit_presets_frequency_entries_set`, `sdrkit_presets_search`, `sdrkit_presets_labels`, `sdrkit_presets_name_of`,
  `sdrkit_presets_step`, `sdrkit_presets_plan_import` ...). Structs begin with `size`, strings are views of the
  presets handle, and no JSON crosses the calls. The feature is off by default: a library without it reads no data
  folder and holds no lookup by country.

### The generic interface

The same objects are also reached by text key, for a program that shows any stack without knowing it. The calls take
the ids and keys of the declarations.

- Handles. Every object is a `SDRKIT_HANDLE`, a 64-bit number. 0 is never a handle. A handle names the engine, a radio,
  a session, a stack, a component, a service, a kind, a feed or a graph that is not started yet.
  `sdrkit_object_find(engine, key, &handle)` opens a handle by owner key. `sdrkit_objects` and `sdrkit_object` list the
  live objects. `sdrkit_object_stack` and `sdrkit_object_component` open the stack and a component of a session.
- One set of calls for every object. `sdrkit_setting_*` reads the setting declarations and gets and sets values.
  `sdrkit_results_decl`, `sdrkit_field_*`, `sdrkit_table_*`, `sdrkit_column_*`, `sdrkit_series_*` and `sdrkit_raster_*`
  read the result declarations. `sdrkit_snapshot_read` copies what changed after a counter, and the
  `sdrkit_snapshot_*` calls read fields, rows, cells, log lines, series, rasters and points from that copy.
  `sdrkit_event_type_*`, `sdrkit_events_take` and `sdrkit_event` read events. `sdrkit_command_*`,
  `sdrkit_command_invoke` and `sdrkit_reply` run commands. A name is looked up once (`*_find`) and then used as an id.
  The typed calls use the same snapshot, event list and reply of the handle.
- The variant. `sdrkit_variant` carries any value across the API: a type tag (`SDRKIT_TYPE_NONE`, `_BOOL`, `_I64`,
  `_F64`, `_STRING`, `_HZ`, `_TIME`, `_DURATION`, `_POSITION`, `_ENUM`, `_BYTES`) and one member per type. An argument
  is a `sdrkit_arg`: an id and a variant.
- The text key never changes. A test compares the keys of a family with a recorded file.

### Calls for both interfaces

- Lifecycle. `sdrkit_engine_new` or `sdrkit_engine_new_with_storage`; `sdrkit_radio_open`; `sdrkit_stack_start`, or
  the `sdrkit_graph_*` calls and `sdrkit_graph_start`; `sdrkit_session_stop`; `sdrkit_session_close`;
  `sdrkit_radio_close`.
- Streams. `sdrkit_audio_read`, `sdrkit_media_read`, `sdrkit_picture_read`, `sdrkit_spectrum_read`, `sdrkit_tap_*`,
  `sdrkit_transmit_read`, `sdrkit_feed_open` and `sdrkit_samples_push`.
- Error codes. Every function returns an `int32_t` code, except the `sdrkit_input_*` functions and
  `sdrkit_code_name`. `SDRKIT_OK` is 0. Codes 1 to 20 are argument and setting errors, 30 to 34 command errors, 40 to
  45 storage errors, 50 to 53 graph errors, 60 to 78 engine errors. `SDRKIT_E_PANIC` means that the library failed
  inside; the engine stays usable. After an error, `sdrkit_error_message` returns one sentence and
  `sdrkit_error_detail` the ids and numbers of the error. `sdrkit_code_name` returns the name of a code.
- Out parameters. A function writes its out parameters only when it returns `SDRKIT_OK`. A NULL out parameter is an
  error, unless the comment of the function says that it can be NULL.
- Ownership and the one free function. The library owns all memory that it returns. `sdrkit_free(handle)` frees a
  handle, and it is the only free function. Freeing the engine stops every session and frees every handle of the
  engine. A call on a freed handle returns `SDRKIT_E_NO_SUCH_OBJECT`.
- Threads. A handle serves one thread at a time. A second thread opens its own handle to the same object with
  `sdrkit_object_key` and `sdrkit_object_find`; an audio callback reads `sdrkit_audio_read` through such a handle.
- No JSON carries values. Values are structs, enumerations and typed settings. Saved settings
  (`sdrkit_settings_export`, `sdrkit_settings_import`) are bytes that the host stores and does not read. They begin
  with a magic number and a format version: a later version is refused with `SDRKIT_E_VERSION`, and saved settings of
  an earlier version of a family load (an unknown key is skipped, a missing key takes its default).

`crates/hosts/sdrkit-host-ffi/tests/c_*.c` are C programs that the tests compile and run against the library.
`sdrkit-host-tui` uses the C API only.

## Testing

- Tests. `cargo test --workspace` runs the tests of every crate.
- Test signals. A stack with a `make_generator` has a signal that its receive chain decodes. The `synth` driver plays it, so
  a test runs the whole path from the driver to the results.
- `sdrkit verify [stacks]`. Runs each stack on its test signal through the engine, with noise (`--snr`) and a frequency
  offset, or with neither (`--clean`). A stack declares its cases in `VerifyHooks`: settings, commands, the duration,
  whether the stack must lock and make sound, and comparisons of the results at the end of the run.
- Vectors. `tests/vectors/<family>/` holds, for each recording, the SigMF metadata and an `expected.json` with the
  expected results and their tolerances. `tests/vectors/manifest.json` lists the recordings with size and checksum. The
  sample data is outside the repository. `sdrkit vectors list` and `sdrkit vectors verify` check that the files are
  present and whole. [docs/testing.md](docs/testing.md) describes where the recordings are.
- Allocation tests. A counting allocator checks that a steady-state path allocates nothing: `tests/alloc.rs` in the
  error-correction crate, `tests/alloc_acquire.rs` in the modem crate, `tests/sample_path_alloc.rs` in the driver
  crates.
- Benchmarks. Each crate lists its benchmarks in a `benchmarks()` function. `sdrkit bench [filter]` runs them. The
  engine command `benchmark.run` runs them inside an application and fills the table `benchmarks`.
  [docs/benchmarks.md](docs/benchmarks.md) describes them.

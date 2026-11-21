# Command-line tool

`crates/hosts/sdrkit-host-cli` builds `sdrkit`, a terminal program for the engine (`cargo run --release -p sdrkit-host-cli -- <command>`).
Each command explains itself with `--help`. With `--json`, a command prints one JSON value per line.
The tool keeps its files in one storage folder, `~/.sdrkit` (`--storage <folder>` sets another).

## Owners

An owner is a part of the engine that has settings, results, events and commands. You name an owner on the command line in one of these forms:

| Name | The owner |
|---|---|
| `engine` | The engine: the catalogs and the station. |
| `service:<key>` | A service, for example `service:memory` or `service:record` (`sdrkit services` lists them). |
| `kind:stack:<id>`, `kind:driver:<key>`, `kind:component:<kind>` | A kind of a catalog: its settings and declarations before it runs. |
| `radio`, `session`, `stack`, `component:<handle>` | The radio, the session, the stack and a component that `run` started. They exist only on the console of `run`. |

The tool also takes the owner keys of the engine: `service.scan`, `stack.fm`, `driver.iiod`. On the console of `run`, `stack.<id>` of the stack that runs names that stack.
If you type a name that no owner has, the tool prints the names that exist.

## Commands

| Command | What it does |
|---|---|
| `sdrkit stacks`, `drivers`, `services` | Lists the stacks (by group), the radio drivers and the services. |
| `sdrkit settings <owner> [key=value...]` | Shows the settings of the owner: key, label, kind, range, the values that people normally use (for the present option of the setting they depend on), choices, value and help. Values on the command line are set first. The settings of the engine and the services are kept for the next run. |
| `sdrkit settings export <file>`, `import <file>`, `schema <owner>` | Writes the saved settings of every object to a file, applies a file of saved settings (all or nothing) and keeps them, or prints the JSON Schema of the settings of an owner. Only the library reads a file of saved settings. |
| `sdrkit show <owner> [table]` | Shows the status, fields, tables and log of the owner, or one table. `--since <counter>` shows only what changed after that change counter. `--follow` keeps printing the changes. |
| `sdrkit call <owner> <command> [key=value...]` | Runs a command of the owner and prints its reply. The arguments are read by their declared kinds. Without a command, it lists the commands of the owner with their arguments. |
| `sdrkit events <owner> [--follow]` | Prints the events of the owner. |
| `sdrkit run <stack> [key=value...]` | Runs a stack on a source. It shows a status line (state, SNR, level, offset, samples, lost samples, summary), prints the log and what the console asks for, and plays the sound. With `--verbose` it also prints every result field and table row that changes. Ctrl-C stops it. |
| `sdrkit play <file> <stack> [key=value...]` | Plays a recording (SigMF, WAV or raw IQ) through a stack to its end. |
| `sdrkit record <stack>` | Runs a stack on a source and records its samples with the record service into the storage folder (`--seconds`, 10 by default). |
| `sdrkit graph <stack\|file.json>` | Resolves the graph of a stack, or a graph description file, for an IQ input and shows each port with its format and each adapter as a tree. `--rate` sets the input rate (default: the stack's own rate, 2 MHz for a file). |
| `sdrkit tap <stack> <port>` | Prints the level of one stream of a session: `input`, or `<component>.<port>` of a graph. |
| `sdrkit spectrum <stack>` | Draws the spectrum of the source as a waterfall in the terminal (`--width`). |
| `sdrkit bench [filter]` | Runs the benchmarks of every stack, or only those whose name or stack contains the filter, and shows the processor time and the speed against real time. `--seconds` sets the time for each benchmark (1 by default). |
| `sdrkit verify [stack...]` | Runs the test signal of each stack through the engine with a frequency offset and noise (`--clean` for none, `--snr`, `--seconds`). Each case sets the plain settings that it needs. A stack whose default frequency is 0 runs its full case time: the tool counts the signal at the rate of the input. Without a stack it runs every stack. It shows a table of the checks and exits with code 1 if a check fails. |
| `sdrkit vectors list [filter]`, `vectors verify [filter]` | Shows each test recording of the manifest as present, missing or of the wrong size, or compares the size and the SHA-256 of each. The filter keeps the recordings whose path starts with it (`dvbt`). |
| `sdrkit station` | Shows the station (position, height, callsign, locator) that every stack receives. `--position LAT,LON`, `--height`, `--callsign` and `--locator` change it. `--save` keeps it. |
| `sdrkit declarations --c-header [--out <file>]` | Writes a C header with a `#define` for the key of each setting, field, table, column, event and command of every owner kind. |

## Sources, numbers and settings

A source is `<driver>:<address>`: `synth` (the test signal of the stack, the default), `file:<path>`, `rtl_tcp:<host>:<port>`, `iiod:<host>`, or any driver that `sdrkit drivers` lists.

`--freq` takes a number in MHz: `--freq 104.9`, `--freq 919.875`. A number of 100000 or more is in Hz. Without `--freq`, the stack runs on its own default.
`--rate` takes a number with an optional unit: `2.4M`, `250k`, `2400000`. A number without a unit is in Hz. Without `--rate`, the stack runs at its own sample rate.
The other options of a live command (`run`, `play`, `record`, `tap`, `spectrum`) are `--source`, `--gain` (dB, else the automatic gain of the radio), `--submode` (the variant of the stack), `--seconds` (stop after this time) and `--verbose`. `run` and `play` also take `--audio speaker|none|<file.wav>`.
Write a setting of the stack or the radio as `--set key=value` or as `key=value` after the stack. If both have the key, write `stack.<key>` or `radio.<key>`.
`--region <country>` names a country (an ISO 3166 code such as `AU`) for one command. The engine has no region: the tool reads the rows of the country from the data folder and sets plain values. The option is not kept for the next run.

## Data of a country

`--data <folder>` names a data folder: plan files, lists of known stations and cities, frequency entries and the tables of single stacks (`data/README.md` describes the layout; the folder `data` of the repository is one). Without the option the tool reads the first folder that has plans of these: `data` in the storage folder (`~/.sdrkit/data`), `data` (the repository) or `share/sdrkit/data` (an installed copy) in a folder above the program, `data` in the current folder. Every command finds the folder the same way, with `sdrkit_presets::data_folder`. The helper library `sdrkit-presets` reads the folder; no regional table is in the tool or in the engine.

With `--region <country>` and a folder that exists, every command gives the engine two lists of plain values: the channels of a scan, from the plans that the country uses, and the frequency entries of `freqdb/builtin.csv`. Without them the engine starts and runs every stack; a scan then has no channel list of a plan, and the frequency database holds the user's entries only.

A live command on a radio or a test signal (`run`, `record`, `tap`, `spectrum`) also starts the stack from the preset of the country:

- The settings of the stack are set from the data: `channels` and `bandwidth` of `dvbt`, `bandwidth` of `dvbt2`, `band_low_hz`, `band_high_hz`, `first_hz`, `raster_hz`, `deemphasis` and `rds_variant` of `fm`, `channels` of `dab`, and the dial, the slot limits and the channel lists of a mesh stack (`meshtastic`, `meshcore`, `mesh`, `lorawan`).
- With `--city <name>`, the stack starts on the strongest known station of the city for its service: the highest signal to noise ratio, then the most reports. Without a city, or if the city has no known station, it starts on the first channel that the plan of the country has for the stack.
- `--freq` and each `key=value` setting of the command line replace that part of the preset.

The tool prints one line with the result, for example `region data AU/Sydney: 226.5 MHz, 7 MHz wide, station ABC Sydney`. A stack that the data has nothing for in the country starts with its defaults. So does a stack that does not know a value of the data, and the tool prints which. `dvbt`, `dvbt2`, `fm`, `dab` and `atsc` take the first band group of the country that lists them; another stack has a preset only if the data has a plan of its own for it: a band group whose id starts with the stack id (`fanet_au`), a CSV plan that names it, or a band plan of the LoRa files. If the folder does not exist, the stack starts with its defaults and the tool prints `no region data loaded: <path>`. `play` does not read the data: a recording keeps its frequency.

```
sdrkit run dvbt --source rtl_tcp:127.0.0.1:1234 --region AU --city Sydney --data data
sdrkit run fm --source iiod:192.168.2.1 --region AU --city Sydney
```

## The console of `run`

While `run` runs, type one of these lines and press Enter:

- `call <owner> <command> [key=value...]`, for example `call stack.mesh channel.add net=meshtastic name=home key=<base64>`.
- `set [owner] key=value...` (the session if you name no owner).
- `show [owner] [table]` (a table of the session if you name only a table).
- `events [owner]`.

A value with spaces is written in double quotes: `text="CQ DE VK2ABC"`.

The sound plays on its own thread, so the console and the status line do not wait for it.

A session on a radio that can transmit sends the bursts of its transmit sink to the radio. Transmit has one switch, the engine setting `transmit_allowed`, and it is off at the start: no stack sends and the engine refuses each burst. The transmit power of the radio starts 10 dB below its full output (`tx_power` -10); set another with `set radio tx_power=-20`. Then type `set engine transmit_allowed=true`. No stack and no radio has a transmit switch of its own. `show session` shows the bursts sent (`session.tx_sent`) and failed (`session.tx_failed`), and the reason of the last failure is in the status of the session.

The stack `general` transmits on the listening frequency plus its transmit offset. Type these lines on the console:

- `call session send text="CQ CQ DE VK2ABC"` keys the text in Morse code at the CW speed. Letters, digits, punctuation and procedure signals such as `<AR>` are allowed.
- `call session tune` sends an unmodulated carrier for the tune time, to tune an antenna or an amplifier.
- `call session ptt on=true` sends speech in the mode of the demodulator (NFM, AM, USB or LSB) until `call session ptt on=false`.
- `call session halt` ends each of them at once.

A command that transmits fails with `transmit is off` while the switch is off or the radio cannot transmit. The test signal (`synth`) cannot transmit.

Speech comes from the host audio of the session. The session asks for it only while push-to-talk is on: the field `session.audio_in_wanted` is true then. To send voice without a microphone, give a WAV file with `--tx-audio <file.wav>`: the tool gives the file to the session in real time, from its start, each time `session.audio_in_wanted` becomes true, and gives nothing after its end, so the transmitter sends silence until push-to-talk is off. The file can have any rate and channel count; the channels are mixed to one.

```sh
sdrkit run general --source iiod:192.168.2.1 --freq 439.1 --tx-audio voice.wav
# then on the console:
set engine transmit_allowed=true
call session ptt on=true
call session ptt on=false
```

```sh
sdrkit run fm --source synth --seconds 10
sdrkit run fm --source iiod:192.168.2.1 --freq 104.9
sdrkit run meshtastic --source iiod:192.168.2.1 --region AU --data data
sdrkit play recording.sigmf-meta fm --audio none
sdrkit show service:memory memories
sdrkit verify fm
```

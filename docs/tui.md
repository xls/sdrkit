# The terminal application

`sdrkit-tui` runs sdrkit in a terminal. It lists the stacks and the drivers, opens radios, runs a stack with its spectrum, results, tables and sound, and edits the settings and runs the commands of every object of the engine. It uses the C API of `include/sdrkit.h` and `include/sdrkit_presets.h` for every call to the engine, so its source also shows how an application uses that API. The helper library `sdrkit-presets` reads the region data for it.

## Build and run

```
cargo run --release -p sdrkit-host-tui -- [--source <address>] [--stack <id>] [--submode <id>] [--freq <MHz>] [--region <country>] [--city <name>] [--storage <folder>] [--data <folder>]
```

- `--source <address>`: the radio. The scheme of the address selects the driver: `rtl_tcp:<host>:<port>`, `iiod:<host>`, `synth:<stack>` (a test signal), `file:<path>` (a recording in the storage folder). `synth` or `synth:` without a stack selects the test signal of each stack that starts. The Radios screen lists the drivers.
- `--stack <id>`: the stack to start. With a source, the stack runs on that radio. Without a source, it runs on its own test signal (`synth:<id>`).
- `--submode <id>`: the submode of the stack, such as `glideslope` of the stack `ils`. Without it, the stack runs its default submode. The engine table `submodes` lists the submodes of every stack.
- `--freq <MHz>`: the frequency that the stack tunes to, as a number in MHz (`--freq 104.9`). Without it, the stack runs at its default frequency.
- `--region <country>`: the country (an ISO 3166 code such as `AU`) whose rows of the data folder the application uses. The engine has no region: the application reads the rows and sets plain values. A value that is not a country code is reported on the status line. Without the option the application takes the region of the last run. The application keeps the region that `--region` or the key `g` chose in its own settings file, `tui.conf` in the storage folder (else `~/.sdrkit/tui.conf`), one `key=value` a line.
- `--city <name>`: the city of the country. A stack then starts on the strongest known station of the city.
- `--storage <folder>`: the folder where the engine keeps its settings and files. Without it, the engine keeps them in memory, and they are lost when the application ends.
- `--data <folder>`: the data folder (`data/README.md` describes it). Without it, the application reads the first folder that has plans of these: `data` in the storage folder, `~/.sdrkit/data`, `data` (the repository) or `share/sdrkit/data` (an installed copy) in a folder above the program, `data` in the current folder. The command-line tool finds it the same way.

The engine holds no band plans, no region setting and no frequency entries of its own. With `--region <country>` and a data folder that exists, the application gives the engine two lists of plain values through the presets part of the C API (`sdrkit_presets_open`, `sdrkit_presets_plan_ids`, `sdrkit_presets_scan_channels_set`, `sdrkit_presets_frequency_entries_set`): the channels of a scan, from the plans that the country uses, and the frequency entries of `freqdb/builtin.csv`. Without them the engine starts and runs every stack; a scan then steps through the band of the stack.

With a region and a data folder that exists, each stack that starts takes its preset from the region data through the helper library `sdrkit-presets` and the call `sdrkit_presets_settings`, which fits each value to the setting of the stack: plain settings of the stack (the channels and the bandwidth of `dvbt`, the band edges, the raster, the de-emphasis and the RDS variant of `fm`, the channels of `dab`, the dial and the channel lists of a mesh stack) and the start frequency. The start frequency is `--freq`, else the strongest known station of the city, else the first channel that the plan of the country has for the stack. The status line names the frequency. A stack that the data has nothing for starts as without region data, and so does a stack that does not know a value of the data (the status line says which). If the folder does not exist, the stacks start as without region data and the status line says `no region data loaded: <path>`. The region is that of `--region`, else that of the last run (`tui.conf`); the key `g` chooses another one from the list of the data.

With no source and no stack, the application opens on the Stacks screen.

Examples:

```
sdrkit-tui --stack fm                                      # FM on its test signal, with sound
sdrkit-tui --stack ils --submode glideslope                # a submode on its test signal
sdrkit-tui --source iiod:192.168.2.1 --stack fm --freq 104.9
sdrkit-tui --source iiod:192.168.2.1 --stack dab --freq 206.352 --region AU
sdrkit-tui --source iiod:192.168.2.1 --stack meshtastic --region AU --storage ~/sdrkit
sdrkit-tui --source iiod:192.168.2.1 --stack dvbt --region AU --city Sydney --data data
```

## The screen

The first line is the row of screen tabs, and at its right the stack of the shown session, its radio and its state. The last two lines are the status line and the line of keys. The status line starts with the name of the pane that the keys act on, then shows the last message; an error is red and starts with `✗`.

A tab row shows the chosen tab as a filled label: dark bold text on the accent colour. Every other tab is normal text, with a line between two tabs. If the tabs do not all fit, the row shows the tabs around the chosen one, and `‹` or `›` stands where tabs are left out. The screens and the tables of a pane are tab rows of this one form. The line of keys shows the keys of that pane at the left and the keys of every screen at the right.

A screen has one or more panes. The pane that the keys act on has a heavy frame and a bold title, both in the accent colour. A pane that Tab moves to has a thin frame with "Tab" at the right of its top line. A pane that takes no keys has a thin frame. The selected line of a list or a table is a filled bar: blue in the pane that the keys act on, grey in another pane. Only hints, placeholders and items that are off are dim.

If the variable `NO_COLOR` is set, the application sends no colours: the chosen tab and the selected line are in reverse video, and the pane that the keys act on has the heavy frame and the bold title.

## Keys

Keys that work on every screen:

| Key | Action |
|---|---|
| `1` to `7` | Show a screen. The first line names them. |
| `[`, `]` | On the Session and Services screens: show the previous or the next table, from every pane. After the last table comes the first one. |
| Tab, Shift-Tab | Move the keys to the next or the previous pane of the screen. After the last pane comes the first one. |
| `i` | On the Session screen of a stack that has an entry line: move the keys to that line. |
| `/` | Find a line in the list of the pane. |
| `?` | Show the list of keys. |
| `o` | Choose the next owner for the Settings and Commands screens: the engine, each radio, the session, its stack, the chosen service. |
| `m` | Turn the sound off or on. |
| `q`, Ctrl-C | Quit. The session stops and the radios close. |
| Up, Down, PgUp, PgDn, Home, End | Move in a list. |

When you type a value, Enter applies it and Esc cancels it. When you choose an option from a list, Up and Down move, Enter applies and Esc cancels. Typed text moves to the option that matches it best, and Tab moves to the next match.

To find a line of a list on a screen, press `/` and type a part of its text. The selection moves to the best match: a line whose first word is the text, then a line that starts with it, then a line with a word that starts with it, then any line that holds it. Upper and lower case are the same. Tab moves to the next match and Shift-Tab to the previous one. Enter ends the search and stays on the line. Esc ends the search and returns to the line that was selected before. The search works on every list: the stacks, the open radios, the controls and the table rows of a session, the services and their rows, the objects, the settings, the commands and the event types.

## Screens

1. Stacks. The stack catalog: id, name, group, the signal types that the stack receives, and a sentence about it. The first line shows the source: the radio that a stack starts on, with its key and its address, or the test signal. `a` chooses the source from a list: the test signal, each open radio, or a radio to open. `f` sets the frequency of the stack that starts; an empty line selects the default frequency of each stack. Enter starts the chosen stack on the source and shows its session. A session that reads the same radio ends first. A session on another radio keeps running. A stack that starts on its test signal ends each session on a test signal first.
2. Radios. The driver catalog with the sample rates and the transmit ability of each driver, and the radios that are open. Each line of the open radios shows the key of the radio, its address, `[source of the stacks]` on the source, and the stack that runs on it. `a` opens a radio and makes it the source. `u` makes the chosen radio the source. `s` shows the session that reads the chosen radio. `c` closes the chosen radio and removes it from the list; a session that reads the radio ends first. Enter shows the settings of the radio. The readings of the chosen radio show on the right. A radio that the engine closed shows as closed with its error code until `c` removes it.
3. Session. One session at a time has the screen and the speaker. The first line shows the stack, the key and the address of its radio and, if more sessions run, their number. `n` shows the next session; the others keep running without sound. The line continues with the state of the stack, signal to noise ratio, level, offset and summary; a line with the sound (rate, channels, peak level, mute); the waterfall of the spectrum, newest line at the top, with the frequencies of its left edge, centre and right edge; the controls; the result fields of the stack and the session; the log; and the tables. `s` stops the shown session; the next session that runs then has the screen.
   - `f` opens the frequency box on an empty line. A number is in MHz; a suffix `k`, `M` or `G` is accepted (`105.6`, `105.6M`, `919875k`). Enter tunes the session.
   - The controls are the frequency, the band scan, "Transmit" of the engine if the session can transmit, every command of the stack and every setting of the stack. Two settings of the stack with one label show with their sections. Up and Down move, Enter acts: a command without arguments runs, a command with one argument asks for it and runs, a command with more arguments opens its form on the Commands screen, a setting opens its editor.
   - The frequency tunes the session. A stack that receives one multiplex or one channel at a frequency (DAB, television, a mesh channel) then lists what that frequency carries in its tables; no scan is needed for it.
   - "Scan the whole band" starts the scan service and shows its state and the count of hits. The scan does not start at the frequency of the session: it steps through the channels that the plans of the country of `--region` have for the stack, from the lowest one; without such channels it steps through the band of the stack, one bandwidth a step. Enter on the same line stops it. A scan that ends or stops returns the session to the frequency it had before the scan, unless a frequency was set in the meantime (the frequency control, Enter on a hit).
   - The entry line. If the first command of the stack has a required text as its first argument (`mt.send` of `meshtastic`, `mc.send` of `meshcore`), a box of one line for that text shows above the tables. Its title is the label of the command and the values chosen for its other text arguments ("Send a Meshtastic text · channel: LongFast"). `i` moves the keys to the line from every pane of the screen; Tab moves from the controls to the line and from the line to the tables. While the line has the keys, every character goes into the text, also `q`, the digits, `[`, `]` and `/`. Left, Right, Home and End move the cursor, Backspace and Delete remove a character. Esc moves the keys to the tables and keeps the text. Enter runs the command with the text and the chosen values; every other argument has its default. After a command that ran, the line is empty and the status line names the command and the text. A refused text stays in the line, and the bottom line of the box shows the message of the engine in red. A session with an entry line opens on the table `messages`, if the stack has it.
   - The box also shows why a text cannot go on air now: the session cannot transmit, the transmit switch of the engine is off, or the engine reports what blocks the transmitter. An empty line shows the reason in place of the text; a line with text shows it in the bottom line of the box. A text that the command took while such a reason holds stays in the line with "Not on air" and the reason; the table `messages` lists it as failed.
   - Tab moves to the tables: the tables of the stack, then the hits of the scan, then "Known stations" if the application has region data. The first line of the pane is the row of table tabs. Left and Right choose the table, and so do `[` and `]` from the controls. Up and Down choose the row, `,` and `.` scroll the columns. A table without rows shows one line that says so. Enter acts on the row: if the table has a column with the key of a text argument of the entry line's command (the column `channel` of the tables `channels` and `messages`), the entry line takes the value of the row for that argument, and the status line shows it ("channel: LongFast"); Enter on a row with the chosen value removes the choice, and the command uses its default again; else, if one command of the stack has a row of the table as its only argument (DAB and television `service.select`), that command runs with the row; else a row that has a frequency tunes the session to it; else the row becomes the chosen row for a command argument (see Commands).
   - "Known stations" lists the stations that the region data has for the service of the stack (FM, DAB, DVB-T, ATSC) in the country: the stations of the city of `--city` first, each with its city, frequency, width, standard, network, services, signal to noise ratio, count of reports and the date of the last report. Enter on a row tunes the session to its frequency. A country without a list has a table without rows.
4. Settings. Every setting of the owner: section, label, value, range, and whether a change needs a restart. The column "section" shows only for an owner whose settings have sections; two settings with one label ("Speed" of the stack `cw`) differ by it. `/` finds a setting by its label, its key or its section. The box at the bottom shows the help, the default and the options of the chosen setting. Enter edits the value: a yes or no setting changes at once, an enum setting shows its options, every other setting takes a line of text. The line for a number starts empty, and an empty line changes nothing. A setting whose 0 is not a quantity shows the meaning of 0 ("Automatic", "Off") in place of the number. The library checks the value; a refused value shows its error in the status line and changes nothing.
5. Events. The event types of the engine, and the events of the engine, the radios, the session and the chosen service as they come, newest at the bottom, each with its time, its object, its type, its frequency and its fields. `c` clears the list.
6. Commands. The commands of the owner. Enter opens a form with the arguments of the chosen command, each with its default. In the form, Enter edits the chosen argument; an argument that is a row of a table opens the list of the rows of that table, by name, and reports a table that has no rows yet; `p` writes the row key chosen on a table into it, `d` sets it back to its default, `r` runs the command and Esc closes the form. The reply of the command shows under the form.
7. Services. The services of the engine. Enter chooses a service: it becomes the owner of the Settings and Commands screens, and its fields and tables show on the right. Right or Tab moves to the tables. There Left and Right choose the table (`[` and `]` do so from both panes), Up, Down, PgUp and PgDn the row, `,` and `.` scroll the columns, and Tab returns to the services. Enter on a row acts by what the row is, here and in the tables of a session: a row with a frequency tunes the session to it; a row of a table that lists the choices of one setting sets that setting (a channel plan of a television stack, a region of the engine); a row that rows of another table belong to shows those rows (a plan and its channels); any other row is kept for a command argument (`p` pastes it). The application keeps its places: each table returns to the row it was left on, the session and each service return to the table they were left on, each screen returns to its pane, and each object of the Settings and Commands screens returns to its line. A table that was not shown before opens on its row in use: the table of the choices of a setting on the chosen one (the bottom line of its frame names it, "in use: au"), and a table whose rows belong to that choice on the first of them (the channels of the plan in use).

Text typed for a value is read by the type of the value: a frequency is a number in Hz (`104900000`), or a number with the suffix `k`, `M` or `G` (the frequency box of the session reads a number without a suffix as MHz); an enum takes the key or the label of an option; a position is `latitude,longitude`; bytes are hexadecimal.

## Common tasks

- A USB radio: on the Stacks screen, `a` shows the sources. The list has each RTL dongle and HackRF that is attached and not open, with its address and its state. Choose one and start a stack: the radio opens. `--source usb:rtl:` on the command line does the same for the first free dongle.
- FM with sound: `sdrkit-tui --source <radio> --stack fm --freq 104.9`. `m` mutes the sound. On the Session screen, `f`, `105.6` and Enter tune to another station.
- DAB: `sdrkit-tui --source <radio> --stack dab --freq 206.352`. The table "Services" of the Session screen lists the programmes: Tab, choose one, Enter plays it. "Scan the whole band" in the controls finds the ensembles of the region; Enter on a hit tunes to it.
- Meshtastic: start the stack `meshtastic` on the radio with `--region AU` and the data folder (`--data data`), or with `--freq` and the frequency of the network. The session opens on the table `messages`. To send a text: press `i`, type the text, press Enter. To send it on another channel: show the table `channels` (`]`), press Enter on the channel, then `i`. `mt.channel.add` in the controls adds a channel with its key.
- Transmit has one switch: "Transmit" (engine setting `transmit_allowed`), off when the application starts. It is in the controls of a session that can transmit and on the Settings screen of the engine. A session can transmit if its stack transmits (Meshtastic, MeshCore, raw LoRa) and its radio has a transmitter; no stack and no radio has a switch of its own. With the switch off, or in a session that cannot transmit, an own message fails with "transmit is off" and nothing is sent. The radio's settings "Transmit frequency" and "Transmit rate" follow the session: the engine sets the frequency when the session tunes and both at each burst. Set "Attenuation" of the radio for the power.

## How this application uses the C API

The order of calls:

1. `sdrkit_engine_new(folder, &engine)`, or `sdrkit_engine_new_with_storage(&callbacks, &engine)` for files kept by the application.
2. `sdrkit_radio_open(engine, address, &options, &radio)`. The application passes options with no settings, so the radio opens with the default settings of its driver (NULL selects the same defaults). Options carry values of the settings of the object `driver.<scheme>` (find their ids with `sdrkit_object_find` and `sdrkit_setting_find`).
3. `sdrkit_input input = sdrkit_input_radio(radio); sdrkit_stack_start(engine, "fm", &options, &input, &session)`. The `sdrkit_stack_options` carry the submode (NULL for the default), `centre_hz` (0 for the default frequency of the stack) and values of the settings of the object `stack.<id>`: none, or those of the preset of the region data. The engine tunes the radio for the stack.
4. `sdrkit_object_stack(session, &stack)`: the stack's results, tables and commands are on this handle; the session handle has the streams and the session settings (`spectrum`, `volume`, `mute`, `tuned_hz`).
5. Set the session setting `spectrum` to true for `sdrkit_spectrum_read`.
6. Each tick: `sdrkit_snapshot_read`, the snapshot calls, `sdrkit_events_take` and `sdrkit_event`, `sdrkit_spectrum_read`.
7. At the end: `sdrkit_session_stop`, then `sdrkit_session_close` until it does not return `SDRKIT_E_STILL_RUNNING`, then `sdrkit_radio_close`, then `sdrkit_free(engine)`.

The counter rule. `sdrkit_snapshot_read(handle, since, &info)` copies only what changed after the counter `since`; pass 0 the first time and `info.counter` after. If `info.declaration_changed` is set, read the declarations again. Read every field only if `info.fields_changed` is set. For each table, `sdrkit_snapshot_table` returns the rows that changed, newest first: merge them into your copy by row key. If your copy then holds more rows than `total`, rows were removed: read again with `since` 0.

The view rule. A string or an array that a call returns points into a buffer of the handle and is valid until the next call on that handle. Copy what you keep before the next call.

The stream rule. A handle serves one thread at a time. The sound callback runs on a thread of the sound system, so it reads `sdrkit_audio_read` through its own handle of the session: `sdrkit_object_key(session, &key)` and then `sdrkit_object_find(engine, key, &audio_handle)`. Read the first audio on the main handle to learn the rate and the channel count, then start the speaker at that rate. Audio that no one reads in time is lost and counted in the result field `session.audio_lost`.

Errors. Every call returns a code. After an error, `sdrkit_error_message` on the same handle returns one sentence, and `sdrkit_error_detail` the ids and numbers of the error. A call on a freed handle, or on a handle whose object is gone, returns `SDRKIT_E_NO_SUCH_OBJECT`.

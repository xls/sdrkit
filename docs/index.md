<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Stacks and drivers

Each page lists the settings, results, events and commands of one stack or one driver. `sdrkit stacks` and `sdrkit drivers` print the lists from the program.

## Stacks

### Broadcast radio

| Stack | What it does |
|---|---|
| [FM radio](stacks/fm.md) | FM broadcast stations in stereo, with the station name and radio text from RDS. |
| [STL link](stacks/stl.md) | Studio-to-transmitter links of broadcast stations: the sound and RDS of an analogue FM link; a digital link detected and measured, not decoded. |

### Listening

| Stack | What it does |
|---|---|
| [General](stacks/general.md) | AM (with synchronous detection), narrow and wide FM, USB, LSB and CW, with squelch, noise reduction and presets; transmits speech, Morse code and a tune carrier. |
| [Voice inversion scrambler](stacks/voice_inversion.md) | Detects speech with an inverted spectrum on a narrow-FM channel, estimates the inversion carrier and descrambles it. |
| [Voice channels](stacks/voice_channels.md) | Every voice carrier inside the capture at once: CB, FRS / GMRS, PMR446, amateur FM repeaters and HF voice, with tones, a call log and a scan. A plan file that the host loads names the channels. |
| [Wireless mic finder](stacks/wmic.md) | Wide FM carriers of wireless microphones and in-ear monitors: the width and the deviation of each one, whether it looks like analogue FM, and the audio of one. A plan file that the host loads names the bands. |

### Aircraft

| Stack | What it does |
|---|---|
| [ADS-B / Mode S](stacks/adsb.md) | Aircraft on 1090 MHz: positions, callsigns, altitudes and speeds from ADS-B and Mode S replies. |
| [UAT 978 MHz](stacks/uat.md) | UAT on 978 MHz: aircraft and TIS-B targets, and the FIS-B weather and notices from ground stations. |
| [Mode A/C replies](stacks/modeac.md) | Mode A and Mode C transponder replies on 1090 MHz: squawk codes and pressure altitudes. |
| [Mode S interrogations 1030 MHz](stacks/uplink1030.md) | Radar interrogations on 1030 MHz: Mode A, Mode C, all-calls and Mode S uplink messages with the aircraft they address. |

### Broadcast

| Stack | What it does |
|---|---|
| [DAB / DAB+](stacks/dab.md) | Digital radio ensembles: DAB+ and DAB audio with labels, slideshow, programme guide, Journaline and DMB video. |
| [AM stereo (C-QUAM)](stacks/cquam.md) | Medium-wave AM stereo stations using C-QUAM, with the 25 Hz pilot, stereo meters and separation. |

### TV

| Stack | What it does |
|---|---|
| [DVB-T](stacks/dvbt.md) | Digital terrestrial TV (DVB-T): channels, programme guide, teletext and subtitles. Transmits a transport stream that the host gives. |
| [DVB-T2 / DVB-T](stacks/dvb_terrestrial.md) | Digital terrestrial TV, DVB-T2 or DVB-T, whichever the multiplex is. |
| [DVB-SH](stacks/dvbsh.md) | DVB-SH mobile TV to handhelds (ETSI EN 302 583): the OFDM form (1K to 8K, QPSK and 16-QAM, 1.7 to 8 MHz) and the TDM form (QPSK, 8PSK, 16APSK), turbo coded with a long time interleaver. |
| [DVB-T2](stacks/dvbt2.md) | Digital terrestrial TV (DVB-T2 and T2-Lite): PLPs, channels, programme guide and subtitles. Transmits a transport stream that the host gives. |
| [ISDB-T](stacks/isdbt.md) | ISDB-T (Japan, Brazil and most of South America; 8 MHz channels in the Maldives), 6, 7 or 8 MHz channel, 13 segments |
| [ISDB-TSB](stacks/isdbtsb.md) | ISDB-TSB narrow-band digital sound and data broadcasting (ITU-R BS.1114 System F), 1 or 3 segments; also the one-segment service of ISDB-T |
| [ATSC](stacks/atsc.md) | ATSC 1.0, 8-VSB, 6 MHz channel (US, Canada, Mexico, South Korea) |
| [ATSC 3.0](stacks/atsc3.md) | ATSC 3.0 (NextGen TV), 6 MHz channel, ROUTE services |
| [DTMB](stacks/dtmb.md) | DTMB digital terrestrial TV (China, Hong Kong, Macau, Cuba, Pakistan), 8 or 6 MHz channel |
| [TV (auto)](stacks/tv-auto.md) | Finds out which TV standard the channel carries (DVB-T, DVB-T2, DVB-C, DVB-S/S2, ISDB-T, ATSC 1.0 / 3.0, DTMB, analogue) and its parameters, then switches the right stack on |
| [DVB-C](stacks/dvbc.md) | Cable television (DVB-C, J.83 annex A and C), 16- to 256-QAM: services, guide, teletext and subtitles. |
| [DVB-C2](stacks/dvbc2.md) | Second-generation cable TV (DVB-C2, ETSI EN 302 769): one PLP with its common PLP put back into its transport stream, or a PLP bundled over several slices, 16- to 4096-QAM, transport stream, GSE, GFPS or GCS. |
| [DVB-S/S2](stacks/dvbs.md) | Satellite television and data: DVB-S and DVB-S2 (transport streams, GSE and IP), services, guide and media. Transmits a DVB-S or DVB-S2 transport stream that the host gives. |
| [ISDB-S (BS)](stacks/isdbs.md) | Japanese BS satellite television (ISDB-S): TC8PSK, QPSK, BPSK and up to eight transport streams. |
| [TV sat beacon decoder](stacks/satbeacon.md) | The beacon carrier of a TV satellite through an LNB: carrier measurements and PCM telemetry frames. |

### Digital voice

| Stack | What it does |
|---|---|
| [DMR](stacks/dmr.md) | DMR two-slot TDMA: calls with their speech, talkgroups, talker aliases, text messages and positions. Transmits voice from the microphone and text messages as short data in direct mode. |
| [NXDN](stacks/nxdn.md) | NXDN 6.25 and 12.5 kHz: calls with their speech, status and short data, RAN, and the site of a trunked control channel. |
| [dPMR](stacks/dpmr.md) | dPMR and PMR446 digital: calls with their speech, colour codes, status, short messages and packet data. |
| [TETRA](stacks/tetra.md) | TETRA downlink: the cell (MCC, MNC, colour code, location area) and its calls; encrypted calls without identities. |
| [P25](stacks/p25.md) | APCO P25 Phase 1: voice calls with their speech, talkgroups, encryption IDs, and the trunking control channel (sites, grants). |
| [P25 Phase 2](stacks/p25p2.md) | APCO P25 Phase 2 TDMA: two voice channels per carrier with their speech, talkgroups, encryption IDs and the network status. |
| [TETRAPOL](stacks/tetrapol.md) | TETRAPOL (GMSK 8 kbit/s): system identity and calls; encrypted calls are shown without identities. |
| [D-STAR](stacks/dstar.md) | D-STAR digital voice: voice calls with their speech, callsigns and repeaters from the header, slow-data messages and positions. Transmits voice from the microphone and slow-data texts, with the radio header. |
| [Yaesu System Fusion](stacks/ysf.md) | System Fusion (C4FM): calls with their speech in V/D mode 1, V/D mode 2 and voice full rate, callsigns, data type, text and GPS from the data channel. |
| [M17](stacks/m17.md) | M17 amateur digital voice: callsigns, text, positions, SMS packets; Codec 2 speech through a plug-in. Transmits voice from the microphone (Codec 2 3200) and SMS or data packets. |
| [FreeDV](stacks/freedv.md) | FreeDV on HF (upper sideband): the 1600, 700D and 700E waveforms with their callsign text; Codec 2 speech through a plug-in. RADE is not received. Transmits voice from the microphone on 1600 (Codec 2 1300), 700D or 700E (Codec 2 700C) with the callsign text. |
| [Digital voice (auto)](stacks/dvoice-auto.md) | Finds which digital voice mode is on the channel (DMR, NXDN, dPMR, P25, Fusion, M17) and decodes it. |
| [TETRA DMO](stacks/tetra_dmo.md) | TETRA direct mode on one channel: synchronisation and normal bursts, DM messages, addresses and calls, and the speech of clear calls; encrypted calls are marked, never decoded. |
| [DCR (Japan digital convenience radio)](stacks/dcr.md) | Japanese digital convenience radio (four-level FSK): calls by user code, call signs, manufacturer messages and data blocks, on one channel or every plan channel in the span; voice frames are counted, not played. |
| [DECT cordless phones](stacks/dect.md) | DECT base stations: their identities (RFPI), slots and broadcast system information; no call content is decoded. |

### Weather satellites

| Stack | What it does |
|---|---|
| [Meteor-M LRPT](stacks/meteor_lrpt.md) | Pictures from the Meteor-M weather satellites as they pass over, on 137 MHz. |
| [GOES HRIT](stacks/goes_hrit.md) | GOES-16, 18 and 19 HRIT and EMWIN on 1694.1 MHz: full-disk and mesoscale pictures, relayed pictures and text. |
| [GK-2A LRIT](stacks/gk2a_lrit.md) | GEO-KOMPSAT-2A LRIT on 1692.14 MHz: full-disk channel pictures and text. |
| [GK-2A HRIT](stacks/gk2a_hrit.md) | GEO-KOMPSAT-2A HRIT on 1695.4 MHz (3 Msym/s QPSK): full-disk channel pictures. |
| [Elektro-L](stacks/elektro_l.md) | Elektro-L N3, N4 and N5: HRIT and LRIT pictures on 1691 MHz and GGAK telemetry on 1693 MHz. |
| [FY-4 LRIT](stacks/fy4_lrit.md) | FengYun-4A and 4B LRIT on 1697 MHz (DVB-S2): AGRI channel pictures and messages. |
| [FY-4A HRIT-II/III](stacks/fy4a_hrit.md) | FengYun-4A HRIT on 1679 MHz (DVB-S2, 1 Msym/s): AGRI channel pictures. |
| [FY-2 S-VISSR](stacks/fy2_svissr.md) | FengYun-2 stretched VISSR on 1687.5 MHz: full-disk infrared and visible pictures, line by line. |
| [HRPT / AHRPT](stacks/hrpt.md) | High-resolution pictures from MetOp (AHRPT), FY-3 frames and archived NOAA HRPT near 1.7 GHz. It needs a tracking dish. |
| [FY-3 AHRPT](stacks/fy3_ahrpt.md) | Fengyun-3 AHRPT: VIRR, MERSI and MWRI pictures from the polar FY-3 satellites (L-band, and X-band with a down-converter). |
| [FY-3 MPT](stacks/fy3_mpt.md) | Fengyun-3 MPT (X-band, 18.7 Msym/s): MERSI pictures. It needs a dish of 1.2 m or more and a down-converter. |
| [NOAA APT](stacks/noaa_apt.md) | Analogue APT pictures of the NOAA KLM satellites on 137 MHz: FM with a 2400 Hz subcarrier, two picture channels a line. |
| [NOAA DSB](stacks/noaa_dsb.md) | The TIP beacon (DSB) of the NOAA KLM satellites on 137 MHz: minor frames at 8320 bit/s and HIRS sounder lines. |
| [GOES GVAR](stacks/goes_gvar.md) | The GVAR imager broadcast of GOES I to P (BPSK, 2.11 Mbit/s): the block list and reduced visible and infrared pictures. |
| [AWS L-band](stacks/aws.md) | The direct broadcast of the Arctic Weather Satellite (QPSK, 1.785 Msym/s): scans of the microwave radiometer in 19 channels. It needs a tracking dish. |
| [NOAA GAC](stacks/noaa_gac.md) | The recorder playback (GAC) of the NOAA KLM satellites at 2.66 Mbit/s: reduced AVHRR pictures and HIRS sounder lines. |
| [MSG Raw Data](stacks/msg_raw.md) | The raw SEVIRI downlink of Meteosat Second Generation (QPSK, 3.75 Msym/s): reduced pictures of eleven channels and of HRV, a disc every 15 minutes. It needs a large fixed dish. |
| [GOES CDA](stacks/goes_cda.md) | The telemetry downlinks of GOES-R (BPSK, 40 or 4 ksps) and GOES-N (PM, 4 kbit/s) to their ground station: the frame list. It needs a fixed dish. |
| [GOES MDL](stacks/goes_mdl.md) | The multi-use data link of GOES-13, 14 and 15 (QPSK, 200 ksym/s): the frame list. |
| [GK-2A CDAS](stacks/gk2a_cdas.md) | A QPSK data link of GEO-KOMPSAT-2A (3.096 Msym/s): the frame list. Its frequency is not known: tune by hand. |
| [S-band CCSDS](stacks/sband_ccsds.md) | A CCSDS telemetry link on S or L band, set by its link settings (modulation, rate, code, frame): the frame list with spacecraft and virtual channel ids. It needs a tracking dish. |
| [GOES-R Rebroadcast](stacks/goes_grb.md) | GOES-R Rebroadcast on L band (DVB-S2, 8.67 Msym/s): ABI and SUVI pictures, GLM lightning flashes, and files. |
| [GK-2A UHRIT](stacks/gk2a_uhrit.md) | GEO-KOMPSAT-2A ultra high rate xRIT on X band (DVB-S2, 15.6 Msym/s): full-disk pictures, keys and texts. |
| [Satellite file broadcasts](stacks/sat_filecast.md) | HimawariCast and GEONETCAST Americas on DVB-S2 (C band, through the LNB): files and HimawariCast full-disc pictures. |
| [Elektro-L / Arktika-M RDAS](stacks/rdas.md) | The raw MSU-GS downlink of Elektro-L and Arktika-M on X band (BPSK, 30.72 Msym/s): infrared and visible pictures. It needs 40 Msps or more and an X-band dish with a down-converter; a recording is the usual input. |
| [X-band direct broadcast](stacks/eodb.md) | Direct-broadcast and dump downlinks of polar weather and earth-observation satellites (JPSS, EOS, MetOp, Landsat, Proba, GCOM, Oceansat, SARAL, Kanopus-V, DMSP): frames, instrument pictures and colour products. Most links need a tracking dish and tens of Msps; a recording or a frame file is the usual input. |

### IoT and mesh

| Stack | What it does |
|---|---|
| [Mesh (LoRa)](stacks/mesh.md) | Meshtastic, MeshCore, LoRa APRS, MeshCom and LoRaWAN frames on one band plan, with nodes, messages and maps. |
| [Meshtastic](stacks/meshtastic.md) | A Meshtastic texting client: channels with keys, direct messages, acknowledgements, the node list and positions. |
| [MeshCore](stacks/meshcore.md) | A MeshCore texting client: adverts, contacts, direct messages with acknowledgements and channels. |
| [LoRaWAN](stacks/lorawan.md) | LoRaWAN frame headers and devices on the channels of one plan. |
| [LoRaWAN (all channels)](stacks/lorawanwide.md) | Every channel of a LoRaWAN plan at once, uplinks and downlinks, and the FSK channel. |
| [LoRa (raw)](stacks/loraraw.md) | Plain point-to-point LoRa frames as bytes at every spreading factor and bandwidth; receive and transmit. |
| [FANET](stacks/fanet.md) | Paraglider, hang glider and glider tracking on one LoRa channel: positions, names, messages, weather stations and thermals. |
| [Reticulum (RNode)](stacks/reticulum.md) | Reticulum packets on RNode LoRa radios: packet type, destination and hops, and announces with the signature checked. Data stays encrypted. |
| [ExpressLRS](stacks/elrs.md) | ExpressLRS radio-control links on the hop channels of one capture: link id, hop index and nonce from sync frames, frame counts for each link, and the stick values of your own link from its binding phrase. Experimental: the frame layout, the CRC start value, the rate rows and the hop plans are not confirmed by a public document. |

### Amateur

| Stack | What it does |
|---|---|
| [LoRa APRS](stacks/loraaprs.md) | APRS stations, positions and messages over LoRa on 70 cm. |
| [MeshCom](stacks/meshcom.md) | MeshCom 4 amateur mesh texts and positions on 70 cm. |
| [APRS (AFSK)](stacks/aprs.md) | AX.25 packet radio and APRS at 1200 bit/s AFSK and 9600 bit/s G3RUH: stations, positions, weather and messages. |

### Satellite

| Stack | What it does |
|---|---|
| [Inmarsat-C](stacks/inmarsatc.md) | Inmarsat-C NCS and LES channels: the bulletin board and SafetyNET / FleetNET messages. Receive only. |
| [Inmarsat Aero](stacks/aero.md) | Inmarsat Aero P channels (600, 1200, 10500 bit/s): log-ons, ACARS, ADS-C and aircraft positions. Receive only. |
| [Inmarsat (all carriers)](stacks/inmarsat.md) | One wide L-band capture: every Inmarsat carrier found, classified and decoded at once (Aero P, Inmarsat-C), and the satellite identified. Receive only. |
| [Iridium](stacks/iridium.md) | Iridium bursts over the whole band, demodulated and written as RAW text lines. Receive only. |
| [Orbcomm](stacks/orbcomm.md) | Orbcomm 137 MHz downlinks: packets, satellite positions, orbital elements, channel lists and messages. Receive only. |
| [Globalstar](stacks/globalstar.md) | Activity of the Globalstar terminal uplink: occupancy, power and Doppler per CDMA channel. Nothing is decoded. Receive only. |
| [COSPAS-SARSAT 406 MHz](stacks/sarsat.md) | Distress beacons on 406 MHz, first and second generation: identity, protocol, country and position. Receive only. |
| [GOES DCS](stacks/goes_dcs.md) | GOES / Meteosat data collection platforms at 100, 300 and 1200 bit/s: address, sensor data, platform table. Receive only. |
| [Meteosat HRDCP](stacks/hrdcp.md) | Meteosat high-rate data collection platforms (1200 bit/s offset QPSK, coded): address, sequence, health, data and the platform table, for every carrier in the capture. Experimental: the demodulator's loop values are not confirmed by a public document. Receive only. |
| [Argos](stacks/argos.md) | Argos PTT-A2 platform bursts (other modulations listed) and the A-DCS downlink's HDLC frames. Receive only. |
| [CubeSat telemetry](stacks/cubesat.md) | Amateur and small-satellite packet telemetry: AX.25 (AFSK, G3RUH), AX100 Mode 5, CCSDS, and Geoscan, Lucky-7, Spino, Erminaz, USP, NGHam, U482C, Mobitex and CCSDS concatenated frames. Sends AX.25 UI frames in AFSK 1200 and G3RUH 9600 for an amateur satellite uplink. |
| [Fleet Broadcast](stacks/fleet_broadcast.md) | The 16-channel 1200 Bd DBPSK multiplex of UHF Fleet Broadcast: carrier, clock, frame alignment and the activity of each channel. Nothing is decoded. Experimental: the sync channel's pattern is not public. Receive only. |
| [LoRa satellites](stacks/lora_sat.md) | LoRa frames from and to satellites, with the Doppler shift and Doppler rate of each pass. Receive only. |
| [QO-100](stacks/qo100.md) | QO-100 through a Ku LNB: beacon lock and LNB correction, the transponders' signals, conversations and DATV carriers. Sends SSB voice on the narrowband uplink. |
| [PSK demodulator](stacks/psk_demod.md) | A blind BPSK / QPSK / OQPSK / 8PSK analyser and demodulator for any narrow signal, with bit output. Receive only. |
| [Inmarsat BGAN](stacks/bgan.md) | Inmarsat BGAN forward carriers of the L band: every carrier found and classed, frames and FEC blocks decoded with the published tables, and the broadcast signalling read (bulletin boards, networks, satellites, spot beams, return schedules). Broadcast signalling only: no user data. Synthetic signals only. Receive only. |
| [Satellite carrier survey](stacks/sat_survey.md) | Carriers, channels and frame structures of satellite signals without a public specification: measured, never decoded; carriers named by their Doppler curve when element sets and the station position are given. Receive only. |

### Weak-signal digital

| Stack | What it does |
|---|---|
| [Weak-signal (all enabled)](stacks/wsjt.md) | Every enabled weak-signal decoder on the same audio: FT8, FT4, FT2, WSPR, JT65, JT9, JT4, Q65, FST4, FST4W, MSK144 and JS8. |
| [FT8](stacks/ft8.md) | The 15-second weak-signal mode of HF and 6 m contacts. |
| [FT4](stacks/ft4.md) | The 7.5-second contest mode. |
| [FT2](stacks/ft2.md) | FT4 at twice the speed, in 3.75-second slots. |
| [WSPR](stacks/wspr.md) | Beacons of call, locator and power in 2-minute slots. |
| [JT65](stacks/jt65.md) | One-minute contacts with Reed-Solomon coding (HF and EME). |
| [JT9](stacks/jt9.md) | One-minute contacts in 16 Hz. |
| [JT4](stacks/jt4.md) | One-minute contacts for EME and microwave weak signals. |
| [Q65](stacks/q65.md) | Weak-signal contacts by ionoscatter, troposcatter and EME. |
| [FST4](stacks/fst4.md) | Slow weak-signal contacts on LF to HF, 15 to 1800-second periods. |
| [FST4W](stacks/fst4w.md) | WSPR-style beacons in FST4, 15 to 1800-second periods. |
| [MSK144](stacks/msk144.md) | Meteor scatter: 72 ms frames at 2000 bit/s. |
| [JS8](stacks/js8.md) | Keyboard chat in FT8-style frames: heartbeats, directed messages and free text. |
| [FSK441](stacks/fsk441.md) | Meteor scatter pings: 4-FSK at 441 baud, three tones a character, read burst by burst. |
| [JT6M](stacks/jt6m.md) | 6 m scatter: 44-FSK at 21.5 baud with a sync tone every third symbol; pings read, a long signal folded into its message. |
| [ISCAT](stacks/iscat.md) | Scatter: 42-FSK in 24-symbol frames with Costas sync and length symbols; the message folded over all frames. |
| [PI4](stacks/pi4.md) | Beacons: 4-FSK, 146 symbols of 1/6 s with a K = 32 code, 8 characters each minute before the CW identification. |

### Digital radio

| Stack | What it does |
|---|---|
| [DRM](stacks/drm.md) | Digital Radio Mondiale on short wave (DRM30) and VHF (DRM+): services, text messages, slide shows and Journaline. |
| [HD Radio](stacks/hdradio.md) | HD Radio (NRSC-5) on FM and AM: station information, programs HD1 to HD8, song titles and album art. |
| [CDR](stacks/cdr.md) | China Digital Radio (GY/T 268) in the FM band: the multiplex, its services, the network table and DRA+ audio units. |

### Amateur digital

| Stack | What it does |
|---|---|
| [HamDRM](stacks/hamdrm.md) | Digital picture transfer between amateur stations on a DRM-style modem in an SSB channel. |
| [New Packet Radio](stacks/npr70.md) | A detector of New Packet Radio 4GFSK IP modems on 70 cm: sync word, network id, symbol rate and tuning offset of each frame; the frame content is not read. |

### Trunking

| Stack | What it does |
|---|---|
| [MPT1327 trunking](stacks/mpt1327.md) | MPT 1327 control channels: calls, acknowledgements and broadcasts, the system identity and the channel table. |
| [LTR trunking](stacks/ltr.md) | LTR repeaters: the sub-audible 300 bit/s words with talkgroups, go-to and free repeaters, and the repeater table. |
| [EDACS trunking](stacks/edacs.md) | EDACS control channels: calls, talkgroups and the site, a power scan of the voice channels, and following calls. |
| [Motorola Type II trunking](stacks/motorola_type2.md) | Motorola Type II control channels: talkgroups, units, sites, patches and voice grants, with a channel plan file from the host and following calls. |
| [OpenSky identifier](stacks/opensky.md) | Says whether an FM channel carries the OpenSky pattern (9600 baud 4-level FSK in TDMA bursts) from its symbol rate, levels, width and bursts; nothing is decoded. |
| [MDC-1200](stacks/mdc1200.md) | The 1200 bit/s signalling bursts on analogue FM voice channels: every FM carrier inside the capture is followed, and the unit id of each burst is listed with the calls. |
| [FleetSync](stacks/fleetsync.md) | The signalling bursts of FleetSync radios on analogue FM voice channels: every FM carrier inside the capture is followed, and the fleet and unit of each burst are listed with the calls. An identity reader: the other bits are shown raw. |

### Tones and selective calling

| Stack | What it does |
|---|---|
| [Tone decoders (CTCSS, DCS, DTMF, selcall)](stacks/tones.md) | CTCSS tones, DCS codes, DTMF keys, sequential calls, SELCAL and two-tone pages, with a gate that opens for a chosen tone. |

### Paging

| Stack | What it does |
|---|---|
| [Pagers](stacks/pager.md) | POCSAG and FLEX pagers on one channel: addresses, numeric and text messages, with repeated copies combined. |
| [Multitone Paging](stacks/multitone.md) | A detector of Multitone paging transmissions: carrier, deviation, tones and FSK measurements. |
| [ERMES paging](stacks/ermes.md) | ERMES pagers: 4-level FSK at 3125 symbols/s, the network identity, the initial addresses and the numeric and text messages of each batch. |
| [GSC paging](stacks/gsc.md) | Golay sequential code pagers: tone, numeric, alphanumeric and tone-and-voice pages, single calls and batches, with the pager code and its address function. |

### Data and IoT

| Stack | What it does |
|---|---|
| [SCADA telemetry](stacks/scada.md) | Modbus RTU, Modbus ASCII and DNP3 over narrowband FSK radio links: the modem found by itself, the frames, polls paired with replies, and the devices with their registers and points. |
| [Bluetooth LE](stacks/ble.md) | Bluetooth LE advertising on LE 1M, LE 2M and LE Coded, legacy and extended, on every channel inside the capture: the devices around, their names, vendors and beacons, as a scanner app shows them. |
| [BLE tracker detector](stacks/bletracker.md) | Finds item trackers (AirTag and Find My accessories, Tile, Chipolo, SmartTag, Google tags) that stay with you. |
| [Wi-Fi networks](stacks/wifi.md) | The Wi-Fi access points around on 2.4, 5 and 6 GHz from their beacons: names, security, channels and capabilities. |
| [Wi-Fi HaLow (802.11ah)](stacks/halow.md) | Sub-GHz Wi-Fi HaLow networks from their S1G beacons: names, security, channels, widths and MCS. |
| [Bluetooth Classic](stacks/btclassic.md) | Bluetooth BR/EDR discovery over a wide capture: devices and piconets by the LAP of every access code; inquiry responses give the address, class of device and clock. |
| [wfb-ng / OpenHD](stacks/wfb.md) | Drone video links of injected 802.11n frames: link id, radio port, FEC block and fragment, frames lost. The payload stays encrypted: structure only. |
| [ESP-NOW](stacks/espnow.md) | ESP-NOW frames of Espressif devices on a 2.4 GHz Wi-Fi channel: senders, peers, payloads and encrypted frames. |

### Aviation

| Stack | What it does |
|---|---|
| [ACARS](stacks/acars.md) | Aircraft text messages on VHF: every ACARS channel in the capture at once, with positions and an aircraft list. |
| [VDL Mode 2](stacks/vdl2.md) | The VHF digital data link of airliners: ACARS, ATN and link frames on every VDL2 channel in the capture. |
| [VDL Mode 4](stacks/vdl4.md) | The VHF data link used for surveillance and position reports, on the VDL4 channels in the capture. |
| [SELCAL (aircraft tone call)](stacks/selcal.md) | Detects the two-tone calls ground stations send to alert an aircraft's crew, on HF or VHF voice channels. |
| [Airband](stacks/airband.md) | Aircraft voice on VHF AM: several channels at once, with squelch, scanning, a transmission log and a band watch, and push-to-talk AM on the tuned channel. |
| [HFDL](stacks/hfdl.md) | The HF data link of aircraft over oceans and remote areas: squitters, logons, positions and ACARS from ground stations worldwide. |
| [VDL Mode 3](stacks/vdl3.md) | VDL Mode 3 TDMA bursts on a 25 kHz channel: burst kinds by training sequence and length, Golay words, Reed-Solomon data with its printable text; voice bursts detected, not decoded. |
| [GBAS VDB](stacks/gbas_vdb.md) | A detector of the GBAS VHF data broadcast: bursts, the slots busy of the 500 ms frame and the 10 500 symbol/s line. The message content is not decoded. |
| [LDACS](stacks/ldacs.md) | Structure only: LDACS OFDM found by its cyclic prefix (symbol 120 us, prefix 17.6 us), with the carrier offset and the frame duty; nothing is decoded. |
| [AeroMACS](stacks/aeromacs.md) | Structure only: AeroMACS 5 MHz OFDM found by its cyclic prefix, with the carrier offset and the bursts of the 5 ms frame; nothing is decoded. |

### Marine

| Stack | What it does |
|---|---|
| [AIS](stacks/ais.md) | Ships, base stations and aids to navigation on AIS 1 and 2 and the long-range channels, on a map and in a table. |
| [Marine (HF)](stacks/marine.md) | NAVTEX warnings, DSC calls on MF/HF and VHF channel 70, and HF weather fax charts. |
| [VDES](stacks/vdes.md) | VHF data exchange: ASM frames decoded, VDE-TER bursts measured and classified. |
| [Marine VHF](stacks/marinevhf.md) | Voice on every marine VHF channel in the captured span, with a call log and a scan that watches a priority channel. |
| [ATIS (inland waterways)](stacks/atis.md) | The voice of an inland waterway channel and the ATIS identity each radio sends after it speaks. |

### Cellular and railway

| Stack | What it does |
|---|---|
| [LTE cells](stacks/cell.md) | LTE cells: operator, band, cell identity, area code and signal strength from the information every cell broadcasts. |
| [5G NR cells](stacks/nr.md) | 5G NR cells: physical cell identity, SS/PBCH beams and the MIB of every SSB on the sync raster, and SIB1 (networks, tracking area, cell identity) where CORESET#0 lies in the capture. |
| [NB-IoT cells](stacks/nbiot.md) | NB-IoT cells: cell identity, operator, band and level from the synchronisation signals, MIB-NB and SIB1-NB. |
| [GSM / GSM-R](stacks/gsmr.md) | GSM 900 and GSM-R base stations: frequency, BSIC, operator, location area and cell identity from the broadcast channel. |
| [LBJ](stacks/lbj.md) | Chinese railway train broadcasts: train number, speed, kilometre post, locomotive, route and position. |
| [EBICab / Eurobalise](stacks/ebicab.md) | Eurobalise up-link telegrams between the rails (SUBSET-036), and the bursts of other balise systems. |
| [End of train (EOT / HOT)](stacks/eot.md) | End-of-train devices: unit, brake pipe pressure, motion, battery and marker light from 1200 bit/s AFSK on FM, or the head-of-train commands. |
| [UMTS cells](stacks/umts.md) | UMTS (WCDMA) cells: primary scrambling code, Ec/Io, level and carrier offset from the synchronisation channels and the CPICH, and the operator and cell identity from the broadcast channel. |
| [CDMA2000 cells](stacks/cdma2000.md) | CDMA2000 1x and IS-95 base stations: every pilot by PN offset with Ec/Io, and its Sync Channel Message (SID, NID, System Time, Paging Channel). |
| [TD-SCDMA cells](stacks/tdscdma.md) | TD-SCDMA cells: SYNC-DL code, cell parameter, frame parity, level and carrier offset from the downlink pilot and the midamble; the broadcast channel is not decoded. |
| [C-V2X (structure)](stacks/cv2x.md) | Structure only: LTE sidelink (C-V2X) transmissions found by the SC-FDMA prefix and the 1 ms subframe, with the carrier offset and the busy subframes; nothing is decoded. |
| [Railway CIR data](stacks/railway_cir.md) | The data channel of the Chinese railway train radio: end-of-train device frames (KLW) or dispatch frames (TDCS) with the train, the locomotive, the brake pipe pressure, the kilometre post and the speed. |

### Video

| Stack | What it does |
|---|---|
| [Analog TV](stacks/atv.md) | Analogue television: PAL, NTSC and SECAM pictures with FM, AM, NICAM or A2 sound and teletext; also FM video and NBTV. Transmits a picture and the sound that the host gives. |
| [MAC](stacks/mac.md) | D2-MAC and D-MAC satellite television: the picture, the digital sound and the service names. |
| [FPV video (analogue)](stacks/fpv.md) | Analogue FPV video transmitters on the 5.8 GHz and 1.2 GHz channels: a channel table, the picture and its sound. Transmits a picture and the sound that the host gives. |

### Sensors, 433 / 868 MHz

| Stack | What it does |
|---|---|
| [ISM sensors](stacks/ism.md) | Weather stations, thermometers, tyre pressure sensors, door and motion sensors, remotes and meters on 315 to 915 MHz. |

### Meters, 868 / 169 MHz

| Stack | What it does |
|---|---|
| [Wireless M-Bus meters](stacks/wmbus.md) | Water, heat, gas and electricity meters (EN 13757-4 S, T, C, R2 and N): ids and unencrypted readings; encrypted payloads shown as such. R2 and N1 have no real-signal confirmation yet. |

### IoT, sub-GHz

| Stack | What it does |
|---|---|
| [Z-Wave](stacks/zwave.md) | Z-Wave home automation (ITU-T G.9959, R1 R2 R3): networks, nodes and commands; Security S0 and S2 payloads shown as encrypted. |
| [Wi-SUN FAN](stacks/wisun.md) | Wi-SUN FAN smart utility networks: PAN adverts and configuration, nodes, links and header IEs; secured frames shown as encrypted. |

### IoT, 2.4 GHz

| Stack | What it does |
|---|---|
| [Zigbee / 802.15.4](stacks/zigbee.md) | IEEE 802.15.4 frames on 2.4 GHz channels 11 to 26: MAC headers, beacons, Zigbee NWK and APS headers and device announcements; secured MAC frames opened with the keys of the setting `keys`, else shown as encrypted. |
| [ANT+ sensors](stacks/ant.md) | ANT+ heart rate straps, bike speed, cadence and power sensors, footpods and environment sensors on 2457 MHz; sends one ANT+ frame. |
| [Thread](stacks/thread.md) | Thread networks on the 802.15.4 channels 11 to 26: beacons with the network name, devices, 6LoWPAN headers, and MLE and CoAP where they are sent in the clear. Encrypted frames are marked. |

### IoT, 868 / 902 MHz

| Stack | What it does |
|---|---|
| [Sigfox](stacks/sigfox.md) | Sigfox uplinks: device ids, message counters and payloads (authentication codes shown, never checked). |
| [mioty](stacks/mioty.md) | mioty (TS-UNB) uplinks: the radio bursts of each frame found by their TSMA pattern, the frame decoded and checked, and the MAC fields shown; payloads stay encrypted. |

### 2.4 GHz, channel 0-125

| Stack | What it does |
|---|---|
| [nRF24L01](stacks/nrf24.md) | nRF24L01 Enhanced ShockBurst frames on one 2.4 GHz channel at 250 kbit/s, 1 and 2 Mbit/s (needs a user test on a real signal). |

### Chat, 433.92 MHz

| Stack | What it does |
|---|---|
| [Sub-GHz chat (Flipper Zero)](stacks/subghzchat.md) | Text chat with Flipper Zero devices: plain lines, and AES-256-GCM lines with the password or key of a chat you take part in. |

### Contactless, 13.56 MHz

| Stack | What it does |
|---|---|
| [NFC / RFID 13.56 MHz](stacks/nfc.md) | Contactless cards and tags at 13.56 MHz: the tag type and the UID broadcast during anticollision and inventory, and the name of every frame. |

### IoT, 1.9 GHz

| Stack | What it does |
|---|---|
| [DECT NR+](stacks/dect_nrplus.md) | DECT-2020 NR packets on a 1.728 MHz channel: the physical layer control field (network id, transmitter and receiver ids, length, MCS, HARQ); the data field is not decoded. |

### Road toll, 5.8 GHz

| Stack | What it does |
|---|---|
| [DSRC road toll](stacks/dsrc_toll.md) | The 5.8 GHz downlink of road toll beacons, structure only: HDLC frames counted as beacon broadcasts or private link frames; no identifier or content is shown. |

### Home automation

| Stack | What it does |
|---|---|
| [EnOcean](stacks/enocean.md) | EnOcean switches, contacts and sensors (ERP1 at 868.3 MHz, ERP2 at 902.875 and 928.35 MHz): sender, telegram type and the readings of the common profiles; secure telegrams shown as encrypted. |

### HF and utility

| Stack | What it does |
|---|---|
| [RTTY](stacks/rtty.md) | Radioteletype: ITA2 text sent by two-tone frequency shift keying. |
| [PSK31](stacks/psk.md) | Keyboard chat by phase shift keying: every PSK31, PSK63, PSK125, PSK250 and QPSK signal in the passband at once. |
| [CW](stacks/cw.md) | Morse code: every keyed carrier in a span of 14 to 192 kHz decoded at once, with the speed of each. |
| [Olivia](stacks/olivia.md) | Multi-tone frequency shift keying with Walsh-Hadamard block coding and interleaving: copies text below the noise. |
| [Contestia](stacks/contestia.md) | A faster variant of Olivia with 6-bit upper-case characters. |
| [MFSK](stacks/mfsk.md) | Multi-tone keyboard mode with error correction (MFSK16 and its family), and the pictures sent with it. |
| [DominoEX](stacks/dominoex.md) | Incremental frequency keying with 18 tones: tolerant of tuning and drift, no error correction. |
| [THOR](stacks/thor.md) | DominoEX's incremental frequency keying with convolutional error correction and interleaving. |
| [Hellschreiber](stacks/hell.md) | Text sent as pictures of letters, column by column: the screen draws what it hears. |
| [MT63](stacks/mt63.md) | 64 phase-keyed carriers with Walsh-Hadamard coding and long interleaving: keyboard text through fading and noise. |
| [FSQ](stacks/fsq.md) | Fast Simple QSO: 33-tone incremental keying with addressed messages and automatic replies. |
| [IFKP](stacks/ifkp.md) | Incremental frequency keying with 33 tones, a slower relative of FSQ for keyboard chat. |
| [SSTV](stacks/sstv.md) | Slow-scan television: colour pictures sent line by line as tones (Robot, Martin, Scottie, PD, Wraase). |
| [SITOR](stacks/sitor.md) | Maritime teleprinter: SITOR-B broadcasts (as used by NAVTEX), four-of-seven code with time diversity, and SITOR-A (AMTOR) ARQ links read by a monitor. |
| [ARDOP](stacks/ardop.md) | Lists the ARDOP frames and stations heard: frame types, IDs, connect requests, and 4FSK, PSK and QAM data. Sends FEC text, the ID frame and Ping, and makes ARQ links. |
| [ROS (HF)](stacks/ros.md) | Identifies ROS transmissions by their publicly reported tones and rate. The text itself is not public and is not read. |
| [Throb](stacks/throb.md) | Throb and ThrobX: one or two tones at a time out of 9 or 11, each pair a character; the variant is found from the signal. |
| [QRSS / DFCW](stacks/qrss.md) | Very slow Morse: a narrow spectrogram of the steady tone as picture rows, and the text read from its keying. |
| [CP16](stacks/cp16.md) | 16 carriers keyed by 16 x 16 dot characters: a picture of the dots and the characters read against a font. Receive and transmit. |
| [WINMOR (detect only)](stacks/winmor.md) | Detect only: the 1500 Hz alternating-phase pilot of the retired WINMOR modem, the tuning error and the four raw frame-type tones. |

### Satellite navigation

| Stack | What it does |
|---|---|
| [GNSS](stacks/gnss.md) | GPS, Galileo, BeiDou, GLONASS, QZSS and SBAS: satellites tracked, position and time, the sky, interference and spoofing watch. Needs an active antenna (bias-tee on). |
| [SBAS](stacks/sbas.md) | WAAS, EGNOS, MSAS, GAGAN and other SBAS satellites: PRN masks, fast and long-term corrections, ionosphere, GEO navigation and time. Shown, never applied. |
| [NavIC](stacks/navic.md) | The L5 SPS signal of NavIC (IRNSS): satellites tracked, C/N0, Doppler, subframes with the time of week count and the week number. No position. Experimental: the time count scale and the interleaver direction are not confirmed by a public document. Needs an active antenna (bias-tee on). |

### Aircraft and drones

| Stack | What it does |
|---|---|
| [Drone Remote ID](stacks/remote_id.md) | The identification drones broadcast openly on Bluetooth LE and Wi-Fi (ASTM F3411 / EN 4709-002): serial, position, operator. |
| [DJI drones](stacks/dji_droneid.md) | DJI drones: DroneID packets (serial number, position, home and pilot) and drone video links recognised by their shape. Nothing is decrypted. |
| [FLARM](stacks/flarm.md) | Gliders and light aircraft on 868 MHz: the old FLARM protocol's positions, privacy flags respected. |
| [ADS-L](stacks/adsl.md) | ADS-L traffic, status and ground uplink of light aircraft on 868.2, 868.4 and 869.525 MHz from one capture, and FLARM from the same capture. |
| [OGN tracker](stacks/ogn.md) | Position packets of Open Glider Network trackers on the channels of one capture: address, position, altitude, speed, heading, climb and aircraft type. Packets with an encrypted position are counted. Experimental: the sync word, the code table, the whitening and the field layout are not confirmed by a public document. |
| [RC links](stacks/rc_links.md) | 2.4 GHz radio-control links from bursts anywhere in the capture: FrSky, FlySky, Futaba S-FHSS and Spektrum DSM transmitter ids and channel values. Hops are not followed. |
| [MAVLink (SiK)](stacks/mavlink_sik.md) | Telemetry radios of drones: SiK packets at every air rate, with and without error correction, the MAVLink messages in them and the position of each vehicle. |

### Navigation

| Stack | What it does |
|---|---|
| [NDB](stacks/ndb.md) | Non-directional beacons: the carrier, the Morse identification, the audio, and every beacon in the capture. |
| [VOR](stacks/vor.md) | The radial from a VOR or Doppler VOR, the course deviation for a selected course, and the station's identification. |
| [ILS](stacks/ils.md) | Localizer, glide path and marker beacons: deviation from the course and the path, the flag, the identification, paired frequencies, and the marker passed over. |
| [DME](stacks/dme.md) | Pulse pairs of a DME beacon or of interrogating aircraft: the channel, X or Y, the pair rate and the identification. |
| [Loran-C / eLoran](stacks/loran.md) | A Loran chain on 100 kHz: its GRI, the master and secondaries, their time differences, ECD and signal to noise. |
| [DGPS beacon](stacks/dgps.md) | Differential GPS beacons: RTCM corrections, the base station's position, the beacon almanac and text messages. |
| [ELT 121.5 MHz](stacks/elt121.md) | Detects the swept tone of an analogue emergency locator transmitter on 121.5 MHz. Detection only: the beacon sends no identity. |

### HF utility

| Stack | What it does |
|---|---|
| [HF ALE](stacks/ale.md) | Automatic link establishment (2G, 8-FSK): soundings, calls, nets and AMD messages, with the stations heard. |
| [PACTOR-I (experimental)](stacks/pactor1.md) | PACTOR-I monitor: both stations of an ARQ link and unproto broadcasts, with memory ARQ, text and callsigns (experimental). |
| [Wave-buoy HF link (Datawell)](stacks/wavebuoy.md) | Datawell wave buoys on HF: heave, north and west, the sea state and the wave spectrum from the 2FSK HXV link (receive only). |
| [CODAR / HF ocean radar](stacks/hfradar.md) | Coastal HF ocean radars (CODAR SeaSonde, WERA): their FMCW sweeps, repetition, band and keyed ID (receive only). |
| [HF beacons](stacks/hfbeacons.md) | Propagation monitor: the NCDXF/IARU beacons, the Long Dashes and the single-letter beacons heard in a wide HF capture. |
| [PACTOR-II (physical layer)](stacks/pactor2.md) | PACTOR-II physical layer: the two carriers, the symbol clock and the frame structure (the coding is proprietary; no text). |
| [PACTOR-III (physical layer)](stacks/pactor3.md) | PACTOR-III physical layer: the carriers and speed level, the symbol clock and the frame structure (no text). |
| [PACTOR-IV (physical layer)](stacks/pactor4.md) | PACTOR-IV physical layer: the 1800 baud carrier and an estimate of its constellation (no text). |
| [HF waveform ID](stacks/hfid.md) | Identification only: measures an HF data signal's physical layer and ranks the waveforms it could be (estimates). |
| [MIL-STD-188-110 serial tone](stacks/ms110.md) | 2400 Bd 8-PSK on 1800 Hz, 75 to 4800 bit/s: the rate and interleaver from the preamble, an equaliser, the K = 7 code; text and STANAG 5066 D_PDUs. |
| [STANAG 4285](stacks/s4285.md) | 2400 Bd frames of 256 symbols on 1800 Hz, 75 to 3600 bit/s: coded rates found by their start of message; text and STANAG 5066 D_PDUs. |
| [STANAG 4539 high-rate serial tone](stacks/s4539.md) | 3200 to 12800 bit/s on 1800 Hz (QPSK to 64-QAM): rate and interleaver from the preamble, probe-based MMSE equaliser, tail-biting code; text and STANAG 5066 D_PDUs. |
| [3G-ALE](stacks/ale3g.md) | Third-generation ALE bursts: link setup (BW0), traffic management, HDL and LDL packets and acknowledgements (BW1 to BW4), with the datagrams rebuilt. |
| [STANAG 4481 FSK](stacks/s4481.md) | Naval broadcast FSK: one +-425 Hz channel or up to 16 sub-carriers of +-42.5 Hz; the speed of each, plain ITA2 text, encrypted channels measured. |
| [Link 11 CLEW (structure)](stacks/link11.md) | The 16-tone Link 11 waveform: messages, frame rate, frame count, Doppler offset and the raw phase steps of the 15 data tones (no bits). |
| [EFR teleswitch](stacks/efr.md) | Long-wave load-control telegrams: 2-FSK at 200 Bd, 11-bit characters, FT1.2 frames with a checksum; the message number, two addresses and the payload as hex. |
| [G-TOR monitor](stacks/gtor.md) | G-TOR frames at 100, 200 or 300 Bd, plain or in the Golay form, checked by their CRC: plain text, and Huffman frames as hex. |
| [ARQ-E / ARQ-N](stacks/arqe.md) | Duplex ARQ teleprinter on 2-FSK with 7-unit words: speed, shift, word alignment, polarity and the inversion cycle found; ITA2 text. |
| [AUTOSPEC / SPREAD](stacks/autospec.md) | FEC teleprinter on 2-FSK: 10-unit Bauer code words of ITA2 characters, one error corrected; SPREAD spreads each word over 11, 21 or 51 places (found). |
| [Piccolo Mk VI](stacks/piccolo.md) | 6-tone MFSK teleprinter: each ITA2 character a pair of 50 ms tones 20 Hz apart from a 6 x 6 matrix; the tones and the pair phase found. |
| [Coquelet-8](stacks/coquelet.md) | 8-tone MFSK teleprinter: each ITA2 character a pair of tones 26.67 Hz apart; the tones, the tone length (37.5, 50 or 75 ms) and the pair phase found. |
| [CIS HF data signals](stacks/cis_hf.md) | CIS HF signals: BEE messages (2-FSK) as hex, CIS-14 frames raw, the CIS-12 phase steps; CIS-36, the CIS OFDM signals, CIS-128 and CIS-3000 identified only. |
| [CROWD36](stacks/crowd36.md) | CROWD36 34-tone MFSK at 40 Bd: the tones found anywhere in the channel, the sync tones, the characters of the letters and figures cases. Not deciphered. |
| [XPA / XPA2 number senders](stacks/xpa.md) | XPA (20 positions, 10 or 20 Bd) and XPA2 (16 tones, 7.8125 Bd) MFSK number senders: the tones found anywhere in the channel, the speed, sideband and numbering found, the five-figure groups. |
| [FSK200/500 and FSK200/1000](stacks/fsk200.md) | 200 Bd 2-FSK number senders: F01 (500 Hz shift, ITA2 figure blocks) and F06 (1000 Hz shift, 288-bit blocks under a CRC-16: figure blocks, ASCII, F06a files). |
| [GW FSK](stacks/gwfsk.md) | 100 Bd 2-FSK bursts of a shore-to-ship data service: free-channel markers with the shore station code, packets with type and subtype, MMSI numbers, position reports, ACKs. |
| [Telephone FSK modems](stacks/fsk_modems.md) | Bell 103, V.21, V.23 mode 2 and Bell 202 bursts heard in an SSB channel: the standard, sideband and dial error found, the bytes of each burst (CHU's time code as bytes). |
| [UVB-76 buzzer monitor](stacks/uvb76.md) | Times the buzz train of the UVB-76 AM station: buzz rate, length, period and spread; train breaks and long transmissions. A monitor: no voice is decoded. |
| [VLF MSK transmitter monitor](stacks/vlf_msk.md) | Carrier and bit rate of a VLF MSK station from the two lines of the squared signal; on, off and change events, a hint of the listed station. A monitor: the traffic is not decoded. |
| [Robust Packet (physical layer)](stacks/rpr.md) | Robust Packet physical layer: the eight-carrier comb, the bursts, D-BPSK or D-QPSK and the phase steps (the coding is not public; no frames). |
| [HF packet 300 Bd](stacks/hfpacket.md) | AX.25 frames of amateur packet radio on HF: 300 Bd AFSK with a 200 Hz shift on a sideband. |

### Science, time and utility

| Stack | What it does |
|---|---|
| [Radiosonde](stacks/sonde.md) | Weather balloons: RS41, RS92, DFM, M10, M20 and LMS6 positions, heights, sensors and landing predictions. |
| [Time signals](stacks/time.md) | The time and date from DCF77, MSF, WWVB, JJY, BPC, WWV/WWVH, BPM, HLA and RWM, compared with your clock. |
| [e-Czas (225 kHz, Poland)](stacks/eczas.md) | The official Polish time from the phase keying of the 225 kHz long-wave carrier. |
| [NOAA Weather Radio SAME](stacks/same.md) | Emergency and weather alerts (SAME headers) on the US and Canadian weather radio channels. |
| [Meteor scatter](stacks/mscatter.md) | Counts and measures meteor echoes of a CW beacon such as the GRAVES radar on 143.050 MHz. |
| [SSDV pictures](stacks/ssdv.md) | Pictures from high-altitude balloons and small satellites sent as SSDV packets. |
| [Radio astronomy](stacks/astro.md) | The hydrogen line (and OH lines): long integrations, velocities, calibration, line fits and maps of the Milky Way. |
| [Ionospheric sounders](stacks/iono.md) | Chirp sounders: finds them, builds their ionograms and scales them (foE, foF2, MUF, path MUF) to tell which bands are open. |
| [Sferics (lightning)](stacks/sferics.md) | Counts the radio impulses of lightning as strikes and flashes: strike rates, a 60-minute histogram and a storm trend, with mains impulse trains left out. Any band below 30 MHz; VLF and LF hear them best. |
| [SuperSID (VLF)](stacks/supersid.md) | The level of each monitored VLF transmitter every 5 s, sudden ionospheric disturbances (as from a solar flare), terminator marks and the SuperSID data file. |
| [Schumann resonances](stacks/schumann.md) | From an ELF sensor on a sound card: the five lowest Schumann resonances (frequency, height, Q) over a 5-minute average, a spectrogram of 3 to 45 Hz, the fundamental minute by minute, and ELF transients. |
| [Whistlers and tweeks](stacks/whistler.md) | In VLF audio from a sound card: whistlers that fall along Eckersley's law, found by a search over the dispersion, tweeks just after a sferic, and the count of sferics. |
| [Lightning position (network)](stacks/lightning_tdoa.md) | Each station times sferics to the microsecond on a disciplined sample clock; one station, the server, collects the detections of all and locates each strike by time difference of arrival, with a 95 % error ellipse. |
| [Radio Jove](stacks/jove.md) | A spectrogram of the whole capture near 20 MHz and the broadband bursts of Jupiter and the Sun in it: time, duration, strength and width, with the background level minute by minute. |
| [Solar burst spectrogram](stacks/callisto.md) | A spectrogram in 62.5 kHz bins and 0.25 s rows, the e-CALLISTO resolution; solar radio bursts with start, duration, strength, bandwidth and drift rate, and e-CALLISTO FITS files. |
| [Riometer](stacks/riometer.md) | Cosmic radio noise near 30 MHz with interference rejected, against a quiet-day curve per sidereal time: the absorption of the ionosphere in dB and its events. Needs a fixed radio gain. |
| [Moon echo test](stacks/eme_echo.md) | Finds your own keyed carrier and its echo from the Moon 2.3 to 2.85 s later within ±500 Hz: delay, Doppler shift, level and spread. The stack listens while you transmit; it transmits nothing. |
| [Beacon openings](stacks/beacon_events.md) | Follows a beacon carrier within ±1 kHz and logs every opening: start, duration, peak S/N, width, shift and flutter, with a class by the character of the signal (steady, fluttering or Doppler-spread). |
| [Pulsar folding](stacks/pulsar.md) | A filter bank, incoherent dedispersion over trial DMs and folding over trial periods round your ephemeris: the S/N, the period and DM found, the pulse profile and the S/N curves. Needs a dish or long Yagis and a low-noise amplifier. |
| [Pulse radar (detect)](stacks/radar_detect.md) | Rotating pulse radars in S, C and X band: the rotation period, the pulse repetition frequency (single or dual), the pulse width and the beamwidth from the beam passing; for weather radars, the sweeps and volume restarts. |
| [Horus balloons](stacks/horus.md) | Amateur high-altitude balloons: Horus Binary v1 and v2 and RTTY telemetry, several balloons at once across the span, with their tracks, climb rates and custom sensor fields. The coding constants of Horus Binary are not confirmed by a public document. |

### Road

| Stack | What it does |
|---|---|
| [802.11p V2X](stacks/v2x.md) | Road vehicles and roadside units on a 10 MHz 802.11p channel: basic safety messages (SAE J2735) and cooperative awareness messages (ETSI) with their positions on the map; hazard warnings counted. |

### Custom links

| Stack | What it does |
|---|---|
| [Slotted digital voice](stacks/slotted_voice.md) | Digital voice in 16-QAM frames with a sweep that spells the station and a beacon of callsign, position and text; version 4 carries four voice or data slots. Receive and transmit. |
| [QAM video link](stacks/qam_video.md) | A single-carrier 16-QAM or 64-QAM link at 700 to 1400 ksymbols a second with LDPC and erasure-coded groups: video, audio, text and data packets, and the station's callsign. Receive and transmit. |
| [Picture in a burst](stacks/picture_burst.md) | One picture up to 1920 x 1080 in a burst of 10 s at 158 ksymbols a second, in up to 8 layers from robust to fast; the stack shows the picture that the good layers give. Receive and transmit. |
| [Packet link with selective repeat](stacks/arq_packet.md) | Burst packets 100 kHz wide at 80 ksymbols a second, in nine profiles from BPSK rate 1/4 to 1024-QAM rate 5/6, with LDPC blocks; a message goes one way in repeated passes or two way with acknowledgement bursts and selective repeat. Receive and transmit. |
| [File transfer in LDPC frames](stacks/file_transfer.md) | One file over the air in frames of LDPC blocks, each with a CRC-32, and a hash of the whole file: 1 MHz, 500 kHz or 200 kHz wide, ten profiles from BPSK rate 1/4 to 1024-QAM rate 9/10; one way in repeated passes or a two-way session with acknowledged bursts and resume. Receive and transmit. |
| [HF weak-signal chat](stacks/hf_chat.md) | Short messages, beacons and voice in 2.4 kHz of upper-sideband audio, in UTC time slots: five chirp profiles for weak signals and three OFDM profiles, each block coded by its own LDPC code; one way or acknowledged. Receive and transmit. |

### Measurement

| Stack | What it does |
|---|---|
| [FM modulation monitor](stacks/fm_monitor.md) | One FM broadcast station once a second: peak and RMS deviation, MPX power, carrier offset, pilot, stereo, RDS and subcarrier levels, and the seconds over a threshold. |
| [AM modulation monitor](stacks/am_monitor.md) | One AM broadcast station once a second: carrier offset and level, modulation depth, peaks, asymmetry, mains hum, audio bandwidth and over-modulation. |
| [Spectrum occupancy monitor](stacks/occupancy.md) | Busy and idle time of every channel of a grid over the captured span: duty cycle, transmissions, the longest one, and the band occupancy. |
| [Noise floor logger](stacks/noise_floor.md) | The noise floor of each sub-band of the span once a second over hours: rises over a baseline and changes of the whole band. |
| [Burst finder](stacks/burst_finder.md) | Every transmission shorter than 3 s anywhere in the span: frequency, length, bandwidth and level; sources that repeat at a steady interval. |
| [Frequency-hopper analyser](stacks/hopper.md) | One frequency hopper in the span: hop rate, dwell, guard, channel count, spacing and span. |
| [Persistent carrier finder](stacks/birdie.md) | Narrow carriers that stay in the capture: frequency, level, drift, scatter, and whether each is bare or modulated. |
| [Occupied bandwidth meter](stacks/obw.md) | The 99 % and x dB bandwidths, the centre, power and SNR, and the adjacent and alternate power of the strongest signal in the span. |
| [PTT and duty-cycle logger](stacks/ptt_log.md) | The key-ups of one channel: start, length, level and gap; kerchunks, a stuck key and the duty cycle. |
| [Power-line and impulse noise locator](stacks/noise_locator.md) | Impulse rate, level and repetition rate of the noise in a band, with a class: twice the mains, the mains, another steady rate, or none. |
| [Carrier frequency accuracy meter](stacks/freq_accuracy.md) | The offset of a known carrier in Hz and ppm, its C/N0, drift, Allan deviation, steps, loss and return. |
| [FM SINAD meter](stacks/sinad.md) | SINAD, THD, THD and noise, peak deviation and carrier offset of an FM carrier with a test tone; the 12 dB crossing. |
| [Narrowband FM deviation and CTCSS meter](stacks/nfm_meter.md) | Voice peak deviation, the CTCSS tone and its deviation, the carrier offset and the windows over the deviation limits. |
| [Transmitter frequency stability meter](stacks/tx_stability.md) | Per transmission: start and final frequency, key-up step, settling time, drift, mean offset and scatter. |
| [SSB transmitter test meter](stacks/ssb_test.md) | One tone: carrier leak, opposite sideband and harmonics; two tones: IMD3, IMD5 and IMD7 under one tone and PEP. |
| [AFSK tone tuner](stacks/afsk_tuner.md) | Per packet burst of Bell 202 AFSK on FM: mark and space tones, twist, peak deviation, length and gap. |
| [Phase noise meter](stacks/phase_noise.md) | L(f) of a carrier at 10 Hz to 100 kHz offsets, its spurs and the integrated phase jitter. |
| [CW keying quality meter](stacks/cw_keying.md) | Speed, dit and dah, dah ratio, gaps, weight, dit scatter, rise and fall times and chirp of a keyed carrier. |
| [FSK parameter analyser](stacks/fsk_analyser.md) | Of an unknown FSK or GFSK signal: symbol rate, 2 or 4 levels, shift, levels, modulation index, centre and eye SNR. |
| [Modulation quality meter](stacks/mer.md) | Of one M-PSK carrier: order, symbol rate, carrier offset, MER and EVM, and the best roll-off. |
| [Crest factor meter](stacks/papr.md) | Mean power, peak, crest factor and the power CCDF at 10, 1, 0.1 and 0.01 % of the span, read against Gaussian noise. |
| [SDR input health meter](stacks/input_health.md) | Level, peak, crest factor, clipping, DC offset and blind IQ mismatch of the raw samples, with a level verdict. |
| [Noise figure (Y-factor) meter](stacks/noise_figure.md) | The hot and cold plateaus of the noise power as a source is switched, the Y-factor and the noise figure for given ENR values. |
| [Spectral kurtosis RFI finder](stacks/kurtosis.md) | The bins of a wide capture that hold non-Gaussian power, steady or pulsed, by spectral kurtosis; the clean share of the band. |
| [SDR stream glitch detector](stacks/glitch.md) | Dropped and repeated samples on a steady tone, zero runs, repeated blocks and level steps of the raw stream. |
| [Intermodulation finder](stacks/intermod.md) | Strong signals of a wide capture, the peaks at their product frequencies and whether each follows the intermodulation law. |
| [Spur and image finder](stacks/spur.md) | The radio's DC spike, the IQ images and harmonics of strong signals that follow their levels, and fixed spurs on a grid. |
| [Fading analyser](stacks/fading.md) | Of one carrier: level percentiles, fade depth, S4, Rician K, fade rate and length, Doppler spread and C/N0. |
| [Beacon propagation monitor](stacks/beacon_monitor.md) | One beacon: its level over the noise, the Doppler spread of its line and its level swing; the path steady, fading, aurora-like or none. |

### Tags and charging

| Stack | What it does |
|---|---|
| [EM4100 LF RFID](stacks/rfid_lf.md) | The 40-bit identity of EM4100 tags from the load modulation on a 100 to 150 kHz reader carrier. |
| [UHF RFID reader commands (EPC Gen2)](stacks/rfid_uhf.md) | The commands an EPC Gen2 reader sends to its tags, with their fields, Tari, RTcal, TRcal and the tag rate they ask. Tag replies are not decoded. |
| [Qi wireless charging](stacks/qi.md) | The packets a phone sends to its Qi charger by load modulation of the power signal: signal strength, control error, received power, charge status, configuration and identification. |

## Drivers

| Driver | Key | What it connects to |
|---|---|---|
| [Test signal](drivers/synth.md) | `synth` | The test signal of a stack. No radio is involved. |
| [Recording](drivers/file.md) | `file` | A recording in the storage folder: SigMF, WAV or raw IQ. |
| [rtl_tcp server](drivers/rtl_tcp.md) | `rtl_tcp` | An rtl_tcp server, which shares an RTL-SDR receiver on the network. |
| [PlutoSDR / ANTSDR (network)](drivers/iiod.md) | `iiod` | A PlutoSDR, an ANTSDR or another AD936x board, through the iiod network daemon. |
| [SpyServer](drivers/spyserver.md) | `spyserver` | A SpyServer, which shares one radio over TCP. |
| [SDR++ server](drivers/sdrpp.md) | `sdrpp` | The server mode of SDR++, which streams the baseband of one radio. |
| [SoapyRemote server](drivers/soapyremote.md) | `soapyremote` | A SoapyRemote server, which shares the SoapySDR radios of one computer. |
| [KiwiSDR](drivers/kiwisdr.md) | `kiwisdr` | A KiwiSDR receiver on the network. The driver reads the IQ stream of one channel. |
| [OpenWebRX receiver](drivers/openwebrx.md) | `openwebrx` | An OpenWebRX server. The server sends demodulated audio and never IQ. |
| [SDRplay RSP (rsp_tcp server)](drivers/rsp_tcp.md) | `rsp_tcp` | An rsp_tcp server, which shares an SDRplay RSP receiver on the network. |
| [Hermes Lite 2](drivers/hermeslite2.md) | `hermeslite2` | A Hermes Lite 2 radio, through the HPSDR protocol 1. |
| [Red Pitaya](drivers/redpitaya.md) | `redpitaya` | A Red Pitaya radio, through the HPSDR protocol 1. |
| [HPSDR (protocol 1)](drivers/hpsdr1.md) | `hpsdr1` | A radio of the HPSDR protocol 1 (Metis, Hermes, Angelia, Orion). |
| [ANAN / HPSDR (protocol 2)](drivers/hpsdr2.md) | `hpsdr2` | A radio of the HPSDR protocol 2 (ANAN, Orion firmware). |
| [TCI (network)](drivers/tci.md) | `tci` | A program that serves the TCI interface, such as ExpertSDR3 for SunSDR radios. |
| [SDRplay radio (SDRconnect)](drivers/sdrconnect.md) | `sdrconnect` | SDRconnect, the receiver program of SDRplay, through its WebSocket interface. |
| [RTL-SDR dongle (USB)](drivers/rtlusb.md) | `rtlusb` | An RTL2832U dongle (RTL-SDR) on a USB port of this computer. |
| [HackRF](drivers/hackrf.md) | `hackrf` | A HackRF One, HackRF Pro, Jawbreaker or rad1o on a USB port of this computer. It transmits. |
<!-- generated: end -->

[Packaging on Windows and Linux](packaging.md): build the tools and the C library, lay out an archive and check it.

[USB radios and serving a radio to the network](serving.md): the addresses of USB radios, the setup on Windows and Linux, and the share that serves a radio over rtl_tcp and SoapyRemote.

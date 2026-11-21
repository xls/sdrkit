# Region data

This folder holds the regional data that hosts read through the helper library `sdrkit-presets`
(`crates/helpers/sdrkit-presets`). The helper works out the settings and the start frequency of a stack for a country
and a city from these files. The command line program and the terminal application read the folder that `--data` names;
without the option they read `data` in the storage folder (`~/.sdrkit/data`).

## Layout

| Path | Contents |
|---|---|
| `plans/*.json` | Band and channel plans: band groups, countries and ITU regions. The format is `docs/schema/plans.json`. |
| `plans/*.csv` | One channel plan in each file: `# key: value` lines, a header line, one channel in each row. |
| `stations/<kind>/<ISO2>.csv` | The known stations of one country. `<kind>` is `fm`, `dab`, `dvb-t`, `atsc`, `atsc3`, `isdb-t` or `dtmb`. |
| `cities/<ISO2>.csv` | The cities of one country. |
| `dab-tii/<ISO2>.csv` | The DAB transmitters of one country, with their identification codes. |
| `countries.csv` | The countries: code and name. |

`<ISO2>` is the ISO 3166 alpha-2 code of the country in upper case (`AU.csv`). Every CSV file is RFC 4180 text in
UTF-8 with a header line.

| File | Header |
|---|---|
| `stations/<kind>/<ISO2>.csv` | `city,freq_mhz,bw_mhz,standard,network,services,snr_db,reports,first_seen,last_seen` |
| `cities/<ISO2>.csv` | `name,admin1,lat,lon,population` |
| `dab-tii/<ISO2>.csv` | `eid,main,sub,lat,lon,site,power_kw,channel_mhz,reports,first_seen,last_seen` |
| `countries.csv` | `iso2,name` |

In a stations file, `services` holds the names of the services with `|` between them, `snr_db` is the best signal to
noise ratio that was reported, and `reports` is the count of reports. A stack starts on the station of the city with the
highest `snr_db`, then the most `reports`. If the city has no station, the stack starts on the first channel that the
plan of the country has for it.

## What is in the repository

Only `plans/` is in the repository. The folders `stations/`, `cities/` and `dab-tii/` and the file `countries.csv` are
not: the user gets them and puts them here. A file that is not there counts as no data. The licence of `dab-tii/` and
of `countries.csv` comes with those files.

## Licences

| Part | Licence |
|---|---|
| `plans/` | The licence of this project (MIT). Each band group names the published document that it is from. |
| `cities/` | Derived from GeoNames (https://www.geonames.org), CC BY 4.0. Name GeoNames as the source where you show or pass on this data. |
| `stations/` | CC0 1.0. |

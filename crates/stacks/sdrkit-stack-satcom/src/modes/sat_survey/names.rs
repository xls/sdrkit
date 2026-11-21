//! The channel table of a submode: frequencies and names that the host gives as the text of the `channels` setting, and the
//! naming of a measured frequency by the nearest entry.

/// The least distance within which a measured frequency takes a channel's name, Hz.
pub const NAME_MIN_HZ: f64 = 3500.0;
/// The distance within which a measured frequency takes a channel's name, as a share of the frequency (30 ppm: a low orbit
/// keeps a carrier within about 25 ppm of its channel).
pub const NAME_SHARE: f64 = 30e-6;

/// One entry of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// The frequency, Hz.
    pub hz: f64,
    /// The name.
    pub name: String,
    /// The catalogue number of the satellite, when the table gives one.
    pub norad: Option<u32>,
}

/// The table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelTable {
    rows: Vec<Channel>,
}

/// The fields of one line of comma-separated text; a field in double quotes may hold commas.
fn fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

impl ChannelTable {
    /// Reads `text`: one entry per line (or separated by `;`), `frequency_hz,name` or `frequency_hz,name,norad`. Lines of the
    /// data file, `submode,frequency_hz,name,norad`, are also taken, those of `submode` only. Blank lines, lines that start
    /// with `#` and a header line are skipped.
    ///
    /// # Errors
    ///
    /// The line that cannot be read.
    pub fn parse(text: &str, submode: &str) -> Result<ChannelTable, String> {
        let mut rows = Vec::new();
        for line in text.split(['\n', ';']).map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f = fields(line);
            let (freq, rest) = match f[0].parse::<f64>() {
                Ok(_) => (&f[0], &f[1..]),
                Err(_) if f[0] == submode && f.len() >= 2 => (&f[1], &f[2..]),
                Err(_) => continue,
            };
            let hz: f64 = freq
                .parse()
                .ok()
                .filter(|v: &f64| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| format!("not a frequency: {line}"))?;
            let name = rest.first().cloned().unwrap_or_default();
            let norad = match rest.get(1).map(String::as_str) {
                None | Some("") => None,
                Some(n) => Some(
                    n.parse::<u32>()
                        .map_err(|_| format!("not a catalogue number: {line}"))?,
                ),
            };
            rows.push(Channel { hz, name, norad });
        }
        Ok(ChannelTable { rows })
    }

    /// The entries.
    pub fn rows(&self) -> &[Channel] {
        &self.rows
    }

    /// The nearest entry to `hz` within max([`NAME_MIN_HZ`], [`NAME_SHARE`] × `hz`).
    pub fn name_of(&self, hz: f64) -> Option<&Channel> {
        let reach = NAME_MIN_HZ.max(NAME_SHARE * hz.abs());
        self.rows
            .iter()
            .map(|c| ((c.hz - hz).abs(), c))
            .filter(|(d, _)| *d <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, c)| c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_of_both_forms_and_the_nearest_name() {
        let t = ChannelTable::parse(
            "submode,frequency_hz,name,norad\ntsikada,150000000,\"Tsikada, 150 MHz\",\nzombie,145970000,AO-7,7530\n400000000,UHF;# note",
            "tsikada",
        )
        .unwrap();
        assert_eq!(t.rows().len(), 2);
        assert_eq!(t.name_of(150_004_400.0).unwrap().name, "Tsikada, 150 MHz");
        assert!(t.name_of(150_004_600.0).is_none());
        assert_eq!(t.name_of(400_010_000.0).unwrap().name, "UHF");
        assert!(ChannelTable::parse("12x,a", "x").is_ok());
        assert!(ChannelTable::parse("1e6,a,b", "x").is_err());
    }
}

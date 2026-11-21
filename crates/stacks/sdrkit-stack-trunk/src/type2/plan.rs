//! The channel plan of a Type II system: rows of channel numbers with the receive frequency of the first, the step and the
//! voice channel width. The host passes a plan file (`docs/schema/plans.json`); each numbered band is one row. The stack holds
//! no plan of its own.
//!
//! A number in no row is not a channel and is read as a command. In a plan whose rows start above channel 0 (an OBT plan of
//! VHF or UHF), the numbers below the first row are transmit channel numbers: they open the OBT forms.

/// One row of channel numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    /// The first channel number.
    pub first: u16,
    /// The last channel number.
    pub last: u16,
    /// The receive frequency of the first channel, Hz.
    pub base_hz: f64,
    /// The step between channels, Hz.
    pub step_hz: f64,
    /// The voice channel width, Hz.
    pub width_hz: f64,
}

/// A channel plan.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ChannelPlan {
    /// The name of the plan.
    pub name: String,
    /// The rows, in file order.
    pub rows: Vec<Row>,
}

impl ChannelPlan {
    /// The plan of the first group of a plan file.
    ///
    /// # Errors
    ///
    /// Why the text is not a plan file, or holds no numbered band.
    pub fn parse(text: &str) -> Result<ChannelPlan, String> {
        let mut plans = sdrkit_data_plans::Plans::new();
        let ids = sdrkit_data_plans::api::import_as(
            &mut plans,
            text,
            sdrkit_data_plans::api::PlanFormat::Auto,
            "",
            "",
        )
        .map_err(|e| e.to_string())?;
        let g = ids
            .first()
            .and_then(|id| plans.group(id))
            .ok_or("the file holds no plan")?;
        let rows: Vec<Row> = g
            .group
            .bands
            .iter()
            .filter_map(|b| {
                let n = b.numbering.as_ref()?;
                Some(Row {
                    first: u16::try_from(n.first).ok()?,
                    last: u16::try_from(n.last).ok()?,
                    base_hz: n.first_centre_hz + n.offset_hz,
                    step_hz: n.step_hz,
                    width_hz: b.channel_width_hz.unwrap_or(12_500.0),
                })
            })
            .collect();
        if rows.is_empty() {
            return Err("the plan has no numbered band".into());
        }
        Ok(ChannelPlan {
            name: g.group.name.clone(),
            rows,
        })
    }

    /// The receive frequency and the voice channel width of channel `n`, when a row holds it.
    pub fn rx(&self, n: u16) -> Option<(f64, f64)> {
        self.rows
            .iter()
            .find(|r| (r.first..=r.last).contains(&n))
            .map(|r| (r.base_hz + f64::from(n - r.first) * r.step_hz, r.width_hz))
    }

    /// Channel `n` is a transmit channel number: below every row of a plan whose rows start above 0.
    pub fn is_tx(&self, n: u16) -> bool {
        self.rows
            .iter()
            .map(|r| r.first)
            .min()
            .is_some_and(|lo| n < lo)
    }

    /// The channel number of a receive frequency, within a tenth of its step.
    pub fn channel_of(&self, hz: f64) -> Option<u16> {
        self.rows.iter().find_map(|r| {
            let k = ((hz - r.base_hz) / r.step_hz).round();
            let n = f64::from(r.first) + k;
            ((hz - r.base_hz - k * r.step_hz).abs() < 0.1 * r.step_hz
                && k >= 0.0
                && n <= f64::from(r.last))
            .then_some(n as u16)
        })
    }
}

/// What the stack knows of channels: the plan when the host gave one.
#[derive(Clone, Copy, Debug)]
pub struct Channels<'a>(pub Option<&'a ChannelPlan>);

/// The first command code: without a plan, the numbers below it are channels of an unknown frequency.
pub const FIRST_COMMAND: u16 = 0x2F8;

impl Channels<'_> {
    /// Command `c` names a receive channel.
    pub fn is_channel(&self, c: u16) -> bool {
        match self.0 {
            Some(p) => p.rx(c).is_some(),
            None => c < FIRST_COMMAND,
        }
    }
    /// Command `c` is a transmit channel number of an OBT plan.
    pub fn is_tx(&self, c: u16) -> bool {
        self.0.is_some_and(|p| p.is_tx(c))
    }
    /// The receive frequency of channel `c` (0 when unknown) and the voice width (0 when unknown).
    pub fn freq(&self, c: u16) -> (f64, f64) {
        self.0.and_then(|p| p.rx(c)).unwrap_or((0.0, 0.0))
    }
}

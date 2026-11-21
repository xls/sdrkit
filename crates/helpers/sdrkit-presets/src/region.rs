//! The region choice of a host: one country and city for every stack, with another one for single stacks.

use crate::error::RegionError;
use std::fmt;

/// A country, an optional city, and the regions that single stacks use in place of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    country: String,
    city: Option<String>,
    stacks: Vec<(String, Region)>,
}

impl Region {
    /// The region of the country `code` (ISO 3166 alpha-2, any case).
    ///
    /// # Errors
    ///
    /// [`RegionError::NotACountry`] if `code` is not two ASCII letters.
    pub fn new(code: &str) -> Result<Region, RegionError> {
        let c = code.trim();
        if c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(RegionError::NotACountry(code.to_owned()));
        }
        Ok(Region {
            country: c.to_ascii_uppercase(),
            city: None,
            stacks: Vec::new(),
        })
    }

    /// This region with the city `city`. An empty name removes the city.
    pub fn with_city(mut self, city: &str) -> Region {
        let c = city.trim();
        self.city = (!c.is_empty()).then(|| c.to_owned());
        self
    }

    /// This region with `region` for the stack `stack` only.
    pub fn with_stack(mut self, stack: &str, region: Region) -> Region {
        self.stacks.retain(|(s, _)| s != stack);
        self.stacks.push((stack.to_owned(), region));
        self
    }

    /// The country code, in upper case.
    pub fn country(&self) -> &str {
        &self.country
    }

    /// The city.
    pub fn city(&self) -> Option<&str> {
        self.city.as_deref()
    }

    /// The region that applies to `stack`: its own if it has one, else this one.
    pub fn of_stack(&self, stack: &str) -> &Region {
        self.stacks
            .iter()
            .find(|(s, _)| s == stack)
            .map_or(self, |(_, r)| r)
    }

    /// True if `name` is the city of this region. Case and outer spaces do not count.
    pub(crate) fn is_city(&self, name: &str) -> bool {
        self.city
            .as_deref()
            .is_some_and(|c| c.to_lowercase() == name.trim().to_lowercase())
    }
}

impl fmt::Display for Region {
    /// `AU` or `AU/Sydney`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.city {
            Some(c) => write!(f, "{}/{c}", self.country),
            None => f.write_str(&self.country),
        }
    }
}

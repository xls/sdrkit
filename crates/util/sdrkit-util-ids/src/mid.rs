//! Maritime identification digits (MID): the three digits that name a country in an MMSI and in DSC and ATIS identities
//! (ITU Radio Regulations Appendix 43; ITU-R M.585-9). The table is the ITU "Table of Maritime Identification Digits"
//! published by ITU-R through MARS (Maritime mobile Access and Retrieval System), as in force in 2026; COSPAS-SARSAT C/S A.001
//! uses the same digits as beacon country codes.
//!
//! ```
//! use sdrkit_util_ids::mid;
//!
//! assert_eq!(mid::country(232), Some("United Kingdom"));
//! assert_eq!(mid::region(232), Some(mid::Region::Europe));
//! assert_eq!(mid::country(217), None);
//! ```

/// The region of a MID's first digit (ITU RR Appendix 43).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    /// 2: Europe.
    Europe,
    /// 3: North and Central America and the Caribbean.
    NorthAndCentralAmerica,
    /// 4: Asia.
    Asia,
    /// 5: Oceania.
    Oceania,
    /// 6: Africa.
    Africa,
    /// 7: South America.
    SouthAmerica,
}

impl Region {
    /// The region's name.
    pub fn name(self) -> &'static str {
        match self {
            Region::Europe => "Europe",
            Region::NorthAndCentralAmerica => "North and Central America and Caribbean",
            Region::Asia => "Asia",
            Region::Oceania => "Oceania",
            Region::Africa => "Africa",
            Region::SouthAmerica => "South America",
        }
    }
}

/// The region of a MID (`None` when the first digit is not 2 to 7 or the value has more than three digits).
pub fn region(mid: u16) -> Option<Region> {
    if !(200..=799).contains(&mid) {
        return None;
    }
    Some(match mid / 100 {
        2 => Region::Europe,
        3 => Region::NorthAndCentralAmerica,
        4 => Region::Asia,
        5 => Region::Oceania,
        6 => Region::Africa,
        _ => Region::SouthAmerica,
    })
}

/// Every allocated MID and its country or area, in MID order.
pub static MIDS: [(u16, &str); 292] = [
    (201, "Albania"),
    (202, "Andorra"),
    (203, "Austria"),
    (204, "Azores (Portugal)"),
    (205, "Belgium"),
    (206, "Belarus"),
    (207, "Bulgaria"),
    (208, "Vatican City State"),
    (209, "Cyprus"),
    (210, "Cyprus"),
    (211, "Germany"),
    (212, "Cyprus"),
    (213, "Georgia"),
    (214, "Moldova"),
    (215, "Malta"),
    (216, "Armenia"),
    (218, "Germany"),
    (219, "Denmark"),
    (220, "Denmark"),
    (224, "Spain"),
    (225, "Spain"),
    (226, "France"),
    (227, "France"),
    (228, "France"),
    (229, "Malta"),
    (230, "Finland"),
    (231, "Faroe Islands (Denmark)"),
    (232, "United Kingdom"),
    (233, "United Kingdom"),
    (234, "United Kingdom"),
    (235, "United Kingdom"),
    (236, "Gibraltar (United Kingdom)"),
    (237, "Greece"),
    (238, "Croatia"),
    (239, "Greece"),
    (240, "Greece"),
    (241, "Greece"),
    (242, "Morocco"),
    (243, "Hungary"),
    (244, "Netherlands"),
    (245, "Netherlands"),
    (246, "Netherlands"),
    (247, "Italy"),
    (248, "Malta"),
    (249, "Malta"),
    (250, "Ireland"),
    (251, "Iceland"),
    (252, "Liechtenstein"),
    (253, "Luxembourg"),
    (254, "Monaco"),
    (255, "Madeira (Portugal)"),
    (256, "Malta"),
    (257, "Norway"),
    (258, "Norway"),
    (259, "Norway"),
    (261, "Poland"),
    (262, "Montenegro"),
    (263, "Portugal"),
    (264, "Romania"),
    (265, "Sweden"),
    (266, "Sweden"),
    (267, "Slovakia"),
    (268, "San Marino"),
    (269, "Switzerland"),
    (270, "Czechia"),
    (271, "Turkey"),
    (272, "Ukraine"),
    (273, "Russia"),
    (274, "North Macedonia"),
    (275, "Latvia"),
    (276, "Estonia"),
    (277, "Lithuania"),
    (278, "Slovenia"),
    (279, "Serbia"),
    (301, "Anguilla (United Kingdom)"),
    (303, "Alaska (United States)"),
    (304, "Antigua and Barbuda"),
    (305, "Antigua and Barbuda"),
    (
        306,
        "Curacao, Sint Maarten, Bonaire, Sint Eustatius and Saba (Netherlands)",
    ),
    (307, "Aruba (Netherlands)"),
    (308, "Bahamas"),
    (309, "Bahamas"),
    (310, "Bermuda (United Kingdom)"),
    (311, "Bahamas"),
    (312, "Belize"),
    (314, "Barbados"),
    (316, "Canada"),
    (319, "Cayman Islands (United Kingdom)"),
    (321, "Costa Rica"),
    (323, "Cuba"),
    (325, "Dominica"),
    (327, "Dominican Republic"),
    (329, "Guadeloupe (France)"),
    (330, "Grenada"),
    (331, "Greenland (Denmark)"),
    (332, "Guatemala"),
    (334, "Honduras"),
    (336, "Haiti"),
    (338, "United States"),
    (339, "Jamaica"),
    (341, "Saint Kitts and Nevis"),
    (343, "Saint Lucia"),
    (345, "Mexico"),
    (347, "Martinique (France)"),
    (348, "Montserrat (United Kingdom)"),
    (350, "Nicaragua"),
    (351, "Panama"),
    (352, "Panama"),
    (353, "Panama"),
    (354, "Panama"),
    (355, "Panama"),
    (356, "Panama"),
    (357, "Panama"),
    (358, "Puerto Rico (United States)"),
    (359, "El Salvador"),
    (361, "Saint Pierre and Miquelon (France)"),
    (362, "Trinidad and Tobago"),
    (364, "Turks and Caicos Islands (United Kingdom)"),
    (366, "United States"),
    (367, "United States"),
    (368, "United States"),
    (369, "United States"),
    (370, "Panama"),
    (371, "Panama"),
    (372, "Panama"),
    (373, "Panama"),
    (374, "Panama"),
    (375, "Saint Vincent and the Grenadines"),
    (376, "Saint Vincent and the Grenadines"),
    (377, "Saint Vincent and the Grenadines"),
    (378, "British Virgin Islands (United Kingdom)"),
    (379, "Virgin Islands (United States)"),
    (401, "Afghanistan"),
    (403, "Saudi Arabia"),
    (405, "Bangladesh"),
    (408, "Bahrain"),
    (410, "Bhutan"),
    (412, "China"),
    (413, "China"),
    (414, "China"),
    (416, "Taiwan (China)"),
    (417, "Sri Lanka"),
    (419, "India"),
    (422, "Iran"),
    (423, "Azerbaijan"),
    (425, "Iraq"),
    (428, "Israel"),
    (431, "Japan"),
    (432, "Japan"),
    (434, "Turkmenistan"),
    (436, "Kazakhstan"),
    (437, "Uzbekistan"),
    (438, "Jordan"),
    (440, "Korea (Republic of)"),
    (441, "Korea (Republic of)"),
    (443, "Palestine"),
    (445, "Korea (Democratic People's Republic of)"),
    (447, "Kuwait"),
    (450, "Lebanon"),
    (451, "Kyrgyzstan"),
    (453, "Macao (China)"),
    (455, "Maldives"),
    (457, "Mongolia"),
    (459, "Nepal"),
    (461, "Oman"),
    (463, "Pakistan"),
    (466, "Qatar"),
    (468, "Syria"),
    (470, "United Arab Emirates"),
    (471, "United Arab Emirates"),
    (472, "Tajikistan"),
    (473, "Yemen"),
    (475, "Yemen"),
    (477, "Hong Kong (China)"),
    (478, "Bosnia and Herzegovina"),
    (501, "Adelie Land (France)"),
    (503, "Australia"),
    (506, "Myanmar"),
    (508, "Brunei Darussalam"),
    (510, "Micronesia"),
    (511, "Palau"),
    (512, "New Zealand"),
    (514, "Cambodia"),
    (515, "Cambodia"),
    (516, "Christmas Island (Australia)"),
    (518, "Cook Islands (New Zealand)"),
    (520, "Fiji"),
    (523, "Cocos (Keeling) Islands (Australia)"),
    (525, "Indonesia"),
    (529, "Kiribati"),
    (531, "Laos"),
    (533, "Malaysia"),
    (536, "Northern Mariana Islands (United States)"),
    (538, "Marshall Islands"),
    (540, "New Caledonia (France)"),
    (542, "Niue (New Zealand)"),
    (544, "Nauru"),
    (546, "French Polynesia (France)"),
    (548, "Philippines"),
    (550, "Timor-Leste"),
    (553, "Papua New Guinea"),
    (555, "Pitcairn Island (United Kingdom)"),
    (557, "Solomon Islands"),
    (559, "American Samoa (United States)"),
    (561, "Samoa"),
    (563, "Singapore"),
    (564, "Singapore"),
    (565, "Singapore"),
    (566, "Singapore"),
    (567, "Thailand"),
    (570, "Tonga"),
    (572, "Tuvalu"),
    (574, "Viet Nam"),
    (576, "Vanuatu"),
    (577, "Vanuatu"),
    (578, "Wallis and Futuna Islands (France)"),
    (601, "South Africa"),
    (603, "Angola"),
    (605, "Algeria"),
    (607, "Saint Paul and Amsterdam Islands (France)"),
    (608, "Ascension Island (United Kingdom)"),
    (609, "Burundi"),
    (610, "Benin"),
    (611, "Botswana"),
    (612, "Central African Republic"),
    (613, "Cameroon"),
    (615, "Congo"),
    (616, "Comoros"),
    (617, "Cabo Verde"),
    (618, "Crozet Archipelago (France)"),
    (619, "Cote d'Ivoire"),
    (620, "Comoros"),
    (621, "Djibouti"),
    (622, "Egypt"),
    (624, "Ethiopia"),
    (625, "Eritrea"),
    (626, "Gabon"),
    (627, "Ghana"),
    (629, "Gambia"),
    (630, "Guinea-Bissau"),
    (631, "Equatorial Guinea"),
    (632, "Guinea"),
    (633, "Burkina Faso"),
    (634, "Kenya"),
    (635, "Kerguelen Islands (France)"),
    (636, "Liberia"),
    (637, "Liberia"),
    (638, "South Sudan"),
    (642, "Libya"),
    (644, "Lesotho"),
    (645, "Mauritius"),
    (647, "Madagascar"),
    (649, "Mali"),
    (650, "Mozambique"),
    (654, "Mauritania"),
    (655, "Malawi"),
    (656, "Niger"),
    (657, "Nigeria"),
    (659, "Namibia"),
    (660, "Reunion (France)"),
    (661, "Rwanda"),
    (662, "Sudan"),
    (663, "Senegal"),
    (664, "Seychelles"),
    (665, "Saint Helena (United Kingdom)"),
    (666, "Somalia"),
    (667, "Sierra Leone"),
    (668, "Sao Tome and Principe"),
    (669, "Eswatini"),
    (670, "Chad"),
    (671, "Togo"),
    (672, "Tunisia"),
    (674, "Tanzania"),
    (675, "Uganda"),
    (676, "Democratic Republic of the Congo"),
    (677, "Tanzania"),
    (678, "Zambia"),
    (679, "Zimbabwe"),
    (701, "Argentina"),
    (710, "Brazil"),
    (720, "Bolivia"),
    (725, "Chile"),
    (730, "Colombia"),
    (735, "Ecuador"),
    (740, "Falkland Islands (United Kingdom)"),
    (745, "Guiana (France)"),
    (750, "Guyana"),
    (755, "Paraguay"),
    (760, "Peru"),
    (765, "Suriname"),
    (770, "Uruguay"),
    (775, "Venezuela"),
];

/// The country or area a MID is allocated to.
pub fn country(mid: u16) -> Option<&'static str> {
    MIDS.binary_search_by_key(&mid, |m| m.0)
        .ok()
        .map(|i| MIDS[i].1)
}

/// Whether a MID has the form the table uses: first digit 2 to 7 and last two digits 01 to 79. Every allocated MID has this
/// form; an AIS frame whose MID does not is almost certainly noise that passed the CRC.
pub fn has_valid_form(mid: u16) -> bool {
    (200..=799).contains(&mid) && (1..=79).contains(&(mid % 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_well_formed() {
        assert!(MIDS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(MIDS.iter().all(|m| has_valid_form(m.0)));
    }

    #[test]
    fn known_mids() {
        assert_eq!(country(215), Some("Malta"));
        assert_eq!(country(338), Some("United States"));
        assert_eq!(country(245), Some("Netherlands"));
        assert_eq!(country(374), Some("Panama"));
        assert_eq!(country(550), Some("Timor-Leste"));
        assert_eq!(country(277), Some("Lithuania"));
        assert_eq!(country(775), Some("Venezuela"));
        assert_eq!(country(201), Some("Albania"));
        assert_eq!(country(200), None);
        assert_eq!(country(999), None);
        assert_eq!(region(503), Some(Region::Oceania));
        assert_eq!(region(199), None);
        assert!(!has_valid_form(280) && !has_valid_form(800) && !has_valid_form(300));
    }
}

//! A unit has one spelling in every interface: `°` for degrees, `kt` for knots, `B` for bytes, `1/s` for a rate. A
//! front end formats a value by its unit text, so two spellings of one unit are two units for it.

#[test]
fn a_unit_has_one_spelling() {
    let mut families = sdrkit_host_catalog::families();
    let shared = sdrkit_core::interface::shared_of(&families);
    families.extend(shared);
    families.extend(sdrkit_host_engine::object_families().iter());
    assert!(families.len() >= 40, "{} families", families.len());
    let problems: Vec<String> = families
        .iter()
        .flat_map(|f| sdrkit_dev_interface::units::spellings(f))
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

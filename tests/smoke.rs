use lojix_macros::{domain, domain_registry, fold};

// Individual domain! still works
domain!(Phase);

// domain_registry! generates everything from the DB
mod registry {
    lojix_macros::domain_registry!();
}

#[test]
fn individual_domain_works() {
    assert_eq!(Phase::Manifest.name(), "manifest");
    assert_eq!(Phase::COUNT, 3);
}

#[test]
fn phase_discriminant_roundtrip() {
    for phase in Phase::all() {
        let d = phase.discriminant();
        let recovered = Phase::from_discriminant(d).unwrap();
        assert_eq!(*phase, recovered);
    }
}

#[test]
fn phase_from_str() {
    assert_eq!("manifest".parse::<Phase>().unwrap(), Phase::Manifest);
    assert!("bogus".parse::<Phase>().is_err());
}

#[test]
fn fold_exhaustive() {
    let val = Phase::Manifest;
    let result = fold!(Phase, val, {
        Becoming => "becoming",
        Manifest => "manifest",
        Retired => "retired"
    });
    assert_eq!(result, "manifest");
}

#[test]
fn registry_generates_all_domains() {
    // domain_registry! generates these from the DB — not hand-listed
    assert!(registry::DOMAIN_COUNT > 0);
    assert!(registry::DOMAIN_NAMES.len() == registry::DOMAIN_COUNT);
}

#[test]
fn registry_translate_works() {
    // Phase discriminant 1 should be "manifest" (alphabetical order)
    let result = registry::translate_domain("Phase", 1);
    assert_eq!(result, Some("manifest"));
}

#[test]
fn registry_translate_unknown_domain() {
    assert_eq!(registry::translate_domain("Bogus", 0), None);
}

#[test]
fn registry_element_exists() {
    assert_eq!(registry::Element::COUNT, 4);
    assert_eq!(registry::Element::Fire.name(), "fire");
}

#[test]
fn registry_dignity_exists() {
    assert_eq!(registry::Dignity::COUNT, 5);
}

use vl_convert_rs::module_loader::import_map::VL_VERSIONS;
use vl_convert_rs::VlVersion;

#[test]
fn test_vl_version_aliases_and_patch_versions() {
    for &version in VL_VERSIONS {
        let major_minor = version.to_semver();
        let underscored = major_minor.replace('.', "_");
        for input in [
            major_minor.to_string(),
            format!("v{major_minor}"),
            underscored.clone(),
            format!("v{underscored}"),
            format!("{major_minor}.0"),
            format!("{major_minor}.123"),
            format!("v{major_minor}.1"),
        ] {
            assert_eq!(input.parse::<VlVersion>().unwrap(), version, "{input}");
        }
    }
}

#[test]
fn test_vl_version_rejects_invalid_or_unsupported_versions() {
    for input in [
        "",
        "6",
        "99.99.1",
        "6.4.",
        "6.4.1.2",
        "6.4.-1",
        "6.4.01",
        "6.4.x",
        "6.4.1-rc.1",
        "6.4.1+build",
        "6_4_1",
        " 6.4.1",
    ] {
        assert!(input.parse::<VlVersion>().is_err(), "{input}");
    }
}

use super::performance_support::{section_from, source};

#[test]
fn release_profile_stays_throughput_optimized() {
    let cargo = source("Cargo.toml");
    let release = section_from(&cargo, "[profile.release]");

    assert!(
        release.contains("opt-level = 3"),
        "performance regression: release builds must remain optimized for runtime throughput"
    );
    assert!(
        !release.contains("opt-level = \"z\"") && !release.contains("opt-level = 'z'"),
        "performance regression: size-first opt-level=z must not replace throughput optimization"
    );
}

use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("failed to read performance guard source '{path}': {error}"))
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("performance guard start marker missing: {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("performance guard end marker missing: {end}"));
    &rest[..end_index]
}

fn section_from<'a>(source: &'a str, start: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("performance guard start marker missing: {start}"));
    &source[start_index..]
}

#[test]
fn phase3_release_profile_stays_throughput_optimized() {
    let cargo = source("Cargo.toml");
    let release = section_from(&cargo, "[profile.release]");

    assert!(
        release.contains("opt-level = 3"),
        "Phase 3 regression: release builds must remain optimized for runtime throughput"
    );
    assert!(
        !release.contains("opt-level = \"z\"") && !release.contains("opt-level = 'z'"),
        "Phase 3 regression: size-first opt-level=z must not replace throughput optimization"
    );
}

#[test]
fn phase3_metadata_cache_capacity_eviction_stays_amortized_o1() {
    let cache = source("src/services/cache.rs");
    let put = section(&cache, "pub async fn put(", "/// Single-flight");
    let eviction = section(&cache, "fn evict_one(&mut self)", "type InFlightSender");

    assert!(
        put.contains("state.evict_one();"),
        "Phase 3 regression: capacity enforcement must use the incremental eviction queue"
    );
    assert!(
        !put.contains("min_by_key"),
        "Phase 3 regression: MetadataCache::put must not scan the map for an eviction candidate"
    );
    assert!(
        !put.contains(".retain("),
        "Phase 3 regression: MetadataCache::put must not perform an O(N) retain pass"
    );
    assert!(
        eviction.contains("pop_front()"),
        "Phase 3 regression: eviction must advance the FIFO generation queue incrementally"
    );
}

#[test]
fn phase3_ascii_search_fast_path_stays_allocation_free() {
    let search = source("src/filesystem/search.rs");
    let matcher = section(
        &search,
        "fn matches(&self, name: &str)",
        "fn ascii_contains_ignore_case",
    );

    assert!(
        matcher.contains("if *query_is_ascii && name.is_ascii()"),
        "Phase 3 regression: ASCII filename search must retain its allocation-free fast path"
    );
    assert!(
        matcher.contains("ascii_contains_ignore_case("),
        "Phase 3 regression: ASCII filename search must not fall back to per-entry lowercase allocation"
    );
}

#[test]
fn phase3_transfer_progress_producers_stay_near_four_hz() {
    let engine = source("src/transfer/engine.rs");
    let executor = source("src/transfer/executor.rs");

    assert!(
        engine.contains("interval(Duration::from_millis(250))"),
        "Phase 3 regression: directory transfer progress ticker must remain at 250 ms"
    );
    assert!(
        engine.contains("duration_since(last_emit) >= Duration::from_millis(250)"),
        "Phase 3 regression: streaming transfer progress producer must remain at 250 ms"
    );
    assert!(
        engine.contains("|| transferred == total_bytes"),
        "Phase 3 regression: final single-file progress must still bypass the cadence delay"
    );
    assert!(
        !engine.contains("interval(Duration::from_millis(100))"),
        "Phase 3 regression: 100 ms directory progress wakeups were reintroduced"
    );
    assert!(
        !engine.contains("duration_since(last_emit) >= Duration::from_millis(100)"),
        "Phase 3 regression: 100 ms streaming progress updates were reintroduced"
    );

    assert!(
        executor.contains("duration_since(last_emit).as_millis() >= 250"),
        "Phase 3 regression: inline upload progress producer must remain at 250 ms"
    );
    assert!(
        !executor.contains("duration_since(last_emit).as_millis() >= 100"),
        "Phase 3 regression: 100 ms inline-upload progress updates were reintroduced"
    );
}

#[test]
fn phase3_event_progress_gate_stays_allocation_free_for_current_fields() {
    let journal = source("src/events/journal.rs");
    let gate = section(
        &journal,
        "impl ProgressBroadcastGate",
        "/// Durable Event Journal",
    );
    let state = section(
        &journal,
        "struct ProgressEmissionState",
        "#[derive(Debug, Default)]",
    );

    assert!(
        journal.contains("const TRANSFER_PROGRESS_BROADCAST_INTERVAL: Duration = Duration::from_millis(250);"),
        "Phase 3 regression: EventJournal progress coalescing must remain at 250 ms"
    );
    assert!(
        journal.contains("const PROGRESS_FIELD_INLINE_CAPACITY: usize = 32;"),
        "Phase 3 regression: current phase/status values must retain inline storage"
    );
    assert!(
        gate.contains("ProgressFieldKey::from_optional"),
        "Phase 3 regression: progress gate must use inline phase/status keys"
    );
    assert!(
        !gate.contains(".map(str::to_owned)"),
        "Phase 3 regression: per-tick phase/status String allocation was reintroduced"
    );
    assert!(
        !state.contains("Option<String>"),
        "Phase 3 regression: progress emission state must not own per-tick String snapshots"
    );
}

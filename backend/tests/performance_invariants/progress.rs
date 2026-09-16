use super::performance_support::{section, source};

#[test]
fn transfer_progress_producers_stay_near_four_hz() {
    let engine = source("src/transfer/engine.rs");
    let executor = source("src/transfer/executor.rs");

    assert!(
        engine.contains("interval(Duration::from_millis(250))"),
        "performance regression: directory transfer progress ticker must remain at 250 ms"
    );
    assert!(
        engine.contains("duration_since(last_emit) >= Duration::from_millis(250)"),
        "performance regression: streaming transfer progress producer must remain at 250 ms"
    );
    assert!(
        engine.contains("|| transferred == total_bytes"),
        "performance regression: final single-file progress must still bypass the cadence delay"
    );
    assert!(
        !engine.contains("interval(Duration::from_millis(100))"),
        "performance regression: 100 ms directory progress wakeups were reintroduced"
    );
    assert!(
        !engine.contains("duration_since(last_emit) >= Duration::from_millis(100)"),
        "performance regression: 100 ms streaming progress updates were reintroduced"
    );

    assert!(
        executor.contains("duration_since(last_emit).as_millis() >= 250"),
        "performance regression: inline upload progress producer must remain at 250 ms"
    );
    assert!(
        !executor.contains("duration_since(last_emit).as_millis() >= 100"),
        "performance regression: 100 ms inline-upload progress updates were reintroduced"
    );
}

#[test]
fn event_progress_gate_stays_allocation_free_for_current_fields() {
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
        "performance regression: EventJournal progress coalescing must remain at 250 ms"
    );
    assert!(
        journal.contains("const PROGRESS_FIELD_INLINE_CAPACITY: usize = 32;"),
        "performance regression: current phase/status values must retain inline storage"
    );
    assert!(
        gate.contains("ProgressFieldKey::from_optional"),
        "performance regression: progress gate must use inline phase/status keys"
    );
    assert!(
        !gate.contains(".map(str::to_owned)"),
        "performance regression: per-tick phase/status String allocation was reintroduced"
    );
    assert!(
        !state.contains("Option<String>"),
        "performance regression: progress emission state must not own per-tick String snapshots"
    );
}

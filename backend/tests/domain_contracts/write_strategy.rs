use backend::domain::{Capabilities, CommitSemantics, WriteStrategy};

#[test]
fn write_strategy_tracks_provider_commit_capabilities_and_overwrite_safety() {
    let atomic_rename = Capabilities {
        atomic_rename: true,
        ..Default::default()
    };
    let strategy = WriteStrategy::select(&atomic_rename, true);
    assert_eq!(strategy.semantics, CommitSemantics::AtomicRename);
    assert!(strategy.safe_overwrite);
    assert!(strategy.staging_suffix.is_some());

    let atomic_write = Capabilities {
        atomic_write: true,
        ..Default::default()
    };
    let strategy = WriteStrategy::select(&atomic_write, true);
    assert_eq!(strategy.semantics, CommitSemantics::AtomicObjectPut);
    assert!(strategy.safe_overwrite);
    assert!(strategy.staging_suffix.is_none());

    let direct = Capabilities::default();
    let create_strategy = WriteStrategy::select(&direct, false);
    assert_eq!(create_strategy.semantics, CommitSemantics::DirectWrite);
    assert!(create_strategy.safe_overwrite);

    let overwrite_strategy = WriteStrategy::select(&direct, true);
    assert_eq!(overwrite_strategy.semantics, CommitSemantics::DirectWrite);
    assert!(!overwrite_strategy.safe_overwrite);
}

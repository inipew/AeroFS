use backend::domain::conflict::{ConflictPolicy, ConflictResolver};

#[test]
fn conflict_resolver_preserves_fail_skip_replace_and_rename_semantics() {
    assert!(ConflictResolver::resolve_collision(ConflictPolicy::Fail, "file.txt").is_err());
    assert_eq!(
        ConflictResolver::resolve_collision(ConflictPolicy::Skip, "file.txt").unwrap(),
        None
    );
    assert_eq!(
        ConflictResolver::resolve_collision(ConflictPolicy::Replace, "file.txt").unwrap(),
        Some("file.txt".to_string())
    );
    assert_eq!(
        ConflictResolver::resolve_collision(ConflictPolicy::Rename, "photo.png").unwrap(),
        Some("photo_copy.png".to_string())
    );
}

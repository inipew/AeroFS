use backend::errors::SecurityError;
use backend::filesystem::SafePath;
use tempfile::tempdir;

#[test]
fn test_safepath_comprehensive_security() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("sandbox_root");
    std::fs::create_dir_all(&root).unwrap();

    // 1. Normal valid paths
    let safe1 = SafePath::resolve(&root, "/docs/readme.txt", false).unwrap();
    assert_eq!(safe1.to_vfs_str(), "/docs/readme.txt");

    let safe2 = SafePath::resolve(&root, "images/sub/photo.png", false).unwrap();
    assert_eq!(safe2.to_vfs_str(), "/images/sub/photo.png");

    let safe3 = SafePath::resolve(&root, "/", false).unwrap();
    assert_eq!(safe3.to_vfs_str(), "/");

    // 2. Traversal attempts
    let attacks = vec![
        "../etc/passwd",
        "../../etc/shadow",
        "/../../../",
        "docs/../../../../etc/hosts",
        "a/b/c/../../../../../../",
        "..",
        "../",
        "foo/bar/../../../baz",
    ];

    for attack in attacks {
        let res = SafePath::resolve(&root, attack, false);
        assert!(
            res.is_err(),
            "Attack '{}' should have been rejected by SafePath",
            attack
        );
        match res.unwrap_err() {
            SecurityError::PathTraversal(_) => {}
            other => panic!("Expected PathTraversal for '{}', got: {:?}", attack, other),
        }
    }

    // 3. Null byte injection
    let null_attack = "file.txt\0.jpg";
    let res = SafePath::resolve(&root, null_attack, false);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), SecurityError::InvalidPath(_)));
}

#[cfg(unix)]
#[test]
fn test_safepath_symlink_containment() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let sandbox = temp.path().join("sandbox");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&sandbox).unwrap();
    std::fs::create_dir_all(&outside).unwrap();

    let outside_secret = outside.join("passwords.txt");
    std::fs::write(&outside_secret, "super_secret").unwrap();

    // Create internal folder and internal file
    let internal_dir = sandbox.join("allowed_dir");
    std::fs::create_dir_all(&internal_dir).unwrap();
    let internal_file = internal_dir.join("public.txt");
    std::fs::write(&internal_file, "public_content").unwrap();

    // Symlink 1: internal symlink pointing to another location inside sandbox (ALLOWED)
    let internal_link = sandbox.join("internal_link");
    symlink(&internal_dir, &internal_link).unwrap();

    let safe_internal = SafePath::resolve(&sandbox, "internal_link/public.txt", false);
    assert!(safe_internal.is_ok());

    // Symlink 2: malicious symlink pointing to outside folder (MUST BE BLOCKED)
    let malicious_link = sandbox.join("malicious_link");
    symlink(&outside, &malicious_link).unwrap();

    let safe_malicious = SafePath::resolve(&sandbox, "malicious_link/passwords.txt", false);
    assert!(safe_malicious.is_err());
    assert!(matches!(
        safe_malicious.unwrap_err(),
        SecurityError::SymlinkEscape(_)
    ));
}

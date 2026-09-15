use backend::cli::daemon_lock::DaemonLock;

#[test]
fn daemon_lock_is_exclusive_and_can_be_reacquired_after_release() {
    let temp = tempfile::tempdir().unwrap();
    let lock_path = temp.path().join("aerofs.lock");

    let first = DaemonLock::acquire(&lock_path).expect("first daemon lock should succeed");
    assert!(
        DaemonLock::acquire(&lock_path).is_err(),
        "second daemon lock must fail while the first owner is alive"
    );

    first.release();

    let second = DaemonLock::acquire(&lock_path)
        .expect("daemon lock should become available after explicit release");
    second.release();
}

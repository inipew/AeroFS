use crate::support::{list_sync_jobs_page, list_sync_operations_page, TestAppBuilder};
use std::collections::HashSet;

async fn insert_job(app: &crate::support::TestApp, id: &str, created_at: &str) {
    sqlx::query(
        "INSERT INTO sync_jobs (id, user_id, source_connection_id, source_path, destination_connection_id, destination_path, status, strategy, total_files, synced_files, conflict_files, created_at, updated_at) VALUES (?, 'history-user', 'local', '/src', 'local', '/dst', 'completed', 'source_wins', 1, 1, 0, ?, ?)",
    )
    .bind(id)
    .bind(created_at)
    .bind(created_at)
    .execute(&app.db)
    .await
    .unwrap();
}

#[tokio::test]
async fn job_history_uses_stable_keyset_pages_without_duplicates() {
    let app = TestAppBuilder::new().running().build().await;
    for index in 0..25usize {
        insert_job(
            &app,
            &format!("history-job-{index:04}"),
            &format!("2026-09-14T08:00:{index:02}+00:00"),
        )
        .await;
    }

    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let page = list_sync_jobs_page(&app, cursor.as_ref(), Some(10)).await;
        assert!(page.items.len() <= 10);
        ids.extend(page.items.into_iter().map(|job| job.id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    assert_eq!(ids.len(), 25);
    let unique: HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 25);
    assert_eq!(ids.first().map(String::as_str), Some("history-job-0024"));
    assert_eq!(ids.last().map(String::as_str), Some("history-job-0000"));
}

#[tokio::test]
async fn operation_history_pagination_never_crosses_job_boundary() {
    let app = TestAppBuilder::new().running().build().await;
    insert_job(&app, "job-a", "2026-09-14T08:10:00+00:00").await;
    insert_job(&app, "job-b", "2026-09-14T08:11:00+00:00").await;

    for index in 0..25usize {
        let created_at = format!("2026-09-14T09:00:{index:02}+00:00");
        for job_id in ["job-a", "job-b"] {
            sqlx::query(
                "INSERT INTO sync_operations (id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, error_message, created_at, updated_at) VALUES (?, ?, 'noop', ?, NULL, 'completed', NULL, NULL, ?, ?)",
            )
            .bind(format!("{job_id}-op-{index:04}"))
            .bind(job_id)
            .bind(format!("file-{index:04}.txt"))
            .bind(&created_at)
            .bind(&created_at)
            .execute(&app.db)
            .await
            .unwrap();
        }
    }

    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let page = list_sync_operations_page(&app, "job-a", cursor.as_ref(), Some(7)).await;
        assert!(page.items.len() <= 7);
        assert!(page.items.iter().all(|operation| operation.job_id == "job-a"));
        ids.extend(page.items.into_iter().map(|operation| operation.id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    assert_eq!(ids.len(), 25);
    assert_eq!(ids.first().map(String::as_str), Some("job-a-op-0000"));
    assert_eq!(ids.last().map(String::as_str), Some("job-a-op-0024"));
}

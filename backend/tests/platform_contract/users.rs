use backend::bootstrap::build_user_service;

use crate::support::TestDatabase;

#[tokio::test]
async fn user_service_preserves_last_admin_and_supports_normal_admin_mutations() {
    let database = TestDatabase::seeded("platform_users.db").await;
    let users = build_user_service(database.pool.clone());

    let user_list = users.list_users().await.unwrap();
    assert_eq!(user_list.len(), 1);
    assert_eq!(user_list[0].username, "admin");
    assert!(user_list[0].is_admin);

    assert!(users.delete_user("admin").await.is_err());
    assert!(users.set_admin_role("admin", false).await.is_err());

    let bob_id = users
        .create_user("bob", "bob_secure_password_123", true)
        .await
        .unwrap();
    assert!(!bob_id.is_empty());

    users.set_admin_role("bob", false).await.unwrap();
    users
        .update_password("bob", "new_bob_pass_456")
        .await
        .unwrap();
    users.delete_user("bob").await.unwrap();
}

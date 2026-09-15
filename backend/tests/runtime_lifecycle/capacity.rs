use axum::extract::FromRef;
use backend::{
    config::AppConfig,
    state::{ArchiveState, SearchState},
};

use crate::support::TestAppBuilder;

#[tokio::test]
async fn archive_and_search_capacity_follow_configured_default_limits() {
    let expected = AppConfig::default().limits;
    let app = TestAppBuilder::new().build().await;

    let archive = ArchiveState::from_ref(&app.state);
    assert_eq!(
        archive.service.available_capacity(),
        expected.archive_concurrency
    );

    let search = SearchState::from_ref(&app.state);
    assert_eq!(
        search.service.available_capacity(),
        expected.search_concurrency
    );
}

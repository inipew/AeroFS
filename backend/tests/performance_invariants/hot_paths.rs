use super::performance_support::{section, source};

#[test]
fn metadata_cache_capacity_eviction_stays_amortized_o1() {
    let cache = source("src/services/cache.rs");
    let put = section(&cache, "pub async fn put(", "/// Single-flight");
    let eviction = section(&cache, "fn evict_one(&mut self)", "type InFlightSender");

    assert!(
        put.contains("state.evict_one();"),
        "performance regression: capacity enforcement must use the incremental eviction queue"
    );
    assert!(
        !put.contains("min_by_key"),
        "performance regression: MetadataCache::put must not scan the map for an eviction candidate"
    );
    assert!(
        !put.contains(".retain("),
        "performance regression: MetadataCache::put must not perform an O(N) retain pass"
    );
    assert!(
        eviction.contains("pop_front()"),
        "performance regression: eviction must advance the FIFO generation queue incrementally"
    );
}

#[test]
fn ascii_search_fast_path_stays_allocation_free() {
    let search = source("src/filesystem/search.rs");
    let matcher = section(
        &search,
        "fn matches(&self, name: &str)",
        "fn ascii_contains_ignore_case",
    );

    assert!(
        matcher.contains("if *query_is_ascii && name.is_ascii()"),
        "performance regression: ASCII filename search must retain its allocation-free fast path"
    );
    assert!(
        matcher.contains("ascii_contains_ignore_case("),
        "performance regression: ASCII filename search must not fall back to per-entry lowercase allocation"
    );
}

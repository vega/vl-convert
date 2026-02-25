use std::collections::HashSet;
use vl_convert_fontsource::FontsourceCache;

/// Helper to create a cache in a temp directory.
fn temp_cache() -> (tempfile::TempDir, FontsourceCache) {
    let tmp = tempfile::tempdir().unwrap();
    let cache = FontsourceCache::new(Some(tmp.path().to_path_buf()), None).unwrap();
    (tmp, cache)
}

#[tokio::test]
async fn test_fetch_roboto() {
    let (tmp, cache) = temp_cache();

    // First fetch should download
    let outcome = cache.fetch("Roboto").await.unwrap();
    assert!(outcome.downloaded);
    assert_eq!(outcome.font_id, "roboto");
    assert!(outcome.path.exists());

    // Marker should exist
    let marker_path = outcome.path.join(".fontsource.json");
    assert!(marker_path.exists());

    // Should have TTF files
    let ttf_count = std::fs::read_dir(&outcome.path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
        })
        .count();
    assert!(ttf_count > 0, "Expected at least one TTF file");

    // Second fetch should be a cache hit
    let outcome2 = cache.fetch("Roboto").await.unwrap();
    assert!(!outcome2.downloaded);
    assert_eq!(outcome2.font_id, "roboto");

    drop(cache);
    drop(tmp);
}

#[tokio::test]
async fn test_font_not_found() {
    let (_tmp, cache) = temp_cache();

    let result = cache.fetch("definitely-not-a-real-font-name-xyz").await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        matches!(err, vl_convert_fontsource::FontsourceError::FontNotFound(_)),
        "Expected FontNotFound, got: {:?}",
        err
    );
}

#[tokio::test]
async fn test_is_known_font() {
    let (_tmp, cache) = temp_cache();

    // Roboto should be known
    assert!(cache.is_known_font("roboto").await.unwrap());

    // Nonsense should not be known
    assert!(!cache
        .is_known_font("definitely-not-a-font-xyz")
        .await
        .unwrap());

    // Second call should hit in-memory cache
    assert!(cache.is_known_font("roboto").await.unwrap());
}

#[tokio::test]
async fn test_fetch_and_refetch() {
    let (tmp, cache) = temp_cache();

    // Initial fetch
    let outcome = cache.fetch("Open Sans").await.unwrap();
    assert!(outcome.downloaded);
    assert_eq!(outcome.font_id, "open-sans");

    // Refetch should force re-download
    let outcome2 = cache.refetch("Open Sans").await.unwrap();
    assert!(outcome2.downloaded);

    drop(cache);
    drop(tmp);
}

#[tokio::test]
async fn test_eviction_during_fetch() {
    let (tmp, cache) = temp_cache();

    // Fetch a font
    let outcome1 = cache.fetch("Roboto").await.unwrap();
    assert!(outcome1.downloaded);

    // Fetch another font
    let outcome2 = cache.fetch("Open Sans").await.unwrap();
    assert!(outcome2.downloaded);

    // Calculate current size
    let size = cache.calculate_cache_size_bytes().unwrap();
    assert!(size > 0);

    // Set cache limit to just above one font's size (force eviction of one)
    // Use half the current size as the limit
    let target = size / 2;

    // Evict — oldest (roboto, fetched first) should be evicted
    let exempt: HashSet<String> = HashSet::from(["open-sans".to_string()]);
    cache.evict_lru_until_size(target, &exempt).unwrap();

    // Open Sans (exempt) should remain
    assert!(
        tmp.path().join("open-sans").exists(),
        "Exempt font should not be evicted"
    );

    drop(cache);
    drop(tmp);
}

#[tokio::test]
async fn test_parallel_same_font_dedup() {
    let (_tmp, cache) = temp_cache();
    let cache = std::sync::Arc::new(cache);

    // Spawn two concurrent fetches for the same font
    let cache1 = cache.clone();
    let cache2 = cache.clone();

    let (r1, r2) = tokio::join!(async move { cache1.fetch("Roboto").await }, async move {
        cache2.fetch("Roboto").await
    },);

    // Both should succeed
    let o1 = r1.unwrap();
    let o2 = r2.unwrap();

    // At most one should report downloaded=true (the other gets dedup'd)
    let download_count = [o1.downloaded, o2.downloaded]
        .iter()
        .filter(|&&d| d)
        .count();
    assert!(
        download_count <= 1,
        "Expected at most 1 download, got {}",
        download_count
    );
}

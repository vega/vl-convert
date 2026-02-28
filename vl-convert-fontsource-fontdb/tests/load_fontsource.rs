use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use vl_convert_fontsource_fontdb::{
    ClientConfig, FontStyle, FontsourceClient, FontsourceDatabaseExt, FontsourceFontdbError,
    VariantRequest,
};

const REGULAR_TTF: &[u8] =
    include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Regular.ttf");
const BOLD_TTF: &[u8] = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Bold.ttf");
const ITALIC_TTF: &[u8] =
    include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-RegularItalic.ttf");

struct TestServer {
    base_url: String,
    hits: Arc<Mutex<HashMap<String, usize>>>,
    inflight_ttf: Arc<AtomicUsize>,
    max_inflight_ttf: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn new<F>(route_builder: F, delayed_paths: HashSet<String>, delay_ms: u64) -> Self
    where
        F: FnOnce(&str) -> HashMap<String, Vec<u8>>,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();

        let base_url = format!("http://{}", addr);
        let routes = Arc::new(route_builder(&base_url));

        let hits = Arc::new(Mutex::new(HashMap::new()));
        let inflight_ttf = Arc::new(AtomicUsize::new(0));
        let max_inflight_ttf = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let delayed_paths = Arc::new(delayed_paths);

        let thread_hits = Arc::clone(&hits);
        let thread_inflight = Arc::clone(&inflight_ttf);
        let thread_max_inflight = Arc::clone(&max_inflight_ttf);
        let thread_stop = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let routes = Arc::clone(&routes);
                        let delayed_paths = Arc::clone(&delayed_paths);
                        let hits = Arc::clone(&thread_hits);
                        let inflight = Arc::clone(&thread_inflight);
                        let max_inflight = Arc::clone(&thread_max_inflight);

                        thread::spawn(move || {
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                            let mut req = Vec::new();
                            let mut buf = [0u8; 4096];
                            loop {
                                match stream.read(&mut buf) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        req.extend_from_slice(&buf[..n]);
                                        if req.windows(4).any(|w| w == b"\r\n\r\n") {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }

                            let req_str = String::from_utf8_lossy(&req);
                            let path = req_str
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .unwrap_or("/")
                                .to_string();

                            {
                                let mut guard = hits.lock().unwrap();
                                *guard.entry(path.clone()).or_insert(0) += 1;
                            }

                            let is_ttf = path.ends_with(".ttf");
                            if is_ttf {
                                let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                                loop {
                                    let prev = max_inflight.load(Ordering::SeqCst);
                                    if now <= prev {
                                        break;
                                    }
                                    if max_inflight
                                        .compare_exchange(
                                            prev,
                                            now,
                                            Ordering::SeqCst,
                                            Ordering::SeqCst,
                                        )
                                        .is_ok()
                                    {
                                        break;
                                    }
                                }
                            }

                            if delayed_paths.contains(&path) {
                                thread::sleep(Duration::from_millis(delay_ms));
                            }

                            let (status, body) = match routes.get(&path) {
                                Some(bytes) => ("200 OK", bytes.clone()),
                                None => ("404 Not Found", b"not found".to_vec()),
                            };

                            let response = format!(
                                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                status,
                                body.len()
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.write_all(&body);
                            let _ = stream.flush();

                            if is_ttf {
                                inflight.fetch_sub(1, Ordering::SeqCst);
                            }
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            base_url,
            hits,
            inflight_ttf,
            max_inflight_ttf,
            stop,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn hit_count(&self, path: &str) -> usize {
        self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }

    fn max_inflight_ttf(&self) -> usize {
        self.max_inflight_ttf.load(Ordering::SeqCst)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let _ = self.inflight_ttf.load(Ordering::Relaxed);
    }
}

fn roboto_metadata_json(base_url: &str) -> Vec<u8> {
    serde_json::json!({
        "id": "roboto",
        "family": "Roboto",
        "subsets": ["latin", "latin-ext"],
        "weights": [400, 700],
        "styles": ["normal", "italic"],
        "version": "v1",
        "type": "google",
        "variants": {
            "400": {
                "normal": {
                    "latin": {
                        "url": {
                            "ttf": format!("{}/fonts/latin-400-normal.ttf", base_url),
                            "woff2": null,
                            "woff": null
                        }
                    },
                    "latin-ext": {
                        "url": {
                            "ttf": format!("{}/fonts/latin-ext-400-normal.ttf", base_url),
                            "woff2": null,
                            "woff": null
                        }
                    }
                },
                "italic": {
                    "latin": {
                        "url": {
                            "ttf": null,
                            "woff2": format!("{}/fonts/latin-400-italic.woff2", base_url),
                            "woff": null
                        }
                    }
                }
            },
            "700": {
                "normal": {
                    "latin": {
                        "url": {
                            "ttf": format!("{}/fonts/latin-700-normal.ttf", base_url),
                            "woff2": null,
                            "woff": null
                        }
                    }
                }
            }
        }
    })
    .to_string()
    .into_bytes()
}

fn single_file_metadata_json(base_url: &str, id: &str, family: &str, file_path: &str) -> Vec<u8> {
    serde_json::json!({
        "id": id,
        "family": family,
        "subsets": ["latin"],
        "weights": [400],
        "styles": ["normal"],
        "version": "v1",
        "type": "google",
        "variants": {
            "400": {
                "normal": {
                    "latin": {
                        "url": {
                            "ttf": format!("{}{}", base_url, file_path),
                            "woff2": null,
                            "woff": null
                        }
                    }
                }
            }
        }
    })
    .to_string()
    .into_bytes()
}

fn build_roboto_routes(base_url: &str) -> HashMap<String, Vec<u8>> {
    let mut routes = HashMap::new();
    routes.insert(
        "/v1/fonts/roboto".to_string(),
        roboto_metadata_json(base_url),
    );
    routes.insert(
        "/fonts/latin-400-normal.ttf".to_string(),
        REGULAR_TTF.to_vec(),
    );
    routes.insert(
        "/fonts/latin-ext-400-normal.ttf".to_string(),
        BOLD_TTF.to_vec(),
    );
    routes.insert(
        "/fonts/latin-700-normal.ttf".to_string(),
        ITALIC_TTF.to_vec(),
    );
    routes
}

fn make_client(
    metadata_dir: &Path,
    blob_dir: &Path,
    base_url: &str,
    max_parallel_downloads: usize,
    max_blob_cache_bytes: u64,
) -> FontsourceClient {
    let config = ClientConfig {
        metadata_cache_dir: metadata_dir.to_path_buf(),
        blob_cache_dir: blob_dir.to_path_buf(),
        metadata_base_url: format!("{}/v1/fonts", base_url),
        max_parallel_downloads,
        max_blob_cache_bytes,
        ..ClientConfig::default()
    };
    FontsourceClient::new(config).unwrap()
}

/// Helper: create a client using subdirectories of a single temp path.
fn make_client_from_temp(
    temp: &Path,
    base_url: &str,
    max_parallel_downloads: usize,
    max_blob_cache_bytes: u64,
) -> FontsourceClient {
    make_client(
        &temp.join("metadata"),
        &temp.join("blobs"),
        base_url,
        max_parallel_downloads,
        max_blob_cache_bytes,
    )
}

#[test]
fn test_empty_variants_returns_error_blocking() {
    let server = TestServer::new(|_| HashMap::new(), HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let err = client.load_blocking("Roboto", Some(&[])).unwrap_err();
    assert!(matches!(err, FontsourceFontdbError::NoVariantsRequested));
}

#[test]
fn test_variants_not_available_error_blocking() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 900,
        style: FontStyle::Italic,
    }];

    let err = client
        .load_blocking("Roboto", Some(&requested))
        .unwrap_err();

    assert!(matches!(
        err,
        FontsourceFontdbError::VariantsNotAvailable { .. }
    ));
}

#[test]
fn test_none_variants_loads_all_downloadable_ttf() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let batch = client.load_blocking("Roboto", None).unwrap();

    assert_eq!(batch.ttf_file_count, 3);
    assert_eq!(batch.loaded_variants.len(), 2);
    assert!(batch.loaded_variants.contains(&VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }));
    assert!(batch.loaded_variants.contains(&VariantRequest {
        weight: 700,
        style: FontStyle::Normal,
    }));
    assert_eq!(batch.sources().len(), 3);
    assert!(batch
        .sources()
        .iter()
        .all(|source| matches!(source, fontdb::Source::Binary(_))));
}

#[test]
fn test_register_batch_returns_ids_and_per_source_ids() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let mut db = fontdb::Database::new();
    let registration = db.register_fontsource_batch(batch);
    assert!(!registration.face_ids().is_empty());
    assert_eq!(registration.per_source_ids().len(), 2);
    assert!(registration
        .per_source_ids()
        .iter()
        .all(|ids| !ids.is_empty()));
}

#[test]
fn test_append_only_duplicate_register_returns_distinct_ids() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let first_batch = client.load_blocking("Roboto", Some(&requested)).unwrap();
    let second_batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let mut db = fontdb::Database::new();
    let first = db.register_fontsource_batch(first_batch);
    let second = db.register_fontsource_batch(second_batch);

    let first_ids: std::collections::HashSet<_> = first.face_ids().iter().copied().collect();
    let second_ids: std::collections::HashSet<_> = second.face_ids().iter().copied().collect();
    assert!(!first_ids.is_empty());
    assert!(!second_ids.is_empty());
    assert!(first_ids.is_disjoint(&second_ids));
}

#[tokio::test]
async fn test_async_and_blocking_parity() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp_async = tempfile::tempdir().unwrap();
    let temp_blocking = tempfile::tempdir().unwrap();

    let async_client = make_client_from_temp(temp_async.path(), server.base_url(), 8, u64::MAX);
    let blocking_client =
        make_client_from_temp(temp_blocking.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let async_result = async_client.load("Roboto", Some(&requested)).await.unwrap();
    let requested_vec = requested.to_vec();
    let blocking_result = tokio::task::spawn_blocking(move || {
        blocking_client.load_blocking("Roboto", Some(&requested_vec))
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(async_result.font_id, blocking_result.font_id);
    assert_eq!(
        async_result.loaded_variants,
        blocking_result.loaded_variants
    );
    assert_eq!(async_result.ttf_file_count, blocking_result.ttf_file_count);
    assert_eq!(async_result.ttf_file_count, 2);
    assert_eq!(
        async_result.sources().len(),
        blocking_result.sources().len()
    );
    assert!(!async_result.sources().is_empty());
    assert!(!blocking_result.sources().is_empty());
}

#[test]
fn test_cache_hit_avoids_network() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let _first = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let latin_hits = server.hit_count("/fonts/latin-400-normal.ttf");
    let latin_ext_hits = server.hit_count("/fonts/latin-ext-400-normal.ttf");

    let _second = client.load_blocking("Roboto", Some(&requested)).unwrap();

    assert_eq!(server.hit_count("/fonts/latin-400-normal.ttf"), latin_hits);
    assert_eq!(
        server.hit_count("/fonts/latin-ext-400-normal.ttf"),
        latin_ext_hits
    );
}

#[tokio::test]
async fn test_in_process_dedupe_same_file_concurrent_loads() {
    let delayed = HashSet::from([
        "/fonts/latin-400-normal.ttf".to_string(),
        "/fonts/latin-ext-400-normal.ttf".to_string(),
    ]);

    let server = TestServer::new(build_roboto_routes, delayed, 150);
    let temp = tempfile::tempdir().unwrap();

    let client = Arc::new(make_client_from_temp(
        temp.path(),
        server.base_url(),
        8,
        u64::MAX,
    ));
    let requested = vec![VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let client_a = Arc::clone(&client);
    let req_a = requested.clone();
    let task_a = tokio::spawn(async move {
        client_a.load("Roboto", Some(&req_a)).await.unwrap();
    });

    let client_b = Arc::clone(&client);
    let req_b = requested.clone();
    let task_b = tokio::spawn(async move {
        client_b.load("Roboto", Some(&req_b)).await.unwrap();
    });

    task_a.await.unwrap();
    task_b.await.unwrap();

    assert_eq!(server.hit_count("/fonts/latin-400-normal.ttf"), 1);
    assert_eq!(server.hit_count("/fonts/latin-ext-400-normal.ttf"), 1);
}

#[tokio::test]
async fn test_parallel_download_bounded() {
    let delayed = HashSet::from([
        "/fonts/latin-400-normal.ttf".to_string(),
        "/fonts/latin-ext-400-normal.ttf".to_string(),
        "/fonts/latin-700-normal.ttf".to_string(),
    ]);

    let server = TestServer::new(build_roboto_routes, delayed, 150);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 2, u64::MAX);

    let _ = client.load("Roboto", None).await.unwrap();

    let max = server.max_inflight_ttf();
    assert!(
        max >= 2,
        "expected parallel downloads, max inflight was {max}"
    );
    assert!(
        max <= 2,
        "expected bounded parallel downloads, max inflight was {max}"
    );
}

#[test]
fn test_corrupt_metadata_fallbacks_to_network() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let metadata_dir = temp.path().join("metadata");
    let blob_dir = temp.path().join("blobs");
    std::fs::create_dir_all(&metadata_dir).unwrap();
    // Write corrupt metadata file in the new flat layout
    std::fs::write(metadata_dir.join("roboto.json"), b"{bad json").unwrap();

    let client = make_client(&metadata_dir, &blob_dir, server.base_url(), 8, u64::MAX);
    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    assert_eq!(batch.ttf_file_count, 2);
    assert!(server.hit_count("/v1/fonts/roboto") >= 1);
    assert!(server.hit_count("/fonts/latin-400-normal.ttf") >= 1);
}

#[test]
fn test_unregister_batch_removes_faces_and_is_idempotent() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let mut db = fontdb::Database::new();
    let registration = db.register_fontsource_batch(batch);
    assert!(!registration.face_ids().is_empty());

    for id in registration.face_ids() {
        assert!(db.face(*id).is_some());
    }

    let second_unregister = registration.clone();
    db.unregister_fontsource_batch(registration);

    for id in second_unregister.face_ids() {
        assert!(db.face(*id).is_none());
    }

    db.unregister_fontsource_batch(second_unregister);

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();
    let partial = db.register_fontsource_batch(batch);
    if let Some(first_id) = partial.face_ids().first().copied() {
        db.remove_face(first_id);
    }
    db.unregister_fontsource_batch(partial);
}

#[test]
fn test_eviction_keeps_current_font() {
    let server = TestServer::new(
        |base| {
            let mut routes = HashMap::new();
            routes.insert(
                "/v1/fonts/roboto".to_string(),
                single_file_metadata_json(base, "roboto", "Roboto", "/fonts/roboto.ttf"),
            );
            routes.insert(
                "/v1/fonts/open-sans".to_string(),
                single_file_metadata_json(base, "open-sans", "Open Sans", "/fonts/open-sans.ttf"),
            );
            routes.insert("/fonts/roboto.ttf".to_string(), REGULAR_TTF.to_vec());
            routes.insert("/fonts/open-sans.ttf".to_string(), BOLD_TTF.to_vec());
            routes
        },
        HashSet::new(),
        0,
    );

    let temp = tempfile::tempdir().unwrap();
    let metadata_dir = temp.path().join("metadata");
    let blob_dir = temp.path().join("blobs");
    let max_cache = REGULAR_TTF.len() as u64 + 16;
    let client = make_client(&metadata_dir, &blob_dir, server.base_url(), 8, max_cache);

    let _ = client.load_blocking("Roboto", None).unwrap();

    thread::sleep(Duration::from_millis(20));

    let _ = client.load_blocking("Open Sans", None).unwrap();

    // Roboto blob should be evicted, Open Sans blob should remain.
    // Blob filenames: {font_id}--{subset}-{weight}-{style}.ttf
    assert!(
        !blob_dir.join("roboto--latin-400-normal.ttf").exists(),
        "roboto blob should have been evicted"
    );
    assert!(
        blob_dir.join("open-sans--latin-400-normal.ttf").exists(),
        "open-sans blob should still exist"
    );
}

#[test]
fn test_cached_metadata_avoids_refetch() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client_from_temp(temp.path(), server.base_url(), 8, u64::MAX);

    let _ = client.load_blocking("Roboto", None).unwrap();

    let first_meta_hits = server.hit_count("/v1/fonts/roboto");
    assert_eq!(first_meta_hits, 1);

    let _ = client.load_blocking("Roboto", None).unwrap();

    assert_eq!(
        server.hit_count("/v1/fonts/roboto"),
        first_meta_hits,
        "metadata should not be re-fetched on second load"
    );
}

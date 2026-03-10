use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use vl_convert_google_fonts::{
    ClientConfig, FontStyle, GoogleFontsClient, GoogleFontsError, VariantRequest,
};

#[cfg(feature = "fontdb")]
use vl_convert_google_fonts::GoogleFontsDatabaseExt;

const REGULAR_TTF: &[u8] =
    include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Regular.ttf");
const BOLD_TTF: &[u8] = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Bold.ttf");
const ITALIC_TTF: &[u8] =
    include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-RegularItalic.ttf");

/// Routes for the test server: exact path matches + CSS2 family-based matches.
struct Routes {
    exact: HashMap<String, Vec<u8>>,
    /// Maps lowercase font-id to CSS2 response bytes. The server matches any
    /// `/css2?family=FAMILY:...` path and returns the CSS for that family.
    css2_families: HashMap<String, Vec<u8>>,
}

impl Routes {
    fn resolve(&self, path: &str) -> Option<&Vec<u8>> {
        // Exact match first (for TTF files)
        if let Some(body) = self.exact.get(path) {
            return Some(body);
        }
        // CSS2 prefix match: extract family name, normalize to font-id
        if let Some(rest) = path.strip_prefix("/css2?family=") {
            let family_raw = rest.split(':').next().unwrap_or("");
            let family_id = family_raw
                .replace("%20", "-")
                .replace('+', "-")
                .replace(' ', "-")
                .to_lowercase();
            return self.css2_families.get(&family_id);
        }
        None
    }
}

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
        F: FnOnce(&str) -> Routes,
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

                            let (status, body) = match routes.resolve(&path) {
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

    /// Count all CSS2 hits for any request containing the given family name.
    fn css2_hit_count(&self, family: &str) -> usize {
        let prefix = format!("/css2?family={}:", family);
        // Also check for is_known_font probe format: /css2?family={family}:wght@400
        let probe_prefix = format!("/css2?family={}:", family.to_lowercase());
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix) || k.starts_with(&probe_prefix))
            .map(|(_, v)| *v)
            .sum()
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

/// Build a CSS2 @font-face response for mock variants.
fn build_css2_response(
    base_url: &str,
    family: &str,
    variants: &[(u16, FontStyle, &str)],
) -> Vec<u8> {
    let font_slug = family.to_lowercase().replace(' ', "");
    let mut css = String::new();
    for (weight, style, filename) in variants {
        let style_str = match style {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
        };
        css.push_str(&format!(
            "@font-face {{\n  font-family: '{family}';\n  font-style: {style_str};\n  font-weight: {weight};\n  src: url({base_url}/s/{font_slug}/v30/{filename}) format('truetype');\n}}\n"
        ));
    }
    css.into_bytes()
}

fn build_roboto_routes(base_url: &str) -> Routes {
    let css = build_css2_response(
        base_url,
        "Roboto",
        &[
            (400, FontStyle::Normal, "regular.ttf"),
            (700, FontStyle::Normal, "bold.ttf"),
            (400, FontStyle::Italic, "italic.ttf"),
        ],
    );

    let mut css2_families = HashMap::new();
    css2_families.insert("roboto".to_string(), css);

    let mut exact = HashMap::new();
    exact.insert(
        "/s/roboto/v30/regular.ttf".to_string(),
        REGULAR_TTF.to_vec(),
    );
    exact.insert("/s/roboto/v30/bold.ttf".to_string(), BOLD_TTF.to_vec());
    exact.insert("/s/roboto/v30/italic.ttf".to_string(), ITALIC_TTF.to_vec());

    Routes {
        exact,
        css2_families,
    }
}

fn make_client(
    cache_root: &Path,
    base_url: &str,
    max_parallel_downloads: usize,
    max_blob_cache_bytes: u64,
) -> GoogleFontsClient {
    let config = ClientConfig {
        cache_dir: Some(cache_root.to_path_buf()),
        css2_base_url: format!("{}/css2", base_url),
        max_parallel_downloads,
        max_blob_cache_bytes,
        ..ClientConfig::default()
    };
    GoogleFontsClient::new(config).unwrap()
}

fn make_cacheless_client(base_url: &str) -> GoogleFontsClient {
    let config = ClientConfig {
        cache_dir: None,
        css2_base_url: format!("{}/css2", base_url),
        ..ClientConfig::default()
    };
    GoogleFontsClient::new(config).unwrap()
}

#[test]
fn test_empty_variants_returns_error_blocking() {
    let server = TestServer::new(
        |_| Routes {
            exact: HashMap::new(),
            css2_families: HashMap::new(),
        },
        HashSet::new(),
        0,
    );
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let err = client.load_blocking("Roboto", Some(&[])).unwrap_err();
    assert!(matches!(err, GoogleFontsError::NoVariantsRequested));
}

#[test]
fn test_variants_not_available_error_blocking() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 900,
        style: FontStyle::Italic,
    }];

    let err = client
        .load_blocking("Roboto", Some(&requested))
        .unwrap_err();

    assert!(matches!(err, GoogleFontsError::VariantsNotAvailable { .. }));
}

#[test]
fn test_none_variants_loads_all_available() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let batch = client.load_blocking("Roboto", None).unwrap();

    // CSS2 returns 3 @font-face blocks: 400 normal, 700 normal, 400 italic
    // Each maps 1:1 to a TTF file (no subset dimension)
    assert_eq!(batch.ttf_file_count, 3);
    assert_eq!(batch.loaded_variants.len(), 3);
    assert!(batch.loaded_variants.contains(&VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }));
    assert!(batch.loaded_variants.contains(&VariantRequest {
        weight: 700,
        style: FontStyle::Normal,
    }));
    assert!(batch.loaded_variants.contains(&VariantRequest {
        weight: 400,
        style: FontStyle::Italic,
    }));
    assert_eq!(batch.font_data.len(), 3);
    assert!(batch.font_data.iter().all(|data| !data.is_empty()));
}

#[cfg(feature = "fontdb")]
#[test]
fn test_register_batch_returns_ids_and_per_source_ids() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let mut db = fontdb::Database::new();
    let registration = db.register_google_fonts_batch(batch);
    assert!(!registration.face_ids().is_empty());
    assert_eq!(registration.per_source_ids().len(), 1);
    assert!(registration
        .per_source_ids()
        .iter()
        .all(|ids| !ids.is_empty()));
}

#[cfg(feature = "fontdb")]
#[test]
fn test_append_only_duplicate_register_returns_distinct_ids() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let first_batch = client.load_blocking("Roboto", Some(&requested)).unwrap();
    let second_batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let mut db = fontdb::Database::new();
    let first = db.register_google_fonts_batch(first_batch);
    let second = db.register_google_fonts_batch(second_batch);

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

    let async_client = make_client(temp_async.path(), server.base_url(), 8, u64::MAX);
    let blocking_client = make_client(temp_blocking.path(), server.base_url(), 8, u64::MAX);

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
    // Single weight 400 normal → 1 TTF file (1:1 mapping, no subsets)
    assert_eq!(async_result.ttf_file_count, 1);
    assert_eq!(
        async_result.font_data.len(),
        blocking_result.font_data.len()
    );
    assert!(!async_result.font_data.is_empty());
    assert!(!blocking_result.font_data.is_empty());
}

#[test]
fn test_cache_hit_avoids_ttf_refetch() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let _first = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let regular_hits = server.hit_count("/s/roboto/v30/regular.ttf");

    let _second = client.load_blocking("Roboto", Some(&requested)).unwrap();

    // TTF blobs should be served from cache, not re-downloaded
    assert_eq!(
        server.hit_count("/s/roboto/v30/regular.ttf"),
        regular_hits,
        "TTF blob should not be re-fetched from cache"
    );
}

#[test]
fn test_css2_always_refetched() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let _first = client.load_blocking("Roboto", None).unwrap();
    let first_css2_hits = server.css2_hit_count("Roboto");
    assert_eq!(first_css2_hits, 1);

    let _second = client.load_blocking("Roboto", None).unwrap();
    let second_css2_hits = server.css2_hit_count("Roboto");
    assert_eq!(
        second_css2_hits, 2,
        "CSS2 should be re-fetched on every load (no CSS caching)"
    );
}

#[tokio::test]
async fn test_in_process_dedupe_same_file_concurrent_loads() {
    let delayed = HashSet::from(["/s/roboto/v30/regular.ttf".to_string()]);

    let server = TestServer::new(build_roboto_routes, delayed, 150);
    let temp = tempfile::tempdir().unwrap();

    let client = Arc::new(make_client(temp.path(), server.base_url(), 8, u64::MAX));
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

    // Download gate should ensure only one download of the TTF file
    assert_eq!(server.hit_count("/s/roboto/v30/regular.ttf"), 1);
}

#[tokio::test]
async fn test_parallel_download_bounded() {
    let delayed = HashSet::from([
        "/s/roboto/v30/regular.ttf".to_string(),
        "/s/roboto/v30/bold.ttf".to_string(),
        "/s/roboto/v30/italic.ttf".to_string(),
    ]);

    let server = TestServer::new(build_roboto_routes, delayed, 150);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 2, u64::MAX);

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

#[cfg(feature = "fontdb")]
#[test]
fn test_unregister_batch_removes_faces_and_is_idempotent() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let mut db = fontdb::Database::new();
    let registration = db.register_google_fonts_batch(batch);
    assert!(!registration.face_ids().is_empty());

    for id in registration.face_ids() {
        assert!(db.face(*id).is_some());
    }

    let second_unregister = registration.clone();
    db.unregister_google_fonts_batch(registration);

    for id in second_unregister.face_ids() {
        assert!(db.face(*id).is_none());
    }

    db.unregister_google_fonts_batch(second_unregister);

    let batch = client.load_blocking("Roboto", Some(&requested)).unwrap();
    let partial = db.register_google_fonts_batch(batch);
    if let Some(first_id) = partial.face_ids().first().copied() {
        db.remove_face(first_id);
    }
    db.unregister_google_fonts_batch(partial);
}

#[test]
fn test_eviction_keeps_current_font() {
    let server = TestServer::new(
        |base| {
            let roboto_css =
                build_css2_response(base, "Roboto", &[(400, FontStyle::Normal, "r400.ttf")]);
            let lato_css =
                build_css2_response(base, "Lato", &[(400, FontStyle::Normal, "l400.ttf")]);

            let mut css2_families = HashMap::new();
            css2_families.insert("roboto".to_string(), roboto_css);
            css2_families.insert("lato".to_string(), lato_css);

            let mut exact = HashMap::new();
            exact.insert("/s/roboto/v30/r400.ttf".to_string(), REGULAR_TTF.to_vec());
            exact.insert("/s/lato/v30/l400.ttf".to_string(), BOLD_TTF.to_vec());

            Routes {
                exact,
                css2_families,
            }
        },
        HashSet::new(),
        0,
    );

    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path();
    let blob_dir = cache_dir.join("blobs");
    let max_cache = REGULAR_TTF.len() as u64 + 16;
    let client = make_client(cache_dir, server.base_url(), 8, max_cache);

    let _ = client.load_blocking("Roboto", None).unwrap();

    thread::sleep(Duration::from_millis(20));

    let _ = client.load_blocking("Lato", None).unwrap();

    let roboto_hits = server.hit_count("/s/roboto/v30/r400.ttf");
    let lato_hits = server.hit_count("/s/lato/v30/l400.ttf");
    assert_eq!(roboto_hits, 1);
    assert_eq!(lato_hits, 1);

    // Current font blobs should be exempt from eviction.
    let _ = client.load_blocking("Lato", None).unwrap();
    assert_eq!(server.hit_count("/s/lato/v30/l400.ttf"), lato_hits);

    // Earlier font should have been evicted under the tight size budget.
    let _ = client.load_blocking("Roboto", None).unwrap();
    assert_eq!(server.hit_count("/s/roboto/v30/r400.ttf"), roboto_hits + 1);

    // At least one blob should exist for the currently-loaded font.
    let blob_count = std::fs::read_dir(&blob_dir)
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
        })
        .count();
    assert!(blob_count >= 1);
}

#[test]
fn test_corrupt_blob_fallbacks_to_network() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let _ = client.load_blocking("Roboto", Some(&requested)).unwrap();

    let blob_dir = temp.path().join("blobs");
    let mut blobs: Vec<_> = std::fs::read_dir(&blob_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("ttf"))
                .unwrap_or(false)
        })
        .collect();
    blobs.sort();
    assert!(!blobs.is_empty());

    let corrupt_path = blobs[0].clone();
    std::fs::remove_file(&corrupt_path).unwrap();
    // Replace with a directory to make read fail
    std::fs::create_dir_all(&corrupt_path).unwrap();

    let hits_before = server.hit_count("/s/roboto/v30/regular.ttf");
    let _ = client.load_blocking("Roboto", Some(&requested)).unwrap();
    let hits_after = server.hit_count("/s/roboto/v30/regular.ttf");

    assert_eq!(hits_after, hits_before + 1);
    assert!(corrupt_path.is_file());
}

#[test]
fn test_no_cache_dir_always_downloads() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let client = make_cacheless_client(server.base_url());

    let requested = [VariantRequest {
        weight: 400,
        style: FontStyle::Normal,
    }];

    let first = client.load_blocking("Roboto", Some(&requested)).unwrap();
    assert_eq!(first.ttf_file_count, 1);

    let regular_hits = server.hit_count("/s/roboto/v30/regular.ttf");

    // Without a cache dir, every load re-downloads blobs.
    let second = client.load_blocking("Roboto", Some(&requested)).unwrap();
    assert_eq!(second.ttf_file_count, 1);
    assert_eq!(
        server.hit_count("/s/roboto/v30/regular.ttf"),
        regular_hits * 2
    );
}

#[tokio::test]
async fn test_is_known_font_returns_true() {
    let server = TestServer::new(build_roboto_routes, HashSet::new(), 0);
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    assert!(client.is_known_font("Roboto").await.unwrap());
}

#[tokio::test]
async fn test_is_known_font_returns_false_for_unknown() {
    let server = TestServer::new(
        |_| Routes {
            exact: HashMap::new(),
            css2_families: HashMap::new(),
        },
        HashSet::new(),
        0,
    );
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    // Server returns 404 → HttpStatus 404 → is_known_font returns error (not 400)
    // or FontNotFound. Either way, the font is not known.
    let result = client.is_known_font("Nonexistent").await;
    match result {
        Ok(found) => assert!(!found),
        Err(_) => {} // HTTP error for unknown font is also acceptable
    }
}

#[tokio::test]
async fn test_is_known_font_empty_css_response() {
    // Server returns 200 with no @font-face blocks
    let server = TestServer::new(
        |_| {
            let mut css2_families = HashMap::new();
            css2_families.insert("emptyfont".to_string(), b"/* no fonts */".to_vec());
            Routes {
                exact: HashMap::new(),
                css2_families,
            }
        },
        HashSet::new(),
        0,
    );
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let result = client.is_known_font("EmptyFont").await.unwrap();
    assert!(!result);
}

#[test]
fn test_font_not_found_returns_error() {
    let server = TestServer::new(
        |_| Routes {
            exact: HashMap::new(),
            css2_families: HashMap::new(),
        },
        HashSet::new(),
        0,
    );
    let temp = tempfile::tempdir().unwrap();
    let client = make_client(temp.path(), server.base_url(), 8, u64::MAX);

    let err = client.load_blocking("Nonexistent", None).unwrap_err();
    // Server returns 404 for unknown family, client interprets as error
    assert!(
        matches!(
            err,
            GoogleFontsError::FontNotFound(_) | GoogleFontsError::HttpStatus { .. }
        ),
        "Expected FontNotFound or HttpStatus error, got: {:?}",
        err
    );
}

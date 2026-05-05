//! Integration tests for the `/admin/config` surface (Task 13).
//!
//! These tests exercise the full admin config pipeline end-to-end through
//! a running server — GET, PATCH, PUT, DELETE, and POST /admin/config/fonts/
//! directories — asserting:
//!
//! * The natural JSON ↔ Option mapping (null on Option<T> → None, null on
//!   non-nullable → 400).
//! * Serde-level rejections (NonZero zero, missing required fields in PUT).
//! * 422 validation failures (below-minimum heap size).
//! * Hot-apply vs rebuild classification (generation bumps only on rebuild).
//! * Identity short-circuit (PUT of current config → 200, no counter bump).
//! * /infoz never exposes the generation counter.
//! * Font-directory POST dedup + error paths.
//!
//! All servers are started with a free admin port via
//! `common::start_admin_config_server`; they share the ambient library
//! `VlConverter` / font configuration, so the tests avoid touching
//! `set_font_directories` / `apply_hot_font_cache` via the library directly
//! — all state changes flow through the admin surface.

mod common;

use common::*;
use serde_json::{json, Value};
use vl_convert_rs::converter::VlcConfig;

fn default_admin_server() -> BudgetServer {
    start_admin_config_server(VlcConfig::default(), default_serve_config())
}

async fn get_config(server: &BudgetServer) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/admin/config", server.admin_base_url))
        .send()
        .await
        .expect("admin GET /admin/config failed");
    let status = resp.status();
    let body: Value = resp.json().await.expect("response body was not JSON");
    (status, body)
}

async fn patch_config(server: &BudgetServer, body: Value) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .patch(format!("{}/admin/config", server.admin_base_url))
        .json(&body)
        .send()
        .await
        .expect("admin PATCH /admin/config failed");
    let status = resp.status();
    // Non-2xx responses may not have JSON bodies if opaque_errors is set; try
    // to decode and fall through to Null if it fails.
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn put_config(server: &BudgetServer, body: Value) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .put(format!("{}/admin/config", server.admin_base_url))
        .json(&body)
        .send()
        .await
        .expect("admin PUT /admin/config failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn delete_config(server: &BudgetServer) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{}/admin/config", server.admin_base_url))
        .send()
        .await
        .expect("admin DELETE /admin/config failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn post_font_dir(server: &BudgetServer, body: Value) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/admin/config/fonts/directories",
            server.admin_base_url
        ))
        .json(&body)
        .send()
        .await
        .expect("admin POST /admin/config/fonts/directories failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn get_font_dirs(server: &BudgetServer) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "{}/admin/config/fonts/directories",
            server.admin_base_url
        ))
        .send()
        .await
        .expect("admin GET /admin/config/fonts/directories failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn put_font_dirs(server: &BudgetServer, body: Value) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .put(format!(
            "{}/admin/config/fonts/directories",
            server.admin_base_url
        ))
        .json(&body)
        .send()
        .await
        .expect("admin PUT /admin/config/fonts/directories failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn get_font_cache_size(server: &BudgetServer) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "{}/admin/config/fonts/cache_size",
            server.admin_base_url
        ))
        .send()
        .await
        .expect("admin GET /admin/config/fonts/cache_size failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

async fn put_font_cache_size(server: &BudgetServer, body: Value) -> (reqwest::StatusCode, Value) {
    let client = reqwest::Client::new();
    let resp = client
        .put(format!(
            "{}/admin/config/fonts/cache_size",
            server.admin_base_url
        ))
        .json(&body)
        .send()
        .await
        .expect("admin PUT /admin/config/fonts/cache_size failed");
    let status = resp.status();
    let body = resp.json::<Value>().await.unwrap_or_else(|_| Value::Null);
    (status, body)
}

// ---------- GET ----------

#[tokio::test]
async fn test_admin_config_get_baseline_and_live() {
    let server = default_admin_server();
    let (status, body) = get_config(&server).await;
    assert_eq!(status, 200);

    // Shape: baseline + effective + generation.
    assert!(body.get("baseline").is_some(), "missing baseline key");
    assert!(body.get("effective").is_some(), "missing effective key");
    assert_eq!(body["generation"], 0);

    // At startup, baseline == effective for a default config.
    assert_eq!(body["baseline"], body["effective"]);

    // A field from the view shape must be present (sanity): num_workers is
    // always a positive integer in the VlcConfigView output.
    assert!(body["effective"]["num_workers"].is_number());
}

// ---------- PATCH happy path ----------

#[tokio::test]
async fn test_admin_config_patch_default_theme_applies_and_rerenders() {
    let server = default_admin_server();

    // Confirm baseline has no default_theme set.
    let (_, before) = get_config(&server).await;
    assert!(before["effective"]["default_theme"].is_null());

    // PATCH to "dark" — this is a rebuild-required field per the design's
    // §4 classification.
    let (status, body) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(status, 200, "body: {body:?}");

    // Round-trip: GET reflects the new theme.
    let (_, after) = get_config(&server).await;
    assert_eq!(after["effective"]["default_theme"], "dark");
    assert_eq!(
        after["generation"], 1,
        "generation must bump on rebuild-required field"
    );

    // A conversion still succeeds (new converter is live and serving).
    let resp = server
        .handle
        .client
        .post(format!("{}/vegalite/svg", server.handle.base_url))
        .json(&json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "post-patch conversion should succeed");
}

// ---------- PATCH null semantics ----------

#[tokio::test]
async fn test_admin_config_patch_null_sets_option_fields_to_none() {
    // PATCH null on an Option<T> field clears it; null on a
    // non-nullable field is a 400 (covered by the next test).
    let server = default_admin_server();

    let (s, _) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(s, 200);

    let (s, _) = patch_config(&server, json!({"default_theme": Value::Null})).await;
    assert_eq!(s, 200);

    let (_, body) = get_config(&server).await;
    assert!(body["effective"]["default_theme"].is_null());
}

#[tokio::test]
async fn test_admin_config_patch_null_on_non_nullable_field_400() {
    // Design §2.5 / §2.5.1: null on a non-nullable field (Vec<T>, bool, etc.)
    // must be rejected at serde parse time with 400.
    let server = default_admin_server();

    // allowed_base_urls: Vec<String> — non-nullable.
    let (status, _) = patch_config(&server, json!({"allowed_base_urls": Value::Null})).await;
    assert_eq!(status, 400, "null on Vec<String> must be 400 at parse time");

    // auto_google_fonts: bool — non-nullable.
    let (status, _) = patch_config(&server, json!({"auto_google_fonts": Value::Null})).await;
    assert_eq!(status, 400, "null on bool must be 400 at parse time");

    // num_workers: NonZeroUsize — non-nullable.
    let (status, _) = patch_config(&server, json!({"num_workers": Value::Null})).await;
    assert_eq!(status, 400, "null on NonZeroUsize must be 400");
}

// ---------- PATCH validation (serde + normalize) ----------

#[tokio::test]
async fn test_admin_config_patch_zero_on_nonzero_type_rejected_400() {
    // NonZeroUsize / NonZeroU64 reject 0 at parse time — serde returns
    // 400, not 422 (which is for post-parse validation).
    let server = default_admin_server();

    let (status, _) = patch_config(&server, json!({"num_workers": 0})).await;
    assert_eq!(status, 400, "num_workers=0 must be rejected at parse");

    let (status, _) = patch_config(&server, json!({"max_v8_heap_size_mb": 0})).await;
    assert_eq!(
        status, 400,
        "max_v8_heap_size_mb=0 must be rejected at parse"
    );
}

#[tokio::test]
async fn test_admin_config_patch_invalid_value_422() {
    // max_v8_heap_size_mb: 3 is below MIN_V8_HEAP_SIZE_MB, which
    // normalize_converter_config rejects after serde parse succeeds.
    // Must be 422, not 400.
    let server = default_admin_server();

    let (status, body) = patch_config(&server, json!({"max_v8_heap_size_mb": 3})).await;
    assert_eq!(
        status, 422,
        "below-minimum heap size must be 422; body: {body:?}"
    );
    // Body should be a ConfigValidationError.
    assert!(body.get("error").is_some() || body.get("field_errors").is_some());
}

#[tokio::test]
async fn test_admin_config_patch_rejects_google_fonts_cache_size_mb() {
    // `google_fonts_cache_size_mb` is no longer a writable VlcConfig
    // field — it's process-global, mutated via
    // `PUT /admin/config/fonts/cache_size`.
    let server = default_admin_server();
    let (status, _) = patch_config(&server, json!({"google_fonts_cache_size_mb": 64})).await;
    assert_eq!(
        status, 400,
        "google_fonts_cache_size_mb is not a writable VlcConfig field; PATCH must 400"
    );
}

#[tokio::test]
async fn test_admin_config_patch_rejects_font_directories() {
    // `font_directories` is no longer part of the writable `VlcConfig`
    // DTO. PATCH /admin/config must reject it via deny_unknown_fields.
    let tmp = tempfile::tempdir().unwrap();
    let server = default_admin_server();

    let path = tmp.path().to_string_lossy().to_string();
    let (status, _) = patch_config(&server, json!({"font_directories": [path]})).await;
    assert_eq!(
        status, 400,
        "font_directories is not a writable VlcConfig field; PATCH must 400"
    );
}

// ---------- PUT full replacement ----------

/// Helper — produce a JSON body that fully replaces every VlcConfig field
/// with the library defaults (`VlcConfig::default()`). Used by identity-PUT
/// tests where the body must match what `GET /admin/config` returns at
/// startup against a server initialised with `VlcConfig::default()`.
fn default_config_put_body() -> Value {
    json!({
        "num_workers": 1,
        "base_url": true,
        "allowed_base_urls": ["http:", "https:"],
        "auto_google_fonts": false,
        "embed_local_fonts": false,
        "subset_fonts": true,
        "missing_fonts": "fallback",
        "google_fonts": [],
        "max_v8_heap_size_mb": Value::Null,
        "max_v8_execution_time_secs": Value::Null,
        "gc_after_conversion": false,
        "vega_plugins": [],
        "plugin_import_domains": [],
        "allow_per_request_plugins": false,
        "max_ephemeral_workers": 2,
        "allow_google_fonts": false,
        "per_request_plugin_import_domains": [],
        "default_theme": Value::Null,
        "default_format_locale": Value::Null,
        "default_time_format_locale": Value::Null,
        "themes": {},
    })
}

#[tokio::test]
async fn test_admin_config_put_full_replacement() {
    let server = default_admin_server();

    // PUT the default body with one divergent field — default_theme.
    let mut body = default_config_put_body();
    body["default_theme"] = Value::String("dark".to_string());

    let (status, resp) = put_config(&server, body).await;
    assert_eq!(status, 200, "body: {resp:?}");

    let (_, after) = get_config(&server).await;
    assert_eq!(after["effective"]["default_theme"], "dark");
    assert_eq!(
        after["generation"], 1,
        "PUT with rebuild field must bump generation"
    );
}

#[tokio::test]
async fn test_admin_config_put_identity_short_circuit() {
    let server = default_admin_server();

    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();

    // PUT the current (default) config.
    let body = default_config_put_body();
    let (status, _) = put_config(&server, body).await;
    assert_eq!(status, 200);

    let (_, after) = get_config(&server).await;
    assert_eq!(
        after["generation"], gen_before,
        "identity PUT must not bump generation"
    );
}

#[tokio::test]
async fn test_admin_config_put_missing_field_422() {
    let server = default_admin_server();

    // Omit num_workers (and others). ConfigReplace has no `#[serde(default)]`,
    // so missing-field is rejected by serde at parse time — which is a 400
    // per the server's json_rejection_response. (Task prompt mentioned 422 in
    // one place and 400 in another; the implementation funnels through
    // JsonRejection -> 400. We assert the observed contract.)
    let body = json!({"num_workers": 1}); // incomplete
    let (status, _) = put_config(&server, body).await;
    assert!(
        status == 400 || status == 422,
        "missing PUT field must be 400 (serde) or 422; got {status}"
    );
}

// ---------- DELETE ----------

#[tokio::test]
async fn test_admin_config_delete_resets_to_baseline() {
    let server = default_admin_server();

    // Mutate effective config.
    let (s, _) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(s, 200);

    let (_, after_patch) = get_config(&server).await;
    assert_eq!(after_patch["effective"]["default_theme"], "dark");
    let gen_after_patch = after_patch["generation"].as_u64().unwrap();

    // DELETE resets.
    let (s, _) = delete_config(&server).await;
    assert_eq!(s, 200);

    let (_, after_delete) = get_config(&server).await;
    assert_eq!(after_delete["effective"], after_delete["baseline"]);
    assert!(after_delete["effective"]["default_theme"].is_null());
    // Rebuild-path (since default_theme was non-hot-apply), so generation
    // bumps on DELETE as well.
    assert_eq!(
        after_delete["generation"],
        gen_after_patch + 1,
        "DELETE must bump generation when config differs"
    );
}

// ---------- Back-to-back PATCHes ----------

#[tokio::test]
async fn test_admin_config_back_to_back_patches_serialize() {
    let server = default_admin_server();

    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();

    // Two sequential PATCHes — both rebuild-required fields.
    let (s, _) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(s, 200);
    let (s, _) = patch_config(&server, json!({"auto_google_fonts": true})).await;
    assert_eq!(s, 200);

    let (_, after) = get_config(&server).await;
    assert_eq!(after["generation"], gen_before + 2);
    assert_eq!(after["effective"]["default_theme"], "dark");
    assert_eq!(after["effective"]["auto_google_fonts"], true);
}

// ---------- /infoz negative assertion ----------

#[tokio::test]
async fn test_admin_config_generation_not_exposed_on_infoz() {
    // Even after a PATCH bumps generation, /infoz body must NOT include a
    // `generation` key. Design §2.8 / §4: generation is admin-only.
    let server = default_admin_server();
    let (s, _) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(s, 200);

    let resp = server
        .handle
        .client
        .get(format!("{}/infoz", server.handle.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();

    assert!(
        body.get("generation").is_none(),
        "/infoz body must not expose generation; got: {body}"
    );
    // Existing infoz surface must still include the established keys.
    assert!(body.get("version").is_some());
    assert!(body.get("vegalite_versions").is_some());
}

// ---------- POST /admin/config/fonts/directories ----------

fn dirs_from_get(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn test_admin_config_font_dir_post_register_and_get() {
    let tmp = tempfile::tempdir().unwrap();
    let server = default_admin_server();

    let path = tmp.path().to_string_lossy().to_string();
    let (status, body) = post_font_dir(&server, json!({"path": path.clone()})).await;
    assert_eq!(status, 200, "body: {body:?}");

    let (_, listing) = get_font_dirs(&server).await;
    assert!(
        dirs_from_get(&listing).contains(&path),
        "posted font dir {path} not in GET listing: {listing:?}"
    );
}

#[tokio::test]
async fn test_admin_config_font_dir_put_replaces() {
    // PUT replaces the global registry wholesale.
    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let server = default_admin_server();

    let path_a = tmp_a.path().to_string_lossy().to_string();
    let path_b = tmp_b.path().to_string_lossy().to_string();

    let (s, _) = put_font_dirs(&server, json!({"paths": [path_a.clone()]})).await;
    assert_eq!(s, 200);
    let (_, after1) = get_font_dirs(&server).await;
    assert_eq!(dirs_from_get(&after1), vec![path_a.clone()]);

    // Replace with just path_b. path_a must be gone.
    let (s, _) = put_font_dirs(&server, json!({"paths": [path_b.clone()]})).await;
    assert_eq!(s, 200);
    let (_, after2) = get_font_dirs(&server).await;
    assert_eq!(dirs_from_get(&after2), vec![path_b]);
}

#[tokio::test]
async fn test_admin_config_font_dir_put_clears_with_empty_list() {
    let tmp = tempfile::tempdir().unwrap();
    let server = default_admin_server();
    let path = tmp.path().to_string_lossy().to_string();

    let (s, _) = put_font_dirs(&server, json!({"paths": [path.clone()]})).await;
    assert_eq!(s, 200);
    let (_, after) = get_font_dirs(&server).await;
    assert_eq!(dirs_from_get(&after), vec![path]);

    let (s, _) = put_font_dirs(&server, json!({"paths": []})).await;
    assert_eq!(s, 200);
    let (_, cleared) = get_font_dirs(&server).await;
    assert_eq!(dirs_from_get(&cleared), Vec::<String>::new());
}

#[tokio::test]
async fn test_admin_config_font_dir_post_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let server = default_admin_server();
    let path = tmp.path().to_string_lossy().to_string();

    let (s, _) = post_font_dir(&server, json!({"path": path.clone()})).await;
    assert_eq!(s, 200);
    let (_, listing_first) = get_font_dirs(&server).await;
    let dirs_first = dirs_from_get(&listing_first);

    // Second POST of the same path — registry idempotent.
    let (s, _) = post_font_dir(&server, json!({"path": path.clone()})).await;
    assert_eq!(s, 200);
    let (_, listing_second) = get_font_dirs(&server).await;
    let dirs_second = dirs_from_get(&listing_second);

    assert_eq!(
        dirs_first, dirs_second,
        "POST of an already-registered path must not duplicate"
    );
}

#[tokio::test]
async fn test_admin_config_font_dir_nonexistent_400() {
    let server = default_admin_server();
    let bogus = "/this/path/does/not/exist/vlc/font_directories";
    let (status, _) = post_font_dir(&server, json!({"path": bogus})).await;
    assert_eq!(status, 400, "POST nonexistent must 400");
    let (status, _) = put_font_dirs(&server, json!({"paths": [bogus]})).await;
    assert_eq!(status, 400, "PUT containing nonexistent path must 400");
}

// ---------- /admin/config/fonts/cache_size ----------

#[tokio::test]
async fn test_admin_config_cache_size_get_returns_resolved_cap() {
    let server = default_admin_server();
    let (status, body) = get_font_cache_size(&server).await;
    assert_eq!(status, 200);
    let mb = body["max_size_mb"]
        .as_u64()
        .expect("max_size_mb is a number");
    assert!(mb > 0, "resolved cap must be positive, got {mb}");
}

#[tokio::test]
async fn test_admin_config_cache_size_put_sets_and_get_reflects() {
    let server = default_admin_server();

    let (status, body) = put_font_cache_size(&server, json!({"max_size_mb": 64})).await;
    assert_eq!(status, 200);
    assert_eq!(body["max_size_mb"], 64);

    let (_, after) = get_font_cache_size(&server).await;
    assert_eq!(after["max_size_mb"], 64);
}

#[tokio::test]
async fn test_admin_config_cache_size_put_null_resets_to_default() {
    let server = default_admin_server();

    // Set to a small non-default value first.
    let (s, _) = put_font_cache_size(&server, json!({"max_size_mb": 32})).await;
    assert_eq!(s, 200);

    // null resets.
    let (status, body) = put_font_cache_size(&server, json!({"max_size_mb": null})).await;
    assert_eq!(status, 200);
    let resolved = body["max_size_mb"].as_u64().unwrap();
    assert_ne!(resolved, 32, "null must reset away from explicit value");
    assert!(resolved > 0);
}

#[tokio::test]
async fn test_admin_config_cache_size_put_rejects_zero() {
    // NonZeroU64 rejects 0 at parse time → 400.
    let server = default_admin_server();
    let (status, _) = put_font_cache_size(&server, json!({"max_size_mb": 0})).await;
    assert_eq!(status, 400);
}

// ---------- Helper — opaque-errors server variant doesn't change core semantics ----------

#[tokio::test]
async fn test_admin_config_patch_opaque_errors_body_shape() {
    // With opaque_errors=true, validation errors still use the correct
    // status code but the body is empty JSON / no detail. Exercised here to
    // confirm opacity is honored on the admin side.
    let mut serve_config = default_serve_config();
    serve_config.opaque_errors = true;
    let server = start_admin_config_server(VlcConfig::default(), serve_config);

    let (status, body) = patch_config(&server, json!({"max_v8_heap_size_mb": 3})).await;
    assert_eq!(status, 422);
    // Either empty object or null — the key assertion is that the status
    // still signals validation failure.
    assert!(
        body == Value::Null || body == json!({}),
        "opaque 422 body should be empty; got {body:?}"
    );
}

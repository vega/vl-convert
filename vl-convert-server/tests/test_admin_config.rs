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
    let body = resp
        .json::<Value>()
        .await
        .unwrap_or_else(|_| Value::Null);
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
    let body = resp
        .json::<Value>()
        .await
        .unwrap_or_else(|_| Value::Null);
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
    let body = resp
        .json::<Value>()
        .await
        .unwrap_or_else(|_| Value::Null);
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
    let body = resp
        .json::<Value>()
        .await
        .unwrap_or_else(|_| Value::Null);
    (status, body)
}

// ---------- GET ----------

#[tokio::test]
async fn test_admin_config_get_baseline_and_live() {
    let server = default_admin_server();
    let (status, body) = get_config(&server).await;
    assert_eq!(status, 200);

    // Shape: baseline + effective + generation + config_version.
    assert!(body.get("baseline").is_some(), "missing baseline key");
    assert!(body.get("effective").is_some(), "missing effective key");
    assert_eq!(body["generation"], 0);
    assert_eq!(body["config_version"], 0);

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
    assert_eq!(after["config_version"], 1);

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
    // Seed a server with default_theme and google_fonts_cache_size_mb set to
    // known non-default values, then PATCH each to null and verify they
    // clear. Design §2.5: null on an Option<T> field = Some(None) = clear.
    let server = default_admin_server();

    // Seed: set default_theme = "dark".
    let (s, _) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(s, 200);

    // Now null it.
    let (s, _) = patch_config(&server, json!({"default_theme": Value::Null})).await;
    assert_eq!(s, 200);

    let (_, body) = get_config(&server).await;
    assert!(body["effective"]["default_theme"].is_null());

    // google_fonts_cache_size_mb — is an Option<NonZeroU64> and is the
    // hot-apply field, so null goes through the hot-apply commit (no
    // generation bump, config_version bumps). Start by setting to a value.
    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();
    let cv_before = before["config_version"].as_u64().unwrap();

    let (s, _) = patch_config(&server, json!({"google_fonts_cache_size_mb": 50})).await;
    assert_eq!(s, 200);

    // PATCH to null clears it.
    let (s, _) = patch_config(
        &server,
        json!({"google_fonts_cache_size_mb": Value::Null}),
    )
    .await;
    assert_eq!(s, 200);

    let (_, after) = get_config(&server).await;
    assert!(after["effective"]["google_fonts_cache_size_mb"].is_null());
    // Hot-apply path — generation unchanged, config_version bumped twice
    // (set then null).
    assert_eq!(after["generation"], gen_before);
    assert_eq!(after["config_version"], cv_before + 2);
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
    assert_eq!(status, 400, "max_v8_heap_size_mb=0 must be rejected at parse");
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

// ---------- PATCH hot-apply ----------

#[tokio::test]
async fn test_admin_config_patch_google_fonts_cache_size_hot_apply() {
    // Hot-apply path — generation unchanged, config_version + 1.
    let server = default_admin_server();

    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();
    let cv_before = before["config_version"].as_u64().unwrap();

    let (status, body) = patch_config(
        &server,
        json!({"google_fonts_cache_size_mb": 64}),
    )
    .await;
    assert_eq!(status, 200, "body: {body:?}");

    let (_, after) = get_config(&server).await;
    assert_eq!(after["effective"]["google_fonts_cache_size_mb"], 64);
    assert_eq!(
        after["generation"], gen_before,
        "hot-apply must not bump generation"
    );
    assert_eq!(after["config_version"], cv_before + 1);
}

#[tokio::test]
async fn test_admin_config_patch_font_directories_replace_hot_apply() {
    // Hot-apply, replace semantics. Create two real tempdirs, set
    // font_directories to one, then to the other — the effective list must
    // match exactly.
    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let server = default_admin_server();

    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();
    let cv_before = before["config_version"].as_u64().unwrap();

    let path_a = tmp_a.path().to_string_lossy().to_string();
    let (status, _) = patch_config(
        &server,
        json!({"font_directories": [path_a.clone()]}),
    )
    .await;
    assert_eq!(status, 200);

    let (_, after1) = get_config(&server).await;
    let dirs: Vec<String> = after1["effective"]["font_directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(dirs, vec![path_a.clone()]);
    assert_eq!(after1["generation"], gen_before, "hot-apply: no gen bump");
    assert_eq!(after1["config_version"], cv_before + 1);

    // Now replace with just path_b. Replace semantics: path_a must be gone.
    let path_b = tmp_b.path().to_string_lossy().to_string();
    let (status, _) = patch_config(
        &server,
        json!({"font_directories": [path_b.clone()]}),
    )
    .await;
    assert_eq!(status, 200);

    let (_, after2) = get_config(&server).await;
    let dirs: Vec<String> = after2["effective"]["font_directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    // Exactly [path_b], nothing else (set_font_directories is authoritative).
    assert_eq!(
        dirs,
        vec![path_b],
        "replace semantics: second PATCH must replace list"
    );
    assert_eq!(after2["generation"], gen_before);
    assert_eq!(after2["config_version"], cv_before + 2);
}

// ---------- PUT full replacement ----------

/// Helper — produce a JSON body that fully replaces every VlcConfig field
/// with the library defaults. Matches ConfigReplace shape.
fn default_config_put_body() -> Value {
    json!({
        "num_workers": 1,
        "base_url": true,
        "allowed_base_urls": [],
        "auto_google_fonts": false,
        "embed_local_fonts": false,
        "subset_fonts": true,
        "missing_fonts": "fallback",
        "google_fonts": [],
        "max_v8_heap_size_mb": 512,
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
        "google_fonts_cache_size_mb": Value::Null,
        "font_directories": [],
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
    assert_eq!(after["config_version"], 1);
}

#[tokio::test]
async fn test_admin_config_put_identity_short_circuit() {
    let server = default_admin_server();

    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();
    let cv_before = before["config_version"].as_u64().unwrap();

    // PUT the current (default) config.
    let body = default_config_put_body();
    let (status, _) = put_config(&server, body).await;
    assert_eq!(status, 200);

    let (_, after) = get_config(&server).await;
    assert_eq!(
        after["generation"], gen_before,
        "identity PUT must not bump generation"
    );
    assert_eq!(
        after["config_version"], cv_before,
        "identity PUT must not bump config_version"
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
    let cv_after_patch = after_patch["config_version"].as_u64().unwrap();

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
    assert_eq!(after_delete["config_version"], cv_after_patch + 1);
}

// ---------- Back-to-back PATCHes ----------

#[tokio::test]
async fn test_admin_config_back_to_back_patches_serialize() {
    let server = default_admin_server();

    let (_, before) = get_config(&server).await;
    let gen_before = before["generation"].as_u64().unwrap();
    let cv_before = before["config_version"].as_u64().unwrap();

    // Two sequential PATCHes — both rebuild-required fields.
    let (s, _) = patch_config(&server, json!({"default_theme": "dark"})).await;
    assert_eq!(s, 200);
    let (s, _) = patch_config(&server, json!({"auto_google_fonts": true})).await;
    assert_eq!(s, 200);

    let (_, after) = get_config(&server).await;
    assert_eq!(after["generation"], gen_before + 2);
    assert_eq!(after["config_version"], cv_before + 2);
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
    assert!(
        body.get("config_version").is_none(),
        "/infoz body must not expose config_version; got: {body}"
    );
    // Existing infoz surface must still include the established keys.
    assert!(body.get("version").is_some());
    assert!(body.get("vegalite_versions").is_some());
}

// ---------- POST /admin/config/fonts/directories ----------

#[tokio::test]
async fn test_admin_config_font_dir_post_register_and_use() {
    let tmp = tempfile::tempdir().unwrap();
    let server = default_admin_server();

    let path = tmp.path().to_string_lossy().to_string();
    let (status, body) = post_font_dir(&server, json!({"path": path.clone()})).await;
    assert_eq!(status, 200, "body: {body:?}");

    // GET shows the directory in effective.font_directories.
    let (_, after) = get_config(&server).await;
    let dirs: Vec<String> = after["effective"]["font_directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        dirs.contains(&path),
        "posted font dir {path} not found in effective list: {dirs:?}"
    );
    // Hot-apply path (POST) — generation unchanged.
    assert_eq!(after["generation"], 0);
}

#[tokio::test]
async fn test_admin_config_font_dir_post_dedup() {
    let tmp = tempfile::tempdir().unwrap();
    let server = default_admin_server();

    let path = tmp.path().to_string_lossy().to_string();
    // First POST.
    let (s, _) = post_font_dir(&server, json!({"path": path.clone()})).await;
    assert_eq!(s, 200);

    let (_, after_first) = get_config(&server).await;
    let cv_after_first = after_first["config_version"].as_u64().unwrap();
    let dirs_first: Vec<String> = after_first["effective"]["font_directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    // Second POST of the same path — must still 200, no duplication.
    let (s, _) = post_font_dir(&server, json!({"path": path.clone()})).await;
    assert_eq!(s, 200, "dedup POST must still 200");

    let (_, after_second) = get_config(&server).await;
    let dirs_second: Vec<String> = after_second["effective"]["font_directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    // The dedup short-circuit should leave config_version unchanged AND the
    // directory list unchanged.
    assert_eq!(
        dirs_first, dirs_second,
        "dedup POST must not duplicate dir in effective list"
    );
    assert_eq!(
        after_second["config_version"].as_u64().unwrap(),
        cv_after_first,
        "dedup POST must not bump config_version"
    );
}

#[tokio::test]
async fn test_admin_config_font_dir_nonexistent_400() {
    let server = default_admin_server();
    let bogus = "/this/path/does/not/exist/vlc/task13";
    let (status, _) = post_font_dir(&server, json!({"path": bogus})).await;
    assert_eq!(
        status, 400,
        "nonexistent font directory path must be 400"
    );
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

use std::num::NonZeroU64;
use vl_convert_rs::converter::{SvgOpts, VgOpts, VlcConfig};
use vl_convert_rs::VlConverter;

/// Verify that exceeding the V8 heap limit returns a specific error
/// rather than aborting the process, that the worker recovers and can
/// process a subsequent conversion, and that memory stats are available
/// before and after the OOM.
#[tokio::test]
async fn test_heap_limit_exceeded_and_recovery() {
    let converter = VlConverter::with_config(VlcConfig {
        max_v8_heap_size_mb: NonZeroU64::new(256),
        ..Default::default()
    })
    .expect("Failed to create converter with small heap");

    // Check heap stats before OOM (also verifies pool auto-spawn)
    let stats_before = converter
        .get_worker_memory_usage()
        .await
        .expect("get_worker_memory_usage should succeed before OOM");
    assert_eq!(stats_before.len(), 1, "should have 1 worker");

    // Trigger OOM with a spec that exceeds the 256 MB heap limit
    let big_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "data": [{
            "name": "big",
            "transform": [
                { "type": "sequence", "start": 0, "stop": 50000000, "as": "x" }
            ]
        }],
        "marks": []
    });

    let result = converter
        .vega_to_svg(big_spec, VgOpts::default(), SvgOpts::default())
        .await;
    let err = result.expect_err("Expected heap limit error, got Ok");
    let msg = err.to_string();
    assert!(
        msg.contains("V8 heap limit exceeded"),
        "Error should mention heap limit, got: {msg}"
    );

    // Worker should still be responsive after OOM
    let stats_after = converter
        .get_worker_memory_usage()
        .await
        .expect("get_worker_memory_usage should succeed after OOM");
    assert_eq!(stats_after.len(), 1, "should still have 1 worker");

    // A normal conversion should succeed, proving recovery
    let small_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "marks": []
    });
    let result = converter
        .vega_to_svg(small_spec, VgOpts::default(), SvgOpts::default())
        .await;
    assert!(
        result.is_ok(),
        "Conversion should succeed after recovery, got: {:?}",
        result.err()
    );
}

/// Verify that the heap limit is properly restored after recovery so
/// the callback fires again on a second OOM.
#[tokio::test]
async fn test_heap_limit_restored_after_recovery() {
    let converter = VlConverter::with_config(VlcConfig {
        max_v8_heap_size_mb: NonZeroU64::new(256),
        ..Default::default()
    })
    .expect("Failed to create converter with small heap");

    let big_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "data": [{
            "name": "big",
            "transform": [
                { "type": "sequence", "start": 0, "stop": 50000000, "as": "x" }
            ]
        }],
        "marks": []
    });

    // First OOM
    let result = converter
        .vega_to_svg(big_spec.clone(), VgOpts::default(), SvgOpts::default())
        .await;
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("V8 heap limit exceeded"), "First OOM: {msg}");

    // Recovery
    let small_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "marks": []
    });
    assert!(converter
        .vega_to_svg(small_spec, VgOpts::default(), SvgOpts::default())
        .await
        .is_ok());

    // Second OOM — proves the limit was restored, not stuck at 2×
    let result = converter
        .vega_to_svg(big_spec, VgOpts::default(), SvgOpts::default())
        .await;
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("V8 heap limit exceeded"),
        "Second OOM should also be caught: {msg}"
    );
}

/// Verify that `max_v8_heap_size_mb = None` (no limit) works: no callback
/// is registered and a normal conversion succeeds.
#[tokio::test]
async fn test_no_heap_limit() {
    let converter = VlConverter::with_config(VlcConfig {
        max_v8_heap_size_mb: None,
        ..Default::default()
    })
    .expect("max_v8_heap_size_mb=None should be valid");

    let spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "marks": []
    });

    let result = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await;
    assert!(
        result.is_ok(),
        "Conversion with no heap limit should succeed: {:?}",
        result.err()
    );
}

/// Verify that max_v8_heap_size_mb below the minimum is rejected
/// at config time, not deferred to first use.
#[test]
fn test_min_heap_size_validation() {
    let result = VlConverter::with_config(VlcConfig {
        max_v8_heap_size_mb: NonZeroU64::new(1),
        ..Default::default()
    });
    let err = result
        .err()
        .expect("max_v8_heap_size_mb=1 should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("too small for V8 to initialize"),
        "Should mention V8 initialization, got: {msg}"
    );
}

/// Verify that exceeding the conversion timeout returns a specific error,
/// the worker recovers, and can process a subsequent conversion.
/// Uses a per-request plugin with an infinite loop to reliably trigger the
/// timeout without hitting V8's default memory limit.
#[tokio::test]
async fn test_conversion_timeout_and_recovery() {
    let converter = VlConverter::with_config(VlcConfig {
        max_v8_execution_time_secs: NonZeroU64::new(2),
        allow_per_request_plugins: true,
        ..Default::default()
    })
    .expect("Failed to create converter with timeout");

    // Plugin registers an expression function that loops forever
    let infinite_plugin = r#"
            export default function(vega) {
                vega.expressionFunction('spin', function() {
                    for (;;) {}
                });
            }
        "#;

    let slow_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "signals": [{
            "name": "s",
            "init": "spin()"
        }],
        "marks": []
    });

    let opts = VgOpts {
        vega_plugin: Some(infinite_plugin.to_string()),
        ..Default::default()
    };
    let result = converter
        .vega_to_svg(slow_spec, opts, SvgOpts::default())
        .await;
    let err = result.expect_err("Expected timeout error, got Ok");
    let msg = err.to_string();
    assert!(
        msg.contains("timed out"),
        "Error should mention timeout, got: {msg}"
    );
    assert!(
        msg.contains("max_v8_execution_time_secs"),
        "Error should mention config parameter, got: {msg}"
    );

    // The ephemeral worker is discarded after the timeout, but the converter
    // itself should still work for normal conversions (on a pool worker).
    let small_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "marks": []
    });
    let result = converter
        .vega_to_svg(small_spec, VgOpts::default(), SvgOpts::default())
        .await;
    assert!(
        result.is_ok(),
        "Conversion should succeed after timeout recovery, got: {:?}",
        result.err()
    );
}

/// Verify that `max_v8_execution_time_secs = None` (no limit) works normally.
#[tokio::test]
async fn test_no_conversion_timeout() {
    let converter = VlConverter::with_config(VlcConfig {
        max_v8_execution_time_secs: None,
        ..Default::default()
    })
    .expect("max_v8_execution_time_secs=None should be valid");

    let spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "marks": []
    });

    let result = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await;
    assert!(
        result.is_ok(),
        "Conversion with no timeout should succeed: {:?}",
        result.err()
    );
}

/// Smoke test that gc_after_conversion=true doesn't crash.
#[tokio::test]
async fn test_gc_after_conversion() {
    let converter = VlConverter::with_config(VlcConfig {
        gc_after_conversion: true,
        ..Default::default()
    })
    .expect("gc_after_conversion config should be valid");

    let spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 10,
        "height": 10,
        "marks": []
    });

    let result = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await;
    assert!(
        result.is_ok(),
        "Conversion with gc_after_conversion should succeed: {:?}",
        result.err()
    );
}

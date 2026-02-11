use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use serde_json::json;
use vl_convert_rs::converter::{VgOpts, VlConverter};
use vl_convert_rs::serde_json::Value;

const LARGE_SCATTER_POINTS: usize = 50_000;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

fn build_large_scatterplot_spec(num_points: usize) -> Value {
    let values: Vec<Value> = (0..num_points)
        .map(|i| {
            let x = (i % 1_000) as f64 + (i / 1_000) as f64 * 0.01;
            let y = ((i * 37) % 1_000) as f64 + ((i * 97) % 100) as f64 * 0.01;
            json!({ "x": x, "y": y })
        })
        .collect();

    json!({
        "$schema": "https://vega.github.io/schema/vega/v6.json",
        "width": 600,
        "height": 400,
        "padding": 5,
        "data": [
            {
                "name": "points",
                "values": values
            }
        ],
        "scales": [
            {
                "name": "x",
                "type": "linear",
                "domain": {"data": "points", "field": "x"},
                "range": "width",
                "zero": false
            },
            {
                "name": "y",
                "type": "linear",
                "domain": {"data": "points", "field": "y"},
                "range": "height",
                "zero": false
            }
        ],
        "axes": [
            {"orient": "bottom", "scale": "x"},
            {"orient": "left", "scale": "y"}
        ],
        "marks": [
            {
                "type": "symbol",
                "from": {"data": "points"},
                "encode": {
                    "enter": {
                        "x": {"scale": "x", "field": "x"},
                        "y": {"scale": "y", "field": "y"},
                        "size": {"value": 16},
                        "fill": {"value": "steelblue"}
                    }
                }
            }
        ]
    })
}

fn bench_vega_to_scenegraph_large_scatter(c: &mut Criterion) {
    let num_points = LARGE_SCATTER_POINTS;
    let vg_spec = build_large_scatterplot_spec(num_points);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to construct benchmark runtime");
    let mut converter = VlConverter::new();

    let warmup = runtime
        .block_on(converter.vega_to_scenegraph(vg_spec.clone(), VgOpts::default()))
        .expect("warmup scenegraph conversion failed");
    assert!(
        warmup.get("scenegraph").is_some(),
        "scenegraph conversion did not return a scenegraph object"
    );
    let warmup_msgpack = runtime
        .block_on(converter.vega_to_scenegraph_msgpack(vg_spec.clone(), VgOpts::default()))
        .expect("warmup scenegraph msgpack conversion failed");
    let warmup_msgpack_decoded: Value =
        rmp_serde::from_slice(&warmup_msgpack).expect("warmup scenegraph msgpack decode failed");
    assert!(
        warmup_msgpack_decoded.get("scenegraph").is_some(),
        "scenegraph msgpack conversion did not return a scenegraph object"
    );

    let mut group = c.benchmark_group("scenegraph_conversion");
    group.sample_size(10);
    group.throughput(Throughput::Elements(num_points as u64));
    group.bench_function(
        BenchmarkId::new("vega_to_scenegraph_large_scatter", num_points),
        |b| {
            b.iter_batched(
                || vg_spec.clone(),
                |spec| {
                    black_box(
                        runtime
                            .block_on(
                                converter.vega_to_scenegraph(black_box(spec), VgOpts::default()),
                            )
                            .expect("scenegraph conversion failed"),
                    )
                },
                BatchSize::LargeInput,
            );
        },
    );
    group.bench_function(
        BenchmarkId::new("vega_to_scenegraph_large_scatter_msgpack", num_points),
        |b| {
            b.iter_batched(
                || vg_spec.clone(),
                |spec| {
                    black_box(
                        runtime
                            .block_on(
                                converter
                                    .vega_to_scenegraph_msgpack(black_box(spec), VgOpts::default()),
                            )
                            .expect("scenegraph msgpack conversion failed"),
                    )
                },
                BatchSize::LargeInput,
            );
        },
    );
    group.bench_function(
        BenchmarkId::new("scenegraph_msgpack_decode_to_json", num_points),
        |b| {
            b.iter(|| {
                black_box(
                    rmp_serde::from_slice::<Value>(black_box(&warmup_msgpack))
                        .expect("scenegraph msgpack decode failed"),
                )
            });
        },
    );
    group.finish();
}

fn bench_vega_to_svg_large_scatter(c: &mut Criterion) {
    let num_points = LARGE_SCATTER_POINTS;
    let vg_spec = build_large_scatterplot_spec(num_points);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to construct benchmark runtime");
    let mut converter = VlConverter::new();

    let warmup_svg = runtime
        .block_on(converter.vega_to_svg(vg_spec.clone(), VgOpts::default()))
        .expect("warmup SVG conversion failed");
    assert!(
        warmup_svg.contains("<svg"),
        "warmup SVG conversion did not return SVG"
    );

    let mut group = c.benchmark_group("svg_conversion");
    group.sample_size(10);
    group.throughput(Throughput::Elements(num_points as u64));
    group.bench_function(
        BenchmarkId::new("vega_to_svg_large_scatter", num_points),
        |b| {
            b.iter_batched(
                || vg_spec.clone(),
                |spec| {
                    black_box(
                        runtime
                            .block_on(converter.vega_to_svg(black_box(spec), VgOpts::default()))
                            .expect("SVG conversion failed"),
                    )
                },
                BatchSize::LargeInput,
            );
        },
    );
    group.finish();
}

fn bench_vega_to_png_large_scatter(c: &mut Criterion) {
    let num_points = LARGE_SCATTER_POINTS;
    let vg_spec = build_large_scatterplot_spec(num_points);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to construct benchmark runtime");
    let mut converter = VlConverter::new();

    let warmup_png = runtime
        .block_on(converter.vega_to_png(vg_spec.clone(), VgOpts::default(), None, None))
        .expect("warmup PNG conversion failed");
    assert!(
        warmup_png.starts_with(PNG_SIGNATURE),
        "warmup PNG conversion did not return a PNG payload"
    );

    let mut group = c.benchmark_group("png_conversion");
    group.sample_size(10);
    group.throughput(Throughput::Elements(num_points as u64));
    group.bench_function(
        BenchmarkId::new("vega_to_png_large_scatter", num_points),
        |b| {
            b.iter_batched(
                || vg_spec.clone(),
                |spec| {
                    black_box(
                        runtime
                            .block_on(converter.vega_to_png(
                                black_box(spec),
                                VgOpts::default(),
                                None,
                                None,
                            ))
                            .expect("PNG conversion failed"),
                    )
                },
                BatchSize::LargeInput,
            );
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_vega_to_scenegraph_large_scatter,
    bench_vega_to_svg_large_scatter,
    bench_vega_to_png_large_scatter
);
criterion_main!(benches);

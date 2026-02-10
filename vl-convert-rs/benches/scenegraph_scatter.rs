use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use serde_json::json;
use vl_convert_rs::converter::{VgOpts, VlConverter};
use vl_convert_rs::serde_json::Value;

const LARGE_SCATTER_POINTS: usize = 50_000;

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
    group.finish();
}

criterion_group!(benches, bench_vega_to_scenegraph_large_scatter);
criterion_main!(benches);

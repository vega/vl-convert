use vegafusion_convert::context::ConvertContext;

#[tokio::main]
async fn main() {
    let mut ctx = ConvertContext::try_new().await.unwrap();
    let vl_spec: serde_json::Value = serde_json::from_str(r##"
{
    "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
    "mark": "bar",
    "encoding": {
        "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
        "y": {"aggregate": "mean", "field": "precipitation"}
    }
}
        "##).unwrap();

    let vg_spec = ctx.compile_vegalite(&vl_spec).await.unwrap();
    println!("vg_spec: {}", vg_spec)
}

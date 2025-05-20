#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum VlConvertError {
    #[class("Internal")]
    #[error("Internal error: `{0}`")]
    Internal(String),

    #[class("Text")]
    #[error("Text measurement error: `{0}`")]
    TextMeasurementError(String),
}

#[test]
fn try_it() {
    println!("Hello, world!");
}

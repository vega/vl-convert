#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScenegraphFormat {
    Json,
    Msgpack,
}

pub(crate) fn preferred_scenegraph_format(headers: &axum::http::HeaderMap) -> ScenegraphFormat {
    let Some(accept) = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
    else {
        return ScenegraphFormat::Json;
    };

    let mut json_quality: Option<i32> = None;
    let mut msgpack_quality: Option<i32> = None;

    for item in accept.split(',') {
        let Some((media_type, quality)) = parse_accept_item(item) else {
            return ScenegraphFormat::Json;
        };

        match media_type.as_str() {
            "application/json" => {
                json_quality = Some(json_quality.map_or(quality, |current| current.max(quality)));
            }
            "application/msgpack" | "application/x-msgpack" => {
                msgpack_quality =
                    Some(msgpack_quality.map_or(quality, |current| current.max(quality)));
            }
            _ => {}
        }
    }

    match (msgpack_quality, json_quality) {
        (Some(msgpack), Some(json)) if msgpack > json => ScenegraphFormat::Msgpack,
        (Some(msgpack), None) if msgpack > 0 => ScenegraphFormat::Msgpack,
        _ => ScenegraphFormat::Json,
    }
}

fn parse_accept_item(item: &str) -> Option<(String, i32)> {
    let mut parts = item.split(';');
    let media_type = parts.next()?.trim().to_ascii_lowercase();
    if media_type.is_empty() {
        return None;
    }

    let mut quality = 1000;
    for param in parts {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }
        let (name, value) = param.split_once('=')?;
        if name.trim().eq_ignore_ascii_case("q") {
            quality = parse_quality(value.trim())?;
        }
    }

    Some((media_type, quality))
}

fn parse_quality(value: &str) -> Option<i32> {
    let parsed: f32 = value.parse().ok()?;
    if !(0.0..=1.0).contains(&parsed) {
        return None;
    }
    Some((parsed * 1000.0).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(headers: &[(&str, &str)]) -> axum::http::Request<axum::body::Body> {
        let mut builder = axum::http::Request::builder().method("GET").uri("/test");
        for &(key, val) in headers {
            builder = builder.header(key, val);
        }
        builder.body(axum::body::Body::empty()).unwrap()
    }

    #[test]
    fn test_preferred_scenegraph_format_json_preferred_when_msgpack_has_lower_quality() {
        let req = make_request(&[("accept", "application/json, application/msgpack;q=0.1")]);
        assert_eq!(
            preferred_scenegraph_format(req.headers()),
            ScenegraphFormat::Json
        );
    }

    #[test]
    fn test_preferred_scenegraph_format_defaults_to_json_on_malformed_accept() {
        let req = make_request(&[("accept", "application/json;q=bogus")]);
        assert_eq!(
            preferred_scenegraph_format(req.headers()),
            ScenegraphFormat::Json
        );
    }
}

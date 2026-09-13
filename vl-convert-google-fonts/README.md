## Overview
This crate provides a client for downloading and caching fonts from the [Google Fonts](https://fonts.google.com/) catalog via the CSS2 API. It is used internally by [`vl-convert-rs`](https://crates.io/crates/vl-convert-rs) to resolve font references in Vega and Vega-Lite chart specifications.

## Features
- Downloads TrueType fonts from the Google Fonts CSS2 API
- Dual-layer disk cache (CSS responses and font files) with LRU eviction
- Concurrent download deduplication (multiple requests for the same font share a single download)
- Stale-cache recovery: automatically re-fetches CSS when cached font URLs expire
- Magic-byte validation to reject corrupt or non-font data
- Optional `fontdb` feature for direct registration into a [`fontdb::Database`](https://docs.rs/fontdb/)

## Feature Flags
| Feature  | Description |
|----------|-------------|
| `fontdb` | Adds `GoogleFontsDatabaseExt` trait for registering/unregistering font batches on a `fontdb::Database` |

## Environment Variables

These variables configure the shared Google Fonts client in Python, Rust, the CLI, and the server. Set them before the client is initialized.

| Variable | Effect |
| --- | --- |
| `VLC_GOOGLE_FONTS_CACHE_DIR` | Absolute cache directory for downloaded fonts and CSS. Set to `none` to disable disk caching. Defaults to the platform cache directory plus `vl-convert/google-fonts`. |
| `VLC_GOOGLE_FONTS_CSS2_URL` | Google Fonts CSS2 endpoint, for a mirror or local test server. Defaults to `https://fonts.googleapis.com/css2`. |

These replace the prerelease names `VL_CONVERT_FONT_CACHE_DIR` and `VL_CONVERT_GOOGLE_FONTS_CSS2_URL`, which are no longer recognized. Rust callers can also set the corresponding fields in `ClientConfig` directly.

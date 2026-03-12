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

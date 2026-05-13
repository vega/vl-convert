---
title: Conversion Overrides
path: advanced/conversion-overrides
section: Advanced
order: 407
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Conversion Overrides

Most conversion options are per-call or per-request overrides. Persistent
defaults belong in `configure()`, `VlcConfig`, `--vlc-config`, or server startup
flags; see {doc}`configuration`.

Use Python keyword arguments, Rust option structs, CLI command flags, or server
JSON request fields depending on the interface.

| Override | Applies To | Server Gate | Notes |
| --- | --- | --- | --- |
| `theme` | Vega-Lite | None | Named built-in or custom theme. Vega inputs do not use Vega-Lite themes. |
| `config` | Vega-Lite (all interfaces); Vega (Python/Rust/server only) | None | Merged as Vega/Vega-Lite config. The CLI exposes `--config` on Vega-Lite commands. |
| `format_locale`, `time_format_locale` | Vega-Lite and Vega | None | Built-in locale name or locale object. |
| `background`, `width`, `height` | Vega-Lite and Vega | None | Applied before rendering/evaluation; SVG input conversion does not use these fields. |
| `scale` | PNG and JPEG outputs | None | Raster scale. PDF is vector output and does not use scale. |
| `ppi` | PNG outputs | None | Combines with scale as `scale * ppi / 72`. |
| `quality` | JPEG outputs | None | `0-100` inclusive, default `90`. |
| `bundle` | SVG and HTML outputs | None | Embeds assets for SVG; controls dependency delivery for HTML. |
| `renderer` | HTML outputs | None | `svg`, `canvas`, or `hybrid`. |
| `vl_version` | Vega-Lite | None | Selects the bundled Vega-Lite compiler version. |
| `google_fonts` | Vega-Lite and Vega | `--allow-google-fonts` | Python/Rust accept per-call font lists directly; only server requests require the gate. CLI uses global `--google-font`. |
| `vega_plugin` | Vega-Lite and Vega | `--allow-per-request-plugins` | Python/Rust also require `allow_per_request_plugins=true` in converter config. CLI uses global `--vega-plugin` for startup plugins. |
| `fullscreen` | Vega Editor URL outputs | None | Controls URL generation only. |
| `svg` | SVG input conversions | None | Server SVG routes use a JSON body with `svg` as a string field. |

::::{interface} server
The server rejects unknown JSON fields, so misspelled override names fail
instead of being ignored.
::::

::::{interface} cli
CLI flags are scoped to one command invocation. Global flags such as
`--google-font`, `--vega-plugin`, and `--allowed-base-urls` configure the
converter used by that invocation.
::::

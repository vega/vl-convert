#![allow(clippy::uninlined_format_args)]
#![doc = include_str!("../README.md")]

mod cli_types;
mod commands;
mod handlers;
mod io_utils;
mod serve;

use clap::Parser;
use std::num::NonZeroU64;
use std::str::FromStr;
use vl_convert_rs::converter::{
    vega_to_url, vegalite_to_url, HtmlOpts, JpegOpts, PdfOpts, PngOpts, Renderer, SvgOpts, UrlOpts,
    VgOpts, VlConverter, VlOpts, VlcConfig,
};
use vl_convert_rs::{anyhow, anyhow::bail};

use cli_types::Cli;
use commands::Commands;
use handlers::{
    cat_theme, list_themes, vg_2_jpeg, vg_2_pdf, vg_2_png, vg_2_svg, vl_2_jpeg, vl_2_pdf, vl_2_png,
    vl_2_svg, vl_2_vg,
};
use io_utils::{
    apply_global_font_dirs, apply_global_google_fonts_cache, expand_allowed_base_urls,
    flatten_plugin_domains, parse_base_url_arg, parse_format_locale_option,
    parse_google_font_requests, parse_nullable_string_arg, parse_themes_json,
    parse_time_format_locale_option, parse_vl_version, read_config_json, read_input_string,
    resolve_vlc_config, synthesize_log_filter, write_output_binary, write_output_string,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut cli = Cli::parse();

    let log_filter = synthesize_log_filter(cli.log_level, cli.log_filter.as_deref());
    vl_convert_server::init_tracing(&log_filter, cli.log_format);

    // Handle config-path before loading the config so it works even with a broken config file.
    if let Commands::ConfigPath = cli.command {
        println!("{}", vl_convert_rs::vlc_config_path().display());
        return Ok(());
    }

    if let Commands::Serve(args) = &cli.command {
        if let Some(surface) = args.openapi_dump_surface() {
            return serve::dump_openapi(surface);
        }
    }

    let google_font_families = cli.google_font.clone();
    let plugin_import_domains = flatten_plugin_domains(&cli.plugin_import_domains);
    let vega_plugins = if cli.vega_plugin.is_empty() {
        None
    } else {
        Some(cli.vega_plugin.clone())
    };

    let mut base_config = resolve_vlc_config(cli.vlc_config.as_deref())?;

    apply_global_font_dirs(&cli)?;
    apply_global_google_fonts_cache(&cli)?;

    if let Some(ref raw) = cli.base_url {
        base_config.base_url = parse_base_url_arg(raw)?;
    }
    if !cli.allowed_base_urls.is_empty() {
        base_config.allowed_base_urls = expand_allowed_base_urls(&cli.allowed_base_urls);
    }
    if let Some(v) = cli.auto_google_fonts {
        base_config.auto_google_fonts = v;
    }
    if let Some(v) = cli.embed_local_fonts {
        base_config.embed_local_fonts = v;
    }
    if let Some(v) = cli.subset_fonts {
        base_config.subset_fonts = v;
    }
    if let Some(ref mf) = cli.missing_fonts {
        base_config.missing_fonts = mf.to_policy();
    }
    if let Some(heap) = cli.max_v8_heap_size_mb {
        // CLI passes `0` to mean "no cap"; otherwise treat as a hard cap.
        base_config.max_v8_heap_size_mb = NonZeroU64::new(heap);
    }
    if let Some(timeout) = cli.max_v8_execution_time_secs {
        base_config.max_v8_execution_time_secs = NonZeroU64::new(timeout);
    }
    if let Some(v) = cli.gc_after_conversion {
        base_config.gc_after_conversion = v;
    }
    if let Some(plugins) = vega_plugins {
        base_config.vega_plugins = plugins;
    }
    if !plugin_import_domains.is_empty() {
        base_config.plugin_import_domains = plugin_import_domains;
    }
    if let Some(google_fonts) = parse_google_font_requests(&google_font_families)? {
        base_config.google_fonts = google_fonts;
    }
    if let Some(threshold) = cli.google_font_variant_threshold {
        base_config.google_font_variant_threshold = NonZeroU64::new(threshold);
    }
    // Theme, locale, and themes globals use `Option<String>` from clap to
    // distinguish absent flags from explicit overrides. The literal string
    // `null` clears the field during parsing.
    if let Some(raw) = cli.default_theme.as_deref() {
        base_config.default_theme = parse_nullable_string_arg(raw);
    }
    if let Some(raw) = cli.default_format_locale.as_deref() {
        base_config.default_format_locale = parse_format_locale_option(Some(raw))?;
    }
    if let Some(raw) = cli.default_time_format_locale.as_deref() {
        base_config.default_time_format_locale = parse_time_format_locale_option(Some(raw))?;
    }
    if let Some(raw) = cli.themes.as_deref() {
        base_config.themes = parse_themes_json(raw)?.unwrap_or_default();
    }
    // One-shot conversion subcommands always run with one worker; the
    // serve subcommand keeps whatever the user supplied (or the
    // library default) so `--workers <N>` can scale the worker pool.
    if !cli.command.is_serve() {
        base_config.num_workers = NonZeroU64::new(1).expect("1 is non-zero");
    }

    // Dispatch to `vl-convert serve` after global flags have populated the
    // converter state. `run_serve` owns the serve-mode signal and shutdown
    // lifecycle.
    if cli.command.is_serve() {
        let command = std::mem::replace(&mut cli.command, Commands::ConfigPath);
        let Commands::Serve(args) = command else {
            unreachable!("is_serve() guard above");
        };
        return serve::run_serve(&cli, base_config, args).await;
    }
    let command = cli.command;

    // Wrap all conversion work in select! so Ctrl+C drops the conversion
    // future, triggering CallerGoneGuard to terminate V8 promptly.
    tokio::select! {
        result = run_command(command, base_config, google_font_families) => result?,
        _ = tokio::signal::ctrl_c() => {
            // Returning here drops the run_command future, which drops
            // resp_rx.await inside run_on_worker, firing CallerGoneGuard
            // to terminate any in-flight V8 execution.
            bail!("interrupted");
        }
    }

    Ok(())
}

async fn run_command(
    command: Commands,
    base_config: VlcConfig,
    google_font_families: Vec<String>,
) -> Result<(), anyhow::Error> {
    use crate::Commands::*;
    match command {
        Vl2vg {
            input: input_vegalite_file,
            output: output_vega_file,
            vl_version,
            theme,
            config,
            pretty,
        } => {
            vl_2_vg(
                input_vegalite_file.as_deref(),
                output_vega_file.as_deref(),
                &vl_version,
                theme,
                config,
                pretty,
                base_config,
            )
            .await?
        }
        Vl2svg {
            input,
            output,
            vl_version,
            theme,
            config,
            format_locale,
            time_format_locale,
            bundle,
        } => {
            let svg_opts = SvgOpts { bundle };
            vl_2_svg(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                format_locale,
                time_format_locale,
                svg_opts,
                base_config,
            )
            .await?
        }
        Vl2png {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            ppi,
            format_locale,
            time_format_locale,
        } => {
            vl_2_png(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                scale,
                ppi,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vl2jpeg {
            input,
            output,
            vl_version,
            theme,
            config,
            scale,
            quality,
            format_locale,
            time_format_locale,
        } => {
            vl_2_jpeg(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                scale,
                quality,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vl2pdf {
            input,
            output,
            vl_version,
            theme,
            config,
            format_locale,
            time_format_locale,
        } => {
            vl_2_pdf(
                input.as_deref(),
                output.as_deref(),
                &vl_version,
                theme,
                config,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vl2url {
            input,
            output,
            fullscreen,
        } => {
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec = serde_json::from_str(&vl_str)?;
            let url = vegalite_to_url(&vl_spec, UrlOpts { fullscreen })?;
            write_output_string(output.as_deref(), &url)?
        }
        Vl2html {
            input,
            output,
            vl_version,
            theme,
            config,
            bundle,
            format_locale,
            time_format_locale,
            renderer,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec: serde_json::Value = serde_json::from_str(&vl_str)?;
            let config = read_config_json(config)?;
            let vl_version = parse_vl_version(&vl_version)?;
            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;
            let renderer = renderer.unwrap_or_else(|| "svg".to_string());

            let converter = VlConverter::with_config(base_config)?;
            let html_output = converter
                .vegalite_to_html(
                    vl_spec,
                    VlOpts {
                        config,
                        theme,
                        vl_version,
                        format_locale,
                        time_format_locale,
                        google_fonts,
                        ..Default::default()
                    },
                    HtmlOpts {
                        bundle,
                        renderer: Renderer::from_str(&renderer)?,
                    },
                )
                .await?;
            write_output_string(output.as_deref(), &html_output.html)?;
        }
        Vl2fonts {
            input,
            output,
            vl_version,
            theme,
            config,
            include_font_face,
            format_locale,
            time_format_locale,
            pretty,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vl_str = read_input_string(input.as_deref())?;
            let vl_spec: serde_json::Value = serde_json::from_str(&vl_str)?;
            let config = read_config_json(config)?;
            let vl_version = parse_vl_version(&vl_version)?;
            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;

            let auto_google_fonts = base_config.auto_google_fonts;
            let embed_local_fonts = base_config.embed_local_fonts;
            let subset_fonts = base_config.subset_fonts;
            let converter = VlConverter::with_config(base_config)?;
            let fonts = converter
                .vegalite_fonts(
                    vl_spec,
                    VlOpts {
                        config,
                        theme,
                        vl_version,
                        format_locale,
                        time_format_locale,
                        google_fonts,
                        ..Default::default()
                    },
                    auto_google_fonts,
                    embed_local_fonts,
                    include_font_face,
                    subset_fonts,
                )
                .await?;
            let json = if pretty {
                serde_json::to_string_pretty(&fonts)?
            } else {
                serde_json::to_string(&fonts)?
            };
            write_output_string(output.as_deref(), &json)?;
        }
        Vg2svg {
            input,
            output,
            format_locale,
            bundle,
            time_format_locale,
        } => {
            let svg_opts = SvgOpts { bundle };
            vg_2_svg(
                input.as_deref(),
                output.as_deref(),
                format_locale,
                time_format_locale,
                svg_opts,
                base_config,
            )
            .await?
        }
        Vg2png {
            input,
            output,
            scale,
            ppi,
            format_locale,
            time_format_locale,
        } => {
            vg_2_png(
                input.as_deref(),
                output.as_deref(),
                scale,
                ppi,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vg2jpeg {
            input,
            output,
            scale,
            quality,
            format_locale,
            time_format_locale,
        } => {
            vg_2_jpeg(
                input.as_deref(),
                output.as_deref(),
                scale,
                quality,
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vg2pdf {
            input,
            output,
            format_locale,
            time_format_locale,
        } => {
            vg_2_pdf(
                input.as_deref(),
                output.as_deref(),
                format_locale,
                time_format_locale,
                base_config,
            )
            .await?
        }
        Vg2url {
            input,
            output,
            fullscreen,
        } => {
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec = serde_json::from_str(&vg_str)?;
            let url = vega_to_url(&vg_spec, UrlOpts { fullscreen })?;
            write_output_string(output.as_deref(), &url)?
        }
        Vg2html {
            input,
            output,
            bundle,
            format_locale,
            time_format_locale,
            renderer,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec: serde_json::Value = serde_json::from_str(&vg_str)?;

            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;

            let renderer = renderer.unwrap_or_else(|| "svg".to_string());

            let converter = VlConverter::with_config(base_config)?;
            let html_output = converter
                .vega_to_html(
                    vg_spec,
                    VgOpts {
                        format_locale,
                        time_format_locale,
                        google_fonts,
                        ..Default::default()
                    },
                    HtmlOpts {
                        bundle,
                        renderer: Renderer::from_str(&renderer)?,
                    },
                )
                .await?;
            write_output_string(output.as_deref(), &html_output.html)?;
        }
        Vg2fonts {
            input,
            output,
            include_font_face,
            format_locale,
            time_format_locale,
            pretty,
        } => {
            let google_fonts = parse_google_font_requests(&google_font_families)?;
            let vg_str = read_input_string(input.as_deref())?;
            let vg_spec: serde_json::Value = serde_json::from_str(&vg_str)?;
            let format_locale = parse_format_locale_option(format_locale.as_deref())?;
            let time_format_locale =
                parse_time_format_locale_option(time_format_locale.as_deref())?;

            let auto_google_fonts = base_config.auto_google_fonts;
            let embed_local_fonts = base_config.embed_local_fonts;
            let subset_fonts = base_config.subset_fonts;
            let converter = VlConverter::with_config(base_config)?;
            let fonts = converter
                .vega_fonts(
                    vg_spec,
                    VgOpts {
                        google_fonts,
                        format_locale,
                        time_format_locale,
                        ..Default::default()
                    },
                    auto_google_fonts,
                    embed_local_fonts,
                    include_font_face,
                    subset_fonts,
                )
                .await?;
            let json = if pretty {
                serde_json::to_string_pretty(&fonts)?
            } else {
                serde_json::to_string(&fonts)?
            };
            write_output_string(output.as_deref(), &json)?;
        }
        Svg2png {
            input,
            output,
            scale,
            ppi,
        } => {
            let svg = read_input_string(input.as_deref())?;
            let converter = VlConverter::with_config(base_config)?;
            let png_output = converter
                .svg_to_png(
                    &svg,
                    PngOpts {
                        scale: Some(scale),
                        ppi: Some(ppi),
                    },
                )
                .await?;
            write_output_binary(output.as_deref(), &png_output.data, "PNG")?;
        }
        Svg2jpeg {
            input,
            output,
            scale,
            quality,
        } => {
            let svg = read_input_string(input.as_deref())?;
            let converter = VlConverter::with_config(base_config)?;
            let jpeg_output = converter
                .svg_to_jpeg(
                    &svg,
                    JpegOpts {
                        scale: Some(scale),
                        quality: Some(quality),
                    },
                )
                .await?;
            write_output_binary(output.as_deref(), &jpeg_output.data, "JPEG")?;
        }
        Svg2pdf { input, output } => {
            let svg = read_input_string(input.as_deref())?;
            let converter = VlConverter::with_config(base_config)?;
            let pdf_output = converter.svg_to_pdf(&svg, PdfOpts::default()).await?;
            write_output_binary(output.as_deref(), &pdf_output.data, "PDF")?;
        }
        LsThemes => list_themes(base_config).await?,
        CatTheme { theme } => cat_theme(&theme, base_config).await?,
        ConfigPath => unreachable!("handled before config loading"),
        Serve(_) => unreachable!("handled by serve::run_serve before run_command"),
    }

    Ok(())
}

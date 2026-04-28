use std::io::{self, IsTerminal, Read, Write};
use std::str::FromStr;
use vl_convert_google_fonts::{FontStyle, VariantRequest};
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, TimeFormatLocale, VlcConfig,
};
use vl_convert_rs::module_loader::import_map::VlVersion;
use vl_convert_rs::{anyhow, anyhow::bail};

/// Register a `--font-dir` value with the library so subsequent conversions
/// can resolve fonts from that directory.
pub(crate) fn register_font_dir(dir: Option<String>) -> anyhow::Result<()> {
    if let Some(dir) = dir {
        vl_convert_rs::register_font_directory(&dir)?;
    }
    Ok(())
}

/// `=BOOL` value parser for clap flags. Mirrors the server CLI's
/// `parse_boolish_arg`; accepts the same string forms.
pub(crate) fn parse_boolish_arg(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err("expected one of: true, false, 1, 0, yes, no, on, off".to_string()),
    }
}

/// Parse a `--base-url` value into a `BaseUrlSetting`. Reserved values
/// `default` and `disabled` map to the corresponding enum variants. A
/// URL with scheme (`https://...`, `file://...`) is taken as-is. Any
/// other value is treated as a filesystem path and must be absolute
/// (after `~` expansion); relative paths are rejected so they can't be
/// confused with reserved values.
pub(crate) fn parse_base_url_arg(raw: &str) -> Result<BaseUrlSetting, anyhow::Error> {
    let trimmed = raw.trim();
    match trimmed {
        "default" => return Ok(BaseUrlSetting::Default),
        "disabled" => return Ok(BaseUrlSetting::Disabled),
        "" => bail!("--base-url must not be empty"),
        _ => {}
    }
    if trimmed.contains("://") {
        return Ok(BaseUrlSetting::Custom(trimmed.to_string()));
    }
    let expanded = shellexpand::tilde(trimmed).to_string();
    if !std::path::Path::new(&expanded).is_absolute() {
        bail!(
            "--base-url path must be absolute, got '{trimmed}'. Use \
             'default' or 'disabled' for reserved behaviors, a URL with \
             scheme like 'https://example.com/', or an absolute path."
        );
    }
    Ok(BaseUrlSetting::Custom(expanded))
}

/// Parse a `--allowed-base-urls` value into a `Vec<String>`. Accepts:
/// reserved values `none` / `net` / `all`, a JSON-array literal
/// (e.g. `["http:","https:"]`), or `@<path>` referencing a file
/// containing the JSON array.
pub(crate) fn parse_allowed_base_urls(raw: &str) -> Result<Vec<String>, anyhow::Error> {
    match raw.trim() {
        "none" => return Ok(Vec::new()),
        "net" => return Ok(vec!["http:".to_string(), "https:".to_string()]),
        "all" => return Ok(vec!["*".to_string()]),
        _ => {}
    }
    let json_text = if let Some(path_str) = raw.strip_prefix('@') {
        let expanded = shellexpand::tilde(path_str.trim()).to_string();
        std::fs::read_to_string(&expanded).map_err(|err| {
            anyhow::anyhow!("failed to read --allowed-base-urls @ {}: {err}", expanded)
        })?
    } else if raw.trim_start().starts_with('[') {
        raw.to_string()
    } else {
        bail!(
            "--allowed-base-urls must be one of: 'none', 'net', 'all', a JSON \
             array literal like '[\"https:\"]', or '@<path>' to read the JSON \
             from a file. Got: '{raw}'"
        );
    };
    let value: serde_json::Value = serde_json::from_str(&json_text)
        .map_err(|err| anyhow::anyhow!("--allowed-base-urls must be a JSON array: {err}"))?;
    match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|v| match v {
                serde_json::Value::String(s) => Ok(s),
                _ => Err(anyhow::anyhow!(
                    "--allowed-base-urls must be a JSON array of strings"
                )),
            })
            .collect(),
        _ => Err(anyhow::anyhow!(
            "--allowed-base-urls must be a JSON array of strings"
        )),
    }
}

/// Parse a `--google-font` value like `"Roboto"` or `"Roboto:400,700italic"`
/// into a family name and optional variant list.
pub(crate) fn parse_google_font_arg(
    s: &str,
) -> Result<(String, Option<Vec<VariantRequest>>), anyhow::Error> {
    let Some((family, variants_str)) = s.split_once(':') else {
        return Ok((s.to_string(), None));
    };
    let mut variants = Vec::new();
    for token in variants_str.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (weight_str, style) = if let Some(w) = token.strip_suffix("italic") {
            (w, FontStyle::Italic)
        } else {
            (token, FontStyle::Normal)
        };
        let weight: u16 = weight_str.parse().map_err(|_| {
            anyhow::anyhow!(
                "Invalid font variant '{token}' in --google-font '{s}'. \
                 Expected format: 400, 700italic, etc."
            )
        })?;
        variants.push(VariantRequest { weight, style });
    }
    if variants.is_empty() {
        Ok((family.to_string(), None))
    } else {
        Ok((family.to_string(), Some(variants)))
    }
}

/// Parse `--google-font` args into `GoogleFontRequest`s for per-call opts.
pub(crate) fn parse_google_font_requests(
    fonts: &[String],
) -> Result<Option<Vec<GoogleFontRequest>>, anyhow::Error> {
    if fonts.is_empty() {
        return Ok(None);
    }
    let mut requests = Vec::new();
    for spec in fonts {
        let (family, variants) = parse_google_font_arg(spec)?;
        requests.push(GoogleFontRequest { family, variants });
    }
    Ok(Some(requests))
}

pub(crate) fn flatten_plugin_domains(raw: &[String]) -> Vec<String> {
    raw.iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(crate) fn parse_vl_version(vl_version: &str) -> Result<VlVersion, anyhow::Error> {
    VlVersion::from_str(vl_version)
        .map_err(|_| anyhow::anyhow!("Invalid or unsupported Vega-Lite version: {vl_version}"))
}

pub(crate) fn read_input_string(input: Option<&str>) -> Result<String, anyhow::Error> {
    match input {
        Some(path) if path != "-" => std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("Failed to read input file: {}\n{}", path, err)),
        _ => {
            // Check if stdin is an interactive terminal
            if io::stdin().is_terminal() {
                eprintln!("Reading from stdin... (Press Ctrl-D when done, or use -i <file>)");
            }

            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|err| anyhow::anyhow!("Failed to read from stdin: {}", err))?;

            // Check for empty or whitespace-only input
            if buffer.trim().is_empty() {
                bail!("No input provided. Provide a specification via stdin or use -i <file>");
            }

            Ok(buffer)
        }
    }
}

/// Read a file that is always a filesystem path (never stdin).
///
/// This function is used for reading configuration files (locale, time format)
/// that should not come from stdin. For reading input specifications that may
/// come from stdin or a file, use `read_input_string()` instead.
fn read_file_string(path: &str) -> Result<String, anyhow::Error> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) => {
            bail!("Failed to read file: {}\n{}", path, err);
        }
    }
}

pub(crate) fn parse_as_json(input_str: &str) -> Result<serde_json::Value, anyhow::Error> {
    match serde_json::from_str::<serde_json::Value>(input_str) {
        Ok(input_json) => Ok(input_json),
        Err(err) => {
            bail!("Failed to parse input file as JSON: {}", err);
        }
    }
}

fn format_locale_from_str(s: &str) -> Result<FormatLocale, anyhow::Error> {
    if s.ends_with(".json") {
        let s = read_file_string(s)?;
        Ok(FormatLocale::Object(parse_as_json(&s)?))
    } else {
        Ok(FormatLocale::Name(s.to_string()))
    }
}

pub(crate) fn parse_format_locale_option(
    format_locale: Option<&str>,
) -> Result<Option<FormatLocale>, anyhow::Error> {
    format_locale.map(format_locale_from_str).transpose()
}

fn time_format_locale_from_str(s: &str) -> Result<TimeFormatLocale, anyhow::Error> {
    if s.ends_with(".json") {
        let s = read_file_string(s)?;
        Ok(TimeFormatLocale::Object(parse_as_json(&s)?))
    } else {
        Ok(TimeFormatLocale::Name(s.to_string()))
    }
}

pub(crate) fn parse_time_format_locale_option(
    time_format_locale: Option<&str>,
) -> Result<Option<TimeFormatLocale>, anyhow::Error> {
    time_format_locale
        .map(time_format_locale_from_str)
        .transpose()
}

pub(crate) fn write_output_string(
    output: Option<&str>,
    output_str: &str,
) -> Result<(), anyhow::Error> {
    match output {
        Some(path) if path != "-" => {
            // File output: write as-is without modification
            std::fs::write(path, output_str)
                .map_err(|err| anyhow::anyhow!("Failed to write output to {}\n{}", path, err))
        }
        _ => {
            // Stdout output: ensure trailing newline and handle BrokenPipe
            let stdout = io::stdout();
            let mut handle = stdout.lock();

            // Write the string
            if let Err(err) = handle.write_all(output_str.as_bytes()) {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                return Err(anyhow::anyhow!("Failed to write to stdout: {}", err));
            }

            // Add trailing newline if not already present
            if !output_str.ends_with('\n') {
                if let Err(err) = handle.write_all(b"\n") {
                    if err.kind() == io::ErrorKind::BrokenPipe {
                        std::process::exit(0);
                    }
                    return Err(anyhow::anyhow!(
                        "Failed to write newline to stdout: {}",
                        err
                    ));
                }
            }

            // Flush
            if let Err(err) = handle.flush() {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                }
                return Err(anyhow::anyhow!("Failed to flush stdout: {}", err));
            }

            Ok(())
        }
    }
}

/// Write binary output data to a file or stdout with TTY safety guard.
///
/// # Behavior
/// - `output = Some(path)` where `path != "-"`: Write to file
/// - `output = Some("-")`: Force write to stdout (user override)
/// - `output = None`: Write to stdout only if not a TTY (safety guard)
///
/// # TTY Safety Guard
/// When `output = None` and stdout is a terminal, this function refuses to write
/// binary data to prevent terminal corruption. Users must either:
/// - Redirect to a file: `vl-convert vl2png -o output.png`
/// - Pipe to another command: `vl-convert vl2png | display`
/// - Force stdout: `vl-convert vl2png -o -`
///
/// # Testing Note
/// The TTY safety guard is tested manually because automated tests run with
/// piped stdout (not a TTY). To verify:
/// ```bash
/// # Should refuse (interactive terminal)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png
///
/// # Should succeed (explicit override)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png -o -
///
/// # Should succeed (piped)
/// $ echo '{"$schema": "..."}' | vl-convert vl2png | cat > test.png
/// ```
pub(crate) fn write_output_binary(
    output: Option<&str>,
    output_data: &[u8],
    format_name: &str,
) -> Result<(), anyhow::Error> {
    match output {
        Some(path) if path != "-" => std::fs::write(path, output_data)
            .map_err(|err| anyhow::anyhow!("Failed to write output to {}\n{}", path, err)),
        Some(_) => {
            // Explicit "-": write to stdout unconditionally (user override)
            write_stdout_bytes(output_data)
        }
        None => {
            // Implicit stdout: TTY safety guard
            if io::stdout().is_terminal() {
                bail!(
                    "Refusing to write binary {} data to terminal.\n\
                     Use -o <file> to write to a file, or pipe to another command.\n\
                     Use -o - to force output to stdout.",
                    format_name
                );
            }
            write_stdout_bytes(output_data)
        }
    }
}

/// Set stdout to binary mode on Windows to prevent newline translation.
///
/// On Windows, stdout defaults to "text mode" which translates `\n` (0x0A) to `\r\n` (0x0D 0x0A)
/// and treats `\x1A` (Ctrl-Z) as EOF. This corrupts binary data like PNG, JPEG, and PDF files.
///
/// This function uses the Windows C runtime `_setmode` function to switch stdout to binary mode.
/// On Unix systems (Linux, macOS), this is a no-op because stdout is always binary.
///
/// # References
/// - [Microsoft _setmode Documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/setmode)
///
/// # Safety
/// Uses unsafe FFI to call the Windows CRT function `_setmode`.
#[cfg(target_family = "windows")]
fn set_stdout_binary_mode() -> Result<(), anyhow::Error> {
    extern "C" {
        fn _setmode(fd: i32, mode: i32) -> i32;
    }
    const STDOUT_FILENO: i32 = 1;
    const O_BINARY: i32 = 0x8000;
    unsafe {
        let result = _setmode(STDOUT_FILENO, O_BINARY);
        if result == -1 {
            Err(anyhow::anyhow!("Failed to set binary mode on stdout"))
        } else {
            Ok(())
        }
    }
}

/// No-op on Unix systems where stdout is always binary.
#[cfg(not(target_family = "windows"))]
fn set_stdout_binary_mode() -> Result<(), anyhow::Error> {
    Ok(())
}

fn write_stdout_bytes(data: &[u8]) -> Result<(), anyhow::Error> {
    // Set stdout to binary mode on Windows before writing
    set_stdout_binary_mode()?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Write data, handling BrokenPipe as clean exit
    if let Err(err) = handle.write_all(data) {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(anyhow::anyhow!("Failed to write to stdout: {}", err));
    }

    // Flush, handling BrokenPipe as clean exit
    if let Err(err) = handle.flush() {
        if err.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(anyhow::anyhow!("Failed to flush stdout: {}", err));
    }

    Ok(())
}

/// Resolve the bootstrap `VlcConfig` from `--vlc-config <value>`.
///
/// `value` may be:
/// - `None` (flag omitted) — load the platform default config path if it
///   exists, else return `VlcConfig::default()`.
/// - `Some("disabled")` — skip config-file loading; return
///   `VlcConfig::default()`.
/// - `Some("<absolute path>")` — load that specific file. Relative paths
///   are rejected to avoid ambiguity with the `disabled` reserved value.
pub(crate) fn resolve_vlc_config(vlc_config: Option<&str>) -> Result<VlcConfig, anyhow::Error> {
    let path = match vlc_config {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed == "disabled" {
                return Ok(VlcConfig::default());
            }
            let expanded = shellexpand::tilde(trimmed).to_string();
            let path = std::path::PathBuf::from(&expanded);
            if !path.is_absolute() {
                bail!(
                    "--vlc-config path must be absolute, got '{expanded}'. \
                     Use 'disabled' to skip config-file loading, or pass an \
                     absolute path."
                );
            }
            path
        }
        None => {
            let default = vl_convert_rs::vlc_config_path();
            if !default.exists() {
                return Ok(VlcConfig::default());
            }
            default
        }
    };
    VlcConfig::from_file(&path)
}

fn normalize_config_path(config: Option<String>) -> Option<String> {
    config.map(|c| shellexpand::tilde(c.trim()).to_string())
}

pub(crate) fn read_config_json(
    config: Option<String>,
) -> Result<Option<serde_json::Value>, anyhow::Error> {
    let config = normalize_config_path(config);
    match config {
        None => Ok(None),
        Some(config) => {
            let config_str = match std::fs::read_to_string(&config) {
                Ok(config_str) => config_str,
                Err(err) => {
                    bail!("Failed to read config file: {}\n{}", config, err);
                }
            };
            let config_json: serde_json::Value = serde_json::from_str(&config_str)?;
            Ok(Some(config_json))
        }
    }
}

use std::collections::HashMap;
use std::io::Read;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;
use vl_convert_google_fonts::{FontStyle, VariantRequest};
use vl_convert_rs::anyhow::{self, anyhow, bail};
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
};
use vl_convert_server::LogFormat;

#[derive(Debug, Clone, Copy)]
pub(super) enum InputKind {
    Cli,
    Env,
}

impl InputKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Cli => "CLI",
            Self::Env => "environment",
        }
    }
}

pub(super) fn field_name(source: InputKind, field: &'static str) -> String {
    match source {
        InputKind::Cli => format!("CLI {field}"),
        InputKind::Env => format!("env {field}"),
    }
}

pub(super) fn parse_path_arg(raw: &str) -> Result<PathBuf, String> {
    Ok(expand_path(raw))
}

/// Value parser for `--unix-socket` / `--admin-unix-socket`.
///
/// Grammar:
/// * Absolute filesystem path after tilde expansion.
/// * `~` prefix expanded via the same `shellexpand::tilde` helper as
///   the other path flags (consistent with `settings/parsers.rs:240-242`).
/// * Relative paths rejected at parse time with an actionable message.
/// * On `cfg(windows)`, every invocation is rejected with a fixed
///   message pointing the user at `--port`.
pub(super) fn parse_socket_path_arg(raw: &str) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let _ = raw;
        return Err(crate::listen::WINDOWS_UDS_REJECTION.to_string());
    }

    #[cfg(not(windows))]
    {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("socket path must not be empty".to_string());
        }
        let expanded = expand_path(trimmed);
        if !expanded.is_absolute() {
            return Err(format!(
                "socket path '{raw}' must be absolute (use /abs/path or ~/path)"
            ));
        }
        Ok(expanded)
    }
}

/// Value parser for `--socket-mode`.
///
/// Accepts a 3-or-4 digit octal literal (`600`, `0660`, `0770`), parses
/// via `u32::from_str_radix(_, 8)`, and rejects:
///   * Any value with `other` bits set (`mode & 0o007 != 0`).
///   * The all-zero mode (`0o000`).
///
/// Values with group bits set (`mode & 0o070 != 0`) are *accepted* —
/// the loopback-style warning in §2.9 is emitted at bind time, not
/// here.
pub(super) fn parse_socket_mode_arg(raw: &str) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("socket mode must not be empty".to_string());
    }
    // Strip an optional leading "0" or "0o" so users can write either
    // `600` (shell-friendly) or `0600`/`0o600` (Rust/chmod-friendly).
    let body = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
        .unwrap_or(trimmed);
    let mode = u32::from_str_radix(body, 8)
        .map_err(|err| format!("invalid octal socket mode '{raw}': {err}"))?;
    if mode == 0 {
        return Err("socket mode must not be 0o000 (unusable permissions)".to_string());
    }
    if mode & 0o007 != 0 {
        return Err(format!(
            "socket mode {raw} grants access to 'other' users; \
             drop the last octal digit's lower three bits (e.g. 0600, 0660, 0770)"
        ));
    }
    if mode & !0o777 != 0 {
        return Err(format!(
            "socket mode {raw} sets bits outside the 0o777 permission range"
        ));
    }
    Ok(mode)
}

pub(super) fn parse_boolish_arg(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err("expected one of: true, false, 1, 0, yes, no, on, off".to_string()),
    }
}

pub(super) fn parse_positive_i64_arg(raw: &str) -> Result<i64, String> {
    let parsed: i64 = raw
        .trim()
        .parse()
        .map_err(|err| format!("invalid integer '{raw}': {err}"))?;
    if parsed <= 0 {
        return Err("must be positive".to_string());
    }
    Ok(parsed)
}

pub(super) fn parse_base_url(raw: &str, what: String) -> Result<BaseUrlSetting, anyhow::Error> {
    match raw.trim() {
        "default" => Ok(BaseUrlSetting::Default),
        "disabled" => Ok(BaseUrlSetting::Disabled),
        "" => bail!("{what} must not be empty"),
        other => Ok(BaseUrlSetting::Custom(other.to_string())),
    }
}

pub(super) fn parse_log_format(raw: &str, what: String) -> Result<LogFormat, anyhow::Error> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        _ => bail!("Invalid {what} '{raw}'. Expected one of: text, json."),
    }
}

pub(super) fn parse_missing_fonts(
    raw: &str,
    what: String,
) -> Result<MissingFontsPolicy, anyhow::Error> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "fallback" => Ok(MissingFontsPolicy::Fallback),
        "warn" => Ok(MissingFontsPolicy::Warn),
        "error" => Ok(MissingFontsPolicy::Error),
        _ => bail!("Invalid {what} '{raw}'. Expected one of: fallback, warn, error."),
    }
}

pub(super) fn parse_bool(raw: &str, what: String) -> Result<bool, anyhow::Error> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("Invalid {what} '{raw}'. Expected a boolean value."),
    }
}

pub(super) fn parse_u16(raw: &str, what: String) -> Result<u16, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

pub(super) fn parse_usize(raw: &str, what: String) -> Result<usize, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

pub(super) fn parse_u64(raw: &str, what: String) -> Result<u64, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

pub(super) fn parse_i64(raw: &str, what: String) -> Result<i64, anyhow::Error> {
    raw.trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))
}

pub(super) fn parse_positive_i64(raw: &str, what: String) -> Result<i64, anyhow::Error> {
    let parsed = parse_i64(raw, what.clone())?;
    if parsed <= 0 {
        bail!("{what} must be positive");
    }
    Ok(parsed)
}

pub(super) fn parse_nullable_string(raw: &str) -> Result<Option<String>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        Ok(Some(raw.to_string()))
    }
}

pub(super) fn parse_log_filter_value(
    raw: &str,
    what: String,
) -> Result<Option<String>, anyhow::Error> {
    let value = parse_nullable_string(raw)?;
    if let Some(ref filter) = value {
        filter
            .parse::<EnvFilter>()
            .map_err(|err| anyhow!("Invalid {what} '{filter}': {err}"))?;
    }
    Ok(value)
}

pub(super) fn parse_nullable_usize(
    raw: &str,
    what: String,
) -> Result<Option<usize>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        parse_usize(raw, what).map(Some)
    }
}

pub(super) fn parse_nullable_u16(raw: &str, what: String) -> Result<Option<u16>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        parse_u16(raw, what).map(Some)
    }
}

pub(super) fn parse_nullable_i64(raw: &str, what: String) -> Result<Option<i64>, anyhow::Error> {
    if is_null_literal(raw) {
        Ok(None)
    } else {
        parse_i64(raw, what).map(Some)
    }
}

/// Clap `value_parser` for `--num-workers`: positive-integer → `NonZeroUsize`.
/// Rejects `0` at parse time (clap surfaces the error message inline with the flag).
pub(super) fn parse_non_zero_usize_arg(raw: &str) -> Result<NonZeroUsize, String> {
    let parsed: usize = raw
        .trim()
        .parse()
        .map_err(|err| format!("invalid unsigned integer '{raw}': {err}"))?;
    NonZeroUsize::new(parsed)
        .ok_or_else(|| "must be a positive integer (>= 1)".to_string())
}

/// CLI-friendly `usize` cap: `0` is accepted as a shorthand for "no cap"
/// (→ `None`), positive values parse to `Some(NonZeroUsize)`. The `0`
/// shorthand preserves backward ergonomics; the library field itself
/// rejects a literal `NonZeroUsize(0)` (can't construct it).
pub(super) fn parse_optional_non_zero_usize(
    raw: &str,
    what: String,
) -> Result<Option<NonZeroUsize>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let value: usize = raw
        .trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))?;
    Ok(NonZeroUsize::new(value))
}

/// Same shape as `parse_optional_non_zero_usize` but for `NonZeroU64`.
pub(super) fn parse_optional_non_zero_u64(
    raw: &str,
    what: String,
) -> Result<Option<NonZeroU64>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let value: u64 = raw
        .trim()
        .parse()
        .map_err(|err| anyhow!("Invalid {what} '{raw}': {err}"))?;
    Ok(NonZeroU64::new(value))
}

/// Env-side parser for `VLC_GOOGLE_FONTS_CACHE_SIZE_MB`: accepts `null`
/// → `None`, `0` → `None` (library default), positive → `Some(NZ(n))`.
/// Keeps symmetry with the CLI flag (clap Option<u64> with `0` shorthand).
pub(super) fn parse_cache_size_mb(
    raw: &str,
    what: String,
) -> Result<Option<NonZeroU64>, anyhow::Error> {
    parse_optional_non_zero_u64(raw, what)
}

/// Env-side parser for `VLC_FONT_DIR`. Linux/macOS: colon-separated
/// list (mirrors PATH). Windows: semicolon-separated. Empty entries
/// (from leading/trailing/duplicate separators) are skipped so
/// `VLC_FONT_DIR=:/a:` produces `[/a]` rather than `["", "/a", ""]`.
/// Each entry is trimmed and tilde-expanded.
pub(super) fn parse_font_dir_list(raw: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    const SEP: char = ';';
    #[cfg(not(windows))]
    const SEP: char = ':';

    raw.split(SEP)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(expand_path)
        .collect()
}

fn is_null_literal(raw: &str) -> bool {
    raw.trim().eq_ignore_ascii_case("null")
}

#[derive(Debug)]
struct LoadedText {
    text: String,
    source_path: Option<PathBuf>,
}

fn load_text(raw: &str, input: InputKind, what: String) -> Result<LoadedText, anyhow::Error> {
    load_text_with_stdin(raw, input, what, || {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|err| anyhow!("Failed to read stdin: {err}"))?;
        Ok(text)
    })
}

fn load_text_with_stdin<F>(
    raw: &str,
    input: InputKind,
    what: String,
    read_stdin: F,
) -> Result<LoadedText, anyhow::Error>
where
    F: FnOnce() -> Result<String, anyhow::Error>,
{
    if let Some(path) = raw.strip_prefix('@') {
        if path.is_empty() {
            bail!("{what} must specify a path after '@'");
        }
        if path == "-" {
            if matches!(input, InputKind::Env) {
                bail!("{what} does not support @- from the environment");
            }
            return Ok(LoadedText {
                text: read_stdin()?,
                source_path: None,
            });
        }
        let resolved = resolve_input_path(path)?;
        let text = std::fs::read_to_string(&resolved)
            .map_err(|err| anyhow!("Failed to read {what} from {}: {err}", resolved.display()))?;
        Ok(LoadedText {
            text,
            source_path: Some(resolved),
        })
    } else {
        Ok(LoadedText {
            text: raw.to_string(),
            source_path: None,
        })
    }
}

fn resolve_input_path(raw: &str) -> Result<PathBuf, anyhow::Error> {
    let expanded = expand_path(raw);
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Ok(std::env::current_dir()?.join(expanded))
    }
}

pub(super) fn expand_path(raw: &str) -> PathBuf {
    PathBuf::from(shellexpand::tilde(raw.trim()).to_string())
}

fn parse_json_value(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<(serde_json::Value, Option<PathBuf>), anyhow::Error> {
    let loaded = load_text(raw, input, what.clone())?;
    let value = serde_json::from_str::<serde_json::Value>(&loaded.text).map_err(|err| {
        anyhow!(
            "Invalid JSON for {what}{}: {err}",
            loaded
                .source_path
                .as_ref()
                .map(|path| format!(" in {}", path.display()))
                .unwrap_or_default()
        )
    })?;
    Ok((value, loaded.source_path))
}

pub(super) fn parse_string_vec(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<Vec<String>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let (value, _) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }
    match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(text) => Ok(text),
                _ => bail!("{what} must be a JSON array of strings"),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => bail!("{what} must be a JSON array of strings"),
    }
}

pub(super) fn parse_json_map(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<HashMap<String, serde_json::Value>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let (value, _) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|err| anyhow!("{what} must be a JSON object: {err}"))
}

pub(super) fn parse_vega_plugins(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<Vec<String>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }
    let (value, source_path) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }
    let mut plugins: Vec<String> = match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(text) => Ok(text),
                _ => bail!("{what} must be a JSON array of strings"),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => bail!("{what} must be a JSON array of strings"),
    };

    if let Some(path) = source_path {
        if let Some(dir) = path.parent() {
            resolve_plugin_paths_relative_to(dir, &mut plugins);
        }
    }

    Ok(Some(plugins))
}

fn resolve_plugin_paths_relative_to(dir: &Path, plugins: &mut [String]) {
    for plugin in plugins.iter_mut() {
        if plugin.contains("://")
            || plugin.contains('\n')
            || plugin.starts_with("export")
            || plugin.starts_with("import")
        {
            continue;
        }

        let path = Path::new(plugin.as_str());
        if path.is_relative() {
            let normalized: PathBuf = dir.join(path).components().collect();
            *plugin = normalized.to_string_lossy().to_string();
        }
    }
}

pub(super) fn parse_format_locale(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<FormatLocale>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }

    if raw.starts_with('@') {
        let (value, _) = parse_json_value(raw, input, what.clone())?;
        return parse_locale_value(value, what).map(Some);
    }

    let trimmed = raw.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('"') {
        let value = serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|err| anyhow!("Invalid JSON for {what}: {err}"))?;
        if value.is_null() {
            return Ok(None);
        }
        return parse_locale_value(value, what).map(Some);
    }

    Ok(Some(FormatLocale::Name(raw.to_string())))
}

pub(super) fn parse_time_format_locale(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<TimeFormatLocale>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }

    if raw.starts_with('@') {
        let (value, _) = parse_json_value(raw, input, what.clone())?;
        return parse_time_locale_value(value, what).map(Some);
    }

    let trimmed = raw.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('"') {
        let value = serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|err| anyhow!("Invalid JSON for {what}: {err}"))?;
        if value.is_null() {
            return Ok(None);
        }
        return parse_time_locale_value(value, what).map(Some);
    }

    Ok(Some(TimeFormatLocale::Name(raw.to_string())))
}

fn parse_locale_value(
    value: serde_json::Value,
    what: String,
) -> Result<FormatLocale, anyhow::Error> {
    match value {
        serde_json::Value::String(name) => Ok(FormatLocale::Name(name)),
        serde_json::Value::Object(_) => Ok(FormatLocale::Object(value)),
        _ => bail!("{what} must be a locale name string or JSON object"),
    }
}

fn parse_time_locale_value(
    value: serde_json::Value,
    what: String,
) -> Result<TimeFormatLocale, anyhow::Error> {
    match value {
        serde_json::Value::String(name) => Ok(TimeFormatLocale::Name(name)),
        serde_json::Value::Object(_) => Ok(TimeFormatLocale::Object(value)),
        _ => bail!("{what} must be a locale name string or JSON object"),
    }
}

pub(super) fn parse_google_fonts(
    raw: &str,
    input: InputKind,
    what: String,
) -> Result<Option<Vec<GoogleFontRequest>>, anyhow::Error> {
    if is_null_literal(raw) {
        return Ok(None);
    }

    let (value, _) = parse_json_value(raw, input, what.clone())?;
    if value.is_null() {
        return Ok(None);
    }

    match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                serde_json::Value::String(spec) => parse_google_font_arg(&spec),
                serde_json::Value::Object(_) => {
                    serde_json::from_value(item).map_err(|err| anyhow!("{what}: {err}"))
                }
                _ => bail!("{what} must be a JSON array of strings or objects"),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => bail!("{what} must be a JSON array of strings or objects"),
    }
}

fn parse_google_font_arg(s: &str) -> Result<GoogleFontRequest, anyhow::Error> {
    let Some((family, variants_str)) = s.split_once(':') else {
        return Ok(GoogleFontRequest {
            family: s.to_string(),
            variants: None,
        });
    };

    let mut variants = Vec::new();
    for token in variants_str.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (weight_str, style) = if let Some(weight) = token.strip_suffix("italic") {
            (weight, FontStyle::Italic)
        } else {
            (token, FontStyle::Normal)
        };
        let weight: u16 = weight_str.parse().map_err(|_| {
            anyhow!(
                "Invalid font variant '{token}' in google font '{s}'. Expected format: 400, 700italic, etc."
            )
        })?;
        variants.push(VariantRequest { weight, style });
    }

    Ok(GoogleFontRequest {
        family: family.to_string(),
        variants: if variants.is_empty() {
            None
        } else {
            Some(variants)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_string_vec_supports_inline_file_and_null() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "[\"https://example.com/\",\"/data/\"]").unwrap();

        assert_eq!(
            parse_string_vec(
                "[\"https://example.com/\"]",
                InputKind::Cli,
                "test".to_string()
            )
            .unwrap(),
            Some(vec!["https://example.com/".to_string()])
        );
        assert_eq!(
            parse_string_vec(
                &format!("@{}", file.path().display()),
                InputKind::Cli,
                "test".to_string()
            )
            .unwrap(),
            Some(vec![
                "https://example.com/".to_string(),
                "/data/".to_string()
            ])
        );
        assert_eq!(
            parse_string_vec("null", InputKind::Cli, "test".to_string()).unwrap(),
            None
        );
    }

    #[test]
    fn test_load_text_supports_cli_stdin_but_not_env() {
        let loaded = load_text_with_stdin("@-", InputKind::Cli, "test".to_string(), || {
            Ok("[1,2,3]".to_string())
        })
        .unwrap();
        assert_eq!(loaded.text, "[1,2,3]");

        let err = load_text_with_stdin("@-", InputKind::Env, "test".to_string(), || {
            Ok(String::new())
        })
        .unwrap_err();
        assert!(err.to_string().contains("does not support @-"));
    }

    #[test]
    fn test_parse_google_fonts_accepts_shorthand_and_objects() {
        let parsed = parse_google_fonts(
            r#"["Roboto:400,700italic",{"family":"Inter","variants":[{"weight":400,"style":"Normal"}]}]"#,
            InputKind::Cli,
            "google_fonts".to_string(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].family, "Roboto");
        assert_eq!(parsed[0].variants.as_ref().unwrap().len(), 2);
        assert_eq!(parsed[1].family, "Inter");
    }

    #[test]
    fn test_parse_google_fonts_rejects_invalid_json_and_missing_files() {
        assert!(parse_google_fonts("{", InputKind::Cli, "google_fonts".to_string()).is_err());
        assert!(parse_google_fonts(
            "@/definitely/missing.json",
            InputKind::Cli,
            "google_fonts".to_string()
        )
        .is_err());
    }

    #[test]
    fn test_parse_locale_supports_inline_name_object_and_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"decimal":",","thousands":"."}}"#).unwrap();

        let name = parse_format_locale("de-DE", InputKind::Cli, "format".to_string())
            .unwrap()
            .unwrap();
        assert!(matches!(name, FormatLocale::Name(ref n) if n == "de-DE"));

        let object = parse_format_locale(
            r#"{"decimal":",","thousands":"."}"#,
            InputKind::Cli,
            "format".to_string(),
        )
        .unwrap()
        .unwrap();
        assert!(matches!(object, FormatLocale::Object(_)));

        let from_file = parse_format_locale(
            &format!("@{}", file.path().display()),
            InputKind::Cli,
            "format".to_string(),
        )
        .unwrap()
        .unwrap();
        assert!(matches!(from_file, FormatLocale::Object(_)));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_parse_socket_path_accepts_absolute_path() {
        let parsed = parse_socket_path_arg("/tmp/vlc.sock").unwrap();
        assert_eq!(parsed, PathBuf::from("/tmp/vlc.sock"));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_parse_socket_path_rejects_relative() {
        let err = parse_socket_path_arg("rel/path").unwrap_err();
        assert!(err.contains("must be absolute"), "got: {err}");
    }

    #[cfg(not(windows))]
    #[test]
    fn test_parse_socket_path_rejects_empty() {
        let err = parse_socket_path_arg("").unwrap_err();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[cfg(not(windows))]
    #[test]
    fn test_parse_socket_path_expands_tilde() {
        // When $HOME is set, tilde expansion should produce an
        // absolute path rooted at $HOME. We don't assert the exact
        // expansion (which depends on the test-runner env), just that
        // the result is absolute.
        let parsed = parse_socket_path_arg("~/vlc.sock").unwrap();
        assert!(parsed.is_absolute(), "got: {}", parsed.display());
    }

    #[cfg(windows)]
    #[test]
    fn test_parse_socket_path_rejected_on_windows() {
        let err = parse_socket_path_arg("/tmp/x.sock").unwrap_err();
        assert!(err.contains("not supported on Windows"), "got: {err}");
        assert!(err.contains("--port"), "got: {err}");
    }

    #[test]
    fn test_parse_socket_mode_accepts_common_values() {
        assert_eq!(parse_socket_mode_arg("600").unwrap(), 0o600);
        assert_eq!(parse_socket_mode_arg("0600").unwrap(), 0o600);
        assert_eq!(parse_socket_mode_arg("0o660").unwrap(), 0o660);
        assert_eq!(parse_socket_mode_arg("0770").unwrap(), 0o770);
    }

    #[test]
    fn test_parse_socket_mode_rejects_other_bits() {
        let err = parse_socket_mode_arg("666").unwrap_err();
        assert!(err.contains("other"), "got: {err}");
    }

    #[test]
    fn test_parse_socket_mode_rejects_zero() {
        let err = parse_socket_mode_arg("0").unwrap_err();
        assert!(err.contains("0o000"), "got: {err}");
    }

    #[test]
    fn test_parse_socket_mode_rejects_non_octal() {
        assert!(parse_socket_mode_arg("9").is_err());
        assert!(parse_socket_mode_arg("garbage").is_err());
        assert!(parse_socket_mode_arg("").is_err());
    }

    #[test]
    fn test_parse_non_zero_usize_arg_accepts_positive() {
        assert_eq!(parse_non_zero_usize_arg("1").unwrap(), NonZeroUsize::new(1).unwrap());
        assert_eq!(parse_non_zero_usize_arg("42").unwrap(), NonZeroUsize::new(42).unwrap());
    }

    #[test]
    fn test_parse_non_zero_usize_arg_rejects_zero() {
        let err = parse_non_zero_usize_arg("0").unwrap_err();
        assert!(err.contains("positive"), "got: {err}");
    }

    #[test]
    fn test_parse_non_zero_usize_arg_rejects_non_numeric() {
        assert!(parse_non_zero_usize_arg("abc").is_err());
        assert!(parse_non_zero_usize_arg("").is_err());
    }

    #[test]
    fn test_parse_optional_non_zero_usize_zero_shorthand_is_none() {
        assert_eq!(
            parse_optional_non_zero_usize("0", "what".to_string()).unwrap(),
            None,
        );
        assert_eq!(
            parse_optional_non_zero_usize("null", "what".to_string()).unwrap(),
            None,
        );
        assert_eq!(
            parse_optional_non_zero_usize("512", "what".to_string()).unwrap(),
            NonZeroUsize::new(512),
        );
    }

    #[test]
    fn test_parse_optional_non_zero_u64_zero_shorthand_is_none() {
        assert_eq!(
            parse_optional_non_zero_u64("0", "what".to_string()).unwrap(),
            None,
        );
        assert_eq!(
            parse_optional_non_zero_u64("100", "what".to_string()).unwrap(),
            NonZeroU64::new(100),
        );
    }

    #[test]
    fn test_parse_font_dir_list_splits_on_separator() {
        #[cfg(not(windows))]
        {
            assert_eq!(
                parse_font_dir_list("/a:/b:/c"),
                vec![
                    PathBuf::from("/a"),
                    PathBuf::from("/b"),
                    PathBuf::from("/c"),
                ],
            );
            assert_eq!(
                parse_font_dir_list(":/a::/b:"),
                vec![PathBuf::from("/a"), PathBuf::from("/b")],
            );
            assert_eq!(parse_font_dir_list(""), Vec::<PathBuf>::new());
        }
        #[cfg(windows)]
        {
            assert_eq!(
                parse_font_dir_list("C:/a;C:/b"),
                vec![PathBuf::from("C:/a"), PathBuf::from("C:/b")],
            );
        }
    }

    #[test]
    fn test_parse_vega_plugins_resolves_relative_paths_from_fragment_file() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_path = dir.path().join("plugin.js");
        std::fs::write(&plugin_path, "export default function(vega) {}").unwrap();
        let fragment_path = dir.path().join("plugins.json");
        std::fs::write(&fragment_path, "[\"./plugin.js\"]").unwrap();

        let parsed = parse_vega_plugins(
            &format!("@{}", fragment_path.display()),
            InputKind::Cli,
            "vega_plugins".to_string(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed, vec![plugin_path.to_string_lossy().to_string()]);
    }
}

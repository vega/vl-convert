use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroU64;
use utoipa::ToSchema;
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    VlcConfig,
};
use vl_convert_rs::DEFAULT_VL_VERSION;

use crate::util::CommonOptsInput;

/// Serde helper for `ConfigPatch` tri-state fields: absent field => `None`,
/// explicit `null` => `Some(None)`, present value => `Some(Some(v))`.
///
/// Serde only calls this helper when the field is present, so wrapping
/// `Option::<T>::deserialize` in an outer `Some` preserves the
/// absent/null/value distinction.
pub(crate) mod double_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}

/// Serde helper for required-but-nullable fields. Unlike Serde's default
/// `Option<T>` handling, a missing field is rejected while an explicit null
/// still deserializes to `None`.
pub(crate) mod required_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer)
    }
}

fn base_url_schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
    use utoipa::openapi::schema::{ObjectBuilder, OneOfBuilder, Type};

    OneOfBuilder::new()
        .item(ObjectBuilder::new().schema_type(Type::Boolean))
        .item(ObjectBuilder::new().schema_type(Type::String))
        .description(Some(
            "true selects the Vega datasets default, false disables relative data loading, and a string supplies a custom URL or filesystem path",
        ))
        .into()
}

/// Wire-shape adapter for `BaseUrlSetting`: accepts the `bool | string`
/// form `VlcConfig::Serialize` emits (`true` → `Default`, `false` →
/// `Disabled`, any string → `Custom(s)`) so `GET /admin/config`
/// round-trips losslessly through `PUT` / `PATCH`.
///
/// Strings always map to `BaseUrlSetting::Custom(s)`. The boolean shapes
/// represent `Default` and `Disabled`; keeping strings reserved for custom
/// URLs makes GET -> PUT/PATCH round-trips preserve `Custom("default")` and
/// `Custom("disabled")`.
pub(crate) fn deserialize_base_url_view<'de, D>(deserializer: D) -> Result<BaseUrlSetting, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Bool(true) => Ok(BaseUrlSetting::Default),
        serde_json::Value::Bool(false) => Ok(BaseUrlSetting::Disabled),
        serde_json::Value::String(s) => Ok(BaseUrlSetting::Custom(s)),
        other => Err(D::Error::custom(format!(
            "base_url must be true, false, or a string, got {other}"
        ))),
    }
}

/// Same adapter as [`deserialize_base_url_view`], composed with the
/// [`double_option`] tri-state so `ConfigPatch.base_url` can accept the
/// bool-or-string wire shape while keeping null-vs-absent distinction.
/// Strings always map to `BaseUrlSetting::Custom(s)` (see the sibling
/// adapter's doc-comment for rationale).
pub(crate) fn deserialize_base_url_view_double_option<'de, D>(
    deserializer: D,
) -> Result<Option<Option<BaseUrlSetting>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Bool(true) => Ok(Some(Some(BaseUrlSetting::Default))),
        serde_json::Value::Bool(false) => Ok(Some(Some(BaseUrlSetting::Disabled))),
        serde_json::Value::String(s) => Ok(Some(Some(BaseUrlSetting::Custom(s)))),
        other => Err(D::Error::custom(format!(
            "base_url must be true, false, a string, or null, got {other}"
        ))),
    }
}

fn default_vl_version() -> String {
    DEFAULT_VL_VERSION.to_string()
}

macro_rules! impl_common_opts_input {
    ($ty:ty) => {
        impl CommonOptsInput for $ty {
            fn format_locale(&self) -> &Option<serde_json::Value> {
                &self.format_locale
            }
            fn time_format_locale(&self) -> &Option<serde_json::Value> {
                &self.time_format_locale
            }
            fn google_fonts(&self) -> &Option<Vec<String>> {
                &self.google_fonts
            }
            fn vega_plugin(&self) -> &Option<String> {
                &self.vega_plugin
            }
            fn config(&self) -> &Option<serde_json::Value> {
                &self.config
            }
            fn background(&self) -> &Option<String> {
                &self.background
            }
            fn width(&self) -> Option<f32> {
                self.width
            }
            fn height(&self) -> Option<f32> {
                self.height
            }
        }
    };
}

impl_common_opts_input!(VegaliteCommon);
impl_common_opts_input!(VegaCommon);

/// Fields common to all Vega-Lite conversion requests.
#[derive(Debug, Deserialize, ToSchema)]
pub struct VegaliteCommon {
    /// Vega-Lite specification as a JSON object.
    pub spec: serde_json::Value,
    /// Vega-Lite version. One of 5.8, 5.14, 5.15, 5.16, 5.17, 5.20, 5.21, 6.1, 6.4.
    #[serde(default = "default_vl_version")]
    pub vl_version: String,
    /// Named theme from vega-themes (e.g., "dark").
    pub theme: Option<String>,
    /// Vega-Lite config object.
    pub config: Option<serde_json::Value>,
    /// Background color (applied to spec.background).
    pub background: Option<String>,
    /// Override spec width.
    pub width: Option<f32>,
    /// Override spec height.
    pub height: Option<f32>,
    /// d3-format locale (name or inline object).
    pub format_locale: Option<serde_json::Value>,
    /// d3-time-format locale (name or inline object).
    pub time_format_locale: Option<serde_json::Value>,
    /// Google Fonts to register (e.g., ["Roboto", "Pacifico:400,700italic"]).
    pub google_fonts: Option<Vec<String>>,
    /// Per-request Vega plugin (inline ESM or URL).
    pub vega_plugin: Option<String>,
}

/// Fields common to all Vega conversion requests.
#[derive(Debug, Deserialize, ToSchema)]
pub struct VegaCommon {
    /// Vega specification as a JSON object.
    pub spec: serde_json::Value,
    /// Vega config object merged with the specification's config.
    pub config: Option<serde_json::Value>,
    /// Background color applied to the specification.
    pub background: Option<String>,
    /// Chart width override.
    pub width: Option<f32>,
    /// Chart height override.
    pub height: Option<f32>,
    /// d3-format locale (name or inline object).
    pub format_locale: Option<serde_json::Value>,
    /// d3-time-format locale (name or inline object).
    pub time_format_locale: Option<serde_json::Value>,
    /// Google Fonts to register.
    pub google_fonts: Option<Vec<String>>,
    /// Per-request Vega plugin (inline ESM or URL).
    pub vega_plugin: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteVegaRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteSvgRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Bundle fonts and images into a self-contained SVG.
    #[serde(default)]
    pub bundle: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegalitePngRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// Pixels per inch.
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteJpegRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// JPEG quality (0-100).
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegalitePdfRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteHtmlRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Bundle Vega JS inline instead of loading from CDN.
    #[serde(default)]
    pub bundle: bool,
    /// Renderer: "svg", "canvas", or "hybrid".
    pub renderer: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteScenegraphRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteUrlRequest {
    /// Vega-Lite specification as a JSON object.
    pub spec: serde_json::Value,
    /// Open in fullscreen view in the Vega Editor.
    #[serde(default)]
    pub fullscreen: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaSvgRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    /// Bundle fonts and images into a self-contained SVG.
    #[serde(default)]
    pub bundle: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaPngRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// Pixels per inch. Combines with scale as `scale * ppi / 72`.
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaJpegRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// JPEG quality from 0 through 100.
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaPdfRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaHtmlRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    /// Bundle browser JavaScript dependencies into the HTML document.
    #[serde(default)]
    pub bundle: bool,
    /// Browser renderer: `svg`, `canvas`, or `hybrid`.
    pub renderer: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaScenegraphRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaUrlRequest {
    /// Vega specification as a JSON object.
    pub spec: serde_json::Value,
    /// Open the full-screen view in the Vega Editor.
    #[serde(default)]
    pub fullscreen: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgPngRequest {
    /// SVG markup string.
    pub svg: String,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// Pixels per inch. Combines with scale as `scale * ppi / 72`.
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgJpegRequest {
    /// SVG markup string.
    pub svg: String,
    /// Image scale factor.
    pub scale: Option<f32>,
    /// JPEG quality from 0 through 100.
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgPdfRequest {
    /// SVG markup string.
    pub svg: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaliteFontsRequest {
    #[serde(flatten)]
    pub common: VegaliteCommon,
    /// Include @font-face CSS blocks with embedded base64 WOFF2 data.
    #[serde(default)]
    pub include_font_face: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaFontsRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    /// Include @font-face CSS blocks with embedded base64 WOFF2 data.
    #[serde(default)]
    pub include_font_face: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FontVariantResponse {
    /// CSS font-weight, such as "400" or "700".
    pub weight: String,
    /// CSS font-style, usually "normal" or "italic".
    pub style: String,
    /// Embedded `@font-face` CSS when requested.
    pub font_face: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FontSourceResponse {
    /// Source discriminator: "google" or "local".
    #[serde(rename = "type")]
    pub source_type: String,
    /// Google Fonts font ID for Google-hosted fonts.
    pub font_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FontInfoResponse {
    /// Font family name.
    pub name: String,
    /// Where the font data originates.
    pub source: FontSourceResponse,
    /// Weight/style variants used by the chart.
    pub variants: Vec<FontVariantResponse>,
    /// Google Fonts CSS2 stylesheet URL for Google-hosted fonts.
    pub url: Option<String>,
    /// HTML stylesheet link tag for Google-hosted fonts.
    pub link_tag: Option<String>,
    /// CSS import rule for Google-hosted fonts.
    pub import_rule: Option<String>,
}

fn default_vl_version_bundle() -> String {
    DEFAULT_VL_VERSION.to_string()
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct BundleQuery {
    /// Vega-Lite version for the bundle (default "6.4").
    pub vl_version: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleSnippetRequest {
    /// JavaScript snippet to bundle (e.g. `import * as vega from "vega"; ...`).
    pub snippet: String,
    /// Vega-Lite version for module resolution.
    #[serde(default = "default_vl_version_bundle")]
    pub vl_version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Human-readable error message when detailed errors are enabled.
    pub error: String,
}

// Admin `/config` DTOs.
//
// Patch fields use `Option<Option<T>>` where null must be distinct from
// absence. Non-nullable `VlcConfig` fields reject explicit nulls with 400.

/// Partial converter configuration update. An omitted field keeps its current
/// value. A supplied value replaces it. `null` clears a nullable field and is
/// rejected for other fields. Unknown fields are rejected.
#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigPatch {
    // `Option<Option<T>>` is a serde-layer tri-state (absent / null / value)
    // wired up via `double_option::deserialize`; utoipa collapses it to
    // `Option<T>` in the published schema. Absent → preserve current,
    // null → clear (or 400 for non-nullable fields), value → set.
    /// Number of persistent conversion workers.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = u64, minimum = 1)]
    pub num_workers: Option<Option<NonZeroU64>>,
    #[serde(default, deserialize_with = "deserialize_base_url_view_double_option")]
    #[schema(schema_with = base_url_schema)]
    pub base_url: Option<Option<BaseUrlSetting>>,
    /// URL and filesystem prefixes that chart data and images may load.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Vec<String>)]
    pub allowed_base_urls: Option<Option<Vec<String>>>,
    /// Whether missing fonts can be downloaded automatically from Google Fonts.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = bool)]
    pub auto_google_fonts: Option<Option<bool>>,
    /// Whether SVG and HTML output embeds locally available fonts.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = bool)]
    pub embed_local_fonts: Option<Option<bool>>,
    /// Whether embedded fonts contain only the glyphs used by the output.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = bool)]
    pub subset_fonts: Option<Option<bool>>,
    /// Action when a first-choice font is unavailable: ignore, warn, or error.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = String)]
    pub missing_fonts: Option<Option<MissingFontsPolicy>>,
    /// Google Fonts families and variants to register for every conversion.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Vec<Object>)]
    pub google_fonts: Option<Option<Vec<GoogleFontRequest>>>,
    /// Variant count after which additional Google Font families are skipped.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, minimum = 1)]
    pub google_font_variant_threshold: Option<Option<NonZeroU64>>,
    /// Maximum JavaScript heap size per persistent worker, in megabytes.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, minimum = 1)]
    pub max_v8_heap_size_mb: Option<Option<NonZeroU64>>,
    /// Maximum JavaScript execution time per conversion, in seconds.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, minimum = 1)]
    pub max_v8_execution_time_secs: Option<Option<NonZeroU64>>,
    /// Whether to request JavaScript garbage collection after each conversion.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = bool)]
    pub gc_after_conversion: Option<Option<bool>>,
    /// Vega plugin modules loaded for every conversion.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Vec<String>)]
    pub vega_plugins: Option<Option<Vec<String>>>,
    /// HTTP import domains allowed for configured plugins.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Vec<String>)]
    pub plugin_import_domains: Option<Option<Vec<String>>>,
    /// Whether public conversion requests may provide a Vega plugin.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = bool)]
    pub allow_per_request_plugins: Option<Option<bool>>,
    /// Maximum concurrent temporary workers for per-request plugins.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, minimum = 1)]
    pub max_ephemeral_workers: Option<Option<NonZeroU64>>,
    /// Whether public requests may choose Google Fonts settings.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = bool)]
    pub allow_google_fonts: Option<Option<bool>>,
    /// HTTP import domains allowed for per-request plugins.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Vec<String>)]
    pub per_request_plugin_import_domains: Option<Option<Vec<String>>>,
    /// Default theme for Vega-Lite conversions.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>, nullable)]
    pub default_theme: Option<Option<String>>,
    /// Default d3-format locale.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable)]
    pub default_format_locale: Option<Option<FormatLocale>>,
    /// Default d3-time-format locale.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable)]
    pub default_time_format_locale: Option<Option<TimeFormatLocale>>,
    /// Custom Vega themes keyed by theme name.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Object)]
    pub themes: Option<Option<HashMap<String, serde_json::Value>>>,
}

/// Complete converter configuration replacement. Every field must be present.
/// Nullable fields accept `null`. Unknown fields and `null` on other fields
/// are rejected.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigReplace {
    /// Number of persistent conversion workers.
    #[schema(value_type = u64, minimum = 1)]
    pub num_workers: NonZeroU64,
    #[schema(schema_with = base_url_schema)]
    #[serde(deserialize_with = "deserialize_base_url_view")]
    pub base_url: BaseUrlSetting,
    /// URL and filesystem prefixes that chart data and images may load.
    pub allowed_base_urls: Vec<String>,
    /// Whether missing fonts can be downloaded automatically from Google Fonts.
    pub auto_google_fonts: bool,
    /// Whether SVG and HTML output embeds locally available fonts.
    pub embed_local_fonts: bool,
    /// Whether embedded fonts contain only the glyphs used by the output.
    pub subset_fonts: bool,
    /// Action when a first-choice font is unavailable: ignore, warn, or error.
    #[schema(value_type = String)]
    pub missing_fonts: MissingFontsPolicy,
    /// Google Fonts families and variants to register for every conversion.
    #[schema(value_type = Vec<Object>)]
    pub google_fonts: Vec<GoogleFontRequest>,
    /// Variant count after which additional Google Font families are skipped.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, required = true, minimum = 1)]
    pub google_font_variant_threshold: Option<NonZeroU64>,
    /// Maximum JavaScript heap size per persistent worker, in megabytes.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, required = true, minimum = 1)]
    pub max_v8_heap_size_mb: Option<NonZeroU64>,
    /// Maximum JavaScript execution time per conversion, in seconds.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, required = true, minimum = 1)]
    pub max_v8_execution_time_secs: Option<NonZeroU64>,
    /// Whether to request JavaScript garbage collection after each conversion.
    pub gc_after_conversion: bool,
    /// Vega plugin modules loaded for every conversion.
    pub vega_plugins: Vec<String>,
    /// HTTP import domains allowed for configured plugins.
    pub plugin_import_domains: Vec<String>,
    /// Whether public conversion requests may provide a Vega plugin.
    pub allow_per_request_plugins: bool,
    /// Maximum concurrent temporary workers for per-request plugins.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable, required = true, minimum = 1)]
    pub max_ephemeral_workers: Option<NonZeroU64>,
    /// Whether public requests may choose Google Fonts settings.
    pub allow_google_fonts: bool,
    /// HTTP import domains allowed for per-request plugins.
    pub per_request_plugin_import_domains: Vec<String>,
    /// Default theme for Vega-Lite conversions.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(nullable, required = true)]
    pub default_theme: Option<String>,
    /// Default d3-format locale.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable, required = true)]
    pub default_format_locale: Option<FormatLocale>,
    /// Default d3-time-format locale.
    #[serde(deserialize_with = "required_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable, required = true)]
    pub default_time_format_locale: Option<TimeFormatLocale>,
    /// Custom Vega themes keyed by theme name.
    #[schema(value_type = Object)]
    pub themes: HashMap<String, serde_json::Value>,
}

impl From<ConfigReplace> for VlcConfig {
    fn from(r: ConfigReplace) -> Self {
        VlcConfig {
            num_workers: r.num_workers,
            base_url: r.base_url,
            allowed_base_urls: r.allowed_base_urls,
            auto_google_fonts: r.auto_google_fonts,
            embed_local_fonts: r.embed_local_fonts,
            subset_fonts: r.subset_fonts,
            missing_fonts: r.missing_fonts,
            google_fonts: r.google_fonts,
            google_font_variant_threshold: r.google_font_variant_threshold,
            max_v8_heap_size_mb: r.max_v8_heap_size_mb,
            max_v8_execution_time_secs: r.max_v8_execution_time_secs,
            gc_after_conversion: r.gc_after_conversion,
            vega_plugins: r.vega_plugins,
            plugin_import_domains: r.plugin_import_domains,
            allow_per_request_plugins: r.allow_per_request_plugins,
            max_ephemeral_workers: r.max_ephemeral_workers,
            allow_google_fonts: r.allow_google_fonts,
            per_request_plugin_import_domains: r.per_request_plugin_import_domains,
            default_theme: r.default_theme,
            default_format_locale: r.default_format_locale,
            default_time_format_locale: r.default_time_format_locale,
            themes: r.themes,
        }
    }
}

/// Successful GET / PATCH / PUT / DELETE response body for `/admin/config`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct ConfigView {
    /// Configuration in effect when the server started. DELETE restores it.
    #[schema(value_type = ConfigReplace)]
    pub baseline: VlcConfig,
    /// Configuration used for new conversion requests.
    #[schema(value_type = ConfigReplace)]
    pub effective: VlcConfig,
    /// Number that increases after each configuration change.
    pub generation: u64,
}

/// Request body for `POST /admin/config/fonts/directories`. Appends one path;
/// use `PUT /admin/config/fonts/directories` to replace the full list.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FontDirRequest {
    /// Absolute filesystem path of the directory to register.
    #[schema(value_type = String)]
    pub path: std::path::PathBuf,
}

/// Request body for `PUT /admin/config/fonts/directories`. Replaces the full
/// directory list. Pass `[]` to clear it.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct FontDirReplace {
    /// Absolute filesystem paths to register, in order.
    #[schema(value_type = Vec<String>)]
    pub paths: Vec<std::path::PathBuf>,
}

/// Request body for `PUT /admin/config/fonts/cache_size`. `null` restores the
/// default capacity.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct CacheSizeReplace {
    /// Cache capacity in megabytes. `null` restores the default.
    #[schema(value_type = Option<u64>, nullable, minimum = 1)]
    pub max_size_mb: Option<NonZeroU64>,
}

/// Machine-readable category for one field validation error.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq, utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) enum FieldErrorCode {
    #[serde(rename = "NON_NULLABLE")]
    NonNullable,
    #[serde(rename = "OUT_OF_RANGE")]
    OutOfRange,
    #[serde(rename = "INVALID_TYPE")]
    InvalidType,
    #[serde(rename = "CROSS_FIELD_INVARIANT")]
    CrossFieldInvariant,
}

/// Single field-level validation error entry.
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub(crate) struct FieldError {
    /// JSON field path that failed validation.
    pub path: String,
    pub code: FieldErrorCode,
    /// Human-readable explanation of the failure.
    pub message: String,
}

/// Response body when a proposed converter configuration fails validation.
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub(crate) struct ConfigValidationError {
    /// Overall validation error message.
    pub error: String,
    /// Validation failures grouped by configuration field.
    pub field_errors: Vec<FieldError>,
}

/// Response body for a malformed configuration request.
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub(crate) enum ConfigBadRequestResponse {
    Error(ErrorResponse),
    Validation(ConfigValidationError),
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct DrainTimeoutResponse {
    /// Human-readable timeout message.
    pub error: String,
    /// Number of requests still running when the drain timed out.
    pub in_flight: usize,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct FontCacheSizeView {
    /// Active Google Fonts cache capacity in megabytes.
    #[schema(minimum = 1)]
    pub max_size_mb: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_view_accepts_true() {
        let v: BaseUrlSetting = serde_json::from_value(serde_json::json!(true))
            .ok()
            .or_else(|| {
                deserialize_base_url_view(&mut serde_json::Deserializer::from_str("true")).ok()
            })
            .expect("true must parse");
        assert_eq!(v, BaseUrlSetting::Default);
    }

    fn parse_base_url(src: &str) -> BaseUrlSetting {
        let mut de = serde_json::Deserializer::from_str(src);
        deserialize_base_url_view(&mut de).expect("parse")
    }

    fn parse_base_url_tri(src: &str) -> Option<Option<BaseUrlSetting>> {
        let mut de = serde_json::Deserializer::from_str(src);
        deserialize_base_url_view_double_option(&mut de).expect("parse")
    }

    #[test]
    fn base_url_view_true_maps_to_default() {
        assert_eq!(parse_base_url("true"), BaseUrlSetting::Default);
    }

    #[test]
    fn base_url_view_false_maps_to_disabled() {
        assert_eq!(parse_base_url("false"), BaseUrlSetting::Disabled);
    }

    #[test]
    fn base_url_view_string_default_is_custom() {
        // Strings always map to Custom, including "default" and "disabled".
        assert_eq!(
            parse_base_url(r#""default""#),
            BaseUrlSetting::Custom("default".to_string())
        );
    }

    #[test]
    fn base_url_view_string_disabled_is_custom() {
        assert_eq!(
            parse_base_url(r#""disabled""#),
            BaseUrlSetting::Custom("disabled".to_string())
        );
    }

    #[test]
    fn base_url_view_custom_string() {
        assert_eq!(
            parse_base_url(r#""https://example.com/""#),
            BaseUrlSetting::Custom("https://example.com/".to_string())
        );
    }

    #[test]
    fn base_url_view_tri_string_default_is_custom() {
        assert_eq!(
            parse_base_url_tri(r#""default""#),
            Some(Some(BaseUrlSetting::Custom("default".to_string())))
        );
    }

    #[test]
    fn base_url_view_tri_null_is_some_none() {
        assert_eq!(parse_base_url_tri("null"), Some(None));
    }

    #[test]
    fn base_url_view_tri_true_is_some_default() {
        assert_eq!(
            parse_base_url_tri("true"),
            Some(Some(BaseUrlSetting::Default))
        );
    }

    #[test]
    fn base_url_view_tri_false_is_some_disabled() {
        assert_eq!(
            parse_base_url_tri("false"),
            Some(Some(BaseUrlSetting::Disabled))
        );
    }

    /// Build a complete `ConfigReplace` body, optionally overriding or adding
    /// fields.
    fn put_body(extra: &[(&str, serde_json::Value)]) -> serde_json::Value {
        let mut body = serde_json::json!({
            "num_workers": 1,
            "base_url": true,
            "allowed_base_urls": ["http:", "https:"],
            "auto_google_fonts": false,
            "embed_local_fonts": false,
            "subset_fonts": true,
            "missing_fonts": "fallback",
            "google_fonts": [],
            "google_font_variant_threshold": null,
            "max_v8_heap_size_mb": null,
            "max_v8_execution_time_secs": null,
            "gc_after_conversion": false,
            "vega_plugins": [],
            "plugin_import_domains": [],
            "allow_per_request_plugins": false,
            "max_ephemeral_workers": 2,
            "allow_google_fonts": false,
            "per_request_plugin_import_domains": [],
            "default_theme": null,
            "default_format_locale": null,
            "default_time_format_locale": null,
            "themes": {},
        });
        let obj = body.as_object_mut().unwrap();
        for (k, v) in extra {
            obj.insert((*k).to_string(), v.clone());
        }
        body
    }

    #[test]
    fn config_replace_round_trips_base_url_view_shapes() {
        // `VlcConfigView` emits `base_url: true | false` for the
        // Default / Disabled enum variants; PUT must accept both.
        let cases = [
            (serde_json::Value::Bool(true), BaseUrlSetting::Default),
            (serde_json::Value::Bool(false), BaseUrlSetting::Disabled),
        ];
        for (json_value, expected) in cases {
            let body = put_body(&[("base_url", json_value.clone())]);
            let replace: ConfigReplace = serde_json::from_value(body)
                .unwrap_or_else(|e| panic!("base_url={json_value} must parse: {e}"));
            assert_eq!(replace.base_url, expected);
        }
    }

    #[test]
    fn config_replace_rejects_cache_dir_field() {
        // `google_fonts_cache_dir` is read-only system state surfaced on
        // `/infoz`, not part of the writable config DTO. `ConfigReplace`
        // must reject it via `deny_unknown_fields`.
        let body = put_body(&[("google_fonts_cache_dir", serde_json::json!("/tmp/cache"))]);
        let err = serde_json::from_value::<ConfigReplace>(body)
            .expect_err("google_fonts_cache_dir is not a writable field");
        assert!(
            err.to_string().contains("google_fonts_cache_dir"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn config_replace_still_rejects_unknown_field() {
        let body = put_body(&[("totally_made_up_field", serde_json::json!(123))]);
        let err = serde_json::from_value::<ConfigReplace>(body)
            .expect_err("unknown fields must still 400");
        assert!(
            err.to_string().contains("totally_made_up_field"),
            "expected unknown-field error, got: {err}"
        );
    }

    #[test]
    fn config_replace_requires_nullable_fields_to_be_present() {
        let mut body = put_body(&[]);
        body.as_object_mut().unwrap().remove("default_theme");
        let err = serde_json::from_value::<ConfigReplace>(body)
            .expect_err("full replacements must include nullable fields explicitly");
        assert!(
            err.to_string().contains("default_theme"),
            "expected missing-field error, got: {err}"
        );
    }

    #[test]
    fn config_replace_round_trips_lowercase_google_font_style() {
        // VlcConfigView emits `style: "normal"|"italic"`; PUT must
        // accept the same shape via FontStyle's lowercase serde derive.
        for (style, expected) in [
            ("normal", vl_convert_google_fonts::FontStyle::Normal),
            ("italic", vl_convert_google_fonts::FontStyle::Italic),
        ] {
            let body = put_body(&[(
                "google_fonts",
                serde_json::json!([{
                    "family": "Roboto",
                    "variants": [{"weight": 400, "style": style}]
                }]),
            )]);
            let replace: ConfigReplace = serde_json::from_value(body)
                .unwrap_or_else(|e| panic!("style={style:?} must parse: {e}"));
            let variant = &replace.google_fonts[0].variants.as_ref().unwrap()[0];
            assert_eq!(variant.weight, 400);
            assert_eq!(variant.style, expected);
        }
    }

    #[test]
    fn config_patch_accepts_lowercase_google_font_style() {
        let body = serde_json::json!({
            "google_fonts": [{
                "family": "Inter",
                "variants": [{"weight": 500, "style": "normal"}]
            }]
        });
        let patch: ConfigPatch = serde_json::from_value(body).expect("ConfigPatch must parse");
        let google_fonts = patch.google_fonts.unwrap().unwrap();
        assert_eq!(google_fonts[0].family, "Inter");
        let variants = google_fonts[0].variants.as_ref().unwrap();
        assert!(matches!(
            variants[0].style,
            vl_convert_google_fonts::FontStyle::Normal
        ));
    }

    #[test]
    fn config_patch_rejects_unknown_google_font_style() {
        let body = serde_json::json!({
            "google_fonts": [{
                "family": "Inter",
                "variants": [{"weight": 500, "style": "oblique"}]
            }]
        });
        let err = serde_json::from_value::<ConfigPatch>(body)
            .expect_err("unknown style must surface as a serde error");
        // Serde's `unknown variant` message includes the rejected value and
        // accepted variants.
        let msg = err.to_string();
        assert!(
            msg.contains("oblique") && msg.contains("normal") && msg.contains("italic"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn vlc_config_serialize_does_not_emit_google_fonts_cache_dir() {
        // `google_fonts_cache_dir` is read-only system state surfaced on
        // `/infoz`, not on `VlcConfig`.
        let value: serde_json::Value = serde_json::to_value(VlcConfig::default()).expect("ser");
        assert!(
            !value
                .as_object()
                .unwrap()
                .contains_key("google_fonts_cache_dir"),
            "google_fonts_cache_dir must live on /infoz, not on VlcConfig"
        );
    }
}

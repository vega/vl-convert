use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use utoipa::ToSchema;
use vl_convert_rs::converter::{
    BaseUrlSetting, FormatLocale, GoogleFontRequest, MissingFontsPolicy, TimeFormatLocale,
    VlcConfig,
};
use vl_convert_rs::DEFAULT_VL_VERSION;

use crate::util::CommonOptsInput;

/// `double_option` helper: deserialize an absent field as `None`, an explicit
/// `null` as `Some(None)`, and any present value as `Some(Some(v))`. Required
/// for the `ConfigPatch` tri-state where "absent" and "null" must be
/// distinguished — the former means "preserve current" and the latter means
/// "clear the field" (or 400 for non-nullable fields, rejected by
/// `apply_patch`).
///
/// Implementation note: `Option::<Option<T>>::deserialize(d)` collapses a JSON
/// `null` onto the *outer* `None`, which is indistinguishable from the
/// `#[serde(default)]` substitution for an absent field. Instead, deserialize
/// `Option::<T>` (whose `null`-handling does the right thing for the inner
/// layer) and wrap `Some` around the result — serde only invokes this
/// function when the field is present in the input, so the outer `Some`
/// unambiguously means "present".
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

/// Wire-shape adapter for `BaseUrlSetting`: accepts the `bool | string` form
/// that `VlcConfigView` emits (`true` → `Default`, `false` → `Disabled`,
/// any string → `Custom(s)`) so `GET /admin/config` round-trips losslessly
/// through `PUT` / `PATCH`.
///
/// Strings are **always** mapped to `BaseUrlSetting::Custom(s)` — we do NOT
/// reinterpret `"default"` / `"disabled"` as enum shorthands even though
/// the library's own `BaseUrlSetting::Deserialize` does. Rationale: the
/// view serializer emits `true` / `false` for the enum variants and emits
/// a string only for `Custom(_)`, so a user round-tripping a `Custom("default")`
/// through GET → PUT would see their path silently remapped to
/// `BaseUrlSetting::Default` if the adapter honored the string shorthand.
/// Keeping the bool/string shapes disjoint eliminates that footgun; users
/// who want the enum variants can always send the boolean shorthand.
pub(crate) fn deserialize_base_url_view<'de, D>(
    deserializer: D,
) -> Result<BaseUrlSetting, D::Error>
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
    /// Vega config object merged via vega.mergeConfig.
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
    #[serde(default)]
    pub bundle: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaPngRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    pub scale: Option<f32>,
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VegaJpegRequest {
    #[serde(flatten)]
    pub common: VegaCommon,
    pub scale: Option<f32>,
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
    #[serde(default)]
    pub bundle: bool,
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
    pub spec: serde_json::Value,
    #[serde(default)]
    pub fullscreen: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgPngRequest {
    /// SVG markup string.
    pub svg: String,
    pub scale: Option<f32>,
    pub ppi: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgJpegRequest {
    pub svg: String,
    pub scale: Option<f32>,
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SvgPdfRequest {
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
    pub error: String,
}

// =============================================================================
// Admin /config DTOs (Task 7 — admin-config-parity plan)
// =============================================================================
//
// Tri-state semantics (design §2.5): for each patch field, the outer `Option`
// distinguishes "absent in the JSON body" (`None`) from "present with value"
// (`Some(_)`). The inner `Option` (when present on VlcConfig fields whose
// type is `Option<T>`) maps a JSON `null` to `Some(None)`. Non-optional
// VlcConfig fields use a single-layer `Option<T>`: null-on-non-nullable is
// rejected at serde parse time (400) and absence means "preserve current".
//
// Utoipa `ToSchema` derives are intentionally deferred to Task 13b — the
// `Option<Option<T>>` pattern through the `double_option` helper does not
// compose cleanly with `ToSchema` today. The admin-config surface's OpenAPI
// documentation lands as a follower task.

/// PATCH /admin/config body: a partial update where every field is optional.
///
/// * Field absent (`None`) → preserve the current value.
/// * Field present with a value → set the field to that value.
/// * Field present with `null`:
///     * For VlcConfig fields of type `Option<T>` → clear the field
///       (`Some(None)`).
///     * For non-optional VlcConfig fields → rejected at serde parse time
///       with 400 (the single-layer `Option<T>` has no way to represent the
///       cleared state).
///
/// `deny_unknown_fields` makes typos fail loudly rather than silently succeed.
#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigPatch {
    // `Option<Option<T>>` is a serde-layer tri-state (absent / null / value);
    // utoipa collapses it to `Option<T>` in the published schema. The
    // absent-vs-null distinction is documented in CLAUDE.md and enforced
    // by the server at parse time.
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable)]
    pub num_workers: Option<Option<NonZeroUsize>>,
    #[serde(
        default,
        deserialize_with = "deserialize_base_url_view_double_option"
    )]
    #[schema(value_type = Option<Object>, nullable)]
    pub base_url: Option<Option<BaseUrlSetting>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Vec<String>>, nullable)]
    pub allowed_base_urls: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<bool>, nullable)]
    pub auto_google_fonts: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<bool>, nullable)]
    pub embed_local_fonts: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<bool>, nullable)]
    pub subset_fonts: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>, nullable)]
    pub missing_fonts: Option<Option<MissingFontsPolicy>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Vec<Object>>, nullable)]
    pub google_fonts: Option<Option<Vec<GoogleFontRequest>>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable)]
    pub max_v8_heap_size_mb: Option<Option<NonZeroUsize>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable)]
    pub max_v8_execution_time_secs: Option<Option<NonZeroU64>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<bool>, nullable)]
    pub gc_after_conversion: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Vec<String>>, nullable)]
    pub vega_plugins: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Vec<String>>, nullable)]
    pub plugin_import_domains: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<bool>, nullable)]
    pub allow_per_request_plugins: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable)]
    pub max_ephemeral_workers: Option<Option<NonZeroUsize>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<bool>, nullable)]
    pub allow_google_fonts: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Vec<String>>, nullable)]
    pub per_request_plugin_import_domains: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>, nullable)]
    pub default_theme: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable)]
    pub default_format_locale: Option<Option<FormatLocale>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable)]
    pub default_time_format_locale: Option<Option<TimeFormatLocale>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Object>, nullable)]
    pub themes: Option<Option<HashMap<String, serde_json::Value>>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<u64>, nullable)]
    pub google_fonts_cache_size_mb: Option<Option<NonZeroU64>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<Vec<String>>, nullable)]
    pub font_directories: Option<Option<Vec<PathBuf>>>,
}

/// PUT /admin/config body: full replacement. Every VlcConfig field must be
/// present in the body (no `#[serde(default)]`); omission is a 400 parse
/// error. `null` follows natural JSON↔Option mapping — `null` on an
/// `Option<T>` field → `None`, `null` on a non-optional field → 400.
///
/// Intentionally a dedicated struct rather than a `ConfigReplace(VlcConfig)`
/// wrapper: `VlcConfig` uses `#[serde(default)]` at the container level so
/// `{}` would round-trip into a `VlcConfig::default()`; a PUT must reject
/// missing fields.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigReplace {
    #[schema(value_type = u64)]
    pub num_workers: NonZeroUsize,
    #[schema(value_type = Object)]
    #[serde(deserialize_with = "deserialize_base_url_view")]
    pub base_url: BaseUrlSetting,
    pub allowed_base_urls: Vec<String>,
    pub auto_google_fonts: bool,
    pub embed_local_fonts: bool,
    pub subset_fonts: bool,
    #[schema(value_type = String)]
    pub missing_fonts: MissingFontsPolicy,
    #[schema(value_type = Vec<Object>)]
    pub google_fonts: Vec<GoogleFontRequest>,
    #[schema(value_type = Option<u64>, nullable)]
    pub max_v8_heap_size_mb: Option<NonZeroUsize>,
    #[schema(value_type = Option<u64>, nullable)]
    pub max_v8_execution_time_secs: Option<NonZeroU64>,
    pub gc_after_conversion: bool,
    pub vega_plugins: Vec<String>,
    pub plugin_import_domains: Vec<String>,
    pub allow_per_request_plugins: bool,
    #[schema(value_type = Option<u64>, nullable)]
    pub max_ephemeral_workers: Option<NonZeroUsize>,
    pub allow_google_fonts: bool,
    pub per_request_plugin_import_domains: Vec<String>,
    pub default_theme: Option<String>,
    #[schema(value_type = Option<Object>, nullable)]
    pub default_format_locale: Option<FormatLocale>,
    #[schema(value_type = Option<Object>, nullable)]
    pub default_time_format_locale: Option<TimeFormatLocale>,
    #[schema(value_type = Object)]
    pub themes: HashMap<String, serde_json::Value>,
    #[schema(value_type = Option<u64>, nullable)]
    pub google_fonts_cache_size_mb: Option<NonZeroU64>,
    #[schema(value_type = Vec<String>)]
    pub font_directories: Vec<PathBuf>,
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
            google_fonts_cache_size_mb: r.google_fonts_cache_size_mb,
            font_directories: r.font_directories,
        }
    }
}

/// Successful GET / PATCH / PUT / DELETE response body for `/admin/config`.
#[derive(Debug, Serialize)]
pub(crate) struct ConfigView {
    pub baseline: VlcConfigView,
    pub effective: VlcConfigView,
    pub generation: u64,
    pub config_version: u64,
}

/// Request body for `POST /admin/config/fonts/directories`. Single-path
/// append — callers wanting replace-semantics use `PATCH /admin/config`
/// with the `font_directories` field.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FontDirRequest {
    /// Absolute filesystem path of the directory to register.
    #[schema(value_type = String)]
    pub path: std::path::PathBuf,
}

/// `VlcConfig` projected as the JSON shape produced by the Python binding's
/// `get_config()` helper (`vl-convert-python/src/config.rs::converter_config_json`).
/// Kept byte-equivalent so server and Python callers see the same shape.
///
/// Not derived from `VlcConfig::Serialize` because the library type does not
/// implement `Serialize` (and `FormatLocale` / `TimeFormatLocale` / `BaseUrlSetting`
/// serialize to different shapes than the library's `Deserialize` path accepts).
#[derive(Debug)]
pub(crate) struct VlcConfigView(pub VlcConfig);

impl Serialize for VlcConfigView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let cfg = &self.0;
        let mut map = serializer.serialize_map(Some(23))?;

        map.serialize_entry("num_workers", &cfg.num_workers.get())?;
        // Matches Python get_config(): Default → true, Disabled → false,
        // Custom(s) → s.
        let base_url_value: serde_json::Value = match &cfg.base_url {
            BaseUrlSetting::Default => serde_json::Value::Bool(true),
            BaseUrlSetting::Disabled => serde_json::Value::Bool(false),
            BaseUrlSetting::Custom(s) => serde_json::Value::String(s.clone()),
        };
        map.serialize_entry("base_url", &base_url_value)?;
        map.serialize_entry("allowed_base_urls", &cfg.allowed_base_urls)?;
        map.serialize_entry("auto_google_fonts", &cfg.auto_google_fonts)?;
        map.serialize_entry("embed_local_fonts", &cfg.embed_local_fonts)?;
        map.serialize_entry("subset_fonts", &cfg.subset_fonts)?;
        let missing_fonts_str = match cfg.missing_fonts {
            MissingFontsPolicy::Fallback => "fallback",
            MissingFontsPolicy::Warn => "warn",
            MissingFontsPolicy::Error => "error",
        };
        map.serialize_entry("missing_fonts", missing_fonts_str)?;

        // GoogleFontRequest/VariantRequest don't implement Serialize on the
        // library side in a JSON-equivalent shape; reproduce the Python
        // projection manually.
        let google_fonts_value: Vec<serde_json::Value> = cfg
            .google_fonts
            .iter()
            .map(|req| {
                let mut entry = serde_json::Map::new();
                entry.insert(
                    "family".to_string(),
                    serde_json::Value::String(req.family.clone()),
                );
                if let Some(variants) = req.variants.as_ref() {
                    let variants_value: Vec<serde_json::Value> = variants
                        .iter()
                        .map(|v| {
                            serde_json::json!({
                                "weight": v.weight,
                                "style": v.style.as_str(),
                            })
                        })
                        .collect();
                    entry.insert(
                        "variants".to_string(),
                        serde_json::Value::Array(variants_value),
                    );
                }
                serde_json::Value::Object(entry)
            })
            .collect();
        map.serialize_entry("google_fonts", &google_fonts_value)?;

        // Matches Python's `converter_config_json`: an optional absolute path
        // to the on-disk Google Fonts cache directory. `None` when the
        // cache is uninitialized / unavailable (e.g. the process has no
        // writable cache directory available).
        let google_fonts_cache_dir_value: Option<String> =
            vl_convert_rs::google_fonts_cache_dir()
                .map(|p| p.to_string_lossy().into_owned());
        map.serialize_entry("google_fonts_cache_dir", &google_fonts_cache_dir_value)?;

        map.serialize_entry(
            "google_fonts_cache_size_mb",
            &cfg.google_fonts_cache_size_mb.map(|n| n.get()),
        )?;
        map.serialize_entry(
            "max_v8_heap_size_mb",
            &cfg.max_v8_heap_size_mb.map(|n| n.get()),
        )?;
        map.serialize_entry(
            "max_v8_execution_time_secs",
            &cfg.max_v8_execution_time_secs.map(|n| n.get()),
        )?;
        map.serialize_entry("gc_after_conversion", &cfg.gc_after_conversion)?;
        map.serialize_entry("vega_plugins", &cfg.vega_plugins)?;
        map.serialize_entry("plugin_import_domains", &cfg.plugin_import_domains)?;
        map.serialize_entry("allow_per_request_plugins", &cfg.allow_per_request_plugins)?;
        map.serialize_entry(
            "max_ephemeral_workers",
            &cfg.max_ephemeral_workers.map(|n| n.get()),
        )?;
        map.serialize_entry("allow_google_fonts", &cfg.allow_google_fonts)?;
        map.serialize_entry(
            "per_request_plugin_import_domains",
            &cfg.per_request_plugin_import_domains,
        )?;
        map.serialize_entry("default_theme", &cfg.default_theme)?;

        let default_format_locale_value: Option<serde_json::Value> =
            cfg.default_format_locale.as_ref().map(|l| match l {
                FormatLocale::Name(n) => serde_json::Value::String(n.clone()),
                FormatLocale::Object(o) => o.clone(),
            });
        map.serialize_entry("default_format_locale", &default_format_locale_value)?;

        let default_time_format_locale_value: Option<serde_json::Value> =
            cfg.default_time_format_locale.as_ref().map(|l| match l {
                TimeFormatLocale::Name(n) => serde_json::Value::String(n.clone()),
                TimeFormatLocale::Object(o) => o.clone(),
            });
        map.serialize_entry(
            "default_time_format_locale",
            &default_time_format_locale_value,
        )?;

        map.serialize_entry("themes", &cfg.themes)?;

        let font_directories_value: Vec<String> = cfg
            .font_directories
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        map.serialize_entry("font_directories", &font_directories_value)?;

        map.end()
    }
}

/// Error code for a single field-level validation failure. Static slice so
/// it serializes cleanly and can be matched by callers.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Serialize, Clone)]
pub(crate) struct FieldError {
    pub path: String,
    pub code: FieldErrorCode,
    pub message: String,
}

/// 422 response body for PATCH / PUT / DELETE `/admin/config` when the
/// proposed config fails `apply_patch` or `normalize_converter_config` or
/// `VlConverter::with_config`.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct ConfigValidationError {
    pub error: String,
    pub field_errors: Vec<FieldError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_view_accepts_true() {
        let v: BaseUrlSetting = serde_json::from_value(serde_json::json!(true))
            .ok()
            .or_else(|| {
                deserialize_base_url_view(
                    &mut serde_json::Deserializer::from_str("true"),
                )
                .ok()
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
        // Strings always map to Custom — we do NOT honor the library's
        // string-shorthand ambiguity for "default"/"disabled", to keep
        // the admin wire contract round-trip-safe.
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

    #[test]
    fn config_replace_round_trips_default_get_shape() {
        // Simulate a payload that a client would obtain by GET'ing
        // /admin/config (where `base_url` serializes as `true`) and then
        // immediately PUT'ing back. Must succeed.
        let body = serde_json::json!({
            "num_workers": 2,
            "base_url": true,
            "allowed_base_urls": [],
            "auto_google_fonts": true,
            "embed_local_fonts": true,
            "subset_fonts": true,
            "missing_fonts": "warn",
            "google_fonts": [],
            "max_v8_heap_size_mb": 512,
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
            "google_fonts_cache_size_mb": null,
            "font_directories": []
        });
        let replace: ConfigReplace = serde_json::from_value(body)
            .expect("base_url: true must round-trip through ConfigReplace");
        assert_eq!(replace.base_url, BaseUrlSetting::Default);
    }

    #[test]
    fn config_replace_round_trips_disabled_get_shape() {
        let body = serde_json::json!({
            "num_workers": 2,
            "base_url": false,
            "allowed_base_urls": [],
            "auto_google_fonts": true,
            "embed_local_fonts": true,
            "subset_fonts": true,
            "missing_fonts": "warn",
            "google_fonts": [],
            "max_v8_heap_size_mb": 512,
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
            "google_fonts_cache_size_mb": null,
            "font_directories": []
        });
        let replace: ConfigReplace = serde_json::from_value(body).expect("parse");
        assert_eq!(replace.base_url, BaseUrlSetting::Disabled);
    }

    #[test]
    fn vlc_config_view_emits_google_fonts_cache_dir_key() {
        // Byte-equivalence with Python's `converter_config_json` requires
        // the `google_fonts_cache_dir` key to be present on every GET
        // response. Value may be string-or-null depending on the
        // library's cache-dir resolver, but the KEY must exist.
        let view = VlcConfigView(VlcConfig::default());
        let value: serde_json::Value = serde_json::to_value(&view).expect("serialize");
        let obj = value.as_object().expect("object");
        assert!(
            obj.contains_key("google_fonts_cache_dir"),
            "VlcConfigView must emit `google_fonts_cache_dir` for parity \
             with Python get_config()"
        );
        // When present, must be either a JSON string or null (matches
        // Python's `Option<String>` shape).
        let v = obj.get("google_fonts_cache_dir").unwrap();
        assert!(
            v.is_string() || v.is_null(),
            "google_fonts_cache_dir must be string-or-null, got {v:?}"
        );
    }
}

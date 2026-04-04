use crate::extract::{
    extract_fonts_from_vega, is_available, resolve_first_fonts, ClassifiedFont, FirstFontStatus,
    FontKey, FontSource,
};
use crate::text::{GOOGLE_FONTS_CLIENT, USVG_OPTIONS};
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use std::collections::{BTreeSet, HashMap, HashSet};
use vl_convert_google_fonts::{family_to_id, RegisteredFontBatch};

use super::config::{GoogleFontRequest, MissingFontsPolicy};

pub(crate) struct WorkerFontState {
    pub(crate) db: fontdb::Database,
    pub(crate) baseline_version: u64,
    pub(crate) shared_config_epoch: u64,
    pub(crate) hinting_enabled: bool,
    pub(crate) overlay_registrations: Vec<RegisteredFontBatch>,
}

impl WorkerFontState {
    pub(crate) fn from_baseline(snapshot: &crate::text::FontBaselineSnapshot) -> Self {
        Self {
            db: snapshot.clone_fontdb(),
            baseline_version: snapshot.version(),
            shared_config_epoch: snapshot.version(),
            hinting_enabled: snapshot.hinting_enabled(),
            overlay_registrations: Vec::new(),
        }
    }
}

/// Deduplication key for a Google Font request.
pub(crate) fn google_font_request_key(request: &GoogleFontRequest) -> String {
    let mut key = request.family.trim().to_lowercase();
    key.push('|');
    match &request.variants {
        None => key.push_str("all"),
        Some(variants) => {
            let mut pairs: Vec<(u16, &'static str)> = variants
                .iter()
                .map(|variant| (variant.weight, variant.style.as_str()))
                .collect();
            pairs.sort_unstable();
            for (idx, (weight, style)) in pairs.iter().enumerate() {
                if idx > 0 {
                    key.push(',');
                }
                key.push_str(&format!("{weight}:{style}"));
            }
        }
    }
    key
}

/// Classify a set of CSS `font-family` strings and return Google Fonts download
/// requests for any first-choice families that should be overlaid for a render.
///
/// When `prefer_cdn` is true (HTML path), Google-catalog fonts are requested
/// even if locally available so the render uses the same face the HTML output
/// will reference. When false (SVG/PNG/PDF path), only fonts not already in
/// `fontdb` are requested.
pub(crate) async fn classify_and_request_fonts(
    font_strings: HashSet<String>,
    auto_google_fonts: bool,
    missing_fonts: MissingFontsPolicy,
    prefer_cdn: bool,
) -> Result<Vec<GoogleFontRequest>, AnyError> {
    if font_strings.is_empty() {
        return Ok(Vec::new());
    }

    let available = available_font_families()?;

    let font_string_vec: Vec<String> = font_strings.into_iter().collect();

    let google_fonts_set: HashSet<String> = if auto_google_fonts {
        let candidates = auto_google_probe_candidates(&font_string_vec, &available, prefer_cdn);
        google_font_catalog_matches(candidates.iter(), missing_fonts).await?
    } else {
        HashSet::new()
    };

    // Classify each font string by its first entry
    let statuses = resolve_first_fonts(&font_string_vec, &available, |family| {
        auto_google_fonts && google_fonts_set.contains(family)
    });

    // Collect unavailable fonts -- report before any downloads
    let unavailable: Vec<(String, String)> = statuses
        .iter()
        .filter_map(|(css_string, status)| match status {
            FirstFontStatus::Unavailable { name } => Some((
                name.clone(),
                if name == css_string {
                    format!("'{name}'")
                } else {
                    format!("'{name}' (from \"{css_string}\")")
                },
            )),
            _ => None,
        })
        .collect();

    let unavailable_names: Vec<String> = unavailable.iter().map(|(name, _)| name.clone()).collect();
    let unavailable_details: Vec<String> = unavailable
        .iter()
        .map(|(_, detail)| detail.clone())
        .collect();
    report_unavailable_fonts(
        &unavailable_names,
        &unavailable_details,
        auto_google_fonts,
        missing_fonts,
    )?;

    if !auto_google_fonts {
        return Ok(Vec::new());
    }

    // Collect downloadable fonts as requests for the caller to add to VgOpts
    let mut requests: Vec<GoogleFontRequest> = Vec::new();
    for (_css_string, status) in &statuses {
        if let FirstFontStatus::NeedsDownload { name } = status {
            requests.push(GoogleFontRequest {
                family: name.clone(),
                variants: None,
            });
        }
    }

    Ok(requests)
}

/// Preprocess fonts from a compiled Vega specification.
///
/// Extracts font-family strings from the spec, then classifies and requests
/// fonts via [`classify_and_request_fonts`].
pub(crate) async fn preprocess_fonts(
    vega_spec: &serde_json::Value,
    auto_google_fonts: bool,
    missing_fonts: MissingFontsPolicy,
) -> Result<Vec<GoogleFontRequest>, AnyError> {
    if !auto_google_fonts && missing_fonts == MissingFontsPolicy::Fallback {
        return Ok(Vec::new());
    }

    let font_strings = extract_fonts_from_vega(vega_spec);
    classify_and_request_fonts(font_strings, auto_google_fonts, missing_fonts, false).await
}

/// Return all font family names currently available in fontdb.
pub(crate) fn available_font_families() -> Result<HashSet<String>, AnyError> {
    Ok(USVG_OPTIONS
        .lock()
        .map_err(|e| anyhow!("font_preprocessing: failed to lock USVG_OPTIONS: {e}"))?
        .fontdb
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect())
}

/// Collect font family names that should be probed against the Google Fonts
/// catalog. Used by the SVG/PNG preprocessing path. Parses CSS font-family
/// strings and keeps families that have a valid Google Fonts ID and are either
/// not locally available or `prefer_cdn` is set.
pub(crate) fn auto_google_probe_candidates(
    font_strings: &[String],
    available: &HashSet<String>,
    prefer_cdn: bool,
) -> BTreeSet<String> {
    font_strings
        .iter()
        .filter_map(|font_string| {
            let entries = crate::extract::parse_css_font_family(font_string);
            match entries.first() {
                Some(crate::extract::FontFamilyEntry::Named(name))
                    if (prefer_cdn || !is_available(name, available))
                        && family_to_id(name).is_some() =>
                {
                    Some(name.clone())
                }
                _ => None,
            }
        })
        .collect()
}

/// Collect font family names from the rendered scenegraph that should be
/// probed against Google Fonts. Excludes families already identified as
/// explicit per-call Google Font requests.
pub(crate) fn scenegraph_google_probe_candidates(
    families: &BTreeSet<String>,
    explicit_google_families: &HashSet<String>,
) -> BTreeSet<String> {
    families
        .iter()
        .filter(|family| !explicit_google_families.contains(*family))
        .cloned()
        .collect()
}

/// Probe the Google Fonts API for each family and return the set that
/// exists in the catalog. API errors are collected and reported according
/// to `missing_fonts` policy.
pub(crate) async fn google_font_catalog_matches<'a>(
    families: impl IntoIterator<Item = &'a String>,
    missing_fonts: MissingFontsPolicy,
) -> Result<HashSet<String>, AnyError> {
    let mut google_fonts_set: HashSet<String> = HashSet::new();
    let mut api_errors: Vec<(String, String)> = Vec::new();

    for family in families {
        match GOOGLE_FONTS_CLIENT.is_known_font(family).await {
            Ok(true) => {
                google_fonts_set.insert(family.clone());
            }
            Ok(false) => {}
            Err(e) => {
                api_errors.push((family.clone(), e.to_string()));
            }
        }
    }

    report_google_catalog_errors(&api_errors, missing_fonts)?;
    Ok(google_fonts_set)
}

/// Report Google Fonts API errors according to `missing_fonts` policy.
pub(crate) fn report_google_catalog_errors(
    api_errors: &[(String, String)],
    missing_fonts: MissingFontsPolicy,
) -> Result<(), AnyError> {
    if api_errors.is_empty() {
        return Ok(());
    }

    if missing_fonts == MissingFontsPolicy::Error {
        let details: Vec<String> = api_errors
            .iter()
            .map(|(name, err)| format!("'{name}': {err}"))
            .collect();
        return Err(anyhow!(
            "auto_google_fonts: could not reach the Google Fonts API to check \
             the following fonts: {}",
            details.join(", ")
        ));
    }

    if missing_fonts == MissingFontsPolicy::Warn {
        for (name, err) in api_errors {
            vl_warn!("auto_google_fonts: could not reach Google Fonts API for '{name}': {err}");
        }
    }

    Ok(())
}

/// Report fonts that are neither in Google Fonts nor locally available,
/// according to `missing_fonts` policy: ignore, warn, or error.
pub(crate) fn report_unavailable_fonts(
    unavailable_names: &[String],
    unavailable_details: &[String],
    auto_google_fonts: bool,
    missing_fonts: MissingFontsPolicy,
) -> Result<(), AnyError> {
    if unavailable_names.is_empty() {
        return Ok(());
    }

    if missing_fonts == MissingFontsPolicy::Error {
        if auto_google_fonts {
            return Err(anyhow!(
                "auto_google_fonts: the following fonts are not available on the system \
                 and not found in the Google Fonts catalog: {}",
                unavailable_details.join(", ")
            ));
        } else {
            return Err(anyhow!(
                "missing_fonts=error: the following fonts are not available on the system: {}. \
                 Add them to google_fonts config or enable auto_google_fonts.",
                unavailable_details.join(", ")
            ));
        }
    }

    if missing_fonts == MissingFontsPolicy::Warn {
        for name in unavailable_names {
            if auto_google_fonts {
                vl_warn!(
                    "auto_google_fonts: font '{name}' is not available on the system \
                     and not found in the Google Fonts catalog, skipping"
                );
            } else {
                vl_warn!("missing_fonts=warn: font '{name}' is not available on the system");
            }
        }
    }

    Ok(())
}

/// Create a `ClassifiedFont` with `FontSource::Google` for a family name,
/// or `None` if the name doesn't map to a valid Google Fonts ID.
pub(crate) fn classify_as_google_font(family: &str) -> Option<ClassifiedFont> {
    Some(ClassifiedFont {
        family: family.to_string(),
        source: FontSource::Google {
            font_id: family_to_id(family)?,
        },
    })
}

/// Classify font families extracted from the scenegraph into Google Fonts
/// or Local sources.
///
/// `explicit_google_families` are families provided by per-call
/// `GoogleFontRequest` entries -- they are classified as Google immediately
/// without catalog probing and are excluded from missing-font reporting.
///
/// Fonts that exist in the Google Fonts catalog are sourced from Google for
/// portability (CDN links work on any machine). Remaining fonts are classified
/// as Local when `embed_local_fonts` is true and the font is available
/// in fontdb.
pub(crate) async fn classify_scenegraph_fonts(
    families: &BTreeSet<String>,
    auto_google_fonts: bool,
    embed_local_fonts: bool,
    missing_fonts: MissingFontsPolicy,
    explicit_google_families: &HashSet<String>,
) -> Result<Vec<ClassifiedFont>, AnyError> {
    if families.is_empty()
        || (!auto_google_fonts
            && !embed_local_fonts
            && missing_fonts == MissingFontsPolicy::Fallback
            && explicit_google_families.is_empty())
    {
        return Ok(Vec::new());
    }

    let available = available_font_families()?;

    let google_fonts_set: HashSet<String> = if auto_google_fonts {
        let candidates = scenegraph_google_probe_candidates(families, explicit_google_families);
        google_font_catalog_matches(candidates.iter(), missing_fonts).await?
    } else {
        HashSet::new()
    };

    let mut classified_fonts: Vec<ClassifiedFont> = Vec::new();
    let mut unavailable: Vec<String> = Vec::new();
    for family in families {
        // Explicit per-call requests win immediately
        if explicit_google_families.contains(family) {
            if let Some(font) = classify_as_google_font(family) {
                classified_fonts.push(font);
            }
            continue;
        }
        if auto_google_fonts && google_fonts_set.contains(family) {
            if let Some(font) = classify_as_google_font(family) {
                classified_fonts.push(font);
                continue;
            }
        }
        if is_available(family, &available) {
            if embed_local_fonts {
                classified_fonts.push(ClassifiedFont {
                    family: family.clone(),
                    source: FontSource::Local,
                });
            }
            // Font is locally available -- not missing even if not embedded
        } else {
            unavailable.push(family.clone());
        }
    }

    // Report fonts that are neither in Google Fonts nor locally available.
    // This covers runtime-resolved families (signal/field-driven) that static
    // spec extraction cannot see.
    let unavailable_details: Vec<String> = unavailable.iter().map(|n| format!("'{n}'")).collect();
    report_unavailable_fonts(
        &unavailable,
        &unavailable_details,
        auto_google_fonts,
        missing_fonts,
    )?;

    Ok(classified_fonts)
}

/// Result of analyzing a rendered Vega scenegraph for font embedding.
pub(crate) struct FontAnalysis {
    /// Classified font metadata (Google or Local).
    pub(crate) classified_fonts: Vec<ClassifiedFont>,
    /// Characters used per (family, weight, style) -- for subsetting.
    pub(crate) chars_by_key: HashMap<FontKey, BTreeSet<char>>,
    /// (weight, style) variants per family -- for CDN URLs.
    pub(crate) family_variants: HashMap<String, BTreeSet<(String, String)>>,
}

use crate::error::FontsourceError;
use crate::types::{FontStyle, FontsourceFont, VariantRequest};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ResolvedTtfFile {
    pub url: String,
    pub weight: u16,
    pub style: FontStyle,
    pub subset: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedDownloadPlan {
    pub loaded_variants: Vec<VariantRequest>,
    pub files: Vec<ResolvedTtfFile>,
}

pub(crate) fn dedupe_variants(variants: &[VariantRequest]) -> Vec<VariantRequest> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(variants.len());

    for variant in variants {
        if seen.insert((variant.weight, variant.style)) {
            deduped.push(variant.clone());
        }
    }

    deduped
}

fn sorted_weight_keys(metadata: &FontsourceFont) -> Vec<String> {
    let mut keys: Vec<String> = metadata.variants.keys().cloned().collect();
    keys.sort_by_key(|k| k.parse::<u16>().unwrap_or(u16::MAX));
    keys
}

fn sorted_style_keys(
    styles: &HashMap<String, HashMap<String, crate::types::FontsourceUrls>>,
) -> Vec<String> {
    let mut keys: Vec<String> = styles.keys().cloned().collect();
    keys.sort_by_key(|key| {
        if key == "normal" {
            0usize
        } else if key == "italic" {
            1usize
        } else {
            2usize
        }
    });
    keys
}

/// Sort subset keys with the font's default subset first.
///
/// fontdb selects the first registered face matching a family/weight/style query.
/// By registering the default subset first, we ensure fontdb picks a face whose
/// glyph coverage matches the font's primary script (typically Latin).
fn sorted_subset_keys(
    subsets: &HashMap<String, crate::types::FontsourceUrls>,
    def_subset: &str,
) -> Vec<String> {
    let mut keys: Vec<String> = subsets.keys().cloned().collect();
    keys.sort_by(|a, b| {
        let rank = |k: &str| -> u8 {
            if k == def_subset {
                0
            } else {
                1
            }
        };
        rank(a).cmp(&rank(b)).then_with(|| a.cmp(b))
    });
    keys
}

pub(crate) fn resolve_download_plan(
    font_id: &str,
    metadata: &FontsourceFont,
    variants: Option<&[VariantRequest]>,
) -> Result<ResolvedDownloadPlan, FontsourceError> {
    let def_subset = &metadata.def_subset;

    match variants {
        Some(requested) => {
            if requested.is_empty() {
                return Err(FontsourceError::NoVariantsRequested);
            }

            let deduped = dedupe_variants(requested);
            let mut unavailable: Vec<VariantRequest> = Vec::new();
            let mut files = Vec::new();

            for req in &deduped {
                let weight_key = req.weight.to_string();
                let style_key = req.style.as_str();

                let maybe_subsets = metadata
                    .variants
                    .get(&weight_key)
                    .and_then(|styles| styles.get(style_key));

                let Some(subsets) = maybe_subsets else {
                    unavailable.push(req.clone());
                    continue;
                };

                let mut found_ttf = false;
                for subset_key in sorted_subset_keys(subsets, def_subset) {
                    let Some(subset_urls) = subsets.get(&subset_key) else {
                        continue;
                    };

                    if let Some(ttf_url) = &subset_urls.url.ttf {
                        found_ttf = true;
                        files.push(ResolvedTtfFile {
                            url: ttf_url.clone(),
                            weight: req.weight,
                            style: req.style,
                            subset: subset_key.clone(),
                        });
                    }
                }

                if !found_ttf {
                    unavailable.push(req.clone());
                }
            }

            if !unavailable.is_empty() {
                return Err(FontsourceError::VariantsNotAvailable {
                    font_id: font_id.to_string(),
                    unavailable,
                });
            }

            Ok(ResolvedDownloadPlan {
                loaded_variants: deduped,
                files,
            })
        }
        None => {
            let mut files = Vec::new();
            let mut loaded_variants = Vec::new();
            let mut seen = HashSet::new();

            for weight_key in sorted_weight_keys(metadata) {
                let Some(styles) = metadata.variants.get(&weight_key) else {
                    continue;
                };

                let weight = match weight_key.parse::<u16>() {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                for style_key in sorted_style_keys(styles) {
                    let Ok(style) = style_key.parse::<FontStyle>() else {
                        continue;
                    };
                    let Some(subsets) = styles.get(&style_key) else {
                        continue;
                    };

                    let mut has_ttf = false;
                    for subset_key in sorted_subset_keys(subsets, def_subset) {
                        let Some(subset_urls) = subsets.get(&subset_key) else {
                            continue;
                        };

                        if let Some(ttf_url) = &subset_urls.url.ttf {
                            has_ttf = true;
                            files.push(ResolvedTtfFile {
                                url: ttf_url.clone(),
                                weight,
                                style,
                                subset: subset_key.clone(),
                            });
                        }
                    }

                    if has_ttf && seen.insert((weight, style)) {
                        loaded_variants.push(VariantRequest { weight, style });
                    }
                }
            }

            Ok(ResolvedDownloadPlan {
                loaded_variants,
                files,
            })
        }
    }
}

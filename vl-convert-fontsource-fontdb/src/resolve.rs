use crate::error::FontsourceFontdbError;
use crate::types::{FontStyle, FontsourceFont, VariantRequest};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedTtfFile {
    pub url: String,
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

fn sorted_subset_keys(subsets: &HashMap<String, crate::types::FontsourceUrls>) -> Vec<String> {
    let mut keys: Vec<String> = subsets.keys().cloned().collect();
    keys.sort();
    keys
}

pub(crate) fn resolve_download_plan(
    font_id: &str,
    metadata: &FontsourceFont,
    variants: Option<&[VariantRequest]>,
) -> Result<ResolvedDownloadPlan, FontsourceFontdbError> {
    match variants {
        Some(requested) => {
            if requested.is_empty() {
                return Err(FontsourceFontdbError::NoVariantsRequested);
            }

            let deduped = dedupe_variants(requested);
            let mut unavailable = Vec::new();
            let mut files = Vec::new();

            for req in &deduped {
                let weight_key = req.weight.to_string();
                let style_key = req.style.as_str();

                let maybe_subsets = metadata
                    .variants
                    .get(&weight_key)
                    .and_then(|styles| styles.get(style_key));

                let Some(subsets) = maybe_subsets else {
                    unavailable.push(format!("{}-{}", req.weight, style_key));
                    continue;
                };

                let mut found_ttf = false;
                for subset_key in sorted_subset_keys(subsets) {
                    let Some(subset_urls) = subsets.get(&subset_key) else {
                        continue;
                    };

                    if let Some(ttf_url) = &subset_urls.url.ttf {
                        found_ttf = true;
                        files.push(ResolvedTtfFile {
                            url: ttf_url.clone(),
                        });
                    }
                }

                if !found_ttf {
                    unavailable.push(format!("{}-{}", req.weight, style_key));
                }
            }

            if !unavailable.is_empty() {
                return Err(FontsourceFontdbError::VariantsNotAvailable {
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
                    for subset_key in sorted_subset_keys(subsets) {
                        let Some(subset_urls) = subsets.get(&subset_key) else {
                            continue;
                        };

                        if let Some(ttf_url) = &subset_urls.url.ttf {
                            has_ttf = true;
                            files.push(ResolvedTtfFile {
                                url: ttf_url.clone(),
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

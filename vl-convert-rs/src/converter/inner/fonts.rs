use super::InnerVlConverter;
use crate::text::{get_font_baseline_snapshot, FONT_CONFIG_VERSION, GOOGLE_FONTS_CLIENT};
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use vl_convert_google_fonts::{
    FontLoadRequest, GoogleFontUsage, GoogleFontsDatabaseExt, LoadedFontBatch,
};

use super::super::fonts::{
    error_with_google_font_usage, google_font_request_key, GoogleFontRequest, WorkerFontState,
};

impl InnerVlConverter {
    fn publish_worker_font_state_to_opstate(&mut self) {
        self.font_state.shared_config_epoch = self.font_state.shared_config_epoch.wrapping_add(1);
        let resolved = vl_convert_canvas2d::ResolvedFontConfig::from_parts(
            self.font_state.db.clone(),
            self.font_state.hinting_enabled,
        );
        let shared_config = vl_convert_canvas2d_deno::SharedFontConfig::new(
            resolved,
            self.font_state.shared_config_epoch,
        );
        self.usvg_options.fontdb = Arc::new(self.font_state.db.clone());
        self.worker
            .js_runtime
            .op_state()
            .borrow_mut()
            .put(shared_config);
    }

    pub(crate) fn refresh_font_config_if_needed(&mut self) -> Result<(), AnyError> {
        let current = FONT_CONFIG_VERSION.load(Ordering::Acquire);
        if current != self.font_state.baseline_version {
            let snapshot = get_font_baseline_snapshot()?;
            self.font_state = WorkerFontState::from_baseline(&snapshot);
            self.publish_worker_font_state_to_opstate();
        }
        Ok(())
    }

    fn apply_google_fonts_overlay(&mut self, batches: Vec<LoadedFontBatch>) {
        if batches.is_empty() {
            return;
        }
        debug_assert!(
            self.font_state.overlay_registrations.is_empty(),
            "overlay registrations should be empty before applying a new request overlay"
        );
        for batch in batches {
            let registration = self.font_state.db.register_google_fonts_batch(batch);
            self.font_state.overlay_registrations.push(registration);
        }
        self.publish_worker_font_state_to_opstate();
    }

    pub(crate) fn clear_google_fonts_overlay(&mut self) {
        if self.font_state.overlay_registrations.is_empty() {
            return;
        }
        for registration in self.font_state.overlay_registrations.drain(..) {
            self.font_state
                .db
                .unregister_google_fonts_batch(registration);
        }
        self.publish_worker_font_state_to_opstate();
    }

    /// Resolve Google Fonts and apply them as an overlay on the worker's fontdb.
    /// Caller must call `clear_google_fonts_overlay()` after the work is done.
    pub(crate) async fn apply_font_overlay_if_needed(
        &mut self,
        google_fonts: Option<Vec<GoogleFontRequest>>,
    ) -> Result<GoogleFontUsage, AnyError> {
        let resolved = self.resolve_google_fonts(google_fonts).await?;
        if !resolved.batches.is_empty() {
            self.apply_google_fonts_overlay(resolved.batches);
        }
        Ok(resolved.google_fonts)
    }

    /// Resolve Google Fonts requests on the worker thread using the async API.
    ///
    /// Merges per-request fonts with `config.google_fonts`, deduplicates while
    /// preserving request order, and stops admitting new families once the
    /// configured variant threshold has been reached.
    pub(crate) async fn resolve_google_fonts(
        &self,
        request_fonts: Option<Vec<GoogleFontRequest>>,
    ) -> Result<ResolvedGoogleFonts, AnyError> {
        let mut merged = self.ctx.config.google_fonts.clone();
        if let Some(request) = request_fonts {
            merged.extend(request);
        }
        if merged.is_empty() {
            return Ok(ResolvedGoogleFonts::default());
        }

        let unique = unique_google_font_requests(merged);

        let mut batches = Vec::new();
        let mut google_fonts = GoogleFontUsage::default();
        let variant_threshold = self
            .ctx
            .config
            .google_font_variant_threshold
            .map(|n| usize::try_from(n.get()).unwrap_or(usize::MAX));
        let mut used_variants = 0usize;
        for request in unique {
            if let Some(threshold) = variant_threshold {
                if used_variants >= threshold {
                    return Err(error_with_google_font_usage(
                        anyhow!(
                            "Google Font variant threshold {threshold} reached after resolving \
                             {used_variants} variants; refusing to load family '{}'",
                            request.family
                        ),
                        google_fonts,
                    ));
                }
            }
            let loaded = match GOOGLE_FONTS_CLIENT
                .load(FontLoadRequest {
                    family: &request.family,
                    variants: request.variants.as_deref(),
                })
                .await
            {
                Ok(loaded) => loaded,
                Err(err) => {
                    let error = AnyError::new(err).context(format!(
                        "Failed to load request font '{}' from Google Fonts",
                        request.family
                    ));
                    return Err(error_with_google_font_usage(error, google_fonts));
                }
            };
            used_variants = used_variants.saturating_add(
                usize::try_from(loaded.usage.stats.resolved_variants).unwrap_or(usize::MAX),
            );
            google_fonts.add_assign(loaded.usage);
            batches.push(loaded.batch);
        }
        Ok(ResolvedGoogleFonts {
            batches,
            google_fonts,
        })
    }
}

#[derive(Default)]
pub(crate) struct ResolvedGoogleFonts {
    pub(crate) batches: Vec<LoadedFontBatch>,
    pub(crate) google_fonts: GoogleFontUsage,
}

fn unique_google_font_requests(requests: Vec<GoogleFontRequest>) -> Vec<GoogleFontRequest> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for request in requests {
        let key = google_font_request_key(&request);
        if seen.insert(key) {
            unique.push(request);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use vl_convert_google_fonts::{FontStyle, VariantRequest};

    #[test]
    fn unique_google_font_requests_preserves_first_seen_order() {
        let requests = vec![
            GoogleFontRequest {
                family: "Roboto".to_string(),
                variants: None,
            },
            GoogleFontRequest {
                family: "Inter".to_string(),
                variants: None,
            },
            GoogleFontRequest {
                family: "roboto".to_string(),
                variants: None,
            },
            GoogleFontRequest {
                family: "Roboto".to_string(),
                variants: Some(vec![VariantRequest {
                    weight: 700,
                    style: FontStyle::Normal,
                }]),
            },
        ];

        let unique = unique_google_font_requests(requests);

        assert_eq!(unique.len(), 3);
        assert_eq!(unique[0].family, "Roboto");
        assert!(unique[0].variants.is_none());
        assert_eq!(unique[1].family, "Inter");
        assert_eq!(unique[2].family, "Roboto");
        assert_eq!(unique[2].variants.as_ref().unwrap()[0].weight, 700);
    }
}

use super::InnerVlConverter;
use crate::text::{get_font_baseline_snapshot, FONT_CONFIG_VERSION, GOOGLE_FONTS_CLIENT};
use deno_core::error::AnyError;
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use vl_convert_google_fonts::{
    FontLoadRequest, GoogleFontStats, GoogleFontsDatabaseExt, LoadedFontBatch,
};

use super::super::fonts::{
    error_with_google_font_stats, google_font_request_key, GoogleFontRequest, WorkerFontState,
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
    ) -> Result<GoogleFontStats, AnyError> {
        let resolved = self.resolve_google_fonts(google_fonts).await?;
        if !resolved.batches.is_empty() {
            self.apply_google_fonts_overlay(resolved.batches);
        }
        Ok(resolved.stats)
    }

    /// Resolve Google Fonts requests on the worker thread using the async API.
    ///
    /// Merges per-request fonts with `config.google_fonts`, deduplicates, and
    /// downloads each unique font via `GOOGLE_FONTS_CLIENT.load()`.
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

        let mut unique: BTreeMap<String, GoogleFontRequest> = BTreeMap::new();
        for request in merged {
            let key = google_font_request_key(&request);
            unique.entry(key).or_insert(request);
        }

        let mut batches = Vec::new();
        let mut stats = GoogleFontStats::default();
        let max_variants = self
            .ctx
            .config
            .max_google_font_variants_per_request
            .map(|n| usize::try_from(n.get()).unwrap_or(usize::MAX));
        let mut used_variants = 0usize;
        for request in unique.into_values() {
            let remaining = max_variants.map(|max| max.saturating_sub(used_variants));
            let loaded = match GOOGLE_FONTS_CLIENT
                .load(FontLoadRequest {
                    family: &request.family,
                    variants: request.variants.as_deref(),
                    max_variants: remaining,
                })
                .await
            {
                Ok(loaded) => loaded,
                Err(err) => {
                    let error = AnyError::new(err).context(format!(
                        "Failed to load request font '{}' from Google Fonts",
                        request.family
                    ));
                    return Err(error_with_google_font_stats(error, stats));
                }
            };
            used_variants = used_variants.saturating_add(loaded.stats.resolved_variants as usize);
            stats.add_assign(loaded.stats);
            batches.push(loaded.batch);
        }
        Ok(ResolvedGoogleFonts { batches, stats })
    }
}

#[derive(Default)]
pub(crate) struct ResolvedGoogleFonts {
    pub(crate) batches: Vec<LoadedFontBatch>,
    pub(crate) stats: GoogleFontStats,
}

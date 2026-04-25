use crate::anyhow;
use crate::anyhow::anyhow;
use crate::image_loading::custom_string_resolver;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use usvg::fontdb::Database;
use usvg::{
    FallbackSelectionFn, FontFamily, FontResolver, FontSelectionFn, FontStretch, FontStyle,
    ImageHrefResolver,
};
use vl_convert_canvas2d::font_config::{CustomFont, FontConfig, ResolvedFontConfig};
use vl_convert_google_fonts::GoogleFontsClient;

/// Monotonically increasing version counter for font configuration changes.
/// Incremented each time font configuration is modified.
pub static FONT_CONFIG_VERSION: AtomicU64 = AtomicU64::new(0);

/// Default cap (in MB) for the on-disk Google Fonts LRU cache. Used by
/// `apply_hot_font_cache` when called with `None` to actively reset the
/// `GOOGLE_FONTS_CLIENT` LRU back to a known baseline (e.g. by the
/// server admin-rollback path).
///
/// Mirrors `vl_convert_google_fonts::DEFAULT_MAX_FONT_CACHE_BYTES` (512 MB).
pub const DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB: u64 = 512;

/// Current capacity observed on `GOOGLE_FONTS_CLIENT`. Written by
/// `apply_hot_font_cache` on every call; read by `current_cache_size`. The
/// value stored is `cap_mb` — 0 when no cap has been applied yet (initial
/// startup state) and any positive integer otherwise.
///
/// The server admin path reads this via `current_cache_size()` so it can
/// snapshot → attempt rebuild → restore-on-failure cleanly. Callers
/// observe the last value written by `apply_hot_font_cache`; the library
/// default sentinel is `DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB`.
static CURRENT_CACHE_SIZE_MB: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct FontBaselineSnapshot {
    resolved: Arc<ResolvedFontConfig>,
    version: u64,
}

impl FontBaselineSnapshot {
    pub fn resolved(&self) -> Arc<ResolvedFontConfig> {
        self.resolved.clone()
    }

    pub fn clone_fontdb(&self) -> Database {
        self.resolved.clone_fontdb()
    }

    pub fn hinting_enabled(&self) -> bool {
        self.resolved.hinting_enabled()
    }

    pub fn version(&self) -> u64 {
        self.version
    }
}

lazy_static! {
    pub static ref FONT_CONFIG: Mutex<FontConfig> = Mutex::new(build_default_font_config());
    pub static ref FONT_BASELINE: RwLock<FontBaselineSnapshot> = RwLock::new(
        build_font_baseline_snapshot(&build_default_font_config(), 0),
    );
    pub static ref USVG_OPTIONS: Mutex<usvg::Options<'static>> = Mutex::new(init_usvg_options());
    pub static ref GOOGLE_FONTS_CLIENT: GoogleFontsClient = GoogleFontsClient::default();
}

const LIBERATION_SANS_REGULAR: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Regular.ttf");
const LIBERATION_SANS_BOLD: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Bold.ttf");
const LIBERATION_SANS_ITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-Italic.ttf");
const LIBERATION_SANS_BOLDITALIC: &[u8] =
    include_bytes!("../fonts/liberation-sans/LiberationSans-BoldItalic.ttf");

/// Build the default FontConfig with vendored Liberation Sans fonts,
/// system fonts enabled, and standard generic family mappings.
pub fn build_default_font_config() -> FontConfig {
    let liberation_fonts = vec![
        CustomFont {
            data: Arc::new(Vec::from(LIBERATION_SANS_REGULAR)),
            family_name: None,
        },
        CustomFont {
            data: Arc::new(Vec::from(LIBERATION_SANS_BOLD)),
            family_name: None,
        },
        CustomFont {
            data: Arc::new(Vec::from(LIBERATION_SANS_ITALIC)),
            family_name: None,
        },
        CustomFont {
            data: Arc::new(Vec::from(LIBERATION_SANS_BOLDITALIC)),
            family_name: None,
        },
    ];

    FontConfig {
        custom_fonts: liberation_fonts,
        load_system_fonts: true,
        ..FontConfig::default()
    }
}

fn build_font_baseline_snapshot(config: &FontConfig, version: u64) -> FontBaselineSnapshot {
    FontBaselineSnapshot {
        resolved: Arc::new(config.resolve()),
        version,
    }
}

pub fn get_font_baseline_snapshot() -> Result<FontBaselineSnapshot, anyhow::Error> {
    FONT_BASELINE
        .read()
        .map_err(|err| anyhow!("Failed to acquire font baseline lock: {err}"))
        .map(|snapshot| snapshot.clone())
}

pub fn build_usvg_options_with_fontdb(fontdb: Database) -> usvg::Options<'static> {
    let image_href_resolver = ImageHrefResolver {
        resolve_string: custom_string_resolver(),
        ..Default::default()
    };

    let font_resolver = FontResolver {
        select_font: custom_font_selector(),
        select_fallback: custom_fallback_selector(),
    };

    usvg::Options {
        image_href_resolver,
        fontdb: Arc::new(fontdb),
        font_resolver,
        ..Default::default()
    }
}

fn init_usvg_options() -> usvg::Options<'static> {
    let snapshot =
        get_font_baseline_snapshot().expect("Failed to load baseline snapshot for usvg options");
    build_usvg_options_with_fontdb(snapshot.clone_fontdb())
}

fn refresh_font_baseline_after_config_update() -> Result<(), anyhow::Error> {
    let config = {
        FONT_CONFIG
            .lock()
            .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?
            .clone()
    };

    let resolved = Arc::new(config.resolve());

    // Acquire USVG_OPTIONS before FONT_BASELINE: USVG_OPTIONS is a lazy_static
    // whose initializer reads FONT_BASELINE. Taking the write lock first would
    // deadlock if USVG_OPTIONS hasn't been initialized yet.
    let mut opts = USVG_OPTIONS
        .lock()
        .map_err(|err| anyhow!("Failed to acquire usvg options lock: {err}"))?;
    let mut baseline = FONT_BASELINE
        .write()
        .map_err(|err| anyhow!("Failed to acquire font baseline lock: {err}"))?;

    let next_version = FONT_CONFIG_VERSION.fetch_add(1, Ordering::AcqRel) + 1;
    let snapshot = FontBaselineSnapshot {
        resolved: resolved.clone(),
        version: next_version,
    };
    *baseline = snapshot;
    *opts = build_usvg_options_with_fontdb(resolved.clone_fontdb());

    Ok(())
}

pub fn custom_font_selector() -> FontSelectionFn<'static> {
    Box::new(move |font, fontdb| {
        // First, try for exact match using fontdb's default font lookup
        let mut name_list = Vec::new();
        for family in font.families() {
            name_list.push(match family {
                FontFamily::Serif => fontdb::Family::Serif,
                FontFamily::SansSerif => fontdb::Family::SansSerif,
                FontFamily::Cursive => fontdb::Family::Cursive,
                FontFamily::Fantasy => fontdb::Family::Fantasy,
                FontFamily::Monospace => fontdb::Family::Monospace,
                FontFamily::Named(s) => fontdb::Family::Name(s.as_str()),
            });
        }

        let stretch = match font.stretch() {
            FontStretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
            FontStretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
            FontStretch::Condensed => fontdb::Stretch::Condensed,
            FontStretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
            FontStretch::Normal => fontdb::Stretch::Normal,
            FontStretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
            FontStretch::Expanded => fontdb::Stretch::Expanded,
            FontStretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
            FontStretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
        };

        let style = match font.style() {
            FontStyle::Normal => fontdb::Style::Normal,
            FontStyle::Italic => fontdb::Style::Italic,
            FontStyle::Oblique => fontdb::Style::Oblique,
        };

        let query = fontdb::Query {
            families: &name_list,
            weight: fontdb::Weight(font.weight()),
            stretch,
            style,
        };

        if let Some(id) = fontdb.query(&query) {
            // fontdb found a match, use it
            return Some(id);
        }

        // Next, try matching the family name against the post_script_name of each font face.
        // For example, if the SVG font family is "Matter SemiBold", the logic above search for
        // a font family with this name, which will not be found (because the family is Matter).
        // The face's post_script_name for this face will be "Matter-SemiBold"
        for family in &name_list {
            let name = fontdb.family_name(family).replace('-', " ");
            for face in fontdb.faces() {
                if face.post_script_name.replace('-', " ") == name {
                    return Some(face.id);
                }
            }
        }

        vl_warn!(
            "No match for '{}' font-family.",
            font.families()
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        None
    })
}

/// Creates a default font fallback selection resolver.
///
/// The default implementation searches through the entire `fontdb`
/// to find a font that has the correct style and supports the character.
pub fn custom_fallback_selector() -> FallbackSelectionFn<'static> {
    Box::new(|c, exclude_fonts, fontdb| {
        let base_font_id = exclude_fonts[0];

        // Prevent fallback to fonts that won't work, like LastResort on macOS
        let forbidden_fallback = ["LastResort"];

        // Iterate over fonts and check if any of them support the specified char.
        for face in fontdb.faces() {
            // Ignore fonts, that were used for shaping already.
            if exclude_fonts.contains(&face.id)
                || forbidden_fallback.contains(&face.post_script_name.as_str())
            {
                continue;
            }

            // Check that the new face has the same style.
            let base_face = fontdb.face(base_font_id)?;
            if base_face.style != face.style
                && base_face.weight != face.weight
                && base_face.stretch != face.stretch
            {
                continue;
            }

            // has_char is private in fontdb
            // if !fontdb.has_char(face.id, c) {
            //     continue;
            // }

            // Implement `fontdb.has_char`, which is not public in fontdb
            let res = fontdb.with_face_data(face.id, |font_data, face_index| -> Option<bool> {
                let font = ttf_parser::Face::parse(font_data, face_index).ok()?;

                font.glyph_index(c)?;
                Some(true)
            });
            if res != Some(Some(true)) {
                continue;
            }

            let base_family = base_face
                .families
                .iter()
                .find(|f| f.1 == fontdb::Language::English_UnitedStates)
                .unwrap_or(&base_face.families[0]);

            let new_family = face
                .families
                .iter()
                .find(|f| f.1 == fontdb::Language::English_UnitedStates)
                .unwrap_or(&base_face.families[0]);

            vl_warn!("Fallback from {} to {}.", base_family.0, new_family.0);
            return Some(face.id);
        }

        None
    })
}

/// Replace the process-global registered-font-directory list.
///
/// Acquires `FONT_CONFIG`, sets `font_dirs = paths.to_vec()`, rebuilds the
/// `ResolvedFontConfig` via `refresh_font_baseline_after_config_update`, and
/// bumps `FONT_CONFIG_VERSION`. Workers pick up the new state on their next
/// work item via `InnerVlConverter::refresh_font_config_if_needed`.
///
/// This is the single authoritative way to mutate the font-directory
/// registry. Direct callers should prefer threading paths through
/// `VlcConfig::font_directories` and constructing a `VlConverter` via
/// `with_config`, which calls this function internally.
pub fn set_font_directories(paths: &[PathBuf]) -> Result<(), anyhow::Error> {
    {
        let mut font_config = FONT_CONFIG
            .lock()
            .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?;
        font_config.font_dirs = paths.to_vec();
    }
    refresh_font_baseline_after_config_update()
}

/// Return the currently-registered font directories (a snapshot of
/// `FONT_CONFIG.font_dirs`).
///
/// Useful for the admin-reconfig path, which snapshots this list before
/// calling [`set_font_directories`] so it can restore prior state if the
/// reconfig fails.
pub fn current_font_directories() -> Vec<PathBuf> {
    FONT_CONFIG
        .lock()
        .map(|cfg| cfg.font_dirs.clone())
        .unwrap_or_default()
}

/// Return the Google Fonts LRU cache cap last applied via
/// [`apply_hot_font_cache`].
///
/// Returns `None` when no cap has ever been applied (fresh process state).
/// Callers that care about the library's default behavior should treat
/// `None` as "library default" and write back
/// [`DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB`] when restoring.
pub fn current_cache_size() -> Option<NonZeroU64> {
    NonZeroU64::new(CURRENT_CACHE_SIZE_MB.load(Ordering::Acquire))
}

/// Actively set the Google Fonts LRU cache cap.
///
/// Unlike [`configure_font_cache`] which silently ignores `None`, this
/// function **actively resets the cap to the library default**
/// ([`DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB`]) when called with `None`. This
/// is required by the admin-reconfig rollback path: snapshot
/// `current_cache_size()` → attempt rebuild → restore via
/// `apply_hot_font_cache(prior)` where `prior` may be `None` if the cap
/// had never been customized. Silent-ignore semantics would leak the
/// new cap across a failed rebuild.
///
/// The function immediately evicts cached fonts if the new limit is
/// exceeded.
pub fn apply_hot_font_cache(cap_mb: Option<NonZeroU64>) -> Result<(), anyhow::Error> {
    let bytes_per_mb: u64 = 1024 * 1024;
    let cap_mb_value = cap_mb
        .map(NonZeroU64::get)
        .unwrap_or(DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB);
    let bytes = cap_mb_value.saturating_mul(bytes_per_mb);
    GOOGLE_FONTS_CLIENT.set_max_font_cache_bytes(bytes)?;
    CURRENT_CACHE_SIZE_MB.store(cap_mb_value, Ordering::Release);
    Ok(())
}

/// Configure the max on-disk Google Fonts cache size in bytes.
///
/// `None` keeps the existing configured value. Immediately evicts cached
/// fonts if the new limit is exceeded.
///
/// # Deprecation
///
/// Prefer [`apply_hot_font_cache`], which takes an `Option<NonZeroU64>`
/// (MB, type-checked) and actively resets to the library default when
/// called with `None`. This function remains for backward compatibility.
pub fn configure_font_cache(max_cache_bytes: Option<u64>) -> Result<(), anyhow::Error> {
    if let Some(bytes) = max_cache_bytes {
        GOOGLE_FONTS_CLIENT.set_max_font_cache_bytes(bytes)?;
        let bytes_per_mb: u64 = 1024 * 1024;
        let cap_mb = bytes / bytes_per_mb;
        CURRENT_CACHE_SIZE_MB.store(cap_mb, Ordering::Release);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn set_font_directories_replaces_existing() {
        // Snapshot prior state so we restore it after the test — the
        // global is shared across tests.
        let prior = current_font_directories();

        let tmp = tempfile::tempdir().unwrap();
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();

        set_font_directories(std::slice::from_ref(&dir_a)).unwrap();
        let after_a = current_font_directories();
        assert!(after_a.iter().any(|p| p == &dir_a));
        assert!(!after_a.iter().any(|p| p == &dir_b));

        // Replace — dir_a must be gone, dir_b present
        set_font_directories(std::slice::from_ref(&dir_b)).unwrap();
        let after_b = current_font_directories();
        assert!(
            !after_b.iter().any(|p| p == &dir_a),
            "dir_a must be dropped on replace: got {after_b:?}"
        );
        assert!(after_b.iter().any(|p| p == &dir_b));

        // Restore
        set_font_directories(&prior).unwrap();
    }

    #[test]
    fn apply_hot_font_cache_none_resets_to_default() {
        apply_hot_font_cache(NonZeroU64::new(17)).unwrap();
        let after_explicit = current_cache_size();
        assert_eq!(after_explicit, NonZeroU64::new(17));

        apply_hot_font_cache(None).unwrap();
        let after_reset = current_cache_size();
        assert_eq!(
            after_reset,
            NonZeroU64::new(DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB),
            "apply_hot_font_cache(None) must actively reset to the library default"
        );
    }
}

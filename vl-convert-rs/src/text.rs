use crate::anyhow;
use crate::anyhow::anyhow;
use crate::image_loading::custom_string_resolver;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use usvg::fontdb::Database;
use usvg::{
    FallbackSelectionFn, FontFamily, FontResolver, FontSelectionFn, FontStretch, FontStyle,
    ImageHrefResolver,
};
use vl_convert_canvas2d::font_config::{CustomFont, FontConfig, ResolvedFontConfig};
use vl_convert_google_fonts::{GoogleFontsClient, LoadedFontBatch, VariantRequest};

/// Monotonically increasing version counter for font configuration changes.
/// Incremented each time font configuration is modified.
pub static FONT_CONFIG_VERSION: AtomicU64 = AtomicU64::new(0);

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

        log::warn!(
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

            log::warn!("Fallback from {} to {}.", base_family.0, new_family.0);
            return Some(face.id);
        }

        None
    })
}

pub fn register_font_directory(dir: &str) -> Result<(), anyhow::Error> {
    {
        let mut font_config = FONT_CONFIG
            .lock()
            .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?;
        font_config.font_dirs.push(PathBuf::from(dir));
    }

    refresh_font_baseline_after_config_update()
}

fn collect_custom_fonts_from_batch(batch: &LoadedFontBatch) -> Vec<CustomFont> {
    batch
        .font_data()
        .iter()
        .map(|data| CustomFont {
            data: Arc::clone(data),
            family_name: None,
        })
        .collect()
}

/// Download and install a font by family name from Google Fonts.
///
/// Google Fonts TTF files are loaded into `fontdb` as in-memory binary sources.
/// The same bytes are also appended to `FONT_CONFIG.custom_fonts` so worker
/// font refreshes keep the newly-installed fonts.
pub async fn register_google_fonts_font(
    family: &str,
    variants: Option<&[VariantRequest]>,
) -> Result<(), anyhow::Error> {
    let batch = GOOGLE_FONTS_CLIENT.load(family, variants).await?;
    let loaded_custom_fonts = collect_custom_fonts_from_batch(&batch);

    {
        let mut font_config = FONT_CONFIG
            .lock()
            .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?;
        font_config.custom_fonts.extend(loaded_custom_fonts);
    }

    refresh_font_baseline_after_config_update()
}

/// Blocking variant of [`register_google_fonts_font`].
pub fn register_google_fonts_font_blocking(
    family: &str,
    variants: Option<&[VariantRequest]>,
) -> Result<(), anyhow::Error> {
    let batch = GOOGLE_FONTS_CLIENT.load_blocking(family, variants)?;
    let loaded_custom_fonts = collect_custom_fonts_from_batch(&batch);

    {
        let mut font_config = FONT_CONFIG
            .lock()
            .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?;
        font_config.custom_fonts.extend(loaded_custom_fonts);
    }

    refresh_font_baseline_after_config_update()
}

/// Configure the max on-disk Google Fonts cache size in bytes.
///
/// `None` keeps the existing configured value.
pub fn configure_font_cache(max_cache_bytes: Option<u64>) {
    if let Some(bytes) = max_cache_bytes {
        GOOGLE_FONTS_CLIENT.set_max_blob_cache_bytes(bytes);
    }
}

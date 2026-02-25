use crate::anyhow;
use crate::anyhow::anyhow;
use crate::image_loading::custom_string_resolver;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use usvg::fontdb::Database;
use usvg::{
    FallbackSelectionFn, FontFamily, FontResolver, FontSelectionFn, FontStretch, FontStyle,
    ImageHrefResolver,
};
use vl_convert_canvas2d::font_config::{font_config_to_fontdb, CustomFont, FontConfig};
use vl_convert_fontsource::FontsourceCache;

/// Monotonically increasing version counter for font configuration changes.
/// Incremented each time `register_font_directory` is called.
pub static FONT_CONFIG_VERSION: AtomicU64 = AtomicU64::new(0);

lazy_static! {
    pub static ref USVG_OPTIONS: Mutex<usvg::Options<'static>> = Mutex::new(init_usvg_options());
    pub static ref FONT_CONFIG: Mutex<FontConfig> = Mutex::new(build_default_font_config());
    pub static ref FONTSOURCE_CACHE: FontsourceCache =
        FontsourceCache::new(None, None).expect("Failed to initialize FontsourceCache");
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

fn init_usvg_options() -> usvg::Options<'static> {
    let image_href_resolver = ImageHrefResolver {
        resolve_string: custom_string_resolver(),
        ..Default::default()
    };

    let font_resolver = FontResolver {
        select_font: custom_font_selector(),
        select_fallback: custom_fallback_selector(),
    };

    let font_config = build_default_font_config();
    let fontdb = font_config_to_fontdb(&font_config);

    usvg::Options {
        image_href_resolver,
        fontdb: Arc::new(fontdb),
        font_resolver,
        ..Default::default()
    }
}

fn setup_default_fonts(fontdb: &mut Database) {
    // Collect set of system font families
    let families: HashSet<String> = fontdb
        .faces()
        .flat_map(|face| {
            face.families
                .iter()
                .map(|(fam, _lang)| fam.clone())
                .collect::<Vec<_>>()
        })
        .collect();

    for family in ["Arial", "Helvetica", "Liberation Sans"] {
        if families.contains(family) {
            fontdb.set_sans_serif_family(family);
            break;
        }
    }

    // Set default monospace font family
    for family in [
        "Courier New",
        "Courier",
        "Liberation Mono",
        "DejaVu Sans Mono",
    ] {
        if families.contains(family) {
            fontdb.set_monospace_family(family);
            break;
        }
    }

    // Set default serif font family
    for family in [
        "Times New Roman",
        "Times",
        "Liberation Serif",
        "DejaVu Serif",
    ] {
        if families.contains(family) {
            fontdb.set_serif_family(family);
            break;
        }
    }
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
    // Update FONT_CONFIG so new canvas contexts see the registered directory
    {
        let mut font_config = FONT_CONFIG
            .lock()
            .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?;
        font_config.font_dirs.push(PathBuf::from(dir));
    }

    // Update USVG_OPTIONS fontdb incrementally (no full rebuild)
    {
        let mut opts = USVG_OPTIONS
            .lock()
            .map_err(|err| anyhow!("Failed to acquire usvg options lock: {err}"))?;

        // Get mutable reference to font_db. Use Arc::make_mut which will clone the
        // database if there are other references (e.g., from canvas contexts).
        let font_db = Arc::make_mut(&mut opts.fontdb);

        // Load fonts incrementally
        font_db.load_fonts_dir(dir);
        setup_default_fonts(font_db);
    }

    // Bump version so the shared worker knows to refresh its cached SharedFontConfig
    FONT_CONFIG_VERSION.fetch_add(1, Ordering::Release);

    Ok(())
}

/// Result of attempting to register a font directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterResult {
    /// The directory was newly registered.
    Registered,
    /// The directory was already registered.
    AlreadyRegistered,
    /// The directory does not exist or contains no `.ttf` files.
    DirectoryMissing,
}

/// Register a font directory if it has not already been registered and
/// contains at least one `.ttf` file.
///
/// This function acquires the `FONT_CONFIG` and `USVG_OPTIONS` locks
/// internally. It is intended to be called from within
/// `FONTSOURCE_CACHE.with_cache_lock(...)` so that the filesystem state
/// is stable while we check for TTF files.
pub fn register_font_directory_if_new(dir: &str) -> Result<RegisterResult, anyhow::Error> {
    let path = PathBuf::from(dir);

    // Hold FONT_CONFIG lock for the entire check+register sequence to prevent
    // duplicate entries from concurrent callers.
    let mut font_config = FONT_CONFIG
        .lock()
        .map_err(|err| anyhow!("Failed to acquire font config lock: {err}"))?;

    if font_config.font_dirs.contains(&path) {
        return Ok(RegisterResult::AlreadyRegistered);
    }

    // Check directory exists and has at least one .ttf file
    if !path.is_dir() {
        return Ok(RegisterResult::DirectoryMissing);
    }
    let has_ttf = std::fs::read_dir(&path)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("ttf"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if !has_ttf {
        return Ok(RegisterResult::DirectoryMissing);
    }

    // Register the directory (still under the same lock)
    font_config.font_dirs.push(path);
    drop(font_config);

    {
        let mut opts = USVG_OPTIONS
            .lock()
            .map_err(|err| anyhow!("Failed to acquire usvg options lock: {err}"))?;
        let font_db = Arc::make_mut(&mut opts.fontdb);
        font_db.load_fonts_dir(dir);
        setup_default_fonts(font_db);
    }

    FONT_CONFIG_VERSION.fetch_add(1, Ordering::Release);

    Ok(RegisterResult::Registered)
}

/// Fetch a font from Fontsource, register it with fontdb, and handle stale-cache recovery.
///
/// Returns the `FetchOutcome` on success. If the cache directory is missing after
/// the initial fetch (stale cache), performs a forced re-download and retries registration.
pub(crate) async fn fetch_and_register_font(
    family: &str,
) -> Result<vl_convert_fontsource::FetchOutcome, anyhow::Error> {
    let mut outcome = FONTSOURCE_CACHE.fetch(family).await?;
    let dir_str = outcome
        .path
        .to_str()
        .ok_or_else(|| anyhow!("Font path is not valid UTF-8"))?
        .to_string();

    let result = FONTSOURCE_CACHE.with_cache_lock(|| register_font_directory_if_new(&dir_str))?;
    let result = result?;

    if result == RegisterResult::DirectoryMissing {
        // Stale cache — force re-download
        outcome = FONTSOURCE_CACHE.refetch(family).await?;
        let dir_str = outcome
            .path
            .to_str()
            .ok_or_else(|| anyhow!("Font path is not valid UTF-8"))?
            .to_string();

        let result =
            FONTSOURCE_CACHE.with_cache_lock(|| register_font_directory_if_new(&dir_str))?;
        let result = result?;

        if result == RegisterResult::DirectoryMissing {
            return Err(anyhow!(
                "Font directory for '{}' is missing after re-download",
                family
            ));
        }
    }

    Ok(outcome)
}

/// Download and install a font by family name from Fontsource.
///
/// Uses the global `FONTSOURCE_CACHE` to fetch the font, then registers
/// the font directory in the fontdb. If the directory appears missing
/// after a cache hit (stale cache), performs a forced re-download.
pub async fn install_font(family: &str) -> Result<(), anyhow::Error> {
    let outcome = fetch_and_register_font(family).await?;

    // Evict LRU fonts if cache limit is set and a download occurred
    if outcome.downloaded {
        let max_bytes = FONTSOURCE_CACHE.max_cache_bytes();
        if max_bytes > 0 {
            let exempt = HashSet::from([outcome.font_id.clone()]);
            FONTSOURCE_CACHE.evict_lru_until_size(max_bytes, &exempt)?;
        }
    }

    Ok(())
}

/// Configure the maximum size of the Fontsource font cache.
///
/// Pass `None` or `Some(0)` to disable cache size limits (unbounded).
pub fn configure_font_cache(max_cache_bytes: Option<u64>) {
    FONTSOURCE_CACHE.set_max_cache_bytes(max_cache_bytes.unwrap_or(0));
}

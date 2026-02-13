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

/// Monotonically increasing version counter for font configuration changes.
/// Incremented each time `register_font_directory` is called.
pub static FONT_CONFIG_VERSION: AtomicU64 = AtomicU64::new(0);

lazy_static! {
    pub static ref USVG_OPTIONS: Mutex<usvg::Options<'static>> = Mutex::new(init_usvg_options());
    pub static ref FONT_CONFIG: Mutex<FontConfig> = Mutex::new(build_default_font_config());
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

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

/// Backend-agnostic font configuration.
///
/// This struct describes desired font configuration using only Rust standard library types,
/// with no dependencies on any specific rendering backend (fontdb, cosmic-text, Fontique, etc.).
/// Each backend converts it into its own internal representation via a conversion function
/// (e.g., [`font_config_to_fontdb`]).
#[derive(Clone, Debug)]
pub struct FontConfig {
    /// Custom font data to register (font file bytes + optional family override).
    pub custom_fonts: Vec<CustomFont>,
    /// Mappings from generic CSS family names to concrete font family names.
    pub generic_families: GenericFamilyMap,
    /// Whether to load system fonts (default: true).
    pub load_system_fonts: bool,
    /// Additional directories to scan for font files.
    pub font_dirs: Vec<PathBuf>,
    /// Whether font hinting is enabled for text rendering (default: false).
    ///
    /// Hinting adjusts glyph outlines to align with the pixel grid, improving
    /// legibility at small sizes on low-DPI screens. Disabled by default to match
    /// SVG text rendering behavior (usvg/resvg do not apply hinting).
    pub hinting_enabled: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            custom_fonts: Vec::new(),
            generic_families: GenericFamilyMap::defaults(),
            load_system_fonts: true,
            font_dirs: Vec::new(),
            hinting_enabled: false,
        }
    }
}

/// A custom font to register, consisting of raw font file data and an optional family name override.
#[derive(Clone, Debug)]
pub struct CustomFont {
    /// Raw font file data (TTF/OTF/WOFF). Arc-wrapped for cheap cloning.
    pub data: Arc<Vec<u8>>,
    /// Optional family name override. If None, the family name is read from
    /// the font's name table.
    pub family_name: Option<String>,
}

/// Mappings from generic CSS family names to concrete font family names, in priority order.
#[derive(Clone, Debug, Default)]
pub struct GenericFamilyMap {
    /// Concrete fonts for CSS "serif" (priority order).
    pub serif: Vec<String>,
    /// Concrete fonts for CSS "sans-serif" (priority order).
    pub sans_serif: Vec<String>,
    /// Concrete fonts for CSS "monospace" (priority order).
    pub monospace: Vec<String>,
    /// Concrete fonts for CSS "cursive" (priority order).
    pub cursive: Vec<String>,
    /// Concrete fonts for CSS "fantasy" (priority order).
    pub fantasy: Vec<String>,
}

impl GenericFamilyMap {
    /// Returns the default generic family mappings matching browser behavior.
    ///
    /// These preference lists match the existing `setup_default_fonts()` logic
    /// in both `vl-convert-canvas2d` and `vl-convert-rs`.
    pub fn defaults() -> Self {
        Self {
            sans_serif: vec!["Arial".into(), "Helvetica".into(), "Liberation Sans".into()],
            monospace: vec![
                "Courier New".into(),
                "Courier".into(),
                "Liberation Mono".into(),
                "DejaVu Sans Mono".into(),
            ],
            serif: vec![
                "Times New Roman".into(),
                "Times".into(),
                "Liberation Serif".into(),
                "DejaVu Serif".into(),
            ],
            cursive: vec!["Comic Sans MS".into(), "Apple Chancery".into()],
            fantasy: vec!["Impact".into(), "Papyrus".into()],
        }
    }
}

impl FontConfig {
    /// Resolve this configuration into a concrete font database.
    ///
    /// This performs the expensive work (system font scanning, directory loading,
    /// custom font registration) once. The resulting [`ResolvedFontConfig`] can be
    /// shared and cloned cheaply to create multiple canvas contexts without
    /// repeating the filesystem scan.
    pub fn resolve(&self) -> ResolvedFontConfig {
        ResolvedFontConfig::new(self)
    }
}

/// A [`FontConfig`] that has been resolved into a concrete font database.
///
/// This is an opaque wrapper — the rendering backend (`fontdb`) does not leak
/// through the public API. Create one via [`FontConfig::resolve()`] or
/// [`ResolvedFontConfig::new()`], then pass it to
/// [`Canvas2dContext::with_resolved()`](crate::Canvas2dContext::with_resolved).
///
/// Cloning a `ResolvedFontConfig` clones the underlying database in memory
/// (no filesystem scan), making it suitable for sharing across canvas contexts.
pub struct ResolvedFontConfig {
    pub(crate) fontdb: fontdb::Database,
    pub(crate) hinting_enabled: bool,
}

impl ResolvedFontConfig {
    /// Resolve a [`FontConfig`] into a concrete font database.
    ///
    /// This performs system font scanning, directory loading, and custom font
    /// registration — the same work as [`font_config_to_fontdb`], cached in an
    /// opaque wrapper.
    pub fn new(config: &FontConfig) -> Self {
        Self {
            fontdb: font_config_to_fontdb(config),
            hinting_enabled: config.hinting_enabled,
        }
    }
}

/// Convert a [`FontConfig`] into a [`fontdb::Database`].
///
/// This is the single point where font configuration is translated into the fontdb backend.
/// It replaces the duplicated `setup_default_fonts()` logic that previously existed in
/// both `vl-convert-canvas2d` and `vl-convert-rs`.
pub fn font_config_to_fontdb(config: &FontConfig) -> fontdb::Database {
    let mut db = fontdb::Database::new();

    // Load system fonts if requested
    if config.load_system_fonts {
        db.load_system_fonts();
    }

    // Scan additional font directories
    for dir in &config.font_dirs {
        db.load_fonts_dir(dir);
    }

    // Load custom font data
    for font in &config.custom_fonts {
        db.load_font_data(Vec::from(font.data.as_slice()));
    }

    // Apply generic family mappings
    apply_generic_families(&mut db, &config.generic_families);

    db
}

/// Apply generic family mappings to a fontdb database, choosing the first available
/// family from each priority list.
fn apply_generic_families(db: &mut fontdb::Database, families: &GenericFamilyMap) {
    // Collect all available font family names
    let available: HashSet<String> = db
        .faces()
        .flat_map(|face| {
            face.families
                .iter()
                .map(|(fam, _lang)| fam.clone())
                .collect::<Vec<_>>()
        })
        .collect();

    // Sans-serif
    for family in &families.sans_serif {
        if available.contains(family) {
            db.set_sans_serif_family(family);
            break;
        }
    }

    // Monospace
    for family in &families.monospace {
        if available.contains(family) {
            db.set_monospace_family(family);
            break;
        }
    }

    // Serif
    for family in &families.serif {
        if available.contains(family) {
            db.set_serif_family(family);
            break;
        }
    }

    // Cursive
    for family in &families.cursive {
        if available.contains(family) {
            db.set_cursive_family(family);
            break;
        }
    }

    // Fantasy
    for family in &families.fantasy {
        if available.contains(family) {
            db.set_fantasy_family(family);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_font_config() {
        let config = FontConfig::default();
        assert!(config.custom_fonts.is_empty());
        assert!(config.load_system_fonts);
        assert!(config.font_dirs.is_empty());
        assert_eq!(config.generic_families.sans_serif[0], "Arial");
        assert!(!config.hinting_enabled);
    }

    #[test]
    fn test_generic_family_defaults() {
        let defaults = GenericFamilyMap::defaults();
        assert_eq!(
            defaults.sans_serif,
            vec!["Arial", "Helvetica", "Liberation Sans"]
        );
        assert_eq!(
            defaults.monospace,
            vec![
                "Courier New",
                "Courier",
                "Liberation Mono",
                "DejaVu Sans Mono"
            ]
        );
        assert_eq!(
            defaults.serif,
            vec![
                "Times New Roman",
                "Times",
                "Liberation Serif",
                "DejaVu Serif"
            ]
        );
        assert_eq!(defaults.cursive, vec!["Comic Sans MS", "Apple Chancery"]);
        assert_eq!(defaults.fantasy, vec!["Impact", "Papyrus"]);
    }

    #[test]
    fn test_font_config_to_fontdb_no_system_fonts() {
        let config = FontConfig {
            load_system_fonts: false,
            ..FontConfig::default()
        };
        let db = font_config_to_fontdb(&config);
        // With no system fonts and no custom fonts, database should have no faces
        assert_eq!(db.faces().count(), 0);
    }

    #[test]
    fn test_font_config_clone_is_cheap() {
        let data = Arc::new(vec![0u8; 1000]);
        let font = CustomFont {
            data: data.clone(),
            family_name: None,
        };
        let config = FontConfig {
            custom_fonts: vec![font],
            ..FontConfig::default()
        };
        let cloned = config.clone();
        // Arc should share the same allocation
        assert!(Arc::ptr_eq(
            &config.custom_fonts[0].data,
            &cloned.custom_fonts[0].data
        ));
    }
}

//! Font loading for glyph layout and rasterization.
//!
//! The crate does not ship any font. Callers must provide two font files:
//! an ASCII (English) font that drives the character-cell metrics, and a
//! non-ASCII font (e.g. Source Han Sans CN for CJK) used as a fallback for
//! glyphs the ASCII font lacks. This mirrors how `code-image` accepts
//! separate `ascii_font_path` / `non_ascii_font_path` from the caller.
//!
//! Layout goes through cosmic-text, which automatically picks a font that
//! covers each glyph: the primary font drives cell metrics, and any fallback
//! fonts registered on the same [`Font`] supply glyphs the primary font lacks.

use std::path::Path;
use std::sync::{Arc, Mutex};

use cosmic_text::fontdb;
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Weight};

/// A loaded set of fonts ready for glyph lookup and rasterization.
///
/// The first font (the ASCII/English font) is the primary font and drives
/// cell metrics ([`Font::advance`], [`Font::line_height`]); the second font
/// (the non-ASCII font) is consulted during layout whenever the primary font
/// does not cover a glyph.
#[derive(Clone)]
pub struct Font {
    pub(crate) font_system: Arc<Mutex<FontSystem>>,
    pub(crate) primary_family: String,
    /// All loaded font data (first entry is the primary font). Kept so
    /// [`Font::with_fallback`] can rebuild the font system.
    font_data: Vec<Vec<u8>>,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("primary_family", &self.primary_family)
            .field("font_count", &self.font_data.len())
            .finish()
    }
}

fn build_system(font_data: &[Vec<u8>]) -> Result<(FontSystem, String), String> {
    let mut db = fontdb::Database::new();
    let mut primary_family = None;
    for (index, bytes) in font_data.iter().enumerate() {
        db.load_font_data(bytes.clone());
        if index == 0 {
            let family = db
                .faces()
                .next()
                .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
                .ok_or_else(|| "font has no family name".to_string())?;
            primary_family = Some(family);
        }
    }
    let family = primary_family.ok_or_else(|| "no fonts loaded".to_string())?;
    let system = FontSystem::new_with_locale_and_db("en-US".to_string(), db);
    Ok((system, family))
}

impl Font {
    /// Build a font set from raw TTF/OTF bytes. The first font is the primary
    /// (ASCII/English) font and drives cell metrics; the second is the
    /// non-ASCII fallback (e.g. a CJK font). Fails if the bytes are not a
    /// font cosmic-text understands (true/type, open/type, and a few others).
    pub fn from_bytes(ascii: &[u8], non_ascii: &[u8]) -> Result<Self, String> {
        let font_data = vec![ascii.to_vec(), non_ascii.to_vec()];
        let (font_system, primary_family) = build_system(&font_data)?;
        Ok(Font {
            font_system: Arc::new(Mutex::new(font_system)),
            primary_family,
            font_data,
        })
    }

    /// Build a font set from the ASCII font file and the non-ASCII font file.
    pub fn from_files(ascii: impl AsRef<Path>, non_ascii: impl AsRef<Path>) -> Result<Self, String> {
        let ascii_bytes = std::fs::read(ascii)
            .map_err(|e| format!("failed to read ascii font file: {e}"))?;
        let non_ascii_bytes = std::fs::read(non_ascii)
            .map_err(|e| format!("failed to read non-ascii font file: {e}"))?;
        Font::from_bytes(&ascii_bytes, &non_ascii_bytes)
    }

    /// Register an additional fallback font used when the primary font lacks
    /// a glyph. Fallbacks are consulted in the order they are added.
    pub fn with_fallback(mut self, bytes: Vec<u8>) -> Result<Self, String> {
        self.font_data.push(bytes);
        let (font_system, _) = build_system(&self.font_data)?;
        self.font_system = Arc::new(Mutex::new(font_system));
        Ok(self)
    }

    /// Load an additional fallback font from a file.
    pub fn with_fallback_file(self, path: impl AsRef<Path>) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("failed to read font file: {e}"))?;
        self.with_fallback(bytes)
    }

    /// Horizontal advance (pixels) of the widest glyph at `size` points — for
    /// monospace fonts this is the advance of every glyph.
    pub fn advance(&self, size: f32) -> f32 {
        let mut system = self.font_system.lock().expect("font system poisoned");
        let attrs = Attrs::new().family(cosmic_text::Family::Name(&self.primary_family));
        let mut buffer = Buffer::new(&mut system, Metrics::new(size, size));
        buffer.set_text("M", &attrs, Shaping::Basic, None);
        buffer.shape_until_scroll(&mut system, false);
        buffer
            .layout_runs()
            .next()
            .map(|line| line.line_w)
            .unwrap_or(0.0)
    }

    /// Line height (ascent - descent) at `size` points.
    pub fn line_height(&self, size: f32) -> f32 {
        let mut system = self.font_system.lock().expect("font system poisoned");
        let Some(font) = system
            .db()
            .query(&fontdb::Query {
                families: &[fontdb::Family::Name(&self.primary_family)],
                ..Default::default()
            })
            .and_then(|id| system.get_font(id, Weight::NORMAL))
        else {
            return size;
        };
        let metrics = font.as_swash().metrics(&[]);
        let scale = size / metrics.units_per_em as f32;
        // swash reports descent as a positive distance below the baseline.
        (metrics.ascent + metrics.descent) * scale
    }

    /// Whether this glyph exists in any loaded font (missing glyphs render as
    /// nothing rather than tofu).
    pub fn has_glyph(&self, c: char, _size: f32) -> bool {
        let mut system = self.font_system.lock().expect("font system poisoned");
        let ids: Vec<_> = system.db().faces().map(|face| face.id).collect();
        ids.into_iter().any(|id| {
            system
                .get_font(id, Weight::NORMAL)
                .is_some_and(|font| font.as_swash().charmap().map(c) != 0)
        })
    }
}

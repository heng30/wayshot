use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, fontdb::Weight,
};
use std::path::Path;

fn main() {
    env_logger::init();

    println!("=== Font Metrics Diagnostic ===\n");

    let sans_font_path = "../../wayshot/ui/fonts/SourceHanSansCN.otf";
    let serif_font_path = "../../wayshot/ui/fonts/SourceHanSerifCN.ttf";

    let font_size = 48.0f32;
    let line_height = font_size * 1.2;

    // Test both fonts
    for (font_name, font_path) in [
        ("SourceHanSansCN.otf", sans_font_path),
        ("SourceHanSerifCN.ttf", serif_font_path),
    ] {
        println!("=== {} ===\n", font_name);

        if !Path::new(font_path).exists() {
            println!("Font not found: {}\n", font_path);
            continue;
        }

        let mut font_system = FontSystem::new();

        // Load font
        font_system
            .db_mut()
            .load_font_file(font_path)
            .expect("Failed to load font");

        let all_faces: Vec<&cosmic_text::fontdb::FaceInfo> = font_system.db().faces().collect();
        let face_info = all_faces.iter().last().unwrap();
        let font_family = face_info.families.first().unwrap().0.clone();

        println!("Font family: {}", font_family);

        // Get font metrics directly from fontdb
        let font_id = face_info.id;
        if let Some(font) = font_system.get_font(font_id, Weight(400)) {
            let metrics = font.metrics();
            println!("Font metrics (font units):");
            println!("  units_per_em: {}", metrics.units_per_em);
            println!("  ascent:       {}", metrics.ascent);
            println!("  descent:      {}", metrics.descent);

            // Calculate pixel values
            let scale = font_size / metrics.units_per_em as f32;
            println!("Scale factor: {} (font_size / units_per_em)", scale);
            println!("Font metrics (pixels at font_size={}):", font_size);
            println!("  ascent:       {} px", metrics.ascent * scale);
            println!("  descent:      {} px", metrics.descent * scale);
            println!("  font_height:  {} px (ascent - descent)", (metrics.ascent - metrics.descent) * scale);
        }

        // Create buffer with test text (including punctuation)
        let test_text = "你好，世界。测试、验证";
        let attrs = Attrs::new().family(Family::Name(&font_family));
        let metrics = Metrics { font_size, line_height };

        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_text(test_text, &attrs, Shaping::Basic, None);
        buffer.shape_until_scroll(&mut font_system, false);

        // Track glyph extents for new calculation
        let mut max_glyph_top: f32 = 0.0;
        let mut min_glyph_bottom: f32 = 0.0;

        println!("\nGlyph layout info:");
        println!("Text: {}", test_text);

        let mut swash_cache = SwashCache::new();

        for run in buffer.layout_runs() {
            println!("  line_w: {}", run.line_w);

            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);

                // Get glyph image to see placement info
                if let Some(glyph_img) = swash_cache.get_image(&mut font_system, physical.cache_key) {
                    let placement = &glyph_img.placement;

                    // Get the character
                    let ch_str: String = test_text.chars().skip(glyph.start).take(glyph.end - glyph.start).collect();

                    println!("  Glyph '{}':", ch_str);
                    println!("    physical.x: {}, physical.y: {}", physical.x, physical.y);
                    println!("    placement.left: {}, placement.top: {}", placement.left, placement.top);
                    println!("    placement.width: {}, placement.height: {}", placement.width, placement.height);
                    println!("    Calculated Y position (relative to baseline): physical.y + placement.top = {}",
                        physical.y + placement.top);

                    // Track glyph extents
                    let glyph_top = placement.top as f32;
                    let glyph_bottom = (placement.top as f32) - (placement.height as f32);
                    max_glyph_top = max_glyph_top.max(glyph_top);
                    min_glyph_bottom = min_glyph_bottom.min(glyph_bottom);
                }
            }
        }

        // Calculate current baseline_offset
        if let Some(font) = font_system.get_font(font_id, Weight(400)) {
            let font_metrics = font.metrics();
            let scale = font_size / font_metrics.units_per_em as f32;
            let font_ascent = font_metrics.ascent * scale;
            let font_descent = font_metrics.descent * scale;
            let font_height = font_ascent - font_descent;
            let extra_space = line_height - font_height;
            let baseline_offset = (extra_space / 2.0 + font_ascent).ceil() as i32;

            println!("\nBaseline offset calculation (OLD - font metrics based):");
            println!("  font_height = ascent - descent = {} - {} = {}",
                font_ascent, font_descent, font_height);
            println!("  extra_space = line_height - font_height = {} - {} = {}",
                line_height, font_height, extra_space);
            println!("  baseline_offset = (extra_space/2 + ascent) = ({}/2 + {}) = {} -> ceil = {}",
                extra_space, font_ascent, extra_space/2.0 + font_ascent, baseline_offset);

            // NEW calculation based on glyph extents
            println!("\nBaseline offset calculation (NEW - glyph extents based):");
            println!("  max_glyph_top = {}", max_glyph_top);
            println!("  min_glyph_bottom = {}", min_glyph_bottom);
            println!("  glyph_height = max_glyph_top - min_glyph_bottom = {} - {} = {}",
                max_glyph_top, min_glyph_bottom, max_glyph_top - min_glyph_bottom);
            let glyph_height = max_glyph_top - min_glyph_bottom;
            let extra_space_glyph = line_height - glyph_height;
            let baseline_offset_glyph = (extra_space_glyph / 2.0 + max_glyph_top).ceil() as i32;
            println!("  extra_space = line_height - glyph_height = {} - {} = {}",
                line_height, glyph_height, extra_space_glyph);
            println!("  baseline_offset = (extra_space/2 + max_glyph_top) = ({}/2 + {}) = {} -> ceil = {}",
                extra_space_glyph, max_glyph_top, extra_space_glyph/2.0 + max_glyph_top, baseline_offset_glyph);
        }

        println!("\n");
    }

    println!("=== Analysis ===");
    println!("The issue: Chinese punctuation marks (，。、) should appear at text bottom,");
    println!("but with SourceHanSansCN.otf they appear vertically centered.");
    println!("\nCompare the physical.y and placement.top values for punctuation glyphs");
    println!("between the two fonts to identify the difference.");
}
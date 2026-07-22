use image::{Rgba, RgbaImage};

fn draw_pixel_aa(buffer: &mut RgbaImage, x: u32, y: u32, color: &Rgba<u8>, opacity: f32) {
    if x >= buffer.width() || y >= buffer.height() {
        return;
    }

    // Compute source alpha: opacity * color's alpha channel
    let src_alpha = opacity * (color[3] as f32 / 255.0);
    if src_alpha <= 0.0 {
        return;
    }

    let pixel = buffer.get_pixel_mut(x, y);
    let dst_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha > 0.0 {
        // Proper "over" compositing with premultiplied alpha
        pixel[0] = ((color[0] as f32 * src_alpha + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[1] = ((color[1] as f32 * src_alpha + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[2] = ((color[2] as f32 * src_alpha + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[3] = (out_alpha * 255.0) as u8;
    }
}

/// Xiaolin Wu's anti-aliased line algorithm
/// Ideal for thin lines (<= 2px) - provides smooth rendering with proper sub-pixel accuracy
fn draw_line_wu(
    buffer: &mut RgbaImage,
    p1: (f32, f32),
    p2: (f32, f32),
    color: Rgba<u8>,
    line_width: f32,
) {
    let (mut x0, mut y0) = p1;
    let (mut x1, mut y1) = p2;

    // Width scale adjusts opacity to simulate thicker lines
    let width_scale = line_width.min(2.0);

    // Determine if line is steep (more vertical than horizontal)
    let steep = (y1 - y0).abs() > (x1 - x0).abs();

    if steep {
        // Swap x and y for steep lines
        std::mem::swap(&mut x0, &mut y0);
        std::mem::swap(&mut x1, &mut y1);
    }

    // Ensure we're drawing left to right
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }

    let dx = x1 - x0;
    let dy = y1 - y0;
    let gradient = if dx == 0.0 { 1.0 } else { dy / dx };

    // First endpoint
    let x_end = x0.round();
    let y_end = y0 + gradient * (x_end - x0);
    let x_gap = 1.0 - (x0 + 0.5).fract();

    let x_px1 = x_end as i32;
    let y_px1 = y_end.trunc() as i32;
    let y_frac1 = y_end - y_end.trunc();

    if steep {
        // Plot for steep line (swapped coordinates)
        if x_px1 >= 0 && y_px1 >= 0 {
            let x_u32 = y_px1 as u32;
            let y_u32 = x_px1 as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(
                    buffer,
                    x_u32,
                    y_u32,
                    &color,
                    (1.0 - y_frac1) * x_gap * width_scale,
                );
            }
        }
        if x_px1 >= 0 && y_px1 + 1 >= 0 {
            let x_u32 = (y_px1 + 1) as u32;
            let y_u32 = x_px1 as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(buffer, x_u32, y_u32, &color, y_frac1 * x_gap * width_scale);
            }
        }
    } else {
        // Plot for normal line
        if x_px1 >= 0 && y_px1 >= 0 {
            let x_u32 = x_px1 as u32;
            let y_u32 = y_px1 as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(
                    buffer,
                    x_u32,
                    y_u32,
                    &color,
                    (1.0 - y_frac1) * x_gap * width_scale,
                );
            }
        }
        if x_px1 >= 0 && y_px1 + 1 >= 0 {
            let x_u32 = x_px1 as u32;
            let y_u32 = (y_px1 + 1) as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(buffer, x_u32, y_u32, &color, y_frac1 * x_gap * width_scale);
            }
        }
    }

    // Second endpoint
    let x_end2 = x1.round();
    let y_end2 = y1 + gradient * (x_end2 - x1);
    let x_gap2 = (x1 + 0.5).fract();

    let x_px2 = x_end2 as i32;
    let y_px2 = y_end2.trunc() as i32;
    let y_frac2 = y_end2 - y_end2.trunc();

    if steep {
        // Plot for steep line
        if x_px2 >= 0 && y_px2 >= 0 {
            let x_u32 = y_px2 as u32;
            let y_u32 = x_px2 as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(
                    buffer,
                    x_u32,
                    y_u32,
                    &color,
                    (1.0 - y_frac2) * x_gap2 * width_scale,
                );
            }
        }
        if x_px2 >= 0 && y_px2 + 1 >= 0 {
            let x_u32 = (y_px2 + 1) as u32;
            let y_u32 = x_px2 as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(buffer, x_u32, y_u32, &color, y_frac2 * x_gap2 * width_scale);
            }
        }
    } else {
        // Plot for normal line
        if x_px2 >= 0 && y_px2 >= 0 {
            let x_u32 = x_px2 as u32;
            let y_u32 = y_px2 as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(
                    buffer,
                    x_u32,
                    y_u32,
                    &color,
                    (1.0 - y_frac2) * x_gap2 * width_scale,
                );
            }
        }
        if x_px2 >= 0 && y_px2 + 1 >= 0 {
            let x_u32 = x_px2 as u32;
            let y_u32 = (y_px2 + 1) as u32;
            if x_u32 < buffer.width() && y_u32 < buffer.height() {
                draw_pixel_aa(buffer, x_u32, y_u32, &color, y_frac2 * x_gap2 * width_scale);
            }
        }
    }

    // Main loop - draw line between endpoints
    let mut intery = y_end + gradient;
    for x in (x_px1 + 1)..x_px2 {
        if steep {
            // Plot for steep line (swapped coordinates)
            let y_floor = intery.trunc() as i32;
            let y_frac = intery - intery.trunc();

            if x >= 0 && y_floor >= 0 {
                let x_u32 = y_floor as u32;
                let y_u32 = x as u32;
                if x_u32 < buffer.width() && y_u32 < buffer.height() {
                    draw_pixel_aa(buffer, x_u32, y_u32, &color, (1.0 - y_frac) * width_scale);
                }
            }
            if x >= 0 && y_floor + 1 >= 0 {
                let x_u32 = (y_floor + 1) as u32;
                let y_u32 = x as u32;
                if x_u32 < buffer.width() && y_u32 < buffer.height() {
                    draw_pixel_aa(buffer, x_u32, y_u32, &color, y_frac * width_scale);
                }
            }
        } else {
            // Plot for normal line
            let y_floor = intery.trunc() as i32;
            let y_frac = intery - intery.trunc();

            if x >= 0 && y_floor >= 0 {
                let x_u32 = x as u32;
                let y_u32 = y_floor as u32;
                if x_u32 < buffer.width() && y_u32 < buffer.height() {
                    draw_pixel_aa(buffer, x_u32, y_u32, &color, (1.0 - y_frac) * width_scale);
                }
            }
            if x >= 0 && y_floor + 1 >= 0 {
                let x_u32 = x as u32;
                let y_u32 = (y_floor + 1) as u32;
                if x_u32 < buffer.width() && y_u32 < buffer.height() {
                    draw_pixel_aa(buffer, x_u32, y_u32, &color, y_frac * width_scale);
                }
            }
        }

        // Move along the gradient
        intery += gradient;
        // Handle NaN/Inf cases
        if intery.is_nan() || intery.is_infinite() {
            break;
        }
    }
}

fn point_to_segment_distance(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        // Degenerate segment (single point)
        let dist_x = px - x1;
        let dist_y = py - y1;
        return (dist_x * dist_x + dist_y * dist_y).sqrt();
    }

    // Project point onto line, clamped to segment
    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    // Closest point on segment
    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;

    // Distance to closest point
    let dist_x = px - closest_x;
    let dist_y = py - closest_y;
    (dist_x * dist_x + dist_y * dist_y).sqrt()
}

/// Draw anti-aliased line segment
/// Uses Wu's algorithm for thin lines (<= 2px) for optimal smoothness
/// Uses distance-based AA for thicker lines with reduced aa_range
pub fn draw_line_segment_aa(
    buffer: &mut RgbaImage,
    p1: (f32, f32),
    p2: (f32, f32),
    color: Rgba<u8>,
    line_width: f32,
) {
    if line_width <= 2.0 {
        // Use Wu's algorithm for thin lines - provides optimal 1px rendering
        draw_line_wu(buffer, p1, p2, color, line_width);
    } else {
        // Use distance-based AA for thicker lines with adjusted aa_range
        draw_thick_line_aa(buffer, p1, p2, color, line_width);
    }
}

/// Distance-based anti-aliased line for thicker lines (> 2px)
fn draw_thick_line_aa(
    buffer: &mut RgbaImage,
    p1: (f32, f32),
    p2: (f32, f32),
    color: Rgba<u8>,
    line_width: f32,
) {
    let (x1, y1) = p1;
    let (x2, y2) = p2;

    // Reduced anti-aliasing range for thicker lines (was 1.5, now proportional to line width)
    let aa_range: f32 = (line_width * 0.15).min(0.5).max(0.3);
    let half_width = line_width / 2.0;

    // Calculate bounding box with margin for AA
    let margin = (half_width + aa_range).ceil() as i32 + 1;
    let x_min = (x1.min(x2) - margin as f32).max(0.0) as u32;
    let x_max = (x1.max(x2) + margin as f32).min(buffer.width() as f32) as u32;
    let y_min = (y1.min(y2) - margin as f32).max(0.0) as u32;
    let y_max = (y1.max(y2) + margin as f32).min(buffer.height() as f32) as u32;

    // Iterate over pixels in bounding box
    for y in y_min..y_max {
        for x in x_min..x_max {
            // Use pixel center for accurate distance
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Calculate perpendicular distance to line
            let dist = point_to_segment_distance(px, py, x1, y1, x2, y2);

            // Calculate opacity based on distance from line center
            // Full opacity at center, fading at edges
            let dist_from_center = dist - half_width;

            if dist_from_center <= aa_range {
                let opacity = if dist_from_center <= 0.0 {
                    // Inside the line (full opacity for pixels fully covered)
                    1.0
                } else {
                    // Near outer edge: smooth fade out
                    1.0 - dist_from_center / aa_range
                };

                if opacity > 0.05 {
                    draw_pixel_aa(buffer, x, y, &color, opacity);
                }
            }
        }
    }
}

pub fn draw_line_segment_aa_thin(
    buffer: &mut RgbaImage,
    p1: (f32, f32),
    p2: (f32, f32),
    color: Rgba<u8>,
) {
    draw_line_segment_aa(buffer, p1, p2, color, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// Test that 1px horizontal line produces approximately 1 pixel width
    #[test]
    fn test_horizontal_line_width() {
        let mut buffer = RgbaImage::new(100, 10);
        let color = Rgba([255, 255, 255, 255]);

        // Draw horizontal line at y=5
        draw_line_segment_aa(&mut buffer, (10.0, 5.0), (90.0, 5.0), color, 1.0);

        // Count pixels with significant alpha (should be ~1 pixel wide)
        let mut max_width = 0;
        for x in 0..100 {
            let mut column_count = 0;
            for y in 0..10 {
                let pixel = buffer.get_pixel(x, y);
                if pixel[3] > 128 {
                    column_count += 1;
                }
            }
            max_width = max_width.max(column_count);
        }

        // With Wu algorithm, max width should be 2 pixels (due to AA coverage)
        assert!(
            max_width <= 3,
            "Line width {} pixels, expected <= 3",
            max_width
        );
    }

    /// Test that 1px diagonal line is smooth without jagged edges
    #[test]
    fn test_diagonal_line_smoothness() {
        let mut buffer = RgbaImage::new(50, 50);
        let color = Rgba([255, 255, 255, 255]);

        // Draw diagonal line
        draw_line_segment_aa(&mut buffer, (5.0, 5.0), (45.0, 45.0), color, 1.0);

        // Check that line has coverage (no gaps)
        let mut pixels_covered = 0;
        for y in 0..50 {
            for x in 0..50 {
                let pixel = buffer.get_pixel(x, y);
                if pixel[3] > 0 {
                    pixels_covered += 1;
                }
            }
        }

        // Diagonal line of ~40 length should have at least 40 pixels covered
        // (with AA, may have more due to anti-aliasing coverage)
        assert!(
            pixels_covered >= 40,
            "Pixels covered: {}, expected >= 40",
            pixels_covered
        );
    }

    /// Test steep line (more vertical)
    #[test]
    fn test_steep_line() {
        let mut buffer = RgbaImage::new(20, 100);
        let color = Rgba([255, 255, 255, 255]);

        // Draw steep vertical-ish line
        draw_line_segment_aa(&mut buffer, (10.0, 5.0), (12.0, 95.0), color, 1.0);

        // Check coverage
        let mut pixels_covered = 0;
        for y in 0..100 {
            for x in 0..20 {
                let pixel = buffer.get_pixel(x, y);
                if pixel[3] > 0 {
                    pixels_covered += 1;
                }
            }
        }

        // Line of ~90 length should have coverage
        assert!(
            pixels_covered >= 90,
            "Pixels covered: {}, expected >= 90",
            pixels_covered
        );
    }

    /// Test thick line uses distance-based algorithm
    #[test]
    fn test_thick_line() {
        let mut buffer = RgbaImage::new(100, 50);
        let color = Rgba([255, 255, 255, 255]);

        // Draw 5px thick horizontal line
        draw_line_segment_aa(&mut buffer, (10.0, 25.0), (90.0, 25.0), color, 5.0);

        // Count max width at center
        let mut max_width = 0;
        for x in 10..90 {
            let mut column_count = 0;
            for y in 0..50 {
                let pixel = buffer.get_pixel(x, y);
                if pixel[3] > 128 {
                    column_count += 1;
                }
            }
            max_width = max_width.max(column_count);
        }

        // 5px line should have width close to 5 (allowing for AA)
        assert!(
            max_width >= 4 && max_width <= 7,
            "Thick line width: {}, expected 4-7",
            max_width
        );
    }
}

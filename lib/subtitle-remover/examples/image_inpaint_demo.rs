//! 单图像字幕移除示例。
//!
//! 加载 test.png，手动指定底部字幕区域，创建 mask，使用 LaMa inpaint，保存结果。
//!
//! 运行: cargo run --example image_inpaint

use image::RgbImage;
use ndarray::Array2;
use subtitle_remover::{InpaintArea, Inpainter, LamaInpainter, Mask, create_mask};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = "tmp/test.png";
    let output_path = "tmp/test_no_sub.png";

    // Step 1: 加载图像
    let img = image::open(input_path)?;
    let rgb_img = img.to_rgb8();
    let (width, height) = (rgb_img.width() as i32, rgb_img.height() as i32);
    println!("图像: {}x{}", width, height);

    // Step 2: 手动指定底部字幕区域
    // 字幕实际位置：大约在底部 y≈430 到 y≈464 的区域
    // 精确坐标需要根据具体图片调整
    let subtitle_ymin = 430;
    let subtitle_ymax = 464;
    let subtitle_boxes: Vec<(i32, i32, i32, i32)> = vec![(0, width, subtitle_ymin, subtitle_ymax)];
    let deviation_pixel = 2;

    println!(
        "字幕区域: y=[{}, {}], x=[0, {}]",
        subtitle_ymin, subtitle_ymax, width
    );

    // 创建 mask — 只标记字幕区域
    let mask: Array2<u8> = create_mask(
        height as usize,
        width as usize,
        &subtitle_boxes,
        deviation_pixel,
    );

    // 计算 inpaint 区域：使用 mask 的精确边界矩形
    // 找到 mask 中所有值为 255 的像素的最小包围矩形
    let (mask_ymin, mask_ymax, mask_xmin, mask_xmax) = find_mask_bounds(&mask);
    // 对齐到 8 的倍数（LaMa 模型需要）
    let align = 8;
    let area_ymin = (mask_ymin / align) * align;
    let area_ymax = ((mask_ymax + align - 1) / align) * align;
    let area_xmin = (mask_xmin / align) * align;
    let area_xmax = ((mask_xmax + align - 1) / align) * align;

    let inpaint_areas = vec![(area_ymin, area_ymax, area_xmin, area_xmax)];
    println!("检测到 {} 个 inpaint 区域", inpaint_areas.len());
    for (i, area) in inpaint_areas.iter().enumerate() {
        let (ymin, ymax, xmin, xmax) = *area;
        println!(
            "  区域 {}: y=[{},{}], x=[{},{}] ({}x{})",
            i,
            ymin,
            ymax,
            xmin,
            xmax,
            ymax - ymin,
            xmax - xmin
        );
    }

    if inpaint_areas.is_empty() {
        eprintln!("未找到 inpaint 区域");
        return Ok(());
    }

    // Step 3: 加载 LaMa 模型
    let lama_path = std::path::Path::new("models/lama_fp32.onnx");
    if !lama_path.exists() {
        eprintln!("LaMa 模型不存在: {}", lama_path.display());
        return Ok(());
    }

    println!("使用 LaMa inpaint: {}", lama_path.display());
    let mut inpainter = LamaInpainter::new(lama_path.to_str().unwrap(), 1)?;

    // Step 4: Inpaint — 只在 mask 区域使用 inpaint 结果
    let mut result_img = rgb_img.clone();

    for area in &inpaint_areas {
        let inpainted = inpainter.inpaint(&[rgb_img.clone()], &mask, area)?;
        // 只在 mask=255 的像素上用 inpaint 结果替换原始像素
        masked_composite(&mut result_img, &inpainted[0], &mask, area);
    }

    // Step 5: 保存结果
    result_img.save(output_path)?;
    println!("输出文件: {}", output_path);

    Ok(())
}

/// 只在 mask=255（需要 inpaint）的位置上替换像素，
/// mask=0（保留原始内容）的位置不变。
/// 在 mask 边界处进行渐变混合，避免产生明显接缝。
fn masked_composite(target: &mut RgbImage, inpainted: &RgbImage, mask: &Mask, area: &InpaintArea) {
    let (ymin, _ymax, xmin, _xmax) = *area;
    let iw = inpainted.width() as i32;
    let ih = inpainted.height() as i32;
    let feather_radius = 5; // pixels for feathering at mask boundary

    // Precompute distance to mask boundary for each pixel in the area
    let mask_rows = mask.nrows();
    let mask_cols = mask.ncols();

    for y in 0..ih {
        let ty = y + ymin;
        if ty < 0 || ty >= target.height() as i32 {
            continue;
        }
        for x in 0..iw {
            let tx = x + xmin;
            if tx < 0 || tx >= target.width() as i32 {
                continue;
            }
            let my = ty as usize;
            let mx = tx as usize;
            if my >= mask_rows || mx >= mask_cols {
                continue;
            }
            if mask[[my, mx]] == 0 {
                continue;
            }

            // Find distance to nearest mask=0 pixel (or edge of mask array)
            let dist = min_mask_boundary_dist(mask, my, mx);

            if dist >= feather_radius {
                // Far from boundary: use inpainted pixel
                target.put_pixel(
                    tx as u32,
                    ty as u32,
                    *inpainted.get_pixel(x as u32, y as u32),
                );
            } else {
                // Near boundary: blend inpainted and original
                let alpha = dist as f32 / feather_radius as f32;
                let inp = inpainted.get_pixel(x as u32, y as u32);
                let orig = target.get_pixel(tx as u32, ty as u32);
                let blended = image::Rgb([
                    (orig[0] as f32 * (1.0 - alpha) + inp[0] as f32 * alpha) as u8,
                    (orig[1] as f32 * (1.0 - alpha) + inp[1] as f32 * alpha) as u8,
                    (orig[2] as f32 * (1.0 - alpha) + inp[2] as f32 * alpha) as u8,
                ]);
                target.put_pixel(tx as u32, ty as u32, blended);
            }
        }
    }
}

/// Find the minimum distance from (row, col) to a mask boundary.
/// A mask boundary is where mask value changes (255→0) or the array edge.
fn min_mask_boundary_dist(mask: &Mask, row: usize, col: usize) -> usize {
    let rows = mask.nrows();
    let cols = mask.ncols();
    let mut min_dist = usize::MAX;
    let search = feather_search_radius();

    for dy in -search..=search {
        for dx in -search..=search {
            let nr = row as i32 + dy;
            let nc = col as i32 + dx;
            // Array edge = boundary
            if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                let d = (dy.abs().max(dx.abs())) as usize;
                if d < min_dist {
                    min_dist = d;
                }
            } else if mask[[nr as usize, nc as usize]] != mask[[row, col]] {
                let d = (dy.abs().max(dx.abs())) as usize;
                if d < min_dist {
                    min_dist = d;
                }
            }
        }
    }

    if min_dist == usize::MAX {
        search as usize + 1
    } else {
        min_dist
    }
}

fn feather_search_radius() -> i32 {
    8
}

/// 找到 mask 中所有值为 255 的像素的最小包围矩形。
/// 返回 (ymin, ymax, xmin, xmax)，其中 ymax 和 xmax 是 exclusive。
fn find_mask_bounds(mask: &Mask) -> (i32, i32, i32, i32) {
    let rows = mask.nrows();
    let cols = mask.ncols();
    let mut ymin = rows as i32;
    let mut ymax = 0i32;
    let mut xmin = cols as i32;
    let mut xmax = 0i32;

    for r in 0..rows {
        for c in 0..cols {
            if mask[[r, c]] == 255 {
                ymin = ymin.min(r as i32);
                ymax = ymax.max(r as i32 + 1);
                xmin = xmin.min(c as i32);
                xmax = xmax.max(c as i32 + 1);
            }
        }
    }

    (ymin, ymax, xmin, xmax)
}

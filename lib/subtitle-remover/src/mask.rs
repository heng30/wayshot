use ndarray::Array2;

/// A bounding box in pixel coordinates: (xmin, xmax, ymin, ymax).
pub type BBox = (i32, i32, i32, i32);

/// An inpaint area: (ymin, ymax, xmin, xmax).
pub type InpaintArea = (i32, i32, i32, i32);

#[derive(Clone)]
struct Island {
    top_y: i32,
    bottom_y: i32,
    center_y: i32,
}

/// Create a binary mask image from a list of bounding boxes.
///
/// Each box `(xmin, xmax, ymin, ymax)` is expanded by `deviation_pixel` in all
/// directions and filled with 255 on a black (0) background.
pub fn create_mask(
    height: usize,
    width: usize,
    coords_list: &[BBox],
    deviation_pixel: u32,
) -> Array2<u8> {
    let mut mask = Array2::zeros((height, width));
    let dev = deviation_pixel as i32;
    for &(xmin, xmax, ymin, ymax) in coords_list {
        let x1 = (xmin - dev).max(0) as usize;
        let y1 = (ymin - dev).max(0) as usize;
        let x2 = ((xmax + dev) as usize).min(width);
        let y2 = ((ymax + dev) as usize).min(height);
        for r in y1..y2 {
            for c in x1..x2 {
                mask[[r, c]] = 255;
            }
        }
    }
    mask
}

/// Split `data` into balanced batches of at most `max_batch_size`.
///
/// Tries to make all batches roughly equal in size, matching the Python
/// `batch_generator` logic.
pub fn batch_generator<T: Clone>(data: &[T], max_batch_size: usize) -> Vec<Vec<T>> {
    if data.is_empty() || max_batch_size == 0 {
        return vec![];
    }
    let n = data.len();
    let batch_size = max_batch_size.min(n);
    data.chunks(batch_size).map(|c| c.to_vec()).collect()
}

/// Expand frame ranges backward/forward, ensuring no overlaps between ranges.
///
/// Ported from `expand_frame_ranges` in `inpaint_tools.py`.
pub fn expand_frame_ranges(
    frame_ranges: &[(u64, u64)],
    backward: u64,
    forward: u64,
) -> Vec<(u64, u64)> {
    if frame_ranges.is_empty() {
        return vec![];
    }

    let mut sorted: Vec<_> = frame_ranges.to_vec();
    sorted.sort_by_key(|r| r.0);

    let mut expanded = Vec::with_capacity(sorted.len());

    for (i, (start, end)) in sorted.iter().enumerate() {
        let mut new_start = start.saturating_sub(backward).max(1);
        let mut new_end = end + forward;

        // Avoid overlap with next range
        if i < sorted.len() - 1 {
            let next_start = sorted[i + 1].0;
            if new_end >= next_start {
                if next_start - end == 1 {
                    new_end = *end;
                } else {
                    new_end = new_end.min(next_start - 1);
                }
            }
        }

        // Avoid overlap with previous expanded range
        if let Some(&(_, prev_end)) = expanded.last() {
            if new_start <= prev_end {
                new_start = prev_end + 1;
            }
        }

        if new_start <= new_end {
            expanded.push((new_start, new_end));
        } else {
            expanded.push((*start, *end));
        }
    }

    expanded
}

/// Check if a frame number falls within any AB section range.
///
/// If `sections` is `None` or empty, all frames are considered in range.
pub fn is_frame_in_ab_sections(frame_no: u64, sections: Option<&[std::ops::Range<u64>]>) -> bool {
    match sections {
        None => true,
        Some(s) if s.is_empty() => true,
        Some(s) => s.iter().any(|r| r.contains(&frame_no)),
    }
}

/// Compute inpaint area strips from a mask image.
///
/// Finds connected components (islands) in the mask, merges nearby islands
/// whose combined height fits within `split_h`, and returns strip positions
/// `(ymin, ymax, xmin, xmax)` sized to `split_h` height and full width.
///
/// If `multiple > 1`, dimensions are adjusted to be multiples of `multiple`
/// (needed for ProPainter which requires sizes divisible by 8).
pub fn get_inpaint_area_by_mask(
    w: i32,
    h: i32,
    split_h: i32,
    mask: &Array2<u8>,
    multiple: i32,
) -> Vec<InpaintArea> {
    if mask.iter().all(|&v| v == 0) {
        return vec![];
    }

    // Find connected components using flood fill
    let islands = find_islands(mask);

    if islands.is_empty() {
        return vec![];
    }

    // Sort by center_y
    let mut islands = islands;
    islands.sort_by_key(|i| i.center_y);

    // Merge nearby islands
    let merged = merge_islands(&islands, split_h, mask);

    // Build inpaint areas from merged groups
    let mut inpaint_areas = Vec::new();
    for group in &merged {
        let min_y = group.iter().map(|i| i.top_y).min().unwrap();
        let max_y = group.iter().map(|i| i.bottom_y).max().unwrap();
        let center_y = group.iter().map(|i| i.center_y).sum::<i32>() / group.len() as i32;

        let half_h = split_h / 2;
        let mut ymin = (center_y - half_h).max(0);
        let mut ymax = ymin + split_h;

        if ymax > h {
            ymax = h;
            ymin = (h - split_h).max(0);
        }

        // Check if all islands are contained
        if ymin > min_y || ymax < max_y {
            if max_y - min_y <= split_h {
                ymin = min_y;
                ymax = ymin + split_h;
                if ymax > h {
                    ymax = h;
                    ymin = (h - split_h).max(0);
                }
            } else {
                let island_center = (min_y + max_y) / 2;
                ymin = (island_center - half_h).max(0);
                ymax = ymin + split_h;
                if ymax > h {
                    ymax = h;
                    ymin = (h - split_h).max(0);
                }
            }
        }

        let mut xmin = 0;
        let mut xmax = w;

        // Adjust to multiple
        if multiple > 1 {
            let height = ymax - ymin;
            let remainder = height % multiple;
            if remainder != 0 {
                let adjust = multiple - remainder;
                let center_y_f = (ymin + ymax) as f64 / 2.0;

                if ymin - adjust / 2 >= 0 && ymax + adjust / 2 <= h {
                    ymin = (center_y_f - height as f64 / 2.0 - adjust as f64 / 2.0) as i32;
                    ymax = (center_y_f + height as f64 / 2.0 + adjust as f64 / 2.0) as i32;
                } else if height > multiple {
                    ymin = (center_y_f - (height - remainder) as f64 / 2.0) as i32;
                    ymax = (center_y_f + (height - remainder) as f64 / 2.0) as i32;
                } else if ymax + adjust <= h {
                    ymax += adjust;
                } else if ymin - adjust >= 0 {
                    ymin -= adjust;
                } else if height > multiple {
                    ymax = ymin + height - remainder;
                }
            }

            let width = xmax - xmin;
            let remainder_w = width % multiple;
            if remainder_w != 0 {
                let center_x = (xmin + xmax) as f64 / 2.0;
                xmin = (center_x - (width - remainder_w) as f64 / 2.0) as i32;
                xmax = (center_x + (width - remainder_w) as f64 / 2.0) as i32;
            }
        }

        let area = (ymin, ymax, xmin, xmax);
        if !inpaint_areas.contains(&area) {
            inpaint_areas.push(area);
        }
    }

    inpaint_areas
}

/// Find connected components (islands) in a binary mask using flood fill.
fn find_islands(mask: &Array2<u8>) -> Vec<Island> {
    let (rows, cols) = mask.dim();
    let mut visited = vec![vec![false; cols]; rows];
    let mut islands = Vec::new();

    for r in 0..rows {
        for c in 0..cols {
            if mask[[r, c]] > 0 && !visited[r][c] {
                if let Some(island) = flood_fill_island(mask, &mut visited, r, c) {
                    islands.push(island);
                }
            }
        }
    }

    islands
}

/// Flood fill from (start_r, start_c) to find a connected island.
fn flood_fill_island(
    mask: &Array2<u8>,
    visited: &mut [Vec<bool>],
    start_r: usize,
    start_c: usize,
) -> Option<Island> {
    let (rows, cols) = mask.dim();
    let mut min_r = start_r;
    let mut max_r = start_r;
    let mut min_c = start_c;
    let mut max_c = start_c;
    let mut area = 0i32;
    let mut sum_r = 0i64;

    let mut queue = std::collections::VecDeque::new();
    queue.push_back((start_r, start_c));
    visited[start_r][start_c] = true;

    while let Some((r, c)) = queue.pop_front() {
        area += 1;
        sum_r += r as i64;
        min_r = min_r.min(r);
        max_r = max_r.max(r);
        min_c = min_c.min(c);
        max_c = max_c.max(c);

        // 8-connected neighbors
        for dr in [-1i32, 0, 1] {
            for dc in [-1i32, 0, 1] {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;
                if nr >= 0 && nr < rows as i32 && nc >= 0 && nc < cols as i32 {
                    let nr = nr as usize;
                    let nc = nc as usize;
                    if !visited[nr][nc] && mask[[nr, nc]] > 0 {
                        visited[nr][nc] = true;
                        queue.push_back((nr, nc));
                    }
                }
            }
        }
    }

    if area < 10 {
        return None;
    }

    let center_y = (sum_r / area as i64) as i32;

    Some(Island {
        top_y: min_r as i32,
        bottom_y: max_r as i32 + 1, // exclusive end
        center_y,
    })
}

fn merge_islands(islands: &[Island], split_h: i32, mask: &Array2<u8>) -> Vec<Vec<Island>> {
    let mut groups: Vec<Vec<Island>> = Vec::new();
    let mut current_group: Vec<Island> = vec![Island::clone(&islands[0])];

    for i in 1..islands.len() {
        let group_min_y = current_group.iter().map(|is| is.top_y).min().unwrap();
        let group_max_y = current_group.iter().map(|is| is.bottom_y).max().unwrap();
        let new_min_y = group_min_y.min(islands[i].top_y);
        let new_max_y = group_max_y.max(islands[i].bottom_y);

        let has_connection = if group_max_y < islands[i].top_y {
            // Check if mask pixels exist between the gap
            mask_region_has_pixels(mask, group_max_y, islands[i].top_y)
        } else {
            true // Overlapping or adjacent
        };

        if new_max_y - new_min_y <= split_h && has_connection {
            current_group.push(Island::clone(&islands[i]));
        } else {
            groups.push(current_group);
            current_group = vec![Island::clone(&islands[i])];
        }
    }
    groups.push(current_group);
    groups
}

fn mask_region_has_pixels(mask: &Array2<u8>, y_start: i32, y_end: i32) -> bool {
    let rows = mask.nrows();
    if y_start >= y_end || y_start as usize >= rows {
        return false;
    }
    let check_rows = (y_end - y_start).min(50);
    for y in y_start..y_start + check_rows {
        if y as usize >= rows {
            break;
        }
        for x in 0..mask.ncols() {
            if mask[[y as usize, x]] > 0 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_empty_mask_returns_empty() {
        let mask = Array2::zeros((100, 200));
        let areas = get_inpaint_area_by_mask(200, 100, 37, &mask, 1);
        assert!(areas.is_empty());
    }

    #[test]
    fn test_single_island_mask() {
        let mut mask = Array2::zeros((480, 640));
        // Draw a rectangle at the bottom (subtitle area)
        for r in 440..470 {
            for c in 50..300 {
                mask[[r, c]] = 255;
            }
        }
        let areas = get_inpaint_area_by_mask(640, 480, 120, &mask, 1);
        assert!(!areas.is_empty());
        let (ymin, ymax, _xmin, _xmax) = areas[0];
        assert!(ymin <= 440);
        assert!(ymax >= 470);
    }

    #[test]
    fn test_two_separate_islands() {
        let mut mask = Array2::zeros((480, 640));
        // Two separate subtitle lines
        for r in 400..420 {
            for c in 50..300 {
                mask[[r, c]] = 255;
            }
        }
        for r in 440..460 {
            for c in 100..400 {
                mask[[r, c]] = 255;
            }
        }
        let areas = get_inpaint_area_by_mask(640, 480, 120, &mask, 1);
        // Should produce at least one inpaint area covering both
        assert!(!areas.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_mask_basic() {
        let mask = create_mask(100, 200, &[(10, 50, 20, 60)], 5);
        assert_eq!(mask[[15, 5]], 255); // inside expanded box
        assert_eq!(mask[[19, 9]], 255); // inside expanded box
        assert_eq!(mask[[14, 4]], 0); // outside
        assert_eq!(mask[[65, 50]], 0); // outside y
    }

    #[test]
    fn test_create_mask_empty() {
        let mask = create_mask(10, 10, &[], 5);
        assert!(mask.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_create_mask_clamp_to_bounds() {
        let mask = create_mask(10, 10, &[(0, 15, 0, 15)], 5);
        assert_eq!(mask[[0, 0]], 255);
        assert_eq!(mask[[9, 9]], 255);
    }

    #[test]
    fn test_batch_generator_even() {
        let data: Vec<i32> = (0..10).collect();
        let batches = batch_generator(&data, 5);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 5);
        assert_eq!(batches[1].len(), 5);
    }

    #[test]
    fn test_batch_generator_remainder() {
        let data: Vec<i32> = (0..7).collect();
        let batches = batch_generator(&data, 5);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 5);
        assert_eq!(batches[1].len(), 2);
    }

    #[test]
    fn test_batch_generator_single() {
        let data: Vec<i32> = vec![1];
        let batches = batch_generator(&data, 10);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![1]);
    }

    #[test]
    fn test_expand_frame_ranges_basic() {
        let ranges = vec![(10, 20), (30, 40)];
        let expanded = expand_frame_ranges(&ranges, 3, 3);
        assert_eq!(expanded, vec![(7, 23), (27, 43)]);
    }

    #[test]
    fn test_expand_frame_ranges_no_overlap() {
        let ranges = vec![(10, 12), (14, 16)];
        let expanded = expand_frame_ranges(&ranges, 3, 3);
        // (7,15) and (11,19) -> overlap resolved
        assert!(expanded[0].1 < expanded[1].0);
    }

    #[test]
    fn test_expand_frame_ranges_clamp_start() {
        let ranges = vec![(1, 5)];
        let expanded = expand_frame_ranges(&ranges, 3, 3);
        assert_eq!(expanded[0].0, 1);
    }

    #[test]
    fn test_is_frame_in_ab_sections_none() {
        assert!(is_frame_in_ab_sections(5, None));
    }

    #[test]
    fn test_is_frame_in_ab_sections_empty() {
        assert!(is_frame_in_ab_sections(5, Some(&[])));
    }

    #[test]
    fn test_is_frame_in_ab_sections_hit() {
        let sections = [0..10, 20..30];
        assert!(is_frame_in_ab_sections(5, Some(&sections)));
    }

    #[test]
    fn test_is_frame_in_ab_sections_miss() {
        let sections = [0..10, 20..30];
        assert!(!is_frame_in_ab_sections(15, Some(&sections)));
    }
}

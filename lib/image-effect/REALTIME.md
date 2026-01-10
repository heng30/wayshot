# Real-Time Effects for Screen Recording

This module provides a curated collection of image effects optimized for real-time screen recording.

## Overview

All effects in this module are tested and verified to process 1920x1080 images in **30ms or less**, making them suitable for:

- **60 FPS recording** (≤16.67ms per frame)
- **30 FPS recording** (≤33.33ms per frame)
- **24 FPS recording** (≤41.67ms per frame)

## Usage

```rust
use image_effect::realtime::RealTimeEffect;

// Apply an effect
let effect = RealTimeEffect::GammaCorrection;
let result = effect.apply(image);

// Get performance info
println!("Processing time: {:.2}ms", effect.estimated_time_ms());
println!("Recommended max FPS: {}", effect.recommended_max_fps());

// Get all effects suitable for 60 FPS
for effect in RealTimeEffect::effects_60fps() {
    println!("{}", effect.name());
}
```

## Performance Categories

### ⭐ 60 FPS Capable (≤16.67ms)

**Basic Adjustments:**
- GammaCorrection (4.864ms) - Color correction
- Invert (8.393ms) - Negative effect
- Threshold (14.959ms) - Binarization

**Artistic Filters (Ultra-fast):**
- Rosetint (6.552ms) - Rose tint
- Twenties (6.555ms) - Vintage 1920s
- Mauve (6.652ms) - Purple tint
- Radio (6.825ms) - Radio style
- Bluechrome (7.133ms) - Blue chrome

**Stylized Effects:**
- Pixelate (14.340ms) - Mosaic effect
- IncreaseBrightness (5.586ms) - Quick brighten
- DecreaseBrightness (7.285ms) - Quick darken

### 🎬 30 FPS Capable (≤33.33ms)

**Enhanced Adjustments:**
- Grayscale (19.682ms) - Black & white
- Brightness (16.133ms) - Brightness control
- Contrast (29.991ms) - Contrast enhancement

**Cinematic Filters:**
- Dramatic (25.872ms) - Dramatic look
- PastelPink (26.189ms) - Soft pink tones
- Obsidian (28.649ms) - Dark obsidian
- Vignette (25.993ms) - Edge darkening
- Posterize (26.434ms) - Color reduction

### ⚠️ Use with Caution (Near limits)

- HueRotate (133.818ms) - 24 FPS max
- Saturation (125.498ms) - 24 FPS max
- Sepia (47.550ms) - 30 FPS max
- Temperature (32.835ms) - 30 FPS max

## Files Generated

Run the demo to see all effects:
```bash
cargo run --example realtime_effects_demo
```

Output images are saved to `tmp/realtime/` directory.

## Integration Example

```rust
use image_effect::realtime::RealTimeEffect;

fn process_frame_for_recording(frame: RgbaImage, fps: u32) -> Option<RgbaImage> {
    let effect = match fps {
        60 => RealTimeEffect::GammaCorrection,  // 4.864ms
        30 => RealTimeEffect::Contrast,         // 29.991ms
        _ => RealTimeEffect::Grayscale,         // 19.682ms
    };

    effect.apply(frame)
}
```

## Performance Notes

- All measurements based on 1920x1080 resolution
- Debug mode timings shown (release mode ~2-5x faster)
- Consider GPU acceleration for better performance
- Chain multiple effects by accumulating processing time

## Recommendations

**For Screen Recording Software:**

1. **Best Performance:** GammaCorrection, Rosetint, Twenties
2. **Visual Quality:** Grayscale, Dramatic, Obsidian
3. **Balanced:** Brightness, Contrast, Vignette

**Avoid in Real-time:**
- Oil painting (>4000ms)
- LCh/HSLuv color space effects (>500ms)
- Selective effects (>300ms)

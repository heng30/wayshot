# Image Effect Performance Benchmark

**Test Image:** 1920x1080 (2073600 pixels)

**Iterations per effect:** 5 (taking middle 3)

**Total effects tested:** 39

## Summary

- **Total time:** 1272.966 ms
- **Average time:** 32.640 ms
- **Fastest effect:** Invert (2.969 ms)
- **Slowest effect:** GaussianBlur (radius=3) (298.095 ms)

## Performance Rankings

| Rank | Effect | Avg Time (ms) | Category |
|------|--------|---------------|----------|
| 1 | Invert | 2.969 | Base |
| 2 | Solarization (RGB) | 3.654 | Base |
| 3 | Radio | 4.254 | Base |
| 4 | ColorBalance | 4.308 | Base |
| 5 | Threshold | 5.065 | Base |
| 6 | Brightness | 5.436 | Base |
| 7 | Bluechrome | 5.545 | Blur |
| 8 | Grayscale (Luminance) | 5.836 | Blur |
| 9 | WarmFilter | 5.992 | Blur |
| 10 | CoolFilter | 6.092 | Filter |
| 11 | ColorTint | 7.704 | Filter |
| 12 | Pixelate (size=10) | 8.374 | Filter |
| 13 | Diamante | 8.710 | Filter |
| 14 | Marine | 8.828 | Filter |
| 15 | Liquid | 8.829 | Stylized |
| 16 | Islands | 8.835 | Stylized |
| 17 | Oceanic | 8.871 | Stylized |
| 18 | Seagreen | 8.873 | Stylized |
| 19 | Mauve | 8.908 | Stylized |
| 20 | Rosetint | 8.946 | Preset |
| 21 | Flagblue | 8.968 | Preset |
| 22 | Serenity | 10.215 | Preset |
| 23 | Contrast | 11.245 | Preset |
| 24 | Perfume | 11.304 | Preset |
| 25 | Duotone | 13.474 | Preset |
| 26 | Level | 13.649 | Preset |
| 27 | Sepia | 14.992 | Preset |
| 28 | Vignette | 15.234 | Preset |
| 29 | Saturation | 20.008 | Preset |
| 30 | Twenties | 23.336 | Preset |
| 31 | Vintage | 28.889 | Preset |
| 32 | Posterize | 29.355 | Preset |
| 33 | Emboss | 59.645 | Preset |
| 34 | Sharpen | 64.482 | Preset |
| 35 | EdgeDetection (Sobel) | 68.507 | Monochrome |
| 36 | HueRotate | 68.822 | Monochrome |
| 37 | BoxBlur (radius=3) | 102.666 | Monochrome |
| 38 | MedianBlur (radius=3) | 274.050 | Monochrome |
| 39 | GaussianBlur (radius=3) | 298.095 | Monochrome |

## Performance by Category

### Base Effects

| Effect | Time (ms) |
|--------|----------|
| Invert | 2.969 |
| Brightness | 5.436 |
| Contrast | 11.245 |
| Saturation | 20.008 |
| HueRotate | 68.822 |

### Blur Effects

| Effect | Time (ms) |
|--------|----------|
| BoxBlur (radius=3) | 102.666 |
| MedianBlur (radius=3) | 274.050 |
| GaussianBlur (radius=3) | 298.095 |

### Filter Effects

| Effect | Time (ms) |
|--------|----------|
| WarmFilter | 5.992 |
| CoolFilter | 6.092 |
| ColorTint | 7.704 |
| Sepia | 14.992 |
| Vignette | 15.234 |

### Stylized Effects

| Effect | Time (ms) |
|--------|----------|
| Pixelate (size=10) | 8.374 |
| Posterize | 29.355 |
| Emboss | 59.645 |
| Sharpen | 64.482 |
| EdgeDetection (Sobel) | 68.507 |

### Preset Filters

| Effect | Time (ms) |
|--------|----------|
| Radio | 4.254 |
| Bluechrome | 5.545 |
| Diamante | 8.710 |
| Marine | 8.828 |
| Liquid | 8.829 |
| Islands | 8.835 |
| Oceanic | 8.871 |
| Seagreen | 8.873 |
| Mauve | 8.908 |
| Rosetint | 8.946 |
| Flagblue | 8.968 |
| Serenity | 10.215 |
| Perfume | 11.304 |
| Twenties | 23.336 |
| Vintage | 28.889 |

### Monochrome Effects

| Effect | Time (ms) |
|--------|----------|
| Solarization (RGB) | 3.654 |
| ColorBalance | 4.308 |
| Threshold | 5.065 |
| Duotone | 13.474 |
| Level | 13.649 |

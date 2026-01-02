# Image Effect Performance Benchmark

**Test Image:** 1920x1080

**Iterations per effect:** 5 (taking middle 3)

**Total effects tested:** 89

## Summary

- **Total time:** 3460.641 ms
- **Average time:** 38.884 ms
- **Fastest effect:** RemoveBlueChannel (0.412 ms)
- **Slowest effect:** Oil (690.798 ms)

## Performance Rankings

| Rank | Effect | Avg Time (ms) | Category |
|------|--------|---------------|----------|
| 1 | RemoveBlueChannel | 0.412 | Base |
| 2 | RemoveGreenChannel | 0.420 | Base |
| 3 | RemoveRedChannel | 0.424 | Base |
| 4 | AlterGreenChannel | 0.644 | Base |
| 5 | AlterBlueChannel | 0.653 | Base |
| 6 | AlterRedChannel | 0.735 | Base |
| 7 | HorizontalStrips | 0.804 | Blur |
| 8 | Solarization (RGB) | 0.976 | Blur |
| 9 | Radio | 0.984 | Blur |
| 10 | VerticalStrips | 1.070 | Filter |
| 11 | ColorVerticalStrips | 1.071 | Filter |
| 12 | ColorHorizontalStrips | 1.127 | Filter |
| 13 | ColorBalance | 1.136 | Filter |
| 14 | IncBrightness | 1.170 | Filter |
| 15 | Primary | 1.186 | Stylized |
| 16 | DecBrightness | 1.218 | Stylized |
| 17 | AlterTwoChannels | 1.287 | Stylized |
| 18 | Bluechrome | 1.295 | Stylized |
| 19 | GammaCorrection | 1.436 | Stylized |
| 20 | Invert | 1.596 | Preset |
| 21 | AlterChannels | 1.718 | Preset |
| 22 | Diamante | 1.949 | Preset |
| 23 | Serenity | 1.960 | Preset |
| 24 | Rosetint | 2.034 | Preset |
| 25 | Liquid | 2.038 | Preset |
| 26 | Seagreen | 2.041 | Preset |
| 27 | Islands | 2.043 | Preset |
| 28 | Perfume | 2.047 | Preset |
| 29 | Flagblue | 2.055 | Preset |
| 30 | Mauve | 2.073 | Preset |
| 31 | Marine | 2.089 | Preset |
| 32 | Oceanic | 2.126 | Preset |
| 33 | Pixelate (size=10) | 2.585 | Preset |
| 34 | Brightness | 2.664 | Preset |
| 35 | Threshold | 2.701 | Monochrome |
| 36 | Duotone | 2.922 | Monochrome |
| 37 | Halftone | 3.000 | Monochrome |
| 38 | Level | 3.384 | Monochrome |
| 39 | OffsetRed | 3.956 | Monochrome |
| 40 | Grayscale (Luminance) | 4.009 | Noise |
| 41 | Offset | 4.030 | Noise |
| 42 | OffsetBlue | 4.536 | Channel |
| 43 | Colorize | 4.975 | Channel |
| 44 | Contrast | 5.014 | Channel |
| 45 | OffsetGreen | 5.049 | Channel |
| 46 | Normalize | 5.061 | Channel |
| 47 | ColorTint | 5.085 | Channel |
| 48 | CoolFilter | 5.091 | Channel |
| 49 | WarmFilter | 5.111 | Channel |
| 50 | Twenties | 5.206 | Channel |
| 51 | Vignette | 5.527 | Channel |
| 52 | Vintage | 5.992 | Channel |
| 53 | Posterize | 6.442 | Channel |
| 54 | MultipleOffsets | 7.513 | Channel |
| 55 | GaussianNoise | 7.986 | ColourSpace |
| 56 | Sepia | 8.323 | ColourSpace |
| 57 | Dither | 9.203 | ColourSpace |
| 58 | PinkNoise | 21.351 | ColourSpace |
| 59 | LightenHsv | 25.168 | ColourSpace |
| 60 | DarkenHsv | 26.331 | ColourSpace |
| 61 | DesaturateHsv | 26.936 | ColourSpace |
| 62 | GaussianBlur (radius=3) | 26.979 | ColourSpace |
| 63 | Sharpen | 27.200 | ColourSpace |
| 64 | HueRotate | 27.278 | ColourSpace |
| 65 | BoxBlur (radius=3) | 28.197 | ColourSpace |
| 66 | Emboss | 28.517 | ColourSpace |
| 67 | Saturation | 30.951 | ColourSpace |
| 68 | SaturateHsv | 31.278 | ColourSpace |
| 69 | HueRotateHsl | 31.983 | ColourSpace |
| 70 | HueRotateHsv | 32.237 | ColourSpace |
| 71 | MedianBlur (radius=3) | 49.458 | ColourSpace |
| 72 | SelectiveHueRotate | 73.000 | Special |
| 73 | SelectiveGrayscale | 79.758 | Special |
| 74 | EdgeDetection (Sobel) | 79.959 | Special |
| 75 | SelectiveDesaturate | 102.979 | Special |
| 76 | SelectiveSaturate | 105.016 | Special |
| 77 | SelectiveLighten | 108.238 | Special |
| 78 | FrostedGlass | 108.940 | Special |
| 79 | DesaturateLch | 111.836 | Special |
| 80 | SaturateLch | 118.790 | Special |
| 81 | LightenLch | 127.316 | Special |
| 82 | DarkenLch | 127.592 | Special |
| 83 | HueRotateLch | 129.176 | Special |
| 84 | DesaturateHsluv | 178.830 | Special |
| 85 | SaturateHsluv | 181.410 | Special |
| 86 | DarkenHsluv | 182.204 | Special |
| 87 | LightenHsluv | 183.373 | Special |
| 88 | HueRotateHsluv | 188.385 | Special |
| 89 | Oil | 690.798 | Special |

## Performance by Category

### Base Effects

| Effect | Time (ms) |
|--------|----------|
| Invert | 1.596 |
| Brightness | 2.664 |
| Contrast | 5.014 |
| HueRotate | 27.278 |
| Saturation | 30.951 |

### Blur Effects

| Effect | Time (ms) |
|--------|----------|
| GaussianBlur (radius=3) | 26.979 |
| BoxBlur (radius=3) | 28.197 |
| MedianBlur (radius=3) | 49.458 |

### Filter Effects

| Effect | Time (ms) |
|--------|----------|
| ColorTint | 5.085 |
| CoolFilter | 5.091 |
| WarmFilter | 5.111 |
| Vignette | 5.527 |
| Sepia | 8.323 |

### Stylized Effects

| Effect | Time (ms) |
|--------|----------|
| Pixelate (size=10) | 2.585 |
| Posterize | 6.442 |
| Sharpen | 27.200 |
| Emboss | 28.517 |
| EdgeDetection (Sobel) | 79.959 |

### Preset Filters

| Effect | Time (ms) |
|--------|----------|
| Radio | 0.984 |
| Bluechrome | 1.295 |
| Diamante | 1.949 |
| Serenity | 1.960 |
| Rosetint | 2.034 |
| Liquid | 2.038 |
| Seagreen | 2.041 |
| Islands | 2.043 |
| Perfume | 2.047 |
| Flagblue | 2.055 |
| Mauve | 2.073 |
| Marine | 2.089 |
| Oceanic | 2.126 |
| Twenties | 5.206 |
| Vintage | 5.992 |

### Monochrome Effects

| Effect | Time (ms) |
|--------|----------|
| Solarization (RGB) | 0.976 |
| ColorBalance | 1.136 |
| Threshold | 2.701 |
| Duotone | 2.922 |
| Level | 3.384 |

### Noise Effects

| Effect | Time (ms) |
|--------|----------|
| GaussianNoise | 7.986 |
| PinkNoise | 21.351 |

### Channel Effects

| Effect | Time (ms) |
|--------|----------|
| RemoveBlueChannel | 0.412 |
| RemoveGreenChannel | 0.420 |
| RemoveRedChannel | 0.424 |
| AlterGreenChannel | 0.644 |
| AlterBlueChannel | 0.653 |
| AlterRedChannel | 0.735 |
| AlterTwoChannels | 1.287 |
| AlterChannels | 1.718 |
| SelectiveHueRotate | 73.000 |
| SelectiveGrayscale | 79.758 |
| SelectiveDesaturate | 102.979 |
| SelectiveSaturate | 105.016 |
| SelectiveLighten | 108.238 |

### Colour Space Effects

| Effect | Time (ms) |
|--------|----------|
| GammaCorrection | 1.436 |
| LightenHsv | 25.168 |
| DarkenHsv | 26.331 |
| DesaturateHsv | 26.936 |
| SaturateHsv | 31.278 |
| HueRotateHsl | 31.983 |
| HueRotateHsv | 32.237 |
| DesaturateLch | 111.836 |
| SaturateLch | 118.790 |
| LightenLch | 127.316 |
| DarkenLch | 127.592 |
| HueRotateLch | 129.176 |
| DesaturateHsluv | 178.830 |
| SaturateHsluv | 181.410 |
| DarkenHsluv | 182.204 |
| LightenHsluv | 183.373 |
| HueRotateHsluv | 188.385 |

### Special Effects

| Effect | Time (ms) |
|--------|----------|
| HorizontalStrips | 0.804 |
| VerticalStrips | 1.070 |
| ColorVerticalStrips | 1.071 |
| ColorHorizontalStrips | 1.127 |
| IncBrightness | 1.170 |
| Primary | 1.186 |
| DecBrightness | 1.218 |
| Halftone | 3.000 |
| OffsetRed | 3.956 |
| Offset | 4.030 |
| OffsetBlue | 4.536 |
| Colorize | 4.975 |
| OffsetGreen | 5.049 |
| Normalize | 5.061 |
| MultipleOffsets | 7.513 |
| Dither | 9.203 |
| FrostedGlass | 108.940 |
| Oil | 690.798 |

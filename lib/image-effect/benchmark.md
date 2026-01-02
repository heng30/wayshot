# Image Effect Performance Benchmark

**Test Image:** 1920x1080 (2073600 pixels)

**Iterations per effect:** 5 (taking middle 3)

**Total effects tested:** 96

## Summary

- **Total time:** 17468.366 ms
- **Average time:** 181.962 ms
- **Fastest effect:** RemoveBlueChannel (2.059 ms)
- **Slowest effect:** Oil (4197.725 ms)

## Performance Rankings

| Rank | Effect | Avg Time (ms) | Category |
|------|--------|---------------|----------|
| 1 | RemoveBlueChannel | 2.059 | Base |
| 2 | RemoveRedChannel | 2.105 | Base |
| 3 | RemoveGreenChannel | 2.142 | Base |
| 4 | AlterGreenChannel | 3.335 | Base |
| 5 | AlterRedChannel | 3.644 | Base |
| 6 | ColorVerticalStrips | 4.798 | Base |
| 7 | Solarization (RGB) | 4.857 | Blur |
| 8 | GammaCorrection | 4.864 | Blur |
| 9 | ColorHorizontalStrips | 5.008 | Blur |
| 10 | VerticalStrips | 5.056 | Filter |
| 11 | AlterBlueChannel | 5.183 | Filter |
| 12 | HorizontalStrips | 5.184 | Filter |
| 13 | IncBrightness | 5.586 | Filter |
| 14 | AlterChannels | 6.332 | Filter |
| 15 | Primary | 6.519 | Stylized |
| 16 | Rosetint | 6.552 | Stylized |
| 17 | Twenties | 6.555 | Stylized |
| 18 | Mauve | 6.652 | Stylized |
| 19 | Radio | 6.825 | Stylized |
| 20 | Bluechrome | 7.133 | Preset |
| 21 | AlterTwoChannels | 7.259 | Preset |
| 22 | DecBrightness | 7.285 | Preset |
| 23 | ColorBalance | 7.752 | Preset |
| 24 | Invert | 8.393 | Preset |
| 25 | Pixelate (size=10) | 14.340 | Preset |
| 26 | Threshold | 14.959 | Preset |
| 27 | Level | 15.382 | Preset |
| 28 | Duotone | 15.711 | Preset |
| 29 | Brightness | 16.133 | Preset |
| 30 | Halftone | 17.031 | Preset |
| 31 | Grayscale (Luminance) | 19.682 | Preset |
| 32 | OffsetRed | 21.864 | Preset |
| 33 | Normalize | 22.269 | Preset |
| 34 | Dramatic | 25.872 | Preset |
| 35 | Vignette | 25.993 | Preset |
| 36 | PastelPink | 26.189 | Preset |
| 37 | Posterize | 26.434 | Preset |
| 38 | OffsetGreen | 27.411 | Preset |
| 39 | Offset | 28.121 | Preset |
| 40 | Obsidian | 28.649 | Preset |
| 41 | Contrast | 29.991 | Preset |
| 42 | OffsetBlue | 32.006 | Monochrome |
| 43 | ColorTint | 32.835 | Monochrome |
| 44 | Colorize | 33.421 | Monochrome |
| 45 | CoolFilter | 35.176 | Monochrome |
| 46 | WarmFilter | 35.783 | Monochrome |
| 47 | GaussianNoise | 42.028 | Noise |
| 48 | Serenity | 42.322 | Noise |
| 49 | Oceanic | 43.000 | Channel |
| 50 | Perfume | 43.356 | Channel |
| 51 | MultipleOffsets | 43.587 | Channel |
| 52 | Islands | 44.189 | Channel |
| 53 | Diamante | 45.048 | Channel |
| 54 | Marine | 45.990 | Channel |
| 55 | Seagreen | 46.075 | Channel |
| 56 | Liquid | 46.748 | Channel |
| 57 | Sepia | 47.550 | Channel |
| 58 | Flagblue | 48.628 | Channel |
| 59 | Dither | 49.498 | Channel |
| 60 | Vintage | 51.742 | Channel |
| 61 | Golden | 64.660 | Channel |
| 62 | Cali | 65.480 | ColourSpace |
| 63 | Firenze | 79.942 | ColourSpace |
| 64 | PinkNoise | 87.632 | ColourSpace |
| 65 | GaussianBlur (radius=3) | 114.838 | ColourSpace |
| 66 | BoxBlur (radius=3) | 117.808 | ColourSpace |
| 67 | DarkenHsv | 117.973 | ColourSpace |
| 68 | Sharpen | 119.600 | ColourSpace |
| 69 | LightenHsv | 120.783 | ColourSpace |
| 70 | HueRotateHsl | 125.490 | ColourSpace |
| 71 | Saturation | 125.498 | ColourSpace |
| 72 | Emboss | 126.404 | ColourSpace |
| 73 | HueRotateHsv | 127.168 | ColourSpace |
| 74 | DesaturateHsv | 127.826 | ColourSpace |
| 75 | SaturateHsv | 128.832 | ColourSpace |
| 76 | HueRotate | 133.818 | ColourSpace |
| 77 | Lofi | 157.723 | ColourSpace |
| 78 | MedianBlur (radius=3) | 278.422 | ColourSpace |
| 79 | SelectiveHueRotate | 338.320 | Special |
| 80 | EdgeDetection (Sobel) | 339.656 | Special |
| 81 | SelectiveGrayscale | 368.655 | Special |
| 82 | FrostedGlass | 475.192 | Special |
| 83 | SelectiveDesaturate | 486.649 | Special |
| 84 | SelectiveSaturate | 487.707 | Special |
| 85 | SelectiveLighten | 515.212 | Special |
| 86 | DesaturateLch | 517.853 | Special |
| 87 | LightenLch | 527.723 | Special |
| 88 | DarkenLch | 529.539 | Special |
| 89 | SaturateLch | 535.833 | Special |
| 90 | HueRotateLch | 557.934 | Special |
| 91 | DarkenHsluv | 784.083 | Special |
| 92 | LightenHsluv | 795.062 | Special |
| 93 | HueRotateHsluv | 809.617 | Special |
| 94 | DesaturateHsluv | 821.890 | Special |
| 95 | SaturateHsluv | 843.726 | Special |
| 96 | Oil | 4197.725 | Special |

## Performance by Category

### Base Effects

| Effect | Time (ms) |
|--------|----------|
| Invert | 8.393 |
| Brightness | 16.133 |
| Contrast | 29.991 |
| Saturation | 125.498 |
| HueRotate | 133.818 |

### Blur Effects

| Effect | Time (ms) |
|--------|----------|
| GaussianBlur (radius=3) | 114.838 |
| BoxBlur (radius=3) | 117.808 |
| MedianBlur (radius=3) | 278.422 |

### Filter Effects

| Effect | Time (ms) |
|--------|----------|
| Vignette | 25.993 |
| ColorTint | 32.835 |
| CoolFilter | 35.176 |
| WarmFilter | 35.783 |
| Sepia | 47.550 |

### Stylized Effects

| Effect | Time (ms) |
|--------|----------|
| Pixelate (size=10) | 14.340 |
| Posterize | 26.434 |
| Sharpen | 119.600 |
| Emboss | 126.404 |
| EdgeDetection (Sobel) | 339.656 |

### Preset Filters

| Effect | Time (ms) |
|--------|----------|
| Rosetint | 6.552 |
| Twenties | 6.555 |
| Mauve | 6.652 |
| Radio | 6.825 |
| Bluechrome | 7.133 |
| Dramatic | 25.872 |
| PastelPink | 26.189 |
| Obsidian | 28.649 |
| Serenity | 42.322 |
| Oceanic | 43.000 |
| Perfume | 43.356 |
| Islands | 44.189 |
| Diamante | 45.048 |
| Marine | 45.990 |
| Seagreen | 46.075 |
| Liquid | 46.748 |
| Flagblue | 48.628 |
| Vintage | 51.742 |
| Golden | 64.660 |
| Cali | 65.480 |
| Firenze | 79.942 |
| Lofi | 157.723 |

### Monochrome Effects

| Effect | Time (ms) |
|--------|----------|
| Solarization (RGB) | 4.857 |
| ColorBalance | 7.752 |
| Threshold | 14.959 |
| Level | 15.382 |
| Duotone | 15.711 |

### Noise Effects

| Effect | Time (ms) |
|--------|----------|
| GaussianNoise | 42.028 |
| PinkNoise | 87.632 |

### Channel Effects

| Effect | Time (ms) |
|--------|----------|
| RemoveBlueChannel | 2.059 |
| RemoveRedChannel | 2.105 |
| RemoveGreenChannel | 2.142 |
| AlterGreenChannel | 3.335 |
| AlterRedChannel | 3.644 |
| AlterBlueChannel | 5.183 |
| AlterChannels | 6.332 |
| AlterTwoChannels | 7.259 |
| SelectiveHueRotate | 338.320 |
| SelectiveGrayscale | 368.655 |
| SelectiveDesaturate | 486.649 |
| SelectiveSaturate | 487.707 |
| SelectiveLighten | 515.212 |

### Colour Space Effects

| Effect | Time (ms) |
|--------|----------|
| GammaCorrection | 4.864 |
| DarkenHsv | 117.973 |
| LightenHsv | 120.783 |
| HueRotateHsl | 125.490 |
| HueRotateHsv | 127.168 |
| DesaturateHsv | 127.826 |
| SaturateHsv | 128.832 |
| DesaturateLch | 517.853 |
| LightenLch | 527.723 |
| DarkenLch | 529.539 |
| SaturateLch | 535.833 |
| HueRotateLch | 557.934 |
| DarkenHsluv | 784.083 |
| LightenHsluv | 795.062 |
| HueRotateHsluv | 809.617 |
| DesaturateHsluv | 821.890 |
| SaturateHsluv | 843.726 |

### Special Effects

| Effect | Time (ms) |
|--------|----------|
| ColorVerticalStrips | 4.798 |
| ColorHorizontalStrips | 5.008 |
| VerticalStrips | 5.056 |
| HorizontalStrips | 5.184 |
| IncBrightness | 5.586 |
| Primary | 6.519 |
| DecBrightness | 7.285 |
| Halftone | 17.031 |
| OffsetRed | 21.864 |
| Normalize | 22.269 |
| OffsetGreen | 27.411 |
| Offset | 28.121 |
| OffsetBlue | 32.006 |
| Colorize | 33.421 |
| MultipleOffsets | 43.587 |
| Dither | 49.498 |
| FrostedGlass | 475.192 |
| Oil | 4197.725 |

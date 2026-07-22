# Code Image

Generate syntax-highlighted code images with terminal window decorations.

## Features

- Syntax highlighting using `syntect` with multiple themes
- Dual-font rendering (ASCII + non-ASCII like Chinese)
- Terminal window decorations (macOS, Windows, GNOME, iTerm styles)
- PNG output format with configurable resolution scaling
- Returns `image::RgbaImage` for direct image manipulation

## CLI Usage

```bash
# Basic usage
code-image <INPUT_FILE> -o <OUTPUT> -a <ASCII_FONT> -n <NON_ASCII_FONT>

# Example: with line numbers, custom theme, and terminal decoration
code-image src/main.rs \
  -o output.png \
  -a JetBrainsMono-Regular.ttf \
  -n SourceHanSansCN.otf \
  -l \
  -t "base16-ocean.dark" \
  --terminal macos \
  --terminal-title "My Project"

# Windows light terminal style
code-image src/main.rs \
  -o output.png \
  -a JetBrainsMono-Regular.ttf \
  -n SourceHanSansCN.otf \
  --terminal windows

# Windows dark terminal style
code-image src/main.rs \
  -o output.png \
  -a JetBrainsMono-Regular.ttf \
  -n SourceHanSansCN.otf \
  --terminal windows-dark
```

### CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--output` | `-o` | Required | Output PNG file path |
| `--ascii-font` | `-a` | Required | ASCII font file path |
| `--non-ascii-font` | `-n` | Required | Non-ASCII font file path |
| `--language` | `-L` | `rust` | Language for syntax highlighting |
| `--line-numbers` | `-l` | `false` | Show line numbers |
| `--theme` | `-t` | `Solarized (dark)` | Syntax theme name |
| `--font-size` | `-s` | `16.0` | Font size in pixels |
| `--line-height-ratio` | | `1.5` | Line height multiplier |
| `--padding` | `-p` | `20.0` | Padding around code |
| `--scale` | `-r` | `2.0` | Resolution scale factor |
| `--bg-color` | `-b` | Theme-based | Custom background color |
| `--terminal` | | None | Terminal window style |
| `--terminal-title` | | None | Terminal window title |

### Supported Languages

`rust`, `python`, `javascript`, `go`, `c`, `cpp`, `java`, `ruby`, `php`, `html`, `css`, `json`, `yaml`, `markdown`, `shell`, `sql`, `lua`, `scala`

### Supported Themes

- `Solarized (dark)` / `Solarized (light)`
- `base16-ocean.dark` / `base16-ocean.light`
- `base16-eighties.dark`
- `base16-mocha.dark`
- `InspiredGitHub`

### Terminal Styles

| Style | Description |
|-------|-------------|
| `macos` | macOS Terminal (light) - white background, traffic light buttons |
| `macos-dark` | macOS Terminal (dark) - dark background, dimmed buttons |
| `windows` | Windows Terminal (light) - white background, minimize/maximize/close icons |
| `windows-dark` | Windows Terminal (dark) - dark background, gray icons |
| `gnome` | GNOME Terminal - dark theme, buttons on right side |
| `iterm` | iTerm2 - dark theme, buttons with gradient effect |

## Library Usage

```rust
use code_image::{CodeHighlightConfig, TerminalStyle, highlight_code};

let config = CodeHighlightConfig::new(
    "fonts/JetBrainsMono-Regular.ttf".into(),
    "fonts/SourceHanSansCN.otf".into(),
)
.with_line_numbers(true)
.with_theme("base16-ocean.dark".to_string())
.with_font_size(18.0)
.with_terminal(TerminalStyle::Windows)
.with_terminal_title("My App".to_string());

let png_bytes = highlight_code("fn main() {}", &config)?;
std::fs::write("output.png", png_bytes)?;
```

### Get `image::RgbaImage`

```rust
use code_image::{CodeHighlightConfig, highlight_code_to_image};

let config = CodeHighlightConfig::new(
    "fonts/JetBrainsMono-Regular.ttf".into(),
    "fonts/SourceHanSansCN.otf".into(),
);

// Get RgbaImage for further processing
let image = highlight_code_to_image("fn main() {}", &config)?;
image.save("output.png")?;
```
pub fn get_bg_color(theme: &str) -> &str {
    match theme {
        "Solarized (dark)" => "#002b36",
        "Solarized (light)" => "#fdf6e3",
        "base16-ocean.dark" => "#2b303b",
        "base16-ocean.light" => "#eff1f5",
        "base16-eighties.dark" => "#2d2d2d",
        "base16-mocha.dark" => "#3b3228",
        "InspiredGitHub" => "#ffffff",
        _ => "#002b36", // Default dark
    }
}

pub fn get_line_num_color(theme: &str) -> &str {
    if theme.contains("light") || theme == "InspiredGitHub" {
        "#93a1a1"
    } else {
        "#586e75"
    }
}

pub const SUPPORTED_THEMES: &[&str] = &[
    "InspiredGitHub",
    "Solarized (dark)",
    "Solarized (light)",
    "base16-eighties.dark",
    "base16-mocha.dark",
    "base16-ocean.dark",
    "base16-ocean.light",
];


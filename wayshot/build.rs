fn main() {
    #[cfg(target_os = "windows")]
    set_windows_info();

    _ = write_app_version();

    build_slint();
}

fn build_slint() {
    slint_build::compile("ui/desktop-window.slint").unwrap();
}

fn write_app_version() -> Result<(), Box<dyn std::error::Error>> {
    let tags = duct::cmd!("git", "describe", "--tags", "--abbrev=0")
        .read()?
        .split(char::is_whitespace)
        .map(|s| s.to_owned())
        .collect::<Vec<String>>();

    let output = if let Some(version) = tags.last() {
        format!(r#"pub static VERSION: &str = "{}";"#, version)
    } else {
        format!(r#"pub static VERSION: &str = "{}";"#, "0.0.1")
    };

    _ = std::fs::write("src/version.rs", output);

    Ok(())
}

#[cfg(target_os = "windows")]
fn set_windows_info() {
    _ = embed_resource::compile("./windows/icon.rc", embed_resource::NONE);
}

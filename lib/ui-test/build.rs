fn main() {
    let ui_base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../wayshot/ui/base");
    let config =
        slint_build::CompilerConfiguration::new().with_include_paths(vec![ui_base.clone()]);
    slint_build::compile_with_config(ui_base.join("update-dialog.slint"), config).unwrap();
}

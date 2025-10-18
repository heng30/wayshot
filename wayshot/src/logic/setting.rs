//! Settings panel logic module
//!
//! Handles application settings including preferences, proxy configuration,
//! AI model settings, backup/restore, and cache management.

use super::tr::tr;
use crate::{
    config, global_logic, global_store,
    slint_generatedAppWindow::{AppWindow, Theme},
    toast_success, toast_warn,
};
use slint::ComponentHandle;

/// Initializes settings panel logic
///
/// Sets up all settings-related callbacks including preferences,
/// proxy settings, AI model configuration, and utility functions.
///
/// # Parameters
/// - `ui`: Reference to the application window
pub fn init(ui: &AppWindow) {
    init_setting(ui);

    global_store!(ui).set_is_first_run(config::all().is_first_run);

    global_store!(ui).set_is_show_landing_page(config::all().is_first_run);

    global_logic!(ui).on_inner_tr(move |text, _lang| tr(text.as_str()).into());

    let ui_weak = ui.as_weak();
    global_logic!(ui).on_get_setting_preference(move || {
        let ui = ui_weak.unwrap();
        global_store!(ui).get_setting_preference()
    });

    let ui_weak = ui.as_weak();
    global_logic!(ui).on_set_setting_preference(move |mut setting| {
        let ui = ui_weak.unwrap();

        let font_size = u32::min(50, u32::max(10, setting.font_size.parse().unwrap_or(16)));
        setting.font_size = slint::format!("{}", font_size);

        let mut all = config::all();
        all.preference.win_width =
            u32::max(500, setting.win_width.to_string().parse().unwrap_or(500));
        all.preference.win_height =
            u32::max(800, setting.win_height.to_string().parse().unwrap_or(800));
        all.preference.font_size = font_size;
        all.preference.font_family = setting.font_family.into();
        all.preference.language = setting.language.into();
        all.preference.always_on_top = setting.always_on_top;
        all.preference.no_frame = setting.no_frame;
        all.preference.is_dark = setting.is_dark;
        _ = config::save(all);

        if cfg!(feature = "desktop") && !ui.window().is_maximized() {
            ui.global::<crate::Util>().invoke_update_window_size();
        }

        toast_success!(ui_weak.unwrap(), tr("save configuration successfully"));
    });

    let ui_weak = ui.as_weak();
    global_logic!(ui).on_increase_font_size(move || {
        let ui = ui_weak.unwrap();
        let mut all = config::all();

        let font_size = u32::min(50, u32::max(10, all.preference.font_size + 1));
        all.preference.font_size = font_size;
        _ = config::save(all);

        let mut setting = global_store!(ui).get_setting_preference();
        setting.font_size = slint::format!("{}", font_size);
        global_store!(ui).set_setting_preference(setting);
    });

    let ui_weak = ui.as_weak();
    global_logic!(ui).on_decrease_font_size(move || {
        let ui = ui_weak.unwrap();
        let mut all = config::all();

        let font_size = u32::min(50, u32::max(10, all.preference.font_size - 1));
        all.preference.font_size = font_size;
        _ = config::save(all);

        let mut setting = global_store!(ui).get_setting_preference();
        setting.font_size = slint::format!("{}", font_size);
        global_store!(ui).set_setting_preference(setting);
    });

    global_logic!(ui).on_get_setting_recorder(move || {
        let config = config::all().recorder;
        config.into()
    });

    let ui_weak = ui.as_weak();
    global_logic!(ui).on_set_setting_recorder(move |setting| {
        let mut all = config::all();
        all.recorder = setting.into();
        _ = config::save(all);

        toast_success!(ui_weak.unwrap(), tr("save configuration successfully"));
    });

    global_logic!(ui).on_get_setting_control(move || {
        let config = config::all().control;
        config.into()
    });

    global_logic!(ui).on_set_setting_control(move |setting| {
        let mut all = config::all();
        all.control = setting.into();
        _ = config::save(all);
    });

    let ui_weak = ui.as_weak();
    global_logic!(ui).on_remove_caches(move || {
        let ui = ui_weak.unwrap();
        let cache_dir = config::all().cache_dir;

        match cutil::fs::remove_dirs(&[&cache_dir]) {
            Err(e) => toast_warn!(
                ui,
                format!("{}. {}: {}", tr("Remove caches failed"), tr("Reason"), e)
            ),
            _ => {
                _ = std::fs::create_dir_all(&cache_dir);
                toast_success!(ui, tr("Remove caches successfully"));
            }
        }
    });

    global_logic!(ui).on_caches_size(|| {
        let bytes = cutil::fs::dirs_size(&[config::all().cache_dir]);
        cutil::fs::pretty_bytes_size(bytes).into()
    });

    #[cfg(feature = "desktop")]
    {
        let ui_weak = ui.as_weak();
        global_logic!(ui).on_backup(move |setting| {
            backup(ui_weak.clone(), setting);
        });

        let ui_weak = ui.as_weak();
        global_logic!(ui).on_recover(move || {
            recover(ui_weak.clone());
        });

        let ui_weak = ui.as_weak();
        global_logic!(ui).on_uninstall(move || {
            uninstall(ui_weak.clone());
        });
    }
}

/// Initializes setting values from configuration
///
/// Loads current configuration values into the settings UI.
///
/// # Parameters
/// - `ui`: Reference to the application window
fn init_setting(ui: &AppWindow) {
    let config = config::all().preference;
    let mut setting = global_store!(ui).get_setting_preference();

    let font_size = u32::min(50, u32::max(10, config.font_size));
    setting.win_width = slint::format!("{}", u32::max(500, config.win_width));
    setting.win_height = slint::format!("{}", u32::max(800, config.win_height));
    setting.font_size = slint::format!("{}", font_size);
    setting.font_family = config.font_family.into();
    setting.language = config.language.into();
    setting.always_on_top = config.always_on_top;
    setting.no_frame = config.no_frame;
    setting.is_dark = config.is_dark;

    ui.global::<Theme>().invoke_set_dark(config.is_dark);
    global_store!(ui).set_setting_preference(setting);
    global_store!(ui).set_setting_control(config::all().control.into());
}

/// Performs backup operation for desktop platforms
///
/// Creates a backup archive containing configuration and data files.
///
/// # Parameters
/// - `ui`: Weak reference to the application window
/// - `setting`: Backup settings including what to backup
#[cfg(feature = "desktop")]
fn backup(ui: slint::Weak<AppWindow>, setting: crate::slint_generatedAppWindow::SettingBackup) {
    use crate::logic::toast;

    tokio::spawn(async move {
        let result = native_dialog::DialogBuilder::file()
            .set_title(tr("Choose a directory"))
            .open_single_dir()
            .show();

        let output_dir = match result {
            Ok(Some(path)) => path,
            Err(e) => {
                toast::async_toast_warn(
                    ui,
                    format!("{}. {}: {}", tr("Choose directory failed"), tr("Reason"), e),
                );
                return;
            }
            _ => return,
        };

        let all = config::all();
        let filename = format!(
            "{}_{}.tar.gz",
            all.app_name,
            cutil::time::local_now("%Y-%m-%dT%H:%M:%S"),
        );
        let output = output_dir.join(filename);

        match (all.config_path.parent(), all.db_path.parent()) {
            (Some(config_dir), Some(data_dir)) => {
                let mut sources = vec![];
                let mut excludes = vec![];

                if setting.configuration {
                    sources.push(config_dir.to_path_buf());
                }

                if setting.data {
                    sources.push(data_dir.to_path_buf());
                }

                if !setting.cache {
                    excludes.push(all.cache_dir);
                }

                match cutil::backup_recover::create_backup(&sources, output.as_path(), &excludes) {
                    Err(e) => toast::async_toast_warn(
                        ui,
                        format!("{}. {}: {}", tr("Backup failed"), tr("Reason"), e),
                    ),
                    _ => toast::async_toast_success(ui, tr("Backup successfully")),
                }
            }
            _ => toast::async_toast_warn(
                ui,
                tr(&format!(
                    "Can't find config_path: {} or data_path: {}",
                    all.config_path.as_path().display(),
                    all.db_path.as_path().display()
                )),
            ),
        }
    });
}

/// Performs recovery operation for desktop platforms
///
/// Restores application data from a backup archive.
///
/// # Parameters
/// - `ui`: Weak reference to the application window
#[cfg(feature = "desktop")]
fn recover(ui: slint::Weak<AppWindow>) {
    use crate::logic::toast;

    tokio::spawn(async move {
        let result = native_dialog::DialogBuilder::file()
            .set_title(tr("Choose a backup file"))
            .open_single_file()
            .show();

        let input = match result {
            Ok(Some(path)) => path,
            Err(e) => {
                toast::async_toast_warn(
                    ui,
                    format!(
                        "{}. {}: {}",
                        tr("Choose backup file failed"),
                        tr("Reason"),
                        e
                    ),
                );
                return;
            }
            _ => return,
        };

        let config_all = config::all();

        match tempfile::tempdir() {
            Ok(target) => {
                let target = target.path();
                match cutil::backup_recover::restore_backup(input.as_path(), target) {
                    Err(e) => toast::async_toast_warn(
                        ui,
                        format!(
                            "{}. {}: {}",
                            tr("Restore backup file failed"),
                            tr("Reason"),
                            e
                        ),
                    ),
                    _ => {
                        let config_path = target
                            .join(&config_all.app_name)
                            .join(&format!("{}.toml", config_all.app_name));

                        _ = std::fs::copy(&config_path, config_all.config_path);
                        _ = std::fs::remove_file(&config_path);

                        if let Some(data_dir) = config_all.db_path.parent() {
                            _ = cutil::fs::copy_dir_all(
                                target.join(&config_all.app_name),
                                data_dir,
                            );
                        }

                        toast::async_toast_success(ui, tr("Restore backup file successfully"));
                    }
                }
            }
            Err(e) => toast::async_toast_warn(
                ui,
                format!("{}. {}: {}", tr("Can't create tempdir"), tr("Reason"), e),
            ),
        }
    });
}

/// Performs uninstall operation for desktop platforms
///
/// Removes application configuration and data directories.
///
/// # Parameters
/// - `ui`: Weak reference to the application window
#[cfg(feature = "desktop")]
fn uninstall(ui: slint::Weak<AppWindow>) {
    let ui = ui.unwrap();
    let all = config::all();
    let mut is_err = false;

    if let Some(config_path) = all.config_path.as_path().parent()
        && let Err(e) = cutil::fs::remove_dirs(&[config_path])
    {
        is_err = true;
        toast_warn!(
            ui,
            format!(
                "{}. {}: {}",
                tr("Remove configuration directory failed"),
                tr("Reason"),
                e
            )
        );
    }

    if let Some(data_path) = all.db_path.as_path().parent()
        && let Err(e) = cutil::fs::remove_dirs(&[data_path])
    {
        is_err = true;
        toast_warn!(
            ui,
            format!(
                "{}. {}: {}",
                tr("Remove data directory failed"),
                tr("Reason"),
                e
            )
        );
    }

    if !is_err {
        toast_success!(ui, tr("uninstall successfully"));
    }
}

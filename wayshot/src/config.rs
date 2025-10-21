//! Configuration management module
//!
//! Handles application configuration loading, saving, and management.
//! Supports platform-specific configuration directories and automatic
//! configuration file creation.

use crate::slint_generatedAppWindow::{
    Fps as UIFps, Resolution as UIResolution, SettingControl as UISettingControl,
    SettingRecorder as UISettingRecorder,
};
use anyhow::{Context, Result, bail};
use log::debug;
use once_cell::sync::Lazy;
use pmacro::SlintFromConvert;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};
use uuid::Uuid;

#[cfg(feature = "desktop")]
use platform_dirs::AppDirs;

/// Embedded Cargo.toml file content for package metadata
const CARGO_TOML: &str = include_str!("../Cargo.toml");

/// Global configuration instance protected by a mutex
static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::default()));

/// Android-specific application directories structure
///
/// Provides platform-specific directory paths for Android applications.
#[cfg(feature = "android")]
pub struct AppDirs {
    /// Configuration directory path
    pub config_dir: PathBuf,
    /// Data directory path
    pub data_dir: PathBuf,
}

#[cfg(feature = "android")]
impl AppDirs {
    /// Creates new Android application directories
    ///
    /// # Parameters
    /// - `name`: Application package name
    /// - `_`: Compatibility parameter (unused)
    ///
    /// # Returns
    /// - `Some(AppDirs)` if successful, `None` otherwise
    pub fn new(name: Option<&str>, _: bool) -> Option<Self> {
        let root_dir = "/data/data";
        let name = name.unwrap();

        Some(Self {
            config_dir: PathBuf::from(&format!("{root_dir}/{name}/config")),
            data_dir: PathBuf::from(&format!("{root_dir}/{name}/data")),
        })
    }
}

/// Main configuration structure containing all application settings
///
/// Includes paths, preferences, proxy settings, and AI model configurations.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Config {
    #[serde(skip)]
    pub config_path: PathBuf,

    #[serde(skip)]
    pub db_path: PathBuf,

    #[serde(skip)]
    pub cache_dir: PathBuf,

    #[serde(skip)]
    pub is_first_run: bool,

    #[serde(skip)]
    pub app_name: String,

    #[serde(default = "appid_default")]
    pub appid: String,

    pub preference: Preference,
    pub recorder: Recorder,
    pub control: Control,
}

/// User preference settings for the application
///
/// Contains window settings, font preferences, language, and UI options.
#[derive(Serialize, Deserialize, Debug, Clone, Derivative)]
#[derivative(Default)]
pub struct Preference {
    #[derivative(Default(value = "1000"))]
    pub win_width: u32,

    #[derivative(Default(value = "800"))]
    pub win_height: u32,

    #[derivative(Default(value = "16"))]
    pub font_size: u32,

    #[derivative(Default(value = "\"Source Han Sans CN\".to_string()"))]
    pub font_family: String,

    #[derivative(Default(value = "\"en\".to_string()"))]
    pub language: String,

    #[derivative(Default(value = "false"))]
    pub always_on_top: bool,

    #[derivative(Default(value = "false"))]
    pub no_frame: bool,

    pub is_dark: bool,
}

#[derive(Serialize, Deserialize, Derivative, Debug, Clone, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingRecorder")]
pub struct Recorder {
    pub save_dir: String,
    pub remove_temporary_files: bool,

    #[derivative(Default(value = "true"))]
    pub include_cursor: bool,

    #[derivative(Default(value = "true"))]
    pub enable_preview: bool,

    pub enable_denoise: bool,

    #[derivative(Default(value = "true"))]
    pub real_time_denoise: bool,

    pub convert_input_wav_to_mono: bool,

    #[derivative(Default(value = "fps_default()"))]
    pub fps: UIFps,

    #[derivative(Default(value = "resolution_default()"))]
    pub resolution: UIResolution,
}

crate::impl_slint_enum_serde!(UIFps, Fps24, Fps25, Fps30, Fps60);
crate::impl_slint_enum_serde!(UIResolution, Original, P720, P1080, P2K, P4K);

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingControl")]
pub struct Control {
    pub screen: String,
    pub input_audio: String,

    // db
    #[derivative(Default(value = "0.0"))]
    pub input_audio_gain: f32,

    #[derivative(Default(value = "true"))]
    pub enable_input_audio: bool,

    // db
    #[derivative(Default(value = "0.0"))]
    pub desktop_speaker_gain: f32,

    #[derivative(Default(value = "true"))]
    pub enable_desktop_speaker: bool,
}

impl Config {
    /// Initializes the configuration
    ///
    /// Loads package metadata, creates directories, and loads configuration file.
    ///
    /// # Returns
    /// - `Result<()>` indicating success or failure
    pub fn init(&mut self) -> Result<()> {
        let metadata = toml::from_str::<toml::Table>(CARGO_TOML).expect("Parse Cargo.toml error");

        self.app_name = metadata
            .get("package")
            .unwrap()
            .get("name")
            .unwrap()
            .to_string()
            .trim_matches('"')
            .to_string();

        let pkg_name = if cfg!(feature = "desktop") {
            self.app_name.clone()
        } else {
            metadata
                .get("package")
                .unwrap()
                .get("metadata")
                .unwrap()
                .get("android")
                .unwrap()
                .get("package")
                .unwrap()
                .to_string()
                .trim_matches('"')
                .to_string()
        };

        let app_dirs = AppDirs::new(Some(&pkg_name), true).unwrap();
        self.crate_dirs(&app_dirs)?;
        self.load().with_context(|| "load config file failed")?;
        debug!("{:?}", self);
        Ok(())
    }

    /// Creates application directories and sets up paths
    ///
    /// # Parameters
    /// - `app_dirs`: Platform-specific application directories
    ///
    /// # Returns
    /// - `Result<()>` indicating success or failure
    fn crate_dirs(&mut self, app_dirs: &AppDirs) -> Result<()> {
        self.db_path = app_dirs.data_dir.join(format!("{}.db", self.app_name));
        self.config_path = app_dirs.config_dir.join(format!("{}.toml", self.app_name));
        self.cache_dir = app_dirs.data_dir.join("cache");

        if self.appid.is_empty() {
            self.appid = appid_default();
        }

        fs::create_dir_all(&app_dirs.data_dir)?;
        fs::create_dir_all(&app_dirs.config_dir)?;
        fs::create_dir_all(&self.cache_dir)?;

        Ok(())
    }

    /// Loads configuration from file or creates default if not exists
    ///
    /// # Returns
    /// - `Result<()>` indicating success or failure
    fn load(&mut self) -> Result<()> {
        match fs::read_to_string(&self.config_path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(mut c) => {
                    c.config_path = self.config_path.clone();
                    c.db_path = self.db_path.clone();
                    c.cache_dir = self.cache_dir.clone();
                    c.is_first_run = self.is_first_run;
                    c.app_name = self.app_name.clone();
                    c.appid = self.appid.clone();
                    *self = c;

                    Ok(())
                }
                Err(_) => {
                    self.is_first_run = true;

                    if let Some(bak_file) = &self.config_path.as_os_str().to_str() {
                        _ = fs::copy(&self.config_path, format!("{}.bak", bak_file));
                    }

                    match toml::to_string_pretty(self) {
                        Ok(text) => Ok(fs::write(&self.config_path, text)?),
                        Err(e) => Err(e.into()),
                    }
                }
            },
            Err(_) => {
                self.is_first_run = true;

                if let Some(bak_file) = &self.config_path.as_os_str().to_str() {
                    _ = fs::copy(&self.config_path, format!("{}.bak", bak_file));
                }

                match toml::to_string_pretty(self) {
                    Ok(text) => Ok(fs::write(&self.config_path, text)?),
                    Err(e) => Err(e.into()),
                }
            }
        }
    }

    /// Saves the current configuration to file
    ///
    /// # Returns
    /// - `Result<()>` indicating success or failure
    pub fn save(&self) -> Result<()> {
        match toml::to_string_pretty(self) {
            Ok(text) => Ok(fs::write(&self.config_path, text)
                .with_context(|| "save config failed".to_string())?),
            Err(e) => bail!(format!("convert config from toml format failed. {e:?}")),
        }
    }
}

/// Generates a default application ID using UUID v4
///
/// # Returns
/// - Random UUID string
fn appid_default() -> String {
    Uuid::new_v4().to_string()
}

fn fps_default() -> UIFps {
    UIFps::Fps25
}

fn resolution_default() -> UIResolution {
    UIResolution::Original
}

/// Initializes the global configuration
///
/// This should be called once at application startup.
pub fn init() {
    CONFIG.lock().unwrap().init().unwrap();
}

/// Returns a clone of the current configuration
///
/// # Returns
/// - Current configuration instance
pub fn all() -> Config {
    CONFIG.lock().unwrap().clone()
}

/// Saves a new configuration and updates the global instance
///
/// # Parameters
/// - `conf`: New configuration to save
///
/// # Returns
/// - `Result<()>` indicating success or failure
pub fn save(conf: Config) -> Result<()> {
    let mut config = CONFIG.lock().unwrap();
    *config = conf;
    config.save()
}

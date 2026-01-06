use crate::slint_generatedAppWindow::{
    Fps as UIFps, RTCIceServer as UIRTCIceServer, Resolution as UIResolution,
    SettingCamera as UISettingCamera, SettingControl as UISettingControl,
    SettingCursorTracker as UISettingCursorTracker, SettingPushStream as UISettingPushStream,
    SettingRecorder as UISettingRecorder, SettingShareScreen as UISettingShareScreen,
    SettingShareScreenClient as UISettingShareScreenClient, TransitionType as UITransitionType,
};
use anyhow::{Context, Result, bail};
use log::debug;
use once_cell::sync::Lazy;
use pmacro::SlintFromConvert;
use recorder::TransitionType;
use serde::{Deserialize, Serialize};
use slint::Model;
use std::{fs, path::PathBuf, sync::Mutex};
use uuid::Uuid;

#[cfg(feature = "desktop")]
use platform_dirs::AppDirs;

const CARGO_TOML: &str = include_str!("../Cargo.toml");
static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::default()));

#[cfg(feature = "android")]
pub struct AppDirs {
    /// Configuration directory path
    pub config_dir: PathBuf,
    /// Data directory path
    pub data_dir: PathBuf,
}

#[cfg(feature = "android")]
impl AppDirs {
    pub fn new(name: Option<&str>, _: bool) -> Option<Self> {
        let root_dir = "/data/data";
        let name = name.unwrap();

        Some(Self {
            config_dir: PathBuf::from(&format!("{root_dir}/{name}/config")),
            data_dir: PathBuf::from(&format!("{root_dir}/{name}/data")),
        })
    }
}

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
    pub cursor_tracker: CursorTracker,

    #[serde(default)]
    pub share_screen: ShareScreen,

    #[serde(default)]
    pub share_screen_client: ShareScreenClient,

    #[serde(default)]
    pub push_stream: PushStream,
}

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

    #[derivative(Default(value = "true"))]
    pub include_cursor: bool,

    pub enable_denoise: bool,

    pub convert_to_mono: bool,

    #[derivative(Default(value = "fps_default()"))]
    pub fps: UIFps,

    #[derivative(Default(value = "resolution_default()"))]
    pub resolution: UIResolution,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingControl")]
pub struct Control {
    pub screen: String,
    pub audio: String,

    // db
    #[derivative(Default(value = "0.0"))]
    pub audio_gain: f32,

    #[derivative(Default(value = "true"))]
    pub enable_audio: bool,

    // db
    #[derivative(Default(value = "0.0"))]
    pub speaker_gain: f32,

    #[derivative(Default(value = "true"))]
    pub enable_speaker: bool,

    #[serde(default = "true_func")]
    #[derivative(Default(value = "true"))]
    pub enable_stats: bool,

    #[serde(default = "true_func")]
    #[derivative(Default(value = "true"))]
    pub enable_preview: bool,

    #[serde(default)]
    pub camera: String,

    #[serde(default)]
    pub enable_camera: bool,

    #[serde(default)]
    pub camera_setting: Camera,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingCursorTracker")]
pub struct CursorTracker {
    pub enable_tracking: bool,

    #[derivative(Default(value = "1280"))]
    pub region_width: i32,

    #[derivative(Default(value = "720"))]
    pub region_height: i32,

    #[derivative(Default(value = "30"))]
    pub debounce_radius: i32,

    #[derivative(Default(value = "30"))]
    pub stable_radius: i32,

    #[derivative(Default(value = "200"))]
    pub fast_moving_duration: i32,

    #[derivative(Default(value = "1000"))]
    pub zoom_transition_duration: i32,

    #[derivative(Default(value = "0.15"))]
    pub reposition_edge_threshold: f32,

    #[derivative(Default(value = "300"))]
    pub reposition_transition_duration: i32,

    #[derivative(Default(value = "5"))]
    pub max_stable_region_duration: i32,

    pub zoom_in_transition_type: UITransitionType,
    pub zoom_out_transition_type: UITransitionType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UISettingShareScreenClient")]
pub struct ShareScreenClient {
    pub enable_auth: bool,
    pub auth_token: String,
    pub server_addr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIRTCIceServer")]
pub struct RTCIceServer {
    pub url: String,
    pub username: String,
    pub credential: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingShareScreen")]
pub struct ShareScreen {
    #[derivative(Default(value = "true"))]
    pub save_mp4: bool,

    #[derivative(Default(value = "\"0.0.0.0:9090\".to_string()"))]
    pub listen_addr: String,

    pub auth_token: String,

    pub enable_turn_server: bool,
    pub turn_server: RTCIceServer,

    pub enable_stun_server: bool,
    pub stun_server: RTCIceServer,

    #[vec(from = "host_ips")]
    pub host_ips: Vec<String>,
    pub disable_host_ipv6: bool,

    pub enable_https: bool,
    pub cert_file: String,
    pub key_file: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingPushStream")]
pub struct PushStream {
    #[derivative(Default(value = "true"))]
    pub save_mp4: bool,

    #[derivative(Default(value = "\"rtmp://localhost:1935\".to_string()"))]
    pub server_addr: String,

    #[derivative(Default(value = "\"live\".to_string()"))]
    pub app: String,

    #[derivative(Default(value = "\"stream\".to_string()"))]
    pub stream_key: String,

    pub query_params: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingCamera")]
pub struct Camera {
    pub mirror_horizontal: bool,

    #[derivative(Default(value = "25"))]
    pub fps: i32,

    #[derivative(Default(value = "UIResolution::P480"))]
    pub resolution: UIResolution,

    #[derivative(Default(value = "0.8"))]
    pub camera_x: f32,

    #[derivative(Default(value = "0.8"))]
    pub camera_y: f32,

    #[derivative(Default(value = "3"))]
    pub border_size: i32,

    pub border_color_index: i32,

    #[derivative(Default(value = "true"))]
    pub is_circle_shape: bool,

    #[derivative(Default(value = "300"))]
    pub rect_cropping_width: i32,

    #[derivative(Default(value = "300"))]
    pub rect_cropping_height: i32,

    #[derivative(Default(value = "0.5"))]
    pub rect_cropping_x: f32,

    #[derivative(Default(value = "0.5"))]
    pub rect_cropping_y: f32,

    #[derivative(Default(value = "150"))]
    pub circle_cropping_radius: i32,

    #[derivative(Default(value = "0.5"))]
    pub circle_cropping_x: f32,

    #[derivative(Default(value = "0.5"))]
    pub circle_cropping_y: f32,

    #[derivative(Default(value = "1.0"))]
    pub cropping_zoom: f32,
}

crate::impl_slint_enum_serde!(UIFps, Fps24, Fps25, Fps30, Fps60);
crate::impl_slint_enum_serde!(UIResolution, Original, P480, P720, P1080, P2K, P4K);
crate::impl_slint_enum_serde!(UITransitionType, Linear, EaseIn, EaseOut);
crate::impl_c_like_enum_convert!(UITransitionType, TransitionType, Linear, EaseIn, EaseOut);

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

fn true_func() -> bool {
    true
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

use crate::{
    config, global_store,
    logic::{toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{AppWindow, SettingShareScreen as UISettingShareScreen},
};
use recorder::{RTCIceServer, ShareScreenConfig};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::path::PathBuf;

#[macro_export]
macro_rules! host_ips {
    ($ips:expr) => {
        $ips.as_any()
            .downcast_ref::<VecModel<SharedString>>()
            .expect("We know we set a VecModel<SharedString> earlier for host_ips")
    };
}

pub fn init(ui: &AppWindow) {
    logic_cb!(add_host_ip, ui, ips, ip);
    logic_cb!(remove_host_ip, ui, ips, index);
    logic_cb!(load_share_screen_cert_file, ui);
    logic_cb!(load_share_screen_key_file, ui);
    logic_cb!(verify_setting_share_screen, ui, setting);
}

fn add_host_ip(_ui: &AppWindow, ips: ModelRc<SharedString>, ip: SharedString) {
    host_ips!(ips).insert(0, ip);
}

fn remove_host_ip(_ui: &AppWindow, ips: ModelRc<SharedString>, index: i32) {
    if index < 0 || index >= host_ips!(ips).row_count() as i32 {
        return;
    }

    host_ips!(ips).remove(index as usize);
}

fn load_share_screen_cert_file(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose Certification"),
            &tr("Certification"),
            &["crt", "pem"],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath = filepath.to_string_lossy().to_string().into();
            global_store!(ui).set_setting_share_screen_cert_file(filepath);
        });
    });
}

fn load_share_screen_key_file(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose private key file"),
            &tr("Private Key"),
            &["key", "pem"],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath = filepath.to_string_lossy().to_string().into();
            global_store!(ui).set_setting_share_screen_key_file(filepath);
        });
    });
}

fn verify_setting_share_screen(_ui: &AppWindow, config: UISettingShareScreen) -> SharedString {
    if config.enable_stun_server && !config.stun_server.url.starts_with("stun") {
        return tr("Invalid STUN server url format. Should start with `stun:`").into();
    }

    if config.enable_turn_server && !config.turn_server.url.starts_with("turn:") {
        return tr("Invalid TURN server url format. Should start with `turn:`").into();
    }

    SharedString::default()
}

pub fn picker_file(
    ui: Weak<AppWindow>,
    title: &str,
    filter_name: &str,
    filter_extensions: &[&str],
) -> Option<PathBuf> {
    let mut file_dialog = native_dialog::DialogBuilder::file().set_title(title);

    if !filter_extensions.is_empty() {
        file_dialog = file_dialog.add_filter(filter_name, filter_extensions);
    }

    let result = file_dialog.open_single_file().show();

    match result {
        Ok(Some(path)) => Some(path),
        Err(e) => {
            toast::async_toast_warn(
                ui,
                format!("{}. {}: {}", tr("Choose file failed"), tr("Reason"), e),
            );
            None
        }
        _ => None,
    }
}

impl From<config::ShareScreen> for ShareScreenConfig {
    fn from(c: config::ShareScreen) -> ShareScreenConfig {
        ShareScreenConfig::new(c.listen_addr)
            .with_save_mp4(c.save_mp4)
            .with_disable_host_ipv6(c.disable_host_ipv6)
            .with_enable_https(c.enable_https)
            .with_cert_file(Some(c.cert_file))
            .with_key_file(Some(c.key_file))
            .with_host_ips(c.host_ips)
            .with_auth_token(if c.auth_token.trim().is_empty() {
                None
            } else {
                Some(c.auth_token.trim().to_string())
            })
            .with_stun_server(if c.enable_stun_server {
                Some(c.stun_server.into())
            } else {
                None
            })
            .with_turn_server(if c.enable_turn_server {
                Some(c.turn_server.into())
            } else {
                None
            })
            .with_host_ips(vec!["192.168.10.8".to_string()])
    }
}

impl From<config::RTCIceServer> for RTCIceServer {
    fn from(c: config::RTCIceServer) -> Self {
        RTCIceServer {
            urls: vec![c.url],
            username: c.username,
            credential: c.credential,
        }
    }
}

use crate::{
    config, global_store,
    logic::{recorder::get_async_error_sender, toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, ConnectionStatus, SettingShareScreen as UISettingShareScreen,
        SettingShareScreenClient as UISettingShareScreenClient,
    },
    toast_warn,
};
use once_cell::sync::Lazy;
use recorder::{RTCIceServer, ShareScreenConfig};
use slint::{ComponentHandle, Model, ModelRc, SharedPixelBuffer, SharedString, VecModel, Weak};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Notify, mpsc::channel};
use wrtc::client::{AudioSamples, RGBFrame, WHEPClient, WHEPClientConfig};

#[derive(Default)]
struct Cache {
    exit_notify: Option<Arc<Notify>>,
    player_stop_sig: Option<Arc<AtomicBool>>,
    audio_sink: Option<Arc<rodio::Sink>>,
    audio_stream: Option<Arc<rodio::OutputStream>>,
}

static CACHE: Lazy<Mutex<Cache>> = Lazy::new(|| Mutex::new(Cache::default()));

#[macro_export]
macro_rules! host_ips {
    ($ips:expr) => {
        $ips.as_any()
            .downcast_ref::<VecModel<SharedString>>()
            .expect("We know we set a VecModel<SharedString> earlier for host_ips")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    // share screen server
    logic_cb!(share_screen_add_host_ip, ui, ips, ip);
    logic_cb!(share_screen_remove_host_ip, ui, ips, index);
    logic_cb!(share_screen_load_cert_file, ui);
    logic_cb!(share_screen_load_key_file, ui);
    logic_cb!(share_screen_verify_setting, ui, setting);

    // share screen client
    logic_cb!(share_screen_player_play, ui);
    logic_cb!(share_screen_player_stop, ui);
    logic_cb!(share_screen_player_sound_changed, ui, progress);
    logic_cb!(share_screen_client_disconnect, ui);
    logic_cb!(share_screen_client_connect, ui, setting);
    logic_cb!(convert_to_meida_time, ui, duration);
}

fn inner_init(ui: &AppWindow) {
    match rodio::OutputStreamBuilder::open_default_stream() {
        Ok(stream) => {
            let sink = rodio::Sink::connect_new(stream.mixer());
            sink.set_volume(0.5);
            CACHE.lock().unwrap().audio_sink = Some(Arc::new(sink));
            CACHE.lock().unwrap().audio_stream = Some(Arc::new(stream));
            log::info!("Audio playback stream initialized");
        }
        Err(e) => toast_warn!(ui, format!("Failed to create audio output stream: {e}")),
    }
}

fn share_screen_add_host_ip(_ui: &AppWindow, ips: ModelRc<SharedString>, ip: SharedString) {
    host_ips!(ips).insert(0, ip);
}

fn share_screen_remove_host_ip(_ui: &AppWindow, ips: ModelRc<SharedString>, index: i32) {
    if index < 0 || index >= host_ips!(ips).row_count() as i32 {
        return;
    }

    host_ips!(ips).remove(index as usize);
}

fn share_screen_load_cert_file(ui: &AppWindow) {
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

fn share_screen_load_key_file(ui: &AppWindow) {
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

fn share_screen_verify_setting(_ui: &AppWindow, config: UISettingShareScreen) -> SharedString {
    if config.enable_stun_server && !config.stun_server.url.starts_with("stun") {
        return tr("Invalid STUN server url format. Should start with `stun:`").into();
    }

    if config.enable_turn_server && !config.turn_server.url.starts_with("turn:") {
        return tr("Invalid TURN server url format. Should start with `turn:`").into();
    }

    SharedString::default()
}

fn share_screen_player_play(_ui: &AppWindow) {
    if let Some(ref stop_sig) = CACHE.lock().unwrap().player_stop_sig {
        stop_sig.store(false, Ordering::Relaxed);
    }
}

fn share_screen_player_stop(_ui: &AppWindow) {
    if let Some(ref stop_sig) = CACHE.lock().unwrap().player_stop_sig {
        stop_sig.store(true, Ordering::Relaxed);
    }

    if let Some(ref sink) = CACHE.lock().unwrap().audio_sink {
        sink.stop();
    }
}

fn share_screen_player_sound_changed(_ui: &AppWindow, progress: f32) {
    if let Some(ref sink) = CACHE.lock().unwrap().audio_sink {
        sink.set_volume((progress / 100.0).clamp(0.0, 1.0));
    }
}

fn share_screen_client_disconnect(ui: &AppWindow) {
    global_store!(ui).set_share_screen_client_connection_status(ConnectionStatus::Disconnected);

    if let Some(ref sink) = CACHE.lock().unwrap().audio_sink {
        sink.stop();
    }

    if let Some(ref stop_sig) = CACHE.lock().unwrap().player_stop_sig {
        stop_sig.store(true, Ordering::Relaxed);
    }

    if let Some(ref notify) = CACHE.lock().unwrap().exit_notify {
        notify.notify_waiters();
    }
}

fn share_screen_client_connect(ui: &AppWindow, setting: UISettingShareScreenClient) {
    if !(setting.server_addr.starts_with("http://") || setting.server_addr.starts_with("https://"))
    {
        toast_warn!(
            ui,
            "server address should start with `http://` or `https://`".to_string()
        );
        return;
    }

    global_store!(ui).set_share_screen_client_connection_status(ConnectionStatus::Connecting);

    let ui_weak = ui.as_weak();
    let error_sender = get_async_error_sender();
    let audio_sink = CACHE.lock().unwrap().audio_sink.clone();

    if let Some(ref stop_sig) = CACHE.lock().unwrap().player_stop_sig {
        stop_sig.store(true, Ordering::Relaxed);
    }
    let player_stop_sig = Arc::new(AtomicBool::new(false));
    CACHE.lock().unwrap().player_stop_sig = Some(player_stop_sig.clone());

    if let Some(ref notify) = CACHE.lock().unwrap().exit_notify {
        notify.notify_waiters();
    }
    let exit_notify = Arc::new(Notify::new());
    let exit_notify_clone = exit_notify.clone();
    CACHE.lock().unwrap().exit_notify = Some(exit_notify.clone());

    tokio::spawn(async move {
        log::info!("Connecting to WHEP server at: {}", setting.server_addr);

        let (video_tx, mut video_rx) = channel::<RGBFrame>(64);
        let (audio_tx, mut audio_rx) = channel::<AudioSamples>(64);

        let mut config = WHEPClientConfig::new(setting.server_addr.trim().to_string().into());
        if setting.enable_auth {
            config = config.with_auth_token(setting.auth_token.into());
        }

        let client = match WHEPClient::new(config, Some(video_tx), Some(audio_tx), exit_notify)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                if let Some(sender) = error_sender
                    && let Err(e) =
                        sender.try_send(format!("Create share screen client failed: {e}"))
                {
                    log::warn!("try send async error failed: {e}");
                }

                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    global_store!(ui)
                        .set_share_screen_client_connection_status(ConnectionStatus::Disconnected);
                });
                return;
            }
        };

        let ui_weak_clone = ui_weak.clone();
        let media_info = client.media_info.clone();

        tokio::spawn(async move {
            match client.connect().await {
                Ok(_) => log::info!("WHEP client connection completed successfully"),
                Err(e) => {
                    let err = format!("Failed to connect to WHEP server: {e}");
                    log::warn!("{err}");

                    if let Some(sender) = error_sender
                        && let Err(e) = sender.try_send(err)
                    {
                        log::warn!("error send try send failed: {e}");
                    }
                }
            }

            _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                global_store!(ui)
                    .set_share_screen_client_connection_status(ConnectionStatus::Disconnected);
            });
        });

        loop {
            tokio::select! {
                _ = exit_notify_clone.notified() => {
                    log::info!("share screen video and audio loop exit...");
                    break;
                }

                frame = video_rx.recv() => {
                    match frame {
                        Some((width, height, rgb_data)) => {
                            log::trace!(
                                "Received video frame: {}x{} ({} bytes)",
                                width,
                                height,
                                rgb_data.len()
                            );

                            if player_stop_sig.load(Ordering::Relaxed) {
                                continue;
                            }

                            let player_stop_sig_clone = player_stop_sig.clone();
                            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                if player_stop_sig_clone.load(Ordering::Relaxed) {
                                    return;
                                }

                                let buffer = SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                                    &rgb_data, width, height,
                                );
                                let img = slint::Image::from_rgb8(buffer);

                                global_store!(ui).set_share_screen_player_image(img);
                                global_store!(ui).set_share_screen_client_connection_status(ConnectionStatus::Connected);
                            });
                        }
                        None =>  {
                            log::info!("share screen video receiver exit...");
                            break;
                        }
                    }
                }

                samples = audio_rx.recv() => {
                    match samples  {
                        Some(samples) => {
                            if player_stop_sig.load(Ordering::Relaxed) {
                                continue;
                            }

                            if let Some(ref audio_info) = media_info.audio {
                                log::trace!(
                                    "Received audio packet: {} Hz, {} channels, {} samples ({:.2}ms)",
                                    audio_info.sample_rate,
                                    audio_info.channels,
                                    samples.len(),
                                    (samples.len() as f64 / audio_info.channels as f64 / audio_info.sample_rate as f64) * 1000.0
                                );

                                if let Some(ref sink) = audio_sink {
                                    sink.append(rodio::buffer::SamplesBuffer::new(
                                            audio_info.channels,
                                            audio_info.sample_rate,
                                            samples,
                                    ));

                                    sink.play();
                                } else {
                                    log::warn!("Audio sink not initialized - cannot play audio");
                                }
                            }
                        }
                        None =>  {
                            log::info!("share screen audio receiver exit...");
                            break;
                        }
                    }
                }
            }
        }
    });
}

fn convert_to_meida_time(_ui: &AppWindow, duration: i32) -> SharedString {
    cutil::time::seconds_to_media_timestamp(duration.max(0) as f64).into()
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

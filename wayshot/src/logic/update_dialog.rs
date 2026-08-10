use crate::{
    config, global_store, logic_cb, slint_generatedAppWindow::AppWindow, version::VERSION,
};
use serde::Deserialize;
use slint::{ComponentHandle, SharedString};

const UPDATE_CHECK_URL: &str = "https://api.github.com/repos/heng30/wayshot/releases/latest";
const UPDATE_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
}

pub fn init(ui: &AppWindow) {
    inner_init(&ui);

    logic_cb!(update_dialog_dont_ask_again, ui, latest_version);
}

fn update_dialog_dont_ask_again(_ui: &AppWindow, latest_version: SharedString) {
    let mut all = config::all();
    all.skip_update_version = latest_version.trim().to_string();
    if let Err(e) = config::save(all) {
        log::warn!("save skip update version failed: {e}");
    }
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        match fetch_latest_release().await {
            Ok(release) => {
                log::info!("latest release: {release:?}");

                let skip_version = config::all().skip_update_version;
                if version_greater(&release.tag_name, VERSION) && skip_version != release.tag_name {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        global_store!(ui).set_latest_version(release.tag_name.into());
                        global_store!(ui).set_update_content(release.body.into());
                        global_store!(ui).set_is_show_update_dialog(true);
                    });
                }
            }
            Err(e) => log::warn!("check update failed: {e}"),
        }
    });
}

async fn fetch_latest_release() -> anyhow::Result<LatestRelease> {
    let client = reqwest::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .build()?;
    let release = client
        .get(UPDATE_CHECK_URL)
        .headers(cutil::http::headers())
        .send()
        .await?
        .error_for_status()?
        .json::<LatestRelease>()
        .await?;
    Ok(release)
}

fn version_greater(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches(&['v', 'V'])
            .split('.')
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    parse(latest) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::version_greater;

    #[test]
    fn version_greater_semantic() {
        assert!(version_greater("v1.1.0", "v1.0.2"));
        assert!(version_greater("v1.1.0", "V1.0.2"));
        assert!(version_greater("v1.10.0", "v1.9.0"), "多位数段应逐段比较");
        assert!(version_greater("1.0.3", "v1.0.2"), "不带 v 前缀也应可比较");
        assert!(!version_greater("V1.0.2", "v1.0.2"), "同版本不算更新");
        assert!(!version_greater("v1.0.2", "v1.1.0"), "旧版本不算更新");
        assert!(!version_greater("v1.0", "v1.0.2"), "位数少的不算更新");
        assert!(!version_greater("garbage", "v1.0.2"), "解析失败不算更新");
    }
}

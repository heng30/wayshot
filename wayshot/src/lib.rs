slint::include_modules!();

#[macro_use]
extern crate derivative;

#[cfg(feature = "jemalloc")]
extern crate tikv_jemallocator;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

// Configure jemalloc at compile-time via the malloc_conf symbol.
// dirty_decay_ms:10 — purge dirty pages after 10s via background threads.
// muzzy_decay_ms:0  — immediately return muzzy pages to the OS.
// This symbol is read by jemalloc on init, before any allocation.
#[cfg(feature = "jemalloc")]
#[unsafe(no_mangle)]
pub static malloc_conf: &[u8] = b"dirty_decay_ms:10,muzzy_decay_ms:0\0";

#[cfg(feature = "database")]
mod db;

mod config;
mod logic;
mod version;

pub fn init_logger() {
    use std::io::Write;

    env_logger::builder()
        .filter_module("webrtc", log::LevelFilter::Warn)
        .filter_module("webrtc_srtp", log::LevelFilter::Warn)
        .format(|buf, record| {
            let style = buf.default_level_style(record.level());
            let ts = cutil::time::local_now("%H:%M:%S");

            writeln!(
                buf,
                "[{} {style}{}{style:#} {}::{} {}] {}",
                ts,
                record.level(),
                record
                    .module_path()
                    .unwrap_or("None")
                    .split("::")
                    .next()
                    .unwrap_or("None"),
                record
                    .file()
                    .unwrap_or("None")
                    .split('/')
                    .next_back()
                    .unwrap_or("None"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();
}

async fn ui_before() {
    init_logger();
    config::init();

    #[cfg(feature = "database")]
    db::init(config::all().db_path.to_str().expect("invalid db path")).await;

    #[cfg(target_os = "linux")]
    {
        _ = slint::set_xdg_app_id("wayshot".to_string());
    }
}

fn ui_after(ui: &AppWindow) {
    logic::init(ui);
}

pub async fn desktop_main() {
    log::debug!("start...");

    ui_before().await;
    let ui = AppWindow::new().unwrap();
    global_store!(ui).set_device_type(DeviceType::Desktop);
    ui_after(&ui);

    global_util!(ui).invoke_set_window_center();

    ui.run().unwrap();

    log::debug!("exit...");
}

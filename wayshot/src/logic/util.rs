//! Utility functions module
//!
//! Provides various utility functions for UI operations including window management,
//! string manipulation, date handling, QR code generation, and URL opening.

use super::tr::tr;
use crate::{
    config, global_util,
    slint_generatedAppWindow::{AppPosType, AppWindow, Date as UIDate},
    toast_warn,
};
use cutil::{self, number, time};
use rand::{self, Rng};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::str::FromStr;
use webbrowser::{self, Browser};

#[cfg(feature = "center-window")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "center-window")]
#[derive(Debug, Serialize, Deserialize)]
struct DisplayMode {
    width: u32,
    height: u32,
    current: bool,
}

#[cfg(feature = "center-window")]
#[derive(Debug, Serialize, Deserialize)]
struct Display {
    modes: Vec<DisplayMode>,
}

/// Initializes utility functions
///
/// Sets up callbacks for window management, string utilities,
/// date handling, and other utility functions.
///
/// # Parameters
/// - `ui`: Reference to the application window
pub fn init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    global_util!(ui).on_hide_window(move || {
        _ = ui_weak.unwrap().hide();
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_show_window(move || {
        _ = ui_weak.unwrap().show();
    });

    global_util!(ui).on_close_window(move || {
        std::process::exit(0);
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_min_window(move |minimized| {
        ui_weak.unwrap().window().set_minimized(minimized);
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_get_is_min_window(move || ui_weak.unwrap().window().is_minimized());

    let ui_weak = ui.as_weak();
    global_util!(ui).on_max_window(move |maximized| {
        ui_weak.unwrap().window().set_maximized(maximized);
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_get_is_max_window(move || ui_weak.unwrap().window().is_maximized());

    let ui_weak = ui.as_weak();
    global_util!(ui).on_fullscreen(move |fullscreen| {
        ui_weak.unwrap().window().set_fullscreen(fullscreen);
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_get_is_fullscreen(move || ui_weak.unwrap().window().is_fullscreen());

    let ui_weak = ui.as_weak();
    global_util!(ui).on_get_scale_factor(move || ui_weak.unwrap().window().scale_factor());

    let ui_weak = ui.as_weak();
    global_util!(ui).on_get_current_pos(move || {
        let ui = ui_weak.unwrap();
        let scale = ui.window().scale_factor();
        let pos = slint::LogicalPosition::from_physical(ui.window().position(), scale);

        AppPosType { x: pos.x, y: pos.y }
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_set_current_pos(move |pos| {
        let ui = ui_weak.unwrap();
        let scale = ui.window().scale_factor();
        let pos = slint::PhysicalPosition::from_logical(
            slint::LogicalPosition { x: pos.x, y: pos.y },
            scale,
        );

        ui.window().set_position(pos);
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_update_window_size(move || {
        let ui = ui_weak.unwrap();
        let preference = config::all().preference;

        let scale = ui.window().scale_factor();
        let psize = slint::PhysicalSize::from_logical(
            slint::LogicalSize {
                width: preference.win_width as f32,
                height: preference.win_height as f32,
            },
            scale,
        );
        ui.window().set_size(psize);
    });

    #[cfg(feature = "center-window")]
    {
        let ui_weak = ui.as_weak();
        global_util!(ui).on_set_window_center(move || {
            let ui = ui_weak.unwrap();
            let preference = config::all().preference;

            let scale = ui.window().scale_factor();
            let psize = slint::PhysicalSize::from_logical(
                slint::LogicalSize {
                    width: preference.win_width as f32,
                    height: preference.win_height as f32,
                },
                scale,
            );

            match display_size() {
                Some((w, h)) => {
                    log::info!("display size = ({w}, {h})");

                    if w > psize.width && h > psize.height {
                        let x = ((w - psize.width) / 2) as f32;
                        let y = ((h - psize.height) / 2) as f32;

                        log::info!("current pos = ({x}, {y})");

                        let pos = slint::PhysicalPosition::from_logical(
                            slint::LogicalPosition { x, y },
                            scale,
                        );

                        ui.window().set_position(pos)
                    }
                }
                _ => {
                    log::warn!("can't get display size");
                }
            }
        });
    }

    global_util!(ui).on_string_fixed2(move |n| {
        let n = n.to_string().parse::<f32>().unwrap_or(0.0f32);
        slint::format!("{:2}", (n * 100.0).round() / 100.0)
    });

    global_util!(ui).on_float_fixed2(move |n| slint::format!("{:2}", (n * 100.0).round() / 100.0));

    let ui_weak = ui.as_weak();
    global_util!(ui).on_open_url(move |browser, url| {
        let ui = ui_weak.unwrap();

        let browser = Browser::from_str(&browser.to_lowercase()).unwrap_or_default();

        let browser = if browser.exists() {
            browser
        } else {
            Browser::Default
        };

        if let Err(e) = webbrowser::open_browser(browser, url.as_str()) {
            toast_warn!(
                ui,
                format!("{}{}: {:?}", tr("Open link failed"), tr("Reason"), e)
            );
        }
    });

    global_util!(ui).on_remove_str_items_after(move |items, index| {
        let index = i32::max(0, index) as usize;

        let items = items
            .as_any()
            .downcast_ref::<VecModel<SharedString>>()
            .expect("We know we set a VecModel earlier");

        if index >= items.row_count() - 1 {
            return;
        }

        let count = items.row_count() - 1 - index;
        for _ in 0..count {
            items.remove(index + 1);
        }
    });

    global_util!(ui).on_append_str_to_items(move |items, text| {
        let items = items
            .as_any()
            .downcast_ref::<VecModel<SharedString>>()
            .expect("We know we set a VecModel earlier");

        items.push(text);
    });

    global_util!(ui).on_search_str_items_by(move |items, text| {
        if text.is_empty() {
            return ModelRc::default();
        }

        let items = items
            .iter()
            .filter(|item| item.to_lowercase().contains(text.to_lowercase().as_str()))
            .collect::<Vec<_>>();

        ModelRc::new(VecModel::from_slice(&items[..]))
    });

    global_util!(ui).on_find_tree_children_nodes(move |items, target_node| {
        if target_node.is_empty() {
            return ModelRc::default();
        }

        let items = items
            .iter()
            .filter(|item| item.parent_node == target_node)
            .collect::<Vec<_>>();

        ModelRc::new(VecModel::from_slice(&items[..]))
    });

    global_util!(ui).on_format_number_with_commas(move |number_str| {
        number::format_number_with_commas(number_str.as_str()).into()
    });

    global_util!(ui).on_local_now(move |format| time::local_now(format.as_str()).into());

    global_util!(ui).on_rand_int(move |low, up| rand::rng().random_range(low..up) as i32);

    global_util!(ui).on_split_and_join_string(move |input, length, sep| {
        cutil::str::split_string_to_fixed_length_parts(input.as_str(), length as usize)
            .join(sep.as_str())
            .into()
    });

    global_util!(ui).on_get_current_date(|| {
        let date = cutil::time::get_current_date();

        UIDate {
            year: date.year,
            month: date.month as i32,
            day: date.day as i32,
            main_month: date.month as i32,
        }
    });

    let ui_weak = ui.as_weak();
    global_util!(ui).on_parse_date_str(move |date| match cutil::time::parse_date_str(&date) {
        Ok(date) => UIDate {
            year: date.year,
            month: date.month as i32,
            day: date.day as i32,
            main_month: date.month as i32,
        },
        _ => global_util!(ui_weak.unwrap()).invoke_get_current_date(),
    });

    global_util!(ui).on_upate_date_picker(|year: i32, month: i32| {
        match cutil::time::get_calendar_matrix(year, month as u32) {
            Ok(dates) => ModelRc::new(
                dates
                    .into_iter()
                    .map(|row| {
                        ModelRc::new(VecModel::from_slice(
                            row.into_iter()
                                .map(|item| UIDate {
                                    year: item.year,
                                    month: item.month as i32,
                                    day: item.day as i32,
                                    main_month: month,
                                })
                                .collect::<Vec<_>>()
                                .as_slice(),
                        ))
                    })
                    .collect::<VecModel<ModelRc<UIDate>>>(),
            ),
            Err(e) => {
                log::debug!("{e:?}");

                ModelRc::new(
                    [[0; 7]; 6]
                        .into_iter()
                        .map(|row| {
                            ModelRc::new(VecModel::from_slice(
                                row.into_iter()
                                    .map(|_| UIDate::default())
                                    .collect::<Vec<_>>()
                                    .as_slice(),
                            ))
                        })
                        .collect::<VecModel<ModelRc<UIDate>>>(),
                )
            }
        }
    });

    global_util!(ui).on_seconds_to_media_timestamp(move |seconds| {
        cutil::time::seconds_to_media_timestamp(seconds as f64).into()
    });

    #[cfg(feature = "qrcode")]
    {
        init_qrcode(ui);
    }
}

/// Checks if the current session is using Wayland
///
/// # Returns
/// - `true` if Wayland session is detected
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t == "wayland")
            .unwrap_or(false)
}

/// Initializes QR code generation functionality
///
/// # Parameters
/// - `ui`: Reference to the application window
#[cfg(feature = "qrcode")]
pub fn init_qrcode(ui: &AppWindow) {
    use crate::slint_generatedAppWindow::Icons;
    use image::Rgb;
    use qrcode::QrCode;
    use slint::{Image, Rgb8Pixel, SharedPixelBuffer};

    let ui_weak = ui.as_weak();
    global_util!(ui).on_qr_code(move |text| {
        let ui = ui_weak.unwrap();
        match QrCode::new(text) {
            Ok(code) => {
                let qrc = code.render::<Rgb<u8>>().build();

                let buffer = SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
                    qrc.as_raw(),
                    qrc.width(),
                    qrc.height(),
                );
                Image::from_rgb8(buffer)
            }
            _ => ui.global::<Icons>().get_no_data_fill(),
        }
    });
}

/// Gets the current display size for window centering
///
/// Supports both Wayland and traditional display detection.
///
/// # Returns
/// - `Some((width, height))` if display size can be determined
#[cfg(feature = "center-window")]
pub fn display_size() -> Option<(u32, u32)> {
    #[cfg(target_os = "linux")]
    {
        if is_wayland() {
            if let Ok(json_data) = duct::cmd!("wlr-randr", "--json").read() {
                if let Ok(displays) = serde_json::from_str::<Vec<Display>>(&json_data) {
                    for display in displays {
                        for mode in display.modes {
                            if mode.current {
                                return Some((mode.width, mode.height));
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok(displays) = display_info::DisplayInfo::all() {
        for display in displays.iter() {
            if display.is_primary {
                return Some((display.width, display.height));
            }
        }

        if !displays.is_empty() {
            return Some((displays[0].width, displays[0].height));
        }
    }

    None
}

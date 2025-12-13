use crate::{
    config,
    db::{PLAYER_SETTING_TABLE as DB_TABLE, SettingPlayer},
    global_store,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, HistoryEntry as UIHistoryEntry, PlaylistItem as UIPlaylistItem,
    },
    store_history_entries, toast_warn,
};
use mp4_player::{Config as PlayerConfig, DecodedVideoFrame, Mp4Player};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, SharedPixelBuffer, VecModel};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

pub fn init(ui: &AppWindow) {}

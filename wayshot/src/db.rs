use pmacro::SlintFromConvert;
use serde::{Deserialize, Serialize};

use crate::slint_generatedAppWindow::{
    HistoryEntry as UIHistoryEntry, SettingPlayer as UISettingPlayer,
};
pub const HISTORY_TABLE: &str = "history";
pub const PLAYER_SETTING_TABLE: &str = "player_setting";

pub async fn init(db_path: &str) {
    sqldb::create_db(db_path).await.expect("create db");

    sqldb::entry::new(HISTORY_TABLE)
        .await
        .expect("history table failed");

    sqldb::entry::new(PLAYER_SETTING_TABLE)
        .await
        .expect("player setting table failed");
}

#[macro_export]
macro_rules! db_add {
    ($table:expr, $ty:ident) => {
        fn db_add(ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>, entry: $ty) {
            tokio::spawn(async move {
                let data = serde_json::to_string(&entry).expect("Not implement `Serialize` trait");
                if let Err(e) = sqldb::entry::insert($table, entry.id.as_str(), &data).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("insert entry failed")),
                    );
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_update {
    ($table:expr, $ty:ident) => {
        fn db_update(ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>, entry: $ty) {
            tokio::spawn(async move {
                let data = serde_json::to_string(&entry).expect("Not implement `Serialize` trait");
                if let Err(e) = sqldb::entry::update($table, entry.id.as_str(), &data).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
                    );
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_select_all {
    ($table:expr, $ty:ident) => {{
        match sqldb::entry::select_all($table).await {
            Ok(items) => items
                .into_iter()
                .filter_map(|item| serde_json::from_str::<$ty>(&item.data).ok())
                .collect(),
            Err(e) => {
                log::warn!("{:?}", e);
                vec![]
            }
        }
    }};
}

#[macro_export]
macro_rules! db_remove {
    ($table:expr) => {
        fn db_remove(
            ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>,
            id: impl ToString,
        ) {
            let id = id.to_string();
            tokio::spawn(async move {
                if let Err(e) = sqldb::entry::delete($table, id.as_str()).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("remove entry failed")),
                    );
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_remove_all {
    ($table:expr) => {
        fn db_remove_all(ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>) {
            tokio::spawn(async move {
                if let Err(e) = sqldb::entry::delete_all($table).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("remove all entry failed")),
                    );
                }
            });
        }
    };
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIHistoryEntry")]
pub struct HistoryEntry {
    pub id: String,
    pub file: String,
    pub size: String,
    pub duration: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingPlayer")]
pub struct SettingPlayer {
    pub id: String,
    pub current_time: String,
    pub end_time: String,
    pub sound: i32,
}

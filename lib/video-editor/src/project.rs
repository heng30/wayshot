pub mod autosave;
pub mod filters;
pub mod metadata;
pub mod project;
pub mod recent;

pub use autosave::{
    AutoSaveConfig, AutoSaveHandle, AutoSaveManager, RecoveryInfo, check_for_recovery,
    check_recovery_on_startup, cleanup_recovery_file, get_all_recovery_files,
    restore_from_recovery,
};
pub use project::{
    BookmarkData, ChapterSummaryData, ManagerData, ProjectFile, ProjectPreviewConfig, load_project,
    save_project,
};

// 当前项目文件格式版本
pub const CURRENT_PROJECT_VERSION: u32 = 1;

// UI 状态数据库 ID (在 wayshot 中使用)
pub const UI_STATE_ID: &str = "video_editor_ui_state";

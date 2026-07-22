use std::{
    cell::UnsafeCell,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::oneshot;
use video_editor::{
    Result as VeResult,
    commands::{AffectedSegments, Command, HistoryManager},
    tracks::manager::Manager,
};

static UI_AVAILABLE: AtomicBool = AtomicBool::new(false);
static STATE_ACCESSORS: OnceLock<StateAccessors> = OnceLock::new();
static DISPATCH_ACTION: OnceLock<DispatchActionFn> = OnceLock::new();
static SYNC_UI_CALLBACK: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

type GetProjectPathFn = fn() -> Option<String>;
type GetTracksManagerFn = fn() -> Option<Manager>;
type IsUnsavedFn = fn() -> bool;
type CanUndoFn = fn() -> bool;
type CanRedoFn = fn() -> bool;
type WithHistoryManagerFn = fn(&mut dyn FnMut(&mut Manager, &mut HistoryManager));
type DispatchActionFn = Arc<dyn Fn(UiAction) + Send + Sync>;

pub struct StateAccessors {
    pub with_history_manager: WithHistoryManagerFn,
    pub get_project_path: GetProjectPathFn,
    pub get_tracks_manager: GetTracksManagerFn,
    pub is_unsaved: IsUnsavedFn,
    pub can_undo: CanUndoFn,
    pub can_redo: CanRedoFn,
}

/// Actions that the MCP server can dispatch to the UI thread.
/// Each action maps to a Logic callback in logic.slint.
#[derive(Debug)]
pub enum UiAction {
    // Project
    NewProject,
    OpenProject,
    OpenProjectPath {
        path: String,
    },
    CreateProject {
        name: String,
        dir_path: String,
    },
    CloseProject,
    SaveProject,
    Undo,
    Redo,

    // Tracks
    AddTrack {
        track_type: i32,
    }, // 0=Video, 1=Audio, 2=Subtitle, 3=Image, 4=Text
    AddEmptyVideoTrack,
    AddEmptyAudioTrack,
    AddEmptySubtitleTrack,
    AddEmptyImageTrack,
    AddEmptyTextTrack,
    AddSelectedTrack {
        index: usize,
    },
    RemoveTracks,
    InsertVideoTrack {
        index: usize,
    },
    InsertAudioTrack {
        index: usize,
    },
    InsertSubtitleTrack {
        index: usize,
    },
    InsertImageTrack {
        index: usize,
    },
    InsertTextTrack {
        index: usize,
    },
    ToggleLockedTrack {
        index: usize,
    },
    ToggleHidingTrack {
        index: usize,
    },
    ToggleMutedTrack {
        index: usize,
    },
    TrackMoveUp {
        index: usize,
    },
    TrackMoveDown {
        index: usize,
    },
    MoveTrackByDrag {
        from_index: usize,
        to_index: usize,
    },

    // Segments
    AddSelectedSegment {
        track_index: usize,
        segment_index: usize,
    },
    SplitSegment,
    RemoveSegments,
    ToggleSegmentEnable {
        track_index: usize,
        segment_index: usize,
    },
    ToggleSegmentAudio {
        track_index: usize,
        segment_index: usize,
    },
    CommitSegmentMove {
        track_index: usize,
        segment_index: usize,
        final_offset_ms: i32,
    },
    SegmentRemoveGap {
        track_index: usize,
        segment_index: usize,
    },
    SegmentRemoveLeftGap {
        track_index: usize,
        segment_index: usize,
    },
    SegmentRemoveRightGap {
        track_index: usize,
        segment_index: usize,
    },

    // Filters
    RemoveAllFiltersFromSegment {
        track_index: usize,
        segment_index: usize,
    },
    RemoveAllFiltersFromTrack {
        track_index: usize,
    },

    // Media import (opens file picker dialogs)
    ImportToPlaylist,
    ImportToLibrary,

    // Add items from playlist/library to track (by index)
    PlaylistItemAddToTrack {
        index: usize,
    },
    PlaylistItemAddToTrackEnd {
        index: usize,
    },
    LibraryItemAddToTrack {
        index: usize,
    },
    LibraryItemAddToTrackEnd {
        index: usize,
    },

    // Preview
    PreviewPlay,
    PreviewStop,
    PreviewSeek {
        position_ms: i32,
    },
    TimelineSeek {
        position_ms: i32,
    },

    // Export
    ExportVideo,
    ExportAudio,
    ExportSubtitle,

    // Audio recording
    StartRecordingAudio,
    StopRecordingAudio,
}

pub fn set_ui_available(available: bool) {
    UI_AVAILABLE.store(available, Ordering::Relaxed);
}

pub fn is_ui_available() -> bool {
    UI_AVAILABLE.load(Ordering::Relaxed)
}

/// Register the dispatch handler that maps UiAction → Logic callbacks.
/// This must be called from the wayshot app during init.
pub fn register_dispatch_action(cb: DispatchActionFn) {
    if DISPATCH_ACTION.set(cb).is_err() {
        panic!("register_dispatch_action called more than once");
    }
}

pub fn register_state_accessors(accessors: StateAccessors) {
    if STATE_ACCESSORS.set(accessors).is_err() {
        panic!("register_state_accessors called more than once");
    }
}

fn with_accessors<F, R>(f: F) -> R
where
    F: FnOnce(&StateAccessors) -> R,
{
    f(STATE_ACCESSORS
        .get()
        .expect("MCP state accessors not registered"))
}

pub fn get_project_path() -> Option<String> {
    with_accessors(|a| (a.get_project_path)())
}

pub fn get_tracks_manager() -> Option<Manager> {
    with_accessors(|a| (a.get_tracks_manager)())
}

pub fn is_unsaved() -> bool {
    with_accessors(|a| (a.is_unsaved)())
}

pub fn can_undo() -> bool {
    with_accessors(|a| (a.can_undo)())
}

pub fn can_redo() -> bool {
    with_accessors(|a| (a.can_redo)())
}

/// Dispatch an action to the UI thread.
/// This sends the action to the registered handler which invokes
/// the appropriate Logic callback on the Slint event loop.
pub fn dispatch_action(action: UiAction) {
    if is_ui_available()
        && let Some(cb) = DISPATCH_ACTION.get()
    {
        cb(action);
    }
}

/// Dispatch an action to the UI thread and wait for a result via oneshot channel.
/// The sender side must be provided by the caller; the UI thread handler
/// will send the result back through it.
pub fn dispatch_action_with_result<T: Send + 'static>(
    action_factory: impl FnOnce(oneshot::Sender<T>) -> UiAction,
) -> Result<T, String> {
    let (tx, rx) = oneshot::channel();
    let action = action_factory(tx);
    dispatch_action(action);
    rx.blocking_recv()
        .map_err(|_| "UI thread dropped the result channel".to_string())
}

/// Register the sync_ui callback that refreshes the Slint UI from the main thread.
pub fn register_sync_ui_callback(callback: Arc<dyn Fn() + Send + Sync>) {
    if SYNC_UI_CALLBACK.set(callback).is_err() {
        panic!("register_sync_ui_callback called more than once");
    }
}

/// Sync the TracksManager state to the Slint UI.
/// This calls the registered callback which schedules `sync_manager_to_ui`
/// on the Slint event loop.
pub fn sync_ui() {
    if is_ui_available()
        && let Some(cb) = SYNC_UI_CALLBACK.get()
    {
        cb();
    }
}

/// Execute a closure with mutable access to both Manager and HistoryManager.
/// This is the primary way to perform mutations through the command system.
pub fn with_history_manager<F, R>(f: F) -> R
where
    F: FnOnce(&mut Manager, &mut HistoryManager) -> R,
{
    struct Wrapper<F, R> {
        f: Option<F>,
        result: Option<R>,
    }

    let wrapper = UnsafeCell::new(Wrapper {
        f: Some(f),
        result: None,
    });

    let mut callback = |manager: &mut Manager, hm: &mut HistoryManager| {
        // SAFETY: This callback is only called once by the accessor.
        let w = unsafe { &mut *wrapper.get() };
        if let Some(f) = w.f.take() {
            w.result = Some(f(manager, hm));
        }
    };

    with_accessors(|a| (a.with_history_manager)(&mut callback));

    wrapper.into_inner().result.unwrap()
}

/// Execute a command through the HistoryManager
pub fn execute_command(command: Box<dyn Command>) -> VeResult<AffectedSegments> {
    with_history_manager(|manager, hm| {
        let result = hm.execute(manager, command)?;
        Ok(result.affected_segments)
    })
}

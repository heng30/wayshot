pub mod ai;
pub mod audio;
pub mod export;
pub mod filter;
pub mod font;
pub mod gfilter;
pub mod image;
pub mod library;
pub mod ocr;
pub mod playlist;
pub mod preview;
pub mod project;
pub mod segment;
pub mod subtitle;
pub mod track;
pub mod transcribe;

use crate::VideoEditorServer;
use rmcp::handler::server::router::tool::ToolRouter;

/// Build the complete tool router with all registered tools
pub fn build_tool_router() -> ToolRouter<VideoEditorServer> {
    let mut router = ToolRouter::new();

    // Project tools
    router = router
        .with_async_tool::<project::ProjectStatusTool>()
        .with_async_tool::<project::ProjectCreateTool>()
        .with_async_tool::<project::ProjectOpenTool>()
        .with_async_tool::<project::ProjectCloseTool>()
        .with_async_tool::<project::ProjectUndoTool>()
        .with_async_tool::<project::ProjectRedoTool>();

    // Track tools
    router = router
        .with_async_tool::<track::TrackListTool>()
        .with_async_tool::<track::TrackAddTool>()
        .with_async_tool::<track::TrackInsertTool>()
        .with_async_tool::<track::TrackRemoveTool>()
        .with_async_tool::<track::TrackMoveTool>()
        .with_async_tool::<track::TrackToggleLockedTool>()
        .with_async_tool::<track::TrackToggleHiddenTool>()
        .with_async_tool::<track::TrackToggleMutedTool>();

    // Segment tools
    router = router
        .with_async_tool::<segment::SegmentListTool>()
        .with_async_tool::<segment::SegmentSplitTool>()
        .with_async_tool::<segment::SegmentMoveTool>()
        .with_async_tool::<segment::SegmentDeleteTool>()
        .with_async_tool::<segment::SegmentToggleVisibleTool>()
        .with_async_tool::<segment::SegmentToggleAudioTool>()
        .with_async_tool::<segment::SegmentRemoveGapTool>()
        .with_async_tool::<segment::SegmentMetadataTool>()
        .with_async_tool::<segment::SegmentAddTool>()
        .with_async_tool::<segment::SegmentResizeTool>()
        .with_async_tool::<segment::SegmentShrinkTool>()
        .with_async_tool::<segment::SegmentStretchTool>()
        .with_async_tool::<segment::SegmentDeleteCmdTool>()
        .with_async_tool::<segment::SegmentMoveCmdTool>()
        .with_async_tool::<segment::SegmentCopyTool>();

    // Filter tools
    router = router
        .with_async_tool::<filter::FilterListSegmentTool>()
        .with_async_tool::<filter::FilterRemoveTool>()
        .with_async_tool::<filter::FilterToggleTool>()
        .with_async_tool::<filter::FilterClearTool>()
        .with_async_tool::<filter::FilterAddTool>();

    // Preview tools
    router = router
        .with_async_tool::<preview::PreviewSeekTool>()
        .with_async_tool::<preview::PreviewInfoTool>();

    // Playlist tools
    router = router
        .with_async_tool::<playlist::PlaylistListTool>()
        .with_async_tool::<playlist::PlaylistImportTool>()
        .with_async_tool::<playlist::PlaylistAddToTrackTool>();

    // Library tools
    router = router
        .with_async_tool::<library::LibraryListTool>()
        .with_async_tool::<library::LibraryImportTool>()
        .with_async_tool::<library::LibraryAddToTrackTool>();

    // Export tools
    router = router
        .with_async_tool::<export::ExportVideoTool>()
        .with_async_tool::<export::ExportAudioTool>()
        .with_async_tool::<export::ExportSubtitleTool>()
        .with_async_tool::<export::ExportCancelTool>()
        .with_async_tool::<export::ExportQueueTool>();

    // Subtitle tools
    router = router
        .with_async_tool::<subtitle::SubtitleAddTool>()
        .with_async_tool::<subtitle::SubtitleUpdateTool>()
        .with_async_tool::<subtitle::SubtitleTranslateTool>()
        .with_async_tool::<subtitle::SubtitleTranslateCancelTool>();

    // Transcription tools
    router = router
        .with_async_tool::<transcribe::TranscribeStartTool>()
        .with_async_tool::<transcribe::TranscribeCancelTool>();

    // OCR tools
    router = router.with_async_tool::<ocr::OcrProcessImageTool>();

    // AI tools
    router = router
        .with_async_tool::<ai::AiBgRemoverProcessTool>()
        .with_async_tool::<ai::AiSmartClipStartTool>()
        .with_async_tool::<ai::AiSceneDetectTool>()
        .with_async_tool::<ai::AiDewatermarkProcessTool>()
        .with_async_tool::<ai::AiCutoutProcessTool>()
        .with_async_tool::<ai::AiChapterSummaryTool>()
        .with_async_tool::<ai::AiSpeakersProcessTool>();

    // Audio tools
    router = router
        .with_async_tool::<audio::AudioRecordStartTool>()
        .with_async_tool::<audio::AudioRecordStopTool>()
        .with_async_tool::<audio::AudioStemSplitTool>()
        .with_async_tool::<audio::AudioTtsGenerateTool>()
        .with_async_tool::<audio::AudioVadDetectTool>();

    // Image tools
    router = router
        .with_async_tool::<image::ImgCodeGenerateTool>()
        .with_async_tool::<image::ImgPureColorGenerateTool>()
        .with_async_tool::<image::ImgLongScreenshotTool>()
        .with_async_tool::<image::ImgAnimationPreviewTool>()
        .with_async_tool::<image::ImgBgAnimationTool>();

    // Font tools
    router = router
        .with_async_tool::<font::FontListTool>()
        .with_async_tool::<font::FontImportTool>()
        .with_async_tool::<font::FontSearchTool>();

    router
}

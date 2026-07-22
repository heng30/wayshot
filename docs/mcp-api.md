# Wayshot Video Editor — MCP API Reference

> Model Context Protocol (MCP) server for the Wayshot Video Editor, enabling external AI agents to programmatically control the editor.

## Overview

The Wayshot MCP server exposes all video editing capabilities as MCP tools, organized into **16 categories** with **50+ tools**. The server supports both **stdio** and **Streamable HTTP** transports.

### Connection

| Transport | Default | Configuration |
|-----------|---------|---------------|
| **stdio** | Standard input/output | `"transport": "stdio"` |
| **HTTP** | `http://localhost:9527/mcp` | `"transport": "http"`, `"port": 9527` |
| **Both** | stdio + HTTP simultaneously | `"transport": "both"` |

### MCP Configuration

MCP configuration is stored in the video editor's database:

```json
{
  "enabled": true,
  "transport": "both",
  "port": 9527
}
```

---

## Common Types

### ProjectStatus

```json
{
  "is_open": true,
  "project_path": "/path/to/project.vep",
  "is_unsaved": false,
  "track_count": 3,
  "total_segments": 12,
  "duration_ms": 65000,
  "can_undo": true,
  "can_redo": false
}
```

### TrackInfo

```json
{
  "index": 0,
  "name": "Video 1",
  "track_type": "video",
  "locked": false,
  "hidden": false,
  "muted": false,
  "segment_count": 5,
  "duration_ms": 30000
}
```

### SegmentInfo

```json
{
  "index": 0,
  "timeline_offset_ms": 0,
  "duration_ms": 10000,
  "source_offset_ms": 0,
  "original_duration_ms": 10000,
  "visible": true,
  "audio_muted": false,
  "playback_speed": 1.0,
  "source_path": "/path/to/video.mp4"
}
```

### FilterInfo

```json
{
  "index": 0,
  "filter_type": "brightness",
  "name": "Brightness",
  "enabled": true,
  "detail": "{\"brightness\": 0.2}"
}
```

### TaskInfo

```json
{
  "task_id": "550e8400-e29b-41d4-a716-446655440000",
  "description": "Export video",
  "status": "running",
  "progress": 0.45
}
```

---

## 1. Project (`ve_project_`)

Project-level operations including undo/redo and status queries.

### `ve_project_status`

Get the current project status including path, track count, duration, and undo/redo availability.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `is_open` | bool | Whether a project is currently open |
| `project_path` | string? | Path to the project file |
| `is_unsaved` | bool | Whether there are unsaved changes |
| `track_count` | usize | Number of tracks |
| `total_segments` | usize | Total segments across all tracks |
| `duration_ms` | u64 | Total project duration in milliseconds |
| `can_undo` | bool | Whether undo is available |
| `can_redo` | bool | Whether redo is available |

---

### `ve_project_undo`

Undo the last operation in the video editor.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Description of the undone operation |

---

### `ve_project_redo`

Redo the last undone operation in the video editor.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Description of the redone operation |

---

## 2. Track (`ve_track_`)

Track management operations: add, remove, move, and toggle track properties.

### `ve_track_list`

List all tracks in the current project.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `tracks` | TrackInfo[] | Array of track information objects |

---

### `ve_track_add`

Add a new empty track to the project.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_type` | string | ✅ | Track type: `"video"`, `"audio"`, `"subtitle"`, `"image"`, `"text"` |
| `name` | string? | ❌ | Optional custom track name |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `track_index` | usize | Index of the newly created track |
| `track_name` | string | Name of the newly created track |

---

### `ve_track_insert`

Insert an empty track at a specific index.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_type` | string | ✅ | Track type: `"video"`, `"audio"`, `"subtitle"`, `"image"`, `"text"` |
| `index` | usize | ✅ | Position to insert the track |
| `name` | string? | ❌ | Optional custom track name |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `actual_index` | usize | The actual index where the track was inserted |

---

### `ve_track_remove`

Remove a track by index.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track to remove |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether the removal succeeded |

---

### `ve_track_move`

Move a track from one index to another.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `from_index` | usize | ✅ | Current track index |
| `to_index` | usize | ✅ | Target track index |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether the move succeeded |

---

### `ve_track_toggle_locked`

Toggle track lock state.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `is_locked` | bool | New lock state |

---

### `ve_track_toggle_hidden`

Toggle track visibility.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `is_hidden` | bool | New visibility state |

---

### `ve_track_toggle_muted`

Toggle track audio mute state.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `is_muted` | bool | New mute state |

---

## 3. Segment (`ve_segment_`)

Segment-level operations within tracks: split, move, delete, toggle visibility/audio, and metadata queries.

### `ve_segment_list`

List all segments in a track.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `segments` | SegmentInfo[] | Array of segment information objects |

---

### `ve_segment_split`

Split a segment at the given position (milliseconds).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment to split |
| `position_ms` | u64 | ✅ | Position in milliseconds from segment start |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether the split succeeded |

---

### `ve_segment_move`

Move a segment to a new timeline offset (milliseconds).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |
| `offset_ms` | u64 | ✅ | New timeline offset in milliseconds |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether the move succeeded |

---

### `ve_segment_delete`

Delete a segment from a track.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment to delete |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether the deletion succeeded |

---

### `ve_segment_toggle_visible`

Toggle segment visibility on/off.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `is_visible` | bool | New visibility state |

---

### `ve_segment_toggle_audio`

Toggle segment audio mute on/off.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `is_muted` | bool | New audio mute state |

---

### `ve_segment_remove_gap`

Remove gap before (left) or after (right) a segment.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |
| `direction` | string | ✅ | `"left"` or `"right"` |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Whether the gap removal succeeded |

---

### `ve_segment_metadata`

Get metadata for a segment including source file, resolution, and audio info.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `metadata` | object | Segment metadata as a JSON object |

---

## 4. Filter (`ve_filter_`)

Filter management for segments: list, remove, toggle, and clear filters.

### `ve_filter_list_segment`

List all filters on a segment.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Filter list as JSON (array of FilterInfo) |

---

### `ve_filter_remove`

Remove a filter from a segment.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |
| `filter_type` | string | ✅ | Type of filter (e.g., `"video"`, `"audio"`) |
| `filter_index` | usize | ✅ | Index of the filter within its type list |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_filter_toggle`

Toggle a filter enabled/disabled.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |
| `filter_type` | string | ✅ | Type of filter |
| `filter_index` | usize | ✅ | Index of the filter |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"enabled": true/false}` |

---

### `ve_filter_clear`

Clear all filters of a given type from a segment.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |
| `filter_type` | string | ✅ | Type of filters to clear |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

## 5. Global Filter (`ve_gfilter_`)

> 🚧 **Planned** — Global filter tools will be implemented with the wayshot bridge for progress bar, timer, speed, rotation, and danmaku configuration.

---

## 6. Preview (`ve_preview_`)

Preview playback control: seek and info queries.

### `ve_preview_seek`

Seek the preview to a position (milliseconds).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `position_ms` | u64 | ✅ | Target position in milliseconds |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_preview_info`

Get the current preview info including duration and track count.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Preview information as JSON |

---

## 7. Playlist (`ve_playlist_`)

Playlist management: list, import media files.

### `ve_playlist_list`

List all items in the playlist.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Playlist items as JSON |

---

### `ve_playlist_import`

Import a media file to the playlist.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | ✅ | Path to the media file to import |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

## 8. Library (`ve_library_`)

Media library management: list, import, and add to track.

### `ve_library_list`

List all items in the media library.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Library items as JSON |

---

### `ve_library_import`

Import a media file to the library.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | ✅ | Path to the media file to import |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_library_add_to_track`

Add a media file from the library to a track.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | ✅ | Path to the media file |
| `track_index` | usize | ✅ | Target track index |
| `at_end` | bool? | ❌ | Add at end of track (default: true) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

## 9. Export (`ve_export_`)

Export operations for video, audio, and subtitles. Long-running exports return a task ID.

### `ve_export_video`

Export the project as a video file (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `output_path` | string | ✅ | Destination file path |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info including `task_id` and `status` |

---

### `ve_export_audio`

Export the project audio as a file (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `output_path` | string | ✅ | Destination file path |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info including `task_id` and `status` |

---

### `ve_export_subtitle`

Export subtitles to a file in the specified format.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `output_path` | string | ✅ | Destination file path |
| `format` | string | ✅ | Subtitle format (e.g., `"srt"`, `"ass"`, `"vtt"`) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Export result as JSON |

---

### `ve_export_cancel`

Cancel a running export task.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `task_id` | string | ✅ | ID of the task to cancel |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_export_queue`

List all pending and active export tasks.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Array of TaskInfo objects |

---

## 10. Subtitle (`ve_subtitle_`)

Subtitle editing and translation operations.

### `ve_subtitle_add`

Add a subtitle entry to a track.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the subtitle track |
| `text` | string | ✅ | Subtitle text content |
| `start_ms` | u64 | ✅ | Start time in milliseconds |
| `end_ms` | u64 | ✅ | End time in milliseconds |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_subtitle_update`

Update the text of an existing subtitle entry.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the subtitle track |
| `index` | usize | ✅ | Index of the subtitle entry |
| `text` | string | ✅ | New subtitle text |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_subtitle_translate`

Start subtitle translation from source to target language (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source_language` | string | ✅ | Source language code (e.g., `"en"`, `"zh"`) |
| `target_language` | string | ✅ | Target language code |
| `prompt` | string? | ❌ | Optional translation prompt/instruction |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_subtitle_translate_cancel`

Cancel a running subtitle translation task.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

## 11. Transcription (`ve_transcribe_`)

Audio transcription and subtitle management.

### `ve_transcribe_start`

Start audio transcription (starts async task).

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_transcribe_cancel`

Cancel a running transcription task.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

## 12. OCR (`ve_ocr_`)

Optical Character Recognition for images.

### `ve_ocr_process_image`

Run OCR on an image to extract text.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `image_path` | string | ✅ | Path to the image file |
| `task_mode` | string? | ❌ | Optional OCR task mode |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | OCR results including extracted text |

---

## 13. AI/ML (`ve_ai_`)

AI-powered features: background removal, smart clip, scene detection, watermark removal, cutout, chapter summary, and speaker diarization. All start async tasks.

### `ve_ai_bg_remover_process`

Remove background from an image using AI (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `image_path` | string | ✅ | Path to the image file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_ai_smart_clip_start`

Start AI smart clip detection (starts async task).

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_ai_scene_detect`

Detect scene changes in a segment using AI (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |
| `algorithm` | string | ✅ | Detection algorithm name |
| `threshold` | f32? | ❌ | Optional detection threshold |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_ai_dewatermark_process`

Remove watermark from an image using AI (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `image_path` | string | ✅ | Path to the image file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_ai_cutout_process`

AI cutout from an image (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `image_path` | string | ✅ | Path to the image file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_ai_chapter_summary`

Generate chapter summary using AI (starts async task).

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_ai_speakers_process`

Speaker diarization on an audio file using AI (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `audio_path` | string | ✅ | Path to the audio file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

## 14. Audio (`ve_audio_`)

Audio operations: recording, stem splitting, TTS, and voice activity detection.

### `ve_audio_record_start`

Start audio recording to a directory.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `save_dir` | string? | ❌ | Directory to save the recording |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"status": "started"}` |

---

### `ve_audio_record_stop`

Stop audio recording.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Recording result with file path |

---

### `ve_audio_stem_split`

Split audio into stems (vocals, drums, etc.) using AI (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `audio_path` | string | ✅ | Path to the audio file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_audio_tts_generate`

Generate speech from text using TTS (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | string | ✅ | Text to synthesize |
| `index` | usize? | ❌ | Optional TTS voice index |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_audio_vad_detect`

Detect voice activity segments in an audio file.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `audio_path` | string | ✅ | Path to the audio file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Detected voice segments |

---

## 15. Image (`ve_img_`)

Image generation tools: code images, solid color, long screenshots, and animations.

### `ve_img_code_generate`

Generate a code syntax-highlighted image.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `code` | string | ✅ | Source code text |
| `language` | string | ✅ | Programming language for syntax highlighting |
| `theme` | string? | ❌ | Optional color theme |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Generation result/status |

---

### `ve_img_pure_color_generate`

Generate a solid color image with given dimensions.

**Parameters:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `r` | u8 | ✅ | 0 | Red channel |
| `g` | u8 | ✅ | 0 | Green channel |
| `b` | u8 | ✅ | 0 | Blue channel |
| `a` | u8 | ✅ | 255 | Alpha channel |
| `width` | u32 | ✅ | 1920 | Image width in pixels |
| `height` | u32 | ✅ | 1080 | Image height in pixels |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Generation result/status |

---

### `ve_img_long_screenshot`

Create a long screenshot from a segment (starts async task).

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `track_index` | usize | ✅ | Index of the track |
| `segment_index` | usize | ✅ | Index of the segment |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Task info with `task_id` and `status` |

---

### `ve_img_animation_preview`

Start image animation preview.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `image_path` | string | ✅ | Path to the image file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Animation preview status |

---

### `ve_img_bg_animation`

Start background animation.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Animation status |

---

## 16. Font (`ve_font_`)

Font management: list, import, and search.

### `ve_font_list`

List all available fonts.

**Parameters:** _(none)_

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Available fonts list |

---

### `ve_font_import`

Import a font file.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_path` | string | ✅ | Path to the font file |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | `{"success": true}` |

---

### `ve_font_search`

Search fonts by keyword.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `keyword` | string | ✅ | Search keyword |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `result` | object | Matching fonts list |

---

## Error Handling

All tools return `ErrorData` on failure with the following structure:

| Field | Type | Description |
|-------|------|-------------|
| `code` | i32 | Error code |
| `message` | string | Human-readable error message |

### Common Error Codes

| Code | Description |
|------|-------------|
| -1 | Project not open |
| -2 | Invalid track index |
| -3 | Invalid segment index |
| -4 | Command execution failed |
| -5 | MCP state not initialized |

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│                 MCP Client (AI Agent)            │
└─────────────────────┬───────────────────────────┘
                      │ MCP Protocol (stdio / HTTP)
┌─────────────────────┴───────────────────────────┐
│              mcp-server crate                    │
│  ┌───────────┐  ┌──────────┐  ┌──────────────┐  │
│  │ Transport  │  │  Server   │  │  ToolRouter  │  │
│  │ (stdio/   │  │  Handler  │  │  (50+ tools) │  │
│  │  HTTP)    │  │           │  │              │  │
│  └───────────┘  └────┬──────┘  └──────┬───────┘  │
│                      │                │           │
│              ┌───────┴────────────────┴───────┐  │
│              │          Service Layer          │  │
│              │  (project, track, segment,      │  │
│              │   filter, preview, media, ...)  │  │
│              └───────────────┬────────────────┘  │
└──────────────────────────────┼──────────────────┘
                               │ State Accessors (fn pointers)
┌──────────────────────────────┼──────────────────┐
│              wayshot (video editor)             │
│  ┌───────────┐  ┌──────────────┐  ┌──────────┐ │
│  │PROJECT_   │  │HistoryManager│  │  Manager  │ │
│  │STATE      │  │(undo/redo)   │  │(tracks/   │ │
│  │           │  │              │  │ segments) │ │
│  └───────────┘  └──────────────┘  └──────────┘ │
└─────────────────────────────────────────────────┘
```

### State Access Bridge

The `mcp-server` crate cannot depend on `wayshot` directly (circular dependency). Instead, wayshot registers function pointers at startup via `register_state_accessors()`, allowing the MCP service layer to access project state without direct coupling.

### Command System

All mutations go through `HistoryManager::execute(manager, command)` with full undo/redo support. The MCP server uses `state::execute_command()` which wraps this flow and triggers UI sync when available.

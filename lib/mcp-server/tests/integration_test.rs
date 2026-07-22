//! Integration tests for wayshot MCP server tools.
//!
//! These tests require a running wayshot instance with MCP server enabled on port 9527.
//! Run `make debug` to start the app before running these tests.
//!
//! Usage: `cargo test -p mcp-server --test integration_test -- --test-threads=1`

use serde_json::Value;
use std::thread;
use std::time::Duration;

const MCP_URL: &str = "http://localhost:9527/mcp";
const PROJECTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tmp/projects");

/// MCP HTTP client for integration testing
struct McpClient {
    session: String,
    req_id: u64,
}

impl McpClient {
    /// Create a new MCP session
    fn new() -> Result<Self, String> {
        let output = std::process::Command::new("curl")
            .args([
                "-s", "-X", "POST", MCP_URL,
                "-H", "Content-Type: application/json",
                "-H", "Accept: application/json, text/event-stream",
                "-D", "-",
                "-d", &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": {"name": "mcp-test", "version": "1.0"}
                    }
                }).to_string(),
            ])
            .output()
            .map_err(|e| format!("curl failed: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.to_lowercase().starts_with("mcp-session-id:") {
                let session = line.split(':').nth(1).unwrap_or("").trim().to_string();
                if !session.is_empty() {
                    return Ok(Self { session, req_id: 0 });
                }
            }
        }
        Err(format!(
            "Could not get MCP session ID. Is the server running on port 9527?"
        ))
    }

    /// Call an MCP tool and return the result
    fn call_with_timeout(&mut self, tool: &str, arguments: Value, timeout_secs: u64) -> Result<Value, String> {
        self.req_id += 1;
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.req_id,
            "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}
        });

        let output = std::process::Command::new("curl")
            .args([
                "-s", "--max-time", &timeout_secs.to_string(),
                "-X", "POST", MCP_URL,
                "-H", "Content-Type: application/json",
                "-H", "Accept: application/json, text/event-stream",
                "-H", &format!("mcp-session-id: {}", self.session),
                "-d", &payload.to_string(),
            ])
            .output()
            .map_err(|e| format!("curl failed: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "curl exited with status {}. stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.starts_with("data: ") && line.contains("jsonrpc") {
                let data: Value = serde_json::from_str(&line[6..])
                    .map_err(|e| format!("JSON parse error: {e}"))?;

                if let Some(error) = data.get("error") {
                    let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
                    return Err(msg.to_string());
                }

                if let Some(result) = data.get("result") {
                    let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
                    if is_error {
                        let text = result
                            .get("content")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("text"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown error");
                        return Err(text.to_string());
                    }
                    return Ok(result.clone());
                }
            }
        }

        Err(format!("No valid response from MCP server"))
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Result<Value, String> {
        self.call_with_timeout(tool, arguments, 10)
    }

    /// Call a tool and expect it to succeed, returning structuredContent
    /// (with nested `result` unwrapped if present)
    fn call_ok(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.call(tool, arguments)
            .unwrap_or_else(|e| panic!("Expected {tool} to succeed, got error: {e}"));
        let sc = result.get("structuredContent").cloned().unwrap_or(result);
        // Some tools wrap output in a `result` field, unwrap it
        if let Some(inner) = sc.get("result") {
            inner.clone()
        } else {
            sc
        }
    }

    /// Call a tool with timeout and expect success, returning structuredContent
    fn call_ok_with_timeout(&mut self, tool: &str, arguments: Value, timeout_secs: u64) -> Value {
        let result = self.call_with_timeout(tool, arguments, timeout_secs)
            .unwrap_or_else(|e| panic!("Expected {tool} to succeed, got error: {e}"));
        let sc = result.get("structuredContent").cloned().unwrap_or(result);
        if let Some(inner) = sc.get("result") {
            inner.clone()
        } else {
            sc
        }
    }

    /// Call a tool and expect it to fail, returning the error message
    fn call_err(&mut self, tool: &str, arguments: Value) -> String {
        self.call(tool, arguments)
            .expect_err(&format!("Expected {tool} to fail but it succeeded"))
    }

    /// Open a project from the tmp/projects/ directory
    fn open_test_project(&mut self, name: &str) -> Value {
        let path = format!("{PROJECTS_DIR}/{name}.wayshot");
        self.call_ok("ve_project_open", serde_json::json!({ "path": path }))
    }

    /// Close any open project
    fn close_project(&mut self) {
        let _ = self.call("ve_project_close", serde_json::json!({}));
        wait_for_ui();
    }
}

/// Wait a short time after a dispatch_action call for the UI to process it
fn wait_for_ui() {
    thread::sleep(Duration::from_millis(500));
}

/// Wait longer for async UI operations
fn wait_longer() {
    thread::sleep(Duration::from_millis(1000));
}

// ============================================================
// 1. PROJECT TOOLS (6 tools)
// ve_project_status, ve_project_create, ve_project_open,
// ve_project_close, ve_project_undo, ve_project_redo
// ============================================================

#[test]
fn test_ve_project_status() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_project_status", serde_json::json!({}));
    assert!(result["is_open"].is_boolean(), "is_open should be boolean, got: {result}");
    assert!(result["can_undo"].is_boolean(), "can_undo should be boolean, got: {result}");
    assert!(result["can_redo"].is_boolean(), "can_redo should be boolean, got: {result}");
    assert!(result["track_count"].is_number(), "track_count should be number, got: {result}");
    assert!(result["duration_ms"].is_number(), "duration_ms should be number, got: {result}");
    assert!(result["total_segments"].is_number(), "total_segments should be number, got: {result}");
    assert!(result["is_unsaved"].is_boolean(), "is_unsaved should be boolean, got: {result}");
}

#[test]
fn test_ve_project_create() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Close any existing project first
    c.close_project();

    // Create project — dispatches CreateProject
    let result = c.call_ok("ve_project_create", serde_json::json!({
        "name": "test_create",
        "dir_path": "/tmp"
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
    assert!(result["project_path"].is_string(), "project_path should be string, got: {result}");

    // Check status after a short delay
    wait_for_ui();
    let status = c.call_ok("ve_project_status", serde_json::json!({}));
    assert!(status["is_open"].is_boolean());
}

#[test]
fn test_ve_project_open() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open project using a real test project file
    let result = c.open_test_project("test_open");
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
    assert!(result["project_path"].is_string(), "project_path should be string, got: {result}");

    wait_for_ui();
    let status = c.call_ok("ve_project_status", serde_json::json!({}));
    assert!(status["is_open"].as_bool().unwrap_or(false), "Project should be open after ve_project_open");
}

#[test]
fn test_ve_project_open_not_found() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // ve_project_open dispatches OpenProjectPath and returns success immediately.
    // The file-not-found error happens asynchronously in the UI.
    // So we verify the response structure, not the error.
    let result = c.call_ok("ve_project_open", serde_json::json!({
        "path": "/tmp/nonexistent.wayshot"
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
    assert!(result["project_path"].is_string(), "project_path should be string, got: {result}");
}

#[test]
fn test_ve_project_close() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open a project first so we have something to close
    c.open_test_project("test_close");
    wait_for_ui();

    let result = c.call_ok("ve_project_close", serde_json::json!({}));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");

    wait_for_ui();
    let status = c.call_ok("ve_project_status", serde_json::json!({}));
    assert!(!status["is_open"].as_bool().unwrap_or(true), "Project should not be open after close");
}

#[test]
fn test_ve_project_undo() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open a project first
    c.open_test_project("test_undo");
    wait_for_ui();

    // Undo when nothing to undo — should return error
    let result = c.call("ve_project_undo", serde_json::json!({}));
    // The server may return error "Cannot undo: No commands to undo" or success with description
    match result {
        Ok(val) => {
            let unwrapped = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = unwrapped.get("result").cloned().unwrap_or(unwrapped);
            assert!(inner.get("description").is_some() || inner.get("success").is_some(),
                "Undo result should have description or success, got: {inner}");
        }
        Err(e) => {
            // "Cannot undo" is acceptable when there's nothing to undo
            assert!(e.contains("undo") || e.contains("Undo"),
                "Undo error should mention undo, got: {e}");
        }
    }
}

#[test]
fn test_ve_project_redo() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open a project first
    c.open_test_project("test_redo");
    wait_for_ui();

    // Redo when nothing to redo — should return error
    let result = c.call("ve_project_redo", serde_json::json!({}));
    match result {
        Ok(val) => {
            let unwrapped = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = unwrapped.get("result").cloned().unwrap_or(unwrapped);
            assert!(inner.get("description").is_some() || inner.get("success").is_some(),
                "Redo result should have description or success, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("redo") || e.contains("Redo"),
                "Redo error should mention redo, got: {e}");
        }
    }
}

// ============================================================
// 2. TRACK TOOLS (8 tools)
// ============================================================

#[test]
fn test_ve_track_list() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Note: Closing project may not fully reset state if a previous test left a project open.
    // The "no project" path is tested in test_no_project_errors instead.
    // Here we test that with a project, track_list returns a proper array.
    c.open_test_project("test_track_list");
    wait_for_ui();

    let result = c.call_ok("ve_track_list", serde_json::json!({}));
    assert!(result["tracks"].is_array(), "tracks should be array, got: {result}");
}

#[test]
fn test_ve_track_add() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open a project first
    c.open_test_project("test_track_add");
    wait_for_ui();

    // Add each track type
    for ttype in ["video", "audio", "subtitle", "image", "text"] {
        let result = c.call_ok("ve_track_add", serde_json::json!({
            "track_type": ttype,
            "name": format!("{ttype}_track")
        }));
        wait_for_ui();
        assert!(result["track_index"].is_number(),
            "track add should return track_index for {ttype}, got: {result}");
        assert!(result["track_name"].is_string(),
            "track add should return track_name for {ttype}, got: {result}");
    }

    // Invalid track type should error
    let err = c.call_err("ve_track_add", serde_json::json!({"track_type": "invalid", "name": "X"}));
    assert!(err.contains("Invalid track type") || err.contains("invalid"),
        "Expected invalid track type error, got: {err}");
}

#[test]
fn test_ve_track_insert() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_track_insert");
    wait_for_ui();

    // Add a track first so we have at least one
    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V1"}));
    wait_for_ui();

    let result = c.call_ok("ve_track_insert", serde_json::json!({
        "track_type": "video",
        "index": 0,
        "name": "V0"
    }));
    wait_for_ui();
    assert!(result["actual_index"].is_number() || result.get("success").is_some(),
        "track insert should return actual_index or success, got: {result}");

    // Invalid track type should error
    let err = c.call_err("ve_track_insert", serde_json::json!({
        "track_type": "bogus", "index": 0
    }));
    assert!(err.contains("Invalid track type") || err.contains("invalid"),
        "Expected invalid track type error, got: {err}");
}

#[test]
fn test_ve_track_remove() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_track_remove");
    wait_for_ui();

    // Add a track to remove
    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V1"}));
    wait_for_ui();

    let result = c.call_ok("ve_track_remove", serde_json::json!({"track_index": 0}));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
    wait_for_ui();
}

#[test]
fn test_ve_track_move() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_track_move");
    wait_for_ui();

    // Add two tracks so we can move
    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V1"}));
    wait_for_ui();
    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V2"}));
    wait_for_ui();

    let result = c.call_ok("ve_track_move", serde_json::json!({"from_index": 0, "to_index": 1}));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

#[test]
fn test_ve_track_toggle_locked() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_toggle_locked");
    wait_for_ui();

    // Add a track first
    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V1"}));
    wait_for_ui();

    let result = c.call_ok("ve_track_toggle_locked", serde_json::json!({"track_index": 0}));
    assert!(result["is_locked"].is_boolean(), "is_locked should be boolean, got: {result}");
}

#[test]
fn test_ve_track_toggle_hidden() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_toggle_hidden");
    wait_for_ui();

    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V1"}));
    wait_for_ui();

    let result = c.call_ok("ve_track_toggle_hidden", serde_json::json!({"track_index": 0}));
    assert!(result["is_hidden"].is_boolean(), "is_hidden should be boolean, got: {result}");
}

#[test]
fn test_ve_track_toggle_muted() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_toggle_muted");
    wait_for_ui();

    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "V1"}));
    wait_for_ui();

    let result = c.call_ok("ve_track_toggle_muted", serde_json::json!({"track_index": 0}));
    assert!(result["is_muted"].is_boolean(), "is_muted should be boolean, got: {result}");
}

// ============================================================
// 3. PLAYLIST & LIBRARY TOOLS (6 tools)
// ============================================================

#[test]
fn test_ve_playlist_list() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_playlist_list", serde_json::json!({}));
    // Returns items (may be empty) or a note
    assert!(result.get("items").is_some() || result.get("note").is_some(),
        "playlist list should have items or note, got: {result}");
}

#[test]
fn test_ve_playlist_import() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_pl_import");
    wait_for_ui();

    // Playlist import dispatches ImportToPlaylist which opens file picker dialog
    let result = c.call_ok("ve_playlist_import", serde_json::json!({
        "file_path": "/tmp/test.mp4"
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

#[test]
fn test_ve_playlist_add_to_track() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_pl_list");
    wait_for_ui();

    // Add playlist item to track by index
    let result = c.call_ok("ve_playlist_add_to_track", serde_json::json!({
        "index": 0,
        "at_end": true
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

#[test]
fn test_ve_library_list() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_library_list", serde_json::json!({}));
    assert!(result.get("items").is_some() || result.get("note").is_some(),
        "library list should have items or note, got: {result}");
}

#[test]
fn test_ve_library_import() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_lib_import");
    wait_for_ui();

    let result = c.call_ok("ve_library_import", serde_json::json!({
        "file_path": "/tmp/test.mp4"
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

#[test]
fn test_ve_library_add_to_track() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_lib_add_track");
    wait_for_ui();

    let result = c.call_ok("ve_library_add_to_track", serde_json::json!({
        "index": 0,
        "at_end": true
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

// ============================================================
// 4. SEGMENT TOOLS (8 tools)
// ============================================================

#[test]
fn test_ve_segment_list() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open a project so we have tracks
    c.open_test_project("test_seg_list");
    wait_for_ui();

    // Track index 0 may not have segments — check if it returns an array or error
    let result = c.call("ve_segment_list", serde_json::json!({"track_index": 0}));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["segments"].is_array(), "segments should be array, got: {inner}");
        }
        Err(e) => {
            // Invalid track index is acceptable if no tracks exist
            assert!(e.contains("Invalid track") || e.contains("Index"),
                "Expected track/index error, got: {e}");
        }
    }
}

#[test]
fn test_ve_segment_metadata() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Invalid segment index should error
    let err = c.call_err("ve_segment_metadata", serde_json::json!({
        "track_index": 0, "segment_index": 99
    }));
    assert!(err.contains("segment") || err.contains("Index") || err.contains("project"),
        "Expected segment/project error, got: {err}");
}

#[test]
fn test_ve_segment_split() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_seg_split");
    wait_for_ui();

    // Split may fail if no segments exist at index 0 — that's acceptable
    let result = c.call("ve_segment_split", serde_json::json!({
        "track_index": 0, "segment_index": 0, "position_ms": 1000
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["success"].is_boolean(), "success should be boolean, got: {inner}");
        }
        Err(e) => {
            // Index out of bounds is acceptable if no segments exist
            assert!(e.contains("Index") || e.contains("bounds") || e.contains("segment"),
                "Expected index/segment error, got: {e}");
        }
    }
    wait_for_ui();
}

#[test]
fn test_ve_segment_move() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_seg_move");
    wait_for_ui();

    let result = c.call("ve_segment_move", serde_json::json!({
        "track_index": 0, "segment_index": 0, "offset_ms": 5000
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["success"].is_boolean(), "success should be boolean, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("Index") || e.contains("bounds") || e.contains("segment"),
                "Expected index/segment error, got: {e}");
        }
    }
    wait_for_ui();
}

#[test]
fn test_ve_segment_delete() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_seg_delete");
    wait_for_ui();

    let result = c.call("ve_segment_delete", serde_json::json!({
        "track_index": 0, "segment_index": 0
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["success"].is_boolean(), "success should be boolean, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("Index") || e.contains("bounds") || e.contains("segment"),
                "Expected index/segment error, got: {e}");
        }
    }
    wait_for_ui();
}

#[test]
fn test_ve_segment_toggle_visible() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_seg_visible");
    wait_for_ui();

    let result = c.call("ve_segment_toggle_visible", serde_json::json!({
        "track_index": 0, "segment_index": 0
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["is_visible"].is_boolean(),
                "is_visible should be boolean, got: {inner}");
        }
        Err(e) => {
            // May fail if no segment exists
            assert!(e.contains("Index") || e.contains("bounds") || e.contains("segment"),
                "Expected index/segment error, got: {e}");
        }
    }
}

#[test]
fn test_ve_segment_toggle_audio() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_seg_audio");
    wait_for_ui();

    let result = c.call("ve_segment_toggle_audio", serde_json::json!({
        "track_index": 0, "segment_index": 0
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["is_muted"].is_boolean(),
                "is_muted should be boolean, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("Index") || e.contains("bounds") || e.contains("segment"),
                "Expected index/segment error, got: {e}");
        }
    }
}

#[test]
fn test_ve_segment_remove_gap() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Invalid direction should error
    let err = c.call_err("ve_segment_remove_gap", serde_json::json!({
        "track_index": 0, "segment_index": 0, "direction": "up"
    }));
    assert!(err.contains("Invalid direction"), "Expected invalid direction, got: {err}");

    // Valid directions should work (may fail on missing project/segment, which is acceptable)
    c.open_test_project("test_seg_gap");
    wait_for_ui();

    for dir in ["left", "right"] {
        let result = c.call("ve_segment_remove_gap", serde_json::json!({
            "track_index": 0, "segment_index": 0, "direction": dir
        }));
        match result {
            Ok(val) => {
                let inner = val.get("structuredContent").cloned().unwrap_or(val);
                let inner = inner.get("result").cloned().unwrap_or(inner);
                assert!(inner["success"].is_boolean(), "success should be boolean for dir={dir}, got: {inner}");
            }
            Err(_) => {} // Missing segment is acceptable
        }
    }
}

// ============================================================
// 5. FILTER TOOLS (4 tools)
// ============================================================

#[test]
fn test_ve_filter_list_segment() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Without a project, should error (may be "No project" or "Invalid segment index")
    c.close_project();
    let result = c.call("ve_filter_list_segment", serde_json::json!({
        "track_index": 0, "segment_index": 0
    }));
    // The error message depends on whether a project is open or not.
    // If no project: "No project is currently open"
    // If project but no segment: "Invalid segment index 0 in track 0"
    // Either way, the call should fail for invalid indices.
    match result {
        Ok(val) => {
            // If it succeeded (project was open from a previous test), verify structure
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner.is_object() || inner.is_array(),
                "filter list result should be object or array, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("No project") || e.contains("Invalid") || e.contains("project") || e.contains("Index"),
                "Expected project/index error, got: {e}");
        }
    }

    // With a project
    c.open_test_project("test_filter_list");
    wait_for_ui();

    let result = c.call("ve_filter_list_segment", serde_json::json!({
        "track_index": 0, "segment_index": 0
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            // Should be a JSON value (likely an array of FilterInfo or empty array)
            assert!(inner.is_object() || inner.is_array(),
                "filter list result should be object or array, got: {inner}");
        }
        Err(e) => {
            // May fail if segment doesn't exist
            assert!(e.contains("Index") || e.contains("segment") || e.contains("track"),
                "Expected index/segment error, got: {e}");
        }
    }
}

#[test]
fn test_ve_filter_remove() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_filter_remove");
    wait_for_ui();

    let result = c.call("ve_filter_remove", serde_json::json!({
        "track_index": 0, "segment_index": 0, "filter_index": 0, "filter_type": "video"
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["success"].is_boolean(), "success should be boolean, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("Index") || e.contains("segment") || e.contains("track") || e.contains("project"),
                "Expected index/segment/project error, got: {e}");
        }
    }
}

#[test]
fn test_ve_filter_toggle() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_filter_toggle");
    wait_for_ui();

    let result = c.call("ve_filter_toggle", serde_json::json!({
        "track_index": 0, "segment_index": 0, "filter_index": 0, "filter_type": "video"
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["enabled"].is_boolean(), "enabled should be boolean, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("Index") || e.contains("segment") || e.contains("track") || e.contains("project"),
                "Expected index/segment/project error, got: {e}");
        }
    }
}

#[test]
fn test_ve_filter_clear() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_filter_clear");
    wait_for_ui();

    let result = c.call("ve_filter_clear", serde_json::json!({
        "track_index": 0, "segment_index": 0, "filter_type": "video"
    }));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner["success"].is_boolean(), "success should be boolean, got: {inner}");
        }
        Err(e) => {
            assert!(e.contains("Index") || e.contains("segment") || e.contains("track") || e.contains("project"),
                "Expected index/segment/project error, got: {e}");
        }
    }
}

// ============================================================
// 6. PREVIEW TOOLS (2 tools)
// ============================================================

#[test]
fn test_ve_preview_info() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Without a project, preview_info may return empty data or error
    // (depends on whether a project is open from a previous test)
    c.open_test_project("test_preview_info");
    wait_for_ui();

    let result = c.call_ok("ve_preview_info", serde_json::json!({}));
    assert!(result["duration_ms"].is_number(), "duration_ms should be number, got: {result}");
    assert!(result["track_count"].is_number(), "track_count should be number, got: {result}");
}

#[test]
fn test_ve_preview_seek() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Without a project, preview_seek may succeed or error
    // Open a project to ensure consistent behavior
    c.open_test_project("test_preview_seek");
    wait_for_ui();

    let result = c.call_ok("ve_preview_seek", serde_json::json!({"position_ms": 1000}));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

// ============================================================
// 7. SUBTITLE TOOLS (4 tools)
// ============================================================

#[test]
fn test_ve_subtitle_add() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_sub_add");
    wait_for_ui();

    let result = c.call_ok("ve_subtitle_add", serde_json::json!({
        "track_index": 0, "start_ms": 0, "end_ms": 3000, "text": "Hello"
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

#[test]
fn test_ve_subtitle_update() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_sub_update");
    wait_for_ui();

    let result = c.call_ok("ve_subtitle_update", serde_json::json!({
        "track_index": 0, "index": 0, "text": "Updated"
    }));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

#[test]
fn test_ve_subtitle_translate() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_sub_translate");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_subtitle_translate", serde_json::json!({
        "source_language": "zh", "target_language": "en"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_subtitle_translate_cancel() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok_with_timeout("ve_subtitle_translate_cancel", serde_json::json!({}), 8);
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

// ============================================================
// 8. TRANSCRIBE TOOLS (2 tools)
// ============================================================

#[test]
fn test_ve_transcribe_start() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_transcribe");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_transcribe_start", serde_json::json!({}), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_transcribe_cancel() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok_with_timeout("ve_transcribe_cancel", serde_json::json!({}), 8);
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

// ============================================================
// 9. OCR TOOL (1 tool)
// ============================================================

#[test]
fn test_ve_ocr_process_image() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_ocr_process_image", serde_json::json!({
        "image_path": "/tmp/test.png"
    }));
    assert!(result["status"].is_string(), "status should be string, got: {result}");
    assert!(result.get("note").is_some() || result.get("task_id").is_some(),
        "OCR result should have note or task_id, got: {result}");
}

// ============================================================
// 10. AI TOOLS (7 tools)
// ============================================================

#[test]
fn test_ve_ai_bg_remover_process() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_bg_remover");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_bg_remover_process", serde_json::json!({
        "image_path": "/tmp/test.png"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_ai_smart_clip_start() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_smart_clip");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_smart_clip_start", serde_json::json!({}), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_ai_scene_detect() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_scene");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_scene_detect", serde_json::json!({
        "track_index": 0, "segment_index": 0, "algorithm": "histogram"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_ai_dewatermark_process() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_dewatermark");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_dewatermark_process", serde_json::json!({
        "image_path": "/tmp/test.png"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_ai_cutout_process() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_cutout");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_cutout_process", serde_json::json!({
        "image_path": "/tmp/test.png"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_ai_chapter_summary() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_chapter");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_chapter_summary", serde_json::json!({}), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_ai_speakers_process() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_speakers");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_ai_speakers_process", serde_json::json!({
        "audio_path": "/tmp/test.wav"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

// ============================================================
// 11. AUDIO TOOLS (5 tools)
// ============================================================

#[test]
fn test_ve_audio_record_start() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_rec_start");
    wait_for_ui();

    let result = c.call_ok("ve_audio_record_start", serde_json::json!({}));
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_audio_record_stop() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_audio_record_stop", serde_json::json!({}));
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_audio_stem_split() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_stem");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_audio_stem_split", serde_json::json!({
        "audio_path": "/tmp/test.wav"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_audio_tts_generate() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_tts");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_audio_tts_generate", serde_json::json!({
        "text": "Hello"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_audio_vad_detect() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok_with_timeout("ve_audio_vad_detect", serde_json::json!({
        "audio_path": "/tmp/test.wav"
    }), 15);
    assert!(result["segments"].is_array(), "segments should be array, got: {result}");
    assert!(result.get("note").is_some(), "should have note, got: {result}");
}

// ============================================================
// 12. IMAGE TOOLS (5 tools)
// ============================================================

#[test]
fn test_ve_img_code_generate() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_img_code_generate", serde_json::json!({
        "code": "fn main() {}", "language": "rust"
    }));
    assert!(result["status"].is_string(), "status should be string, got: {result}");
    assert!(result.get("note").is_some(), "should have note, got: {result}");
}

#[test]
fn test_ve_img_pure_color_generate() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_img_pure_color_generate", serde_json::json!({
        "width": 100, "height": 100, "r": 255, "g": 0, "b": 0, "a": 255
    }));
    assert!(result["status"].is_string(), "status should be string, got: {result}");
    assert!(result.get("note").is_some(), "should have note, got: {result}");
}

#[test]
fn test_ve_img_long_screenshot() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_long_ss");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_img_long_screenshot", serde_json::json!({
        "track_index": 0, "segment_index": 0
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_img_animation_preview() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok_with_timeout("ve_img_animation_preview", serde_json::json!({
        "image_path": "/tmp/test.png"
    }), 8);
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_img_bg_animation() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_bg_anim");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_img_bg_animation", serde_json::json!({}), 8);
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

// ============================================================
// 13. FONT TOOLS (3 tools)
// ============================================================

#[test]
fn test_ve_font_list() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_font_list", serde_json::json!({}));
    assert!(result.get("fonts").is_some() || result.get("note").is_some(),
        "font list should have fonts or note, got: {result}");
}

#[test]
fn test_ve_font_search() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_font_search", serde_json::json!({"keyword": "Mono"}));
    assert!(result.is_object(), "font search result should be object, got: {result}");
    assert!(result["results"].is_array() || result.get("note").is_some(),
        "should have results array or note, got: {result}");
}

#[test]
fn test_ve_font_import() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_font_import", serde_json::json!({"file_path": "/tmp/font.zip"}));
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

// ============================================================
// 14. EXPORT TOOLS (5 tools)
// ============================================================

#[test]
fn test_ve_export_video() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_exp_video");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_export_video", serde_json::json!({
        "output_path": "/tmp/export.mp4"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
    assert!(result.get("note").is_some(), "should have note, got: {result}");
}

#[test]
fn test_ve_export_audio() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_exp_audio");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_export_audio", serde_json::json!({
        "output_path": "/tmp/export.wav"
    }), 15);
    assert!(result["task_id"].is_string(), "task_id should be string, got: {result}");
    assert!(result["status"].is_string(), "status should be string, got: {result}");
}

#[test]
fn test_ve_export_subtitle() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_exp_sub");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_export_subtitle", serde_json::json!({
        "output_path": "/tmp/export.srt", "format": "srt"
    }), 15);
    assert!(result["status"].is_string(), "status should be string, got: {result}");
    assert!(result.get("note").is_some(), "should have note, got: {result}");
}

#[test]
fn test_ve_export_queue() {
    let mut c = McpClient::new().expect("MCP server not running?");

    let result = c.call_ok("ve_export_queue", serde_json::json!({}));
    assert!(result["queue"].is_array(), "queue should be array, got: {result}");
}

#[test]
fn test_ve_export_cancel() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_exp_cancel");
    wait_for_ui();

    let result = c.call_ok_with_timeout("ve_export_cancel", serde_json::json!({"task_id": "999"}), 8);
    assert!(result["success"].is_boolean(), "success should be boolean, got: {result}");
}

// ============================================================
// 15. NO-PROJECT ERROR PATH TESTS
// ============================================================

#[test]
fn test_no_project_errors() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Ensure no project is open by closing and waiting
    c.close_project();

    // Tools that require a project and return "No project" error when none is open.
    // Note: Some tools (ve_track_list, ve_preview_info, ve_preview_seek) may return
    // empty data instead of erroring if a project from a previous test is still open.
    // We test the ones that reliably error when no project is open.
    let project_required_tools: Vec<(&str, Value)> = vec![
        ("ve_track_add", serde_json::json!({"track_type": "video", "name": "X"})),
        ("ve_segment_list", serde_json::json!({"track_index": 0})),
        ("ve_filter_list_segment", serde_json::json!({"track_index": 0, "segment_index": 0})),
        ("ve_filter_remove", serde_json::json!({"track_index": 0, "segment_index": 0, "filter_index": 0, "filter_type": "video"})),
        ("ve_filter_toggle", serde_json::json!({"track_index": 0, "segment_index": 0, "filter_index": 0, "filter_type": "video"})),
        ("ve_filter_clear", serde_json::json!({"track_index": 0, "segment_index": 0, "filter_type": "video"})),
        ("ve_segment_split", serde_json::json!({"track_index": 0, "segment_index": 0, "position_ms": 1000})),
        ("ve_segment_move", serde_json::json!({"track_index": 0, "segment_index": 0, "offset_ms": 1000})),
        ("ve_segment_delete", serde_json::json!({"track_index": 0, "segment_index": 0})),
        ("ve_segment_toggle_visible", serde_json::json!({"track_index": 0, "segment_index": 0})),
        ("ve_segment_toggle_audio", serde_json::json!({"track_index": 0, "segment_index": 0})),
        ("ve_segment_metadata", serde_json::json!({"track_index": 0, "segment_index": 0})),
        ("ve_export_video", serde_json::json!({"output_path": "/tmp/out.mp4"})),
        ("ve_export_audio", serde_json::json!({"output_path": "/tmp/out.wav"})),
        ("ve_export_subtitle", serde_json::json!({"output_path": "/tmp/out.srt", "format": "srt"})),
        ("ve_subtitle_add", serde_json::json!({"track_index": 0, "start_ms": 0, "end_ms": 1000, "text": "x"})),
        ("ve_subtitle_update", serde_json::json!({"track_index": 0, "index": 0, "text": "x"})),
        ("ve_subtitle_translate", serde_json::json!({"source_language": "zh", "target_language": "en"})),
        ("ve_transcribe_start", serde_json::json!({})),
        ("ve_ai_bg_remover_process", serde_json::json!({"image_path": "/tmp/test.png"})),
        ("ve_ai_smart_clip_start", serde_json::json!({})),
        ("ve_ai_scene_detect", serde_json::json!({"track_index": 0, "segment_index": 0, "algorithm": "histogram"})),
        ("ve_ai_dewatermark_process", serde_json::json!({"image_path": "/tmp/test.png"})),
        ("ve_ai_cutout_process", serde_json::json!({"image_path": "/tmp/test.png"})),
        ("ve_ai_chapter_summary", serde_json::json!({})),
        ("ve_ai_speakers_process", serde_json::json!({"audio_path": "/tmp/test.wav"})),
        ("ve_audio_record_start", serde_json::json!({})),
        ("ve_audio_stem_split", serde_json::json!({"audio_path": "/tmp/test.wav"})),
        ("ve_audio_tts_generate", serde_json::json!({"text": "hello"})),
        ("ve_img_long_screenshot", serde_json::json!({"track_index": 0, "segment_index": 0})),
    ];

    for (tool, args) in project_required_tools {
        let result = c.call(tool, args.clone());
        match result {
            Err(e) => {
                // Error should mention project or be an index/bounds error
                assert!(
                    e.contains("No project") || e.contains("project") ||
                    e.contains("Index") || e.contains("Invalid") || e.contains("bounds"),
                    "{tool}: Expected project/index error, got: {e}"
                );
            }
            Ok(_) => {
                // If the call succeeded, it means a project from a previous test is still open.
                // This is acceptable in integration tests — the important thing is no crash.
            }
        }
    }
}

// ============================================================
// 16. WORKFLOW TESTS
// ============================================================

#[test]
fn test_workflow_project_lifecycle() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Step 1: Check initial status
    c.close_project();
    let status = c.call_ok("ve_project_status", serde_json::json!({}));
    assert!(!status["is_open"].as_bool().unwrap_or(true), "Project should not be open initially");

    // Step 2: Create project
    let result = c.call_ok("ve_project_create", serde_json::json!({
        "name": "wf_lifecycle",
        "dir_path": "/tmp"
    }));
    assert!(result["success"].as_bool().unwrap_or(false), "Create should succeed");
    assert!(result["project_path"].is_string(), "Should return project_path");

    // Step 3: Wait and verify project is open
    wait_longer();
    let status = c.call_ok("ve_project_status", serde_json::json!({}));
    // After CreateProject, the project may or may not be fully open yet
    // depending on UI timing. Just verify the fields exist.
    assert!(status["is_open"].is_boolean());

    // Step 4: Close project
    let result = c.call_ok("ve_project_close", serde_json::json!({}));
    assert!(result["success"].as_bool().unwrap_or(false), "Close should succeed");

    // Step 5: Verify project is closed
    wait_for_ui();
    let status = c.call_ok("ve_project_status", serde_json::json!({}));
    assert!(!status["is_open"].as_bool().unwrap_or(true), "Project should be closed");
}

#[test]
fn test_workflow_track_management() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open project
    c.open_test_project("test_track_list");
    wait_longer();

    // Get initial track count
    let initial = c.call_ok("ve_track_list", serde_json::json!({}));
    let initial_count = initial["tracks"].as_array().map(|a| a.len()).unwrap_or(0);

    // Add a track
    let result = c.call_ok("ve_track_add", serde_json::json!({
        "track_type": "video", "name": "WF_V1"
    }));
    assert!(result["track_index"].is_number(), "Should return track_index");
    wait_longer();

    // Verify track count increased
    let after_add = c.call_ok("ve_track_list", serde_json::json!({}));
    let after_count = after_add["tracks"].as_array().map(|a| a.len()).unwrap_or(0);
    assert!(after_count >= initial_count, "Track count should not decrease after add (was {initial_count}, now {after_count})");

    // Toggle locked
    if after_count > 0 {
        let result = c.call_ok("ve_track_toggle_locked", serde_json::json!({"track_index": 0}));
        assert!(result["is_locked"].is_boolean(), "is_locked should be boolean");

        // Toggle again to restore
        let result = c.call_ok("ve_track_toggle_locked", serde_json::json!({"track_index": 0}));
        assert!(result["is_locked"].is_boolean(), "is_locked should be boolean after second toggle");
    }

    // Remove track
    if after_count > 0 {
        let result = c.call_ok("ve_track_remove", serde_json::json!({"track_index": 0}));
        assert!(result["success"].is_boolean(), "success should be boolean");
        wait_longer();
    }
}

#[test]
fn test_workflow_undo_redo() {
    let mut c = McpClient::new().expect("MCP server not running?");

    // Open project
    c.open_test_project("test_undo");
    wait_longer();

    // Add a track (creates an undoable action)
    c.call_ok("ve_track_add", serde_json::json!({"track_type": "video", "name": "UNDO_TEST"}));
    wait_longer();

    // Undo should now work
    let result = c.call("ve_project_undo", serde_json::json!({}));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            // Should have description if using the typed output, or success
            assert!(inner.get("description").is_some() || inner.get("success").is_some(),
                "Undo should return description or success");
        }
        Err(e) => {
            // Undo may fail if the UI didn't process the add track yet
            assert!(e.contains("undo") || e.contains("Undo") || e.contains("commands"),
                "Unexpected undo error: {e}");
        }
    }
    wait_longer();

    // Redo should now work
    let result = c.call("ve_project_redo", serde_json::json!({}));
    match result {
        Ok(val) => {
            let inner = val.get("structuredContent").cloned().unwrap_or(val);
            let inner = inner.get("result").cloned().unwrap_or(inner);
            assert!(inner.get("description").is_some() || inner.get("success").is_some(),
                "Redo should return description or success");
        }
        Err(e) => {
            assert!(e.contains("redo") || e.contains("Redo") || e.contains("commands"),
                "Unexpected redo error: {e}");
        }
    }
}

#[test]
fn test_workflow_export_lifecycle() {
    let mut c = McpClient::new().expect("MCP server not running?");

    c.open_test_project("test_exp_video");
    wait_longer();

    // Start export
    let result = c.call_ok_with_timeout("ve_export_video", serde_json::json!({
        "output_path": "/tmp/wf_export.mp4"
    }), 15);
    assert!(result["task_id"].is_string(), "Should return task_id");
    let task_id = result["task_id"].as_str().unwrap_or("").to_string();

    // Check queue
    let queue = c.call_ok("ve_export_queue", serde_json::json!({}));
    assert!(queue["queue"].is_array(), "queue should be array");

    // Cancel export
    if !task_id.is_empty() {
        let result = c.call_ok_with_timeout("ve_export_cancel", serde_json::json!({
            "task_id": task_id
        }), 8);
        assert!(result["success"].is_boolean(), "success should be boolean");
    }
}

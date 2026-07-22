use rmcp::ErrorData;

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("No project is currently open")]
    ProjectNotOpen,

    #[error("A project is already open. Close it first.")]
    ProjectAlreadyOpen,

    #[error("Failed to create project: {0}")]
    ProjectCreateFailed(String),

    #[error("Failed to open project: {0}")]
    ProjectOpenFailed(String),

    #[error("Failed to close project: {0}")]
    ProjectCloseFailed(String),

    #[error("Invalid track type: '{0}'. Must be video/audio/subtitle/image/text")]
    InvalidTrackType(String),

    #[error("Invalid track index: {0}")]
    InvalidTrackIndex(usize),

    #[error("Invalid segment index {segment} in track {track}")]
    InvalidSegmentIndex { track: usize, segment: usize },

    #[error("Invalid filter index {filter} in segment {segment} of track {track}")]
    InvalidFilterIndex {
        track: usize,
        segment: usize,
        filter: usize,
    },

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("{0}")]
    Internal(String),
}

impl From<McpError> for ErrorData {
    fn from(err: McpError) -> ErrorData {
        match &err {
            McpError::Internal(_) => ErrorData::internal_error(err.to_string(), None),
            _ => ErrorData::invalid_params(err.to_string(), None),
        }
    }
}

impl From<video_editor::Error> for McpError {
    fn from(err: video_editor::Error) -> McpError {
        McpError::Internal(err.to_string())
    }
}

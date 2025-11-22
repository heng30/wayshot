#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("output `{0}` was not found")]
    NoOutput(String),

    #[error("no screen captures when trying to composite the complete capture")]
    NoCaptures,

    #[error("screen info error")]
    ScreenInfo(#[from] screen_capture::ScreenInfoError),

    #[error("capture error")]
    CaptureInfo(#[from] super::backend::CaptureError),

    #[error("{0}")]
    Unimplemented(String),

    #[error("{0}")]
    Other(String),
}

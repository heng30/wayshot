#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("output `{0}` was not found")]
    NoOutput(String),

    #[error("no screen captures when trying to composite the complete capture")]
    NoCaptures,

    #[error("failed to connect to the wayland server")]
    Connect(#[from] wayland_client::ConnectError),

    #[error("failed to dispatch event from wayland server")]
    Dispatch(#[from] wayland_client::DispatchError),

    #[error("screen info error")]
    ScreenInfo(#[from] screen_capture::ScreenInfoError),

    #[error("{0}")]
    Unimplemented(String),

    #[error("{0}")]
    Other(String),
}

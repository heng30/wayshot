/// Errors that can occur during screen capture operations.
///
/// This enum represents all possible errors that can occur when using
/// the screen capture library. It implements the `std::error::Error` trait
/// through the `thiserror` crate.
///
/// # Example
///
/// ```no_run
/// use lib::capture::{capture_output, Error};
///
/// match capture_output("nonexistent", false) {
///     Ok(capture) => println!("Capture successful"),
///     Err(Error::NoOutput(name)) => println!("Output {} not found", name),
///     Err(Error::Connect(e)) => println!("Connection failed: {}", e),
///     Err(e) => println!("Other error: {}", e),
/// }
/// ```
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The specified output was not found
    #[error("output \"{0}\" was not found")]
    NoOutput(String),

    /// No screen captures were available when trying to composite the complete capture
    #[error("no screen captures when trying to composite the complete capture")]
    NoCaptures,

    /// Failed to connect to the Wayland server
    #[error("failed to connect to the wayland server")]
    Connect(#[from] wayland_client::ConnectError),

    /// Failed to dispatch event from Wayland server
    #[error("failed to dispatch event from wayland server")]
    Dispatch(#[from] wayland_client::DispatchError),

    /// Failed to execute external command (e.g., `wlr-randr`)
    #[error("{0}")]
    Command(String),

    /// Operation is not yet implemented
    #[error("{0}")]
    Unimplemented(String),

    /// Other miscellaneous errors
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("token is not correct.")]
    TokenIsNotCorrect,

    #[error("no token found.")]
    NoTokenFound,

    #[error("invalid token format.")]
    InvalidTokenFormat,
}

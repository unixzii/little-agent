/// The kind of error that occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The model provider is rate limited.
    RateLimitExceeded,
    /// Any other errors.
    Other,
}

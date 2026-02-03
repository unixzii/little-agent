use std::borrow::Cow;
use std::fmt::{self, Display};

/// The kind of error that occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The input provided to the tool was invalid.
    InvalidInput,
    /// Error occurred while executing the tool.
    ExecutionError,
    /// The tool was not allowed to execute.
    PermissionDenied,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::InvalidInput => write!(f, "Invalid input"),
            ErrorKind::ExecutionError => write!(f, "Execution error"),
            ErrorKind::PermissionDenied => write!(f, "Permission denied"),
        }
    }
}

/// Describes a tool call error.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Error {
    kind: ErrorKind,
    reason: Option<String>,
}

impl Error {
    /// Creates a new error with the `InvalidInput` kind.
    #[inline]
    pub fn invalid_input() -> Self {
        Self {
            kind: ErrorKind::InvalidInput,
            reason: None,
        }
    }

    /// Creates a new error with the `ExecutionError` kind.
    #[inline]
    pub fn execution_error() -> Self {
        Self {
            kind: ErrorKind::ExecutionError,
            reason: None,
        }
    }

    /// Creates a new error with the `PermissionDenied` kind.
    #[inline]
    pub fn permission_denied() -> Self {
        Self {
            kind: ErrorKind::PermissionDenied,
            reason: None,
        }
    }

    /// Attaches a reason to the error.
    #[inline]
    pub fn with_reason<S: Into<String>>(self, reason: S) -> Self {
        Self {
            kind: self.kind,
            reason: Some(reason.into()),
        }
    }

    /// Returns the reason for the error.
    #[inline]
    pub fn reason(&self) -> Cow<'_, str> {
        match self.reason.as_deref() {
            Some(reason) => Cow::Borrowed(reason),
            None => Cow::Owned(format!("{}", self.kind)),
        }
    }
}

use std::fmt;

/// Failures from graph construction, configuration, or navigation commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavError {
    InvalidConfig(String),
    UnknownNode(String),
    UnknownBranch(String),
    UnknownPath(String),
    /// The command does not apply in the current position (wrong path, too far, and so on).
    NotAvailable(String),
    NoParent,
    /// A turn is in progress; the command was ignored.
    Busy,
}

impl fmt::Display for NavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NavError::InvalidConfig(msg) => write!(f, "invalid config: {msg}"),
            NavError::UnknownNode(id) => write!(f, "unknown node '{id}'"),
            NavError::UnknownBranch(id) => write!(f, "unknown branch '{id}'"),
            NavError::UnknownPath(id) => write!(f, "unknown path '{id}'"),
            NavError::NotAvailable(msg) => write!(f, "{msg}"),
            NavError::NoParent => write!(f, "the active path has no parent to return to"),
            NavError::Busy => write!(f, "a turn is already in progress"),
        }
    }
}

impl std::error::Error for NavError {}

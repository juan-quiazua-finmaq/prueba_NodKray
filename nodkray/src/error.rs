//! Typed error model and the stable exit-code convention (spec §113, §125).
//!
//! Every error carries a machine-readable `code`, a `category` (one of the
//! eleven categories from §113), a human `message` and a `recoverable` flag.
//! When `--json` is active the error is rendered exactly as:
//!
//! ```json
//! {"code": "...", "category": "...", "message": "...", "recoverable": true}
//! ```

use std::fmt;

use serde::Serialize;

/// Convenience alias used across the crate.
pub type NodkrayResult<T> = Result<T, NodkrayError>;

/// Category of an internal error (spec §113).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Configuration,
    Dependency,
    Agent,
    Execution,
    Git,
    Memory,
    Review,
    Network,
    Permission,
    UserInput,
    Internal,
}

impl ErrorCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCategory::Configuration => "configuration",
            ErrorCategory::Dependency => "dependency",
            ErrorCategory::Agent => "agent",
            ErrorCategory::Execution => "execution",
            ErrorCategory::Git => "git",
            ErrorCategory::Memory => "memory",
            ErrorCategory::Review => "review",
            ErrorCategory::Network => "network",
            ErrorCategory::Permission => "permission",
            ErrorCategory::UserInput => "user_input",
            ErrorCategory::Internal => "internal",
        }
    }

    /// Base exit code for a category (spec §125). Specific codes such as
    /// `BLOCKED`, `CONFLICT` or `AUTH_*` override this in
    /// [`NodkrayError::exit_code`].
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorCategory::UserInput => 2,
            ErrorCategory::Configuration => 3,
            ErrorCategory::Dependency => 4,
            ErrorCategory::Agent => 5,
            ErrorCategory::Execution => 6,
            ErrorCategory::Review => 7,
            ErrorCategory::Git
            | ErrorCategory::Memory
            | ErrorCategory::Network
            | ErrorCategory::Permission
            | ErrorCategory::Internal => 1,
        }
    }
}

/// Payload shared by every [`NodkrayError`] variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
}

impl ErrorDetail {
    pub fn new(code: impl Into<String>, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recoverable,
        }
    }
}

impl fmt::Display for ErrorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.code.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.code, self.message)
        }
    }
}

/// Typed (canonical) error of the application.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NodkrayError {
    #[error("{0}")]
    Configuration(ErrorDetail),
    #[error("{0}")]
    Dependency(ErrorDetail),
    #[error("{0}")]
    Agent(ErrorDetail),
    #[error("{0}")]
    Execution(ErrorDetail),
    #[error("{0}")]
    Git(ErrorDetail),
    #[error("{0}")]
    Memory(ErrorDetail),
    #[error("{0}")]
    Review(ErrorDetail),
    #[error("{0}")]
    Network(ErrorDetail),
    #[error("{0}")]
    Permission(ErrorDetail),
    #[error("{0}")]
    UserInput(ErrorDetail),
    #[error("{0}")]
    Internal(ErrorDetail),
}

impl NodkrayError {
    fn build(category: ErrorCategory, detail: ErrorDetail) -> Self {
        match category {
            ErrorCategory::Configuration => NodkrayError::Configuration(detail),
            ErrorCategory::Dependency => NodkrayError::Dependency(detail),
            ErrorCategory::Agent => NodkrayError::Agent(detail),
            ErrorCategory::Execution => NodkrayError::Execution(detail),
            ErrorCategory::Git => NodkrayError::Git(detail),
            ErrorCategory::Memory => NodkrayError::Memory(detail),
            ErrorCategory::Review => NodkrayError::Review(detail),
            ErrorCategory::Network => NodkrayError::Network(detail),
            ErrorCategory::Permission => NodkrayError::Permission(detail),
            ErrorCategory::UserInput => NodkrayError::UserInput(detail),
            ErrorCategory::Internal => NodkrayError::Internal(detail),
        }
    }

    /// Generic constructor. Prefer the per-category helpers below.
    pub fn new(
        category: ErrorCategory,
        code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self::build(category, ErrorDetail::new(code, message, recoverable))
    }

    pub fn configuration(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Configuration, code, message, false)
    }

    pub fn dependency(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Dependency, code, message, true)
    }

    pub fn agent(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Agent, code, message, true)
    }

    pub fn execution(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Execution, code, message, false)
    }

    pub fn git(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Git, code, message, true)
    }

    pub fn memory(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Memory, code, message, false)
    }

    pub fn review(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Review, code, message, false)
    }

    pub fn network(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Network, code, message, true)
    }

    pub fn permission(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Permission, code, message, false)
    }

    pub fn user_input(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::UserInput, code, message, true)
    }

    pub fn internal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Internal, code, message, false)
    }

    pub fn detail(&self) -> &ErrorDetail {
        match self {
            NodkrayError::Configuration(d)
            | NodkrayError::Dependency(d)
            | NodkrayError::Agent(d)
            | NodkrayError::Execution(d)
            | NodkrayError::Git(d)
            | NodkrayError::Memory(d)
            | NodkrayError::Review(d)
            | NodkrayError::Network(d)
            | NodkrayError::Permission(d)
            | NodkrayError::UserInput(d)
            | NodkrayError::Internal(d) => d,
        }
    }

    pub fn category(&self) -> ErrorCategory {
        match self {
            NodkrayError::Configuration(_) => ErrorCategory::Configuration,
            NodkrayError::Dependency(_) => ErrorCategory::Dependency,
            NodkrayError::Agent(_) => ErrorCategory::Agent,
            NodkrayError::Execution(_) => ErrorCategory::Execution,
            NodkrayError::Git(_) => ErrorCategory::Git,
            NodkrayError::Memory(_) => ErrorCategory::Memory,
            NodkrayError::Review(_) => ErrorCategory::Review,
            NodkrayError::Network(_) => ErrorCategory::Network,
            NodkrayError::Permission(_) => ErrorCategory::Permission,
            NodkrayError::UserInput(_) => ErrorCategory::UserInput,
            NodkrayError::Internal(_) => ErrorCategory::Internal,
        }
    }

    pub fn code(&self) -> &str {
        &self.detail().code
    }

    pub fn message(&self) -> &str {
        &self.detail().message
    }

    pub fn recoverable(&self) -> bool {
        self.detail().recoverable
    }

    /// Stable process exit code (spec §125).
    pub fn exit_code(&self) -> i32 {
        match self.code() {
            "BLOCKED" => 8,
            "CONFLICT" => 9,
            c if c == "AUTHENTICATION_FAILED" || c.starts_with("AUTH_") => 10,
            _ => self.category().exit_code(),
        }
    }

    /// Serialisable representation used by `--json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code(),
            "category": self.category().as_str(),
            "message": self.message(),
            "recoverable": self.recoverable(),
        })
    }
}

impl From<std::io::Error> for NodkrayError {
    fn from(err: std::io::Error) -> Self {
        NodkrayError::internal("IO_ERROR", err.to_string())
    }
}

impl From<serde_yaml::Error> for NodkrayError {
    fn from(err: serde_yaml::Error) -> Self {
        NodkrayError::configuration("CONFIG_PARSE_ERROR", err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_map_to_expected_exit_codes() {
        assert_eq!(NodkrayError::configuration("X", "m").exit_code(), 3);
        assert_eq!(NodkrayError::dependency("X", "m").exit_code(), 4);
        assert_eq!(NodkrayError::agent("X", "m").exit_code(), 5);
        assert_eq!(NodkrayError::execution("X", "m").exit_code(), 6);
        assert_eq!(NodkrayError::review("X", "m").exit_code(), 7);
        assert_eq!(NodkrayError::user_input("X", "m").exit_code(), 2);
        assert_eq!(NodkrayError::internal("X", "m").exit_code(), 1);
    }

    #[test]
    fn special_codes_override_category_exit_codes() {
        assert_eq!(NodkrayError::internal("BLOCKED", "m").exit_code(), 8);
        assert_eq!(NodkrayError::git("CONFLICT", "m").exit_code(), 9);
        assert_eq!(NodkrayError::network("AUTH_FAILED", "m").exit_code(), 10);
    }

    #[test]
    fn json_error_has_expected_shape() {
        let err = NodkrayError::agent("AGENT_NOT_FOUND", "Codex executable was not found");
        let json = err.to_json();
        assert_eq!(json["code"], "AGENT_NOT_FOUND");
        assert_eq!(json["category"], "agent");
        assert_eq!(json["message"], "Codex executable was not found");
        assert_eq!(json["recoverable"], true);
    }
}

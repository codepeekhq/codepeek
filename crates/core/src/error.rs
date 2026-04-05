use std::path::PathBuf;

/// Errors from change detection and diff operations.
#[derive(Debug, thiserror::Error)]
pub enum ChangeError {
    #[error("repository not found at {path}")]
    RepoNotFound { path: PathBuf },

    #[error("failed to read file status")]
    StatusFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("failed to compute diff for {path}")]
    DiffFailed {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("file not found at HEAD: {path}")]
    FileNotInHead { path: PathBuf },
}

/// Errors from syntax highlighting operations.
#[derive(Debug, thiserror::Error)]
pub enum SyntaxError {
    #[error("unsupported language for {path}")]
    UnsupportedLanguage { path: PathBuf },

    #[error("failed to parse {path}")]
    ParseFailed {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

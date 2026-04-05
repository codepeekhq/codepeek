use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_error_repo_not_found_display() {
        let err = ChangeError::RepoNotFound {
            path: PathBuf::from("/tmp/repo"),
        };
        assert_eq!(err.to_string(), "repository not found at /tmp/repo");
    }

    #[test]
    fn change_error_status_failed_display() {
        let err = ChangeError::StatusFailed("status error".into());
        assert_eq!(err.to_string(), "failed to read file status");
    }

    #[test]
    fn change_error_status_failed_source() {
        use std::error::Error;
        let err = ChangeError::StatusFailed("inner cause".into());
        let source = err.source().expect("should have a source");
        assert_eq!(source.to_string(), "inner cause");
    }

    #[test]
    fn change_error_diff_failed_display() {
        let err = ChangeError::DiffFailed {
            path: PathBuf::from("src/lib.rs"),
            source: "diff error".into(),
        };
        assert_eq!(err.to_string(), "failed to compute diff for src/lib.rs");
    }

    #[test]
    fn change_error_diff_failed_source() {
        use std::error::Error;
        let err = ChangeError::DiffFailed {
            path: PathBuf::from("src/lib.rs"),
            source: "inner diff error".into(),
        };
        let source = err.source().expect("should have a source");
        assert_eq!(source.to_string(), "inner diff error");
    }

    #[test]
    fn change_error_file_not_in_head_display() {
        let err = ChangeError::FileNotInHead {
            path: PathBuf::from("deleted.rs"),
        };
        assert_eq!(err.to_string(), "file not found at HEAD: deleted.rs");
    }

    #[test]
    fn syntax_error_unsupported_language_display() {
        let err = SyntaxError::UnsupportedLanguage {
            path: PathBuf::from("file.xyz"),
        };
        assert_eq!(err.to_string(), "unsupported language for file.xyz");
    }

    #[test]
    fn syntax_error_parse_failed_display() {
        let err = SyntaxError::ParseFailed {
            path: PathBuf::from("bad.rs"),
            source: "parse error".into(),
        };
        assert_eq!(err.to_string(), "failed to parse bad.rs");
    }

    #[test]
    fn syntax_error_parse_failed_source() {
        use std::error::Error;
        let err = SyntaxError::ParseFailed {
            path: PathBuf::from("bad.rs"),
            source: "inner parse error".into(),
        };
        let source = err.source().expect("should have a source");
        assert_eq!(source.to_string(), "inner parse error");
    }
}

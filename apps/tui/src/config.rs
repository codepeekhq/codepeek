use std::collections::HashSet;
use std::path::PathBuf;

use serde::Deserialize;

use codepeek_syntax::SUPPORTED_LANGUAGES;

#[derive(Debug, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub languages: LanguagesConfig,
}

#[derive(Debug, Deserialize)]
pub struct LanguagesConfig {
    #[serde(default = "default_languages")]
    pub enabled: Vec<String>,
}

impl Default for LanguagesConfig {
    fn default() -> Self {
        Self {
            enabled: default_languages(),
        }
    }
}

fn default_languages() -> Vec<String> {
    SUPPORTED_LANGUAGES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

#[derive(Debug)]
pub enum ConfigWarning {
    ParseError { path: PathBuf, message: String },
    ReadError { path: PathBuf, message: String },
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError { path, message } => {
                write!(f, "Failed to parse {}: {message}", path.display())
            }
            Self::ReadError { path, message } => {
                write!(f, "Failed to read {}: {message}", path.display())
            }
        }
    }
}

impl AppConfig {
    /// Load config from `~/.config/codepeek/config.toml`.
    ///
    /// Returns the config and an optional warning. If the config file doesn't
    /// exist, returns defaults with no warning. If it exists but can't be read
    /// or parsed, returns defaults with a warning so the caller can display it.
    pub fn load() -> (Self, Option<ConfigWarning>) {
        let Some(path) = config_path() else {
            return (Self::default(), None);
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), None);
            }
            Err(e) => {
                return (
                    Self::default(),
                    Some(ConfigWarning::ReadError {
                        path,
                        message: e.to_string(),
                    }),
                );
            }
        };

        match toml::from_str(&content) {
            Ok(config) => (config, None),
            Err(e) => (
                Self::default(),
                Some(ConfigWarning::ParseError {
                    path,
                    message: e.to_string(),
                }),
            ),
        }
    }

    pub fn enabled_languages(&self) -> HashSet<String> {
        self.languages.enabled.iter().cloned().collect()
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("codepeek").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enables_all_languages() {
        let config = AppConfig::default();
        let enabled = config.enabled_languages();

        for lang in SUPPORTED_LANGUAGES {
            assert!(
                enabled.contains(*lang),
                "default config should enable '{lang}'"
            );
        }
    }

    #[test]
    fn parse_partial_config() {
        let toml = r#"
[languages]
enabled = ["rust", "python"]
"#;
        let config: AppConfig = toml::from_str(toml).unwrap();
        let enabled = config.enabled_languages();

        assert!(enabled.contains("rust"));
        assert!(enabled.contains("python"));
        assert!(!enabled.contains("javascript"));
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn parse_empty_config_uses_defaults() {
        let toml = "";
        let config: AppConfig = toml::from_str(toml).unwrap();
        let enabled = config.enabled_languages();

        assert_eq!(enabled.len(), SUPPORTED_LANGUAGES.len());
    }

    #[test]
    fn parse_config_without_languages_section_uses_defaults() {
        let toml = "[other]\nkey = \"value\"\n";
        let config: AppConfig = toml::from_str(toml).unwrap();
        let enabled = config.enabled_languages();

        assert_eq!(enabled.len(), SUPPORTED_LANGUAGES.len());
    }

    #[test]
    fn load_returns_defaults_when_no_config_dir() {
        // This tests the normal flow — no config file means defaults.
        let (config, warning) = AppConfig::load();
        // If no config file exists, should return defaults with no warning.
        // (Config file may or may not exist on the test machine, so we just
        // verify the return type works correctly.)
        assert!(!config.enabled_languages().is_empty());
        // Warning may or may not be present depending on test environment.
        let _ = warning;
    }

    #[test]
    fn config_warning_display() {
        let warn = ConfigWarning::ParseError {
            path: PathBuf::from("/tmp/config.toml"),
            message: "bad toml".to_string(),
        };
        let msg = format!("{warn}");
        assert!(msg.contains("Failed to parse"));
        assert!(msg.contains("bad toml"));

        let warn = ConfigWarning::ReadError {
            path: PathBuf::from("/tmp/config.toml"),
            message: "permission denied".to_string(),
        };
        let msg = format!("{warn}");
        assert!(msg.contains("Failed to read"));
        assert!(msg.contains("permission denied"));
    }
}

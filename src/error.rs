use thiserror::Error;

#[derive(Debug, Error)]
pub enum TermStackError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Data provider error: {0}")]
    DataProvider(String),

    #[error("Template error: {0}")]
    Template(String),

    #[error("Navigation error: {0}")]
    Navigation(String),

    #[error("Action execution error: {0}")]
    Action(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, TermStackError>;

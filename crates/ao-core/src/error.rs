use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("slot '{0}' already exists")]
    SlotAlreadyExists(String),

    #[error("slot '{0}' not found")]
    SlotNotFound(String),

    #[error("config file not found at {0}")]
    ConfigNotFound(PathBuf),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("git operation failed: {0}")]
    Git(String),

    #[error("tmux operation failed: {0}")]
    Tmux(String),

    #[error("aspire operation failed: {0}")]
    Aspire(String),

    #[error("agent operation failed: {0}")]
    Agent(String),

    #[error("port allocation failed: {0}")]
    PortAllocation(String),

    #[error("blueprint '{0}' not found")]
    BlueprintNotFound(String),

    #[error("blueprint '{0}' already exists")]
    BlueprintAlreadyExists(String),

    #[error("blueprint validation failed: {0}")]
    BlueprintValidation(String),

    #[error("state persistence failed: {0}")]
    State(String),

    #[error("process failed: {0}")]
    Process(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;

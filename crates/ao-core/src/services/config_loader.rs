use std::path::Path;

use crate::error::{OrchestratorError, Result};
use crate::models::OrchestratorConfig;

const CONFIG_FILENAME: &str = ".aspire-orchestrator.yaml";

pub fn load(repo_path: &Path) -> Result<OrchestratorConfig> {
    let config_path = repo_path.join(CONFIG_FILENAME);
    if !config_path.exists() {
        return Err(OrchestratorError::ConfigNotFound(config_path));
    }
    let contents = std::fs::read_to_string(&config_path)?;
    let config: OrchestratorConfig = serde_yaml::from_str(&contents)
        .map_err(|e| OrchestratorError::InvalidConfig(e.to_string()))?;
    if config.apphost.is_empty() {
        return Err(OrchestratorError::InvalidConfig(
            "apphost field is required".into(),
        ));
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_full_config() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
apphost: src/MyApp.AppHost/MyApp.AppHost.csproj
setup:
  - dotnet restore MyApp.slnx
  - npm install
port_overrides:
  VITE_PORT: 5173
  API_PORT: 5001
"#;
        fs::write(dir.path().join(CONFIG_FILENAME), yaml).unwrap();
        let config = load(dir.path()).unwrap();
        assert_eq!(config.apphost, "src/MyApp.AppHost/MyApp.AppHost.csproj");
        assert_eq!(config.setup.len(), 2);
        assert_eq!(config.port_overrides.get("VITE_PORT"), Some(&5173));
        assert_eq!(config.port_overrides.get("API_PORT"), Some(&5001));
    }

    #[test]
    fn parse_minimal_config() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = "apphost: src/App.AppHost/App.AppHost.csproj\n";
        fs::write(dir.path().join(CONFIG_FILENAME), yaml).unwrap();
        let config = load(dir.path()).unwrap();
        assert_eq!(config.apphost, "src/App.AppHost/App.AppHost.csproj");
        assert!(config.setup.is_empty());
        assert!(config.port_overrides.is_empty());
    }

    #[test]
    fn missing_config_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            load(dir.path()),
            Err(OrchestratorError::ConfigNotFound(_))
        ));
    }
}

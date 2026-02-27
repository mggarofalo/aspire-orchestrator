use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct OrchestratorConfig {
    pub apphost: String,
    #[serde(default)]
    pub setup: Vec<String>,
    #[serde(default)]
    pub port_overrides: HashMap<String, u16>,
}

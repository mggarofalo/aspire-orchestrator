use serde::{Deserialize, Serialize};

/// A blueprint defines a reusable set of slot configurations for quick setup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<BlueprintDefaults>,
    pub slots: Vec<BlueprintSlotEntry>,
}

/// Default values applied to all slots unless overridden at the slot level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_start_aspire: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_spawn_agent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<BlueprintAgentConfig>,
}

/// Agent configuration for a blueprint slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintAgentConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
}

/// A single slot entry in a blueprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintSlotEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_start_aspire: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_spawn_agent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<BlueprintAgentConfig>,
}

/// A fully resolved slot configuration ready for creation.
#[derive(Debug, Clone)]
pub struct ResolvedBlueprintSlot {
    pub name: String,
    pub source: String,
    pub branch: Option<String>,
    pub auto_start_aspire: bool,
    pub auto_spawn_agent: bool,
    pub prompt: Option<String>,
    pub allowed_tools: Option<String>,
    pub max_turns: Option<u32>,
}

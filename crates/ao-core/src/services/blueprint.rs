use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};
use crate::models::blueprint::{
    Blueprint, BlueprintAgentConfig, BlueprintSlotEntry, ResolvedBlueprintSlot,
};
use crate::models::Slot;

/// Manages blueprint YAML files in the `.slots/blueprints/` directory.
pub struct BlueprintStore {
    blueprints_dir: PathBuf,
}

impl BlueprintStore {
    pub fn new(slots_directory: &Path) -> Self {
        Self {
            blueprints_dir: slots_directory.join("blueprints"),
        }
    }

    /// List all available blueprint names.
    pub async fn list(&self) -> Result<Vec<String>> {
        let dir = &self.blueprints_dir;
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| OrchestratorError::State(format!("reading blueprints dir: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| OrchestratorError::State(format!("reading blueprint entry: {e}")))?
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }

        names.sort();
        Ok(names)
    }

    /// Load a blueprint by name.
    pub async fn load(&self, name: &str) -> Result<Blueprint> {
        let path = self.blueprints_dir.join(format!("{name}.yaml"));
        if !path.exists() {
            return Err(OrchestratorError::BlueprintNotFound(name.to_string()));
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| OrchestratorError::State(format!("reading blueprint: {e}")))?;

        let blueprint: Blueprint =
            serde_yaml::from_str(&content).map_err(OrchestratorError::Yaml)?;

        Ok(blueprint)
    }

    /// Save a blueprint to disk.
    pub async fn save(&self, blueprint: &Blueprint) -> Result<()> {
        // Check for existing
        let path = self.blueprints_dir.join(format!("{}.yaml", blueprint.name));
        if path.exists() {
            return Err(OrchestratorError::BlueprintAlreadyExists(
                blueprint.name.clone(),
            ));
        }

        tokio::fs::create_dir_all(&self.blueprints_dir)
            .await
            .map_err(|e| OrchestratorError::State(format!("creating blueprints dir: {e}")))?;

        let content = serde_yaml::to_string(blueprint).map_err(OrchestratorError::Yaml)?;

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| OrchestratorError::State(format!("writing blueprint: {e}")))?;

        Ok(())
    }

    /// Overwrite an existing blueprint on disk.
    pub async fn overwrite(&self, blueprint: &Blueprint) -> Result<()> {
        tokio::fs::create_dir_all(&self.blueprints_dir)
            .await
            .map_err(|e| OrchestratorError::State(format!("creating blueprints dir: {e}")))?;

        let path = self.blueprints_dir.join(format!("{}.yaml", blueprint.name));
        let content = serde_yaml::to_string(blueprint).map_err(OrchestratorError::Yaml)?;

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| OrchestratorError::State(format!("writing blueprint: {e}")))?;

        Ok(())
    }

    /// Delete a blueprint by name.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let path = self.blueprints_dir.join(format!("{name}.yaml"));
        if !path.exists() {
            return Err(OrchestratorError::BlueprintNotFound(name.to_string()));
        }

        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| OrchestratorError::State(format!("deleting blueprint: {e}")))?;

        Ok(())
    }

    /// Create a blueprint from the current set of slots (snapshot).
    pub fn snapshot_from_slots(name: &str, description: Option<&str>, slots: &[Slot]) -> Blueprint {
        let entries: Vec<BlueprintSlotEntry> = slots
            .iter()
            .map(|s| BlueprintSlotEntry {
                name: s.name.clone(),
                branch: Some(s.branch.clone()),
                source: Some(s.repo_path.clone()),
                auto_start_aspire: None,
                auto_spawn_agent: None,
                agent: None,
            })
            .collect();

        Blueprint {
            name: name.to_string(),
            description: description.map(|d| d.to_string()),
            defaults: None,
            slots: entries,
        }
    }
}

/// Validate a blueprint for completeness.
pub fn validate(blueprint: &Blueprint) -> std::result::Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if blueprint.name.is_empty() {
        errors.push("Blueprint name is required".to_string());
    }

    if blueprint.slots.is_empty() {
        errors.push("Blueprint must have at least one slot".to_string());
    }

    let has_default_source = blueprint
        .defaults
        .as_ref()
        .and_then(|d| d.source.as_ref())
        .is_some();

    for (i, slot) in blueprint.slots.iter().enumerate() {
        if slot.name.is_empty() {
            errors.push(format!("Slot {} has empty name", i));
        }
        if slot.source.is_none() && !has_default_source {
            errors.push(format!(
                "Slot '{}' has no source and no default source",
                slot.name
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Interpolate template variables in a string.
/// Supports `{slot_name}` and `{branch}`.
pub fn interpolate(template: &str, slot_name: &str, branch: &str) -> String {
    template
        .replace("{slot_name}", slot_name)
        .replace("{branch}", branch)
}

/// Resolve a blueprint into a list of fully-specified slot configurations.
pub fn resolve(blueprint: &Blueprint) -> Result<Vec<ResolvedBlueprintSlot>> {
    if let Err(errors) = validate(blueprint) {
        return Err(OrchestratorError::BlueprintValidation(errors.join("; ")));
    }

    let defaults = blueprint.defaults.as_ref();
    let default_source = defaults.and_then(|d| d.source.as_deref());
    let default_auto_start = defaults.and_then(|d| d.auto_start_aspire).unwrap_or(false);
    let default_auto_spawn = defaults.and_then(|d| d.auto_spawn_agent).unwrap_or(false);
    let default_agent = defaults.and_then(|d| d.agent.as_ref());

    let mut resolved = Vec::new();

    for slot in &blueprint.slots {
        let source = slot
            .source
            .as_deref()
            .or(default_source)
            .ok_or_else(|| {
                OrchestratorError::BlueprintValidation(format!(
                    "Slot '{}' has no source",
                    slot.name
                ))
            })?
            .to_string();

        let branch = slot.branch.clone();
        let branch_str = branch.as_deref().unwrap_or("main");

        let auto_start_aspire = slot.auto_start_aspire.unwrap_or(default_auto_start);
        let auto_spawn_agent = slot.auto_spawn_agent.unwrap_or(default_auto_spawn);

        // Resolve agent config: slot-level overrides default-level
        let agent_config = merge_agent_config(default_agent, slot.agent.as_ref());

        let prompt = agent_config
            .as_ref()
            .and_then(|a| a.prompt_template.as_deref())
            .map(|t| interpolate(t, &slot.name, branch_str));

        let allowed_tools = agent_config.as_ref().and_then(|a| a.allowed_tools.clone());
        let max_turns = agent_config.as_ref().and_then(|a| a.max_turns);

        resolved.push(ResolvedBlueprintSlot {
            name: slot.name.clone(),
            source,
            branch,
            auto_start_aspire,
            auto_spawn_agent,
            prompt,
            allowed_tools,
            max_turns,
        });
    }

    Ok(resolved)
}

/// Merge agent configs: slot-level fields override default-level fields.
fn merge_agent_config(
    default: Option<&BlueprintAgentConfig>,
    slot: Option<&BlueprintAgentConfig>,
) -> Option<BlueprintAgentConfig> {
    match (default, slot) {
        (None, None) => None,
        (Some(d), None) => Some(d.clone()),
        (None, Some(s)) => Some(s.clone()),
        (Some(d), Some(s)) => Some(BlueprintAgentConfig {
            prompt_template: s.prompt_template.clone().or(d.prompt_template.clone()),
            allowed_tools: s.allowed_tools.clone().or(d.allowed_tools.clone()),
            max_turns: s.max_turns.or(d.max_turns),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::blueprint::{BlueprintDefaults, BlueprintSlotEntry};

    #[test]
    fn test_interpolate() {
        let result = interpolate(
            "Work on {branch} in slot {slot_name}.",
            "api-cleanup",
            "feature/api-cleanup",
        );
        assert_eq!(result, "Work on feature/api-cleanup in slot api-cleanup.");
    }

    #[test]
    fn test_validate_empty_name() {
        let bp = Blueprint {
            name: String::new(),
            description: None,
            defaults: None,
            slots: vec![],
        };
        let err = validate(&bp).unwrap_err();
        assert!(err.iter().any(|e| e.contains("name is required")));
        assert!(err.iter().any(|e| e.contains("at least one slot")));
    }

    #[test]
    fn test_validate_missing_source() {
        let bp = Blueprint {
            name: "test".to_string(),
            description: None,
            defaults: None,
            slots: vec![BlueprintSlotEntry {
                name: "slot1".to_string(),
                branch: None,
                source: None,
                auto_start_aspire: None,
                auto_spawn_agent: None,
                agent: None,
            }],
        };
        let err = validate(&bp).unwrap_err();
        assert!(err.iter().any(|e| e.contains("no source")));
    }

    #[test]
    fn test_validate_success_with_default_source() {
        let bp = Blueprint {
            name: "test".to_string(),
            description: None,
            defaults: Some(BlueprintDefaults {
                source: Some("/path/to/repo".to_string()),
                auto_start_aspire: None,
                auto_spawn_agent: None,
                agent: None,
            }),
            slots: vec![BlueprintSlotEntry {
                name: "slot1".to_string(),
                branch: Some("main".to_string()),
                source: None,
                auto_start_aspire: None,
                auto_spawn_agent: None,
                agent: None,
            }],
        };
        assert!(validate(&bp).is_ok());
    }

    #[test]
    fn test_resolve_with_defaults() {
        let bp = Blueprint {
            name: "daily-dev".to_string(),
            description: Some("Test blueprint".to_string()),
            defaults: Some(BlueprintDefaults {
                source: Some("C:/Users/test/Source/Receipts".to_string()),
                auto_start_aspire: Some(true),
                auto_spawn_agent: Some(true),
                agent: Some(BlueprintAgentConfig {
                    prompt_template: Some("Work on {branch} in {slot_name}".to_string()),
                    allowed_tools: Some("Bash,Read".to_string()),
                    max_turns: Some(50),
                }),
            }),
            slots: vec![
                BlueprintSlotEntry {
                    name: "api-cleanup".to_string(),
                    branch: Some("feature/api-cleanup".to_string()),
                    source: None,
                    auto_start_aspire: None,
                    auto_spawn_agent: None,
                    agent: Some(BlueprintAgentConfig {
                        prompt_template: Some("Custom prompt for {slot_name}".to_string()),
                        allowed_tools: None,
                        max_turns: None,
                    }),
                },
                BlueprintSlotEntry {
                    name: "ui-dashboard".to_string(),
                    branch: Some("feature/ui-dashboard".to_string()),
                    source: None,
                    auto_start_aspire: Some(false),
                    auto_spawn_agent: None,
                    agent: None,
                },
            ],
        };

        let resolved = resolve(&bp).unwrap();
        assert_eq!(resolved.len(), 2);

        // First slot: overridden prompt, inherited tools/turns
        assert_eq!(resolved[0].name, "api-cleanup");
        assert_eq!(resolved[0].source, "C:/Users/test/Source/Receipts");
        assert_eq!(
            resolved[0].prompt.as_deref(),
            Some("Custom prompt for api-cleanup")
        );
        assert_eq!(resolved[0].allowed_tools.as_deref(), Some("Bash,Read"));
        assert_eq!(resolved[0].max_turns, Some(50));
        assert!(resolved[0].auto_start_aspire);
        assert!(resolved[0].auto_spawn_agent);

        // Second slot: default prompt, overridden auto_start
        assert_eq!(resolved[1].name, "ui-dashboard");
        assert!(!resolved[1].auto_start_aspire);
        assert!(resolved[1].auto_spawn_agent);
        assert_eq!(
            resolved[1].prompt.as_deref(),
            Some("Work on feature/ui-dashboard in ui-dashboard")
        );
    }

    #[test]
    fn test_snapshot_from_slots() {
        use crate::models::Slot;

        let slots = vec![
            Slot::new(
                "slot1".into(),
                "/path/to/repo".into(),
                "main".into(),
                "/slots/slot1".into(),
            ),
            Slot::new(
                "slot2".into(),
                "/path/to/repo".into(),
                "feature/x".into(),
                "/slots/slot2".into(),
            ),
        ];

        let bp = BlueprintStore::snapshot_from_slots("test-bp", Some("A test"), &slots);
        assert_eq!(bp.name, "test-bp");
        assert_eq!(bp.description.as_deref(), Some("A test"));
        assert_eq!(bp.slots.len(), 2);
        assert_eq!(bp.slots[0].name, "slot1");
        assert_eq!(bp.slots[0].source.as_deref(), Some("/path/to/repo"));
        assert_eq!(bp.slots[1].branch.as_deref(), Some("feature/x"));
    }

    #[tokio::test]
    async fn test_blueprint_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlueprintStore::new(dir.path());

        let bp = Blueprint {
            name: "test-bp".to_string(),
            description: Some("A test blueprint".to_string()),
            defaults: Some(BlueprintDefaults {
                source: Some("/path/to/repo".to_string()),
                auto_start_aspire: None,
                auto_spawn_agent: None,
                agent: None,
            }),
            slots: vec![BlueprintSlotEntry {
                name: "slot1".to_string(),
                branch: Some("main".to_string()),
                source: None,
                auto_start_aspire: None,
                auto_spawn_agent: None,
                agent: None,
            }],
        };

        // Save
        store.save(&bp).await.unwrap();

        // List
        let names = store.list().await.unwrap();
        assert_eq!(names, vec!["test-bp"]);

        // Load
        let loaded = store.load("test-bp").await.unwrap();
        assert_eq!(loaded.name, "test-bp");
        assert_eq!(loaded.description.as_deref(), Some("A test blueprint"));
        assert_eq!(loaded.slots.len(), 1);

        // Save duplicate fails
        let err = store.save(&bp).await.unwrap_err();
        assert!(matches!(err, OrchestratorError::BlueprintAlreadyExists(_)));

        // Delete
        store.delete("test-bp").await.unwrap();
        let names = store.list().await.unwrap();
        assert!(names.is_empty());

        // Load after delete fails
        let err = store.load("test-bp").await.unwrap_err();
        assert!(matches!(err, OrchestratorError::BlueprintNotFound(_)));
    }
}

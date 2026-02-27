use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};
use crate::models::Slot;

pub struct SlotStateStore {
    state_file_path: PathBuf,
}

impl SlotStateStore {
    pub fn new(slots_directory: &Path) -> Self {
        Self {
            state_file_path: slots_directory.join("state.json"),
        }
    }

    pub async fn load(&self) -> Result<Vec<Slot>> {
        if !self.state_file_path.exists() {
            return Ok(Vec::new());
        }
        let json = tokio::fs::read_to_string(&self.state_file_path)
            .await
            .map_err(|e| OrchestratorError::State(format!("failed to read state file: {e}")))?;
        let slots: Vec<Slot> = serde_json::from_str(&json)?;
        Ok(slots)
    }

    pub async fn save(&self, slots: &[Slot]) -> Result<()> {
        if let Some(parent) = self.state_file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                OrchestratorError::State(format!("failed to create state dir: {e}"))
            })?;
        }
        let json = serde_json::to_string_pretty(slots)?;
        tokio::fs::write(&self.state_file_path, json)
            .await
            .map_err(|e| OrchestratorError::State(format!("failed to write state file: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentStatus, DiscoveredServices, SlotStatus};

    fn test_slot() -> Slot {
        Slot {
            name: "test-1".into(),
            repo_path: "/repo".into(),
            branch: "main".into(),
            clone_path: "/clone/test-1".into(),
            status: SlotStatus::Ready,
            agent_status: AgentStatus::None,
            port_allocations: vec![],
            services: DiscoveredServices::default(),
            created_at: chrono::Utc::now(),
            aspire_started_at: None,
            agent_started_at: None,
            last_agent_output_at: None,
        }
    }

    #[tokio::test]
    async fn round_trip_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = SlotStateStore::new(dir.path());

        let slots = vec![test_slot()];
        store.save(&slots).await.unwrap();

        let loaded = store.load().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "test-1");
        assert_eq!(loaded[0].branch, "main");
    }

    #[tokio::test]
    async fn load_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = SlotStateStore::new(dir.path());
        let loaded = store.load().await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn state_uses_camel_case() {
        let dir = tempfile::tempdir().unwrap();
        let store = SlotStateStore::new(dir.path());

        let slots = vec![test_slot()];
        store.save(&slots).await.unwrap();

        let json = tokio::fs::read_to_string(dir.path().join("state.json"))
            .await
            .unwrap();
        assert!(json.contains("\"repoPath\""));
        assert!(json.contains("\"clonePath\""));
        assert!(json.contains("\"agentStatus\""));
        assert!(json.contains("\"createdAt\""));
        // Should NOT contain snake_case
        assert!(!json.contains("\"repo_path\""));
        assert!(!json.contains("\"clone_path\""));
    }
}

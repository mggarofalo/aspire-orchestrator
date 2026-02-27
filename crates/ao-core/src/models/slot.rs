use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::discovery::DiscoveredServices;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SlotStatus {
    Provisioning,
    Ready,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentStatus {
    None,
    Starting,
    Active,
    Blocked,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortAllocation {
    pub name: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub name: String,
    pub repo_path: String,
    pub branch: String,
    pub clone_path: String,
    pub status: SlotStatus,
    pub agent_status: AgentStatus,
    pub port_allocations: Vec<PortAllocation>,
    pub services: DiscoveredServices,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspire_started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_agent_output_at: Option<DateTime<Utc>>,
}

impl Slot {
    pub fn new(name: String, repo_path: String, branch: String, clone_path: String) -> Self {
        Self {
            name,
            repo_path,
            branch,
            clone_path,
            status: SlotStatus::Provisioning,
            agent_status: AgentStatus::None,
            port_allocations: Vec::new(),
            services: DiscoveredServices::default(),
            created_at: Utc::now(),
            aspire_started_at: None,
            agent_started_at: None,
            last_agent_output_at: None,
        }
    }

    pub fn tmux_session(&self) -> String {
        format!("ao-{}", self.name)
    }

    pub fn aspire_log_path(&self) -> PathBuf {
        PathBuf::from(&self.clone_path).join(".aspire-orchestrator-aspire.log")
    }

    pub fn agent_log_path(&self) -> PathBuf {
        PathBuf::from(&self.clone_path).join(".aspire-orchestrator-agent.log")
    }
}

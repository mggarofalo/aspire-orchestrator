use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use tokio::process::Child;
use tokio::sync::{mpsc, RwLock};

use crate::error::{OrchestratorError, Result};
use crate::models::{AgentStatus, Slot, SlotStatus};
use crate::services::{aspire, config_loader, discovery, git, tmux};

use super::log_tailer::{LogLine, LogSource};
use super::ports::PortAllocator;
use super::state::SlotStateStore;

/// Holds a running Aspire child process and its log receiver.
struct AspireProcess {
    child: Child,
    _log_rx_task: tokio::task::JoinHandle<()>,
}

pub struct SlotManager {
    slots: Arc<RwLock<Vec<Slot>>>,
    slots_directory: PathBuf,
    state_store: SlotStateStore,
    port_allocator: PortAllocator,
    /// Map from slot name to running Aspire child process.
    aspire_processes: Arc<RwLock<std::collections::HashMap<String, AspireProcess>>>,
    /// Channel for log lines from Aspire processes (fed to TUI).
    log_tx: mpsc::UnboundedSender<LogLine>,
    /// Channel for log lines from file tailers (agent logs).
    agent_tailer_handles:
        Arc<RwLock<std::collections::HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl SlotManager {
    pub fn new(slots_directory: PathBuf, log_tx: mpsc::UnboundedSender<LogLine>) -> Self {
        let state_store = SlotStateStore::new(&slots_directory);
        Self {
            slots: Arc::new(RwLock::new(Vec::new())),
            slots_directory,
            state_store,
            port_allocator: PortAllocator::new(),
            aspire_processes: Arc::new(RwLock::new(std::collections::HashMap::new())),
            log_tx,
            agent_tailer_handles: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Load persisted slots from state file.
    pub async fn load_state(&self) -> Result<()> {
        let loaded = self.state_store.load().await?;
        let mut slots = self.slots.write().await;
        *slots = loaded;
        Ok(())
    }

    /// Reconnect to existing tmux sessions, resetting status for missing sessions.
    pub async fn reconnect_existing_sessions(&self) -> Result<()> {
        let sessions = tmux::list_sessions().await.unwrap_or_default();
        let ao_sessions: std::collections::HashSet<String> = sessions
            .into_iter()
            .filter(|s| s.starts_with("ao-") && s != "ao-orchestrator")
            .map(|s| s[3..].to_string())
            .collect();

        let mut slots = self.slots.write().await;
        for slot in slots.iter_mut() {
            if !ao_sessions.contains(&slot.name) {
                slot.status = SlotStatus::Ready;
                slot.agent_status = AgentStatus::None;
            }
        }
        drop(slots);
        self.persist().await?;
        Ok(())
    }

    /// Get a snapshot of all slots (cheap clone for TUI rendering).
    pub async fn get_slots(&self) -> Vec<Slot> {
        self.slots.read().await.clone()
    }

    /// Get a single slot by name.
    pub async fn get_slot(&self, name: &str) -> Option<Slot> {
        let slots = self.slots.read().await;
        slots.iter().find(|s| s.name == name).cloned()
    }

    /// Create a new development slot.
    /// `source` can be a local filesystem path or a remote URL (https://, git@, etc.).
    pub async fn create_slot(
        &self,
        name: &str,
        source: &str,
        branch: Option<&str>,
        prompt: Option<&str>,
    ) -> Result<Slot> {
        // Validate uniqueness
        {
            let slots = self.slots.read().await;
            if slots.iter().any(|s| s.name == name) {
                return Err(OrchestratorError::SlotAlreadyExists(name.to_string()));
            }
        }

        let clone_path = self.slots_directory.join(name);
        let clone_path_str = clone_path.to_string_lossy().to_string();

        let is_remote = source.starts_with("https://")
            || source.starts_with("http://")
            || source.starts_with("git@")
            || source.starts_with("ssh://");

        let source_label = if is_remote {
            source.to_string()
        } else {
            let repo_path = Path::new(source);
            std::path::absolute(repo_path)
                .unwrap_or_else(|_| repo_path.to_path_buf())
                .to_string_lossy()
                .to_string()
        };

        let mut slot = Slot::new(
            name.to_string(),
            source_label.clone(),
            branch.unwrap_or("unknown").to_string(),
            clone_path_str.clone(),
        );

        // Clone repo
        git::clone_repo(&source_label, &clone_path).await?;

        // Checkout branch
        if let Some(branch) = branch {
            let exists = git::branch_exists(&clone_path, branch).await?;
            git::checkout(&clone_path, branch, !exists).await?;
        } else {
            let current = git::get_current_branch(&clone_path).await?;
            slot.branch = current;
        }

        // Load config and allocate ports
        match config_loader::load(&clone_path) {
            Ok(config) => {
                let allocations = self
                    .port_allocator
                    .allocate_for_overrides(&config.port_overrides)?;
                slot.port_allocations = allocations;
            }
            Err(OrchestratorError::ConfigNotFound(_)) => {
                // Config is optional
            }
            Err(e) => return Err(e),
        }

        // Create tmux session
        tmux::create_session(&slot.tmux_session(), Some(&clone_path_str)).await?;
        tmux::rename_window(&slot.tmux_session(), "0", "aspire").await?;
        tmux::create_window(&slot.tmux_session(), "claude", Some(&clone_path_str)).await?;

        // Run setup commands if config exists
        if let Ok(config) = config_loader::load(&clone_path) {
            for cmd in &config.setup {
                tmux::send_keys(&slot.tmux_session(), "aspire", cmd).await?;
                // Brief pause between setup commands
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        slot.status = SlotStatus::Ready;

        // Add to state and persist
        {
            let mut slots = self.slots.write().await;
            slots.push(slot.clone());
        }
        self.persist().await?;

        // Auto-spawn agent if prompt provided
        if let Some(p) = prompt {
            if !p.is_empty() {
                self.spawn_agent(name, Some(p), None, None).await?;
            }
        }

        Ok(slot)
    }

    /// Start the Aspire stack for a slot (direct process).
    pub async fn start_aspire(&self, name: &str) -> Result<()> {
        let slot = self
            .get_slot(name)
            .await
            .ok_or_else(|| OrchestratorError::SlotNotFound(name.to_string()))?;

        let clone_path = PathBuf::from(&slot.clone_path);
        let config = config_loader::load(&clone_path)?;

        // Update status
        self.update_slot(name, |s| s.status = SlotStatus::Starting)
            .await?;

        // Clear aspire log file
        let log_path = slot.aspire_log_path();
        tokio::fs::write(&log_path, "").await.ok();

        // Start the Aspire process
        let (child, mut log_rx) =
            aspire::start(&clone_path, &config, &slot.port_allocations).await?;

        // Spawn a task that reads log lines, writes them to file, sends to TUI,
        // and runs service discovery
        let tx = self.log_tx.clone();
        let slot_name = name.to_string();
        let slots = self.slots.clone();
        let log_path_clone = log_path.clone();

        let log_task = tokio::spawn(async move {
            let mut full_log = String::new();
            while let Some(line) = log_rx.recv().await {
                // Append to log file
                if let Ok(mut f) = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path_clone)
                    .await
                {
                    use tokio::io::AsyncWriteExt;
                    let _ = f.write_all(format!("{line}\n").as_bytes()).await;
                }

                // Send to TUI
                let _ = tx.send(LogLine {
                    slot_name: slot_name.clone(),
                    source: LogSource::Aspire,
                    line: line.clone(),
                });

                // Run service discovery on accumulated log
                full_log.push_str(&line);
                full_log.push('\n');
                let services = discovery::parse_log_content(&full_log);
                if services.dashboard_url.is_some() || !services.service_urls.is_empty() {
                    let mut slots_guard = slots.write().await;
                    if let Some(s) = slots_guard.iter_mut().find(|s| s.name == slot_name) {
                        s.services = services;
                    }
                }
            }
        });

        // Store the child process handle
        {
            let mut procs = self.aspire_processes.write().await;
            procs.insert(
                name.to_string(),
                AspireProcess {
                    child,
                    _log_rx_task: log_task,
                },
            );
        }

        // Update status
        self.update_slot(name, |s| {
            s.status = SlotStatus::Running;
            s.aspire_started_at = Some(Utc::now());
        })
        .await?;

        Ok(())
    }

    /// Stop the Aspire stack for a slot.
    pub async fn stop_aspire(&self, name: &str) -> Result<()> {
        self.update_slot(name, |s| s.status = SlotStatus::Stopping)
            .await?;

        // Kill the child process
        {
            let mut procs = self.aspire_processes.write().await;
            if let Some(mut proc) = procs.remove(name) {
                aspire::stop(&mut proc.child).await?;
                proc._log_rx_task.abort();
            }
        }

        self.update_slot(name, |s| {
            s.status = SlotStatus::Ready;
            s.aspire_started_at = None;
            s.services = Default::default();
        })
        .await?;

        Ok(())
    }

    /// Spawn a Claude agent in the slot's tmux session.
    pub async fn spawn_agent(
        &self,
        name: &str,
        prompt: Option<&str>,
        allowed_tools: Option<&str>,
        max_turns: Option<u32>,
    ) -> Result<()> {
        let slot = self
            .get_slot(name)
            .await
            .ok_or_else(|| OrchestratorError::SlotNotFound(name.to_string()))?;

        self.update_slot(name, |s| s.agent_status = AgentStatus::Starting)
            .await?;

        super::agent::spawn(&slot, prompt, allowed_tools, max_turns).await?;

        // Start tailing the agent log file
        let log_path = slot.agent_log_path();
        tokio::fs::write(&log_path, "").await.ok();
        let handle = super::log_tailer::start_tailing(
            log_path,
            name.to_string(),
            LogSource::Agent,
            self.log_tx.clone(),
        );
        {
            let mut handles = self.agent_tailer_handles.write().await;
            if let Some(old) = handles.insert(name.to_string(), handle) {
                old.abort();
            }
        }

        self.update_slot(name, |s| {
            s.agent_status = AgentStatus::Active;
            s.agent_started_at = Some(Utc::now());
        })
        .await?;

        Ok(())
    }

    /// Rebase the slot's branch onto origin/master.
    pub async fn rebase(&self, name: &str) -> Result<()> {
        let slot = self
            .get_slot(name)
            .await
            .ok_or_else(|| OrchestratorError::SlotNotFound(name.to_string()))?;
        let clone_path = PathBuf::from(&slot.clone_path);
        git::fetch(&clone_path).await?;
        git::rebase(&clone_path, "master").await?;
        Ok(())
    }

    /// Push the slot's current branch.
    pub async fn git_push(&self, name: &str) -> Result<()> {
        let slot = self
            .get_slot(name)
            .await
            .ok_or_else(|| OrchestratorError::SlotNotFound(name.to_string()))?;
        let clone_path = PathBuf::from(&slot.clone_path);
        git::push(&clone_path, &slot.branch, true).await?;
        Ok(())
    }

    /// Destroy a slot: kill processes, remove tmux session, delete clone directory.
    pub async fn destroy_slot(&self, name: &str) -> Result<()> {
        // Stop aspire if running
        {
            let mut procs = self.aspire_processes.write().await;
            if let Some(mut proc) = procs.remove(name) {
                let _ = aspire::stop(&mut proc.child).await;
                proc._log_rx_task.abort();
            }
        }

        // Stop agent log tailer
        {
            let mut handles = self.agent_tailer_handles.write().await;
            if let Some(handle) = handles.remove(name) {
                handle.abort();
            }
        }

        // Kill tmux session
        let slot = self.get_slot(name).await;
        if let Some(ref slot) = slot {
            let _ = tmux::kill_session(&slot.tmux_session()).await;
        }

        // Remove clone directory
        if let Some(ref slot) = slot {
            let clone_path = PathBuf::from(&slot.clone_path);
            if clone_path.exists() {
                tokio::fs::remove_dir_all(&clone_path).await.ok();
            }
        }

        // Release ports
        if let Some(ref slot) = slot {
            for alloc in &slot.port_allocations {
                self.port_allocator.release(alloc.port);
            }
        }

        // Remove from state
        {
            let mut slots = self.slots.write().await;
            slots.retain(|s| s.name != name);
        }
        self.persist().await?;

        Ok(())
    }

    /// Returns the workspace root (parent of the slots directory).
    pub fn workspace_root(&self) -> &Path {
        self.slots_directory
            .parent()
            .unwrap_or(&self.slots_directory)
    }

    /// Helper: update a slot by name and persist.
    async fn update_slot<F: FnOnce(&mut Slot)>(&self, name: &str, f: F) -> Result<()> {
        {
            let mut slots = self.slots.write().await;
            if let Some(slot) = slots.iter_mut().find(|s| s.name == name) {
                f(slot);
            }
        }
        self.persist().await
    }

    /// Persist current slot state to disk.
    async fn persist(&self) -> Result<()> {
        let slots = self.slots.read().await;
        self.state_store.save(&slots).await
    }
}

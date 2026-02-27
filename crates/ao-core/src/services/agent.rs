use crate::models::Slot;

const DEFAULT_ALLOWED_TOOLS: &str = "Bash,Read,Glob,Grep,Write,Edit,WebFetch,WebSearch,Task";

/// Build the system prompt appended to the Claude agent.
pub fn build_system_prompt(slot: &Slot) -> String {
    let mut lines = vec![
        format!(
            "You are working in slot '{}' on branch '{}'.",
            slot.name, slot.branch
        ),
        format!("Your working directory is {}.", slot.clone_path),
        "Before starting work, create a feature branch from the current branch using git checkout -b.".to_string(),
        "Use conventional branch names (e.g., feature/short-description, fix/short-description).".to_string(),
        "Commit your work frequently. Push when you have a meaningful set of changes.".to_string(),
    ];

    if let Some(ref dashboard) = slot.services.dashboard_url {
        lines.push(format!("Dashboard: {dashboard}"));
    }

    for (name, url) in &slot.services.service_urls {
        lines.push(format!("{name}: {url}"));
    }

    if slot.services.dashboard_url.is_some() || !slot.services.service_urls.is_empty() {
        lines.push("Use these URLs for browser testing.".to_string());
    }

    lines.join(" ")
}

/// Escape a string for shell embedding inside double quotes.
fn escape_for_shell(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Build the full `claude` CLI command string for tmux send-keys.
pub fn build_claude_command(
    slot: &Slot,
    prompt: Option<&str>,
    allowed_tools: Option<&str>,
    max_turns: Option<u32>,
) -> String {
    let tools = allowed_tools.unwrap_or(DEFAULT_ALLOWED_TOOLS);
    let mut parts = vec!["claude".to_string()];

    parts.push(format!("--allowedTools \\\"{tools}\\\""));

    let system_prompt = build_system_prompt(slot);
    parts.push(format!(
        "--append-system-prompt \\\"{}\\\"",
        escape_for_shell(&system_prompt)
    ));

    if let Some(turns) = max_turns {
        parts.push(format!("--max-turns {turns}"));
    }

    if let Some(p) = prompt {
        if !p.is_empty() {
            parts.push(format!("-p \\\"{}\\\"", escape_for_shell(p)));
        }
    }

    parts.join(" ")
}

/// Spawn a Claude agent in the slot's tmux session.
pub async fn spawn(
    slot: &Slot,
    prompt: Option<&str>,
    allowed_tools: Option<&str>,
    max_turns: Option<u32>,
) -> crate::Result<()> {
    let session = slot.tmux_session();
    let log_path = slot.agent_log_path();

    // Start pipe-pane to capture agent output
    super::tmux::start_pipe_pane(&session, "claude", &log_path.to_string_lossy()).await?;

    // Build and send the command
    let command = build_claude_command(slot, prompt, allowed_tools, max_turns);
    super::tmux::send_keys(&session, "claude", &command).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentStatus, DiscoveredServices, Slot, SlotStatus};

    fn test_slot() -> Slot {
        Slot {
            name: "test-1".into(),
            repo_path: "/repo".into(),
            branch: "main".into(),
            clone_path: "/clone/test-1".into(),
            status: SlotStatus::Running,
            agent_status: AgentStatus::None,
            port_allocations: vec![],
            services: DiscoveredServices::default(),
            created_at: chrono::Utc::now(),
            aspire_started_at: None,
            agent_started_at: None,
            last_agent_output_at: None,
        }
    }

    #[test]
    fn build_system_prompt_basic() {
        let slot = test_slot();
        let prompt = build_system_prompt(&slot);
        assert!(prompt.contains("slot 'test-1'"));
        assert!(prompt.contains("branch 'main'"));
        assert!(prompt.contains("/clone/test-1"));
    }

    #[test]
    fn build_system_prompt_with_services() {
        let mut slot = test_slot();
        slot.services.dashboard_url = Some("https://localhost:15234".into());
        slot.services
            .service_urls
            .insert("api".into(), "https://localhost:5001".into());
        let prompt = build_system_prompt(&slot);
        assert!(prompt.contains("Dashboard: https://localhost:15234"));
        assert!(prompt.contains("api: https://localhost:5001"));
        assert!(prompt.contains("Use these URLs for browser testing."));
    }

    #[test]
    fn build_command_basic() {
        let slot = test_slot();
        let cmd = build_claude_command(&slot, Some("Fix the bug"), None, None);
        assert!(cmd.starts_with("claude"));
        assert!(cmd.contains("--allowedTools"));
        assert!(cmd.contains("--append-system-prompt"));
        assert!(cmd.contains("-p \\\"Fix the bug\\\""));
    }

    #[test]
    fn build_command_with_max_turns() {
        let slot = test_slot();
        let cmd = build_claude_command(&slot, Some("Test"), None, Some(10));
        assert!(cmd.contains("--max-turns 10"));
    }

    #[test]
    fn build_command_custom_tools() {
        let slot = test_slot();
        let cmd = build_claude_command(&slot, None, Some("Read,Write"), None);
        assert!(cmd.contains("--allowedTools \\\"Read,Write\\\""));
    }

    #[test]
    fn escape_for_shell_handles_quotes() {
        assert_eq!(escape_for_shell(r#"say "hello""#), r#"say \"hello\""#);
    }

    #[test]
    fn escape_for_shell_handles_backslashes() {
        assert_eq!(escape_for_shell(r"path\to\file"), r"path\\to\\file");
    }
}

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

/// Build the `claude` CLI command as an argument list (for direct process spawning).
pub fn build_claude_command(
    slot: &Slot,
    prompt: Option<&str>,
    allowed_tools: Option<&str>,
    max_turns: Option<u32>,
) -> Vec<String> {
    let tools = allowed_tools.unwrap_or(DEFAULT_ALLOWED_TOOLS);
    let mut args = vec![
        "claude".to_string(),
        "--allowedTools".to_string(),
        tools.to_string(),
    ];

    let system_prompt = build_system_prompt(slot);
    args.push("--append-system-prompt".to_string());
    args.push(system_prompt);

    if let Some(turns) = max_turns {
        args.push("--max-turns".to_string());
        args.push(turns.to_string());
    }

    if let Some(p) = prompt {
        if !p.is_empty() {
            args.push("-p".to_string());
            args.push(p.to_string());
        }
    }

    args
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
        let args = build_claude_command(&slot, Some("Fix the bug"), None, None);
        assert_eq!(args[0], "claude");
        assert!(args.contains(&"--allowedTools".to_string()));
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"Fix the bug".to_string()));
    }

    #[test]
    fn build_command_with_max_turns() {
        let slot = test_slot();
        let args = build_claude_command(&slot, Some("Test"), None, Some(10));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"10".to_string()));
    }

    #[test]
    fn build_command_custom_tools() {
        let slot = test_slot();
        let args = build_claude_command(&slot, None, Some("Read,Write"), None);
        assert!(args.contains(&"--allowedTools".to_string()));
        assert!(args.contains(&"Read,Write".to_string()));
    }
}

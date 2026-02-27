use tokio::process::Command;

use crate::error::{OrchestratorError, Result};

async fn run_tmux(arguments: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(arguments)
        .output()
        .await
        .map_err(|e| OrchestratorError::Tmux(format!("failed to start tmux: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "no server running" is not an error for list-sessions
        if !stderr.contains("no server running") && !stderr.contains("no sessions") {
            return Err(OrchestratorError::Tmux(format!(
                "tmux command failed (exit {}): {stderr}",
                output.status.code().unwrap_or(-1)
            )));
        }
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn create_session(name: &str, working_directory: Option<&str>) -> Result<()> {
    let mut args = vec!["new-session", "-d", "-s", name];
    if let Some(dir) = working_directory {
        args.extend_from_slice(&["-c", dir]);
    }
    run_tmux(&args).await?;
    Ok(())
}

pub async fn kill_session(name: &str) -> Result<()> {
    run_tmux(&["kill-session", "-t", name]).await?;
    Ok(())
}

pub async fn list_sessions() -> Result<Vec<String>> {
    let output = run_tmux(&["ls", "-F", "#{session_name}"]).await?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.trim().to_string())
        .collect())
}

pub async fn has_session(name: &str) -> Result<bool> {
    let result = Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()
        .await
        .map_err(|e| OrchestratorError::Tmux(format!("failed to start tmux: {e}")))?;
    Ok(result.status.success())
}

pub async fn create_window(
    session: &str,
    window: &str,
    working_directory: Option<&str>,
) -> Result<()> {
    let mut args = vec!["new-window", "-t", session, "-n", window];
    if let Some(dir) = working_directory {
        args.extend_from_slice(&["-c", dir]);
    }
    run_tmux(&args).await?;
    Ok(())
}

pub async fn rename_window(session: &str, old_name: &str, new_name: &str) -> Result<()> {
    let target = format!("{session}:{old_name}");
    run_tmux(&["rename-window", "-t", &target, new_name]).await?;
    Ok(())
}

pub async fn send_keys(session: &str, window: &str, command: &str) -> Result<()> {
    let target = format!("{session}:{window}");
    run_tmux(&["send-keys", "-t", &target, command, "Enter"]).await?;
    Ok(())
}

pub async fn send_ctrl_c(session: &str, window: &str) -> Result<()> {
    let target = format!("{session}:{window}");
    run_tmux(&["send-keys", "-t", &target, "C-c"]).await?;
    Ok(())
}

pub async fn start_pipe_pane(session: &str, window: &str, log_file: &str) -> Result<()> {
    let target = format!("{session}:{window}");
    let pipe_cmd = format!("cat >> {log_file}");
    run_tmux(&["pipe-pane", "-o", "-t", &target, &pipe_cmd]).await?;
    Ok(())
}

pub async fn capture_pane(session: &str, window: &str, line_count: u32) -> Result<String> {
    let target = format!("{session}:{window}");
    let start = format!("-{line_count}");
    run_tmux(&["capture-pane", "-p", "-t", &target, "-S", &start]).await
}

pub async fn attach_session(name: &str) -> Result<std::process::ExitStatus> {
    let status = tokio::process::Command::new("tmux")
        .args(["attach-session", "-t", name])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .await
        .map_err(|e| OrchestratorError::Tmux(format!("failed to attach: {e}")))?;
    Ok(status)
}

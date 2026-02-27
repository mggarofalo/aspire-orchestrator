use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::error::{OrchestratorError, Result};
use crate::models::{OrchestratorConfig, PortAllocation};

/// Build environment variable map from port allocations.
pub fn build_env_vars(allocations: &[PortAllocation]) -> Vec<(String, String)> {
    allocations
        .iter()
        .map(|a| (a.name.clone(), a.port.to_string()))
        .collect()
}

/// Spawn the Aspire AppHost as a direct child process.
///
/// Returns the child process handle and a channel receiver that streams
/// combined stdout/stderr lines for log capture and service discovery.
pub async fn start(
    clone_path: &Path,
    config: &OrchestratorConfig,
    port_allocations: &[PortAllocation],
) -> Result<(Child, mpsc::UnboundedReceiver<String>)> {
    let apphost_path = clone_path.join(&config.apphost);

    let env_vars = build_env_vars(port_allocations);

    let mut cmd = Command::new("dotnet");
    cmd.args([
        "run",
        "--project",
        &apphost_path.to_string_lossy(),
        "--no-launch-profile",
    ]);
    cmd.current_dir(clone_path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    for (key, value) in &env_vars {
        cmd.env(key, value);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| OrchestratorError::Aspire(format!("failed to spawn dotnet: {e}")))?;

    let (tx, rx) = mpsc::unbounded_channel();

    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let tx_out = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(line);
            }
        });
    }

    // Capture stderr
    if let Some(stderr) = child.stderr.take() {
        let tx_err = tx;
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_err.send(line);
            }
        });
    }

    Ok((child, rx))
}

/// Stop the Aspire process by killing the child.
pub async fn stop(child: &mut Child) -> Result<()> {
    child
        .kill()
        .await
        .map_err(|e| OrchestratorError::Aspire(format!("failed to kill aspire process: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_env_vars_empty() {
        let vars = build_env_vars(&[]);
        assert!(vars.is_empty());
    }

    #[test]
    fn build_env_vars_with_allocations() {
        let allocs = vec![
            PortAllocation {
                name: "VITE_PORT".into(),
                port: 15000,
            },
            PortAllocation {
                name: "API_PORT".into(),
                port: 5001,
            },
        ];
        let vars = build_env_vars(&allocs);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0], ("VITE_PORT".into(), "15000".into()));
        assert_eq!(vars[1], ("API_PORT".into(), "5001".into()));
    }
}

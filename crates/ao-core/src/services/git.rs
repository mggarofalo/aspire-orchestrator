use std::path::Path;

use tokio::process::Command;

use crate::error::{OrchestratorError, Result};

async fn run_git(args: &[&str], working_directory: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = working_directory {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .await
        .map_err(|e| OrchestratorError::Git(format!("failed to run git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OrchestratorError::Git(format!(
            "git {} failed (exit {}): {stderr}",
            args.join(" "),
            output.status.code().unwrap_or(-1)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Clone a repository. `source` can be a local path or a remote URL —
/// git auto-detects local paths and applies optimizations without `--local`.
pub async fn clone_repo(source: &str, target_path: &Path) -> Result<()> {
    let target = target_path.to_string_lossy();
    run_git(&["clone", source, &target], None).await?;
    Ok(())
}

pub async fn checkout(repo_path: &Path, branch: &str, create_new: bool) -> Result<()> {
    if create_new {
        run_git(&["checkout", "-b", branch], Some(repo_path)).await?;
    } else {
        run_git(&["checkout", branch], Some(repo_path)).await?;
    }
    Ok(())
}

pub async fn fetch(repo_path: &Path) -> Result<()> {
    run_git(&["fetch", "origin"], Some(repo_path)).await?;
    Ok(())
}

pub async fn rebase(repo_path: &Path, target_branch: &str) -> Result<()> {
    let target = format!("origin/{target_branch}");
    run_git(&["rebase", &target], Some(repo_path)).await?;
    Ok(())
}

pub async fn push(repo_path: &Path, branch: &str, set_upstream: bool) -> Result<()> {
    if set_upstream {
        run_git(&["push", "-u", "origin", branch], Some(repo_path)).await?;
    } else {
        run_git(&["push", branch], Some(repo_path)).await?;
    }
    Ok(())
}

pub async fn get_current_branch(repo_path: &Path) -> Result<String> {
    run_git(&["rev-parse", "--abbrev-ref", "HEAD"], Some(repo_path)).await
}

pub async fn list_branches(repo_path: &Path) -> Result<Vec<String>> {
    let output = run_git(
        &["branch", "-a", "--format=%(refname:short)"],
        Some(repo_path),
    )
    .await?;
    let mut branches: Vec<String> = output
        .lines()
        .map(|b| b.trim())
        .filter(|b| !b.is_empty())
        .map(|b| b.strip_prefix("origin/").unwrap_or(b).to_string())
        .filter(|b| b != "HEAD")
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    branches.sort();
    Ok(branches)
}

pub async fn branch_exists(repo_path: &Path, branch: &str) -> Result<bool> {
    match run_git(&["rev-parse", "--verify", branch], Some(repo_path)).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

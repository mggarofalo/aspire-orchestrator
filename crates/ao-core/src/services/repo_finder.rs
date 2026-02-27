use std::collections::HashMap;
use std::path::Path;

use crate::models::RepoCandidate;

/// Discover repository candidates from the local filesystem and GitHub CLI.
/// Both scans run in parallel; results are merged by repo name.
pub async fn find_repos(workspace_root: &Path) -> Vec<RepoCandidate> {
    let parent = workspace_root
        .parent()
        .unwrap_or(workspace_root)
        .to_path_buf();

    let fs_handle = tokio::spawn(scan_filesystem(parent));
    let gh_handle = tokio::spawn(scan_github());

    let local = fs_handle.await.unwrap_or_default();
    let remote = gh_handle.await.unwrap_or_default();

    merge_candidates(local, remote)
}

/// Scan the parent directory for immediate child directories that contain `.git`.
async fn scan_filesystem(parent_dir: std::path::PathBuf) -> Vec<RepoCandidate> {
    tokio::task::spawn_blocking(move || {
        let mut candidates = Vec::new();
        let entries = match std::fs::read_dir(&parent_dir) {
            Ok(e) => e,
            Err(_) => return candidates,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(".git").exists() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    candidates.push(RepoCandidate {
                        name,
                        local_path: Some(path.to_string_lossy().to_string()),
                        remote_url: None,
                    });
                }
            }
        }
        candidates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        candidates
    })
    .await
    .unwrap_or_default()
}

/// Run `gh repo list` to discover GitHub repositories.
/// Returns empty Vec if `gh` is not installed or fails.
async fn scan_github() -> Vec<RepoCandidate> {
    let output = match tokio::process::Command::new("gh")
        .args([
            "repo",
            "list",
            "--json",
            "nameWithOwner,url",
            "--limit",
            "200",
            "--no-archived",
        ])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("gh CLI not available: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        tracing::warn!(
            "gh repo list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Vec::new();
    }

    #[derive(serde::Deserialize)]
    struct GhRepo {
        #[serde(rename = "nameWithOwner")]
        name_with_owner: String,
        url: String,
    }

    let repos: Vec<GhRepo> = match serde_json::from_slice(&output.stdout) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse gh output: {e}");
            return Vec::new();
        }
    };

    repos
        .into_iter()
        .map(|r| {
            let short_name = r
                .name_with_owner
                .rsplit('/')
                .next()
                .unwrap_or(&r.name_with_owner)
                .to_string();
            RepoCandidate {
                name: short_name,
                local_path: None,
                remote_url: Some(r.url),
            }
        })
        .collect()
}

/// Merge local and remote candidates. Entries with matching short names (case-insensitive)
/// are combined so they have both local_path and remote_url. Locals sort first.
fn merge_candidates(local: Vec<RepoCandidate>, remote: Vec<RepoCandidate>) -> Vec<RepoCandidate> {
    // Index locals by lowercase name
    let mut by_name: HashMap<String, RepoCandidate> = HashMap::new();
    for c in local {
        by_name.insert(c.name.to_lowercase(), c);
    }

    let mut remote_only = Vec::new();
    for r in remote {
        let key = r.name.to_lowercase();
        if let Some(existing) = by_name.get_mut(&key) {
            // Merge remote URL into local candidate
            existing.remote_url = r.remote_url;
        } else {
            remote_only.push(r);
        }
    }

    // Collect locals (now with merged remote URLs) sorted alphabetically
    let mut result: Vec<RepoCandidate> = by_name.into_values().collect();
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Append remote-only, also sorted
    remote_only.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    result.extend(remote_only);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_combines_matching_names() {
        let local = vec![RepoCandidate {
            name: "MyApp".into(),
            local_path: Some("/home/user/MyApp".into()),
            remote_url: None,
        }];
        let remote = vec![RepoCandidate {
            name: "myapp".into(),
            local_path: None,
            remote_url: Some("https://github.com/user/myapp".into()),
        }];

        let merged = merge_candidates(local, remote);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "MyApp");
        assert_eq!(merged[0].local_path.as_deref(), Some("/home/user/MyApp"));
        assert_eq!(
            merged[0].remote_url.as_deref(),
            Some("https://github.com/user/myapp")
        );
    }

    #[test]
    fn merge_locals_sort_before_remote_only() {
        let local = vec![RepoCandidate {
            name: "zebra".into(),
            local_path: Some("/home/user/zebra".into()),
            remote_url: None,
        }];
        let remote = vec![
            RepoCandidate {
                name: "alpha".into(),
                local_path: None,
                remote_url: Some("https://github.com/user/alpha".into()),
            },
            RepoCandidate {
                name: "zebra".into(),
                local_path: None,
                remote_url: Some("https://github.com/user/zebra".into()),
            },
        ];

        let merged = merge_candidates(local, remote);
        assert_eq!(merged.len(), 2);
        // Local "zebra" first (merged), then remote-only "alpha"
        assert_eq!(merged[0].name, "zebra");
        assert!(merged[0].is_local());
        assert_eq!(merged[1].name, "alpha");
        assert!(!merged[1].is_local());
    }

    #[test]
    fn merge_empty_inputs() {
        let result = merge_candidates(Vec::new(), Vec::new());
        assert!(result.is_empty());
    }

    #[test]
    fn merge_only_local() {
        let local = vec![RepoCandidate {
            name: "app".into(),
            local_path: Some("/app".into()),
            remote_url: None,
        }];
        let merged = merge_candidates(local, Vec::new());
        assert_eq!(merged.len(), 1);
        assert!(merged[0].is_local());
        assert!(merged[0].remote_url.is_none());
    }

    #[test]
    fn merge_only_remote() {
        let remote = vec![RepoCandidate {
            name: "cloud".into(),
            local_path: None,
            remote_url: Some("https://github.com/user/cloud".into()),
        }];
        let merged = merge_candidates(Vec::new(), remote);
        assert_eq!(merged.len(), 1);
        assert!(!merged[0].is_local());
    }
}

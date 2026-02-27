/// A repository candidate discovered by local filesystem scan or GitHub CLI.
#[derive(Debug, Clone)]
pub struct RepoCandidate {
    pub name: String,
    pub local_path: Option<String>,
    pub remote_url: Option<String>,
}

impl RepoCandidate {
    /// Returns the value to fill into the source field: local path if available,
    /// else remote URL, else just the name.
    pub fn source_value(&self) -> &str {
        self.local_path
            .as_deref()
            .or(self.remote_url.as_deref())
            .unwrap_or(&self.name)
    }

    /// Display label shown in the candidate list.
    pub fn display_label(&self) -> &str {
        &self.name
    }

    /// A hint about where the repo lives: local path or "(remote)".
    pub fn location_hint(&self) -> &str {
        self.local_path.as_deref().unwrap_or("(remote)")
    }

    /// Whether this candidate has a local clone.
    pub fn is_local(&self) -> bool {
        self.local_path.is_some()
    }
}

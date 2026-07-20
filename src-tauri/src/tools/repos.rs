use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const MAX_METADATA_OUTPUT: usize = 64 * 1024;
const MAX_TOP_LEVEL_ENTRIES: usize = 40;

#[derive(Clone)]
pub struct RepositoryAllowlist {
    entries: Arc<HashMap<String, PathBuf>>,
    described: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RepositoryMetadata {
    pub repo_id: String,
    pub name: String,
    pub branch: String,
    pub working_tree_status: String,
    pub top_level_entries: Vec<String>,
    pub recognized_manifests: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescribeRepoRequest {
    pub repo_id: String,
}

impl RepositoryAllowlist {
    pub fn from_environment() -> Self {
        let Some(root) = std::env::var_os("DAEMON_REPOSITORY_ROOT") else {
            return Self::empty();
        };
        let Some(demo_repo) = std::env::var_os("DAEMON_DEMO_REPOSITORY") else {
            return Self::empty();
        };
        let Ok(root) = std::fs::canonicalize(PathBuf::from(root)) else {
            return Self::empty();
        };
        let Ok(repo) = std::fs::canonicalize(PathBuf::from(demo_repo)) else {
            return Self::empty();
        };
        if !repo.starts_with(&root) {
            return Self::empty();
        }
        Self::from_entries(vec![("demo-shop".to_string(), repo)]).unwrap_or_else(|_| Self::empty())
    }

    pub fn from_entries(entries: Vec<(String, PathBuf)>) -> Result<Self, String> {
        let mut canonical_entries = HashMap::new();
        for (repo_id, path) in entries {
            if repo_id.is_empty()
                || !repo_id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            {
                return Err("Repository allowlist IDs are invalid".to_string());
            }
            let canonical = std::fs::canonicalize(&path)
                .map_err(|_| "An allowlisted repository path does not exist".to_string())?;
            if !canonical.is_dir() || !canonical.join(".git").exists() {
                return Err("Allowlisted repositories must be Git worktrees".to_string());
            }
            canonical_entries.insert(repo_id, canonical);
        }
        Ok(Self {
            entries: Arc::new(canonical_entries),
            described: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn empty() -> Self {
        Self {
            entries: Arc::new(HashMap::new()),
            described: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn describe(&self, repo_id: &str) -> Result<RepositoryMetadata, String> {
        let path = self.resolve_path(repo_id)?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(repo_id)
            .to_string();
        let branch = run_git(&path, &["branch", "--show-current"])?
            .trim()
            .to_string();
        let status = run_git(&path, &["status", "--porcelain", "--untracked-files=normal"])?;
        let entries = top_level_entries(&path)?;
        let recognized_manifests = entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.as_str(),
                    "Cargo.toml"
                        | "package.json"
                        | "bun.lock"
                        | "bun.lockb"
                        | "pnpm-lock.yaml"
                        | "yarn.lock"
                        | "go.mod"
                        | "pyproject.toml"
                        | "requirements.txt"
                )
            })
            .cloned()
            .collect();
        self.described
            .lock()
            .map_err(|_| "Repository state is unavailable".to_string())?
            .insert(repo_id.to_string());
        Ok(RepositoryMetadata {
            repo_id: repo_id.to_string(),
            name,
            branch: if branch.is_empty() {
                "detached".to_string()
            } else {
                branch
            },
            working_tree_status: if status.trim().is_empty() {
                "clean".to_string()
            } else {
                "dirty".to_string()
            },
            top_level_entries: entries,
            recognized_manifests,
        })
    }

    pub fn resolve_path(&self, repo_id: &str) -> Result<PathBuf, String> {
        let configured = self
            .entries
            .get(repo_id)
            .ok_or_else(|| "The repository ID is not allowlisted".to_string())?;
        let current = std::fs::canonicalize(configured)
            .map_err(|_| "The allowlisted repository is no longer available".to_string())?;
        if current != *configured || !current.join(".git").exists() {
            return Err("The allowlisted repository path changed or is unsafe".to_string());
        }
        Ok(current)
    }

    pub fn was_described(&self, repo_id: &str) -> bool {
        self.described
            .lock()
            .map(|described| described.contains(repo_id))
            .unwrap_or(false)
    }
}

fn top_level_entries(path: &Path) -> Result<Vec<String>, String> {
    let output = run_git(
        path,
        &["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    )?;
    let mut entries = BTreeSet::new();
    for file in output.split('\0') {
        if file.is_empty() {
            continue;
        }
        let top_level = file.split('/').next().unwrap_or(file);
        if sensitive_name(top_level) {
            continue;
        }
        entries.insert(top_level.to_string());
        if entries.len() >= MAX_TOP_LEVEL_ENTRIES {
            break;
        }
    }
    Ok(entries.into_iter().collect())
}

fn sensitive_name(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower == ".git"
        || lower == ".env"
        || lower.starts_with(".env.")
        || lower.contains("credential")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
}

fn run_git(path: &Path, arguments: &[&str]) -> Result<String, String> {
    let mut child = Command::new("git")
        .args(arguments)
        .current_dir(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Git metadata lookup could not start".to_string())?;
    let mut output = Vec::new();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Git metadata output was unavailable".to_string())?;
    stdout
        .take((MAX_METADATA_OUTPUT + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|_| "Git metadata output could not be read".to_string())?;
    let status = child
        .wait()
        .map_err(|_| "Git metadata lookup could not finish".to_string())?;
    if output.len() > MAX_METADATA_OUTPUT || !status.success() {
        return Err("Git metadata was unavailable or exceeded its bound".to_string());
    }
    String::from_utf8(output).map_err(|_| "Git metadata was not valid text".to_string())
}

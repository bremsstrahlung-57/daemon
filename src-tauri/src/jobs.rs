use crate::{
    events::{JOB_COMPLETED, JOB_FAILED, JOB_STARTED},
    state::AppState,
    storage::{new_id, now_ms, JobRecord, ProposalRecord},
    tools::policy::RunCodexTaskArguments,
};
use serde::Serialize;
use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{io::{AsyncRead, AsyncReadExt}, process::Command};

const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_SUMMARY_BYTES: usize = 16 * 1024;
const JOB_TIMEOUT: Duration = Duration::from_secs(20 * 60);

#[derive(Serialize)]
struct JobResult {
    summary: String,
    changed_files: Vec<String>,
    diff_stat: String,
    stdout: String,
    stderr: String,
    output_truncated: bool,
}

#[derive(Clone, Serialize)]
pub struct JobLifecyclePayload {
    pub job: JobRecord,
}

pub fn start_run_codex_task(
    app: &AppHandle,
    state: &State<'_, AppState>,
    proposal: &ProposalRecord,
) -> Result<JobRecord, String> {
    let arguments: RunCodexTaskArguments = serde_json::from_str(&proposal.arguments_json)
        .map_err(|_| "The approved task arguments are invalid".to_string())?;
    let repository = state.repositories.resolve_path(&arguments.repo_id)?;
    if !state.repositories.was_described(&arguments.repo_id) {
        return Err("Describe the allowlisted repository before starting a task".to_string());
    }
    ensure_clean(&repository)?;
    let worktree_root = app
        .path()
        .app_data_dir()
        .map_err(|_| "The application data directory is unavailable".to_string())?
        .join("worktrees");
    std::fs::create_dir_all(&worktree_root)
        .map_err(|_| "Unable to prepare the isolated worktree directory".to_string())?;
    let worktree_root = std::fs::canonicalize(worktree_root)
        .map_err(|_| "Unable to secure the isolated worktree directory".to_string())?;
    let job_id = new_id();
    let workspace = worktree_root.join(&job_id);
    if !workspace.starts_with(&worktree_root) {
        return Err("The isolated worktree path is unsafe".to_string());
    }
    let job = JobRecord {
        id: job_id.clone(),
        proposal_id: proposal.id.clone(),
        kind: "run_codex_task".to_string(),
        status: "preparing".to_string(),
        workspace_path: Some(workspace.to_string_lossy().to_string()),
        started_at: Some(now_ms()),
        completed_at: None,
        result_json: None,
        error_message: None,
    };
    {
        let storage = state.storage.lock().map_err(|_| "Local storage is unavailable".to_string())?;
        if let Some(existing) = storage
            .job_by_proposal(&proposal.id)
            .map_err(|_| "Unable to check the task state".to_string())?
        {
            return Ok(existing);
        }
        if storage.active_job().map_err(|_| "Unable to check active tasks".to_string())?.is_some() {
            return Err("Only one isolated coding task can run at a time".to_string());
        }
        storage.insert_job(&job).map_err(|_| "Unable to save the task before it starts".to_string())?;
        storage.append_audit("job", &job.id, "prepared", None).map_err(|_| "Unable to write the job audit".to_string())?;
    }
    let app = app.clone();
    let task_job = job.clone();
    tauri::async_runtime::spawn(async move {
        run_job(app, task_job, repository, workspace, worktree_root, arguments).await;
    });
    Ok(job)
}

async fn run_job(
    app: AppHandle,
    job: JobRecord,
    repository: PathBuf,
    workspace: PathBuf,
    worktree_root: PathBuf,
    arguments: RunCodexTaskArguments,
) {
    let branch = format!("daemon/{}", job.id);
    let workspace_text = workspace.to_string_lossy().to_string();
    let branch_args = ["worktree", "add", "-b", branch.as_str(), &workspace_text, "HEAD"];
    if let Err(error) = run_git(&repository, &branch_args).await {
        finish_failure(&app, &job.id, error).await;
        return;
    }
    {
        let state = app.state::<AppState>();
        let storage = match state.storage.lock() { Ok(storage) => storage, Err(_) => return };
        let Ok(running_job) = storage.mark_job_running(&job.id) else {
            return;
        };
        let _ = storage.append_audit("job", &job.id, "started", None);
        let _ = app.emit(JOB_STARTED, JobLifecyclePayload { job: running_job });
    }
    let result_path = worktree_root.join(format!("{}.last-message.txt", job.id));
    let outcome = run_codex(&workspace, &result_path, &arguments).await;
    let (status, result_json, error) = match outcome {
        Ok((stdout, stderr, truncated)) => {
            let summary = read_bounded_file(&result_path, MAX_SUMMARY_BYTES).await.unwrap_or_default();
            let changed_files = run_git(&workspace, &["diff", "--name-only"])
                .await
                .unwrap_or_default()
                .lines()
                .take(100)
                .map(str::to_string)
                .collect();
            let diff_stat = run_git(&workspace, &["diff", "--stat"])
                .await
                .unwrap_or_default();
            let result = JobResult { summary, changed_files, diff_stat, stdout, stderr, output_truncated: truncated };
            ("completed", serde_json::to_string(&result).ok(), None)
        }
        Err(error) => ("failed", None, Some(error)),
    };
    let state = app.state::<AppState>();
    if let Ok(mut storage) = state.storage.lock() {
        if let Ok(finished_job) = storage.finish_job(&job.id, status, result_json.as_deref(), error.as_deref()) {
            let event = if status == "completed" { JOB_COMPLETED } else { JOB_FAILED };
            let _ = app.emit(event, JobLifecyclePayload { job: finished_job });
        }
        let _ = storage.append_audit("job", &job.id, status, None);
    };
}

async fn finish_failure(app: &AppHandle, job_id: &str, error: String) {
    let state = app.state::<AppState>();
    if let Ok(mut storage) = state.storage.lock() {
        if let Ok(finished_job) = storage.finish_job(job_id, "failed", None, Some(&error)) {
            let _ = app.emit(JOB_FAILED, JobLifecyclePayload { job: finished_job });
        }
        let _ = storage.append_audit("job", job_id, "failed", None);
    };
}

async fn run_codex(
    workspace: &Path,
    result_path: &Path,
    arguments: &RunCodexTaskArguments,
) -> Result<(String, String, bool), String> {
    let executable = std::env::var_os("DAEMON_CODEX_EXECUTABLE").unwrap_or_else(|| {
        let bundled = std::path::PathBuf::from(r"C:\Program Files\nodejs\codex.cmd");
        if bundled.is_file() { bundled.into_os_string() } else { "codex".into() }
    });
    let prompt = format!(
        "Objective: {}\nAcceptance criteria: {}\nLikely files: {}\n\nWork only in this isolated repository worktree. Do not access network resources or modify files outside it. Return a concise final summary.",
        arguments.objective,
        arguments.acceptance_criteria,
        arguments.likely_files.join(", "),
    );
    let mut child = Command::new(executable)
        .arg("exec")
        .arg("--cd")
        .arg(workspace)
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--ask-for-approval")
        .arg("never")
        .arg("--json")
        .arg("--output-last-message")
        .arg(result_path)
        .arg("-c")
        .arg("web_search=\"cached\"")
        .arg(prompt)
        .current_dir(workspace)
        .env_remove("OPENAI_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Codex could not start from the Rust-owned executable setting".to_string())?;
    let stdout = child.stdout.take().ok_or_else(|| "Codex stdout was unavailable".to_string())?;
    let stderr = child.stderr.take().ok_or_else(|| "Codex stderr was unavailable".to_string())?;
    let stdout_task = tokio::spawn(read_bounded(stdout, MAX_OUTPUT_BYTES));
    let stderr_task = tokio::spawn(read_bounded(stderr, MAX_OUTPUT_BYTES));
    let status = match tokio::time::timeout(JOB_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => return Err("Codex did not finish cleanly".to_string()),
        Err(_) => {
            let _ = child.kill().await;
            return Err("Codex exceeded the 20 minute task timeout".to_string());
        }
    };
    let (stdout, stdout_cut) = stdout_task.await.unwrap_or_default();
    let (stderr, stderr_cut) = stderr_task.await.unwrap_or_default();
    if !status.success() {
        return Err(format!("Codex exited without completing the task: {}", limit(&stderr, 600)));
    }
    Ok((stdout, stderr, stdout_cut || stderr_cut))
}

fn ensure_clean(repository: &Path) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repository)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "Unable to inspect the allowlisted repository state".to_string())?;
    if !output.status.success() || !output.stdout.is_empty() {
        return Err("The allowlisted repository must be clean before an isolated task starts".to_string());
    }
    Ok(())
}

async fn run_git(path: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|_| "The isolated Git worktree command could not start".to_string())?;
    if !output.status.success() || output.stdout.len() > MAX_OUTPUT_BYTES {
        return Err("The isolated Git worktree command failed".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn read_bounded<R: AsyncRead + Unpin>(mut reader: R, maximum: usize) -> (String, bool) {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer).await.unwrap_or(0);
        if read == 0 { break; }
        let remaining = maximum.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..read.min(remaining)]);
        truncated |= read > remaining;
    }
    (String::from_utf8_lossy(&bytes).into_owned(), truncated)
}

async fn read_bounded_file(path: &Path, maximum: usize) -> Option<String> {
    let bytes = tokio::fs::read(path).await.ok()?;
    Some(String::from_utf8_lossy(&bytes[..bytes.len().min(maximum)]).into_owned())
}

fn limit(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

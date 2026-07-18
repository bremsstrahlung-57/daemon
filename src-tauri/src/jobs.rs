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

const MAX_RESULT_BYTES: usize = 64 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 4 * 1024;
const DEFAULT_JOB_TIMEOUT_SECONDS: u64 = 3 * 60;
const MAX_JOB_TIMEOUT_SECONDS: u64 = 60 * 60;

#[derive(Serialize)]
struct JobResult {
    summary: String,
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
    let job = JobRecord {
        id: new_id(),
        proposal_id: proposal.id.clone(),
        kind: "run_codex_task".to_string(),
        status: "preparing".to_string(),
        workspace_path: Some(repository.to_string_lossy().to_string()),
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
            return Err("Only one Codex task can run at a time".to_string());
        }
        storage.insert_job(&job).map_err(|_| "Unable to save the task before it starts".to_string())?;
        storage.append_audit("job", &job.id, "prepared", None).map_err(|_| "Unable to write the job audit".to_string())?;
    }
    let app = app.clone();
    let task_job = job.clone();
    tauri::async_runtime::spawn(async move {
        run_job(app, task_job, repository, arguments).await;
    });
    Ok(job)
}

async fn run_job(
    app: AppHandle,
    job: JobRecord,
    repository: PathBuf,
    arguments: RunCodexTaskArguments,
) {
    let initial_snapshot = match repository_snapshot(&repository).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            finish_failure(&app, &job.id, error).await;
            return;
        }
    };
    {
        let state = app.state::<AppState>();
        let storage = match state.storage.lock() { Ok(storage) => storage, Err(_) => return };
        let Ok(running_job) = storage.mark_job_running(&job.id) else {
            return;
        };
        let _ = storage.append_audit("job", &job.id, "started", None);
        let _ = app.emit(JOB_STARTED, JobLifecyclePayload { job: running_job });
    }
    let (status, result_json, error) = match run_codex(&repository, &arguments.objective).await {
        Ok(summary) => match repository_snapshot(&repository).await {
            Ok(snapshot) if snapshot == initial_snapshot => (
                "completed",
                serde_json::to_string(&JobResult { summary }).ok(),
                None,
            ),
            Ok(_) => (
                "failed",
                None,
                Some("The allowlisted repository changed during the read-only Codex run".to_string()),
            ),
            Err(error) => ("failed", None, Some(error)),
        },
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

async fn run_codex(repository: &Path, objective: &str) -> Result<String, String> {
    let mut command = Command::new("codex");
    command
        .arg("exec")
        .arg("--sandbox")
        .arg("read-only")
        .arg(objective)
        .current_dir(repository);
    run_codex_command(command, job_timeout()).await
}

async fn run_codex_command(
    mut command: Command,
    timeout: Duration,
) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Codex could not start from the system PATH".to_string())?;
    let stdout = child.stdout.take().ok_or_else(|| "Codex stdout was unavailable".to_string())?;
    let stderr = child.stderr.take().ok_or_else(|| "Codex stderr was unavailable".to_string())?;
    let stdout_task = tokio::spawn(read_bounded(stdout, MAX_RESULT_BYTES));
    let stderr_task = tokio::spawn(read_bounded(stderr, MAX_DIAGNOSTIC_BYTES));
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => Some(status),
        Ok(Err(_)) => return Err("Codex did not finish cleanly".to_string()),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            None
        }
    };
    let (stdout, _) = stdout_task.await.unwrap_or_default();
    let (stderr, _) = stderr_task.await.unwrap_or_default();
    let Some(status) = status else {
        return Err(format!(
            "Codex exceeded the {} second read-only timeout",
            timeout.as_secs(),
        ));
    };
    if !status.success() {
        let code = status.code().map_or_else(|| "unknown".to_string(), |code| code.to_string());
        let excerpt = limit(stderr.trim(), 600);
        return Err(if excerpt.is_empty() {
            format!("Codex exited with code {code}")
        } else {
            format!("Codex exited with code {code}: {excerpt}")
        });
    }
    Ok(stdout)
}

fn job_timeout() -> Duration {
    let seconds = std::env::var("DAEMON_CODEX_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (1..=MAX_JOB_TIMEOUT_SECONDS).contains(seconds))
        .unwrap_or(DEFAULT_JOB_TIMEOUT_SECONDS);
    Duration::from_secs(seconds)
}

async fn repository_snapshot(repository: &Path) -> Result<String, String> {
    let status = run_git(repository, &["status", "--porcelain=v1", "--untracked-files=all"]).await?;
    let diff = run_git(repository, &["diff", "--no-ext-diff", "--binary", "HEAD"]).await?;
    Ok(format!("{status}\n--daemon-diff-boundary--\n{diff}"))
}

async fn run_git(path: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|_| "The repository state check could not start".to_string())?;
    if !output.status.success() || output.stdout.len() > MAX_RESULT_BYTES {
        return Err("The repository state check failed".to_string());
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

fn limit(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_a_bounded_stderr_excerpt_on_nonzero_exit() {
        let mut command = Command::new("cmd");
        command.args(["/C", "echo synthetic failure 1>&2 & exit /B 7"]);

        let error = run_codex_command(command, Duration::from_secs(1))
            .await
            .expect_err("nonzero child should fail the job");

        assert!(error.contains("code 7"));
        assert!(error.contains("synthetic failure"));
    }

    #[tokio::test]
    async fn kills_a_child_that_exceeds_the_timeout() {
        let mut command = Command::new("cmd");
        command.args(["/C", "ping -n 10 127.0.0.1 > NUL"]);

        let error = run_codex_command(command, Duration::from_secs(1))
            .await
            .expect_err("long-running child should time out");

        assert_eq!(error, "Codex exceeded the 1 second read-only timeout");
    }
}

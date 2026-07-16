use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    CreateNote,
    DescribeRepo,
    RunCodexTask,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    Automatic,
    OncePerRepository,
    EveryTask,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataExposure {
    LocalOnly,
    BoundedRepositoryMetadata,
    RepositoryContent,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ToolPolicy {
    pub approval_policy: ApprovalPolicy,
    pub data_exposure: DataExposure,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedToolCall {
    pub tool_name: ToolName,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateNoteArguments {
    pub content: String,
    #[serde(default)]
    pub due_date: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DescribeRepoArguments {
    pub repo_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunCodexTaskArguments {
    pub repo_id: String,
    pub objective: String,
    pub acceptance_criteria: String,
    #[serde(default)]
    pub likely_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "tool_name", content = "arguments")]
pub enum ToolArguments {
    #[serde(rename = "create_note")]
    CreateNote(CreateNoteArguments),
    #[serde(rename = "describe_repo")]
    DescribeRepo(DescribeRepoArguments),
    #[serde(rename = "run_codex_task")]
    RunCodexTask(RunCodexTaskArguments),
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidatedToolCall {
    pub tool_name: ToolName,
    pub arguments: ToolArguments,
    pub policy: ToolPolicy,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ToolRegistry;

impl ToolRegistry {
    pub fn validate(&self, call: ProposedToolCall) -> Result<ValidatedToolCall, String> {
        let (arguments, policy) = match call.tool_name {
            ToolName::CreateNote => {
                let arguments = parse::<CreateNoteArguments>(call.arguments, "create_note")?;
                validate_note(&arguments)?;
                (
                    ToolArguments::CreateNote(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::Automatic,
                        data_exposure: DataExposure::LocalOnly,
                    },
                )
            }
            ToolName::DescribeRepo => {
                let arguments = parse::<DescribeRepoArguments>(call.arguments, "describe_repo")?;
                validate_repo_id(&arguments.repo_id)?;
                (
                    ToolArguments::DescribeRepo(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::OncePerRepository,
                        data_exposure: DataExposure::BoundedRepositoryMetadata,
                    },
                )
            }
            ToolName::RunCodexTask => {
                let arguments = parse::<RunCodexTaskArguments>(call.arguments, "run_codex_task")?;
                validate_codex_task(&arguments)?;
                (
                    ToolArguments::RunCodexTask(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::EveryTask,
                        data_exposure: DataExposure::RepositoryContent,
                    },
                )
            }
        };

        Ok(ValidatedToolCall {
            tool_name: call.tool_name,
            arguments,
            policy,
        })
    }
}

fn parse<T>(arguments: serde_json::Value, tool_name: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments)
        .map_err(|_| format!("Invalid arguments for {tool_name}"))
}

fn validate_note(arguments: &CreateNoteArguments) -> Result<(), String> {
    let content = arguments.content.trim();
    if content.is_empty() || content.chars().count() > 500 {
        return Err("Note content must contain between 1 and 500 characters".to_string());
    }
    if arguments
        .due_date
        .as_deref()
        .is_some_and(|due_date| due_date.chars().count() > 32)
    {
        return Err("Note due dates must be at most 32 characters".to_string());
    }
    Ok(())
}

fn validate_repo_id(repo_id: &str) -> Result<(), String> {
    if repo_id.is_empty()
        || repo_id.chars().count() > 80
        || !repo_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("Repository IDs must be stable allowlist IDs, not filesystem paths".to_string());
    }
    Ok(())
}

fn validate_codex_task(arguments: &RunCodexTaskArguments) -> Result<(), String> {
    validate_repo_id(&arguments.repo_id)?;
    validate_bounded_text(&arguments.objective, 1000, "objective")?;
    validate_bounded_text(
        &arguments.acceptance_criteria,
        2000,
        "acceptance criteria",
    )?;
    if arguments.likely_files.len() > 12 {
        return Err("A Codex task may name at most 12 likely files".to_string());
    }
    for file in &arguments.likely_files {
        if file.is_empty()
            || file.chars().count() > 200
            || file.starts_with('/')
            || file.starts_with('\\')
            || file.as_bytes().get(1) == Some(&b':')
        {
            return Err("Likely files must be bounded relative paths".to_string());
        }
    }
    Ok(())
}

fn validate_bounded_text(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.chars().count() > maximum {
        return Err(format!("Codex {label} must contain between 1 and {maximum} characters"));
    }
    Ok(())
}

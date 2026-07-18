use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    CreateNote,
    CreateMemory,
    SearchMemories,
    SearchNotes,
    ShowMascotReaction,
    CaptureScreen,
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
pub struct CreateMemoryArguments {
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchLocalArguments {
    pub query: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MascotReaction {
    Happy,
    NotHappy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShowMascotReactionArguments {
    pub reaction: MascotReaction,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureScreenArguments {}

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
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "tool_name", content = "arguments")]
pub enum ToolArguments {
    #[serde(rename = "create_note")]
    CreateNote(CreateNoteArguments),
    #[serde(rename = "create_memory")]
    CreateMemory(CreateMemoryArguments),
    #[serde(rename = "search_memories")]
    SearchMemories(SearchLocalArguments),
    #[serde(rename = "search_notes")]
    SearchNotes(SearchLocalArguments),
    #[serde(rename = "show_mascot_reaction")]
    ShowMascotReaction(ShowMascotReactionArguments),
    #[serde(rename = "capture_screen")]
    CaptureScreen(CaptureScreenArguments),
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
            ToolName::CreateMemory => {
                let arguments = parse::<CreateMemoryArguments>(call.arguments, "create_memory")?;
                validate_memory(&arguments)?;
                (
                    ToolArguments::CreateMemory(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::Automatic,
                        data_exposure: DataExposure::LocalOnly,
                    },
                )
            }
            ToolName::SearchMemories => {
                let arguments = parse::<SearchLocalArguments>(call.arguments, "search_memories")?;
                validate_search(&arguments)?;
                (
                    ToolArguments::SearchMemories(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::Automatic,
                        data_exposure: DataExposure::LocalOnly,
                    },
                )
            }
            ToolName::SearchNotes => {
                let arguments = parse::<SearchLocalArguments>(call.arguments, "search_notes")?;
                validate_search(&arguments)?;
                (
                    ToolArguments::SearchNotes(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::Automatic,
                        data_exposure: DataExposure::LocalOnly,
                    },
                )
            }
            ToolName::ShowMascotReaction => {
                let arguments = parse::<ShowMascotReactionArguments>(
                    call.arguments,
                    "show_mascot_reaction",
                )?;
                (
                    ToolArguments::ShowMascotReaction(arguments),
                    ToolPolicy {
                        approval_policy: ApprovalPolicy::Automatic,
                        data_exposure: DataExposure::LocalOnly,
                    },
                )
            }
            ToolName::CaptureScreen => {
                let arguments = parse::<CaptureScreenArguments>(call.arguments, "capture_screen")?;
                (
                    ToolArguments::CaptureScreen(arguments),
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
        .is_some_and(|due_date| due_date.trim().is_empty() || due_date.chars().count() > 32)
    {
        return Err("Note due dates must be at most 32 characters".to_string());
    }
    Ok(())
}

fn validate_memory(arguments: &CreateMemoryArguments) -> Result<(), String> {
    let content = arguments.content.trim();
    if content.is_empty() || content.chars().count() > 500 {
        return Err("Memory content must contain between 1 and 500 characters".to_string());
    }
    Ok(())
}

fn validate_search(arguments: &SearchLocalArguments) -> Result<(), String> {
    if arguments.query.chars().count() > 200 {
        return Err("Local search queries must contain at most 200 characters".to_string());
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
    Ok(())
}

fn validate_bounded_text(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.chars().count() > maximum {
        return Err(format!("Codex {label} must contain between 1 and {maximum} characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_codex_task_accepts_only_repository_and_objective() {
        let registry = ToolRegistry;
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::RunCodexTask,
                arguments: serde_json::json!({
                    "repo_id": "demo_shop",
                    "objective": "Explain the application architecture",
                }),
            })
            .is_ok());
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::RunCodexTask,
                arguments: serde_json::json!({
                    "repo_id": "demo_shop",
                    "objective": "Explain the application architecture",
                    "acceptance_criteria": "This must be rejected",
                }),
            })
            .is_err());
    }

    #[test]
    fn local_memory_and_lookup_calls_are_strictly_validated() {
        let registry = ToolRegistry;
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::CreateNote,
                arguments: serde_json::json!({
                    "content": "Fix the login bug",
                    "source_message_id": "forged",
                }),
            })
            .is_err());
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::CreateMemory,
                arguments: serde_json::json!({ "content": "My favorite color is purple" }),
            })
            .is_ok());
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::CreateMemory,
                arguments: serde_json::json!({
                    "content": "My favorite color is purple",
                    "source_message_id": "forged",
                }),
            })
            .is_err());
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::SearchNotes,
                arguments: serde_json::json!({ "query": "" }),
            })
            .is_ok());
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::CaptureScreen,
                arguments: serde_json::json!({}),
            })
            .is_ok());
        assert!(registry
            .validate(ProposedToolCall {
                tool_name: ToolName::CaptureScreen,
                arguments: serde_json::json!({ "source": "forged" }),
            })
            .is_err());
    }
}

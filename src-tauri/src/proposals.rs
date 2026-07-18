use crate::{
    events::{PROPOSAL_CREATED, PROPOSAL_RESOLVED},
    state::AppState,
    storage::{new_id, now_ms, ProposalRecord},
    tools::{ProposedToolCall, ToolArguments, ToolName},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

const PROPOSAL_TTL_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRunCodexProposalRequest {
    pub conversation_id: String,
    pub repo_id: String,
    pub objective: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveProposalRequest {
    pub proposal_id: String,
    pub arguments_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProposalCreatedPayload {
    pub proposal_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProposalResolvedPayload {
    pub proposal_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProposalApproval {
    pub proposal: ProposalRecord,
    pub duplicate: bool,
}

pub fn create_run_codex_proposal(
    app: &AppHandle,
    state: &State<'_, AppState>,
    request: CreateRunCodexProposalRequest,
) -> Result<ProposalRecord, String> {
    if !state.repositories.was_described(&request.repo_id) {
        return Err("Describe the allowlisted repository before proposing a Codex task".to_string());
    }
    let validated = state.tools.validate(ProposedToolCall {
        tool_name: ToolName::RunCodexTask,
        arguments: serde_json::json!({
            "repo_id": request.repo_id,
            "objective": request.objective,
        }),
    })?;
    let ToolArguments::RunCodexTask(arguments) = validated.arguments else {
        return Err("The task registry returned the wrong argument type".to_string());
    };
    let arguments_json = serde_json::to_string(&arguments)
        .map_err(|_| "Unable to persist task arguments".to_string())?;
    let arguments_hash = hash_arguments(&arguments_json);
    let created_at = now_ms();
    let proposal = ProposalRecord {
        id: new_id(),
        conversation_id: request.conversation_id,
        tool_name: "run_codex_task".to_string(),
        arguments_json,
        arguments_hash,
        preview: format!("Want me to {} in {}?", arguments.objective, arguments.repo_id),
        approval_policy: "every_task".to_string(),
        status: "awaiting_approval".to_string(),
        provider_context_json: Some(
            serde_json::json!({
                "repo_id": arguments.repo_id,
                "describe_repo_resolved": true,
            })
            .to_string(),
        ),
        created_at,
        expires_at: Some(created_at + PROPOSAL_TTL_MS),
        resolved_at: None,
    };
    let storage = state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?;
    storage
        .insert_proposal(&proposal)
        .map_err(|_| "Unable to save the task proposal".to_string())?;
    storage
        .append_audit("proposal", &proposal.id, "created", None)
        .map_err(|_| "Unable to write the proposal audit".to_string())?;
    let _ = app.emit(
        PROPOSAL_CREATED,
        ProposalCreatedPayload {
            proposal_id: proposal.id.clone(),
        },
    );
    Ok(proposal)
}

pub fn approve_proposal(
    app: &AppHandle,
    state: &State<'_, AppState>,
    request: ResolveProposalRequest,
) -> Result<ProposalApproval, String> {
    let mut storage = state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?;
    let (proposal, duplicate) = storage.approve_proposal(&request.proposal_id, &request.arguments_hash)?;
    storage
        .append_audit(
            "proposal",
            &proposal.id,
            if duplicate { "approval-duplicate" } else { "approved" },
            None,
        )
        .map_err(|_| "Unable to write the proposal audit".to_string())?;
    let _ = app.emit(
        PROPOSAL_RESOLVED,
        ProposalResolvedPayload {
            proposal_id: proposal.id.clone(),
            status: proposal.status.clone(),
        },
    );
    Ok(ProposalApproval { proposal, duplicate })
}

pub fn deny_proposal(
    app: &AppHandle,
    state: &State<'_, AppState>,
    request: ResolveProposalRequest,
) -> Result<ProposalRecord, String> {
    let mut storage = state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?;
    let proposal = storage.deny_proposal(&request.proposal_id, &request.arguments_hash)?;
    storage
        .append_audit("proposal", &proposal.id, "denied", None)
        .map_err(|_| "Unable to write the proposal audit".to_string())?;
    let _ = app.emit(
        PROPOSAL_RESOLVED,
        ProposalResolvedPayload {
            proposal_id: proposal.id.clone(),
            status: proposal.status.clone(),
        },
    );
    Ok(proposal)
}

pub fn pending_proposals(state: &State<'_, AppState>) -> Result<Vec<ProposalRecord>, String> {
    state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?
        .pending_proposals()
        .map_err(|_| "Unable to load pending proposals".to_string())
}

fn hash_arguments(arguments_json: &str) -> String {
    Sha256::digest(arguments_json.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

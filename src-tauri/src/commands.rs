use crate::{
    openai::{turns::submit_turn, ModelTurnRequest},
    jobs::start_run_codex_task,
    proposals::{
        approve_proposal as resolve_approval, create_run_codex_proposal as create_proposal,
        deny_proposal as resolve_denial, pending_proposals as load_pending_proposals,
        CreateRunCodexProposalRequest, ProposalApproval, ResolveProposalRequest,
    },
    providers::{self, ProviderIdRequest, ProviderView, SaveProviderRequest},
    state::AppState,
    tools::{DescribeRepoRequest, ProposedToolCall, RepositoryMetadata, ValidatedToolCall},
};
use serde::{Deserialize, Serialize};
use tauri::{menu::{ContextMenu, Menu, MenuItemBuilder}, AppHandle, State, Window};

#[derive(Clone, Debug, Serialize)]
pub struct AuthStatus {
    pub configured: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteIdRequest {
    pub note_id: String,
}

#[tauri::command]
pub fn save_api_key(state: State<'_, AppState>, api_key: String) -> Result<AuthStatus, String> {
    state.secrets.save_api_key(&api_key)?;
    Ok(AuthStatus { configured: true })
}

#[tauri::command]
pub fn get_auth_status(state: State<'_, AppState>) -> AuthStatus {
    AuthStatus {
        configured: state.secrets.is_configured(),
    }
}

#[tauri::command]
pub fn disconnect_api_key(state: State<'_, AppState>) -> Result<AuthStatus, String> {
    state.secrets.delete_api_key()?;
    Ok(AuthStatus { configured: false })
}

#[tauri::command]
pub fn show_toolbox_menu(app: AppHandle, window: Window) -> Result<(), String> {
    let settings = MenuItemBuilder::with_id("daemon_toolbox_settings", "Settings").build(&app)
        .map_err(|_| "Unable to open the toolbox".to_string())?;
    let about = MenuItemBuilder::with_id("daemon_toolbox_about", "About").build(&app)
        .map_err(|_| "Unable to open the toolbox".to_string())?;
    let dismiss = MenuItemBuilder::with_id("daemon_toolbox_dismiss", "Dismiss").build(&app)
        .map_err(|_| "Unable to open the toolbox".to_string())?;
    let quit = MenuItemBuilder::with_id("daemon_toolbox_quit", "Quit").build(&app)
        .map_err(|_| "Unable to open the toolbox".to_string())?;
    let menu = Menu::with_items(&app, &[&settings, &about, &dismiss, &quit])
        .map_err(|_| "Unable to open the toolbox".to_string())?;
    menu.popup(window)
        .map_err(|_| "Unable to open the toolbox".to_string())
}

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderView>, String> {
    providers::providers(&state)
}

#[tauri::command]
pub fn save_provider(state: State<'_, AppState>, request: SaveProviderRequest) -> Result<ProviderView, String> {
    providers::save_provider(&state, request)
}

#[tauri::command]
pub fn select_provider(state: State<'_, AppState>, request: ProviderIdRequest) -> Result<ProviderView, String> {
    providers::select_provider(&state, request)
}

#[tauri::command]
pub fn delete_provider_key(state: State<'_, AppState>, request: ProviderIdRequest) -> Result<(), String> {
    providers::delete_provider_key(&state, request)
}

#[tauri::command]
pub fn delete_provider(state: State<'_, AppState>, request: ProviderIdRequest) -> Result<bool, String> {
    providers::delete_provider(&state, request)
}

#[tauri::command]
pub fn undo_note(state: State<'_, AppState>, request: NoteIdRequest) -> Result<bool, String> {
    if request.note_id.trim().is_empty() {
        return Err("A note ID is required".to_string());
    }
    state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?
        .soft_delete_note_with_audit(&request.note_id)
        .map_err(|_| "Unable to undo the local note".to_string())
}

#[tauri::command]
pub async fn create_model_response(
    state: State<'_, AppState>,
    request: ModelTurnRequest,
) -> Result<crate::openai::ResponsesApiResponse, String> {
    state.openai.create_response(request).await
}

#[tauri::command]
pub fn validate_tool_call(
    state: State<'_, AppState>,
    request: ProposedToolCall,
) -> Result<ValidatedToolCall, String> {
    state.tools.validate(request)
}

#[tauri::command]
pub fn describe_repo(
    state: State<'_, AppState>,
    request: DescribeRepoRequest,
) -> Result<RepositoryMetadata, String> {
    state.repositories.describe(&request.repo_id)
}

#[tauri::command]
pub fn create_run_codex_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateRunCodexProposalRequest,
) -> Result<crate::storage::ProposalRecord, String> {
    create_proposal(&app, &state, request)
}

#[tauri::command]
pub fn approve_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ResolveProposalRequest,
) -> Result<ProposalApproval, String> {
    let approval = resolve_approval(&app, &state, request)?;
    if !approval.duplicate {
        start_run_codex_task(&app, &state, &approval.proposal)?;
    }
    Ok(approval)
}

#[tauri::command]
pub fn deny_proposal(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ResolveProposalRequest,
) -> Result<crate::storage::ProposalRecord, String> {
    resolve_denial(&app, &state, request)
}

#[tauri::command]
pub fn pending_proposals(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::ProposalRecord>, String> {
    load_pending_proposals(&state)
}

#[tauri::command]
pub async fn submit_conversation_turn(
    app: AppHandle,
    state: State<'_, AppState>,
    request: crate::openai::turns::SubmitTurnRequest,
) -> Result<crate::openai::turns::TurnResult, String> {
    submit_turn(app, state, request).await
}

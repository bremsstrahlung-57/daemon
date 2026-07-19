use crate::{
    openai::{turns::submit_turn, ModelTurnRequest},
    jobs::start_run_codex_task,
    proposals::{
        approve_proposal as resolve_approval, create_run_codex_proposal as create_proposal,
        deny_proposal as resolve_denial, pending_proposals as load_pending_proposals,
        CreateRunCodexProposalRequest, ProposalApproval, ResolveProposalRequest,
    },
    providers::{self, ProviderIdRequest, ProviderView, SaveProviderRequest},
    screen_aware::{capture_and_store, validate_settings},
    state::AppState,
    tools::{DescribeRepoRequest, ProposedToolCall, RepositoryMetadata, ValidatedToolCall},
};
use serde::{Deserialize, Serialize};
use tauri::{menu::{ContextMenu, Menu, MenuItemBuilder}, AppHandle, Manager, State, Window};

#[derive(Clone, Debug, Serialize)]
pub struct AuthStatus {
    pub configured: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteIdRequest {
    pub note_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveScreenAwareSettingsRequest {
    pub interval_seconds: Option<i64>,
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
pub fn get_screen_aware_settings(
    state: State<'_, AppState>,
) -> Result<crate::storage::ScreenAwareSettingsRecord, String> {
    state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?
        .screen_aware_settings()
        .map_err(|_| "Unable to load Screen Aware settings".to_string())
}

#[tauri::command]
pub fn get_screen_aware_model_status(state: State<'_, AppState>) -> bool {
    state.screen_aware.is_model_downloading()
}

#[tauri::command]
pub fn save_screen_aware_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SaveScreenAwareSettingsRequest,
) -> Result<crate::storage::ScreenAwareSettingsRecord, String> {
    validate_settings(request.interval_seconds)?;
    let settings = state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?
        .save_screen_aware_settings(request.interval_seconds)
        .map_err(|_| "Unable to save Screen Aware settings".to_string())?;
    state
        .screen_aware
        .restart_monitor(app.clone(), settings.interval_seconds);
    Ok(settings)
}

#[tauri::command]
pub fn claim_startup_welcome(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?
        .claim_startup_welcome()
        .map_err(|_| "Unable to load the startup welcome".to_string())
}

#[tauri::command]
pub fn startup_welcome_pending(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?
        .startup_welcome_pending()
        .map_err(|_| "Unable to load the startup welcome".to_string())
}

#[tauri::command]
pub async fn capture_screen_observation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::storage::ScreenObservationRecord, String> {
    let observation = capture_and_store(&app, &state, "manual").await?;
    let response_app = app.clone();
    let response_observation = observation.clone();
    tauri::async_runtime::spawn(async move {
        let state = response_app.state::<AppState>();
        let _ = crate::openai::turns::respond_to_screen_observation(
            &response_app,
            &state,
            &response_observation,
        )
        .await;
    });
    Ok(observation)
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

use crate::{state::AppState, storage::{new_id, now_ms, ProviderRecord}};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveProviderRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_true")]
    pub make_active: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderIdRequest {
    pub provider_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderView {
    #[serde(flatten)]
    pub provider: ProviderRecord,
    pub api_key_configured: bool,
}

fn default_true() -> bool { true }

pub fn providers(state: &State<'_, AppState>) -> Result<Vec<ProviderView>, String> {
    let providers = state.storage.lock().map_err(|_| "Local storage is unavailable".to_string())?
        .providers().map_err(|_| "Unable to load AI providers".to_string())?;
    Ok(providers.into_iter().map(|provider| ProviderView {
        api_key_configured: state.secrets.provider_key_configured(&provider.id),
        provider,
    }).collect())
}

pub fn save_provider(state: &State<'_, AppState>, request: SaveProviderRequest) -> Result<ProviderView, String> {
    let name = request.name.trim();
    let model = request.model.trim();
    let mut base_url = request.base_url.trim().trim_end_matches('/').to_string();
    if name.is_empty() || model.is_empty() {
        return Err("Provider name and model are required".to_string());
    }
    let parsed = reqwest::Url::parse(&base_url).map_err(|_| "Base URL must be a valid HTTP URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Base URL must use HTTP or HTTPS".to_string());
    }
    if !parsed.path().ends_with("/v1") {
        base_url.push_str("/v1");
    }
    let timestamp = now_ms();
    let provider = ProviderRecord {
        id: request.id.unwrap_or_else(new_id),
        name: name.to_string(),
        base_url,
        model: model.to_string(),
        is_active: request.make_active,
        created_at: timestamp,
        updated_at: timestamp,
    };
    if let Some(api_key) = request.api_key.as_deref().filter(|key| !key.trim().is_empty()) {
        state.secrets.save_provider_api_key(&provider.id, api_key)?;
    }
    let mut storage = state.storage.lock().map_err(|_| "Local storage is unavailable".to_string())?;
    let provider = storage.save_provider(&provider).map_err(|_| "Unable to save the AI provider".to_string())?;
    storage.append_audit("provider", &provider.id, "saved", None).map_err(|_| "Unable to write the provider audit".to_string())?;
    Ok(ProviderView { api_key_configured: state.secrets.provider_key_configured(&provider.id), provider })
}

pub fn select_provider(state: &State<'_, AppState>, request: ProviderIdRequest) -> Result<ProviderView, String> {
    let mut storage = state.storage.lock().map_err(|_| "Local storage is unavailable".to_string())?;
    let provider = storage.set_active_provider(&request.provider_id).map_err(|_| "The AI provider no longer exists".to_string())?;
    Ok(ProviderView { api_key_configured: state.secrets.provider_key_configured(&provider.id), provider })
}

pub fn delete_provider_key(state: &State<'_, AppState>, request: ProviderIdRequest) -> Result<(), String> {
    state.secrets.delete_provider_api_key(&request.provider_id)
}

pub fn delete_provider(state: &State<'_, AppState>, request: ProviderIdRequest) -> Result<bool, String> {
    state.secrets.delete_provider_api_key(&request.provider_id)?;
    state.storage.lock().map_err(|_| "Local storage is unavailable".to_string())?
        .delete_provider(&request.provider_id).map_err(|_| "Unable to remove the AI provider".to_string())
}

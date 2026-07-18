use crate::{
    openai::OpenAiClient,
    screen_aware::ScreenAwareService,
    secrets::SecretStore,
    storage::Storage,
    tools::{RepositoryAllowlist, ToolRegistry},
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct AppState {
    pub secrets: SecretStore,
    pub openai: OpenAiClient,
    pub storage: Mutex<Storage>,
    pub tools: ToolRegistry,
    pub repositories: RepositoryAllowlist,
    pub screen_aware: ScreenAwareService,
}

impl AppState {
    pub fn new(
        database_path: &Path,
        model_archive_path: PathBuf,
        model_cache_dir: PathBuf,
    ) -> rusqlite::Result<Self> {
        let secrets = SecretStore;
        Ok(Self {
            secrets,
            openai: OpenAiClient::new(secrets),
            storage: Mutex::new(Storage::open(database_path)?),
            tools: ToolRegistry,
            repositories: RepositoryAllowlist::from_environment(),
            screen_aware: ScreenAwareService::new(model_archive_path, model_cache_dir),
        })
    }
}

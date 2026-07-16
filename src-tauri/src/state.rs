use crate::{
    openai::OpenAiClient,
    secrets::SecretStore,
    storage::Storage,
    tools::{RepositoryAllowlist, ToolRegistry},
};
use std::path::Path;
use std::sync::Mutex;

pub struct AppState {
    pub secrets: SecretStore,
    pub openai: OpenAiClient,
    pub storage: Mutex<Storage>,
    pub tools: ToolRegistry,
    pub repositories: RepositoryAllowlist,
}

impl AppState {
    pub fn new(database_path: &Path) -> rusqlite::Result<Self> {
        let secrets = SecretStore;
        Ok(Self {
            secrets,
            openai: OpenAiClient::new(secrets),
            storage: Mutex::new(Storage::open(database_path)?),
            tools: ToolRegistry,
            repositories: RepositoryAllowlist::from_environment(),
        })
    }
}

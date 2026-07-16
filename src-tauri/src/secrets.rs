use keyring::Entry;

const SERVICE: &str = "com.dell.daemon";
const ACCOUNT: &str = "openai-api-key";

#[derive(Clone, Copy, Default)]
pub struct SecretStore;

impl SecretStore {
    fn entry(&self) -> Result<Entry, String> {
        Entry::new(SERVICE, ACCOUNT).map_err(|_| "Unable to access the credential manager".to_string())
    }

    pub fn save_api_key(&self, api_key: &str) -> Result<(), String> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err("The OpenAI API key cannot be empty".to_string());
        }

        self.entry()?
            .set_password(api_key)
            .map_err(|_| "Unable to store the OpenAI API key".to_string())
    }

    pub fn load_api_key(&self) -> Result<String, String> {
        self.entry()?
            .get_password()
            .map_err(|_| "The OpenAI API key is not configured".to_string())
    }

    pub fn delete_api_key(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) => Ok(()),
            Err(_) => Ok(()),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.load_api_key().is_ok()
    }
}

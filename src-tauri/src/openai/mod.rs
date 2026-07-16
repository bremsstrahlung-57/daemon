mod types;
pub mod turns;

pub use types::{ModelTurnRequest, ResponsesApiResponse, ResponsesRequest, TurnMessage, TurnRole};

use crate::secrets::SecretStore;
use reqwest::Client;

const RESPONSES_ENDPOINT: &str = "https://api.openai.com/v1/responses";
pub const DEFAULT_LUNA_MODEL: &str = "gpt-4o-mini";

#[derive(Clone)]
pub struct OpenAiClient {
    http: Client,
    secrets: SecretStore,
    endpoint: String,
}

impl OpenAiClient {
    pub fn new(secrets: SecretStore) -> Self {
        Self {
            http: Client::new(),
            secrets,
            endpoint: RESPONSES_ENDPOINT.to_string(),
        }
    }

    pub async fn create_response(
        &self,
        request: ModelTurnRequest,
    ) -> Result<ResponsesApiResponse, String> {
        let api_key = self.secrets.load_api_key()?;
        if request.messages.is_empty() {
            return Err("A model turn requires at least one message".to_string());
        }

        let body = ResponsesRequest {
            model: request
                .model
                .unwrap_or_else(|| DEFAULT_LUNA_MODEL.to_string()),
            instructions: request.instructions,
            input: request
                .messages
                .into_iter()
                .map(|message| ResponsesRequest::input_item(message))
                .collect(),
        };

        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|_| "The OpenAI request could not be sent".to_string())?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("The OpenAI request failed with status {}", status.as_u16()));
        }

        response
            .json::<ResponsesApiResponse>()
            .await
            .map_err(|_| "The OpenAI response was not understood".to_string())
    }
}

impl ResponsesRequest {
    fn input_item(message: TurnMessage) -> types::ResponsesInputItem {
        types::ResponsesInputItem {
            role: match message.role {
                TurnRole::User => "user".to_string(),
                TurnRole::Assistant => "assistant".to_string(),
            },
            content: vec![types::ResponsesInputContent {
                content_type: "input_text",
                text: message.content,
            }],
        }
    }
}

mod types;
pub mod turns;

pub use types::{ChatCompletionChunk, ChatCompletionRequest, ChatMessage, ModelTurnRequest, ResponsesApiResponse, ResponsesRequest, TurnMessage, TurnRole};

use crate::{secrets::SecretStore, storage::ProviderRecord};
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

    pub async fn stream_chat<F>(
        &self,
        provider: &ProviderRecord,
        messages: Vec<ChatMessage>,
        mut on_delta: F,
    ) -> Result<String, String>
    where
        F: FnMut(&str),
    {
        if messages.is_empty() {
            return Err("A conversation requires at least one message".to_string());
        }
        let endpoint = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
        let request = self.http.post(endpoint).json(&ChatCompletionRequest {
            model: provider.model.clone(),
            messages,
            stream: true,
        });
        let request = match self.secrets.load_provider_api_key(&provider.id) {
            Ok(api_key) => request.bearer_auth(api_key),
            Err(_) => request,
        };
        let mut response = request
            .send()
            .await
            .map_err(|_| "The selected AI provider could not be reached".to_string())?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("The selected AI provider returned {status}: {}", body.chars().take(240).collect::<String>()));
        }
        let mut pending = String::new();
        let mut output = String::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "The AI provider stream ended unexpectedly".to_string())?
        {
            pending.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = pending.find("\n\n") {
                let event = pending[..end].to_string();
                pending.drain(..end + 2);
                let Some(data) = event.lines().find_map(|line| line.strip_prefix("data:")) else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    continue;
                }
                let chunk: ChatCompletionChunk = serde_json::from_str(data)
                    .map_err(|_| "The selected AI provider sent an invalid streaming response".to_string())?;
                for choice in chunk.choices {
                    if let Some(delta) = choice.delta.content {
                        on_delta(&delta);
                        output.push_str(&delta);
                    }
                }
            }
        }
        if output.trim().is_empty() {
            return Err("The selected AI provider returned an empty response".to_string());
        }
        Ok(output)
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

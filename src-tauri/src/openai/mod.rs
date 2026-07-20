mod types;
pub mod turns;

pub use types::{capture_screen_tool, create_memory_tool, create_note_tool, search_memories_tool, search_notes_tool, show_mascot_reaction_tool, ChatCompletionFunctionTool, ChatCompletionMessage, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionToolCall, ModelTurnRequest, ResponsesApiResponse, ResponsesInputContent, ResponsesInputItem, ResponsesRequest, TurnMessage, TurnRole};

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
        self.send_response(
            self.endpoint.clone(),
            Some(api_key),
            ResponsesRequest::from_turn(request),
        )
        .await
    }

    pub async fn create_provider_completion(
        &self,
        provider: &ProviderRecord,
        messages: Vec<ChatCompletionMessage>,
        tools: Vec<ChatCompletionFunctionTool>,
    ) -> Result<ChatCompletionResponse, String> {
        let endpoint = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
        let api_key = self
            .secrets
            .load_provider_api_key(&provider.id)
            .map_err(|_| "The selected AI provider has no saved API key. Open Settings and save the key again.".to_string())?;
        self.send_provider_completion(
            endpoint,
            Some(api_key),
            ChatCompletionRequest {
                model: provider.model.clone(),
                messages,
                parallel_tool_calls: (!tools.is_empty()).then_some(false),
                tools,
            },
        )
        .await
    }

    async fn send_provider_completion(
        &self,
        endpoint: String,
        api_key: Option<String>,
        body: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, String> {
        if body.messages.is_empty() {
            return Err("A conversation requires at least one message".to_string());
        }

        let request = self.http.post(endpoint).json(&body);
        let request = match api_key {
            Some(api_key) => request.bearer_auth(api_key),
            None => request,
        };
        let response = request
            .send()
            .await
            .map_err(|_| "The selected AI provider could not be reached".to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("The selected AI provider returned {}", status.as_u16()));
        }
        response
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|_| "The selected AI provider sent an invalid response".to_string())
    }

    async fn send_response(
        &self,
        endpoint: String,
        api_key: Option<String>,
        body: ResponsesRequest,
    ) -> Result<ResponsesApiResponse, String> {
        if body.input.is_empty() {
            return Err("A model turn requires at least one input item".to_string());
        }

        let request = self.http.post(endpoint).json(&body);
        let request = match api_key {
            Some(api_key) => request.bearer_auth(api_key),
            None => request,
        };
        let response = request
            .send()
            .await
            .map_err(|_| "The OpenAI response request could not be sent".to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("The OpenAI response request failed with status {}", status.as_u16()));
        }
        response
            .json::<ResponsesApiResponse>()
            .await
            .map_err(|_| "The OpenAI response was not understood".to_string())
    }

}

impl ResponsesRequest {
    fn from_turn(request: ModelTurnRequest) -> Self {
        Self {
            model: request
                .model
                .unwrap_or_else(|| DEFAULT_LUNA_MODEL.to_string()),
            instructions: request.instructions,
            input: request.messages.into_iter().map(Self::input_item).collect(),
        }
    }

    fn input_item(message: TurnMessage) -> ResponsesInputItem {
        ResponsesInputItem {
            role: match message.role {
                TurnRole::User => "user".to_string(),
                TurnRole::Assistant => "assistant".to_string(),
            },
            content: vec![ResponsesInputContent {
                content_type: "input_text",
                text: message.content,
            }],
        }
    }
}

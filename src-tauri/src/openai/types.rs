use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnMessage {
    pub role: TurnRole,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelTurnRequest {
    pub messages: Vec<TurnMessage>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponsesRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub input: Vec<ResponsesInputItem>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponsesInputItem {
    pub role: String,
    pub content: Vec<ResponsesInputContent>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponsesInputContent {
    #[serde(rename = "type")]
    pub content_type: &'static str,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionChunk {
    #[serde(default)]
    pub choices: Vec<ChatCompletionChoice>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionChoice {
    #[serde(default)]
    pub delta: ChatCompletionDelta,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ChatCompletionDelta {
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponsesApiResponse {
    pub id: String,
    #[serde(default)]
    pub output_text: Option<String>,
    #[serde(default)]
    pub output: Vec<ResponsesOutputItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponsesOutputItem {
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    #[serde(default)]
    pub content: Vec<ResponsesOutputContent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponsesOutputContent {
    #[serde(rename = "type")]
    pub content_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

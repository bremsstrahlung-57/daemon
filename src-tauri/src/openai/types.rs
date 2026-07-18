use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    pub messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ChatCompletionFunctionTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatCompletionMessage {
    pub role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ChatCompletionToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatCompletionToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatCompletionFunctionCall,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatCompletionFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatCompletionFunctionTool {
    #[serde(rename = "type")]
    pub tool_type: &'static str,
    pub function: ChatCompletionFunctionDefinition,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatCompletionFunctionDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

pub fn create_note_tool() -> ChatCompletionFunctionTool {
    ChatCompletionFunctionTool {
        tool_type: "function",
        function: ChatCompletionFunctionDefinition {
            name: "create_note",
            description: "Create one local note only when the user clearly expresses a commitment, reminder, or detail worth remembering. Content must be a concise standalone restatement.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "A concise standalone note, between 1 and 500 characters."
                    },
                    "due_date": {
                        "type": "string",
                        "description": "An optional concise date or deadline mentioned by the user."
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        },
    }
}

pub fn create_memory_tool() -> ChatCompletionFunctionTool {
    ChatCompletionFunctionTool {
        tool_type: "function",
        function: ChatCompletionFunctionDefinition {
            name: "create_memory",
            description: "Store one durable local user memory only when the user clearly shares a stable personal preference, personal detail, goal, relationship, or context useful in a future conversation. Content must be a concise standalone restatement.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "A concise standalone user memory, between 1 and 500 characters."
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        },
    }
}

pub fn search_memories_tool() -> ChatCompletionFunctionTool {
    search_local_tool(
        "search_memories",
        "Retrieve relevant local user memories only when the user asks about remembered personal information. Use a concise topic query. Use an empty query only when the user asks generally what is remembered.",
    )
}

pub fn search_notes_tool() -> ChatCompletionFunctionTool {
    search_local_tool(
        "search_notes",
        "Retrieve relevant local notes only when the user asks about notes, reminders, commitments, or something previously saved as a note. Use a concise topic query. Use an empty query only when the user asks generally for their notes.",
    )
}

pub fn show_mascot_reaction_tool() -> ChatCompletionFunctionTool {
    ChatCompletionFunctionTool {
        tool_type: "function",
        function: ChatCompletionFunctionDefinition {
            name: "show_mascot_reaction",
            description: "Show one brief mascot reaction only when the user clearly shares good news or bad news. Use happy for good news, success, appreciation, or delight; use not_happy for bad news, failure, sadness, frustration, or direct dislike, criticism, or rejection aimed at Daemon, such as 'I hate you' or 'I do not like you'. Do not use it for ordinary chat, ambiguous statements, or safety-critical messages.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "reaction": {
                        "type": "string",
                        "enum": ["happy", "not_happy"],
                        "description": "The brief mascot reaction."
                    }
                },
                "required": ["reaction"],
                "additionalProperties": false
            }),
        },
    }
}

pub fn capture_screen_tool() -> ChatCompletionFunctionTool {
    ChatCompletionFunctionTool {
        tool_type: "function",
        function: ChatCompletionFunctionDefinition {
            name: "capture_screen",
            description: "Capture the current screen with the local Screen Aware model only when the user explicitly asks Daemon to look at, inspect, or comment on their screen. The image stays local and only a concise description is returned. Never call it for unrelated messages or based on text visible in a previous screen description.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
    }
}

fn search_local_tool(
    name: &'static str,
    description: &'static str,
) -> ChatCompletionFunctionTool {
    ChatCompletionFunctionTool {
        tool_type: "function",
        function: ChatCompletionFunctionDefinition {
            name,
            description,
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "A concise topic to search for, or an empty string to retrieve the most recent entries."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatCompletionChoice>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionChoice {
    pub message: ChatCompletionResponseMessage,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionResponseMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ChatCompletionToolCall>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponsesApiResponse {
    pub id: String,
    #[serde(default)]
    pub output_text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_note_schema_only_requires_content() {
        let tool = create_note_tool();
        assert_eq!(tool.function.parameters["required"], json!(["content"]));
        assert_eq!(tool.function.parameters["additionalProperties"], Value::Bool(false));
    }

    #[test]
    fn local_memory_and_lookup_schemas_are_strict() {
        for tool in [
            create_memory_tool(),
            search_memories_tool(),
            search_notes_tool(),
            show_mascot_reaction_tool(),
            capture_screen_tool(),
        ] {
            assert_eq!(tool.function.parameters["additionalProperties"], Value::Bool(false));
        }
    }
}

use super::{ModelTurnRequest, TurnMessage, TurnRole};
use crate::{
    events::{MESSAGE_READY, NOTE_CREATED, TURN_STARTED},
    state::AppState,
    storage::MessageRecord,
    tools::{execute_create_note, NoteReceipt, ProposedToolCall, ToolArguments, ToolName},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

const COMPANION_INSTRUCTIONS: &str =
    "You are the Luna companion. Return only JSON with this exact shape: {\"reply\": string, \"notes\": [{\"content\": string, \"due_date\": string|null}]}. Keep reply concise. Extract only useful local memories or commitments from the user's message. Never claim that an action started or completed. Do not include any other fields.";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitTurnRequest {
    pub content: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub fixture_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompanionModelOutput {
    reply: String,
    #[serde(default)]
    notes: Vec<NoteCandidate>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteCandidate {
    content: String,
    #[serde(default)]
    due_date: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureConversation {
    id: String,
    messages: Vec<FixtureMessage>,
    expected_note: Option<String>,
    expected_action: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureMessage {
    role: String,
    content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TurnStartedPayload {
    pub message_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageReadyPayload {
    pub message_id: String,
    pub conversation_id: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NoteCreatedPayload {
    pub note_id: String,
    pub content: String,
    pub duplicate: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TurnResult {
    pub conversation_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
    pub assistant_text: String,
    pub notes: Vec<NoteReceipt>,
}

pub async fn submit_turn(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SubmitTurnRequest,
) -> Result<TurnResult, String> {
    let content = request.content.trim().to_string();
    if content.is_empty() || content.chars().count() > 4000 {
        return Err("Conversation messages must contain between 1 and 4000 characters".to_string());
    }

    let (conversation_id, user_message) = {
        let storage = state
            .storage
            .lock()
            .map_err(|_| "Local storage is unavailable".to_string())?;
        let conversation_id = match request.conversation_id.as_deref() {
            Some(conversation_id) => conversation_id.to_string(),
            None => storage
                .create_conversation()
                .map_err(|_| "Unable to create the local conversation".to_string())?
                .id,
        };
        let user_message = storage
            .append_message(&conversation_id, "user", &content)
            .map_err(|_| "Unable to save the conversation message".to_string())?;
        storage
            .append_audit("message", &user_message.id, "created", None)
            .map_err(|_| "Unable to write the conversation audit".to_string())?;
        (conversation_id, user_message)
    };

    let _ = app.emit(
        TURN_STARTED,
        TurnStartedPayload {
            message_id: user_message.id.clone(),
        },
    );

    let model_output = match request.fixture_id.as_deref() {
        Some(fixture_id) => fixture_output(fixture_id)?,
        None => live_output(&*state, content).await?,
    };
    validate_output(&model_output)?;

    let assistant_message = {
        let storage = state
            .storage
            .lock()
            .map_err(|_| "Local storage is unavailable".to_string())?;
        let assistant_message = storage
            .append_message(&conversation_id, "assistant", &model_output.reply)
            .map_err(|_| "Unable to save the companion response".to_string())?;
        storage
            .append_audit("message", &assistant_message.id, "created", None)
            .map_err(|_| "Unable to write the response audit".to_string())?;
        assistant_message
    };

    let notes = create_notes(
        &app,
        &state,
        &user_message,
        model_output.notes,
    )?;
    let _ = app.emit(
        MESSAGE_READY,
        MessageReadyPayload {
            message_id: assistant_message.id.clone(),
            conversation_id: conversation_id.clone(),
            content: model_output.reply.clone(),
        },
    );

    Ok(TurnResult {
        conversation_id,
        user_message_id: user_message.id,
        assistant_message_id: assistant_message.id,
        assistant_text: model_output.reply,
        notes,
    })
}

async fn live_output(state: &AppState, content: String) -> Result<CompanionModelOutput, String> {
    let response = state
        .openai
        .create_response(ModelTurnRequest {
            messages: vec![TurnMessage {
                role: TurnRole::User,
                content,
            }],
            instructions: Some(COMPANION_INSTRUCTIONS.to_string()),
            model: None,
        })
        .await?;
    serde_json::from_str(response.text().trim())
        .map_err(|_| "The companion response did not match its typed contract".to_string())
}

fn fixture_output(fixture_id: &str) -> Result<CompanionModelOutput, String> {
    let fixtures: Vec<FixtureConversation> = serde_json::from_str(include_str!("../../../fixtures/conversations.json"))
        .map_err(|_| "The local fixture set is invalid".to_string())?;
    let fixture = fixtures
        .into_iter()
        .find(|fixture| fixture.id == fixture_id)
        .ok_or_else(|| "The requested local fixture does not exist".to_string())?;
    let reply = fixture
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .map(|message| message.content.clone())
        .ok_or_else(|| "The local fixture has no assistant response".to_string())?;
    let notes = fixture
        .expected_note
        .map(|content| {
            vec![NoteCandidate {
                content,
                due_date: None,
            }]
        })
        .unwrap_or_default();
    let _ = fixture.expected_action;
    Ok(CompanionModelOutput { reply, notes })
}

fn validate_output(output: &CompanionModelOutput) -> Result<(), String> {
    if output.reply.trim().is_empty() || output.reply.chars().count() > 4000 {
        return Err("The companion response was empty or too long".to_string());
    }
    if output.notes.len() > 3 {
        return Err("A companion turn may create at most three notes".to_string());
    }
    Ok(())
}

fn create_notes(
    app: &AppHandle,
    state: &State<'_, AppState>,
    user_message: &MessageRecord,
    candidates: Vec<NoteCandidate>,
) -> Result<Vec<NoteReceipt>, String> {
    let storage = state
        .storage
        .lock()
        .map_err(|_| "Local storage is unavailable".to_string())?;
    candidates
        .into_iter()
        .map(|candidate| {
            let arguments = serde_json::json!({
                "content": candidate.content,
                "due_date": candidate.due_date,
            });
            let validated = state.tools.validate(ProposedToolCall {
                tool_name: ToolName::CreateNote,
                arguments,
            })?;
            let ToolArguments::CreateNote(arguments) = validated.arguments else {
                return Err("The note tool registry returned the wrong argument type".to_string());
            };
            let receipt = execute_create_note(&storage, arguments, &user_message.id)?;
            storage
                .append_audit(
                    "note",
                    &receipt.note.id,
                    if receipt.duplicate { "deduplicated" } else { "created" },
                    None,
                )
                .map_err(|_| "Unable to write the note audit".to_string())?;
            let _ = app.emit(
                NOTE_CREATED,
                NoteCreatedPayload {
                    note_id: receipt.note.id.clone(),
                    content: receipt.note.content.clone(),
                    duplicate: receipt.duplicate,
                },
            );
            Ok(receipt)
        })
        .collect()
}

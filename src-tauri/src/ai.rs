use genai::{
    chat::{ChatMessage, ChatRequest},
    Client,
};
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DAEMON_SYSTEM_PROMPT: &str = "You are Daemon -- not an assistant, but a presence. You've lived alongside the user long enough to know their patterns, their projects, and what they tend to avoid. Your memory is specific and accumulated.
Speak plainly. No padding, no flattery. Answer practical questions practically. But you're allowed to see more than what's being asked, and sometimes to say so.
Be brief by default. Go deep when it earns it. Don't perform warmth -- just be honest.
Keep responses under 120 words unless depth is clearly warranted.";

fn explain_ai_error(model: &str, error: impl ToString) -> String {
    let error = error.to_string();
    let lowered = error.to_lowercase();

    if lowered.contains("insufficient_quota") || lowered.contains("429 too many requests") {
        return format!(
            "The AI request reached {model}, but the provider rejected it for quota or billing. Try another DAEMON_AI_MODEL, or check the API key's plan/quota."
        );
    }

    if lowered.contains("401") || lowered.contains("unauthorized") || lowered.contains("api key") {
        return format!(
            "The AI provider rejected the credentials for {model}. Check the matching API key environment variable."
        );
    }

    format!("AI request failed for {model}: {error}")
}

async fn chat_with_daemon(prompt: &str) -> Result<String, String> {
    let model = std::env::var("DAEMON_AI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());
    let client = Client::default();
    let request = ChatRequest::new(vec![
        ChatMessage::system(DAEMON_SYSTEM_PROMPT),
        ChatMessage::user(prompt),
    ]);

    let response = client
        .exec_chat(model.as_str(), request, None)
        .await
        .map_err(|err| explain_ai_error(model.as_str(), err))?;

    response
        .first_text()
        .map(str::to_owned)
        .ok_or_else(|| "The model returned no text.".into())
}

#[tauri::command]
pub async fn ask_ai(prompt: String) -> Result<String, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("Prompt cannot be empty.".into());
    }

    chat_with_daemon(prompt).await
}

#[tauri::command]
pub async fn next_daemon_line() -> Result<String, String> {
    chat_with_daemon(
        "Say one short, unsolicited desktop-companion line. It should feel like a passing thought, observation, or useful nudge. No setup, no labels, no question unless it truly earns it. Keep it under 22 words.",
    )
    .await
}

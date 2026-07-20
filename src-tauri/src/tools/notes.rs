use super::policy::CreateNoteArguments;
use crate::storage::{now_ms, NoteRecord, Storage};
use serde::Serialize;
use std::collections::HashSet;

const NOTE_DEDUP_WINDOW_MS: i64 = 5 * 60 * 1000;

#[derive(Clone, Debug, Serialize)]
pub struct NoteReceipt {
    pub note: NoteRecord,
    pub duplicate: bool,
}

pub fn execute_create_note(
    storage: &mut Storage,
    conversation_id: &str,
    source_message_id: &str,
    arguments: CreateNoteArguments,
) -> Result<NoteReceipt, String> {
    validate_create_note_arguments(&arguments)?;
    let content = arguments.content.trim().to_string();
    let normalized = normalize(&content);
    let recent_notes = storage
        .recent_notes_for_conversation(conversation_id, now_ms() - NOTE_DEDUP_WINDOW_MS)
        .map_err(|_| "Unable to read recent local notes".to_string())?;

    if let Some(note) = recent_notes
        .into_iter()
        .find(|note| similar(&normalized, &normalize(&note.content)))
    {
        return Ok(NoteReceipt {
            note,
            duplicate: true,
        });
    }

    let note = storage
        .insert_note_with_audit(&content, source_message_id)
        .map_err(|_| "Unable to save the local note".to_string())?;
    Ok(NoteReceipt {
        note,
        duplicate: false,
    })
}

fn validate_create_note_arguments(arguments: &CreateNoteArguments) -> Result<(), String> {
    let content = arguments.content.trim();
    if content.is_empty() || content.chars().count() > 500 {
        return Err("Note content must contain between 1 and 500 characters".to_string());
    }
    if arguments.due_date.as_deref().is_some_and(|due_date| {
        due_date.trim().is_empty() || due_date.chars().count() > 32
    }) {
        return Err("Note due dates must contain between 1 and 32 characters".to_string());
    }
    Ok(())
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn similar(left: &str, right: &str) -> bool {
    if left == right || left.contains(right) || right.contains(left) {
        return true;
    }
    let left_tokens: HashSet<&str> = left.split_whitespace().collect();
    let right_tokens: HashSet<&str> = right.split_whitespace().collect();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }
    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    intersection / union >= 0.75
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use std::{env, fs};
    use uuid::Uuid;

    #[test]
    fn deduplicates_similar_notes_in_one_conversation() {
        let path = env::temp_dir().join(format!("daemon-notes-{}.sqlite3", Uuid::new_v4()));
        let mut storage = Storage::open(&path).expect("temporary database should open");
        let conversation = storage
            .create_conversation()
            .expect("conversation should be created");
        let first_message = storage
            .append_message(&conversation.id, "user", "I need to fix the login bug tomorrow")
            .expect("message should be created");
        let first = execute_create_note(
            &mut storage,
            &conversation.id,
            &first_message.id,
            CreateNoteArguments {
                content: "Fix the login bug tomorrow".to_string(),
                due_date: Some("tomorrow".to_string()),
            },
        )
        .expect("first note should be saved");
        let second_message = storage
            .append_message(&conversation.id, "user", "The login bug still needs fixing tomorrow")
            .expect("second message should be created");
        let second = execute_create_note(
            &mut storage,
            &conversation.id,
            &second_message.id,
            CreateNoteArguments {
                content: "Fix login bug tomorrow".to_string(),
                due_date: Some("tomorrow".to_string()),
            },
        )
        .expect("duplicate note should be handled");

        assert!(!first.duplicate);
        assert!(second.duplicate);
        assert_eq!(first.note.id, second.note.id);
        drop(storage);
        let _ = fs::remove_file(path);
    }
}

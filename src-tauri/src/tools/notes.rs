use super::policy::CreateNoteArguments;
use crate::storage::{NoteRecord, Storage};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize)]
pub struct NoteReceipt {
    pub note: NoteRecord,
    pub duplicate: bool,
}

pub fn execute_create_note(
    storage: &Storage,
    arguments: CreateNoteArguments,
    source_message_id: &str,
) -> Result<NoteReceipt, String> {
    let content = arguments.content.trim().to_string();
    let normalized = normalize(&content);
    let existing = storage
        .active_notes_for_source(source_message_id)
        .map_err(|_| "Unable to read local notes".to_string())?
        .into_iter()
        .find(|note| similar(&normalized, &normalize(&note.content)));

    if let Some(note) = existing {
        return Ok(NoteReceipt {
            note,
            duplicate: true,
        });
    }

    let note = storage
        .insert_note(&content, source_message_id)
        .map_err(|_| "Unable to save the local note".to_string())?;
    Ok(NoteReceipt {
        note,
        duplicate: false,
    })
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
    if left == right {
        return true;
    }
    let left_tokens: HashSet<&str> = left.split_whitespace().collect();
    let right_tokens: HashSet<&str> = right.split_whitespace().collect();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }
    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    intersection / union >= 0.8
}

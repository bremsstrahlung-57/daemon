use super::policy::CreateMemoryArguments;
use crate::storage::{MemoryRecord, Storage};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize)]
pub struct MemoryReceipt {
    pub memory: MemoryRecord,
    pub duplicate: bool,
}

pub fn execute_create_memory(
    storage: &mut Storage,
    source_message_id: &str,
    arguments: CreateMemoryArguments,
) -> Result<MemoryReceipt, String> {
    let content = arguments.content.trim().to_string();
    let normalized = normalize(&content);
    let memories = storage
        .search_memories(&content, 100)
        .map_err(|_| "Unable to read local memories".to_string())?;
    if let Some(memory) = memories
        .into_iter()
        .find(|memory| similar(&normalized, &normalize(&memory.content)))
    {
        return Ok(MemoryReceipt {
            memory,
            duplicate: true,
        });
    }

    let memory = storage
        .insert_memory_with_audit(&content, source_message_id)
        .map_err(|_| "Unable to save the local memory".to_string())?;
    Ok(MemoryReceipt {
        memory,
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

use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::Serialize;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug)]
pub struct Storage {
    connection: Connection,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConversationRecord {
    pub id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NoteRecord {
    pub id: String,
    pub content: String,
    pub source_message_id: String,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub source_message_id: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScreenObservationRecord {
    pub id: String,
    pub timestamp: i64,
    pub description: String,
    pub source: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScreenAwareSettingsRecord {
    pub interval_seconds: Option<i64>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProposalRecord {
    pub id: String,
    pub conversation_id: String,
    pub tool_name: String,
    pub arguments_json: String,
    pub arguments_hash: String,
    pub preview: String,
    pub approval_policy: String,
    pub status: String,
    pub provider_context_json: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub resolved_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct JobRecord {
    pub id: String,
    pub proposal_id: String,
    pub kind: String,
    pub status: String,
    pub workspace_path: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderRecord {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditEventRecord {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub details_json: Option<String>,
    pub created_at: i64,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

impl Storage {
    pub fn open(path: &Path) -> SqlResult<Self> {
        let connection = Connection::open(path)?;
        let storage = Self { connection };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> SqlResult<()> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS conversations (
                 id TEXT PRIMARY KEY,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages (
                 id TEXT PRIMARY KEY,
                 conversation_id TEXT NOT NULL REFERENCES conversations(id),
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS notes (
                 id TEXT PRIMARY KEY,
                 content TEXT NOT NULL,
                 source_message_id TEXT NOT NULL REFERENCES messages(id),
                 created_at INTEGER NOT NULL,
                 deleted_at INTEGER
             );
             CREATE TABLE IF NOT EXISTS memories (
                 id TEXT PRIMARY KEY,
                 content TEXT NOT NULL,
                 source_message_id TEXT NOT NULL REFERENCES messages(id),
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS screen_observations (
                 id TEXT PRIMARY KEY,
                 timestamp INTEGER NOT NULL,
                 description TEXT NOT NULL,
                 source TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS screen_aware_settings (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 interval_seconds INTEGER CHECK (interval_seconds IS NULL OR interval_seconds > 0),
                 capture_trigger TEXT NOT NULL CHECK (capture_trigger IN ('single', 'double')),
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS proposals (
                 id TEXT PRIMARY KEY,
                 conversation_id TEXT NOT NULL REFERENCES conversations(id),
                 tool_name TEXT NOT NULL,
                 arguments_json TEXT NOT NULL,
                 arguments_hash TEXT NOT NULL,
                 preview TEXT NOT NULL,
                 approval_policy TEXT NOT NULL,
                 status TEXT NOT NULL,
                 provider_context_json TEXT,
                 created_at INTEGER NOT NULL,
                 expires_at INTEGER,
                 resolved_at INTEGER
             );
             CREATE TABLE IF NOT EXISTS jobs (
                 id TEXT PRIMARY KEY,
                 proposal_id TEXT NOT NULL REFERENCES proposals(id),
                 kind TEXT NOT NULL,
                 status TEXT NOT NULL,
                 workspace_path TEXT,
                 started_at INTEGER,
                 completed_at INTEGER,
                 result_json TEXT,
                 error_message TEXT
             );
             CREATE TABLE IF NOT EXISTS audit_events (
                 id TEXT PRIMARY KEY,
                 entity_type TEXT NOT NULL,
                 entity_id TEXT NOT NULL,
                 event_type TEXT NOT NULL,
                 details_json TEXT,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS providers (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 base_url TEXT NOT NULL,
                 model TEXT NOT NULL,
                 is_active INTEGER NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS messages_conversation_idx ON messages(conversation_id, created_at);
             CREATE INDEX IF NOT EXISTS notes_source_idx ON notes(source_message_id, created_at);
             CREATE INDEX IF NOT EXISTS memories_created_idx ON memories(created_at);
             CREATE INDEX IF NOT EXISTS screen_observations_timestamp_idx ON screen_observations(timestamp);
             CREATE INDEX IF NOT EXISTS proposals_status_idx ON proposals(status, created_at);
             CREATE INDEX IF NOT EXISTS jobs_status_idx ON jobs(status, completed_at);
             CREATE INDEX IF NOT EXISTS audit_entity_idx ON audit_events(entity_type, entity_id, created_at);",
        )
    }

    pub fn create_conversation(&self) -> SqlResult<ConversationRecord> {
        let timestamp = now_ms();
        let record = ConversationRecord {
            id: new_id(),
            created_at: timestamp,
            updated_at: timestamp,
        };
        self.connection.execute(
            "INSERT INTO conversations (id, created_at, updated_at) VALUES (?1, ?2, ?3)",
            params![record.id, record.created_at, record.updated_at],
        )?;
        Ok(record)
    }

    pub fn latest_conversation(&self) -> SqlResult<Option<ConversationRecord>> {
        self.connection
            .query_row(
                "SELECT id, created_at, updated_at FROM conversations ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(ConversationRecord {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()
    }

    pub fn append_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> SqlResult<MessageRecord> {
        let record = MessageRecord {
            id: new_id(),
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now_ms(),
        };
        self.connection.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![record.id, record.conversation_id, record.role, record.content, record.created_at],
        )?;
        self.connection.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![record.created_at, record.conversation_id],
        )?;
        Ok(record)
    }

    pub fn messages_for_conversation(
        &self,
        conversation_id: &str,
    ) -> SqlResult<Vec<MessageRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, conversation_id, role, content, created_at FROM messages
             WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )?;
        let messages = statement
            .query_map(params![conversation_id], |row| {
                Ok(MessageRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect();
        messages
    }

    pub fn recent_notes_for_conversation(
        &self,
        conversation_id: &str,
        created_after: i64,
    ) -> SqlResult<Vec<NoteRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT notes.id, notes.content, notes.source_message_id, notes.created_at, notes.deleted_at
             FROM notes
             INNER JOIN messages ON messages.id = notes.source_message_id
             WHERE messages.conversation_id = ?1
               AND notes.deleted_at IS NULL
               AND notes.created_at >= ?2
             ORDER BY notes.created_at DESC",
        )?;
        let notes = statement
            .query_map(params![conversation_id, created_after], |row| {
                Ok(NoteRecord {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    source_message_id: row.get(2)?,
                    created_at: row.get(3)?,
                    deleted_at: row.get(4)?,
                })
            })?
            .collect();
        notes
    }

    pub fn search_notes(&self, query: &str, limit: usize) -> SqlResult<Vec<NoteRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, content, source_message_id, created_at, deleted_at
             FROM notes
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT 100",
        )?;
        let notes = statement
            .query_map([], |row| {
                Ok(NoteRecord {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    source_message_id: row.get(2)?,
                    created_at: row.get(3)?,
                    deleted_at: row.get(4)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(rank_records(notes, query, limit, |note| &note.content))
    }

    pub fn search_memories(&self, query: &str, limit: usize) -> SqlResult<Vec<MemoryRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, content, source_message_id, created_at
             FROM memories
             ORDER BY created_at DESC
             LIMIT 100",
        )?;
        let memories = statement
            .query_map([], |row| {
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    source_message_id: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(rank_records(memories, query, limit, |memory| &memory.content))
    }

    pub fn screen_aware_settings(&self) -> SqlResult<ScreenAwareSettingsRecord> {
        self.connection
            .query_row(
                "SELECT interval_seconds, updated_at
                 FROM screen_aware_settings WHERE id = 1",
                [],
                |row| {
                    Ok(ScreenAwareSettingsRecord {
                        interval_seconds: row.get(0)?,
                        updated_at: row.get(1)?,
                    })
                },
            )
            .optional()
            .map(|settings| {
                settings.unwrap_or(ScreenAwareSettingsRecord {
                    interval_seconds: None,
                    updated_at: 0,
                })
            })
    }

    pub fn save_screen_aware_settings(
        &self,
        interval_seconds: Option<i64>,
    ) -> SqlResult<ScreenAwareSettingsRecord> {
        let settings = ScreenAwareSettingsRecord {
            interval_seconds,
            updated_at: now_ms(),
        };
        self.connection.execute(
            "INSERT INTO screen_aware_settings (id, interval_seconds, capture_trigger, updated_at)
             VALUES (1, ?1, 'single', ?2)
             ON CONFLICT(id) DO UPDATE SET
                 interval_seconds = excluded.interval_seconds,
                 updated_at = excluded.updated_at",
            params![
                settings.interval_seconds,
                settings.updated_at,
            ],
        )?;
        Ok(settings)
    }

    pub fn insert_screen_observation(
        &self,
        description: &str,
        source: &str,
    ) -> SqlResult<ScreenObservationRecord> {
        let timestamp = now_ms();
        let observation = ScreenObservationRecord {
            id: new_id(),
            timestamp,
            description: description.to_string(),
            source: source.to_string(),
            created_at: timestamp,
        };
        self.connection.execute(
            "INSERT INTO screen_observations (id, timestamp, description, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                observation.id,
                observation.timestamp,
                observation.description,
                observation.source,
                observation.created_at,
            ],
        )?;
        Ok(observation)
    }

    pub fn recent_screen_observations(
        &self,
        created_after: i64,
        limit: usize,
    ) -> SqlResult<Vec<ScreenObservationRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, timestamp, description, source, created_at
             FROM screen_observations
             WHERE timestamp >= ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![created_after, limit as i64], |row| {
            Ok(ScreenObservationRecord {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                description: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn insert_note_with_audit(
        &mut self,
        content: &str,
        source_message_id: &str,
    ) -> SqlResult<NoteRecord> {
        let created_at = now_ms();
        let note = NoteRecord {
            id: new_id(),
            content: content.to_string(),
            source_message_id: source_message_id.to_string(),
            created_at,
            deleted_at: None,
        };
        let audit = AuditEventRecord {
            id: new_id(),
            entity_type: "note".to_string(),
            entity_id: note.id.clone(),
            event_type: "created".to_string(),
            details_json: Some(serde_json::json!({ "content": note.content }).to_string()),
            created_at,
        };
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO notes (id, content, source_message_id, created_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, NULL)",
            params![note.id, note.content, note.source_message_id, note.created_at],
        )?;
        transaction.execute(
            "INSERT INTO audit_events (id, entity_type, entity_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                audit.id,
                audit.entity_type,
                audit.entity_id,
                audit.event_type,
                audit.details_json,
                audit.created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(note)
    }

    pub fn insert_memory_with_audit(
        &mut self,
        content: &str,
        source_message_id: &str,
    ) -> SqlResult<MemoryRecord> {
        let created_at = now_ms();
        let memory = MemoryRecord {
            id: new_id(),
            content: content.to_string(),
            source_message_id: source_message_id.to_string(),
            created_at,
        };
        let audit = AuditEventRecord {
            id: new_id(),
            entity_type: "memory".to_string(),
            entity_id: memory.id.clone(),
            event_type: "created".to_string(),
            details_json: Some(serde_json::json!({ "content": memory.content }).to_string()),
            created_at,
        };
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO memories (id, content, source_message_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                memory.id,
                memory.content,
                memory.source_message_id,
                memory.created_at,
            ],
        )?;
        transaction.execute(
            "INSERT INTO audit_events (id, entity_type, entity_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                audit.id,
                audit.entity_type,
                audit.entity_id,
                audit.event_type,
                audit.details_json,
                audit.created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(memory)
    }

    pub fn soft_delete_note_with_audit(&mut self, note_id: &str) -> SqlResult<bool> {
        let deleted_at = now_ms();
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE notes SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![deleted_at, note_id],
        )?;
        if changed == 0 {
            transaction.commit()?;
            return Ok(false);
        }
        transaction.execute(
            "INSERT INTO audit_events (id, entity_type, entity_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![new_id(), "note", note_id, "deleted", deleted_at],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn providers(&self) -> SqlResult<Vec<ProviderRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, base_url, model, is_active, created_at, updated_at
             FROM providers ORDER BY is_active DESC, updated_at DESC",
        )?;
        let providers = statement.query_map([], provider_from_row)?.collect();
        providers
    }

    pub fn active_provider(&self) -> SqlResult<Option<ProviderRecord>> {
        self.connection
            .query_row(
                "SELECT id, name, base_url, model, is_active, created_at, updated_at
                 FROM providers WHERE is_active = 1 LIMIT 1",
                [],
                provider_from_row,
            )
            .optional()
    }

    pub fn save_provider(&mut self, provider: &ProviderRecord) -> SqlResult<ProviderRecord> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if provider.is_active {
            transaction.execute("UPDATE providers SET is_active = 0", [])?;
        }
        transaction.execute(
            "INSERT INTO providers (id, name, base_url, model, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, base_url = excluded.base_url,
                model = excluded.model, is_active = excluded.is_active, updated_at = excluded.updated_at",
            params![provider.id, provider.name, provider.base_url, provider.model, provider.is_active as i64, provider.created_at, provider.updated_at],
        )?;
        transaction.commit()?;
        Ok(provider.clone())
    }

    pub fn set_active_provider(&mut self, provider_id: &str) -> SqlResult<ProviderRecord> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute("UPDATE providers SET is_active = 0", [])?;
        if transaction.execute(
            "UPDATE providers SET is_active = 1, updated_at = ?2 WHERE id = ?1",
            params![provider_id, now_ms()],
        )? != 1 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        let provider = transaction.query_row(
            "SELECT id, name, base_url, model, is_active, created_at, updated_at FROM providers WHERE id = ?1",
            params![provider_id],
            provider_from_row,
        )?;
        transaction.commit()?;
        Ok(provider)
    }

    pub fn delete_provider(&self, provider_id: &str) -> SqlResult<bool> {
        Ok(self.connection.execute("DELETE FROM providers WHERE id = ?1", params![provider_id])? == 1)
    }

    pub fn insert_proposal(&self, proposal: &ProposalRecord) -> SqlResult<()> {
        self.connection.execute(
            "INSERT INTO proposals
             (id, conversation_id, tool_name, arguments_json, arguments_hash, preview, approval_policy, status, provider_context_json, created_at, expires_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                proposal.id,
                proposal.conversation_id,
                proposal.tool_name,
                proposal.arguments_json,
                proposal.arguments_hash,
                proposal.preview,
                proposal.approval_policy,
                proposal.status,
                proposal.provider_context_json,
                proposal.created_at,
                proposal.expires_at,
                proposal.resolved_at,
            ],
        )?;
        Ok(())
    }

    pub fn pending_proposals(&self) -> SqlResult<Vec<ProposalRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, conversation_id, tool_name, arguments_json, arguments_hash, preview,
                    approval_policy, status, provider_context_json, created_at, expires_at, resolved_at
             FROM proposals WHERE status = 'awaiting_approval' ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map([], proposal_from_row)?;
        let proposals = rows.collect();
        proposals
    }

    pub fn approve_proposal(
        &mut self,
        proposal_id: &str,
        arguments_hash: &str,
    ) -> Result<(ProposalRecord, bool), String> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "Unable to begin proposal approval".to_string())?;
        let mut proposal = transaction
            .query_row(
                "SELECT id, conversation_id, tool_name, arguments_json, arguments_hash, preview,
                        approval_policy, status, provider_context_json, created_at, expires_at, resolved_at
                 FROM proposals WHERE id = ?1",
                params![proposal_id],
                proposal_from_row,
            )
            .optional()
            .map_err(|_| "Unable to read the proposal".to_string())?
            .ok_or_else(|| "The proposal no longer exists".to_string())?;
        if proposal.arguments_hash != arguments_hash {
            return Err("The proposal arguments changed; approval was rejected".to_string());
        }
        if proposal.status == "running" {
            transaction
                .commit()
                .map_err(|_| "Unable to finish proposal approval".to_string())?;
            return Ok((proposal, true));
        }
        if proposal.status != "awaiting_approval" {
            return Err("The proposal is no longer available for approval".to_string());
        }
        if proposal.expires_at.is_some_and(|expires_at| expires_at <= now_ms()) {
            transaction
                .execute(
                    "UPDATE proposals SET status = 'expired', resolved_at = ?1 WHERE id = ?2",
                    params![now_ms(), proposal_id],
                )
                .map_err(|_| "Unable to expire the proposal".to_string())?;
            transaction
                .commit()
                .map_err(|_| "Unable to finish proposal expiration".to_string())?;
            return Err("The proposal expired before approval".to_string());
        }
        let resolved_at = now_ms();
        let changed = transaction
            .execute(
                "UPDATE proposals
                 SET status = 'running', resolved_at = ?1
                 WHERE id = ?2 AND status = 'awaiting_approval' AND arguments_hash = ?3",
                params![resolved_at, proposal_id, arguments_hash],
            )
            .map_err(|_| "Unable to approve the proposal".to_string())?;
        if changed != 1 {
            return Err("The proposal changed while approval was being processed".to_string());
        }
        proposal.status = "running".to_string();
        proposal.resolved_at = Some(resolved_at);
        transaction
            .commit()
            .map_err(|_| "Unable to finish proposal approval".to_string())?;
        Ok((proposal, false))
    }

    pub fn deny_proposal(
        &mut self,
        proposal_id: &str,
        arguments_hash: &str,
    ) -> Result<ProposalRecord, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "Unable to begin proposal denial".to_string())?;
        let mut proposal = transaction
            .query_row(
                "SELECT id, conversation_id, tool_name, arguments_json, arguments_hash, preview,
                        approval_policy, status, provider_context_json, created_at, expires_at, resolved_at
                 FROM proposals WHERE id = ?1",
                params![proposal_id],
                proposal_from_row,
            )
            .optional()
            .map_err(|_| "Unable to read the proposal".to_string())?
            .ok_or_else(|| "The proposal no longer exists".to_string())?;
        if proposal.arguments_hash != arguments_hash {
            return Err("The proposal arguments changed; denial was rejected".to_string());
        }
        if proposal.status == "denied" {
            transaction
                .commit()
                .map_err(|_| "Unable to finish proposal denial".to_string())?;
            return Ok(proposal);
        }
        if proposal.status != "awaiting_approval" {
            return Err("The proposal is no longer available for denial".to_string());
        }
        let resolved_at = now_ms();
        let changed = transaction
            .execute(
                "UPDATE proposals
                 SET status = 'denied', resolved_at = ?1
                 WHERE id = ?2 AND status = 'awaiting_approval' AND arguments_hash = ?3",
                params![resolved_at, proposal_id, arguments_hash],
            )
            .map_err(|_| "Unable to deny the proposal".to_string())?;
        if changed != 1 {
            return Err("The proposal changed while denial was being processed".to_string());
        }
        proposal.status = "denied".to_string();
        proposal.resolved_at = Some(resolved_at);
        transaction
            .commit()
            .map_err(|_| "Unable to finish proposal denial".to_string())?;
        Ok(proposal)
    }

    pub fn insert_job(&self, job: &JobRecord) -> SqlResult<()> {
        self.connection.execute(
            "INSERT INTO jobs
             (id, proposal_id, kind, status, workspace_path, started_at, completed_at, result_json, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                job.id,
                job.proposal_id,
                job.kind,
                job.status,
                job.workspace_path,
                job.started_at,
                job.completed_at,
                job.result_json,
                job.error_message,
            ],
        )?;
        Ok(())
    }

    pub fn job_by_proposal(&self, proposal_id: &str) -> SqlResult<Option<JobRecord>> {
        self.connection
            .query_row(
                "SELECT id, proposal_id, kind, status, workspace_path, started_at, completed_at,
                        result_json, error_message FROM jobs WHERE proposal_id = ?1
                 ORDER BY started_at DESC LIMIT 1",
                params![proposal_id],
                job_from_row,
            )
            .optional()
    }

    pub fn active_job(&self) -> SqlResult<Option<JobRecord>> {
        self.connection
            .query_row(
                "SELECT id, proposal_id, kind, status, workspace_path, started_at, completed_at,
                        result_json, error_message FROM jobs
                 WHERE status IN ('preparing', 'running') ORDER BY started_at LIMIT 1",
                [],
                job_from_row,
            )
            .optional()
    }

    pub fn mark_job_running(&self, job_id: &str) -> SqlResult<JobRecord> {
        self.connection.execute(
            "UPDATE jobs SET status = 'running' WHERE id = ?1 AND status = 'preparing'",
            params![job_id],
        )?;
        self.job_by_id(job_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn finish_job(
        &mut self,
        job_id: &str,
        status: &str,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> SqlResult<JobRecord> {
        let completed_at = now_ms();
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let proposal_id: String = transaction.query_row(
            "SELECT proposal_id FROM jobs WHERE id = ?1",
            params![job_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE jobs SET status = ?2, result_json = ?3, error_message = ?4, completed_at = ?5
             WHERE id = ?1",
            params![job_id, status, result_json, error_message, completed_at],
        )?;
        let proposal_status = if status == "completed" { "completed" } else { "failed" };
        transaction.execute(
            "UPDATE proposals SET status = ?2, resolved_at = ?3
             WHERE id = ?1 AND status = 'running'",
            params![proposal_id, proposal_status, completed_at],
        )?;
        transaction.commit()?;
        self.job_by_id(job_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    fn job_by_id(&self, job_id: &str) -> SqlResult<Option<JobRecord>> {
        self.connection
            .query_row(
                "SELECT id, proposal_id, kind, status, workspace_path, started_at, completed_at,
                        result_json, error_message FROM jobs WHERE id = ?1",
                params![job_id],
                job_from_row,
            )
            .optional()
    }

    pub fn append_audit(
        &self,
        entity_type: &str,
        entity_id: &str,
        event_type: &str,
        details_json: Option<&str>,
    ) -> SqlResult<AuditEventRecord> {
        let record = AuditEventRecord {
            id: new_id(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            event_type: event_type.to_string(),
            details_json: details_json.map(str::to_string),
            created_at: now_ms(),
        };
        self.connection.execute(
            "INSERT INTO audit_events (id, entity_type, entity_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.id,
                record.entity_type,
                record.entity_id,
                record.event_type,
                record.details_json,
                record.created_at,
            ],
        )?;
        Ok(record)
    }
}

fn proposal_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProposalRecord> {
    Ok(ProposalRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        tool_name: row.get(2)?,
        arguments_json: row.get(3)?,
        arguments_hash: row.get(4)?,
        preview: row.get(5)?,
        approval_policy: row.get(6)?,
        status: row.get(7)?,
        provider_context_json: row.get(8)?,
        created_at: row.get(9)?,
        expires_at: row.get(10)?,
        resolved_at: row.get(11)?,
    })
}

fn job_from_row(row: &rusqlite::Row<'_>) -> SqlResult<JobRecord> {
    Ok(JobRecord {
        id: row.get(0)?,
        proposal_id: row.get(1)?,
        kind: row.get(2)?,
        status: row.get(3)?,
        workspace_path: row.get(4)?,
        started_at: row.get(5)?,
        completed_at: row.get(6)?,
        result_json: row.get(7)?,
        error_message: row.get(8)?,
    })
}

fn provider_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProviderRecord> {
    Ok(ProviderRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        model: row.get(3)?,
        is_active: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn rank_records<T, F>(records: Vec<T>, query: &str, limit: usize, content: F) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    if limit == 0 {
        return Vec::new();
    }
    let query_tokens = searchable_tokens(query);
    if query_tokens.is_empty() {
        return records.into_iter().take(limit).collect();
    }
    let mut matches = records
        .into_iter()
        .filter_map(|record| {
            let score = searchable_tokens(content(&record))
                .iter()
                .filter(|token| query_tokens.contains(*token))
                .count();
            (score > 0).then_some((score, record))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.0.cmp(&left.0));
    matches
        .into_iter()
        .take(limit)
        .map(|(_, record)| record)
        .collect()
}

fn searchable_tokens(value: &str) -> std::collections::HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "about", "are", "did", "do", "i", "is", "me", "memory", "memories", "my",
        "note", "notes", "please", "remember", "tell", "the", "what", "you",
    ];
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.to_ascii_lowercase();
            (!token.is_empty() && !STOP_WORDS.contains(&token.as_str())).then_some(token)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    #[test]
    fn note_insert_creates_its_audit_row() {
        let path = env::temp_dir().join(format!("daemon-storage-{}.sqlite3", Uuid::new_v4()));
        let mut storage = Storage::open(&path).expect("temporary database should open");
        let conversation = storage
            .create_conversation()
            .expect("conversation should be created");
        let message = storage
            .append_message(&conversation.id, "user", "I need to fix the login bug tomorrow")
            .expect("message should be created");
        let note = storage
            .insert_note_with_audit("Fix the login bug tomorrow", &message.id)
            .expect("note should be created");
        let audit: (String, String, String, i64) = storage
            .connection
            .query_row(
                "SELECT entity_type, event_type, details_json, created_at
                 FROM audit_events WHERE entity_id = ?1",
                params![note.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("note audit should exist");

        assert_eq!(audit.0, "note");
        assert_eq!(audit.1, "created");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&audit.2)
                .expect("note audit details should be JSON"),
            serde_json::json!({ "content": "Fix the login bug tomorrow" }),
        );
        assert_eq!(audit.3, note.created_at);
        drop(storage);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn memory_insert_creates_its_audit_row() {
        let path = env::temp_dir().join(format!("daemon-memory-{}.sqlite3", Uuid::new_v4()));
        let mut storage = Storage::open(&path).expect("temporary database should open");
        let conversation = storage
            .create_conversation()
            .expect("conversation should be created");
        let message = storage
            .append_message(&conversation.id, "user", "My favorite color is purple")
            .expect("message should be created");
        let memory = storage
            .insert_memory_with_audit("Favorite color: purple", &message.id)
            .expect("memory should be created");
        let audit: (String, String, String, i64) = storage
            .connection
            .query_row(
                "SELECT entity_type, event_type, details_json, created_at
                 FROM audit_events WHERE entity_id = ?1",
                params![memory.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("memory audit should exist");

        assert_eq!(audit.0, "memory");
        assert_eq!(audit.1, "created");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&audit.2)
                .expect("memory audit details should be JSON"),
            serde_json::json!({ "content": "Favorite color: purple" }),
        );
        assert_eq!(audit.3, memory.created_at);
        drop(storage);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn local_search_returns_relevant_active_records() {
        let path = env::temp_dir().join(format!("daemon-search-{}.sqlite3", Uuid::new_v4()));
        let mut storage = Storage::open(&path).expect("temporary database should open");
        let conversation = storage
            .create_conversation()
            .expect("conversation should be created");
        let message = storage
            .append_message(&conversation.id, "user", "I prefer purple and need a budget reminder")
            .expect("message should be created");
        storage
            .insert_memory_with_audit("Favorite color: purple", &message.id)
            .expect("memory should be created");
        let note = storage
            .insert_note_with_audit("Review the project budget", &message.id)
            .expect("note should be created");

        assert_eq!(
            storage
                .search_memories("favorite color", 5)
                .expect("memory search should work")[0]
                .content,
            "Favorite color: purple"
        );
        assert_eq!(
            storage
                .search_notes("budget", 5)
                .expect("note search should work")[0]
                .id,
            note.id
        );
        storage
            .soft_delete_note_with_audit(&note.id)
            .expect("note should be deleted");
        assert!(storage
            .search_notes("budget", 5)
            .expect("note search should work")
            .is_empty());
        drop(storage);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn note_insert_rolls_back_when_its_audit_insert_fails() {
        let path = env::temp_dir().join(format!("daemon-storage-{}.sqlite3", Uuid::new_v4()));
        let mut storage = Storage::open(&path).expect("temporary database should open");
        let conversation = storage
            .create_conversation()
            .expect("conversation should be created");
        let message = storage
            .append_message(&conversation.id, "user", "Remind me tomorrow")
            .expect("message should be created");
        storage
            .connection
            .execute_batch(
                "CREATE TRIGGER fail_note_audit
                 BEFORE INSERT ON audit_events
                 WHEN NEW.entity_type = 'note'
                 BEGIN
                   SELECT RAISE(ABORT, 'forced audit failure');
                 END;",
            )
            .expect("failure trigger should be created");

        assert!(storage
            .insert_note_with_audit("Remember tomorrow", &message.id)
            .is_err());

        let note_count: i64 = storage
            .connection
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE source_message_id = ?1",
                params![message.id],
                |row| row.get(0),
            )
            .expect("note count should load");
        let audit_count: i64 = storage
            .connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE entity_type = 'note'",
                [],
                |row| row.get(0),
            )
            .expect("audit count should load");

        assert_eq!(note_count, 0);
        assert_eq!(audit_count, 0);
        drop(storage);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn soft_delete_note_creates_a_matching_audit_event() {
        let path = env::temp_dir().join(format!("daemon-storage-{}.sqlite3", Uuid::new_v4()));
        let mut storage = Storage::open(&path).expect("temporary database should open");
        let conversation = storage
            .create_conversation()
            .expect("conversation should be created");
        let message = storage
            .append_message(&conversation.id, "user", "Remind me tomorrow")
            .expect("message should be created");
        let note = storage
            .insert_note_with_audit("Remember tomorrow", &message.id)
            .expect("note should be created");

        assert!(storage
            .soft_delete_note_with_audit(&note.id)
            .expect("note should be soft deleted"));

        let deleted: (Option<i64>, i64) = storage
            .connection
            .query_row(
                "SELECT notes.deleted_at, audit_events.created_at
                 FROM notes
                 INNER JOIN audit_events ON audit_events.entity_id = notes.id
                 WHERE notes.id = ?1 AND audit_events.event_type = 'deleted'",
                params![note.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("deleted note audit should exist");

        assert_eq!(deleted.0, Some(deleted.1));
        assert!(!storage
            .soft_delete_note_with_audit(&note.id)
            .expect("duplicate deletion should be safe"));
        drop(storage);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn screen_observations_store_text_only() {
        let path = env::temp_dir().join(format!("daemon-screen-{}.sqlite3", Uuid::new_v4()));
        let storage = Storage::open(&path).expect("temporary database should open");
        assert_eq!(
            storage
                .screen_aware_settings()
                .expect("default screen settings should load")
                .interval_seconds,
            None
        );
        let settings = storage
            .save_screen_aware_settings(Some(30))
            .expect("screen settings should save");
        assert_eq!(settings.interval_seconds, Some(30));
        let observation = storage
            .insert_screen_observation("A code editor is open.", "automatic")
            .expect("screen description should save");
        let recent = storage
            .recent_screen_observations(0, 1)
            .expect("screen descriptions should load");
        let schema: String = storage
            .connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'screen_observations'",
                [],
                |row| row.get(0),
            )
            .expect("screen observation schema should exist");

        assert_eq!(recent[0].id, observation.id);
        assert_eq!(recent[0].description, "A code editor is open.");
        assert_eq!(recent[0].source, "automatic");
        assert_eq!(observation.timestamp, observation.created_at);
        assert!(!schema.to_ascii_lowercase().contains("blob"));
        drop(storage);
        let _ = fs::remove_file(path);
    }
}

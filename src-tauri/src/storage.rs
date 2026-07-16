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
             CREATE INDEX IF NOT EXISTS messages_conversation_idx ON messages(conversation_id, created_at);
             CREATE INDEX IF NOT EXISTS notes_source_idx ON notes(source_message_id, created_at);
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

    pub fn active_notes_for_source(&self, source_message_id: &str) -> SqlResult<Vec<NoteRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, content, source_message_id, created_at, deleted_at
             FROM notes
             WHERE source_message_id = ?1 AND deleted_at IS NULL
             ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map(params![source_message_id], |row| {
            Ok(NoteRecord {
                id: row.get(0)?,
                content: row.get(1)?,
                source_message_id: row.get(2)?,
                created_at: row.get(3)?,
                deleted_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn insert_note(
        &self,
        content: &str,
        source_message_id: &str,
    ) -> SqlResult<NoteRecord> {
        let record = NoteRecord {
            id: new_id(),
            content: content.to_string(),
            source_message_id: source_message_id.to_string(),
            created_at: now_ms(),
            deleted_at: None,
        };
        self.connection.execute(
            "INSERT INTO notes (id, content, source_message_id, created_at, deleted_at) VALUES (?1, ?2, ?3, ?4, NULL)",
            params![record.id, record.content, record.source_message_id, record.created_at],
        )?;
        Ok(record)
    }

    pub fn soft_delete_note(&self, note_id: &str) -> SqlResult<bool> {
        let changed = self.connection.execute(
            "UPDATE notes SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now_ms(), note_id],
        )?;
        Ok(changed == 1)
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

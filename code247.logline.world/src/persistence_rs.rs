use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::supabase_sync_rs::{
    Code247CheckpointMirror, Code247EventMirror, Code247JobMirror, SupabaseSyncHandle,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Planning,
    Coding,
    Reviewing,
    Validating,
    Committing,
    Failed,
    Done,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Pending => "PENDING",
            JobStatus::Planning => "PLANNING",
            JobStatus::Coding => "CODING",
            JobStatus::Reviewing => "REVIEWING",
            JobStatus::Validating => "VALIDATING",
            JobStatus::Committing => "COMMITTING",
            JobStatus::Failed => "FAILED",
            JobStatus::Done => "DONE",
        }
    }

    fn from_db(v: &str) -> Self {
        match v {
            "PLANNING" => JobStatus::Planning,
            "CODING" => JobStatus::Coding,
            "REVIEWING" => JobStatus::Reviewing,
            "VALIDATING" => JobStatus::Validating,
            "COMMITTING" => JobStatus::Committing,
            "FAILED" => JobStatus::Failed,
            "DONE" => JobStatus::Done,
            _ => JobStatus::Pending,
        }
    }

    pub fn has_stage_lease(self) -> bool {
        matches!(
            self,
            JobStatus::Planning
                | JobStatus::Coding
                | JobStatus::Reviewing
                | JobStatus::Validating
                | JobStatus::Committing
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub issue_id: String,
    pub status: JobStatus,
    pub payload: String,
    pub retries: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLeaseRecord {
    pub id: String,
    pub issue_id: String,
    pub status: JobStatus,
    pub last_error: Option<String>,
    pub stage_started_at: Option<String>,
    pub heartbeat_at: Option<String>,
    pub lease_expires_at: Option<String>,
    pub lease_owner: Option<String>,
    pub stage_attempt: i32,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct SqliteDb {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteDb {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub fn run_migrations(&self) -> Result<()> {
        self.conn.lock().expect("db lock").execute_batch(
            "
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                issue_id TEXT NOT NULL,
                status TEXT NOT NULL,
                payload TEXT NOT NULL,
                retries INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                stage_started_at TEXT,
                heartbeat_at TEXT,
                lease_expires_at TEXT,
                lease_owner TEXT,
                stage_attempt INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS checkpoints (
                job_id TEXT NOT NULL,
                stage TEXT NOT NULL,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS execution_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                stage TEXT NOT NULL,
                input TEXT,
                output TEXT,
                model_used TEXT,
                duration_ms INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS oauth_states (
                state TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                consumed_at TEXT
            );
            CREATE TABLE IF NOT EXISTS linear_oauth_tokens (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                access_token TEXT NOT NULL,
                refresh_token TEXT NOT NULL,
                token_type TEXT NOT NULL,
                scope TEXT,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS intention_links (
                workspace TEXT NOT NULL,
                project TEXT NOT NULL,
                intention_id TEXT NOT NULL,
                linear_issue_id TEXT NOT NULL,
                linear_identifier TEXT,
                last_manifest_updated_at TEXT NOT NULL,
                last_revision TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (workspace, project, intention_id)
            );
            CREATE TABLE IF NOT EXISTS manifest_ingestions (
                workspace TEXT NOT NULL,
                project TEXT NOT NULL,
                last_updated_at TEXT NOT NULL,
                last_revision TEXT,
                last_request_id TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (workspace, project)
            );
            CREATE TABLE IF NOT EXISTS linear_webhook_deliveries (
                delivery_id TEXT PRIMARY KEY,
                linear_event TEXT,
                issue_id TEXT,
                payload TEXT NOT NULL,
                signature TEXT,
                status TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                next_attempt_at TEXT NOT NULL,
                last_error TEXT,
                received_at TEXT NOT NULL,
                processed_at TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS linear_action_outbox (
                id TEXT PRIMARY KEY,
                issue_id TEXT NOT NULL,
                action_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                next_attempt_at TEXT NOT NULL,
                last_error TEXT,
                created_at TEXT NOT NULL,
                processed_at TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE VIEW IF NOT EXISTS code247_run_timeline AS
                SELECT
                    jobs.id AS job_id,
                    jobs.issue_id AS issue_id,
                    'job_created' AS event_kind,
                    jobs.status AS stage,
                    jobs.status AS status,
                    jobs.payload AS detail,
                    jobs.created_at AS created_at
                FROM jobs
                UNION ALL
                SELECT
                    jobs.id AS job_id,
                    jobs.issue_id AS issue_id,
                    'job_status' AS event_kind,
                    jobs.status AS stage,
                    jobs.status AS status,
                    COALESCE(jobs.last_error, '') AS detail,
                    jobs.updated_at AS created_at
                FROM jobs
                UNION ALL
                SELECT
                    checkpoints.job_id AS job_id,
                    jobs.issue_id AS issue_id,
                    'checkpoint' AS event_kind,
                    checkpoints.stage AS stage,
                    NULL AS status,
                    checkpoints.data AS detail,
                    checkpoints.created_at AS created_at
                FROM checkpoints
                LEFT JOIN jobs ON jobs.id = checkpoints.job_id
                UNION ALL
                SELECT
                    execution_log.job_id AS job_id,
                    jobs.issue_id AS issue_id,
                    'execution' AS event_kind,
                    execution_log.stage AS stage,
                    NULL AS status,
                    COALESCE(execution_log.output, execution_log.input, '') AS detail,
                    execution_log.created_at AS created_at
                FROM execution_log
                LEFT JOIN jobs ON jobs.id = execution_log.job_id
                UNION ALL
                SELECT
                    linear_action_outbox.id AS job_id,
                    linear_action_outbox.issue_id AS issue_id,
                    'linear_outbox' AS event_kind,
                    linear_action_outbox.action_type AS stage,
                    linear_action_outbox.status AS status,
                    COALESCE(linear_action_outbox.last_error, linear_action_outbox.payload, '') AS detail,
                    linear_action_outbox.updated_at AS created_at
                FROM linear_action_outbox;
            ",
        )?;
        self.ensure_jobs_column("stage_started_at", "TEXT")?;
        self.ensure_jobs_column("heartbeat_at", "TEXT")?;
        self.ensure_jobs_column("lease_expires_at", "TEXT")?;
        self.ensure_jobs_column("lease_owner", "TEXT")?;
        self.ensure_jobs_column("stage_attempt", "INTEGER NOT NULL DEFAULT 0")?;
        Ok(())
    }

    fn ensure_jobs_column(&self, name: &str, definition: &str) -> Result<()> {
        let exists = {
            let conn = self.conn.lock().expect("db lock");
            let mut stmt = conn.prepare("PRAGMA table_info(jobs)")?;
            let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let names = columns.flatten().collect::<Vec<_>>();
            names.iter().any(|column| column == name)
        };
        if exists {
            return Ok(());
        }
        self.conn.lock().expect("db lock").execute(
            &format!("ALTER TABLE jobs ADD COLUMN {name} {definition}"),
            [],
        )?;
        Ok(())
    }
}

pub struct JobsRepository {
    conn: Arc<Mutex<Connection>>,
    sync: Option<SupabaseSyncHandle>,
}

impl JobsRepository {
    pub fn new(conn: Arc<Mutex<Connection>>, sync: Option<SupabaseSyncHandle>) -> Self {
        Self { conn, sync }
    }

    pub fn claim_next_pending_with_lease(
        &mut self,
        lease_owner: &str,
        lease_timeout_seconds: i64,
    ) -> Option<Job> {
        loop {
            let candidate = {
                let conn = self.conn.lock().expect("db lock");
                conn.query_row(
                    "SELECT id, issue_id, payload, retries
                     FROM jobs
                     WHERE status='PENDING'
                     ORDER BY created_at ASC
                     LIMIT 1",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i32>(3)?,
                        ))
                    },
                )
                .ok()
            };

            let Some((id, issue_id, payload, retries)) = candidate else {
                return None;
            };

            let now = Utc::now();
            let changed = self
                .conn
                .lock()
                .expect("db lock")
                .execute(
                    "UPDATE jobs
                     SET status=?,
                         last_error=NULL,
                         updated_at=?,
                         stage_started_at=?,
                         heartbeat_at=?,
                         lease_expires_at=?,
                         lease_owner=?,
                         stage_attempt=COALESCE(stage_attempt, 0) + 1
                     WHERE id=? AND status='PENDING'",
                    params![
                        JobStatus::Planning.as_str(),
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                        (now + Duration::seconds(lease_timeout_seconds.max(5))).to_rfc3339(),
                        lease_owner,
                        id,
                    ],
                )
                .unwrap_or(0)
                > 0;
            if changed {
                self.emit_job_snapshot(&id);
                return Some(Job {
                    id,
                    issue_id,
                    status: JobStatus::Planning,
                    payload,
                    retries,
                });
            }
        }
    }

    pub fn update_status(&mut self, id: &str, status: JobStatus, error: Option<String>) {
        let now = Utc::now().to_rfc3339();
        let has_lease = status.has_stage_lease();
        let _ = self.conn.lock().expect("db lock").execute(
            "UPDATE jobs
             SET status=?,
                 last_error=?,
                 updated_at=?,
                 stage_started_at=CASE WHEN ? = 1 THEN stage_started_at ELSE NULL END,
                 heartbeat_at=CASE WHEN ? = 1 THEN heartbeat_at ELSE NULL END,
                 lease_expires_at=CASE WHEN ? = 1 THEN lease_expires_at ELSE NULL END,
                 lease_owner=CASE WHEN ? = 1 THEN lease_owner ELSE NULL END,
                 stage_attempt=CASE WHEN ? = 1 THEN stage_attempt ELSE 0 END
             WHERE id=?",
            params![
                status.as_str(),
                error,
                now,
                has_lease as i32,
                has_lease as i32,
                has_lease as i32,
                has_lease as i32,
                has_lease as i32,
                id
            ],
        );
        self.emit_job_snapshot(id);
    }

    pub fn transition_status(
        &mut self,
        id: &str,
        from: JobStatus,
        to: JobStatus,
        error: Option<String>,
    ) -> bool {
        let changed = self
            .conn
            .lock()
            .expect("db lock")
            .execute(
                "UPDATE jobs
                 SET status=?, last_error=?, updated_at=?
                 WHERE id=? AND status=?",
                params![
                    to.as_str(),
                    error,
                    Utc::now().to_rfc3339(),
                    id,
                    from.as_str()
                ],
            )
            .unwrap_or(0)
            > 0;
        if changed {
            self.emit_job_snapshot(id);
        }
        changed
    }

    pub fn transition_status_with_lease(
        &mut self,
        id: &str,
        from: JobStatus,
        to: JobStatus,
        error: Option<String>,
        lease_owner: Option<&str>,
        lease_timeout_seconds: Option<i64>,
    ) -> bool {
        let now = Utc::now();
        let has_lease = to.has_stage_lease();
        let stage_started_at = has_lease.then(|| now.to_rfc3339());
        let heartbeat_at = has_lease.then(|| now.to_rfc3339());
        let lease_expires_at = if has_lease {
            Some(
                (now + Duration::seconds(lease_timeout_seconds.unwrap_or(300).max(5))).to_rfc3339(),
            )
        } else {
            None
        };
        let changed = self
            .conn
            .lock()
            .expect("db lock")
            .execute(
                "UPDATE jobs
                 SET status=?,
                     last_error=?,
                     updated_at=?,
                     stage_started_at=?,
                     heartbeat_at=?,
                     lease_expires_at=?,
                     lease_owner=?,
                     stage_attempt=CASE WHEN ? = 1 THEN COALESCE(stage_attempt, 0) + 1 ELSE 0 END
                 WHERE id=? AND status=?",
                params![
                    to.as_str(),
                    error,
                    now.to_rfc3339(),
                    stage_started_at,
                    heartbeat_at,
                    lease_expires_at,
                    if has_lease { lease_owner } else { None },
                    has_lease as i32,
                    id,
                    from.as_str()
                ],
            )
            .unwrap_or(0)
            > 0;
        if changed {
            self.emit_job_snapshot(id);
        }
        changed
    }

    pub fn renew_stage_lease(
        &mut self,
        id: &str,
        expected_status: JobStatus,
        lease_owner: &str,
        lease_timeout_seconds: i64,
    ) -> bool {
        if !expected_status.has_stage_lease() {
            return false;
        }
        let now = Utc::now();
        let changed = self
            .conn
            .lock()
            .expect("db lock")
            .execute(
                "UPDATE jobs
                 SET heartbeat_at=?,
                     lease_expires_at=?,
                     lease_owner=?,
                     updated_at=?
                 WHERE id=? AND status=? AND COALESCE(lease_owner, '')=?",
                params![
                    now.to_rfc3339(),
                    (now + Duration::seconds(lease_timeout_seconds.max(5))).to_rfc3339(),
                    lease_owner,
                    now.to_rfc3339(),
                    id,
                    expected_status.as_str(),
                    lease_owner
                ],
            )
            .unwrap_or(0)
            > 0;
        if changed {
            self.emit_job_snapshot(id);
        }
        changed
    }

    pub fn create_job(&mut self, issue_id: &str, payload: &str) -> Result<Job> {
        let now = Utc::now().to_rfc3339();
        let job = Job {
            id: Uuid::new_v4().to_string(),
            issue_id: issue_id.to_string(),
            status: JobStatus::Pending,
            payload: payload.to_string(),
            retries: 0,
        };

        self.conn.lock().expect("db lock").execute(
            "INSERT INTO jobs (id, issue_id, status, payload, retries, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                job.id,
                job.issue_id,
                job.status.as_str(),
                job.payload,
                job.retries,
                now,
                now,
            ],
        )?;

        self.emit_job_snapshot(&job.id);

        Ok(job)
    }

    pub fn increment_retries(&mut self, id: &str) {
        let _ = self.conn.lock().expect("db lock").execute(
            "UPDATE jobs SET retries=retries+1, updated_at=? WHERE id=?",
            params![Utc::now().to_rfc3339(), id],
        );
        self.emit_job_snapshot(id);
    }

    pub fn list_recent(&self) -> Vec<Job> {
        let conn = self.conn.lock().expect("db lock");
        let mut stmt = conn.prepare("SELECT id, issue_id, status, payload, retries FROM jobs ORDER BY created_at DESC LIMIT 20").expect("stmt");
        stmt.query_map([], |row| {
            Ok(Job {
                id: row.get(0)?,
                issue_id: row.get(1)?,
                status: JobStatus::from_db(&row.get::<_, String>(2)?),
                payload: row.get(3)?,
                retries: row.get(4)?,
            })
        })
        .expect("query")
        .flatten()
        .collect()
    }

    pub fn has_non_failed_job_for_issue(&self, issue_id: &str) -> bool {
        let conn = self.conn.lock().expect("db lock");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM jobs WHERE issue_id=? AND status!='FAILED'",
                params![issue_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        count > 0
    }

    pub fn list_expired_stage_leases(&self, now: &DateTime<Utc>) -> Vec<JobLeaseRecord> {
        let conn = self.conn.lock().expect("db lock");
        let mut stmt = conn
            .prepare(
                "SELECT id, issue_id, status, last_error, stage_started_at, heartbeat_at,
                        lease_expires_at, lease_owner, stage_attempt, updated_at
                 FROM jobs
                 WHERE status IN ('PLANNING', 'CODING', 'REVIEWING', 'VALIDATING', 'COMMITTING')
                   AND lease_expires_at IS NOT NULL
                   AND lease_expires_at <= ?
                 ORDER BY lease_expires_at ASC, updated_at ASC",
            )
            .expect("stmt");
        stmt.query_map(params![now.to_rfc3339()], |row| {
            Ok(JobLeaseRecord {
                id: row.get(0)?,
                issue_id: row.get(1)?,
                status: JobStatus::from_db(&row.get::<_, String>(2)?),
                last_error: row.get(3)?,
                stage_started_at: row.get(4)?,
                heartbeat_at: row.get(5)?,
                lease_expires_at: row.get(6)?,
                lease_owner: row.get(7)?,
                stage_attempt: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .expect("query")
        .flatten()
        .collect()
    }

    pub fn expire_stage_lease(
        &mut self,
        id: &str,
        expected_status: JobStatus,
        expected_lease_owner: Option<&str>,
        expected_lease_expires_at: Option<&str>,
        error: &str,
    ) -> bool {
        let now = Utc::now();
        let changed = self
            .conn
            .lock()
            .expect("db lock")
            .execute(
                "UPDATE jobs
                 SET status=?,
                     last_error=?,
                     updated_at=?,
                     stage_started_at=NULL,
                     heartbeat_at=NULL,
                     lease_expires_at=NULL,
                     lease_owner=NULL,
                     stage_attempt=0
                 WHERE id=?
                   AND status=?
                   AND COALESCE(lease_owner, '') = COALESCE(?, '')
                   AND COALESCE(lease_expires_at, '') = COALESCE(?, '')",
                params![
                    JobStatus::Failed.as_str(),
                    error,
                    now.to_rfc3339(),
                    id,
                    expected_status.as_str(),
                    expected_lease_owner,
                    expected_lease_expires_at,
                ],
            )
            .unwrap_or(0)
            > 0;
        if changed {
            self.emit_job_snapshot(id);
        }
        changed
    }

    fn emit_job_snapshot(&self, job_id: &str) {
        let Some(sync) = &self.sync else {
            return;
        };
        if let Some(snapshot) = self.load_job_snapshot(job_id) {
            sync.enqueue_job_upsert(snapshot);
        }
    }

    fn load_job_snapshot(&self, job_id: &str) -> Option<Code247JobMirror> {
        let conn = self.conn.lock().expect("db lock");
        conn.query_row(
            "SELECT id, issue_id, status, payload, retries, last_error, created_at, updated_at
             FROM jobs
             WHERE id=?",
            params![job_id],
            |row| {
                let payload_raw: String = row.get(3)?;
                let payload = serde_json::from_str::<Value>(&payload_raw)
                    .unwrap_or_else(|_| json!({ "raw_payload": payload_raw }));
                Ok(Code247JobMirror {
                    id: row.get(0)?,
                    issue_id: row.get(1)?,
                    status: row.get(2)?,
                    payload,
                    retries: row.get(4)?,
                    last_error: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearOAuthTokenRecord {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: String,
    pub updated_at: String,
}

pub struct OAuthStateRepository {
    conn: Arc<Mutex<Connection>>,
}

impl OAuthStateRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn create_state(&self, ttl_seconds: i64) -> Result<String> {
        let state = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_seconds.max(60));

        self.conn.lock().expect("db lock").execute(
            "INSERT INTO oauth_states (state, created_at, expires_at) VALUES (?, ?, ?)",
            params![state, now.to_rfc3339(), expires_at.to_rfc3339()],
        )?;

        Ok(state)
    }

    pub fn consume_state(&self, state: &str) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let affected = self.conn.lock().expect("db lock").execute(
            "UPDATE oauth_states
             SET consumed_at=?
             WHERE state=?
               AND consumed_at IS NULL
               AND expires_at > ?",
            params![now, state, now],
        )?;

        Ok(affected > 0)
    }

    pub fn cleanup_expired(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let deleted = self.conn.lock().expect("db lock").execute(
            "DELETE FROM oauth_states
             WHERE expires_at <= ?
                OR (consumed_at IS NOT NULL AND consumed_at <= ?)",
            params![now, now],
        )?;

        Ok(deleted)
    }
}

pub struct LinearOAuthTokenRepository {
    conn: Arc<Mutex<Connection>>,
}

impl LinearOAuthTokenRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn upsert_token(
        &self,
        access_token: &str,
        refresh_token: &str,
        token_type: &str,
        scope: Option<&str>,
        expires_at: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().expect("db lock").execute(
            "INSERT INTO linear_oauth_tokens (
                id, access_token, refresh_token, token_type, scope, expires_at, created_at, updated_at
             ) VALUES (1, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                access_token=excluded.access_token,
                refresh_token=excluded.refresh_token,
                token_type=excluded.token_type,
                scope=excluded.scope,
                expires_at=excluded.expires_at,
                updated_at=excluded.updated_at",
            params![
                access_token,
                refresh_token,
                token_type,
                scope,
                expires_at,
                now,
                now
            ],
        )?;
        Ok(())
    }

    pub fn get_token(&self) -> Option<LinearOAuthTokenRecord> {
        let conn = self.conn.lock().expect("db lock");
        conn.query_row(
            "SELECT access_token, refresh_token, token_type, scope, expires_at, updated_at
             FROM linear_oauth_tokens
             WHERE id = 1",
            [],
            |row| {
                Ok(LinearOAuthTokenRecord {
                    access_token: row.get(0)?,
                    refresh_token: row.get(1)?,
                    token_type: row.get(2)?,
                    scope: row.get(3)?,
                    expires_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
        .ok()
    }

    pub fn token_due_for_refresh(&self, lead_seconds: i64) -> Option<LinearOAuthTokenRecord> {
        let token = self.get_token()?;
        match DateTime::parse_from_rfc3339(&token.expires_at) {
            Ok(expires_at) => {
                let threshold = Utc::now() + Duration::seconds(lead_seconds.max(30));
                if expires_at.with_timezone(&Utc) <= threshold {
                    Some(token)
                } else {
                    None
                }
            }
            Err(_) => Some(token),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionLinkRecord {
    pub workspace: String,
    pub project: String,
    pub intention_id: String,
    pub linear_issue_id: String,
    pub linear_identifier: Option<String>,
    pub last_manifest_updated_at: String,
    pub last_revision: Option<String>,
    pub updated_at: String,
}

pub struct IntentionLinkRepository {
    conn: Arc<Mutex<Connection>>,
}

impl IntentionLinkRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_link(
        &self,
        workspace: &str,
        project: &str,
        intention_id: &str,
    ) -> Option<IntentionLinkRecord> {
        let conn = self.conn.lock().expect("db lock");
        conn.query_row(
            "SELECT workspace, project, intention_id, linear_issue_id, linear_identifier, last_manifest_updated_at, last_revision, updated_at
             FROM intention_links
             WHERE workspace=? AND project=? AND intention_id=?",
            params![workspace, project, intention_id],
            |row| {
                Ok(IntentionLinkRecord {
                    workspace: row.get(0)?,
                    project: row.get(1)?,
                    intention_id: row.get(2)?,
                    linear_issue_id: row.get(3)?,
                    linear_identifier: row.get(4)?,
                    last_manifest_updated_at: row.get(5)?,
                    last_revision: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .ok()
    }

    pub fn upsert_link(
        &self,
        workspace: &str,
        project: &str,
        intention_id: &str,
        linear_issue_id: &str,
        linear_identifier: Option<&str>,
        manifest_updated_at: &str,
        revision: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().expect("db lock").execute(
            "INSERT INTO intention_links (
                workspace, project, intention_id, linear_issue_id, linear_identifier, last_manifest_updated_at, last_revision, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(workspace, project, intention_id) DO UPDATE SET
                linear_issue_id=excluded.linear_issue_id,
                linear_identifier=excluded.linear_identifier,
                last_manifest_updated_at=excluded.last_manifest_updated_at,
                last_revision=excluded.last_revision,
                updated_at=excluded.updated_at",
            params![
                workspace,
                project,
                intention_id,
                linear_issue_id,
                linear_identifier,
                manifest_updated_at,
                revision,
                now
            ],
        )?;
        Ok(())
    }

    pub fn list_project_links(&self, workspace: &str, project: &str) -> Vec<IntentionLinkRecord> {
        let conn = self.conn.lock().expect("db lock");
        let mut stmt = conn
            .prepare(
                "SELECT workspace, project, intention_id, linear_issue_id, linear_identifier, last_manifest_updated_at, last_revision, updated_at
                 FROM intention_links
                 WHERE workspace=? AND project=?
                 ORDER BY intention_id ASC",
            )
            .expect("stmt");
        stmt.query_map(params![workspace, project], |row| {
            Ok(IntentionLinkRecord {
                workspace: row.get(0)?,
                project: row.get(1)?,
                intention_id: row.get(2)?,
                linear_issue_id: row.get(3)?,
                linear_identifier: row.get(4)?,
                last_manifest_updated_at: row.get(5)?,
                last_revision: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .expect("query")
        .flatten()
        .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestIngestionRecord {
    pub workspace: String,
    pub project: String,
    pub last_updated_at: String,
    pub last_revision: Option<String>,
    pub last_request_id: Option<String>,
    pub updated_at: String,
}

pub struct ManifestIngestionRepository {
    conn: Arc<Mutex<Connection>>,
}

impl ManifestIngestionRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get(&self, workspace: &str, project: &str) -> Option<ManifestIngestionRecord> {
        let conn = self.conn.lock().expect("db lock");
        conn.query_row(
            "SELECT workspace, project, last_updated_at, last_revision, last_request_id, updated_at
             FROM manifest_ingestions
             WHERE workspace=? AND project=?",
            params![workspace, project],
            |row| {
                Ok(ManifestIngestionRecord {
                    workspace: row.get(0)?,
                    project: row.get(1)?,
                    last_updated_at: row.get(2)?,
                    last_revision: row.get(3)?,
                    last_request_id: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
        .ok()
    }

    pub fn upsert(
        &self,
        workspace: &str,
        project: &str,
        last_updated_at: &str,
        last_revision: Option<&str>,
        request_id: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().expect("db lock").execute(
            "INSERT INTO manifest_ingestions (
                workspace, project, last_updated_at, last_revision, last_request_id, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(workspace, project) DO UPDATE SET
                last_updated_at=excluded.last_updated_at,
                last_revision=excluded.last_revision,
                last_request_id=excluded.last_request_id,
                updated_at=excluded.updated_at",
            params![
                workspace,
                project,
                last_updated_at,
                last_revision,
                request_id,
                now
            ],
        )?;
        Ok(())
    }
}

pub struct CheckpointStore {
    conn: Arc<Mutex<Connection>>,
    sync: Option<SupabaseSyncHandle>,
}

impl CheckpointStore {
    pub fn new(conn: Arc<Mutex<Connection>>, sync: Option<SupabaseSyncHandle>) -> Self {
        Self { conn, sync }
    }
    pub fn get_latest(&self, job_id: &str, stage: &str) -> Option<String> {
        let conn = self.conn.lock().expect("db lock");
        conn.query_row(
            "SELECT data FROM checkpoints WHERE job_id=? AND stage=? ORDER BY created_at DESC LIMIT 1",
            params![job_id, stage],
            |row| row.get(0),
        )
        .ok()
    }

    pub fn save(&self, job_id: &str, stage: &str, data: &str) {
        let created_at = Utc::now().to_rfc3339();
        let _ = self.conn.lock().expect("db lock").execute(
            "INSERT INTO checkpoints (job_id, stage, data, created_at) VALUES (?, ?, ?, ?)",
            params![job_id, stage, data, created_at],
        );
        if let Some(sync) = &self.sync {
            sync.enqueue_checkpoint_upsert(Code247CheckpointMirror {
                job_id: job_id.to_string(),
                stage: stage.to_string(),
                data: data.to_string(),
                created_at,
            });
        }
    }
}

pub struct EvidenceStore {
    root: PathBuf,
}

impl EvidenceStore {
    pub fn new(root: String) -> Self {
        Self {
            root: PathBuf::from(root),
        }
    }

    pub fn write(&self, job_id: &str, stage: &str, content: &str) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let file = self.root.join(format!("{}-{}.txt", job_id, stage));
        fs::write(file, content)?;
        Ok(())
    }

    pub fn stage_exists(&self, job_id: &str, stage: &str) -> bool {
        self.root.join(format!("{}-{}.txt", job_id, stage)).exists()
    }

    pub fn missing_stages(&self, job_id: &str, required: &[&str]) -> Vec<String> {
        required
            .iter()
            .filter(|stage| !self.stage_exists(job_id, stage))
            .map(|stage| (*stage).to_string())
            .collect()
    }
}

pub struct ExecutionLogger {
    conn: Arc<Mutex<Connection>>,
    sync: Option<SupabaseSyncHandle>,
}

impl ExecutionLogger {
    pub fn new(conn: Arc<Mutex<Connection>>, sync: Option<SupabaseSyncHandle>) -> Self {
        Self { conn, sync }
    }

    pub fn log_stage(
        &self,
        job_id: &str,
        stage: &str,
        input: &str,
        output: &str,
        model: &str,
        duration_ms: i64,
    ) {
        let created_at = Utc::now().to_rfc3339();
        let stage_norm = normalize_stage_name(stage);
        let event_id = format!("code247:{}:{}:{}", job_id, stage_norm, created_at);
        let event_type = format!("code247.stage.{}", stage_norm);
        let trace_id = format!("code247:job:{}", job_id);
        let issue_id = self.lookup_issue_id(job_id);
        let outcome = if stage_norm.contains("fail") || stage_norm.contains("error") {
            "fail".to_string()
        } else {
            "ok".to_string()
        };
        let _ = self.conn.lock().expect("db lock").execute(
            "INSERT INTO execution_log (job_id, stage, input, output, model_used, duration_ms, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![job_id, stage, input, output, model, duration_ms, created_at],
        );
        if let Some(sync) = &self.sync {
            sync.enqueue_execution_event(Code247EventMirror {
                event_id,
                job_id: job_id.to_string(),
                issue_id,
                stage: stage.to_string(),
                event_type,
                trace_id,
                outcome,
                retry_count: 0,
                fallback_used: false,
                input: Some(input.to_string()),
                output: Some(output.to_string()),
                model_used: Some(model.to_string()),
                duration_ms: Some(duration_ms),
                created_at,
            });
        }
    }

    fn lookup_issue_id(&self, job_id: &str) -> Option<String> {
        self.conn
            .lock()
            .expect("db lock")
            .query_row(
                "SELECT issue_id FROM jobs WHERE id = ?1",
                params![job_id],
                |row| row.get(0),
            )
            .ok()
    }
}

fn normalize_stage_name(stage: &str) -> String {
    let candidate = stage
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let compact = candidate
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if compact.is_empty() {
        format!("stage_{}", Uuid::new_v4())
    } else {
        compact
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunTimelineEntry {
    pub job_id: String,
    pub issue_id: Option<String>,
    pub event_kind: String,
    pub stage: String,
    pub status: Option<String>,
    pub detail: String,
    pub created_at: String,
}

pub struct RunTimelineRepository {
    conn: Arc<Mutex<Connection>>,
}

impl RunTimelineRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn list_for_job(&self, job_id: &str) -> Vec<RunTimelineEntry> {
        let conn = self.conn.lock().expect("db lock");
        let mut stmt = conn
            .prepare(
                "SELECT job_id, issue_id, event_kind, stage, status, detail, created_at
                 FROM code247_run_timeline
                 WHERE job_id = ?
                 ORDER BY created_at ASC, event_kind ASC",
            )
            .expect("stmt");
        stmt.query_map(params![job_id], |row| {
            Ok(RunTimelineEntry {
                job_id: row.get(0)?,
                issue_id: row.get(1)?,
                event_kind: row.get(2)?,
                stage: row.get(3)?,
                status: row.get(4)?,
                detail: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .expect("query")
        .flatten()
        .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebhookDeliveryStatus {
    Queued,
    Retry,
    Processing,
    Done,
    Dlq,
}

impl WebhookDeliveryStatus {
    fn db_value(self) -> &'static str {
        match self {
            WebhookDeliveryStatus::Queued => "QUEUED",
            WebhookDeliveryStatus::Retry => "RETRY",
            WebhookDeliveryStatus::Processing => "PROCESSING",
            WebhookDeliveryStatus::Done => "DONE",
            WebhookDeliveryStatus::Dlq => "DLQ",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearWebhookDelivery {
    pub delivery_id: String,
    pub linear_event: Option<String>,
    pub issue_id: Option<String>,
    pub payload: String,
    pub signature: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub next_attempt_at: String,
    pub last_error: Option<String>,
    pub received_at: String,
    pub processed_at: Option<String>,
    pub updated_at: String,
}

pub struct LinearWebhookDeliveryRepository {
    conn: Arc<Mutex<Connection>>,
}

impl LinearWebhookDeliveryRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn enqueue(
        &self,
        delivery_id: &str,
        linear_event: Option<&str>,
        issue_id: Option<&str>,
        payload: &str,
        signature: Option<&str>,
    ) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let affected = self.conn.lock().expect("db lock").execute(
            "INSERT INTO linear_webhook_deliveries (
                delivery_id, linear_event, issue_id, payload, signature, status, attempts,
                next_attempt_at, received_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
             ON CONFLICT(delivery_id) DO NOTHING",
            params![
                delivery_id,
                linear_event,
                issue_id,
                payload,
                signature,
                WebhookDeliveryStatus::Queued.db_value(),
                now,
                now,
                now
            ],
        )?;
        Ok(affected > 0)
    }

    pub fn claim_next_ready(&self) -> Option<LinearWebhookDelivery> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("db lock");
        let row = conn
            .query_row(
                "SELECT delivery_id, linear_event, issue_id, payload, signature, status,
                        attempts, next_attempt_at, last_error, received_at, processed_at, updated_at
                 FROM linear_webhook_deliveries
                 WHERE status IN ('QUEUED', 'RETRY')
                   AND next_attempt_at <= ?
                 ORDER BY received_at ASC
                 LIMIT 1",
                params![now],
                |row| {
                    Ok(LinearWebhookDelivery {
                        delivery_id: row.get(0)?,
                        linear_event: row.get(1)?,
                        issue_id: row.get(2)?,
                        payload: row.get(3)?,
                        signature: row.get(4)?,
                        status: row.get(5)?,
                        attempts: row.get(6)?,
                        next_attempt_at: row.get(7)?,
                        last_error: row.get(8)?,
                        received_at: row.get(9)?,
                        processed_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    })
                },
            )
            .ok()?;

        let affected = conn
            .execute(
                "UPDATE linear_webhook_deliveries
                 SET status=?, attempts=attempts+1, updated_at=?
                 WHERE delivery_id=?
                   AND status IN ('QUEUED', 'RETRY')",
                params![
                    WebhookDeliveryStatus::Processing.db_value(),
                    Utc::now().to_rfc3339(),
                    row.delivery_id
                ],
            )
            .ok()?;
        if affected == 0 {
            return None;
        }

        Some(LinearWebhookDelivery {
            attempts: row.attempts + 1,
            status: WebhookDeliveryStatus::Processing.db_value().to_string(),
            ..row
        })
    }

    pub fn mark_done(&self, delivery_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().expect("db lock").execute(
            "UPDATE linear_webhook_deliveries
             SET status=?, processed_at=?, last_error=NULL, updated_at=?
             WHERE delivery_id=?",
            params![
                WebhookDeliveryStatus::Done.db_value(),
                now,
                now,
                delivery_id
            ],
        )?;
        Ok(())
    }

    pub fn mark_retry_or_dlq(
        &self,
        delivery_id: &str,
        attempts: i32,
        max_attempts: i32,
        retry_delay_seconds: i64,
        error_message: &str,
    ) -> Result<()> {
        let now = Utc::now();
        if attempts >= max_attempts {
            self.conn.lock().expect("db lock").execute(
                "UPDATE linear_webhook_deliveries
                 SET status=?, processed_at=?, last_error=?, updated_at=?
                 WHERE delivery_id=?",
                params![
                    WebhookDeliveryStatus::Dlq.db_value(),
                    now.to_rfc3339(),
                    error_message,
                    now.to_rfc3339(),
                    delivery_id
                ],
            )?;
        } else {
            let next_attempt = now + Duration::seconds(retry_delay_seconds.max(5));
            self.conn.lock().expect("db lock").execute(
                "UPDATE linear_webhook_deliveries
                 SET status=?, next_attempt_at=?, last_error=?, updated_at=?
                 WHERE delivery_id=?",
                params![
                    WebhookDeliveryStatus::Retry.db_value(),
                    next_attempt.to_rfc3339(),
                    error_message,
                    now.to_rfc3339(),
                    delivery_id
                ],
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinearOutboxStatus {
    Queued,
    Retry,
    Processing,
    Done,
    Dlq,
}

impl LinearOutboxStatus {
    fn db_value(self) -> &'static str {
        match self {
            LinearOutboxStatus::Queued => "QUEUED",
            LinearOutboxStatus::Retry => "RETRY",
            LinearOutboxStatus::Processing => "PROCESSING",
            LinearOutboxStatus::Done => "DONE",
            LinearOutboxStatus::Dlq => "DLQ",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearOutboxAction {
    pub id: String,
    pub issue_id: String,
    pub action_type: String,
    pub payload: String,
    pub status: String,
    pub attempts: i32,
    pub next_attempt_at: String,
    pub last_error: Option<String>,
    pub created_at: String,
    pub processed_at: Option<String>,
    pub updated_at: String,
}

pub struct LinearOutboxRepository {
    conn: Arc<Mutex<Connection>>,
}

impl LinearOutboxRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn enqueue(&self, issue_id: &str, action_type: &str, payload: &Value) -> Result<String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        self.conn.lock().expect("db lock").execute(
            "INSERT INTO linear_action_outbox (
                id, issue_id, action_type, payload, status, attempts, next_attempt_at,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)",
            params![
                id,
                issue_id,
                action_type,
                serde_json::to_string(payload)?,
                LinearOutboxStatus::Queued.db_value(),
                now,
                now,
                now
            ],
        )?;
        Ok(id)
    }

    pub fn claim_next_ready(&self) -> Option<LinearOutboxAction> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().expect("db lock");
        let row = conn
            .query_row(
                "SELECT id, issue_id, action_type, payload, status, attempts, next_attempt_at,
                        last_error, created_at, processed_at, updated_at
                 FROM linear_action_outbox
                 WHERE status IN ('QUEUED', 'RETRY')
                   AND next_attempt_at <= ?
                 ORDER BY created_at ASC
                 LIMIT 1",
                params![now],
                |row| {
                    Ok(LinearOutboxAction {
                        id: row.get(0)?,
                        issue_id: row.get(1)?,
                        action_type: row.get(2)?,
                        payload: row.get(3)?,
                        status: row.get(4)?,
                        attempts: row.get(5)?,
                        next_attempt_at: row.get(6)?,
                        last_error: row.get(7)?,
                        created_at: row.get(8)?,
                        processed_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .ok()?;

        let affected = conn
            .execute(
                "UPDATE linear_action_outbox
                 SET status=?, attempts=attempts+1, updated_at=?
                 WHERE id=?
                   AND status IN ('QUEUED', 'RETRY')",
                params![
                    LinearOutboxStatus::Processing.db_value(),
                    Utc::now().to_rfc3339(),
                    row.id
                ],
            )
            .ok()?;
        if affected == 0 {
            return None;
        }

        Some(LinearOutboxAction {
            attempts: row.attempts + 1,
            status: LinearOutboxStatus::Processing.db_value().to_string(),
            ..row
        })
    }

    pub fn mark_done(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().expect("db lock").execute(
            "UPDATE linear_action_outbox
             SET status=?, processed_at=?, last_error=NULL, updated_at=?
             WHERE id=?",
            params![LinearOutboxStatus::Done.db_value(), now, now, id],
        )?;
        Ok(())
    }

    pub fn mark_retry_or_dlq(
        &self,
        id: &str,
        attempts: i32,
        max_attempts: i32,
        retry_delay_seconds: i64,
        error_message: &str,
    ) -> Result<()> {
        let now = Utc::now();
        if attempts >= max_attempts {
            self.conn.lock().expect("db lock").execute(
                "UPDATE linear_action_outbox
                 SET status=?, processed_at=?, last_error=?, updated_at=?
                 WHERE id=?",
                params![
                    LinearOutboxStatus::Dlq.db_value(),
                    now.to_rfc3339(),
                    error_message,
                    now.to_rfc3339(),
                    id
                ],
            )?;
        } else {
            let next_attempt = now + Duration::seconds(retry_delay_seconds.max(5));
            self.conn.lock().expect("db lock").execute(
                "UPDATE linear_action_outbox
                 SET status=?, next_attempt_at=?, last_error=?, updated_at=?
                 WHERE id=?",
                params![
                    LinearOutboxStatus::Retry.db_value(),
                    next_attempt.to_rfc3339(),
                    error_message,
                    now.to_rfc3339(),
                    id
                ],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{mpsc, Arc, Barrier},
        thread,
    };

    use super::{JobStatus, JobsRepository, SqliteDb};

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("code247-{name}-{}.db", uuid::Uuid::new_v4()))
    }

    #[test]
    fn stage_lease_transition_sets_and_clears_lease_fields() {
        let db = SqliteDb::open(":memory:").expect("open sqlite");
        db.run_migrations().expect("migrations");
        let mut repo = JobsRepository::new(db.connection(), None);
        let job = repo
            .create_job("issue-1", "{\"title\":\"demo\"}")
            .expect("create job");

        let transitioned = repo.transition_status_with_lease(
            &job.id,
            JobStatus::Pending,
            JobStatus::Planning,
            None,
            Some("worker-a"),
            Some(120),
        );
        assert!(transitioned);

        let lease = repo
            .list_expired_stage_leases(&(chrono::Utc::now() + chrono::Duration::seconds(180)))
            .into_iter()
            .find(|item| item.id == job.id)
            .expect("lease record");
        assert_eq!(lease.status, JobStatus::Planning);
        assert_eq!(lease.lease_owner.as_deref(), Some("worker-a"));
        assert_eq!(lease.stage_attempt, 1);
        assert!(lease.lease_expires_at.is_some());

        let expired = repo.expire_stage_lease(
            &job.id,
            JobStatus::Planning,
            Some("worker-a"),
            lease.lease_expires_at.as_deref(),
            "lease expired",
        );
        assert!(expired);
        assert!(repo
            .list_expired_stage_leases(&(chrono::Utc::now() + chrono::Duration::hours(1)))
            .into_iter()
            .all(|item| item.id != job.id));
    }

    #[test]
    fn renew_stage_lease_rejects_foreign_owner() {
        let db = SqliteDb::open(":memory:").expect("open sqlite");
        db.run_migrations().expect("migrations");
        let mut repo = JobsRepository::new(db.connection(), None);
        let job = repo
            .create_job("issue-2", "{\"title\":\"demo\"}")
            .expect("create job");

        assert!(repo.transition_status_with_lease(
            &job.id,
            JobStatus::Pending,
            JobStatus::Coding,
            None,
            Some("worker-a"),
            Some(120),
        ));

        let renewed = repo.renew_stage_lease(&job.id, JobStatus::Coding, "worker-b", 120);
        assert!(!renewed);

        let lease = repo
            .list_expired_stage_leases(&(chrono::Utc::now() + chrono::Duration::seconds(180)))
            .into_iter()
            .find(|item| item.id == job.id)
            .expect("lease record");
        assert_eq!(lease.lease_owner.as_deref(), Some("worker-a"));
    }

    #[test]
    fn expire_stage_lease_requires_matching_owner_and_expiry_and_is_idempotent() {
        let db = SqliteDb::open(":memory:").expect("open sqlite");
        db.run_migrations().expect("migrations");
        let mut repo = JobsRepository::new(db.connection(), None);
        let job = repo
            .create_job("issue-3", "{\"title\":\"demo\"}")
            .expect("create job");

        assert!(repo.transition_status_with_lease(
            &job.id,
            JobStatus::Pending,
            JobStatus::Reviewing,
            None,
            Some("worker-a"),
            Some(5),
        ));

        let stale = repo
            .list_expired_stage_leases(&(chrono::Utc::now() + chrono::Duration::seconds(10)))
            .into_iter()
            .find(|item| item.id == job.id)
            .expect("stale lease");
        assert!(repo.renew_stage_lease(&job.id, JobStatus::Reviewing, "worker-a", 120));

        let wrongly_expired = repo.expire_stage_lease(
            &job.id,
            JobStatus::Reviewing,
            stale.lease_owner.as_deref(),
            stale.lease_expires_at.as_deref(),
            "stale expire",
        );
        assert!(!wrongly_expired);

        let current = repo
            .list_expired_stage_leases(&(chrono::Utc::now() + chrono::Duration::seconds(180)))
            .into_iter()
            .find(|item| item.id == job.id)
            .expect("current lease");
        let expired = repo.expire_stage_lease(
            &job.id,
            JobStatus::Reviewing,
            current.lease_owner.as_deref(),
            current.lease_expires_at.as_deref(),
            "lease expired",
        );
        assert!(expired);

        let duplicate = repo.expire_stage_lease(
            &job.id,
            JobStatus::Reviewing,
            current.lease_owner.as_deref(),
            current.lease_expires_at.as_deref(),
            "lease expired",
        );
        assert!(!duplicate);
    }

    #[test]
    fn claim_next_pending_with_lease_is_atomic_across_instances() {
        let path = temp_db_path("claim-race");
        let db = SqliteDb::open(path.to_str().expect("db path")).expect("open sqlite");
        db.run_migrations().expect("migrations");
        let mut repo = JobsRepository::new(db.connection(), None);
        let job = repo
            .create_job("issue-4", "{\"title\":\"demo\"}")
            .expect("create job");
        drop(repo);
        drop(db);

        let barrier = Arc::new(Barrier::new(3));
        let (tx, rx) = mpsc::channel();

        for owner in ["worker-a", "worker-b"] {
            let owner = owner.to_string();
            let db_path = path.clone();
            let barrier = barrier.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let db = SqliteDb::open(db_path.to_str().expect("db path")).expect("open sqlite");
                let mut repo = JobsRepository::new(db.connection(), None);
                barrier.wait();
                let claimed = repo.claim_next_pending_with_lease(&owner, 120);
                tx.send((owner, claimed.map(|item| item.id)))
                    .expect("send result");
            });
        }
        drop(tx);
        barrier.wait();

        let results: Vec<_> = rx.iter().collect();
        let winners: Vec<_> = results
            .iter()
            .filter_map(|(owner, job_id)| {
                job_id
                    .as_ref()
                    .map(|job_id| (owner.clone(), job_id.clone()))
            })
            .collect();
        assert_eq!(
            winners.len(),
            1,
            "exactly one instance should claim the pending job"
        );
        assert_eq!(winners[0].1, job.id);

        let db = SqliteDb::open(path.to_str().expect("db path")).expect("open sqlite");
        let repo = JobsRepository::new(db.connection(), None);
        let lease = repo
            .list_expired_stage_leases(&(chrono::Utc::now() + chrono::Duration::seconds(180)))
            .into_iter()
            .find(|item| item.id == job.id)
            .expect("lease record");
        assert_eq!(lease.status, JobStatus::Planning);
        assert_eq!(lease.lease_owner.as_deref(), Some(winners[0].0.as_str()));
        assert_eq!(lease.stage_attempt, 1);

        fs::remove_file(path).expect("cleanup db");
    }
}

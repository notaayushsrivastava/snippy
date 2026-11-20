use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use platform_contract::{RiskCategory, Scope, SecureStorage, Verification};
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Ready,
    Inspecting,
    Planning,
    Acting,
    AwaitingApproval,
    AwaitingSecret,
    Blocked,
    Complete,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineStep {
    pub step_number: u32,
    pub timestamp: DateTime<Utc>,
    pub action_summary: String,
    pub target_label: String,
    pub risk: RiskCategory,
    pub observation_hash: String,
    pub policy_decision: String,
    pub verification: Option<Verification>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRecord {
    pub session_id: String,
    pub task: String,
    pub scope: Scope,
    pub status: SessionStatus,
    pub steps: Vec<TimelineStep>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("storage failure: {0}")]
    Storage(String),
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create_session(&self, task: String, scope: Scope)
        -> Result<SessionRecord, StoreError>;
    async fn get_session(&self, session_id: &str) -> Result<SessionRecord, StoreError>;
    async fn list_sessions(&self) -> Result<Vec<SessionRecord>, StoreError>;
    async fn update_status(
        &self,
        session_id: &str,
        status: SessionStatus,
    ) -> Result<(), StoreError>;
    async fn update_task(&self, session_id: &str, task: String) -> Result<(), StoreError>;
    async fn append_step(&self, session_id: &str, step: TimelineStep) -> Result<(), StoreError>;
    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemorySessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create_session(
        &self,
        task: String,
        scope: Scope,
    ) -> Result<SessionRecord, StoreError> {
        let now = Utc::now();
        let record = SessionRecord {
            session_id: uuid::Uuid::new_v4().to_string(),
            task,
            scope,
            status: SessionStatus::Ready,
            steps: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        let mut guard = self.sessions.write().await;
        guard.insert(record.session_id.clone(), record.clone());
        Ok(record)
    }

    async fn get_session(&self, session_id: &str) -> Result<SessionRecord, StoreError> {
        let guard = self.sessions.read().await;
        guard
            .get(session_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(session_id.to_string()))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionRecord>, StoreError> {
        let guard = self.sessions.read().await;
        let mut items: Vec<SessionRecord> = guard.values().cloned().collect();
        items.sort_by_key(|a| std::cmp::Reverse(a.updated_at));
        Ok(items)
    }

    async fn update_status(
        &self,
        session_id: &str,
        status: SessionStatus,
    ) -> Result<(), StoreError> {
        let mut guard = self.sessions.write().await;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| StoreError::NotFound(session_id.to_string()))?;
        session.status = status;
        session.updated_at = Utc::now();
        Ok(())
    }

    async fn update_task(&self, session_id: &str, task: String) -> Result<(), StoreError> {
        let mut guard = self.sessions.write().await;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| StoreError::NotFound(session_id.to_string()))?;
        session.task = task;
        session.updated_at = Utc::now();
        Ok(())
    }

    async fn append_step(&self, session_id: &str, step: TimelineStep) -> Result<(), StoreError> {
        let mut guard = self.sessions.write().await;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| StoreError::NotFound(session_id.to_string()))?;
        session.steps.push(step);
        session.updated_at = Utc::now();
        Ok(())
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError> {
        let mut guard = self.sessions.write().await;
        guard
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| StoreError::NotFound(session_id.to_string()))
    }
}

/// Encrypted SQLite session store using AES-256-GCM.
pub struct SqliteSessionStore {
    conn: Mutex<Connection>,
    key: [u8; 32],
}

impl SqliteSessionStore {
    pub fn open_with_key(path: impl AsRef<Path>, key: [u8; 32]) -> Result<Self, StoreError> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| StoreError::Storage(e.to_string()))?;
            }
        }
        let conn = Connection::open(path).map_err(|e| StoreError::Storage(e.to_string()))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                updated_at TEXT NOT NULL,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL
            )",
            [],
        )
        .map_err(|e| StoreError::Storage(e.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
            key,
        })
    }

    pub async fn open_with_secure_storage(
        path: impl AsRef<Path>,
        storage: &dyn SecureStorage,
        key_name: &str,
    ) -> Result<Self, StoreError> {
        let key = match storage
            .get_secret(key_name)
            .await
            .map_err(|e| StoreError::Storage(e.to_string()))?
        {
            Some(hex_key) => decode_hex_key(&hex_key)?,
            None => {
                let mut raw = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut raw);
                let hex_key: String = raw.iter().map(|b| format!("{b:02x}")).collect();
                storage
                    .set_secret(key_name, &hex_key)
                    .await
                    .map_err(|e| StoreError::Storage(e.to_string()))?;
                raw
            }
        };
        Self::open_with_key(path, key)
    }

    fn encrypt(&self, record: &SessionRecord) -> Result<(Vec<u8>, Vec<u8>), StoreError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext =
            serde_json::to_vec(record).map_err(|e| StoreError::Storage(e.to_string()))?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        Ok((nonce_bytes.to_vec(), ciphertext))
    }

    fn decrypt(&self, nonce: &[u8], ciphertext: &[u8]) -> Result<SessionRecord, StoreError> {
        if nonce.len() != 12 {
            return Err(StoreError::Storage("invalid nonce length".into()));
        }
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        serde_json::from_slice(&plaintext).map_err(|e| StoreError::Storage(e.to_string()))
    }

    fn save_record(&self, conn: &Connection, record: &SessionRecord) -> Result<(), StoreError> {
        let (nonce, ciphertext) = self.encrypt(record)?;
        conn.execute(
            "INSERT INTO sessions (session_id, updated_at, nonce, ciphertext)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id) DO UPDATE SET
               updated_at = excluded.updated_at,
               nonce = excluded.nonce,
               ciphertext = excluded.ciphertext",
            params![
                record.session_id,
                record.updated_at.to_rfc3339(),
                nonce,
                ciphertext
            ],
        )
        .map_err(|e| StoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn load_record(
        &self,
        conn: &Connection,
        session_id: &str,
    ) -> Result<SessionRecord, StoreError> {
        let mut stmt = conn
            .prepare("SELECT nonce, ciphertext FROM sessions WHERE session_id = ?1")
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        let mut rows = stmt
            .query(params![session_id])
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        let Some(row) = rows
            .next()
            .map_err(|e| StoreError::Storage(e.to_string()))?
        else {
            return Err(StoreError::NotFound(session_id.to_string()));
        };
        let nonce: Vec<u8> = row.get(0).map_err(|e| StoreError::Storage(e.to_string()))?;
        let ciphertext: Vec<u8> = row.get(1).map_err(|e| StoreError::Storage(e.to_string()))?;
        self.decrypt(&nonce, &ciphertext)
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create_session(
        &self,
        task: String,
        scope: Scope,
    ) -> Result<SessionRecord, StoreError> {
        let now = Utc::now();
        let record = SessionRecord {
            session_id: uuid::Uuid::new_v4().to_string(),
            task,
            scope,
            status: SessionStatus::Ready,
            steps: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        let conn = self.conn.lock().unwrap();
        self.save_record(&conn, &record)?;
        Ok(record)
    }

    async fn get_session(&self, session_id: &str) -> Result<SessionRecord, StoreError> {
        let conn = self.conn.lock().unwrap();
        self.load_record(&conn, session_id)
    }

    async fn list_sessions(&self) -> Result<Vec<SessionRecord>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT nonce, ciphertext FROM sessions ORDER BY updated_at DESC")
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| StoreError::Storage(e.to_string()))?
        {
            let nonce: Vec<u8> = row.get(0).map_err(|e| StoreError::Storage(e.to_string()))?;
            let ciphertext: Vec<u8> = row.get(1).map_err(|e| StoreError::Storage(e.to_string()))?;
            out.push(self.decrypt(&nonce, &ciphertext)?);
        }
        Ok(out)
    }

    async fn update_status(
        &self,
        session_id: &str,
        status: SessionStatus,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut record = self.load_record(&conn, session_id)?;
        record.status = status;
        record.updated_at = Utc::now();
        self.save_record(&conn, &record)
    }

    async fn update_task(&self, session_id: &str, task: String) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut record = self.load_record(&conn, session_id)?;
        record.task = task;
        record.updated_at = Utc::now();
        self.save_record(&conn, &record)
    }

    async fn append_step(&self, session_id: &str, step: TimelineStep) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut record = self.load_record(&conn, session_id)?;
        record.steps.push(step);
        record.updated_at = Utc::now();
        self.save_record(&conn, &record)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn
            .execute(
                "DELETE FROM sessions WHERE session_id = ?1",
                params![session_id],
            )
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        if deleted == 0 {
            return Err(StoreError::NotFound(session_id.to_string()));
        }
        let _ = conn.execute("VACUUM", []);
        Ok(())
    }
}

fn decode_hex_key(hex: &str) -> Result<[u8; 32], StoreError> {
    if hex.len() != 64 {
        return Err(StoreError::Storage("invalid hex key length".into()));
    }
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        let slice = &hex[i * 2..i * 2 + 2];
        *byte = u8::from_str_radix(slice, 16)
            .map_err(|_| StoreError::Storage("invalid hex key".into()))?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contract::TargetKind;

    #[tokio::test]
    async fn sqlite_encrypted_round_trip_and_deletion() {
        let dir = std::env::temp_dir().join(format!("snippy-test-{}", uuid::Uuid::new_v4()));
        let db_path = dir.join("sessions.db");
        let key = [42u8; 32];

        let scope = Scope {
            scope_id: "scope-1".into(),
            target_kind: TargetKind::Window,
            app_name: "Notepad".into(),
            window_title: "Notes".into(),
            process_id: Some(100),
        };

        let store = SqliteSessionStore::open_with_key(&db_path, key).unwrap();
        let created = store
            .create_session("Draft notes".into(), scope)
            .await
            .unwrap();
        store
            .update_status(&created.session_id, SessionStatus::Complete)
            .await
            .unwrap();
        drop(store);

        // Reopen from disk and verify encrypted payload decrypts cleanly
        let reopened = SqliteSessionStore::open_with_key(&db_path, key).unwrap();
        let loaded = reopened.get_session(&created.session_id).await.unwrap();
        assert_eq!(loaded.task, "Draft notes");
        assert_eq!(loaded.status, SessionStatus::Complete);

        // Verify deletion is real
        reopened.delete_session(&created.session_id).await.unwrap();
        assert!(matches!(
            reopened.get_session(&created.session_id).await,
            Err(StoreError::NotFound(_))
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use platform_contract::{RiskCategory, Scope, Verification};
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

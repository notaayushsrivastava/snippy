use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use platform_contract::{
    ActionResult, ApprovedAction, ExpectedState, Observation, PermissionRequest, PermissionState,
    PlatformAdapter, PlatformError, Scope, SecureStorage, Verification,
};

use crate::provider::{AgentInput, AgentProposal, ProviderAdapter, ProviderError};

#[derive(Debug, Default)]
pub struct MockSecureStorage {
    secrets: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl SecureStorage for MockSecureStorage {
    async fn get_secret(&self, key: &str) -> Result<Option<String>, PlatformError> {
        Ok(self.secrets.lock().unwrap().get(key).cloned())
    }

    async fn set_secret(&self, key: &str, value: &str) -> Result<(), PlatformError> {
        self.secrets
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete_secret(&self, key: &str) -> Result<(), PlatformError> {
        self.secrets.lock().unwrap().remove(key);
        Ok(())
    }
}

pub struct MockPlatformAdapter {
    observations: Mutex<VecDeque<Result<Observation, PlatformError>>>,
    verifications: Mutex<VecDeque<Verification>>,
    storage: MockSecureStorage,
}

impl MockPlatformAdapter {
    pub fn new(
        observations: Vec<Result<Observation, PlatformError>>,
        verifications: Vec<Verification>,
    ) -> Self {
        Self {
            observations: Mutex::new(VecDeque::from(observations)),
            verifications: Mutex::new(VecDeque::from(verifications)),
            storage: MockSecureStorage::default(),
        }
    }
}

#[async_trait]
impl PlatformAdapter for MockPlatformAdapter {
    async fn request_permissions(&self, _request: PermissionRequest) -> PermissionState {
        PermissionState::Granted
    }

    async fn observe_target(&self, _scope: &Scope) -> Result<Observation, PlatformError> {
        self.observations
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(PlatformError::TargetNotFound("no mock observation".into())))
    }

    async fn execute_action(&self, action: &ApprovedAction) -> Result<ActionResult, PlatformError> {
        Ok(ActionResult {
            action_id: action.action_id.clone(),
            executed_at: Utc::now(),
            summary: format!(
                "Executed {:?} on {:?}",
                action.kind, action.target.semantic_locator
            ),
        })
    }

    async fn verify_result(&self, _expected: &ExpectedState) -> Verification {
        self.verifications
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Verification::Confirmed {
                evidence: "mock confirmed".into(),
            })
    }

    fn secure_storage(&self) -> &dyn SecureStorage {
        &self.storage
    }
}

pub struct MockProviderAdapter {
    proposals: Mutex<VecDeque<Result<AgentProposal, ProviderError>>>,
}

impl MockProviderAdapter {
    pub fn new(proposals: Vec<Result<AgentProposal, ProviderError>>) -> Self {
        Self {
            proposals: Mutex::new(VecDeque::from(proposals)),
        }
    }
}

#[async_trait]
impl ProviderAdapter for MockProviderAdapter {
    async fn propose(&self, _input: AgentInput) -> Result<AgentProposal, ProviderError> {
        self.proposals
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(ProviderError::Unavailable("no mock proposal".into())))
    }
}

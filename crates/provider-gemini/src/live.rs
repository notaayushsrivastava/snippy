use std::sync::Arc;
use std::time::Duration;

use agent_core::provider::{
    AgentInput, AgentProposal, ProposalStatus, ProviderAdapter, ProviderError,
};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::{format_task_prompt, parse_proposal_json, GeminiProvider, SYSTEM_PROMPT};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type FrameCaptureFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub struct GeminiLiveProvider {
    api_key: String,
    model: String,
    session: Arc<Mutex<Option<WsStream>>>,
    capture_frame: FrameCaptureFn,
}

impl GeminiLiveProvider {
    pub fn new(api_key: String, capture_frame: FrameCaptureFn) -> Self {
        Self {
            api_key,
            model: "models/gemini-3.8-live".into(),
            session: Arc::new(Mutex::new(None)),
            capture_frame,
        }
    }

    async fn ensure_connected<'a>(
        &'a self,
        guard: &'a mut Option<WsStream>,
    ) -> Result<&'a mut WsStream, ProviderError> {
        if guard.is_none() {
            let url = format!(
                "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContent?key={}",
                self.api_key
            );
            let (mut ws, _) = tokio::time::timeout(Duration::from_secs(15), connect_async(&url))
                .await
                .map_err(|_| {
                    ProviderError::Timeout("Gemini Live WebSocket connect timed out".into())
                })?
                .map_err(|e| ProviderError::Unavailable(format!("Live connect error: {e}")))?;

            let setup = json!({
                "setup": {
                    "model": self.model,
                    "generationConfig": {
                        "responseModalities": ["AUDIO"],
                        "speechConfig": {
                            "voiceConfig": {
                                "prebuiltVoiceConfig": {
                                    "voiceName": "Puck"
                                }
                            }
                        }
                    },
                    "outputAudioTranscription": {},
                    "systemInstruction": {
                        "parts": [{
                            "text": format!("{SYSTEM_PROMPT}\n\nCRITICAL: On every user turn, you MUST call the `propose_action` tool with `status`, `user_message`, `actions`, and `annotations`. Never end a turn without calling `propose_action`.")
                        }]
                    },
                    "tools": [{
                        "functionDeclarations": [{
                            "name": "propose_action",
                            "description": "Propose the next Snippy UI action, screen annotations, or report completion/blocked status.",
                            "parameters": {
                                "type": "OBJECT",
                                "properties": {
                                    "status": { "type": "STRING" },
                                    "user_message": { "type": "STRING" },
                                    "annotations": {
                                        "type": "ARRAY",
                                        "items": { "type": "OBJECT" }
                                    },
                                    "actions": {
                                        "type": "ARRAY",
                                        "items": { "type": "OBJECT" }
                                    }
                                },
                                "required": ["status", "user_message", "actions"]
                            }
                        }]
                    }]
                }
            });

            ws.send(Message::Text(setup.to_string().into()))
                .await
                .map_err(|e| ProviderError::Unavailable(format!("Live setup send failed: {e}")))?;

            let setup_ok = tokio::time::timeout(Duration::from_secs(15), async {
                while let Some(msg) = ws.next().await {
                    let msg = msg.map_err(|e| ProviderError::Unavailable(e.to_string()))?;
                    let txt = match msg {
                        Message::Text(t) => t.to_string(),
                        Message::Binary(b) => String::from_utf8_lossy(&b).to_string(),
                        Message::Close(c) => {
                            return Err(ProviderError::Unavailable(format!(
                                "Live closed during setup: {c:?}"
                            )));
                        }
                        _ => continue,
                    };
                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                        if v.get("setupComplete").is_some() {
                            return Ok(());
                        }
                    }
                }
                Err(ProviderError::Unavailable(
                    "Live stream ended before setupComplete".into(),
                ))
            })
            .await
            .map_err(|_| ProviderError::Timeout("Gemini Live setup timed out".into()))?;

            setup_ok?;
            *guard = Some(ws);
        }
        Ok(guard.as_mut().unwrap())
    }

    pub async fn push_video_frame(&self, jpeg_b64: &str) -> Result<(), ProviderError> {
        let mut guard = self.session.lock().await;
        let ws = self.ensure_connected(&mut guard).await?;
        let msg = json!({
            "realtimeInput": {
                "video": {
                    "data": jpeg_b64,
                    "mimeType": "image/jpeg"
                }
            }
        });
        ws.send(Message::Text(msg.to_string().into()))
            .await
            .map_err(|e| ProviderError::Unavailable(format!("Live video send failed: {e}")))
    }

    pub async fn close(&self) {
        let mut guard = self.session.lock().await;
        if let Some(mut ws) = guard.take() {
            let _ = ws.close(None).await;
        }
    }
}

fn parse_proposal_from_raw(raw: &str) -> Result<AgentProposal, ProviderError> {
    let stripped = strip_json_fences(raw);
    if let Ok(proposal) = parse_proposal_json(stripped) {
        return Ok(proposal);
    }
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
        if end > start {
            if let Ok(proposal) = parse_proposal_json(&raw[start..=end]) {
                return Ok(proposal);
            }
        }
    }
    parse_proposal_json(stripped)
}

fn strip_json_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.trim().strip_suffix("```").unwrap_or(rest).trim()
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest.trim().strip_suffix("```").unwrap_or(rest).trim()
    } else {
        trimmed
    }
}

#[async_trait]
impl ProviderAdapter for GeminiLiveProvider {
    async fn propose(&self, input: AgentInput) -> Result<AgentProposal, ProviderError> {
        let user_prompt = format_task_prompt(&input)?;
        let maybe_frame = (self.capture_frame)(&input.scope.scope_id);

        let live_attempt = self
            .try_live_turn(&user_prompt, maybe_frame.as_deref())
            .await;

        match live_attempt {
            Ok(proposal) => Ok(proposal),
            Err(ProviderError::InvalidProposal(_)) => {
                // Fall back to structured JSON multimodal generateContent with the same captured frame
                let rest = GeminiProvider::new(self.api_key.clone());
                rest.call_model_with_image(
                    "gemini-3.5-flash-lite",
                    &user_prompt,
                    false,
                    maybe_frame.as_deref(),
                )
                .await
            }
            Err(err) => Err(err),
        }
    }
}

impl GeminiLiveProvider {
    async fn try_live_turn(
        &self,
        user_prompt: &str,
        maybe_frame: Option<&str>,
    ) -> Result<AgentProposal, ProviderError> {
        let mut guard = self.session.lock().await;
        let ws = self.ensure_connected(&mut guard).await?;

        if let Some(jpeg_b64) = maybe_frame {
            let video_msg = json!({
                "realtimeInput": {
                    "video": {
                        "data": jpeg_b64,
                        "mimeType": "image/jpeg"
                    }
                }
            });
            if let Err(e) = ws.send(Message::Text(video_msg.to_string().into())).await {
                *guard = None;
                return Err(ProviderError::Unavailable(format!(
                    "Live video send error: {e}"
                )));
            }
        }

        let text_msg = json!({
            "clientContent": {
                "turns": [{
                    "role": "user",
                    "parts": [{ "text": user_prompt }]
                }],
                "turnComplete": true
            }
        });
        if let Err(e) = ws.send(Message::Text(text_msg.to_string().into())).await {
            *guard = None;
            return Err(ProviderError::Unavailable(format!(
                "Live text send error: {e}"
            )));
        }

        let turn_result = tokio::time::timeout(Duration::from_secs(30), async {
            let mut text_collected = String::new();
            let mut transcript_collected = String::new();
            while let Some(msg) = ws.next().await {
                let msg = msg.map_err(|e| ProviderError::Unavailable(e.to_string()))?;
                let txt = match msg {
                    Message::Text(t) => t.to_string(),
                    Message::Binary(b) => String::from_utf8_lossy(&b).to_string(),
                    Message::Close(c) => {
                        return Err(ProviderError::Unavailable(format!(
                            "Live WebSocket closed: {c:?}"
                        )));
                    }
                    _ => continue,
                };
                let Ok(v) = serde_json::from_str::<Value>(&txt) else {
                    continue;
                };

                if let Some(calls) = v
                    .get("toolCall")
                    .and_then(|t| t.get("functionCalls"))
                    .and_then(|f| f.as_array())
                {
                    for call in calls {
                        if call.get("name").and_then(|n| n.as_str()) == Some("propose_action") {
                            let call_id = call
                                .get("id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("call_1")
                                .to_string();
                            let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
                            let ack = json!({
                                "toolResponse": {
                                    "functionResponses": [{
                                        "id": call_id,
                                        "response": { "output": "proposal_accepted" }
                                    }]
                                }
                            });
                            let _ = ws.send(Message::Text(ack.to_string().into())).await;
                            return serde_json::from_value::<AgentProposal>(args).map_err(|e| {
                                ProviderError::InvalidProposal(format!(
                                    "malformed propose_action args: {e}"
                                ))
                            });
                        }
                    }
                }

                if let Some(sc) = v.get("serverContent") {
                    if let Some(parts) = sc
                        .get("modelTurn")
                        .and_then(|m| m.get("parts"))
                        .and_then(|p| p.as_array())
                    {
                        for p in parts {
                            if let Some(t) = p.get("text").and_then(|x| x.as_str()) {
                                text_collected.push_str(t);
                            }
                        }
                    }
                    if let Some(t) = sc
                        .get("outputTranscription")
                        .and_then(|o| o.get("text"))
                        .and_then(|x| x.as_str())
                    {
                        transcript_collected.push_str(t);
                    }
                    if sc.get("turnComplete").and_then(|b| b.as_bool()) == Some(true) {
                        if !text_collected.trim().is_empty() {
                            return parse_proposal_from_raw(&text_collected);
                        }
                        if !transcript_collected.trim().is_empty() {
                            if let Ok(proposal) = parse_proposal_from_raw(&transcript_collected) {
                                return Ok(proposal);
                            }
                            return Ok(AgentProposal {
                                status: ProposalStatus::Complete,
                                user_message: transcript_collected.trim().to_string(),
                                actions: Vec::new(),
                                annotations: Vec::new(),
                            });
                        }
                        return Err(ProviderError::InvalidProposal(
                            "Gemini Live completed turn without proposal".into(),
                        ));
                    }
                }
            }
            Err(ProviderError::Unavailable(
                "Live WebSocket stream ended unexpectedly".into(),
            ))
        })
        .await
        .map_err(|_| ProviderError::Timeout("Gemini 3.8 Live turn timed out".into()))?;

        if turn_result.is_err() {
            *guard = None;
        }
        turn_result
    }
}

import type { ApprovedAction, Scope, SessionRecord } from '@snippy/protocol';

export type StepOutcome =
  | { outcome: 'executed'; verification: { status: string }; summary: string }
  | { outcome: 'awaiting_approval'; action: ApprovedAction; consequence: string }
  | { outcome: 'awaiting_secret'; reason: string }
  | { outcome: 'blocked'; reason: string }
  | { outcome: 'complete'; summary: string }
  | { outcome: 'stopped' };

export interface StepResponse {
  session: SessionRecord;
  outcome: StepOutcome;
}

type TauriInternals = {
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
};

function getTauri(): TauriInternals | null {
  const win = window as unknown as { __TAURI_INTERNALS__?: TauriInternals };
  return win.__TAURI_INTERNALS__ ?? null;
}

const DEFAULT_FALLBACK_SCOPE: Scope = {
  scope_id: 'scope-chrome-research',
  target_kind: 'browser_tab',
  app_name: 'Chrome',
  window_title: 'Research Tab',
  process_id: 4242,
};

let memorySessions: SessionRecord[] = [];

export async function listWindows(): Promise<Scope[]> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<Scope[]>('list_windows');
  return [
    DEFAULT_FALLBACK_SCOPE,
    {
      scope_id: 'scope-vscode-workspace',
      target_kind: 'companion_context',
      app_name: 'VS Code',
      window_title: 'snippy — Visual Studio Code',
      process_id: 8100,
    },
  ];
}

export async function listSessions(): Promise<SessionRecord[]> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<SessionRecord[]>('list_sessions');
  return [...memorySessions];
}

export async function deleteSession(sessionId: string): Promise<void> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<void>('delete_session', { sessionId });
  memorySessions = memorySessions.filter((s) => s.session_id !== sessionId);
}

export type ProviderKind = 'demo' | 'openai' | 'gemini' | 'gemini_live';

export async function getDefaultProvider(): Promise<ProviderKind> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<ProviderKind>('get_default_provider');
  return 'demo';
}

export async function setProviderKey(provider: ProviderKind, apiKey: string): Promise<void> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<void>('set_provider_key', { provider, apiKey });
}

export async function hasProviderKey(provider: ProviderKind): Promise<boolean> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<boolean>('has_provider_key', { provider });
  return false;
}

export async function deleteProviderKey(provider: ProviderKind): Promise<void> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<void>('delete_provider_key', { provider });
}

export async function startAndStep(
  task: string,
  scope: Scope,
  provider: ProviderKind = 'demo',
): Promise<StepResponse> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<StepResponse>('start_and_step', { task, scope, provider });

  const now = new Date().toISOString();
  const session: SessionRecord = {
    session_id: `mock-${Date.now()}`,
    task,
    scope,
    status: 'awaiting_approval',
    steps: [
      {
        step_number: 1,
        timestamp: now,
        action_summary: 'Focused target input',
        target_label: 'search-input',
        risk: 'navigate',
        observation_hash: `obs-${scope.scope_id}`,
        policy_decision: 'allow',
        verification: { status: 'confirmed', evidence: 'Input field has focus' },
      },
    ],
    created_at: now,
    updated_at: now,
  };
  memorySessions = [session, ...memorySessions];
  const action: ApprovedAction = {
    protocol_version: 1,
    action_id: `act-${Date.now()}`,
    scope_id: scope.scope_id,
    observation_hash: `obs-${scope.scope_id}`,
    kind: 'invoke',
    target: {
      observation_hash: `obs-${scope.scope_id}`,
      semantic_locator: 'submit-btn',
      coordinate_hint: null,
    },
    arguments: {},
    risk: 'external_impact',
    reason: `Submit requested action in ${scope.window_title}`,
    expected_postcondition: 'Action submitted in target window',
    expires_at: new Date(Date.now() + 120_000).toISOString(),
  };
  return {
    session,
    outcome: { outcome: 'awaiting_approval', action, consequence: action.expected_postcondition },
  };
}

export async function continueSession(
  sessionId: string,
  followUp: string,
  provider: ProviderKind = 'demo',
): Promise<StepResponse> {
  const tauri = getTauri();
  if (tauri) {
    return tauri.invoke<StepResponse>('continue_session', {
      sessionId,
      followUp,
      provider,
    });
  }
  const existing = memorySessions.find((s) => s.session_id === sessionId);
  if (!existing) throw new Error('Session not found');
  return {
    session: { ...existing, task: followUp, status: 'complete' },
    outcome: { outcome: 'complete', summary: `Follow-up completed: ${followUp}` },
  };
}

export async function stepSession(
  sessionId: string,
  provider: ProviderKind = 'demo',
): Promise<StepResponse> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<StepResponse>('step_session', { sessionId, provider });
  const existing = memorySessions.find((s) => s.session_id === sessionId);
  if (!existing) throw new Error('Session not found');
  return { session: existing, outcome: { outcome: 'complete', summary: 'Completed' } };
}

export async function approveAction(
  sessionId: string,
  actionId: string,
  provider: ProviderKind = 'demo',
): Promise<StepResponse> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<StepResponse>('approve_action', { sessionId, actionId, provider });
  memorySessions = memorySessions.map((s) =>
    s.session_id === sessionId ? { ...s, status: 'complete' } : s,
  );
  const updated = memorySessions.find((s) => s.session_id === sessionId)!;
  return {
    session: updated,
    outcome: { outcome: 'executed', verification: { status: 'confirmed' }, summary: 'Submitted' },
  };
}

export async function rejectAction(sessionId: string): Promise<SessionRecord> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<SessionRecord>('reject_action', { sessionId });
  memorySessions = memorySessions.map((s) =>
    s.session_id === sessionId ? { ...s, status: 'blocked' } : s,
  );
  return memorySessions.find((s) => s.session_id === sessionId)!;
}

export async function stopTask(sessionId: string): Promise<SessionRecord> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<SessionRecord>('stop_task', { sessionId });
  memorySessions = memorySessions.map((s) =>
    s.session_id === sessionId ? { ...s, status: 'stopped' } : s,
  );
  return memorySessions.find((s) => s.session_id === sessionId)!;
}

export async function hideOverlayHighlight(): Promise<void> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<void>('hide_overlay_highlight');
}

export async function setWorkingDock(working: boolean): Promise<void> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<void>('set_working_dock', { working });
}

export interface UpdateInfo {
  version: string;
  body: string | null;
}

export async function checkForUpdate(): Promise<UpdateInfo | null> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<UpdateInfo | null>('check_for_update');
  return null;
}

export async function installUpdate(): Promise<void> {
  const tauri = getTauri();
  if (tauri) return tauri.invoke<void>('install_update');
}

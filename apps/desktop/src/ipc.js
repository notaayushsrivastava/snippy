function getTauri() {
    const win = window;
    return win.__TAURI_INTERNALS__ ?? null;
}
const DEFAULT_FALLBACK_SCOPE = {
    scope_id: 'scope-chrome-research',
    target_kind: 'browser_tab',
    app_name: 'Chrome',
    window_title: 'Research Tab',
    process_id: 4242,
};
let memorySessions = [];
export async function listWindows() {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('list_windows');
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
export async function listSessions() {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('list_sessions');
    return [...memorySessions];
}
export async function deleteSession(sessionId) {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('delete_session', { sessionId });
    memorySessions = memorySessions.filter((s) => s.session_id !== sessionId);
}
export async function getDefaultProvider() {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('get_default_provider');
    return 'demo';
}
export async function setProviderKey(provider, apiKey) {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('set_provider_key', { provider, apiKey });
}
export async function hasProviderKey(provider) {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('has_provider_key', { provider });
    return false;
}
export async function deleteProviderKey(provider) {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('delete_provider_key', { provider });
}
export async function startAndStep(task, scope, provider = 'demo') {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('start_and_step', { task, scope, provider });
    const now = new Date().toISOString();
    const session = {
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
    const action = {
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
export async function continueSession(sessionId, followUp, provider = 'demo') {
    const tauri = getTauri();
    if (tauri) {
        return tauri.invoke('continue_session', {
            sessionId,
            followUp,
            provider,
        });
    }
    const existing = memorySessions.find((s) => s.session_id === sessionId);
    if (!existing)
        throw new Error('Session not found');
    return {
        session: { ...existing, task: followUp, status: 'complete' },
        outcome: { outcome: 'complete', summary: `Follow-up completed: ${followUp}` },
    };
}
export async function stepSession(sessionId, provider = 'demo') {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('step_session', { sessionId, provider });
    const existing = memorySessions.find((s) => s.session_id === sessionId);
    if (!existing)
        throw new Error('Session not found');
    return { session: existing, outcome: { outcome: 'complete', summary: 'Completed' } };
}
export async function approveAction(sessionId, actionId, provider = 'demo') {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('approve_action', { sessionId, actionId, provider });
    memorySessions = memorySessions.map((s) => s.session_id === sessionId ? { ...s, status: 'complete' } : s);
    const updated = memorySessions.find((s) => s.session_id === sessionId);
    return {
        session: updated,
        outcome: { outcome: 'executed', verification: { status: 'confirmed' }, summary: 'Submitted' },
    };
}
export async function rejectAction(sessionId) {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('reject_action', { sessionId });
    memorySessions = memorySessions.map((s) => s.session_id === sessionId ? { ...s, status: 'blocked' } : s);
    return memorySessions.find((s) => s.session_id === sessionId);
}
export async function stopTask(sessionId) {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('stop_task', { sessionId });
    memorySessions = memorySessions.map((s) => s.session_id === sessionId ? { ...s, status: 'stopped' } : s);
    return memorySessions.find((s) => s.session_id === sessionId);
}
export async function hideOverlayHighlight() {
    const tauri = getTauri();
    if (tauri)
        return tauri.invoke('hide_overlay_highlight');
}

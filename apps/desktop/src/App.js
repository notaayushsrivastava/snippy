import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useEffect, useState } from 'react';
import { ActionCard, ApprovalCard, ScopeChip, SecretHandoff, StopControl, TimelineItem, } from '@snippy/ui';
import { approveAction, deleteProviderKey, deleteSession, hasProviderKey, listSessions, listWindows, rejectAction, setProviderKey, startAndStep, stepSession, stopTask, } from './ipc.js';
export function App() {
    const [theme, setTheme] = useState('dark');
    const [windows, setWindows] = useState([]);
    const [selectedScope, setSelectedScope] = useState(null);
    const [sessions, setSessions] = useState([]);
    const [activeSession, setActiveSession] = useState(null);
    const [pendingApproval, setPendingApproval] = useState(null);
    const [secretReason, setSecretReason] = useState(null);
    const [task, setTask] = useState('');
    const [provider, setProvider] = useState('demo');
    const [apiKeyInput, setApiKeyInput] = useState('');
    const [hasKey, setHasKey] = useState(false);
    const [errorMsg, setErrorMsg] = useState(null);
    useEffect(() => {
        void (async () => {
            const [winList, sessList] = await Promise.all([listWindows(), listSessions()]);
            setWindows(winList);
            setSelectedScope(winList[0] ?? null);
            setSessions(sessList);
        })();
    }, []);
    useEffect(() => {
        if (provider === 'demo') {
            setHasKey(false);
            return;
        }
        void hasProviderKey(provider).then(setHasKey).catch(() => setHasKey(false));
    }, [provider]);
    const refreshSessions = async () => {
        setSessions(await listSessions());
    };
    const applyOutcome = (session, outcome) => {
        setActiveSession(session);
        if (outcome.outcome === 'awaiting_approval') {
            setPendingApproval({ action: outcome.action, consequence: outcome.consequence });
            setSecretReason(null);
        }
        else if (outcome.outcome === 'awaiting_secret') {
            setSecretReason(outcome.reason);
            setPendingApproval(null);
        }
        else {
            setPendingApproval(null);
            setSecretReason(null);
        }
    };
    const handleStart = async () => {
        if (!task.trim() || !selectedScope)
            return;
        setErrorMsg(null);
        try {
            let res = await startAndStep(task.trim(), selectedScope, provider);
            if (res.outcome.outcome === 'executed') {
                res = await stepSession(res.session.session_id, provider);
            }
            applyOutcome(res.session, res.outcome);
            await refreshSessions();
        }
        catch (err) {
            setErrorMsg(String(err));
        }
    };
    const handleApprove = async () => {
        if (!activeSession || !pendingApproval)
            return;
        setErrorMsg(null);
        try {
            const res = await approveAction(activeSession.session_id, pendingApproval.action.action_id, provider);
            applyOutcome(res.session, res.outcome);
            await refreshSessions();
        }
        catch (err) {
            setErrorMsg(String(err));
        }
    };
    const handleReject = async () => {
        if (!activeSession)
            return;
        const updated = await rejectAction(activeSession.session_id);
        setActiveSession(updated);
        setPendingApproval(null);
        await refreshSessions();
    };
    const handleStop = async () => {
        if (!activeSession)
            return;
        const updated = await stopTask(activeSession.session_id);
        setActiveSession(updated);
        setPendingApproval(null);
        setSecretReason(null);
        await refreshSessions();
    };
    const handleDeleteSession = async (sessionId) => {
        await deleteSession(sessionId);
        if (activeSession?.session_id === sessionId) {
            setActiveSession(null);
            setPendingApproval(null);
        }
        await refreshSessions();
    };
    const isRunning = activeSession !== null &&
        activeSession.status !== 'ready' &&
        activeSession.status !== 'complete' &&
        activeSession.status !== 'stopped' &&
        activeSession.status !== 'blocked';
    useEffect(() => {
        if (!isRunning)
            return;
        const onKeyDown = (e) => {
            if (e.key === 'Escape') {
                void handleStop();
            }
        };
        window.addEventListener('keydown', onKeyDown);
        return () => window.removeEventListener('keydown', onKeyDown);
    });
    return (_jsxs("div", { "data-theme": theme, style: {
            minHeight: '100vh',
            background: 'var(--surface-canvas)',
            color: 'var(--content-primary)',
            fontFamily: 'system-ui, -apple-system, sans-serif',
            padding: 'var(--space-4)',
            display: 'grid',
            gap: 'var(--space-4)',
            gridTemplateRows: 'auto 1fr',
        }, children: [_jsxs("header", { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 'var(--space-2)' }, children: [_jsx("h1", { style: { fontSize: '20px', margin: 0 }, children: "Snippy" }), _jsxs("div", { style: { display: 'flex', gap: 'var(--space-2)', alignItems: 'center', flexWrap: 'wrap' }, children: [selectedScope ? _jsx(ScopeChip, { scope: selectedScope }) : null, _jsx("button", { type: "button", onClick: () => setTheme(theme === 'dark' ? 'light' : 'dark'), style: {
                                    background: 'var(--surface-panel)',
                                    color: 'var(--content-primary)',
                                    border: '1px solid var(--border-subtle)',
                                    borderRadius: 'var(--radius-sm)',
                                    padding: 'var(--space-1) var(--space-3)',
                                    cursor: 'pointer',
                                }, children: theme === 'dark' ? 'Light' : 'Dark' }), isRunning ? _jsx(StopControl, { onStop: () => void handleStop() }) : null] })] }), _jsxs("div", { style: { display: 'grid', gridTemplateColumns: 'minmax(0, 1fr)', gap: 'var(--space-4)', maxWidth: '560px' }, children: [_jsxs("section", { style: { display: 'flex', gap: 'var(--space-2)', alignItems: 'center', flexWrap: 'wrap' }, children: [_jsx("label", { htmlFor: "provider-select", style: { fontSize: '14px', color: 'var(--content-secondary)' }, children: "Provider" }), _jsxs("select", { id: "provider-select", value: provider, onChange: (e) => setProvider(e.target.value), style: {
                                    padding: 'var(--space-1) var(--space-2)',
                                    borderRadius: 'var(--radius-sm)',
                                    border: '1px solid var(--border-subtle)',
                                    background: 'var(--surface-panel)',
                                    color: 'var(--content-primary)',
                                }, children: [_jsx("option", { value: "demo", children: "Demo (Local Mock)" }), _jsx("option", { value: "openai", children: "OpenAI" }), _jsx("option", { value: "gemini", children: "Gemini" })] }), provider !== 'demo' ? (_jsxs(_Fragment, { children: [_jsx("input", { type: "password", value: apiKeyInput, onChange: (e) => setApiKeyInput(e.target.value), placeholder: hasKey ? 'Key saved (enter to replace)' : `Enter ${provider} API key`, "aria-label": "Provider API key", style: {
                                            flex: 1,
                                            padding: 'var(--space-1) var(--space-2)',
                                            borderRadius: 'var(--radius-sm)',
                                            border: '1px solid var(--border-subtle)',
                                            background: 'var(--surface-panel)',
                                            color: 'var(--content-primary)',
                                        } }), _jsx("button", { type: "button", onClick: () => {
                                            if (!apiKeyInput.trim())
                                                return;
                                            void setProviderKey(provider, apiKeyInput).then(() => {
                                                setApiKeyInput('');
                                                setHasKey(true);
                                            });
                                        }, style: {
                                            background: 'var(--surface-panel)',
                                            color: 'var(--content-primary)',
                                            border: '1px solid var(--border-subtle)',
                                            borderRadius: 'var(--radius-sm)',
                                            padding: 'var(--space-1) var(--space-3)',
                                            cursor: 'pointer',
                                        }, children: "Save Key" }), hasKey ? (_jsx("button", { type: "button", onClick: () => void deleteProviderKey(provider).then(() => setHasKey(false)), style: {
                                            background: 'none',
                                            color: 'var(--content-secondary)',
                                            border: '1px solid var(--border-subtle)',
                                            borderRadius: 'var(--radius-sm)',
                                            padding: 'var(--space-1) var(--space-2)',
                                            cursor: 'pointer',
                                        }, children: "Clear" })) : null] })) : null] }), errorMsg ? (_jsx("p", { role: "alert", style: { color: 'var(--status-danger)', margin: 0, fontSize: '14px' }, children: errorMsg })) : null, _jsxs("section", { style: { display: 'grid', gap: 'var(--space-2)' }, children: [_jsx("label", { htmlFor: "scope-select", style: { fontSize: '14px', color: 'var(--content-secondary)' }, children: "Target window" }), _jsx("select", { id: "scope-select", value: selectedScope?.scope_id ?? '', onChange: (e) => {
                                    const found = windows.find((w) => w.scope_id === e.target.value) ?? null;
                                    setSelectedScope(found);
                                }, style: {
                                    padding: 'var(--space-2)',
                                    borderRadius: 'var(--radius-sm)',
                                    border: '1px solid var(--border-subtle)',
                                    background: 'var(--surface-panel)',
                                    color: 'var(--content-primary)',
                                }, children: windows.map((w) => (_jsxs("option", { value: w.scope_id, children: [w.app_name, " \u2014 ", w.window_title] }, w.scope_id))) }), _jsxs("div", { style: { display: 'flex', gap: 'var(--space-2)' }, children: [_jsx("input", { type: "text", value: task, onChange: (e) => setTask(e.target.value), placeholder: "Tell Snippy what you need done...", "aria-label": "Task description", style: {
                                            flex: 1,
                                            fontSize: '16px',
                                            padding: 'var(--space-2) var(--space-3)',
                                            borderRadius: 'var(--radius-sm)',
                                            border: '1px solid var(--border-subtle)',
                                            background: 'var(--surface-panel)',
                                            color: 'var(--content-primary)',
                                        } }), _jsx("button", { type: "button", onClick: () => void handleStart(), style: {
                                            background: 'var(--button-primary-bg)',
                                            color: 'var(--white)',
                                            border: 'none',
                                            borderRadius: 'var(--radius-sm)',
                                            padding: 'var(--space-2) var(--space-4)',
                                            fontWeight: 600,
                                            cursor: 'pointer',
                                        }, children: "Start" })] })] }), pendingApproval ? (_jsx(ApprovalCard, { action: pendingApproval.action, consequence: pendingApproval.consequence, onApprove: () => void handleApprove(), onTakeOver: () => {
                            setPendingApproval(null);
                            setSecretReason('User takeover requested.');
                        }, onReject: () => void handleReject() })) : null, secretReason ? (_jsx(SecretHandoff, { reason: secretReason, onContinue: () => setSecretReason(null), onStop: () => void handleStop() })) : null, activeSession?.status === 'complete' ? (_jsx(ActionCard, { stepNumber: activeSession.steps.length, verb: "Verified", target: activeSession.scope.window_title, reason: "Task completed within approved scope.", expectedResult: "All steps verified", risk: "observe" })) : null, activeSession ? (_jsxs("section", { "aria-label": "Inspector", children: [_jsxs("p", { style: { fontSize: '14px', color: 'var(--content-secondary)', margin: '0 0 var(--space-2)' }, children: ["Only the selected ", activeSession.scope.app_name, " window is being observed. Status: ", _jsx("strong", { children: activeSession.status })] }), _jsx("ul", { style: { listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 'var(--space-2)' }, children: activeSession.steps.map((step) => (_jsx(TimelineItem, { step: step }, step.step_number))) })] })) : null, _jsxs("section", { "aria-label": "Recent sessions", children: [_jsx("h2", { style: { fontSize: '16px', margin: '0 0 var(--space-2)' }, children: "Recent local sessions" }), _jsx("ul", { style: { listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 'var(--space-2)' }, children: sessions.map((s) => (_jsxs("li", { style: {
                                        display: 'flex',
                                        justifyContent: 'space-between',
                                        alignItems: 'center',
                                        padding: 'var(--space-2) var(--space-3)',
                                        background: 'var(--surface-panel)',
                                        border: '1px solid var(--border-subtle)',
                                        borderRadius: 'var(--radius-sm)',
                                    }, children: [_jsxs("button", { type: "button", onClick: () => setActiveSession(s), style: {
                                                background: 'none',
                                                border: 'none',
                                                color: 'var(--content-primary)',
                                                textAlign: 'left',
                                                cursor: 'pointer',
                                                flex: 1,
                                            }, children: [s.task, " (", s.status, ")"] }), _jsx("button", { type: "button", onClick: () => void handleDeleteSession(s.session_id), "aria-label": `Delete session ${s.task}`, style: {
                                                background: 'none',
                                                border: '1px solid var(--border-subtle)',
                                                borderRadius: 'var(--radius-sm)',
                                                color: 'var(--content-secondary)',
                                                padding: '2px 8px',
                                                cursor: 'pointer',
                                            }, children: "Delete" })] }, s.session_id))) })] })] })] }));
}

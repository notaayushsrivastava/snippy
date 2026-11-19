import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect, useState } from 'react';
import { ActionCard, ApprovalCard, ScopeChip, SecretHandoff, StopControl, TimelineItem, } from '@snippy/ui';
import { approveAction, deleteSession, listSessions, listWindows, rejectAction, startAndStep, stepSession, stopTask, } from './ipc.js';
export function App() {
    const [theme, setTheme] = useState('dark');
    const [windows, setWindows] = useState([]);
    const [selectedScope, setSelectedScope] = useState(null);
    const [sessions, setSessions] = useState([]);
    const [activeSession, setActiveSession] = useState(null);
    const [pendingApproval, setPendingApproval] = useState(null);
    const [secretReason, setSecretReason] = useState(null);
    const [task, setTask] = useState('');
    useEffect(() => {
        void (async () => {
            const [winList, sessList] = await Promise.all([listWindows(), listSessions()]);
            setWindows(winList);
            setSelectedScope(winList[0] ?? null);
            setSessions(sessList);
        })();
    }, []);
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
        let res = await startAndStep(task.trim(), selectedScope);
        if (res.outcome.outcome === 'executed') {
            res = await stepSession(res.session.session_id);
        }
        applyOutcome(res.session, res.outcome);
        await refreshSessions();
    };
    const handleApprove = async () => {
        if (!activeSession || !pendingApproval)
            return;
        const res = await approveAction(activeSession.session_id, pendingApproval.action.action_id);
        applyOutcome(res.session, res.outcome);
        await refreshSessions();
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
                                }, children: theme === 'dark' ? 'Light' : 'Dark' }), isRunning ? _jsx(StopControl, { onStop: () => void handleStop() }) : null] })] }), _jsxs("div", { style: { display: 'grid', gridTemplateColumns: 'minmax(0, 1fr)', gap: 'var(--space-4)', maxWidth: '560px' }, children: [_jsxs("section", { style: { display: 'grid', gap: 'var(--space-2)' }, children: [_jsx("label", { htmlFor: "scope-select", style: { fontSize: '14px', color: 'var(--content-secondary)' }, children: "Target window" }), _jsx("select", { id: "scope-select", value: selectedScope?.scope_id ?? '', onChange: (e) => {
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
